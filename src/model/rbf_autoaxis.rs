//! Build Surface's auto-axis method: the fold axis read off the points
//! inside one domain, then the stretched spline along it.
//!
//! The first pass is the exact spline through every fitted point.
//! Its curvature, over a lattice laid across the domain, is averaged into
//! one tensor, and the direction of least curvature is the fold axis. A
//! clear axis gets the anisotropic spline with a fixed ratio through
//! every fitted point; anything else keeps the first pass, the exact spline
//! byte for byte. The thresholds are fixed; none is the user's. Every
//! transcendental comes from `libm` and every sum runs in one fixed order,
//! so the axis, and so the surface, is the same for any input order, thread
//! count, native or wasm.

use anyhow::{Context, Result};
use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{
        progress::Phase,
        rbf::{NODE_BUDGET, RbfSurface},
        rbf_anisotropic::{AnisotropicSurface, Stretch},
    },
};

/// The stretch a clear axis gets; no clear axis gets none.
pub(crate) const AXIS_RATIO: f64 = 4.0;

/// No clear axis when the smaller principal curvature is more than this
/// fraction of the larger.
pub(crate) const MOST_CONTRAST: f64 = 0.20;

/// No clear axis when the larger curvature bends a chord across the domain
/// by less than this, in metres: mu1 W² / 8, W the domain's equivalent
/// diameter.
pub(crate) const LEAST_SAG: f64 = 2.0;

/// No clear axis when the fold is drilled at fewer holes than this per
/// wavelength across it.
pub(crate) const LEAST_HOLES_PER_WAVELENGTH: f64 = 5.0;

/// Curvature lattice steps across the domain's equivalent diameter.
const LATTICE_ACROSS: f64 = 40.0;

/// The curvature's central differences step this fraction of the lattice
/// step either side of a node.
const DIFFERENCE_FRACTION: f64 = 0.01;

/// Slack on the contrast and holes comparisons, and on the lattice's reach
/// to the domain's edge.
const TOLERANCE: f64 = 1e-12;
const REACH_SLACK: f64 = 1e-9;

/// Nodes between cancellation checks and progress reports.
const PROGRESS_STRIDE: usize = 64;

/// What the first pass says about the fold axis over one domain, and
/// whether it is clear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisEstimate {
    /// The direction of least curvature in degrees clockwise from grid
    /// north, in [0, 180), and the smaller principal curvature over the
    /// larger; `None` when the domain shows no curvature at all.
    pub(crate) least: Option<(f64, f64)>,
    /// How far, in metres, the larger curvature bends a chord across the
    /// domain's equivalent diameter.
    pub(crate) sag: f64,
    /// The fold's wavelength across the axis over the hole spacing; 0 when
    /// either is undefined.
    pub(crate) holes_per_wavelength: f64,
    /// The domain's equivalent diameter, √(4 area / π), in metres.
    pub(crate) width: f64,
    /// Curvature lattice nodes inside the domain.
    pub(crate) nodes: usize,
    /// Fitted points inside the domain, after merging.
    pub(crate) points_inside: usize,
    /// The axis, when every test says it is clear.
    pub(crate) axis: Option<f64>,
}

/// The spline an auto-axis build delivers.
#[derive(Debug)]
pub(crate) enum AutoAxisSpline {
    /// No clear axis: the first pass itself.
    Exact(RbfSurface),
    /// A clear axis: stretched along it by [`AXIS_RATIO`].
    Stretched(AnisotropicSurface),
}

#[derive(Debug)]
pub(crate) struct AutoAxisSurface {
    pub(crate) spline: AutoAxisSpline,
    pub(crate) estimate: AxisEstimate,
}

impl AutoAxisSurface {
    /// Fits the first pass through `points`, reads the axis over the domain
    /// `ring` bounds (`inside` tells its ground, edge included), and fits
    /// the stretched spline through every point when the axis is clear.
    pub(crate) fn fit(points: &[DVec3], ring: &[DVec2], inside: impl Fn(DVec2) -> bool + Sync, cancel: &CancelFlag, progress: &Phase) -> Result<Self> {
        let first = RbfSurface::fit(points, cancel, &progress.phase(0.0, 0.45))?;
        let estimate = estimate_axis(&first, ring, inside, cancel, &progress.phase(0.45, 0.55))?;
        let spline = match estimate.axis {
            Some(azimuth) => {
                drop(first);
                let stretch = Stretch { azimuth, ratio: AXIS_RATIO };
                AutoAxisSpline::Stretched(AnisotropicSurface::fit(points, stretch, cancel, &progress.phase(0.55, 1.0))?)
            }
            None => AutoAxisSpline::Exact(first),
        };
        progress.finish();
        Ok(Self { spline, estimate })
    }
}

/// The fold axis of `surface` over the domain `ring` bounds. Curvature is
/// read at lattice nodes inside the domain, never at the points, where the
/// spline's second derivatives are unbounded.
pub(crate) fn estimate_axis(surface: &RbfSurface, ring: &[DVec2], inside: impl Fn(DVec2) -> bool + Sync, cancel: &CancelFlag, progress: &Phase) -> Result<AxisEstimate> {
    let (lower, upper) = ring.iter().fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), at| (low.min(*at), high.max(*at)));
    let centre = (lower + upper) * 0.5;
    let area = ring_area(ring, centre);
    let width = (4.0 * area / std::f64::consts::PI).sqrt();
    let points_inside = surface.points().iter().filter(|point| inside(point.truncate())).count();
    let nodes = lattice_nodes(lower, upper, centre, width / LATTICE_ACROSS, &inside, cancel)?;
    let curvature = node_curvature(surface, &nodes.positions, nodes.step * DIFFERENCE_FRACTION, cancel, progress)?;
    let mut estimate = AxisEstimate {
        least: None,
        sag: 0.0,
        holes_per_wavelength: 0.0,
        width,
        nodes: nodes.positions.len(),
        points_inside,
        axis: None,
    };
    let Some(tensor) = strength_weighted_tensor(&curvature) else {
        return Ok(estimate);
    };
    let (larger, smaller) = tensor.principal();
    if larger.is_nan() || larger <= 0.0 {
        return Ok(estimate);
    }
    let azimuth = tensor.least_azimuth();
    let contrast = smaller / larger;
    estimate.least = Some((azimuth, contrast));
    estimate.sag = larger * width * width / 8.0;
    if let Some(wavelength) = wavelength_across(&curvature, azimuth)
        && points_inside > 0
    {
        estimate.holes_per_wavelength = wavelength / (area / points_inside as f64).sqrt();
    }
    let clear = estimate.holes_per_wavelength >= LEAST_HOLES_PER_WAVELENGTH - TOLERANCE && contrast <= MOST_CONTRAST + TOLERANCE && estimate.sag >= LEAST_SAG;
    estimate.axis = clear.then_some(azimuth);
    progress.finish();
    Ok(estimate)
}

/// The area `ring` bounds, by the shoelace formula about `centre`, summed
/// from its lowest vertex in plan order towards the lower of that vertex's
/// neighbours, so where the ring starts and which way it runs cannot change
/// a bit of it.
fn ring_area(ring: &[DVec2], centre: DVec2) -> f64 {
    let count = ring.len();
    if count < 3 {
        return 0.0;
    }
    let plan = |a: &DVec2, b: &DVec2| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y));
    let start = (0..count).min_by(|&a, &b| plan(&ring[a], &ring[b])).unwrap_or(0);
    let (next, previous) = (ring[(start + 1) % count], ring[(start + count - 1) % count]);
    let step = if plan(&next, &previous).is_le() { 1 } else { count - 1 };
    let mut twice = 0.0;
    for index in 0..count {
        let a = ring[(start + index * step) % count] - centre;
        let b = ring[(start + (index + 1) * step) % count] - centre;
        twice += a.x * b.y - b.x * a.y;
    }
    twice.abs() / 2.0
}

/// The curvature lattice's nodes inside the domain, row by row from the
/// lowest y, and the step they were laid at.
struct LatticeNodes {
    positions: Vec<DVec2>,
    step: f64,
}

/// Nodes at `centre` plus whole multiples of `step`, as far as the box
/// reaches either way, kept where `inside` holds. A ring whose box is far
/// larger than its area, a hairline on the diagonal, would ask for more
/// candidates than a grid may hold, so the step doubles until they fit.
fn lattice_nodes(lower: DVec2, upper: DVec2, centre: DVec2, mut step: f64, inside: &(impl Fn(DVec2) -> bool + Sync), cancel: &CancelFlag) -> Result<LatticeNodes> {
    if !(step.is_finite() && step > 0.0) {
        return Ok(LatticeNodes { positions: Vec::new(), step: 0.0 });
    }
    let reach = |step: f64| (((upper.x - centre.x) / step + REACH_SLACK).floor(), ((upper.y - centre.y) / step + REACH_SLACK).floor());
    let (mut across, mut up) = reach(step);
    while (2.0 * across + 1.0) * (2.0 * up + 1.0) > NODE_BUDGET as f64 {
        step *= 2.0;
        (across, up) = reach(step);
    }
    let (across, up) = (across.max(0.0) as i64, up.max(0.0) as i64);
    let rows: Vec<Vec<DVec2>> = (-up..=up)
        .into_par_iter()
        .map(|row| -> Result<Vec<DVec2>> {
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            let y = centre.y + row as f64 * step;
            let mut kept = Vec::new();
            for column in -across..=across {
                let node = DVec2::new(centre.x + column as f64 * step, y);
                if node.cmpge(lower).all() && node.cmple(upper).all() && inside(node) {
                    kept.try_reserve(1).context("Not enough memory for the curvature lattice")?;
                    kept.push(node);
                }
            }
            Ok(kept)
        })
        .collect::<Result<_>>()?;
    let total = rows.iter().map(Vec::len).sum();
    let mut positions = Vec::new();
    positions.try_reserve_exact(total).context("Not enough memory for the curvature lattice")?;
    rows.into_iter().for_each(|row| positions.extend(row));
    Ok(LatticeNodes { positions, step })
}

/// The first pass's slope and second derivatives at one node.
#[derive(Clone, Copy, Default)]
struct NodeCurvature {
    slope: DVec2,
    xx: f64,
    xy: f64,
    yy: f64,
}

/// Slope and curvature at every node, in the nodes' order. The exact
/// spline answers the slope analytically; the second derivatives are its
/// slope's central differences `step` either side, the mixed one averaged
/// both ways so the result is symmetric.
fn node_curvature(surface: &RbfSurface, nodes: &[DVec2], step: f64, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<NodeCurvature>> {
    let mut curvature = Vec::new();
    curvature.try_reserve_exact(nodes.len()).context("Not enough memory for the curvature lattice")?;
    curvature.resize(nodes.len(), NodeCurvature::default());
    let evaluated = progress.counter(nodes.len());
    let (east, north) = (DVec2::new(step, 0.0), DVec2::new(0.0, step));
    curvature
        .par_chunks_mut(PROGRESS_STRIDE)
        .zip(nodes.par_chunks(PROGRESS_STRIDE))
        .try_for_each(|(slots, nodes)| -> Result<()> {
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            for (slot, &node) in slots.iter_mut().zip(nodes) {
                let (east_ahead, east_behind) = (surface.gradient(node + east), surface.gradient(node - east));
                let (north_ahead, north_behind) = (surface.gradient(node + north), surface.gradient(node - north));
                *slot = NodeCurvature {
                    slope: surface.gradient(node),
                    xx: (east_ahead.x - east_behind.x) / (2.0 * step),
                    xy: ((north_ahead.x - north_behind.x) + (east_ahead.y - east_behind.y)) / (4.0 * step),
                    yy: (north_ahead.y - north_behind.y) / (2.0 * step),
                };
            }
            evaluated.advance_by(slots.len());
            Ok(())
        })?;
    Ok(curvature)
}

/// A symmetric 2 x 2 tensor [[xx, xy], [xy, yy]].
#[derive(Clone, Copy)]
struct Tensor {
    xx: f64,
    xy: f64,
    yy: f64,
}

impl Tensor {
    /// The larger principal value, then the smaller.
    fn principal(&self) -> (f64, f64) {
        let mean = (self.xx + self.yy) / 2.0;
        let half = (self.xx - self.yy) / 2.0;
        let radius = (half * half + self.xy * self.xy).sqrt();
        (mean + radius, mean - radius)
    }

    /// The azimuth, clockwise from north in [0, 180), of the smaller
    /// principal direction. The larger lies 1/2 atan2(2 xy, xx - yy)
    /// anticlockwise from east, the smaller a right angle on, and an
    /// azimuth runs the other way from north.
    fn least_azimuth(&self) -> f64 {
        let larger = 0.5 * libm::atan2(2.0 * self.xy, self.xx - self.yy);
        let azimuth = (-larger).to_degrees().rem_euclid(180.0);
        if azimuth >= 180.0 { 0.0 } else { azimuth }
    }
}

/// The mean over the nodes of each Hessian's absolute value, the matrix
/// with its principal values made positive, weighted by its largest
/// principal value, so crests and troughs add rather than cancel. `None`
/// when no node bends at all.
fn strength_weighted_tensor(curvature: &[NodeCurvature]) -> Option<Tensor> {
    let mut sum = Tensor { xx: 0.0, xy: 0.0, yy: 0.0 };
    let mut weight = 0.0;
    for node in curvature {
        let mean = (node.xx + node.yy) / 2.0;
        let half = (node.xx - node.yy) / 2.0;
        let radius = (half * half + node.xy * node.xy).sqrt();
        // Principal values mean ± radius: the largest in size is
        // |mean| + radius, and |a| + |b| is twice the larger of |mean|
        // and radius. |H| = (H² + |det H| I) / (|a| + |b|).
        let strength = mean.abs() + radius;
        let magnitudes = 2.0 * mean.abs().max(radius);
        if magnitudes.is_nan() || magnitudes <= 0.0 {
            continue;
        }
        let determinant = (node.xx * node.yy - node.xy * node.xy).abs();
        let scale = strength / magnitudes;
        sum.xx += scale * (node.xx * node.xx + node.xy * node.xy + determinant);
        sum.xy += scale * (node.xy * (node.xx + node.yy));
        sum.yy += scale * (node.xy * node.xy + node.yy * node.yy + determinant);
        weight += strength;
    }
    (weight > 0.0).then(|| Tensor {
        xx: sum.xx / weight,
        xy: sum.xy / weight,
        yy: sum.yy / weight,
    })
}

/// The fold's wavelength across `azimuth`: 2 π times the spread of the
/// slope across the axis over the root mean square of the curvature across
/// it. For a sine across the axis the two are its slope and curvature
/// amplitudes, whose ratio is the wavelength over 2 π; the mean slope, the
/// regional dip, carries no fold and is taken out. `None` with no
/// curvature across the axis.
fn wavelength_across(curvature: &[NodeCurvature], azimuth: f64) -> Option<f64> {
    let count = curvature.len() as f64;
    let radians = azimuth.to_radians();
    let across = DVec2::new(libm::cos(radians), -libm::sin(radians));
    let slope = |node: &NodeCurvature| node.slope.x * across.x + node.slope.y * across.y;
    let mean = curvature.iter().map(slope).sum::<f64>() / count;
    let spread = (curvature.iter().map(|node| (slope(node) - mean) * (slope(node) - mean)).sum::<f64>() / count).sqrt();
    let bend = |node: &NodeCurvature| node.xx * across.x * across.x + 2.0 * node.xy * across.x * across.y + node.yy * across.y * across.y;
    let root_mean_square = (curvature.iter().map(|node| bend(node) * bend(node)).sum::<f64>() / count).sqrt();
    (root_mean_square > 0.0).then(|| 2.0 * std::f64::consts::PI * spread / root_mean_square)
}
