//! Drawing aids for a string being drawn: give back the last vertex, add
//! the next one as typed, or a step from the last by bearing and distance,
//! or copy a run of an existing string, carry on one from either end, or
//! redraw one between two of its vertices.
//! On a section the vertex goes onto the plane, as a click there does.

use glam::{DVec2, DVec3};

use super::bezier::{choose_closed_span, directed_span_points, replace_polyline_span_with_bezier};
use crate::{
    app::{App, PICK_THRESHOLD_PX},
    i18n::tr,
    model::{Command, Object, ObjectId, PolyVertex, SceneEntityId},
    rendering::{camera::SectionSlab, pick},
    ui::state::{ActiveTool, CopyAlong, EditorState, NextVertexPanel, StringRework, copy_along_live, string_rework_live},
    userspace_log, userspace_warn,
};

/// How far a typed point may sit off the section before the move onto it is
/// worth reporting, in metres.
const REPORTED_SECTION_SHIFT: f64 = 0.01;

impl<'a> App<'a> {
    /// Give back the last vertex of the string being drawn, keeping the rest.
    /// The Line tool saves each segment as it is drawn, so at a segment's
    /// end this takes the saved segment back and draws on from its start.
    pub(crate) fn remove_last_stroke_vertex(&mut self) {
        // Carrying on a string, Backspace stops at the string's end.
        if self.editor.pending_stroke.len() == 1 && string_rework_live(&self.editor).is_some() {
            return;
        }
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
        if ends_stroke(&self.editor.pending_stroke, world) {
            return;
        }
        let Some(layer) = self.active_layer() else {
            return;
        };
        self.push_stroke_point(world, layer);
        self.invalidate_overlay();
        self.sync_next_vertex_panel();
    }

    /// Start Copy Along a String on the polyline being drawn: its next two
    /// clicks pick a run of an existing string to append.
    pub(crate) fn arm_copy_along_string(&mut self) {
        // Asked for before a polyline is begun, it begins one: the copied run
        // then starts the new string.
        if self.editor.active_workspace.has_drawing_aids() && self.editor.active_tool != ActiveTool::MakePoly {
            self.set_active_tool_from_toolbar(ActiveTool::MakePoly);
        }
        if !self.editor.active_workspace.has_drawing_aids() || self.editor.active_tool != ActiveTool::MakePoly || self.editor.poly_finish_dialog {
            return;
        }
        self.editor.copy_along = Some(CopyAlong {
            anchor: self.editor.pending_stroke.first().copied(),
            first: None,
        });
        self.invalidate_overlay();
    }

    /// Leave Copy Along a String and go on drawing.
    pub(crate) fn cancel_copy_along(&mut self) {
        if self.editor.copy_along.take().is_some() {
            self.invalidate_overlay();
        }
    }

    /// A click while copying: the vertex the run starts from, then the one
    /// it ends at on the same string, which appends the run.
    pub(crate) fn copy_along_click(&mut self) {
        let Some(copy) = copy_along_live(&self.editor).copied() else {
            self.cancel_copy_along();
            return;
        };
        match copy.first {
            None => {
                let Some((SceneEntityId::Object(id), _)) = self.pick_copy_source() else {
                    return;
                };
                let Some((index, world)) = self.string_vertex_under_cursor(id, None) else {
                    return;
                };
                self.editor.copy_along = Some(CopyAlong {
                    first: Some((id, index, world)),
                    ..copy
                });
                self.invalidate_overlay();
            }
            Some((id, first, _)) => {
                let Some((second, _)) = self.string_vertex_under_cursor(id, Some(first)) else {
                    return;
                };
                if second == first {
                    return;
                }
                let run = match self.scene_document.get_object(id) {
                    Some(Object::Polyline { verts, closed, .. }) => copy_run(verts, *closed, first, second),
                    _ => Vec::new(),
                };
                self.editor.copy_along = None;
                self.append_copied_run(run);
            }
        }
    }

    /// Start Carry On a String: the next click picks the string, and the
    /// end nearer the click starts the stroke.
    pub(crate) fn arm_carry_on_string(&mut self) {
        self.arm_string_rework(StringRework::Picking);
    }

    /// Start Redraw Between Vertices: the next click picks the vertex the
    /// string is redrawn from.
    pub(crate) fn arm_redraw_run(&mut self) {
        self.arm_string_rework(StringRework::PickingRun);
    }

    fn arm_string_rework(&mut self, picking: StringRework) {
        if self.editor.active_workspace.has_drawing_aids() && self.editor.active_tool != ActiveTool::MakePoly {
            self.set_active_tool_from_toolbar(ActiveTool::MakePoly);
        }
        if !self.editor.active_workspace.has_drawing_aids()
            || self.editor.active_tool != ActiveTool::MakePoly
            || self.editor.poly_finish_dialog
            || !self.editor.pending_stroke.is_empty()
        {
            return;
        }
        self.editor.copy_along = None;
        self.editor.string_rework = Some(picking);
        self.invalidate_overlay();
    }

    /// Leave Carry On a String before its pick and go on drawing.
    pub(crate) fn cancel_carry_on(&mut self) {
        if self.editor.string_rework.take().is_some() {
            self.invalidate_overlay();
        }
    }

    /// The click that picks the string to carry on: its end nearer the
    /// cursor becomes the stroke's first point. A closed string has no end.
    /// For a redraw, the vertex under the cursor, of any string.
    pub(crate) fn carry_on_pick(&mut self) {
        if !self.editing_ready() {
            return;
        }
        let Some((SceneEntityId::Object(id), _)) = self.pick_under_cursor() else {
            return;
        };
        // Locked and hidden strings are not picked; the string is read from
        // its own project.
        if !self.activate_project_for_object(id) || !self.editor.canvas_edits_object(self.active_document(), id) || self.editor.active_tool != ActiveTool::MakePoly {
            return;
        }
        let Some(string @ Object::Polyline { verts, closed, .. }) = self.active_document().get_object(id) else {
            return;
        };
        if string_rework_live(&self.editor) == Some(&StringRework::PickingRun) {
            let (Some((start, _)), string) = (self.string_vertex_under_cursor(id, None), string.clone()) else {
                return;
            };
            self.editor.redraw_from(string, start);
            self.sync_next_vertex_panel();
            self.invalidate_overlay();
            return;
        }
        let start = if *closed { Some(0) } else { self.string_end_nearer_cursor(verts) };
        let (Some(start), string) = (start, string.clone()) else {
            return;
        };
        if !self.editor.carry_on_from(string, start) {
            userspace_warn!("{}", tr!("cmd-string-rework-closed"));
        }
        self.sync_next_vertex_panel();
        self.invalidate_overlay();
    }

    /// Of an open string's two ends, the index of the one nearer the cursor
    /// on screen; an end outside the section's slab is not offered.
    fn string_end_nearer_cursor(&self, verts: &[PolyVertex]) -> Option<usize> {
        let graphics = self.graphics.as_ref()?;
        let cursor = graphics.window_to_viewport_px(self.editor.cursor_screen_px?);
        let cursor = DVec2::new(f64::from(cursor.0), f64::from(cursor.1));
        let (view_proj, screen, slab) = (graphics.view_proj(), graphics.screen_size_pub(), graphics.section_slab());
        let last = verts.len().checked_sub(1).filter(|&last| last > 0)?;
        [0, last]
            .into_iter()
            .filter_map(|index| Some((index, pick::slab_screen_point(slab, &view_proj, screen, verts[index].pos)?.distance(cursor))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// Finish a carry on, when one is live: write the stroke's new vertices
    /// onto the string, closed or not, as one undo step. False when none is
    /// under way, so the stroke is saved as a string of its own.
    pub(crate) fn finish_carry_on(&mut self, closed: bool) -> bool {
        if self.redraw_holds_stroke() {
            return true;
        }
        let Some(StringRework::From { id, start, before, .. }) = string_rework_live(&self.editor).cloned() else {
            return false;
        };
        let added = self.editor.pending_stroke.split_off(1);
        if !self.end_string_rework(id, &before) {
            return true;
        }
        let mut after = before.clone();
        if let Object::Polyline { verts, closed: after_closed, .. } = &mut after {
            *verts = carried_on(verts, start, &added, closed);
            *after_closed = closed && verts.len() >= 3;
        }
        if after != before {
            self.execute_edit(Command::Replace { before, after });
        }
        true
    }

    /// A click while redrawing: on another vertex of the string it ends the
    /// redraw, ahead of any snap; anywhere else it adds a point as usual.
    pub(crate) fn redraw_click(&mut self) {
        let Some(&StringRework::Redraw { id, start, .. }) = string_rework_live(&self.editor) else {
            return;
        };
        match self.string_vertex_under_cursor(id, Some(start)) {
            Some((end, _)) if end != start => self.finish_redraw(end),
            _ => self.place_stroke_point(),
        }
    }

    /// While a redraw is live, Enter, a right click or the finish dialog
    /// only repeat its prompt: the stroke ends on a vertex of the string.
    pub(crate) fn redraw_holds_stroke(&mut self) -> bool {
        if !matches!(string_rework_live(&self.editor), Some(StringRework::Redraw { .. })) {
            return false;
        }
        self.editor.poly_finish_dialog = false;
        self.editor.poly_finish_dialog_px = None;
        userspace_log!("{}", tr!("ui-click-redraw-last-vertex"));
        true
    }

    /// End a redraw on vertex `end` of its string: the run from the picked
    /// vertex to `end` gives way to the points drawn, as one undo step.
    fn finish_redraw(&mut self, end: usize) {
        let Some(StringRework::Redraw { id, start, before, .. }) = string_rework_live(&self.editor).cloned() else {
            return;
        };
        let drawn = self.editor.pending_stroke.split_off(1);
        if !self.end_string_rework(id, &before) {
            return;
        }
        let Object::Polyline { verts, closed, .. } = &before else {
            return;
        };
        // Counted against the run drawn as one straight segment.
        let (Some(new), Some(chord)) = (redrawn(verts, *closed, start, end, &drawn), redrawn(verts, *closed, start, end, &[])) else {
            return;
        };
        let count = new.len() - chord.len();
        let mut after = before.clone();
        if let Object::Polyline { verts, .. } = &mut after {
            *verts = new;
        }
        if after != before {
            self.execute_edit(Command::Replace { before, after });
            userspace_log!("{}", tr!("cmd-string-rework-redrew", count = count.to_string()));
        }
    }

    /// End the carry on or redraw and drop its stroke, then read string `id`
    /// again: true when it still reads `before` and may be edited.
    fn end_string_rework(&mut self, id: ObjectId, before: &Object) -> bool {
        self.editor.string_rework = None;
        self.editor.pending_stroke.clear();
        self.editor.poly_finish_dialog = false;
        self.editor.poly_finish_dialog_px = None;
        self.invalidate_geometry();
        // Read again, not taken from the pick: a write over a string edited
        // since would undo that edit.
        let current = self.active_document().get_object(id);
        if current != Some(before) || !self.editor.canvas_edits_object(self.active_document(), id) {
            userspace_warn!("{}", tr!("cmd-string-rework-changed"));
            return false;
        }
        true
    }

    /// The string a copy starts from, picked as any click picks but with
    /// locked strings allowed: the copy only reads it.
    fn pick_copy_source(&self) -> Option<(SceneEntityId, DVec3)> {
        self.graphics.as_ref()?.pick_at_cursor(
            PICK_THRESHOLD_PX,
            &self.triangulations,
            &self.editor.hidden_handles,
            &Default::default(),
            self.editor.xray_enabled,
        )
    }

    /// The vertex of string `id` nearest the cursor on screen, within reach
    /// and inside the section's slab when one is up.
    fn string_vertex_under_cursor(&self, id: ObjectId, other_than: Option<usize>) -> Option<(usize, DVec3)> {
        let graphics = self.graphics.as_ref()?;
        let cursor = graphics.window_to_viewport_px(self.editor.cursor_screen_px?);
        let cursor = DVec2::new(f64::from(cursor.0), f64::from(cursor.1));
        let Some(Object::Polyline { verts, .. }) = self.scene_document.get_object(id) else {
            return None;
        };
        let (view_proj, screen, slab) = (graphics.view_proj(), graphics.screen_size_pub(), graphics.section_slab());
        verts
            .iter()
            .enumerate()
            .filter_map(|(index, vertex)| Some((index, vertex.pos, pick::slab_screen_point(slab, &view_proj, screen, vertex.pos)?.distance(cursor))))
            .filter(|&(_, _, distance)| distance < f64::from(PICK_THRESHOLD_PX * 2.5))
            // Of vertices equally near, one other than `other_than`: an open
            // string whose ends meet has two at the same place.
            .min_by(|a, b| a.2.total_cmp(&b.2).then((Some(a.0) == other_than).cmp(&(Some(b.0) == other_than))))
            .map(|(index, world, _)| (index, world))
    }

    /// Append a copied run vertex by vertex, as clicks would: a vertex that
    /// already ends the stroke is not doubled, and one back on the stroke's
    /// first vertex closes the polyline there.
    fn append_copied_run(&mut self, run: Vec<DVec3>) {
        let originals = run.clone();
        let run = match self.drawing_section() {
            Some(slab) => {
                let (run, shift) = pinned_run(run, &slab);
                if shift > REPORTED_SECTION_SHIFT {
                    userspace_log!("{}", tr!("cmd-next-vertex-moved-onto-section", distance = format!("{shift:.2}")));
                }
                run
            }
            None => run,
        };
        let mut added = 0;
        for (original, point) in originals.into_iter().zip(run) {
            // A click snapped to the string keeps the vertex where it is, off
            // the plane within the slab, so the stroke is matched against the
            // vertex as it was as well as as it is now.
            if ends_stroke(&self.editor.pending_stroke, original) {
                continue;
            }
            let point = match self.editor.pending_stroke.first() {
                Some(&first) if self.editor.pending_stroke.len() > 1 && (first - original).length_squared() <= 1.0e-16 => first,
                _ => point,
            };
            let before = self.editor.pending_stroke.len();
            self.add_stroke_vertex(point);
            let after = self.editor.pending_stroke.len();
            if after > before {
                added += 1;
            }
            // Closed on its first vertex and saved: the run ends with it.
            if after == 0 {
                break;
            }
        }
        self.editor.stroke_leg_broken = false;
        self.invalidate_overlay();
        if added > 0 {
            userspace_log!("{}", tr!("cmd-next-vertex-copied-vertices", count = added.to_string()));
        }
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

/// Whether `point` is where the stroke already ends, by the coincidence test
/// the polyline tool uses for a string brought back to its first vertex.
fn ends_stroke(stroke: &[DVec3], point: DVec3) -> bool {
    stroke.last().is_some_and(|last| (*last - point).length_squared() <= 1.0e-16)
}

fn draws_strings(tool: ActiveTool) -> bool {
    matches!(tool, ActiveTool::MakeLine | ActiveTool::MakePoly)
}

/// String `verts` carried on from its vertex `start` (0 or the last) by
/// the points `added`, in the order drawn: from the start they go in front,
/// reversed, so the string keeps its direction. Closed, the far end joins
/// the last point drawn. The old segments keep their arcs; the new ones are
/// straight.
pub(crate) fn carried_on(verts: &[PolyVertex], start: usize, added: &[DVec3], closed: bool) -> Vec<PolyVertex> {
    let added = added.iter().map(|&point| PolyVertex::straight(point));
    let mut verts: Vec<PolyVertex> = if start == 0 {
        added.rev().chain(verts.iter().copied()).collect()
    } else {
        // The old end's bulge led nowhere; kept, it would bend the first
        // new segment.
        let mut verts = verts.to_vec();
        if let Some(end) = verts.last_mut() {
            end.bulge = 0.0;
        }
        verts.extend(added);
        verts
    };
    if closed {
        // A last point drawn on the far end closes there, not doubled: that
        // new point goes, never the old far end and its arc.
        if verts.len() > 3
            && verts
                .first()
                .zip(verts.last())
                .is_some_and(|(first, last)| (first.pos - last.pos).length_squared() <= 1.0e-16)
        {
            if start == 0 {
                verts.remove(0);
            } else {
                verts.pop();
            }
        }
        // The closing segment is new, so straight.
        if let Some(end) = verts.last_mut() {
            end.bulge = 0.0;
        }
    }
    verts
}

/// String `verts` with its run from vertex `first` to vertex `last` redrawn
/// through the points `drawn`, in the order drawn from `first`: an open
/// string loses the vertices between the two, a ring the shorter way round.
/// The run's arcs go, the rest stay. None when `first` is `last` or either
/// is out of range.
pub(crate) fn redrawn(verts: &[PolyVertex], closed: bool, first: usize, last: usize, drawn: &[DVec3]) -> Option<Vec<PolyVertex>> {
    let (start, end) = match closed {
        true => choose_closed_span(verts, first, last, false),
        false => (first.min(last), first.max(last)),
    };
    let mut drawn = drawn.to_vec();
    // A point drawn last on the end vertex, by a copy along, is not doubled.
    let end_at = verts.get(last)?.pos;
    while drawn.last().is_some_and(|point| (end_at - *point).length_squared() <= 1.0e-16) {
        drawn.pop();
    }
    if start != first {
        drawn.reverse();
    }
    // One segment of the Bezier span replace is the chord: the run taken out.
    let mut verts = replace_polyline_span_with_bezier(verts, closed, start, end, DVec3::ZERO, DVec3::ZERO, 1)?;
    // A ring run across vertex 0 comes back rotated, ending on `start`.
    let at = if start < end { start + 1 } else { start - end + 1 };
    verts.splice(at..at, drawn.into_iter().map(PolyVertex::straight));
    Some(verts)
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

/// The points of a string from vertex `first` to vertex `second`, in that
/// order, arcs tessellated. An open string runs back when `second` comes
/// first; a ring goes the shorter way round, forward on a tie.
pub(crate) fn copy_run(verts: &[PolyVertex], closed: bool, first: usize, second: usize) -> Vec<DVec3> {
    let (start, end) = match closed {
        true => choose_closed_span(verts, first, second, false),
        false => (first.min(second), first.max(second)),
    };
    let mut run = directed_span_points(verts, closed, start, end);
    if start != first {
        run.reverse();
    }
    run
}

/// A copied run moved square onto the section, with the furthest any point
/// had to move.
fn pinned_run(run: Vec<DVec3>, slab: &SectionSlab) -> (Vec<DVec3>, f64) {
    let mut shift = 0.0_f64;
    let run = run
        .into_iter()
        .map(|point| {
            let on_plane = onto_section(slab, point);
            shift = shift.max(on_plane.distance(point));
            on_plane
        })
        .collect();
    (run, shift)
}
