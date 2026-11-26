//! The tools for survey and geology work: coordinate systems and moving
//! items between them, reference surfaces and points picked off drillholes,
//! linking downhole geophysics, and rain simulation.

use serde_json::{Value, json};

use super::{Target, entity, failed, ok};
use crate::{
    app::{App, commands::file::FileDialogAction},
    model::{
        ObjectId,
        drill_hole::{DrillHoleRef, ReferenceSide, ReferenceTarget},
        survey::SystemDefinition,
        water::WaterSettings,
    },
    ui::state::{UiCommand, WaterRunStatus},
};

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
    let ids = json!({ "type": "array", "items": { "type": "integer", "minimum": 0 } });
    vec![
        tool(
            "get_coordinate_systems",
            "Get coordinate systems",
            "Read the coordinate systems saved on this computer (registry systems by EPSG code, and mine grids defined from another system by origins, rotation and scale) and which is the mine's own.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "set_coordinate_system",
            "Save or delete a coordinate system",
            "Save a coordinate system definition, in the shape get_coordinate_systems shows (replace names the one it replaces), delete one by name, or choose the mine coordinate system (mine_system, null for none).",
            json!({
                "definition": { "type": "object" },
                "replace": { "type": "string" },
                "delete": { "type": "string" },
                "mine_system": { "type": ["string", "null"] },
            }),
            &[],
            false,
        ),
        tool(
            "transform_items",
            "Move items between coordinate systems",
            "Convert items' coordinates from one saved coordinate system to another, as Survey > Transform does: design objects, layers' objects, surfaces, block models, point clouds, rasters and drillholes. Runs in the background; one undo step.",
            json!({
                "items": super::items_schema("What to convert."),
                "from": { "type": "string", "description": "The system they are in, by name." },
                "to": { "type": "string", "description": "The system to convert them to, by name." },
            }),
            &["items", "from", "to"],
            false,
        ),
        tool(
            "reference_surface",
            "Build a reference surface",
            "Build a surface through design points (such as a seam roof picked off drillholes), bent to follow open control strings, and clipped to a closed extent, as Reference Surface does.",
            json!({
                "points": { "type": "array", "minItems": 3, "items": { "type": "integer", "minimum": 0 }, "description": "Design point object ids." },
                "controls": ids,
                "extent": { "type": "integer", "minimum": 0, "description": "A closed polyline to clip to." },
            }),
            &["points"],
            false,
        ),
        tool(
            "reference_points",
            "Pick reference points off drillholes",
            "Put one design point on each hole where a logged code, or any code of a working section, starts (roof) or ends (floor), on a new layer, as Reference Points does.",
            json!({
                "drillholes": { "type": "integer", "minimum": 0 },
                "holes": { "type": "array", "items": { "type": "string" }, "description": "Hole ids. Every hole if left out." },
                "field": { "type": "string", "description": "The categorical field the codes are in." },
                "code": { "type": "string" },
                "working_section": { "type": "string", "description": "In place of code." },
                "side": { "type": "string", "enum": ["roof", "floor"] },
            }),
            &["drillholes", "field", "side"],
            false,
        ),
        tool(
            "link_geophysics",
            "Link downhole geophysics",
            "Link downhole geophysics files (LAS or CSV logs) to a drillhole collection, as Link Geophysics does, so each hole's logs can be read.",
            json!({ "drillholes": { "type": "integer", "minimum": 0 }, "paths": { "type": "array", "minItems": 1, "items": { "type": "string" } } }),
            &["drillholes", "paths"],
            false,
        ),
        tool(
            "run_rain_simulation",
            "Simulate rain on a surface",
            "Rain on a surface and follow the water as it runs off, ponds and soaks in, as Rain Simulation does. The run opens in a tab of its own; rain_simulation shows it at an instant, and render_view with view window looks at it while its tab is shown. Settings left out take their defaults (20 mm over 4 hours, 4 hours run on, roughness 0.035, 2 mm/h infiltration, 30 s steps).",
            json!({
                "surface": { "type": "integer", "minimum": 0 },
                "rain_mm": { "type": "number", "exclusiveMinimum": 0 },
                "rain_hours": { "type": "number", "exclusiveMinimum": 0 },
                "run_on_hours": { "type": "number", "minimum": 0 },
                "roughness": { "type": "number", "exclusiveMinimum": 0, "description": "Manning's n." },
                "infiltration_mm_h": { "type": "number", "minimum": 0 },
                "step_s": { "type": "number", "exclusiveMinimum": 0 },
                "subdivide_m": { "type": "number", "exclusiveMinimum": 0, "description": "Split triangle sides longer than this, for detail." },
            }),
            &["surface"],
            false,
        ),
        tool(
            "rain_simulation",
            "Show a rain simulation",
            "List the rain runs open, with how far each has got, and show the open one at hours in; or close a run.",
            json!({
                "run": { "type": "integer", "minimum": 0, "description": "The run to show. The open one if left out." },
                "hours": { "type": "number", "minimum": 0, "description": "Show the water this many hours in." },
                "close": { "type": "boolean", "description": "Close run (discarding it)." },
            }),
            &[],
            false,
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_survey_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "get_coordinate_systems" => Ok(json!({
                "systems": serde_json::to_value(&self.editor.survey.definitions).unwrap_or_default(),
                "mine_system": self.editor.survey.local_system,
            })),
            "set_coordinate_system" => self.mcp_set_coordinate_system(arguments),
            "transform_items" => self.mcp_transform_items(arguments),
            "reference_surface" => self.mcp_reference_surface(arguments),
            "reference_points" => self.mcp_reference_points(arguments),
            "link_geophysics" => self.mcp_link_geophysics(arguments),
            "run_rain_simulation" => self.mcp_run_rain(arguments),
            "rain_simulation" => self.mcp_rain(arguments),
            _ => return None,
        };
        Some(result.map_or_else(failed, |value| ok(&value)))
    }

    fn mcp_do(&mut self, command: UiCommand) -> Result<(), String> {
        self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))
    }

    fn mcp_set_coordinate_system(&mut self, arguments: &Value) -> Result<Value, String> {
        let mut done = Vec::new();
        if let Some(definition) = arguments.get("definition") {
            let definition: SystemDefinition =
                serde_json::from_value(definition.clone()).map_err(|error| format!("The definition is not in the shape get_coordinate_systems shows: {error}"))?;
            done.push(format!("saved {}", definition.name));
            self.mcp_do(UiCommand::SaveSurveyDefinition {
                target: arguments["replace"].as_str().map(ToOwned::to_owned),
                definition,
            })?;
        }
        if let Some(name) = arguments["delete"].as_str() {
            self.mcp_do(UiCommand::DeleteSurveyDefinition(name.to_owned()))?;
            done.push(format!("deleted {name}"));
        }
        if let Some(system) = arguments.get("mine_system") {
            let system = system.as_str().map(ToOwned::to_owned);
            done.push(format!("mine system {}", system.as_deref().unwrap_or("none")));
            self.mcp_do(UiCommand::SetSurveyLocalSystem(system))?;
        }
        // The window's own commands put a failure in the dialog, not in their result.
        if let Some(message) = self.editor.survey.definition_message.take() {
            return Err(message);
        }
        Ok(json!({ "done": done }))
    }

    fn mcp_transform_items(&mut self, arguments: &Value) -> Result<Value, String> {
        let targets = self.mcp_targets(&arguments["items"])?;
        let system = |key: &str| -> Result<String, String> {
            let name = arguments[key].as_str().ok_or_else(|| format!("Give {key}."))?;
            if self.editor.survey.definitions.iter().any(|definition| definition.name == name) {
                Ok(name.to_owned())
            } else {
                Err(format!("There is no coordinate system called {name}. get_coordinate_systems lists them."))
            }
        };
        let (from, to) = (system("from")?, system("to")?);
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let mut handles = Vec::new();
        for target in targets {
            match target {
                Target::Layer(layer) => handles.extend(
                    document
                        .objects()
                        .iter()
                        .filter(|object| object.layer() == layer)
                        .map(|object| crate::model::SceneEntityId::Object(object.id())),
                ),
                target => handles.extend(entity(target)),
            }
        }
        self.mcp_do(UiCommand::TransformSurvey {
            entities: handles,
            from: Some(from),
            to: Some(to),
        })?;
        Ok(json!({ "started": true }))
    }

    fn mcp_reference_surface(&mut self, arguments: &Value) -> Result<Value, String> {
        let list = |key: &str| -> Vec<ObjectId> { arguments[key].as_array().into_iter().flatten().filter_map(Value::as_u64).map(ObjectId).collect() };
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let (points, controls) = (list("points"), list("controls"));
        let extent = arguments["extent"].as_u64().map(ObjectId);
        if let Some(missing) = points.iter().chain(&controls).chain(&extent).find(|id| document.get_object(**id).is_none()) {
            return Err(format!("There is no design object with id {} in a loaded layer.", missing.0));
        }
        let name = self.reference_surface_name(&points);
        let spacing = self.editor.reference_surface_spacing;
        self.mcp_do(UiCommand::BuildReferenceSurface {
            points,
            controls,
            extent,
            spacing,
            name,
        })?;
        Ok(json!({ "started": true }))
    }

    fn mcp_reference_points(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["drillholes"].as_u64().ok_or("Give the drillholes' id.")?;
        let collection = self
            .drill_holes
            .iter()
            .find(|collection| collection.id.0 == id)
            .ok_or_else(|| format!("There is no drillhole collection with id {id}."))?;
        let field = arguments["field"].as_str().ok_or("Give the field.")?.to_owned();
        match collection.dataset.fields.iter().find(|candidate| candidate.key == field).map(|candidate| &candidate.kind) {
            None => return Err(format!("The drillholes have no field {field}.")),
            Some(crate::model::drill_hole::DrillFieldKind::Numeric { .. }) => {
                return Err(format!("{field} is numeric: reference points are picked where a categorical field's code starts or ends."));
            }
            Some(crate::model::drill_hole::DrillFieldKind::Categorical { .. }) => {}
        }
        let target = match (arguments["code"].as_str(), arguments["working_section"].as_str()) {
            (Some(code), None) => ReferenceTarget::Code(code.to_owned()),
            (None, Some(section)) => ReferenceTarget::Section(section.to_owned()),
            _ => return Err("Give a code or a working_section.".to_owned()),
        };
        let side = match arguments["side"].as_str() {
            Some("roof") => ReferenceSide::Roof,
            Some("floor") => ReferenceSide::Floor,
            _ => return Err("side is roof or floor.".to_owned()),
        };
        let dataset = collection.id;
        let holes = match arguments["holes"].as_array() {
            None => (0..collection.dataset.holes.len()).map(|hole| DrillHoleRef { dataset, hole }).collect(),
            Some(names) => names
                .iter()
                .map(|name| {
                    let name = name.as_str().unwrap_or_default();
                    collection
                        .dataset
                        .holes
                        .iter()
                        .position(|hole| hole.dhid == name)
                        .map(|hole| DrillHoleRef { dataset, hole })
                        .ok_or_else(|| format!("There is no hole {name}."))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        let label = target.label();
        let picks = self
            .build_reference_points(holes, field.clone(), target, side)
            .ok_or_else(|| format!("No hole holds {label} in {field}: nothing was placed."))?;
        Ok(json!({
            "layer": picks.layer.0,
            "points": picks.points,
            "holes_without_it": picks.without,
            "holes_where_it_runs_more_than_once": picks.flagged,
        }))
    }

    fn mcp_link_geophysics(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["drillholes"].as_u64().ok_or("Give the drillholes' id.")?;
        let dataset = self
            .drill_holes
            .iter()
            .find(|collection| collection.id.0 == id)
            .map(|collection| collection.id)
            .ok_or_else(|| format!("There is no drillhole collection with id {id}."))?;
        let paths = arguments["paths"]
            .as_array()
            .ok_or("Give the paths.")?
            .iter()
            .map(|path| {
                let path = std::path::PathBuf::from(path.as_str().unwrap_or_default());
                if path.is_absolute() && path.is_file() {
                    Ok(path)
                } else {
                    Err(format!("{} is not a file at an absolute path.", path.display()))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let count = paths.len();
        self.execute_file_dialog_action(FileDialogAction::LinkGeophysics { dataset, paths })
            .map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "linked": count }))
    }

    fn mcp_run_rain(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["surface"].as_u64().ok_or("Give the surface's id.")?;
        let surface = self
            .triangulations
            .iter()
            .find(|surface| surface.id.0 == id)
            .map(|surface| surface.id)
            .ok_or_else(|| format!("There is no surface with id {id}."))?;
        let defaults = WaterSettings::default();
        let number = |key: &str, default: f64| arguments[key].as_f64().unwrap_or(default);
        let settings = WaterSettings {
            rain_mm: number("rain_mm", defaults.rain_mm),
            rain_hours: number("rain_hours", defaults.rain_hours),
            run_on_hours: number("run_on_hours", defaults.run_on_hours),
            roughness: number("roughness", defaults.roughness),
            infiltration_mm_h: number("infiltration_mm_h", defaults.infiltration_mm_h),
            step_s: number("step_s", defaults.step_s),
            subdivide_m: arguments["subdivide_m"].as_f64(),
        };
        if !settings.is_valid() {
            return Err("Those settings do not make a run: rain, its hours, roughness and the step must be above 0, and run on and infiltration not below it.".to_owned());
        }
        let before: Vec<u64> = self.water.ids().collect();
        self.mcp_do(UiCommand::RunWaterSimulation { surface, settings })?;
        // The run is made at once; its tab, on the next frame.
        let run = self.water.ids().find(|id| !before.contains(id));
        self.sync_water();
        Ok(json!({ "run": run }))
    }

    fn mcp_rain(&mut self, arguments: &Value) -> Result<Value, String> {
        let run = match arguments["run"].as_u64() {
            Some(run) if !self.water.has(run) => return Err(format!("There is no rain run {run}.")),
            Some(run) => Some(run),
            None => self.editor.water.open,
        };
        if arguments["close"].as_bool().unwrap_or(false) {
            let run = run.ok_or("There is no rain run open to close.")?;
            self.mcp_do(UiCommand::CloseWaterRun(run))?;
        } else if let Some(run) = run {
            self.editor.water.open = Some(run);
        }
        if let Some(hours) = arguments["hours"].as_f64() {
            self.editor.water.playing = false;
            self.editor.water.time_s = hours.max(0.0) * 3600.0;
        }
        // What the window would show on its next frame, said now.
        self.sync_water();
        self.redraw_requested = true;
        let water = &self.editor.water;
        let shown = water.shown.as_ref().map(|shown| {
            let status = match &shown.status {
                WaterRunStatus::Running(fraction) => json!({ "running": fraction }),
                WaterRunStatus::Failed(message) => json!({ "failed": message }),
                WaterRunStatus::Ready => json!("ready"),
            };
            json!({
                "status": status,
                "hours_shown": shown.instant_s.map(|seconds| seconds / 3600.0),
                "hours_worked_out": shown.loaded_s / 3600.0,
                "hours_long": shown.settings.duration_s() / 3600.0,
                "raining": water.raining,
            })
        });
        Ok(json!({
            "runs": water.tabs.iter().map(|tab| json!({ "run": tab.id, "title": tab.title })).collect::<Vec<_>>(),
            "open": water.open,
            "shown": shown,
        }))
    }
}
