//! Pit Design through MCP: pit design objects, read, made from closed
//! polylines, changed and exploded as the Pit Design tool
//! does, and `design_pit`, which makes one in one call.

use serde_json::{Value, json};

use super::{failed, merge_patch, ok};
use crate::{
    app::{App, commands::pit_design::pit_parts},
    model::{
        Object, ObjectId,
        pit_design::{
            Design, End,
            object::{Outcome, PitObject, ScenarioId},
            settings::{PitDesignSettings, StopMode},
        },
        triangulation::TriangulationId,
    },
    ui::dialogs::pit_design::{NoDesign, end_text, no_design_text, problem_text},
};

const SETTINGS_DESCRIPTION: &str = "A scenario's settings: direction (up from a floor's toe, or down from a crest); bench_height; face_angle (degrees from horizontal); berm_width; wall_radius (the tightest the walls turn); stop (benches, elevation or daylight) with benches, stop_elevation, or topography (a surface id to daylight against, building up); ramp (true or false) with ramp_width, ramp_gradient (a percentage: 10 is 1 in 10, held along the road's steeper edge), ramp_start ([x, y] near where it leaves the string, or null for the string's first point), climbs_clockwise, switchback (true turns back every switchback_every benches round inside_radius). Its surface is made with triangulate, which takes a pit design as every string it draws.";

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_pit_designs",
            "title": "Read the pit designs",
            "description": "Read the pit designs in the project's loaded layers. A pit design is a design object: its own vertices are the closed string it is designed from, and it draws as that string and the benches and ramp designed from it, worked out again whenever its string or settings change (fresh says whether that has caught up). It holds scenarios, each a full set of settings, one active. Without design: a summary of each. With design (its object id): that one whole, every scenario's settings and what the active one comes to included, which set_pit_design takes back.",
            "inputSchema": {
                "type": "object",
                "properties": { "design": { "type": "integer", "minimum": 0 } },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "set_pit_design",
            "title": "Make or change a pit design",
            "description": format!("Make a closed polyline a pit design, in its place and keeping its id (string), or change a pit design (design). settings is a JSON merge patch over a scenario's settings, the active one unless scenario names another: give only what changes. A new design starts from the window's defaults. active makes a scenario the one the design is built with. The design is worked out at once: the result says what it comes to. Its string is edited like any polyline's vertices (move_objects and the vertex tools), and the design follows. {SETTINGS_DESCRIPTION}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "design": { "type": "integer", "minimum": 0, "description": "A pit design's object id." },
                    "string": { "type": "integer", "minimum": 0, "description": "A closed polyline to make a pit design: the floor's toe building up, the crest building down." },
                    "name": { "type": "string", "minLength": 1 },
                    "scenario": { "type": "integer", "minimum": 0, "description": "The scenario settings and scenario_name change. The active one if left out." },
                    "scenario_name": { "type": "string", "minLength": 1 },
                    "settings": { "type": "object" },
                    "active": { "type": "integer", "minimum": 0, "description": "Make this scenario active." },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "manage_pit_design",
            "title": "Explode a pit design or change its scenarios",
            "description": "explode a pit design into polylines on its layer (its string keeps the object's id, and each bench, crest and ramp string becomes a polyline of its own): they are no longer designed; add_scenario (a copy of the active scenario, made active, named name); remove_scenario (the active one, or scenario; a design keeps at least one). A pit design is deleted with delete_objects, copied with copy_objects. Returns any new ids.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "design": { "type": "integer", "minimum": 0 },
                    "action": { "type": "string", "enum": ["explode", "add_scenario", "remove_scenario"] },
                    "name": { "type": "string", "minLength": 1 },
                    "scenario": { "type": "integer", "minimum": 0 },
                },
                "required": ["design", "action"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false },
        }),
        json!({
            "name": "design_pit",
            "title": "Design a pit in one call",
            "description": "Make a pit design from a closed polyline, in its place, in one call: set_pit_design with string and these settings. Up from a floor's toe (each bench a face at face_angle rising bench_height, then a berm berm_width wide) or down from a crest. Exactly one of benches, stop_rl or daylight says where it stops. The ramp climbs at gradient along the steeper edge of the road, starting where the string passes nearest start, as a spiral round the walls or, with switchback_every, turning back on a level pad. Every ring is kept sound: where the next bench has no room, the design stops at the last good one and says why. With preview it only says what the design comes to, and makes nothing. Returns the design's id, which the other pit design tools take.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "string": { "type": "integer", "minimum": 0, "description": "The closed polyline to design from: the floor's toe building up, the crest building down." },
                    "name": { "type": "string", "minLength": 1 },
                    "direction": { "type": "string", "enum": ["up", "down"], "description": "Up from a floor, or down from a crest. up if left out." },
                    "bench_height": { "type": "number", "exclusiveMinimum": 0 },
                    "face_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90, "description": "Degrees from horizontal." },
                    "berm_width": { "type": "number", "minimum": 0 },
                    "wall_radius": { "type": "number", "minimum": 0, "description": "The tightest the walls turn: sharp corners are rounded to it. 0 if left out." },
                    "benches": { "type": "integer", "minimum": 1, "description": "Stop after this many benches." },
                    "stop_rl": { "type": "number", "description": "Stop at this RL, the last bench cut short to meet it." },
                    "daylight": { "type": "integer", "minimum": 0, "description": "A loaded topography surface: build up until a whole bench stands clear of it." },
                    "ramp": {
                        "type": "object",
                        "description": "The haul ramp. None if left out.",
                        "properties": {
                            "width": { "type": "number", "exclusiveMinimum": 0 },
                            "gradient": { "type": "number", "exclusiveMinimum": 0, "description": "Rise over run as a percentage: 10 is 1 in 10." },
                            "start": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] near where it leaves the string. The string's first point if left out." },
                            "clockwise": { "type": "boolean", "description": "Climb clockwise seen from above. False if left out." },
                            "switchback_every": { "type": "integer", "minimum": 1, "description": "Turn back every this many benches. A spiral if left out." },
                            "inside_radius": { "type": "number", "minimum": 0, "description": "The tightest the road's inside edge turns on a switchback. 15 if left out." },
                        },
                        "required": ["width", "gradient"],
                        "additionalProperties": false,
                    },
                    "preview": { "type": "boolean", "description": "Only say what the design comes to; make nothing. False if left out." },
                },
                "required": ["string", "bench_height", "face_angle", "berm_width"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
    ]
}

/// The settings `design_pit`'s arguments give.
fn one_call_settings(arguments: &Value) -> Result<PitDesignSettings, String> {
    use crate::model::pit_design::Direction;
    let number = |key: &str| arguments[key].as_f64().ok_or_else(|| format!("design_pit needs {key}."));
    let mut settings = PitDesignSettings {
        direction: match arguments["direction"].as_str() {
            None | Some("up") => Direction::Up,
            Some("down") => Direction::Down,
            Some(other) => return Err(format!("direction is up or down, not {other}.")),
        },
        bench_height: number("bench_height")?,
        face_angle: number("face_angle")?,
        berm_width: number("berm_width")?,
        wall_radius: arguments["wall_radius"].as_f64().unwrap_or(0.0),
        ramp: false,
        ..PitDesignSettings::default()
    };
    let stops = [&arguments["benches"], &arguments["stop_rl"], &arguments["daylight"]]
        .iter()
        .filter(|value| !value.is_null())
        .count();
    if stops != 1 {
        return Err("Give exactly one of benches, stop_rl or daylight.".to_owned());
    }
    if let Some(benches) = arguments["benches"].as_u64() {
        settings.stop = StopMode::Benches;
        settings.benches = u32::try_from(benches).unwrap_or(u32::MAX);
    } else if let Some(rl) = arguments["stop_rl"].as_f64() {
        settings.stop = StopMode::Elevation;
        settings.stop_elevation = rl;
    } else if let Some(surface) = arguments["daylight"].as_u64() {
        settings.stop = StopMode::Daylight;
        settings.topography = Some(TriangulationId(surface));
    } else {
        return Err("benches is a whole number, stop_rl an RL and daylight a surface id.".to_owned());
    }
    let ramp = &arguments["ramp"];
    if !ramp.is_null() {
        settings.ramp = true;
        settings.ramp_width = ramp["width"].as_f64().ok_or("ramp needs width.")?;
        settings.ramp_gradient = ramp["gradient"].as_f64().ok_or("ramp needs gradient, as a percentage.")?;
        settings.ramp_start = match (ramp["start"][0].as_f64(), ramp["start"][1].as_f64()) {
            (Some(x), Some(y)) => Some(glam::DVec2::new(x, y)),
            _ if ramp["start"].is_null() => None,
            _ => return Err("ramp.start is [x, y].".to_owned()),
        };
        settings.climbs_clockwise = ramp["clockwise"].as_bool().unwrap_or(false);
        if let Some(every) = ramp["switchback_every"].as_u64() {
            settings.switchback = true;
            settings.switchback_every = u32::try_from(every).unwrap_or(u32::MAX);
        }
        settings.inside_radius = ramp["inside_radius"].as_f64().unwrap_or(15.0);
    }
    Ok(settings)
}

/// Why a design ended, in a word.
fn end_name(end: End) -> &'static str {
    match end {
        End::Complete => "complete",
        End::Daylight => "daylight",
        End::OutOfRoom => "out_of_room",
        End::RampTooLong => "no_room_for_ramp",
        End::LeftGround => "left_topography",
    }
}

/// What a design comes to.
fn summary(design: &Design) -> Value {
    let round = |value: f64| (value * 100.0).round() / 100.0;
    json!({
        "benches": design.benches,
        "ended": end_name(design.end),
        "ended_because": end_text(design),
        "rls": design.levels.iter().copied().map(round).collect::<Vec<_>>(),
        "final_rl": round(design.final_elevation()),
        "volume": design.volume.round(),
        "ramp_length": round(design.ramp_length),
        "ramp_exit": design.ramp_exit.map(|exit| [round(exit.x), round(exit.y), round(exit.z)]),
        "ramp_start_moved": (design.ramp_start_moved > 0.0).then(|| round(design.ramp_start_moved)),
        "strings": design.strings.len(),
        "bench_report": design.report.iter().map(|bench| {
            let mut row = json!({ "rl": round(bench.elevation), "height": round(bench.height), "area": bench.area.round() });
            if let Some(road) = bench.ramp {
                row["ramp"] = json!({
                    "grade_percent": round(road.steepest * 100.0),
                    "flat": round(road.flat),
                    "lengthened": round(road.lengthened),
                    "narrowest": round(road.narrowest),
                    "switchback": road.switchback,
                });
            }
            row
        }).collect::<Vec<_>>(),
    })
}

/// Why there is no design, said so the AI can put it right.
fn no_design_message(reason: &NoDesign) -> String {
    match reason {
        NoDesign::NotClosed => "The string must be a closed polyline round some area.".to_owned(),
        NoDesign::NoTopography => "Daylighting needs a topography surface id.".to_owned(),
        NoDesign::TopographyNotLoaded(name) => format!("The topography {name} is not loaded. Load it with set_loaded first."),
        NoDesign::Settings(_) => no_design_text(reason),
    }
}

/// What a design came to, or why it came to nothing.
fn comes_to(design: &Result<Design, NoDesign>) -> Value {
    match design {
        Ok(design) => summary(design),
        Err(reason) => json!({ "problem": no_design_message(reason) }),
    }
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_pit_design_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "get_pit_designs" => self.mcp_get_pit_designs(arguments),
            "set_pit_design" => self.mcp_set_pit_design(arguments),
            "manage_pit_design" => self.mcp_manage_pit_design(arguments),
            "design_pit" => self.mcp_design_pit(arguments),
            _ => return None,
        };
        Some(match result {
            Ok(value) => ok(&value),
            Err(message) => failed(message),
        })
    }

    /// The pit design object `arguments` names, and what it is designed with.
    fn mcp_pit_design(&self, arguments: &Value) -> Result<(ObjectId, PitObject), String> {
        let id = arguments["design"].as_u64().map(ObjectId).ok_or("Give design, a pit design's object id.")?;
        let object = self
            .workspace
            .active_document()
            .and_then(|document| document.get_object(id))
            .ok_or_else(|| format!("There is no design object with id {} in a loaded layer: get_pit_designs lists the pit designs.", id.0))?;
        let (_, pit) = pit_parts(object).ok_or_else(|| format!("Object {} is a {}, not a pit design: set_pit_design with string makes a closed polyline one.", id.0, object.kind_name()))?;
        Ok((id, pit.clone()))
    }

    fn mcp_describe_pit(&self, object: &Object) -> Value {
        let Some((verts, pit)) = pit_parts(object) else {
            return Value::Null;
        };
        let settings = pit.settings();
        let ground = settings
            .ground()
            .and_then(|id| self.triangulations.iter().find(|surface| surface.id == id))
            .filter(|surface| surface.state.loaded)
            .map(|surface| surface.geometry);
        let layer = self.workspace.active_document().and_then(|document| document.layer(object.layer()));
        json!({
            "id": object.id().0,
            "name": pit.name,
            "layer": layer.map(|layer| json!({ "id": layer.id.0, "name": layer.name })),
            "active": pit.scenario().map(|scenario| scenario.id.0),
            "scenarios": pit.scenarios.iter().map(|scenario| json!({ "id": scenario.id.0, "name": scenario.name })).collect::<Vec<_>>(),
            "fresh": pit.is_fresh(verts, ground),
        })
    }

    fn mcp_get_pit_designs(&self, arguments: &Value) -> Result<Value, String> {
        if arguments.get("design").is_some() {
            let (id, pit) = self.mcp_pit_design(arguments)?;
            let object = self.operand_object(id).ok_or("The design is gone.")?;
            let mut value = self.mcp_describe_pit(object);
            if let Some((verts, _)) = pit_parts(object) {
                value["string"] = json!(verts.iter().map(|vertex| [vertex.pos.x, vertex.pos.y, vertex.pos.z]).collect::<Vec<_>>());
            }
            value["scenarios"] = json!(
                pit.scenarios
                    .iter()
                    .map(|scenario| json!({ "id": scenario.id.0, "name": scenario.name, "settings": scenario.settings }))
                    .collect::<Vec<_>>()
            );
            value["comes_to"] = match pit.derived.as_ref().map(|derived| &derived.outcome) {
                Some(Outcome::Design(design)) => summary(design),
                Some(Outcome::NotClosed) => json!({ "problem": no_design_message(&NoDesign::NotClosed) }),
                Some(Outcome::Problem(problem)) => json!({ "problem": problem_text(*problem) }),
                None => json!({ "problem": "Not worked out yet." }),
            };
            return Ok(value);
        }
        let designs: Vec<Value> = self
            .workspace
            .active_document()
            .map(|document| document.objects().iter().filter(|object| pit_parts(object).is_some()).map(|object| self.mcp_describe_pit(object)).collect())
            .unwrap_or_default();
        Ok(json!({ "designs": designs }))
    }

    /// A closed polyline's id, if `arguments` gives one.
    fn mcp_closed_polyline(&self, arguments: &Value) -> Result<Option<ObjectId>, String> {
        let Some(id) = arguments["string"].as_u64().map(ObjectId) else {
            return Ok(None);
        };
        let object = self
            .workspace
            .active_document()
            .and_then(|document| document.get_object(id))
            .ok_or_else(|| format!("There is no design object with id {} in a loaded layer.", id.0))?;
        if !(matches!(object, Object::Polyline { .. }) && object.encloses_area()) {
            return Err(format!("Object {} is a {}: a pit design is made from a closed polyline round some area.", id.0, object.kind_name()));
        }
        Ok(Some(id))
    }

    fn mcp_set_pit_design(&mut self, arguments: &Value) -> Result<Value, String> {
        let (id, mut pit) = match (arguments.get("design").is_some(), self.mcp_closed_polyline(arguments)?) {
            (true, None) => self.mcp_pit_design(arguments)?,
            (false, Some(string)) => {
                self.make_pit_design(string, arguments["name"].as_str().map(str::to_owned), None)
                    .map_err(|error| format!("{error:#}"))?;
                self.mcp_pit_design(&json!({ "design": string.0 }))?
            }
            (true, Some(_)) => return Err("Give design to change a pit design, or string to make one, not both.".to_owned()),
            (false, None) => return Err("Give design, a pit design's id, or string, a closed polyline to make one.".to_owned()),
        };
        if let Some(name) = arguments["name"].as_str() {
            pit.name = name.to_owned();
        }
        if let Some(active) = arguments["active"].as_u64().map(ScenarioId) {
            if !pit.scenarios.iter().any(|scenario| scenario.id == active) {
                return Err(format!("The design has no scenario with id {}.", active.0));
            }
            pit.active = active;
        }
        let target = arguments["scenario"].as_u64().map(ScenarioId).or_else(|| pit.scenario().map(|scenario| scenario.id));
        let scenario = pit
            .scenarios
            .iter_mut()
            .find(|scenario| Some(scenario.id) == target)
            .ok_or_else(|| format!("The design has no scenario with id {}.", target.map_or(0, |id| id.0)))?;
        if let Some(name) = arguments["scenario_name"].as_str() {
            scenario.name = name.to_owned();
        }
        if let Some(patch) = arguments.get("settings") {
            let mut value = serde_json::to_value(&scenario.settings).map_err(|error| error.to_string())?;
            merge_patch(&mut value, patch);
            scenario.settings = serde_json::from_value(value).map_err(|error| format!("The settings do not fit a pit design scenario: {error}"))?;
        }
        let design = self.set_pit_now(id, pit).map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "design": id.0, "comes_to": comes_to(&design) }))
    }

    fn mcp_manage_pit_design(&mut self, arguments: &Value) -> Result<Value, String> {
        let (id, mut pit) = self.mcp_pit_design(arguments)?;
        match arguments["action"].as_str().unwrap_or_default() {
            "explode" => {
                let ids = self.explode_pit_design(id).map_err(|error| format!("{error:#}"))?;
                Ok(json!({ "polylines": ids.iter().map(|id| id.0).collect::<Vec<_>>() }))
            }
            "add_scenario" => {
                let name = arguments["name"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| crate::i18n::tr!("pit-design-scenario-copy-name", name = pit.scenario().map(|scenario| scenario.name.clone()).unwrap_or_default()));
                let scenario = pit.add_scenario(name);
                let design = self.set_pit_now(id, pit).map_err(|error| format!("{error:#}"))?;
                Ok(json!({ "design": id.0, "scenario": scenario.0, "comes_to": comes_to(&design) }))
            }
            "remove_scenario" => {
                if let Some(scenario) = arguments["scenario"].as_u64().map(ScenarioId) {
                    if !pit.scenarios.iter().any(|candidate| candidate.id == scenario) {
                        return Err(format!("The design has no scenario with id {}.", scenario.0));
                    }
                    pit.active = scenario;
                }
                if !pit.remove_scenario() {
                    return Err("A design keeps at least one scenario.".to_owned());
                }
                let active = pit.active;
                let design = self.set_pit_now(id, pit).map_err(|error| format!("{error:#}"))?;
                Ok(json!({ "design": id.0, "active": active.0, "comes_to": comes_to(&design) }))
            }
            other => Err(format!("action is explode, add_scenario or remove_scenario, not {other}.")),
        }
    }

    fn mcp_design_pit(&mut self, arguments: &Value) -> Result<Value, String> {
        let settings = one_call_settings(arguments)?;
        let string = self.mcp_closed_polyline(arguments)?.ok_or("design_pit needs string, a closed polyline's id.")?;
        let verts = match self.operand_object(string) {
            Some(Object::Polyline { verts, .. }) => verts.clone(),
            _ => return Err("design_pit needs string, a closed polyline's id.".to_owned()),
        };
        let design = self.design_pit_now(&verts, &settings).and_then(|preview| preview.design);
        let built = match &design {
            Ok(built) => built,
            Err(reason) => return Err(no_design_message(reason)),
        };
        if arguments["preview"].as_bool().unwrap_or(false) {
            return Ok(summary(built));
        }
        if built.benches == 0 {
            return Err(format!("Not even one bench fits: {}", end_text(built)));
        }
        self.make_pit_design(string, arguments["name"].as_str().map(str::to_owned), Some(settings))
            .map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "design": string.0, "comes_to": comes_to(&design) }))
    }
}
