//! The simplex solver: HiGHS's `HEkk` family and `solveLpSimplex`.

mod dual;
mod dual_rhs;
mod dual_row;
pub(crate) mod ekk;
pub(crate) mod nla;
pub mod options;
mod primal;
mod scale;

use ekk::{Ekk, SimplexLp};
use nla::Scale;
use options::{EdgeWeightStrategy, Options, SimplexStrategy};

use crate::matrix::ColWise;

/// How a solve ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    NotSet,
    /// The model has no columns (HiGHS's `kModelEmpty`).
    ModelEmpty,
    Optimal,
    Infeasible,
    UnboundedOrInfeasible,
    Unbounded,
    ObjectiveBound,
    TimeLimit,
    IterationLimit,
    Interrupt,
    Unknown,
    SolveError,
}

impl ModelStatus {
    /// The number of HiGHS's `HighsModelStatus` value.
    pub fn highs_number(self) -> i32 {
        match self {
            ModelStatus::NotSet => 0,
            ModelStatus::SolveError => 4,
            ModelStatus::ModelEmpty => 6,
            ModelStatus::Optimal => 7,
            ModelStatus::Infeasible => 8,
            ModelStatus::UnboundedOrInfeasible => 9,
            ModelStatus::Unbounded => 10,
            ModelStatus::ObjectiveBound => 11,
            ModelStatus::TimeLimit => 13,
            ModelStatus::IterationLimit => 14,
            ModelStatus::Unknown => 15,
            ModelStatus::Interrupt => 17,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Algorithm {
    None,
    Primal,
    Dual,
}

/// What the simplex solver returns, in the space of the LP it was given.
#[derive(Clone, Debug)]
pub(crate) struct SimplexResult {
    pub status: ModelStatus,
    pub col_value: Vec<f64>,
    pub col_dual: Vec<f64>,
    pub row_value: Vec<f64>,
    pub row_dual: Vec<f64>,
    pub objective: f64,
    pub iterations: usize,
    pub basis: crate::Basis,
    /// The factor pivot threshold the solve ended with.
    pub factor_pivot_threshold: f64,
    /// Interior point iterations and crossover pivots, when IPX ran.
    pub ipm_iterations: usize,
    pub crossover_iterations: usize,
    /// The primal and dual infeasibilities of the returned basis in the
    /// unscaled LP, as HiGHS's `HighsInfo` reports them.
    pub infeasibilities: Infeasibilities,
}

/// Counts, largest and sums of primal and dual infeasibilities; counts
/// are `None` when not known (HiGHS's -1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Infeasibilities {
    pub num_primal: Option<usize>,
    pub max_primal: f64,
    pub sum_primal: f64,
    pub num_dual: Option<usize>,
    pub max_dual: f64,
    pub sum_dual: f64,
}

/// Solve `lp` with the dual simplex method from scratch: scaled if that
/// helps, then cleaned up on the unscaled LP if the unscaled solution is not
/// optimal.
pub(crate) fn solve_lp_simplex(lp: SimplexLp, options: &Options, basis: Option<&crate::Basis>, interrupt: Option<ekk::Interrupt>) -> SimplexResult {
    let scale = if options.simplex_scale_strategy != options::SCALE_STRATEGY_OFF {
        scale::scale_lp(options, &mut lp.clone()).0
    } else {
        None
    };
    let mut ekk = None;
    solve_lp_simplex_in(&mut ekk, &lp, scale.as_ref(), options, basis, interrupt)
}

/// The scale factors HiGHS's `scaleLp` would choose for `lp`, if any,
/// and the strategy it records as used.
pub(crate) fn scale_factors(options: &Options, lp: &SimplexLp) -> (Option<Scale>, i32) {
    scale::scale_lp(options, &mut lp.clone())
}

/// `lp`'s scaled copy, as HiGHS's `applyScale` makes it.
pub(crate) fn scaled_lp(lp: &SimplexLp, scale: &Scale) -> SimplexLp {
    let mut scaled = lp.clone();
    for col in 0..lp.num_col {
        scaled.col_lower[col] /= scale.col[col];
        scaled.col_upper[col] /= scale.col[col];
        scaled.col_cost[col] *= scale.col[col];
    }
    for row in 0..lp.num_row {
        scaled.row_lower[row] *= scale.row[row];
        scaled.row_upper[row] *= scale.row[row];
    }
    scaled.a.apply_scale(&scale.col, &scale.row);
    scaled
}

/// HiGHS's `solveLpSimplex` with the `HEkk` it keeps between solves: `ekk`
/// is reused when there is one, so a solve after changing only bounds or
/// costs starts from the last basis, factor and edge weights. `lp` is the
/// unscaled LP and `scale` the factors it was given, if any; `basis` is
/// set when the kept `Ekk` has none.
pub(crate) fn solve_lp_simplex_in(
    slot: &mut Option<Ekk>,
    lp: &SimplexLp,
    scale: Option<&Scale>,
    options: &Options,
    basis: Option<&crate::Basis>,
    interrupt: Option<ekk::Interrupt>,
) -> SimplexResult {
    let moved = match scale {
        Some(scale) => scaled_lp(lp, scale),
        None => lp.clone(),
    };
    // HiGHS's `moveLp`.
    let ekk = match slot {
        Some(ekk) => {
            ekk.lp = moved;
            ekk.options = options.clone();
            ekk.simplex_in_scaled_space = scale.is_some();
            ekk.scaled_a = None;
            ekk.nla.scale = None;
            ekk.status.has_ar_matrix = false;
            ekk.reset_for_run();
            ekk
        }
        None => slot.insert(Ekk::new(options.clone(), moved, scale.is_some())),
    };
    ekk.interrupt = interrupt;
    if !ekk.status.has_basis
        && let Some(basis) = basis
    {
        ekk.set_basis(basis);
    }
    ekk.solve(false, options.simplex_strategy);
    let Some(scale) = scale else {
        return result(ekk);
    };
    let scaled_status = ekk.model_status;
    if scaled_status == ModelStatus::SolveError {
        return result(ekk);
    }
    let unscaled = unscaled_infeasibilities(ekk, scale);
    let (num_unscaled_primal, num_unscaled_dual) = (unscaled.num_primal.unwrap_or(0), unscaled.num_dual.unwrap_or(0));
    let scaled_optimality_but_unscaled_infeasibilities = scaled_status == ModelStatus::Optimal && (num_unscaled_primal > 0 || num_unscaled_dual > 0);
    // HiGHS's `scaled_objective_target_but_unscaled_primal_infeasibilities`
    // needs an objective target, which this solver has no option for.
    let scaled_objective_bound_but_unscaled_dual_infeasibilities = scaled_status == ModelStatus::ObjectiveBound && num_unscaled_dual > 0;
    let refine = scaled_optimality_but_unscaled_infeasibilities
        || matches!(
            scaled_status,
            ModelStatus::Infeasible | ModelStatus::UnboundedOrInfeasible | ModelStatus::Unbounded | ModelStatus::Unknown
        )
        || scaled_objective_bound_but_unscaled_dual_infeasibilities;
    if !refine {
        let mut out = result(ekk);
        out.infeasibilities = unscaled;
        unscale(&mut out, scale, ekk.lp.num_col);
        // HiGHS's `setNlaPointersForLpAndScale`: queries of the factor
        // after the solve are in the unscaled LP's terms.
        ekk.nla.scale = Some(scale.clone());
        return out;
    }

    // HiGHS takes the scaled solve's solution and basis, and unscales the
    // solution, before deciding on refinement; that is the solution kept
    // when the infeasibility proof holds for the unscaled LP.
    let mut scaled_out = result(ekk);
    unscale(&mut scaled_out, scale, ekk.lp.num_col);

    // Move the unscaled LP in, keeping the basis and its factor.
    ekk.lp = lp.clone();
    ekk.set_unscaled_with(scale.clone());
    if scaled_status == ModelStatus::Infeasible && ekk.dual_ray_record.is_some() && ekk.proof_of_primal_infeasibility_from_ray() {
        let mut out = scaled_out;
        out.status = ModelStatus::Infeasible;
        out.infeasibilities = unscaled;
        return out;
    }
    let primal = num_unscaled_primal == 0;
    let saved_strategy = ekk.info.dual_edge_weight_strategy;
    if !primal && ekk.status.has_basis && !ekk.status.has_dual_steepest_edge_weights {
        ekk.info.dual_edge_weight_strategy = EdgeWeightStrategy::Devex;
    }
    ekk.solve(true, if primal { SimplexStrategy::Primal } else { SimplexStrategy::Dual });
    if scaled_status != ModelStatus::ObjectiveBound && ekk.model_status == ModelStatus::ObjectiveBound && ekk.info.num_dual_infeasibilities > 0 {
        ekk.solve(true, SimplexStrategy::Primal);
    }
    ekk.info.dual_edge_weight_strategy = saved_strategy;
    result(ekk)
}

fn result(ekk: &mut Ekk) -> SimplexResult {
    let (col_value, col_dual, row_value, row_dual) = ekk.get_solution();
    SimplexResult {
        status: ekk.model_status,
        col_value,
        col_dual,
        row_value,
        row_dual,
        objective: ekk.info.primal_objective_value,
        iterations: ekk.iteration_count,
        basis: ekk.get_basis(),
        factor_pivot_threshold: ekk.info.factor_pivot_threshold,
        ipm_iterations: 0,
        crossover_iterations: 0,
        infeasibilities: Infeasibilities {
            num_primal: usize::try_from(ekk.info.num_primal_infeasibilities).ok(),
            max_primal: ekk.info.max_primal_infeasibility,
            sum_primal: ekk.info.sum_primal_infeasibilities,
            num_dual: usize::try_from(ekk.info.num_dual_infeasibilities).ok(),
            max_dual: ekk.info.max_dual_infeasibility,
            sum_dual: ekk.info.sum_dual_infeasibilities,
        },
    }
}

fn unscale(out: &mut SimplexResult, scale: &Scale, num_col: usize) {
    for col in 0..num_col {
        out.col_value[col] *= scale.col[col];
        out.col_dual[col] /= scale.col[col];
    }
    for row in 0..scale.row.len() {
        out.row_value[row] /= scale.row[row];
        out.row_dual[row] *= scale.row[row];
    }
}

/// HiGHS's `getUnscaledInfeasibilities`: the primal and dual
/// infeasibilities of the scaled solve's basis once unscaled.
fn unscaled_infeasibilities(ekk: &Ekk, scale: &Scale) -> Infeasibilities {
    let num_col = ekk.lp.num_col;
    let num_row = ekk.lp.num_row;
    let primal_tolerance = ekk.options.primal_feasibility_tolerance;
    let dual_tolerance = ekk.options.dual_feasibility_tolerance;
    let info = &ekk.info;
    let basis = &ekk.basis;
    let mut out = Infeasibilities {
        num_primal: Some(0),
        num_dual: Some(0),
        ..Infeasibilities::default()
    };
    let mut num_dual = 0;
    for var in 0..num_col + num_row {
        if basis.nonbasic_flag[var] == 0 || info.work_lower[var] == info.work_upper[var] {
            continue;
        }
        let scale_mu = if var < num_col { 1.0 / scale.col[var] } else { scale.row[var - num_col] };
        let unscaled_dual = info.work_dual[var] * scale_mu;
        let infeasibility = if info.work_lower[var] == f64::NEG_INFINITY && info.work_upper[var] == f64::INFINITY {
            unscaled_dual.abs()
        } else {
            -(basis.nonbasic_move[var] as f64) * unscaled_dual
        };
        if infeasibility > 0.0 {
            if infeasibility >= dual_tolerance {
                num_dual += 1;
            }
            out.max_dual = out.max_dual.max(infeasibility);
            out.sum_dual += infeasibility;
        }
    }
    let mut num_primal = 0;
    for row in 0..num_row {
        let var = basis.basic_index[row];
        let scale_mu = if var < num_col { scale.col[var] } else { 1.0 / scale.row[var - num_col] };
        let lower = info.base_lower[row] * scale_mu;
        let value = info.base_value[row] * scale_mu;
        let upper = info.base_upper[row] * scale_mu;
        let infeasibility = if value < lower - primal_tolerance {
            lower - value
        } else if value > upper + primal_tolerance {
            value - upper
        } else {
            0.0
        };
        if infeasibility > 0.0 {
            num_primal += 1;
            out.max_primal = out.max_primal.max(infeasibility);
            out.sum_primal += infeasibility;
        }
    }
    out.num_primal = Some(num_primal);
    out.num_dual = Some(num_dual);
    out
}

/// Build the simplex LP from column-wise data.
#[allow(clippy::too_many_arguments)]
pub(crate) fn simplex_lp(
    sense: f64,
    offset: f64,
    col_cost: Vec<f64>,
    col_lower: Vec<f64>,
    col_upper: Vec<f64>,
    row_lower: Vec<f64>,
    row_upper: Vec<f64>,
    start: Vec<usize>,
    index: Vec<usize>,
    value: Vec<f64>,
) -> SimplexLp {
    let num_col = col_cost.len();
    let num_row = row_lower.len();
    SimplexLp {
        num_col,
        num_row,
        sense,
        offset,
        col_cost,
        col_lower,
        col_upper,
        row_lower,
        row_upper,
        a: ColWise {
            num_col,
            num_row,
            start,
            index,
            value,
        },
    }
}
