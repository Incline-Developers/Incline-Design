//! Port of `nlhdlr_quadratic.c`: nonlinear handler to handle quadratic
//! expressions.
//!
//! The intersection cuts (`nlhdlrEnfoQuadratic` and its helpers) need the
//! eigenvalues of the quadratic, which need Lapack; Lapack is not available
//! in the reference build, so the handler never participates in separation
//! and those are not ported.

use crate::{
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, INTERVAL_INFINITY},
    interval::{self, Interval},
    nlhdlr::{
        self, NLHDLR_METHOD_ACTIVITY, NLHDLR_METHOD_ALL, NLHDLR_METHOD_NONE, NLHDLR_METHOD_SEPAABOVE, NLHDLR_METHOD_SEPABELOW, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod,
    },
    retcode::ScipResult,
    rounding::{self, RoundMode},
    scip::Scip,
    types::{ConsId, ConshdlrId, ScipResultCode, SolId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "quadratic";
const NLHDLR_DESC: &str = "handler for quadratic expressions";
const NLHDLR_DETECTPRIORITY: i32 = 1;
const NLHDLR_ENFOPRIORITY: i32 = 100;

/* properties of the quadratic nlhdlr statistics table */
const INTERCUTS_MINVIOL: f64 = 1e-4;
const DEFAULT_USEINTERCUTS: bool = false;
const DEFAULT_USESTRENGTH: bool = false;
const DEFAULT_USEMONOIDAL: bool = true;
const DEFAULT_USEMINREP: bool = true;
const DEFAULT_USEBOUNDS: bool = false;
const DEFAULT_NCUTSROOT: i32 = 20;
const DEFAULT_NCUTS: i32 = 2;

/*
 * Data structures
 */

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
#[derive(Clone, Debug, Default)]
struct ExprData {
    /// Curvature of the quadratic representation of the expression.
    curvature: ExprCurv,
    /// Activity of linear part.
    linactivity: Interval,

    /* activities of quadratic parts as defined in nlhdlrIntevalQuadratic */
    /// Minimum activity of quadratic part where only terms with finite min
    /// activity contribute.
    minquadfiniteact: f64,
    /// Maximum activity of quadratic part where only terms with finite max
    /// activity contribute.
    maxquadfiniteact: f64,
    /// Number of quadratic terms contributing -infinity to activity.
    nneginfinityquadact: i32,
    /// Number of quadratic terms contributing +infinity to activity.
    nposinfinityquadact: i32,
    /// Activity of each quadratic term as defined in nlhdlrIntevalQuadratic
    /// (empty while not allocated).
    quadactivities: Vec<Interval>,
    /// Activity of quadratic part (sum of quadactivities).
    quadactivity: Interval,
    /// Value of activities tag when activities were computed.
    activitiestag: i64,

    /// Whether we are using the nlhdlr also for separation.
    separating: bool,
    /// Whether the quadratic expression is in original (non-aux) variables.
    origvars: bool,
}

/// `SCIP_NLHDLRDATA`: nonlinear handler data.
#[derive(Default)]
struct NlhdlrData {
    /* parameter */
    /// Whether to use intersection cuts for quadratic constraints or not.
    useintersectioncuts: bool,
    /// Whether the strengthening should be used.
    usestrengthening: bool,
    /// Whether monoidal strengthening should be used.
    usemonoidal: bool,
    /// Whether the minimal representation of the S-free set should be used
    /// (instead of the gauge).
    useminrep: bool,
    /// Use bounds of variables in quadratic as rays for intersection cuts.
    useboundsasrays: bool,
    /// Limit for number of cuts generated consecutively.
    ncutslimit: i32,
    /// Limit for number of cuts generated at root node.
    ncutslimitroot: i32,
    /// Maximal rank a slackvar can have.
    maxrank: i32,
    /// Minimal cut violation the generated cuts must fulfill to be added to
    /// the LP.
    mincutviolation: f64,
    /// Minimal violation the constraint must fulfill such that a cut can be
    /// generated.
    minviolation: f64,
    /// Determines at which nodes cut is used (if it's -1, it's used only at
    /// the root node, if it's n >= 0, it's used at every multiple of n).
    atwhichnodes: i32,
    /// Limit for number of rays we do the strengthening for.
    nstrengthlimit: i32,
    /// Should we try to sparisfy the intersection cuts?
    sparsifycuts: bool,
    /// Should cut be generated even with bad numerics when restricting to
    /// ray?
    ignorebadrayrestriction: bool,
    /// Should cut be added even when range / efficacy is large?
    ignorehighre: bool,
    /// For monoidal strengthening, should we track more statistics (more
    /// expensive).
    trackmore: bool,
}

#[inline]
fn ed(scip: &Scip, nlhdlrexprdata: usize) -> &ExprData {
    nlhdlr::exprdata::<ExprData>(scip, nlhdlrexprdata)
}

#[inline]
fn edm(scip: &mut Scip, nlhdlrexprdata: usize) -> &mut ExprData {
    nlhdlr::exprdata_mut::<ExprData>(scip, nlhdlrexprdata)
}

#[inline]
fn hd(scip: &Scip, nlhdlr: NlhdlrId) -> &NlhdlrData {
    nlhdlr::hdlrdata::<NlhdlrData>(scip, nlhdlr)
}

/// C's `MAX(a, b)`: `a >= b ? a : b`.
#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// `isPropagable`: returns whether a quadratic is "propagable", i.e., some
/// expression appears in at least two terms.
fn is_propagable(scip: &Scip, qexpr: ExprId) -> bool {
    for t in &expr::expr_get_quadratic_data(scip, qexpr).quadexprterms {
        // Actually MIN(2, nadjbilin), but we check >= 2.
        if (t.lincoef != 0.0) as i32 + (t.sqrcoef != 0.0) as i32 + t.nadjbilin >= 2 {
            return true;
        }
    }

    false
}

/// `isPropagableTerm`: returns whether a quadratic term is "propagable".
///
/// A term is propagable, if its variable (aka child expr) appears at least
/// twice, which is the case if at least two of the following hold: it
/// appears as a linear term (coef*expr), it appears as a square term
/// (coef*expr^2), it appears in a bilinear term, it appears in another
/// bilinear term.
fn is_propagable_term(scip: &Scip, qexpr: ExprId, idx: usize) -> bool {
    let t = &expr::expr_get_quadratic_data(scip, qexpr).quadexprterms[idx];

    // Actually MIN(2, nadjbilin), but we check >= 2.
    (t.lincoef != 0.0) as i32 + (t.sqrcoef != 0.0) as i32 + t.nadjbilin >= 2
}

/// `propagateBoundsQuadExpr`: solves a quadratic equation a expr^2 + b expr
/// in rhs (with b an interval) and reduces bounds on `expr` or deduces
/// infeasibility if possible.
fn propagate_bounds_quad_expr(scip: &mut Scip, e: ExprId, sqrcoef: f64, b: Interval, rhs: Interval, infeasible: &mut bool, nreductions: &mut i32) -> ScipResult {
    let exprbounds = crate::cons_nonlinear::get_expr_bounds_nonlinear(scip, e);
    if interval::is_empty(INTERVAL_INFINITY, exprbounds) {
        *infeasible = true;
        return Ok(());
    }

    // Compute solution of a*x^2 + b*x in rhs.
    let mut a = Interval::default();
    a.set(sqrcoef);
    let mut newrange = Interval::default();
    interval::solve_univariate_quad_expression(INTERVAL_INFINITY, &mut newrange, a, b, rhs, exprbounds);

    crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, e, newrange, infeasible, Some(nreductions))
}

/// `propagateBoundsLinExpr`: solves a linear equation b expr in rhs (with b
/// a scalar) and reduces bounds on `expr` or deduces infeasibility if
/// possible.
fn propagate_bounds_lin_expr(scip: &mut Scip, e: ExprId, b: f64, rhs: Interval, infeasible: &mut bool, nreductions: &mut i32) -> ScipResult {
    // Compute solution of b*x in rhs.
    let mut newrange = Interval::default();
    interval::div_scalar(INTERVAL_INFINITY, &mut newrange, rhs, b);

    crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, e, newrange, infeasible, Some(nreductions))
}

/// `computeMaxBoundaryForBilinearProp`: returns max of a/x - c*x for x in
/// {x1, x2} with x1, x2 > 0.
fn compute_max_boundary_for_bilinear_prop(a: f64, c: f64, x1: f64, x2: f64) -> f64 {
    let cneg = interval::negate_real(c);

    let roundmode = rounding::get_mode();
    rounding::set_mode(RoundMode::Upwards);
    let cand1 = rounding::add(rounding::div(a, x1), rounding::mul(cneg, x1));
    let cand2 = rounding::add(rounding::div(a, x2), rounding::mul(cneg, x2));
    rounding::set_mode(roundmode);

    cmax(cand1, cand2)
}

/// `computeMaxForBilinearProp`: returns max of a/x - c*x for x in dom; it
/// assumes that dom is contained in (0, +inf).
fn compute_max_for_bilinear_prop(a: f64, c: f64, dom: Interval) -> f64 {
    // If a >= 0, then the function is convex which means the maximum is at one
    // of the boundaries; if c = 0, then the function is monotone which means the
    // maximum is also at one of the boundaries; if a < 0, then the function is
    // concave. The function then has a maximum if and only if there is a point
    // with derivative 0, that is, iff -a/x^2 - c = 0 has a solution; i.e. if
    // -a/c >= 0, i.e. (using a<0 and c != 0), c > 0. Otherwise (that is, c<0),
    // the maximum is at one of the boundaries.
    if a >= 0.0 || c <= 0.0 {
        return compute_max_boundary_for_bilinear_prop(a, c, dom.inf, dom.sup);
    }

    // Now, the (unrestricted) maximum is at sqrt(-a/c); if the argmax is not in
    // the interior of dom then the solution is at a boundary, too; we check this
    // by computing an interval that contains sqrt(-a/c) first.
    let mut argmax = Interval::default();
    argmax.set(-a);
    let t = argmax;
    interval::div_scalar(INTERVAL_INFINITY, &mut argmax, t, c);
    let t = argmax;
    interval::square_root(INTERVAL_INFINITY, &mut argmax, t);

    // If the interval containing sqrt(-a/c) does not intersect with the
    // interior of dom, then the (restricted) maximum is at a boundary (we could
    // even say at which boundary, but that doesn't save much).
    if argmax.sup <= dom.inf || argmax.inf >= dom.sup {
        return compute_max_boundary_for_bilinear_prop(a, c, dom.inf, dom.sup);
    }

    // The maximum at sqrt(-a/c) is -2*sqrt(-a*c), so we compute an upper bound
    // for that by computing a lower bound for 2*sqrt(-a*c).
    let roundmode = rounding::get_mode();
    rounding::set_mode(RoundMode::Downwards);
    let negunresmax = rounding::mul(2.0, interval::nextafter(rounding::sqrt(rounding::mul(interval::negate_real(a), c)), 0.0));
    rounding::set_mode(roundmode);

    // If the interval containing sqrt(-a/c) is contained in dom, then we can
    // return -negunresmax.
    if argmax.inf >= dom.inf && argmax.sup <= dom.sup {
        return -negunresmax;
    }

    // Now what is left is the case where we cannot say for sure whether
    // sqrt(-a/c) is contained in dom or not, so we are conservative and return
    // the max of both cases, i.e., the max of the upper bounds on
    // -2*sqrt(-a*c), a/dom.inf-c*dom.inf, a/dom.sup-c*dom.sup.
    let boundarymax = compute_max_boundary_for_bilinear_prop(a, c, dom.inf, dom.sup);
    cmax(boundarymax, -negunresmax)
}

/// `computeRangeForBilinearProp`: computes the range of rhs/x - coef * x
/// for x in exprdom; this is used for the propagation of bilinear terms.
///
/// If 0 is in the exprdom, we set range to R (even though this is not quite
/// correct, it is correct for the intended use of the function).
fn compute_range_for_bilinear_prop(mut exprdom: Interval, mut coef: f64, mut rhs: Interval, range: &mut Interval) {
    if exprdom.inf <= 0.0 && 0.0 <= exprdom.sup {
        range.set_entire(INTERVAL_INFINITY);
        return;
    }

    // Reduce to positive case.
    if exprdom.sup < 0.0 {
        let t = exprdom;
        interval::mul_scalar(INTERVAL_INFINITY, &mut exprdom, t, -1.0);
        let t = rhs;
        interval::mul_scalar(INTERVAL_INFINITY, &mut rhs, t, -1.0);
        coef *= -1.0;
    }

    // Compute maximum and minimum.
    let max = compute_max_for_bilinear_prop(rhs.sup, coef, exprdom);
    let min = -compute_max_for_bilinear_prop(-rhs.inf, -coef, exprdom);

    // Set interval.
    range.set_bounds(min, max);
}

/// `reversePropagateLinearExpr`: reverse propagates coef_i expr_i +
/// constant in rhs.
#[allow(clippy::too_many_arguments)]
fn reverse_propagate_linear_expr(scip: &mut Scip, linexprs: &[ExprId], lincoefs: &[f64], constant: f64, rhs: Interval, infeasible: &mut bool, nreductions: &mut i32) -> ScipResult {
    let nlinexprs = linexprs.len();
    if nlinexprs == 0 {
        return Ok(());
    }

    let oldboundslin: Vec<Interval> = linexprs.iter().map(|&e| scip.exprs[e].activity).collect();
    let mut newboundslin = vec![Interval::default(); nlinexprs];

    // TODO use SCIPgetExprBoundsNonlinear(scip, linexprs[i]) ?
    *nreductions = interval::propagate_weighted_sum(INTERVAL_INFINITY, &oldboundslin, lincoefs, constant, rhs, &mut newboundslin, infeasible);

    if *nreductions > 0 && !*infeasible {
        // SCIP is more conservative with what constitutes a reduction than
        // interval arithmetic so we follow SCIP.
        *nreductions = 0;
        let mut i = 0;
        while i < nlinexprs && !*infeasible {
            crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, linexprs[i], newboundslin[i], infeasible, Some(&mut *nreductions))?;
            i += 1;
        }
    }

    Ok(())
}

/*
 * Callback methods of nonlinear handler
 */

/// `nlhdlrFreeexprdataQuadratic`: callback to free expression specific
/// data.
fn nlhdlr_freeexprdata_quadratic(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    scip.nlhdlrexprdatas.remove(*nlhdlrexprdata);
    *nlhdlrexprdata = NONE;
    Ok(())
}

/// `nlhdlrDetectQuadratic`: callback to detect structure in expression
/// tree.
///
/// A term is quadratic if it is a product expression of two expressions, or
/// it is power expression of an expression with exponent 2.0. A propagable
/// quadratic expression is a sum expression such that there is at least one
/// expr that appears at least twice; non-propagable terms are handled as if
/// they were linear terms (see the C for the details). Simple convex
/// quadratics like x^2 + y^2 are ignored since the default nlhdlr will take
/// care of them.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_quadratic(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    // Don't check if all enforcement methods are already ensured.
    if (*enforcing & NLHDLR_METHOD_ALL) == NLHDLR_METHOD_ALL {
        return Ok(());
    }

    // If it is not a sum of at least two terms, it is not interesting.
    // TODO: constraints of the form l<= x*y <= r ?
    if !expr::expr_is_sum(scip, e) || scip.exprs[e].children.len() < 2 {
        return Ok(());
    }

    // If we are in a subSCIP we don't want to separate intersection cuts.
    if crate::copy::get_subscip_depth(scip) > 0 {
        nlhdlr::hdlrdata_mut::<NlhdlrData>(scip, nlhdlr).useintersectioncuts = false;
    }

    // Check whether expression is quadratic (a sum with at least one square or
    // bilinear term).
    let isquadratic = expr::expr_check_quadratic(scip, e)?;

    // Not quadratic -> nothing for us.
    if !isquadratic {
        return Ok(());
    }

    let propagable = is_propagable(scip, e);

    // If we are not propagable and are in presolving, return.
    if !propagable && scip.set.stage == crate::scip::Stage::Presolving {
        return Ok(());
    }

    // If we do not use intersection cuts and are not propagable, then we do not
    // want to handle it at all; if not propagable, then we need to check the
    // curvature to decide if we want to generate intersection cuts.
    let useintersectioncuts = hd(scip, nlhdlr).useintersectioncuts;
    if !propagable && !useintersectioncuts {
        return Ok(());
    }

    // Store quadratic in nlhdlrexprdata.
    let id = scip.nlhdlrexprdatas.push(Box::new(ExprData::default()));
    *nlhdlrexprdata = id;

    // Every propagable quadratic expression will be handled since we can
    // propagate.
    if propagable {
        *participating |= NLHDLR_METHOD_ACTIVITY;
        *enforcing |= NLHDLR_METHOD_ACTIVITY;

        let (linexprs, nquadexprs) = {
            let q = expr::expr_get_quadratic_data(scip, e);
            (q.linexprs.clone(), q.quadexprterms.len())
        };
        edm(scip, id).quadactivities = vec![Interval::default(); nquadexprs];

        // Notify children of quadratic that we will need their activity for
        // propagation.
        for &le in &linexprs {
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, le, false, true, false, false)?;
        }

        for i in 0..nquadexprs {
            let (argexpr, adjbilin, sqrexpr) = {
                let t = &expr::expr_get_quadratic_data(scip, e).quadexprterms[i];
                (t.expr, t.adjbilin.clone(), t.sqrexpr)
            };

            if is_propagable_term(scip, e, i) {
                crate::cons_nonlinear::register_expr_usage_nonlinear(scip, argexpr, false, true, false, false)?;
            } else {
                // Non-propagable quadratic is either a single square term or a single
                // bilinear term; we should make use nlhdlrs in pow or product for this
                // term, so we register usage of the square or product expr instead of
                // argexpr.
                if sqrexpr != NONE {
                    crate::cons_nonlinear::register_expr_usage_nonlinear(scip, sqrexpr, false, true, false, false)?;
                } else {
                    // We have expr1 * other_expr or other_expr * expr1; know that expr1 is
                    // non propagable, but to decide if we want the bounds of expr1 or of
                    // the product expr1 * other_expr (or other_expr * expr1), we have to
                    // decide whether other_expr is also non propagable; due to the way we
                    // sort bilinear terms (by frequency), we can deduce that other_expr
                    // doesn't appear anywhere else (i.e. is non propagable) if the product
                    // is of the form expr1 * other_expr; however, if we see other_expr *
                    // expr1 we need to find other_expr and check whether it is propagable.
                    let (expr1, prodexpr) = {
                        let b = &expr::expr_get_quadratic_data(scip, e).bilinexprterms[adjbilin[0] as usize];
                        (b.expr1, b.prodexpr)
                    };

                    if expr1 == argexpr {
                        crate::cons_nonlinear::register_expr_usage_nonlinear(scip, prodexpr, false, true, false, false)?;
                    } else {
                        // Check if other_expr is propagable in which case we need the bounds
                        // of expr1; otherwise we just need the bounds of the product and this
                        // will be (or was) registered when the loop takes us to the quadexpr
                        // other_expr.
                        // TODO this should be done faster, maybe store pos1 in bilinexprterm
                        // or store quadexprterm's in bilinexprterm.
                        for j in 0..nquadexprs {
                            let exprj = expr::expr_get_quadratic_data(scip, e).quadexprterms[j].expr;
                            if expr1 == exprj {
                                if is_propagable_term(scip, e, j) {
                                    crate::cons_nonlinear::register_expr_usage_nonlinear(scip, argexpr, false, true, false, false)?;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Check if we are going to separate or not.
    edm(scip, id).curvature = ExprCurv::Unknown;

    // For now, we do not care about separation if it is not required.
    if (*enforcing & NLHDLR_METHOD_SEPABOTH) == NLHDLR_METHOD_SEPABOTH {
        // If nobody can do anything, remove data.
        if *participating == NLHDLR_METHOD_NONE {
            nlhdlr_freeexprdata_quadratic(scip, nlhdlr, e, nlhdlrexprdata)?;
        }
        return Ok(());
    }

    // Check if we can do something more: check curvature of quadratic function
    // stored in nlexprdata; this is currently only used to decide whether we
    // want to separate, so it can be skipped if in presolve.
    let curvature = expr::expr_compute_quadratic_curvature(scip, e, None, useintersectioncuts)?;
    edm(scip, id).curvature = curvature;

    // Get eigenvalues to be able to check whether they were computed.
    let haseigenvalues = expr::expr_get_quadratic_data(scip, e).eigenvalues.is_some();

    // If we use intersection cuts then we can handle any non-convex quadratic.
    if useintersectioncuts && haseigenvalues && (*enforcing & NLHDLR_METHOD_SEPABELOW) == 0 && curvature != ExprCurv::Convex {
        *participating |= NLHDLR_METHOD_SEPABELOW;
    }

    if useintersectioncuts && haseigenvalues && (*enforcing & NLHDLR_METHOD_SEPAABOVE) == 0 && curvature != ExprCurv::Concave {
        *participating |= NLHDLR_METHOD_SEPAABOVE;
    }

    // If nobody can do anything, remove data.
    if *participating == NLHDLR_METHOD_NONE {
        nlhdlr_freeexprdata_quadratic(scip, nlhdlr, e, nlhdlrexprdata)?;
        return Ok(());
    }

    // We only need auxiliary variables if we are going to separate.
    if *participating & NLHDLR_METHOD_SEPABOTH != 0 {
        let (linexprs, quadexprs) = {
            let q = expr::expr_get_quadratic_data(scip, e);
            (q.linexprs.clone(), q.quadexprterms.iter().map(|t| t.expr).collect::<Vec<_>>())
        };

        // Expressions appearing linearly.
        for le in linexprs {
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, le, true, false, false, false)?;
        }

        // Expressions appearing quadratically.
        for qe in quadexprs {
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, qe, true, false, false, false)?;
        }

        edm(scip, id).separating = true;
    }

    if expr::expr_are_quadratic_exprs_variables(scip, e) {
        scip.exprs[e].curvature = curvature;
        edm(scip, id).origvars = true;
    }

    Ok(())
}

/// `nlhdlrEvalauxQuadratic`: nonlinear handler auxiliary evaluation
/// callback.
fn nlhdlr_evalaux_quadratic(scip: &mut Scip, _nlhdlr: NlhdlrId, e: ExprId, nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    // If the quadratic is in the original variable we can just evaluate the
    // expression.
    if ed(scip, nlhdlrexprdata).origvars {
        *auxvalue = scip.exprs[e].evalvalue;
        return Ok(());
    }

    // TODO there was a *auxvalue = SCIPevalExprQuadratic(scip,
    // nlhdlrexprdata->qexpr, sol); here; any reason why not using this
    // anymore?

    let q = expr::expr_get_quadratic_data(scip, e);

    *auxvalue = q.constant;

    // Linear exprs.
    for (i, &le) in q.linexprs.iter().enumerate() {
        *auxvalue += q.lincoefs[i] * crate::api::get_sol_val(scip, sol, crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, le));
    }

    // Quadratic terms.
    for t in &q.quadexprterms {
        let solval = crate::api::get_sol_val(scip, sol, crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, t.expr));
        *auxvalue += (t.lincoef + t.sqrcoef * solval) * solval;
    }

    // Bilinear terms.
    for b in &q.bilinexprterms {
        *auxvalue += b.coef
            * crate::api::get_sol_val(scip, sol, crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, b.expr1))
            * crate::api::get_sol_val(scip, sol, crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, b.expr2));
    }

    Ok(())
}

/// `nlhdlrEnfoQuadratic`: nonlinear handler enforcement callback
/// (intersection cuts).
#[allow(clippy::too_many_arguments)]
fn nlhdlr_enfo_quadratic(
    _scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _cons: ConsId,
    _nlhdlr: NlhdlrId,
    _expr: ExprId,
    _nlhdlrexprdata: usize,
    _sol: SolId,
    _auxvalue: f64,
    _overestimate: bool,
    _allowweakcuts: bool,
    _separated: bool,
    _addbranchscores: bool,
    _branchcandonly: bool,
    _result: &mut ScipResultCode,
) -> ScipResult {
    // Only called for expressions the handler separates, which needs the
    // eigenvalues from Lapack.
    unimplemented!("nlhdlrEnfoQuadratic (intersection cuts need Lapack, which the reference build lacks)");
}

/// `nlhdlrIntevalQuadratic`: nonlinear handler forward propagation
/// callback.
fn nlhdlr_inteval_quadratic(
    scip: &mut Scip,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    iv: &mut Interval,
    _intevalvar: Option<ExprIntEvalVar>,
    _intevalvardata: usize,
) -> ScipResult {
    let (constant, linexprs, lincoefs, nquadexprs) = {
        let q = expr::expr_get_quadratic_data(scip, e);
        (q.constant, q.linexprs.clone(), q.lincoefs.clone(), q.quadexprterms.len())
    };

    // Compute activity of linear part, if some linear term has changed.
    {
        let mut linactivity = Interval::default();
        linactivity.set(constant);
        for (i, &le) in linexprs.iter().enumerate() {
            let mut linterminterval = scip.exprs[le].activity;
            if interval::is_empty(INTERVAL_INFINITY, linterminterval) {
                edm(scip, nlhdlrexprdata).linactivity = linactivity;
                iv.set_empty();
                return Ok(());
            }
            let t = linterminterval;
            interval::mul_scalar(INTERVAL_INFINITY, &mut linterminterval, t, lincoefs[i]);
            let t = linactivity;
            interval::add(INTERVAL_INFINITY, &mut linactivity, t, linterminterval);
        }
        edm(scip, nlhdlrexprdata).linactivity = linactivity;
    }

    // Compute activity of quadratic part.
    {
        let d = edm(scip, nlhdlrexprdata);
        d.nneginfinityquadact = 0;
        d.nposinfinityquadact = 0;
        d.minquadfiniteact = 0.0;
        d.maxquadfiniteact = 0.0;
        d.quadactivity.set(0.0);

        for i in 0..nquadexprs {
            let (qexpr, lincoef, sqrcoef, adjbilin, sqrexpr) = {
                let t = &expr::expr_get_quadratic_data(scip, e).quadexprterms[i];
                (t.expr, t.lincoef, t.sqrcoef, t.adjbilin.clone(), t.sqrexpr)
            };
            let nadjbilin = adjbilin.len();

            let quadlb;
            let quadub;
            if !is_propagable_term(scip, e, i) {
                // Term is not propagable, i.e., the exprs involved in term only appear
                // once; thus use the activity of the quadratic term directly and not
                // the activity of the exprs involed in the term. See also documentation
                // of DETECT.
                if sqrcoef != 0.0 {
                    let mut tmp = scip.exprs[sqrexpr].activity;
                    if interval::is_empty(INTERVAL_INFINITY, tmp) {
                        iv.set_empty();
                        return Ok(());
                    }
                    let t = tmp;
                    interval::mul_scalar(INTERVAL_INFINITY, &mut tmp, t, sqrcoef);
                    quadlb = tmp.inf;
                    quadub = tmp.sup;
                } else {
                    let (expr1, prodcoef, prodexpr) = {
                        let b = &expr::expr_get_quadratic_data(scip, e).bilinexprterms[adjbilin[0] as usize];
                        (b.expr1, b.coef, b.prodexpr)
                    };
                    if expr1 == qexpr {
                        // The quadratic expression expr1 appears only as expr1 * expr2, so
                        // its 'q' is expr1 * expr2.
                        let mut tmp = scip.exprs[prodexpr].activity;
                        if interval::is_empty(INTERVAL_INFINITY, tmp) {
                            iv.set_empty();
                            return Ok(());
                        }
                        let t = tmp;
                        interval::mul_scalar(INTERVAL_INFINITY, &mut tmp, t, prodcoef);
                        quadlb = tmp.inf;
                        quadub = tmp.sup;
                    } else {
                        // The quadratic expression expr1 appears as expr2 * expr1, thus its
                        // 'q' is empty, see also the Notes in the documentation of the
                        // function.
                        edm(scip, nlhdlrexprdata).quadactivities[i].set(0.0);
                        continue;
                    }
                }
            } else {
                if interval::is_empty(INTERVAL_INFINITY, scip.exprs[qexpr].activity) {
                    iv.set_empty();
                    return Ok(());
                }

                // b = [c_l]
                let mut b = Interval::default();
                b.set(lincoef);
                for &adj in adjbilin.iter().take(nadjbilin) {
                    let (expr1, expr2, bilincoef) = {
                        let bt = &expr::expr_get_quadratic_data(scip, e).bilinexprterms[adj as usize];
                        (bt.expr1, bt.expr2, bt.coef)
                    };

                    if expr1 != qexpr {
                        continue;
                    }

                    let mut bterm = scip.exprs[expr2].activity;
                    if interval::is_empty(INTERVAL_INFINITY, bterm) {
                        iv.set_empty();
                        return Ok(());
                    }

                    // b += [b_jl * expr_j] for j \in P_l
                    let t = bterm;
                    interval::mul_scalar(INTERVAL_INFINITY, &mut bterm, t, bilincoef);
                    let t = b;
                    interval::add(INTERVAL_INFINITY, &mut b, t, bterm);
                }

                // TODO: under which assumptions do we know that we just need to compute
                // min or max? its probably the locks that give some information here.
                let qact = scip.exprs[qexpr].activity;
                quadub = interval::quad_upper_bound(INTERVAL_INFINITY, sqrcoef, b, qact);

                // TODO: implement SCIPintervalQuadLowerBound.
                {
                    let mut minusb = Interval::default();
                    minusb.set_bounds(-b.sup, -b.inf);

                    quadlb = -interval::quad_upper_bound(INTERVAL_INFINITY, -sqrcoef, minusb, qact);
                }
            }

            let d = edm(scip, nlhdlrexprdata);
            d.quadactivities[i].set_bounds(quadlb, quadub);
            let (qa, qi) = (d.quadactivity, d.quadactivities[i]);
            interval::add(INTERVAL_INFINITY, &mut d.quadactivity, qa, qi);

            // Get number of +/-infinity contributions and compute finite activity.
            if quadlb <= -INTERVAL_INFINITY {
                d.nneginfinityquadact += 1;
            } else {
                let roundmode = rounding::get_mode();
                rounding::set_mode(RoundMode::Downwards);
                d.minquadfiniteact = rounding::add(d.minquadfiniteact, quadlb);
                rounding::set_mode(roundmode);
            }
            if quadub >= INTERVAL_INFINITY {
                d.nposinfinityquadact += 1;
            } else {
                let roundmode = rounding::get_mode();
                rounding::set_mode(RoundMode::Upwards);
                d.maxquadfiniteact = rounding::add(d.maxquadfiniteact, quadub);
                rounding::set_mode(roundmode);
            }
        }
    }

    // Interval evaluation is linear activity + quadactivity.
    let (la, qa) = (ed(scip, nlhdlrexprdata).linactivity, ed(scip, nlhdlrexprdata).quadactivity);
    interval::add(INTERVAL_INFINITY, iv, la, qa);

    let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");
    edm(scip, nlhdlrexprdata).activitiestag = crate::cons_nonlinear::get_cur_bounds_tag_nonlinear(scip, conshdlr);

    Ok(())
}

/// `nlhdlrReversepropQuadratic`: nonlinear handler reverse propagation
/// callback.
///
/// The implemented technique is a proxy for solving the problem min/max{ x_i
/// : quad expr in [quad expr] } and as such can be improved (see the C for
/// the derivation).
#[allow(clippy::too_many_arguments)]
fn nlhdlr_reverseprop_quadratic(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    bounds: Interval,
    infeasible: &mut bool,
    nreductions: &mut i32,
) -> ScipResult {
    *nreductions = 0;

    // Not possible to conclude finite bounds if the interval of the expression
    // is [-inf,inf].
    if interval::is_entire(INTERVAL_INFINITY, bounds) {
        return Ok(());
    }

    // Ensure that partial activities as stored in nlhdlrexprdata are uptodate;
    // if the activity stored in expr is more recent than the partial activities
    // stored in this nlhdlrexprdata, then we should reevaluate the partial
    // activities.
    let mut quadactivity = Interval::default();
    if scip.exprs[e].activitytag > ed(scip, nlhdlrexprdata).activitiestag {
        nlhdlr_inteval_quadratic(scip, nlhdlr, e, nlhdlrexprdata, &mut quadactivity, None, 0)?;
    }

    let (constant, linexprs, lincoefs, nquadexprs) = {
        let q = expr::expr_get_quadratic_data(scip, e);
        (q.constant, q.linexprs.clone(), q.lincoefs.clone(), q.quadexprterms.len())
    };

    // Propagate linear part in rhs = expr's interval - quadratic activity;
    // first, reconstruct the quadratic activity.
    {
        let d = ed(scip, nlhdlrexprdata);
        quadactivity.set_bounds(
            if d.nneginfinityquadact > 0 { -INTERVAL_INFINITY } else { d.minquadfiniteact },
            if d.nposinfinityquadact > 0 { INTERVAL_INFINITY } else { d.maxquadfiniteact },
        );
    }

    let mut rhs = Interval::default();
    interval::sub(INTERVAL_INFINITY, &mut rhs, bounds, quadactivity);

    reverse_propagate_linear_expr(scip, &linexprs, &lincoefs, constant, rhs, infeasible, nreductions)?;

    // Stop if we find infeasibility.
    if *infeasible {
        return Ok(());
    }

    // Propagate quadratic part in expr's interval - linear activity, where
    // linear activity was computed in INTEVAL.
    let linactivity = ed(scip, nlhdlrexprdata).linactivity;
    interval::sub(INTERVAL_INFINITY, &mut rhs, bounds, linactivity);

    let mut bilinexprs: Vec<ExprId> = Vec::with_capacity(nquadexprs);
    let mut bilincoefs: Vec<f64> = Vec::with_capacity(nquadexprs);

    for i in 0..nquadexprs {
        let (qexpr, lincoef, sqrcoef, adjbilin, sqrexpr) = {
            let t = &expr::expr_get_quadratic_data(scip, e).quadexprterms[i];
            (t.expr, t.lincoef, t.sqrcoef, t.adjbilin.clone(), t.sqrexpr)
        };
        let nadjbilin = adjbilin.len();

        // rhs_i = rhs - rest_i; to compute rest_i = [\sum_{k \neq i} q_k] we just
        // have to substract the activity of q_i from quadactivity; however, care
        // must be taken about infinities (see the C).
        let mut rest_i = Interval::default();
        {
            let d = ed(scip, nlhdlrexprdata);
            let qi = d.quadactivities[i];

            // Compute rest_i.sup.
            if qi.sup < INTERVAL_INFINITY && d.nposinfinityquadact == 0 {
                let roundmode = rounding::get_mode();
                rounding::set_mode(RoundMode::Upwards);
                rest_i.sup = rounding::sub(d.maxquadfiniteact, qi.sup);
                rounding::set_mode(roundmode);
            } else if qi.sup >= INTERVAL_INFINITY && d.nposinfinityquadact == 1 {
                rest_i.sup = d.maxquadfiniteact;
            } else {
                rest_i.sup = INTERVAL_INFINITY;
            }

            // Compute rest_i.inf.
            if qi.inf > -INTERVAL_INFINITY && d.nneginfinityquadact == 0 {
                let roundmode = rounding::get_mode();
                rounding::set_mode(RoundMode::Downwards);
                rest_i.inf = rounding::sub(d.minquadfiniteact, qi.inf);
                rounding::set_mode(roundmode);
            } else if qi.inf <= -INTERVAL_INFINITY && d.nneginfinityquadact == 1 {
                rest_i.inf = d.minquadfiniteact;
            } else {
                rest_i.inf = -INTERVAL_INFINITY;
            }
        }

        // Compute rhs_i.
        let mut rhs_i = Interval::default();
        interval::sub(INTERVAL_INFINITY, &mut rhs_i, rhs, rest_i);

        if interval::is_entire(INTERVAL_INFINITY, rhs_i) {
            continue;
        }

        // Try to propagate.
        if !is_propagable_term(scip, e, i) {
            if sqrcoef != 0.0 {
                // Solve sqrcoef sqrexpr in rhs_i.
                propagate_bounds_lin_expr(scip, sqrexpr, sqrcoef, rhs_i, infeasible, nreductions)?;
            } else {
                // qexpr only appears in a term of the form qexpr * other_expr (or
                // other_expr * qexpr); we only care about getting bounds for the
                // product, thus we will compute these bounds when qexpr appears as
                // qexpr * other_expr; note that if it appears as other_expr * qexpr,
                // then when we process other_expr bounds for the product will be
                // computed.
                // TODO: we can actually avoid computing rhs_i in the case that qexpr
                // is not propagable and it appears as other_expr * qexpr.
                let (expr1, prodcoef, prodexpr) = {
                    let b = &expr::expr_get_quadratic_data(scip, e).bilinexprterms[adjbilin[0] as usize];
                    (b.expr1, b.coef, b.prodexpr)
                };

                if expr1 == qexpr {
                    // Solve prodcoef prodexpr in rhs_i.
                    propagate_bounds_lin_expr(scip, prodexpr, prodcoef, rhs_i, infeasible, nreductions)?;
                }
            }
        } else {
            bilinexprs.clear();
            bilincoefs.clear();

            // Set b to [c_l].
            let mut b = Interval::default();
            b.set(lincoef);

            // Add [\sum_{j \in P_l} b_lj expr_j + c_l] into b.
            for &adj in adjbilin.iter().take(nadjbilin) {
                let (expr1, expr2, bilincoef) = {
                    let bt = &expr::expr_get_quadratic_data(scip, e).bilinexprterms[adj as usize];
                    (bt.expr1, bt.expr2, bt.coef)
                };

                if expr1 != qexpr {
                    continue;
                }

                let expr2bounds = crate::cons_nonlinear::get_expr_bounds_nonlinear(scip, expr2);
                if interval::is_empty(INTERVAL_INFINITY, expr2bounds) {
                    *infeasible = true;
                    break;
                }

                // b += [b_lj * expr_j] for j \in P_l
                let mut bterm = Interval::default();
                interval::mul_scalar(INTERVAL_INFINITY, &mut bterm, expr2bounds, bilincoef);
                let t = b;
                interval::add(INTERVAL_INFINITY, &mut b, t, bterm);

                // Remember b_lj and expr_j to propagate them too.
                bilinexprs.push(expr2);
                bilincoefs.push(bilincoef);
            }

            if !*infeasible {
                // Solve a_i expr_i^2 + b expr_i in rhs_i.
                propagate_bounds_quad_expr(scip, qexpr, sqrcoef, b, rhs_i, infeasible, nreductions)?;
            }

            if !bilinexprs.is_empty() && !*infeasible {
                // If 0 is not in [expr_i], then propagate bilincoefs^T bilinexpr in
                // rhs_i/expr_i - a_i expr_i - c_i.
                let qexprbounds = crate::cons_nonlinear::get_expr_bounds_nonlinear(scip, qexpr);
                if interval::is_empty(INTERVAL_INFINITY, qexprbounds) {
                    *infeasible = true;
                } else {
                    // Compute bilinrhs := [rhs_i/expr_i - a_i expr_i].
                    let mut bilinrhs = Interval::default();
                    compute_range_for_bilinear_prop(qexprbounds, sqrcoef, rhs_i, &mut bilinrhs);

                    if !interval::is_entire(INTERVAL_INFINITY, bilinrhs) {
                        let mut nreds = 0;

                        // Propagate \sum_{j \in P_i} b_ij expr_j + c_i in bilinrhs.
                        reverse_propagate_linear_expr(scip, &bilinexprs, &bilincoefs, lincoef, bilinrhs, infeasible, &mut nreds)?;

                        // TODO FIXME: we are overestimating the number of reductions: an
                        // expr might be tightened many times!
                        *nreductions += nreds;
                    }
                }
            }
        }

        // Stop if we find infeasibility.
        if *infeasible {
            break;
        }
    }

    Ok(())
}

/// `nlhdlrFreehdlrdataQuadratic`: callback to free data of handler.
fn nlhdlr_freehdlrdata_quadratic(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    scip.nlhdlrs[nlhdlr].data = None;
    Ok(())
}

/// `nlhdlrCopyhdlrQuadratic`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_quadratic(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_quadratic(targetscip)
}

/// `SCIPincludeNlhdlrQuadratic`: includes quadratic nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_quadratic(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Create nonlinear handler specific data.
    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_quadratic,
        nlhdlr_evalaux_quadratic,
        Some(Box::new(NlhdlrData::default())),
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_quadratic));
    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_freehdlrdata_quadratic));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_freeexprdata_quadratic));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, None, Some(nlhdlr_enfo_quadratic), None, None);
    nlhdlr::nlhdlr_set_prop(scip, nlhdlr, Some(nlhdlr_inteval_quadratic), Some(nlhdlr_reverseprop_quadratic));

    // Parameters.
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, nlhdlrs, nlhdlr, data, NlhdlrData, $f)
        };
    }
    let infinity = scip.set.infinity();
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/useintersectioncuts",
        "whether to use intersection cuts for quadratic constraints to separate",
        t!(bool, useintersectioncuts),
        false,
        DEFAULT_USEINTERCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/usestrengthening",
        "whether the strengthening should be used",
        t!(bool, usestrengthening),
        false,
        DEFAULT_USESTRENGTH,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/usemonoidal",
        "whether monoidal strengthening should be used",
        t!(bool, usemonoidal),
        false,
        DEFAULT_USEMONOIDAL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/useminrep",
        "whether the minimal representation of the S-free set should be used (instead of the gauge)",
        t!(bool, useminrep),
        false,
        DEFAULT_USEMINREP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/useboundsasrays",
        "use bounds of variables in quadratic as rays for intersection cuts",
        t!(bool, useboundsasrays),
        false,
        DEFAULT_USEBOUNDS,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/quadratic/ncutslimit",
        "limit for number of cuts generated consecutively",
        t!(i32, ncutslimit),
        false,
        DEFAULT_NCUTS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/quadratic/ncutslimitroot",
        "limit for number of cuts generated at root node",
        t!(i32, ncutslimitroot),
        false,
        DEFAULT_NCUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/quadratic/maxrank",
        "maximal rank a slackvar can have",
        t!(i32, maxrank),
        false,
        i32::MAX,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "nlhdlr/quadratic/mincutviolation",
        "minimal cut violation the generated cuts must fulfill to be added to the LP",
        t!(f64, mincutviolation),
        false,
        1e-4,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "nlhdlr/quadratic/minviolation",
        "minimal violation the constraint must fulfill such that a cut is generated",
        t!(f64, minviolation),
        false,
        INTERCUTS_MINVIOL,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/quadratic/atwhichnodes",
        "determines at which nodes cut is used (if it's -1, it's used only at the root node, if it's n >= 0, it's used at every multiple of n",
        t!(i32, atwhichnodes),
        false,
        1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/quadratic/nstrengthlimit",
        "limit for number of rays we do the strengthening for",
        t!(i32, nstrengthlimit),
        false,
        i32::MAX,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/sparsifycuts",
        "should we try to sparisfy the intersection cut?",
        t!(bool, sparsifycuts),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/ignorebadrayrestriction",
        "should cut be generated even with bad numerics when restricting to ray?",
        t!(bool, ignorebadrayrestriction),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/ignorenhighre",
        "should cut be added even when range / efficacy is large?",
        t!(bool, ignorehighre),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/quadratic/trackmore",
        "for monoidal strengthening, should we track more statistics (more expensive)?",
        t!(bool, trackmore),
        false,
        false,
        None,
    )?;

    // (The statistics table is not ported.)

    Ok(())
}
