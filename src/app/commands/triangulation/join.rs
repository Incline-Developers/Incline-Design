//! Join Surfaces: several surfaces as one.
//!
//! Made for surfaces that already fit together, such as a topography cut back
//! to a pit shell and the pit design trimmed to it: their pieces meet edge to
//! edge, and joined, the points they share along the seam become one, so the
//! result is a single continuous surface rather than parts lying side by
//! side. Surfaces that overlap in plan are joined as they are, and the
//! overlap is reported: setting a pit or dump into a topography, where one
//! replaces the other, is Merge Shell into Topography's work.

use rayon::prelude::*;

use super::{
    cuts::{clip_target_triangle_to_reference_xy_exact_into, triangle_xy_bounds},
    *,
};
use crate::model::geometry::{signed_area_xy, triangle_xy_area};

/// The overlap below which two surfaces are taken to meet rather than to
/// overlap: rounding along a shared seam leaves slivers, and the smallest
/// patch worth a warning is far larger than this.
const OVERLAP_IGNORED_SQUARE_METRES: f64 = 1.0;
/// The same, as a share of the smaller surface's plan area.
const OVERLAP_IGNORED_SHARE: f64 = 1.0e-4;

/// One surface being joined, snapshotted on the UI thread.
struct JoinSource {
    name: String,
    mesh: std::sync::Arc<mesh_data::Triangulation>,
    spatial: std::sync::Arc<crate::model::spatial::TriangleBvh>,
}

/// Two surfaces that overlap in plan, and over how much.
struct Overlap {
    first: usize,
    second: usize,
    area: f64,
}

/// The joined surface, and what the log says about it.
struct JoinOutput {
    generated: crate::model::triangulation::GeneratedTriangulation,
    names: Vec<String>,
    /// How far the surfaces meet, along the edges they now share.
    seam_length: f64,
    overlaps: Vec<Overlap>,
}

impl App<'_> {
    /// Join `surface_ids` into one new surface named `name`, sharing the
    /// points where they meet; with `unload_sources`, the surfaces joined are
    /// unloaded in the same undo step.
    pub(crate) fn join_surfaces(&mut self, surface_ids: Vec<TriangulationId>, name: String, unload_sources: bool) -> Result<()> {
        let mut ids: Vec<TriangulationId> = Vec::with_capacity(surface_ids.len());
        for id in surface_ids {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.len() < 2 {
            anyhow::bail!(tr!("err-tri-join-two-surfaces"));
        }
        let sources = ids
            .iter()
            .map(|id| {
                let surface = self
                    .triangulations
                    .iter()
                    .find(|surface| surface.id == *id)
                    .ok_or_else(|| anyhow::anyhow!(tr!("err-tri-triangulation-not-found")))?;
                if !surface.state.loaded || surface.state.deferred.is_some() {
                    anyhow::bail!(tr!("err-tri-join-surface-not-loaded", name = surface.name.clone()));
                }
                Ok(JoinSource {
                    name: surface.name.clone(),
                    mesh: surface.mesh.clone(),
                    spatial: surface.spatial.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let keys = ids.iter().map(|id| crate::app::jobs::JobKey::Triangulation(*id)).collect();
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<JoinOutput> {
            let output = join_sources(&sources, name, cancel, progress)?;
            if cancel.is_cancelled() {
                anyhow::bail!(crate::app::jobs::Cancelled);
            }
            Ok(output)
        };
        let apply = move |app: &mut App, result: Result<JoinOutput>| match result {
            Ok(output) => {
                let mesh = &output.generated.mesh;
                let (vertices, faces) = (mesh.vertex_count().to_string(), mesh.face_count().to_string());
                let surfaces = output.names.len().to_string();
                if output.seam_length > 0.0 {
                    userspace_log!(
                        "{}",
                        tr!(
                            "cmd-join-joined-surfaces",
                            count = surfaces,
                            name = output.generated.name.clone(),
                            vertices = vertices,
                            faces = faces,
                            seam = format_length(output.seam_length)
                        )
                    );
                } else {
                    userspace_log!(
                        "{}",
                        tr!(
                            "cmd-join-joined-surfaces-apart",
                            count = surfaces,
                            name = output.generated.name.clone(),
                            vertices = vertices,
                            faces = faces
                        )
                    );
                }
                for overlap in &output.overlaps {
                    userspace_warn!(
                        "{}",
                        tr!(
                            "cmd-join-surfaces-overlap",
                            first = output.names[overlap.first].clone(),
                            second = output.names[overlap.second].clone(),
                            area = format!("{:.0}", overlap.area)
                        )
                    );
                }
                let unload = if unload_sources { app.unload_triangulation_commands(&ids) } else { Vec::new() };
                let section = SectionKind::natural_for(MemberKind::Triangulation);
                app.add_generated_triangulations(vec![session::new_triangulation(output.generated, section)], unload, true);
            }
            Err(error) => {
                crate::userspace_error!("{}", tr!("cmd-join-failed", message = format!("{error:#}")));
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-join-joining-surfaces"), keys, compute, apply);
        Ok(())
    }
}

/// The worker half: every source's faces in one mesh with the points they
/// share joined, how far they meet, and where they overlap.
fn join_sources(sources: &[JoinSource], name: String, cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress) -> Result<JoinOutput> {
    // Where each source's faces start, in the joined face list.
    let mut face_starts = Vec::with_capacity(sources.len() + 1);
    let mut vertices = Vec::with_capacity(sources.iter().map(|source| source.mesh.vertex_count()).sum());
    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(sources.iter().map(|source| source.mesh.face_count()).sum());
    for source in sources {
        face_starts.push(faces.len());
        let base = u32::try_from(vertices.len()).map_err(|_| anyhow::anyhow!(tr!("err-tri-join-too-large")))?;
        vertices.extend_from_slice(source.mesh.vertices());
        faces.extend(source.mesh.face_vertex_indices_iter().map(|face| face.map(|index| base + index as u32)));
    }
    face_starts.push(faces.len());
    if u32::try_from(vertices.len()).is_err() {
        anyhow::bail!(tr!("err-tri-join-too-large"));
    }
    progress.set_fraction(0.1);

    let (vertices, faces) = crate::model::triangulation::join_near_points(vertices, faces);
    if cancel.is_cancelled() {
        anyhow::bail!(crate::app::jobs::Cancelled);
    }
    progress.set_fraction(0.35);
    let seam_length = seam_length(&vertices, &faces, &face_starts);
    progress.set_fraction(0.5);
    let overlaps = overlaps(sources, cancel, &progress.phase(0.5, 0.8));
    if cancel.is_cancelled() {
        anyhow::bail!(crate::app::jobs::Cancelled);
    }

    let (vertices, faces) = crate::model::triangulation::drop_collapsed_faces(vertices, faces);
    let generated = session::build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, |_| Vec::new())?;
    Ok(JoinOutput {
        generated,
        names: sources.iter().map(|source| source.name.clone()).collect(),
        seam_length,
        overlaps,
    })
}

/// How long the edges are that faces from two different sources share: where
/// the surfaces were joined.
fn seam_length(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]], face_starts: &[usize]) -> f64 {
    let source_of = |face: usize| face_starts.partition_point(|start| *start <= face) - 1;
    // Each side of each face as (edge, source), sorted so the faces sharing
    // an edge come together.
    let mut sides: Vec<(u64, u32)> = faces
        .par_iter()
        .enumerate()
        .flat_map_iter(|(index, face)| {
            let source = source_of(index) as u32;
            (0..3).filter_map(move |corner| {
                let (a, b) = (face[corner], face[(corner + 1) % 3]);
                (a != b).then(|| ((u64::from(a.min(b)) << 32) | u64::from(a.max(b)), source))
            })
        })
        .collect();
    sides.par_sort_unstable();
    sides
        .chunk_by(|left, right| left.0 == right.0)
        .filter(|run| run.iter().any(|side| side.1 != run[0].1))
        .map(|run| {
            let (a, b) = (vertices[(run[0].0 >> 32) as usize], vertices[(run[0].0 & u64::from(u32::MAX)) as usize]);
            ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
        })
        .sum()
}

/// Each pair of sources that overlap in plan by more than slivers along a
/// seam, with the area they share. The smaller of each pair is walked over
/// the larger's index.
fn overlaps(sources: &[JoinSource], cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Phase) -> Vec<Overlap> {
    let plan_areas: Vec<f64> = sources.iter().map(|source| plan_area(&source.mesh)).collect();
    let pairs: Vec<(usize, usize)> = (0..sources.len()).flat_map(|first| (first + 1..sources.len()).map(move |second| (first, second))).collect();
    let walked = progress.counter(pairs.len());
    let mut found = Vec::new();
    for (first, second) in pairs {
        if cancel.is_cancelled() {
            break;
        }
        let (small, large) = if sources[first].mesh.face_count() <= sources[second].mesh.face_count() {
            (&sources[first], &sources[second])
        } else {
            (&sources[second], &sources[first])
        };
        let area = overlap_area(small, large);
        let ignored = OVERLAP_IGNORED_SQUARE_METRES.max(OVERLAP_IGNORED_SHARE * plan_areas[first].min(plan_areas[second]));
        if area > ignored {
            found.push(Overlap { first, second, area });
        }
        walked.advance_by(1);
    }
    found
}

/// The plan area two surfaces share, as the sum of their faces' overlaps.
fn overlap_area(small: &JoinSource, large: &JoinSource) -> f64 {
    let small_vertices = small.mesh.vertices();
    let large_vertices = large.mesh.vertices();
    let large_faces: Vec<[usize; 3]> = large.mesh.face_vertex_indices_iter().collect();
    let small_faces: Vec<[usize; 3]> = small.mesh.face_vertex_indices_iter().collect();
    small_faces
        .par_iter()
        .map_init(
            || (Vec::new(), Vec::new()),
            |(stack, overlap), face| {
                let target = face.map(|index| small_vertices[index]);
                if triangle_xy_area(target) == 0.0 {
                    return 0.0;
                }
                let bounds = triangle_xy_bounds(target);
                let mut area = 0.0;
                large.spatial.for_each_xy_bounds_candidate_index_with_stack(bounds.0, bounds.1, stack, |index| {
                    let Some(face) = large_faces.get(index) else {
                        return;
                    };
                    let reference = face.map(|corner| large_vertices[corner]);
                    // Exact half-planes, as the cuts use: faces either side of a
                    // seam share no area at all, where the tolerant clip would
                    // find a sliver the tolerance wide all along it.
                    clip_target_triangle_to_reference_xy_exact_into(target, reference, overlap);
                    if overlap.len() >= 3 {
                        area += signed_area_xy(overlap).abs();
                    }
                });
                area
            },
        )
        .sum()
}

/// A surface's area seen from above.
fn plan_area(mesh: &mesh_data::Triangulation) -> f64 {
    let vertices = mesh.vertices();
    let faces: Vec<[usize; 3]> = mesh.face_vertex_indices_iter().collect();
    faces.par_iter().map(|face| triangle_xy_area(face.map(|index| vertices[index])).abs()).sum()
}

/// A length for the log: metres below a kilometre, kilometres above.
fn format_length(metres: f64) -> String {
    if metres < 1000.0 {
        format!("{metres:.1} m")
    } else {
        format!("{:.2} km", metres / 1000.0)
    }
}
