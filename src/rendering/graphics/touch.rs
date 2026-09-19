use glam::DVec2;
use winit::event::{DeviceId, MouseButton, Touch, TouchPhase};

use super::Graphics;

#[derive(Default)]
pub(super) struct TouchGesture {
    pub(super) contacts: Vec<(DeviceId, u64, DVec2)>,
    pending_anchor: Option<DVec2>,
    multi_touch: bool,
}

#[derive(Debug, PartialEq)]
enum Gesture {
    None,
    Anchor(DVec2),
    Orbit(DVec2),
    PanZoom { centre: DVec2, delta: DVec2, scale: f64 },
    End,
}

impl TouchGesture {
    fn owns(&self, touch: &Touch, allow_start: bool) -> bool {
        self.contacts.iter().any(|(device, id, _)| *device == touch.device_id && *id == touch.id) || (touch.phase == TouchPhase::Started && allow_start)
    }

    fn update(&mut self, touch: &Touch, allow_start: bool, orbit_threshold: f64) -> Gesture {
        let position = DVec2::new(touch.location.x, touch.location.y);
        let index = self.contacts.iter().position(|(device, id, _)| *device == touch.device_id && *id == touch.id);
        match touch.phase {
            TouchPhase::Started if allow_start && index.is_none() => {
                self.contacts.push((touch.device_id, touch.id, position));
                if self.contacts.len() == 1 {
                    // A touch-down may be the first half of a pan/pinch.
                    self.pending_anchor = Some(position);
                    Gesture::None
                } else {
                    self.pending_anchor = None;
                    self.multi_touch = true;
                    // Clear any one-finger orbit pivot and its marker immediately.
                    Gesture::End
                }
            }
            TouchPhase::Moved => {
                let Some(index) = index else { return Gesture::None };
                let previous = self.contacts[index].2;
                let old_pair = (self.contacts.len() == 2).then(|| (self.contacts[0].2, self.contacts[1].2));
                self.contacts[index].2 = position;
                if let Some((a, b)) = old_pair {
                    let (c, d) = (self.contacts[0].2, self.contacts[1].2);
                    let old_distance = a.distance(b);
                    let new_distance = c.distance(d);
                    Gesture::PanZoom {
                        centre: (c + d) * 0.5,
                        delta: (c + d - a - b) * 0.5,
                        scale: if old_distance >= 1.0 && new_distance >= 1.0 { old_distance / new_distance } else { 1.0 },
                    }
                } else if self.contacts.len() == 1 && !self.multi_touch {
                    if let Some(anchor) = self.pending_anchor {
                        if position.distance(anchor) < orbit_threshold {
                            return Gesture::None;
                        }
                        self.pending_anchor = None;
                        Gesture::Anchor(anchor)
                    } else {
                        Gesture::Orbit(position - previous)
                    }
                } else {
                    Gesture::None
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let Some(index) = index else { return Gesture::None };
                self.contacts.remove(index);
                if self.contacts.is_empty() {
                    self.pending_anchor = None;
                    self.multi_touch = false;
                    Gesture::End
                } else {
                    // Keep a pan/pinch gesture latched until all fingers lift.
                    Gesture::None
                }
            }
            _ => Gesture::None,
        }
    }
}

impl Graphics<'_> {
    /// `None` routes the contact to egui. Captured viewport contacts return
    /// `Some`, with true requesting a fresh world-space orbit anchor.
    pub(crate) fn touch_input(&mut self, touch: &Touch) -> Option<bool> {
        let px = self.window_to_viewport_px((touch.location.x as f32, touch.location.y as f32));
        let screen = self.screen_size();
        let over_overlay = self.gui.overlay_at_physical_position(touch.location.x as f32, touch.location.y as f32);
        let allow_start = px.0 >= 0.0 && px.1 >= 0.0 && px.0 < screen.0 && px.1 < screen.1 && !over_overlay;
        // Decide ownership before removing an ended/cancelled contact. A finger
        // stays with the UI or viewport where it started even across boundaries.
        if !self.touch_gesture.owns(touch, allow_start) {
            return None;
        }
        let gesture = self.touch_gesture.update(touch, allow_start, 6.0 * self.window.scale_factor());
        // Apply each move before accepting another contact transition: a queued
        // orbit delta must never be applied around a subsequently picked pivot.
        match gesture {
            Gesture::None => {}
            Gesture::Anchor(position) => {
                self.camera_controller.mouse_loc = self.window_to_viewport_px((position.x as f32, position.y as f32));
                self.camera_controller.cancel_view_transition();
                if let Some(slice) = self.slice_view.as_mut() {
                    slice.turn_to = None;
                }
                self.mark_interaction();
                return Some(true);
            }
            Gesture::Orbit(delta) => {
                self.mark_interaction();
                if let Some(slice) = self.slice_view.as_mut() {
                    slice.orbit += delta;
                    self.update_slice_touch_camera();
                } else {
                    self.camera_controller.process_mouse(Some(MouseButton::Right), delta.x, delta.y);
                    self.camera_controller
                        .update_camera(&mut self.camera, &mut self.projection, std::time::Duration::ZERO, screen);
                }
            }
            Gesture::PanZoom { centre, delta, scale } => {
                self.mark_interaction();
                self.camera_controller.mouse_loc = self.window_to_viewport_px((centre.x as f32, centre.y as f32));
                if let Some(slice) = self.slice_view.as_mut() {
                    slice.pan += DVec2::new(-delta.x, delta.y);
                    slice.scroll += if scale < 1.0 { (1.0 / scale - 1.0) / 0.005 } else { (1.0 - scale) / 0.005 };
                    self.update_slice_touch_camera();
                } else {
                    self.camera_controller.process_mouse(Some(MouseButton::Middle), delta.x, delta.y);
                    self.camera_controller
                        .update_camera(&mut self.camera, &mut self.projection, std::time::Duration::ZERO, screen);
                    let zoom = (self.projection.zoom * scale).max(1.0e-4);
                    let offset = crate::rendering::camera::view_plane_offset(
                        &self.camera,
                        self.projection.zoom - zoom,
                        screen.0 as f64 / screen.1.max(1.0) as f64,
                        screen,
                        self.camera_controller.mouse_loc,
                    );
                    self.camera.frame_keep_orientation(self.camera.target() + offset, zoom);
                    self.projection.zoom = zoom;
                }
            }
            Gesture::End => {
                self.camera_controller.end_orbit();
                self.orbit_marker = None;
            }
        }
        Some(false)
    }

    fn update_slice_touch_camera(&mut self) {
        self.update_slice_camera(std::time::Duration::ZERO, self.orbit_marker);
    }
}
