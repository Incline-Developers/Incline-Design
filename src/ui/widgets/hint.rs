//! The hint shown while hovering a widget, in place of egui's tooltip.
//!
//! egui's tooltip is a popup frame: the window's own fill, menu margins and
//! the body font's full line height, so a few lines of help stand tall and
//! read as one more panel. A hint is a darker card that sits close around its
//! text, with lines and paragraphs packed tighter than in the panels.
//!
//! When and where it opens are egui's (`egui::Tooltip`): only the card is
//! ours. Use the [`Hint`] methods on a [`egui::Response`] wherever egui's
//! `on_hover_*` would go; Clippy refuses the stock ones (`clippy.toml`).

use egui::{Response, Ui, WidgetText};

use super::toolbar::GROUP_CORNER_RADIUS;

/// The card's padding: a little wider than tall, as text is.
const MARGIN: egui::Margin = egui::Margin::symmetric(7, 3);
/// Text size, a step under the panels' body text.
const TEXT_SIZE: f32 = 12.0;
/// Line pitch: tighter than Noto Sans' own, which leaves room for accents a
/// short note never has.
const LINE_HEIGHT: f32 = 15.0;
/// The gap a blank line between paragraphs leaves, in place of a whole line.
const PARAGRAPH_GAP: f32 = 4.0;

/// Hover hints on a response, mirroring egui's `on_hover_*` family.
pub(crate) trait Hint: Sized {
    /// `text` while the pointer rests on the enabled widget.
    fn hint(self, text: impl Into<WidgetText>) -> Self;
    /// `text` while the pointer rests on the widget when it is disabled,
    /// usually to say why.
    fn disabled_hint(self, text: impl Into<WidgetText>) -> Self;
    /// `text` beside the pointer rather than below the widget, for a large
    /// widget whose hint depends on where in it the pointer is.
    fn hint_at_pointer(self, text: impl Into<WidgetText>) -> Self;
    /// Any contents on the hint's card, while hovering the enabled widget.
    fn hint_ui(self, content: impl FnOnce(&mut Ui)) -> Self;
    /// [`Hint::hint_ui`] beside the pointer.
    fn hint_ui_at_pointer(self, content: impl FnOnce(&mut Ui)) -> Self;
}

impl Hint for Response {
    fn hint(self, text: impl Into<WidgetText>) -> Self {
        show(egui::Tooltip::for_enabled(&self), |ui| hint_text(ui, text.into()));
        self
    }

    fn disabled_hint(self, text: impl Into<WidgetText>) -> Self {
        show(egui::Tooltip::for_disabled(&self), |ui| hint_text(ui, text.into()));
        self
    }

    fn hint_at_pointer(self, text: impl Into<WidgetText>) -> Self {
        show(egui::Tooltip::for_enabled(&self).at_pointer(), |ui| hint_text(ui, text.into()));
        self
    }

    fn hint_ui(self, content: impl FnOnce(&mut Ui)) -> Self {
        show(egui::Tooltip::for_enabled(&self), content);
        self
    }

    fn hint_ui_at_pointer(self, content: impl FnOnce(&mut Ui)) -> Self {
        show(egui::Tooltip::for_enabled(&self).at_pointer(), content);
        self
    }
}

/// The card's fill: a step below the panel it opens over, short of the gap
/// between regions, so it reads as lying on the panel rather than as a hole.
pub(crate) fn fill(visuals: &egui::Visuals) -> egui::Color32 {
    super::shifted(visuals.panel_fill, if visuals.dark_mode { -9 } else { -12 })
}

fn show(mut tooltip: egui::Tooltip<'_>, content: impl FnOnce(&mut Ui)) {
    let visuals = tooltip.popup.ctx().global_style().visuals.clone();
    let frame = egui::Frame::new()
        .inner_margin(MARGIN)
        .corner_radius(GROUP_CORNER_RADIUS)
        .fill(fill(&visuals))
        .stroke(crate::ui::chrome::region_stroke(&visuals));
    tooltip.popup = tooltip.popup.frame(frame);
    tooltip.show(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        content(ui);
    });
}

/// Plain text goes in at the hint's size and pitch, a paragraph at a time;
/// text that brings its own styling is shown as it is.
fn hint_text(ui: &mut Ui, text: WidgetText) {
    let WidgetText::Text(text) = text else {
        ui.label(text);
        return;
    };
    for (index, paragraph) in text.split("\n\n").enumerate() {
        if index > 0 {
            ui.add_space(PARAGRAPH_GAP);
        }
        ui.label(egui::RichText::new(paragraph).size(TEXT_SIZE).line_height(Some(LINE_HEIGHT)));
    }
}
