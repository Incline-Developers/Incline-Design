//! Drawing aids for a string being drawn: give back the last vertex, add
//! the next one as typed, or a step from the last by bearing and distance.
//! On a section the vertex goes onto the plane, as a click there does.

use glam::{DVec2, DVec3};

use crate::{
    app::App,
    i18n::tr,
    model::{Command, Object},
    rendering::camera::SectionSlab,
    ui::state::{ActiveTool, EditorState, NextVertexPanel},
    userspace_log,
};

/// How far a typed point may sit off the section before the move onto it is
/// worth reporting, in metres.
const REPORTED_SECTION_SHIFT: f64 = 0.01;

impl<'a> App<'a> {
    /// Give back the last vertex of the string being drawn, keeping the rest.
    /// The Line tool saves each segment as it is drawn, so at a segment's
    /// end this takes the saved segment back and draws on from its start.
    pub(crate) fn remove_last_stroke_vertex(&mut self) {
        let taken = if self.editor.active_tool == ActiveTool::MakeLine && self.editor.pending_stroke.len() == 1 {
            self.take_back_line_segment() || take_back_stroke_vertex(&mut self.editor)
        } else {
            take_back_stroke_vertex(&mut self.editor)
        };
        // The stroke is drawn on the overlay, which repaints only when asked.
        if taken {
            self.editor.stroke_leg_broken = true;
            self.sync_next_vertex_panel();
            self.invalidate_overlay();
        }
    }

    /// Keep the Next Vertex panel's point on the string's last vertex, so an
    /// Add after a click or a Backspace starts from where the string ends.
    pub(crate) fn sync_next_vertex_panel(&mut self) {
        if let (Some(panel), Some(last)) = (self.editor.next_vertex_panel.as_mut(), self.editor.pending_stroke.last()) {
            panel.point = last.to_array();
        }
    }

    /// Note where the segment the Line tool has just saved starts. Only the
    /// start is kept: the save itself waits while a layer is restored, so
    /// the segment is found by its ends when it is taken back.
    pub(crate) fn record_line_segment(&mut self, start: DVec3) {
        self.editor.line_chain.push(start);
    }

    /// Delete the newest segment running from the noted start to where the
    /// stroke now starts, as one undo step, and draw on from its start. With
    /// no such segment (edited or deleted since) the notes are dropped.
    fn take_back_line_segment(&mut self) -> bool {
        let (Some(start), Some(&end)) = (self.editor.line_chain.pop(), self.editor.pending_stroke.first()) else {
            return false;
        };
        let segment = self.workspace.active_document().and_then(|document| {
            document
                .objects()
                .iter()
                .rev()
                .find(|object| matches!(object, Object::Polyline { verts, closed: false, .. } if verts.len() == 2 && verts[0].pos == start && verts[1].pos == end))
                .cloned()
        });
        let Some(object) = segment else {
            self.editor.line_chain.clear();
            return false;
        };
        self.execute_edit(Command::DeleteObject { object, index: None });
        self.editor.pending_stroke = vec![start];
        true
    }

    /// Open the Next Vertex panel at the string's last vertex, or at the
    /// cursor before there is one.
    pub(crate) fn open_next_vertex_panel(&mut self) {
        if !draws_strings(self.editor.active_tool) {
            return;
        }
        let start = self.editor.pending_stroke.last().copied().or(self.editor.cursor_world).unwrap_or(DVec3::ZERO);
        self.editor.next_vertex_panel.get_or_insert_with(NextVertexPanel::default).point = start.to_array();
        self.redraw_requested = true;
    }

    pub(crate) fn add_typed_stroke_vertex(&mut self, world: DVec3) {
        let world = self.onto_drawing_section(world);
        self.add_stroke_vertex(world);
    }

    /// Add the vertex `distance` metres on the level from the last along
    /// `bearing`, rising at `dip`. On a section the bearing is the section's.
    pub(crate) fn add_stroke_vertex_along_bearing(&mut self, bearing: f64, distance: f64, dip: f64) {
        let Some(&last) = self.editor.pending_stroke.last() else {
            return;
        };
        let bearing = self.drawing_section().map_or(bearing, |slab| {
            // Seen from the back, screen right runs the other way.
            let from_front = self.graphics.as_ref().and_then(|graphics| graphics.section_viewed_from_front()).unwrap_or(true);
            (section_bearing(&slab) + if from_front { 0.0 } else { 180.0 }).rem_euclid(360.0)
        });
        let world = self.onto_drawing_section(along_bearing(last, bearing, distance, dip));
        self.add_stroke_vertex(world);
    }

    fn add_stroke_vertex(&mut self, world: DVec3) {
        if !self.editing_ready() || !draws_strings(self.editor.active_tool) || self.editor.poly_finish_dialog || !world.is_finite() {
            return;
        }
        // The point already ends the stroke: adding it again would close a
        // polyline on itself or save a line of no length.
        if self.editor.pending_stroke.last().is_some_and(|last| (*last - world).length_squared() <= 1.0e-16) {
            return;
        }
        let Some(layer) = self.active_layer() else {
            return;
        };
        self.push_stroke_point(world, layer);
        self.invalidate_overlay();
        self.sync_next_vertex_panel();
    }

    /// The section a vertex must go onto: a Geology section, while one is up.
    fn drawing_section(&self) -> Option<SectionSlab> {
        if !self.editor.slice_mode_enabled || !self.editor.active_workspace.edits_in_slice_view() {
            return None;
        }
        self.graphics.as_ref().and_then(|graphics| graphics.section_slab())
    }

    fn onto_drawing_section(&self, world: DVec3) -> DVec3 {
        let Some(slab) = self.drawing_section() else {
            return world;
        };
        let on_plane = onto_section(&slab, world);
        let shift = on_plane.distance(world);
        if shift > REPORTED_SECTION_SHIFT {
            userspace_log!("{}", tr!("cmd-next-vertex-moved-onto-section", distance = format!("{shift:.2}")));
        }
        on_plane
    }
}

fn draws_strings(tool: ActiveTool) -> bool {
    matches!(tool, ActiveTool::MakeLine | ActiveTool::MakePoly)
}

/// Drop the stroke's last vertex; false when there is none to give back or
/// the polyline's finish dialog is up.
fn take_back_stroke_vertex(editor: &mut EditorState) -> bool {
    draws_strings(editor.active_tool) && !editor.poly_finish_dialog && editor.pending_stroke.pop().is_some()
}

/// The point `distance` metres on the level from `from` along `bearing`
/// (degrees clockwise from grid north), rising at `dip` degrees whichever
/// way it goes. A negative distance steps back along the bearing.
pub(crate) fn along_bearing(from: DVec3, bearing: f64, distance: f64, dip: f64) -> DVec3 {
    let bearing = bearing.to_radians();
    from + DVec3::new(distance * bearing.sin(), distance * bearing.cos(), distance.abs() * dip.to_radians().tan())
}

/// The bearing that reads left to right on a section.
fn section_bearing(slab: &SectionSlab) -> f64 {
    let along = DVec2::new(slab.normal.y, -slab.normal.x);
    along.x.atan2(along.y).to_degrees().rem_euclid(360.0)
}

/// `world` moved square onto the section plane.
fn onto_section(slab: &SectionSlab, world: DVec3) -> DVec3 {
    world - slab.normal * slab.signed_distance(world)
}
