//! One of a few options laid side by side across a sunken track, the chosen
//! one on the raised tile Planning's page tabs use, sliding across to the
//! next one picked: for a choice small enough to show every option at once.

use std::hash::Hash;

use super::{
    shifted,
    tab_strip::{paint_tile, sliding_tile},
    toolbar::GROUP_CORNER_RADIUS,
};

pub(crate) struct SlideChoice<'a, H> {
    id_salt: H,
    labels: &'a [String],
    selected: usize,
}

impl<'a, H: Hash + std::fmt::Debug> SlideChoice<'a, H> {
    pub(crate) fn new(id_salt: H, labels: &'a [String], selected: usize) -> Self {
        Self { id_salt, labels, selected }
    }

    /// Lay the options out across `rect`, each an equal share of it, and
    /// return the one clicked this frame.
    pub(crate) fn show(self, ui: &mut egui::Ui, rect: egui::Rect) -> Option<usize> {
        if self.labels.is_empty() || !rect.is_positive() {
            return None;
        }
        let id = ui.make_persistent_id(&self.id_salt);
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        painter.rect(
            rect,
            GROUP_CORNER_RADIUS,
            shifted(visuals.panel_fill, if visuals.dark_mode { -8 } else { -10 }),
            visuals.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let width = rect.width() / self.labels.len() as f32;
        let slot = |index: usize| egui::Rect::from_min_size(egui::pos2(rect.left() + width * index as f32, rect.top()), egui::vec2(width, rect.height()));
        let mut clicked = None;
        let mut hovered = None;
        for index in 0..self.labels.len() {
            let response = ui.interact(slot(index), id.with(index), egui::Sense::click());
            if response.hovered() {
                hovered = Some(index);
            }
            if response.clicked() && index != self.selected {
                clicked = Some(index);
            }
        }
        let chosen = clicked.unwrap_or(self.selected).min(self.labels.len() - 1);
        paint_tile(&painter, &visuals, sliding_tile(ui.ctx(), id, slot(chosen).shrink(2.0)));
        let font = egui::TextStyle::Button.resolve(ui.style());
        for (index, label) in self.labels.iter().enumerate() {
            let color = if index == chosen || hovered == Some(index) {
                visuals.strong_text_color()
            } else {
                visuals.text_color()
            };
            let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), font.clone(), color);
            job.wrap.max_width = (width - 8.0).max(1.0);
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let galley = ui.painter().layout_job(job);
            painter.galley(slot(index).center() - galley.size() / 2.0, galley, color);
        }
        clicked
    }
}
