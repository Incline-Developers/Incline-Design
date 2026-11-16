//! The Scenarios island down the left of the Gantt, Charts and Calendar: the
//! project's schedules, the open one selected, and the open one's name, what
//! it starts from and its horizon. See [`crate::model::schedule::scenarios`].

use crate::{
    i18n::tr,
    model::schedule::{
        SchedulePlan,
        periods::PeriodUnit,
        scenarios::{ScenarioId, ScenarioRow},
    },
    ui::{
        EditorState,
        elements::planning_setup::PlanningLayout,
        state::{ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, PropertyTable, grid_add_action_row, grid_columns_row, stack_slot, table_stack},
            hint::Hint,
            island::{Island, Side},
        },
    },
};

/// How a scenario's horizon reads in the list: "52 weeks, from Scenario 1".
fn summary(row: &ScenarioRow, rows: &[ScenarioRow]) -> String {
    let scale = crate::model::schedule::periods::PeriodScale {
        unit: row.horizon.unit,
        start: crate::ui::elements::schedule_periods::clock_start(),
    };
    let count = scale.count(row.horizon.end_day).to_string();
    let horizon = match row.horizon.unit {
        PeriodUnit::Day => tr!("scenario-horizon-days", count = count),
        PeriodUnit::Week => tr!("scenario-horizon-weeks", count = count),
        PeriodUnit::Month => tr!("scenario-horizon-months", count = count),
    };
    match row.base.and_then(|base| rows.iter().find(|other| other.id == base)) {
        Some(base) => tr!("scenario-summary-from", horizon = horizon, base = base.name.clone()),
        None => horizon,
    }
}

/// The island, or the strip it collapses to. Returns its regions and seam.
pub(crate) fn draw_island(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) -> PlanningLayout {
    let mut layout = PlanningLayout::default();
    let island = Island::new("schedule_scenarios_island", Side::Left)
        .default_width(260.0)
        .min_width(220.0)
        .max_width(420.0)
        .flush();
    if editor.show_schedule_scenarios {
        let island = island.show(ui, |ui, rect| draw_panel(ui, rect, editor, plan, session, commands));
        layout.regions.extend(island.regions);
        layout.grips.push(island.grip);
    } else {
        let strip = island.show_collapsed(ui);
        layout.regions.push(strip.region);
        if strip.clicked {
            editor.show_schedule_scenarios = true;
        }
    }
    layout
}

fn draw_panel(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let rows = plan.scenario_rows();
    let mut edits = Vec::new();
    table_stack(ui, rect, "schedule_scenarios", |ui| {
        let mut close = false;
        let columns = [(tr!("scenario-name"), 0.6), (tr!("scenario-value"), 0.4)];
        DataGrid::new("schedule_scenario_list", stack_slot(ui), &tr!("scenario-list"))
            .title_close(&mut close)
            .columns(&columns)
            .show(ui, |ui| {
                for row in &rows {
                    let detail = summary(row, &rows);
                    let value = editor
                        .schedule_scenario_values
                        .iter()
                        .find(|(id, _)| *id == row.id)
                        .map_or_else(String::new, |(_, value)| crate::model::schedule::cashflow::money_text(*value));
                    let (response, _) = grid_columns_row(ui, &[0.6, 0.4], &[&row.name, &value], row.open);
                    let response = response.hint(if value.is_empty() { detail } else { tr!("scenario-value-hint", summary = detail) });
                    if response.clicked() && !row.open {
                        edits.push(UiCommand::schedule(session, ScheduleEdit::OpenScenario(row.id)));
                    }
                    if rows.len() > 1 {
                        context_menu_popup(&response, &row.name, |ui| {
                            if ContextMenuAction::new(tr!("scenario-delete")).show(ui).clicked() {
                                edits.push(UiCommand::schedule(session, ScheduleEdit::DeleteScenario(row.id)));
                                ui.close();
                            }
                        });
                    }
                }
                if grid_add_action_row(ui, &tr!("scenario-new")) {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::AddScenario { copy: false }));
                }
                if grid_add_action_row(ui, &tr!("scenario-copy")) {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::AddScenario { copy: true }));
                }
            });
        if close {
            editor.show_schedule_scenarios = false;
        }
        let Some(open) = rows.iter().find(|row| row.open) else { return };
        PropertyTable::new("schedule_scenario_open", stack_slot(ui), &open.name).show(ui, |table| {
            table.header(&tr!("planning-property"), &tr!("planning-value"));
            let (response, committed) = table.committed_entry(("schedule_scenario_name", open.id.0), &tr!("scenario-name"), &open.name, None);
            if let Some(name) = committed
                && name.trim() != open.name
            {
                edits.push(UiCommand::schedule(session, ScheduleEdit::RenameScenario { id: open.id, name }));
            }
            response.hint(tr!("scenario-name-help"));
            // What it may start from: none, or any scenario that does not
            // already start from it.
            let candidates: Vec<(Option<ScenarioId>, String)> = std::iter::once((None, tr!("scenario-base-none")))
                .chain(
                    rows.iter()
                        .filter(|other| other.id != open.id && !plan.scenarios().would_ring(open.id, other.id))
                        .map(|other| (Some(other.id), other.name.clone())),
                )
                .collect();
            let mut base = open.base;
            let current = candidates
                .iter()
                .find(|(id, _)| *id == base)
                .map_or_else(|| tr!("scenario-base-none"), |(_, name)| name.clone());
            if rows.len() > 1
                && table
                    .combo(("schedule_scenario_base", open.id.0), &tr!("scenario-base"), &mut base, &current, candidates)
                    .hint(tr!("scenario-base-help"))
                    .changed()
            {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetScenarioBase { id: open.id, base }));
            }
            let mut unit = plan.periods().unit;
            let unit_text = super::schedule_periods::unit_label(unit);
            if table
                .combo(
                    ("schedule_scenario_unit", open.id.0),
                    &tr!("periods-unit"),
                    &mut unit,
                    &unit_text,
                    PeriodUnit::ALL.map(|unit| (unit, super::schedule_periods::unit_label(unit))),
                )
                .changed()
            {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetPeriodUnit(unit)));
            }
            super::schedule_optimisation::horizon_row(table, editor, plan, session, &mut edits);
            super::schedule_optimisation::step_row(table, editor, plan, session, &mut edits);
            let inherited = plan.inherited_bars().len();
            table.readonly(&tr!("scenario-bars"), &open.bars.to_string(), None, None);
            if inherited > 0 {
                table
                    .readonly(&tr!("scenario-inherited-bars"), &inherited.to_string(), None, None)
                    .hint(tr!("scenario-inherited-bars-help"));
            }
            if rows.len() > 1 {
                let mut scenario_only = editor.schedule_cells_scenario_only;
                let scope = |only: bool| if only { tr!("scenario-cells-scope-open") } else { tr!("scenario-cells-scope-all") };
                let current = scope(scenario_only);
                table
                    .combo(
                        ("schedule_scenario_cells_scope", open.id.0),
                        &tr!("scenario-cells-scope"),
                        &mut scenario_only,
                        &current,
                        [false, true].map(|only| (only, scope(only))),
                    )
                    .hint(tr!("scenario-cells-scope-help"));
                editor.schedule_cells_scenario_only = scenario_only;
                let own = plan.scenarios().open_scenario().map_or(&[][..], |scenario| scenario.cells.as_slice());
                if !own.is_empty() {
                    table.readonly(&tr!("scenario-cells"), &own.len().to_string(), None, None).hint(tr!("scenario-cells-help"));
                    if table.action_row(&tr!("scenario-cells-clear")).clicked() {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::ResetScenarioCells(own.iter().map(|cell| cell.address()).collect()),
                        ));
                    }
                }
            }
        });
    });
    commands.append(&mut edits);
}
