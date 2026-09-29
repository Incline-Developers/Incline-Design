//! A thin plate spline fitted in a stretched plan frame, for surfaces that
//! turn quickly across a fold axis and slowly along it.
//!
//! The frame turns the plan so the fold axis runs along its first axis, then
//! multiplies the across-axis coordinate by a ratio of at least one, and
//! [`RbfSurface`] is fitted there unchanged. The same-spot rules are
//! judged in project coordinates first, as [`RbfSurface`] judges them; the
//! stretched distance between points is never less than the plan distance,
//! so the frame finds nothing more to merge. The map is linear, so the fit
//! stays exact at every point. Every transcendental comes from `libm` and no
//! step fuses a multiply-add, so native and wasm builds agree to the bit.

use anyhow::{Context, Result};
use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{
        progress::Phase,
        rbf::{Lattice, MERGE_DISTANCE, MERGE_HEIGHT, POINT_BUDGET, RbfSurface},
    },
};

/// Nodes between cancellation checks and progress reports.
const PROGRESS_STRIDE: usize = 4096;

/// How the plan is stretched: the fold axis as an azimuth in degrees
/// clockwise from north (+y), and the ratio, at least one, by which
/// distances across the axis are multiplied. The azimuth is axial: 0 and 180
/// are the same axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stretch {
    pub(crate) azimuth: f64,
    pub(crate) ratio: f64,
}

/// The map from project plan positions to the stretched frame.
#[derive(Clone, Copy, Debug)]
struct Frame {
    origin: DVec2,
    along: DVec2,
    across: DVec2,
    ratio: f64,
}

impl Frame {
    /// The frame for `stretch` about `origin`. The azimuth is reduced into
    /// [0, 180) first, so an axis and its reverse give one map exactly.
    fn new(stretch: Stretch, origin: DVec2) -> Self {
        let mut azimuth = stretch.azimuth.rem_euclid(180.0);
        if azimuth >= 180.0 {
            azimuth = 0.0;
        }
        let radians = azimuth.to_radians();
        let (sine, cosine) = (libm::sin(radians), libm::cos(radians));
        Self {
            origin,
            along: DVec2::new(sine, cosine),
            across: DVec2::new(cosine, -sine),
            ratio: stretch.ratio,
        }
    }

    /// Along-axis distance from the origin, then the across-axis distance
    /// multiplied by the ratio. The dot products are written out so no
    /// multiply-add can fuse.
    fn map(&self, at: DVec2) -> DVec2 {
        let dx = at.x - self.origin.x;
        let dy = at.y - self.origin.y;
        let along = self.along.x * dx + self.along.y * dy;
        let across = self.across.x * dx + self.across.y * dy;
        DVec2::new(along, self.ratio * across)
    }
}

/// An exact interpolating surface z = f(x, y) whose distances are measured
/// in a stretched frame. Positions are project coordinates throughout.
#[derive(Debug)]
pub(crate) struct AnisotropicSurface {
    frame_surface: RbfSurface,
    frame: Frame,
    /// Merged points in project coordinates, in canonical order.
    points: Vec<DVec3>,
    merged: usize,
}

impl AnisotropicSurface {
    /// Fits the surface through `points`. Input order does not matter:
    /// points are sorted into one canonical order before anything else.
    pub(crate) fn fit(points: &[DVec3], stretch: Stretch, cancel: &CancelFlag, progress: &Phase) -> Result<Self> {
        if !stretch.azimuth.is_finite() {
            anyhow::bail!("The fold axis azimuth must be a finite number of degrees");
        }
        if !stretch.ratio.is_finite() || stretch.ratio < 1.0 {
            anyhow::bail!("The stretch ratio must be a finite number of at least 1, {} given", stretch.ratio);
        }
        if let Some(point) = points.iter().find(|point| !point.is_finite()) {
            anyhow::bail!("Point ({}, {}, {}) is not a finite number", point.x, point.y, point.z);
        }
        let given = points.len();
        let (kept, merged) = merge_in_plan(points)?;
        if kept.len() > POINT_BUDGET {
            anyhow::bail!("{} points after merging ({given} given) exceed the budget of {POINT_BUDGET} for one surface", kept.len());
        }
        let origin = if kept.is_empty() {
            DVec2::ZERO
        } else {
            kept.iter().fold(DVec2::ZERO, |sum, point| sum + point.truncate()) / kept.len() as f64
        };
        let frame = Frame::new(stretch, origin);
        let mut framed = Vec::new();
        framed.try_reserve_exact(kept.len()).context("Not enough memory for the surface points")?;
        framed.extend(kept.iter().map(|point| frame.map(point.truncate()).extend(point.z)));
        let frame_surface = RbfSurface::fit(&framed, cancel, progress)?;
        let merged = merged + frame_surface.merged();
        Ok(Self {
            frame_surface,
            frame,
            points: kept,
            merged,
        })
    }

    /// Points dropped as duplicates of a nearby point at the same height.
    pub(crate) fn merged(&self) -> usize {
        self.merged
    }

    /// Points the surface passes through, after merging.
    pub(crate) fn point_count(&self) -> usize {
        self.points.len()
    }

    /// The points the surface passes through, after merging, in project
    /// coordinates and canonical order: x, then y, then z.
    pub(crate) fn points(&self) -> &[DVec3] {
        &self.points
    }

    pub(crate) fn height(&self, at: DVec2) -> f64 {
        self.frame_surface.height(self.frame.map(at))
    }

    /// Heights at the nodes of `lattice` that `keep` accepts, row by row
    /// from its lowest y; the rest are NaN. Nodes are independent, so the
    /// result does not depend on the thread count or the order they finish
    /// in.
    pub(crate) fn grid_where(&self, lattice: Lattice, keep: impl Fn(DVec2) -> bool + Sync, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<f64>> {
        let count = lattice.node_count();
        let mut heights = Vec::new();
        heights
            .try_reserve_exact(count)
            .with_context(|| format!("Not enough memory to allocate {count} grid nodes"))?;
        heights.resize(count, f64::NAN);
        let task_count = rayon::current_num_threads().saturating_mul(4).max(1);
        let chunk_size = count.div_ceil(task_count).max(1);
        let evaluated = progress.counter(count);
        heights.par_chunks_mut(chunk_size).enumerate().try_for_each(|(chunk_index, chunk)| -> Result<()> {
            let chunk_base = chunk_index * chunk_size;
            for (step, slice) in chunk.chunks_mut(PROGRESS_STRIDE).enumerate() {
                if cancel.is_cancelled() {
                    anyhow::bail!("Cancelled");
                }
                let base = chunk_base + step * PROGRESS_STRIDE;
                for (offset, slot) in slice.iter_mut().enumerate() {
                    let index = base + offset;
                    let node = lattice.node(index % lattice.columns(), index / lattice.columns());
                    if keep(node) {
                        *slot = self.height(node);
                    }
                }
                evaluated.advance_by(slice.len());
            }
            Ok(())
        })?;
        progress.finish();
        Ok(heights)
    }
}

/// x, then y, then z.
fn canonical(a: &DVec3, b: &DVec3) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z))
}

/// Squared plan distance, written out so no multiply-add can fuse.
fn distance_squared(a: DVec2, b: DVec2) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// The same-spot rule of `RbfSurface::fit`, judged in project coordinates:
/// sorts the points by x, then y, then z, and drops each one closer than
/// [`MERGE_DISTANCE`] in plan to one already kept. Refuses a pair that close
/// whose heights differ by more than [`MERGE_HEIGHT`], whether the earlier
/// one was kept or dropped, naming every such pair in the one refusal.
/// Returns the kept points and how many merged. That rule is private to
/// `rbf.rs`, so this mirrors it.
fn merge_in_plan(points: &[DVec3]) -> Result<(Vec<DVec3>, usize)> {
    let mut sorted = Vec::new();
    sorted.try_reserve_exact(points.len()).context("Not enough memory for the surface points")?;
    sorted.extend_from_slice(points);
    sorted.sort_unstable_by(canonical);
    let mut kept: Vec<DVec3> = Vec::new();
    kept.try_reserve_exact(sorted.len()).context("Not enough memory for the surface points")?;
    let mut dropped: Vec<bool> = Vec::new();
    dropped.try_reserve_exact(sorted.len()).context("Not enough memory for the surface points")?;
    let mut merged = 0;
    let mut clashes = Vec::new();
    for (index, point) in sorted.iter().enumerate() {
        let mut duplicate = false;
        let earlier = sorted[..index].iter().zip(&dropped).rev();
        for (other, &other_dropped) in earlier.take_while(|(other, _)| point.x - other.x < MERGE_DISTANCE) {
            if distance_squared(point.truncate(), other.truncate()) >= MERGE_DISTANCE * MERGE_DISTANCE {
                continue;
            }
            if (point.z - other.z).abs() > MERGE_HEIGHT {
                clashes.push(format!(
                    "({:.3}, {:.3}, {:.3}) and ({:.3}, {:.3}, {:.3})",
                    other.x, other.y, other.z, point.x, point.y, point.z
                ));
            }
            duplicate |= !other_dropped;
        }
        dropped.push(duplicate);
        if duplicate {
            merged += 1;
        } else {
            kept.push(*point);
        }
    }
    if !clashes.is_empty() {
        anyhow::bail!(
            "{} pair(s) of points are closer than {MERGE_DISTANCE} m in plan but differ in height:\n{}",
            clashes.len(),
            clashes.join("\n")
        );
    }
    Ok((kept, merged))
}
