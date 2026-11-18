//! Planning's page tabs: Setup, then each view of what it produces, in a row
//! of tile tabs across the window under the viewport bar, over every Planning
//! page alike. See [`crate::ui::widgets::tab_strip`].
//!
//! A workspace with water runs open gets the same row, for as long as it has
//! one: [`draw_water_tabs`].

use crate::ui::{
    EditorState, chrome,
    state::{PlanningView, UiCommand},
    widgets::tab_strip::{TileTabs, TileTabsPaint},
};

/// The id of the row's panel.
const PANEL_ID: &str = "planning_tabs";
const WATER_PANEL_ID: &str = "water_tabs";
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
            let tabs = TileTabs::new("planning_views", &labels, selected).show(ui, row);
            if let Some(index) = tabs.clicked {
                commands.push(UiCommand::SetPlanningView(PlanningView::ALL[index]));
            }
            tabs.paint
        });
    PlanningTabs {
        rect: response.response.rect,
        paint: response.inner,
    }
}

/// The same row for a workspace with water runs open: the workspace's own
/// tab first, then one for each run, which its cross closes. The row is only
/// there while the workspace has a run, and scrolls sideways once the tabs
/// outgrow the window.
pub(crate) fn draw_water_tabs(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) -> PlanningTabs {
    let workspace = editor.active_workspace;
    let runs: Vec<_> = editor.water.tabs.iter().filter(|tab| tab.workspace == workspace).cloned().collect();
    let labels: Vec<String> = std::iter::once(workspace.label()).chain(runs.iter().map(|run| run.title.clone())).collect();
    let closable: Vec<bool> = (0..labels.len()).map(|index| index > 0).collect();
    let selected = editor.water.open.and_then(|id| runs.iter().position(|run| run.id == id)).map_or(0, |index| index + 1);
    let response = egui::Panel::top(WATER_PANEL_ID)
        .resizable(false)
        .show_separator_line(false)
        .exact_size(ROW_HEIGHT)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let mut drawn = None;
            // Inset to the regions' own left and right edges.
            let inset = egui::Rect::from_min_max(ui.max_rect().min + egui::vec2(chrome::margin(), 0.0), ui.max_rect().max - egui::vec2(chrome::margin(), 0.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inset), |ui| {
                super::bar_strip(ui, "water_tabs_strip", ROW_HEIGHT, |ui, row| {
                    let tabs = TileTabs::new("water_tabs", &labels, selected).closable(&closable).fitted().show(ui, row);
                    let width = tabs.width;
                    drawn = Some(tabs);
                    width
                });
            });
            drawn
        });
    let rect = response.response.rect;
    let Some(tabs) = response.inner else {
        // The strip always draws its contents; this is only what an empty
        // row would be.
        let empty: [String; 0] = [];
        return PlanningTabs {
            rect,
            paint: TileTabs::new("water_tabs_empty", &empty, 0).show(ui, egui::Rect::NOTHING).paint,
        };
    };
    if let Some(index) = tabs.closed {
        commands.push(UiCommand::CloseWaterRun(runs[index - 1].id));
    } else if let Some(index) = tabs.clicked {
        // Either way the viewport changes what it shows, so what was picked
        // up or selected in the other is put down.
        editor.water.open = index.checked_sub(1).map(|run| runs[run].id);
        editor.water.playing = false;
        commands.push(UiCommand::ClearSelection);
        commands.push(UiCommand::SetActiveTool(crate::ui::state::ActiveTool::None));
    }
    PlanningTabs { rect, paint: tabs.paint }
}
