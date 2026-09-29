//! The lattice a surface grid is held on when only part of its box is
//! wanted: [`Lattice`]'s snapped nodes, kept row by row as runs of the
//! columns a build needs, so the node budget counts those nodes and memory
//! follows them rather than the box. A long thin extent on the diagonal is
//! not refused for the empty box around it.
//!
//! Node positions use exactly [`Lattice`]'s arithmetic, so a node
//! here and the same node there are the same bits, and grids built either
//! way at one spacing still share their common nodes exactly.
//!
//! [`Lattice`]: crate::model::rbf::Lattice

use anyhow::{Context, Result};
use glam::DVec2;
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{progress::Phase, rbf::NODE_BUDGET},
};

/// Nodes between cancellation checks and progress reports.
const PROGRESS_STRIDE: usize = 4096;

/// The box of lattice nodes covering an extent, snapped to multiples of the
/// spacing in project coordinates as [`Lattice`] snaps it. It holds
/// no nodes, so it carries no node budget; only its sides are limited, so
/// every column and row index fits in 32 bits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LatticeBox {
    spacing: f64,
    first: [i64; 2],
    columns: usize,
    rows: usize,
}

impl LatticeBox {
    /// The smallest box covering `lower..=upper`, by [`Lattice`]'s own
    /// arithmetic and its guard on index magnitude.
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
        if columns.max(rows) > NODE_BUDGET as f64 {
            // A snapped side has at most ceil(extent / spacing) + 2 nodes.
            let longest = (upper - lower).max_element();
            anyhow::bail!(
                "The grid spans {columns} x {rows} nodes, over {NODE_BUDGET} along a side; a spacing of {} m or more fits",
                (10.0 * longest / (NODE_BUDGET - 2) as f64).ceil() / 10.0
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

    pub(crate) fn node(&self, column: usize, row: usize) -> DVec2 {
        DVec2::new((self.first[0] + column as i64) as f64 * self.spacing, (self.first[1] + row as i64) as f64 * self.spacing)
    }

    /// The column and row of the node at exactly `at`, if the box has one.
    pub(crate) fn node_at(&self, at: DVec2) -> Option<(usize, usize)> {
        let column = usize::try_from((at.x / self.spacing).round() as i64 - self.first[0]).ok()?;
        let row = usize::try_from((at.y / self.spacing).round() as i64 - self.first[1]).ok()?;
        (column < self.columns && row < self.rows && self.node(column, row) == at).then_some((column, row))
    }
}

/// Runs of columns, row by row: each run is the columns from its first up
/// to but not including its second, ascending and apart within a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RowRuns {
    /// Where each row's runs start in `runs`, one more than the rows.
    starts: Vec<usize>,
    runs: Vec<[u32; 2]>,
}

impl RowRuns {
    /// The runs `runs_of` gives each of `rows` rows, rows worked in
    /// parallel and joined in row order, so the result does not depend on
    /// the thread count.
    pub(crate) fn collect(rows: usize, cancel: &CancelFlag, runs_of: impl Fn(usize, &mut Vec<[u32; 2]>) -> Result<()> + Sync) -> Result<Self> {
        let task_count = rayon::current_num_threads().saturating_mul(4).max(1);
        let chunk = rows.div_ceil(task_count).max(1);
        let pieces = (0..rows.div_ceil(chunk))
            .into_par_iter()
            .map(|piece| -> Result<(Vec<usize>, Vec<[u32; 2]>)> {
                let rows = piece * chunk..rows.min((piece + 1) * chunk);
                let mut counts = Vec::new();
                counts.try_reserve_exact(rows.len()).context("Not enough memory for the grid rows")?;
                let mut runs = Vec::new();
                for row in rows {
                    if cancel.is_cancelled() {
                        anyhow::bail!("Cancelled");
                    }
                    let before = runs.len();
                    runs_of(row, &mut runs)?;
                    counts.push(runs.len() - before);
                }
                Ok((counts, runs))
            })
            .collect::<Result<Vec<_>>>()?;
        let total = pieces.iter().map(|(_, runs)| runs.len()).sum();
        let mut starts = Vec::new();
        starts.try_reserve_exact(rows + 1).context("Not enough memory for the grid rows")?;
        let mut runs = Vec::new();
        runs.try_reserve_exact(total).context("Not enough memory for the grid rows")?;
        starts.push(0);
        for (counts, piece) in pieces {
            for count in counts {
                starts.push(starts[starts.len() - 1] + count);
            }
            runs.extend_from_slice(&piece);
        }
        Ok(Self { starts, runs })
    }

    pub(crate) fn rows(&self) -> usize {
        self.starts.len().saturating_sub(1)
    }

    /// The runs of one row; none past the last row.
    pub(crate) fn row(&self, row: usize) -> &[[u32; 2]] {
        match (self.starts.get(row), self.starts.get(row + 1)) {
            (Some(&start), Some(&end)) => &self.runs[start..end],
            _ => &[],
        }
    }
}

/// The nodes of a [`LatticeBox`] a build needs, held as runs of columns row
/// by row and numbered in row order from the lowest y, then by column: the
/// order of the box's own nodes with the rest left out. Never more than
/// [`NODE_BUDGET`] nodes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpanLattice {
    bounds: LatticeBox,
    runs: RowRuns,
    /// The number of the first node of each run.
    run_starts: Vec<usize>,
    count: usize,
}

impl SpanLattice {
    /// The nodes of `runs` in `bounds`, one row of runs per row of nodes.
    /// Refuses, before anything the size of the nodes is allocated, more
    /// nodes than the budget, naming the count and a spacing at which the
    /// same extent, `ring`, fits.
    pub(crate) fn new(bounds: LatticeBox, runs: RowRuns, ring: &[DVec2]) -> Result<Self> {
        if runs.rows() != bounds.rows {
            anyhow::bail!("Grid runs do not match the grid's rows");
        }
        let mut count = 0usize;
        for row in 0..runs.rows() {
            let mut end = 0;
            for (index, &[start, stop]) in runs.row(row).iter().enumerate() {
                if start >= stop || (index > 0 && start <= end) || stop as usize > bounds.columns {
                    anyhow::bail!("Grid runs are out of order in row {row}");
                }
                end = stop;
                count = count.saturating_add((stop - start) as usize);
            }
        }
        if count > NODE_BUDGET {
            anyhow::bail!(
                "The grid needs {count} nodes inside the extent, over the budget of {NODE_BUDGET}; a spacing of {} m or more fits",
                fitting_spacing(ring, NODE_BUDGET)
            );
        }
        let mut run_starts = Vec::new();
        run_starts.try_reserve_exact(runs.runs.len()).context("Not enough memory for the grid rows")?;
        let mut next = 0;
        for &[start, stop] in &runs.runs {
            run_starts.push(next);
            next += (stop - start) as usize;
        }
        Ok(Self { bounds, runs, run_starts, count })
    }

    pub(crate) fn bounds(&self) -> &LatticeBox {
        &self.bounds
    }

    pub(crate) fn spacing(&self) -> f64 {
        self.bounds.spacing
    }

    /// Columns of the box, needed or not.
    #[allow(dead_code)]
    pub(crate) fn columns(&self) -> usize {
        self.bounds.columns
    }

    /// Rows of the box, needed or not.
    #[allow(dead_code)]
    pub(crate) fn rows(&self) -> usize {
        self.bounds.rows
    }

    pub(crate) fn node(&self, column: usize, row: usize) -> DVec2 {
        self.bounds.node(column, row)
    }

    /// The nodes the build needs.
    pub(crate) fn node_count(&self) -> usize {
        self.count
    }

    /// The number of the node at `column`, `row`, if the build needs it.
    pub(crate) fn index(&self, column: usize, row: usize) -> Option<usize> {
        let runs = self.runs.row(row);
        let found = runs.partition_point(|run| run[1] as usize <= column);
        let run = runs.get(found)?;
        let first = self.runs.starts[row] + found;
        (run[0] as usize <= column).then(|| self.run_starts[first] + column - run[0] as usize)
    }

    /// Every needed node's column and row, in their numbered order.
    pub(crate) fn columns_and_rows(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..self.runs.rows()).flat_map(move |row| {
            self.runs
                .row(row)
                .iter()
                .flat_map(move |&[start, stop]| (start as usize..stop as usize).map(move |column| (column, row)))
        })
    }

    /// Every needed node's position, in their numbered order.
    pub(crate) fn nodes(&self) -> impl Iterator<Item = DVec2> + '_ {
        self.columns_and_rows().map(|(column, row)| self.node(column, row))
    }

    /// `height` at every needed node `keep` accepts, in their numbered
    /// order; NaN at the rest. Nodes are independent, so they run in
    /// parallel and the result does not depend on the thread count or the
    /// order they finish in. Memory for the heights is reserved first.
    pub(crate) fn heights(&self, height: impl Fn(DVec2) -> f64 + Sync, keep: impl Fn(DVec2) -> bool + Sync, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<f64>> {
        let count = self.count;
        let mut heights = Vec::new();
        heights
            .try_reserve_exact(count)
            .with_context(|| format!("Not enough memory to allocate {count} grid nodes"))?;
        heights.resize(count, f64::NAN);
        let task_count = rayon::current_num_threads().saturating_mul(4).max(1);
        let chunk_size = count.div_ceil(task_count).max(1);
        let evaluated = progress.counter(count);
        heights.par_chunks_mut(chunk_size).enumerate().try_for_each(|(chunk_index, chunk)| -> Result<()> {
            // The run holding the chunk's first node, then onward in order.
            let base = chunk_index * chunk_size;
            let mut run = self.run_starts.partition_point(|&start| start <= base) - 1;
            let mut row = self.runs.starts.partition_point(|&start| start <= run) - 1;
            let mut column = self.runs.runs[run][0] as usize + (base - self.run_starts[run]);
            for slice in chunk.chunks_mut(PROGRESS_STRIDE) {
                if cancel.is_cancelled() {
                    anyhow::bail!("Cancelled");
                }
                for slot in slice.iter_mut() {
                    while column == self.runs.runs[run][1] as usize {
                        run += 1;
                        column = self.runs.runs[run][0] as usize;
                        while self.runs.starts[row + 1] <= run {
                            row += 1;
                        }
                    }
                    let node = self.node(column, row);
                    if keep(node) {
                        *slot = height(node);
                    }
                    column += 1;
                }
                evaluated.advance_by(slice.len());
            }
            Ok(())
        })?;
        progress.finish();
        Ok(heights)
    }
}

/// A spacing, rounded up to 0.1 m, at which the nodes a build needs inside
/// or beside `ring` fit `budget`. Every such node is a corner of a cell
/// meeting the ring's area, so within a cell's diagonal of it, and each
/// node owns a square of the spacing t about it; so they all lie within
/// r = t (sqrt 2 + 1 / sqrt 2), under 2.2 t, of the area, which holds at
/// most A + 2 r P + pi r^2 square metres for an area A and perimeter P.
/// This solves A u^2 + 4.4 P u + 4.84 pi = budget for u, the nodes per
/// metre along a side.
pub(crate) fn fitting_spacing(ring: &[DVec2], budget: usize) -> f64 {
    let count = ring.len();
    let edges = (0..count).map(|index| (ring[index], ring[(index + 1) % count]));
    let area = edges.clone().map(|(a, b)| a.perp_dot(b)).sum::<f64>().abs() / 2.0;
    let perimeter: f64 = edges.map(|(a, b)| a.distance(b)).sum();
    let spare = budget as f64 - 4.84 * std::f64::consts::PI;
    let half = 2.2 * perimeter;
    let per_metre = if area > 0.0 {
        ((half * half + area * spare).sqrt() - half) / area
    } else {
        spare / (2.0 * half)
    };
    (10.0 / per_metre).ceil() / 10.0
}
