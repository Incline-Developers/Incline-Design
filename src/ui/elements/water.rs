//! A water run's tab: the timeline under the viewport and the read-out
//! beside it.
//!
//! The run itself, and the surfaces the viewport draws for it, are the app's
//! (`app/water.rs`); what is shown here is the mirror of the open run it
//! keeps in [`crate::ui::state::WaterView`].

use crate::{
    i18n::tr,
    model::water::{BAND_DEPTHS, WaterSettings},
    ui::{
        chrome,
        state::{EditorState, UiCommand, WaterRunStatus, WaterShown},
        widgets::{
            data_table::DataTable,
            island::{Island, IslandResponse, Side},
            menu::{self, MenuButton, MenuFieldBool},
            scrubber::Scrubber,
            toolbar::GROUP_CORNER_RADIUS,
            transport::{Transport, transport_button},
        },
    },
};

const TIMELINE_PANEL_ID: &str = "water_timeline";
const PANEL_ID: &str = "water_panel";
/// Real seconds a whole run plays through in.
const PLAY_SECONDS: f64 = 20.0;
/// The most a frame moves the cursor on by, so a stalled frame does not
/// skip the run.
const MAX_PLAY_STEP_S: f64 = 0.1;

/// Seconds into a run as hours and minutes: `3:05`.
pub(crate) fn clock(time_s: f64) -> String {
    let minutes = (time_s / 60.0).round().max(0.0) as u64;
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

fn number(value: f64) -> String {
    crate::app::water::plain_number(value)
}

/// A whole number with its thousands parted by thin spaces.
fn grouped(value: f64) -> String {
    let digits = format!("{:.0}", value.abs());
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('\u{2009}');
        }
        out.push(digit);
    }
    if value < -0.5 { format!("-{out}") } else { out }
}

/// The timeline under the viewport: Play, Back to the start, the instant
/// shown and a scrubber over the whole run.
pub(crate) fn draw_timeline(ui: &mut egui::Ui, editor: &mut EditorState) -> egui::Rect {
    egui::Panel::bottom(TIMELINE_PANEL_ID)
        .resizable(false)
        .show_separator_line(false)
        .exact_size(44.0)
        .frame(chrome::region_frame(ui).inner_margin(egui::Margin::symmetric(14, 8)))
        .show(ui, |ui| {
            let duration = editor.water.shown.as_ref().map_or(0.0, |shown| shown.settings.duration_s());
            let ready = editor.water.shown.as_ref().is_some_and(|shown| shown.status == WaterRunStatus::Ready) && !editor.water.show_deepest;
            advance_playback(ui, editor, duration, ready);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_enabled_ui(ready, |ui| {
                    if editor.water.playing {
                        if transport_button(ui, Transport::Pause, None).clicked() {
                            editor.water.playing = false;
                        }
                    } else if transport_button(ui, Transport::Play, None).clicked() {
                        if editor.water.time_s >= duration {
                            editor.water.time_s = 0.0;
                        }
                        editor.water.playing = true;
                    }
                    if transport_button(ui, Transport::Rewind, None).clicked() {
                        editor.water.time_s = 0.0;
                        editor.water.playing = false;
                    }
                });
                ui.add_space(8.0);
                // The instant the viewport shows, which is the kept one at
                // or before the cursor. As wide as the longest it can be, so
                // the scrubber beside it does not move under playback.
                let shown = editor.water.shown.as_ref().and_then(|shown| shown.instant.as_ref()).map_or(0.0, |instant| instant.time_s);
                let text = if editor.water.show_deepest {
                    tr!("water-deepest")
                } else {
                    tr!("water-clock", time = clock(shown), end = clock(duration))
                };
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
                let slider = ui.add_enabled(ready, Scrubber::new(&mut editor.water.time_s, 0.0..=duration.max(f64::EPSILON)));
                // Taking hold of the cursor stops playback.
                if slider.dragged() || slider.changed() {
                    editor.water.playing = false;
                }
            });
        })
        .response
        .rect
}

/// Move the cursor on by the real time since the last frame while playback
/// is on, stopping at the end of the run.
fn advance_playback(ui: &egui::Ui, editor: &mut EditorState, duration: f64, ready: bool) {
    let water = &mut editor.water;
    if !water.playing || !ready || duration <= 0.0 {
        water.playing = false;
        water.play_clock = None;
        return;
    }
    let now = ui.input(|input| input.time);
    let step_s = water.play_clock.map_or(0.0, |last| (now - last).clamp(0.0, MAX_PLAY_STEP_S));
    water.play_clock = Some(now);
    water.time_s = (water.time_s.max(0.0) + step_s * duration / PLAY_SECONDS).min(duration);
    if water.time_s >= duration {
        water.playing = false;
        water.play_clock = None;
    } else {
        ui.ctx().request_repaint();
    }
}

/// The run's read-out, down the right edge: how it is going, what was run,
/// where the water is at the instant shown, and the ponds.
pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    Island::new(PANEL_ID, Side::Right).default_width(300.0).min_width(240.0).max_width(460.0).show(ui, |ui, _| {
        let Some(shown) = editor.water.shown.clone() else {
            return;
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            draw_status(ui, &shown, commands);
            menu::panel_section(ui, tr!("water-section-run"));
            draw_settings(ui, &shown);
            if shown.status != WaterRunStatus::Ready {
                return;
            }
            menu::panel_section(ui, tr!("water-section-view"));
            MenuFieldBool::new(tr!("water-show-deepest"), &mut editor.water.show_deepest)
                .help_text(tr!("water-show-deepest-help"))
                .show(ui);
            draw_legend(ui);
            if ui.add(MenuButton::new(tr!("water-save-surface"))).clicked() {
                commands.push(UiCommand::SaveWaterSurface(shown.id));
            }
            if let Some(instant) = shown.instant.as_ref().filter(|_| !editor.water.show_deepest) {
                menu::panel_section(ui, tr!("water-section-balance", time = clock(instant.time_s)));
                let row = |ui: &mut egui::Ui, label: String, volume: f64| {
                    super::properties::read_only_row(ui, &label, &tr!("water-cubic-metres", volume = grouped(volume)));
                };
                row(ui, tr!("water-balance-rained"), instant.rained);
                row(ui, tr!("water-balance-on-surface"), instant.on_surface);
                row(ui, tr!("water-balance-infiltrated"), instant.infiltrated);
                row(ui, tr!("water-balance-left"), instant.left);

                menu::panel_section(ui, tr!("water-section-ponds", count = instant.ponds.len().to_string()));
                if instant.ponds.is_empty() {
                    ui.label(egui::RichText::new(tr!("water-ponds-none")).color(ui.visuals().weak_text_color()));
                } else {
                    let header = [tr!("water-pond-volume"), tr!("water-pond-area"), tr!("water-pond-depth"), tr!("water-pond-level")];
                    let aligns = [egui::Align::Max; 4];
                    let ponds = &instant.ponds;
                    let cells = |row: usize| {
                        let pond = &ponds[row];
                        vec![grouped(pond.volume), grouped(pond.area), format!("{:.2}", pond.max_depth), format!("{:.2}", pond.level)]
                    };
                    DataTable::new("water_ponds", &header, ponds.len(), &cells)
                        .aligns(&aligns)
                        .fingerprint(instant.time_s.to_bits() ^ shown.id)
                        .show(ui);
                }
            }
        });
    })
}

/// How the run is going: its progress while it runs, why it failed if it
/// did, and nothing once it is ready.
fn draw_status(ui: &mut egui::Ui, shown: &WaterShown, commands: &mut Vec<UiCommand>) {
    match &shown.status {
        WaterRunStatus::Running(fraction) => {
            ui.label(tr!("water-status-running"));
            let bar = egui::ProgressBar::new(fraction.unwrap_or(0.0)).corner_radius(GROUP_CORNER_RADIUS);
            ui.add(if fraction.is_some() { bar.show_percentage() } else { bar.animate(true) });
            if ui.add(MenuButton::new(tr!("water-stop"))).clicked() {
                commands.push(UiCommand::CloseWaterRun(shown.id));
            }
        }
        WaterRunStatus::Failed(error) => {
            ui.label(egui::RichText::new(tr!("water-status-failed")).color(ui.visuals().error_fg_color));
            ui.label(error);
        }
        WaterRunStatus::Ready => {}
    }
}

fn draw_settings(ui: &mut egui::Ui, shown: &WaterShown) {
    let WaterSettings {
        rain_mm,
        rain_hours,
        run_on_hours,
        roughness,
        infiltration_mm_h,
        step_s,
        subdivide_m,
    } = shown.settings;
    let row = |ui: &mut egui::Ui, label: String, value: String| super::properties::read_only_row(ui, &label, &value);
    row(ui, tr!("water-surface"), shown.surface.clone());
    row(ui, tr!("water-rain"), tr!("water-rain-value", rain = number(rain_mm), hours = number(rain_hours)));
    row(ui, tr!("water-run-on"), tr!("water-hours", hours = number(run_on_hours)));
    row(ui, tr!("water-roughness"), format!("{roughness}"));
    row(ui, tr!("water-infiltration"), tr!("water-mm-per-hour", rate = number(infiltration_mm_h)));
    row(ui, tr!("water-step"), tr!("water-seconds", seconds = number(step_s)));
    if let Some(longest) = subdivide_m {
        row(ui, tr!("water-subdivide"), tr!("water-metres", metres = number(longest)));
    }
    row(
        ui,
        tr!("water-mesh"),
        tr!("water-mesh-value", vertices = grouped(shown.vertices as f64), faces = grouped(shown.faces as f64)),
    );
    if shown.status == WaterRunStatus::Ready {
        row(ui, tr!("water-area"), tr!("water-square-metres", area = grouped(shown.area)));
        row(ui, tr!("water-deepest-anywhere"), tr!("water-metres", metres = format!("{:.2}", shown.deepest)));
    }
}

/// The depth each colour of water stands for.
fn draw_legend(ui: &mut egui::Ui) {
    for (band, start) in BAND_DEPTHS.iter().enumerate() {
        ui.horizontal(|ui| {
            let side = ui.spacing().interact_size.y * 0.7;
            let (swatch, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
            let [r, g, b, _] = crate::app::water::band_color(band);
            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
            ui.painter().rect_filled(swatch, GROUP_CORNER_RADIUS, color);
            let text = match BAND_DEPTHS.get(band + 1) {
                Some(end) => tr!("water-band", from = number(f64::from(*start)), to = number(f64::from(*end))),
                None => tr!("water-band-over", from = number(f64::from(*start))),
            };
            ui.label(text);
        });
    }
}
