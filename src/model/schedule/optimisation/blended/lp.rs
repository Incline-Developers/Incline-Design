//! The linear programs the hourly dispatch solves, one per interval, and the
//! solver they go to.
//!
//! The dispatch only builds a [`LinearProgram`]: columns with an objective
//! weight and bounds, rows with bounds over a few columns. [`LinearProgram::maximise`]
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

    /// The column values at a maximum of the objective, or why there are none.
    pub(crate) fn maximise(&self) -> Result<Vec<f64>, String> {
        let tidy = self.tidied()?;
        #[cfg(not(target_arch = "wasm32"))]
        tidy.dump();
        tidy.maximise_solver()
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
        let Some(dir) = std::env::var_os("INCLINE_LP_DUMP") else {
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
    fn tidied(&self) -> Result<Self, String> {
        let mut rows: Vec<Row> = Vec::with_capacity(self.rows.len());
        let mut seen: std::collections::HashMap<Vec<(usize, u64)>, usize> = std::collections::HashMap::new();
        for row in &self.rows {
            let mut terms: Vec<(Col, f64)> = Vec::with_capacity(row.terms.len());
            let mut sorted = row.terms.clone();
            sorted.sort_by_key(|(col, _)| col.0);
            for (col, coefficient) in sorted {
                match terms.last_mut() {
                    Some((last, total)) if *last == col => *total += coefficient,
                    _ => terms.push((col, coefficient)),
                }
            }
            terms.retain(|(_, coefficient)| *coefficient != 0.0 && coefficient.is_finite());
            if terms.is_empty() {
                // 0 has to lie within the row's bounds, or nothing can.
                let slack = 1e-9 * row.lower.abs().max(row.upper.abs()).max(1.0);
                if row.lower > slack || row.upper < -slack {
                    return Err(format!("a constraint with no terms needs {}..{}", row.lower, row.upper));
                }
                continue;
            }
            let key: Vec<(usize, u64)> = terms.iter().map(|(col, coefficient)| (col.0, coefficient.to_bits())).collect();
            match seen.get(&key) {
                Some(&index) => {
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
                    seen.insert(key, rows.len());
                    rows.push(Row {
                        lower: row.lower,
                        upper: row.upper,
                        terms,
                    });
                }
            }
        }
        Ok(Self {
            objective: self.objective.clone(),
            column_bounds: self.column_bounds.clone(),
            rows,
        })
    }

    /// Solve with the `solver` crate, HiGHS's presolve and dual simplex method in Rust,
    /// which takes the same steps HiGHS does.
    fn maximise_solver(&self) -> Result<Vec<f64>, String> {
        let solution = solver::solve(&self.to_solver(), &solver::Options::default());
        if solution.status != solver::Status::Optimal {
            return Err(format!("solver ended {:?}", solution.status));
        }
        Ok(solution.col_value)
    }
}

/// The solver the hourly dispatch uses in this build, for the run's report.
pub(crate) fn backend_name() -> &'static str {
    "solver"
}
