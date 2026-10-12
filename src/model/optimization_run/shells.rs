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
//! solved side by side. Each solve is itself split into parallel regions; see
//! [`sweep`].

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
    /// Rock blocks in the shell's pit (cumulative). Tonnes, costs and value
    /// are in the run's report, at the base price.
    pub(crate) blocks: usize,
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
    let count = plan.count();
    let mut shell_values = ShellValues {
        values,
        plan,
        distance: Vec::new(),
    };
    let cells = grid.cell_count();
    let lattice = slopes.lattice(grid);
    let solves = count.max(1);
    let mut solved = 0usize;

    // The largest shell, on the whole grid.
    let outer: Vec<f64> = (0..cells).into_par_iter().map(|cell| shell_values.value(count, cell)).collect();
    let pit = sweep(&Domain::grid(cells), &lattice, Some(&slopes.precedence), &outer, cancel)?;
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
                let domain = Domain {
                    cells: Arc::new(band),
                    local: Arc::clone(&local),
                    band: Some((Arc::clone(&snapshot), outer as u16)),
                };
                let pit = sweep(&domain, &lattice, None, &band_values, cancel)?;
                let inside = pit.iter().zip(domain.cells.iter()).filter(|(inside, _)| **inside).map(|(_, &cell)| cell).collect();
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

    // Blocks first held by each shell, then summed into each shell's pit.
    let mut first_held = vec![0usize; count + 1];
    for (cell, &shell) in label.iter().enumerate() {
        if shell != OUTSIDE && !values.air[cell] {
            first_held[usize::from(shell)] += 1;
        }
    }
    let mut blocks = 0;
    let summaries = (1..=count)
        .map(|shell| {
            blocks += first_held[shell];
            ShellSummary {
                factor: shell_values.revenue_factor(shell),
                distance: shell_values.distance_share(shell),
                blocks,
            }
        })
        .collect();
    Ok(Shells { label, summaries })
}

/// The smallest optimal pit for values in solver units.
fn solve_encoded(precedence: &Precedence, encoded: &[i64], cancel: &CancelFlag) -> Result<Vec<bool>> {
    let mut solver = Solver::new(precedence, encoded).map_err(|error| anyhow!("{error}"))?;
    match solver.solve_with_cancel(|| cancel.is_cancelled()) {
        Ok(_) => {}
        Err(mineflow::Error::Cancelled) => bail!("Cancelled"),
        Err(error) => bail!("{error}"),
    }
    Ok(solver.pit().map_err(|error| anyhow!("{error}"))?.into_vec())
}

/// Levels per round of a [`sweep`], and the side of its tiles in cells. Tuned
/// on a 16M cell, ten shell model.
const SWEEP_LEVELS: usize = 8;
const SWEEP_TILE: usize = 48;
/// Once the regions have held as many cells as the domain, a round that
/// settles fewer cells than this share of the cells its regions held ends the
/// sweep: what is left is ore too deep or too poor to pay for its own cone,
/// and cheaper to leave to the final solve.
const SWEEP_MIN_YIELD: f64 = 0.02;

/// The smallest optimal pit for `values` (one per domain cell) on `domain`.
/// `whole` is the grid's own precedence, when `domain` is the whole grid.
///
/// A region holding every antecedent of its cells is closed, and the smallest
/// optimal pit `A` of a closed region lies inside the domain's, `B`: `A ∩ B`
/// is a pit of the region worth `v(A) + v(B) - v(A ∪ B) >= v(A)`, as `B` is
/// optimal, so it is optimal too and, `A` being the smallest, equal to `A`. So
/// the domain is swept from the top in bands of levels, each cut into tiles,
/// and each tile solves the cone of its unsettled positive cells in parallel;
/// what those pits mine is settled. Settled cells are a closed set already
/// paid for, so later cones stop at them and stay small, and the argument
/// holds on the graph that remains. One final solve over the unsettled cells
/// is exact for the same reason. Every solve uses the one encoding of
/// `values`, so rounding cannot set them apart.
///
/// Small solves are also far kinder to the cache than one large one, so this
/// is quicker even on a single thread.
fn sweep(domain: &Domain, lattice: &Lattice, whole: Option<&Precedence>, values: &[f64], cancel: &CancelFlag) -> Result<Vec<bool>> {
    let scale = ValueScale::auto(values).map_err(|error| anyhow!("{error}"))?;
    let encoded = scale.encode(values).map_err(|error| anyhow!("{error}"))?;
    let len = domain.cells.len();
    let [nx, ny, _] = lattice.dims.map(|dim| dim as usize);
    // The domain's bounds, so a small band does not scan the whole grid.
    let (mut lower, mut upper) = ([usize::MAX; 3], [0; 3]);
    for &cell in domain.cells.iter() {
        let cell = cell as usize;
        let at = [cell % nx, (cell / nx) % ny, cell / (nx * ny)];
        for axis in 0..3 {
            lower[axis] = lower[axis].min(at[axis]);
            upper[axis] = upper[axis].max(at[axis] + 1);
        }
    }
    let tiles: Vec<(usize, usize)> = (lower[1]..upper[1])
        .step_by(SWEEP_TILE)
        .flat_map(|y| (lower[0]..upper[0]).step_by(SWEEP_TILE).map(move |x| (x, y)))
        .collect();

    let mut settled = Arc::new(vec![false; len]);
    let (mut settled_count, mut spent) = (0usize, 0usize);
    let mut top = upper[2];
    while top > lower[2] {
        if cancel.is_cancelled() {
            bail!("Cancelled");
        }
        let bottom = top.saturating_sub(SWEEP_LEVELS).max(lower[2]);
        let found: Vec<Result<(usize, Vec<u32>)>> = tiles
            .par_iter()
            .map_init(
                || CellSet::new(len),
                |seen, &(x0, y0)| {
                    let (x1, y1) = ((x0 + SWEEP_TILE).min(upper[0]), (y0 + SWEEP_TILE).min(upper[1]));
                    // The tile's unsettled positive cells, then everything they need.
                    let mut region = Vec::new();
                    for z in bottom..top {
                        for y in y0..y1 {
                            for x in x0..x1 {
                                if let Some(index) = domain.index(x + nx * (y + ny * z))
                                    && encoded[index] > 0
                                    && !settled[index]
                                    && seen.insert(index)
                                {
                                    region.push(index as u32);
                                }
                            }
                        }
                    }
                    let mut next = 0;
                    while next < region.len() {
                        lattice.antecedents(domain.cells[region[next] as usize] as usize, |cell| {
                            if let Some(index) = domain.index(cell)
                                && !settled[index]
                                && seen.insert(index)
                            {
                                region.push(index as u32);
                            }
                        });
                        next += 1;
                    }
                    for &index in &region {
                        seen.remove(index as usize);
                    }
                    if region.is_empty() {
                        return Ok((0, Vec::new()));
                    }
                    region.sort_unstable();
                    let held = region.len();
                    let region = Region {
                        lattice: lattice.clone(),
                        domain: domain.clone(),
                        cells: Arc::new(region),
                        dense: None,
                        settled: Arc::clone(&settled),
                    };
                    Ok((held, solve_region(region, &encoded, cancel)?))
                },
            )
            .collect();
        let settled_now = Arc::get_mut(&mut settled).expect("regions are dropped");
        let (mut held, mut gained) = (0usize, 0usize);
        for result in found {
            let (region, mined) = result?;
            held += region;
            for index in mined {
                gained += usize::from(!std::mem::replace(&mut settled_now[index as usize], true));
            }
        }
        settled_count += gained;
        spent += held;
        if spent > len && (gained as f64) < SWEEP_MIN_YIELD * held as f64 {
            break;
        }
        top = bottom;
    }

    if settled_count == 0
        && let Some(whole) = whole
    {
        return solve_encoded(whole, &encoded, cancel);
    }
    let mut dense = vec![u32::MAX; len];
    let mut rest = Vec::with_capacity(len - settled_count);
    for (index, &done) in settled.iter().enumerate() {
        if !done {
            dense[index] = rest.len() as u32;
            rest.push(index as u32);
        }
    }
    let region = Region {
        lattice: lattice.clone(),
        domain: domain.clone(),
        cells: Arc::new(rest),
        dense: Some(Arc::new(dense)),
        settled: Arc::clone(&settled),
    };
    let mined = solve_region(region, &encoded, cancel)?;
    let mut pit = Arc::try_unwrap(settled).expect("regions are dropped");
    for index in mined {
        pit[index as usize] = true;
    }
    Ok(pit)
}

/// The cells `region`'s smallest optimal pit mines, by domain index.
fn solve_region(region: Region, encoded: &[i64], cancel: &CancelFlag) -> Result<Vec<u32>> {
    let values: Vec<i64> = region.cells.iter().map(|&index| encoded[index as usize]).collect();
    if values.iter().all(|&value| value <= 0) {
        return Ok(Vec::new());
    }
    let cells = Arc::clone(&region.cells);
    let precedence = Precedence::from_source(region).map_err(|error| anyhow!("{error}"))?;
    let pit = solve_encoded(&precedence, &values, cancel)?;
    Ok(pit.iter().zip(cells.iter()).filter(|(inside, _)| **inside).map(|(_, &index)| index).collect())
}

/// A set of domain cells as one bit each, cleared cell by cell so it can be
/// reused without touching the whole domain.
struct CellSet(Vec<u64>);

impl CellSet {
    fn new(cells: usize) -> Self {
        Self(vec![0; cells.div_ceil(64)])
    }

    /// Adds `cell`, returning whether it was absent.
    fn insert(&mut self, cell: usize) -> bool {
        let (word, bit) = (cell / 64, 1u64 << (cell % 64));
        let absent = self.0[word] & bit == 0;
        self.0[word] |= bit;
        absent
    }

    fn remove(&mut self, cell: usize) {
        self.0[cell / 64] &= !(1u64 << (cell % 64));
    }
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

/// The slope patterns laid on the grid.
#[derive(Clone)]
struct Lattice {
    dims: [i64; 3],
    offsets: Arc<Vec<Vec<Offset>>>,
    /// Which of `offsets` each grid cell uses; `None` when all use the first.
    keys: Option<Arc<Vec<u16>>>,
}

impl Lattice {
    /// Calls `visit` with each grid cell `cell` needs mined first.
    fn antecedents(&self, cell: usize, mut visit: impl FnMut(usize)) {
        let [nx, ny, nz] = self.dims;
        let index = cell as i64;
        let (x, y, z) = (index % nx, (index / nx) % ny, index / (nx * ny));
        let pattern = self.keys.as_ref().map_or(0, |keys| keys[cell] as usize);
        for &(dx, dy, dz) in &self.offsets[pattern] {
            let (x, y, z) = (x + dx, y + dy, z + dz);
            if x < 0 || y < 0 || z < 0 || x >= nx || y >= ny || z >= nz {
                continue;
            }
            visit((x + nx * (y + ny * z)) as usize);
        }
    }
}

impl Slopes {
    fn lattice(&self, grid: &Grid) -> Lattice {
        Lattice {
            dims: grid.dims.map(|dim| dim as i64),
            offsets: Arc::clone(&self.offsets),
            keys: self.keys.clone(),
        }
    }
}

/// The cells one solve covers: the whole grid, or one band of it.
#[derive(Clone)]
struct Domain {
    /// Grid index of each domain cell, ascending.
    cells: Arc<Vec<u32>>,
    /// Domain index of every grid cell, or `u32::MAX`. A round's bands share
    /// one, so it is only meaningful together with `band`.
    local: Arc<Vec<u32>>,
    /// For a band: the labels when its round began, and its own label.
    band: Option<(Arc<Vec<u16>>, u16)>,
}

impl Domain {
    fn grid(cells: usize) -> Self {
        let all = Arc::new((0..cells as u32).collect::<Vec<_>>());
        Self {
            cells: Arc::clone(&all),
            local: all,
            band: None,
        }
    }

    /// The domain index of grid cell `cell`, if the domain holds it. Every
    /// other cell a domain cell needs is already mined: a band lies inside a
    /// closed outer shell, around an inner one.
    fn index(&self, cell: usize) -> Option<usize> {
        if let Some((label, own)) = &self.band
            && label[cell] != *own
        {
            return None;
        }
        let index = self.local[cell];
        (index != u32::MAX).then_some(index as usize)
    }
}

/// The precedence among a closed set of domain cells: the slope pattern on the
/// grid, less the cells outside the domain or settled, which are mined.
struct Region {
    lattice: Lattice,
    domain: Domain,
    /// The region's domain indices, ascending; a block's index is its position.
    cells: Arc<Vec<u32>>,
    /// The block of every domain index, for a region too large to search.
    dense: Option<Arc<Vec<u32>>>,
    settled: Arc<Vec<bool>>,
}

impl PrecedenceSource for Region {
    fn num_blocks(&self) -> i64 {
        self.cells.len() as i64
    }

    fn antecedents(&self, block: i64, out: &mut Vec<i64>) {
        let cell = self.domain.cells[self.cells[block as usize] as usize] as usize;
        self.lattice.antecedents(cell, |cell| {
            let Some(index) = self.domain.index(cell) else {
                return;
            };
            if self.settled[index] {
                return;
            }
            let block = match &self.dense {
                Some(dense) => dense[index] as usize,
                None => self.cells.binary_search(&(index as u32)).expect("a region holds every unsettled antecedent"),
            };
            out.push(block as i64);
        });
    }
}
