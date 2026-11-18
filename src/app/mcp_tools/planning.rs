//! The tools for pit optimisation and NPV scheduling: listing scenarios and
//! their schedules, running them, and reading what a run found.

use serde_json::{Value, json};

use super::{failed, merge_patch, ok};
use crate::{
    app::App,
    model::{
        npv_schedule::NpvSchedule,
        optimization::{ConstantsRow, OptimizationScenario},
        optimization_run::report::{Basis, OptimizationReport},
    },
    ui::state::{NpvScheduleStatus, ScenarioStatus, UiCommand},
};

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_optimization",
            "title": "Get pit optimisation",
            "description": "List the project's pit optimisation scenarios and the NPV schedules under each: ids, names, the block model each scenario runs on, and each one's status (never run, up to date, stale since a change, queued, or running with how far along; a schedule is blocked until its scenario has an up-to-date run). An up-to-date schedule carries its NPVs: best case, worst case and fixed lead, and the periods it takes. Give scenario to have that one's last result shell by shell (revenue factor, ore and waste tonnes, revenue, cost and cash flow, cumulative from the first shell); give schedule too to have that schedule's pushbacks.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario": { "type": "integer", "minimum": 0 },
                    "schedule": { "type": "integer", "minimum": 0, "description": "A schedule of scenario." },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "run_optimization",
            "title": "Run pit optimisation",
            "description": "Run a pit optimisation scenario, as its Run button does; it queues if others are running. Give schedule too to run that NPV schedule of it instead: a scenario without an up-to-date run is run first. If the inputs are not loaded, the user is asked in the window whether to load them. Runs in the background: get_tasks follows the run (and can wait for it), and get_optimization reads the result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario": { "type": "integer", "minimum": 0 },
                    "schedule": { "type": "integer", "minimum": 0 },
                },
                "required": ["scenario"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
        json!({
            "name": "get_scenario",
            "title": "Get a scenario's settings",
            "description": "Read one pit optimisation scenario's settings whole, as the project keeps them (block model fields, prices, costs, recoveries, slopes, shells and output), and the project's constants they may refer to. Give schedule to read that NPV schedule's settings instead. set_scenario takes these back.",
            "inputSchema": {
                "type": "object",
                "properties": { "scenario": { "type": "integer", "minimum": 0 }, "schedule": { "type": "integer", "minimum": 0 } },
                "required": ["scenario"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "set_scenario",
            "title": "Add or change a scenario or schedule",
            "description": "Change a pit optimisation scenario's settings, or make a new scenario (leave scenario out). With schedule, change that NPV schedule of the scenario instead, or make a new schedule (schedule null). settings is a JSON merge patch over the settings get_scenario shows: give only what changes; objects merge, arrays and values replace, null removes. A new one starts from the defaults the window's Add gives. Returns the ids.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario": { "type": "integer", "minimum": 0 },
                    "schedule": { "type": ["integer", "null"], "minimum": 0 },
                    "name": { "type": "string", "minLength": 1 },
                    "settings": { "type": "object" },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "manage_scenario",
            "title": "Delete, duplicate or derive scenarios and schedules",
            "description": "delete or duplicate a pit optimisation scenario, or (with schedule) one of its NPV schedules; move or copy a schedule to another scenario (to); make a new scenario from a run's shells at revenue factors (shells_at, scenario only); or make a schedule's pushbacks from shells at revenue factors (pushbacks_at, with schedule). Returns any new id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario": { "type": "integer", "minimum": 0 },
                    "schedule": { "type": "integer", "minimum": 0 },
                    "action": { "type": "string", "enum": ["delete", "duplicate", "move", "copy", "shells_at", "pushbacks_at"] },
                    "to": { "type": "integer", "minimum": 0, "description": "move and copy: the scenario the schedule goes to." },
                    "factors": { "type": "array", "items": { "type": "number" }, "description": "shells_at and pushbacks_at: revenue factors." },
                },
                "required": ["scenario", "action"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false },
        }),
        json!({
            "name": "set_constants",
            "title": "Set the optimisation constants",
            "description": "Replace the project's optimisation constants, the named values scenarios' settings can refer to, in the shape get_scenario shows them (rows of groups and constants).",
            "inputSchema": {
                "type": "object",
                "properties": { "constants": { "type": "array" } },
                "required": ["constants"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_planning_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        if !matches!(
            name,
            "get_optimization" | "run_optimization" | "get_scenario" | "set_scenario" | "manage_scenario" | "set_constants"
        ) {
            return None;
        }
        // The project's scenarios are read in the first time anything asks.
        self.ensure_optimization_scenarios_loaded();
        Some(match name {
            "get_optimization" => self.mcp_optimization(arguments),
            "run_optimization" => self.mcp_run_optimization(arguments),
            "get_scenario" => self.mcp_get_scenario(arguments).map_or_else(failed, |value| ok(&value)),
            "set_scenario" => self.mcp_set_scenario(arguments).map_or_else(failed, |value| ok(&value)),
            "manage_scenario" => self.mcp_manage_scenario(arguments).map_or_else(failed, |value| ok(&value)),
            _ => self.mcp_set_constants(arguments).map_or_else(failed, |value| ok(&value)),
        })
    }

    fn mcp_scenario(&self, id: u64) -> Result<&OptimizationScenario, String> {
        self.editor
            .optimization
            .scenarios
            .iter()
            .find(|scenario| scenario.id == id)
            .ok_or_else(|| format!("There is no scenario with id {id}. get_optimization lists them."))
    }

    fn mcp_schedule(&self, scenario: u64, id: u64) -> Result<&NpvSchedule, String> {
        self.mcp_scenario(scenario)?
            .schedules
            .iter()
            .find(|schedule| schedule.id == id)
            .ok_or_else(|| format!("Scenario {scenario} has no schedule with id {id}."))
    }

    fn mcp_get_scenario(&self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["scenario"].as_u64().ok_or("Give the scenario's id.")?;
        if let Some(schedule) = arguments["schedule"].as_u64() {
            let schedule = self.mcp_schedule(id, schedule)?;
            return Ok(json!({ "schedule": serde_json::to_value(schedule).unwrap_or_default() }));
        }
        let mut settings = serde_json::to_value(self.mcp_scenario(id)?).unwrap_or_default();
        // Schedules and the constants' copy are read and set on their own.
        if let Value::Object(settings) = &mut settings {
            settings.remove("schedules");
            settings.remove("constants");
        }
        Ok(json!({ "scenario": settings, "constants": serde_json::to_value(&self.editor.optimization.constants).unwrap_or_default() }))
    }

    fn mcp_set_scenario(&mut self, arguments: &Value) -> Result<Value, String> {
        let name = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()).map(ToOwned::to_owned);
        let patch = arguments.get("settings").cloned().unwrap_or(json!({}));
        let scenario_id = arguments["scenario"].as_u64();
        if arguments.get("schedule").is_some() {
            let parent = scenario_id.ok_or("A schedule belongs to a scenario: give scenario.")?;
            let schedule = match arguments["schedule"].as_u64() {
                Some(id) => self.mcp_schedule(parent, id)?.clone(),
                None => self.new_npv_schedule(parent).ok_or_else(|| format!("There is no scenario with id {parent}."))?,
            };
            let is_new = !self.mcp_scenario(parent)?.schedules.iter().any(|existing| existing.id == schedule.id);
            let mut value = serde_json::to_value(&schedule).map_err(|error| error.to_string())?;
            merge_patch(&mut value, &patch);
            let mut edited: NpvSchedule = serde_json::from_value(value).map_err(|error| format!("The settings do not fit a schedule: {error}"))?;
            edited.id = schedule.id;
            if is_new && let Some(name) = &name {
                edited.name.clone_from(name);
            }
            let id = edited.id;
            self.handle_reported_ui_command(UiCommand::SaveNpvSchedule {
                parent,
                schedule: Box::new(edited),
                then_close: false,
            })
            .map_err(|error| format!("{error:#}"))?;
            if !is_new && let Some(name) = name {
                self.handle_reported_ui_command(UiCommand::RenameNpvSchedule { parent, id, name })
                    .map_err(|error| format!("{error:#}"))?;
            }
            return Ok(json!({ "scenario": parent, "schedule": id }));
        }
        let scenario = match scenario_id {
            Some(id) => self.mcp_scenario(id)?.clone(),
            None => self.new_optimization_scenario(),
        };
        let is_new = scenario_id.is_none();
        let mut value = serde_json::to_value(&scenario).map_err(|error| error.to_string())?;
        merge_patch(&mut value, &patch);
        let mut edited: OptimizationScenario = serde_json::from_value(value).map_err(|error| format!("The settings do not fit a scenario: {error}"))?;
        edited.id = scenario.id;
        if is_new && let Some(name) = &name {
            edited.name.clone_from(name);
        }
        let id = edited.id;
        self.handle_reported_ui_command(UiCommand::SaveOptimizationScenario {
            scenario: Box::new(edited),
            then_close: false,
        })
        .map_err(|error| format!("{error:#}"))?;
        if !is_new && let Some(name) = name {
            self.handle_reported_ui_command(UiCommand::RenameOptimizationScenario { id, name })
                .map_err(|error| format!("{error:#}"))?;
        }
        Ok(json!({ "scenario": id }))
    }

    fn mcp_manage_scenario(&mut self, arguments: &Value) -> Result<Value, String> {
        let parent = arguments["scenario"].as_u64().ok_or("Give the scenario's id.")?;
        self.mcp_scenario(parent)?;
        let schedule = arguments["schedule"].as_u64();
        if let Some(id) = schedule {
            self.mcp_schedule(parent, id)?;
        }
        let factors = || -> Result<Vec<f64>, String> {
            let factors: Vec<f64> = arguments["factors"].as_array().ok_or("Give factors.")?.iter().filter_map(Value::as_f64).collect();
            if factors.is_empty() {
                Err("Give at least one revenue factor.".to_owned())
            } else {
                Ok(factors)
            }
        };
        let scenario_ids = |app: &Self| -> Vec<u64> { app.editor.optimization.scenarios.iter().map(|scenario| scenario.id).collect() };
        let schedule_ids = |app: &Self| -> Vec<u64> {
            app.editor
                .optimization
                .scenarios
                .iter()
                .flat_map(|scenario| scenario.schedules.iter().map(|schedule| schedule.id))
                .collect()
        };
        let (before_scenarios, before_schedules) = (scenario_ids(self), schedule_ids(self));
        let command = match (arguments["action"].as_str(), schedule) {
            (Some("delete"), None) => UiCommand::DeleteOptimizationScenario(parent),
            (Some("duplicate"), None) => UiCommand::DuplicateOptimizationScenario(parent),
            (Some("shells_at"), None) => UiCommand::CopyScenarioWithShells { id: parent, factors: factors()? },
            (Some("delete"), Some(id)) => UiCommand::DeleteNpvSchedule { parent, id },
            (Some("duplicate"), Some(id)) => UiCommand::DuplicateNpvSchedule { parent, id },
            (Some("pushbacks_at"), Some(id)) => UiCommand::NpvScheduleFromPushbacks { parent, id, factors: factors()? },
            (Some(action @ ("move" | "copy")), Some(id)) => {
                let to = arguments["to"].as_u64().ok_or("Give the scenario it goes to.")?;
                self.mcp_scenario(to)?;
                if action == "move" {
                    UiCommand::MoveNpvSchedule { id, from: parent, to }
                } else {
                    UiCommand::CopyNpvScheduleTo { id, from: parent, to }
                }
            }
            (Some("move" | "copy" | "pushbacks_at"), None) => return Err("That works on a schedule: give schedule.".to_owned()),
            (Some("shells_at"), Some(_)) => return Err("shells_at works on a scenario: leave schedule out.".to_owned()),
            _ => return Err("action is delete, duplicate, move, copy, shells_at or pushbacks_at.".to_owned()),
        };
        self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))?;
        let new_scenario = scenario_ids(self).into_iter().find(|id| !before_scenarios.contains(id));
        let new_schedule = schedule_ids(self).into_iter().find(|id| !before_schedules.contains(id));
        Ok(json!({ "done": arguments["action"], "new_scenario": new_scenario, "new_schedule": new_schedule }))
    }

    fn mcp_set_constants(&mut self, arguments: &Value) -> Result<Value, String> {
        let rows: Vec<ConstantsRow> =
            serde_json::from_value(arguments["constants"].clone()).map_err(|error| format!("The constants are not in the shape get_scenario shows: {error}"))?;
        let count = rows.len();
        self.handle_reported_ui_command(UiCommand::SaveOptimizationConstants { rows, then_close: false })
            .map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "rows": count }))
    }

    fn mcp_optimization(&self, arguments: &Value) -> Value {
        let state = &self.editor.optimization;
        let scenarios: Vec<Value> = state
            .scenarios
            .iter()
            .map(|scenario| {
                let (status, fraction) = scenario_status(state.status(scenario));
                let schedules: Vec<Value> = scenario
                    .schedules
                    .iter()
                    .map(|schedule| {
                        let (status, fraction) = schedule_status(state.schedule_status(scenario, schedule));
                        let summary = state.schedule_summary(scenario, schedule).map(|summary| {
                            json!({
                                "periods": summary.periods,
                                "npv_best": summary.npv_best,
                                "npv_worst": summary.npv_worst,
                                "npv_fixed_lead": summary.npv_lead,
                                "fixed_lead_npv_peaks_at_pushback": summary.peak_before_last,
                            })
                        });
                        json!({ "id": schedule.id, "name": schedule.name, "status": status, "fraction_done": fraction, "result": summary })
                    })
                    .collect();
                json!({
                    "id": scenario.id,
                    "name": scenario.name,
                    "block_model": scenario.block_model,
                    "status": status,
                    "fraction_done": fraction,
                    "has_result": state.results.contains_key(&scenario.id),
                    "schedules": schedules,
                })
            })
            .collect();
        let mut result = json!({ "scenarios": scenarios });
        if let Some(id) = arguments["scenario"].as_u64() {
            let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == id) else {
                return failed(format!("There is no scenario with id {id}."));
            };
            result["result"] = state.results.get(&id).map_or(Value::Null, |run| shells(&run.report));
            if let Some(schedule) = arguments["schedule"].as_u64() {
                if !scenario.schedules.iter().any(|candidate| candidate.id == schedule) {
                    return failed(format!("Scenario {id} has no schedule with id {schedule}."));
                }
                result["schedule_result"] = state.schedule_results.get(&schedule).map_or(Value::Null, |run| {
                    let pushbacks: Vec<Value> = run
                        .pits
                        .iter()
                        .enumerate()
                        .map(|(index, pit)| json!({ "pushback": index + 1, "outer_shell": pit.shell + 1, "ore_tonnes": pit.ore, "waste_tonnes": pit.waste }))
                        .collect();
                    json!({
                        "discount_rate": run.rate,
                        "initial_capex": run.capex_initial,
                        "lead_benches": run.lead,
                        "pushbacks_cumulative": pushbacks,
                    })
                });
            }
        }
        ok(&result)
    }

    fn mcp_run_optimization(&mut self, arguments: &Value) -> Value {
        let Some(id) = arguments["scenario"].as_u64() else {
            return failed("run_optimization needs a scenario's id.");
        };
        let state = &self.editor.optimization;
        let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == id) else {
            return failed(format!("There is no scenario with id {id}. get_optimization lists them."));
        };
        let command = match arguments["schedule"].as_u64() {
            None => UiCommand::RunOptimizationScenario(id),
            Some(schedule_id) => {
                let Some(schedule) = scenario.schedules.iter().find(|schedule| schedule.id == schedule_id) else {
                    return failed(format!("Scenario {id} has no schedule with id {schedule_id}."));
                };
                // The schedule's play button stops a run in progress: running
                // it again must not do that.
                if matches!(state.schedule_status(scenario, schedule), NpvScheduleStatus::Running(_) | NpvScheduleStatus::Queued) {
                    return ok(&json!({ "scenario": id, "schedule": schedule_id, "status": "already running or queued" }));
                }
                let issues = schedule.issues(scenario);
                if !issues.is_empty() {
                    return failed(format!("The schedule {} cannot run yet: {}", schedule.name, issues.join("; ")));
                }
                UiCommand::RunNpvSchedule { parent: id, id: schedule_id }
            }
        };
        let schedule_id = arguments["schedule"].as_u64();
        if let Err(error) = self.handle_reported_ui_command(command) {
            return failed(format!("{error:#}"));
        }
        let state = &self.editor.optimization;
        let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == id) else {
            return failed("The scenario has gone.");
        };
        let status = match schedule_id.and_then(|schedule| scenario.schedules.iter().find(|candidate| candidate.id == schedule)) {
            Some(schedule) => schedule_status(state.schedule_status(scenario, schedule)).0,
            None => scenario_status(state.status(scenario)).0,
        };
        let status = match status {
            "running" | "queued" => status,
            // Nothing started: the window is asking the user to load inputs.
            _ => "waiting for the user to agree, in the window, to load the inputs",
        };
        ok(&json!({ "scenario": id, "schedule": schedule_id, "status": status }))
    }
}

/// A scenario's status as the tools name it, and how far along a run is.
fn scenario_status(status: ScenarioStatus) -> (&'static str, Option<f32>) {
    match status {
        ScenarioStatus::NeverRun => ("never_run", None),
        ScenarioStatus::UpToDate => ("up_to_date", None),
        ScenarioStatus::Stale => ("stale", None),
        ScenarioStatus::Queued => ("queued", None),
        ScenarioStatus::Running(fraction) => ("running", Some(fraction)),
    }
}

/// A schedule's status as the tools name it, and how far along a run is.
fn schedule_status(status: NpvScheduleStatus) -> (&'static str, Option<f32>) {
    match status {
        NpvScheduleStatus::Blocked => ("blocked", None),
        NpvScheduleStatus::NeverRun => ("never_run", None),
        NpvScheduleStatus::UpToDate => ("up_to_date", None),
        NpvScheduleStatus::Stale => ("stale", None),
        NpvScheduleStatus::Queued => ("queued", None),
        NpvScheduleStatus::Running(fraction) => ("running", Some(fraction)),
    }
}

/// An optimisation result, shell by shell, cumulative.
fn shells(report: &OptimizationReport) -> Value {
    let rows: Vec<Value> = report
        .shells
        .iter()
        .enumerate()
        .map(|(index, shell)| {
            let bucket = report.bucket(Basis::Cumulative, index, None);
            let (ore, waste) = report.ore_waste(Basis::Cumulative, index);
            json!({
                "shell": shell.number,
                "revenue_factor": shell.factor,
                "ore_tonnes": ore,
                "waste_tonnes": waste,
                "revenue": bucket.revenue(),
                "cost": bucket.total_cost(),
                "cash_flow": bucket.cash_flow(),
            })
        })
        .collect();
    json!({
        "destinations": report.destinations,
        "elements": report.elements.iter().map(|element| element.name.clone()).collect::<Vec<_>>(),
        "shells": rows,
    })
}
