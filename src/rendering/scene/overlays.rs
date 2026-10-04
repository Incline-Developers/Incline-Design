//! Overlay scene assembly.

use glam::{DMat4, DVec3};

use crate::{
    Size,
    model::{Document, Object, ObjectPoint, geometry::circle_polyline_vertices},
    rendering::{
        StrokeInstance, Vertex,
        geometry::{DrawContext, draw_line, draw_screen_cross, draw_screen_point_marker, draw_screen_point_marker_sized, tessellate_polyline_stroke},
        graphics::{ACTIVE_POINT_COLOR, DOC_LINE_WIDTH, MEASUREMENT_COLOR, PREVIEW_COLOR},
        pick::world_to_screen,
    },
    ui::state::{ActiveTool, EditorState},
};

pub(crate) struct OverlaySceneBuildInput<'a> {
    pub(crate) editor: &'a EditorState,
    pub(crate) document: &'a Document,
    pub(crate) overlay_strokes: &'a mut Vec<StrokeInstance>,
    pub(crate) view_proj: DMat4,
    pub(crate) screen_size: Size,
    pub(crate) scene_origin: DVec3,
    pub(crate) scale_factor: f32,
}

/// Blast outlines sit under the cut lines that will divide them, so they are
/// drawn a shade heavier than ordinary design geometry to read as a boundary
/// rather than as one more line on the bench.
const BLAST_OUTLINE_WIDTH: f32 = 2.5;
/// Ground taken out of mining: still outlined, so it can be picked and put
/// back, but grey against the live outlines around it.
const EXCLUDED_OUTLINE_COLOR: [f32; 4] = [0.5, 0.5, 0.5, 0.9];

/// The Haulage Layout's selected dig block, and the line to where it joins
/// the roads: the selection orange the rest of the scene uses.
pub(crate) const HAUL_SELECTED_BLOCK: [f32; 4] = [1.0, 0.7, 0.1, 1.0];
/// How a dig block meets the roads, as the Layout tints it: left to the
/// nearest road within reach, held to nodes chosen for it, or out of reach.
/// Blocks that are not dug are only context.
pub(crate) const HAUL_AUTO_BLOCK: [f32; 3] = [0.25, 0.85, 0.4];
pub(crate) const HAUL_MANUAL_BLOCK: [f32; 3] = [0.35, 0.65, 1.0];
pub(crate) const HAUL_FAR_BLOCK: [f32; 3] = [0.95, 0.25, 0.25];
pub(crate) const HAUL_PLACED_BLOCK: [f32; 3] = [0.6, 0.6, 0.6];

/// The tint a Layout block is drawn in, before its alpha.
pub(crate) fn haul_block_tint(block: &crate::ui::state::HaulBlock) -> [f32; 3] {
    if !block.dug {
        HAUL_PLACED_BLOCK
    } else if !block.links.is_empty() {
        HAUL_MANUAL_BLOCK
    } else if block.connected {
        HAUL_AUTO_BLOCK
    } else {
        HAUL_FAR_BLOCK
    }
}

/// Dashes in an automatic join's line, which tell it from a chosen one.
const HAUL_AUTO_DASHES: u32 = 7;

/// How many pieces a leg replacing an existing connector is broken into. Odd,
/// so a dashed run starts and ends on a mark rather than on a gap.
const TIE_OVERWRITE_DASHES: usize = 9;
/// Width the previewed connectors are drawn at, a shade over the solid ties
/// behind them so a leg reads over the one it would replace.
const TIE_PREVIEW_WIDTH: f32 = 2.0;
/// The tie-in a click would confirm, drawn in the colour of the product it
/// would be laid with.
///
/// A leg replacing a connector that is already there is drawn broken instead
/// of solid: overwriting is what a second tie across the same two holes does,
/// and it should be visible before the click rather than after it.
fn draw_tie_preview(overlay: &mut DrawContext<'_>, editor: &EditorState) {
    let color = editor.active_product().map_or(PREVIEW_COLOR, |product| {
        let [red, green, blue, _] = product.color.to_srgba_unmultiplied();
        [f32::from(red) / 255.0, f32::from(green) / 255.0, f32::from(blue) / 255.0, 1.0]
    });
    // Paint the complete snap corridor before its captured connector legs.
    // This is the actual 12px-per-side test used by `tie_chain_between`, so
    // users can see why a nearby collar will or will not be included.
    if let (Some(anchor), Some(end)) = (editor.tie_anchor_world, editor.tie_path_end_world) {
        let corridor = [color[0], color[1], color[2], 0.07];
        let corridor_width = crate::rendering::graphics::projections::TIE_CORRIDOR_PIXELS * 2.0 / overlay.scale_factor.max(f32::EPSILON);
        draw_line(overlay, anchor, end, corridor_width, corridor);
    }
    // The hole the chain is running from, marked so an armed chain is visible
    // with the pointer over nothing to tie it to.
    if let Some(anchor) = editor.tie_anchor_world {
        draw_screen_cross(overlay, anchor, 9.0, 2.0, color);
    }
    for leg in &editor.tie_preview {
        // A leg replacing a connector is broken so the overwrite is visible
        // before it is committed.
        if !leg.overwrite {
            draw_line(overlay, leg.start, leg.end, TIE_PREVIEW_WIDTH, color);
            continue;
        }
        for dash in (0..TIE_OVERWRITE_DASHES).step_by(2) {
            let from = dash as f64 / TIE_OVERWRITE_DASHES as f64;
            let to = (dash + 1) as f64 / TIE_OVERWRITE_DASHES as f64;
            draw_line(overlay, leg.start.lerp(leg.end, from), leg.start.lerp(leg.end, to), TIE_PREVIEW_WIDTH, color);
        }
    }
}

pub(crate) fn rebuild_editor_overlay(input: OverlaySceneBuildInput<'_>) {
    let OverlaySceneBuildInput {
        editor,
        document,
        overlay_strokes,
        view_proj,
        screen_size,
        scene_origin,
        scale_factor,
    } = input;

    overlay_strokes.clear();

    let mut unused_fill_vertices: Vec<Vertex> = Vec::new();
    let mut unused_fill_indices = Vec::new();
    let mut overlay = DrawContext::unstyled(overlay_strokes, &mut unused_fill_vertices, &mut unused_fill_indices, scene_origin, scale_factor);

    // Blast outlines are derived from the bench, not stored objects, so they
    // are drawn here rather than through the scene's document geometry.
    for outline in editor.blasting_outlines.iter().filter(|_| editor.is_blasting_step()) {
        let selected = editor.selected_blast == Some(crate::ui::state::BlastShapeRef::new(outline.solid, outline.bench_base, outline.anchor));
        for ring in &outline.rings {
            let verts: Vec<crate::model::PolyVertex> = ring.iter().map(|point| crate::model::PolyVertex::straight(*point)).collect();
            tessellate_polyline_stroke(
                &mut overlay,
                &verts,
                true,
                if selected { BLAST_OUTLINE_WIDTH * 2.5 } else { BLAST_OUTLINE_WIDTH },
                match (selected, outline.excluded) {
                    (true, _) => [1.0, 0.7, 0.1, 1.0],
                    (false, true) => EXCLUDED_OUTLINE_COLOR,
                    (false, false) => PREVIEW_COLOR,
                },
            );
        }
    }

    // Dig blocks belong to the step that draws them; the Blasting step before
    // it shows its benches undivided.
    if editor.is_dig_strips_step() {
        for outline in &editor.dig_outlines {
            let selected = editor.selected_dig_block == Some(crate::ui::state::BlastShapeRef::new(outline.solid, outline.bench_base, outline.anchor));
            for ring in &outline.rings {
                let verts: Vec<_> = ring.iter().map(|point| crate::model::PolyVertex::straight(*point)).collect();
                tessellate_polyline_stroke(
                    &mut overlay,
                    &verts,
                    true,
                    if selected { 4.0 } else { 1.5 },
                    match (selected, outline.excluded) {
                        (true, _) => [1.0, 0.7, 0.1, 1.0],
                        (false, true) => EXCLUDED_OUTLINE_COLOR,
                        (false, false) => [0.2, 0.9, 0.8, 1.0],
                    },
                );
            }
        }
    }

    let stroke_preview = editor.pending_stroke.clone();
    for pair in stroke_preview.windows(2) {
        draw_line(&mut overlay, pair[0], pair[1], DOC_LINE_WIDTH, PREVIEW_COLOR);
    }

    if editor.is_haulage_page() {
        // Blocks out of reach are tinted already; a cross each would bury them.
        for issue in editor.haul_issues.iter().filter(|issue| issue.blocks.is_empty()) {
            draw_screen_cross(&mut overlay, issue.pos, 9.0, 2.0, [1.0, 0.65, 0.15, 1.0]);
        }
        // Joins are drawn one line per node, from the middle of the blocks
        // using it, rather than one per block: the chosen ones always, solid,
        // so a link is never invisible; the automatic ones only for the
        // selection, dashed, so the two are never mistaken for each other.
        let mut manual: Vec<(DVec3, f64, bool, Vec<DVec3>)> = Vec::new();
        let mut automatic: Vec<(DVec3, Vec<DVec3>)> = Vec::new();
        for block in editor.haul_blocks.iter().filter(|b| !editor.haul_hidden.hides(b.solid, b.bench, b.flitch, b.blast)) {
            let selected = editor.haul_selected_blocks.contains(&block.id);
            let [r, g, b] = haul_block_tint(block);
            let (color, width) = if selected {
                (HAUL_SELECTED_BLOCK, 2.0)
            } else if block.dug {
                ([r, g, b, 0.45], 1.25)
            } else {
                ([r, g, b, 0.3], 1.0)
            };
            for ring in &block.rings {
                let verts: Vec<_> = ring.iter().copied().map(crate::model::PolyVertex::straight).collect();
                tessellate_polyline_stroke(&mut overlay, &verts, true, width, color);
            }
            for &(_, at) in &block.links {
                match manual.iter_mut().find(|group| group.0 == at && group.1 == block.flitch.base && group.2 == selected) {
                    Some(group) => group.3.push(block.point()),
                    None => manual.push((at, block.flitch.base, selected, vec![block.point()])),
                }
            }
            if selected && let Some(join) = block.join {
                match automatic.iter_mut().find(|group| group.0 == join) {
                    Some(group) => group.1.push(block.point()),
                    None => automatic.push((join, vec![block.point()])),
                }
            }
        }
        let middle = |points: &[DVec3]| points.iter().copied().sum::<DVec3>() / points.len().max(1) as f64;
        let [r, g, b] = HAUL_MANUAL_BLOCK;
        for (at, _, selected, points) in &manual {
            let from = middle(points);
            draw_line(&mut overlay, from, *at, if *selected { 3.0 } else { 2.0 }, [r, g, b, 0.9]);
            draw_screen_cross(&mut overlay, from, 6.0, 2.0, [r, g, b, 1.0]);
            draw_screen_cross(&mut overlay, *at, 6.0, 2.0, [r, g, b, 1.0]);
        }
        for (join, points) in &automatic {
            let from = middle(points);
            for step in 0..HAUL_AUTO_DASHES {
                let (start, end) = (
                    f64::from(2 * step) / f64::from(2 * HAUL_AUTO_DASHES - 1),
                    f64::from(2 * step + 1) / f64::from(2 * HAUL_AUTO_DASHES - 1),
                );
                draw_line(&mut overlay, from.lerp(*join, start), from.lerp(*join, end), 2.5, HAUL_SELECTED_BLOCK);
            }
            draw_screen_cross(&mut overlay, from, 6.0, 2.0, HAUL_SELECTED_BLOCK);
        }
        // The nodes picked so far for the selected blocks, and where they
        // would join them.
        let picking: Vec<_> = editor
            .haul_blocks
            .iter()
            .filter(|b| editor.haul_selected_blocks.contains(&b.id))
            .map(crate::ui::state::HaulBlock::point)
            .collect();
        for &at in &editor.haul_link_points {
            if !picking.is_empty() {
                draw_line(&mut overlay, middle(&picking), at, 1.5, PREVIEW_COLOR);
            }
            draw_screen_cross(&mut overlay, at, 9.0, 2.5, PREVIEW_COLOR);
        }
    }
    for pair in editor.haul_points.windows(2) {
        draw_line(&mut overlay, pair[0], pair[1], 3.0, PREVIEW_COLOR);
    }
    for &point in &editor.haul_points {
        draw_screen_cross(&mut overlay, point, 7.0, 2.0, PREVIEW_COLOR);
    }
    if editor.haul_draw
        && let Some(cursor) = editor.haul_cursor
    {
        if let Some(&last) = editor.haul_points.last() {
            draw_line(&mut overlay, last, cursor, 3.0, PREVIEW_COLOR);
        }
        draw_screen_cross(&mut overlay, cursor, 7.0, 2.0, PREVIEW_COLOR);
    }
    // A dragged node or bend point: its roads, redrawn through where it
    // would land.
    if let Some(crate::ui::state::HaulDrag { target, pos: Some(pos), .. }) = editor.haul_drag {
        let network = input.document.haulage();
        for road in &network.roads {
            let mut points = network.points(road);
            let moved = match target {
                crate::ui::state::HaulDragTarget::Node(id) if road.from == id => points.first_mut(),
                crate::ui::state::HaulDragTarget::Node(id) if road.to == id => points.last_mut(),
                crate::ui::state::HaulDragTarget::Shape(id, index) if road.id == id => points.get_mut(index + 1),
                _ => None,
            };
            let Some(point) = moved else { continue };
            *point = pos;
            for pair in points.windows(2) {
                draw_line(&mut overlay, pair[0], pair[1], 3.0, PREVIEW_COLOR);
            }
        }
        draw_screen_cross(&mut overlay, pos, 9.0, 2.0, PREVIEW_COLOR);
    }
    // The route check is the Layout panel's, and draws only beside it.
    if let Some(route) = editor.haul_route.as_ref().filter(|_| editor.is_haulage_page()) {
        // The return first, so the loaded haul draws over it where they share road.
        for (points, color, loaded) in [(&route.empty_path, [1.0, 0.85, 0.1, 1.0], false), (&route.loaded_path, [1.0, 0.2, 0.2, 1.0], true)] {
            for (i, pair) in points.windows(2).enumerate() {
                if route.grade_lengthened && ((loaded && i == 0) || (!loaded && i + 2 == points.len())) {
                    for step in (0..20).step_by(2) {
                        draw_line(
                            &mut overlay,
                            pair[0].lerp(pair[1], f64::from(step) / 20.0),
                            pair[0].lerp(pair[1], f64::from(step + 1) / 20.0),
                            4.0,
                            color,
                        );
                    }
                } else {
                    draw_line(&mut overlay, pair[0], pair[1], 4.0, color);
                }
            }
        }
    }
    draw_tie_preview(&mut overlay, editor);
    if editor.poly_finish_dialog {
        // Dialog is open: draw a dashed closing line from last point to first point.
        // Dash size is fixed in screen pixels so it stays visible at any zoom level.
        if let (Some(&first), Some(&last)) = (stroke_preview.first(), stroke_preview.last()) {
            let total_world = (first - last).length();
            if total_world > 1e-9 {
                let px_per_world = {
                    let s0 = world_to_screen(&view_proj, last, screen_size);
                    let s1 = world_to_screen(&view_proj, first, screen_size);
                    if let (Some(a), Some(b)) = (s0, s1) {
                        let sdx = b.x - a.x;
                        let sdy = b.y - a.y;
                        (sdx * sdx + sdy * sdy).sqrt() / total_world
                    } else {
                        1.0
                    }
                };
                let dash_world = 6.0 / px_per_world;
                let gap_world = 4.0 / px_per_world;
                let step = dash_world + gap_world;
                let dir = (first - last) / total_world;
                let mut t = 0.0f64;
                while t < total_world {
                    let t0 = t;
                    let t1 = (t + dash_world).min(total_world);
                    draw_line(&mut overlay, last + dir * t0, last + dir * t1, DOC_LINE_WIDTH, PREVIEW_COLOR);
                    t += step;
                }
            }
        }
    } else if let (Some(&last), Some(cursor)) = (editor.pending_stroke.last(), editor.cursor_world) {
        draw_line(&mut overlay, last, cursor, DOC_LINE_WIDTH, PREVIEW_COLOR);
    }

    if editor.active_tool == ActiveTool::MakeCircle
        && let Some(draft) = editor.circle_draft.as_ref()
    {
        draw_screen_cross(&mut overlay, draft.center, 7.0, 1.5, PREVIEW_COLOR);
        if let Some(radius) = draft.preview_radius(editor.cursor_world) {
            let bearing = editor.cursor_world.map_or(glam::DVec2::X, |cursor| cursor.truncate() - draft.center.truncate());
            if let Some(verts) = circle_polyline_vertices(draft.center, radius, bearing) {
                draw_line(&mut overlay, draft.center, verts[0].pos, 1.5, PREVIEW_COLOR);
                tessellate_polyline_stroke(&mut overlay, &verts, true, DOC_LINE_WIDTH, PREVIEW_COLOR);
            }
        }
    }

    if editor.active_tool == ActiveTool::VerticalSlice
        && let Some(start) = editor.slice_pending_start
        && let Some(cursor) = editor.cursor_world
    {
        // The slice line is flat in XY; preview it at the start's elevation.
        let end = DVec3::new(cursor.x, cursor.y, start.z);
        draw_line(&mut overlay, start, end, 2.0, PREVIEW_COLOR);
    }

    if editor.active_tool == ActiveTool::MeasureDistance
        && let Some(start) = editor.measurement_start
        && let Some(end) = editor.measurement_end.or(editor.cursor_world)
    {
        draw_line(&mut overlay, start, end, 2.0, MEASUREMENT_COLOR);
    }

    if editor.active_tool == ActiveTool::MeasureBatterAngle {
        let mut points = editor.batter_angle_points.clone();
        if points.len() < 3
            && let Some(cursor) = editor.cursor_world
        {
            points.push(cursor);
        }

        if points.len() >= 2 {
            draw_line(&mut overlay, points[0], points[1], 2.0, MEASUREMENT_COLOR);
        }
        if points.len() >= 3 {
            if let Some(measurement) = crate::ui::state::batter_angle_measurement(points.as_slice()) {
                let baseline = points[1] - points[0];
                let along = (measurement.projection - points[0]).dot(baseline);
                let extension_start = if along < 0.0 {
                    Some(points[0])
                } else if along > baseline.length_squared() {
                    Some(points[1])
                } else {
                    None
                };
                if let Some(start) = extension_start {
                    draw_line(&mut overlay, start, measurement.projection, 1.0, MEASUREMENT_COLOR);
                }
                draw_line(&mut overlay, measurement.projection, points[2], 2.0, MEASUREMENT_COLOR);
            } else {
                draw_line(&mut overlay, points[1], points[2], 2.0, MEASUREMENT_COLOR);
            }
        }
    }

    if matches!(editor.active_tool, ActiveTool::Move | ActiveTool::DeletePoints)
        && let Some(hover) = editor.tool_hover_vertex_world
    {
        draw_screen_point_marker_sized(&mut overlay, hover, 11.0, ACTIVE_POINT_COLOR);
    }

    let marker_target_id = editor.move_vertex_target.map(|(id, _)| id);
    if let Some(target_id) = marker_target_id
        && let Some(obj) = document.get_object(target_id)
    {
        match (obj, editor.move_vertex_target.map(|(_, point)| point)) {
            (Object::Circle { center, .. }, Some(ObjectPoint::Center)) => {
                draw_screen_point_marker_sized(&mut overlay, *center, 11.0, ACTIVE_POINT_COLOR);
            }
            (Object::Polyline { verts, .. }, Some(ObjectPoint::Vertex(selected_index))) => {
                for (index, v) in verts.iter().enumerate() {
                    let selected = index == selected_index;
                    draw_screen_point_marker_sized(
                        &mut overlay,
                        v.pos,
                        if selected { 11.0 } else { 9.0 },
                        if selected { ACTIVE_POINT_COLOR } else { crate::ui::SELECTION_COLOR_F32 },
                    );
                }
            }
            (Object::Point { pos, .. }, Some(ObjectPoint::Vertex(0))) => {
                draw_screen_point_marker_sized(&mut overlay, *pos, 11.0, ACTIVE_POINT_COLOR);
            }
            _ => {}
        }
    }

    if !editor.fuse_segments.is_empty() || editor.fuse_awaiting_endpoint.is_some() {
        let mut chain_pts: Vec<DVec3> = Vec::new();
        for seg in &editor.fuse_segments {
            if let Some(Object::Polyline { verts, .. }) = document.get_object(seg.object_id) {
                let mut ordered: Vec<DVec3> = if seg.closed {
                    (0..verts.len()).map(|offset| verts[(seg.start_index + offset) % verts.len()].pos).collect()
                } else if seg.reversed {
                    verts.iter().rev().map(|v| v.pos).collect()
                } else {
                    verts.iter().map(|v| v.pos).collect()
                };
                if chain_pts.last().zip(ordered.first()).is_some_and(|(a, b)| (*a - *b).length_squared() <= 1.0e-16) {
                    ordered.remove(0);
                }
                chain_pts.extend(ordered);
            }
        }
        for pair in chain_pts.windows(2) {
            draw_line(&mut overlay, pair[0], pair[1], DOC_LINE_WIDTH, PREVIEW_COLOR);
        }

        if let Some(tail) = editor.fuse_chain_tail {
            draw_screen_point_marker_sized(&mut overlay, tail, 11.0, crate::ui::SELECTION_COLOR_F32);
        }

        if editor.fuse_awaiting_endpoint.is_none()
            && let Some(endpoint) = editor.fuse_close_marker
        {
            draw_screen_point_marker(&mut overlay, endpoint, ACTIVE_POINT_COLOR);
        }

        if editor.fuse_awaiting_endpoint.is_some() {
            for &(_, marker) in &editor.fuse_endpoint_markers {
                let is_tail = editor.fuse_chain_tail.is_some_and(|tail| (tail - marker).length_squared() < 1e-10);
                if !is_tail {
                    draw_screen_point_marker(&mut overlay, marker, ACTIVE_POINT_COLOR);
                }
            }
        }
    }
}
