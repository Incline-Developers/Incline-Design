//! Solving for nested pit shells with mineflow.
//!
//! Whittle's parameterisation, adapted to a solver that keeps nothing between
//! solves: values never fall from one shell to the next, so every smallest
//! optimal pit holds the one before it and sits inside the one after. The
//! largest shell is solved on the whole grid; every other shell only on the
//! *band* between the two nearest shells already known, splitting each gap at
//! its middle. A band solve is exact: what lies in the inner shell is already
//! mined, and nothing a band block needs can lie outside the outer shell,
//! which is closed. The bands of one round never share a cell, so they are
//! solved side by side.

use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use glam::DVec2;
use mineflow::{Precedence, PrecedenceSource, Solver, ValueScale};
use rayon::prelude::*;

use super::{
    grid::{Grid, NO_BLOCK},
    prepare::{Front, ShellPlan, SlopeSpec},
    values::BlockValues,
};
use crate::{app::jobs::CancelFlag, model::progress::Phase};

/// Not in any shell.
pub(crate) const OUTSIDE: u16 = 0;

/// What each shell holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShellSummary {
    /// The revenue factor.
    pub(crate) factor: f64,
    /// A directional shell's share of the distance across the final pit.
    pub(crate) distance: Option<f64>,
    pub(crate) blocks: usize,
    pub(crate) tonnes: f64,
    pub(crate) ore_tonnes: f64,
    pub(crate) waste_tonnes: f64,
    pub(crate) value: f64,
}

/// Every shell of a run.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shells {
    /// Per cell, the first shell (1-based) that holds it, or [`OUTSIDE`]; shell
    /// `k` is every cell labelled `1..=k`.
    pub(crate) label: Vec<u16>,
    pub(crate) summaries: Vec<ShellSummary>,
}

/// A cell's value in shell `k` (1-based).
struct ShellValues<'a> {
    values: &'a BlockValues,
    plan: &'a ShellPlan,
    /// Directional runs: each cell's share of the distance across the final pit.
    distance: Vec<f64>,
}

impl ShellValues<'_> {
    /// A directional shell's share of the distance: k/N.
    fn distance_share(&self, shell: usize) -> Option<f64> {
        match self.plan {
            ShellPlan::Factors(_) => None,
            ShellPlan::Directional { factors, .. } => Some(shell as f64 / factors.len() as f64),
        }
    }

    fn revenue_factor(&self, shell: usize) -> f64 {
        match self.plan {
            ShellPlan::Factors(factors) | ShellPlan::Directional { factors, .. } => factors[shell - 1],
        }
    }

    /// Whether the cell may earn in `shell`; a directional shell lets ore count
    /// only up to its share of the distance.
    fn earns(&self, shell: usize, cell: usize) -> bool {
        self.distance.is_empty() || self.distance_share(shell).is_none_or(|share| self.distance[cell] <= share + 1.0e-12)
    }

    fn value(&self, shell: usize, cell: usize) -> f64 {
        if self.earns(shell, cell) {
            self.values.value(cell, self.revenue_factor(shell))
        } else {
            self.values.waste[cell]
        }
    }

    fn is_ore(&self, shell: usize, cell: usize) -> bool {
        self.earns(shell, cell) && self.values.destination(cell, self.revenue_factor(shell)).is_some()
    }
}

type Offset = (i64, i64, i64);

/// Slope angles are grouped to this step, one pattern per group (mineflow's
/// own default for per-block slopes).
const ANGLE_STEP_DEGREES: f64 = 0.1;

/// The precedence the slopes give: mineflow's graph for whole-grid solves and
/// the same patterns as offsets for band solves.
pub(crate) struct Slopes {
    precedence: Precedence,
    offsets: Arc<Vec<Vec<Offset>>>,
    keys: Option<Arc<Vec<u16>>>,
    /// Blocks whose angle field was blank or out of range, given the default.
    pub(crate) blocks_with_default_angle: usize,
}

/// How many benches up a slope pattern reaches. Not exposed: MineFlow's
/// default, enough for the overall angle to hold to about a degree.
fn bench_count(grid: &Grid) -> i64 {
    (grid.dims[2] as i64).clamp(1, 12)
}

/// Build the slope patterns: one for a uniform slope, one per distinct angle
/// (to [`ANGLE_STEP_DEGREES`]) for angles read per block. Cells with no block
/// take the field's default angle.
pub(crate) fn slopes(grid: &Grid, spec: &SlopeSpec, block_of_cell: &[u32]) -> Result<Slopes> {
    let model = grid.mineflow();
    let benches = bench_count(grid);
    let error = |error: mineflow::Error| anyhow!("{error}");
    let slopes = match spec {
        SlopeSpec::Uniform(slope) => {
            let pattern = mineflow::Pattern::min_search(&model, slope, benches).map_err(error)?;
            Slopes {
                precedence: Precedence::regular_3d(&model, &pattern).map_err(error)?,
                offsets: Arc::new(vec![pattern.offsets().map_err(error)?]),
                keys: None,
                blocks_with_default_angle: 0,
            }
        }
        SlopeSpec::Field { values, fallback } => {
            let step = |angle: f64| (angle / ANGLE_STEP_DEGREES).round() as i64;
            let cell_steps: Vec<(i64, bool)> = block_of_cell
                .par_iter()
                .map(|&block| {
                    if block == NO_BLOCK {
                        return (step(*fallback), false);
                    }
                    let angle = values[block as usize];
                    if angle.is_finite() && angle > 0.0 && angle < 90.0 && step(angle) > 0 {
                        (step(angle), false)
                    } else {
                        (step(*fallback), true)
                    }
                })
                .collect();
            let mut distinct: Vec<i64> = cell_steps.iter().map(|(step, _)| *step).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            distinct.dedup();
            let patterns = distinct
                .par_iter()
                .map(|&key| {
                    let angle = mineflow::degrees((key as f64 * ANGLE_STEP_DEGREES).min(90.0));
                    mineflow::Slope::constant(angle).and_then(|slope| mineflow::Pattern::min_search(&model, &slope, benches))
                })
                .collect::<mineflow::Result<Vec<_>>>()
                .map_err(error)?;
            let keys: Vec<u16> = cell_steps
                .iter()
                .map(|(step, _)| distinct.binary_search(step).expect("every step is listed") as u16)
                .collect();
            let indices: Vec<i64> = keys.iter().map(|&key| i64::from(key)).collect();
            let precedence = Precedence::regular_3d_keyed(&model, &patterns, &indices).map_err(error)?;
            crate::userspace_log!(
                "{}",
                crate::i18n::tr!(
                    "opt-run-slope-angles",
                    count = distinct.len().to_string(),
                    from = format!("{:.1}", distinct[0] as f64 * ANGLE_STEP_DEGREES),
                    to = format!("{:.1}", distinct[distinct.len() - 1] as f64 * ANGLE_STEP_DEGREES)
                )
            );
            Slopes {
                precedence,
                offsets: Arc::new(patterns.iter().map(|pattern| pattern.offsets()).collect::<mineflow::Result<Vec<_>>>().map_err(error)?),
                keys: Some(Arc::new(keys)),
                blocks_with_default_angle: cell_steps.iter().filter(|(_, defaulted)| *defaulted).count(),
            }
        }
    };
    let offsets = slopes.offsets.iter().map(Vec::len).max().unwrap_or(0);
    crate::userspace_log!("{}", crate::i18n::tr!("opt-run-pattern", offsets = offsets.to_string(), benches = benches.to_string()));
    Ok(slopes)
}

/// Solve every shell of `plan`.
pub(crate) fn solve(plan: &ShellPlan, grid: &Grid, values: &BlockValues, slopes: &Slopes, cancel: &CancelFlag, progress: &Phase) -> Result<Shells> {
    let precedence = &slopes.precedence;
    let count = plan.count();
    let mut shell_values = ShellValues {
        values,
        plan,
        distance: Vec::new(),
    };
    let cells = grid.cell_count();
    let solves = count.max(1);
    let mut solved = 0usize;

    // The largest shell, on the whole grid.
    let outer: Vec<f64> = (0..cells).into_par_iter().map(|cell| shell_values.value(count, cell)).collect();
    let pit = solve_values(precedence, &outer, cancel)?;
    drop(outer);
    solved += 1;
    progress.set_items(solved as u64, solves as u64);
    let mut label: Vec<u16> = pit.iter().map(|&inside| if inside { count as u16 } else { OUTSIDE }).collect();
    drop(pit);

    if let ShellPlan::Directional { start, front, .. } = plan {
        shell_values.distance = distances(grid, &label, *start, *front);
    }

    // Split the gaps between known shells, one round at a time.
    let mut gaps: Vec<(usize, usize)> = if count >= 2 { vec![(0, count)] } else { Vec::new() };
    while !gaps.is_empty() {
        if cancel.is_cancelled() {
            bail!("Cancelled");
        }
        // Each gap is known by its outer shell, which no other gap shares.
        let mut gap_of_outer = vec![u32::MAX; count + 1];
        for (index, &(_, outer)) in gaps.iter().enumerate() {
            gap_of_outer[outer] = index as u32;
        }
        let mut bands: Vec<Vec<u32>> = vec![Vec::new(); gaps.len()];
        let mut local = vec![u32::MAX; cells];
        for (cell, &shell) in label.iter().enumerate() {
            let gap = gap_of_outer[shell as usize];
            if shell != OUTSIDE && gap != u32::MAX {
                let band = &mut bands[gap as usize];
                local[cell] = band.len() as u32;
                band.push(cell as u32);
            }
        }
        let local = Arc::new(local);
        let snapshot = Arc::new(label.clone());

        let results: Vec<Result<(usize, Vec<u32>)>> = gaps
            .par_iter()
            .zip(bands.into_par_iter())
            .map(|(&(inner, outer), band)| {
                let middle = (inner + outer) / 2;
                if band.is_empty() {
                    return Ok((middle, Vec::new()));
                }
                let band_values: Vec<f64> = band.iter().map(|&cell| shell_values.value(middle, cell as usize)).collect();
                let band = Arc::new(band);
                let source = Band {
                    dims: grid.dims.map(|dim| dim as i64),
                    offsets: Arc::clone(&slopes.offsets),
                    keys: slopes.keys.clone(),
                    cells: Arc::clone(&band),
                    local: Arc::clone(&local),
                    label: Arc::clone(&snapshot),
                    outer: outer as u16,
                };
                let precedence = Precedence::from_source(source).map_err(|error| anyhow!("{error}"))?;
                let pit = solve_values(&precedence, &band_values, cancel)?;
                let inside = pit.iter().zip(band.iter()).filter(|(inside, _)| **inside).map(|(_, &cell)| cell).collect();
                Ok((middle, inside))
            })
            .collect();
        for result in results {
            let (middle, inside) = result?;
            for cell in inside {
                label[cell as usize] = middle as u16;
            }
            solved += 1;
        }
        progress.set_items(solved.min(solves) as u64, solves as u64);
        gaps = gaps
            .iter()
            .flat_map(|&(inner, outer)| {
                let middle = (inner + outer) / 2;
                [(inner, middle), (middle, outer)]
            })
            .filter(|(inner, outer)| outer - inner >= 2)
            .collect();
    }

    let summaries = (1..=count)
        .map(|shell| {
            (0..cells)
                .into_par_iter()
                .filter(|&cell| label[cell] != OUTSIDE && label[cell] as usize <= shell && !values.air[cell])
                .fold(ShellSummary::default, |mut summary, cell| {
                    let tonnes = values.tonnes[cell];
                    summary.blocks += 1;
                    summary.tonnes += tonnes;
                    if shell_values.is_ore(shell, cell) {
                        summary.ore_tonnes += tonnes;
                    } else {
                        summary.waste_tonnes += tonnes;
                    }
                    summary.value += shell_values.value(shell, cell);
                    summary
                })
                .reduce(ShellSummary::default, |a, b| ShellSummary {
                    factor: 0.0,
                    distance: None,
                    blocks: a.blocks + b.blocks,
                    tonnes: a.tonnes + b.tonnes,
                    ore_tonnes: a.ore_tonnes + b.ore_tonnes,
                    waste_tonnes: a.waste_tonnes + b.waste_tonnes,
                    value: a.value + b.value,
                })
        })
        .zip(1..=count)
        .map(|(summary, shell)| ShellSummary {
            factor: shell_values.revenue_factor(shell),
            distance: shell_values.distance_share(shell),
            ..summary
        })
        .collect();
    Ok(Shells { label, summaries })
}

/// The smallest optimal pit for `values` over `precedence`.
fn solve_values(precedence: &Precedence, values: &[f64], cancel: &CancelFlag) -> Result<Vec<bool>> {
    let scale = ValueScale::auto(values).map_err(|error| anyhow!("{error}"))?;
    let encoded = scale.encode(values).map_err(|error| anyhow!("{error}"))?;
    let mut solver = Solver::new(precedence, &encoded).map_err(|error| anyhow!("{error}"))?;
    match solver.solve_with_cancel(|| cancel.is_cancelled()) {
        Ok(_) => {}
        Err(mineflow::Error::Cancelled) => bail!("Cancelled"),
        Err(error) => bail!("{error}"),
    }
    Ok(solver.pit().map_err(|error| anyhow!("{error}"))?.into_vec())
}

/// Each cell's share of the way across the final pit as the front advances
/// from `start`: 0 at the pit cell the front reaches first, 1 at the last. A
/// straight front measures along its bearing (cells behind the start count as
/// at it); a radial one measures the plan distance from the point.
fn distances(grid: &Grid, label: &[u16], start: DVec2, front: Front) -> Vec<f64> {
    let direction = match front {
        Front::Straight { bearing } => Some(DVec2::new(bearing.to_radians().sin(), bearing.to_radians().cos())),
        Front::Radial => None,
    };
    let along = |cell: usize| {
        let offset = grid.centroid(cell).truncate() - start;
        direction.map_or_else(|| offset.length(), |direction| offset.dot(direction).max(0.0))
    };
    let (nearest, furthest) = (0..grid.cell_count())
        .into_par_iter()
        .filter(|&cell| label[cell] != OUTSIDE)
        .map(|cell| {
            let distance = along(cell);
            (distance, distance)
        })
        .reduce(|| (f64::INFINITY, f64::NEG_INFINITY), |a, b| (a.0.min(b.0), a.1.max(b.1)));
    let span = furthest - nearest;
    (0..grid.cell_count())
        .into_par_iter()
        .map(|cell| if span > 0.0 { ((along(cell) - nearest) / span).clamp(0.0, 1.0) } else { 0.0 })
        .collect()
}

/// The precedence among one band's cells: the slope pattern on the grid, kept
/// to cells of the same band.
struct Band {
    dims: [i64; 3],
    offsets: Arc<Vec<Vec<Offset>>>,
    /// Which of `offsets` each grid cell uses; `None` when all use the first.
    keys: Option<Arc<Vec<u16>>>,
    /// The band's cells, by local index.
    cells: Arc<Vec<u32>>,
    /// Local index of every cell of this round's bands.
    local: Arc<Vec<u32>>,
    /// The labels when the round began; the band is the cells labelled `outer`.
    label: Arc<Vec<u16>>,
    outer: u16,
}

impl PrecedenceSource for Band {
    fn num_blocks(&self) -> i64 {
        self.cells.len() as i64
    }

    fn antecedents(&self, block: i64, out: &mut Vec<i64>) {
        let [nx, ny, nz] = self.dims;
        let cell = i64::from(self.cells[block as usize]);
        let (x, y, z) = (cell % nx, (cell / nx) % ny, cell / (nx * ny));
        let pattern = self.keys.as_ref().map_or(0, |keys| keys[cell as usize] as usize);
        for &(dx, dy, dz) in &self.offsets[pattern] {
            let (x, y, z) = (x + dx, y + dy, z + dz);
            if x < 0 || y < 0 || z < 0 || x >= nx || y >= ny || z >= nz {
                continue;
            }
            let neighbour = (x + nx * (y + ny * z)) as usize;
            if self.label[neighbour] == self.outer {
                out.push(i64::from(self.local[neighbour]));
            }
        }
    }
}
