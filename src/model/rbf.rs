//! Thin plate spline surface through scattered points, gridded on a lattice
//! snapped to the project coordinates.
//!
//! The spline passes exactly through every point and bends as little as
//! possible between them, with a plane carrying the trend. Every
//! transcendental comes from `libm` and no step fuses a multiply-add, so
//! native and wasm builds agree to the bit.

// Dip, `Lattice` and the grid calls have no caller.
#![cfg_attr(not(test), allow(dead_code))]

use anyhow::{Context, Result};
use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{kernel, progress::Phase},
};

/// Most points one surface solves for, counted after merging. The system
/// is dense: memory grows with the square of the count, the solve its cube.
pub(crate) const POINT_BUDGET: usize = 4_000;

/// Most nodes one grid holds.
pub(crate) const NODE_BUDGET: usize = 4_000_000;

/// Lattice spacing in metres unless the caller chooses another.
pub(crate) const DEFAULT_SPACING: f64 = 5.0;

/// Points closer than this in plan, in metres, are one point.
pub(crate) const MERGE_DISTANCE: f64 = 0.01;

/// Largest height difference, in metres, between two points that merge.
pub(crate) const MERGE_HEIGHT: f64 = 0.001;

/// Nodes between cancellation checks and progress reports.
const PROGRESS_STRIDE: usize = 4096;

/// The kernel r² ln r, written on the squared distance as ½ r² ln r².
fn kernel_value(r2: f64) -> f64 {
    if r2 > 0.0 { 0.5 * r2 * libm::log(r2) } else { 0.0 }
}

/// The kernel's gradient over the offset from its centre: the gradient of
/// r² ln r is (ln r² + 1) times that offset.
fn kernel_slope(r2: f64) -> f64 {
    if r2 > 0.0 { libm::log(r2) + 1.0 } else { 0.0 }
}

/// An exact interpolating surface z = f(x, y).
#[derive(Debug)]
pub(crate) struct RbfSurface {
    origin: DVec2,
    scale: f64,
    /// Merged points in canonical order, local and scaled.
    centres: Vec<DVec2>,
    /// The same points in project coordinates, with their heights, for
    /// reports.
    points: Vec<DVec3>,
    weights: Vec<f64>,
    /// Constant, x and y coefficients of the plane, in local coordinates.
    plane: [f64; 3],
    merged: usize,
}

/// Dip in degrees from horizontal, and the azimuth of steepest descent in
/// degrees clockwise from north (+y), in 0..360; 0 where the surface is flat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dip {
    pub(crate) dip: f64,
    pub(crate) direction: f64,
}

impl RbfSurface {
    /// Fits the surface through `points`. Input order does not matter:
    /// points are sorted into one canonical order before anything else.
    pub(crate) fn fit(points: &[DVec3], cancel: &CancelFlag, progress: &Phase) -> Result<Self> {
        if let Some(point) = points.iter().find(|point| !point.is_finite()) {
            anyhow::bail!("Point ({}, {}, {}) is not a finite number", point.x, point.y, point.z);
        }
        let given = points.len();
        let (points, merged) = merge_close_points(points)?;
        if points.len() > POINT_BUDGET {
            anyhow::bail!("{} points after merging ({given} given) exceed the budget of {POINT_BUDGET} for one surface", points.len());
        }
        if points.len() < 3 {
            anyhow::bail!("A surface needs at least three points, {} given", points.len());
        }
        refuse_collinear(&points)?;

        let origin = points.iter().fold(DVec2::ZERO, |sum, point| sum + point.truncate()) / points.len() as f64;
        let scale = points.iter().fold(0.0f64, |spread, point| spread.max((point.truncate() - origin).abs().max_element()));
        let mut centres = Vec::new();
        centres.try_reserve_exact(points.len()).context("Not enough memory for the surface points")?;
        centres.extend(points.iter().map(|point| (point.truncate() - origin) / scale));

        let n = centres.len();
        let size = n + 3;
        let width = size + 1;
        let cells = size.checked_mul(width).context("Surface system size overflows")?;
        let mut matrix = allocated_values(cells, 0.0, "surface system entries")?;
        for (row, (centre, point)) in matrix.chunks_exact_mut(width).zip(centres.iter().zip(&points)) {
            for (cell, other) in row.iter_mut().zip(&centres) {
                *cell = kernel_value(distance_squared(*centre, *other));
            }
            row[n..].copy_from_slice(&[1.0, centre.x, centre.y, point.z]);
        }
        for (column, centre) in centres.iter().enumerate() {
            matrix[n * width + column] = 1.0;
            matrix[(n + 1) * width + column] = centre.x;
            matrix[(n + 2) * width + column] = centre.y;
        }
        let mut weights = solve(&mut matrix, size, cancel, progress)?;
        drop(matrix);
        let plane = [weights[n], weights[n + 1], weights[n + 2]];
        weights.truncate(n);
        progress.finish();
        Ok(Self {
            origin,
            scale,
            centres,
            points,
            weights,
            plane,
            merged,
        })
    }

    /// Points dropped as duplicates of a nearby point at the same height.
    pub(crate) fn merged(&self) -> usize {
        self.merged
    }

    /// Points the surface passes through, after merging.
    pub(crate) fn point_count(&self) -> usize {
        self.centres.len()
    }

    /// The points the surface passes through, after merging, in canonical
    /// order: x, then y, then z.
    pub(crate) fn points(&self) -> &[DVec3] {
        &self.points
    }

    pub(crate) fn height(&self, at: DVec2) -> f64 {
        let local = self.local(at);
        let mut height = self.plane[0] + self.plane[1] * local.x + self.plane[2] * local.y;
        for (centre, weight) in self.centres.iter().zip(&self.weights) {
            height += weight * kernel_value(distance_squared(local, *centre));
        }
        height
    }

    /// Slope dz/dx, dz/dy in project units.
    pub(crate) fn gradient(&self, at: DVec2) -> DVec2 {
        let local = self.local(at);
        let mut slope = DVec2::new(self.plane[1], self.plane[2]);
        for (centre, weight) in self.centres.iter().zip(&self.weights) {
            let offset = local - *centre;
            slope += offset * (weight * kernel_slope(offset.x * offset.x + offset.y * offset.y));
        }
        slope / self.scale
    }

    pub(crate) fn dip(&self, at: DVec2) -> Dip {
        let slope = self.gradient(at);
        let steepness = (slope.x * slope.x + slope.y * slope.y).sqrt();
        let dip = libm::atan(steepness).to_degrees();
        if steepness == 0.0 {
            return Dip { dip, direction: 0.0 };
        }
        let mut direction = libm::atan2(-slope.x, -slope.y).to_degrees();
        if direction < 0.0 {
            direction += 360.0;
        }
        if direction >= 360.0 {
            direction = 0.0;
        }
        Dip { dip, direction }
    }

    /// Heights at every node of `lattice`, row by row from its lowest y.
    /// Nodes are independent, so the result does not depend on the thread
    /// count or the order they finish in.
    pub(crate) fn grid(&self, lattice: Lattice, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<f64>> {
        self.grid_where(lattice, |_| true, cancel, progress)
    }

    /// As [`Self::grid`], but only the nodes `keep` accepts are evaluated;
    /// the rest are NaN.
    pub(crate) fn grid_where(&self, lattice: Lattice, keep: impl Fn(DVec2) -> bool + Sync, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<f64>> {
        let count = lattice.node_count();
        let mut heights = allocated_values(count, f64::NAN, "grid nodes")?;
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
                    let node = lattice.node(index % lattice.columns, index / lattice.columns);
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

    fn local(&self, at: DVec2) -> DVec2 {
        (at - self.origin) / self.scale
    }
}

/// Grid nodes at whole multiples of the spacing in project coordinates, so
/// any two lattices at one spacing share their common nodes exactly. A
/// lattice never exceeds [`NODE_BUDGET`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lattice {
    spacing: f64,
    first: [i64; 2],
    columns: usize,
    rows: usize,
}

impl Lattice {
    /// The smallest lattice covering `lower..=upper`. Refuses, before
    /// anything is allocated, a lattice over the node budget.
    pub(crate) fn covering(lower: DVec2, upper: DVec2, spacing: f64) -> Result<Self> {
        if !lower.is_finite() || !upper.is_finite() || lower.cmpgt(upper).any() {
            anyhow::bail!("Grid extent must be finite, with its lower corner below its upper");
        }
        if !spacing.is_finite() || spacing <= 0.0 {
            anyhow::bail!("Grid spacing must be a positive number of metres");
        }
        let first = (lower / spacing).floor();
        let last = (upper / spacing).ceil();
        // Node indices must stay integers that f64 holds exactly.
        const INDEX_LIMIT: f64 = 4.0e15;
        if first.abs().max_element() > INDEX_LIMIT || last.abs().max_element() > INDEX_LIMIT {
            anyhow::bail!("Grid extent is too far from the origin for a spacing of {spacing} m");
        }
        let columns = last.x - first.x + 1.0;
        let rows = last.y - first.y + 1.0;
        let nodes = columns * rows;
        if nodes > NODE_BUDGET as f64 {
            anyhow::bail!(
                "The grid needs {nodes} nodes ({columns} x {rows}), over the budget of {NODE_BUDGET}; a spacing of {} m or more fits",
                fitting_spacing(upper - lower)
            );
        }
        Ok(Self {
            spacing,
            first: [first.x as i64, first.y as i64],
            columns: columns as usize,
            rows: rows as usize,
        })
    }

    pub(crate) fn spacing(&self) -> f64 {
        self.spacing
    }

    /// Lattice index of the first node; the node itself is index x spacing.
    pub(crate) fn first(&self) -> [i64; 2] {
        self.first
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn node_count(&self) -> usize {
        self.columns * self.rows
    }

    pub(crate) fn node(&self, column: usize, row: usize) -> DVec2 {
        DVec2::new((self.first[0] + column as i64) as f64 * self.spacing, (self.first[1] + row as i64) as f64 * self.spacing)
    }
}

/// A spacing, rounded up to 0.1 m, for a lattice over `extent` to fit the
/// node budget: it solves (w t + 2)(h t + 2) = budget for t, the nodes per
/// metre. A snapped axis can hold ceil(w t) + 2 nodes, so where the
/// rounding adds nothing it can miss by a node per axis.
fn fitting_spacing(extent: DVec2) -> f64 {
    let budget = NODE_BUDGET as f64;
    let (sum, product) = (extent.x + extent.y, extent.x * extent.y);
    let per_metre = if product > 0.0 {
        ((sum * sum + product * (budget - 4.0)).sqrt() - sum) / product
    } else {
        (budget - 4.0) / (2.0 * sum)
    };
    (10.0 / per_metre).ceil() / 10.0
}

/// Two points close in plan with a steep slope between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SteepPair {
    /// The pair in canonical order: x, then y, then z.
    pub(crate) first: DVec3,
    pub(crate) second: DVec3,
    /// Plan distance between them, in metres.
    pub(crate) distance: f64,
    /// Height between them, in metres, never negative.
    pub(crate) rise: f64,
    /// Slope between them, in degrees from horizontal.
    pub(crate) slope: f64,
}

/// Every pair of points closer than `within` in plan whose slope,
/// atan(rise / distance), exceeds `steeper_than` degrees: steepest first,
/// ties in canonical order. Meant for the merged points, no two of which
/// are within [`MERGE_DISTANCE`]; it reports and changes nothing. A sweep
/// along x over the points sorted, so only pairs less than `within` apart
/// in x are measured.
pub(crate) fn steep_pairs(points: &[DVec3], within: f64, steeper_than: f64) -> Result<Vec<SteepPair>> {
    let mut sorted = Vec::new();
    sorted.try_reserve_exact(points.len()).context("Not enough memory for the surface points")?;
    sorted.extend_from_slice(points);
    sorted.sort_unstable_by(canonical);
    let mut pairs = Vec::new();
    for (index, first) in sorted.iter().enumerate() {
        for second in sorted[index + 1..].iter().take_while(|second| second.x - first.x < within) {
            let distance = distance_squared(first.truncate(), second.truncate()).sqrt();
            if distance >= within {
                continue;
            }
            let rise = (second.z - first.z).abs();
            let slope = libm::atan2(rise, distance).to_degrees();
            if slope > steeper_than {
                pairs.try_reserve(1).context("Not enough memory for the steep pairs")?;
                pairs.push(SteepPair {
                    first: *first,
                    second: *second,
                    distance,
                    rise,
                    slope,
                });
            }
        }
    }
    // Stable, so pairs equally steep stay in the order the sweep met them.
    pairs.sort_by(|a, b| b.slope.total_cmp(&a.slope));
    Ok(pairs)
}

/// x, then y, then z.
fn canonical(a: &DVec3, b: &DVec3) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z))
}

/// Sorts the points by x, then y, then z, and drops each one closer than
/// [`MERGE_DISTANCE`] in plan to one already kept, so a row of dropped
/// points cannot carry a merge further than that. Refuses a pair that close
/// whose heights differ by more than [`MERGE_HEIGHT`], whether the earlier
/// one was kept or dropped, so a dropped point still answers for its own
/// height. Every such pair is named in the one refusal, so all of them can
/// be fixed in one pass. Returns the kept points and how many merged.
fn merge_close_points(points: &[DVec3]) -> Result<(Vec<DVec3>, usize)> {
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

/// Refuses points that all lie within [`MERGE_DISTANCE`] of one line in
/// plan: they fix no plane. The line runs from the first point to the one
/// farthest from it, and the distance is metric, so the test means the same
/// at any coordinate magnitude.
fn refuse_collinear(points: &[DVec3]) -> Result<()> {
    let first = points.first().map(|point| point.truncate()).unwrap_or_default();
    let far = points
        .iter()
        .map(|point| point.truncate())
        .max_by(|a, b| distance_squared(first, *a).total_cmp(&distance_squared(first, *b)))
        .unwrap_or_default();
    if !points
        .iter()
        .any(|point| kernel::signed_distance_to_line(point.truncate(), first, far).abs() > MERGE_DISTANCE)
    {
        anyhow::bail!(
            "The {} points lie on one line in plan, within {MERGE_DISTANCE} m; a surface needs points off it",
            points.len()
        );
    }
    Ok(())
}

/// Solves the square system held row by row in `matrix`, its right-hand side
/// in the last column: LU factorisation with partial pivoting, applied to
/// the right-hand side as it forms, then back substitution. Single-threaded
/// in a fixed order, so every machine gets the same answer.
fn solve(matrix: &mut [f64], size: usize, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<f64>> {
    let width = size + 1;
    if matrix.len() != size * width {
        anyhow::bail!("Surface system has the wrong shape");
    }
    let tiny = matrix.iter().fold(0.0f64, |largest, value| largest.max(value.abs())) * f64::EPSILON;
    for column in 0..size {
        if cancel.is_cancelled() {
            anyhow::bail!("Cancelled");
        }
        let pivot_row = (column..size)
            .max_by(|&a, &b| matrix[a * width + column].abs().total_cmp(&matrix[b * width + column].abs()))
            .unwrap_or(column);
        let pivot = matrix[pivot_row * width + column];
        if !pivot.is_finite() || pivot.abs() <= tiny {
            anyhow::bail!("The points give a singular surface system");
        }
        if pivot_row != column {
            for offset in column..width {
                matrix.swap(pivot_row * width + offset, column * width + offset);
            }
        }
        let (done, below) = matrix.split_at_mut((column + 1) * width);
        let pivot_tail = &done[column * width + column + 1..];
        for row in below.chunks_exact_mut(width) {
            let factor = row[column] / pivot;
            if factor != 0.0 {
                for (cell, above) in row[column + 1..].iter_mut().zip(pivot_tail) {
                    *cell -= factor * above;
                }
            }
        }
        let left = (size - column - 1) as f32 / size as f32;
        progress.set_fraction(1.0 - left * left * left);
    }
    let mut solution = allocated_values(size, 0.0, "surface weights")?;
    for column in (0..size).rev() {
        let row = &matrix[column * width..(column + 1) * width];
        let mut value = row[size];
        for (coefficient, known) in row[column + 1..size].iter().zip(&solution[column + 1..]) {
            value -= coefficient * known;
        }
        solution[column] = value / row[column];
    }
    if solution.iter().any(|value| !value.is_finite()) {
        anyhow::bail!("The points give a singular surface system");
    }
    Ok(solution)
}

/// Squared plan distance, written out so no multiply-add can fuse.
fn distance_squared(a: DVec2, b: DVec2) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn allocated_values(count: usize, initial: f64, description: &str) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .with_context(|| format!("Not enough memory to allocate {count} {description}"))?;
    values.resize(count, initial);
    Ok(values)
}
