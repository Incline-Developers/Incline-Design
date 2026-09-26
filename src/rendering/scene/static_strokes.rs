//! Persistent GPU cache for stroke geometry of stable polylines.
//!
//! The per-rebuild scene builder re-tessellates every object on any document
//! change, which is fine for hand-drawn documents but ruinous once an
//! operation (contouring, string imports) drops tens of thousands of dense
//! polylines into the document. This cache claims those polylines, groups them
//! into per-layer chunks with their own GPU buffers, and re-tessellates only
//! the chunks whose members actually changed. Everything else - points, text,
//! filled polylines, and translucent polylines, which need the blended
//! document stage - stays on the per-rebuild path. Selection and hover are
//! applied by the stroke shader through each member's style slot, so they
//! never evict a member from its chunk.

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use glam::DVec3;

use crate::{
    model::{Document, FillStyle, LayerId, Object, ObjectId, SceneEntityId},
    rendering::{
        StrokeInstance, Vertex,
        geometry::{DrawContext, tessellate_polyline_stroke},
        pick::{PickRecord, StrokeBlocks, world_bounds_from_local_positions},
        scene::document_style::DocumentStyleSlots,
    },
    ui::state::EditorState,
};

/// Soft per-chunk stroke-instance budget (24 MiB of instance data). Small
/// enough that re-tessellating one chunk (an edited member) stays within a
/// frame; large enough that contour-scale documents need only dozens of draw
/// calls.
const CHUNK_INSTANCE_BUDGET: usize = 512 * 1024;

pub(crate) struct StaticStrokeChunk {
    layer: LayerId,
    /// Mirrors the layer's visibility each sync so draw and pick can skip the
    /// chunk without rebuilding anything when a layer is toggled.
    pub(crate) layer_visible: bool,
    members: Vec<ObjectId>,
    /// Estimated stroke instances including members assigned since the last
    /// rebuild; used only to decide when to start a new chunk.
    estimated_instances: usize,
    dirty: bool,
    /// CPU copy retained for picking (cursor pick and box select read
    /// positions back).
    pub(crate) strokes: Vec<StrokeInstance>,
    pub(crate) stroke_blocks: StrokeBlocks,
    /// Per-member pick records with ranges into this chunk's buffers (fill
    /// ranges are always empty - filled polylines are ineligible).
    pub(crate) records: Vec<PickRecord>,
    /// Union of member pick bounds, used to reject this entire CPU stream on
    /// cursor queries that land elsewhere.
    pub(crate) world_bounds: Option<(DVec3, DVec3)>,
    pub(crate) instance_gpu: Option<wgpu::Buffer>,
    instance_capacity: usize,
    pub(crate) instance_count: u32,
}

impl StaticStrokeChunk {
    fn new(layer: LayerId) -> Self {
        Self {
            layer,
            layer_visible: true,
            members: Vec::new(),
            estimated_instances: 0,
            dirty: false,
            strokes: Vec::new(),
            stroke_blocks: StrokeBlocks::default(),
            records: Vec::new(),
            world_bounds: None,
            instance_gpu: None,
            instance_capacity: 0,
            instance_count: 0,
        }
    }

    pub(crate) fn drawable(&self) -> bool {
        self.layer_visible && self.instance_count > 0
    }
}

#[derive(Default)]
pub(crate) struct StaticStrokeCache {
    chunks: Vec<StaticStrokeChunk>,
    /// Chunk index and last-built fingerprint per claimed object.
    object_chunk: HashMap<ObjectId, (usize, u64)>,
    claimed: HashSet<ObjectId>,
    /// Order-independent fingerprint of `claimed`, so the stream builder can
    /// tell when the set it must skip has changed.
    claimed_key: u64,
    cached_scene_origin: DVec3,
    cached_scale_factor: f32,
}

/// Everything the cache bakes into vertices besides the object's own data.
/// A mismatch forces the member's chunk to rebuild.
fn fingerprint(object_revision: u64, rgba: [f32; 4], layer: LayerId) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    object_revision.hash(&mut hasher);
    rgba.map(f32::to_bits).hash(&mut hasher);
    layer.hash(&mut hasher);
    hasher.finish()
}

/// Whether the cache may own this object's stroke geometry. Hidden objects
/// draw nothing, and translucent ones need the blended document stage.
fn eligible(object: &Object, editor: &EditorState, rgba: [f32; 4]) -> bool {
    let Object::Polyline { closed, fill, .. } = object else {
        return false;
    };
    if *closed && *fill != FillStyle::Clear {
        return false;
    }
    // Transparent strokes stay in the document stream so they can be routed
    // through the non-depth-writing transparency pass.
    if rgba[3] < 0.999 {
        return false;
    }
    let handle = SceneEntityId::Object(object.id());
    !(editor.hidden_handles.contains(&handle) || editor.translucent_handles.contains(&handle))
}

#[derive(Clone, Copy)]
struct StaticStrokeLimits {
    instances: usize,
    buffer_bytes: usize,
}

impl StaticStrokeLimits {
    fn from_device(device: &wgpu::Device) -> Self {
        let buffer_bytes = usize::try_from(device.limits().max_buffer_size).unwrap_or(usize::MAX);
        Self {
            instances: (buffer_bytes / size_of::<StrokeInstance>()).min(CHUNK_INSTANCE_BUDGET),
            buffer_bytes,
        }
    }
}

/// Conservative bounds for the straight-polyline tessellator. Bulged lines
/// stay on the checked main stream because one logical arc may expand to
/// thousands of primitives and does not make a predictable static-cache
/// member.
fn estimate_stroke(object: &Object) -> Option<usize> {
    let Object::Polyline { verts, closed, .. } = object else {
        return None;
    };
    if verts.iter().any(|vertex| vertex.bulge.abs() > f64::EPSILON) {
        return None;
    }
    let segments = verts.len().saturating_sub(1).saturating_add(usize::from(*closed && verts.len() >= 2));
    let joins = if *closed && verts.len() >= 2 { verts.len() } else { verts.len().saturating_sub(2) };
    // A segment and a round join are one instance each. Degenerate and
    // nearly collinear geometry only reduces the count.
    Some(segments.saturating_add(joins))
}

fn can_append(current: usize, next: usize, limit: usize) -> bool {
    current.checked_add(next).is_some_and(|combined| combined <= limit)
}

impl StaticStrokeCache {
    /// Object ids whose stroke geometry this cache owns; the scene builder
    /// must skip them.
    pub(crate) fn claimed(&self) -> &HashSet<ObjectId> {
        &self.claimed
    }

    pub(crate) fn claimed_key(&self) -> u64 {
        self.claimed_key
    }

    pub(crate) fn chunks(&self) -> &[StaticStrokeChunk] {
        &self.chunks
    }

    /// Reconcile the cache with the current document and editor state,
    /// re-tessellating and re-uploading only chunks with changed members.
    /// Runs on every geometry rebuild, so per-object work here must stay
    /// cheap (a fingerprint compare) for unchanged objects.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        document: &Document,
        editor: &EditorState,
        slots: &mut DocumentStyleSlots,
        scene_origin: DVec3,
        scale_factor: f32,
    ) {
        let limits = StaticStrokeLimits::from_device(device);
        // Origin and scale factor are baked into every instance.
        if scene_origin != self.cached_scene_origin || (scale_factor - self.cached_scale_factor).abs() > f32::EPSILON {
            self.chunks.clear();
            self.object_chunk.clear();
            self.cached_scene_origin = scene_origin;
            self.cached_scale_factor = scale_factor;
        }

        self.claimed.clear();
        self.claimed_key = 0;
        for object in document.objects() {
            let rgba = document.object_rgba(object);
            if !document.layer(object.layer()).is_some_and(|layer| layer.loaded) || !eligible(object, editor, rgba) {
                continue;
            }
            // Oversized and bulged members remain on the main stream, whose
            // checked truncation handles them without creating an unbounded
            // cache allocation.
            let Some(estimate) = estimate_stroke(object).filter(|estimate| *estimate <= limits.instances) else {
                continue;
            };
            let id = object.id();
            let fp = fingerprint(document.object_revision(id), rgba, object.layer());
            match self.object_chunk.get(&id).copied() {
                Some((chunk_index, cached_fp)) => {
                    let moved_layer = self.chunks[chunk_index].layer != object.layer();
                    if moved_layer {
                        self.remove_member(id, chunk_index);
                        self.assign(object.layer(), id, estimate, limits);
                    } else if cached_fp != fp {
                        self.chunks[chunk_index].dirty = true;
                    }
                }
                None => self.assign(object.layer(), id, estimate, limits),
            }
            self.claimed.insert(id);
            self.claimed_key ^= {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                id.hash(&mut hasher);
                hasher.finish()
            };
        }

        // Members that vanished or became ineligible release their chunk.
        let stale: Vec<(ObjectId, usize)> = self
            .object_chunk
            .iter()
            .filter(|(id, _)| !self.claimed.contains(id))
            .map(|(&id, &(chunk_index, _))| (id, chunk_index))
            .collect();
        for (id, chunk_index) in stale {
            self.remove_member(id, chunk_index);
            self.object_chunk.remove(&id);
        }

        if self.chunks.iter().any(|chunk| chunk.members.is_empty()) {
            self.chunks.retain(|chunk| !chunk.members.is_empty());
            self.reindex_chunks();
        }
        for chunk in &mut self.chunks {
            chunk.layer_visible = document.layer(chunk.layer).map(|layer| layer.loaded).unwrap_or(true);
        }

        for chunk_index in 0..self.chunks.len() {
            if self.chunks[chunk_index].dirty {
                self.rebuild_chunk(chunk_index, device, queue, document, slots, scene_origin, scale_factor, limits);
            }
        }
    }

    pub(crate) fn release_layers(&mut self, layers: &[LayerId]) {
        self.chunks.retain(|chunk| !layers.contains(&chunk.layer));
        self.reindex_chunks();
    }

    fn reindex_chunks(&mut self) {
        let retained: HashSet<_> = self.chunks.iter().flat_map(|chunk| chunk.members.iter().copied()).collect();
        self.object_chunk.retain(|id, _| retained.contains(id));
        for (index, chunk) in self.chunks.iter().enumerate() {
            for id in &chunk.members {
                if let Some((cached_index, _)) = self.object_chunk.get_mut(id) {
                    *cached_index = index;
                }
            }
        }
        self.claimed.retain(|id| self.object_chunk.contains_key(id));
        self.chunks.shrink_to_fit();
        self.object_chunk.shrink_to_fit();
        self.claimed.shrink_to_fit();
    }

    fn assign(&mut self, layer: LayerId, id: ObjectId, estimate: usize, limits: StaticStrokeLimits) {
        let chunk_index = self
            .chunks
            .iter()
            .position(|chunk| chunk.layer == layer && can_append(chunk.estimated_instances, estimate, limits.instances))
            .unwrap_or_else(|| {
                self.chunks.push(StaticStrokeChunk::new(layer));
                self.chunks.len() - 1
            });
        let chunk = &mut self.chunks[chunk_index];
        chunk.members.push(id);
        chunk.estimated_instances += estimate;
        chunk.dirty = true;
        // The real fingerprint is stored when the chunk rebuilds.
        self.object_chunk.insert(id, (chunk_index, 0));
    }

    fn remove_member(&mut self, id: ObjectId, chunk_index: usize) {
        let chunk = &mut self.chunks[chunk_index];
        chunk.members.retain(|member| *member != id);
        chunk.dirty = true;
    }

    #[allow(clippy::too_many_arguments)]
    fn rebuild_chunk(
        &mut self,
        chunk_index: usize,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        document: &Document,
        slots: &mut DocumentStyleSlots,
        scene_origin: DVec3,
        scale_factor: f32,
        limits: StaticStrokeLimits,
    ) {
        let chunk = &mut self.chunks[chunk_index];
        chunk.strokes.clear();
        chunk.records.clear();

        // Eligible polylines never emit fill geometry; these stay empty.
        let mut unused_fill_vertices: Vec<Vertex> = Vec::new();
        let mut unused_fill_indices: Vec<u32> = Vec::new();

        for member_index in 0..chunk.members.len() {
            let id = chunk.members[member_index];
            let Some(object) = document.get_object(id) else {
                continue;
            };
            let Object::Polyline { verts, closed, line_weight, .. } = object else {
                continue;
            };
            let rgba = document.object_rgba(object);
            let stroke_start = chunk.strokes.len() as u32;
            {
                let mut draw_ctx = DrawContext::unstyled(&mut chunk.strokes, &mut unused_fill_vertices, &mut unused_fill_indices, scene_origin, scale_factor);
                draw_ctx.style = slots.slot(id);
                tessellate_polyline_stroke(&mut draw_ctx, verts, *closed, *line_weight, rgba);
            }
            let stroke_end = chunk.strokes.len() as u32;
            if stroke_end > stroke_start
                && let Some(world_bounds) = world_bounds_from_local_positions(
                    chunk.strokes[stroke_start as usize..stroke_end as usize].iter().flat_map(|stroke| {
                        let (start, end) = stroke.world_ends();
                        [start, end]
                    }),
                    scene_origin,
                )
            {
                chunk.records.push(PickRecord {
                    entity: SceneEntityId::Object(id),
                    world_bounds,
                    stroke_range: (stroke_start, stroke_end),
                    fill_range: (0, 0),
                    fill_index_range: (0, 0),
                    fill_opaque: false,
                });
            }
            let fp = fingerprint(document.object_revision(id), rgba, object.layer());
            self.object_chunk.insert(id, (chunk_index, fp));
        }

        chunk.estimated_instances = chunk.strokes.len();
        chunk.stroke_blocks = StrokeBlocks::build(&chunk.strokes, scene_origin);
        chunk.world_bounds = chunk
            .records
            .iter()
            .map(|record| record.world_bounds)
            .reduce(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)));
        chunk.dirty = false;

        let uploaded = upload(
            device,
            queue,
            &mut chunk.instance_gpu,
            &mut chunk.instance_capacity,
            bytemuck::cast_slice(&chunk.strokes),
            wgpu::BufferUsages::VERTEX,
            "Static Stroke Chunk Instance Buffer",
            limits.buffer_bytes,
        );
        chunk.instance_count = if uploaded { u32::try_from(chunk.strokes.len()).unwrap_or(0) } else { 0 };
    }
}

/// (Re)create `buffer` if `data` outgrew it, then write `data`.
#[allow(clippy::too_many_arguments)]
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &mut Option<wgpu::Buffer>,
    capacity_bytes: &mut usize,
    data: &[u8],
    usage: wgpu::BufferUsages,
    label: &'static str,
    max_buffer_bytes: usize,
) -> bool {
    if data.is_empty() {
        return true;
    }
    if data.len() > max_buffer_bytes {
        crate::userspace_error!(
            "{label} needs {} MiB, exceeding the GPU's {} MiB per-buffer limit; this stroke chunk will not be drawn",
            data.len() / (1024 * 1024),
            max_buffer_bytes / (1024 * 1024),
        );
        *buffer = None;
        *capacity_bytes = 0;
        return false;
    }
    if buffer.is_none() || data.len() > *capacity_bytes {
        *capacity_bytes = checked_buffer_capacity(data.len(), max_buffer_bytes).expect("static stroke data was checked against the device buffer limit");
        *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: *capacity_bytes as wgpu::BufferAddress,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    if let Some(buffer) = buffer {
        queue.write_buffer(buffer, 0, data);
    }
    true
}

fn checked_buffer_capacity(required: usize, maximum: usize) -> Option<usize> {
    if required == 0 || required > maximum {
        return None;
    }
    Some(required.checked_next_power_of_two().unwrap_or(maximum).min(maximum))
}
