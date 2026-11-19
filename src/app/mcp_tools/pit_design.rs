//! Pit Design through MCP: design a pit's benches and ramp from a closed
//! string, as the Pit Design dialog does, and add it or only say what it
//! comes to.

use serde_json::{Value, json};

use super::{failed, ok};
use crate::{
    app::App,
    model::{
        ObjectId,
        pit_design::{Design, Direction, End},
        triangulation::TriangulationId,
    },
    ui::dialogs::pit_design::{NoDesign, PitDesignSettings, StopMode},
};

pub(super) fn definitions() -> Vec<Value> {
    vec![json!({
        "name": "design_pit",
        "title": "Design a pit",
        "description": "Design a pit's benches and haul ramp from a closed polyline, as Design > Pit Design does: up from a floor's toe (each bench a face at face_angle rising bench_height, then a berm berm_width wide) or down from a crest. Exactly one of benches, stop_rl or daylight says where it stops. The ramp climbs at gradient along the steeper edge of the road, starting where the string passes nearest start, as a spiral round the walls or, with switchback_every, turning back on a level pad. Every ring is kept sound: where the next bench has no room, the design stops at the last good one and says why. With preview it only says what the design comes to; otherwise its strings (toes, crests, ramp edges and centreline) go on a new design layer, and with surface its surface too (trimmed to the daylight topography with trim), as one undo step. A surface runs in the background: the result gives its task, which get_tasks follows.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "string": { "type": "integer", "minimum": 0, "description": "The closed polyline to design from: the floor's toe building up, the crest building down." },
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
                "surface": { "type": "boolean", "description": "Also build the pit's surface. False if left out." },
                "trim": { "type": "boolean", "description": "Trim the surface to the daylight topography. True if left out, with daylight." },
                "preview": { "type": "boolean", "description": "Only say what the design comes to; add nothing. False if left out." },
            },
            "required": ["string", "bench_height", "face_angle", "berm_width"],
            "additionalProperties": false,
        },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
    })]
}

/// The settings `arguments` give, as the dialog would hold them.
fn settings(arguments: &Value) -> Result<PitDesignSettings, String> {
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
        surface: arguments["surface"].as_bool().unwrap_or(false),
        trim_to_topography: arguments["trim"].as_bool().unwrap_or(true),
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
        "ended_because": crate::ui::dialogs::pit_design::end_text(design),
        "rls": design.levels.iter().copied().map(round).collect::<Vec<_>>(),
        "final_rl": round(design.final_elevation()),
        "volume": design.volume.round(),
        "ramp_length": round(design.ramp_length),
        "ramp_exit": design.ramp_exit.map(|exit| [round(exit.x), round(exit.y), round(exit.z)]),
        "ramp_start_moved": (design.ramp_start_moved > 0.0).then(|| round(design.ramp_start_moved)),
        "strings": design.strings.len(),
    })
}

impl App<'_> {
    /// Run `name` if it is this tool.
    pub(super) fn call_mcp_pit_design_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        (name == "design_pit").then(|| self.mcp_design_pit(arguments))
    }

    fn mcp_design_pit(&mut self, arguments: &Value) -> Value {
        let settings = match settings(arguments) {
            Ok(settings) => settings,
            Err(error) => return failed(error),
        };
        let Some(string) = arguments["string"].as_u64().map(ObjectId) else {
            return failed("design_pit needs string, a closed polyline's id.");
        };
        if self.workspace.active_document().and_then(|document| document.get_object(string)).is_none() {
            return failed(format!("There is no design object with id {} in a loaded layer.", string.0));
        }
        let preview = match self.design_pit_now(string, &settings) {
            Ok(preview) => preview,
            Err(reason) => return failed(no_design_message(&reason)),
        };
        let design = match preview.design {
            Ok(design) => design,
            Err(reason) => return failed(no_design_message(&reason)),
        };
        let mut result = summary(&design);
        if arguments["preview"].as_bool().unwrap_or(false) {
            return ok(&result);
        }
        if design.benches == 0 {
            return failed(format!("Not even one bench fits: {}", crate::ui::dialogs::pit_design::end_text(&design)));
        }
        if !self.activate_project_for_object(string) {
            return failed("The string's project could not be made the active one.");
        }
        let before = self.running_task_tickets();
        let layer_before = self.editor.active_layer;
        if let Err(error) = self.add_designed_pit(design, &settings) {
            return failed(format!("{error:#}"));
        }
        let tasks: Vec<Value> = self
            .background_tasks
            .reported
            .iter()
            .filter(|task| !before.contains(&task.ticket))
            .map(|task| json!({ "id": task.ticket.0, "task": task.label.trim_end_matches(['.', '…', ' ']) }))
            .collect();
        if tasks.is_empty() {
            result["layer"] = json!(self.editor.active_layer.filter(|layer| Some(*layer) != layer_before).map(|layer| layer.0));
        } else {
            result["tasks"] = json!(tasks);
            result["note"] = json!("The strings and surface are added together once the surface is built: get_tasks says what was made.");
        }
        ok(&result)
    }
}

/// Why there is no design, said so the AI can put it right.
fn no_design_message(reason: &NoDesign) -> String {
    match reason {
        NoDesign::NoString | NoDesign::NotClosed => "The string must be a closed polyline round some area.".to_owned(),
        NoDesign::NoTopography => "daylight needs a topography surface id.".to_owned(),
        NoDesign::TopographyNotLoaded(name) => format!("The topography {name} is not loaded. Load it with set_loaded first."),
        NoDesign::Settings(_) => crate::ui::dialogs::pit_design::no_design_text(reason),
    }
}
