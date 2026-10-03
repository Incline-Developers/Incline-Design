//! An on/off switch with its label, for settings that take effect at once.
//!
//! A checkbox reads as "include this item"; a switch reads as "this mode is
//! on", which is what an Auto run is. It also keeps a lone setting in a
//! toolbar from looking like one more square button beside the icon buttons.

use super::toolbar::GROUP_CORNER_RADIUS;

/// Track size of the switch. The knob is the track's height less a margin.
const TRACK: egui::Vec2 = egui::vec2(26.0, 14.0);
const KNOB_MARGIN: f32 = 2.0;
const LABEL_GAP: f32 = 6.0;

pub(crate) struct Toggle<'a> {
    value: &'a mut bool,
    label: egui::WidgetText,
}

impl<'a> Toggle<'a> {
    pub(crate) fn new(value: &'a mut bool, label: impl Into<egui::WidgetText>) -> Self {
        Self { value, label: label.into() }
    }
}

impl egui::Widget for Toggle<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let galley = self.label.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body);
        let size = egui::vec2(TRACK.x + LABEL_GAP + galley.size().x, TRACK.y.max(galley.size().y));
        let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
        if response.clicked() {
            *self.value = !*self.value;
            response.mark_changed();
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *self.value, galley.text()));
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let on = ui.ctx().animate_bool_responsive(response.id, *self.value);
        let visuals = ui.visuals();
        let widget = ui.style().interact(&response);
        let track = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - TRACK.y / 2.0), TRACK);
        let off_fill = widget.bg_fill;
        let fill = off_fill.lerp_to_gamma(visuals.selection.bg_fill, on);
        let painter = ui.painter();
        // Squared off like every other control: one corner radius for the
        // whole window, so the knob is a rounded square rather than a disc.
        painter.rect(track, GROUP_CORNER_RADIUS, fill, widget.bg_stroke, egui::StrokeKind::Inside);
        let knob = egui::Vec2::splat(TRACK.y - 2.0 * KNOB_MARGIN);
        let x = egui::lerp((track.left() + TRACK.y / 2.0)..=(track.right() - TRACK.y / 2.0), on);
        painter.rect_filled(
            egui::Rect::from_center_size(egui::pos2(x, track.center().y), knob),
            GROUP_CORNER_RADIUS,
            widget.fg_stroke.color,
        );
        let text_pos = egui::pos2(track.right() + LABEL_GAP, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, widget.text_color());
        response
    }
}
