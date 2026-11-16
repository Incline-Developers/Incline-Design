//! Play, Pause, Rewind and single steps: small square buttons with their
//! glyphs painted, shared by the blast review's timeline, Animate's playback,
//! a rain simulation's and the Gantt's Inspector; and the keys every one of
//! those timelines takes ([`timeline_keys`]).

use super::hint::Hint;
use crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS;

#[derive(Clone, Copy)]
pub(crate) enum Transport {
    Play,
    Pause,
    Rewind,
    /// One step back or forward: a chevron.
    StepBack,
    StepForward,
}

/// A small square transport button with its glyph painted rather than set
/// in type, so it does not depend on the UI font carrying media symbols.
///
/// The glyphs say what they do, so a button takes a hint only for what its
/// glyph cannot show, such as how far a step goes.
pub(crate) fn transport_button(ui: &mut egui::Ui, kind: Transport, hint: Option<String>) -> egui::Response {
    let side = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    // Play and pause are the bar's one action, so they take the accent like a
    // primary button; rewind sits beside them as a secondary one.
    let (fill, color) = if matches!(kind, Transport::Play | Transport::Pause) {
        let accent = crate::ui::widgets::menu::accent_fill(ui.visuals());
        let fill = if response.is_pointer_button_down_on() {
            crate::ui::widgets::shifted(accent, -16)
        } else if response.hovered() {
            crate::ui::widgets::shifted(accent, 16)
        } else {
            accent
        };
        (fill, egui::Color32::WHITE)
    } else {
        let visuals = ui.style().interact(&response);
        (visuals.bg_fill, visuals.fg_stroke.color)
    };
    ui.painter().rect_filled(rect, GROUP_CORNER_RADIUS, fill);
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
        Transport::StepBack | Transport::StepForward => {
            let dx = if matches!(kind, Transport::StepBack) { -r * 0.5 } else { r * 0.5 };
            let points = vec![egui::pos2(c.x - dx, c.y - r), egui::pos2(c.x + dx, c.y), egui::pos2(c.x - dx, c.y + r)];
            ui.painter().add(egui::Shape::line(points, egui::Stroke::new(1.8, color)));
        }
    }
    match hint {
        Some(hint) => response.hint(hint),
        None => response,
    }
}

/// How long an arrow is held before the cursor stops stepping and runs.
const SCRUB_HOLD_S: f64 = 0.35;

/// What a timeline's keys ask of it this frame.
#[derive(Clone, Copy, Default)]
pub(crate) struct TimelineKeys {
    /// Space: play, or pause.
    pub(crate) toggle: bool,
    /// An arrow is down or was pressed, which takes the cursor off playback.
    pub(crate) scrubbing: bool,
    /// Steps to move the cursor by, back negative: one for each arrow press,
    /// `shift_steps` for each made with Shift.
    pub(crate) steps: f64,
    /// Real seconds to run the cursor on by, back negative, once an arrow
    /// has been held: the timeline moves it at its playback speed.
    pub(crate) run_s: f64,
}

/// The keys a timeline takes while it is on screen: Space plays and pauses,
/// and the arrows step the cursor, running it once held. Nothing while
/// `enabled` is off or a field is being typed in, which keeps its keys.
///
/// `salt` names the timeline, for how long its arrow has been held.
pub(crate) fn timeline_keys(ctx: &egui::Context, salt: &'static str, enabled: bool, shift_steps: f64) -> TimelineKeys {
    let hold_id = egui::Id::unique(salt);
    if !enabled || ctx.egui_wants_keyboard_input() {
        ctx.data_mut(|data| data.remove::<f64>(hold_id));
        return TimelineKeys::default();
    }
    // Consumed, so a transport button left with focus is not clicked by the
    // same Space press. Every arrow press counts, each with its own Shift.
    let (toggle, steps, back_down, forward_down, dt) = ctx.input_mut(|input| {
        let toggle = input.consume_key(egui::Modifiers::NONE, egui::Key::Space);
        let mut steps = 0.0;
        for modifiers in [egui::Modifiers::NONE, egui::Modifiers::SHIFT] {
            let unit = if modifiers.shift { shift_steps } else { 1.0 };
            while input.consume_key(modifiers, egui::Key::ArrowLeft) {
                steps -= unit;
            }
            while input.consume_key(modifiers, egui::Key::ArrowRight) {
                steps += unit;
            }
        }
        (
            toggle,
            steps,
            input.key_down(egui::Key::ArrowLeft),
            input.key_down(egui::Key::ArrowRight),
            f64::from(input.stable_dt.min(0.1)),
        )
    });
    let direction = match (back_down, forward_down) {
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => 0.0,
    };
    if direction == 0.0 && steps == 0.0 {
        ctx.data_mut(|data| data.remove::<f64>(hold_id));
        return TimelineKeys {
            toggle,
            ..TimelineKeys::default()
        };
    }
    let held = ctx.data_mut(|data| {
        let held = data.get_temp_mut_or_default::<f64>(hold_id);
        *held += dt;
        *held
    });
    ctx.request_repaint();
    // Repeats from the OS are swallowed by the run, so a held key moves
    // smoothly.
    if direction != 0.0 && held >= SCRUB_HOLD_S {
        TimelineKeys {
            toggle,
            scrubbing: true,
            steps: 0.0,
            run_s: direction * dt,
        }
    } else {
        TimelineKeys {
            toggle,
            scrubbing: true,
            steps,
            run_s: 0.0,
        }
    }
}
