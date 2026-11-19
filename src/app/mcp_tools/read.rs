//! The tools that only look: the project, the selection, the activity log,
//! and what is in each layer, surface, block model and drillhole collection.

use std::collections::BTreeMap;

use glam::DVec3;
use rayon::prelude::*;
use serde_json::{Value, json};

use super::{extent, failed, hex, object_colour, ok, point};
use crate::{
    app::App,
    model::{
        Document, FillStyle, LayerId, Object, ObjectId, SceneEntityId,
        drill_hole::{DrillFieldKind, DrillValue},
        formats::mesh_data,
    },
};

/// How many activity entries `get_activity` returns unless asked for more.
const DEFAULT_ACTIVITY: usize = 20;
/// How many of an activity entry's detail lines `get_activity` returns.
const DEFAULT_ACTIVITY_DETAILS: usize = 20;
/// How many objects `get_layer` returns at a time unless asked for more.
const DEFAULT_OBJECT_PAGE: usize = 200;
/// How many holes `get_drillholes` returns at a time unless asked for more.
const DEFAULT_HOLE_PAGE: usize = 100;

/// The schema of a tool that takes nothing.
fn no_arguments() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

/// The schema of a page through a long list.
fn page_properties(what: &str, default: usize) -> (Value, Value) {
    (
        json!({ "type": "integer", "minimum": 0, "description": format!("How many {what} to skip, to page through. 0 if left out.") }),
        json!({ "type": "integer", "minimum": 1, "description": format!("How many {what} to return. {default} if left out.") }),
    )
}

pub(super) fn definitions() -> Vec<Value> {
    let (object_offset, object_limit) = page_properties("objects", DEFAULT_OBJECT_PAGE);
    let (hole_offset, hole_limit) = page_properties("holes", DEFAULT_HOLE_PAGE);
    vec![
        json!({
            "name": "get_project",
            "title": "Get project",
            "description": "Describe the project open in Incline Design: whether this Incline is the user's window or a headless one of this client's own (mode), its name and file, whether it has unsaved changes, how much of the computer's memory is in use, and every item in it with its id: surfaces (triangulations, with their triangle counts when loaded), block models (block and variable counts, extents), drillhole collections (hole and field counts), point clouds, rasters, and design layers. Whether each is loaded and whether it is hidden, and the explorer folder it is filed in (folders lists them by section). Coordinates are in the project's own units, usually metres, with Z up.",
            "inputSchema": no_arguments(),
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_selection",
            "title": "Get selection",
            "description": "List what the user has selected in Incline Design: each item's kind, id and name. Design objects (polylines, points, circles and text) are listed by id; get_objects reads them.",
            "inputSchema": no_arguments(),
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_activity",
            "title": "Get activity",
            "description": "Read Incline Design's activity log, newest last: what the user and the application have done, with any warnings and errors, each with its time, severity, title, summary and details. A long run of details is cut short, with how many more there were.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "description": "How many of the most recent entries to return. 20 if left out." },
                    "details": { "type": "integer", "minimum": 0, "description": "How many detail lines to return for each entry. 20 if left out." },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_layer",
            "title": "Get design layer",
            "description": "Read the design objects on one design layer, a page at a time: polylines (vertices, closed or open), points, circles and text, each with its id and colour. Text rotation is anticlockwise in degrees. Vertices are [x, y, z]. A polyline's bulges are given only when it has arcs (DXF bulge: tan of a quarter of the arc's included angle, for the segment starting at that vertex). The layer must be loaded; set_visibility loads it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "minimum": 0, "description": "The design layer's id, from get_project." },
                    "offset": object_offset,
                    "limit": object_limit,
                },
                "required": ["id"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_objects",
            "title": "Get design objects",
            "description": "Read design objects by id, as get_layer describes them, with the layer each is on. Useful for what get_selection lists.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ids": { "type": "array", "items": { "type": "integer", "minimum": 0 } },
                },
                "required": ["ids"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_surface",
            "title": "Get surface",
            "description": "Measure one surface (triangulation): vertex and triangle counts, extent, surface area, plan area (each triangle's area seen from above, added up), whether it is closed (every edge shared by two triangles facing the same way) and, if it is, the volume it encloses. The surface must be loaded; set_visibility loads it.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer", "minimum": 0 } },
                "required": ["id"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "sample_surface",
            "title": "Sample surfaces",
            "description": "Read the RL of surfaces seen from above: the z of the uppermost of the given surfaces at each point, or null where none of them is. Give points for a list of places, or grid for a regular grid over an area, answered as rows of RLs, south to north, each row west to east. Useful for fitting a design to the ground. RLs are rounded to the millimetre. The surfaces must be loaded; set_loaded loads them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "surfaces": { "type": "array", "minItems": 1, "items": { "type": "integer", "minimum": 0 }, "description": "Surface ids." },
                    "points": { "type": "array", "minItems": 1, "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "description": "[x, y] pairs in the project's units." },
                    "grid": {
                        "type": "object",
                        "description": "A grid in place of points: from min to max in plan, every spacing, its first point at min. A large grid makes a long answer: 100 by 100 is 10 000 RLs.",
                        "properties": {
                            "min": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] of the south-west corner." },
                            "max": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x, y] of the north-east corner." },
                            "spacing": { "type": "number", "exclusiveMinimum": 0, "description": "Distance between points, east and north." },
                        },
                        "required": ["min", "max", "spacing"],
                        "additionalProperties": false,
                    },
                },
                "required": ["surfaces"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_block_model",
            "title": "Get block model",
            "description": "Describe one block model: its block count, grid, extent, the variable it is coloured by, and its variables with their types and any category names. Name variables in statistics to have each one's count, missing values, minimum, maximum and mean worked out (for a categorical variable, the number of blocks in each category). Statistics need the model loaded; set_visibility loads it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "minimum": 0 },
                    "statistics": { "type": "array", "items": { "type": "string" }, "description": "Names of the variables to work out statistics for." },
                },
                "required": ["id"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "get_drillholes",
            "title": "Get drillholes",
            "description": "Read one drillhole collection, a page of holes at a time: its fields (numeric with their range, or categorical with their categories), and each hole's id, collar [x, y, z], depth, diameter and interval count. Ask for intervals to have each hole's logged intervals too (from and to as downhole depths, with each field's value). The collection must be loaded; set_visibility loads it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "minimum": 0 },
                    "holes": { "type": "array", "items": { "type": "string" }, "description": "Only these holes, by their hole id. Every hole if left out." },
                    "intervals": { "type": "boolean", "description": "Include each hole's intervals. False if left out." },
                    "offset": hole_offset,
                    "limit": hole_limit,
                },
                "required": ["id"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
    ]
}

/// A tool's result, from what it worked out or why it could not.
fn outcome(result: Result<Value, String>) -> Value {
    result.map_or_else(failed, |value| ok(&value))
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_read_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        Some(match name {
            "get_project" => ok(&self.mcp_project()),
            "get_selection" => ok(&self.mcp_selection()),
            "get_activity" => ok(&activity(
                arguments["limit"].as_u64().map_or(DEFAULT_ACTIVITY, |limit| limit as usize),
                arguments["details"].as_u64().map_or(DEFAULT_ACTIVITY_DETAILS, |details| details as usize),
            )),
            "get_layer" => outcome(self.mcp_layer(arguments)),
            "get_objects" => outcome(self.mcp_objects(arguments)),
            "get_surface" => outcome(self.mcp_surface(arguments)),
            "sample_surface" => outcome(self.mcp_sample_surface(arguments)),
            "get_block_model" => outcome(self.mcp_block_model(arguments)),
            "get_drillholes" => outcome(self.mcp_drillholes(arguments)),
            _ => return None,
        })
    }

    fn mcp_project(&self) -> Value {
        let view = self.project_view();
        let project = view.titled_project();
        let layers: Vec<Value> = project
            .map(|project| {
                project
                    .layers
                    .iter()
                    .map(|layer| json!({ "id": layer.id.0, "name": layer.name, "loaded": layer.is_loaded, "hidden": layer.is_hidden, "folder": layer.folder.map(|folder| folder.0) }))
                    .collect()
            })
            .unwrap_or_default();
        json!({
            // Headless is a private Incline the user does not see; window is
            // theirs, open in front of them.
            "mode": if self.headless { "headless" } else { "window" },
            "name": project.map(|project| project.name.clone()),
            "file": project.and_then(|project| project.path.as_ref()).map(|path| path.display().to_string()),
            "unsaved_changes": project.is_some_and(|project| project.dirty),
            "memory": self.editor.memory_usage.map(|usage| json!({
                "used_gb": super::gigabytes(usage.used), "total_gb": super::gigabytes(usage.total),
            })),
            "surfaces": view.triangulations.iter().map(|item| {
                // Counted only where the surface is in memory: an unloaded
                // one's mesh is not to hand, and it is not read in to count it.
                let triangles = self
                    .triangulations
                    .iter()
                    .find(|surface| surface.id == item.id && surface.state.loaded && surface.state.deferred.is_none())
                    .map(|surface| surface.mesh.face_count());
                json!({
                    "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden, "folder": item.folder.map(|folder| folder.0),
                    "triangles": triangles,
                })
            }).collect::<Vec<_>>(),
            "block_models": view.block_models.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden, "folder": item.folder.map(|folder| folder.0),
                "blocks": item.block_count, "variables": item.variable_count,
                "extent": extent(item.lower, item.upper),
            })).collect::<Vec<_>>(),
            "drillholes": view.drill_holes.iter().map(|item| {
                let bounds = self.drill_holes.iter().find(|open| open.id == item.id).and_then(|open| open.dataset.bounds);
                json!({
                    "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden, "folder": item.folder.map(|folder| folder.0),
                    "holes": item.hole_count, "fields": item.field_count,
                    "extent": bounds.map(|(min, max)| extent(min, max)),
                })
            }).collect::<Vec<_>>(),
            "point_clouds": view.point_clouds.iter().map(|item| {
                let bounds = self.point_clouds.iter().find(|open| open.id == item.id).filter(|open| !open.points.is_empty()).map(|open| open.bounds);
                json!({
                    "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden, "folder": item.folder.map(|folder| folder.0),
                    "points": item.point_count, "classified": item.is_classified,
                    "extent": bounds.map(|(min, max)| extent(min, max)),
                })
            }).collect::<Vec<_>>(),
            "rasters": view.raster_textures.iter().map(|item| json!({
                "id": item.id.0, "name": item.name, "loaded": item.is_loaded, "hidden": item.is_hidden, "folder": item.folder.map(|folder| folder.0),
                "draped": item.is_draped, "pixels": item.source_size, "projection": item.projection,
            })).collect::<Vec<_>>(),
            "design_layers": layers,
            "folders": self.mcp_folders(),
        })
    }

    /// The explorer's folders, by section.
    fn mcp_folders(&self) -> Value {
        let Some(project) = self.workspace.active_project() else {
            return json!({});
        };
        let sections: serde_json::Map<String, Value> = crate::model::SectionKind::ALL
            .into_iter()
            .map(|section| {
                let folders: Vec<Value> = project
                    .project
                    .folders
                    .folders(section)
                    .iter()
                    .map(|folder| json!({ "id": folder.id.0, "name": folder.name }))
                    .collect();
                (super::items::section_name(section).to_owned(), json!(folders))
            })
            .filter(|(_, folders)| folders.as_array().is_some_and(|folders| !folders.is_empty()))
            .collect();
        json!(sections)
    }

    pub(super) fn mcp_selection(&self) -> Value {
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
                match name_of(entity) {
                    Some(name) => json!({ "kind": kind, "id": id, "name": name }),
                    None => json!({ "kind": kind, "id": id }),
                }
            })
            .collect();
        selected.sort_by_key(|item| (item["kind"].as_str().unwrap_or_default().to_owned(), item["id"].as_u64()));
        json!({ "selected": selected })
    }

    fn mcp_layer(&self, arguments: &Value) -> Result<Value, String> {
        let Some(id) = arguments["id"].as_u64() else {
            return Err("get_layer needs the layer's id.".into());
        };
        let Some(document) = self.workspace.active_document() else {
            return Err("No project is open.".into());
        };
        let Some(layer) = document.layer(LayerId(id)) else {
            return Err(format!("There is no design layer with id {id}. get_project lists them."));
        };
        if !layer.loaded || document.deferred_layers.contains_key(&layer.id) {
            return Err(format!("The design layer {} is not loaded. Load it with set_visibility, then ask again.", layer.name));
        }
        let offset = arguments["offset"].as_u64().unwrap_or(0) as usize;
        let limit = arguments["limit"].as_u64().map_or(DEFAULT_OBJECT_PAGE, |limit| limit as usize);
        let on_layer: Vec<&Object> = document.objects().iter().filter(|object| object.layer() == layer.id).collect();
        let page: Vec<Value> = on_layer.iter().skip(offset).take(limit).map(|object| object_json(document, object)).collect();
        Ok(json!({
            "id": layer.id.0,
            "name": layer.name,
            "colour": hex(layer.color),
            "hidden": layer.hidden,
            "locked": self.editor.locked_layers.contains(&layer.id),
            "object_count": on_layer.len(),
            "offset": offset,
            "objects": page,
        }))
    }

    fn mcp_objects(&self, arguments: &Value) -> Result<Value, String> {
        let Some(ids) = arguments["ids"].as_array() else {
            return Err("get_objects needs a list of ids.".into());
        };
        let Some(document) = self.workspace.active_document() else {
            return Err("No project is open.".into());
        };
        let mut objects = Vec::new();
        let mut missing = Vec::new();
        for id in ids.iter().filter_map(Value::as_u64) {
            match document.get_object(ObjectId(id)) {
                Some(object) => {
                    let mut value = object_json(document, object);
                    value["layer"] = json!(object.layer().0);
                    objects.push(value);
                }
                None => missing.push(id),
            }
        }
        // An object on a layer that is not loaded is not in memory to find.
        Ok(json!({ "objects": objects, "not_found": missing }))
    }

    fn mcp_surface(&self, arguments: &Value) -> Result<Value, String> {
        let Some(id) = arguments["id"].as_u64() else {
            return Err("get_surface needs the surface's id.".into());
        };
        let Some(surface) = self.triangulations.iter().find(|surface| surface.id.0 == id) else {
            return Err(format!("There is no surface with id {id}. get_project lists them."));
        };
        if !surface.state.loaded || surface.state.deferred.is_some() {
            return Err(format!("The surface {} is not loaded. Load it with set_visibility, then ask again.", surface.name));
        }
        let mesh = &surface.mesh;
        let bounds = mesh.bounds();
        let (min, max) = (DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z), DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z));
        let measured = measure(mesh, (min + max) * 0.5);
        let raster = surface
            .raster_texture
            .and_then(|raster| self.raster_textures.iter().find(|item| item.id == raster))
            .map(|raster| json!({ "id": raster.id.0, "name": raster.name }));
        Ok(json!({
            "id": id,
            "name": surface.name,
            "vertices": mesh.vertex_count(),
            "triangles": mesh.face_count(),
            "extent": (mesh.face_count() > 0).then(|| extent(min, max)),
            "area": measured.area,
            "plan_area": measured.plan_area,
            "closed": measured.closed,
            "volume": measured.closed.then_some(measured.volume.abs()),
            "colour": hex(surface.color),
            "draped_raster": raster,
        }))
    }

    fn mcp_sample_surface(&self, arguments: &Value) -> Result<Value, String> {
        let ids = arguments["surfaces"].as_array().ok_or("sample_surface needs a list of surface ids.")?;
        let mut surfaces = Vec::with_capacity(ids.len());
        for id in ids {
            let id = id.as_u64().ok_or("Surface ids are whole numbers.")?;
            let surface = self
                .triangulations
                .iter()
                .find(|surface| surface.id.0 == id)
                .ok_or_else(|| format!("There is no surface with id {id}. get_project lists them."))?;
            if !surface.state.loaded || surface.state.deferred.is_some() {
                return Err(format!("The surface {} is not loaded. Load it with set_visibility, then ask again.", surface.name));
            }
            surfaces.push(surface);
        }
        let rl = |x: f64, y: f64| crate::app::commands::drawing::drape::uppermost_topology_z(DVec3::new(x, y, 0.0), &surfaces).map(|z| (z * 1000.0).round() / 1000.0);
        if !arguments["grid"].is_null() {
            let grid = &arguments["grid"];
            let corner = |key: &str| match (grid[key][0].as_f64(), grid[key][1].as_f64()) {
                (Some(x), Some(y)) => Ok(glam::DVec2::new(x, y)),
                _ => Err(format!("grid.{key} is [x, y].")),
            };
            let (min, max) = (corner("min")?, corner("max")?);
            let spacing = grid["spacing"].as_f64().filter(|spacing| *spacing > 0.0).ok_or("grid.spacing must be above 0.")?;
            if max.x < min.x || max.y < min.y {
                return Err("grid.max must be north-east of grid.min.".to_owned());
            }
            // Every spacing from min, up to max: a point a hair short of max
            // through rounding still counts.
            let count = |span: f64| (span / spacing + 1.0e-9).floor() as usize + 1;
            let (columns, rows) = (count(max.x - min.x), count(max.y - min.y));
            let rls: Vec<Vec<Option<f64>>> = (0..rows)
                .into_par_iter()
                .map(|row| (0..columns).map(|column| rl(min.x + column as f64 * spacing, min.y + row as f64 * spacing)).collect())
                .collect();
            return Ok(json!({ "origin": [min.x, min.y], "spacing": spacing, "columns": columns, "rows": rows, "rls": rls }));
        }
        let points = arguments["points"].as_array().ok_or("sample_surface needs points, or a grid.")?;
        let rls = points
            .iter()
            .map(|point| {
                let (Some(x), Some(y)) = (point[0].as_f64(), point[1].as_f64()) else {
                    return Err("Each point is [x, y].".to_owned());
                };
                Ok(rl(x, y))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(json!({ "rls": rls }))
    }

    fn mcp_block_model(&self, arguments: &Value) -> Result<Value, String> {
        let Some(id) = arguments["id"].as_u64() else {
            return Err("get_block_model needs the block model's id.".into());
        };
        let Some(model) = self.block_models.iter().find(|model| model.id.0 == id) else {
            return Err(format!("There is no block model with id {id}. get_project lists them."));
        };
        let metadata = &model.model.metadata;
        let variables: Vec<Value> = metadata
            .variables
            .iter()
            .filter(|variable| !variable.special)
            .map(|variable| {
                let categorical = is_categorical(&variable.physical_type);
                let mut value = json!({ "name": variable.name, "type": if categorical { "categorical" } else { variable.physical_type.as_str() } });
                if categorical {
                    value["categories"] = json!(variable.strings.values().collect::<Vec<_>>());
                }
                value
            })
            .collect();
        let size = metadata.upper - metadata.lower;
        let grid = (!metadata.is_irregular && metadata.dims.iter().all(|count| *count > 0)).then(|| {
            json!({
                "blocks": metadata.dims,
                "block_size": [size.x / metadata.dims[0] as f64, size.y / metadata.dims[1] as f64, size.z / metadata.dims[2] as f64],
            })
        });
        let (lower, upper) = model.world_bounds.unwrap_or((metadata.lower, metadata.upper));
        let mut statistics = serde_json::Map::new();
        if let Some(names) = arguments["statistics"].as_array() {
            let loaded = model.state.loaded && model.state.deferred.is_none();
            for name in names.iter().filter_map(Value::as_str) {
                let result = if loaded {
                    variable_statistics(&model.model, name)
                } else {
                    Err(format!("The block model {} is not loaded. Load it with set_visibility, then ask again.", model.name))
                };
                statistics.insert(name.to_owned(), result.unwrap_or_else(|error| json!({ "error": error })));
            }
        }
        Ok(json!({
            "id": id,
            "name": model.name,
            "blocks": metadata.n_blocks,
            "regular": !metadata.is_irregular,
            "grid": grid,
            "extent": extent(lower, upper),
            "coloured_by": model.active_color_variable,
            "variables": variables,
            "statistics": statistics,
        }))
    }

    fn mcp_drillholes(&self, arguments: &Value) -> Result<Value, String> {
        let Some(id) = arguments["id"].as_u64() else {
            return Err("get_drillholes needs the collection's id.".into());
        };
        let Some(collection) = self.drill_holes.iter().find(|collection| collection.id.0 == id) else {
            return Err(format!("There is no drillhole collection with id {id}. get_project lists them."));
        };
        if !collection.state.loaded || collection.state.deferred.is_some() {
            return Err(format!("The drillholes {} are not loaded. Load them with set_visibility, then ask again.", collection.name));
        }
        let dataset = &collection.dataset;
        let fields: Vec<Value> = dataset
            .fields
            .iter()
            .map(|field| match &field.kind {
                DrillFieldKind::Numeric { min, max } => json!({ "key": field.key, "label": field.label, "type": "numeric", "min": min, "max": max }),
                DrillFieldKind::Categorical { categories } => json!({ "key": field.key, "label": field.label, "type": "categorical", "categories": categories }),
            })
            .collect();
        let wanted: Option<Vec<&str>> = arguments["holes"].as_array().map(|holes| holes.iter().filter_map(Value::as_str).collect());
        let with_intervals = arguments["intervals"].as_bool().unwrap_or(false);
        let offset = arguments["offset"].as_u64().unwrap_or(0) as usize;
        let limit = arguments["limit"].as_u64().map_or(DEFAULT_HOLE_PAGE, |limit| limit as usize);
        let chosen: Vec<_> = dataset
            .holes
            .iter()
            .filter(|hole| wanted.as_ref().is_none_or(|wanted| wanted.contains(&hole.dhid.as_str())))
            .collect();
        let holes: Vec<Value> = chosen
            .iter()
            .skip(offset)
            .take(limit)
            .map(|hole| {
                let mut value = json!({
                    "hole": hole.dhid,
                    "collar": point(hole.collar),
                    "depth": hole.trace.last().map(|station| station.depth),
                    "diameter": hole.diameter,
                    "interval_count": hole.intervals.len(),
                });
                if with_intervals {
                    value["intervals"] = hole
                        .intervals
                        .iter()
                        .map(|interval| {
                            let values: serde_json::Map<String, Value> = interval
                                .values
                                .iter()
                                .map(|(key, value)| {
                                    let value = match value {
                                        DrillValue::Numeric(number) => json!(number),
                                        DrillValue::Category(category) => json!(category),
                                    };
                                    (key.clone(), value)
                                })
                                .collect();
                            json!({ "from": interval.from, "to": interval.to, "values": values })
                        })
                        .collect();
                }
                value
            })
            .collect();
        Ok(json!({
            "id": id,
            "name": collection.name,
            "fields": fields,
            "hole_count": chosen.len(),
            "offset": offset,
            "holes": holes,
        }))
    }
}

/// One design object, as the tools describe it.
fn object_json(document: &Document, object: &Object) -> Value {
    let mut value = match object {
        Object::Point { pos, .. } => json!({ "type": "point", "position": point(*pos) }),
        Object::Polyline {
            verts, closed, fill, line_weight, ..
        } => {
            let mut value = json!({
                "type": "polyline",
                "closed": closed,
                "vertices": verts.iter().map(|vertex| point(vertex.pos)).collect::<Vec<_>>(),
                "line_weight": line_weight,
            });
            if verts.iter().any(|vertex| vertex.bulge != 0.0) {
                value["bulges"] = json!(verts.iter().map(|vertex| vertex.bulge).collect::<Vec<_>>());
            }
            if *fill != FillStyle::Clear {
                value["fill"] = json!(fill_name(*fill));
            }
            value
        }
        Object::Circle {
            center,
            radius,
            fill,
            line_weight,
            ..
        } => {
            let mut value = json!({ "type": "circle", "centre": point(*center), "radius": radius, "line_weight": line_weight });
            if *fill != FillStyle::Clear {
                value["fill"] = json!(fill_name(*fill));
            }
            value
        }
        Object::Text {
            pos, content, height, rotation, ..
        } => json!({ "type": "text", "position": point(*pos), "text": content, "height": height, "rotation": rotation.to_degrees() }),
        Object::PitDesign {
            verts, line_weight, pit, ..
        } => {
            let mut value = json!({
                "type": "pit_design",
                "name": pit.name,
                "closed": true,
                "vertices": verts.iter().map(|vertex| point(vertex.pos)).collect::<Vec<_>>(),
                "line_weight": line_weight,
                "benches": pit.design().map(|design| design.benches),
                "strings": object.pit_strings().len(),
                "note": "A pit design: get_pit_designs reads it whole, set_pit_design changes it.",
            });
            if verts.iter().any(|vertex| vertex.bulge != 0.0) {
                value["bulges"] = json!(verts.iter().map(|vertex| vertex.bulge).collect::<Vec<_>>());
            }
            value
        }
    };
    value["id"] = json!(object.id().0);
    value["colour"] = json!(object_colour(object.color()));
    if document.is_object_hidden(object.id()) {
        value["hidden"] = json!(true);
    }
    value
}

/// The name the tools give a fill.
pub(super) fn fill_name(fill: FillStyle) -> &'static str {
    match fill {
        FillStyle::Clear => "clear",
        FillStyle::Crosses => "crosses",
        FillStyle::Slashes => "slashes",
        FillStyle::Solid => "solid",
    }
}

/// Whether a block variable's type is named categories rather than numbers.
fn is_categorical(physical_type: &str) -> bool {
    matches!(physical_type, "namedbyte" | "namedshort")
}

/// A surface's measures, added up a triangle at a time.
#[derive(Default)]
struct Measured {
    area: f64,
    plan_area: f64,
    /// The signed volume: negative when the triangles face inwards.
    volume: f64,
    closed: bool,
}

/// Measure `mesh`, taking volumes about `origin`, near it, so the sums keep
/// their precision at mine-grid coordinates.
///
/// Whether it is closed is found without a table of its edges, which for a
/// large topography would be gigabytes: every triangle side `a -> b` adds
/// `h(a, b) - h(b, a)` to a sum, for a well-mixed hash `h`. A side met by
/// another triangle's `b -> a` cancels it exactly, so the sum is zero when
/// every side is shared by a triangle facing the same way. An open edge, or
/// two triangles facing opposite ways, leaves a remainder, which could only
/// come back to zero by a 64-bit coincidence.
fn measure(mesh: &mesh_data::Triangulation, origin: DVec3) -> Measured {
    let vertices = mesh.vertices();
    let position = |index: usize| {
        let vertex = vertices[index];
        DVec3::new(vertex.x, vertex.y, vertex.z) - origin
    };
    let faces: Vec<[usize; 3]> = mesh.face_vertex_indices_iter().collect();
    let (area, plan_area, volume, edges) = faces
        .par_iter()
        .map(|&[a, b, c]| {
            let (pa, pb, pc) = (position(a), position(b), position(c));
            let normal = (pb - pa).cross(pc - pa);
            let edges = side(a, b).wrapping_add(side(b, c)).wrapping_add(side(c, a));
            (normal.length() * 0.5, normal.z.abs() * 0.5, pa.dot(pb.cross(pc)), edges)
        })
        .reduce(
            || (0.0, 0.0, 0.0, 0u64),
            |left, right| (left.0 + right.0, left.1 + right.1, left.2 + right.2, left.3.wrapping_add(right.3)),
        );
    Measured {
        area,
        plan_area,
        volume: volume / 6.0,
        closed: !faces.is_empty() && edges == 0,
    }
}

/// One triangle side's share of the closure sum: see [`measure`].
fn side(from: usize, to: usize) -> u64 {
    mix(((from as u64) << 32) ^ to as u64).wrapping_sub(mix(((to as u64) << 32) ^ from as u64))
}

/// SplitMix64's finaliser: every input bit moves every output bit.
fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Count, missing values, minimum, maximum and mean of a numeric variable,
/// or the number of blocks in each category of a categorical one.
fn variable_statistics(model: &crate::model::formats::block_model_data::BlockModelData, name: &str) -> Result<Value, String> {
    let variable = model.variable(name).ok_or_else(|| format!("There is no variable called {name}."))?;
    let column = model.columns().get(name).ok_or_else(|| format!("{name} has no values to work from."))?;
    if is_categorical(&variable.physical_type) {
        let counts = column.par_map_runs(|_, values| {
            let mut counts: BTreeMap<Option<u32>, usize> = BTreeMap::new();
            for value in values {
                *counts.entry(value.is_finite().then_some(*value as u32)).or_default() += 1;
            }
            counts
        });
        let mut total: BTreeMap<Option<u32>, usize> = BTreeMap::new();
        for run in counts {
            for (code, count) in run {
                *total.entry(code).or_default() += count;
            }
        }
        let missing = total.remove(&None).unwrap_or(0);
        let categories: serde_json::Map<String, Value> = total
            .into_iter()
            .map(|(code, count)| {
                let code = code.unwrap_or_default();
                (variable.strings.get(&code).cloned().unwrap_or_else(|| code.to_string()), json!(count))
            })
            .collect();
        return Ok(json!({ "categories": categories, "missing": missing }));
    }
    let runs = column.par_map_runs(|_, values| {
        let (mut count, mut sum, mut min, mut max) = (0usize, 0.0_f64, f64::INFINITY, f64::NEG_INFINITY);
        for &value in values.iter().filter(|value| value.is_finite()) {
            count += 1;
            sum += value;
            min = min.min(value);
            max = max.max(value);
        }
        (count, sum, min, max, values.len())
    });
    let (count, sum, min, max, all) = runs.into_iter().fold((0, 0.0, f64::INFINITY, f64::NEG_INFINITY, 0), |total, run| {
        (total.0 + run.0, total.1 + run.1, total.2.min(run.2), total.3.max(run.3), total.4 + run.4)
    });
    Ok(json!({
        "count": count,
        "missing": all - count,
        "min": (count > 0).then_some(min),
        "max": (count > 0).then_some(max),
        "mean": (count > 0).then_some(sum / count as f64),
    }))
}

/// The last `limit` activity entries, oldest first.
fn activity(limit: usize, detail_limit: usize) -> Value {
    let (_, entries) = crate::logging::console_snapshot();
    let start = entries.len().saturating_sub(limit);
    let entries: Vec<Value> = entries[start..]
        .iter()
        .map(|entry| {
            let mut value = json!({
                "time": entry.timestamp,
                "severity": format!("{:?}", entry.severity).to_lowercase(),
                "title": entry.title,
                "summary": entry.summary,
                "details": entry.details.iter().take(detail_limit).collect::<Vec<_>>(),
            });
            // One job can log thousands of lines (every constraint a cut
            // skipped); the count says there were more without filling the
            // client's context with them.
            if entry.details.len() > detail_limit {
                value["more_details"] = json!(entry.details.len() - detail_limit);
            }
            value
        })
        .collect();
    json!({ "entries": entries })
}
