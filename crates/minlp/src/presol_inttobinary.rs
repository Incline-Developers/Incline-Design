//! Port of `presol_inttobinary.c`: converts integer variables with domain
//! [a,a+1] to binaries.

use crate::{
    api,
    cons::*,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_var,
    types::{ImplintType, PresolId, ScipResultCode, VarType},
};

const PRESOL_NAME: &str = "inttobinary";
const PRESOL_DESC: &str = "converts integer variables with domain [a,a+1] to binaries";
/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = 7000000;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_FAST;

/*
 * Callback methods of presolver
 */

/// `presolExecInttobinary`: presolving execution method.
fn presol_exec_inttobinary(
    scip: &mut Scip,
    _presol: PresolId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if scip_var::do_not_aggr(scip) {
        return Ok(());
    }

    // Get the problem variables.
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;
    if nintvars == 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Copy the integer variables into an own array, since adding binary
    // variables affects the left-most slots in the array and thereby
    // interferes with our search loop.
    let vars = scip.probs[TRANSPROB].vars.0[nbinvars..nbinvars + nintvars].to_vec();

    // Scan the integer variables for possible conversion into binaries.
    for &var in &vars {
        // If variable cannot be aggregated, we cannot perform the conversion.
        if scip_var::do_not_aggr_var(scip, var) {
            continue;
        }

        // Get variable's bounds.
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;

        // Check if bounds are exactly one apart; if the lower bound is too large,
        // aggregations will be rejected.
        if scip.set.is_eq(lb, ub - 1.0) && !scip.set.is_huge_value(lb.abs() / scip.set.feastol()) {
            // Create binary variable.
            let binvarname = format!("{}_bin", scip.vars[var].name);
            let (initial, removable) = (scip.vars[var].initial, scip.vars[var].removable);
            let mut binvar = scip_var::create_var_impl(scip, &binvarname, 0.0, 1.0, 0.0, VarType::Binary, ImplintType::None, initial, removable)?;
            scip_var::add_var(scip, binvar)?;

            // Aggregate integer and binary variable.
            let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, var, binvar, 1.0, -1.0, lb)?;

            // Release binary variable.
            api::release_var(scip, &mut binvar)?;

            // It can be the case that this aggregation detects an infeasibility;
            // for example, during the copy of the variable bounds from the integer
            // variable to the binary variable, infeasibility can be detected; this
            // can happen because an upper bound or a lower bound of such a variable
            // bound variable was "just" changed and the varbound constraint
            // handler, who would detect that infeasibility (since it was creating
            // it from a varbound constraint), was called before that bound change
            // was detected due to the presolving priorities.
            if infeasible {
                *result = ScipResultCode::Cutoff;
                break;
            } else if aggregated {
                counts.nchgvartypes += 1;
                counts.naggrvars += 1;
                *result = ScipResultCode::Success;
            }
        }
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `presolCopyInttobinary`: copy method for presolver plugins (called when SCIP
/// copies plugins).
fn presol_copy_inttobinary(scip: &mut Scip, _sourcescip: &Scip, _presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver.
    include_presol_inttobinary(scip)?;
    Ok(())
}

/// `SCIPincludePresolInttobinary`: creates the inttobinary presolver and
/// includes it in SCIP.
pub fn include_presol_inttobinary(scip: &mut Scip) -> ScipResult {
    // Include presolver.
    let presol = api::include_presol_basic(
        scip,
        PRESOL_NAME,
        PRESOL_DESC,
        PRESOL_PRIORITY,
        PRESOL_MAXROUNDS,
        PRESOL_TIMING,
        presol_exec_inttobinary,
        None,
    )?;
    scip.presols[presol].presolcopy = Some(presol_copy_inttobinary);

    Ok(())
}
