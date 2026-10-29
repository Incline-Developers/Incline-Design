//! Port of `nlhdlr_signomial.c`: nonlinear handler for signomial
//! expressions.

use crate::{
    def::INVALID,
    expr::{self, ExprId},
    interval::Interval,
    nlhdlr::{self, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, SideType, SolId, VarId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "signomial";
const NLHDLR_DESC: &str = "handler for signomial expressions";
const NLHDLR_DETECTPRIORITY: i32 = 30;
const NLHDLR_ENFOPRIORITY: i32 = 30;

/* handler specific parameters */
/// Maximum number of variables when underestimating a concave power
/// function (maximum: 14).
const NLHDLR_MAXNUNDERVARS: i32 = 14;
/// Minimum scale factor when scaling a cut (minimum: 1e-6).
const NLHDLR_MINCUTSCALE: f64 = 1e-5;

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
///
/// A signomial expression admits the form c x^a = y, where y is an
/// auxiliary variable representing this expression and all x are positive.
///
/// The natural formulation of the expression is defined as x^a = t = y/c,
/// where t is a non-existant slack variable denoting y/c. The variables in x
/// with positive exponents form positive variables u, and the associated
/// exponents form positive exponents f. The variables in x with negative
/// exponents and t form negative variables v, and the associated exponents
/// form negative exponents g. Let s = max(|f|,|g|) be a normalization
/// constant, where |.| denotes the L1 norm. Apply a scaling step: Dividing
/// the entries of f by s, and dividing the entries of g by s as well. Then
/// x^a = t has a reformulation u^f = v^g, where u^f, v^g are two concave
/// power functions.
struct ExprData {
    /// Coefficient c.
    coef: f64,
    /// Expression factors representing x.
    factors: Vec<ExprId>,
    /// Number of factors.
    nfactors: usize,
    /// Number of variables (x,y).
    nvars: usize,
    /// Exponents e.
    exponents: Vec<f64>,
    /// Number of positive variables u.
    nposvars: i32,
    /// Number of negative variables v.
    nnegvars: i32,
    /// Indicators for sign of variables after reformulation, TRUE for
    /// positive, FALSE for negative.
    signs: Vec<bool>,
    /// Exponents of (x,t) after reformulation (always positive).
    refexponents: Vec<f64>,
    /// Are all variables already got?
    isstorecapture: bool,

    /* working parameters will be modified after getting all variables */
    /// Variables (x,y).
    vars: Vec<VarId>,
    /// Intervals storing lower and upper bounds of variables (x,y).
    intervals: Vec<Interval>,
    /// The upper/lower bounds of variables, used in
    /// SCIPcomputeFacetVertexPolyhedralNonlinear().
    boxv: Vec<f64>,
    /// The values of variables, used in
    /// SCIPcomputeFacetVertexPolyhedralNonlinear().
    xstar: Vec<f64>,
    /// The coefficients of variables, returned by
    /// SCIPcomputeFacetVertexPolyhedralNonlinear().
    facetcoefs: Vec<f64>,
}

/// `SCIP_NLHDLRDATA`: nonlinear handler data.
struct NlhdlrData {
    /* parameters */
    /// Maximum number of variables in underestimating a concave power
    /// function.
    maxnundervars: i32,
    /// Minimum scale factor when scaling a cut.
    mincutscale: f64,
}

#[inline]
fn ed(scip: &Scip, nlhdlrexprdata: usize) -> &ExprData {
    nlhdlr::exprdata::<ExprData>(scip, nlhdlrexprdata)
}

#[inline]
fn edm(scip: &mut Scip, nlhdlrexprdata: usize) -> &mut ExprData {
    nlhdlr::exprdata_mut::<ExprData>(scip, nlhdlrexprdata)
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
 * Local methods
 */

/// `reformRowprep`: reforms a rowprep to a standard form for nonlinear
/// handlers.
///
/// The input rowprep is of the form a_u u + a_v v + b. If in the
/// overestimate mode, then we relax t <= x^a, i.e., 0 <= u^f - v^g. This
/// implies that t is in v = (v',t). Therefore, the valid inequality is 0 <=
/// a_u u + a_v v + b. As overestimate mode requires that t is in the left
/// hand side, the coefficients of t must be made negative while keeping the
/// sign of the inequality, we can show that a_t <= 0, so it suffices to
/// divide the both sides by -a_t >= 0, which yields t <= (a_u u + a_v' v' +
/// b) / -a_t. A rowprep in standard form only contains an estimator of the
/// expression and no auxvar. If in the underestimate mode, then we relax x^a
/// <= t, i.e., u^f - v^g <= 0, and similarly obtain (a_u u + a_v' v' + b) /
/// -a_t <= t.
fn reform_rowprep(scip: &Scip, d: &ExprData, rowprep: &mut RowPrep, mincutscale: f64, success: &mut bool) {
    let nvars = rowprep.get_n_vars();

    // Find auxvar's cut coefficient and set it to zero.
    let auxvar = d.vars[d.nfactors];
    let mut coefauxvar = 1.0;
    for i in 0..nvars {
        if rowprep.vars[i] == auxvar {
            coefauxvar = rowprep.coefs[i];
            rowprep.coefs[i] = 0.0;
            break;
        }
    }

    if scip.set.is_zero(coefauxvar) || coefauxvar >= 0.0 {
        *success = false;
    } else {
        // The reformation scales the cut so that coefficients and constant are
        // divided by the absolute value of coefauxvar.
        let scale = -1.0 / coefauxvar;
        for i in 0..nvars {
            if rowprep.vars[i] == auxvar {
                continue;
            }
            rowprep.coefs[i] *= scale;
        }

        // Set the side to scale*side by adding -side and adding scale*side.
        let side = rowprep.side;
        rowprep.add_side(side * (-1.0 + scale));

        *success = scip.set.is_gt(scale, mincutscale);
    }
}

/// `storeCaptureVars`: store and capture variables associated with the
/// expression and its subexpressions.
fn store_capture_vars(scip: &mut Scip, e: ExprId, nlhdlrexprdata: usize) {
    // Get and capture variables x.
    let nfactors = ed(scip, nlhdlrexprdata).nfactors;
    for c in 0..nfactors {
        let factor = ed(scip, nlhdlrexprdata).factors[c];
        let var = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, factor);
        edm(scip, nlhdlrexprdata).vars[c] = var;
        crate::var::var_capture(scip, var);
    }

    // Get and capture variable y.
    let var = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e);
    edm(scip, nlhdlrexprdata).vars[nfactors] = var;
    crate::var::var_capture(scip, var);

    edm(scip, nlhdlrexprdata).isstorecapture = true;
}

/// `checkSignomialBounds`: get bounds of variables x,t and check whether
/// they are box constrained signomial variables.
fn check_signomial_bounds(scip: &mut Scip, nlhdlrexprdata: usize) -> bool {
    let mut productinf = 1.0;
    let mut productsup = 1.0;

    // Get bounds of x.
    let nfactors = ed(scip, nlhdlrexprdata).nfactors;
    for c in 0..nfactors {
        let var = ed(scip, nlhdlrexprdata).vars[c];
        let inf = scip.vars[var].locdom.lb;
        let sup = scip.vars[var].locdom.ub;

        // If the bounds of the variable are not positive and finite, or (bounds
        // equal) then the expression is not a signomial.
        if !scip.set.is_positive(inf) || !scip.set.is_positive(sup) || scip.set.is_infinity(sup) || scip.set.is_eq(inf, sup) {
            return false;
        }

        let d = edm(scip, nlhdlrexprdata);
        d.intervals[c].inf = inf;
        d.intervals[c].sup = cmax(sup, inf + 0.1);
        let powinf = inf.powf(d.exponents[c]);
        let powsup = sup.powf(d.exponents[c]);
        productinf *= cmin(powinf, powsup);
        productsup *= cmax(powinf, powsup);
    }

    // Compute bounds of t by bounds of x.
    let d = edm(scip, nlhdlrexprdata);
    d.intervals[nfactors].inf = productinf;
    d.intervals[nfactors].sup = productsup;

    true
}

/// `nlhdlrExprEvalPower`: evaluate expression at solution w.r.t. auxiliary
/// variables.
fn nlhdlr_expr_eval_power(signs: &[bool], refexponents: &[f64], sign: bool, args: &[f64]) -> f64 {
    let mut val = 1.0;
    let mut j = 0;
    for i in 0..signs.len() {
        if signs[i] != sign {
            continue;
        }

        // The reformulated exponent of args[j] is found.
        val *= args[j].powf(refexponents[i]);
        j += 1;
    }

    val
}

/// `estimateSpecialPower`: determine whether a power function w^h is
/// special and add an overunderestimator or underestimator to a given
/// rowprep.
///
/// w^h is special, if all variables are fixed, or it is a constant to
/// estimate, a univariate power to estimate, or a bivariate power to
/// underestimate. The estimator is multiplied by the multiplier and stored
/// in the rowprep.
#[allow(clippy::too_many_arguments)]
fn estimate_special_power(scip: &Scip, d: &ExprData, sign: bool, multiplier: f64, overestimate: bool, sol: SolId, rowprep: &mut RowPrep, isspecial: &mut bool, success: &mut bool) {
    let mut allfixed = true;

    *success = false;

    // Check whether all variables are fixed.
    for i in 0..d.nvars {
        if d.signs[i] != sign {
            continue;
        }

        if !scip.set.is_rel_eq(d.intervals[i].inf, d.intervals[i].sup) {
            allfixed = false;
            break;
        }
    }

    // Allfixed is a special case.
    if allfixed {
        // Return a constant.
        let mut funcval = 1.0;

        for i in 0..d.nvars {
            if d.signs[i] != sign {
                continue;
            }

            let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
            let val = crate::api::get_sol_val(scip, sol, d.vars[i]) / scale;
            funcval *= val.powf(d.refexponents[i]);
        }
        rowprep.add_constant(multiplier * funcval);
        *isspecial = true;

        return;
    }

    // Number of variables in the power function.
    let nsignvars = if sign { d.nposvars } else { d.nnegvars };

    // If the power function has no more than 2 variables, this a special
    // case.
    *isspecial = nsignvars <= 1 || (nsignvars == 2 && !overestimate);
    if !*isspecial {
        return;
    }

    if nsignvars == 0 {
        // Constant case.
        rowprep.add_constant(multiplier);
        *success = true;
    } else if nsignvars == 1 {
        // Univariate case, w^h.
        for i in 0..d.nvars {
            if d.signs[i] == sign {
                // The variable w is found.
                let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
                let var = d.vars[i];
                let refexponent = d.refexponents[i];

                if refexponent == 1.0 {
                    // h = 1, a univariate linear function. Only rescale, no need for
                    // linearization.
                    rowprep.add_term(var, multiplier / scale);
                } else {
                    // A univariate power function.
                    let mut facetconstant = 0.0;
                    let mut facetcoef = 0.0;
                    let val = crate::api::get_sol_val(scip, sol, var) / scale;
                    // Local (using bounds) depends on whether we under- or overestimate.
                    let mut islocal = !overestimate;

                    crate::expr_pow::estimate_root(
                        scip,
                        refexponent,
                        overestimate,
                        d.intervals[i].inf,
                        d.intervals[i].sup,
                        val,
                        &mut facetconstant,
                        &mut facetcoef,
                        &mut islocal,
                        success,
                    );
                    rowprep.add_constant(multiplier * facetconstant);
                    rowprep.add_term(var, multiplier * facetcoef / scale);
                }
            }
        }
        *success = true;
    } else if nsignvars == 2 && !overestimate {
        // Bivariate case, f(w) = w^h = f_0(w_0) f_1(w_1). The convex under
        // envelope is the maxmimum of two affine functions, each of which is
        // determined by three extreme points the box bound. One affine function
        // is supported by three lower left points; the other affine function is
        // supported by three upper right points. For a point xstar in the box,
        // its corresponding affine function can be determined by which region
        // (upper right or lower left half space) the point is in. Thus, we can
        // determine the region, and use the associated three extreme point to
        // interpolate the affine function.

        let mut facetcoefs = [0.0, 0.0];
        let mut vars = [NONE, NONE];
        let mut refexponents = [0.0, 0.0];
        let mut xstar = [0.0, 0.0];
        let mut scale = [0.0, 0.0];
        let mut boxv = [0.0, 0.0, 0.0, 0.0];
        let mut j = 0;

        // Get refexponents, xstar, scale, and box.
        for i in 0..d.nvars {
            if d.signs[i] != sign {
                continue;
            }
            boxv[2 * j] = d.intervals[i].inf;
            boxv[2 * j + 1] = d.intervals[i].sup;
            refexponents[j] = d.refexponents[i];
            scale[j] = if i == d.nvars - 1 { d.coef } else { 1.0 };
            vars[j] = d.vars[i];
            // (The C reads vars[j] here, not vars[i].)
            xstar[j] = crate::api::get_sol_val(scip, sol, d.vars[j]) / scale[j];
            j += 1;
        }

        // Compute the box length.
        let dw0 = boxv[1] - boxv[0];
        let dw1 = boxv[3] - boxv[2];

        // Determine the location (upper right or lower left half sapce) of the
        // xstar. (dw1, dw0) is the direction vector to the upper right half
        // space.
        let isupperright = ((xstar[0] - boxv[0]) * dw1 + (xstar[1] - boxv[3]) * dw0) > 0.0;

        // Compute function values of f_0, f_1 at vertices.
        let f0w0l = boxv[0].powf(refexponents[0]);
        let f0w0u = boxv[1].powf(refexponents[0]);
        let f1w1l = boxv[2].powf(refexponents[1]);
        let f1w1u = boxv[3].powf(refexponents[1]);
        let fw0lw1u = f0w0l * f1w1u;
        let fw0uw1l = f0w0u * f1w1l;
        let facetconstant = if isupperright {
            // Underestimator: fw0uw1u + (fw0uw1u - fw0lw1u) / (dw0) * (x0 - w0u) +
            // (fw0uw1u - fw0uw1l) / (dw1) * (x1 - w1u).
            let fw0uw1u = f0w0u * f1w1u;
            facetcoefs[0] = (fw0uw1u - fw0lw1u) / dw0;
            facetcoefs[1] = (fw0uw1u - fw0uw1l) / dw1;
            fw0uw1u - facetcoefs[0] * boxv[1] - facetcoefs[1] * boxv[3]
        } else {
            // Underestimator: fw0lw1l + (fw0uw1l - fw0lw1l) / (dw0) * (x0 - w0l) +
            // (fw0lw1u- fw0lw1l) / (dw1) * (x1 - w1l).
            let fw0lw1l = f0w0l * f1w1l;
            facetcoefs[0] = (fw0uw1l - fw0lw1l) / dw0;
            facetcoefs[1] = (fw0lw1u - fw0lw1l) / dw1;
            fw0lw1l - facetcoefs[0] * boxv[0] - facetcoefs[1] * boxv[2]
        };
        rowprep.add_constant(multiplier * facetconstant);
        rowprep.add_term(vars[0], multiplier * facetcoefs[0] / scale[0]);
        rowprep.add_term(vars[1], multiplier * facetcoefs[1] / scale[1]);
        *success = true;
    }
}

/// `underEstimatePower`: adds an underestimator for a multivariate concave
/// power function w^h to a given rowprep.
///
/// Calls `SCIPcomputeFacetVertexPolyhedralNonlinear()` for w^h and box set
/// to local bounds of auxiliary variables. The estimator is multiplied by
/// the multiplier and stored in the rowprep.
#[allow(clippy::too_many_arguments)]
fn under_estimate_power(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlrexprdata: usize,
    sign: bool,
    multiplier: f64,
    sol: SolId,
    targetvalue: f64,
    rowprep: &mut RowPrep,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    // Number of variables of sign.
    let d = ed(scip, nlhdlrexprdata);
    let nsignvars = if sign { d.nposvars } else { d.nnegvars } as usize;

    // Data structure to evaluate the power function.
    let signs = d.signs.clone();
    let refexponents = d.refexponents.clone();

    // Compute box constraints and reference value of variables.
    let mut xstar = std::mem::take(&mut edm(scip, nlhdlrexprdata).xstar);
    let mut boxv = std::mem::take(&mut edm(scip, nlhdlrexprdata).boxv);
    let mut j = 0;
    {
        let d = ed(scip, nlhdlrexprdata);
        for i in 0..d.nvars {
            if d.signs[i] != sign {
                continue;
            }
            boxv[2 * j] = d.intervals[i].inf;
            boxv[2 * j + 1] = d.intervals[i].sup;
            let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
            xstar[j] = crate::api::get_sol_val(scip, sol, d.vars[i]) / scale;
            j += 1;
        }
    }

    // Find a facet of the underestimator.
    let mut facetcoefs = std::mem::take(&mut edm(scip, nlhdlrexprdata).facetcoefs);
    let mut facetconstant = 0.0;
    let mut evalfun = |_: &mut Scip, args: &[f64]| nlhdlr_expr_eval_power(&signs, &refexponents, sign, args);
    *success = crate::cons_nonlinear::compute_facet_vertex_polyhedral_nonlinear(
        scip,
        conshdlr,
        false,
        &mut evalfun,
        &xstar,
        &boxv,
        nsignvars,
        targetvalue,
        &mut facetcoefs,
        &mut facetconstant,
    )?;

    {
        let d = edm(scip, nlhdlrexprdata);
        d.xstar = xstar;
        d.boxv = boxv;
        d.facetcoefs = facetcoefs;
    }

    if !*success {
        return Ok(());
    }

    // Set coefficients in the rowprep.
    rowprep.add_constant(multiplier * facetconstant);
    let d = ed(scip, nlhdlrexprdata);
    let mut j = 0;
    for i in 0..d.nvars {
        if d.signs[i] != sign {
            continue;
        }
        let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
        rowprep.add_term(d.vars[i], multiplier * d.facetcoefs[j] / scale);
        j += 1;
    }

    Ok(())
}

/// `overEstimatePower`: adds an overestimator for a concave power function
/// w^h to a given rowprep; the estimator is multiplied by the multiplier and
/// stored in the rowprep.
fn over_estimate_power(scip: &Scip, d: &ExprData, sign: bool, multiplier: f64, sol: SolId, rowprep: &mut RowPrep, success: &mut bool) {
    *success = false;

    // Compute the value and the sum of reformulated exponents of the power
    // function.
    let mut sumrefexponents = 0.0;
    let mut funcval = 1.0;
    for i in 0..d.nvars {
        if d.signs[i] != sign {
            continue;
        }
        let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
        let mut val = crate::api::get_sol_val(scip, sol, d.vars[i]) / scale;
        val = cmax(val, 0.1);
        let refexponent = d.refexponents[i];
        sumrefexponents += refexponent;
        funcval *= val.powf(refexponent);
    }

    // Overestimate by gradient cut: w'^h + h w'^(h - 1)(w - w').
    let facetconstant = (1.0 - sumrefexponents) * funcval;
    rowprep.add_constant(multiplier * facetconstant);
    for i in 0..d.nvars {
        if d.signs[i] != sign {
            continue;
        }
        let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
        let var = d.vars[i];
        let mut val = crate::api::get_sol_val(scip, sol, var) / scale;
        val = cmax(val, 0.1);
        let facetcoef = d.refexponents[i] * funcval / val;
        rowprep.add_term(var, multiplier * facetcoef / scale);
    }

    *success = true;
}

/*
 * Callback methods of nonlinear handler
 */

/// `nlhdlrEstimateSignomial`: nonlinear handler estimation callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_estimate_signomial(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
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
    *success = false;
    *addedbranchscores = false;

    // Store and capture the vars of an expression, if the vars are not stored
    // and captured yet.
    if !ed(scip, nlhdlrexprdata).isstorecapture {
        store_capture_vars(scip, e, nlhdlrexprdata);
    }

    // Check whether all variables have finite positive bounds, which is
    // necessary for the expression to be a signomial term.
    // TODO consider allowing 0.0 lower bounds for u variables (and handle
    // gradients at 0.0).
    let isboxsignomial = check_signomial_bounds(scip, nlhdlrexprdata);

    if !isboxsignomial {
        return Ok(());
    }

    let (maxnundervars, mincutscale) = {
        let h = nlhdlr::hdlrdata::<NlhdlrData>(scip, nlhdlr);
        (h.maxnundervars, h.mincutscale)
    };

    // Determine the sign of variables for over- and underestimators, and the
    // multiplier for estimators in the rowprep.
    let (oversign, undersign, overmultiplier, undermultiplier, nundervars) = if overestimate {
        // Relax t <= x^a, i.e., 0 <= u^f - v^g, overestimate u^f, underestimate
        // v^g.
        (true, false, 1.0, -1.0, ed(scip, nlhdlrexprdata).nnegvars)
    } else {
        // Relax x^a <= t, i.e., u^f - v^g <= 0, underestimate u^f, overestimate
        // v^g.
        (false, true, -1.0, 1.0, ed(scip, nlhdlrexprdata).nposvars)
    };

    // Compute the value of the overestimator, which is a target value for
    // computing the underestimator efficiently.
    let mut targetunder = 1.0;
    {
        let d = ed(scip, nlhdlrexprdata);
        for i in 0..d.nvars {
            if d.signs[i] == oversign {
                let scale = if i == d.nvars - 1 { d.coef } else { 1.0 };
                targetunder *= (crate::api::get_sol_val(scip, sol, d.vars[i]) / scale).powf(d.refexponents[i]);
            }
        }
    }

    // Create a rowprep and allocate memory for it.
    let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);
    rowprep.ensure_size(ed(scip, nlhdlrexprdata).nvars + 1);

    // Only underestimate a concave function, if the number of variables is
    // below a given threshold.
    if nundervars <= maxnundervars {
        // Compute underestimator.
        let mut isspecial = false;

        // Check and compute the special case.
        estimate_special_power(
            scip,
            ed(scip, nlhdlrexprdata),
            undersign,
            undermultiplier,
            false,
            sol,
            &mut rowprep,
            &mut isspecial,
            success,
        );

        if !isspecial {
            under_estimate_power(scip, conshdlr, nlhdlrexprdata, undersign, undermultiplier, sol, targetunder, &mut rowprep, success)?;
        }
    }

    if *success {
        // Compute overestimator.
        let mut isspecial = false;

        // Check and compute the special case.
        estimate_special_power(scip, ed(scip, nlhdlrexprdata), oversign, overmultiplier, true, sol, &mut rowprep, &mut isspecial, success);

        if !isspecial {
            over_estimate_power(scip, ed(scip, nlhdlrexprdata), oversign, overmultiplier, sol, &mut rowprep, success);
        }
    }

    if *success {
        reform_rowprep(scip, ed(scip, nlhdlrexprdata), &mut rowprep, mincutscale, success);

        if *success {
            rowpreps.push(rowprep);
        }
    }

    Ok(())
}

/// `nlhdlrDetectSignomial`: callback to detect structure in expression
/// tree.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_signomial(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    // For now, we do not get active if separation is already (or does not need
    // to be) provided.
    if (*enforcing & NLHDLR_METHOD_SEPABOTH) == NLHDLR_METHOD_SEPABOTH {
        return Ok(());
    }

    // Check for product expressions with more than one child.
    if expr::expr_is_product(scip, e) && scip.exprs[e].children.len() >= 2 {
        let nf = scip.exprs[e].children.len();
        let nvars = nf + 1;
        let mut ismultilinear = true;

        // Get monomial information.
        let (coef, exponents, factors) = crate::scip_expr::get_expr_monomial_data(scip, e);

        // Skip multilinear terms, since we wouldn't do better than expr_product.
        for &exp in &exponents {
            if !scip.set.is_eq(exp, 1.0) {
                ismultilinear = false;
                break;
            }
        }

        if ismultilinear {
            // If multilinear, we free memory of the expression data and do
            // nothing.
            return Ok(());
        } else {
            let mut sumlexponents = 0.0;
            let mut sumrexponents = 1.0;
            let mut nposvars = 0;

            let mut signs = vec![false; nvars];
            let mut refexponents = vec![0.0; nvars];

            // Detect more information for the reformulation: we first compute the
            // sum of positive and negative exponents and determine the sign
            // indicators.
            for c in 0..nf {
                // Capture sub expressions.
                expr::expr_capture(scip, factors[c]);
                if exponents[c] > 0.0 {
                    // Add a positive exponent.
                    sumlexponents += exponents[c];
                    signs[c] = true;
                    nposvars += 1;
                } else {
                    // Subtract a negative exponent.
                    sumrexponents -= exponents[c];
                    signs[c] = false;
                }
                // Set null to working variables, meaning that they are not stored
                // yet.
            }
            signs[nf] = false;

            // Compute the normalization constant.
            let normalize = cmax(sumlexponents, sumrexponents);

            // Normalize positive and negative exponents.
            for c in 0..nf {
                if signs[c] {
                    refexponents[c] = exponents[c] / normalize;
                } else {
                    refexponents[c] = -exponents[c] / normalize;
                }
            }
            refexponents[nf] = 1.0 / normalize;

            // Create expression data for the nonlinear handler.
            *nlhdlrexprdata = scip.nlhdlrexprdatas.push(Box::new(ExprData {
                coef,
                factors: factors.clone(),
                nfactors: nf,
                nvars,
                exponents,
                nposvars,
                nnegvars: nf as i32 - nposvars + 1,
                signs,
                refexponents,
                isstorecapture: false,
                vars: vec![NONE; nvars],
                intervals: vec![Interval::default(); nvars],
                boxv: vec![0.0; 2 * nvars],
                xstar: vec![0.0; nvars],
                facetcoefs: vec![0.0; nvars],
            }));

            // Tell children that we will use their auxvar and use its activity for
            // estimation.
            for &f in &factors {
                crate::cons_nonlinear::register_expr_usage_nonlinear(scip, f, true, false, true, true)?;
            }
        }
    }

    if *nlhdlrexprdata != NONE {
        *participating = NLHDLR_METHOD_SEPABOTH;
    }

    Ok(())
}

/// `nlhdlrEvalauxSignomial`: auxiliary evaluation callback of nonlinear
/// handler.
fn nlhdlr_evalaux_signomial(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    let d = ed(scip, nlhdlrexprdata);

    *auxvalue = d.coef;
    for c in 0..d.nfactors {
        let var = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.factors[c]);
        let val = crate::api::get_sol_val(scip, sol, var);
        if scip.set.is_positive(val) {
            *auxvalue *= val.powf(d.exponents[c]);
        } else {
            *auxvalue = INVALID;
            break;
        }
    }

    Ok(())
}

/// `nlhdlrCopyhdlrSignomial`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_signomial(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_signomial(targetscip)
}

/// `nlhdlrFreehdlrDataSignomial`: callback to free data of handler.
fn nlhdlr_freehdlr_data_signomial(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    scip.nlhdlrs[nlhdlr].data = None;
    Ok(())
}

/// `nlhdlrFreeExprDataSignomial`: callback to free expression specific
/// data.
fn nlhdlr_free_expr_data_signomial(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    let d = scip.nlhdlrexprdatas.remove(*nlhdlrexprdata).downcast::<ExprData>().unwrap();

    // Release expressions.
    for mut f in d.factors {
        expr::expr_release(scip, &mut f)?;
    }

    // Release variables.
    if d.isstorecapture {
        for mut v in d.vars {
            if v != NONE {
                crate::api::release_var(scip, &mut v)?;
            }
        }
    }

    // Free memory.
    *nlhdlrexprdata = NONE;

    Ok(())
}

/*
 * nonlinear handler specific interface methods
 */

/// `SCIPincludeNlhdlrSignomial`: includes signomial nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_signomial(scip: &mut Scip) -> ScipResult {
    // Create nonlinear handler specific data.
    let nlhdlrdata = NlhdlrData {
        maxnundervars: 0,
        mincutscale: 0.0,
    };

    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_signomial,
        nlhdlr_evalaux_signomial,
        Some(Box::new(nlhdlrdata)),
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_signomial));
    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_freehdlr_data_signomial));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_signomial));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, None, None, Some(nlhdlr_estimate_signomial), None);

    // Parameters.
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, nlhdlrs, nlhdlr, data, NlhdlrData, $f)
        };
    }
    crate::paramset::paramset_add_int(
        scip,
        "nlhdlr/signomial/maxnundervars",
        "maximum number of variables when underestimating a concave power function",
        t!(i32, maxnundervars),
        true,
        NLHDLR_MAXNUNDERVARS,
        2,
        14,
        None,
    )?;
    crate::paramset::paramset_add_real(
        scip,
        "nlhdlr/signomial/mincutscale",
        "minimum scale factor when scaling a cut",
        t!(f64, mincutscale),
        true,
        NLHDLR_MINCUTSCALE,
        1e-6,
        1e6,
        None,
    )?;

    Ok(())
}
