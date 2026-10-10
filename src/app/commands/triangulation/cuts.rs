use std::sync::Arc;

use glam::DVec2;
use rayon::prelude::*;

use super::*;
use crate::{
    app::jobs::CancelFlag,
    model::{
        geometry::{clip_polyline_by_xy_edge, signed_area_xy, triangle_xy_area},
        grid_surface::GridSurface,
        progress::Phase,
        spatial::TriangleBvh,
    },
};

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

    /// Clip the seam whose roof and floor are `targets` to the limits
    /// given, Keep below first, into a new roof, floor and solid between
    /// them in the Modelling section, selected when made; the originals stay
    /// as they are. A depth below ground is kept with the project, since it
    /// is the deposit's.
    pub(crate) fn cut_triangulation_to_surface(&mut self, targets: Vec<TriangulationId>, upper: Option<TriUpperCut>, lower: Option<TriLowerCut>) -> Result<()> {
        if seam_targets(&targets).is_none() {
            anyhow::bail!("{}", tr!("cmd-cuts-to-surface-select-seam"));
        }
        if upper.is_none() && lower.is_none() {
            anyhow::bail!("{}", tr!("cmd-cuts-to-surface-no-cut"));
        }
        let (upper_id, upper_level) = match upper {
            Some(TriUpperCut::Surface(id)) => (Some(id), None),
            Some(TriUpperCut::Level(level)) => (None, Some(level)),
            None => (None, None),
        };
        let (lower_id, lower_level) = match lower {
            Some(TriLowerCut::Surface(id) | TriLowerCut::Depth { ground: id, .. }) => (Some(id), None),
            Some(TriLowerCut::Level(level)) => (None, Some(level)),
            None => (None, None),
        };
        if [upper_level, lower_level].into_iter().flatten().any(|level| !level.is_finite()) {
            anyhow::bail!("{}", tr!("tri-clip-to-surface-level-invalid"));
        }
        if [upper_id, lower_id].into_iter().flatten().any(|id| targets.contains(&id)) {
            anyhow::bail!("{}", tr!("cmd-cuts-to-surface-cuts-itself"));
        }
        let depth = match lower {
            Some(TriLowerCut::Depth { depth, .. }) if !(depth.is_finite() && depth > 0.0) => anyhow::bail!("{}", tr!("project-cut-depth-positive")),
            Some(TriLowerCut::Depth { depth, .. }) => Some(depth),
            _ => None,
        };
        let read = |id: TriangulationId| -> Result<(String, Arc<mesh_data::Triangulation>, Arc<TriangleBvh>)> {
            let item = self
                .triangulations
                .iter()
                .find(|item| item.id == id && item.state.loaded)
                .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-thickness-points-surface-gone")))?;
            Ok((item.name.clone(), Arc::clone(&item.mesh), Arc::clone(&item.spatial)))
        };
        let surfaces = targets.iter().map(|&id| read(id)).collect::<Result<Vec<_>>>()?;
        let (upper_read, lower_read) = (upper_id.map(read).transpose()?, lower_id.map(read).transpose()?);
        let cuts = to_surface_cuts_label(
            upper_read.as_ref().map(|read| read.0.as_str()),
            upper_level,
            lower_read.as_ref().map(|read| read.0.as_str()),
            lower_level,
            depth,
        );
        if let Some(depth) = depth {
            let changed = self.workspace.active_project_mut().is_some_and(|project| {
                let modelling = &mut project.project.metadata.modelling;
                let changed = modelling.cut_depth != Some(depth);
                modelling.cut_depth = Some(depth);
                changed
            });
            if changed {
                self.touch_active_project_content();
            }
        }
        let mut keys = Vec::new();
        if let Some(project) = self.workspace.active_project() {
            keys.push(crate::app::jobs::JobKey::Project {
                runtime_id: project.runtime_id,
                document_revision: project.project.document.revision(),
            });
        }
        keys.extend(
            targets
                .iter()
                .chain(upper_id.iter())
                .chain(lower_id.iter())
                .map(|&id| crate::app::jobs::JobKey::Triangulation(id)),
        );
        let section = SectionKind::derived_for(MemberKind::Triangulation, [SectionKind::Modelling]);
        let compute = move |cancel: &CancelFlag, progress: &crate::model::progress::Progress| -> Result<ToSurfaceOutcome> {
            let upper_surface = upper_read.map(|(_, mesh, spatial)| CutReader::new(mesh, spatial)).transpose()?;
            let ground = lower_read.map(|(_, mesh, spatial)| CutReader::new(mesh, spatial)).transpose()?;
            let upper = match (upper_surface.as_ref(), upper_level) {
                (Some(surface), _) => Some(UpperCut::Surface(surface)),
                (None, Some(level)) => Some(UpperCut::Level(level)),
                (None, None) => None,
            };
            let lower = match (ground.as_ref(), depth, lower_level) {
                (Some(ground), Some(depth), _) => Some(LowerCut::Depth { ground, depth }),
                (Some(ground), None, _) => Some(LowerCut::Surface(ground)),
                (None, _, Some(level)) => Some(LowerCut::Level(level)),
                (None, _, None) => None,
            };
            let mut grids = Vec::with_capacity(2);
            for (name, mesh, spatial) in surfaces {
                match GridSurface::read(mesh, spatial) {
                    Ok(grid) => grids.push((name, grid)),
                    Err(problem) => return Ok(ToSurfaceOutcome::Refused(crate::app::commands::thickness_points::not_a_grid(&name, problem))),
                }
            }
            let [(first_name, first), (second_name, second)] = <[_; 2]>::try_from(grids).map_err(|_| anyhow::anyhow!("{}", tr!("cmd-cuts-to-surface-select-seam")))?;
            let mut cut = clip_seam_by_cuts(&first, &second, upper.as_ref(), lower.as_ref(), cancel, &progress.phase(0.0, 0.6))?;
            let (roof_name, floor_name) = if cut.swapped { (second_name, first_name) } else { (first_name, second_name) };
            let seam = tr!("cmd-cuts-to-surface-seam", roof = roof_name.clone(), floor = floor_name.clone());
            if !cut.changed {
                return Ok(ToSurfaceOutcome::NotCut { seam, cut });
            }
            let named = |name: &str, word: String| crate::app::canvas::derived_triangulation_name(name, &word);
            let edges = crate::model::triangulation::unique_edges;
            let faces = std::mem::take(&mut cut.faces);
            let roof = session::build_generated_triangulation(
                named(&roof_name, tr!("common-cut")),
                std::mem::take(&mut cut.roof),
                faces.clone(),
                TriSurfaceType::Surface,
                edges,
            )?;
            let floor = session::build_generated_triangulation(named(&floor_name, tr!("common-cut")), std::mem::take(&mut cut.floor), faces, TriSurfaceType::Surface, edges)?;
            let solid = session::build_generated_triangulation(
                named(&roof_name, tr!("cmd-cuts-to-surface-solid")),
                std::mem::take(&mut cut.solid_vertices),
                std::mem::take(&mut cut.solid_faces),
                TriSurfaceType::SolidClosed,
                edges,
            )?;
            Ok(ToSurfaceOutcome::Made {
                seam,
                generated: Box::new([roof, floor, solid]),
                cut,
            })
        };
        let apply = move |app: &mut App, result: Result<ToSurfaceOutcome>| {
            let outcome = match result {
                Ok(outcome) => outcome,
                Err(error) => {
                    userspace_warn!("{}", tr!("cmd-session-triangulation-failed", message = format!("{error:#}")));
                    return;
                }
            };
            match outcome {
                ToSurfaceOutcome::Made { seam, generated, cut } => {
                    let mut made = Vec::new();
                    for generated in *generated {
                        app.insert_generated_triangulation_in(generated, section);
                        made.extend(app.active_triangulation);
                    }
                    let name_of = |index: usize| {
                        made.get(index)
                            .and_then(|id| app.triangulations.iter().find(|item| item.id == *id))
                            .map(|item| item.name.clone())
                            .unwrap_or_default()
                    };
                    userspace_log!(
                        "{}",
                        tr!(
                            "cmd-cuts-to-surface-made",
                            roof = name_of(0),
                            floor = name_of(1),
                            solid = name_of(2),
                            surface = seam.clone(),
                            upper = cut.to_upper.to_string(),
                            lower = cut.to_lower.to_string(),
                            removed = cut.removed.to_string(),
                            crossed = cut.crossed.to_string(),
                            uncovered = cut.uncovered.to_string(),
                            nodes = cut.nodes.to_string(),
                            volume = format!("{:.0}", cut.volume),
                            cuts = cuts.clone()
                        )
                    );
                    warn_uncovered(&seam, &cut);
                    // The roof, floor and solid become the selection, ready for
                    // the next step.
                    if !made.is_empty() {
                        app.select_only(made.into_iter().map(SceneEntityId::Triangulation));
                        app.invalidate_geometry();
                    }
                }
                ToSurfaceOutcome::NotCut { seam, cut } => {
                    userspace_log!("{}", tr!("cmd-cuts-to-surface-not-cut", surface = seam.clone(), cuts = cuts.clone()));
                    warn_uncovered(&seam, &cut);
                }
                ToSurfaceOutcome::Refused(reason) => userspace_warn!("{}", reason),
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-cuts-to-surface-cutting"), keys, compute, apply);
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
pub(super) fn clip_triangle_z(v: [mesh_data::Vertex; 3], z_min: f64, z_max: f64) -> Vec<[mesh_data::Vertex; 3]> {
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

pub(super) fn lerp_at_z(a: mesh_data::Vertex, b: mesh_data::Vertex, z: f64) -> mesh_data::Vertex {
    if (b.z - a.z).abs() < 1e-12 {
        return a;
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
    let target_vertices = target.vertices();
    let reference_surface = validate_reference_surface(reference)?;
    if reference_surface.skipped_vertical_faces > 0 {
        userspace_warn!("{}", tr!("cmd-cuts-ignored-vertical-faces", count = reference_surface.skipped_vertical_faces.to_string()));
    }

    let mut output_vertices = Vec::new();
    let mut output_faces = Vec::new();

    // One clip per target face, so faces walked is an exact measure.
    let face_count = target.face_count() as u64;
    for (index, face) in target.face_vertex_indices_iter().enumerate() {
        if index.is_multiple_of(4096) {
            progress.set_items(index as u64, face_count);
        }
        let target_triangle = [target_vertices[face[0]], target_vertices[face[1]], target_vertices[face[2]]];
        let target_bounds = triangle_xy_bounds(target_triangle);

        for reference_index in reference_surface
            .spatial
            .xy_bounds_candidate_indices(&reference_surface.mesh, target_bounds.0, target_bounds.1)
        {
            let reference_triangle = reference_surface.triangles[reference_index];
            let overlap = clip_target_triangle_to_reference_xy(target_triangle, reference_triangle);
            if overlap.len() < 3 {
                continue;
            }

            let polyline: Vec<SurfaceClipVertex> = overlap
                .into_iter()
                .map(|point| {
                    let reference_z = bary_z(point.x, point.y, reference_triangle);
                    SurfaceClipVertex {
                        point,
                        height_delta: point.z - reference_z,
                    }
                })
                .collect();
            let clipped = clip_surface_polyline(polyline, side);
            append_surface_clip_polyline(&clipped, &mut output_vertices, &mut output_faces);
        }
    }

    Ok((output_vertices, output_faces))
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

    let overlap_area_tolerance = reference_xy_overlap_area_tolerance(reference);
    for (index, triangle) in prepared.triangles.iter().copied().enumerate() {
        let bounds = triangle_xy_bounds(triangle);
        for other_index in prepared.spatial.xy_bounds_candidate_indices(&prepared.mesh, bounds.0, bounds.1) {
            if other_index <= index {
                continue;
            }
            let overlap = triangle_intersection_xy(triangle, prepared.triangles[other_index]);
            let overlap_area = signed_area_xy(&overlap).abs();
            if overlap_area > overlap_area_tolerance {
                let z_delta = overlap_z_delta(triangle, prepared.triangles[other_index], &overlap);
                anyhow::bail!(
                    "Reference topology overlaps itself in XY and is not single-valued \
                    (triangles {index} and {other_index} overlap by {overlap_area:.6} \
                    square units with up to {z_delta:.6} Z difference)"
                );
            }
        }
    }
    Ok(prepared)
}

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
    add_split_constraints(&mut cdt, conflicting_edges, site, origin);

    let cells: Vec<[glam::DVec2; 3]> = cdt
        .inner_faces()
        .map(|face| {
            face.vertices().map(|vertex| {
                let position = vertex.position();
                glam::DVec2::new(position.x + origin.x, position.y + origin.y)
            })
        })
        .collect();
    Ok(cells)
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
    let vertices = reference.vertices();
    if reference.face_count() == 0 {
        anyhow::bail!("Reference topology contains no faces");
    }

    let xy_area_tolerance = reference_xy_overlap_area_tolerance(reference);
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

/// The roof and floor of one seam, in either order, when `targets` is
/// exactly two different surfaces; anything else is not one seam.
pub(crate) fn seam_targets(targets: &[TriangulationId]) -> Option<[TriangulationId; 2]> {
    match *targets {
        [first, second] if first != second => Some([first, second]),
        _ => None,
    }
}

/// What became of one seam clipped to the cuts.
enum ToSurfaceOutcome {
    Made {
        seam: String,
        /// Roof, floor and solid, in that order.
        generated: Box<[crate::model::triangulation::GeneratedTriangulation; 3]>,
        cut: SeamCut,
    },
    /// No node lay beyond the cuts, so nothing was made.
    NotCut { seam: String, cut: SeamCut },
    /// Not one regular grid, with the reason.
    Refused(String),
}

/// The cuts a run used, for every line it reports.
fn to_surface_cuts_label(upper: Option<&str>, upper_level: Option<f64>, lower: Option<&str>, lower_level: Option<f64>, depth: Option<f64>) -> String {
    let upper = match (upper, upper_level) {
        (Some(name), _) => Some(tr!("cmd-cuts-to-surface-upper", name = name.to_owned())),
        (None, Some(level)) => Some(tr!("cmd-cuts-to-surface-upper-level", level = level.to_string())),
        (None, None) => None,
    };
    let lower = match (lower, depth, lower_level) {
        (Some(name), Some(depth), _) => Some(tr!("cmd-cuts-to-surface-lower-depth", depth = depth.to_string(), name = name.to_owned())),
        (Some(name), None, _) => Some(tr!("cmd-cuts-to-surface-lower", name = name.to_owned())),
        (None, _, Some(level)) => Some(tr!("cmd-cuts-to-surface-lower-level", level = level.to_string())),
        (None, _, None) => None,
    };
    [upper, lower].into_iter().flatten().collect::<Vec<_>>().join(", ")
}

/// Nodes with no cut under them are never passed over in silence.
fn warn_uncovered(seam: &str, cut: &SeamCut) {
    if cut.uncovered > 0 {
        userspace_warn!("{}", tr!("cmd-cuts-to-surface-uncovered", surface = seam.to_owned(), count = cut.uncovered.to_string()));
    }
}

// Clipping a seam to cuts. Unlike the clips above, which remove what lies
// beyond a cutter and re-triangulate along the crossing, a seam's roof and
// floor are read node for node: where only the roof crosses a cut it is laid
// flat on the cut, where both cross the node goes, so roof and floor end
// where they meet on the cut, and the result lines up node for node with
// the surfaces it came from.

/// Vertices between cancellation checks.
const CANCEL_STRIDE: usize = 4096;

/// Per-vertex outcome bits, tallied per node afterwards.
const TO_UPPER: u8 = 1;
const TO_LOWER: u8 = 2;
const UNCOVERED: u8 = 4;
const CROSSED: u8 = 8;

/// A cut surface as [`clip_seam_by_cuts`] reads it: off its lattice when
/// it is a grid, else off the triangle under the point.
pub(super) struct CutReader {
    /// The surface as a lattice, when its triangles are one.
    grid: Option<GridSurface>,
    surface: PreparedReferenceSurface,
}

impl CutReader {
    /// Read `mesh` as a grid when it is one. A triangulated cut must be
    /// single-valued in plan, as Trim to Topology requires of its topology.
    pub(super) fn new(mesh: Arc<mesh_data::Triangulation>, spatial: Arc<TriangleBvh>) -> Result<Self> {
        let grid = GridSurface::read(Arc::clone(&mesh), spatial).ok();
        let surface = match grid {
            // One height per node already, so the overlap check is not needed.
            Some(_) => prepare_reference_surface_relaxed(&mesh)?,
            None => validate_reference_surface(&mesh)?,
        };
        if surface.skipped_vertical_faces > 0 {
            userspace_warn!("{}", tr!("cmd-cuts-ignored-vertical-faces", count = surface.skipped_vertical_faces.to_string()));
        }
        Ok(Self { grid, surface })
    }

    /// The cut's height under `at`, `None` where it does not cover `at` in
    /// plan. On a grid it is read bilinearly; a cell missing a node at the
    /// grid's cut edge falls back to the triangle under `at`.
    pub(super) fn height(&self, at: DVec2) -> Option<f64> {
        match &self.grid {
            Some(grid) if !grid.covers(at) => None,
            Some(grid) => grid.bilinear(at).or_else(|| self.under(at)),
            None => self.under(at),
        }
    }

    /// The height of the triangle under `at`, edges included; the highest
    /// where two share an edge.
    fn under(&self, at: DVec2) -> Option<f64> {
        let mut height: Option<f64> = None;
        self.surface.spatial.for_each_xy_bounds_candidate_index(at, at, |index| {
            if let Some(z) = point_in_triangle_bary_z(at.x, at.y, self.surface.triangles[index]) {
                height = Some(height.map_or(z, |found| found.max(z)));
            }
        });
        height
    }
}

/// The upper cut (Keep below): a surface, or an RL.
pub(super) enum UpperCut<'a> {
    /// A surface cut as it stands.
    Surface(&'a CutReader),
    /// A level, the same height everywhere, so it covers every node.
    Level(f64),
}

impl UpperCut<'_> {
    /// Height of the upper cut under `at`.
    pub(super) fn height(&self, at: DVec2) -> Option<f64> {
        match self {
            UpperCut::Surface(surface) => surface.height(at),
            UpperCut::Level(level) => Some(*level),
        }
    }
}

/// The lower cut (Keep above): a surface, an RL, or a depth below a ground
/// surface.
pub(super) enum LowerCut<'a> {
    /// A surface cut as it stands.
    Surface(&'a CutReader),
    /// A level, the same height everywhere, so it covers every node.
    Level(f64),
    /// The ground surface lowered by `depth` metres.
    Depth { ground: &'a CutReader, depth: f64 },
}

impl LowerCut<'_> {
    /// Height of the lower cut under `at` (ground - depth for Depth).
    pub(super) fn height(&self, at: DVec2) -> Option<f64> {
        match self {
            LowerCut::Surface(surface) => surface.height(at),
            LowerCut::Level(level) => Some(*level),
            LowerCut::Depth { ground, depth } => ground.height(at).map(|height| height - depth),
        }
    }
}

/// Heights a roof and floor node may differ by and still be one point of
/// the solid, in metres.
const SEAM_WELD: f64 = 1e-6;

/// A seam after its cuts: roof and floor on the nodes kept, one set of
/// faces for both, the solid between them, and what the cuts did, counted
/// per lattice node.
pub(super) struct SeamCut {
    pub(super) roof: Vec<mesh_data::Vertex>,
    pub(super) floor: Vec<mesh_data::Vertex>,
    /// Faces of roof and floor alike, anticlockwise in plan.
    pub(super) faces: Vec<[u32; 3]>,
    pub(super) solid_vertices: Vec<mesh_data::Vertex>,
    pub(super) solid_faces: Vec<[u32; 3]>,
    /// Volume between roof and floor, in cubic metres.
    pub(super) volume: f64,
    /// Lattice nodes the roof and floor share.
    pub(super) nodes: usize,
    /// Nodes whose roof was laid flat on the upper cut.
    pub(super) to_upper: usize,
    /// Nodes whose floor was laid flat on the lower cut.
    pub(super) to_lower: usize,
    /// Nodes where roof and floor both lay beyond a cut: gone from both.
    pub(super) removed: usize,
    /// Nodes where a chosen cut (either one) has no height: left as they
    /// were by that cut.
    pub(super) uncovered: usize,
    /// Nodes where the upper cut lies below the lower cut.
    pub(super) crossed: usize,
    /// Whether any node moved or went; false means "not cut", no surface is
    /// made.
    pub(super) changed: bool,
    /// Whether the second surface given is the roof.
    pub(super) swapped: bool,
}

const REMOVED: u8 = 16;

/// Clip the seam whose roof and floor are `first` and `second`, in either
/// order (the roof is the higher on average), to the cuts: per node, Keep
/// below lays the roof flat on the upper cut where only the roof crosses it
/// and removes the node where the floor does too; Keep above mirrors it.
/// Only the nodes and faces the two share are read; a node with no cut
/// under it is left as it was.
pub(super) fn clip_seam_by_cuts(
    first: &GridSurface,
    second: &GridSurface,
    upper: Option<&UpperCut<'_>>,
    lower: Option<&LowerCut<'_>>,
    cancel: &CancelFlag,
    progress: &Phase,
) -> Result<SeamCut> {
    if upper.is_none() && lower.is_none() {
        anyhow::bail!("{}", tr!("cmd-cuts-to-surface-no-cut"));
    }
    if (first.spacing() - second.spacing()).abs() > first.spacing() * 1e-6 {
        anyhow::bail!("{}", tr!("cmd-cuts-to-surface-not-one-lattice"));
    }
    // The faces of the first whose three corners are nodes of both.
    let given = first.mesh().vertices();
    let shared = |index: usize| -> Option<((usize, usize), DVec2, f64)> {
        let at = DVec2::new(given[index].x, given[index].y);
        let node = first.node_at(at)?;
        let other = second.node_at(at).and_then(|(column, row)| second.height(column, row))?;
        Some((node, at, other))
    };
    let no_memory = || anyhow::anyhow!("{}", tr!("cmd-cuts-to-surface-no-memory"));
    let mut slot: std::collections::HashMap<(usize, usize), u32> = std::collections::HashMap::new();
    let (mut plan, mut top, mut bottom, mut faces) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for face in first.mesh().face_vertex_indices_iter() {
        let Some(corners) = face.iter().map(|&index| shared(index)).collect::<Option<Vec<_>>>() else {
            continue;
        };
        let mut indices = [0u32; 3];
        for (corner, (node, at, other)) in indices.iter_mut().zip(corners) {
            *corner = *slot.entry(node).or_insert_with(|| {
                plan.push(at);
                top.push(first.height(node.0, node.1).unwrap_or(f64::NAN));
                bottom.push(other);
                (plan.len() - 1) as u32
            });
        }
        faces.try_reserve(1).map_err(|_| no_memory())?;
        faces.push(indices);
    }
    if faces.is_empty() {
        anyhow::bail!("{}", tr!("cmd-cuts-to-surface-not-one-lattice"));
    }
    let mean_gap = top.iter().zip(&bottom).map(|(a, b)| a - b).sum::<f64>() / top.len() as f64;
    let swapped = mean_gap < 0.0;
    let (mut roof, mut floor) = if swapped { (bottom, top) } else { (top, bottom) };
    let mut flags: Vec<u8> = Vec::new();
    flags.try_reserve_exact(plan.len()).map_err(|_| no_memory())?;
    flags.resize(plan.len(), 0);
    let phase = progress.phase(0.0, 1.0);
    let done = phase.counter(plan.len());
    plan.par_chunks(CANCEL_STRIDE)
        .zip(roof.par_chunks_mut(CANCEL_STRIDE))
        .zip(floor.par_chunks_mut(CANCEL_STRIDE))
        .zip(flags.par_chunks_mut(CANCEL_STRIDE))
        .try_for_each(|(((plan, roof), floor), flags)| -> Result<()> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!("common-cancelled"));
            }
            for (((&at, roof), floor), flag) in plan.iter().zip(roof.iter_mut()).zip(floor.iter_mut()).zip(flags.iter_mut()) {
                let upper_height = upper.and_then(|cut| cut.height(at));
                let lower_height = lower.and_then(|cut| cut.height(at));
                if (upper.is_some() && upper_height.is_none()) || (lower.is_some() && lower_height.is_none()) {
                    *flag |= UNCOVERED;
                }
                // Equality is not a crossing: only a node strictly beyond the
                // cut counts.
                if let Some(cut) = upper_height {
                    if *floor > cut {
                        *flag |= REMOVED;
                    } else if *roof > cut {
                        *roof = cut;
                        *flag |= TO_UPPER;
                    }
                }
                if let Some(cut) = lower_height.filter(|_| *flag & REMOVED == 0) {
                    if *roof < cut {
                        *flag |= REMOVED;
                    } else if *floor < cut {
                        *floor = cut;
                        *flag |= TO_LOWER;
                    }
                }
                if let (Some(top), Some(bottom)) = (upper_height, lower_height)
                    && top < bottom
                {
                    *flag |= CROSSED;
                }
            }
            done.advance_by(plan.len());
            Ok(())
        })?;
    let count = |bit: u8| flags.iter().filter(|&&flag| flag & bit != 0 && (bit == REMOVED || flag & REMOVED == 0)).count();
    let (to_upper, to_lower, removed) = (count(TO_UPPER), count(TO_LOWER), count(REMOVED));
    let (uncovered, crossed) = (
        flags.iter().filter(|&&flag| flag & UNCOVERED != 0).count(),
        flags.iter().filter(|&&flag| flag & CROSSED != 0).count(),
    );
    let changed = to_upper + to_lower + removed > 0;

    // The faces kept, anticlockwise in plan, on the nodes they use.
    let mut renumber = vec![u32::MAX; plan.len()];
    let mut kept_nodes = Vec::new();
    let mut kept = Vec::new();
    for face in faces {
        if face.iter().any(|&index| flags[index as usize] & REMOVED != 0) {
            continue;
        }
        let [a, b, c] = face.map(|index| plan[index as usize]);
        let face = if (b - a).perp_dot(c - a) < 0.0 { [face[0], face[2], face[1]] } else { face };
        kept.push(face.map(|index| {
            let slot = &mut renumber[index as usize];
            if *slot == u32::MAX {
                *slot = kept_nodes.len() as u32;
                kept_nodes.push(index as usize);
            }
            *slot
        }));
    }
    if kept.is_empty() {
        anyhow::bail!("{}", tr!("cmd-cuts-to-surface-nothing-left"));
    }
    let vertex = |index: usize, z: f64| mesh_data::Vertex {
        x: plan[index].x,
        y: plan[index].y,
        z,
    };
    let roof_vertices: Vec<_> = kept_nodes.iter().map(|&index| vertex(index, roof[index])).collect();
    let floor_vertices: Vec<_> = kept_nodes.iter().map(|&index| vertex(index, floor[index])).collect();
    let thickness = |index: u32| (roof_vertices[index as usize].z - floor_vertices[index as usize].z).max(0.0);
    let volume = kept
        .iter()
        .map(|&[a, b, c]| {
            let corner = |index: u32| DVec2::new(roof_vertices[index as usize].x, roof_vertices[index as usize].y);
            let area = 0.5 * (corner(b) - corner(a)).perp_dot(corner(c) - corner(a));
            area * (thickness(a) + thickness(b) + thickness(c)) / 3.0
        })
        .sum();
    let (solid_vertices, solid_faces) = close_seam_solid(&roof_vertices, &floor_vertices, &kept);
    phase.finish();
    Ok(SeamCut {
        roof: roof_vertices,
        floor: floor_vertices,
        faces: kept,
        solid_vertices,
        solid_faces,
        volume,
        nodes: plan.len(),
        to_upper,
        to_lower,
        removed,
        uncovered,
        crossed,
        changed,
        swapped,
    })
}

/// Interim, until a shared solid builder replaces it: builds a closed solid
/// between the roof and floor grids.
///
/// The roof faces are its top and the floor faces, turned over, its bottom;
/// a wall stands on every outer edge of the faces where roof and floor are
/// apart. Where they meet the two share the point, so no wall is needed and
/// faces flat between them are left out.
fn close_seam_solid(roof: &[mesh_data::Vertex], floor: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    let mut vertices = roof.to_vec();
    let below: Vec<u32> = roof
        .iter()
        .zip(floor)
        .enumerate()
        .map(|(index, (top, bottom))| {
            if top.z - bottom.z <= SEAM_WELD {
                index as u32
            } else {
                vertices.push(*bottom);
                (vertices.len() - 1) as u32
            }
        })
        .collect();
    let apart = |index: u32| below[index as usize] != index;
    let mut solid = Vec::new();
    let mut edges: std::collections::HashMap<(u32, u32), (u32, [u32; 2])> = std::collections::HashMap::new();
    for &[a, b, c] in faces {
        for (from, to) in [(a, b), (b, c), (c, a)] {
            edges.entry((from.min(to), from.max(to))).or_insert((0, [from, to])).0 += 1;
        }
        if apart(a) || apart(b) || apart(c) {
            solid.push([a, b, c]);
            solid.push([below[a as usize], below[c as usize], below[b as usize]]);
        }
    }
    let mut outer: Vec<[u32; 2]> = edges.into_values().filter(|(count, _)| *count == 1).map(|(_, edge)| edge).collect();
    outer.sort_unstable();
    for [from, to] in outer {
        let (from_below, to_below) = (below[from as usize], below[to as usize]);
        for wall in [[to, from, from_below], [to, from_below, to_below]] {
            if wall[0] != wall[1] && wall[1] != wall[2] && wall[0] != wall[2] {
                solid.push(wall);
            }
        }
    }
    (vertices, solid)
}
