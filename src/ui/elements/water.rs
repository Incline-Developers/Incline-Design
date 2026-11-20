//! A rain simulation's tab: the timeline under the viewport, which is all of
//! its interface.
//!
//! The run itself, and the surfaces the viewport draws for it, are the app's
//! (`app/water.rs`); what is shown here is the mirror of the open run it
//! keeps in [`crate::ui::state::WaterView`].

use crate::{
    i18n::tr,
    ui::{
        chrome,
        state::{EditorState, WaterRunStatus},
        widgets::{
            scrubber::Scrubber,
            transport::{Transport, timeline_keys, transport_button},
        },
    },
};

const TIMELINE_PANEL_ID: &str = "water_timeline";
/// Simulated seconds played in a real one: five minutes.
const PLAY_RATE: f64 = 300.0;
/// What an arrow key steps the cursor by, in simulated seconds, and how many
/// of those steps it takes with Shift: five minutes, and an hour.
const KEY_STEP_S: f64 = 300.0;
const SHIFT_KEY_STEPS: f64 = 12.0;
/// The most a frame moves the cursor on by, in real seconds, so a stalled
/// frame does not skip the run.
const MAX_PLAY_STEP_S: f64 = 0.1;

/// Seconds into a run as hours and minutes: `3:05`.
pub(crate) fn clock(time_s: f64) -> String {
    let minutes = (time_s / 60.0).round().max(0.0) as u64;
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

/// The timeline under the viewport: Play, Back to the start, the instant
/// shown and a scrubber over the whole run. It plays as soon as the run has
/// anything worked out, as far as it has got: the scrubber's track is lighter
/// up to there, and playback that catches the run up waits on it.
pub(crate) fn draw_timeline(ui: &mut egui::Ui, editor: &mut EditorState) -> egui::Rect {
    egui::Panel::bottom(TIMELINE_PANEL_ID)
        .resizable(false)
        .show_separator_line(false)
        .exact_size(44.0)
        .frame(chrome::region_frame(ui).inner_margin(egui::Margin::symmetric(14, 8)))
        .show(ui, |ui| {
            let duration = editor.water.shown.as_ref().map_or(0.0, |shown| shown.settings.duration_s());
            let running = editor.water.shown.as_ref().is_some_and(|shown| matches!(shown.status, WaterRunStatus::Running(_)));
            // As far as the cursor can go, and where playback stops for good.
            let loaded = editor.water.shown.as_ref().map_or(0.0, |shown| shown.loaded_s).min(duration);
            let end = if running { duration } else { loaded };
            let ready = loaded > 0.0;
            keys(ui.ctx(), editor, loaded, end, ready);
            advance_playback(ui, editor, loaded, end, ready);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_enabled_ui(ready, |ui| {
                    if editor.water.playing {
                        if transport_button(ui, Transport::Pause, None).clicked() {
                            editor.water.playing = false;
                        }
                    } else if transport_button(ui, Transport::Play, None).clicked() {
                        play(editor, end);
                    }
                    if transport_button(ui, Transport::Rewind, None).clicked() {
                        editor.water.time_s = 0.0;
                        editor.water.playing = false;
                    }
                });
                ui.add_space(8.0);
                // The instant the viewport shows. As wide as the longest it
                // can be, so the scrubber beside it does not move under
                // playback.
                let shown = editor.water.shown.as_ref().and_then(|shown| shown.instant_s).unwrap_or(0.0);
                let text = tr!("water-clock", time = clock(shown), end = clock(duration));
                let widest = tr!("water-clock", time = clock(duration), end = clock(duration));
                let font = egui::TextStyle::Body.resolve(ui.style());
                let width = [&text, &widest]
                    .map(|text| ui.painter().layout_no_wrap(text.clone(), font.clone(), egui::Color32::PLACEHOLDER).size().x)
                    .into_iter()
                    .fold(0.0, f32::max);
                let (cell, _) = ui.allocate_exact_size(egui::vec2(width, ui.spacing().interact_size.y), egui::Sense::hover());
                let color = if ready { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
                ui.painter().text(cell.left_center(), egui::Align2::LEFT_CENTER, text, font, color);
                ui.add_space(8.0);
                let mut scrubber = Scrubber::new(&mut editor.water.time_s, 0.0..=duration.max(f64::EPSILON));
                if running {
                    scrubber = scrubber.loaded(loaded);
                }
                let slider = ui.add_enabled(ready, scrubber);
                // Taking hold of the cursor stops playback.
                if slider.dragged() || slider.changed() {
                    editor.water.playing = false;
                }
            });
        })
        .response
        .rect
}

/// Start playback, from the start again once it has played through to `end`.
fn play(editor: &mut EditorState, end: f64) {
    if editor.water.time_s >= end {
        editor.water.time_s = 0.0;
    }
    editor.water.playing = true;
}

/// The timeline's keys, as Animate's: Space plays and pauses, and the arrows
/// step the cursor five minutes (an hour with Shift), running at the playback
/// speed once held.
fn keys(ctx: &egui::Context, editor: &mut EditorState, loaded: f64, end: f64, ready: bool) {
    let keys = timeline_keys(ctx, "water_timeline_scrub_hold", ready, SHIFT_KEY_STEPS);
    if keys.toggle {
        if editor.water.playing {
            editor.water.playing = false;
        } else {
            play(editor, end);
        }
    }
    if !keys.scrubbing {
        return;
    }
    let water = &mut editor.water;
    water.playing = false;
    // A step goes from the whole five minutes, as Animate's does from the
    // whole hour.
    if keys.run_s != 0.0 {
        water.time_s += keys.run_s * PLAY_RATE;
    } else if keys.steps != 0.0 {
        let at = water.time_s / KEY_STEP_S;
        water.time_s = (if keys.steps > 0.0 { at.floor() } else { at.ceil() } + keys.steps) * KEY_STEP_S;
    }
    water.time_s = water.time_s.clamp(0.0, loaded);
}

/// Move the cursor on by the real time since the last frame while playback
/// is on, as far as the run has been worked out (`loaded`), stopping at
/// `end`: the end of the run, or of what there is of one that stopped short.
fn advance_playback(ui: &egui::Ui, editor: &mut EditorState, loaded: f64, end: f64, ready: bool) {
    let water = &mut editor.water;
    if !water.playing || !ready || end <= 0.0 {
        water.playing = false;
        water.play_clock = None;
        return;
    }
    let now = ui.input(|input| input.time);
    let step_s = water.play_clock.map_or(0.0, |last| (now - last).clamp(0.0, MAX_PLAY_STEP_S));
    water.play_clock = Some(now);
    water.time_s = (water.time_s.max(0.0) + step_s * PLAY_RATE).min(loaded);
    if water.time_s >= end {
        water.playing = false;
        water.play_clock = None;
    } else {
        ui.ctx().request_repaint();
    }
}
