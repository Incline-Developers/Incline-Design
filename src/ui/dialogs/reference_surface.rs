//! The build surface dialog: the points, controls and optional extent
//! selected when it was opened, gridded into one new surface.

use crate::{
    i18n::tr,
    ui::{
        state::{EditorState, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuButton, selected_source_field},
    },
};

pub(crate) fn draw_reference_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
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
            selected_source_field(
                ui,
                tr!("context-points"),
                draft.points_label.clone(),
                tr!("reference-surface-points-surface-built-from-selected"),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(
                ui,
                tr!("reference-surface-controls"),
                draft.controls_label.clone(),
                tr!("reference-surface-selected-open-strings-surface-made"),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(ui, tr!("reference-surface-extent"), draft.extent_label.clone(), tr!("reference-surface-extent-help"), width);
            menu::menu_note(ui, tr!("reference-surface-grids-selected-points-plan-into"));
            ui.small(tr!("reference-surface-points-outside-extent-still-shape"));
            // Read only here: one place to change them, and the build uses
            // what the project holds.
            ui.small(project.modelling.summary());
            ui.small(tr!("reference-surface-change-these-in-preferences"));
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary()).clicked() || confirm {
                    build = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
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
