//! The tools that look at the scene, and the one that changes what the
//! window shows the user.
//!
//! A look is stateless: `render_view` takes the whole view as arguments,
//! draws it offscreen and hands back the picture, so nothing the AI does to
//! look moves the user's camera or touches their selection, and the same
//! arguments give the same picture. `show_in_window` is the one way the AI
//! changes the window's view, for when it means to show the user something.

use std::collections::HashSet;

use glam::DVec3;
use serde_json::{Value, json};

use super::{Target, entity, failed, items_schema, object_points, ok};
use crate::{
    app::App,
    model::{ItemRef, SceneEntityId},
    rendering::graphics::{
        frame::SceneContent,
        offscreen::{ViewAim, ViewRequest, ViewportPixels},
    },
    ui::state::{StandardView, UiCommand},
};

/// A picture's longest side unless asked for another.
const DEFAULT_SIZE: u32 = 1280;
/// The smallest picture worth drawing, and the largest a side may be.
const MIN_SIZE: u32 = 64;
const MAX_SIZE: u32 = 4096;
/// The angle a view looks from when none is given: towards north-north-east,
/// down at 40 degrees, which shows a pit's or dump's shape at a glance.
const DEFAULT_AZIMUTH: f64 = 30.0;
const DEFAULT_ELEVATION: f64 = 40.0;

/// How thick a section's slab is in a picture unless asked for another.
const DEFAULT_SECTION_WIDTH: f64 = 3.0;

const NAMED_VIEWS: [&str; 7] = ["plan", "below", "north", "south", "east", "west", "window"];

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "set_visibility",
            "title": "Show or hide an item",
            "description": "Show or hide one item in the project, as the eye beside it in the explorer does. This is part of the project, saved with it and seen by the user; to leave items out of one picture, use render_view's only, show and hide instead. Showing an item that is not loaded loads it first, which for a large item takes a moment: get_project says when it is loaded. Ids and kinds are as get_project reports them.",
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
        json!({
            "name": "render_view",
            "title": "Look at the project",
            "description": "Draw a picture of the project's 3D scene from a view given entirely by the arguments, to see what it looks like or what a change did. It is drawn on its own: the user's window, camera and selection are not touched, and the same arguments give the same picture. With no arguments it frames everything shown, looking north-north-east and down at 40 degrees. Only loaded items can be drawn (set_loaded loads one).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "frame": items_schema("What to fit in the picture. Everything drawn if left out. A raster cannot be framed: frame the surface it is draped on."),
                    "box": {
                        "type": "object",
                        "description": "A box to fit in the picture, in place of frame: its corners as [x, y, z].",
                        "properties": {
                            "min": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                            "max": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                        },
                        "required": ["min", "max"],
                        "additionalProperties": false,
                    },
                    "view": {
                        "type": "string",
                        "enum": NAMED_VIEWS,
                        "description": "plan looks straight down, north up; below looks up from underneath; north, south, east and west look level from that side towards the opposite one; window is the user's own view, exactly as their window frames it (frame, box, azimuth, elevation and perspective are then ignored). Leave out to use azimuth and elevation.",
                    },
                    "azimuth": { "type": "number", "description": format!("The compass bearing the camera looks towards, in degrees: 0 north, 90 east. {DEFAULT_AZIMUTH} if left out.") },
                    "elevation": { "type": "number", "minimum": -90, "maximum": 90, "description": format!("How far the camera looks down, in degrees: 90 straight down, 0 level, negative up from below. {DEFAULT_ELEVATION} if left out.") },
                    "perspective": { "type": "boolean", "description": "Draw in perspective rather than orthographic (true to scale everywhere). False if left out." },
                    "section": json!({
                        "type": "object",
                        "description": "Look square-on at a vertical section along a line in plan instead, showing what lies within a slab round it, framed on the line. frame, box, view, azimuth, elevation and perspective are then ignored.",
                        "properties": {
                            "from": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] one end of the section line." },
                            "to": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] the other end." },
                            "width": { "type": "number", "exclusiveMinimum": 0, "description": format!("How thick the slab is. {DEFAULT_SECTION_WIDTH} if left out.") },
                        },
                        "required": ["from", "to"],
                        "additionalProperties": false,
                    }),
                    "only": items_schema("Draw only these, leaving everything else out. A design layer stands for its objects."),
                    "show": items_schema("Draw these too, though they are hidden in the project."),
                    "hide": items_schema("Leave these out of the picture."),
                    "highlight": items_schema("Draw these as selected, to pick them out."),
                    "width": { "type": "integer", "minimum": MIN_SIZE, "maximum": MAX_SIZE, "description": format!("The picture's width in pixels. {DEFAULT_SIZE} if neither size is given; the other side follows a 16:10 shape (the window's for view window) when only one is.") },
                    "height": { "type": "integer", "minimum": MIN_SIZE, "maximum": MAX_SIZE },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
        json!({
            "name": "show_in_window",
            "title": "Show the user something",
            "description": "Change what the user's window shows: frame items, turn the view, select items, or switch workspace. This is the only tool that moves the user's view or selection, so use it when they have asked to be shown something, not to look for yourself (render_view does that without disturbing them). The window's own selection is read with get_selection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "frame": items_schema("Frame these in the window, keeping its angle unless view is given. Everything shown if given empty."),
                    "view": { "type": "string", "enum": ["plan", "below", "north", "south", "east", "west"], "description": "Turn the window's view to look from this direction, as the view cube does." },
                    "select": items_schema("Select these in the window, in place of what is selected. Given empty, clears the selection. Locked items cannot be selected and are counted as skipped."),
                    "workspace": { "type": "string", "enum": ["production", "drill_and_blast", "geology", "survey", "planning"], "description": "Switch to this workspace, as its tabs do." },
                    "page": { "type": "string", "enum": ["setup", "reserves", "calendar", "gantt", "charts", "animate"], "description": "In planning, show this page." },
                    "schedule_hours": { "type": "number", "minimum": 0, "description": "Move the schedule's playback to this many hours from its start, for animate." },
                    "section": json!({
                        "type": "object",
                        "description": "Put the window in section view along a line in plan, as the section tool does.",
                        "properties": {
                            "from": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] one end of the section line." },
                            "to": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] the other end." },
                            "width": { "type": "number", "exclusiveMinimum": 0, "description": "How thick a slab is shown. The tool's own if left out." },
                        },
                        "required": ["from", "to"],
                        "additionalProperties": false,
                    }),
                    "leave_section": { "type": "boolean", "description": "Take the window out of section view." },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
    ]
}

/// The editor state a requested view is drawn with in place of the
/// window's: what is hidden and what is highlighted.
struct SceneFilter {
    hidden: HashSet<SceneEntityId>,
    selected: HashSet<SceneEntityId>,
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_view_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        Some(match name {
            "set_visibility" => self.mcp_set_visibility(arguments),
            "render_view" => self.mcp_render_view(arguments).unwrap_or_else(failed),
            "show_in_window" => self.mcp_show_in_window(arguments).unwrap_or_else(failed),
            _ => return None,
        })
    }

    fn mcp_set_visibility(&mut self, arguments: &Value) -> Value {
        let (Some(kind), Some(id), Some(visible)) = (arguments["kind"].as_str(), arguments["id"].as_u64(), arguments["visible"].as_bool()) else {
            return failed("set_visibility needs kind, id and visible.");
        };
        let command = match self.mcp_target(kind, id) {
            Ok(Target::Item(item)) => UiCommand::SetItemVisible(item, visible),
            Ok(Target::Layer(layer)) => UiCommand::SetLayerVisible(layer, visible),
            Ok(Target::Object(_)) => return failed("set_visibility shows and hides whole items and layers, not single design objects."),
            Err(error) => return failed(error),
        };
        self.handle_ui_commands(vec![command]);
        ok(&json!({ "kind": kind, "id": id, "visible": visible }))
    }

    fn mcp_render_view(&mut self, arguments: &Value) -> Result<Value, String> {
        let named = arguments["view"].as_str();
        if let Some(name) = named
            && !NAMED_VIEWS.contains(&name)
        {
            return Err(format!("view is one of {}.", NAMED_VIEWS.join(", ")));
        }
        let window = named == Some("window");
        if window && self.headless {
            return Err("This Incline has no window (it was started with mcp --headless), so there is no view of the user's to draw.".to_owned());
        }
        let size = self.mcp_view_size(arguments, window)?;
        let aim = if let Some((from, to, width)) = section(&arguments["section"])? {
            ViewAim::Section {
                from,
                to,
                width: width.unwrap_or(DEFAULT_SECTION_WIDTH),
            }
        } else if window {
            ViewAim::Window
        } else {
            let (forward, up) = view_direction(named, arguments)?;
            ViewAim::Frame {
                forward,
                up,
                bounds: self.mcp_view_bounds(arguments)?,
            }
        };
        let request = ViewRequest {
            aim,
            perspective: arguments["perspective"].as_bool().unwrap_or(false),
            size,
        };
        let mut filter = self.mcp_scene_filter(arguments, window)?;
        let pixels = self.draw_mcp_view(&request, &mut filter)?;
        let described = match (&request.aim, named) {
            (ViewAim::Section { width, .. }, _) => format!("a section {width} thick"),
            (ViewAim::Window, _) => "the user's view".to_owned(),
            (ViewAim::Frame { .. }, Some(name)) => format!("a {name} view"),
            (ViewAim::Frame { .. }, None) => {
                let (azimuth, elevation) = angles(arguments);
                format!("a view looking towards {azimuth:.0} degrees, {elevation:.0} degrees down")
            }
        };
        picture(&pixels, &described)
    }

    /// The picture's size: as asked, or the default shape round what was.
    fn mcp_view_size(&self, arguments: &Value, window: bool) -> Result<(u32, u32), String> {
        let side = |key: &str| -> Result<Option<u32>, String> {
            match arguments[key].as_u64() {
                None => Ok(None),
                Some(value) if (u64::from(MIN_SIZE)..=u64::from(MAX_SIZE)).contains(&value) => Ok(Some(value as u32)),
                Some(_) => Err(format!("{key} is from {MIN_SIZE} to {MAX_SIZE} pixels.")),
            }
        };
        let shape = if window {
            let (width, height) = self.graphics.as_ref().map_or((16.0, 10.0), |graphics| graphics.screen_size_pub());
            f64::from(width.max(1.0)) / f64::from(height.max(1.0))
        } else {
            1.6
        };
        let fit = |value: f64| (value.round() as u32).clamp(MIN_SIZE, MAX_SIZE);
        Ok(match (side("width")?, side("height")?) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => (width, fit(f64::from(width) / shape)),
            (None, Some(height)) => (fit(f64::from(height) * shape), height),
            (None, None) if shape >= 1.0 => (DEFAULT_SIZE, fit(f64::from(DEFAULT_SIZE) / shape)),
            (None, None) => (fit(f64::from(DEFAULT_SIZE) * shape), DEFAULT_SIZE),
        })
    }

    /// The box a view frames: `box`, the items in `frame`, or `None` for
    /// everything drawn.
    fn mcp_view_bounds(&self, arguments: &Value) -> Result<Option<(DVec3, DVec3)>, String> {
        if !arguments["box"].is_null() {
            let corner = |key: &str| super::parse_point(&arguments["box"][key]).ok_or_else(|| format!("box needs {key} as [x, y, z]."));
            let (min, max) = (corner("min")?, corner("max")?);
            return Ok(Some((min.min(max), min.max(max))));
        }
        let targets = self.mcp_targets(&arguments["frame"])?;
        if targets.is_empty() {
            return Ok(None);
        }
        let mut bounds: Option<(DVec3, DVec3)> = None;
        for target in targets {
            if let Some((min, max)) = self.mcp_bounds(target)? {
                bounds = Some(bounds.map_or((min, max), |(low, high)| (low.min(min), high.max(max))));
            }
        }
        bounds.map(Some).ok_or_else(|| "Nothing among those items has anything to frame.".to_owned())
    }

    /// What a view hides and highlights: the project's own visibility with
    /// `show` and `hide` applied, or only `only`; and `highlight`. The
    /// user's view keeps their selection unless asked to highlight.
    fn mcp_scene_filter(&self, arguments: &Value, window: bool) -> Result<SceneFilter, String> {
        let only = self.mcp_entities(&arguments["only"])?;
        let mut hidden = if arguments["only"].is_null() {
            self.editor.hidden_handles.clone()
        } else {
            let keep: HashSet<SceneEntityId> = only.into_iter().collect();
            self.mcp_all_entities().filter(|entity| !keep.contains(entity)).collect()
        };
        for shown in self.mcp_entities(&arguments["show"])? {
            hidden.remove(&shown);
        }
        hidden.extend(self.mcp_entities(&arguments["hide"])?);
        let selected = if window && arguments["highlight"].is_null() {
            self.editor.selected_handles.clone()
        } else {
            self.mcp_entities(&arguments["highlight"])?.into_iter().collect()
        };
        Ok(SceneFilter { hidden, selected })
    }

    /// The scene entities `items` names, a design layer as its objects.
    fn mcp_entities(&self, items: &Value) -> Result<Vec<SceneEntityId>, String> {
        let targets = self.mcp_targets(items)?;
        let mut entities = Vec::new();
        for target in targets {
            match target {
                Target::Layer(layer) => entities.extend(
                    self.scene_document
                        .objects()
                        .iter()
                        .filter(|object| object.layer() == layer)
                        .map(|object| SceneEntityId::Object(object.id())),
                ),
                target => entities.extend(entity(target)),
            }
        }
        Ok(entities)
    }

    /// Every entity the scene could draw.
    fn mcp_all_entities(&self) -> impl Iterator<Item = SceneEntityId> + '_ {
        self.scene_document
            .objects()
            .iter()
            .map(|object| SceneEntityId::Object(object.id()))
            .chain(self.triangulations.iter().map(|item| item.entity_id()))
            .chain(self.block_models.iter().map(|item| item.entity_id()))
            .chain(self.drill_holes.iter().map(|item| item.entity_id()))
            .chain(self.point_clouds.iter().map(|item| item.entity_id()))
            .chain(self.raster_textures.iter().map(|item| SceneEntityId::Raster(item.id)))
    }

    /// Draw `request` with `filter` standing in for the window's visibility
    /// and selection, which are put back after.
    fn draw_mcp_view(&mut self, request: &ViewRequest, filter: &mut SceneFilter) -> Result<ViewportPixels, String> {
        let window = matches!(request.aim, ViewAim::Window);
        let showing_solid_preview = self.showing_solid_preview();
        let drawn = crate::app::events::drawn_surfaces(
            &self.editor,
            showing_solid_preview,
            &self.triangulations,
            &self.solid_view_body,
            &self.reserves_scene,
            &self.schedule_animation,
            &self.water,
            self.solid_preview.as_ref(),
        );
        // What the window draws stays on the GPU though this view leaves it
        // out, so the window's next frame does not upload it again.
        let retained: HashSet<_> = drawn
            .0
            .iter()
            .chain(drawn.1)
            .chain(&self.triangulations)
            .map(|surface| surface.id)
            .chain(self.solid_view_retained.iter().copied())
            .collect();
        // The user's view draws what their window does; any other draws the
        // project.
        let (triangulations, solid_preview) = if window { drawn } else { (&self.triangulations[..], drawn.1) };
        let graphics = self.graphics.as_mut().ok_or("The renderer is not ready yet.")?;
        let editor = &mut self.editor;
        std::mem::swap(&mut editor.hidden_handles, &mut filter.hidden);
        std::mem::swap(&mut editor.selected_handles, &mut filter.selected);
        let drill_holes_selected = std::mem::take(&mut editor.selected_drill_holes);
        let tie_ins_selected = std::mem::take(&mut editor.selected_tie_ins);
        let hovered = std::mem::take(&mut editor.tri_hover_handles);
        let highlighted = editor.tool_highlight_id.take();
        let pixels = graphics.render_view(
            request,
            SceneContent {
                editor,
                document: &self.scene_document,
                triangulations,
                block_models: &self.block_models,
                drill_holes: &self.drill_holes,
                point_clouds: &self.point_clouds,
                rasters: &self.raster_textures,
                solid_preview,
                retained_surfaces: &retained,
            },
        );
        std::mem::swap(&mut editor.hidden_handles, &mut filter.hidden);
        std::mem::swap(&mut editor.selected_handles, &mut filter.selected);
        editor.selected_drill_holes = drill_holes_selected;
        editor.selected_tie_ins = tie_ins_selected;
        editor.tri_hover_handles = hovered;
        editor.tool_highlight_id = highlighted;
        self.redraw_requested = true;
        pixels
    }

    /// The box round one target, `None` when it has nothing in it, or why it
    /// cannot be framed.
    fn mcp_bounds(&self, target: Target) -> Result<Option<(DVec3, DVec3)>, String> {
        let unloaded = |name: &str| format!("{name} is not loaded, so there is nothing of it to frame. Load it with set_loaded first.");
        let around = |points: &mut dyn Iterator<Item = DVec3>| {
            points.fold(None, |bounds: Option<(DVec3, DVec3)>, point| {
                Some(bounds.map_or((point, point), |(min, max)| (min.min(point), max.max(point))))
            })
        };
        Ok(match target {
            Target::Item(ItemRef::Triangulation(id)) => {
                let surface = self.triangulations.iter().find(|surface| surface.id == id).ok_or("That surface has gone.")?;
                if !surface.state.loaded || surface.state.deferred.is_some() {
                    return Err(unloaded(&surface.name));
                }
                let bounds = surface.mesh.bounds();
                (surface.mesh.face_count() > 0).then(|| (DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z), DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z)))
            }
            Target::Item(ItemRef::BlockModel(id)) => {
                let model = self.block_models.iter().find(|model| model.id == id).ok_or("That block model has gone.")?;
                Some(model.world_bounds.unwrap_or((model.model.metadata.lower, model.model.metadata.upper)))
            }
            Target::Item(ItemRef::DrillHole(id)) => {
                let collection = self.drill_holes.iter().find(|collection| collection.id == id).ok_or("Those drillholes have gone.")?;
                if !collection.state.loaded {
                    return Err(unloaded(&collection.name));
                }
                collection.dataset.bounds
            }
            Target::Item(ItemRef::PointCloud(id)) => {
                let cloud = self.point_clouds.iter().find(|cloud| cloud.id == id).ok_or("That point cloud has gone.")?;
                if cloud.points.is_empty() {
                    return Err(unloaded(&cloud.name));
                }
                Some(cloud.bounds)
            }
            Target::Item(ItemRef::Raster(_)) => return Err("A raster cannot be framed on its own: frame the surface it is draped on.".to_owned()),
            Target::Layer(layer) => {
                let document = self.workspace.active_document().ok_or("No project is open.")?;
                let name = document.layer(layer).map(|layer| layer.name.clone()).unwrap_or_default();
                if document.deferred_layers.contains_key(&layer) || document.layer(layer).is_some_and(|layer| !layer.loaded) {
                    return Err(unloaded(&name));
                }
                around(&mut document.objects().iter().filter(|object| object.layer() == layer).flat_map(object_points))
            }
            Target::Object(id) => {
                let document = self.workspace.active_document().ok_or("No project is open.")?;
                around(&mut document.get_object(id).into_iter().flat_map(object_points))
            }
        })
    }

    fn mcp_show_in_window(&mut self, arguments: &Value) -> Result<Value, String> {
        if self.headless {
            return Err("This Incline has no window (it was started with mcp --headless), so there is nothing to show the user.".to_owned());
        }
        let mut done = serde_json::Map::new();
        if arguments["workspace"].is_string() || arguments["page"].is_string() || arguments["schedule_hours"].is_number() {
            let result = self.mcp_show_workspace(arguments)?;
            done.insert("shown".to_owned(), result);
        }
        if !arguments["select"].is_null() {
            done.insert("selected".to_owned(), self.mcp_window_select(&arguments["select"])?);
        }
        if let Some(name) = arguments["view"].as_str() {
            let view = match name {
                "plan" => StandardView::Up,
                "below" => StandardView::Down,
                "north" => StandardView::North,
                "south" => StandardView::South,
                "east" => StandardView::East,
                "west" => StandardView::West,
                _ => return Err("view is plan, below, north, south, east or west.".to_owned()),
            };
            // Framed straight after, the turn is made at once: a swing keeps
            // the point it turns about, and would undo the framing as it ran.
            match self.graphics.as_mut() {
                Some(graphics) if !arguments["frame"].is_null() && !self.editor.slice_mode_enabled => graphics.snap_view_direction(view),
                _ => self.handle_ui_commands(vec![UiCommand::SetStandardView(view)]),
            }
            done.insert("view".to_owned(), json!(name));
        }
        if !arguments["frame"].is_null() {
            done.insert("framed".to_owned(), self.mcp_window_frame(&arguments["frame"])?);
        }
        if arguments["leave_section"].as_bool() == Some(true) {
            self.leave_slice_mode();
            done.insert("section_view".to_owned(), json!(false));
        }
        if let Some((from, to, width)) = section(&arguments["section"])? {
            if let Some(width) = width {
                self.editor.slice_width_input = width;
            }
            // Moving to another section leaves the one shown first.
            self.leave_slice_mode();
            if self.graphics.is_none() {
                return Err("The viewport is not ready yet.".to_owned());
            }
            if !self.enter_section(from, to) {
                return Err("from and to are the same point seen from above: a section needs a line.".to_owned());
            }
            done.insert("section_view".to_owned(), json!({ "width": self.editor.slice_width_input }));
        }
        if done.is_empty() {
            return Err("Say what to show: frame, view, select, section, leave_section, workspace, page or schedule_hours.".to_owned());
        }
        self.redraw_requested = true;
        Ok(ok(&Value::Object(done)))
    }

    /// Frame `items` in the window, or everything shown when there are none.
    fn mcp_window_frame(&mut self, items: &Value) -> Result<Value, String> {
        let targets = self.mcp_targets(items)?;
        if targets.is_empty() {
            self.handle_ui_commands(vec![UiCommand::ZoomToExtents]);
            return Ok(json!("everything shown"));
        }
        let mut bounds: Option<(DVec3, DVec3)> = None;
        for target in targets {
            if let Some((min, max)) = self.mcp_bounds(target)? {
                bounds = Some(bounds.map_or((min, max), |(low, high)| (low.min(min), high.max(max))));
            }
        }
        let (min, max) = bounds.ok_or("Nothing among those items has anything to frame.")?;
        let triangulations = self.framed_triangulations().to_vec();
        let graphics = self.graphics.as_mut().ok_or("The viewport is not ready yet.")?;
        graphics.frame_bounds(
            min,
            max,
            &self.scene_document,
            &triangulations,
            &self.block_models,
            &self.drill_holes,
            &self.point_clouds,
            &self.editor.hidden_handles,
        );
        Ok(super::extent(min, max))
    }

    /// Select `items` in the window, in place of its selection.
    fn mcp_window_select(&mut self, items: &Value) -> Result<Value, String> {
        let handles = self.mcp_entities(items)?;
        // Locked is locked to every way of selecting.
        let frozen = &self.editor.frozen_handles;
        let (handles, skipped): (Vec<_>, Vec<_>) = handles.into_iter().partition(|handle| !frozen.contains(handle));
        self.editor.clear_scene_selection();
        self.editor.selected_layers.clear();
        self.editor.selected_handles.extend(handles);
        self.editor.tri_selected_object_ids.clear();
        self.editor.tri_selected_layer_ids.clear();
        // The surface the tools default to is the one selected, if only one is.
        let mut surfaces = self.editor.selected_handles.iter().filter_map(|handle| match handle {
            SceneEntityId::Triangulation(id) => Some(*id),
            _ => None,
        });
        self.active_triangulation = match (surfaces.next(), surfaces.next()) {
            (Some(id), None) => Some(id),
            _ => None,
        };
        self.invalidate_geometry();
        self.invalidate_overlay();
        let mut result = self.mcp_selection();
        if !skipped.is_empty() {
            result["skipped_locked"] = json!(skipped.len());
        }
        Ok(result)
    }

    /// Switch the window's workspace, planning page and schedule playback.
    fn mcp_show_workspace(&mut self, arguments: &Value) -> Result<Value, String> {
        use crate::ui::state::{PlanningView, Workspace};
        if let Some(name) = arguments["workspace"].as_str() {
            let workspace = match name {
                "production" => Workspace::Production,
                "drill_and_blast" => Workspace::DrillAndBlast,
                "geology" => Workspace::Geology,
                "survey" => Workspace::Survey,
                "planning" => Workspace::Planning,
                _ => return Err("workspace is production, drill_and_blast, geology, survey or planning.".to_owned()),
            };
            let mut commands = Vec::new();
            crate::ui::elements::main_menu::select_workspace(&mut self.editor, &mut commands, workspace);
            self.handle_ui_commands(commands);
        }
        if let Some(name) = arguments["page"].as_str() {
            let view = match name {
                "setup" => PlanningView::Setup,
                "reserves" => PlanningView::Reserves,
                "calendar" => PlanningView::Calendar,
                "gantt" => PlanningView::Gantt,
                "charts" => PlanningView::Charts,
                "animate" => PlanningView::Animate,
                _ => return Err("page is setup, reserves, calendar, gantt, charts or animate.".to_owned()),
            };
            if self.editor.active_workspace != Workspace::Planning {
                let mut commands = Vec::new();
                crate::ui::elements::main_menu::select_workspace(&mut self.editor, &mut commands, Workspace::Planning);
                self.handle_ui_commands(commands);
            }
            self.handle_ui_commands(vec![UiCommand::SetPlanningView(view)]);
        }
        if let Some(hours) = arguments["schedule_hours"].as_f64() {
            self.editor.schedule_time_h = hours.max(0.0);
        }
        self.invalidate_geometry();
        self.redraw_requested = true;
        Ok(json!({ "workspace": format!("{:?}", self.editor.active_workspace), "schedule_hours": self.editor.schedule_time_h }))
    }

}

/// A section's line and slab width, if `value` gives one.
fn section(value: &Value) -> Result<Option<(DVec3, DVec3, Option<f64>)>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let end = |key: &str| super::parse_point(&value[key]).ok_or_else(|| format!("section needs {key} as [x, y, z]."));
    let width = match value["width"].as_f64() {
        Some(width) if width > 0.0 => Some(width),
        Some(_) => return Err("A section's width is above 0.".to_owned()),
        None => None,
    };
    Ok(Some((end("from")?, end("to")?, width)))
}

/// The angles `render_view` looks from when no named view is given.
fn angles(arguments: &Value) -> (f64, f64) {
    (
        arguments["azimuth"].as_f64().unwrap_or(DEFAULT_AZIMUTH),
        arguments["elevation"].as_f64().unwrap_or(DEFAULT_ELEVATION).clamp(-90.0, 90.0),
    )
}

/// The direction a view looks, and which way is up in it.
fn view_direction(named: Option<&str>, arguments: &Value) -> Result<(DVec3, DVec3), String> {
    use crate::rendering::graphics::camera::standard_view_basis;
    let standard = match named {
        Some("plan") => Some(StandardView::Up),
        Some("below") => Some(StandardView::Down),
        Some("north") => Some(StandardView::North),
        Some("south") => Some(StandardView::South),
        Some("east") => Some(StandardView::East),
        Some("west") => Some(StandardView::West),
        Some(other) => return Err(format!("{other} is not a view render_view can frame.")),
        None => None,
    };
    if let Some(view) = standard {
        return Ok(standard_view_basis(view));
    }
    let (azimuth, elevation) = angles(arguments);
    let (azimuth, elevation) = (azimuth.to_radians(), elevation.to_radians());
    let heading = DVec3::new(azimuth.sin(), azimuth.cos(), 0.0);
    let forward = heading * elevation.cos() - DVec3::Z * elevation.sin();
    // Looking straight down or up, up in the picture is the heading.
    let up = if elevation.cos().abs() < 1.0e-6 { heading } else { DVec3::Z };
    Ok((forward, up))
}

/// `pixels` as an MCP image result, with a line saying what it shows.
fn picture(pixels: &ViewportPixels, described: &str) -> Result<Value, String> {
    use base64::Engine;

    let mut rgba = pixels.rgba.clone();
    // The scene is drawn opaque, but what is left in the alpha channel is not
    // always 255, and a viewer would show the picture see-through.
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    let png = crate::rendering::graphics::screenshot::encode_png(&rgba, pixels.width, pixels.height).map_err(|error| format!("The picture could not be made: {error:#}"))?;
    Ok(json!({
        "content": [
            { "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" },
            { "type": "text", "text": format!("{described}, {} by {} pixels.", pixels.width, pixels.height) },
        ],
        "isError": false,
    }))
}
