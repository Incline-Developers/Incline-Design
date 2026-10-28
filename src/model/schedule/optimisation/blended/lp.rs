//! The linear programs the hourly dispatch solves, one per interval, and the
//! solver they go to.
//!
//! The dispatch only builds a [`LinearProgram`]: columns with an objective
//! weight and bounds, rows with bounds over a few columns. [`LinearProgram::maximise_from`]
//! hands it to whichever backend the build has, so the same schedule logic
//! runs natively and in the browser.

use std::ops::{Bound, RangeBounds};

/// A column of a [`LinearProgram`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Col(usize);

impl Col {
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Default)]
pub(crate) struct LinearProgram {
    objective: Vec<f64>,
    column_bounds: Vec<(f64, f64)>,
    rows: Vec<Row>,
}

#[derive(Clone)]
struct Row {
    lower: f64,
    upper: f64,
    terms: Vec<(Col, f64)>,
}

fn bounds(range: impl RangeBounds<f64>) -> (f64, f64) {
    let lower = match range.start_bound() {
        Bound::Included(&value) | Bound::Excluded(&value) => value,
        Bound::Unbounded => f64::NEG_INFINITY,
    };
    let upper = match range.end_bound() {
        Bound::Included(&value) | Bound::Excluded(&value) => value,
        Bound::Unbounded => f64::INFINITY,
    };
    (lower, upper)
}

impl LinearProgram {
    pub(crate) fn add_column(&mut self, objective: f64, range: impl RangeBounds<f64>) -> Col {
        self.objective.push(objective);
        self.column_bounds.push(bounds(range));
        Col(self.objective.len() - 1)
    }

    pub(crate) fn add_row(&mut self, range: impl RangeBounds<f64>, terms: Vec<(Col, f64)>) {
        let (lower, upper) = bounds(range);
        self.rows.push(Row { lower, upper, terms });
    }

    /// The objective at `solution`'s column values.
    pub(crate) fn value(&self, solution: &[f64]) -> f64 {
        self.objective.iter().zip(solution).map(|(weight, value)| weight * value).sum()
    }

    /// The column values at a maximum of the objective, or why there are
    /// none, starting from the basis the last program of the same shape in
    /// `warm` ended on, and leaving this one's there.
    pub(crate) fn maximise_from(&mut self, warm: &mut WarmStarts) -> Result<Vec<f64>, String> {
        self.tidy()?;
        #[cfg(not(target_arch = "wasm32"))]
        self.dump();
        self.maximise_solver(warm)
    }

    /// Maximise with the interior point method, crossover and a simplex
    /// finish, stopping when `stop` says so. For one large program, where
    /// the simplex method's many degenerate pivots cost far more than the
    /// interior point method's few factorisations.
    pub(crate) fn maximise_ipm(&mut self, stop: Option<solver::Interrupt>) -> Result<Vec<f64>, String> {
        self.tidy()?;
        #[cfg(not(target_arch = "wasm32"))]
        self.dump();
        // Crossover takes the point to an exact vertex, so the interior
        // point method need not close the last digits of its gap: on these
        // programs it can stall there, and a stall falls back on a simplex
        // solve from scratch.
        let options = solver::Options {
            solver: solver::Solver::Ipm,
            ipm_optimality_tolerance: 1e-6,
            ..solver::Options::default()
        };
        let solution = solver::solve_from(&self.to_solver(), &options, None, stop);
        if solution.status != solver::Status::Optimal {
            return Err(format!("solver ended {:?}", solution.status));
        }
        Ok(solution.col_value)
    }

    /// What a basis carries over between programs: their columns, and each
    /// row's columns. Bounds, weights and coefficients may differ.
    fn shape(&self) -> u64 {
        use std::hash::{BuildHasher, Hash, Hasher};
        let mut hasher = foldhash::fast::FixedState::default().build_hasher();
        self.objective.len().hash(&mut hasher);
        for row in &self.rows {
            row.terms.len().hash(&mut hasher);
            for (col, _) in &row.terms {
                col.0.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// This program as a [`solver::Problem`].
    fn to_solver(&self) -> solver::Problem {
        let mut problem = solver::Problem::new(solver::Sense::Maximise);
        for (&weight, &(lower, upper)) in self.objective.iter().zip(&self.column_bounds) {
            problem.add_col(weight, lower, upper);
        }
        for row in &self.rows {
            problem.add_row(row.lower, row.upper, row.terms.iter().map(|&(col, coefficient)| (col.0, coefficient)));
        }
        problem
    }

    /// With `INCLINE_LP_DUMP` set to a directory, write this program there
    /// as MPS, for benchmarking solvers on the programs real schedules make.
    #[cfg(not(target_arch = "wasm32"))]
    fn dump(&self) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Read once: the dispatch solves thousands of programs.
        static DIR: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
        let Some(dir) = DIR.get_or_init(|| std::env::var_os("INCLINE_LP_DUMP")) else {
            return;
        };
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::path::Path::new(&dir).join(format!("dispatch-{}-{number:06}.mps", std::process::id()));
        if let Err(error) = std::fs::write(&path, solver::mps::write(&self.to_solver())) {
            log::warn!("could not write {}: {error}", path.display());
        }
    }

    /// The same program with what a simplex basis chokes on taken out: a
    /// column repeated within a row is one term, zero terms are dropped, a
    /// row with no terms left is checked and dropped, and rows over exactly
    /// the same terms - a reclaim cap and the equality that spends it, a dig
    /// pin and its block's total - are one row with the tighter bounds.
    ///
    /// Two parallel rows are the classic way to a basis that is singular
    /// apart from rounding.
    fn tidy(&mut self) -> Result<(), String> {
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut rows: Vec<Row> = Vec::with_capacity(self.rows.len());
        let state = foldhash::fast::FixedState::default();
        let mut seen: std::collections::HashMap<u64, usize, foldhash::fast::FixedState> = std::collections::HashMap::with_capacity_and_hasher(self.rows.len(), state.clone());
        let mut previous: Vec<Option<usize>> = Vec::with_capacity(self.rows.len());
        for mut row in std::mem::take(&mut self.rows) {
            let terms = &mut row.terms;
            terms.sort_by_key(|(col, _)| col.0);
            terms.dedup_by(|next, held| {
                if next.0 == held.0 {
                    held.1 += next.1;
                    true
                } else {
                    false
                }
            });
            if let Some((col, coefficient)) = terms.iter().find(|(_, coefficient)| !coefficient.is_finite()) {
                return Err(format!("a constraint has coefficient {coefficient} on column {}", col.0));
            }
            terms.retain(|(_, coefficient)| *coefficient != 0.0);
            if terms.is_empty() {
                // 0 has to lie within the row's bounds, or nothing can. Each
                // side is held to its own tolerance: a distant bound on one
                // side says nothing about how close the other must be.
                let tolerance = |bound: f64| 1e-9 * bound.abs().max(1.0);
                if row.lower.is_nan() || row.upper.is_nan() || row.lower > tolerance(row.lower) || row.upper < -tolerance(row.upper) {
                    return Err(format!("a constraint with no terms needs {}..{}", row.lower, row.upper));
                }
                continue;
            }
            // Keep only the hash in the index: copying every row's terms
            // into a key costs an allocation on every hourly solve. Check
            // actual terms along the collision chain before merging bounds.
            let mut hasher = state.build_hasher();
            terms.len().hash(&mut hasher);
            for (col, coefficient) in terms.iter() {
                col.0.hash(&mut hasher);
                coefficient.to_bits().hash(&mut hasher);
            }
            let key = hasher.finish();
            let mut found = seen.get(&key).copied();
            while let Some(index) = found {
                if rows[index].terms.len() == terms.len() && rows[index].terms.iter().zip(terms.iter()).all(|((a, x), (b, y))| a == b && x.to_bits() == y.to_bits()) {
                    break;
                }
                found = previous[index];
            }
            match found {
                Some(index) => {
                    let held = &mut rows[index];
                    held.lower = held.lower.max(row.lower);
                    held.upper = held.upper.min(row.upper);
                    // Equal in all but rounding: one equality, not an
                    // infeasible pair.
                    if held.lower > held.upper {
                        let slack = 1e-9 * held.lower.abs().max(held.upper.abs()).max(1.0);
                        if held.lower - held.upper > slack {
                            return Err(format!("two constraints over the same terms need {} and {}", held.lower, held.upper));
                        }
                        held.upper = held.lower;
                    }
                }
                None => {
                    previous.push(seen.insert(key, rows.len()));
                    rows.push(row);
                }
            }
        }
        self.rows = rows;
        Ok(())
    }

    /// Solve with the `solver` crate, HiGHS's presolve and dual simplex method in Rust,
    /// which takes the same steps HiGHS does.
    fn maximise_solver(&self, warm: &mut WarmStarts) -> Result<Vec<f64>, String> {
        let shape = self.shape();
        let problem = self.to_solver();
        let options = solver::Options::default();
        let mut solution = match warm.take(shape) {
            Some(basis) => {
                let found = solver::solve_from(&problem, &options, Some(&basis), None);
                // A basis that does not suit this program is no reason to
                // fail it: solve afresh.
                if found.status == solver::Status::Optimal {
                    found
                } else {
                    solver::solve(&problem, &options)
                }
            }
            None => solver::solve(&problem, &options),
        };
        if solution.status != solver::Status::Optimal {
            return Err(format!("solver ended {:?}", solution.status));
        }
        if let Some(basis) = solution.basis.take() {
            warm.keep(shape, basis);
        }
        Ok(solution.col_value)
    }
}

/// The bases the last few programs of each shape ended on, most recent
/// first, for [`LinearProgram::maximise_from`]. The hourly dispatch solves
/// much the same program interval after interval, with new bounds: started
/// from the last one's basis, the solve skips presolve and takes a few
/// iterations instead of tens.
#[derive(Default)]
pub(crate) struct WarmStarts {
    bases: Vec<(u64, solver::Basis)>,
}

impl WarmStarts {
    /// Day boundaries, crusher limits and segment refinements cycle through
    /// more than a few shapes. Keep their bases across those transitions so
    /// returning to a shape does not need another cold solve.
    const HELD: usize = 32;

    fn take(&mut self, shape: u64) -> Option<solver::Basis> {
        let at = self.bases.iter().position(|(held, _)| *held == shape)?;
        Some(self.bases.remove(at).1)
    }

    fn keep(&mut self, shape: u64, basis: solver::Basis) {
        self.bases.retain(|(held, _)| *held != shape);
        self.bases.insert(0, (shape, basis));
        self.bases.truncate(Self::HELD);
    }
}

/// The solver the hourly dispatch uses in this build, for the run's report.
pub(crate) fn backend_name() -> &'static str {
    "solver"
}
