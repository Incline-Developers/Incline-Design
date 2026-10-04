//! Camera handling for the offscreen 3D previews - the Solids pages, the
//! sequence editor and the blast window - so each is driven the way the main
//! viewport is: middle drag pans, right drag orbits about the ground under the
//! pointer, the wheel zooms, and the left button is left to the page.

use crate::ui::{EditorState, state::SolidPreviewView};

/// Where `pos` falls on `image`, `0..1` across and down.
pub(crate) fn image_uv(image: egui::Rect, pos: egui::Pos2) -> [f32; 2] {
    let local = pos - image.min;
    [local.x / image.width().max(1.0), local.y / image.height().max(1.0)]
}

/// Move `view` by what the pointer did over the preview `image` this frame.
/// Returns whether it moved.
///
/// `enabled` false leaves the camera alone - a pane that is display only
/// still gets hovered and scrolled over.
pub(crate) fn navigate(ui: &egui::Ui, response: &egui::Response, image: egui::Rect, view: &mut SolidPreviewView, editor: &mut EditorState, enabled: bool) -> bool {
    if !enabled {
        return false;
    }
    let pixels_per_point = ui.ctx().pixels_per_point();
    let frame_height = f64::from(image.height() * pixels_per_point).max(1.0);
    let mut changed = false;

    // The pivot is asked for on the press rather than once the drag has
    // begun, so the renderer has found it by the time the view first turns.
    let pressed_at = ui.input(|input| input.pointer.press_origin().filter(|_| input.pointer.button_pressed(egui::PointerButton::Secondary)));
    if let Some(origin) = pressed_at.filter(|origin| image.contains(*origin)) {
        editor.solid_preview_pivot = None;
        editor.solid_preview_pivot_request = Some(image_uv(image, origin));
        ui.ctx().request_repaint();
    }
    let delta = response.drag_delta() * pixels_per_point;
    if delta != egui::Vec2::ZERO {
        let delta = [f64::from(delta.x), f64::from(delta.y)];
        if response.dragged_by(egui::PointerButton::Middle) {
            view.pan_by_pixels(delta, frame_height);
            changed = true;
        } else if response.dragged_by(egui::PointerButton::Secondary) {
            view.orbit_about(delta, frame_height, editor.solid_preview_pivot);
            changed = true;
        }
    }
    // The pivot keeps its place on screen through the orbit, so its mark
    // stays where the drag began.
    if response.dragged_by(egui::PointerButton::Secondary)
        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
    {
        crate::ui::elements::cursors::paint_pivot_marker(&ui.painter().with_clip_rect(image), origin);
    }

    if response.hovered() {
        let scroll = ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                        egui::MouseWheelUnit::Point => f64::from(delta.y * pixels_per_point),
                        // One wheel line is about a hundred physical pixels,
                        // matching the main camera's convention.
                        egui::MouseWheelUnit::Line => f64::from(delta.y) * 100.0,
                        egui::MouseWheelUnit::Page => f64::from(delta.y) * frame_height,
                    }),
                    _ => None,
                })
                .sum::<f64>()
        });
        if scroll != 0.0 {
            match response.hover_pos().filter(|_| editor.plan_zoom_towards_cursor) {
                Some(pointer) => {
                    let aspect = f64::from(image.width() / image.height().max(1.0));
                    view.zoom_at(scroll, image_uv(image, pointer).map(f64::from), aspect);
                }
                None => view.zoom_by_scroll(scroll),
            }
            changed = true;
        }
    }
    if changed {
        ui.ctx().request_repaint();
    }
    changed
}

/// The viewport's own orientation gizmo in the top right of a preview,
/// turning `view` to the direction an axis click names. Returns whether it
/// did. Hidden with the main viewport's gizmo.
pub(crate) fn orientation_gizmo(ui: &mut egui::Ui, id: &str, image: egui::Rect, view: &mut SolidPreviewView, editor: &EditorState) -> bool {
    if !editor.show_world_axis_gizmo {
        return false;
    }
    let (_, up) = view.screen_basis();
    let forward = view.forward();
    let gizmo = crate::ui::elements::cursors::draw_orientation_gizmo(ui, egui::Id::new(id), image, forward.as_vec3().to_array(), up.as_vec3().to_array(), false);
    let Some(direction) = gizmo.clicked else { return false };
    view.face(direction);
    ui.ctx().request_repaint();
    true
}
