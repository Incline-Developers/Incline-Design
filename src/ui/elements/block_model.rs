use crate::{
    i18n::tr,
    model::{
        block_model::OpenBlockModel,
        drill_hole::{DrillFieldKind, OpenDrillHoleDataset},
        kriging::KrigingGrid,
    },
    ui::{
        EditorState, UiCommand,
        state::OreFilterMode,
        widgets::menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldF64, MenuFieldText, MenuFieldU32, menu_field_label, selected_source_field},
    },
};

pub(crate) fn draw_create_block_model_dialog(ui: &mut egui::Ui, editor: &mut EditorState, drill_holes: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    let mut open = true;
    DragableMenu::new("create_block_model_dialog", tr!("common-create-block-model"))
        .open(&mut open)
        .min_width(390.0)
        .show(ui.ctx(), |ui| {
            menu::menu_note(ui, tr!("block-model-ordinary-kriging-estimates-numeric-d"));
            ui.add_space(4.0);

            // The collection is the one that was selected when the dialog
            // opened. It can still be unloaded or removed from under the
            // dialog, which takes the run button with it.
            let selected_dataset = editor
                .kriging_drill_hole_id
                .and_then(|id| drill_holes.iter().find(|dataset| dataset.state.loaded && dataset.id == id));
            selected_source_field(
                ui,
                tr!("ws-menubar-drillholes"),
                selected_dataset.map_or_else(|| tr!("block-model-no-drill-holes-selected"), |dataset| dataset.name.clone()),
                tr!("block-model-selected-drill-holes-collection-whos"),
                220.0,
            );

            let numeric_fields: Vec<_> = selected_dataset
                .map(|dataset| dataset.dataset.fields.iter().filter(|field| matches!(field.kind, DrillFieldKind::Numeric { .. })).collect())
                .unwrap_or_default();
            editor.kriging_variables.retain(|selected| numeric_fields.iter().any(|field| field.key == *selected));
            let selected_labels: Vec<_> = numeric_fields
                .iter()
                .filter(|field| editor.kriging_variables.contains(&field.key))
                .map(|field| field.label.as_str())
                .collect();
            let selected_text = match selected_labels.as_slice() {
                [] => tr!("block-model-choose-numeric-variables"),
                [label] => (*label).to_owned(),
                labels => tr!("block-model-count-variables-selected", count = labels.len().to_string()),
            };
            let mut variable_changed = false;
            // Resolved out here: inside the row's own `ui` the column would be
            // measured against this row alone rather than the whole dialog.
            let estimate_variables_label = tr!("block-model-estimate-variables");
            let column_width = menu::field_column_for(ui, &estimate_variables_label, true);
            ui.horizontal(|ui| {
                menu_field_label(
                    ui,
                    estimate_variables_label.clone().into(),
                    Some(tr!("block-model-numeric-interval-fields-interpolate").into()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("kriging_variables")
                        .selected_text(selected_text)
                        .width(column_width)
                        .show_ui(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button(tr!("block-model-select-all")).clicked() {
                                    editor.kriging_variables = numeric_fields.iter().map(|field| field.key.clone()).collect();
                                    variable_changed = true;
                                }
                                if ui.small_button(tr!("common-clear")).clicked() {
                                    editor.kriging_variables.clear();
                                    variable_changed = true;
                                }
                            });
                            ui.separator();
                            for field in &numeric_fields {
                                let mut selected = editor.kriging_variables.contains(&field.key);
                                if ui.checkbox(&mut selected, &field.label).changed() {
                                    variable_changed = true;
                                    if selected {
                                        editor.kriging_variables.push(field.key.clone());
                                    } else {
                                        editor.kriging_variables.retain(|key| key != &field.key);
                                    }
                                }
                            }
                        });
                });
            });
            if variable_changed
                && let Some(DrillFieldKind::Numeric { min, max }) = editor
                    .kriging_variables
                    .first()
                    .and_then(|variable| selected_dataset.and_then(|dataset| dataset.dataset.field(variable)))
                    .map(|field| &field.kind)
            {
                let spread = max - min;
                editor.kriging_sill = (spread * spread / 12.0).max(1.0e-6);
            }
            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.kriging_name_input).show(ui);

            menu::menu_section(ui, tr!("block-model-block-grid"));
            vector_fields(
                ui,
                &tr!("block-model-minimum"),
                &tr!("block-model-lower-x-y-z-edges"),
                &mut editor.kriging_lower,
                f64::MIN..=f64::MAX,
            );
            vector_fields(
                ui,
                &tr!("block-model-maximum"),
                &tr!("block-model-upper-x-y-z-extent"),
                &mut editor.kriging_upper,
                f64::MIN..=f64::MAX,
            );
            vector_fields(
                ui,
                &tr!("block-model-block-size"),
                &tr!("block-model-full-x-y-z-dimensions"),
                &mut editor.kriging_cell,
                0.001..=f64::MAX,
            );

            let grid = KrigingGrid {
                lower: editor.kriging_lower,
                upper: editor.kriging_upper,
                cell: editor.kriging_cell,
            };
            let dimensions = grid.dimensions().ok();
            let block_count = dimensions.and_then(|dims| dims.into_iter().try_fold(1usize, |count, dim| count.checked_mul(dim)));
            match (dimensions, block_count) {
                (Some(dims), Some(count)) => {
                    ui.small(tr!("block-grid-summary", x = dims[0], y = dims[1], z = dims[2], count = count));
                }
                _ => {
                    ui.colored_label(ui.visuals().error_fg_color, tr!("block-model-grid-bounds-block-sizes-invalid"));
                }
            }

            menu::menu_section(ui, tr!("block-model-spherical-variogram-search"));
            MenuFieldF64::new(tr!("block-model-range-search-radius"), &mut editor.kriging_range, 0.001..=f64::MAX)
                .help_text(tr!("block-model-samples-farther-than-distance-exclud"))
                .show(ui);
            MenuFieldF64::new(tr!("block-model-partial-sill"), &mut editor.kriging_sill, 0.000001..=f64::MAX)
                .help_text(tr!("block-model-spatially-correlated-variance-contri"))
                .show(ui);
            MenuFieldF64::new(tr!("block-model-nugget"), &mut editor.kriging_nugget, 0.0..=f64::MAX)
                .help_text(tr!("block-model-variance-effectively-zero-separation"))
                .show(ui);
            MenuFieldU32::new(tr!("block-model-minimum-samples"), &mut editor.kriging_min_samples, 1..=64)
                .help_text(tr!("block-model-minimum-nearby-samples-required-esti"))
                .show(ui);
            MenuFieldU32::new(tr!("block-model-maximum-samples"), &mut editor.kriging_max_samples, 1..=64)
                .help_text(tr!("block-model-maximum-nearest-samples-used-each"))
                .show(ui);

            let ready = selected_dataset.is_some()
                && !editor.kriging_variables.is_empty()
                && !editor.kriging_name_input.trim().is_empty()
                && block_count.is_some()
                && editor.kriging_range.is_finite()
                && editor.kriging_range > 0.0
                && editor.kriging_sill.is_finite()
                && editor.kriging_sill > 0.0
                && editor.kriging_nugget.is_finite()
                && editor.kriging_nugget >= 0.0
                && editor.kriging_min_samples > 0
                && editor.kriging_min_samples <= editor.kriging_max_samples
                && editor.kriging_max_samples <= 64;
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("common-create")).primary().enabled(ready)).clicked() || (confirm && ready) {
                    commands.push(UiCommand::ExecuteCreateBlockModel {
                        drill_hole_id: editor.kriging_drill_hole_id.unwrap(),
                        variables: editor.kriging_variables.clone(),
                        name: editor.kriging_name_input.trim().to_owned(),
                        lower: editor.kriging_lower,
                        upper: editor.kriging_upper,
                        cell: editor.kriging_cell,
                        range: editor.kriging_range,
                        sill: editor.kriging_sill,
                        nugget: editor.kriging_nugget,
                        min_samples: editor.kriging_min_samples,
                        max_samples: editor.kriging_max_samples,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.block_model_create_open = false;
                }
            });
        });
    if !open {
        editor.block_model_create_open = false;
    }
}

fn vector_fields(ui: &mut egui::Ui, label: &str, help_text: &str, value: &mut glam::DVec3, range: std::ops::RangeInclusive<f64>) {
    // Three boxes sharing the column the fields above them use, so the grid
    // rows line up with the rest of the dialog rather than sizing themselves
    // to whatever numbers they happen to hold.
    let row_height = ui.spacing().interact_size.y;
    let gap = ui.spacing().item_spacing.x;
    let each = ((menu::field_column_for(ui, label, true) - gap * 2.0) / 3.0).max(36.0);
    ui.horizontal(|ui| {
        menu_field_label(ui, label.into(), Some(help_text.into()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(
                [each, row_height],
                egui::DragValue::new(&mut value.z)
                    .range(range.clone())
                    .prefix(format!("{} ", tr!("block-model-z")))
                    .speed(1.0)
                    .max_decimals(3),
            );
            ui.add_sized(
                [each, row_height],
                egui::DragValue::new(&mut value.y)
                    .range(range.clone())
                    .prefix(format!("{} ", tr!("block-model-y")))
                    .speed(1.0)
                    .max_decimals(3),
            );
            ui.add_sized(
                [each, row_height],
                egui::DragValue::new(&mut value.x)
                    .range(range)
                    .prefix(format!("{} ", tr!("block-model-x")))
                    .speed(1.0)
                    .max_decimals(3),
            );
        });
    });
}

pub(crate) fn draw_ore_triangulation_dialog(ui: &mut egui::Ui, editor: &mut EditorState, block_models: &[OpenBlockModel], commands: &mut Vec<UiCommand>) {
    let mut open = true;
    DragableMenu::new("create_ore_triangulation_dialog", tr!("common-create-ore-triangulation"))
        .open(&mut open)
        .min_width(340.0)
        .show(ui.ctx(), |ui| {
            // The model is the one that was selected when the dialog opened,
            // and can still be unloaded or removed from under it.
            let selected_model = editor
                .ore_block_model_id
                .and_then(|id| block_models.iter().find(|model| model.state.loaded && model.id == id));
            selected_source_field(
                ui,
                tr!("common-block-model"),
                selected_model.map_or_else(|| tr!("block-model-no-block-model-selected"), |model| model.name.clone()),
                tr!("block-model-selected-block-model-whose-blocks"),
                190.0,
            );

            let variables: Vec<String> = selected_model
                .map(|model| model.model.numeric_variables().into_iter().filter(|var| !var.special).map(|var| var.name.clone()).collect())
                .unwrap_or_default();
            if !variables.is_empty() && !variables.contains(&editor.ore_variable) {
                editor.ore_variable = variables[0].clone();
            }
            let variable_label = if editor.ore_variable.is_empty() {
                tr!("block-model-choose-numeric-variable")
            } else {
                editor.ore_variable.clone()
            };
            MenuFieldCombo::new(
                "ore_variable",
                tr!("block-model-variable"),
                &mut editor.ore_variable,
                variable_label.as_str(),
                variables.iter().map(|name| (name.clone(), name.clone().into())),
            )
            .show(ui);

            let ge_threshold_label = tr!("block-model-threshold-2");
            let le_threshold_label = tr!("block-model-threshold");
            let between_label = tr!("block-model-between");
            let mode_label = match editor.ore_filter_mode {
                OreFilterMode::GreaterOrEqual => ge_threshold_label.clone(),
                OreFilterMode::LessOrEqual => le_threshold_label.clone(),
                OreFilterMode::Between => between_label.clone(),
            };
            MenuFieldCombo::new(
                "ore_filter_mode",
                tr!("common-filter"),
                &mut editor.ore_filter_mode,
                mode_label,
                [
                    (OreFilterMode::GreaterOrEqual, ge_threshold_label.into()),
                    (OreFilterMode::LessOrEqual, le_threshold_label.into()),
                    (OreFilterMode::Between, between_label.into()),
                ],
            )
            .show(ui);

            MenuFieldF64::new(tr!("block-model-threshold-min"), &mut editor.ore_min_input, f64::MIN..=f64::MAX).show(ui);
            if editor.ore_filter_mode == OreFilterMode::Between {
                MenuFieldF64::new(tr!("common-max"), &mut editor.ore_max_input, f64::MIN..=f64::MAX).show(ui);
            }
            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.ore_name_input).show(ui);

            let min = editor.ore_min_input;
            let max = editor.ore_max_input;
            let ready = selected_model.is_some()
                && !editor.ore_variable.is_empty()
                && !editor.ore_name_input.trim().is_empty()
                && min.is_finite()
                && (editor.ore_filter_mode != OreFilterMode::Between || max.is_finite());
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("common-create")).primary().enabled(ready)).clicked() || (confirm && ready) {
                    commands.push(UiCommand::ExecuteCreateOreTriangulation {
                        block_model_id: editor.ore_block_model_id.unwrap(),
                        variable: editor.ore_variable.clone(),
                        mode: editor.ore_filter_mode,
                        min,
                        max: if editor.ore_filter_mode == OreFilterMode::Between { max } else { min },
                        name: editor.ore_name_input.trim().to_owned(),
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.ore_triangulation_open = false;
                }
            });
        });
    if !open {
        editor.ore_triangulation_open = false;
    }
}
