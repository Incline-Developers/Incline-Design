//! Pit Design through MCP: bench design objects, read, made from closed
//! polylines, changed and exploded as the Pit Design tool
//! does, and `design_benches`, which makes one in one call.

use serde_json::{Value, json};

use super::{failed, merge_patch, ok};
use crate::{
    app::{App, commands::pit_design::pit_parts},
    model::{
        Object, ObjectId,
        pit_design::{
            Design, End,
            checks::Check,
            evaluation::Evaluation,
            object::{Outcome, PitObject},
            settings::{PitDesignSettings, RampSettings, StopMode},
        },
        triangulation::TriangulationId,
    },
    ui::dialogs::pit_design::{NoDesign, end_text, no_design_text, problem_text},
};

const SETTINGS_DESCRIPTION: &str = "A bench design's settings: kind (pit, cut into the ground, or dump, a dump or stockpile built on it); direction (a pit up from its floor's toe or down from its crest, a dump up from its toe stepping in or down from its top stepping out); bench_height; face_angle (degrees from horizontal); berm_width; bench_rules (height_cycle, a list of heights the benches take in turn; catch_every and catch_width, a berm that wide every that many benches; zones, bands of RL from..to whose benches take their own height, face_angle, berm_width and wall_radius, a bench taking the zone it starts in and the last listed winning; sectors, each a bearing the walls face (dip direction, degrees from north) with its face_angle, berm_width and optional inter_ramp target, interpolated round the compass; regions, each an outline ([x, y] points) whose walls take its face_angle and berm_width, blending over blend metres across its edge; inter_ramp, a target to check the walls against; a region counts over a zone, a zone over a sector, a sector over the design); wall_radius (the tightest the walls turn); stop (benches, elevation or daylight) with benches, stop_elevation, or topography (a surface id: a pit built up daylights against it, a dump built down meets it; any design is measured against it); rehab (a dump's rehabilitation, or null: slope, its overall angle in degrees; top, the highest RL its top may stand at, or null for the dump's own; contour, how far apart its contours are drawn; the dump is reshaped, holding its volume, into one even slope down to the topography, and get_bench_designs reports its top, volume, cut and fill and footprints); evaluation (block_model, the id of a block model the design's ground is measured in by the project's reserve fields, and shell, the id of an optimisation shell surface a pit is set against); ramps, a list (empty for none), each with width, gradient (a percentage: 10 is 1 in 10, held along the road's steeper edge), start ([x, y] near where it leaves the string, or null for the string's first point), climbs_clockwise, inside_radius (the tightest the road's inside edge should turn, checked all along it), switchback (true turns back every switchback_every benches, or at 0 only at turns_at RLs, round inside_radius, turn_widening wider round the turn, climbing at turn_grade round it or level at 0), from and to (RLs it begins and ends at, the way the design is built; null for the whole pit), exit_flat (a level run where it leaves the pit), junction_flat (a level run where each stretch leaves its berm), bands (bands of RL from..to whose stretches take their own width and/or a flat before they climb; a change of width tapers over five times the change); two ramps on one bench keep a road's width apart. Its surface is made with triangulate, which takes a bench design as every string it draws.";

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_bench_designs",
            "title": "Read the bench designs",
            "description": "Read the bench designs in the project's loaded layers. A bench design is a design object: its own vertices are the closed string it is designed from, and it draws as that string and the benches and ramp designed from it, worked out again whenever its string or settings change (fresh says whether that has caught up). Without design: a summary of each. With design (its object id): that one whole, its settings and what they come to included, which set_bench_design takes back, and its evaluation: the volume between it and its topography in all (and bench by bench with bench_by_bench), and with settings.evaluation.block_model, each reserve field's figure in all, by category (and by bench), and with settings.evaluation.shell, what it takes beyond the shell (gained) and what of it it leaves (left) (current says whether that is what the design comes to now, or waiting why not), and its daylight line, where it meets the topography (drawn with the design, and part of what explode_bench_design gives). What it comes to lists its checks: each place it falls short of its settings (the ramp steeper, flatter or narrower than asked or turning tighter than inside_radius, a berm narrower, a bench face steeper, or walls steeper between ramps than their target), at its worst point, with the bench (its place in bench_report) and ramp it is on, its value, what was asked and how far along it runs. Its ramps are always roads in the haul network, each along its centreline and joined to the network's nodes and roads it comes within the join tolerance of, made again as the design changes and gone with it. It is built until it meets its topography where it can (a pit built up from its floor, or a dump down from its top, with settings.topography and settings.stop daylight); otherwise to settings.benches, or settings.stop_elevation. Once it is designed and measured it settles: it lets go of its topography, a design that daylit keeping the bench count it reached as settings.benches (stop benches), and its evaluation and daylight line staying as they were measured. Changing it after that designs it to that count, with no topography needed; give daylight again to design it against the ground once more.",
            "inputSchema": {
                "type": "object",
                "properties": { "design": { "type": "integer", "minimum": 0 }, "bench_by_bench": bench_by_bench_schema() },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "set_bench_design",
            "title": "Make or change a bench design",
            "description": format!("Make a closed polyline a bench design, in its place and keeping its id (string), or change a bench design (design). settings is a JSON merge patch over its settings: give only what changes. A new design starts from the Pit Design tool's defaults, with no ramp. The design is worked out at once: the result says what it comes to. Its string is edited like any polyline's vertices (move_objects and the vertex tools), and the design follows. {SETTINGS_DESCRIPTION}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "design": { "type": "integer", "minimum": 0, "description": "A bench design's object id." },
                    "string": { "type": "integer", "minimum": 0, "description": "A closed polyline to make a bench design: the floor's toe building up, the crest building down." },
                    "name": { "type": "string", "minLength": 1 },
                    "settings": { "type": "object" },
                    "bench_by_bench": bench_by_bench_schema(),
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "explode_bench_design",
            "title": "Explode a bench design",
            "description": "Turn a bench design into polylines on its layer, as the Pit Design tool's Explode does: its string keeps the object's id, and each toe, crest and ramp string becomes a polyline of its own. They are no longer designed. A bench design is deleted with delete_objects and copied with copy_objects, like any object. Returns the polylines' ids.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "design": { "type": "integer", "minimum": 0 },
                },
                "required": ["design"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false },
        }),
        json!({
            "name": "design_benches",
            "title": "Design a pit in one call",
            "description": "Make a pit or dump design from a closed polyline, in its place, in one call: set_bench_design with string and these settings. A pit up from a floor's toe (each bench a face at face_angle rising bench_height, then a berm berm_width wide) or down from a crest. Exactly one of benches, stop_rl or daylight says where it stops. The ramp climbs at gradient along the steeper edge of the road, starting where the string passes nearest start, as a spiral round the walls or, with switchback_every, turning back on a level pad. Every ring is kept sound: where the next bench has no room, the design stops at the last good one and says why. With preview it only says what the design comes to, and makes nothing. Returns the design's id, which the other bench design tools take.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "string": { "type": "integer", "minimum": 0, "description": "The closed polyline to design from: the floor's toe building up, the crest building down." },
                    "name": { "type": "string", "minLength": 1 },
                    "kind": { "type": "string", "enum": ["pit", "dump"], "description": "A pit cut into the ground, or a dump or stockpile built on it. pit if left out." },
                    "direction": { "type": "string", "enum": ["up", "down"], "description": "A pit up from its floor or down from its crest; a dump up from its toe or down from its top. up if left out." },
                    "bench_height": { "type": "number", "exclusiveMinimum": 0 },
                    "face_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90, "description": "Degrees from horizontal." },
                    "berm_width": { "type": "number", "minimum": 0 },
                    "wall_radius": { "type": "number", "minimum": 0, "description": "The tightest the walls turn: sharp corners are rounded to it. 0 if left out." },
                    "bench_rules": {
                        "type": "object",
                        "description": "How the benches vary. None if left out.",
                        "properties": {
                            "height_cycle": { "type": "array", "items": { "type": "number", "exclusiveMinimum": 0 }, "description": "Heights the benches take in turn from the string, over and over, in place of bench_height." },
                            "catch_every": { "type": "integer", "minimum": 0, "description": "Every this many benches, the berm is catch_width wide instead. 0 for none." },
                            "catch_width": { "type": "number", "minimum": 0, "description": "0 leaves no berm there." },
                            "sectors": {
                                "type": "array",
                                "description": "The walls cut by the way they face, interpolated round the compass between sectors. They replace face_angle and berm_width; zones and regions count over them.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "bearing": { "type": "number", "description": "The bearing the walls face, as their dip direction, in degrees from north: the north wall faces 180." },
                                        "face_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90 },
                                        "berm_width": { "type": "number", "minimum": 0 },
                                        "inter_ramp": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90, "description": "The inter-ramp angle these walls should come to." },
                                    },
                                    "required": ["bearing", "face_angle", "berm_width"],
                                    "additionalProperties": false,
                                },
                            },
                            "regions": {
                                "type": "array",
                                "description": "Areas whose walls are cut otherwise, over everything else.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "outline": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "minItems": 3, "description": "[x, y] points, closed." },
                                        "face_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90 },
                                        "berm_width": { "type": "number", "minimum": 0 },
                                    },
                                    "required": ["outline"],
                                    "additionalProperties": false,
                                },
                            },
                            "blend": { "type": "number", "minimum": 0, "description": "How far across a region's edge its values blend in. 20 if left out." },
                            "inter_ramp": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90, "description": "The inter-ramp angle the walls should come to, where a sector sets none." },
                            "zones": {
                                "type": "array",
                                "description": "Bands of RL whose benches are cut otherwise. A bench takes the zone it starts in; where zones overlap, the last listed counts.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "from": { "type": "number" },
                                        "to": { "type": "number" },
                                        "height": { "type": "number", "exclusiveMinimum": 0 },
                                        "face_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90 },
                                        "berm_width": { "type": "number", "minimum": 0 },
                                        "wall_radius": { "type": "number", "minimum": 0, "description": "The tightest its walls turn." },
                                    },
                                    "required": ["from", "to"],
                                    "additionalProperties": false,
                                },
                            },
                        },
                        "additionalProperties": false,
                    },
                    "benches": { "type": "integer", "minimum": 1, "description": "Stop after this many benches." },
                    "stop_rl": { "type": "number", "description": "Stop at this RL, the last bench cut short to meet it." },
                    "daylight": { "type": "integer", "minimum": 0, "description": "A loaded topography surface: a pit built up until a whole bench stands clear of it, or a dump built down until a whole lift meets it." },
                    "topography": { "type": "integer", "minimum": 0, "description": "A surface the design is measured against, where it does not daylight against one." },
                    "block_model": { "type": "integer", "minimum": 0, "description": "A block model the design's ground is measured in, by the project's reserve fields: get_bench_designs then reports it in all, by category and bench by bench." },
                    "shell": { "type": "integer", "minimum": 0, "description": "An optimisation shell surface (a closed solid, or its floor and walls) the pit is set against: get_bench_designs then reports what it takes beyond the shell and what of it it leaves." },
                    "ramp": {
                        "type": "object",
                        "description": "The haul ramp. None if left out.",
                        "properties": {
                            "width": { "type": "number", "exclusiveMinimum": 0 },
                            "gradient": { "type": "number", "exclusiveMinimum": 0, "description": "Rise over run as a percentage: 10 is 1 in 10." },
                            "start": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] near where it leaves the string. The string's first point if left out." },
                            "clockwise": { "type": "boolean", "description": "Climb clockwise seen from above. False if left out." },
                            "switchback_every": { "type": "integer", "minimum": 1, "description": "Turn back every this many benches. A spiral if left out." },
                            "inside_radius": { "type": "number", "minimum": 0, "description": "The tightest the road's inside edge should turn: what a switchback turns round, and what the whole road is checked against. 15 if left out." },
                            "turns_at": { "type": "array", "items": { "type": "number" }, "description": "RLs it switches back at: on the first bench starting at or past each, or the next that has room. With or without switchback_every." },
                            "turn_widening": { "type": "number", "minimum": 0, "description": "How much wider than its width the road is round the middle of a switchback. 0 if left out." },
                            "turn_grade": { "type": "number", "minimum": 0, "description": "The grade, as a percentage, a switchback's inside edge climbs at round the turn, the stretch after it climbing what is left: usually 2 to 3 less than gradient. 0 (a level turn on a pad) if left out." },
                            "from": { "type": "number", "description": "The RL it begins at, the way the design is built: on the benches starting from it. The string if left out." },
                            "to": { "type": "number", "description": "The RL it ends at: the benches starting beyond it have no ramp. The last bench if left out." },
                            "exit_flat": { "type": "number", "minimum": 0, "description": "How far it runs level where it leaves the pit: at its top building up (where its last bench is known), at its first bench building down. 0 if left out." },
                            "junction_flat": { "type": "number", "minimum": 0, "description": "How far each stretch runs level where it leaves its bench's berm, so the berm is reached on the flat. 0 if left out." },
                            "bands": {
                                "type": "array",
                                "description": "Bands of RL whose stretches of road are laid otherwise: narrower (a one-lane bottom), or running level at least so far before they climb. A change of width tapers over five times the change.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "from": { "type": "number" },
                                        "to": { "type": "number" },
                                        "width": { "type": "number", "exclusiveMinimum": 0 },
                                        "flat": { "type": "number", "minimum": 0 },
                                    },
                                    "required": ["from", "to"],
                                    "additionalProperties": false,
                                },
                            },
                        },
                        "required": ["width", "gradient"],
                        "additionalProperties": false,
                    },
                    "ramps": { "type": "array", "description": "Several haul ramps, each as ramp describes, in place of ramp: on a bench with two, they keep a road's width apart.", "items": {
                        "type": "object",
                        "description": "The haul ramp. None if left out.",
                        "properties": {
                            "width": { "type": "number", "exclusiveMinimum": 0 },
                            "gradient": { "type": "number", "exclusiveMinimum": 0, "description": "Rise over run as a percentage: 10 is 1 in 10." },
                            "start": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] near where it leaves the string. The string's first point if left out." },
                            "clockwise": { "type": "boolean", "description": "Climb clockwise seen from above. False if left out." },
                            "switchback_every": { "type": "integer", "minimum": 1, "description": "Turn back every this many benches. A spiral if left out." },
                            "inside_radius": { "type": "number", "minimum": 0, "description": "The tightest the road's inside edge should turn: what a switchback turns round, and what the whole road is checked against. 15 if left out." },
                            "turns_at": { "type": "array", "items": { "type": "number" }, "description": "RLs it switches back at: on the first bench starting at or past each, or the next that has room. With or without switchback_every." },
                            "turn_widening": { "type": "number", "minimum": 0, "description": "How much wider than its width the road is round the middle of a switchback. 0 if left out." },
                            "turn_grade": { "type": "number", "minimum": 0, "description": "The grade, as a percentage, a switchback's inside edge climbs at round the turn, the stretch after it climbing what is left: usually 2 to 3 less than gradient. 0 (a level turn on a pad) if left out." },
                            "from": { "type": "number", "description": "The RL it begins at, the way the design is built: on the benches starting from it. The string if left out." },
                            "to": { "type": "number", "description": "The RL it ends at: the benches starting beyond it have no ramp. The last bench if left out." },
                            "exit_flat": { "type": "number", "minimum": 0, "description": "How far it runs level where it leaves the pit: at its top building up (where its last bench is known), at its first bench building down. 0 if left out." },
                            "junction_flat": { "type": "number", "minimum": 0, "description": "How far each stretch runs level where it leaves its bench's berm, so the berm is reached on the flat. 0 if left out." },
                            "bands": {
                                "type": "array",
                                "description": "Bands of RL whose stretches of road are laid otherwise: narrower (a one-lane bottom), or running level at least so far before they climb. A change of width tapers over five times the change.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "from": { "type": "number" },
                                        "to": { "type": "number" },
                                        "width": { "type": "number", "exclusiveMinimum": 0 },
                                        "flat": { "type": "number", "minimum": 0 },
                                    },
                                    "required": ["from", "to"],
                                    "additionalProperties": false,
                                },
                            },
                        },
                        "required": ["width", "gradient"],
                        "additionalProperties": false,
                    } },
                    "preview": { "type": "boolean", "description": "Only say what the design comes to; make nothing. False if left out." },
                    "bench_by_bench": bench_by_bench_schema(),
                },
                "required": ["string", "bench_height", "face_angle", "berm_width"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
    ]
}

/// The settings `design_benches`'s arguments give.
fn one_call_settings(arguments: &Value) -> Result<PitDesignSettings, String> {
    use crate::model::pit_design::Direction;
    let number = |key: &str| arguments[key].as_f64().ok_or_else(|| format!("design_benches needs {key}."));
    let mut settings = PitDesignSettings {
        kind: match arguments["kind"].as_str() {
            None | Some("pit") => crate::model::pit_design::Kind::Pit,
            Some("dump") => crate::model::pit_design::Kind::Dump,
            Some(other) => return Err(format!("kind is pit or dump, not {other}.")),
        },
        direction: match arguments["direction"].as_str() {
            None | Some("up") => Direction::Up,
            Some("down") => Direction::Down,
            Some(other) => return Err(format!("direction is up or down, not {other}.")),
        },
        bench_height: number("bench_height")?,
        face_angle: number("face_angle")?,
        berm_width: number("berm_width")?,
        wall_radius: arguments["wall_radius"].as_f64().unwrap_or(0.0),
        bench_rules: match &arguments["bench_rules"] {
            Value::Null => crate::model::pit_design::BenchRules::default(),
            rules => serde_json::from_value(rules.clone()).map_err(|error| format!("bench_rules: {error}"))?,
        },
        ramps: Vec::new(),
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
    if let Some(surface) = arguments["topography"].as_u64() {
        settings.topography = settings.topography.or(Some(TriangulationId(surface)));
    }
    settings.evaluation.block_model = arguments["block_model"].as_u64().map(crate::model::block_model::BlockModelId);
    settings.evaluation.shell = arguments["shell"].as_u64().map(TriangulationId);
    settings.ramps = match (&arguments["ramp"], &arguments["ramps"]) {
        (Value::Null, Value::Null) => Vec::new(),
        (ramp, Value::Null) => vec![ramp_settings(ramp)?],
        (Value::Null, Value::Array(ramps)) => ramps.iter().map(ramp_settings).collect::<Result<_, _>>()?,
        _ => return Err("Give ramp or ramps, not both; ramps is a list.".to_owned()),
    };
    Ok(settings)
}

/// One ramp's settings, as `design_benches` gives them.
fn ramp_settings(ramp: &Value) -> Result<RampSettings, String> {
    let mut every = None;
    if let Some(value) = ramp["switchback_every"].as_u64() {
        every = Some(u32::try_from(value).unwrap_or(u32::MAX));
    }
    let turns_at: Vec<f64> = match &ramp["turns_at"] {
        Value::Null => Vec::new(),
        rls => serde_json::from_value(rls.clone()).map_err(|error| format!("turns_at: {error}"))?,
    };
    Ok(RampSettings {
        width: ramp["width"].as_f64().ok_or("A ramp needs width.")?,
        gradient: ramp["gradient"].as_f64().ok_or("A ramp needs gradient, as a percentage.")?,
        start: match (ramp["start"][0].as_f64(), ramp["start"][1].as_f64()) {
            (Some(x), Some(y)) => Some(glam::DVec2::new(x, y)),
            _ if ramp["start"].is_null() => None,
            _ => return Err("A ramp's start is [x, y].".to_owned()),
        },
        climbs_clockwise: ramp["clockwise"].as_bool().unwrap_or(false),
        switchback: every.is_some() || !turns_at.is_empty(),
        switchback_every: every.unwrap_or(0),
        turns_at,
        inside_radius: ramp["inside_radius"].as_f64().unwrap_or(15.0),
        turn_widening: ramp["turn_widening"].as_f64().unwrap_or(0.0),
        turn_grade: ramp["turn_grade"].as_f64().unwrap_or(0.0),
        from: ramp["from"].as_f64(),
        to: ramp["to"].as_f64(),
        bands: match &ramp["bands"] {
            Value::Null => Vec::new(),
            bands => serde_json::from_value(bands.clone()).map_err(|error| format!("ramp bands: {error}"))?,
        },
        exit_flat: ramp["exit_flat"].as_f64().unwrap_or(0.0),
        junction_flat: ramp["junction_flat"].as_f64().unwrap_or(0.0),
    })
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

/// What a check is called in what MCP reports: what falls short, with its
/// figures in the units it says.
fn check_name(check: Check) -> &'static str {
    match check {
        Check::Steep => "ramp_steeper_percent",
        Check::Flatter => "ramp_flatter_percent",
        Check::Narrow => "ramp_narrower_m",
        Check::Tight => "ramp_turns_tighter_m",
        Check::Berm => "berm_narrower_m",
        Check::Face => "face_steeper_degrees",
        Check::InterRamp => "walls_steeper_degrees",
    }
}

/// What a design comes to: bench by bench too with `benches`, which is
/// most of it, so an AI iterating on a design reads the totals unless it asks.
fn summary(design: &Design, benches: bool) -> Value {
    let round = |value: f64| (value * 100.0).round() / 100.0;
    let mut value = json!({
        "benches": design.benches,
        "ended": end_name(design.end),
        "ended_because": end_text(design),
        "rls": design.levels.iter().copied().map(round).collect::<Vec<_>>(),
        "final_rl": round(design.final_elevation()),
        "volume": design.volume.round(),
        "ramp_length": round(design.ramp_length),
        "ramp_exit": design.ramp_exit.map(|exit| [round(exit.x), round(exit.y), round(exit.z)]),
        "ramp_exits": design.ramp_exits.iter().map(|exit| exit.map(|exit| [round(exit.x), round(exit.y), round(exit.z)])).collect::<Vec<_>>(),
        "blocked_ramp": design.blocked_ramp,
        "ramp_start_moved": (design.ramp_start_moved > 0.0).then(|| round(design.ramp_start_moved)),
        "strings": design.strings.len(),
        "walls": design.walls.iter().map(|wall| json!({
            "facing": round(wall.bearing),
            "designed": round(wall.designed),
            "achieved": round(wall.achieved),
            "steepest": round(wall.steepest),
            "target": wall.target.map(round),
            "too_steep": wall.too_steep(),
            "samples": wall.samples,
        })).collect::<Vec<_>>(),
        "checks": design.flags.iter().map(|flag| {
            // A grade as a percentage, as the settings give it.
            let figure = |value: f64| match flag.check {
                Check::Steep | Check::Flatter => round(value * 100.0),
                _ => round(value),
            };
            json!({
                "check": check_name(flag.check),
                "at": [round(flag.at.x), round(flag.at.y), round(flag.at.z)],
                "bench": flag.bench,
                "ramp": flag.ramp,
                "value": figure(flag.value),
                "asked": figure(flag.asked),
                "length": round(flag.length),
            })
        }).collect::<Vec<_>>(),
    });
    if !benches {
        return value;
    }
    value["bench_report"] = design
        .report
        .iter()
        .map(|bench| {
            let mut row = json!({
                "rl": round(bench.elevation),
                "height": round(bench.height),
                "face_angle": span(bench.face_angle),
                "berm": span(bench.berm),
                "area": bench.area.round(),
            });
            let roads: Vec<Value> = bench
                .ramps
                .iter()
                .map(|road| {
                    json!({
                        "ramp": road.ramp,
                        "grade_percent": round(road.steepest * 100.0),
                        "flat": round(road.flat),
                        "lengthened": round(road.lengthened),
                        "narrowest": round(road.narrowest),
                        "switchback": road.switchback,
                        "turn_moved_from": road.turn_moved_from.map(round),
                        "turn_rise": round(road.turn_rise),
                    })
                })
                .collect();
            if let Some(first) = roads.first() {
                row["ramp"] = first.clone();
            }
            if roads.len() > 1 {
                row["ramps"] = Value::Array(roads);
            }
            row
        })
        .collect::<Vec<_>>()
        .into();
    value
}

/// Whether a call asks for its design bench by bench, as well as in all.
fn bench_by_bench(arguments: &Value) -> bool {
    arguments["bench_by_bench"].as_bool().unwrap_or(false)
}

/// The schema of the argument that asks for a design bench by bench.
fn bench_by_bench_schema() -> Value {
    json!({ "type": "boolean", "description": "Give the design bench by bench too: bench_report, and the evaluation's benches. False if left out: the totals, checks and walls only." })
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
fn comes_to(design: &Result<Design, NoDesign>, benches: bool) -> Value {
    match design {
        Ok(design) => summary(design, benches),
        Err(reason) => json!({ "problem": no_design_message(reason) }),
    }
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_pit_design_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "get_bench_designs" => self.mcp_get_pit_designs(arguments),
            "set_bench_design" => self.mcp_set_pit_design(arguments),
            "explode_bench_design" => self.mcp_explode_pit_design(arguments),
            "design_benches" => self.mcp_design_pit(arguments),
            _ => return None,
        };
        Some(match result {
            Ok(value) => ok(&value),
            Err(message) => failed(message),
        })
    }

    /// The bench design object `arguments` names, and what it is designed with.
    fn mcp_pit_design(&self, arguments: &Value) -> Result<(ObjectId, PitObject), String> {
        let id = arguments["design"].as_u64().map(ObjectId).ok_or("Give design, a bench design's object id.")?;
        let object = self
            .workspace
            .active_document()
            .and_then(|document| document.get_object(id))
            .ok_or_else(|| format!("There is no design object with id {} in a loaded layer: get_bench_designs lists the bench designs.", id.0))?;
        let (_, pit) = pit_parts(object).ok_or_else(|| {
            format!(
                "Object {} is a {}, not a bench design: set_bench_design with string makes a closed polyline one.",
                id.0,
                object.kind_name()
            )
        })?;
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
            "fresh": pit.is_fresh(verts, ground),
        })
    }

    fn mcp_get_pit_designs(&self, arguments: &Value) -> Result<Value, String> {
        let benches = bench_by_bench(arguments);
        if arguments.get("design").is_some() {
            let (id, pit) = self.mcp_pit_design(arguments)?;
            let object = self.operand_object(id).ok_or("The design is gone.")?;
            let mut value = self.mcp_describe_pit(object);
            if let Some((verts, _)) = pit_parts(object) {
                value["string"] = json!(verts.iter().map(|vertex| [vertex.pos.x, vertex.pos.y, vertex.pos.z]).collect::<Vec<_>>());
            }
            value["settings"] = json!(pit.settings);
            value["comes_to"] = match pit.derived.as_ref().map(|derived| &derived.outcome) {
                Some(Outcome::Design(design)) => summary(design, benches),
                Some(Outcome::NotClosed) => json!({ "problem": no_design_message(&NoDesign::NotClosed) }),
                Some(Outcome::Problem(problem)) => json!({ "problem": problem_text(*problem) }),
                None => json!({ "problem": "Not worked out yet." }),
            };
            value["evaluation"] = self.mcp_evaluation(id, pit.evaluation.as_ref(), benches);
            if pit.design().is_some_and(|design| pit.settings.can_daylight() && design.end != End::Daylight) && pit.evaluation.is_some() {
                value["evaluation"]["note"] = json!(
                    "The design stops short of the topography: the ground above its last ring is measured up to the topography as though its walls stood straight up from there."
                );
            }
            return Ok(value);
        }
        let designs: Vec<Value> = self
            .workspace
            .active_document()
            .map(|document| {
                document
                    .objects()
                    .iter()
                    .filter(|object| pit_parts(object).is_some())
                    .map(|object| self.mcp_describe_pit(object))
                    .collect()
            })
            .unwrap_or_default();
        Ok(json!({ "designs": designs }))
    }

    /// What a design takes out of the ground, as last measured, with each
    /// reserve field by name; and whether that is what it comes to now.
    fn mcp_evaluation(&self, id: ObjectId, evaluation: Option<&Evaluation>, benches: bool) -> Value {
        use crate::model::pit_design::evaluation::{Figures, Measured, Waiting};
        let fields = self.workspace.active_document().map(|document| document.reserve_fields().to_vec()).unwrap_or_default();
        let name = |field: crate::model::ReserveFieldId| {
            fields
                .iter()
                .find(|entry| entry.id == field)
                .map_or_else(|| field.0.to_string(), |entry| entry.name.clone())
        };
        let round = |value: f64| (value * 1000.0).round() / 1000.0;
        let measured = |measured: &Measured| {
            let values = |values: &[crate::model::pit_design::evaluation::FieldValue]| -> serde_json::Map<String, Value> {
                values.iter().map(|value| (name(value.field), json!(value.value.map(round)))).collect()
            };
            json!({
                "in_block_model": round(measured.covered),
                "fields": values(&measured.values),
                "by_category": measured.categories.iter().map(|category| json!({
                    "field": name(category.field),
                    "groups": category.groups.iter().map(|group| json!({
                        "name": group.name,
                        "in_block_model": round(group.covered),
                        "fields": values(&group.values),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        };
        let figures = |figures: &Figures| {
            let mut value = json!({ "volume": round(figures.volume) });
            if let Some(found) = &figures.measured {
                value["measured"] = measured(found);
            }
            value
        };
        let waiting = match self.pit_evaluation_waiting(id) {
            None => Value::Null,
            Some(Waiting::Design) => json!("The design is being worked out."),
            Some(Waiting::NoTopography) => json!("No topography is chosen to measure it against: settings.topography."),
            Some(Waiting::NotLoaded(name)) => json!(format!("{name} must be loaded to measure it.")),
            Some(Waiting::Measuring) => json!("Being measured: what follows is from before."),
        };
        let Some(evaluation) = evaluation else {
            return json!({ "current": false, "waiting": waiting });
        };
        if let Some(problem) = &evaluation.problem {
            return json!({ "current": waiting.is_null(), "waiting": waiting, "problem": problem });
        }
        let shell = evaluation.shell.as_ref().map(|shell| match &shell.problem {
            Some(problem) => json!({ "problem": problem }),
            None => json!({ "gained": figures(&shell.gained), "left": figures(&shell.left) }),
        });
        // Each string of its daylight line: how long it runs, where it starts.
        let daylight: Vec<Value> = evaluation
            .daylight
            .iter()
            .map(|string| {
                let length: f64 = string.points.windows(2).map(|pair| pair[0].truncate().distance(pair[1].truncate())).sum();
                json!({
                    "closed": string.closed,
                    "points": string.points.len(),
                    "length": round(length),
                    "from": string.points.first().map(|point| [round(point.x), round(point.y), round(point.z)]),
                })
            })
            .collect();
        let rehab = evaluation.rehab.as_ref().map(|rehab| match &rehab.problem {
            Some(problem) => json!({ "problem": problem }),
            None => json!({
                "top": round(rehab.top),
                "volume": round(rehab.volume),
                "cut": round(rehab.cut),
                "fill": round(rehab.fill),
                "footprint_designed": round(rehab.footprint[0]),
                "footprint_reshaped": round(rehab.footprint[1]),
                "contours": rehab.strings.len(),
            }),
        });
        let mut value = json!({
            "current": waiting.is_null(),
            "waiting": waiting,
            "rehab": rehab,
            "daylight": daylight,
            "shell": shell,
            "total": figures(&evaluation.total),
        });
        if benches {
            value["benches"] = evaluation
                .benches
                .iter()
                .map(|bench| {
                    let mut value = figures(&bench.figures);
                    value["from"] = json!(round(bench.base));
                    value["to"] = json!(round(bench.top));
                    value
                })
                .collect::<Vec<_>>()
                .into();
        }
        value
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
            return Err(format!(
                "Object {} is a {}: a bench design is made from a closed polyline round some area.",
                id.0,
                object.kind_name()
            ));
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
            (true, Some(_)) => return Err("Give design to change a bench design, or string to make one, not both.".to_owned()),
            (false, None) => return Err("Give design, a bench design's id, or string, a closed polyline to make one.".to_owned()),
        };
        if let Some(name) = arguments["name"].as_str() {
            pit.name = name.to_owned();
        }
        if let Some(patch) = arguments.get("settings") {
            let mut value = serde_json::to_value(&pit.settings).map_err(|error| error.to_string())?;
            merge_patch(&mut value, patch);
            pit.settings = serde_json::from_value(value).map_err(|error| format!("The settings do not fit a bench design: {error}"))?;
        }
        let design = self.set_pit_now(id, pit).map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "design": id.0, "comes_to": comes_to(&design, bench_by_bench(arguments)) }))
    }

    fn mcp_explode_pit_design(&mut self, arguments: &Value) -> Result<Value, String> {
        let (id, _) = self.mcp_pit_design(arguments)?;
        let ids = self.explode_pit_design(id).map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "polylines": ids.iter().map(|id| id.0).collect::<Vec<_>>() }))
    }

    fn mcp_design_pit(&mut self, arguments: &Value) -> Result<Value, String> {
        let settings = one_call_settings(arguments)?;
        let string = self.mcp_closed_polyline(arguments)?.ok_or("design_benches needs string, a closed polyline's id.")?;
        let verts = match self.operand_object(string) {
            Some(Object::Polyline { verts, .. }) => verts.clone(),
            _ => return Err("design_benches needs string, a closed polyline's id.".to_owned()),
        };
        let design = self.design_pit_now(&verts, &settings).and_then(|preview| preview.design);
        let built = match &design {
            Ok(built) => built,
            Err(reason) => return Err(no_design_message(reason)),
        };
        if arguments["preview"].as_bool().unwrap_or(false) {
            return Ok(summary(built, bench_by_bench(arguments)));
        }
        if built.benches == 0 {
            return Err(format!("Not even one bench fits: {}", end_text(built)));
        }
        self.make_pit_design(string, arguments["name"].as_str().map(str::to_owned), Some(settings))
            .map_err(|error| format!("{error:#}"))?;
        Ok(json!({ "design": string.0, "comes_to": comes_to(&design, bench_by_bench(arguments)) }))
    }
}

/// A value that may vary round the pit: one number where it does not, its
/// least and greatest where it does.
fn span([low, high]: [f64; 2]) -> Value {
    let round = |value: f64| (value * 100.0).round() / 100.0;
    if (high - low).abs() < 0.005 {
        json!(round(low))
    } else {
        json!([round(low), round(high)])
    }
}
