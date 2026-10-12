//! Toasts: a short message that shows above the status bar, holds for a
//! moment and fades away by itself - feedback that asks for no click.
//!
//! The app or the UI pushes one with [`Toasts::push`]; [`draw_toasts`] paints
//! them once a frame, newest at the bottom, and drops them when they have faded.

use super::toolbar::GROUP_CORNER_RADIUS;
use crate::ui::unthemed_icon;

/// How long a toast stays fully shown, then how long it takes to fade.
const HOLD_SECONDS: f32 = 3.0;
const FADE_SECONDS: f32 = 1.0;
/// Gap between the toasts and the bottom of the window (above the status bar).
const BOTTOM_MARGIN: f32 = 64.0;
const ICON_SIDE: f32 = 18.0;
const MAX_WIDTH: f32 = 520.0;

#[derive(Clone, Debug)]
pub(crate) struct Toast {
    text: String,
    shown_at: web_time::Instant,
}

/// The toasts on screen.
#[derive(Clone, Debug, Default)]
pub(crate) struct Toasts(Vec<Toast>);

impl Toasts {
    /// Show `text` as a success message.
    pub(crate) fn push(&mut self, text: impl Into<String>) {
        self.0.push(Toast {
            text: text.into(),
            shown_at: web_time::Instant::now(),
        });
    }
}

/// Paint the toasts and forget the ones that have faded.
pub(crate) fn draw_toasts(ctx: &egui::Context, toasts: &mut Toasts) {
    toasts.0.retain(|toast| toast.shown_at.elapsed().as_secs_f32() < HOLD_SECONDS + FADE_SECONDS);
    if toasts.0.is_empty() {
        return;
    }
    // Keep frames coming while one fades, so it fades smoothly and goes.
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    let screen = ctx.content_rect();
    let mut bottom = screen.bottom() - BOTTOM_MARGIN;
    for (index, toast) in toasts.0.iter().enumerate().rev() {
        let age = toast.shown_at.elapsed().as_secs_f32();
        let opacity = if age < HOLD_SECONDS {
            1.0
        } else {
            (1.0 - (age - HOLD_SECONDS) / FADE_SECONDS).clamp(0.0, 1.0)
        };
        let area = egui::Area::new(egui::Id::new(("toast", index)))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(egui::pos2(screen.center().x, bottom))
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                let visuals = ui.visuals();
                egui::Frame::new()
                    .fill(visuals.window_fill)
                    .stroke(visuals.window_stroke)
                    .corner_radius(GROUP_CORNER_RADIUS)
                    .shadow(visuals.popup_shadow)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(MAX_WIDTH);
                        ui.horizontal(|ui| {
                            ui.add(egui::Image::new(unthemed_icon!("console_success.svg")).fit_to_exact_size(egui::Vec2::splat(ICON_SIDE)));
                            ui.add(egui::Label::new(&toast.text).wrap());
                        });
                    });
            });
        bottom = area.response.rect.top() - 6.0;
    }
}
