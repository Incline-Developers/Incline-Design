//! A strip of tabs for switching between sibling pages: a recessed track
//! with a raised tile under the open tab, which slides across to the next
//! one when another is picked.
//!
//! Reads as one control set into the bar it sits in, rather than a run of
//! loose labels, so it stays distinct from the workspace tabs above it.

use std::hash::Hash;

use super::{shifted, toolbar::GROUP_CORNER_RADIUS};

/// Space between the track's edge and the tile inside it.
const INSET: f32 = 2.0;
/// Space either side of a tab's label.
const PADDING: f32 = 12.0;
/// How long the tile takes to slide to a newly picked tab.
const SLIDE_SECONDS: f32 = 0.15;

pub(crate) struct TabStrip<'a, H> {
    id_salt: H,
    labels: &'a [String],
    selected: usize,
}

impl<'a, H: Hash + std::fmt::Debug> TabStrip<'a, H> {
    pub(crate) fn new(id_salt: H, labels: &'a [String], selected: usize) -> Self {
        Self { id_salt, labels, selected }
    }

    /// Draw the strip. Returns the index of the tab clicked this frame.
    pub(crate) fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let id = ui.scope_id().with(&self.id_salt);
        let font = egui::TextStyle::Button.resolve(ui.style());
        let galleys: Vec<_> = self
            .labels
            .iter()
            .map(|label| ui.painter().layout_no_wrap(label.clone(), font.clone(), egui::Color32::PLACEHOLDER))
            .collect();
        let height = ui.spacing().interact_size.y + 2.0 * INSET;
        let widths: Vec<f32> = galleys.iter().map(|galley| galley.size().x + 2.0 * PADDING).collect();
        let (track, _) = ui.allocate_exact_size(egui::vec2(widths.iter().sum::<f32>() + 2.0 * INSET, height), egui::Sense::hover());

        let mut x = track.left() + INSET;
        let segments: Vec<egui::Rect> = widths
            .iter()
            .map(|width| {
                let rect = egui::Rect::from_min_size(egui::pos2(x, track.top() + INSET), egui::vec2(*width, height - 2.0 * INSET));
                x += width;
                rect
            })
            .collect();

        let visuals = ui.visuals().clone();
        let dark = visuals.dark_mode;
        let track_fill = shifted(visuals.panel_fill, if dark { -8 } else { -10 });
        let tile_fill = shifted(visuals.panel_fill, if dark { 22 } else { 12 });
        let painter = ui.painter();
        painter.rect_filled(track, GROUP_CORNER_RADIUS, track_fill);

        // The tile slides rather than jumping, so the eye follows it to the
        // page that opened.
        if let Some(target) = segments.get(self.selected) {
            let left = ui.ctx().animate_value_with_time(id.with("tile_left"), target.left(), SLIDE_SECONDS);
            let right = ui.ctx().animate_value_with_time(id.with("tile_right"), target.right(), SLIDE_SECONDS);
            let tile = egui::Rect::from_x_y_ranges(left..=right, target.y_range());
            painter.rect(tile, GROUP_CORNER_RADIUS, tile_fill, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
        }

        let mut clicked = None;
        for (index, (segment, galley)) in segments.into_iter().zip(galleys).enumerate() {
            let chosen = index == self.selected;
            let response = ui.interact(segment, id.with(index), egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, galley.text()));
            if response.hovered() && !chosen {
                ui.painter().rect_filled(segment, GROUP_CORNER_RADIUS, shifted(track_fill, if dark { 8 } else { -8 }));
            }
            let color = if chosen || response.hovered() {
                visuals.strong_text_color()
            } else {
                visuals.text_color()
            };
            ui.painter().galley(segment.center() - galley.size() * 0.5, galley, color);
            if response.clicked() {
                clicked = Some(index);
            }
        }
        clicked
    }
}
