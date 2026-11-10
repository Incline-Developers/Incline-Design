//! Port of `nlhdlr_quotient.c`: quotient nonlinear handler.

use crate::{
    def::INVALID,
    expr::{self, ExprId, ExprIntEvalVar, INTERVAL_INFINITY},
    interval::{self, Interval},
    nlhdlr::{self, NLHDLR_METHOD_ACTIVITY, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, SideType, SolId, VarId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "quotient";
const NLHDLR_DESC: &str = "nonlinear handler for quotient expressions";
const NLHDLR_DETECTPRIORITY: i32 = 20;
const NLHDLR_ENFOPRIORITY: i32 = 20;

/// Translate from one value of infinity to another: if val is >= infty1,
/// then give infty2, else give val.
#[inline]
fn infty2infty(infty1: f64, infty2: f64, val: f64) -> f64 {
    if val >= infty1 { infty2 } else { val }
}

/// C's `MAX(a, b)`: `a >= b ? a : b`.
#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// C's `MIN(a, b)`: `a <= b ? a : b`.
#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/*
 * Data structures
 */

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
#[derive(Clone, Copy, Debug)]
struct ExprData {
    /// Expression of the numerator.
    numexpr: ExprId,
    /// Coefficient of the numerator.
    numcoef: f64,
    /// Constant of the numerator.
    numconst: f64,
    /// Expression of the denominator.
    denomexpr: ExprId,
    /// Coefficient of the denominator.
    denomcoef: f64,
    /// Constant of the denominator.
    denomconst: f64,
    /// Constant.
    constant: f64,
}

#[inline]
fn ed(scip: &Scip, nlhdlrexprdata: usize) -> ExprData {
    *nlhdlr::exprdata::<ExprData>(scip, nlhdlrexprdata)
}

/*
 * Local methods
 */

/// `exprdataCreate`: helper method to create nonlinear handler expression
/// data.
#[allow(clippy::too_many_arguments)]
fn exprdata_create(scip: &mut Scip, numexpr: ExprId, numcoef: f64, numconst: f64, denomexpr: ExprId, denomcoef: f64, denomconst: f64, constant: f64) -> usize {
    // Store values.
    let d = ExprData {
        numexpr,
        numcoef,
        numconst,
        denomexpr,
        denomcoef,
        denomconst,
        constant,
    };

    // Capture expressions.
    expr::expr_capture(scip, numexpr);
    expr::expr_capture(scip, denomexpr);

    scip.nlhdlrexprdatas.push(Box::new(d))
}

/// `exprdataFree`: helper method to free nonlinear handler expression data.
fn exprdata_free(scip: &mut Scip, nlhdlrexprdata: &mut usize) -> ScipResult {
    let d = *scip.nlhdlrexprdatas.remove(*nlhdlrexprdata).downcast::<ExprData>().unwrap();

    // Release expressions.
    let mut denomexpr = d.denomexpr;
    expr::expr_release(scip, &mut denomexpr)?;
    let mut numexpr = d.numexpr;
    expr::expr_release(scip, &mut numexpr)?;

    // Free expression data of nonlinear handler.
    *nlhdlrexprdata = NONE;

    Ok(())
}

/// `transformExpr`: helper method to transform an expression g(x) into
/// a*f(x) + b; returns (f(x), a, b).
fn transform_expr(scip: &Scip, e: ExprId) -> (ExprId, f64, f64) {
    // Expression is a sum with one child.
    if expr::expr_is_sum(scip, e) && scip.exprs[e].children.len() == 1 {
        (
            scip.exprs[e].children[0],
            crate::expr_sum::get_coefs_expr_sum(scip, e)[0],
            crate::expr_sum::get_constant_expr_sum(scip, e),
        )
    } else {
        // Otherwise return 1 * f(x) + 0.
        (e, 1.0, 0.0)
    }
}

/// `detectExpr`: helper method to detect an expression of the form (a*x + b)
/// / (c*y + d) + e.
///
/// Due to the expansion of products, there are two types of expressions that
/// can be detected: 1. prod(f(x), pow(g(y),-1)), 2.
/// sum(prod(f(x),pow(g(y),-1)), pow(g(y),-1)).
///
/// @todo At the moment quotients like xy / z are not detected, because they
/// are turned into a product expression with three children, i.e., x * y * (1
/// / z).
fn detect_expr(scip: &mut Scip, e: ExprId, nlhdlrexprdata: &mut usize, success: &mut bool) -> ScipResult {
    let mut denomexpr = NONE;
    let mut numexpr = NONE;
    let mut xexpr = NONE;
    let mut yexpr = NONE;
    let mut nomfac = 1.0;
    let mut numconst = 0.0;

    *success = false;
    let mut a = 0.0;
    let mut b = 0.0;
    let mut c = 0.0;
    let mut d = 0.0;
    let mut ee = 0.0;

    // Possible structures only have two children.
    if scip.exprs[e].children.len() != 2 {
        return Ok(());
    }

    // Expression must be either a product or a sum.
    if !expr::expr_is_product(scip, e) && !expr::expr_is_sum(scip, e) {
        return Ok(());
    }

    let children = [scip.exprs[e].children[0], scip.exprs[e].children[1]];
    let is_inv = |scip: &Scip, x: ExprId| expr::expr_is_power(scip, x) && crate::expr_pow::get_exponent_expr_pow(scip, x) == -1.0;

    if expr::expr_is_product(scip, e) {
        // Case: prod(f(x), pow(g(y),-1)).
        if is_inv(scip, children[0]) {
            denomexpr = scip.exprs[children[0]].children[0];
            numexpr = children[1];
        } else if is_inv(scip, children[1]) {
            denomexpr = scip.exprs[children[1]].children[0];
            numexpr = children[0];
        }

        // Remember to scale the numerator by the coefficient stored in the product
        // expression.
        nomfac = crate::expr_product::get_coef_expr_product(scip, e);
    } else {
        // Case: sum(prod(f(x),pow(g(y),-1)), pow(g(y),-1)).
        let sumcoefs = crate::expr_sum::get_coefs_expr_sum(scip, e).to_vec();

        // children[0] is 1/g(y) and children[1] is a product of f(x) and 1/g(y).
        if is_inv(scip, children[0]) && expr::expr_is_product(scip, children[1]) && scip.exprs[children[1]].children.len() == 2 {
            let prodcoef = crate::expr_product::get_coef_expr_product(scip, children[1]);

            if children[0] == scip.exprs[children[1]].children[0] {
                denomexpr = scip.exprs[children[0]].children[0];
                numexpr = scip.exprs[children[1]].children[1];
            } else if children[0] == scip.exprs[children[1]].children[1] {
                denomexpr = scip.exprs[children[0]].children[0];
                numexpr = scip.exprs[children[1]].children[0];
            }

            // Remember scalar and constant for numerator.
            nomfac = sumcoefs[1] * prodcoef;
            numconst = sumcoefs[0];
        }
        // children[1] is 1/g(y) and children[0] is a product of f(x) and 1/g(y).
        else if is_inv(scip, children[1]) && expr::expr_is_product(scip, children[0]) && scip.exprs[children[0]].children.len() == 2 {
            let prodcoef = crate::expr_product::get_coef_expr_product(scip, children[0]);

            if children[1] == scip.exprs[children[0]].children[0] {
                denomexpr = scip.exprs[children[1]].children[0];
                numexpr = scip.exprs[children[0]].children[1];
            } else if children[1] == scip.exprs[children[0]].children[1] {
                denomexpr = scip.exprs[children[1]].children[0];
                numexpr = scip.exprs[children[0]].children[0];
            }

            // Remember scalar and constant for numerator.
            nomfac = sumcoefs[0] * prodcoef;
            numconst = sumcoefs[1];
        }

        // Remember the constant of the sum expression.
        ee = crate::expr_sum::get_constant_expr_sum(scip, e);
    }

    let nauxvaruses = |scip: &Scip| crate::cons_nonlinear::get_expr_n_auxvar_uses_nonlinear(scip, e);

    if denomexpr != NONE && numexpr != NONE {
        // Transform numerator and denominator to detect structures like (a * f(x)
        // + b) / (c * f(x) + d).
        (xexpr, a, b) = transform_expr(scip, numexpr);
        (yexpr, c, d) = transform_expr(scip, denomexpr);

        // Detection is only be successful if the expression of the numerator an
        // denominator are the same (so boundtightening can be stronger than
        // default) or we are going to provide estimators (there will be an
        // auxvar).
        *success = xexpr == yexpr || nauxvaruses(scip) > 0;
    }

    // Register usage of xexpr and yexpr; create nonlinear handler expression
    // data.
    if *success {
        // Request auxiliary variables for xexpr and yexpr if we will estimate;
        // mark that the bounds of the expression are important to construct the
        // estimators (TODO check the curvature of the univariate quotient, as
        // bounds may actually not be used); if univariate, then we also do
        // inteval and reverseprop, so mark that the activities will be used for
        // inteval.
        let uses = nauxvaruses(scip) > 0;
        crate::cons_nonlinear::register_expr_usage_nonlinear(scip, xexpr, uses, xexpr == yexpr, uses, uses)?;

        if xexpr != yexpr && nauxvaruses(scip) > 0 {
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, yexpr, true, false, true, true)?;
        }

        a *= nomfac;
        b = nomfac * b + numconst;

        *nlhdlrexprdata = exprdata_create(scip, xexpr, a, b, yexpr, c, d, ee);
    }

    Ok(())
}

/// `intEvalQuotient`: helper method to compute interval for (a x + b) / (c
/// x + d) + e.
fn int_eval_quotient(scip: &Scip, bnds: Interval, a: f64, b: f64, c: f64, d: f64, e: f64) -> Interval {
    let mut result = Interval::default();
    let mut denominterval = Interval::default();
    let mut numinterval = Interval::default();

    // Return empty interval if the domain of x is empty.
    if interval::is_empty(INTERVAL_INFINITY, bnds) {
        result.set_empty();
        return result;
    }

    // Compute bounds for denominator.
    interval::mul_scalar(INTERVAL_INFINITY, &mut denominterval, bnds, c);
    let t = denominterval;
    interval::add_scalar(INTERVAL_INFINITY, &mut denominterval, t, d);

    // There is no useful interval if 0 is in the interior of the interval of
    // the denominator.
    if denominterval.inf < 0.0 && denominterval.sup > 0.0 {
        result.set_entire(INTERVAL_INFINITY);
        return result;
    }

    // a d = b c implies that f(x) = b / d + e, i.e., f is constant.
    if a * d - b * c == 0.0 {
        result.set(b / d + e);
        return result;
    }

    // Evaluate for [x.inf,x.inf] and [x.sup,x.sup] independently.
    result.set_empty();

    for i in 0..2 {
        let mut quotinterval = Interval::default();
        let val = if i == 0 { bnds.inf } else { bnds.sup };

        // Set the resulting interval to a / c if the bounds is infinite.
        if scip.set.is_infinity(val.abs()) {
            quotinterval.set(a);
            let t = quotinterval;
            interval::div_scalar(INTERVAL_INFINITY, &mut quotinterval, t, c);
        } else {
            // a x' + b
            numinterval.set(val);
            let t = numinterval;
            interval::mul_scalar(INTERVAL_INFINITY, &mut numinterval, t, a);
            let t = numinterval;
            interval::add_scalar(INTERVAL_INFINITY, &mut numinterval, t, b);

            // c x' + d
            denominterval.set(val);
            let t = denominterval;
            interval::mul_scalar(INTERVAL_INFINITY, &mut denominterval, t, c);
            let t = denominterval;
            interval::add_scalar(INTERVAL_INFINITY, &mut denominterval, t, d);

            // (a x' + b) / (c x' + d) + e
            interval::div(INTERVAL_INFINITY, &mut quotinterval, numinterval, denominterval);
            let t = quotinterval;
            interval::add_scalar(INTERVAL_INFINITY, &mut quotinterval, t, e);
        }

        // Unify with the resulting interval.
        let t = result;
        interval::unify(&mut result, t, quotinterval);
    }

    result
}

/// `reversepropQuotient`: helper method to compute reverse propagation for
/// (a x + b) / (c x + d) + e.
fn reverseprop_quotient(mut bnds: Interval, a: f64, b: f64, c: f64, d: f64, e: f64) -> Interval {
    let mut result = Interval::default();
    result.set_empty();

    // Return empty interval if the domain of the expression is empty.
    if interval::is_empty(INTERVAL_INFINITY, bnds) {
        return result;
    }

    // Substract constant from bounds of the expression.
    let t = bnds;
    interval::sub_scalar(INTERVAL_INFINITY, &mut bnds, t, e);

    // If the expression is constant or the limit lies inside the domain,
    // nothing can be propagated.
    if a * d - b * c == 0.0 || (bnds.inf < a / c && bnds.sup > a / c) {
        result.set_entire(INTERVAL_INFINITY);
        return result;
    }

    // Compute bounds for [x.inf,x.inf] and [x.sup,x.sup] independently.
    for i in 0..2 {
        let mut denominator = Interval::default();
        let mut numerator = Interval::default();
        let mut quotient = Interval::default();
        let val = if i == 0 { bnds.inf } else { bnds.sup };

        // (d * x' - b)
        numerator.set(d);
        let t = numerator;
        interval::mul_scalar(INTERVAL_INFINITY, &mut numerator, t, val);
        let t = numerator;
        interval::add_scalar(INTERVAL_INFINITY, &mut numerator, t, -b);

        // (a - c * x')
        denominator.set(-c);
        let t = denominator;
        interval::mul_scalar(INTERVAL_INFINITY, &mut denominator, t, val);
        let t = denominator;
        interval::add_scalar(INTERVAL_INFINITY, &mut denominator, t, a);

        // (d * x' - b) / (a - c * x')
        interval::div(INTERVAL_INFINITY, &mut quotient, numerator, denominator);

        // Unify with the resulting interval.
        let t = result;
        interval::unify(&mut result, t, quotient);
    }

    result
}

/// `createRowprep`: adds data to given rowprep; the generated estimator is
/// always locally valid; the constant is moved to the left- or right-hand
/// side.
fn create_rowprep(rowprep: &mut RowPrep, vars: &[VarId], coefs: &[f64], constant: f64) {
    // Create rowprep.
    rowprep.add_side(-constant);
    rowprep.ensure_size(vars.len() + 1);

    // Add coefficients.
    rowprep.add_terms(vars, coefs);
}

/// `estimateUnivariate`: computes an estimator at a given point for the
/// univariate case (ax + b) / (cx + d) + e.
///
/// Depending on the reference point, the estimator is a tangent or a secant
/// on the graph. It depends on whether we are under- or overestimating,
/// whether we are on the left or on the right side of the singularity at
/// -d/c, and whether it is the monotone increasing (ad - bc > 0) or
/// decreasing part (ad - bc < 0). Together, there are 8 cases.
#[allow(clippy::too_many_arguments)]
fn estimate_univariate(
    scip: &Scip,
    lbx: f64,
    ubx: f64,
    gllbx: f64,
    glubx: f64,
    solx: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    coef: &mut f64,
    constant: &mut f64,
    overestimate: bool,
    local: &mut bool,
    branchinguseful: &mut bool,
    success: &mut bool,
) {
    *branchinguseful = true;
    *success = false;
    *coef = 0.0;
    *constant = 0.0;
    let singularity = -d / c;

    // Estimate is globally valid if local and global bounds are equal.
    *local = gllbx != lbx || glubx != ubx;

    // If 0 is in the denom interval, estimation is not possible.
    if scip.set.is_le(lbx, singularity) && scip.set.is_ge(ubx, singularity) {
        return;
    }

    let isinleftpart = ubx < singularity;
    let monincreasing = a * d - b * c > 0.0;

    // This encodes the 8 cases explained above.
    if monincreasing == (overestimate == isinleftpart) {
        // If one of the bounds is infinite, secant cannot be computed.
        if scip.set.is_infinity(-lbx) || scip.set.is_infinity(ubx) {
            return;
        }

        let lbeval = (a * lbx + b) / (c * lbx + d) + e;
        let ubeval = (a * ubx + b) / (c * ubx + d) + e;

        // Compute coefficient and constant of linear estimator.
        *coef = (ubeval - lbeval) / (ubx - lbx);
        *constant = ubeval - (*coef) * ubx;
    } else {
        let soleval = (a * solx + b) / (c * solx + d) + e;

        // Compute coefficient and constant of linear estimator.
        let t = d + c * solx;
        *coef = (a * d - b * c) / (t * t);
        *constant = soleval - (*coef) * solx;

        // Gradient cuts are globally valid if the singularity is not in
        // [gllbx,glubx].
        *local = scip.set.is_le(gllbx, singularity) && scip.set.is_ge(glubx, singularity);

        // Branching will not improve the convexification via tangent cuts.
        *branchinguseful = false;
    }

    // Avoid huge values in the cut.
    if scip.set.is_huge_value(coef.abs()) || scip.set.is_huge_value(constant.abs()) {
        return;
    }

    *success = true;
}

/// The bounds of `var` from its global bounds as an interval intersected
/// with the activity of `e`.
fn global_bounds_with_activity(scip: &mut Scip, var: VarId, e: ExprId) -> ScipResult<Interval> {
    let infinity = scip.set.infinity();
    let mut bnd = Interval::default();
    bnd.set_bounds(
        -infty2infty(infinity, INTERVAL_INFINITY, -scip.vars[var].glbdom.lb),
        infty2infty(infinity, INTERVAL_INFINITY, scip.vars[var].glbdom.ub),
    );
    expr::expr_eval_activity(scip, e)?;
    let t = bnd;
    interval::intersect_eps(&mut bnd, scip.set.epsilon(), t, scip.exprs[e].activity);
    Ok(bnd)
}

/// `estimateUnivariateQuotient`: helper method to compute estimator for the
/// univariate case; the estimator is stored in a given rowprep.
#[allow(clippy::too_many_arguments)]
fn estimate_univariate_quotient(
    scip: &mut Scip,
    sol: SolId,
    xexpr: ExprId,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    overestimate: bool,
    rowprep: &mut RowPrep,
    branchinguseful: &mut bool,
    success: &mut bool,
) -> ScipResult {
    let x = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, xexpr);

    // Get local bounds on xexpr.
    let bnd = global_bounds_with_activity(scip, x, xexpr)?;
    let lbx = bnd.inf;
    let ubx = bnd.sup;

    // Check whether variable has been fixed or has empty interval.
    if scip.set.is_eq(lbx, ubx) || ubx < lbx {
        *success = false;
        return Ok(());
    }

    // Get global variable bounds.
    let gllbx = scip.vars[x].glbdom.lb;
    let glubx = scip.vars[x].glbdom.ub;

    // Get and adjust solution value.
    let mut solx = crate::api::get_sol_val(scip, sol, x);
    solx = cmin(cmax(solx, lbx), ubx);

    // Compute an estimator.
    let mut coef = 0.0;
    let mut constant = 0.0;
    let mut local = false;
    estimate_univariate(
        scip,
        lbx,
        ubx,
        gllbx,
        glubx,
        solx,
        a,
        b,
        c,
        d,
        e,
        &mut coef,
        &mut constant,
        overestimate,
        &mut local,
        branchinguseful,
        success,
    );

    // Add estimator to rowprep, if successful.
    if *success {
        rowprep.name = format!("quot_{}_{}", scip.vars[x].name, crate::api::get_n_lps(scip));
        create_rowprep(rowprep, &[x], &[coef], constant);
        rowprep.local = local;
    }

    Ok(())
}

/// `hcGradCut`: helper method to compute a gradient cut for h^c(x,y) :=
/// 1/y ((x + sqrt(lbx ubx)) / (sqrt(lbx) + sqrt(ubx)))^2 at a given
/// reference point (see Zamora and Grossmann (1988) for more details).
fn hc_grad_cut(lbx: f64, ubx: f64, solx: f64, soly: f64, coefx: &mut f64, coefy: &mut f64, constant: &mut f64) {
    let tmp1 = (lbx * ubx).sqrt() + solx;
    let s = lbx.sqrt() + ubx.sqrt();
    let tmp2 = (s * s) * soly;

    *coefx = 2.0 * tmp1 / tmp2;
    *coefy = -(tmp1 * tmp1) / (tmp2 * soly);
    *constant = 2.0 * (lbx * ubx).sqrt() * tmp1 / tmp2;
}

/// `estimateBivariate`: computes an over- or underestimator at a given point
/// for the bivariate case x/y <=/>= z (see the C for the cases).
#[allow(clippy::too_many_arguments)]
fn estimate_bivariate(
    scip: &Scip,
    mut lbx: f64,
    mut ubx: f64,
    mut lby: f64,
    mut uby: f64,
    lbz: f64,
    ubz: f64,
    mut solx: f64,
    mut soly: f64,
    solz: f64,
    mut overestimate: bool,
    coefx: &mut f64,
    coefy: &mut f64,
    constant: &mut f64,
    branchingusefulx: &mut bool,
    branchingusefuly: &mut bool,
    success: &mut bool,
) {
    let mut negatedx = false;
    let mut negatedy = false;

    *branchingusefulx = true;
    *branchingusefuly = true;
    *success = true;
    *coefx = 0.0;
    *coefy = 0.0;
    *constant = 0.0;

    // If 0 is in [lby,uby], then it is not possible to compute an estimator.
    if scip.set.is_le(lby, 0.0) && scip.set.is_ge(uby, 0.0) {
        *success = false;
        return;
    }

    // Negate bounds of y if it is not positive.
    if uby < 0.0 {
        let tmp = uby;
        uby = -lby;
        lby = -tmp;
        soly = -soly;
        negatedy = true;
        overestimate = !overestimate;
    }

    // Case 1: 0 is in the interior of [lbx,ubx].
    if lbx < 0.0 && 0.0 < ubx {
        let mut mccoefy = 0.0;
        let mut mccoefaux = 0.0;
        let mut mcconst = 0.0;

        // As explained in the description of this method,
        // overestimating/underestimating the bilinear term results in an
        // underestimator/overestimator for x / y.
        crate::nlhdlr_bilinear::add_bilin_mccormick(
            scip,
            1.0,
            lbz,
            ubz,
            solz,
            lby,
            uby,
            soly,
            !overestimate,
            &mut mccoefaux,
            &mut mccoefy,
            &mut mcconst,
            success,
        );

        if !*success {
            return;
        }

        // Resulting estimator is x/b - a/b * y - c/b, where a*y + b*z + c is the
        // estimator for y*z.
        *coefx = 1.0 / mccoefaux;
        *coefy = -mccoefy / mccoefaux;
        *constant = -mcconst / mccoefaux;
    }
    // Case 2: 0 is not in the interior of [lbx,ubx].
    else {
        // Negate bounds of x if it is negative.
        if ubx <= 0.0 {
            let tmp = ubx;
            ubx = -lbx;
            lbx = -tmp;
            solx = -solx;
            negatedx = true;
            overestimate = !overestimate;
        }

        // Case 2a.
        if overestimate {
            // Check where the minimum is attained.
            if uby * solx - lbx * soly + lbx * lby <= lby * solx - ubx * soly + ubx * uby {
                *coefx = 1.0 / lby;
                *coefy = -lbx / (lby * uby);
                *constant = lbx / uby;
            } else {
                *coefx = 1.0 / uby;
                *coefy = -ubx / (lby * uby);
                *constant = ubx / lby;
            }
        }
        // Case 2b.
        else {
            // Compute gradient cut for h^c(x,y) at (solx,soly).
            hc_grad_cut(lbx, ubx, solx, soly, coefx, coefy, constant);

            // Estimator is independent of the bounds of y.
            *branchingusefuly = false;
        }
    }

    // Reverse negations of x and y in the resulting estimator.
    if negatedx {
        *coefx = -*coefx;
    }
    if negatedy {
        *coefy = -*coefy;
    }

    // If exactly one variable has been negated, then we have computed an
    // underestimate/overestimate for the negated expression, which results in
    // an overestimate/underestimate for the original expression.
    if negatedx != negatedy {
        *coefx = -*coefx;
        *coefy = -*coefy;
        *constant = -*constant;
    }

    // Avoid huge values in the estimator.
    if scip.set.is_huge_value(coefx.abs()) || scip.set.is_huge_value(coefy.abs()) || scip.set.is_huge_value(constant.abs()) {
        *success = false;
    }
}

/// `estimateBivariateQuotient`: construct an estimator for a quotient
/// expression of the form (ax + b) / (cy + d) + e; the resulting estimator is
/// stored in a rowprep.
///
/// The method first computes an estimator for x' / y' with x := ax + b and y
/// := cy + d and then transforms this estimator to one for the quotient (ax +
/// b) / (cy + d) + e.
#[allow(clippy::too_many_arguments)]
fn estimate_bivariate_quotient(
    scip: &mut Scip,
    xexpr: ExprId,
    yexpr: ExprId,
    auxvar: VarId,
    sol: SolId,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    overestimate: bool,
    rowprep: &mut RowPrep,
    branchingusefulx: &mut bool,
    branchingusefuly: &mut bool,
    success: &mut bool,
) -> ScipResult {
    let vars = [
        crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, xexpr),
        crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, yexpr),
    ];
    let mut coefs = [0.0, 0.0];
    let mut constant = 0.0;

    // Get bounds for x, y, and z.
    let bnd = global_bounds_with_activity(scip, vars[0], xexpr)?;
    let lbx = bnd.inf;
    let ubx = bnd.sup;

    let bnd = global_bounds_with_activity(scip, vars[1], yexpr)?;
    let lby = bnd.inf;
    let uby = bnd.sup;

    let lbz = scip.vars[auxvar].locdom.lb;
    let ubz = scip.vars[auxvar].locdom.ub;

    // Check whether one of the variables has been fixed or has empty domain.
    if scip.set.is_eq(lbx, ubx) || scip.set.is_eq(lby, uby) || ubx < lbx || uby < lby {
        *success = false;
        return Ok(());
    }

    // Get and adjust solution values.
    let mut solx = crate::api::get_sol_val(scip, sol, vars[0]);
    let mut soly = crate::api::get_sol_val(scip, sol, vars[1]);
    let mut solz = crate::api::get_sol_val(scip, sol, auxvar);
    solx = cmin(cmax(solx, lbx), ubx);
    soly = cmin(cmax(soly, lby), uby);
    solz = cmin(cmax(solz, lbz), ubz);

    // Compute an estimator.
    let (c0, c1) = coefs.split_at_mut(1);
    estimate_bivariate(
        scip,
        // Bounds of x'.
        cmin(a * lbx, a * ubx) + b,
        cmax(a * lbx, a * ubx) + b, // Bounds of y'.
        cmin(c * lby, c * uby) + d,
        cmax(c * lby, c * uby) + d,
        lbz,
        ubz,
        a * solx + b,
        c * soly + d,
        solz,
        overestimate,
        &mut c0[0],
        &mut c1[0],
        &mut constant,
        branchingusefulx,
        branchingusefuly,
        success,
    );

    // Add estimator to rowprep, if successful.
    if *success {
        // Transform estimator Ax' + By'+ C = A(ax + b) + B (cy + d) + C = (Aa) x
        // + (Bc) y + (C + Ab + Bd); add the constant e separately.
        constant += coefs[0] * b + coefs[1] * d + e;
        coefs[0] *= a;
        coefs[1] *= c;

        // Prepare rowprep.
        rowprep.name = format!("quot_{}_{}_{}", scip.vars[vars[0]].name, scip.vars[vars[1]].name, crate::api::get_n_lps(scip));
        create_rowprep(rowprep, &vars, &coefs, constant);
    }

    Ok(())
}

/*
 * Callback methods of nonlinear handler
 */

/// `nlhdlrCopyhdlrQuotient`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_quotient(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_quotient(targetscip)
}

/// `nlhdlrFreeExprDataQuotient`: callback to free expression specific data.
fn nlhdlr_free_expr_data_quotient(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    // Free expression data of nonlinear handler.
    exprdata_free(scip, nlhdlrexprdata)
}

/// `nlhdlrDetectQuotient`: callback to detect structure in expression tree.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_quotient(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    let mut success = false;

    // Call detection routine.
    detect_expr(scip, e, nlhdlrexprdata, &mut success)?;

    if success {
        if crate::cons_nonlinear::get_expr_n_auxvar_uses_nonlinear(scip, e) > 0 {
            *participating = NLHDLR_METHOD_SEPABOTH;
        }

        let d = ed(scip, *nlhdlrexprdata);
        if d.numexpr == d.denomexpr {
            // If univariate, then we also do inteval and reverseprop.
            *participating |= NLHDLR_METHOD_ACTIVITY;

            // If univariate, then all our methods are enforcing.
            *enforcing |= *participating;
        }
    }

    Ok(())
}

/// `nlhdlrEvalauxQuotient`: auxiliary evaluation callback of nonlinear
/// handler.
fn nlhdlr_evalaux_quotient(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    let d = ed(scip, nlhdlrexprdata);

    // Get auxiliary variables.
    let auxvarx = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.numexpr);
    let auxvary = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.denomexpr);

    // Get solution values of the auxiliary variables.
    let solvalx = crate::api::get_sol_val(scip, sol, auxvarx);
    let solvaly = crate::api::get_sol_val(scip, sol, auxvary);

    // Evaluate expression w.r.t. the values of the auxiliary variables.
    let nomval = d.numcoef * solvalx + d.numconst;
    let denomval = d.denomcoef * solvaly + d.denomconst;

    // Return SCIP_INVALID if the denominator evaluates to zero.
    *auxvalue = if denomval != 0.0 { d.constant + nomval / denomval } else { INVALID };

    Ok(())
}

/// `nlhdlrEstimateQuotient`: nonlinear handler under/overestimation
/// callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_estimate_quotient(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    _targetvalue: f64,
    addbranchscores: bool,
    rowpreps: &mut Vec<RowPrep>,
    success: &mut bool,
    addedbranchscores: &mut bool,
) -> ScipResult {
    let mut branchingusefulx = false;
    let mut branchingusefuly = false;

    *addedbranchscores = false;
    *success = false;

    let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);

    let d = ed(scip, nlhdlrexprdata);
    if d.numexpr == d.denomexpr {
        // Univariate case.
        estimate_univariate_quotient(
            scip,
            sol,
            d.numexpr,
            d.numcoef,
            d.numconst,
            d.denomcoef,
            d.denomconst,
            d.constant,
            overestimate,
            &mut rowprep,
            &mut branchingusefulx,
            success,
        )?;
    } else {
        // Bivariate case.
        let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e);
        estimate_bivariate_quotient(
            scip,
            d.numexpr,
            d.denomexpr,
            auxvar,
            sol,
            d.numcoef,
            d.numconst,
            d.denomcoef,
            d.denomconst,
            d.constant,
            overestimate,
            &mut rowprep,
            &mut branchingusefulx,
            &mut branchingusefuly,
            success,
        )?;
    }

    if *success {
        rowpreps.push(rowprep);
    }

    // Add branching scores if requested.
    if addbranchscores {
        let mut exprs = Vec::with_capacity(2);

        if branchingusefulx {
            exprs.push(d.numexpr);
        }
        if branchingusefuly {
            exprs.push(d.denomexpr);
        }

        // Compute violation w.r.t. the auxiliary variable(s).
        let violation = crate::cons_nonlinear::get_expr_rel_aux_violation_nonlinear(scip, e, auxvalue, sol, None, None);

        *addedbranchscores = crate::cons_nonlinear::add_exprs_viol_score_nonlinear(scip, &exprs, violation, sol)?;
    }

    Ok(())
}

/// `nlhdlrSollinearizeQuotient`: nonlinear handler solution linearization
/// callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_sollinearize_quotient(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    _solisbest: bool,
    overestimate: bool,
    underestimate: bool,
) -> ScipResult {
    let d = ed(scip, nlhdlrexprdata);

    // In the bivariate case, we do not get globally valid estimators.
    if d.numexpr != d.denomexpr {
        return Ok(());
    }

    let x = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.numexpr);

    let lbx = scip.vars[x].glbdom.lb;
    let ubx = scip.vars[x].glbdom.ub;
    let mut solx = crate::api::get_sol_val(scip, sol, x);
    solx = cmax(cmin(solx, ubx), lbx);

    // c == 0: overestimate, c == 1: underestimate.
    let start = if overestimate { 0 } else { 1 };
    let end = if underestimate { 2 } else { 1 };
    for c in start..end {
        let mut success = false;
        let mut local = true;
        let mut branchinguseful = false;
        let mut coef = 0.0;
        let mut constant = 0.0;

        // Compute estimator, will be secant or gradient.
        estimate_univariate(
            scip,
            lbx,
            ubx,
            lbx,
            ubx,
            solx,
            d.numcoef,
            d.numconst,
            d.denomcoef,
            d.denomconst,
            d.constant,
            &mut coef,
            &mut constant,
            c == 0,
            &mut local,
            &mut branchinguseful,
            &mut success,
        );

        // Skip if not successful, only locally valid, or just a secant
        // (branchinguseful is TRUE).
        if !success || local || branchinguseful {
            continue;
        }

        let mut rowprep = RowPrep::new(if c == 0 { SideType::Left } else { SideType::Right }, false);
        rowprep.add_term(x, coef);
        rowprep.add_term(crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e), -1.0);
        rowprep.add_constant(constant);
        rowprep.name = format!("quot_{}_sol{}", scip.vars[x].name, scip.sols[sol].index);

        let huge = scip.set.get_huge_value();
        rowprep.cleanup2(scip, sol, huge, Some(&mut success));

        // If cleanup succeeded and rowprep is still global, add to cutpool.
        if success && !rowprep.local {
            let mut row = rowprep.get_row_cons(scip, cons)?;
            crate::api::add_pool_cut(scip, row)?;
            crate::scip_lp::release_row(scip, &mut row)?;
        }
    }

    Ok(())
}

/// `nlhdlrIntevalQuotient`: nonlinear handler interval evaluation callback.
fn nlhdlr_inteval_quotient(
    scip: &mut Scip,
    _nlhdlr: NlhdlrId,
    _expr: ExprId,
    nlhdlrexprdata: usize,
    iv: &mut Interval,
    _intevalvar: Option<ExprIntEvalVar>,
    _intevalvardata: usize,
) -> ScipResult {
    // It is not possible to compute tighter intervals if both expressions are
    // different; we should not be called in this case, as we haven't said we
    // would participate in this activity in detect.
    let d = ed(scip, nlhdlrexprdata);

    // Get activity of the numerator (= denominator) expression.
    let bnds = scip.exprs[d.numexpr].activity;

    // Call interval evaluation for the univariate quotient expression.
    *iv = int_eval_quotient(scip, bnds, d.numcoef, d.numconst, d.denomcoef, d.denomconst, d.constant);

    Ok(())
}

/// `nlhdlrReversepropQuotient`: nonlinear handler callback for reverse
/// propagation.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_reverseprop_quotient(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    _expr: ExprId,
    nlhdlrexprdata: usize,
    bounds: Interval,
    infeasible: &mut bool,
    nreductions: &mut i32,
) -> ScipResult {
    // It is not possible to compute tighter intervals if both expressions are
    // different; we should not be called in this case, as we haven't said we
    // would participate in this activity in detect.
    let d = ed(scip, nlhdlrexprdata);

    // Call reverse propagation.
    let result = reverseprop_quotient(bounds, d.numcoef, d.numconst, d.denomcoef, d.denomconst, d.constant);

    // Tighten bounds of the expression.
    crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, d.numexpr, result, infeasible, Some(nreductions))
}

/*
 * nonlinear handler specific interface methods
 */

/// `SCIPincludeNlhdlrQuotient`: includes quotient nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_quotient(scip: &mut Scip) -> ScipResult {
    // Create nonlinear handler data (none).
    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_quotient,
        nlhdlr_evalaux_quotient,
        None,
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_quotient));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_quotient));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, None, None, Some(nlhdlr_estimate_quotient), None);
    nlhdlr::nlhdlr_set_sollinearize(scip, nlhdlr, Some(nlhdlr_sollinearize_quotient));
    nlhdlr::nlhdlr_set_prop(scip, nlhdlr, Some(nlhdlr_inteval_quotient), Some(nlhdlr_reverseprop_quotient));

    Ok(())
}
