//! HiGHS's KKT check on an LP solution (`lpKktCheck` and `getKktFailures`
//! in `lp_data/HighsSolution.cpp`), which `Highs::run` makes after every
//! LP solve: the objective as the sum of costs times values, the primal
//! and dual infeasibilities HiGHS reports, and the model status those
//! leave (optimal only within a hundred times the tolerances).

use crate::{simplex::ModelStatus, util::CDouble};

const INF: f64 = f64::INFINITY;
const MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION: f64 = 1e2;

/// The LP the check is on, column-wise.
pub(crate) struct KktLp<'a> {
    pub sense: f64,
    pub offset: f64,
    pub col_cost: &'a [f64],
    pub col_lower: &'a [f64],
    pub col_upper: &'a [f64],
    pub row_lower: &'a [f64],
    pub row_upper: &'a [f64],
    pub start: &'a [usize],
    pub index: &'a [usize],
    pub value: &'a [f64],
}

/// The solution: values, and duals when `dual_valid`.
pub(crate) struct KktSolution<'a> {
    pub col_value: &'a [f64],
    pub row_value: &'a [f64],
    pub col_dual: &'a [f64],
    pub row_dual: &'a [f64],
    pub dual_valid: bool,
}

/// The tolerances the check reads (HiGHS's defaults but the feasibility
/// ones).
#[derive(Clone, Copy, Debug)]
pub(crate) struct KktTolerances {
    pub primal_feasibility: f64,
    pub dual_feasibility: f64,
    pub primal_residual: f64,
    pub dual_residual: f64,
    pub optimality: f64,
}

impl KktTolerances {
    pub fn new(primal_feasibility: f64, dual_feasibility: f64) -> Self {
        Self {
            primal_feasibility,
            dual_feasibility,
            primal_residual: 1e-7,
            dual_residual: 1e-7,
            optimality: 1e-7,
        }
    }
}

/// What `HighsInfo` holds after the check. Counts of -1 and infinite
/// measures are HiGHS's illegal values (not computed).
#[derive(Clone, Copy, Debug)]
pub(crate) struct KktInfo {
    pub objective: f64,
    pub num_primal_infeasibilities: i64,
    pub max_primal_infeasibility: f64,
    pub sum_primal_infeasibilities: f64,
    pub num_dual_infeasibilities: i64,
    pub max_dual_infeasibility: f64,
    pub sum_dual_infeasibilities: f64,
    pub num_relative_primal_infeasibilities: i64,
    pub max_relative_primal_infeasibility: f64,
    pub num_relative_dual_infeasibilities: i64,
    pub max_relative_dual_infeasibility: f64,
    pub num_primal_residual_errors: i64,
    pub max_primal_residual_error: f64,
    pub num_relative_primal_residual_errors: i64,
    pub max_relative_primal_residual_error: f64,
    pub num_dual_residual_errors: i64,
    pub max_dual_residual_error: f64,
    pub num_relative_dual_residual_errors: i64,
    pub max_relative_dual_residual_error: f64,
    pub num_complementarity_violations: i64,
    pub max_complementarity_violation: f64,
    pub primal_dual_objective_error: f64,
    /// `primal_solution_status == kSolutionStatusFeasible`.
    pub primal_feasible: bool,
    pub dual_feasible: Option<bool>,
}

impl KktInfo {
    fn invalid() -> Self {
        Self {
            objective: 0.0,
            num_primal_infeasibilities: -1,
            max_primal_infeasibility: INF,
            sum_primal_infeasibilities: INF,
            num_dual_infeasibilities: -1,
            max_dual_infeasibility: INF,
            sum_dual_infeasibilities: INF,
            num_relative_primal_infeasibilities: -1,
            max_relative_primal_infeasibility: INF,
            num_relative_dual_infeasibilities: -1,
            max_relative_dual_infeasibility: INF,
            num_primal_residual_errors: -1,
            max_primal_residual_error: INF,
            num_relative_primal_residual_errors: -1,
            max_relative_primal_residual_error: INF,
            num_dual_residual_errors: -1,
            max_dual_residual_error: INF,
            num_relative_dual_residual_errors: -1,
            max_relative_dual_residual_error: INF,
            num_complementarity_violations: -1,
            max_complementarity_violation: INF,
            primal_dual_objective_error: INF,
            primal_feasible: false,
            dual_feasible: None,
        }
    }
}

/// `std::max`.
fn max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// HiGHS's `infeasibility`: the infeasibility beyond the tolerance, and
/// the residual (at most the tolerance when that is zero).
fn infeasibility(lower: f64, value: f64, upper: f64, tolerance: f64) -> (f64, f64) {
    let mut residual = 0.0;
    let mut infeasibility = 0.0;
    if value < lower - tolerance {
        infeasibility = lower - value;
    }
    if value > upper + tolerance {
        infeasibility = value - upper;
    }
    if tolerance > 0.0 {
        if value < lower {
            residual = lower - value;
        }
        if value > upper {
            residual = value - upper;
        }
    } else {
        residual = infeasibility;
    }
    if infeasibility == 0.0 {
        residual = if tolerance < residual { tolerance } else { residual };
    }
    (infeasibility, residual)
}

const SOLUTION_NO: u8 = 0;
const SOLUTION_LO: u8 = 1;
const SOLUTION_UP: u8 = 2;

/// HiGHS's `getVariableKktFailures` for a continuous variable:
/// `(primal infeasibility, dual infeasibility, at status, mid status)`.
fn variable_kkt_failures(primal_feasibility_tolerance: f64, lower: f64, upper: f64, value: f64, dual: f64) -> (f64, f64, u8, u8) {
    let primal_infeasibility = infeasibility(lower, value, upper, primal_feasibility_tolerance).1;
    let mut at_status = SOLUTION_NO;
    let mut bound_residual = (lower - value).abs();
    if bound_residual * bound_residual <= primal_feasibility_tolerance {
        at_status = SOLUTION_LO;
    } else {
        bound_residual = (value - upper).abs();
        if bound_residual * bound_residual <= primal_feasibility_tolerance {
            at_status = SOLUTION_UP;
        }
    }
    let mut mid_status = SOLUTION_NO;
    let dual_infeasibility;
    if lower < upper {
        let length = upper - lower;
        if lower <= -INF && upper >= INF {
            dual_infeasibility = dual.abs();
        } else if length * length > primal_feasibility_tolerance {
            let middle = (lower + upper) * 0.5;
            if value < middle {
                mid_status = SOLUTION_LO;
                dual_infeasibility = max(-dual, 0.0);
            } else {
                mid_status = SOLUTION_UP;
                dual_infeasibility = max(dual, 0.0);
            }
        } else {
            dual_infeasibility = 0.0;
        }
    } else {
        dual_infeasibility = 0.0;
    }
    (primal_infeasibility, dual_infeasibility, at_status, mid_status)
}

/// HiGHS's `getKktFailures` for an LP (`objective` already set).
fn kkt_failures(lp: &KktLp, sol: &KktSolution, tol: KktTolerances, get_residuals: bool, info: &mut KktInfo) {
    let num_col = lp.col_cost.len();
    let num_row = lp.row_lower.len();
    let have_dual_solution = sol.dual_valid;
    let objective = info.objective;
    *info = KktInfo::invalid();
    info.objective = objective;
    info.num_primal_infeasibilities = 0;
    info.max_primal_infeasibility = 0.0;
    info.sum_primal_infeasibilities = 0.0;
    info.num_relative_primal_infeasibilities = 0;
    info.max_relative_primal_infeasibility = 0.0;
    if get_residuals {
        info.num_primal_residual_errors = 0;
        info.max_primal_residual_error = 0.0;
        info.num_relative_primal_residual_errors = 0;
        info.max_relative_primal_residual_error = 0.0;
    }
    if have_dual_solution {
        info.num_dual_infeasibilities = 0;
        info.max_dual_infeasibility = 0.0;
        info.sum_dual_infeasibilities = 0.0;
        info.num_relative_dual_infeasibilities = 0;
        info.max_relative_dual_infeasibility = 0.0;
        if get_residuals {
            info.num_dual_residual_errors = 0;
            info.max_dual_residual_error = 0.0;
            info.num_relative_dual_residual_errors = 0;
            info.max_relative_dual_residual_error = 0.0;
        }
    }

    // Ax, and A^T y less the costs, for the residuals.
    let mut primal_activity = Vec::new();
    let mut dual_activity = Vec::new();
    if get_residuals {
        let mut quad = vec![CDouble::new(0.0); num_row];
        for col in 0..num_col {
            for k in lp.start[col]..lp.start[col + 1] {
                quad[lp.index[k]] += sol.col_value[col] * lp.value[k];
            }
        }
        primal_activity = quad.into_iter().map(CDouble::value).collect();
        if have_dual_solution {
            dual_activity = (0..num_col)
                .map(|col| {
                    let mut value = CDouble::new(0.0);
                    for k in lp.start[col]..lp.start[col + 1] {
                        value += sol.row_dual[lp.index[k]] * lp.value[k];
                    }
                    value.value()
                })
                .collect();
        }
    }

    let mut max_col_primal_infeasibility = 0.0;
    let mut max_col_dual_infeasibility = 0.0;
    let mut max_relative_col_primal_infeasibility = 0.0;
    let mut max_relative_col_dual_infeasibility = 0.0;
    let mut highs_norm_bounds = 0.0;
    let mut highs_norm_costs = 0.0;
    let mut cost = 0.0;
    let mut dual = 0.0;
    for pass in 0..2 {
        for var in 0..num_col + num_row {
            let is_col = var < num_col;
            let (lower, upper, value);
            if is_col {
                cost = lp.col_cost[var];
                lower = lp.col_lower[var];
                upper = lp.col_upper[var];
                value = sol.col_value[var];
                if have_dual_solution {
                    dual = sol.col_dual[var];
                }
                if pass == 0 {
                    if dual * dual < tol.dual_feasibility {
                        highs_norm_costs = max(cost.abs(), highs_norm_costs);
                    }
                    if get_residuals && have_dual_solution {
                        let mut q_dual_activity = CDouble::new(dual_activity[var]);
                        q_dual_activity -= lp.col_cost[var];
                        dual_activity[var] = q_dual_activity.value();
                    }
                }
            } else {
                let row = var - num_col;
                lower = lp.row_lower[row];
                upper = lp.row_upper[row];
                value = sol.row_value[row];
                if have_dual_solution {
                    dual = sol.row_dual[row];
                }
            }
            dual *= lp.sense;
            let (primal_infeasibility, dual_infeasibility, at_status, mid_status) = variable_kkt_failures(tol.primal_feasibility, lower, upper, value, dual);
            if pass == 0 {
                if at_status == SOLUTION_LO {
                    highs_norm_bounds = max(lower.abs(), highs_norm_bounds);
                } else if at_status == SOLUTION_UP {
                    highs_norm_bounds = max(upper.abs(), highs_norm_bounds);
                }
            } else {
                if primal_infeasibility > 0.0 {
                    if primal_infeasibility > tol.primal_feasibility {
                        info.num_primal_infeasibilities += 1;
                    }
                    if info.max_primal_infeasibility < primal_infeasibility {
                        info.max_primal_infeasibility = primal_infeasibility;
                    }
                    info.sum_primal_infeasibilities += primal_infeasibility;
                    let mut relative_bound_measure = highs_norm_bounds;
                    if at_status == SOLUTION_NO {
                        if mid_status == SOLUTION_NO || mid_status == SOLUTION_LO {
                            relative_bound_measure = max(lower.abs(), relative_bound_measure);
                        } else {
                            relative_bound_measure = max(upper.abs(), relative_bound_measure);
                        }
                    }
                    let relative_primal_infeasibility = primal_infeasibility / (1.0 + relative_bound_measure);
                    if relative_primal_infeasibility > tol.primal_feasibility {
                        info.num_relative_primal_infeasibilities += 1;
                    }
                    if info.max_relative_primal_infeasibility < relative_primal_infeasibility {
                        info.max_relative_primal_infeasibility = relative_primal_infeasibility;
                    }
                }
                if have_dual_solution && dual_infeasibility > 0.0 {
                    if dual_infeasibility > tol.dual_feasibility {
                        info.num_dual_infeasibilities += 1;
                    }
                    if info.max_dual_infeasibility < dual_infeasibility {
                        info.max_dual_infeasibility = dual_infeasibility;
                    }
                    info.sum_dual_infeasibilities += dual_infeasibility;
                    let mut relative_cost_measure = highs_norm_costs;
                    if is_col && cost != 0.0 && dual * dual >= tol.dual_feasibility {
                        relative_cost_measure = max(cost.abs(), relative_cost_measure);
                    }
                    let relative_dual_infeasibility = dual_infeasibility / (1.0 + relative_cost_measure);
                    if relative_dual_infeasibility > tol.dual_feasibility {
                        info.num_relative_dual_infeasibilities += 1;
                    }
                    if info.max_relative_dual_infeasibility < relative_dual_infeasibility {
                        info.max_relative_dual_infeasibility = relative_dual_infeasibility;
                    }
                }
                if !is_col && get_residuals {
                    let row = var - num_col;
                    let primal_residual_error = (primal_activity[row] - sol.row_value[row]).abs();
                    let relative_primal_residual_error = primal_residual_error / (1.0 + highs_norm_bounds);
                    if primal_residual_error > tol.primal_residual {
                        info.num_primal_residual_errors += 1;
                    }
                    if info.max_primal_residual_error < primal_residual_error {
                        info.max_primal_residual_error = primal_residual_error;
                    }
                    if relative_primal_residual_error > tol.primal_residual {
                        info.num_relative_primal_residual_errors += 1;
                    }
                    if info.max_relative_primal_residual_error < relative_primal_residual_error {
                        info.max_relative_primal_residual_error = relative_primal_residual_error;
                    }
                }
                if is_col && get_residuals && have_dual_solution {
                    let dual_residual_error = (dual_activity[var] + sol.col_dual[var]).abs();
                    let relative_dual_residual_error = dual_residual_error / (1.0 + highs_norm_costs);
                    if dual_residual_error > tol.dual_residual {
                        info.num_dual_residual_errors += 1;
                    }
                    if info.max_dual_residual_error < dual_residual_error {
                        info.max_dual_residual_error = dual_residual_error;
                    }
                    if relative_dual_residual_error > tol.dual_residual {
                        info.num_relative_dual_residual_errors += 1;
                    }
                    if info.max_relative_dual_residual_error < relative_dual_residual_error {
                        info.max_relative_dual_residual_error = relative_dual_residual_error;
                    }
                }
            }
            if pass == 1 && num_col > 0 && var == num_col - 1 {
                max_col_primal_infeasibility = info.max_primal_infeasibility;
                max_col_dual_infeasibility = info.max_dual_infeasibility;
                max_relative_col_primal_infeasibility = info.max_relative_primal_infeasibility;
                max_relative_col_dual_infeasibility = info.max_relative_dual_infeasibility;
                info.max_primal_infeasibility = 0.0;
                info.max_dual_infeasibility = 0.0;
                info.max_relative_primal_infeasibility = 0.0;
                info.max_relative_dual_infeasibility = 0.0;
            }
        }
    }
    info.max_primal_infeasibility = max(max_col_primal_infeasibility, info.max_primal_infeasibility);
    info.max_dual_infeasibility = max(max_col_dual_infeasibility, info.max_dual_infeasibility);
    info.max_relative_primal_infeasibility = max(max_relative_col_primal_infeasibility, info.max_relative_primal_infeasibility);
    info.max_relative_dual_infeasibility = max(max_relative_col_dual_infeasibility, info.max_relative_dual_infeasibility);

    if have_dual_solution {
        // `getComplementarityViolations`.
        info.num_complementarity_violations = 0;
        info.max_complementarity_violation = 0.0;
        for var in 0..num_col + num_row {
            let is_col = var < num_col;
            let (primal, dual, lower, upper) = if is_col {
                (sol.col_value[var], sol.col_dual[var], lp.col_lower[var], lp.col_upper[var])
            } else {
                let row = var - num_col;
                (sol.row_value[row], sol.row_dual[row], lp.row_lower[row], lp.row_upper[row])
            };
            let primal_residual = if lower <= -INF && upper >= INF {
                1.0
            } else {
                let mid = (lower + upper) * 0.5;
                if primal < mid { (lower - primal).abs() } else { (upper - primal).abs() }
            };
            let complementarity_violation = primal_residual * dual.abs();
            if complementarity_violation > tol.optimality {
                info.num_complementarity_violations += 1;
            }
            info.max_complementarity_violation = max(complementarity_violation, info.max_complementarity_violation);
        }
        let dual_objective_value = dual_objective_value(lp, sol);
        let abs_objective_difference = (info.objective - dual_objective_value).abs();
        let denominator = 1.0 + info.objective.abs() + dual_objective_value.abs();
        info.primal_dual_objective_error = abs_objective_difference / denominator;
    }

    info.primal_feasible = info.num_primal_infeasibilities == 0;
    if have_dual_solution {
        info.dual_feasible = Some(info.num_dual_infeasibilities == 0);
    }
}

/// HiGHS's `computeDualObjectiveValue` for an LP.
fn dual_objective_value(lp: &KktLp, sol: &KktSolution) -> f64 {
    let num_col = lp.col_cost.len();
    let num_row = lp.row_lower.len();
    let mut value = lp.offset;
    for var in 0..num_col + num_row {
        let is_col = var < num_col;
        let (primal, dual, lower, upper) = if is_col {
            (sol.col_value[var], sol.col_dual[var], lp.col_lower[var], lp.col_upper[var])
        } else {
            let row = var - num_col;
            (sol.row_value[row], sol.row_dual[row], lp.row_lower[row], lp.row_upper[row])
        };
        let bound = if lower <= -INF && upper >= INF {
            1.0
        } else {
            let mid = (lower + upper) * 0.5;
            if primal < mid { lower } else { upper }
        };
        value += bound * dual;
    }
    value
}

/// HiGHS's `lpKktCheck`: the objective and KKT measures of the solution,
/// and the model status they allow.
pub(crate) fn lp_kkt_check(lp: &KktLp, sol: &KktSolution, basis_valid: bool, tol: KktTolerances, model_status: &mut ModelStatus) -> KktInfo {
    let num_col = lp.col_cost.len();
    let mut info = KktInfo::invalid();
    let mut objective = lp.offset;
    for col in 0..num_col {
        objective += lp.col_cost[col] * sol.col_value[col];
    }
    info.objective = objective;
    let get_residuals = !basis_valid;
    kkt_failures(lp, sol, tol, get_residuals, &mut info);
    if *model_status == ModelStatus::UnboundedOrInfeasible && info.num_primal_infeasibilities == 0 && (!get_residuals || info.num_primal_residual_errors == 0) {
        *model_status = ModelStatus::Unbounded;
    }

    let mut max_primal_tolerance_relative_violation: f64 = 0.0;
    let mut max_dual_tolerance_relative_violation: f64 = 0.0;
    let mut primal_dual_objective_tolerance_relative_violation = 0.0;
    if basis_valid {
        if info.num_primal_infeasibilities > 0 {
            max_primal_tolerance_relative_violation = max(info.max_primal_infeasibility / tol.primal_feasibility, max_primal_tolerance_relative_violation);
        }
        if info.num_dual_infeasibilities > 0 {
            max_dual_tolerance_relative_violation = max(info.max_dual_infeasibility / tol.dual_feasibility, max_dual_tolerance_relative_violation);
        }
        let have_residual_errors = info.num_primal_residual_errors != -1;
        if have_residual_errors {
            max_primal_tolerance_relative_violation = max(info.max_relative_primal_residual_error / tol.primal_residual, max_primal_tolerance_relative_violation);
            max_dual_tolerance_relative_violation = max(info.max_relative_dual_residual_error / tol.dual_residual, max_dual_tolerance_relative_violation);
        }
        primal_dual_objective_tolerance_relative_violation = info.primal_dual_objective_error / tol.optimality;
        if info.num_primal_infeasibilities == 0 {
            info.primal_feasible = true;
        }
        if info.num_dual_infeasibilities == 0 {
            info.dual_feasible = Some(true);
        }
        if max_primal_tolerance_relative_violation > MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION {
            info.primal_feasible = false;
        }
        if max_dual_tolerance_relative_violation > MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION {
            info.dual_feasible = Some(false);
        }
    } else {
        let tolerance_relative_violation = info.max_relative_primal_infeasibility / tol.primal_feasibility;
        max_primal_tolerance_relative_violation = max(tolerance_relative_violation, max_primal_tolerance_relative_violation);
        let tolerance_relative_violation = info.max_relative_dual_infeasibility / tol.dual_feasibility;
        max_dual_tolerance_relative_violation = max(tolerance_relative_violation, max_dual_tolerance_relative_violation);
        let tolerance_relative_violation = info.max_relative_primal_residual_error / tol.primal_residual;
        max_primal_tolerance_relative_violation = max(tolerance_relative_violation, max_primal_tolerance_relative_violation);
        let tolerance_relative_violation = info.max_relative_dual_residual_error / tol.dual_residual;
        max_dual_tolerance_relative_violation = max(tolerance_relative_violation, max_dual_tolerance_relative_violation);
        if info.primal_dual_objective_error > tol.optimality {
            primal_dual_objective_tolerance_relative_violation = info.primal_dual_objective_error / tol.optimality;
        }
        info.primal_feasible = max_primal_tolerance_relative_violation <= MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION;
        info.dual_feasible = Some(max_dual_tolerance_relative_violation <= MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION);
    }
    let mut max_tolerance_relative_violation = primal_dual_objective_tolerance_relative_violation;
    max_tolerance_relative_violation = max(max_primal_tolerance_relative_violation, max_tolerance_relative_violation);
    max_tolerance_relative_violation = max(max_dual_tolerance_relative_violation, max_tolerance_relative_violation);
    if *model_status == ModelStatus::Optimal {
        if max_tolerance_relative_violation > MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION {
            *model_status = ModelStatus::Unknown;
        }
    } else if *model_status == ModelStatus::Unknown && max_tolerance_relative_violation <= MAX_ALLOWED_TOLERANCE_RELATIVE_VIOLATION {
        *model_status = ModelStatus::Optimal;
    }
    info
}
