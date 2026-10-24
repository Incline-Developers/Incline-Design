//! Port of `cons_logicor.c`: constraint handler for logic or constraints
//! (equivalent to set covering, but algorithms are suited for depth first
//! search), `1^T x >= 1`.
//!
//! Not ported: the NLP rows (the NLP is not ported), printing and parsing.

use std::any::Any;

use crate::{
    api,
    cons::{ConsFlags, *},
    cvec::CVec,
    event::*,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var, sort,
    types::{BoundType, ConsId, ConshdlrId, EventhdlrId, LockType, ScipResultCode, SolId, VarId, VarStatus},
    var::{BdChgIdx, NONE, var_compare},
};

/* constraint handler properties */
pub const CONSHDLR_NAME: &str = "logicor";
const CONSHDLR_DESC: &str = "logic or constraints";
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 10000;
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -2000000;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -2000000;
/// Frequency for separating cuts; zero means to separate only in the root
/// node.
const CONSHDLR_SEPAFREQ: i32 = 0;
/// Frequency for propagating domains; zero means only preprocessing
/// propagation.
const CONSHDLR_PROPFREQ: i32 = 1;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0
/// for first only.
const CONSHDLR_EAGERFREQ: i32 = 100;
/// Maximal number of presolving rounds the constraint handler participates
/// in (-1: no limit).
const CONSHDLR_MAXPREROUNDS: i32 = -1;
/// Should separation method be delayed, if other separators found cuts?
const CONSHDLR_DELAYSEPA: bool = false;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const CONSHDLR_DELAYPROP: bool = false;
/// Should the constraint handler be skipped, if no constraints are
/// available?
const CONSHDLR_NEEDSCONS: bool = true;

const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_ALWAYS;
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

/// Priority of the constraint handler for upgrading of linear constraints.
const LINCONSUPGD_PRIORITY: i32 = 800000;

const EVENTHDLR_NAME: &str = "logicor";
const EVENTHDLR_DESC: &str = "event handler for logic or constraints";

const CONFLICTHDLR_NAME: &str = "logicor";
const CONFLICTHDLR_DESC: &str = "conflict handler creating logic or constraints";
const CONFLICTHDLR_PRIORITY: i32 = LINCONSUPGD_PRIORITY;

/// Should pairwise constraint comparison be performed in presolving?
const DEFAULT_PRESOLPAIRWISE: bool = true;
/// Should pairwise constraint comparison try to strengthen constraints by
/// removing superflous non-zeros?
const DEFAULT_STRENGTHEN: bool = true;

/// Minimal size of hash table in logicor constraint tables.
const HASHSIZE_LOGICORCONS: i32 = 500;
/// Should hash table be used for detecting redundant constraints in
/// advance.
const DEFAULT_PRESOLUSEHASHING: bool = true;
/// Should dual presolving steps be performed?
const DEFAULT_DUALPRESOLVING: bool = true;
/// Should negated clique information be used in presolving.
const DEFAULT_NEGATEDCLIQUE: bool = true;
/// Should we try to shrink the variables and derive global boundchanges by
/// using cliques and implications.
const DEFAULT_IMPLICATIONS: bool = true;

/// `AGEINCREASE`.
#[inline]
fn ageincrease(n: f64) -> f64 {
    1.0 + 0.2 * n
}

/// The events a logicor constraint catches on its watched variables.
const WATCHED_EVENTS: EventType = EVENTTYPE_UBTIGHTENED | EVENTTYPE_LBRELAXED;

/*
 * Data structures
 */

/// `SCIP_ConshdlrData`: constraint handler data.
pub struct ConshdlrData {
    /// Event handler for events on watched variables.
    pub eventhdlr: EventhdlrId,
    /// Linear constraint handler or `NONE` if not included.
    pub conshdlrlinear: ConshdlrId,
    /// Setppc constraint handler or `NONE` if not included.
    pub conshdlrsetppc: ConshdlrId,
    /// Should pairwise constraint comparison be performed in presolving?
    pub presolpairwise: bool,
    /// Should hash table be used for detecting redundant constraints in
    /// advance.
    pub presolusehashing: bool,
    /// Should dual presolving steps be performed?
    pub dualpresolving: bool,
    /// Should negated clique information be used in presolving.
    pub usenegatedclique: bool,
    /// Should we try to shrink the variables and derive global boundchanges
    /// by using clique and implications.
    pub useimplications: bool,
    /// Should pairwise constraint comparison try to strengthen constraints
    /// by removing superflous non-zeros?
    pub usestrengthening: bool,
    /// Number of cliques after last negated clique presolving round.
    pub nlastcliquesneg: i32,
    /// Number of implications after last negated clique presolving round.
    pub nlastimplsneg: i32,
    /// Number of cliques after last shortening of constraints.
    pub nlastcliquesshorten: i32,
    /// Number of implications after last shortening of constraints.
    pub nlastimplsshorten: i32,
}

/// `SCIP_ConsData`: logic or constraint data.
#[derive(Default)]
pub struct ConsData {
    /// LP row, if constraint is already stored in LP row format.
    pub row: usize,
    /// Variables of the constraint.
    pub vars: CVec<VarId>,
    /// Size of vars array.
    pub varssize: i32,
    /// Number of variables in the constraint.
    pub nvars: i32,
    /// Position of the first watched variable.
    pub watchedvar1: i32,
    /// Position of the second watched variable.
    pub watchedvar2: i32,
    /// Event filter position of first watched variable.
    pub filterpos1: i32,
    /// Event filter position of second watched variable.
    pub filterpos2: i32,
    /// Constraint signature which is need for pairwise comparison.
    pub signature: u32,
    /// Flag indicates if we have some fixed, aggregated or multi-aggregated
    /// variables.
    pub presolved: bool,
    /// Was the 2-variable logic or constraint already added as implication?
    pub impladded: bool,
    /// Are the constraint's variables sorted?
    pub sorted: bool,
    /// Was constraint changed since last redundancy round in preprocessing?
    pub changed: bool,
    /// Are the constraint's equal/negated variables already merged?
    pub merged: bool,
    /// Does this constraint contain aggregations.
    pub existmultaggr: bool,
    /// Is the signature valid.
    pub validsignature: bool,
}

/// The constraint data of a logicor constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a logicor constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the logicor constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/// The constraint handler data of the logicor constraint handler, mutably.
#[inline]
fn hdm(scip: &mut Scip, h: ConshdlrId) -> &mut ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_mut().unwrap().downcast_mut::<ConshdlrData>().unwrap()
}

/*
 * Local methods
 */

/// `lockRounding`: installs rounding locks for the given variable in the
/// given logic or constraint.
fn lock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    api::lock_var_cons(scip, var, cons, true, false)
}

/// `unlockRounding`: removes rounding locks for the given variable in the
/// given logic or constraint.
fn unlock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    api::unlock_var_cons(scip, var, cons, true, false)
}

/// `conshdlrdataCreate`: creates constraint handler data for logic or
/// constraint handler.
fn conshdlrdata_create(eventhdlr: EventhdlrId) -> ConshdlrData {
    ConshdlrData {
        nlastcliquesneg: 0,
        nlastimplsneg: 0,
        nlastcliquesshorten: 0,
        nlastimplsshorten: 0,
        // Set event handler for catching events on watched variables.
        eventhdlr,
        conshdlrlinear: NONE,
        conshdlrsetppc: NONE,
        presolpairwise: false,
        presolusehashing: false,
        dualpresolving: false,
        usenegatedclique: false,
        useimplications: false,
        usestrengthening: false,
    }
}

/// `consdataEnsureVarsSize`: ensures, that the vars array can store at
/// least num entries.
fn consdata_ensure_vars_size(scip: &Scip, c: &mut ConsData, num: i32) {
    if num > c.varssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        c.vars.0.resize(newsize as usize, NONE);
        c.varssize = newsize;
    }
}

/// `consdataCreate`: creates a logic or constraint data object.
fn consdata_create(scip: &mut Scip, vars: &[VarId]) -> ScipResult<ConsData> {
    let nvars = vars.len() as i32;
    let mut c = ConsData { row: NONE, ..ConsData::default() };

    if nvars > 0 {
        c.vars = CVec(vars.to_vec());
        c.varssize = nvars;
        c.nvars = nvars;
    }
    c.watchedvar1 = -1;
    c.watchedvar2 = -1;
    c.filterpos1 = -1;
    c.filterpos2 = -1;
    c.presolved = false;
    c.impladded = false;
    c.changed = true;
    c.sorted = nvars <= 1;
    c.merged = nvars <= 1;
    c.existmultaggr = false;
    c.validsignature = false;

    // Get transformed variables, if we are in the transformed problem.
    if api::is_transformed(scip) {
        let n = c.nvars as usize;
        api::get_transformed_vars(scip, &mut c.vars.0[..n])?;

        // Check for multi-aggregations and capture variables.
        for v in 0..c.nvars {
            let var = crate::var::var_get_probvar(scip, c.vars[v]);
            c.existmultaggr = c.existmultaggr || scip.vars[var].varstatus == VarStatus::MultAggr;
            crate::var::var_capture(scip, c.vars[v]);
        }
    } else {
        // Capture variables.
        for v in 0..c.nvars {
            crate::var::var_capture(scip, c.vars[v]);
        }
    }

    Ok(c)
}

/// `consdataFree`: frees a logic or constraint data.
fn consdata_free(scip: &mut Scip, c: &mut ConsData) -> ScipResult {
    // Release the row.
    if c.row != NONE {
        crate::lp::row_release(scip, c.row)?;
        c.row = NONE;
    }

    // Release variables.
    for v in 0..c.nvars {
        let mut var = c.vars[v];
        api::release_var(scip, &mut var)?;
        c.vars[v] = var;
    }

    Ok(())
}

/// `switchWatchedvars`: stores the given variable numbers as watched
/// variables, and updates the event processing; the event data is the
/// constraint.
fn switch_watchedvars(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, watchedvar1: i32, watchedvar2: i32) -> ScipResult {
    // If one watched variable is equal to the old other watched variable,
    // just switch positions.
    let c = cdm(scip, cons);
    if watchedvar1 == c.watchedvar2 || watchedvar2 == c.watchedvar1 {
        std::mem::swap(&mut c.watchedvar1, &mut c.watchedvar2);
        std::mem::swap(&mut c.filterpos1, &mut c.filterpos2);
    }

    // Drop events on old watched variables.
    let c = cd(scip, cons);
    if c.watchedvar1 != -1 && c.watchedvar1 != watchedvar1 {
        let (var, filterpos) = (c.vars[c.watchedvar1], c.filterpos1);
        api::drop_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, filterpos)?;
    }
    let c = cd(scip, cons);
    if c.watchedvar2 != -1 && c.watchedvar2 != watchedvar2 {
        let (var, filterpos) = (c.vars[c.watchedvar2], c.filterpos2);
        api::drop_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, filterpos)?;
    }

    // Catch events on new watched variables.
    if watchedvar1 != -1 && watchedvar1 != cd(scip, cons).watchedvar1 {
        let var = cd(scip, cons).vars[watchedvar1];
        let mut filterpos = -1;
        api::catch_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, Some(&mut filterpos))?;
        cdm(scip, cons).filterpos1 = filterpos;
    }
    if watchedvar2 != -1 && watchedvar2 != cd(scip, cons).watchedvar2 {
        let var = cd(scip, cons).vars[watchedvar2];
        let mut filterpos = -1;
        api::catch_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, Some(&mut filterpos))?;
        cdm(scip, cons).filterpos2 = filterpos;
    }

    // Set the new watched variables.
    let c = cdm(scip, cons);
    c.watchedvar1 = watchedvar1;
    c.watchedvar2 = watchedvar2;

    Ok(())
}

/// `addCoef`: adds coefficient in logicor constraint.
fn add_coef(scip: &mut Scip, cons: ConsId, mut var: VarId) -> ScipResult {
    // Are we in the transformed problem?
    let transformed = api::cons_is_transformed(scip, cons);

    // Always use transformed variables in transformed constraints.
    if transformed {
        var = api::get_transformed_var(scip, var)?;

        let probvar = crate::var::var_get_probvar(scip, var);
        if !cd(scip, cons).existmultaggr && scip.vars[probvar].varstatus == VarStatus::MultAggr {
            cdm(scip, cons).existmultaggr = true;
        }

        cdm(scip, cons).presolved = false;
    }

    let nvars = cd(scip, cons).nvars;
    let mut data = scip.conss[cons].consdata.take().unwrap();
    consdata_ensure_vars_size(scip, data.downcast_mut::<ConsData>().unwrap(), nvars + 1);
    scip.conss[cons].consdata = Some(data);
    let c = cdm(scip, cons);
    c.vars[c.nvars] = var;
    crate::var::var_capture(scip, var);
    cdm(scip, cons).nvars += 1;

    // We only catch this event in presolving stage.
    if scip.set.stage == Stage::Presolving || scip.set.stage == Stage::InitPresolve {
        let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        api::catch_var_event(scip, var, EVENTTYPE_VARFIXED, eventhdlr, cons, None)?;
    }

    let c = cdm(scip, cons);
    c.sorted = c.nvars == 1;
    c.changed = true;
    c.validsignature = false;

    // Install the rounding locks for the new variable.
    lock_rounding(scip, cons, var)?;

    // Add the new coefficient to the LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        api::add_var_to_row(scip, row, var, 1.0)?;
    }

    cdm(scip, cons).merged = false;

    Ok(())
}

/// `delCoefPos`: deletes coefficient at given position from logic or
/// constraint data.
fn del_coef_pos(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    // Remove the rounding locks of variable.
    let var = cd(scip, cons).vars[pos];
    unlock_rounding(scip, cons, var)?;

    // We only catch this event in presolving stage, so we need to only drop it
    // there.
    if scip.set.stage == Stage::Presolving || scip.set.stage == Stage::InitPresolve {
        api::drop_var_event(scip, var, EVENTTYPE_VARFIXED, eventhdlr, cons, -1)?;
    }

    if api::cons_is_transformed(scip, cons) {
        // If the position is watched, stop watching the position.
        if cd(scip, cons).watchedvar1 == pos {
            let w2 = cd(scip, cons).watchedvar2;
            switch_watchedvars(scip, cons, eventhdlr, w2, -1)?;
        }
        if cd(scip, cons).watchedvar2 == pos {
            let w1 = cd(scip, cons).watchedvar1;
            switch_watchedvars(scip, cons, eventhdlr, w1, -1)?;
        }
    }

    // Release variable.
    let mut v = var;
    api::release_var(scip, &mut v)?;
    cdm(scip, cons).vars[pos] = v;

    // Move the last variable to the free slot.
    let c = cdm(scip, cons);
    if pos != c.nvars - 1 {
        c.vars[pos] = c.vars[c.nvars - 1];
        c.sorted = false;
    }
    c.nvars -= 1;

    // If the last variable (that moved) was watched, update the watched
    // position.
    if c.watchedvar1 == c.nvars {
        c.watchedvar1 = pos;
    }
    if c.watchedvar2 == c.nvars {
        c.watchedvar2 = pos;
    }

    c.changed = true;
    c.validsignature = false;

    cons_enable_propagation(scip, cons);

    Ok(())
}

/// `dualPresolving`: in case a part (more than one variable) in the logic or
/// constraint is independent of every else, we can perform dual reductions;
/// - fix the variable with the smallest object coefficient to one if the
///   constraint is not modifiable and all variable are independant
/// - fix all independant variables with negative object coefficient to one
/// - fix all remaining independant variables to zero
///
/// also added the special case were exactly one variable is locked by this
/// constraint and another variable without any uplocks has a better
/// objective value than this single variable
/// - here we fix the variable to 0.0 (if the objective contribution is
///   non-negative)
///
/// Moreover, if there exists a variable that is only locked by a constraint
/// with two variables, one can aggregate variables.
///
/// Note: the following dual reduction for logic or constraints is already
/// performed by the presolver "dualfix"
///   - if a variable in a set covering constraint is only locked by that
///     constraint and has negative or zero objective coefficient than it can
///     be fixed to one
///
/// Returns whether C's result is set to `SCIP_SUCCESS`.
#[allow(clippy::too_many_arguments)]
fn dual_presolving(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, nfixedvars: &mut i32, ndelconss: &mut i32, nchgcoefs: &mut i32, naggrvars: &mut i32) -> ScipResult<bool> {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};
    const M: LockType = LockType::Model;

    // Constraints for which the check flag is set to FALSE, did not contribute
    // to the lock numbers; therefore, we cannot use the locks to decide for a
    // dual reduction using this constraint; for example after a restart the
    // cuts which are added to the problems have the check flag set to FALSE.
    if !scip.conss[cons].is_checked() {
        return Ok(false);
    }

    let mut nvars = cd(scip, cons).nvars;

    // We don't want to consider small constraints (note that the constraints
    // can be modifiable, so we can't delete this constraint).
    if nvars < 2 {
        return Ok(false);
    }

    let mut idx: i32 = -1;
    let mut idxnouplocks: i32 = -1;
    let mut indepidx: i32 = -1;
    let mut bestobjval = crate::def::INVALID;
    let mut bestobjvalnouplocks = crate::def::INVALID;

    let mut nfixables = 0;

    // Check if we can apply the dual reduction; therefore count the number of
    // variables where the logic or has the only locks on.
    for v in (0..nvars).rev() {
        let mut var = cd(scip, cons).vars[v];

        // Variables with varstatus not equal to SCIP_VARSTATUS_FIXED can also have
        // fixed bounds, but were not removed yet.
        if scip.vars[var].glbdom.ub < 0.5 {
            let last = cd(scip, cons).nvars - 1;
            if idx == last {
                idx = v;
            }

            if idxnouplocks == last {
                idxnouplocks = v;
            }

            if indepidx == last {
                indepidx = v;
            }

            del_coef_pos(scip, cons, eventhdlr, v)?;
            *nchgcoefs += 1;

            continue;
        }

        if scip.vars[var].glbdom.lb > 0.5 {
            // Remove constraint since it is redundant.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;

            return Ok(false);
        }

        // Remember best variable with no uplocks, this variable dominates all
        // other with exactly one downlock.
        if locksdown(scip, var, M) > 1 && locksup(scip, var, M) == 0 {
            let mut objval = 0.0;
            crate::var::var_get_aggregated_obj(scip, var, &mut objval)?;

            // Check if the current variable has a smaller objective coefficient
            // then the best one (C compares against bestobjval here).
            if scip.set.is_lt(objval, bestobjval) {
                idxnouplocks = v;
                bestobjvalnouplocks = objval;
            }
        }

        // In case an other constraints has also locks on that variable we cannot
        // perform a dual reduction on these variables.
        if locksdown(scip, var, M) > 1 || locksup(scip, var, M) > 0 {
            continue;
        }

        nfixables += 1;
        let mut negated = false;

        // Get the active variable.
        crate::var::var_get_probvar_binary(scip, &mut var, &mut negated)?;

        let objval = if negated { -scip.vars[var].obj } else { scip.vars[var].obj };

        // Check if the current variable has a smaller objective coefficient.
        if scip.set.is_lt(objval, bestobjval) {
            idx = v;
            bestobjval = objval;
        }

        if objval >= 0.0 {
            indepidx = v;
        }
    }

    nvars = cd(scip, cons).nvars;
    let vars = |scip: &Scip, v: i32| cd(scip, cons).vars[v];

    // In the special case of two variables, where one variable is independent
    // and is minimized, we can aggregate variables: We have var1 + var2 >= 1
    // and var1 is independent with positive objective. Then var1 + var2 == 1
    // holds.
    if nvars == 2 && indepidx >= 0 {
        let idx2 = 1 - indepidx;

        let (a, b) = (vars(scip, indepidx), vars(scip, idx2));
        scip_var::aggregate_vars(scip, a, b, 1.0, 1.0, 1.0)?;
        *naggrvars += 1;

        // Remove constraint since it is now redundant.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;

        return Ok(true);
    }

    // Check if we have a single variable dominated by another.
    if nfixables == 1 && idxnouplocks >= 0 {
        for v in (0..nvars).rev() {
            let var = vars(scip, v);

            // Check if a variable only appearing in this constraint is dominated by
            // another.
            if locksdown(scip, var, M) == 1 && locksup(scip, var, M) == 0 {
                let mut objval = 0.0;
                crate::var::var_get_aggregated_obj(scip, var, &mut objval)?;

                if scip.set.is_ge(objval, bestobjvalnouplocks) && !scip.set.is_negative(objval) {
                    scip_var::fix_var(scip, var, 0.0)?;
                    *nfixedvars += 1;
                }

                break;
            }
        }
    }

    if nfixables < 2 {
        return Ok(false);
    }

    nvars = cd(scip, cons).nvars;

    // Fix all redundant variables to their best bound; first part of all
    // variables.
    for v in 0..nvars {
        let var = vars(scip, v);

        // In case an other constraints has also locks on that variable we cannot
        // perform a dual reduction on these variables.
        if locksdown(scip, var, M) > 1 || locksup(scip, var, M) > 0 {
            continue;
        }

        if v == idx {
            continue;
        }

        let mut activevar = var;
        let mut negated = false;

        // Get the active variable.
        crate::var::var_get_probvar_binary(scip, &mut activevar, &mut negated)?;

        let objval = if negated { -scip.vars[activevar].obj } else { scip.vars[activevar].obj };

        let fixval = if objval > 0.0 { 0.0 } else { 1.0 };

        scip_var::fix_var(scip, var, fixval)?;
        *nfixedvars += 1;
    }

    // If all variable have our appreciated number of locks and the constraint
    // is not modifiable, or if the bestobjval is less than or equal to zero,
    // we can fix the variable with the smallest objective coefficient to one
    // and the constraint gets redundant.
    if (nfixables == nvars && !scip.conss[cons].modifiable) || bestobjval <= 0.0 {
        let var = vars(scip, idx);
        scip_var::fix_var(scip, var, 1.0)?;
        *nfixedvars += 1;

        // Remove constraint since it is now redundant.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
    }

    Ok(true)
}

/// `applyFixings`: deletes all zero-fixed variables, checks for variables
/// fixed to one, replace all variables which are not active or not a
/// negation of an active variable by their active or negation of an active
/// counterpart. `counts` is `(naddconss, ndelconss)`, or `None` indicating
/// we can not resolve multi-aggregations. Returns whether a variable fixed to
/// one exists in the constraint (`redundant`).
fn apply_fixings(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, nchgcoefs: &mut i32, mut counts: Option<(&mut i32, &mut i32)>) -> ScipResult<bool> {
    let mut redundant = false;
    let mut v = 0;

    // All multi-aggregations should be resolved.
    cdm(scip, cons).existmultaggr = false;
    cdm(scip, cons).presolved = true;

    // Remove zeros and mark constraint redundant when found one variable fixed
    // to one.
    while v < cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];

        if scip.vars[var].glbdom.lb > 0.5 {
            return Ok(true);
        } else if scip.vars[var].glbdom.ub < 0.5 {
            del_coef_pos(scip, cons, eventhdlr, v)?;
            *nchgcoefs += 1;
        } else {
            v += 1;
        }
    }

    if cd(scip, cons).nvars == 0 {
        return Ok(false);
    }

    let nvars = cd(scip, cons).nvars;

    // Allocate temporary memory.
    let mut vars = vec![NONE; nvars as usize];
    let mut negarray = vec![false; nvars as usize];

    // Get active or negation of active variables.
    let consvars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
    scip_var::get_binvar_representatives(scip, &consvars, &mut vars, &mut negarray)?;

    // Renew all variables, important that we do a backwards loop because
    // deletion only affects rear items.
    'outer: for v in (0..nvars).rev() {
        let var = vars[v as usize];

        // Resolve multi-aggregation.
        let status = scip.vars[var].varstatus;
        if status == VarStatus::MultAggr || (status == VarStatus::Negated && scip.vars[scip.vars[var].negatedvar].varstatus == VarStatus::MultAggr) {
            let mut constant = 0.0;
            let mut requiredsize = 0;
            let mut size = 1;

            let mut nconsvars = 1;
            let mut consvars = CVec(vec![var]);
            let mut consvals = CVec(vec![1.0]);

            // Get active variables for new constraint.
            crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, size, &mut constant, &mut requiredsize)?;

            // If space was not enough we need to resize the buffers.
            if requiredsize > size {
                consvars.0.resize(requiredsize as usize, NONE);
                consvals.0.resize(requiredsize as usize, 0.0);
                size = requiredsize;

                crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, size, &mut constant, &mut requiredsize)?;
            }

            let mut easycase = false;

            if scip.set.is_zero(constant) {
                // Add active representation.
                let mut v2 = nconsvars - 1;
                while v2 >= 0 {
                    if !crate::var::var_is_binary(scip, consvars[v2]) {
                        break;
                    }

                    if !scip.set.is_eq(consvals[v2], 1.0) {
                        break;
                    }
                    v2 -= 1;
                }

                if v2 < 0 {
                    easycase = true;
                }
            }

            // We can easily add the coefficients and still have a logicor
            // constraint.
            if easycase {
                // Delete old (multi-aggregated) variable.
                del_coef_pos(scip, cons, eventhdlr, v)?;
                *nchgcoefs += 1;

                // Add active representation.
                for v2 in (0..nconsvars).rev() {
                    add_coef(scip, cons, consvars[v2])?;
                    *nchgcoefs += 1;
                }
            }
            // We need to degrade this logicor constraint to a linear constraint.
            else if counts.is_some() || scip.conss[cons].is_added() {
                // It might happen that there is more than one multi-aggregated
                // variable, so we need to get the whole probvar sum over all
                // variables.
                size = crate::def::max(nconsvars, 1) + nvars - 1;

                // Memory needed is at least old number of variables - 1 + number of
                // variables in first multi-aggregation.
                consvars.0.resize(size as usize, NONE);
                consvals.0.resize(size as usize, 0.0);

                nconsvars = nvars;

                // Add constraint variables to new linear variables.
                for k in (0..nvars).rev() {
                    consvars[k] = vars[k as usize];
                    consvals[k] = 1.0;
                }

                constant = 0.0;

                // Get active variables for new constraint.
                crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, size, &mut constant, &mut requiredsize)?;

                // If space was not enough (we found another multi-aggregation), we
                // need to resize the buffers.
                if requiredsize > size {
                    consvars.0.resize(requiredsize as usize, NONE);
                    consvals.0.resize(requiredsize as usize, 0.0);
                    size = requiredsize;

                    crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, size, &mut constant, &mut requiredsize)?;
                }

                let lhs = 1.0 - constant;
                let rhs = scip.set.infinity();

                // Create linear constraint.
                let name = scip.conss[cons].name.clone();
                let f = cons_flags(scip, cons);
                let n = nconsvars as usize;
                let mut newcons = crate::cons_linear::create_cons_linear(
                    scip,
                    &name,
                    &consvars.0[..n],
                    &consvals.0[..n],
                    lhs,
                    rhs,
                    f.initial,
                    f.separate,
                    f.enforce,
                    f.check,
                    f.propagate,
                    f.local,
                    f.modifiable,
                    f.dynamic,
                    f.removable,
                    f.stickingatnode,
                )?;

                // Add the downgraded constraint to the problem.
                scip_cons::add_cons(scip, newcons)?;
                scip_cons::release_cons(scip, &mut newcons)?;

                // Delete old constraint.
                scip_cons::del_cons(scip, cons)?;
                if let Some((naddconss, ndelconss)) = counts.as_mut() {
                    **ndelconss += 1;
                    **naddconss += 1;
                }

                break 'outer;
            }
            // We need to degrade this logicor constraint to a linear constraint.
            else {
                if var != cd(scip, cons).vars[v] {
                    del_coef_pos(scip, cons, eventhdlr, v)?;
                    add_coef(scip, cons, var)?;
                }

                log::warn!(
                    "logicor constraint <{}> has a multi-aggregated variable, which was not resolved and therefore could lead to aborts",
                    scip.conss[cons].name
                );
            }
        } else if var != cd(scip, cons).vars[v] {
            del_coef_pos(scip, cons, eventhdlr, v)?;

            // The binvar representative might be fixed:
            // - if fixed to 1, the constraint is redundant
            // - if fixed to 0, the representative does not need to be added to the
            //   constraint
            // - if not fixed, we add the representative to the constraint
            if scip.vars[var].glbdom.lb > 0.5 {
                redundant = true;
                break 'outer;
            } else if scip.vars[var].glbdom.ub < 0.5 {
                *nchgcoefs += 1;
            } else {
                add_coef(scip, cons, var)?;
            }
        }
    }

    cdm(scip, cons).presolved = true;

    Ok(redundant)
}

/// `analyzeConflict`: analyzes conflicting assignment on given constraint,
/// and adds conflict constraint to problem.
fn analyze_conflict(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // applicable.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis, and add all variables of infeasible
    // constraint to conflict candidate queue.
    api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

    for v in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];
        api::add_conflict_binvar(scip, var)?;
    }

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `disableCons`: disables or deletes the given constraint, depending on the
/// current depth.
fn disable_cons(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // In case the logic or constraint is satisfied in the depth where it is
    // also valid, we can delete it.
    if api::get_depth(scip) == scip.conss[cons].get_valid_depth() {
        scip_cons::del_cons(scip, cons)?;
    } else {
        cons_disable(scip, cons)?;
    }

    Ok(())
}

/// `mergeMultiples`: find pairs of negated variables in constraint:
/// constraint is redundant; find sets of equal variables in constraint:
/// multiple entries of variable can be replaced by single entry. `entries`
/// is indexed by problem index. Returns whether a variable and its negation
/// are present (`redundant`).
fn merge_multiples(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, entries: &mut [u8], nchgcoefs: &mut i32) -> ScipResult<bool> {
    let nvars = cd(scip, cons).nvars;
    let mut redundant = false;

    if cd(scip, cons).merged {
        return Ok(false);
    }

    if cd(scip, cons).nvars <= 1 {
        cdm(scip, cons).merged = true;
        return Ok(false);
    }

    // Allocate temporary memory.
    let mut negarray = vec![false; nvars as usize];

    let active = |scip: &Scip, var: VarId, negated: bool| if negated { scip.vars[var].negatedvar } else { var };

    // Initialize entries array.
    for v in (0..nvars).rev() {
        let x = cd(scip, cons).vars[v];
        negarray[v as usize] = crate::var::var_is_negated(scip, x);
        let var = active(scip, x, negarray[v as usize]);
        let pos = scip.vars[var].probindex;

        // Var is not active yet.
        entries[pos as usize] = 0;
    }

    // Check all vars for multiple entries, do necessary backwards loop because
    // deletion only affect rear items.
    for v in (0..nvars).rev() {
        let neg = negarray[v as usize];
        let var = active(scip, cd(scip, cons).vars[v], neg);
        let pos = scip.vars[var].probindex as usize;

        // If var occurs first time in constraint init entries array.
        if entries[pos] == 0 {
            entries[pos] = if neg { 2 } else { 1 };
        }
        // If var occurs second time in constraint, first time it was not negated.
        else if entries[pos] == 1 {
            if neg {
                redundant = true;
                break;
            } else {
                del_coef_pos(scip, cons, eventhdlr, v)?;
                *nchgcoefs += 1;
            }
        }
        // If var occurs second time in constraint, first time it was negated.
        else {
            if !neg {
                redundant = true;
                break;
            } else {
                del_coef_pos(scip, cons, eventhdlr, v)?;
                *nchgcoefs += 1;
            }
        }
    }

    cdm(scip, cons).merged = true;

    Ok(redundant)
}

/// `processWatchedVars`: checks constraint for violation only looking at the
/// watched variables, applies fixings if possible; returns `(addcut,
/// mustcheck)`: whether this constraint must be added as a cut, and whether
/// it must be checked for feasibility.
fn process_watched_vars(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, cutoff: &mut bool, reduceddom: &mut bool) -> ScipResult<(bool, bool)> {
    let mut addcut = false;
    let mut mustcheck = false;

    let nvars = cd(scip, cons).nvars;
    let vars = |scip: &Scip, v: i32| cd(scip, cons).vars[v];
    let lb = |scip: &Scip, v: i32| scip.vars[vars(scip, v)].locdom.lb;
    let ub = |scip: &Scip, v: i32| scip.vars[vars(scip, v)].locdom.ub;

    // Check watched variables if they are fixed to one.
    let (cw1, cw2) = (cd(scip, cons).watchedvar1, cd(scip, cons).watchedvar2);
    if cw1 >= 0 && lb(scip, cw1) > 0.5 {
        // The variable is fixed to one, making the constraint redundant ->
        // disable the constraint.
        disable_cons(scip, cons)?;
        return Ok((addcut, mustcheck));
    }
    if cw2 >= 0 && lb(scip, cw2) > 0.5 {
        // The variable is fixed to one, making the constraint redundant ->
        // disable the constraint.
        disable_cons(scip, cons)?;
        return Ok((addcut, mustcheck));
    }

    // Check if watched variables are still unfixed.
    let mut watchedvar1 = -1;
    let mut watchedvar2 = -1;
    let mut nbranchings1 = i64::MAX;
    let mut nbranchings2 = i64::MAX;
    if cw1 >= 0 && ub(scip, cw1) > 0.5 {
        watchedvar1 = cw1;
        nbranchings1 = -1; // prefer keeping the watched variable
    }
    if cw2 >= 0 && ub(scip, cw2) > 0.5 {
        if watchedvar1 == -1 {
            watchedvar1 = cw2;
            nbranchings1 = -1; // prefer keeping the watched variable
        } else {
            watchedvar2 = cw2;
            nbranchings2 = -1; // prefer keeping the watched variable
        }
    }

    // Search for new watched variables.
    if watchedvar2 == -1 {
        for v in 0..nvars {
            // Don't process the watched variables again.
            if v == cw1 || v == cw2 {
                continue;
            }

            // Check, if the variable is fixed.
            if ub(scip, v) < 0.5 {
                continue;
            }

            // Check, if the literal is satisfied.
            if lb(scip, v) > 0.5 {
                // The variable is fixed to one, making the constraint redundant; make
                // sure, the feasible variable is watched and disable the constraint.
                if cw1 != -1 {
                    switch_watchedvars(scip, cons, eventhdlr, cw1, v)?;
                } else {
                    switch_watchedvars(scip, cons, eventhdlr, v, cw2)?;
                }
                disable_cons(scip, cons)?;
                return Ok((addcut, mustcheck));
            }

            // The variable is unfixed and can be used as watched variable.
            let nbranchings = crate::var::var_get_n_branchings_current_run(scip, vars(scip, v), crate::types::BranchDir::Downwards);
            if nbranchings < nbranchings2 {
                if nbranchings < nbranchings1 {
                    watchedvar2 = watchedvar1;
                    nbranchings2 = nbranchings1;
                    watchedvar1 = v;
                    nbranchings1 = nbranchings;
                } else {
                    watchedvar2 = v;
                    nbranchings2 = nbranchings;
                }
            }
        }
    }

    if watchedvar1 == -1 {
        // There is no unfixed variable left -> the constraint is infeasible
        //  - a modifiable constraint must be added as a cut and further pricing
        //    must be performed in the LP solving loop
        //  - an unmodifiable constraint is infeasible and the node can be cut off
        api::reset_cons_age(scip, cons);
        if scip.conss[cons].modifiable {
            addcut = true;
        } else {
            // Use conflict analysis to get a conflict constraint out of the
            // conflicting assignment.
            analyze_conflict(scip, cons)?;

            // Mark the node to be cut off.
            *cutoff = true;
        }
    } else if watchedvar2 == -1 {
        // There is only one unfixed variable:
        // - a modifiable constraint must be checked manually
        // - an unmodifiable constraint is feasible and can be disabled after the
        //   remaining variable is fixed to one
        if scip.conss[cons].modifiable {
            mustcheck = true;
        } else {
            // Fixed remaining variable to one and disable constraint; make sure,
            // the fixed-to-one variable is watched.
            let var = vars(scip, watchedvar1);
            scip_var::infer_binvar_cons(scip, var, true, cons, 0)?;
            api::reset_cons_age(scip, cons);
            let cw1 = cd(scip, cons).watchedvar1;
            if watchedvar1 != cw1 {
                // Keep one of the watched variables.
                switch_watchedvars(scip, cons, eventhdlr, watchedvar1, cw1)?;
            }
            disable_cons(scip, cons)?;
            *reduceddom = true;
        }
    } else {
        // Switch to the new watched variables.
        switch_watchedvars(scip, cons, eventhdlr, watchedvar1, watchedvar2)?;

        // There are at least two unfixed variables -> the constraint must be
        // checked manually.
        mustcheck = true;

        // Disable propagation of constraint until a watched variable gets fixed.
        cons_disable_propagation(scip, cons);

        // Increase aging counter.
        let nvars = cd(scip, cons).nvars;
        api::add_cons_age(scip, cons, ageincrease(nvars as f64))?;
    }

    Ok((addcut, mustcheck))
}

/// `isConsViolated`: checks constraint for violation, returns TRUE iff
/// constraint is violated.
fn is_cons_violated(scip: &mut Scip, cons: ConsId, sol: SolId) -> bool {
    let nvars = cd(scip, cons).nvars;

    // Calculate the constraint's activity.
    let mut sum = 0.0;
    let mut v = 0;
    while v < nvars && sum < 1.0 {
        let solval = api::get_sol_val(scip, sol, cd(scip, cons).vars[v]);
        sum += solval;
        v += 1;
    }

    // Calculate constraint violation and update it in solution.
    if sol != NONE {
        let absviol = 1.0 - sum;
        let relviol = crate::misc::rel_diff(1.0, sum);
        api::update_sol_lp_cons_violation(scip, sol, absviol, relviol);
    }

    scip.set.is_feas_lt(sum, 1.0)
}

/// `createRow`: creates an LP row in a logic or constraint data object.
fn create_row(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let k = &scip.conss[cons];
    let (name, local, modifiable, removable) = (k.name.clone(), k.is_local(), k.modifiable, k.is_removable());
    let inf = scip.set.infinity();
    let row = scip_lp::create_empty_row_cons(scip, cons, &name, 1.0, inf, local, modifiable, removable)?;
    cdm(scip, cons).row = row;

    let n = cd(scip, cons).nvars as usize;
    let vars = cd(scip, cons).vars.0[..n].to_vec();
    scip_lp::add_vars_to_row_same_coef(scip, row, &vars, 1.0)
}

/// `addCut`: adds logic or constraint as cut to the LP; returns whether a
/// cutoff has been detected.
fn add_cut(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let mut cutoff = false;

    if cd(scip, cons).row == NONE {
        // Convert logic or constraint data into LP row.
        create_row(scip, cons)?;
    }

    // Insert LP row as cut.
    let row = cd(scip, cons).row;
    if scip.rows[row].lppos < 0 {
        cutoff = scip_lp::add_row(scip, row, false)?;
    }

    Ok(cutoff)
}

/// `separateCons`: checks constraint for violation, and adds it as a cut if
/// possible.
fn separate_cons(scip: &mut Scip, cons: ConsId, sol: SolId, eventhdlr: EventhdlrId, cutoff: &mut bool, separated: &mut bool, reduceddom: &mut bool) -> ScipResult {
    let mut addcut;
    let mustcheck;

    *cutoff = false;

    // Update and check the watched variables, if they were changed since last
    // processing.
    if sol == NONE && scip.conss[cons].is_propagation_enabled() {
        (addcut, mustcheck) = process_watched_vars(scip, cons, eventhdlr, cutoff, reduceddom)?;
    } else {
        addcut = false;
        mustcheck = true;
    }

    if mustcheck {
        // Variable's fixings didn't give us any information -> we have to check
        // the constraint.
        let row = cd(scip, cons).row;
        if sol == NONE && row != NONE {
            // Skip constraints already in the LP.
            if scip.rows[row].lppos >= 0 {
                return Ok(());
            } else {
                let feasibility = crate::lp::row_get_lp_feasibility(scip, row);
                addcut = scip.set.is_feas_negative(feasibility);
            }
        } else {
            addcut = is_cons_violated(scip, cons, sol);
        }
    }

    if addcut {
        // Insert LP row as cut.
        *cutoff = add_cut(scip, cons)?;
        api::reset_cons_age(scip, cons);
        *separated = true;
    }

    Ok(())
}

/// `enforcePseudo`: enforces the pseudo solution on the given constraint.
fn enforce_pseudo(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, cutoff: &mut bool, infeasible: &mut bool, reduceddom: &mut bool, solvelp: &mut bool) -> ScipResult {
    let addcut;
    let mustcheck;

    // Update and check the watched variables, if they were changed since last
    // processing.
    if scip.conss[cons].is_propagation_enabled() {
        (addcut, mustcheck) = process_watched_vars(scip, cons, eventhdlr, cutoff, reduceddom)?;
    } else {
        addcut = false;
        mustcheck = true;
    }

    if mustcheck {
        if is_cons_violated(scip, cons, NONE) {
            // Constraint was infeasible -> reset age.
            api::reset_cons_age(scip, cons);
            *infeasible = true;
        }
    } else if addcut {
        // A cut must be added to the LP -> we have to solve the LP immediately.
        api::reset_cons_age(scip, cons);
        *solvelp = true;
    }

    Ok(())
}

/// `consdataSort`: sorts logicor constraint's variables by non-decreasing
/// variable index.
fn consdata_sort(scip: &Scip, c: &mut ConsData) {
    if !c.sorted {
        if c.nvars <= 1 {
            c.sorted = true;
        } else {
            let mut var1 = NONE;
            let mut var2 = NONE;

            // Remember watch variables.
            if c.watchedvar1 != -1 {
                var1 = c.vars[c.watchedvar1];
                c.watchedvar1 = -1;
                if c.watchedvar2 != -1 {
                    var2 = c.vars[c.watchedvar2];
                    c.watchedvar2 = -1;
                }
            }

            // Sort variables after index.
            let n = c.nvars as usize;
            sort::sort(&mut c.vars.0[..n], (), |&a, &b| var_compare(scip, a, b));
            c.sorted = true;

            // Correct watched variables.
            let find = |vars: &[VarId], var: VarId| match sort::sortedvec_find(vars, |&a, &b| var_compare(scip, a, b), var) {
                Ok(p) | Err(p) => p as i32,
            };
            if var1 != NONE {
                c.watchedvar1 = find(&c.vars.0[..n], var1);
                if var2 != NONE {
                    c.watchedvar2 = find(&c.vars.0[..n], var2);
                }
            }
        }
    }
}

/// [`consdata_sort`] on the data of `cons`.
fn consdata_sort_cons(scip: &mut Scip, cons: ConsId) {
    let mut data = scip.conss[cons].consdata.take().unwrap();
    consdata_sort(scip, data.downcast_mut::<ConsData>().unwrap());
    scip.conss[cons].consdata = Some(data);
}

/// The logicor hash table callbacks (`hashGetKeyLogicorcons`,
/// `hashKeyEqLogicorcons`, `hashKeyValLogicorcons`). C's key comparison
/// sorts both constraints first; the stored constraints were sorted before
/// insertion and the callers sort the queried one beforehand with
/// [`consdata_sort_cons`].
struct LogicorconsHash<'a> {
    scip: &'a Scip,
}

impl crate::hash::HashOps<ConsId> for LogicorconsHash<'_> {
    type Key = ConsId;

    // The key is the element itself.
    fn get_key(&self, element: &ConsId) -> ConsId {
        *element
    }

    // Returns TRUE iff both keys are equal; two constraints are equal if they
    // have the same variables.
    fn key_eq(&self, key1: &ConsId, key2: &ConsId) -> bool {
        let consdata1 = cd(self.scip, *key1);
        let consdata2 = cd(self.scip, *key2);

        // Checks trivial case.
        if consdata1.nvars != consdata2.nvars {
            return false;
        }

        let mut coefsequal = true;

        for i in 0..consdata1.nvars {
            // Tests if variables are equal.
            if consdata1.vars[i] != consdata2.vars[i] {
                coefsequal = false;
                break;
            }
        }

        coefsequal
    }

    // Returns the hash value of the key.
    fn key_val(&self, key: &ConsId) -> u64 {
        let c = cd(self.scip, *key);
        let minidx = self.scip.vars[c.vars[0usize]].index;
        let mididx = self.scip.vars[c.vars[c.nvars / 2]].index;
        let maxidx = self.scip.vars[c.vars[c.nvars - 1]].index;

        crate::misc::hash_four(c.nvars as u32, minidx as u32, mididx as u32, maxidx as u32) as u64
    }
}

/// `detectRedundantConstraints`: compares each constraint with all other
/// constraints for a possible duplication and removes duplicates using a
/// hash table; also see `removeRedundantConssAndNonzeros()`.
fn detect_redundant_constraints(scip: &mut Scip, conss: &[ConsId], firstchange: &mut i32, ndelconss: &mut i32) -> ScipResult {
    // Create a hash table for the constraint set.
    let mut hashtablesize = conss.len() as i32;
    hashtablesize = crate::def::max(hashtablesize, HASHSIZE_LOGICORCONS);
    let mut hashtable = crate::hash::HashTable::<ConsId>::new(hashtablesize);

    // Check all constraints in the given set for redundancy.
    for &cons0 in conss {
        if !scip.conss[cons0].is_active() || scip.conss[cons0].modifiable {
            continue;
        }

        // Sort the constraint.
        consdata_sort_cons(scip, cons0);

        // Get constraint from current hash table with same variables as cons0.
        let cons1 = hashtable.retrieve(&LogicorconsHash { scip }, &cons0);

        if let Some(cons1) = cons1 {
            // Update flags of constraint which caused the redundancy s.t.
            // nonredundant information doesn't get lost.
            scip_cons::update_cons_flags(scip, cons1, cons0)?;

            // Delete consdel.
            let changed0 = cd(scip, cons0).changed;
            scip_cons::del_cons(scip, cons0)?;
            *ndelconss += 1;

            // Update the first changed constraint to begin the next aggregation
            // round with.
            if changed0 && scip.conss[cons1].consspos < *firstchange {
                *firstchange = scip.conss[cons1].consspos;
            }
        } else {
            // No such constraint in current hash table: insert cons0 into hash
            // table.
            hashtable.insert(&LogicorconsHash { scip }, cons0);
        }
    }

    Ok(())
}

/// `removeRedundantCons`: removes the redundant second constraint and
/// updates the flags of the first one.
fn remove_redundant_cons(scip: &mut Scip, cons0: ConsId, cons1: ConsId, ndelconss: &mut i32) -> ScipResult {
    // Update flags of cons0.
    scip_cons::update_cons_flags(scip, cons0, cons1)?;

    // Delete cons1.
    scip_cons::del_cons(scip, cons1)?;
    *ndelconss += 1;

    Ok(())
}

/// `calcSignature`: compute and return a signature for given variables.
fn calc_signature(scip: &Scip, vars: &[VarId]) -> u32 {
    let mut signature: u32 = 0;
    for &var in vars.iter().rev() {
        signature |= 1u32 << ((scip.vars[var].index as u32) % 32);
    }
    signature
}

/// `consdataCalcSignature`: compute the constraint signature which is used
/// to detect constraints, that contain potentially the same set of
/// variables.
fn consdata_calc_signature(scip: &mut Scip, cons: ConsId) {
    if cd(scip, cons).validsignature {
        return;
    }

    let c = cd(scip, cons);
    let signature = calc_signature(scip, &c.vars.0[..c.nvars as usize]);
    let c = cdm(scip, cons);
    c.signature = signature;
    c.validsignature = true;
}

/// The column representation of the logicor constraints of
/// [`remove_redundant_conss_and_nonzeros`]: `varstopos` maps a variable to
/// its column plus one; each column keeps C's buffer (`occurlist[pos]`, with
/// stale entries past `noccurlistentries[pos]`).
struct OccurList {
    varstopos: crate::hash::HashMap<i32>,
    occurlist: Vec<Vec<ConsId>>,
    noccurlistentries: Vec<i32>,
    occurlistsizes: Vec<i32>,
    occurlistlength: i32,
}

impl OccurList {
    /// The column of `var` (C's image minus one).
    fn pos(&self, var: VarId) -> usize {
        (self.varstopos.get_image(var as u64).unwrap() - 1) as usize
    }

    /// Removes one entry `cons` from column `pos`.
    fn remove_entry(&mut self, pos: usize, cons: ConsId) {
        let mut l = self.noccurlistentries[pos] - 1;
        while l >= 0 {
            if self.occurlist[pos][l as usize] == cons {
                self.noccurlistentries[pos] -= 1;
                let last = self.noccurlistentries[pos] as usize;
                self.occurlist[pos][l as usize] = self.occurlist[pos][last];
                break;
            }
            l -= 1;
        }
    }
}

/// `removeConsFromOccurList`: remove a constraint from the column
/// representation.
fn remove_cons_from_occur_list(scip: &Scip, cons: ConsId, ol: &mut OccurList) {
    let c = cd(scip, cons);

    // Remove constraint from list.
    for v in (0..c.nvars).rev() {
        let pos = ol.pos(c.vars[v]);

        // Remove for each variable one corresponding entry.
        ol.remove_entry(pos, cons);
    }
}

/// `findShortestOccurlist`: determine shortest constraint list in column
/// representation; returns the number of its entries and its column (`None`
/// when some variable is not in the column representation, with zero
/// entries).
fn find_shortest_occurlist(vars: &[VarId], ol: &OccurList) -> (i32, Option<usize>) {
    let mut nentries = i32::MAX;
    let mut shortestlist = None;

    // Find the shortest list.
    for &var in vars.iter().rev() {
        // It might be that a variable is not yet put into the occurlist, then this
        // constraint cannot cover another.
        if !ol.varstopos.exists(var as u64) {
            return (0, None);
        }

        let pos = ol.pos(var);

        // Remember the shortest list.
        if ol.noccurlistentries[pos] < nentries {
            nentries = ol.noccurlistentries[pos];
            shortestlist = Some(pos);
        }
    }

    (nentries, shortestlist)
}

/// `removeRedundantConss`: run a pairwise comparison for detecting
/// subset-constraints of other constraint while using a signature.
fn remove_redundant_conss(scip: &mut Scip, cons: ConsId, ol: &mut OccurList, ndelconss: &mut i32) -> ScipResult {
    let nvars = cd(scip, cons).nvars;
    let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();

    // Determine shortest column.
    let (nentries, shortestlist) = find_shortest_occurlist(&vars, ol);

    // One variable which does not appear in the column representation
    // anymore.
    if nentries == 0 {
        return Ok(());
    }
    let shortestlist = shortestlist.unwrap();

    // Check all constraints in the shortest list for coverage.
    for c in (0..nentries).rev() {
        let cons1 = ol.occurlist[shortestlist][c as usize];

        if cons != cons1 {
            let consdata = cd(scip, cons);
            let consdata1 = cd(scip, cons1);

            // Constraints with the same length cannot be covered and same
            // constraints are removed in detectRedundantConstraints().
            if consdata1.nvars == consdata.nvars {
                continue;
            }

            if (consdata.signature & !consdata1.signature) == 0 {
                let mut v = 0;
                let mut v1 = 0;

                while v < nvars && v1 < consdata1.nvars {
                    let var = vars[v as usize];
                    let var1 = consdata1.vars[v1];

                    let comp = var_compare(scip, var, var1);

                    if comp == 0 {
                        v += 1;
                        v1 += 1;
                    } else if comp > 0 {
                        v1 += 1;
                    } else {
                        break;
                    }
                }

                // cons1 is covered by cons.
                if v == nvars {
                    // Remove cons1 from columns representation.
                    remove_cons_from_occur_list(scip, cons1, ol);

                    // Delete redundant constraint and update constraint flags if
                    // necessary.
                    remove_redundant_cons(scip, cons, cons1, ndelconss)?;
                }
            }
        }
    }

    Ok(())
}

/// `conssLogicorComp`: compararer for sorting constraints after their
/// number of variables.
fn conss_logicor_comp(scip: &Scip, cons1: ConsId, cons2: ConsId) -> i32 {
    cd(scip, cons1).nvars - cd(scip, cons2).nvars
}

/// `addConsToOccurList`: add a constraint to the column representation.
fn add_cons_to_occur_list(scip: &Scip, cons: ConsId, ol: &mut OccurList) {
    use crate::var::var_get_n_locks_down_type as locksdown;

    let c = cd(scip, cons);

    for v in (0..c.nvars).rev() {
        let var = c.vars[v];

        // Check if the variable is not yet put into the occurlist.
        if !ol.varstopos.exists(var as u64) {
            let pos = ol.occurlistlength as usize;

            // Allocate memory.
            ol.occurlistsizes[pos] = locksdown(scip, var, LockType::Model) + 1;
            ol.occurlist[pos] = vec![NONE; ol.occurlistsizes[pos] as usize];

            // Put constraint in list of current variable.
            let n = ol.noccurlistentries[pos] as usize;
            ol.occurlist[pos][n] = cons;
            ol.noccurlistentries[pos] += 1;

            // Add new variable to map.
            let _ = ol.varstopos.insert(var as u64, pos as i32 + 1);
            ol.occurlistlength += 1;
        } else {
            let pos = ol.pos(var);

            // Do we need to resize the array.
            if ol.noccurlistentries[pos] == ol.occurlistsizes[pos] {
                ol.occurlistsizes[pos] = scip.set.calc_mem_grow_size(ol.occurlistsizes[pos] + 1);

                // Resize occurlist for current variable.
                ol.occurlist[pos].resize(ol.occurlistsizes[pos] as usize, NONE);
            }

            // Put constraint in list of current variable.
            let n = ol.noccurlistentries[pos] as usize;
            ol.occurlist[pos][n] = cons;
            ol.noccurlistentries[pos] += 1;
        }
    }
}

/// `removeRedundantNonZeros`: run a pairwise comparison for the given
/// variables against all constraits to detect redundant non-zeros in these
/// constraints; returns whether cons will be deleted.
#[allow(clippy::too_many_arguments)]
fn remove_redundant_non_zeros(scip: &mut Scip, cons: ConsId, artvar: VarId, artpos: i32, ol: &mut OccurList, eventhdlr: EventhdlrId, nchgcoefs: &mut i32) -> ScipResult<bool> {
    let nvars = cd(scip, cons).nvars;
    let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();

    // Temporary exchange the variable for finding the shortest list.
    let mut exchanged = vars.clone();
    exchanged[artpos as usize] = artvar;

    // Determine shortest column.
    let (nentries, shortestlist) = find_shortest_occurlist(&exchanged, ol);

    // One variable which does not appear in the column representation
    // anymore.
    if nentries == 0 {
        return Ok(false);
    }
    let shortestlist = shortestlist.unwrap();

    // Temporary exchange the variable for calculating a valid signature.
    let signature = calc_signature(scip, &exchanged);

    // Check all constraints in the shortest list for coverage.
    for c in (0..nentries).rev() {
        let cons1 = ol.occurlist[shortestlist][c as usize];

        if !scip.conss[cons1].is_active() {
            continue;
        }

        if cons != cons1 {
            // Constraints with the less variables cannot be covered.
            if cd(scip, cons1).nvars < nvars {
                continue;
            }

            let mut pos: i32 = -1;

            if (signature & !cd(scip, cons1).signature) == 0 {
                let consdata1 = cd(scip, cons1);
                let mut v = 0;
                let mut v1 = 0;

                while v < nvars && v1 < consdata1.nvars {
                    // Skip position of artificial variable.
                    if artpos == v {
                        v += 1;
                        continue;
                    }

                    let var1 = consdata1.vars[v1];

                    // Did we find the artificial variable in cons1.
                    if artvar == var1 {
                        // Remember of possible redundant variable.
                        pos = v1;
                        v1 += 1;
                        continue;
                    }

                    let var = vars[v as usize];
                    let comp = var_compare(scip, var, var1);

                    // Check if the cons1 can still be covered.
                    if comp == 0 {
                        v += 1;
                        v1 += 1;
                    } else if comp > 0 {
                        v1 += 1;
                    } else {
                        break;
                    }
                }

                // cons1 is might be covered by the changed constraints cons, meaning
                // that we might remove the artvar from cons1.
                if v == nvars {
                    let mut deleted = false;

                    // If the artificial variable was not yet found, search over the rear
                    // variables in constraint cons1.
                    if pos == -1 {
                        while v1 < consdata1.nvars {
                            if artvar == consdata1.vars[v1] {
                                // Remember of possible redundant variable.
                                pos = v1;
                                break;
                            }
                            v1 += 1;
                        }
                    }

                    if pos >= 0 {
                        // Remove redudant entry in cons1.
                        let conspos = pos;

                        if consdata1.nvars > nvars {
                            let p = ol.pos(artvar);

                            // Remove corresponding entry in column representation.
                            ol.remove_entry(p, cons1);
                        } else {
                            // Delete cons.

                            // Remove cons from columns representation.
                            remove_cons_from_occur_list(scip, cons, ol);

                            // Update flags of cons1.
                            scip_cons::update_cons_flags(scip, cons1, cons)?;

                            scip_cons::del_cons(scip, cons)?;
                            deleted = true;
                        }

                        // Remove variable.
                        del_coef_pos(scip, cons1, eventhdlr, conspos)?;
                        *nchgcoefs += 1;
                        consdata_sort_cons(scip, cons1);
                        consdata_calc_signature(scip, cons1);

                        if deleted {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

/// `strengthenConss`: find and remove redundant non-zero entries.
fn strengthen_conss(scip: &mut Scip, conss: &[ConsId], ol: &mut OccurList, eventhdlr: EventhdlrId, ndelconss: &mut i32, nchgcoefs: &mut i32) -> ScipResult {
    if conss.is_empty() {
        return Ok(());
    }

    for &cons in conss {
        if !scip.conss[cons].is_active() {
            continue;
        }

        let nvars = cd(scip, cons).nvars;

        if nvars == 1 {
            continue;
        }

        for v in (0..nvars).rev() {
            let artvar = scip.vars[cd(scip, cons).vars[v]].negatedvar;

            if artvar != NONE && ol.varstopos.exists(artvar as u64) {
                // Detect and remove redundant non-zero entries; @todo: improve this
                // algorithm by using the information that a constraint variables does
                // not appaer in any other constraint, which means that only this
                // variable needs to be negated to check for redundant non-zeros,
                // therefor change also findShortestOccurlist() to return the
                // corresponding variable/position.
                let deleted = remove_redundant_non_zeros(scip, cons, artvar, v, ol, eventhdlr, nchgcoefs)?;

                if deleted {
                    *ndelconss += 1;
                    break;
                }
            }
        }
    }

    Ok(())
}

/// `prepareCons`: prepares a constraint by removing fixings and merge it;
/// returns whether a variable fixed to one exists in the constraint
/// (`redundant`).
#[allow(clippy::too_many_arguments)]
fn prepare_cons(
    scip: &mut Scip,
    cons: ConsId,
    eventhdlr: EventhdlrId,
    entries: &mut [u8],
    nfixedvars: &mut i32,
    nchgcoefs: &mut i32,
    ndelconss: &mut i32,
    cutoff: &mut bool,
) -> ScipResult<bool> {
    let mut redundant = false;

    // Remove old fixings.
    if !cd(scip, cons).presolved {
        // Remove all variables that are fixed to zero, check redundancy due to
        // fixed-to-one variable.
        redundant = apply_fixings(scip, cons, eventhdlr, nchgcoefs, None)?;
    }

    if !redundant {
        // Merge constraint.
        redundant = merge_multiples(scip, cons, eventhdlr, entries, nchgcoefs)?;
    }

    if redundant {
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;

        return Ok(true);
    }

    if cd(scip, cons).nvars == 0 {
        *cutoff = true;
    } else if cd(scip, cons).nvars == 1 {
        let var = cd(scip, cons).vars[0usize];
        scip_var::fix_var(scip, var, 1.0)?;
        *nfixedvars += 1;

        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        redundant = true;
    }
    cdm(scip, cons).presolved = true;

    Ok(redundant)
}

/// `removeRedundantConssAndNonzeros`: find covered/subsumed constraints and
/// redundant non-zero entries
///
/// covered:
/// e.g.: c1: x1 + x2 + x3 >= 1
///       c2: x1 + x2 + x3 + x4 >= 1
///
/// strengthen:
/// e.g.: c1: x1 + x2 + x3 >= 1
///       c2: x1 + x2 + ~x3 + x4 >= 1
///
///    => c2: x1 + x2 + x4 >= 1
///
/// See "Effective Preprocessing in SAT through Variable and Clause
/// Elimination" by Niklas En and Armin Biere.
#[allow(clippy::too_many_arguments)]
fn remove_redundant_conss_and_nonzeros(
    scip: &mut Scip,
    conss: &[ConsId],
    entries: &mut [u8],
    eventhdlr: EventhdlrId,
    usestrengthening: bool,
    firstchange: &mut i32,
    nfixedvars: &mut i32,
    ndelconss: &mut i32,
    nchgcoefs: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    let nconss = conss.len() as i32;

    if *firstchange > nconss || nconss < 2 {
        return Ok(());
    }

    // Copy constraints to re-order them; C's loop below runs over all nconss
    // slots, including the ones past nmyconss.
    let mut myconss = conss.to_vec();
    let mut nmyconss = nconss;

    let mut lastnfixedvars = -1;
    while *nfixedvars != lastnfixedvars {
        lastnfixedvars = *nfixedvars;
        for c in (0..nconss as usize).rev() {
            let cons = myconss[c];

            if scip.conss[cons].deleted || scip.conss[cons].modifiable {
                myconss[c] = myconss[(nmyconss - 1) as usize];
                nmyconss -= 1;

                continue;
            }

            // Prepare constraint by removing fixings and merge it.
            let redundant = prepare_cons(scip, cons, eventhdlr, entries, nfixedvars, nchgcoefs, ndelconss, cutoff)?;

            if redundant {
                myconss[c] = myconss[(nmyconss - 1) as usize];
                nmyconss -= 1;

                continue;
            }

            if *cutoff {
                return Ok(());
            }

            // Sort the constraint.
            consdata_sort_cons(scip, cons);
        }
    }

    // C reads myconss[0] and myconss[nmyconss - 1] below even when no
    // constraint is left.
    if nmyconss <= 0 {
        return Ok(());
    }

    let n = nmyconss as usize;
    sort::sort(&mut myconss[..n], (), |&a, &b| conss_logicor_comp(scip, a, b));

    // We can stop if strengthening is disabled and all constraints have the
    // same amount of variables.
    if !usestrengthening && cd(scip, myconss[0]).nvars == cd(scip, myconss[n - 1]).nvars {
        return Ok(());
    }

    // @note: in the following we have at least number of nonzeros in logicor
    // constraints + three times two the number of binary variables memory
    // consumption + a map for variables to positions, we need this to get a
    // column base representation.

    // Get number of all possible(incl. implicit) binary variables and their
    // negation.
    let nbinvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip);
    let occurlistsize = (2 * nbinvars) as usize;

    // Allocate memory for the column representation for each variable; create
    // hashmap to map all occuring variables to a position in the list.
    let mut ol = OccurList {
        varstopos: crate::hash::HashMap::new(nmyconss),
        occurlist: vec![Vec::new(); occurlistsize],
        noccurlistentries: vec![0; occurlistsize],
        occurlistsizes: vec![0; occurlistsize],
        occurlistlength: 0,
    };

    // Get maximal number of variables over all logicor constraints.
    let mut c = nmyconss - 1;
    let mut cons = myconss[c as usize];
    let nmaxvars = cd(scip, cons).nvars;

    let mut conschanged = false;

    // Determine all constraints with the maximal number of variables and add
    // them to the column representation.
    loop {
        // Calculate hash-signature.
        consdata_calc_signature(scip, cons);
        conschanged = conschanged || cd(scip, cons).changed;
        cdm(scip, cons).changed = false;

        // Add constraint to column data structure.
        add_cons_to_occur_list(scip, cons, &mut ol);

        c -= 1;
        if c < 0 {
            break;
        }

        cons = myconss[c as usize];
        if cd(scip, cons).nvars != nmaxvars {
            break;
        }
    }

    // Remove covered constraints and left over constraints to the column
    // representation.
    while c >= 0 {
        let cons = myconss[c as usize];

        // Calculate hash-signature.
        consdata_calc_signature(scip, cons);

        // Search for covered constraints.
        if conschanged || cd(scip, cons).changed {
            // Detect covered constraints
            //
            // e.g.: c1: x1 + x2 + x3 >= 1
            //       c2: x1 + x2 + x3 + x4 >= 1
            //
            //    => delete c2
            remove_redundant_conss(scip, cons, &mut ol, ndelconss)?;
            cdm(scip, cons).changed = false;
            conschanged = true;
        }

        // Add constraint to column data structure.
        add_cons_to_occur_list(scip, cons, &mut ol);

        c -= 1;
    }

    // Strengthen constraint while removing non-zeros
    //
    // e.g.: c1: x1 + x2 + x3 >= 1
    //       c2: x1 + x2 + ~x3 + x4 >= 1
    //
    //    => c2: x1 + x2 + x4 >= 1
    //
    // special case:
    //
    // e.g.: c1: x1 + x2 + x3 >= 1
    //       c2: x1 + x2 + ~x3 >= 1
    //
    //    => delete c1; c2: x1 + x2 >= 1
    strengthen_conss(scip, &myconss[..n], &mut ol, eventhdlr, ndelconss, nchgcoefs)?;

    Ok(())
}

const MAX_CONSLENGTH: i32 = 200;

/// `shortenConss`: try to tighten constraints by reducing the number of
/// variables in the constraints using implications and cliques, also derive
/// fixations through them, see `SCIPshrinkDisjunctiveVarSet()`.
#[allow(clippy::too_many_arguments)]
fn shorten_conss(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    eventhdlr: EventhdlrId,
    conss: &[ConsId],
    entries: &mut [u8],
    nfixedvars: &mut i32,
    ndelconss: &mut i32,
    nchgcoefs: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    if conss.is_empty() {
        return Ok(());
    }

    let ncliques = scip_var::get_n_cliques(scip);
    let nimpls = api::get_n_implications(scip);
    if ncliques == hd(scip, conshdlr).nlastcliquesshorten && nimpls == hd(scip, conshdlr).nlastimplsshorten {
        return Ok(());
    }

    hdm(scip, conshdlr).nlastcliquesshorten = ncliques;
    hdm(scip, conshdlr).nlastimplsshorten = nimpls;

    let nbinprobvars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip)) as usize;

    // Allocate temporary memory.
    let mut probvars = vec![NONE; nbinprobvars];
    let mut bounds = vec![0.0; nbinprobvars];
    let mut boundtypes = vec![false; nbinprobvars];
    let mut redundants = vec![false; nbinprobvars];

    for &cons in conss.iter().rev() {
        let mut redundant;

        if scip.conss[cons].deleted {
            continue;
        }

        // Prepare constraint by removing fixings and merge it; we invalidate the
        // presolved flag, because the calls to SCIPcleanupCliques() in this loop
        // may have lead to fixed variables that are not yet removed.
        cdm(scip, cons).presolved = false;
        redundant = prepare_cons(scip, cons, eventhdlr, entries, nfixedvars, nchgcoefs, ndelconss, cutoff)?;

        if redundant {
            continue;
        }

        if *cutoff {
            return Ok(());
        }

        // Do not try to shorten too long constraints.
        let nvars = cd(scip, cons).nvars;
        if nvars > MAX_CONSLENGTH {
            continue;
        }

        // Form necessary data.
        for v in (0..nvars).rev() {
            let var = cd(scip, cons).vars[v];
            if crate::var::var_is_active(scip, var) {
                probvars[v as usize] = var;
                bounds[v as usize] = 1.0;
                boundtypes[v as usize] = false;
            } else {
                probvars[v as usize] = scip.vars[var].negatedvar;
                bounds[v as usize] = 0.0;
                boundtypes[v as usize] = true;
            }
        }

        *cutoff = scip_var::cleanup_cliques(scip)?;

        if *cutoff {
            return Ok(());
        }

        // Use implications and cliques to derive global fixings and to shrink the
        // number of variables in this constraints.
        let mut nredvars = 0;
        redundant = false;
        crate::presolve::shrink_disjunctive_var_set(
            scip,
            &probvars,
            &bounds,
            &boundtypes,
            &mut redundants,
            nvars,
            &mut nredvars,
            nfixedvars,
            &mut redundant,
            cutoff,
            true,
        )?;

        if *cutoff {
            return Ok(());
        }

        // Remove redundant constraint.
        if redundant {
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;

            // Reset redundants array to FALSE.
            let n = cd(scip, cons).nvars as usize;
            redundants[..n].fill(false);
            continue;
        }

        // Remove redundant variables.
        if nredvars > 0 {
            for v in (0..cd(scip, cons).nvars).rev() {
                if redundants[v as usize] {
                    del_coef_pos(scip, cons, eventhdlr, v)?;

                    // Reset entry to FALSE.
                    redundants[v as usize] = false;
                }
            }
            *nchgcoefs += nredvars;

            // If only one variable is left over fix it.
            if cd(scip, cons).nvars == 1 {
                let var = cd(scip, cons).vars[0usize];
                scip_var::fix_var(scip, var, 1.0)?;
                *nfixedvars += 1;

                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            }
            // @todo might also upgrade a two variable constraint to a set-packing
            // constraint.
        }
    }

    // Invalidate the presolved flags, because the calls to
    // SCIPcleanupCliques() may have lead to fixed variables that are not yet
    // removed.
    for &cons in conss.iter().rev() {
        cdm(scip, cons).presolved = false;
    }

    Ok(())
}

const MAXCOMPARISONS: i32 = 1000000;

/// `removeConstraintsDueToNegCliques`: try to find a negated clique in a
/// constraint which makes this constraint redundant but we need to keep the
/// negated clique information alive, so we create a corresponding
/// set-packing constraint.
#[allow(clippy::too_many_arguments)]
fn remove_constraints_due_to_neg_cliques(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conshdlrsetppc: ConshdlrId,
    eventhdlr: EventhdlrId,
    conss: &[ConsId],
    entries: &mut [u8],
    nfixedvars: &mut i32,
    ndelconss: &mut i32,
    nupgdconss: &mut i32,
    nchgcoefs: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    use crate::var::{var_get_n_cliques, var_get_n_impls, var_is_active};

    let nconss = conss.len() as i32;

    if nconss == 0 {
        return Ok(());
    }

    let ncliques = scip_var::get_n_cliques(scip);
    let nimpls = api::get_n_implications(scip);
    if ncliques == hd(scip, conshdlr).nlastcliquesneg && nimpls == hd(scip, conshdlr).nlastimplsneg {
        return Ok(());
    }

    hdm(scip, conshdlr).nlastcliquesneg = ncliques;
    hdm(scip, conshdlr).nlastimplsneg = nimpls;

    // Estimate the maximal number of variables in a logicor constraint.
    let size = api::get_n_vars(scip) - api::get_n_cont_vars(scip);
    if size <= 0 {
        return Ok(());
    }

    // Temporary memory for active/negation of active variables.
    let mut negated = vec![false; size as usize];

    // Iterate over all constraints and try to find negated cliques in
    // logicors.
    for c in (0..nconss).rev() {
        let cons = conss[c as usize];

        if !scip.conss[cons].is_active() {
            continue;
        }

        // Prepare constraint by removing fixings and merge it.
        let redundant = prepare_cons(scip, cons, eventhdlr, entries, nfixedvars, nchgcoefs, ndelconss, cutoff)?;

        if redundant {
            continue;
        }

        if *cutoff {
            return Ok(());
        }

        if scip.conss[cons].modifiable && cd(scip, cons).nvars == 2 {
            continue;
        }

        if c % 100 == 0 && crate::solve::solve_is_stopped(scip, false) {
            break;
        }

        let maxcomppercons = MAXCOMPARISONS / nconss;
        let mut comppercons = 0;

        let nvars0 = cd(scip, cons).nvars;
        let repvars = cd(scip, cons).vars.0[..nvars0 as usize].to_vec();

        // All variables should be active or negative active variables, otherwise
        // something went wrong with applyFixings() called before
        // mergeMultiples().
        for v in (0..nvars0).rev() {
            negated[v as usize] = crate::var::var_is_negated(scip, repvars[v as usize]);
        }

        let mut v = nvars0 - 1;
        while v > 0 {
            let mut var1 = repvars[v as usize];

            // If there is no negated variable, there can't be a negated clique.
            if scip.vars[var1].negatedvar == NONE {
                v -= 1;
                continue;
            }

            // Get active counterpart to check for common cliques.
            let neg1;
            if scip.vars[var1].varstatus == VarStatus::Negated {
                var1 = scip.vars[var1].negatedvar;
                neg1 = true;
            } else {
                neg1 = false;
            }

            if !var_is_active(scip, var1) {
                v -= 1;
                continue;
            }

            // No cliques available.
            if var_get_n_cliques(scip, var1, neg1) == 0 && var_get_n_impls(scip, var1, neg1) == 0 {
                v -= 1;
                continue;
            }

            comppercons += v - 1;

            let mut breakloop = false;

            for w in (0..v).rev() {
                let mut var2 = repvars[w as usize];

                // If there is no negated variable, there can't be a negated clique.
                if scip.vars[var2].negatedvar == NONE {
                    continue;
                }

                let neg2;
                if scip.vars[var2].varstatus == VarStatus::Negated {
                    var2 = scip.vars[var2].negatedvar;
                    neg2 = true;
                } else {
                    neg2 = false;
                }

                if !var_is_active(scip, var2) {
                    continue;
                }

                // No cliques available.
                if var_get_n_cliques(scip, var2, neg2) == 0 && var_get_n_impls(scip, var2, neg2) == 0 {
                    continue;
                }

                // Check if both active variable are the same.
                if var1 == var2 {
                    if neg1 != neg2 {
                        scip_cons::del_cons(scip, cons)?;

                        breakloop = true;
                    } else {
                        del_coef_pos(scip, cons, eventhdlr, v)?;

                        let nvars = cd(scip, cons).nvars;
                        if v < nvars {
                            // delCoefPos replaces the variable on position v with the last
                            // one, so w also need to correct the negated array the same way,
                            // and because of deletion the number of variables is already
                            // decreased.
                            negated[v as usize] = negated[nvars as usize];
                        }
                        *nchgcoefs += 1;
                    }
                    break;
                }

                if conshdlrsetppc != NONE && scip.conss[cons].get_n_upgrade_locks() == 0 && crate::var::vars_have_common_clique(scip, var1, neg1, var2, neg2, true) {
                    // This negated clique information could be created out of this
                    // logicor constraint even if there are more than two variables left
                    // (, for example by probing), we need to keep this information by
                    // creating a setppc constraint instead.

                    // Get correct variables.
                    let v0 = if !neg1 { scip.vars[var1].negatedvar } else { var1 };
                    let v1 = if !neg2 { scip.vars[var2].negatedvar } else { var2 };

                    let name = scip.conss[cons].name.clone();
                    let flags = cons_flags(scip, cons);
                    let mut newcons = crate::cons_setppc::create_cons_setpack(scip, &name, &[v0, v1], &flags)?;

                    scip_cons::add_cons_upgrade(scip, cons, &mut newcons)?;

                    scip_cons::del_cons(scip, cons)?;
                    *nupgdconss += 1;
                    breakloop = true;
                    break;
                }
            }
            if breakloop {
                break;
            }

            // Do not do to many comparisons.
            if comppercons > maxcomppercons {
                break;
            }
            v -= 1;
        }
    }

    Ok(())
}

/// `fixDeleteOrUpgradeCons`: handle all cases with less than three
/// variables in a logicor constraint
///
/// in case a constraint has zero variables left, we detected infeasibility
/// in case a constraint has one variables left, we will fix it to one
/// in case a constraint has two variables left, we will add the implication
/// and upgrade it to a set-packing constraint
#[allow(clippy::too_many_arguments)]
fn fix_delete_or_upgrade_cons(
    scip: &mut Scip,
    cons: ConsId,
    eventhdlr: EventhdlrId,
    conshdlrlinear: ConshdlrId,
    conshdlrsetppc: ConshdlrId,
    nfixedvars: &mut i32,
    nchgbds: &mut i32,
    nchgcoefs: &mut i32,
    ndelconss: &mut i32,
    naddconss: &mut i32,
    nupgdconss: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    *cutoff = false;

    if scip.conss[cons].modifiable {
        return Ok(());
    }

    // If an unmodifiable logicor constraint has only two variables, we can add
    // an implication and we will upgrade this constraint to a set-packing
    // constraint.
    if cd(scip, cons).nvars == 2 {
        // Add implication if not yet done.
        if !cd(scip, cons).impladded {
            // A two-variable logicor constraint x + y >= 1 yields the implication x
            // == 0 -> y == 1, and is represented by the clique inequality ~x + ~y <=
            // 1.
            let vars = cd(scip, cons).vars.0[..2].to_vec();
            let (implinfeasible, nimplbdchgs) = scip_var::add_clique(scip, &vars, &[false, false], false)?;
            *nchgbds += nimplbdchgs;
            if implinfeasible {
                *cutoff = true;
                return Ok(());
            }

            // Adding the above implication could lead to fixings, which render the
            // constraint redundant.
            if nimplbdchgs > 0 {
                // Remove all variables that are fixed to zero, check redundancy due to
                // fixed-to-one variable.
                let redundant = apply_fixings(scip, cons, eventhdlr, nchgcoefs, Some((&mut *naddconss, &mut *ndelconss)))?;

                if redundant {
                    scip_cons::del_cons(scip, cons)?;
                    *ndelconss += 1;
                    return Ok(());
                }
            }
            cdm(scip, cons).impladded = true;
        }

        // Still we have two variables left, we will upgrade this constraint.
        if conshdlrsetppc != NONE && scip.conss[cons].get_n_upgrade_locks() == 0 && cd(scip, cons).nvars == 2 {
            // Get correct variables.
            let (x0, x1) = (cd(scip, cons).vars[0usize], cd(scip, cons).vars[1usize]);
            let v0 = scip_var::get_negated_var(scip, x0)?;
            let v1 = scip_var::get_negated_var(scip, x1)?;

            let name = scip.conss[cons].name.clone();
            let flags = cons_flags(scip, cons);
            let mut newcons = crate::cons_setppc::create_cons_setpack(scip, &name, &[v0, v1], &flags)?;

            scip_cons::add_cons_upgrade(scip, cons, &mut newcons)?;

            scip_cons::del_cons(scip, cons)?;
            *nupgdconss += 1;
        }
    }

    // If unmodifiable constraint has no variables, it is infeasible, if
    // unmodifiable constraint has only one variable, this one can be fixed and
    // the constraint deleted.
    if cd(scip, cons).nvars == 0 {
        *cutoff = true;
    } else if cd(scip, cons).nvars == 1 {
        let var = cd(scip, cons).vars[0usize];
        if scip.vars[var].varstatus != VarStatus::MultAggr {
            let (infeasible, fixed) = scip_var::fix_var(scip, var, 1.0)?;
            if infeasible {
                *cutoff = true;
                return Ok(());
            }
            if fixed {
                *nfixedvars += 1;
            }

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
        } else if conshdlrlinear != NONE {
            let consname = format!("fixmaggr_{}_{}", scip.conss[cons].name, scip.vars[var].name);
            let f = cons_flags(scip, cons);
            let mut conslinear = crate::cons_linear::create_cons_linear(
                scip,
                &consname,
                &[var],
                &[1.0],
                1.0,
                1.0,
                f.initial,
                f.separate,
                f.enforce,
                f.check,
                f.propagate,
                f.local,
                f.modifiable,
                f.dynamic,
                f.removable,
                f.stickingatnode,
            )?;

            // Add the downgraded constraint to the problem.
            scip_cons::add_cons(scip, conslinear)?;
            scip_cons::release_cons(scip, &mut conslinear)?;
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
            *naddconss += 1;
        }
    }

    Ok(())
}

/*
 * upgrading of linear constraints
 */

/// `createNormalizedLogicor`: creates and captures a normalized (with all
/// coefficients +1) logic or constraint.
fn create_normalized_logicor(scip: &mut Scip, name: &str, vars: &[VarId], vals: &[f64], mult: i32, flags: &ConsFlags) -> ScipResult<ConsId> {
    // Get temporary memory.
    let mut transvars = vec![NONE; vars.len()];

    // Negate positive or negative variables.
    for v in 0..vars.len() {
        if mult as f64 * vals[v] > 0.0 {
            transvars[v] = vars[v];
        } else {
            transvars[v] = scip_var::get_negated_var(scip, vars[v])?;
        }
    }

    // Create the constraint.
    create_cons_logicor(scip, name, &transvars, flags)
}

/// `linconsUpgdLogicor`: check, if linear constraint can be upgraded to
/// logic or constraint
/// - logic or constraints consist only of binary variables with a
///   coefficient of +1.0 or -1.0 (variables with -1.0 coefficients can be
///   negated):
///       lhs     <= x1 + ... + xp - y1 - ... - yn <= rhs
/// - negating all variables y = (1-Y) with negative coefficients gives:
///       lhs + n <= x1 + ... + xp + Y1 + ... + Yn <= rhs + n
/// - negating all variables x = (1-X) with positive coefficients and
///   multiplying with -1 gives:
///       p - rhs <= X1 + ... + Xp + y1 + ... + yn <= p - lhs
/// - logic or constraints have left hand side of +1.0, and right hand side
///   of +infinity: x(S) >= 1.0
///   -> without negations:  (lhs == 1 - n  and  rhs == +inf)  or  (lhs ==
///   -inf  and  rhs = p - 1)
fn lincons_upgd_logicor(scip: &mut Scip, cons: ConsId, s: &crate::cons_linear::LinConsStats, upgdcons: &mut ConsId) -> ScipResult {
    let nvars = s.vars.len() as i32;
    let (lhs, rhs) = (s.lhs, s.rhs);
    let set = &scip.set;

    if nvars > 2
        && s.nposbin + s.nnegbin + s.nposimplbin + s.nnegimplbin == nvars
        && s.ncoeffspone + s.ncoeffsnone == nvars
        && ((set.is_eq(lhs, 1.0 - s.ncoeffsnone as f64) && set.is_infinity(rhs)) || (set.is_infinity(-lhs) && set.is_eq(rhs, s.ncoeffspone as f64 - 1.0)))
    {
        // Check, if we have to multiply with -1 (negate the positive vars) or with
        // +1 (negate the negative vars).
        let mult = if set.is_infinity(rhs) { 1 } else { -1 };

        // Create the logic or constraint (an automatically upgraded constraint is
        // always unmodifiable).
        let name = scip.conss[cons].name.clone();
        let flags = cons_flags(scip, cons);
        *upgdcons = create_normalized_logicor(scip, &name, &s.vars, &s.vals, mult, &flags)?;
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce constraints.
fn enforce_constraint(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut separated = false;
    let mut reduceddom = false;

    // Check all useful logic or constraints for feasibility.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff && !reduceddom {
        separate_cons(scip, conss[c], sol, eventhdlr, &mut cutoff, &mut separated, &mut reduceddom)?;
        c += 1;
    }

    // Check all obsolete logic or constraints for feasibility.
    let mut c = nusefulconss as usize;
    while c < conss.len() && !cutoff && !separated && !reduceddom {
        separate_cons(scip, conss[c], sol, eventhdlr, &mut cutoff, &mut separated, &mut reduceddom)?;
        c += 1;
    }

    // Return the correct result.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if separated {
        *result = ScipResultCode::Separated;
    } else if reduceddom {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `consFreeLogicor`: destructor of constraint handler to free constraint
/// handler data (called when SCIP is exiting).
fn cons_free_logicor(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    // Free constraint handler data.
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consInitpreLogicor`: presolving initialization method of constraint
/// handler (called when presolving is about to begin).
fn cons_initpre_logicor(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let h = hdm(scip, conshdlr);
    h.nlastcliquesneg = 0;
    h.nlastimplsneg = 0;
    h.nlastcliquesshorten = 0;
    h.nlastimplsshorten = 0;
    let eventhdlr = h.eventhdlr;

    // Catch all variable event for deleted variables, which is only used in
    // presolving.
    for &cons in conss.iter().rev() {
        for v in (0..cd(scip, cons).nvars).rev() {
            let var = cd(scip, cons).vars[v];
            api::catch_var_event(scip, var, EVENTTYPE_VARFIXED, eventhdlr, cons, None)?;
        }
    }

    Ok(())
}

/// `consExitpreLogicor`: presolving deinitialization method of constraint
/// handler (called after presolving has been finished).
fn cons_exitpre_logicor(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let mut nchgcoefs = 0;
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Drop all variable event for deleted variables, which was only used in
    // presolving.
    for &cons in conss {
        for v in 0..cd(scip, cons).nvars {
            let var = cd(scip, cons).vars[v];
            api::drop_var_event(scip, var, EVENTTYPE_VARFIXED, eventhdlr, cons, -1)?;
        }

        if !scip.conss[cons].deleted && !cd(scip, cons).presolved {
            // We are not allowed to detect infeasibility in the exitpre stage.
            let redundant = apply_fixings(scip, cons, eventhdlr, &mut nchgcoefs, None)?;

            // It may happen that a constraint still contains variables that are
            // fixed to one; for example, this happens when variable fixings have
            // been detected in the last presolving round by some other plugins (see
            // #2941).
            if redundant {
                if scip.conss[cons].is_added() {
                    scip_cons::del_cons(scip, cons)?;
                } else {
                    // We set the presolved flag to FALSE since not all fixing are
                    // removed if redundancy is detected.
                    cdm(scip, cons).presolved = false;
                }
            }
        }
    }

    Ok(())
}

/// `consInitsolLogicor`: adds the nlrows to the NLP if it had been
/// constructed, which the port never does (the NLP is not ported).
fn cons_initsol_logicor(_scip: &mut Scip, _conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    Ok(())
}

/// `consExitsolLogicor`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_logicor(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], _restart: bool) -> ScipResult {
    // Release the rows and nlrows of all constraints.
    for &cons in conss {
        let row = cd(scip, cons).row;
        if row != NONE {
            crate::lp::row_release(scip, row)?;
            cdm(scip, cons).row = NONE;
        }
    }

    Ok(())
}

/// `consDeleteLogicor`: frees specific constraint data.
fn cons_delete_logicor(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    let mut data = consdata.take().unwrap();
    let c = data.downcast_mut::<ConsData>().unwrap();

    if scip.set.stage == Stage::Presolving || scip.set.stage == Stage::InitPresolve {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        for v in (0..c.nvars).rev() {
            api::drop_var_event(scip, c.vars[v], EVENTTYPE_VARFIXED, eventhdlr, cons, -1)?;
        }
    }

    // Free LP row and logic or constraint.
    consdata_free(scip, c)
}

/// `consTransLogicor`: transforms constraint data into data belonging to the
/// transformed problem.
fn cons_trans_logicor(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let s = cd(scip, sourcecons);
    let vars = s.vars.0[..s.nvars as usize].to_vec();

    // Create constraint data for target constraint.
    let targetdata = consdata_create(scip, &vars)?;

    // Create target constraint.
    let name = scip.conss[sourcecons].name.clone();
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    Ok(())
}

/// `consInitlpLogicor`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
fn cons_initlp_logicor(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let mut c = 0;
    while c < conss.len() && !*infeasible {
        *infeasible = add_cut(scip, conss[c])?;
        c += 1;
    }

    Ok(())
}

/// The shared body of `consSepalpLogicor` and `consSepasolLogicor`.
fn sepa_logicor(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut separated = false;
    let mut reduceddom = false;

    // Check all useful logic or constraints for feasibility.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff {
        separate_cons(scip, conss[c], sol, eventhdlr, &mut cutoff, &mut separated, &mut reduceddom)?;
        c += 1;
    }

    // Combine logic or constraints to get more cuts; @todo further cuts of
    // logic or constraints.

    // Return the correct result.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if reduceddom {
        ScipResultCode::ReducedDom
    } else if separated {
        ScipResultCode::Separated
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consSepalpLogicor`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_logicor(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    sepa_logicor(scip, conshdlr, conss, nusefulconss, NONE, result)
}

/// `consSepasolLogicor`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_logicor(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    sepa_logicor(scip, conshdlr, conss, nusefulconss, sol, result)
}

/// `consEnfolpLogicor`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_logicor(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, nusefulconss, NONE, result)
}

/// `consEnforelaxLogicor`: constraint enforcing method of constraint
/// handler for relaxation solutions.
fn cons_enforelax_logicor(scip: &mut Scip, sol: SolId, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, nusefulconss, sol, result)
}

/// `consEnfopsLogicor`: constraint enforcing method of constraint handler
/// for pseudo solutions.
#[allow(clippy::too_many_arguments)]
fn cons_enfops_logicor(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut infeasible = false;
    let mut reduceddom = false;
    let mut solvelp = false;

    // Check all logic or constraints for feasibility.
    let mut c = 0;
    while c < conss.len() && !cutoff && !reduceddom && !solvelp {
        enforce_pseudo(scip, conss[c], eventhdlr, &mut cutoff, &mut infeasible, &mut reduceddom, &mut solvelp)?;
        c += 1;
    }

    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if reduceddom {
        *result = ScipResultCode::ReducedDom;
    } else if solvelp {
        *result = ScipResultCode::SolveLp;
    } else if infeasible {
        *result = ScipResultCode::Infeasible;
    }

    Ok(())
}

/// `consCheckLogicor`: feasibility check method of constraint handler for
/// integral solutions; printing the reason is not ported.
#[allow(clippy::too_many_arguments)]
fn cons_check_logicor(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    checklprows: bool,
    _printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    // Check all logic or constraints for feasibility.
    let mut c = 0;
    while c < conss.len() && (*result == ScipResultCode::Feasible || completely) {
        let cons = conss[c];
        let row = cd(scip, cons).row;
        if checklprows || row == NONE || scip.rows[row].lppos < 0 {
            if is_cons_violated(scip, cons, sol) {
                // Constraint is violated.
                *result = ScipResultCode::Infeasible;
            }
        }
        c += 1;
    }

    Ok(())
}

/// `consPropLogicor`: domain propagation method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_prop_logicor(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nusefulconss: i32,
    _nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut reduceddom = false;

    // Propagate all useful logic or constraints.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff {
        process_watched_vars(scip, conss[c], eventhdlr, &mut cutoff, &mut reduceddom)?;
        c += 1;
    }

    // Return the correct result.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if reduceddom {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consPresolLogicor`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_logicor(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nrounds: i32,
    presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    let nconss = conss.len() as i32;

    *result = ScipResultCode::DidNotFind;

    let h = hd(scip, conshdlr);
    let (eventhdlr, conshdlrlinear, conshdlrsetppc) = (h.eventhdlr, h.conshdlrlinear, h.conshdlrsetppc);
    let (dualpresolving, presolusehashing, presolpairwise, usestrengthening, useimplications, usenegatedclique) = (
        h.dualpresolving,
        h.presolusehashing,
        h.presolpairwise,
        h.usestrengthening,
        h.useimplications,
        h.usenegatedclique,
    );

    let nentries = api::get_n_vars(scip) - api::get_n_cont_vars(scip);

    let oldnfixedvars = counts.nfixedvars;
    let oldnchgbds = counts.nchgbds;
    let oldndelconss = counts.ndelconss;
    let oldnupgdconss = counts.nupgdconss;
    let oldnchgcoefs = counts.nchgcoefs;

    let mut firstchange = i32::MAX;

    let mut entries = vec![0u8; nentries as usize];

    // Whether a change happened since the start (C's repeated success test).
    let changed = |counts: &PresolCounts| {
        counts.nfixedvars > oldnfixedvars || counts.nchgbds > oldnchgbds || counts.nchgcoefs > oldnchgcoefs || counts.ndelconss > oldndelconss || counts.nupgdconss > oldnupgdconss
    };

    // Process constraints.
    let mut c = 0;
    while c < nconss && *result != ScipResultCode::Cutoff && !crate::solve::solve_is_stopped(scip, false) {
        let cons = conss[c as usize];
        c += 1;

        // Force presolving the constraint in the initial round.
        if nrounds == 0 {
            cons_enable_propagation(scip, cons);
        }

        let mut redundant = false;
        if !cd(scip, cons).presolved {
            // Remove all variables that are fixed to zero, check redundancy due to
            // fixed-to-one variable.
            redundant = apply_fixings(scip, cons, eventhdlr, &mut counts.nchgcoefs, Some((&mut counts.naddconss, &mut counts.ndelconss)))?;
        }

        if scip.conss[cons].deleted {
            continue;
        }

        // Find pairs of negated variables in constraint: constraint is redundant;
        // find sets of equal variables in constraint: multiple entries of variable
        // can be replaced by single entry.
        if !redundant {
            redundant = merge_multiples(scip, cons, eventhdlr, &mut entries, &mut counts.nchgcoefs)?;
        }

        if redundant {
            scip_cons::del_cons(scip, cons)?;
            counts.ndelconss += 1;
            *result = ScipResultCode::Success;
            continue;
        } else if !scip.conss[cons].modifiable {
            if cd(scip, cons).nvars <= 2 {
                let mut cutoff = false;

                // Handle all cases with less than three variables in a logicor
                // constraint.
                fix_delete_or_upgrade_cons(
                    scip,
                    cons,
                    eventhdlr,
                    conshdlrlinear,
                    conshdlrsetppc,
                    &mut counts.nfixedvars,
                    &mut counts.nchgbds,
                    &mut counts.nchgcoefs,
                    &mut counts.ndelconss,
                    &mut counts.naddconss,
                    &mut counts.nupgdconss,
                    &mut cutoff,
                )?;

                if cutoff {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                } else if changed(counts) {
                    *result = ScipResultCode::Success;
                }

                if scip.conss[cons].deleted {
                    continue;
                }
            }
        }

        // Perform dual reductions.
        if dualpresolving && scip_var::allow_strong_dual_reds(scip) {
            if dual_presolving(
                scip,
                cons,
                eventhdlr,
                &mut counts.nfixedvars,
                &mut counts.ndelconss,
                &mut counts.nchgcoefs,
                &mut counts.naggrvars,
            )? {
                *result = ScipResultCode::Success;
            }

            // If dual reduction deleted the constraint we take the next.
            if !scip.conss[cons].is_active() {
                continue;
            }

            // In dualpresolving we may have removed variables, so we need to take
            // care of special cases.
            if cd(scip, cons).nvars <= 2 {
                let mut cutoff = false;

                // Handle all cases with less than three variables in a logicor
                // constraint.
                fix_delete_or_upgrade_cons(
                    scip,
                    cons,
                    eventhdlr,
                    conshdlrlinear,
                    conshdlrsetppc,
                    &mut counts.nfixedvars,
                    &mut counts.nchgbds,
                    &mut counts.nchgcoefs,
                    &mut counts.ndelconss,
                    &mut counts.naddconss,
                    &mut counts.nupgdconss,
                    &mut cutoff,
                )?;

                if cutoff {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                } else if changed(counts) {
                    *result = ScipResultCode::Success;
                }

                if scip.conss[cons].deleted {
                    continue;
                }
            }
        }

        // Remember the first changed constraint to begin the next redundancy
        // round with.
        if firstchange == i32::MAX && cd(scip, cons).changed {
            firstchange = c - 1;
        }
    }

    // Fast preprocessing of pairs of logic or constraints, used for equal
    // constraints.
    if firstchange < nconss && presolusehashing {
        // Detect redundant constraints; fast version with hash table instead of
        // pairwise comparison.
        detect_redundant_constraints(scip, conss, &mut firstchange, &mut counts.ndelconss)?;
    }

    // Preprocess pairs of logic or constraints and apply negated clique
    // presolving.
    if api::is_presolve_finished_with(scip, counts) {
        let mut cutoff = false;

        // Check constraints for redundancy.
        if presolpairwise && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
            remove_redundant_conss_and_nonzeros(
                scip,
                conss,
                &mut entries,
                eventhdlr,
                usestrengthening,
                &mut firstchange,
                &mut counts.nfixedvars,
                &mut counts.ndelconss,
                &mut counts.nchgcoefs,
                &mut cutoff,
            )?;

            if cutoff {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }
        }

        if api::is_presolve_finished_with(scip, counts) {
            // Try to tighten constraints by reducing the number of variables in the
            // constraints using implications and cliques, also derive fixations
            // through them, see SCIPshrinkDisjunctiveVarSet().
            if useimplications && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
                shorten_conss(
                    scip,
                    conshdlr,
                    eventhdlr,
                    conss,
                    &mut entries,
                    &mut counts.nfixedvars,
                    &mut counts.ndelconss,
                    &mut counts.nchgcoefs,
                    &mut cutoff,
                )?;

                if cutoff {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                }
            }

            // Check for redundant constraints due to negated clique information.
            if usenegatedclique && (presoltiming & PRESOLTIMING_MEDIUM) != 0 {
                remove_constraints_due_to_neg_cliques(
                    scip,
                    conshdlr,
                    conshdlrsetppc,
                    eventhdlr,
                    conss,
                    &mut entries,
                    &mut counts.nfixedvars,
                    &mut counts.ndelconss,
                    &mut counts.nupgdconss,
                    &mut counts.nchgcoefs,
                    &mut cutoff,
                )?;

                if cutoff {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// `consRespropLogicor`: propagation conflict resolving method of
/// constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_logicor(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    infervar: VarId,
    _inferinfo: i32,
    _boundtype: BoundType,
    _bdchgidx: Option<BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    // The only deductions are variables inferred to 1.0 on logic or
    // constraints where all other variables are assigned to zero.
    for v in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];
        if var != infervar {
            // The reason variable must have been assigned to zero.
            api::add_conflict_binvar(scip, var)?;
        }
    }

    *result = ScipResultCode::Success;

    Ok(())
}

/// `consLockLogicor`: variable rounding lock method of constraint handler.
fn cons_lock_logicor(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    // Lock every single coefficient.
    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
    }

    Ok(())
}

/// `consActiveLogicor`: constraint activation notification method of
/// constraint handler (the nlrow part is not ported).
fn cons_active_logicor(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Catch events on watched variables.
    if cd(scip, cons).watchedvar1 != -1 {
        let var = cd(scip, cons).vars[cd(scip, cons).watchedvar1];
        let mut filterpos = -1;
        api::catch_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, Some(&mut filterpos))?;
        cdm(scip, cons).filterpos1 = filterpos;
    }
    if cd(scip, cons).watchedvar2 != -1 {
        let var = cd(scip, cons).vars[cd(scip, cons).watchedvar2];
        let mut filterpos = -1;
        api::catch_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, Some(&mut filterpos))?;
        cdm(scip, cons).filterpos2 = filterpos;
    }

    Ok(())
}

/// `consDeactiveLogicor`: constraint deactivation notification method of
/// constraint handler.
fn cons_deactive_logicor(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Drop events on watched variables.
    if cd(scip, cons).watchedvar1 != -1 {
        let c = cd(scip, cons);
        let (var, filterpos) = (c.vars[c.watchedvar1], c.filterpos1);
        api::drop_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, filterpos)?;
        let c = cdm(scip, cons);
        c.watchedvar1 = -1;
        c.filterpos1 = -1;
    }
    if cd(scip, cons).watchedvar2 != -1 {
        let c = cd(scip, cons);
        let (var, filterpos) = (c.vars[c.watchedvar2], c.filterpos2);
        api::drop_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, filterpos)?;
        let c = cdm(scip, cons);
        c.watchedvar2 = -1;
        c.filterpos2 = -1;
    }

    // Remove row from NLP, if still in solving (the NLP is not ported).
    Ok(())
}

/// `consGetVarsLogicor`: constraint method of constraint handler which
/// returns the variables (the vector always fits here).
fn cons_get_vars_logicor(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.extend_from_slice(&c.vars.0[..c.nvars as usize]);
    *success = true;
    Ok(())
}

/// `consGetNVarsLogicor`: constraint method of constraint handler which
/// returns the number of variables.
fn cons_get_n_vars_logicor(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = cd(scip, cons).nvars;
    *success = true;
    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecLogicor`: execution method of event handler; the event data is
/// the constraint.
fn event_exec_logicor(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let cons: ConsId = eventdata;

    if event.eventtype == EVENTTYPE_LBRELAXED {
        cons_enable(scip, cons)?;
        cons_enable_propagation(scip, cons);
    } else if event.eventtype == EVENTTYPE_UBTIGHTENED {
        cons_enable_propagation(scip, cons);
    }

    if event.eventtype == EVENTTYPE_VARFIXED {
        let var = event.data.var;

        // We only catch this event in presolving stage.
        cdm(scip, cons).presolved = false;

        if scip.vars[var].varstatus != VarStatus::Fixed {
            if scip.conss[cons].is_active() {
                let x = &scip.vars[var];
                if x.glbdom.lb < 0.5 && x.glbdom.ub > 0.5 {
                    cdm(scip, cons).merged = false;
                }

                if !cd(scip, cons).existmultaggr {
                    let probvar = crate::var::var_get_probvar(scip, var);
                    if scip.vars[probvar].varstatus == VarStatus::MultAggr {
                        cdm(scip, cons).existmultaggr = true;
                    }
                }
            }
        }
    }

    Ok(())
}

/*
 * Callback methods of conflict handler
 */

/// `conflictExecLogicor`: conflict processing method of conflict handler
/// (called when conflict was found).
fn conflict_exec_logicor(scip: &mut Scip, _conflicthdlr: crate::types::ConflicthdlrId, args: &crate::conflict::ConflictExecArgs, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Don't process already resolved conflicts.
    if args.resolved {
        return Ok(());
    }

    let nbdchginfos = args.bdchginfos.len();

    // If the conflict consists of only two (binary) variables, it will be
    // handled by the setppc conflict handler.
    if nbdchginfos == 2 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Create array of variables in conflict constraint.
    let mut vars = vec![NONE; nbdchginfos];
    let mut i = 0;
    while i < nbdchginfos {
        let info = crate::conflict::bdchginfo(scip, args.bdchginfos[i]);
        vars[i] = info.var;

        // We can only treat binary variables.
        if !crate::var::var_is_binary(scip, vars[i]) {
            break;
        }

        // If the variable is fixed to one in the conflict set, we have to use its
        // negation.
        if info.newbound > 0.5 {
            vars[i] = scip_var::get_negated_var(scip, vars[i])?;
        }
        i += 1;
    }

    if i == nbdchginfos {
        // Create a constraint out of the conflict set.
        let napplied = scip.conflict.as_ref().map_or(0, |c| c.nappliedglbconss + c.nappliedlocconss);
        let consname = format!("cf{}_{}", api::get_n_runs(scip), napplied);
        let flags = ConsFlags {
            initial: false,
            separate: args.separate,
            enforce: false,
            check: false,
            propagate: true,
            local: args.local,
            modifiable: false,
            dynamic: args.dynamic,
            removable: args.removable,
            stickingatnode: false,
        };
        let mut cons = create_cons_logicor(scip, &consname, &vars, &flags)?;

        // Add conflict to SCIP.
        scip_cons::add_conflict(scip, args.node, &mut cons, args.validnode, args.conftype, args.cutoffinvolved)?;
        *result = ScipResultCode::ConsAdded;
    }

    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrLogicor`: creates the handler for logic or
/// constraints and includes it in SCIP. Sub-SCIP copying, parsing, printing
/// and the symmetry graphs are added with those modules.
pub fn include_conshdlr_logicor(scip: &mut Scip) -> ScipResult {
    use crate::paramset::paramset_add_bool;

    // Create event handler for events on watched variables.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_logicor, None)?;

    // Create conflict handler for logic or constraints.
    api::include_conflicthdlr_basic(scip, CONFLICTHDLR_NAME, CONFLICTHDLR_DESC, CONFLICTHDLR_PRIORITY, conflict_exec_logicor, None)?;

    // Create constraint handler data.
    let conshdlrdata = conshdlrdata_create(eventhdlr);

    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_logicor),
        Some(cons_enfops_logicor),
        Some(cons_check_logicor),
        Some(cons_lock_logicor),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_logicor);
    h.conscopy = Some(cons_copy_logicor);
    h.consactive = Some(cons_active_logicor);
    h.consdeactive = Some(cons_deactive_logicor);
    h.consdelete = Some(cons_delete_logicor);
    h.consexitpre = Some(cons_exitpre_logicor);
    h.consinitsol = Some(cons_initsol_logicor);
    h.consexitsol = Some(cons_exitsol_logicor);
    h.consfree = Some(cons_free_logicor);
    h.consgetvars = Some(cons_get_vars_logicor);
    h.consgetnvars = Some(cons_get_n_vars_logicor);
    h.consinitpre = Some(cons_initpre_logicor);
    h.consinitlp = Some(cons_initlp_logicor);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_logicor), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_logicor), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_logicor);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_logicor),
        Some(cons_sepasol_logicor),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_logicor);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_logicor);

    let conshdlrlinear = crate::set::set_find_conshdlr(scip, "linear");
    let conshdlrsetppc = crate::set::set_find_conshdlr(scip, "setppc");
    hdm(scip, conshdlr).conshdlrlinear = conshdlrlinear;
    hdm(scip, conshdlr).conshdlrsetppc = conshdlrsetppc;

    if conshdlrlinear != NONE {
        // Include the linear constraint to logicor constraint upgrade in the
        // linear constraint handler.
        crate::cons_linear::include_lincons_upgrade(scip, lincons_upgd_logicor, LINCONSUPGD_PRIORITY, CONSHDLR_NAME)?;
    }

    // Logic or constraint handler parameters.
    paramset_add_bool(
        scip,
        "constraints/logicor/presolpairwise",
        "should pairwise constraint comparison be performed in presolving?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolpairwise),
        true,
        DEFAULT_PRESOLPAIRWISE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/logicor/presolusehashing",
        "should hash table be used for detecting redundant constraints in advance",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolusehashing),
        true,
        DEFAULT_PRESOLUSEHASHING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/logicor/dualpresolving",
        "should dual presolving steps be performed?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, dualpresolving),
        true,
        DEFAULT_DUALPRESOLVING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/logicor/negatedclique",
        "should negated clique information be used in presolving",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, usenegatedclique),
        true,
        DEFAULT_NEGATEDCLIQUE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/logicor/implications",
        "should implications/cliques be used in presolving",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, useimplications),
        true,
        DEFAULT_IMPLICATIONS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/logicor/strengthen",
        "should pairwise constraint comparison try to strengthen constraints by removing superflous non-zeros?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, usestrengthening),
        true,
        DEFAULT_STRENGTHEN,
        None,
    )?;

    Ok(())
}

/// `SCIPcreateConsLogicor`: creates and captures a logic or constraint.
pub fn create_cons_logicor(scip: &mut Scip, name: &str, vars: &[VarId], flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the logicor constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Logic or constraint handler not found.
        return Err(Retcode::InvalidCall);
    }

    // Check whether all variables are binary.
    for &var in vars {
        if !crate::var::var_is_binary(scip, var) {
            // Operand is not binary.
            return Err(Retcode::InvalidData);
        }
    }

    // Create the constraint specific data.
    let consdata = consdata_create(scip, vars)?;

    // Create constraint.
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)?;

    if api::is_transformed(scip) && scip.set.stage == Stage::Presolving {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        for v in (0..cd(scip, cons).nvars).rev() {
            let var = cd(scip, cons).vars[v];
            api::catch_var_event(scip, var, EVENTTYPE_VARFIXED, eventhdlr, cons, None)?;
        }
    }

    Ok(cons)
}

/// `SCIPcreateConsBasicLogicor`: creates and captures a logicor constraint
/// in its most basic version.
pub fn create_cons_basic_logicor(scip: &mut Scip, name: &str, vars: &[VarId]) -> ScipResult<ConsId> {
    let flags = ConsFlags {
        initial: true,
        separate: true,
        enforce: true,
        check: true,
        propagate: true,
        local: false,
        modifiable: false,
        dynamic: false,
        removable: false,
        stickingatnode: false,
    };
    create_cons_logicor(scip, name, vars, &flags)
}

/// Whether the constraint belongs to the logicor constraint handler.
fn is_logicor(scip: &Scip, cons: ConsId) -> bool {
    scip.conshdlrs[scip.conss[cons].conshdlr].name == CONSHDLR_NAME
}

/// `SCIPaddCoefLogicor`: adds coefficient in logic or constraint.
pub fn add_coef_logicor(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    if !is_logicor(scip, cons) {
        // Constraint is not a logic or constraint.
        return Err(Retcode::InvalidData);
    }

    add_coef(scip, cons, var)
}

/// `SCIPgetNVarsLogicor`: gets number of variables in logic or constraint.
pub fn get_n_vars_logicor(scip: &Scip, cons: ConsId) -> i32 {
    assert!(is_logicor(scip, cons), "constraint is not a logic or constraint");
    cd(scip, cons).nvars
}

/// `SCIPgetVarsLogicor`: gets array of variables in logic or constraint.
pub fn get_vars_logicor(scip: &Scip, cons: ConsId) -> &[VarId] {
    assert!(is_logicor(scip, cons), "constraint is not a logic or constraint");
    let c = cd(scip, cons);
    &c.vars.0[..c.nvars as usize]
}

/// `SCIPgetDualsolLogicor`: gets the dual solution of the logic or
/// constraint in the current LP.
pub fn get_dualsol_logicor(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_logicor(scip, cons), "constraint is not a logic or constraint");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualsol(scip, row) } else { 0.0 }
}

/// `SCIPgetDualfarkasLogicor`: gets the dual Farkas value of the logic or
/// constraint in the current infeasible LP.
pub fn get_dualfarkas_logicor(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_logicor(scip, cons), "constraint is not a logic or constraint");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualfarkas(scip, row) } else { 0.0 }
}

/// `SCIPgetRowLogicor`: returns the linear relaxation of the given logic or
/// constraint; may return `NONE` if no LP row was yet created.
pub fn get_row_logicor(scip: &Scip, cons: ConsId) -> usize {
    assert!(is_logicor(scip, cons), "constraint is not a logic or constraint");
    cd(scip, cons).row
}

/// `SCIPcreateRowLogicor`: creates and returns the row of the given logicor
/// constraint.
pub fn create_row_logicor(scip: &mut Scip, cons: ConsId) -> ScipResult {
    assert!(is_logicor(scip, cons), "constraint is not a logic or constraint");
    create_row(scip, cons)
}

/// `SCIPcleanupConssLogicor`: cleans up (multi-)aggregations and fixings
/// from logicor constraints.
pub fn cleanup_conss_logicor(scip: &mut Scip, onlychecked: bool, naddconss: &mut i32, ndelconss: &mut i32, nchgcoefs: &mut i32) -> ScipResult {
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        return Ok(());
    }

    let eventhdlr = hd(scip, conshdlr).eventhdlr;
    let h = &scip.conshdlrs[conshdlr];
    let conss: Vec<ConsId> = if onlychecked {
        h.checkconss.0[..h.ncheckconss as usize].to_vec()
    } else {
        h.conss.0[..h.nactiveconss as usize].to_vec()
    };

    let nentries = api::get_n_vars(scip) - api::get_n_cont_vars(scip);
    let mut entries = vec![0u8; nentries as usize];

    // Loop backwards since then deleted constraints do not interfere with the
    // loop.
    for &cons in conss.iter().rev() {
        let mut redundant = apply_fixings(scip, cons, eventhdlr, nchgcoefs, Some((&mut *naddconss, &mut *ndelconss)))?;

        if scip.conss[cons].deleted {
            continue;
        }

        // Merge constraint.
        if !redundant {
            redundant = merge_multiples(scip, cons, eventhdlr, &mut entries, nchgcoefs)?;
        }

        if redundant {
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
        }
    }

    Ok(())
}

/// `conshdlrCopyLogicor`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_logicor(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_logicor(scip)?;
    *valid = true;
    Ok(())
}

/// `consCopyLogicor`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_logicor(
    scip: &mut Scip,
    cons: &mut ConsId,
    name: Option<&str>,
    sourcescip: &mut Scip,
    _sourceconshdlr: ConshdlrId,
    sourcecons: ConsId,
    maps: &mut crate::copy::CopyMaps,
    flags: &ConsFlags,
    global: bool,
    valid: &mut bool,
) -> ScipResult {
    // Get variables and coefficients of the source constraint.
    let sourcevars = get_vars_logicor(sourcescip, sourcecons).to_vec();
    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);

    // Copy the logic using the linear constraint copy method.
    let rhs = scip.set.infinity();
    (*cons, *valid) = crate::cons_linear::copy_cons_linear(scip, sourcescip, &consname, &sourcevars, None, 1.0, rhs, maps, flags, global)?;
    Ok(())
}
