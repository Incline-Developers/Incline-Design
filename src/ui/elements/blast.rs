//! Drill & Blast's reviews drawn over the viewport: lines of equal firing
//! time, the burden relief legend, the card for the hole under the pointer,
//! and the blast timeline.
//!
//! All of it is egui geometry over the scene, like the initiation cards: it
//! is read, not picked, and must never be hidden behind a surface. The
//! collars themselves are painted by the renderer - see
//! `EditorState::blast_collar_paint` - so the heatmap and the timeline's
//! fired state sit in the scene where the holes are.

use crate::{
    i18n::{tr, tr_format},
    model::{
        blast::{BlastAnalysis, ChargeDeck, DeckKind, Primer, ReliefBand, VIBRATION_WINDOW_MS},
        drill_hole::OpenDrillHoleDataset,
    },
    ui::{
        EditorState,
        state::Workspace,
        widgets::{menu, toolbar::GROUP_CORNER_RADIUS},
    },
};

/// Early and late ends of the contour ramp: amber through to violet, so the
/// direction the round runs in reads off the colours alone.
/// Opacity of the heatmap's colour field over the scene.
const RELIEF_SURFACE_ALPHA: u8 = 150;
const LEGEND_WIDTH: f32 = 260.0;
/// Width of each limit's field under the legend bar.
const LEGEND_FIELD_WIDTH: f32 = 74.0;
/// Gap between a floating tile and the canvas edge, and between tiles.
const TILE_MARGIN: f32 = 12.0;
/// Playback rates offered by the timeline, as firing ms per real ms.
const TIMELINE_SPEEDS: [f64; 5] = [0.02, 0.05, 0.1, 0.25, 1.0];
const TIMELINE_STRIP_HEIGHT: f32 = 44.0;
/// The surface signal's lane above the detonations, and the gap between.
const TIMELINE_LANE_HEIGHT: f32 = 9.0;
const TIMELINE_LANE_GAP: f32 = 3.0;
/// Room under the strip for its time axis.
const TIMELINE_AXIS_HEIGHT: f32 = 15.0;
const TIMELINE_MAX_WIDTH: f32 = 760.0;
/// The colour a detonation is drawn in on the timeline: the firing-collar
/// yellow the renderer uses, so strip and scene read as one.
const TIMELINE_BAR: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xCC, 0x2E);
const TIMELINE_PEAK: egui::Color32 = egui::Color32::from_rgb(0xDE, 0x33, 0x38);
/// A peak held under the site's limit.
const TIMELINE_WITHIN: egui::Color32 = egui::Color32::from_rgb(0x4C, 0xBB, 0x6A);
/// The surface signal: a lit fuse running along each connector, burnt once
/// it has passed.
const FUSE_LIT: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xC2, 0x4A);
const FUSE_SPARK: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xF6, 0xD8);
const FUSE_BURNT: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x9C, 0x4A, 0x16, 0xB0);
/// A hole whose downline is lit and is waiting out its downhole delay.
const HOLE_LIT: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xA8, 0x3A);
/// A detonation: a white-hot core in an expanding ring, then an ember.
const BURST_CORE: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xF4, 0xB8);
const BURST_RING: egui::Color32 = egui::Color32::from_rgb(0xFF, 0x7A, 0x1A);
const EMBER_FILL: egui::Color32 = egui::Color32::from_rgb(0x6E, 0x1A, 0x10);
const EMBER_EDGE: egui::Color32 = egui::Color32::from_rgb(0xC2, 0x40, 0x1F);
/// How long a burst stays bright, in real milliseconds whatever the playback
/// speed, so a detonation reads the same at every rate.
const BURST_REAL_MS: f64 = 350.0;
/// Smallest and largest collar mark the timeline draws, in points: a pattern
/// seen whole still shows every hole going off.
const MARK_MIN_POINTS: f32 = 3.5;
const MARK_MAX_POINTS: f32 = 14.0;

fn color32(color: [f32; 3]) -> egui::Color32 {
    egui::Color32::from_rgb((color[0] * 255.0) as u8, (color[1] * 255.0) as u8, (color[2] * 255.0) as u8)
}

fn tile_frame(visuals: &egui::Visuals) -> egui::Frame {
    egui::Frame::new()
        .fill(menu::menu_surface(visuals))
        .stroke(menu::menu_border(visuals))
        .corner_radius(egui::CornerRadius::same(GROUP_CORNER_RADIUS))
        .inner_margin(egui::Margin::symmetric(10, 8))
}

/// Draw every blast review that is showing over the canvas.
pub(crate) fn draw_blast_overlays(ui: &mut egui::Ui, editor: &mut EditorState, drill_holes: &[OpenDrillHoleDataset], canvas_rect: egui::Rect) {
    if editor.active_workspace != Workspace::DrillAndBlast {
        return;
    }
    let analysis = editor.blast_analysis.clone();
    // The field under the lines: heatmap first, contours over it.
    if let Some(analysis) = analysis.as_deref().filter(|_| editor.blast_review.relief) {
        draw_relief_surface(ui, editor, analysis, canvas_rect);
    }
    draw_contours(ui, editor, canvas_rect);
    let active_visible = editor
        .active_drill_hole
        .is_some_and(|id| drill_holes.iter().any(|dataset| dataset.id == id && dataset.state.loaded));
    if let Some(analysis) = analysis.as_deref().filter(|_| active_visible) {
        if editor.blast_review.timeline {
            if let Some(dataset) = drill_holes.iter().find(|dataset| Some(dataset.id) == editor.active_drill_hole) {
                draw_timeline_scene(ui, editor, analysis, dataset, canvas_rect);
            }
            draw_timeline(ui, editor, analysis, canvas_rect);
        }
        if editor.blast_review.relief {
            draw_relief_legend(ui, editor, analysis, canvas_rect);
        }
    }
    draw_hole_card(ui, editor, analysis.as_deref(), drill_holes, canvas_rect);
}

/// Contour colours from the round's first line to its last: plasma, yellow
/// through orange and magenta to violet, so which way the round runs reads
/// off the colours alone and every stop holds up on the grey ground.
const CONTOUR_RAMP: [egui::Color32; 5] = [
    egui::Color32::from_rgb(0xF0, 0xF9, 0x21),
    egui::Color32::from_rgb(0xFC, 0xA6, 0x36),
    egui::Color32::from_rgb(0xE1, 0x64, 0x62),
    egui::Color32::from_rgb(0xB1, 0x2A, 0x90),
    egui::Color32::from_rgb(0x6A, 0x00, 0xA8),
];
/// Room left along a major line between one label and the next.
const CONTOUR_LABEL_SPACING_PX: f32 = 280.0;

fn contour_color(t: f32) -> egui::Color32 {
    let scaled = t.clamp(0.0, 1.0) * (CONTOUR_RAMP.len() - 1) as f32;
    let index = (scaled.floor() as usize).min(CONTOUR_RAMP.len() - 2);
    CONTOUR_RAMP[index].lerp_to_gamma(CONTOUR_RAMP[index + 1], scaled - index as f32)
}

/// Lines of equal firing time, the way a topographic map draws height: a
/// faint minor line between heavier majors, and the majors labelled along
/// their length with the label turned to run with the line.
fn draw_contours(ui: &egui::Ui, editor: &EditorState, canvas_rect: egui::Rect) {
    if editor.blast_contours_px.is_empty() {
        return;
    }
    let pixels_per_point = ui.ctx().pixels_per_point();
    let painter = ui.painter().with_clip_rect(canvas_rect);
    let latest = editor.blast_contours_px.iter().map(|contour| contour.time_ms).fold(0.0, f64::max).max(1.0);
    let font = egui::FontId::proportional(11.0);
    let halo = menu::menu_surface(ui.visuals()).gamma_multiply(0.9);
    let mut labels: Vec<[egui::Pos2; 4]> = Vec::new();
    let to_pos = |point: (f32, f32)| egui::pos2(point.0 / pixels_per_point, point.1 / pixels_per_point);

    // Minors first so the majors draw over them where they run close.
    let mut order: Vec<&crate::ui::state::ProjectedContour> = editor.blast_contours_px.iter().collect();
    order.sort_by_key(|contour| contour.major);
    for contour in order {
        let color = contour_color((contour.time_ms / latest) as f32);
        let stroke = if contour.major {
            egui::Stroke::new(2.2, color)
        } else {
            egui::Stroke::new(1.0, color.gamma_multiply(0.6))
        };
        // Clipped points split a contour into separately drawn pieces.
        let closing = contour.closed.then(|| contour.points.first().copied().flatten()).flatten();
        let mut pieces: Vec<Vec<egui::Pos2>> = vec![Vec::new()];
        for point in contour.points.iter().copied().chain(closing.map(Some)) {
            match point {
                Some(point) => pieces.last_mut().expect("never empty").push(to_pos(point)),
                None => pieces.push(Vec::new()),
            }
        }
        for piece in pieces.iter().filter(|piece| piece.len() >= 2) {
            painter.add(egui::Shape::line(piece.clone(), stroke));
        }
        if !contour.major {
            continue;
        }
        let text = format!("{:.0}", contour.time_ms);
        for piece in pieces.iter().filter(|piece| piece.len() >= 2) {
            let length: f32 = piece.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
            let count = (length / CONTOUR_LABEL_SPACING_PX).floor() as usize;
            for slot in 0..count.max(usize::from(length > CONTOUR_LABEL_SPACING_PX * 0.4)) {
                let at = if count == 0 { length * 0.5 } else { (slot as f32 + 0.5) * length / count as f32 };
                let Some((anchor, direction)) = point_along(piece, at) else {
                    continue;
                };
                // Turned to the line, but never upside down.
                let mut angle = direction.y.atan2(direction.x);
                if angle > std::f32::consts::FRAC_PI_2 {
                    angle -= std::f32::consts::PI;
                } else if angle < -std::f32::consts::FRAC_PI_2 {
                    angle += std::f32::consts::PI;
                }
                let galley = painter.layout_no_wrap(text.clone(), font.clone(), color);
                let half = galley.size() * 0.5 + egui::vec2(4.0, 1.0);
                let rotation = egui::emath::Rot2::from_angle(angle);
                let corners = [
                    egui::vec2(-half.x, -half.y),
                    egui::vec2(half.x, -half.y),
                    egui::vec2(half.x, half.y),
                    egui::vec2(-half.x, half.y),
                ]
                .map(|corner| anchor + rotation * corner);
                let bounds = egui::Rect::from_points(&corners);
                if !canvas_rect.contains_rect(bounds) || labels.iter().any(|other| egui::Rect::from_points(other).expand(6.0).intersects(bounds)) {
                    continue;
                }
                painter.add(egui::Shape::convex_polygon(corners.to_vec(), halo, egui::Stroke::new(1.0, color.gamma_multiply(0.6))));
                let origin = anchor - rotation * (galley.size() * 0.5);
                painter.add(egui::epaint::TextShape::new(origin, galley, color).with_angle(angle));
                labels.push(corners);
            }
        }
    }
}

/// The point `distance` along a polyline, and the direction it runs there.
fn point_along(points: &[egui::Pos2], distance: f32) -> Option<(egui::Pos2, egui::Vec2)> {
    let mut walked = 0.0;
    for pair in points.windows(2) {
        let step = pair[0].distance(pair[1]);
        if step > 0.0 && walked + step >= distance {
            let t = (distance - walked) / step;
            return Some((pair[0].lerp(pair[1], t), (pair[1] - pair[0]) / step));
        }
        walked += step;
    }
    None
}

/// The relief heatmap: each hole's relief laid over the triangulated
/// pattern and blended across every triangle, so the round reads as a field
/// of hot and cold ground rather than a scatter of coloured dots.
///
/// A hole that fires first has no relief of its own; it takes the mean of
/// its triangle's other corners, so the ground around an initiation point is
/// coloured by the holes about it rather than left as a hole in the map.
fn draw_relief_surface(ui: &egui::Ui, editor: &EditorState, analysis: &BlastAnalysis, canvas_rect: egui::Rect) {
    let collars = &editor.blast_collars_px;
    if collars.len() != analysis.times.len() {
        return;
    }
    let limits = editor.blast_review.limits;
    let pixels_per_point = ui.ctx().pixels_per_point();
    let mut mesh = egui::Mesh::default();
    for triangle in &analysis.triangles {
        let Some(corners) = triangle
            .iter()
            .map(|hole| collars[*hole].map(|point| egui::pos2(point.0 / pixels_per_point, point.1 / pixels_per_point)))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        if triangle.iter().any(|hole| analysis.times[*hole].is_none()) {
            continue;
        }
        let values = triangle.map(|hole| analysis.relief[hole]);
        let known: Vec<f64> = values.iter().flatten().copied().collect();
        if known.is_empty() {
            continue;
        }
        let fallback = known.iter().sum::<f64>() / known.len() as f64;
        let base = mesh.vertices.len() as u32;
        for (corner, value) in corners.iter().zip(values) {
            let [red, green, blue] = crate::model::blast::relief_color(value.unwrap_or(fallback), limits);
            let color = egui::Color32::from_rgba_unmultiplied((red * 255.0) as u8, (green * 255.0) as u8, (blue * 255.0) as u8, RELIEF_SURFACE_ALPHA);
            mesh.colored_vertex(*corner, color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    let painter = ui.painter().with_clip_rect(canvas_rect);
    painter.add(egui::Shape::mesh(mesh));
    // The collars over the colour, small and plain, so the holes still read
    // as points on the field.
    let dot = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200);
    for point in collars.iter().flatten() {
        painter.circle(
            egui::pos2(point.0 / pixels_per_point, point.1 / pixels_per_point),
            1.8,
            dot,
            egui::Stroke::new(0.6, egui::Color32::from_black_alpha(140)),
        );
    }
}

/// The round drawn over the pattern at the playhead: the surface signal
/// burning along each connector, holes whose downline it has lit, and each
/// detonation as a burst that cools to an ember.
///
/// Drawn in window space at a floor size rather than left to the collars'
/// fills, which shrink to a few pixels with the pattern in view - exactly
/// when the round is most worth watching.
fn draw_timeline_scene(ui: &egui::Ui, editor: &EditorState, analysis: &BlastAnalysis, dataset: &OpenDrillHoleDataset, canvas_rect: egui::Rect) {
    let collars = &editor.blast_collars_px;
    if collars.len() != analysis.times.len() {
        return;
    }
    let pixels_per_point = ui.ctx().pixels_per_point();
    let painter = ui.painter().with_clip_rect(canvas_rect);
    let at = |hole: usize| collars[hole].map(|point| egui::pos2(point.0 / pixels_per_point, point.1 / pixels_per_point));
    let playhead = editor.blast_review.playhead_ms;
    let marker_world = dataset
        .dataset
        .holes
        .first()
        .map_or(0.0, |hole| hole.render_radius() * crate::model::drill_hole::COLLAR_MARKER_RADIUS_SCALE) as f32;
    let radius = (marker_world * editor.blast_px_per_world / pixels_per_point).clamp(MARK_MIN_POINTS, MARK_MAX_POINTS);
    let fuse_width = (radius * 0.45).clamp(1.5, 4.0);

    for path in &analysis.signal_paths {
        if playhead <= path.leaves_ms {
            continue;
        }
        let (Some(from), Some(to)) = (at(path.from), at(path.to)) else {
            continue;
        };
        let span = (path.arrives_ms - path.leaves_ms).max(1.0e-6);
        let progress = ((playhead - path.leaves_ms) / span).min(1.0) as f32;
        if progress >= 1.0 {
            painter.line_segment([from, to], egui::Stroke::new(fuse_width * 0.7, FUSE_BURNT));
        } else {
            let tip = from.lerp(to, progress);
            painter.line_segment([from, tip], egui::Stroke::new(fuse_width, FUSE_LIT));
            painter.circle_filled(tip, fuse_width * 1.2, FUSE_SPARK);
        }
    }

    let burst_ms = (BURST_REAL_MS * editor.blast_review.speed).max(VIBRATION_WINDOW_MS);
    for hole in 0..collars.len() {
        let Some(centre) = at(hole) else {
            continue;
        };
        // Only a charge goes off. An empty hole has no downline to light and
        // nothing to detonate - even in a pattern not yet loaded, where the
        // other reviews read every hole as firing - so the fuse runs past it.
        if !analysis.is_loaded(hole) {
            continue;
        }
        let lit = analysis.surface_times[hole].is_some_and(|time| time <= playhead);
        match analysis.times[hole] {
            Some(time) if time <= playhead => {
                let age = ((playhead - time) / burst_ms) as f32;
                if age < 1.0 {
                    let ring = BURST_RING.gamma_multiply(1.0 - age);
                    painter.circle_stroke(centre, radius * (1.4 + 2.6 * age), egui::Stroke::new(2.0, ring));
                    painter.circle_filled(centre, radius * (1.5 - 0.5 * age), BURST_CORE.lerp_to_gamma(BURST_RING, age));
                } else {
                    painter.circle(centre, radius, EMBER_FILL, egui::Stroke::new(1.0, EMBER_EDGE));
                }
            }
            _ if lit => {
                painter.circle_stroke(centre, radius * 1.15, egui::Stroke::new(1.5, HOLE_LIT));
            }
            _ => {}
        }
    }
}

/// The key to the relief heatmap: the colour ramp as a bar with its scale
/// at the ends, each limit's value set directly under its mark, and how many
/// holes fall in each band as chips in the band's colour. The limits are a
/// judgement about this rock, and the field recolours as they are dragged.
fn draw_relief_legend(ui: &egui::Ui, editor: &mut EditorState, analysis: &BlastAnalysis, canvas_rect: egui::Rect) {
    let limits = &mut editor.blast_review.limits;
    let [tight, good, slack] = analysis.band_counts(*limits);
    let free = (0..analysis.times.len()).filter(|hole| analysis.band(*hole, *limits) == ReliefBand::Free).count();
    egui::Area::new(egui::Id::new("relief_legend"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(canvas_rect.left_bottom() + egui::vec2(TILE_MARGIN, -TILE_MARGIN))
        .show(ui.ctx(), |ui| {
            tile_frame(ui.visuals()).show(ui, |ui| {
                ui.set_width(LEGEND_WIDTH);
                let weak = ui.visuals().weak_text_color();
                ui.label(egui::RichText::new(tr!(literal = "Burden relief")).strong());
                ui.label(egui::RichText::new(tr!(literal = "ms per metre to the last neighbour to fire")).small().color(weak));
                ui.add_space(4.0);

                let ramp = crate::model::blast::relief_ramp(*limits);
                let top = ramp[ramp.len() - 1].0.max(1.0e-6);
                let scale_font = egui::FontId::proportional(10.0);
                // The scale's ends over the bar.
                let (ends, _) = ui.allocate_exact_size(egui::vec2(LEGEND_WIDTH, 12.0), egui::Sense::hover());
                ui.painter().text(ends.left_bottom(), egui::Align2::LEFT_BOTTOM, "0", scale_font.clone(), weak);
                ui.painter().text(ends.right_bottom(), egui::Align2::RIGHT_BOTTOM, format!("{top:.0}+"), scale_font, weak);
                ui.add_space(2.0);

                let (bar, _) = ui.allocate_exact_size(egui::vec2(LEGEND_WIDTH, 12.0), egui::Sense::hover());
                let x = |value: f64| bar.left() + (value / top).clamp(0.0, 1.0) as f32 * bar.width();
                let mut mesh = egui::Mesh::default();
                for pair in ramp.windows(2) {
                    let ((from_value, from), (to_value, to)) = (pair[0], pair[1]);
                    let base = mesh.vertices.len() as u32;
                    for (value, color) in [(from_value, from), (to_value, to)] {
                        mesh.colored_vertex(egui::pos2(x(value), bar.top()), color32(color));
                        mesh.colored_vertex(egui::pos2(x(value), bar.bottom()), color32(color));
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                    mesh.add_triangle(base + 1, base + 2, base + 3);
                }
                ui.painter().add(egui::Shape::mesh(mesh));
                let tick = egui::Stroke::new(1.5, ui.visuals().strong_text_color());
                for value in [limits.low, limits.high] {
                    ui.painter()
                        .line_segment([egui::pos2(x(value), bar.top() - 2.0), egui::pos2(x(value), bar.bottom() + 4.0)], tick);
                }

                // Each limit's field centred under its mark, kept inside the
                // bar and clear of the other.
                let (row, _) = ui.allocate_exact_size(egui::vec2(LEGEND_WIDTH, 22.0), egui::Sense::hover());
                let field = egui::vec2(LEGEND_FIELD_WIDTH, 20.0);
                let low_left = (x(limits.low) - field.x * 0.5).clamp(bar.left(), bar.right() - 2.0 * field.x - 4.0);
                let high_left = (x(limits.high) - field.x * 0.5).clamp(low_left + field.x + 4.0, bar.right() - field.x);
                let low_rect = egui::Rect::from_min_size(egui::pos2(low_left, row.top() + 2.0), field);
                let high_rect = egui::Rect::from_min_size(egui::pos2(high_left, row.top() + 2.0), field);
                let high = limits.high;
                ui.put(low_rect, egui::DragValue::new(&mut limits.low).range(0.1..=high).speed(0.1).max_decimals(1).suffix(" ms/m"))
                    .on_hover_text(tr!(literal = "Below this a hole fires before the rock in front of it has moved: tight."));
                let low = limits.low;
                ui.put(
                    high_rect,
                    egui::DragValue::new(&mut limits.high).range(low..=1_000.0).speed(0.1).max_decimals(1).suffix(" ms/m"),
                )
                .on_hover_text(tr!(literal = "Above this the rock in front has long gone: slack, with cut-off and flyrock risk."));
                ui.add_space(6.0);

                // Counts as chips in each band's colour.
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let chip = |ui: &mut egui::Ui, color: egui::Color32, count: usize, label: String| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let (dot, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot.center(), 4.5, color);
                            ui.label(egui::RichText::new(count.to_string()).strong());
                            ui.label(egui::RichText::new(label).small().color(weak));
                        });
                    };
                    let band_color = |value: f64| color32(crate::model::blast::relief_color(value, *limits));
                    chip(ui, band_color(limits.low * 0.5), tight, tr!(literal = "tight"));
                    chip(ui, band_color((limits.low + limits.high) * 0.5), good, tr!(literal = "good"));
                    chip(ui, band_color(limits.high * 1.4), slack, tr!(literal = "slack"));
                    chip(ui, egui::Color32::WHITE, free, tr!(literal = "free face"));
                });
            });
        });
}

/// The card beside the hole under the pointer: what it is, when it goes,
/// what relieves it, and its loaded column.
fn draw_hole_card(ui: &egui::Ui, editor: &EditorState, analysis: Option<&BlastAnalysis>, drill_holes: &[OpenDrillHoleDataset], canvas_rect: egui::Rect) {
    let Some(hover) = editor.blast_hover else {
        return;
    };
    let Some(dataset) = drill_holes.iter().find(|dataset| dataset.id == hover.hole.dataset) else {
        return;
    };
    let Some(hole) = dataset.dataset.holes.get(hover.hole.hole) else {
        return;
    };
    // The analysis is of the active dataset; another one's hole gets no timing.
    let analysis = analysis.filter(|analysis| editor.active_drill_hole == Some(dataset.id) && analysis.times.len() == dataset.dataset.holes.len());
    let charge = dataset.dataset.charges.get(&hover.hole.hole);
    let (top, bottom) = match (hole.trace.first(), hole.trace.last()) {
        (Some(first), Some(last)) => (first.depth, last.depth),
        _ => (0.0, 0.0),
    };
    let pixels_per_point = ui.ctx().pixels_per_point();
    let collar = egui::pos2(hover.screen_px.0 / pixels_per_point, hover.screen_px.1 / pixels_per_point);
    // Kept clear of the pointer and flipped back inside the canvas near its edges.
    let card_size = egui::vec2(250.0, if charge.is_some() { 230.0 } else { 120.0 });
    let mut pos = collar + egui::vec2(18.0, 18.0);
    if pos.x + card_size.x > canvas_rect.right() - TILE_MARGIN {
        pos.x = collar.x - 18.0 - card_size.x;
    }
    if pos.y + card_size.y > canvas_rect.bottom() - TILE_MARGIN {
        pos.y = (collar.y - 18.0 - card_size.y).max(canvas_rect.top() + TILE_MARGIN);
    }

    egui::Area::new(egui::Id::new("blast_hole_card"))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            tile_frame(ui.visuals()).show(ui, |ui| {
                ui.set_width(card_size.x - 20.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&hole.dhid).strong());
                    let mut detail = format!("{:.1} m", bottom - top);
                    if let Some(diameter) = hole.diameter {
                        detail.push_str(&format!(" · Ø {:.0} mm", diameter * 1_000.0));
                    }
                    ui.label(egui::RichText::new(detail).weak());
                });
                let weak = ui.visuals().weak_text_color();
                let warn = ui.visuals().warn_fg_color;
                egui::Grid::new("blast_hole_card_grid").num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                    let mut row = |label: String, value: String, color: Option<egui::Color32>| {
                        ui.label(egui::RichText::new(label).color(weak));
                        let text = egui::RichText::new(value);
                        ui.label(match color {
                            Some(color) => text.color(color),
                            None => text,
                        });
                        ui.end_row();
                    };
                    if let Some(analysis) = analysis {
                        let index = hover.hole.hole;
                        match analysis.times[index] {
                            Some(time) => row(tr!(literal = "Fires at"), format!("{time:.0} ms"), None),
                            None if analysis.is_empty_hole(index) => row(tr!(literal = "Fires at"), tr!(literal = "empty, won't detonate"), None),
                            None => row(tr!(literal = "Fires at"), tr!(literal = "not reached"), Some(warn)),
                        }
                        let band = analysis.band(index, editor.blast_review.limits);
                        let relief = match (analysis.relief[index], analysis.relieved_by[index]) {
                            (Some(value), Some(by)) => {
                                let name = dataset.dataset.holes.get(by).map_or("?", |hole| hole.dhid.as_str());
                                tr_format!(literal = "%value% ms/m from %hole%", value = format!("{value:.1}"), hole = name)
                            }
                            _ if band == ReliefBand::Free => tr!(literal = "fires first: free face"),
                            _ => "-".to_owned(),
                        };
                        let color = match (band, analysis.relief[index]) {
                            (ReliefBand::Tight | ReliefBand::Slack, Some(value)) => Some(color32(crate::model::blast::relief_color(value, editor.blast_review.limits))),
                            _ => None,
                        };
                        row(tr!(literal = "Relief"), relief, color);
                        if let Some(charge) = charge {
                            row(tr!(literal = "Explosive"), format!("{:.1} kg", analysis.mass_kg[index]), None);
                            if let Some(powder_factor) = analysis.hole_powder_factor(index) {
                                row(tr!(literal = "Powder factor"), format!("{powder_factor:.2} kg/m³"), None);
                            }
                            row(tr!(literal = "Rule"), charge.rule.clone(), None);
                        }
                    } else if let Some(charge) = charge {
                        row(tr!(literal = "Explosive"), format!("{:.1} kg", charge.mass_kg(hole.diameter)), None);
                        row(tr!(literal = "Rule"), charge.rule.clone(), None);
                    }
                });
                match charge {
                    Some(charge) => {
                        ui.add_space(6.0);
                        draw_column_with_legend(ui, &charge.decks, &charge.primers, top, bottom, 104.0);
                    }
                    None => {
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(tr!(literal = "Not loaded")).weak().italics());
                    }
                }
            });
        });
}

/// A loaded column drawn upright, collar at the top, with each deck named
/// beside it. Shared by the hole card and the rule editor's preview.
pub(crate) fn draw_column_with_legend(ui: &mut egui::Ui, decks: &[ChargeDeck], primers: &[Primer], top: f64, bottom: f64, height: f32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, height), egui::Sense::hover());
        paint_column(ui.painter(), rect, decks, primers, top, bottom, ui.visuals());
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for deck in decks {
                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().rect_filled(swatch, 1.5, color32(deck.color));
                    let what = match deck.kind {
                        DeckKind::Explosive => deck.product.clone(),
                        DeckKind::Stemming | DeckKind::Air => format!("{} ({})", deck.product, deck.kind.label().to_lowercase()),
                    };
                    ui.label(egui::RichText::new(format!("{:.2} m  {what}", deck.length())).small());
                });
            }
            if let Some(primer) = primers.first() {
                ui.label(
                    egui::RichText::new(tr_format!(
                        literal = "%count% primer(s) · %delay% ms downhole",
                        count = primers.len(),
                        delay = primer.delay_ms
                    ))
                    .small()
                    .weak(),
                );
            }
        });
    });
}

/// Paint `decks` into `rect` from `top` (the collar, at the rect's top) down
/// to `bottom`, primers marked where they sit.
pub(crate) fn paint_column(painter: &egui::Painter, rect: egui::Rect, decks: &[ChargeDeck], primers: &[Primer], top: f64, bottom: f64, visuals: &egui::Visuals) {
    let span = (bottom - top).max(1.0e-6);
    let y = |depth: f64| rect.top() + ((depth - top) / span).clamp(0.0, 1.0) as f32 * rect.height();
    painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    for deck in decks {
        let band = egui::Rect::from_x_y_ranges(rect.x_range(), y(deck.from)..=y(deck.to));
        painter.rect_filled(band, 0.0, color32(deck.color));
    }
    for primer in primers {
        let center = egui::pos2(rect.center().x, y(primer.depth));
        let marker = egui::Rect::from_center_size(center, egui::vec2(rect.width() * 0.55, 5.0));
        painter.rect(
            marker,
            1.0,
            TIMELINE_BAR,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0x6A, 0x46, 0x00)),
            egui::StrokeKind::Inside,
        );
    }
    painter.rect_stroke(rect, GROUP_CORNER_RADIUS, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
}

#[derive(Clone, Copy)]
enum Transport {
    Play,
    Pause,
    Rewind,
}

/// A small square transport button with its glyph painted rather than set
/// in type, so it does not depend on the UI font carrying media symbols.
fn transport_button(ui: &mut egui::Ui, kind: Transport, tooltip: String) -> egui::Response {
    let side = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, GROUP_CORNER_RADIUS, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let color = visuals.fg_stroke.color;
    let c = rect.center();
    let r = side * 0.24;
    match kind {
        Transport::Play => {
            let points = vec![egui::pos2(c.x - r * 0.8, c.y - r), egui::pos2(c.x + r, c.y), egui::pos2(c.x - r * 0.8, c.y + r)];
            ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
        }
        Transport::Pause => {
            for dx in [-r * 0.55, r * 0.55] {
                ui.painter()
                    .rect_filled(egui::Rect::from_center_size(egui::pos2(c.x + dx, c.y), egui::vec2(r * 0.55, r * 2.0)), 1.0, color);
            }
        }
        Transport::Rewind => {
            ui.painter()
                .rect_filled(egui::Rect::from_center_size(egui::pos2(c.x - r * 0.9, c.y), egui::vec2(r * 0.35, r * 2.0)), 0.5, color);
            let points = vec![egui::pos2(c.x + r, c.y - r), egui::pos2(c.x - r * 0.55, c.y), egui::pos2(c.x + r, c.y + r)];
            ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
        }
    }
    response.on_hover_text(tooltip)
}

/// The blast played through: transport and the site's charge limit, a strip
/// of the surface signal over the detonations it sets off that scrubs the
/// playhead, and what is going off in the 8 ms at it against the busiest.
fn draw_timeline(ui: &egui::Ui, editor: &mut EditorState, analysis: &BlastAnalysis, canvas_rect: egui::Rect) {
    let Some(end) = analysis.timeline_end_ms() else {
        egui::Area::new(egui::Id::new("blast_timeline"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(canvas_rect.center_bottom() - egui::vec2(0.0, TILE_MARGIN))
            .show(ui.ctx(), |ui| {
                tile_frame(ui.visuals()).show(ui, |ui| {
                    ui.label(tr!(literal = "Set an initiation point and tie the holes in to play the round"));
                });
            });
        editor.blast_review.playing = false;
        return;
    };
    // What the readout counts to: the last detonation, or - in a pattern
    // being loaded with nothing reached loaded yet - the signal's end.
    let duration = analysis.duration_ms.unwrap_or(end);
    let review = &mut editor.blast_review;
    if review.playing {
        let dt = f64::from(ui.ctx().input(|input| input.stable_dt).min(0.1));
        review.playhead_ms += dt * 1_000.0 * review.speed;
        if review.playhead_ms >= end {
            review.playhead_ms = end;
            review.playing = false;
        }
        ui.ctx().request_repaint();
    }
    let width = (canvas_rect.width() - 2.0 * TILE_MARGIN).clamp(260.0, TIMELINE_MAX_WIDTH);
    let loaded = analysis.charged_holes > 0;

    egui::Area::new(egui::Id::new("blast_timeline"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(canvas_rect.center_bottom() - egui::vec2(0.0, TILE_MARGIN))
        .show(ui.ctx(), |ui| {
            tile_frame(ui.visuals()).show(ui, |ui| {
                ui.set_width(width - 20.0);
                ui.horizontal(|ui| {
                    if review.playing {
                        if transport_button(ui, Transport::Pause, tr!(literal = "Pause")).clicked() {
                            review.playing = false;
                        }
                    } else if transport_button(ui, Transport::Play, tr!(literal = "Play")).clicked() {
                        if review.playhead_ms >= end {
                            review.playhead_ms = 0.0;
                        }
                        review.playing = true;
                    }
                    if transport_button(ui, Transport::Rewind, tr!(literal = "Back to the start")).clicked() {
                        review.playhead_ms = 0.0;
                    }
                    ui.label(egui::RichText::new(format!("{:.0} ms", review.playhead_ms.min(duration))).strong());
                    ui.label(egui::RichText::new(tr_format!(literal = "of %duration% ms", duration = format!("{duration:.0}"))).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let speed_label = |speed: f64| {
                            if speed >= 1.0 {
                                tr!(literal = "Real time")
                            } else {
                                format!("{:.0}× slower", 1.0 / speed)
                            }
                        };
                        egui::ComboBox::from_id_salt("blast_timeline_speed")
                            .selected_text(speed_label(review.speed))
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for speed in TIMELINE_SPEEDS {
                                    ui.selectable_value(&mut review.speed, speed, speed_label(speed));
                                }
                            });
                        if loaded {
                            ui.add_space(12.0);
                            // The site's charge-per-delay limit, held to only
                            // while it is ticked.
                            let mut limit = review.mic_limit_kg.unwrap_or(analysis.peak_mass.value.max(1.0).round());
                            let mut on = review.mic_limit_kg.is_some();
                            ui.add_enabled(on, egui::DragValue::new(&mut limit).range(1.0..=1_000_000.0).speed(5.0).max_decimals(0).suffix(" kg"));
                            ui.checkbox(&mut on, tr!(literal = "MIC limit")).on_hover_text(tr!(
                                literal = "The most explosive allowed to detonate in any 8 ms at this site. Windows over it are flagged."
                            ));
                            review.mic_limit_kg = on.then_some(limit);
                        }
                    });
                });
                ui.add_space(6.0);
                if let Some(playhead) = timeline_strip(ui, analysis, review.playhead_ms, end, review.mic_limit_kg) {
                    review.playhead_ms = playhead;
                    review.playing = false;
                }
                ui.add_space(2.0);
                if !loaded {
                    ui.label(
                        egui::RichText::new(tr!(
                            literal = "No holes are loaded: the surface signal plays, but nothing detonates. Load holes with the Charge Holes tool."
                        ))
                        .weak(),
                    );
                    return;
                }
                let (holes, mass) = analysis.window_at(review.playhead_ms - VIBRATION_WINDOW_MS + 1.0e-9);
                ui.horizontal(|ui| {
                    let mut now = tr_format!(literal = "Now: %holes% hole(s)", holes = holes);
                    if analysis.total_mass_kg > 0.0 {
                        now.push_str(&format!(" · {mass:.0} kg"));
                    }
                    now.push_str(&tr!(literal = " in 8 ms"));
                    ui.label(now);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let has_mass = analysis.total_mass_kg > 0.0;
                        let peak = if has_mass { analysis.peak_mass } else { analysis.peak_holes };
                        let mut text = if has_mass {
                            tr_format!(
                                literal = "Peak %mass% kg at %time% ms",
                                mass = format!("{:.0}", peak.value),
                                time = format!("{:.0}", peak.start_ms)
                            )
                        } else {
                            tr_format!(literal = "Peak %holes% hole(s) at %time% ms", holes = peak.value, time = format!("{:.0}", peak.start_ms))
                        };
                        // Red only for a broken limit: a peak is not a fault.
                        let color = match review.mic_limit_kg.filter(|_| has_mass) {
                            Some(limit) if peak.value > limit => {
                                text.push_str(&tr_format!(literal = ", %over% kg over", over = format!("{:.0}", peak.value - limit)));
                                TIMELINE_PEAK
                            }
                            Some(_) => {
                                text.push_str(&tr!(literal = ", within limit"));
                                TIMELINE_WITHIN
                            }
                            None => ui.visuals().text_color(),
                        };
                        ui.label(egui::RichText::new(text).color(color));
                    });
                });
            });
        });
}

/// The strip itself: the surface signal as a thin lane over the detonations,
/// the time across beneath. Windows over the charge limit are shaded red.
/// Returns the playhead the user scrubbed to, if they did.
fn timeline_strip(ui: &mut egui::Ui, analysis: &BlastAnalysis, playhead: f64, end: f64, limit: Option<f64>) -> Option<f64> {
    let height = TIMELINE_LANE_HEIGHT + TIMELINE_LANE_GAP + TIMELINE_STRIP_HEIGHT;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height + TIMELINE_AXIS_HEIGHT), egui::Sense::click_and_drag());
    let strip = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), height));
    let lane = egui::Rect::from_min_size(strip.min, egui::vec2(strip.width(), TIMELINE_LANE_HEIGHT));
    let bars = egui::Rect::from_min_max(egui::pos2(strip.left(), lane.bottom() + TIMELINE_LANE_GAP), strip.max);
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let weak = visuals.weak_text_color();
    painter.rect_filled(lane, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    painter.rect_filled(bars, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    let x = |time: f64| strip.left() + (time / end).clamp(0.0, 1.0) as f32 * strip.width();

    // Binned to roughly three pixels, so a dense round reads as a histogram
    // rather than a comb.
    let bins = ((strip.width() / 3.0) as usize).max(1);
    let bin_of = |time: f64| ((time / end) * bins as f64).floor().clamp(0.0, (bins - 1) as f64) as usize;
    let bin_width = strip.width() / bins as f32;
    let bin_passed = |bin: usize| (bin as f64 + 0.5) / bins as f64 * end <= playhead;

    // The surface signal: how many downlines it lights in each bin.
    let mut lit = vec![0usize; bins];
    for time in analysis.surface_times.iter().flatten() {
        lit[bin_of(*time)] += 1;
    }
    let busiest = lit.iter().copied().max().unwrap_or(1).max(1) as f32;
    for (bin, count) in lit.iter().enumerate().filter(|(_, count)| **count > 0) {
        let left = strip.left() + bin as f32 * bin_width;
        let alpha = 0.35 + 0.65 * (*count as f32 / busiest);
        let color = if bin_passed(bin) { FUSE_LIT } else { FUSE_LIT.gamma_multiply(0.3) };
        painter.rect_filled(
            egui::Rect::from_x_y_ranges(left..=left + bin_width.max(1.5) - 0.5, lane.shrink(1.0).y_range()),
            0.0,
            color.gamma_multiply(alpha),
        );
    }

    // Windows over the limit, shaded behind the bars they hold.
    if let Some(limit) = limit {
        for (index, hole) in analysis.firing_order.iter().enumerate() {
            if analysis.window_mass.get(index).is_some_and(|mass| *mass > limit) {
                let start = analysis.times[*hole].unwrap_or(0.0);
                let band = egui::Rect::from_x_y_ranges(x(start)..=x(start + VIBRATION_WINDOW_MS).max(x(start) + 1.0), bars.y_range());
                painter.rect_filled(band, 0.0, TIMELINE_PEAK.gamma_multiply(0.22));
            }
        }
    }

    // Detonations - of loaded holes only, as in the scene - and whether any
    // of a bin's opens a window over the limit.
    let mut counts = vec![0usize; bins];
    let mut over = vec![false; bins];
    for (index, hole) in analysis.firing_order.iter().enumerate() {
        if !analysis.is_loaded(*hole) {
            continue;
        }
        let bin = bin_of(analysis.times[*hole].unwrap_or(0.0));
        counts[bin] += 1;
        over[bin] |= limit.is_some_and(|limit| analysis.window_mass.get(index).is_some_and(|mass| *mass > limit));
    }
    let tallest = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    for (bin, count) in counts.iter().enumerate().filter(|(_, count)| **count > 0) {
        let height = (*count as f32 / tallest) * (bars.height() - 6.0);
        let left = strip.left() + bin as f32 * bin_width;
        let bar = egui::Rect::from_min_max(egui::pos2(left, bars.bottom() - height), egui::pos2(left + bin_width.max(1.5) - 0.5, bars.bottom()));
        let base = if over[bin] { TIMELINE_PEAK } else { TIMELINE_BAR };
        painter.rect_filled(bar, 0.0, if bin_passed(bin) { base } else { base.gamma_multiply(0.35) });
    }

    // The busiest window, outlined - in red only when it breaks the limit.
    if analysis.charged_holes > 0 {
        let has_mass = analysis.total_mass_kg > 0.0;
        let peak = if has_mass { analysis.peak_mass } else { analysis.peak_holes };
        let broken = has_mass && limit.is_some_and(|limit| peak.value > limit);
        let outline = egui::Rect::from_x_y_ranges(x(peak.start_ms)..=x(peak.start_ms + VIBRATION_WINDOW_MS).max(x(peak.start_ms) + 2.0), bars.y_range());
        painter.rect_stroke(outline, 0.0, egui::Stroke::new(1.0, if broken { TIMELINE_PEAK } else { weak }), egui::StrokeKind::Inside);
    }

    // The playhead across both lanes, with the 8 ms behind it shaded.
    let window = egui::Rect::from_x_y_ranges(x(playhead - VIBRATION_WINDOW_MS)..=x(playhead), bars.y_range());
    painter.rect_filled(window, 0.0, visuals.selection.bg_fill.gamma_multiply(0.35));
    painter.line_segment(
        [egui::pos2(x(playhead), strip.top()), egui::pos2(x(playhead), strip.bottom())],
        egui::Stroke::new(2.0, visuals.strong_text_color()),
    );
    painter.rect_stroke(lane, GROUP_CORNER_RADIUS, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    painter.rect_stroke(bars, GROUP_CORNER_RADIUS, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);

    // Time beneath, every round step.
    let step = crate::model::blast::nice_step(end / 6.0);
    let font = egui::FontId::proportional(10.0);
    let mut tick = 0.0;
    while tick <= end + 1.0e-6 {
        let tick_x = x(tick);
        painter.line_segment([egui::pos2(tick_x, strip.bottom()), egui::pos2(tick_x, strip.bottom() + 3.0)], egui::Stroke::new(1.0, weak));
        let align = if tick == 0.0 { egui::Align2::LEFT_TOP } else { egui::Align2::CENTER_TOP };
        let label = if tick == 0.0 { "0 ms".to_owned() } else { format!("{tick:.0}") };
        if tick_x < strip.right() - 14.0 || tick == 0.0 {
            painter.text(egui::pos2(tick_x, strip.bottom() + 3.0), align, label, font.clone(), weak);
        }
        tick += step;
    }

    let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal).on_hover_text(tr!(
        literal = "Top: the surface signal lighting each downline. Below: detonations. Click or drag to move the playhead."
    ));
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        return Some((((pointer.x - strip.left()) / strip.width()).clamp(0.0, 1.0) as f64) * end);
    }
    None
}
