//! Port of `nlhdlr_bilinear.c`: bilinear nonlinear handler.

use std::collections::HashMap;

use crate::{
    def::INVALID,
    expr::{self, ExprId, ExprIntEvalVar, INTERVAL_INFINITY},
    interval::{self, Interval},
    nlhdlr::{self, NLHDLR_METHOD_ACTIVITY, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    quad,
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, SideType, SolId, VarId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "bilinear";
const NLHDLR_DESC: &str = "bilinear handler for expressions";
/// It is important that the nlhdlr runs after the default nldhlr.
const NLHDLR_DETECTPRIORITY: i32 = -10;
const NLHDLR_ENFOPRIORITY: i32 = -10;

/// Minimum interiority for a reference point for applying separation.
const MIN_INTERIORITY: f64 = 0.01;
/// Minimum size of variable bounds for applying separation.
const MIN_ABSBOUNDSIZE: f64 = 0.1;

/*
 * Data structures
 */

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
#[derive(Clone, Debug, Default)]
pub struct ExprData {
    /// Inequalities for underestimation.
    pub underineqs: [f64; 6],
    /// Total number of inequalities for underestimation.
    pub nunderineqs: i32,
    /// Inequalities for overestimation.
    pub overineqs: [f64; 6],
    /// Total number of inequalities for overestimation.
    pub noverineqs: i32,
    /// Id of the last node that has been used for separation.
    pub lastnodeid: i64,
    /// Number of separation calls of the last node.
    pub nseparoundslastnode: i32,
}

/// `SCIP_NLHDLRDATA`: nonlinear handler data.
struct NlhdlrData {
    /// Expressions that have been detected by the nlhdlr.
    exprs: Vec<ExprId>,
    /// Hashmap to store the position of each expression in the exprs array
    /// (`None` until created).
    exprmap: Option<HashMap<ExprId, usize>>,

    /* parameter */
    /// Whether to use the interval evaluation callback of the nlhdlr.
    useinteval: bool,
    /// Whether to use the reverse propagation callback of the nlhdlr.
    usereverseprop: bool,
    /// Maximum number of separation rounds in the root node.
    maxseparoundsroot: i32,
    /// Maximum number of separation rounds in a local node.
    maxseparounds: i32,
    /// Maximum depth to apply separation.
    maxsepadepth: i32,
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

#[inline]
fn hdm(scip: &mut Scip, nlhdlr: NlhdlrId) -> &mut NlhdlrData {
    nlhdlr::hdlrdata_mut::<NlhdlrData>(scip, nlhdlr)
}

/*
 * Local methods
 */

/// `getIneqViol`: helper function to compute the violation of an
/// inequality of the form xcoef * x <= ycoef * y + constant for two corner
/// points of the domain [lbx,ubx] x [lby,uby].
fn get_ineq_viol(scip: &Scip, x: VarId, y: VarId, xcoef: f64, ycoef: f64, constant: f64) -> (f64, f64) {
    let norm = (xcoef * xcoef + ycoef * ycoef).sqrt();
    let (lbx, ubx) = (scip.vars[x].locdom.lb, scip.vars[x].locdom.ub);
    let (lby, uby) = (scip.vars[y].locdom.lb, scip.vars[y].locdom.ub);

    // Inequality can be used for underestimating xy if and only if xcoef *
    // ycoef > 0.
    if xcoef * ycoef >= 0.0 {
        // Violation for top-left and bottom-right corner.
        (
            cmax(0.0, (xcoef * lbx - ycoef * uby - constant) / norm),
            cmax(0.0, (xcoef * ubx - ycoef * lby - constant) / norm),
        )
    } else {
        // Violation for top-right and bottom-left corner.
        (
            cmax(0.0, (xcoef * ubx - ycoef * uby - constant) / norm),
            cmax(0.0, (xcoef * lbx - ycoef * lby - constant) / norm),
        )
    }
}

/// `useBilinIneqs`: auxiliary function to decide whether to use
/// inequalities for a strong relaxation of bilinear terms or not.
fn use_bilin_ineqs(scip: &Scip, x: VarId, y: VarId, refx: f64, refy: f64) -> bool {
    // Get variable bounds.
    let lbx = scip.vars[x].locdom.lb;
    let ubx = scip.vars[x].locdom.ub;
    let lby = scip.vars[y].locdom.lb;
    let uby = scip.vars[y].locdom.ub;

    // Compute interiority.
    let eps = scip.set.epsilon();
    let interiorityx = cmin(refx - lbx, ubx - refx) / cmax(ubx - lbx, eps);
    let interiorityy = cmin(refy - lby, uby - refy) / cmax(uby - lby, eps);
    let interiority = 2.0 * cmin(interiorityx, interiorityy);

    ubx - lbx >= MIN_ABSBOUNDSIZE && uby - lby >= MIN_ABSBOUNDSIZE && interiority >= MIN_INTERIORITY
}

/// `updateBilinearRelaxation`: helper function to update the best
/// relaxation for a bilinear term when using valid linear inequalities.
#[allow(clippy::too_many_arguments)]
fn update_bilinear_relaxation(
    scip: &Scip,
    x: VarId,
    y: VarId,
    bilincoef: f64,
    violside: SideType,
    refx: f64,
    refy: f64,
    ineqs: &[f64],
    nineqs: usize,
    mccormickval: f64,
    bestcoefx: &mut f64,
    bestcoefy: &mut f64,
    bestconst: &mut f64,
    bestval: &mut f64,
    success: &mut bool,
) {
    let mut constshift = [0.0, 0.0];
    let mut constant = 0.0;
    let mut xcoef = 0.0;
    let mut ycoef = 0.0;
    let mut update = false;

    // No inequalities available.
    if nineqs == 0 {
        return;
    }

    let lbx = scip.vars[x].locdom.lb;
    let ubx = scip.vars[x].locdom.ub;
    let lby = scip.vars[y].locdom.lb;
    let uby = scip.vars[y].locdom.ub;
    let overestimate = violside == SideType::Left;

    let set = &scip.set;

    // Check cases for which we can't compute a tighter relaxation.
    if set.is_feas_le(refx, lbx) || set.is_feas_ge(refx, ubx) || set.is_feas_le(refy, lby) || set.is_feas_ge(refy, uby) {
        return;
    }

    // Due to the feasibility tolerances of the LP and NLP solver, it might
    // possible that the reference point is violating the linear inequalities;
    // to ensure that we compute a valid underestimate, we relax the linear
    // inequality by changing its constant part.
    for i in 0..nineqs {
        constshift[i] = cmax(0.0, ineqs[3 * i] * refx - ineqs[3 * i + 1] * refy - ineqs[3 * i + 2]);
    }

    // Try to use both inequalities.
    if nineqs == 2 {
        compute_bilin_envelope2(
            scip,
            bilincoef,
            lbx,
            ubx,
            refx,
            lby,
            uby,
            refy,
            overestimate,
            ineqs[0],
            ineqs[1],
            ineqs[2] + constshift[0],
            ineqs[3],
            ineqs[4],
            ineqs[5] + constshift[1],
            &mut xcoef,
            &mut ycoef,
            &mut constant,
            &mut update,
        );

        if update {
            let val = xcoef * refx + ycoef * refy + constant;
            let relimpr = 1.0 - ((val - bilincoef * refx * refy).abs() + 1e-4) / ((*bestval - bilincoef * refx * refy).abs() + 1e-4);
            let absimpr = (val - *bestval).abs();

            // Update relaxation if possible.
            if relimpr > 0.05 && absimpr > 1e-3 && ((overestimate && set.is_rel_lt(val, *bestval)) || (!overestimate && set.is_rel_gt(val, *bestval))) {
                *bestcoefx = xcoef;
                *bestcoefy = ycoef;
                *bestconst = constant;
                *bestval = val;
                *success = true;
            }
        }
    }

    // Use inequalities individually.
    for i in 0..nineqs {
        compute_bilin_envelope1(
            scip,
            bilincoef,
            lbx,
            ubx,
            refx,
            lby,
            uby,
            refy,
            overestimate,
            ineqs[3 * i],
            ineqs[3 * i + 1],
            ineqs[3 * i + 2] + constshift[i],
            &mut xcoef,
            &mut ycoef,
            &mut constant,
            &mut update,
        );

        if update {
            let val = xcoef * refx + ycoef * refy + constant;
            let relimpr = 1.0 - ((val - bilincoef * refx * refy).abs() + 1e-4) / ((mccormickval - bilincoef * refx * refy).abs() + 1e-4);
            let absimpr = (val - *bestval).abs();

            // Update relaxation if possible.
            if relimpr > 0.05 && absimpr > 1e-3 && ((overestimate && set.is_rel_lt(val, *bestval)) || (!overestimate && set.is_rel_gt(val, *bestval))) {
                *bestcoefx = xcoef;
                *bestcoefy = ycoef;
                *bestconst = constant;
                *bestval = val;
                *success = true;
            }
        }
    }
}

/// `isPointFeasible`: helper function to determine whether a given point
/// satisfy given inequalities.
#[allow(clippy::too_many_arguments)]
fn is_point_feasible(scip: &Scip, x: f64, y: f64, lbx: f64, ubx: f64, lby: f64, uby: f64, ineqs: &[f64], nineqs: usize) -> bool {
    let set = &scip.set;

    // Check whether point satisfies the bounds.
    if set.is_lt(x, lbx) || set.is_gt(x, ubx) || set.is_lt(y, lby) || set.is_gt(y, uby) {
        return false;
    }

    // Check whether point satisfy the linear inequalities.
    for i in 0..nineqs {
        let coefx = ineqs[3 * i];
        let coefy = ineqs[3 * i + 1];
        let constant = ineqs[3 * i + 2];

        // TODO check with an absolute comparison?
        if set.is_gt(coefx * x - coefy * y - constant, 0.0) {
            return false;
        }
    }

    true
}

/// `getFeasiblePointsBilinear`: helper function for computing all vertices
/// of the polytope described by the linear inequalities and the local
/// extrema of the bilinear term along each inequality.
///
/// There are at most 22 points where the min/max can be achieved (given
/// that there are at most 4 inequalities): corners of [lbx,ubx]x[lby,uby]
/// (4), two intersection points for each inequality with the box (8),
/// global maximum / minimum on each inequality (4), intersection between two
/// inequalities (6).
#[allow(clippy::too_many_arguments)]
fn get_feasible_points_bilinear(
    scip: &mut Scip,
    e: ExprId,
    exprbounds: Interval,
    underineqs: &[f64],
    nunderineqs: usize,
    overineqs: &[f64],
    noverineqs: usize,
    levelset: bool,
    xs: &mut Vec<f64>,
    ys: &mut Vec<f64>,
) {
    let mut ineqs = [0.0; 12];
    let mut nineqs = 0;

    xs.clear();
    ys.clear();

    // Collect inequalities.
    for i in 0..noverineqs {
        ineqs[3 * nineqs] = overineqs[3 * i];
        ineqs[3 * nineqs + 1] = overineqs[3 * i + 1];
        ineqs[3 * nineqs + 2] = overineqs[3 * i + 2];
        nineqs += 1;
    }
    for i in 0..nunderineqs {
        ineqs[3 * nineqs] = underineqs[3 * i];
        ineqs[3 * nineqs + 1] = underineqs[3 * i + 1];
        ineqs[3 * nineqs + 2] = underineqs[3 * i + 2];
        nineqs += 1;
    }

    // Collect children.
    let child1 = scip.exprs[e].children[0];
    let child2 = scip.exprs[e].children[1];

    // Collect bounds of children.
    let (boundsx, boundsy) = if !levelset {
        // If called from inteval, then use activity.
        (scip.exprs[child1].activity, scip.exprs[child2].activity)
    } else {
        // If called from reverseprop, then use bounds.
        let bx = crate::cons_nonlinear::get_expr_bounds_nonlinear(scip, child1);
        let by = crate::cons_nonlinear::get_expr_bounds_nonlinear(scip, child2);

        // If children bounds are empty, then returning with *npoints==0 is the
        // way to go.
        if interval::is_empty(INTERVAL_INFINITY, bx) || interval::is_empty(INTERVAL_INFINITY, by) {
            return;
        }
        (bx, by)
    };
    let lbx = boundsx.inf;
    let ubx = boundsx.sup;
    let lby = boundsy.inf;
    let uby = boundsy.sup;

    let scip: &Scip = scip;
    let add = |px: f64, py: f64, xs: &mut Vec<f64>, ys: &mut Vec<f64>| {
        if is_point_feasible(scip, px, py, lbx, ubx, lby, uby, &ineqs, nineqs) {
            xs.push(px);
            ys.push(py);
        }
    };

    // Corner points that satisfy all inequalities.
    for i in 0..4 {
        let cx = if i < 2 { lbx } else { ubx };
        let cy = if i % 2 == 0 { lby } else { uby };

        add(cx, cy, xs, ys);
    }

    // Intersection point of inequalities with [lbx,ubx] x [lby,uby] and
    // extremum of xy on each inequality.
    for i in 0..nineqs {
        let coefx = ineqs[3 * i];
        let coefy = ineqs[3 * i + 1];
        let constant = ineqs[3 * i + 2];
        let mut px = [lbx, ubx, (coefy * lby + constant) / coefx, (coefy * uby + constant) / coefx, 0.0];
        let mut py = [(coefx * lbx - constant) / coefy, (coefx * ubx - constant) / coefy, lby, uby, 0.0];

        // The last entry corresponds to the extremum of xy on the line.
        py[4] = (-constant) / (2.0 * coefy);
        px[4] = constant / (2.0 * coefx);

        for j in 0..5 {
            add(px[j], py[j], xs, ys);
        }
    }

    // Intersection point between two inequalities.
    for i in 0..nineqs.saturating_sub(1) {
        let coefx1 = ineqs[3 * i];
        let coefy1 = ineqs[3 * i + 1];
        let constant1 = ineqs[3 * i + 2];

        for j in i + 1..nineqs {
            let coefx2 = ineqs[3 * j];
            let coefy2 = ineqs[3 * j + 1];
            let constant2 = ineqs[3 * j + 2];

            // No intersection point -> skip.
            if scip.set.is_zero(coefx2 * coefy1 - coefx1 * coefy2) {
                continue;
            }

            let py = (constant2 * coefx1 - constant1 * coefx2) / (coefx2 * coefy1 - coefx1 * coefy2);
            let px = (coefy1 * py + constant1) / coefx1;

            add(px, py, xs, ys);
        }
    }

    // Consider the intersection of the level set with 1. the boundary of the
    // box, 2. the linear inequalities; this adds at most for 4 (level set
    // curves) * 4 (inequalities) * 2 (intersection points) for all linear
    // inequalities and 4 (level set curves) * 2 (intersection points) with the
    // boundary of the box.
    if !levelset {
        return;
    }

    // Compute intersection of level sets with the boundary.
    let prodcoef = crate::expr_product::get_coef_expr_product(scip, e);
    for i in 0..2 {
        let vals = [lbx, ubx, lby, uby];

        // Fix auxiliary variable to its lower or upper bound and consider the
        // coefficient of the product.
        let mut val = if i == 0 { exprbounds.inf } else { exprbounds.sup };
        val /= prodcoef;

        for k in 0..4 {
            if !scip.set.is_zero(vals[k]) {
                let res = val / vals[k];

                // Fix x to lbx or ubx.
                if k < 2 && is_point_feasible(scip, vals[k], res, lbx, ubx, lby, uby, &ineqs, nineqs) {
                    xs.push(vals[k]);
                    ys.push(res);
                }
                // Fix y to lby or uby.
                else if k >= 2 && is_point_feasible(scip, res, vals[k], lbx, ubx, lby, uby, &ineqs, nineqs) {
                    xs.push(res);
                    ys.push(vals[k]);
                }
            }
        }
    }

    // Compute intersection points of level sets with the linear inequalities.
    for i in 0..nineqs {
        let coefx = ineqs[3 * i];
        let coefy = ineqs[3 * i + 1];
        let constant = ineqs[3 * i + 2];

        // Solve system of coefx x = coefy y + constant and X = xy which is the
        // same as computing the solutions of (coefy / coefx) y^2 + (constant /
        // coefx) y = inf(X) or sup(X).
        let mut sqrcoef = Interval::default();
        sqrcoef.set(coefy / coefx);
        let mut lincoef = Interval::default();
        lincoef.set(constant / coefx);

        for k in 0..2 {
            let mut rhs = Interval::default();
            let mut ybnds = Interval::default();
            let mut result = Interval::default();

            // Set right-hand side.
            if k == 0 {
                rhs.set(exprbounds.inf);
            } else {
                rhs.set(exprbounds.sup);
            }

            ybnds.set_bounds(lby, uby);
            interval::solve_univariate_quad_expression(INTERVAL_INFINITY, &mut result, sqrcoef, lincoef, rhs, ybnds);

            // Interval is empty -> no solution available.
            if interval::is_empty(INTERVAL_INFINITY, result) {
                continue;
            }

            // Compute and check point.
            let py = result.inf;
            let px = (coefy * py + constant) / coefx;

            if is_point_feasible(scip, px, py, lbx, ubx, lby, uby, &ineqs, nineqs) {
                xs.push(px);
                ys.push(py);
            }

            // Check for a second solution.
            if result.inf != result.sup {
                let py = result.sup;
                let px = (coefy * py + constant) / coefx;

                if is_point_feasible(scip, px, py, lbx, ubx, lby, uby, &ineqs, nineqs) {
                    xs.push(px);
                    ys.push(py);
                }
            }
        }
    }
}

/// `intevalBilinear`: computes interval for a bilinear term when using at
/// least one inequality.
fn inteval_bilinear(scip: &mut Scip, e: ExprId, underineqs: &[f64], nunderineqs: usize, overineqs: &[f64], noverineqs: usize) -> Interval {
    let mut iv = Interval { inf: 0.0, sup: 0.0 };

    // No inequalities available -> skip computation.
    if noverineqs == 0 && nunderineqs == 0 {
        iv.set_entire(INTERVAL_INFINITY);
        return iv;
    }

    // x or y has empty interval -> empty.
    let (c0, c1) = (scip.exprs[e].children[0], scip.exprs[e].children[1]);
    if interval::is_empty(INTERVAL_INFINITY, scip.exprs[c0].activity) || interval::is_empty(INTERVAL_INFINITY, scip.exprs[c1].activity) {
        iv.set_empty();
        return iv;
    }

    // Compute all feasible points (since we use levelset == FALSE, the value
    // of interval doesn't matter).
    let mut xs = Vec::with_capacity(22);
    let mut ys = Vec::with_capacity(22);
    get_feasible_points_bilinear(scip, e, iv, underineqs, nunderineqs, overineqs, noverineqs, false, &mut xs, &mut ys);

    // No feasible point left -> return an empty interval.
    if xs.is_empty() {
        iv.set_empty();
        return iv;
    }

    // Compute the minimum and maximum over all computed points.
    let mut inf = xs[0] * ys[0];
    let mut sup = inf;
    for i in 1..xs.len() {
        inf = cmin(inf, xs[i] * ys[i]);
        sup = cmax(sup, xs[i] * ys[i]);
    }

    // Adjust infinite values.
    inf = cmax(inf, -INTERVAL_INFINITY);
    sup = cmin(sup, INTERVAL_INFINITY);

    // Multiply resulting interval with coefficient of the product expression.
    iv.set_bounds(inf, sup);
    let coef = crate::expr_product::get_coef_expr_product(scip, e);
    if coef != 1.0 {
        let b = iv;
        interval::mul_scalar(INTERVAL_INFINITY, &mut iv, b, coef);
    }

    iv
}

/// `reversePropBilinear`: uses inequalities for bilinear terms to get
/// stronger bounds during reverse propagation.
#[allow(clippy::too_many_arguments)]
fn reverse_prop_bilinear(
    scip: &mut Scip,
    e: ExprId,
    exprbounds: Interval,
    underineqs: &[f64],
    nunderineqs: usize,
    overineqs: &[f64],
    noverineqs: usize,
    intervalx: &mut Interval,
    intervaly: &mut Interval,
) {
    let mut first = true;

    // Set intervals to be empty.
    intervalx.set_empty();
    intervaly.set_empty();

    // Compute feasible points.
    let mut xs = Vec::with_capacity(62);
    let mut ys = Vec::with_capacity(62);
    get_feasible_points_bilinear(scip, e, exprbounds, underineqs, nunderineqs, overineqs, noverineqs, true, &mut xs, &mut ys);

    // No feasible points left -> problem is infeasible.
    if xs.is_empty() {
        return;
    }

    // Get bounds of the product expression.
    let exprinf = exprbounds.inf;
    let exprsup = exprbounds.sup;

    // Update intervals with the computed points.
    let coef = crate::expr_product::get_coef_expr_product(scip, e);
    let feastol = scip.set.feastol();
    for i in 0..xs.len() {
        let val = coef * xs[i] * ys[i];

        // Only accept points for which the value of x*y is in the interval of
        // the product expression.
        // NOTE: in order to consider all relevant points, we are a bit
        // conservative here and relax the interval of the expression by
        // SCIPfeastol().
        if scip.set.is_rel_ge(val, exprinf - feastol) && scip.set.is_rel_le(val, exprsup + feastol) {
            if first {
                intervalx.set(xs[i]);
                intervaly.set(ys[i]);
                first = false;
            } else {
                intervalx.inf = cmin(intervalx.inf, xs[i]);
                intervalx.sup = cmax(intervalx.sup, xs[i]);
                intervaly.inf = cmin(intervaly.inf, ys[i]);
                intervaly.sup = cmax(intervaly.sup, ys[i]);
            }
        }
    }
}

/// `computeBilinEnvelope2` (static): helper function to compute the convex
/// envelope of a bilinear term when two linear inequalities are given; we
/// use the same notation and formulas as in Locatelli 2016.
#[allow(clippy::too_many_arguments)]
fn compute_bilin_envelope2_inner(
    scip: &Scip,
    x: f64,
    y: f64,
    mi: f64,
    qi: f64,
    mj: f64,
    qj: f64,
    xi: &mut f64,
    yi: &mut f64,
    xj: &mut f64,
    yj: &mut f64,
    xcoef: &mut f64,
    ycoef: &mut f64,
    constant: &mut f64,
) {
    if scip.set.is_eq(mi, mj) {
        // xi = (x + mi * y - qi) / (2.0*mi)
        let mut xiq = quad::prod_dd(mi, y);
        xiq = quad::sum_qd(xiq, x);
        xiq = quad::sum_qd(xiq, -qi);
        xiq = quad::div_qd(xiq, 2.0 * mi);

        // yi = mi*(*xi) + qi
        let mut yiq = quad::prod_qd(xiq, mi);
        yiq = quad::sum_qd(yiq, qi);

        // xj = (*xi) + (qi - qj)/ (2.0*mi)
        let mut xjq = quad::sum_dd(qi, -qj);
        xjq = quad::div_qd(xjq, 2.0 * mi);
        xjq = quad::sum_qq(xjq, xiq);

        // yj = mj * (*xj) + qj
        let mut yjq = quad::prod_qd(xjq, mj);
        yjq = quad::sum_qd(yjq, qj);

        // ycoef = (*xi) + (qi - qj) / (4.0*mi) note that this is wrong in
        // Locatelli 2016
        let mut ycoefq = quad::sum_dd(qi, -qj);
        ycoefq = quad::div_qd(ycoefq, 4.0 * mi);
        ycoefq = quad::sum_qq(ycoefq, xiq);

        // xcoef = 2.0*mi*(*xi) - mi * (*ycoef) + qi
        let mut xcoefq = quad::prod_qd(xiq, 2.0 * mi);
        let tmpq = quad::prod_qd(ycoefq, -mi);
        xcoefq = quad::sum_qq(xcoefq, tmpq);
        xcoefq = quad::sum_qd(xcoefq, qi);

        // constant = -mj*SQR(*xj) - (*ycoef) * qj
        let mut constantq = quad::square_q(xjq);
        constantq = quad::prod_qd(constantq, -mj);
        let tmpq = quad::prod_qd(ycoefq, -qj);
        constantq = quad::sum_qq(constantq, tmpq);

        *xi = xiq.to_dbl();
        *yi = yiq.to_dbl();
        *xj = xjq.to_dbl();
        *yj = yjq.to_dbl();
        *ycoef = ycoefq.to_dbl();
        *xcoef = xcoefq.to_dbl();
        *constant = constantq.to_dbl();
    } else if mi > 0.0 {
        // xi = (y + sqrt(mi*mj)*x - qi) / (REALABS(mi) + sqrt(mi*mj))
        let mut xiq = quad::prod_dd(mi, mj);
        xiq = quad::sqrt_q(xiq);
        xiq = quad::prod_qd(xiq, x);
        xiq = quad::sum_qd(xiq, y);
        // (y + sqrt(mi*mj)*x - qi)
        xiq = quad::sum_qd(xiq, -qi);
        let mut tmpq = quad::prod_dd(mi, mj);
        tmpq = quad::sqrt_q(tmpq);
        // REALABS(mi) + sqrt(mi*mj)
        tmpq = quad::sum_qd(tmpq, mi.abs());
        xiq = quad::div_qq(xiq, tmpq);

        // yi = mi*(*xi) + qi
        let mut yiq = quad::prod_qd(xiq, mi);
        yiq = quad::sum_qd(yiq, qi);

        // xj = (y + sqrt(mi*mj)*x - qj) / (REALABS(mj) + sqrt(mi*mj))
        let mut xjq = quad::prod_dd(mi, mj);
        xjq = quad::sqrt_q(xjq);
        xjq = quad::prod_qd(xjq, x);
        xjq = quad::sum_qd(xjq, y);
        // (y + sqrt(mi*mj)*x - qj)
        xjq = quad::sum_qd(xjq, -qj);
        let mut tmpq = quad::prod_dd(mi, mj);
        tmpq = quad::sqrt_q(tmpq);
        // REALABS(mj) + sqrt(mi*mj)
        tmpq = quad::sum_qd(tmpq, mj.abs());
        xjq = quad::div_qq(xjq, tmpq);

        // yj = mj*(*xj) + qj
        let mut yjq = quad::prod_qd(xjq, mj);
        yjq = quad::sum_qd(yjq, qj);

        // ycoef = (2.0*mj*(*xj) + qj - 2.0*mi*(*xi) - qi) / (mj - mi)
        let mut ycoefq = quad::prod_qd(xjq, 2.0 * mj);
        ycoefq = quad::sum_qd(ycoefq, qj);
        let tmpq = quad::prod_qd(xiq, -2.0 * mi);
        ycoefq = quad::sum_qq(ycoefq, tmpq);
        ycoefq = quad::sum_qd(ycoefq, -qi);
        let tmpq = quad::sum_dd(mj, -mi);
        ycoefq = quad::div_qq(ycoefq, tmpq);

        // xcoef = 2.0*mj*(*xj) + qj - mj*(*ycoef)
        let mut xcoefq = quad::prod_qd(xjq, 2.0 * mj);
        xcoefq = quad::sum_qd(xcoefq, qj);
        let tmpq = quad::prod_qd(ycoefq, -mj);
        xcoefq = quad::sum_qq(xcoefq, tmpq);

        // constant = -mj*SQR(*xj) - (*ycoef) * qj
        let mut constantq = quad::square_q(xjq);
        constantq = quad::prod_qd(constantq, -mj);
        let tmpq = quad::prod_qd(ycoefq, -qj);
        constantq = quad::sum_qq(constantq, tmpq);

        *xi = xiq.to_dbl();
        *yi = yiq.to_dbl();
        *xj = xjq.to_dbl();
        *yj = yjq.to_dbl();
        *ycoef = ycoefq.to_dbl();
        *xcoef = xcoefq.to_dbl();
        *constant = constantq.to_dbl();
    } else {
        // Apply variable transformation x = -x in case for overestimation.
        compute_bilin_envelope2_inner(scip, -x, y, -mi, qi, -mj, qj, xi, yi, xj, yj, xcoef, ycoef, constant);

        // Revert transformation; multiply cut by -1 and change -x by x.
        *xi = -*xi;
        *xj = -*xj;
        *ycoef = -*ycoef;
        *constant = -*constant;
    }
}

/*
 * Callback methods of nonlinear handler
 */

/// `nlhdlrCopyhdlrBilinear`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_bilinear(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_bilinear(targetscip)
}

/// `nlhdlrFreehdlrdataBilinear`: callback to free data of handler.
fn nlhdlr_freehdlrdata_bilinear(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    scip.nlhdlrs[nlhdlr].data = None;
    Ok(())
}

/// `nlhdlrFreeExprDataBilinear`: callback to free expression specific data.
fn nlhdlr_free_expr_data_bilinear(scip: &mut Scip, nlhdlr: NlhdlrId, e: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    let h = hdm(scip, nlhdlr);
    let pos = h.exprmap.as_ref().unwrap()[&e];
    let nexprs = h.exprs.len();

    // Move the last expression to the free position.
    if nexprs > 0 && pos != nexprs - 1 {
        let lastexpr = h.exprs[nexprs - 1];
        h.exprs[pos] = lastexpr;
        h.exprs[nexprs - 1] = NONE;
        h.exprmap.as_mut().unwrap().insert(lastexpr, pos);
    }

    // Remove expression from the nonlinear handler data.
    h.exprmap.as_mut().unwrap().remove(&e);
    h.exprs.pop();
    let mut e = e;
    expr::expr_release(scip, &mut e)?;

    // Free nonlinear handler expression data.
    if *nlhdlrexprdata != NONE {
        scip.nlhdlrexprdatas.remove(*nlhdlrexprdata);
        *nlhdlrexprdata = NONE;
    }

    Ok(())
}

/// `nlhdlrExitBilinear`: callback to be called in deinitialization.
fn nlhdlr_exit_bilinear(_scip: &mut Scip, _nlhdlr: NlhdlrId) -> ScipResult {
    Ok(())
}

/// `nlhdlrDetectBilinear`: callback to detect structure in expression tree.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_bilinear(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    _enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    // Only during solving will we have the extra inequalities that we rely on
    // so much here.
    if scip.set.stage < crate::scip::Stage::InitSolve {
        return Ok(());
    }

    // Check for product expressions with two children.
    if expr::expr_is_product(scip, e) && scip.exprs[e].children.len() == 2 && hd(scip, nlhdlr).exprmap.as_ref().is_none_or(|m| !m.contains_key(&e)) {
        let children = [scip.exprs[e].children[0], scip.exprs[e].children[1]];

        // Detection is only successful if both children will have auxiliary
        // variable or are variables that are not binary variables.
        let mut valid = true;
        for &c in &children {
            if crate::cons_nonlinear::get_expr_n_auxvar_uses_nonlinear(scip, c) == 0
                && (!expr::expr_is_var(scip, c) || crate::var::var_is_binary(scip, crate::expr_var::get_var_expr_var(scip, c)))
            {
                valid = false;
                break;
            }
        }

        if valid {
            // Create expression data for the nonlinear handler.
            *nlhdlrexprdata = scip.nlhdlrexprdatas.push(Box::new(ExprData {
                lastnodeid: -1,
                ..ExprData::default()
            }));

            // Create expression map, if not done so far.
            if hd(scip, nlhdlr).exprmap.is_none() {
                let nvars = crate::api::get_n_vars(scip) as usize;
                hdm(scip, nlhdlr).exprmap = Some(HashMap::with_capacity(nvars));
            }

            // Add expression to nlhdlrdata and capture it.
            expr::expr_capture(scip, e);
            let h = hdm(scip, nlhdlr);
            let pos = h.exprs.len();
            h.exprs.push(e);
            h.exprmap.as_mut().unwrap().insert(e, pos);

            // Tell children that we will use their auxvar and use its activity for
            // both estimate and domain propagation.
            let useact = hd(scip, nlhdlr).useinteval || hd(scip, nlhdlr).usereverseprop;
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, children[0], true, useact, true, true)?;
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, children[1], true, useact, true, true)?;
        }
    }

    if *nlhdlrexprdata != NONE {
        // We want to join separation and domain propagation, if not disabled by
        // parameter.
        *participating = NLHDLR_METHOD_SEPABOTH;

        if hd(scip, nlhdlr).useinteval || hd(scip, nlhdlr).usereverseprop {
            *participating |= NLHDLR_METHOD_ACTIVITY;
        }
    }

    Ok(())
}

/// `nlhdlrEvalauxBilinear`: auxiliary evaluation callback of nonlinear
/// handler.
fn nlhdlr_evalaux_bilinear(scip: &mut Scip, _nlhdlr: NlhdlrId, e: ExprId, _nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    let var1 = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[0]);
    let var2 = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[1]);
    let coef = crate::expr_product::get_coef_expr_product(scip, e);

    *auxvalue = coef * crate::api::get_sol_val(scip, sol, var1) * crate::api::get_sol_val(scip, sol, var2);

    Ok(())
}

/// `nlhdlrEstimateBilinear`: nonlinear handler under/overestimation
/// callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_estimate_bilinear(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    _auxvalue: f64,
    overestimate: bool,
    _targetvalue: f64,
    _addbranchscores: bool,
    rowpreps: &mut Vec<RowPrep>,
    success: &mut bool,
    addedbranchscores: &mut bool,
) -> ScipResult {
    let mut lincoefx = 0.0;
    let mut lincoefy = 0.0;
    let mut linconstant = 0.0;
    let mut mccsuccess = true;

    *success = false;
    *addedbranchscores = false;

    // Check whether an inequality is available.
    if ed(scip, nlhdlrexprdata).noverineqs == 0 && ed(scip, nlhdlrexprdata).nunderineqs == 0 {
        return Ok(());
    }

    let nodeid = scip.nodes[crate::tree::tree_get_current_node(scip)].number;

    // Update last node.
    let d = edm(scip, nlhdlrexprdata);
    if d.lastnodeid != nodeid {
        d.lastnodeid = nodeid;
        d.nseparoundslastnode = 0;
    }

    // Update separation round.
    d.nseparoundslastnode += 1;
    let nseparoundslastnode = d.nseparoundslastnode;

    // Check working limits.
    let depth = crate::api::get_depth(scip);
    let h = hd(scip, nlhdlr);
    if (depth == 0 && nseparoundslastnode > h.maxseparoundsroot) || (depth > 0 && nseparoundslastnode > h.maxseparounds) || depth > h.maxsepadepth {
        return Ok(());
    }

    // Collect variables.
    let x = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[0]);
    let y = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[1]);
    let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e);
    let (lbx, ubx) = (scip.vars[x].locdom.lb, scip.vars[x].locdom.ub);
    let (lby, uby) = (scip.vars[y].locdom.lb, scip.vars[y].locdom.ub);

    // Get and adjust the reference points.
    let refpointx = cmin(cmax(crate::api::get_sol_val(scip, sol, x), lbx), ubx);
    let refpointy = cmin(cmax(crate::api::get_sol_val(scip, sol, y), lby), uby);

    // Use McCormick inequalities to decide whether we want to separate or
    // not.
    let prodcoef = crate::expr_product::get_coef_expr_product(scip, e);
    add_bilin_mccormick(
        scip,
        prodcoef,
        lbx,
        ubx,
        refpointx,
        lby,
        uby,
        refpointy,
        overestimate,
        &mut lincoefx,
        &mut lincoefy,
        &mut linconstant,
        &mut mccsuccess,
    );

    // Too large values in McCormick inequalities -> skip.
    if !mccsuccess {
        return Ok(());
    }

    // Compute violation for the McCormick relaxation.
    let mut violation = lincoefx * refpointx + lincoefy * refpointy + linconstant - crate::api::get_sol_val(scip, sol, auxvar);
    if overestimate {
        violation = -violation;
    }

    // Only use a tighter relaxations if McCormick does not separate the
    // reference point.
    if scip.set.is_feas_le(violation, 0.0) && use_bilin_ineqs(scip, x, y, refpointx, refpointy) {
        let useoverestineq = if prodcoef > 0.0 { overestimate } else { !overestimate };
        let mccormickval = lincoefx * refpointx + lincoefy * refpointy + linconstant;

        // McCormick relaxation is too weak.
        let mut bestval = mccormickval;

        // Get the inequalities that might lead to a tighter relaxation.
        let (ineqs, nineqs) = {
            let d = ed(scip, nlhdlrexprdata);
            if useoverestineq { (d.overineqs, d.noverineqs) } else { (d.underineqs, d.nunderineqs) }
        };

        // Use linear inequalities to update relaxation.
        update_bilinear_relaxation(
            scip,
            x,
            y,
            prodcoef,
            if overestimate { SideType::Left } else { SideType::Right },
            refpointx,
            refpointy,
            &ineqs,
            nineqs as usize,
            mccormickval,
            &mut lincoefx,
            &mut lincoefy,
            &mut linconstant,
            &mut bestval,
            success,
        );
    }

    if *success {
        let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);
        rowprep.add_constant(linconstant);
        rowprep.ensure_size(2);
        rowprep.add_term(x, lincoefx);
        rowprep.add_term(y, lincoefy);
        rowpreps.push(rowprep);
    }

    Ok(())
}

/// `nlhdlrIntevalBilinear`: nonlinear handler interval evaluation callback.
fn nlhdlr_inteval_bilinear(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    iv: &mut Interval,
    _intevalvar: Option<ExprIntEvalVar>,
    _intevalvardata: usize,
) -> ScipResult {
    let d = ed(scip, nlhdlrexprdata).clone();
    if hd(scip, nlhdlr).useinteval && d.nunderineqs + d.noverineqs > 0 {
        let tmp = inteval_bilinear(scip, e, &d.underineqs, d.nunderineqs as usize, &d.overineqs, d.noverineqs as usize);

        // Intersect intervals if we have learned a tighter interval.
        if scip.set.is_gt(tmp.inf, iv.inf) || scip.set.is_lt(tmp.sup, iv.sup) {
            let b = *iv;
            interval::intersect(iv, b, tmp);
        }
    }

    Ok(())
}

/// `nlhdlrReversepropBilinear`: nonlinear handler callback for reverse
/// propagation.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_reverseprop_bilinear(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    bounds: Interval,
    infeasible: &mut bool,
    nreductions: &mut i32,
) -> ScipResult {
    let d = ed(scip, nlhdlrexprdata).clone();
    if hd(scip, nlhdlr).usereverseprop && d.nunderineqs + d.noverineqs > 0 {
        let mut intervalx = Interval::default();
        let mut intervaly = Interval::default();
        intervalx.set_entire(INTERVAL_INFINITY);
        intervaly.set_entire(INTERVAL_INFINITY);

        // Compute bounds on x and y.
        reverse_prop_bilinear(
            scip,
            e,
            bounds,
            &d.underineqs,
            d.nunderineqs as usize,
            &d.overineqs,
            d.noverineqs as usize,
            &mut intervalx,
            &mut intervaly,
        );

        // Tighten bounds of x.
        let c0 = scip.exprs[e].children[0];
        crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, c0, intervalx, infeasible, Some(&mut *nreductions))?;

        if !*infeasible {
            // Tighten bounds of y.
            let c1 = scip.exprs[e].children[1];
            crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, c1, intervaly, infeasible, Some(&mut *nreductions))?;
        }
    }

    Ok(())
}

/*
 * nonlinear handler specific interface methods
 */

/// `SCIPincludeNlhdlrBilinear`: includes bilinear nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_bilinear(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // Create nonlinear handler specific data.
    let nlhdlrdata = NlhdlrData {
        exprs: Vec::new(),
        exprmap: None,
        useinteval: false,
        usereverseprop: false,
        maxseparoundsroot: 0,
        maxseparounds: 0,
        maxsepadepth: 0,
    };

    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_bilinear,
        nlhdlr_evalaux_bilinear,
        Some(Box::new(nlhdlrdata)),
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_bilinear));
    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_freehdlrdata_bilinear));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_bilinear));
    nlhdlr::nlhdlr_set_init_exit(scip, nlhdlr, None, Some(nlhdlr_exit_bilinear));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, None, None, Some(nlhdlr_estimate_bilinear), None);
    nlhdlr::nlhdlr_set_prop(scip, nlhdlr, Some(nlhdlr_inteval_bilinear), Some(nlhdlr_reverseprop_bilinear));

    // Parameters.
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, nlhdlrs, nlhdlr, data, NlhdlrData, $f)
        };
    }
    paramset_add_bool(
        scip,
        "nlhdlr/bilinear/useinteval",
        "whether to use the interval evaluation callback of the nlhdlr",
        t!(bool, useinteval),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/bilinear/usereverseprop",
        "whether to use the reverse propagation callback of the nlhdlr",
        t!(bool, usereverseprop),
        false,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/bilinear/maxseparoundsroot",
        "maximum number of separation rounds in the root node",
        t!(i32, maxseparoundsroot),
        false,
        10,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/bilinear/maxseparounds",
        "maximum number of separation rounds in a local node",
        t!(i32, maxseparounds),
        false,
        1,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/bilinear/maxsepadepth",
        "maximum depth to apply separation",
        t!(i32, maxsepadepth),
        false,
        i32::MAX,
        0,
        i32::MAX,
        None,
    )?;

    // (The statistics table is not ported.)

    Ok(())
}

/// `SCIPgetExprsBilinear`: returns an array of expressions that have been
/// detected by the bilinear nonlinear handler.
pub fn get_exprs_bilinear(scip: &Scip, nlhdlr: NlhdlrId) -> &[ExprId] {
    &hd(scip, nlhdlr).exprs
}

/// `SCIPgetNExprsBilinear`: returns the total number of expressions that
/// have been detected by the bilinear nonlinear handler.
pub fn get_n_exprs_bilinear(scip: &Scip, nlhdlr: NlhdlrId) -> usize {
    hd(scip, nlhdlr).exprs.len()
}

/// `SCIPaddIneqBilinear`: adds a globally valid inequality of the form
/// xcoef * x <= ycoef * y + constant to a product expression of the form
/// x * y; returns whether the inequality has been accepted.
pub fn add_ineq_bilinear(scip: &mut Scip, nlhdlr: NlhdlrId, e: ExprId, mut xcoef: f64, mut ycoef: f64, mut constant: f64) -> ScipResult<bool> {
    let mut success = false;

    // Find nonlinear handler expression handler data.
    let nlhdlrexprdata = crate::cons_nonlinear::get_nlhdlr_expr_data_nonlinear(scip, nlhdlr, e);
    if nlhdlrexprdata == NONE {
        log::warn!("nonlinear expression data has not been found. Skip SCIPaddConsExprExprProductBilinearIneq()");
        return Ok(false);
    }

    // Ignore inequalities that only yield to a (possible) bound tightening.
    if scip.set.is_feas_zero(xcoef) || scip.set.is_feas_zero(ycoef) {
        return Ok(false);
    }

    // Collect variables.
    let x = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[0]);
    let y = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[1]);

    // Normalize inequality s.t. xcoef in {-1,1}.
    if !scip.set.is_eq(xcoef.abs(), 1.0) {
        constant /= xcoef.abs();
        ycoef /= xcoef.abs();
        xcoef /= xcoef.abs();
    }

    // Coefficients of the inequality determine whether the inequality can be
    // used for under- or overestimation.
    let underestimate = xcoef * ycoef > 0.0;

    // Compute violation of the inequality of the important corner points.
    let (viol1, viol2) = get_ineq_viol(scip, x, y, xcoef, ycoef, constant);

    // Inequality does not cutoff one of the important corner points -> skip.
    if scip.set.is_feas_le(cmax(viol1, viol2), 0.0) {
        return Ok(false);
    }

    let (mut ineqs, nineqs) = {
        let d = ed(scip, nlhdlrexprdata);
        if underestimate {
            (d.underineqs, d.nunderineqs as usize)
        } else {
            (d.overineqs, d.noverineqs as usize)
        }
    };

    // Check for a duplicate.
    for i in 0..nineqs {
        if scip.set.is_feas_eq(xcoef, ineqs[3 * i]) && scip.set.is_feas_eq(ycoef, ineqs[3 * i + 1]) && scip.set.is_feas_eq(constant, ineqs[3 * i + 2]) {
            return Ok(false);
        }
    }

    // Compute violations of existing inequalities.
    for i in 0..nineqs {
        let (ineqviol1, ineqviol2) = get_ineq_viol(scip, x, y, ineqs[3 * i], ineqs[3 * i + 1], ineqs[3 * i + 2]);

        // Check whether an existing inequality is dominating the candidate.
        if scip.set.is_ge(ineqviol1, viol1) && scip.set.is_ge(ineqviol2, viol2) {
            // (The C returns here; replacements made above stay.)
            store_ineqs(scip, nlhdlrexprdata, underestimate, &ineqs);
            return Ok(success);
        }

        // Replace inequality if candidate is dominating it.
        if scip.set.is_lt(ineqviol1, viol1) && scip.set.is_lt(ineqviol2, viol2) {
            ineqs[3 * i] = xcoef;
            ineqs[3 * i + 1] = ycoef;
            ineqs[3 * i + 2] = constant;
            success = true;
        }
    }

    // Inequality is not dominated by other inequalities -> add if we have less
    // than 2 inequalities.
    if nineqs < 2 {
        ineqs[3 * nineqs] = xcoef;
        ineqs[3 * nineqs + 1] = ycoef;
        ineqs[3 * nineqs + 2] = constant;
        success = true;

        // Increase number of inequalities.
        let d = edm(scip, nlhdlrexprdata);
        if underestimate {
            d.nunderineqs += 1;
        } else {
            d.noverineqs += 1;
        }
    }

    store_ineqs(scip, nlhdlrexprdata, underestimate, &ineqs);

    if success {
        // With the added inequalities, we can potentially compute tighter
        // activities for the expression, so constraints that contain this
        // expression should be propagated again. We don't have a direct
        // expression to constraint mapping, though. This call marks all
        // expr-constraints which include any of the variables that this
        // expression depends on for propagation.
        crate::cons_nonlinear::mark_expr_propagate_nonlinear(scip, e)?;
    }

    Ok(success)
}

/// Writes the inequalities `SCIPaddIneqBilinear` edits through its pointer
/// back to the expression data.
fn store_ineqs(scip: &mut Scip, nlhdlrexprdata: usize, underestimate: bool, ineqs: &[f64; 6]) {
    let d = edm(scip, nlhdlrexprdata);
    if underestimate {
        d.underineqs = *ineqs;
    } else {
        d.overineqs = *ineqs;
    }
}

/// `SCIPaddBilinLinearization`: computes coefficients of linearization of a
/// bilinear term in a reference point.
#[allow(clippy::too_many_arguments)]
pub fn add_bilin_linearization(scip: &Scip, bilincoef: f64, refpointx: f64, refpointy: f64, lincoefx: &mut f64, lincoefy: &mut f64, linconstant: &mut f64, success: &mut bool) {
    if bilincoef == 0.0 {
        return;
    }

    if scip.set.is_infinity(refpointx.abs()) || scip.set.is_infinity(refpointy.abs()) {
        *success = false;
        return;
    }

    // bilincoef * x * y -> bilincoef * (refpointx * refpointy + refpointy * (x
    // - refpointx) + refpointx * (y - refpointy)) = -bilincoef * refpointx *
    // refpointy + bilincoef * refpointy * x + bilincoef * refpointx * y
    let constant = -bilincoef * refpointx * refpointy;

    if scip.set.is_infinity((bilincoef * refpointx).abs()) || scip.set.is_infinity((bilincoef * refpointy).abs()) || scip.set.is_infinity(constant.abs()) {
        *success = false;
        return;
    }

    *lincoefx += bilincoef * refpointy;
    *lincoefy += bilincoef * refpointx;
    *linconstant += constant;
}

/// `SCIPaddBilinMcCormick`: computes coefficients of linearization of a
/// bilinear term in a reference point.
#[allow(clippy::too_many_arguments)]
pub fn add_bilin_mccormick(
    scip: &Scip,
    mut bilincoef: f64,
    lbx: f64,
    ubx: f64,
    refpointx: f64,
    lby: f64,
    uby: f64,
    refpointy: f64,
    overestimate: bool,
    lincoefx: &mut f64,
    lincoefy: &mut f64,
    linconstant: &mut f64,
    success: &mut bool,
) {
    let set = &scip.set;
    let mut constant;
    let mut coefx;
    let mut coefy;

    if bilincoef == 0.0 {
        return;
    }

    if overestimate {
        bilincoef = -bilincoef;
    }

    if set.is_rel_eq(lbx, ubx) && set.is_rel_eq(lby, uby) {
        // Both x and y are mostly fixed.
        coefx = 0.0;
        coefy = 0.0;

        // Estimate x * y by constant.
        let cand1 = lbx * lby;
        let cand2 = lbx * uby;
        let cand3 = ubx * lby;
        let cand4 = ubx * uby;

        // Take most conservative value for underestimator.
        if bilincoef < 0.0 {
            constant = bilincoef * cmax(cmax(cand1, cand2), cmax(cand3, cand4));
        } else {
            constant = bilincoef * cmin(cmin(cand1, cand2), cmin(cand3, cand4));
        }
    } else if bilincoef > 0.0 {
        // Either x or y is not fixed and coef > 0.0.
        if !set.is_infinity(-lbx)
            && !set.is_infinity(-lby)
            && (set.is_infinity(ubx) || set.is_infinity(uby) || (uby - refpointy) * (ubx - refpointx) >= (refpointy - lby) * (refpointx - lbx))
        {
            if set.is_rel_eq(lbx, ubx) {
                // x*y = lbx * y + (x-lbx) * y >= lbx * y + (x-lbx) * lby >= lbx * y +
                // min{(ubx-lbx) * lby, 0 * lby}.
                coefx = 0.0;
                coefy = bilincoef * lbx;
                constant = bilincoef * if lby < 0.0 { (ubx - lbx) * lby } else { 0.0 };
            } else if set.is_rel_eq(lby, uby) {
                // x*y = lby * x + (y-lby) * x >= lby * x + (y-lby) * lbx >= lby * x +
                // min{(uby-lby) * lbx, 0 * lbx}.
                coefx = bilincoef * lby;
                coefy = 0.0;
                constant = bilincoef * if lbx < 0.0 { (uby - lby) * lbx } else { 0.0 };
            } else {
                coefx = bilincoef * lby;
                coefy = bilincoef * lbx;
                constant = -bilincoef * lbx * lby;
            }
        } else if !set.is_infinity(ubx) && !set.is_infinity(uby) {
            if set.is_rel_eq(lbx, ubx) {
                // x*y = ubx * y + (x-ubx) * y >= ubx * y + (x-ubx) * uby >= ubx * y +
                // min{(lbx-ubx) * uby, 0 * uby}.
                coefx = 0.0;
                coefy = bilincoef * ubx;
                constant = bilincoef * if uby > 0.0 { (lbx - ubx) * uby } else { 0.0 };
            } else if set.is_rel_eq(lby, uby) {
                // x*y = uby * x + (y-uby) * x >= uby * x + (y-uby) * ubx >= uby * x +
                // min{(lby-uby) * ubx, 0 * ubx}.
                coefx = bilincoef * uby;
                coefy = 0.0;
                constant = bilincoef * if ubx > 0.0 { (lby - uby) * ubx } else { 0.0 };
            } else {
                coefx = bilincoef * uby;
                coefy = bilincoef * ubx;
                constant = -bilincoef * ubx * uby;
            }
        } else {
            *success = false;
            return;
        }
    } else {
        // Either x or y is not fixed and coef < 0.0.
        if !set.is_infinity(ubx) && !set.is_infinity(-lby) && (set.is_infinity(-lbx) || set.is_infinity(uby) || (ubx - lbx) * (refpointy - lby) <= (uby - lby) * (refpointx - lbx))
        {
            if set.is_rel_eq(lbx, ubx) {
                // x*y = ubx * y + (x-ubx) * y <= ubx * y + (x-ubx) * lby <= ubx * y +
                // max{(lbx-ubx) * lby, 0 * lby}.
                coefx = 0.0;
                coefy = bilincoef * ubx;
                constant = bilincoef * if lby < 0.0 { (lbx - ubx) * lby } else { 0.0 };
            } else if set.is_rel_eq(lby, uby) {
                // x*y = lby * x + (y-lby) * x <= lby * x + (y-lby) * ubx <= lby * x +
                // max{(uby-lby) * ubx, 0 * ubx}.
                coefx = bilincoef * lby;
                coefy = 0.0;
                constant = bilincoef * if ubx > 0.0 { (uby - lby) * ubx } else { 0.0 };
            } else {
                coefx = bilincoef * lby;
                coefy = bilincoef * ubx;
                constant = -bilincoef * ubx * lby;
            }
        } else if !set.is_infinity(-lbx) && !set.is_infinity(uby) {
            if set.is_rel_eq(lbx, ubx) {
                // x*y = lbx * y + (x-lbx) * y <= lbx * y + (x-lbx) * uby <= lbx * y +
                // max{(ubx-lbx) * uby, 0 * uby}.
                coefx = 0.0;
                coefy = bilincoef * lbx;
                constant = bilincoef * if uby > 0.0 { (ubx - lbx) * uby } else { 0.0 };
            } else if set.is_rel_eq(lby, uby) {
                // x*y = uby * x + (y-uby) * x <= uby * x + (y-uby) * lbx <= uby * x +
                // max{(lby-uby) * lbx, 0 * lbx}.
                coefx = bilincoef * uby;
                coefy = 0.0;
                constant = bilincoef * if lbx < 0.0 { (lby - uby) * lbx } else { 0.0 };
            } else {
                coefx = bilincoef * uby;
                coefy = bilincoef * lbx;
                constant = -bilincoef * lbx * uby;
            }
        } else {
            *success = false;
            return;
        }
    }

    if set.is_infinity(coefx.abs()) || set.is_infinity(coefy.abs()) || set.is_infinity(constant.abs()) {
        *success = false;
        return;
    }

    if overestimate {
        coefx = -coefx;
        coefy = -coefy;
        constant = -constant;
    }

    *lincoefx += coefx;
    *lincoefy += coefy;
    *linconstant += constant;
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

/// `SCIPcomputeBilinEnvelope1`: computes coefficients of linearization of a
/// bilinear term in a reference point when given a linear inequality
/// involving only the variables of the bilinear term.
///
/// The formulas are extracted from "Convex envelopes of bivariate functions
/// through the solution of KKT systems" by Marco Locatelli.
#[allow(clippy::too_many_arguments)]
pub fn compute_bilin_envelope1(
    scip: &Scip,
    bilincoef: f64,
    lbx: f64,
    ubx: f64,
    refpointx: f64,
    lby: f64,
    uby: f64,
    refpointy: f64,
    mut overestimate: bool,
    xcoef: f64,
    ycoef: f64,
    constant: f64,
    lincoefx: &mut f64,
    lincoefy: &mut f64,
    linconstant: &mut f64,
    success: &mut bool,
) {
    let set = &scip.set;
    let xs = [lbx, ubx];
    let mut ys = [lby, uby];

    *success = false;
    *lincoefx = INVALID;
    *lincoefy = INVALID;
    *linconstant = INVALID;

    // Reference point does not satisfy linear inequality.
    if set.is_feas_gt(xcoef * refpointx - ycoef * refpointy - constant, 0.0) {
        return;
    }

    // Compute minimal and maximal bounds on x and y for accepting the reference
    // point.
    let minx = lbx + 0.01 * (ubx - lbx);
    let maxx = ubx - 0.01 * (ubx - lbx);
    let miny = lby + 0.01 * (uby - lby);
    let maxy = uby - 0.01 * (uby - lby);

    // Check whether the reference point is in [minx,maxx]x[miny,maxy].
    if set.is_le(refpointx, minx) || set.is_ge(refpointx, maxx) || set.is_le(refpointy, miny) || set.is_ge(refpointy, maxy) {
        return;
    }

    // Always consider xy without the bilinear coefficient.
    if bilincoef < 0.0 {
        overestimate = !overestimate;
    }

    // We use same notation as in "Convex envelopes of bivariate functions
    // through the solution of KKT systems", 2016.
    // mj = xcoef / ycoef
    let mjq = quad::div_dd(xcoef, ycoef);

    // qj = -constant / ycoef
    let qjq = quad::div_dd(-constant, ycoef);

    // mj > 0 => underestimate; mj < 0 => overestimate.
    if set.is_negative(mjq.to_dbl()) != overestimate {
        return;
    }

    // Get the corner point that satisfies the linear inequality xcoef*x <=
    // ycoef*y + constant.
    if !overestimate {
        ys[0] = uby;
        ys[1] = lby;
    }

    let mut vx = INVALID;
    let mut vy = INVALID;
    let mut n = 0;
    for i in 0..2 {
        let activity = xcoef * xs[i] - ycoef * ys[i] - constant;
        if set.is_le(activity, 0.0) {
            // Corner point is satisfies inequality.
            vx = xs[i];
            vy = ys[i];
        } else if set.is_feas_gt(activity, 0.0) {
            // Corner point is clearly cut off.
            n += 1;
        }
    }

    // Skip if no corner point satisfies the inequality or if no corner point is
    // cut off (that is, all corner points satisfy the inequality almost
    // [1e-9..1e-6]).
    if n != 1 || vx == INVALID || vy == INVALID {
        return;
    }

    // denom = mj*(refpointx - vx) + vy - refpointy
    // refpoint - vx
    let mut denomq = quad::sum_dd(refpointx, -vx);
    // mj * (refpoint - vx)
    denomq = quad::prod_qq(denomq, mjq);
    // mj * (refpoint - vx) + vy
    denomq = quad::sum_qd(denomq, vy);
    // mj * (refpoint - vx) + vy - refpointy
    denomq = quad::sum_qd(denomq, -refpointy);

    if set.is_zero(denomq.to_dbl()) {
        return;
    }

    // (xj,yj) is the projection onto the line xcoef*x = ycoef*y + constant.
    // xj = (refpointx*(vy - qj) - vx*(refpointy - qj)) / denom
    // - qj
    let mut xjq = quad::prod_qd(qjq, -1.0);
    // vy - qj
    xjq = quad::sum_qd(xjq, vy);
    // refpointx * (vy - qj)
    xjq = quad::prod_qd(xjq, refpointx);
    // - qj
    let mut tmpq = quad::prod_qd(qjq, -1.0);
    // refpointy - qj
    tmpq = quad::sum_qd(tmpq, refpointy);
    // - vx * (refpointy - qj)
    tmpq = quad::prod_qd(tmpq, -vx);
    // refpointx * (vy - qj) - vx * (refpointy - qj)
    xjq = quad::sum_qq(xjq, tmpq);
    // (refpointx * (vy - qj) - vx * (refpointy - qj)) / denom
    xjq = quad::div_qq(xjq, denomq);

    // yj = mj * xj + qj
    let mut yjq = quad::prod_qq(mjq, xjq);
    yjq = quad::sum_qq(yjq, qjq);

    // Check whether the projection is in [minx,maxx] x [miny,maxy]; this
    // avoids numerical difficulties when the projection is close to the
    // variable bounds.
    if set.is_le(xjq.to_dbl(), minx) || set.is_ge(xjq.to_dbl(), maxx) || set.is_le(yjq.to_dbl(), miny) || set.is_ge(yjq.to_dbl(), maxy) {
        return;
    }

    // lincoefy = (mj*SQR(xj) - 2.0*mj*vx*xj - qj*vx + vx*vy) / (vy - mj*vx -
    // qj)
    // xj^2
    let mut lincoefyq = quad::square_q(xjq);
    // mj * xj^2
    lincoefyq = quad::prod_qq(lincoefyq, mjq);
    // mj * xj
    let mut tmpq = quad::prod_qq(mjq, xjq);
    // -2 * vx * mj * xj
    tmpq = quad::prod_qd(tmpq, -2.0 * vx);
    // mj * xj^2 -2 * vx * mj * xj
    lincoefyq = quad::sum_qq(lincoefyq, tmpq);
    // -qj * vx
    let tmpq = quad::prod_qd(qjq, -vx);
    // mj * xj^2 -2 * vx * mj * xj -qj * vx
    lincoefyq = quad::sum_qq(lincoefyq, tmpq);
    // vx * vy
    let tmpq = quad::prod_dd(vx, vy);
    // mj * xj^2 -2 * vx * mj * xj -qj * vx + vx * vy
    lincoefyq = quad::sum_qq(lincoefyq, tmpq);
    // mj * vx
    let mut tmpq = quad::prod_qd(mjq, vx);
    // -vy + mj * vx
    tmpq = quad::sum_qd(tmpq, -vy);
    // -vy + mj * vx + qj
    tmpq = quad::sum_qq(tmpq, qjq);
    // vy - mj * vx - qj
    tmpq.scale(-1.0);
    // (mj * xj^2 -2 * vx * mj * xj -qj * vx + vx * vy) / (vy - mj * vx - qj)
    lincoefyq = quad::div_qq(lincoefyq, tmpq);

    // lincoefx = 2.0*mj*xj + qj - mj*(*lincoefy)
    // mj * xj
    let mut lincoefxq = quad::prod_qq(mjq, xjq);
    // 2 * mj * xj
    lincoefxq.scale(2.0);
    // 2 * mj * xj + qj
    lincoefxq = quad::sum_qq(lincoefxq, qjq);
    // mj * lincoefy
    let mut tmpq = quad::prod_qq(mjq, lincoefyq);
    // - mj * lincoefy
    tmpq.scale(-1.0);
    // 2 * mj * xj + qj - mj * lincoefy
    lincoefxq = quad::sum_qq(lincoefxq, tmpq);

    // linconstant = -mj*SQR(xj) - (*lincoefy)*qj
    // xj^2
    let mut linconstantq = quad::square_q(xjq);
    // mj * xj^2
    linconstantq = quad::prod_qq(linconstantq, mjq);
    // - mj * xj^2
    linconstantq.scale(-1.0);
    // lincoefy * qj
    let mut tmpq = quad::prod_qq(lincoefyq, qjq);
    // - lincoefy * qj
    tmpq.scale(-1.0);
    // - mj * xj^2 - lincoefy * qj
    linconstantq = quad::sum_qq(linconstantq, tmpq);

    // Consider the bilinear coefficient.
    lincoefxq = quad::prod_qd(lincoefxq, bilincoef);
    lincoefyq = quad::prod_qd(lincoefyq, bilincoef);
    linconstantq = quad::prod_qd(linconstantq, bilincoef);
    *lincoefx = lincoefxq.to_dbl();
    *lincoefy = lincoefyq.to_dbl();
    *linconstant = linconstantq.to_dbl();

    // Cut needs to be tight at (vx,vy) and (xj,yj); otherwise we consider the
    // cut to be numerically bad.
    *success = set.is_feas_eq((*lincoefx) * vx + (*lincoefy) * vy + (*linconstant), bilincoef * vx * vy)
        && set.is_feas_eq(
            (*lincoefx) * xjq.to_dbl() + (*lincoefy) * yjq.to_dbl() + (*linconstant),
            bilincoef * xjq.to_dbl() * yjq.to_dbl(),
        );
}

/// `SCIPcomputeBilinEnvelope2`: computes coefficients of linearization of a
/// bilinear term in a reference point when given two linear inequalities
/// involving only the variables of the bilinear term.
///
/// The formulas are extracted from "Convex envelopes of bivariate functions
/// through the solution of KKT systems" by Marco Locatelli.
#[allow(clippy::too_many_arguments)]
pub fn compute_bilin_envelope2(
    scip: &Scip,
    bilincoef: f64,
    lbx: f64,
    ubx: f64,
    refpointx: f64,
    lby: f64,
    uby: f64,
    refpointy: f64,
    mut overestimate: bool,
    xcoef1: f64,
    ycoef1: f64,
    constant1: f64,
    xcoef2: f64,
    ycoef2: f64,
    constant2: f64,
    lincoefx: &mut f64,
    lincoefy: &mut f64,
    linconstant: &mut f64,
    success: &mut bool,
) {
    let set = &scip.set;

    *success = false;
    *lincoefx = INVALID;
    *lincoefy = INVALID;
    *linconstant = INVALID;

    // Reference point does not satisfy linear inequalities.
    if set.is_feas_gt(xcoef1 * refpointx - ycoef1 * refpointy - constant1, 0.0) || set.is_feas_gt(xcoef2 * refpointx - ycoef2 * refpointy - constant2, 0.0) {
        return;
    }

    // Compute minimal and maximal bounds on x and y for accepting the reference
    // point.
    let minx = lbx + 0.01 * (ubx - lbx);
    let maxx = ubx - 0.01 * (ubx - lbx);
    let miny = lby + 0.01 * (uby - lby);
    let maxy = uby - 0.01 * (uby - lby);

    // Check the reference point is in the interior of the domain.
    if set.is_le(refpointx, minx) || set.is_ge(refpointx, maxx) || set.is_le(refpointy, miny) || set.is_feas_ge(refpointy, maxy) {
        return;
    }

    // The sign of the x-coefficients of the two inequalities must be
    // different; otherwise the convex or concave envelope can be computed via
    // SCIPcomputeBilinEnvelope1 for each inequality separately.
    if (xcoef1 > 0.0) == (xcoef2 > 0.0) {
        return;
    }

    // Always consider xy without the bilinear coefficient.
    if bilincoef < 0.0 {
        overestimate = !overestimate;
    }

    // We use same notation as in "Convex envelopes of bivariate functions
    // through the solution of KKT systems", 2016.
    let mi = xcoef1 / ycoef1;
    let qi = -constant1 / ycoef1;
    let mj = xcoef2 / ycoef2;
    let qj = -constant2 / ycoef2;

    // mi, mj > 0 => underestimate; mi, mj < 0 => overestimate.
    if set.is_negative(mi) != overestimate || set.is_negative(mj) != overestimate {
        return;
    }

    // Compute cut according to Locatelli 2016.
    let (mut xi, mut yi, mut xj, mut yj) = (0.0, 0.0, 0.0, 0.0);
    let (mut xcoef, mut ycoef, mut constant) = (0.0, 0.0, 0.0);
    compute_bilin_envelope2_inner(
        scip,
        refpointx,
        refpointy,
        mi,
        qi,
        mj,
        qj,
        &mut xi,
        &mut yi,
        &mut xj,
        &mut yj,
        &mut xcoef,
        &mut ycoef,
        &mut constant,
    );

    // It might happen that (xi,yi) = (xj,yj) if the two lines intersect.
    if set.is_eq(xi, xj) && set.is_eq(yi, yj) {
        return;
    }

    // Check whether projected points are in the interior.
    if set.is_le(xi, minx) || set.is_ge(xi, maxx) || set.is_le(yi, miny) || set.is_ge(yi, maxy) {
        return;
    }
    if set.is_le(xj, minx) || set.is_ge(xj, maxx) || set.is_le(yj, miny) || set.is_ge(yj, maxy) {
        return;
    }

    *lincoefx = bilincoef * xcoef;
    *lincoefy = bilincoef * ycoef;
    *linconstant = bilincoef * constant;

    // Cut needs to be tight at (vx,vy) and (xj,yj).
    *success = set.is_feas_eq((*lincoefx) * xi + (*lincoefy) * yi + (*linconstant), bilincoef * xi * yi)
        && set.is_feas_eq((*lincoefx) * xj + (*lincoefy) * yj + (*linconstant), bilincoef * xj * yj);
}
