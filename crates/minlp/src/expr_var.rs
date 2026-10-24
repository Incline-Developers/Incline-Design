//! Port of `expr_var.c`: variable expression handler.

use std::any::Any;

use crate::{
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, ExprOwnerCreate, ExprhdlrId, INTERVAL_INFINITY, Monotone},
    interval::Interval,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ImplintType, SolId, VarId},
};

const EXPRHDLR_NAME: &str = "var";
const EXPRHDLR_DESC: &str = "SCIP variable expression";
const EXPRHDLR_PRECEDENCE: u32 = 0;

/// `EXPRHDLR_HASHKEY`.
fn exprhdlr_hashkey() -> u32 {
    crate::misc::calc_fib_hash(22153.0)
}

/// `infty2infty`.
#[inline]
fn infty2infty(infty1: f64, infty2: f64, val: f64) -> f64 {
    if val >= infty1 { infty2 } else { val }
}

/// `simplifyVar`: simplifies a variable expression.
///
/// We replace the variable when fixed by its value. If a variable is fixed,
/// (multi)aggregated or more generally, inactive, we replace it with its
/// active counterpart.
///
/// Implementation note: SCIP_STAGE_PRESOLVED is the stage right after
/// exitPresolve is called, therefore, once we are in this stage, or any
/// subsequent stage, then all vars must be active.
fn simplify_var(scip: &mut Scip, expr: ExprId, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    let var = get_var_expr_var(scip, expr);

    // If var is active then there is nothing to simplify.
    if crate::var::var_is_active(scip, var) {
        *simplifiedexpr = expr;
        // We have to capture it, since it must simulate a "normal" simplified
        // call in which a new expression is created.
        expr::expr_capture(scip, *simplifiedexpr);
        return Ok(());
    }

    // var is not active; obtain active representation var = constant + sum
    // coefs_i vars_i.
    let varssize = 5;
    let mut vars = crate::cvec::CVec(vec![crate::var::NONE; varssize]);
    let mut coefs = crate::cvec::CVec(vec![0.0; varssize]);

    vars[0] = var;
    coefs[0] = 1.0;
    let mut constant = 0.0;
    let mut nvars = 1;
    if !crate::var::var_is_original(scip, var) {
        let mut requsize = 0;
        crate::var::var_get_active_representatives(scip, &mut vars, &mut coefs, &mut nvars, varssize as i32, &mut constant, &mut requsize)?;

        if requsize > varssize as i32 {
            vars.0.resize(requsize as usize, crate::var::NONE);
            coefs.0.resize(requsize as usize, 0.0);
            crate::var::var_get_active_representatives(scip, &mut vars, &mut coefs, &mut nvars, requsize, &mut constant, &mut requsize)?;
        }
    }

    // Create expression for constant + sum coefs_i vars_i.
    let mut sumexpr = crate::expr_sum::create_expr_sum(scip, &[], None, constant, ownercreate, ownercreatedata)?;

    for i in 0..nvars as usize {
        let mut child = create_expr_var(scip, vars[i], ownercreate, ownercreatedata)?;
        crate::expr_sum::append_expr_sum_expr(scip, sumexpr, child, coefs[i]);
        expr::expr_release(scip, &mut child)?;
    }

    // Simplify since it might not really be a sum.
    crate::scip_expr::call_expr_simplify(scip, sumexpr, simplifiedexpr, ownercreate, ownercreatedata)?;

    // We cannot handle fixings to infinity at the moment (TODO we should).

    // Release no longer used sumexpr.
    expr::expr_release(scip, &mut sumexpr)?;

    Ok(())
}

/// `compareVar`: the order of two variable expressions is that of their
/// variables' indices.
fn compare_var(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let index1 = scip.vars[get_var_expr_var(scip, expr1)].index;
    let index2 = scip.vars[get_var_expr_var(scip, expr2)].index;

    if index1 < index2 {
        -1
    } else if index1 == index2 {
        0
    } else {
        1
    }
}

/// `copyhdlrVar`.
fn copyhdlr_var(scip: &mut Scip, _source: &Scip, _exprhdlr: ExprhdlrId) -> ScipResult {
    include_exprhdlr_var(scip)
}

/// `copydataVar`: copying into a different SCIP should be handled on the
/// `SCIPexprCopy()` level (via mapexpr).
fn copydata_var(scip: &mut Scip, sourceexpr: ExprId) -> ScipResult<Box<dyn Any>> {
    let var = get_var_expr_var(scip, sourceexpr);
    crate::var::var_capture(scip, var);
    Ok(Box::new(var))
}

/// `freedataVar`.
fn freedata_var(scip: &mut Scip, expr: ExprId) -> ScipResult {
    let mut var = get_var_expr_var(scip, expr);
    crate::api::release_var(scip, &mut var)?;
    scip.exprs[expr].exprdata = None;
    Ok(())
}

/// `evalVar`.
fn eval_var(scip: &mut Scip, expr: ExprId, val: &mut f64, sol: SolId) -> ScipResult {
    *val = crate::api::get_sol_val(scip, sol, get_var_expr_var(scip, expr));
    Ok(())
}

/// `bwdiffVar`: this should never happen because variable expressions do
/// not have children.
fn bwdiff_var(_scip: &mut Scip, _expr: ExprId, _childidx: i32, _val: &mut f64) -> ScipResult {
    Err(Retcode::InvalidCall)
}

/// `fwdiffVar`.
fn fwdiff_var(scip: &mut Scip, expr: ExprId, dot: &mut f64, direction: SolId) -> ScipResult {
    *dot = crate::api::get_sol_val(scip, direction, get_var_expr_var(scip, expr));
    Ok(())
}

/// `bwfwdiffVar`: this should never happen because variable expressions do
/// not have children.
fn bwfwdiff_var(_scip: &mut Scip, _expr: ExprId, _childidx: i32, _bardot: &mut f64, _direction: SolId) -> ScipResult {
    Err(Retcode::InvalidCall)
}

/// `intevalVar`: expression interval evaluation callback.
fn inteval_var(scip: &mut Scip, expr: ExprId, interval: &mut Interval, intevalvar: Option<ExprIntEvalVar>, intevalvardata: usize) -> ScipResult {
    let var = get_var_expr_var(scip, expr);

    if let Some(intevalvar) = intevalvar {
        *interval = intevalvar(scip, var, intevalvardata);
    } else {
        let lb = scip.vars[var].locdom.lb;
        let ub = scip.vars[var].locdom.ub;

        let inf = scip.set.infinity();
        interval.set_bounds(-infty2infty(inf, INTERVAL_INFINITY, -lb), infty2infty(inf, INTERVAL_INFINITY, ub));
    }

    Ok(())
}

/// `hashVar`.
fn hash_var(scip: &mut Scip, expr: ExprId, hashkey: &mut u32, _childrenhashes: &[u32]) -> ScipResult {
    let var = get_var_expr_var(scip, expr);

    *hashkey = exprhdlr_hashkey();
    *hashkey ^= crate::misc::calc_fib_hash(scip.vars[var].index as f64);

    Ok(())
}

/// `curvatureVar`: x -> x is linear, convex, and concave.
fn curvature_var(_scip: &mut Scip, _expr: ExprId, _exprcurvature: ExprCurv, success: &mut bool, _childcurv: &mut [ExprCurv]) -> ScipResult {
    *success = true;
    Ok(())
}

/// `monotonicityVar`.
fn monotonicity_var(_scip: &mut Scip, _expr: ExprId, _childidx: i32, result: &mut Monotone) -> ScipResult {
    *result = Monotone::Inc;
    Ok(())
}

/// `integralityVar`.
fn integrality_var(scip: &mut Scip, expr: ExprId, integrality: &mut ImplintType) -> ScipResult {
    let var = get_var_expr_var(scip, expr);

    *integrality = if !crate::var::var_is_integral(scip, var) {
        ImplintType::None
    } else if scip.vars[var].varimpltype == ImplintType::Weak {
        ImplintType::Weak
    } else {
        ImplintType::Strong
    };

    Ok(())
}

/// `SCIPincludeExprhdlrVar`: creates the handler for variable expression
/// and includes it into SCIP.
pub fn include_exprhdlr_var(scip: &mut Scip) -> ScipResult {
    let exprhdlr = crate::scip_expr::include_exprhdlr(scip, EXPRHDLR_NAME, Some(EXPRHDLR_DESC), EXPRHDLR_PRECEDENCE, eval_var, None)?;

    let h = &mut scip.exprhdlrs[exprhdlr];
    h.copyhdlr = Some(copyhdlr_var);
    h.copydata = Some(copydata_var);
    h.freedata = Some(freedata_var);
    h.simplify = Some(simplify_var);
    h.compare = Some(compare_var);
    h.print = true;
    h.inteval = Some(inteval_var);
    h.hash = Some(hash_var);
    h.bwdiff = Some(bwdiff_var);
    h.fwdiff = Some(fwdiff_var);
    h.bwfwdiff = Some(bwfwdiff_var);
    h.curvature = Some(curvature_var);
    h.monotonicity = Some(monotonicity_var);
    h.integrality = Some(integrality_var);

    Ok(())
}

/// `SCIPcreateExprVar`: creates a variable expression.
pub fn create_expr_var(scip: &mut Scip, var: VarId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    // Capture the variable so that it doesn't disappear while the expr still
    // points to it.
    crate::var::var_capture(scip, var);

    let h = scip.set.exprhdlrvar.expect("var expression handler");
    expr::expr_create(scip, h, Some(Box::new(var)), &[], ownercreate, ownercreatedata)
}

/// `SCIPgetVarExprVar`: gets the variable of a variable expression.
#[inline]
pub fn get_var_expr_var(scip: &Scip, expr: ExprId) -> VarId {
    *scip.exprs[expr].exprdata.as_ref().unwrap().downcast_ref::<VarId>().unwrap()
}
