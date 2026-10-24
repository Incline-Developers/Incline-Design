//! Port of `prop_genvbounds.c`: generalized variable bounds propagator.
//!
//! Generalized variable bounds `+/- x_i >= sum_j a_j x_j + c * cutoffbound +
//! constant` are added by other plugins (OBBT) through [`gen_vbound_add`].
//! The debug-solution check is not ported, and neither is
//! `propagating/genvbounds/propasconss` (default off).

use std::{any::Any, collections::HashMap};

use crate::{
    api,
    cons::{PRESOLTIMING_FAST, PROPTIMING_ALWAYS, PresolCounts, PresolTiming, PropTiming},
    cvec::CVec,
    event::{EVENTTYPE_LBTIGHTENED, EVENTTYPE_UBTIGHTENED, Event},
    misc::Digraph,
    paramset::paramset_add_bool,
    prop::PROPTIMING_NONE,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_var,
    types::{BoundType, ConfType, EventhdlrId, PropId, ScipResultCode, VarId, VarStatus},
    var::{BdChgIdx, NONE},
};

const PROP_NAME: &str = "genvbounds";
const PROP_DESC: &str = "generalized variable bounds propagator";
const PROP_TIMING: PropTiming = PROPTIMING_ALWAYS;
/// Propagator priority.
const PROP_PRIORITY: i32 = 3000000;
/// Propagator frequency.
const PROP_FREQ: i32 = 1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = false;
/// Priority of the presolving method (>= 0: before, < 0: after constraint
/// handlers); combined with presolvers.
const PROP_PRESOL_PRIORITY: i32 = -2000000;
/// Timing of the presolving method (fast, medium, or exhaustive).
const PROP_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST;
/// Maximal number of presolving rounds the presolver participates in (-1: no
/// limit).
const PROP_PRESOL_MAXROUNDS: i32 = -1;
/// Apply global propagation?
const DEFAULT_GLOBAL_PROPAGATION: bool = true;
/// Apply genvbounds in root node if no new incumbent was found?
const DEFAULT_PROPAGATE_IN_ROOT_NODE: bool = true;
/// Sort genvbounds and wait for bound change events? (otherwise all
/// genvbounds are applied in each node)
const DEFAULT_SORT: bool = true;
/// Should genvbounds be transformed to (linear) constraints?
const DEFAULT_PROPASCONSS: bool = false;

const EVENTHDLR_NAME: &str = "genvbounds";
const EVENTHDLR_DESC: &str = "event handler for generalized variable bounds propagator";

/*
 * Data structures
 */

/// `GENVBOUND`: genvbound data.
struct GenVBound {
    /// Variables x_j occurring in this generalized variable bound.
    vars: Vec<VarId>,
    /// Variable x_i.
    var: VarId,
    /// Coefficients a_j of the variables listed in vars.
    coefs: Vec<f64>,
    /// Constant term in generalized variable bound.
    constant: f64,
    /// Cutoff bound's coefficient.
    cutoffcoef: f64,
    /// Index of this genvbound in genvboundstore array.
    index: i32,
    /// Type of bound provided by the genvbound, lower/upper if +/- x_i on
    /// left-hand side.
    boundtype: BoundType,
    /// Contains a relaxation-only variable.
    relaxonly: bool,
}

impl GenVBound {
    fn ncoefs(&self) -> usize {
        self.vars.len()
    }
}

/// `SCIP_EventData`: starting indices data structure.
struct EventData {
    /// Variable.
    var: VarId,
    /// The first indices of genvbounds in components that are impacted by a
    /// change of this bound.
    startindices: Vec<i32>,
    /// The components corresponding to startindices.
    startcomponents: Vec<i32>,
}

/// `SCIP_PropData`: propagator data.
#[derive(Default)]
struct PropData {
    /// Genvbounds, by id; `genvboundstore` holds ids into it (the C pointers).
    genvbounds: Vec<Option<GenVBound>>,
    /// Array to store genvbounds (`NULL`: None); fast access is provided by
    /// hashmaps lbgenvbounds and ubgenvbounds.
    genvboundstore: Option<Vec<usize>>,
    /// Lower bound event data (`NULL`: None).
    lbevents: Option<Vec<EventData>>,
    /// Upper bound event data.
    ubevents: Vec<EventData>,
    /// Genvbounds propagator event handler.
    eventhdlr: EventhdlrId,
    /// Lower bound genvbounds by variable.
    lbgenvbounds: HashMap<VarId, usize>,
    /// Upper bound genvbounds by variable.
    ubgenvbounds: HashMap<VarId, usize>,
    /// Lower bound event data by variable.
    lbeventsmap: HashMap<VarId, usize>,
    /// Upper bound event data by variable.
    ubeventsmap: HashMap<VarId, usize>,
    /// Fast access to startindices array (component + 1 -> index + 1;
    /// `NULL`: None).
    startmap: Option<HashMap<i32, i32>>,
    /// Pointer to genvbounds propagator.
    prop: PropId,
    /// Last node number where events for starting indices were caught.
    lastnodenumber: i64,
    /// Artificial variable representing primal cutoff bound.
    cutoffboundvar: VarId,
    /// The components starting indices in genvboundstore array (`NULL`:
    /// None); the entry componentsstart[ncomponents] is equal to ngenvbounds.
    componentsstart: Option<Vec<i32>>,
    /// Indices of components where local propagation should start.
    startindices: Vec<i32>,
    /// Components corresponding to indices stored in startindices array.
    startcomponents: Vec<i32>,
    /// Indices of components where global propagation, i.e., propagation of
    /// an improved primal bound, should start.
    gstartindices: Vec<i32>,
    /// Components corresponding to indices stored in gstartindices array.
    gstartcomponents: Vec<i32>,
    /// Cutoff bound's value last time genvbounds propagator was called.
    lastcutoff: f64,
    /// Number of genvbounds stored in genvboundstore array.
    ngenvbounds: i32,
    /// Number of components in genvboundstore array.
    ncomponents: i32,
    /// Number of indices stored in startindices array.
    nindices: i32,
    /// Number of indices stored in gstartindices array.
    ngindices: i32,
    /// Number of data entries in lbevents array.
    nlbevents: i32,
    /// Number of data entries in ubevents array.
    nubevents: i32,
    /// Whether array genvboundstore is topologically sorted.
    issorted: bool,
    /// Apply global propagation?
    global: bool,
    /// Apply genvbounds in root node if no new incumbent was found?
    propinrootnode: bool,
    /// Sort genvbounds and wait for bound change events?
    sort: bool,
    /// Should genvbounds be transformed to (linear) constraints?
    propasconss: bool,
    /// Initial propagator timing.
    inittiming: PropTiming,
}

fn pd(scip: &Scip, prop: PropId) -> &PropData {
    scip.props[prop].propdata.as_ref().unwrap().downcast_ref::<PropData>().unwrap()
}

fn pdm(scip: &mut Scip, prop: PropId) -> &mut PropData {
    scip.props[prop].propdata.as_mut().unwrap().downcast_mut::<PropData>().unwrap()
}

/// The genvbound stored at position `pos` of genvboundstore.
fn gvb(scip: &Scip, prop: PropId, pos: usize) -> &GenVBound {
    let p = pd(scip, prop);
    p.genvbounds[p.genvboundstore.as_ref().unwrap()[pos]].as_ref().unwrap()
}

/// `SCIP_EVENTDATA*` as the event's data: lower bound events are even,
/// upper bound events odd.
fn event_data_id(boundtype: BoundType, i: usize) -> usize {
    2 * i + usize::from(boundtype == BoundType::Upper)
}

/*
 * Local methods
 */

/// `getCutoffboundGenVBound`: returns correct cutoff bound value.
fn get_cutoffbound_gen_vbound(scip: &Scip) -> f64 {
    // The cutoff bound is valid w.r.t. the current objective function in the
    // transformed problem; during presolving, however, the objective function
    // can change (e.g., when a variable is fixed, its contribution in the
    // objective is subtracted from the cutoff bound and added to the
    // objective offset); we solve this by transforming the contribution of the
    // cutoff bound in the generalized variable bound to the original problem
    // as described in function SCIPgenVBoundAdd().
    (api::get_cutoffbound(scip) + api::get_trans_objoffset(scip)) * api::get_trans_objscale(scip)
}

/// `getGenVBound`: returns corresponding genvbound id if there is one.
fn get_gen_vbound(p: &PropData, var: VarId, boundtype: BoundType) -> Option<usize> {
    let hashmap = if boundtype == BoundType::Lower { &p.lbgenvbounds } else { &p.ubgenvbounds };
    hashmap.get(&var).copied()
}

/// `getGenVBoundsMinActivity`: calculates the minactivity of a linear
/// combination of variables stored in an array.
fn get_gen_vbounds_min_activity(scip: &Scip, vars: &[VarId], coefs: &[f64], global: bool) -> f64 {
    let mut minval = 0.0;

    for i in 0..vars.len() {
        let v = &scip.vars[vars[i]];

        // Get global or local bound.
        let bound = if global {
            if coefs[i] > 0.0 { v.glbdom.lb } else { v.glbdom.ub }
        } else if coefs[i] > 0.0 {
            v.locdom.lb
        } else {
            v.locdom.ub
        };

        // With infinite bounds we cannot compute a valid minactivity and
        // return minus infinity.
        if scip.set.is_infinity(bound) || scip.set.is_infinity(-bound) {
            return -scip.set.infinity();
        }

        // Add contribution to minactivity.
        minval += coefs[i] * bound;
    }

    minval
}

/// `getGenVBoundsMinActivityConflict`: calculates the minactivity of a
/// linear combination of variables stored in the current conflict set.
fn get_gen_vbounds_min_activity_conflict(scip: &Scip, vars: &[VarId], coefs: &[f64], bdchgidx: Option<&BdChgIdx>) -> f64 {
    let mut minval = 0.0;

    for i in 0..vars.len() {
        let mut bound;

        if coefs[i] > 0.0 {
            // Get bound at current bound change.
            bound = api::get_var_lb_at_index(scip, vars[i], bdchgidx, true);

            // If bdchgidx is not NULL, use the possibly tighter bound already
            // in the current conflict set.
            if bdchgidx.is_some() && api::get_conflict_var_lb(scip, vars[i]) > bound {
                bound = api::get_conflict_var_lb(scip, vars[i]);
            }
        } else {
            // Get bound at current bound change.
            bound = api::get_var_ub_at_index(scip, vars[i], bdchgidx, true);

            // If bdchgidx is not NULL, use the possibly tighter bound already
            // in the current conflict set.
            if bdchgidx.is_some() && api::get_conflict_var_ub(scip, vars[i]) < bound {
                bound = api::get_conflict_var_ub(scip, vars[i]);
            }
        }

        // With infinite bounds we cannot compute a valid minactivity and
        // return minus infinity.
        if scip.set.is_infinity(bound) || scip.set.is_infinity(-bound) {
            return -scip.set.infinity();
        }

        // Add contribution to minactivity.
        minval += coefs[i] * bound;
    }

    minval
}

/// `getGenVBoundsBound`: returns a valid bound given by a generalized
/// variable bound.
fn get_gen_vbounds_bound(scip: &Scip, genvbound: &GenVBound, global: bool) -> f64 {
    let mut boundval = get_gen_vbounds_min_activity(scip, &genvbound.vars, &genvbound.coefs, global);

    if scip.set.is_infinity(-boundval) {
        return if genvbound.boundtype == BoundType::Lower {
            -scip.set.infinity()
        } else {
            scip.set.infinity()
        };
    }

    if genvbound.cutoffcoef != 0.0 {
        boundval += genvbound.cutoffcoef * get_cutoffbound_gen_vbound(scip);
    }

    boundval += genvbound.constant;

    if genvbound.boundtype == BoundType::Upper {
        boundval *= -1.0;
    }

    boundval
}

/// `createStartingData`: allocate local and global startindices,
/// startcomponents and startmap.
fn create_starting_data(p: &mut PropData) {
    debug_assert!(p.startmap.is_none());
    debug_assert!(p.nindices == -1);
    debug_assert!(p.ngindices == -1);
    debug_assert!(p.ngenvbounds >= 1);
    debug_assert!(p.ncomponents >= 1);

    // Allocate memory for arrays.
    let n = p.ncomponents as usize;
    p.startindices = vec![0; n];
    p.startcomponents = vec![0; n];
    p.gstartindices = vec![0; n];
    p.gstartcomponents = vec![0; n];

    // Create hashmap.
    p.startmap = Some(HashMap::new());

    p.nindices = 0;
    p.ngindices = 0;
}

/// `freeStartingData`: free local and global startindices, startcomponents
/// and startmap.
fn free_starting_data(p: &mut PropData) {
    if p.startmap.is_some() {
        p.startindices = Vec::new();
        p.startcomponents = Vec::new();
        p.startmap = None;
        p.nindices = -1;

        p.gstartindices = Vec::new();
        p.gstartcomponents = Vec::new();
        p.ngindices = -1;
    }
}

/// `fillGlobalStartingData`.
fn fill_global_starting_data(scip: &Scip, prop: PropId) -> (Vec<i32>, Vec<i32>) {
    let p = pd(scip, prop);
    debug_assert!(p.ngindices == 0);

    let componentsstart = p.componentsstart.as_ref().unwrap();
    let mut gstartcomponents = Vec::new();
    let mut gstartindices = Vec::new();

    for i in 0..p.ncomponents as usize {
        for j in componentsstart[i]..componentsstart[i + 1] {
            let genvbound = gvb(scip, prop, j as usize);
            if !scip.set.is_zero(genvbound.cutoffcoef) {
                gstartcomponents.push(i as i32);
                gstartindices.push(j);

                // Go to next component.
                break;
            }
        }
    }

    (gstartcomponents, gstartindices)
}

/// `resetLocalStartingData`: resets local starting data.
fn reset_local_starting_data(p: &mut PropData) {
    debug_assert!(p.nindices >= 0);
    p.startmap.as_mut().unwrap().clear();
    p.nindices = 0;
}

/// `freeComponentsData`: frees sorted components data.
fn free_components_data(p: &mut PropData) {
    if p.componentsstart.is_some() {
        debug_assert!(p.ncomponents > 0);
        p.componentsstart = None;
        p.ncomponents = -1;
    }
}

/// `freeGenVBound`: frees memory allocated for a generalized variable bound.
fn free_gen_vbound(scip: &mut Scip, prop: PropId, id: usize) -> ScipResult {
    let genvbound = pdm(scip, prop).genvbounds[id].take().unwrap();

    // Release variables.
    for mut v in genvbound.vars {
        api::release_var(scip, &mut v)?;
    }
    let mut var = genvbound.var;
    api::release_var(scip, &mut var)?;

    Ok(())
}

/// `freeGenVBounds`: helper function to release all genvbounds.
fn free_gen_vbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    if pd(scip, prop).genvboundstore.is_none() {
        return Ok(());
    }

    // Disable genvbounds propagator.
    if pd(scip, prop).ngenvbounds >= 1 {
        scip.props[prop].timingmask = PROPTIMING_NONE;
    }

    // Free genvbounds.
    for i in (0..pd(scip, prop).ngenvbounds as usize).rev() {
        let id = pd(scip, prop).genvboundstore.as_ref().unwrap()[i];
        free_gen_vbound(scip, prop, id)?;
    }

    let p = pdm(scip, prop);

    // Free genvboundstore hashmaps.
    p.lbgenvbounds = HashMap::new();
    p.ubgenvbounds = HashMap::new();

    // Free genvboundstore array.
    p.genvboundstore = None;
    p.genvbounds = Vec::new();

    // Set the number of genvbounds to zero.
    p.ngenvbounds = 0;

    // Free componentsstart array.
    free_components_data(p);

    // Free starting indices data.
    free_starting_data(p);

    // Release the cutoffboundvar and undo the locks.
    if p.cutoffboundvar != NONE {
        let mut cutoffboundvar = p.cutoffboundvar;
        api::add_var_locks_type(scip, cutoffboundvar, crate::types::LockType::Model, -1, -1)?;
        api::release_var(scip, &mut cutoffboundvar)?;
        pdm(scip, prop).cutoffboundvar = NONE;
    }

    Ok(())
}

/// `freeGenVBoundsRelaxOnly`: helper function to release relax-only
/// genvbounds.
fn free_gen_vbounds_relax_only(scip: &mut Scip, prop: PropId) -> ScipResult {
    if pd(scip, prop).genvboundstore.is_none() {
        return Ok(());
    }

    // Free genvbounds.
    let mut freedgenvbound = false;
    let mut i = 0;
    while i < pd(scip, prop).ngenvbounds as usize {
        let (id, relaxonly, boundtype, var) = {
            let g = gvb(scip, prop, i);
            (pd(scip, prop).genvboundstore.as_ref().unwrap()[i], g.relaxonly, g.boundtype, g.var)
        };
        if relaxonly {
            let p = pdm(scip, prop);
            if boundtype == BoundType::Lower {
                p.lbgenvbounds.remove(&var);
            } else {
                p.ubgenvbounds.remove(&var);
            }

            free_gen_vbound(scip, prop, id)?;
            let p = pdm(scip, prop);
            let last = p.ngenvbounds as usize - 1;
            if i != last {
                let store = p.genvboundstore.as_mut().unwrap();
                store[i] = store[last];
                let moved = store[i];
                p.genvbounds[moved].as_mut().unwrap().index = i as i32;
            }
            p.ngenvbounds -= 1;
            p.genvboundstore.as_mut().unwrap().truncate(p.ngenvbounds as usize);

            p.issorted = false;
            freedgenvbound = true;
        } else {
            i += 1;
        }
    }

    if freedgenvbound {
        let p = pdm(scip, prop);

        // Free componentsstart array.
        free_components_data(p);

        // Free starting indices data.
        free_starting_data(p);
    }

    // Disable genvbounds propagator.
    if pd(scip, prop).ngenvbounds == 0 {
        scip.props[prop].timingmask = PROPTIMING_NONE;
    }

    Ok(())
}

/// `resolveGenVBoundPropagation`: resolves propagation of lower bound on
/// +/- left-hand side variable of a generalized variable bound; returns
/// whether the explanation was successful.
fn resolve_gen_vbound_propagation(scip: &mut Scip, prop: PropId, pos: usize, bdchgidx: Option<&BdChgIdx>, boundval: &mut f64) -> ScipResult<bool> {
    let (vars, coefs, cutoffcoef, constant) = {
        let g = gvb(scip, prop, pos);
        (g.vars.clone(), g.coefs.clone(), g.cutoffcoef, g.constant)
    };
    let nvars = vars.len();

    // Subtract constant terms from bound value.
    let mut tmpboundval = *boundval;
    tmpboundval -= cutoffcoef * get_cutoffbound_gen_vbound(scip);
    tmpboundval -= constant;

    // Compute minimal activity; if bdchgidx is NULL, we create the initial
    // conflict and use local bounds.
    let minactivity = get_gen_vbounds_min_activity_conflict(scip, &vars, &coefs, bdchgidx);

    // A genvbound might have been replaced since the propagation took place,
    // hence we have to check that the current genvbound can explain the
    // propagation at the given bound change index; note that by now, with
    // smaller cutoff bound, we might even perform a stronger propagation.
    if scip.set.is_lt(minactivity, tmpboundval) {
        return Ok(false);
    }

    let mut slack = if minactivity - tmpboundval >= 0.0 { minactivity - tmpboundval } else { 0.0 };

    // Add variables on the right-hand side as reasons for propagation.
    for i in 0..nvars {
        // Coefficient is positive.
        if coefs[i] > 0.0 {
            // Get bound at current bound change.
            let lbatindex = api::get_var_lb_at_index(scip, vars[i], bdchgidx, true);

            // Get bound already enforced by conflict set.
            let conflictlb = api::get_conflict_var_lb(scip, vars[i]);

            // If bound is already enforced by conflict set we do not need to
            // add the bound change; since we used the tighest bound already
            // when computing the initial minactivity, the slack is already
            // correct.
            if !scip.set.is_le(lbatindex, conflictlb) {
                // Compute relaxed bound that would suffice to explain the
                // bound change.
                let mut relaxedlb = lbatindex - (slack / coefs[i]);

                // Add variable to conflict set.
                api::add_conflict_relaxed_lb(scip, vars[i], bdchgidx, relaxedlb)?;

                // Get new bound of variable in conflict set; after possible
                // bound widening in SCIPaddConflictLbRelaxed(), it should be
                // between conflictlb and lbatindex.
                relaxedlb = api::get_conflict_var_lb(scip, vars[i]);

                // Update slack and ensure that its nonegative.
                slack -= coefs[i] * (lbatindex - relaxedlb);
                slack = if slack >= 0.0 { slack } else { 0.0 };
            }
        }
        // Coefficient is negative.
        else {
            // Get bound at current bound change.
            let ubatindex = api::get_var_ub_at_index(scip, vars[i], bdchgidx, true);

            // Get bound already enforced by conflict set.
            let conflictub = api::get_conflict_var_ub(scip, vars[i]);

            // If bound is already enforced by conflict set we do not need to
            // add the bound change; since we used the tighest bound already
            // when computing the initial minactivity, the slack is already
            // correct.
            if !scip.set.is_ge(ubatindex, conflictub) {
                // Compute relaxed bound that would suffice to explain the
                // bound change.
                let mut relaxedub = ubatindex - (slack / coefs[i]);

                // Add variable to conflict set.
                api::add_conflict_relaxed_ub(scip, vars[i], bdchgidx, relaxedub)?;

                // Get new bound of variable in conflict set; after possible
                // bound widening in SCIPaddConflictUbRelaxed(), it should be
                // between conflictub and ubatindex.
                relaxedub = api::get_conflict_var_ub(scip, vars[i]);

                // Update slack and ensure that its nonegative.
                slack -= coefs[i] * (ubatindex - relaxedub);
                slack = if slack >= 0.0 { slack } else { 0.0 };
            }
        }
    }

    // If slack is positive, return increased boundval.
    if scip.set.is_positive(slack) {
        tmpboundval += slack;
    }

    // Add constant terms again.
    tmpboundval += cutoffcoef * get_cutoffbound_gen_vbound(scip);
    tmpboundval += constant;

    // Boundval should not have been decreased; if this happened nevertheless,
    // maybe due to numerical errors, we quit without success.
    if scip.set.is_lt(tmpboundval, *boundval) {
        return Ok(false);
    }

    // Return widened boundval.
    *boundval = tmpboundval;

    Ok(true)
}

/// `analyzeGenVBoundConflict`: create initial conflict.
fn analyze_gen_vbound_conflict(scip: &mut Scip, prop: PropId, pos: usize) -> ScipResult {
    // Check if conflict analysis is applicable.
    if !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis.
    let cutoffinvolved = !scip.set.is_infinity(api::get_cutoffbound(scip).abs());
    api::init_conflict_analysis(scip, ConfType::Propagation, cutoffinvolved)?;

    let (var, boundtype) = {
        let g = gvb(scip, prop, pos);
        (g.var, g.boundtype)
    };
    let feastol = scip.set.feastol();
    let cmax = |a: f64, b: f64| if a >= b { a } else { b };

    // Left-hand side variable >= ...
    if boundtype == BoundType::Lower {
        // Get minimal right-hand side bound that leads to infeasibility; first
        // try with a factor of 2 for robustness.
        let ublocal = scip.vars[var].locdom.ub;
        let mut bound = ublocal.abs();
        let mut infeasthreshold = cmax(bound, 1.0) * 2.0 * feastol;
        bound = ublocal + infeasthreshold;

        // Add right-hand side variables that force the lower bound of the
        // left-hand side variable above its upper bound to conflict set.
        let mut success = resolve_gen_vbound_propagation(scip, prop, pos, None, &mut bound)?;

        // If infeasibility cannot be proven with the tighter bound, try with
        // actual bound.
        if !success {
            bound = ublocal.abs();
            infeasthreshold = cmax(bound, 1.0) * feastol;
            bound = ublocal + infeasthreshold;

            success = resolve_gen_vbound_propagation(scip, prop, pos, None, &mut bound)?;
            success = success && scip.set.is_feas_gt(bound, scip.vars[var].locdom.ub);
        }

        // Compute upper bound on left-hand side variable that leads to
        // infeasibility.
        bound -= infeasthreshold;
        success = success && scip.set.is_ge(bound, scip.vars[var].locdom.ub);

        // Initial reason could not be constructed, maybe due to numerics; do
        // not apply conflict analysis.
        if !success {
            return Ok(());
        }

        // If bound is already enforced by conflict set we do not have to add
        // it.
        if !scip.set.is_ge(bound, api::get_conflict_var_ub(scip, var)) {
            api::add_conflict_relaxed_ub(scip, var, None, bound)?;
        }
    }
    // Left-hand side variable <= ..., i.e., - left-hand side variable >= ...
    else {
        // Get minimal right-hand side bound that leads to infeasibility; try
        // with a factor of 2 first for robustness.
        let lblocal = scip.vars[var].locdom.lb;
        let mut bound = lblocal.abs();
        let mut infeasthreshold = cmax(bound, 1.0) * 2.0 * feastol;
        bound = -lblocal + infeasthreshold;

        // Add right-hand side variables that force the upper bound of the
        // left-hand side variable below its lower bound to conflict set.
        let mut success = resolve_gen_vbound_propagation(scip, prop, pos, None, &mut bound)?;

        // If infeasibility cannot be proven with the tighter bound, try with
        // actual bound.
        if !success {
            bound = lblocal.abs();
            infeasthreshold = cmax(bound, 1.0) * feastol;
            bound = -lblocal + infeasthreshold;

            success = resolve_gen_vbound_propagation(scip, prop, pos, None, &mut bound)?;
            success = success && scip.set.is_feas_lt(-bound, scip.vars[var].locdom.lb);
        }

        // Compute lower bound on left-hand side variable that leads to
        // infeasibility.
        bound = -bound + infeasthreshold;
        success = success && scip.set.is_le(bound, scip.vars[var].locdom.lb);

        // Initial reason could not be constructed, maybe due to numerics; do
        // not apply conflict analysis.
        if !success {
            return Ok(());
        }

        // If bound is already enforced by conflict set we do not have to add
        // it.
        if !scip.set.is_le(bound, api::get_conflict_var_lb(scip, var)) {
            api::add_conflict_relaxed_lb(scip, var, None, bound)?;
        }
    }

    // Analyze the conflict.
    api::analyze_conflict(scip, 0)?;

    Ok(())
}

/// `applyGenVBound`: apply propagation for one generalized variable bound;
/// also if the left-hand side variable is locally fixed, we compute the
/// right-hand side minactivity to possibly detect infeasibility.
fn apply_gen_vbound(scip: &mut Scip, prop: PropId, pos: usize, global: bool, result: &mut ScipResultCode, nchgbds: Option<&mut i32>) -> ScipResult {
    let (boundval, var, boundtype, ncoefs, index) = {
        let g = gvb(scip, prop, pos);
        (get_gen_vbounds_bound(scip, g, global), g.var, g.boundtype, g.ncoefs(), g.index)
    };

    if scip.set.is_infinity(boundval.abs()) {
        return Ok(());
    }

    let (infeas, tightened);

    // Tighten bound globally.
    if global || ncoefs == 0 {
        (infeas, tightened) = if boundtype == BoundType::Lower {
            scip_var::tighten_var_lb_global(scip, var, boundval, false)?
        } else {
            scip_var::tighten_var_ub_global(scip, var, boundval, false)?
        };
    }
    // Tighten bound locally and start conflict analysis in case of
    // infeasibility; as inferinfo we pass the index of the genvbound that was
    // used for propagation.
    else if boundtype == BoundType::Lower {
        (infeas, tightened) = scip_var::infer_var_lb_prop(scip, var, boundval, prop, index, false)?;

        // Initialize conflict analysis if infeasible.
        if infeas {
            analyze_gen_vbound_conflict(scip, prop, pos)?;
        }
    } else {
        (infeas, tightened) = scip_var::infer_var_ub_prop(scip, var, boundval, prop, index, false)?;

        // Initialize conflict analysis if infeasible.
        if infeas {
            analyze_gen_vbound_conflict(scip, prop, pos)?;
        }
    }

    // Handle result.
    if infeas {
        *result = ScipResultCode::Cutoff;
    } else if tightened {
        *result = if scip.set.stage == Stage::Presolving {
            ScipResultCode::Success
        } else {
            ScipResultCode::ReducedDom
        };
        if let Some(nchgbds) = nchgbds {
            *nchgbds += 1;
        }
    }

    Ok(())
}

/// `dropAndFreeEvents`: drops all events caught by genvbounds propagator and
/// frees their data (with `freeAllEventData`).
fn drop_and_free_events(scip: &mut Scip, prop: PropId) -> ScipResult {
    let eventhdlr = pd(scip, prop).eventhdlr;

    if let Some(lbevents) = pdm(scip, prop).lbevents.take() {
        let ubevents = std::mem::take(&mut pdm(scip, prop).ubevents);

        for i in (0..lbevents.len()).rev() {
            // Drop event.
            api::drop_var_event(scip, lbevents[i].var, EVENTTYPE_LBTIGHTENED, eventhdlr, event_data_id(BoundType::Lower, i), -1)?;
        }

        for i in (0..ubevents.len()).rev() {
            // Drop event.
            api::drop_var_event(scip, ubevents[i].var, EVENTTYPE_UBTIGHTENED, eventhdlr, event_data_id(BoundType::Upper, i), -1)?;
        }

        // Free event data.
        let p = pdm(scip, prop);
        p.lbeventsmap = HashMap::new();
        p.ubeventsmap = HashMap::new();
        p.nlbevents = -1;
        p.nubevents = -1;
    }

    Ok(())
}

/// `addEventData` (with `getEventData`): adds an event to the event array
/// lbevents (if boundtype == lower) or ubevents (if boundtype == upper).
fn add_event_data(p: &mut PropData, var: VarId, startindex: i32, startcomponent: i32, boundtype: BoundType) {
    // Get eventdata entry.
    let (hashmap, events) = if boundtype == BoundType::Lower {
        (&mut p.lbeventsmap, p.lbevents.as_mut().unwrap())
    } else {
        (&mut p.ubeventsmap, &mut p.ubevents)
    };
    let idx = match hashmap.get(&var) {
        Some(&idx) => idx,
        None => {
            // Set up new eventdata entry and store it in eventarray.
            events.push(EventData {
                var,
                startindices: Vec::new(),
                startcomponents: Vec::new(),
            });
            hashmap.insert(var, events.len() - 1);
            events.len() - 1
        }
    };
    let eventdata = &mut events[idx];

    if eventdata.startcomponents.last() == Some(&startcomponent) {
        // If there is already a starting index for startcomponent stored at
        // the last entry of eventdata->startindices, it should be smaller;
        // this relies on the implementation of setUpEvents(), calling
        // addEventData() in topological order.
        debug_assert!(*eventdata.startindices.last().unwrap() < startindex);
    } else {
        // Append starting information.
        eventdata.startcomponents.push(startcomponent);
        eventdata.startindices.push(startindex);
    }
}

/// `setUpEvents`.
fn set_up_events(scip: &mut Scip, prop: PropId) -> ScipResult {
    debug_assert!(pd(scip, prop).lbevents.is_none());
    debug_assert!(pd(scip, prop).issorted);

    // Allocate lbevents, ubevents, and their hashmaps.
    {
        let p = pdm(scip, prop);
        p.lbevents = Some(Vec::new());
        p.ubevents = Vec::new();
        p.lbeventsmap = HashMap::new();
        p.ubeventsmap = HashMap::new();
    }

    // Loop over all components of genvboundstore.
    let ncomponents = pd(scip, prop).ncomponents as usize;
    for i in 0..ncomponents {
        let (start, end) = {
            let cs = pd(scip, prop).componentsstart.as_ref().unwrap();
            (cs[i], cs[i + 1])
        };

        // Loop over all genvbounds in this component.
        for j in start..end {
            let (vars, coefs) = {
                let g = gvb(scip, prop, j as usize);
                (g.vars.clone(), g.coefs.clone())
            };

            // Loop over all coefficients in this genvbound.
            for k in 0..vars.len() {
                let boundtype = if scip.set.is_positive(coefs[k]) { BoundType::Lower } else { BoundType::Upper };
                add_event_data(pdm(scip, prop), vars[k], j, i as i32, boundtype);
            }
        }
    }

    let eventhdlr = {
        let p = pdm(scip, prop);
        p.nlbevents = p.lbevents.as_ref().unwrap().len() as i32;
        p.nubevents = p.ubevents.len() as i32;
        p.eventhdlr
    };

    // Register lower bound events.
    for i in 0..pd(scip, prop).nlbevents as usize {
        let var = pd(scip, prop).lbevents.as_ref().unwrap()[i].var;
        api::catch_var_event(scip, var, EVENTTYPE_LBTIGHTENED, eventhdlr, event_data_id(BoundType::Lower, i), None)?;
    }

    // Register upper bound events.
    for i in 0..pd(scip, prop).nubevents as usize {
        let var = pd(scip, prop).ubevents[i].var;
        api::catch_var_event(scip, var, EVENTTYPE_UBTIGHTENED, eventhdlr, event_data_id(BoundType::Upper, i), None)?;
    }

    Ok(())
}

/// `sortGenVBounds`: performs a topological sort on genvboundstore array.
///
/// The genvbounds graph is defined as follows: Given two genvbounds
///
///   (genvbound1)      c1 * x_i1 >= RHS1
///
/// and
///
///   (genvbound2)      c2 * x_i2 >= RHS2,
///
/// there is an arc from genvbound1 to genvbound2 iff c1 = +1 and x_i1
/// appears with positive coefficient in RHS2 or c1 = -1 and x_i1 appears with
/// negative coefficient in RHS2; in this case, a bound change of x_i1 deduced
/// from genvbound1 improves genvbound2's minactivity in RHS2.
///
/// The method computes the strongly connected components and sorts them
/// topologically. The order of the nodes in an strongly connected component
/// is arbitrary.
fn sort_gen_vbounds(scip: &mut Scip, prop: PropId) {
    debug_assert!(pd(scip, prop).componentsstart.is_none());

    let ngenvbounds = pd(scip, prop).ngenvbounds as usize;

    // Create digraph.
    let mut graph = Digraph::new(ngenvbounds as i32);

    // Add outgoing arcs for each genvbound.
    for i in 0..ngenvbounds {
        let p = pd(scip, prop);
        let genvbound = gvb(scip, prop, i);

        for j in 0..genvbound.ncoefs() {
            if scip.set.is_positive(genvbound.coefs[j]) && p.lbgenvbounds.contains_key(&genvbound.vars[j]) {
                let from = p.genvbounds[p.lbgenvbounds[&genvbound.vars[j]]].as_ref().unwrap().index;
                graph.add_arc(from, i as i32);
            } else if scip.set.is_negative(genvbound.coefs[j]) && p.ubgenvbounds.contains_key(&genvbound.vars[j]) {
                let from = p.genvbounds[p.ubgenvbounds[&genvbound.vars[j]]].as_ref().unwrap().index;
                graph.add_arc(from, i as i32);
            }
        }
    }

    // Perform the topological sort.
    let ncomponents = graph.compute_undirected_components(1, None);
    graph.topo_sort_components();

    // Allocate memory for genvboundssorted and componentsstart array.
    let mut genvboundssorted = Vec::with_capacity(ngenvbounds);
    let mut componentsstart = vec![0i32; ncomponents as usize + 1];

    // Allocate memory for strong component arrays.
    let nnodes = graph.get_n_nodes() as usize;
    let mut strongcomponents = vec![0i32; nnodes];
    let mut strongcompstartidx = vec![0i32; nnodes + 1];

    // Compute sorted genvbounds array, fill componentsstart array.
    let store = pd(scip, prop).genvboundstore.as_ref().unwrap().clone();
    componentsstart[ncomponents as usize] = ngenvbounds as i32;
    for i in 0..ncomponents {
        let nodes = graph.get_component(i);
        let nnodes = nodes.len();
        componentsstart[i as usize] = genvboundssorted.len() as i32;

        // Compute the strong components of the i-th undirected component.
        if nnodes > 2 {
            graph.compute_directed_components(i, &mut strongcomponents, &mut strongcompstartidx);

            for j in 0..nnodes {
                // Take the nodes at the end of the strong components array
                // first to respect the topological order of the different
                // strong components.
                let node = strongcomponents[nnodes - j - 1];
                genvboundssorted.push(store[node as usize]);
            }
        } else {
            for &node in nodes {
                genvboundssorted.push(store[node as usize]);
            }
        }
    }
    debug_assert_eq!(genvboundssorted.len(), ngenvbounds);

    // Copy sorted genvbounds into genvboundstore.
    let p = pdm(scip, prop);
    for (i, &id) in genvboundssorted.iter().enumerate() {
        p.genvbounds[id].as_mut().unwrap().index = i as i32;
    }
    p.genvboundstore = Some(genvboundssorted);
    p.ncomponents = ncomponents;
    p.componentsstart = Some(componentsstart);

    // Remember genvboundstore as sorted.
    p.issorted = true;
}

/// `applyGenVBounds`: apply propagation of generalized variable bounds.
fn apply_gen_vbounds(scip: &mut Scip, prop: PropId, global: bool, result: &mut ScipResultCode, mut nchgbds: Option<&mut i32>) -> ScipResult {
    if *result == ScipResultCode::DidNotRun {
        *result = ScipResultCode::DidNotFind;
    }

    // If the genvbounds are not sorted, i.e. if root node processing has not
    // been finished, yet, we just propagate in the order in which they have
    // been added to genvboundstore.
    if !pd(scip, prop).issorted {
        let mut j = 0;
        while j < pd(scip, prop).ngenvbounds as usize && *result != ScipResultCode::Cutoff {
            if !crate::var::var_is_active(scip, gvb(scip, prop, j).var) {
                // @todo resolve multiaggregation in exitpre
            } else {
                apply_gen_vbound(scip, prop, j, global, result, nchgbds.as_deref_mut())?;
            }
            j += 1;
        }

        return Ok(());
    }

    // Otherwise, we propagate only components affected by the latest bound
    // changes.
    let (startingcomponents, startingindices, nindices) = {
        let p = pd(scip, prop);
        if global {
            (p.gstartcomponents.clone(), p.gstartindices.clone(), p.ngindices)
        } else {
            (p.startcomponents.clone(), p.startindices.clone(), p.nindices)
        }
    };

    let mut i = 0;
    while i < nindices as usize && *result != ScipResultCode::Cutoff {
        let end = pd(scip, prop).componentsstart.as_ref().unwrap()[startingcomponents[i] as usize + 1];
        let mut j = startingindices[i];
        while j < end && *result != ScipResultCode::Cutoff {
            if !crate::var::var_is_active(scip, gvb(scip, prop, j as usize).var) {
                // @todo resolve multiaggregation in exitpre
            } else {
                apply_gen_vbound(scip, prop, j as usize, global, result, nchgbds.as_deref_mut())?;
            }
            j += 1;
        }
        i += 1;
    }

    // We dont want to run again caused by this starting data.
    if !global {
        reset_local_starting_data(pdm(scip, prop));
    }

    Ok(())
}

/// `initPropdata`: initialize propagator data.
fn init_propdata(p: &mut PropData) {
    debug_assert!(p.ngenvbounds == -1);

    // Init genvboundstore.
    p.genvboundstore = Some(Vec::new());
    p.genvbounds = Vec::new();
    p.ngenvbounds = 0;

    // Init genvboundstore hashmaps.
    p.lbgenvbounds = HashMap::new();
    p.ubgenvbounds = HashMap::new();
}

/// `addNewGenVBound`: adds a new genvbound to genvboundstore array and sets
/// a hashmap entry.
fn add_new_gen_vbound(scip: &mut Scip, prop: PropId, id: usize) {
    // Enable genvbounds propagator.
    if pd(scip, prop).ngenvbounds == 0 {
        scip.props[prop].timingmask = pd(scip, prop).inittiming;
    }

    // New index is propdata->ngenvbounds.
    let p = pdm(scip, prop);
    let (var, boundtype) = {
        let g = p.genvbounds[id].as_ref().unwrap();
        (g.var, g.boundtype)
    };
    if boundtype == BoundType::Lower {
        p.lbgenvbounds.insert(var, id);
    } else {
        p.ubgenvbounds.insert(var, id);
    }
    p.genvboundstore.as_mut().unwrap().push(id);
    p.genvbounds[id].as_mut().unwrap().index = p.ngenvbounds;
    p.ngenvbounds += 1;
}

/// `execGenVBounds`: runs propagation routine.
fn exec_gen_vbounds(scip: &mut Scip, prop: PropId, result: &mut ScipResultCode, local: bool, mut nchgbds: Option<&mut i32>) -> ScipResult {
    // We only sort after the root node is finished; this avoids having to sort
    // again after adding more genvbounds; if the genvbounds are not sorted,
    // we will simply propagate all of them in the order given.
    if pd(scip, prop).sort && !api::in_probing(scip) && api::get_depth(scip) > 0 {
        if !pd(scip, prop).issorted {
            *result = ScipResultCode::DidNotFind;

            // Drop and free old events.
            drop_and_free_events(scip, prop)?;

            // Free old starting data.
            free_starting_data(pdm(scip, prop));

            // Free sorted components data.
            free_components_data(pdm(scip, prop));

            // Sort genvbounds.
            sort_gen_vbounds(scip, prop);

            // Create starting data.
            create_starting_data(pdm(scip, prop));

            // Fill global starting data.
            let (gstartcomponents, gstartindices) = fill_global_starting_data(scip, prop);
            let p = pdm(scip, prop);
            p.ngindices = gstartindices.len() as i32;
            p.gstartcomponents = gstartcomponents;
            p.gstartindices = gstartindices;
        }

        // Set up new events to catch (if not done so far).
        if pd(scip, prop).lbevents.is_none() {
            set_up_events(scip, prop)?;
        }
    }

    // Apply global propagation if primal bound has improved.
    if pd(scip, prop).global && api::get_depth(scip) > 0 && scip.set.is_feas_lt(api::get_cutoffbound(scip), pd(scip, prop).lastcutoff) {
        if pd(scip, prop).ngindices > 0 {
            apply_gen_vbounds(scip, prop, true, result, nchgbds.as_deref_mut())?;
        }

        // Within the tree, the objective function should not change anymore,
        // hence the cutoff bound should be a stable point of reference.
        pdm(scip, prop).lastcutoff = api::get_cutoffbound(scip);
    }

    // Apply local propagation if allowed.
    if local && *result != ScipResultCode::Cutoff {
        // Check if local propagation in root node is allowed.
        if api::get_depth(scip) > 0 || pd(scip, prop).propinrootnode {
            // If genvbounds are already sorted, check if bound change events
            // were caught; otherwise apply all genvbounds.
            let issorted = pd(scip, prop).issorted;
            if !issorted || (scip.nodes[crate::tree::tree_get_current_node(scip)].number == pd(scip, prop).lastnodenumber && pd(scip, prop).nindices > 0) {
                apply_gen_vbounds(scip, prop, false, result, nchgbds)?;
            }
        }
    }

    Ok(())
}

/*
 * Public methods
 */

/// `SCIPgenVBoundAdd`: adds a generalized variable bound to the genvbounds
/// propagator; if there is already a genvbound for the bound `boundtype` of
/// variable `var`, it will be replaced.
#[allow(clippy::too_many_arguments)]
pub fn gen_vbound_add(scip: &mut Scip, genvboundprop: PropId, vars: &[VarId], var: VarId, coefs: &[f64], coefcutoffbound: f64, constant: f64, boundtype: BoundType) -> ScipResult {
    let prop = genvboundprop;
    let ncoefs = vars.len();

    if coefcutoffbound > 0.0 || scip.set.is_infinity(-constant) {
        log::error!("cannot create generalized variable bound from invalid data");
        return Err(Retcode::InvalidData);
    }

    // Initialize propdata if not done yet.
    if pd(scip, prop).genvboundstore.is_none() {
        init_propdata(pdm(scip, prop));
    }

    let existing = get_gen_vbound(pd(scip, prop), var, boundtype);
    let newgenvbound = existing.is_none();

    // Release previous variables.
    if let Some(id) = existing {
        let oldvars = std::mem::take(&mut pdm(scip, prop).genvbounds[id].as_mut().unwrap().vars);
        for mut v in oldvars {
            api::release_var(scip, &mut v)?;
        }
    }

    // Check if there already is a genvbound corresponding to this bound,
    // freeing its data and overwriting it (the arrays are replaced in every
    // case).
    let id = match existing {
        Some(id) => {
            let g = pdm(scip, prop).genvbounds[id].as_mut().unwrap();
            g.vars = vars[..ncoefs].to_vec();
            g.coefs = coefs[..ncoefs].to_vec();
            id
        }
        None => {
            // Allocate memory for genvbound data.
            let p = pdm(scip, prop);
            p.genvbounds.push(Some(GenVBound {
                vars: vars[..ncoefs].to_vec(),
                var: NONE,
                coefs: coefs[..ncoefs].to_vec(),
                constant: 0.0,
                cutoffcoef: 0.0,
                index: -1,
                boundtype,
                relaxonly: false,
            }));
            p.genvbounds.len() - 1
        }
    };

    // Set up data for genvbound.
    let relaxonly = scip.vars[var].relaxationonly;
    {
        let g = pdm(scip, prop).genvbounds[id].as_mut().unwrap();
        g.boundtype = boundtype;
        g.var = var;
        g.constant = constant;
        g.relaxonly = relaxonly;
    }

    // Capture variables and check for relax-only vars.
    for &v in &vars[..ncoefs] {
        crate::var::var_capture(scip, v);
        if scip.vars[v].relaxationonly {
            pdm(scip, prop).genvbounds[id].as_mut().unwrap().relaxonly = true;
        }
    }
    if newgenvbound {
        crate::var::var_capture(scip, var);
    }

    // The cutoff bound is valid w.r.t. the current objective function in the
    // transformed problem; during presolving, however, the objective function
    // can change (e.g., when a variable is fixed, its contribution in the
    // objective is subtracted from the cutoff bound and added to the
    // objective offset); we solve this by transforming the contribution of the
    // cutoff bound in the generalized variable bound to the original problem
    // as follows:
    //
    //    +/- var >= ... + z * SCIPgetCutoffbound() + constant
    //
    // becomes
    //
    //    +/- var >= ... + (z / SCIPgetTransObjscale()) * origcutoffbound + (constant - z * SCIPgetTransObjoffset())
    //
    // with SCIPgetCutoffbound() = origcutoffbound / SCIPgetTransObjscale() -
    // SCIPgetTransObjoffset(); in the propagation later, we will use
    // (SCIPgetCutoffbound() + SCIPgetTransObjoffset()) *
    // SCIPgetTransObjscale(), see function getCutoffboundGenVBound().
    let (objscale, objoffset) = (api::get_trans_objscale(scip), api::get_trans_objoffset(scip));
    let isneg = scip.set.is_negative(coefcutoffbound);
    {
        let g = pdm(scip, prop).genvbounds[id].as_mut().unwrap();
        if isneg {
            g.cutoffcoef = coefcutoffbound / objscale;
            g.constant -= coefcutoffbound * objoffset;
        } else {
            g.cutoffcoef = 0.0;
        }
    }

    // If genvbound is not overwritten, create a new entry in genvboundstore.
    if newgenvbound {
        add_new_gen_vbound(scip, prop, id);
    }

    // Mark genvbounds array to be re-sorted.
    pdm(scip, prop).issorted = false;

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propCopyGenvbounds`: copy method for propagator plugins.
fn prop_copy_genvbounds(scip: &mut Scip, _source: &Scip, _prop: PropId) -> ScipResult {
    include_prop_genvbounds(scip)
}

/// `propInitGenvbounds`: initialization method of propagator.
fn prop_init_genvbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    let infinity = scip.set.infinity();
    let timingmask = scip.props[prop].timingmask;
    let p = pdm(scip, prop);

    p.genvboundstore = None;
    p.genvbounds = Vec::new();
    p.lbevents = None;
    p.ubevents = Vec::new();
    p.lbgenvbounds = HashMap::new();
    p.ubgenvbounds = HashMap::new();
    p.lbeventsmap = HashMap::new();
    p.ubeventsmap = HashMap::new();
    p.startmap = None;
    p.componentsstart = None;
    p.startindices = Vec::new();
    p.startcomponents = Vec::new();
    p.gstartindices = Vec::new();
    p.gstartcomponents = Vec::new();
    p.lastcutoff = infinity;
    p.lastnodenumber = -1;
    p.cutoffboundvar = NONE;
    p.ngenvbounds = -1;
    p.ncomponents = -1;
    p.nindices = -1;
    p.ngindices = -1;
    p.nlbevents = -1;
    p.nubevents = -1;
    p.issorted = false;

    p.prop = prop;

    // Store genvbounds timing.
    p.inittiming = timingmask;

    // Disable genvbounds propagator, enabled when genvbounds are added.
    scip.props[prop].timingmask = PROPTIMING_NONE;

    Ok(())
}

/// `propPresolGenvbounds`: presolving method of propagator.
fn prop_presol_genvbounds(
    scip: &mut Scip,
    prop: PropId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do not run if no genvbounds were added yet.
    if pd(scip, prop).ngenvbounds < 1 {
        return Ok(());
    }

    if !scip_var::allow_strong_dual_reds(scip) {
        return Ok(());
    }

    // Propagate.
    exec_gen_vbounds(scip, prop, result, true, Some(&mut counts.nchgbds))
}

/// `propInitpreGenvbounds`: presolving initialization method of propagator.
fn prop_initpre_genvbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Lock the variable because it should not be deleted after a restart.
    let cutoffboundvar = pd(scip, prop).cutoffboundvar;
    if cutoffboundvar != NONE {
        api::add_var_locks_type(scip, cutoffboundvar, crate::types::LockType::Model, 1, 1)?;
    }

    Ok(())
}

/// `propExitpreGenvbounds`: presolving deinitialization method of
/// propagator; removes fixed, aggregated, negated, and multi-aggregated
/// variables from right-hand side.
fn prop_exitpre_genvbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    // There should be no events on the right-hand side variables.
    debug_assert!(pd(scip, prop).lbevents.is_none());

    let mut i = 0;
    while i < pd(scip, prop).ngenvbounds.max(0) as usize {
        let id = pd(scip, prop).genvboundstore.as_ref().unwrap()[i];

        // Store variables of the genvbound to release them properly.
        let (oldvars, oldcoefs, mut constant) = {
            let g = pd(scip, prop).genvbounds[id].as_ref().unwrap();
            (g.vars.clone(), g.coefs.clone(), g.constant)
        };

        // Replace non-active by active variables and update constant; note
        // that this may result in coefficients where SCIPisZero() is true;
        // this should not create any problems.
        let mut newvars = CVec(oldvars.clone());
        let mut newcoefs = CVec(oldcoefs);
        let mut ncoefs = oldvars.len() as i32;
        let mut requiredsize = 0;
        let varssize = ncoefs;
        crate::var::var_get_active_representatives(scip, &mut newvars, &mut newcoefs, &mut ncoefs, varssize, &mut constant, &mut requiredsize)?;

        // If space was not enough we need to resize the buffers.
        if requiredsize > ncoefs {
            newvars.0.resize(requiredsize as usize, NONE);
            newcoefs.0.resize(requiredsize as usize, 0.0);
            crate::var::var_get_active_representatives(scip, &mut newvars, &mut newcoefs, &mut ncoefs, requiredsize, &mut constant, &mut requiredsize)?;
        }
        newvars.0.truncate(ncoefs as usize);
        newcoefs.0.truncate(ncoefs as usize);

        // Capture new and release old variables.
        for &v in &newvars.0 {
            crate::var::var_capture(scip, v);
        }
        for mut v in oldvars {
            api::release_var(scip, &mut v)?;
        }

        let (var, cutoffcoef, boundtype) = {
            let g = pdm(scip, prop).genvbounds[id].as_mut().unwrap();
            g.vars = newvars.0;
            g.coefs = newcoefs.0;
            g.constant = constant;
            (g.var, g.cutoffcoef, g.boundtype)
        };

        // If the resulting genvbound is trivial, remove it; we remove all
        // genvbounds with an aggregated or multi-aggregated genvbound->var;
        // tightening aggregated variables might lead to some asserts in tree.c
        // if the active variable has been already tightened (see !398).
        //
        // @todo replace aggregated variable by their active part
        let status = scip.vars[var].varstatus;
        if (ncoefs == 0 && scip.set.is_zero(cutoffcoef)) || status == VarStatus::MultAggr || status == VarStatus::Aggregated {
            // Remove genvbound from hashmap.
            let p = pdm(scip, prop);
            if boundtype == BoundType::Lower {
                p.lbgenvbounds.remove(&var);
            } else {
                p.ubgenvbounds.remove(&var);
            }

            // Free genvbound and fill gap.
            free_gen_vbound(scip, prop, id)?;
            let p = pdm(scip, prop);
            p.ngenvbounds -= 1;

            // Move the last genvbound to the i-th position.
            let n = p.ngenvbounds as usize;
            let store = p.genvboundstore.as_mut().unwrap();
            if i < n {
                store[i] = store[n];
                let moved = store[i];
                p.genvbounds[moved].as_mut().unwrap().index = i as i32;

                // Mark genvbounds array to be re-sorted.
                p.issorted = false;
            }
            p.genvboundstore.as_mut().unwrap().truncate(n);
        } else {
            i += 1;
        }
    }

    // Disable genvbounds propagator.
    if pd(scip, prop).ngenvbounds == 0 {
        scip.props[prop].timingmask = PROPTIMING_NONE;
    }

    Ok(())
}

/// `propExitGenvbounds`: deinitialization method of propagator.
fn prop_exit_genvbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Free remaining genvbounds.
    free_gen_vbounds(scip, prop)?;

    // Reset genvbounds timing.
    scip.props[prop].timingmask = pd(scip, prop).inittiming;

    Ok(())
}

/// `propExecGenvbounds`: execution method of propagator.
fn prop_exec_genvbounds(scip: &mut Scip, prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    debug_assert!(pd(scip, prop).ngenvbounds >= 1);

    // Do not run if propagation w.r.t. current objective is not allowed.
    if !scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    // Update upper bound of the cutoffboundvar.
    if pd(scip, prop).cutoffboundvar != NONE {
        unimplemented!("propagating/genvbounds/propasconss");
    }

    // Add the genvbounds in the genvboundstore as constraints to the problem;
    // afterwards clear the genvboundstore.
    if pd(scip, prop).propasconss {
        unimplemented!("propagating/genvbounds/propasconss (createConstraints)");
    }

    // Propagate locally and globally.
    let local = !api::in_probing(scip);
    exec_gen_vbounds(scip, prop, result, local, None)?;

    // When called in presolving stage the result is set to SCIP_SUCCESS
    // instead of SCIP_REDUCEDDOM, this is corrected here.
    if *result == ScipResultCode::Success {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `propRespropGenvbounds`: propagation conflict resolving method of
/// propagator.
#[allow(clippy::too_many_arguments)]
fn prop_resprop_genvbounds(
    scip: &mut Scip,
    prop: PropId,
    infervar: VarId,
    inferinfo: i32,
    _boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    // As inferinfo we passed the index of the genvbound that was used for
    // propagation; the genvbound might have been replaced, but also the new
    // genvbound at this position has the same variable on the left-hand side.
    *result = ScipResultCode::DidNotFind;

    // Check also in optimized mode that inferinfo is correct.
    if inferinfo >= pd(scip, prop).ngenvbounds {
        log::error!("generalized variable bounds propagator received inferinfo out of range; propagation not resolved, safe to continue");
        return Ok(());
    }

    // Get genvbound responsible for the bound change.
    let pos = inferinfo as usize;
    let (var, boundtype) = {
        let g = gvb(scip, prop, pos);
        (g.var, g.boundtype)
    };

    // Check also in optimized mode that inferinfo is correct.
    if var != infervar {
        log::error!("generalized variable bounds propagator received incorrect inferinfo; propagation not resolved, but it's safe to continue");
        return Ok(());
    }

    // Get value of bound change on left-hand side.
    let mut boundval = if boundtype == BoundType::Lower {
        api::get_var_lb_at_index(scip, var, bdchgidx.as_ref(), true)
    } else {
        -api::get_var_ub_at_index(scip, var, bdchgidx.as_ref(), true)
    };

    // If left-hand side variable is integral, it suffices to explain a bound
    // change greater than boundval - 1.
    if crate::var::var_is_integral(scip, var) {
        let roundedboundval = scip.set.feas_ceil(boundval - 1.0) + 2.0 * scip.set.feastol();
        boundval = if boundval <= roundedboundval { boundval } else { roundedboundval };
    }

    // Resolve propagation.
    if resolve_gen_vbound_propagation(scip, prop, pos, bdchgidx.as_ref(), &mut boundval)? {
        *result = ScipResultCode::Success;
    }

    Ok(())
}

/// `propExitsolGenvbounds`: solving process deinitialization method of
/// propagator.
fn prop_exitsol_genvbounds(scip: &mut Scip, prop: PropId, _restart: bool) -> ScipResult {
    if !scip.stat.inrestart {
        // Free all genvbounds if we are not in a restart.
        free_gen_vbounds(scip, prop)?;
    } else {
        // Free all genvbounds that use relax-only variables if we are in a
        // restart.
        free_gen_vbounds_relax_only(scip, prop)?;
    }

    // Drop and free all events.
    drop_and_free_events(scip, prop)
}

/// `propFreeGenvbounds`: destructor of propagator to free user data.
fn prop_free_genvbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    scip.props[prop].propdata = None;
    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecGenvbounds`: the event handler data is the propagator.
fn event_exec_genvbounds(scip: &mut Scip, eventhdlr: EventhdlrId, _event: &Event, eventdata: usize) -> ScipResult {
    let node = crate::tree::tree_get_current_node(scip);

    // Ignore final events.
    if node == NONE {
        return Ok(());
    }

    let prop: PropId = *scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref::<PropId>().unwrap();
    let nodenumber = scip.nodes[node].number;
    let p = pdm(scip, prop);

    // @todo find a way to identify when we are at a new probing node; in
    // probing mode, the node number is always zero, so we may miss resetting
    // the starting data and may propagate more than necessary.
    // Check if we need to reset old local starting indices data.
    if nodenumber != p.lastnodenumber {
        reset_local_starting_data(p);
        p.lastnodenumber = nodenumber;
    }

    let ed = if eventdata.is_multiple_of(2) {
        &p.lbevents.as_ref().unwrap()[eventdata / 2]
    } else {
        &p.ubevents[eventdata / 2]
    };
    let starts: Vec<(i32, i32)> = ed.startcomponents.iter().copied().zip(ed.startindices.iter().copied()).collect();

    for (component, startidx) in starts {
        // There is already an entry for this component.
        if let Some(&img) = p.startmap.as_ref().unwrap().get(&(component + 1)) {
            // Get its index.
            let componentidx = (img - 1) as usize;

            if p.startindices[componentidx] > startidx {
                p.startindices[componentidx] = startidx;
            }
        } else {
            // Get a new entry.
            let componentidx = p.nindices;

            // Store index.
            p.startcomponents[componentidx as usize] = component;
            p.startindices[componentidx as usize] = startidx;

            // Store component in hashmap.
            p.startmap.as_mut().unwrap().insert(component + 1, componentidx + 1);

            // Increase number of starting indices.
            p.nindices += 1;
        }
    }

    Ok(())
}

/*
 * propagator specific interface methods
 */

/// `SCIPincludePropGenvbounds`: creates the genvbounds propagator and
/// includes it in SCIP.
pub fn include_prop_genvbounds(scip: &mut Scip) -> ScipResult {
    // Create genvbounds propagator data.
    let propdata = PropData {
        inittiming: PROP_TIMING,
        ngenvbounds: -1,
        cutoffboundvar: NONE,
        ..Default::default()
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
        prop_exec_genvbounds,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;

    let p = &mut scip.props[prop];
    p.propcopy = Some(prop_copy_genvbounds);
    p.propfree = Some(prop_free_genvbounds);
    p.propinit = Some(prop_init_genvbounds);
    p.propinitpre = Some(prop_initpre_genvbounds);
    p.propexitpre = Some(prop_exitpre_genvbounds);
    p.propexit = Some(prop_exit_genvbounds);
    p.propexitsol = Some(prop_exitsol_genvbounds);
    crate::prop::prop_set_presol(scip, prop, prop_presol_genvbounds, PROP_PRESOL_PRIORITY, PROP_PRESOL_MAXROUNDS, PROP_PRESOLTIMING)?;
    scip.props[prop].propresprop = Some(prop_resprop_genvbounds);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, props, prop, propdata, PropData, $f)
        };
    }
    paramset_add_bool(
        scip,
        "propagating/genvbounds/global",
        "apply global propagation?",
        t!(bool, global),
        true,
        DEFAULT_GLOBAL_PROPAGATION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/genvbounds/propinrootnode",
        "apply genvbounds in root node if no new incumbent was found?",
        t!(bool, propinrootnode),
        true,
        DEFAULT_PROPAGATE_IN_ROOT_NODE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/genvbounds/sort",
        "sort genvbounds and wait for bound change events?",
        t!(bool, sort),
        true,
        DEFAULT_SORT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/genvbounds/propasconss",
        "should genvbounds be transformed to (linear) constraints?",
        t!(bool, propasconss),
        true,
        DEFAULT_PROPASCONSS,
        None,
    )?;

    // Include event handler.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_genvbounds, Some(Box::new(prop) as Box<dyn Any>))?;
    pdm(scip, prop).eventhdlr = eventhdlr;

    Ok(())
}
