//! Port of `presol_trivial.c`: trivial presolver: round fractional bounds on
//! integer variables, fix variables with equal bounds.

use crate::{
    api,
    cons::*,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_var,
    types::{PresolId, ScipResultCode},
};

const PRESOL_NAME: &str = "trivial";
const PRESOL_DESC: &str = "round fractional bounds on integers, fix variables with equal bounds";
/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = 9000000;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_FAST;

/*
 * Callback methods of presolver
 */

/// `presolExecTrivial`: presolving execution method.
fn presol_exec_trivial(
    scip: &mut Scip,
    _presol: PresolId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    // Get the problem variables (the array, read live: fixings change it).
    let nvars = api::get_n_vars(scip);

    // Scan the variables for trivial bound reductions (loop backwards, since a
    // variable fixing can change the current and the subsequent slots in the
    // vars array).
    for v in (0..nvars).rev() {
        let var = scip.probs[TRANSPROB].vars[v];

        // Get variable's bounds.
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;

        // Is variable integral?
        if crate::var::var_is_integral(scip, var) {
            // Round fractional bounds on integer variables.
            let newlb = scip.set.feas_ceil(lb);
            let newub = scip.set.feas_floor(ub);

            // Check bounds on variable for infeasibility.
            if newlb > newub + 0.5 {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }

            // Fix variables with equal bounds.
            if newlb > newub - 0.5 {
                let (infeasible, _fixed) = scip_var::fix_var(scip, var, newlb)?;
                if infeasible {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                }
                counts.nfixedvars += 1;
            } else {
                // Round fractional bounds.
                if !scip.set.is_feas_eq(lb, newlb) {
                    scip_var::chg_var_lb(scip, var, newlb)?;
                    counts.nchgbds += 1;
                }
                if !scip.set.is_feas_eq(ub, newub) {
                    scip_var::chg_var_ub(scip, var, newub)?;
                    counts.nchgbds += 1;
                }
            }
        } else {
            // Check bounds on continuous variable for infeasibility.
            if scip.set.is_feas_gt(lb, ub) {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }

            // Fix variables with equal bounds.
            if scip.set.is_eq(lb, ub) {
                // Prefer integral values (especially 0) over midpoint.
                let mut fixval = scip.set.round(lb);
                if fixval < lb || fixval > ub {
                    fixval = (lb + ub) / 2.0;
                }

                let (infeasible, _fixed) = scip_var::fix_var(scip, var, fixval)?;
                if infeasible {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                }
                counts.nfixedvars += 1;
            }
        }
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `presolCopyTrivial`: copy method for presolver plugins (called when SCIP
/// copies plugins).
fn presol_copy_trivial(scip: &mut Scip, _sourcescip: &Scip, _presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver.
    include_presol_trivial(scip)?;
    Ok(())
}

/// `SCIPincludePresolTrivial`: creates the trivial presolver and includes it
/// in SCIP.
pub fn include_presol_trivial(scip: &mut Scip) -> ScipResult {
    // Include presolver.
    let presol = api::include_presol_basic(scip, PRESOL_NAME, PRESOL_DESC, PRESOL_PRIORITY, PRESOL_MAXROUNDS, PRESOL_TIMING, presol_exec_trivial, None)?;
    scip.presols[presol].presolcopy = Some(presol_copy_trivial);

    Ok(())
}
