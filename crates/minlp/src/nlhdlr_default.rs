//! Port of `nlhdlr_default.c`: default nonlinear handler that calls
//! expression handler methods.

use crate::{
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, INTERVAL_INFINITY},
    interval::{self, Interval},
    nlhdlr::{self, NLHDLR_METHOD_ACTIVITY, NLHDLR_METHOD_SEPAABOVE, NLHDLR_METHOD_SEPABELOW, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, SideType, SolId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "default";
const NLHDLR_DESC: &str = "default handler for expressions";
const NLHDLR_DETECTPRIORITY: i32 = 0;
const NLHDLR_ENFOPRIORITY: i32 = 0;

/// Translate from one value of infinity to another: if val is >= infty1,
/// then give infty2, else give val.
#[inline]
fn infty2infty(infty1: f64, infty2: f64, val: f64) -> f64 {
    if val >= infty1 { infty2 } else { val }
}

/// Whether underestimation uses activity.
const UNDERESTIMATEUSESACTIVITY: usize = 0x1;
/// Whether overestimation uses activity.
const OVERESTIMATEUSESACTIVITY: usize = 0x2;

/// The flags C stores in the expression data pointer itself (`NULL` is no
/// flag).
#[inline]
fn flags(nlhdlrexprdata: usize) -> usize {
    if nlhdlrexprdata == NONE { 0 } else { nlhdlrexprdata }
}

/// The bounds of the auxiliary variable `auxvar` as an interval.
fn auxvar_bounds(scip: &Scip, lb: f64, ub: f64) -> Interval {
    let infinity = scip.set.infinity();
    let mut i = Interval::default();
    i.set_bounds(-infty2infty(infinity, INTERVAL_INFINITY, -lb), infty2infty(infinity, INTERVAL_INFINITY, ub));
    i
}

/// `evalExprInAux`: evaluates an expression w.r.t. the values in the
/// auxiliary variables.
fn eval_expr_in_aux(scip: &mut Scip, expr: ExprId, val: &mut f64, sol: SolId) -> ScipResult {
    let children = scip.exprs[expr].children.clone();
    let mut childvals = vec![0.0; children.len()];
    for (c, &child) in children.iter().enumerate() {
        let childvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);
        // There should be an auxiliary variable, because we created them in
        // detect for every child if we said that we will separate; at the
        // moment, EVALAUX should only be called for nlhdlrs that said they will
        // separate; if that changes, then we should handle this here, e.g., via
        // *val = SCIPexprGetEvalValue(expr); break;
        childvals[c] = crate::api::get_sol_val(scip, sol, childvar);
    }

    let h = scip.exprs[expr].exprhdlr;
    expr::exprhdlr_eval_expr(scip, h, expr, val, Some(&childvals), NONE)
}

/// `nlhdlrDetectDefault`: check whether expression should be handled by the
/// default nlhdlr; if no nlhdlr so far provides enforcement or
/// boundtightening for expr, then the default nlhdlr takes over.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_default(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    expr: ExprId,
    _cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    let mut estimatebelowusesactivity = false;
    let mut estimateaboveusesactivity = false;

    let exprhdlr = scip.exprs[expr].exprhdlr;

    if *enforcing & NLHDLR_METHOD_ACTIVITY == 0 {
        // Expr handlers having reverseprop but no inteval is something that we
        // don't support at the moment for simplicity.

        // Participate in inteval and/or reverseprop if that is not yet provided
        // in enforcing and we have inteval.
        if scip.exprhdlrs[exprhdlr].inteval.is_some() {
            *participating = NLHDLR_METHOD_ACTIVITY;
        }
    }

    // Participate in sepa if exprhdlr for expr has an estimate callback and
    // sepa below or above is still missing.
    if (*enforcing & NLHDLR_METHOD_SEPABOTH) != NLHDLR_METHOD_SEPABOTH && scip.exprhdlrs[exprhdlr].estimate.is_some() {
        // Communicate back that the nlhdlr will provide the separation on the
        // currently missing sides.
        if *enforcing & NLHDLR_METHOD_SEPABELOW == 0 {
            *participating |= NLHDLR_METHOD_SEPABELOW;
        }

        if *enforcing & NLHDLR_METHOD_SEPAABOVE == 0 {
            *participating |= NLHDLR_METHOD_SEPAABOVE;
        }
    }

    if *participating == 0 {
        return Ok(());
    }

    // Since this is the default handler, we enforce where we participate.
    *enforcing |= *participating;

    // Increment activity usage counter and create auxiliary variables if
    // necessary; if separating, first guess whether we will use activities in
    // estimate (distinguish under- and overestimation); we assume that the
    // exprhdlr will use activity on all children iff we are estimating on a
    // nonconvex side.
    // TODO it would be better to request this information directly from the
    // exprhdlr than inferring it from curvature, but with the currently
    // available exprhdlr that wouldn't make a difference.
    if *participating & NLHDLR_METHOD_SEPABOTH != 0 {
        // Allocate memory to store the required curvature of the children
        // (though we don't use it).
        let mut childcurv = vec![ExprCurv::Unknown; scip.exprs[expr].children.len()];

        if *participating & NLHDLR_METHOD_SEPABELOW != 0 {
            // Check whether the expression is convex.
            let mut isconvex = false;
            expr::exprhdlr_curvature_expr(scip, exprhdlr, expr, ExprCurv::Convex, &mut isconvex, &mut childcurv)?;
            estimatebelowusesactivity = !isconvex;
        }

        if *participating & NLHDLR_METHOD_SEPAABOVE != 0 {
            // Check whether the expression is concave.
            let mut isconcave = false;
            expr::exprhdlr_curvature_expr(scip, exprhdlr, expr, ExprCurv::Concave, &mut isconcave, &mut childcurv)?;
            estimateaboveusesactivity = !isconcave;
        }
    }

    // Indicate enforcement methods required in children:
    // - if separating, make sure that (auxiliary) variable will exist
    // - if activity computation, then register activity usage
    // - if estimating on a non-convex side, then indicate activity usage for
    //   separation for that side
    let children = scip.exprs[expr].children.clone();
    for child in children {
        // todo skip auxvarusage for value-expressions? would then need update
        // in evalExprInAux, too.
        crate::cons_nonlinear::register_expr_usage_nonlinear(
            scip,
            child,
            *participating & NLHDLR_METHOD_SEPABOTH != 0,
            *participating & NLHDLR_METHOD_ACTIVITY != 0,
            estimatebelowusesactivity,
            estimateaboveusesactivity,
        )?;
    }

    // Remember estimatebelowusesactivity and estimateaboveusesactivity in
    // nlhdlrexprdata.
    let f = (if estimatebelowusesactivity { UNDERESTIMATEUSESACTIVITY } else { 0 }) | (if estimateaboveusesactivity { OVERESTIMATEUSESACTIVITY } else { 0 });
    *nlhdlrexprdata = if f == 0 { NONE } else { f };

    Ok(())
}

/// `nlhdlrEvalAuxDefault`: evaluate expression w.r.t. values of auxiliary
/// variables in children.
fn nlhdlr_eval_aux_default(scip: &mut Scip, _nlhdlr: NlhdlrId, expr: ExprId, _nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    eval_expr_in_aux(scip, expr, auxvalue, sol)
}

/// `nlhdlrInitSepaDefault`: initialize LP relaxation by initial estimators.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_init_sepa_default(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    _nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    overestimate: bool,
    underestimate: bool,
    infeasible: &mut bool,
) -> ScipResult {
    *infeasible = false;

    let exprhdlr = scip.exprs[expr].exprhdlr;
    if scip.exprhdlrs[exprhdlr].initestimates.is_none() {
        return Ok(());
    }

    // Use global bounds of auxvar as global valid bounds for children; if at
    // root node (thus local=global) and estimate actually uses bounds, then
    // intersect with (local) activity of expression.
    let children = scip.exprs[expr].children.clone();
    let nchildren = children.len();
    let mut childrenbounds = vec![Interval::default(); nchildren];
    for (i, &child) in children.iter().enumerate() {
        let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);
        let (lb, ub) = (scip.vars[auxvar].glbdom.lb, scip.vars[auxvar].glbdom.ub);
        childrenbounds[i] = auxvar_bounds(scip, lb, ub);
        if crate::api::get_depth(scip) == 0
            && ((underestimate && (flags(nlhdlrexprdata) & UNDERESTIMATEUSESACTIVITY) != 0) || (overestimate && (flags(nlhdlrexprdata) & OVERESTIMATEUSESACTIVITY) != 0))
        {
            expr::expr_eval_activity(scip, child)?;
            let b = childrenbounds[i];
            interval::intersect(&mut childrenbounds[i], b, scip.exprs[child].activity);
        }

        if interval::is_empty(INTERVAL_INFINITY, childrenbounds[i]) {
            *infeasible = true;
            return Ok(());
        }
    }

    // Allocate each coefficients array.
    let mut coefs: Vec<Vec<f64>> = (0..expr::EXPR_MAXINITESTIMATES).map(|_| vec![0.0; nchildren]).collect();
    let mut constant = [0.0; expr::EXPR_MAXINITESTIMATES];

    // Create rowprep.
    let mut rowprep = RowPrep::new(SideType::Right, false);
    rowprep.ensure_size(nchildren + 1);

    // Call the separation initialization callback of the expression handler
    // and turn estimates into SCIP rows.
    let mut i = 0;
    while i < 2 && !*infeasible {
        let mut nreturned = 0;
        if i == 0 && underestimate {
            expr::exprhdlr_init_estimates_expr(scip, exprhdlr, expr, &childrenbounds, false, &mut coefs, &mut constant, &mut nreturned)?;
        }
        if i == 1 && overestimate {
            expr::exprhdlr_init_estimates_expr(scip, exprhdlr, expr, &childrenbounds, true, &mut coefs, &mut constant, &mut nreturned)?;
            rowprep.sidetype = SideType::Left;
        }

        let mut j = 0;
        while j < nreturned as usize && !*infeasible {
            let mut success = false;

            rowprep.reset();

            for (v, &child) in children.iter().enumerate() {
                let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);
                rowprep.add_term(auxvar, coefs[j][v]);
            }
            rowprep.add_term(crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, expr), -1.0);
            rowprep.add_constant(constant[j]);

            // Special treatment for sums to get equality rows.
            if j == 0 && expr::expr_is_sum(scip, expr) {
                // Improve numerics by scaling only (does not relax inequality).
                let scalefactor = rowprep.scaleup(scip, 1.0, Some(&mut success));
                if success && scalefactor == 1.0 && underestimate && overestimate {
                    // If the rowprep didn't have to be changed, then turn it into a
                    // row, change this to an equality, and add it to the LP.
                    // TODO do this also if not actually needing both under- and
                    // overestimator (should still be valid, but also stronger?).
                    rowprep.name = format!("initestimate_sum{j}");

                    let mut row = rowprep.get_row_cons(scip, cons)?;

                    // Since we did not relax the estimator, we can turn the row into
                    // an equality.
                    if scip.set.is_infinity(scip.rows[row].rhs) {
                        let lhs = scip.rows[row].lhs;
                        crate::lp::row_chg_rhs(scip, row, lhs)?;
                    } else {
                        let rhs = scip.rows[row].rhs;
                        crate::lp::row_chg_lhs(scip, row, rhs)?;
                    }

                    *infeasible = crate::scip_lp::add_row(scip, row, false)?;

                    crate::scip_lp::release_row(scip, &mut row)?;

                    // To break outside loop on i, too.
                    i = 2;
                    break;
                }
            }

            // Straighten out numerics.
            let huge = scip.set.get_huge_value();
            rowprep.cleanup2(scip, NONE, huge, Some(&mut success));

            // If cleanup removed all but one variable, then the cut is
            // essentially a bound; we can skip this and rely on boundtightening.
            if success && rowprep.get_n_vars() > 1 {
                // Add the cut.
                rowprep.name = format!("init{}estimate{j}_{}", if i == 0 { "under" } else { "over" }, scip.exprhdlrs[exprhdlr].name);
                let mut row = rowprep.get_row_cons(scip, cons)?;
                *infeasible = crate::scip_lp::add_row(scip, row, false)?;

                crate::scip_lp::release_row(scip, &mut row)?;
            }

            j += 1;
        }

        i += 1;
    }

    Ok(())
}

/// `nlhdlrEstimateDefault`: compute linear estimator.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_estimate_default(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    targetvalue: f64,
    addbranchscores: bool,
    rowpreps: &mut Vec<RowPrep>,
    success: &mut bool,
    addedbranchscores: &mut bool,
) -> ScipResult {
    *addedbranchscores = false;

    let exprhdlr = scip.exprs[expr].exprhdlr;
    let children = scip.exprs[expr].children.clone();
    let nchildren = children.len();

    let mut localbounds = vec![Interval::default(); nchildren];
    let mut globalbounds = vec![Interval::default(); nchildren];
    let mut refpoint = vec![0.0; nchildren];
    // We need to pass a branchcand array to exprhdlr's estimate also if not
    // asked to add branching scores.
    let mut branchcand = vec![false; nchildren];

    'terminate: {
        for (c, &child) in children.iter().enumerate() {
            let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);

            let (lb, ub) = (scip.vars[auxvar].locdom.lb, scip.vars[auxvar].locdom.ub);
            localbounds[c] = auxvar_bounds(scip, lb, ub);

            if flags(nlhdlrexprdata) & (if overestimate { OVERESTIMATEUSESACTIVITY } else { UNDERESTIMATEUSESACTIVITY }) != 0 {
                // If expr estimate uses bounds, then intersect the auxvar bounds with
                // the current activity, in case the latter is a bit tighter.
                expr::expr_eval_activity(scip, child)?;
                let b = localbounds[c];
                interval::intersect_eps(&mut localbounds[c], scip.set.epsilon(), b, scip.exprs[child].activity);

                if interval::is_empty(INTERVAL_INFINITY, localbounds[c]) {
                    *success = false;
                    break 'terminate;
                }
            } else {
                // If we think that expr estimate wouldn't use bounds, then just set
                // something valid.
            }

            let (glb, gub) = (scip.vars[auxvar].glbdom.lb, scip.vars[auxvar].glbdom.ub);
            globalbounds[c] = auxvar_bounds(scip, glb, gub);

            refpoint[c] = crate::api::get_sol_val(scip, sol, auxvar);

            branchcand[c] = true;
        }

        let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);

        // Make sure enough space is available in rowprep arrays.
        rowprep.ensure_size(nchildren);

        // Call the estimation callback of the expression handler (the
        // coefficients go to a buffer, then to the rowprep).
        let mut coefs = vec![0.0; nchildren];
        let mut constant = 0.0;
        let mut local = false;
        expr::exprhdlr_estimate_expr(
            scip,
            exprhdlr,
            expr,
            &localbounds,
            &globalbounds,
            &refpoint,
            overestimate,
            targetvalue,
            &mut coefs,
            &mut constant,
            &mut local,
            success,
            &mut branchcand,
        )?;

        if *success {
            rowprep.local = local;

            // Add variables to rowprep (coefs were already added by
            // SCIPexprhdlrEstimateExpr).
            for (i, &child) in children.iter().enumerate() {
                let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);
                rowprep.add_term(auxvar, coefs[i]);
            }

            rowprep.add_constant(constant);

            rowprep.name = format!(
                "{}estimate_{}{}_{}{}",
                if overestimate { "over" } else { "under" },
                scip.exprhdlrs[exprhdlr].name,
                expr,
                if sol != NONE { "sol" } else { "lp" },
                if sol != NONE { scip.sols[sol].index as i64 } else { crate::api::get_n_lps(scip) }
            );

            rowpreps.push(rowprep);
        }

        if addbranchscores {
            let violation = crate::cons_nonlinear::get_expr_rel_aux_violation_nonlinear(scip, expr, auxvalue, sol, None, None);

            if nchildren == 1 {
                if branchcand[0] {
                    *addedbranchscores = crate::cons_nonlinear::add_exprs_viol_score_nonlinear(scip, &children, violation, sol)?;
                }
            } else {
                // Get list of those children that have the branchcand-flag set.
                let exprs: Vec<ExprId> = (0..nchildren).filter(|&c| branchcand[c]).map(|c| children[c]).collect();

                *addedbranchscores = crate::cons_nonlinear::add_exprs_viol_score_nonlinear(scip, &exprs, violation, sol)?;
            }

            if *addedbranchscores {
                // Count this branchscore as belonging to the exprhdlr, too; thus, it
                // will be counted for the default nlhdlr, but also for this
                // exprhdlr.
                scip.exprhdlrs[exprhdlr].nbranchscores += 1;
            }
        }
    }

    Ok(())
}

/// `nlhdlrSollinearizeDefault`: solution linearization callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_sollinearize_default(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    _nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    _solisbest: bool,
    overestimate: bool,
    underestimate: bool,
) -> ScipResult {
    let exprhdlr = scip.exprs[expr].exprhdlr;
    let children = scip.exprs[expr].children.clone();
    let nchildren = children.len();

    if !overestimate && !underestimate {
        return Ok(());
    }

    // Skip on sum, as the estimator doesn't depend on the reference point
    // (expr is linear in auxvars).
    if expr::expr_is_sum(scip, expr) {
        return Ok(());
    }

    // If expr estimate would use bounds, then the function is very likely not
    // convex (w.r.t. global bounds), so skip.
    let f = flags(nlhdlrexprdata);
    if (overestimate && (f & OVERESTIMATEUSESACTIVITY) != 0) && (underestimate && (f & UNDERESTIMATEUSESACTIVITY) != 0) {
        return Ok(());
    }

    let mut bounds = vec![Interval::default(); nchildren];
    let mut refpoint = vec![0.0; nchildren];
    // We need to pass a branchcand array to exprhdlr's estimate also if not
    // asked to add branching scores.
    let mut branchcand = vec![false; nchildren];

    for (c, &child) in children.iter().enumerate() {
        let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);

        let (lb, ub) = (scip.vars[auxvar].glbdom.lb, scip.vars[auxvar].glbdom.ub);
        bounds[c] = auxvar_bounds(scip, lb, ub);

        refpoint[c] = crate::api::get_sol_val(scip, sol, auxvar);
    }

    // rnd == 0: overestimate, rnd == 1: underestimate.
    let start = if overestimate { 0 } else { 1 };
    let end = if underestimate { 2 } else { 1 };
    for rnd in start..end {
        let mut success = false;

        if rnd == 0 && (f & OVERESTIMATEUSESACTIVITY) != 0 {
            continue;
        }
        if rnd == 1 && (f & UNDERESTIMATEUSESACTIVITY) != 0 {
            continue;
        }

        let mut rowprep = RowPrep::new(if rnd == 0 { SideType::Left } else { SideType::Right }, false);

        // Make sure enough space is available in rowprep arrays.
        rowprep.ensure_size(nchildren + 1);

        for b in branchcand.iter_mut() {
            *b = true;
        }

        // Call the estimation callback of the expression handler; since we pass
        // the global bounds as local bounds, too, we can ignore whether
        // resulting estimator is marked as local.
        let mut coefs = vec![0.0; nchildren];
        let mut constant = 0.0;
        let mut local = false;
        let target = if rnd == 0 { scip.set.infinity() } else { -scip.set.infinity() };
        expr::exprhdlr_estimate_expr(
            scip,
            exprhdlr,
            expr,
            &bounds,
            &bounds,
            &refpoint,
            rnd == 0,
            target,
            &mut coefs,
            &mut constant,
            &mut local,
            &mut success,
            &mut branchcand,
        )?;

        if success {
            // Add variables to rowprep (coefs were already added by
            // SCIPexprhdlrEstimateExpr).
            for (i, &child) in children.iter().enumerate() {
                let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);
                rowprep.add_term(auxvar, coefs[i]);
            }

            rowprep.add_constant(constant);

            rowprep.name = format!(
                "{}estimate_{}{}_sol{}notify",
                if rnd == 0 { "over" } else { "under" },
                scip.exprhdlrs[exprhdlr].name,
                expr,
                scip.sols[sol].index
            );

            // Complete estimator to cut and clean it up.
            rowprep.add_term(crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, expr), -1.0);
            let huge = scip.set.get_huge_value();
            rowprep.cleanup2(scip, sol, huge, Some(&mut success));
        }

        // If cleanup succeeded and rowprep is still global, add to cutpool.
        if success && !rowprep.local {
            let mut row = rowprep.get_row_cons(scip, cons)?;
            crate::api::add_pool_cut(scip, row)?;
            crate::scip_lp::release_row(scip, &mut row)?;
        }
    }

    Ok(())
}

/// `nlhdlrIntevalDefault`: interval-evaluate expression w.r.t. activity of
/// children.
fn nlhdlr_inteval_default(
    scip: &mut Scip,
    _nlhdlr: NlhdlrId,
    expr: ExprId,
    _nlhdlrexprdata: usize,
    interval: &mut Interval,
    intevalvar: Option<ExprIntEvalVar>,
    intevalvardata: usize,
) -> ScipResult {
    // Call the interval evaluation callback of the expression handler.
    let h = scip.exprs[expr].exprhdlr;
    expr::exprhdlr_int_eval_expr(scip, h, expr, interval, intevalvar, intevalvardata)
}

/// `nlhdlrReversepropDefault`: tighten bounds on children from bounds on
/// expression and bounds on children.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_reverseprop_default(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _nlhdlr: NlhdlrId,
    expr: ExprId,
    _nlhdlrexprdata: usize,
    bounds: Interval,
    infeasible: &mut bool,
    nreductions: &mut i32,
) -> ScipResult {
    *nreductions = 0;

    let children = scip.exprs[expr].children.clone();
    let mut childrenbounds: Vec<Interval> = Vec::with_capacity(children.len());
    for &child in &children {
        childrenbounds.push(crate::cons_nonlinear::get_expr_bounds_nonlinear(scip, child));
    }

    // Call the reverse propagation callback of the expression handler.
    let h = scip.exprs[expr].exprhdlr;
    expr::exprhdlr_reverse_prop_expr(scip, h, expr, bounds, &mut childrenbounds, infeasible)?;

    if !*infeasible {
        for (c, &child) in children.iter().enumerate() {
            crate::cons_nonlinear::tighten_expr_interval_nonlinear(scip, child, childrenbounds[c], infeasible, Some(nreductions))?;
        }
        scip.exprhdlrs[h].ndomreds += *nreductions as i64;
    }

    Ok(())
}

/// `nlhdlrCopyhdlrDefault`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_default(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_default(targetscip)
}

/// `nlhdlrFreeExprDataDefault`: callback to free expression specific data.
fn nlhdlr_free_expr_data_default(_scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    *nlhdlrexprdata = NONE;
    Ok(())
}

/// `SCIPincludeNlhdlrDefault`: includes default nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_default(scip: &mut Scip) -> ScipResult {
    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_default,
        nlhdlr_eval_aux_default,
        None,
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_default));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_default));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, Some(nlhdlr_init_sepa_default), None, Some(nlhdlr_estimate_default), None);
    nlhdlr::nlhdlr_set_sollinearize(scip, nlhdlr, Some(nlhdlr_sollinearize_default));
    nlhdlr::nlhdlr_set_prop(scip, nlhdlr, Some(nlhdlr_inteval_default), Some(nlhdlr_reverseprop_default));

    Ok(())
}
