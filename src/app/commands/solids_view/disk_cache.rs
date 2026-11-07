//! Solids results kept on disk between sessions, so opening a project does not
//! cut its solids again.
//!
//! A survey-sized solid takes minutes to build and is the same every time its
//! surfaces and plan are. Each stage's product is written to the cache
//! directory under a key made from everything that decides it: the content of
//! both surfaces, the solid's kind and plan, and the program that built it,
//! taken from the executable itself, so a build whose cutter differs can never
//! read another's results. A file that fails its checksum, or was written by
//! another format, is ignored and rebuilt.
//!
//! The cache is the user's cache directory, shared by every project; the least
//! recently used files go once it passes [`CAPACITY`]. Native builds only: the
//! browser rebuilds, as it always has.

use std::sync::Arc;

use glam::DVec2;

use super::{BlastFace, Carving, CutBand, DigPiece, SolidBlasting, SolidBody, SolidPart, SolidPartition};
use crate::{
    app::commands::{
        solids::preview_triangulation,
        triangulation::solid_between::{BuiltFrom, SolidMesh},
    },
    model::{
        DigBlockId, Solid, SolidId,
        formats::mesh_data::{Triangulation, Vertex},
        triangulation::{OpenTriangulation, TriangulationId},
    },
    ui::state::{BenchSelection, BlastShapeRef},
};

/// Raised whenever what a file holds, or how, changes.
const FORMAT: u32 = 3;
#[cfg(not(target_arch = "wasm32"))]
const MAGIC: &[u8; 8] = b"INCLSOLD";

/// The most the cache may hold before its least recently used files go.
#[cfg(not(target_arch = "wasm32"))]
const CAPACITY: u64 = 12 << 30;

/// Which stage's product a file holds.
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Envelope,
    Body,
    Blasting,
    Partition,
}

impl Kind {
    #[cfg(not(target_arch = "wasm32"))]
    fn name(self) -> &'static str {
        match self {
            Self::Envelope => "envelope",
            Self::Body => "body",
            Self::Blasting => "blasting",
            Self::Partition => "partition",
        }
    }
}

/// The key a solid built between two surfaces is kept under: what it was
/// built from, with the program that built it. `None` where nothing can be
/// kept.
pub(crate) fn solid_mesh_key(built_from: BuiltFrom) -> Option<u64> {
    use std::hash::Hasher;
    let mut key = twox_hash::XxHash3_64::with_seed(program()?);
    key.write_u32(FORMAT);
    key.write(b"solid between surfaces");
    key.write_u64(built_from.0);
    key.write_u64(built_from.1);
    key.write_u8(u8::from(built_from.2));
    Some(key.finish())
}

/// The key a solid's later stages are kept under, each from the one before
/// (see [`super::body_key`]): its envelope, and the solid it belongs to,
/// whose identity its blasts and dig blocks carry.
pub(super) fn chain_key(solid: &Solid, built_from: BuiltFrom) -> Option<u64> {
    use std::hash::Hasher;
    let mut key = twox_hash::XxHash3_64::with_seed(solid_mesh_key(built_from)?);
    key.write_u64(solid.id.0);
    key.write_u8(solid.kind as u8);
    Some(key.finish())
}

/// A hash of the running executable, read once: results are only ever read
/// back by the build that wrote them.
#[cfg(not(target_arch = "wasm32"))]
fn program() -> Option<u64> {
    static PROGRAM: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *PROGRAM.get_or_init(|| {
        use std::{hash::Hasher, io::Read};
        let mut file = std::fs::File::open(std::env::current_exe().ok()?).ok()?;
        let mut hash = twox_hash::XxHash3_64::new();
        let mut buffer = vec![0; 1 << 20];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => hash.write(&buffer[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => {
                    log::warn!("Solids cache off: could not read the program to key it: {error}");
                    return None;
                }
            }
        }
        Some(hash.finish())
    })
}

#[cfg(target_arch = "wasm32")]
fn program() -> Option<u64> {
    None
}

/// Read back what [`store`] kept under `key`, or `None` if nothing usable is
/// there.
pub(crate) fn load<T>(kind: Kind, key: u64, read: impl FnOnce(&mut Reader) -> Option<T>) -> Option<T> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let started = std::time::Instant::now();
        let path = directory()?.join(file_name(kind, key));
        let file = std::fs::File::open(&path).ok()?;
        let length = file.metadata().ok()?.len();
        let mut reader = Reader::new(Box::new(file), length)?;
        let value = (|| {
            let mut magic = [0; 8];
            reader.fill(&mut magic)?;
            (&magic == MAGIC && reader.u32()? == FORMAT && reader.u64()? == key).then_some(())?;
            let value = read(&mut reader)?;
            reader.finish().then_some(value)
        })();
        match &value {
            Some(_) => {
                // Marks it recently used.
                let _ = std::fs::File::options()
                    .append(true)
                    .open(&path)
                    .and_then(|file| file.set_modified(std::time::SystemTime::now()));
                log::info!("Solids cache: read the {} from {} in {:.2?}", kind.name(), path.display(), started.elapsed());
            }
            None => {
                log::warn!("Solids cache: {} is unreadable or damaged; rebuilding it", path.display());
                let _ = std::fs::remove_file(&path);
            }
        }
        value
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (kind, key, read);
        None
    }
}

/// Keep a product under `key`, written beside the cache and moved into place
/// whole, so a reader never sees half of one. Failure only means the next
/// open builds it again, so it is logged and otherwise ignored.
pub(crate) fn store(kind: Kind, key: u64, write: impl FnOnce(&mut Writer) -> Option<()>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let started = std::time::Instant::now();
        let Some(directory) = directory() else { return };
        let result = (|| -> anyhow::Result<u64> {
            let mut temporary = tempfile::Builder::new().prefix(".writing-").tempfile_in(directory)?;
            let mut sink = std::io::BufWriter::with_capacity(BUFFER, temporary.as_file_mut());
            let mut writer = Writer::new(&mut sink);
            writer.bytes(MAGIC);
            writer.u32(FORMAT);
            writer.u64(key);
            anyhow::ensure!(write(&mut writer).is_some(), "the {} holds something the cache does not keep", kind.name());
            let size = writer.finish()?;
            drop(sink);
            temporary.persist(directory.join(file_name(kind, key))).map_err(|error| error.error)?;
            Ok(size)
        })();
        match result {
            Ok(size) => {
                log::info!("Solids cache: kept the {} ({} MB) in {:.2?}", kind.name(), size >> 20, started.elapsed());
                evict(directory);
            }
            Err(error) => log::warn!("Solids cache: could not keep the {}: {error:#}", kind.name()),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (kind, key, write);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn file_name(kind: Kind, key: u64) -> String {
    format!("{}-{key:016x}.bin", kind.name())
}

#[cfg(not(target_arch = "wasm32"))]
fn directory() -> Option<&'static std::path::Path> {
    static DIRECTORY: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            let directory = dirs::cache_dir()?.join("Incline_Design").join("solids");
            match std::fs::create_dir_all(&directory) {
                Ok(()) => Some(directory),
                Err(error) => {
                    log::warn!("Solids cache off: could not create {}: {error}", directory.display());
                    None
                }
            }
        })
        .as_deref()
}

/// Remove the least recently used files until the cache fits [`CAPACITY`].
/// Unfinished writes of a session that stopped part way go as well, once
/// they are an hour old.
#[cfg(not(target_arch = "wasm32"))]
fn evict(directory: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(directory) else { return };
    let now = std::time::SystemTime::now();
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else { continue };
        let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with(".writing-") {
            if now.duration_since(modified).is_ok_and(|age| age.as_secs() > 3600) {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        files.push((modified, metadata.len(), path));
    }
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in files {
        if total <= CAPACITY {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
}

/// Encodes values little-endian into a buffer that is hashed and written a
/// megabyte at a time, with the hash of it all at the end.
pub(crate) struct Writer<'a> {
    sink: &'a mut dyn std::io::Write,
    buffer: Vec<u8>,
    hash: twox_hash::XxHash3_64,
    written: u64,
    error: Option<std::io::Error>,
}

const BUFFER: usize = 1 << 20;

impl<'a> Writer<'a> {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(sink: &'a mut dyn std::io::Write) -> Self {
        Self {
            sink,
            buffer: Vec::with_capacity(BUFFER),
            hash: twox_hash::XxHash3_64::new(),
            written: 0,
            error: None,
        }
    }

    fn flush(&mut self) {
        use std::hash::Hasher;
        self.hash.write(&self.buffer);
        self.written += self.buffer.len() as u64;
        if self.error.is_none()
            && let Err(error) = self.sink.write_all(&self.buffer)
        {
            self.error = Some(error);
        }
        self.buffer.clear();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn finish(mut self) -> anyhow::Result<u64> {
        use std::hash::Hasher;
        self.flush();
        if let Some(error) = self.error {
            return Err(error.into());
        }
        self.sink.write_all(&self.hash.finish().to_le_bytes())?;
        self.sink.flush()?;
        Ok(self.written + 8)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        if self.buffer.len() + bytes.len() > BUFFER {
            self.flush();
        }
        self.buffer.extend_from_slice(bytes);
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    pub(super) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    pub(super) fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    pub(super) fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    pub(super) fn string(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes(value.as_bytes());
    }
}

/// Reads what a [`Writer`] wrote, hashing as it goes. Every length is checked
/// against what the file has left before anything is allocated for it, so a
/// damaged file is found out rather than believed.
pub(crate) struct Reader {
    source: Box<dyn std::io::Read>,
    buffer: Vec<u8>,
    position: usize,
    /// Bytes of the body, the closing hash aside, not yet in `buffer`.
    unread: u64,
    hash: twox_hash::XxHash3_64,
}

impl Reader {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(source: Box<dyn std::io::Read>, length: u64) -> Option<Self> {
        Some(Self {
            source,
            buffer: Vec::new(),
            position: 0,
            unread: length.checked_sub(8)?,
            hash: twox_hash::XxHash3_64::new(),
        })
    }

    /// Bytes left before the closing hash.
    fn left(&self) -> u64 {
        self.unread + (self.buffer.len() - self.position) as u64
    }

    fn refill(&mut self) -> Option<()> {
        use std::hash::Hasher;
        let take = self.unread.min(BUFFER as u64) as usize;
        if take == 0 {
            return None;
        }
        self.buffer.drain(..self.position);
        self.position = 0;
        let start = self.buffer.len();
        self.buffer.resize(start + take, 0);
        self.source.read_exact(&mut self.buffer[start..]).ok()?;
        self.hash.write(&self.buffer[start..]);
        self.unread -= take as u64;
        Some(())
    }

    fn fill(&mut self, out: &mut [u8]) -> Option<()> {
        let mut done = 0;
        while done < out.len() {
            if self.position == self.buffer.len() {
                self.refill()?;
            }
            let take = (out.len() - done).min(self.buffer.len() - self.position);
            out[done..done + take].copy_from_slice(&self.buffer[self.position..self.position + take]);
            self.position += take;
            done += take;
        }
        Some(())
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        if self.buffer.len() - self.position >= N {
            let bytes = self.buffer[self.position..self.position + N].try_into().ok()?;
            self.position += N;
            return Some(bytes);
        }
        let mut bytes = [0; N];
        self.fill(&mut bytes)?;
        Some(bytes)
    }

    /// Whether everything was read and the closing hash matches.
    #[cfg(not(target_arch = "wasm32"))]
    fn finish(mut self) -> bool {
        use std::hash::Hasher;
        let mut stored = [0; 8];
        self.left() == 0 && self.source.read_exact(&mut stored).is_ok() && u64::from_le_bytes(stored) == self.hash.finish()
    }

    /// A counted run of `size`-byte items, each decoded by `decode`, taken a
    /// buffer at a time rather than a value at a time.
    fn items<T>(&mut self, size: usize, decode: impl Fn(&[u8]) -> T) -> Option<Vec<T>> {
        let count = self.count(size as u64)?;
        let mut items = Vec::with_capacity(count);
        while items.len() < count {
            let whole = (self.buffer.len() - self.position) / size;
            if whole == 0 {
                self.refill()?;
                continue;
            }
            let take = whole.min(count - items.len());
            let bytes = &self.buffer[self.position..self.position + take * size];
            items.extend(bytes.chunks_exact(size).map(&decode));
            self.position += take * size;
        }
        Some(items)
    }

    pub(super) fn u8(&mut self) -> Option<u8> {
        Some(self.array::<1>()?[0])
    }

    pub(super) fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub(super) fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }

    pub(super) fn u64(&mut self) -> Option<u64> {
        self.array().map(u64::from_le_bytes)
    }

    pub(super) fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }

    pub(super) fn f64(&mut self) -> Option<f64> {
        self.u64().map(f64::from_bits)
    }

    /// A count of items at least `size` bytes each, checked against what is
    /// left.
    pub(super) fn count(&mut self, size: u64) -> Option<usize> {
        let count = self.u64()?;
        (count.checked_mul(size.max(1))? <= self.left()).then_some(count as usize)
    }

    pub(super) fn string(&mut self) -> Option<String> {
        let length = self.count(1)?;
        let mut bytes = vec![0; length];
        self.fill(&mut bytes)?;
        String::from_utf8(bytes).ok()
    }
}

/// `count` items read by `read`, held at exactly that length.
fn read_counted<T>(input: &mut Reader, count: usize, mut read: impl FnMut(&mut Reader) -> Option<T>) -> Option<Vec<T>> {
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push(read(input)?);
    }
    Some(items)
}

fn write_points(out: &mut Writer, points: &[DVec2]) {
    out.usize(points.len());
    for point in points {
        out.f64(point.x);
        out.f64(point.y);
    }
}

fn read_points(input: &mut Reader) -> Option<Vec<DVec2>> {
    input.items(16, |item| {
        DVec2::new(
            f64::from_le_bytes(item[..8].try_into().expect("eight bytes")),
            f64::from_le_bytes(item[8..].try_into().expect("eight bytes")),
        )
    })
}

fn write_face(out: &mut Writer, face: &[Vec<DVec2>]) {
    out.usize(face.len());
    for ring in face {
        write_points(out, ring);
    }
}

fn read_face(input: &mut Reader) -> Option<Vec<Vec<DVec2>>> {
    let count = input.count(8)?;
    read_counted(input, count, read_points)
}

fn write_footprints(out: &mut Writer, footprints: &std::collections::HashMap<u64, Vec<Vec<DVec2>>>) {
    let mut keys: Vec<&u64> = footprints.keys().collect();
    keys.sort_unstable();
    out.usize(keys.len());
    for key in keys {
        out.u64(*key);
        write_face(out, &footprints[key]);
    }
}

fn read_footprints(input: &mut Reader) -> Option<std::collections::HashMap<u64, Vec<Vec<DVec2>>>> {
    let count = input.count(16)?;
    (0..count).map(|_| Some((input.u64()?, read_face(input)?))).collect()
}

fn write_selection(out: &mut Writer, selection: BenchSelection) {
    out.f64(selection.base);
    out.f64(selection.top);
    out.bool(selection.is_flitch);
}

fn read_selection(input: &mut Reader) -> Option<BenchSelection> {
    Some(BenchSelection {
        base: input.f64()?,
        top: input.f64()?,
        is_flitch: input.bool()?,
    })
}

fn write_band(out: &mut Writer, band: &CutBand) {
    write_selection(out, band.selection);
    out.usize(band.interval);
    out.usize(band.position);
}

fn read_band(input: &mut Reader) -> Option<CutBand> {
    Some(CutBand {
        selection: read_selection(input)?,
        interval: usize::try_from(input.u64()?).ok()?,
        position: usize::try_from(input.u64()?).ok()?,
    })
}

fn write_shape(out: &mut Writer, shape: BlastShapeRef) {
    out.u64(shape.solid.0);
    out.u64(shape.bench);
    out.u64(shape.anchor[0]);
    out.u64(shape.anchor[1]);
}

fn read_shape(input: &mut Reader) -> Option<BlastShapeRef> {
    Some(BlastShapeRef {
        solid: SolidId(input.u64()?),
        bench: input.u64()?,
        anchor: [input.u64()?, input.u64()?],
    })
}

fn write_option<T>(out: &mut Writer, value: Option<T>, write: impl FnOnce(&mut Writer, T)) {
    out.bool(value.is_some());
    if let Some(value) = value {
        write(out, value);
    }
}

fn read_option<T>(input: &mut Reader, read: impl FnOnce(&mut Reader) -> Option<T>) -> Option<Option<T>> {
    if input.bool()? { read(input).map(Some) } else { Some(None) }
}

/// Parts whose meshes a later stage's parts may share: a flitch or bench left
/// whole is its body part's mesh, not a copy of it, and is kept as such.
type Shared<'a> = [&'a [SolidPart]];

/// A built mesh and how it is drawn. Its name is the solid's, given back on
/// reading as it is on building; anything set on a mesh only while it is
/// shown is never kept, and a mesh carrying any is not written.
fn write_mesh(out: &mut Writer, mesh: &OpenTriangulation, shared: &Shared) -> Option<()> {
    if mesh.raster_texture.is_some() || mesh.flitch_style.is_some() || mesh.depth_shade.is_some() || mesh.pattern_from.is_some() || mesh.raster_opacity != 1.0 {
        return None;
    }
    out.u64(mesh.id.0);
    for channel in mesh.color.into_iter().chain(mesh.line_color) {
        out.f32(channel);
    }
    write_option(out, mesh.line_weight, |out, weight| out.f32(weight));
    out.bool(mesh.cull_back_faces);
    out.bool(mesh.always_show_edges);
    let base = shared.iter().enumerate().find_map(|(list, parts)| {
        parts
            .iter()
            .position(|part| Arc::ptr_eq(&part.mesh.mesh, &mesh.mesh) && Arc::ptr_eq(&part.mesh.surface_face_order, &mesh.surface_face_order))
            .map(|index| (list, index))
    });
    match base {
        Some((list, index)) => {
            out.u8(1);
            out.usize(list);
            out.usize(index);
        }
        None => {
            out.u8(0);
            let vertices = mesh.mesh.vertices();
            out.usize(vertices.len());
            for vertex in vertices {
                out.f64(vertex.x);
                out.f64(vertex.y);
                out.f64(vertex.z);
            }
            out.usize(mesh.mesh.face_count());
            for face in mesh.mesh.face_indices() {
                face.into_iter().for_each(|index| out.u32(index));
            }
            out.usize(mesh.surface_face_order.len());
            for face in mesh.surface_face_order.iter() {
                out.u32(*face);
            }
        }
    }
    let shared_edges = base.is_some_and(|(list, index)| Arc::ptr_eq(&shared[list][index].mesh.edges, &mesh.edges));
    out.bool(shared_edges);
    if !shared_edges {
        out.usize(mesh.edges.len());
        for edge in mesh.edges.iter() {
            edge.iter().for_each(|index| out.u32(*index));
        }
    }
    Some(())
}

/// How a mesh read back is numbered: as written, or afresh where the build
/// numbers each one it makes.
#[derive(Clone, Copy)]
enum Id {
    Kept,
    Fresh,
}

fn read_mesh(input: &mut Reader, name: &str, id: Id, shared: &Shared) -> Option<OpenTriangulation> {
    let stored_id = TriangulationId(input.u64()?);
    let mut colors = [0.0f32; 8];
    for channel in &mut colors {
        *channel = input.f32()?;
    }
    let line_weight = read_option(input, Reader::f32)?;
    let cull_back_faces = input.bool()?;
    let always_show_edges = input.bool()?;
    let f64_at = |bytes: &[u8], at: usize| f64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"));
    let u32_at = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
    let (mesh, spatial, order, base_edges) = match input.u8()? {
        1 => {
            let list = usize::try_from(input.u64()?).ok()?;
            let index = usize::try_from(input.u64()?).ok()?;
            let base = &shared.get(list)?.get(index)?.mesh;
            (base.mesh.clone(), base.spatial.clone(), base.surface_face_order.clone(), Some(base.edges.clone()))
        }
        0 => {
            let vertices = input.items(24, |item| Vertex {
                x: f64_at(item, 0),
                y: f64_at(item, 8),
                z: f64_at(item, 16),
            })?;
            let faces = input.items(12, |item| [u32_at(item, 0), u32_at(item, 4), u32_at(item, 8)])?;
            let order = input.items(4, |item| u32_at(item, 0))?;
            let mesh = Arc::new(Triangulation::from_vertices_and_faces(vertices, faces).ok()?);
            let spatial = Arc::new(crate::model::spatial::TriangleBvh::deferred(mesh.clone()));
            (mesh, spatial, Arc::new(order), None)
        }
        _ => return None,
    };
    let edges = match (input.bool()?, base_edges) {
        (true, Some(edges)) => edges,
        (false, _) => Arc::new(input.items(8, |item| [u32_at(item, 0), u32_at(item, 4)])?),
        (true, None) => return None,
    };
    let mut item = preview_triangulation(name.to_owned(), mesh, spatial, edges, order, colors[..4].try_into().ok()?, colors[4..].try_into().ok()?);
    item.id = match id {
        Id::Kept => stored_id,
        Id::Fresh => super::next_view_id(),
    };
    item.line_weight = line_weight;
    item.cull_back_faces = cull_back_faces;
    item.always_show_edges = always_show_edges;
    Some(item)
}

fn write_part(out: &mut Writer, part: &SolidPart, shared: &Shared) -> Option<()> {
    write_mesh(out, &part.mesh, shared)?;
    write_band(out, &part.band);
    write_selection(out, part.bench);
    write_option(out, part.blast, write_shape);
    // A block's id is handed out on reading, as on building; the project's
    // own record of block identities settles it either way.
    if part.block.as_ref().is_some_and(|block| !block.replaces.is_empty()) {
        return None;
    }
    write_option(out, part.block.as_ref(), |out, block| {
        write_shape(out, block.key);
        out.string(&block.name);
        out.f64(block.area);
        write_face(out, &block.face);
    });
    write_option(out, part.volume, |out, volume| out.f64(volume));
    Some(())
}

fn read_part(input: &mut Reader, name: &str, id: Id, shared: &Shared) -> Option<SolidPart> {
    Some(SolidPart {
        mesh: read_mesh(input, name, id, shared)?,
        band: read_band(input)?,
        bench: read_selection(input)?,
        blast: read_option(input, read_shape)?,
        block: read_option(input, |input| {
            Some(DigPiece {
                key: read_shape(input)?,
                name: input.string()?,
                area: input.f64()?,
                face: Arc::new(read_face(input)?),
                id: DigBlockId(super::NEXT_DIG_BLOCK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
                replaces: Vec::new(),
            })
        })?,
        volume: read_option(input, Reader::f64)?,
    })
}

fn write_parts(out: &mut Writer, parts: &[SolidPart], shared: &Shared) -> Option<()> {
    out.usize(parts.len());
    parts.iter().try_for_each(|part| write_part(out, part, shared))
}

fn read_parts(input: &mut Reader, name: &str, id: Id, shared: &Shared) -> Option<Vec<SolidPart>> {
    let count = input.count(64)?;
    read_counted(input, count, |input| read_part(input, name, id, shared))
}

pub(crate) fn write_solid_mesh(out: &mut Writer, solid: &SolidMesh) -> Option<()> {
    if !solid.edges.is_empty() {
        return None;
    }
    let vertices = solid.mesh.vertices();
    out.usize(vertices.len());
    for vertex in vertices {
        out.f64(vertex.x);
        out.f64(vertex.y);
        out.f64(vertex.z);
    }
    out.usize(solid.mesh.face_count());
    for face in solid.mesh.face_indices() {
        face.into_iter().for_each(|index| out.u32(index));
    }
    out.usize(solid.surface_face_order.len());
    solid.surface_face_order.iter().for_each(|face| out.u32(*face));
    out.f64(solid.volume);
    Some(())
}

/// A solid built between two surfaces, as [`write_solid_mesh`] kept it.
pub(crate) fn read_solid_mesh(input: &mut Reader, built_from: BuiltFrom) -> Option<SolidMesh> {
    let f64_at = |bytes: &[u8], at: usize| f64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"));
    let u32_at = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
    let vertices = input.items(24, |item| Vertex {
        x: f64_at(item, 0),
        y: f64_at(item, 8),
        z: f64_at(item, 16),
    })?;
    let faces = input.items(12, |item| [u32_at(item, 0), u32_at(item, 4), u32_at(item, 8)])?;
    let order = input.items(4, |item| u32_at(item, 0))?;
    let volume = input.f64()?;
    let mesh = Arc::new(Triangulation::from_vertices_and_faces(vertices, faces).ok()?);
    Some(SolidMesh {
        spatial: Arc::new(crate::model::spatial::TriangleBvh::deferred(mesh.clone())),
        edges: Arc::new(Vec::new()),
        surface_face_order: Arc::new(order),
        mesh,
        volume,
        built_from,
    })
}

pub(super) fn write_body(out: &mut Writer, body: &SolidBody) -> Option<()> {
    write_parts(out, &body.bench_parts, &[])?;
    write_parts(out, &body.flitch_parts, &[])?;
    write_footprints(out, &body.bench_footprints);
    write_footprints(out, &body.flitch_footprints);
    out.usize(body.occupied_bands.len());
    body.occupied_bands.iter().for_each(|band| write_band(out, band));
    Some(())
}

pub(super) fn read_body(input: &mut Reader, solid: &Solid) -> Option<SolidBody> {
    Some(SolidBody {
        bench_parts: read_parts(input, &solid.name, Id::Fresh, &[])?,
        // Numbered from the preview's own id, as the build numbers them.
        flitch_parts: read_parts(input, &solid.name, Id::Kept, &[])?,
        bench_footprints: read_footprints(input)?,
        flitch_footprints: read_footprints(input)?,
        occupied_bands: {
            let count = input.count(25)?;
            (0..count).map(|_| read_band(input)).collect::<Option<_>>()?
        },
    })
}

pub(super) fn write_blasting(out: &mut Writer, blasting: &SolidBlasting) -> Option<()> {
    out.usize(blasting.blast_faces.len());
    for blast in &blasting.blast_faces {
        write_selection(out, blast.bench);
        write_face(out, &blast.face);
        out.f64(blast.anchor[0]);
        out.f64(blast.anchor[1]);
        out.f64(blast.area);
    }
    Some(())
}

pub(super) fn read_blasting(input: &mut Reader) -> Option<SolidBlasting> {
    let count = input.count(41)?;
    let blast_faces = (0..count)
        .map(|_| {
            Some(BlastFace {
                bench: read_selection(input)?,
                face: read_face(input)?,
                anchor: [input.f64()?, input.f64()?],
                area: input.f64()?,
            })
        })
        .collect::<Option<_>>()?;
    Some(SolidBlasting { blast_faces })
}

/// Each level's outlines and the parts they were cut into; an outline its
/// part's dig block holds is kept once, as that block's.
fn write_carved(out: &mut Writer, levels: &std::collections::HashMap<u64, super::CarvedLevel>, parts: &[SolidPart]) {
    let mut keys: Vec<&u64> = levels.keys().collect();
    keys.sort_unstable();
    out.usize(keys.len());
    for key in keys {
        out.u64(*key);
        out.usize(levels[key].len());
        for (face, part) in &levels[key] {
            out.usize(*part);
            let held = parts.get(*part).and_then(|part| part.block.as_ref()).is_some_and(|block| Arc::ptr_eq(&block.face, face));
            out.bool(held);
            if !held {
                write_face(out, face);
            }
        }
    }
}

fn read_carved(input: &mut Reader, parts: &[SolidPart]) -> Option<std::collections::HashMap<u64, super::CarvedLevel>> {
    let count = input.count(16)?;
    let mut levels = std::collections::HashMap::with_capacity(count);
    for _ in 0..count {
        let key = input.u64()?;
        let pieces = input.count(9)?;
        let level = read_counted(input, pieces, |input| {
            let part = usize::try_from(input.u64()?).ok().filter(|part| *part < parts.len())?;
            let face = match input.bool()? {
                true => parts[part].block.as_ref()?.face.clone(),
                false => Arc::new(read_face(input)?),
            };
            Some((face, part))
        })?;
        levels.insert(key, level);
    }
    Some(levels)
}

pub(super) fn write_partition(out: &mut Writer, partition: &SolidPartition) -> Option<()> {
    let body = &partition.carving.body;
    let shared: &Shared = &[&body.bench_parts, &body.flitch_parts];
    write_parts(out, &partition.parts, shared)?;
    write_parts(out, &partition.blast_parts, shared)?;
    write_carved(out, &partition.carving.flitches, &partition.parts);
    write_carved(out, &partition.carving.benches, &partition.blast_parts);
    Some(())
}

/// The partition as cut from `body`, which it lends its pieces to the next.
pub(super) fn read_partition(input: &mut Reader, solid: &Solid, body: &Arc<SolidBody>) -> Option<SolidPartition> {
    let shared: &Shared = &[&body.bench_parts, &body.flitch_parts];
    let parts = read_parts(input, &solid.name, Id::Fresh, shared)?;
    let blast_parts = read_parts(input, &solid.name, Id::Fresh, shared)?;
    let flitches = read_carved(input, &parts)?;
    let benches = read_carved(input, &blast_parts)?;
    Some(SolidPartition {
        parts,
        blast_parts,
        carving: Carving {
            body: body.clone(),
            flitches,
            benches,
        },
    })
}
