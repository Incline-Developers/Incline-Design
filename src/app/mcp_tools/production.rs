//! The tools for the production schedule and the haul network: their setup,
//! read and written whole as the project file keeps them, running their
//! steps and the schedule itself, and the schedule's report.

use serde_json::{Value, json};

use super::{failed, merge_patch, ok};
use crate::{
    app::App,
    model::{
        Command,
        haulage::HaulNetwork,
        schedule::{AutoRun, SchedulePlan, ScheduleResult},
    },
    ui::{
        elements::schedule_report::ReportGrouping,
        state::{HaulageStep, ScheduleStep, UiCommand},
    },
};

/// The schedule's setup steps, as the tools name them.
const SCHEDULE_STEPS: [(&str, ScheduleStep); 15] = [
    ("configuration", ScheduleStep::Configuration),
    ("periods", ScheduleStep::Periods),
    ("loader_classes", ScheduleStep::LoaderClasses),
    ("loader_agents", ScheduleStep::LoaderAgents),
    ("delays", ScheduleStep::Delays),
    ("drill_blast", ScheduleStep::DrillBlast),
    ("stockpiles", ScheduleStep::Stockpiles),
    ("dumps", ScheduleStep::Dumps),
    ("crushers", ScheduleStep::Crushers),
    ("destinations", ScheduleStep::Destinations),
    ("trucking_rules", ScheduleStep::TruckingRules),
    ("cashflow", ScheduleStep::Cashflow),
    ("solids", ScheduleStep::Solids),
    ("haulage", ScheduleStep::Haulage),
    ("readiness", ScheduleStep::Readiness),
];

/// One list of the schedule setup an item at a time: where its items and its
/// next id sit in the setup's JSON, and how one is deleted, through the plan's
/// own removal so what refers to it is dealt with as the window deals with it.
struct ScheduleList {
    name: &'static str,
    items: &'static str,
    next_id: &'static str,
    delete: fn(&mut SchedulePlan, u64) -> ScheduleResult,
}

const SCHEDULE_LISTS: [ScheduleList; 8] = [
    ScheduleList {
        name: "loader_class",
        items: "/classes",
        next_id: "/next_class_id",
        delete: |plan, id| plan.remove_class(crate::model::schedule::LoaderClassId(id)),
    },
    ScheduleList {
        name: "loader_agent",
        items: "/agents",
        next_id: "/next_agent_id",
        delete: |plan, id| plan.remove_agent(crate::model::schedule::LoaderAgentId(id)),
    },
    ScheduleList {
        name: "bar",
        items: "/bars",
        next_id: "/next_bar_id",
        delete: |plan, id| plan.remove_bar(crate::model::schedule::BarId(id)),
    },
    ScheduleList {
        name: "destination",
        items: "/routing/standalone",
        next_id: "/routing/next_standalone_id",
        delete: |plan, id| {
            let id = crate::model::schedule::StandaloneDestinationId(id);
            plan.routing_mut().remove_standalone(id)?;
            plan.remove_destination_grade_targets(crate::model::schedule::DestinationId::Standalone(id));
            Ok(())
        },
    },
    ScheduleList {
        name: "routing_rule",
        items: "/routing/rules",
        next_id: "/routing/next_rule_id",
        delete: |plan, id| plan.routing_mut().remove_rule(crate::model::schedule::RuleId(id)),
    },
    ScheduleList {
        name: "truck_class",
        items: "/trucks/classes",
        next_id: "/trucks/next_class_id",
        delete: |plan, id| plan.trucks_mut().remove_class(crate::model::schedule::TruckClassId(id)),
    },
    ScheduleList {
        name: "trucking_rule",
        items: "/trucks/rules",
        next_id: "/trucks/next_rule_id",
        delete: |plan, id| plan.trucks_mut().remove_rule(crate::model::schedule::TruckingRuleId(id)),
    },
    ScheduleList {
        name: "cashflow_rule",
        items: "/cashflow/rules",
        next_id: "/cashflow/next_rule_id",
        delete: |plan, id| plan.cashflow_mut().remove_rule(crate::model::schedule::CashflowRuleId(id)),
    },
];

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "idempotentHint": read_only },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    let steps: Vec<&str> = SCHEDULE_STEPS.iter().map(|(name, _)| *name).collect();
    vec![
        tool(
            "get_schedule_setup",
            "Get the production schedule setup",
            "Read the production schedule's whole setup as the project keeps it: loader classes and agents, the bars on the calendar (what each loader digs or reclaims, and when), periods, delays, drill and blast, stockpiles, dumps, crushers, destinations, trucking rules, cash flow and grade targets. Also the haul network (roads, nodes and truck classes) with network, and whether each pipeline runs itself after an edit. set_schedule_setup takes these back.",
            json!({
                "network": { "type": "boolean", "description": "Include the haul network. False if left out." },
                "examples": { "type": "boolean", "description": "Also give a small example setup in the same shape (a loader class and agent, a dig bar and a reclaim bar, a crusher, stockpile and dump, an ore and a waste routing rule, a truck class and trucking rule, and cash flow rules: a cost on every tonne and a value on ore delivered to the crusher; with no value to earn, loaders stand idle) and choices, the values each enumerated setting may take by its JSON pointer: what a new setup's lists hold and how they refer to each other. False if left out." },
            }),
            &[],
            true,
        ),
        tool(
            "set_schedule_setup",
            "Change the production schedule setup",
            "Change the production schedule's setup, the haul network, or both, as JSON merge patches over what get_schedule_setup shows: give only what changes; objects merge, arrays and values replace, null removes. Each is checked as a project file's would be, and is one undo step. auto_run says which pipelines rerun by themselves after an edit.",
            json!({
                "schedule": { "type": "object" },
                "network": { "type": "object" },
                "auto_run": {
                    "type": "object",
                    "properties": { "solids": { "type": "boolean" }, "haulage": { "type": "boolean" }, "schedule": { "type": "boolean" } },
                },
            }),
            &[],
            false,
        ),
        tool(
            "set_bar_dig_blocks",
            "Set the dig blocks a bar digs",
            "Set the ground a dig bar digs, in order, as picking dig blocks into it on the Gantt does: by dig block ids (get_reserves by block lists them, in the order given) or by a pit solid, optionally narrowed to benches by their base RL (top bench first, each flitch top down). Leave bar out to make a new dig bar for agent first, over window (from hour 0, open-ended, if left out). replace (the default) puts these in place of what the bar dug; append adds them after. The ids belong to the planning steps' last run: run_planning again and they must be read again. One undo step. Returns the bar's id, its block count and its tonnes.",
            json!({
                "bar": { "type": "integer", "minimum": 0, "description": "The dig bar to set. A new one if left out." },
                "name": { "type": "string", "description": "A new bar's name. Its work's own label if left out." },
                "agent": { "type": "integer", "minimum": 0, "description": "A new bar's loader agent." },
                "priority": { "type": "integer", "minimum": 0, "description": "A new bar's lane: lower is worked first. The loader's next free lane if left out." },
                "window": {
                    "type": "object",
                    "properties": { "start_h": { "type": "number", "minimum": 0 }, "end_h": { "type": "number" } },
                    "required": ["start_h"],
                    "additionalProperties": false,
                    "description": "A new bar's window, in hours from the schedule's start; no end_h is open-ended.",
                },
                "blocks": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "Dig block ids, in the order they are dug." },
                "solid": { "type": "integer", "minimum": 0, "description": "In place of blocks: every dig block of this solid." },
                "benches": { "type": "array", "items": { "type": "number" }, "description": "With solid: only the benches with these base RLs." },
                "mode": { "type": "string", "enum": ["replace", "append"], "description": "replace if left out." },
            }),
            &[],
            false,
        ),
        tool(
            "edit_schedule_item",
            "Add, change or delete one schedule setup item",
            "Add, change or delete one item of a production schedule list without sending the list whole: a loader class or agent, a bar, a destination (crusher, stockpile or dump), a routing rule, a truck class, a trucking rule or a cash flow rule. add starts from the matching item of get_schedule_setup's examples, puts each of item's fields in place of the example's (so give only what differs, such as a name and kind) and allocates the id; position puts a rule that far up its list (rules are tried in order), the end if left out. update merges item into the item with that id. delete removes it as the window does: an agent's bars become unassigned, and a destination a routing rule still delivers to is refused. Checked as a project file's would be, and one undo step. Returns the item as it now stands. Dig bars' blocks are set with set_bar_dig_blocks.",
            json!({
                "list": { "type": "string", "enum": SCHEDULE_LISTS.iter().map(|list| list.name).collect::<Vec<_>>() },
                "action": { "type": "string", "enum": ["add", "update", "delete"] },
                "id": { "type": "integer", "minimum": 0, "description": "update and delete: the item's id." },
                "item": { "type": "object", "description": "add: the new item's fields; update: a JSON merge patch over the item." },
                "position": { "type": "integer", "minimum": 0, "description": "add: where in the list it goes, 0 first. The end if left out." },
            }),
            &["list", "action"],
            false,
        ),
        tool(
            "add_haul_road",
            "Add a haul road",
            "Add one road to the haul network through points, as drawing it in Setup's Road Network step does. Each end joins the node already within the network's join tolerance of it (and stays on it, draped or not), or becomes a new node; the points between are the road's shape. drape lays it on a surface (a design or the topography) instead of the RLs given, with a point every drape_spacing metres. at_end makes its last end where trucks dump at or reclaim from a destination. One undo step. Returns the roads' ids (one, or one per piece with node_every_m), its nodes, its length and its steepest grade (rise over run), to hold against the trucks' maximum_grade. get_schedule_status's haulage step says which dig blocks still reach no road.",
            json!({
                "points": { "type": "array", "minItems": 2, "items": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }, "description": "[x, y, z] along the road, first end to last." },
                "name": { "type": "string" },
                "drape": { "type": "integer", "minimum": 0, "description": "A surface to lay the road on. It must be loaded." },
                "drape_spacing": { "type": "number", "exclusiveMinimum": 0, "description": "With drape: the most metres between points. 10 if left out." },
                "speed_limit_kph": { "type": "number", "exclusiveMinimum": 0 },
                "node_every_m": { "type": "number", "exclusiveMinimum": 0, "description": "Split the road into pieces about this long, with a node at each join. Dig blocks join the network at nodes, so a ramp past several benches needs them along it. Nodes at its ends only if left out." },
                "at_end": {
                    "type": "object",
                    "properties": {
                        "destination": { "type": "integer", "minimum": 0, "description": "A destination's id (get_schedule_setup's routing standalone)." },
                        "role": { "type": "string", "enum": ["dump", "reclaim", "dump_and_reclaim"], "description": "dump if left out." },
                    },
                    "required": ["destination"],
                    "additionalProperties": false,
                },
            }),
            &["points"],
            false,
        ),
        tool(
            "get_schedule_status",
            "Get the schedule's readiness",
            "Read where the production schedule stands: each Setup step's state (not_run, stale, blocked, queued, running, complete, failed, cancelled) with its message and problems, each bar's tonnes and problems, and what stops the schedule being calculated, if anything. run_schedule check runs the steps first.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "run_schedule",
            "Run the production schedule",
            "Run the production schedule, as the Schedule workspace's buttons do: check (every setup step, running the Solids and Haulage pipelines when they are not current), a single setup step, period (calculate the next period), all (calculate every period, to the planning end day: schedule.experiment.planning_end_day), improve (search for a better schedule than the one calculated), or cancel. haulage runs the haul network's steps (network, truck_classes, or both when left out). Runs in the background: get_tasks follows it, and get_schedule_report reads the result.",
            json!({
                "what": { "type": "string", "enum": ["check", "step", "period", "all", "improve", "cancel", "haulage"] },
                "step": { "type": "string", "enum": steps, "description": "what step: the setup step." },
                "haulage_step": { "type": "string", "enum": ["network", "truck_classes"], "description": "what haulage: one step. Both if left out." },
            }),
            &["what"],
            false,
        ),
        tool(
            "export_schedule_video",
            "Record the schedule as a video",
            "Record the calculated production schedule's animation to a video file, as Animate's export does: stretches of the schedule (hours from the start) played back to back at hours_per_second, at fps frames a second and width by height pixels. The view is placed as render_view places one (frame, box, view, azimuth, elevation, perspective, section) on the first frame and held to the end; with none given it frames everything Animate draws, and view window follows the user's window instead. Shows the Animate page first. Runs in the background.",
            video_schema(json!({
                "path": { "type": "string", "description": "An absolute path; its extension picks the format." },
                "segments": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "description": "[start, end] hours. The whole schedule if left out." },
                "hours_per_second": { "type": "number", "exclusiveMinimum": 0 },
                "fps": { "type": "number", "exclusiveMinimum": 0, "description": "30 if left out." },
                "width": { "type": "integer", "minimum": 16, "description": "1920 if left out." },
                "height": { "type": "integer", "minimum": 16, "description": "1080 if left out." },
                "destination_labels": { "type": "boolean" },
                "machine_labels": { "type": "boolean" },
            })),
            &["path", "hours_per_second"],
            false,
        ),
        tool(
            "get_schedule_report",
            "Get the schedule report",
            "Read the calculated production schedule as the report export writes it: tables of tonnes dug and reclaimed per loader, delivered and stockpiled per destination with grades, haulage, loader time and trucks, by day, by week, or over the whole schedule. Each table comes as its columns and rows (numbers as numbers, a blank as null), or the whole report as the export's CSV text. Also how well the solver did.",
            json!({
                "by": { "type": "string", "enum": ["day", "week", "whole"], "description": "day if left out." },
                "sections": { "type": "array", "items": { "type": "string" }, "description": "Only these tables, by title (Movements, Haulage, Loader time, Stockpiles, Crushers, Dumps, Trucks and any others the report has). All if left out." },
                "format": { "type": "string", "enum": ["tables", "csv"], "description": "tables if left out." },
            }),
            &[],
            true,
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_production_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "get_schedule_setup" => self.mcp_schedule_setup(arguments),
            "set_bar_dig_blocks" => self.mcp_set_bar_dig_blocks(arguments),
            "edit_schedule_item" => self.mcp_edit_schedule_item(arguments),
            "add_haul_road" => self.mcp_add_haul_road(arguments),
            "get_schedule_status" => self.mcp_schedule_status(),
            "set_schedule_setup" => self.mcp_set_schedule_setup(arguments),
            "run_schedule" => self.mcp_run_schedule(arguments),
            "get_schedule_report" => self.mcp_schedule_report(arguments),
            "export_schedule_video" => self.mcp_export_video(arguments),
            _ => return None,
        };
        Some(result.map_or_else(failed, |value| ok(&value)))
    }

    fn mcp_schedule_setup(&self, arguments: &Value) -> Result<Value, String> {
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let plan = document.schedule();
        let auto = plan.auto_run();
        let mut result = json!({
            "schedule": serde_json::to_value(plan).map_err(|error| error.to_string())?,
            "auto_run": { "solids": auto.solids, "haulage": auto.haulage, "schedule": auto.schedule },
        });
        if arguments["network"].as_bool().unwrap_or(false) {
            result["network"] = serde_json::to_value(document.haulage()).map_err(|error| error.to_string())?;
        }
        if arguments["examples"].as_bool().unwrap_or(false) {
            let example = serde_json::to_value(example_plan(document).map_err(|error| super::plain(&error.message()))?).map_err(|error| error.to_string())?;
            result["choices"] = super::enum_choices::<SchedulePlan>(&example);
            let network = serde_json::to_value(example_network().map_err(|error| format!("{error:#}"))?).map_err(|error| error.to_string())?;
            result["examples"] = json!({
                "note": "An example only, not this project's: a setup in this shape is set with set_schedule_setup. Ids are numbers each list allocates from its next_*_id, which goes up with every one added. A dig bar's members are filled with set_bar_dig_blocks, never by hand. A field condition names a reserve field by its id (get_solids lists them). Grades are carried through stockpiles only when listed in experiment.grades, so a rule that tests a grade on reclaimed material needs it there. Every haul needs a truck class a trucking rule allows, and a road route: roads join nodes, a node with a role is where trucks dump at (or reclaim from) a destination, and dig blocks within the settings' auto_join_m of a road join it by themselves. A road runs from its from node to its to node through its verts, the points between them (none for a straight road), and its grade must suit the trucks' maximum_grade.",
                "schedule": example,
                "network": network,
            });
        }
        Ok(result)
    }

    fn mcp_set_schedule_setup(&mut self, arguments: &Value) -> Result<Value, String> {
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let mut changed = Vec::new();
        if let Some(patch) = arguments.get("schedule") {
            let before = document.schedule().clone();
            let mut value = serde_json::to_value(&before).map_err(|error| error.to_string())?;
            merge_patch(&mut value, patch);
            let mut after: SchedulePlan = serde_json::from_value(value).map_err(|error| format!("The schedule setup does not fit: {error}"))?;
            after.validate_loaded().map_err(|error| super::plain(&error.message()))?;
            if after != before {
                self.execute_job_edit(Command::SetSchedulePlan {
                    before: Box::new(before),
                    after: Box::new(after),
                });
                changed.push("schedule");
            }
        }
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        if let Some(patch) = arguments.get("network") {
            let before = document.haulage().clone();
            let mut value = serde_json::to_value(&before).map_err(|error| error.to_string())?;
            merge_patch(&mut value, patch);
            let mut after: HaulNetwork = serde_json::from_value(value).map_err(|error| format!("The haul network does not fit: {error}"))?;
            after.validate().map_err(|error| format!("{error:#}"))?;
            if after != before {
                self.execute_job_edit(Command::SetHaulNetwork {
                    before: Box::new(before),
                    after: Box::new(after),
                });
                self.editor.haul_route = None;
                self.refresh_haulage_view();
                changed.push("network");
            }
        }
        if let Some(auto) = arguments["auto_run"].as_object() {
            let document = self.workspace.active_document().ok_or("No project is open.")?;
            let current = document.schedule().auto_run();
            let flag = |key: &str, now: bool| auto.get(key).and_then(Value::as_bool).unwrap_or(now);
            let wanted = AutoRun {
                solids: flag("solids", current.solids),
                haulage: flag("haulage", current.haulage),
                schedule: flag("schedule", current.schedule),
            };
            self.handle_reported_ui_command(UiCommand::SetAutoRun(wanted)).map_err(|error| format!("{error:#}"))?;
            changed.push("auto_run");
        }
        self.invalidate_geometry();
        Ok(json!({ "changed": changed }))
    }

    fn mcp_set_bar_dig_blocks(&mut self, arguments: &Value) -> Result<Value, String> {
        use crate::model::schedule::{BarId, DigBlockPick, LoaderAgentId, WorkWindow};

        let snapshot = self
            .planning_snapshot()
            .map_err(|not_ready| format!("The dig blocks are not ready: {}", super::plain(&not_ready.describe())))?;
        // The blocks picked, in the order they are dug.
        let picked: Vec<crate::model::DigBlockId> = match (arguments["blocks"].as_array(), arguments["solid"].as_u64()) {
            (Some(ids), None) => ids
                .iter()
                .map(|id| {
                    let id = id.as_u64().ok_or("blocks are dig block ids.")?;
                    snapshot
                        .blocks
                        .iter()
                        .find(|block| block.id.0 == id)
                        .map(|block| block.id)
                        .ok_or_else(|| format!("There is no dig block {id} in the planning steps' last run: get_reserves by block lists them."))
                })
                .collect::<Result<_, String>>()?,
            (None, Some(solid)) => {
                let benches: Option<Vec<f64>> = arguments["benches"].as_array().map(|benches| benches.iter().filter_map(Value::as_f64).collect());
                let mut blocks: Vec<_> = snapshot
                    .blocks
                    .iter()
                    .filter(|block| block.solid.0 == solid)
                    .filter(|block| benches.as_ref().is_none_or(|benches| benches.iter().any(|base| (base - block.bench.base).abs() < 1e-6)))
                    .collect();
                if blocks.is_empty() {
                    return Err(format!(
                        "Solid {solid} has no dig blocks{}: get_reserves by block lists them.",
                        if benches.is_some() { " on those benches" } else { "" }
                    ));
                }
                // Top down: the highest bench first, each flitch top down,
                // and within a flitch the blocks by their number.
                let number = |name: &str| (name.trim().parse::<u64>().unwrap_or(u64::MAX), name.to_owned());
                blocks.sort_by(|a, b| {
                    b.bench
                        .base
                        .total_cmp(&a.bench.base)
                        .then(b.flitch.base.total_cmp(&a.flitch.base))
                        .then_with(|| number(&a.name).cmp(&number(&b.name)))
                });
                blocks.into_iter().map(|block| block.id).collect()
            }
            (Some(_), Some(_)) => return Err("Give blocks or solid, not both.".to_owned()),
            (None, None) => return Err("Give blocks (dig block ids) or solid.".to_owned()),
        };
        let references = picked
            .iter()
            .map(|block| {
                self.add_dig_block(&DigBlockPick {
                    block: *block,
                    generation: snapshot.generation,
                })
                .map_err(|reason| super::plain(&reason))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let before = document.schedule().clone();
        let mut after = before.clone();
        let refused = |error: crate::model::schedule::ScheduleError| super::plain(&error.message());
        let bar = match arguments["bar"].as_u64() {
            Some(id) => {
                let bar = after.bar(BarId(id)).ok_or_else(|| format!("There is no bar {id}: get_schedule_setup lists them."))?;
                if bar.dig_order().is_none() {
                    return Err(format!("Bar {id} does not dig: it reclaims, waits or works blasts."));
                }
                BarId(id)
            }
            None => {
                let agent = arguments["agent"].as_u64().map(LoaderAgentId);
                if let Some(agent) = agent
                    && !after.agents().iter().any(|existing| existing.id == agent)
                {
                    return Err(format!("There is no loader agent {}: get_schedule_setup lists them.", agent.0));
                }
                let window = match arguments.get("window") {
                    None | Some(Value::Null) => WorkWindow::default(),
                    Some(window) => serde_json::from_value(window.clone()).map_err(|error| format!("window is {{start_h, end_h}}: {error}"))?,
                };
                let priority = match arguments["priority"].as_u64() {
                    Some(priority) => u32::try_from(priority).map_err(|_| "priority is too large.")?,
                    None => after
                        .bars()
                        .iter()
                        .filter(|bar| agent.is_some() && bar.agent == agent && bar.window.overlaps(window))
                        .map(|bar| bar.priority.saturating_add(1))
                        .max()
                        .unwrap_or(0),
                };
                after.add_bar(arguments["name"].as_str().unwrap_or_default(), agent, priority, window).map_err(refused)?
            }
        };
        if arguments["mode"].as_str().unwrap_or("replace") == "replace" {
            while after.bar(bar).is_some_and(|bar| !bar.members().is_empty()) {
                after.remove_bar_member(bar, 0).map_err(refused)?;
            }
        }
        for reference in references {
            let end = after.bar(bar).map_or(0, |bar| bar.members().len());
            after.insert_bar_member(bar, end, reference).map_err(refused)?;
        }
        let members = after.bar(bar).map_or(0, |bar| bar.members().len());
        if after != before {
            self.execute_job_edit(Command::SetSchedulePlan {
                before: Box::new(before),
                after: Box::new(after),
            });
            self.invalidate_geometry();
        }
        // What the bar now digs, measured as the Gantt measures it.
        let tonnes = self.bar_report(bar).and_then(|report| report.tonnes);
        Ok(json!({ "bar": bar.0, "dig_blocks": members, "tonnes": tonnes }))
    }

    fn mcp_edit_schedule_item(&mut self, arguments: &Value) -> Result<Value, String> {
        let name = arguments["list"].as_str().ok_or("Give the list.")?;
        let list = SCHEDULE_LISTS
            .iter()
            .find(|list| list.name == name)
            .ok_or_else(|| format!("list is one of {}.", SCHEDULE_LISTS.iter().map(|list| list.name).collect::<Vec<_>>().join(", ")))?;
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let before = document.schedule().clone();
        let id = arguments["id"].as_u64();
        let (after, id) = match arguments["action"].as_str() {
            Some("delete") => {
                let id = id.ok_or("Give the id of the item to delete.")?;
                let mut after = before.clone();
                (list.delete)(&mut after, id).map_err(|error| super::plain(&error.message()))?;
                (after, id)
            }
            Some(action @ ("add" | "update")) => {
                let mut value = serde_json::to_value(&before).map_err(|error| error.to_string())?;
                let item = arguments.get("item").cloned().unwrap_or_else(|| json!({}));
                let id = if action == "add" {
                    let example = serde_json::to_value(example_plan(document).map_err(|error| super::plain(&error.message()))?).map_err(|error| error.to_string())?;
                    let mut new = example.pointer(list.items).and_then(|items| items.get(0)).cloned().unwrap_or_else(|| json!({}));
                    // Each field given replaces the example's whole, rather
                    // than merging into it: a bar's work or a rule's sources
                    // is one choice of several, and merging two choices
                    // would make a value that is neither.
                    let fields = item.as_object().ok_or("item is an object of the new item's fields.")?;
                    for (key, field) in fields {
                        new[key] = field.clone();
                    }
                    let id = value.pointer(list.next_id).and_then(Value::as_u64).unwrap_or(0);
                    new["id"] = json!(id);
                    if let Some(next) = value.pointer_mut(list.next_id) {
                        *next = json!(id + 1);
                    }
                    let items = value.pointer_mut(list.items).and_then(Value::as_array_mut).ok_or("The list is not in the setup.")?;
                    let position = arguments["position"]
                        .as_u64()
                        .map_or(items.len(), |position| usize::try_from(position).unwrap_or(usize::MAX).min(items.len()));
                    items.insert(position, new);
                    id
                } else {
                    let id = id.ok_or("Give the id of the item to change.")?;
                    let items = value.pointer_mut(list.items).and_then(Value::as_array_mut).ok_or("The list is not in the setup.")?;
                    let existing = items
                        .iter_mut()
                        .find(|existing| existing["id"].as_u64() == Some(id))
                        .ok_or_else(|| format!("There is no {} with id {id}: get_schedule_setup lists them.", list.name))?;
                    merge_patch(existing, &item);
                    existing["id"] = json!(id);
                    id
                };
                let mut after: SchedulePlan = serde_json::from_value(value).map_err(|error| format!("The {} does not fit: {error}", list.name))?;
                after.validate_loaded().map_err(|error| super::plain(&error.message()))?;
                (after, id)
            }
            _ => return Err("action is add, update or delete.".to_owned()),
        };
        let item = serde_json::to_value(&after).ok().and_then(|value| {
            value
                .pointer(list.items)
                .and_then(Value::as_array)
                .and_then(|items| items.iter().find(|item| item["id"].as_u64() == Some(id)).cloned())
        });
        if after != before {
            self.execute_job_edit(Command::SetSchedulePlan {
                before: Box::new(before),
                after: Box::new(after),
            });
            self.invalidate_geometry();
        }
        Ok(json!({ "list": list.name, "id": id, "item": item }))
    }

    fn mcp_add_haul_road(&mut self, arguments: &Value) -> Result<Value, String> {
        use glam::DVec3;

        use crate::model::{
            haulage::network::NodeRole,
            schedule::destinations::{DestinationId, StandaloneDestinationId},
        };

        let mut points: Vec<DVec3> = arguments["points"]
            .as_array()
            .ok_or("Give the road's points.")?
            .iter()
            .map(|point| super::parse_point(point).ok_or_else(|| "Each point is [x, y, z].".to_owned()))
            .collect::<Result<_, _>>()?;
        if points.len() < 2 {
            return Err("A road needs at least two points.".to_owned());
        }
        // An end given on an existing node joins it there, wherever draping
        // would put it: the road meets the network where it was drawn to.
        let (joined_first, joined_last) = {
            let document = self.workspace.active_document().ok_or("No project is open.")?;
            let network = document.haulage();
            let tolerance = network.settings.join_tolerance_m;
            let joined = |at: DVec3| {
                network
                    .nodes
                    .iter()
                    .filter(|node| node.pos.distance(at) <= tolerance)
                    .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
                    .map(|node| node.pos)
            };
            (joined(points[0]), joined(points[points.len() - 1]))
        };
        if let Some(surface) = arguments["drape"].as_u64() {
            let surface = self
                .triangulations
                .iter()
                .find(|candidate| candidate.id.0 == surface)
                .ok_or_else(|| format!("There is no surface with id {surface}. get_project lists them."))?;
            if !surface.state.loaded || surface.state.deferred.is_some() {
                return Err(format!("The surface {} is not loaded: set_loaded loads it.", surface.name));
            }
            let spacing = arguments["drape_spacing"].as_f64().filter(|spacing| *spacing > 0.0).unwrap_or(10.0);
            let mut draped = Vec::new();
            for pair in points.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                let run = from.truncate().distance(to.truncate());
                let steps = ((run / spacing).ceil() as usize).max(1);
                for step in 0..steps {
                    draped.push(from.lerp(to, step as f64 / steps as f64));
                }
            }
            draped.push(points[points.len() - 1]);
            for point in &mut draped {
                point.z = crate::app::commands::drawing::drape::uppermost_topology_z(*point, &[surface])
                    .ok_or_else(|| format!("The road leaves the surface {} at ({:.1}, {:.1}).", surface.name, point.x, point.y))?;
            }
            points = draped;
        }
        if let Some(node) = joined_first {
            points[0] = node;
        }
        if let Some(node) = joined_last {
            let end = points.len() - 1;
            points[end] = node;
        }
        let role = match arguments.get("at_end") {
            None | Some(Value::Null) => None,
            Some(end) => {
                let destination = DestinationId::Standalone(StandaloneDestinationId(end["destination"].as_u64().ok_or("at_end needs the destination's id.")?));
                let document = self.workspace.active_document().ok_or("No project is open.")?;
                if !document
                    .schedule()
                    .routing()
                    .standalone
                    .iter()
                    .any(|existing| DestinationId::Standalone(existing.id) == destination)
                {
                    return Err("There is no destination with that id: get_schedule_setup lists them under routing standalone.".to_owned());
                }
                Some(match end["role"].as_str() {
                    None | Some("dump") => NodeRole::Dump(destination),
                    Some("reclaim") => NodeRole::Reclaim(destination),
                    Some("dump_and_reclaim") => NodeRole::DumpAndReclaim(destination),
                    Some(_) => return Err("role is dump, reclaim or dump_and_reclaim.".to_owned()),
                })
            }
        };
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let before = document.haulage().clone();
        let mut after = before.clone();
        let tolerance = after.settings.join_tolerance_m;
        let end_node = |network: &mut HaulNetwork, at: DVec3| -> Result<crate::model::haulage::NodeId, String> {
            match network
                .nodes
                .iter()
                .filter(|node| node.pos.distance(at) <= tolerance)
                .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
            {
                Some(node) => Ok(node.id),
                None => network.add_node(at).map_err(|error| format!("{error:#}")),
            }
        };
        // The road's pieces: the whole of it, or pieces about node_every_m
        // long, cut at points put in at those distances along it.
        let pieces: Vec<Vec<DVec3>> = match arguments["node_every_m"].as_f64().filter(|every| *every > 0.0) {
            None => vec![points.clone()],
            Some(every) => {
                let mut pieces = vec![vec![points[0]]];
                let mut along = 0.0;
                for pair in points.windows(2) {
                    let (from, to) = (pair[0], pair[1]);
                    let length = from.distance(to);
                    let mut done = 0.0;
                    while along + (length - done) >= every {
                        done += every - along;
                        let cut = from.lerp(to, done / length);
                        if let Some(piece) = pieces.last_mut() {
                            piece.push(cut);
                        }
                        pieces.push(vec![cut]);
                        along = 0.0;
                    }
                    along += length - done;
                    if let Some(piece) = pieces.last_mut() {
                        piece.push(to);
                    }
                }
                // A last piece too short to stand alone joins the one before.
                if pieces.len() > 1
                    && pieces
                        .last()
                        .is_some_and(|piece| piece.windows(2).map(|pair| pair[0].distance(pair[1])).sum::<f64>() < every * 0.25)
                    && let Some(tail) = pieces.pop()
                    && let Some(piece) = pieces.last_mut()
                {
                    piece.pop();
                    piece.extend(tail);
                }
                pieces
            }
        };
        let name = arguments["name"].as_str().unwrap_or_default().to_owned();
        let mut roads = Vec::new();
        let mut nodes = Vec::new();
        for (index, piece) in pieces.iter().enumerate() {
            let from = end_node(&mut after, piece[0])?;
            let to = end_node(&mut after, piece[piece.len() - 1])?;
            if from == to {
                return Err("The road's two ends join the same node.".to_owned());
            }
            let piece_name = if pieces.len() > 1 {
                format!("{name} {}", index + 1).trim().to_owned()
            } else {
                name.clone()
            };
            let road = after
                .add_road(piece_name, from, to, piece[1..piece.len() - 1].to_vec())
                .map_err(|error| format!("{error:#}"))?;
            if let Some(limit) = arguments["speed_limit_kph"].as_f64()
                && let Some(entry) = after.roads.iter_mut().find(|entry| entry.id == road)
            {
                entry.speed_limit_kph = Some(limit);
            }
            if nodes.is_empty() {
                nodes.push(from);
            }
            nodes.push(to);
            roads.push(road);
        }
        let (from, to) = (nodes[0], nodes[nodes.len() - 1]);
        let (first, last) = (points[0], points[points.len() - 1]);
        if let Some(role) = role
            && let Some(node) = after.nodes.iter_mut().find(|node| node.id == to)
        {
            node.role = Some(role);
        }
        after.validate().map_err(|error| format!("{error:#}"))?;
        // Measured on the road as it was stored: its ends at its nodes.
        let shape: Vec<DVec3> = std::iter::once(after.nodes.iter().find(|node| node.id == from).map_or(first, |node| node.pos))
            .chain(points[1..points.len() - 1].iter().copied())
            .chain(std::iter::once(after.nodes.iter().find(|node| node.id == to).map_or(last, |node| node.pos)))
            .collect();
        let length: f64 = shape.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        let steepest = shape
            .windows(2)
            .filter_map(|pair| {
                let run = pair[0].truncate().distance(pair[1].truncate());
                (run > 1e-9).then(|| (pair[1].z - pair[0].z).abs() / run)
            })
            .fold(0.0, f64::max);
        self.execute_job_edit(Command::SetHaulNetwork {
            before: Box::new(before),
            after: Box::new(after),
        });
        self.editor.haul_route = None;
        self.refresh_haulage_view();
        self.invalidate_geometry();
        Ok(json!({
            "roads": roads.iter().map(|road| road.0).collect::<Vec<_>>(),
            "nodes": nodes.iter().map(|node| node.0).collect::<Vec<_>>(),
            "length_m": length,
            "steepest_grade": steepest,
        }))
    }

    fn mcp_schedule_status(&mut self) -> Result<Value, String> {
        use crate::app::planning_pipeline::StageState;

        self.workspace.active_document().ok_or("No project is open.")?;
        self.mirror_schedule_stages();
        self.mirror_schedule_calculation();
        let state = |state: StageState| match state {
            StageState::NotRun => "not_run",
            StageState::Stale => "stale",
            StageState::Blocked => "blocked",
            StageState::Queued => "queued",
            StageState::Running => "running",
            StageState::Complete => "complete",
            StageState::Failed => "failed",
            StageState::Cancelled => "cancelled",
        };
        let steps: Vec<Value> = SCHEDULE_STEPS
            .iter()
            .map(|(name, step)| {
                let view = &self.editor.schedule_stages[step.index()];
                let problems: Vec<Value> = view
                    .diagnostics
                    .iter()
                    .map(|diagnostic| json!({ "problem": super::plain(&diagnostic.line()), "blocking": diagnostic.blocking }))
                    .collect();
                json!({
                    "step": name,
                    "state": state(view.state),
                    "message": view.message.as_deref().map(super::plain),
                    "problems": problems,
                })
            })
            .collect();
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let bars: Vec<Value> = document
            .schedule()
            .bars()
            .iter()
            .map(|bar| {
                let report = self.bar_report(bar.id);
                // The problems name a block by its number within its flitch,
                // which the Gantt shows beside its row; here the row is
                // spelled out for each member that has one.
                let members: Vec<Value> = report
                    .as_ref()
                    .map(|report| {
                        report
                            .members
                            .iter()
                            .filter(|member| !member.excluded && (member.unresolved.is_some() || member.tonnes.is_none()))
                            .map(|member| {
                                json!({
                                    "position": member.position,
                                    "dig_block": member.resolved.map(|block| block.0),
                                    "solid": member.solid_name,
                                    "bench": member.bench,
                                    "flitch": member.flitch,
                                    "block": member.name,
                                    "unresolved": member.unresolved.as_deref().map(super::plain),
                                    "tonnes": member.tonnes,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                json!({
                    "bar": bar.id.0,
                    "name": bar.name(),
                    "dig_blocks": bar.members().len(),
                    "tonnes": report.as_ref().and_then(|report| report.tonnes),
                    "problems": report.as_ref().map(|report| report.problems.iter().map(|problem| super::plain(&problem.message())).collect::<Vec<_>>()).unwrap_or_default(),
                    "members_with_problems": members,
                })
            })
            .collect();
        Ok(json!({
            "steps": steps,
            "bars": bars,
            "calculation_blocked": self.editor.schedule_calculation_blocked.as_deref().map(super::plain),
            "running": self.editor.schedule_run_active,
            "calculated": self.editor.schedule_result.is_some(),
        }))
    }

    fn mcp_run_schedule(&mut self, arguments: &Value) -> Result<Value, String> {
        let commands = match arguments["what"].as_str() {
            Some("check") => vec![UiCommand::RunAllScheduleStages],
            Some("step") => {
                let name = arguments["step"].as_str().ok_or("Give the step.")?;
                let step = SCHEDULE_STEPS
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, step)| *step)
                    .ok_or_else(|| format!("{name} is not a schedule step."))?;
                vec![UiCommand::RunScheduleStage(step)]
            }
            Some(what @ ("period" | "all" | "improve")) => {
                use crate::app::{schedule_pipeline::ScheduleNotReady, schedule_run::ScheduleRunMode};
                let mode = match what {
                    "period" => ScheduleRunMode::Period,
                    "all" => ScheduleRunMode::All,
                    _ => ScheduleRunMode::Improve,
                };
                self.mcp_schedule_serial_asked = self.schedule_run_serial;
                // The Setup steps are checked first, as the Gantt's buttons
                // do. Where they are still running (the Solids run a project
                // starts as it opens, say), the button would be refused; here
                // the calculation waits for them instead.
                self.run_all_schedule_steps();
                match self.schedule_run_inputs() {
                    Ok(_) => {}
                    Err(ScheduleNotReady::Running(_)) => {
                        // Answered once it has run: the held call waits on
                        // this and then on the calculation's own task.
                        self.mcp_pending_schedule_run = Some(mode);
                        return Ok(json!({ "started": what, "after": "the Setup steps still running" }));
                    }
                    Err(reason) => {
                        return Err(format!(
                            "The schedule cannot be calculated yet: {} (get_schedule_status says more).",
                            super::plain(&reason.describe())
                        ));
                    }
                }
                self.start_schedule_run(mode);
                return Ok(json!({ "started": what }));
            }
            Some("cancel") => vec![
                UiCommand::CancelScheduleCalculation,
                UiCommand::CancelScheduleRun,
                UiCommand::CancelHaulageRun,
                UiCommand::CancelPlanningRun,
            ],
            Some("haulage") => match arguments["haulage_step"].as_str() {
                None => vec![UiCommand::RunAllHaulageStages],
                Some("network") => vec![UiCommand::RunHaulageStage(HaulageStep::Network)],
                Some("truck_classes") => vec![UiCommand::RunHaulageStage(HaulageStep::TruckClasses)],
                Some(_) => return Err("haulage_step is network or truck_classes.".to_owned()),
            },
            _ => return Err("what is check, step, period, all, improve, cancel or haulage.".to_owned()),
        };
        for command in commands {
            self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))?;
        }
        Ok(json!({ "started": arguments["what"] }))
    }

    fn mcp_export_video(&mut self, arguments: &Value) -> Result<Value, String> {
        use crate::{
            rendering::graphics::{offscreen::ViewAim, video_capture::VideoView},
            ui::state::{PlanningView, VideoExportRequest, Workspace},
        };
        let view = match self.mcp_view_aim(arguments)? {
            ViewAim::Window => None,
            aim => Some(VideoView {
                aim,
                perspective: arguments["perspective"].as_bool().unwrap_or(false),
            }),
        };
        let path = std::path::PathBuf::from(arguments["path"].as_str().ok_or("Give the path.")?);
        if !path.is_absolute() {
            return Err(format!("{} is not an absolute path.", path.display()));
        }
        // The recording plays Animate, so Animate is shown first.
        let mut commands = Vec::new();
        crate::ui::elements::main_menu::select_workspace(&mut self.editor, &mut commands, Workspace::Planning);
        self.handle_ui_commands(commands);
        self.handle_ui_commands(vec![UiCommand::SetPlanningView(PlanningView::Animate)]);
        self.editor.schedule_animation_enabled = true;
        // Animate shows the held result; it is put on the page here rather
        // than on the next frame, which a window with no frames never draws.
        self.mirror_schedule_calculation();
        let schedule = self
            .editor
            .schedule_result
            .as_deref()
            .ok_or("No schedule has been calculated yet: run_schedule with all calculates one.")?;
        let segments = match arguments["segments"].as_array() {
            None => vec![(0.0, schedule.requested_end_h)],
            Some(segments) => segments
                .iter()
                .map(
                    |segment| match segment.as_array().map(|pair| (pair.first().and_then(Value::as_f64), pair.get(1).and_then(Value::as_f64))) {
                        Some((Some(start), Some(end))) if start < end => Ok((start, end)),
                        _ => Err("Each segment is [start, end] hours, start before end.".to_owned()),
                    },
                )
                .collect::<Result<Vec<_>, _>>()?,
        };
        let request = VideoExportRequest {
            segments,
            hours_per_second: arguments["hours_per_second"]
                .as_f64()
                .filter(|rate| *rate > 0.0)
                .ok_or("hours_per_second must be above 0.")?,
            fps: arguments["fps"].as_f64().unwrap_or(30.0),
            width: arguments["width"].as_u64().map_or(1920, |width| u32::try_from(width).unwrap_or(u32::MAX)),
            height: arguments["height"].as_u64().map_or(1080, |height| u32::try_from(height).unwrap_or(u32::MAX)),
            destination_labels: arguments["destination_labels"].as_bool().unwrap_or(true),
            machine_labels: arguments["machine_labels"].as_bool().unwrap_or(true),
        };
        self.execute_file_dialog_action(crate::app::commands::file::FileDialogAction::ExportVideo(path.clone(), request))
            .map_err(|error| format!("{error:#}"))?;
        if let Some(recording) = self.video_recording.as_mut() {
            recording.view = view;
        }
        Ok(json!({ "recording": path.display().to_string() }))
    }

    fn mcp_schedule_report(&mut self, arguments: &Value) -> Result<Value, String> {
        // The report reads the held result, and names blocks by their bars'
        // areas, as the schedule pages show them; only a window showing one
        // of them keeps those up to date.
        self.mirror_schedule_bar_views();
        self.mirror_schedule_calculation();
        let grouping = match arguments["by"].as_str() {
            None | Some("day") => ReportGrouping::Day,
            Some("week") => ReportGrouping::Week,
            Some("whole") => ReportGrouping::Whole,
            Some(_) => return Err("by is day, week or whole.".to_owned()),
        };
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let schedule = self
            .editor
            .schedule_result
            .as_deref()
            .ok_or("No schedule has been calculated yet: run_schedule with all calculates one.")?;
        let (_, text) = crate::ui::elements::schedule_calendar::schedule_report_text(&self.editor, document.schedule(), document, grouping, ',')
            .ok_or("No schedule has been calculated yet.")?;
        let report = &schedule.report;
        let mut result = json!({
            "solver": {
                "quality": report.quality.map(|quality| format!("{quality:?}").to_lowercase()),
                "objective": report.objective,
                "bound": report.bound,
                "gap": report.gap,
                "solve_seconds": report.solve_s,
            },
            "hours_calculated": schedule.requested_end_h,
        });
        let text = super::plain(&text);
        let wanted: Option<Vec<&str>> = arguments["sections"].as_array().map(|sections| sections.iter().filter_map(Value::as_str).collect());
        match arguments["format"].as_str() {
            Some("csv") if wanted.is_none() => result["report_csv"] = json!(text),
            Some("csv") => {
                let kept: Vec<&str> = text
                    .split("\n\n")
                    .filter(|block| {
                        block
                            .lines()
                            .next()
                            .is_some_and(|title| wanted.as_ref().is_some_and(|wanted| wanted.iter().any(|name| name.eq_ignore_ascii_case(title.trim()))))
                    })
                    .collect();
                result["report_csv"] = json!(kept.join("\n\n"));
            }
            None | Some("tables") => {
                let tables = report_tables(&text, wanted.as_deref())?;
                result["title"] = json!(text.lines().next().unwrap_or_default().trim_matches('"'));
                result["tables"] = tables;
            }
            Some(_) => return Err("format is tables or csv.".to_owned()),
        }
        Ok(result)
    }
}

/// The video's own arguments with render_view's for placing its view.
fn video_schema(mut properties: Value) -> Value {
    for key in ["frame", "box", "view", "azimuth", "elevation", "perspective", "section"] {
        properties[key] = super::view::view_property(key);
    }
    properties
}

/// A small setup with one of most things, made through the plan's own
/// methods so it is always one the project would take: the shapes a new
/// setup's lists hold, and how their ids refer to each other. Field
/// conditions name the project's first reserve fields where it has them.
fn example_plan(document: &crate::model::Document) -> crate::model::schedule::ScheduleResult<SchedulePlan> {
    use crate::model::{
        ReserveFieldId,
        schedule::{
            WorkWindow,
            destinations::{Bound, ConditionTest, DestinationId, DestinationKind, FieldCondition},
        },
    };

    let field = |index: usize| document.reserve_fields().get(index).map_or(ReserveFieldId(index as u64), |field| field.id);
    let mut plan = SchedulePlan::default();
    plan.set_tonnage_field(Some(field(0)));
    let class = plan.add_class("Excavator", 1500.0)?;
    let agent = plan.add_agent("EX01", class)?;
    let first_week = WorkWindow { start_h: 0.0, end_h: Some(168.0) };
    plan.add_bar("Stage 1", Some(agent), 0, first_week)?;
    let routing = plan.routing_mut();
    let crusher = DestinationId::Standalone(routing.add_standalone("Crusher", DestinationKind::Crusher)?);
    let stockpile = DestinationId::Standalone(routing.add_standalone("ROM stockpile", DestinationKind::Stockpile)?);
    let dump = DestinationId::Standalone(routing.add_standalone("Waste dump", DestinationKind::Dump)?);
    let ore = routing.add_rule("Ore", vec![crusher, stockpile])?;
    routing.add_rule("Waste", vec![dump])?;
    if let Some(rule) = routing.rules.iter_mut().find(|rule| rule.id == ore) {
        rule.conditions = vec![FieldCondition {
            field: field(1),
            test: ConditionTest::Range {
                lower: Some(Bound { value: 55.0, inclusive: true }),
                upper: None,
            },
        }];
    }
    // Reclaimed material carries only the grades listed here, so the ore
    // rule's grade must be one of them for the stockpile's ore to be routed.
    plan.experiment_mut().grades = vec![(field(1), crate::model::schedule::experiment::GradeUnit::Percent)];
    plan.add_reclaim_bar("Rehandle", Some(agent), 1, WorkWindow { start_h: 168.0, end_h: None }, vec![stockpile], None)?;
    let truck = plan.trucks_mut().add_class("Haul truck")?;
    // A class starts with no trucks, which hauls nothing.
    if let Some(class) = plan.trucks_mut().classes.iter_mut().find(|class| class.id == truck) {
        class.calendar.default_units = 6;
    }
    plan.trucks_mut().add_rule("All hauls", vec![truck])?;
    let mining = plan.cashflow_mut().add_rule("Mining cost")?;
    let delivered = plan.cashflow_mut().add_rule("Ore delivered")?;
    for rule in &mut plan.cashflow_mut().rules {
        if rule.id == mining {
            rule.value_per_tonne = -3.0;
        } else if rule.id == delivered {
            rule.destinations = crate::model::schedule::destinations::DestinationSelection::Only(vec![crusher]);
            rule.value_per_tonne = 25.0;
        }
    }
    // Served as an example, so it must be one the project would take.
    plan.validate_loaded()?;
    Ok(plan)
}

/// A road out of a pit to the example setup's crusher, with branches on to
/// its stockpile and waste dump, made through the network's own methods.
fn example_network() -> anyhow::Result<HaulNetwork> {
    use glam::DVec3;

    use crate::model::{
        haulage::network::NodeRole,
        schedule::destinations::{DestinationId, StandaloneDestinationId},
    };

    let mut network = HaulNetwork::default();
    let pit = network.add_node(DVec3::new(500.0, 500.0, 40.0))?;
    let crusher = network.add_node(DVec3::new(500.0, 1150.0, 100.0))?;
    let stockpile = network.add_node(DVec3::new(600.0, 1150.0, 100.0))?;
    let dump = network.add_node(DVec3::new(400.0, 1150.0, 100.0))?;
    network.add_road("Ramp".to_owned(), pit, crusher, Vec::new())?;
    network.add_road("To stockpile".to_owned(), crusher, stockpile, Vec::new())?;
    network.add_road("To dump".to_owned(), crusher, dump, Vec::new())?;
    let roles = [
        (crusher, NodeRole::Dump(DestinationId::Standalone(StandaloneDestinationId(0)))),
        (stockpile, NodeRole::DumpAndReclaim(DestinationId::Standalone(StandaloneDestinationId(1)))),
        (dump, NodeRole::Dump(DestinationId::Standalone(StandaloneDestinationId(2)))),
    ];
    for (node, role) in roles {
        if let Some(node) = network.nodes.iter_mut().find(|candidate| candidate.id == node) {
            node.role = Some(role);
        }
    }
    // Served as an example, so it must be one the project would take.
    network.validate()?;
    Ok(network)
}

/// The report's CSV text as its tables: after the title, each block is a
/// table's title line, then its header and rows. Read from the export's own
/// text, so what the tables say and what the export writes cannot differ.
fn report_tables(text: &str, wanted: Option<&[&str]>) -> Result<Value, String> {
    let mut tables = serde_json::Map::new();
    for block in text.split("\n\n").skip(1) {
        let Some((title, body)) = block.split_once('\n') else { continue };
        let title = title.trim();
        if wanted.is_some_and(|wanted| !wanted.iter().any(|name| name.eq_ignore_ascii_case(title))) {
            continue;
        }
        let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(body.as_bytes());
        let columns: Vec<String> = reader.headers().map_err(|error| error.to_string())?.iter().map(ToOwned::to_owned).collect();
        let rows: Vec<Value> = reader
            .records()
            .map(|record| {
                record.map(|record| {
                    Value::Array(
                        record
                            .iter()
                            .map(|cell| {
                                if cell.is_empty() {
                                    Value::Null
                                } else {
                                    match cell.parse::<f64>().ok().filter(|number| number.is_finite()) {
                                        None => json!(cell),
                                        // Whole figures as whole numbers, as the text wrote them.
                                        Some(_) if let Ok(whole) = cell.parse::<i64>() => json!(whole),
                                        Some(number) => json!(number),
                                    }
                                }
                            })
                            .collect(),
                    )
                })
            })
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?;
        tables.insert(title.to_owned(), json!({ "columns": columns, "rows": rows }));
    }
    Ok(Value::Object(tables))
}
