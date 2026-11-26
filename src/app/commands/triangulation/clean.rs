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
    /// A separate piece of the surface lies inside it.
    Island,
    /// Its edges do not close into one loop.
    Unclosed,
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

    let (points, remap) = welded_points(mesh.vertices());
    check()?;
    progress.set_fraction(0.2);
    // Only points a face uses count: a stray copy no face uses is just left
    // out.
    let mut used = vec![false; remap.len()];
    let mut used_welded = vec![false; points.len()];
    for index in mesh.face_indices().flatten() {
        used[index as usize] = true;
        used_welded[remap[index as usize] as usize] = true;
    }
    report.points_welded = used.iter().filter(|&&used| used).count() - used_welded.iter().filter(|&&used| used).count();
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
    // The faces with no plan area are left out, counted on the welded input,
    // since a repair is built from the faces with plan area and counts none.
    let tolerance = reference_xy_overlap_area_tolerance(&welded);
    report.vertical_faces_left_out = welded
        .face_indices()
        .filter(|face| triangle_xy_area(face.map(|index| welded.vertices()[index as usize])).abs() <= tolerance)
        .count();
    if report.vertical_faces_left_out == welded.face_count() {
        anyhow::bail!(tr!("err-surf-clean-all-vertical"));
    }
    let (prepared, repaired) = single_valued_surface(&welded, keep)?;
    report.repaired = repaired;
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

/// The points of `vertices` with coincident ones welded, and for each vertex
/// the index of its point.
pub(super) fn welded_points(vertices: &[mesh_data::Vertex]) -> (Vec<mesh_data::Vertex>, Vec<u32>) {
    let mut vertices = vertices.to_vec();
    let weld = Weld::of(&vertices);
    let all: Vec<usize> = (0..vertices.len()).collect();
    merge_coincident(&mut vertices, &all, weld);
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
    (points, remap)
}

/// `faces` on only the points they use, kept in their own order.
pub(super) fn used_points(vertices: &[mesh_data::Vertex], faces: Vec<[u32; 3]>) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
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

/// An edge only one face uses, run with that face on its left in plan.
#[derive(Clone, Copy)]
struct OpenEdge {
    from: u32,
    to: u32,
    /// Whether the face runs it this way too.
    forward: bool,
}

/// The patches that close the surface's flat holes, one per hole, and the
/// holes left open. With `fill` false every hole is left open and listed.
///
/// Each open edge runs with its face on its left in plan, whichever way the
/// face is wound, so an outer rim runs anticlockwise and a hole clockwise.
/// At a point where rims meet, the walk turns into the gap it is tracing,
/// so two holes that share a point are two holes, found the same way on
/// every run. A hole with any of the surface inside it is never lidded.
fn fill_holes(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]], fill: bool) -> (Vec<Vec<[u32; 3]>>, Vec<OpenHole>) {
    // Every face's run of each of its edges, low and high corner first, then
    // as the face runs it with its third corner: an edge one face runs alone
    // is open.
    let mut sides: Vec<[u32; 5]> = faces
        .iter()
        .flat_map(|&[a, b, c]| [(a, b, c), (b, c, a), (c, a, b)])
        .filter(|(from, to, _)| from != to)
        .map(|(from, to, third)| [from.min(to), from.max(to), from, to, third])
        .collect();
    sides.sort_unstable_by_key(|side| (side[0], side[1]));
    let point = |index: u32| vertices[index as usize];
    let up = |[a, b, c]: [mesh_data::Vertex; 3]| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    // How most faces are wound, for the faces whose own side cannot be told.
    let clockwise = 2 * faces.iter().filter(|face| up(face.map(point)) < 0.0).count() > faces.len();
    let open: Vec<OpenEdge> = sides
        .chunk_by(|a, b| a[..2] == b[..2])
        .filter(|run| run.len() == 1)
        .map(|run| {
            let [_, _, from, to, third] = run[0];
            let (a, b) = (point(from), point(to));
            let (bx, by) = (b.x - a.x, b.y - a.y);
            let side = up([a, b, point(third)]);
            // A face with next to no plan area says nothing of its side, so
            // it is taken to lie the way most faces are wound.
            let flip = if side.abs() > 1.0e-9 * (bx * bx + by * by) { side < 0.0 } else { clockwise };
            if flip {
                OpenEdge {
                    from: to,
                    to: from,
                    forward: false,
                }
            } else {
                OpenEdge { from, to, forward: true }
            }
        })
        .collect();
    let mut order: Vec<usize> = (0..open.len()).collect();
    order.sort_unstable_by_key(|&edge| (open[edge].from, open[edge].to));
    let leaving = |at: u32| {
        let first = order.partition_point(|&edge| open[edge].from < at);
        let last = order.partition_point(|&edge| open[edge].from <= at);
        &order[first..last]
    };
    // Arriving along `edge`, the edge that turns least anticlockwise from
    // the way back: the one bounding the gap on the walk's right.
    let next = |edge: usize| -> Option<usize> {
        let OpenEdge { from: back, to: at, .. } = open[edge];
        let (here, back) = (point(at), point(back));
        let behind = (back.y - here.y).atan2(back.x - here.x);
        let turn = |edge: usize| {
            let to = point(open[edge].to);
            let turn = ((to.y - here.y).atan2(to.x - here.x) - behind).rem_euclid(std::f64::consts::TAU);
            if turn > 0.0 { turn } else { std::f64::consts::TAU }
        };
        leaving(at).iter().copied().min_by(|&x, &y| turn(x).total_cmp(&turn(y)).then(open[x].to.cmp(&open[y].to)))
    };

    // Face middles in x order, to find what lies inside a hole; made only if
    // a hole is ever looked into.
    let mut middles: Option<Vec<(f64, f64)>> = None;
    let mut used = vec![false; open.len()];
    let (mut patches, mut left) = (Vec::new(), Vec::new());
    for &start in &order {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut ring_edges = vec![start];
        let closed = loop {
            let Some(edge) = next(ring_edges[ring_edges.len() - 1]) else {
                break false;
            };
            if edge == start {
                break true;
            }
            if used[edge] {
                break false;
            }
            used[edge] = true;
            ring_edges.push(edge);
        };
        let rim: Vec<[mesh_data::Vertex; 2]> = ring_edges.iter().map(|&edge| [point(open[edge].from), point(open[edge].to)]).collect();
        if !closed {
            left.push(open_hole(&rim, HoleReason::Unclosed));
            continue;
        }
        if ring_edges.len() < 3 {
            continue;
        }
        let ring: Vec<u32> = ring_edges.iter().map(|&edge| open[edge].from).collect();
        let polygon: Vec<mesh_data::Vertex> = ring.iter().map(|&index| point(index)).collect();
        // Twice the signed area, taken from the first point so that nothing
        // is lost far from the origin: an outer rim is anticlockwise.
        let first = polygon[0];
        let (area, perimeter) = polygon.iter().zip(polygon.iter().cycle().skip(1)).fold((0.0, 0.0), |(area, perimeter), (a, b)| {
            let cross = (a.x - first.x) * (b.y - first.y) - (a.y - first.y) * (b.x - first.x);
            (area + cross, perimeter + (b.x - a.x).hypot(b.y - a.y))
        });
        if area > 1.0e-12 * perimeter * perimeter {
            continue;
        }
        if !fill {
            left.push(open_hole(&rim, HoleReason::FacesLeftOut));
            continue;
        }
        if !rim_is_flat(&polygon) {
            left.push(open_hole(&rim, HoleReason::NotFlat));
            continue;
        }
        if ring.len() > FLAT_HOLE_POINTS {
            left.push(open_hole(&rim, HoleReason::TooManyPoints));
            continue;
        }
        // A rim through one point twice runs round a piece of the surface
        // that touches it there.
        let mut corners = ring.clone();
        corners.sort_unstable();
        let pinched = corners.windows(2).any(|pair| pair[0] == pair[1]);
        let middles = middles.get_or_insert_with(|| {
            let mut middles: Vec<(f64, f64)> = faces
                .iter()
                .map(|face| {
                    let [a, b, c] = face.map(point);
                    ((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0)
                })
                .collect();
            middles.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            middles
        });
        let (low, high) = polygon
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), vertex| (low.min(vertex.y), high.max(vertex.y)));
        let (west, east) = polygon
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(west, east), vertex| (west.min(vertex.x), east.max(vertex.x)));
        let from = middles.partition_point(|middle| middle.0 < west);
        let holds_surface = pinched
            || middles[from..]
                .iter()
                .take_while(|middle| middle.0 <= east)
                .any(|&(x, y)| (low..=high).contains(&y) && ring_contains(&polygon, mesh_data::Vertex::new(x, y, 0.0)));
        if holds_surface {
            left.push(open_hole(&rim, HoleReason::Island));
        } else {
            // The patch runs each rim edge against the face beside it, so it
            // faces the way most of those faces do.
            let forward = ring_edges.iter().filter(|&&edge| open[edge].forward).count();
            let ring: Vec<u32> = if 2 * forward >= ring_edges.len() { ring } else { ring.into_iter().rev().collect() };
            let patch = fill_ring(vertices, &ring);
            if patch.is_empty() {
                left.push(open_hole(&rim, HoleReason::NotFlat));
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

/// A hole left open, from its rim's edges.
fn open_hole(rim: &[[mesh_data::Vertex; 2]], reason: HoleReason) -> OpenHole {
    let length = |[a, b]: &[mesh_data::Vertex; 2]| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let longest = rim
        .iter()
        .max_by(|x, y| length(x).total_cmp(&length(y)))
        .copied()
        .unwrap_or([mesh_data::Vertex::new(0.0, 0.0, 0.0); 2]);
    OpenHole {
        reason,
        edges: rim.len(),
        length: rim.iter().map(length).sum(),
        at: mesh_data::Vertex::new(
            (longest[0].x + longest[1].x) / 2.0,
            (longest[0].y + longest[1].y) / 2.0,
            (longest[0].z + longest[1].z) / 2.0,
        ),
    }
}

/// What is logged when a clean leaves the surface as it was.
fn unchanged_message(surface: &str, holes_left: usize) -> String {
    if holes_left > 0 {
        tr!("cmd-clean-surface-nothing-closed", surface = surface.to_string())
    } else {
        tr!("cmd-clean-surface-nothing-changed", surface = surface.to_string())
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
        // is in. A surface the clean leaves as it was makes no copy.
        let surface = source_name.clone();
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<(Option<GeneratedTriangulationLog>, Vec<OpenHole>, usize)> {
            let CleanedSurface { vertices, faces, report } = clean_mesh(&mesh, layer, cancel, &progress.phase(0.0, 0.9))?;
            if vertices == mesh.vertices() && faces.iter().copied().eq(mesh.face_indices()) {
                return Ok((None, report.holes_left, report.vertical_faces_left_out));
            }
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
            Ok((Some(GeneratedTriangulationLog { generated, message }), report.holes_left, report.vertical_faces_left_out))
        };
        let apply = move |app: &mut App, result: Result<(Option<GeneratedTriangulationLog>, Vec<OpenHole>, usize)>| {
            let (result, holes, vertical) = match result {
                Ok((log, holes, vertical)) => (log.map(Ok), holes, vertical),
                Err(error) => (Some(Err(error)), Vec::new(), 0),
            };
            let made = match result {
                Some(result) => {
                    let made = result.is_ok();
                    app.apply_generated_triangulation_job(result, &[]);
                    made
                }
                None => {
                    userspace_log!("{}", unchanged_message(&surface, holes.len()));
                    true
                }
            };
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
                        HoleReason::Island => tr!("cmd-clean-surface-hole-island", edges = edges, length = length, x = x, y = y, z = z),
                        HoleReason::Unclosed => tr!("cmd-clean-surface-hole-unclosed", edges = edges, length = length, x = x, y = y, z = z),
                    };
                    userspace_log!("{}", message);
                }
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-clean-surface-cleaning"), vec![JobKey::Triangulation(id)], compute, apply);
        Ok(())
    }
}
