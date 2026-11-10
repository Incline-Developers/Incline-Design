//! Planning's page tabs: Setup, then each view of what it produces, in a row
//! of tile tabs across the window under the viewport bar, over every Planning
//! page alike. See [`crate::ui::widgets::tab_strip`].

use crate::ui::{
    EditorState, chrome,
    state::{PlanningView, UiCommand},
    widgets::tab_strip::{TileTabs, TileTabsPaint},
};

/// The id of the row's panel.
const PANEL_ID: &str = "planning_tabs";
/// Height the row claims, which the open tab's tile fills: the regions above
/// and below keep their own gap's width clear of it, the same seam as between
/// any two regions.
const ROW_HEIGHT: f32 = 22.0;

/// The row as drawn this frame: the rect it claimed, and what to paint once
/// the regions around it have been rounded off.
pub(crate) struct PlanningTabs {
    pub(crate) rect: egui::Rect,
    paint: TileTabsPaint,
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
    // is, with only the open tab's tile raised off it.
    let response = egui::Panel::top(PANEL_ID)
        .resizable(false)
        .show_separator_line(false)
        .exact_size(ROW_HEIGHT)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let rect = ui.max_rect();
            // From the regions' own left edge, inset from the panel by the gap.
            let row = egui::Rect::from_min_max(
                egui::pos2(rect.left() + chrome::margin(), rect.top()),
                egui::pos2(rect.right() - chrome::margin(), rect.bottom()),
            );
            let (clicked, paint) = TileTabs::new("planning_views", &labels, selected).show(ui, row);
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
