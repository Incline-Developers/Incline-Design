//! A scrubber: a position along a timeline or a sequence, set by clicking or
//! dragging anywhere along a thin track. The part played so far is drawn in
//! the accent, so how far along it is reads without looking for the handle.

use std::ops::RangeInclusive;

use super::{menu::accent_fill, shifted, toolbar::GROUP_CORNER_RADIUS};

/// Height of the track the handle runs along.
const TRACK_HEIGHT: f32 = 4.0;
/// The handle: a rounded upright tile, wider while it is held or hovered.
const HANDLE_WIDTH: f32 = 8.0;
const HANDLE_HOT_WIDTH: f32 = 10.0;

pub(crate) struct Scrubber<'a> {
    value: &'a mut f64,
    range: RangeInclusive<f64>,
    width: Option<f32>,
}

impl<'a> Scrubber<'a> {
    pub(crate) fn new(value: &'a mut f64, range: RangeInclusive<f64>) -> Self {
        Self { value, range, width: None }
    }

    /// How wide the track is; the rest of the row by default.
    pub(crate) fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl egui::Widget for Scrubber<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self { value, range, width } = self;
        let height = ui.spacing().interact_size.y;
        let width = width.unwrap_or_else(|| ui.available_width());
        let (rect, mut response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
        let (low, high) = (*range.start(), *range.end());
        let span = (high - low).max(f64::EPSILON);
        // Inset by half a handle, so the handle stays whole at either end.
        let track_x = (rect.left() + HANDLE_HOT_WIDTH / 2.0)..=(rect.right() - HANDLE_HOT_WIDTH / 2.0);
        let track_width = (track_x.end() - track_x.start()).max(1.0);

        if (response.is_pointer_button_down_on() || response.dragged())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let t = f64::from(((pointer.x - track_x.start()) / track_width).clamp(0.0, 1.0));
            let next = low + t * span;
            if next != *value {
                *value = next;
                response.mark_changed();
            }
        }
        response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *value, ""));
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let visuals = ui.visuals();
        let fade = |color: egui::Color32| if ui.is_enabled() { color } else { color.gamma_multiply(0.4) };
        let t = ((*value - low) / span).clamp(0.0, 1.0) as f32;
        let x = egui::lerp(track_x.clone(), t);
        let track = egui::Rect::from_x_y_ranges(track_x.clone(), egui::Rangef::point(rect.center().y).expand(TRACK_HEIGHT / 2.0));
        let painter = ui.painter();
        painter.rect_filled(track, GROUP_CORNER_RADIUS, fade(shifted(visuals.panel_fill, if visuals.dark_mode { 24 } else { -24 })));
        painter.rect_filled(
            egui::Rect::from_x_y_ranges(*track_x.start()..=x, track.y_range()),
            GROUP_CORNER_RADIUS,
            fade(accent_fill(visuals)),
        );
        let hot = ui.is_enabled() && (response.hovered() || response.dragged());
        let handle_width = if hot { HANDLE_HOT_WIDTH } else { HANDLE_WIDTH };
        let handle = egui::Rect::from_center_size(egui::pos2(x, rect.center().y), egui::vec2(handle_width, height * 0.7));
        let handle_fill = if visuals.dark_mode { egui::Color32::from_gray(225) } else { egui::Color32::WHITE };
        let stroke = if hot {
            egui::Stroke::new(1.5, accent_fill(visuals))
        } else {
            visuals.widgets.noninteractive.bg_stroke
        };
        painter.rect(handle, GROUP_CORNER_RADIUS, fade(handle_fill), stroke, egui::StrokeKind::Inside);
        response
    }
}
