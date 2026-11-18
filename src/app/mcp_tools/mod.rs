//! The MCP server's tools: what an AI tool connected to Incline can ask of
//! it (`app/mcp.rs`).
//!
//! Each is written by hand for what it does, rather than generated from the
//! UI's commands: the descriptions are what the AI reads to choose one, and
//! what one returns is shaped for it to read. They read the window's own
//! state, the same the explorer and panels draw from, and change it only by
//! sending the commands the window's own controls send, or by running the
//! same undoable edits they run, so undo, the activity log and every redraw
//! follow as they would for a click.
//!
//! - [`read`]: the project, the selection, the activity log, and what is in
//!   each layer, surface, block model and drillhole collection.
//! - [`view`]: the selection, the camera, and a look at the viewport.
//! - [`edit`]: undo and redo, layers, and design objects.
//! - [`files`]: importing, exporting, saving and opening.
//! - [`style`]: colours, display options and section view.
//! - [`items`]: renaming, deleting, loading, locking and filing items.
//! - [`objects`]: editing design objects, and the drawing tools that make
//!   new ones from them.
//! - [`jobs`]: long work, run in the background: building, cutting,
//!   contouring and thinning surfaces, surfacing and classifying point
//!   clouds, and estimating block models.
//! - [`planning`]: pit optimisation and NPV schedules.
//! - [`solids`]: the Planning workspace's reserve fields, solids, steps and
//!   reserves.
//! - [`production`]: the production schedule and haul network.
//! - [`blast`]: drill patterns, initiation, tie-ins and charging.
//! - [`survey`]: coordinate systems, reference surfaces and points,
//!   geophysics, and rain simulation.
//!
//! Every result is JSON sent as the tool's text, except the viewport's
//! picture, which is an image.

mod blast;
mod edit;
mod files;
mod items;
mod jobs;
mod objects;
mod planning;
mod production;
mod read;
mod solids;
mod style;
mod survey;
mod view;

use glam::DVec3;
pub(super) use jobs::PendingWait;
use serde_json::{Value, json};
pub(super) use view::PendingCapture;

use crate::{
    app::App,
    model::{
        ItemRef, LayerId, Object, ObjectColor, ObjectId, SceneEntityId, block_model::BlockModelId, drill_hole::DrillHoleId, point_cloud::PointCloudId, raster::RasterTextureId,
        triangulation::TriangulationId,
    },
};

/// The tools, as `tools/list` describes them.
pub(super) fn definitions() -> Vec<Value> {
    let mut tools = read::definitions();
    tools.extend(view::definitions());
    tools.extend(edit::definitions());
    tools.extend(objects::definitions());
    tools.extend(items::definitions());
    tools.extend(style::definitions());
    tools.extend(files::definitions());
    tools.extend(jobs::definitions());
    tools.extend(planning::definitions());
    tools.extend(solids::definitions());
    tools.extend(production::definitions());
    tools.extend(blast::definitions());
    tools.extend(survey::definitions());
    tools
}

/// The kinds of thing a tool can be pointed at, as `get_project` and
/// `get_selection` name them.
const KINDS: [&str; 7] = ["surface", "block_model", "drillholes", "point_cloud", "raster", "design_layer", "design_object"];

/// The JSON schema of a list of `{kind, id}` pairs.
fn items_schema(description: &str) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": {
            "type": "object",
            "properties": {
                "kind": { "type": "string", "enum": KINDS },
                "id": { "type": "integer", "minimum": 0 },
            },
            "required": ["kind", "id"],
            "additionalProperties": false,
        },
    })
}

/// How full the machine's memory is, as a fraction, before every tool's
/// result carries a warning.
const MEMORY_WARNING_FRACTION: f32 = 0.8;

/// How many new warnings a tool result lists before saying how many more.
const NEW_WARNINGS_SHOWN: usize = 5;

/// `bytes` in gigabytes, to one decimal place.
fn gigabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

/// A tool's successful result.
fn ok(value: &Value) -> Value {
    json!({ "content": [{ "type": "text", "text": plain(&serde_json::to_string_pretty(value).unwrap_or_default()) }], "isError": false })
}

/// `text` without the invisible marks the translations put round each value
/// they fill in (`i18n`), which mean nothing to an AI reading it.
fn plain(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\u{2068}' | '\u{2069}')).collect()
}

/// A tool's failure, said so the AI can correct itself.
fn failed(message: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": plain(&message.into()) }], "isError": true })
}

/// A colour as `#rrggbb`, from the linear RGBA the project keeps.
fn hex(rgba: [f32; 4]) -> String {
    use crate::rendering::color::linear_to_srgb_byte;
    format!(
        "#{:02x}{:02x}{:02x}",
        linear_to_srgb_byte(rgba[0]),
        linear_to_srgb_byte(rgba[1]),
        linear_to_srgb_byte(rgba[2])
    )
}

/// `#rrggbb` (or `rrggbb`) as the linear RGBA the project keeps.
fn parse_hex(text: &str) -> Option<[f32; 4]> {
    let digits = text.trim().trim_start_matches('#');
    if digits.len() != 6 {
        return None;
    }
    u32::from_str_radix(digits, 16).ok().map(crate::rendering::color::hex_to_linear_rgba)
}

/// A point as `[x, y, z]`.
fn point(pos: DVec3) -> Value {
    json!([pos.x, pos.y, pos.z])
}

/// `[x, y, z]` as a point.
fn parse_point(value: &Value) -> Option<DVec3> {
    let coordinates = value.as_array()?;
    let [x, y, z] = coordinates.as_slice() else {
        return None;
    };
    let point = DVec3::new(x.as_f64()?, y.as_f64()?, z.as_f64()?);
    point.is_finite().then_some(point)
}

/// `patch` laid over `target` as a JSON merge patch (RFC 7396): objects
/// merge key by key, `null` removes a key, and anything else replaces what
/// was there, arrays whole.
fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    let Value::Object(target) = target else {
        return;
    };
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

/// A box as `{min, max}`.
fn extent(min: DVec3, max: DVec3) -> Value {
    json!({ "min": point(min), "max": point(max) })
}

/// What a `{kind, id}` names, once it is known to be in the project.
#[derive(Clone, Copy)]
enum Target {
    Item(ItemRef),
    Layer(LayerId),
    Object(ObjectId),
}

impl App<'_> {
    /// Run the tool `tools/call` names, with its arguments. `capture_viewport`
    /// is answered by `app/mcp.rs` itself, as it waits for a frame.
    pub(super) fn call_mcp_tool(&mut self, params: &Value) -> Value {
        let mut result = self.dispatch_mcp_tool(params);
        let notes: Vec<String> = self.mcp_new_warnings().into_iter().chain(self.mcp_memory_warning()).collect();
        if let Some(content) = result["content"].as_array_mut() {
            content.extend(notes.into_iter().map(|text| json!({ "type": "text", "text": text })));
        }
        result
    }

    /// The warnings and errors logged since the last tool call, which the
    /// window shows the user and the AI would otherwise only find by
    /// reading the activity log: a background job that failed, a dialog
    /// waiting on the user.
    fn mcp_new_warnings(&mut self) -> Option<String> {
        let (_, entries) = crate::logging::console_snapshot();
        let flagged = entries
            .iter()
            .filter(|entry| matches!(entry.severity, crate::logging::ConsoleSeverity::Warn | crate::logging::ConsoleSeverity::Error));
        let Some(seen) = self.mcp_seen_warnings.as_mut() else {
            self.mcp_seen_warnings = Some(flagged.map(|entry| entry.id).collect());
            return None;
        };
        let new: Vec<&crate::logging::ConsoleEntry> = flagged.filter(|entry| seen.insert(entry.id)).collect();
        if new.is_empty() {
            return None;
        }
        let mut lines: Vec<String> = new
            .iter()
            .take(NEW_WARNINGS_SHOWN)
            .map(|entry| format!("{} ({:?}): {}", entry.title, entry.severity, entry.summary))
            .collect();
        if new.len() > NEW_WARNINGS_SHOWN {
            lines.push(format!("and {} more; get_activity lists them.", new.len() - NEW_WARNINGS_SHOWN));
        }
        Some(format!("New in the activity log since the last call:\n{}", lines.join("\n")))
    }

    /// A warning for the AI when the machine's memory is nearly full, so it
    /// frees some before a big job runs it out: the window is what an
    /// out-of-memory kill takes down, with any unsaved work.
    fn mcp_memory_warning(&self) -> Option<String> {
        let usage = self.editor.memory_usage?;
        if usage.fraction() < MEMORY_WARNING_FRACTION {
            return None;
        }
        Some(format!(
            "Warning: this computer's memory is {:.0}% full ({} of {} GB in use). Before anything heavy, save the project to clear its undo history and unload items the work no longer needs (set_loaded), or a big job may run it out of memory and close Incline Design, losing unsaved work.",
            usage.fraction() * 100.0,
            gigabytes(usage.used),
            gigabytes(usage.total)
        ))
    }

    /// Run the tool `tools/call` names.
    fn dispatch_mcp_tool(&mut self, params: &Value) -> Value {
        let arguments = &params["arguments"];
        let name = params["name"].as_str().unwrap_or_default();
        if let Some(result) = self.call_mcp_read_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_view_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_edit_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_file_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_style_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_item_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_object_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_job_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_planning_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_solid_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_production_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_blast_tool(name, arguments) {
            return result;
        }
        if let Some(result) = self.call_mcp_survey_tool(name, arguments) {
            return result;
        }
        failed(format!("There is no tool called {name}."))
    }

    /// What `kind` and `id` name, or why they name nothing.
    fn mcp_target(&self, kind: &str, id: u64) -> Result<Target, String> {
        let missing = || format!("There is no {kind} with id {id} in the project. get_project lists them.");
        let found = match kind {
            "surface" => self
                .triangulations
                .iter()
                .any(|item| item.id.0 == id)
                .then_some(Target::Item(ItemRef::Triangulation(TriangulationId(id)))),
            "block_model" => self
                .block_models
                .iter()
                .any(|item| item.id.0 == id)
                .then_some(Target::Item(ItemRef::BlockModel(BlockModelId(id)))),
            "drillholes" => self
                .drill_holes
                .iter()
                .any(|item| item.id.0 == id)
                .then_some(Target::Item(ItemRef::DrillHole(DrillHoleId(id)))),
            "point_cloud" => self
                .point_clouds
                .iter()
                .any(|item| item.id.0 == id)
                .then_some(Target::Item(ItemRef::PointCloud(PointCloudId(id)))),
            "raster" => self
                .raster_textures
                .iter()
                .any(|item| item.id.0 == id)
                .then_some(Target::Item(ItemRef::Raster(RasterTextureId(id)))),
            "design_layer" => self
                .workspace
                .active_document()
                .and_then(|document| document.layer(LayerId(id)))
                .map(|_| Target::Layer(LayerId(id))),
            "design_object" => self
                .workspace
                .active_document()
                .and_then(|document| document.get_object(ObjectId(id)))
                .map(|_| Target::Object(ObjectId(id))),
            _ => return Err(format!("{kind} is not a kind of item. The kinds are {}.", KINDS.join(", "))),
        };
        found.ok_or_else(missing)
    }

    /// Every `{kind, id}` in `items`, or the first that names nothing.
    fn mcp_targets(&self, items: &Value) -> Result<Vec<Target>, String> {
        let Some(items) = items.as_array() else {
            return Ok(Vec::new());
        };
        items
            .iter()
            .map(|item| match (item["kind"].as_str(), item["id"].as_u64()) {
                (Some(kind), Some(id)) => self.mcp_target(kind, id),
                _ => Err("Each item needs a kind and an id.".to_owned()),
            })
            .collect()
    }
}

/// The scene entity a target selects. A layer is selected through its
/// objects, so it has none of its own.
fn entity(target: Target) -> Option<SceneEntityId> {
    match target {
        Target::Item(ItemRef::Triangulation(id)) => Some(SceneEntityId::Triangulation(id)),
        Target::Item(ItemRef::BlockModel(id)) => Some(SceneEntityId::BlockModel(id)),
        Target::Item(ItemRef::DrillHole(id)) => Some(SceneEntityId::DrillHole(id)),
        Target::Item(ItemRef::PointCloud(id)) => Some(SceneEntityId::PointCloud(id)),
        Target::Item(ItemRef::Raster(id)) => Some(SceneEntityId::Raster(id)),
        Target::Object(id) => Some(SceneEntityId::Object(id)),
        Target::Layer(_) => None,
    }
}

/// The points that place a design object: its vertices, the corners of a
/// circle's square, or its one position.
fn object_points(object: &Object) -> Vec<DVec3> {
    match object {
        Object::Point { pos, .. } | Object::Text { pos, .. } => vec![*pos],
        Object::Polyline { verts, .. } => verts.iter().map(|vertex| vertex.pos).collect(),
        Object::Circle { center, radius, .. } => vec![*center - DVec3::new(*radius, *radius, 0.0), *center + DVec3::new(*radius, *radius, 0.0)],
    }
}

/// How the AI reads an object's colour: its own, or its layer's.
fn object_colour(color: ObjectColor) -> String {
    match color {
        ObjectColor::ByLayer => "by_layer".to_owned(),
        ObjectColor::Fixed(rgba) => hex(rgba),
    }
}
