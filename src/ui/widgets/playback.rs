//! Pieces of a playback deck floating over the viewport: the tile it sits in,
//! and the transport buttons (from [`super::transport`]). Shared by the blast
//! timeline and the field animation deck.

pub(crate) use super::transport::{Transport, transport_button};
use crate::ui::widgets::{menu, toolbar::GROUP_CORNER_RADIUS};

/// Gap between a floating tile and the canvas edge, and between tiles.
pub(crate) const TILE_MARGIN: f32 = 12.0;

/// The frame of a tile floating over the viewport.
pub(crate) fn tile_frame(visuals: &egui::Visuals) -> egui::Frame {
    egui::Frame::new()
        .fill(menu::menu_surface(visuals))
        .stroke(menu::menu_border(visuals))
        .corner_radius(egui::CornerRadius::same(GROUP_CORNER_RADIUS))
        .inner_margin(egui::Margin::symmetric(10, 8))
}
