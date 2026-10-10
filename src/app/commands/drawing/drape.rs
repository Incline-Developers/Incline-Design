use std::collections::HashSet;

use glam::{DVec2, DVec3};

use crate::{
    app::App,
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
    userspace_warn,
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
                let loaded_ids: HashSet<_> = self.triangulations.iter().map(|topology| topology.id).collect();
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
        self.editor.drape_phase = DrapePhase::Designs;
        self.editor.drape_object_ids.clear();
        self.editor.selection_box_start_px = None;
        self.editor.selection_box_current_px = None;
        self.editor.selected_handles.clear();
        self.editor.active_tool = ActiveTool::None;
        self.invalidate_geometry();
    }

    fn apply_drape_to_topologies(&mut self, topology_ids: &[TriangulationId]) {
        let selected_ids: HashSet<_> = topology_ids.iter().copied().collect();
        let surfaces: Vec<_> = self.triangulations.iter().filter(|topology| selected_ids.contains(&topology.id)).collect();
        if surfaces.is_empty() {
            userspace_warn!("{}", tr!("cmd-drape-selected-topologies-no-longer-loaded"));
            return;
        }

        let mut intersected_vertices = 0usize;
        let mut changed_vertices = 0usize;
        let along_triangles = self.editor.drape_along_triangles;
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
        crossings.retain(|t| *t > 1.0e-9 && *t < 1.0 - 1.0e-9);
        crossings.sort_by(f64::total_cmp);
        crossings.dedup_by(|later, earlier| (*later - *earlier).abs() <= 1.0e-9);
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
