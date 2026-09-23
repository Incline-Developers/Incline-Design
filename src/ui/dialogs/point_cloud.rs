//! Point-cloud dialogs: joining tiled clouds into one, and classifying ground.

use crate::{
    i18n::{tr, tr_format},
    model::ground_filter::{ClothTerrain, estimate_classify_memory_bytes, recommended_cloth_resolution},
    ui::{
        dialogs::triangulation::format_bytes,
        state::{EditorState, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldText, MenuFieldU32},
    },
};

/// Height the cloud checklist is allowed before it starts scrolling. A survey
/// delivery can be dozens of tiles; the dialog stays a dialog regardless.
const JOIN_LIST_MAX_HEIGHT: f32 = 220.0;
/// Estimated classify memory above which the dialog warns, and above which it
/// refuses to run - the same limits as terrain TIN generation.
const CLASSIFY_WARN_BYTES: u64 = 6 * 1024 * 1024 * 1024;
const CLASSIFY_HARD_BYTES: u64 = 48 * 1024 * 1024 * 1024;

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

fn cloth_terrain_label(terrain: ClothTerrain) -> String {
    match terrain {
        ClothTerrain::Steep => tr!(literal = "Steep (pit walls, benches)"),
        ClothTerrain::Relief => tr!(literal = "Relief (dumps, rolling ground)"),
        ClothTerrain::Flat => tr!(literal = "Flat (pads, structures)"),
    }
}

/// Classify ground and noise in the selected point clouds.
///
/// A delivery that never went through a ground filter can be neither drawn by
/// class nor reduced to bare earth, so this is what makes an unclassified
/// cloud usable for terrain.
pub(crate) fn draw_point_cloud_classify_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.point_cloud_classify_open {
        return;
    }

    let loaded: Vec<_> = project.point_clouds.iter().filter(|cloud| cloud.is_loaded).collect();
    let mut open = true;
    DragableMenu::new("point_cloud_classify_dialog", tr!(literal = "Classify Point Clouds"))
        .open(&mut open)
        .min_width(380.0)
        .show(ui.ctx(), |ui| {
            menu::menu_note(
                ui,
                tr!(literal = "Mark each point as ground, noise or unclassified. A cloth is pressed up \
                 under the cloud and settles on the ground surface; points within the ground \
                 threshold of it are ground. Any existing classes are replaced; undo restores \
                 them."),
            );
            ui.add_space(4.0);

            // Clouds unloaded or removed since the dialog opened drop out
            // rather than block the run.
            editor.point_cloud_classify_sources.retain(|id| loaded.iter().any(|cloud| cloud.id == *id));
            let selected: Vec<_> = loaded.iter().filter(|cloud| editor.point_cloud_classify_sources.contains(&cloud.id)).collect();
            let total: usize = selected.iter().map(|cloud| cloud.point_count).sum();
            MenuField::new(tr!(literal = "Point clouds"))
                .help_text(tr!(literal = "The selected clouds, each classified on its own. Close the dialog to \
                     classify a different set."))
                .show(ui, |ui, _row_height, _field_width| {
                    ui.label(tr_format!(literal = "%count% selected · %points% points", count = selected.len(), points = total));
                });
            egui::ScrollArea::vertical()
                .id_salt("point_cloud_classify_list")
                .max_height(JOIN_LIST_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for cloud in &selected {
                        ui.weak(tr_format!(literal = "%name% (%count% points)", name = &cloud.name, count = cloud.point_count));
                    }
                });

            ui.add_space(4.0);
            ui.separator();

            let params = &mut editor.point_cloud_classify_params;
            let terrain_label = cloth_terrain_label(params.terrain);
            MenuFieldCombo::new(
                "point_cloud_classify_terrain",
                tr!(literal = "Terrain"),
                &mut params.terrain,
                terrain_label,
                [ClothTerrain::Steep, ClothTerrain::Relief, ClothTerrain::Flat].map(|terrain| (terrain, cloth_terrain_label(terrain).into())),
            )
            .help_text(tr!(literal = "The ground the cloud covers. Steep follows walls down from their crests; \
                 Flat uses a stiffer cloth that bridges large buildings and plant but rounds \
                 off sharp breaks."))
            .width(220.0)
            .show(ui);
            MenuFieldF64::new(tr!(literal = "Cloth resolution"), &mut params.cloth_resolution, 0.05..=50.0)
                .help_text(tr!(literal = "Spacing of the cloth's particles. Around the cloud's point spacing is a good \
                     start; finer follows the ground more closely but needs denser points."))
                .speed(0.05)
                .suffix(tr!(literal = " m"))
                .max_decimals(2)
                .width(220.0)
                .show(ui);
            if let Some(spacing) = editor.point_cloud_classify_spacing {
                let recommended = recommended_cloth_resolution(spacing);
                MenuField::new(tr!(literal = "Recommended"))
                    .help_text(tr!(literal = "A cloth resolution about one and a half times the spacing of the \
                         sparsest selected cloud's points, so every particle has returns under it."))
                    .show(ui, |ui, _row_height, _field_width| {
                        ui.label(tr_format!(
                            literal = "%resolution% m (points ~%spacing% m apart)",
                            resolution = recommended,
                            spacing = format!("{spacing:.2}")
                        ));
                        if params.cloth_resolution != recommended && ui.add(MenuButton::new(tr!(literal = "Use"))).clicked() {
                            params.cloth_resolution = recommended;
                        }
                    });
            }
            // Clouds are classified one after another, so the largest sets
            // the peak. Warn before a fine cloth over a wide cloud risks the
            // process rather than after.
            let estimate = selected
                .iter()
                .filter_map(|cloud| {
                    let (_, extent) = editor.point_cloud_classify_extents.iter().find(|(id, _)| *id == cloud.id)?;
                    Some(estimate_classify_memory_bytes(
                        *extent,
                        cloud.point_count,
                        params.cloth_resolution,
                        params.classify_vegetation,
                    ))
                })
                .max()
                .unwrap_or(0);
            let mut memory_ok = true;
            if estimate >= CLASSIFY_WARN_BYTES {
                memory_ok = estimate < CLASSIFY_HARD_BYTES;
                let color = if memory_ok { ui.visuals().warn_fg_color } else { ui.visuals().error_fg_color };
                let tail = if memory_ok {
                    tr!(literal = "Raise the cloth resolution if your machine has less RAM.")
                } else {
                    tr!(literal = "This exceeds a safe limit; raise the cloth resolution to continue.")
                };
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(tr!("tri-estimated-memory", estimate = format_bytes(estimate), detail = tail)).color(color));
                    });
            }
            MenuFieldF64::new(tr!(literal = "Ground threshold"), &mut params.class_threshold, 0.01..=10.0)
                .help_text(tr!(literal = "Points closer than this to the settled cloth, measured across its surface, \
                     are ground."))
                .speed(0.01)
                .suffix(tr!(literal = " m"))
                .max_decimals(2)
                .width(220.0)
                .show(ui);
            MenuFieldBool::new(tr!(literal = "Recover steep slopes"), &mut params.slope_recovery)
                .help_text(tr!(literal = "Let the cloth follow walls down from their crests, where its stiffness \
                     would otherwise hold it off the face. Turn off only on gentle ground \
                     crowded with plant."))
                .show(ui);
            MenuFieldBool::new(tr!(literal = "Classify vegetation"), &mut params.classify_vegetation)
                .help_text(tr!(literal = "Sort the returns with a trained classifier that reads the shape of the \
                     points around each one: ground, vegetation - banded low (under 1 m), medium \
                     (under 3 m) or high by height - and everything else, such as buildings and \
                     plant, left unclassified. Turn this off to use the cloth alone."))
                .show(ui);

            ui.add_space(4.0);
            ui.separator();

            MenuFieldBool::new(tr!(literal = "Mark noise"), &mut params.remove_noise)
                .help_text(tr!(literal = "Mark isolated returns - birds, dust, multipath blunders - as noise before \
                     the ground is found, so a stray low point cannot drag the cloth down."))
                .show(ui);
            ui.add_enabled_ui(params.remove_noise, |ui| {
                MenuFieldF64::new(tr!(literal = "Noise radius"), &mut params.noise_radius, 0.05..=100.0)
                    .help_text(tr!(literal = "How far around each point to count neighbours."))
                    .speed(0.05)
                    .suffix(tr!(literal = " m"))
                    .max_decimals(2)
                    .width(220.0)
                    .show(ui);
                MenuFieldU32::new(tr!(literal = "Minimum neighbours"), &mut params.noise_min_neighbours, 1..=1000)
                    .help_text(tr!(literal = "Points with fewer neighbours than this within the noise radius are noise."))
                    .width(220.0)
                    .show(ui);
            });

            menu::menu_actions(ui, |ui| {
                let can_run = !selected.is_empty() && memory_ok;
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!(literal = "Classify")).primary().enabled(can_run)).clicked() || (confirm && can_run) {
                    commands.push(UiCommand::ExecutePointCloudClassify {
                        cloud_ids: selected.iter().map(|cloud| cloud.id).collect(),
                        params: editor.point_cloud_classify_params,
                    });
                    editor.point_cloud_classify_open = false;
                }
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.point_cloud_classify_open = false;
                }
            });
        });
    if !open {
        editor.point_cloud_classify_open = false;
    }
}
