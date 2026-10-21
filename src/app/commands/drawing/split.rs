use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::insert_point::{CrossingCuts, crossing_cuts, edge_count, insert_parameters};
use crate::{
    app::{App, PICK_THRESHOLD_PX},
    i18n::tr,
    logging::CommandReportSpec,
    model::{Command, Object, ObjectId, PolyVertex, SceneEntityId},
    rendering::pick,
    ui::state::ActiveTool,
    userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    pub(crate) fn split_at_points_click(&mut self) {
        if self.editor.split_poly_id.is_none() {
            self.pick_split_polyline();
        } else if self.editor.split_selected_verts[0].is_none() || self.editor.split_selected_verts[1].is_none() {
            self.pick_split_vertex();
        }
    }

    pub(crate) fn split_init_from_selection(&mut self) {
        let selected = self
            .editor
            .selected_handles
            .iter()
            .filter_map(|handle| match handle {
                SceneEntityId::Object(id) => Some(*id),
                _ => None,
            })
            .find(|id| self.is_valid_split_polyline(*id));

        if let Some(id) = selected {
            self.editor.split_poly_id = Some(id);
            self.editor.split_selected_verts = [None; 2];
            self.invalidate_overlay();
        }
    }

    fn pick_split_polyline(&mut self) {
        let picked = self.pick_under_cursor();

        let Some((SceneEntityId::Object(object_id), _)) = picked else {
            return;
        };
        if !self.activate_project_for_object(object_id) || !self.is_valid_split_polyline(object_id) {
            return;
        }

        self.editor.split_poly_id = Some(object_id);
        self.editor.split_selected_verts = [None; 2];
        self.editor.selected_handles.insert(SceneEntityId::Object(object_id));
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    fn pick_split_vertex(&mut self) {
        let Some(object_id) = self.editor.split_poly_id else {
            return;
        };
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let Some(cursor_px) = self.editor.cursor_screen_px else {
            return;
        };

        let (verts, closed) = match self.scene_document.get_object(object_id) {
            Some(Object::Polyline { verts, closed, .. }) => (verts.clone(), *closed),
            _ => return,
        };

        let vp = graphics.view_proj();
        let screen = graphics.screen_size_pub();
        let cursor_px = graphics.window_to_viewport_px(cursor_px);
        let cursor = glam::DVec2::new(f64::from(cursor_px.0), f64::from(cursor_px.1));
        let mut best_dist = (PICK_THRESHOLD_PX * 2.5) as f64;
        let mut best_idx = None;

        for (index, vertex) in verts.iter().enumerate() {
            if let Some(screen_pos) = pick::slab_screen_point(graphics.section_slab(), &vp, screen, vertex.pos) {
                let dist = screen_pos.distance(cursor);
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(index);
                }
            }
        }

        let Some(vertex_index) = best_idx else {
            return;
        };

        // An open line needs only one interior split point. Its endpoints
        // would leave an empty piece and are therefore not actionable.
        if !closed {
            if vertex_index == 0 || vertex_index + 1 == verts.len() {
                userspace_warn!("{}", tr!("cmd-split-points-needs-interior-vertex"));
                return;
            }
            self.editor.split_selected_verts = [Some(vertex_index), None];
            self.commit_split_at_points();
            return;
        }

        if self.editor.split_selected_verts[0].is_none() {
            self.editor.split_selected_verts[0] = Some(vertex_index);
            self.invalidate_overlay();
            return;
        }

        if self.editor.split_selected_verts[0] == Some(vertex_index) {
            return;
        }

        self.editor.split_selected_verts[1] = Some(vertex_index);
        self.commit_split_at_points();
    }

    fn commit_split_at_points(&mut self) {
        let Some(object_id) = self.editor.split_poly_id else {
            return;
        };
        let [Some(first), second] = self.editor.split_selected_verts else {
            return;
        };
        if !self.activate_project_for_object(object_id) {
            return;
        }

        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        let doc = &mut project.project.document;
        let Some(source) = doc.get_object(object_id).cloned() else {
            return;
        };
        let Object::Polyline {
            layer,
            verts,
            color,
            fill,
            line_weight,
            closed,
            ..
        } = &source
        else {
            return;
        };

        let pieces = if *closed {
            let Some(second) = second else {
                return;
            };
            split_closed_ring(verts, first, second)
        } else {
            let pieces = split_open_line(verts, first);
            if pieces.is_none() {
                userspace_warn!("{}", tr!("cmd-split-points-needs-interior-vertex"));
            }
            pieces
        };
        let Some((first_ring, second_ring)) = pieces else {
            self.editor.split_selected_verts = [None; 2];
            self.invalidate_overlay();
            return;
        };

        let first_id = doc.allocate_object_id();
        let second_id = doc.allocate_object_id();
        let first_object = Object::Polyline {
            id: first_id,
            layer: *layer,
            verts: first_ring,
            closed: false,
            color: *color,
            fill: *fill,
            line_weight: *line_weight,
        };
        let second_object = Object::Polyline {
            id: second_id,
            layer: *layer,
            verts: second_ring,
            closed: false,
            color: *color,
            fill: *fill,
            line_weight: *line_weight,
        };

        self.execute_edit(Command::Batch(vec![
            Command::delete_object(source),
            Command::AddObject(first_object),
            Command::AddObject(second_object),
        ]));

        self.editor.selected_handles.clear();
        self.clear_split_at_points_state();
        self.editor.active_tool = ActiveTool::None;
        crate::logging::report_completed_action(
            CommandReportSpec::new(crate::i18n::tr!("cmd-split-line"), crate::i18n::tr!("cmd-split-created-2-open-polylines")),
            crate::i18n::tr!("cmd-split-polyline-into-two"),
        );
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    pub(crate) fn cancel_split_at_points(&mut self) {
        self.clear_split_at_points_state();
        self.editor.active_tool = ActiveTool::None;
        self.invalidate_overlay();
    }

    fn clear_split_at_points_state(&mut self) {
        self.editor.split_poly_id = None;
        self.editor.split_selected_verts = [None; 2];
        self.editor.split_poly_verts_screen_px.clear();
    }

    fn is_valid_split_polyline(&self, object_id: ObjectId) -> bool {
        self.scene_document.get_object(object_id).is_some_and(|object| {
            matches!(
                object,
                Object::Polyline { verts, closed: true, .. } if verts.len() >= 3
            ) || matches!(
                object,
                Object::Polyline { verts, closed: false, .. } if verts.len() >= 3
            )
        })
    }

    /// Split at Crossings: the first click picks the string to cut, the
    /// next the string that crosses it.
    pub(crate) fn split_at_crossings_click(&mut self) {
        if !self.editing_ready() {
            return;
        }
        let Some((SceneEntityId::Object(id), _)) = self.pick_under_cursor() else {
            return;
        };
        match self.editor.split_crossings_target.take() {
            None => self.pick_crossings_target(id),
            Some(before) => self.split_where_crossed(before, id),
        }
    }

    /// As [`Self::split_init_from_selection`]: a selected string the canvas
    /// may edit is the one to cut.
    pub(crate) fn split_crossings_init_from_selection(&mut self) {
        let selected = self.editor.selected_handles.iter().find_map(|handle| match handle {
            SceneEntityId::Object(id)
                if matches!(self.scene_document.get_object(*id), Some(Object::Polyline { .. })) && self.editor.canvas_edits_object(&self.scene_document, *id) =>
            {
                Some(*id)
            }
            _ => None,
        });
        if let Some(id) = selected {
            self.pick_crossings_target(id);
        }
    }

    /// Take string `id` as the one to cut, read from its own project. A
    /// locked string may cut another but is not cut itself.
    fn pick_crossings_target(&mut self, id: ObjectId) {
        if !self.activate_project_for_object(id) || !self.editor.canvas_edits_object(self.active_document(), id) {
            return;
        }
        let Some(string @ Object::Polyline { verts, closed, .. }) = self.active_document().get_object(id) else {
            return;
        };
        if edge_count(verts, *closed) == 0 {
            return;
        }
        self.editor.split_crossings_target = Some(string.clone());
        self.editor.selected_handles.insert(SceneEntityId::Object(id));
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Cut string `before` at every crossing with string `cutter` into open
    /// pieces with new ids, as Split Polyline At Points does, one undo.
    fn split_where_crossed(&mut self, before: Object, cutter: ObjectId) {
        let Object::Polyline {
            id,
            layer,
            ref verts,
            closed,
            color,
            fill,
            line_weight,
        } = before
        else {
            return;
        };
        // The cutter as drawn, locked or not; the string itself is no cutter.
        let Some(Object::Polyline {
            verts: cutter_verts,
            closed: cutter_closed,
            ..
        }) = self.scene_document.get_object(cutter).filter(|_| cutter != id)
        else {
            self.editor.split_crossings_target = Some(before);
            return;
        };
        let slab = self.graphics.as_ref().and_then(|graphics| graphics.section_slab());
        let (vertices, parameters) = crossing_cuts((verts, closed), &[(cutter_verts, *cutter_closed)], slab);
        // Read again, not taken from the pick: a write over a string edited
        // since would undo that edit.
        if !self.activate_project_for_object(id) || self.active_document().get_object(id) != Some(&before) || !self.editor.canvas_edits_object(self.active_document(), id) {
            userspace_warn!("{}", tr!("cmd-string-rework-changed"));
            self.editor.selected_handles.remove(&SceneEntityId::Object(id));
            self.invalidate_geometry();
            self.invalidate_overlay();
            return;
        }
        let pieces = split_at_crossings(verts, closed, &vertices, parameters);
        if pieces.is_empty() {
            userspace_warn!("{}", tr!("cmd-string-split-no-crossing"));
            self.editor.split_crossings_target = Some(before);
            return;
        }
        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        let doc = &mut project.project.document;
        let count = pieces.len();
        let mut commands = vec![Command::delete_object(before)];
        for verts in pieces {
            commands.push(Command::AddObject(Object::Polyline {
                id: doc.allocate_object_id(),
                layer,
                verts,
                closed: false,
                color,
                fill,
                line_weight,
            }));
        }
        self.execute_edit(Command::Batch(commands));
        // The tool stays up for the next string.
        self.editor.selected_handles.clear();
        match count {
            1 => userspace_log!("{}", tr!("cmd-string-split-opened")),
            _ => userspace_log!("{}", tr!("cmd-string-split-into", count = count.to_string())),
        }
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Remove Between Crossings: take out the piece of the string under the
    /// cursor between the crossings either side, one undo. An end piece
    /// shortens the string, as Relimit does; a middle piece leaves two
    /// strings with new ids, as Split Polyline At Points does.
    pub(crate) fn remove_between_crossings_click(&mut self) {
        if !self.editing_ready() {
            return;
        }
        let Some((SceneEntityId::Object(id), _)) = self.pick_under_cursor() else {
            return;
        };
        // Picking takes a locked string too: it may cut, not be cut. The
        // string is read again from its own project, not from the scene.
        if !self.activate_project_for_object(id) || !self.editor.canvas_edits_object(self.active_document(), id) {
            return;
        }
        let Some(before @ Object::Polyline { verts, closed, .. }) = self.active_document().get_object(id) else {
            return;
        };
        if let Some(at) = self.edge_under_cursor(verts, *closed) {
            self.remove_piece_at(before.clone(), at);
        }
    }

    /// Take out the piece of string `before`, as read from the active
    /// project, around `at` (an edge and the parameter along it).
    fn remove_piece_at(&mut self, before: Object, at: (usize, f64)) {
        let Object::Polyline { id, ref verts, closed, .. } = before else {
            return;
        };
        let (vertices, parameters) = self.shown_crossings(id, verts, closed);
        let Some((_, kept)) = piece_between_crossings(verts, closed, &vertices, &parameters, at) else {
            userspace_warn!("{}", tr!("cmd-string-split-no-crossing"));
            return;
        };
        let command = match <[_; 1]>::try_from(kept) {
            Ok([verts]) => {
                let mut after = before.clone();
                if let Object::Polyline { verts: kept, closed, .. } = &mut after {
                    (*kept, *closed) = (verts, false);
                }
                Command::Replace { before, after }
            }
            Err(kept) => {
                let Some(project) = self.workspace.active_project_mut() else {
                    return;
                };
                let doc = &mut project.project.document;
                let mut commands = vec![Command::delete_object(before.clone())];
                for verts in kept {
                    let mut piece = before.clone();
                    if let Object::Polyline { id, verts: kept, closed, .. } = &mut piece {
                        (*id, *kept, *closed) = (doc.allocate_object_id(), verts, false);
                    }
                    commands.push(Command::AddObject(piece));
                }
                self.editor.selected_handles.remove(&SceneEntityId::Object(id));
                Command::Batch(commands)
            }
        };
        self.execute_edit(command);
        userspace_log!("{}", tr!("cmd-string-trim-removed"));
        self.clear_insert_point_marker();
        self.invalidate_geometry();
    }

    /// The piece a click would remove from the string under the cursor. The
    /// crossings are kept for the string hovered until the scene, what is
    /// hidden or the section changes, so moving along it costs no search.
    pub(crate) fn removable_piece_hit(&mut self) -> Option<Vec<PolyVertex>> {
        let (SceneEntityId::Object(id), _) = self.pick_under_cursor()? else {
            return None;
        };
        if !self.editor.canvas_edits_object(&self.scene_document, id) {
            return None;
        }
        let key = self.shown_crossings_key(id);
        let Some(Object::Polyline { verts, closed, .. }) = self.scene_document.get_object(id) else {
            return None;
        };
        if self.removal_crossings.as_ref().is_none_or(|(cached, _)| *cached != key) {
            let crossings = self.shown_crossings(id, verts, *closed);
            self.removal_crossings = Some((key, crossings));
        }
        let Some(Object::Polyline { verts, closed, .. }) = self.scene_document.get_object(id) else {
            return None;
        };
        let at = self.edge_under_cursor(verts, *closed)?;
        let (_, (vertices, parameters)) = self.removal_crossings.as_ref()?;
        piece_between_crossings(verts, *closed, vertices, parameters, at).map(|(piece, _)| piece)
    }

    /// Where the strings shown cross string `id`: every string drawn but the
    /// ones hidden, locked strings included.
    fn shown_crossings(&self, id: ObjectId, verts: &[PolyVertex], closed: bool) -> CrossingCuts {
        let cutters: Vec<(&[PolyVertex], bool)> = self
            .scene_document
            .objects()
            .iter()
            .filter_map(|object| match object {
                Object::Polyline { id: other, verts, closed, .. } if *other != id && !self.editor.hidden_handles.contains(&SceneEntityId::Object(*other)) => {
                    Some((verts.as_slice(), *closed))
                }
                _ => None,
            })
            .collect();
        let slab = self.graphics.as_ref().and_then(|graphics| graphics.section_slab());
        crossing_cuts((verts, closed), &cutters, slab)
    }

    /// What [`Self::shown_crossings`] reads beside string `id`: the
    /// documents, the strings hidden on the canvas and the section.
    fn shown_crossings_key(&self, id: ObjectId) -> u64 {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        self.workspace.composite_key().hash(&mut hasher);
        // Order-insensitive, as Insert Points at Crossings keys a selection.
        let hidden = self.editor.hidden_handles.iter().fold(0_u64, |fold, handle| match handle {
            SceneEntityId::Object(id) => fold ^ id.0.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            _ => fold,
        });
        (hidden, self.editor.hidden_handles.len()).hash(&mut hasher);
        if let Some(slab) = self.graphics.as_ref().and_then(|graphics| graphics.section_slab()) {
            [slab.point, slab.normal].map(|v| v.to_array().map(f64::to_bits)).hash(&mut hasher);
            slab.half_width.to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Put down the string picked to cut, if any.
    pub(crate) fn clear_split_at_crossings(&mut self) {
        if self.editor.split_crossings_target.take().is_some() {
            self.invalidate_overlay();
        }
    }
}

/// String `verts` cut into open pieces where [`crossing_cuts`] found
/// crossings: on `vertices`, and at `parameters` inside its edges. An open
/// string is not cut at its ends; a ring cut once is opened there. Empty
/// when there is nothing to cut.
pub(super) fn split_at_crossings(verts: &[PolyVertex], closed: bool, vertices: &[usize], parameters: Vec<Vec<f64>>) -> Vec<Vec<PolyVertex>> {
    if edge_count(verts, closed) == 0 {
        return Vec::new();
    }
    // Each cut's place once the crossings are vertices of their own.
    let mut cuts = Vec::new();
    let mut index = 0;
    for (vertex, added) in parameters.iter().map(Vec::len).enumerate() {
        if vertices.contains(&vertex) {
            cuts.push(index);
        }
        cuts.extend(index + 1..=index + added);
        index += added + 1;
    }
    let verts = insert_parameters(verts, closed, parameters);
    if !closed {
        cuts.retain(|&cut| cut > 0 && cut + 1 < verts.len());
        if cuts.is_empty() {
            return Vec::new();
        }
        let ends: Vec<usize> = std::iter::once(0).chain(cuts).chain(std::iter::once(verts.len() - 1)).collect();
        return ends.windows(2).map(|run| with_straight_closing_edge(verts[run[0]..=run[1]].to_vec())).collect();
    }
    match cuts[..] {
        [] => Vec::new(),
        [cut] => {
            let mut opened = ring_slice(&verts, cut, (cut + verts.len() - 1) % verts.len());
            opened.push(verts[cut]);
            vec![with_straight_closing_edge(opened)]
        }
        _ => (0..cuts.len())
            .map(|run| with_straight_closing_edge(ring_slice(&verts, cuts[run], cuts[(run + 1) % cuts.len()])))
            .collect(),
    }
}

/// The piece of string `verts` between the crossings either side of `at`
/// (an edge and the parameter along it), or between a crossing and an end,
/// and what is left without it: one string, or two when the piece was in
/// the middle. A ring opens and stays one string. Only the two crossings
/// that end the piece become vertices. `None` with no piece to take: no
/// crossing, or a ring crossed once.
pub(super) fn piece_between_crossings(
    verts: &[PolyVertex],
    closed: bool,
    vertices: &[usize],
    parameters: &[Vec<f64>],
    at: (usize, f64),
) -> Option<(Vec<PolyVertex>, Vec<Vec<PolyVertex>>)> {
    if at.0 >= parameters.len() {
        return None;
    }
    // Every crossing in order along the string as an edge and parameter, one
    // on vertex i as (i, 0). An open string's ends are not crossings.
    let mut crossings = Vec::new();
    for (edge, edge_parameters) in parameters.iter().enumerate() {
        if vertices.contains(&edge) && (closed || edge > 0) {
            crossings.push((edge, 0.0));
        }
        let mut sorted = edge_parameters.clone();
        sorted.sort_by(f64::total_cmp);
        crossings.extend(sorted.into_iter().map(|t| (edge, t)));
    }
    let mut before = crossings.iter().copied().rfind(|&crossing| crossing <= at);
    let mut after = crossings.iter().copied().find(|&crossing| crossing > at);
    if closed {
        if crossings.len() < 2 {
            return None;
        }
        before = before.or(crossings.last().copied());
        after = after.or(crossings.first().copied());
    }
    let mut ends = vec![Vec::new(); parameters.len()];
    for (edge, t) in before.into_iter().chain(after) {
        if t > 0.0 {
            ends[edge].push(t);
        }
    }
    // Where a crossing sits once the two ends are vertices.
    let index = |(edge, t): (usize, f64)| edge + ends[..edge].iter().map(Vec::len).sum::<usize>() + usize::from(t > 0.0) + ends[edge].iter().filter(|&&end| end < t).count();
    let (start, end) = (before.map(index), after.map(index));
    let verts = insert_parameters(verts, closed, ends);
    if closed {
        let (start, end) = (start?, end?);
        return Some((ring_slice(&verts, start, end), vec![with_straight_closing_edge(ring_slice(&verts, end, start))]));
    }
    let head = |cut: usize| with_straight_closing_edge(verts[..=cut].to_vec());
    match (start, end) {
        (None, None) => None,
        (None, Some(end)) => Some((verts[..=end].to_vec(), vec![verts[end..].to_vec()])),
        (Some(start), None) => Some((verts[start..].to_vec(), vec![head(start)])),
        (Some(start), Some(end)) => Some((verts[start..=end].to_vec(), vec![head(start), verts[end..].to_vec()])),
    }
}

fn split_open_line(verts: &[PolyVertex], split_index: usize) -> Option<(Vec<PolyVertex>, Vec<PolyVertex>)> {
    if verts.len() < 3 || split_index == 0 || split_index + 1 >= verts.len() {
        return None;
    }
    let mut first = verts[..=split_index].to_vec();
    first.last_mut().expect("the first piece contains the split vertex").bulge = 0.0;
    let second = verts[split_index..].to_vec();
    Some((first, second))
}

fn split_closed_ring(verts: &[PolyVertex], first: usize, second: usize) -> Option<(Vec<PolyVertex>, Vec<PolyVertex>)> {
    // Both pieces are open, so adjacent vertices are fine: the edge between
    // them becomes a single-segment polyline.
    if first == second || first >= verts.len() || second >= verts.len() || verts.len() < 3 {
        return None;
    }

    let first_ring = ring_slice(verts, first, second);
    let second_ring = ring_slice(verts, second, first);
    Some((with_straight_closing_edge(first_ring), with_straight_closing_edge(second_ring)))
}

fn ring_slice(verts: &[PolyVertex], start: usize, end: usize) -> Vec<PolyVertex> {
    let mut out = Vec::new();
    let mut index = start;
    loop {
        out.push(verts[index]);
        if index == end {
            break;
        }
        index = (index + 1) % verts.len();
    }
    out
}

fn with_straight_closing_edge(mut verts: Vec<PolyVertex>) -> Vec<PolyVertex> {
    if let Some(last) = verts.last_mut() {
        last.bulge = 0.0;
    }
    verts
}
