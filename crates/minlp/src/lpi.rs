//! SCIP's LP solver interface (`lpi/lpi_highs.cpp`) over crates/solver,
//! the Rust port of the HiGHS this interface was written for.

/// `SCIP_OBJSEN`.
pub const OBJSEN_MAXIMIZE: i32 = -1;
pub const OBJSEN_MINIMIZE: i32 = 1;

/// `SCIP_LPPARAM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpParam {
    FromScratch = 0,
    FastMip = 1,
    Scaling = 2,
    Presolving = 3,
    Pricing = 4,
    LpInfo = 5,
    Feastol = 6,
    DualFeastol = 7,
    BarrierConvTol = 8,
    ObjLim = 9,
    LpItLim = 10,
    LpTiLim = 11,
    Markowitz = 12,
    RowRepSwitch = 13,
    Threads = 14,
    ConditionLimit = 15,
    Timing = 16,
    RandomSeed = 17,
    Polishing = 18,
    Refactor = 19,
}

/// `SCIP_BASESTAT`.
pub const BASESTAT_LOWER: u32 = 0;
pub const BASESTAT_BASIC: u32 = 1;
pub const BASESTAT_UPPER: u32 = 2;
pub const BASESTAT_ZERO: u32 = 3;

/// `SCIP_LPSOLQUALITY`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpSolQuality {
    EstimCondition = 0,
    ExactCondition = 1,
}

/// `SCIP_PRICING`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pricing {
    LpiDefault = 0,
    Auto = 1,
    Full = 2,
    Partial = 3,
    Steep = 4,
    SteepQStart = 5,
    Devex = 6,
}

/// `SCIP_LPISTATE`: a basis. C packs the statuses two bits each; the
/// packing is never observable.
#[derive(Clone, Debug, Default)]
pub struct LpiState {
    pub ncols: i32,
    pub nrows: i32,
    pub cstat: Vec<i32>,
    pub rstat: Vec<i32>,
}

/// `SCIP_LPINORMS`: the HiGHS interface stores none.
#[derive(Clone, Debug, Default)]
pub struct LpiNorms;

use solver::{Basis, BasisStatus, EdgeWeightStrategy, LpSolver, Options, Problem, SimplexStrategy, Status};

use crate::retcode::{Retcode, ScipResult};

/// `SCIP_LPI`: the HiGHS interface's data, over crates/solver's `Highs`.
pub struct Lpi {
    pub highs: LpSolver,
    pub nthreads: i32,
    pub fromscratch: bool,
    pub solved: bool,
    pub presolve: bool,
    pub pricing: Pricing,
}

/// `HIGHS_relDiff`.
fn highs_rel_diff(val1: f64, val2: f64) -> f64 {
    (val1 - val2) / crate::def::max3(1.0, val1.abs(), val2.abs())
}

/// `basestatToHighsBasisStatus`.
fn basestat_to_highs_basis_status(stat: i32) -> BasisStatus {
    match stat as u32 {
        BASESTAT_LOWER => BasisStatus::Lower,
        BASESTAT_BASIC => BasisStatus::Basic,
        BASESTAT_UPPER => BasisStatus::Upper,
        BASESTAT_ZERO => BasisStatus::Zero,
        _ => {
            log::error!("Unknown Basis Status returned. Please use supported HiGHS version!");
            BasisStatus::Zero
        }
    }
}

/// `(int) HighsBasisStatus`: kLower 0, kBasic 1, kUpper 2, kZero 3,
/// kNonbasic 4.
fn highs_basis_status_to_int(status: BasisStatus) -> i32 {
    match status {
        BasisStatus::Lower => 0,
        BasisStatus::Basic => 1,
        BasisStatus::Upper => 2,
        BasisStatus::Zero => 3,
        BasisStatus::Nonbasic => 4,
    }
}

/// `invalidateSolution`.
fn invalidate_solution(lpi: &mut Lpi) {
    lpi.solved = false;
}

/// `lpiSolve`: runs HiGHS and checks its status; with presolve on and no
/// invertible basis afterwards, solves again without presolve.
fn lpi_solve(lpi: &mut Lpi) -> ScipResult {
    if lpi.fromscratch {
        lpi.highs.clear_solver();
    }
    lpi.highs.options.presolve = lpi.presolve;

    // The optimization result may be reliable even if HiGHS returns a
    // warning status, e.g., HiGHS always returns with a warning status if
    // the iteration limit was hit.
    let model_status = lpi.highs.run();
    match model_status {
        Status::ModelEmpty
        | Status::Optimal
        | Status::Infeasible
        | Status::UnboundedOrInfeasible
        | Status::Unbounded
        | Status::ObjectiveBound
        | Status::TimeLimit
        | Status::IterationLimit => {}
        _ => {
            log::error!("HiGHS terminated with model status <{model_status:?}> after simplex strategy <Dual (serial)> (1)");
            return Err(Retcode::LpError);
        }
    }
    if !lpi.highs.has_invert() && lpi.highs.options.presolve {
        lpi.presolve = false;
        let retcode = lpi_solve(lpi);
        if retcode.is_err() {
            log::error!("HiGHS terminated with model status <{:?}> after trying to recover inverse", lpi.highs.status());
        }
        lpi.presolve = true;
        retcode?;
    }
    lpi.solved = true;
    Ok(())
}

/// `SCIPlpiGetSolverName`.
pub fn lpi_get_solver_name() -> &'static str {
    "HiGHS 1.15.0"
}

/// `SCIPlpiSetIntegralityInformation`: not implemented by the HiGHS
/// interface.
pub fn lpi_set_integrality_information(_lpi: &mut Lpi, _int_info: &[i32]) -> ScipResult {
    log::error!("SCIPlpiSetIntegralityInformation() has not been implemented yet");
    Err(Retcode::LpError)
}

/// `SCIPlpiHasBarrierSolve`.
pub fn lpi_has_barrier_solve() -> bool {
    false
}

/// `SCIPlpiCreate`: creates an LP problem object.
pub fn lpi_create(_name: &str, objsen: i32) -> ScipResult<Lpi> {
    let highs = LpSolver::new(&Problem::default(), Options::default());
    let mut lpi = Lpi {
        highs,
        nthreads: 1,
        fromscratch: false,
        solved: false,
        presolve: true,
        pricing: Pricing::LpiDefault,
    };
    lpi.highs.clear_model();
    lpi.highs.report_empty_model();
    invalidate_solution(&mut lpi);
    lpi_chg_objsen(&mut lpi, objsen)?;
    lpi_set_intpar(&mut lpi, LpParam::Scaling, 1)?;
    lpi_set_intpar(&mut lpi, LpParam::Presolving, 1)?;
    lpi.highs.options.lp_presolve_requires_basis_postsolve = true;
    let pricing = lpi.pricing as i32;
    lpi_set_intpar(&mut lpi, LpParam::Pricing, pricing)?;
    Ok(lpi)
}

/// `SCIPlpiLoadColLP`: copies LP data with column matrix into LP solver;
/// column `j` has the entries `ind[beg[j]..beg[j + 1]]` (the last up to
/// `ind.len()`).
#[allow(clippy::too_many_arguments)]
pub fn lpi_load_col_lp(lpi: &mut Lpi, objsen: i32, obj: &[f64], lb: &[f64], ub: &[f64], lhs: &[f64], rhs: &[f64], beg: &[i32], ind: &[i32], val: &[f64]) -> ScipResult {
    let sense = if objsen == OBJSEN_MINIMIZE { solver::Sense::Minimise } else { solver::Sense::Maximise };
    let beg: Vec<usize> = beg.iter().map(|&b| b as usize).collect();
    let ind: Vec<usize> = ind.iter().map(|&i| i as usize).collect();
    lpi.highs.pass_model(sense, 0.0, obj, lb, ub, lhs, rhs, &beg, &ind, val);
    Ok(())
}

/// `SCIPlpiAddCols`: adds columns to the LP; column `j` has the entries
/// `ind[beg[j]..beg[j + 1]]` (the last up to `ind.len()`).
pub fn lpi_add_cols(lpi: &mut Lpi, obj: &[f64], lb: &[f64], ub: &[f64], beg: &[i32], ind: &[i32], val: &[f64]) -> ScipResult {
    invalidate_solution(lpi);
    let beg: Vec<usize> = beg.iter().map(|&b| b as usize).collect();
    let ind: Vec<usize> = ind.iter().map(|&i| i as usize).collect();
    lpi.highs.add_cols(obj, lb, ub, &beg, &ind, val);
    Ok(())
}

/// `SCIPlpiDelCols`: deletes all columns in the given range.
pub fn lpi_del_cols(lpi: &mut Lpi, firstcol: i32, lastcol: i32) -> ScipResult {
    // Handle empty range.
    if firstcol > lastcol {
        return Ok(());
    }
    invalidate_solution(lpi);
    lpi.highs.delete_cols_range(firstcol as usize, lastcol as usize);
    Ok(())
}

/// `SCIPlpiDelColset`: deletes columns with `dstat` 1, writing each
/// column's new position (-1 if deleted) into `dstat`.
pub fn lpi_del_colset(lpi: &mut Lpi, dstat: &mut [i32]) -> ScipResult {
    invalidate_solution(lpi);
    let delete: Vec<bool> = dstat.iter().map(|&d| d != 0).collect();
    let new_index = lpi.highs.delete_cols(&delete);
    for (d, n) in dstat.iter_mut().zip(new_index) {
        *d = if n == solver::NONE { -1 } else { n as i32 };
    }
    Ok(())
}

/// `SCIPlpiAddRows`: adds rows to the LP; row `i` has the entries
/// `ind[beg[i]..beg[i + 1]]` (the last up to `ind.len()`).
pub fn lpi_add_rows(lpi: &mut Lpi, lhs: &[f64], rhs: &[f64], beg: &[i32], ind: &[i32], val: &[f64]) -> ScipResult {
    invalidate_solution(lpi);
    let beg: Vec<usize> = beg.iter().map(|&b| b as usize).collect();
    let ind: Vec<usize> = ind.iter().map(|&i| i as usize).collect();
    lpi.highs.add_rows(lhs, rhs, &beg, &ind, val);
    Ok(())
}

/// `SCIPlpiDelRows`: deletes all rows in the given range.
pub fn lpi_del_rows(lpi: &mut Lpi, firstrow: i32, lastrow: i32) -> ScipResult {
    // Handle empty range.
    if firstrow > lastrow {
        return Ok(());
    }
    invalidate_solution(lpi);
    lpi.highs.delete_rows_range(firstrow as usize, lastrow as usize);
    Ok(())
}

/// `SCIPlpiDelRowset`: deletes rows with `dstat` 1, writing each row's new
/// position (-1 if deleted) into `dstat`.
pub fn lpi_del_rowset(lpi: &mut Lpi, dstat: &mut [i32]) -> ScipResult {
    invalidate_solution(lpi);
    let delete: Vec<bool> = dstat.iter().map(|&d| d != 0).collect();
    let new_index = lpi.highs.delete_rows(&delete);
    for (d, n) in dstat.iter_mut().zip(new_index) {
        *d = if n == solver::NONE { -1 } else { n as i32 };
    }
    Ok(())
}

/// `SCIPlpiClear`: clears the whole LP.
pub fn lpi_clear(lpi: &mut Lpi) -> ScipResult {
    invalidate_solution(lpi);
    lpi.highs.clear_model();
    Ok(())
}

/// `SCIPlpiChgBounds`: changes lower and upper bounds of columns.
pub fn lpi_chg_bounds(lpi: &mut Lpi, ind: &[i32], lb: &[f64], ub: &[f64]) -> ScipResult {
    invalidate_solution(lpi);
    for i in 0..ind.len() {
        if lpi_is_infinity(lpi, lb[i]) {
            log::error!("LP Error: fixing lower bound for variable {} to infinity", ind[i]);
            return Err(Retcode::LpError);
        }
        if lpi_is_infinity(lpi, -ub[i]) {
            log::error!("LP Error: fixing upper bound for variable {} to -infinity", ind[i]);
            return Err(Retcode::LpError);
        }
    }
    lpi.highs.change_col_bounds((0..ind.len()).map(|i| (ind[i] as usize, lb[i], ub[i])));
    Ok(())
}

/// `SCIPlpiChgSides`: changes left and right hand sides of rows.
pub fn lpi_chg_sides(lpi: &mut Lpi, ind: &[i32], lhs: &[f64], rhs: &[f64]) -> ScipResult {
    invalidate_solution(lpi);
    lpi.highs.change_row_bounds((0..ind.len()).map(|i| (ind[i] as usize, lhs[i], rhs[i])));
    Ok(())
}

/// `SCIPlpiChgCoef`: changes a single coefficient.
pub fn lpi_chg_coef(lpi: &mut Lpi, row: i32, col: i32, newval: f64) -> ScipResult {
    invalidate_solution(lpi);
    lpi.highs.change_coeff(row as usize, col as usize, newval);
    Ok(())
}

/// `SCIPlpiChgObjsen`: changes the objective sense.
pub fn lpi_chg_objsen(lpi: &mut Lpi, objsen: i32) -> ScipResult {
    invalidate_solution(lpi);
    lpi.highs
        .change_sense(if objsen == OBJSEN_MINIMIZE { solver::Sense::Minimise } else { solver::Sense::Maximise });
    Ok(())
}

/// `SCIPlpiChgObj`: changes objective values of columns in the LP.
pub fn lpi_chg_obj(lpi: &mut Lpi, ind: &[i32], obj: &[f64]) -> ScipResult {
    invalidate_solution(lpi);
    lpi.highs.change_col_costs((0..ind.len()).map(|i| (ind[i] as usize, obj[i])));
    Ok(())
}

/// `SCIPlpiGetNRows`.
pub fn lpi_get_nrows(lpi: &Lpi) -> i32 {
    lpi.highs.num_rows() as i32
}

/// `SCIPlpiGetNCols`.
pub fn lpi_get_ncols(lpi: &Lpi) -> i32 {
    lpi.highs.num_cols() as i32
}

/// `SCIPlpiGetObj`: gets objective coefficients from LP problem object.
pub fn lpi_get_obj(lpi: &Lpi, firstcol: i32, lastcol: i32, vals: &mut [f64]) {
    for i in firstcol..=lastcol {
        vals[(i - firstcol) as usize] = lpi.highs.col_cost()[i as usize];
    }
}

/// `SCIPlpiGetBounds`: gets current bounds from LP problem object.
pub fn lpi_get_bounds(lpi: &Lpi, firstcol: i32, lastcol: i32, mut lbs: Option<&mut [f64]>, mut ubs: Option<&mut [f64]>) {
    for i in firstcol..=lastcol {
        if let Some(lbs) = lbs.as_deref_mut() {
            lbs[(i - firstcol) as usize] = lpi.highs.col_lower()[i as usize];
        }
        if let Some(ubs) = ubs.as_deref_mut() {
            ubs[(i - firstcol) as usize] = lpi.highs.col_upper()[i as usize];
        }
    }
}

/// `SCIPlpiGetSides`: gets current row sides from LP problem object.
pub fn lpi_get_sides(lpi: &Lpi, firstrow: i32, lastrow: i32, mut lhss: Option<&mut [f64]>, mut rhss: Option<&mut [f64]>) {
    for i in firstrow..=lastrow {
        if let Some(lhss) = lhss.as_deref_mut() {
            lhss[(i - firstrow) as usize] = lpi.highs.row_lower()[i as usize];
        }
        if let Some(rhss) = rhss.as_deref_mut() {
            rhss[(i - firstrow) as usize] = lpi.highs.row_upper()[i as usize];
        }
    }
}

/// `SCIPlpiSolvePrimal`: the reference build has no
/// `SCIP_WITH_HIGHSPRIMAL`, so this solves with the dual simplex.
pub fn lpi_solve_primal(lpi: &mut Lpi) -> ScipResult {
    lpi_solve_dual(lpi)
}

/// `SCIPlpiSolveDual`: calls the serial dual simplex (no
/// `SCIP_WITH_HIGHSPARALLEL`).
pub fn lpi_solve_dual(lpi: &mut Lpi) -> ScipResult {
    lpi.highs.options.simplex_strategy = SimplexStrategy::Dual;
    lpi_solve(lpi)
}

/// `SCIPlpiSolveBarrier`: HiGHS's interface solves with the dual simplex.
pub fn lpi_solve_barrier(lpi: &mut Lpi, _crossover: bool) -> ScipResult {
    lpi_solve_dual(lpi)
}

/// `SCIPlpiStartStrongbranch`.
pub fn lpi_start_strongbranch(_lpi: &mut Lpi) -> ScipResult {
    Ok(())
}

/// `SCIPlpiEndStrongbranch`.
pub fn lpi_end_strongbranch(_lpi: &mut Lpi) -> ScipResult {
    Ok(())
}

/// The dual bound the HiGHS interface reports for any strong branching:
/// the current optimal value, else the infinity for the sense.
fn strongbranch_dualbound(lpi: &mut Lpi) -> ScipResult<f64> {
    let mut dualbound = if lpi.highs.sense() == 1.0 { -lpi_infinity(lpi) } else { lpi_infinity(lpi) };
    if lpi_is_optimal(lpi) {
        dualbound = lpi_get_objval(lpi)?;
    }
    Ok(dualbound)
}

/// `SCIPlpiStrongbranchFrac`: HiGHS's interface performs no strong
/// branching, returning the current dual bound for both children.
#[allow(clippy::too_many_arguments)]
pub fn lpi_strongbranch_frac(
    lpi: &mut Lpi,
    _col: i32,
    _psol: f64,
    _itlim: i32,
    down: Option<&mut f64>,
    up: Option<&mut f64>,
    downvalid: &mut bool,
    upvalid: &mut bool,
    iter: &mut i32,
) -> ScipResult {
    let dualbound = strongbranch_dualbound(lpi)?;
    if let Some(down) = down {
        *down = dualbound;
    }
    if let Some(up) = up {
        *up = dualbound;
    }
    *downvalid = true;
    *upvalid = true;
    *iter = -1;
    Ok(())
}

/// `SCIPlpiStrongbranchInt`.
#[allow(clippy::too_many_arguments)]
pub fn lpi_strongbranch_int(
    lpi: &mut Lpi,
    col: i32,
    psol: f64,
    itlim: i32,
    down: Option<&mut f64>,
    up: Option<&mut f64>,
    downvalid: &mut bool,
    upvalid: &mut bool,
    iter: &mut i32,
) -> ScipResult {
    lpi_strongbranch_frac(lpi, col, psol, itlim, down, up, downvalid, upvalid, iter)
}

/// `SCIPlpiStrongbranchesFrac`.
#[allow(clippy::too_many_arguments)]
pub fn lpi_strongbranches_frac(
    lpi: &mut Lpi,
    cols: &[i32],
    _psols: &[f64],
    _itlim: i32,
    down: &mut [f64],
    up: &mut [f64],
    downvalid: &mut [bool],
    upvalid: &mut [bool],
    iter: &mut i32,
) -> ScipResult {
    let dualbound = strongbranch_dualbound(lpi)?;
    for j in 0..cols.len() {
        down[j] = dualbound;
        up[j] = dualbound;
        downvalid[j] = true;
        upvalid[j] = true;
    }
    *iter = -1;
    Ok(())
}

/// `SCIPlpiStrongbranchesInt`.
#[allow(clippy::too_many_arguments)]
pub fn lpi_strongbranches_int(
    lpi: &mut Lpi,
    cols: &[i32],
    psols: &[f64],
    itlim: i32,
    down: &mut [f64],
    up: &mut [f64],
    downvalid: &mut [bool],
    upvalid: &mut [bool],
    iter: &mut i32,
) -> ScipResult {
    lpi_strongbranches_frac(lpi, cols, psols, itlim, down, up, downvalid, upvalid, iter)
}

/// `SCIPlpiWasSolved`.
pub fn lpi_was_solved(lpi: &Lpi) -> bool {
    lpi.solved
}

/// `SCIPlpiGetSolFeasibility`: (primal feasible, dual feasible).
pub fn lpi_get_sol_feasibility(lpi: &mut Lpi) -> ScipResult<(bool, bool)> {
    Ok((lpi_is_primal_feasible(lpi), lpi_is_dual_feasible(lpi)))
}

/// `SCIPlpiExistsPrimalRay`.
pub fn lpi_exists_primal_ray(lpi: &Lpi) -> bool {
    let model_status = lpi.highs.status();
    model_status == Status::Unbounded || model_status == Status::UnboundedOrInfeasible
}

/// `SCIPlpiHasPrimalRay`.
pub fn lpi_has_primal_ray(lpi: &Lpi) -> bool {
    if !lpi_is_primal_unbounded(lpi) {
        return false;
    }
    if lpi.highs.num_rows() == 0 {
        return true;
    }
    lpi.highs.has_primal_ray()
}

/// `SCIPlpiIsPrimalUnbounded`.
pub fn lpi_is_primal_unbounded(lpi: &Lpi) -> bool {
    lpi.highs.status() == Status::Unbounded
}

/// Whether an empty model's rows have bounds excluding zero.
fn empty_model_rows_infeasible(lpi: &Lpi) -> bool {
    let (lower, upper) = (lpi.highs.row_lower(), lpi.highs.row_upper());
    (0..lpi.highs.num_rows()).any(|i| lower[i] > 0.0 || upper[i] < 0.0)
}

/// `SCIPlpiIsPrimalInfeasible`.
pub fn lpi_is_primal_infeasible(lpi: &Lpi) -> bool {
    let model_status = lpi.highs.status();
    if model_status == Status::ModelEmpty {
        return empty_model_rows_infeasible(lpi);
    }
    model_status == Status::Infeasible || model_status == Status::UnboundedOrInfeasible
}

/// `SCIPlpiIsPrimalFeasible`.
pub fn lpi_is_primal_feasible(lpi: &Lpi) -> bool {
    let model_status = lpi.highs.status();
    if model_status == Status::ModelEmpty {
        return !empty_model_rows_infeasible(lpi);
    }
    model_status == Status::Optimal || model_status == Status::Unbounded
}

/// `SCIPlpiExistsDualRay`.
pub fn lpi_exists_dual_ray(lpi: &Lpi) -> bool {
    !lpi_is_primal_feasible(lpi)
}

/// `SCIPlpiHasDualRay`.
pub fn lpi_has_dual_ray(lpi: &Lpi) -> bool {
    if lpi.highs.status() == Status::ModelEmpty {
        return !lpi_is_primal_feasible(lpi);
    }
    lpi.highs.has_dual_ray()
}

/// `SCIPlpiIsDualUnbounded`.
pub fn lpi_is_dual_unbounded(lpi: &Lpi) -> bool {
    lpi_is_dual_feasible(lpi) && !lpi_is_primal_feasible(lpi)
}

/// `SCIPlpiIsDualInfeasible`.
pub fn lpi_is_dual_infeasible(lpi: &Lpi) -> bool {
    let model_status = lpi.highs.status();
    model_status == Status::Unbounded || model_status == Status::UnboundedOrInfeasible
}

/// `SCIPlpiIsDualFeasible`.
pub fn lpi_is_dual_feasible(lpi: &Lpi) -> bool {
    let model_status = lpi.highs.status();
    if model_status == Status::Optimal || model_status == Status::ModelEmpty {
        true
    } else if model_status == Status::Unbounded || model_status == Status::UnboundedOrInfeasible {
        false
    } else {
        // `num_dual_infeasibilities` (HiGHS's info, -1 when unknown).
        lpi.highs.infeasibilities().and_then(|info| info.num_dual) == Some(0)
    }
}

/// `SCIPlpiIsOptimal`.
pub fn lpi_is_optimal(lpi: &Lpi) -> bool {
    let model_status = lpi.highs.status();
    if model_status == Status::ModelEmpty {
        lpi_is_primal_feasible(lpi)
    } else {
        model_status == Status::Optimal
    }
}

/// `SCIPlpiIsStable`: if an objective limit is set and HiGHS claims that
/// it is exceeded, we check that this is indeed the case.
pub fn lpi_is_stable(lpi: &Lpi) -> bool {
    if lpi_is_objlim_exc(lpi) {
        let mut objlimit = lpi.highs.options.objective_bound;
        let mut objvalue = lpi.highs.objective();
        if lpi.highs.sense() == -1.0 {
            objlimit *= -1.0;
            objvalue *= -1.0;
        }
        if !lpi_is_infinity(lpi, objlimit) && highs_rel_diff(objvalue, objlimit) < -1e-9 {
            return false;
        }
    }
    true
}

/// `SCIPlpiIsObjlimExc`.
pub fn lpi_is_objlim_exc(lpi: &Lpi) -> bool {
    lpi.highs.status() == Status::ObjectiveBound
}

/// `SCIPlpiIsIterlimExc`.
pub fn lpi_is_iterlim_exc(lpi: &Lpi) -> bool {
    lpi.highs.status() == Status::IterationLimit
}

/// `SCIPlpiIsTimelimExc`.
pub fn lpi_is_timelim_exc(lpi: &Lpi) -> bool {
    lpi.highs.status() == Status::TimeLimit
}

/// `SCIPlpiGetInternalStatus`: HiGHS's model status number.
pub fn lpi_get_internal_status(lpi: &Lpi) -> i32 {
    lpi.highs.status().highs_number()
}

/// `SCIPlpiIgnoreInstability`.
pub fn lpi_ignore_instability(_lpi: &mut Lpi) -> ScipResult<bool> {
    Ok(true)
}

/// `SCIPlpiGetObjval`: HiGHS's `objective_function_value`.
pub fn lpi_get_objval(lpi: &mut Lpi) -> ScipResult<f64> {
    Ok(lpi.highs.objective())
}

/// `SCIPlpiGetSol`: gets primal and dual solution vectors.
pub fn lpi_get_sol(
    lpi: &mut Lpi,
    objval: Option<&mut f64>,
    primsol: Option<&mut [f64]>,
    dualsol: Option<&mut [f64]>,
    activity: Option<&mut [f64]>,
    redcost: Option<&mut [f64]>,
) -> ScipResult {
    if let Some(objval) = objval {
        *objval = lpi.highs.objective();
    }
    let (ncols, nrows) = (lpi.highs.num_cols(), lpi.highs.num_rows());
    let empty: &[f64] = &[];
    let col_value = lpi.highs.col_value().unwrap_or(empty);
    let col_dual = lpi.highs.col_dual().unwrap_or(empty);
    let row_value = lpi.highs.row_value().unwrap_or(empty);
    let row_dual = lpi.highs.row_dual().unwrap_or(empty);
    if col_value.len() != ncols || col_dual.len() != ncols || row_value.len() != nrows || row_dual.len() != nrows {
        log::warn!("HiGHS returned solution vector of inconsistent dimension");
        return Err(Retcode::LpError);
    }
    if let Some(primsol) = primsol {
        primsol[..ncols].copy_from_slice(col_value);
    }
    if let Some(dualsol) = dualsol {
        dualsol[..nrows].copy_from_slice(row_dual);
    }
    if let Some(activity) = activity {
        activity[..nrows].copy_from_slice(row_value);
    }
    if let Some(redcost) = redcost {
        redcost[..ncols].copy_from_slice(col_dual);
    }
    Ok(())
}

/// `SCIPlpiGetPrimalRay`: gets primal ray for unbounded LPs.
pub fn lpi_get_primal_ray(lpi: &mut Lpi, ray: &mut [f64]) -> ScipResult {
    let mut success = false;
    if lpi.highs.num_rows() == 0 {
        let maximize = lpi.highs.sense() == -1.0;
        for (i, ray_i) in ray[..lpi.highs.num_cols()].iter_mut().enumerate() {
            let mut minobj = lpi.highs.col_cost()[i];
            if maximize {
                minobj = -minobj;
            }
            if lpi_is_infinity(lpi, -lpi.highs.col_lower()[i]) && minobj > 0.0 {
                *ray_i = -1.0;
                success = true;
            } else if lpi_is_infinity(lpi, lpi.highs.col_upper()[i]) && minobj < 0.0 {
                *ray_i = 1.0;
                success = true;
            } else {
                *ray_i = 0.0;
            }
        }
    } else if let Some(primal_ray) = lpi.highs.get_primal_ray() {
        ray[..primal_ray.len()].copy_from_slice(&primal_ray);
        success = true;
    }
    if success { Ok(()) } else { Err(Retcode::LpError) }
}

/// `SCIPlpiGetDualfarkas`: gets dual Farkas proof for infeasibility.
pub fn lpi_get_dualfarkas(lpi: &mut Lpi, dualfarkas: &mut [f64]) -> ScipResult {
    if lpi.highs.status() == Status::ModelEmpty {
        let dualdir = if lpi.highs.sense() == 1.0 { 1.0 } else { -1.0 };
        let (lower, upper) = (lpi.highs.row_lower(), lpi.highs.row_upper());
        for i in 0..lpi.highs.num_rows() {
            dualfarkas[i] = if lower[i] > 0.0 {
                dualdir
            } else if upper[i] < 0.0 {
                -dualdir
            } else {
                0.0
            };
        }
        return Ok(());
    }
    match lpi.highs.get_dual_ray() {
        Some(dual_ray) => {
            dualfarkas[..dual_ray.len()].copy_from_slice(&dual_ray);
            Ok(())
        }
        None => Err(Retcode::LpError),
    }
}

/// `SCIPlpiGetIterations`: HiGHS's `simplex_iteration_count`.
pub fn lpi_get_iterations(lpi: &mut Lpi) -> ScipResult<i32> {
    Ok(lpi.highs.iterations() as i32)
}

/// `SCIPlpiGetBase`: gets current basis status for columns and rows.
pub fn lpi_get_base(lpi: &mut Lpi, cstat: Option<&mut [i32]>, rstat: Option<&mut [i32]>) -> ScipResult {
    let basis = lpi.highs.basis_any();
    if let Some(cstat) = cstat {
        for i in 0..lpi.highs.num_cols() {
            cstat[i] = highs_basis_status_to_int(basis.col_status[i]);
        }
    }
    if let Some(rstat) = rstat {
        for i in 0..lpi.highs.num_rows() {
            rstat[i] = highs_basis_status_to_int(basis.row_status[i]);
        }
    }
    Ok(())
}

/// `SCIPlpiSetBase`: sets current basis status for columns and rows.
pub fn lpi_set_base(lpi: &mut Lpi, cstat: Option<&[i32]>, rstat: Option<&[i32]>) -> ScipResult {
    let (ncols, nrows) = (lpi.highs.num_cols(), lpi.highs.num_rows());
    let mut basis = Basis {
        col_status: vec![BasisStatus::Lower; ncols],
        row_status: vec![BasisStatus::Lower; nrows],
    };
    if let Some(cstat) = cstat {
        for i in 0..ncols {
            basis.col_status[i] = basestat_to_highs_basis_status(cstat[i]);
        }
    }
    if let Some(rstat) = rstat {
        for i in 0..nrows {
            basis.row_status[i] = basestat_to_highs_basis_status(rstat[i]);
        }
    }
    if !lpi.highs.set_basis(&basis) {
        log::error!("Error in HiGHS function call");
        return Err(Retcode::LpError);
    }
    Ok(())
}

/// `SCIPlpiGetBasisInd`: returns the indices of the basic columns and
/// rows; basic column n gives value n, basic row m gives value -1-m.
pub fn lpi_get_basis_ind(lpi: &mut Lpi, bind: &mut [i32]) -> ScipResult {
    if lpi.highs.basis().is_none() {
        return Err(Retcode::Error);
    }
    match lpi.highs.basic_variables_signed() {
        Some(basic) => {
            bind[..basic.len()].copy_from_slice(&basic);
            Ok(())
        }
        None => Err(Retcode::LpError),
    }
}

/// Copies a dense or sparse vector from HiGHS into `coef`, `inds` and
/// `ninds` as the B^-1 getters return it.
fn store_vector(vector: (Vec<f64>, Vec<usize>), coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) {
    let (dense, index) = vector;
    coef[..dense.len()].copy_from_slice(&dense);
    if let (Some(inds), Some(ninds)) = (inds, ninds) {
        for (k, &i) in index.iter().enumerate() {
            inds[k] = i as i32;
        }
        *ninds = index.len() as i32;
    }
}

/// Runs a B^-1 getter, solving with the dual simplex first if HiGHS has no
/// invertible basis (as the interface retries).
fn binv_getter(lpi: &mut Lpi, get: impl Fn(&mut LpSolver) -> Option<(Vec<f64>, Vec<usize>)>) -> ScipResult<(Vec<f64>, Vec<usize>)> {
    if get(&mut lpi.highs).is_none() {
        lpi_solve_dual(lpi)?;
    }
    get(&mut lpi.highs).ok_or_else(|| {
        log::error!("Error in HiGHS function call");
        Retcode::LpError
    })
}

/// `SCIPlpiGetBInvRow`: gets a row from the inverse basis matrix B^-1.
pub fn lpi_get_binv_row(lpi: &mut Lpi, r: i32, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    let want_sparse = inds.is_some() && ninds.is_some();
    let vector = binv_getter(lpi, |highs| highs.basis_inverse_row(r as usize, want_sparse))?;
    store_vector(vector, coef, inds, ninds);
    Ok(())
}

/// `SCIPlpiGetBInvCol`: gets a column from the inverse basis matrix B^-1.
pub fn lpi_get_binv_col(lpi: &mut Lpi, c: i32, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    let want_sparse = inds.is_some() && ninds.is_some();
    let vector = binv_getter(lpi, |highs| highs.basis_inverse_col(c as usize, want_sparse))?;
    store_vector(vector, coef, inds, ninds);
    Ok(())
}

/// `SCIPlpiGetBInvARow`: gets a row from the product of B^-1 and the
/// coefficient matrix A.
pub fn lpi_get_binv_a_row(lpi: &mut Lpi, r: i32, binvrow: Option<&[f64]>, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    let want_sparse = inds.is_some() && ninds.is_some();
    let vector = binv_getter(lpi, |highs| highs.reduced_row(r as usize, binvrow, want_sparse))?;
    store_vector(vector, coef, inds, ninds);
    Ok(())
}

/// `SCIPlpiGetBInvACol`: gets a column from the product of B^-1 and the
/// coefficient matrix A.
pub fn lpi_get_binv_a_col(lpi: &mut Lpi, c: i32, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    let want_sparse = inds.is_some() && ninds.is_some();
    let vector = binv_getter(lpi, |highs| highs.reduced_column(c as usize, want_sparse))?;
    store_vector(vector, coef, inds, ninds);
    Ok(())
}

/// `SCIPlpiGetState`: stores the basis (the packing into two bits each is
/// unobservable and left out).
pub fn lpi_get_state(lpi: &mut Lpi) -> ScipResult<LpiState> {
    let (ncols, nrows) = (lpi.highs.num_cols(), lpi.highs.num_rows());
    let mut cstat = vec![0; ncols];
    let mut rstat = vec![0; nrows];
    lpi_get_base(lpi, Some(&mut cstat), Some(&mut rstat))?;

    // The C packs two bits per status: kNonbasic (4) would lose its high
    // bit, as here.
    for s in cstat.iter_mut().chain(rstat.iter_mut()) {
        *s &= 3;
    }
    Ok(LpiState {
        ncols: ncols as i32,
        nrows: nrows as i32,
        cstat,
        rstat,
    })
}

/// `SCIPlpiSetState`: loads a stored basis, completing it for columns and
/// rows added since.
pub fn lpi_set_state(lpi: &mut Lpi, lpistate: &LpiState) -> ScipResult {
    let (lpncols, lpnrows) = (lpi.highs.num_cols(), lpi.highs.num_rows());
    let mut cstat = vec![0; lpncols.max(lpistate.ncols as usize)];
    let mut rstat = vec![0; lpnrows.max(lpistate.nrows as usize)];
    cstat[..lpistate.ncols as usize].copy_from_slice(&lpistate.cstat);
    rstat[..lpistate.nrows as usize].copy_from_slice(&lpistate.rstat);

    // Extend the basis to the current LP beyond the previously existing
    // columns.
    for (i, cstat_i) in (lpistate.ncols as usize..lpncols).zip(&mut cstat[lpistate.ncols as usize..lpncols]) {
        *cstat_i = if !lpi_is_infinity(lpi, -lpi.highs.col_lower()[i]) {
            BASESTAT_LOWER as i32
        } else if !lpi_is_infinity(lpi, lpi.highs.col_upper()[i]) {
            BASESTAT_UPPER as i32
        } else {
            BASESTAT_ZERO as i32
        };
    }
    for rstat_i in &mut rstat[lpistate.nrows as usize..lpnrows] {
        *rstat_i = BASESTAT_BASIC as i32;
    }

    // Load basis information.
    lpi_set_base(lpi, Some(&cstat), Some(&rstat))
}

/// `SCIPlpiClearState`.
pub fn lpi_clear_state(lpi: &mut Lpi) -> ScipResult {
    lpi.highs.clear_solver();
    Ok(())
}

/// `SCIPlpiHasStateBasis`.
pub fn lpi_has_state_basis(_lpi: &Lpi, _lpistate: &LpiState) -> bool {
    true
}

/// `SCIPlpiGetNorms`: the HiGHS interface stores no norms.
pub fn lpi_get_norms(_lpi: &mut Lpi) -> ScipResult<Option<LpiNorms>> {
    Ok(None)
}

/// `SCIPlpiSetNorms`.
pub fn lpi_set_norms(_lpi: &mut Lpi, _lpinorms: &LpiNorms) -> ScipResult {
    Ok(())
}

/// `SCIPlpiGetIntpar`: gets integer parameter of LP.
pub fn lpi_get_intpar(lpi: &Lpi, param: LpParam) -> ScipResult<i32> {
    let o = &lpi.highs.options;
    Ok(match param {
        LpParam::FromScratch => lpi.fromscratch as i32,
        LpParam::LpInfo => o.output_flag as i32,
        LpParam::Scaling => {
            let ival = o.simplex_scale_strategy;
            if ival <= 0 {
                0
            } else if ival <= 2 {
                1
            } else {
                2
            }
        }
        LpParam::Presolving => lpi.presolve as i32,
        LpParam::Pricing => lpi.pricing as i32,
        LpParam::Threads => lpi.nthreads,
        LpParam::LpItLim => o.simplex_iteration_limit.min(i32::MAX as usize) as i32,
        LpParam::RandomSeed => o.random_seed as i32,
        _ => return Err(Retcode::ParameterUnknown),
    })
}

/// `SCIPlpiSetIntpar`: sets integer parameter of LP.
pub fn lpi_set_intpar(lpi: &mut Lpi, param: LpParam, ival: i32) -> ScipResult {
    match param {
        LpParam::FromScratch => lpi.fromscratch = ival != 0,
        LpParam::LpInfo => lpi.highs.options.output_flag = ival != 0,
        LpParam::Scaling => {
            lpi.highs.options.simplex_scale_strategy = match ival {
                0 => 0,
                1 => 2,
                _ => 4,
            }
        }
        LpParam::Presolving => {
            lpi.presolve = ival != 0;
            lpi.highs.options.presolve = ival != 0;
        }
        LpParam::Pricing => {
            lpi.pricing = match ival {
                0 => Pricing::LpiDefault,
                1 => Pricing::Auto,
                2 => Pricing::Full,
                3 => Pricing::Partial,
                4 => Pricing::Steep,
                5 => Pricing::SteepQStart,
                6 => Pricing::Devex,
                _ => return Err(Retcode::LpError),
            };
            let strategy = match lpi.pricing {
                Pricing::LpiDefault | Pricing::Partial | Pricing::Auto => EdgeWeightStrategy::Choose,
                Pricing::Devex => EdgeWeightStrategy::Devex,
                Pricing::Full | Pricing::Steep | Pricing::SteepQStart => EdgeWeightStrategy::SteepestEdge,
            };
            lpi.highs.options.primal_edge_weight_strategy = strategy;
            lpi.highs.options.dual_edge_weight_strategy = strategy;
        }
        LpParam::Threads => lpi.nthreads = ival,
        // HiGHS rejects a negative limit, keeping the old one.
        LpParam::LpItLim if ival < 0 => return Err(Retcode::LpError),
        LpParam::LpItLim => lpi.highs.options.simplex_iteration_limit = ival as usize,
        LpParam::RandomSeed if ival < 0 => return Err(Retcode::LpError),
        LpParam::RandomSeed => lpi.highs.options.random_seed = ival as u32,
        _ => return Err(Retcode::ParameterUnknown),
    }
    Ok(())
}

/// `SCIPlpiGetRealpar`: gets floating point parameter of LP.
pub fn lpi_get_realpar(lpi: &Lpi, param: LpParam) -> ScipResult<f64> {
    let o = &lpi.highs.options;
    Ok(match param {
        LpParam::Feastol => o.primal_feasibility_tolerance,
        LpParam::DualFeastol => o.dual_feasibility_tolerance,
        LpParam::LpTiLim => o.time_limit,
        LpParam::ObjLim => o.objective_bound,
        _ => return Err(Retcode::ParameterUnknown),
    })
}

/// `SCIPlpiSetRealpar`: sets floating point parameter of LP.
pub fn lpi_set_realpar(lpi: &mut Lpi, param: LpParam, mut dval: f64) -> ScipResult {
    let o = &mut lpi.highs.options;
    match param {
        LpParam::Feastol => {
            dval = crate::def::max(dval, 1e-10);
            o.primal_feasibility_tolerance = dval;
        }
        LpParam::DualFeastol => {
            dval = crate::def::max(dval, 1e-10);
            o.dual_feasibility_tolerance = dval;
        }
        LpParam::LpTiLim => o.time_limit = dval,
        LpParam::ObjLim => o.objective_bound = dval,
        _ => return Err(Retcode::ParameterUnknown),
    }
    Ok(())
}

/// `SCIPlpiInterrupt`.
pub fn lpi_interrupt(_lpi: &mut Lpi, _interrupt: bool) -> ScipResult {
    Ok(())
}

/// `SCIPlpiInfinity`: HiGHS's `kHighsInf`.
pub fn lpi_infinity(_lpi: &Lpi) -> f64 {
    f64::INFINITY
}

/// `SCIPlpiIsInfinity`.
pub fn lpi_is_infinity(_lpi: &Lpi, val: f64) -> bool {
    val >= f64::INFINITY
}
