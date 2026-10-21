use glam::DVec3;

use crate::{
    app::{App, CollarMoveSession, GizmoDragConstraint, GizmoDragState, MoveSession, VertexDrag},
    logging::CommandReportSpec,
    model::{
        Command, Object, ObjectId, ObjectPoint, SceneEntityId,
        drill_hole::{DrillHoleRef, HolePlacement},
    },
    rendering::camera::SectionSlab,
    ui::state::ActiveTool,
};

impl<'a> App<'a> {
    pub(crate) fn apply_move_delta(&mut self, delta: DVec3) {
        // Which of the two translate tools this is cannot be read off the
        // active tool alone: the panel's Apply button clears the tool before
        // the command it pushed is handled. The live session says it instead,
        // and the tool answers only for the case where nothing was previewed.
        if self.collar_move_session.is_some() || self.editor.active_tool == ActiveTool::MoveCollar {
            self.apply_collar_move_delta(delta);
            return;
        }
        let delta = self.held_to_section(delta);
        self.ensure_move_session_original();
        self.preview_move_delta(delta);
        let Some(session) = self.move_session_original.take() else {
            return;
        };
        let active_runtime_id = self.workspace.active_project().map(|project| project.runtime_id);
        if active_runtime_id != Some(session.project_runtime_id) {
            self.restore_move_session(session);
            self.reset_move_editor_state();
            return;
        }
        let commands: Vec<Command> = session
            .originals
            .into_iter()
            .filter_map(|before| {
                let after = self.active_document().get_object(before.id()).cloned()?;
                objects_differ(&before, &after).then_some(Command::Replace { before, after })
            })
            .collect();

        let moved = commands.len();
        if moved > 0 {
            self.record_applied_edit(Command::Batch(commands));
            crate::logging::report_completed_action(
                CommandReportSpec::new(
                    crate::i18n::tr!("common-move-selection"),
                    crate::i18n::tr!("common-count-object-s", count = moved.to_string()),
                ),
                crate::i18n::tr!("cmd-move-tool-moved-objects", delta = delta.to_string(), count = moved.to_string()),
            );
        }
        self.reset_move_editor_state();
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Put down whichever gesture is standing - a design move, a collar move
    /// or a collar turn - and roll the geometry back to where it was captured.
    /// All three write into the same captured originals, so one cancel covers
    /// every path that has to abandon an edit in progress.
    pub(crate) fn cancel_move_delta(&mut self) {
        self.gizmo_drag = None;
        self.vertex_drag = None;
        self.editor.gizmo_drag_axis_index = None;
        self.editor.gizmo_drag_plane_index = None;
        self.restore_move_session_original();
        self.restore_collar_session();
        self.reset_rotate_editor_state();
        self.reset_move_editor_state();
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Commit any pending move to history without applying an additional delta.
    ///
    /// Unlike `cancel_move_delta` (which reverts objects), this keeps the
    /// current document state - whatever `preview_move_delta` last produced -
    /// and pushes a Replace command for each changed object.  Used by save
    /// paths so that an in-progress move is preserved rather than silently lost.
    pub(crate) fn commit_pending_move(&mut self) {
        self.gizmo_drag = None;
        self.vertex_drag = None;
        self.collar_rotate_drag = None;
        self.editor.gizmo_drag_axis_index = None;
        self.editor.gizmo_drag_plane_index = None;
        self.editor.rotate_gizmo_drag_ring = None;
        // A standing turn is checked first: it leaves the collars where they
        // were, so the move delta read below would see zero and let it stand.
        if let Some(rotation) = self.collar_rotation {
            self.apply_collar_rotation(rotation);
            return;
        }
        // A collar preview is already written into the dataset it belongs to.
        // Its delta is not carried on the session, so it is read back off the
        // first hole the preview moved - every hole shares one delta - and the
        // move is then committed through the same path an interactive release
        // takes, so it lands on the undo stack rather than merely standing.
        if let Some(delta) = self.pending_collar_move_delta() {
            self.apply_collar_move_delta(delta);
            return;
        }

        let Some(session) = self.move_session_original.take() else {
            self.reset_move_editor_state();
            return;
        };
        let active_runtime_id = self.workspace.active_project().map(|project| project.runtime_id);
        if active_runtime_id != Some(session.project_runtime_id) {
            self.restore_move_session(session);
            self.reset_move_editor_state();
            return;
        }
        let commands: Vec<Command> = session
            .originals
            .into_iter()
            .filter_map(|before| {
                let after = self.active_document().get_object(before.id()).cloned()?;
                objects_differ(&before, &after).then_some(Command::Replace { before, after })
            })
            .collect();

        let moved = commands.len();
        if moved > 0 {
            self.record_applied_edit(Command::Batch(commands));
        }

        self.reset_move_editor_state();
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Whether any translate or turn gesture is standing over the document.
    /// Every path that has to settle or abandon an edit in progress - saving,
    /// switching project, changing tool - asks this one question, so a new
    /// gesture is wired into all of them by capturing a session.
    pub(crate) fn has_pending_move_delta(&self) -> bool {
        self.move_session_original.is_some()
            || self.collar_move_session.is_some()
            || self.editor.move_vertex_target.is_some()
            || self.gizmo_drag.is_some()
            || self.collar_rotate_drag.is_some()
    }

    pub(crate) fn begin_gizmo_drag(&mut self, axis_idx: u8, axis: DVec3, cursor_px: (f32, f32)) {
        if !self.editing_ready() {
            return;
        }
        if !self.has_move_targets() {
            return;
        }
        self.ensure_move_session_original();
        let (axis_screen_dir, px_per_world_unit) = self.gizmo_axis_screen_basis(axis_idx, cursor_px);
        let start_delta = DVec3::new(self.editor.move_panel_delta[0], self.editor.move_panel_delta[1], self.editor.move_panel_delta[2]);

        self.editor.gizmo_drag_axis_index = Some(axis_idx);
        self.editor.gizmo_drag_plane_index = None;
        self.gizmo_drag = Some(GizmoDragState {
            constraint: GizmoDragConstraint::Axis {
                axis,
                screen_dir: axis_screen_dir,
                px_per_world_unit,
            },
            start_cursor_screen_px: cursor_px,
            start_delta,
        });
    }

    pub(crate) fn begin_gizmo_plane_drag(&mut self, plane_idx: u8, axes: [DVec3; 2], cursor_px: (f32, f32)) {
        if !self.editing_ready() || !self.has_move_targets() {
            return;
        }
        let Some(screen_basis) = self.gizmo_plane_screen_basis(plane_idx) else {
            return;
        };
        // Edge-on planes cannot be solved reliably from pointer movement.
        if plane_world_delta((1.0, 1.0), screen_basis).is_none() {
            return;
        }
        self.ensure_move_session_original();
        let start_delta = DVec3::new(self.editor.move_panel_delta[0], self.editor.move_panel_delta[1], self.editor.move_panel_delta[2]);
        self.editor.gizmo_drag_axis_index = None;
        self.editor.gizmo_drag_plane_index = Some(plane_idx);
        self.gizmo_drag = Some(GizmoDragState {
            constraint: GizmoDragConstraint::Plane { axes, screen_basis },
            start_cursor_screen_px: cursor_px,
            start_delta,
        });
    }

    pub(crate) fn move_gizmo_to_cursor(&mut self) {
        let Some(gizmo) = self.gizmo_drag.as_ref() else {
            return;
        };
        let Some(cursor_px) = self.editor.cursor_screen_px else {
            return;
        };
        let screen_delta = (cursor_px.0 - gizmo.start_cursor_screen_px.0, cursor_px.1 - gizmo.start_cursor_screen_px.1);
        let world_delta = match gizmo.constraint {
            GizmoDragConstraint::Axis {
                axis,
                screen_dir,
                px_per_world_unit,
            } => {
                let pixels = screen_delta.0 * screen_dir.0 + screen_delta.1 * screen_dir.1;
                gizmo.start_delta + axis * (f64::from(pixels) / px_per_world_unit)
            }
            GizmoDragConstraint::Plane { axes, screen_basis } => {
                let Some((first, second)) = plane_world_delta(screen_delta, screen_basis) else {
                    return;
                };
                gizmo.start_delta + axes[0] * first + axes[1] * second
            }
        };
        let world_delta = self.held_to_section(world_delta);
        self.editor.move_panel_delta = [world_delta.x, world_delta.y, world_delta.z];
        self.preview_move_delta(world_delta);
        self.invalidate_geometry();
    }

    pub(crate) fn finish_gizmo_drag(&mut self) {
        let Some(_gizmo) = self.gizmo_drag.take() else {
            return;
        };
        self.editor.gizmo_drag_axis_index = None;
        self.editor.gizmo_drag_plane_index = None;
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    pub(crate) fn ensure_move_session_original(&mut self) {
        if self.editor.active_tool == ActiveTool::MoveCollar {
            self.ensure_collar_session();
            return;
        }
        // The two tools never preview at once: whichever is being started
        // rolls the other one back before it captures anything of its own.
        self.restore_collar_session();
        let selected_ids = self.move_target_object_ids();
        if selected_ids.is_empty() {
            self.move_session_original = None;
            return;
        }
        let should_refresh = self
            .move_session_original
            .as_ref()
            .map(|session| {
                self.workspace.active_project().map(|project| project.runtime_id) != Some(session.project_runtime_id)
                    || session.originals.len() != selected_ids.len()
                    || session.originals.iter().any(|object| !selected_ids.contains(&object.id()))
            })
            .unwrap_or(true);
        if should_refresh {
            // A changed target set must never use already-previewed geometry as
            // its new baseline. Restore first, then capture the new selection.
            if self.move_session_original.is_some() {
                self.restore_move_session_original();
            }
            let Some(project_runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
                return;
            };
            self.move_session_original = Some(MoveSession {
                project_runtime_id,
                originals: selected_ids.iter().filter_map(|&object_id| self.active_document().get_object(object_id).cloned()).collect(),
            });
        }
    }

    pub(crate) fn preview_move_delta(&mut self, delta: DVec3) {
        if self.collar_move_session.is_some() {
            self.preview_collar_move_delta(delta);
            return;
        }
        let delta = self.held_to_section(delta);
        let Some(session) = self.move_session_original.as_ref() else {
            return;
        };
        let project_runtime_id = session.project_runtime_id;
        let originals = session.originals.clone();
        let vertex_target = self.editor.move_vertex_target;
        if let Some(index) = self.workspace.project_index_for_runtime_id(project_runtime_id)
            && let Some(project) = self.workspace.projects.get_mut(index)
        {
            for object in originals {
                let mut moved = object.clone();
                translate_move_target(&mut moved, vertex_target, delta);
                project.project.document.replace_object(moved);
            }
        }
    }

    /// On a section a move stays on it: the part of `delta` across the
    /// section plane is dropped, typed or dragged.
    fn held_to_section(&self, delta: DVec3) -> DVec3 {
        match self.graphics.as_ref().and_then(|graphics| graphics.section_slab()) {
            Some(slab) => delta - slab.normal * delta.dot(slab.normal),
            None => delta,
        }
    }

    pub(crate) fn restore_move_session_original(&mut self) {
        let Some(session) = self.move_session_original.take() else {
            return;
        };
        self.restore_move_session(session);
    }

    fn restore_move_session(&mut self, session: MoveSession) {
        let Some(index) = self.workspace.project_index_for_runtime_id(session.project_runtime_id) else {
            return;
        };
        let project = &mut self.workspace.projects[index];
        for object in session.originals {
            project.project.document.replace_object(object);
        }
    }

    fn reset_move_editor_state(&mut self) {
        self.editor.move_vertex_target = None;
        self.editor.move_panel_delta = [0.0; 3];
        self.editor.move_panel_last_preview = [f64::NAN; 3];
    }

    fn selected_object_ids(&self) -> Vec<ObjectId> {
        self.editor
            .selected_handles
            .iter()
            .filter_map(|handle| match handle {
                SceneEntityId::Object(object_id) => Some(*object_id),
                _ => None,
            })
            .collect()
    }

    fn move_target_object_ids(&self) -> Vec<ObjectId> {
        if let Some((object_id, _)) = self.editor.move_vertex_target {
            vec![object_id]
        } else {
            self.selected_object_ids()
        }
    }

    /// Whether the active translate tool has anything under it to move. Both
    /// gizmo drags ask this before they capture a session, so neither tool
    /// starts a drag against an empty selection.
    fn has_move_targets(&self) -> bool {
        if self.editor.active_tool == ActiveTool::MoveCollar {
            !self.collar_targets().is_empty()
        } else {
            !self.move_target_object_ids().is_empty()
        }
    }

    /// The holes a collar gesture is holding, in a fixed order so a session
    /// can be compared against the selection without regard to hash order.
    /// Holes in datasets that are no longer loaded are dropped: a closed
    /// dataset has no geometry left to move or turn.
    pub(super) fn collar_targets(&self) -> Vec<DrillHoleRef> {
        let mut targets: Vec<DrillHoleRef> = self
            .editor
            .selected_drill_holes
            .iter()
            .copied()
            .filter(|target| {
                self.drill_holes
                    .iter()
                    .any(|dataset| dataset.id == target.dataset && dataset.state.loaded && target.hole < dataset.dataset.holes.len())
            })
            .collect();
        targets.sort_by_key(|target| (target.dataset.0, target.hole));
        targets
    }

    /// Capture the selected holes as they stand, so every preview - a move or
    /// a turn - can be written from the originals rather than from the last
    /// preview. Mirrors `ensure_move_session_original`, down to re-capturing
    /// when the selection changes under a live preview.
    pub(super) fn ensure_collar_session(&mut self) {
        // Starting a collar gesture ends any design move that was still previewing.
        self.restore_move_session_original();
        let targets = self.collar_targets();
        if targets.is_empty() {
            // Nothing left to move - the holes were deselected, or the dataset
            // holding them was closed - so a preview standing over them is put
            // back rather than abandoned where it was dragged to.
            self.restore_collar_session();
            return;
        }
        let should_refresh = self
            .collar_move_session
            .as_ref()
            .map(|session| {
                self.workspace.active_project().map(|project| project.runtime_id) != Some(session.project_runtime_id)
                    || session.originals.len() != targets.len()
                    || session.originals.iter().zip(&targets).any(|((captured, _), target)| captured != target)
            })
            .unwrap_or(true);
        if !should_refresh {
            return;
        }
        // A changed target set must never use already-previewed geometry as
        // its new baseline. Restore first, then capture the new selection.
        self.restore_collar_session();
        let Some(project_runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let originals: Vec<_> = targets
            .into_iter()
            .filter_map(|target| {
                let dataset = self.drill_holes.iter().find(|dataset| dataset.id == target.dataset)?;
                Some((target, dataset.dataset.holes.get(target.hole)?.placement()))
            })
            .collect();
        let mut epochs: Vec<(crate::model::drill_hole::DrillHoleId, u64)> = Vec::new();
        for (target, _) in &originals {
            if epochs.iter().any(|(id, _)| *id == target.dataset) {
                continue;
            }
            if let Some(dataset) = self.drill_holes.iter().find(|dataset| dataset.id == target.dataset) {
                epochs.push((dataset.id, dataset.state.epoch()));
            }
        }
        self.collar_move_session = Some(CollarMoveSession {
            project_runtime_id,
            originals,
            epochs,
        });
    }

    /// Roll back a live collar preview - a move or a turn, they share the one
    /// session - if it is holding holes in the dataset that is about to be
    /// closed or removed. The holes go back where they were rather than the
    /// preview being left standing over a dataset that is no longer the one it
    /// was captured from.
    pub(crate) fn cancel_collar_move_touching(&mut self, dataset: crate::model::drill_hole::DrillHoleId) {
        if self
            .collar_move_session
            .as_ref()
            .is_some_and(|session| session.originals.iter().any(|(target, _)| target.dataset == dataset))
        {
            self.cancel_move_delta();
        }
    }

    /// How far a live Move Collar preview currently stands from where it was
    /// captured, or `None` when no preview is standing.
    fn pending_collar_move_delta(&self) -> Option<DVec3> {
        let session = self.collar_move_session.as_ref()?;
        let (target, original) = session.originals.first()?;
        let dataset = self.drill_holes.iter().find(|dataset| dataset.id == target.dataset)?;
        let hole = dataset.dataset.holes.get(target.hole)?;
        Some(hole.collar - original.collar)
    }

    fn preview_collar_move_delta(&mut self, delta: DVec3) {
        // The session is taken rather than borrowed so the originals can be
        // read while the datasets holding them are written, without cloning
        // every hole again on each frame of a drag.
        let Some(session) = self.collar_move_session.take() else {
            return;
        };
        self.write_collar_holes(&session.originals, delta);
        self.collar_move_session = Some(session);
    }

    /// Put the captured holes back exactly as they were, ending the preview.
    ///
    /// The datasets' content epochs go back too: the preview wrote to them,
    /// but the content it wrote is gone, so an abandoned drag must not leave
    /// them looking edited.
    pub(crate) fn restore_collar_session(&mut self) {
        let Some(session) = self.collar_move_session.take() else {
            return;
        };
        self.write_collar_holes(&session.originals, DVec3::ZERO);
        self.restore_collar_epochs(&session.epochs);
        self.invalidate_topology_bounds_and_redraw();
    }

    pub(super) fn restore_collar_epochs(&mut self, epochs: &[(crate::model::drill_hole::DrillHoleId, u64)]) {
        for (id, epoch) in epochs {
            if let Some(dataset) = self.drill_holes.iter_mut().find(|dataset| dataset.id == *id) {
                dataset.state.restore_epoch(*epoch);
            }
        }
    }

    /// Settle a collar move at `delta` as one undo step.
    ///
    /// The live preview is rolled back first - positions and epochs both - so
    /// the command applies from the state the drag started in and undo has
    /// somewhere clean to return to. Applying it immediately rewrites the same
    /// positions the preview was already showing, so nothing moves on screen.
    fn apply_collar_move_delta(&mut self, delta: DVec3) {
        self.ensure_collar_session();
        let Some(session) = self.collar_move_session.take() else {
            return;
        };
        // A project switch under a live preview leaves the holes where they
        // were rather than committing them against whatever is open now.
        let same_project = self.workspace.active_project().map(|project| project.runtime_id) == Some(session.project_runtime_id);
        self.write_collar_holes(&session.originals, DVec3::ZERO);
        self.restore_collar_epochs(&session.epochs);
        let moved = session.originals.len();
        if !same_project || moved == 0 || delta == DVec3::ZERO {
            self.reset_move_editor_state();
            self.invalidate_topology_bounds_and_redraw();
            return;
        }

        let mut per_dataset: Vec<(crate::model::drill_hole::DrillHoleId, Vec<(usize, HolePlacement)>)> = Vec::new();
        for (target, placement) in session.originals {
            match per_dataset.iter_mut().find(|(id, _)| *id == target.dataset) {
                Some((_, holes)) => holes.push((target.hole, placement)),
                None => per_dataset.push((target.dataset, vec![(target.hole, placement)])),
            }
        }
        let commands: Vec<Command> = per_dataset
            .into_iter()
            .map(|(dataset, originals)| Command::MoveCollars { dataset, originals, delta })
            .collect();
        self.execute_edit(if commands.len() == 1 {
            commands.into_iter().next().expect("checked length")
        } else {
            Command::Batch(commands)
        });
        crate::logging::report_completed_action(
            CommandReportSpec::new(
                crate::i18n::tr!("common-move-collar"),
                crate::i18n::tr!("cmd-move-tool-count-hole-s", count = moved.to_string()),
            ),
            crate::i18n::tr!("cmd-move-tool-moved-collars", delta = delta.to_string(), count = moved.to_string()),
        );
        self.reset_move_editor_state();
        self.invalidate_topology_bounds_and_redraw();
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Rewrite every captured hole as its original translated by `delta` - a
    /// zero delta therefore restores, which is the whole of how a preview is
    /// rolled back.
    fn write_collar_holes(&mut self, originals: &[(DrillHoleRef, HolePlacement)], delta: DVec3) {
        self.write_collar_placements(originals, |hole, original| hole.set_placement(original, delta));
    }

    /// Rewrite every captured hole from the original it was captured at, by
    /// whatever rule `write` applies - a translation for Move Collar, a turn
    /// for Rotate Collar. Writing from the originals rather than from what is
    /// on screen is what keeps a drag free of accumulated drift.
    ///
    /// Each dataset is unshared once and its extent recomputed, and its
    /// revision carries the change through to the instance cache: see
    /// `rendering::scene::drill_hole_cache`.
    pub(super) fn write_collar_placements(&mut self, originals: &[(DrillHoleRef, HolePlacement)], write: impl Fn(&mut crate::model::drill_hole::DrillHole, &HolePlacement)) {
        for dataset in &mut self.drill_holes {
            if !originals.iter().any(|(target, _)| target.dataset == dataset.id) {
                continue;
            }
            let data = std::sync::Arc::make_mut(&mut dataset.dataset);
            for (target, original) in originals.iter().filter(|(target, _)| target.dataset == dataset.id) {
                let Some(hole) = data.holes.get_mut(target.hole) else {
                    continue;
                };
                write(hole, original);
            }
            data.refresh_bounds();
            dataset.state.touch();
        }
        // A redraw only: the cached scene bounds are dropped once the move
        // settles rather than on every frame of a drag, which would throw away
        // the design AABBs cached alongside them each time the pointer moves.
        self.request_topology_redraw();
    }

    fn gizmo_axis_screen_basis(&self, axis_idx: u8, cursor_px: (f32, f32)) -> ((f32, f32), f64) {
        let gizmo = &self.editor.move_gizmo;
        let index = usize::from(axis_idx).min(2);
        let center = gizmo.center_px.unwrap_or(cursor_px);
        let tip = gizmo.axis_tip_px[index].unwrap_or(cursor_px);
        let dx = tip.0 - center.0;
        let dy = tip.1 - center.1;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        ((dx / len, dy / len), gizmo.axis_px_per_world[index].max(0.001))
    }

    /// Screen-pixel vectors produced by one world unit along each of the two
    /// axes a plane handle - or the view ring - constrains movement to.
    fn gizmo_plane_screen_basis(&self, plane_idx: u8) -> Option<[(f64, f64); 2]> {
        let gizmo = &self.editor.move_gizmo;
        if plane_idx == crate::ui::state::MOVE_GIZMO_VIEW_PLANE {
            return gizmo.view_axes.is_some().then_some(gizmo.view_basis_px);
        }
        let center = gizmo.center_px?;
        let vector = |index: usize| {
            let tip = gizmo.axis_tip_px[index]?;
            let dx = f64::from(tip.0 - center.0);
            let dy = f64::from(tip.1 - center.1);
            let length = (dx * dx + dy * dy).sqrt();
            let px_per_world = gizmo.axis_px_per_world[index];
            (length > 1.0e-6).then_some((dx / length * px_per_world, dy / length * px_per_world))
        };
        match plane_idx {
            0 => Some([vector(0)?, vector(1)?]),
            1 => Some([vector(0)?, vector(2)?]),
            2 => Some([vector(1)?, vector(2)?]),
            _ => None,
        }
    }
}

fn plane_world_delta(screen_delta: (f32, f32), screen_basis: [(f64, f64); 2]) -> Option<(f64, f64)> {
    let [(ax, ay), (bx, by)] = screen_basis;
    let determinant = ax * by - ay * bx;
    let scale = (ax * ax + ay * ay).sqrt().max((bx * bx + by * by).sqrt()).max(1.0);
    if determinant.abs() <= scale * scale * 1.0e-4 {
        return None;
    }
    let dx = f64::from(screen_delta.0);
    let dy = f64::from(screen_delta.1);
    Some(((dx * by - dy * bx) / determinant, (ax * dy - ay * dx) / determinant))
}

/// How far a press on a vertex travels, in points, before it drags.
const VERTEX_DRAG_START_PT: f32 = 4.0;

impl<'a> App<'a> {
    /// Geology's Move: a press that picked a vertex drags it once the mouse
    /// moves; a click without moving leaves the gizmo on it, as before.
    pub(crate) fn begin_vertex_drag(&mut self) {
        self.vertex_drag = None;
        // Holding another vertex ends the last one's undo step.
        if self.editor.vertex_edit_run.is_some_and(|(held, _)| Some(held) != self.editor.move_vertex_target) {
            self.editor.vertex_edit_run = None;
        }
        if !self.editor.active_workspace.has_drawing_aids() {
            return;
        }
        let (Some(press_px), Some(start)) = (self.editor.cursor_screen_px, self.held_vertex_position()) else {
            return;
        };
        let Some(start_world) = self.graphics.as_ref().and_then(|graphics| graphics.cursor_world(start.z)) else {
            return;
        };
        self.vertex_drag = Some(VertexDrag {
            press_px,
            level: start.z,
            start_world,
            moved: false,
        });
    }

    /// The held vertex follows the cursor: on its own level in plan, in the
    /// plane on a section.
    pub(crate) fn drag_vertex_to_cursor(&mut self) {
        let (Some(drag), Some(cursor_px)) = (self.vertex_drag, self.editor.cursor_screen_px) else {
            return;
        };
        if !self.editor.active_workspace.has_drawing_aids() || self.editor.active_tool != ActiveTool::Move {
            self.vertex_drag = None;
            return;
        }
        if !drag.moved {
            let travelled = (cursor_px.0 - drag.press_px.0).hypot(cursor_px.1 - drag.press_px.1);
            if travelled < self.points_to_px(VERTEX_DRAG_START_PT) {
                return;
            }
            self.vertex_drag = Some(VertexDrag { moved: true, ..drag });
        }
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let Some(world) = graphics.cursor_world(drag.level) else {
            return;
        };
        let delta = dragged_delta(drag.start_world, world, graphics.section_slab());
        self.ensure_move_session_original();
        self.preview_move_delta(delta);
        self.editor.move_panel_delta = delta.to_array();
        self.editor.move_panel_last_preview = self.editor.move_panel_delta;
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Put the dragged vertex down, joining the vertex's undo step, and
    /// keep it held.
    pub(crate) fn finish_vertex_drag(&mut self) {
        let Some(drag) = self.vertex_drag.take() else {
            return;
        };
        let Some(target) = self.editor.move_vertex_target.filter(|_| drag.moved) else {
            return;
        };
        let delta = DVec3::from_array(self.editor.move_panel_delta);
        self.record_vertex_edit(target, |app| app.apply_move_delta(delta));
        self.hold_vertex(target);
    }

    /// The Drag a Vertex row: Move armed, unless it is already.
    pub(crate) fn arm_drag_vertex(&mut self) {
        if self.editor.active_tool != ActiveTool::Move {
            self.set_active_tool_from_toolbar(ActiveTool::Move);
        }
    }

    /// The Nudge RL row: Move armed if it is not, and the panel open.
    pub(crate) fn open_nudge_panel(&mut self) {
        self.arm_drag_vertex();
        self.editor.nudge_panel_open = self.editor.active_tool == ActiveTool::Move;
        self.redraw_requested = true;
    }

    /// Raise the vertex Move holds by `rise` metres, lower it if negative:
    /// each nudge joins the vertex's undo step, and the vertex stays held.
    pub(crate) fn nudge_held_vertex(&mut self, rise: f64) {
        if !self.editing_ready() || !rise.is_finite() || rise == 0.0 || self.editor.active_tool != ActiveTool::Move {
            return;
        }
        // Undo, or an edit by another path, may have taken the vertex away.
        let Some(target) = self.resolve_held_vertex() else {
            self.cancel_move_delta();
            return;
        };
        if self.held_vertex_position().is_none() {
            self.cancel_move_delta();
            return;
        }
        // A move standing over the vertex, dragged or typed, lands first.
        self.commit_pending_move();
        self.hold_vertex(target);
        self.record_vertex_edit(target, |app| app.apply_move_delta(DVec3::Z * rise));
        self.hold_vertex(target);
    }

    /// Run `edit` on held vertex `target` so its undo step joins the run of
    /// drags and nudges already made on that vertex: one undo takes it back
    /// to where the work on it began. Another vertex starts a run of its own.
    fn record_vertex_edit(&mut self, target: (ObjectId, ObjectPoint), edit: impl FnOnce(&mut Self)) {
        self.vertex_edit_revision = Some(run_onto(self.editor.vertex_edit_run, target));
        edit(self);
        self.editor.vertex_edit_run = self.vertex_edit_revision.take().flatten().map(|revision| (target, revision));
    }

    /// Where the vertex Move holds is now, if it is still there and may be
    /// edited: locked and hidden strings are refused.
    fn held_vertex_position(&self) -> Option<DVec3> {
        let (id, point) = self.editor.move_vertex_target?;
        if !self.editor.canvas_edits_object(self.active_document(), id) {
            return None;
        }
        move_target_position(self.active_document().get_object(id)?, point)
    }

    /// The vertex the last drag or nudge held, found again by where it was
    /// left if its string was renumbered since; `None` when it is gone.
    /// A vertex Move picked by other means is taken as it is.
    fn resolve_held_vertex(&mut self) -> Option<(ObjectId, ObjectPoint)> {
        let target = self.editor.move_vertex_target?;
        let Some((_, at)) = self.editor.held_vertex_at.filter(|(held, _)| *held == target) else {
            return Some(target);
        };
        let object = self.active_document().get_object(target.0)?;
        if move_target_position(object, target.1).is_some_and(|now| now.distance_squared(at) <= 1.0e-18) {
            return Some(target);
        }
        let Object::Polyline { verts, .. } = object else {
            return None;
        };
        let index = verts.iter().position(|vertex| vertex.pos.distance_squared(at) <= 1.0e-18)?;
        let found = (target.0, ObjectPoint::Vertex(index));
        self.hold_vertex(found);
        Some(found)
    }

    fn hold_vertex(&mut self, target: (ObjectId, ObjectPoint)) {
        self.editor.held_vertex_at = self
            .active_document()
            .get_object(target.0)
            .and_then(|object| move_target_position(object, target.1))
            .map(|at| (target, at));
        self.editor.move_vertex_target = Some(target);
        self.editor.selected_handles.clear();
        self.editor.selected_handles.insert(SceneEntityId::Object(target.0));
        self.invalidate_geometry();
        self.invalidate_overlay();
    }
}

/// The revision a drag or nudge of `target` may extend: the run's, if the
/// run is on that same vertex.
fn run_onto(run: Option<((ObjectId, ObjectPoint), u64)>, target: (ObjectId, ObjectPoint)) -> Option<u64> {
    run.filter(|(held, _)| *held == target).map(|(_, revision)| revision)
}

/// How far a drag from `start` to `now` moves the vertex: across the
/// section's plane not at all, and in plan only on the level.
fn dragged_delta(start: DVec3, now: DVec3, slab: Option<SectionSlab>) -> DVec3 {
    let delta = now - start;
    match slab {
        Some(slab) => delta - slab.normal * delta.dot(slab.normal),
        None => DVec3::new(delta.x, delta.y, 0.0),
    }
}

/// Where the point a Move vertex target names is on `object`.
pub(crate) fn move_target_position(object: &Object, point: ObjectPoint) -> Option<DVec3> {
    match (object, point) {
        (Object::Polyline { verts, .. }, ObjectPoint::Vertex(index)) => verts.get(index).map(|vertex| vertex.pos),
        (Object::Circle { center, .. }, ObjectPoint::Center) => Some(*center),
        (Object::Point { pos, .. }, ObjectPoint::Vertex(0)) => Some(*pos),
        _ => None,
    }
}

fn translate_move_target(object: &mut Object, vertex_target: Option<(ObjectId, ObjectPoint)>, delta: DVec3) {
    let Some((target_id, point)) = vertex_target else {
        object.translate(delta);
        return;
    };
    if object.id() != target_id {
        return;
    }
    match (object, point) {
        (Object::Polyline { verts, .. }, ObjectPoint::Vertex(vertex_index)) => {
            if let Some(vertex) = verts.get_mut(vertex_index) {
                vertex.pos += delta;
            }
        }
        // Dragging a circle's centre moves the whole circle.
        (object @ Object::Circle { .. }, ObjectPoint::Center) => object.translate(delta),
        (Object::Point { pos, .. }, ObjectPoint::Vertex(0)) => {
            *pos += delta;
        }
        _ => {}
    }
}

fn objects_differ(before: &Object, after: &Object) -> bool {
    before != after
}
