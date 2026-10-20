//! HiGHS's side of IPX (`IpxWrapper.cpp`, `solveLp` in `HighsSolve.cpp`):
//! the LP handed over with a slack column per boxed row, IPX's basic
//! solution taken back, and a simplex cleanup when IPX cannot say the
//! LP is solved.

use super::{
    control::{Control, Parameters},
    lp_solver::LpSolver,
    model::Model,
    sparse::{Int, u},
    *,
};
use crate::{
    BasisStatus, Options,
    simplex::{
        self, ModelStatus, SimplexResult,
        ekk::{Interrupt, SimplexLp},
    },
};

const INF: f64 = f64::INFINITY;

/// HiGHS's `solveLp` with `solver = "ipm"` and crossover on: IPX, then
/// the simplex method from IPX's basis (or from scratch, without one)
/// when IPX's status is not one HiGHS accepts.
pub(crate) fn solve_lp(lp: &SimplexLp, options: &Options, interrupt: Option<Interrupt>) -> SimplexResult {
    let mut result = solve_lp_ipx(lp, options, interrupt.clone());
    if result.status == ModelStatus::Unknown || result.status == ModelStatus::UnboundedOrInfeasible {
        let basis = result.basis_valid.then_some(&result.out.basis);
        let mut cleanup = simplex::solve_lp_simplex(lp.clone(), options, basis, interrupt);
        cleanup.ipm_iterations = result.out.ipm_iterations;
        cleanup.crossover_iterations = result.out.crossover_iterations;
        return cleanup;
    }
    result.out.status = result.status;
    result.out
}

/// The analytic centre of `lp`'s feasible region, as the MIP solver asks
/// HiGHS for it: IPX on the LP (whose costs are zero) without crossover and
/// with centring steps (`run_centring`), at most 200 iterations. Returns
/// HiGHS's model status and, when IPX gave one, the interior point's
/// column values.
///
/// The status is the one `Highs::optimizeLp` reports, after
/// `callLpKktCheck` has judged the interior point: IPX can call a centre
/// optimal whose optimality conditions are far off, and HiGHS then reports
/// it unknown, so the MIP solver fixes nothing from it. Without that check
/// a centre still short of the region's interior put columns at a bound
/// that feasible points leave it, and fixing them cut off the optimum.
pub(crate) fn analytic_centre(lp: &SimplexLp, options: &Options, interrupt: Option<Interrupt>) -> (ModelStatus, Option<Vec<f64>>) {
    let parameters = Parameters {
        dualize: -1,
        ipm_feasibility_tol: options.primal_feasibility_tolerance.min(options.dual_feasibility_tolerance),
        ipm_optimality_tol: options.ipm_optimality_tolerance,
        // Without crossover, IPX stops at its own tolerances.
        start_crossover_tol: -1.0,
        time_limit: options.time_limit,
        ipm_maxiter: 200,
        run_crossover: 0,
        run_centring: 1,
        max_centring_steps: 5,
        centring_ratio_tolerance: 100.0,
        ..Parameters::default()
    };
    let mut control = Control::new(parameters, interrupt);
    let data = fill_in_ipx_data(lp);
    let mut model = Model::default();
    let load_status = model.load(
        control.params.dualize,
        control.params.scale,
        data.num_row,
        data.num_col,
        &data.ap,
        &data.ai,
        &data.ax,
        &data.rhs,
        &data.constraint_type,
        data.offset,
        &data.obj,
        &data.col_lb,
        &data.col_ub,
    );
    if load_status != 0 {
        return (ModelStatus::SolveError, None);
    }
    let mut lps = LpSolver::default();
    let solve_status = lps.solve(&mut control, &model);
    let info = &lps.info;
    if solve_status != IPX_STATUS_SOLVED && solve_status != IPX_STATUS_STOPPED {
        return (ModelStatus::SolveError, None);
    }
    let error_status = |status| matches!(status, IPX_STATUS_FAILED | IPX_STATUS_DEBUG);
    if error_status(info.status_ipm) || error_status(info.status_crossover) {
        return (ModelStatus::SolveError, None);
    }
    let mut status = if solve_status == IPX_STATUS_STOPPED {
        match info.status_ipm {
            IPX_STATUS_USER_INTERRUPT => ModelStatus::Interrupt,
            IPX_STATUS_TIME_LIMIT => ModelStatus::TimeLimit,
            IPX_STATUS_ITER_LIMIT => ModelStatus::IterationLimit,
            _ => ModelStatus::Unknown,
        }
    } else if info.status_ipm == IPX_STATUS_PRIMAL_INFEAS {
        ModelStatus::Infeasible
    } else if info.status_ipm == IPX_STATUS_DUAL_INFEAS {
        ModelStatus::UnboundedOrInfeasible
    } else if info.status_ipm != IPX_STATUS_OPTIMAL && info.status_ipm != IPX_STATUS_IMPRECISE {
        return (ModelStatus::SolveError, None);
    } else if info.status_ipm == IPX_STATUS_IMPRECISE {
        // HiGHS does no simplex cleanup when centring.
        ModelStatus::Unknown
    } else {
        ModelStatus::Optimal
    };
    // `getHighsNonVertexSolution`, then `callLpKktCheck`.
    let Some((x, _, _, slack, y, zl, zu)) = lps.interior_solution(&model) else {
        return (status, None);
    };
    let [col_value, row_value, col_dual, row_dual] = interior_solution_to_highs(lp, &data, &x, &slack, &y, &zl, &zu);
    let kkt_lp = crate::kkt::KktLp {
        sense: lp.sense,
        offset: lp.offset,
        col_cost: &lp.col_cost,
        col_lower: &lp.col_lower,
        col_upper: &lp.col_upper,
        row_lower: &lp.row_lower,
        row_upper: &lp.row_upper,
        start: &lp.a.start,
        index: &lp.a.index,
        value: &lp.a.value,
    };
    let solution = crate::kkt::KktSolution {
        col_value: &col_value,
        row_value: &row_value,
        col_dual: &col_dual,
        row_dual: &row_dual,
        dual_valid: true,
    };
    let tolerances = crate::kkt::KktTolerances::new(options.primal_feasibility_tolerance, options.dual_feasibility_tolerance);
    crate::kkt::lp_kkt_check(&kkt_lp, &solution, false, tolerances, &mut status);
    (status, Some(col_value))
}

/// HiGHS's `ipxSolutionToHighsSolution`: an IPX interior point as column
/// and row values and duals, `[col_value, row_value, col_dual, row_dual]`.
/// A boxed row's value and dual are its slack column's; a free row, which
/// IPX does not have, takes its activity.
fn interior_solution_to_highs(lp: &SimplexLp, data: &IpxData, x: &[f64], slack: &[f64], y: &[f64], zl: &[f64], zu: &[f64]) -> [Vec<f64>; 4] {
    let col_value = x[..lp.num_col].to_vec();
    let mut col_dual: Vec<f64> = (0..lp.num_col).map(|col| zl[col] - zu[col]).collect();
    let mut activity = vec![0.0; lp.num_row];
    if u(data.num_row) < lp.num_row {
        for col in 0..lp.num_col {
            for el in lp.a.start[col]..lp.a.start[col + 1] {
                activity[lp.a.index[el]] += col_value[col] * lp.a.value[el];
            }
        }
    }
    let mut row_value = vec![0.0; lp.num_row];
    let mut row_dual = vec![0.0; lp.num_row];
    let (mut ipx_row, mut ipx_slack) = (0, lp.num_col);
    for row in 0..lp.num_row {
        let (lower, upper) = (lp.row_lower[row], lp.row_upper[row]);
        if lower <= -INF && upper >= INF {
            row_value[row] = activity[row];
            continue;
        }
        if lower > -INF && upper < INF && lower < upper {
            row_value[row] = x[ipx_slack];
            row_dual[row] = zl[ipx_slack] - zu[ipx_slack];
            ipx_slack += 1;
        } else {
            row_value[row] = data.rhs[ipx_row] - slack[ipx_row];
            row_dual[row] = y[ipx_row];
        }
        ipx_row += 1;
    }
    if lp.sense < 0.0 {
        col_dual.iter_mut().chain(row_dual.iter_mut()).for_each(|dual| *dual = -*dual);
    }
    [col_value, row_value, col_dual, row_dual]
}

struct IpxResult {
    status: ModelStatus,
    out: SimplexResult,
    basis_valid: bool,
}

/// HiGHS's `solveLpIpx`.
fn solve_lp_ipx(lp: &SimplexLp, options: &Options, interrupt: Option<Interrupt>) -> IpxResult {
    let mut out = SimplexResult {
        status: ModelStatus::NotSet,
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
    };
    let failed = |status, out| IpxResult { status, out, basis_valid: false };
    let parameters = Parameters {
        dualize: -1,
        ipm_feasibility_tol: options.primal_feasibility_tolerance.min(options.dual_feasibility_tolerance),
        ipm_optimality_tol: options.ipm_optimality_tolerance,
        start_crossover_tol: options.start_crossover_tolerance,
        time_limit: options.time_limit,
        ipm_maxiter: options.ipm_iteration_limit.min(i32::MAX as usize) as Int,
        run_crossover: 1,
        ..Parameters::default()
    };
    let mut control = Control::new(parameters, interrupt);
    let data = fill_in_ipx_data(lp);
    let mut model = Model::default();
    let load_status = model.load(
        control.params.dualize,
        control.params.scale,
        data.num_row,
        data.num_col,
        &data.ap,
        &data.ai,
        &data.ax,
        &data.rhs,
        &data.constraint_type,
        data.offset,
        &data.obj,
        &data.col_lb,
        &data.col_ub,
    );
    if load_status != 0 {
        return failed(ModelStatus::SolveError, out);
    }
    let mut lps = LpSolver::default();
    let solve_status = lps.solve(&mut control, &model);
    let info = &lps.info;
    out.ipm_iterations = u(info.iter);
    out.crossover_iterations = u(info.updates_crossover);
    if solve_status != IPX_STATUS_SOLVED && solve_status != IPX_STATUS_STOPPED {
        return failed(ModelStatus::SolveError, out);
    }
    let error_status = |status| matches!(status, IPX_STATUS_FAILED | IPX_STATUS_DEBUG);
    if error_status(info.status_ipm) || error_status(info.status_crossover) {
        return failed(ModelStatus::SolveError, out);
    }
    if solve_status == IPX_STATUS_STOPPED {
        // HiGHS hands back the interior point here; nothing reads it.
        let status = if info.status_crossover == IPX_STATUS_TIME_LIMIT {
            ModelStatus::TimeLimit
        } else {
            match info.status_ipm {
                IPX_STATUS_USER_INTERRUPT => ModelStatus::Interrupt,
                IPX_STATUS_TIME_LIMIT => ModelStatus::TimeLimit,
                IPX_STATUS_ITER_LIMIT => ModelStatus::IterationLimit,
                _ => ModelStatus::Unknown,
            }
        };
        return failed(status, out);
    }
    if info.status_ipm == IPX_STATUS_PRIMAL_INFEAS {
        return failed(ModelStatus::Infeasible, out);
    }
    if info.status_ipm == IPX_STATUS_DUAL_INFEAS {
        return failed(ModelStatus::UnboundedOrInfeasible, out);
    }
    if info.status_ipm != IPX_STATUS_OPTIMAL && info.status_ipm != IPX_STATUS_IMPRECISE {
        return failed(ModelStatus::SolveError, out);
    }
    if info.status_crossover != IPX_STATUS_NOT_RUN && info.status_crossover != IPX_STATUS_OPTIMAL && info.status_crossover != IPX_STATUS_IMPRECISE {
        return failed(ModelStatus::SolveError, out);
    }
    let have_basic_solution = info.status_crossover != IPX_STATUS_NOT_RUN;
    let imprecise_solution = info.status_crossover == IPX_STATUS_IMPRECISE || info.status_ipm == IPX_STATUS_IMPRECISE;
    let mut basis_valid = false;
    if have_basic_solution {
        let Some((x, slack, y, z, cbasis, vbasis)) = lps.basic_solution(&model) else {
            return failed(ModelStatus::SolveError, out);
        };
        if !basic_solution_to_highs(lp, &data, &x, &slack, &y, &z, &cbasis, &vbasis, &mut out) {
            return failed(ModelStatus::SolveError, out);
        }
        basis_valid = true;
    }
    // `getKktFailures` sets HiGHS's objective from the column values.
    let mut objective = lp.offset;
    for col in 0..lp.num_col {
        objective += lp.col_cost[col] * out.col_value[col];
    }
    out.objective = objective;
    let status = if imprecise_solution { ModelStatus::Unknown } else { ModelStatus::Optimal };
    IpxResult { status, out, basis_valid }
}

struct IpxData {
    num_col: Int,
    num_row: Int,
    offset: f64,
    obj: Vec<f64>,
    col_lb: Vec<f64>,
    col_ub: Vec<f64>,
    ap: Vec<Int>,
    ai: Vec<Int>,
    ax: Vec<f64>,
    rhs: Vec<f64>,
    constraint_type: Vec<u8>,
}

/// HiGHS's `fillInIpxData` and `fillInRhsAndConstraints`. As there, a
/// boxed row's slack takes the row's index in the LP, not with free rows
/// removed (presolve leaves no free rows).
fn fill_in_ipx_data(lp: &SimplexLp) -> IpxData {
    let num_row = lp.num_row;
    let (row_lower, row_upper) = (&lp.row_lower, &lp.row_upper);
    let mut general_bounded_rows = Vec::new();
    let mut free_rows = Vec::new();
    for row in 0..num_row {
        if row_lower[row] < row_upper[row] && row_lower[row] > -INF && row_upper[row] < INF {
            general_bounded_rows.push(row);
        } else if row_lower[row] <= -INF && row_upper[row] >= INF {
            free_rows.push(row);
        }
    }
    let num_slack = general_bounded_rows.len();
    let mut rhs = Vec::with_capacity(num_row);
    let mut constraint_type = Vec::with_capacity(num_row);
    for row in 0..num_row {
        let (lower, upper) = (row_lower[row], row_upper[row]);
        if lower > -INF && upper >= INF {
            rhs.push(lower);
            constraint_type.push(b'>');
        } else if lower <= -INF && upper < INF {
            rhs.push(upper);
            constraint_type.push(b'<');
        } else if lower == upper {
            rhs.push(upper);
            constraint_type.push(b'=');
        } else if lower > -INF && upper < INF {
            rhs.push(0.0);
            constraint_type.push(b'=');
        }
    }
    let mut reduced_rowmap = vec![-1 as Int; num_row];
    let mut counter = 0;
    let mut findex = 0;
    for (row, mapped) in reduced_rowmap[..num_row].iter_mut().enumerate() {
        if findex < free_rows.len() && free_rows[findex] == row {
            findex += 1;
        } else {
            *mapped = counter;
            counter += 1;
        }
    }
    let num_col = lp.num_col + num_slack;
    let constrained = |row: usize| row_lower[row] > -INF || row_upper[row] < INF;
    let mut ap = vec![0 as Int; num_col + 1];
    for col in 0..lp.num_col {
        let size = (lp.a.start[col]..lp.a.start[col + 1]).filter(|&k| constrained(lp.a.index[k])).count();
        ap[col + 1] = ap[col] + size as Int;
    }
    for col in lp.num_col..num_col {
        ap[col + 1] = ap[col] + 1;
    }
    let nnz = lp.a.start[lp.num_col];
    let mut ai = Vec::with_capacity(nnz + num_slack);
    let mut ax = Vec::with_capacity(nnz + num_slack);
    for k in 0..nnz {
        let row = lp.a.index[k];
        if constrained(row) {
            ai.push(reduced_rowmap[row]);
            ax.push(lp.a.value[k]);
        }
    }
    for &row in &general_bounded_rows {
        ai.push(row as Int);
        ax.push(-1.0);
    }
    let mut col_lb = vec![0.0; num_col];
    let mut col_ub = vec![0.0; num_col];
    for col in 0..lp.num_col {
        col_lb[col] = if lp.col_lower[col] <= -INF { -INF } else { lp.col_lower[col] };
        col_ub[col] = if lp.col_upper[col] >= INF { INF } else { lp.col_upper[col] };
    }
    for (slack, &row) in general_bounded_rows.iter().enumerate() {
        col_lb[lp.num_col + slack] = row_lower[row];
        col_ub[lp.num_col + slack] = row_upper[row];
    }
    let mut obj = vec![0.0; num_col];
    for col in 0..lp.num_col {
        obj[col] = lp.sense * lp.col_cost[col];
    }
    IpxData {
        num_col: num_col as Int,
        num_row: (num_row - free_rows.len()) as Int,
        offset: lp.sense * lp.offset,
        obj,
        col_lb,
        col_ub,
        ap,
        ai,
        ax,
        rhs,
        constraint_type,
    }
}

/// HiGHS's `ipxBasicSolutionToHighsBasicSolution`. IPX's `x`, `z` and
/// `vbasis` cover the columns and then the boxed rows' slacks; `slack`,
/// `y` and `cbasis` its rows.
#[allow(clippy::too_many_arguments)]
fn basic_solution_to_highs(lp: &SimplexLp, data: &IpxData, x: &[f64], slack: &[f64], y: &[f64], z: &[f64], cbasis: &[Int], vbasis: &[Int], out: &mut SimplexResult) -> bool {
    let (rhs, constraint_type) = (&data.rhs, &data.constraint_type);
    let get_row_activities = (data.num_row as usize) < lp.num_row;
    let mut row_activity = if get_row_activities { vec![0.0; lp.num_row] } else { Vec::new() };
    let mut num_basic_variables = 0;
    for col in 0..lp.num_col {
        let (status, dual) = match vbasis[col] {
            IPX_BASIC => (BasisStatus::Basic, 0.0),
            IPX_NONBASIC_LB => (BasisStatus::Lower, z[col]),
            IPX_NONBASIC_UB => (BasisStatus::Upper, z[col]),
            IPX_SUPERBASIC => (BasisStatus::Zero, z[col]),
            _ => return false,
        };
        out.basis.col_status[col] = status;
        out.col_value[col] = x[col];
        out.col_dual[col] = dual;
        if get_row_activities {
            for el in lp.a.start[col]..lp.a.start[col + 1] {
                row_activity[lp.a.index[el]] += out.col_value[col] * lp.a.value[el];
            }
        }
        if status == BasisStatus::Basic {
            num_basic_variables += 1;
        }
    }
    let mut ipx_row = 0;
    let mut ipx_slack = lp.num_col;
    for row in 0..lp.num_row {
        let (lower, upper) = (lp.row_lower[row], lp.row_upper[row]);
        let (status, value, dual);
        if lower <= -INF && upper >= INF {
            (status, value, dual) = (BasisStatus::Basic, row_activity[row], 0.0);
        } else {
            if lower > -INF && upper < INF && lower < upper {
                let (slack_value, slack_dual) = (x[ipx_slack], z[ipx_slack]);
                (status, value, dual) = if cbasis[ipx_row] == IPX_BASIC || vbasis[ipx_slack] == IPX_BASIC {
                    (BasisStatus::Basic, slack_value, 0.0)
                } else if vbasis[ipx_slack] == IPX_NONBASIC_LB {
                    (BasisStatus::Lower, slack_value, slack_dual)
                } else if vbasis[ipx_slack] == IPX_NONBASIC_UB {
                    (BasisStatus::Upper, slack_value, slack_dual)
                } else {
                    return false;
                };
                ipx_slack += 1;
            } else if cbasis[ipx_row] == IPX_BASIC {
                (status, value, dual) = (BasisStatus::Basic, rhs[ipx_row] - slack[ipx_row], 0.0);
            } else {
                let row_value = rhs[ipx_row] - slack[ipx_row];
                let row_dual = y[ipx_row];
                let row_status = match constraint_type[ipx_row] {
                    b'>' => BasisStatus::Lower,
                    b'<' => BasisStatus::Upper,
                    b'=' if row_dual >= 0.0 => BasisStatus::Lower,
                    b'=' => BasisStatus::Upper,
                    _ => return false,
                };
                (status, value, dual) = (row_status, row_value, row_dual);
            }
            ipx_row += 1;
        }
        out.basis.row_status[row] = status;
        out.row_value[row] = value;
        out.row_dual[row] = dual;
        if status == BasisStatus::Basic {
            num_basic_variables += 1;
        }
    }
    debug_assert_eq!(num_basic_variables, lp.num_row);
    if lp.sense < 0.0 {
        for dual in out.col_dual.iter_mut().chain(out.row_dual.iter_mut()) {
            *dual *= -1.0;
        }
    }
    true
}
