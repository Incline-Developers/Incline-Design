//! The tools that change what the window shows without changing the
//! project's content: visibility, the selection, the camera, and a look at
//! the viewport.

use std::{
    collections::HashSet,
    sync::mpsc::{Receiver, TryRecvError, sync_channel},
    time::{Duration, Instant},
};

use glam::DVec3;
use serde_json::{Value, json};

use super::{Target, entity, failed, items_schema, object_points, ok};
use crate::{
    app::App,
    mcp::Call,
    model::{ItemRef, SceneEntityId},
    rendering::graphics::screenshot::ViewportPixels,
    ui::state::{StandardView, UiCommand},
};

/// The longest side of the viewport picture unless asked for another.
const DEFAULT_CAPTURE_SIZE: u32 = 1280;
/// The smallest picture worth sending.
const MIN_CAPTURE_SIZE: u32 = 64;
/// How long a look at the viewport waits for a frame before giving up: a
/// minimised window draws none.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "set_visibility",
            "title": "Show or hide an item",
            "description": "Show or hide one item in Incline Design's viewport, as the eye beside it in the explorer does. Showing an item that is not loaded loads it first, which for a large item takes a moment: get_project says when it is loaded. Ids and kinds are as get_project reports them.",
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
            "name": "select",
            "title": "Select",
            "description": "Change what is selected in Incline Design, as clicking in the viewport or explorer does. The window's own tools (triangulate, cut, contour and the rest) work on the selection. A design layer stands for every object on it. Locked items, and objects on locked layers, cannot be selected and are listed as skipped. Returns the selection as get_selection does.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "items": items_schema("What to select. Ids and kinds are as get_project, get_layer and get_selection report them."),
                    "mode": {
                        "type": "string",
                        "enum": ["replace", "add", "remove"],
                        "description": "replace (the default) selects only these; add adds them to the selection; remove takes them out of it. replace with no items clears the selection.",
                    },
                },
                "required": ["items"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
        json!({
            "name": "zoom_to",
            "title": "Zoom to",
            "description": "Frame items in Incline Design's viewport, keeping the angle it is looked at from. With no items, frames everything shown.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "items": items_schema("What to frame. Everything shown if left out or empty. A raster cannot be framed: frame the surface it is draped on."),
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
        json!({
            "name": "set_view",
            "title": "Set view direction",
            "description": "Turn Incline Design's viewport to look from one direction, as the view cube does: plan looks down from above, below looks up from underneath, and north, south, east and west look from that side towards the opposite one. The camera swings there over a moment; capture_viewport waits for it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view": { "type": "string", "enum": ["plan", "below", "north", "south", "east", "west"] },
                },
                "required": ["view"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
        json!({
            "name": "show_workspace",
            "title": "Show a workspace",
            "description": "Switch Incline Design to a workspace, as its tabs do: production (design), drill_and_blast, geology, survey or planning. In planning, view picks the page: setup, reserves, calendar, gantt, charts or animate. schedule_hours moves the schedule's playback to that many hours from the start, for animate.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace": { "type": "string", "enum": ["production", "drill_and_blast", "geology", "survey", "planning"] },
                    "view": { "type": "string", "enum": ["setup", "reserves", "calendar", "gantt", "charts", "animate"] },
                    "schedule_hours": { "type": "number", "minimum": 0 },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
        json!({
            "name": "capture_viewport",
            "title": "Look at the viewport",
            "description": "Take a picture of Incline Design's 3D viewport as the user sees it, without the panels round it, to check what the project looks like or what a change did.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "max_size": { "type": "integer", "minimum": MIN_CAPTURE_SIZE, "description": format!("The longest side of the picture in pixels; a larger viewport is scaled down to it. {DEFAULT_CAPTURE_SIZE} if left out.") },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true },
        }),
    ]
}

/// A `capture_viewport` call waiting for the frame that answers it.
pub(crate) struct PendingCapture {
    call: Call,
    max_size: u32,
    asked_at: Instant,
    /// Where the frame's pixels come, once the renderer has been asked for
    /// them. Asked for only once the camera has stopped moving.
    pixels: Option<Receiver<Result<ViewportPixels, String>>>,
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_view_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        Some(match name {
            "set_visibility" => self.mcp_set_visibility(arguments),
            "select" => self.mcp_select(arguments),
            "zoom_to" => self.mcp_zoom_to(arguments),
            "set_view" => self.mcp_set_view(arguments),
            "show_workspace" => self.mcp_show_workspace(arguments),
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

    fn mcp_select(&mut self, arguments: &Value) -> Value {
        let targets = match self.mcp_targets(&arguments["items"]) {
            Ok(targets) => targets,
            Err(error) => return failed(error),
        };
        let mode = arguments["mode"].as_str().unwrap_or("replace");
        if !matches!(mode, "replace" | "add" | "remove") {
            return failed("mode is replace, add or remove.");
        }
        let Some(document) = self.workspace.active_document() else {
            return failed("No project is open.");
        };
        let mut handles = Vec::new();
        for target in targets {
            match target {
                Target::Layer(layer) => handles.extend(
                    document
                        .objects()
                        .iter()
                        .filter(|object| object.layer() == layer)
                        .map(|object| SceneEntityId::Object(object.id())),
                ),
                target => handles.extend(entity(target)),
            }
        }
        // Locked is locked to every way of selecting.
        let frozen = &self.editor.frozen_handles;
        let (handles, skipped): (Vec<_>, Vec<_>) = handles.into_iter().partition(|handle| !frozen.contains(handle));
        match mode {
            "replace" => {
                self.editor.clear_scene_selection();
                self.editor.selected_layers.clear();
                self.editor.selected_handles.extend(handles);
            }
            "add" => self.editor.selected_handles.extend(handles),
            _ => {
                let removed: HashSet<_> = handles.into_iter().collect();
                self.editor.selected_handles.retain(|handle| !removed.contains(handle));
            }
        }
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
        ok(&result)
    }

    fn mcp_zoom_to(&mut self, arguments: &Value) -> Value {
        let targets = match self.mcp_targets(&arguments["items"]) {
            Ok(targets) => targets,
            Err(error) => return failed(error),
        };
        if targets.is_empty() {
            self.handle_ui_commands(vec![UiCommand::ZoomToExtents]);
            return ok(&json!({ "framed": "everything shown" }));
        }
        let mut bounds: Option<(DVec3, DVec3)> = None;
        for target in targets {
            let found = match self.mcp_bounds(target) {
                Ok(found) => found,
                Err(error) => return failed(error),
            };
            if let Some((min, max)) = found {
                bounds = Some(bounds.map_or((min, max), |(low, high)| (low.min(min), high.max(max))));
            }
        }
        let Some((min, max)) = bounds else {
            return failed("Nothing among those items has anything to frame.");
        };
        let triangulations = self.framed_triangulations().to_vec();
        let Some(graphics) = self.graphics.as_mut() else {
            return failed("The viewport is not ready yet.");
        };
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
        self.redraw_requested = true;
        ok(&json!({ "framed": super::extent(min, max) }))
    }

    /// The box round one target, `None` when it has nothing in it, or why it
    /// cannot be framed.
    fn mcp_bounds(&self, target: Target) -> Result<Option<(DVec3, DVec3)>, String> {
        let unloaded = |name: &str| format!("{name} is not loaded, so there is nothing of it to frame. Load it with set_visibility first.");
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

    fn mcp_set_view(&mut self, arguments: &Value) -> Value {
        let view = match arguments["view"].as_str() {
            Some("plan") => StandardView::Up,
            Some("below") => StandardView::Down,
            Some("north") => StandardView::North,
            Some("south") => StandardView::South,
            Some("east") => StandardView::East,
            Some("west") => StandardView::West,
            _ => return failed("view is plan, below, north, south, east or west."),
        };
        self.handle_ui_commands(vec![UiCommand::SetStandardView(view)]);
        ok(&json!({ "view": arguments["view"] }))
    }

    fn mcp_show_workspace(&mut self, arguments: &Value) -> Value {
        use crate::ui::state::{PlanningView, Workspace};
        if let Some(name) = arguments["workspace"].as_str() {
            let workspace = match name {
                "production" => Workspace::Production,
                "drill_and_blast" => Workspace::DrillAndBlast,
                "geology" => Workspace::Geology,
                "survey" => Workspace::Survey,
                "planning" => Workspace::Planning,
                _ => return failed("workspace is production, drill_and_blast, geology, survey or planning."),
            };
            let mut commands = Vec::new();
            crate::ui::elements::main_menu::select_workspace(&mut self.editor, &mut commands, workspace);
            self.handle_ui_commands(commands);
        }
        if let Some(name) = arguments["view"].as_str() {
            let view = match name {
                "setup" => PlanningView::Setup,
                "reserves" => PlanningView::Reserves,
                "calendar" => PlanningView::Calendar,
                "gantt" => PlanningView::Gantt,
                "charts" => PlanningView::Charts,
                "animate" => PlanningView::Animate,
                _ => return failed("view is setup, reserves, calendar, gantt, charts or animate."),
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
        ok(&json!({ "workspace": format!("{:?}", self.editor.active_workspace), "schedule_hours": self.editor.schedule_time_h }))
    }

    /// Take a `capture_viewport` call, to be answered once a frame has been
    /// drawn for it (`poll_mcp_captures`).
    pub(crate) fn begin_mcp_capture(&mut self, call: Call) {
        let max_size = call.request["params"]["arguments"]["max_size"]
            .as_u64()
            .map_or(DEFAULT_CAPTURE_SIZE, |size| u32::try_from(size).unwrap_or(u32::MAX).max(MIN_CAPTURE_SIZE));
        self.mcp_captures.push(PendingCapture {
            call,
            max_size,
            asked_at: Instant::now(),
            pixels: None,
        });
        self.poll_mcp_captures();
    }

    /// Answer the captures whose frames have been drawn, ask the renderer for
    /// the next one once the camera is still, and give up on any that have
    /// waited too long. Returns whether any are still waiting, to keep the
    /// event loop coming back.
    pub(crate) fn poll_mcp_captures(&mut self) -> bool {
        let mut waiting = Vec::new();
        for mut capture in std::mem::take(&mut self.mcp_captures) {
            let pixels = capture.pixels.as_ref().map(Receiver::try_recv);
            match pixels {
                Some(Ok(result)) => {
                    let id = capture.call.request["id"].clone();
                    let result = result.and_then(|pixels| picture(&pixels, capture.max_size)).unwrap_or_else(failed);
                    capture.call.reply(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
                    continue;
                }
                // The renderer dropped the request: something else took the
                // one capture it holds at a time. Ask again.
                Some(Err(TryRecvError::Disconnected)) => capture.pixels = None,
                Some(Err(TryRecvError::Empty)) | None => {}
            }
            if capture.asked_at.elapsed() > CAPTURE_TIMEOUT {
                let id = capture.call.request["id"].clone();
                let result = failed("The viewport did not draw a frame in time. Is Incline Design's window minimised?");
                capture.call.reply(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
                continue;
            }
            waiting.push(capture);
        }
        // One capture at a time: the renderer holds one request.
        let asked = waiting.iter().any(|capture| capture.pixels.is_some());
        if !asked
            && let Some(capture) = waiting.first_mut()
            && let Some(graphics) = self.graphics.as_mut()
            && !graphics.view_transition_running()
            && !graphics.screenshot_pending()
        {
            let (reply, pixels) = sync_channel(1);
            graphics.request_mcp_capture(reply);
            capture.pixels = Some(pixels);
        }
        let any = !waiting.is_empty();
        if any {
            self.redraw_requested = true;
        }
        self.mcp_captures = waiting;
        any
    }
}

/// The viewport's pixels as an MCP image result, scaled down to `max_size`
/// on its longest side.
fn picture(pixels: &ViewportPixels, max_size: u32) -> Result<Value, String> {
    use base64::Engine;

    let longest = pixels.width.max(pixels.height).max(1);
    let (width, height) = if longest > max_size {
        let scale = f64::from(max_size) / f64::from(longest);
        (
            ((f64::from(pixels.width) * scale).round() as u32).max(1),
            ((f64::from(pixels.height) * scale).round() as u32).max(1),
        )
    } else {
        (pixels.width, pixels.height)
    };
    let mut rgba = shrink(&pixels.rgba, pixels.width, pixels.height, width, height);
    // The scene is drawn opaque, but what is left in the alpha channel is not
    // always 255, and a viewer would show the picture see-through.
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    let png = crate::rendering::graphics::screenshot::encode_png(&rgba, width, height).map_err(|error| format!("The picture could not be made: {error:#}"))?;
    Ok(json!({
        "content": [
            { "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" },
            { "type": "text", "text": format!("The viewport, {width} by {height} pixels.") },
        ],
        "isError": false,
    }))
}

/// `rgba`, `width` by `height`, averaged down to `to_width` by `to_height`:
/// each pixel out is the mean of the pixels it covers, so thin lines fade
/// rather than vanish.
fn shrink(rgba: &[u8], width: u32, height: u32, to_width: u32, to_height: u32) -> Vec<u8> {
    if (width, height) == (to_width, to_height) {
        return rgba.to_vec();
    }
    let (width, height, to_width, to_height) = (width as usize, height as usize, to_width as usize, to_height as usize);
    let mut out = vec![0u8; to_width * to_height * 4];
    for y in 0..to_height {
        let (top, bottom) = (y * height / to_height, ((y + 1) * height / to_height).max(y * height / to_height + 1).min(height));
        for x in 0..to_width {
            let (left, right) = (x * width / to_width, ((x + 1) * width / to_width).max(x * width / to_width + 1).min(width));
            let mut sum = [0u32; 4];
            for row in top..bottom {
                for column in left..right {
                    let at = (row * width + column) * 4;
                    for (total, value) in sum.iter_mut().zip(&rgba[at..at + 4]) {
                        *total += u32::from(*value);
                    }
                }
            }
            let count = ((bottom - top) * (right - left)) as u32;
            let at = (y * to_width + x) * 4;
            for (value, total) in out[at..at + 4].iter_mut().zip(sum) {
                *value = (total / count) as u8;
            }
        }
    }
    out
}
