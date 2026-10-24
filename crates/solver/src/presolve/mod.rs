//! HiGHS's LP presolve and postsolve, and the way `Highs::run` uses them:
//! presolve the LP, solve what remains, postsolve that solution and basis
//! back to the original LP, then confirm (or finish) with the simplex
//! method from that basis.

mod hpresolve;
pub(crate) mod matrix;
pub(crate) mod mip_postsolve;
pub(crate) mod mip_presolve;
mod postsolve;
mod sum_bounds;

use hpresolve::{Lp, Presolve, PresolveStatus, Settings};
use postsolve::{PostsolveStack, Status, Tolerances};

use crate::{
    BasisStatus, Options,
    simplex::{
        self, ModelStatus, SimplexResult,
        ekk::{Interrupt, SimplexLp},
        options::Solver,
    },
};

/// An absent index (HiGHS's -1).
pub(crate) const NONE: usize = usize::MAX;

/// `std::max`, which keeps its first argument unless the second is
/// greater, NaN included; `f64::max` would drop a NaN instead.
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min`, which keeps its first argument unless the second is less.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// What presolving and solving the reduced LP leave for the original LP.
pub(crate) enum Presolved {
    /// Presolve changed nothing: solve the original LP directly.
    NotReduced,
    /// The outcome, with no solve of the original LP to follow.
    Done(SimplexResult),
    /// Presolve found the LP unbounded or infeasible; HiGHS tells which
    /// with the primal simplex method on the original LP.
    UnboundedOrInfeasible,
    /// The reduced LP's basis, postsolved: the original LP is solved from
    /// it with the simplex method's strategy `Choose`.
    Postsolved {
        basis: crate::Basis,
        iterations: usize,
        ipm_iterations: (usize, usize),
        /// The reduced solve's final factor pivot threshold, if any.
        factor_pivot_threshold: Option<f64>,
    },
}

/// Presolve `lp` as HiGHS does, solve what remains and postsolve its
/// basis.
pub(crate) fn solve(lp: &SimplexLp, options: &Options, interrupt: Option<Interrupt>) -> Presolved {
    let num_col = lp.num_col;
    let num_row = lp.num_row;
    let sense = lp.sense;
    let model = Lp {
        num_col,
        num_row,
        col_cost: lp.col_cost.iter().map(|&cost| sense * cost).collect(),
        col_lower: lp.col_lower.clone(),
        col_upper: lp.col_upper.clone(),
        row_lower: lp.row_lower.clone(),
        row_upper: lp.row_upper.clone(),
        offset: sense * lp.offset,
        start: lp.a.start.clone(),
        index: lp.a.index.clone(),
        value: lp.a.value.clone(),
    };
    let settings = Settings {
        primal_feasibility_tolerance: options.primal_feasibility_tolerance,
        dual_feasibility_tolerance: options.dual_feasibility_tolerance,
        small_matrix_value: options.small_matrix_value,
        time_limit: options.time_limit,
        ..Settings::default()
    };
    let mut stack = PostsolveStack::default();
    stack.initialize_index_maps(num_row, num_col);
    let mut presolve = Presolve::new(model, settings);
    let status = presolve.run(&mut stack);

    let tolerances = Tolerances {
        primal_feasibility: options.primal_feasibility_tolerance,
        dual_feasibility: options.dual_feasibility_tolerance,
    };
    let mut iterations = 0;
    let mut ipm_iterations = (0, 0);
    let mut factor_pivot_threshold = None;
    let (mut solution, mut basis) = match status {
        PresolveStatus::NotReduced => return Presolved::NotReduced,
        PresolveStatus::Infeasible => return Presolved::Done(empty_result(lp, ModelStatus::Infeasible)),
        PresolveStatus::UnboundedOrInfeasible => return Presolved::UnboundedOrInfeasible,
        PresolveStatus::ReducedToEmpty => (
            postsolve::Solution {
                dual_valid: true,
                ..Default::default()
            },
            postsolve::Basis {
                valid: true,
                ..Default::default()
            },
        ),
        PresolveStatus::Reduced => {
            let mut reduced = std::mem::take(&mut presolve.model);
            if !clean_bounds(&mut reduced, options.primal_feasibility_tolerance) || reduced.num_row == 0 || reduced.value.is_empty() {
                return Presolved::NotReduced;
            }
            let reduced_lp = simplex::simplex_lp(
                1.0,
                reduced.offset,
                reduced.col_cost,
                reduced.col_lower,
                reduced.col_upper,
                reduced.row_lower,
                reduced.row_upper,
                reduced.start,
                reduced.index,
                reduced.value,
            );
            // HiGHS lifts any objective bound for the reduced solve.
            let reduced_options = Options {
                objective_bound: f64::INFINITY,
                ..options.clone()
            };
            let result = match options.solver {
                Solver::Simplex => simplex::solve_lp_simplex(reduced_lp, &reduced_options, None, interrupt.clone()),
                Solver::Ipm => crate::ipx::solve_lp(&reduced_lp, &reduced_options, interrupt.clone()),
            };
            iterations = result.iterations;
            ipm_iterations = (result.ipm_iterations, result.crossover_iterations);
            factor_pivot_threshold = Some(result.factor_pivot_threshold);
            if !matches!(result.status, ModelStatus::Optimal | ModelStatus::Unknown) {
                let mut out = empty_result(lp, result.status);
                out.iterations = iterations;
                (out.ipm_iterations, out.crossover_iterations) = ipm_iterations;
                return Presolved::Done(out);
            }
            let solution = postsolve::Solution {
                col_value: result.col_value,
                col_dual: result.col_dual,
                row_value: result.row_value,
                row_dual: result.row_dual,
                dual_valid: true,
            };
            let basis = postsolve::Basis {
                col_status: result.basis.col_status.iter().map(|&status| from_basis_status(status)).collect(),
                row_status: result.basis.row_status.iter().map(|&status| from_basis_status(status)).collect(),
                valid: true,
            };
            (solution, basis)
        }
    };

    stack.undo(tolerances, &mut solution, &mut basis);
    calculate_row_values_quad(lp, &mut solution);
    refine_basis(lp, &solution, &mut basis);
    let basis = crate::Basis {
        col_status: basis.col_status.iter().map(|&status| to_basis_status(status)).collect(),
        row_status: basis.row_status.iter().map(|&status| to_basis_status(status)).collect(),
    };
    Presolved::Postsolved {
        basis,
        iterations,
        ipm_iterations,
        factor_pivot_threshold: factor_pivot_threshold.filter(|&threshold| threshold > 0.0),
    }
}

fn empty_result(lp: &SimplexLp, status: ModelStatus) -> SimplexResult {
    SimplexResult {
        status,
        col_value: vec![0.0; lp.num_col],
        col_dual: vec![0.0; lp.num_col],
        row_value: vec![0.0; lp.num_row],
        row_dual: vec![0.0; lp.num_row],
        objective: f64::NAN,
        iterations: 0,
        basis: crate::Basis {
            col_status: vec![BasisStatus::Lower; lp.num_col],
            row_status: vec![BasisStatus::Basic; lp.num_row],
        },
        factor_pivot_threshold: 0.0,
        ipm_iterations: 0,
        crossover_iterations: 0,
        infeasibilities: Default::default(),
    }
}

/// HiGHS's `cleanBounds` after presolve: bounds crossed by no more than
/// the tolerance meet in the middle; crossed by more, the LP is refused.
fn clean_bounds(lp: &mut Lp, tolerance: f64) -> bool {
    for (lower, upper) in lp
        .col_lower
        .iter_mut()
        .zip(lp.col_upper.iter_mut())
        .chain(lp.row_lower.iter_mut().zip(lp.row_upper.iter_mut()))
    {
        let residual = *lower - *upper;
        if residual > tolerance {
            return false;
        } else if residual > 0.0 {
            let mid = 0.5 * (*lower + *upper);
            *lower = mid;
            *upper = mid;
        }
    }
    true
}

/// HiGHS's `calculateRowValuesQuad`.
fn calculate_row_values_quad(lp: &SimplexLp, solution: &mut postsolve::Solution) {
    let mut row_value = vec![crate::util::CDouble::new(0.0); lp.num_row];
    for col in 0..lp.num_col {
        for k in lp.a.start[col]..lp.a.start[col + 1] {
            row_value[lp.a.index[k]] += lp.a.value[k] * solution.col_value[col];
        }
    }
    solution.row_value = row_value.into_iter().map(|value| value.value()).collect();
}

/// HiGHS's `refineBasis`: give each nonbasic variable whose bound
/// postsolve left open the bound it sits nearest.
fn refine_basis(lp: &SimplexLp, solution: &postsolve::Solution, basis: &mut postsolve::Basis) {
    let refine = |lower: f64, upper: f64, value: f64| {
        if lower == upper {
            Status::Lower
        } else if lower != f64::NEG_INFINITY {
            if upper != f64::INFINITY {
                if value < 0.5 * (lower + upper) { Status::Lower } else { Status::Upper }
            } else {
                Status::Lower
            }
        } else if upper != f64::INFINITY {
            Status::Upper
        } else {
            Status::Zero
        }
    };
    for col in 0..lp.num_col {
        if basis.col_status[col] == Status::Nonbasic {
            basis.col_status[col] = refine(lp.col_lower[col], lp.col_upper[col], solution.col_value[col]);
        }
    }
    for row in 0..lp.num_row {
        if basis.row_status[row] == Status::Nonbasic {
            basis.row_status[row] = refine(lp.row_lower[row], lp.row_upper[row], solution.row_value[row]);
        }
    }
}

fn from_basis_status(status: BasisStatus) -> Status {
    match status {
        BasisStatus::Lower => Status::Lower,
        BasisStatus::Basic => Status::Basic,
        BasisStatus::Upper => Status::Upper,
        BasisStatus::Zero => Status::Zero,
        BasisStatus::Nonbasic => Status::Nonbasic,
    }
}

fn to_basis_status(status: Status) -> BasisStatus {
    match status {
        Status::Lower | Status::Nonbasic => BasisStatus::Lower,
        Status::Basic => BasisStatus::Basic,
        Status::Upper => BasisStatus::Upper,
        Status::Zero => BasisStatus::Zero,
    }
}
