//! The raised tile under an open tab, which slides across to the next one
//! when another is picked: under the workspace tabs in the menu bar, and the
//! slide behind Planning's page tabs (`widgets::page_tabs`).

use super::{shifted, toolbar::GROUP_CORNER_RADIUS};

/// How long the tile takes to slide to a newly picked tab.
const SLIDE_SECONDS: f32 = 0.15;

/// The raised tile under the open tab, at `rect`.
pub(crate) fn paint_tile(painter: &egui::Painter, visuals: &egui::Visuals, rect: egui::Rect) {
    let fill = shifted(visuals.panel_fill, if visuals.dark_mode { 22 } else { 12 });
    painter.rect(rect, GROUP_CORNER_RADIUS, fill, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
}

/// Where the tile is this frame, sliding towards `target`, the open tab.
pub(crate) fn sliding_tile(ctx: &egui::Context, id: egui::Id, target: egui::Rect) -> egui::Rect {
    let left = ctx.animate_value_with_time(id.with("tile_left"), target.left(), SLIDE_SECONDS);
    let right = ctx.animate_value_with_time(id.with("tile_right"), target.right(), SLIDE_SECONDS);
    egui::Rect::from_x_y_ranges(left..=right, target.y_range())
}
