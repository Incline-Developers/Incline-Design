//! The Optimization workspace's windows: the scenarios list, and the editor a
//! scenario opens in. Only one of the two is up at a time - adding or editing
//! hides the list, closing the editor brings it back.
//!
//! The editor is one page of sections, top to bottom: Inputs, Constants,
//! Mining costs, Processing costs, Revenues, Outputs. See
//! `.claude/skills/optimization/SKILL.md` for what each field means and
//! `.claude/skills/ui-components/SKILL.md` for the grid and the input/constant
//! field the sections are built from.

use std::time::Duration;

use crate::{
    i18n::tr,
    model::{
        block_model::OpenBlockModel,
        optimization::{
            AirMode, BlockModelFields, Constant, ConstantValue, ConstantsRow, ElementCost, FactorInput, FieldValue, GradeUnit, HaulageCost, HaulageMode, OptimizationScenario,
            ProcessingMethod, RevenueRow, RocktypeCost, RosetteInterpolation, RosetteIssue, RosetteRow, SalesUnit, ShellDirection, ShellFieldMode, ShellFieldValue, ShellMode,
            SlopeMode, ValueType, group_names, move_constants_row, parse_factor_list, shell_count_for_step, shell_step_for_count, unique_name,
        },
    },
    ui::{
        state::{EditorState, OptimizationState, ScenarioDraft, ScenarioGridSelections, ScenarioStatus, ScenarioTab, UiCommand, UiProjectView},
        themed_icon, unthemed_icon,
        widgets::{
            data_grid::{DataGrid, GridAction, GridButtons, GridColumn, GridRow, apply_flat, move_to_slot},
            menu::{self, DragableMenu, MenuButton, menu_note},
            rosette_diagram::{HEIGHT as ROSETTE_HEIGHT, draw_rosette},
            toolbar::ToolbarButton,
            value_field::{self, FieldEditor, ValueField, edit_count, mark_invalid},
        },
    },
};

const EDITOR_WIDTH: f32 = 800.0;
/// Space between the settings and the section menu beside them.
const SIDE_GAP: f32 = 8.0;
const MENU_ITEM_HEIGHT: f32 = 32.0;
const MENU_ICON: f32 = 22.0;
const LIST_WIDTH: f32 = 560.0;
const ICON_SIDE: f32 = 26.0;
const LABEL_WIDTH: f32 = 270.0;
/// Width of the starting point and mining direction stacks, label over control.
const STACK_WIDTH: f32 = (CONTROL_WIDTH - 8.0) / 2.0;
const CONTROL_WIDTH: f32 = 300.0;

pub(crate) fn draw_optimization_dialogs(ui: &mut egui::Ui, editor: &mut EditorState, block_models: &[OpenBlockModel], project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // The progress rings move while a run goes, whichever window is up.
    if editor.optimization.any_running() {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
    // While a starting point is picked in the viewport the editor steps aside;
    // the prompt is the viewport banner (`ui::viewport_message`).
    if editor.optimization.start_pick.is_some() {
        editor.optimization.pick_was_active = true;
    } else if editor.optimization.draft.is_some() {
        // The Escape that cancelled the pick is still in this frame's input; it
        // must not also close the editor it returns to.
        if std::mem::take(&mut editor.optimization.pick_was_active) {
            menu::dialog_cancel_pressed(ui.ctx());
        }
        draw_scenario_editor(ui, &mut editor.optimization, block_models, project, commands);
    } else if editor.optimization.list_open {
        draw_scenarios_list(ui, &mut editor.optimization, commands);
    }
    value_field::draw_constant_picker(ui.ctx());
}

// ── Scenarios list ──

fn draw_scenarios_list(ui: &mut egui::Ui, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let mut open = true;
    let mut close = false;
    let mut run = None;
    // What the buttons ask for, sent after any name still being typed is committed.
    let mut actions: Vec<UiCommand> = Vec::new();
    DragableMenu::new("optimization_scenarios_dialog", tr!("opt-scenarios-title"))
        .open(&mut open)
        .min_width(LIST_WIDTH)
        .max_width(LIST_WIDTH)
        .show(ui.ctx(), |ui| {
            ui.add_space(4.0);
            if state.scenarios.is_empty() {
                menu_note(ui, tr!("opt-scenarios-empty"));
            }
            for scenario in &state.scenarios {
                let status = state.status(scenario);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        // Drawn right to left, so they read: edit, duplicate,
                        // delete, state, run.
                        if play_button(ui, status).clicked() {
                            run = Some(scenario.id);
                        }
                        status_icon(ui, status);
                        let delete = ToolbarButton::new(egui::Image::new(unthemed_icon!("delete_scenario.svg")), tr!("opt-delete"))
                            .id_salt(("opt_delete", scenario.id))
                            .button_side(ICON_SIDE);
                        if ui.add(delete).clicked() {
                            actions.push(UiCommand::DeleteOptimizationScenario(scenario.id));
                        }
                        let duplicate = ToolbarButton::new(egui::Image::new(themed_icon!(ui, "duplicate.svg")), tr!("opt-duplicate"))
                            .id_salt(("opt_duplicate", scenario.id))
                            .button_side(ICON_SIDE);
                        if ui.add(duplicate).clicked() {
                            actions.push(UiCommand::DuplicateOptimizationScenario(scenario.id));
                        }
                        let edit = ToolbarButton::new(egui::Image::new(themed_icon!(ui, "edit.svg")), tr!("opt-edit"))
                            .id_salt(("opt_edit", scenario.id))
                            .button_side(ICON_SIDE);
                        if ui.add(edit).clicked() {
                            actions.push(UiCommand::EditOptimizationScenario(scenario.id));
                        }
                        draw_name_field(ui, scenario.id, &scenario.name, commands);
                    });
                });
                ui.add_space(2.0);
            }
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-add-scenario")).primary()).clicked() {
                    actions.push(UiCommand::AddOptimizationScenario);
                }
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
                // Import and export take the left of the same row.
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let accent = menu::accent_fill(ui.visuals());
                    let import = ToolbarButton::new(egui::Image::new(unthemed_icon!("load_scenarios.svg")).tint(accent), tr!("opt-import-json"))
                        .id_salt("opt_import")
                        .button_side(ICON_SIDE);
                    if ui.add(import).clicked() {
                        actions.push(UiCommand::ImportOptimizationScenarios);
                    }
                    let export = ToolbarButton::new(egui::Image::new(unthemed_icon!("export_scenarios.svg")).tint(accent), tr!("opt-export-json"))
                        .id_salt("opt_export")
                        .button_side(ICON_SIDE);
                    if ui.add_enabled_ui(!state.scenarios.is_empty(), |ui| ui.add(export)).inner.clicked() {
                        actions.push(UiCommand::ExportOptimizationScenarios);
                    }
                });
            });
        });
    // A button clicked while a name is still being typed commits the name
    // first: its field may not have lost focus yet this frame.
    if !actions.is_empty() || run.is_some() || close || !open {
        commit_typed_names(ui.ctx(), state, commands);
    }
    commands.extend(actions);
    if let Some(id) = run {
        let busy = state
            .scenarios
            .iter()
            .find(|scenario| scenario.id == id)
            .is_some_and(|scenario| matches!(state.status(scenario), ScenarioStatus::Running(_) | ScenarioStatus::Queued));
        commands.push(if busy {
            UiCommand::CancelOptimizationScenario(id)
        } else {
            UiCommand::RunOptimizationScenario(id)
        });
    }
    if close || !open {
        state.list_open = false;
    }
}

fn name_field_ids(id: u64) -> (egui::Id, egui::Id) {
    let field_id = egui::Id::new(("opt_scenario_name", id));
    (field_id, field_id.with("typed"))
}

/// Send the rename of every name still being typed, and take the focus from
/// its field, so the next command sees the new name.
fn commit_typed_names(ctx: &egui::Context, state: &OptimizationState, commands: &mut Vec<UiCommand>) {
    for scenario in &state.scenarios {
        let (field_id, buffer_id) = name_field_ids(scenario.id);
        let Some(typed) = ctx.data_mut(|data| data.remove_temp::<String>(buffer_id)) else {
            continue;
        };
        ctx.memory_mut(|memory| memory.surrender_focus(field_id));
        let typed = typed.trim();
        if !typed.is_empty() && typed != scenario.name {
            commands.push(UiCommand::RenameOptimizationScenario {
                id: scenario.id,
                name: typed.to_owned(),
            });
        }
    }
}

/// The scenario's name, typed in place; the rename is sent when focus leaves,
/// or when any button of the list is clicked ([`commit_typed_names`]).
fn draw_name_field(ui: &mut egui::Ui, id: u64, name: &str, commands: &mut Vec<UiCommand>) {
    let (field_id, buffer_id) = name_field_ids(id);

    // What was typed is kept until the field is left, whatever the focus
    // state says on the frame it is lost.
    let pending = ui.data_mut(|data| data.get_temp::<String>(buffer_id));
    let mut text = pending.clone().unwrap_or_else(|| name.to_owned());
    let response = ui.add(egui::TextEdit::singleline(&mut text).id(field_id).desired_width(ui.available_width()));
    if response.changed() {
        ui.data_mut(|data| data.insert_temp(buffer_id, text.clone()));
    }
    if pending.is_some() && !response.has_focus() {
        let typed = text.trim();
        if !typed.is_empty() && typed != name {
            commands.push(UiCommand::RenameOptimizationScenario { id, name: typed.to_owned() });
        }
        ui.data_mut(|data| data.remove::<String>(buffer_id));
    }
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
    ui.add_sized([ICON_SIDE, ICON_SIDE], image).on_hover_text(tooltip);
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
    let tooltip = match running {
        Some(_) if status == ScenarioStatus::Queued => format!("{}\n{}", tr!("opt-status-queued"), tr!("opt-cancel-run")),
        Some(progress) => format!(
            "{}\n{}",
            tr!("opt-status-running-percent", percent = ((progress * 100.0) as u32).to_string()),
            tr!("opt-cancel-run")
        ),
        None => tr!("opt-run"),
    };
    response.on_hover_text(tooltip)
}

// ── Scenario editor ──

fn model_fields(block_models: &[OpenBlockModel], name: &str) -> BlockModelFields {
    block_models.iter().find(|model| model.name == name).map(BlockModelFields::of_open).unwrap_or_default()
}

fn draw_scenario_editor(ui: &mut egui::Ui, state: &mut OptimizationState, block_models: &[OpenBlockModel], project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let Some(draft) = state.draft.as_mut() else {
        return;
    };
    value_field::publish_constants(ui.ctx(), &draft.scenario.constants);
    let ctx = ui.ctx().clone();
    let mut open = true;
    let mut close_requested = false;
    let mut save = false;
    let title = format!("{}: {}{}", tr!("opt-scenario-title"), draft.scenario.name, if draft.dirty() { "*" } else { "" });
    let body_height = (ctx.content_rect().height() - 260.0).clamp(240.0, 560.0);
    let menu_expanded = draft.menu_expanded;
    let side_width = section_menu_width(&ctx, menu_expanded);
    let mut toggle_menu = false;
    DragableMenu::new("optimization_scenario_editor", title)
        .open(&mut open)
        .min_width(EDITOR_WIDTH + SIDE_GAP + side_width)
        .max_width(EDITOR_WIDTH + SIDE_GAP + side_width)
        .show(&ctx, |ui| {
            outline_fields(ui);
            let ready = !draft.scenario.name.trim().is_empty() && (draft.scenario != draft.saved || draft.is_new);
            let ScenarioDraft { scenario, selections, tab, .. } = &mut *draft;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // The settings keep one width however the section menu is
                // sized: the menu grows the window to the right instead.
                let content = ui.allocate_ui_with_layout(egui::vec2(EDITOR_WIDTH - 16.0, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(EDITOR_WIDTH - 16.0);
                    // As tall as the menu's first entry, so the rule under it
                    // lines up with the one under the menu's toggle.
                    ui.allocate_ui_with_layout(egui::vec2(EDITOR_WIDTH - 16.0, MENU_ITEM_HEIGHT), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(tab.label()).size(17.0).strong());
                    });
                    ui.separator();
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
                                ScenarioTab::Constants => draw_constants(ui, scenario, selections, body_height),
                                ScenarioTab::MiningCosts => draw_mining(ui, scenario, &fields, selections),
                                ScenarioTab::ProcessingCosts => draw_processing(ui, scenario, &fields, selections),
                                ScenarioTab::Revenues => draw_revenues(ui, scenario, &fields, selections),
                                ScenarioTab::Constraints => draw_constraints(ui, scenario, &fields, selections),
                                ScenarioTab::Outputs => draw_outputs(ui, scenario, &fields, commands),
                            }
                        });
                });
                // A rule between the settings and the menu.
                ui.add_space(SIDE_GAP / 2.0);
                let rule = content.response.rect;
                let x = ui.cursor().left();
                ui.painter().vline(x, rule.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
                ui.add_space(SIDE_GAP / 2.0);
                let choice = draw_section_menu(ui, content.response.rect.height(), side_width, menu_expanded, tab, ready);
                toggle_menu = choice.toggle;
                save = choice.save;
                close_requested |= choice.close;
            });
            // Escape closes the editor - unless a window over it wants the key.
            if !draft.confirm_close && !value_field::picker_open(ui.ctx()) && menu::dialog_cancel_pressed(ui.ctx()) {
                close_requested = true;
            }
        });
    if toggle_menu {
        draft.menu_expanded = !draft.menu_expanded;
    }
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
fn outline_fields(ui: &mut egui::Ui) {
    if !ui.visuals().dark_mode {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(185));
    }
}

// ── Section menu ──

struct MenuChoice {
    toggle: bool,
    save: bool,
    close: bool,
}

fn section_icon(tab: ScenarioTab) -> egui::Image<'static> {
    egui::Image::new(match tab {
        ScenarioTab::Inputs => unthemed_icon!("section_inputs.svg"),
        ScenarioTab::Constants => unthemed_icon!("constant_pick.svg"),
        ScenarioTab::MiningCosts => unthemed_icon!("section_mining_costs.svg"),
        ScenarioTab::ProcessingCosts => unthemed_icon!("section_processing_costs.svg"),
        ScenarioTab::Revenues => unthemed_icon!("section_revenues.svg"),
        ScenarioTab::Constraints => unthemed_icon!("section_constraints.svg"),
        ScenarioTab::Outputs => unthemed_icon!("section_outputs.svg"),
    })
}

/// How wide the section menu is: icons only, or wide enough for the longest
/// section name in the current language.
fn section_menu_width(ctx: &egui::Context, expanded: bool) -> f32 {
    let narrow = MENU_ITEM_HEIGHT + 4.0;
    if !expanded {
        return narrow;
    }
    let painter = ctx.layer_painter(egui::LayerId::background());
    let font = egui::FontId::proportional(14.0);
    let labels = ScenarioTab::ALL.into_iter().map(ScenarioTab::label).chain([tr!("opt-save-scenario"), tr!("common-close")]);
    let longest = labels
        .map(|label| painter.layout_no_wrap(label, font.clone(), egui::Color32::WHITE).size().x)
        .fold(0.0, f32::max);
    narrow + longest + 16.0
}

/// The menu beside the settings: a collapse arrow, one entry per section, and
/// Save and Close pushed to the bottom. `height` is the settings' height.
fn draw_section_menu(ui: &mut egui::Ui, height: f32, width: f32, expanded: bool, current: &mut ScenarioTab, can_save: bool) -> MenuChoice {
    let mut choice = MenuChoice {
        toggle: false,
        save: false,
        close: false,
    };
    ui.allocate_ui_with_layout(egui::vec2(width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        let top = ui.cursor().top();
        let (toggle_icon, toggle_tip) = if expanded {
            (egui::Image::new(unthemed_icon!("menu_collapse.svg")), tr!("opt-menu-collapse"))
        } else {
            (egui::Image::new(unthemed_icon!("menu_expand.svg")), tr!("opt-menu-expand"))
        };
        choice.toggle = menu_item(ui, toggle_icon, &toggle_tip, width, expanded, false, true, false);
        ui.separator();
        ui.spacing_mut().item_spacing.y = 2.0;
        for tab in ScenarioTab::ALL {
            if menu_item(ui, section_icon(tab), &tab.label(), width, expanded, *current == tab, true, !expanded) {
                *current = tab;
            }
        }
        let bottom = 2.0 * MENU_ITEM_HEIGHT + 2.0 + ui.spacing().item_spacing.y * 2.0 + 8.0;
        ui.add_space((top + height - ui.cursor().top() - bottom).max(4.0));
        ui.separator();
        choice.save = menu_item(
            ui,
            egui::Image::new(unthemed_icon!("menu_save.svg")),
            &tr!("opt-save-scenario"),
            width,
            expanded,
            false,
            can_save,
            !expanded,
        );
        choice.close = menu_item(
            ui,
            egui::Image::new(unthemed_icon!("menu_close.svg")),
            &tr!("common-close"),
            width,
            expanded,
            false,
            true,
            !expanded,
        );
    });
    choice
}

/// One entry of the section menu: its icon, and its name when `expanded`
/// (otherwise the name is the tooltip). Returns whether it was clicked.
#[allow(clippy::too_many_arguments)]
fn menu_item(ui: &mut egui::Ui, icon: egui::Image<'static>, label: &str, width: f32, expanded: bool, selected: bool, enabled: bool, tooltip: bool) -> bool {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, MENU_ITEM_HEIGHT), sense);
    let visuals = ui.visuals();
    let fill = if selected {
        Some(visuals.selection.bg_fill)
    } else if response.hovered() && enabled {
        Some(visuals.widgets.hovered.weak_bg_fill)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, fill);
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + (MENU_ITEM_HEIGHT + 4.0) / 2.0, rect.center().y), egui::Vec2::splat(MENU_ICON));
    icon.tint(egui::Color32::WHITE.gamma_multiply(if enabled { 1.0 } else { 0.35 })).paint_at(ui, icon_rect);
    if expanded {
        let colour = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
        ui.painter().text(
            egui::pos2(rect.left() + MENU_ITEM_HEIGHT + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(14.0),
            colour,
        );
    }
    if tooltip {
        response.clone().on_hover_text(label);
    }
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
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
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
// Every labelled field in the editor is one row of two columns: the label at
// the left in a column of fixed width, and the control starting right where
// that column ends, at one fixed width. Radio options take the label column
// and put what they choose in the control column, so a group of them lines up
// the same way a column of plain fields does.

fn form_row<R>(ui: &mut egui::Ui, label: impl FnOnce(&mut egui::Ui), control: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let height = ui.spacing().interact_size.y;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.allocate_ui_with_layout(egui::vec2(LABEL_WIDTH, height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(LABEL_WIDTH);
            ui.set_max_width(LABEL_WIDTH);
            label(ui);
        });
        ui.allocate_ui_with_layout(egui::vec2(CONTROL_WIDTH, height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(CONTROL_WIDTH);
            ui.set_max_width(CONTROL_WIDTH);
            control(ui)
        })
        .inner
    })
    .inner
}

/// [`form_row`] for a row whose label and control both use the same value,
/// which two closures capturing it could not: it is handed to each in turn.
fn form_row_with<S, R>(ui: &mut egui::Ui, state: &mut S, label: impl FnOnce(&mut egui::Ui, &mut S), control: impl FnOnce(&mut egui::Ui, &mut S) -> R) -> R {
    row_with(ui, state, Some(CONTROL_WIDTH), label, control)
}

/// A form row whose control is wider than the usual column - several controls
/// side by side - and takes the rest of the row.
fn form_row_wide_with<S, R>(ui: &mut egui::Ui, state: &mut S, label: impl FnOnce(&mut egui::Ui, &mut S), control: impl FnOnce(&mut egui::Ui, &mut S) -> R) -> R {
    row_with(ui, state, None, label, control)
}

fn row_with<S, R>(ui: &mut egui::Ui, state: &mut S, control_width: Option<f32>, label: impl FnOnce(&mut egui::Ui, &mut S), control: impl FnOnce(&mut egui::Ui, &mut S) -> R) -> R {
    let height = ui.spacing().interact_size.y;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.allocate_ui_with_layout(egui::vec2(LABEL_WIDTH, height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(LABEL_WIDTH);
            ui.set_max_width(LABEL_WIDTH);
            label(ui, state);
        });
        match control_width {
            Some(width) => {
                ui.allocate_ui_with_layout(egui::vec2(width, height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(width);
                    ui.set_max_width(width);
                    control(ui, state)
                })
                .inner
            }
            None => ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| control(ui, state)).inner,
        }
    })
    .inner
}

fn labeled_row<R>(ui: &mut egui::Ui, label: String, help: Option<String>, control: impl FnOnce(&mut egui::Ui) -> R) -> R {
    form_row(
        ui,
        |ui| {
            let response = ui.add(egui::Label::new(label).truncate());
            if let Some(help) = help {
                response.on_hover_text(help);
            }
        },
        control,
    )
}

fn form_checkbox(ui: &mut egui::Ui, label: String, value: &mut bool) -> egui::Response {
    labeled_row(ui, label, None, |ui| ui.checkbox(value, ""))
}

/// A drop-down over `names`. Whatever the scenario already holds stays listed,
/// so a model that is unloaded for the moment does not blank the choice.
fn form_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: String, value: &mut String, names: &[String]) -> egui::Response {
    labeled_row(ui, label, None, |ui| inline_combo(ui, id, value, names, tr!("opt-none-selected"), CONTROL_WIDTH - 8.0))
}

/// An input/constant field in a labelled row.
fn form_value<T: FieldEditor>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: String,
    help: Option<String>,
    value: &mut FieldValue<T>,
    range: (f64, f64),
) -> egui::Response {
    labeled_row(ui, label, help, |ui| ValueField::new(id, value).range(range.0, range.1).show(ui))
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
fn inline_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, names: &[String], empty_label: String, width: f32) -> egui::Response {
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
    response
}

/// A drop-down that fills the grid cell it is drawn in. A cell left empty is
/// outlined in red when `required`.
fn cell_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, names: &[String], empty_label: String, required: bool) {
    let width = (ui.available_width() - 8.0).max(40.0);
    let response = inline_combo(ui, id, value, names, empty_label, width);
    if required && value.is_empty() {
        mark_invalid(ui, &response);
    }
}

/// A drop-down over a fixed list of choices that fills the grid cell it is drawn in.
fn cell_choice<T: Copy + PartialEq>(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut T, all: &[T], label: impl Fn(T) -> String) {
    let width = (ui.available_width() - 8.0).max(40.0);
    egui::ComboBox::from_id_salt(id).selected_text(label(*value)).width(width).truncate().show_ui(ui, |ui| {
        for option in all {
            ui.selectable_value(value, *option, label(*option));
        }
    });
}

/// A text box that fills its grid cell, outlined in red when `required` and empty.
fn cell_text(ui: &mut egui::Ui, value: &mut String, required: bool) {
    let response = ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
    if required && value.trim().is_empty() {
        mark_invalid(ui, &response);
    }
}

fn plain_rows(count: usize) -> Vec<GridRow> {
    vec![GridRow::Item; count]
}

/// A framed group of radio options, as the air exclusion and haulage use.
fn option_group(ui: &mut egui::Ui, title: String, rows: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new(title).strong());
        rows(ui);
    });
}

// ── Inputs ──

fn draw_inputs(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &mut BlockModelFields, block_models: &[OpenBlockModel], project: &UiProjectView) {
    // Hidden models too: an unloaded model keeps its fields, and a run reads its values back.
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
        let response = form_value(
            ui,
            "opt_default_density",
            tr!("opt-default-density"),
            Some(tr!("opt-default-density-help")),
            &mut scenario.default_density,
            (0.001, f64::MAX),
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
        form_checkbox(ui, tr!("opt-exclude-air"), &mut scenario.exclude_air);
        if !scenario.exclude_air {
            return;
        }
        form_row_with(
            ui,
            &mut *scenario,
            |ui, scenario| {
                ui.radio_value(&mut scenario.air_mode, AirMode::Topography, tr!("opt-air-use-topography"));
            },
            |ui, scenario| {
                ui.add_enabled_ui(scenario.air_mode == AirMode::Topography, |ui| {
                    let layers: Vec<String> = project
                        .triangulations
                        .iter()
                        .map(|entry| entry.name.clone())
                        .chain(project.point_clouds.iter().map(|entry| entry.name.clone()))
                        .collect();
                    inline_combo(
                        ui,
                        "opt_air_topography",
                        &mut scenario.air_topography,
                        &layers,
                        tr!("opt-none-selected"),
                        CONTROL_WIDTH - 8.0,
                    );
                });
            },
        );
        form_row_with(
            ui,
            &mut *scenario,
            |ui, scenario| {
                ui.radio_value(&mut scenario.air_mode, AirMode::Rocktype, tr!("opt-air-use-rocktype"));
            },
            |ui, scenario| {
                ui.add_enabled_ui(scenario.air_mode == AirMode::Rocktype, |ui| {
                    // Until the rock type field offers its values the value is typed.
                    if values.is_empty() {
                        ui.add(egui::TextEdit::singleline(&mut scenario.air_rocktype).desired_width(CONTROL_WIDTH - 8.0));
                    } else {
                        inline_combo(ui, "opt_air_rocktype", &mut scenario.air_rocktype, &values, tr!("opt-none-selected"), CONTROL_WIDTH - 8.0);
                    }
                });
            },
        );
    });
}

// ── Constants ──

fn draw_constants(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, selections: &mut ScenarioGridSelections, height: f32) {
    let rows: Vec<GridRow> = scenario
        .constants
        .iter()
        .map(|row| match row {
            ConstantsRow::Group { name, collapsed } => GridRow::Group {
                name: name.clone(),
                collapsed: *collapsed,
            },
            ConstantsRow::Constant(_) => GridRow::Item,
        })
        .collect();
    let names: Vec<String> = scenario.constants.iter().filter_map(ConstantsRow::constant).map(|constant| constant.name.clone()).collect();
    let columns = vec![
        GridColumn::new(tr!("opt-col-name"), 25.0),
        GridColumn::new(tr!("opt-col-type"), 15.0),
        GridColumn::new(tr!("opt-col-value"), 15.0),
        GridColumn::new(tr!("opt-col-description"), 45.0),
    ];
    let constants = &mut scenario.constants;
    let mut actions = DataGrid::new("opt_constants", columns)
        .buttons(GridButtons::default().reorder().groups())
        .fill_height(height - 8.0)
        .show(ui, &mut selections.constants, &rows, |ui, row, column| {
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
                        .width((ui.available_width() - 8.0).max(40.0))
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
                    let id = egui::Id::new(("opt_constant_value", row));
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
                                .width((width - 8.0).max(40.0))
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
    apply_constants_actions(&mut scenario.constants, &mut selections.constants, &mut actions);
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
                        ValueField::new(("opt_rocktype_cost", row), &mut cost.cost).range(0.0, f64::MAX).show(ui);
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
        form_row_with(
            ui,
            &mut *haulage,
            |ui, haulage| {
                ui.radio_value(&mut haulage.mode, HaulageMode::Value, tr!("opt-haulage-value"));
            },
            |ui, haulage| {
                ui.add_enabled_ui(haulage.mode == HaulageMode::Value, |ui| {
                    ValueField::new((id, "value"), &mut haulage.value).range(0.0, f64::MAX).show(ui);
                });
            },
        );
        form_row_with(
            ui,
            &mut *haulage,
            |ui, haulage| {
                ui.radio_value(&mut haulage.mode, HaulageMode::Field, tr!("opt-haulage-field"));
            },
            |ui, haulage| {
                ui.add_enabled_ui(haulage.mode == HaulageMode::Field, |ui| {
                    let response = inline_combo(ui, (id, "field"), &mut haulage.field, numeric_fields, tr!("opt-none-selected"), CONTROL_WIDTH - 8.0);
                    if haulage.mode == HaulageMode::Field && haulage.field.is_empty() {
                        mark_invalid(ui, &response);
                    }
                });
            },
        );
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
                        ValueField::new(("opt_method_cost", row), &mut method.processing_cost).range(0.0, f64::MAX).show(ui);
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
                            ValueField::new(("opt_recovery", index, row), &mut element.recovery).range(0.0, 100.0).show(ui);
                        }
                        _ => {
                            ValueField::new(("opt_element_cost", index, row), &mut element.cost).range(0.0, f64::MAX).show(ui);
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
        );
        let before = methods[index].ga_cost.clone();
        form_row_wide_with(
            ui,
            &mut (&mut *methods, &mut *same_for_all),
            |ui, _| {
                ui.label(tr!("opt-ga-costs"));
            },
            |ui, (methods, same_for_all)| {
                ui.allocate_ui(egui::vec2(CONTROL_WIDTH - 130.0, ui.spacing().interact_size.y), |ui| {
                    ValueField::new(("opt_ga", index), &mut methods[index].ga_cost).range(0.0, f64::MAX).show(ui);
                });
                if ui.checkbox(same_for_all, tr!("opt-ga-same-for-all")).changed() && **same_for_all {
                    // Turning it on makes every method follow this one.
                    let value = methods[index].ga_cost.clone();
                    for method in methods.iter_mut() {
                        method.ga_cost = value.clone();
                    }
                }
            },
        );
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
                            ValueField::new(("opt_price", row), &mut revenue.price).range(0.0, f64::MAX).show(ui);
                        }
                        _ => {
                            ValueField::new(("opt_selling_cost", row), &mut revenue.selling_cost).range(0.0, f64::MAX).show(ui);
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
        form_row_with(
            ui,
            &mut *slope,
            |ui, slope| {
                ui.radio_value(&mut slope.mode, SlopeMode::Single, tr!("opt-slope-default-angle"));
            },
            |ui, slope| {
                // The default angle is also the field's fallback.
                ui.add_enabled_ui(slope.mode != SlopeMode::Rosette, |ui| {
                    ValueField::new("opt_slope_angle", &mut slope.angle).range(1.0, 89.0).show(ui);
                });
            },
        );
        form_row(
            ui,
            |ui| {
                ui.radio_value(&mut slope.mode, SlopeMode::Rosette, tr!("opt-slope-use-rosettes"));
            },
            |ui| {
                ui.label(egui::RichText::new(tr!("opt-slope-rosettes-hint")).weak());
            },
        );
        form_row_with(
            ui,
            &mut *slope,
            |ui, slope| {
                ui.radio_value(&mut slope.mode, SlopeMode::Field, tr!("opt-slope-field"));
            },
            |ui, slope| {
                ui.add_enabled_ui(slope.mode == SlopeMode::Field, |ui| {
                    let response = inline_combo(ui, "opt_slope_field", &mut slope.field, &fields.numeric, tr!("opt-none-selected"), CONTROL_WIDTH - 8.0);
                    if slope.mode == SlopeMode::Field && slope.field.is_empty() {
                        mark_invalid(ui, &response);
                    }
                });
            },
        );
        if slope.mode == SlopeMode::Field {
            ui.label(egui::RichText::new(tr!("opt-slope-field-hint")).weak());
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
                            let response = entry.bearing.edit(ui, egui::Id::new(("opt_rosette_bearing", row)), ui.available_width(), (0.0, 360.0));
                            let same = bearings.iter().enumerate().any(|(other, value)| other != row && *value == bearings[row]);
                            if same {
                                mark_invalid(ui, &response);
                            }
                        } else {
                            ValueField::new(("opt_rosette_angle", row), &mut entry.angle).range(1.0, 89.0).show(ui);
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
        form_row(
            ui,
            |ui| {
                ui.label(tr!("opt-interpolation")).on_hover_text(tr!("opt-interpolation-hint"));
            },
            |ui| {
                egui::ComboBox::from_id_salt("opt_rosette_interpolation")
                    .selected_text(slope.interpolation.label())
                    .width(CONTROL_WIDTH - 8.0)
                    .show_ui(ui, |ui| {
                        for option in RosetteInterpolation::available(rows) {
                            ui.selectable_value(&mut slope.interpolation, option, option.label());
                        }
                    })
                    .response
                    .on_hover_text(tr!("opt-interpolation-hint"));
            },
        );
    });
}

// ── Outputs ──

fn draw_outputs(ui: &mut egui::Ui, scenario: &mut OptimizationScenario, fields: &BlockModelFields, commands: &mut Vec<UiCommand>) {
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
        form_row(
            ui,
            |ui| {
                ui.label(tr!("opt-select-mode"));
            },
            |ui| {
                ui.radio_value(&mut output.mode, ShellMode::Single, tr!("opt-mode-single"));
            },
        );
        form_row(
            ui,
            |_| {},
            |ui| {
                ui.radio_value(&mut output.mode, ShellMode::Multiple, tr!("opt-mode-multiple"));
            },
        );
        if output.mode == ShellMode::Multiple {
            option_group(ui, tr!("opt-factors-title"), |ui| {
                // Two ways to give the factors: a range, or a list typed out.
                form_row_with(
                    ui,
                    &mut output.factor_input,
                    |ui, input| {
                        ui.radio_value(input, FactorInput::Range, tr!("opt-factor-input-range"));
                    },
                    |ui, input| {
                        ui.radio_value(input, FactorInput::List, tr!("opt-factor-input-list"));
                    },
                );
                if output.factor_input == FactorInput::List {
                    // A plain string, read when run: the list is not worth constants.
                    let response = labeled_row(ui, tr!("opt-factor-list"), Some(tr!("opt-factor-list-hint")), |ui| {
                        ui.add(egui::TextEdit::singleline(&mut output.factor_list).desired_width(ui.available_width()))
                    });
                    if let Err(issue) = parse_factor_list(&output.factor_list) {
                        mark_invalid(ui, &response);
                        menu_note(ui, issue.message());
                    }
                    return;
                }
                // Plain numbers: a revenue factor range is not worth a constant.
                let from_changed = labeled_row(ui, tr!("opt-factor-from"), None, |ui| {
                    output.factor_from.edit(ui, egui::Id::new("opt_factor_from"), ui.available_width(), (0.0, f64::MAX))
                })
                .changed();
                let to_changed = labeled_row(ui, tr!("opt-factor-to"), None, |ui| {
                    output.factor_to.edit(ui, egui::Id::new("opt_factor_to"), ui.available_width(), (0.0, f64::MAX))
                })
                .changed();
                let (from, to) = (output.factor_from, output.factor_to);
                // Step and number of shells follow each other: change one and the
                // other is worked out from it between the two ends.
                let mut step = output.factor_step;
                if labeled_row(ui, tr!("opt-factor-step"), None, |ui| {
                    step.edit(ui, egui::Id::new("opt_factor_step"), ui.available_width(), (0.0001, f64::MAX))
                })
                .changed()
                {
                    output.factor_step = step;
                    if let Some(count) = shell_count_for_step(from, to, step) {
                        output.shell_count = count.max(2);
                        output.factor_step = shell_step_for_count(from, to, output.shell_count).unwrap_or(step);
                    }
                }
                let mut count = output.shell_count;
                if labeled_row(ui, tr!("opt-shell-count"), None, |ui| {
                    edit_count(ui, egui::Id::new("opt_shell_count"), &mut count, ui.available_width(), (2, 1000))
                })
                .changed()
                {
                    output.shell_count = count;
                    if let Some(step) = shell_step_for_count(from, to, count) {
                        output.factor_step = step;
                    }
                }
                if (from_changed || to_changed)
                    && let Some(step) = shell_step_for_count(from, to, output.shell_count)
                {
                    output.factor_step = step;
                }
                if to <= from {
                    menu_note(ui, tr!("opt-factor-range-invalid"));
                }
            });
            let can_pick = !scenario_block_model.is_empty() && !fields.all.is_empty();
            form_row_wide_with(
                ui,
                &mut *output,
                |ui, output| {
                    ui.checkbox(&mut output.use_directional_shells, tr!("opt-directional-shells"))
                        .on_hover_text(tr!("opt-directional-shells-hint"));
                },
                |ui, output| {
                    ui.add_enabled_ui(output.use_directional_shells, |ui| {
                        // Two stacks side by side, each a label over its control.
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                            ui.allocate_ui_with_layout(egui::vec2(STACK_WIDTH, 0.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.set_width(STACK_WIDTH);
                                ui.spacing_mut().item_spacing.y = 4.0;
                                ui.label(tr!("opt-start-point-label"));
                                // Picked shows the point's whole metres on hover; a click
                                // picks again, with the old point marked in the model.
                                let (label, hint) = match output.shell_start {
                                    Some((x, y)) => (tr!("opt-picked"), tr!("opt-start-point", x = format!("{x:.0}"), y = format!("{y:.0}"))),
                                    None => (tr!("opt-pick-start"), tr!("opt-pick-start-hint")),
                                };
                                let response = ui
                                    .add_enabled(can_pick, MenuButton::new(label).selected(output.shell_start.is_some()).min_width(STACK_WIDTH))
                                    .on_hover_text(hint);
                                if output.shell_start.is_none() {
                                    mark_invalid(ui, &response);
                                }
                                if response.clicked() {
                                    commands.push(UiCommand::BeginShellStartPick);
                                }
                            });
                            ui.allocate_ui_with_layout(egui::vec2(STACK_WIDTH, 0.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.set_width(STACK_WIDTH);
                                ui.spacing_mut().item_spacing.y = 4.0;
                                ui.label(tr!("opt-mining-direction"));
                                egui::ComboBox::from_id_salt("opt_shell_direction")
                                    .selected_text(output.shell_direction.label())
                                    .width(STACK_WIDTH)
                                    .show_ui(ui, |ui| {
                                        for direction in ShellDirection::ALL {
                                            ui.selectable_value(&mut output.shell_direction, direction, direction.label());
                                        }
                                    });
                            });
                        });
                    });
                },
            );
        }
    });

    option_group(ui, tr!("opt-group-files"), |ui| {
        // Reports: the box, and where they go. Ticking it with no folder yet asks
        // for one straight away.
        form_row_wide_with(
            ui,
            &mut (&mut *output, &mut *commands),
            |ui, (output, commands)| {
                if ui.checkbox(&mut output.create_reports, tr!("opt-create-reports")).changed() && output.create_reports && output.reports_folder.is_empty() {
                    commands.push(UiCommand::ChooseOptimizationReportsFolder);
                }
            },
            |ui, (output, commands)| {
                ui.add_enabled_ui(output.create_reports, |ui| {
                    let mut shown = output.reports_folder.clone();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut shown)
                            .hint_text(tr!("opt-reports-folder-hint"))
                            .interactive(false)
                            .desired_width(CONTROL_WIDTH - 90.0),
                    );
                    if output.create_reports && output.reports_folder.is_empty() {
                        mark_invalid(ui, &response);
                    }
                    // A browser has no folders to write into.
                    let browse = ui.add_enabled(!cfg!(target_arch = "wasm32"), MenuButton::new(tr!("opt-browse")));
                    if browse.clicked() {
                        commands.push(UiCommand::ChooseOptimizationReportsFolder);
                    }
                });
            },
        );

        // Shells: solid, surface, both or neither, and the layer they go in.
        form_row_wide_with(
            ui,
            &mut *output,
            |ui, _| {
                ui.label(tr!("opt-create-shells"));
            },
            |ui, output| {
                ui.checkbox(&mut output.shell_as_solid, tr!("opt-shell-solid"));
                ui.checkbox(&mut output.shell_as_surface, tr!("opt-shell-surface"));
                ui.add_enabled_ui(output.shell_as_solid || output.shell_as_surface, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut output.shell_layer_name)
                            .hint_text(tr!("opt-layer-name"))
                            .desired_width(180.0),
                    );
                });
            },
        );
    });

    option_group(ui, tr!("opt-group-write-back"), |ui| {
        // The shell value in the block model - its number or its revenue
        // factor - in an existing field or a new one.
        form_row_wide_with(
            ui,
            &mut *output,
            |ui, output| {
                ui.checkbox(&mut output.write_shell_field, tr!("opt-write-shell-field"))
                    .on_hover_text(tr!("opt-shell-field-value-hint"));
            },
            |ui, output| {
                ui.add_enabled_ui(output.write_shell_field, |ui| {
                    egui::ComboBox::from_id_salt("opt_shell_field_value")
                        .selected_text(output.shell_field_value.label())
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for value in ShellFieldValue::ALL {
                                ui.selectable_value(&mut output.shell_field_value, value, value.label());
                            }
                        })
                        .response
                        .on_hover_text(tr!("opt-shell-field-value-hint"));
                    let label = |mode: ShellFieldMode| match mode {
                        ShellFieldMode::Existing => tr!("opt-shell-field-existing"),
                        ShellFieldMode::New => tr!("opt-shell-field-new"),
                    };
                    egui::ComboBox::from_id_salt("opt_shell_field_mode")
                        .selected_text(label(output.shell_field_mode))
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for mode in [ShellFieldMode::Existing, ShellFieldMode::New] {
                                ui.selectable_value(&mut output.shell_field_mode, mode, label(mode));
                            }
                        });
                    // A field that exists already is overwritten, so a rerun
                    // updates the field its first run made.
                    let response = match output.shell_field_mode {
                        ShellFieldMode::Existing => inline_combo(ui, "opt_shell_field", &mut output.shell_field, &free, tr!("opt-none-selected"), 150.0),
                        ShellFieldMode::New => ui.add(
                            egui::TextEdit::singleline(&mut output.shell_new_field)
                                .hint_text(tr!("opt-shell-new-field-hint"))
                                .desired_width(150.0),
                        ),
                    };
                    if let Some(issue) = &shell_field_issue {
                        mark_invalid(ui, &response);
                        response.on_hover_text(issue);
                    }
                });
            },
        );
    });
}
