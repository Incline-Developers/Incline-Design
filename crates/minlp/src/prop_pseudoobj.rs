//! Port of `prop_pseudoobj.c`: pseudo objective propagator.
//!
//! This propagator propagates the objective function using the cutoff bound
//! and the pseudo objective value (the objective activity with each
//! variable at its best bound) to tighten variable bounds, and the global
//! lower bound against the maximum pseudo objective activity. Binary
//! variables use their implications (cliques) to strengthen the objective
//! change of a fixing.

use std::any::Any;

use crate::{
    api,
    cons::*,
    event::*,
    hash::{HashOps, HashTable},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_var,
    types::{BoundType, EventhdlrId, LockType, PropId, ScipResultCode, VarId},
    var::{BdChgIdx, NONE, var_get_best_bound_type, var_get_n_locks_down_type, var_get_n_locks_up_type, var_get_worst_bound_type},
};

/* propagator properties */
const PROP_NAME: &str = "pseudoobj";
const PROP_DESC: &str = "pseudo objective function propagator";
const PROP_TIMING: PropTiming = PROPTIMING_BEFORELP | PROPTIMING_DURINGLPLOOP | PROPTIMING_AFTERLPLOOP;
/// Propagator priority.
const PROP_PRIORITY: i32 = 3000000;
/// Propagator frequency.
const PROP_FREQ: i32 = 1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = false;
/// Priority of the presolving method (>= 0: before, < 0: after constraint
/// handlers); combined with presolvers.
const PROP_PRESOL_PRIORITY: i32 = 6000000;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PROP_PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolving method (fast, medium, or exhaustive).
const PROP_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST;

const EVENTHDLR_NAME: &str = "pseudoobj";
const EVENTHDLR_DESC: &str = "bound change event handler for pseudo objective function propagator";

/* default parameter values */
/// Minimal number of successive non-binary variable propagator whithout a
/// bound reduction before aborted.
const DEFAULT_MINUSELESS: i32 = 100;
/// Maximal fraction of non-binary variables with non-zero objective without
/// a bound reduction before aborted.
const DEFAULT_MAXVARSFRAC: f64 = 0.1;
/// Do we want to propagate all non-binary variables if we are propagating
/// the root node?
const DEFAULT_PROPFULLINROOT: bool = true;
/// Propagate new cutoff bound directly globally.
const DEFAULT_PROPCUTOFFBOUND: bool = true;
/// Should the propagator be forced even if active pricer are present? Note
/// that can be done if it is known that the pseudo objective activity is
/// given by the zero bound for all variables which are currently not present
/// in the problem.
const DEFAULT_FORCE: bool = false;
/// Number of variable added after the propagator is reinitialized?
const DEFAULT_MAXNEWVARS: i32 = 1000;
/// Use implications to strengthen the propagation of binary variable
/// (increasing the objective change)?
const DEFAULT_PROPUSEIMPLICS: bool = true;
/// Use implications to strengthen the resolve propagation of binary
/// variable (increasing the objective change)?
const DEFAULT_RESPROPUSEIMPLICS: bool = true;
/// Maximum number of binary variables the implications are used if turned
/// on (-1: unlimited)?
const DEFAULT_MAXIMPLVARS: i32 = 50000;

/// The maximal clique length implications are collected from.
const MAX_CLIQUELENGTH: i32 = 50;

/*
 * Data structures
 */

/// `SCIP_OBJIMPLICS`: implication data structure for objective
/// contributions of a binary variable.
#[derive(Debug, Default)]
pub struct ObjImplics {
    /// Variables y in implications y == 0 or y == 1, first we store the
    /// implications by x == 0 and second the implications x == 1.
    pub objvars: Vec<VarId>,
    /// Maximum objective contribution if variables x is fixed to zero or one.
    pub maxobjchg: f64,
    /// Number of all implications result through for x == 0.
    pub nlbimpls: i32,
    /// Number of all implications result through for x == 1.
    pub nubimpls: i32,
    /// Size of the objvars array.
    pub size: i32,
}

/// `SCIP_PropData`: propagator data.
pub struct PropData {
    /// Event handler for global bound change events.
    pub eventhdlr: EventhdlrId,
    /// Binary variables with non-zero objective contribution w.r.t. minimum
    /// activity of the objective function.
    pub minactvars: Vec<VarId>,
    /// Implication data structure for the binary variables w.r.t. minimum
    /// activity.
    pub minactimpls: Vec<ObjImplics>,
    /// Binary variables with non-zero objective contribution w.r.t. maximum
    /// activity of the objective function.
    pub maxactvars: Vec<VarId>,
    /// The maximal potential change of the objective if the binary variable
    /// is fixed to its best bound w.r.t. maximum activity of the objective
    /// function.
    pub maxactchgs: Vec<f64>,
    /// Non-binary variable with non-zero objective coefficient.
    pub objintvars: Vec<VarId>,
    /// Hash table used during resolving of a bound change (conflict analysis).
    pub addedvars: Option<HashTable<VarId>>,
    /// Last lower bound which was propagated.
    pub lastlowerbound: f64,
    /// Last cutoff bound used for propagation.
    pub cutoffbound: f64,
    /// Last global pseudo objective used in presolving.
    pub glbpseudoobjval: f64,
    /// Maximal fraction of non-binary variables with non-zero objective
    /// without a bound reduction before aborted.
    pub maxvarsfrac: f64,
    /// Maximal global pseudo objective activity.
    pub maxpseudoobjact: f64,
    /// Number of coefficients contributing with infinite value to
    /// maxpseudoobjact.
    pub maxpseudoobjactinf: i32,
    /// Number of binary variables with non-zero objective contribution w.r.t.
    /// minimum activity of the objective function.
    pub nminactvars: i32,
    /// Number of binary variables with non-zero objective contribution w.r.t.
    /// maximum activity of the objective function.
    pub nmaxactvars: i32,
    /// Number of non-binary variables with non-zero objective.
    pub nobjintvars: i32,
    /// Minimal number of successive non-binary variable propagator whithout a
    /// bound reduction before aborted.
    pub minuseless: i32,
    /// Last non-binary variable number that was looked at.
    pub lastvarnum: i32,
    /// Index of first globally non-fixed binary variable in minactvars array.
    pub glbfirstnonfixed: i32,
    /// Index of first globally non-fixed binary variable in maxctvars array.
    pub maxactfirstnonfixed: i32,
    /// Index of first locally non-fixed binary variable in minactvars array.
    pub firstnonfixed: i32,
    /// Counter for counting number of new variables added.
    pub nnewvars: i32,
    /// Number of variable added after the propagator is reinitialized?
    pub maxnewvars: i32,
    /// Maximum number of binary variables the implications are used if turned
    /// on (-1: unlimited)?
    pub maximplvars: i32,
    /// Size of minactvars and minactimpls array.
    pub minactsize: i32,
    /// Size of maxactvars and maxactchgs array.
    pub maxactsize: i32,
    /// Size of objintvars array.
    pub objintvarssize: i32,
    /// Are global domains propagated.
    pub glbpropagated: bool,
    /// Do we want to propagate all non-binary variables if we are propagating
    /// the root node.
    pub propfullinroot: bool,
    /// Propagate new cutoff bound directly globally.
    pub propcutoffbound: bool,
    /// Should the propagator be forced even if active pricer are present?
    pub force: bool,
    /// Do we catch the variable added event?
    pub catchvaradded: bool,
    /// Use implications to strengthen the propagation of binary variable
    /// (increasing the objective change)?
    pub propuseimplics: bool,
    /// Use implications to strengthen the resolve propagation of binary
    /// variable (increasing the objective change)?
    pub respropuseimplics: bool,
    /// Is propagator data structure initialized.
    pub initialized: bool,
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
 * Comparer
 */

/// `objimplicsComp`: compares objective implications w.r.t. their maximum
/// contribution.
fn objimplics_comp(objimplics1: &ObjImplics, objimplics2: &ObjImplics) -> i32 {
    if objimplics1.maxobjchg > objimplics2.maxobjchg {
        return 1;
    }
    if objimplics1.maxobjchg < objimplics2.maxobjchg {
        return -1;
    }
    0
}

/// `varCompObj`: compare variables w.r.t. (i) the absolute value the
/// objective coefficient; (ii) the locks which indicate most effect -- for
/// the variables with a positive (negative) objective coefficient the down
/// (up) lock is used since this lock indicates that tightened of the upper
/// (lower) bound will triegger further domain propagations; (iii) the other
/// locks; (iv) variable problem index.
fn var_comp_obj(scip: &Scip, var1: VarId, var2: VarId) -> i32 {
    let (obj1, obj2) = (scip.vars[var1].obj, scip.vars[var2].obj);

    // First criterion is the absolute value of objective coefficient.
    if obj1.abs() < obj2.abs() {
        return -1;
    } else if obj1.abs() > obj2.abs() {
        return 1;
    }

    // Second criterion the locks which indicate most effect.
    let locks1 = if obj1 > 0.0 {
        var_get_n_locks_down_type(scip, var1, LockType::Model)
    } else {
        var_get_n_locks_up_type(scip, var1, LockType::Model)
    };
    let locks2 = if obj2 > 0.0 {
        var_get_n_locks_down_type(scip, var2, LockType::Model)
    } else {
        var_get_n_locks_up_type(scip, var2, LockType::Model)
    };

    if locks1 < locks2 {
        return -1;
    }
    if locks1 > locks2 {
        return 1;
    }

    // Third criterion the other locks.
    let locks1 = if obj1 > 0.0 {
        var_get_n_locks_up_type(scip, var1, LockType::Model)
    } else {
        var_get_n_locks_down_type(scip, var1, LockType::Model)
    };
    let locks2 = if obj2 > 0.0 {
        var_get_n_locks_up_type(scip, var2, LockType::Model)
    } else {
        var_get_n_locks_down_type(scip, var2, LockType::Model)
    };

    if locks1 < locks2 {
        return -1;
    }
    if locks1 > locks2 {
        return 1;
    }

    // Forth criterion use the problem index.
    crate::var::var_compare(scip, var1, var2)
}

/// The useless-clique table's callbacks: elements are clique ids
/// (`cliqueGetHashkey`, `cliqueIsHashkeyEq`, `cliqueGetHashkeyVal`).
struct CliqueKeys<'a> {
    scip: &'a Scip,
}

impl HashOps<usize> for CliqueKeys<'_> {
    type Key = usize;

    fn get_key(&self, element: &usize) -> usize {
        *element
    }

    fn key_eq(&self, key1: &usize, key2: &usize) -> bool {
        key1 == key2
    }

    fn key_val(&self, key: &usize) -> u64 {
        self.scip.cliques[*key].id as u64
    }
}

/// The added-variable table's callbacks (`SCIPvarGetHashkey`,
/// `SCIPvarIsHashkeyEq`, `SCIPvarGetHashkeyVal`).
struct VarKeys<'a> {
    scip: &'a Scip,
}

impl HashOps<VarId> for VarKeys<'_> {
    type Key = VarId;

    fn get_key(&self, element: &VarId) -> VarId {
        *element
    }

    fn key_eq(&self, key1: &VarId, key2: &VarId) -> bool {
        key1 == key2
    }

    fn key_val(&self, key: &VarId) -> u64 {
        self.scip.vars[*key].index as u32 as u64
    }
}

/// The cliques of `var` for `varfixing` (`SCIPvarGetCliques`).
fn var_cliques(scip: &Scip, var: VarId, varfixing: bool) -> Vec<usize> {
    match scip.vars[var].cliquelist.as_ref() {
        Some(l) => l.cliques[varfixing as usize].0[..l.ncliques[varfixing as usize] as usize].to_vec(),
        None => Vec::new(),
    }
}

/// The position image of a binary objective variable
/// (`SCIPhashmapGetImageInt(binobjvarmap, var)`).
type BinObjVarMap = std::collections::HashMap<VarId, usize>;

/*
 * methods for SCIP_OBJIMPLICS data structure
 */

/// `objimplicsCreate`: creates an objective implication data structure,
/// fixes (globally) variables which are implied by lower and upper bound
/// fixing, and clears the collected arrays for lower and upper bound.
#[allow(clippy::too_many_arguments)]
fn objimplics_create(
    scip: &mut Scip,
    objvars: &[VarId],
    binobjvarmap: Option<&BinObjVarMap>,
    collectedlbvars: &mut [bool],
    collectedubvars: &mut [bool],
    mut maxlbobjchg: f64,
    mut maxubobjchg: f64,
    nlbimpls: usize,
    nubimpls: usize,
) -> ScipResult<ObjImplics> {
    let mut objimplics = ObjImplics::default();

    if nlbimpls + nubimpls == 0 {
        objimplics.maxobjchg = 0.0;
    } else {
        let binobjvarmap = binobjvarmap.unwrap();
        objimplics.objvars = Vec::with_capacity(nlbimpls + nubimpls);
        objimplics.size = (nlbimpls + nubimpls) as i32;

        for &var in &objvars[..nlbimpls] {
            let pos = binobjvarmap[&var];

            if collectedubvars[pos] {
                if var_get_best_bound_type(scip, var) == BoundType::Lower {
                    scip_var::tighten_var_lb_global(scip, var, 1.0, false)?;
                    maxlbobjchg -= scip.vars[var].obj;
                } else {
                    scip_var::tighten_var_ub_global(scip, var, 0.0, false)?;
                    maxlbobjchg += scip.vars[var].obj;
                }
            } else {
                objimplics.objvars.push(var);
            }
            collectedlbvars[pos] = false;
        }
        objimplics.nlbimpls = objimplics.objvars.len() as i32;

        for &var in &objvars[nlbimpls..nlbimpls + nubimpls] {
            let pos = binobjvarmap[&var];

            if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
                if var_get_best_bound_type(scip, var) == BoundType::Lower {
                    maxubobjchg -= scip.vars[var].obj;
                } else {
                    maxubobjchg += scip.vars[var].obj;
                }
            } else {
                objimplics.objvars.push(var);
            }
            collectedubvars[pos] = false;
        }
        objimplics.nubimpls = objimplics.objvars.len() as i32 - objimplics.nlbimpls;

        // Capture the variables.
        for &v in &objimplics.objvars {
            crate::var::var_capture(scip, v);
        }
    }

    objimplics.maxobjchg = crate::def::max(maxlbobjchg, maxubobjchg);

    Ok(objimplics)
}

/// `objimplicsFree`: frees an objective implication data structure.
fn objimplics_free(scip: &mut Scip, objimplics: &mut ObjImplics) -> ScipResult {
    // Release all variables.
    let n = (objimplics.nlbimpls + objimplics.nubimpls) as usize;
    for v in 0..n {
        let mut var = objimplics.objvars[v];
        api::release_var(scip, &mut var)?;
    }

    // Free objective variable array.
    *objimplics = ObjImplics::default();

    Ok(())
}

/// `objimplicsDelPos`: remove the given variable at the given pos from the
/// objective implication data structure.
fn objimplics_del_pos(scip: &mut Scip, objimplics: &mut ObjImplics, pos: usize) -> ScipResult {
    // Release variable.
    let mut var = objimplics.objvars[pos];
    api::release_var(scip, &mut var)?;

    let (nlb, nub) = (objimplics.nlbimpls as usize, objimplics.nubimpls as usize);

    // Copy last lower bound variable to that position.
    if pos < nlb {
        objimplics.nlbimpls -= 1;
        let nlb = nlb - 1;

        // Copy last lower bound variable to that position.
        objimplics.objvars[pos] = objimplics.objvars[nlb];

        // Copy last upper bound variable to open slot.
        objimplics.objvars[nlb] = objimplics.objvars[nlb + nub];
    } else {
        objimplics.nubimpls -= 1;
        let nub = nub - 1;

        // Copy last upper bound variable to that position.
        objimplics.objvars[pos] = objimplics.objvars[nlb + nub];
    }

    Ok(())
}

/*
 * Local methods
 */

/// `catchObjEvent`: catch bound change events if the variable has a non-zero
/// objective coefficient to check if the maximum activity of the objective
/// function changed; the event data is the propagator.
fn catch_obj_event(scip: &mut Scip, prop: PropId, eventhdlr: EventhdlrId, var: VarId) -> ScipResult {
    let objval = scip.vars[var].obj;

    if !scip.set.is_zero(objval) {
        if objval > 0.0 {
            api::catch_var_event(scip, var, EVENTTYPE_GUBCHANGED, eventhdlr, prop, None)?;
        } else {
            api::catch_var_event(scip, var, EVENTTYPE_GLBCHANGED, eventhdlr, prop, None)?;
        }
    }

    Ok(())
}

/// `dropObjEvent`: drop variable event w.r.t. objective coefficient.
fn drop_obj_event(scip: &mut Scip, prop: PropId, eventhdlr: EventhdlrId, var: VarId) -> ScipResult {
    let objval = scip.vars[var].obj;

    // Drop bound change event.
    if !scip.set.is_zero(objval) {
        if objval > 0.0 {
            api::drop_var_event(scip, var, EVENTTYPE_GUBCHANGED, eventhdlr, prop, -1)?;
        } else {
            api::drop_var_event(scip, var, EVENTTYPE_GLBCHANGED, eventhdlr, prop, -1)?;
        }
    }

    Ok(())
}

/// `dropVarEvents`: drop all variable events.
fn drop_var_events(scip: &mut Scip, prop: PropId) -> ScipResult {
    let eventhdlr = pd(scip, prop).eventhdlr;

    // Drop all events and release variables.
    let minactvars = pd(scip, prop).minactvars[..pd(scip, prop).nminactvars as usize].to_vec();
    for mut var in minactvars {
        // Drop bound relax event which is caught for all binary variables which
        // are used for propagation the objective function via the minimum
        // activity of the objective function.
        api::drop_var_event(scip, var, EVENTTYPE_BOUNDRELAXED, eventhdlr, prop, -1)?;

        // Release variable.
        api::release_var(scip, &mut var)?;
    }

    // Release variables.
    let maxactvars = pd(scip, prop).maxactvars[..pd(scip, prop).nmaxactvars as usize].to_vec();
    for mut var in maxactvars {
        // Drop events which are needed for evaluating the maximum activity of
        // the objective function.
        drop_obj_event(scip, prop, eventhdlr, var)?;

        // Release variable.
        api::release_var(scip, &mut var)?;
    }

    // Drop all events and release variables.
    let objintvars = pd(scip, prop).objintvars[..pd(scip, prop).nobjintvars as usize].to_vec();
    for mut var in objintvars {
        // Drop events which are needed for evaluating the maximum activity of
        // the objective function.
        drop_obj_event(scip, prop, eventhdlr, var)?;

        // Release variable.
        api::release_var(scip, &mut var)?;
    }

    Ok(())
}

/// `propdataReset`: reset propagatore data structure.
fn propdata_reset(propdata: &mut PropData) {
    use crate::def::INVALID;
    propdata.minactvars = Vec::new();
    propdata.minactimpls = Vec::new();
    propdata.maxactvars = Vec::new();
    propdata.maxactchgs = Vec::new();
    propdata.objintvars = Vec::new();
    propdata.nminactvars = 0;
    propdata.nmaxactvars = 0;
    propdata.nobjintvars = 0;
    propdata.maxpseudoobjact = INVALID;
    propdata.maxpseudoobjactinf = 0;
    propdata.lastvarnum = -1;
    propdata.glbpropagated = false;
    propdata.cutoffbound = INVALID;
    propdata.lastlowerbound = -INVALID;
    propdata.glbpseudoobjval = -INVALID;
    propdata.glbfirstnonfixed = 0;
    propdata.maxactfirstnonfixed = 0;
    propdata.firstnonfixed = 0;
    propdata.nnewvars = 0;
    propdata.minactsize = 0;
    propdata.maxactsize = 0;
    propdata.objintvarssize = 0;
    propdata.catchvaradded = false;
    propdata.initialized = false;
}

/// `propdataExit`: free propagator data.
fn propdata_exit(scip: &mut Scip, prop: PropId) -> ScipResult {
    if !pd(scip, prop).initialized {
        return Ok(());
    }

    pdm(scip, prop).addedvars = None;

    // Drop events for the variables.
    drop_var_events(scip, prop)?;

    let mut impls = std::mem::take(&mut pdm(scip, prop).minactimpls);
    let n = pd(scip, prop).nminactvars as usize;
    for imp in impls.iter_mut().take(n) {
        objimplics_free(scip, imp)?;
    }

    // Free memory for non-zero objective variables and reset propagator data
    // structure.
    propdata_reset(pdm(scip, prop));

    Ok(())
}

/// `getVarObjchg`: returns the objective change for the given binary
/// variable.
fn get_var_objchg(scip: &Scip, var: VarId, boundtype: BoundType, bound: BoundType) -> f64 {
    // Collect contribution of variable itself.
    (bound as i32 - (boundtype == BoundType::Upper) as i32) as f64 * scip.vars[var].obj
}

/// `collectMinactImplicVar`: returns the objective change provided by the
/// implication variable by fixing it to the given bound w.r.t. minimum
/// activity of the objective function; additionally it collects all
/// contributors for that objective change.
fn collect_minact_implic_var(scip: &Scip, var: VarId, binobjvarmap: &BinObjVarMap, collectedvars: &mut [bool], contributors: &mut Vec<VarId>) -> f64 {
    // Ignore global fixed variables.
    if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
        return 0.0;
    }

    let objval = scip.vars[var].obj;

    // Ignore variables with zero objective coefficient.
    if scip.set.is_zero(objval) {
        return 0.0;
    }

    let pos = binobjvarmap[&var];

    // Check if the variables was already collected through other cliques.
    if collectedvars[pos] {
        return 0.0;
    }

    // Collect variable.
    contributors.push(var);

    // Mark variable to be collected.
    collectedvars[pos] = true;

    // Return the absolute value of the objective coefficient as constriction.
    objval.abs()
}

/// `collectMinactImplicVars`: returns the objective change provided by the
/// implications of the given variable by fixing it to the given bound w.r.t.
/// minimum activity of the objective function; additionally it collects all
/// contributors for that objective change.
#[allow(clippy::too_many_arguments)]
fn collect_minact_implic_vars(
    scip: &Scip,
    var: VarId,
    bound: BoundType,
    binobjvarmap: &BinObjVarMap,
    collectedvars: &mut [bool],
    contributors: &mut Vec<VarId>,
    uselesscliques: &mut HashTable<usize>,
    objchg: &mut f64,
) -> ScipResult {
    let varfixing = bound == BoundType::Upper;
    let cliques = var_cliques(scip, var, varfixing);
    let ops = CliqueKeys { scip };

    // Collect all implication which are given via cliques.
    for &clique in &cliques {
        // Check if the clique was previously detected to be useless with respect
        // to minimum activity.
        if uselesscliques.exists(&ops, &clique) {
            continue;
        }

        let c = &scip.cliques[clique];
        let nbinvars = c.nvars;

        if nbinvars > MAX_CLIQUELENGTH {
            uselesscliques.insert(&ops, clique);
            continue;
        }

        let mut useless = true;

        for v in 0..nbinvars as usize {
            let implvar = c.vars[v];

            if implvar == var {
                // Check if the clique is useful at all.
                if useless {
                    let objval = scip.vars[var].obj;

                    if varfixing == (var_get_best_bound_type(scip, var) == BoundType::Upper) && !scip.set.is_zero(objval) {
                        useless = false;
                    }
                }
            } else if c.values[v] == (var_get_best_bound_type(scip, implvar) == BoundType::Upper) {
                useless = false;
                *objchg += collect_minact_implic_var(scip, implvar, binobjvarmap, collectedvars, contributors);
            }
        }

        // If the clique is useless store it in the hash table to skip it later.
        if useless {
            uselesscliques.insert(&ops, clique);
        }
    }

    Ok(())
}

/// `getMinactImplicObjchg`: returns the objective change provided by the
/// implications of the given variable by fixing it to the given bound w.r.t.
/// minimum activity of the objective function; this can be done w.r.t.
/// global variable bounds (`local == false`), w.r.t. local variable bounds
/// (`local == true && bdchgidx == None`), and w.r.t. given time stamp
/// (`local == true && bdchgidx != None`).
fn get_minact_implic_objchg(scip: &mut Scip, objimplics: &mut ObjImplics, bdchgidx: Option<&BdChgIdx>, bound: BoundType, local: bool, objchg: &mut f64) -> ScipResult {
    let (mut v, mut nbinvars) = if bound == BoundType::Lower {
        (0, objimplics.nlbimpls as usize)
    } else {
        (objimplics.nlbimpls as usize, (objimplics.nlbimpls + objimplics.nubimpls) as usize)
    };

    // Loop over all implications.
    while v < nbinvars {
        let implvar = objimplics.objvars[v];

        if local {
            let lb = api::get_var_lb_at_index(scip, implvar, bdchgidx, false) > 0.5;
            let ub = api::get_var_ub_at_index(scip, implvar, bdchgidx, false) > 0.5;

            // Check if variable is fixed.
            if lb || !ub {
                v += 1;
                continue;
            }
        } else {
            let lb = scip.vars[implvar].glbdom.lb > 0.5;
            let ub = scip.vars[implvar].glbdom.ub > 0.5;

            // Check if variable is global fixed; if so remove it from the
            // objective implication data structure and continue with the next
            // candidate.
            if lb || !ub {
                objimplics_del_pos(scip, objimplics, v)?;
                nbinvars -= 1;
                continue;
            }
        }

        // Add objective change.
        *objchg += scip.vars[implvar].obj.abs();
        v += 1;
    }

    Ok(())
}

/// `collectMinactObjchg`: computes for the given binary variable the
/// objective contribution by fixing it to given bound w.r.t. minimum
/// activity of the objective function; additionally it collects all
/// contributors for that objective change.
fn collect_minact_objchg(
    scip: &Scip,
    var: VarId,
    bound: BoundType,
    binobjvarmap: &BinObjVarMap,
    collectedvars: &mut [bool],
    contributors: &mut Vec<VarId>,
    uselesscliques: &mut HashTable<usize>,
) -> ScipResult<f64> {
    // Collects the contribution of the variable itself w.r.t. the best bound.
    let mut objchg = get_var_objchg(scip, var, var_get_best_bound_type(scip, var), bound);

    // Add the objective contribution from the implication variable.
    collect_minact_implic_vars(scip, var, bound, binobjvarmap, collectedvars, contributors, uselesscliques, &mut objchg)?;

    Ok(objchg)
}

/// `getMinactObjchg`: computes for the given binary variable the objective
/// contribution by fixing it to given bound w.r.t. minimum activity of the
/// objective function (see [`get_minact_implic_objchg`] for `local` and
/// `bdchgidx`).
fn get_minact_objchg(scip: &mut Scip, var: VarId, objimplics: &mut ObjImplics, bdchgidx: Option<&BdChgIdx>, bound: BoundType, local: bool) -> ScipResult<f64> {
    // Collects the contribution of the variable itself w.r.t. the best bound.
    let mut objchg = get_var_objchg(scip, var, var_get_best_bound_type(scip, var), bound);

    // Add the objective contribution from the implication variable.
    get_minact_implic_objchg(scip, objimplics, bdchgidx, bound, local, &mut objchg)?;

    Ok(objchg)
}

/// `getMaxactImplicObjchg`: returns the global (that means w.r.t. global
/// bounds of the variables) objective change provided by all cliques of the
/// given variable by fixing it to the given bound w.r.t. maximum activity of
/// the objective function.
fn get_maxact_implic_objchg(scip: &Scip, var: VarId, bound: BoundType) -> f64 {
    let varfixing = bound == BoundType::Upper;
    let mut objchg = 0.0;

    let cliques = var_cliques(scip, var, varfixing);

    if !cliques.is_empty() {
        let nentries = (api::get_n_vars(scip) - api::get_n_cont_vars(scip) + 1) as usize;
        let mut ids: Vec<usize> = Vec::with_capacity(2 * nentries);

        // @todo move this memory allocation to SCIP_SET and add a memory list
        // there, to decrease the number of allocations and clear ups.
        let mut entries = vec![0i32; nentries];

        // Iterate over all cliques and determine all importantimplications.
        for &clique in cliques.iter().rev() {
            let c = &scip.cliques[clique];
            let nclqvars = c.nvars;

            if nclqvars > MAX_CLIQUELENGTH {
                continue;
            }

            // Iterate over all clique variables.
            for v in (0..nclqvars as usize).rev() {
                let clqvar = c.vars[v];
                let objmult = (!c.values[v]) as i32 - var_get_worst_bound_type(scip, clqvar) as i32;
                let x = &scip.vars[clqvar];

                // Ignore binary variable which are either fixed and were the
                // objective contribution will not be zero.
                if clqvar != var && objmult != 0 && crate::var::var_is_active(scip, clqvar) && (x.glbdom.lb < 0.5 && x.glbdom.ub > 0.5) && !scip.set.is_zero(x.obj) {
                    let probindex = (x.probindex + 1) as usize;

                    // Check that the variable was not yet visited.
                    if entries[probindex] == 0 {
                        // Memorize probindex.
                        ids.push(probindex);

                        // Mark variable as visited.
                        entries[probindex] = objmult;
                    }
                }
            }
        }

        let probvars = api::get_vars(scip);

        // Add all implied objective values.
        for &id in ids.iter().rev() {
            let clqvar = probvars[id - 1];
            let obj = scip.vars[clqvar].obj;

            objchg += entries[id] as f64 * obj;
        }
    }

    // Collect non-binary implication information.
    let nvars = crate::var::var_get_n_impls(scip, var, varfixing) as usize;

    if nvars > 0 {
        let im = scip.vars[var].implics.as_ref().unwrap();
        let f = varfixing as usize;

        for v in (0..nvars).rev() {
            let implvar = im.vars[f][v];
            let lb = scip.vars[implvar].locdom.lb;
            let ub = scip.vars[implvar].locdom.ub;
            let obj = scip.vars[implvar].obj;

            // Ignore binary variable which are fixed or not of column status.
            if scip.set.is_zero(obj) {
                continue;
            }

            // Add up objective change if applicable.
            let (bt, bd) = (im.types[f][v], im.bounds[f][v]);
            if bt == BoundType::Lower && var_get_worst_bound_type(scip, implvar) == BoundType::Lower && scip.set.is_feas_gt(bd, lb) {
                objchg += (bd - lb) * obj;
            } else if bt == BoundType::Upper && var_get_worst_bound_type(scip, implvar) == BoundType::Upper && scip.set.is_feas_lt(bd, ub) {
                objchg += (bd - ub) * obj;
            }
        }
    }

    objchg
}

/// `getMaxactObjchg`: computes for the given binary variable the gloabl
/// (that means w.r.t. global bounds of the variables) objective contribution
/// by fixing it to given bound w.r.t. maximum activity of the objective
/// function.
fn get_maxact_objchg(scip: &Scip, var: VarId, bound: BoundType, useimplics: bool) -> f64 {
    let mut objchg = 0.0;

    // Check if the implications should be used to increase the objective
    // contribution for given variable.
    if useimplics {
        // Using cliques and @todo other implications.
        objchg = get_maxact_implic_objchg(scip, var, bound);
    }

    // Collects the contribution of the variable itself w.r.t. the worst bound.
    objchg + get_var_objchg(scip, var, var_get_worst_bound_type(scip, var), bound)
}

/// `resetContributors`: reset variables array which marks variables which
/// are collected.
fn reset_contributors(binobjvarmap: &BinObjVarMap, collectedvars: &mut [bool], contributors: &[VarId]) {
    for var in contributors {
        collectedvars[binobjvarmap[var]] = false;
    }
}

/// `collectMinactVar`: check if the given variable should be collected for
/// the minimum activity propagation; returns the objective implication data
/// structure w.r.t. minimum activity if it should.
#[allow(clippy::too_many_arguments)]
fn collect_minact_var(
    scip: &mut Scip,
    var: VarId,
    useimplics: bool,
    binobjvarmap: &BinObjVarMap,
    collectedlbvars: &mut [bool],
    collectedubvars: &mut [bool],
    uselesscliques: &mut HashTable<usize>,
) -> ScipResult<Option<ObjImplics>> {
    let objval = scip.vars[var].obj;
    let mut collect = !scip.set.is_zero(objval);
    let mut objimplics = None;

    let nlbcliques = crate::var::var_get_n_cliques(scip, var, false);
    let nubcliques = crate::var::var_get_n_cliques(scip, var, true);

    // Check if implications should be used and if implications are existing.
    if useimplics && nlbcliques + nubcliques > 0 {
        let mut contributors: Vec<VarId> = Vec::new();

        // Get contribution of variable by fixing it to its lower bound w.r.t.
        // minimum activity of the objective function.
        let lbobjchg = collect_minact_objchg(scip, var, BoundType::Lower, binobjvarmap, collectedlbvars, &mut contributors, uselesscliques)?;
        let mut nlbcontributors = contributors.len();

        // Ignore implications if the variable has a zero objective coefficient
        // and implications only one variable, since this is covered by that
        // implied variable.
        if !collect && nlbcontributors == 1 {
            // Reset lower bound contributors.
            reset_contributors(binobjvarmap, collectedlbvars, &contributors[..1]);
            contributors.clear();
            nlbcontributors = 0;
        }

        // Get contribution of variable by fixing it to its upper bound w.r.t.
        // minimum activity of the objective function.
        let ubobjchg = collect_minact_objchg(scip, var, BoundType::Upper, binobjvarmap, collectedubvars, &mut contributors, uselesscliques)?;
        let mut nubcontributors = contributors.len() - nlbcontributors;

        // Ignore implications if the variable has a zero objective coefficient
        // and implications only one variable, since this is covered by that
        // implied variable.
        if !collect && nubcontributors == 1 {
            // Reset upper bound contributors.
            reset_contributors(binobjvarmap, collectedubvars, &contributors[nlbcontributors..]);
            contributors.truncate(nlbcontributors);
            nubcontributors = 0;
        }

        if collect || nlbcontributors > 1 || nubcontributors > 1 {
            // Creates an objective implication data structure, fixes (globally)
            // variables which are implied by lower and upper bound fixing, and
            // clears the collected arrays for lower and upper bound.
            objimplics = Some(objimplics_create(
                scip,
                &contributors,
                Some(binobjvarmap),
                collectedlbvars,
                collectedubvars,
                lbobjchg,
                ubobjchg,
                nlbcontributors,
                nubcontributors,
            )?);
            collect = true;
        } else {
            // Reset lower bound contributors.
            reset_contributors(binobjvarmap, collectedlbvars, &contributors[..nlbcontributors]);

            // Reset upper bound contributors.
            reset_contributors(binobjvarmap, collectedubvars, &contributors[nlbcontributors..]);
        }
    } else if collect {
        let lbobjchg = get_var_objchg(scip, var, var_get_best_bound_type(scip, var), BoundType::Lower);
        let ubobjchg = get_var_objchg(scip, var, var_get_best_bound_type(scip, var), BoundType::Upper);

        // Creates an "empty" objective implication data structure.
        objimplics = Some(objimplics_create(scip, &[], None, &mut [], &mut [], lbobjchg, ubobjchg, 0, 0)?);
    }

    Ok(if collect { objimplics } else { None })
}

/// `collectMaxactVar`: check if the given variable should be collected for
/// the maximum activity propagation; returns `(objchg, isnotzero)`.
fn collect_maxact_var(scip: &Scip, var: VarId, useimplics: bool) -> (f64, bool) {
    // Get contribution of variable by fixing it to its lower bound w.r.t.
    // maximum activity of the objective function.
    let lbobjchg = get_maxact_objchg(scip, var, BoundType::Lower, useimplics);

    // Get contribution of variable by fixing it to its upper bound w.r.t.
    // maximum activity of the objective function.
    let ubobjchg = get_maxact_objchg(scip, var, BoundType::Upper, useimplics);

    let objchg = crate::def::min(lbobjchg, ubobjchg);

    // Only consider variables with non-zero objective contribution.
    (objchg, !scip.set.is_zero(objchg))
}

/// `propdataInit`: initializate the propagator.
fn propdata_init(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Get problem variables.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nintvars = nvars - api::get_n_cont_vars(scip) as usize;
    let mut nbinvars = 0;
    let mut nobjvars = 0;
    let mut nbinobjvars = 0;

    let mut binobjvarmap: BinObjVarMap = BinObjVarMap::new();

    // Count and collect variable problem indices of variables with non-zero
    // objective coefficient.
    for &var in &vars {
        if !scip.set.is_zero(scip.vars[var].obj) {
            nobjvars += 1;
            if crate::var::var_is_binary(scip, var) {
                binobjvarmap.insert(var, nbinobjvars + 1);
                nbinobjvars += 1;
            }
        }

        if crate::var::var_is_binary(scip, var) {
            nbinvars += 1;
        }
    }

    let mut nminactvars = 0;
    let mut nmaxactvars = 0;
    let mut nobjintvars = 0;
    let mut nobjcontvars = 0;

    if nobjvars > 0 {
        let p = pd(scip, prop);
        let eventhdlr = p.eventhdlr;
        let useimplics = p.propuseimplics && (nbinobjvars as i32) < p.maximplvars;

        // Allocate memory for all arrays.
        let objintvarssize = nobjvars - nbinobjvars;
        let mut minactvars: Vec<VarId> = Vec::with_capacity(nbinvars);
        let mut minactimpls: Vec<ObjImplics> = Vec::with_capacity(nbinvars);
        let mut maxactvars: Vec<VarId> = Vec::with_capacity(nbinvars);
        let mut maxactchgs: Vec<f64> = Vec::with_capacity(nbinvars);
        let mut objintvars: Vec<VarId> = Vec::with_capacity(objintvarssize);
        {
            let p = pdm(scip, prop);
            p.minactsize = nbinvars as i32;
            p.maxactsize = nbinvars as i32;
            p.objintvarssize = objintvarssize as i32;
        }

        let mut collectedlbvars = Vec::new();
        let mut collectedubvars = Vec::new();
        let mut uselesscliques = HashTable::new(1);
        if useimplics {
            // Create temporary buffer.
            collectedlbvars = vec![false; nbinobjvars + 1];
            collectedubvars = vec![false; nbinobjvars + 1];

            let ncliques = scip_var::get_n_cliques(scip);

            if ncliques > 0 {
                uselesscliques = HashTable::new(ncliques);
            }
        }

        // Collect the variables with non-zero objective contribution and catch
        // global bound tighten events that decrease the maximal pseudo objective
        // activity.
        let mut v = 0;
        while v < nvars && (nobjintvars == 0 || nobjintvars < objintvarssize) {
            let var = vars[v];
            let objval = scip.vars[var].obj;

            if crate::var::var_is_binary(scip, var) {
                // Ignore variables which are globally fixed.
                if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
                    v += 1;
                    continue;
                }

                // Check if the variable should be collected for the minimum activity
                // propagation.
                let objimplics = collect_minact_var(scip, var, useimplics, &binobjvarmap, &mut collectedlbvars, &mut collectedubvars, &mut uselesscliques)?;

                if let Some(objimplics) = objimplics {
                    // Collect the binary variable with non-zero objective
                    // contribution.
                    minactvars.push(var);
                    minactimpls.push(objimplics);
                    nminactvars += 1;

                    // Catch bound relax event for the binary variable to handel the
                    // firstnonfixed index correctly.
                    api::catch_var_event(scip, var, EVENTTYPE_BOUNDRELAXED, eventhdlr, prop, None)?;

                    // Captures the variable.
                    crate::var::var_capture(scip, var);
                }

                // Check if the variable should be collected for the maximum activity
                // propagation.
                let (objchg, collect) = collect_maxact_var(scip, var, useimplics);

                if collect {
                    // Collect the binary variable with non-zero objective
                    // contribution.
                    maxactvars.push(var);
                    maxactchgs.push(-objchg);
                    nmaxactvars += 1;

                    // Catch bound change events if the variable has a non-zero
                    // objective coefficient to check if the maximum activity of the
                    // objective function changed.
                    catch_obj_event(scip, prop, eventhdlr, var)?;

                    // Captures the variable.
                    crate::var::var_capture(scip, var);
                }
            } else {
                // Only consider non-binary variables with a non-zero objective.
                if scip.set.is_zero(objval) {
                    v += 1;
                    continue;
                }

                objintvars.push(var);
                nobjintvars += 1;
                if v >= nintvars {
                    nobjcontvars += 1;
                }

                // Catch bound change events if the variable has a non-zero
                // objective coefficient to check if the maximum activity of the
                // objective function changed.
                catch_obj_event(scip, prop, eventhdlr, var)?;

                // Captures the variable.
                crate::var::var_capture(scip, var);
            }
            v += 1;
        }

        if nminactvars == 0 {
            pdm(scip, prop).minactsize = 0;
        } else {
            // Sort binary variables with respect to the absolute value of their
            // maximal potential objective contribution for the minimum activity of
            // the objective function (`SCIPsortDownPtrPtr` with the implication
            // pointers as keys: the permutation is decided by the comparator
            // alone, so sorting positions gives the same order).
            let mut order: Vec<usize> = (0..nminactvars).collect();
            crate::sort::sort_down(&mut order, (&mut minactvars[..],), |&a, &b| objimplics_comp(&minactimpls[a], &minactimpls[b]));
            let mut taken: Vec<Option<ObjImplics>> = minactimpls.into_iter().map(Some).collect();
            minactimpls = order.iter().map(|&i| taken[i].take().unwrap()).collect();
        }

        if nmaxactvars == 0 {
            pdm(scip, prop).maxactsize = 0;
        } else {
            // Sort binary variables with respect to the absolute value of their
            // maximal potential objective contribution for the maximum activity of
            // the objective function.
            crate::sort::sort_down(&mut maxactchgs, (&mut maxactvars[..],), crate::sort::cmp_real);
        }

        if nobjintvars == 0 {
            pdm(scip, prop).objintvarssize = 0;
        } else {
            // Sort integer variables with respect to the absolute value of their
            // objective coefficient.
            let nint = nobjintvars - nobjcontvars;
            crate::sort::sort_down(&mut objintvars[..nint], (), |&a, &b| var_comp_obj(scip, a, b));

            // Sort continuous variables with respect to the absolute value of their
            // objective coefficient.
            crate::sort::sort_down(&mut objintvars[nint..nobjintvars], (), |&a, &b| var_comp_obj(scip, a, b));
        }

        let p = pdm(scip, prop);
        p.minactvars = minactvars;
        p.minactimpls = minactimpls;
        p.maxactvars = maxactvars;
        p.maxactchgs = maxactchgs;
        p.objintvars = objintvars;
    }

    let inf = scip.set.infinity();
    let p = pdm(scip, prop);
    p.nminactvars = nminactvars as i32;
    p.nmaxactvars = nmaxactvars;
    p.nobjintvars = nobjintvars as i32;
    p.maxpseudoobjact = crate::def::INVALID;
    p.maxpseudoobjactinf = 0;
    p.lastvarnum = -1;
    p.glbfirstnonfixed = 0;
    p.maxactfirstnonfixed = 0;
    p.firstnonfixed = 0;
    p.nnewvars = 0;
    p.cutoffbound = inf;
    p.lastlowerbound = -inf;
    p.glbpseudoobjval = -inf;
    p.initialized = true;

    // Due to scaling after presolving we need to update the global
    // pseudoactivity and the cutoffbound.
    p.glbpropagated = false;
    let glbpseudoobjval = api::get_global_pseudo_objval(scip);
    let cutoffbound = api::get_cutoffbound(scip);
    let p = pdm(scip, prop);
    p.glbpseudoobjval = glbpseudoobjval;
    p.cutoffbound = cutoffbound;

    // Create hash table which is used for resolving bound changes.
    p.addedvars = if nminactvars > 0 { Some(HashTable::new(nvars as i32)) } else { None };

    Ok(())
}

/// `addConflictBounds`: adds for the given non-binary variable a conflict
/// bound depending on its objective contribution.
fn add_conflict_bounds(scip: &mut Scip, var: VarId, bdchgidx: Option<&BdChgIdx>, reqpseudoobjval: &mut f64) -> ScipResult {
    let objval = scip.vars[var].obj;

    if objval > 0.0 {
        let glblb = scip.vars[var].glbdom.lb;
        let loclb = api::get_var_lb_at_index(scip, var, bdchgidx, false);

        // Check if the local lower bound (at time stamp bdchgidx) is larger than
        // the global lower bound.
        if scip.set.is_gt(loclb, glblb) {
            api::add_conflict_lb(scip, var, bdchgidx)?;

            // Hard comparison is enough to make requiredpseudoobjval
            // nonincreasing.
            *reqpseudoobjval -= (loclb - glblb) * objval;
        }
    } else {
        let glbub = scip.vars[var].glbdom.ub;
        let locub = api::get_var_ub_at_index(scip, var, bdchgidx, false);

        // Check if the local upper bound (at time stamp bdchgidx) is smaller than
        // the global upper bound.
        if scip.set.is_lt(locub, glbub) {
            api::add_conflict_ub(scip, var, bdchgidx)?;

            // Hard comparison is enough to make requiredpseudoobjval
            // nonincreasing.
            *reqpseudoobjval -= (locub - glbub) * objval;
        }
    }

    Ok(())
}

/// `getConflictImplics`: check for the given implication variables if they
/// also contribute to the required minimum activity; returns whether an
/// implication is found.
fn get_conflict_implics(scip: &Scip, vars: &[VarId], bdchgidx: Option<&BdChgIdx>, addedvars: &mut HashTable<VarId>, reqpseudoobjval: &mut f64) -> bool {
    let mut foundimplics = false;
    let ops = VarKeys { scip };

    for &var in vars {
        // We need to take the bounds after the bdchgidx here, since the variable
        // of the bound change may be the implied one; we already counted its
        // contribution before, so we want to see it as fixed here, which it is
        // after the bound change.
        let lb = api::get_var_lb_at_index(scip, var, bdchgidx, true);
        let ub = api::get_var_ub_at_index(scip, var, bdchgidx, true);

        if lb < 0.5 && ub > 0.5 && !addedvars.exists(&ops, &var) {
            *reqpseudoobjval -= scip.vars[var].obj.abs();
            addedvars.insert(&ops, var);
            foundimplics = true;
        }
    }

    foundimplics
}

/// `addConflictBinvar`: adds for the given binary variable a conflict bound
/// depending on its objective contribution.
#[allow(clippy::too_many_arguments)]
fn add_conflict_binvar(
    scip: &mut Scip,
    var: VarId,
    bdchgidx: Option<&BdChgIdx>,
    objimplics: Option<&ObjImplics>,
    mut addedvars: Option<&mut HashTable<VarId>>,
    respropuseimplics: bool,
    reqpseudoobjval: &mut f64,
) -> ScipResult {
    if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
        return Ok(());
    }

    let lb = api::get_var_lb_at_index(scip, var, bdchgidx, false);
    let ub = api::get_var_ub_at_index(scip, var, bdchgidx, false);

    let objval = scip.vars[var].obj;
    let mut foundimplics = false;

    // Only consider variables which are fixed.
    if lb > 0.5 {
        if respropuseimplics {
            let o = objimplics.unwrap();
            let (nlb, nub) = (o.nlbimpls as usize, o.nubimpls as usize);
            foundimplics = get_conflict_implics(scip, &o.objvars[nlb..nlb + nub], bdchgidx, addedvars.as_deref_mut().unwrap(), reqpseudoobjval);
        }

        // Check if the binary variable has a positive contribution (positive
        // objective coefficient since it is fixed to one) or is needed due a
        // positive contribution of an implied variable.
        if foundimplics || scip.set.is_positive(objval) {
            api::add_conflict_lb(scip, var, None)?;
            *reqpseudoobjval -= crate::def::max(0.0, objval);

            if let Some(added) = addedvars {
                added.insert(&VarKeys { scip }, var);
            }
        }
    } else if ub < 0.5 {
        if respropuseimplics {
            let o = objimplics.unwrap();
            let nlb = o.nlbimpls as usize;
            foundimplics = get_conflict_implics(scip, &o.objvars[..nlb], bdchgidx, addedvars.as_deref_mut().unwrap(), reqpseudoobjval);
        }

        // Check if the binary variable has a positive contribution (negative
        // objective coefficient since it is fixed to zero) or is needed due a
        // positive contribution of an implied variable.
        if foundimplics || scip.set.is_negative(objval) {
            api::add_conflict_ub(scip, var, None)?;
            *reqpseudoobjval += crate::def::min(0.0, objval);

            if let Some(added) = addedvars {
                added.insert(&VarKeys { scip }, var);
            }
        }
    }

    Ok(())
}

/// `adjustCutoffbound`: resolves a propagation by supplying the variables
/// whose bound changes increased the pseudo objective value above the cutoff
/// bound.
#[allow(clippy::too_many_arguments)]
fn adjust_cutoffbound(
    scip: &mut Scip,
    prop: PropId,
    var: VarId,
    inferinfo: i32,
    boundtype: BoundType,
    bdchgidx: Option<&BdChgIdx>,
    addedvars: Option<&mut HashTable<VarId>>,
    cutoffbound: &mut f64,
) -> ScipResult {
    if inferinfo != -1 {
        // Get the objective contribution if we would fix the binary inference
        // variable to its other bound.
        *cutoffbound -= get_var_objchg(scip, var, var_get_best_bound_type(scip, var), boundtype);

        let objimplics = &pd(scip, prop).minactimpls[inferinfo as usize];
        let (start, end) = if boundtype == BoundType::Lower {
            (0, objimplics.nlbimpls as usize)
        } else {
            (objimplics.nlbimpls as usize, (objimplics.nlbimpls + objimplics.nubimpls) as usize)
        };
        let objvars = objimplics.objvars[start..end].to_vec();

        if let Some(added) = addedvars {
            get_conflict_implics(scip, &objvars, bdchgidx, added, cutoffbound);
        }
    } else {
        let objval = scip.vars[var].obj;

        let (mut newbound, glbbound) = if objval > 0.0 {
            (api::get_var_ub_at_index(scip, var, bdchgidx, true), scip.vars[var].glbdom.lb)
        } else {
            (api::get_var_lb_at_index(scip, var, bdchgidx, true), scip.vars[var].glbdom.ub)
        };

        // In case the variable is integral we just need to prove the newbound
        // plus/minus (1 - epsilon) since the this bound would be rounded to
        // newbound due to integrability which is global information.
        if crate::var::var_is_integral(scip, var) {
            if objval > 0.0 {
                newbound += 1.0 - 10.0 * scip.set.feastol();
            } else {
                newbound -= 1.0 - 10.0 * scip.set.feastol();
            }
        }

        // Adjust the cutoff bound by the portion the inference variable
        // contributes to the presudo objective activity (minactivity).
        *cutoffbound -= objval * (newbound - glbbound);
    }

    Ok(())
}

/// `resolvePropagation`: resolves a propagation by supplying the variables
/// whose bound changes increased the pseudo objective value above the cutoff
/// bound; `infervar` is `NONE` for conflict analysis initialization.
fn resolve_propagation(scip: &mut Scip, prop: PropId, mut cutoffbound: f64, infervar: VarId, inferinfo: i32, boundtype: BoundType, bdchgidx: Option<&BdChgIdx>) -> ScipResult {
    let mut infinity = false;
    let nvars = pd(scip, prop).nminactvars as usize;

    // The addedvars table is taken out of the propagator data while it is in
    // use.
    let mut addedvars: Option<HashTable<VarId>> = None;
    let reqpseudoobjval;

    // The global pseudo value gives us a global valid minimal activity. The
    // global pseudo objective activity can be minus infinity. In that case all
    // variable are part of the reason/explanation. If the global pseudo
    // objective activity is greater than the required minactivity, the local
    // bound change which has to explained is actually (now) a global one. That
    // means, the reason/explanation is empty.
    let glbpseudoobjval = api::get_global_pseudo_objval(scip);

    if scip.set.is_infinity(-glbpseudoobjval) {
        infinity = true;
        reqpseudoobjval = cutoffbound;
    } else {
        // Clear hash table for storing variables which are not needed to add the
        // reason due to global implications or already added.
        if nvars > 0 {
            let mut t = pdm(scip, prop).addedvars.take().unwrap();
            t.remove_all();
            addedvars = Some(t);
        }

        if infervar != NONE {
            adjust_cutoffbound(scip, prop, infervar, inferinfo, boundtype, bdchgidx, addedvars.as_mut(), &mut cutoffbound)?;
        }

        reqpseudoobjval = cutoffbound - glbpseudoobjval;
    }
    let mut reqpseudoobjval = reqpseudoobjval;

    // The variables responsible for the propagation are the ones with obj > 0
    // and local lb > global lb, or obj < 0 and local ub < global ub: collect
    // all variables which contribute positively to the pseudo objective value
    // (minimum activity) until we reached the (adjusted) required minimum
    // activity for the inference bound change.

    // First, consider the binary variables.
    if nvars > 0 {
        let vars = pd(scip, prop).minactvars[..nvars].to_vec();
        let first = pd(scip, prop).glbfirstnonfixed as usize;
        let respropuseimplics = pd(scip, prop).respropuseimplics;

        if infinity {
            // If the required minimum activity is minus infinity, we have to add all
            // variables which contribute the local pseudo objective activity.
            for &var in &vars[first..nvars] {
                // Binary variables can have a zero objective value.
                if var == infervar {
                    continue;
                }

                add_conflict_binvar(scip, var, bdchgidx, None, None, false, &mut reqpseudoobjval)?;
            }
        } else {
            let mut v = first;
            while v < nvars && scip.set.is_positive(reqpseudoobjval) {
                let var = vars[v];

                // Binary variables can have a zero objective value.
                if var == infervar || addedvars.as_ref().unwrap().exists(&VarKeys { scip }, &var) {
                    v += 1;
                    continue;
                }

                let implics = std::mem::take(&mut pdm(scip, prop).minactimpls[v]);
                let r = add_conflict_binvar(scip, var, bdchgidx, Some(&implics), addedvars.as_mut(), respropuseimplics, &mut reqpseudoobjval);
                pdm(scip, prop).minactimpls[v] = implics;
                r?;
                v += 1;
            }
        }
    }

    if let Some(t) = addedvars {
        pdm(scip, prop).addedvars = Some(t);
    }

    let vars = pd(scip, prop).objintvars[..pd(scip, prop).nobjintvars as usize].to_vec();

    // Second, consider the non-binary variables.
    let mut v = 0;
    while v < vars.len() && (infinity || scip.set.is_positive(reqpseudoobjval)) {
        let var = vars[v];
        v += 1;

        if var == infervar {
            continue;
        }

        add_conflict_bounds(scip, var, bdchgidx, &mut reqpseudoobjval)?;
    }

    Ok(())
}

/// `propagateCutoffboundVar`: propagates the given variable against the
/// cutoff bound; `prop` is `NONE` for global propagation; returns whether
/// the variable domain was tightened.
#[allow(clippy::too_many_arguments)]
fn propagate_cutoffbound_var(scip: &mut Scip, prop: PropId, var: VarId, inferinfo: i32, objchg: f64, cutoffbound: f64, pseudoobjval: f64, local: bool) -> ScipResult<bool> {
    use crate::quad::*;

    // Collect bounds of the variable.
    let (lb, ub) = if local {
        (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub)
    } else {
        (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub)
    };

    if scip.set.is_feas_eq(lb, ub) {
        return Ok(false);
    }

    // Depending on the objective contribution we can try to tighten the lower
    // or upper bound of the variable.
    let tightened;
    if objchg > 0.0 {
        // The new variable bound is lb + (cutoffbound - pseudoobjval) / objchg.
        let mut newbdq = sum_dd(cutoffbound, -pseudoobjval);
        newbdq = div_qd(newbdq, objchg);
        newbdq = sum_qd(newbdq, lb);
        let newbd = newbdq.to_dbl();

        (_, tightened) = if local {
            scip_var::infer_var_ub_prop(scip, var, newbd, prop, inferinfo, false)?
        } else {
            scip_var::tighten_var_ub_global(scip, var, newbd, false)?
        };
    } else {
        // The new variable bound is ub + (cutoffbound - pseudoobjval) / objchg.
        let mut newbdq = sum_dd(cutoffbound, -pseudoobjval);
        newbdq = div_qd(newbdq, objchg);
        newbdq = sum_qd(newbdq, ub);
        let newbd = newbdq.to_dbl();

        (_, tightened) = if local {
            scip_var::infer_var_lb_prop(scip, var, newbd, prop, inferinfo, false)?
        } else {
            scip_var::tighten_var_lb_global(scip, var, newbd, false)?
        };
    }

    Ok(tightened)
}

/// `propagateCutoffboundBinvar`: propagates the given binary variable
/// against the cutoff bound; returns whether the variable domain was
/// tightened and sets `cutoff` if a cutoff was detected.
#[allow(clippy::too_many_arguments)]
fn propagate_cutoffbound_binvar(scip: &mut Scip, prop: PropId, var: VarId, pos: usize, cutoffbound: f64, pseudoobjval: f64, cutoff: &mut bool, local: bool) -> ScipResult<bool> {
    let mut objimplics = std::mem::take(&mut pdm(scip, prop).minactimpls[pos]);

    // Get objective change in case of fixing the variable to its lower bound
    // (that is zero).
    let lbobjchg = get_minact_objchg(scip, var, &mut objimplics, None, BoundType::Lower, local);

    // Get objective change in case of fixing the variable to its upper bound
    // (that is one).
    let ubobjchg = lbobjchg.and_then(|l| get_minact_objchg(scip, var, &mut objimplics, None, BoundType::Upper, local).map(|u| (l, u)));
    pdm(scip, prop).minactimpls[pos] = objimplics;
    let (lbobjchg, ubobjchg) = ubobjchg?;

    let mut tightened = false;

    // Nothing can be done if the objective contribution is zero independently
    // of the bound.
    if scip.set.is_zero(lbobjchg) && scip.set.is_zero(ubobjchg) {
        return Ok(false);
    }

    // If the lbobjchg and ubobjchg are both able to fix the variable to its
    // upper (1.0) or lower (0.0) bound, respectively, we detected an cutoff.
    // There is no need to use SCIPisLT() in case the objective is integral
    // since the cutoff bound in that case is the upper bound minus 1 plus the
    // SCIPcutoffbounddelta() (which is MIN(100.0 * feastol, 0.0001)). However,
    // if the objective is not integral we have to check w.r.t. an epsilon to
    // avoid numerical problems.
    if scip.set.is_lt(cutoffbound, pseudoobjval + ubobjchg) && scip.set.is_lt(cutoffbound, pseudoobjval + lbobjchg) {
        // Check if conflict analysis is applicable.
        if local && api::is_conflict_analysis_applicable(scip) {
            // Initialize conflict analysis.
            api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, true)?;

            // Add all variable whose best bound changes increased the pseudo
            // objective value above to cutoff bound.
            resolve_propagation(scip, prop, pseudoobjval, NONE, -1, BoundType::Upper, None)?;

            // Analyze the conflict.
            api::analyze_conflict(scip, 0)?;
        }

        *cutoff = true;
    } else {
        // Try to tighten the variable bound use the larger objective
        // contribution.
        let objchg = if lbobjchg > ubobjchg { -lbobjchg } else { ubobjchg };

        tightened = propagate_cutoffbound_var(scip, prop, var, pos as i32, objchg, cutoffbound, pseudoobjval, local)?;
    }

    Ok(tightened)
}

/// `propagateCutoffboundGlobally`: globally propagates if a new cutoff bound
/// or global pseudo objective value (minimum activity of the objective
/// function) is available.
fn propagate_cutoffbound_globally(scip: &mut Scip, prop: PropId, nchgbds: &mut i32, cutoff: &mut bool) -> ScipResult {
    // This method should not be called in the root node of the search tree
    // since the standard propagation already does the job.
    let pseudoobjval = api::get_global_pseudo_objval(scip);
    let cutoffbound = pd(scip, prop).cutoffbound;

    // Nothing can be done if the global pseudo objective is minus infinity.
    if scip.set.is_infinity(-pseudoobjval) {
        return Ok(());
    }

    // Check if the global pseudo objective value (minimum activity of the
    // objective function) is greater or equal to the cutoff bound.
    if scip.set.is_ge(pseudoobjval, cutoffbound) {
        *cutoff = true;
        return Ok(());
    }

    let nminactvars = pd(scip, prop).nminactvars as usize;
    let nobjintvars = pd(scip, prop).nobjintvars as usize;

    *cutoff = false;

    // Always propagate the binary variables completely.
    let mut v = pd(scip, prop).glbfirstnonfixed as usize;
    while v < nminactvars {
        let var = pd(scip, prop).minactvars[v];

        // Check if the variables is already globally fixed; if so continue with
        // the potential candidate.
        if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
            v += 1;
            continue;
        }

        // Propagates the cutoff bound for the given binary variable.
        let tightened = propagate_cutoffbound_binvar(scip, prop, var, v, cutoffbound, pseudoobjval, cutoff, false)?;

        // The binary variables are sorted in non-increasing manner w.r.t. the
        // absolute value of their objective contribution w.r.t. minimum activity
        // (pseudo objective value) of the objective function; these values are
        // the increase in the pseudo objective activity we would get if we fix
        // the variable to its worse bound; hence, we can stop if for a variable
        // this potential increase is not enough too exceed the cutoff bound.
        if !tightened {
            break;
        }

        if *cutoff {
            return Ok(());
        }

        // The variable might not be globally fixed right away since this would
        // destroy the local internal data structure of a search node; the bound
        // change is in that case pending; hence we cannot assert that the
        // variable is globally fixed.
        *nchgbds += 1;
        v += 1;
    }

    {
        let p = pdm(scip, prop);
        p.glbfirstnonfixed = v as i32;
        p.firstnonfixed = crate::def::max(p.firstnonfixed, v as i32);
    }

    // Check all binary variables which could potentially be fixed.
    while v < nminactvars && cutoffbound - pseudoobjval < pd(scip, prop).minactimpls[v].maxobjchg {
        let var = pd(scip, prop).minactvars[v];

        // Check if the variables is already globally fixed; if so continue with
        // the potential candidate.
        if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
            v += 1;
            continue;
        }

        // Propagates the cutoff bound for the given binary variable.
        let tightened = propagate_cutoffbound_binvar(scip, prop, var, v, cutoffbound, pseudoobjval, cutoff, false)?;

        // Check if the domain of the variable was reduced.
        if tightened {
            *nchgbds += 1;
        }

        if *cutoff {
            return Ok(());
        }
        v += 1;
    }

    // Propagate the non-binary variables completely.
    for v in 0..nobjintvars {
        let var = pd(scip, prop).objintvars[v];

        // Try to tighten the bound of the variable.
        let obj = scip.vars[var].obj;
        let tightened = propagate_cutoffbound_var(scip, NONE, var, -1, obj, cutoffbound, pseudoobjval, false)?;

        // Check if the domain of the variable was reduced.
        if tightened {
            *nchgbds += 1;
        }
    }

    pdm(scip, prop).glbpropagated = true;

    Ok(())
}

/// `propagateCutoffboundBinvars`: propagates the cutoff bound for binary
/// variables (c*x <= cutoff).
fn propagate_cutoffbound_binvars(scip: &mut Scip, prop: PropId, cutoffbound: f64, pseudoobjval: f64, nfixedvars: &mut i32, cutoff: &mut bool) -> ScipResult {
    let nminactvars = pd(scip, prop).nminactvars as usize;

    // Always propagate the binary variables completely; note that the variables
    // before the firstnonfixed indexed are already locally fixed and those
    // before glbfirstnonfixed are already globally fixed.
    *cutoff = false;

    let mut v = pd(scip, prop).firstnonfixed as usize;
    while v < nminactvars {
        let var = pd(scip, prop).minactvars[v];

        // Check if the variable is already locally fixed; in that case we just
        // continue with the next potential candidate.
        if scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5 {
            v += 1;
            continue;
        }

        // Propagates the cutoff bound for the given binary variable.
        let tightened = propagate_cutoffbound_binvar(scip, prop, var, v, cutoffbound, pseudoobjval, cutoff, true)?;

        // The binary variables are sorted in non-increasing manner w.r.t. the
        // absolute value of their objective contribution w.r.t. minimum activity
        // of the objective function; these values are the increase in the pseudo
        // objective activity (minimum activity of the objective function) we
        // would get if we fix the variable to its worse bound; hence, we can stop
        // if for a variable this potential increase is not enough too exceed the
        // cutoff bound.
        if !tightened {
            break;
        }

        if *cutoff {
            return Ok(());
        }

        // Check that the binary variable is locally fixed.
        *nfixedvars += 1;
        v += 1;
    }

    pdm(scip, prop).firstnonfixed = v as i32;

    // Check all binary variables which could potentially be fixed.
    while v < nminactvars && cutoffbound - pseudoobjval < pd(scip, prop).minactimpls[v].maxobjchg {
        let var = pd(scip, prop).minactvars[v];

        // Check if the variable is already locally fixed; in that case we just
        // continue with the next potential candidate.
        if scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5 {
            v += 1;
            continue;
        }

        // Propagates the cutoff bound for the given binary variable.
        let tightened = propagate_cutoffbound_binvar(scip, prop, var, v, cutoffbound, pseudoobjval, cutoff, true)?;

        if tightened {
            *nfixedvars += 1;
        }

        if *cutoff {
            return Ok(());
        }
        v += 1;
    }

    Ok(())
}

/// `propagateCutoffbound`: propagates the cutoff bound c*x <= cutoff.
fn propagate_cutoffbound(scip: &mut Scip, prop: PropId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Get current pseudo objective value (minimum activity of the objective
    // function) and cutoff bound.
    let pseudoobjval = api::get_pseudo_objval(scip);
    if scip.set.is_infinity(-pseudoobjval) {
        return Ok(());
    }
    let cutoffbound = api::get_cutoffbound(scip);
    if scip.set.is_infinity(cutoffbound) {
        return Ok(());
    }

    // A new global pseudo objective value could be used to retrieve global
    // fixings. There is, however, no need to check if a new global pseudo
    // objective value is available. This is the case since a new (better)
    // global pseudo activity implies that a global bound change was
    // performed. That causes that the root node of the search tree gets marked
    // for repropagation. That will result in a propagation call of the pseudo
    // objective propagator.

    // Check current cutoff bound.
    if cutoffbound < pd(scip, prop).cutoffbound {
        let p = pdm(scip, prop);
        p.glbpropagated = false;
        p.cutoffbound = cutoffbound;
    }

    let mut nchgbds = 0;
    let mut cutoff = false;
    *result = ScipResultCode::DidNotFind;

    // Check if we have a new cutoff bound; in that case we globally propagate
    // this new bound; there is no need to propagate the cutoff bound if we are
    // in the root node since this will be done by the standard local
    // propagation.
    if pd(scip, prop).propcutoffbound && !pd(scip, prop).glbpropagated && api::get_depth(scip) > 0 {
        // First globally propagate new cutoff bound or pseudo objective activity.
        propagate_cutoffbound_globally(scip, prop, &mut nchgbds, &mut cutoff)?;

        if cutoff {
            // We are done with solving since a global pseudo activity is greater or
            // equal to the cutoff bound.
            let root = api::get_root_node(scip);
            api::cutoff_node(scip, root)?;

            *result = ScipResultCode::Cutoff;
            return Ok(());
        }

        // Check if the global propagation cut off the active/current node.
        if scip.tree.as_ref().unwrap().cutoffdepth <= api::get_depth(scip) {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }
    }

    // Check if the pseudo objective value (minimum activity of the objective
    // function) is greater or equal to the cutoff bound.
    if scip.set.is_ge(pseudoobjval, cutoffbound) {
        // Check if conflict analysis is applicable.
        if api::is_conflict_analysis_applicable(scip) {
            // Initialize conflict analysis.
            api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, true)?;

            // Add all variable whose best bound changes increased the pseudo
            // objective value above the cutoff bound.
            resolve_propagation(scip, prop, cutoffbound, NONE, -1, BoundType::Upper, None)?;

            // Analyze the conflict.
            api::analyze_conflict(scip, 0)?;
        }

        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // Propagate binary variables.
    propagate_cutoffbound_binvars(scip, prop, cutoffbound, pseudoobjval, &mut nchgbds, &mut cutoff)?;

    if cutoff {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // Tighten domains of non-binary variables, if they would increase the
    // pseudo objective value above the cutoff bound.
    let nobjintvars = pd(scip, prop).nobjintvars as usize;
    if pd(scip, prop).propfullinroot && api::get_depth(scip) == 0 {
        // Propagate all non-binary variables.
        for v in 0..nobjintvars {
            let var = pd(scip, prop).objintvars[v];
            let objval = scip.vars[var].obj;

            // Try to tighten the bound of the variable.
            let tightened = propagate_cutoffbound_var(scip, NONE, var, -1, objval, cutoffbound, pseudoobjval, false)?;

            // Check if the domain of the variable was reduced.
            if tightened {
                nchgbds += 1;
            }
        }
    } else {
        // Compute maximum number of useless propagations before aborting (C casts
        // the fraction to int before multiplying).
        let p = pd(scip, prop);
        let nmaxuseless = crate::def::max(p.minuseless, (p.maxvarsfrac as i32) * (nobjintvars as i32));
        let mut nuseless = 0;
        let mut v = p.lastvarnum;

        let mut c = 0;
        while c < nobjintvars && nuseless < nmaxuseless {
            v += 1;
            if v as usize >= nobjintvars {
                v = 0;
            }

            let var = pd(scip, prop).objintvars[v as usize];
            let objval = scip.vars[var].obj;

            // Try to tighten the bound of the variable.
            let tightened = propagate_cutoffbound_var(scip, prop, var, -1, objval, cutoffbound, pseudoobjval, true)?;

            // Check if the domain of the variable was reduced.
            if tightened {
                nchgbds += 1;
            } else {
                nuseless += 1;
            }
            c += 1;
        }
        pdm(scip, prop).lastvarnum = v;
    }

    // Check if we chanced bounds.
    if nchgbds > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `calcMaxObjPseudoactivity`: recalculates the maximum objective
/// pseudoactivity.
fn calc_max_obj_pseudoactivity(scip: &mut Scip, prop: PropId) {
    // Get problem variables.
    let vars = api::get_vars(scip).to_vec();

    // Calculate current max pseudo activity and largest contribution.
    let mut maxpseudoobjact = 0.0;
    let mut maxpseudoobjactinf = 0;

    for var in vars {
        let objval = scip.vars[var].obj;
        let mut contrib;
        if scip.set.is_positive(objval) {
            contrib = scip.vars[var].glbdom.ub;
            if !scip.set.is_infinity(contrib) {
                contrib *= objval;
            }
        } else if scip.set.is_negative(objval) {
            contrib = scip.vars[var].glbdom.lb;
            if !scip.set.is_infinity(-contrib) {
                contrib *= objval;
            } else {
                contrib *= -1.0;
            }
        } else {
            continue;
        }

        if scip.set.is_infinity(contrib) {
            maxpseudoobjactinf += 1;
        } else {
            maxpseudoobjact += contrib;
        }
    }

    let p = pdm(scip, prop);
    p.maxpseudoobjact = maxpseudoobjact;
    p.maxpseudoobjactinf = maxpseudoobjactinf;
}

/// `updateMaxObjPseudoactivity`: updates the pseudo objective activity if
/// necessary.
fn update_max_obj_pseudoactivity(scip: &mut Scip, prop: PropId) {
    // If necessary, calculate the maximum pseudo objective activity.
    if pd(scip, prop).maxpseudoobjact == crate::def::INVALID {
        calc_max_obj_pseudoactivity(scip, prop);
    }
}

/// `getMaxObjPseudoactivityResidualValue`: returns the residual pseudo
/// objective activity without the given value.
fn get_max_obj_pseudoactivity_residual_value(scip: &mut Scip, prop: PropId, contrib: f64) -> f64 {
    // If necessary, calculate the maximum pseudo objective activity.
    if pd(scip, prop).maxpseudoobjact == crate::def::INVALID {
        calc_max_obj_pseudoactivity(scip, prop);
    }

    let inf = scip.set.infinity();
    let p = pd(scip, prop);

    if scip.set.is_infinity(contrib) {
        // Check if this variable yields the only infinite contribution.
        if p.maxpseudoobjactinf == 1 { p.maxpseudoobjact } else { inf }
    } else {
        // Check if there is an infinite contribution.
        if p.maxpseudoobjactinf >= 1 { inf } else { p.maxpseudoobjact - contrib }
    }
}

/// `getMaxObjPseudoactivityResidual`: returns the residual pseudo objective
/// activity.
fn get_max_obj_pseudoactivity_residual(scip: &mut Scip, prop: PropId, var: VarId) -> f64 {
    let objval = scip.vars[var].obj;
    let mut contrib;
    if var_get_worst_bound_type(scip, var) == BoundType::Upper {
        contrib = scip.vars[var].glbdom.ub;
        if !scip.set.is_infinity(contrib) {
            contrib *= objval;
        }
    } else {
        contrib = scip.vars[var].glbdom.lb;
        if !scip.set.is_infinity(-contrib) {
            contrib *= objval;
        } else {
            contrib *= -1.0;
        }
    }

    get_max_obj_pseudoactivity_residual_value(scip, prop, contrib)
}

/// `getMaxObjPseudoactivity`: returns the maximum pseudo objective activity
/// of the objective function.
fn get_max_obj_pseudoactivity(scip: &mut Scip, prop: PropId) -> f64 {
    get_max_obj_pseudoactivity_residual_value(scip, prop, 0.0)
}

/// `propagateLowerboundBinvar`: propagates the global domain of the given
/// binary variable against the lower bound (c*x >= lowerbound); returns
/// `(infeasible, tightened)`.
fn propagate_lowerbound_binvar(scip: &mut Scip, var: VarId, lowerbound: f64, maxpseudoobjact: f64, useimplics: bool) -> ScipResult<(bool, bool)> {
    // @todo Instead of running always over all implications use
    // SCIP_OBJIMPLICS in the same way as for the propagation of the minimum
    // activity against the cutoff bound. If so we could use the cliques as
    // well.

    // Get contribution of variable by fixing it to its lower bound w.r.t.
    // maximum activity of the objective function.
    let lbobjchg = get_maxact_objchg(scip, var, BoundType::Lower, useimplics);

    // Get contribution of variable by fixing it to its upper bound w.r.t.
    // maximum activity of the objective function.
    let ubobjchg = get_maxact_objchg(scip, var, BoundType::Upper, useimplics);

    // If the maximum activity of the objective function without the
    // contribution of the given variable shrinks below the global lower bound,
    // the contribution of the variable is need; hence, we can fix it to
    // corresponding bound globally.
    if scip.set.is_feas_lt(maxpseudoobjact + lbobjchg, lowerbound) && scip.set.is_feas_lt(maxpseudoobjact + ubobjchg, lowerbound) {
        // Fixing the variable to zero or one leads to decreases of the maximum
        // activity below the lower bound, hence, we detected an cutoff.
        Ok((true, false))
    } else if scip.set.is_feas_lt(maxpseudoobjact + lbobjchg, lowerbound) {
        scip_var::tighten_var_lb_global(scip, var, 1.0, false)
    } else if scip.set.is_feas_lt(maxpseudoobjact + ubobjchg, lowerbound) {
        // C tightens the lower bound to 0.0 here, which changes nothing.
        scip_var::tighten_var_lb_global(scip, var, 0.0, false)
    } else {
        Ok((false, false))
    }
}

/// `propagateLowerboundVar`: propagates the global domains of the given
/// variable with non-zero objective coefficient against the lower bound
/// (c*x >= lowerbound); returns `(infeasible, tightened)`.
fn propagate_lowerbound_var(scip: &mut Scip, prop: PropId, var: VarId, lowerbound: f64) -> ScipResult<(bool, bool)> {
    let objval = scip.vars[var].obj;

    // Get residual pseudo objective activity, that is the pseudo objective
    // activity without the given variable.
    let residual = get_max_obj_pseudoactivity_residual(scip, prop, var);

    if scip.set.is_infinity(residual) {
        return Ok((false, false));
    }

    // Compute potential mew bound.
    let newbd = (lowerbound - residual) / objval;

    // In case the variable is integral we force the update of the new bound.
    if objval > 0.0 {
        let lb = scip.vars[var].glbdom.lb;

        if !crate::var::var_is_integral(scip, var) {
            return scip_var::tighten_var_lb_global(scip, var, newbd, false);
        } else if scip.set.is_feas_gt(newbd, lb) {
            return scip_var::tighten_var_lb_global(scip, var, newbd, true);
        }
    } else {
        let ub = scip.vars[var].glbdom.ub;

        if !crate::var::var_is_integral(scip, var) {
            return scip_var::tighten_var_ub_global(scip, var, newbd, false);
        } else if scip.set.is_feas_lt(newbd, ub) {
            return scip_var::tighten_var_ub_global(scip, var, newbd, true);
        }
    }

    Ok((false, false))
}

/// `propagateLowerbound`: propagates the global lower (dual) bound c*x >=
/// lowerbound.
fn propagate_lowerbound(scip: &mut Scip, prop: PropId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;
    let mut cutoff = false;
    let mut nchgbds = 0;

    // Check if there is a chance to find a reduction.
    let lowerbound = api::get_lowerbound(scip);
    if scip.set.is_infinity(-lowerbound) {
        return Ok(());
    }

    // If the lower bound did not change since the last propagation as well as
    // the global bounds of the variables with a non-zero objective coefficient
    // we do nothing since there is no new information available.
    if scip.set.is_le(lowerbound, pd(scip, prop).lastlowerbound) && pd(scip, prop).maxpseudoobjact < crate::def::INVALID {
        return Ok(());
    }

    // Updates the pseudo objective activity if necessary.
    update_max_obj_pseudoactivity(scip, prop);

    // If more than one variable contributes an infinity to the maximal pseudo
    // activity we can do nothing.
    if pd(scip, prop).maxpseudoobjactinf > 1 {
        return Ok(());
    }

    let mut maxpseudoobjact = get_max_obj_pseudoactivity(scip, prop);

    // If the maximum pseudo objective activity is smaller than the lower bound
    // the problem is infeasible.
    if scip.set.is_dualfeas_lt(maxpseudoobjact, lowerbound) {
        cutoff = true;
    } else {
        let useimplics = pd(scip, prop).propuseimplics;

        if pd(scip, prop).maxpseudoobjactinf == 0 && !scip.set.is_infinity(maxpseudoobjact) {
            let nmaxactvars = pd(scip, prop).nmaxactvars as usize;

            let mut v = pd(scip, prop).maxactfirstnonfixed as usize;
            while v < nmaxactvars {
                let var = pd(scip, prop).maxactvars[v];

                // Check if the variables is already globally fixed; if so continue
                // with the next potential candidate.
                if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
                    v += 1;
                    continue;
                }

                // Try to propagate variable domain globally.
                let tightened;
                (cutoff, tightened) = propagate_lowerbound_binvar(scip, var, lowerbound, maxpseudoobjact, useimplics)?;

                // The binary variables are sorted in non-increasing manner w.r.t. the
                // absolute value of their objective contribution w.r.t. maximum
                // activity of the objective function; these values are the decrease
                // we would get with the maximum pseudo objective activity if we fix
                // the variable to its best bound; hence, we can stop if for a
                // variable this potential decrease is not enough anymore to fall
                // below the lower bound. In case a fixing was performed, the variable
                // might not be globally fixed right away since this would destroy the
                // local internal data structure of a search node; the bound change is
                // in that case pending; hence we cannot assert that the variable is
                // globally fixed.
                if !tightened {
                    break;
                }

                if cutoff {
                    break;
                }

                // Update maximum pseudo activity since the previous global bound
                // change might invalidated the maximum pseudo activity.
                maxpseudoobjact = get_max_obj_pseudoactivity(scip, prop);
                nchgbds += 1;
                v += 1;
            }

            // Update globally fixed index if abort criterion was applied.
            pdm(scip, prop).maxactfirstnonfixed = v as i32;

            // Check all binary variables which could potentially be fixed.
            while v < nmaxactvars && maxpseudoobjact - lowerbound < pd(scip, prop).maxactchgs[v] && !cutoff {
                let var = pd(scip, prop).maxactvars[v];

                // Check if the variables is already globally fixed; if so continue
                // with the potential candidate.
                if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
                    v += 1;
                    continue;
                }

                // Propagates the cutoff bound for the given binary variable.
                let tightened;
                (cutoff, tightened) = propagate_lowerbound_binvar(scip, var, lowerbound, maxpseudoobjact, useimplics)?;

                if tightened {
                    // Update maximum pseudo activity since the previous global bound
                    // change might invalidated the maximum pseudo activity.
                    maxpseudoobjact = get_max_obj_pseudoactivity(scip, prop);
                    nchgbds += 1;
                }
                v += 1;
            }
        }

        let nobjintvars = pd(scip, prop).nobjintvars as usize;

        // Propagate c*x >= lowerbound for non-binary variables.
        let mut v = 0;
        while v < nobjintvars && !cutoff {
            let var = pd(scip, prop).objintvars[v];

            // Try to propagate variable domain globally.
            let tightened;
            (cutoff, tightened) = propagate_lowerbound_var(scip, prop, var, lowerbound)?;

            if tightened {
                nchgbds += 1;
            }
            v += 1;
        }
    }

    // Evaluate propagation results.
    if cutoff {
        // We are done with solving since a global bound change is infeasible:
        // cutoff root node.
        let root = api::get_root_node(scip);
        api::cutoff_node(scip, root)?;
        *result = ScipResultCode::Cutoff;
    } else if nchgbds > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    // Remember the lower bound which we already propagated.
    pdm(scip, prop).lastlowerbound = lowerbound;

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propFreePseudoobj`: destructor of propagator to free user data (called
/// when SCIP is exiting).
fn prop_free_pseudoobj(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Free propagator data.
    scip.props[prop].propdata = None;
    Ok(())
}

/// `propInitsolPseudoobj`: solving process initialization method of
/// propagator (called when branch and bound process is about to begin).
fn prop_initsol_pseudoobj(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Do nothing if active pricer are present and force flag is not TRUE.
    if !pd(scip, prop).force && api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // If active pricer are present we want to catch the variable added event
    // (pricing is not ported).
    if api::get_n_active_pricers(scip) > 0 {
        return Err(Retcode::InvalidCall);
    }

    Ok(())
}

/// `propExitsolPseudoobj`: solving process deinitialization method of
/// propagator (called before branch and bound process data is freed).
fn prop_exitsol_pseudoobj(scip: &mut Scip, prop: PropId, _restart: bool) -> ScipResult {
    // The variable added event is only caught with active pricers, which the
    // port does not have. Free propagator data.
    propdata_exit(scip, prop)
}

/// `propPresolPseudoobj`: presolving method of propagator.
fn prop_presol_pseudoobj(
    scip: &mut Scip,
    prop: PropId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do nothing if active pricer are present and force flag is not TRUE.
    if !pd(scip, prop).force && api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // Do nothing if objective propagation is not allowed.
    if !scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    let pseudoobjval = api::get_global_pseudo_objval(scip);
    if scip.set.is_infinity(-pseudoobjval) {
        return Ok(());
    }

    let cutoffbound = api::get_cutoffbound(scip);
    if scip.set.is_infinity(cutoffbound) {
        return Ok(());
    }

    if scip.set.is_ge(pseudoobjval, cutoffbound) {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // Only propagate if a new cutoff bound or global pseudo objective value is
    // available.
    if cutoffbound < pd(scip, prop).cutoffbound || pseudoobjval > pd(scip, prop).glbpseudoobjval {
        *result = ScipResultCode::DidNotFind;
        let oldnchgbds = counts.nchgbds;

        // Get the problem variables (the array, read live: fixings change it).
        let nvars = api::get_n_vars(scip);

        // Scan the variables for pseudoobj bound reductions (loop backwards,
        // since a variable fixing can change the current and the subsequent
        // slots in the vars array).
        for v in (0..nvars).rev() {
            let var = scip.probs[crate::prob::TRANSPROB].vars[v];
            let objval = scip.vars[var].obj;

            if scip.set.is_zero(objval) {
                continue;
            }

            let tightened = propagate_cutoffbound_var(scip, NONE, var, -1, objval, cutoffbound, pseudoobjval, false)?;

            if tightened {
                counts.nchgbds += 1;
            }
        }

        // Evaluate if bound change was detected.
        if counts.nchgbds > oldnchgbds {
            *result = ScipResultCode::Success;
        }

        // Store the old values.
        let p = pdm(scip, prop);
        p.cutoffbound = cutoffbound;
        p.glbpseudoobjval = pseudoobjval;
        p.glbpropagated = true;
    }

    Ok(())
}

/// `propExecPseudoobj`: execution method of propagator.
fn prop_exec_pseudoobj(scip: &mut Scip, prop: PropId, proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if api::in_probing(scip) {
        return Ok(());
    }

    if proptiming == PROPTIMING_DURINGLPLOOP && api::get_depth(scip) != 0 {
        return Ok(());
    }

    // Do nothing if active pricer are present and force flag is not TRUE.
    if !pd(scip, prop).force && api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // Do not run if propagation w.r.t. objective is not allowed.
    if !scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    // Check if enough new variable are added (due to column generation to
    // reinitialized the propagator data).
    if !pd(scip, prop).initialized || pd(scip, prop).nnewvars > pd(scip, prop).maxnewvars {
        // Free current propdata data.
        propdata_exit(scip, prop)?;

        // Initialize propdata data from scratch.
        propdata_init(scip, prop)?;
    }

    // Nothing to do for zero objective.
    let p = pd(scip, prop);
    if p.nminactvars == 0 && p.nmaxactvars == 0 && p.nobjintvars == 0 {
        return Ok(());
    }

    // Propagate c*x <= cutoff.
    propagate_cutoffbound(scip, prop, result)?;

    let p = pd(scip, prop);
    if *result != ScipResultCode::Cutoff && (p.nmaxactvars > 0 || p.nobjintvars > 0) && scip.set.stage == Stage::Solving {
        let mut dualresult = ScipResultCode::DidNotRun;

        // Propagates the global lower (dual) bound c*x >= lowerbound.
        propagate_lowerbound(scip, prop, &mut dualresult)?;

        if dualresult == ScipResultCode::ReducedDom || dualresult == ScipResultCode::Cutoff {
            *result = dualresult;
        }
    }

    Ok(())
}

/// `propRespropPseudoobj`: propagation conflict resolving method of
/// propagator.
#[allow(clippy::too_many_arguments)]
fn prop_resprop_pseudoobj(
    scip: &mut Scip,
    prop: PropId,
    infervar: VarId,
    inferinfo: i32,
    boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    let cutoffbound = api::get_cutoffbound(scip);

    // Resolve the propagation of the inference variable w.r.t. required
    // minactivity.
    resolve_propagation(scip, prop, cutoffbound, infervar, inferinfo, boundtype, bdchgidx.as_ref())?;

    *result = ScipResultCode::Success;

    Ok(())
}

/*
 * Event handler
 */

/// `eventExecPseudoobj`: execution method of bound change event handler;
/// the event data is the propagator.
fn event_exec_pseudoobj(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let prop: PropId = eventdata;
    let p = pdm(scip, prop);

    match event.eventtype {
        EVENTTYPE_LBRELAXED | EVENTTYPE_UBRELAXED => {
            // If bound got relaxed we need to start up front for trial of bound
            // tightening.
            p.firstnonfixed = 0;
        }
        EVENTTYPE_VARADDED => {
            p.nnewvars += 1;
        }
        _ => {
            // Invalidate the maximum pseudo objective activity.
            p.maxpseudoobjact = crate::def::INVALID;
            p.maxpseudoobjactinf = 0;
        }
    }

    Ok(())
}

/*
 * propagator specific interface methods
 */

/// `propCopyPseudoobj`: copy method for propagator plugins (called when SCIP
/// copies plugins).
fn prop_copy_pseudoobj(scip: &mut Scip, _sourcescip: &Scip, _prop: PropId) -> ScipResult {
    // Call inclusion method of propagator.
    include_prop_pseudoobj(scip)?;
    Ok(())
}

/// `SCIPincludePropPseudoobj`: creates the pseudo objective function
/// propagator and includes it in SCIP.
pub fn include_prop_pseudoobj(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Include event handler for gloabl bound change events and variable added
    // event (in case of pricing).
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_pseudoobj, None)?;

    // Create pseudoobj propagator data and reset propagator data structure.
    let mut propdata = PropData {
        eventhdlr,
        minactvars: Vec::new(),
        minactimpls: Vec::new(),
        maxactvars: Vec::new(),
        maxactchgs: Vec::new(),
        objintvars: Vec::new(),
        addedvars: None,
        lastlowerbound: 0.0,
        cutoffbound: 0.0,
        glbpseudoobjval: 0.0,
        maxvarsfrac: DEFAULT_MAXVARSFRAC,
        maxpseudoobjact: 0.0,
        maxpseudoobjactinf: 0,
        nminactvars: 0,
        nmaxactvars: 0,
        nobjintvars: 0,
        minuseless: DEFAULT_MINUSELESS,
        lastvarnum: 0,
        glbfirstnonfixed: 0,
        maxactfirstnonfixed: 0,
        firstnonfixed: 0,
        nnewvars: 0,
        maxnewvars: DEFAULT_MAXNEWVARS,
        maximplvars: DEFAULT_MAXIMPLVARS,
        minactsize: 0,
        maxactsize: 0,
        objintvarssize: 0,
        glbpropagated: false,
        propfullinroot: DEFAULT_PROPFULLINROOT,
        propcutoffbound: DEFAULT_PROPCUTOFFBOUND,
        force: DEFAULT_FORCE,
        catchvaradded: false,
        propuseimplics: DEFAULT_PROPUSEIMPLICS,
        respropuseimplics: DEFAULT_RESPROPUSEIMPLICS,
        initialized: false,
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
        prop_exec_pseudoobj,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;
    scip.props[prop].propcopy = Some(prop_copy_pseudoobj);

    // Set optional callbacks via setter functions.
    scip.props[prop].propfree = Some(prop_free_pseudoobj);
    scip.props[prop].propinitsol = Some(prop_initsol_pseudoobj);
    scip.props[prop].propexitsol = Some(prop_exitsol_pseudoobj);
    crate::prop::prop_set_presol(scip, prop, prop_presol_pseudoobj, PROP_PRESOL_PRIORITY, PROP_PRESOL_MAXROUNDS, PROP_PRESOLTIMING)?;
    scip.props[prop].propresprop = Some(prop_resprop_pseudoobj);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, props, prop, propdata, PropData, $f)
        };
    }

    // Add pseudoobj propagator parameters.
    paramset_add_int(
        scip,
        &format!("propagating/{PROP_NAME}/minuseless"),
        "minimal number of successive non-binary variable propagations without a bound reduction before aborted",
        t!(i32, minuseless),
        true,
        DEFAULT_MINUSELESS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("propagating/{PROP_NAME}/maxvarsfrac"),
        "maximal fraction of non-binary variables with non-zero objective without a bound reduction before aborted",
        t!(f64, maxvarsfrac),
        true,
        DEFAULT_MAXVARSFRAC,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/propfullinroot"),
        "whether to propagate all non-binary variables when we are propagating the root node",
        t!(bool, propfullinroot),
        true,
        DEFAULT_PROPFULLINROOT,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/propcutoffbound"),
        "propagate new cutoff bound directly globally",
        t!(bool, propcutoffbound),
        true,
        DEFAULT_PROPCUTOFFBOUND,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/force"),
        "should the propagator be forced even if active pricer are present?",
        t!(bool, force),
        true,
        DEFAULT_FORCE,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{PROP_NAME}/maxnewvars"),
        "number of variables added after the propagator is reinitialized?",
        t!(i32, maxnewvars),
        true,
        DEFAULT_MAXNEWVARS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/propuseimplics"),
        "use implications to strengthen the propagation of binary variable (increasing the objective change)?",
        t!(bool, propuseimplics),
        true,
        DEFAULT_PROPUSEIMPLICS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("propagating/{PROP_NAME}/respropuseimplics"),
        "use implications to strengthen the resolve propagation of binary variable (increasing the objective change)?",
        t!(bool, respropuseimplics),
        true,
        DEFAULT_RESPROPUSEIMPLICS,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{PROP_NAME}/maximplvars"),
        "maximum number of binary variables the implications are used if turned on (-1: unlimited)?",
        t!(i32, maximplvars),
        true,
        DEFAULT_MAXIMPLVARS,
        -1,
        i32::MAX,
        None,
    )?;

    Ok(())
}

/// `SCIPpropagateCutoffboundVar`: propagates the cutoff bound for the given
/// variables; returns whether the domain was tightened.
pub fn propagate_cutoffbound_var_public(scip: &mut Scip, prop: PropId, var: VarId, cutoffbound: f64, pseudoobjval: f64) -> ScipResult<bool> {
    let objval = scip.vars[var].obj;
    propagate_cutoffbound_var(scip, prop, var, -1, objval, cutoffbound, pseudoobjval, true)
}
