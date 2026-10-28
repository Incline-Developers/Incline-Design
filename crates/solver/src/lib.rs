//! Solver: a pure Rust port of the HiGHS linear optimisation solver.
//!
//! [`solve`] takes a [`Problem`] and returns a [`Solution`]. The solver is
//! HiGHS's serial dual simplex method, with its scaling, dual steepest edge
//! pricing, bound-flipping ratio test, cost perturbation and primal simplex
//! cleanup, ported to safe Rust.
//!
//! Unless [`Options::presolve`] is off, a solve from scratch first runs
//! HiGHS's LP presolve, solves the reduced LP, and postsolves its basis
//! for a final simplex solve of the original LP.
//!
//! A solve returns its optimal [`Basis`]; passing it to [`solve_from`] for
//! a changed problem of the same shape starts there instead of from
//! scratch, as HiGHS does with a basis set.
//!
//! [`solve_mip`] solves a problem with integer columns as HiGHS's MIP
//! solver does with one thread: presolve, the root node's cuts and
//! heuristics, restarts, symmetry handling and branch and bound.

#![allow(clippy::needless_range_loop, reason = "a port of HiGHS's indexed loops, kept recognisable")]
#![allow(clippy::neg_cmp_op_on_partial_ord, reason = "HiGHS's negated comparisons, which differ from the plain ones on NaN")]
#![allow(clippy::manual_clamp, reason = "HiGHS's min/max pairs, which unlike clamp accept any bounds and NaN")]
#![allow(clippy::too_many_arguments, reason = "HiGHS's signatures, kept for the port")]
#![allow(clippy::type_complexity, reason = "HiGHS's nested containers, kept for the port")]
#![allow(clippy::explicit_counter_loop, reason = "HiGHS's loops with a second counter, kept for the port")]

mod factor;
#[allow(dead_code, reason = "the MIP solver is being ported; nothing calls it yet")]
mod hash_table;
#[allow(dead_code, reason = "the MIP solver is being ported; nothing calls it yet")]
mod hash_tree;
mod hvector;
#[allow(dead_code, reason = "the MIP solver is being ported; nothing calls it yet")]
mod integers;
mod ipx;
mod kkt;
mod lp_solver;
mod matrix;
#[allow(dead_code, reason = "the MIP solver is being ported; nothing calls it yet")]
mod mip;
pub mod mps;
mod pdqsort;
mod presolve;
mod problem;
#[allow(dead_code, reason = "the MIP solver is being ported; nothing calls it yet")]
mod rb_tree;
mod simplex;
mod util;

pub use lp_solver::{LpSolver, NONE};
#[doc(hidden)]
pub use mip::{MipSettings, MipSolution, MipStatus, solve_mip};
pub use problem::{Problem, Sense};
pub use simplex::{
    Infeasibilities, ModelStatus as Status,
    ekk::Interrupt,
    options::{
        EdgeWeightStrategy, Options, PriceStrategy, SCALE_STRATEGY_CHOOSE, SCALE_STRATEGY_EQUILIBRATION, SCALE_STRATEGY_FORCED_EQUILIBRATION, SCALE_STRATEGY_MAX_VALUE,
        SCALE_STRATEGY_OFF, SimplexStrategy, Solver,
    },
};

/// Where a variable (a column, or a row's activity) sits in a basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasisStatus {
    Lower,
    Basic,
    Upper,
    /// Nonbasic and free, at zero.
    Zero,
    /// Nonbasic, at a bound still to be chosen (HiGHS's `kNonbasic`).
    Nonbasic,
}

/// A simplex basis: a status for each column and each row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Basis {
    pub col_status: Vec<BasisStatus>,
    pub row_status: Vec<BasisStatus>,
}

/// A solve's outcome, in the problem's own terms.
#[derive(Clone, Debug)]
pub struct Solution {
    pub status: Status,
    pub objective: f64,
    pub col_value: Vec<f64>,
    pub col_dual: Vec<f64>,
    /// Each row's activity.
    pub row_value: Vec<f64>,
    pub row_dual: Vec<f64>,
    /// Simplex iterations.
    pub iterations: usize,
    /// Interior point iterations, with [`Solver::Ipm`].
    pub ipm_iterations: usize,
    /// Crossover pivots from the interior point to a basis.
    pub crossover_iterations: usize,
    /// The final basis, when the simplex solver ran.
    pub basis: Option<Basis>,
}

/// Solve `problem`'s linear relaxation (integrality is ignored).
pub fn solve(problem: &Problem, options: &Options) -> Solution {
    solve_from(problem, options, None, None)
}

/// [`solve`] from `basis` if there is one (its sizes must match the
/// problem's), stopping with [`Status::Interrupt`] once `interrupt` says
/// so.
pub fn solve_from(problem: &Problem, options: &Options, basis: Option<&Basis>, interrupt: Option<Interrupt>) -> Solution {
    let mut solver = LpSolver::new(problem, options.clone());
    solver.set_interrupt(interrupt);
    // The interior point method has no use for a basis, so HiGHS drops it.
    if options.solver == Solver::Simplex
        && let Some(basis) = basis.filter(|basis| basis_has_right_rank(basis))
    {
        solver.set_basis(basis);
    }
    let status = solver.run();
    let num_col = problem.num_cols();
    let num_row = problem.num_rows();
    let values = |values: Option<&[f64]>, len: usize| values.map_or_else(|| vec![0.0; len], <[f64]>::to_vec);
    Solution {
        status,
        objective: if solver.col_value().is_some() { solver.objective() } else { f64::NAN },
        col_value: values(solver.col_value(), num_col),
        col_dual: values(solver.col_dual(), num_col),
        row_value: values(solver.row_value(), num_row),
        row_dual: values(solver.row_dual(), num_row),
        iterations: solver.iterations(),
        ipm_iterations: solver.ipm_iterations(),
        crossover_iterations: solver.crossover_iterations(),
        basis: solver.basis().cloned(),
    }
}

/// A basis has one basic variable per row.
fn basis_has_right_rank(basis: &Basis) -> bool {
    let num_basic = basis.col_status.iter().chain(&basis.row_status).filter(|status| **status == BasisStatus::Basic).count();
    num_basic == basis.row_status.len()
}

/// An LP whose rows have no entries: each column sits at the bound its
/// cost prefers, and each row must allow zero.
pub(crate) fn solve_unconstrained(problem: &Problem, options: &Options, sense: f64, col_lower: &[f64], col_upper: &[f64], row_lower: &[f64], row_upper: &[f64]) -> Solution {
    let num_col = problem.num_cols();
    let num_row = problem.num_rows();
    let tolerance = options.primal_feasibility_tolerance;
    let mut status = Status::Optimal;
    let mut col_value = vec![0.0; num_col];
    let mut col_dual = vec![0.0; num_col];
    for col in 0..num_col {
        let (lower, upper) = (col_lower[col], col_upper[col]);
        if lower > upper + tolerance {
            status = Status::Infeasible;
        }
        let cost = sense * problem.col_cost[col];
        col_dual[col] = problem.col_cost[col];
        col_value[col] = if cost > 0.0 {
            if lower == f64::NEG_INFINITY {
                if status == Status::Optimal {
                    status = Status::Unbounded;
                }
                upper.min(0.0)
            } else {
                lower
            }
        } else if cost < 0.0 {
            if upper == f64::INFINITY {
                if status == Status::Optimal {
                    status = Status::Unbounded;
                }
                lower.max(0.0)
            } else {
                upper
            }
        } else if lower.is_finite() {
            lower
        } else if upper.is_finite() {
            upper
        } else {
            0.0
        };
    }
    for row in 0..num_row {
        if row_lower[row] > tolerance || row_upper[row] < -tolerance {
            status = Status::Infeasible;
        }
    }
    let objective = problem.objective_at(&col_value);
    Solution {
        status,
        objective,
        col_value,
        col_dual,
        row_value: vec![0.0; num_row],
        row_dual: vec![0.0; num_row],
        iterations: 0,
        ipm_iterations: 0,
        crossover_iterations: 0,
        basis: None,
    }
}
