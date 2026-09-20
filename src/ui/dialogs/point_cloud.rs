//! Point-cloud dialogs: joining tiled clouds into one.

use crate::{
    i18n::{tr, tr_format},
    ui::{
        state::{EditorState, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldText},
    },
};

/// Height the cloud checklist is allowed before it starts scrolling. A survey
/// delivery can be dozens of tiles; the dialog stays a dialog regardless.
const JOIN_LIST_MAX_HEIGHT: f32 = 220.0;

/// Join several loaded point clouds into one.
///
/// Tiled deliveries arrive as one file per tile, and the tools that consume a
/// cloud - terrain reconstruction above all - take a single cloud, so the
/// tiles have to be merged before they are of any use.
pub(crate) fn draw_point_cloud_join_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.point_cloud_join_open {
        return;
    }

    let loaded: Vec<_> = project.point_clouds.iter().filter(|cloud| cloud.is_loaded).collect();
    let mut open = true;
    DragableMenu::new("point_cloud_join_dialog", tr!(literal = "Join Point Clouds"))
        .open(&mut open)
        .min_width(360.0)
        .show(ui.ctx(), |ui| {
            menu::menu_note(
                ui,
                tr!(literal = "Combine the selected point clouds into one new cloud, so a single \
                 triangulation can be built across all of them. Per-point colours are kept; a \
                 cloud without them contributes its display colour."),
            );
            ui.add_space(4.0);

            // Clouds unloaded or removed since the dialog opened are no longer
            // joinable, so they leave the source list rather than block the run.
            editor.point_cloud_join_sources.retain(|id| loaded.iter().any(|cloud| cloud.id == *id));
            let selected: Vec<_> = loaded.iter().filter(|cloud| editor.point_cloud_join_sources.contains(&cloud.id)).collect();
            let total: usize = selected.iter().map(|cloud| cloud.point_count).sum();

            // The clouds come from the selection the dialog opened with, so
            // this states the set rather than offering one to tick through.
            MenuField::new(tr!(literal = "Point clouds"))
                .help_text(tr!(literal = "The selected clouds, copied into the joined cloud. Close the dialog to \
                     join a different set."))
                .show(ui, |ui, _row_height, _field_width| {
                    ui.label(tr_format!(literal = "%count% selected · %points% points", count = selected.len(), points = total));
                });
            egui::ScrollArea::vertical()
                .id_salt("point_cloud_join_list")
                .max_height(JOIN_LIST_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for cloud in &selected {
                        ui.weak(tr_format!(literal = "%name% (%count% points)", name = &cloud.name, count = cloud.point_count));
                    }
                });

            ui.add_space(4.0);
            ui.separator();

            MenuFieldText::new(tr!(literal = "Output name"), &mut editor.point_cloud_join_name_input)
                .help_text(tr!(literal = "Name assigned to the joined point cloud."))
                .width(220.0)
                .hint_text(tr!(literal = "Joined Cloud"))
                .show(ui);
            MenuFieldBool::new(tr!(literal = "Remove sources"), &mut editor.point_cloud_join_remove_sources)
                .help_text(tr!(literal = "Delete the selected clouds from the project once the join completes, \
                     freeing the memory their duplicate copy would otherwise hold."))
                .show(ui);

            menu::menu_actions(ui, |ui| {
                let can_run = selected.len() >= 2 && !editor.point_cloud_join_name_input.trim().is_empty();
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!(literal = "Join")).primary().enabled(can_run)).clicked() || (confirm && can_run) {
                    commands.push(UiCommand::ExecutePointCloudJoin {
                        cloud_ids: selected.iter().map(|cloud| cloud.id).collect(),
                        name: editor.point_cloud_join_name_input.trim().to_owned(),
                        remove_sources: editor.point_cloud_join_remove_sources,
                    });
                    editor.point_cloud_join_open = false;
                }
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.point_cloud_join_open = false;
                }
            });
        });
    if !open {
        editor.point_cloud_join_open = false;
    }
}
