//! Planning's page tabs: Setup, then each view of what it produces, in a row
//! of browser-style tabs across the window under the viewport bar, over every
//! Planning page alike. See [`crate::ui::widgets::page_tabs`].

use crate::ui::{
    EditorState, chrome,
    state::{PlanningView, UiCommand},
    widgets::page_tabs::{PageTabs, PageTabsPaint},
};

/// The id of the row's panel.
const PANEL_ID: &str = "planning_tabs";
/// Height the row claims. The open tab stands this tall: it starts a gap's
/// width below the top, leaving the same seam to the viewport bar as between
/// any two regions, and reaches a gap's width past the bottom to the regions
/// under it.
const ROW_HEIGHT: f32 = 24.0;

/// The row as drawn this frame: the rect it claimed, and what to paint once
/// the regions under it have been rounded off.
pub(crate) struct PlanningTabs {
    pub(crate) rect: egui::Rect,
    paint: PageTabsPaint,
}

impl PlanningTabs {
    /// Paint the tabs. Call after `chrome::paint_regions`.
    pub(crate) fn paint(&self, ctx: &egui::Context) {
        self.paint.paint(ctx);
    }
}

/// Claim the row across the top of the workspace and lay the tabs out in it.
pub(crate) fn draw_planning_tabs(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) -> PlanningTabs {
    let labels = PlanningView::ALL.map(PlanningView::label);
    let selected = PlanningView::ALL.iter().position(|view| *view == editor.planning_view).unwrap_or(0);
    // Not a region: the row is the window behind the regions, as the menu bar
    // is, with only the open tab standing up out of it.
    let response = egui::Panel::top(PANEL_ID)
        .resizable(false)
        .show_separator_line(false)
        .exact_size(ROW_HEIGHT)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let rect = ui.max_rect();
            let row = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + chrome::margin()), rect.max);
            let (clicked, paint) = PageTabs::new("planning_views", &labels, selected).show(ui, row, rect.bottom() + chrome::margin());
            if let Some(index) = clicked {
                commands.push(UiCommand::SetPlanningView(PlanningView::ALL[index]));
            }
            paint
        });
    PlanningTabs {
        rect: response.response.rect,
        paint: response.inner,
    }
}
