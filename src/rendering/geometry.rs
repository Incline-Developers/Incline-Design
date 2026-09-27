//! CPU scene tessellation primitives. Domain geometry remains double precision;
//! vertices are rebased to a scene origin only at the GPU boundary.

use glam::DVec3;

use crate::{
    model::{PolyVertex, geometry::tessellate_bulge_segment},
    rendering::{
        StrokeInstance, Vertex,
        scene::document_style::{STROKE_ROUND, STROKE_SCREEN_AXIS, STYLE_SLOT_NONE},
    },
};

pub(crate) struct DrawContext<'a> {
    pub(crate) strokes: &'a mut Vec<StrokeInstance>,
    pub(crate) fill_vertex_buf: &'a mut Vec<Vertex>,
    pub(crate) fill_index_buf: &'a mut Vec<u32>,
    pub(crate) scene_origin: DVec3,
    pub(crate) scale_factor: f32,
    /// Style slot stamped on everything drawn; see `scene::document_style`.
    pub(crate) style: u32,
}

impl<'a> DrawContext<'a> {
    /// A context for geometry the editor never restyles.
    pub(crate) fn unstyled(
        strokes: &'a mut Vec<StrokeInstance>,
        fill_vertex_buf: &'a mut Vec<Vertex>,
        fill_index_buf: &'a mut Vec<u32>,
        scene_origin: DVec3,
        scale_factor: f32,
    ) -> Self {
        Self {
            strokes,
            fill_vertex_buf,
            fill_index_buf,
            scene_origin,
            scale_factor,
            style: STYLE_SLOT_NONE,
        }
    }

    fn push(&mut self, start: [f32; 3], end: [f32; 3], half_width_px: f32, flags: u32, color: [f32; 4]) {
        self.strokes.push(StrokeInstance {
            start,
            half_width_px,
            end,
            style: self.style | flags,
            color,
        });
    }
}

/// Cosine of the turn angle below which a round join is visually redundant:
/// the wedge gap between adjacent stroke quads is `half_width * tan(angle/2)`,
/// sub-pixel at document line widths for turns under ~11 degrees. Densely
/// sampled polylines (contours, imported strings) are almost entirely such
/// turns, and skipping their joins keeps translucent strings from darkening
/// at every vertex.
const JOIN_COLLINEAR_COS: f64 = 0.98;

/// Whether the turn from direction `incoming` to `outgoing` is sharp enough
/// that the joint between their stroke quads needs a round join to cover it.
pub(crate) fn needs_round_join(incoming: DVec3, outgoing: DVec3) -> bool {
    let scale = incoming.length_squared() * outgoing.length_squared();
    if scale <= f64::EPSILON {
        return false;
    }
    incoming.dot(outgoing) < JOIN_COLLINEAR_COS * scale.sqrt()
}

fn local(point: DVec3, origin: DVec3) -> [f32; 3] {
    let p = point - origin;
    [p.x as f32, p.y as f32, p.z as f32]
}

pub(crate) fn draw_line(ctx: &mut DrawContext, start: DVec3, end: DVec3, line_width: f32, color: [f32; 4]) {
    if (end - start).length_squared() <= f64::EPSILON {
        return;
    }
    let half = (line_width * ctx.scale_factor).max(1.0) * 0.5;
    let (start, end) = (local(start, ctx.scene_origin), local(end, ctx.scene_origin));
    ctx.push(start, end, half, 0, color);
}

/// Draw a filled circle (sphere indicator) in screen space at the given world position.
/// `radius_px` is in device pixels.
pub(crate) fn draw_screen_sphere(ctx: &mut DrawContext, center: DVec3, radius_px: f32, color: [f32; 4]) {
    let position = local(center, ctx.scene_origin);
    ctx.push(position, position, radius_px, STROKE_ROUND, color);
}

/// Snap marker matching the design-vertex markers drawn by `design_point.wgsl`:
/// same pixel footprint and dark outline, round instead of square and filled
/// with `color` so a snapped vertex reads as highlighted rather than covered.
pub(crate) fn draw_screen_point_marker(ctx: &mut DrawContext, center: DVec3, color: [f32; 4]) {
    draw_screen_point_marker_sized(ctx, center, 10.0, color);
}

/// The same outlined marker at an explicit outer diameter in logical pixels, so
/// overlays that need a size hierarchy (an active endpoint against its
/// candidates, a selected vertex against its siblings) still read as one family.
pub(crate) fn draw_screen_point_marker_sized(ctx: &mut DrawContext, center: DVec3, outer_px: f32, color: [f32; 4]) {
    let inner_px = (outer_px - 3.0).max(1.0);
    draw_screen_sphere(ctx, center, outer_px * 0.5 * ctx.scale_factor, crate::rendering::graphics::POINT_MARKER_COLOR);
    draw_screen_sphere(ctx, center, inner_px * 0.5 * ctx.scale_factor, color);
}

pub(crate) fn draw_screen_cross(ctx: &mut DrawContext, center: DVec3, half_size_px: f32, line_width: f32, color: [f32; 4]) {
    // Both arms use pixel offsets from a single world anchor. Projecting world
    // X/Y directions makes the cross skew or collapse at shallow camera dips.
    let position = local(center, ctx.scene_origin);
    let half_size = half_size_px * ctx.scale_factor;
    let half_width = (line_width * ctx.scale_factor).max(1.0) * 0.5;
    ctx.push(position, [half_size, 0.0, 0.0], half_width, STROKE_SCREEN_AXIS, color);
    ctx.push(position, [0.0, half_size, 0.0], half_width, STROKE_SCREEN_AXIS, color);
}

/// Add a camera-independent round join in pixel space at a world position.
pub(crate) fn draw_round_join(ctx: &mut DrawContext, center: DVec3, line_width: f32, color: [f32; 4]) {
    let radius = (line_width * ctx.scale_factor).max(1.0) * 0.5;
    let position = local(center, ctx.scene_origin);
    ctx.push(position, position, radius, STROKE_ROUND, color);
}

/// Tessellate a polyline's stroke (segments, arcs and the round joins between
/// them) into the context's stroke buffers. Shared by the per-rebuild scene
/// builder and the static stroke chunk cache so both produce identical
/// geometry.
pub(crate) fn tessellate_polyline_stroke(ctx: &mut DrawContext, verts: &[PolyVertex], closed: bool, line_weight: f32, line_rgba: [f32; 4]) {
    for segment in verts.windows(2) {
        draw_bulge_segment(ctx, segment[0].pos, segment[1].pos, segment[0].bulge, line_weight, line_rgba);
    }
    if closed
        && verts.len() >= 2
        && let (Some(first), Some(last)) = (verts.first(), verts.last())
    {
        draw_bulge_segment(ctx, last.pos, first.pos, last.bulge, line_weight, line_rgba);
    }
    let n = verts.len();
    let joint_indices = if closed && n >= 2 {
        0..n
    } else if n > 2 {
        1..n - 1
    } else {
        0..0
    };
    for i in joint_indices {
        let prev = &verts[(i + n - 1) % n];
        let cur = &verts[i];
        let next = &verts[(i + 1) % n];
        // Bulged segments meet the joint at the arc tangent,
        // not the chord direction, so always keep their joins.
        let keep = prev.bulge.abs() > f64::EPSILON || cur.bulge.abs() > f64::EPSILON || needs_round_join(cur.pos - prev.pos, next.pos - cur.pos);
        if keep {
            draw_round_join(ctx, cur.pos, line_weight, line_rgba);
        }
    }
}

pub(crate) fn draw_bulge_segment(ctx: &mut DrawContext, start: DVec3, end: DVec3, bulge: f64, line_width: f32, color: [f32; 4]) {
    let points = tessellate_bulge_segment(start, end, bulge);
    let segments = points.len().saturating_sub(1);
    if segments == 0 {
        return;
    }
    // The turn between consecutive arc segments is theta/segments; when it is
    // near-collinear the interior joins are invisible and can be skipped.
    let theta = 4.0 * bulge.atan();
    let interior_joins = bulge.is_finite() && bulge.abs() > f64::EPSILON && (theta / segments as f64).cos() < JOIN_COLLINEAR_COS;
    for (index, pair) in points.windows(2).enumerate() {
        draw_line(ctx, pair[0], pair[1], line_width, color);
        if index + 1 < segments && interior_joins {
            draw_round_join(ctx, pair[1], line_width, color);
        }
    }
}
