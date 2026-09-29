//! Triangulation creation and processing dialogs.

use crate::{
    i18n::tr,
    model::{Document, Object, ObjectId, SceneEntityId, triangulation::TriangulationId},
    rendering::color::{color32_to_rgba, rgba_to_color32},
    ui::{
        state::{ContourOutputLayer, EditorState, TriCreatePhase, TriPolylineClipMode, TriSurfaceCutSide, TriSurfaceType, TriangulationPickTarget, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldText, MenuFieldU32, selected_source_field},
    },
};

/// Reset the Create Triangulation workflow state.
fn tri_reset_state(editor: &mut EditorState) {
    tri_close_dialog(editor);
    editor.selected_handles.clear();
}

/// Close the dialog but leave the viewport selection alone.
///
/// Used when the dialog is dismissed rather than run: the selection was made
/// before it opened, so cancelling should not cost the user that work.
fn tri_close_dialog(editor: &mut EditorState) {
    editor.tri_create_open = false;
    editor.tri_create_phase = TriCreatePhase::MainDialog;
    editor.tri_create_picker_px = None;
    editor.tri_hover_handles.clear();
    editor.tri_selected_object_ids.clear();
    editor.tri_selected_layer_ids.clear();
    editor.tri_name_input.clear();
    editor.tri_surface_type = TriSurfaceType::Surface;
}

/// Compact summary of a viewport selection, grouped by object kind
/// ("5 polylines, 2 strings"). Preferred over per-item chip lists.
///
/// Each noun is pluralised by its own count through Fluent, so a language with
/// more than the two English plural forms (Russian's one/few/many) reads right.
fn selection_summary(object_ids: &[ObjectId], document: &Document) -> String {
    let (mut points, mut polylines, mut strings, mut texts, mut circles) = (0i64, 0i64, 0i64, 0i64, 0i64);
    for &oid in object_ids {
        match document.get_object(oid) {
            Some(Object::Point { .. }) => points += 1,
            Some(Object::Polyline { closed: true, .. }) => polylines += 1,
            Some(Object::Polyline { closed: false, .. }) => strings += 1,
            Some(Object::Circle { .. }) => circles += 1,
            Some(Object::Text { .. }) => texts += 1,
            None => {}
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if polylines > 0 {
        parts.push(tr!("tri-count-polylines", count = polylines));
    }
    if circles > 0 {
        parts.push(tr!("tri-count-circles", count = circles));
    }
    if strings > 0 {
        parts.push(tr!("tri-count-strings", count = strings));
    }
    if points > 0 {
        parts.push(tr!("tri-count-points", count = points));
    }
    if texts > 0 {
        parts.push(tr!("tri-count-texts", count = texts));
    }
    if parts.is_empty() {
        let total = object_ids.len() as i64;
        tr!("tri-count-objects", count = total)
    } else {
        parts.join(", ")
    }
}

fn tri_surface_type_label(surface_type: TriSurfaceType) -> String {
    match surface_type {
        TriSurfaceType::Surface => tr!("tri-type-open-surface"),
        TriSurfaceType::SolidClosed => tr!("tri-type-solid-closed"),
    }
}

const PICK_SELECTOR_WIDTH: f32 = 210.0;
const PICK_BUTTON_WIDTH: f32 = 52.0;
const PICKER_DIALOG_MIN_WIDTH: f32 = 430.0;
const PICKER_DIALOG_MAX_WIDTH: f32 = 450.0;

fn pick_button_label() -> String {
    tr!("drill-pattern-pick")
}

fn pick_button_width(ui: &egui::Ui, label: &str) -> f32 {
    menu::natural_button_width(ui, label, PICK_BUTTON_WIDTH)
}

fn picker_control_width(ui: &egui::Ui) -> f32 {
    let label = pick_button_label();
    PICK_SELECTOR_WIDTH + ui.spacing().item_spacing.x + pick_button_width(ui, &label)
}

/// Colour buttons use egui's full interaction width rather than the row
/// height. Keep the contour row's current comfortable numeric widths, then
/// align every other contour control to that actual rendered extent.
fn contour_control_width(ui: &egui::Ui) -> f32 {
    let interact_size = ui.spacing().interact_size;
    picker_control_width(ui) + 2.0 * (interact_size.x - interact_size.y).max(0.0)
}

fn tool_help_panel(ui: &mut egui::Ui, text: impl Into<String>) {
    menu::menu_note(ui, text);
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else {
        format!("{:.0} MB", bytes / MIB)
    }
}

/// Two equal-sized operation choices centred within the same width as a
/// selector plus its Pick button. Returns the clicked choice index.
fn centered_choice_buttons(ui: &mut egui::Ui, row_height: f32, choices: [(String, bool); 2]) -> (egui::Response, Option<usize>) {
    const BUTTON_WIDTH: f32 = 100.0;
    let gap = ui.spacing().item_spacing.x;
    let button_width = choices
        .iter()
        .map(|(label, _)| menu::natural_button_width(ui, label, BUTTON_WIDTH))
        .fold(BUTTON_WIDTH, f32::max);
    let buttons_width = button_width * 2.0 + gap;
    let width = picker_control_width(ui).max(buttons_width);
    let leading_space = ((width - buttons_width) * 0.5).max(0.0);
    let mut clicked = None;
    let response = ui
        .allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(leading_space);
            for (index, (label, selected)) in choices.into_iter().enumerate() {
                if ui.add(MenuButton::new(label).selected(selected).min_width(button_width)).clicked() {
                    clicked = Some(index);
                }
            }
        })
        .response;
    (response, clicked)
}

fn viewport_pick_status(ui: &mut egui::Ui, editor: &EditorState, empty_text: &str) {
    let text = editor.viewport_pick_hover_label.as_deref().unwrap_or(empty_text);
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(250.0);
            ui.label(egui::RichText::new(text).italics());
        });
}

fn triangulation_picker_field(
    ui: &mut egui::Ui,
    id_source: &'static str,
    label: impl Into<egui::WidgetText>,
    value: &mut Option<TriangulationId>,
    selected_text: impl Into<egui::WidgetText>,
    options: impl IntoIterator<Item = (Option<TriangulationId>, egui::WidgetText)>,
    help_text: impl Into<egui::WidgetText>,
) -> bool {
    let width = picker_control_width(ui);
    triangulation_picker_field_with_width(ui, id_source, label, value, selected_text, options, help_text, width)
}

#[allow(clippy::too_many_arguments)]
fn triangulation_picker_field_with_width(
    ui: &mut egui::Ui,
    id_source: &'static str,
    label: impl Into<egui::WidgetText>,
    value: &mut Option<TriangulationId>,
    selected_text: impl Into<egui::WidgetText>,
    options: impl IntoIterator<Item = (Option<TriangulationId>, egui::WidgetText)>,
    help_text: impl Into<egui::WidgetText>,
    width: f32,
) -> bool {
    let selected_text = selected_text.into();
    let pick_label = pick_button_label();
    let pick_width = pick_button_width(ui, &pick_label);
    let mut pick_clicked = false;
    MenuField::new(label).help_text(help_text).show(ui, |ui, row_height, _| {
        let selector_width = width - ui.spacing().item_spacing.x - pick_width;
        ui.allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt(id_source)
                .selected_text(selected_text.clone())
                .width(selector_width)
                .show_ui(ui, |ui| {
                    for (option, text) in options {
                        ui.selectable_value(value, option, text);
                    }
                })
                .response
                .on_hover_text(selected_text);
            pick_clicked = ui
                .add(MenuButton::new(pick_label).min_width(pick_width))
                .on_hover_text(tr!("tri-choose-input-clicking-loaded-surface"))
                .clicked();
        })
        .response
    });
    pick_clicked
}

/// Name of a loaded surface, or a stand-in if it went away while the dialog
/// was open - unloading or deleting it is a legitimate thing to do, and the
/// run button is already disabled by then.
fn surface_name(project: &UiProjectView, tri_id: Option<TriangulationId>) -> String {
    tri_id
        .and_then(|id| project.triangulations.iter().find(|entry| entry.is_loaded && entry.id == id))
        .map_or_else(|| tr!("tri-no-surface-selected"), |entry| entry.name.clone())
}

pub(crate) fn draw_triangulation_pick_prompt(ui: &mut egui::Ui, editor: &mut EditorState) {
    let Some(target) = editor.triangulation_pick_target else {
        return;
    };
    let mut open = true;
    DragableMenu::new("triangulation_pick_from_view_dialog", tr!("tri-pick-from-view"))
        .open(&mut open)
        .min_width(280.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui.ctx(), |ui| {
            ui.label(target.prompt());
            ui.label(egui::RichText::new(tr!("tri-only-loaded-triangulations-can-picke")).weak());
            ui.add_space(6.0);
            viewport_pick_status(ui, editor, &tr!("tri-move-cursor-over-loaded-surface"));
            ui.add_space(6.0);
            if ui.add(MenuButton::new(tr!("tri-cancel-pick"))).clicked() {
                editor.triangulation_pick_target = None;
                editor.viewport_pick_hover_label = None;
                editor.tri_hover_handles.clear();
            }
        });
    if !open {
        editor.triangulation_pick_target = None;
        editor.viewport_pick_hover_label = None;
        editor.tri_hover_handles.clear();
    }
}

/// Movable main dialog for the Create Triangulation workflow.
pub(crate) fn draw_tri_create_main_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    if !editor.tri_create_open || editor.tri_create_phase != TriCreatePhase::MainDialog {
        return;
    }

    let mut open = true;
    DragableMenu::new("create_triangulation_selection_dialog", tr!("tri-create-title"))
        .open(&mut open)
        .min_width(370.0)
        .show(ui.ctx(), |ui| {
            tool_help_panel(ui, tr!("tri-create-help"));
            ui.add_space(4.0);

            // --- Selection summary ---
            // The run works on the selection the dialog opened with, so the
            // set is reported here rather than offered for editing: changing
            // it means closing the dialog and selecting again.
            let has_selection = !editor.tri_selected_object_ids.is_empty();
            let hover_selection = if has_selection {
                let summary = tr!("tri-selection-selected", summary = selection_summary(&editor.tri_selected_object_ids, document));
                ui.label(summary).hovered()
            } else {
                ui.colored_label(egui::Color32::GRAY, tr!("tri-selection-none"));
                false
            };

            // Hovering the summary highlights the whole selection in the viewport.
            editor.tri_hover_handles = if hover_selection {
                editor.tri_selected_object_ids.iter().map(|&oid| SceneEntityId::Object(oid)).collect()
            } else {
                std::collections::HashSet::new()
            };

            ui.add_space(4.0);
            {
                ui.separator();

                // --- Surface / solid type ---
                let surface_type_label = tri_surface_type_label(editor.tri_surface_type);
                MenuFieldCombo::new(
                    "tri_surface_type",
                    tr!("tri-create-type-label"),
                    &mut editor.tri_surface_type,
                    surface_type_label,
                    [TriSurfaceType::Surface, TriSurfaceType::SolidClosed].map(|surface_type| (surface_type, tri_surface_type_label(surface_type).into())),
                )
                .help_text(tr!("tri-create-type-help"))
                .width(210.0)
                .show(ui);
            }

            ui.separator();

            // --- Name + Triangulate ---
            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_name_input)
                .help_text(tr!("tri-create-output-name-help"))
                .width(PICK_SELECTOR_WIDTH)
                .hint_text(tr!("tri-create-output-name-hint"))
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let ready = has_selection && !editor.tri_name_input.trim().is_empty();
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                let cancel = menu::dialog_cancel_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("tri-create-run")).primary().enabled(ready)).clicked() || (confirm && ready) {
                    let object_ids: Vec<ObjectId> = editor.tri_selected_object_ids.clone();
                    let name = editor.tri_name_input.trim().to_owned();
                    let surface_type = editor.tri_surface_type;
                    let command = UiCommand::ExecuteCreateTriangulation { name, object_ids, surface_type };
                    tri_reset_state(editor);
                    commands.push(command);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || cancel {
                    tri_close_dialog(editor);
                }
            });
        });

    if !open {
        tri_close_dialog(editor);
    }
}

/// Shown when Create Triangulation needs either corrected input or an explicit
/// recovery policy. Actionable failures use concise explanations because the
/// exact contributing geometry is already highlighted in the viewport.
pub(crate) fn draw_tri_create_failure_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(failure) = editor.tri_create_failure.clone() else {
        return;
    };

    let mut open = true;
    let mut dismiss = false;
    DragableMenu::new("triangulation_failed_dialog", tr!("tri-triangulation-failed"))
        .open(&mut open)
        .min_width(340.0)
        .show(ui.ctx(), |ui| {
            if failure.weld_retry_available {
                ui.colored_label(egui::Color32::LIGHT_RED, tr!("tri-nearby-breakline-vertices-do-not"));
                ui.add_space(4.0);
                ui.strong(tr!("tri-recommended-weld-retry"));
                ui.colored_label(egui::Color32::GRAY, tr!("tri-vertices-within-5-cm-xy"));
                menu::menu_actions(ui, |ui| {
                    if ui.add(MenuButton::new(tr!("tri-weld-retry")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                        commands.push(UiCommand::ExecuteCreateTriangulationWithWeld {
                            name: failure.name.clone(),
                            object_ids: failure.object_ids.clone(),
                            surface_type: failure.surface_type,
                        });
                        dismiss = true;
                    }
                    if ui.add(MenuButton::new(tr!("survey-close"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                        dismiss = true;
                    }
                });
            } else if failure.upper_surface_retry_available {
                ui.colored_label(egui::Color32::LIGHT_RED, tr!("tri-highlighted-breakline-edges-cross-ov"));
                ui.add_space(4.0);
                ui.strong(tr!("tri-solution-generate-upper-surface"));
                ui.colored_label(egui::Color32::GRAY, tr!("tri-higher-edge-will-enforced-each"));
                menu::menu_actions(ui, |ui| {
                    if ui.add(MenuButton::new(tr!("tri-generate-upper-surface")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                        commands.push(UiCommand::ExecuteCreateTriangulationUpperSurface {
                            name: failure.name.clone(),
                            object_ids: failure.object_ids.clone(),
                            surface_type: failure.surface_type,
                            coarse_weld: failure.coarse_weld_applied,
                        });
                        dismiss = true;
                    }
                    if ui.add(MenuButton::new(tr!("survey-close"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                        dismiss = true;
                    }
                });
            } else {
                ui.colored_label(egui::Color32::LIGHT_RED, &failure.message);
                // Only one button, so Enter and Escape both dismiss.
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                let cancel = menu::dialog_cancel_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("survey-close"))).clicked() || confirm || cancel {
                    dismiss = true;
                }
            }
        });

    if dismiss || !open {
        editor.tri_create_failure = None;
    }
}

pub(crate) fn draw_cut_poly_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_cut_poly_open || editor.triangulation_pick_target.is_some() {
        return;
    }
    if let Some(object_id) = editor.tri_cut_poly_object_id {
        let boundary_is_valid = document.get_object(object_id).is_some_and(Object::encloses_area);
        if !boundary_is_valid {
            editor.tri_cut_poly_object_id = None;
            editor.tri_cut_poly_object_name.clear();
            if editor.tool_highlight_id == Some(object_id) {
                editor.tool_highlight_id = None;
            }
        }
    }

    let mut open = true;
    DragableMenu::new("clip_surface_by_polyline_dialog", tr!("tri-clip-surface-polyline"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui.ctx(), |ui| {
            let width = picker_control_width(ui);
            selected_source_field(
                ui,
                tr!("tri-type-open-surface"),
                surface_name(project, editor.tri_cut_poly_tri_id),
                tr!("tri-selected-surface-which-will-clipped"),
                width,
            );

            ui.add_space(4.0);

            selected_source_field(
                ui,
                tr!("tri-boundary-polyline"),
                if editor.tri_cut_poly_object_id.is_some() {
                    editor.tri_cut_poly_object_name.clone()
                } else {
                    tr!("tri-no-boundary-selected")
                },
                tr!("tri-selected-closed-polyline-whose-xy"),
                width,
            );

            ui.add_space(4.0);

            MenuField::new(tr!("tri-result"))
                .help_text(tr!("tri-keep-inside-discards-surface-outside"))
                .show(ui, |ui, row_height, _| {
                    let (response, clicked) = centered_choice_buttons(
                        ui,
                        row_height,
                        [
                            (TriPolylineClipMode::KeepInside.label(), editor.tri_cut_poly_mode == TriPolylineClipMode::KeepInside),
                            (TriPolylineClipMode::KeepOutside.label(), editor.tri_cut_poly_mode == TriPolylineClipMode::KeepOutside),
                        ],
                    );
                    if clicked == Some(0) {
                        editor.tri_cut_poly_mode = TriPolylineClipMode::KeepInside;
                    } else if clicked == Some(1) {
                        editor.tri_cut_poly_mode = TriPolylineClipMode::KeepOutside;
                    }
                    response
                });
            tool_help_panel(
                ui,
                match editor.tri_cut_poly_mode {
                    TriPolylineClipMode::KeepInside => tr!("tri-keeps-only-surface-within-polyline"),
                    TriPolylineClipMode::KeepOutside => tr!("tri-removes-surface-within-polyline-boun"),
                },
            );

            ui.add_space(4.0);

            // Output name
            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_poly_name_input)
                .help_text(tr!("tri-clip-creates-new-triangulation-name"))
                .width(width)
                .hint_text(tr!("tri-e-g-mysurf-cut"))
                .show(ui);
            MenuFieldBool::new(tr!("tri-unload-source-surface"), &mut editor.tri_cut_poly_unload_source)
                .help_text(tr!("tri-once-clip-succeeds-unload-source"))
                .show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_cut_poly_tri_id.is_some() && editor.tri_cut_poly_object_id.is_some() && !editor.tri_cut_poly_name_input.trim().is_empty();

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-clip")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(tri_id), Some(poly_id)) = (editor.tri_cut_poly_tri_id, editor.tri_cut_poly_object_id)
                {
                    commands.push(UiCommand::ExecuteCutTriangulationByPolyline {
                        tri_id,
                        polyline_id: poly_id,
                        mode: editor.tri_cut_poly_mode,
                        name: editor.tri_cut_poly_name_input.trim().to_owned(),
                        unload_source: editor.tri_cut_poly_unload_source,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.tri_cut_poly_open = false;
                    editor.tool_highlight_id = None;
                }
            });
        });
    if !open {
        editor.tri_cut_poly_open = false;
        editor.tool_highlight_id = None;
    }
}

pub(crate) fn draw_cut_z_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_cut_z_open || editor.triangulation_pick_target.is_some() {
        return;
    }
    let mut open = true;
    DragableMenu::new("slice_triangulation_z_dialog", tr!("tri-slice-triangulation-z-range"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let width = picker_control_width(ui);
            let selected_name = surface_name(project, editor.tri_cut_z_tri_id);
            selected_source_field(ui, tr!("tri-type-open-surface"), selected_name, tr!("tri-selected-surface-whose-elevation-ran"), width);

            ui.add_space(4.0);

            MenuField::new(tr!("tri-axis-range", axis = crate::model::survey::axis_name(2).to_string()))
                .help_text(tr!("tri-minimum-maximum-elevations-retained"))
                .show(ui, |ui, row_height, _| {
                    let width = picker_control_width(ui);
                    let gap = ui.spacing().item_spacing.x;
                    let field_width = (width - gap) * 0.5;
                    ui.allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add_sized(
                            [field_width, row_height],
                            egui::DragValue::new(&mut editor.tri_cut_z_min_input)
                                .range(f64::MIN..=f64::MAX)
                                .speed(0.1)
                                .prefix(format!("{} ", tr!("tri-min")))
                                .max_decimals(2),
                        );
                        ui.add_sized(
                            [field_width, row_height],
                            egui::DragValue::new(&mut editor.tri_cut_z_max_input)
                                .range(f64::MIN..=f64::MAX)
                                .speed(0.1)
                                .prefix(format!("{} ", tr!("common-max")))
                                .max_decimals(2),
                        );
                    })
                    .response
                });

            ui.add_space(4.0);

            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_z_name_input)
                .help_text(tr!("tri-name-assigned-elevation-clipped-outp"))
                .width(width)
                .hint_text(tr!("tri-e-g-mysurf-slice"))
                .show(ui);
            MenuFieldBool::new(tr!("tri-unload-source-surface"), &mut editor.tri_cut_z_unload_source)
                .help_text(tr!("tri-once-slice-succeeds-unload-source"))
                .show(ui);

            ui.add_space(6.0);
            ui.separator();

            let z_min = editor.tri_cut_z_min_input;
            let z_max = editor.tri_cut_z_max_input;
            let valid_z_range = z_min.is_finite() && z_max.is_finite() && z_min < z_max;
            let can_run = editor.tri_cut_z_tri_id.is_some() && valid_z_range && !editor.tri_cut_z_name_input.trim().is_empty();

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("common-slice")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let Some(tri_id) = editor.tri_cut_z_tri_id
                {
                    commands.push(UiCommand::ExecuteCutTriangulationByZ {
                        tri_id,
                        z_min,
                        z_max,
                        name: editor.tri_cut_z_name_input.trim().to_owned(),
                        unload_source: editor.tri_cut_z_unload_source,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.tri_cut_z_open = false;
                }
            });
        });
    if !open {
        editor.tri_cut_z_open = false;
    }
}

pub(crate) fn draw_cut_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_cut_surface_open || editor.triangulation_pick_target.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("trim_surface_to_topology_dialog", tr!("tri-trim-topology"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let loaded: Vec<(TriangulationId, &str)> = project
                .triangulations
                .iter()
                .filter(|entry| entry.is_loaded)
                .map(|entry| (entry.id, entry.name.as_str()))
                .collect();

            // Match the other topology tools: the reference topology comes
            // first, followed by the surface that will be changed.
            let reference_label = editor
                .tri_cut_surface_reference_id
                .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            let target_id = editor.tri_cut_surface_target_id;
            if triangulation_picker_field(
                ui,
                "cut_surface_reference",
                tr!("tri-topology"),
                &mut editor.tri_cut_surface_reference_id,
                reference_label,
                loaded.iter().filter(|(id, _)| Some(*id) != target_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-reference-topology-defines-where-oth"),
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::TrimTopology);
            }

            let target_label = editor
                .tri_cut_surface_target_id
                .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            let old_target = editor.tri_cut_surface_target_id;
            let reference_id = editor.tri_cut_surface_reference_id;
            if triangulation_picker_field(
                ui,
                "cut_surface_target",
                tr!("tri-surface-trim"),
                &mut editor.tri_cut_surface_target_id,
                target_label,
                loaded.iter().filter(|(id, _)| Some(*id) != reference_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-surface-will-changed-selected-topolo"),
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::TrimSurface);
            }

            if editor.tri_cut_surface_target_id != old_target
                && editor.tri_cut_surface_name_auto
                && let Some(name) = editor
                    .tri_cut_surface_target_id
                    .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
            {
                let path = std::path::Path::new(name);
                let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or(name);
                let ext = path.extension().and_then(|value| value.to_str());
                editor.tri_cut_surface_name_input = if let Some(ext) = ext {
                    format!("{stem}_trimmed.{ext}")
                } else {
                    format!("{stem}_trimmed")
                };
            }

            ui.add_space(4.0);

            MenuField::new(tr!("tri-operation"))
                .help_text(tr!("tri-choose-which-side-reference-topology"))
                .show(ui, |ui, row_height, _| {
                    let (response, clicked) = centered_choice_buttons(
                        ui,
                        row_height,
                        [
                            (TriSurfaceCutSide::CutTop.trim_label(), editor.tri_cut_surface_side == TriSurfaceCutSide::CutTop),
                            (TriSurfaceCutSide::CutBottom.trim_label(), editor.tri_cut_surface_side == TriSurfaceCutSide::CutBottom),
                        ],
                    );
                    if clicked == Some(0) {
                        editor.tri_cut_surface_side = TriSurfaceCutSide::CutTop;
                    } else if clicked == Some(1) {
                        editor.tri_cut_surface_side = TriSurfaceCutSide::CutBottom;
                    }
                    response
                });

            tool_help_panel(
                ui,
                tr!(
                    "tri-keeps-surface-relation-topology-with",
                    relation = editor.tri_cut_surface_side.retained_relation().to_string()
                ),
            );

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_surface_name_input)
                .help_text(tr!("tri-name-assigned-trimmed-output-surface"))
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-e-g-design-trimmed"))
                .show(ui)
                .changed()
            {
                editor.tri_cut_surface_name_auto = false;
            }
            MenuFieldBool::new(tr!("tri-unload-source-surface"), &mut editor.tri_cut_surface_unload_source)
                .help_text(tr!("tri-once-trim-succeeds-unload-surface"))
                .show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_cut_surface_target_id.is_some()
                && editor.tri_cut_surface_reference_id.is_some()
                && editor.tri_cut_surface_target_id != editor.tri_cut_surface_reference_id
                && !editor.tri_cut_surface_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-trim")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(target_id), Some(reference_id)) = (editor.tri_cut_surface_target_id, editor.tri_cut_surface_reference_id)
                {
                    commands.push(UiCommand::ExecuteCutTriangulationBySurface {
                        target_id,
                        reference_id,
                        side: editor.tri_cut_surface_side,
                        name: editor.tri_cut_surface_name_input.trim().to_owned(),
                        unload_source: editor.tri_cut_surface_unload_source,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.tri_cut_surface_open = false;
                }
            });
        });

    if !open {
        editor.tri_cut_surface_open = false;
    }
}

pub(crate) fn draw_cut_topology_to_pit_shell_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_cut_pitshell_open || editor.triangulation_pick_target.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("cut_topology_with_pit_shell_dialog", tr!("tri-cut-topology-pit-shell"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let loaded: Vec<(TriangulationId, &str)> = project
                .triangulations
                .iter()
                .filter(|entry| entry.is_loaded)
                .map(|entry| (entry.id, entry.name.as_str()))
                .collect();

            let topology_label = editor
                .tri_cut_pitshell_topology_id
                .and_then(|id| loaded.iter().find(|(lid, _)| *lid == id).map(|(_, n)| *n))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            let old_topology_id = editor.tri_cut_pitshell_topology_id;
            if triangulation_picker_field(
                ui,
                "cut_pitshell_topology",
                tr!("tri-topology"),
                &mut editor.tri_cut_pitshell_topology_id,
                topology_label,
                loaded
                    .iter()
                    .filter(|(id, _)| Some(*id) != editor.tri_cut_pitshell_pitshell_id)
                    .map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-existing-ground-topology-will-cut"),
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::CutPitTopology);
            }

            if editor.tri_cut_pitshell_topology_id != old_topology_id
                && editor.tri_cut_pitshell_name_auto
                && let Some(name) = editor
                    .tri_cut_pitshell_topology_id
                    .and_then(|id| loaded.iter().find(|(lid, _)| *lid == id).map(|(_, n)| *n))
            {
                let path = std::path::Path::new(name);
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
                let ext = path.extension().and_then(|e| e.to_str());
                editor.tri_cut_pitshell_name_input = if let Some(ext) = ext { format!("{stem}_cut.{ext}") } else { format!("{stem}_cut") };
            }

            let pitshell_label = editor
                .tri_cut_pitshell_pitshell_id
                .and_then(|id| loaded.iter().find(|(lid, _)| *lid == id).map(|(_, n)| *n))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            if triangulation_picker_field(
                ui,
                "cut_pitshell_shell",
                tr!("tri-pit-shell"),
                &mut editor.tri_cut_pitshell_pitshell_id,
                pitshell_label,
                loaded
                    .iter()
                    .filter(|(id, _)| Some(*id) != editor.tri_cut_pitshell_topology_id)
                    .map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-pit-design-surface-only-areas"),
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::CutPitShell);
            }

            ui.add_space(4.0);
            tool_help_panel(ui, tr!("tri-removes-topology-where-pit-shell"));

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_pitshell_name_input)
                .help_text(tr!("tri-name-assigned-topology-after-pit"))
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-e-g-topo-cut"))
                .show(ui)
                .changed()
            {
                editor.tri_cut_pitshell_name_auto = false;
            }
            MenuFieldBool::new(tr!("tri-unload-source-topology"), &mut editor.tri_cut_pitshell_unload_source)
                .help_text(tr!("tri-once-cut-succeeds-unload-original"))
                .show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_cut_pitshell_topology_id.is_some() && editor.tri_cut_pitshell_pitshell_id.is_some() && !editor.tri_cut_pitshell_name_input.trim().is_empty();

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("common-cut")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(topology_id), Some(pit_shell_id)) = (editor.tri_cut_pitshell_topology_id, editor.tri_cut_pitshell_pitshell_id)
                {
                    commands.push(UiCommand::ExecuteCutTopologyByPitShell {
                        topology_id,
                        pit_shell_id,
                        name: editor.tri_cut_pitshell_name_input.trim().to_owned(),
                        unload_source: editor.tri_cut_pitshell_unload_source,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.tri_cut_pitshell_open = false;
                    editor.tool_highlight_id = None;
                }
            });
        });

    if !open {
        editor.tri_cut_pitshell_open = false;
        editor.tool_highlight_id = None;
    }
}

pub(crate) fn draw_include_solid_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_include_solid_open || editor.triangulation_pick_target.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("merge_shell_into_topology_dialog", tr!("common-merge-shell-into-topology"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let loaded: Vec<(TriangulationId, &str)> = project
                .triangulations
                .iter()
                .filter(|entry| entry.is_loaded)
                .map(|entry| (entry.id, entry.name.as_str()))
                .collect();

            let topology_label = editor
                .tri_include_solid_topology_id
                .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            let old_topology = editor.tri_include_solid_topology_id;
            if triangulation_picker_field(
                ui,
                "include_solid_topology",
                tr!("tri-topology"),
                &mut editor.tri_include_solid_topology_id,
                topology_label,
                loaded.iter().map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-base-topology-will-receive-pit"),
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::IncludeTopology);
            }

            if editor.tri_include_solid_topology_id != old_topology {
                if editor.tri_include_solid_shape_id == editor.tri_include_solid_topology_id {
                    editor.tri_include_solid_shape_id = None;
                }
                if editor.tri_include_solid_name_auto
                    && let Some(name) = editor
                        .tri_include_solid_topology_id
                        .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                {
                    let stem = std::path::Path::new(name).file_stem().and_then(|value| value.to_str()).unwrap_or(name);
                    editor.tri_include_solid_name_input = format!("{stem}_with_shape");
                }
            }

            let shape_label = editor
                .tri_include_solid_shape_id
                .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            let topology_id = editor.tri_include_solid_topology_id;
            if triangulation_picker_field(
                ui,
                "include_solid_shape",
                tr!("tri-pit-stockpile-solid"),
                &mut editor.tri_include_solid_shape_id,
                shape_label,
                loaded.iter().filter(|(id, _)| Some(*id) != topology_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-closed-pit-stockpile-solid-whose"),
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::IncludeShape);
            }

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_include_solid_name_input)
                .help_text(tr!("tri-name-assigned-merged-topology-pit"))
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-e-g-topo-pit"))
                .show(ui)
                .changed()
            {
                editor.tri_include_solid_name_auto = false;
            }
            MenuFieldBool::new(tr!("tri-save-two-entities"), &mut editor.tri_include_solid_save_as_two)
                .help_text(tr!("tri-keep-clipped-topology-included-shape"))
                .show(ui);
            MenuFieldBool::new(tr!("tri-hide-unload-sources"), &mut editor.tri_include_solid_hide_old)
                .help_text(tr!("tri-once-merge-succeeds-unload-source"))
                .show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_include_solid_topology_id.is_some()
                && editor.tri_include_solid_shape_id.is_some()
                && editor.tri_include_solid_topology_id != editor.tri_include_solid_shape_id
                && !editor.tri_include_solid_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-merge")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(topology_id), Some(shape_id)) = (editor.tri_include_solid_topology_id, editor.tri_include_solid_shape_id)
                {
                    commands.push(UiCommand::ExecuteIncludeSolidInTopology {
                        topology_id,
                        shape_id,
                        name: editor.tri_include_solid_name_input.trim().to_owned(),
                        save_as_two: editor.tri_include_solid_save_as_two,
                        hide_old: editor.tri_include_solid_hide_old,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.tri_include_solid_open = false;
                }
            });
        });

    if !open {
        editor.tri_include_solid_open = false;
    }
}

pub(crate) fn draw_contour_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_contour_open || editor.triangulation_pick_target.is_some() {
        return;
    }
    let mut open = true;
    DragableMenu::new("generate_contour_lines_dialog", tr!("tri-generate-contour-lines"))
        .open(&mut open)
        // The combined interval row has four controls; give its long label
        // and info marker a clear gutter without changing control alignment.
        .min_width(PICKER_DIALOG_MIN_WIDTH + 62.0)
        .max_width(PICKER_DIALOG_MAX_WIDTH + 62.0)
        .show(ui.ctx(), |ui| {
            let control_width = contour_control_width(ui);
            selected_source_field(
                ui,
                tr!("tri-type-open-surface"),
                surface_name(project, editor.tri_contour_tri_id),
                tr!("tri-selected-surface-from-which-contour"),
                control_width,
            );

            ui.add_space(4.0);

            let mut minor_color = rgba_to_color32(editor.tri_contour_minor_color);
            let mut major_color = rgba_to_color32(editor.tri_contour_major_color);
            MenuField::new(tr!("tri-intervals-colours"))
                .help_text(tr!("tri-minor-controls-ordinary-contours-maj"))
                .show(ui, |ui, row_height, _| {
                    let width = control_width;
                    let gap = ui.spacing().item_spacing.x;
                    let colour_width = ui.spacing().interact_size.x;
                    let value_width = (width - colour_width * 2.0 - gap * 3.0) * 0.5;
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, row_height),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add_sized(
                                [value_width, row_height],
                                egui::DragValue::new(&mut editor.tri_contour_minor_interval_input)
                                    .range(1e-6..=f64::MAX)
                                    .speed(0.1)
                                    .prefix(format!("{} ", tr!("tri-minor")))
                                    .max_decimals(3),
                            );
                            crate::ui::widgets::color::edit_srgba(ui, &mut minor_color, egui::color_picker::Alpha::OnlyBlend);
                            ui.add_sized(
                                [value_width, row_height],
                                egui::DragValue::new(&mut editor.tri_contour_major_interval_input)
                                    .range(1e-6..=f64::MAX)
                                    .speed(0.1)
                                    .prefix(format!("{} ", tr!("tri-major")))
                                    .max_decimals(3),
                            );
                            crate::ui::widgets::color::edit_srgba(ui, &mut major_color, egui::color_picker::Alpha::OnlyBlend);
                        },
                    )
                    .response
                });
            editor.tri_contour_minor_color = color32_to_rgba(minor_color);
            editor.tri_contour_major_color = color32_to_rgba(major_color);

            MenuField::new(tr!("tri-limit-z-range"))
                .help_text(tr!("tri-when-enabled-generate-contours-only"))
                .show(ui, |ui, row_height, _| {
                    let width = control_width;
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, row_height),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add_sized(
                                [row_height, row_height],
                                egui::Checkbox::new(&mut editor.tri_contour_use_z_range, ""),
                            );
                            if editor.tri_contour_use_z_range {
                                let gap = ui.spacing().item_spacing.x;
                                let value_width = (width - row_height - gap * 2.0) * 0.5;
                                ui.add_sized(
                                    [value_width, row_height],
                                    egui::DragValue::new(&mut editor.tri_contour_z_min_input)
                                        .range(f64::MIN..=f64::MAX)
                                        .speed(0.1)
                                        .prefix(format!("{} ", tr!("tri-min")))
                                        .max_decimals(2),
                                );
                                ui.add_sized(
                                    [value_width, row_height],
                                    egui::DragValue::new(&mut editor.tri_contour_z_max_input)
                                        .range(f64::MIN..=f64::MAX)
                                        .speed(0.1)
                                        .prefix(format!("{} ", tr!("common-max")))
                                        .max_decimals(2),
                                );
                            } else {
                                ui.weak(tr!("tri-use-full-surface-elevation-range"));
                            }
                        },
                    )
                    .response
                });

            ui.add_space(4.0);

            let active_project = project.projects.iter().find(|entry| entry.is_active);
            let active_layers = active_project
                .map(|entry| entry.layers.as_slice())
                .unwrap_or(&[]);
            if editor
                .tri_contour_target_layer
                .is_some_and(|id| !active_layers.iter().any(|layer| layer.id == id))
            {
                editor.tri_contour_target_layer = None;
            }
            let output_layer_label = editor
                .tri_contour_target_layer
                .and_then(|id| active_layers.iter().find(|layer| layer.id == id))
                .map(|layer| layer.name.clone())
                .unwrap_or_else(|| tr!("tri-new-layer"));
            MenuFieldCombo::new(
                "contour_output_layer",
                tr!("tri-output-layer"),
                &mut editor.tri_contour_target_layer,
                output_layer_label,
                std::iter::once((None, tr!("tri-new-layer").into())).chain(
                    active_layers
                        .iter()
                        .map(|layer| (Some(layer.id), layer.name.clone().into())),
                ),
            )
            .help_text(tr!("tri-create-new-layer-contours-append"))
            .width(control_width)
            .show(ui);

            if editor.tri_contour_target_layer.is_none()
                && MenuFieldText::new(tr!("tri-new-layer-name"), &mut editor.tri_contour_layer_name_input)
                    .help_text(tr!("tri-name-assigned-newly-created-contour"))
                    .width(control_width)
                    .hint_text(tr!("tri-e-g-surface-contour"))
                    .show(ui)
                    .changed()
            {
                editor.tri_contour_layer_name_auto = false;
            }
            let new_layer_name = editor.tri_contour_layer_name_input.trim().to_owned();
            let new_layer_name_conflicts = editor.tri_contour_target_layer.is_none()
                && active_layers
                    .iter()
                    .any(|layer| layer.name == new_layer_name);
            if editor.tri_contour_target_layer.is_none() && new_layer_name_conflicts {
                ui.colored_label(egui::Color32::LIGHT_RED, tr!("tri-layer-already-exists-select-above"));
            }

            ui.add_space(6.0);
            ui.separator();

            let minor_interval = editor.tri_contour_minor_interval_input;
            let major_interval = editor.tri_contour_major_interval_input;
            let valid_intervals = minor_interval.is_finite()
                && major_interval.is_finite()
                && minor_interval >= 1e-6
                && major_interval >= 1e-6
                && major_interval >= minor_interval;
            let z_range = editor.tri_contour_use_z_range.then_some((
                editor.tri_contour_z_min_input,
                editor.tri_contour_z_max_input,
            ));
            let valid_z_range =
                z_range.is_none_or(|(lo, hi)| lo.is_finite() && hi.is_finite() && lo < hi);
            let can_run = editor.tri_contour_tri_id.is_some()
                && valid_intervals
                && valid_z_range
                && project.has_active_project
                && (editor.tri_contour_target_layer.is_some()
                    || (!new_layer_name.is_empty() && !new_layer_name_conflicts));

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-generate")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let Some(tri_id) = editor.tri_contour_tri_id
                {
                    let output_layer = editor
                        .tri_contour_target_layer
                        .map(ContourOutputLayer::Existing)
                        .unwrap_or_else(|| ContourOutputLayer::New(new_layer_name.clone()));
                    commands.push(UiCommand::ExecuteContourTriangulation {
                        tri_id,
                        major_interval,
                        minor_interval,
                        major_color: editor.tri_contour_major_color,
                        minor_color: editor.tri_contour_minor_color,
                        z_range,
                        output_layer,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.tri_contour_open = false;
                }
            });
        });
    if !open {
        editor.tri_contour_open = false;
    }
}

/// Survey point-cloud reconstruction: build an open terrain TIN from XY/Z
/// samples.
pub(crate) fn draw_point_cloud_tin_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.point_cloud_tin_open {
        return;
    }
    use crate::app::commands::triangulation::{TerrainBudget, TerrainSampler, TerrainTinParams, estimate_terrain_tin_memory_bytes, terrain_budget_target};

    let mut open = true;
    DragableMenu::new("point_cloud_create_triangulation_dialog", tr!("tri-create-title"))
        .open(&mut open)
        .min_width(400.0)
        .show(ui.ctx(), |ui| {
            tool_help_panel(ui, tr!("tri-reconstruct-triangulated-terrain-sur"));
            ui.add_space(4.0);

            // The cloud is the one that was selected when the dialog opened.
            // It can still be unloaded or deleted from under the dialog, which
            // takes the run button with it.
            let selected = editor.point_cloud_tin_cloud_id.and_then(|id| {
                project
                    .point_clouds
                    .iter()
                    .find(|cloud| cloud.is_loaded && cloud.id == id)
                    .map(|cloud| (cloud.id, cloud.name.as_str(), cloud.point_count, cloud.is_classified))
            });
            selected_source_field(
                ui,
                tr!("common-point-cloud"),
                selected.map(|(_, name, ..)| name.to_owned()).unwrap_or_else(|| tr!("tri-no-point-cloud-selected")),
                tr!("tri-selected-point-cloud-whose-points"),
                220.0,
            );

            // Bare earth is the surveyor's first move on a delivery, so it is
            // offered right under the cloud it applies to and left on. A cloud
            // that never went through a ground filter has nothing to offer, and
            // says so rather than showing a switch that would change nothing.
            let classified = selected.is_some_and(|(.., classified)| classified);
            let mut ground_only = classified && editor.point_cloud_tin_ground_only;
            ui.add_enabled_ui(classified, |ui| {
                let field = MenuFieldBool::new(tr!("tri-ground-points-only"), &mut ground_only).help_text(if classified {
                    tr!("tri-reconstruct-from-points-classified-b")
                } else {
                    tr!("tri-cloud-carries-no-classifications-so")
                });
                if field.show(ui).changed() {
                    editor.point_cloud_tin_ground_only = ground_only;
                }
            });

            let sampler_label = match editor.point_cloud_tin_sampler {
                TerrainSampler::Adaptive => tr!("tri-adaptive-quadtree"),
                TerrainSampler::Grid => tr!("tri-uniform-grid"),
            };
            MenuFieldCombo::new(
                "point_cloud_tin_sampler",
                tr!("tri-method"),
                &mut editor.point_cloud_tin_sampler,
                sampler_label,
                [
                    (TerrainSampler::Adaptive, tr!("tri-adaptive-quadtree").into()),
                    (TerrainSampler::Grid, tr!("tri-uniform-grid").into()),
                ],
            )
            .help_text(tr!("tri-adaptive-concentrates-vertices-compl"))
            .width(220.0)
            .show(ui);

            ui.add_space(4.0);
            ui.separator();

            // Budget: percentage (fractions allowed) or an absolute vertex count.
            let budget_label = if editor.point_cloud_tin_budget_is_percent {
                tr!("tri-percentage-cloud")
            } else {
                tr!("tri-vertex-count")
            };
            MenuFieldCombo::new(
                "point_cloud_tin_budget_mode",
                tr!("tri-budget"),
                &mut editor.point_cloud_tin_budget_is_percent,
                budget_label,
                [(true, tr!("tri-percentage-cloud").into()), (false, tr!("tri-vertex-count").into())],
            )
            .help_text(tr!("tri-cap-surface-share-source-points"))
            .width(220.0)
            .show(ui);

            if editor.point_cloud_tin_budget_is_percent {
                MenuFieldF64::new(tr!("tri-percentage"), &mut editor.point_cloud_tin_percent, 0.001..=100.0)
                    .help_text(tr!("tri-share-source-points-keep-fractions"))
                    .speed(0.05)
                    .suffix(tr!("tri-text"))
                    .max_decimals(3)
                    .width(220.0)
                    .show(ui);
            } else {
                MenuFieldU32::new(tr!("tri-vertex-count"), &mut editor.point_cloud_tin_limit, 3..=50_000_000)
                    .help_text(tr!("tri-exact-number-surface-vertices-target"))
                    .speed(1000.0)
                    .width(220.0)
                    .show(ui);
            }

            let budget = if editor.point_cloud_tin_budget_is_percent {
                TerrainBudget::Percent(editor.point_cloud_tin_percent)
            } else {
                TerrainBudget::Count(editor.point_cloud_tin_limit as usize)
            };
            // The job applies the budget to the points it surfaces, so with the
            // ground filter on, the share and the estimate are of ground alone.
            let surfaced_count = selected.map(|(id, _, point_count, _)| match editor.point_cloud_tin_ground_count {
                Some((ground_id, ground_count)) if ground_only && ground_id == id => ground_count,
                _ => point_count,
            });
            if let Some(point_count) = surfaced_count {
                let target = terrain_budget_target(point_count, budget);
                let percent = if point_count > 0 { target as f64 * 100.0 / point_count as f64 } else { 0.0 };
                tool_help_panel(
                    ui,
                    tr!(
                        "tri-up-target-point-count-points",
                        target = target.to_string(),
                        point_count = point_count.to_string(),
                        percent = format!("{percent:.3}")
                    ),
                );
            }

            if editor.point_cloud_tin_sampler == TerrainSampler::Adaptive {
                MenuFieldU32::new(tr!("tri-candidate-detail"), &mut editor.point_cloud_tin_candidate_mult, 1..=8)
                    .help_text(tr!("tri-candidate-fine-cells-per-budgeted"))
                    .suffix(tr!("drill-hole-text"))
                    .width(220.0)
                    .show(ui);
            }

            // Estimated transient memory, so an over-ambitious budget is
            // flagged before it risks the process rather than after.
            if let Some(point_count) = surfaced_count {
                const WARN_BYTES: u64 = 6 * 1024 * 1024 * 1024;
                let estimate = estimate_terrain_tin_memory_bytes(point_count, ground_only, budget, editor.point_cloud_tin_sampler, editor.point_cloud_tin_candidate_mult);
                if estimate >= WARN_BYTES {
                    let tail = tr!("tri-reduce-budget-candidate-detail-if");
                    egui::Frame::new()
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(tr!("tri-estimated-memory", estimate = format_bytes(estimate), detail = tail)).color(ui.visuals().warn_fg_color));
                        });
                }
            }

            ui.add_space(4.0);
            ui.separator();

            MenuFieldF64::new(tr!("tri-max-edge-length"), &mut editor.point_cloud_tin_max_edge, 0.0..=1_000_000.0)
                .help_text(tr!("tri-reject-reconstructed-triangle-edges"))
                .speed(1.0)
                .suffix(tr!("common-m"))
                .width(220.0)
                .show(ui);

            MenuFieldF64::new(tr!("tri-fill-holes-up"), &mut editor.point_cloud_tin_hole_fill, 0.0..=10_000.0)
                .help_text(tr!("tri-bridge-gaps-boundary-concavities-nar"))
                .speed(0.5)
                .suffix(tr!("common-m"))
                .max_decimals(2)
                .width(220.0)
                .show(ui);

            ui.add_space(4.0);
            ui.separator();

            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.point_cloud_tin_name_input)
                .help_text(tr!("tri-name-assigned-reconstructed-triangul"))
                .width(220.0)
                .hint_text(tr!("tri-type-open-surface"))
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let can_run = selected.is_some() && !editor.point_cloud_tin_name_input.trim().is_empty();
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-generate")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let Some((cloud_id, ..)) = selected
                {
                    commands.push(UiCommand::ExecutePointCloudTin {
                        cloud_id,
                        params: TerrainTinParams {
                            name: editor.point_cloud_tin_name_input.trim().to_owned(),
                            budget,
                            max_edge: editor.point_cloud_tin_max_edge,
                            sampler: editor.point_cloud_tin_sampler,
                            candidate_multiplier: editor.point_cloud_tin_candidate_mult,
                            hole_fill_distance: editor.point_cloud_tin_hole_fill,
                            ground_only,
                        },
                    });
                    editor.point_cloud_tin_open = false;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.point_cloud_tin_open = false;
                }
            });
        });
    if !open {
        editor.point_cloud_tin_open = false;
    }
}
