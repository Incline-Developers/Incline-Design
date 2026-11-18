//! The tools that change design objects already drawn: their style and
//! place, their geometry, and the drawing tools that make new objects from
//! old ones (offset, batter and berm, explode, points at crossings).

use glam::DVec3;
use serde_json::{Value, json};

use super::{edit::build_object, failed, ok, parse_hex, parse_point, read::fill_name};
use crate::{
    app::App,
    i18n::tr,
    logging::CommandReportSpec,
    model::{Axis, Command, FillStyle, LayerId, Object, ObjectColor, ObjectId, PolyVertex, SceneEntityId, geometry},
    ui::state::{BatterBermMode, UiCommand},
};

/// The schema of a list of object ids.
fn ids() -> Value {
    json!({ "type": "array", "minItems": 1, "items": { "type": "integer", "minimum": 0 }, "description": "Design object ids." })
}

/// A tool that changes the project, as one undo step.
fn changes(name: &str, title: &str, description: &str, properties: Value, required: &[&str], idempotent: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": format!("{description} One undo step."),
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": idempotent },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    vec![
        changes(
            "edit_objects",
            "Edit design objects",
            "Change the style of design objects, or set one coordinate of all their vertices, as the properties panel and Design > Move to do. Only what is given changes. closed, fill and line_weight apply to the polylines and circles among them.",
            json!({
                "ids": ids(),
                "colour": { "type": "string", "description": "#rrggbb, or by_layer." },
                "closed": { "type": "boolean" },
                "fill": { "type": "string", "enum": FillStyle::ALL.map(fill_name) },
                "line_weight": { "type": "number", "minimum": 0 },
                "x": { "type": "number", "description": "Set every vertex's x to this." },
                "y": { "type": "number", "description": "Set every vertex's y to this." },
                "z": { "type": "number", "description": "Set every vertex's z (RL) to this." },
            }),
            &["ids"],
            true,
        ),
        changes(
            "move_objects",
            "Move or copy design objects",
            "Move design objects by an offset, to another design layer, or both. With copy, the originals stay and moved copies are made; their ids are returned.",
            json!({
                "ids": ids(),
                "offset": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[dx, dy, dz] to move by." },
                "layer": { "type": "integer", "minimum": 0, "description": "A design layer to move them to." },
                "copy": { "type": "boolean", "description": "Leave the originals and move copies. False if left out." },
            }),
            &["ids"],
            false,
        ),
        changes(
            "replace_object",
            "Replace a design object's geometry",
            "Redraw one design object in place, keeping its id and layer: give it whole, as add_objects takes one (its type may change). This is how to move vertices, add or remove them, trim, extend or join lines, or change text.",
            json!({
                "id": { "type": "integer", "minimum": 0 },
                "object": { "type": "object", "description": "The object, as one item of add_objects' objects." },
            }),
            &["id", "object"],
            true,
        ),
        changes(
            "set_objects_hidden",
            "Hide or show design objects",
            "Hide design objects in the viewport without deleting them, or show them again, as Hide Selection does.",
            json!({ "ids": ids(), "hidden": { "type": "boolean" } }),
            &["ids", "hidden"],
            true,
        ),
        changes(
            "offset_objects",
            "Offset design objects",
            "Draw an offset copy of each polyline or circle, as the Offset tool does: a parallel line distance away on one side, raised by z_change. Or, with batter_angle and to_rl, a batter projected down or up at that angle until it reaches the RL. Returns the new objects' ids.",
            json!({
                "ids": ids(),
                "side": { "type": "string", "enum": ["outside", "inside", "left", "right"], "description": "For closed polylines and circles: outside or inside. For open polylines: left or right of the direction they are drawn in." },
                "distance": { "type": "number", "exclusiveMinimum": 0, "description": "Horizontal offset distance." },
                "z_change": { "type": "number", "description": "Raise (or, below 0, lower) the offset by this. 0 if left out." },
                "batter_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90, "description": "Batter angle from horizontal in degrees, with to_rl, in place of distance." },
                "to_rl": { "type": "number", "description": "The RL a batter is projected to." },
            }),
            &["ids", "side"],
            false,
        ),
        changes(
            "batter_berm",
            "Bench a pit or stockpile outline",
            "Bench out from a toe or crest outline, as Batter and Berm does: each bench a batter at batter_angle rising (or falling) bench_height, then a flat berm berm_width wide. pit benches outwards and up from a pit floor (or inwards and down from a crest, with downward); stockpile benches inwards and up from a toe (or outwards and down from a crest, with downward). Returns the new outlines' ids.",
            json!({
                "id": { "type": "integer", "minimum": 0, "description": "A polyline or circle." },
                "kind": { "type": "string", "enum": ["pit", "stockpile"] },
                "benches": { "type": "integer", "minimum": 1 },
                "bench_height": { "type": "number", "exclusiveMinimum": 0 },
                "batter_angle": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 90, "description": "Degrees from horizontal." },
                "berm_width": { "type": "number", "exclusiveMinimum": 0 },
                "downward": { "type": "boolean", "description": "Bench downwards instead. False if left out." },
            }),
            &["id", "kind", "benches", "bench_height", "batter_angle", "berm_width"],
            false,
        ),
        changes(
            "explode_polylines",
            "Explode polylines",
            "Break each polyline into its separate two-vertex lines, as Explode does.",
            json!({ "ids": ids() }),
            &["ids"],
            false,
        ),
        changes(
            "insert_points",
            "Insert vertices",
            "Add vertices to polylines without changing their shape: where they cross each other seen from above (at least two polylines), or where they pass through an RL.",
            json!({
                "ids": ids(),
                "at_elevation": { "type": "number", "description": "Insert where each polyline passes this RL. Where they cross each other if left out." },
            }),
            &["ids"],
            true,
        ),
        changes(
            "drape_objects",
            "Drape onto surfaces",
            "Lay design objects onto surfaces, as Drape to Topology does: each vertex (and each point and text) moves straight up or down to the uppermost of the surfaces at its x and y. Vertices outside every surface stay where they are, and circles are left alone. Returns how many objects and vertices changed.",
            json!({
                "ids": ids(),
                "surfaces": { "type": "array", "minItems": 1, "items": { "type": "integer", "minimum": 0 }, "description": "Surface ids. They must be loaded; set_visibility loads them." },
            }),
            &["ids", "surfaces"],
            true,
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_object_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        Some(match name {
            "edit_objects" => self.mcp_edit_objects(arguments),
            "move_objects" => self.mcp_move_objects(arguments),
            "replace_object" => self.mcp_replace_object(arguments),
            "set_objects_hidden" => self.mcp_set_objects_hidden(arguments),
            "offset_objects" => self.mcp_offset_objects(arguments),
            "batter_berm" => self.mcp_batter_berm(arguments),
            "explode_polylines" => self.mcp_explode(arguments),
            "insert_points" => self.mcp_insert_points(arguments),
            "drape_objects" => self.mcp_drape_objects(arguments),
            _ => return None,
        })
    }

    /// The objects `ids` names, each on an unlocked layer.
    fn mcp_editable_objects(&self, arguments: &Value, key: &str) -> Result<Vec<Object>, String> {
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let ids = arguments[key].as_array().ok_or_else(|| format!("{key} needs a list of design object ids."))?;
        ids.iter()
            .map(|id| {
                let id = id.as_u64().ok_or("Object ids are whole numbers.")?;
                let object = document
                    .get_object(ObjectId(id))
                    .ok_or_else(|| format!("There is no design object with id {id} in a loaded layer."))?;
                if self.editor.locked_layers.contains(&object.layer()) {
                    return Err(format!("Design object {id} is on a locked layer. The user can unlock it in the explorer."));
                }
                Ok(object.clone())
            })
            .collect()
    }

    /// Apply `commands` as one undo step, reported as `title` in the
    /// activity log.
    fn mcp_apply(&mut self, commands: Vec<Command>, title: String, count: usize) {
        if commands.is_empty() {
            return;
        }
        self.execute_job_edit(Command::Batch(commands));
        self.invalidate_geometry();
        self.invalidate_overlay();
        crate::logging::report_completed_action(
            CommandReportSpec::new(title.clone(), tr!("common-count-object-s", count = count.to_string())),
            tr!("mcp-changed-objects", title = title, count = count.to_string()),
        );
    }

    /// Add `objects` with fresh ids, returning the ids.
    fn mcp_add_new(&mut self, objects: Vec<Object>, title: String) -> Vec<u64> {
        let Some(project) = self.workspace.active_project_mut() else {
            return Vec::new();
        };
        let document = &mut project.project.document;
        let mut ids = Vec::with_capacity(objects.len());
        let commands: Vec<Command> = objects
            .into_iter()
            .map(|object| {
                let id = document.allocate_object_id();
                ids.push(id.0);
                let layer = object.layer();
                Command::AddObject(object.with_id_and_layer(id, layer))
            })
            .collect();
        let count = commands.len();
        self.mcp_apply(commands, title, count);
        ids
    }

    fn mcp_edit_objects(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let colour = match arguments["colour"].as_str() {
            None => None,
            Some("by_layer") => Some(ObjectColor::ByLayer),
            Some(text) => match parse_hex(text) {
                Some(rgba) => Some(ObjectColor::Fixed(rgba)),
                None => return failed(format!("{text} is not a colour: give #rrggbb or by_layer.")),
            },
        };
        let fill = match arguments["fill"].as_str() {
            None => None,
            Some(name) => match FillStyle::ALL.into_iter().find(|fill| fill_name(*fill) == name) {
                Some(fill) => Some(fill),
                None => return failed(format!("{name} is not a fill.")),
            },
        };
        let closed = arguments["closed"].as_bool();
        let weight = arguments["line_weight"].as_f64().map(|weight| weight as f32);
        let axes: Vec<(Axis, f64)> = [(Axis::X, "x"), (Axis::Y, "y"), (Axis::Z, "z")]
            .into_iter()
            .filter_map(|(axis, key)| arguments[key].as_f64().map(|value| (axis, value)))
            .collect();
        let mut commands = Vec::new();
        for before in objects {
            let mut after = before.clone();
            for (axis, value) in &axes {
                after.set_axis_position(*axis, *value);
            }
            match &mut after {
                Object::Point { color, .. } | Object::Text { color, .. } => {
                    if let Some(colour) = colour {
                        *color = colour;
                    }
                }
                Object::Polyline {
                    color,
                    fill: object_fill,
                    line_weight,
                    closed: object_closed,
                    ..
                } => {
                    colour.inspect(|colour| *color = *colour);
                    fill.inspect(|fill| *object_fill = *fill);
                    weight.inspect(|weight| *line_weight = *weight);
                    closed.inspect(|closed| *object_closed = *closed);
                }
                Object::Circle {
                    color,
                    fill: object_fill,
                    line_weight,
                    ..
                } => {
                    colour.inspect(|colour| *color = *colour);
                    fill.inspect(|fill| *object_fill = *fill);
                    weight.inspect(|weight| *line_weight = *weight);
                }
            }
            if let Err(error) = after.validate_geometry() {
                return failed(format!("Design object {}: {error}", before.id().0));
            }
            if after != before {
                commands.push(Command::Replace { before, after });
            }
        }
        let count = commands.len();
        self.mcp_apply(commands, tr!("mcp-edit-objects"), count);
        ok(&json!({ "changed": count }))
    }

    fn mcp_move_objects(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let offset = match &arguments["offset"] {
            Value::Null => DVec3::ZERO,
            value => match parse_point(value) {
                Some(offset) => offset,
                None => return failed("offset must be [dx, dy, dz]."),
            },
        };
        let layer = arguments["layer"].as_u64().map(LayerId);
        if let Some(layer) = layer
            && let Err(error) = self.mcp_editable_layer(layer)
        {
            return failed(error);
        }
        let copy = arguments["copy"].as_bool().unwrap_or(false);
        if copy {
            let copies = objects
                .into_iter()
                .map(|mut object| {
                    object.translate(offset);
                    let layer = layer.unwrap_or(object.layer());
                    object.with_id_and_layer(object.id(), layer)
                })
                .collect();
            let ids = self.mcp_add_new(copies, tr!("mcp-copy-objects"));
            return ok(&json!({ "copies": ids }));
        }
        let commands: Vec<Command> = objects
            .into_iter()
            .filter_map(|before| {
                let mut after = before.clone();
                after.translate(offset);
                let after = after.with_id_and_layer(before.id(), layer.unwrap_or(before.layer()));
                (after != before).then_some(Command::Replace { before, after })
            })
            .collect();
        let count = commands.len();
        self.mcp_apply(commands, tr!("mcp-move-objects"), count);
        ok(&json!({ "moved": count }))
    }

    fn mcp_replace_object(&mut self, arguments: &Value) -> Value {
        let before = match self.mcp_editable_objects(&json!({ "ids": [arguments["id"]] }), "ids") {
            Ok(mut objects) => objects.remove(0),
            Err(error) => return failed(error),
        };
        let defaults = (before.color(), before.line_weight().unwrap_or(self.editor.tool_line_weight));
        let after = match build_object(&arguments["object"], before.layer(), defaults) {
            Ok(object) => object.with_id_and_layer(before.id(), before.layer()),
            Err(error) => return failed(error),
        };
        let id = before.id().0;
        self.mcp_apply(vec![Command::Replace { before, after }], tr!("mcp-replace-object"), 1);
        ok(&json!({ "replaced": id }))
    }

    fn mcp_set_objects_hidden(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let Some(hidden) = arguments["hidden"].as_bool() else {
            return failed("set_objects_hidden needs hidden.");
        };
        let Some(document) = self.workspace.active_document() else {
            return failed("No project is open.");
        };
        let commands: Vec<Command> = objects
            .iter()
            .filter(|object| document.is_object_hidden(object.id()) != hidden)
            .map(|object| Command::SetObjectHidden {
                id: object.id(),
                before: !hidden,
                after: hidden,
            })
            .collect();
        let count = commands.len();
        if hidden {
            self.editor
                .selected_handles
                .retain(|handle| !objects.iter().any(|object| *handle == SceneEntityId::Object(object.id())));
        }
        self.mcp_apply(commands, if hidden { tr!("common-hide-selection") } else { tr!("mcp-show-objects") }, count);
        ok(&json!({ "changed": count }))
    }

    fn mcp_offset_objects(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let side = match arguments["side"].as_str() {
            Some(side @ ("outside" | "inside" | "left" | "right")) => side,
            _ => return failed("side is outside, inside, left or right."),
        };
        let shift = match (arguments["distance"].as_f64(), arguments["batter_angle"].as_f64(), arguments["to_rl"].as_f64()) {
            (_, Some(angle), Some(rl)) if angle > 0.0 && angle < 90.0 => Shift::Batter {
                tan: angle.to_radians().tan(),
                rl,
            },
            (Some(distance), None, None) if distance > 0.0 => Shift::Flat {
                distance,
                z: arguments["z_change"].as_f64().unwrap_or(0.0),
            },
            _ => return failed("Give distance (above 0), or batter_angle (between 0 and 90) and to_rl."),
        };
        let mut made = Vec::new();
        for object in &objects {
            match offset(object, side, shift) {
                Ok(Some(new)) => made.push(new),
                Ok(None) => return failed(format!("The offset of design object {} collapses to nothing.", object.id().0)),
                Err(error) => return failed(format!("Design object {}: {error}", object.id().0)),
            }
        }
        let ids = self.mcp_add_new(made, tr!("cmd-offset-create-offset"));
        ok(&json!({ "ids": ids }))
    }

    fn mcp_batter_berm(&mut self, arguments: &Value) -> Value {
        let object = match self.mcp_editable_objects(&json!({ "ids": [arguments["id"]] }), "ids") {
            Ok(mut objects) => objects.remove(0),
            Err(error) => return failed(error),
        };
        if !matches!(object, Object::Polyline { .. } | Object::Circle { .. }) {
            return failed("Batter and berm works from a polyline or a circle.");
        }
        let mode = match arguments["kind"].as_str() {
            Some("pit") => BatterBermMode::Pit,
            Some("stockpile") => BatterBermMode::Stockpile,
            _ => return failed("kind is pit or stockpile."),
        };
        let (Some(benches), Some(height), Some(angle), Some(width)) = (
            arguments["benches"].as_u64(),
            arguments["bench_height"].as_f64(),
            arguments["batter_angle"].as_f64(),
            arguments["berm_width"].as_f64(),
        ) else {
            return failed("batter_berm needs benches, bench_height, batter_angle and berm_width.");
        };
        if benches == 0 || height <= 0.0 || width <= 0.0 || !(angle > 0.0 && angle < 90.0) {
            return failed("benches, bench_height and berm_width must be above 0, and batter_angle between 0 and 90.");
        }
        let before = self.workspace.active_document().map_or(0, |document| document.objects().len());
        // The dialog's own path: the same state its fields edit, its preview
        // and its Create button.
        let editor = &mut self.editor;
        editor.batter_berm_target_id = Some(object.id());
        editor.batter_berm_dialog_open = true;
        editor.batter_berm_preview_key = None;
        editor.batter_berm_mode = mode;
        editor.batter_berm_direction_up = !arguments["downward"].as_bool().unwrap_or(false);
        editor.batter_berm_benches = u32::try_from(benches).unwrap_or(u32::MAX);
        editor.batter_berm_bench_height = height;
        editor.batter_berm_angle = angle;
        editor.batter_berm_width = width;
        self.update_batter_berm_preview();
        let built = self.editor.batter_berm_benches;
        if self.editor.batter_berm_rings_world.is_empty() && self.editor.batter_berm_ring_circles.as_ref().is_none_or(Vec::is_empty) {
            self.cancel_batter_berm();
            return failed("No bench fits: the outline closes up before the first berm. Try a shorter bench or narrower berm.");
        }
        self.commit_batter_berm();
        let ids: Vec<u64> = self
            .workspace
            .active_document()
            .map(|document| document.objects()[before.min(document.objects().len())..].iter().map(|object| object.id().0).collect())
            .unwrap_or_default();
        ok(&json!({ "benches": built, "ids": ids }))
    }

    fn mcp_explode(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let mut exploded = 0;
        for object in objects.iter().filter(|object| matches!(object, Object::Polyline { .. })) {
            self.explode_polyline(object.id());
            exploded += 1;
        }
        ok(&json!({ "exploded": exploded }))
    }

    fn mcp_drape_objects(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let Some(surfaces) = arguments["surfaces"].as_array() else {
            return failed("drape_objects needs a list of surface ids.");
        };
        let mut surface_ids = Vec::with_capacity(surfaces.len());
        for id in surfaces {
            let Some(id) = id.as_u64() else {
                return failed("Surface ids are whole numbers.");
            };
            let Some(surface) = self.triangulations.iter().find(|surface| surface.id.0 == id) else {
                return failed(format!("There is no surface with id {id}. get_project lists them."));
            };
            if !surface.state.loaded || surface.state.deferred.is_some() {
                return failed(format!("The surface {} is not loaded. Load it with set_visibility, then ask again.", surface.name));
            }
            surface_ids.push(surface.id);
        }
        let object_ids: Vec<ObjectId> = objects.iter().map(Object::id).collect();
        match self.drape_objects(&object_ids, &surface_ids) {
            Some((objects, intersected, changed)) => {
                self.invalidate_geometry();
                ok(&json!({ "objects_changed": objects, "vertices_on_a_surface": intersected, "vertices_moved": changed }))
            }
            None => failed("None of those surfaces is loaded."),
        }
    }

    fn mcp_insert_points(&mut self, arguments: &Value) -> Value {
        let objects = match self.mcp_editable_objects(arguments, "ids") {
            Ok(objects) => objects,
            Err(error) => return failed(error),
        };
        let polylines: Vec<ObjectId> = objects.iter().filter(|object| matches!(object, Object::Polyline { .. })).map(Object::id).collect();
        let command = match arguments["at_elevation"].as_f64() {
            Some(elevation) => UiCommand::InsertPointsAtElevation { object_ids: polylines, elevation },
            None => {
                if polylines.len() < 2 {
                    return failed("Crossings need at least two polylines.");
                }
                UiCommand::InsertPointsAtCrossings { object_ids: polylines }
            }
        };
        match self.handle_reported_ui_command(command) {
            Ok(()) => ok(&json!({ "done": true })),
            Err(error) => failed(format!("{error:#}")),
        }
    }
}

/// How an offset moves a line.
#[derive(Clone, Copy)]
enum Shift {
    /// Out by `distance`, up by `z`.
    Flat { distance: f64, z: f64 },
    /// Out at a batter whose slope is `tan`, to the RL `rl`.
    Batter { tan: f64, rl: f64 },
}

/// The offset of one polyline or circle to `side`, or `None` if it
/// collapses.
fn offset(object: &Object, side: &str, shift: Shift) -> Result<Option<Object>, String> {
    if let Some((center, radius)) = object.circle() {
        let outward = match side {
            "outside" => true,
            "inside" => false,
            _ => return Err("A circle is offset outside or inside.".to_owned()),
        };
        let (distance, z) = match shift {
            Shift::Flat { distance, z } => (distance, center.z + z),
            Shift::Batter { tan, rl } => (((rl - center.z) / tan).abs(), rl),
        };
        let radius = if outward { radius + distance } else { radius - distance };
        let mut new = object.clone();
        if let Object::Circle { center: c, radius: r, .. } = &mut new {
            *c = DVec3::new(center.x, center.y, z);
            *r = radius;
        }
        return Ok((radius > 1.0e-9).then_some(new));
    }
    let Some((verts, closed)) = object.tessellated_path() else {
        return Err("Only polylines and circles can be offset.".to_owned());
    };
    match (side, closed) {
        ("outside" | "inside", false) => return Err("An open polyline is offset left or right.".to_owned()),
        ("left" | "right", true) => return Err("A closed polyline is offset outside or inside.".to_owned()),
        _ => {}
    }
    // Which way the positive side lies, found by trying it.
    let probe = geometry::geometric_offset(&verts, closed, 1.0, 0.0);
    let positive_is = if closed {
        if geometry::signed_area_xy(&probe).abs() > geometry::signed_area_xy(&verts).abs() {
            "outside"
        } else {
            "inside"
        }
    } else {
        let along = verts[1] - verts[0];
        let moved = probe[0] - verts[0];
        if along.x * moved.y - along.y * moved.x > 0.0 { "left" } else { "right" }
    };
    let sign = if positive_is == side { 1.0 } else { -1.0 };
    let new_verts = match shift {
        Shift::Flat { distance, z } => geometry::geometric_offset(&verts, closed, sign * distance, z),
        Shift::Batter { tan, rl } => geometry::geometric_offset_project_to_rl(&verts, closed, sign, tan, rl),
    };
    let (Some(fill), Some(line_weight)) = (object.fill(), object.line_weight()) else {
        return Err("Only polylines and circles can be offset.".to_owned());
    };
    let new = Object::Polyline {
        id: object.id(),
        layer: object.layer(),
        verts: new_verts.into_iter().map(PolyVertex::straight).collect(),
        closed,
        color: object.color(),
        fill,
        line_weight,
    };
    new.validate_geometry()?;
    Ok(Some(new))
}
