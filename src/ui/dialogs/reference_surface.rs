//! The build surface dialog: the points and optional extent selected when it
//! was opened, triangulated into one new surface.

use crate::{
    i18n::tr,
    ui::{
        state::{EditorState, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, selected_source_field},
    },
};

pub(crate) fn draw_reference_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(draft) = editor.reference_surface_dialog.as_mut() else {
        return;
    };
    let mut open = true;
    let mut build = false;
    // Both are read after the menu closes: `open` is borrowed by the window
    // for as long as its body runs, so neither button can clear it in place.
    let mut cancel = false;
    DragableMenu::new("reference_surface_dialog", tr!(literal = "Build Surface"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            selected_source_field(
                ui,
                tr!(literal = "Points"),
                draft.points_label.clone(),
                tr!(literal = "The points the surface is built from, as selected when the dialog opened. Close the dialog to select different ones."),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(
                ui,
                tr!(literal = "Extent"),
                draft.extent_label.clone(),
                tr!(literal = "The selected closed string the finished surface is clipped to; points outside it still shape the surface."),
                width,
            );
            menu::menu_note(ui, tr!(literal = "Triangulates the selected points in plan into a new surface. Each build adds a surface."));
            ui.small(tr!(literal = "Points outside the extent still shape the surface; only the surface is clipped to it."));
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!(literal = "Make")).primary()).clicked() || confirm {
                    build = true;
                }
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if build {
        commands.push(UiCommand::BuildReferenceSurface {
            points: draft.points.clone(),
            extent: draft.extent,
        });
        open = false;
    }
    if !open || cancel {
        editor.reference_surface_dialog = None;
    }
}
