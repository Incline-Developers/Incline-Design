//! Port of `prop_dualfix.c`: fixing roundable variables to best bound.

use crate::{
    api,
    cons::*,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_var,
    types::{LockType, PropId, ScipResultCode},
};

const PROP_NAME: &str = "dualfix";
const PROP_DESC: &str = "roundable variables dual fixing";
const PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;
/// Propagation priority.
const PROP_PRIORITY: i32 = 8000000;
/// Propagation frequency.
const PROP_FREQ: i32 = 0;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = false;
/// Priority of the propagator (>= 0: before, < 0: after constraint
/// handlers).
const PROP_PRESOL_PRIORITY: i32 = 8000000;
/// Maximal number of propving rounds the propver participates in (-1: no
/// limit).
const PROP_PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolving method (fast, medium, or exhaustive).
const PROP_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST;

/*
 * Local methods
 */

/// `performDualfix`: perform dual presolving.
fn perform_dualfix(scip: &mut Scip, nfixedvars: &mut i32, unbounded: &mut bool, cutoff: &mut bool) -> ScipResult {
    use crate::var::{var_get_n_locks_down_type, var_get_n_locks_up_type, var_may_round_down, var_may_round_up};

    // Get active problem variables (the array, read live: fixings change it).
    let nvars = api::get_n_vars(scip);

    // Look for fixable variables; loop backwards, since a variable fixing can
    // change the current and the subsequent slots in the vars array.
    for v in (0..nvars).rev() {
        let var = scip.probs[TRANSPROB].vars[v];

        // Don't perform dual presolving operations on deleted variables.
        if scip.vars[var].deleted {
            continue;
        }

        // Ignore already fixed variables.
        if scip.set.is_eq(scip.vars[var].locdom.lb, scip.vars[var].locdom.ub) {
            continue;
        }

        let obj = scip.vars[var].obj;
        let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
        let mut bound;

        // If the objective coefficient of the variable is 0 and it may be
        // rounded both up and down, then fix it to the closest feasible value to
        // 0.
        if scip.set.is_zero(obj) && var_may_round_down(scip, var) && var_may_round_up(scip, var) {
            bound = glb;
            if scip.set.is_lt(bound, 0.0) {
                if scip.set.is_le(0.0, gub) {
                    bound = 0.0;
                } else {
                    // Try to take an integer value, only for polishing.
                    let roundbound = scip.set.floor(gub);
                    if roundbound < bound {
                        bound = gub;
                    } else {
                        bound = roundbound;
                    }
                }
            } else {
                // Try to take an integer value, only for polishing.
                let roundbound = scip.set.ceil(bound);
                if roundbound < gub {
                    bound = roundbound;
                }
            }
        } else {
            // If it is always possible to round variable in direction of objective
            // value, fix it to its proper bound.
            if var_may_round_down(scip, var) && !scip.set.is_negative(obj) {
                bound = glb;
                if scip.set.is_infinity(-bound) {
                    // Variable can be fixed to -infinity.
                    if scip.set.stage > Stage::Presolving {
                        // Fixing variables to infinity is not allowed after
                        // presolving, since LP-solvers cannot handle this
                        // consistently. We thus have to ignore this (should better be
                        // handled in presolving).
                        continue;
                    }
                    if scip.set.is_zero(obj) && var_get_n_locks_up_type(scip, var, LockType::Model) == 1 {
                        // Variable is only contained in one constraint: we hope that the
                        // corresponding constraint handler is clever enough to
                        // set/aggregate the variable to something more useful than
                        // -infinity and do nothing here.
                        continue;
                    }
                }
            } else if var_may_round_up(scip, var) && !scip.set.is_positive(obj) {
                bound = gub;
                if scip.set.is_infinity(bound) {
                    // Variable can be fixed to infinity.
                    if scip.set.stage > Stage::Presolving {
                        // Fixing variables to infinity is not allowed after
                        // presolving (see above).
                        continue;
                    }
                    if scip.set.is_zero(obj) && var_get_n_locks_down_type(scip, var, LockType::Model) == 1 {
                        // Variable is only contained in one constraint (see above).
                        continue;
                    }
                }
            } else {
                continue;
            }
        }

        if scip.set.is_infinity(bound.abs()) {
            if !scip.set.is_zero(obj) {
                *unbounded = true;
            }
            return Ok(());
        }

        // Apply the fixing.
        let (infeasible, _fixed) = scip_var::fix_var(scip, var, bound)?;
        if infeasible {
            *cutoff = true;
            return Ok(());
        }

        // SCIPfixVar only changes bounds if not already feaseq. Only if in
        // presolve and not probing, variables will always be fixed.
        *nfixedvars += 1;
    }

    Ok(())
}

/*
 * Callback methods
 */

/// `propPresolDualfix`: presolving method of propagator.
fn prop_presol_dualfix(
    scip: &mut Scip,
    _prop: PropId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if !scip_var::allow_strong_dual_reds(scip) {
        return Ok(());
    }

    let mut cutoff = false;
    let mut unbounded = false;
    let oldnfixedvars = counts.nfixedvars;

    perform_dualfix(scip, &mut counts.nfixedvars, &mut unbounded, &mut cutoff)?;

    // Evaluate propagation result.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if unbounded {
        ScipResultCode::Unbounded
    } else if counts.nfixedvars > oldnfixedvars {
        ScipResultCode::Success
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `propExecDualfix`: execution method of propagator.
fn prop_exec_dualfix(scip: &mut Scip, _prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Don't run in probing or in repropagation since this can lead to wrong
    // conclusion; do not run if propagation w.r.t. current objective is not
    // allowed.
    if api::in_probing(scip) || crate::tree::tree_in_repropagation(scip) || !scip_var::allow_strong_dual_reds(scip) {
        return Ok(());
    }

    let mut cutoff = false;
    let mut unbounded = false;
    let mut nfixedvars = 0;

    perform_dualfix(scip, &mut nfixedvars, &mut unbounded, &mut cutoff)?;

    // Evaluate propagation result.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if unbounded {
        ScipResultCode::Unbounded
    } else if nfixedvars > 0 {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `propCopyDualfix`: copy method for propagator plugins (called when SCIP
/// copies plugins).
fn prop_copy_dualfix(scip: &mut Scip, _sourcescip: &Scip, _prop: PropId) -> ScipResult {
    // Call inclusion method of propagator.
    include_prop_dualfix(scip)?;
    Ok(())
}

/// `SCIPincludePropDualfix`: creates the dual fixing propagator and includes
/// it in SCIP.
pub fn include_prop_dualfix(scip: &mut Scip) -> ScipResult {
    // Include propagator.
    let prop = api::include_prop_basic(scip, PROP_NAME, PROP_DESC, PROP_PRIORITY, PROP_FREQ, PROP_DELAY, PROP_TIMING, prop_exec_dualfix, None)?;
    scip.props[prop].propcopy = Some(prop_copy_dualfix);

    crate::prop::prop_set_presol(scip, prop, prop_presol_dualfix, PROP_PRESOL_PRIORITY, PROP_PRESOL_MAXROUNDS, PROP_PRESOLTIMING)?;

    Ok(())
}
