//! The schedule editor: one schedule of an optimization scenario, in the
//! scenario editor's frame - the same section menu at the right, the same
//! fixed-height body, the same form rows and grids (borrowed from
//! `optimization.rs`). Value cells resolve against the parent scenario's
//! constants. See `.claude/specs/scheduling/stage-1-ui.md`.

use super::optimization::{
    EDITOR_WIDTH, MENU_ITEM_HEIGHT, SIDE_GAP, body_height, cell_combo, cell_text, constants_menu_item, field_row, form_choice, form_combo, form_toggle, form_value, menu_item,
    model_fields, option_group, outline_fields, plain_rows, unit_per_tonne, unit_percent,
};
use crate::{
    i18n::tr,
    model::{
        block_model::OpenBlockModel,
        npv_schedule::{Capacity, EventTarget, NpvSchedule, NpvScheduleEvent, PeriodCase, ProcessLimit, PushbackChoice, SellLimit, Stockpile, element_names, method_names},
        optimization::{OptimizationScenario, ShellFieldMode, format_factor, round_factor},
    },
    ui::{
        state::{NpvScheduleDraft, NpvScheduleGridSelections, NpvScheduleTab, OptimizationState, UiCommand},
        widgets::{
            editable_grid::{DataGrid, GridAction, GridButtons, GridColumn, apply_flat},
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, menu_note},
            value_field::{self, ValueField, edit_count, mark_invalid},
        },
    },
};

/// A white line icon, tinted by `menu_item` (as the scenario editor's).
macro_rules! tinted_icon {
    ($name:literal) => {
        egui::Image::new(egui::include_image!(concat!("../../../res/ui/icons_dark/", $name)))
    };
}

fn section_icon(tab: NpvScheduleTab) -> egui::Image<'static> {
    match tab {
        NpvScheduleTab::Pushbacks => tinted_icon!("filter_shells.svg"),
        NpvScheduleTab::Periods => tinted_icon!("section_constraints.svg"),
        NpvScheduleTab::Capacities => tinted_icon!("section_processing_costs.svg"),
        NpvScheduleTab::Stockpiles => tinted_icon!("section_mining_costs.svg"),
        NpvScheduleTab::CostsEvents => tinted_icon!("section_revenues.svg"),
        NpvScheduleTab::Outputs => tinted_icon!("section_outputs.svg"),
    }
}

fn section_menu_width(ctx: &egui::Context) -> f32 {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let font = egui::FontId::proportional(14.0);
    let labels = NpvScheduleTab::ALL
        .into_iter()
        .map(NpvScheduleTab::label)
        .chain([tr!("opt-constants-open"), tr!("sched-save"), tr!("common-close")]);
    let longest = labels
        .map(|label| painter.layout_no_wrap(label, font.clone(), egui::Color32::WHITE).size().x)
        .fold(0.0, f32::max);
    MENU_ITEM_HEIGHT + 4.0 + longest + 16.0
}

/// Sections and the global constants' entry, then Save and Close at the
/// bottom: (save, close, open the constants).
fn draw_section_menu(ui: &mut egui::Ui, height: f32, width: f32, current: &mut NpvScheduleTab, can_save: bool) -> (bool, bool, bool) {
    let mut choice = (false, false, false);
    ui.allocate_ui_with_layout(egui::vec2(width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 2.0;
        let top = ui.cursor().top();
        for tab in NpvScheduleTab::ALL {
            if menu_item(ui, section_icon(tab), &tab.label(), width, *current == tab, true) {
                *current = tab;
            }
        }
        choice.2 = constants_menu_item(ui, width);
        let bottom = 2.0 * MENU_ITEM_HEIGHT + ui.spacing().item_spacing.y * 2.0;
        ui.add_space((top + height - ui.cursor().top() - bottom).max(4.0));
        choice.0 = menu_item(ui, tinted_icon!("save_project.svg"), &tr!("sched-save"), width, false, can_save);
        choice.1 = menu_item(ui, tinted_icon!("close_project.svg"), &tr!("common-close"), width, false, true);
    });
    choice
}

pub(crate) fn draw_schedule_editor(ui: &mut egui::Ui, state: &mut OptimizationState, block_models: &[OpenBlockModel], commands: &mut Vec<UiCommand>) {
    let Some(draft) = state.schedule_draft.as_mut() else {
        return;
    };
    let Some(parent) = state.scenarios.iter().find(|scenario| scenario.id == draft.parent).cloned() else {
        // The scenario went (deleted elsewhere): nothing left to edit it in.
        state.schedule_draft = None;
        state.list_open = true;
        return;
    };
    value_field::publish_constants(ui.ctx(), &parent.constants);
    let ctx = ui.ctx().clone();
    let mut open = true;
    let mut close_requested = false;
    let mut save = false;
    let title = format!(
        "{}: {} \u{203A} {}{}",
        tr!("sched-title"),
        parent.name,
        draft.schedule.name,
        if draft.dirty() { "*" } else { "" }
    );
    let body_height = body_height(&ctx);
    let side_width = section_menu_width(&ctx);
    DragableMenu::new("schedule_editor", title)
        .open(&mut open)
        .min_width(EDITOR_WIDTH + SIDE_GAP + side_width)
        .max_width(EDITOR_WIDTH + SIDE_GAP + side_width)
        .show(&ctx, |ui| {
            outline_fields(ui);
            let ready = draft.dirty() || draft.is_new;
            let NpvScheduleDraft { schedule, selections, tab, .. } = &mut *draft;
            let item_spacing = ui.spacing().item_spacing;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let content = ui.allocate_ui_with_layout(egui::vec2(EDITOR_WIDTH - 16.0, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = item_spacing;
                    ui.set_width(EDITOR_WIDTH - 16.0);
                    ui.allocate_ui_with_layout(egui::vec2(EDITOR_WIDTH - 16.0, MENU_ITEM_HEIGHT), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(tab.label()).size(17.0).strong());
                    });
                    egui::ScrollArea::vertical()
                        .max_height(body_height)
                        .min_scrolled_height(body_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| match *tab {
                            NpvScheduleTab::Pushbacks => draw_pushbacks(ui, schedule, &parent),
                            NpvScheduleTab::Periods => draw_periods(ui, schedule),
                            NpvScheduleTab::Capacities => draw_capacities(ui, schedule, &parent, selections),
                            NpvScheduleTab::Stockpiles => draw_stockpiles(ui, schedule, &parent, selections),
                            NpvScheduleTab::CostsEvents => draw_costs(ui, schedule, &parent, selections),
                            NpvScheduleTab::Outputs => draw_outputs(ui, schedule, &parent, block_models),
                        });
                });
                ui.add_space(SIDE_GAP);
                let (clicked_save, clicked_close, open_constants) = draw_section_menu(ui, content.response.rect.height(), side_width, tab, ready);
                save = clicked_save;
                close_requested |= clicked_close;
                if open_constants {
                    commands.push(UiCommand::OpenOptimizationConstants);
                }
            });
            if !draft.confirm_close && !value_field::picker_open(ui.ctx()) && menu::dialog_cancelled(ui.ctx()) {
                close_requested = true;
            }
        });
    if !open {
        close_requested = true;
    }
    if save {
        commands.push(UiCommand::SaveNpvSchedule {
            parent: draft.parent,
            schedule: Box::new(draft.schedule.clone()),
            then_close: false,
        });
    }
    if close_requested {
        if draft.dirty() {
            draft.confirm_close = true;
        } else {
            state.schedule_draft = None;
            state.list_open = true;
            return;
        }
    }
    if draft.confirm_close {
        draw_confirm_close(&ctx, state, commands);
    }
}

/// Save / Discard / Cancel for a schedule closed with unsaved changes.
fn draw_confirm_close(ctx: &egui::Context, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let Some(draft) = state.schedule_draft.as_mut() else {
        return;
    };
    let mut open = true;
    let mut choice = None;
    let dialog = DragableMenu::new("schedule_unsaved_dialog", tr!("opt-unsaved-title"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(380.0)
        .show(ctx, |ui| {
            menu_note(ui, tr!("sched-unsaved-message", name = draft.schedule.name.clone()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("sched-save")).primary()).clicked() {
                    choice = Some(0);
                }
                if ui.add(MenuButton::new(tr!("opt-discard")).danger()).clicked() {
                    choice = Some(1);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    choice = Some(2);
                }
            });
        });
    if let Some(dialog) = dialog {
        ctx.move_to_top(dialog.response.layer_id);
    }
    if !open {
        choice = Some(2);
    }
    match choice {
        Some(0) => {
            draft.confirm_close = false;
            commands.push(UiCommand::SaveNpvSchedule {
                parent: draft.parent,
                schedule: Box::new(draft.schedule.clone()),
                then_close: true,
            });
        }
        Some(1) => {
            state.schedule_draft = None;
            state.list_open = true;
        }
        Some(_) => draft.confirm_close = false,
        None => {}
    }
}

fn unit_tonnes() -> String {
    format!(" {}", tr!("sched-unit-tonnes"))
}

/// A plain whole number in a labelled row.
fn form_count(ui: &mut egui::Ui, id: &str, label: String, help: Option<String>, value: &mut u32, range: (u32, u32)) -> egui::Response {
    field_row(ui, label, help, |ui, width| edit_count(ui, egui::Id::unique(id), value, width, range))
}

// ── Pushbacks ──

fn draw_pushbacks(ui: &mut egui::Ui, schedule: &mut NpvSchedule, parent: &OptimizationScenario) {
    let factors = parent.output.factors().unwrap_or_default();
    option_group(ui, tr!("sched-group-pushbacks"), |ui| {
        form_choice(
            ui,
            "sched_pushbacks",
            tr!("sched-pushbacks"),
            Some(tr!("sched-pushbacks-hint")),
            &mut schedule.pushbacks,
            &PushbackChoice::ALL,
            PushbackChoice::label,
        );
        match schedule.pushbacks {
            PushbackChoice::EveryShell => {}
            PushbackChoice::Factors => {
                if factors.is_empty() {
                    menu_note(ui, tr!("sched-no-parent-factors"));
                }
                // The parent's factors, ticked to make pushbacks; any chosen
                // factor the parent no longer has stays listed, in red.
                let mut shown: Vec<f64> = factors.clone();
                for factor in &schedule.pushback_factors {
                    if !shown.iter().any(|known| round_factor(*known) == round_factor(*factor)) {
                        shown.push(*factor);
                    }
                }
                shown.sort_by(f64::total_cmp);
                field_row(ui, tr!("sched-pushback-factors"), Some(tr!("sched-pushback-factors-hint")), |ui, _| {
                    ui.horizontal_wrapped(|ui| {
                        for factor in shown {
                            let known = factors.iter().any(|known| round_factor(*known) == round_factor(factor));
                            let index = schedule.pushback_factors.iter().position(|chosen| round_factor(*chosen) == round_factor(factor));
                            let mut ticked = index.is_some();
                            let response = ui.checkbox(&mut ticked, format_factor(factor));
                            if !known {
                                mark_invalid(ui, &response);
                            }
                            if response.changed() {
                                match index {
                                    Some(index) => {
                                        schedule.pushback_factors.remove(index);
                                    }
                                    None => schedule.pushback_factors.push(factor),
                                }
                                schedule.pushback_factors.sort_by(f64::total_cmp);
                            }
                        }
                    });
                });
                if schedule.pushback_factors.is_empty() {
                    menu_note(ui, tr!("sched-issue-no-pushbacks"));
                }
            }
        }
        form_count(
            ui,
            "sched_lead",
            tr!("sched-lead-benches"),
            Some(tr!("sched-lead-benches-hint")),
            &mut schedule.lead_benches,
            (0, 1000),
        );
        menu_note(ui, tr!("sched-cases-note"));
    });
}

// ── Periods ──

fn draw_periods(ui: &mut egui::Ui, schedule: &mut NpvSchedule) {
    option_group(ui, tr!("sched-group-periods"), |ui| {
        menu_note(ui, tr!("sched-periods-note"));
        let response = form_count(
            ui,
            "sched_max_periods",
            tr!("sched-max-periods"),
            Some(tr!("sched-max-periods-hint")),
            &mut schedule.max_periods,
            (1, 10_000),
        );
        if schedule.max_periods == 0 {
            mark_invalid(ui, &response);
        }
        form_value(
            ui,
            "sched_discount_rate",
            tr!("sched-discount-rate"),
            None,
            &mut schedule.discount_rate,
            (0.0, 1000.0),
            unit_percent(),
        );
        menu_note(ui, tr!("sched-discount-note"));
    });
}

// ── Capacities ──

fn draw_capacities(ui: &mut egui::Ui, schedule: &mut NpvSchedule, parent: &OptimizationScenario, selections: &mut NpvScheduleGridSelections) {
    let methods = method_names(parent);
    let elements = element_names(parent);
    option_group(ui, tr!("sched-group-mining"), |ui| {
        form_capacity(
            ui,
            "sched_mining_capacity",
            tr!("sched-mining-capacity"),
            None,
            &mut schedule.mining_capacity,
            unit_tonnes(),
        );
    });

    let mut fill_all = false;
    option_group(ui, tr!("sched-group-processing"), |ui| {
        let columns = vec![GridColumn::new(tr!("sched-method"), 50.0), GridColumn::new(tr!("sched-col-max"), 50.0)];
        let rows = plain_rows(schedule.process_limits.len());
        let limits = &mut schedule.process_limits;
        let mut actions = DataGrid::new("sched_process_limits", columns)
            .buttons(GridButtons::default().fill_all())
            .show(ui, &mut selections.process, &rows, |ui, row, column| {
                let Some(limit) = limits.get_mut(row) else {
                    return;
                };
                if column == 0 {
                    cell_combo(ui, ("sched_process_method", row), &mut limit.method, &methods, tr!("opt-none-selected"), true);
                } else {
                    cell_capacity(ui, ("sched_process_max", row), &mut limit.max_tonnes, unit_tonnes());
                }
            });
        fill_all |= actions.contains(&GridAction::FillAll);
        apply_flat(&mut schedule.process_limits, &mut selections.process, &mut actions, ProcessLimit::default);
        form_capacity(
            ui,
            "sched_total_limit",
            tr!("sched-total-process-limit"),
            Some(tr!("sched-total-limit-hint")),
            &mut schedule.total_process_limit,
            unit_tonnes(),
        );
    });

    option_group(ui, tr!("sched-group-selling"), |ui| {
        let columns = vec![GridColumn::new(tr!("sched-element"), 50.0), GridColumn::new(tr!("sched-col-max-sold"), 50.0)];
        let rows = plain_rows(schedule.sell_limits.len());
        let limits = &mut schedule.sell_limits;
        let mut actions = DataGrid::new("sched_sell_limits", columns)
            .buttons(GridButtons::default().fill_all())
            .show(ui, &mut selections.selling, &rows, |ui, row, column| {
                let Some(limit) = limits.get_mut(row) else {
                    return;
                };
                if column == 0 {
                    cell_combo(ui, ("sched_sell_element", row), &mut limit.element, &elements, tr!("opt-none-selected"), true);
                } else {
                    let unit = parent
                        .revenues
                        .iter()
                        .find(|revenue| revenue.element == limit.element)
                        .map(|revenue| format!(" {}", revenue.sales_unit.label()))
                        .unwrap_or_default();
                    cell_capacity(ui, ("sched_sell_max", row), &mut limit.max_sold, unit);
                }
            });
        fill_all |= actions.contains(&GridAction::FillAll);
        apply_flat(&mut schedule.sell_limits, &mut selections.selling, &mut actions, SellLimit::default);
        menu_note(ui, tr!("sched-selling-note"));
    });
    // Add all, in either grid: every method and every sold element.
    if fill_all {
        schedule.add_all_limits(parent);
    }
    if schedule.unlimited() {
        menu_note(ui, tr!("sched-issue-no-limits"));
    }
}

// ── Stockpiles ──

fn draw_stockpiles(ui: &mut egui::Ui, schedule: &mut NpvSchedule, parent: &OptimizationScenario, selections: &mut NpvScheduleGridSelections) {
    let methods = method_names(parent);
    // Grades are of the main quality field, in its grade unit.
    let grade_unit = parent
        .revenues
        .iter()
        .find(|revenue| revenue.element == parent.quality_field)
        .map(|revenue| format!(" {}", revenue.grade_unit.label()))
        .unwrap_or_default();
    option_group(ui, tr!("sched-group-stockpiles"), |ui| {
        menu_note(ui, tr!("sched-stockpiles-note"));
        if parent.quality_field.is_empty() {
            menu_note(ui, tr!("sched-issue-no-quality"));
        }
        let columns = vec![
            GridColumn::new(tr!("sched-col-name"), 18.0),
            GridColumn::new(tr!("sched-col-feeds"), 22.0),
            GridColumn::new(tr!("sched-col-capacity"), 26.0),
            GridColumn::new(tr!("sched-col-min-grade"), 17.0),
            GridColumn::new(tr!("sched-col-rehandle"), 17.0),
        ];
        let rows = plain_rows(schedule.stockpiles.len());
        let names: Vec<String> = schedule.stockpiles.iter().map(|stockpile| stockpile.name.trim().to_lowercase()).collect();
        let stockpiles = &mut schedule.stockpiles;
        let mut actions = DataGrid::new("sched_stockpiles", columns)
            .buttons(GridButtons::default())
            .show(ui, &mut selections.stockpile, &rows, |ui, row, column| {
                let Some(stockpile) = stockpiles.get_mut(row) else {
                    return;
                };
                match column {
                    0 => {
                        let duplicate = names.iter().enumerate().any(|(other, name)| other != row && !name.is_empty() && *name == names[row]);
                        let response = ui.add(egui::TextEdit::singleline(&mut stockpile.name).desired_width(ui.available_width()));
                        if stockpile.name.trim().is_empty() || duplicate {
                            mark_invalid(ui, &response);
                        }
                    }
                    1 => cell_multi_combo(ui, ("sched_stockpile_feeds", row), &mut stockpile.feeds, &methods),
                    2 => cell_capacity(ui, ("sched_stockpile_capacity", row), &mut stockpile.capacity, unit_tonnes()),
                    3 => {
                        ValueField::new(("sched_stockpile_min_grade", row), &mut stockpile.min_grade)
                            .range(0.0, f64::MAX)
                            .suffix(grade_unit.clone())
                            .show(ui)
                            .hint(tr!("sched-stockpile-min-grade-hint", field = parent.quality_field.clone()));
                    }
                    _ => {
                        ValueField::new(("sched_stockpile_rehandle", row), &mut stockpile.rehandle_cost)
                            .range(0.0, f64::MAX)
                            .suffix(unit_per_tonne())
                            .show(ui);
                    }
                }
            });
        let count = schedule.stockpiles.len() + 1;
        apply_flat(&mut schedule.stockpiles, &mut selections.stockpile, &mut actions, || {
            Stockpile::named(tr!("sched-default-stockpile", number = count.to_string()))
        });
    });

    // The selected stockpile's initial balance: what is on it when the
    // schedule starts, at a grade of the main quality field.
    let Some(stockpile) = selections.stockpile.and_then(|row| schedule.stockpiles.get_mut(row)) else {
        return;
    };
    option_group(ui, tr!("sched-group-initial-balance", name = stockpile.name.clone()), |ui| {
        form_value(
            ui,
            "sched_stockpile_initial_tonnes",
            tr!("sched-initial-tonnes"),
            Some(tr!("sched-initial-balance-hint")),
            &mut stockpile.initial_tonnes,
            (0.0, f64::MAX),
            unit_tonnes(),
        );
        let label = if parent.quality_field.is_empty() {
            tr!("sched-col-grade")
        } else {
            tr!("sched-initial-grade", field = parent.quality_field.clone())
        };
        form_value(
            ui,
            "sched_stockpile_initial_grade",
            label,
            None,
            &mut stockpile.initial_grade,
            (0.0, f64::MAX),
            grade_unit.clone(),
        );
    });
}

/// A capacity in a labelled row: see [`capacity_controls`].
fn form_capacity(ui: &mut egui::Ui, id: &str, label: String, help: Option<String>, capacity: &mut Capacity, suffix: String) {
    field_row(ui, label, help, |ui, _| capacity_controls(ui, id, capacity, suffix));
}

/// A capacity filling its grid cell: see [`capacity_controls`].
fn cell_capacity(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, capacity: &mut Capacity, suffix: String) {
    capacity_controls(ui, id, capacity, suffix);
}

/// The amount, with a switch inside its box at the left: on, the amount is the
/// limit; off, there is none and the box is greyed. The amount stays while
/// off, so switching back on gives it back; a limit of 0 is red, since it lets
/// nothing through.
fn capacity_controls(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, capacity: &mut Capacity, suffix: String) {
    let Capacity { limited, amount } = capacity;
    let response = ValueField::new(id, amount).range(0.0, f64::MAX).suffix(suffix).switch(limited).show(ui);
    if !capacity.limited {
        let _ = response.disabled_hint(tr!("sched-no-limit-hint"));
    } else if capacity.amount.constant.is_none() && capacity.amount.value == 0.0 {
        mark_invalid(ui, &response);
        response.hint(tr!("sched-zero-limit-hint"));
    }
}

/// A drop-down of `names` with a tick each, filling its grid cell; closed, it
/// shows the ticked names. Red with none ticked, or one `names` lacks.
fn cell_multi_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, chosen: &mut Vec<String>, names: &[String]) {
    let width = ui.available_width().max(40.0);
    let text = if chosen.is_empty() { tr!("opt-none-selected") } else { chosen.join(", ") };
    let mut listed: Vec<String> = names.to_vec();
    listed.extend(chosen.iter().filter(|name| !names.contains(name)).cloned());
    let response = egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .width(width)
        .truncate()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            for name in &listed {
                let mut ticked = chosen.contains(name);
                if ui.checkbox(&mut ticked, name.as_str()).changed() {
                    if ticked {
                        chosen.push(name.clone());
                        // In the parent's order, whatever the order ticked.
                        chosen.sort_by_key(|name| listed.iter().position(|listed| listed == name));
                    } else {
                        chosen.retain(|other| other != name);
                    }
                }
            }
        })
        .response;
    let unknown = chosen.iter().any(|name| !names.contains(name));
    if chosen.is_empty() || unknown {
        mark_invalid(ui, &response);
        response.hint(if unknown { tr!("sched-feeds-unknown") } else { tr!("sched-feeds-none") });
    }
}

// ── Costs and events ──

fn draw_costs(ui: &mut egui::Ui, schedule: &mut NpvSchedule, parent: &OptimizationScenario, selections: &mut NpvScheduleGridSelections) {
    option_group(ui, tr!("sched-group-costs"), |ui| {
        form_value(
            ui,
            "sched_capex_initial",
            tr!("sched-capex-initial"),
            Some(tr!("sched-capex-initial-hint")),
            &mut schedule.capex_initial,
            (0.0, f64::MAX),
            String::new(),
        );
        form_value(
            ui,
            "sched_fixed_cost",
            tr!("sched-fixed-cost"),
            Some(tr!("sched-fixed-cost-hint")),
            &mut schedule.fixed_cost,
            (0.0, f64::MAX),
            String::new(),
        );
    });

    // Every target an event's change can apply to.
    let mut targets = vec![EventTarget::None, EventTarget::Mining, EventTarget::TotalProcess];
    targets.extend(method_names(parent).into_iter().map(EventTarget::Method));
    targets.extend(element_names(parent).into_iter().map(EventTarget::Selling));
    option_group(ui, tr!("sched-group-events"), |ui| {
        menu_note(ui, tr!("sched-events-note"));
        let columns = vec![
            GridColumn::new(tr!("sched-col-period"), 12.0),
            GridColumn::new(tr!("sched-col-description"), 28.0),
            GridColumn::new(tr!("sched-col-capex"), 18.0),
            GridColumn::new(tr!("sched-col-target"), 24.0),
            GridColumn::new(tr!("sched-col-change"), 18.0),
        ];
        let rows = plain_rows(schedule.events.len());
        let max_periods = schedule.max_periods;
        let events = &mut schedule.events;
        let mut actions = DataGrid::new("sched_events", columns)
            .buttons(GridButtons::default().reorder())
            .show(ui, &mut selections.event, &rows, |ui, row, column| {
                let Some(event) = events.get_mut(row) else {
                    return;
                };
                match column {
                    0 => {
                        let response = edit_count(ui, egui::Id::unique(("sched_event_period", row)), &mut event.period, ui.available_width(), (1, 10_000));
                        if event.period == 0 || event.period > max_periods {
                            mark_invalid(ui, &response);
                        }
                    }
                    1 => cell_text(ui, &mut event.description, false),
                    2 => {
                        ValueField::new(("sched_event_capex", row), &mut event.capex).range(0.0, f64::MAX).show(ui);
                    }
                    3 => {
                        let width = ui.available_width().max(40.0);
                        let known = targets.contains(&event.target);
                        let response = egui::ComboBox::from_id_salt(("sched_event_target", row))
                            .selected_text(event.target.label())
                            .width(width)
                            .truncate()
                            .show_ui(ui, |ui| {
                                for target in &targets {
                                    ui.selectable_value(&mut event.target, target.clone(), target.label());
                                }
                            })
                            .response;
                        if !known {
                            mark_invalid(ui, &response);
                        }
                    }
                    _ => {
                        // A change may lower a capacity: negatives allowed.
                        let enabled = event.target != EventTarget::None;
                        let unit = match &event.target {
                            EventTarget::Selling(element) => parent
                                .revenues
                                .iter()
                                .find(|revenue| revenue.element == *element)
                                .map(|revenue| format!(" {}", revenue.sales_unit.label()))
                                .unwrap_or_default(),
                            _ => unit_tonnes(),
                        };
                        ui.add_enabled_ui(enabled, |ui| {
                            ValueField::new(("sched_event_delta", row), &mut event.delta)
                                .range(f64::MIN, f64::MAX)
                                .suffix(unit)
                                .show(ui);
                        });
                    }
                }
            });
        apply_flat(&mut schedule.events, &mut selections.event, &mut actions, NpvScheduleEvent::default);
    });
}

// ── Outputs ──

fn draw_outputs(ui: &mut egui::Ui, schedule: &mut NpvSchedule, parent: &OptimizationScenario, block_models: &[OpenBlockModel]) {
    let fields = model_fields(block_models, &parent.block_model);
    let free: Vec<String> = {
        let used = parent.used_fields();
        fields.text.iter().filter(|name| !used.contains(name.as_str())).cloned().collect()
    };
    let issue = schedule.period_field_target(parent, &fields).err();
    option_group(ui, tr!("sched-group-write-back"), |ui| {
        form_toggle(
            ui,
            tr!("sched-write-period-field"),
            Some(tr!("sched-write-period-field-hint")),
            &mut schedule.write_period_field,
        );
        if schedule.write_period_field {
            form_choice(
                ui,
                "sched_period_case",
                tr!("sched-period-case"),
                None,
                &mut schedule.period_case,
                &PeriodCase::ALL,
                PeriodCase::label,
            );
            form_choice(
                ui,
                "sched_period_field_mode",
                tr!("opt-shell-field-target"),
                None,
                &mut schedule.period_field_mode,
                &[ShellFieldMode::Existing, ShellFieldMode::New],
                |mode| match mode {
                    ShellFieldMode::Existing => tr!("opt-shell-field-existing"),
                    ShellFieldMode::New => tr!("opt-shell-field-new"),
                },
            );
            let response = match schedule.period_field_mode {
                ShellFieldMode::Existing => form_combo(ui, "sched_period_field", tr!("sched-period-field-name"), &mut schedule.period_field, &free),
                ShellFieldMode::New => field_row(ui, tr!("sched-period-field-name"), None, |ui, width| {
                    ui.add(egui::TextEdit::singleline(&mut schedule.period_new_field).desired_width(width))
                }),
            };
            if let Some(issue) = &issue {
                mark_invalid(ui, &response);
                response.hint(issue);
            }
        }
        form_toggle(ui, tr!("sched-create-reports"), Some(tr!("sched-create-reports-hint")), &mut schedule.create_reports);
    });
    // What stops the schedule running, all in one place.
    let issues = schedule.issues(parent);
    if !issues.is_empty() {
        option_group(ui, tr!("sched-group-issues"), |ui| {
            for issue in issues {
                ui.label(egui::RichText::new(format!("\u{2022} {issue}")).color(ui.visuals().error_fg_color));
            }
        });
    }
}
