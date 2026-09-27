//! Geometry picking against the world-space vertex buffers the renderer
//! already produces.
//!
//! Each top-level entity records the range of stroke/fill vertices it emitted
//! (`PickRecord`). To pick, we project those world-space vertices to the screen
//! and find the geometry nearest the cursor, returning its true world position
//! (including Z) and owning entity handle.

use std::{collections::HashSet, ops::Range};

use glam::{DMat4, DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    Size,
    model::{
        Document, Object, ObjectId, ObjectPoint, SceneEntityId,
        spatial::{ObjectSnapIndex, projected_box_overlaps},
    },
    rendering::{StrokeInstance, Vertex, camera::SectionSlab},
};

#[derive(Clone, Debug)]
pub(crate) struct PickRecord {
    pub(crate) entity: SceneEntityId,
    /// Cached world-space bounds of all rendered vertices owned by this
    /// record. Picking projects this small box before touching potentially
    /// very large CPU vertex/index ranges.
    pub(crate) world_bounds: (DVec3, DVec3),
    /// Half-open range into the stroke instance buffer.
    pub(crate) stroke_range: (u32, u32),
    /// Half-open range into the fill vertex buffer.
    pub(crate) fill_range: (u32, u32),
    /// Half-open range into the fill index buffer.
    pub(crate) fill_index_range: (u32, u32),
    /// Whether this record's solid fill fully occludes geometry behind it.
    pub(crate) fill_opaque: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PickHit {
    pub(crate) entity: SceneEntityId,
    pub(crate) world: DVec3,
}

/// One set of pick records with the CPU vertex/index buffers their ranges
/// index into. The per-rebuild stream is one group; each static stroke chunk
/// is another (with no fill geometry).
#[derive(Clone, Copy)]
pub(crate) struct PickGeometry<'a> {
    /// Optional cached bounds for the whole stream chunk. A cursor outside it
    /// skips every member record without touching the CPU geometry.
    pub(crate) world_bounds: Option<(DVec3, DVec3)>,
    pub(crate) records: &'a [PickRecord],
    pub(crate) strokes: &'a [StrokeInstance],
    /// Block bounds over `strokes`, built alongside them.
    pub(crate) stroke_blocks: &'a StrokeBlocks,
    pub(crate) fill_verts: &'a [Vertex],
    pub(crate) fill_indices: &'a [u32],
}

/// Consecutive stroke instances per block.
const STROKE_BLOCK: usize = 16;
/// Blocks per BVH leaf.
const BLOCKS_PER_LEAF: usize = 4;

/// A BVH over the world bounds of fixed runs of consecutive stroke
/// instances. A record's own bounds say little when it encloses the cursor -
/// concentric contours put every ring's box under it - but consecutive
/// segments of one string are neighbours, so their runs bound tightly, and a
/// pick visits only the runs near the cursor wherever their records are.
#[derive(Default)]
pub(crate) struct StrokeBlocks {
    /// Instance count the blocks were built over; picking a stream of any
    /// other length walks it whole rather than trust stale bounds.
    len: usize,
    blocks: Vec<(DVec3, DVec3)>,
    /// Block indices in leaf order.
    order: Vec<u32>,
    nodes: Vec<StrokeBlockNode>,
}

#[derive(Clone, Copy)]
struct StrokeBlockNode {
    min: DVec3,
    max: DVec3,
    /// Leaf: first index into `order`. Interior: the right child; the left
    /// child follows its parent.
    start_or_right: u32,
    /// Blocks in a leaf; zero for an interior node.
    count: u32,
}

impl StrokeBlocks {
    pub(crate) fn build(strokes: &[StrokeInstance], scene_origin: DVec3) -> Self {
        let blocks = strokes
            .par_chunks(STROKE_BLOCK)
            .map(|block| {
                world_bounds_from_local_positions(
                    block.iter().flat_map(|stroke| {
                        let (start, end) = stroke.world_ends();
                        [start, end]
                    }),
                    scene_origin,
                )
                .unwrap_or((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)))
            })
            .collect::<Vec<_>>();
        let mut order = (0..blocks.len() as u32)
            .filter(|&block| blocks[block as usize].0.cmple(blocks[block as usize].1).all())
            .collect::<Vec<_>>();
        let mut nodes = Vec::with_capacity(2 * order.len().div_ceil(BLOCKS_PER_LEAF));
        if !order.is_empty() {
            let len = order.len();
            build_block_node(&blocks, &mut order, 0, len, &mut nodes);
        }
        Self {
            len: strokes.len(),
            blocks,
            order,
            nodes,
        }
    }

    /// Visit, in ascending order, the instance ranges of blocks whose bounds
    /// project within `threshold` pixels of `cursor`; the whole stream when
    /// the blocks were built for another.
    fn for_each_near(&self, len: usize, view_proj: &DMat4, screen: Size, cursor: DVec2, threshold: f64, mut visit: impl FnMut(Range<usize>)) {
        if self.len != len {
            if len > 0 {
                visit(0..len);
            }
            return;
        }
        let mut near: Vec<u32> = Vec::new();
        let mut stack = if self.nodes.is_empty() { Vec::new() } else { vec![0_usize] };
        while let Some(index) = stack.pop() {
            let node = self.nodes[index];
            if !projected_box_overlaps(node.min, node.max, view_proj, screen, cursor, threshold) {
                continue;
            }
            if node.count > 0 {
                let leaf = &self.order[node.start_or_right as usize..(node.start_or_right + node.count) as usize];
                near.extend(leaf.iter().filter(|&&block| {
                    let (min, max) = self.blocks[block as usize];
                    projected_box_overlaps(min, max, view_proj, screen, cursor, threshold)
                }));
            } else {
                stack.push(node.start_or_right as usize);
                stack.push(index + 1);
            }
        }
        near.sort_unstable();
        for block in near {
            let start = block as usize * STROKE_BLOCK;
            visit(start..(start + STROKE_BLOCK).min(len));
        }
    }
}

fn build_block_node(blocks: &[(DVec3, DVec3)], order: &mut [u32], start: usize, end: usize, nodes: &mut Vec<StrokeBlockNode>) -> usize {
    let (min, max) = order[start..end]
        .iter()
        .fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(min, max), &block| {
            let (a, b) = blocks[block as usize];
            (min.min(a), max.max(b))
        });
    let index = nodes.len();
    nodes.push(StrokeBlockNode {
        min,
        max,
        start_or_right: start as u32,
        count: (end - start) as u32,
    });
    if end - start <= BLOCKS_PER_LEAF {
        return index;
    }
    let extent = max - min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let middle = start + (end - start) / 2;
    let centre = |block: &u32| {
        let (a, b) = blocks[*block as usize];
        (a[axis] + b[axis]) * 0.5
    };
    order[start..end].select_nth_unstable_by(middle - start, |a, b| centre(a).total_cmp(&centre(b)));
    build_block_node(blocks, order, start, middle, nodes);
    let right = build_block_node(blocks, order, middle, end, nodes);
    nodes[index].start_or_right = right as u32;
    nodes[index].count = 0;
    index
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextPickRecord {
    pub(crate) entity: SceneEntityId,
    pub(crate) corners: [DVec3; 4],
}

/// Project a world point to physical screen pixels. Returns `None` when the
/// point is on/behind the camera plane (`w <= 0`).
pub(crate) fn world_to_screen(view_proj: &DMat4, world: DVec3, screen: Size) -> Option<DVec2> {
    let clip = *view_proj * world.extend(1.0);
    if clip.w.abs() <= f64::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    Some(DVec2::new((ndc.x * 0.5 + 0.5) * screen.0 as f64, (0.5 - ndc.y * 0.5) * screen.1 as f64))
}

/// Project a world point to physical screen pixels without rejecting points
/// outside the camera's fitted depth range. This is for foreground overlays
/// whose screen position remains meaningful even when scene clipping excludes
/// the world point itself.
pub(crate) fn world_to_screen_unclipped_depth(view_proj: &DMat4, world: DVec3, screen: Size) -> Option<DVec2> {
    let clip = *view_proj * world.extend(1.0);
    if clip.w.abs() <= f64::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    ndc.is_finite()
        .then(|| DVec2::new((ndc.x * 0.5 + 0.5) * screen.0 as f64, (0.5 - ndc.y * 0.5) * screen.1 as f64))
}

/// Parameter `t in [0, 1]` of the closest point on segment `a-b` to `p`.
pub(crate) fn closest_t_on_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= f64::EPSILON {
        return 0.0;
    }
    ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
}

/// Clamp a recorded half-open buffer range to the data that survived GPU
/// stream truncation. A record may begin beyond the retained prefix; that is
/// an empty range, not a slice with `start > end`.
pub(crate) fn clamped_range(range: (u32, u32), len: usize) -> Range<usize> {
    let start = (range.0 as usize).min(len);
    let end = (range.1 as usize).min(len).max(start);
    start..end
}

/// Compute world-space bounds from renderer-local f32 positions. Scene builds
/// call this once when creating a pick record; cursor polls reuse the result.
pub(crate) fn world_bounds_from_local_positions(positions: impl Iterator<Item = [f32; 3]>, scene_origin: DVec3) -> Option<(DVec3, DVec3)> {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    let mut any = false;
    for position in positions {
        let world = local_vertex_world(position, scene_origin);
        if !world.is_finite() {
            continue;
        }
        min = min.min(world);
        max = max.max(world);
        any = true;
    }
    any.then_some((min, max))
}

/// Recover the world-space point corresponding to a screen-space parameter
/// along a projected segment. Perspective projection is affine only after
/// dividing by clip W, so interpolating the endpoints directly with
/// `screen_t` returns the wrong point whenever their depths differ.
pub(crate) fn perspective_correct_segment_point(view_proj: &DMat4, a: DVec3, b: DVec3, screen_t: f64) -> DVec3 {
    let t = screen_t.clamp(0.0, 1.0);
    let wa = (*view_proj * a.extend(1.0)).w;
    let wb = (*view_proj * b.extend(1.0)).w;
    if wa.abs() <= f64::EPSILON || wb.abs() <= f64::EPSILON {
        return a.lerp(b, t);
    }
    let a_weight = (1.0 - t) / wa;
    let b_weight = t / wb;
    let denominator = a_weight + b_weight;
    if denominator.abs() <= f64::EPSILON || !denominator.is_finite() {
        return a.lerp(b, t);
    }
    (a * a_weight + b * b_weight) / denominator
}

fn perspective_correct_triangle_point(view_proj: &DMat4, points: [DVec3; 3], screen_weights: DVec3) -> DVec3 {
    let clip_w = points.map(|point| (*view_proj * point.extend(1.0)).w);
    if clip_w.iter().any(|w| w.abs() <= f64::EPSILON) {
        return points[0] * screen_weights.x + points[1] * screen_weights.y + points[2] * screen_weights.z;
    }
    let weights = DVec3::new(screen_weights.x / clip_w[0], screen_weights.y / clip_w[1], screen_weights.z / clip_w[2]);
    let denominator = weights.element_sum();
    if denominator.abs() <= f64::EPSILON || !denominator.is_finite() {
        return points[0] * screen_weights.x + points[1] * screen_weights.y + points[2] * screen_weights.z;
    }
    (points[0] * weights.x + points[1] * weights.y + points[2] * weights.z) / denominator
}

fn projected_depth(view_proj: &DMat4, world: DVec3) -> f64 {
    let clip = *view_proj * world.extend(1.0);
    if clip.w.abs() <= f64::EPSILON {
        f64::INFINITY
    } else {
        // Reversed-Z stores nearer geometry at larger NDC depth. Negate it so
        // the existing CPU pick ordering can continue treating smaller as nearer.
        -(clip.z / clip.w)
    }
}

fn screen_hit_is_better(distance: f64, depth: f64, best_distance: f64, best_depth: f64) -> bool {
    const DISTANCE_TIE_EPSILON: f64 = 1.0e-6;
    distance < best_distance - DISTANCE_TIE_EPSILON || ((distance - best_distance).abs() <= DISTANCE_TIE_EPSILON && depth < best_depth)
}

fn record_projects_near_cursor(record: &PickRecord, view_proj: &DMat4, screen: Size, cursor: DVec2, threshold: f64) -> bool {
    projected_box_overlaps(record.world_bounds.0, record.world_bounds.1, view_proj, screen, cursor, threshold)
}

pub(crate) fn local_vertex_world(position: [f32; 3], scene_origin: DVec3) -> DVec3 {
    DVec3::from_array(position.map(f64::from)) + scene_origin
}

/// Project a world point after testing it against `slab`, the section's
/// world-space visible slice: a candidate is judged against the slab, not
/// the camera's depth range. A `None` slab means every view shows it all.
pub(crate) fn slab_screen_point(slab: Option<SectionSlab>, view_proj: &DMat4, screen: Size, world: DVec3) -> Option<DVec2> {
    if slab.is_some_and(|slab| !slab.contains(world)) {
        return None;
    }
    world_to_screen(view_proj, world, screen)
}

/// Clip `a`-`b` to `slab` and project the endpoints; `None` if either step fails.
pub(crate) fn slab_clipped_screen_segment(slab: Option<SectionSlab>, view_proj: &DMat4, screen: Size, a: DVec3, b: DVec3) -> Option<(DVec2, DVec2)> {
    let (wa, wb) = slab_clipped_segment(slab, a, b)?;
    let sa = world_to_screen(view_proj, wa, screen)?;
    let sb = world_to_screen(view_proj, wb, screen)?;
    Some((sa, sb))
}

/// Clip `a`-`b` to the section slab: returns the portion inside it, or
/// `None` if both endpoints lie outside the same wall.
pub(crate) fn slab_clipped_segment(slab: Option<SectionSlab>, a: DVec3, b: DVec3) -> Option<(DVec3, DVec3)> {
    let Some(slab) = slab else {
        return Some((a, b));
    };
    let (enter, leave) = slab.clip_segment(a, b)?;
    Some((a.lerp(b, enter), a.lerp(b, leave)))
}

/// Find the entity geometry nearest the cursor within `threshold_px`.
///
/// Rendered stroke quads emit a fixed two-triangle index pattern. We recognize
/// that pattern and test the centerline once, rather than testing every
/// tessellated triangle edge. Screen-space markers and round joins use other
/// index patterns, so they fall back to the triangle-edge path below.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pick_nearest(
    groups: &[PickGeometry<'_>],
    scene_origin: DVec3,
    view_proj: &DMat4,
    screen: Size,
    cursor_px: (f32, f32),
    threshold_px: f32,
    frozen: &HashSet<SceneEntityId>,
    slab: Option<SectionSlab>,
) -> Option<PickHit> {
    let cursor = DVec2::new(cursor_px.0 as f64, cursor_px.1 as f64);
    let mut best_dist = threshold_px as f64;
    let mut best_stroke_depth = f64::INFINITY;
    let mut best_stroke_hit: Option<PickHit> = None;
    let mut best_fill_vertex_dist = threshold_px as f64;
    let mut best_fill_vertex_depth = f64::INFINITY;
    let mut best_fill_vertex_hit: Option<PickHit> = None;
    let mut best_fill_hit: Option<PickHit> = None;
    let mut best_fill_depth = f64::INFINITY;
    let mut best_opaque_fill_hit: Option<PickHit> = None;
    let mut best_opaque_fill_depth = f64::INFINITY;

    for group in groups {
        let PickGeometry {
            world_bounds,
            records,
            strokes,
            stroke_blocks,
            fill_verts,
            fill_indices,
        } = *group;
        if world_bounds.is_some_and(|bounds| !projected_box_overlaps(bounds.0, bounds.1, view_proj, screen, cursor, threshold_px as f64)) {
            continue;
        }
        // Strokes: only the instance runs near the cursor, attributed back
        // to their records, which the builders emit in stream order.
        let mut last_record = 0;
        stroke_blocks.for_each_near(strokes.len(), view_proj, screen, cursor, threshold_px as f64, |near| {
            let first = records[last_record..].partition_point(|rec| (rec.stroke_range.1 as usize) <= near.start) + last_record;
            last_record = first;
            for rec in records[first..].iter().take_while(|rec| (rec.stroke_range.0 as usize) < near.end) {
                // Frozen entities are visible but not selectable.
                if frozen.contains(&rec.entity) {
                    continue;
                }
                let owned = clamped_range(rec.stroke_range, strokes.len());
                let span = owned.start.max(near.start)..owned.end.min(near.end);
                // Lines are measured along their length. A record with none
                // (a point's cross, a lone marker) is measured at its
                // anchors; a line's joins sit on its own ends and add nothing.
                let has_lines = strokes[owned]
                    .iter()
                    .any(|stroke| !stroke.selection_only() && stroke.world_ends().0 != stroke.world_ends().1);
                for stroke in strokes[span].iter().filter(|stroke| !stroke.selection_only()) {
                    let (a, b) = stroke.world_ends();
                    if has_lines && a == b {
                        continue;
                    }
                    let (wa, wb) = (local_vertex_world(a, scene_origin), local_vertex_world(b, scene_origin));
                    let Some((wa, wb)) = slab_clipped_segment(slab, wa, wb) else {
                        continue;
                    };
                    if let (Some(sa), Some(sb)) = (world_to_screen(view_proj, wa, screen), world_to_screen(view_proj, wb, screen)) {
                        let t = closest_t_on_segment(cursor, sa, sb);
                        let dist = (sa + (sb - sa) * t).distance(cursor);
                        let world = perspective_correct_segment_point(view_proj, wa, wb, t);
                        let depth = projected_depth(view_proj, world);
                        if screen_hit_is_better(dist, depth, best_dist, best_stroke_depth) {
                            best_dist = dist;
                            best_stroke_depth = depth;
                            best_stroke_hit = Some(PickHit { entity: rec.entity, world });
                        }
                    }
                }
            }
        });

        for rec in records {
            // Fills are judged per record; a record without one is skipped
            // before any projection.
            if rec.fill_range.0 == rec.fill_range.1 && rec.fill_index_range.0 == rec.fill_index_range.1 {
                continue;
            }
            if frozen.contains(&rec.entity) || !record_projects_near_cursor(rec, view_proj, screen, cursor, threshold_px as f64) {
                continue;
            }
            for vert in &fill_verts[clamped_range(rec.fill_range, fill_verts.len())] {
                let world = local_vertex_world(vert.pos, scene_origin);
                if let Some(sp) = slab_screen_point(slab, view_proj, screen, world) {
                    let dist = sp.distance(cursor);
                    let depth = projected_depth(view_proj, world);
                    if screen_hit_is_better(dist, depth, best_fill_vertex_dist, best_fill_vertex_depth) {
                        best_fill_vertex_dist = dist;
                        best_fill_vertex_depth = depth;
                        best_fill_vertex_hit = Some(PickHit { entity: rec.entity, world });
                    }
                }
            }

            let record_fill_indices = &fill_indices[clamped_range(rec.fill_index_range, fill_indices.len())];
            for triangle in record_fill_indices.as_chunks::<3>().0 {
                let [Some(a), Some(b), Some(c)] = [
                    fill_verts.get(triangle[0] as usize),
                    fill_verts.get(triangle[1] as usize),
                    fill_verts.get(triangle[2] as usize),
                ] else {
                    continue;
                };
                let wa = local_vertex_world(a.pos, scene_origin);
                let wb = local_vertex_world(b.pos, scene_origin);
                let wc = local_vertex_world(c.pos, scene_origin);
                let (Some(sa), Some(sb), Some(sc)) = (
                    world_to_screen(view_proj, wa, screen),
                    world_to_screen(view_proj, wb, screen),
                    world_to_screen(view_proj, wc, screen),
                ) else {
                    continue;
                };
                if let Some(weights) = triangle_weights(cursor, sa, sb, sc) {
                    let world = perspective_correct_triangle_point(view_proj, [wa, wb, wc], weights);
                    // Reject only the hit point; the rest of the triangle may still be visible.
                    if slab.is_some_and(|slab| !slab.contains(world)) {
                        continue;
                    }
                    let depth = projected_depth(view_proj, world);
                    if depth < best_fill_depth {
                        best_fill_depth = depth;
                        best_fill_hit = Some(PickHit { entity: rec.entity, world });
                    }
                    if rec.fill_opaque && depth < best_opaque_fill_depth {
                        best_opaque_fill_depth = depth;
                        best_opaque_fill_hit = Some(PickHit { entity: rec.entity, world });
                    }
                }
            }
        }
    }

    let foreground = best_stroke_hit
        .map(|hit| (hit, best_stroke_depth))
        .or_else(|| best_fill_vertex_hit.map(|hit| (hit, best_fill_vertex_depth)));
    if let Some((hit, depth)) = foreground {
        if best_opaque_fill_depth + 1.0e-9 < depth {
            return best_opaque_fill_hit;
        }
        return Some(hit);
    }
    best_fill_hit
}

/// True only when every corner of the quad lies beyond the same slab wall.
fn quad_excluded_by_slab(slab: &SectionSlab, corners: &[DVec3; 4]) -> bool {
    let distance = corners.map(|corner| slab.signed_distance(corner));
    distance.iter().all(|&d| d > slab.half_width) || distance.iter().all(|&d| d < -slab.half_width)
}

pub(crate) fn pick_text(
    records: &[TextPickRecord],
    view_proj: &DMat4,
    screen: Size,
    cursor_px: (f32, f32),
    frozen: &HashSet<SceneEntityId>,
    slab: Option<SectionSlab>,
) -> Option<PickHit> {
    let cursor = DVec2::new(f64::from(cursor_px.0), f64::from(cursor_px.1));
    let mut best: Option<(f64, f64, PickHit)> = None;
    for record in records {
        // Exclude out-of-slab labels before they can outrank visible geometry on depth.
        if frozen.contains(&record.entity) || slab.is_some_and(|slab| quad_excluded_by_slab(&slab, &record.corners)) {
            continue;
        }
        let [Some(a), Some(b), Some(c), Some(d)] = record.corners.map(|corner| world_to_screen(view_proj, corner, screen)) else {
            continue;
        };
        let world = if let Some(weights) = triangle_weights(cursor, a, b, c) {
            perspective_correct_triangle_point(view_proj, [record.corners[0], record.corners[1], record.corners[2]], weights)
        } else if let Some(weights) = triangle_weights(cursor, a, c, d) {
            perspective_correct_triangle_point(view_proj, [record.corners[0], record.corners[2], record.corners[3]], weights)
        } else {
            continue;
        };
        // Reject a hit point beyond the slab, as for fill triangles.
        if slab.is_some_and(|slab| !slab.contains(world)) {
            continue;
        }
        let center_distance = ((a + b + c + d) * 0.25).distance_squared(cursor);
        let depth = projected_depth(view_proj, world);
        if best
            .as_ref()
            .is_none_or(|(distance, best_depth, _)| screen_hit_is_better(center_distance, depth, *distance, *best_depth))
        {
            best = Some((center_distance, depth, PickHit { entity: record.entity, world }));
        }
    }
    best.map(|(_, _, hit)| hit)
}

/// Find the polyline vertex nearest the cursor. Returns `(object_id, vertex_index, world_pos)`.
/// Only considers unfrozen, visible objects whose layer is visible in `doc`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pick_nearest_vertex(
    doc: &Document,
    hidden: &HashSet<SceneEntityId>,
    frozen: &HashSet<SceneEntityId>,
    view_proj: &DMat4,
    screen: Size,
    cursor_px: (f32, f32),
    threshold_px: f32,
    slab: Option<SectionSlab>,
) -> Option<(ObjectId, ObjectPoint, DVec3)> {
    pick_nearest_vertex_from_indices(
        doc,
        0..doc.objects().len(),
        hidden,
        frozen,
        view_proj,
        screen,
        cursor_px,
        threshold_px,
        VertexPickFilter::AnyEditable,
        slab,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VertexPickFilter {
    /// Point objects plus stored polyline vertices.
    AnyEditable,
    /// Vertices that can be removed without invalidating a polyline.
    DeletablePolyline,
}

/// Spatially accelerated design-vertex pick. The index must have been built
/// from `doc`; callers keep it revision-aligned with the composite scene.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pick_nearest_vertex_indexed(
    doc: &Document,
    index: &ObjectSnapIndex,
    hidden: &HashSet<SceneEntityId>,
    frozen: &HashSet<SceneEntityId>,
    view_proj: &DMat4,
    screen: Size,
    cursor_px: (f32, f32),
    threshold_px: f32,
    filter: VertexPickFilter,
    slab: Option<SectionSlab>,
) -> Option<(ObjectId, ObjectPoint, DVec3)> {
    let cursor = DVec2::new(f64::from(cursor_px.0), f64::from(cursor_px.1));
    let candidates = index.candidates(view_proj, screen, cursor, f64::from(threshold_px));
    pick_nearest_vertex_from_indices(doc, candidates, hidden, frozen, view_proj, screen, cursor_px, threshold_px, filter, slab)
}

#[allow(clippy::too_many_arguments)]
fn pick_nearest_vertex_from_indices(
    doc: &Document,
    object_indices: impl IntoIterator<Item = usize>,
    hidden: &HashSet<SceneEntityId>,
    frozen: &HashSet<SceneEntityId>,
    view_proj: &DMat4,
    screen: Size,
    cursor_px: (f32, f32),
    threshold_px: f32,
    filter: VertexPickFilter,
    slab: Option<SectionSlab>,
) -> Option<(ObjectId, ObjectPoint, DVec3)> {
    let cursor = DVec2::new(f64::from(cursor_px.0), f64::from(cursor_px.1));
    let mut best_dist = threshold_px as f64;
    let mut best: Option<(ObjectId, ObjectPoint, DVec3)> = None;

    for object_index in object_indices {
        let Some(object) = doc.objects().get(object_index) else {
            continue;
        };
        let entity = SceneEntityId::Object(object.id());
        if hidden.contains(&entity) || frozen.contains(&entity) {
            continue;
        }
        if !doc.layer(object.layer()).map(|l| l.loaded).unwrap_or(true) {
            continue;
        }
        match object {
            // A circle's one handle is its centre, for every filter: it has no
            // vertices, so there is nothing a deletable-vertex pick could take.
            Object::Circle { center, .. } if filter == VertexPickFilter::AnyEditable => {
                if let Some(sp) = slab_screen_point(slab, view_proj, screen, *center) {
                    let d = sp.distance(cursor);
                    if d < best_dist {
                        best_dist = d;
                        best = Some((object.id(), ObjectPoint::Center, *center));
                    }
                }
            }
            Object::Polyline { verts, closed, .. } if filter == VertexPickFilter::AnyEditable || verts.len() > if *closed { 3 } else { 2 } => {
                for (i, vert) in verts.iter().enumerate() {
                    if let Some(sp) = slab_screen_point(slab, view_proj, screen, vert.pos) {
                        let d = sp.distance(cursor);
                        if d < best_dist {
                            best_dist = d;
                            best = Some((object.id(), ObjectPoint::Vertex(i), vert.pos));
                        }
                    }
                }
            }
            Object::Point { pos, .. } if filter == VertexPickFilter::AnyEditable => {
                if let Some(sp) = slab_screen_point(slab, view_proj, screen, *pos) {
                    let d = sp.distance(cursor);
                    if d < best_dist {
                        best_dist = d;
                        best = Some((object.id(), ObjectPoint::Vertex(0), *pos));
                    }
                }
            }
            _ => {}
        }
    }

    best
}

pub(crate) fn triangle_weights(point: DVec2, a: DVec2, b: DVec2, c: DVec2) -> Option<DVec3> {
    let denominator = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    let u = ((b.y - c.y) * (point.x - c.x) + (c.x - b.x) * (point.y - c.y)) / denominator;
    let v = ((c.y - a.y) * (point.x - c.x) + (a.x - c.x) * (point.y - c.y)) / denominator;
    let w = 1.0 - u - v;
    (u >= 0.0 && v >= 0.0 && w >= 0.0).then_some(DVec3::new(u, v, w))
}
