//! Port of `expr_value.c`: constant value expression handler.

use std::any::Any;

use crate::{
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, ExprOwnerCreate, ExprhdlrId, Monotone},
    interval::Interval,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ImplintType, SolId},
};

const EXPRHDLR_NAME: &str = "val";
const EXPRHDLR_DESC: &str = "constant value";
const EXPRHDLR_PRECEDENCE: u32 = 10000;

/// `EXPRHDLR_HASHKEY`.
fn exprhdlr_hashkey() -> u32 {
    crate::misc::calc_fib_hash(36787.0)
}

/// `SCIP_ExprData`: expression data.
struct ExprData {
    /// The constant value.
    value: f64,
}

/// `compareValue`.
fn compare_value(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let val1 = get_value_expr_value(scip, expr1);
    let val2 = get_value_expr_value(scip, expr2);

    if val1 < val2 {
        -1
    } else if val1 == val2 {
        0
    } else {
        1
    }
}

/// `copyhdlrValue`.
fn copyhdlr_value(scip: &mut Scip, _source: &Scip, _exprhdlr: ExprhdlrId) -> ScipResult {
    include_exprhdlr_value(scip)
}

/// `copydataValue`.
fn copydata_value(scip: &mut Scip, sourceexpr: ExprId) -> ScipResult<Box<dyn Any>> {
    Ok(Box::new(ExprData {
        value: get_value_expr_value(scip, sourceexpr),
    }))
}

/// `freedataValue`.
fn freedata_value(scip: &mut Scip, expr: ExprId) -> ScipResult {
    scip.exprs[expr].exprdata = None;
    Ok(())
}

/// `evalValue`.
fn eval_value(scip: &mut Scip, expr: ExprId, val: &mut f64, _sol: SolId) -> ScipResult {
    *val = get_value_expr_value(scip, expr);
    Ok(())
}

/// `bwdiffValue`: should never be called since value expressions do not
/// have children.
fn bwdiff_value(_scip: &mut Scip, _expr: ExprId, _childidx: i32, _val: &mut f64) -> ScipResult {
    Err(Retcode::InvalidCall)
}

/// `fwdiffValue`.
fn fwdiff_value(_scip: &mut Scip, _expr: ExprId, dot: &mut f64, _direction: SolId) -> ScipResult {
    *dot = 0.0;
    Ok(())
}

/// `bwfwdiffValue`: should never be called since value expressions do not
/// have children.
fn bwfwdiff_value(_scip: &mut Scip, _expr: ExprId, _childidx: i32, _bardot: &mut f64, _direction: SolId) -> ScipResult {
    Err(Retcode::InvalidCall)
}

/// `intevalValue`.
fn inteval_value(scip: &mut Scip, expr: ExprId, interval: &mut Interval, _intevalvar: Option<ExprIntEvalVar>, _intevalvardata: usize) -> ScipResult {
    interval.set(get_value_expr_value(scip, expr));
    Ok(())
}

/// `hashValue`.
fn hash_value(scip: &mut Scip, expr: ExprId, hashkey: &mut u32, _childrenhashes: &[u32]) -> ScipResult {
    *hashkey = exprhdlr_hashkey();
    *hashkey ^= crate::misc::calc_fib_hash(get_value_expr_value(scip, expr));
    Ok(())
}

/// `curvatureValue`.
fn curvature_value(_scip: &mut Scip, _expr: ExprId, _exprcurvature: ExprCurv, success: &mut bool, _childcurv: &mut [ExprCurv]) -> ScipResult {
    *success = true;
    Ok(())
}

/// `monotonicityValue`.
fn monotonicity_value(_scip: &mut Scip, _expr: ExprId, _childidx: i32, result: &mut Monotone) -> ScipResult {
    *result = Monotone::Const;
    Ok(())
}

/// `integralityValue`.
fn integrality_value(scip: &mut Scip, expr: ExprId, integrality: &mut ImplintType) -> ScipResult {
    *integrality = if crate::def::eps_is_int(get_value_expr_value(scip, expr), 0.0) {
        ImplintType::Strong
    } else {
        ImplintType::None
    };
    Ok(())
}

/// `SCIPincludeExprhdlrValue`: creates the handler for constant value
/// expression and includes it into SCIP.
pub fn include_exprhdlr_value(scip: &mut Scip) -> ScipResult {
    let exprhdlr = crate::scip_expr::include_exprhdlr(scip, EXPRHDLR_NAME, Some(EXPRHDLR_DESC), EXPRHDLR_PRECEDENCE, eval_value, None)?;

    let h = &mut scip.exprhdlrs[exprhdlr];
    h.copyhdlr = Some(copyhdlr_value);
    h.copydata = Some(copydata_value);
    h.freedata = Some(freedata_value);
    h.compare = Some(compare_value);
    h.print = true;
    h.inteval = Some(inteval_value);
    h.hash = Some(hash_value);
    h.bwdiff = Some(bwdiff_value);
    h.fwdiff = Some(fwdiff_value);
    h.bwfwdiff = Some(bwfwdiff_value);
    h.curvature = Some(curvature_value);
    h.monotonicity = Some(monotonicity_value);
    h.integrality = Some(integrality_value);

    Ok(())
}

/// `SCIPcreateExprValue`: creates constant value expression.
pub fn create_expr_value(scip: &mut Scip, value: f64, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    let h = scip.set.exprhdlrval.expect("value expression handler");
    expr::expr_create(scip, h, Some(Box::new(ExprData { value })), &[], ownercreate, ownercreatedata)
}

/// `SCIPgetValueExprValue`: gets the value of a constant value expression.
#[inline]
pub fn get_value_expr_value(scip: &Scip, expr: ExprId) -> f64 {
    scip.exprs[expr].exprdata.as_ref().unwrap().downcast_ref::<ExprData>().unwrap().value
}
