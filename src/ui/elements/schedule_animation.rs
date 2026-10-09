//! Controls belonging to Schedule's main-viewport animation page.

use crate::{
    i18n::tr,
    model::{Document, schedule::SCHEDULE_PERIOD_H},
    ui::{EditorState, chrome, widgets::toolbar::GROUP_CORNER_RADIUS},
};

pub(crate) const TIMELINE_PANEL_ID: &str = "schedule_animation_timeline";

/// Strides the period rules step through as the band fills up, coarsest last.
/// Round numbers, so a thinned scale still reads as one: every second day,
/// every fifth, every tenth, and so on.
const RULE_STRIDES: [usize; 10] = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000];
/// Closer together than this and a rule at every period boundary is a grey
/// block rather than a scale, so the rules thin to the next round stride.
const MIN_RULE_SPACING: f32 = 5.0;
/// Narrower than this and a period is drawn as part of one strip rather
/// than as its own tile: a tile needs room for its gaps and still to read.
const MIN_TILE_WIDTH: f32 = 8.0;
/// The space between two period tiles.
const TILE_GAP: f32 = 2.0;
/// The row above the band that carries the cursor's instant.
const READOUT_ROW_H: f32 = 22.0;
const READOUT_SIZE: f32 = 16.0;
const STATUS_SIZE: f32 = 12.0;

pub(crate) fn draw_timeline(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, plan: &crate::model::schedule::SchedulePlan) -> egui::Rect {
    egui::Panel::bottom(TIMELINE_PANEL_ID)
        .resizable(false)
        .show_separator_line(chrome::show_separator_line(ui))
        .exact_size(104.0)
        .frame(chrome::region_frame(ui).inner_margin(egui::Margin::symmetric(14, 8)))
        .show(ui, |ui| {
            let horizon = editor.schedule_animation_horizon_h.max(0.0);

            // The band no longer names its periods, so this line is the only
            // thing that says where the cursor is. It is centred over the
            // scrubber it describes, and it is the whole row: the status only
            // appears when the slider is off, and the instant is then a fixed
            // zero saying nothing, so the two never compete for the space.
            //
            // Painted into a row of fixed height rather than laid out from
            // widgets, so nothing below it can move. Text that changes width
            // as the cursor is dragged, and an indicator that comes and goes
            // with the work, both shift a laid-out row - and the band and the
            // slider under it then shake for the length of the drag.
            let (readout, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), READOUT_ROW_H), egui::Sense::hover());
            if editor.schedule_animation_enabled {
                response.on_hover_ui(|ui| {
                    if let Some(schedule) = &editor.schedule_result {
                        let destinations = crate::model::schedule::destinations::available(document.solids(), plan.routing());
                        for pile in &schedule.piles {
                            if let Some((tonnes, _)) = schedule.inventory_at(pile.destination, editor.schedule_animation_shown_h) {
                                let name = destinations
                                    .iter()
                                    .find(|entry| entry.id == pile.destination)
                                    .map(|entry| entry.name.clone())
                                    .unwrap_or_default();
                                ui.label(tr!("schedule-animation-inventory", stockpile = name, tonnes = format!("{tonnes:.1}")));
                            }
                        }
                    }
                });
            }
            if editor.schedule_animation_enabled || editor.schedule_animation_status.is_empty() {
                // The instant the geometry was cut for, not the instant the
                // slider is at: while a batch is in flight those differ, and
                // the number that names what is on screen is the one that can
                // be trusted against it.
                //
                // Dimmed while the cut is being caught up with, which is the
                // only progress this needs to report. Said with the colour
                // rather than with a spinner because a spinner is animated,
                // and an animated widget asks for a repaint every frame it is
                // visible - the whole scene redrawn, for as long as a scrub
                // keeps work in flight. Warned when a block would not cut at
                // all: the view then holds ground from an earlier instant that
                // no later frame will replace, so the state persists rather
                // than passing like the lag does.
                let color = if editor.schedule_animation_degraded {
                    ui.visuals().warn_fg_color
                } else if editor.schedule_animation_pending {
                    ui.visuals().weak_text_color()
                } else {
                    ui.visuals().strong_text_color()
                };
                paint_centered_line(ui, readout, instant_label(editor.schedule_animation_shown_h), READOUT_SIZE, color);
            } else {
                paint_centered_line(ui, readout, editor.schedule_animation_status.clone(), STATUS_SIZE, ui.visuals().weak_text_color());
            }

            let band_height = 20.0;
            let (band, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), band_height), egui::Sense::hover());
            paint_period_band(ui, band, horizon, editor.schedule_time_h, &plan.periods().colors);

            ui.spacing_mut().slider_width = ui.available_width();
            ui.spacing_mut().interact_size.y = 28.0;
            ui.add_enabled(
                editor.schedule_animation_enabled,
                egui::Slider::new(&mut editor.schedule_time_h, 0.0..=horizon.max(f64::EPSILON))
                    .show_value(false)
                    .clamping(egui::SliderClamping::Edits),
            );
        })
        .response
        .rect
}

/// Paints one line centred in `rect`, shortened with an ellipsis rather than
/// spilling past the edges.
fn paint_centered_line(ui: &egui::Ui, rect: egui::Rect, text: String, size: f32, color: egui::Color32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text, egui::FontId::proportional(size), color);
    job.wrap.max_width = rect.width();
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    let placed = egui::Rect::from_center_size(rect.center(), galley.size());
    ui.painter().galley(placed.min, galley, color);
}

/// The band under the readout: the run divided into its periods, with the
/// current one lit.
///
/// The divisions are what say how long a period is against the run, so they
/// stay however many there are - thinned to a round stride once they are too
/// close together to read as separate marks. Nothing is written inside them.
/// A hundred day numbers crammed into a hundred narrow cells named nothing and
/// overran the band; the instant is spelled out in full on the line above,
/// where it has the whole width to itself.
fn paint_period_band(ui: &egui::Ui, rect: egui::Rect, horizon: f64, current_h: f64, colors: &std::collections::BTreeMap<u32, [u8; 3]>) {
    let painter = ui.painter();
    let visuals = ui.visuals();
    let rule = egui::Stroke::new(1.0, visuals.widgets.inactive.bg_stroke.color);
    // On the panel's own colour: the tiles, or the strip, are the band.
    if horizon <= 0.0 {
        painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.widgets.inactive.weak_bg_fill);
        return;
    }
    let periods = (horizon / SCHEDULE_PERIOD_H).ceil().max(1.0) as usize;
    let cell_width = rect.width() / periods as f32;
    let current = ((current_h / SCHEDULE_PERIOD_H).floor().max(0.0) as usize).min(periods - 1);
    let span = |period: usize| {
        let left = egui::lerp(rect.x_range(), ((period as f64 * SCHEDULE_PERIOD_H) / horizon).min(1.0) as f32);
        let right = egui::lerp(rect.x_range(), (((period + 1) as f64 * SCHEDULE_PERIOD_H) / horizon).min(1.0) as f32);
        (left, right)
    };

    if cell_width >= MIN_TILE_WIDTH {
        // Wide enough for each period to be its own tile, with a gap either
        // side: the gaps are what say where one period ends.
        for period in 0..periods {
            let (left, right) = span(period);
            let tile = egui::Rect::from_x_y_ranges((left + TILE_GAP * 0.5)..=(right - TILE_GAP * 0.5), rect.y_range());
            let fill = colors.get(&(period as u32)).map_or(visuals.widgets.inactive.weak_bg_fill, |color| {
                super::schedule_periods::period_color(*color).gamma_multiply(0.85)
            });
            painter.rect_filled(tile, GROUP_CORNER_RADIUS, fill);
            if period == current {
                painter.rect_stroke(tile, GROUP_CORNER_RADIUS, egui::Stroke::new(1.5, visuals.selection.stroke.color), egui::StrokeKind::Inside);
            }
        }
    } else {
        // Too many to tile: one strip, coloured where days are marked, with
        // the current period lit and rules thinned to a round stride.
        painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.widgets.inactive.weak_bg_fill);
        for (period, color) in colors.range(..periods as u32) {
            let (left, right) = span(*period as usize);
            painter.rect_filled(
                egui::Rect::from_x_y_ranges(left..=right, rect.y_range()).shrink2(egui::vec2(0.0, 1.0)),
                0.0,
                super::schedule_periods::period_color(*color).gamma_multiply(0.7),
            );
        }
        let (left, right) = span(current);
        let cell = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(right, rect.bottom()));
        // A single-pixel cell has nothing left after shrinking, and the
        // highlight is the only thing marking the period at that width.
        painter.rect_filled(
            if cell.width() > 3.0 { cell.shrink(1.0) } else { cell },
            GROUP_CORNER_RADIUS,
            visuals.selection.bg_fill.gamma_multiply(0.45),
        );
        if let Some(stride) = RULE_STRIDES.into_iter().find(|stride| cell_width * *stride as f32 >= MIN_RULE_SPACING) {
            for period in (stride..periods).step_by(stride) {
                let boundary = egui::lerp(rect.x_range(), (period as f64 * SCHEDULE_PERIOD_H / horizon) as f32);
                painter.vline(boundary, rect.y_range(), rule);
            }
        }
        painter.rect_stroke(rect, GROUP_CORNER_RADIUS, rule, egui::StrokeKind::Inside);
    }

    let cursor_x = egui::lerp(rect.x_range(), (current_h.clamp(0.0, horizon) / horizon) as f32);
    painter.vline(cursor_x, rect.y_range(), egui::Stroke::new(2.0, visuals.selection.stroke.color));
}

fn instant_label(hours: f64) -> String {
    super::schedule_periods::instant_label(hours)
}
