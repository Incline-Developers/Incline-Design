//! The other surface of a seam, hung from its reference surface by the
//! seam's true thickness.
//!
//! True thickness is fitted with the same exact spline as Build Surface and
//! evaluated at every vertex of the reference surface, so the two surfaces
//! share their lattice and their cut and add and subtract node by node. At
//! each vertex it becomes vertical thickness under the reference surface's
//! slope there, read off its own nodes, and the other surface lies that far
//! below a roof or above a floor. Beyond the outline of the points the
//! spline's trend carries on for [`TREND_REACH`] and then holds, so it does
//! not run on past the data [Heckscher & Saunders, "Precision in the dip";
//! Heckscher, Hughes, Saunders & Owen 2024, "Beyond traditional methods"].

use anyhow::Result;
use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    i18n::tr,
    model::{
        drill_hole::ReferenceSide,
        formats::mesh_data,
        grid_surface::GridSurface,
        kernel::{self, PolyContainment},
        progress::Progress,
        rbf::{MERGE_DISTANCE, MERGE_HEIGHT, RbfSurface},
    },
};

/// How far past the points' outline, in metres, the thickness trend carries
/// before it holds: the reach of a surface built without a mask.
pub(crate) const TREND_REACH: f64 = super::OUTLINE_BUFFER;

/// Vertices between cancellation checks.
const CANCEL_STRIDE: usize = 4096;

/// How many clashing pairs a refusal names before it counts the rest.
const CLASH_LINES: usize = 20;

/// One measured thickness as the grid takes it: whose it is, where, and how
/// thick.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThicknessSample {
    pub(crate) name: String,
    pub(crate) at: DVec2,
    pub(crate) thickness: f64,
}

/// The latest thickness points made against a reference surface, kept for
/// its grid.
#[derive(Debug)]
pub(crate) struct ThicknessRunRecord {
    /// The surface's revision when the points were measured.
    pub(crate) revision: u64,
    /// The point layer the run made.
    pub(crate) name: String,
    /// The working section measured, and the side the surface was built on.
    pub(crate) seam: String,
    pub(crate) side: ReferenceSide,
    pub(crate) samples: Vec<ThicknessSample>,
}

/// Why a surface has no thickness run to grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunProblem {
    Missing,
    /// The run was measured against an earlier revision of the surface.
    Stale,
}

/// The run to grid against a surface at `revision`: the latest, and only
/// when it was measured against that revision.
pub(crate) fn current_run(revision: u64, run: Option<&ThicknessRunRecord>) -> Result<&ThicknessRunRecord, RunProblem> {
    match run {
        None => Err(RunProblem::Missing),
        Some(run) if run.revision != revision => Err(RunProblem::Stale),
        Some(run) => Ok(run),
    }
}

/// One node of the grid: the reference surface's height there, the true
/// and vertical thickness, and the other surface's height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SeamNode {
    pub(crate) at: DVec2,
    pub(crate) reference: f64,
    pub(crate) thickness: f64,
    pub(crate) vertical: f64,
    pub(crate) other: f64,
}

/// The other surface and the grids it was made from.
pub(crate) struct SeamGrid {
    pub(crate) vertices: Vec<mesh_data::Vertex>,
    pub(crate) faces: Vec<[u32; 3]>,
    /// The reference surface's nodes, row by row from the lowest y.
    pub(crate) nodes: Vec<SeamNode>,
    /// Nodes where the spline fell below zero thickness, held at zero.
    pub(crate) held: usize,
    /// Nodes further past the points' outline than the trend carries, which
    /// took the thickness held there.
    pub(crate) beyond: usize,
    /// Points the thickness spline passes through, after merging.
    pub(crate) used: usize,
    /// Points merged into a nearby one of the same thickness.
    pub(crate) merged: usize,
}

/// Grid the true thickness through `samples` on the reference surface and
/// hang the other surface from it: below a roof reference, above a floor
/// one. Refuses before fitting when two samples share a place with
/// different thicknesses.
pub(crate) fn seam_grid(reference: &GridSurface, side: ReferenceSide, samples: &[ThicknessSample], cancel: &CancelFlag, progress: &Progress) -> Result<SeamGrid> {
    refuse_clashes(samples)?;
    if samples.len() < super::MINIMUM_POINTS {
        anyhow::bail!(
            "{}",
            tr!(
                "cmd-seam-surface-too-few-points",
                count = samples.len().to_string(),
                minimum = super::MINIMUM_POINTS.to_string()
            )
        );
    }
    let points: Vec<DVec3> = samples.iter().map(|sample| sample.at.extend(sample.thickness)).collect();
    let spline = RbfSurface::fit(&points, cancel, &progress.phase(0.0, 0.3))?;
    // The fit has refused points on one line, so the outline has an area.
    let outline = super::plan_hull(spline.points())?;
    // A node further out than the reach takes the thickness the reach out
    // on the line from the nearest outline point to it.
    let held_at = |at: DVec2| match kernel::point_in_polyline(at, outline.iter().copied()) {
        PolyContainment::Inside | PolyContainment::OnBoundary => None,
        PolyContainment::Outside => {
            let edge = nearest_on_ring(&outline, at);
            let out = at - edge;
            let distance = out.length();
            (distance > TREND_REACH).then(|| edge + out * (TREND_REACH / distance))
        }
    };
    let sign = match side {
        ReferenceSide::Roof => -1.0,
        ReferenceSide::Floor => 1.0,
    };
    let mesh = reference.mesh();
    let given = mesh.vertices();
    let no_memory = || anyhow::anyhow!("{}", tr!("cmd-seam-surface-no-memory"));
    let mut vertices = Vec::new();
    vertices.try_reserve_exact(given.len()).map_err(|_| no_memory())?;
    vertices.extend_from_slice(given);
    // Per vertex: the spline's thickness before it is held at zero, the
    // vertical thickness, and whether it lies past the reach.
    let mut found: Vec<(f64, f64, bool)> = Vec::new();
    found.try_reserve_exact(given.len()).map_err(|_| no_memory())?;
    found.resize(given.len(), (f64::NAN, f64::NAN, false));
    let evaluated = progress.phase(0.3, 0.9).counter(given.len());
    vertices
        .par_chunks_mut(CANCEL_STRIDE)
        .zip(found.par_chunks_mut(CANCEL_STRIDE))
        .try_for_each(|(vertices, found)| -> Result<()> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!("common-cancelled"));
            }
            for (vertex, found) in vertices.iter_mut().zip(found.iter_mut()) {
                let at = DVec2::new(vertex.x, vertex.y);
                let edge = held_at(at);
                let thickness = spline.height(edge.unwrap_or(at));
                let slope = reference.slope(at);
                let vertical = thickness.max(0.0) * (1.0 + slope.x * slope.x + slope.y * slope.y).sqrt();
                vertex.z += sign * vertical;
                *found = (thickness, vertical, edge.is_some());
            }
            evaluated.advance_by(vertices.len());
            Ok(())
        })?;
    let mut nodes: Vec<((usize, usize), SeamNode)> = Vec::new();
    let (mut held, mut beyond) = (0, 0);
    for ((at, other), (given, &(thickness, vertical, outside))) in vertices.iter().map(|vertex| (DVec2::new(vertex.x, vertex.y), vertex.z)).zip(given.iter().zip(&found)) {
        let Some((column, row)) = reference.node_at(at) else {
            continue;
        };
        held += usize::from(thickness < 0.0);
        beyond += usize::from(outside);
        nodes.push((
            (row, column),
            SeamNode {
                at,
                reference: given.z,
                thickness: thickness.max(0.0),
                vertical,
                other,
            },
        ));
    }
    nodes.sort_by_key(|(key, _)| *key);
    let faces = mesh.face_vertex_indices_iter().map(|face| face.map(|index| index as u32)).collect();
    progress.phase(0.9, 1.0).finish();
    Ok(SeamGrid {
        vertices,
        faces,
        nodes: nodes.into_iter().map(|(_, node)| node).collect(),
        held,
        beyond,
        used: spline.point_count(),
        merged: spline.merged(),
    })
}

/// The point of the closed `ring` nearest `at`: its projection onto the
/// nearest edge, or that edge's nearer end.
fn nearest_on_ring(ring: &[DVec2], at: DVec2) -> DVec2 {
    let mut best = (f64::INFINITY, at);
    for (index, &start) in ring.iter().enumerate() {
        let end = ring[(index + 1) % ring.len()];
        let along = end - start;
        let length = along.length_squared();
        let share = if length > 0.0 { ((at - start).dot(along) / length).clamp(0.0, 1.0) } else { 0.0 };
        let near = start + along * share;
        let distance = near.distance_squared(at);
        if distance < best.0 {
            best = (distance, near);
        }
    }
    best.1
}

/// Refuse, naming both, every two samples closer than the merge distance in
/// plan whose thicknesses differ: an exact surface cannot pass through both.
fn refuse_clashes(samples: &[ThicknessSample]) -> Result<()> {
    let mut order: Vec<usize> = (0..samples.len()).collect();
    order.sort_by(|a, b| samples[*a].at.x.total_cmp(&samples[*b].at.x));
    let mut clashes = Vec::new();
    for (place, &first) in order.iter().enumerate() {
        let a = &samples[first];
        for &second in &order[place + 1..] {
            let b = &samples[second];
            if b.at.x - a.at.x >= MERGE_DISTANCE {
                break;
            }
            if a.at.distance(b.at) < MERGE_DISTANCE && (a.thickness - b.thickness).abs() > MERGE_HEIGHT {
                clashes.push((a, b));
            }
        }
    }
    if clashes.is_empty() {
        return Ok(());
    }
    let mut lines = vec![tr!("cmd-seam-surface-clash-heading", count = clashes.len().to_string())];
    lines.extend(clashes.iter().take(CLASH_LINES).map(|(a, b)| {
        tr!(
            "cmd-seam-surface-clash",
            first = a.name.clone(),
            first_thickness = format!("{:.3}", a.thickness),
            second = b.name.clone(),
            second_thickness = format!("{:.3}", b.thickness)
        )
    }));
    if clashes.len() > CLASH_LINES {
        lines.push(tr!("cmd-thickness-points-and-more", more = (clashes.len() - CLASH_LINES).to_string()));
    }
    anyhow::bail!("{}", lines.join("\n"))
}
