//! The tools that start long work: building, cutting, contouring and
//! thinning surfaces, surfacing and classifying point clouds, and estimating
//! block models. Each starts the same job the
//! window's dialog starts, and returns once it is under way; `get_tasks`
//! follows it, and can wait for it to finish.


use serde_json::{Value, json};

use super::{failed, ok, parse_hex, parse_point};
use crate::{
    app::{
        App,
        commands::triangulation::{TerrainBudget, TerrainTinParams},
    },
    model::{LayerId, ObjectId, drill_hole::DrillFieldKind, ground_filter::ClothTerrain, point_cloud::PointCloudId, triangulation::TriangulationId},
    ui::state::{ContourOutputLayer, OreFilterMode, SolidRegion, TriPolylineClipMode, TriSurfaceCutSide, TriSurfaceType, UiCommand},
};

/// How many finished tasks `get_tasks` lists.
const FINISHED_LISTED: usize = 10;

/// The schema of a surface id.
fn surface(description: &str) -> Value {
    json!({ "type": "integer", "minimum": 0, "description": description })
}

/// The schema of a new item's name.
fn name(what: &str) -> Value {
    json!({ "type": "string", "minLength": 1, "description": format!("The name of the new {what}.") })
}

/// The schema of whether to unload the surface worked on once done.
fn unload_source() -> Value {
    json!({ "type": "boolean", "description": "Unload the source surface once the result is in, to save memory. False if left out." })
}

/// A tool that starts work: changes the project, not idempotently.
fn starts(name: &str, title: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": format!("{description} Runs in the background: returns once started, and get_tasks follows it (and can wait for it). The result lands as one undo step; get_project lists it, and get_activity says if it failed."),
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_tasks",
            "title": "Get tasks",
            "description": "List the work Incline Design is doing in the background (loading, building surfaces, optimisation and the rest) with how far along each is, and the last few tasks that finished. Give wait_seconds to wait until nothing is running, or that long, before answering. A finished task may have failed: get_activity says.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wait_seconds": { "type": "number", "minimum": 0, "description": "Wait up to this long for the running tasks to finish. Answers at once if left out. MCP clients often give up on a call after a few minutes, so wait in steps of a minute or so." },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        starts(
            "triangulate",
            "Triangulate design objects",
            "Build a surface from design polylines and points, as Create Triangulation does: the objects are the surface's breaklines and spot heights. solid makes a closed solid from closed polylines (a stack of outlines) instead of an open surface.",
            json!({
                "objects": { "type": "array", "minItems": 1, "items": { "type": "integer", "minimum": 0 }, "description": "Ids of the design objects to build from." },
                "name": name("surface"),
                "solid": { "type": "boolean", "description": "Build a closed solid rather than an open surface. False if left out." },
            }),
            &["objects", "name"],
        ),
        starts(
            "contour_surface",
            "Contour a surface",
            "Draw contour lines of a surface as design polylines, as Generate Contour Lines does: majors every major_interval, minors every minor_interval between them.",
            json!({
                "surface": surface("The surface to contour."),
                "major_interval": { "type": "number", "exclusiveMinimum": 0 },
                "minor_interval": { "type": "number", "minimum": 0, "description": "0 for no minors." },
                "z_min": { "type": "number", "description": "Contour only from this RL. The whole surface if z_min and z_max are left out." },
                "z_max": { "type": "number", "description": "Contour only up to this RL." },
                "labels": { "type": "boolean", "description": "Write each major's RL on it as text. False if left out." },
                "major_colour": { "type": "string", "description": "#rrggbb. The dialog's if left out." },
                "minor_colour": { "type": "string", "description": "#rrggbb. The dialog's if left out." },
                "layer": { "type": "integer", "minimum": 0, "description": "An existing design layer to draw on." },
                "new_layer": { "type": "string", "minLength": 1, "description": "The name of a new design layer to draw on, when layer is left out." },
            }),
            &["surface", "major_interval", "minor_interval"],
        ),
        starts(
            "cut_surface_by_polyline",
            "Cut a surface by a polyline",
            "Cut a surface along a closed polyline seen from above, as Cut Triangulation by Polyline does, keeping the part inside or outside it as a new surface.",
            json!({
                "surface": surface("The surface to cut."),
                "polyline": { "type": "integer", "minimum": 0, "description": "The id of a closed design polyline." },
                "keep": { "type": "string", "enum": ["inside", "outside"], "description": "inside if left out." },
                "name": name("surface"),
                "unload_source": unload_source(),
            }),
            &["surface", "polyline", "name"],
        ),
        starts(
            "cut_surface_by_elevation",
            "Cut a surface between two RLs",
            "Keep the part of a surface between two RLs, cut flat at both, as a new surface, as Cut Triangulation by Z Range does.",
            json!({
                "surface": surface("The surface to cut."),
                "z_min": { "type": "number" },
                "z_max": { "type": "number" },
                "name": name("surface"),
                "unload_source": unload_source(),
            }),
            &["surface", "z_min", "z_max", "name"],
        ),
        starts(
            "trim_surface",
            "Trim a surface to a topography",
            "Trim a surface where it passes above or below a topography, as Trim to Topology does, keeping the rest as a new surface.",
            json!({
                "surface": surface("The surface to trim."),
                "topography": surface("The surface it is trimmed against."),
                "keep": { "type": "string", "enum": ["below", "above"], "description": "The side of the topography to keep. below if left out." },
                "name": name("surface"),
                "unload_source": unload_source(),
            }),
            &["surface", "topography", "name"],
        ),
        starts(
            "build_solid",
            "Build a solid from two surfaces",
            "Build the closed solid between a design surface and the topography it meets, as Build Solid from Surfaces does: cut is where the design lies below the topography (a pit), fill where it lies above (a dump or stockpile).",
            json!({
                "design": surface("The design surface."),
                "topography": surface("The topography."),
                "region": { "type": "string", "enum": ["cut", "fill"] },
                "name": name("solid"),
            }),
            &["design", "topography", "region", "name"],
        ),
        starts(
            "simplify_surface",
            "Simplify a surface",
            "Thin a surface to fewer triangles as a new surface, as Simplify does, keeping its shape as closely as it can.",
            json!({
                "surface": surface("The surface to thin."),
                "keep": { "type": "number", "exclusiveMinimum": 0, "maximum": 1, "description": "The share of its triangles to keep, 0 to 1." },
                "max_deviation": { "type": "number", "exclusiveMinimum": 0, "description": "Stop before any point moves further than this from the original. No bound if left out." },
                "name": name("surface"),
                "unload_source": unload_source(),
            }),
            &["surface", "keep", "name"],
        ),
        starts(
            "cut_topography_by_pit",
            "Cut a topography to a pit shell",
            "Trim a topography to where it lies outside a pit shell's footprint, as Cut Topology to Pit Shell does, ready to merge the pit in.",
            json!({
                "topography": surface("The topography to cut."),
                "pit": surface("The pit shell."),
                "name": name("surface"),
                "unload_source": unload_source(),
            }),
            &["topography", "pit", "name"],
        ),
        starts(
            "include_solid_in_topography",
            "Merge a pit or stockpile into a topography",
            "Replace the part of a topography a pit or stockpile solid covers with the solid's own surface, as Include Pit/Stockpile Solid does, giving the topography with the pit dug or the stockpile placed.",
            json!({
                "topography": surface("The topography."),
                "solid": surface("The pit or stockpile solid."),
                "name": name("surface"),
                "as_two": { "type": "boolean", "description": "Keep the result as two surfaces, the trimmed topography and the shell, rather than one. False if left out." },
                "hide_old": { "type": "boolean", "description": "Hide the topography and solid it was made from. True if left out." },
            }),
            &["topography", "solid", "name"],
        ),
        starts(
            "surface_point_cloud",
            "Surface a point cloud",
            "Build a terrain surface from a point cloud, as Point Cloud TIN does: the points are thinned to a vertex budget, keeping more where the ground is complex, and triangulated.",
            json!({
                "cloud": { "type": "integer", "minimum": 0, "description": "The point cloud's id." },
                "name": name("surface"),
                "percent": { "type": "number", "exclusiveMinimum": 0, "maximum": 100, "description": "Vertex budget as a percentage of the points. The dialog's (1 % unless changed) if neither this nor vertices is given." },
                "vertices": { "type": "integer", "minimum": 3, "description": "Vertex budget as a count, in place of percent." },
                "max_edge": { "type": "number", "minimum": 0, "description": "Leave out triangles with a side longer than this. 0 (no bound) if left out." },
                "fill_holes_up_to": { "type": "number", "minimum": 0, "description": "Bridge gaps in the points narrower than this. 0 if left out." },
                "ground_only": { "type": "boolean", "description": "Build from the points classified as ground only. True if left out; ignored by an unclassified cloud." },
            }),
            &["cloud", "name"],
        ),
        starts(
            "classify_point_clouds",
            "Classify point clouds",
            "Classify each point cloud's points into ground, vegetation and noise, as Classify Point Clouds does, rewriting their classes in place.",
            json!({
                "clouds": { "type": "array", "minItems": 1, "items": { "type": "integer", "minimum": 0 } },
                "terrain": { "type": "string", "enum": ["steep", "relief", "flat"], "description": "steep: pit walls, benches and stockpiles; relief: rolling ground and dumps; flat: flat ground with large structures. The dialog's if left out." },
            }),
            &["clouds"],
        ),
        starts(
            "join_point_clouds",
            "Join point clouds",
            "Join several loaded point clouds into one new cloud, as Join Point Clouds does.",
            json!({
                "clouds": { "type": "array", "minItems": 2, "items": { "type": "integer", "minimum": 0 } },
                "name": name("point cloud"),
                "remove_sources": { "type": "boolean", "description": "Delete the clouds joined once the new one is in. False if left out." },
            }),
            &["clouds", "name"],
        ),
        starts(
            "estimate_block_model",
            "Estimate a block model",
            "Estimate a regular block model from drillhole intervals by ordinary kriging, as Create Block Model does: each block's value of each variable from the nearest samples, under a spherical variogram.",
            json!({
                "drillholes": { "type": "integer", "minimum": 0, "description": "The drillhole collection's id." },
                "variables": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "Numeric fields to estimate, by key, from get_drillholes." },
                "name": name("block model"),
                "block_size": { "type": "array", "items": { "type": "number", "exclusiveMinimum": 0 }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] size of a block." },
                "min": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "The model's lowest corner. The drillholes' extent if min and max are left out." },
                "max": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "The model's highest corner." },
                "holes": { "type": "array", "items": { "type": "string" }, "description": "Only these holes, by hole id. Every hole if left out." },
                "range": { "type": "number", "exclusiveMinimum": 0, "description": "Variogram range. The dialog's if left out." },
                "sill": { "type": "number", "exclusiveMinimum": 0, "description": "Variogram sill. The dialog's if left out." },
                "nugget": { "type": "number", "minimum": 0, "description": "Variogram nugget. The dialog's if left out." },
                "min_samples": { "type": "integer", "minimum": 1, "description": "The dialog's if left out." },
                "max_samples": { "type": "integer", "minimum": 1, "maximum": 64, "description": "The dialog's if left out." },
            }),
            &["drillholes", "variables", "name", "block_size"],
        ),
        starts(
            "ore_surface",
            "Surface the ore in a block model",
            "Wrap the blocks of a block model whose variable is at least, at most or between given values in a closed surface, as Create Ore Triangulation does.",
            json!({
                "block_model": { "type": "integer", "minimum": 0 },
                "variable": { "type": "string" },
                "at_least": { "type": "number", "description": "Blocks with at least this value. Give at_least, at_most, or both for a band." },
                "at_most": { "type": "number", "description": "Blocks with at most this value." },
                "name": name("surface"),
            }),
            &["block_model", "variable", "name"],
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_job_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let command = match name {
            "get_tasks" => return Some(ok(&self.mcp_tasks())),
            "triangulate" => self.mcp_triangulate(arguments),
            "contour_surface" => self.mcp_contour(arguments),
            "cut_surface_by_polyline" => self.mcp_cut_by_polyline(arguments),
            "cut_surface_by_elevation" => self.mcp_cut_by_elevation(arguments),
            "trim_surface" => self.mcp_trim(arguments),
            "build_solid" => self.mcp_build_solid(arguments),
            "simplify_surface" => self.mcp_simplify(arguments),
            "cut_topography_by_pit" => (|| {
                Ok(UiCommand::ExecuteCutTopologyByPitShell {
                    topology_id: self.mcp_surface_id(arguments, "topography")?,
                    pit_shell_id: self.mcp_surface_id(arguments, "pit")?,
                    name: required_name(arguments)?,
                    unload_source: arguments["unload_source"].as_bool().unwrap_or(false),
                })
            })(),
            "include_solid_in_topography" => (|| {
                Ok(UiCommand::ExecuteIncludeSolidInTopology {
                    topology_id: self.mcp_surface_id(arguments, "topography")?,
                    shape_id: self.mcp_surface_id(arguments, "solid")?,
                    name: required_name(arguments)?,
                    save_as_two: arguments["as_two"].as_bool().unwrap_or(false),
                    hide_old: arguments["hide_old"].as_bool().unwrap_or(true),
                })
            })(),
            "surface_point_cloud" => self.mcp_surface_point_cloud(arguments),
            "classify_point_clouds" => self.mcp_classify_point_clouds(arguments),
            "join_point_clouds" => self.mcp_join_point_clouds(arguments),
            "estimate_block_model" => self.mcp_estimate_block_model(arguments),
            "ore_surface" => self.mcp_ore_surface(arguments),
            _ => return None,
        };
        Some(match command {
            Ok(command) => match self.handle_reported_ui_command(command) {
                Ok(()) => ok(&json!({ "started": name, "running": self.mcp_tasks()["running"] })),
                Err(error) => failed(format!("{error:#}")),
            },
            Err(error) => failed(error),
        })
    }

    /// What is running, and what finished last.
    pub(super) fn mcp_tasks(&self) -> Value {
        let running: Vec<Value> = self
            .background_tasks
            .reported
            .iter()
            .map(|task| {
                let progress = task.progress.snapshot();
                json!({
                    "task": task.label.trim_end_matches(['.', '…', ' ']),
                    "seconds": task.started.elapsed().as_secs_f64().round(),
                    "fraction_done": progress.as_ref().map(|progress| progress.fraction),
                    "items": progress.and_then(|progress| progress.units),
                })
            })
            .collect();
        let finished: Vec<Value> = self
            .editor
            .task_history
            .iter()
            .take(FINISHED_LISTED)
            .map(|task| json!({ "task": task.text, "seconds": task.duration.as_secs_f64() }))
            .collect();
        json!({ "running": running, "finished_newest_first": finished })
    }

    /// The id of a surface that is in the project.
    fn mcp_surface_id(&self, arguments: &Value, key: &str) -> Result<TriangulationId, String> {
        let id = arguments[key].as_u64().ok_or_else(|| format!("{key} needs a surface's id."))?;
        self.triangulations
            .iter()
            .find(|surface| surface.id.0 == id)
            .map(|surface| surface.id)
            .ok_or_else(|| format!("There is no surface with id {id}. get_project lists them."))
    }

    fn mcp_triangulate(&self, arguments: &Value) -> Result<UiCommand, String> {
        let document = self.workspace.active_document().ok_or("No project is open.")?;
        let ids: Vec<ObjectId> = arguments["objects"]
            .as_array()
            .ok_or("triangulate needs objects.")?
            .iter()
            .filter_map(Value::as_u64)
            .map(ObjectId)
            .collect();
        if let Some(missing) = ids.iter().find(|id| document.get_object(**id).is_none()) {
            return Err(format!("There is no design object with id {} in a loaded layer.", missing.0));
        }
        Ok(UiCommand::ExecuteCreateTriangulation {
            name: required_name(arguments)?,
            object_ids: ids,
            surface_type: if arguments["solid"].as_bool().unwrap_or(false) {
                TriSurfaceType::SolidClosed
            } else {
                TriSurfaceType::Surface
            },
        })
    }

    fn mcp_contour(&self, arguments: &Value) -> Result<UiCommand, String> {
        let tri_id = self.mcp_surface_id(arguments, "surface")?;
        let major_interval = arguments["major_interval"]
            .as_f64()
            .filter(|interval| *interval > 0.0)
            .ok_or("major_interval must be above 0.")?;
        let minor_interval = arguments["minor_interval"]
            .as_f64()
            .filter(|interval| *interval >= 0.0)
            .ok_or("minor_interval must be 0 or more.")?;
        let colour = |key: &str, default: [f32; 4]| match arguments[key].as_str() {
            None => Ok(default),
            Some(text) => parse_hex(text).ok_or_else(|| format!("{key} must be #rrggbb.")),
        };
        let z_range = match (arguments["z_min"].as_f64(), arguments["z_max"].as_f64()) {
            (None, None) => None,
            (Some(min), Some(max)) if min < max => Some((min, max)),
            _ => return Err("Give both z_min and z_max, z_min below z_max, or neither.".to_owned()),
        };
        let output_layer = match (arguments["layer"].as_u64(), arguments["new_layer"].as_str()) {
            (Some(layer), _) => {
                let document = self.workspace.active_document().ok_or("No project is open.")?;
                document.layer(LayerId(layer)).ok_or_else(|| format!("There is no design layer with id {layer}."))?;
                ContourOutputLayer::Existing(LayerId(layer))
            }
            (None, Some(name)) => ContourOutputLayer::New(name.to_owned()),
            (None, None) => return Err("Give a layer to draw on, or a new_layer name.".to_owned()),
        };
        Ok(UiCommand::ExecuteContourTriangulation {
            tri_id,
            major_interval,
            minor_interval,
            major_color: colour("major_colour", self.editor.tri_contour_major_color)?,
            minor_color: colour("minor_colour", self.editor.tri_contour_minor_color)?,
            z_range,
            contour_labels: arguments["labels"].as_bool().unwrap_or(false),
            output_layer,
        })
    }

    fn mcp_cut_by_polyline(&self, arguments: &Value) -> Result<UiCommand, String> {
        let polyline = ObjectId(arguments["polyline"].as_u64().ok_or("polyline needs a design polyline's id.")?);
        self.workspace
            .active_document()
            .and_then(|document| document.get_object(polyline))
            .ok_or_else(|| format!("There is no design object with id {} in a loaded layer.", polyline.0))?;
        Ok(UiCommand::ExecuteCutTriangulationByPolyline {
            tri_id: self.mcp_surface_id(arguments, "surface")?,
            polyline_id: polyline,
            mode: match arguments["keep"].as_str() {
                None | Some("inside") => TriPolylineClipMode::KeepInside,
                Some("outside") => TriPolylineClipMode::KeepOutside,
                Some(_) => return Err("keep is inside or outside.".to_owned()),
            },
            name: required_name(arguments)?,
            unload_source: arguments["unload_source"].as_bool().unwrap_or(false),
        })
    }

    fn mcp_cut_by_elevation(&self, arguments: &Value) -> Result<UiCommand, String> {
        let (Some(z_min), Some(z_max)) = (arguments["z_min"].as_f64(), arguments["z_max"].as_f64()) else {
            return Err("cut_surface_by_elevation needs z_min and z_max.".to_owned());
        };
        if z_min >= z_max {
            return Err("z_min must be below z_max.".to_owned());
        }
        Ok(UiCommand::ExecuteCutTriangulationByZ {
            tri_id: self.mcp_surface_id(arguments, "surface")?,
            z_min,
            z_max,
            name: required_name(arguments)?,
            unload_source: arguments["unload_source"].as_bool().unwrap_or(false),
        })
    }

    fn mcp_trim(&self, arguments: &Value) -> Result<UiCommand, String> {
        Ok(UiCommand::ExecuteCutTriangulationBySurface {
            target_id: self.mcp_surface_id(arguments, "surface")?,
            reference_id: self.mcp_surface_id(arguments, "topography")?,
            side: match arguments["keep"].as_str() {
                None | Some("below") => TriSurfaceCutSide::CutTop,
                Some("above") => TriSurfaceCutSide::CutBottom,
                Some(_) => return Err("keep is below or above.".to_owned()),
            },
            name: required_name(arguments)?,
            unload_source: arguments["unload_source"].as_bool().unwrap_or(false),
        })
    }

    fn mcp_build_solid(&self, arguments: &Value) -> Result<UiCommand, String> {
        Ok(UiCommand::ExecuteBuildSolidFromSurfaces {
            design_id: self.mcp_surface_id(arguments, "design")?,
            topography_id: self.mcp_surface_id(arguments, "topography")?,
            region: match arguments["region"].as_str() {
                Some("cut") => SolidRegion::Cut,
                Some("fill") => SolidRegion::Fill,
                _ => return Err("region is cut or fill.".to_owned()),
            },
            name: required_name(arguments)?,
        })
    }

    fn mcp_simplify(&self, arguments: &Value) -> Result<UiCommand, String> {
        let keep = arguments["keep"]
            .as_f64()
            .filter(|keep| *keep > 0.0 && *keep <= 1.0)
            .ok_or("keep is a share of the triangles, above 0 and at most 1.")?;
        Ok(UiCommand::SimplifyTriangulation {
            surface: self.mcp_surface_id(arguments, "surface")?,
            keep,
            max_deviation: arguments["max_deviation"].as_f64(),
            name: required_name(arguments)?,
            unload_source: arguments["unload_source"].as_bool().unwrap_or(false),
        })
    }

    /// The point clouds `key` lists, each in the project.
    fn mcp_point_cloud_ids(&self, arguments: &Value, key: &str) -> Result<Vec<PointCloudId>, String> {
        let ids = arguments[key].as_array().ok_or_else(|| format!("{key} needs a list of point cloud ids."))?;
        ids.iter()
            .map(|id| {
                let id = id.as_u64().ok_or("Point cloud ids are whole numbers.")?;
                self.point_clouds
                    .iter()
                    .find(|cloud| cloud.id.0 == id)
                    .map(|cloud| cloud.id)
                    .ok_or_else(|| format!("There is no point cloud with id {id}. get_project lists them."))
            })
            .collect()
    }

    fn mcp_surface_point_cloud(&self, arguments: &Value) -> Result<UiCommand, String> {
        let id = arguments["cloud"].as_u64().ok_or("surface_point_cloud needs the cloud's id.")?;
        let cloud = self
            .point_clouds
            .iter()
            .find(|cloud| cloud.id.0 == id)
            .ok_or_else(|| format!("There is no point cloud with id {id}. get_project lists them."))?;
        let editor = &self.editor;
        let budget = match (arguments["percent"].as_f64(), arguments["vertices"].as_u64()) {
            (Some(percent), _) if percent > 0.0 && percent <= 100.0 => TerrainBudget::Percent(percent),
            (Some(_), _) => return Err("percent is above 0 and at most 100.".to_owned()),
            (None, Some(count)) => TerrainBudget::Count(usize::try_from(count).unwrap_or(usize::MAX)),
            (None, None) if editor.point_cloud_tin_budget_is_percent => TerrainBudget::Percent(editor.point_cloud_tin_percent),
            (None, None) => TerrainBudget::Count(editor.point_cloud_tin_limit as usize),
        };
        Ok(UiCommand::ExecutePointCloudTin {
            cloud_id: cloud.id,
            params: TerrainTinParams {
                name: required_name(arguments)?,
                budget,
                max_edge: arguments["max_edge"].as_f64().unwrap_or(0.0),
                sampler: editor.point_cloud_tin_sampler,
                candidate_multiplier: editor.point_cloud_tin_candidate_mult,
                hole_fill_distance: arguments["fill_holes_up_to"].as_f64().unwrap_or(0.0),
                ground_only: arguments["ground_only"].as_bool().unwrap_or(true),
            },
        })
    }

    fn mcp_classify_point_clouds(&self, arguments: &Value) -> Result<UiCommand, String> {
        let mut params = self.editor.point_cloud_classify_params;
        match arguments["terrain"].as_str() {
            None => {}
            Some("steep") => params.terrain = ClothTerrain::Steep,
            Some("relief") => params.terrain = ClothTerrain::Relief,
            Some("flat") => params.terrain = ClothTerrain::Flat,
            Some(_) => return Err("terrain is steep, relief or flat.".to_owned()),
        }
        Ok(UiCommand::ExecutePointCloudClassify {
            cloud_ids: self.mcp_point_cloud_ids(arguments, "clouds")?,
            params,
        })
    }

    fn mcp_join_point_clouds(&self, arguments: &Value) -> Result<UiCommand, String> {
        let cloud_ids = self.mcp_point_cloud_ids(arguments, "clouds")?;
        if cloud_ids.len() < 2 {
            return Err("Join at least two point clouds.".to_owned());
        }
        Ok(UiCommand::ExecutePointCloudJoin {
            cloud_ids,
            name: required_name(arguments)?,
            remove_sources: arguments["remove_sources"].as_bool().unwrap_or(false),
        })
    }

    fn mcp_estimate_block_model(&self, arguments: &Value) -> Result<UiCommand, String> {
        let id = arguments["drillholes"].as_u64().ok_or("estimate_block_model needs the drillholes' id.")?;
        let collection = self
            .drill_holes
            .iter()
            .find(|collection| collection.id.0 == id)
            .ok_or_else(|| format!("There is no drillhole collection with id {id}. get_project lists them."))?;
        if !collection.state.loaded || collection.state.deferred.is_some() {
            return Err(format!("The drillholes {} are not loaded. Load them with set_visibility first.", collection.name));
        }
        let dataset = &collection.dataset;
        let variables: Vec<String> = arguments["variables"]
            .as_array()
            .ok_or("variables needs a list of field keys.")?
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
        for variable in &variables {
            match dataset.fields.iter().find(|field| field.key == *variable) {
                Some(field) if matches!(field.kind, DrillFieldKind::Numeric { .. }) => {}
                Some(_) => return Err(format!("{variable} is categorical; only numeric fields can be estimated.")),
                None => return Err(format!("The drillholes have no field {variable}. get_drillholes lists them.")),
            }
        }
        let cell = parse_point(&arguments["block_size"])
            .filter(|cell| cell.min_element() > 0.0)
            .ok_or("block_size must be [x, y, z], each above 0.")?;
        let (lower, upper) = match (parse_point(&arguments["min"]), parse_point(&arguments["max"])) {
            (Some(lower), Some(upper)) if lower.cmplt(upper).all() => (lower, upper),
            (None, None) => dataset.bounds.ok_or("The drillholes have no extent: give min and max.")?,
            _ => return Err("Give both min and max, min below max on every axis, or neither.".to_owned()),
        };
        let holes = match arguments["holes"].as_array() {
            None => None,
            Some(names) => Some(
                names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|name| dataset.holes.iter().position(|hole| hole.dhid == name).ok_or_else(|| format!("There is no hole {name}.")))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        let editor = &self.editor;
        let positive = |key: &str, default: f64| match arguments[key].as_f64() {
            None => Ok(default),
            Some(value) if value > 0.0 => Ok(value),
            Some(_) => Err(format!("{key} must be above 0.")),
        };
        let samples = |key: &str, default: u32| arguments[key].as_u64().map_or(default, |count| u32::try_from(count).unwrap_or(u32::MAX));
        let (min_samples, max_samples) = (samples("min_samples", editor.kriging_min_samples), samples("max_samples", editor.kriging_max_samples));
        if min_samples == 0 || min_samples > max_samples || max_samples > 64 {
            return Err("min_samples must be at least 1 and no more than max_samples, which is at most 64.".to_owned());
        }
        Ok(UiCommand::ExecuteCreateBlockModel {
            drill_hole_id: collection.id,
            holes,
            variables,
            name: required_name(arguments)?,
            lower,
            upper,
            cell,
            range: positive("range", editor.kriging_range)?,
            sill: positive("sill", editor.kriging_sill)?,
            nugget: arguments["nugget"].as_f64().filter(|nugget| *nugget >= 0.0).unwrap_or(editor.kriging_nugget),
            min_samples,
            max_samples,
        })
    }

    fn mcp_ore_surface(&self, arguments: &Value) -> Result<UiCommand, String> {
        let id = arguments["block_model"].as_u64().ok_or("ore_surface needs the block model's id.")?;
        let model = self
            .block_models
            .iter()
            .find(|model| model.id.0 == id)
            .ok_or_else(|| format!("There is no block model with id {id}. get_project lists them."))?;
        let variable = arguments["variable"].as_str().ok_or("ore_surface needs a variable.")?;
        if model.model.variable(variable).is_none() {
            return Err(format!("The block model has no variable {variable}. get_block_model lists them."));
        }
        let (mode, min, max) = match (arguments["at_least"].as_f64(), arguments["at_most"].as_f64()) {
            (Some(min), Some(max)) if min <= max => (OreFilterMode::Between, min, max),
            (Some(_), Some(_)) => return Err("at_least must not be above at_most.".to_owned()),
            (Some(min), None) => (OreFilterMode::GreaterOrEqual, min, min),
            (None, Some(max)) => (OreFilterMode::LessOrEqual, max, max),
            (None, None) => return Err("Give at_least, at_most, or both.".to_owned()),
        };
        Ok(UiCommand::ExecuteCreateOreTriangulation {
            block_model_id: model.id,
            variable: variable.to_owned(),
            mode,
            min,
            max,
            name: required_name(arguments)?,
        })
    }
}

/// A new item's name, from the arguments.
pub(super) fn required_name(arguments: &Value) -> Result<String, String> {
    arguments["name"]
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Give the new item a name.".to_owned())
}
