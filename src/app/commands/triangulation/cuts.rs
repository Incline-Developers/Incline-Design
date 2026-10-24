use super::*;
use crate::model::geometry::{clip_polyline_by_xy_edge, signed_area_xy, triangle_xy_area};

impl<'a> App<'a> {
    /// Cut the triangulation, clipping each triangle against the XY polyline boundary
    /// using Sutherland-Hodgman. Produces smooth edges instead of centroid-based jagged cuts.
    pub(crate) fn cut_triangulation_by_polyline(
        &mut self,
        tri_id: TriangulationId,
        polyline_id: ObjectId,
        mode: TriPolylineClipMode,
        name: String,
        unload_source: bool,
    ) -> Result<()> {
        let (mesh, tri_name) = {
            let tri = self
                .triangulations
                .iter()
                .find(|t| t.id == tri_id)
                .ok_or_else(|| anyhow::anyhow!("Triangulation not found"))?;
            (tri.mesh.clone(), tri.name.clone())
        };

        let poly_verts: Vec<glam::DVec3> = match self.scene_document.get_object(polyline_id).and_then(Object::closed_boundary) {
            Some(points) => points,
            None => anyhow::bail!("Selected object is not a closed polyline or circle"),
        };
        if poly_verts.len() < 3 {
            anyhow::bail!("Selected polyline has fewer than 3 boundary points");
        }

        // Run the clip + mesh/BVH build off the UI thread; the polyline and mesh
        // are snapshotted above so the worker never touches `self`.
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            // Clipping walks every face; the mesh and BVH build after it are
            // single calls, so they close out the bar.
            let (new_verts, new_faces) = clip_mesh_by_polyline_xy(&mesh, &poly_verts, mode, &progress.phase(0.0, 0.8));
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            if new_faces.is_empty() {
                let retained_region = match mode {
                    TriPolylineClipMode::KeepInside => "inside",
                    TriPolylineClipMode::KeepOutside => "outside",
                };
                anyhow::bail!("No surface geometry falls {retained_region} the selected polyline");
            }
            let generated = session::build_generated_triangulation(name, new_verts, new_faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)?;
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: crate::i18n::tr!("cmd-cuts-clipped-surface-name-polyline-mode", name = tri_name.to_string(), mode = mode.label().to_string()),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, unload_source.then_some(tri_id).as_slice());
        };
        self.spawn_job_reporting_progress(
            crate::i18n::tr!("cmd-cuts-clipping-surface-polyline"),
            vec![crate::app::jobs::JobKey::Triangulation(tri_id)],
            compute,
            apply,
        );
        Ok(())
    }

    /// Trim a topology to the region a pit shell does not excavate, so the two meshes meet at
    /// a seam that follows their true 3D contact line (where the shell crosses the terrain) -
    /// not a fixed design polyline. Topology under parts of the shell that float above the
    /// ground is kept. The pit shell mesh may be multi-valued in XY (walls, benches) or a
    /// watertight closed solid; its flat crest cap never forces removal on its own.
    pub(crate) fn cut_topology_by_pit_shell(&mut self, topology_id: TriangulationId, pit_shell_id: TriangulationId, name: String, unload_source: bool) -> Result<()> {
        if topology_id == pit_shell_id {
            anyhow::bail!("Topology and pit shell must be different triangulations");
        }

        let (topology_mesh, topology_name) = {
            let tri = self
                .triangulations
                .iter()
                .find(|t| t.id == topology_id)
                .ok_or_else(|| anyhow::anyhow!("Topology not found"))?;
            (tri.mesh.clone(), tri.name.clone())
        };
        let pit_shell_mesh = {
            let tri = self
                .triangulations
                .iter()
                .find(|t| t.id == pit_shell_id)
                .ok_or_else(|| anyhow::anyhow!("Pit shell not found"))?;
            tri.mesh.clone()
        };

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            // Three passes, each a single call over one of the meshes: the bar
            // steps between them on weights estimating their relative cost.
            let pit_shell = prepare_pit_shell_surface(&pit_shell_mesh)?;
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            progress.set_fraction(0.2);
            let envelope = build_pit_shell_lower_envelope(&pit_shell)?;
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            progress.set_fraction(0.5);
            let (new_verts, new_faces) = clip_topology_to_pit_shell(&topology_mesh, &envelope, &progress.phase(0.5, 0.9));
            if new_faces.is_empty() {
                anyhow::bail!("No topology geometry falls outside the pit shell");
            }
            let generated = session::build_generated_triangulation(name, new_verts, new_faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)?;
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: crate::i18n::tr!("cmd-cuts-cut-topology-name-pit-shell", name = topology_name.to_string()),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, unload_source.then_some(topology_id).as_slice());
        };
        self.spawn_job_reporting_progress(
            crate::i18n::tr!("cmd-cuts-cutting-topology-pit-shell"),
            vec![crate::app::jobs::JobKey::Triangulation(topology_id), crate::app::jobs::JobKey::Triangulation(pit_shell_id)],
            compute,
            apply,
        );
        Ok(())
    }

    /// Cut the triangulation to the Z band [z_min, z_max], clipping triangles
    /// that straddle the boundary planes.
    pub(crate) fn cut_triangulation_by_z(&mut self, tri_id: TriangulationId, z_min: f64, z_max: f64, name: String, unload_source: bool) -> Result<()> {
        if z_min >= z_max {
            anyhow::bail!("Z min must be less than Z max");
        }

        let (mesh, tri_name) = {
            let tri = self
                .triangulations
                .iter()
                .find(|t| t.id == tri_id)
                .ok_or_else(|| anyhow::anyhow!("Triangulation not found"))?;
            (tri.mesh.clone(), tri.name.clone())
        };

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            let verts_raw = mesh.vertices();
            let mut new_verts: Vec<mesh_data::Vertex> = Vec::new();
            let mut new_faces: Vec<[u32; 3]> = Vec::new();
            // One clip per face, so faces walked is an exact measure; the mesh
            // and BVH build after the loop close out the bar.
            let clip = progress.phase(0.0, 0.8);
            let face_count = mesh.face_count() as u64;

            for (index, face) in mesh.face_vertex_indices_iter().enumerate() {
                if index.is_multiple_of(65_536) {
                    if cancel.is_cancelled() {
                        anyhow::bail!("Cancelled");
                    }
                    clip.set_items(index as u64, face_count);
                }
                let raw = [verts_raw[face[0]], verts_raw[face[1]], verts_raw[face[2]]];
                for clipped in clip_triangle_z(raw, z_min, z_max) {
                    let base = new_verts.len() as u32;
                    new_verts.extend_from_slice(&clipped);
                    new_faces.push([base, base + 1, base + 2]);
                }
            }

            if new_faces.is_empty() {
                anyhow::bail!("No triangulation geometry lies within the specified Z range");
            }
            let generated = session::build_generated_triangulation(name, new_verts, new_faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)?;
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: crate::i18n::tr!(
                    "cmd-cuts-cut-triangulation-name-z-band",
                    name = tri_name.to_string(),
                    min = format!("{z_min:.3}"),
                    max = format!("{z_max:.3}")
                ),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, unload_source.then_some(tri_id).as_slice());
        };
        self.spawn_job_reporting_progress(
            crate::i18n::tr!("cmd-cuts-cutting-triangulation-z"),
            vec![crate::app::jobs::JobKey::Triangulation(tri_id)],
            compute,
            apply,
        );
        Ok(())
    }

    /// Vertically clip one triangulation against another topology. Only the XY
    /// overlap with the reference topology is emitted.
    pub(crate) fn cut_triangulation_by_surface(
        &mut self,
        target_id: TriangulationId,
        reference_id: TriangulationId,
        side: TriSurfaceCutSide,
        name: String,
        unload_source: bool,
    ) -> Result<()> {
        if target_id == reference_id {
            anyhow::bail!("Surface to trim and topology must be different triangulations");
        }

        let (target_mesh, target_name) = {
            let target = self
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == target_id)
                .ok_or_else(|| anyhow::anyhow!("Surface to trim not found"))?;
            (target.mesh.clone(), target.name.clone())
        };
        let (reference_mesh, reference_name) = {
            let reference = self
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == reference_id)
                .ok_or_else(|| anyhow::anyhow!("Reference topology not found"))?;
            (reference.mesh.clone(), reference.name.clone())
        };

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            let (new_vertices, new_faces) = clip_mesh_by_surface(&target_mesh, &reference_mesh, side, &progress.phase(0.0, 0.8))?;
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            if new_faces.is_empty() {
                let retained = side.retained_relation();
                anyhow::bail!("No surface geometry lies {retained} the topology within its XY coverage");
            }
            let generated = session::build_generated_triangulation(name, new_vertices, new_faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)?;
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: crate::i18n::tr!(
                    "cmd-cuts-trimmed-surface",
                    surface = target_name.to_string(),
                    topology = reference_name.to_string(),
                    mode = side.trim_label().to_string()
                ),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, unload_source.then_some(target_id).as_slice());
        };
        self.spawn_job_reporting_progress(
            crate::i18n::tr!("cmd-cuts-trimming-surface-topology"),
            vec![crate::app::jobs::JobKey::Triangulation(target_id), crate::app::jobs::JobKey::Triangulation(reference_id)],
            compute,
            apply,
        );
        Ok(())
    }
}

/// Clip a mesh against an XY polyline, retaining either its footprint or its complement.
///
/// The polyline is triangulated once with earcut, then each mesh triangle is clipped against
/// every overlapping polyline triangle using the in-house Sutherland–Hodgman + SAT path
/// (`clip_target_triangle_to_reference_xy`) shared with the pit-shell and include tools.
/// This avoids the `geo` boolean-op allocation storm on large meshes and lets the per-face
/// work run in parallel.
pub(super) fn clip_mesh_by_polyline_xy(
    mesh: &mesh_data::Triangulation,
    polyline: &[glam::DVec3],
    mode: TriPolylineClipMode,
    progress: &crate::model::progress::Phase,
) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    use rayon::prelude::*;

    let prepared = match PreparedClipPolyline::build(polyline) {
        Some(p) => p,
        None => return (Vec::new(), Vec::new()),
    };

    let target_vertices = mesh.vertices();
    let faces: Vec<[usize; 3]> = mesh.face_vertex_indices_iter().collect();

    let task_count = rayon::current_num_threads().saturating_mul(4).max(1);
    let chunk_size = faces.len().div_ceil(task_count).max(1);
    // Faces are clipped in parallel chunks, so the shared counter reports what
    // has finished rather than the position of any one worker.
    let clipped = progress.counter(faces.len());
    let partials: Vec<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> = faces
        .par_chunks(chunk_size)
        .map(|chunk| {
            let mut chunk_vertices = Vec::new();
            let mut chunk_faces = Vec::new();
            let mut candidate_stack: Vec<usize> = Vec::new();
            let mut candidate_indices: Vec<usize> = Vec::new();
            // Scratch buffer reused across (mesh_triangle, polyline_triangle) pairs.
            // Cleared before each clip; capacity grows once to the worst-case ~6-gon.
            let mut overlap_polyline: Vec<glam::DVec3> = Vec::new();

            for face in chunk.iter().copied() {
                let target = [target_vertices[face[0]], target_vertices[face[1]], target_vertices[face[2]]];

                // AABB reject against the polyline's overall XY footprint. For a small
                // polyline on a large mesh this alone skips ~99% of triangles.
                let (tri_min, tri_max) = triangle_xy_bounds(target);
                if !prepared.xy_bounds_overlap(tri_min, tri_max) {
                    if mode == TriPolylineClipMode::KeepOutside {
                        let polyline: Vec<glam::DVec3> = target.iter().map(|point| glam::DVec3::new(point.x, point.y, point.z)).collect();
                        append_polyline_fan(polyline.iter().copied(), &mut chunk_vertices, &mut chunk_faces);
                    }
                    continue;
                }

                match mode {
                    TriPolylineClipMode::KeepInside => {
                        prepared
                            .spatial
                            .for_each_xy_bounds_candidate_index_with_stack(tri_min, tri_max, &mut candidate_stack, |index| {
                                let poly_triangle = prepared.triangles[index];
                                overlap_polyline.clear();
                                clip_target_triangle_to_reference_xy_exact_into(target, poly_triangle, &mut overlap_polyline);
                                if overlap_polyline.len() < 3 {
                                    return;
                                }
                                // Each overlap is a convex polyline (≤6 verts: a triangle
                                // clipped by 3 half-planes), so a fan triangulates it
                                // exactly without earcut. Z is preserved from the mesh
                                // triangle by the clip, so no per-vertex bary_z either.
                                append_polyline_fan(overlap_polyline.iter().copied(), &mut chunk_vertices, &mut chunk_faces);
                            });
                    }
                    TriPolylineClipMode::KeepOutside => {
                        candidate_indices.clear();
                        prepared
                            .spatial
                            .for_each_xy_bounds_candidate_index_with_stack(tri_min, tri_max, &mut candidate_stack, |index| candidate_indices.push(index));
                        let mut pieces = vec![target.iter().map(|point| glam::DVec3::new(point.x, point.y, point.z)).collect::<Vec<_>>()];
                        for &index in &candidate_indices {
                            let poly_triangle = prepared.triangles[index];
                            let mut next_pieces = Vec::new();
                            for piece in pieces {
                                next_pieces.extend(subtract_triangle_xy(&piece, poly_triangle));
                            }
                            pieces = next_pieces;
                            if pieces.is_empty() {
                                break;
                            }
                        }
                        for piece in pieces {
                            append_polyline_fan(piece.iter().copied(), &mut chunk_vertices, &mut chunk_faces);
                        }
                    }
                }
            }

            clipped.advance_by(chunk.len());
            (chunk_vertices, chunk_faces)
        })
        .collect();

    let mut output_vertices = Vec::with_capacity(partials.iter().map(|(v, _)| v.len()).sum());
    let mut output_faces = Vec::with_capacity(partials.iter().map(|(_, f)| f.len()).sum());
    for (vertices, faces) in partials {
        let base = output_vertices.len() as u32;
        output_vertices.extend(vertices);
        output_faces.extend(faces.into_iter().map(|face| [base + face[0], base + face[1], base + face[2]]));
    }

    (output_vertices, output_faces)
}

/// Subtract one XY clip triangle from a convex 3D polyline. At each clip edge,
/// the portion outside that edge is final output while the inside remainder is
/// passed to the next edge. The final remainder is the intersection and is
/// discarded. This partitions the difference into disjoint convex pieces and
/// interpolates Z at every new boundary point.
fn subtract_triangle_xy(polyline: &[glam::DVec3], clip_triangle: [mesh_data::Vertex; 3]) -> Vec<Vec<glam::DVec3>> {
    if polyline.len() < 3 {
        return Vec::new();
    }
    let clip_points = clip_triangle.map(|point| glam::DVec2::new(point.x, point.y));
    let clip_ccw = triangle_xy_area(clip_triangle) > 0.0;
    let mut remainder = polyline.to_vec();
    let mut outside = Vec::with_capacity(3);
    for edge_index in 0..3 {
        let edge_a = clip_points[edge_index];
        let edge_b = clip_points[(edge_index + 1) % 3];
        let piece = clip_polyline_by_xy_edge_exact(&remainder, edge_a, edge_b, !clip_ccw);
        if piece.len() >= 3 && signed_area_xy(&piece).abs() > 1e-18 {
            outside.push(piece);
        }
        remainder = clip_polyline_by_xy_edge_exact(&remainder, edge_a, edge_b, clip_ccw);
        if remainder.len() < 3 {
            break;
        }
    }
    outside
}

/// Exact-boundary half-plane clip used only while partitioning a difference.
/// The general geometry helper deliberately has a tolerance on both sides;
/// using that for complementary halves would create a thin overlapping strip.
fn clip_polyline_by_xy_edge_exact(polyline: &[glam::DVec3], edge_a: glam::DVec2, edge_b: glam::DVec2, keep_left: bool) -> Vec<glam::DVec3> {
    if polyline.is_empty() {
        return Vec::new();
    }
    let signed_distance = |point: glam::DVec3| {
        let distance = crate::model::kernel::signed_distance_to_line(glam::DVec2::new(point.x, point.y), edge_a, edge_b);
        if keep_left { distance } else { -distance }
    };
    let mut output = Vec::new();
    let mut previous = *polyline.last().expect("polyline is non-empty");
    let mut previous_distance = signed_distance(previous);
    let mut previous_inside = previous_distance >= 0.0;
    for &current in polyline {
        let current_distance = signed_distance(current);
        let current_inside = current_distance >= 0.0;
        if current_inside != previous_inside {
            let denominator = previous_distance - current_distance;
            if denominator.abs() > 1e-20 {
                let t = (previous_distance / denominator).clamp(0.0, 1.0);
                output.push(previous.lerp(current, t));
            }
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_distance = current_distance;
        previous_inside = current_inside;
    }
    crate::model::geometry::deduplicate_ring_by(output, |a, b| a.distance_squared(b) <= crate::model::geometry::RING_DEDUP_EPS_SQ)
}

/// Exact-boundary triangle intersection used by polyline KeepInside. It must
/// share the same classifier as [`subtract_triangle_xy`] so the inside and
/// outside results are a true partition even within the general XY tolerance.
fn clip_target_triangle_to_reference_xy_exact_into(target: [mesh_data::Vertex; 3], reference: [mesh_data::Vertex; 3], output: &mut Vec<glam::DVec3>) {
    let mut polyline: Vec<glam::DVec3> = target.into_iter().map(|point| glam::DVec3::new(point.x, point.y, point.z)).collect();
    let clip_ccw = triangle_xy_area(reference) > 0.0;
    for edge_index in 0..3 {
        let edge_a = glam::DVec2::new(reference[edge_index].x, reference[edge_index].y);
        let edge_b = glam::DVec2::new(reference[(edge_index + 1) % 3].x, reference[(edge_index + 1) % 3].y);
        polyline = clip_polyline_by_xy_edge_exact(&polyline, edge_a, edge_b, clip_ccw);
        if polyline.len() < 3 {
            break;
        }
    }
    output.clear();
    output.extend(polyline);
}

/// Push a convex polyline into the mesh buffers as a triangle fan, dropping
/// degenerate (zero-area) triangles.
pub(super) fn append_polyline_fan(points: impl ExactSizeIterator<Item = glam::DVec3>, vertices: &mut Vec<mesh_data::Vertex>, faces: &mut Vec<[u32; 3]>) {
    let count = points.len();
    if count < 3 {
        return;
    }
    let base = vertices.len() as u32;
    vertices.extend(points.map(|p| mesh_data::Vertex::new(p.x, p.y, p.z)));
    for i in 1..(count - 1) as u32 {
        let face = [base, base + i, base + i + 1];
        let a = vertices[face[0] as usize];
        let b = vertices[face[1] as usize];
        let c = vertices[face[2] as usize];
        let ab = glam::DVec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let ac = glam::DVec3::new(c.x - a.x, c.y - a.y, c.z - a.z);
        if ab.cross(ac).length_squared() > 1e-20 {
            faces.push(face);
        }
    }
}

/// A closed XY polyline pre-triangulated (via earcut) and indexed by a `TriangleBvh`
/// for fast candidate enumeration against many mesh triangles. Built once per
/// `clip_mesh_by_polyline_xy` call.
///
/// Mirrors `PreparedReferenceSurface` but the source is a flat polyline ring instead
/// of a triangulation, and the polyline triangles' Z is irrelevant (only XY drives
/// the containment test - Z on the output mesh comes from the target triangle via
/// `clip_target_triangle_to_reference_xy_into`).
pub(super) struct PreparedClipPolyline {
    pub(super) triangles: Vec<[mesh_data::Vertex; 3]>,
    pub(super) spatial: crate::model::spatial::TriangleBvh,
    pub(super) xy_min: glam::DVec2,
    pub(super) xy_max: glam::DVec2,
}

impl PreparedClipPolyline {
    /// Returns `None` if the polyline has fewer than 3 vertices or no XY area.
    pub(super) fn build(polyline: &[glam::DVec3]) -> Option<Self> {
        if polyline.len() < 3 {
            return None;
        }

        let mut xy_min = glam::DVec2::splat(f64::INFINITY);
        let mut xy_max = glam::DVec2::splat(f64::NEG_INFINITY);
        let flat: Vec<[f64; 2]> = polyline
            .iter()
            .map(|p| {
                xy_min = xy_min.min(glam::DVec2::new(p.x, p.y));
                xy_max = xy_max.max(glam::DVec2::new(p.x, p.y));
                [p.x, p.y]
            })
            .collect();

        let mut earcut_indices: Vec<usize> = Vec::new();
        earcut::Earcut::new().earcut(flat.iter().copied(), &[], &mut earcut_indices);
        if earcut_indices.len() < 3 {
            return None;
        }

        // Build unindexed triangle vertices. Z is unused by the clip path; 0.0 is
        // a stable placeholder that won't perturb the SAT or edge-clip math.
        let mut prepared_vertices: Vec<mesh_data::Vertex> = Vec::with_capacity(earcut_indices.len());
        let mut prepared_faces: Vec<[u32; 3]> = Vec::with_capacity(earcut_indices.len() / 3);
        let mut triangles: Vec<[mesh_data::Vertex; 3]> = Vec::with_capacity(earcut_indices.len() / 3);
        for tri in earcut_indices.as_chunks::<3>().0 {
            let corners = [
                mesh_data::Vertex::new(flat[tri[0]][0], flat[tri[0]][1], 0.0),
                mesh_data::Vertex::new(flat[tri[1]][0], flat[tri[1]][1], 0.0),
                mesh_data::Vertex::new(flat[tri[2]][0], flat[tri[2]][1], 0.0),
            ];
            // Skip zero-area sliver triangles earcut occasionally emits on near-
            // collinear input - they would never produce overlap with anything.
            if triangle_xy_area(corners).abs() <= 1e-18 {
                continue;
            }
            let base = prepared_vertices.len() as u32;
            prepared_vertices.extend_from_slice(&corners);
            prepared_faces.push([base, base + 1, base + 2]);
            triangles.push(corners);
        }
        if triangles.is_empty() {
            return None;
        }

        let mesh = mesh_data::Triangulation::from_vertices_and_faces(prepared_vertices, prepared_faces).ok()?;
        let spatial = crate::model::spatial::TriangleBvh::build(&mesh);

        Some(Self {
            triangles,
            spatial,
            xy_min,
            xy_max,
        })
    }

    /// Cheap broad-phase check: does the supplied XY AABB touch this polyline's
    /// overall footprint (with `XY_TOL` padding)?
    fn xy_bounds_overlap(&self, min: glam::DVec2, max: glam::DVec2) -> bool {
        const TOL: f64 = crate::model::kernel::XY_TOL;
        max.x >= self.xy_min.x - TOL && min.x <= self.xy_max.x + TOL && max.y >= self.xy_min.y - TOL && min.y <= self.xy_max.y + TOL
    }
}

/// Barycentric Z interpolation: given XY point (x, y) inside triangle `v`, return its Z.
pub(super) fn bary_z(x: f64, y: f64, v: [mesh_data::Vertex; 3]) -> f64 {
    let denom = (v[1].y - v[2].y) * (v[0].x - v[2].x) + (v[2].x - v[1].x) * (v[0].y - v[2].y);
    if denom.abs() < 1e-12 {
        return (v[0].z + v[1].z + v[2].z) / 3.0;
    }
    let w0 = ((v[1].y - v[2].y) * (x - v[2].x) + (v[2].x - v[1].x) * (y - v[2].y)) / denom;
    let w1 = ((v[2].y - v[0].y) * (x - v[2].x) + (v[0].x - v[2].x) * (y - v[2].y)) / denom;
    let w2 = 1.0 - w0 - w1;
    w0 * v[0].z + w1 * v[1].z + w2 * v[2].z
}

/// Clip a triangle to the band z_min <= z <= z_max, returning 0–2 result triangles.
pub(crate) fn clip_triangle_z(v: [mesh_data::Vertex; 3], z_min: f64, z_max: f64) -> Vec<[mesh_data::Vertex; 3]> {
    let mut result = clip_triangle_plane(v, z_min, true);
    let above_min = std::mem::take(&mut result);
    for tri in above_min {
        result.extend(clip_triangle_plane(tri, z_max, false));
    }
    result
}

/// Clip a triangle against a single Z plane, keeping the side selected by `keep_above`.
pub(super) fn clip_triangle_plane(v: [mesh_data::Vertex; 3], z_plane: f64, keep_above: bool) -> Vec<[mesh_data::Vertex; 3]> {
    let inside: [bool; 3] = v.map(|vi| if keep_above { vi.z >= z_plane } else { vi.z <= z_plane });
    let count = inside.iter().filter(|&&b| b).count();
    match count {
        0 => vec![],
        3 => vec![v],
        1 => {
            let in_i = inside.iter().position(|&b| b).unwrap();
            let a = v[in_i];
            let b = v[(in_i + 1) % 3];
            let c = v[(in_i + 2) % 3];
            vec![[a, lerp_at_z(a, b, z_plane), lerp_at_z(a, c, z_plane)]]
        }
        2 => {
            let out_i = inside.iter().position(|&b| !b).unwrap();
            let c = v[out_i];
            let a = v[(out_i + 1) % 3];
            let b = v[(out_i + 2) % 3];
            let p = lerp_at_z(c, a, z_plane);
            let q = lerp_at_z(c, b, z_plane);
            vec![[a, b, q], [a, q, p]]
        }
        _ => unreachable!(),
    }
}

/// Where the segment from `a` to `b` crosses height `z`.
///
/// The ends are taken in one fixed order whichever way round they are given,
/// so the two triangles sharing an edge put its crossing at bit-for-bit the
/// same point. Interpolated from opposite ends, the two could differ in the
/// last place - enough, now and then, to land either side of a weld step and
/// leave a crack in the slab the edge belongs to.
pub(super) fn lerp_at_z(a: mesh_data::Vertex, b: mesh_data::Vertex, z: f64) -> mesh_data::Vertex {
    let swapped = (b.x, b.y, b.z) < (a.x, a.y, a.z);
    let (a, b) = if swapped { (b, a) } else { (a, b) };
    if (b.z - a.z).abs() < 1e-12 {
        // Level: the end the caller gave first, as before.
        return if swapped { b } else { a };
    }
    let t = (z - a.z) / (b.z - a.z);
    mesh_data::Vertex::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y), z)
}

#[derive(Clone, Copy)]
pub(super) struct SurfaceClipVertex {
    pub(super) point: glam::DVec3,
    pub(super) height_delta: f64,
}

pub(super) fn clip_mesh_by_surface(
    target: &mesh_data::Triangulation,
    reference: &mesh_data::Triangulation,
    side: TriSurfaceCutSide,
    progress: &crate::model::progress::Phase,
) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> {
    let reference_surface = validate_reference_surface(reference)?;
    if reference_surface.skipped_vertical_faces > 0 {
        userspace_warn!("{}", tr!("cmd-cuts-ignored-vertical-faces", count = reference_surface.skipped_vertical_faces.to_string()));
    }
    Ok(clip_mesh_by_prepared_surface(target, &reference_surface, side, 0.0, progress))
}

/// [`clip_mesh_by_surface`] against a reference already prepared and known to
/// be single-valued.
pub(super) fn clip_mesh_by_prepared_surface(
    target: &mesh_data::Triangulation,
    reference_surface: &PreparedReferenceSurface,
    side: TriSurfaceCutSide,
    clearance: f64,
    progress: &crate::model::progress::Phase,
) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    // Moving the reference `clearance` away from the kept side keeps only
    // what clears it by that much. Ground where the two surfaces run within
    // it of each other is taken as where they meet.
    let offset = match side {
        TriSurfaceCutSide::CutTop => clearance,
        TriSurfaceCutSide::CutBottom => -clearance,
    };
    use rayon::prelude::*;

    let target_vertices = target.vertices();
    let target_faces: Vec<[usize; 3]> = target.face_vertex_indices_iter().collect();
    // Each face is clipped on its own, so the faces are shared out in
    // chunks and the pieces joined back in face order: the sheet comes out
    // the same whichever thread clipped what.
    let task_count = rayon::current_num_threads().saturating_mul(8).max(1);
    let chunk_size = target_faces.len().div_ceil(task_count).max(1);
    let clipped_faces = progress.counter(target_faces.len());
    let partials: Vec<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> = target_faces
        .par_chunks(chunk_size)
        .map(|chunk| {
            let mut output_vertices = Vec::new();
            let mut output_faces = Vec::new();
            let mut overlap = Vec::new();
            let mut candidates = Vec::new();
            for face in chunk {
                let target_triangle = [target_vertices[face[0]], target_vertices[face[1]], target_vertices[face[2]]];
                let target_bounds = triangle_xy_bounds(target_triangle);
                reference_surface
                    .spatial
                    .for_each_xy_bounds_candidate_index_with_stack(target_bounds.0, target_bounds.1, &mut candidates, |reference_index| {
                        let reference_triangle = reference_surface.triangles[reference_index];
                        // Exact half-planes: the reference triangles tile the plane, so
                        // the pieces of one target face must tile it too. The tolerant
                        // clip keeps a point up to `XY_TOL` past an edge where it is,
                        // while the neighbour across that edge cuts at the edge itself,
                        // and the two pieces then fail to share their corners - a crack
                        // wherever a target edge passes within a tenth of a millimetre of
                        // a reference vertex, which a solid then reports as open.
                        clip_target_triangle_to_reference_xy_exact_into(target_triangle, reference_triangle, &mut overlap);
                        if overlap.len() < 3 {
                            return;
                        }

                        let polyline: Vec<SurfaceClipVertex> = overlap
                            .iter()
                            .copied()
                            .map(|point| {
                                let reference_z = bary_z(point.x, point.y, reference_triangle);
                                SurfaceClipVertex {
                                    point,
                                    height_delta: point.z - reference_z + offset,
                                }
                            })
                            .collect();
                        let mut clipped = clip_surface_polyline(polyline, side);
                        if offset != 0.0 {
                            // Where this sheet stops it lies `clearance` from the
                            // reference, and the reference's own sheet stops at the same
                            // line in plan, `clearance` the other way. Both put their
                            // edge half way between, so the two meet there.
                            for vertex in clipped.iter_mut().filter(|vertex| vertex.height_delta.abs() <= 1e-6) {
                                vertex.point.z += offset / 2.0;
                            }
                        }
                        append_surface_clip_polyline(&clipped, &mut output_vertices, &mut output_faces);
                    });
            }
            clipped_faces.advance_by(chunk.len());
            (output_vertices, output_faces)
        })
        .collect();

    let mut output_vertices = Vec::with_capacity(partials.iter().map(|(vertices, _)| vertices.len()).sum());
    let mut output_faces = Vec::with_capacity(partials.iter().map(|(_, faces)| faces.len()).sum());
    for (vertices, faces) in partials {
        let base = output_vertices.len() as u32;
        output_vertices.extend(vertices);
        output_faces.extend(faces.into_iter().map(|face| face.map(|index| base + index)));
    }
    (output_vertices, output_faces)
}

/// Trim a topology to the region the pit shell does not excavate, so a separately rendered
/// pit shell fills the removed area and the two meshes meet along their true 3D contact
/// line (where the shell surface crosses the terrain) rather than a fixed design polyline.
///
/// `envelope` must be the shell's lower envelope (see `build_pit_shell_lower_envelope`):
/// a single-valued 2.5D surface giving, at every XY point of the shell's footprint, the
/// lowest shell surface there. A topology point is excavated exactly when it lies at or
/// above that envelope. Because the envelope cells tile the plane, each topology triangle
/// is rebuilt cell by cell: the overlap with an open cell is kept whole, and the overlap
/// with a covered cell keeps only the part below the cell's plane. Every fragment is a
/// convex polyline (a triangle–triangle overlap split by one half-plane), so emission is a
/// simple fan - no polyline booleans or ear-cutting, whose floating-point failure modes make
/// this construction preferable. Where the shell floats *above* the terrain -
/// e.g. a flat design crest standing over undulating ground near the rim - the topology is
/// below the envelope and is **kept**, so the cut is flush with the real contact line
/// instead of the shell's widest XY extent. Because the lower envelope of a watertight
/// solid is its floor, a flat crest cap never forces removal on its own. Triangles that
/// touch no excavated region pass through unchanged and unfragmented.
pub(super) fn clip_topology_to_pit_shell(
    topology: &mesh_data::Triangulation,
    envelope: &PitShellLowerEnvelope,
    progress: &crate::model::progress::Phase,
) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    use rayon::prelude::*;

    let topology_vertices = topology.vertices();
    let topology_faces: Vec<[usize; 3]> = topology.face_vertex_indices_iter().collect();

    let pass_through = |target: &[mesh_data::Vertex; 3], vertices: &mut Vec<mesh_data::Vertex>, faces: &mut Vec<[u32; 3]>| {
        let base = vertices.len() as u32;
        vertices.extend_from_slice(target);
        faces.push([base, base + 1, base + 2]);
    };

    let task_count = rayon::current_num_threads().saturating_mul(4).max(1);
    let chunk_size = topology_faces.len().div_ceil(task_count).max(1);
    let clipped = progress.counter(topology_faces.len());
    let partials: Vec<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> = topology_faces
        .par_chunks(chunk_size)
        .map(|chunk| {
            let mut chunk_vertices = Vec::new();
            let mut chunk_faces = Vec::new();
            let mut candidate_stack = Vec::new();
            let mut fragments: Vec<Vec<SurfaceClipVertex>> = Vec::new();

            for face in chunk.iter().copied() {
                let target = [topology_vertices[face[0]], topology_vertices[face[1]], topology_vertices[face[2]]];
                let bounds = triangle_xy_bounds(target);

                // Fragment the triangle across the overlapping cells. `height_delta` is linear
                // over each fragment, so a positive delta at some overlap vertex is exactly the
                // condition for that cell to excavate part of the triangle.
                fragments.clear();
                let mut saw_candidate = false;
                let mut any_excavated = false;
                envelope
                    .spatial
                    .for_each_xy_bounds_candidate_index_with_stack(bounds.0, bounds.1, &mut candidate_stack, |index| {
                        saw_candidate = true;
                        let cell = envelope.triangles[index];
                        let overlap = clip_target_triangle_to_reference_xy(target, cell);
                        if overlap.len() < 3 {
                            return;
                        }
                        if !envelope.covered[index] {
                            fragments.push(overlap.into_iter().map(|point| SurfaceClipVertex { point, height_delta: 0.0 }).collect());
                            return;
                        }
                        let polyline: Vec<SurfaceClipVertex> = overlap
                            .into_iter()
                            .map(|point| {
                                let reference_z = bary_z(point.x, point.y, cell);
                                SurfaceClipVertex {
                                    point,
                                    height_delta: point.z - reference_z,
                                }
                            })
                            .collect();
                        if polyline.iter().any(|vertex| vertex.height_delta > 1e-9) {
                            any_excavated = true;
                        }
                        // Keep the part below the envelope (height_delta <= 0): the shell
                        // floats above the terrain there and removes nothing.
                        fragments.push(clip_surface_polyline(polyline, TriSurfaceCutSide::CutTop));
                    });

                if !saw_candidate || !any_excavated {
                    // Beyond even the envelope's padding, or nothing under this triangle is
                    // excavated (open cells only, or the shell floats above the terrain
                    // everywhere here): keep it whole and unfragmented.
                    pass_through(&target, &mut chunk_vertices, &mut chunk_faces);
                    continue;
                }

                for fragment in &fragments {
                    append_surface_clip_polyline(fragment, &mut chunk_vertices, &mut chunk_faces);
                }
            }

            clipped.advance_by(chunk.len());
            (chunk_vertices, chunk_faces)
        })
        .collect();

    let mut output_vertices = Vec::with_capacity(partials.iter().map(|(vertices, _)| vertices.len()).sum());
    let mut output_faces = Vec::with_capacity(partials.iter().map(|(_, faces)| faces.len()).sum());
    for (vertices, faces) in partials {
        let base = output_vertices.len() as u32;
        output_vertices.extend(vertices);
        output_faces.extend(faces.into_iter().map(|face| [base + face[0], base + face[1], base + face[2]]));
    }

    (output_vertices, output_faces)
}

#[derive(Debug)]
pub(super) struct PreparedReferenceSurface {
    pub(super) mesh: std::sync::Arc<mesh_data::Triangulation>,
    pub(super) triangles: Vec<[mesh_data::Vertex; 3]>,
    pub(super) spatial: crate::model::spatial::TriangleBvh,
    pub(super) skipped_vertical_faces: usize,
}

pub(super) fn validate_reference_surface(reference: &mesh_data::Triangulation) -> Result<PreparedReferenceSurface> {
    let prepared = prepare_reference_surface_relaxed(reference)?;
    if let Some(fold) = first_fold(&prepared, reference_xy_overlap_area_tolerance(reference)) {
        anyhow::bail!(
            "Reference topology overlaps itself in XY and is not single-valued \
            (triangles {} and {} overlap by {:.6} square units with up to {:.6} Z difference)",
            fold.first,
            fold.second,
            fold.area,
            fold.z_delta
        );
    }
    Ok(prepared)
}

/// Two faces of one surface covering the same ground in plan at different
/// heights.
struct Fold {
    first: usize,
    second: usize,
    area: f64,
    z_delta: f64,
    /// Whether the two faces give the same height where they overlap.
    coincident: bool,
}

/// The first place `prepared` covers the same ground twice, if it does.
fn first_fold(prepared: &PreparedReferenceSurface, overlap_area_tolerance: f64) -> Option<Fold> {
    folds(prepared, overlap_area_tolerance, false).0
}

/// The first place `prepared` covers the same ground twice, and with `all`,
/// which of its faces take part in any fold.
fn folds(prepared: &PreparedReferenceSurface, overlap_area_tolerance: f64, all: bool) -> (Option<Fold>, Vec<bool>) {
    use rayon::prelude::*;

    // Each face is checked against the faces after it, a face at a time and
    // independently, so the faces are shared out across threads. The first
    // fold reported is still the one a walk in face order meets first.
    let fold_of = |index: usize, stack: &mut Vec<usize>, found: &mut Vec<Fold>| {
        let triangle = prepared.triangles[index];
        let bounds = triangle_xy_bounds(triangle);
        prepared.spatial.for_each_xy_bounds_candidate_index_with_stack(bounds.0, bounds.1, stack, |other_index| {
            if other_index <= index || (!all && !found.is_empty()) {
                return;
            }
            let overlap = triangle_intersection_xy(triangle, prepared.triangles[other_index]);
            let overlap_area = signed_area_xy(&overlap).abs();
            if overlap_area <= overlap_area_tolerance {
                return;
            }
            let z_delta = overlap_z_delta(triangle, prepared.triangles[other_index], &overlap);
            // Coincident triangles - a sliver duplicated where two surfaces
            // were merged - still give one height wherever they overlap, so
            // the surface is single-valued there, and only a fold, where the
            // heights differ, is refused. They are still two layers, though,
            // and taken whole each counts the ground under it again, so they
            // are noted for the repair to make one.
            let coincident = z_delta <= REFERENCE_COINCIDENT_Z_TOLERANCE;
            if !coincident || all {
                found.push(Fold {
                    first: index,
                    second: other_index,
                    area: overlap_area,
                    z_delta,
                    coincident,
                });
            }
        });
    };
    let mut found: Vec<Fold> = if all {
        (0..prepared.triangles.len())
            .into_par_iter()
            .fold(
                || (Vec::new(), Vec::new()),
                |(mut stack, mut found), index| {
                    fold_of(index, &mut stack, &mut found);
                    (stack, found)
                },
            )
            .flat_map_iter(|(_, found)| found)
            .collect()
    } else {
        (0..prepared.triangles.len())
            .into_par_iter()
            .map_init(Vec::new, |stack, index| {
                let mut found = Vec::new();
                fold_of(index, stack, &mut found);
                found.into_iter().min_by_key(|fold| fold.second)
            })
            .find_first(Option::is_some)
            .flatten()
            .into_iter()
            .collect()
    };
    found.sort_by_key(|fold| (fold.first, fold.second));
    let mut folded = vec![false; if all { prepared.triangles.len() } else { 0 }];
    if all {
        for fold in &found {
            folded[fold.first] = true;
            folded[fold.second] = true;
        }
    }
    (found.into_iter().find(|fold| !fold.coincident), folded)
}

/// Which sheet of a folded surface [`single_valued_surface`] keeps where the
/// surface covers the same ground more than once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FoldLayer {
    Lowest,
    Highest,
}

/// How close in plan two points of a surface are taken to be one, in model
/// units, when it has to be repaired. Folds and cracks in real surfaces are
/// mostly points a stitch or a merge left a hair apart, or a hair off the edge
/// they were meant to split.
pub(super) const REPAIR_WELD: f64 = 1.0e-3;

/// `surface` prepared as a clip reference, repaired first if it folds over
/// itself in plan, repeats faces over the same ground, or is cracked; the
/// flag says whether it had to be.
///
/// A design or topography that folds - an overhang, or more often a sliver
/// flipped where two surfaces were stitched together - gives two heights for
/// the same ground, so it cannot say which side of it a point is on. One that
/// is cracked - points a hair apart, or a point a hair off the edge it was
/// meant to split - has edges that no neighbouring face shares, and a solid
/// built from it is open along them.
///
/// The repair welds those points, splits those edges, and re-triangulates the
/// surface's own points in plan with its own edges enforced, so every face
/// that was sound comes back as it was, vertical walls included. Where it
/// folds, the `keep` sheet is the one kept. The copy shares its points, so it
/// is cracked nowhere; it only moves the surface where it folded, by at most
/// the fold's own height, and by the weld where it was cracked.
pub(super) fn single_valued_surface(surface: &mesh_data::Triangulation, keep: FoldLayer) -> Result<(PreparedReferenceSurface, bool)> {
    let prepared = prepare_reference_surface_relaxed(surface)?;
    let (fold, folded) = folds(&prepared, reference_xy_overlap_area_tolerance(surface), true);
    let bounds = surface.bounds();
    let mut graph = SurfaceGraph::weld(&prepared, glam::DVec2::new(bounds.min.x, bounds.min.y), keep);
    // Only the edges of faces that fold can cross. They are noted before the
    // T-junctions are split, while they are still the faces' own edges.
    let folding: HashSet<(usize, usize)> = folded
        .iter()
        .zip(&graph.corners)
        .filter(|(folded, _)| **folded)
        .flat_map(|(_, corners)| [(corners[0], corners[1]), (corners[1], corners[2]), (corners[2], corners[0])])
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a.min(b), a.max(b)))
        .collect();
    graph.split_t_junctions();
    // Faces that overlap at one height are no fold, but are still two
    // layers where the surface should have one.
    let layered = folded.iter().any(|&folded| folded);
    if fold.is_none() && !layered && !graph.cracked {
        return Ok((prepared, false));
    }
    graph.split_crossings(&folding);
    if let Some(fold) = fold {
        log::info!(
            "Repairing surface: faces {} and {} overlap by {:.6} square units with up to {:.6} Z difference",
            fold.first,
            fold.second,
            fold.area,
            fold.z_delta
        );
    }
    let repaired = graph.triangulate(&prepared)?;
    // The copy is a planar triangulation, so it cannot fold, and every face
    // with any area is kept: a sliver dropped here would be a hole in the
    // solid.
    Ok((prepare_reference_surface(&repaired, 0.0)?, true))
}

/// `surface` with each point that lies within [`REPAIR_WELD`] of one of
/// `anchors`' points in plan moved onto it, keeping its own height; `None`
/// when no point moved.
///
/// Two surfaces built one from the other - a design stitched into the
/// topography it was cut from - share points that rounding has moved a hair
/// apart. Clipped against each other, each puts its own copy into the rim
/// where they meet, and a hair is still a gap.
pub(super) fn snap_surface_points(surface: &mesh_data::Triangulation, anchors: &mesh_data::Triangulation) -> Result<Option<mesh_data::Triangulation>> {
    let cell = |value: f64| (value / REPAIR_WELD).floor() as i64;
    let mut cells: HashMap<(i64, i64), Vec<glam::DVec2>> = HashMap::new();
    for anchor in anchors.vertices() {
        let point = glam::DVec2::new(anchor.x, anchor.y);
        cells.entry((cell(point.x), cell(point.y))).or_default().push(point);
    }
    let mut moved = false;
    let vertices: Vec<mesh_data::Vertex> = surface
        .vertices()
        .iter()
        .map(|vertex| {
            let point = glam::DVec2::new(vertex.x, vertex.y);
            let (cx, cy) = (cell(point.x), cell(point.y));
            let nearest = (cx - 1..=cx + 1)
                .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
                .filter_map(|key| cells.get(&key))
                .flatten()
                .copied()
                .map(|anchor| (anchor.distance(point), anchor))
                .filter(|(distance, _)| *distance <= REPAIR_WELD)
                .min_by(|left, right| left.0.total_cmp(&right.0));
            match nearest {
                Some((distance, anchor)) if distance > 0.0 => {
                    moved = true;
                    mesh_data::Vertex::new(anchor.x, anchor.y, vertex.z)
                }
                _ => *vertex,
            }
        })
        .collect();
    if !moved {
        return Ok(None);
    }
    let faces = surface.face_vertex_indices_iter().map(|face| face.map(|index| index as u32)).collect();
    Ok(Some(mesh_data::Triangulation::from_vertices_and_faces(vertices, faces)?))
}

/// A surface's points, welded, and the edges between them, in plan relative
/// to `origin`.
struct SurfaceGraph {
    /// Each point as the surface gave it, so a point that was sound comes
    /// back bit for bit; `points` are relative to `origin` for triangulating.
    world: Vec<glam::DVec2>,
    points: Vec<glam::DVec2>,
    heights: Vec<f64>,
    edges: HashSet<(usize, usize)>,
    /// Each of the surface's faces as the points it was welded to.
    corners: Vec<[usize; 3]>,
    keep: FoldLayer,
    /// Whether any points had to be welded or edges split to make it whole.
    cracked: bool,
}

impl SurfaceGraph {
    /// Weld the corners of `prepared`'s faces that lie within
    /// [`REPAIR_WELD`] of each other in plan. A triangulation in plan holds
    /// only one point at a place; where the welded points differ in height a
    /// wall stands there, and [`Self::triangulate`] gives it back as one
    /// point per height. Only points a hair apart at the same height are a
    /// crack.
    fn weld(prepared: &PreparedReferenceSurface, origin: glam::DVec2, keep: FoldLayer) -> Self {
        // By distance rather than by rounding onto a grid, so two points a
        // hair apart can never land either side of a grid line and stay apart.
        let cell = |value: f64| (value / REPAIR_WELD).floor() as i64;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        let mut exact: HashMap<(u64, u64, u64), usize> = HashMap::with_capacity(prepared.triangles.len());
        let mut graph = Self {
            world: Vec::new(),
            points: Vec::new(),
            heights: Vec::new(),
            edges: HashSet::new(),
            corners: Vec::with_capacity(prepared.triangles.len()),
            keep,
            cracked: false,
        };
        for triangle in &prepared.triangles {
            let mut indices = [0usize; 3];
            for (slot, corner) in triangle.iter().enumerate() {
                // Most corners repeat a point exactly - each is shared by
                // several faces - and one lookup finds those.
                if let Some(&index) = exact.get(&(corner.x.to_bits(), corner.y.to_bits(), corner.z.to_bits())) {
                    indices[slot] = index;
                    continue;
                }
                let local = glam::DVec2::new(corner.x - origin.x, corner.y - origin.y);
                let (cx, cy) = (cell(local.x), cell(local.y));
                let found = (cx - 1..=cx + 1)
                    .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
                    .filter_map(|key| cells.get(&key))
                    .flatten()
                    .copied()
                    .find(|&index| graph.points[index].distance(local) <= REPAIR_WELD);
                indices[slot] = match found {
                    Some(index) if graph.points[index] == local && graph.heights[index] == corner.z => index,
                    Some(index) => {
                        graph.cracked |= graph.points[index] != local && (graph.heights[index] - corner.z).abs() <= REFERENCE_COINCIDENT_Z_TOLERANCE;
                        if graph.better(corner.z, graph.heights[index]) {
                            graph.heights[index] = corner.z;
                        }
                        index
                    }
                    None => {
                        graph.world.push(glam::DVec2::new(corner.x, corner.y));
                        graph.points.push(local);
                        graph.heights.push(corner.z);
                        cells.entry((cx, cy)).or_default().push(graph.points.len() - 1);
                        graph.points.len() - 1
                    }
                };
                exact.insert((corner.x.to_bits(), corner.y.to_bits(), corner.z.to_bits()), indices[slot]);
            }
            for i in 0..3 {
                let (a, b) = (indices[i], indices[(i + 1) % 3]);
                if a != b {
                    graph.edges.insert(if a < b { (a, b) } else { (b, a) });
                }
            }
            graph.corners.push(indices);
        }
        graph
    }

    fn better(&self, candidate: f64, current: f64) -> bool {
        match self.keep {
            FoldLayer::Lowest => candidate < current,
            FoldLayer::Highest => candidate > current,
        }
    }

    /// Split every edge at the points lying on it in plan: where a long edge
    /// on one side of a seam meets several short ones on the other, the
    /// surface is open along the seam until the long one is split to match.
    /// A point on an edge at another height is the foot or head of a wall
    /// standing on it, which splits it just the same.
    fn split_t_junctions(&mut self) {
        use rayon::prelude::*;

        if self.edges.is_empty() {
            return;
        }
        let mean = self.edges.iter().map(|&(a, b)| self.points[a].distance(self.points[b])).sum::<f64>() / self.edges.len() as f64;
        let size = mean.max(16.0 * REPAIR_WELD);
        let cell = |value: f64| (value / size).floor() as i64;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, point) in self.points.iter().enumerate() {
            cells.entry((cell(point.x), cell(point.y))).or_default().push(index);
        }

        // Each edge looks for the points along it on its own; the edges are
        // shared out across threads and the splits applied afterwards.
        let edges: Vec<(usize, usize)> = self.edges.iter().copied().collect();
        let found: Vec<((usize, usize), Vec<usize>, bool)> = edges
            .par_iter()
            .map_init(Vec::new, |on_edge: &mut Vec<(f64, usize)>, &(a, b)| {
                let (from, to) = (self.points[a], self.points[b]);
                let span = to - from;
                let length = span.length();
                if length <= 2.0 * REPAIR_WELD {
                    return None;
                }
                let (min, max) = (from.min(to) - REPAIR_WELD, from.max(to) + REPAIR_WELD);
                on_edge.clear();
                for x in cell(min.x)..=cell(max.x) {
                    for y in cell(min.y)..=cell(max.y) {
                        for &index in cells.get(&(x, y)).into_iter().flatten() {
                            if index == a || index == b {
                                continue;
                            }
                            let offset = self.points[index] - from;
                            let along = offset.dot(span) / length;
                            if along <= REPAIR_WELD || along >= length - REPAIR_WELD || span.perp_dot(offset).abs() / length > REPAIR_WELD {
                                continue;
                            }
                            on_edge.push((along / length, index));
                        }
                    }
                }
                if on_edge.is_empty() {
                    return None;
                }
                on_edge.sort_by(|left, right| left.0.total_cmp(&right.0));
                let cracked = on_edge.iter().any(|&(t, index)| {
                    let height = self.heights[a] + t * (self.heights[b] - self.heights[a]);
                    (self.heights[index] - height).abs() <= REFERENCE_COINCIDENT_Z_TOLERANCE
                });
                let chain = std::iter::once(a).chain(on_edge.iter().map(|&(_, index)| index)).chain(std::iter::once(b)).collect();
                Some(((a, b), chain, cracked))
            })
            .flatten()
            .collect();
        let mut split = Vec::with_capacity(found.len());
        for (edge, chain, cracked) in found {
            self.cracked |= cracked;
            split.push((edge, chain));
        }
        for (edge, chain) in split {
            self.edges.remove(&edge);
            for pair in chain.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if a != b {
                    self.edges.insert(if a < b { (a, b) } else { (b, a) });
                }
            }
        }
    }

    /// Split the edges that cross in plan where they cross. They only do
    /// where the surface folds, and enforced whole they cannot be; left out,
    /// the triangulation would join points across the fold's outline, where
    /// the kept sheet steps from one fold to the next, and slope over the step.
    /// A crossing within the weld of a point already here is that point.
    fn split_crossings(&mut self, folding: &HashSet<(usize, usize)>) {
        // Edges between points of folding faces: their own, and any piece of
        // one a T-junction split off between two such points.
        let folding_points: HashSet<usize> = folding.iter().flat_map(|&(a, b)| [a, b]).collect();
        let edges: Vec<(usize, usize)> = self
            .edges
            .iter()
            .copied()
            .filter(|&(a, b)| folding.contains(&(a, b)) || folding_points.contains(&a) && folding_points.contains(&b))
            .collect();
        if edges.is_empty() {
            return;
        }
        let mean = edges.iter().map(|&(a, b)| self.points[a].distance(self.points[b])).sum::<f64>() / edges.len() as f64;
        let size = mean.max(16.0 * REPAIR_WELD);
        let cell = |value: f64| (value / size).floor() as i64;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, &(a, b)) in edges.iter().enumerate() {
            let (min, max) = (self.points[a].min(self.points[b]), self.points[a].max(self.points[b]));
            for x in cell(min.x)..=cell(max.x) {
                for y in cell(min.y)..=cell(max.y) {
                    cells.entry((x, y)).or_default().push(index);
                }
            }
        }

        // Every crossing, as a parameter along each edge it splits.
        let mut crossings: Vec<(usize, f64, glam::DVec2, f64)> = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        for indices in cells.values() {
            for (slot, &first) in indices.iter().enumerate() {
                for &second in &indices[slot + 1..] {
                    let pair = (first.min(second), first.max(second));
                    if !seen.insert(pair) {
                        continue;
                    }
                    let ((a, b), (c, d)) = (edges[pair.0], edges[pair.1]);
                    if a == c || a == d || b == c || b == d {
                        continue;
                    }
                    let (p, r) = (self.points[a], self.points[b] - self.points[a]);
                    let (q, s) = (self.points[c], self.points[d] - self.points[c]);
                    let denominator = r.perp_dot(s);
                    if denominator.abs() <= f64::EPSILON * r.length() * s.length() {
                        continue;
                    }
                    let t = (q - p).perp_dot(s) / denominator;
                    let u = (q - p).perp_dot(r) / denominator;
                    if t <= 0.0 || t >= 1.0 || u <= 0.0 || u >= 1.0 {
                        continue;
                    }
                    let at = p + r * t;
                    let height = self.heights[a] + t * (self.heights[b] - self.heights[a]);
                    crossings.push((pair.0, t, at, height));
                    crossings.push((pair.1, u, at, height));
                }
            }
        }
        if crossings.is_empty() {
            return;
        }

        // Weld each crossing to a point already here, or add it.
        let weld_cell = |point: glam::DVec2| ((point.x / REPAIR_WELD).floor() as i64, (point.y / REPAIR_WELD).floor() as i64);
        let mut near: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, point) in self.points.iter().enumerate() {
            near.entry(weld_cell(*point)).or_default().push(index);
        }
        let origin = self.world[0] - self.points[0];
        let mut along: HashMap<usize, Vec<(f64, usize)>> = HashMap::new();
        for (edge, t, at, height) in crossings {
            let (cx, cy) = weld_cell(at);
            let found = (cx - 1..=cx + 1)
                .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
                .filter_map(|key| near.get(&key))
                .flatten()
                .copied()
                .find(|&index| self.points[index].distance(at) <= REPAIR_WELD);
            let index = found.unwrap_or_else(|| {
                self.world.push(at + origin);
                self.points.push(at);
                self.heights.push(height);
                near.entry((cx, cy)).or_default().push(self.points.len() - 1);
                self.points.len() - 1
            });
            let (a, b) = edges[edge];
            if index != a && index != b {
                along.entry(edge).or_default().push((t, index));
            }
        }
        for (edge, mut points) in along {
            points.sort_by(|left, right| left.0.total_cmp(&right.0));
            let (a, b) = edges[edge];
            self.edges.remove(&(a, b));
            let chain: Vec<usize> = std::iter::once(a).chain(points.into_iter().map(|(_, index)| index)).chain(std::iter::once(b)).collect();
            for pair in chain.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                if from != to {
                    self.edges.insert(if from < to { (from, to) } else { (to, from) });
                }
            }
        }
    }

    /// Triangulate the welded points in plan with the edges enforced, keeping
    /// the cells over ground `prepared` covers.
    ///
    /// Each cell lies on the kept face over it, and its corners take that
    /// face's heights, so wherever the surface was sound it comes back
    /// exactly. That includes a vertical wall, as two points at one place in
    /// plan, which is how the solid builder expects to meet one. Where the
    /// surface folds its edges are split where they cross
    /// ([`Self::split_crossings`]), so no cell straddles a fold's outline; any
    /// crossing rounding still leaves is left out. The small steps a fold
    /// leaves between cells meeting at a point are closed onto the kept sheet.
    fn triangulate(self, prepared: &PreparedReferenceSurface) -> Result<mesh_data::Triangulation> {
        let points: Vec<spade::Point2<f64>> = self.points.iter().map(|point| spade::Point2::new(point.x, point.y)).collect();
        // Nothing is split here, so the triangulation's points are the graph's.
        let (_, cells) = constrained_triangulation(points, self.edges.clone(), "Repaired surface", None)?;
        let world = |index: usize| self.world[index];

        // The kept sheet over each cell, from its centroid; cells over ground
        // the surface does not cover - the triangulation fills the convex
        // hull - are dropped.
        let mut stack = Vec::new();
        let mut kept: Vec<([usize; 3], [f64; 3])> = Vec::with_capacity(cells.len());
        for corners in cells {
            let centroid = (world(corners[0]) + world(corners[1]) + world(corners[2])) / 3.0;
            let mut face: Option<(f64, usize)> = None;
            prepared.spatial.for_each_xy_bounds_candidate_index_with_stack(centroid, centroid, &mut stack, |index| {
                if let Some(z) = point_in_triangle_bary_z(centroid.x, centroid.y, prepared.triangles[index])
                    && face.is_none_or(|(best, _)| self.better(z, best))
                {
                    face = Some((z, index));
                }
            });
            let Some((_, face)) = face else {
                continue;
            };
            let triangle = prepared.triangles[face];
            let heights = corners.map(|corner| {
                let point = world(corner);
                if let Some(own) = triangle.iter().find(|vertex| vertex.x == point.x && vertex.y == point.y) {
                    return own.z;
                }
                match point_in_triangle_bary_z(point.x, point.y, triangle) {
                    Some(z) => z,
                    // A fold's corner the cell's face does not reach: the kept
                    // sheet at that point instead.
                    None => self.sheet_height(prepared, point, self.heights[corner], &mut stack),
                }
            });
            kept.push((corners, heights));
        }

        // Cells meeting at a point agree on its height unless a wall stands
        // there. Heights within `REPAIR_STEP` of each other are one sheet.
        let mut at_point: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new(); self.world.len()];
        for (cell, (corners, heights)) in kept.iter().enumerate() {
            for slot in 0..3 {
                at_point[corners[slot]].push((heights[slot], cell, slot));
            }
        }
        let mut vertices = Vec::new();
        let mut faces: Vec<[u32; 3]> = kept.iter().map(|_| [0; 3]).collect();
        for (point, mut uses) in at_point.into_iter().enumerate() {
            uses.sort_by(|left, right| left.0.total_cmp(&right.0));
            let place = world(point);
            let mut start = 0;
            while start < uses.len() {
                let mut end = start + 1;
                while end < uses.len() && uses[end].0 - uses[end - 1].0 <= REPAIR_STEP {
                    end += 1;
                }
                let height = match self.keep {
                    FoldLayer::Lowest => uses[start].0,
                    FoldLayer::Highest => uses[end - 1].0,
                };
                vertices.push(mesh_data::Vertex::new(place.x, place.y, height));
                for &(_, cell, slot) in &uses[start..end] {
                    faces[cell][slot] = (vertices.len() - 1) as u32;
                }
                start = end;
            }
        }
        if faces.is_empty() {
            anyhow::bail!("Repaired surface has no XY footprint left to rebuild");
        }
        Ok(mesh_data::Triangulation::from_vertices_and_faces(vertices, faces)?)
    }

    /// The height of the kept sheet at `point`: the kept one of the faces
    /// holding it, or `fallback` where none quite does.
    fn sheet_height(&self, prepared: &PreparedReferenceSurface, point: glam::DVec2, fallback: f64, stack: &mut Vec<usize>) -> f64 {
        let mut height: Option<f64> = None;
        prepared.spatial.for_each_xy_bounds_candidate_index_with_stack(point, point, stack, |index| {
            if let Some(z) = point_in_triangle_bary_z(point.x, point.y, prepared.triangles[index])
                && height.is_none_or(|best| self.better(z, best))
            {
                height = Some(z);
            }
        });
        height.unwrap_or(fallback)
    }
}

/// Steps no taller than this, in model units, between cells of a repaired
/// surface that meet at one point are closed: they are what is left of a fold.
/// Taller ones are walls the surface was built with, and are kept.
const REPAIR_STEP: f64 = 0.1;

/// Two overlapping reference triangles this close in height are one surface.
const REFERENCE_COINCIDENT_Z_TOLERANCE: f64 = 1.0e-3;

pub(super) fn reference_xy_overlap_area_tolerance(reference: &mesh_data::Triangulation) -> f64 {
    let bounds = reference.bounds();
    let dx = bounds.max.x - bounds.min.x;
    let dy = bounds.max.y - bounds.min.y;
    (dx.abs().max(dy.abs()).powi(2) * 1.0e-14).max(1.0e-10)
}

fn overlap_z_delta(a: [mesh_data::Vertex; 3], b: [mesh_data::Vertex; 3], overlap: &[glam::DVec2]) -> f64 {
    if overlap.is_empty() {
        return 0.0;
    }
    let mut samples = overlap.to_vec();
    let centroid = overlap.iter().copied().fold(glam::DVec2::ZERO, |sum, point| sum + point) / overlap.len() as f64;
    samples.push(centroid);

    samples
        .into_iter()
        .map(|point| (bary_z(point.x, point.y, a) - bary_z(point.x, point.y, b)).abs())
        .fold(0.0, f64::max)
}

/// Prepare a pit shell mesh as input to `build_pit_shell_lower_envelope`. Unlike
/// `prepare_reference_surface_relaxed`, this keeps vertical and near-vertical wall faces -
/// they have little or no XY-projected area, but their edges are exactly the boundaries the
/// envelope arrangement must respect (bench crests, wall toes), and their surfaces define
/// the envelope over wall bands. Only genuinely degenerate faces (zero area in 3D -
/// duplicate or collinear points) are dropped.
pub(super) fn prepare_pit_shell_surface(pit_shell: &mesh_data::Triangulation) -> Result<PreparedReferenceSurface> {
    let vertices = pit_shell.vertices();
    if pit_shell.face_count() == 0 {
        anyhow::bail!("Pit shell contains no faces");
    }

    let mut prepared_vertices = Vec::new();
    let mut prepared_faces = Vec::new();
    let mut triangles = Vec::new();

    for face in pit_shell.face_vertex_indices_iter() {
        let triangle = [vertices[face[0]], vertices[face[1]], vertices[face[2]]];
        if triangle_area_3d(triangle) <= 1e-9 {
            continue;
        }
        let base = prepared_vertices.len() as u32;
        prepared_vertices.extend_from_slice(&triangle);
        prepared_faces.push([base, base + 1, base + 2]);
        triangles.push(triangle);
    }

    if triangles.is_empty() {
        anyhow::bail!("Pit shell contains no usable (non-degenerate) faces");
    }

    let mesh = mesh_data::Triangulation::from_vertices_and_faces(prepared_vertices, prepared_faces)?;
    let spatial = crate::model::spatial::TriangleBvh::build(&mesh);
    Ok(PreparedReferenceSurface {
        mesh: std::sync::Arc::new(mesh),
        triangles,
        spatial,
        skipped_vertical_faces: 0,
    })
}

pub(super) fn triangle_area_3d(triangle: [mesh_data::Vertex; 3]) -> f64 {
    let a = glam::DVec3::new(triangle[0].x, triangle[0].y, triangle[0].z);
    let b = glam::DVec3::new(triangle[1].x, triangle[1].y, triangle[1].z);
    let c = glam::DVec3::new(triangle[2].x, triangle[2].y, triangle[2].z);
    (b - a).cross(c - a).length() * 0.5
}

/// The pit shell's lower envelope: a triangulated planar subdivision that tiles all of XY.
/// Cells with `covered[i] == true` carry the lowest shell surface over their footprint in
/// their vertex Z; open cells (outside the shell footprint, or padding around it) never
/// remove topology and carry no meaningful Z.
#[derive(Debug)]
pub(super) struct PitShellLowerEnvelope {
    _mesh: std::sync::Arc<mesh_data::Triangulation>,
    triangles: Vec<[mesh_data::Vertex; 3]>,
    covered: Vec<bool>,
    spatial: crate::model::spatial::TriangleBvh,
}

/// Insert the constraint edges the bulk loader rejected as crossing, splitting them at
/// the intersections. spade's splitting insert asserts internally when the computed
/// intersection point snaps onto nearby existing geometry that still blocks the
/// constraint - near-coincident constraint sets do this, e.g. re-running Include over a
/// footprint whose contact ring a previous run already stitched into the topology. The
/// CDTs built here only classify cells by an interior sample, so a dropped hairline
/// constraint is harmless; recover from the panic, keep the remaining constraints, and
/// warn instead of crashing.
pub(super) fn add_split_constraints(cdt: &mut spade::ConstrainedDelaunayTriangulation<spade::Point2<f64>>, conflicting_edges: Vec<[usize; 2]>, site: &str, origin: glam::DVec2) {
    use spade::Triangulation as _;

    let mut skipped = 0usize;
    for [a, b] in conflicting_edges {
        let handle_a = spade::handles::FixedVertexHandle::from_index(a);
        let handle_b = spade::handles::FixedVertexHandle::from_index(b);
        let inserted = crate::logging::catch_panic_quietly(|| {
            cdt.add_constraint_and_split(handle_a, handle_b, |point| point);
        });
        if inserted.is_none() {
            skipped += 1;
            let from = cdt.vertex(handle_a).position();
            let to = cdt.vertex(handle_b).position();
            userspace_warn!(
                "{}",
                tr!(
                    "cmd-cuts-site-skipped-constraint-from-x",
                    site = site.to_string(),
                    from_x = format!("{:.4}", from.x + origin.x),
                    from_y = format!("{:.4}", from.y + origin.y),
                    to_x = format!("{:.4}", to.x + origin.x),
                    to_y = format!("{:.4}", to.y + origin.y)
                )
            );
        }
    }
    if skipped > 0 {
        userspace_warn!("{}", tr!("cmd-cuts-skipped-degenerate-edges", site = site.to_string(), skipped = skipped.to_string()));
    }
}

/// Constrained Delaunay cells over `points` with `edges` enforced, in world XY.
/// `points` must already be deduplicated, so the indices spade reports for
/// conflicting constraints are ours; those are split rather than dropped.
/// `name` labels errors and `site` the split warnings.
pub(super) fn constrained_cells(points: Vec<spade::Point2<f64>>, edges: HashSet<(usize, usize)>, name: &str, site: &str, origin: glam::DVec2) -> Result<Vec<[glam::DVec2; 3]>> {
    let (positions, cells) = constrained_triangulation(points, edges, name, Some((site, origin)))?;
    Ok(cells.into_iter().map(|cell| cell.map(|index| positions[index] + origin)).collect())
}

/// [`constrained_cells`] as indices into the triangulation's own points, in
/// the coordinates they were given in. The first `points.len()` are `points`
/// in order; any after them are where crossing constraints were split. With
/// no `split` (the site and origin its warnings name), crossing constraints
/// are left out instead.
pub(super) fn constrained_triangulation(
    points: Vec<spade::Point2<f64>>,
    edges: HashSet<(usize, usize)>,
    name: &str,
    split: Option<(&str, glam::DVec2)>,
) -> Result<(Vec<glam::DVec2>, Vec<[usize; 3]>)> {
    use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation as _};

    let mut edge_list: Vec<[usize; 2]> = edges.into_iter().map(|(a, b)| [a, b]).collect();
    edge_list.sort_unstable();

    let point_count = points.len();
    let mut conflicting_edges: Vec<[usize; 2]> = Vec::new();
    let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> = ConstrainedDelaunayTriangulation::try_bulk_load_cdt(points, edge_list, |edge| conflicting_edges.push(edge))
        .map_err(|error| anyhow::anyhow!("{name} CDT bulk load failed: {error:?}"))?;
    if cdt.num_vertices() != point_count {
        anyhow::bail!("{name} CDT dropped vertices unexpectedly during bulk load");
    }
    if let Some((site, origin)) = split {
        add_split_constraints(&mut cdt, conflicting_edges, site, origin);
    }

    let positions = cdt
        .vertices()
        .map(|vertex| {
            let position = vertex.position();
            glam::DVec2::new(position.x, position.y)
        })
        .collect();
    let cells = cdt.inner_faces().map(|face| face.vertices().map(|vertex| vertex.fix().index())).collect();
    Ok((positions, cells))
}

/// How far beyond the shell bounds the envelope's open padding cells extend (metres).
/// Larger than any real survey extent, so every topology triangle lands inside the padded
/// triangulation and is handled by the same per-cell path.
pub(super) const ENVELOPE_PADDING: f64 = 1.0e7;

/// Build the pit shell's lower envelope: the single-valued 2.5D surface giving, at every
/// XY point of the shell's footprint, the lowest shell surface there. This reduces the
/// multi-valued shell (walls, benches, watertight solids) to the one surface that decides
/// excavation - a topology point is excavated exactly when it lies above the lower envelope.
///
/// The construction is exact, not sampled. Every shell triangle edge is projected to XY and
/// inserted as a CDT constraint (splitting where edges cross), so no output cell interior
/// crosses the projected boundary of any shell face. Within one cell the set of covering
/// shell faces is therefore constant, and - because a valid shell does not self-intersect,
/// so faces overlapping in XY never cross in 3D - their vertical order is constant too.
/// One interior sample per cell then identifies the lowest covering face exactly, and that
/// face's plane supplies the cell's corner elevations. Exactly vertical faces contribute
/// their edges as constraints (bench crests and wall toes land on cell boundaries, where
/// the envelope legitimately jumps) but never supply elevations. Cells with no covering
/// face - outside the shell's (possibly concave) footprint, or in the far padding - are
/// kept as open cells so the envelope tiles the whole plane.
pub(super) fn build_pit_shell_lower_envelope(pit_shell: &PreparedReferenceSurface) -> Result<PitShellLowerEnvelope> {
    use rayon::prelude::*;
    use spade::Point2;

    // Work in coordinates local to the shell's minimum corner: at real-world (UTM-scale)
    // magnitudes the absolute coordinates would erode the precision of the CDT's
    // constraint-splitting intersection points.
    let bounds = pit_shell.mesh.bounds();
    let origin = glam::DVec2::new(bounds.min.x, bounds.min.y);
    let extent = glam::DVec2::new(bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y);

    // Dedup projected points exactly (by bit pattern, with -0.0 normalised so it cannot
    // alias 0.0 under spade's positional dedup) and collect each undirected edge once, so
    // the CDT can be bulk-loaded instead of point-located per triangle corner - the
    // incremental build dominated the whole cut on large shells. Only edges the bulk
    // loader rejects as conflicting (crossing another constraint in projection, i.e.
    // walls over benches) go through the splitting insert.
    let normalized = |value: f64| if value == 0.0 { 0.0 } else { value };
    let mut points: Vec<Point2<f64>> = Vec::with_capacity(pit_shell.triangles.len() + 4);
    for (corner_x, corner_y) in [
        (-ENVELOPE_PADDING, -ENVELOPE_PADDING),
        (extent.x + ENVELOPE_PADDING, -ENVELOPE_PADDING),
        (extent.x + ENVELOPE_PADDING, extent.y + ENVELOPE_PADDING),
        (-ENVELOPE_PADDING, extent.y + ENVELOPE_PADDING),
    ] {
        points.push(Point2::new(corner_x, corner_y));
    }
    let mut point_indices: HashMap<(u64, u64), usize> = HashMap::new();
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    for triangle in &pit_shell.triangles {
        let mut indices = [0usize; 3];
        for (slot, point) in triangle.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() {
                anyhow::bail!("Pit shell contains non-finite coordinates");
            }
            let local_x = normalized(point.x - origin.x);
            let local_y = normalized(point.y - origin.y);
            indices[slot] = *point_indices.entry((local_x.to_bits(), local_y.to_bits())).or_insert_with(|| {
                points.push(Point2::new(local_x, local_y));
                points.len() - 1
            });
        }
        for i in 0..3 {
            let a = indices[i];
            let b = indices[(i + 1) % 3];
            if a != b {
                edges.insert(if a < b { (a, b) } else { (b, a) });
            }
        }
    }
    let cells = constrained_cells(points, edges, "Pit shell", "Pit shell envelope", origin)?;
    let classified: Vec<([mesh_data::Vertex; 3], bool)> = cells
        .par_iter()
        .fold(
            || (Vec::new(), Vec::new()),
            |(mut candidate_stack, mut output), corners| {
                let centroid = (corners[0] + corners[1] + corners[2]) / 3.0;
                let lowest = lowest_covering_shell_triangle(pit_shell, centroid, &mut candidate_stack);
                let cell = corners.map(|corner| {
                    let z = lowest.map_or(0.0, |lowest| bary_z(corner.x, corner.y, lowest));
                    mesh_data::Vertex::new(corner.x, corner.y, z)
                });
                if triangle_xy_area(cell).abs() > 1e-12 {
                    output.push((cell, lowest.is_some()));
                }
                (candidate_stack, output)
            },
        )
        .map(|(_, output)| output)
        .reduce(Vec::new, |mut left, right| {
            left.extend(right);
            left
        });

    let mut vertices = Vec::with_capacity(classified.len() * 3);
    let mut faces = Vec::with_capacity(classified.len());
    let mut triangles = Vec::with_capacity(classified.len());
    let mut covered = Vec::with_capacity(classified.len());
    for (cell, is_covered) in classified {
        let base = vertices.len() as u32;
        vertices.extend_from_slice(&cell);
        faces.push([base, base + 1, base + 2]);
        triangles.push(cell);
        covered.push(is_covered);
    }

    if !covered.iter().any(|&is_covered| is_covered) {
        anyhow::bail!("Pit shell has no XY footprint to build a lower envelope from");
    }

    let mesh = mesh_data::Triangulation::from_vertices_and_faces(vertices, faces)?;
    let spatial = crate::model::spatial::TriangleBvh::build(&mesh);
    Ok(PitShellLowerEnvelope {
        _mesh: std::sync::Arc::new(mesh),
        triangles,
        covered,
        spatial,
    })
}

/// The lowest shell face covering `point` in XY. Inclusive of face edges; exactly vertical
/// faces (degenerate XY projection) never match.
fn lowest_covering_shell_triangle(pit_shell: &PreparedReferenceSurface, point: glam::DVec2, candidate_stack: &mut Vec<usize>) -> Option<[mesh_data::Vertex; 3]> {
    let mut lowest: Option<(f64, [mesh_data::Vertex; 3])> = None;
    pit_shell.spatial.for_each_xy_bounds_candidate_index_with_stack(point, point, candidate_stack, |index| {
        let triangle = pit_shell.triangles[index];
        if let Some(z) = point_in_triangle_bary_z(point.x, point.y, triangle)
            && lowest.is_none_or(|(lowest_z, _)| z < lowest_z)
        {
            lowest = Some((z, triangle));
        }
    });
    lowest.map(|(_, triangle)| triangle)
}

/// Barycentric Z of `(x, y)` on triangle `v`, inclusive of XY edges. `None` outside the
/// triangle or when its XY projection is degenerate.
pub(super) fn point_in_triangle_bary_z(x: f64, y: f64, v: [mesh_data::Vertex; 3]) -> Option<f64> {
    let denom = (v[1].y - v[2].y) * (v[0].x - v[2].x) + (v[2].x - v[1].x) * (v[0].y - v[2].y);
    if denom.abs() < 1e-12 {
        return None;
    }
    let w0 = ((v[1].y - v[2].y) * (x - v[2].x) + (v[2].x - v[1].x) * (y - v[2].y)) / denom;
    let w1 = ((v[2].y - v[0].y) * (x - v[2].x) + (v[0].x - v[2].x) * (y - v[2].y)) / denom;
    let w2 = 1.0 - w0 - w1;
    if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
        return None;
    }
    Some(w0 * v[0].z + w1 * v[1].z + w2 * v[2].z)
}

pub(super) fn prepare_reference_surface_relaxed(reference: &mesh_data::Triangulation) -> Result<PreparedReferenceSurface> {
    prepare_reference_surface(reference, reference_xy_overlap_area_tolerance(reference))
}

/// Prepare `reference` for clipping, dropping the faces with no more than
/// `xy_area_tolerance` of area in plan as vertical.
fn prepare_reference_surface(reference: &mesh_data::Triangulation, xy_area_tolerance: f64) -> Result<PreparedReferenceSurface> {
    let vertices = reference.vertices();
    if reference.face_count() == 0 {
        anyhow::bail!("Reference topology contains no faces");
    }

    let mut prepared_vertices = Vec::new();
    let mut prepared_faces = Vec::new();
    let mut triangles = Vec::new();
    let mut skipped_vertical_faces = 0usize;

    for face in reference.face_vertex_indices_iter() {
        let triangle = [vertices[face[0]], vertices[face[1]], vertices[face[2]]];
        if triangle_xy_area(triangle).abs() <= xy_area_tolerance {
            skipped_vertical_faces += 1;
            continue;
        }
        let base = prepared_vertices.len() as u32;
        prepared_vertices.extend_from_slice(&triangle);
        prepared_faces.push([base, base + 1, base + 2]);
        triangles.push(triangle);
    }

    if triangles.is_empty() {
        anyhow::bail!("Reference topology must contain at least one non-vertical 2.5D face");
    }

    let mesh = mesh_data::Triangulation::from_vertices_and_faces(prepared_vertices, prepared_faces)?;
    let spatial = crate::model::spatial::TriangleBvh::build(&mesh);
    Ok(PreparedReferenceSurface {
        mesh: std::sync::Arc::new(mesh),
        triangles,
        spatial,
        skipped_vertical_faces,
    })
}

pub(super) fn triangle_xy_bounds(triangle: [mesh_data::Vertex; 3]) -> (glam::DVec2, glam::DVec2) {
    let mut min = glam::DVec2::splat(f64::INFINITY);
    let mut max = glam::DVec2::splat(f64::NEG_INFINITY);
    for point in triangle {
        let xy = glam::DVec2::new(point.x, point.y);
        min = min.min(xy);
        max = max.max(xy);
    }
    (min, max)
}

pub(super) fn triangle_intersection_xy(subject: [mesh_data::Vertex; 3], clip: [mesh_data::Vertex; 3]) -> Vec<glam::DVec2> {
    let mut polyline: Vec<glam::DVec2> = subject.iter().map(|point| glam::DVec2::new(point.x, point.y)).collect();
    let clip_points = clip.map(|point| glam::DVec2::new(point.x, point.y));
    let clip_ccw = triangle_xy_area(clip) > 0.0;

    for edge_index in 0..3 {
        let edge_a = clip_points[edge_index];
        let edge_b = clip_points[(edge_index + 1) % 3];
        polyline = clip_polyline_by_xy_edge(&polyline, edge_a, edge_b, clip_ccw);
        if polyline.len() < 3 {
            break;
        }
    }
    polyline
}

pub(super) fn clip_target_triangle_to_reference_xy(target: [mesh_data::Vertex; 3], reference: [mesh_data::Vertex; 3]) -> Vec<glam::DVec3> {
    let mut out = Vec::new();
    clip_target_triangle_to_reference_xy_into(target, reference, &mut out);
    out
}

/// In-place variant of `clip_target_triangle_to_reference_xy`: clears `out` and
/// appends the convex overlap polyline (possibly empty). Lets high-frequency
/// callers (e.g. `clip_mesh_by_polyline_xy`) reuse one scratch buffer across
/// many triangle-pair tests instead of allocating per call.
pub(super) fn clip_target_triangle_to_reference_xy_into(target: [mesh_data::Vertex; 3], reference: [mesh_data::Vertex; 3], out: &mut Vec<glam::DVec3>) {
    out.clear();
    if !triangles_overlap_xy_sat(target, reference) {
        return;
    }
    out.extend(target.iter().map(|p| glam::DVec3::new(p.x, p.y, p.z)));
    let reference_points = reference.map(|p| glam::DVec2::new(p.x, p.y));
    let reference_ccw = triangle_xy_area(reference) > 0.0;
    // Sutherland–Hodgman against the 3 reference half-planes. Each iteration
    // may shrink the polyline; bail early if it empties out.
    for edge_index in 0..3 {
        let next = clip_polyline_by_xy_edge(out, reference_points[edge_index], reference_points[(edge_index + 1) % 3], reference_ccw);
        *out = next;
        if out.len() < 3 {
            out.clear();
            return;
        }
    }
}

#[inline(always)]
fn triangles_overlap_xy_sat(a: [mesh_data::Vertex; 3], b: [mesh_data::Vertex; 3]) -> bool {
    for triangle in [a, b] {
        for edge_index in 0..3 {
            if separates_triangles_xy(a, b, triangle[edge_index], triangle[(edge_index + 1) % 3]) {
                return false;
            }
        }
    }

    true
}

#[inline(always)]
fn separates_triangles_xy(a: [mesh_data::Vertex; 3], b: [mesh_data::Vertex; 3], edge_a: mesh_data::Vertex, edge_b: mesh_data::Vertex) -> bool {
    let edge_x = edge_b.x - edge_a.x;
    let edge_y = edge_b.y - edge_a.y;
    let edge_len_sq = edge_x * edge_x + edge_y * edge_y;
    if edge_len_sq <= f64::EPSILON {
        return false;
    }

    let (a_min, a_max) = project_triangle_onto_edge_normal_xy(a, edge_a, edge_x, edge_y);
    let (b_min, b_max) = project_triangle_onto_edge_normal_xy(b, edge_a, edge_x, edge_y);
    let gap = if a_max < b_min {
        b_min - a_max
    } else if b_max < a_min {
        a_min - b_max
    } else {
        return false;
    };

    const XY_TOL_SQ: f64 = crate::model::kernel::XY_TOL * crate::model::kernel::XY_TOL;
    gap * gap > XY_TOL_SQ * edge_len_sq
}

#[inline(always)]
fn project_triangle_onto_edge_normal_xy(triangle: [mesh_data::Vertex; 3], edge_a: mesh_data::Vertex, edge_x: f64, edge_y: f64) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in triangle {
        let projected = edge_x * (point.y - edge_a.y) - edge_y * (point.x - edge_a.x);
        min = min.min(projected);
        max = max.max(projected);
    }
    (min, max)
}

pub(super) fn clip_surface_polyline(polyline: Vec<SurfaceClipVertex>, side: TriSurfaceCutSide) -> Vec<SurfaceClipVertex> {
    if polyline.is_empty() {
        return polyline;
    }
    let retained = |delta: f64| match side {
        TriSurfaceCutSide::CutTop => delta <= 1e-9,
        TriSurfaceCutSide::CutBottom => delta >= -1e-9,
    };

    let mut output = Vec::new();
    let mut previous = *polyline.last().expect("polyline is non-empty");
    let mut previous_inside = retained(previous.height_delta);
    for current in polyline {
        let current_inside = retained(current.height_delta);
        if current_inside != previous_inside {
            let denominator = previous.height_delta - current.height_delta;
            if denominator.abs() > 1e-20 {
                let t = (previous.height_delta / denominator).clamp(0.0, 1.0);
                output.push(SurfaceClipVertex {
                    point: previous.point.lerp(current.point, t),
                    height_delta: 0.0,
                });
            }
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    output
}

pub(super) fn append_surface_clip_polyline(polyline: &[SurfaceClipVertex], vertices: &mut Vec<mesh_data::Vertex>, faces: &mut Vec<[u32; 3]>) {
    append_polyline_fan(polyline.iter().map(|vertex| vertex.point), vertices, faces);
}
