//! Port of `prop_redcost.c`: propagator using the LP reduced cost and the
//! cutoff bound.
//!
//! This propagator uses the reduced cost of an optimal solved LP relaxation
//! to propagate the variables against the cutoff bound; binary variables use
//! the root reduced costs at the root node, and optionally the implied
//! reduced costs.

use std::any::Any;

use crate::{
    api,
    cons::*,
    lpi::{BASESTAT_BASIC, BASESTAT_LOWER, BASESTAT_UPPER, BASESTAT_ZERO},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_var,
    types::{ColId, LpSolStat, PropId, ScipResultCode, VarId},
    var::{var_get_best_root_lp_objval, var_get_best_root_redcost, var_get_best_root_sol},
};

/* propagator properties */
const PROP_NAME: &str = "redcost";
const PROP_DESC: &str = "reduced cost strengthening propagator";
const PROP_TIMING: PropTiming = PROPTIMING_DURINGLPLOOP | PROPTIMING_AFTERLPLOOP;
/// Propagator priority.
const PROP_PRIORITY: i32 = 1000000;
/// Propagator frequency.
const PROP_FREQ: i32 = 1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = false;

/* default parameter values */
/// Should reduced cost fixing be also applied to continuous variables?
const DEFAULT_CONTINUOUS: bool = false;
/// Should implications be used to strength the reduced cost for binary
/// variables?
const DEFAULT_USEIMPLICS: bool = false;
/// Should the propagator be forced even if active pricer are present? Note
/// that the reductions are always valid, but installing an upper bound on
/// priced variables may lead to problems in pricing (existing variables at
/// their upper bound may be priced again since they may have negative
/// reduced costs).
const DEFAULT_FORCE: bool = false;

/*
 * Data structures
 */

/// `SCIP_PropData`: propagator data.
pub struct PropData {
    /// Should reduced cost fixing be also applied to continuous variables?
    pub continuous: bool,
    /// Maximum reduced cost of a single binary variable.
    pub maxredcost: f64,
    /// Are the implied reduced cost useful.
    pub usefullimplics: bool,
    /// Should implications be used to strength the reduced cost for binary
    /// variables?
    pub useimplics: bool,
    /// Should the propagator be forced even if active pricer are present?
    pub force: bool,
}

#[inline]
fn pd(scip: &Scip, prop: PropId) -> &PropData {
    scip.props[prop].propdata.as_ref().unwrap().downcast_ref::<PropData>().unwrap()
}

#[inline]
fn pdm(scip: &mut Scip, prop: PropId) -> &mut PropData {
    scip.props[prop].propdata.as_mut().unwrap().downcast_mut::<PropData>().unwrap()
}

/*
 * Local methods
 */

/// `propagateRootRedcostBinvar`: propagate the given binary variable/column
/// using the root reduced cost stored in the SCIP internal data structures
/// and check if the implications can be useful. Depending on that
/// implications are used or not used during the search to strength the
/// reduced costs.
fn propagate_root_redcost_binvar(scip: &mut Scip, prop: PropId, var: VarId, col: ColId, cutoffbound: f64, nchgbds: &mut i32) -> ScipResult {
    // Skip binary variable if it is locally fixed.
    if scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5 {
        return Ok(());
    }

    let rootredcost = var_get_best_root_redcost(scip, var);
    let rootsol = var_get_best_root_sol(scip, var);
    let rootlpobjval = var_get_best_root_lp_objval(scip, var);

    if scip.set.is_dualfeas_zero(rootredcost) {
        return Ok(());
    }

    if rootsol > 0.5 {
        // Update maximum reduced cost of a single binary variable.
        let p = pdm(scip, prop);
        p.maxredcost = crate::def::max(p.maxredcost, -rootredcost);

        if rootlpobjval - rootredcost > cutoffbound {
            scip_var::chg_var_lb(scip, var, 1.0)?;
            *nchgbds += 1;
            return Ok(());
        }
    } else {
        // Update maximum reduced cost of a single binary variable.
        let p = pdm(scip, prop);
        p.maxredcost = crate::def::max(p.maxredcost, rootredcost);

        if rootlpobjval + rootredcost > cutoffbound {
            scip_var::chg_var_ub(scip, var, 0.0)?;
            *nchgbds += 1;
            return Ok(());
        }
    }

    // Evaluate if the implications are useful; the implications are seen to be
    // useful if they provide an increase for the root reduced costs.
    if !pd(scip, prop).usefullimplics {
        let mut lbredcost = api::get_var_impl_redcost(scip, var, false);
        let mut ubredcost = api::get_var_impl_redcost(scip, var, true);

        match scip.cols[col].basisstatus {
            BASESTAT_LOWER => {
                ubredcost -= api::get_var_redcost(scip, var);
            }
            BASESTAT_UPPER => {
                lbredcost -= api::get_var_redcost(scip, var);
            }
            _ => {}
        }

        pdm(scip, prop).usefullimplics = (lbredcost < 0.0) || (ubredcost > 0.0);
    }

    Ok(())
}

/// `propagateRedcostBinvar`: propagate the given binary variable/column
/// using the reduced cost.
fn propagate_redcost_binvar(scip: &mut Scip, prop: PropId, var: VarId, col: ColId, requiredredcost: f64, nchgbds: &mut i32, cutoff: &mut bool) -> ScipResult {
    // Skip binary variable if it is locally fixed.
    if scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5 {
        return Ok(());
    }

    // First use the redcost cost to fix the binary variable.
    match scip.cols[col].basisstatus {
        BASESTAT_LOWER => {
            let redcost = api::get_var_redcost(scip, var);

            if redcost > requiredredcost {
                scip_var::chg_var_ub(scip, var, 0.0)?;
                *nchgbds += 1;
                return Ok(());
            }
        }
        BASESTAT_UPPER => {
            let redcost = api::get_var_redcost(scip, var);

            if -redcost > requiredredcost {
                scip_var::chg_var_lb(scip, var, 1.0)?;
                *nchgbds += 1;
                return Ok(());
            }
        }
        BASESTAT_BASIC => return Ok(()),
        BASESTAT_ZERO => return Ok(()),
        _ => {
            // Invalid basis state.
            return Err(Retcode::InvalidData);
        }
    }

    // Second, if the implications should be used and if the implications are
    // seen to be promising use the implied reduced costs to fix the binary
    // variable.
    if pd(scip, prop).useimplics && pd(scip, prop).usefullimplics {
        // Collect implied reduced costs if the variable would be fixed to its
        // lower bound.
        let lbredcost = api::get_var_impl_redcost(scip, var, false);

        // Collect implied reduced costs if the variable would be fixed to its
        // upper bound.
        let ubredcost = api::get_var_impl_redcost(scip, var, true);

        if -lbredcost > requiredredcost && ubredcost > requiredredcost {
            *cutoff = true;
        } else if -lbredcost > requiredredcost {
            scip_var::chg_var_lb(scip, var, 1.0)?;
            *nchgbds += 1;
        } else if ubredcost > requiredredcost {
            scip_var::chg_var_ub(scip, var, 0.0)?;
            *nchgbds += 1;
        }

        // Update maximum reduced cost of a single binary variable.
        let p = pdm(scip, prop);
        p.maxredcost = crate::def::max3(p.maxredcost, -lbredcost, ubredcost);
    }

    Ok(())
}

/// `propagateRedcostVar`: propagate the given none binary variable/column
/// using the reduced cost.
fn propagate_redcost_var(scip: &mut Scip, var: VarId, col: ColId, lpobjval: f64, cutoffbound: f64, nchgbds: &mut i32) -> ScipResult {
    match scip.cols[col].basisstatus {
        BASESTAT_LOWER => {
            let redcost = api::get_col_redcost(scip, col);

            if scip.set.is_dualfeas_positive(redcost) {
                let oldlb = scip.vars[var].locdom.lb;
                let oldub = scip.vars[var].locdom.ub;

                if scip.set.is_feas_lt(oldlb, oldub) {
                    // Calculate reduced cost based bound.
                    let mut newub = (cutoffbound - lpobjval) / redcost + oldlb;

                    // Check, if new bound is good enough:
                    //  - integer variables: take all possible strengthenings
                    //  - continuous variables: strengthening must cut part of the
                    //    variable's dynamic range, and at least 20% of the current
                    //    domain
                    let strengthen = if crate::var::var_is_integral(scip, var) {
                        newub = scip_var::adjusted_var_ub(scip, var, newub);
                        newub < oldub - 0.5
                    } else {
                        newub < scip.cols[col].maxprimsol && newub <= 0.2 * oldlb + 0.8 * oldub
                    };

                    if strengthen {
                        // Strengthen upper bound.
                        scip_var::chg_var_ub(scip, var, newub)?;
                        *nchgbds += 1;
                    }
                }
            }
        }
        BASESTAT_BASIC => {}
        BASESTAT_UPPER => {
            let redcost = api::get_col_redcost(scip, col);

            if scip.set.is_dualfeas_negative(redcost) {
                let oldlb = scip.vars[var].locdom.lb;
                let oldub = scip.vars[var].locdom.ub;

                if scip.set.is_feas_lt(oldlb, oldub) {
                    // Calculate reduced cost based bound.
                    let mut newlb = (cutoffbound - lpobjval) / redcost + oldub;

                    // Check, if new bound is good enough (see above).
                    let strengthen = if crate::var::var_is_integral(scip, var) {
                        newlb = scip_var::adjusted_var_lb(scip, var, newlb);
                        newlb > oldlb + 0.5
                    } else {
                        newlb > scip.cols[col].minprimsol && newlb >= 0.8 * oldlb + 0.2 * oldub
                    };

                    // Check, if new bound is good enough: at least 20% strengthening
                    // for continuous variables.
                    if strengthen {
                        // Strengthen lower bound.
                        scip_var::chg_var_lb(scip, var, newlb)?;
                        *nchgbds += 1;
                    }
                }
            }
        }
        BASESTAT_ZERO => {}
        _ => {
            // Invalid basis state.
            return Err(Retcode::InvalidData);
        }
    }

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propFreeRedcost`: destructor of propagator to free user data (called
/// when SCIP is exiting).
fn prop_free_redcost(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Free propagator data.
    scip.props[prop].propdata = None;
    Ok(())
}

/// `propInitsolRedcost`: solving process initialization method of
/// propagator (called when branch and bound process is about to begin).
fn prop_initsol_redcost(scip: &mut Scip, prop: PropId) -> ScipResult {
    let p = pdm(scip, prop);
    p.usefullimplics = false;
    p.maxredcost = 0.0;

    Ok(())
}

/// `propExecRedcost`: reduced cost propagation method for an LP solution.
fn prop_exec_redcost(scip: &mut Scip, prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // In case we have a zero objective function, we skip the reduced cost
    // propagator.
    if api::get_n_obj_vars(scip) == 0 {
        return Ok(());
    }

    // Propagator can only be applied during solving stage.
    if scip.set.stage < Stage::Solving {
        return Ok(());
    }

    // We cannot apply reduced cost fixing, if we want to solve exactly (exact
    // solving is not ported).

    // Only call propagator, if the current node has an LP.
    if !api::has_current_node_lp(scip) {
        return Ok(());
    }

    // Only call propagator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call propagator, if the current LP is a valid relaxation.
    if !api::is_lp_relax(scip) {
        return Ok(());
    }

    // We cannot apply reduced cost strengthening, if no simplex basis is
    // available.
    if !api::is_lp_sol_basic(scip) {
        return Ok(());
    }

    // Do not run if propagation w.r.t. objective is not allowed.
    if !scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    // Get current cutoff bound.
    let cutoffbound = api::get_cutoffbound(scip);

    // Reduced cost strengthening can only be applied, if we have a finite
    // cutoff.
    if scip.set.is_infinity(cutoffbound) {
        return Ok(());
    }

    // Get LP columns.
    let cols = api::get_lp_cols(scip);
    let ncols = cols.len();

    // Do nothing if the LP has no columns (is empty).
    if ncols == 0 {
        return Ok(());
    }

    // Do nothing if active pricer are present and force flag is not TRUE.
    if !pd(scip, prop).force && api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // Check if all integral variables are fixed and the continuous variables
    // should not be propagated.
    if !pd(scip, prop).continuous && api::get_n_pseudo_branch_cands(scip) == 0 {
        return Ok(());
    }

    // Get LP objective value.
    let lpobjval = api::get_lp_objval(scip);

    // Check if binary variables should be propagated.
    let propbinvars = (api::get_depth(scip) == 0) || (cutoffbound - lpobjval < 5.0 * pd(scip, prop).maxredcost);

    // Skip the propagator if the problem has only binary variables and those
    // should not be propagated.
    if !propbinvars && api::get_n_vars(scip) == api::get_n_bin_vars(scip) {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;
    let mut cutoff = false;
    let mut nchgbds = 0;

    // Compute the required reduced cost which are needed for a binary variable
    // to be fixed.
    let requiredredcost = cutoffbound - lpobjval;

    // Check reduced costs for non-basic columns.
    let mut c = 0;
    while c < ncols && !cutoff {
        let col = cols[c];
        let var = scip.cols[col].var;
        c += 1;

        // Skip continuous variables in case the corresponding parameter is set.
        if !pd(scip, prop).continuous && !crate::var::var_is_integral(scip, var) {
            continue;
        }

        if crate::var::var_is_binary(scip, var) {
            if propbinvars {
                if api::get_depth(scip) == 0 {
                    propagate_root_redcost_binvar(scip, prop, var, col, cutoffbound, &mut nchgbds)?;
                } else {
                    propagate_redcost_binvar(scip, prop, var, col, requiredredcost, &mut nchgbds, &mut cutoff)?;
                }
            }
        } else {
            propagate_redcost_var(scip, var, col, lpobjval, cutoffbound, &mut nchgbds)?;
        }
    }

    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if nchgbds > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `propCopyRedcost`: copy method for propagator plugins (called when SCIP
/// copies plugins).
fn prop_copy_redcost(scip: &mut Scip, _sourcescip: &Scip, _prop: PropId) -> ScipResult {
    // Call inclusion method of propagator.
    include_prop_redcost(scip)?;
    Ok(())
}

/// `SCIPincludePropRedcost`: creates the redcost propagator and includes it
/// in SCIP.
pub fn include_prop_redcost(scip: &mut Scip) -> ScipResult {
    // Create redcost propagator data (the block memory is uninitialised in C
    // until the parameters and initsol set it).
    let propdata = PropData {
        continuous: DEFAULT_CONTINUOUS,
        maxredcost: 0.0,
        usefullimplics: false,
        useimplics: DEFAULT_USEIMPLICS,
        force: DEFAULT_FORCE,
    };

    // Include propagator.
    let prop = api::include_prop_basic(
        scip,
        PROP_NAME,
        PROP_DESC,
        PROP_PRIORITY,
        PROP_FREQ,
        PROP_DELAY,
        PROP_TIMING,
        prop_exec_redcost,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;
    scip.props[prop].propcopy = Some(prop_copy_redcost);

    // Set optional callbacks via setter functions.
    scip.props[prop].propinitsol = Some(prop_initsol_redcost);
    scip.props[prop].propfree = Some(prop_free_redcost);

    // Add redcost propagator parameters.
    crate::paramset::paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/continuous"),
        "should reduced cost fixing be also applied to continuous variables?",
        crate::data_target!(bool, props, prop, propdata, PropData, continuous),
        false,
        DEFAULT_CONTINUOUS,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/useimplics"),
        "should implications be used to strength the reduced cost for binary variables?",
        crate::data_target!(bool, props, prop, propdata, PropData, useimplics),
        false,
        DEFAULT_USEIMPLICS,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/force"),
        "should the propagator be forced even if active pricer are present?",
        crate::data_target!(bool, props, prop, propdata, PropData, force),
        true,
        DEFAULT_FORCE,
        None,
    )?;

    Ok(())
}
