use std::collections::HashSet;

use glam::{DVec2, DVec3};

use crate::{
    app::{App, jobs::JobKey},
    i18n::tr,
    logging::CommandReportSpec,
    model::{
        Command, Object, PolyVertex, SceneEntityId,
        formats::mesh_data::Triangulation,
        kernel::{self, SegSeg},
        spatial::TriangleBvh,
        triangulation::{OpenTriangulation, TriangulationId},
    },
    ui::state::{ActiveTool, DrapePhase},
    userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    /// Advance from design selection to topology selection, or apply the drape
    /// when both selection steps are complete.
    pub(crate) fn confirm_drape_selection(&mut self) {
        if self.editor.active_tool != ActiveTool::DrapeToTopology {
            return;
        }

        match self.editor.drape_phase {
            DrapePhase::Designs => {
                // Whatever the tool cannot drape was never selectable in the
                // first place - see `is_drapeable` - so this takes the
                // selection as it stands.
                let active_object_ids = self.active_project_object_ids();
                let object_ids: Vec<_> = self
                    .editor
                    .selected_handles
                    .iter()
                    .filter_map(|handle| match handle {
                        SceneEntityId::Object(id) if active_object_ids.contains(id) => Some(*id),
                        _ => None,
                    })
                    .collect();
                if object_ids.is_empty() {
                    userspace_warn!("{}", tr!("cmd-drape-select-one-more-design-objects"));
                    return;
                }

                self.editor.drape_object_ids = object_ids;
                self.editor.drape_phase = DrapePhase::Topologies;
                self.editor.selected_handles.clear();
                self.invalidate_geometry();
            }
            DrapePhase::Topologies => {
                let loaded_ids: HashSet<_> = self.triangulations.iter().filter(|topology| topology.state.loaded).map(|topology| topology.id).collect();
                let topology_ids: Vec<_> = self
                    .editor
                    .selected_handles
                    .iter()
                    .filter_map(|handle| match handle {
                        SceneEntityId::Triangulation(id) if loaded_ids.contains(id) => Some(*id),
                        _ => None,
                    })
                    .collect();
                if topology_ids.is_empty() {
                    userspace_warn!("{}", tr!("cmd-drape-select-one-more-topologies-drape"));
                    return;
                }
                self.apply_drape_to_topologies(&topology_ids);
            }
        }
    }

    /// Arm Drape so that each string also follows the surface between its
    /// vertices; a Drape already armed keeps its selection step.
    pub(crate) fn arm_drape_along_triangles(&mut self) {
        if self.editor.active_tool != ActiveTool::DrapeToTopology {
            self.set_active_tool_from_toolbar(ActiveTool::DrapeToTopology);
        }
        if self.editor.active_tool == ActiveTool::DrapeToTopology {
            self.editor.drape_along_triangles = true;
        }
    }

    pub(crate) fn cancel_drape(&mut self) {
        self.editor.drape_along_triangles = false;
        self.drape_waiting_on.clear();
        self.editor.drape_phase = DrapePhase::Designs;
        self.editor.drape_object_ids.clear();
        self.editor.selection_box_start_px = None;
        self.editor.selection_box_current_px = None;
        self.editor.selected_handles.clear();
        self.editor.active_tool = ActiveTool::None;
        self.invalidate_geometry();
    }

    /// A big surface has no tree until asked, and building one here would
    /// hold the frame. Build the chosen surfaces' trees in the background, all
    /// but those another job or a query is already building, and wait on all
    /// of them to drape. The build's job resumes the drape when it lands; a
    /// tree built elsewhere is looked for by `poll_drape_waiting_elsewhere`.
    /// True while it waits.
    fn index_surfaces_for_drape(&mut self, selected_ids: &HashSet<TriangulationId>) -> bool {
        let unbuilt: Vec<_> = self
            .triangulations
            .iter()
            .filter(|topology| selected_ids.contains(&topology.id) && !topology.spatial.is_built())
            .map(|topology| (topology.id, (*topology.spatial).clone()))
            .collect();
        if unbuilt.is_empty() {
            return false;
        }
        let unbuilt_count = unbuilt.len();
        let claimed: Vec<_> = unbuilt
            .into_iter()
            .filter_map(|(id, spatial)| spatial.claim_build().map(|claim| (id, spatial, claim)))
            .collect();
        self.drape_waits_elsewhere = claimed.len() < unbuilt_count;
        if !claimed.is_empty() {
            let ids: Vec<_> = claimed.iter().map(|(id, _, _)| *id).collect();
            let keys: Vec<_> = ids.iter().map(|id| JobKey::Triangulation(*id)).collect();
            self.spawn_job(
                tr!("cmd-drape-indexing-surfaces"),
                keys,
                move |_cancel| {
                    for (_, spatial, claim) in claimed {
                        spatial.ensure_built();
                        drop(claim);
                    }
                    Ok(())
                },
                move |app, result| match result {
                    Ok(()) => app.resume_waiting_drape(),
                    Err(error) => {
                        userspace_warn!("{error}");
                        if app.drape_waiting_on.iter().any(|id| ids.contains(id)) {
                            app.drape_waiting_on.clear();
                        }
                    }
                },
            );
        }
        let mut waiting: Vec<_> = selected_ids.iter().copied().collect();
        waiting.sort_by_key(|id| id.0);
        self.drape_waiting_on = waiting;
        // A tree built elsewhere wakes nothing when it lands: one frame to
        // start looking.
        if self.drape_waits_elsewhere {
            self.redraw_requested = true;
        }
        true
    }

    /// A Drape Following Triangles waiting on its surfaces' trees drapes once
    /// they are all built, and is dropped, with a line in the console, if its
    /// tool or the surfaces chosen changed meanwhile. Called when the drape's
    /// build job lands, and by [`App::poll_drape_waiting_elsewhere`].
    pub(crate) fn resume_waiting_drape(&mut self) {
        if self.drape_waiting_on.is_empty() {
            return;
        }
        let chosen = std::mem::take(&mut self.drape_waiting_on);
        let loaded: HashSet<_> = self.triangulations.iter().filter(|topology| topology.state.loaded).map(|topology| topology.id).collect();
        // An unloaded surface in the selection was never one the drape took.
        let selected = self.editor.selected_handles.iter().filter(|handle| match handle {
            SceneEntityId::Triangulation(id) => loaded.contains(id),
            _ => true,
        });
        let still_chosen = self.editor.active_tool == ActiveTool::DrapeToTopology
            && self.editor.drape_phase == DrapePhase::Topologies
            && self.editor.drape_along_triangles
            && chosen.iter().all(|id| loaded.contains(id))
            && same_surfaces_selected(selected, &chosen);
        if !still_chosen {
            userspace_log!("{}", tr!("cmd-drape-dropped-while-indexing"));
            return;
        }
        self.apply_drape_to_topologies(&chosen);
    }

    /// Once a frame: the one wait kept beside the job queue. A tree this drape
    /// builds itself resumes it from the job. Leo's background build for a
    /// query has no job whose apply could, so while the drape waits on a tree
    /// another builder claimed, or on one no job builds any more (its own job
    /// cancelled, as unloading or deleting the surface does), this looks again
    /// each frame, as it does once the tool is put down without the drape
    /// being cancelled. A tree no job is building shows the wait in the status
    /// bar.
    pub(crate) fn poll_drape_waiting_elsewhere(&mut self) {
        if self.drape_waiting_on.is_empty() && self.drape_wait_shown.is_none() {
            return;
        }
        // A waiting surface gone or unloaded, or unbuilt with no job building
        // it; or no job left on any, as after a cancelled job built its tree.
        // A job still running on a waiting surface leaves the rest to it.
        let has_job = |id: &TriangulationId| self.job_pending(&JobKey::Triangulation(*id));
        let unworked = self.drape_waiting_on.iter().any(|id| {
            self.triangulations
                .iter()
                .find(|topology| topology.id == *id)
                .is_none_or(|topology| !topology.state.loaded || (!has_job(id) && !topology.spatial.is_built()))
        });
        let stalled = unworked || !self.drape_waiting_on.iter().any(has_job);
        // Put down by a project switch, which does not cancel the drape.
        let abandoned = !self.drape_waiting_on.is_empty()
            && (self.editor.active_tool != ActiveTool::DrapeToTopology || self.editor.drape_phase != DrapePhase::Topologies || !self.editor.drape_along_triangles);
        if self.drape_waits_elsewhere || stalled || abandoned {
            self.resume_waiting_drape();
        }
        let quiet = self
            .triangulations
            .iter()
            .any(|topology| self.drape_waiting_on.contains(&topology.id) && !topology.spatial.is_built() && !self.job_pending(&JobKey::Triangulation(topology.id)));
        let shown = self.drape_wait_shown.take();
        self.drape_wait_shown = self.show_build_wait(shown, quiet, || tr!("cmd-drape-indexing-surfaces"));
    }

    fn apply_drape_to_topologies(&mut self, topology_ids: &[TriangulationId]) {
        // This confirm stands in for any earlier one still waiting; it waits
        // again below if a tree is still missing.
        self.drape_waiting_on.clear();
        let selected_ids: HashSet<_> = topology_ids.iter().copied().collect();
        if !self.triangulations.iter().any(|topology| selected_ids.contains(&topology.id)) {
            userspace_warn!("{}", tr!("cmd-drape-selected-topologies-no-longer-loaded"));
            return;
        }

        let along_triangles = self.editor.drape_along_triangles;
        if along_triangles && self.index_surfaces_for_drape(&selected_ids) {
            return;
        }
        let surfaces: Vec<_> = self.triangulations.iter().filter(|topology| selected_ids.contains(&topology.id)).collect();
        let mut intersected_vertices = 0usize;
        let mut changed_vertices = 0usize;
        let meshes: Vec<_> = surfaces.iter().map(|surface| (&*surface.mesh, &*surface.spatial)).collect();
        let replacements: Vec<_> = self
            .editor
            .drape_object_ids
            .iter()
            .filter_map(|id| {
                let before = self.active_document().get_object(*id)?.clone();
                let mut after = before.clone();
                if along_triangles && let Object::Polyline { verts, closed, .. } = &mut after {
                    *verts = with_triangle_edge_vertices(verts, *closed, &meshes);
                }
                let (intersected, changed) = drape_object(&mut after, &surfaces);
                intersected_vertices += intersected;
                changed_vertices += changed;
                (before != after).then_some(Command::Replace { before, after })
            })
            .collect();
        let changed_objects = replacements.len();

        if !replacements.is_empty() {
            self.execute_edit(Command::Batch(replacements));
        }

        let selected_objects = self.editor.drape_object_ids.clone();
        self.editor.selected_handles = selected_objects.iter().copied().map(SceneEntityId::Object).collect();
        self.editor.drape_object_ids.clear();
        self.editor.drape_phase = DrapePhase::Designs;
        self.editor.active_tool = ActiveTool::None;
        self.invalidate_geometry();

        if intersected_vertices == 0 {
            userspace_warn!("{}", tr!("cmd-drape-no-intersections"));
        } else {
            crate::logging::report_completed_action(
                CommandReportSpec::new(
                    tr!("common-drape-topology"),
                    tr!(
                        "cmd-drape-objects-changed-object-s-changed",
                        objects = changed_objects.to_string(),
                        changed = changed_vertices.to_string(),
                        intersected = intersected_vertices.to_string()
                    ),
                ),
                tr!(
                    "cmd-drape-draped-intersected-vertices-changed",
                    intersected = intersected_vertices.to_string(),
                    changed = changed_vertices.to_string()
                ),
            );
        }
    }
}

/// `verts` with a vertex added wherever a straight span crosses a triangle
/// edge of `surfaces` in plan, so a drape lays the string on the surface
/// everywhere rather than only at its vertices. Bulged spans are left as
/// they are. The new vertices sit on the span; the drape sets their height.
fn with_triangle_edge_vertices(verts: &[PolyVertex], closed: bool, surfaces: &[(&Triangulation, &TriangleBvh)]) -> Vec<PolyVertex> {
    let count = verts.len();
    if count < 2 {
        return verts.to_vec();
    }
    let spans = if closed { count } else { count - 1 };
    let mut result = Vec::with_capacity(count);
    let mut stack = Vec::new();
    for index in 0..spans {
        let start = verts[index];
        let end = verts[(index + 1) % count];
        result.push(start);
        if start.bulge.abs() > f64::EPSILON {
            continue;
        }
        let (a, b) = (start.pos.truncate(), end.pos.truncate());
        let mut crossings = Vec::new();
        for (mesh, spatial) in surfaces {
            let mesh_vertices = mesh.vertices();
            let corner = |vertex: usize| DVec2::new(mesh_vertices[vertex].x, mesh_vertices[vertex].y);
            spatial.for_each_xy_bounds_candidate_index_with_stack(a.min(b), a.max(b), &mut stack, |triangle| {
                let Some(face) = mesh.face(triangle) else {
                    return;
                };
                let [p, q, r] = face.indices_zero_based();
                for (c, d) in [(p, q), (q, r), (r, p)] {
                    if let SegSeg::Crossing { t, .. } | SegSeg::Touching { t, .. } = kernel::segment_segment(a, b, corner(c), corner(d)) {
                        crossings.push(t);
                    }
                }
            });
        }
        // Kept apart by the ground distance two points count as one.
        let length = (b - a).length();
        crossings.retain(|t| *t * length > kernel::XY_TOL && (1.0 - *t) * length > kernel::XY_TOL);
        crossings.sort_by(f64::total_cmp);
        crossings.dedup_by(|later, earlier| (*later - *earlier).abs() * length <= kernel::XY_TOL);
        result.extend(crossings.into_iter().map(|t| PolyVertex {
            pos: start.pos.lerp(end.pos, t),
            bulge: 0.0,
        }));
    }
    if !closed {
        result.push(verts[count - 1]);
    }
    result
}

/// Move every stored design vertex vertically to the uppermost selected
/// topology at the same XY coordinate. A vertex outside all selected topology
/// footprints is deliberately left untouched.
fn drape_object(object: &mut Object, surfaces: &[&OpenTriangulation]) -> (usize, usize) {
    let mut intersected = 0usize;
    let mut changed = 0usize;
    let mut drape_point = |point: &mut DVec3| {
        let Some(z) = uppermost_topology_z(*point, surfaces) else {
            return;
        };
        intersected += 1;
        if point.z != z {
            point.z = z;
            changed += 1;
        }
    };

    match object {
        Object::Point { pos, .. } | Object::Text { pos, .. } => drape_point(pos),
        // Refused before it gets here; a circle has no vertices to drape and
        // draping its centre alone would move the shape, not lay it down.
        Object::Circle { .. } => {}
        Object::Polyline { verts, .. } => {
            for vertex in verts {
                drape_point(&mut vertex.pos);
            }
        }
    }
    (intersected, changed)
}

fn uppermost_topology_z(point: DVec3, surfaces: &[&OpenTriangulation]) -> Option<f64> {
    surfaces
        .iter()
        .filter_map(|surface| {
            let bounds = surface.mesh.bounds();
            if point.x < bounds.min.x || point.x > bounds.max.x || point.y < bounds.min.y || point.y > bounds.max.y {
                return None;
            }
            let z_span = (bounds.max.z - bounds.min.z).abs();
            let origin_z = bounds.max.z + (z_span * 1.0e-6).max(1.0);
            surface.spatial.ray_hit(&surface.mesh, DVec3::new(point.x, point.y, origin_z), -DVec3::Z).map(|hit| hit.z)
        })
        .max_by(f64::total_cmp)
}

/// Whether the selected surfaces are exactly `chosen`: a surface added to the
/// selection while the trees build is not one the drape was asked for.
fn same_surfaces_selected<'h>(selected: impl IntoIterator<Item = &'h SceneEntityId>, chosen: &[TriangulationId]) -> bool {
    let selected: HashSet<_> = selected
        .into_iter()
        .filter_map(|handle| match handle {
            SceneEntityId::Triangulation(id) => Some(*id),
            _ => None,
        })
        .collect();
    selected == chosen.iter().copied().collect()
}
