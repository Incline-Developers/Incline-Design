//! Port of `prop_rootredcost.c`: reduced cost strengthening using root node
//! reduced costs and the cutoff bound.
//!
//! This propagator works only for binary and integer variables. It uses the
//! best root reduced costs and the corresponding root LP objective values
//! to strengthen the global bounds against the current cutoff bound.

use std::any::Any;

use crate::{
    api,
    cons::*,
    def::INVALID,
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_var,
    types::{PropId, ScipResultCode, VarId},
    var::{var_get_best_root_lp_objval, var_get_best_root_redcost, var_get_best_root_sol},
};

/* propagator properties */
const PROP_NAME: &str = "rootredcost";
const PROP_DESC: &str = "reduced cost strengthening using root node reduced costs and the cutoff bound";
const PROP_TIMING: PropTiming = PROPTIMING_BEFORELP | PROPTIMING_AFTERLPLOOP;
/// Propagator priority.
const PROP_PRIORITY: i32 = 10000000;
/// Propagator frequency.
const PROP_FREQ: i32 = 1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = false;

/* default parameter values */
/// Should only binary variables be propagated?
const DEFAULT_ONLYBINARY: bool = false;
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
    /// Variables with non-zero root reduced cost.
    pub redcostvars: Vec<VarId>,
    /// Cutoff bound for which the root reduced costs were already processed.
    pub lastcutoffbound: f64,
    /// Number of variables with non-zero root reduced cost.
    pub nredcostvars: i32,
    /// Number of binary variables with non-zero root reduced cost.
    pub nredcostbinvars: i32,
    /// Index of first globally non-fixed binary variable.
    pub glbfirstnonfixed: i32,
    /// Is the propagator data initialized.
    pub initialized: bool,
    /// Should only binary variables be propagated?
    pub onlybinary: bool,
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

/// `propdataReset`: reset structure memember of propagator data structure.
fn propdata_reset(propdata: &mut PropData) {
    propdata.redcostvars = Vec::new();
    propdata.lastcutoffbound = INVALID;
    propdata.nredcostvars = 0;
    propdata.nredcostbinvars = 0;
    propdata.glbfirstnonfixed = 0;
    propdata.initialized = false;
}

/// `varCompRedcost`: compare variables with non-zero reduced cost w.r.t. (i)
/// the cutoff bound which would lead to a fixing, (ii) variable problem
/// index.
fn var_comp_redcost(scip: &Scip, var1: VarId, var2: VarId) -> i32 {
    // Collect sorting key for both variables.
    let key1 = var_get_best_root_redcost(scip, var1).abs() + var_get_best_root_lp_objval(scip, var1);
    let key2 = var_get_best_root_redcost(scip, var2).abs() + var_get_best_root_lp_objval(scip, var2);

    if key1 < key2 {
        return -1;
    } else if key1 > key2 {
        return 1;
    }

    // Second criterion: use the problem index; the problem index is unique.
    // That means the resulting sorting is unique.
    crate::var::var_compare(scip, var1, var2)
}

/// `countNonZeroRootRedcostVars`: counts the number of variables with
/// non-zero root reduced cost.
fn count_non_zero_root_redcost_vars(scip: &Scip, vars: &[VarId]) -> i32 {
    // Count number of variables with non-zero root reduced cost.
    vars.iter().filter(|&&v| !scip.set.is_dualfeas_zero(var_get_best_root_redcost(scip, v))).count() as i32
}

/// `propdataExit`: free propagator data.
fn propdata_exit(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Release all variables.
    let vars = std::mem::take(&mut pdm(scip, prop).redcostvars);
    let n = pd(scip, prop).nredcostvars as usize;
    for &v in &vars[..n] {
        let mut v = v;
        api::release_var(scip, &mut v)?;
    }

    // Free memory for non-zero reduced cost variables.
    propdata_reset(pdm(scip, prop));

    Ok(())
}

/// `propdataInit`: initializate the propagator.
fn propdata_init(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Check if the propagator data structure is already initialized.
    if pd(scip, prop).initialized {
        return Ok(());
    }

    // Get problem variables.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbinvars = api::get_n_bin_vars(scip) as usize;

    // Count binary variables with non-zero root reduced cost.
    let mut nredcostbinvars = count_non_zero_root_redcost_vars(scip, &vars[..nbinvars]);

    // Count non-binary variables with non-zero root reduced cost.
    let mut nredcostvars = count_non_zero_root_redcost_vars(scip, &vars[nbinvars..nvars]);

    nredcostvars += nredcostbinvars;

    let mut redcostvars = Vec::new();

    // Collect the variables with non-zero reduced costs.
    if nredcostvars > 0 {
        let mut k = 0usize;
        redcostvars = vec![crate::var::NONE; nredcostvars as usize];
        for &var in &vars {
            let redcost = var_get_best_root_redcost(scip, var);

            if scip.set.is_dualfeas_zero(redcost) {
                continue;
            }

            // Check if one of the non-binary variables is implicit binary.
            if k >= nredcostbinvars as usize && crate::var::var_is_binary(scip, var) {
                // Move the first non-binary variable to end of the array.
                redcostvars[k] = redcostvars[nredcostbinvars as usize];

                // Place the binary variable at the end of the binary section.
                redcostvars[nredcostbinvars as usize] = var;
                nredcostbinvars += 1;
            } else {
                redcostvars[k] = var;
            }

            // Captures the variable.
            crate::var::var_capture(scip, var);

            k += 1;

            // Check if already visited all variable with non-zero redcostective
            // coefficient.
            if k == nredcostvars as usize {
                break;
            }
        }

        // Sort binary variables with respect to their cutoff bound which would
        // lead to an fixing; this order can be used to efficiently propagate the
        // binary variables.
        crate::sort::sort_down(&mut redcostvars[..nredcostbinvars as usize], (), |a, b| var_comp_redcost(scip, *a, *b));
    }

    let inf = scip.set.infinity();
    let p = pdm(scip, prop);
    p.redcostvars = redcostvars;
    p.nredcostvars = nredcostvars;
    p.nredcostbinvars = nredcostbinvars;
    p.glbfirstnonfixed = 0;
    p.lastcutoffbound = inf;
    p.initialized = true;

    Ok(())
}

/// `propagateRootRedcostVar`: propagates the root reduced cost against the
/// cutoff bound for the given variable; returns `(infeasible, tightened)`.
fn propagate_root_redcost_var(scip: &mut Scip, var: VarId, cutoffbound: f64) -> ScipResult<(bool, bool)> {
    let rootredcost = var_get_best_root_redcost(scip, var);

    // SCIPisLPDualReliable should always return TRUE if the dual feasibility
    // check is enabled and the LP claims to have a dual feasible solution (see
    // the reference for why propagation does not depend on it).
    let rootsol = var_get_best_root_sol(scip, var);
    let rootlpobjval = var_get_best_root_lp_objval(scip, var);

    // Calculate reduced cost based bound.
    let newbd = rootsol + (cutoffbound - rootlpobjval) / rootredcost;

    if scip.set.is_dualfeas_positive(rootredcost) {
        // Strengthen upper bound.
        scip_var::tighten_var_ub_global(scip, var, newbd, false)
    } else {
        // Strengthen lower bound.
        scip_var::tighten_var_lb_global(scip, var, newbd, false)
    }
}

/// `propagateBinaryBestRootRedcost`: propagate binary variables with
/// non-zero root reduced cost.
fn propagate_binary_best_root_redcost(scip: &mut Scip, prop: PropId, cutoffbound: f64, nchgbds: &mut i32, cutoff: &mut bool) -> ScipResult {
    // The binary variables are stored in the beginning of the variable array;
    // these variables are sorted w.r.t. cutoff bound which would lead to a
    // fixing; that give us an abort criterion (see below).
    let (first, nbin) = (pd(scip, prop).glbfirstnonfixed, pd(scip, prop).nredcostbinvars);

    // Propagate binary variables.
    let mut v = first;
    while v < nbin {
        let var = pd(scip, prop).redcostvars[v as usize];

        // Check if the variables is already globally fixed; if so continue with
        // the next potential candidate.
        if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
            v += 1;
            continue;
        }

        // Try to tighten the variable bound.
        let tightened;
        (*cutoff, tightened) = propagate_root_redcost_var(scip, var, cutoffbound)?;

        if tightened {
            // The variable might not be globally fixed right away since this would
            // destroy the local internal data structure of a search node; the
            // bound change is in that case pending; hence we cannot assert that
            // the variable is globally fixed.
            *nchgbds += 1;
        } else {
            // The binary variables are sorted in non-increasing manner w.r.t. their
            // cutoff bound which would lead to a global fixing, abs(rootredcost) +
            // rootlpobjval: if one binary variable cannot be fixed anymore all the
            // remaining once can also not be fixed since these have only an smaller
            // or equal cutoff bound which would lead to global fixing. Hence, we
            // can break that loop.
            break;
        }
        v += 1;
    }

    // Store the index of the variable which is not globally fixed.
    pdm(scip, prop).glbfirstnonfixed = v;

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propFreeRootredcost`: destructor of propagator to free user data (called
/// when SCIP is exiting).
fn prop_free_rootredcost(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Free propagator data.
    scip.props[prop].propdata = None;
    Ok(())
}

/// `propExitsolRootredcost`: solving process deinitialization method of
/// propagator (called before branch and bound process data is freed).
fn prop_exitsol_rootredcost(scip: &mut Scip, prop: PropId, _restart: bool) -> ScipResult {
    // Reset propagator data structure.
    propdata_exit(scip, prop)
}

/// `propExecRootredcost`: execution method of propagator.
fn prop_exec_rootredcost(scip: &mut Scip, prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // In case we have a zero objective fucntion, we skip the root reduced cost
    // propagator.
    if api::get_n_obj_vars(scip) == 0 {
        return Ok(());
    }

    // Propagator can only be applied during solving stage.
    if scip.set.stage != Stage::Solving {
        return Ok(());
    }

    // The propagator should run in all nodes except the root node; for the root
    // node the redcost propagator does the job already.
    if api::get_depth(scip) < 1 {
        return Ok(());
    }

    // First check root LP objective value if it exists.
    let lpobjval = api::get_lp_root_objval(scip);
    if lpobjval == INVALID {
        return Ok(());
    }

    // Do not run in probing mode since this propagator changes global variable
    // bounds.
    if api::in_probing(scip) {
        return Ok(());
    }

    // Do not run if propagation w.r.t. objective is not allowed.
    if !scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    // Do nothing if active pricer are present and force flag is not TRUE.
    if !pd(scip, prop).force && api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // Get current cutoff bound.
    let cutoffbound = api::get_cutoffbound(scip);

    // Reduced cost strengthening can only be applied, if we have a finite upper
    // bound on the LP value.
    if scip.set.is_infinity(cutoffbound) {
        return Ok(());
    }

    // Initialize propagator data structure.
    propdata_init(scip, prop)?;

    if cutoffbound == pd(scip, prop).lastcutoffbound {
        return Ok(());
    }

    // Get variables.
    let nredcostvars = pd(scip, prop).nredcostvars;

    // Since no variables has non-zero reduced cost do nothing.
    if nredcostvars == 0 {
        return Ok(());
    }

    // Store cutoff bound to remember later that for that particular cutoff
    // bound the propagation was already preformed.
    pdm(scip, prop).lastcutoffbound = cutoffbound;

    *result = ScipResultCode::DidNotFind;

    let mut nchgbds = 0;
    let mut cutoff = false;

    // Propagate the binary variables with non-zero root reduced cost.
    propagate_binary_best_root_redcost(scip, prop, cutoffbound, &mut nchgbds, &mut cutoff)?;

    if !pd(scip, prop).onlybinary {
        // Check reduced costs for non-binary variables that were columns of the
        // root LP.
        let mut v = pd(scip, prop).nredcostbinvars;
        while v < nredcostvars && !cutoff {
            let var = pd(scip, prop).redcostvars[v as usize];

            // Try to tighten the variable bound.
            let tightened;
            (cutoff, tightened) = propagate_root_redcost_var(scip, var, cutoffbound)?;

            if tightened {
                nchgbds += 1;
            }
            v += 1;
        }
    }

    // Evaluate propagation results.
    if cutoff {
        // We are done with solving since the cutoff is w.r.t. a global bound
        // change; cutoff root node.
        let root = api::get_root_node(scip);
        api::cutoff_node(scip, root)?;
        *result = ScipResultCode::Cutoff;
    } else if nchgbds > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `propCopyRootredcost`: copy method for propagator plugins (called when SCIP
/// copies plugins).
fn prop_copy_rootredcost(scip: &mut Scip, _sourcescip: &Scip, _prop: PropId) -> ScipResult {
    // Call inclusion method of propagator.
    include_prop_rootredcost(scip)?;
    Ok(())
}

/// `SCIPincludePropRootredcost`: creates the root node reduced cost
/// strengthening propagator and includes it in SCIP.
pub fn include_prop_rootredcost(scip: &mut Scip) -> ScipResult {
    // Create rootredcost propagator data.
    let mut propdata = PropData {
        redcostvars: Vec::new(),
        lastcutoffbound: INVALID,
        nredcostvars: 0,
        nredcostbinvars: 0,
        glbfirstnonfixed: 0,
        initialized: false,
        onlybinary: false,
        force: false,
    };
    propdata_reset(&mut propdata);

    // Include propagator.
    let prop = api::include_prop_basic(
        scip,
        PROP_NAME,
        PROP_DESC,
        PROP_PRIORITY,
        PROP_FREQ,
        PROP_DELAY,
        PROP_TIMING,
        prop_exec_rootredcost,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;
    scip.props[prop].propcopy = Some(prop_copy_rootredcost);

    // Set optional callbacks via setter functions.
    scip.props[prop].propfree = Some(prop_free_rootredcost);
    scip.props[prop].propexitsol = Some(prop_exitsol_rootredcost);

    crate::paramset::paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/onlybinary"),
        "should only binary variables be propagated?",
        crate::data_target!(bool, props, prop, propdata, PropData, onlybinary),
        true,
        DEFAULT_ONLYBINARY,
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
