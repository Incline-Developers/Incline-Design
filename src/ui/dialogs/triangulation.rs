//! Triangulation creation and processing dialogs.

use std::num::NonZeroUsize;

use crate::{
    i18n::tr,
    model::{
        Document, Object, ObjectId,
        prismoid_grid::{CellValueRole, PrismoidGridId, ValuesMark, check_difference_name, check_marks},
        triangulation::{TriangulationId, same_quality_name},
    },
    rendering::color::{color32_to_rgba, rgba_to_color32},
    ui::{
        state::{
            ContourOutputLayer, EditorState, SelectionPickTool, SolidRegion, ToolSubject, TriCreatePhase, TriCutSource, TriLowerCut, TriPolylineClipMode, TriSurfaceCutSide,
            TriSurfaceType, TriUpperCut, TriangulationPickTarget, UiCommand, UiPrismoidGridEntry, UiProjectView, apply_strata_actions, refresh_strata_drafts,
        },
        widgets::{
            editable_grid::{DataGrid, GridButtons, GridColumn, GridRow, cell_choice, cell_text},
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldRange, MenuFieldText, MenuFieldU32, selected_source_field},
        },
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
/// Width of the prompt shown while a viewport pick waits. Fixed, so the name
/// under the cursor cannot resize it.
pub(super) const PICK_PROMPT_WIDTH: f32 = 360.0;
const PICKER_DIALOG_MIN_WIDTH: f32 = 430.0;
const PICKER_DIALOG_MAX_WIDTH: f32 = 450.0;
/// Room for Build Strata's three columns.
const STRATA_DIALOG_WIDTH: f32 = 560.0;

/// The cell sizes Build Prismoid Grid offers, as multiples of the surfaces'
/// spacing.
const PRISMOID_CELL_STEPS: [usize; 6] = [1, 2, 3, 4, 5, 10];

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
            // The name under the cursor changes as it moves, so the row keeps
            // its width and a long name is cut short rather than resizing
            // the window.
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(text).italics()).truncate());
        });
}

#[allow(clippy::too_many_arguments)]
fn triangulation_picker_field(
    ui: &mut egui::Ui,
    id_source: &'static str,
    label: impl Into<egui::WidgetText>,
    value: &mut Option<TriangulationId>,
    selected_text: impl Into<egui::WidgetText>,
    options: impl IntoIterator<Item = (Option<TriangulationId>, egui::WidgetText)>,
    help_text: impl Into<egui::WidgetText>,
    project: &UiProjectView,
) -> bool {
    let width = picker_control_width(ui);
    triangulation_picker_field_with_width(ui, id_source, label, value, selected_text, options, help_text, project, width)
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
    project: &UiProjectView,
    width: f32,
) -> bool {
    // An unloaded surface has no mesh to work on, so it is not offered. Say
    // so only when one is actually missing from the list.
    let some_unloaded = project.triangulations.iter().any(|entry| !entry.is_loaded);
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
                    if some_unloaded {
                        ui.separator();
                        ui.label(egui::RichText::new(tr!("triangulation-picker-loaded-only")).weak());
                    }
                });
            pick_clicked = ui
                .add(MenuButton::new(pick_label).min_width(pick_width))
                .hint(tr!("tri-choose-input-clicking-loaded-surface"))
                .clicked();
        })
        .response
    });
    pick_clicked
}

/// The surface a tool works on: the loaded ones in a list, beside a Pick
/// button for clicking one in the viewport. A surface selected when the tool
/// opened is already here. Returns the surface chosen from the list, and
/// whether Pick was pressed; the caller sends the first to the app, which
/// derives the tool's other fields from it.
fn subject_surface_field(
    ui: &mut egui::Ui,
    id_source: &'static str,
    current: Option<TriangulationId>,
    help_text: impl Into<egui::WidgetText>,
    project: &UiProjectView,
    width: f32,
) -> (Option<TriangulationId>, bool) {
    // A surface unloaded or deleted from under the dialog reads as none
    // chosen, which is a legitimate thing to have happened.
    let loaded = || project.triangulations.iter().filter(|entry| entry.is_loaded);
    let shown = current.filter(|id| loaded().any(|entry| entry.id == *id));
    let label = shown
        .and_then(|id| loaded().find(|entry| entry.id == id))
        .map_or_else(|| tr!("tri-select"), |entry| entry.name.clone());
    let mut value = shown;
    let pick = triangulation_picker_field_with_width(
        ui,
        id_source,
        tr!("tri-type-open-surface"),
        &mut value,
        label,
        loaded().map(|entry| (Some(entry.id), entry.name.as_str().into())),
        help_text,
        project,
        width,
    );
    (value.filter(|_| value != shown), pick)
}

/// A loaded prismoid grid's entry, or `None` once it was unloaded or deleted
/// while the dialog was open, as `surface_name` treats a surface: the run
/// button is disabled by then.
fn loaded_prismoid_grid(project: &UiProjectView, id: Option<PrismoidGridId>) -> Option<&UiPrismoidGridEntry> {
    id.and_then(|id| project.prismoid_grids.iter().find(|entry| entry.is_loaded && entry.id == id))
}

/// The prismoid grid a tool works on, from a list of the loaded ones; one
/// selected when the tool opened is already here. Returns the grid chosen,
/// for the caller to send to the app.
pub(crate) fn prismoid_grid_field(
    ui: &mut egui::Ui,
    id_source: &'static str,
    label: impl Into<egui::WidgetText>,
    current: Option<PrismoidGridId>,
    help_text: impl Into<egui::WidgetText>,
    project: &UiProjectView,
    width: f32,
) -> Option<PrismoidGridId> {
    let shown = loaded_prismoid_grid(project, current);
    let mut value = shown.map(|entry| entry.id);
    MenuFieldCombo::new(
        id_source,
        label,
        &mut value,
        shown.map_or_else(|| tr!("tri-select"), |entry| entry.name.clone()),
        project
            .prismoid_grids
            .iter()
            .filter(|entry| entry.is_loaded)
            .map(|entry| (Some(entry.id), entry.name.as_str().into())),
    )
    .help_text(help_text)
    .width(width)
    .show(ui);
    value.filter(|id| shown.is_none_or(|entry| entry.id != *id))
}

pub(crate) fn draw_triangulation_pick_prompt(ui: &mut egui::Ui, editor: &mut EditorState) {
    let Some(target) = editor.triangulation_pick_target else {
        return;
    };
    let mut open = true;
    DragableMenu::new("triangulation_pick_from_view_dialog", tr!("tri-pick-from-view"))
        .open(&mut open)
        .min_width(PICK_PROMPT_WIDTH)
        .max_width(PICK_PROMPT_WIDTH)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui.ctx(), |ui| {
            ui.label(target.prompt());
            let empty_text = if target.picks_boundary() {
                tr!("tri-move-cursor-over-closed-polyline")
            } else {
                ui.label(egui::RichText::new(tr!("tri-only-loaded-pickable")).weak());
                tr!("tri-move-cursor-over-loaded-surface")
            };
            ui.add_space(6.0);
            viewport_pick_status(ui, editor, &empty_text);
            ui.add_space(6.0);
            if ui.add(MenuButton::new(tr!("tri-cancel-pick"))).clicked() {
                editor.cancel_triangulation_pick();
            }
        });
    if !open {
        editor.cancel_triangulation_pick();
    }
}

/// Movable main dialog for the Create Triangulation workflow.
pub(crate) fn draw_tri_create_main_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    // Hidden while its Pick has the viewport.
    if !editor.tri_create_open || editor.tri_create_phase != TriCreatePhase::MainDialog || editor.selection_pick.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("create_triangulation_selection_dialog", tr!("tri-create-title"))
        .open(&mut open)
        .min_width(370.0)
        .show(ui.ctx(), |ui| {
            tool_help_panel(ui, tr!("tri-create-objects-help"));
            ui.add_space(4.0);

            // The objects the run works on: those selected when the dialog
            // opened, changed with Pick.
            let has_selection = !editor.tri_selected_object_ids.is_empty();
            let summary = if has_selection {
                tr!("tri-selection-selected", summary = selection_summary(&editor.tri_selected_object_ids, document))
            } else {
                tr!("ui-selected-objects", count = 0)
            };
            if menu::picked_selection_field(ui, tr!("sequence-objects"), summary, tr!("pick-selection-edges-help"), picker_control_width(ui)) {
                commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::CreateTriangulation));
            }

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
                .width(PICK_SELECTOR_WIDTH)
                .hint_text(tr!("tri-create-output-name-hint"))
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let ready = has_selection && !editor.tri_name_input.trim().is_empty();
                let confirm = menu::dialog_confirmed(ui.ctx());
                let cancel = menu::dialog_cancelled(ui.ctx());
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
                    if ui.add(MenuButton::new(tr!("tri-weld-retry")).primary()).clicked() || menu::dialog_confirmed(ui.ctx()) {
                        commands.push(UiCommand::ExecuteCreateTriangulationWithWeld {
                            name: failure.name.clone(),
                            object_ids: failure.object_ids.clone(),
                            surface_type: failure.surface_type,
                        });
                        dismiss = true;
                    }
                    if ui.add(MenuButton::new(tr!("survey-close"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                        dismiss = true;
                    }
                });
            } else if failure.upper_surface_retry_available {
                ui.colored_label(egui::Color32::LIGHT_RED, tr!("tri-breaklines-cross"));
                ui.add_space(4.0);
                ui.strong(tr!("tri-solution-generate-upper-surface"));
                ui.colored_label(egui::Color32::GRAY, tr!("tri-higher-edge-will-enforced-each"));
                menu::menu_actions(ui, |ui| {
                    if ui.add(MenuButton::new(tr!("tri-generate-upper-surface")).primary()).clicked() || menu::dialog_confirmed(ui.ctx()) {
                        commands.push(UiCommand::ExecuteCreateTriangulationUpperSurface {
                            name: failure.name.clone(),
                            object_ids: failure.object_ids.clone(),
                            surface_type: failure.surface_type,
                            coarse_weld: failure.coarse_weld_applied,
                        });
                        dismiss = true;
                    }
                    if ui.add(MenuButton::new(tr!("survey-close"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                        dismiss = true;
                    }
                });
            } else {
                ui.colored_label(egui::Color32::LIGHT_RED, &failure.message);
                // Only one button, so Enter and Escape both dismiss.
                let confirm = menu::dialog_confirmed(ui.ctx());
                let cancel = menu::dialog_cancelled(ui.ctx());
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
            let (chosen, pick) = subject_surface_field(ui, "clip_surface", editor.tri_cut_poly_tri_id, tr!("tri-clip-surface-help"), project, width);
            if let Some(id) = chosen {
                commands.push(UiCommand::SetToolSubject(ToolSubject::ClipSurface(id)));
            }
            if pick {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::ClipSurface);
            }

            ui.add_space(4.0);

            // Polylines carry no names to list, so the boundary is picked by
            // clicking it; one selected with the surface is already here.
            let boundary = if editor.tri_cut_poly_object_id.is_some() {
                editor.tri_cut_poly_object_name.clone()
            } else {
                tr!("tri-no-boundary-selected")
            };
            let pick_label = pick_button_label();
            let pick_width = pick_button_width(ui, &pick_label);
            MenuField::new(tr!("tri-boundary-polyline"))
                .help_text(tr!("tri-boundary-polyline-help"))
                .show(ui, |ui, row_height, _| {
                    ui.allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let name_width = width - ui.spacing().item_spacing.x - pick_width;
                        ui.allocate_ui_with_layout(egui::vec2(name_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(name_width);
                            ui.add(egui::Label::new(boundary.clone()).truncate());
                        });
                        if ui.add(MenuButton::new(pick_label).min_width(pick_width)).clicked() {
                            editor.triangulation_pick_target = Some(TriangulationPickTarget::ClipBoundary);
                        }
                    })
                    .response
                });

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
                    TriPolylineClipMode::KeepOutside => tr!("tri-remove-inside-help"),
                },
            );

            ui.add_space(4.0);

            // Output name
            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_poly_name_input)
                .help_text(tr!("tri-clip-creates-new-triangulation-name"))
                .width(width)
                .hint_text(tr!("tri-e-g-mysurf-cut"))
                .show(ui);
            MenuFieldBool::new(tr!("tri-unload-source-surface"), &mut editor.tri_cut_poly_unload_source).show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_cut_poly_tri_id.is_some() && editor.tri_cut_poly_object_id.is_some() && !editor.tri_cut_poly_name_input.trim().is_empty();

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
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
            let (chosen, pick) = subject_surface_field(ui, "slice_surface", editor.tri_cut_z_tri_id, tr!("tri-slice-surface-help"), project, width);
            if let Some(id) = chosen {
                commands.push(UiCommand::SetToolSubject(ToolSubject::SliceSurface(id)));
            }
            if pick {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::SliceSurface);
            }

            ui.add_space(4.0);

            let (z_low, z_high) = editor.tri_cut_z_limits;
            MenuFieldRange::new(
                tr!("tri-axis-range", axis = crate::model::survey::axis_name(2).to_string()),
                &mut editor.tri_cut_z_min_input,
                &mut editor.tri_cut_z_max_input,
                z_low..=z_high,
            )
            .width(width)
            .show(ui);

            ui.add_space(4.0);

            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_z_name_input)
                .width(width)
                .hint_text(tr!("tri-e-g-mysurf-slice"))
                .show(ui);
            MenuFieldBool::new(tr!("tri-unload-source-surface"), &mut editor.tri_cut_z_unload_source).show(ui);

            ui.add_space(6.0);
            ui.separator();

            let z_min = editor.tri_cut_z_min_input;
            let z_max = editor.tri_cut_z_max_input;
            let valid_z_range = z_min.is_finite() && z_max.is_finite() && z_min < z_max;
            let can_run = editor.tri_cut_z_tri_id.is_some() && valid_z_range && !editor.tri_cut_z_name_input.trim().is_empty();

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
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
                tr!("tri-reference-topology-help"),
                project,
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
                tr!("tri-target-surface-help"),
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::TrimSurface);
            }

            // Also when the dialog opened with the surface already chosen
            // from the selection, and so has no name yet.
            if (editor.tri_cut_surface_target_id != old_target || editor.tri_cut_surface_name_input.is_empty())
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
                tr!("tri-keep-surface-relation-help", relation = editor.tri_cut_surface_side.retained_relation().to_string()),
            );

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_surface_name_input)
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-e-g-design-trimmed"))
                .show(ui)
                .changed()
            {
                editor.tri_cut_surface_name_auto = false;
            }
            MenuFieldBool::new(tr!("tri-unload-source-surface"), &mut editor.tri_cut_surface_unload_source).show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_cut_surface_target_id.is_some()
                && editor.tri_cut_surface_reference_id.is_some()
                && editor.tri_cut_surface_target_id != editor.tri_cut_surface_reference_id
                && !editor.tri_cut_surface_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_cut_surface_open = false;
                }
            });
        });

    if !open {
        editor.tri_cut_surface_open = false;
    }
}

/// Clip to Surface: the seam whose roof and floor were selected when it
/// opened is clipped to the Keep below and Keep above limits, into a new
/// roof, floor and solid. Each limit is a surface, the usual choice, or a
/// typed RL; Keep above may also be a depth below a surface.
pub(crate) fn draw_cut_to_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_cut_to_open || editor.triangulation_pick_target.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("clip_to_surface_dialog", tr!("tri-clip-to-surface"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            // A surface being clipped is never offered as a cut.
            let cuts: Vec<(TriangulationId, &str)> = project
                .triangulations
                .iter()
                .filter(|entry| entry.is_loaded && !editor.tri_cut_to_targets.contains(&entry.id))
                .map(|entry| (entry.id, entry.name.as_str()))
                .collect();
            for value in [&mut editor.tri_cut_to_upper_id, &mut editor.tri_cut_to_lower_id] {
                if value.is_some_and(|id| !cuts.iter().any(|(cut, _)| *cut == id)) {
                    *value = None;
                }
            }
            let name_of = |id: Option<TriangulationId>| {
                id.and_then(|id| cuts.iter().find(|(cut, _)| *cut == id))
                    .map_or_else(|| tr!("common-none"), |(_, name)| (*name).to_owned())
            };
            let options = || std::iter::once((None, tr!("common-none").into())).chain(cuts.iter().map(|(id, name)| (Some(*id), (*name).into())));
            let source_label = |source: TriCutSource| match source {
                TriCutSource::Surface => tr!("tri-clip-to-surface-from-surface"),
                TriCutSource::Level => tr!("tri-clip-to-surface-from-level"),
                TriCutSource::Depth => tr!("tri-clip-to-surface-from-depth"),
            };
            // Empty text is an empty row; anything else must read as a number.
            let level_of = |input: &str| -> Result<Option<f64>, ()> {
                let input = input.trim();
                if input.is_empty() {
                    return Ok(None);
                }
                input.parse::<f64>().ok().filter(|level| level.is_finite()).map(Some).ok_or(())
            };

            selected_source_field(
                ui,
                tr!("tri-clip-to-surface-targets"),
                tr!("state-clip-to-surface-count", count = editor.tri_cut_to_targets.len().to_string()),
                tr!("tri-clip-to-surface-targets-help"),
                picker_control_width(ui),
            );
            ui.add_space(4.0);

            // Keep below: a surface or an RL. Ok(None) is a row left empty,
            // Err a row that cannot run as typed.
            let upper_source_text = source_label(editor.tri_cut_to_upper_source);
            MenuFieldCombo::new(
                "clip_to_surface_upper_source",
                tr!("tri-clip-to-surface-upper"),
                &mut editor.tri_cut_to_upper_source,
                upper_source_text,
                [TriCutSource::Surface, TriCutSource::Level].map(|source| (source, source_label(source).into())),
            )
            .help_text(tr!("tri-clip-to-surface-upper-help"))
            .width(picker_control_width(ui))
            .show(ui);
            let upper: Result<Option<TriUpperCut>, ()> = match editor.tri_cut_to_upper_source {
                TriCutSource::Level => {
                    MenuFieldText::new(tr!("tri-clip-to-surface-level"), &mut editor.tri_cut_to_upper_level_input)
                        .help_text(tr!("tri-clip-to-surface-level-help"))
                        .width(picker_control_width(ui))
                        .show(ui);
                    let level = level_of(&editor.tri_cut_to_upper_level_input);
                    if level.is_err() {
                        tool_help_panel(ui, tr!("tri-clip-to-surface-level-invalid"));
                    }
                    level.map(|level| level.map(TriUpperCut::Level))
                }
                TriCutSource::Surface | TriCutSource::Depth => {
                    let upper_label = name_of(editor.tri_cut_to_upper_id);
                    if triangulation_picker_field(
                        ui,
                        "clip_to_surface_upper",
                        tr!("tri-clip-to-surface-surface"),
                        &mut editor.tri_cut_to_upper_id,
                        upper_label,
                        options(),
                        tr!("tri-clip-to-surface-surface-help"),
                        project,
                    ) {
                        editor.triangulation_pick_target = Some(TriangulationPickTarget::ClipToUpper);
                    }
                    Ok(editor.tri_cut_to_upper_id.map(TriUpperCut::Surface))
                }
            };
            ui.add_space(4.0);

            // Keep above: a surface, an RL, or a depth below a surface.
            let lower_source_text = source_label(editor.tri_cut_to_lower_source);
            MenuFieldCombo::new(
                "clip_to_surface_lower_source",
                tr!("tri-clip-to-surface-lower"),
                &mut editor.tri_cut_to_lower_source,
                lower_source_text,
                [TriCutSource::Surface, TriCutSource::Level, TriCutSource::Depth].map(|source| (source, source_label(source).into())),
            )
            .help_text(tr!("tri-clip-to-surface-lower-help"))
            .width(picker_control_width(ui))
            .show(ui);
            let lower: Result<Option<TriLowerCut>, ()> = match editor.tri_cut_to_lower_source {
                TriCutSource::Level => {
                    MenuFieldText::new(tr!("tri-clip-to-surface-level"), &mut editor.tri_cut_to_lower_level_input)
                        .help_text(tr!("tri-clip-to-surface-level-help"))
                        .width(picker_control_width(ui))
                        .show(ui);
                    let level = level_of(&editor.tri_cut_to_lower_level_input);
                    if level.is_err() {
                        tool_help_panel(ui, tr!("tri-clip-to-surface-level-invalid"));
                    }
                    level.map(|level| level.map(TriLowerCut::Level))
                }
                TriCutSource::Surface | TriCutSource::Depth => {
                    let by_depth = editor.tri_cut_to_lower_source == TriCutSource::Depth;
                    let lower_label = name_of(editor.tri_cut_to_lower_id);
                    if triangulation_picker_field(
                        ui,
                        "clip_to_surface_lower",
                        tr!("tri-clip-to-surface-surface"),
                        &mut editor.tri_cut_to_lower_id,
                        lower_label,
                        options(),
                        if by_depth {
                            tr!("tri-clip-to-surface-ground-help")
                        } else {
                            tr!("tri-clip-to-surface-surface-help")
                        },
                        project,
                    ) {
                        editor.triangulation_pick_target = Some(TriangulationPickTarget::ClipToLower);
                    }
                    if by_depth {
                        MenuFieldText::new(tr!("tri-clip-to-surface-depth"), &mut editor.tri_cut_to_depth_input)
                            .help_text(tr!("tri-clip-to-surface-depth-help"))
                            .width(picker_control_width(ui))
                            .show(ui);
                        let depth = editor.tri_cut_to_depth_input.trim().parse::<f64>().ok().filter(|depth| depth.is_finite() && *depth > 0.0);
                        if depth.is_none() {
                            tool_help_panel(ui, tr!("project-cut-depth-positive"));
                        }
                        // A depth with no surface above it is an empty row.
                        match (editor.tri_cut_to_lower_id, depth) {
                            (None, _) => Ok(None),
                            (Some(ground), Some(depth)) => Ok(Some(TriLowerCut::Depth { ground, depth })),
                            (Some(_), None) => Err(()),
                        }
                    } else {
                        Ok(editor.tri_cut_to_lower_id.map(TriLowerCut::Surface))
                    }
                }
            };
            tool_help_panel(ui, tr!("tri-clip-to-surface-note"));

            // At least one row complete, and no row half typed.
            let can_run = matches!((upper, lower), (Ok(upper), Ok(lower)) if upper.is_some() || lower.is_some());

            ui.add_space(6.0);
            ui.separator();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-clip")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Ok(upper), Ok(lower)) = (upper, lower)
                {
                    commands.push(UiCommand::ExecuteCutTriangulationToSurface {
                        targets: editor.tri_cut_to_targets.clone(),
                        upper,
                        lower,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_cut_to_open = false;
                }
            });
        });

    if !open {
        editor.tri_cut_to_open = false;
    }
}

/// The Build Solid from Surfaces tool: pick a design surface and the
/// topography it meets, and get the closed volume between them.
///
/// The same construction the Solids Setup page previews, so a solid inspected
/// there and one built here are the same mesh.
pub(crate) fn draw_build_solid_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_solid_open || editor.triangulation_pick_target.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("build_solid_from_surfaces_dialog", tr!("tri-build-solid-surfaces"))
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

            let topography_id = editor.tri_solid_topography_id;
            let design_label = editor
                .tri_solid_design_id
                .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            let old_design = editor.tri_solid_design_id;
            if triangulation_picker_field(
                ui,
                "build_solid_design",
                tr!("tri-design-surface"),
                &mut editor.tri_solid_design_id,
                design_label,
                loaded.iter().filter(|(id, _)| Some(*id) != topography_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-pit-shell-dump-design"),
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::SolidDesign);
            }

            let design_id = editor.tri_solid_design_id;
            let topography_label = editor
                .tri_solid_topography_id
                .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("tri-select"));
            if triangulation_picker_field(
                ui,
                "build_solid_topography",
                tr!("solids-topography"),
                &mut editor.tri_solid_topography_id,
                topography_label,
                loaded.iter().filter(|(id, _)| Some(*id) != design_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-ground-design-measured-against"),
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::SolidTopography);
            }

            if editor.tri_solid_design_id != old_design
                && editor.tri_solid_name_auto
                && let Some(name) = editor
                    .tri_solid_design_id
                    .and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name))
            {
                editor.tri_solid_name_input = crate::app::canvas::derived_triangulation_name(name, &tr!("tri-type-solid-closed"));
            }

            ui.add_space(4.0);

            MenuField::new(tr!("tri-volume"))
                .help_text(tr!("tri-which-two-volumes-surfaces"))
                .show(ui, |ui, row_height, _| {
                    let (response, clicked) = centered_choice_buttons(ui, row_height, SolidRegion::ALL.map(|region| (region.label(), editor.tri_solid_region == region)));
                    if let Some(index) = clicked {
                        editor.tri_solid_region = SolidRegion::ALL[index];
                    }
                    response
                });

            tool_help_panel(ui, tr!("tri-encloses-over-area-two", region = editor.tri_solid_region.description().to_string()));

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_solid_name_input)
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-north-pit-solid"))
                .show(ui)
                .changed()
            {
                editor.tri_solid_name_auto = false;
            }

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_solid_design_id.is_some()
                && editor.tri_solid_topography_id.is_some()
                && editor.tri_solid_design_id != editor.tri_solid_topography_id
                && !editor.tri_solid_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-build")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(design_id), Some(topography_id)) = (editor.tri_solid_design_id, editor.tri_solid_topography_id)
                {
                    commands.push(UiCommand::ExecuteBuildSolidFromSurfaces {
                        design_id,
                        topography_id,
                        region: editor.tri_solid_region,
                        name: editor.tri_solid_name_input.trim().to_owned(),
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_solid_open = false;
                }
            });
        });

    if !open {
        editor.tri_solid_open = false;
    }
}

/// The Build Prismoid Grid tool: pick a roof and a floor on one grid and get
/// the cells between them.
pub(crate) fn draw_build_prismoid_grid_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_prismoid_open || editor.triangulation_pick_target.is_some() {
        return;
    }

    let mut open = true;
    DragableMenu::new("build_prismoid_grid_dialog", tr!("tri-build-prismoid-grid"))
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
            let loaded_name = |id: Option<TriangulationId>| id.and_then(|id| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name));

            tool_help_panel(ui, tr!("tri-build-prismoid-grid-help"));

            let floor_id = editor.tri_prismoid_floor_id;
            let roof_label = loaded_name(editor.tri_prismoid_roof_id).map_or_else(|| tr!("tri-select"), str::to_owned);
            let old_roof = editor.tri_prismoid_roof_id;
            if triangulation_picker_field(
                ui,
                "build_prismoid_roof",
                tr!("drill-hole-roof"),
                &mut editor.tri_prismoid_roof_id,
                roof_label,
                loaded.iter().filter(|(id, _)| Some(*id) != floor_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-prismoid-roof-help"),
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::PrismoidRoof);
            }

            let roof_id = editor.tri_prismoid_roof_id;
            let floor_label = loaded_name(editor.tri_prismoid_floor_id).map_or_else(|| tr!("tri-select"), str::to_owned);
            if triangulation_picker_field(
                ui,
                "build_prismoid_floor",
                tr!("drill-hole-floor"),
                &mut editor.tri_prismoid_floor_id,
                floor_label,
                loaded.iter().filter(|(id, _)| Some(*id) != roof_id).map(|(id, name)| (Some(*id), (*name).into())),
                tr!("tri-prismoid-floor-help"),
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::PrismoidFloor);
            }

            if editor.tri_prismoid_roof_id != old_roof
                && editor.tri_prismoid_name_auto
                && let Some(name) = loaded_name(editor.tri_prismoid_roof_id)
            {
                editor.tri_prismoid_name_input = crate::app::canvas::derived_triangulation_name(name, &tr!("common-prismoid-grid"));
            }

            ui.add_space(4.0);

            cell_size_field(ui, "build_prismoid_cell_size", &mut editor.tri_prismoid_step);

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_prismoid_name_input)
                .help_text(tr!("tri-name-assigned-prismoid-grid"))
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-prismoid-grid-name-hint"))
                .show(ui)
                .changed()
            {
                editor.tri_prismoid_name_auto = false;
            }

            ui.add_space(6.0);
            ui.separator();

            let can_run = loaded_name(editor.tri_prismoid_roof_id).is_some()
                && loaded_name(editor.tri_prismoid_floor_id).is_some()
                && editor.tri_prismoid_roof_id != editor.tri_prismoid_floor_id
                && !editor.tri_prismoid_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-build")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(roof_id), Some(floor_id)) = (editor.tri_prismoid_roof_id, editor.tri_prismoid_floor_id)
                {
                    commands.push(UiCommand::ExecuteBuildPrismoidGrid {
                        roof_id,
                        floor_id,
                        step: editor.tri_prismoid_step,
                        name: editor.tri_prismoid_name_input.trim().to_owned(),
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_prismoid_open = false;
                }
            });
        });

    if !open {
        editor.tri_prismoid_open = false;
    }
}

/// The cell size of a prismoid, as a whole multiple of the surfaces'
/// spacing.
fn cell_size_field(ui: &mut egui::Ui, id: &str, step: &mut NonZeroUsize) {
    let step_label = |step: NonZeroUsize| match step.get() {
        1 => tr!("tri-prismoid-cell-size-surfaces"),
        times => tr!("tri-prismoid-cell-size-times", times = times.to_string()),
    };
    let selected = step_label(*step);
    MenuFieldCombo::new(
        id,
        tr!("tri-prismoid-cell-size"),
        step,
        selected,
        PRISMOID_CELL_STEPS.into_iter().filter_map(NonZeroUsize::new).map(|step| (step, step_label(step).into())),
    )
    .help_text(tr!("tri-prismoid-cell-size-help"))
    .width(picker_control_width(ui))
    .show(ui);
}

/// The Build Strata tool: horizons listed top down, each a surface, its name
/// and the name of the interval below it, built into a prismoid grid for
/// every interval.
pub(crate) fn draw_build_strata_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_strata_open {
        return;
    }

    let mut open = true;
    DragableMenu::new("build_strata_dialog", tr!("tri-build-strata"))
        .open(&mut open)
        .min_width(STRATA_DIALOG_WIDTH)
        .max_width(STRATA_DIALOG_WIDTH)
        .show(ui.ctx(), |ui| {
            let loaded: Vec<(TriangulationId, &str)> = project
                .triangulations
                .iter()
                .filter(|entry| entry.is_loaded)
                .map(|entry| (entry.id, entry.name.as_str()))
                .collect();
            let loaded_name = |id: TriangulationId| loaded.iter().find(|(loaded_id, _)| *loaded_id == id).map(|(_, name)| *name);
            let surface_name = |id: TriangulationId| loaded_name(id).map(str::to_owned);

            tool_help_panel(ui, tr!("tri-build-strata-help"));

            refresh_strata_drafts(&mut editor.tri_strata_rows, &mut editor.tri_strata_intervals, surface_name);
            let (rows, intervals) = (&mut editor.tri_strata_rows, &mut editor.tri_strata_intervals);
            let chosen: Vec<Option<TriangulationId>> = rows.iter().map(|row| row.surface).collect();
            let grid_rows = vec![GridRow::Item; rows.len()];
            let columns = vec![
                GridColumn::new(tr!("tri-strata-surface"), 35.0),
                GridColumn::new(tr!("tri-strata-horizon"), 30.0),
                GridColumn::new(tr!("tri-strata-interval-below"), 35.0),
            ];
            let actions = DataGrid::new("build_strata_horizons", columns).buttons(GridButtons::default().reorder()).show(
                ui,
                &mut editor.tri_strata_selected,
                &grid_rows,
                |ui, row, column| match column {
                    0 => {
                        // Each surface is offered once across the list.
                        let choices: Vec<_> = std::iter::once(None)
                            .chain(loaded.iter().map(|(id, _)| Some(*id)).filter(|id| *id == chosen[row] || !chosen.contains(id)))
                            .collect();
                        let label = |id: Option<TriangulationId>| id.and_then(loaded_name).map_or_else(|| tr!("tri-select"), str::to_owned);
                        cell_choice(ui, ("build_strata_surface", row), &mut rows[row].surface, &choices, label);
                    }
                    1 => {
                        let before = rows[row].horizon.clone();
                        cell_text(ui, &mut rows[row].horizon, true);
                        if rows[row].horizon != before {
                            rows[row].horizon_typed = true;
                        }
                    }
                    _ => match intervals.get_mut(row) {
                        Some(interval) => {
                            let before = interval.name.clone();
                            cell_text(ui, &mut interval.name, true);
                            if interval.name != before {
                                interval.typed = true;
                            }
                        }
                        None => {
                            ui.label(tr!("inventory-portion-missing"));
                        }
                    },
                },
            );
            apply_strata_actions(rows, intervals, &mut editor.tri_strata_selected, actions);
            refresh_strata_drafts(rows, intervals, surface_name);
            menu::menu_note(ui, tr!("tri-strata-list-help"));

            ui.add_space(4.0);

            cell_size_field(ui, "build_strata_cell_size", &mut editor.tri_strata_step);

            if editor.tri_strata_name_auto {
                editor.tri_strata_name_input = editor
                    .tri_strata_rows
                    .first()
                    .and_then(|row| row.surface)
                    .and_then(loaded_name)
                    .map(|top| crate::app::canvas::derived_triangulation_name(top, &tr!("common-strata")))
                    .unwrap_or_default();
            }
            if MenuFieldText::new(tr!("tri-strata-name"), &mut editor.tri_strata_name_input)
                .help_text(tr!("tri-strata-name-help"))
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-strata-name-hint"))
                .show(ui)
                .changed()
            {
                editor.tri_strata_name_auto = false;
            }

            ui.add_space(6.0);
            ui.separator();

            let rows = &editor.tri_strata_rows;
            let surfaces_once = rows
                .iter()
                .enumerate()
                .all(|(index, row)| row.surface.and_then(loaded_name).is_some() && !rows[..index].iter().any(|earlier| earlier.surface == row.surface));
            let horizons_once = rows
                .iter()
                .enumerate()
                .all(|(index, row)| !row.horizon.trim().is_empty() && !rows[..index].iter().any(|earlier| earlier.horizon.trim() == row.horizon.trim()));
            let can_run = rows.len() >= 2
                && surfaces_once
                && horizons_once
                && editor.tri_strata_intervals.iter().all(|interval| !interval.name.trim().is_empty())
                && !editor.tri_strata_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if ui.add(MenuButton::new(tr!("tri-build")).primary().enabled(can_run)).clicked() || (confirm && can_run) {
                    commands.push(UiCommand::ExecuteBuildStrata {
                        horizons: rows.iter().filter_map(|row| Some((row.surface?, row.horizon.trim().to_owned()))).collect(),
                        intervals: editor.tri_strata_intervals.iter().map(|interval| interval.name.trim().to_owned()).collect(),
                        step: editor.tri_strata_step,
                        name: editor.tri_strata_name_input.trim().to_owned(),
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_strata_open = false;
                }
            });
        });

    if !open {
        editor.tri_strata_open = false;
    }
}

/// Add Qualities: the prismoid grid, the quality surfaces read into it, each
/// ticked by hand, and the ticked one chosen as its density.
pub(crate) fn draw_add_prismoid_qualities_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_prismoid_qualities_open {
        return;
    }

    let mut open = true;
    DragableMenu::new("add_prismoid_qualities_dialog", tr!("tri-add-prismoid-qualities"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let grid = loaded_prismoid_grid(project, editor.tri_prismoid_qualities_grid);
            let marked: Vec<_> = project
                .triangulations
                .iter()
                .filter_map(|entry| entry.quality.as_ref().map(|quality| (entry, quality)))
                .collect();
            editor.tri_prismoid_qualities_ticked.retain(|id| marked.iter().any(|(entry, _)| entry.id == *id));

            tool_help_panel(ui, tr!("tri-add-prismoid-qualities-help"));
            if let Some(id) = prismoid_grid_field(
                ui,
                "add_prismoid_qualities_grid",
                tr!("common-prismoid-grid"),
                editor.tri_prismoid_qualities_grid,
                tr!("tri-prismoid-qualities-grid-help"),
                project,
                picker_control_width(ui),
            ) {
                commands.push(UiCommand::SetToolSubject(ToolSubject::PrismoidQualities(id)));
            }

            menu::menu_section(ui, tr!("tri-prismoid-qualities-surfaces"));
            if marked.is_empty() {
                menu::menu_note(ui, tr!("tri-prismoid-qualities-none"));
            }
            for (entry, quality) in &marked {
                let was = editor.tri_prismoid_qualities_ticked.contains(&entry.id);
                let mut ticked = was;
                let row = tr!(
                    "tri-prismoid-qualities-row",
                    surface = entry.name.clone(),
                    quality = quality.name.clone(),
                    unit = quality.unit.clone()
                );
                MenuFieldBool::new(row.trim_end(), &mut ticked).show(ui);
                if ticked && !was {
                    editor.tri_prismoid_qualities_ticked.push(entry.id);
                } else if was && !ticked {
                    editor.tri_prismoid_qualities_ticked.retain(|id| *id != entry.id);
                }
                let notes: Vec<String> = [
                    (editor.tri_prismoid_qualities_density == Some(entry.id)).then(|| tr!("tri-prismoid-qualities-density")),
                    (!entry.is_loaded).then(|| tr!("tri-prismoid-qualities-loads")),
                    grid.is_some_and(|grid| grid.values.iter().any(|(name, _)| same_quality_name(name, &quality.name)))
                        .then(|| tr!("tri-prismoid-qualities-replaces")),
                ]
                .into_iter()
                .flatten()
                .collect();
                if !notes.is_empty() {
                    menu::menu_note(ui, notes.join(", "));
                }
            }

            let ticked: Vec<(TriangulationId, &str)> = editor
                .tri_prismoid_qualities_ticked
                .iter()
                .filter_map(|id| marked.iter().find(|(entry, _)| entry.id == *id).map(|(_, quality)| (*id, quality.name.as_str())))
                .collect();
            if editor.tri_prismoid_qualities_density.is_some_and(|density| !ticked.iter().any(|(id, _)| *id == density)) {
                editor.tri_prismoid_qualities_density = None;
            }
            let surface_name = |id: TriangulationId| marked.iter().find(|(entry, _)| entry.id == id).map_or("", |(entry, _)| entry.name.as_str());
            let chosen = editor.tri_prismoid_qualities_density.map(surface_name);
            ui.add_space(4.0);
            ui.add_enabled_ui(!ticked.is_empty(), |ui| {
                MenuFieldCombo::new(
                    "add_prismoid_qualities_density",
                    tr!("tri-prismoid-qualities-density-field"),
                    &mut editor.tri_prismoid_qualities_density,
                    chosen.map_or_else(|| tr!("common-none"), str::to_owned),
                    std::iter::once((None, tr!("common-none").into())).chain(ticked.iter().map(|(id, _)| (Some(*id), surface_name(*id).to_owned().into()))),
                )
                .help_text(tr!("tri-prismoid-qualities-density-field-help"))
                .width(picker_control_width(ui))
                .show(ui);
            });

            // A density the grid holds stays unless a ticked surface of its
            // quality replaces it.
            let held = grid
                .and_then(|grid| grid.values.iter().find(|(_, role)| *role == CellValueRole::Density))
                .map(|(name, _)| name.as_str())
                .filter(|name| !ticked.iter().any(|(_, quality)| same_quality_name(quality, name)));
            let basis = match editor.tri_prismoid_qualities_density.map(surface_name).or(held) {
                Some(density) => tr!("tri-prismoid-qualities-by-tonnes", density = density.to_owned()),
                None => tr!("tri-prismoid-qualities-by-volume"),
            };
            menu::menu_note(ui, basis);

            // The refusals the marks alone decide, said before the work starts.
            let refusal = grid.and_then(|grid| {
                let held: Vec<_> = grid.values.iter().map(|(name, role)| (name.as_str(), *role)).collect();
                let marks: Vec<_> = ticked
                    .iter()
                    .filter_map(|(id, _)| marked.iter().find(|(entry, _)| entry.id == *id))
                    .map(|(entry, quality)| ValuesMark {
                        surface: &entry.name,
                        name: &quality.name,
                        unit: &quality.unit,
                        role: if editor.tri_prismoid_qualities_density == Some(entry.id) {
                            CellValueRole::Density
                        } else {
                            CellValueRole::Quality
                        },
                    })
                    .collect();
                check_marks(&held, &marks)
                    .err()
                    .map(|refusal| crate::app::commands::prismoid_grid::values_refusal_words(refusal, &grid.name))
            });
            if let Some(refusal) = &refusal {
                menu::menu_note(ui, refusal.as_str());
            }

            ui.add_space(6.0);
            ui.separator();

            let can_run = grid.is_some() && !ticked.is_empty() && refusal.is_none();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-add")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let Some(grid) = editor.tri_prismoid_qualities_grid
                {
                    commands.push(UiCommand::ExecuteAddPrismoidQualities {
                        grid,
                        surfaces: editor.tri_prismoid_qualities_ticked.clone(),
                        density: editor.tri_prismoid_qualities_density,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_prismoid_qualities_open = false;
                }
            });
        });

    if !open {
        editor.tri_prismoid_qualities_open = false;
    }
}

/// Prismoid Grid to Surface: the grid, the value its surface's height is
/// (vertical thickness, or one of the grid's values) and the surface's
/// name, derived from the grid and the value until typed.
pub(crate) fn draw_prismoid_grid_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_prismoid_surface_open {
        return;
    }

    let mut open = true;
    DragableMenu::new("prismoid_grid_surface_dialog", tr!("tri-prismoid-grid-surface"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let width = picker_control_width(ui);
            let grid = loaded_prismoid_grid(project, editor.tri_prismoid_surface_grid);
            let values: Vec<&str> = grid.map(|grid| grid.values.iter().map(|(name, _)| name.as_str()).collect()).unwrap_or_default();
            if editor
                .tri_prismoid_surface_value
                .as_deref()
                .is_some_and(|value| !values.iter().any(|name| same_quality_name(name, value)))
            {
                editor.tri_prismoid_surface_value = None;
            }

            tool_help_panel(ui, tr!("tri-prismoid-grid-surface-help"));
            if let Some(id) = prismoid_grid_field(
                ui,
                "prismoid_grid_surface_grid",
                tr!("common-prismoid-grid"),
                editor.tri_prismoid_surface_grid,
                tr!("tri-prismoid-grid-surface-grid-help"),
                project,
                width,
            ) {
                commands.push(UiCommand::SetToolSubject(ToolSubject::PrismoidSurface(id)));
            }

            ui.add_space(4.0);

            let thickness = tr!("thickness-points-column-vertical");
            let chosen = editor.tri_prismoid_surface_value.clone().unwrap_or_else(|| thickness.clone());
            MenuFieldCombo::new(
                "prismoid_grid_surface_value",
                tr!("tri-prismoid-grid-surface-value"),
                &mut editor.tri_prismoid_surface_value,
                chosen.clone(),
                std::iter::once((None, thickness.into())).chain(values.iter().map(|name| (Some((*name).to_owned()), (*name).into()))),
            )
            .width(width)
            .show(ui);

            if editor.tri_prismoid_surface_name_auto
                && let Some(grid) = grid
            {
                let derived = crate::app::canvas::derived_triangulation_name(&grid.name, &chosen);
                if editor.tri_prismoid_surface_name_input != derived {
                    editor.tri_prismoid_surface_name_input = derived;
                }
            }
            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_prismoid_surface_name_input)
                .width(width)
                .show(ui)
                .changed()
            {
                editor.tri_prismoid_surface_name_auto = false;
            }

            ui.add_space(6.0);
            ui.separator();

            let can_run = grid.is_some() && !editor.tri_prismoid_surface_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("common-ok")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let Some(grid) = editor.tri_prismoid_surface_grid
                {
                    commands.push(UiCommand::ExecutePrismoidGridToSurface {
                        grid,
                        value: editor.tri_prismoid_surface_value.clone(),
                        name: editor.tri_prismoid_surface_name_input.trim().to_owned(),
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_prismoid_surface_open = false;
                }
            });
        });

    if !open {
        editor.tri_prismoid_surface_open = false;
    }
}

/// Difference between Prismoid Grids: this grid and the other, pre-set from
/// the selection and swappable over the loaded grids, the quantity compared
/// (vertical thickness, or a value both hold) and the name the difference
/// is kept under on this grid, derived from the quantity until typed.
pub(crate) fn draw_difference_prismoid_grids_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_prismoid_difference_open {
        return;
    }

    let mut open = true;
    DragableMenu::new("difference_prismoid_grids_dialog", tr!("tri-difference-prismoid-grids"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            let width = picker_control_width(ui);
            let grids: Vec<&UiPrismoidGridEntry> = project.prismoid_grids.iter().filter(|entry| entry.is_loaded).collect();
            let this = loaded_prismoid_grid(project, editor.tri_prismoid_difference_grid);
            let other = loaded_prismoid_grid(project, editor.tri_prismoid_difference_other);
            // The values both grids hold, by this grid's names.
            let shared: Vec<&str> = match (this, other) {
                (Some(this), Some(other)) => this
                    .values
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .filter(|name| other.values.iter().any(|(held, _)| same_quality_name(held, name)))
                    .collect(),
                _ => Vec::new(),
            };
            if editor
                .tri_prismoid_difference_value
                .as_deref()
                .is_some_and(|value| !shared.iter().any(|name| same_quality_name(name, value)))
            {
                editor.tri_prismoid_difference_value = None;
            }

            tool_help_panel(ui, tr!("tri-difference-prismoid-grids-help"));
            let grid_name = |entry: Option<&UiPrismoidGridEntry>| entry.map_or_else(|| tr!("tri-select"), |entry| entry.name.clone());
            MenuFieldCombo::new(
                "difference_prismoid_grids_this",
                tr!("tri-prismoid-this-grid"),
                &mut editor.tri_prismoid_difference_grid,
                grid_name(this),
                grids.iter().map(|entry| (Some(entry.id), entry.name.clone().into())),
            )
            .width(width)
            .show(ui);
            MenuFieldCombo::new(
                "difference_prismoid_grids_other",
                tr!("tri-prismoid-other-grid"),
                &mut editor.tri_prismoid_difference_other,
                grid_name(other),
                grids.iter().map(|entry| (Some(entry.id), entry.name.clone().into())),
            )
            .width(width)
            .show(ui);

            ui.add_space(4.0);

            let thickness = tr!("thickness-points-column-vertical");
            let chosen = editor.tri_prismoid_difference_value.clone().unwrap_or_else(|| thickness.clone());
            MenuFieldCombo::new(
                "difference_prismoid_grids_quantity",
                tr!("tri-prismoid-quantity"),
                &mut editor.tri_prismoid_difference_value,
                chosen.clone(),
                std::iter::once((None, thickness.into())).chain(shared.iter().map(|name| (Some((*name).to_owned()), (*name).into()))),
            )
            .width(width)
            .show(ui);

            if editor.tri_prismoid_difference_name_auto {
                let derived = tr!("tri-prismoid-difference-default", quantity = chosen);
                if editor.tri_prismoid_difference_name_input != derived {
                    editor.tri_prismoid_difference_name_input = derived;
                }
            }
            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_prismoid_difference_name_input)
                .width(width)
                .show(ui)
                .changed()
            {
                editor.tri_prismoid_difference_name_auto = false;
            }

            // The name's refusal, said before the work starts.
            let refusal = difference_name_refusal(this, other, &editor.tri_prismoid_difference_name_input);
            if let Some(refusal) = &refusal {
                menu::menu_note(ui, refusal.as_str());
            }

            ui.add_space(6.0);
            ui.separator();

            let loaded = |id: Option<PrismoidGridId>| id.filter(|id| grids.iter().any(|entry| entry.id == *id));
            let pair = match (loaded(editor.tri_prismoid_difference_grid), loaded(editor.tri_prismoid_difference_other)) {
                (Some(grid), Some(other)) if grid != other => Some((grid, other)),
                _ => None,
            };
            let can_run = pair.is_some() && refusal.is_none() && !editor.tri_prismoid_difference_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("common-ok")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let Some((grid, other)) = pair
                {
                    commands.push(UiCommand::ExecuteDifferencePrismoidGrids {
                        grid,
                        other,
                        value: editor.tri_prismoid_difference_value.clone(),
                        name: editor.tri_prismoid_difference_name_input.trim().to_owned(),
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_prismoid_difference_open = false;
                }
            });
        });

    if !open {
        editor.tri_prismoid_difference_open = false;
    }
}

/// Why a difference of `this` grid less `other` cannot be kept as `name`,
/// in words: a name the grid keeps for its own values, or its density's.
fn difference_name_refusal(this: Option<&UiPrismoidGridEntry>, other: Option<&UiPrismoidGridEntry>, name: &str) -> Option<String> {
    let (this, other) = (this?, other?);
    let held: Vec<_> = this.values.iter().map(|(name, role)| (name.as_str(), *role)).collect();
    check_difference_name(&held, &other.name, name.trim())
        .err()
        .map(|refusal| crate::app::commands::prismoid_grid::values_refusal_words(refusal, &this.name))
}

/// Mask Prismoid Grid by Polyline: the grid, the closed polyline and the
/// side kept, as the polyline cut takes them; nothing new is made, so there
/// is no name.
pub(crate) fn draw_mask_prismoid_grid_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_prismoid_mask_open || editor.triangulation_pick_target.is_some() {
        return;
    }
    if let Some(object_id) = editor.tri_prismoid_mask_object_id
        && !document.get_object(object_id).is_some_and(Object::encloses_area)
    {
        editor.tri_prismoid_mask_object_id = None;
        editor.tri_prismoid_mask_object_name.clear();
        if editor.tool_highlight_id == Some(object_id) {
            editor.tool_highlight_id = None;
        }
    }

    let mut open = true;
    DragableMenu::new("mask_prismoid_grid_by_polyline_dialog", tr!("tri-mask-prismoid-grid-polyline"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui.ctx(), |ui| {
            let width = picker_control_width(ui);
            let grid = loaded_prismoid_grid(project, editor.tri_prismoid_mask_grid);
            if let Some(id) = prismoid_grid_field(
                ui,
                "mask_prismoid_grid_grid",
                tr!("common-prismoid-grid"),
                editor.tri_prismoid_mask_grid,
                tr!("tri-mask-grid-help"),
                project,
                width,
            ) {
                commands.push(UiCommand::SetToolSubject(ToolSubject::MaskGrid(id)));
            }

            ui.add_space(4.0);

            // Polylines carry no names to list, so the boundary is picked by
            // clicking it; one selected with the grid is already here.
            let boundary = if editor.tri_prismoid_mask_object_id.is_some() {
                editor.tri_prismoid_mask_object_name.clone()
            } else {
                tr!("tri-no-boundary-selected")
            };
            let pick_label = pick_button_label();
            let pick_width = pick_button_width(ui, &pick_label);
            MenuField::new(tr!("tri-boundary-polyline"))
                .help_text(tr!("tri-mask-boundary-help"))
                .show(ui, |ui, row_height, _| {
                    ui.allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let name_width = width - ui.spacing().item_spacing.x - pick_width;
                        ui.allocate_ui_with_layout(egui::vec2(name_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(name_width);
                            ui.add(egui::Label::new(boundary.clone()).truncate());
                        });
                        if ui.add(MenuButton::new(pick_label).min_width(pick_width)).clicked() {
                            editor.triangulation_pick_target = Some(TriangulationPickTarget::PrismoidMaskBoundary);
                        }
                    })
                    .response
                });

            ui.add_space(4.0);

            MenuField::new(tr!("tri-result")).show(ui, |ui, row_height, _| {
                let (response, clicked) = centered_choice_buttons(
                    ui,
                    row_height,
                    [
                        (TriPolylineClipMode::KeepInside.label(), editor.tri_prismoid_mask_mode == TriPolylineClipMode::KeepInside),
                        (TriPolylineClipMode::KeepOutside.label(), editor.tri_prismoid_mask_mode == TriPolylineClipMode::KeepOutside),
                    ],
                );
                if clicked == Some(0) {
                    editor.tri_prismoid_mask_mode = TriPolylineClipMode::KeepInside;
                } else if clicked == Some(1) {
                    editor.tri_prismoid_mask_mode = TriPolylineClipMode::KeepOutside;
                }
                response
            });
            tool_help_panel(
                ui,
                match editor.tri_prismoid_mask_mode {
                    TriPolylineClipMode::KeepInside => tr!("tri-mask-keep-inside-help"),
                    TriPolylineClipMode::KeepOutside => tr!("tri-mask-keep-outside-help"),
                },
            );

            ui.add_space(6.0);
            ui.separator();

            let can_run = grid.is_some() && editor.tri_prismoid_mask_object_id.is_some();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("tri-mask")).primary().enabled(can_run)).clicked() || (confirm && can_run))
                    && let (Some(grid), Some(polyline_id)) = (editor.tri_prismoid_mask_grid, editor.tri_prismoid_mask_object_id)
                {
                    commands.push(UiCommand::ExecuteMaskPrismoidGridByPolyline {
                        grid,
                        polyline_id,
                        mode: editor.tri_prismoid_mask_mode,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_prismoid_mask_open = false;
                }
            });
        });

    if !open {
        editor.tri_prismoid_mask_open = false;
    }
}

/// Create Block Model from Prismoid Grid: only the grid, whose prismoids
/// become the blocks.
pub(crate) fn draw_prismoid_block_model_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.tri_prismoid_block_open {
        return;
    }

    let mut open = true;
    DragableMenu::new("prismoid_block_model_dialog", tr!("common-create-block-model-from-prismoid-grid"))
        .open(&mut open)
        .min_width(PICKER_DIALOG_MIN_WIDTH)
        .max_width(PICKER_DIALOG_MAX_WIDTH)
        .show(ui.ctx(), |ui| {
            if let Some(id) = prismoid_grid_field(
                ui,
                "prismoid_block_model_grid",
                tr!("common-prismoid-grid"),
                editor.tri_prismoid_block_grid,
                tr!("tri-prismoid-block-model-grid-help"),
                project,
                picker_control_width(ui),
            ) {
                commands.push(UiCommand::SetToolSubject(ToolSubject::PrismoidBlockModel(id)));
            }

            ui.add_space(6.0);
            ui.separator();

            let grid = loaded_prismoid_grid(project, editor.tri_prismoid_block_grid).map(|entry| entry.id);
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("common-create")).primary().enabled(grid.is_some())).clicked() || confirm)
                    && let Some(grid) = grid
                {
                    commands.push(UiCommand::CreateBlockModelFromPrismoidGrid(grid));
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.tri_prismoid_block_open = false;
                }
            });
        });

    if !open {
        editor.tri_prismoid_block_open = false;
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
                project,
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
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::CutPitShell);
            }

            ui.add_space(4.0);
            tool_help_panel(ui, tr!("tri-removes-topology-where-pit-shell"));

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_cut_pitshell_name_input)
                .width(picker_control_width(ui))
                .hint_text(tr!("tri-e-g-topo-cut"))
                .show(ui)
                .changed()
            {
                editor.tri_cut_pitshell_name_auto = false;
            }
            MenuFieldBool::new(tr!("tri-unload-source-topology"), &mut editor.tri_cut_pitshell_unload_source).show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_cut_pitshell_topology_id.is_some() && editor.tri_cut_pitshell_pitshell_id.is_some() && !editor.tri_cut_pitshell_name_input.trim().is_empty();

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
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
                project,
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
                project,
            ) {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::IncludeShape);
            }

            if MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.tri_include_solid_name_input)
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
            MenuFieldBool::new(tr!("tri-hide-unload-sources"), &mut editor.tri_include_solid_hide_old).show(ui);

            ui.add_space(6.0);
            ui.separator();

            let can_run = editor.tri_include_solid_topology_id.is_some()
                && editor.tri_include_solid_shape_id.is_some()
                && editor.tri_include_solid_topology_id != editor.tri_include_solid_shape_id
                && !editor.tri_include_solid_name_input.trim().is_empty();
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
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
            let (chosen, pick) = subject_surface_field(ui, "contour_surface", editor.tri_contour_tri_id, tr!("tri-contour-surface-help"), project, control_width);
            if let Some(id) = chosen {
                commands.push(UiCommand::SetToolSubject(ToolSubject::ContourSurface(id)));
            }
            if pick {
                editor.triangulation_pick_target = Some(TriangulationPickTarget::ContourSurface);
            }

            ui.add_space(4.0);

            let mut minor_color = rgba_to_color32(editor.tri_contour_minor_color);
            let mut major_color = rgba_to_color32(editor.tri_contour_major_color);
            MenuField::new(tr!("tri-intervals-colours"))
                .help_text(tr!("tri-contour-interval-help"))
                .show(ui, |ui, row_height, _| {
                    let width = control_width;
                    let gap = ui.spacing().item_spacing.x;
                    let colour_width = ui.spacing().interact_size.x;
                    let value_width = (width - colour_width * 2.0 - gap * 3.0) * 0.5;
                    ui.allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
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
                    })
                    .response
                });
            editor.tri_contour_minor_color = color32_to_rgba(minor_color);
            editor.tri_contour_major_color = color32_to_rgba(major_color);

            MenuFieldBool::new(tr!("tri-limit-z-range"), &mut editor.tri_contour_use_z_range).show(ui);
            if editor.tri_contour_use_z_range {
                let (z_low, z_high) = editor.tri_contour_z_limits;
                MenuFieldRange::new(
                    tr!("tri-axis-range", axis = crate::model::survey::axis_name(2).to_string()),
                    &mut editor.tri_contour_z_min_input,
                    &mut editor.tri_contour_z_max_input,
                    z_low..=z_high,
                )
                .width(control_width)
                .show(ui);
            }

            ui.add_space(4.0);

            let active_project = project.projects.iter().find(|entry| entry.is_active);
            let active_layers = active_project.map(|entry| entry.layers.as_slice()).unwrap_or(&[]);
            if editor.tri_contour_target_layer.is_some_and(|id| !active_layers.iter().any(|layer| layer.id == id)) {
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
                std::iter::once((None, tr!("tri-new-layer").into())).chain(active_layers.iter().map(|layer| (Some(layer.id), layer.name.clone().into()))),
            )
            .width(control_width)
            .show(ui);

            if editor.tri_contour_target_layer.is_none()
                && MenuFieldText::new(tr!("tri-new-layer-name"), &mut editor.tri_contour_layer_name_input)
                    .width(control_width)
                    .hint_text(tr!("tri-e-g-surface-contour"))
                    .show(ui)
                    .changed()
            {
                editor.tri_contour_layer_name_auto = false;
            }
            let new_layer_name = editor.tri_contour_layer_name_input.trim().to_owned();
            let new_layer_name_conflicts = editor.tri_contour_target_layer.is_none() && active_layers.iter().any(|layer| layer.name == new_layer_name);
            if editor.tri_contour_target_layer.is_none() && new_layer_name_conflicts {
                ui.colored_label(egui::Color32::LIGHT_RED, tr!("tri-layer-already-exists-select-above"));
            }
            MenuFieldBool::new(tr!("tri-contour-labels"), &mut editor.tri_contour_labels)
                .help_text(tr!("tri-contour-labels-help"))
                .show(ui);

            ui.add_space(6.0);
            ui.separator();

            let minor_interval = editor.tri_contour_minor_interval_input;
            let major_interval = editor.tri_contour_major_interval_input;
            let valid_intervals = minor_interval.is_finite() && major_interval.is_finite() && minor_interval >= 1e-6 && major_interval >= 1e-6 && major_interval >= minor_interval;
            let z_range = editor.tri_contour_use_z_range.then_some((editor.tri_contour_z_min_input, editor.tri_contour_z_max_input));
            let valid_z_range = z_range.is_none_or(|(lo, hi)| lo.is_finite() && hi.is_finite() && lo < hi);
            let can_run = editor.tri_contour_tri_id.is_some()
                && valid_intervals
                && valid_z_range
                && project.has_active_project
                && (editor.tri_contour_target_layer.is_some() || (!new_layer_name.is_empty() && !new_layer_name_conflicts));

            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                        contour_labels: editor.tri_contour_labels,
                        output_layer,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
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
            tool_help_panel(ui, tr!("tri-reconstruct-help"));
            ui.add_space(4.0);

            // A cloud selected when the dialog opened is already here. It can
            // still be unloaded or deleted from under the dialog, which takes
            // the run button with it.
            let selected = editor.point_cloud_tin_cloud_id.and_then(|id| {
                project
                    .point_clouds
                    .iter()
                    .find(|cloud| cloud.is_loaded && cloud.id == id)
                    .map(|cloud| (cloud.id, cloud.name.as_str(), cloud.point_count, cloud.is_classified))
            });
            let shown = selected.map(|(id, ..)| id);
            let mut cloud_id = shown;
            MenuFieldCombo::new(
                "point_cloud_tin_cloud",
                tr!("common-point-cloud"),
                &mut cloud_id,
                selected.map(|(_, name, ..)| name.to_owned()).unwrap_or_else(|| tr!("tri-select")),
                project
                    .point_clouds
                    .iter()
                    .filter(|cloud| cloud.is_loaded)
                    .map(|cloud| (Some(cloud.id), cloud.name.as_str().into())),
            )
            .help_text(tr!("tri-point-cloud-source-help"))
            .width(220.0)
            .show(ui);
            if let Some(id) = cloud_id.filter(|_| cloud_id != shown) {
                commands.push(UiCommand::SetToolSubject(ToolSubject::TinPointCloud(id)));
            }

            // Bare earth is the surveyor's first move on a delivery, so it is
            // offered right under the cloud it applies to and left on. A cloud
            // that never went through a ground filter has nothing to offer, and
            // says so rather than showing a switch that would change nothing.
            let classified = selected.is_some_and(|(.., classified)| classified);
            let mut ground_only = classified && editor.point_cloud_tin_ground_only;
            ui.add_enabled_ui(classified, |ui| {
                let field = MenuFieldBool::new(tr!("tri-ground-points-only"), &mut ground_only).help_text(if classified {
                    tr!("tri-reconstruct-ground-only-help")
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
            .help_text(tr!("tri-sampling-method-help"))
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
            .width(220.0)
            .show(ui);

            if editor.point_cloud_tin_budget_is_percent {
                MenuFieldF64::new(tr!("tri-percentage"), &mut editor.point_cloud_tin_percent, 0.001..=100.0)
                    .help_text(tr!("tri-share-source-points-keep-fractions"))
                    .speed(0.05)
                    .suffix(tr!("common-percent-suffix"))
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
                    .suffix(tr!("common-times-sign"))
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
                .help_text(tr!("tri-bridge-gaps-help"))
                .speed(0.5)
                .suffix(tr!("common-m"))
                .max_decimals(2)
                .width(220.0)
                .show(ui);

            ui.add_space(4.0);
            ui.separator();

            MenuFieldText::new(tr!("tri-create-output-name"), &mut editor.point_cloud_tin_name_input)
                .width(220.0)
                .hint_text(tr!("tri-type-open-surface"))
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let can_run = selected.is_some() && !editor.point_cloud_tin_name_input.trim().is_empty();
                let confirm = menu::dialog_confirmed(ui.ctx());
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    editor.point_cloud_tin_open = false;
                }
            });
        });
    if !open {
        editor.point_cloud_tin_open = false;
    }
}
