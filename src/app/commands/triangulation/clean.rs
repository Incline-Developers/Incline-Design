//! Clean Surface: the repairs a surface needs before anything is built on it.

use std::collections::HashMap;

use anyhow::Result;

use super::{
    cuts::{FoldLayer, reference_xy_overlap_area_tolerance, single_valued_surface},
    session,
    solid_between::{FLAT_HOLE_POINTS, Weld, collapsed, fill_flat_holes, merge_coincident, ring_contains, welded_edge_uses},
};
use crate::{
    app::{
        App,
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        formats::mesh_data,
        geometry::triangle_xy_area,
        progress::{Phase, Progress},
        triangulation::{GeneratedTriangulationLog, TriangulationId},
    },
    ui::state::{CleanFoldKeep, TriSurfaceType},
    userspace_log, userspace_warn,
};

/// Why a hole was left open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HoleReason {
    /// Its rim does not lie in one plane.
    NotFlat,
    /// Its rim has more points than a flat hole is filled to.
    TooManyPoints,
    /// Faces with no plan area were left out of the surface, so no hole is
    /// filled.
    FacesLeftOut,
}

/// A hole the clean left open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct OpenHole {
    /// Why it was left.
    pub(super) reason: HoleReason,
    /// How many edges its rim has.
    pub(super) edges: usize,
    /// The rim's length, end to end.
    pub(super) length: f64,
    /// The middle of its longest edge, a point on the surface to look at.
    pub(super) at: mesh_data::Vertex,
}

/// What a clean changed, and what it left for the user.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct CleanReport {
    /// Points that sat on another and were merged into it.
    pub(super) points_welded: usize,
    /// Faces with two corners at one point, dropped.
    pub(super) collapsed_faces: usize,
    /// Whether folds, overlaps or cracks had to be repaired.
    pub(super) repaired: bool,
    /// Flat holes closed.
    pub(super) holes_filled: usize,
    /// Holes left as they were, each with why.
    pub(super) holes_left: Vec<OpenHole>,
    /// Vertical or degenerate faces (no area in plan), left out of the result.
    pub(super) vertical_faces_left_out: usize,
}

pub(super) struct CleanedSurface {
    pub(super) vertices: Vec<mesh_data::Vertex>,
    pub(super) faces: Vec<[u32; 3]>,
    pub(super) report: CleanReport,
}

/// Repair `mesh`: weld points that coincide, drop faces collapsed to a line,
/// keep one sheet where it folds, and close the flat holes. A hole that is
/// not flat, or whose rim has too many points, is reported, not filled, and
/// the surface's outer edge is never taken for one. Vertical or
/// degenerate faces are left out and counted in the report, and then no hole
/// is filled: each is listed instead.
pub(super) fn clean_mesh(mesh: &mesh_data::Triangulation, keep: FoldLayer, cancel: &CancelFlag, progress: &Phase) -> Result<CleanedSurface> {
    let mut report = CleanReport::default();
    let check = || -> Result<()> {
        if cancel.is_cancelled() {
            anyhow::bail!(crate::app::jobs::Cancelled);
        }
        Ok(())
    };

    let mut vertices = mesh.vertices().to_vec();
    let weld = Weld::of(&vertices);
    let all: Vec<usize> = (0..vertices.len()).collect();
    merge_coincident(&mut vertices, &all, weld);
    check()?;
    progress.set_fraction(0.2);
    let mut kept: HashMap<[u64; 3], u32> = HashMap::new();
    let mut points = Vec::new();
    let mut remap = Vec::with_capacity(vertices.len());
    for vertex in &vertices {
        // Adding zero turns -0.0 into 0.0, so the two are one point.
        let key = [vertex.x, vertex.y, vertex.z].map(|value| (value + 0.0).to_bits());
        remap.push(*kept.entry(key).or_insert_with(|| {
            points.push(*vertex);
            points.len() as u32 - 1
        }));
    }
    report.points_welded = vertices.len() - points.len();
    let mut faces: Vec<[u32; 3]> = mesh.face_indices().map(|face| face.map(|index| remap[index as usize])).collect();
    let before = faces.len();
    faces.retain(|face| !collapsed(&face.map(|index| points[index as usize])));
    report.collapsed_faces = before - faces.len();
    check()?;
    progress.set_fraction(0.3);
    if faces.is_empty() {
        anyhow::bail!(tr!("err-surf-clean-all-collapsed"));
    }

    let welded = mesh_data::Triangulation::from_vertices_and_faces(points, faces)?;
    let (prepared, repaired) = single_valued_surface(&welded, keep)?;
    report.repaired = repaired;
    // The faces with no plan area are left out, counted on the welded input,
    // since a repair is built from the faces with plan area and counts none.
    let tolerance = reference_xy_overlap_area_tolerance(&welded);
    report.vertical_faces_left_out = welded
        .face_indices()
        .filter(|face| triangle_xy_area(face.map(|index| welded.vertices()[index as usize])).abs() <= tolerance)
        .count();
    check()?;
    progress.set_fraction(0.8);
    let vertices = prepared.mesh.vertices().to_vec();
    let mut faces: Vec<[u32; 3]> = prepared.mesh.face_indices().collect();
    let (patches, left) = fill_holes(&vertices, &faces, report.vertical_faces_left_out == 0);
    progress.set_fraction(0.95);
    report.holes_filled = patches.len();
    faces.extend(patches.into_iter().flatten());
    report.holes_left = left;
    let (vertices, faces) = used_points(&vertices, faces);
    check()?;
    Ok(CleanedSurface { vertices, faces, report })
}

/// `faces` on only the points they use, kept in their own order.
fn used_points(vertices: &[mesh_data::Vertex], faces: Vec<[u32; 3]>) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    let mut used = vec![false; vertices.len()];
    for index in faces.iter().flatten() {
        used[*index as usize] = true;
    }
    let mut moved = vec![u32::MAX; vertices.len()];
    let mut kept = Vec::new();
    for (index, vertex) in vertices.iter().enumerate() {
        if used[index] {
            moved[index] = kept.len() as u32;
            kept.push(*vertex);
        }
    }
    let faces = faces.into_iter().map(|face| face.map(|index| moved[index as usize])).collect();
    (kept, faces)
}

/// The patches that close the surface's flat holes, one per hole, and the
/// holes left open. With `fill` false every hole is left open and listed.
fn fill_holes(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]], fill: bool) -> (Vec<Vec<[u32; 3]>>, Vec<OpenHole>) {
    // Each edge one face uses, as that face runs it with its third corner.
    let mut uses: HashMap<(u32, u32), (u32, u32, u32, u32)> = HashMap::new();
    for &[a, b, c] in faces {
        for (from, to, third) in [(a, b, c), (b, c, a), (c, a, b)] {
            if from == to {
                continue;
            }
            uses.entry((from.min(to), from.max(to))).and_modify(|entry| entry.0 += 1).or_insert((1, from, to, third));
        }
    }
    let open: Vec<(u32, u32, u32)> = uses.values().filter(|entry| entry.0 == 1).map(|&(_, from, to, third)| (from, to, third)).collect();
    let mut outgoing: HashMap<u32, Vec<usize>> = HashMap::new();
    for (index, &(from, _, _)) in open.iter().enumerate() {
        outgoing.entry(from).or_default().push(index);
    }
    let mut starts: Vec<usize> = (0..open.len()).collect();
    starts.sort_unstable_by_key(|&index| (open[index].0, open[index].1));

    let mut used = vec![false; open.len()];
    let (mut patches, mut left) = (Vec::new(), Vec::new());
    for start in starts {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut ring = vec![open[start].0];
        let mut ring_edges = vec![start];
        let mut current = start;
        let closed = loop {
            let at = open[current].1;
            if at == open[start].0 {
                break true;
            }
            let Some(&next) = outgoing.get(&at).into_iter().flatten().find(|&&edge| !used[edge]) else {
                break false;
            };
            used[next] = true;
            ring.push(open[next].0);
            ring_edges.push(next);
            current = next;
        };
        if !closed || ring.len() < 3 {
            continue;
        }
        // The surface lies outside a hole's rim and inside its own outer one.
        // The rim's face with the most plan area is judged at its centre: a
        // corner can sit on the ring, where inside and outside are not told
        // apart, and a sliver's centre is all but on it.
        let polygon: Vec<mesh_data::Vertex> = ring.iter().map(|&index| vertices[index as usize]).collect();
        let Some([a, b, c]) = ring_edges
            .iter()
            .map(|&edge| [open[edge].0, open[edge].1, open[edge].2].map(|index| vertices[index as usize]))
            .max_by(|x, y| triangle_xy_area(*x).abs().total_cmp(&triangle_xy_area(*y).abs()))
        else {
            continue;
        };
        let centre = mesh_data::Vertex::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0, (a.z + b.z + c.z) / 3.0);
        if ring_contains(&polygon, centre) {
            continue;
        }
        if !fill {
            left.push(open_hole(&polygon, HoleReason::FacesLeftOut));
        } else if !rim_is_flat(&polygon) {
            left.push(open_hole(&polygon, HoleReason::NotFlat));
        } else if ring.len() > FLAT_HOLE_POINTS {
            left.push(open_hole(&polygon, HoleReason::TooManyPoints));
        } else {
            let patch = fill_ring(vertices, &ring);
            if patch.is_empty() {
                left.push(open_hole(&polygon, HoleReason::NotFlat));
            } else {
                patches.push(patch);
            }
        }
    }
    (patches, left)
}

/// How far a rim's points may stand off its plane and still be one, as
/// [`fill_flat_holes`] takes it.
const FLAT_RIM: f64 = 1.0e-4;

/// Whether `rim` lies in one plane (Newell's normal), as [`fill_flat_holes`]
/// judges it, for a rim too long for that function to look at.
fn rim_is_flat(rim: &[mesh_data::Vertex]) -> bool {
    let point = |vertex: &mesh_data::Vertex| glam::DVec3::new(vertex.x, vertex.y, vertex.z);
    let centre = rim.iter().map(point).sum::<glam::DVec3>() / rim.len() as f64;
    let normal = (0..rim.len())
        .map(|index| (point(&rim[index]) - centre).cross(point(&rim[(index + 1) % rim.len()]) - centre))
        .sum::<glam::DVec3>();
    normal
        .try_normalize()
        .is_none_or(|normal| rim.iter().all(|vertex| (point(vertex) - centre).dot(normal).abs() <= FLAT_RIM))
}

/// Close one hole by [`fill_flat_holes`], or nothing when it is not flat.
///
/// That function reads the open edges of a mesh, so it is given a cone over
/// the rim, run the way the surface runs it, whose only open edges are the
/// rim's.
fn fill_ring(vertices: &[mesh_data::Vertex], ring: &[u32]) -> Vec<[u32; 3]> {
    let mut local: Vec<mesh_data::Vertex> = ring.iter().map(|&index| vertices[index as usize]).collect();
    let count = local.len() as f64;
    let (x, y, z) = local.iter().fold((0.0, 0.0, 0.0), |sum, vertex| (sum.0 + vertex.x, sum.1 + vertex.y, sum.2 + vertex.z));
    let apex = local.len() as u32;
    local.push(mesh_data::Vertex::new(x / count, y / count, z / count + 1.0));
    let cone: Vec<[u32; 3]> = (0..apex).map(|index| [index, (index + 1) % apex, apex]).collect();
    fill_flat_holes(&local, &welded_edge_uses(&local, &cone))
        .into_iter()
        .map(|face| face.map(|index| ring[index as usize]))
        .collect()
}

fn open_hole(polygon: &[mesh_data::Vertex], reason: HoleReason) -> OpenHole {
    let edges: Vec<[mesh_data::Vertex; 2]> = polygon.iter().zip(polygon.iter().cycle().skip(1)).map(|(&a, &b)| [a, b]).collect();
    let length = |[a, b]: &[mesh_data::Vertex; 2]| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let longest = edges.iter().max_by(|x, y| length(x).total_cmp(&length(y))).copied().unwrap_or([polygon[0]; 2]);
    OpenHole {
        reason,
        edges: edges.len(),
        length: edges.iter().map(length).sum(),
        at: mesh_data::Vertex::new(
            (longest[0].x + longest[1].x) / 2.0,
            (longest[0].y + longest[1].y) / 2.0,
            (longest[0].z + longest[1].z) / 2.0,
        ),
    }
}

impl App<'_> {
    /// Repair a surface into a new surface: points a hair apart welded, faces
    /// collapsed to a line dropped, one sheet kept where it folds, flat holes
    /// closed. Vertical or degenerate faces are left out. Holes left open and
    /// the faces left out are logged for the user to find.
    pub(crate) fn clean_triangulation(&mut self, id: TriangulationId, keep: CleanFoldKeep, name: String) -> Result<()> {
        let (mesh, source_name, quality) = {
            let source = self
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == id && triangulation.state.loaded)
                .ok_or_else(|| anyhow::anyhow!(tr!("err-surf-clean-not-found")))?;
            (source.mesh.clone(), source.name.clone(), source.quality.clone())
        };
        let layer = match keep {
            CleanFoldKeep::Upper => FoldLayer::Highest,
            CleanFoldKeep::Lower => FoldLayer::Lowest,
        };
        // The holes left open travel beside the surface, to be logged once it
        // is in.
        let surface = source_name.clone();
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<(GeneratedTriangulationLog, Vec<OpenHole>, usize)> {
            let CleanedSurface { vertices, faces, report } = clean_mesh(&mesh, layer, cancel, &progress.phase(0.0, 0.9))?;
            let generated = session::build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, |_| Vec::new())?.with_quality(quality);
            progress.set_fraction(0.95);
            let message = tr!(
                "cmd-clean-surface-cleaned",
                surface = source_name.to_string(),
                welded = report.points_welded.to_string(),
                collapsed = report.collapsed_faces.to_string(),
                repaired = if report.repaired { "yes" } else { "no" }.to_string(),
                filled = report.holes_filled.to_string(),
                left = report.holes_left.len().to_string()
            );
            Ok((GeneratedTriangulationLog { generated, message }, report.holes_left, report.vertical_faces_left_out))
        };
        let apply = move |app: &mut App, result: Result<(GeneratedTriangulationLog, Vec<OpenHole>, usize)>| {
            let (result, holes, vertical) = match result {
                Ok((log, holes, vertical)) => (Ok(log), holes, vertical),
                Err(error) => (Err(error), Vec::new(), 0),
            };
            let made = result.is_ok();
            app.apply_generated_triangulation_job(result, &[]);
            if made {
                if vertical > 0 {
                    userspace_warn!("{}", tr!("cmd-cuts-ignored-vertical-faces-of", surface = surface.clone(), count = vertical.to_string()));
                }
                for hole in &holes {
                    let (edges, length) = (hole.edges.to_string(), format!("{:.2}", hole.length));
                    let (x, y, z) = (format!("{:.2}", hole.at.x), format!("{:.2}", hole.at.y), format!("{:.2}", hole.at.z));
                    let message = match hole.reason {
                        HoleReason::NotFlat => tr!("cmd-clean-surface-hole-left", edges = edges, length = length, x = x, y = y, z = z),
                        HoleReason::TooManyPoints => tr!(
                            "cmd-clean-surface-hole-too-long",
                            most = FLAT_HOLE_POINTS.to_string(),
                            edges = edges,
                            length = length,
                            x = x,
                            y = y,
                            z = z
                        ),
                        HoleReason::FacesLeftOut => tr!("cmd-clean-surface-hole-faces-left-out", edges = edges, length = length, x = x, y = y, z = z),
                    };
                    userspace_log!("{}", message);
                }
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-clean-surface-cleaning"), vec![JobKey::Triangulation(id)], compute, apply);
        Ok(())
    }
}
