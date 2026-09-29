//! Point-cloud dialogs: joining tiled clouds into one, and classifying ground.

use crate::{
    i18n::tr,
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
/// Estimated classify memory above which the dialog warns - the same threshold
/// as terrain TIN generation.
const CLASSIFY_WARN_BYTES: u64 = 6 * 1024 * 1024 * 1024;

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
    DragableMenu::new("point_cloud_join_dialog", tr!("common-join-point-clouds"))
        .open(&mut open)
        .min_width(360.0)
        .show(ui.ctx(), |ui| {
            menu::menu_note(ui, tr!("point-cloud-combine-selected-point-clouds-into"));
            ui.add_space(4.0);

            // Clouds unloaded or removed since the dialog opened are no longer
            // joinable, so they leave the source list rather than block the run.
            editor.point_cloud_join_sources.retain(|id| loaded.iter().any(|cloud| cloud.id == *id));
            let selected: Vec<_> = loaded.iter().filter(|cloud| editor.point_cloud_join_sources.contains(&cloud.id)).collect();
            let total: usize = selected.iter().map(|cloud| cloud.point_count).sum();

            // The clouds come from the selection the dialog opened with, so
            // this states the set rather than offering one to tick through.
            MenuField::new(tr!("point-cloud-point-clouds"))
                .help_text(tr!("point-cloud-selected-clouds-copied-into-joined"))
                .show(ui, |ui, _row_height, _field_width| {
                    ui.label(tr!(
                        "point-cloud-count-selected-points-points",
                        count = selected.len().to_string(),
                        points = total.to_string()
                    ));
                });
            egui::ScrollArea::vertical()
                .id_salt("point_cloud_join_list")
                .max_height(JOIN_LIST_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for cloud in &selected {
                        ui.weak(tr!("point-cloud-name-count-points", name = cloud.name.to_string(), count = cloud.point_count.to_string()));
                    }
                });

            ui.add_space(4.0);
            ui.separator();

            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.point_cloud_join_name_input)
                .help_text(tr!("point-cloud-name-assigned-joined-point-cloud"))
                .width(220.0)
                .hint_text(tr!("common-joined-cloud"))
                .show(ui);
            MenuFieldBool::new(tr!("point-cloud-remove-sources"), &mut editor.point_cloud_join_remove_sources)
                .help_text(tr!("point-cloud-delete-selected-clouds-from-project"))
                .show(ui);

            menu::menu_actions(ui, |ui| {
                let can_run = selected.len() >= 2 && !editor.point_cloud_join_name_input.trim().is_empty();
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("point-cloud-join")).primary().enabled(can_run)).clicked() || (confirm && can_run) {
                    commands.push(UiCommand::ExecutePointCloudJoin {
                        cloud_ids: selected.iter().map(|cloud| cloud.id).collect(),
                        name: editor.point_cloud_join_name_input.trim().to_owned(),
                        remove_sources: editor.point_cloud_join_remove_sources,
                    });
                    editor.point_cloud_join_open = false;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
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
        ClothTerrain::Steep => tr!("point-cloud-steep-pit-walls-benches"),
        ClothTerrain::Relief => tr!("point-cloud-relief-dumps-rolling-ground"),
        ClothTerrain::Flat => tr!("point-cloud-flat-pads-structures"),
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
    DragableMenu::new("point_cloud_classify_dialog", tr!("common-classify-point-clouds"))
        .open(&mut open)
        .min_width(380.0)
        .show(ui.ctx(), |ui| {
            menu::menu_note(ui, tr!("point-cloud-mark-each-point-ground-noise"));
            ui.add_space(4.0);

            // Clouds unloaded or removed since the dialog opened drop out
            // rather than block the run.
            editor.point_cloud_classify_sources.retain(|id| loaded.iter().any(|cloud| cloud.id == *id));
            let selected: Vec<_> = loaded.iter().filter(|cloud| editor.point_cloud_classify_sources.contains(&cloud.id)).collect();
            let total: usize = selected.iter().map(|cloud| cloud.point_count).sum();
            MenuField::new(tr!("point-cloud-point-clouds"))
                .help_text(tr!("point-cloud-selected-clouds-each-classified-its"))
                .show(ui, |ui, _row_height, _field_width| {
                    ui.label(tr!(
                        "point-cloud-count-selected-points-points",
                        count = selected.len().to_string(),
                        points = total.to_string()
                    ));
                });
            egui::ScrollArea::vertical()
                .id_salt("point_cloud_classify_list")
                .max_height(JOIN_LIST_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for cloud in &selected {
                        ui.weak(tr!("point-cloud-name-count-points", name = cloud.name.to_string(), count = cloud.point_count.to_string()));
                    }
                });

            ui.add_space(4.0);
            ui.separator();

            let params = &mut editor.point_cloud_classify_params;
            let terrain_label = cloth_terrain_label(params.terrain);
            MenuFieldCombo::new(
                "point_cloud_classify_terrain",
                tr!("point-cloud-terrain"),
                &mut params.terrain,
                terrain_label,
                [ClothTerrain::Steep, ClothTerrain::Relief, ClothTerrain::Flat].map(|terrain| (terrain, cloth_terrain_label(terrain).into())),
            )
            .help_text(tr!("point-cloud-ground-cloud-covers-steep-follows"))
            .width(220.0)
            .show(ui);
            MenuFieldF64::new(tr!("point-cloud-cloth-resolution"), &mut params.cloth_resolution, 0.05..=50.0)
                .help_text(tr!("point-cloud-spacing-cloth-s-particles-around"))
                .speed(0.05)
                .suffix(format!(" {}", tr!("common-m")))
                .max_decimals(2)
                .width(220.0)
                .show(ui);
            if let Some(spacing) = editor.point_cloud_classify_spacing {
                let recommended = recommended_cloth_resolution(spacing);
                MenuField::new(tr!("point-cloud-recommended"))
                    .help_text(tr!("point-cloud-cloth-resolution-about-one-half"))
                    .show(ui, |ui, _row_height, _field_width| {
                        ui.label(tr!(
                            "point-cloud-resolution-m-points-spacing-m",
                            resolution = recommended.to_string(),
                            spacing = format!("{spacing:.2}")
                        ));
                        if params.cloth_resolution != recommended && ui.add(MenuButton::new(tr!("point-cloud-use"))).clicked() {
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
            if estimate >= CLASSIFY_WARN_BYTES {
                let tail = tr!("point-cloud-raise-cloth-resolution-if-your");
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(tr!("tri-estimated-memory", estimate = format_bytes(estimate), detail = tail)).color(ui.visuals().warn_fg_color));
                    });
            }
            MenuFieldF64::new(tr!("point-cloud-ground-threshold"), &mut params.class_threshold, 0.01..=10.0)
                .help_text(tr!("point-cloud-points-closer-than-settled-cloth"))
                .speed(0.01)
                .suffix(format!(" {}", tr!("common-m")))
                .max_decimals(2)
                .width(220.0)
                .show(ui);
            MenuFieldBool::new(tr!("point-cloud-recover-steep-slopes"), &mut params.slope_recovery)
                .help_text(tr!("point-cloud-let-cloth-follow-walls-down"))
                .show(ui);
            MenuFieldBool::new(tr!("point-cloud-classify-vegetation"), &mut params.classify_vegetation)
                .help_text(tr!("point-cloud-sort-returns-trained-classifier-read"))
                .show(ui);

            ui.add_space(4.0);
            ui.separator();

            MenuFieldBool::new(tr!("point-cloud-mark-noise"), &mut params.remove_noise)
                .help_text(tr!("point-cloud-mark-isolated-returns-birds-dust"))
                .show(ui);
            ui.add_enabled_ui(params.remove_noise, |ui| {
                MenuFieldF64::new(tr!("point-cloud-noise-radius"), &mut params.noise_radius, 0.05..=100.0)
                    .help_text(tr!("point-cloud-how-far-around-each-point"))
                    .speed(0.05)
                    .suffix(format!(" {}", tr!("common-m")))
                    .max_decimals(2)
                    .width(220.0)
                    .show(ui);
                MenuFieldU32::new(tr!("point-cloud-minimum-neighbours"), &mut params.noise_min_neighbours, 1..=1000)
                    .help_text(tr!("point-cloud-points-fewer-neighbours-than-within"))
                    .width(220.0)
                    .show(ui);
            });

            menu::menu_actions(ui, |ui| {
                let can_run = !selected.is_empty();
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("point-cloud-classify")).primary().enabled(can_run)).clicked() || (confirm && can_run) {
                    commands.push(UiCommand::ExecutePointCloudClassify {
                        cloud_ids: selected.iter().map(|cloud| cloud.id).collect(),
                        params: editor.point_cloud_classify_params,
                    });
                    editor.point_cloud_classify_open = false;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.point_cloud_classify_open = false;
                }
            });
        });
    if !open {
        editor.point_cloud_classify_open = false;
    }
}
