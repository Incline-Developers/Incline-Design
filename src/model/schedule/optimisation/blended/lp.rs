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

#[derive(Default)]
pub(crate) struct LinearProgram {
    objective: Vec<f64>,
    column_bounds: Vec<(f64, f64)>,
    rows: Vec<Row>,
}

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

    /// The column values at a maximum of the objective, or why there are none.
    pub(crate) fn maximise(&self) -> Result<Vec<f64>, String> {
        #[cfg(feature = "highs")]
        return self.maximise_highs();
        #[cfg(not(feature = "highs"))]
        return self.maximise_microlp();
    }

    #[cfg(feature = "highs")]
    fn maximise_highs(&self) -> Result<Vec<f64>, String> {
        use highs::{HighsModelStatus, RowProblem, Sense};
        let mut problem = RowProblem::default();
        let cols: Vec<_> = self
            .objective
            .iter()
            .zip(&self.column_bounds)
            .map(|(&weight, &(lower, upper))| problem.add_column(weight, lower..=upper))
            .collect();
        for row in &self.rows {
            problem.add_row(row.lower..=row.upper, row.terms.iter().map(|&(col, coefficient)| (cols[col.0], coefficient)));
        }
        let mut model = problem.optimise(Sense::Maximise);
        model.make_quiet();
        // One thread, so the same input always gives the same schedule.
        model.set_option("threads", 1);
        let solved = model.try_solve().map_err(|status| format!("HiGHS failed: {status:?}"))?;
        if solved.status() != HighsModelStatus::Optimal {
            return Err(format!("HiGHS ended {:?}", solved.status()));
        }
        Ok(solved.get_solution().columns().to_vec())
    }

    #[cfg_attr(feature = "highs", allow(dead_code, reason = "HiGHS solves the dispatch in this build"))]
    fn maximise_microlp(&self) -> Result<Vec<f64>, String> {
        use microlp::{ComparisonOp, OptimizationDirection, Problem};
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let vars: Vec<_> = self
            .objective
            .iter()
            .zip(&self.column_bounds)
            .map(|(&weight, &bounds)| problem.add_var(weight, bounds))
            .collect();
        for row in &self.rows {
            let terms: Vec<_> = row.terms.iter().map(|&(col, coefficient)| (vars[col.0], coefficient)).collect();
            if row.lower == row.upper {
                problem.add_constraint(terms.as_slice(), ComparisonOp::Eq, row.upper);
                continue;
            }
            if row.upper.is_finite() {
                problem.add_constraint(terms.as_slice(), ComparisonOp::Le, row.upper);
            }
            if row.lower.is_finite() {
                problem.add_constraint(terms.as_slice(), ComparisonOp::Ge, row.lower);
            }
        }
        let outcome = problem.solve().map_err(|error| format!("microlp: {error}"))?;
        let solution = outcome.solution().ok_or("microlp was interrupted")?;
        Ok(vars.iter().map(|&var| solution[var]).collect())
    }
}

/// The solver the hourly dispatch uses in this build, for the run's report.
pub(crate) fn backend_name() -> &'static str {
    if cfg!(feature = "highs") { "HiGHS" } else { "microlp 0.6.0" }
}
