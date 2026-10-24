//! A proven upper bound on the blended model's value: its linear relaxation,
//! solved by HiGHS's interior-point method (IPX, as ported to the `solver` crate).
//!
//! SCIP's first LP on a long horizon is slow, because its bundled LP solver,
//! SoPlex, has only simplex methods; a week of hourly intervals needed
//! 110-120 s before SCIP had any bound at all. The same LP took HiGHS's
//! interior-point method 36 s. SCIP cannot use that method - its HiGHS LP
//! interface sends a barrier request to dual simplex - so the relaxation is
//! solved here instead, beside the SCIP run, and only its value is used.
//!
//! The relaxation is the model [`formulate`] builds, with three changes, each
//! of which can only enlarge the feasible set and so keeps the value an upper
//! bound on every schedule the model allows:
//!
//! - every binary may take any value in `0..=1`;
//! - an indicator is posted as its big-M row, which the formulation's own
//!   `big_m` makes valid at any flag value in `0..=1`;
//! - the perfect-mixing equality, `Q_recl * T_open = T_recl * Q_open`, is left
//!   out. The grade-box rows around it stay, so a reclaim's grade is still
//!   held inside the pile's grade ceilings, but not to the pile's blend.
//!
//! The last makes the bound weaker on models that blend, not wrong.

use std::time::Duration;

use web_time::Instant;

use super::{
    formulation::{BlendColumns, BlendSizes, Rows, formulate},
    input::BlendInput,
};
use crate::model::schedule::optimisation::StockpileId;

/// What the relaxation proved.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelaxationBound {
    /// No schedule the model allows is worth more than this.
    pub(crate) value: f64,
    /// Building and solving the relaxation.
    pub(crate) elapsed: Duration,
    /// Perfect-mixing equalities left out; zero means the relaxation is the
    /// model's exact LP relaxation.
    pub(crate) dropped_mixing: usize,
}

/// `stop` is polled while the model is built and between interior-point
/// iterations; once it returns true the solve ends without a bound.
pub(crate) fn relaxation_bound(input: &BlendInput, limit: Option<Duration>, stop: solver::Interrupt) -> Result<RelaxationBound, String> {
    let started = Instant::now();
    let mut rows = LpRows {
        columns: BlendColumns::new(),
        sizes: BlendSizes::default(),
        stop: &*stop,
        problem: solver::Problem::new(solver::Sense::Maximise),
        dropped_mixing: 0,
        merge: Vec::new(),
    };
    formulate(&mut rows, input).map_err(|_| "stopped while building the relaxation".to_owned())?;
    let (problem, dropped_mixing) = (rows.problem, rows.dropped_mixing);
    let value = solve(&problem, limit, stop)?;
    Ok(RelaxationBound {
        value,
        elapsed: started.elapsed(),
        dropped_mixing,
    })
}

/// The relaxation, built straight into a [`solver::Problem`].
struct LpRows<'a> {
    columns: BlendColumns<usize>,
    sizes: BlendSizes,
    stop: &'a (dyn Fn() -> bool + Send + Sync),
    problem: solver::Problem,
    dropped_mixing: usize,
    /// Scratch for merging a row's repeated columns into one entry.
    merge: Vec<(usize, f64)>,
}

impl LpRows<'_> {
    fn column(&mut self, upper: f64, value: f64) -> usize {
        self.sizes.variables += 1;
        self.problem.add_col(value, 0.0, if upper.is_finite() { upper } else { f64::INFINITY })
    }
}

impl Rows for LpRows<'_> {
    type Var = usize;

    fn columns(&mut self) -> &mut BlendColumns<usize> {
        &mut self.columns
    }

    fn cancelled(&self) -> bool {
        (self.stop)()
    }

    fn valued(&mut self, upper: f64, value: f64, _name: &str) -> usize {
        self.column(upper, value)
    }

    /// Relaxed to `0..=1`.
    fn binary(&mut self, _name: &str) -> usize {
        self.sizes.binaries += 1;
        self.column(1.0, 0.0)
    }

    fn linear(&mut self, terms: Vec<(usize, f64)>, lhs: f64, rhs: f64, _name: &str) {
        if terms.is_empty() {
            return;
        }
        self.sizes.linear_constraints += 1;
        self.merge.clear();
        self.merge.extend(terms.into_iter().filter(|(_, coefficient)| *coefficient != 0.0));
        self.merge.sort_unstable_by_key(|(column, _)| *column);
        self.merge.dedup_by(|(column, coefficient), (kept, total)| {
            let repeated = column == kept;
            if repeated {
                *total += *coefficient;
            }
            repeated
        });
        self.sizes.linear_coefficient_entries += self.merge.len();
        self.problem.add_row(lhs, rhs, self.merge.iter().copied());
    }

    /// Left out; see the module docs.
    fn mix(&mut self, _pile: StockpileId, _interval: usize, _grade: usize, _recl_q: usize, _open_t: usize, _recl_t: usize, _open_q: usize, _name: &str) {
        self.dropped_mixing += 1;
    }
}

fn solve(problem: &solver::Problem, limit: Option<Duration>, stop: solver::Interrupt) -> Result<f64, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = std::env::var_os("INCLINE_LP_DUMP") {
        let path = std::path::Path::new(&dir).join(format!("relaxation-{}.mps", std::process::id()));
        if let Err(error) = std::fs::write(&path, solver::mps::write(problem)) {
            log::warn!("could not write {}: {error}", path.display());
        }
    }
    let options = solver::Options {
        solver: solver::Solver::Ipm,
        time_limit: limit.map_or(f64::INFINITY, |limit| limit.as_secs_f64()),
        ..solver::Options::default()
    };
    let solution = solver::solve_from(problem, &options, None, Some(stop));
    if solution.status != solver::Status::Optimal {
        return Err(format!("the relaxation solve ended {:?}", solution.status));
    }
    if !solution.objective.is_finite() {
        return Err(format!("the relaxation reported a non-finite optimum {}", solution.objective));
    }
    Ok(solution.objective)
}
