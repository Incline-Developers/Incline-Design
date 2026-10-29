//! Port of `expr_sum.c`: sum expression handler.

use std::any::Any;

use crate::{
    def::eps_is_int,
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, ExprOwnerCreate, ExprhdlrId, INTERVAL_INFINITY, Monotone},
    exprcurv::exprcurv_multiply,
    interval::{self, Interval},
    retcode::ScipResult,
    scip::Scip,
    scip_expr,
    types::{ImplintType, SolId},
};

const EXPRHDLR_NAME: &str = "sum";
const EXPRHDLR_DESC: &str = "summation with coefficients and a constant";
const EXPRHDLR_PRECEDENCE: u32 = 40000;

/// `EXPRHDLR_HASHKEY`.
fn exprhdlr_hashkey() -> u32 {
    crate::misc::calc_fib_hash(47161.0)
}

/// `SCIP_ExprData`: expression data.
struct ExprData {
    /// Constant coefficient.
    constant: f64,
    /// Coefficients of children (its length is `coefssize`).
    coefficients: Vec<f64>,
}

fn data(scip: &Scip, expr: ExprId) -> &ExprData {
    scip.exprs[expr].exprdata.as_ref().unwrap().downcast_ref::<ExprData>().unwrap()
}

fn data_mut(scip: &mut Scip, expr: ExprId) -> &mut ExprData {
    scip.exprs[expr].exprdata.as_mut().unwrap().downcast_mut::<ExprData>().unwrap()
}

/// `createData`: creates expression data.
fn create_data(ncoefficients: usize, coefficients: Option<&[f64]>, constant: f64) -> ExprData {
    let coefficients = match coefficients {
        Some(c) => c[..ncoefficients].to_vec(),
        None => vec![1.0; ncoefficients],
    };

    ExprData { constant, coefficients }
}

/// `simplifyTerm`: simplifies the `idx`-th child of the sum expression
/// `duplicate` in order for it to be able to be a child of a simplified sum.
///
/// For example, this means that the `idx`-th child cannot be itself a sum;
/// if it is, we have to flatten it, i.e., take all its children and make
/// them children of `duplicate`.
fn simplify_term(scip: &mut Scip, duplicate: ExprId, idx: usize, changed: &mut bool, _ownercreate: Option<ExprOwnerCreate>, _ownercreatedata: usize) -> ScipResult {
    let mut constant = get_constant_expr_sum(scip, duplicate);

    let coef = get_coefs_expr_sum(scip, duplicate)[idx];
    let expr = scip.exprs[duplicate].children[idx];

    // Enforces SS3.
    if expr::expr_is_value(scip, expr) {
        *changed = true;
        constant += coef * crate::expr_value::get_value_expr_value(scip, expr);
        set_constant_expr_sum(scip, duplicate, constant);

        // TODO: remove child?
        data_mut(scip, duplicate).coefficients[idx] = 0.0;

        return Ok(());
    }

    // Enforces SS2.
    if expr::expr_is_sum(scip, expr) {
        *changed = true;

        // Pass constant to parent.
        constant += coef * get_constant_expr_sum(scip, expr);
        set_constant_expr_sum(scip, duplicate, constant);

        // Append all children of expr on parent except the first one.
        let n = scip.exprs[expr].children.len();
        if n > 1 {
            for i in 1..n {
                let child = scip.exprs[expr].children[i];
                let c = coef * get_coefs_expr_sum(scip, expr)[i];
                append_expr_sum_expr(scip, duplicate, child, c);
            }
        }

        // Replace expr with first child; need to get data again since it might be
        // re-allocated.
        let c0 = coef * get_coefs_expr_sum(scip, expr)[0];
        data_mut(scip, duplicate).coefficients[idx] = c0;
        let child0 = scip.exprs[expr].children[0];
        expr::expr_replace_child(scip, duplicate, idx, child0)?;

        return Ok(());
    }

    // SS9 and SS10 move coefficients into exponentials; the exponential
    // expression handler is not ported, so no child can be one.

    // Other types of (simplified) expressions can be a child of a simplified
    // sum.

    Ok(())
}

/// `simplifySum`: simplifies a sum expression.
///
/// Goes through each child and simplifies it; then sorts the simplified
/// children; then sum the children that are equal; finally creates a sum
/// expression with all the children that do not have a 0 coefficient and
/// post-process so that SS6 and SS7 are satisfied.
fn simplify_sum(scip: &mut Scip, expr: ExprId, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    let mut changed = false;

    // TODO: maybe have a flag to know if it is simplified ?
    // TODO: can we do this with a shallow duplicate + copy of children
    // pointer? currently simplifyTerm may modify children, so one would need
    // to be careful.
    let mut duplicate = scip_expr::duplicate_expr(scip, expr, None, 0, ownercreate, ownercreatedata)?;

    let nchildren = scip.exprs[duplicate].children.len();
    for i in 0..nchildren {
        // Enforces SS8 TODO: remove child?
        // We have to ask for the coefs everytime, since it might get realloced in
        // simpifyTerm.
        if get_coefs_expr_sum(scip, duplicate)[i] == 0.0 {
            changed = true;
            continue;
        }

        // Enforces SS2, SS3, SS9, and SS10.
        simplify_term(scip, duplicate, i, &mut changed, ownercreate, ownercreatedata)?;
    }

    // simplifyTerm can add new children to duplicate and realloc them; so get
    // them again.
    let children = scip.exprs[duplicate].children.clone();
    let nchildren = children.len();
    let coefs = get_coefs_expr_sum(scip, duplicate)[..nchildren].to_vec();

    'cleanup: {
        // Treat zero term case.
        if nchildren == 0 {
            let c = get_constant_expr_sum(scip, duplicate);
            *simplifiedexpr = crate::expr_value::create_expr_value(scip, c, ownercreate, ownercreatedata)?;
            break 'cleanup;
        }

        // Treat one term case.
        if nchildren == 1 {
            if coefs[0] == 0.0 {
                let c = get_constant_expr_sum(scip, duplicate);
                *simplifiedexpr = crate::expr_value::create_expr_value(scip, c, ownercreate, ownercreatedata)?;
                break 'cleanup;
            }

            if coefs[0] == 1.0 && get_constant_expr_sum(scip, duplicate) == 0.0 {
                *simplifiedexpr = children[0]; // SS7
            } else {
                *simplifiedexpr = if changed { duplicate } else { expr };
            }

            expr::expr_capture(scip, *simplifiedexpr);

            break 'cleanup;
        }

        // Enforces SS5: sort children.
        let mut order: Vec<usize> = (0..nchildren).collect();
        {
            let s: &Scip = scip;
            crate::sort::sort(&mut order, (), |&ind1, &ind2| expr::expr_compare(s, children[ind1], children[ind2]));
        }

        // Create sorted variant of children and coefs.
        let mut newchildren = vec![0; nchildren];
        let mut newcoefs = vec![0.0; nchildren];
        for i in 0..nchildren {
            newchildren[i] = children[order[i]];
            newcoefs[i] = coefs[order[i]];
            if order[i] != i {
                changed = true;
            }
        }

        // Post-process.

        // Enforces SS4.
        let mut nnewchildren = 0;
        for i in 0..nchildren {
            // Eliminate zero-coefficients.
            if newcoefs[i] == 0.0 {
                changed = true;
                continue;
            }

            // Sum equal expressions.
            if i < nchildren - 1 && expr::expr_compare(scip, newchildren[i], newchildren[i + 1]) == 0 {
                changed = true;
                // If we substract two almost equal not-so-small numbers, then set new
                // coefficient to 0.0 instead of some tiny value that is likely the
                // result of some random round-off error. E.g., on instance ex1221, we
                // have x1^2 + b3 = 1.25. Probing finds an aggregation x1 = 1.11803 -
                // 0.618034 b3. Simplify would then produce 1.25 + 1e-16 x1 = 1.25.
                if scip.set.is_eq(newcoefs[i], -newcoefs[i + 1]) && newcoefs[i].abs() >= 1.0 {
                    newcoefs[i + 1] = 0.0;
                } else {
                    newcoefs[i + 1] += newcoefs[i];
                }
                continue;
            }

            // Move i-th child to new position.
            newchildren[nnewchildren] = newchildren[i];
            newcoefs[nnewchildren] = newcoefs[i];
            nnewchildren += 1;
        }

        // Build sum expression from finalchildren and post-simplify.
        let newconstant = get_constant_expr_sum(scip, duplicate);

        // Enforces SS6: if they are no children, return value.
        if nnewchildren == 0 {
            *simplifiedexpr = crate::expr_value::create_expr_value(scip, newconstant, ownercreate, ownercreatedata)?;

            break 'cleanup;
        }

        // Enforces SS7: if list consists of one expr with coef 1.0 and constant
        // is 0, return that expr.
        if nnewchildren == 1 && newcoefs[0] == 1.0 && newconstant == 0.0 {
            *simplifiedexpr = newchildren[0];
            expr::expr_capture(scip, *simplifiedexpr);

            break 'cleanup;
        }

        // Build sum expression from children.
        if changed {
            *simplifiedexpr = create_expr_sum(
                scip,
                &newchildren[..nnewchildren],
                Some(&newcoefs[..nnewchildren]),
                newconstant,
                ownercreate,
                ownercreatedata,
            )?;

            break 'cleanup;
        }

        *simplifiedexpr = expr;

        // We have to capture it, since it must simulate a "normal" simplified
        // call in which a new expression is created.
        expr::expr_capture(scip, *simplifiedexpr);
    }

    expr::expr_release(scip, &mut duplicate)?;

    Ok(())
}

/// `compareSum`: compares two sum expressions.
///
/// The order of two sum expressions is a lexicographical order on the
/// terms.
///
/// Starting from the *last*, we find the first child where they differ,
/// say, the i-th. Then u < v <=> u_i < v_i. If there are no such children
/// and they have different number of children, then u < v <=>
/// nchildren(u) < nchildren(v). If there are no such children and they have
/// the same number of children, then u < v <=> const(u) < const(v).
/// Otherwise, they are the same.
///
/// Note: we are assuming expression are simplified, so within u, we have
/// u_1 < u_2, etc.
///
/// Example: y + z < x + y + z, 2*x + 3*y < 3*x + 3*y.
fn compare_sum(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let children1 = &scip.exprs[expr1].children;
    let children2 = &scip.exprs[expr2].children;
    let coefs1 = &data(scip, expr1).coefficients;
    let coefs2 = &data(scip, expr2).coefficients;
    let const1 = get_constant_expr_sum(scip, expr1);
    let const2 = get_constant_expr_sum(scip, expr2);

    let mut i = children1.len() as i32 - 1;
    let mut j = children2.len() as i32 - 1;
    while i >= 0 && j >= 0 {
        let compareresult = expr::expr_compare(scip, children1[i as usize], children2[j as usize]);
        if compareresult != 0 {
            return compareresult;
        } else {
            // Expressions are equal, compare coefficient.
            if coefs1[i as usize] < coefs2[j as usize] {
                return -1;
            }
            if coefs1[i as usize] > coefs2[j as usize] {
                return 1;
            }

            // Coefficients are equal, continue.
        }
        i -= 1;
        j -= 1;
    }

    // All children of one expression are children of the other expression,
    // use number of children as a tie-breaker.
    if i < j {
        // expr1 has less elements, hence expr1 < expr2.
        return -1;
    }
    if i > j {
        // expr1 has more elements, hence expr1 > expr2.
        return 1;
    }

    // Everything is equal, use constant/coefficient as tie-breaker.
    if const1 < const2 {
        return -1;
    }
    if const1 > const2 {
        return 1;
    }

    // They are equal.
    0
}

/// `copyhdlrSum`.
fn copyhdlr_sum(scip: &mut Scip, _source: &Scip, _exprhdlr: ExprhdlrId) -> ScipResult {
    include_exprhdlr_sum(scip)
}

/// `copydataSum`.
fn copydata_sum(scip: &mut Scip, sourceexpr: ExprId) -> ScipResult<Box<dyn Any>> {
    let n = scip.exprs[sourceexpr].children.len();
    let d = data(scip, sourceexpr);
    Ok(Box::new(create_data(n, Some(&d.coefficients), d.constant)))
}

/// `freedataSum`.
fn freedata_sum(scip: &mut Scip, expr: ExprId) -> ScipResult {
    scip.exprs[expr].exprdata = None;
    Ok(())
}

/// `evalSum`.
fn eval_sum(scip: &mut Scip, expr: ExprId, val: &mut f64, _sol: SolId) -> ScipResult {
    let d = data(scip, expr);

    *val = d.constant;
    for (c, &child) in scip.exprs[expr].children.iter().enumerate() {
        *val += d.coefficients[c] * scip.exprs[child].evalvalue;
    }

    Ok(())
}

/// `fwdiffSum`.
fn fwdiff_sum(scip: &mut Scip, expr: ExprId, dot: &mut f64, _direction: SolId) -> ScipResult {
    let d = data(scip, expr);

    *dot = 0.0;
    for (c, &child) in scip.exprs[expr].children.iter().enumerate() {
        *dot += d.coefficients[c] * scip.exprs[child].dot;
    }

    Ok(())
}

/// `bwdiffSum`.
fn bwdiff_sum(scip: &mut Scip, expr: ExprId, childidx: i32, val: &mut f64) -> ScipResult {
    *val = get_coefs_expr_sum(scip, expr)[childidx as usize];
    Ok(())
}

/// `bwfwdiffSum`.
fn bwfwdiff_sum(_scip: &mut Scip, _expr: ExprId, _childidx: i32, bardot: &mut f64, _direction: SolId) -> ScipResult {
    *bardot = 0.0;
    Ok(())
}

/// `intevalSum`.
fn inteval_sum(scip: &mut Scip, expr: ExprId, interval: &mut Interval, _intevalvar: Option<ExprIntEvalVar>, _intevalvardata: usize) -> ScipResult {
    let d = data(scip, expr);

    interval.set(d.constant);

    for (c, &child) in scip.exprs[expr].children.iter().enumerate() {
        let childinterval = scip.exprs[child].activity;
        if interval::is_empty(INTERVAL_INFINITY, childinterval) {
            interval.set_empty();
            break;
        }

        // Compute coefficients[c] * childinterval and add the result to the so
        // far computed interval.
        if d.coefficients[c] == 1.0 {
            let cur = *interval;
            interval::add(INTERVAL_INFINITY, interval, cur, childinterval);
        } else {
            let mut suminterval = Interval::default();
            interval::mul_scalar(INTERVAL_INFINITY, &mut suminterval, childinterval, d.coefficients[c]);
            let cur = *interval;
            interval::add(INTERVAL_INFINITY, interval, cur, suminterval);
        }
    }

    Ok(())
}

/// `initEstimatesSum`.
fn init_estimates_sum(scip: &mut Scip, expr: ExprId, _bounds: &[Interval], _overestimate: bool, coefs: &mut [Vec<f64>], constant: &mut [f64], nreturned: &mut i32) -> ScipResult {
    let n = scip.exprs[expr].children.len();
    let d = data(scip, expr);

    coefs[0][..n].copy_from_slice(&d.coefficients[..n]);
    constant[0] = d.constant;
    *nreturned = 1;

    Ok(())
}

/// `estimateSum`.
#[allow(clippy::too_many_arguments)]
fn estimate_sum(
    scip: &mut Scip,
    expr: ExprId,
    _localbounds: &[Interval],
    _globalbounds: &[Interval],
    _refpoint: &[f64],
    _overestimate: bool,
    _targetvalue: f64,
    coefs: &mut [f64],
    constant: &mut f64,
    islocal: &mut bool,
    success: &mut bool,
    branchcand: &mut [bool],
) -> ScipResult {
    let n = scip.exprs[expr].children.len();
    let d = data(scip, expr);

    // NOTE: nlhdlr_default assumes in nlhdlrInitSepaDefault that this
    // estimator can be used for both under- and overestimation.

    coefs[..n].copy_from_slice(&d.coefficients[..n]);
    *constant = d.constant;
    *islocal = false;
    *success = true;

    // For none of our children, branching would improve the underestimator,
    // so set branchcand[i]=FALSE everywhere; if we branch for numerical
    // reasons, then cons-expr-core should figure out what the candidates are.
    branchcand[..n].fill(false);

    Ok(())
}

/// `reversepropSum`.
fn reverseprop_sum(scip: &mut Scip, expr: ExprId, bounds: Interval, childrenbounds: &mut [Interval], infeasible: &mut bool) -> ScipResult {
    let nchildren = scip.exprs[expr].children.len();
    let d = data(scip, expr);

    let mut newbounds = vec![Interval::default(); nchildren];

    let nreductions = interval::propagate_weighted_sum(
        INTERVAL_INFINITY,
        &childrenbounds[..nchildren],
        &d.coefficients[..nchildren],
        d.constant,
        bounds,
        &mut newbounds,
        infeasible,
    );

    if !*infeasible && nreductions > 0 {
        childrenbounds[..nchildren].copy_from_slice(&newbounds);
    }

    Ok(())
}

/// `hashSum`.
fn hash_sum(scip: &mut Scip, expr: ExprId, hashkey: &mut u32, childrenhashes: &[u32]) -> ScipResult {
    let d = data(scip, expr);

    *hashkey = exprhdlr_hashkey();
    *hashkey ^= crate::misc::calc_fib_hash(d.constant);

    for c in 0..scip.exprs[expr].children.len() {
        *hashkey ^= crate::misc::calc_fib_hash(d.coefficients[c]) ^ childrenhashes[c];
    }

    Ok(())
}

/// `curvatureSum`.
fn curvature_sum(scip: &mut Scip, expr: ExprId, exprcurvature: ExprCurv, success: &mut bool, childcurv: &mut [ExprCurv]) -> ScipResult {
    let d = data(scip, expr);

    for i in 0..scip.exprs[expr].children.len() {
        childcurv[i] = exprcurv_multiply(d.coefficients[i], exprcurvature);
    }

    *success = true;

    Ok(())
}

/// `monotonicitySum`.
fn monotonicity_sum(scip: &mut Scip, expr: ExprId, childidx: i32, result: &mut Monotone) -> ScipResult {
    *result = if data(scip, expr).coefficients[childidx as usize] >= 0.0 {
        Monotone::Inc
    } else {
        Monotone::Dec
    };
    Ok(())
}

/// `integralitySum`.
fn integrality_sum(scip: &mut Scip, expr: ExprId, integrality: &mut ImplintType) -> ScipResult {
    let d = data(scip, expr);

    *integrality = if eps_is_int(d.constant, 0.0) { ImplintType::Strong } else { ImplintType::None };

    let children = &scip.exprs[expr].children;
    let mut i = 0;
    while i < children.len() && *integrality != ImplintType::None {
        let child = children[i];

        if eps_is_int(d.coefficients[i], 0.0) {
            let ci = scip.exprs[child].integrality;
            if (ci as i32) < (*integrality as i32) {
                *integrality = ci;
            }
        } else {
            *integrality = ImplintType::None;
        }
        i += 1;
    }

    Ok(())
}

/// `SCIPincludeExprhdlrSum`: creates the handler for sum expressions and
/// includes it into SCIP.
pub fn include_exprhdlr_sum(scip: &mut Scip) -> ScipResult {
    let exprhdlr = scip_expr::include_exprhdlr(scip, EXPRHDLR_NAME, Some(EXPRHDLR_DESC), EXPRHDLR_PRECEDENCE, eval_sum, None)?;

    let h = &mut scip.exprhdlrs[exprhdlr];
    h.copyhdlr = Some(copyhdlr_sum);
    h.copydata = Some(copydata_sum);
    h.freedata = Some(freedata_sum);
    h.simplify = Some(simplify_sum);
    h.compare = Some(compare_sum);
    h.print = true;
    h.inteval = Some(inteval_sum);
    h.initestimates = Some(init_estimates_sum);
    h.estimate = Some(estimate_sum);
    h.reverseprop = Some(reverseprop_sum);
    h.hash = Some(hash_sum);
    h.bwdiff = Some(bwdiff_sum);
    h.fwdiff = Some(fwdiff_sum);
    h.bwfwdiff = Some(bwfwdiff_sum);
    h.curvature = Some(curvature_sum);
    h.monotonicity = Some(monotonicity_sum);
    h.integrality = Some(integrality_sum);
    h.getsymdata = true;

    Ok(())
}

/// `SCIPcreateExprSum`: creates a sum expression.
pub fn create_expr_sum(
    scip: &mut Scip,
    children: &[ExprId],
    coefficients: Option<&[f64]>,
    constant: f64,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    let exprdata = create_data(children.len(), coefficients, constant);

    let h = scip.set.exprhdlrsum.expect("sum expression handler");
    expr::expr_create(scip, h, Some(Box::new(exprdata)), children, ownercreate, ownercreatedata)
}

/// `SCIPsetConstantExprSum`: sets the constant of a summation expression.
pub fn set_constant_expr_sum(scip: &mut Scip, expr: ExprId, constant: f64) {
    data_mut(scip, expr).constant = constant;
}

/// `SCIPappendExprSumExpr`: appends an expression to a sum expression.
pub fn append_expr_sum_expr(scip: &mut Scip, expr: ExprId, child: ExprId, childcoef: f64) {
    let nchildren = scip.exprs[expr].children.len();

    let d = data_mut(scip, expr);
    if d.coefficients.len() < nchildren + 1 {
        d.coefficients.resize(nchildren + 1, 0.0);
    }

    d.coefficients[nchildren] = childcoef;

    expr::expr_append_child(scip, expr, child);
}

/// `SCIPmultiplyByConstantExprSum`: multiplies given sum expression by a
/// constant.
pub fn multiply_by_constant_expr_sum(scip: &mut Scip, expr: ExprId, constant: f64) {
    let n = scip.exprs[expr].children.len();
    let d = data_mut(scip, expr);

    for coefficient in &mut d.coefficients[..n] {
        *coefficient *= constant;
    }
    d.constant *= constant;
}

/// `SCIPmultiplyBySumExprSum`: constructs the expanded product of two sum
/// expressions.
pub fn multiply_by_sum_expr_sum(
    scip: &mut Scip,
    factor1: ExprId,
    factor2: ExprId,
    simplify: bool,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    let constant1 = get_constant_expr_sum(scip, factor1);
    let constant2 = get_constant_expr_sum(scip, factor2);
    let nchildren1 = scip.exprs[factor1].children.len();
    let nchildren2 = scip.exprs[factor2].children.len();

    // TODO might be nice to integrate more with simplify and construct a
    // simplified sum right away.

    let mut product = create_expr_sum(scip, &[], None, constant1 * constant2, ownercreate, ownercreatedata)?;

    // First add constant1 * factor2; constant * constant2 already added
    // above.
    if constant1 != 0.0 {
        for i2 in 0..nchildren2 {
            let child = scip.exprs[factor2].children[i2];
            let c = constant1 * get_coefs_expr_sum(scip, factor2)[i2];
            append_expr_sum_expr(scip, product, child, c);
        }
    }

    for i1 in 0..nchildren1 {
        let coef1 = get_coefs_expr_sum(scip, factor1)[i1];
        let child1 = scip.exprs[factor1].children[i1];

        if constant2 != 0.0 {
            // Add coef1 * child1 * constant2.
            append_expr_sum_expr(scip, product, child1, coef1 * constant2);
        }

        for i2 in 0..nchildren2 {
            // Add coef1 * child1 * coef2 * child2.
            let coef2 = get_coefs_expr_sum(scip, factor2)[i2];
            let child2 = scip.exprs[factor2].children[i2];

            // Create child1 * child2 expr.
            let termfactors = [child1, child2];
            let mut termprod = crate::expr_product::create_expr_product(scip, &termfactors, 1.0, None, 0)?;

            let mut termprodsimplified = if simplify {
                // Simplify child1*child2.
                let (simplified, _changed, _infeasible) = scip_expr::simplify_expr(scip, termprod, ownercreate, ownercreatedata)?;
                expr::expr_release(scip, &mut termprod)?;
                simplified
            } else {
                termprod
            };

            // Append to product.
            append_expr_sum_expr(scip, product, termprodsimplified, coef1 * coef2);
            expr::expr_release(scip, &mut termprodsimplified)?;
        }
    }

    if simplify {
        let (prodsimplified, _changed, _infeasible) = scip_expr::simplify_expr(scip, product, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut product)?;
        product = prodsimplified;
    }

    Ok(product)
}

/// `SCIPpowerExprSum`: constructs the expanded power of a sum expression.
///
/// @attention The number of terms in the expansion grows exponential with
/// the exponent. Be aware of what you wish for.
pub fn power_expr_sum(scip: &mut Scip, base: ExprId, exponent: i32, simplify: bool, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    // For sum_i alpha_i expr_i and exponent p, this constructs
    //   sum_{beta} (p over beta) prod_i (alpha_i expr_i)^beta_i over all
    //   multiindex beta such that sum_i beta_i = p
    // See also https://en.wikipedia.org/wiki/Multinomial_theorem
    // Calculation of multinomials adapted from
    // https://github.com/m-j-w/MultinomialSeries.jl
    //
    // TODO might be nice to integrate more with simplify and construct a
    // simplified sum right away.
    let children = scip.exprs[base].children.clone();
    let nchildren = children.len();
    let coefs = get_coefs_expr_sum(scip, base)[..nchildren].to_vec();
    let constant = get_constant_expr_sum(scip, base);
    let haveconst = constant != 0.0;
    let nterms = nchildren + if haveconst { 1 } else { 0 };
    let exponent_u = exponent as usize;

    let mut result = create_expr_sum(scip, &[], None, 0.0, ownercreate, ownercreatedata)?;

    let mut beta = vec![0i32; nterms];
    let mut factorials = vec![0i32; exponent_u + 1];

    // Precompute factorials k!, k = 0...exponent.
    factorials[0] = 1;
    for i in 1..=exponent_u {
        factorials[i] = factorials[i - 1] * i as i32;
    }

    // Precompute children^k, k=1..exponent.
    let mut childrenpower = vec![vec![crate::var::NONE; exponent_u + 1]; nchildren];
    for i in 0..nchildren {
        childrenpower[i][1] = children[i];
        for (k, i_k) in (2..=exponent_u).zip(&mut childrenpower[i][2..=exponent_u]) {
            *i_k = crate::expr_pow::create_expr_pow(scip, children[i], k as f64, None, 0)?;
            if simplify {
                let (simplified, _changed, _infeasible) = scip_expr::simplify_expr(scip, *i_k, ownercreate, ownercreatedata)?;
                expr::expr_release(scip, i_k)?;
                *i_k = simplified;
            }
        }
    }

    // First multinomial is (exponent,0,0,...).
    beta[0] = exponent;
    let mut betapos = 0usize;
    'terminate: loop {
        // Compute multinomial coef exponent! / (beta[0]! * ... *
        // beta[nterms-1]!).
        let mut multinomialcoef = factorials[exponent_u];
        for i in 0..nterms {
            multinomialcoef /= factorials[beta[i] as usize];
        }

        // Construct new term for expanded sum.
        let mut newterm = crate::expr_product::create_expr_product(scip, &[], 1.0, ownercreate, ownercreatedata)?;
        let mut newtermcoef = multinomialcoef as f64;
        for i in 0..nterms {
            if beta[i] == 0 {
                continue;
            }

            if i == nterms - 1 && haveconst {
                // If constant term, then update newtermcoef.
                newtermcoef *= constant.powf(beta[i] as f64);
                continue;
            }

            // alpha_i^beta_i.
            newtermcoef *= coefs[i].powf(beta[i] as f64);

            // expr_i^beta_i.
            expr::expr_append_child(scip, newterm, childrenpower[i][beta[i] as usize]);
        }

        // Append newterm to sum.
        match scip.exprs[newterm].children.len() {
            0 => {
                // No factor in product, so it is a constant -> update constant in sum.
                let c = get_constant_expr_sum(scip, result) + newtermcoef;
                set_constant_expr_sum(scip, result, c);
            }
            1 => {
                // Only one factor in product -> add this factor itself to sum.
                let child = scip.exprs[newterm].children[0];
                append_expr_sum_expr(scip, result, child, newtermcoef);
            }
            _ => {
                if simplify {
                    // Simplify product.
                    let (simplified, _changed, _infeasible) = scip_expr::simplify_expr(scip, newterm, ownercreate, ownercreatedata)?;
                    expr::expr_release(scip, &mut newterm)?;
                    newterm = simplified;
                }

                // Append new term to sum.
                append_expr_sum_expr(scip, result, newterm, newtermcoef);
            }
        }
        expr::expr_release(scip, &mut newterm)?;

        // Determine next beta.
        while beta[betapos] == 0 {
            betapos -= 1;
        }

        if betapos == nterms - 1 {
            loop {
                if betapos == 0 {
                    break 'terminate;
                }
                betapos -= 1;
                if beta[betapos] != 0 {
                    break;
                }
            }

            let mut restsum = 0;
            for beta_i in &mut beta[betapos + 1..nterms] {
                restsum += *beta_i;
                *beta_i = 0;
            }
            beta[betapos + 1] = restsum;
        }

        if beta[betapos] > 0 {
            beta[betapos] -= 1;
            betapos += 1;
            beta[betapos] += 1;
        }
    }

    if simplify {
        let (simplified, _changed, _infeasible) = scip_expr::simplify_expr(scip, result, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut result)?;
        result = simplified;
    }

    for i in (0..nchildren).rev() {
        for k in (2..=exponent_u).rev() {
            expr::expr_release(scip, &mut childrenpower[i][k])?;
        }
    }

    Ok(result)
}

/// `SCIPgetCoefsExprSum`: gets the coefficients of a summation expression.
#[inline]
pub fn get_coefs_expr_sum(scip: &Scip, expr: ExprId) -> &[f64] {
    &data(scip, expr).coefficients
}

/// `SCIPgetConstantExprSum`: gets the constant of a summation expression.
#[inline]
pub fn get_constant_expr_sum(scip: &Scip, expr: ExprId) -> f64 {
    data(scip, expr).constant
}
