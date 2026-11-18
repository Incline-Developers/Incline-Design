//! The tools that change the project: undo and redo, checkpoints, layers,
//! and design objects. Each change is one undo step, made the way the window's own
//! tools make theirs, and reported in the activity log.

use serde_json::{Value, json};

use super::{failed, ok, parse_hex, parse_point, read::fill_name};
use crate::{
    app::App,
    i18n::tr,
    logging::CommandReportSpec,
    model::{Command, FillStyle, HistoryRoute, LayerId, Object, ObjectColor, ObjectId, PolyVertex, SceneEntityId},
    ui::state::UiCommand,
};

pub(super) fn definitions() -> Vec<Value> {
    let colour = json!({ "type": "string", "description": "#rrggbb, or by_layer to follow the layer's colour." });
    let fill = json!({ "type": "string", "enum": FillStyle::ALL.map(fill_name), "description": "How a closed polyline or circle is filled. clear if left out." });
    vec![
        json!({
            "name": "undo",
            "title": "Undo",
            "description": "Undo the last change to the project in Incline Design, whoever made it, as Edit > Undo does.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "redo",
            "title": "Redo",
            "description": "Redo the last change undone in Incline Design, as Edit > Redo does.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "checkpoint",
            "title": "Set a checkpoint",
            "description": "Name the point the project's undo history stands at now, to come back to with rollback: before trying a design, say, so it can be backed out whole however many steps it took. Answers with every checkpoint and whether it can still be reached. Saving the project clears the undo history, and so every checkpoint before it.",
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string", "minLength": 1, "description": "A name of your own. Reusing one moves it here." } },
                "required": ["name"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
        json!({
            "name": "rollback",
            "title": "Roll back to a checkpoint",
            "description": "Undo every change made since a checkpoint, whoever made it, or redo back to it if it was undone past. The steps undone can be redone until the next change. Steps that need items loaded first finish in the background; get_tasks follows them.",
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string", "description": "A checkpoint's name." } },
                "required": ["name"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true },
        }),
        json!({
            "name": "create_layer",
            "title": "Create design layer",
            "description": "Add an empty design layer to the project and make it the layer the user draws on. Returns its id, for add_objects.",
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string", "minLength": 1 } },
                "required": ["name"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "add_objects",
            "title": "Add design objects",
            "description": "Draw design objects on a design layer, all as one undo step: polylines (at least two vertices, open or closed, with optional DXF bulges for arcs), points, circles and text. Coordinates are [x, y, z] in the project's units. Colour and line weight default to the drawing tools' current ones. Returns the new objects' ids.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "layer": { "type": "integer", "minimum": 0, "description": "The design layer's id. It must be loaded and unlocked." },
                    "objects": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["polyline", "point", "circle", "text"] },
                                "vertices": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }, "description": "polyline: its vertices." },
                                "bulges": { "type": "array", "items": { "type": "number" }, "description": "polyline: one per vertex, for the segment starting there; 0 is straight. All straight if left out." },
                                "closed": { "type": "boolean", "description": "polyline: whether the last vertex joins the first. False if left out." },
                                "position": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "point and text: where it is." },
                                "centre": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "circle: its centre." },
                                "radius": { "type": "number", "exclusiveMinimum": 0, "description": "circle: its radius." },
                                "text": { "type": "string", "description": "text: what it says." },
                                "height": { "type": "number", "exclusiveMinimum": 0, "description": "text: its height in project units." },
                                "rotation": { "type": "number", "description": "text: anticlockwise turn in degrees. 0 if left out." },
                                "colour": colour,
                                "line_weight": { "type": "number", "minimum": 0, "description": "polyline and circle: line weight." },
                                "fill": fill,
                            },
                            "required": ["type"],
                        },
                    },
                },
                "required": ["layer", "objects"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "delete_objects",
            "title": "Delete design objects",
            "description": "Delete design objects by id, all as one undo step. Objects on locked layers are not deleted.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ids": { "type": "array", "minItems": 1, "items": { "type": "integer", "minimum": 0 } },
                },
                "required": ["ids"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true },
        }),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_edit_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        Some(match name {
            "undo" => self.mcp_history_step(true),
            "redo" => self.mcp_history_step(false),
            "checkpoint" => self.mcp_checkpoint(arguments),
            "rollback" => self.mcp_rollback(arguments),
            "create_layer" => self.mcp_create_layer(arguments),
            "add_objects" => self.mcp_add_objects(arguments),
            "delete_objects" => self.mcp_delete_objects(arguments),
            _ => return None,
        })
    }

    fn mcp_history_step(&mut self, undo: bool) -> Value {
        let possible = if undo { self.history.can_undo() } else { self.history.can_redo() };
        if !possible {
            return failed(if undo { "There is nothing to undo." } else { "There is nothing to redo." });
        }
        self.handle_ui_commands(vec![if undo { UiCommand::Undo } else { UiCommand::Redo }]);
        ok(&json!({
            "done": if undo { "undo" } else { "redo" },
            "can_undo": self.history.can_undo(),
            "can_redo": self.history.can_redo(),
        }))
    }

    fn mcp_checkpoint(&mut self, arguments: &Value) -> Value {
        let Some(name) = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()) else {
            return failed("checkpoint needs a name.");
        };
        let mark = self.history.mark();
        self.mcp_checkpoints.retain(|(existing, _)| existing != name);
        self.mcp_checkpoints.push((name.to_owned(), mark));
        ok(&json!({ "checkpoint": name, "checkpoints": self.mcp_checkpoint_list() }))
    }

    /// Every checkpoint, oldest first, with how far back each lies.
    fn mcp_checkpoint_list(&self) -> Vec<Value> {
        self.mcp_checkpoints
            .iter()
            .map(|(name, mark)| {
                let (reachable, steps) = match self.history.route_to(*mark) {
                    Some(HistoryRoute::Here) => (true, 0),
                    Some(HistoryRoute::Undo(steps)) => (true, -(steps as i64)),
                    Some(HistoryRoute::Redo(steps)) => (true, steps as i64),
                    None => (false, 0),
                };
                json!({ "name": name, "reachable": reachable, "steps": steps })
            })
            .collect()
    }

    fn mcp_rollback(&mut self, arguments: &Value) -> Value {
        let name = arguments["name"].as_str().unwrap_or_default();
        let Some(mark) = self.mcp_checkpoints.iter().find(|(existing, _)| existing == name).map(|(_, mark)| *mark) else {
            let names: Vec<&str> = self.mcp_checkpoints.iter().map(|(name, _)| name.as_str()).collect();
            return failed(format!("There is no checkpoint called {name}. There are: {}.", if names.is_empty() { "none".to_owned() } else { names.join(", ") }));
        };
        let steps = match self.history.route_to(mark) {
            None => return failed(format!("The undo history no longer reaches checkpoint {name}: the project was saved since, or the history dropped its oldest steps to stay within its memory budget.")),
            Some(HistoryRoute::Here) => return ok(&json!({ "rolled_back_to": name, "steps": 0 })),
            Some(HistoryRoute::Undo(steps) | HistoryRoute::Redo(steps)) => steps,
        };
        self.history_walk = Some(mark);
        self.continue_history_walk();
        let arrived = self.history.route_to(mark) == Some(HistoryRoute::Here);
        ok(&json!({
            "rolled_back_to": name,
            "steps": steps,
            "finished": arrived,
            "note": if arrived { "" } else { "Some steps are waiting for items to load; they finish in the background." },
        }))
    }

    fn mcp_create_layer(&mut self, arguments: &Value) -> Value {
        let Some(name) = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()) else {
            return failed("create_layer needs a name.");
        };
        match self.create_named_layer(name.to_owned()) {
            Some(id) => ok(&json!({ "id": id.0, "name": name })),
            None => failed("No project is open."),
        }
    }

    fn mcp_add_objects(&mut self, arguments: &Value) -> Value {
        let (Some(layer), Some(objects)) = (arguments["layer"].as_u64().map(LayerId), arguments["objects"].as_array()) else {
            return failed("add_objects needs a layer and a list of objects.");
        };
        if let Err(error) = self.mcp_editable_layer(layer) {
            return failed(error);
        }
        let defaults = (ObjectColor::Fixed(self.editor.tool_line_color), self.editor.tool_line_weight);
        let mut built = Vec::with_capacity(objects.len());
        for (index, description) in objects.iter().enumerate() {
            match build_object(description, layer, defaults) {
                Ok(object) => built.push(object),
                Err(error) => return failed(format!("Object {index}: {error}")),
            }
        }
        let Some(project) = self.workspace.active_project_mut() else {
            return failed("No project is open.");
        };
        let document = &mut project.project.document;
        let mut ids = Vec::with_capacity(built.len());
        let commands: Vec<Command> = built
            .into_iter()
            .map(|object| {
                let id = document.allocate_object_id();
                ids.push(id.0);
                Command::AddObject(object.with_id_and_layer(id, layer))
            })
            .collect();
        let count = commands.len();
        self.execute_job_edit(Command::Batch(commands));
        self.invalidate_geometry();
        crate::logging::report_completed_action(
            CommandReportSpec::new(tr!("mcp-add-objects"), tr!("common-count-object-s", count = count.to_string())),
            tr!("mcp-added-objects", count = count.to_string()),
        );
        ok(&json!({ "layer": layer.0, "ids": ids }))
    }

    fn mcp_delete_objects(&mut self, arguments: &Value) -> Value {
        let Some(ids) = arguments["ids"].as_array() else {
            return failed("delete_objects needs a list of ids.");
        };
        let Some(document) = self.workspace.active_document() else {
            return failed("No project is open.");
        };
        let (mut deleted, mut not_found, mut locked) = (Vec::new(), Vec::new(), Vec::new());
        let mut commands = Vec::new();
        for id in ids.iter().filter_map(Value::as_u64) {
            match document.get_object(ObjectId(id)) {
                None => not_found.push(id),
                Some(object) if self.editor.locked_layers.contains(&object.layer()) => locked.push(id),
                Some(object) => {
                    deleted.push(id);
                    commands.push(Command::delete_object(object.clone()));
                }
            }
        }
        let count = commands.len();
        if count > 0 {
            self.execute_job_edit(Command::Batch(commands));
            self.editor
                .selected_handles
                .retain(|handle| !matches!(handle, SceneEntityId::Object(id) if deleted.contains(&id.0)));
            self.invalidate_geometry();
            self.invalidate_overlay();
            crate::logging::report_completed_action(
                CommandReportSpec::new(tr!("mcp-delete-objects"), tr!("common-count-object-s", count = count.to_string())),
                tr!("mcp-deleted-objects", count = count.to_string()),
            );
        }
        ok(&json!({ "deleted": deleted, "not_found": not_found, "on_locked_layers": locked }))
    }

    /// Whether objects can be drawn on `layer`, or why not.
    pub(super) fn mcp_editable_layer(&self, layer: LayerId) -> Result<(), String> {
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let found = document
            .layer(layer)
            .ok_or_else(|| format!("There is no design layer with id {}. get_project lists them.", layer.0))?;
        if !found.loaded || document.deferred_layers.contains_key(&layer) {
            return Err(format!("The design layer {} is not loaded. Load it with set_visibility first.", found.name));
        }
        if self.editor.locked_layers.contains(&layer) {
            return Err(format!("The design layer {} is locked. The user can unlock it in the explorer.", found.name));
        }
        Ok(())
    }
}

/// One object as `add_objects` describes it, on `layer`, with a placeholder
/// id until it is given its own.
pub(super) fn build_object(description: &Value, layer: LayerId, (default_colour, default_weight): (ObjectColor, f32)) -> Result<Object, String> {
    let colour = match description["colour"].as_str() {
        None => default_colour,
        Some("by_layer") => ObjectColor::ByLayer,
        Some(text) => ObjectColor::Fixed(parse_hex(text).ok_or_else(|| format!("{text} is not a colour: give #rrggbb or by_layer."))?),
    };
    let line_weight = description["line_weight"].as_f64().map_or(default_weight, |weight| weight as f32);
    let fill = match description["fill"].as_str() {
        None => FillStyle::Clear,
        Some(name) => FillStyle::ALL
            .into_iter()
            .find(|fill| fill_name(*fill) == name)
            .ok_or_else(|| format!("{name} is not a fill: give clear, crosses, slashes or solid."))?,
    };
    let at = |key: &str| parse_point(&description[key]).ok_or_else(|| format!("{key} must be [x, y, z]."));
    let id = ObjectId(0);
    let object = match description["type"].as_str() {
        Some("polyline") => {
            let vertices = description["vertices"].as_array().ok_or("A polyline needs its vertices.")?;
            let bulges = description["bulges"].as_array();
            if bulges.is_some_and(|bulges| bulges.len() != vertices.len()) {
                return Err("bulges, when given, needs one for each vertex.".to_owned());
            }
            let verts = vertices
                .iter()
                .enumerate()
                .map(|(index, vertex)| {
                    let pos = parse_point(vertex).ok_or_else(|| format!("Vertex {index} must be [x, y, z]."))?;
                    let bulge = bulges.and_then(|bulges| bulges[index].as_f64()).unwrap_or(0.0);
                    Ok(PolyVertex { pos, bulge })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Object::Polyline {
                id,
                layer,
                verts,
                closed: description["closed"].as_bool().unwrap_or(false),
                color: colour,
                fill,
                line_weight,
            }
        }
        Some("point") => Object::Point {
            id,
            layer,
            pos: at("position")?,
            color: colour,
        },
        Some("circle") => Object::Circle {
            id,
            layer,
            center: at("centre")?,
            radius: description["radius"].as_f64().ok_or("A circle needs its radius.")?,
            color: colour,
            fill,
            line_weight,
        },
        Some("text") => Object::Text {
            id,
            layer,
            pos: at("position")?,
            content: description["text"].as_str().ok_or("Text needs its text.")?.to_owned(),
            height: description["height"].as_f64().ok_or("Text needs its height.")?,
            rotation: description["rotation"].as_f64().unwrap_or(0.0).to_radians(),
            color: colour,
        },
        _ => return Err("type is polyline, point, circle or text.".to_owned()),
    };
    object.validate_geometry()?;
    Ok(object)
}
