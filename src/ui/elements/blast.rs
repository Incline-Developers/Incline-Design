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
        blast::{BlastAnalysis, ChargeDeck, DeckKind, Primer, RELIEF_FREE_COLOR, RELIEF_GOOD_COLOR, RELIEF_SLACK_COLOR, RELIEF_TIGHT_COLOR, ReliefBand, VIBRATION_WINDOW_MS},
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
const CONTOUR_EARLY: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xD0, 0x40);
const CONTOUR_LATE: egui::Color32 = egui::Color32::from_rgb(0x8C, 0x52, 0xFF);
/// A contour run shorter than this on screen goes unlabelled: its label
/// would cover more than it named.
const CONTOUR_LABEL_MIN_PX: f32 = 90.0;
/// Gap between a floating tile and the canvas edge, and between tiles.
const TILE_MARGIN: f32 = 12.0;
/// Playback rates offered by the timeline, as firing ms per real ms.
const TIMELINE_SPEEDS: [f64; 5] = [0.02, 0.05, 0.1, 0.25, 1.0];
const TIMELINE_STRIP_HEIGHT: f32 = 54.0;
const TIMELINE_MAX_WIDTH: f32 = 760.0;
/// The colour a detonation is drawn in on the timeline: the firing-collar
/// yellow the renderer uses, so strip and scene read as one.
const TIMELINE_BAR: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xCC, 0x2E);
const TIMELINE_PEAK: egui::Color32 = egui::Color32::from_rgb(0xDE, 0x33, 0x38);
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
    draw_contours(ui, editor, canvas_rect);
    let analysis = editor.blast_analysis.clone();
    let active_visible = editor
        .active_drill_hole
        .is_some_and(|id| drill_holes.iter().any(|dataset| dataset.id == id && dataset.state.loaded));
    if let Some(analysis) = analysis.as_deref().filter(|_| active_visible) {
        if editor.blast_review.timeline {
            if let Some(dataset) = drill_holes.iter().find(|dataset| Some(dataset.id) == editor.active_drill_hole) {
                draw_timeline_scene(ui, editor, analysis, dataset, canvas_rect);
            }
            draw_timeline(ui, editor, analysis, canvas_rect);
        } else if editor.blast_review.relief {
            draw_relief_legend(ui, editor, analysis, canvas_rect);
        }
    }
    draw_hole_card(ui, editor, analysis.as_deref(), drill_holes, canvas_rect);
}

fn draw_contours(ui: &egui::Ui, editor: &EditorState, canvas_rect: egui::Rect) {
    if editor.blast_contours_px.is_empty() {
        return;
    }
    let pixels_per_point = ui.ctx().pixels_per_point();
    let painter = ui.painter().with_clip_rect(canvas_rect);
    let latest = editor.blast_contours_px.iter().map(|contour| contour.time_ms).fold(0.0, f64::max).max(1.0);
    let font = egui::FontId::proportional(11.0);
    let halo = menu::menu_surface(ui.visuals()).gamma_multiply(0.85);
    let mut labels: Vec<egui::Rect> = Vec::new();
    for contour in &editor.blast_contours_px {
        let color = CONTOUR_EARLY.lerp_to_gamma(CONTOUR_LATE, (contour.time_ms / latest) as f32);
        let stroke = egui::Stroke::new(1.6, color);
        let to_pos = |point: (f32, f32)| egui::pos2(point.0 / pixels_per_point, point.1 / pixels_per_point);
        // Runs between clipped points are drawn piecewise.
        let mut run: Vec<egui::Pos2> = Vec::new();
        let mut longest: (f32, Vec<egui::Pos2>) = (0.0, Vec::new());
        let mut flush = |run: &mut Vec<egui::Pos2>| {
            if run.len() >= 2 {
                let length: f32 = run.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
                if length > longest.0 {
                    longest = (length, run.clone());
                }
                painter.add(egui::Shape::line(std::mem::take(run), stroke));
            }
            run.clear();
        };
        let closing = contour.closed.then(|| contour.points.first().copied().flatten()).flatten();
        for point in contour.points.iter().copied().chain(closing.map(Some)) {
            match point {
                Some(point) => run.push(to_pos(point)),
                None => flush(&mut run),
            }
        }
        flush(&mut run);

        // One label per run, at the middle of its longest visible piece, and
        // only where it neither crowds another label nor overruns a short run.
        let (length, points) = longest;
        if length < CONTOUR_LABEL_MIN_PX {
            continue;
        }
        let mut walked = 0.0;
        let anchor = points
            .windows(2)
            .find_map(|pair| {
                let step = pair[0].distance(pair[1]);
                if walked + step >= length * 0.5 {
                    let t = if step > 0.0 { (length * 0.5 - walked) / step } else { 0.0 };
                    Some(pair[0].lerp(pair[1], t))
                } else {
                    walked += step;
                    None
                }
            })
            .unwrap_or(points[0]);
        let galley = painter.layout_no_wrap(format!("{:.0}", contour.time_ms), font.clone(), color);
        let rect = egui::Rect::from_center_size(anchor, galley.size() + egui::vec2(6.0, 2.0));
        if !canvas_rect.contains_rect(rect) || labels.iter().any(|other| other.expand(4.0).intersects(rect)) {
            continue;
        }
        painter.rect_filled(rect, GROUP_CORNER_RADIUS, halo);
        painter.galley(rect.center() - galley.size() * 0.5, galley, color);
        labels.push(rect);
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

/// The key to the relief heatmap, with its two limits editable in place: the
/// bands are a judgement about this rock, and the collars recolour as they
/// are dragged.
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
                ui.label(egui::RichText::new(tr!(literal = "Burden relief")).strong());
                ui.label(egui::RichText::new(tr!(literal = "ms per metre to the last neighbour to fire")).small().weak());
                ui.add_space(4.0);
                egui::Grid::new("relief_legend_grid").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                    let swatch = |ui: &mut egui::Ui, color: [f32; 3]| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter()
                            .circle(rect.center(), 5.5, color32(color), egui::Stroke::new(1.0, egui::Color32::from_gray(60)));
                    };
                    let count = |ui: &mut egui::Ui, count: usize| {
                        ui.label(egui::RichText::new(count.to_string()).weak());
                    };
                    swatch(ui, RELIEF_TIGHT_COLOR);
                    ui.horizontal(|ui| {
                        ui.label(tr!(literal = "Tight, below"));
                        ui.add(egui::DragValue::new(&mut limits.low).range(0.1..=limits.high).speed(0.1).max_decimals(1).suffix(" ms/m"));
                    });
                    count(ui, tight);
                    ui.end_row();
                    swatch(ui, RELIEF_GOOD_COLOR);
                    ui.label(tr!(literal = "Good"));
                    count(ui, good);
                    ui.end_row();
                    swatch(ui, RELIEF_SLACK_COLOR);
                    ui.horizontal(|ui| {
                        ui.label(tr!(literal = "Slack, above"));
                        let low = limits.low;
                        ui.add(egui::DragValue::new(&mut limits.high).range(low..=1_000.0).speed(0.1).max_decimals(1).suffix(" ms/m"));
                    });
                    count(ui, slack);
                    ui.end_row();
                    swatch(ui, RELIEF_FREE_COLOR);
                    ui.label(tr!(literal = "Fires first: free face"));
                    count(ui, free);
                    ui.end_row();
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
                        let color = matches!(band, ReliefBand::Tight | ReliefBand::Slack).then(|| color32(band.color()));
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

/// The blast played through: transport, a strip of detonations over time
/// that scrubs the playhead, and what is going off in the 8 ms behind it.
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
                    ui.label(egui::RichText::new(format!("{:.0} ms", review.playhead_ms.min(duration))).strong().monospace());
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
                    });
                });
                ui.add_space(4.0);
                if let Some(playhead) = timeline_strip(ui, analysis, review.playhead_ms, end) {
                    review.playhead_ms = playhead;
                    review.playing = false;
                }
                ui.add_space(4.0);
                if analysis.charged_holes == 0 {
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
                    let mut now = tr_format!(literal = "Last 8 ms: %holes% hole(s)", holes = holes);
                    if analysis.total_mass_kg > 0.0 {
                        now.push_str(&format!(" · {mass:.0} kg"));
                    }
                    ui.label(now);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut peak = tr_format!(
                            literal = "Peak: %holes% hole(s) at %time% ms",
                            holes = analysis.peak_holes.value,
                            time = format!("{:.0}", analysis.peak_holes.start_ms)
                        );
                        if analysis.total_mass_kg > 0.0 {
                            peak.push_str(&tr_format!(
                                literal = " · MIC %mass% kg at %time% ms",
                                mass = format!("{:.0}", analysis.peak_mass.value),
                                time = format!("{:.0}", analysis.peak_mass.start_ms)
                            ));
                        }
                        ui.label(egui::RichText::new(peak).color(TIMELINE_PEAK));
                    });
                });
            });
        });
}

/// The strip itself. Returns the playhead the user scrubbed to, if they did.
fn timeline_strip(ui: &mut egui::Ui, analysis: &BlastAnalysis, playhead: f64, end: f64) -> Option<f64> {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TIMELINE_STRIP_HEIGHT), egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    let x = |time: f64| rect.left() + (time / end).clamp(0.0, 1.0) as f32 * rect.width();

    // Detonations binned to roughly three pixels, so a dense round reads as
    // a histogram rather than a comb.
    let bins = ((rect.width() / 3.0) as usize).max(1);
    let mut counts = vec![0usize; bins];
    // Detonations are of loaded holes only, as in the scene.
    for hole in analysis.firing_order.iter().filter(|hole| analysis.is_loaded(**hole)) {
        let time = analysis.times[*hole].unwrap_or(0.0);
        let bin = ((time / end) * bins as f64).floor().clamp(0.0, (bins - 1) as f64) as usize;
        counts[bin] += 1;
    }
    let tallest = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    let bin_width = rect.width() / bins as f32;
    for (bin, count) in counts.iter().enumerate().filter(|(_, count)| **count > 0) {
        let height = (*count as f32 / tallest) * (rect.height() - 10.0);
        let left = rect.left() + bin as f32 * bin_width;
        let bar = egui::Rect::from_min_max(egui::pos2(left, rect.bottom() - height), egui::pos2(left + bin_width.max(1.5) - 0.5, rect.bottom()));
        let fired = analysis.times.is_empty() || (bin as f64 + 0.5) / bins as f64 * end <= playhead;
        painter.rect_filled(bar, 0.0, if fired { TIMELINE_BAR } else { TIMELINE_BAR.gamma_multiply(0.35) });
    }

    // The busiest window by mass - or by holes, where diameters are unknown.
    let peak = if analysis.total_mass_kg > 0.0 { analysis.peak_mass } else { analysis.peak_holes };
    let peak_rect = egui::Rect::from_x_y_ranges(x(peak.start_ms)..=x(peak.start_ms + VIBRATION_WINDOW_MS).max(x(peak.start_ms) + 2.0), rect.y_range());
    if analysis.charged_holes > 0 {
        painter.rect_stroke(peak_rect, 0.0, egui::Stroke::new(1.0, TIMELINE_PEAK), egui::StrokeKind::Inside);
    }

    // The playhead, with the 8 ms behind it shaded.
    let window = egui::Rect::from_x_y_ranges(x(playhead - VIBRATION_WINDOW_MS)..=x(playhead), rect.y_range());
    painter.rect_filled(window, 0.0, visuals.selection.bg_fill.gamma_multiply(0.35));
    painter.line_segment(
        [egui::pos2(x(playhead), rect.top()), egui::pos2(x(playhead), rect.bottom())],
        egui::Stroke::new(2.0, visuals.strong_text_color()),
    );
    painter.rect_stroke(rect, GROUP_CORNER_RADIUS, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);

    let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        return Some((((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64) * end);
    }
    None
}
