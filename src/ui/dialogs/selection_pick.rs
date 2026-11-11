//! The prompt shown while a tool's Pick has the viewport: what to select, how
//! much is selected, and Done, Clear and Cancel.

use crate::{
    i18n::tr,
    ui::{
        dialogs::triangulation::PICK_PROMPT_WIDTH,
        state::{EditorState, SelectionCounts, SelectionPickTool, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton},
    },
};

/// The prompt's words for `tool`, its count, and whether Done can run.
fn pick_status(tool: SelectionPickTool, counts: SelectionCounts) -> (String, String, bool) {
    // What the tool will take from the selection, which may hold other
    // things besides: the count is of those alone.
    match tool {
        SelectionPickTool::SetAxis => (tr!("pick-selection-objects-prompt"), tr!("ui-selected-objects", count = counts.objects.to_string()), true),
        SelectionPickTool::InsertAtElevation => (
            tr!("pick-selection-polylines-prompt"),
            tr!("ui-selected-polylines", count = counts.polylines.to_string()),
            true,
        ),
        // Nothing crosses with fewer than two.
        SelectionPickTool::InsertAtIntersection => (
            tr!("pick-selection-crossing-prompt"),
            tr!("ui-selected-polylines", count = counts.polylines.to_string()),
            counts.polylines >= 2,
        ),
        SelectionPickTool::CreateTriangulation => (
            tr!("pick-selection-edges-prompt"),
            tr!("ui-selected-objects", count = counts.triangulation_sources.to_string()),
            true,
        ),
        SelectionPickTool::ReferencePoints => (
            tr!("pick-selection-holes-prompt"),
            tr!("pick-selection-holes-selected", count = counts.reference_holes.to_string()),
            true,
        ),
        SelectionPickTool::ReferenceSurface => (
            tr!("pick-selection-surface-prompt"),
            tr!("pick-selection-points-selected", count = counts.surface_points.to_string()),
            true,
        ),
        // None picked compares every loaded hole.
        SelectionPickTool::HoleMisfit => (
            tr!("pick-selection-misfit-prompt"),
            tr!(
                "pick-selection-holes-points-selected",
                holes = counts.reference_holes.to_string(),
                points = counts.quality_points.to_string()
            ),
            true,
        ),
        SelectionPickTool::MaskPrismoids => (
            tr!("pick-selection-prismoids-prompt"),
            tr!("pick-selection-prismoids-selected", count = counts.prismoid_cells.to_string()),
            counts.prismoid_cells >= 1,
        ),
        // None picked measures at every loaded hole.
        SelectionPickTool::ThicknessPoints => (
            tr!("pick-selection-thickness-holes-prompt"),
            tr!("pick-selection-holes-selected", count = counts.reference_holes.to_string()),
            true,
        ),
        SelectionPickTool::InsertAtSpacing => (
            tr!("pick-selection-polylines-prompt"),
            tr!("ui-selected-polylines", count = counts.polylines.to_string()),
            true,
        ),
        SelectionPickTool::Thin => (tr!("pick-selection-thin-prompt"), tr!("ui-selected-polylines", count = counts.polylines.to_string()), true),
        SelectionPickTool::Smooth => (
            tr!("pick-selection-smooth-prompt"),
            tr!("ui-selected-polylines", count = counts.polylines.to_string()),
            true,
        ),
        // Without a dialog, Done runs the tool, so it waits for input.
        SelectionPickTool::Reverse => (
            tr!("pick-selection-reverse-prompt"),
            tr!("ui-selected-polylines", count = counts.polylines.to_string()),
            counts.polylines >= 1,
        ),
        SelectionPickTool::CleanStrings => (
            tr!("pick-selection-clean-prompt"),
            tr!("ui-selected-polylines", count = counts.open_strings.to_string()),
            counts.open_strings >= 1,
        ),
    }
}

pub(crate) fn draw_selection_pick_prompt(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) {
    let Some(pick) = editor.selection_pick.as_ref() else {
        return;
    };
    let (prompt, status, ready) = pick_status(pick.tool, editor.selection_counts);
    let mut open = true;
    let (mut done, mut cancel) = (false, false);
    DragableMenu::new("selection_pick_prompt", tr!("tri-pick-from-view"))
        .open(&mut open)
        .min_width(PICK_PROMPT_WIDTH)
        .max_width(PICK_PROMPT_WIDTH)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui.ctx(), |ui| {
            ui.label(prompt);
            ui.label(egui::RichText::new(tr!("pick-selection-how")).weak());
            ui.add_space(6.0);
            ui.label(egui::RichText::new(status).italics());
            ui.add_space(6.0);
            let confirm = menu::dialog_confirmed(ui.ctx());
            let escape = menu::dialog_cancelled(ui.ctx());
            menu::menu_actions(ui, |ui| {
                if (ui.add(MenuButton::new(tr!("pick-selection-done")).primary().enabled(ready)).clicked() || confirm) && ready {
                    done = true;
                }
                if ui.add(MenuButton::new(tr!("common-clear"))).clicked() {
                    commands.push(UiCommand::ClearSelectionPick);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || escape {
                    cancel = true;
                }
            });
        });
    if done {
        commands.push(UiCommand::FinishSelectionPick);
    } else if cancel || !open {
        commands.push(UiCommand::CancelSelectionPick);
    }
}
