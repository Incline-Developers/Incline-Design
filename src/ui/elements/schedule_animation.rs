//! Controls belonging to Schedule's main-viewport animation page.

use crate::{
    i18n::tr,
    model::{Document, schedule::SCHEDULE_PERIOD_H},
    ui::{
        EditorState, chrome,
        state::PlaybackSpan,
        widgets::{
            menu::MenuButton,
            toolbar::GROUP_CORNER_RADIUS,
            transport::{Transport, transport_button},
        },
    },
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
/// The row above the band that carries the cursor's instant, and the
/// playback controls to its left.
const READOUT_ROW_H: f32 = 24.0;
/// Extra room between the readout row and the period band under it.
const BAND_GAP: f32 = 6.0;
/// The space between the playback controls and the widest instant the
/// readout can name.
const CONTROLS_GAP: f32 = 16.0;
/// The shortest playback time the speed field takes, so a span is never
/// played through in no time at all.
const MIN_PLAY_SECONDS: f64 = 0.1;
/// The most one frame moves the cursor by, in real seconds: a frame held up
/// by a stall, or the page left and come back to, moves on by this much of
/// the schedule rather than by the whole gap.
const MAX_PLAY_STEP_S: f64 = 0.25;
const READOUT_SIZE: f32 = 16.0;
const STATUS_SIZE: f32 = 12.0;

pub(crate) fn draw_timeline(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    document: &Document,
    plan: &crate::model::schedule::SchedulePlan,
    commands: &mut Vec<crate::ui::state::UiCommand>,
) -> egui::Rect {
    egui::Panel::bottom(TIMELINE_PANEL_ID)
        .resizable(false)
        .show_separator_line(false)
        .exact_size(112.0)
        .frame(chrome::region_frame(ui).inner_margin(egui::Margin::symmetric(14, 8)))
        .show(ui, |ui| {
            let horizon = editor.schedule_animation_horizon_h.max(0.0);
            advance_playback(ui, editor, horizon);

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
            if let Some(progress) = editor.video_export_progress {
                draw_recording_progress(ui, readout, horizon, progress, commands);
            } else if editor.schedule_animation_enabled {
                draw_playback_controls(ui, editor, readout, horizon);
            }

            ui.add_space(BAND_GAP);
            let band_height = 20.0;
            let (band, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), band_height), egui::Sense::hover());
            paint_period_band(ui, band, horizon, editor.schedule_time_h, &plan.periods().colors);

            ui.spacing_mut().interact_size.y = 28.0;
            let slider = ui.add_enabled(
                editor.schedule_animation_enabled && editor.video_export_progress.is_none(),
                crate::ui::widgets::scrubber::Scrubber::new(&mut editor.schedule_time_h, 0.0..=horizon.max(f64::EPSILON)),
            );
            // Taking hold of the cursor stops playback, as it does in the
            // blast review.
            if slider.dragged() || slider.changed() {
                editor.schedule_animation_playing = false;
            }
        })
        .response
        .rect
}

/// Moves the cursor on by the real time since the last frame, at the set
/// speed, while playback is on. Stops at the end of the run.
fn advance_playback(ui: &egui::Ui, editor: &mut EditorState, horizon: f64) {
    if !editor.schedule_animation_playing || !editor.schedule_animation_enabled {
        editor.schedule_animation_playing = false;
        editor.schedule_animation_play_clock = None;
        return;
    }
    let now = ui.input(|input| input.time);
    let step_s = editor.schedule_animation_play_clock.map_or(0.0, |last| (now - last).clamp(0.0, MAX_PLAY_STEP_S));
    editor.schedule_animation_play_clock = Some(now);
    let hours_per_second = editor.schedule_animation_play_span.hours() / editor.schedule_animation_play_seconds.max(MIN_PLAY_SECONDS);
    editor.schedule_time_h = (editor.schedule_time_h.max(0.0) + step_s * hours_per_second).min(horizon);
    if editor.schedule_time_h >= horizon {
        editor.schedule_animation_playing = false;
        editor.schedule_animation_play_clock = None;
    } else {
        ui.ctx().request_repaint();
    }
}

/// The left of the readout row while a video records: how far it has got,
/// and Cancel.
fn draw_recording_progress(ui: &mut egui::Ui, row: egui::Rect, horizon: f64, (done, total): (usize, usize), commands: &mut Vec<crate::ui::state::UiCommand>) {
    let Some(rect) = controls_rect(ui, row, horizon) else {
        return;
    };
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    ui.set_clip_rect(rect);
    ui.spacing_mut().interact_size.y = READOUT_ROW_H;
    ui.spacing_mut().item_spacing.x = 8.0;
    if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
        commands.push(crate::ui::state::UiCommand::CancelVideoExport);
    }
    let fraction = done as f32 / total.max(1) as f32;
    ui.add(egui::ProgressBar::new(fraction).desired_width(160.0).corner_radius(GROUP_CORNER_RADIUS));
    ui.label(tr!("video-export-recording", done = done.to_string(), total = total.to_string()));
}

/// The part of the readout row left of the widest instant the run can name.
fn controls_rect(ui: &egui::Ui, row: egui::Rect, horizon: f64) -> Option<egui::Rect> {
    let widest = ui
        .painter()
        .layout_no_wrap(instant_label(horizon), egui::FontId::proportional(READOUT_SIZE), egui::Color32::WHITE)
        .size()
        .x;
    let right = row.center().x - widest * 0.5 - CONTROLS_GAP;
    (right > row.left()).then(|| egui::Rect::from_min_max(row.left_top(), egui::pos2(right, row.bottom())))
}

/// Play or Pause, Back to the start, and the speed - `[1 day] = [15 s]` -
/// from the row's left edge. Kept clear of the widest instant the run can
/// name rather than the one shown, so a narrow window never lets the
/// readout run under them while its width changes under playback.
fn draw_playback_controls(ui: &mut egui::Ui, editor: &mut EditorState, row: egui::Rect, horizon: f64) {
    let Some(rect) = controls_rect(ui, row, horizon) else {
        return;
    };
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    ui.set_clip_rect(rect);
    ui.spacing_mut().interact_size.y = READOUT_ROW_H;
    ui.spacing_mut().item_spacing.x = 4.0;
    if editor.schedule_animation_playing {
        if transport_button(&mut ui, Transport::Pause, tr!("animate-pause")).clicked() {
            editor.schedule_animation_playing = false;
        }
    } else if transport_button(&mut ui, Transport::Play, tr!("animate-play")).clicked() {
        // From the start again once it has played through.
        if editor.schedule_time_h >= horizon {
            editor.schedule_time_h = 0.0;
        }
        editor.schedule_animation_playing = true;
    }
    if transport_button(&mut ui, Transport::Rewind, tr!("animate-reset")).clicked() {
        editor.schedule_time_h = 0.0;
        editor.schedule_animation_playing = false;
    }
    ui.add_space(8.0);
    let span = &mut editor.schedule_animation_play_span;
    egui::ComboBox::from_id_salt("animate_play_span")
        .selected_text(span.label())
        .width(72.0)
        .show_ui(&mut ui, |ui| {
            for option in PlaybackSpan::ALL {
                ui.selectable_value(span, option, option.label());
            }
        })
        .response
        .on_hover_text(tr!("animate-speed-note"));
    ui.label("=");
    ui.add(
        egui::DragValue::new(&mut editor.schedule_animation_play_seconds)
            .range(MIN_PLAY_SECONDS..=86_400.0)
            .speed(0.5)
            .max_decimals(1)
            .suffix(" s"),
    )
    .on_hover_text(tr!("animate-speed-note"));
}

/// Paints one line centred in `rect`, shortened with an ellipsis rather than
/// spilling past the edges.
///
/// Centred on the height of its figures rather than on the font's line box,
/// which carries room for descenders under them and so sits the text high.
/// Measured on digits, not on the text itself, so a `y` coming and going
/// never moves the line.
fn paint_centered_line(ui: &egui::Ui, rect: egui::Rect, text: String, size: f32, color: egui::Color32) {
    let font = egui::FontId::proportional(size);
    let figures = ui.painter().layout_no_wrap("0123456789".to_owned(), font.clone(), color).mesh_bounds;
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap.max_width = rect.width();
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    let left = rect.center().x - galley.size().x * 0.5;
    let top = if figures.is_finite() {
        rect.center().y - figures.center().y
    } else {
        rect.center().y - galley.size().y * 0.5
    };
    ui.painter().galley(egui::pos2(left, top), galley, color);
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
