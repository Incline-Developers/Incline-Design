//! The MCP server's tools: what an AI tool connected to Incline can ask of
//! it (`app/mcp.rs`).
//!
//! Each is written by hand for what it does, rather than generated from the
//! UI's commands: the descriptions are what the AI reads to choose one, and
//! what one returns is shaped for it to read. They read the window's own
//! state, the same the explorer and panels draw from, and change it only by
//! sending the commands the window's own controls send, so undo, the
//! activity log and every redraw follow as they would for a click.
//!
//! Every result is JSON, sent as the tool's text.

use serde_json::{Value, json};

use crate::{
    app::App,
    model::{ItemRef, SceneEntityId},
    ui::state::UiCommand,
};

/// How many activity entries `get_activity` returns unless asked for more.
const DEFAULT_ACTIVITY: usize = 20;

/// The tools, as `tools/list` describes them.
pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_project",
            "title": "Get project",
            "description": "Describe the project open in Incline Design: its name and file, whether it has unsaved changes, and every item in it with its id: surfaces (triangulations), block models (block and variable counts, extents), drillhole collections (hole and field counts), point clouds, rasters, and design layers. Whether each is loaded and whether it is hidden. Coordinates are in the project's own units, usually metres.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_selection",
            "title": "Get selection",
            "description": "List what the user has selected in Incline Design: each item's kind, id and name.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_activity",
            "title": "Get activity",
            "description": "Read Incline Design's activity log, newest last: what the user and the application have done, with any warnings and errors, each with its time, severity, title, summary and details.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "description": "How many of the most recent entries to return. 20 if left out." },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "set_visibility",
            "title": "Show or hide an item",
            "description": "Show or hide one item in Incline Design's viewport, as the eye beside it in the explorer does. Showing an item that is not loaded loads it first. Ids and kinds are as get_project reports them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["surface", "block_model", "drillholes", "point_cloud", "raster", "design_layer"] },
                    "id": { "type": "integer", "minimum": 0 },
                    "visible": { "type": "boolean" },
                },
                "required": ["kind", "id", "visible"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
    ]
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

impl App<'_> {
    /// Run the tool `tools/call` names, with its arguments.
    pub(super) fn call_mcp_tool(&mut self, params: &Value) -> Value {
        let arguments = &params["arguments"];
        match params["name"].as_str().unwrap_or_default() {
            "get_project" => ok(&self.mcp_project()),
            "get_selection" => ok(&self.mcp_selection()),
            "get_activity" => {
                let limit = arguments["limit"].as_u64().map_or(DEFAULT_ACTIVITY, |limit| limit as usize);
                ok(&mcp_activity(limit))
            }
            "set_visibility" => self.mcp_set_visibility(arguments),
            name => failed(format!("There is no tool called {name}.")),
        }
    }

    fn mcp_project(&self) -> Value {
        let view = self.project_view();
        let project = view.titled_project();
        let layers: Vec<Value> = project
            .map(|project| {
                project
                    .layers
                    .iter()
                    .map(|layer| json!({ "id": layer.id.0, "name": layer.name, "loaded": layer.is_loaded, "hidden": layer.is_hidden }))
                    .collect()
            })
            .unwrap_or_default();
        json!({
            "name": project.map(|project| project.name.clone()),
            "file": project.and_then(|project| project.path.as_ref()).map(|path| path.display().to_string()),
            "unsaved_changes": project.is_some_and(|project| project.dirty),
            "surfaces": view.triangulations.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden,
            })).collect::<Vec<_>>(),
            "block_models": view.block_models.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden,
                "blocks": item.block_count, "variables": item.variable_count,
                "extent": { "min": [item.lower.x, item.lower.y, item.lower.z], "max": [item.upper.x, item.upper.y, item.upper.z] },
            })).collect::<Vec<_>>(),
            "drillholes": view.drill_holes.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden,
                "holes": item.hole_count, "fields": item.field_count,
            })).collect::<Vec<_>>(),
            "point_clouds": view.point_clouds.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden,
                "points": item.point_count, "classified": item.is_classified,
            })).collect::<Vec<_>>(),
            "rasters": view.raster_textures.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden,
                "draped": item.is_draped, "pixels": item.source_size, "projection": item.projection,
            })).collect::<Vec<_>>(),
            "design_layers": layers,
        })
    }

    fn mcp_selection(&self) -> Value {
        let view = self.project_view();
        let name_of = |entity: &SceneEntityId| -> Option<String> {
            match entity {
                SceneEntityId::Triangulation(id) => view.triangulations.iter().find(|item| item.id == *id).map(|item| item.name.clone()),
                SceneEntityId::BlockModel(id) => view.block_models.iter().find(|item| item.id == *id).map(|item| item.name.clone()),
                SceneEntityId::DrillHole(id) => view.drill_holes.iter().find(|item| item.id == *id).map(|item| item.name.clone()),
                SceneEntityId::PointCloud(id) => view.point_clouds.iter().find(|item| item.id == *id).map(|item| item.name.clone()),
                SceneEntityId::Raster(id) => view.raster_textures.iter().find(|item| item.id == *id).map(|item| item.name.clone()),
                SceneEntityId::Object(_) | SceneEntityId::HaulRoad(_) | SceneEntityId::HaulNode(_) => None,
            }
        };
        let mut selected: Vec<Value> = self
            .editor
            .selected_handles
            .iter()
            .map(|entity| {
                let (kind, id) = match entity {
                    SceneEntityId::Object(id) => ("design_object", id.0),
                    SceneEntityId::Triangulation(id) => ("surface", id.0),
                    SceneEntityId::BlockModel(id) => ("block_model", id.0),
                    SceneEntityId::DrillHole(id) => ("drillholes", id.0),
                    SceneEntityId::PointCloud(id) => ("point_cloud", id.0),
                    SceneEntityId::Raster(id) => ("raster", id.0),
                    SceneEntityId::HaulRoad(id) => ("haul_road", id.0),
                    SceneEntityId::HaulNode(id) => ("haul_node", id.0),
                };
                json!({ "kind": kind, "id": id, "name": name_of(entity) })
            })
            .collect();
        selected.sort_by_key(|item| (item["kind"].as_str().unwrap_or_default().to_owned(), item["id"].as_u64()));
        json!({ "selected": selected })
    }

    fn mcp_set_visibility(&mut self, arguments: &Value) -> Value {
        let (Some(kind), Some(id), Some(visible)) = (arguments["kind"].as_str(), arguments["id"].as_u64(), arguments["visible"].as_bool()) else {
            return failed("set_visibility needs kind, id and visible.");
        };
        let view = self.project_view();
        let command = match kind {
            "surface" if view.triangulations.iter().any(|item| item.id.0 == id) => {
                UiCommand::SetItemVisible(ItemRef::Triangulation(crate::model::triangulation::TriangulationId(id)), visible)
            }
            "block_model" if view.block_models.iter().any(|item| item.id.0 == id) => {
                UiCommand::SetItemVisible(ItemRef::BlockModel(crate::model::block_model::BlockModelId(id)), visible)
            }
            "drillholes" if view.drill_holes.iter().any(|item| item.id.0 == id) => {
                UiCommand::SetItemVisible(ItemRef::DrillHole(crate::model::drill_hole::DrillHoleId(id)), visible)
            }
            "point_cloud" if view.point_clouds.iter().any(|item| item.id.0 == id) => {
                UiCommand::SetItemVisible(ItemRef::PointCloud(crate::model::point_cloud::PointCloudId(id)), visible)
            }
            "raster" if view.raster_textures.iter().any(|item| item.id.0 == id) => UiCommand::SetItemVisible(ItemRef::Raster(crate::model::raster::RasterTextureId(id)), visible),
            "design_layer" if view.titled_project().is_some_and(|project| project.layers.iter().any(|layer| layer.id.0 == id)) => {
                UiCommand::SetLayerVisible(crate::model::LayerId(id), visible)
            }
            _ => return failed(format!("There is no {kind} with id {id} in the project. get_project lists them.")),
        };
        self.handle_ui_commands(vec![command]);
        ok(&json!({ "kind": kind, "id": id, "visible": visible }))
    }
}

/// The last `limit` activity entries, oldest first.
fn mcp_activity(limit: usize) -> Value {
    let (_, entries) = crate::logging::console_snapshot();
    let start = entries.len().saturating_sub(limit);
    let entries: Vec<Value> = entries[start..]
        .iter()
        .map(|entry| {
            json!({
                "time": entry.timestamp,
                "severity": format!("{:?}", entry.severity).to_lowercase(),
                "title": entry.title,
                "summary": entry.summary,
                "details": entry.details,
            })
        })
        .collect();
    json!({ "entries": entries })
}
