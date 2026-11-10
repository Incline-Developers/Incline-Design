//! The linear programs the hourly dispatch solves, one per interval, and the
//! solver they go to.
//!
//! The dispatch only builds a [`LinearProgram`]: columns with an objective
//! weight and bounds, rows with bounds over a few columns. [`LinearProgram::maximise_from`]
//! hands it to whichever backend the build has, so the same schedule logic
//! runs natively and in the browser.

use std::{
    hash::{BuildHasher, Hash, Hasher},
    ops::{Bound, RangeBounds},
};

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
    /// What each column stands for, where its builder said: how
    /// [`WarmStarts`] carries a basis to a program of another shape.
    keys: Vec<Option<u64>>,
}

#[derive(Clone)]
struct Row {
    lower: f64,
    upper: f64,
    terms: Vec<(Col, f64)>,
    /// What the row stands for, where its builder said; see `keys`.
    key: Option<u64>,
}

fn key_of(key: impl Hash) -> u64 {
    foldhash::fast::FixedState::default().hash_one(key)
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
        self.keys.push(None);
        Col(self.objective.len() - 1)
    }

    /// [`Self::add_column`] for a column that stands for `key`: the same
    /// thing in the next program, whatever else has changed, goes by the
    /// same key.
    pub(crate) fn add_keyed_column(&mut self, key: impl Hash, objective: f64, range: impl RangeBounds<f64>) -> Col {
        let col = self.add_column(objective, range);
        self.keys[col.0] = Some(key_of(key));
        col
    }

    pub(crate) fn add_row(&mut self, range: impl RangeBounds<f64>, terms: Vec<(Col, f64)>) {
        let (lower, upper) = bounds(range);
        self.rows.push(Row { lower, upper, terms, key: None });
    }

    /// [`Self::add_row`] for a row that stands for `key`, like
    /// [`Self::add_keyed_column`]: the same limit in the next program goes
    /// by the same key, whichever columns it then holds.
    pub(crate) fn add_keyed_row(&mut self, key: impl Hash, range: impl RangeBounds<f64>, terms: Vec<(Col, f64)>) {
        self.add_row(range, terms);
        self.rows.last_mut().expect("just added").key = Some(key_of(key));
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
    /// Maximise by the interior point method, stopping at `ends`.
    pub(crate) fn maximise_ipm(&mut self, ends: web_time::Instant) -> Result<Vec<f64>, String> {
        self.tidy()?;
        #[cfg(not(target_arch = "wasm32"))]
        self.dump();
        // Crossover takes the point to an exact vertex, so the interior
        // point method need not close the last digits of its gap: on these
        // programs it can stall there. A stalled point is crossed over too,
        // so the simplex clean-up starts from its basis rather than from
        // scratch (long's worst window: 108 s to 17 s, the same optimum).
        let options = solver::Options {
            solver: solver::Solver::Ipm,
            ipm_optimality_tolerance: 1e-6,
            ipm_crossover_on_no_progress: true,
            // HiGHS sizes presolve's search for dependent equations by the
            // time limit, and with none it may search for 1000 s: on long's
            // first reclaim window it took 35 s of a 38 s solve, and the
            // solve took 3 s once it knew its time.
            time_limit: ends.saturating_duration_since(web_time::Instant::now()).as_secs_f64(),
            ..solver::Options::default()
        };
        let stop: solver::Interrupt = std::sync::Arc::new(move || web_time::Instant::now() >= ends);
        let solution = solver::solve_from(&self.to_solver(), &options, None, Some(stop));
        if solution.status != solver::Status::Optimal {
            return Err(crate::i18n::tr!("err-sched-solver-ended", status = format!("{:?}", solution.status)));
        }
        Ok(solution.col_value)
    }

    /// What a basis carries over between programs: their columns, and each
    /// row's columns. Bounds, weights and coefficients may differ.
    fn shape(&self) -> u64 {
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

    /// Every column's key and every row's, or `None` while a column has no
    /// key. A row without a key of its own goes by the keys of its columns:
    /// after [`Self::tidy`] two rows over the same columns differ only in
    /// their coefficients, and are told apart by their order. So do rows
    /// given the same key.
    fn keys(&self) -> Option<Keys> {
        let cols: Vec<u64> = self.keys.iter().copied().collect::<Option<_>>()?;
        let state = foldhash::fast::FixedState::default();
        let mut seen: std::collections::HashMap<u64, u64, foldhash::fast::FixedState> = std::collections::HashMap::with_capacity_and_hasher(self.rows.len(), state.clone());
        let mut members = Vec::new();
        let rows = self
            .rows
            .iter()
            .map(|row| {
                let mut hasher = state.build_hasher();
                match row.key {
                    Some(key) => key.hash(&mut hasher),
                    None => {
                        members.clear();
                        members.extend(row.terms.iter().map(|(col, _)| cols[col.0]));
                        members.sort_unstable();
                        members.hash(&mut hasher);
                    }
                }
                let repeat = seen.entry(hasher.finish()).or_insert(0);
                repeat.hash(&mut hasher);
                *repeat += 1;
                hasher.finish()
            })
            .collect();
        Some(Keys { cols, rows })
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
                return Err(crate::i18n::tr!(
                    "err-sched-bad-coefficient",
                    coefficient = coefficient.to_string(),
                    column = col.0.to_string()
                ));
            }
            terms.retain(|(_, coefficient)| *coefficient != 0.0);
            if terms.is_empty() {
                // 0 has to lie within the row's bounds, or nothing can. Each
                // side is held to its own tolerance: a distant bound on one
                // side says nothing about how close the other must be.
                let tolerance = |bound: f64| 1e-9 * bound.abs().max(1.0);
                if row.lower.is_nan() || row.upper.is_nan() || row.lower > tolerance(row.lower) || row.upper < -tolerance(row.upper) {
                    return Err(crate::i18n::tr!("err-sched-empty-constraint", lower = row.lower.to_string(), upper = row.upper.to_string()));
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
                            return Err(crate::i18n::tr!(
                                "err-sched-conflicting-constraints",
                                lower = held.lower.to_string(),
                                upper = held.upper.to_string()
                            ));
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
        let keys = self.keys();
        // From a basis the matrix goes unscaled, and the dual method prices
        // by the plain rule: on programs this small, scaling and setting up
        // steepest-edge weights cost more than the pivots they save.
        let light = solver::Options {
            simplex_scale_strategy: solver::SCALE_STRATEGY_OFF,
            dual_edge_weight_strategy: solver::EdgeWeightStrategy::Dantzig,
            ..solver::Options::default()
        };
        let found = match warm.take(shape) {
            Some(basis) => Some(solver::solve_from(&problem, &light, Some(&basis), None)),
            // A shape not held: the last program's basis, carried over by
            // what its columns and rows stand for. Most of an interval's
            // program is the one before it, so this is a few pivots where a
            // solve from scratch is a presolve and tens of pivots. By the
            // primal method: with the program's new columns the basis is
            // no longer optimal in the dual's sense, which the dual method
            // all but solves afresh to repair.
            None => keys.as_ref().and_then(|keys| warm.carried(keys, &problem)).map(|basis| {
                let primal = solver::Options {
                    simplex_strategy: solver::SimplexStrategy::Primal,
                    ..light.clone()
                };
                solver::solve_from_alien(&problem, &primal, &basis)
            }),
        };
        // A basis that does not suit this program is no reason to fail it:
        // solve afresh, presolved and scaled.
        let mut solution = match found {
            Some(found) if found.status == solver::Status::Optimal => found,
            _ => solver::solve(&problem, &solver::Options::default()),
        };
        if solution.status != solver::Status::Optimal {
            return Err(crate::i18n::tr!("err-sched-solver-ended", status = format!("{:?}", solution.status)));
        }
        if let Some(basis) = solution.basis.take() {
            if let Some(keys) = keys {
                warm.last = Some((keys, basis.clone()));
            }
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
    /// The last program solved with every column keyed, and its basis.
    last: Option<(Keys, solver::Basis)>,
}

/// What each column and each row of a program stands for.
struct Keys {
    cols: Vec<u64>,
    rows: Vec<u64>,
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

    /// The last program's basis for `problem`, whose columns and rows are
    /// `keys`: each variable it shares with the last program keeps its
    /// status, and the rest start nonbasic for the solver to place. Missing
    /// basic variables the solver makes up with slacks; surplus ones, which
    /// it would refuse, become nonbasic, rows' slacks first.
    fn carried(&self, keys: &Keys, problem: &solver::Problem) -> Option<solver::Basis> {
        use solver::BasisStatus;
        let (held, basis) = self.last.as_ref()?;
        let state = foldhash::fast::FixedState::default();
        let index = |keys: &[u64], statuses: &[BasisStatus]| {
            let mut map: std::collections::HashMap<u64, BasisStatus, foldhash::fast::FixedState> = std::collections::HashMap::with_capacity_and_hasher(keys.len(), state.clone());
            map.extend(keys.iter().copied().zip(statuses.iter().copied()));
            map
        };
        let (held_cols, held_rows) = (index(&held.cols, &basis.col_status), index(&held.rows, &basis.row_status));
        // A nonbasic status names a bound, which this program may not have.
        let fit = |status: BasisStatus, lower: f64, upper: f64| match status {
            BasisStatus::Basic => BasisStatus::Basic,
            BasisStatus::Lower if lower.is_finite() => BasisStatus::Lower,
            BasisStatus::Upper if upper.is_finite() => BasisStatus::Upper,
            BasisStatus::Zero if lower == f64::NEG_INFINITY && upper == f64::INFINITY => BasisStatus::Zero,
            _ => BasisStatus::Nonbasic,
        };
        let carry = |keys: &[u64], held: &std::collections::HashMap<u64, BasisStatus, foldhash::fast::FixedState>, lower: &[f64], upper: &[f64]| -> Vec<BasisStatus> {
            keys.iter()
                .enumerate()
                .map(|(at, key)| held.get(key).map_or(BasisStatus::Nonbasic, |&status| fit(status, lower[at], upper[at])))
                .collect()
        };
        let mut col_status = carry(&keys.cols, &held_cols, &problem.col_lower, &problem.col_upper);
        let mut row_status = carry(&keys.rows, &held_rows, &problem.row_lower, &problem.row_upper);
        let mut surplus = (col_status.iter().chain(&row_status).filter(|status| **status == BasisStatus::Basic).count()).saturating_sub(row_status.len());
        for status in row_status.iter_mut().rev().chain(col_status.iter_mut().rev()) {
            if surplus == 0 {
                break;
            }
            if *status == BasisStatus::Basic {
                *status = BasisStatus::Nonbasic;
                surplus -= 1;
            }
        }
        Some(solver::Basis { col_status, row_status })
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
