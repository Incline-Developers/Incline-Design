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

    /// The objective at `solution`'s column values.
    pub(crate) fn value(&self, solution: &[f64]) -> f64 {
        self.objective.iter().zip(solution).map(|(weight, value)| weight * value).sum()
    }

    /// The column values at a maximum of the objective, or why there are none.
    pub(crate) fn maximise(&self) -> Result<Vec<f64>, String> {
        let tidy = self.tidied()?;
        #[cfg(feature = "highs")]
        return tidy.maximise_highs();
        #[cfg(not(feature = "highs"))]
        return tidy.maximise_microlp();
    }

    /// The same program with what a simplex basis chokes on taken out: a
    /// column repeated within a row is one term, zero terms are dropped, a
    /// row with no terms left is checked and dropped, and rows over exactly
    /// the same terms - a reclaim cap and the equality that spends it, a dig
    /// pin and its block's total - are one row with the tighter bounds.
    ///
    /// Two parallel rows are the classic way to a basis that is singular
    /// apart from rounding, which is what microlp reports as a singular matrix.
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

    /// This program scaled so every row's and then every column's largest
    /// coefficient is near one, with the factor each column was scaled by.
    ///
    /// microlp takes a pivot under an absolute 1e-10 for zero. Tonnes in the
    /// thousands beside truck hours per tonne and grade differences of a
    /// ten-thousandth make that a different test in every row; scaled, it is
    /// the same test everywhere. Powers of two, so scaling rounds nothing.
    #[cfg_attr(feature = "highs", allow(dead_code, reason = "HiGHS scales its own problems"))]
    fn equilibrated(&self) -> (Self, Vec<f64>) {
        let power_of_two = |magnitude: f64| {
            if magnitude > 0.0 && magnitude.is_finite() {
                2f64.powi(-magnitude.log2().round() as i32)
            } else {
                1.0
            }
        };
        let row_scale: Vec<f64> = self
            .rows
            .iter()
            .map(|row| power_of_two(row.terms.iter().map(|(_, c)| c.abs()).fold(0.0, f64::max)))
            .collect();
        let mut column_max = vec![0.0f64; self.objective.len()];
        for (row, scale) in self.rows.iter().zip(&row_scale) {
            for (col, coefficient) in &row.terms {
                column_max[col.0] = column_max[col.0].max((coefficient * scale).abs());
            }
        }
        let column_scale: Vec<f64> = column_max.iter().map(|max| power_of_two(*max)).collect();
        let rows = self
            .rows
            .iter()
            .zip(&row_scale)
            .map(|(row, scale)| Row {
                lower: row.lower * scale,
                upper: row.upper * scale,
                terms: row.terms.iter().map(|(col, coefficient)| (*col, coefficient * scale * column_scale[col.0])).collect(),
            })
            .collect();
        let scaled = Self {
            objective: self.objective.iter().zip(&column_scale).map(|(weight, scale)| weight * scale).collect(),
            column_bounds: self
                .column_bounds
                .iter()
                .zip(&column_scale)
                .map(|((lower, upper), scale)| (lower / scale, upper / scale))
                .collect(),
            rows,
        };
        (scaled, column_scale)
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

    /// Solve with microlp, scaled; and should the scaled basis still come
    /// out singular, once more as given before saying so.
    #[cfg_attr(feature = "highs", allow(dead_code, reason = "HiGHS solves the dispatch in this build"))]
    fn maximise_microlp(&self) -> Result<Vec<f64>, String> {
        let (scaled, column_scale) = self.equilibrated();
        match scaled.maximise_microlp_as_given() {
            Ok(solution) => Ok(solution.iter().zip(&column_scale).map(|(value, scale)| value * scale).collect()),
            Err(scaled_error) => self.maximise_microlp_as_given().map_err(|error| format!("{error} (scaled: {scaled_error})")),
        }
    }

    #[cfg_attr(feature = "highs", allow(dead_code, reason = "HiGHS solves the dispatch in this build"))]
    fn maximise_microlp_as_given(&self) -> Result<Vec<f64>, String> {
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
