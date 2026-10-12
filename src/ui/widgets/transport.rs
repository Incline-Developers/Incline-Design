//! Play, Pause and Rewind: small square buttons with their glyphs painted,
//! shared by the blast review's timeline and Animate's playback.

use crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS;

#[derive(Clone, Copy)]
pub(crate) enum Transport {
    Play,
    Pause,
    Rewind,
}

/// A small square transport button with its glyph painted rather than set
/// in type, so it does not depend on the UI font carrying media symbols.
pub(crate) fn transport_button(ui: &mut egui::Ui, kind: Transport, tooltip: String) -> egui::Response {
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
