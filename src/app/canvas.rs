use crate::{
    app::{App, PICK_THRESHOLD_PX},
    i18n::{tr, tr_format},
    model::{Command, Object, SceneEntityId},
    ui::state::{ActiveTool, DrapePhase, SelectionMode, TriangulationPickTarget, Workspace},
};

impl<'a> App<'a> {
    pub(crate) fn begin_select_or_drag(&mut self) {
        self.pending_selection_click = None;

        // A tool open on a snapshot of the selection freezes it - see
        // `EditorState::selection_locked_by_tool`. The field pickers below run
        // first: the two-surface tools still name their own inputs, and their
        // pick is not a selection change.
        if self.editor.selection_locked_by_tool() {
            return;
        }

        if let Some(target) = self.editor.triangulation_pick_target {
            let picked = self
                .graphics
                .as_ref()
                .and_then(|graphics| graphics.pick_triangulation_at_cursor(&self.triangulations, &self.editor.hidden_handles, &self.editor.frozen_handles));
            if let Some((SceneEntityId::Triangulation(id), _)) = picked {
                let name = self
                    .triangulations
                    .iter()
                    .find(|triangulation| triangulation.id == id)
                    .map(|triangulation| triangulation.name.clone())
                    .unwrap_or_else(|| tr!(literal = "Surface"));
                self.apply_triangulation_field_pick(target, id, &name);
            }
            return;
        }

        // Blast-pattern boundary picker: accept only a valid closed design
        // polyline and return immediately so the ordinary selection tools do
        // not also act on the same click.
        if self.editor.drill_pattern_awaiting_shape_pick {
            let frozen = &self.editor.frozen_handles;
            let active_object_ids = self.active_project_object_ids();
            if let Some((SceneEntityId::Object(id), _)) = self
                .graphics
                .as_ref()
                .and_then(|graphics| graphics.pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, frozen, self.editor.xray_enabled))
                && active_object_ids.contains(&id)
                && let Some(object) = self.scene_document.get_object(id)
                && is_drill_pattern_boundary(object)
            {
                let layer = self.scene_document.layer(object.layer()).map(|layer| layer.name.as_str()).unwrap_or("?");
                self.editor.drill_pattern_boundary_id = Some(id);
                self.editor.drill_pattern_boundary_name = tr_format!(literal = "Polyline on '%layer%'", layer = layer);
                self.editor.drill_pattern_awaiting_shape_pick = false;
                self.editor.viewport_pick_hover_label = None;
                self.editor.tool_highlight_id = Some(id);
                self.invalidate_geometry();
            }
            return;
        }

        // Drape owns a two-stage selection session. Defer both click and box
        // picks until release so each stage can strictly filter entity types.
        if self.editor.active_tool == ActiveTool::DrapeToTopology {
            self.editor.selection_box_start_px = self.editor.cursor_screen_px;
            self.editor.selection_box_current_px = self.editor.cursor_screen_px;
            return;
        }

        if !self.workspace.has_active_project() && self.triangulations.is_empty() {
            return;
        }
        // Picking a new target abandons whatever gesture was standing over the
        // last one - a turn as much as a move.
        if self.editor.active_tool.translates() || self.editor.active_tool.rotates() {
            self.cancel_move_delta();
            self.editor.move_vertex_target = None;
        }
        let frozen = &self.editor.frozen_handles;
        let picked = self.graphics.as_ref().and_then(|graphics| {
            graphics
                .pick_scene_entity_at_cursor(
                    PICK_THRESHOLD_PX,
                    &self.triangulations,
                    &self.drill_holes,
                    &self.editor.hidden_handles,
                    frozen,
                    self.editor.xray_enabled,
                )
                .filter(|pick| self.tool_accepts_pick(pick))
                .map(|pick| (Some(pick), pick.world))
                .or_else(|| graphics.cursor_world(self.editor.z_level).map(|world| (None, world)))
        });
        match picked {
            Some((Some(pick), _)) => {
                // Resolve clicks only on release, so a box can start over any entity.
                self.pending_selection_click = Some(pick);
                self.editor.selection_box_start_px = self.editor.cursor_screen_px;
                self.editor.selection_box_current_px = self.editor.cursor_screen_px;
            }
            Some((None, world)) => {
                self.active_triangulation = None;
                if self.editor.active_tool == ActiveTool::None || self.editor.active_tool.translates() || self.editor.active_tool.rotates() {
                    self.editor.selection_box_start_px = self.editor.cursor_screen_px;
                    self.editor.selection_box_current_px = self.editor.cursor_screen_px;
                } else if self.workspace.has_active_project() {
                    self.editor.on_canvas_click(world);
                } else {
                    self.editor.selected_handles.clear();
                }
            }
            None => {
                // A background click may not intersect the current Z plane
                // (for example, when clicking above the horizon). It should
                // still begin a selection gesture so a short click can clear
                // the current selection.
                self.active_triangulation = None;
                if self.editor.active_tool == ActiveTool::None || self.editor.active_tool.translates() || self.editor.active_tool.rotates() {
                    self.editor.selection_box_start_px = self.editor.cursor_screen_px;
                    self.editor.selection_box_current_px = self.editor.cursor_screen_px;
                } else {
                    return;
                }
            }
        }
        self.invalidate_geometry();
    }

    /// Update the candidate highlight and description shown by dialog-driven
    /// viewport pickers. Work is invoked at the existing bounded hover-poll
    /// rate, and geometry is invalidated only when the candidate changes.
    pub(crate) fn update_viewport_field_pick_hover(&mut self) {
        if self.editor.triangulation_pick_target.is_some() {
            let hovered = self
                .graphics
                .as_ref()
                .and_then(|graphics| graphics.pick_triangulation_at_cursor(&self.triangulations, &self.editor.hidden_handles, &self.editor.frozen_handles))
                .and_then(|(entity, _)| match entity {
                    SceneEntityId::Triangulation(id) => Some(id),
                    _ => None,
                });
            let mut next_handles = std::collections::HashSet::new();
            if let Some(id) = hovered {
                next_handles.insert(SceneEntityId::Triangulation(id));
            }
            let next_label = hovered.and_then(|id| {
                self.triangulations
                    .iter()
                    .find(|triangulation| triangulation.id == id)
                    .map(|triangulation| tr_format!(literal = "Surface | %name%", name = &triangulation.name))
            });
            if self.editor.tri_hover_handles != next_handles || self.editor.viewport_pick_hover_label != next_label {
                self.editor.tri_hover_handles = next_handles;
                self.editor.viewport_pick_hover_label = next_label;
                self.invalidate_geometry();
            }
            return;
        }

        if !self.editor.drill_pattern_awaiting_shape_pick {
            return;
        }

        let raw_hover = self.graphics.as_ref().and_then(|graphics| {
            graphics
                .pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, &self.editor.frozen_handles, self.editor.xray_enabled)
                .map(|(entity, _)| entity)
        });
        let active_object_ids = self.active_project_object_ids();
        let pattern_picker = self.editor.drill_pattern_awaiting_shape_pick;
        let (next_highlight, next_label) = match raw_hover {
            Some(SceneEntityId::Object(id)) if !pattern_picker || active_object_ids.contains(&id) => match self.scene_document.get_object(id) {
                Some(object) if if pattern_picker { is_drill_pattern_boundary(object) } else { object.encloses_area() } => {
                    let layer = self.scene_document.layer(object.layer()).map(|layer| layer.name.as_str()).unwrap_or("?");
                    match object {
                        Object::Circle { radius, .. } => (
                            Some(id),
                            Some(tr_format!(
                                literal = "Circle | Layer: %layer% | radius %radius%",
                                layer = layer,
                                radius = format!("{radius:.3}")
                            )),
                        ),
                        _ => {
                            let count = object.string_geometry().map_or(0, |(verts, _)| verts.len());
                            (
                                Some(id),
                                Some(tr_format!(literal = "Polyline | Layer: %layer% | %count% vertices", layer = layer, count = count)),
                            )
                        }
                    }
                }
                _ => (None, Some(tr!(literal = "Not selectable | Choose a closed polyline"))),
            },
            Some(_) => (None, Some(tr!(literal = "Not selectable | Choose a closed polyline"))),
            None => (None, None),
        };
        if self.editor.tool_highlight_id != next_highlight || self.editor.viewport_pick_hover_label != next_label {
            self.editor.tool_highlight_id = next_highlight;
            self.editor.viewport_pick_hover_label = next_label;
            self.invalidate_geometry();
        }
    }

    fn apply_triangulation_field_pick(&mut self, target: TriangulationPickTarget, id: crate::model::triangulation::TriangulationId, name: &str) {
        match target {
            TriangulationPickTarget::TrimTopology => {
                self.editor.tri_cut_surface_reference_id = Some(id);
                if self.editor.tri_cut_surface_target_id == Some(id) {
                    self.editor.tri_cut_surface_target_id = None;
                }
            }
            TriangulationPickTarget::TrimSurface => {
                self.editor.tri_cut_surface_target_id = Some(id);
                if self.editor.tri_cut_surface_reference_id == Some(id) {
                    self.editor.tri_cut_surface_reference_id = None;
                }
                update_auto_derived_name(
                    &mut self.editor.tri_cut_surface_name_input,
                    self.editor.tri_cut_surface_name_auto,
                    name,
                    &tr!(literal = "Trimmed"),
                );
            }
            TriangulationPickTarget::CutPitTopology => {
                self.editor.tri_cut_pitshell_topology_id = Some(id);
                if self.editor.tri_cut_pitshell_pitshell_id == Some(id) {
                    self.editor.tri_cut_pitshell_pitshell_id = None;
                }
                update_auto_derived_name(
                    &mut self.editor.tri_cut_pitshell_name_input,
                    self.editor.tri_cut_pitshell_name_auto,
                    name,
                    &tr!(literal = "Cut"),
                );
            }
            TriangulationPickTarget::CutPitShell => {
                self.editor.tri_cut_pitshell_pitshell_id = Some(id);
                if self.editor.tri_cut_pitshell_topology_id == Some(id) {
                    self.editor.tri_cut_pitshell_topology_id = None;
                }
            }
            TriangulationPickTarget::IncludeTopology => {
                self.editor.tri_include_solid_topology_id = Some(id);
                if self.editor.tri_include_solid_shape_id == Some(id) {
                    self.editor.tri_include_solid_shape_id = None;
                }
                update_auto_derived_name(
                    &mut self.editor.tri_include_solid_name_input,
                    self.editor.tri_include_solid_name_auto,
                    name,
                    &tr!(literal = "With Shell"),
                );
            }
            TriangulationPickTarget::IncludeShape => {
                self.editor.tri_include_solid_shape_id = Some(id);
                if self.editor.tri_include_solid_topology_id == Some(id) {
                    self.editor.tri_include_solid_topology_id = None;
                }
            }
        }
        self.editor.triangulation_pick_target = None;
        self.editor.viewport_pick_hover_label = None;
        self.editor.tri_hover_handles.clear();
        self.invalidate_geometry();
    }

    pub(crate) fn finish_box_selection(&mut self) {
        let (Some(start), Some(end)) = (self.editor.selection_box_start_px.take(), self.editor.selection_box_current_px.take()) else {
            self.pending_selection_click = None;
            return;
        };
        let dragged = (end.0 - start.0).abs().max((end.1 - start.1).abs()) >= 3.0;
        let pending_selection_click = self.pending_selection_click.take();

        if self.editor.active_tool == ActiveTool::DrapeToTopology {
            self.finish_drape_selection(start, end, dragged);
            return;
        }

        // Delete Points tool: box drag selects point objects and confirms deletion; single click
        // deletes the point (polyline vertex or point object) at the cursor. Polylines and other
        // elements are deliberately never picked, so the tool can't delete a whole element.
        if self.editor.active_tool == crate::ui::state::ActiveTool::DeletePoints {
            if dragged {
                let cross_select = end.0 > start.0;
                let enclosed = self
                    .graphics
                    .as_ref()
                    .map(|g| {
                        if cross_select {
                            g.entities_touching_screen_rect(start, end, &self.editor.frozen_handles)
                        } else {
                            g.entities_in_screen_rect(start, end, &self.editor.frozen_handles)
                        }
                    })
                    .unwrap_or_default();
                let active_object_ids = self.active_project_object_ids();
                self.editor.selected_handles.clear();
                for handle in enclosed {
                    if let SceneEntityId::Object(id) = handle
                        && active_object_ids.contains(&id)
                        && matches!(self.scene_document.get_object(id), Some(Object::Point { .. }))
                    {
                        self.editor.selected_handles.insert(handle);
                    }
                }
                // Always invalidate geometry so deselected objects lose their highlight color.
                self.invalidate_geometry();
                if !self.editor.selected_handles.is_empty() {
                    self.editor.delete_confirm_open = true;
                }
            } else {
                self.delete_at_cursor();
            }
            return;
        }

        if !dragged {
            if self.editor.active_tool == ActiveTool::None && self.select_tie_at_cursor() {
                return;
            }
            if let Some(pick) = pending_selection_click {
                let world = pick.world;
                let handle = pick.entity;
                // A click takes the one hole the cursor was over, in every
                // workspace. Selecting a dataset whole is the explorer's job.
                let hole = pick.hole;

                // Selecting an object may retarget the active project, but never the
                // active layer: that is owned solely by the toolbar layer selector.
                if let SceneEntityId::Object(object_id) = handle {
                    self.activate_project_for_object(object_id);
                }
                // Clicking what is already selected takes it back out of the
                // selection, so any scene entity can be dropped without
                // going for empty space.
                let already_selected = match hole {
                    Some(hole) => self.editor.selected_drill_holes.contains(&hole),
                    None => self.editor.selected_handles.contains(&handle),
                };
                let selection_mode = if self.modifiers.shift_key() {
                    SelectionMode::Toggle
                } else if self.modifiers.control_key() {
                    SelectionMode::Add
                } else if already_selected {
                    SelectionMode::Toggle
                } else {
                    SelectionMode::Replace
                };
                match hole {
                    Some(hole) => self.editor.on_drill_hole_pick(hole, world, selection_mode),
                    None => self.editor.on_canvas_pick(handle, world, selection_mode),
                }
                // A click that dropped its hit must not park the panel on it.
                let still_selected = match hole {
                    Some(hole) => self.editor.selected_drill_holes.contains(&hole),
                    None => self.editor.selected_handles.contains(&handle),
                };
                if still_selected {
                    self.editor.show_picked_hole(hole);
                }
                // A drape has no geometry of its own - it is painted onto the
                // surface - so the click that lands on the surface lands on
                // both. Following what the surface ended up doing covers every
                // selection mode at once: the raster joins a surface that was
                // just selected and leaves one that was just dropped.
                if let SceneEntityId::Triangulation(id) = handle
                    && let Some(draped) = self.triangulations.iter().find(|item| item.id == id).and_then(|item| item.raster_texture)
                {
                    let draped = SceneEntityId::Raster(draped);
                    if self.editor.selected_handles.contains(&handle) {
                        self.editor.selected_handles.insert(draped);
                    } else {
                        self.editor.selected_handles.remove(&draped);
                    }
                }
                self.active_triangulation = match handle {
                    SceneEntityId::Triangulation(id) if self.editor.selected_handles.contains(&handle) => Some(id),
                    _ => None,
                };
                self.invalidate_geometry();
                return;
            }

            let preserve = self.modifiers.shift_key() || self.modifiers.control_key();
            if !preserve {
                self.editor.selected_handles.clear();
                self.editor.selected_drill_holes.clear();
                self.editor.selected_tie_ins.clear();
                self.active_triangulation = None;
                if self.editor.active_tool == crate::ui::state::ActiveTool::Move {
                    self.editor.move_vertex_target = None;
                }
            }
            self.invalidate_geometry();
            return;
        }

        // Box selection: left-to-right (end.x > start.x) = cross select (any vertex
        // inside box); right-to-left (end.x < start.x) = window select (all vertices inside).
        let cross_select = end.0 > start.0;
        // Drill & Blast gives connectors first refusal on the marquee. If no
        // tie-in is taken, it falls back to holes one at a time; Survey's
        // marquee takes point clouds, and production's the design geometry
        // over the same ground.
        if self.editor.active_workspace == Workspace::DrillAndBlast {
            self.finish_blast_box_selection(start, end, cross_select);
            return;
        }
        // Move Design marquees only what it can move, the same as its clicks
        // do - see `tool_accepts_pick`.
        let objects_only = self.editor.active_tool == ActiveTool::Move;
        let point_clouds_only = self.editor.active_workspace == Workspace::Survey && !objects_only;
        let mut enclosed = self
            .graphics
            .as_ref()
            .map(|graphics| {
                if point_clouds_only {
                    graphics.point_clouds_in_screen_rect(start, end, cross_select, &self.editor.hidden_handles, &self.editor.frozen_handles)
                } else if cross_select {
                    graphics.entities_touching_screen_rect(start, end, &self.editor.frozen_handles)
                } else {
                    graphics.entities_in_screen_rect(start, end, &self.editor.frozen_handles)
                }
            })
            .unwrap_or_default();
        let active_object_ids = self.active_project_object_ids();
        enclosed.retain(|handle| match handle {
            SceneEntityId::Object(object_id) => active_object_ids.contains(object_id),
            SceneEntityId::Triangulation(_) => !objects_only,
            SceneEntityId::BlockModel(_) => !objects_only,
            SceneEntityId::DrillHole(_) => !objects_only,
            SceneEntityId::PointCloud(_) => !objects_only,
            // Nothing to enclose: a raster is painted onto a surface rather
            // than occupying the scene, so a marquee never produces one.
            SceneEntityId::Raster(_) => false,
        });
        // Holes ride the same box, taken by their collars: they are not
        // rendered geometry the picker walks, so `enclosed` never holds one,
        // and Move takes none because it marquees only what it can move.
        let holes = self
            .graphics
            .as_ref()
            .filter(|_| !objects_only)
            .map(|graphics| graphics.drill_hole_collars_in_screen_rect(&self.drill_holes, start, end, &self.editor.hidden_handles, &self.editor.frozen_handles))
            .unwrap_or_default();
        if self.modifiers.shift_key() {
            for handle in enclosed {
                if !self.editor.selected_handles.remove(&handle) {
                    self.editor.selected_handles.insert(handle);
                }
            }
            for hole in holes {
                if !self.editor.selected_drill_holes.remove(&hole) {
                    self.editor.selected_drill_holes.insert(hole);
                }
            }
        } else {
            if !self.modifiers.control_key() {
                self.editor.selected_handles.clear();
                // Cleared with the handles, or a box over empty ground would
                // leave the previous box's holes selected.
                self.editor.selected_drill_holes.clear();
            }
            self.editor.selected_handles.extend(enclosed);
            self.editor.selected_drill_holes.extend(holes);
        }
        if self.editor.active_tool == crate::ui::state::ActiveTool::Move {
            self.editor.move_vertex_target = None;
        }
        self.invalidate_geometry();
    }

    /// The Drill & Blast marquee: select tie-ins exclusively when the box
    /// takes any; otherwise select its holes one at a time rather than as the
    /// datasets they came from.
    fn finish_blast_box_selection(&mut self, start: (f32, f32), end: (f32, f32), cross_select: bool) {
        let tie_ins = self
            .graphics
            .as_ref()
            .filter(|_| !self.editor.active_tool.acts_on_collars())
            .map(|graphics| graphics.tie_ins_in_screen_rect(&self.drill_holes, start, end, cross_select, &self.editor.hidden_handles, &self.editor.frozen_handles))
            .unwrap_or_default();
        if !tie_ins.is_empty() {
            if self.modifiers.shift_key() {
                for tie in tie_ins {
                    if !self.editor.selected_tie_ins.remove(&tie) {
                        self.editor.selected_tie_ins.insert(tie);
                    }
                }
            } else {
                if !self.modifiers.control_key() {
                    self.editor.selected_handles.clear();
                    self.editor.selected_drill_holes.clear();
                    self.editor.selected_tie_ins.clear();
                }
                self.editor.selected_tie_ins.extend(tie_ins);
            }
            self.invalidate_geometry();
            return;
        }

        let enclosed = self
            .graphics
            .as_ref()
            .map(|graphics| graphics.drill_holes_in_screen_rect(&self.drill_holes, start, end, cross_select, &self.editor.hidden_handles, &self.editor.frozen_handles))
            .unwrap_or_default();
        if self.modifiers.shift_key() {
            for hole in enclosed {
                if !self.editor.selected_drill_holes.remove(&hole) {
                    self.editor.selected_drill_holes.insert(hole);
                }
            }
        } else {
            if !self.modifiers.control_key() {
                self.editor.selected_handles.clear();
                self.editor.selected_drill_holes.clear();
                self.editor.selected_tie_ins.clear();
            }
            self.editor.selected_drill_holes.extend(enclosed);
        }
        self.invalidate_geometry();
    }

    fn finish_drape_selection(&mut self, start: (f32, f32), end: (f32, f32), dragged: bool) {
        let mut candidates = if dragged {
            let cross_select = end.0 > start.0;
            self.graphics
                .as_ref()
                .map(|graphics| {
                    if cross_select {
                        graphics.entities_touching_screen_rect(start, end, &self.editor.frozen_handles)
                    } else {
                        graphics.entities_in_screen_rect(start, end, &self.editor.frozen_handles)
                    }
                })
                .unwrap_or_default()
        } else {
            self.graphics
                .as_ref()
                .and_then(|graphics| match self.editor.drape_phase {
                    DrapePhase::Designs => graphics
                        .pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, &self.editor.frozen_handles, self.editor.xray_enabled)
                        .map(|(handle, _)| vec![handle]),
                    DrapePhase::Topologies => graphics
                        .pick_triangulation_at_cursor(&self.triangulations, &self.editor.hidden_handles, &self.editor.frozen_handles)
                        .map(|(handle, _)| vec![handle]),
                })
                .unwrap_or_default()
        };

        let active_object_ids = self.active_project_object_ids();
        // Drape selects only what it can drape, the same rule its apply step
        // uses - see `tool_accepts_pick`. Offering a circle and then dropping
        // it at apply time would let the user build a selection the tool was
        // never going to honour.
        candidates.retain(|handle| match (self.editor.drape_phase, handle) {
            (DrapePhase::Designs, SceneEntityId::Object(id)) => {
                if !active_object_ids.contains(id) {
                    return false;
                }
                self.active_document().get_object(*id).is_some_and(is_drapeable)
            }
            (DrapePhase::Topologies, SceneEntityId::Triangulation(_)) => true,
            _ => false,
        });

        if candidates.is_empty() {
            if !self.modifiers.shift_key() && !self.modifiers.control_key() {
                self.editor.selected_handles.clear();
            }
            self.invalidate_geometry();
            return;
        }

        if self.modifiers.shift_key() {
            for handle in candidates {
                if !self.editor.selected_handles.remove(&handle) {
                    self.editor.selected_handles.insert(handle);
                }
            }
        } else if self.modifiers.control_key() {
            self.editor.selected_handles.extend(candidates);
        } else if self.editor.drape_phase == DrapePhase::Topologies && !dragged && candidates.len() == 1 && self.editor.selected_handles.contains(&candidates[0]) {
            self.editor.selected_handles.remove(&candidates[0]);
        } else {
            self.editor.selected_handles.clear();
            self.editor.selected_handles.extend(candidates);
        }
        self.invalidate_geometry();
    }

    pub(crate) fn drag_to_cursor(&mut self) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        let (object_id, plane_z, last) = (drag.object_id, drag.plane_z, drag.last_world);
        let Some(world) = self.graphics.as_ref().and_then(|g| g.cursor_world(plane_z)) else {
            return;
        };
        let delta = world - last;
        if delta.length_squared() <= 0.0 {
            return;
        }
        if let Some(doc) = self.workspace.active_document_mut() {
            doc.translate_object(object_id, delta);
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.last_world = world;
            drag.moved = true;
        }
        self.invalidate_geometry();
    }

    pub(crate) fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if drag.moved
            && let Some(after) = self.active_document().get_object(drag.object_id).cloned()
        {
            self.record_applied_edit(Command::Replace { before: drag.before, after });
        }
    }

    /// Whether the active tool will take what the cursor landed on.
    ///
    /// The gizmo tools take only what they can act on - Move Design the
    /// document's own objects, Move Collar and Rotate Collar the holes of a
    /// drillhole dataset - so a pick they have no use for is treated as a
    /// click on nothing rather than selecting something the gizmo would then
    /// stand uselessly over. A tool that selects for its own reasons, and
    /// plain picking, take anything.
    fn tool_accepts_pick(&self, pick: &crate::rendering::graphics::camera::ScenePick) -> bool {
        match self.editor.active_tool {
            ActiveTool::Move => matches!(pick.entity, SceneEntityId::Object(_)),
            ActiveTool::MoveCollar | ActiveTool::RotateCollar => pick.hole.is_some(),
            _ => true,
        }
    }

    pub(crate) fn active_project_object_ids(&self) -> std::collections::HashSet<crate::model::ObjectId> {
        self.workspace
            .active_project()
            .map(|project| project.project.document.objects().iter().map(Object::id).collect())
            .unwrap_or_default()
    }
}

/// Standardised name for a triangulation derived from another: the source's
/// stem verbatim (its own casing is left alone), then a Title-Case operation
/// word - e.g. `"minedesign.dxf"` with `"Clipped"` becomes `"minedesign Clipped"`.
pub(crate) fn derived_triangulation_name(name: &str, suffix: &str) -> String {
    let stem = std::path::Path::new(name).file_stem().and_then(|value| value.to_str()).unwrap_or(name).trim();
    if stem.is_empty() { name.to_owned() } else { format!("{stem} {suffix}") }
}

fn update_auto_derived_name(output: &mut String, is_auto: bool, source: &str, suffix: &str) {
    if is_auto {
        *output = derived_triangulation_name(source, suffix);
    }
}

pub(crate) fn is_triangulation_polyline(obj: &Object) -> bool {
    matches!(obj, Object::Polyline { .. }) && obj.tessellated_path().is_some_and(|(points, closed)| points.len() >= if closed { 3 } else { 2 })
}

fn is_drill_pattern_boundary(object: &Object) -> bool {
    object.encloses_area()
}

/// Whether the Drape tool will take this object.
///
/// A circle laid over topography is no longer a circle - draping it would have
/// to hand back a polyline - so the tool does not offer one.
fn is_drapeable(object: &Object) -> bool {
    !matches!(object, Object::Circle { .. })
}
