//! A panel popped out of the main window into a window of its own
//! (`app/panel_windows.rs`): the console or the section view's preview.
//!
//! Dressed as the main window is: a bar across the top that is the window's
//! title bar too, naming the panel, and the panel under it with the same gap
//! round it. Closing the window puts the panel back where it was docked.

use crate::ui::{
    chrome,
    elements::{main_menu, title_bar},
    state::{EditorState, PanelKind, UiCommand},
};

/// What a popped-out panel may draw from, beyond the editor.
pub(crate) struct PanelWindowContext<'a> {
    pub(crate) console: &'a crate::logging::ConsoleSnapshot,
}

/// Draw the whole of `kind`'s window.
pub(crate) fn draw_panel_window(root_ui: &mut egui::Ui, kind: PanelKind, editor: &mut EditorState, context: &PanelWindowContext<'_>, commands: &mut Vec<UiCommand>) {
    let window_background = root_ui.painter().add(egui::Shape::Noop);
    title_bar::draw_resize_edges(root_ui.ctx(), editor.panel_frames.get_mut(kind), commands);
    title_bar::draw_window_border(root_ui.ctx(), editor.panel_frames.get_mut(kind));

    let frame = main_menu::title_bar_frame(root_ui);
    egui::Panel::top("panel_window_bar").show_separator_line(false).frame(frame).show(root_ui, |ui| {
        let inner = ui.max_rect();
        let bar = inner + frame.inner_margin;
        let row = egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), ui.spacing().interact_size.y));
        title_bar::interact_background(ui, bar, commands);
        let window_frame = editor.panel_frames.get_mut(kind);
        let controls_left = title_bar::draw_window_controls(ui, bar, row, window_frame, commands);
        window_frame.buttons_centre_y = row.center().y;
        let leading = window_frame.leading_inset.max(inner.left());
        let span = egui::Rangef::new(leading, controls_left - main_menu::WINDOW_BUTTONS_GAP);
        title_bar::draw_title_text(ui, bar, span, row.center().y + main_menu::LABEL_DROP, kind.title());
        ui.allocate_rect(row, egui::Sense::hover());
    });

    // The panel under the bar, with the gap round it that the main window's
    // regions have.
    chrome::claim_gap(root_ui, "panel_window_edge", chrome::Gap::Sides);
    egui::Panel::bottom("panel_window_bottom_gap")
        .exact_size(chrome::margin())
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
        .show(root_ui, |_| {});
    let (regions, grips) = match kind {
        PanelKind::Console => (vec![crate::ui::elements::console::draw_console_window(root_ui, context.console)], Vec::new()),
        PanelKind::SectionPreview => (vec![crate::ui::widgets::viewport::draw_section_preview_window(root_ui, editor, commands)], Vec::new()),
    };

    let ctx = root_ui.ctx();
    chrome::paint_window_background(ctx, window_background, egui::Rect::ZERO);
    let content = ctx.content_rect();
    let below = egui::Rect::from_min_max(egui::pos2(content.left(), content.bottom()), content.max);
    chrome::paint_regions(ctx, regions, below);
    chrome::paint_grips(ctx, grips);
}
