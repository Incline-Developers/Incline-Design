use std::{cmp::Ordering, collections::BinaryHeap, path::PathBuf, sync::Arc};

use glam::{DVec3, Vec3};
use rayon::prelude::*;

use crate::{
    model::project::ProjectItemState,
    rendering::{graphics::frustum::OrientedBox, scene::bounds::fit_chunk_box},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PointCloudId(pub(crate) u64);

/// The decoded contents of a point cloud file, produced on a background
/// thread and sent back to the main thread via channel.
pub(crate) struct LoadedPointCloud {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) points: Arc<Vec<DVec3>>,
    /// Source-order packed RGBA8 values, when the input provides them.
    pub(crate) colors: Option<Arc<Vec<u32>>>,
    /// Source-order ASPRS classification codes, when the input provides them.
    pub(crate) classifications: Option<Arc<Vec<u8>>>,
    /// Spatially ordered, cloud-local render data prepared by the loader.
    pub(crate) prepared: Arc<PreparedPointCloud>,
    pub(crate) bounds: (DVec3, DVec3),
}

#[derive(Clone)]
pub(crate) struct OpenPointCloud {
    pub(crate) id: PointCloudId,
    pub(crate) state: ProjectItemState,
    pub(crate) name: String,
    pub(crate) points: Arc<Vec<DVec3>>,
    /// Source-order packed RGBA8 values, retained for lossless interchange.
    pub(crate) colors: Option<Arc<Vec<u32>>>,
    /// Source-order ASPRS classification codes. Their presence is what offers
    /// the bare-earth filter and the Survey classification view; a cloud whose
    /// file never classified anything carries `None`.
    pub(crate) classifications: Option<Arc<Vec<u8>>>,
    pub(crate) prepared: Arc<PreparedPointCloud>,
    pub(crate) bounds: (DVec3, DVec3),
    /// Uniform colour used when the file carries no per-point colours.
    pub(crate) color: [f32; 4],
    /// Screen-facing splat width in source/world metres.
    pub(crate) point_size: f32,
}

impl OpenPointCloud {
    pub(crate) fn entity_id(&self) -> crate::model::SceneEntityId {
        crate::model::SceneEntityId::PointCloud(self.id)
    }

    /// Whether the cloud has been through a ground filter, and so can be drawn
    /// by class or reduced to bare earth.
    pub(crate) fn is_classified(&self) -> bool {
        self.classifications.is_some()
    }
}

/// ASPRS LAS classification codes Incline names. Everything outside this range
/// is reserved or user-definable and draws in the fallback colour.
pub(crate) const CLASS_NEVER_CLASSIFIED: u8 = 0;
pub(crate) const CLASS_UNCLASSIFIED: u8 = 1;
pub(crate) const CLASS_GROUND: u8 = 2;

/// Display name for an ASPRS classification code, or `None` where the standard
/// reserves the code or leaves it to the producer.
pub(crate) fn classification_name(code: u8) -> Option<&'static str> {
    Some(match code {
        CLASS_NEVER_CLASSIFIED => "Created, never classified",
        CLASS_UNCLASSIFIED => "Unclassified",
        CLASS_GROUND => "Ground",
        3 => "Low vegetation",
        4 => "Medium vegetation",
        5 => "High vegetation",
        6 => "Building",
        7 => "Low point (noise)",
        9 => "Water",
        10 => "Rail",
        11 => "Road surface",
        13 => "Wire - guard",
        14 => "Wire - conductor",
        15 => "Transmission tower",
        16 => "Wire-structure connector",
        17 => "Bridge deck",
        18 => "High noise",
        19 => "Overhead structure",
        20 => "Ignored ground",
        21 => "Snow",
        22 => "Temporal exclusion",
        _ => return None,
    })
}

/// Packed RGBA8 (red in the low byte) for an ASPRS classification code, in the
/// layout [`PointInstance::color`] is read with.
///
/// The palette follows what survey software has long drawn these classes in -
/// bare earth tan, vegetation greening with height, noise a colour that occurs
/// nowhere in terrain - so a classified cloud reads the same here as it did in
/// whatever filtered it.
pub(crate) fn classification_color(code: u8) -> u32 {
    let rgb: u32 = match code {
        CLASS_NEVER_CLASSIFIED => 0x9aa0a6,
        CLASS_UNCLASSIFIED => 0xc8cdd2,
        CLASS_GROUND => 0xb08050,
        3 => 0xa8d08d,
        4 => 0x6aae4f,
        5 => 0x2e7d32,
        6 => 0xe06c4f,
        7 | 18 => 0xff3ddc,
        9 => 0x3f8fd1,
        10 => 0x8e6fbf,
        11 => 0x6e6e6e,
        12 => 0xb9a54b,
        13 => 0xe8c547,
        14 => 0xf2a93b,
        15 => 0xc08a2e,
        16 => 0xd9b45b,
        17 => 0x9c6b3f,
        19 => 0xb0729a,
        20 => 0x7a6046,
        21 => 0xe8f1f8,
        22 => 0x8891a0,
        _ => 0x7f8a99,
    };
    // Stored red-in-the-low-byte to match the packed RGBA8 vertex format.
    let [_, r, g, b] = rgb.to_be_bytes();
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16) | 0xff00_0000
}

/// Position-only instance used by clouds without per-point colours.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PointPosition {
    pub(crate) pos: [f32; 3],
}

/// Position and packed RGBA8 used by coloured clouds.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PointInstance {
    pub(crate) pos: [f32; 3],
    pub(crate) color: u32,
}

/// The two GPU instance layouts share a position, which is all the chunk
/// preparation below needs of them. Going through this instead of a per-layout
/// closure lets one generic builder serve both and keeps the position read
/// inlined.
pub(crate) trait RenderPoint: bytemuck::Pod + Send + Sync {
    fn pos(&self) -> [f32; 3];
}

impl RenderPoint for PointPosition {
    fn pos(&self) -> [f32; 3] {
        self.pos
    }
}

impl RenderPoint for PointInstance {
    fn pos(&self) -> [f32; 3] {
        self.pos
    }
}

pub(crate) enum PreparedPointData {
    Uncolored(Vec<PointPosition>),
    Colored(Vec<PointInstance>),
}

impl PreparedPointData {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Uncolored(points) => bytemuck::cast_slice(points),
            Self::Colored(points) => bytemuck::cast_slice(points),
        }
    }

    pub(crate) fn position(&self, index: usize) -> Option<[f32; 3]> {
        match self {
            Self::Uncolored(points) => points.get(index).map(|point| point.pos),
            Self::Colored(points) => points.get(index).map(|point| point.pos),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PointPickGroup {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) bounds_min: glam::Vec3,
    pub(crate) bounds_max: glam::Vec3,
}

pub(crate) struct PreparedPointChunk {
    /// Points are ordered so every power-of-two representative set down to a
    /// single point is a prefix. All LODs therefore share one CPU and GPU
    /// buffer.
    pub(crate) data: PreparedPointData,
    /// Full resolution through one representative point instance counts.
    pub(crate) level_counts: [u32; POINT_CLOUD_LOD_LEVELS],
    /// Robust (median) nearest-neighbour distance at full resolution, in cloud
    /// units. The renderer scales this by the LOD decimation to estimate the
    /// on-screen point spacing and pick a prefix dense enough to leave no gaps,
    /// replacing a per-frame screen-space coverage measurement. Zero when the
    /// chunk has too few points to define a spacing.
    pub(crate) base_spacing: f32,
    pub(crate) bounds_min: glam::Vec3,
    pub(crate) bounds_max: glam::Vec3,
    /// Culling box fitted to the chunk's points, relative to the cloud origin:
    /// a tilted slab on sloping ground where the axis-aligned box above would
    /// be mostly air (see [`fit_chunk_box`]).
    pub(crate) bounds: OrientedBox,
    /// Small Morton-coherent ranges used for CPU visibility queries without
    /// scanning an entire 256k-point render chunk on every snap poll.
    pub(crate) pick_groups: Vec<PointPickGroup>,
}

/// What a prepared cloud's instance colour channel holds, which decides how the
/// classification view is served.
///
/// A cloud with classifications but no RGB bakes the classification colours
/// straight into the channel, because the view it toggles back to is the
/// cloud's own uniform colour - which the shader applies without touching the
/// vertex buffer. A cloud that also carries RGB keeps that in the channel and
/// its classification code in the otherwise unused alpha byte, which the
/// shader maps through the class palette while the view is on. Neither
/// toggle touches the vertex buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointColorChannel {
    /// No colour channel at all: 12-byte instances drawn in the uniform colour.
    None,
    /// The file's own per-point RGB.
    Source,
    /// ASPRS classification colours.
    Classification,
}

pub(crate) struct PreparedPointCloud {
    pub(crate) origin: DVec3,
    pub(crate) chunks: Vec<PreparedPointChunk>,
    pub(crate) colored: bool,
    pub(crate) color_channel: PointColorChannel,
    /// Whether the instances' alpha bytes hold classification codes (only
    /// ever alongside `PointColorChannel::Source`).
    pub(crate) chunk_classifications: bool,
}

const POINTS_PER_SPATIAL_CHUNK: usize = 256 * 1024;
const POINTS_PER_PICK_GROUP: usize = 512;
// A 256k-point chunk is 2^18 points. Retaining all 19 power-of-two
// resolutions lets a distant chunk fall all the way to one representative
// instead of bottoming out at 1,024 splats (the previous nine-level limit).
// This only grows the per-chunk count table; point storage remains unchanged.
pub(crate) const POINT_CLOUD_LOD_LEVELS: usize = 19;

#[derive(Clone, Copy)]
struct MortonPointIndex {
    key: u64,
    source_index: usize,
}

/// Convert source doubles into a spatially coherent, render-ready hierarchy.
/// This is called by the point-cloud loader, never by the render thread.
pub(crate) fn prepare_for_render(points: &[DVec3], colors: Option<&[u32]>, classifications: Option<&[u8]>, bounds: (DVec3, DVec3)) -> PreparedPointCloud {
    let origin = (bounds.0 + bounds.1) * 0.5;
    let extent = (bounds.1 - bounds.0).max(DVec3::splat(f64::EPSILON));

    // Keep only a compact key/index record while sorting. This calculates the
    // expensive Morton key exactly once, lets the parallel unstable sort move
    // 16-byte records with cheap u64 comparisons, and avoids both Rayon's
    // sequential cached-key permutation and a full intermediate render-point
    // allocation. Widening the record to carry the render instance through the
    // sort - and so skip the gather below entirely - was measurably worse: the
    // sort moves each record O(log n) times, the gather touches it once.
    let mut sorted = points
        .par_iter()
        .enumerate()
        .filter(|(_, point)| point.is_finite())
        .map(|(source_index, point)| MortonPointIndex {
            key: morton_key(*point, bounds.0, extent),
            source_index,
        })
        .collect::<Vec<_>>();
    split_chunk_range(&mut sorted, extent);

    // Resolve each point's render instance exactly once, here, writing the
    // result in Morton order. `source_index` bears no relation to Morton order,
    // so every read is a random access into arrays that run to gigabytes - and
    // chunk building used to repeat that gather ten times per point, once to
    // place the point in its LOD prefix and nine more inside the
    // nearest-neighbour window scan. Doing it once leaves everything downstream
    // walking chunk-local slices that stay in cache.
    match (colors, classifications) {
        (Some(colors), _) => {
            // Points are drawn opaque, so the alpha byte is free to carry the
            // classification code the shader colours by in that view.
            let instances = gather_instances(&sorted, |point| {
                let color = colors.get(point.source_index).copied().unwrap_or(0xffff_ffff);
                PointInstance {
                    pos: (points[point.source_index] - origin).as_vec3().to_array(),
                    color: match classifications {
                        Some(codes) => (color & 0x00ff_ffff) | (u32::from(codes.get(point.source_index).copied().unwrap_or(CLASS_UNCLASSIFIED)) << 24),
                        None => color,
                    },
                }
            });
            PreparedPointCloud {
                origin,
                chunk_classifications: classifications.is_some(),
                chunks: build_chunks(&sorted, &instances, PreparedPointData::Colored),
                colored: true,
                color_channel: PointColorChannel::Source,
            }
        }
        (None, Some(codes)) => {
            let instances = gather_instances(&sorted, |point| PointInstance {
                pos: (points[point.source_index] - origin).as_vec3().to_array(),
                color: classification_color(codes.get(point.source_index).copied().unwrap_or(CLASS_UNCLASSIFIED)),
            });
            PreparedPointCloud {
                origin,
                chunks: build_chunks(&sorted, &instances, PreparedPointData::Colored),
                colored: true,
                color_channel: PointColorChannel::Classification,
                chunk_classifications: false,
            }
        }
        (None, None) => {
            let instances = gather_instances(&sorted, |point| PointPosition {
                pos: (points[point.source_index] - origin).as_vec3().to_array(),
            });
            PreparedPointCloud {
                origin,
                chunks: build_chunks(&sorted, &instances, PreparedPointData::Uncolored),
                colored: false,
                color_channel: PointColorChannel::None,
                chunk_classifications: false,
            }
        }
    }
}

/// Partition `points` in place into chunk-sized kd leaves, as surfaces are
/// (see `triangulation::spatial_surface_face_order`), then Morton-sort each
/// leaf for the LOD ordering. Fixed-size runs of one global Morton order were
/// the old split: a run crossing a high Morton boundary spans far more space
/// than its points need, so neighbouring chunks' boxes overlapped and a small
/// view touched several whole chunks. The slice always starts on a chunk
/// boundary, so cutting after a whole number of chunks keeps both halves
/// aligned with `build_chunks`' fixed stride.
fn split_chunk_range(points: &mut [MortonPointIndex], extent: DVec3) {
    let chunks = points.len().div_ceil(POINTS_PER_SPATIAL_CHUNK);
    if chunks <= 1 {
        // Include the source index so coincident quantized points retain a
        // stable order even though the faster unstable parallel sort is used.
        // Comparing the fields in place beats a `(u64, usize)` sort key: the
        // key would be rebuilt on every comparison, and equal Morton keys are
        // rare enough that the tie-break is almost never reached.
        points.par_sort_unstable_by(|a, b| a.key.cmp(&b.key).then_with(|| a.source_index.cmp(&b.source_index)));
        return;
    }
    // Quantized cell coordinates decoded from the key, so the split reads
    // only the compact records rather than gathering source positions.
    let bounds = |(mut min, mut max): ([u32; 3], [u32; 3]), point: &MortonPointIndex| {
        for axis in 0..3 {
            let cell = morton_axis(point.key, axis);
            min[axis] = min[axis].min(cell);
            max[axis] = max[axis].max(cell);
        }
        (min, max)
    };
    let empty = ([u32::MAX; 3], [0; 3]);
    let (min, max) = points.par_iter().fold(|| empty, bounds).reduce(
        || empty,
        |(a_min, a_max), (b_min, b_max)| {
            (
                std::array::from_fn(|axis| a_min[axis].min(b_min[axis])),
                std::array::from_fn(|axis| a_max[axis].max(b_max[axis])),
            )
        },
    );
    // Cells are normalized per axis, so compare their spans in world units.
    let span = |axis: usize| f64::from(max[axis].saturating_sub(min[axis])) * extent[axis];
    let axis = (0..3).max_by(|&a, &b| span(a).total_cmp(&span(b))).unwrap_or(0);
    let cut = (chunks / 2) * POINTS_PER_SPATIAL_CHUNK;
    points.select_nth_unstable_by_key(cut, |point| morton_axis(point.key, axis));
    let (left, right) = points.split_at_mut(cut);
    rayon::join(|| split_chunk_range(left, extent), || split_chunk_range(right, extent));
}

fn gather_instances<T: RenderPoint>(sorted: &[MortonPointIndex], instance: impl Fn(&MortonPointIndex) -> T + Send + Sync) -> Vec<T> {
    sorted.par_iter().map(instance).collect()
}

fn build_chunks<T: RenderPoint>(sorted: &[MortonPointIndex], instances: &[T], wrap: fn(Vec<T>) -> PreparedPointData) -> Vec<PreparedPointChunk> {
    sorted
        .par_chunks(POINTS_PER_SPATIAL_CHUNK)
        .zip(instances.par_chunks(POINTS_PER_SPATIAL_CHUNK))
        .map(|(keys, chunk)| build_chunk(keys, chunk, wrap))
        .collect()
}

fn build_chunk<T: RenderPoint>(keys: &[MortonPointIndex], chunk: &[T], wrap: fn(Vec<T>) -> PreparedPointData) -> PreparedPointChunk {
    // Measured while the chunk is still in Morton order, and by walking it
    // forwards, before the LOD permutation below scrambles it.
    let base_spacing = chunk_base_spacing(chunk);
    let (indices, level_counts) = density_aware_prefix_indices(keys);
    let ordered = indices.iter().map(|&index| chunk[index]).collect::<Vec<_>>();
    let (bounds_min, bounds_max) = local_bounds(ordered.iter().map(T::pos));
    let chunk_origin = ((bounds_min + bounds_max) * 0.5).as_dvec3();
    let (center, axes, half_extents) = fit_chunk_box(
        chunk,
        |point| Vec3::from_array(point.pos()).as_dvec3() - chunk_origin,
        &[],
        chunk_origin,
        (bounds_max - bounds_min).as_dvec3(),
    );
    let bounds = OrientedBox {
        center: center.as_vec3(),
        axes,
        half_extents,
    };
    let pick_groups = build_pick_groups(&ordered, level_counts, T::pos);
    PreparedPointChunk {
        data: wrap(ordered),
        level_counts,
        base_spacing,
        bounds_min,
        bounds_max,
        bounds,
        pick_groups,
    }
}

pub(crate) fn finite_bounds(points: &[DVec3]) -> Option<(DVec3, DVec3)> {
    let (min, max) = points
        .par_iter()
        .copied()
        .filter(|point| point.is_finite())
        .fold(
            || (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(min, max), point| (min.min(point), max.max(point)),
        )
        .reduce(
            || (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)),
        );
    (min.is_finite() && max.is_finite()).then_some((min, max))
}

/// Estimate the full-resolution point spacing of a Morton-sorted chunk. Morton
/// order places spatial neighbours adjacently, so the nearest of a small window
/// of successors approximates each point's true nearest neighbour without a
/// spatial index. A high percentile - not the median - is used deliberately:
/// the renderer sizes a prefix so splats at this spacing touch, and targeting
/// the 90th percentile means ~90% of the surface is covered (matching the
/// coverage goal of the screen-space probing this replaces). The window-min
/// also rejects the occasional large jump where the curve crosses a cell
/// boundary.
const BASE_SPACING_COVERAGE_PERCENTILE: f32 = 0.9;
/// Window measurements taken per chunk. The percentile describes the chunk's
/// distance distribution, so a strided sample estimates it just as well as
/// measuring every point: 16k draws put the 90th percentile within a fraction
/// of a percent, far inside the factor-of-two steps between LOD levels it
/// feeds. Morton order makes a strided sample spatially spread, not clustered.
const BASE_SPACING_SAMPLES: usize = 16 * 1024;

fn chunk_base_spacing<T: RenderPoint>(chunk: &[T]) -> f32 {
    const NEIGHBOUR_WINDOW: usize = 8;
    if chunk.len() < 2 {
        return 0.0;
    }
    let stride = chunk.len().div_ceil(BASE_SPACING_SAMPLES).max(1);
    let mut spacings = Vec::with_capacity((chunk.len() - 1).div_ceil(stride));
    for index in (0..chunk.len() - 1).step_by(stride) {
        let here = Vec3::from_array(chunk[index].pos());
        let window_end = (index + 1 + NEIGHBOUR_WINDOW).min(chunk.len());
        // Squared distances between finite positions are never NaN, so plain
        // `f32::min` is enough and lowers to one `minss`. The `total_cmp`
        // ordering this replaces cost more than the distances themselves.
        let mut nearest = f32::INFINITY;
        for other in &chunk[index + 1..window_end] {
            nearest = nearest.min(here.distance_squared(Vec3::from_array(other.pos())));
        }
        if nearest.is_finite() {
            spacings.push(nearest.sqrt());
        }
    }
    if spacings.is_empty() {
        return 0.0;
    }
    let percentile = (((spacings.len() - 1) as f32) * BASE_SPACING_COVERAGE_PERCENTILE).round() as usize;
    spacings.select_nth_unstable_by(percentile, f32::total_cmp);
    spacings[percentile]
}

fn build_pick_groups<T>(points: &[T], level_counts: [u32; POINT_CLOUD_LOD_LEVELS], position: impl Fn(&T) -> [f32; 3] + Copy) -> Vec<PointPickGroup> {
    let mut groups = Vec::new();
    let mut tier_start = 0;
    for tier_end in level_counts.iter().rev().map(|count| *count as usize) {
        for start in (tier_start..tier_end).step_by(POINTS_PER_PICK_GROUP) {
            let end = (start + POINTS_PER_PICK_GROUP).min(tier_end);
            let (bounds_min, bounds_max) = local_bounds(points[start..end].iter().map(position));
            groups.push(PointPickGroup {
                start: start as u32,
                end: end as u32,
                bounds_min,
                bounds_max,
            });
        }
        tier_start = tier_end;
    }
    debug_assert_eq!(tier_start, points.len());
    groups
}

/// Coarse prefixes are selected by subdividing occupied Morton space instead
/// of point-array indices. This gives an isolated point its own representative
/// as soon as its spatial cell separates from a dense cluster. The first 1,024
/// points cover every LOD used for an initial GPU upload; finer prefixes fall
/// back to the inexpensive shuffled-Morton order because density differences
/// matter progressively less as a chunk approaches full resolution.
const DENSITY_AWARE_PREFIX_POINTS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MortonCell {
    start: usize,
    end: usize,
    representative: usize,
    split_bit: u32,
}

impl Ord for MortonCell {
    fn cmp(&self, other: &Self) -> Ordering {
        self.split_bit
            .cmp(&other.split_bit)
            // At equal spatial depth, process cells in Morton order for
            // deterministic, spatially coherent refinement.
            .then_with(|| other.start.cmp(&self.start))
            .then_with(|| other.end.cmp(&self.end))
    }
}

impl PartialOrd for MortonCell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn density_aware_prefix_indices(points: &[MortonPointIndex]) -> (Vec<usize>, [u32; POINT_CLOUD_LOD_LEVELS]) {
    let mut level_counts = [0; POINT_CLOUD_LOD_LEVELS];
    if points.is_empty() {
        return (Vec::new(), level_counts);
    }

    let mut ordered = Vec::with_capacity(points.len());
    let mut selected = vec![false; points.len()];
    let mut cells = BinaryHeap::new();
    let root_representative = representative_near_morton_center(points, 0, points.len());
    ordered.push(root_representative);
    selected[root_representative] = true;
    if let Some(root) = morton_cell(points, 0, points.len(), root_representative) {
        cells.push(root);
    }

    // This is the previous fixed-stride order. It remains a useful cheap and
    // deterministic fill order once spatial representatives have established
    // the coarse hierarchy, and guarantees every source point appears once.
    let fallback = shuffled_morton_indices(points.len());
    let mut fallback_cursor = 0;

    for level in (0..POINT_CLOUD_LOD_LEVELS).rev() {
        let stride = 1usize << level;
        let target = points.len().div_ceil(stride);
        let density_target = target.min(DENSITY_AWARE_PREFIX_POINTS);

        while ordered.len() < density_target {
            let Some(cell) = cells.pop() else {
                break;
            };
            let (left, right, introduced) = split_morton_cell(points, cell);
            debug_assert!(!selected[introduced]);
            selected[introduced] = true;
            ordered.push(introduced);
            if let Some(left) = left {
                cells.push(left);
            }
            if let Some(right) = right {
                cells.push(right);
            }
        }

        while ordered.len() < target {
            let index = fallback[fallback_cursor];
            fallback_cursor += 1;
            if !selected[index] {
                selected[index] = true;
                ordered.push(index);
            }
        }
        level_counts[level] = ordered.len() as u32;
    }
    debug_assert!(selected.into_iter().all(|is_selected| is_selected));
    (ordered, level_counts)
}

fn shuffled_morton_indices(point_count: usize) -> Vec<usize> {
    let mut ordered = Vec::with_capacity(point_count);
    for level in (0..POINT_CLOUD_LOD_LEVELS).rev() {
        let stride = 1usize << level;
        if level + 1 == POINT_CLOUD_LOD_LEVELS {
            ordered.extend((0..point_count).step_by(stride));
        } else {
            ordered.extend((stride..point_count).step_by(stride << 1));
        }
    }
    debug_assert_eq!(ordered.len(), point_count);
    ordered
}

fn morton_cell(points: &[MortonPointIndex], start: usize, end: usize, representative: usize) -> Option<MortonCell> {
    let differing_bits = points[start].key ^ points[end - 1].key;
    (differing_bits != 0).then(|| MortonCell {
        start,
        end,
        representative,
        split_bit: u64::BITS - 1 - differing_bits.leading_zeros(),
    })
}

fn split_morton_cell(points: &[MortonPointIndex], cell: MortonCell) -> (Option<MortonCell>, Option<MortonCell>, usize) {
    let mask = 1u64 << cell.split_bit;
    let split = cell.start + points[cell.start..cell.end].partition_point(|point| point.key & mask == 0);
    debug_assert!(split > cell.start && split < cell.end);

    let (left_representative, right_representative, introduced) = if cell.representative < split {
        let right = representative_near_morton_center(points, split, cell.end);
        (cell.representative, right, right)
    } else {
        let left = representative_near_morton_center(points, cell.start, split);
        (left, cell.representative, left)
    };
    (
        morton_cell(points, cell.start, split, left_representative),
        morton_cell(points, split, cell.end, right_representative),
        introduced,
    )
}

fn representative_near_morton_center(points: &[MortonPointIndex], start: usize, end: usize) -> usize {
    debug_assert!(start < end);
    let first = points[start].key;
    let last = points[end - 1].key;
    let target = first + (last - first) / 2;
    let insertion = start + points[start..end].partition_point(|point| point.key < target);
    let after = insertion.min(end - 1);
    if after == start {
        return after;
    }
    let before = after - 1;
    let before_distance = points[before].key.abs_diff(target);
    let after_distance = points[after].key.abs_diff(target);
    if before_distance <= after_distance { before } else { after }
}

fn local_bounds(points: impl Iterator<Item = [f32; 3]>) -> (glam::Vec3, glam::Vec3) {
    points
        .map(glam::Vec3::from_array)
        .fold((glam::Vec3::splat(f32::INFINITY), glam::Vec3::splat(f32::NEG_INFINITY)), |(min, max), point| {
            (min.min(point), max.max(point))
        })
}

fn morton_key(point: DVec3, min: DVec3, extent: DVec3) -> u64 {
    let normalized = ((point - min) / extent).clamp(DVec3::ZERO, DVec3::ONE);
    let scale = ((1u32 << 21) - 1) as f64;
    let x = (normalized.x * scale) as u32;
    let y = (normalized.y * scale) as u32;
    let z = (normalized.z * scale) as u32;
    interleave_21(x) | (interleave_21(y) << 1) | (interleave_21(z) << 2)
}

/// One axis's quantized coordinate back out of a [`morton_key`].
fn morton_axis(key: u64, axis: usize) -> u32 {
    let mut value = (key >> axis) & 0x1249_2492_4924_9249;
    value = (value | value >> 2) & 0x10c3_0c30_c30c_30c3;
    value = (value | value >> 4) & 0x100f_00f0_0f00_f00f;
    value = (value | value >> 8) & 0x001f_0000_ff00_00ff;
    value = (value | value >> 16) & 0x001f_0000_0000_ffff;
    ((value | value >> 32) & 0x1f_ffff) as u32
}

fn interleave_21(value: u32) -> u64 {
    let mut value = u64::from(value & 0x1f_ffff);
    value = (value | value << 32) & 0x001f_0000_0000_ffff;
    value = (value | value << 16) & 0x001f_0000_ff00_00ff;
    value = (value | value << 8) & 0x100f_00f0_0f00_f00f;
    value = (value | value << 4) & 0x10c3_0c30_c30c_30c3;
    (value | value << 2) & 0x1249_2492_4924_9249
}
