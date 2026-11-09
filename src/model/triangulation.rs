use std::{path::PathBuf, sync::Arc};

use crate::model::{formats::mesh_data, project::ProjectItemState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct TriangulationId(pub(crate) u64);

/// A persistent version of one source surface's *geometry*.
///
/// The rules live here so no caller has to remember them:
///
/// - A token is minted once, when an item's geometry first becomes known -
///   at import, at generation, or at load for an older file that never
///   carried one - and is [`Copy`]: everything downstream of it, including
///   schedule references, holds it by value.
/// - Nothing but a change of geometry may mint a new one. Colour, name,
///   visibility, loading state and rendering caches leave it alone, so a
///   token that survives says the ground it names was not redrawn.
/// - It is persisted with the item (OMF element metadata), and eviction
///   round-trips through those same bytes, so a restore hands back the token
///   that left.
/// - Undo of a geometry edit must restore the earlier token with the earlier
///   geometry; an edit after that undo mints a fresh one, so a redo cannot
///   resurrect the retired version. When mesh editing arrives, its commit
///   path mints here and marks the item dirty - nothing else.
///
/// Tokens are UUID v4, minted through the same browser-capable mechanism as
/// [`crate::model::project::ProjectId`]: random, not derived from the clock,
/// pointers, runtime ids or the general item epoch, so two sessions minting
/// concurrently cannot hand each other's token out by construction of the
/// random source rather than by timing assumptions. Random is not "cannot
/// collide" - a v4 collision needs the same 122 random bits twice - but it
/// is the same guarantee every id in the project already stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct GeometryVersion(pub(crate) uuid::Uuid);

impl GeometryVersion {
    /// Mint a token for geometry newly known to this project. Works on every
    /// target the application builds for, browser included.
    pub(crate) fn mint() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// The provenance of a solid's geometry: which source surfaces its body was
/// built from, and which version of each.
///
/// Captured when the Solids artifact job starts, from the inputs that job
/// actually reads - never re-derived from whatever document is current when
/// the job lands - and carried on the committed product into every dig
/// block, so a block can say which ground produced it. A schedule reference
/// stores the stamp of the ground the user selected and asks the current run
/// the conservative question: are these still the same sources at the same
/// versions? When they are not, the answer is "reselect or reconfirm", not a
/// guess - even if the block's outline, band and volume all happen to match,
/// because the same numbers can describe different material in the same
/// envelope.
///
/// Deliberately excludes block models, reserve mappings and the tonnage
/// field: those change what a block *measures*, not which ground it *is*,
/// and must be able to move without invalidating a schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct GroundSourceStamp {
    pub(crate) surface: Option<(TriangulationId, GeometryVersion)>,
    pub(crate) topography: Option<(TriangulationId, GeometryVersion)>,
}

/// The computed outputs of loading a triangulation file, produced on a background
/// thread and sent back to the main thread via channel.
pub(crate) struct LoadedTriangulation {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) mesh: Arc<mesh_data::Triangulation>,
    pub(crate) spatial: Arc<crate::model::spatial::TriangleBvh>,
    pub(crate) edges: Vec<[u32; 2]>,
    pub(crate) surface_face_order: Arc<Vec<u32>>,
}

/// A freshly generated triangulation with its mesh, BVH and edge list already
/// built - the owned, `Send` output of the worker half of a background compute
/// job (include/cut/create). The UI thread turns this into an
/// `OpenTriangulation` via `insert_generated_triangulation`.
pub(crate) struct GeneratedTriangulation {
    pub(crate) name: String,
    pub(crate) mesh: Arc<mesh_data::Triangulation>,
    pub(crate) spatial: Arc<crate::model::spatial::TriangleBvh>,
    pub(crate) edges: Vec<[u32; 2]>,
    pub(crate) surface_face_order: Arc<Vec<u32>>,
    pub(crate) surface_type: crate::ui::state::TriSurfaceType,
}

/// A `GeneratedTriangulation` paired with the single completion log line to
/// emit when it is applied. The common shape for backgrounded cut/create jobs.
pub(crate) struct GeneratedTriangulationLog {
    pub(crate) generated: GeneratedTriangulation,
    pub(crate) message: String,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenTriangulation {
    pub(crate) id: TriangulationId,
    pub(crate) state: ProjectItemState,
    pub(crate) name: String,
    /// Which version of this surface's geometry the mesh holds. Preserved
    /// through unload, eviction, restore and rendering copies; see
    /// [`GeometryVersion`].
    pub(crate) geometry: GeometryVersion,
    pub(crate) mesh: Arc<mesh_data::Triangulation>,
    pub(crate) spatial: Arc<crate::model::spatial::TriangleBvh>,
    /// Shared like the mesh: the Solids pages restyle every dig block on each
    /// pick, and a copy of each block's outline per restyle added up.
    ///
    /// Empty when the outline is every triangle side, as it is for a surface
    /// not marked `always_show_edges`: those are drawn by the surface's own
    /// shader wireframe, and the renderer derives the list only on a device
    /// that has none. A survey's would otherwise hold a pair of indices for
    /// every side of every face, for nothing.
    pub(crate) edges: Arc<Vec<[u32; 2]>>,
    /// Face indices in GPU-chunk order (see `spatial_surface_face_order`),
    /// precomputed off-thread so GPU surface chunking never sorts on the render
    /// thread. `Arc` so cloning an `OpenTriangulation` stays cheap.
    pub(crate) surface_face_order: Arc<Vec<u32>>,
    pub(crate) color: [f32; 4],
    pub(crate) line_color: [f32; 4],
    pub(crate) line_weight: Option<f32>,
    /// Optional georeferenced raster draped over this surface in world XY.
    pub(crate) raster_texture: Option<crate::model::raster::RasterTextureId>,
    pub(crate) raster_opacity: f32,
    /// Runtime-only style for generated planning slabs.
    pub(crate) flitch_style: Option<crate::model::FlitchStyle>,
    pub(crate) cull_back_faces: bool,
    /// Draw this mesh's edges whether or not it is selected. Set for the dig
    /// blocks a flitch is cut into, whose seams are the only thing telling one
    /// block from the next. Their `edges` are that outline, not every triangle
    /// side, so the renderer draws the list rather than a shader wireframe.
    pub(crate) always_show_edges: bool,
    /// Scene-Z range to shade this surface across, as a greyscale ramp over
    /// its own colour. Runtime only, for the Blasting step's plan view, where
    /// depth is the only cue a top-down orthographic camera leaves.
    pub(crate) depth_shade: Option<[f64; 2]>,
    /// Draw `flitch_style`'s pattern only where a plan point's dot product
    /// with `[x, y]` reaches the third value, in world coordinates. Runtime
    /// only, for Animate's blasts, whose hatching prep clears away.
    pub(crate) pattern_from: Option<[f64; 3]>,
}

impl OpenTriangulation {
    pub(crate) fn entity_id(&self) -> crate::model::SceneEntityId {
        crate::model::SceneEntityId::Triangulation(self.id)
    }
}

/// Faces per GPU surface chunk. Chunks are the granularity of frustum culling
/// (and per-chunk debug colouring), so this trades culling precision (smaller
/// = tighter) against per-chunk draw-call/AABB-test overhead (smaller = more).
/// ~100k keeps a multi-million-face mesh in tens of chunks.
pub(crate) const SURFACE_CHUNK_FACES: usize = 100_000;

/// Face indices ordered so every consecutive run of [`SURFACE_CHUNK_FACES`]
/// covers one compact box of the mesh: a kd split of the face centroids, each
/// cut along the longest axis of its range.
///
/// Cuts land only on multiples of `SURFACE_CHUNK_FACES` from the start of the
/// order, so each leaf is exactly one chunk (the last one short) and the GPU
/// upload keeps slicing the order at fixed size. A plain space-filling-curve
/// sort cut every 100k faces leaves chunks that straddle the curve's jumps -
/// L-shapes and ragged strips whose AABBs overlap their neighbours'. Here the
/// leaves tile the mesh without overlap, and a cliff tall enough to be the
/// longest axis is split by height too.
///
/// `select_nth_unstable` per level instead of a full sort, so the work is
/// `O(n log(n / SURFACE_CHUNK_FACES))`, with independent halves on rayon.
/// Computed at mesh build/load time - off the render thread - and stored on
/// `OpenTriangulation`, so the first GPU upload of a huge mesh doesn't hitch.
pub(crate) fn spatial_surface_face_order(mesh: &mesh_data::Triangulation) -> Vec<u32> {
    use rayon::prelude::*;

    let face_count = mesh.face_count();
    // One chunk has nothing to partition, so the faces stay in their order.
    if face_count <= SURFACE_CHUNK_FACES {
        return (0..face_count as u32).collect();
    }
    let vertices = mesh.vertices();
    // Relative to the mesh minimum so `f32` holds plenty of precision for
    // ordering, which halves the record the partitioning moves around.
    let origin = mesh.bounds().min;
    let mut faces: Vec<FaceCentroid> = (0..face_count)
        .into_par_iter()
        .map(|face_index| {
            let face = mesh.face_vertex_indices(face_index).unwrap_or([0, 0, 0]);
            let mut centroid = [0.0; 3];
            for vertex_index in face {
                let point = vertices[vertex_index];
                centroid[0] += point.x - origin.x;
                centroid[1] += point.y - origin.y;
                centroid[2] += point.z - origin.z;
            }
            FaceCentroid {
                centroid: centroid.map(|sum| (sum / 3.0) as f32),
                face: face_index as u32,
            }
        })
        .collect();
    split_chunk_range(&mut faces);
    // Collecting reuses the allocation, which is four times too large for this.
    let mut order: Vec<u32> = faces.into_iter().map(|face| face.face).collect();
    order.shrink_to_fit();
    order
}

struct FaceCentroid {
    centroid: [f32; 3],
    face: u32,
}

/// Partition `faces` in place into chunk-sized leaves. The slice always starts
/// on a chunk boundary, so cutting after a whole number of chunks keeps both
/// halves aligned.
fn split_chunk_range(faces: &mut [FaceCentroid]) {
    let chunks = faces.len().div_ceil(SURFACE_CHUNK_FACES);
    if chunks <= 1 {
        return;
    }
    let bounds = |(mut min, mut max): ([f32; 3], [f32; 3]), face: &FaceCentroid| {
        for axis in 0..3 {
            min[axis] = min[axis].min(face.centroid[axis]);
            max[axis] = max[axis].max(face.centroid[axis]);
        }
        (min, max)
    };
    let empty = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let (min, max) = if faces.len() > 4 * SURFACE_CHUNK_FACES {
        use rayon::prelude::*;
        faces.par_iter().fold(|| empty, bounds).reduce(
            || empty,
            |(a_min, a_max), (b_min, b_max)| {
                (
                    std::array::from_fn(|axis| a_min[axis].min(b_min[axis])),
                    std::array::from_fn(|axis| a_max[axis].max(b_max[axis])),
                )
            },
        )
    } else {
        faces.iter().fold(empty, bounds)
    };
    let axis = (0..3).max_by(|&a, &b| (max[a] - min[a]).total_cmp(&(max[b] - min[b]))).unwrap_or(0);
    let cut = (chunks / 2) * SURFACE_CHUNK_FACES;
    faces.select_nth_unstable_by(cut, |a, b| a.centroid[axis].total_cmp(&b.centroid[axis]));
    let (left, right) = faces.split_at_mut(cut);
    rayon::join(|| split_chunk_range(left), || split_chunk_range(right));
}

/// For each of `count` items, the first with a key equal to its own: itself,
/// or an earlier one.
///
/// Items are dealt by their key's hash into shards, in order, and each shard
/// is matched up on its own, so the whole runs in parallel and needs no sort
/// of all the keys - one of tens of millions, through an index, whose first
/// partitions run on one thread.
pub(crate) fn first_of_equal<K: std::hash::Hash + Eq + Send>(count: usize, key: impl Fn(usize) -> K + Sync) -> Vec<u32> {
    use std::{
        hash::BuildHasher,
        sync::atomic::{AtomicU32, Ordering},
    };

    use rayon::prelude::*;
    const SHARDS: usize = 256;
    const RUN: usize = 1 << 16;
    let hasher = foldhash::fast::FixedState::default();
    let shard = |index: usize| (hasher.hash_one(key(index)) >> 56) as usize % SHARDS;
    // Each run's items by shard, in order; a shard's items are its runs'
    // lists one after another.
    let dealt: Vec<Vec<Vec<u32>>> = (0..count.div_ceil(RUN))
        .into_par_iter()
        .map(|run| {
            let mut shards: Vec<Vec<u32>> = vec![Vec::new(); SHARDS];
            for index in run * RUN..((run + 1) * RUN).min(count) {
                shards[shard(index)].push(index as u32);
            }
            shards
        })
        .collect();
    let first: Vec<AtomicU32> = (0..count).map(|_| AtomicU32::new(0)).collect();
    (0..SHARDS).into_par_iter().for_each(|shard| {
        let mut seen: std::collections::HashMap<K, u32, foldhash::fast::FixedState> = std::collections::HashMap::default();
        for index in dealt.iter().flat_map(|runs| runs[shard].iter().copied()) {
            let earliest = *seen.entry(key(index as usize)).or_insert(index);
            first[index as usize].store(earliest, Ordering::Relaxed);
        }
    });
    drop(dealt);
    first.into_iter().map(AtomicU32::into_inner).collect()
}

/// One vertex per bit-identical position, the first of each kept in order,
/// and the faces renumbered onto them. A cut leaves every face with corners of
/// its own; shared again, the mesh holds about a fifth of the vertices with
/// nothing about its geometry changed.
pub(crate) fn weld_identical_points(vertices: Vec<mesh_data::Vertex>, mut faces: Vec<[u32; 3]>) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    use rayon::prelude::*;

    // Each point's first occurrence.
    let first = first_of_equal(vertices.len(), |index| {
        let vertex = vertices[index];
        [vertex.x.to_bits(), vertex.y.to_bits(), vertex.z.to_bits()]
    });
    let mut renumbered: Vec<u32> = vec![u32::MAX; vertices.len()];
    let mut kept = Vec::with_capacity(vertices.len() / 4);
    for (index, vertex) in vertices.iter().enumerate() {
        if first[index] as usize == index {
            renumbered[index] = kept.len() as u32;
            kept.push(*vertex);
        }
    }
    drop(vertices);
    kept.shrink_to_fit();
    faces.par_iter_mut().for_each(|face| *face = face.map(|index| renumbered[first[index as usize] as usize]));
    (kept, faces)
}

pub(crate) fn unique_edges(mesh: &mesh_data::Triangulation) -> Vec<[u32; 2]> {
    use rayon::prelude::*;

    let mut packed: Vec<u64> = mesh
        .face_vertex_indices_iter()
        .flat_map(|face| {
            [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])].map(|(a, b)| {
                let (a, b) = if a <= b { (a as u32, b as u32) } else { (b as u32, a as u32) };
                (u64::from(a) << 32) | u64::from(b)
            })
        })
        .collect();

    packed.par_sort_unstable();
    packed.dedup();

    // Collected in place, so it would keep the room for every face's three
    // sides, about twice what is left once the shared ones are gone.
    let mut edges: Vec<[u32; 2]> = packed.into_iter().map(|e| [(e >> 32) as u32, e as u32]).collect();
    edges.shrink_to_fit();
    edges
}
