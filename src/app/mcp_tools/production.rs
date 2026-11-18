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
        schedule::{AutoRun, SchedulePlan},
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
            json!({ "network": { "type": "boolean", "description": "Include the haul network. False if left out." } }),
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
            "run_schedule",
            "Run the production schedule",
            "Run the production schedule, as the Schedule workspace's buttons do: check (every setup step, running the Solids and Haulage pipelines when they are not current), a single setup step, period (calculate the next period), all (calculate every period), improve (search for a better schedule than the one calculated), or cancel. haulage runs the haul network's steps (network, truck_classes, or both when left out). Runs in the background: get_tasks follows it, and get_schedule_report reads the result.",
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
            "Record the calculated production schedule's animation to a video file, as Animate's export does: stretches of the schedule (hours from the start) played back to back at hours_per_second, at fps frames a second and width by height pixels. Shows the Animate page first. Runs in the background.",
            json!({
                "path": { "type": "string", "description": "An absolute path; its extension picks the format." },
                "segments": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "description": "[start, end] hours. The whole schedule if left out." },
                "hours_per_second": { "type": "number", "exclusiveMinimum": 0 },
                "fps": { "type": "number", "exclusiveMinimum": 0, "description": "30 if left out." },
                "width": { "type": "integer", "minimum": 16, "description": "1920 if left out." },
                "height": { "type": "integer", "minimum": 16, "description": "1080 if left out." },
                "destination_labels": { "type": "boolean" },
                "machine_labels": { "type": "boolean" },
            }),
            &["path", "hours_per_second"],
            false,
        ),
        tool(
            "get_schedule_report",
            "Get the schedule report",
            "Read the calculated production schedule as the report export writes it: tables of tonnes dug and reclaimed per loader, delivered and stockpiled per destination with grades, truck hours and cash flow, by day, by week, or over the whole schedule, as CSV. Also how well the solver did.",
            json!({ "by": { "type": "string", "enum": ["day", "week", "whole"], "description": "day if left out." } }),
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
            Some("period") => vec![UiCommand::RunSchedulePeriod],
            Some("all") => vec![UiCommand::RunAllSchedulePeriods],
            Some("improve") => vec![UiCommand::ImproveSchedule],
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
        // The recording follows the window's camera through Animate, frame
        // by frame, so it needs a window to follow.
        if self.headless {
            return Err("This Incline has no window (it was started with mcp --headless), and a schedule video is recorded from the window's view of Animate.".to_owned());
        }
        use crate::ui::state::{PlanningView, VideoExportRequest, Workspace};
        let path = std::path::PathBuf::from(arguments["path"].as_str().ok_or("Give the path.")?);
        if !path.is_absolute() {
            return Err(format!("{} is not an absolute path.", path.display()));
        }
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
        // The recording plays Animate, so Animate is shown first.
        let mut commands = Vec::new();
        crate::ui::elements::main_menu::select_workspace(&mut self.editor, &mut commands, Workspace::Planning);
        self.handle_ui_commands(commands);
        self.handle_ui_commands(vec![UiCommand::SetPlanningView(PlanningView::Animate)]);
        self.editor.schedule_animation_enabled = true;
        self.execute_file_dialog_action(crate::app::commands::file::FileDialogAction::ExportVideo(path.clone(), request))
            .map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "recording": path.display().to_string() }))
    }

    fn mcp_schedule_report(&self, arguments: &Value) -> Result<Value, String> {
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
        Ok(json!({
            "solver": {
                "quality": report.quality.map(|quality| format!("{quality:?}").to_lowercase()),
                "objective": report.objective,
                "bound": report.bound,
                "gap": report.gap,
                "solve_seconds": report.solve_s,
            },
            "hours_calculated": schedule.requested_end_h,
            "report_csv": super::plain(&text),
        }))
    }
}
