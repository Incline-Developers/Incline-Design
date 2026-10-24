//! Port of `nlhdlr_perspective.c`: perspective nonlinear handler.

use std::collections::HashMap;

use crate::{
    def::INVALID,
    expr::{self, ExprId},
    interval::Interval,
    nlhdlr::{self, NLHDLR_METHOD_SEPAABOVE, NLHDLR_METHOD_SEPABELOW, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    retcode::{Retcode, ScipResult},
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, ScipResultCode, SolId, VarId, VarStatus, VarType},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "perspective";
const NLHDLR_DESC: &str = "perspective handler for expressions";
/// Detect last so that to make use of what other handlers detected.
const NLHDLR_DETECTPRIORITY: i32 = -20;
/// Enforce first because perspective cuts are always stronger.
const NLHDLR_ENFOPRIORITY: i32 = 125;

/// Maximal number of propagation rounds in probing.
const DEFAULT_MAXPROPROUNDS: i32 = 1;
/// Minimal relative reduction in a variable's domain for applying probing.
const DEFAULT_MINDOMREDUCTION: f64 = 0.1;
/// Minimal violation w.r.t. auxiliary variables for applying probing.
const DEFAULT_MINVIOLPROBING: f64 = 1e-05;
/// Whether to do probing only in separation loop.
const DEFAULT_PROBINGONLYINSEPA: bool = true;
/// Probing frequency (-1 - no probing, 0 - root node only).
const DEFAULT_PROBINGFREQ: i32 = 1;
/// Whether perspective cuts are added only for convex expressions.
const DEFAULT_CONVEXONLY: bool = false;
/// Whether variable semicontinuity is used to tighten variable bounds.
const DEFAULT_TIGHTENBOUNDS: bool = true;
/// Whether to adjust the reference point if indicator is not 1.
const DEFAULT_ADJREFPOINT: bool = true;

/*
 * Data structures
 */

/// `SCVARDATA`: data structure to store information of a semicontinuous
/// variable.
///
/// For a variable x (not stored in the struct), this stores the data of
/// nbnds implications bvars[i] = 0 -> x = vals[i], bvars[i] = 1 -> lbs[i]
/// <= x <= ubs[i], where bvars[i] are binary variables.
#[derive(Clone, Debug, Default)]
struct ScVarData {
    /// Values of the variable when the corresponding bvars[i] = 0.
    vals0: Vec<f64>,
    /// Global lower bounds of the variable when the corresponding bvars[i] =
    /// 1.
    lbs1: Vec<f64>,
    /// Global upper bounds of the variable when the corresponding bvars[i] =
    /// 1.
    ubs1: Vec<f64>,
    /// The binary variables on which the variable domain depends (`nbnds` is
    /// their number).
    bvars: Vec<VarId>,
}

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
///
/// For an expression expr (not stored in the struct), this stores the data
/// of nindicators implications indicators[i] = 0 -> expr = exprvals[0], where
/// indicators[i] is an indicator (binary) variable, corresponding to some
/// bvars entry in SCVarData. Also stores the variables the expression depends
/// on.
#[derive(Clone, Debug, Default)]
struct ExprData {
    /// 'off' values of the expression for each indicator variable (its own
    /// length: C does not shift it when an indicator is removed).
    exprvals0: Vec<f64>,
    /// Expression variables (both original and auxiliary).
    vars: Vec<VarId>,
    /// All indicator variables for the expression (`nindicators` is their
    /// number).
    indicators: Vec<VarId>,
}

/// `SCIP_NLHDLRDATA`: nonlinear handler data.
struct NlhdlrData {
    /// Maps semicontinuous variables to their on/off bounds (SCVarData);
    /// `None` until created.
    scvars: Option<HashMap<VarId, ScVarData>>,

    /* parameters */
    /// Maximal number of propagation rounds in probing.
    maxproprounds: i32,
    /// Minimal relative reduction in a variable's domain for applying
    /// probing.
    mindomreduction: f64,
    /// Minimal violation w.r.t. auxiliary variables for applying probing.
    minviolprobing: f64,
    /// Whether to do probing only in separation loop.
    probingonlyinsepa: bool,
    /// If and when to do probing.
    probingfreq: i32,
    /// Whether perspective cuts are added only for convex expressions.
    convexonly: bool,
    /// Whether variable semicontinuity is used to tighten variable bounds.
    tightenbounds: bool,
    /// Whether to adjust the reference point if indicator is not 1.
    adjrefpoint: bool,
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

/// `SCIPsortedvecFindPtr(..., SCIPvarComp, ...)`.
fn find_var(scip: &Scip, vars: &[VarId], var: VarId) -> Result<usize, usize> {
    crate::sort::sortedvec_find(vars, |&a, &b| crate::var::var_comp(scip, a, b), var)
}

/*
 * Helper methods for working with nlhdlrExprData
 */

/// `freeNlhdlrExprData`: frees nlhdlrexprdata structure.
fn free_nlhdlr_expr_data(scip: &mut Scip, d: ExprData) -> ScipResult {
    for mut v in d.indicators.into_iter().rev() {
        crate::api::release_var(scip, &mut v)?;
    }

    for mut v in d.vars.into_iter().rev() {
        crate::api::release_var(scip, &mut v)?;
    }

    Ok(())
}

/// `removeIndicator`: remove an indicator from nlhdlr expression data.
fn remove_indicator(scip: &mut Scip, nlhdlrexprdata: usize, pos: usize) -> ScipResult {
    let mut v = edm(scip, nlhdlrexprdata).indicators.remove(pos);
    crate::api::release_var(scip, &mut v)
}

/// `addAuxVar`: adds an auxiliary variable to the vars array in
/// nlhdlrexprdata.
fn add_aux_var(scip: &mut Scip, nlhdlrexprdata: usize, auxvarmap: &mut HashMap<VarId, usize>, auxvar: VarId) {
    if auxvarmap.contains_key(&auxvar) {
        return;
    }

    let d = edm(scip, nlhdlrexprdata);
    let pos = d.vars.len();
    d.vars.push(auxvar);
    crate::var::var_capture(scip, auxvar);
    auxvarmap.insert(auxvar, pos);
}

/*
 * Semicontinuous variable methods
 */

/// `addSCVarIndicator`: adds an indicator to the data of a semicontinuous
/// variable.
fn add_sc_var_indicator(scip: &Scip, scvdata: &mut ScVarData, indicator: VarId, val0: f64, lb1: f64, ub1: f64) {
    // Find the position where to insert.
    let pos = match find_var(scip, &scvdata.bvars, indicator) {
        Ok(_) => return,
        Err(pos) => pos,
    };

    // Move entries if needed.
    scvdata.bvars.insert(pos, indicator);
    scvdata.vals0.insert(pos, val0);
    scvdata.lbs1.insert(pos, lb1);
    scvdata.ubs1.insert(pos, ub1);
}

/// `getSCVarDataInd`: find scvardata of var and position of indicator in it
/// (`None` if indicator is not there).
fn get_sc_var_data_ind<'a>(scip: &Scip, scvars: &'a HashMap<VarId, ScVarData>, var: VarId, indicator: VarId) -> Option<(&'a ScVarData, usize)> {
    let scvdata = scvars.get(&var)?;

    // Look for the indicator variable.
    match find_var(scip, &scvdata.bvars, indicator) {
        Ok(pos) => Some((scvdata, pos)),
        Err(_) => None,
    }
}

/// `varIsSemicontinuous`: checks if a variable is semicontinuous and, if
/// needed, updates the scvars hashmap.
///
/// A variable x is semicontinuous if its bounds depend on at least one
/// binary variable called the indicator, and indicator = 0 => x = x^0 for
/// some real constant x^0.
fn var_is_semicontinuous(scip: &mut Scip, nlhdlr: NlhdlrId, var: VarId) -> bool {
    if hd(scip, nlhdlr).scvars.as_ref().unwrap().contains_key(&var) {
        return true;
    }

    let (vlbvars, vlbcoefs, vlbconstants) = match scip.vars[var].vlbs.as_ref() {
        Some(vb) => {
            let n = vb.len as usize;
            (Some(vb.vars.0[..n].to_vec()), vb.coefs.0[..n].to_vec(), vb.constants.0[..n].to_vec())
        }
        None => (None, Vec::new(), Vec::new()),
    };
    let (vubvars, vubcoefs, vubconstants) = match scip.vars[var].vubs.as_ref() {
        Some(vb) => {
            let n = vb.len as usize;
            (Some(vb.vars.0[..n].to_vec()), vb.coefs.0[..n].to_vec(), vb.constants.0[..n].to_vec())
        }
        None => (None, Vec::new(), Vec::new()),
    };
    let nvlbs = vlbcoefs.len();
    let nvubs = vubcoefs.len();
    let glb = scip.vars[var].glbdom.lb;
    let gub = scip.vars[var].glbdom.ub;

    let mut scvdata: Option<ScVarData> = None;

    // Scan through lower bounds; for each binary vlbvar save the corresponding
    // lb0 and lb1. Then check if there is an upper bound with this vlbvar and
    // save ub0 and ub1. If the found bounds imply that the var value is fixed
    // to some val0 when vlbvar = 0, save vlbvar and val0 to scvdata.
    for c in 0..nvlbs {
        let bvar = vlbvars.as_ref().unwrap()[c];
        if scip.vars[bvar].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, bvar) {
            continue;
        }

        let lb0 = cmax(vlbconstants[c], glb);
        let lb1 = cmax(vlbconstants[c] + vlbcoefs[c], glb);

        // Look for bvar in vubvars.
        let found = match vubvars.as_ref() {
            Some(vv) => find_var(scip, vv, bvar).ok(),
            None => None,
        };

        let (ub0, ub1) = if let Some(pos) = found {
            // Save the upper bounds.
            (cmin(vubconstants[pos], gub), cmin(vubconstants[pos] + vubcoefs[pos], gub))
        } else {
            // If there is no upper bound with vubvar = bvar, use global var bounds.
            (gub, gub)
        };

        // The 'off' domain of a semicontinuous var should reduce to a single
        // point and be different from the 'on' domain.
        if scip.set.is_eq(lb0, ub0) && (!scip.set.is_eq(lb0, lb1) || !scip.set.is_eq(ub0, ub1)) {
            let s = scvdata.get_or_insert_with(ScVarData::default);
            add_sc_var_indicator(scip, s, bvar, lb0, lb1, ub1);
        }
    }

    // Look for vubvars that have not been processed yet.
    for c in 0..nvubs {
        let bvar = vubvars.as_ref().unwrap()[c];
        if scip.vars[bvar].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, bvar) {
            continue;
        }

        // Skip vars that are in vlbvars.
        if let Some(vv) = vlbvars.as_ref()
            && find_var(scip, vv, bvar).is_ok()
        {
            continue;
        }

        let lb0 = glb;
        let lb1 = glb;
        let ub0 = cmin(vubconstants[c], gub);
        let ub1 = cmin(vubconstants[c] + vubcoefs[c], gub);

        // The 'off' domain of a semicontinuous var should reduce to a single
        // point and be different from the 'on' domain.
        if scip.set.is_eq(lb0, ub0) && (!scip.set.is_eq(lb0, lb1) || !scip.set.is_eq(ub0, ub1)) {
            let s = scvdata.get_or_insert_with(ScVarData::default);
            add_sc_var_indicator(scip, s, bvar, lb0, lb1, ub1);
        }
    }

    if let Some(s) = scvdata {
        hdm(scip, nlhdlr).scvars.as_mut().unwrap().insert(var, s);
        return true;
    }

    false
}

/*
 * Semicontinuous expression methods
 */

/// `exprIsSemicontinuous`: checks if an expression is semicontinuous.
///
/// An expression is semicontinuous if all of its nonlinear variables are
/// semicontinuous and share at least one common indicator variable.
fn expr_is_semicontinuous(scip: &mut Scip, nlhdlr: NlhdlrId, nlhdlrexprdata: usize, e: ExprId) -> ScipResult<bool> {
    let nvars = ed(scip, nlhdlrexprdata).vars.len();

    // Constant expression is not semicontinuous; variable expressions are of
    // no interest here.
    if nvars == 0 {
        return Ok(false);
    }

    let mut indicators: Option<Vec<VarId>> = None;
    let nonlinear: Option<Vec<bool>>;

    if expr::expr_is_sum(scip, e) {
        // Sums are treated separately because if there are variables that are
        // non-semicontinuous but appear only linearly, we still want to apply
        // perspective to expr.
        let mut nl = vec![false; nvars];
        let mut it = expr::ExprIter::create();

        let children = scip.exprs[e].children.clone();
        for &child in &children {
            if expr::expr_is_var(scip, child) {
                let var = crate::expr_var::get_var_expr_var(scip, child);

                // Save information on semicontinuity of child.
                var_is_semicontinuous(scip, nlhdlr, var);

                // Since child is a variable, go on regardless of the value of
                // var_is_sc.
                continue;
            }

            let mut issc = true;

            it.init(scip, child, expr::ExprIterType::Dfs, false)?;
            let mut curexpr = it.get_current();

            // All nonlinear terms of a sum should be semicontinuous in original
            // variables.
            while !it.is_end() {
                if expr::expr_is_var(scip, curexpr) {
                    let var = crate::expr_var::get_var_expr_var(scip, curexpr);

                    if !scip.vars[var].relaxationonly {
                        let var_is_sc = var_is_semicontinuous(scip, nlhdlr, var);

                        // Mark the variable as nonlinear.
                        let pos = match find_var(scip, &ed(scip, nlhdlrexprdata).vars, var) {
                            Ok(p) | Err(p) => p,
                        };
                        nl[pos] = true;

                        if !var_is_sc {
                            // Non-semicontinuous child which is (due to a previous check)
                            // not a var -> expr is non-semicontinuous.
                            issc = false;
                            break;
                        }
                    }
                }
                curexpr = it.get_next(scip);
            }

            if !issc {
                it.free(scip);
                return Ok(false);
            }
        }
        it.free(scip);
        nonlinear = Some(nl);
    } else {
        // Non-sum expression.
        nonlinear = None;

        // All variables of a non-sum on/off expression should be semicontinuous.
        for v in 0..nvars {
            let var = ed(scip, nlhdlrexprdata).vars[v];
            if !var_is_semicontinuous(scip, nlhdlr, var) {
                return Ok(false);
            }
        }
    }

    // Look for common binary variables for all variables of the expression.
    for v in 0..nvars {
        if let Some(nl) = nonlinear.as_ref()
            && !nl[v]
        {
            continue;
        }

        let var = ed(scip, nlhdlrexprdata).vars[v];
        let bvars = hd(scip, nlhdlr).scvars.as_ref().unwrap()[&var].bvars.clone();

        // We should have exited earlier if there is a nonlinear
        // non-semicontinuous variable.
        match indicators.as_mut() {
            None => indicators = Some(bvars),
            Some(ind) => {
                // SCIPcomputeArraysIntersectionPtr.
                let mut cnt = 0;
                let mut k = 0;
                let n1 = ind.len();
                for v1 in 0..n1 {
                    // Skip duplicate entries.
                    if v1 + 1 < n1 && ind[v1] == ind[v1 + 1] {
                        continue;
                    }

                    for v2 in k..bvars.len() {
                        if crate::var::var_comp(scip, bvars[v2], ind[v1]) > 0 {
                            k = v2;
                            break;
                        }

                        if bvars[v2] == ind[v1] {
                            ind[cnt] = bvars[v2];
                            cnt += 1;
                            k = v2 + 1;
                            break;
                        }
                    }
                }
                ind.truncate(cnt);
            }
        }

        // If we have found out that the intersection is empty, expr is not
        // semicontinuous.
        if indicators.as_ref().is_some_and(Vec::is_empty) {
            return Ok(false);
        }
    }

    // This can happen if all children are linear vars and none are
    // semicontinuous.
    let Some(indicators) = indicators else {
        return Ok(false);
    };

    for &ind in &indicators {
        crate::var::var_capture(scip, ind);
    }
    edm(scip, nlhdlrexprdata).indicators = indicators;

    Ok(true)
}

/// `computeOffValues`: computes the 'off' value of the expression and the
/// 'off' values of semicontinuous auxiliary variables for each indicator
/// variable.
fn compute_off_values(scip: &mut Scip, nlhdlr: NlhdlrId, nlhdlrexprdata: usize, e: ExprId) -> ScipResult {
    let nindicators = ed(scip, nlhdlrexprdata).indicators.len();
    edm(scip, nlhdlrexprdata).exprvals0 = vec![0.0; nindicators];

    let sol = crate::api::create_sol(scip, NONE)?;
    let mut auxvarmap: HashMap<VarId, usize> = HashMap::with_capacity(10);
    let mut it = expr::ExprIter::create();
    let origvars = ed(scip, nlhdlrexprdata).vars.clone();
    let norigvars = origvars.len();
    let mut origvals0 = vec![0.0; norigvars];

    for i in (0..nindicators).rev() {
        let mut hasnonsc = false;
        let indicator = ed(scip, nlhdlrexprdata).indicators[i];

        // Set sol to the off value of all expr vars for this indicator.
        for v in 0..norigvars {
            // Set vals0[v] = 0 if var is non-sc with respect to indicators[i] -
            // then it will not contribute to exprvals0[i] since any non-sc var must
            // be linear.
            match get_sc_var_data_ind(scip, hd(scip, nlhdlr).scvars.as_ref().unwrap(), origvars[v], indicator) {
                None => {
                    origvals0[v] = 0.0;
                    hasnonsc = true;
                }
                Some((s, pos)) => origvals0[v] = s.vals0[pos],
            }
        }
        crate::api::set_sol_vals(scip, sol, &origvars, &origvals0)?;
        expr::expr_eval(scip, e, sol, 0)?;

        if scip.exprs[e].evalvalue == INVALID {
            // TODO should we fix the indicator variable to 1?
            // Since the loop is backwards, this only modifies the already
            // processed part of nlhdlrexprdata->indicators.
            remove_indicator(scip, nlhdlrexprdata, i)?;
            continue;
        }

        edm(scip, nlhdlrexprdata).exprvals0[i] = scip.exprs[e].evalvalue;

        // Iterate through the expression and create scvdata for aux vars.
        it.init(scip, e, expr::ExprIterType::Dfs, false)?;
        let mut curexpr = it.get_current();

        while !it.is_end() {
            let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, curexpr);

            if auxvar != NONE {
                let mut issc = true;

                if hasnonsc {
                    // Expr is a sum with non-semicontinuous linear terms. Therefore,
                    // curexpr might be non-semicontinuous. In that case the auxvar is also
                    // non-semicontinuous, so we will skip on/off bounds computation.
                    if expr::expr_is_var(scip, curexpr) {
                        // Easy case: curexpr is a variable, can check semicontinuity
                        // immediately.
                        let var = crate::expr_var::get_var_expr_var(scip, curexpr);
                        issc = get_sc_var_data_ind(scip, hd(scip, nlhdlr).scvars.as_ref().unwrap(), var, indicator).is_some();
                    } else if expr::expr_is_sum(scip, curexpr) && curexpr == e {
                        // If expr itself is a sum, this is an exception since a sum with
                        // nonlinear terms is allowed to have both semicontinuous and
                        // non-semicontinuous variables; we skip it here and then analyse it
                        // term by term.
                        issc = false;
                    }

                    // (The C checks the remaining case by assertions, which collect
                    // and release the variable expressions of curexpr.)
                    if !expr::expr_is_var(scip, curexpr) && (!expr::expr_is_sum(scip, curexpr) || curexpr != e) {
                        let mut childvarexprs = crate::scip_expr::get_expr_var_exprs(scip, curexpr)?;
                        for ce in childvarexprs.iter_mut() {
                            expr::expr_release(scip, ce)?;
                        }
                    }
                }

                if issc {
                    // We know that all vars are semicontinuous with respect to
                    // exprdata->indicators; it remains to:
                    // - get or create the scvardata structure for auxvar
                    // - if had to create scvardata, add it to scvars hashmap
                    // - add the indicator and the off value (= curexpr's off value) to
                    //   scvardata
                    let mut s = hdm(scip, nlhdlr).scvars.as_mut().unwrap().remove(&auxvar).unwrap_or_default();
                    let (glb, gub) = (scip.vars[auxvar].glbdom.lb, scip.vars[auxvar].glbdom.ub);
                    add_sc_var_indicator(scip, &mut s, indicator, scip.exprs[curexpr].evalvalue, glb, gub);
                    hdm(scip, nlhdlr).scvars.as_mut().unwrap().insert(auxvar, s);
                }

                add_aux_var(scip, nlhdlrexprdata, &mut auxvarmap, auxvar);
            }

            curexpr = it.get_next(scip);
        }
    }

    it.free(scip);
    crate::api::free_sol(scip, sol)?;

    Ok(())
}

/*
 * Probing and bound tightening methods
 */

/// `startProbing`: go into probing and set some variable bounds; creates a
/// copy of sol in `solcopy` if it still is sol; returns whether indicator ==
/// 1 is infeasible.
#[allow(clippy::too_many_arguments)]
fn start_probing(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    nlhdlrexprdata: usize,
    indicator: VarId,
    probingvars: &[VarId],
    probingdoms: &[Interval],
    sol: SolId,
    solcopy: &mut SolId,
) -> ScipResult<bool> {
    let mut cutoff_probing = false;
    let propagate = crate::api::get_depth(scip) == 0;

    // If a copy of sol has not been created yet, then create one now and copy
    // the relevant var values from sol, because sol can change after
    // SCIPstartProbing, e.g., when linked to the LP solution.
    if *solcopy == sol {
        *solcopy = crate::api::create_sol(scip, NONE)?;
        let d = ed(scip, nlhdlrexprdata).clone();
        for &v in &d.vars {
            let val = crate::api::get_sol_val(scip, sol, v);
            crate::api::set_sol_val(scip, *solcopy, v, val)?;
        }
        for &v in &d.indicators {
            let val = crate::api::get_sol_val(scip, sol, v);
            crate::api::set_sol_val(scip, *solcopy, v, val)?;
        }
    }

    // Go into probing.
    crate::scip_probing::start_probing(scip)?;

    // Create a probing node.
    crate::scip_probing::new_probing_node(scip)?;

    // Set indicator to 1.
    crate::scip_probing::chg_var_lb_probing(scip, indicator, 1.0)?;

    // Apply stored bounds.
    for (v, &pv) in probingvars.iter().enumerate() {
        let newlb = probingdoms[v].inf;
        let newub = probingdoms[v].sup;

        if scip.set.is_gt(newlb, scip.vars[pv].locdom.lb) || (newlb >= 0.0 && scip.vars[pv].locdom.lb < 0.0) {
            crate::scip_probing::chg_var_lb_probing(scip, pv, newlb)?;
        }
        if scip.set.is_lt(newub, scip.vars[pv].locdom.ub) || (newub <= 0.0 && scip.vars[pv].locdom.ub > 0.0) {
            crate::scip_probing::chg_var_ub_probing(scip, pv, newub)?;
        }
    }

    if propagate {
        let maxproprounds = hd(scip, nlhdlr).maxproprounds;
        let (cutoff, _ndomreds) = crate::scip_probing::propagate_probing(scip, maxproprounds)?;
        cutoff_probing = cutoff;
    }

    Ok(cutoff_probing)
}

/// `analyseVarOnoffBounds`: analyse on/off bounds on a variable.
///
/// Analyses for 1. tightening bounds in probing for indicator = 1, 2. fixing
/// indicator / detecting cutoff if one or both states are infeasible, 3.
/// tightening local bounds if indicator is fixed.
///
/// `probinglb` and `probingub` are only set if `doprobing` is TRUE. They are
/// either set to bounds that should be used in probing or to `INVALID` if
/// bounds on `var` shouldn't be changed in probing.
#[allow(clippy::too_many_arguments)]
fn analyse_var_onoff_bounds(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    var: VarId,
    indicator: VarId,
    indvalue: bool,
    infeas: &mut bool,
    probinglb: &mut f64,
    probingub: &mut f64,
    doprobing: bool,
    reduceddom: &mut bool,
) -> ScipResult {
    // Shouldn't be called if indicator is fixed to !indvalue.
    *infeas = false;
    *reduceddom = false;

    let found = get_sc_var_data_ind(scip, hd(scip, nlhdlr).scvars.as_ref().unwrap(), var, indicator).map(|(s, pos)| (s.lbs1[pos], s.ubs1[pos], s.vals0[pos]));

    if doprobing {
        *probinglb = INVALID;
        *probingub = INVALID;
    }

    // Nothing to do for non-semicontinuous variables.
    let Some((lbs1, ubs1, vals0)) = found else {
        return Ok(());
    };

    let sclb = if indvalue { lbs1 } else { vals0 };
    let scub = if indvalue { ubs1 } else { vals0 };
    let loclb = scip.vars[var].locdom.lb;
    let locub = scip.vars[var].locdom.ub;

    // Nothing to do for fixed variables.
    if scip.set.is_eq(loclb, locub) {
        return Ok(());
    }

    let tightenbounds = hd(scip, nlhdlr).tightenbounds;

    // Use a non-redundant lower bound.
    if scip.set.is_gt(sclb, scip.vars[var].locdom.lb) || (sclb >= 0.0 && loclb < 0.0) {
        // First check for infeasibility.
        if scip.set.is_feas_gt(sclb, scip.vars[var].locdom.ub) {
            let (inf, bndchgsuccess) = crate::scip_var::fix_var(scip, indicator, if indvalue { 0.0 } else { 1.0 })?;
            *infeas = inf;
            *reduceddom |= bndchgsuccess;
            if *infeas {
                return Ok(());
            }
        } else if tightenbounds && (scip.vars[indicator].locdom.ub <= 0.5 || scip.vars[indicator].locdom.lb >= 0.5) {
            // Indicator is fixed; due to a previous check, here it can only be fixed
            // to indvalue; therefore, sclb is valid for the current node.
            let (inf, bndchgsuccess) = if !indvalue {
                crate::scip_var::fix_var(scip, var, sclb)?
            } else {
                crate::scip_var::tighten_var_lb(scip, var, sclb, false)?
            };
            *infeas = inf;
            *reduceddom |= bndchgsuccess;
            if *infeas {
                return Ok(());
            }
        }
    }

    // Use a non-redundant upper bound.
    if scip.set.is_lt(scub, scip.vars[var].locdom.ub) || (scub <= 0.0 && locub > 0.0) {
        // First check for infeasibility.
        if scip.set.is_feas_lt(scub, scip.vars[var].locdom.lb) {
            let (inf, bndchgsuccess) = crate::scip_var::fix_var(scip, indicator, if indvalue { 0.0 } else { 1.0 })?;
            *infeas = inf;
            *reduceddom |= bndchgsuccess;
            if *infeas {
                return Ok(());
            }
        } else if tightenbounds && (scip.vars[indicator].locdom.ub <= 0.5 || scip.vars[indicator].locdom.lb >= 0.5) {
            // Indicator is fixed; due to a previous check, here it can only be fixed
            // to indvalue; therefore, scub is valid for the current node.
            let (inf, bndchgsuccess) = if !indvalue {
                crate::scip_var::fix_var(scip, var, scub)?
            } else {
                crate::scip_var::tighten_var_ub(scip, var, scub, false)?
            };
            *infeas = inf;
            *reduceddom |= bndchgsuccess;
            if *infeas {
                return Ok(());
            }
        }
    }

    // If a bound change has been found and indvalue == TRUE, try to use the new
    // bounds. This is only done for indvalue == TRUE since this is where enfo
    // asks other nlhdlrs to estimate, and at indicator == FALSE we already only
    // have a single point.
    let mindomreduction = hd(scip, nlhdlr).mindomreduction;
    if doprobing && indvalue && (((scub - sclb) / (locub - loclb)) <= 1.0 - mindomreduction || (sclb >= 0.0 && loclb < 0.0) || (scub <= 0.0 && locub > 0.0)) {
        *probinglb = sclb;
        *probingub = scub;
    }

    Ok(())
}

/// `analyseOnoffBounds`: looks for bound tightenings to be applied either in
/// the current node or in probing.
///
/// Loops through both possible values of indicator and calls
/// analyseVarOnoffBounds(). Might update the `*doprobing` flag by setting it
/// to `FALSE` if indicator is fixed or analyseVarOnoffBounds() hasn't found a
/// sufficient improvement at indicator==1. If `*doprobing==TRUE`, stores
/// bounds suggested by analyseVarOnoffBounds() in order to apply them in
/// probing together with the fixing `indicator=1`.
#[allow(clippy::too_many_arguments)]
fn analyse_onoff_bounds(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    nlhdlrexprdata: usize,
    indicator: VarId,
    probingvars: &mut Vec<VarId>,
    probingdoms: &mut Vec<Interval>,
    doprobing: &mut bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut probinglb = INVALID;
    let mut probingub = INVALID;
    let mut changed = false;

    // No probing if indicator already fixed.
    if scip.vars[indicator].locdom.ub <= 0.5 || scip.vars[indicator].locdom.lb >= 0.5 {
        *doprobing = false;
    }

    // Consider each possible value of indicator.
    for b in 0..2 {
        let nvars = ed(scip, nlhdlrexprdata).vars.len();
        for v in 0..nvars {
            // Nothing left to do if indicator is already fixed to !indvalue (checked
            // in the inner loop since analyseVarOnoff bounds might fix the
            // indicator).
            if (b == 1 && scip.vars[indicator].locdom.ub <= 0.5) || (b == 0 && scip.vars[indicator].locdom.lb >= 0.5) {
                *doprobing = false;
                break;
            }

            let var = ed(scip, nlhdlrexprdata).vars[v];

            let mut infeas = false;
            let mut reduceddom = false;
            analyse_var_onoff_bounds(
                scip,
                nlhdlr,
                var,
                indicator,
                b == 1,
                &mut infeas,
                &mut probinglb,
                &mut probingub,
                *doprobing,
                &mut reduceddom,
            )?;

            if infeas {
                *result = ScipResultCode::Cutoff;
                *doprobing = false;
                return Ok(());
            } else if reduceddom {
                *result = ScipResultCode::ReducedDom;
            }

            if !*doprobing {
                continue;
            }

            // If bounds to be applied in probing have been found, store them.
            if probinglb != INVALID {
                probingvars.push(var);
                probingdoms.push(Interval { inf: probinglb, sup: probingub });
                changed = true;
            }
        }
    }

    if !changed {
        *doprobing = false;
    }

    Ok(())
}

/// `tightenOnBounds`: saves local bounds on all expression variables,
/// including auxiliary variables, obtained from propagating indicator == 1
/// to the corresponding SCVARDATA (should only be used in the root node).
fn tighten_on_bounds(scip: &mut Scip, nlhdlr: NlhdlrId, nlhdlrexprdata: usize, indicator: VarId) {
    let vars = ed(scip, nlhdlrexprdata).vars.clone();
    for var in vars {
        let lb = scip.vars[var].locdom.lb;
        let ub = scip.vars[var].locdom.ub;
        let pos = get_sc_var_data_ind(scip, hd(scip, nlhdlr).scvars.as_ref().unwrap(), var, indicator).map(|(_, p)| p);

        if let Some(pos) = pos {
            let s = hdm(scip, nlhdlr).scvars.as_mut().unwrap().get_mut(&var).unwrap();
            s.lbs1[pos] = cmax(s.lbs1[pos], lb);
            s.ubs1[pos] = cmin(s.ubs1[pos], ub);
        }
    }
}

/*
 * Callback methods of nonlinear handler
 */

/// `nlhdlrCopyhdlrPerspective`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_perspective(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_perspective(targetscip)
}

/// `nlhdlrFreehdlrdataPerspective`: callback to free data of handler.
fn nlhdlr_freehdlrdata_perspective(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    scip.nlhdlrs[nlhdlr].data = None;
    Ok(())
}

/// `nlhdlrFreeExprDataPerspective`: callback to free expression specific
/// data.
fn nlhdlr_free_expr_data_perspective(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    let d = *scip.nlhdlrexprdatas.remove(*nlhdlrexprdata).downcast::<ExprData>().unwrap();
    free_nlhdlr_expr_data(scip, d)?;
    *nlhdlrexprdata = NONE;
    Ok(())
}

/// `nlhdlrExitPerspective`: callback to be called in deinitialization.
fn nlhdlr_exit_perspective(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    hdm(scip, nlhdlr).scvars = None;
    Ok(())
}

/// `nlhdlrDetectPerspective`: callback to detect structure in expression
/// tree.
///
/// We are looking for expressions g(x), where x is a vector of
/// semicontinuous variables that all share at least one indicator variable.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_perspective(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    _enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    let mut hassepabelow = false;
    let mut hassepaabove = false;
    let mut hasnondefault = false;

    // Do not run if we will have no auxvar to add a cut for.
    if crate::cons_nonlinear::get_expr_n_auxvar_uses_nonlinear(scip, e) == 0 {
        return Ok(());
    }

    if crate::api::get_n_bin_vars(scip) == 0 {
        return Ok(());
    }

    let convexonly = hd(scip, nlhdlr).convexonly;
    let nenfos = crate::cons_nonlinear::get_expr_n_enfos_nonlinear(scip, e);
    for i in 0..nenfos.max(0) as usize {
        let enfo = *crate::cons_nonlinear::get_expr_enfo_data_nonlinear(scip, e, i);
        let nlhdlr2 = enfo.nlhdlr;
        let nlhdlr2participates = enfo.nlhdlrparticipation;

        if nlhdlr2participates & NLHDLR_METHOD_SEPABOTH == 0 {
            continue;
        }

        if scip.nlhdlrs[nlhdlr2].estimate.is_none() {
            continue;
        }

        if scip.nlhdlrs[nlhdlr2].name != "default" {
            hasnondefault = true;
        }

        // If we are supposed to run only on convex expressions, than check whether
        // there is a nlhdlr that participates in separation without using
        // activity for it. Otherwise, check for participation regardless of
        // activity usage.
        if (nlhdlr2participates & NLHDLR_METHOD_SEPABELOW) != 0 && (!convexonly || !enfo.sepabelowusesactivity) {
            hassepabelow = true;
        }

        if (nlhdlr2participates & NLHDLR_METHOD_SEPAABOVE) != 0 && (!convexonly || !enfo.sepaaboveusesactivity) {
            hassepaabove = true;
        }
    }

    // If a sum expression is handled only by default nlhdlr, then all the
    // children will have auxiliary vars. Since the sum will then be linear in
    // auxiliary variables, perspective can't improve anything for it.
    if expr::expr_is_sum(scip, e) && !hasnondefault {
        return Ok(());
    }

    // If no other nlhdlr separates, neither does perspective (if convexonly,
    // only separation without using activity counts).
    if !hassepabelow && !hassepaabove {
        return Ok(());
    }

    // Allocate memory.
    let id = scip.nlhdlrexprdatas.push(Box::new(ExprData::default()));
    *nlhdlrexprdata = id;

    if hd(scip, nlhdlr).scvars.is_none() {
        let nvars = crate::api::get_n_vars(scip) as usize;
        hdm(scip, nlhdlr).scvars = Some(HashMap::with_capacity(nvars));
    }

    // Save varexprs to nlhdlrexprdata.
    crate::scip_expr::get_expr_n_vars(scip, e)?;
    let varexprs = crate::scip_expr::get_expr_var_exprs(scip, e)?;
    let mut vars = Vec::with_capacity(varexprs.len());
    for mut ve in varexprs {
        let v = crate::expr_var::get_var_expr_var(scip, ve);
        vars.push(v);
        expr::expr_release(scip, &mut ve)?;
        crate::var::var_capture(scip, v);
    }
    crate::sort::sort(&mut vars, (), |&a, &b| crate::var::var_comp(scip, a, b));
    edm(scip, id).vars = vars;

    // Check if expr is semicontinuous and save indicator variables.
    let success = expr_is_semicontinuous(scip, nlhdlr, id, e)?;

    if success {
        if hassepaabove {
            *participating |= NLHDLR_METHOD_SEPAABOVE;
        }
        if hassepabelow {
            *participating |= NLHDLR_METHOD_SEPABELOW;
        }
    } else {
        nlhdlr_free_expr_data_perspective(scip, nlhdlr, e, nlhdlrexprdata)?;
    }

    Ok(())
}

/// `nlhdlrEvalauxPerspective`: auxiliary evaluation callback of nonlinear
/// handler.
fn nlhdlr_evalaux_perspective(scip: &mut Scip, _nlhdlr: NlhdlrId, e: ExprId, _nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    let auxvarvalue = crate::api::get_sol_val(scip, sol, crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e));
    let mut maxdiff = 0.0;
    *auxvalue = auxvarvalue;

    // Use the auxvalue from one of the other nlhdlrs that estimates for this
    // expr: take the one that is farthest from the current value of auxvar.
    let nenfos = crate::cons_nonlinear::get_expr_n_enfos_nonlinear(scip, e);
    for i in 0..nenfos.max(0) as usize {
        let enfo = *crate::cons_nonlinear::get_expr_enfo_data_nonlinear(scip, e, i);
        let nlhdlr2 = enfo.nlhdlr;

        // Skip nlhdlr that do not participate or do not provide estimate.
        if enfo.nlhdlrparticipation & NLHDLR_METHOD_SEPABOTH == 0 || scip.nlhdlrs[nlhdlr2].estimate.is_none() {
            continue;
        }

        let mut enfoauxval = 0.0;
        nlhdlr::nlhdlr_evalaux(scip, nlhdlr2, e, enfo.nlhdlrexprdata, &mut enfoauxval, sol)?;

        crate::cons_nonlinear::set_expr_enfo_aux_value_nonlinear(scip, e, i, enfoauxval);

        if (enfoauxval - auxvarvalue).abs() > maxdiff && enfoauxval != INVALID {
            maxdiff = (enfoauxval - auxvarvalue).abs();
            *auxvalue = enfoauxval;
        }
    }

    Ok(())
}

/// `nlhdlrInitSepaPerspective`: separation initialization method of a
/// nonlinear handler.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_init_sepa_perspective(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _cons: ConsId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    _overestimate: bool,
    _underestimate: bool,
    _infeasible: &mut bool,
) -> ScipResult {
    // Compute 'off' values of expr and subexprs (and thus auxvars too).
    compute_off_values(scip, nlhdlr, nlhdlrexprdata, e)?;

    // Some indicator variables might have been removed if evaluation failed,
    // check how many remain.
    let d = edm(scip, nlhdlrexprdata);
    let n = d.indicators.len();
    d.exprvals0.truncate(n);

    Ok(())
}

/// `nlhdlrEnfoPerspective`: nonlinear handler enforcement callback.
///
/// "Perspectivies" cuts produced by other nonlinear handlers.
///
/// Suppose that we want to separate x from the set { x : g(x) <= 0 }. If
/// g(x) = g^0 if indicator z = 0, and a cut is given by sum_i a_i x_i + c <=
/// aux, where x_i = x_i^0 if z = 0 for all i, then the "perspectivied" cut is
/// sum_i a_i x_i + c + (1 - z) (g^0 - c - sum_i a_i x_i^0) <= aux. This
/// ensures that at z = 1, the new cut is equivalent to the given cut, and at
/// z = 0 it reduces to g^0 <= aux.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_enfo_perspective(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    allowweakcuts: bool,
    _separated: bool,
    addbranchscores: bool,
    branchcandonly: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    if ed(scip, nlhdlrexprdata).indicators.is_empty() {
        // We might have removed all indicators in initsepa.
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    if branchcandonly {
        // Let the regular calls to the nlhdlrs after perspective register
        // branching candidates.
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e);

    // Detect should have picked only those expressions for which at least one
    // other nlhdlr can enforce.
    let mut enfoposs: Vec<usize> = Vec::new();
    let mut doprobing = false;

    let (convexonly, minviolprobing, probingfreq, probingonlyinsepa, adjrefpointparam) = {
        let h = hd(scip, nlhdlr);
        (h.convexonly, h.minviolprobing, h.probingfreq, h.probingonlyinsepa, h.adjrefpoint)
    };

    // Find suitable nlhdlrs and check if there is enough violation to do
    // probing.
    let nenfos_expr = crate::cons_nonlinear::get_expr_n_enfos_nonlinear(scip, e);
    for j in 0..nenfos_expr.max(0) as usize {
        let enfo = *crate::cons_nonlinear::get_expr_enfo_data_nonlinear(scip, e, j);
        let nlhdlr2 = enfo.nlhdlr;
        let sepausesactivity = if overestimate { enfo.sepaaboveusesactivity } else { enfo.sepabelowusesactivity };

        if nlhdlr2 == nlhdlr {
            continue;
        }

        // If nlhdlr2 cannot estimate, then cannot use it.
        if scip.nlhdlrs[nlhdlr2].estimate.is_none() {
            continue;
        }

        // If nlhdlr2 does not participate in the separation on the desired side
        // (overestimate), then skip it.
        if enfo.nlhdlrparticipation & (if overestimate { NLHDLR_METHOD_SEPAABOVE } else { NLHDLR_METHOD_SEPABELOW }) == 0 {
            continue;
        }

        // If only working on convex-looking expressions, then skip nlhdlr if it
        // uses activity for estimates.
        if convexonly && sepausesactivity {
            continue;
        }

        // Evalaux should have called evalaux of nlhdlr2 by now; check whether
        // handling the violation for nlhdlr2 requires under- or overestimation and
        // this fits to overestimate flag.
        let mut violbelow = false;
        let mut violabove = false;
        let violation = crate::cons_nonlinear::get_expr_abs_aux_violation_nonlinear(scip, e, enfo.auxvalue, sol, Some(&mut violbelow), Some(&mut violabove));

        if (overestimate && !violabove) || (!overestimate && !violbelow) {
            continue;
        }

        // If violation is small, cuts would likely be weak - skip
        // perspectification.
        if !allowweakcuts && violation < scip.set.feastol() {
            continue;
        }

        enfoposs.push(j);

        // Enable probing if tightening the domain could be useful for nlhdlr and
        // violation is above threshold.
        if sepausesactivity && violation >= minviolprobing {
            doprobing = true;
        }
    }

    if enfoposs.is_empty() {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    // Check probing frequency against depth in b&b tree.
    let depth = crate::api::get_depth(scip);
    if probingfreq == -1 || (probingfreq == 0 && depth != 0) || (probingfreq > 0 && depth % probingfreq != 0) {
        doprobing = false;
    }

    // If addbranchscores is TRUE, then we can assume to be in enforcement and
    // not in separation.
    if probingonlyinsepa && addbranchscores {
        doprobing = false;
    }

    // Disable probing if already being in probing or if in a subscip.
    if crate::api::in_probing(scip) || crate::copy::get_subscip_depth(scip) != 0 {
        doprobing = false;
    }

    *result = ScipResultCode::DidNotFind;
    let mut solcopy = sol;
    let mut stop = false;

    let mut rowpreps2: Vec<RowPrep> = Vec::new();
    let mut rowpreps: Vec<RowPrep> = Vec::new();
    let mut addedbranchscores2: Vec<bool> = Vec::new();

    // Build cuts for every indicator variable.
    let mut i = 0;
    'terminate: while i < ed(scip, nlhdlrexprdata).indicators.len() && !stop {
        let indicator = ed(scip, nlhdlrexprdata).indicators[i];
        let mut probingvars: Vec<VarId> = Vec::new();
        let mut probingdoms: Vec<Interval> = Vec::new();
        let mut doprobingind = doprobing;
        let solval = crate::api::get_sol_val(scip, solcopy, indicator);
        let adjrefpoint = adjrefpointparam && !scip.set.is_feas_eq(solval, 1.0);
        let mut soladj = NONE;

        analyse_onoff_bounds(scip, nlhdlr, nlhdlrexprdata, indicator, &mut probingvars, &mut probingdoms, &mut doprobingind, result)?;

        // Don't add perspective cuts for fixed indicators since there is no use
        // for perspectivy.
        if scip.vars[indicator].locdom.lb >= 0.5 {
            i += 1;
            continue;
        }

        if scip.vars[indicator].locdom.ub <= 0.5 {
            // This case is stronger as it implies that everything is fixed;
            // therefore we are now happy.
            break 'terminate;
        }

        if doprobingind {
            let propagate = crate::api::get_depth(scip) == 0;

            let cutoff_probing = start_probing(scip, nlhdlr, nlhdlrexprdata, indicator, &probingvars, &probingdoms, sol, &mut solcopy)?;

            if propagate {
                // We are in the root node and startProbing did propagation; probing
                // propagation might have detected infeasibility.
                if cutoff_probing {
                    // Indicator == 1 is infeasible -> set indicator to 0.
                    crate::scip_probing::end_probing(scip)?;

                    let (cutoff, _fixed) = crate::scip_var::fix_var(scip, indicator, 0.0)?;

                    if cutoff {
                        *result = ScipResultCode::Cutoff;
                        break 'terminate;
                    }

                    i += 1;
                    continue;
                }

                // Probing propagation in the root node can provide better on/off
                // bounds.
                tighten_on_bounds(scip, nlhdlr, nlhdlrexprdata, indicator);
            }
        }

        if adjrefpoint {
            // Make sure that when we adjust the point, we don't divide by something
            // too close to 0.0.
            let indval = cmax(solval, 0.1);

            // Create an adjusted point x^adj = (x* - x0) / z* + x0.
            soladj = crate::api::create_sol(scip, NONE)?;
            let d = ed(scip, nlhdlrexprdata).clone();
            for &v in &d.vars {
                if scip.vars[v].varstatus == VarStatus::Fixed {
                    continue;
                }

                // A non-semicontinuous variable must be linear in expr; skip it.
                let Some(vals0) = get_sc_var_data_ind(scip, hd(scip, nlhdlr).scvars.as_ref().unwrap(), v, indicator).map(|(s, pos)| s.vals0[pos]) else {
                    continue;
                };

                let val = (crate::api::get_sol_val(scip, solcopy, v) - vals0) / indval + vals0;
                crate::api::set_sol_val(scip, soladj, v, val)?;
            }
            for &v in &d.indicators {
                if scip.vars[v].varstatus == VarStatus::Fixed {
                    continue;
                }

                let val = crate::api::get_sol_val(scip, solcopy, v);
                crate::api::set_sol_val(scip, soladj, v, val)?;
            }
            if scip.vars[auxvar].varstatus != VarStatus::Fixed {
                let val = crate::api::get_sol_val(scip, solcopy, auxvar);
                crate::api::set_sol_val(scip, soladj, auxvar, val)?;
            }
        }

        // Use cuts from every suitable nlhdlr.
        for &enfopos in &enfoposs {
            let enfo = *crate::cons_nonlinear::get_expr_enfo_data_nonlinear(scip, e, enfopos);
            let nlhdlr2 = enfo.nlhdlr;
            let nlhdlr2exprdata = enfo.nlhdlrexprdata;
            let mut nlhdlr2auxvalue = enfo.auxvalue;
            let mut success2 = false;
            let mut addedbranchscores2j = false;

            // Ask the nonlinear handler for an estimator.
            let target = crate::api::get_sol_val(scip, solcopy, auxvar);
            if adjrefpoint {
                nlhdlr::nlhdlr_evalaux(scip, nlhdlr2, e, nlhdlr2exprdata, &mut nlhdlr2auxvalue, soladj)?;

                nlhdlr::nlhdlr_estimate(
                    scip,
                    conshdlr,
                    nlhdlr2,
                    e,
                    nlhdlr2exprdata,
                    soladj,
                    nlhdlr2auxvalue,
                    overestimate,
                    target,
                    false,
                    &mut rowpreps2,
                    &mut success2,
                    &mut addedbranchscores2j,
                )?;
            } else {
                nlhdlr::nlhdlr_estimate(
                    scip,
                    conshdlr,
                    nlhdlr2,
                    e,
                    nlhdlr2exprdata,
                    solcopy,
                    nlhdlr2auxvalue,
                    overestimate,
                    target,
                    false,
                    &mut rowpreps2,
                    &mut success2,
                    &mut addedbranchscores2j,
                )?;
            }

            // Perspectivy all cuts from nlhdlr2 and add them to rowpreps.
            for mut rowprep in rowpreps2.drain(..) {
                // Given a rowprep: sum aixi + sum biyi + c, where xi are
                // semicontinuous variables and yi are non-semicontinuous variables
                // (which appear in expr linearly, which detect must have ensured),
                // perspectivy the semicontinuous part by adding (1-z)(g0 - c - sum
                // aix0i) (the constant is treated as belonging to the semicontinuous
                // part).

                // We want cst0 = g0 - c - sum aix0i; first add g0 - c.
                let mut cst0 = ed(scip, nlhdlrexprdata).exprvals0[i] + rowprep.side;

                let mut maxcoef = 0.0;
                for v in 0..rowprep.get_n_vars() {
                    if rowprep.coefs[v].abs() > maxcoef {
                        maxcoef = rowprep.coefs[v].abs();
                    }

                    // A non-semicontinuous variable must be linear in expr; skip it.
                    let Some(vals0) = get_sc_var_data_ind(scip, hd(scip, nlhdlr).scvars.as_ref().unwrap(), rowprep.vars[v], indicator).map(|(s, pos)| s.vals0[pos]) else {
                        continue;
                    };

                    cst0 -= rowprep.coefs[v] * vals0;
                }

                // Only perspectivy when the absolute value of cst0 is not too small.
                // TODO on ex1252a there was cst0=0 - ok to still use the cut?
                if cst0 == 0.0 || maxcoef / cst0.abs() <= 10.0 / scip.set.feastol() {
                    // Update the rowprep by adding cst0 - cst0*z.
                    rowprep.add_constant(cst0);
                    rowprep.add_term(indicator, -cst0);
                } else {
                    continue;
                }

                rowprep.add_term(auxvar, -1.0);

                rowpreps.push(rowprep);
                addedbranchscores2.push(addedbranchscores2j);
            }
        }

        if adjrefpoint {
            crate::api::free_sol(scip, soladj)?;
        }

        if doprobingind {
            crate::scip_probing::end_probing(scip)?;
        }

        // Add all cuts found for indicator i.
        let indname = scip.vars[indicator].name.clone();
        for (r, rowprep) in rowpreps.iter_mut().enumerate() {
            if stop {
                break;
            }

            let mut resultr = ScipResultCode::DidNotFind;

            rowprep.name.push_str("_persp_indicator_");
            rowprep.name.push_str(&indname);

            crate::cons_nonlinear::process_rowprep_nonlinear(
                scip,
                nlhdlr,
                cons,
                e,
                rowprep,
                overestimate,
                auxvar,
                auxvalue,
                allowweakcuts,
                addedbranchscores2[r],
                false,
                solcopy,
                &mut resultr,
            )?;

            if resultr == ScipResultCode::Separated {
                *result = ScipResultCode::Separated;
            } else if resultr == ScipResultCode::Cutoff {
                *result = ScipResultCode::Cutoff;
                stop = true;
            } else if resultr == ScipResultCode::Branched {
                if *result != ScipResultCode::Separated && *result != ScipResultCode::ReducedDom {
                    *result = ScipResultCode::Branched;
                }
            } else if resultr != ScipResultCode::DidNotFind {
                log::error!("estimate called by perspective nonlinear handler returned invalid result <{resultr:?}>");
                return Err(Retcode::InvalidResult);
            }
        }

        // Free all rowpreps for indicator i.
        rowpreps.clear();
        addedbranchscores2.clear();

        i += 1;
    }

    if solcopy != sol {
        crate::api::free_sol(scip, solcopy)?;
    }

    Ok(())
}

/*
 * nonlinear handler specific interface methods
 */

/// `SCIPincludeNlhdlrPerspective`: includes perspective nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_perspective(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Create nonlinear handler data.
    let nlhdlrdata = NlhdlrData {
        scvars: None,
        maxproprounds: 0,
        mindomreduction: 0.0,
        minviolprobing: 0.0,
        probingonlyinsepa: false,
        probingfreq: 0,
        convexonly: false,
        tightenbounds: false,
        adjrefpoint: false,
    };

    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_perspective,
        nlhdlr_evalaux_perspective,
        Some(Box::new(nlhdlrdata)),
    )?;

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, nlhdlrs, nlhdlr, data, NlhdlrData, $f)
        };
    }
    paramset_add_int(
        scip,
        "nlhdlr/perspective/maxproprounds",
        "maximal number of propagation rounds in probing",
        t!(i32, maxproprounds),
        false,
        DEFAULT_MAXPROPROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "nlhdlr/perspective/mindomreduction",
        "minimal relative reduction in a variable's domain for applying probing",
        t!(f64, mindomreduction),
        false,
        DEFAULT_MINDOMREDUCTION,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "nlhdlr/perspective/minviolprobing",
        "minimal violation w.r.t. auxiliary variables for applying probing",
        t!(f64, minviolprobing),
        false,
        DEFAULT_MINVIOLPROBING,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/perspective/probingonlyinsepa",
        "whether to do probing only in separation",
        t!(bool, probingonlyinsepa),
        false,
        DEFAULT_PROBINGONLYINSEPA,
        None,
    )?;
    paramset_add_int(
        scip,
        "nlhdlr/perspective/probingfreq",
        "probing frequency (-1 - no probing, 0 - root node only)",
        t!(i32, probingfreq),
        false,
        DEFAULT_PROBINGFREQ,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/perspective/convexonly",
        "whether perspective cuts are added only for convex expressions",
        t!(bool, convexonly),
        false,
        DEFAULT_CONVEXONLY,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/perspective/tightenbounds",
        "whether variable semicontinuity is used to tighten variable bounds",
        t!(bool, tightenbounds),
        false,
        DEFAULT_TIGHTENBOUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/perspective/adjrefpoint",
        "whether to adjust the reference point",
        t!(bool, adjrefpoint),
        false,
        DEFAULT_ADJREFPOINT,
        None,
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_perspective));
    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_freehdlrdata_perspective));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_perspective));
    nlhdlr::nlhdlr_set_init_exit(scip, nlhdlr, None, Some(nlhdlr_exit_perspective));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, Some(nlhdlr_init_sepa_perspective), Some(nlhdlr_enfo_perspective), None, None);

    Ok(())
}
