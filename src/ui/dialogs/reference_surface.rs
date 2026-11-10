//! The build surface dialog: the points, controls and optional extent
//! selected when it opened or picked since, triangulated into one new surface.

use crate::{
    i18n::tr,
    ui::{
        state::{EditorState, SelectionPickTool, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, picked_selection_field, selected_source_field},
    },
};

pub(crate) fn draw_reference_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    // Hidden while its Pick has the viewport.
    if editor.selection_pick.is_some() {
        return;
    }
    let Some(draft) = editor.reference_surface_dialog.as_mut() else {
        return;
    };
    let mut open = true;
    let mut build = false;
    // Both are read after the menu closes: `open` is borrowed by the window
    // for as long as its body runs, so neither button can clear it in place.
    let mut cancel = false;
    DragableMenu::new("reference_surface_dialog", tr!("common-build-surface"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            // One pick names all three inputs: they are told apart by kind.
            if picked_selection_field(ui, tr!("context-points"), draft.points_label.clone(), tr!("reference-surface-points-help"), width) {
                commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::ReferenceSurface));
            }
            ui.add_space(4.0);
            selected_source_field(
                ui,
                tr!("reference-surface-controls"),
                draft.controls_label.clone(),
                tr!("reference-surface-controls-help"),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(ui, tr!("reference-surface-extent"), draft.extent_label.clone(), tr!("reference-surface-extent-help"), width);
            menu::menu_note(ui, tr!("reference-surface-triangulates-selected-points-plan-in"));
            ui.small(tr!("reference-surface-points-outside-extent-still-shape"));
            if let Some(problem) = &draft.problem {
                ui.colored_label(egui::Color32::from_rgb(200, 70, 70), problem);
            }
            let ready = draft.problem.is_none();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(ready)).clicked() || confirm) && ready {
                    build = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if build {
        commands.push(UiCommand::BuildReferenceSurface {
            points: draft.points.clone(),
            controls: draft.controls.clone(),
            extent: draft.extent,
        });
        open = false;
    }
    if !open || cancel {
        editor.reference_surface_dialog = None;
    }
}
