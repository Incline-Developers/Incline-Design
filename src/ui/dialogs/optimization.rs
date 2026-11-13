//! The pit optimization windows (Production's Block Model menu): the scenarios list, and the editor a
//! scenario opens in. Only one of the two is up at a time - adding or editing
//! hides the list, closing the editor brings it back.
//!
//! The editor is one page of sections, top to bottom: Inputs, Mining costs,
//! Processing costs, Revenues, Constraints, Outputs. The constants are not a
//! section: one list serves every scenario and schedule, in a window that
//! takes the place of the list or editor that opened it
//! ([`draw_constants_window`]).

use std::time::Duration;

use crate::{
    i18n::tr,
    model::{
        block_model::OpenBlockModel,
        npv_schedule::format_money,
        optimization::{
            AirMode, BlockModelFields, Constant, ConstantValue, ConstantsRow, ElementCost, FactorInput, FieldValue, GradeUnit, HaulageCost, HaulageMode, OptimizationScenario,
            ProcessingMethod, RevenueRow, RocktypeCost, RosetteInterpolation, RosetteIssue, RosetteRow, SalesUnit, ShellDirection, ShellFieldMode, ShellFieldValue, ShellMode,
            SlopeMode, ValueType, group_names, move_constants_row, parse_factor_list, shell_count_for_step, shell_prefix_taken, shell_step_for_count, unique_name,
        },
    },
    ui::{
        state::{
            ConstantsDraft, EditorState, ListSelection, NpvScheduleDrop, NpvScheduleStatus, NpvScheduleSummary, OptimizationState, ScenarioDraft, ScenarioGridSelections,
            ScenarioStatus, ScenarioTab, UiCommand, UiProjectView,
        },
        themed_icon, unthemed_icon,
        widgets::{
            collapsible_section::CollapsibleSection,
            editable_grid::{DataGrid, GridAction, GridButtons, GridColumn, GridRow, apply_flat, move_to_slot},
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, menu_note},
            rosette_diagram::{HEIGHT as ROSETTE_HEIGHT, draw_rosette},
            toolbar::ToolbarButton,
            value_field::{self, FieldEditor, ValueField, edit_count, mark_invalid},
        },
    },
};

pub(super) const EDITOR_WIDTH: f32 = 800.0;
/// Space between the settings and the section menu beside them.
pub(super) const SIDE_GAP: f32 = 8.0;
pub(super) const MENU_ITEM_HEIGHT: f32 = 32.0;
const MENU_ICON: f32 = 22.0;
/// Wide enough for the long names sensitivity studies get.
const LIST_WIDTH: f32 = 806.0;
const ICON_SIDE: f32 = crate::ui::widgets::toolbar::TOOL_CELL_SIZE;

pub(crate) fn draw_optimization_dialogs(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    block_models: &[OpenBlockModel],
    project: &UiProjectView,
    canvas_rect: egui::Rect,
    commands: &mut Vec<UiCommand>,
) {
    // The progress rings move while a run goes, whichever window is up.
    if editor.optimization.any_running() {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
    // A graph takes the whole 3D canvas, and the other dialogs step aside
    // until it closes - so Escape reaches it, not a dialog under it. The
    // canvas has the chrome gap at its top and sides from the regions beside
    // it; the bottom toolbar meets it flush, so the same gap is kept there.
    let canvas_rect = egui::Rect::from_min_max(canvas_rect.min, canvas_rect.max - egui::vec2(0.0, crate::ui::chrome::margin()));
    if editor.optimization.schedule_results_view.is_some() {
        super::npv_schedule_results::draw_schedule_results_window(ui, &mut editor.optimization, canvas_rect, commands);
        value_field::close_constant_picker(ui.ctx());
        return;
    }
    if editor.optimization.results_view.is_some() {
        super::optimization_results::draw_results_window(ui, &mut editor.optimization, canvas_rect, commands);
        value_field::close_constant_picker(ui.ctx());
        return;
    }
    // The global constants take the place of the list or editor that opened
    // them, which comes back when they close: one window up at a time.
    if editor.optimization.constants_draft.is_some() {
        draw_constants_window(ui, &mut editor.optimization, commands);
        value_field::close_constant_picker(ui.ctx());
        return;
    }
    // The load prompt is modal and drawn over the window under it, but takes
    // Enter and Escape first: only the first dialog asking for a key gets it.
    let prompt_keys = editor
        .optimization
        .load_prompt
        .is_some()
        .then(|| (menu::dialog_confirmed(ui.ctx()), menu::dialog_cancelled(ui.ctx())));
    // While a starting point is picked in the viewport the editor steps aside;
    // the prompt is the viewport banner (`ui::viewport_message`).
    if editor.optimization.start_pick.is_some() {
        editor.optimization.pick_was_active = true;
    } else if editor.optimization.draft.is_some() {
        // The Escape that cancelled the pick is still in this frame's input; it
        // must not also close the editor it returns to.
        if std::mem::take(&mut editor.optimization.pick_was_active) {
            menu::dialog_cancelled(ui.ctx());
        }
        draw_scenario_editor(ui, &mut editor.optimization, block_models, project, commands);
    } else if editor.optimization.schedule_draft.is_some() {
        super::npv_schedule::draw_schedule_editor(ui, &mut editor.optimization, block_models, commands);
    } else if editor.optimization.list_open {
        draw_scenarios_list(ui, &mut editor.optimization, block_models, project, commands);
    }
    draw_load_prompt(ui, &mut editor.optimization, prompt_keys.unwrap_or_default(), commands);
    // The picker belongs to the editor: it goes with it, and does not come
    // back the next time the editor opens.
    let editing = editor.optimization.draft.is_some() || editor.optimization.schedule_draft.is_some();
    if editing && editor.optimization.start_pick.is_none() {
        value_field::draw_constant_picker(ui.ctx());
    } else {
        value_field::close_constant_picker(ui.ctx());
    }
}

/// "These inputs are unloaded: load them?" before a run or a pick.
/// Modal; `enter` and `escape` are read before the windows under it.
fn draw_load_prompt(ui: &mut egui::Ui, state: &mut OptimizationState, (enter, escape): (bool, bool), commands: &mut Vec<UiCommand>) {
    let Some(prompt) = state.load_prompt.as_ref() else {
        return;
    };
    let names = prompt.items.iter().map(|(_, name)| format!("\u{201C}{name}\u{201D}")).collect::<Vec<_>>().join(", ");
    let mut open = true;
    let mut answer = None;
    // Modal: the list and editor under it wait for an answer.
    menu::modal_sheet(ui.ctx(), "optimization_load_prompt_sheet");
    let dialog = DragableMenu::new("optimization_load_prompt", tr!("opt-load-title"))
        .open(&mut open)
        .min_width(360.0)
        .max_width(360.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("opt-load-question", names = names.clone(), count = prompt.items.len()));
            menu_note(ui, tr!("opt-load-note"));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-load")).primary()).clicked() || enter {
                    answer = Some(true);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || escape {
                    answer = Some(false);
                }
            });
        });
    if let Some(dialog) = dialog {
        ui.ctx().move_to_top(dialog.response.layer_id);
    }
    if let Some(load) = answer.or((!open).then_some(false)) {
        commands.push(UiCommand::ConfirmOptimizationLoad(load));
    }
}

// ── Scenarios list ──

fn draw_scenarios_list(ui: &mut egui::Ui, state: &mut OptimizationState, block_models: &[OpenBlockModel], project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // What each scenario names that the project no longer has, from names only.
    let models: std::collections::HashMap<String, BlockModelFields> = block_models.iter().map(|model| (model.name.clone(), BlockModelFields::of_open(model))).collect();
    let surfaces: Vec<String> = project
        .triangulations
        .iter()
        .map(|entry| entry.name.clone())
        .chain(project.point_clouds.iter().map(|entry| entry.name.clone()))
        .collect();
    let mut open = true;
    let mut close = false;
    let mut show_results = None;
    // What the buttons ask for, sent after any name still being typed is committed.
    let mut actions: Vec<UiCommand> = Vec::new();
    // The picked row, while it is still in the list.
    let selection = state.list_selection.filter(|selection| match *selection {
        ListSelection::Scenario(id) => state.scenarios.iter().any(|scenario| scenario.id == id),
        ListSelection::Schedule { parent, id } => state
            .scenarios
            .iter()
            .find(|scenario| scenario.id == parent)
            .is_some_and(|scenario| scenario.schedules.iter().any(|schedule| schedule.id == id)),
    });
    let mut picked = selection;
    DragableMenu::new("optimization_scenarios_dialog", tr!("opt-scenarios-title"))
        .open(&mut open)
        .min_width(LIST_WIDTH)
        .max_width(LIST_WIDTH)
        .show(ui.ctx(), |ui| {
            // The name fields outlined in the light theme, as in the editor.
            outline_fields(ui);
            draw_list_toolbar(ui, state, selection, &mut actions, &mut show_results);
            ui.add_space(4.0);
            if state.scenarios.is_empty() {
                menu_note(ui, tr!("opt-scenarios-empty"));
            }
            // As tall as the rows up to the editor's body height, then they scroll.
            let max_height = body_height(ui.ctx());
            egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                let mut toggled = None;
                let mut dropped = None;
                for scenario in &state.scenarios {
                    let status = state.status(scenario);
                    let expanded = state.expanded.contains(&scenario.id);
                    let background = ui.painter().add(egui::Shape::Noop);
                    let row = ui.horizontal(|ui| {
                        // The disclosure arrow, or the room it takes, so names line up.
                        if scenario.schedules.is_empty() {
                            ui.add_space(DISCLOSURE_WIDTH);
                        } else if disclosure(ui, expanded).clicked() {
                            toggled = Some(scenario.id);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            // Right to left: the state, then anything missing.
                            status_cell(ui, status);
                            let missing = scenario.missing_references(&models, &surfaces);
                            if !missing.is_empty() {
                                let image = egui::Image::new(unthemed_icon!("status_broken.svg")).fit_to_exact_size(egui::Vec2::splat(ICON_SIDE - 8.0));
                                let lines: Vec<String> = missing.iter().map(|issue| format!("• {issue}")).collect();
                                ui.add_sized([ICON_SIDE, ICON_SIDE], image)
                                    .hint(format!("{}\n{}", tr!("opt-missing-title"), lines.join("\n")));
                            }
                            draw_name_field(
                                ui,
                                scenario.id,
                                &scenario.name,
                                selection == Some(ListSelection::Scenario(scenario.id)),
                                commands,
                                |id, name| UiCommand::RenameOptimizationScenario { id, name },
                            );
                        });
                    });
                    let this = ListSelection::Scenario(scenario.id);
                    select_row(ui, background, row.response.rect, selection == Some(this), this, &mut picked);
                    // A schedule dragged over another scenario's row may be dropped there.
                    if let Some(drag) = egui::DragAndDrop::payload::<NpvScheduleDrag>(ui.ctx())
                        && drag.from != scenario.id
                        && row.response.contains_pointer()
                    {
                        ui.painter().rect_stroke(
                            row.response.rect.expand(1.0),
                            crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS,
                            ui.visuals().selection.stroke,
                            egui::StrokeKind::Inside,
                        );
                    }
                    if egui::DragAndDrop::has_payload_of_type::<NpvScheduleDrag>(ui.ctx())
                        && let Some(drag) = row.response.dnd_release_payload::<NpvScheduleDrag>()
                        && drag.from != scenario.id
                    {
                        dropped = Some(NpvScheduleDrop {
                            id: drag.id,
                            from: drag.from,
                            to: scenario.id,
                            pos: ui.ctx().pointer_latest_pos().unwrap_or(row.response.rect.center()),
                        });
                    }
                    if expanded && !scenario.schedules.is_empty() {
                        ui.add_space(2.0);
                        // The children on a faint accent wash, so they read as the
                        // scenario's; the same room above and below them.
                        egui::Frame::new()
                            .fill(schedule_rows_fill(ui.visuals()))
                            .corner_radius(egui::CornerRadius::same(crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS))
                            .inner_margin(egui::Margin::symmetric(0, 3))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                draw_schedule_rows(ui, state, scenario, selection, &mut picked, commands);
                            });
                        ui.add_space(2.0);
                    }
                }
                if let Some(id) = toggled
                    && !state.expanded.remove(&id)
                {
                    state.expanded.insert(id);
                }
                if let Some(drop) = dropped {
                    state.drop_menu = Some(drop);
                }
            });
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    close = true;
                }
            });
        });
    // A button clicked while a name is still being typed commits the name
    // first: its field may not have lost focus yet this frame.
    if !actions.is_empty() || close || !open {
        commit_typed_names(ui.ctx(), state, commands);
    }
    commands.extend(actions);
    state.list_selection = picked;
    if let Some(id) = show_results {
        state.results_view = Some(crate::ui::state::ResultsView::new(crate::ui::state::ResultsSource::Run(id)));
    }
    if close || !open {
        state.list_open = false;
        state.drop_menu = None;
    } else {
        draw_drop_menu(ui.ctx(), state, commands);
    }
}

/// The list's toolbar, acting on the picked row. A button that does not
/// apply to the picked row is not shown at all:
/// - Add scenario: always.
/// - Add schedule: a scenario picked.
/// - Edit, Duplicate, Delete, Run: any row picked.
/// - Run all: a scenario picked (greyed while it has no schedules).
/// - Results: any row picked (greyed until it has results).
/// - Play the mining: a schedule picked (greyed until it can be played).
/// - Import and export, on the right: always.
fn draw_list_toolbar(ui: &mut egui::Ui, state: &OptimizationState, selection: Option<ListSelection>, actions: &mut Vec<UiCommand>, show_results: &mut Option<u64>) {
    let scenario = selection.and_then(|selection| state.scenarios.iter().find(|scenario| scenario.id == selection.scenario()));
    let schedule = match selection {
        Some(ListSelection::Schedule { id, .. }) => scenario.and_then(|scenario| scenario.schedules.iter().find(|schedule| schedule.id == id)),
        _ => None,
    };
    let scenario_picked = matches!(selection, Some(ListSelection::Scenario(_)));
    let accent = menu::accent_fill(ui.visuals());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let button = |ui: &mut egui::Ui, image: egui::Image<'static>, tooltip: String, salt: &str, enabled: bool| -> bool {
            ui.add_enabled_ui(enabled, |ui| {
                ui.add(ToolbarButton::new(image, tooltip).id_salt(("opt_list_toolbar", salt)).button_side(ICON_SIDE))
            })
            .inner
            .clicked()
        };
        if button(ui, egui::Image::new(themed_icon!(ui, "grid_add.svg")), tr!("opt-add-scenario"), "add", true) {
            actions.push(UiCommand::AddOptimizationScenario);
        }
        // In the schedules' colour (see `schedule_rows_fill`).
        if scenario_picked && let Some(scenario) = scenario {
            let add_icon = egui::Image::new(egui::include_image!("../../../res/ui/icons_dark/schedule_add.svg")).tint(accent);
            if button(ui, add_icon, tr!("sched-add"), "add_npv_schedule", true) {
                actions.push(UiCommand::AddNpvSchedule(scenario.id));
            }
        }
        if button(ui, egui::Image::new(themed_icon!(ui, "constant_pick.svg")), tr!("opt-constants-open"), "constants", true) {
            actions.push(UiCommand::OpenOptimizationConstants);
        }
        if let Some(selection) = selection {
            ui.separator();
            if button(ui, egui::Image::new(themed_icon!(ui, "edit.svg")), tr!("opt-list-edit"), "edit", true) {
                actions.push(match selection {
                    ListSelection::Scenario(id) => UiCommand::EditOptimizationScenario(id),
                    ListSelection::Schedule { parent, id } => UiCommand::EditNpvSchedule { parent, id },
                });
            }
            if button(ui, egui::Image::new(themed_icon!(ui, "duplicate.svg")), tr!("opt-list-duplicate"), "duplicate", true) {
                actions.push(match selection {
                    ListSelection::Scenario(id) => UiCommand::DuplicateOptimizationScenario(id),
                    ListSelection::Schedule { parent, id } => UiCommand::DuplicateNpvSchedule { parent, id },
                });
            }
            if button(ui, egui::Image::new(unthemed_icon!("delete_scenario.svg")), tr!("opt-list-delete"), "delete", true) {
                actions.push(match selection {
                    ListSelection::Scenario(id) => UiCommand::DeleteOptimizationScenario(id),
                    ListSelection::Schedule { parent, id } => UiCommand::DeleteNpvSchedule { parent, id },
                });
            }
            ui.separator();
            // Run, or stop a run going: the scenario's, or the schedule's (which
            // runs its scenario first when that has no up-to-date run).
            let status = match (scenario, schedule) {
                (Some(scenario), Some(schedule)) => schedule_play_status(state.schedule_status(scenario, schedule)),
                (Some(scenario), None) => state.status(scenario),
                _ => ScenarioStatus::NeverRun,
            };
            let play = play_button(ui, status);
            let play = match (scenario, schedule) {
                (Some(scenario), Some(schedule)) if state.schedule_status(scenario, schedule) == NpvScheduleStatus::Blocked => play.hint(tr!("sched-run-parent-first-hint")),
                _ => play,
            };
            if play.clicked() {
                match (selection, status) {
                    (ListSelection::Scenario(id), ScenarioStatus::Running(_) | ScenarioStatus::Queued) => actions.push(UiCommand::CancelOptimizationScenario(id)),
                    (ListSelection::Scenario(id), _) => actions.push(UiCommand::RunOptimizationScenario(id)),
                    (ListSelection::Schedule { parent, id }, _) => actions.push(UiCommand::RunNpvSchedule { parent, id }),
                }
            }
            // Every schedule of the picked scenario, side by side.
            if scenario_picked && let Some(scenario) = scenario {
                let has_schedules = !scenario.schedules.is_empty();
                let tooltip = if has_schedules { tr!("sched-run-all-button") } else { tr!("sched-run-all-none") };
                let response = ui
                    .add_enabled_ui(has_schedules, |ui| {
                        ui.add(
                            ToolbarButton::new(egui::Image::new(unthemed_icon!("run_all.svg")), tooltip.clone())
                                .id_salt(("opt_list_toolbar", "run_all"))
                                .button_side(ICON_SIDE),
                        )
                    })
                    .inner;
                if !has_schedules {
                    let _ = response.clone().disabled_hint(tooltip);
                }
                if response.clicked() {
                    actions.push(UiCommand::RunAllNpvSchedules(scenario.id));
                }
            }
            let has_results = match (scenario, schedule) {
                (Some(scenario), Some(schedule)) => state.schedule_status(scenario, schedule) == NpvScheduleStatus::UpToDate && state.schedule_results.contains_key(&schedule.id),
                (Some(scenario), None) => state.results.contains_key(&scenario.id),
                _ => false,
            };
            let results = egui::Image::new(unthemed_icon!("results_scenario.svg")).tint(accent);
            if button(ui, results, tr!("opt-list-results"), "results", has_results) {
                match selection {
                    ListSelection::Scenario(id) => *show_results = Some(id),
                    ListSelection::Schedule { parent, id } => actions.push(UiCommand::OpenNpvScheduleResults { parent, id }),
                }
            }
            if let (Some(scenario), Some(schedule), ListSelection::Schedule { parent, id }) = (scenario, schedule, selection) {
                let playable = state.schedule_status(scenario, schedule) == NpvScheduleStatus::UpToDate
                    && state.schedule_results.get(&schedule.id).is_some_and(|result| result.order.is_some());
                if button(
                    ui,
                    egui::Image::new(themed_icon!(ui, "schedule_animate.svg")),
                    tr!("sched-anim-button"),
                    "animate",
                    playable,
                ) {
                    actions.push(UiCommand::PlayNpvScheduleAnimation { parent, id });
                }
            }
        }
        // Import and export on the right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let export = egui::Image::new(unthemed_icon!("export_scenarios.svg")).tint(accent);
            if button(ui, export, tr!("opt-export-json"), "export", !state.scenarios.is_empty()) {
                actions.push(UiCommand::ExportOptimizationScenarios);
            }
            let import = egui::Image::new(unthemed_icon!("load_scenarios.svg")).tint(accent);
            if button(ui, import, tr!("opt-import-json"), "import", true) {
                actions.push(UiCommand::ImportOptimizationScenarios);
            }
        });
    });
}

/// Pick a row by clicking anywhere on it (its name field still takes the
/// click to edit), and tint the picked one behind its contents.
fn select_row(ui: &egui::Ui, background: egui::layers::ShapeIdx, rect: egui::Rect, selected: bool, this: ListSelection, picked: &mut Option<ListSelection>) {
    if ui.rect_contains_pointer(rect) && ui.input(|input| input.pointer.primary_clicked()) {
        *picked = Some(this);
    }
    if selected {
        // A clear accent: the selection fill, an accent outline, and a bar at the left.
        let area = rect.expand2(egui::vec2(2.0, 1.0));
        let accent = menu::accent_fill(ui.visuals());
        let radius = crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS;
        let bar = egui::Rect::from_min_size(area.min, egui::vec2(3.0, area.height()));
        ui.painter().set(
            background,
            egui::Shape::Vec(vec![
                egui::Shape::rect_filled(area, radius, ui.visuals().selection.bg_fill.gamma_multiply(0.7)),
                egui::Shape::rect_stroke(area, radius, egui::Stroke::new(1.0, accent), egui::StrokeKind::Inside),
                egui::Shape::rect_filled(bar, radius, accent),
            ]),
        );
    }
}

/// A row's state: its icon, or while running (or queued) a progress ring.
fn status_cell(ui: &mut egui::Ui, status: ScenarioStatus) {
    match status {
        ScenarioStatus::Running(progress) => progress_ring(ui, progress, tr!("opt-status-running-percent", percent = ((progress * 100.0) as u32).to_string())),
        ScenarioStatus::Queued => progress_ring(ui, 0.0, tr!("opt-status-queued")),
        _ => status_icon(ui, status),
    }
}

/// A ring filling as `progress` goes from 0 to 1, as the play button shows it.
fn progress_ring(ui: &mut egui::Ui, progress: f32, tooltip: String) {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(ICON_SIDE), egui::Sense::hover());
    // As wide as the status icons it stands in for (`status_icon`), clear of the row's edges.
    paint_progress(ui, rect.shrink(4.0), progress);
    response.hint(tooltip);
}

fn paint_progress(ui: &egui::Ui, rect: egui::Rect, progress: f32) {
    let center = rect.center();
    let radius = rect.width() / 2.0 - 1.5;
    ui.painter()
        .circle_stroke(center, radius, egui::Stroke::new(2.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
    let steps = ((progress * 64.0).ceil() as usize).max(2);
    let points: Vec<egui::Pos2> = (0..=steps)
        .map(|step| {
            let angle = -std::f32::consts::FRAC_PI_2 + progress * std::f32::consts::TAU * step as f32 / steps as f32;
            center + radius * egui::vec2(angle.cos(), angle.sin())
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, egui::Stroke::new(2.5, egui::Color32::from_rgb(0x2f, 0xb3, 0x44))));
}

// ── Schedules in the list ──

/// What a dragged schedule row carries: the schedule and the scenario it is under.
#[derive(Clone, Copy, Debug)]
struct NpvScheduleDrag {
    id: u64,
    from: u64,
}

/// Room the disclosure arrow takes before a scenario's name.
const DISCLOSURE_WIDTH: f32 = 18.0;

/// The arrow that shows or hides a scenario's schedules.
fn disclosure(ui: &mut egui::Ui, expanded: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(DISCLOSURE_WIDTH - ui.spacing().item_spacing.x, ICON_SIDE), egui::Sense::click());
    let color = if response.hovered() {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    let center = rect.center();
    let half = 4.0;
    let points = if expanded {
        vec![
            center + egui::vec2(-half, -half / 2.0),
            center + egui::vec2(half, -half / 2.0),
            center + egui::vec2(0.0, half),
        ]
    } else {
        vec![
            center + egui::vec2(-half / 2.0, -half),
            center + egui::vec2(half, 0.0),
            center + egui::vec2(-half / 2.0, half),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
    response.hint(if expanded { tr!("sched-hide-schedules") } else { tr!("sched-show-schedules") })
}

/// The wash behind a scenario's schedules: the accent, faint - a touch
/// stronger in the dark theme, where a faint tint sinks into the panel.
fn schedule_rows_fill(visuals: &egui::Visuals) -> egui::Color32 {
    menu::accent_fill(visuals).gamma_multiply(if visuals.dark_mode { 0.16 } else { 0.10 })
}

/// A scenario's schedules, one row each under it: a drag grip on the guide
/// line, the name, the NPV, then its state on the right. Picked like a
/// scenario row; the list's toolbar acts on it.
fn draw_schedule_rows(
    ui: &mut egui::Ui,
    state: &OptimizationState,
    scenario: &OptimizationScenario,
    selection: Option<ListSelection>,
    picked: &mut Option<ListSelection>,
    commands: &mut Vec<UiCommand>,
) {
    let parent = scenario.id;
    // The highest NPV among the scenario's schedules is shown in the accent.
    let summaries: Vec<Option<NpvScheduleSummary>> = scenario.schedules.iter().map(|schedule| state.schedule_summary(scenario, schedule)).collect();
    let top = summaries.iter().flatten().map(|summary| summary.npv_lead).fold(f64::NEG_INFINITY, f64::max);
    let names: Vec<String> = scenario.schedules.iter().map(|schedule| schedule.name.trim().to_lowercase()).collect();
    for (index, schedule) in scenario.schedules.iter().enumerate() {
        // Room between rows only, so the group has the same margin above and below.
        if index > 0 {
            ui.add_space(2.0);
        }
        let status = state.schedule_status(scenario, schedule);
        let issues = schedule.issues(scenario);
        let background = ui.painter().add(egui::Shape::Noop);
        let row = ui.horizontal(|ui| {
            ui.add_space(DISCLOSURE_WIDTH);
            // The grip: drag it onto another scenario to move or copy the schedule.
            let (grip, response) = ui.allocate_exact_size(egui::vec2(ICON_SIDE - 6.0, ICON_SIDE), egui::Sense::drag());
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().vline(grip.left() + 3.0, grip.y_range().expand(1.0), stroke);
            let dots = if response.hovered() || response.dragged() {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().weak_text_color()
            };
            for row in 0..3 {
                for column in 0..2 {
                    let at = egui::pos2(grip.center().x + 2.0 + column as f32 * 4.0, grip.center().y - 4.0 + row as f32 * 4.0);
                    ui.painter().circle_filled(at, 1.2, dots);
                }
            }
            response.dnd_set_drag_payload(NpvScheduleDrag { id: schedule.id, from: parent });
            if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            response.hint(tr!("sched-drag-hint"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                schedule_status_icon(ui, status);
                if !issues.is_empty() {
                    let image = egui::Image::new(unthemed_icon!("status_broken.svg")).fit_to_exact_size(egui::Vec2::splat(ICON_SIDE - 8.0));
                    let lines: Vec<String> = issues.iter().map(|issue| format!("• {issue}")).collect();
                    ui.add_sized([ICON_SIDE, ICON_SIDE], image)
                        .hint(format!("{}\n{}", tr!("sched-issues-title"), lines.join("\n")));
                }
                npv_cell(ui, summaries[index], summaries[index].is_some_and(|summary| summary.npv_lead >= top));
                let duplicate_name = names.iter().enumerate().any(|(other, name)| other != index && *name == names[index]);
                let picked_row = selection == Some(ListSelection::Schedule { parent, id: schedule.id });
                let name = draw_name_field(ui, schedule.id, &schedule.name, picked_row, commands, move |id, name| UiCommand::RenameNpvSchedule {
                    parent,
                    id,
                    name,
                });
                if duplicate_name {
                    mark_invalid(ui, &name);
                    name.hint(tr!("sched-name-taken"));
                }
            });
        });
        let this = ListSelection::Schedule { parent, id: schedule.id };
        select_row(ui, background, row.response.rect, selection == Some(this), this, picked);
    }
}

/// The schedule's NPV, or a dash before a run: right-aligned in a fixed
/// width, so the column lines up down the list.
fn npv_cell(ui: &mut egui::Ui, summary: Option<NpvScheduleSummary>, highest: bool) {
    let text = summary.map_or_else(|| "\u{2014}".to_owned(), |summary| format_money(summary.npv_lead));
    let mut rich = egui::RichText::new(text).monospace();
    if highest {
        rich = rich.color(menu::accent_fill(ui.visuals())).strong();
    }
    let response = ui.add_sized([NPV_WIDTH, ICON_SIDE], egui::Label::new(rich));
    match summary {
        Some(summary) => {
            let mut text = tr!(
                "sched-npv-hover",
                periods = summary.periods.to_string(),
                lead = format_money(summary.npv_lead),
                best = format_money(summary.npv_best),
                worst = format_money(summary.npv_worst)
            );
            if let Some(pushback) = summary.peak_before_last {
                text.push('\n');
                text.push_str(&tr!("sched-npv-hover-peak", pushback = pushback.to_string()));
            }
            response.hint(text)
        }
        None => response.hint(tr!("sched-npv-none")),
    };
}

const NPV_WIDTH: f32 = 82.0;

/// The play button's look for a schedule (the scenario's, minus Blocked).
fn schedule_play_status(status: NpvScheduleStatus) -> ScenarioStatus {
    match status {
        NpvScheduleStatus::Blocked | NpvScheduleStatus::NeverRun => ScenarioStatus::NeverRun,
        NpvScheduleStatus::UpToDate => ScenarioStatus::UpToDate,
        NpvScheduleStatus::Stale => ScenarioStatus::Stale,
        NpvScheduleStatus::Queued => ScenarioStatus::Queued,
        NpvScheduleStatus::Running(progress) => ScenarioStatus::Running(progress),
    }
}

/// The state icon of a schedule: the scenario's icons, and Blocked as the
/// not-run icon dimmed, with why on hover.
fn schedule_status_icon(ui: &mut egui::Ui, status: NpvScheduleStatus) {
    if status != NpvScheduleStatus::Blocked {
        status_icon(ui, schedule_play_status(status));
        return;
    }
    let image = egui::Image::new(unthemed_icon!("status_not_run.svg"))
        .fit_to_exact_size(egui::Vec2::splat(ICON_SIDE - 8.0))
        .tint(egui::Color32::from_white_alpha(110));
    ui.add_sized([ICON_SIDE, ICON_SIDE], image).hint(tr!("sched-blocked"));
}

/// Move here / Duplicate here / Cancel, at the point a schedule was dropped on
/// another scenario.
fn draw_drop_menu(ctx: &egui::Context, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let Some(drop) = state.drop_menu else {
        return;
    };
    let target = state.scenarios.iter().find(|scenario| scenario.id == drop.to).map(|scenario| scenario.name.clone());
    let Some(target) = target else {
        state.drop_menu = None;
        return;
    };
    let mut choice = None;
    let area = egui::Area::new(egui::Id::unique("schedule_drop_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(drop.pos)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(180.0);
                ui.label(egui::RichText::new(tr!("sched-drop-title", scenario = target.clone())).weak());
                if ui.add(MenuButton::new(tr!("sched-move-here")).min_width(170.0)).clicked() {
                    choice = Some(true);
                }
                if ui.add(MenuButton::new(tr!("sched-duplicate-here")).min_width(170.0)).clicked() {
                    choice = Some(false);
                }
                if ui.add(MenuButton::new(tr!("common-cancel")).min_width(170.0)).clicked() {
                    state.drop_menu = None;
                }
            });
        });
    // A click anywhere else, or Escape, cancels.
    let clicked_away = ctx.input(|input| input.pointer.any_pressed()) && !area.response.contains_pointer();
    if clicked_away || menu::dialog_cancelled(ctx) {
        state.drop_menu = None;
    }
    if let Some(moving) = choice {
        state.drop_menu = None;
        commands.push(if moving {
            UiCommand::MoveNpvSchedule {
                id: drop.id,
                from: drop.from,
                to: drop.to,
            }
        } else {
            UiCommand::CopyNpvScheduleTo {
                id: drop.id,
                from: drop.from,
                to: drop.to,
            }
        });
        // The scenario that took it opens, so the result shows.
        state.expanded.insert(drop.to);
    }
}

/// Height the editor's sections scroll within, and the list's rows: fixed, so
/// neither window grows past the screen.
pub(super) fn body_height(ctx: &egui::Context) -> f32 {
    (ctx.content_rect().height() - 260.0).clamp(240.0, 560.0)
}

fn name_field_ids(id: u64) -> (egui::Id, egui::Id) {
    let field_id = egui::Id::unique(("opt_scenario_name", id));
    (field_id, field_id.with("typed"))
}

/// Send the rename of every name still being typed, and take the focus from
/// its field, so the next command sees the new name.
fn commit_typed_names(ctx: &egui::Context, state: &OptimizationState, commands: &mut Vec<UiCommand>) {
    let take = |id: u64, name: &str| {
        let (field_id, buffer_id) = name_field_ids(id);
        let typed = ctx.data_mut(|data| data.remove_temp::<String>(buffer_id))?;
        ctx.memory_mut(|memory| memory.surrender_focus(field_id));
        let typed = typed.trim().to_owned();
        (!typed.is_empty() && typed != name).then_some(typed)
    };
    for scenario in &state.scenarios {
        if let Some(name) = take(scenario.id, &scenario.name) {
            commands.push(UiCommand::RenameOptimizationScenario { id: scenario.id, name });
        }
        for schedule in &scenario.schedules {
            if let Some(name) = take(schedule.id, &schedule.name) {
                commands.push(UiCommand::RenameNpvSchedule {
                    parent: scenario.id,
                    id: schedule.id,
                    name,
                });
            }
        }
    }
}

/// A row's name: a framed label (the frame hints it can be edited), and on a
/// double click a box to rename it in. The rename is sent when the box is
/// left or Enter is pressed, or when any button of the list is clicked
/// ([`commit_typed_names`]); Escape drops it. A single click picks the row
/// (see [`select_row`]).
fn draw_name_field(ui: &mut egui::Ui, id: u64, name: &str, selected: bool, commands: &mut Vec<UiCommand>, rename: impl Fn(u64, String) -> UiCommand) -> egui::Response {
    let (field_id, buffer_id) = name_field_ids(id);
    let editing_id = field_id.with("editing");
    // 1: just double-clicked (focus asked for); 2: being typed in.
    let editing = ui.data(|data| data.get_temp::<u8>(editing_id)).unwrap_or(0);
    if editing == 0 {
        let height = ui.spacing().interact_size.y;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
        let visuals = ui.visuals();
        let stroke = if response.hovered() {
            visuals.widgets.hovered.bg_stroke
        } else {
            visuals.widgets.noninteractive.bg_stroke
        };
        ui.painter()
            .rect_stroke(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, stroke, egui::StrokeKind::Inside);
        // The picked row's name in the strong text colour, over its accent.
        let color = if selected { visuals.strong_text_color() } else { visuals.text_color() };
        let galley = ui.painter().layout(name.to_owned(), egui::TextStyle::Body.resolve(ui.style()), color, rect.width() - 10.0);
        ui.painter_at(rect.shrink(1.0))
            .galley(egui::pos2(rect.left() + 5.0, rect.center().y - galley.size().y / 2.0), galley, color);
        if response.double_clicked() {
            ui.data_mut(|data| data.insert_temp(editing_id, 1u8));
            ui.memory_mut(|memory| memory.request_focus(field_id));
        }
        return response.on_hover_cursor(egui::CursorIcon::Default).hint(tr!("opt-rename-hint"));
    }

    // What was typed is kept until the box is left, whatever the focus state
    // says on the frame it is lost.
    let pending = ui.data_mut(|data| data.get_temp::<String>(buffer_id));
    let mut text = pending.clone().unwrap_or_else(|| name.to_owned());
    let response = ui.add(egui::TextEdit::singleline(&mut text).id(field_id).desired_width(ui.available_width()));
    if response.changed() {
        ui.data_mut(|data| data.insert_temp(buffer_id, text.clone()));
    }
    if editing == 1 {
        response.request_focus();
        ui.data_mut(|data| data.insert_temp(editing_id, 2u8));
        return response;
    }
    let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
    if !response.has_focus() || cancelled {
        let typed = text.trim();
        if pending.is_some() && !cancelled && !typed.is_empty() && typed != name {
            commands.push(rename(id, typed.to_owned()));
        }
        ui.data_mut(|data| {
            data.remove::<String>(buffer_id);
            data.remove::<u8>(editing_id);
        });
        if cancelled {
            ui.memory_mut(|memory| memory.surrender_focus(field_id));
        }
    }
    response
}

fn status_icon(ui: &mut egui::Ui, status: ScenarioStatus) {
    let (source, tooltip) = match status {
        ScenarioStatus::NeverRun => (unthemed_icon!("status_not_run.svg"), tr!("opt-status-never-run")),
        ScenarioStatus::Running(_) => (unthemed_icon!("status_not_run.svg"), tr!("opt-status-running")),
        ScenarioStatus::Queued => (unthemed_icon!("status_not_run.svg"), tr!("opt-status-queued")),
        ScenarioStatus::UpToDate => (unthemed_icon!("status_up_to_date.svg"), tr!("opt-status-up-to-date")),
        ScenarioStatus::Stale => (unthemed_icon!("status_stale.svg"), tr!("opt-status-stale")),
    };
    let image = egui::Image::new(source).fit_to_exact_size(egui::Vec2::splat(ICON_SIDE - 8.0));
    ui.add_sized([ICON_SIDE, ICON_SIDE], image).hint(tooltip);
}

/// The green play button. While the scenario runs, a ring around it fills with
/// the fraction done.
fn play_button(ui: &mut egui::Ui, status: ScenarioStatus) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(ICON_SIDE), egui::Sense::click());
    let running = match status {
        ScenarioStatus::Running(progress) => Some(progress),
        ScenarioStatus::Queued => Some(0.0),
        _ => None,
    };
    if response.hovered() && running.is_none() {
        ui.painter().rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    egui::Image::new(unthemed_icon!("run_scenario.svg")).paint_at(ui, rect.shrink(6.0));
    if let Some(progress) = running {
        paint_progress(ui, rect, progress);
    }
    let tooltip = match running {
        Some(_) if status == ScenarioStatus::Queued => format!("{}\n{}", tr!("opt-status-queued"), tr!("opt-cancel-run")),
        Some(progress) => format!(
            "{}\n{}",
            tr!("opt-status-running-percent", percent = ((progress * 100.0) as u32).to_string()),
            tr!("opt-cancel-run")
        ),
        // At rest it is a play button, which needs no caption.
        None => return response,
    };
    response.hint(tooltip)
}

// ── Scenario editor ──

pub(super) fn model_fields(block_models: &[OpenBlockModel], name: &str) -> BlockModelFields {
    block_models.iter().find(|model| model.name == name).map(BlockModelFields::of_open).unwrap_or_default()
}

fn draw_scenario_editor(ui: &mut egui::Ui, state: &mut OptimizationState, block_models: &[OpenBlockModel], project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // The saved scenarios, for the shell prefix's uniqueness check.
    let others = state.scenarios.clone();
    let Some(draft) = state.draft.as_mut() else {
        return;
    };
    value_field::publish_constants(ui.ctx(), &draft.scenario.constants);
    let ctx = ui.ctx().clone();
    let mut open = true;
    let mut close_requested = false;
    let mut save = false;
    let title = format!("{}: {}{}", tr!("opt-scenario-title"), draft.scenario.name, if draft.dirty() { "*" } else { "" });
    let body_height = body_height(&ctx);
    let side_width = section_menu_width(&ctx);
    DragableMenu::new("optimization_scenario_editor", title)
        .open(&mut open)
        .min_width(EDITOR_WIDTH + SIDE_GAP + side_width)
        .max_width(EDITOR_WIDTH + SIDE_GAP + side_width)
        .show(&ctx, |ui| {
            outline_fields(ui);
            let ready = !draft.scenario.name.trim().is_empty() && (draft.scenario != draft.saved || draft.is_new);
            let ScenarioDraft { scenario, selections, tab, .. } = &mut *draft;
            // The rows inside keep the dialog's own spacing; only the gap
            // between the settings and the menu is taken out here.
            let item_spacing = ui.spacing().item_spacing;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // The settings keep one width however the section menu is
                // sized: the menu grows the window to the right instead.
                let content = ui.allocate_ui_with_layout(egui::vec2(EDITOR_WIDTH - 16.0, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = item_spacing;
                    ui.set_width(EDITOR_WIDTH - 16.0);
                    // As tall as a menu entry, so the title sits level with
                    // the first one.
                    ui.allocate_ui_with_layout(egui::vec2(EDITOR_WIDTH - 16.0, MENU_ITEM_HEIGHT), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(tab.label()).size(17.0).strong());
                    });
                    // One fixed height for every section, so the window does
                    // not change size under the pointer as they are switched.
                    egui::ScrollArea::vertical()
                        .max_height(body_height)
                        .min_scrolled_height(body_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut fields = model_fields(block_models, &scenario.block_model);
                            match *tab {
                                ScenarioTab::Inputs => draw_inputs(ui, scenario, &mut fields, block_models, project),
                                ScenarioTab::MiningCosts => draw_mining(ui, scenario, &fields, selections),
                                ScenarioTab::ProcessingCosts => draw_processing(ui, scenario, &fields, selections),
                                ScenarioTab::Revenues => draw_revenues(ui, scenario, &fields, selections),
                                ScenarioTab::Constraints => draw_constraints(ui, scenario, &fields, selections),
                                ScenarioTab::Outputs => draw_outputs(ui, scenario, &fields, &others, commands),
                            }
                        });
                });
                ui.add_space(SIDE_GAP);
                let choice = draw_section_menu(ui, content.response.rect.height(), side_width, tab, ready);
                save = choice.save;
                close_requested |= choice.close;
                if choice.constants {
                    commands.push(UiCommand::OpenOptimizationConstants);
                }
            });
            // Escape closes the editor - unless a window over it wants the key.
            if !draft.confirm_close && !value_field::picker_open(ui.ctx()) && menu::dialog_cancelled(ui.ctx()) {
                close_requested = true;
            }
        });
    if !open {
        close_requested = true;
    }
    if save {
        let mut scenario = draft.scenario.clone();
        scenario.name = scenario.name.trim().to_owned();
        commands.push(UiCommand::SaveOptimizationScenario {
            scenario: Box::new(scenario),
            then_close: false,
        });
    }
    if close_requested {
        if draft.dirty() {
            draft.confirm_close = true;
        } else {
            state.draft = None;
            state.list_open = true;
            return;
        }
    }
    if draft.confirm_close {
        draw_confirm_close(&ctx, state, commands);
    }
}

/// Give fields an outline at rest in the light theme, where their fill alone
/// is too close to the card to see.
pub(super) fn outline_fields(ui: &mut egui::Ui) {
    if !ui.visuals().dark_mode {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(185));
    }
}

// ── Section menu ──

struct MenuChoice {
    save: bool,
    close: bool,
    /// Open the global constants.
    constants: bool,
}

/// A white line icon, tinted by [`menu_item`] to the entry's text colour in
/// either theme - which is why it is not the themed (black in light) copy.
macro_rules! tinted_icon {
    ($name:literal) => {
        egui::Image::new(egui::include_image!(concat!("../../../res/ui/icons_dark/", $name)))
    };
}

fn section_icon(tab: ScenarioTab) -> egui::Image<'static> {
    match tab {
        ScenarioTab::Inputs => tinted_icon!("section_inputs.svg"),
        ScenarioTab::MiningCosts => tinted_icon!("section_mining_costs.svg"),
        ScenarioTab::ProcessingCosts => tinted_icon!("section_processing_costs.svg"),
        ScenarioTab::Revenues => tinted_icon!("section_revenues.svg"),
        ScenarioTab::Constraints => tinted_icon!("section_constraints.svg"),
        ScenarioTab::Outputs => tinted_icon!("section_outputs.svg"),
    }
}

/// How wide the section menu is: room for the icon and the longest entry in
/// the current language.
fn section_menu_width(ctx: &egui::Context) -> f32 {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let font = egui::FontId::proportional(14.0);
    let labels = ScenarioTab::ALL
        .into_iter()
        .map(ScenarioTab::label)
        .chain([tr!("opt-constants-open"), tr!("opt-save-scenario"), tr!("common-close")]);
    let longest = labels
        .map(|label| painter.layout_no_wrap(label, font.clone(), egui::Color32::WHITE).size().x)
        .fold(0.0, f32::max);
    MENU_ITEM_HEIGHT + 4.0 + longest + 16.0
}

/// The menu beside the settings: one entry per section, and Save and Close
/// pushed to the bottom. `height` is the settings' height.
fn draw_section_menu(ui: &mut egui::Ui, height: f32, width: f32, current: &mut ScenarioTab, can_save: bool) -> MenuChoice {
    let mut choice = MenuChoice {
        save: false,
        close: false,
        constants: false,
    };
    ui.allocate_ui_with_layout(egui::vec2(width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 2.0;
        let top = ui.cursor().top();
        for tab in ScenarioTab::ALL {
            if menu_item(ui, section_icon(tab), &tab.label(), width, *current == tab, true) {
                *current = tab;
            }
        }
        choice.constants = constants_menu_item(ui, width);
        let bottom = 2.0 * MENU_ITEM_HEIGHT + ui.spacing().item_spacing.y * 2.0;
        ui.add_space((top + height - ui.cursor().top() - bottom).max(4.0));
        choice.save = menu_item(ui, tinted_icon!("save_project.svg"), &tr!("opt-save-scenario"), width, false, can_save);
        choice.close = menu_item(ui, tinted_icon!("close_project.svg"), &tr!("common-close"), width, false, true);
    });
    choice
}

/// One entry of the section menu: its icon and its name. Returns whether it
/// was clicked.
pub(super) fn menu_item(ui: &mut egui::Ui, icon: egui::Image<'static>, label: &str, width: f32, selected: bool, enabled: bool) -> bool {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, MENU_ITEM_HEIGHT), sense);
    let visuals = ui.visuals();
    let fill = if selected {
        Some(menu::accent_fill(visuals))
    } else if response.hovered() && enabled {
        Some(visuals.widgets.hovered.weak_bg_fill)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, fill);
    }
    // The current section is filled with the confirm button's accent, and
    // takes its white ink in either theme.
    let tint = if selected { egui::Color32::WHITE } else { ui.visuals().text_color() };
    let tint = if enabled { tint } else { ui.visuals().weak_text_color() };
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + (MENU_ITEM_HEIGHT + 4.0) / 2.0, rect.center().y), egui::Vec2::splat(MENU_ICON));
    icon.tint(tint).paint_at(ui, icon_rect);
    ui.painter().text(
        egui::pos2(rect.left() + MENU_ITEM_HEIGHT + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        tint,
    );
    enabled && response.clicked()
}

enum CloseChoice {
    Save,
    Discard,
    Keep,
}

/// Ask what to do with unsaved changes the user is closing the editor on.
fn draw_confirm_close(ctx: &egui::Context, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let Some(draft) = state.draft.as_mut() else {
        return;
    };
    let mut open = true;
    let mut choice = None;
    let dialog = DragableMenu::new("optimization_unsaved_dialog", tr!("opt-unsaved-title"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(380.0)
        .show(ctx, |ui| {
            menu_note(ui, tr!("opt-unsaved-message", name = draft.scenario.name.clone()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-save-scenario")).primary()).clicked() {
                    choice = Some(CloseChoice::Save);
                }
                if ui.add(MenuButton::new(tr!("opt-discard")).danger()).clicked() {
                    choice = Some(CloseChoice::Discard);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    choice = Some(CloseChoice::Keep);
                }
            });
        });
    if let Some(dialog) = dialog {
        ctx.move_to_top(dialog.response.layer_id);
    }
    if !open {
        choice = Some(CloseChoice::Keep);
    }
    match choice {
        Some(CloseChoice::Save) => {
            let mut scenario = draft.scenario.clone();
            scenario.name = scenario.name.trim().to_owned();
            draft.confirm_close = false;
            commands.push(UiCommand::SaveOptimizationScenario {
                scenario: Box::new(scenario),
                then_close: true,
            });
        }
        Some(CloseChoice::Discard) => {
            state.draft = None;
            state.list_open = true;
        }
        Some(CloseChoice::Keep) => draft.confirm_close = false,
        None => {}
    }
}

// ── Form rows ──
//
// Every labelled setting is a house field row (`menu::MenuField*`): the label
// on the left, the control in the column the card's fields share. A setting
// that can come from more than one place is a "from" drop-down followed by the
// one row its choice needs, rather than a run of radio buttons.

/// A field row whose control `control` draws, handed the column's width.
pub(super) fn field_row<R>(ui: &mut egui::Ui, label: String, help: Option<String>, control: impl FnOnce(&mut egui::Ui, f32) -> R) -> R {
    let mut field = MenuField::new(label);
    if let Some(help) = help {
        field = field.help_text(help);
    }
    field.show(ui, |ui, row_height, width| {
        ui.allocate_ui_with_layout(egui::vec2(width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_width(width);
            control(ui, width)
        })
        .inner
    })
}

pub(super) fn form_toggle(ui: &mut egui::Ui, label: String, help: Option<String>, value: &mut bool) -> egui::Response {
    let mut field = MenuFieldBool::new(label, value);
    if let Some(help) = help {
        field = field.help_text(help);
    }
    field.show(ui)
}

/// A drop-down over `names`. Whatever the scenario already holds stays listed,
/// so a field that is missing for the moment does not blank the choice.
pub(super) fn form_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: String, value: &mut String, names: &[String]) -> egui::Response {
    field_row(ui, label, None, |ui, width| inline_combo(ui, id, value, names, tr!("opt-none-selected"), width))
}

/// A drop-down over a fixed set of choices.
pub(super) fn form_choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: String,
    help: Option<String>,
    value: &mut T,
    all: &[T],
    name: impl Fn(T) -> String,
) -> egui::Response {
    let selected = name(*value);
    let mut combo = MenuFieldCombo::new(id, label, value, selected, all.iter().map(|option| (*option, name(*option).into())));
    if let Some(help) = help {
        combo = combo.help_text(help);
    }
    combo.show(ui)
}

/// An input/constant field in a labelled row, its unit (if any) after the number.
pub(super) fn form_value<T: FieldEditor>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: String,
    help: Option<String>,
    value: &mut FieldValue<T>,
    range: (f64, f64),
    suffix: String,
) -> egui::Response {
    field_row(ui, label, help, |ui, _| ValueField::new(id, value).range(range.0, range.1).suffix(suffix).show(ui))
}

// ── Units ──
//
// Shown after a typed number, as the top bar shows "m" after the elevation.
// Every cost is multiplied by a block's tonnes (`optimization_run::values`),
// so costs are per tonne; prices are per the element's sales unit.

pub(super) fn unit_per_tonne() -> String {
    tr!("opt-unit-per-tonne")
}

fn unit_density() -> String {
    format!(" {}", tr!("opt-unit-density"))
}

fn unit_degrees() -> String {
    tr!("common-degree-suffix")
}

pub(super) fn unit_percent() -> String {
    tr!("common-percent-suffix")
}

/// Per one sales unit ("/oz"); nothing for a plain unit, which has no name.
pub(super) fn unit_per_sales(unit: SalesUnit) -> String {
    if unit == SalesUnit::Unit { String::new() } else { format!("/{}", unit.label()) }
}

fn combo_options(names: &[String], current: &str, empty_label: String) -> Vec<(String, egui::WidgetText)> {
    let mut options: Vec<(String, egui::WidgetText)> = vec![(String::new(), empty_label.into())];
    options.extend(names.iter().map(|name| (name.clone(), name.clone().into())));
    if !current.is_empty() && !names.iter().any(|name| name == current) {
        options.push((current.to_owned(), current.to_owned().into()));
    }
    options
}

/// A drop-down of the given width, unlabelled; changed when the choice is.
pub(super) fn inline_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, names: &[String], empty_label: String, width: f32) -> egui::Response {
    let selected = if value.is_empty() { empty_label.clone() } else { value.clone() };
    let options = combo_options(names, value, empty_label);
    let mut changed = false;
    let mut response = egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(width)
        .truncate()
        .show_ui(ui, |ui| {
            for (option, text) in options {
                changed |= ui.selectable_value(value, option, text).changed();
            }
        })
        .response;
    if changed {
        response.mark_changed();
    }
    // A name the choices no longer hold (a renamed or deleted field, layer or
    // model) stays shown, outlined red, rather than being cleared.
    if !value.is_empty() && !names.iter().any(|name| name == value) {
        mark_invalid(ui, &response);
        response = response.hint(tr!("opt-missing-choice", name = value.clone()));
    }
    response
}

/// A drop-down that fills the grid cell it is drawn in. A cell left empty is
/// outlined in red when `required`.
pub(super) fn cell_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, names: &[String], empty_label: String, required: bool) {
    let width = ui.available_width().max(40.0);
    let response = inline_combo(ui, id, value, names, empty_label, width);
    if required && value.is_empty() {
        mark_invalid(ui, &response);
    }
}

/// A drop-down over a fixed list of choices that fills the grid cell it is drawn in.
pub(super) fn cell_choice<T: Copy + PartialEq>(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut T, all: &[T], label: impl Fn(T) -> String) {
    let width = ui.available_width().max(40.0);
    egui::ComboBox::from_id_salt(id).selected_text(label(*value)).width(width).truncate().show_ui(ui, |ui| {
        for option in all {
            ui.selectable_value(value, *option, label(*option));
        }
    });
}

/// A text box that fills its grid cell, outlined in red when `required` and empty.
pub(super) fn cell_text(ui: &mut egui::Ui, value: &mut String, required: bool) {
    let response = ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
    if required && value.trim().is_empty() {
        mark_invalid(ui, &response);
    }
}

pub(super) fn plain_rows(count: usize) -> Vec<GridRow> {
    vec![GridRow::Item; count]
}

/// One titled card of related settings. Every group in a section shows at
/// once, so the card does not fold.
pub(super) fn option_group(ui: &mut egui::Ui, title: String, rows: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    CollapsibleSection::new(egui::Id::unique(("optimization_group", title.as_str())), title)
        .collapsible(false)
        .show(ui, rows);
}

// ── Inputs ──

fn draw_inputs(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &mut BlockModelFields, block_models: &[OpenBlockModel], project: &UiProjectView) {
    // Unloaded models too: one keeps its field list, and a run asks to load it.
    let models: Vec<String> = block_models.iter().map(|model| model.name.clone()).collect();
    option_group(ui, tr!("opt-group-block-model"), |ui| {
        if form_combo(ui, "opt_block_model", tr!("common-block-model"), &mut scenario.block_model, &models).changed() {
            *fields = model_fields(block_models, &scenario.block_model);
            scenario.apply_block_model(fields);
        }
        if fields.all.is_empty() && !scenario.block_model.is_empty() {
            menu_note(ui, tr!("opt-block-model-not-loaded"));
        }
        if let Some(issue) = fields.grid_issue {
            ui.add(egui::Label::new(egui::RichText::new(issue.message()).color(ui.visuals().warn_fg_color)).wrap());
        }
        // An unloaded model's grid is not read yet, so whether it can be
        // optimised is only known once it is loaded.
        if block_models.iter().any(|model| model.name == scenario.block_model && !model.state.loaded) {
            ui.add(egui::Label::new(egui::RichText::new(tr!("opt-block-model-unloaded")).color(ui.visuals().weak_text_color())).wrap())
                .hint(tr!("opt-block-model-unloaded-hint"));
        }
        let response = form_value(
            ui,
            "opt_default_density",
            tr!("opt-default-density"),
            Some(tr!("opt-default-density-help")),
            &mut scenario.default_density,
            (0.001, f64::MAX),
            unit_density(),
        );
        if scenario.default_density.resolve(&scenario.constants).is_none_or(|density| density <= 0.0) {
            mark_invalid(ui, &response);
        }
        form_combo(ui, "opt_density", tr!("opt-density-field"), &mut scenario.density_field, &fields.numeric);
        let mut quality = scenario.quality_field.clone();
        if form_combo(ui, "opt_quality", tr!("opt-quality-field"), &mut quality, &fields.numeric).changed() {
            scenario.set_quality_field(quality);
        }
        if form_combo(ui, "opt_rocktype", tr!("opt-rocktype-field"), &mut scenario.rocktype_field, &fields.text).changed() {
            let values = fields.rocktype_values(&scenario.rocktype_field);
            scenario.settle_rocktype_values(&values);
        }
    });

    let values = fields.rocktype_values(&scenario.rocktype_field);
    option_group(ui, tr!("opt-air-exclusion"), |ui| {
        form_toggle(ui, tr!("opt-exclude-air"), None, &mut scenario.exclude_air);
        if !scenario.exclude_air {
            return;
        }
        form_choice(
            ui,
            "opt_air_mode",
            tr!("opt-air-source"),
            None,
            &mut scenario.air_mode,
            &[AirMode::Topography, AirMode::Rocktype],
            |mode| match mode {
                AirMode::Topography => tr!("opt-air-use-topography"),
                AirMode::Rocktype => tr!("opt-air-use-rocktype"),
            },
        );
        match scenario.air_mode {
            AirMode::Topography => {
                let layers: Vec<String> = project
                    .triangulations
                    .iter()
                    .map(|entry| entry.name.clone())
                    .chain(project.point_clouds.iter().map(|entry| entry.name.clone()))
                    .collect();
                form_combo(ui, "opt_air_topography", tr!("opt-air-topography"), &mut scenario.air_topography, &layers);
            }
            // Until the rock type field offers its values the value is typed.
            AirMode::Rocktype if values.is_empty() => {
                field_row(ui, tr!("opt-air-rocktype"), None, |ui, width| {
                    ui.add(egui::TextEdit::singleline(&mut scenario.air_rocktype).desired_width(width));
                });
            }
            AirMode::Rocktype => {
                form_combo(ui, "opt_air_rocktype", tr!("opt-air-rocktype"), &mut scenario.air_rocktype, &values);
            }
        }
    });
}

// ── Constants ──

/// The section-menu entry that opens the global constants, after the sections
/// of either editor. Returns whether it was clicked.
pub(super) fn constants_menu_item(ui: &mut egui::Ui, width: f32) -> bool {
    menu_item(ui, tinted_icon!("constant_pick.svg"), &tr!("opt-constants-open"), width, false, true)
}

/// The project's constants, which every scenario and schedule resolves
/// against: Save keeps them (the project reads unsaved) and the window stays;
/// Close, the cross or Escape bring back the dialog that opened it, asking
/// first about unsaved changes.
fn draw_constants_window(ui: &mut egui::Ui, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let Some(draft) = state.constants_draft.as_mut() else {
        return;
    };
    let ctx = ui.ctx().clone();
    let mut open = true;
    let mut close_requested = false;
    let mut save = false;
    let title = format!("{}{}", tr!("opt-constants-title"), if draft.dirty() { "*" } else { "" });
    let height = body_height(&ctx);
    DragableMenu::new("optimization_constants_window", title)
        .open(&mut open)
        .min_width(EDITOR_WIDTH)
        .max_width(EDITOR_WIDTH)
        .show(&ctx, |ui| {
            outline_fields(ui);
            let dirty = draft.dirty();
            let ConstantsDraft { rows, selection, .. } = &mut *draft;
            draw_constants(ui, rows, selection, height);
            menu_note(ui, tr!("opt-constants-note"));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-constants-save")).primary().enabled(dirty)).clicked() {
                    save = true;
                }
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() {
                    close_requested = true;
                }
            });
            if !draft.confirm_close && menu::dialog_cancelled(ui.ctx()) {
                close_requested = true;
            }
        });
    if !open {
        close_requested = true;
    }
    if save {
        commands.push(UiCommand::SaveOptimizationConstants {
            rows: draft.rows.clone(),
            then_close: false,
        });
    }
    if close_requested {
        if draft.dirty() {
            draft.confirm_close = true;
        } else {
            state.constants_draft = None;
            return;
        }
    }
    if !draft.confirm_close {
        return;
    }
    // Unsaved changes on closing: Save / Discard / Cancel.
    let mut open = true;
    let mut choice = None;
    let dialog = DragableMenu::new("optimization_constants_unsaved_dialog", tr!("opt-unsaved-title"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(380.0)
        .show(&ctx, |ui| {
            menu_note(ui, tr!("opt-constants-unsaved-message"));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-constants-save")).primary()).clicked() {
                    choice = Some(CloseChoice::Save);
                }
                if ui.add(MenuButton::new(tr!("opt-discard")).danger()).clicked() {
                    choice = Some(CloseChoice::Discard);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    choice = Some(CloseChoice::Keep);
                }
            });
        });
    if let Some(dialog) = dialog {
        ctx.move_to_top(dialog.response.layer_id);
    }
    if !open {
        choice = Some(CloseChoice::Keep);
    }
    match choice {
        Some(CloseChoice::Save) => {
            draft.confirm_close = false;
            commands.push(UiCommand::SaveOptimizationConstants {
                rows: draft.rows.clone(),
                then_close: true,
            });
        }
        Some(CloseChoice::Discard) => state.constants_draft = None,
        Some(CloseChoice::Keep) => draft.confirm_close = false,
        None => {}
    }
}

/// The constants grid: name, type, value and description, with groups.
fn draw_constants(ui: &mut egui::Ui, constants: &mut Vec<ConstantsRow>, selection: &mut Option<usize>, height: f32) {
    let rows: Vec<GridRow> = constants
        .iter()
        .map(|row| match row {
            ConstantsRow::Group { name, collapsed } => GridRow::Group {
                name: name.clone(),
                collapsed: *collapsed,
            },
            ConstantsRow::Constant(_) => GridRow::Item,
        })
        .collect();
    let names: Vec<String> = constants.iter().filter_map(ConstantsRow::constant).map(|constant| constant.name.clone()).collect();
    let columns = vec![
        GridColumn::new(tr!("opt-col-name"), 25.0),
        GridColumn::new(tr!("opt-col-type"), 15.0),
        GridColumn::new(tr!("opt-col-value"), 15.0),
        GridColumn::new(tr!("opt-col-description"), 45.0),
    ];
    let mut actions = DataGrid::new("opt_constants", columns)
        .buttons(GridButtons::default().reorder().groups())
        .fill_height(height - 8.0)
        .show(ui, selection, &rows, |ui, row, column| {
            let Some(ConstantsRow::Constant(constant)) = constants.get_mut(row) else {
                return;
            };
            match column {
                0 => {
                    let duplicate = names.iter().filter(|name| **name == constant.name).count() > 1;
                    let response = ui.add(egui::TextEdit::singleline(&mut constant.name).desired_width(ui.available_width()));
                    if constant.name.trim().is_empty() || duplicate {
                        mark_invalid(ui, &response);
                    }
                }
                1 => {
                    let mut value_type = constant.value.value_type();
                    egui::ComboBox::from_id_salt(("opt_constant_type", row))
                        .selected_text(value_type.label())
                        .width(ui.available_width().max(40.0))
                        .show_ui(ui, |ui| {
                            for option in ValueType::ALL {
                                ui.selectable_value(&mut value_type, option, option.label());
                            }
                        });
                    if value_type != constant.value.value_type() {
                        constant.value = ConstantValue::empty(value_type);
                    }
                }
                2 => {
                    let width = ui.available_width();
                    let id = egui::Id::unique(("opt_constant_value", row));
                    match &mut constant.value {
                        ConstantValue::Number(number) => {
                            number.edit(ui, id, width, (f64::MIN, f64::MAX));
                        }
                        ConstantValue::Text(text) => {
                            ui.add(egui::TextEdit::singleline(text).desired_width(width));
                        }
                        ConstantValue::YesNo(flag) => {
                            egui::ComboBox::from_id_salt(id)
                                .selected_text(if *flag { tr!("opt-yes") } else { tr!("opt-no") })
                                .width(width.max(40.0))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(flag, true, tr!("opt-yes"));
                                    ui.selectable_value(flag, false, tr!("opt-no"));
                                });
                        }
                    }
                }
                _ => {
                    ui.add(egui::TextEdit::singleline(&mut constant.description).desired_width(ui.available_width()));
                }
            }
        });
    apply_constants_actions(constants, selection, &mut actions);
}

fn apply_constants_actions(rows: &mut Vec<ConstantsRow>, selected: &mut Option<usize>, actions: &mut Vec<GridAction>) {
    for action in actions.drain(..) {
        match action {
            GridAction::Add => {
                let at = selected.map_or(rows.len(), |row| (row + 1).min(rows.len()));
                rows.insert(at, ConstantsRow::Constant(Constant::blank()));
                *selected = Some(at);
            }
            // A group row is deleted alone: the constants under it stay,
            // falling into the group above.
            GridAction::Delete(row) if row < rows.len() => {
                rows.remove(row);
                *selected = rows.len().checked_sub(1).map(|last| row.min(last));
            }
            GridAction::MoveUp(row) => *selected = move_constants_row(rows, row, true).or(*selected),
            GridAction::MoveDown(row) => *selected = move_constants_row(rows, row, false).or(*selected),
            GridAction::Drop { from, slot } if matches!(rows.get(from), Some(ConstantsRow::Constant(_))) => *selected = Some(move_to_slot(rows, from, slot)),
            GridAction::AddGroup(name) => {
                let name = unique_name(group_names(rows), &name);
                rows.push(ConstantsRow::Group { name, collapsed: false });
                *selected = Some(rows.len() - 1);
            }
            GridAction::ToggleGroup(row) => {
                if let Some(ConstantsRow::Group { collapsed, .. }) = rows.get_mut(row) {
                    *collapsed = !*collapsed;
                }
            }
            GridAction::RenameGroup(row, new_name) => {
                if let Some(ConstantsRow::Group { name, .. }) = rows.get_mut(row) {
                    *name = new_name;
                }
            }
            _ => {}
        }
    }
}

// ── Mining costs ──

fn draw_mining(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &BlockModelFields, selections: &mut ScenarioGridSelections) {
    option_group(ui, tr!("opt-group-mining-cost"), |ui| {
        form_value(
            ui,
            "opt_mining_cost",
            tr!("opt-mining-cost"),
            Some(tr!("opt-currency-agnostic")),
            &mut scenario.mining_cost,
            (0.0, f64::MAX),
            unit_per_tonne(),
        );

        let mut field = scenario.cost_field.clone();
        if form_combo(ui, "opt_cost_field", tr!("opt-cost-by-field"), &mut field, &fields.text).changed() {
            let values = fields.rocktype_values(&field);
            scenario.set_cost_field(field, &values);
        }
        if !scenario.cost_field.is_empty() {
            let values = fields.rocktype_values(&scenario.cost_field);
            let columns = vec![GridColumn::new(scenario.cost_field.clone(), 50.0), GridColumn::new(tr!("opt-col-cost"), 50.0)];
            let rows = plain_rows(scenario.rocktype_costs.len());
            let costs = &mut scenario.rocktype_costs;
            let mut actions = DataGrid::new("opt_rocktype_costs", columns).buttons(GridButtons::default().reorder().fill_all()).show(
                ui,
                &mut selections.rocktype_costs,
                &rows,
                |ui, row, column| {
                    let Some(cost) = costs.get_mut(row) else {
                        return;
                    };
                    if column == 0 {
                        if values.is_empty() {
                            cell_text(ui, &mut cost.rocktype, true);
                        } else {
                            cell_combo(ui, ("opt_cost_rocktype", row), &mut cost.rocktype, &values, tr!("opt-none-selected"), true);
                        }
                    } else {
                        ValueField::new(("opt_rocktype_cost", row), &mut cost.cost)
                            .range(0.0, f64::MAX)
                            .suffix(unit_per_tonne())
                            .show(ui);
                    }
                },
            );
            // Add all: a row for each rock type that has none, leaving the rows
            // already set as they are.
            if actions.contains(&GridAction::FillAll) {
                for value in &values {
                    if !scenario.rocktype_costs.iter().any(|cost| cost.rocktype == *value) {
                        scenario.rocktype_costs.push(RocktypeCost {
                            rocktype: value.clone(),
                            ..RocktypeCost::default()
                        });
                    }
                }
            }
            apply_flat(&mut scenario.rocktype_costs, &mut selections.rocktype_costs, &mut actions, RocktypeCost::default);
        }
    });

    draw_haulage(ui, "opt_rehab_cost", tr!("opt-rehab-cost"), &mut scenario.rehab_cost, &fields.numeric);
    draw_haulage(ui, "opt_waste_haulage", tr!("opt-waste-haulage"), &mut scenario.waste_haulage, &fields.numeric);
    draw_haulage(ui, "opt_ore_haulage", tr!("opt-ore-haulage"), &mut scenario.ore_haulage, &fields.numeric);
}

/// A haulage cost: a typed value (or constant), or a numeric block model field.
fn draw_haulage(ui: &mut egui::Ui, id: &str, title: String, haulage: &mut HaulageCost, numeric_fields: &[String]) {
    option_group(ui, title, |ui| {
        form_choice(
            ui,
            (id, "mode"),
            tr!("opt-cost-source"),
            None,
            &mut haulage.mode,
            &[HaulageMode::Value, HaulageMode::Field],
            |mode| match mode {
                HaulageMode::Value => tr!("opt-haulage-value"),
                HaulageMode::Field => tr!("opt-haulage-field"),
            },
        );
        match haulage.mode {
            HaulageMode::Value => {
                form_value(ui, (id, "value"), tr!("opt-col-cost"), None, &mut haulage.value, (0.0, f64::MAX), unit_per_tonne());
            }
            HaulageMode::Field => {
                let response = form_combo(ui, (id, "field"), tr!("opt-cost-field"), &mut haulage.field, numeric_fields);
                if haulage.field.is_empty() {
                    mark_invalid(ui, &response);
                }
            }
        }
    });
}

// ── Processing costs ──

/// The columns of the grid of a processing method's elements.
fn element_columns() -> Vec<GridColumn> {
    vec![
        GridColumn::new(tr!("opt-col-element"), 40.0),
        GridColumn::new(tr!("opt-col-recovery"), 30.0),
        GridColumn::new(tr!("opt-col-element-cost"), 30.0),
    ]
}

fn draw_processing(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &BlockModelFields, selections: &mut ScenarioGridSelections) {
    let values = fields.rocktype_values(&scenario.rocktype_field);
    let quality = scenario.quality_field.clone();
    let columns = vec![
        GridColumn::new(tr!("opt-col-name"), 18.0),
        GridColumn::new(tr!("opt-col-rocktype"), 18.0),
        GridColumn::new(tr!("opt-col-min-grade"), 16.0),
        GridColumn::new(tr!("opt-col-max-grade"), 16.0),
        GridColumn::new(tr!("opt-col-threshold"), 16.0),
        GridColumn::new(tr!("opt-col-processing-cost"), 16.0),
    ];
    option_group(ui, tr!("opt-group-methods"), |ui| {
        let rows = plain_rows(scenario.methods.len());
        let methods = &mut scenario.methods;
        let mut actions = DataGrid::new("opt_methods", columns)
            .buttons(GridButtons::default().reorder().fill_all())
            .show(ui, &mut selections.methods, &rows, |ui, row, column| {
                let Some(method) = methods.get_mut(row) else {
                    return;
                };
                match column {
                    0 => cell_text(ui, &mut method.name, true),
                    1 => {
                        if values.is_empty() {
                            cell_text(ui, &mut method.rocktype, false);
                        } else {
                            cell_combo(ui, ("opt_method_rocktype", row), &mut method.rocktype, &values, tr!("opt-any-rocktype"), false);
                        }
                    }
                    2 => {
                        ValueField::new(("opt_method_min", row), &mut method.min_grade).range(0.0, f64::MAX).show(ui);
                    }
                    3 => {
                        ValueField::new(("opt_method_max", row), &mut method.max_grade).range(0.0, f64::MAX).show(ui);
                    }
                    4 => {
                        ValueField::new(("opt_method_threshold", row), &mut method.threshold).range(0.0, f64::MAX).show(ui);
                    }
                    _ => {
                        ValueField::new(("opt_method_cost", row), &mut method.processing_cost)
                            .range(0.0, f64::MAX)
                            .suffix(unit_per_tonne())
                            .show(ui);
                    }
                }
            });
        // Add all: the selected method (or the last) copied once for each rock
        // type no method takes yet.
        if actions.contains(&GridAction::FillAll) {
            let template = selections
                .methods
                .and_then(|row| scenario.methods.get(row))
                .or(scenario.methods.last())
                .cloned()
                .unwrap_or_else(|| ProcessingMethod {
                    elements: vec![ElementCost {
                        element: quality.clone(),
                        ..ElementCost::default()
                    }],
                    ..ProcessingMethod::default()
                });
            for value in &values {
                if !scenario.methods.iter().any(|method| method.rocktype == *value) {
                    scenario.methods.push(ProcessingMethod {
                        name: value.clone(),
                        rocktype: value.clone(),
                        ..template.clone()
                    });
                }
            }
        }
        apply_flat(&mut scenario.methods, &mut selections.methods, &mut actions, || ProcessingMethod {
            elements: vec![ElementCost::default()],
            ..ProcessingMethod::default()
        });
    });

    // The grid below follows whichever method is selected above.
    let Some(index) = selections.methods.filter(|index| *index < scenario.methods.len()) else {
        if !scenario.methods.is_empty() {
            menu_note(ui, tr!("opt-select-method"));
        }
        return;
    };
    let revenues = scenario.revenues.clone();
    option_group(ui, tr!("opt-elements-of", name = scenario.methods[index].name.clone()), |ui| {
        let method = &mut scenario.methods[index];
        let rows = plain_rows(method.elements.len());
        let elements = &mut method.elements;
        let mut actions =
            DataGrid::new(("opt_elements", index), element_columns())
                .buttons(GridButtons::default().reorder())
                .show(ui, &mut selections.elements, &rows, |ui, row, column| {
                    let Some(element) = elements.get_mut(row) else {
                        return;
                    };
                    match column {
                        0 => cell_combo(ui, ("opt_element", index, row), &mut element.element, &fields.numeric, tr!("opt-none-selected"), true),
                        1 => {
                            ValueField::new(("opt_recovery", index, row), &mut element.recovery)
                                .range(0.0, 100.0)
                                .suffix(unit_percent())
                                .show(ui);
                        }
                        _ => {
                            // Per the sales unit its Revenues row sells it by.
                            let unit = revenues
                                .iter()
                                .find(|revenue| revenue.element == element.element)
                                .map(|revenue| unit_per_sales(revenue.sales_unit));
                            ValueField::new(("opt_element_cost", index, row), &mut element.cost)
                                .range(0.0, f64::MAX)
                                .suffix(unit.unwrap_or_default())
                                .show(ui);
                        }
                    }
                });
        apply_flat(&mut method.elements, &mut selections.elements, &mut actions, ElementCost::default);
        // Each element's units come from its Revenues row.
        let units: Vec<String> = method
            .elements
            .iter()
            .filter(|element| !element.element.is_empty())
            .filter_map(|element| {
                let row = revenues.iter().find(|revenue| revenue.element == element.element)?;
                Some(format!(
                    "{}: {}",
                    element.element,
                    tr!("opt-element-units", grade = row.grade_unit.label(), sales = row.sales_unit.label())
                ))
            })
            .collect();
        ui.label(egui::RichText::new(tr!("opt-element-cost-hint")).weak());
        for line in units {
            ui.label(egui::RichText::new(line).weak());
        }
    });

    // The method's other costs, under its elements.
    let OptimizationScenario {
        methods,
        ga_same_for_all: same_for_all,
        ..
    } = &mut *scenario;
    option_group(ui, tr!("opt-group-other-costs"), |ui| {
        form_value(
            ui,
            ("opt_haulage_factor", index),
            tr!("opt-haulage-factor"),
            None,
            &mut methods[index].haulage_factor,
            (0.0, f64::MAX),
            String::new(),
        );
        let before = methods[index].ga_cost.clone();
        form_value(
            ui,
            ("opt_ga", index),
            tr!("opt-ga-costs"),
            None,
            &mut methods[index].ga_cost,
            (0.0, f64::MAX),
            unit_per_tonne(),
        );
        // Turning it on makes every method follow this one.
        if form_toggle(ui, tr!("opt-ga-same-for-all"), None, same_for_all).changed() && *same_for_all {
            let value = methods[index].ga_cost.clone();
            for method in methods.iter_mut() {
                method.ga_cost = value.clone();
            }
        }
        if *same_for_all && methods[index].ga_cost != before {
            let value = methods[index].ga_cost.clone();
            for method in methods.iter_mut() {
                method.ga_cost = value.clone();
            }
        }
    });
}

// ── Revenues ──

fn draw_revenues(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &BlockModelFields, selections: &mut ScenarioGridSelections) {
    option_group(ui, tr!("opt-group-prices"), |ui| {
        let columns = vec![
            GridColumn::new(tr!("opt-col-element"), 26.0),
            GridColumn::new(tr!("opt-col-grade-unit"), 16.0),
            GridColumn::new(tr!("opt-col-sales-unit"), 16.0),
            GridColumn::new(tr!("opt-col-price"), 21.0),
            GridColumn::new(tr!("opt-col-selling-costs"), 21.0),
        ];
        let rows = plain_rows(scenario.revenues.len());
        let revenues = &mut scenario.revenues;
        let mut actions =
            DataGrid::new("opt_revenues", columns)
                .buttons(GridButtons::default().reorder().fill_all())
                .show(ui, &mut selections.revenues, &rows, |ui, row, column| {
                    let Some(revenue) = revenues.get_mut(row) else {
                        return;
                    };
                    match column {
                        0 => cell_combo(ui, ("opt_revenue_element", row), &mut revenue.element, &fields.numeric, tr!("opt-none-selected"), true),
                        1 => cell_choice(ui, ("opt_grade_unit", row), &mut revenue.grade_unit, &GradeUnit::ALL, GradeUnit::label),
                        2 => cell_choice(ui, ("opt_sales_unit", row), &mut revenue.sales_unit, &SalesUnit::ALL, SalesUnit::label),
                        3 => {
                            let unit = unit_per_sales(revenue.sales_unit);
                            ValueField::new(("opt_price", row), &mut revenue.price).range(0.0, f64::MAX).suffix(unit).show(ui);
                        }
                        _ => {
                            let unit = unit_per_sales(revenue.sales_unit);
                            ValueField::new(("opt_selling_cost", row), &mut revenue.selling_cost)
                                .range(0.0, f64::MAX)
                                .suffix(unit)
                                .show(ui);
                        }
                    }
                });
        // Add all: every element a processing method recovers that has no row yet.
        if actions.contains(&GridAction::FillAll) {
            for element in scenario.processed_elements() {
                if !scenario.revenues.iter().any(|revenue| revenue.element == element) {
                    scenario.revenues.push(RevenueRow { element, ..RevenueRow::default() });
                }
            }
        }
        apply_flat(&mut scenario.revenues, &mut selections.revenues, &mut actions, RevenueRow::default);
        ui.label(egui::RichText::new(tr!("opt-units-hint")).weak());
    });
}

// ── Constraints ──

fn draw_constraints(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &BlockModelFields, selections: &mut ScenarioGridSelections) {
    let OptimizationScenario { slope, constants, .. } = scenario;
    let previous = slope.mode;
    option_group(ui, tr!("opt-overall-slope"), |ui| {
        form_choice(
            ui,
            "opt_slope_mode",
            tr!("opt-slope-source"),
            None,
            &mut slope.mode,
            &[SlopeMode::Single, SlopeMode::Rosette, SlopeMode::Field],
            |mode| match mode {
                SlopeMode::Single => tr!("opt-slope-default-angle"),
                SlopeMode::Rosette => tr!("opt-slope-use-rosettes"),
                SlopeMode::Field => tr!("opt-slope-field"),
            },
        );
        match slope.mode {
            SlopeMode::Single => {
                form_value(ui, "opt_slope_angle", tr!("opt-slope-angle"), None, &mut slope.angle, (1.0, 89.0), unit_degrees());
            }
            SlopeMode::Rosette => menu_note(ui, tr!("opt-slope-rosettes-hint")),
            SlopeMode::Field => {
                let response = form_combo(ui, "opt_slope_field", tr!("opt-slope-angle-field"), &mut slope.field, &fields.numeric);
                if slope.field.is_empty() {
                    mark_invalid(ui, &response);
                }
                // The default angle is the field's fallback.
                form_value(
                    ui,
                    "opt_slope_angle",
                    tr!("opt-slope-fallback-angle"),
                    Some(tr!("opt-slope-field-hint")),
                    &mut slope.angle,
                    (1.0, 89.0),
                    unit_degrees(),
                );
            }
        }
    });
    if slope.mode != SlopeMode::Rosette {
        return;
    }
    // Choosing rosettes starts the table with one row: 45 degrees all round.
    if previous != SlopeMode::Rosette && slope.rosette.is_empty() {
        slope.rosette.push(RosetteRow::default());
    }
    option_group(ui, tr!("opt-group-rosette"), |ui| {
        let bearings: Vec<f64> = slope.rosette.iter().map(|row| row.bearing.rem_euclid(360.0)).collect();
        let rows = plain_rows(slope.rosette.len());
        let sectors = slope.sectors(constants);
        let rosette = &mut slope.rosette;
        // The table takes the left half and the circle the right, the table
        // as tall as the circle.
        let mut actions = Vec::new();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let half = (ui.available_width() - 8.0) / 2.0;
            actions = ui
                .allocate_ui_with_layout(egui::vec2(half, ROSETTE_HEIGHT), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(half);
                    DataGrid::new(
                        "opt_rosette",
                        vec![GridColumn::new(tr!("opt-col-bearing"), 50.0), GridColumn::new(tr!("opt-col-angle"), 50.0)],
                    )
                    .buttons(GridButtons::default())
                    .fill_height(ROSETTE_HEIGHT)
                    .show(ui, &mut selections.rosette, &rows, |ui, row, column| {
                        let Some(entry) = rosette.get_mut(row) else {
                            return;
                        };
                        if column == 0 {
                            // A plain number: a bearing is not worth a constant.
                            let response = entry
                                .bearing
                                .edit_with_suffix(ui, egui::Id::unique(("opt_rosette_bearing", row)), ui.available_width(), (0.0, 360.0), &unit_degrees());
                            let same = bearings.iter().enumerate().any(|(other, value)| other != row && *value == bearings[row]);
                            if same {
                                mark_invalid(ui, &response);
                            }
                        } else {
                            ValueField::new(("opt_rosette_angle", row), &mut entry.angle)
                                .range(1.0, 89.0)
                                .suffix(unit_degrees())
                                .show(ui);
                        }
                    })
                })
                .inner;
            ui.allocate_ui_with_layout(egui::vec2(half, ROSETTE_HEIGHT), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(half);
                match &sectors {
                    Ok(sectors) => draw_rosette(ui, sectors),
                    Err(RosetteIssue::SameBearing) => menu_note(ui, tr!("opt-rosette-same-bearing")),
                    Err(RosetteIssue::NoRows) => menu_note(ui, tr!("opt-rosette-no-rows")),
                }
            });
        });
        // A new row starts one degree past the selected one (or the last), so
        // the circle stays drawn; at 359 it repeats the bearing and the
        // "same bearing" warning stands in for the circle.
        let base = selections.rosette.filter(|row| *row < slope.rosette.len()).or(slope.rosette.len().checked_sub(1));
        let next_bearing = base.map_or(0.0, |row| {
            let bearing = slope.rosette[row].bearing;
            if bearing.rem_euclid(360.0) >= 359.0 { bearing } else { bearing + 1.0 }
        });
        apply_flat(&mut slope.rosette, &mut selections.rosette, &mut actions, || RosetteRow {
            bearing: next_bearing,
            ..RosetteRow::default()
        });
        // Only the interpolations the rows allow are offered; one that no
        // longer fits (after deleting rows) falls back to the sectors.
        let rows = sectors.as_ref().map_or(0, Vec::len);
        if rows < slope.interpolation.min_rows() {
            slope.interpolation = RosetteInterpolation::Step;
        }
        ui.add_space(4.0);
        let available: Vec<RosetteInterpolation> = RosetteInterpolation::available(rows).collect();
        form_choice(
            ui,
            "opt_rosette_interpolation",
            tr!("opt-interpolation"),
            Some(tr!("opt-interpolation-hint")),
            &mut slope.interpolation,
            &available,
            RosetteInterpolation::label,
        );
    });
}

// ── Outputs ──

fn draw_outputs(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &BlockModelFields, others: &[OptimizationScenario], commands: &mut Vec<UiCommand>) {
    let prefix_taken = shell_prefix_taken(others, scenario.id, &scenario.output.shell_layer_name);
    // The shell value is a category (each shell its own colour), so only text
    // fields are offered, less any the settings use, so a run cannot overwrite one.
    let free: Vec<String> = {
        let used = scenario.used_fields();
        fields.text.iter().filter(|name| !used.contains(name.as_str())).cloned().collect()
    };
    let shell_field_issue = scenario.shell_field_target(fields).err();
    let scenario_block_model = scenario.block_model.clone();
    let output = &mut scenario.output;
    if !output.shell_field.is_empty() && !free.contains(&output.shell_field) && fields.all.contains(&output.shell_field) {
        output.shell_field.clear();
    }

    option_group(ui, tr!("opt-group-run-mode"), |ui| {
        form_choice(
            ui,
            "opt_shell_mode",
            tr!("opt-select-mode"),
            None,
            &mut output.mode,
            &[ShellMode::Single, ShellMode::Multiple],
            |mode| match mode {
                ShellMode::Single => tr!("opt-mode-single"),
                ShellMode::Multiple => tr!("opt-mode-multiple"),
            },
        );
    });

    if output.mode == ShellMode::Multiple {
        option_group(ui, tr!("opt-factors-title"), |ui| {
            // Two ways to give the factors: a range, or a list typed out.
            form_choice(
                ui,
                "opt_factor_input",
                tr!("opt-factor-input"),
                None,
                &mut output.factor_input,
                &[FactorInput::Range, FactorInput::List],
                |input| match input {
                    FactorInput::Range => tr!("opt-factor-input-range"),
                    FactorInput::List => tr!("opt-factor-input-list"),
                },
            );
            if output.factor_input == FactorInput::List {
                // A plain string, read when run: the list is not worth constants.
                let response = field_row(ui, tr!("opt-factor-list"), Some(tr!("opt-factor-list-hint")), |ui, width| {
                    ui.add(egui::TextEdit::singleline(&mut output.factor_list).desired_width(width))
                });
                if let Err(issue) = parse_factor_list(&output.factor_list) {
                    mark_invalid(ui, &response);
                    menu_note(ui, issue.message());
                }
                return;
            }
            // Plain numbers: a revenue factor range is not worth a constant.
            let from_changed = field_row(ui, tr!("opt-factor-from"), None, |ui, width| {
                output.factor_from.edit(ui, egui::Id::unique("opt_factor_from"), width, (0.0, f64::MAX))
            })
            .changed();
            let to_changed = field_row(ui, tr!("opt-factor-to"), None, |ui, width| {
                output.factor_to.edit(ui, egui::Id::unique("opt_factor_to"), width, (0.0, f64::MAX))
            })
            .changed();
            let (from, to) = (output.factor_from, output.factor_to);
            // Step and number of shells follow each other: change one and the
            // other is worked out from it between the two ends.
            let mut step = output.factor_step;
            if field_row(ui, tr!("opt-factor-step"), None, |ui, width| {
                step.edit(ui, egui::Id::unique("opt_factor_step"), width, (0.0001, f64::MAX))
            })
            .changed()
            {
                output.factor_step = step;
                if let Some(count) = shell_count_for_step(from, to, step) {
                    output.shell_count = count.max(2);
                    output.factor_step = shell_step_for_count(from, to, output.shell_count).unwrap_or(step);
                }
                output.match_count_to_step();
            }
            let mut count = output.shell_count;
            if field_row(ui, tr!("opt-shell-count"), None, |ui, width| {
                edit_count(ui, egui::Id::unique("opt_shell_count"), &mut count, width, (2, 1000))
            })
            .changed()
            {
                output.shell_count = count;
                if let Some(step) = shell_step_for_count(from, to, count) {
                    output.factor_step = step;
                }
                // The step was rounded; the count shown when the box is left is
                // the one that step makes, which is what a run makes.
                output.match_count_to_step();
            }
            if (from_changed || to_changed)
                && let Some(step) = shell_step_for_count(from, to, output.shell_count)
            {
                output.factor_step = step;
                output.match_count_to_step();
            }
            if to <= from {
                menu_note(ui, tr!("opt-factor-range-invalid"));
            }
        });

        option_group(ui, tr!("opt-group-directional"), |ui| {
            form_toggle(
                ui,
                tr!("opt-directional-shells"),
                Some(tr!("opt-directional-shells-hint")),
                &mut output.use_directional_shells,
            );
            if !output.use_directional_shells {
                return;
            }
            let can_pick = !scenario_block_model.is_empty() && !fields.all.is_empty();
            // Picked shows the point's whole metres on hover; a click picks
            // again, with the old point marked in the model.
            let (label, hint) = match output.shell_start {
                Some((x, y)) => (tr!("opt-picked"), tr!("opt-start-point", x = format!("{x:.0}"), y = format!("{y:.0}"))),
                None => (tr!("opt-pick-start"), tr!("opt-pick-start-hint")),
            };
            let picked = output.shell_start.is_some();
            let response = field_row(ui, tr!("opt-start-point-label"), None, |ui, width| {
                ui.add_enabled(can_pick, MenuButton::new(label).selected(picked).min_width(width)).hint(hint)
            });
            if !picked {
                mark_invalid(ui, &response);
            }
            if response.clicked() {
                commands.push(UiCommand::BeginShellStartPick);
            }
            form_choice(
                ui,
                "opt_shell_direction",
                tr!("opt-mining-direction"),
                None,
                &mut output.shell_direction,
                &ShellDirection::ALL,
                ShellDirection::label,
            );
        });
    }

    option_group(ui, tr!("opt-group-files"), |ui| {
        // Reports: the switch, and where they go. Turning it on with no folder
        // yet asks for one straight away.
        let switched = form_toggle(ui, tr!("opt-create-reports"), Some(tr!("opt-create-reports-hint")), &mut output.create_reports).changed();
        // A browser downloads the report instead, so asks for no folder.
        let wasm = cfg!(target_arch = "wasm32");
        if switched && output.create_reports && output.reports_folder.is_empty() && !wasm {
            commands.push(UiCommand::ChooseOptimizationReportsFolder);
        }
        if output.create_reports {
            field_row(ui, tr!("opt-reports-folder"), None, |ui, width| {
                let browse_width = 80.0;
                let mut shown = output.reports_folder.clone();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut shown)
                        .hint_text(if wasm { tr!("opt-reports-downloaded") } else { tr!("opt-reports-folder-hint") })
                        .interactive(false)
                        .desired_width(width - browse_width - ui.spacing().item_spacing.x),
                );
                if output.reports_folder.is_empty() && !wasm {
                    mark_invalid(ui, &response);
                }
                // A browser has no folders to write into.
                if ui.add_enabled(!wasm, MenuButton::new(tr!("opt-browse")).min_width(browse_width)).clicked() {
                    commands.push(UiCommand::ChooseOptimizationReportsFolder);
                }
            });
        }

        // Shells: solid, surface, both or neither, and the name they share.
        form_toggle(ui, tr!("opt-create-shell-solids"), None, &mut output.shell_as_solid);
        form_toggle(ui, tr!("opt-create-shell-surfaces"), None, &mut output.shell_as_surface);
        // The shells' names start with this, so no two scenarios may share it.
        if output.shell_as_solid || output.shell_as_surface {
            field_row(ui, tr!("opt-shell-name-prefix"), Some(tr!("opt-shell-name-prefix-hint")), |ui, width| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut output.shell_layer_name)
                        .hint_text(tr!("opt-layer-name"))
                        .desired_width(width),
                );
                let empty = output.shell_layer_name.trim().is_empty();
                if empty || prefix_taken {
                    mark_invalid(ui, &response);
                    response.hint(if empty { tr!("opt-shell-prefix-empty") } else { tr!("opt-shell-prefix-taken") });
                }
            });
        }
    });

    option_group(ui, tr!("opt-group-write-back"), |ui| {
        // The shell value in the block model - its number or its revenue
        // factor - in an existing field or a new one.
        form_toggle(ui, tr!("opt-write-shell-field"), Some(tr!("opt-shell-field-value-hint")), &mut output.write_shell_field);
        if !output.write_shell_field {
            return;
        }
        form_choice(
            ui,
            "opt_shell_field_value",
            tr!("opt-shell-field-value"),
            None,
            &mut output.shell_field_value,
            &ShellFieldValue::ALL,
            ShellFieldValue::label,
        );
        form_choice(
            ui,
            "opt_shell_field_mode",
            tr!("opt-shell-field-target"),
            None,
            &mut output.shell_field_mode,
            &[ShellFieldMode::Existing, ShellFieldMode::New],
            |mode| match mode {
                ShellFieldMode::Existing => tr!("opt-shell-field-existing"),
                ShellFieldMode::New => tr!("opt-shell-field-new"),
            },
        );
        // A field that exists already is overwritten, so a rerun updates the
        // field its first run made.
        let response = match output.shell_field_mode {
            ShellFieldMode::Existing => form_combo(ui, "opt_shell_field", tr!("opt-shell-field-name"), &mut output.shell_field, &free),
            ShellFieldMode::New => field_row(ui, tr!("opt-shell-field-name"), None, |ui, width| {
                ui.add(
                    egui::TextEdit::singleline(&mut output.shell_new_field)
                        .hint_text(tr!("opt-shell-new-field-hint"))
                        .desired_width(width),
                )
            }),
        };
        if let Some(issue) = &shell_field_issue {
            mark_invalid(ui, &response);
            response.hint(issue);
        }
    });
}
