//! The button that pops a docked panel out into a window of its own
//! (`app/panel_windows.rs`), in the panel's corner. Closing that window docks
//! it again.
//!
//! A box with an arrow leaving it, painted rather than drawn from an icon so
//! it stays sharp at any size. Not in the browser, which has one window.

use super::hint::Hint;

/// Side of the button's tile.
const BUTTON_SIZE: f32 = 18.0;
/// Space between the tile and the corner of the region it sits in.
const CORNER_INSET: f32 = 4.0;
/// Side of the box drawn in the tile.
const GLYPH_SIZE: f32 = 9.0;
/// Length of each stroke of the arrow's head.
const ARROW_HEAD: f32 = 3.5;

/// A pop-out button floating in the top-right corner of `region`, over what
/// the panel draws there. Returns whether it was pressed.
pub(crate) fn corner_button(ctx: &egui::Context, id_salt: &str, region: egui::Rect, hint: impl Into<egui::WidgetText>) -> bool {
    if cfg!(target_arch = "wasm32") || !region.is_positive() {
        return false;
    }
    let pos = egui::pos2(region.right() - CORNER_INSET - BUTTON_SIZE, region.top() + CORNER_INSET);
    egui::Area::new(egui::Id::unique(("pop_out_button", id_salt)))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(BUTTON_SIZE), egui::Sense::hover());
            button(ui, rect, hint).clicked()
        })
        .inner
}

/// The button itself, at `rect`.
fn button(ui: &mut egui::Ui, rect: egui::Rect, hint: impl Into<egui::WidgetText>) -> egui::Response {
    let id = ui.scope_id().with(("pop_out", rect.min.x.to_bits(), rect.min.y.to_bits()));
    let response = ui.interact(rect, id, egui::Sense::click());
    // Only the glyph answers the pointer, brightening; the tile stays bare.
    let visuals = ui.visuals();
    let color = if response.hovered() || response.is_pointer_button_down_on() {
        visuals.strong_text_color()
    } else {
        visuals.weak_text_color()
    };
    let painter = ui.painter();
    paint_glyph(painter, rect.center(), color);
    response.hint(hint)
}

/// A box open at its top-right corner, with an arrow through the gap.
fn paint_glyph(painter: &egui::Painter, centre: egui::Pos2, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.0, color);
    let snap = |x: f32, y: f32| egui::pos2(painter.round_to_pixel_center(x), painter.round_to_pixel_center(y));
    let half = GLYPH_SIZE / 2.0;
    let (left, top, right, bottom) = (centre.x - half, centre.y - half + 1.0, centre.x + half - 1.0, centre.y + half);
    let gap = GLYPH_SIZE * 0.45;
    painter.line(
        vec![snap(right, top + gap), snap(right, bottom), snap(left, bottom), snap(left, top), snap(right - gap, top)],
        stroke,
    );
    // The arrow runs from the box's middle to past its open corner.
    let inner = egui::pos2(centre.x - 0.5, centre.y + 0.5);
    let outer = egui::pos2(right + 2.0, top - 2.0);
    let (from, tip) = (inner, outer);
    painter.line_segment([from, tip], stroke);
    // The head: a stroke back along each axis from the tip, the arrow
    // running at 45 degrees.
    let back = (from - tip).normalized() * ARROW_HEAD * std::f32::consts::SQRT_2;
    painter.line_segment([tip, tip + egui::vec2(back.x, 0.0)], stroke);
    painter.line_segment([tip, tip + egui::vec2(0.0, back.y)], stroke);
}
