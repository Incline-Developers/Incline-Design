//! Port of `cons_and.c`: constraint handler for AND-constraints,
//! `r = and(x1, ..., xn)`.
//!
//! Not ported: printing and parsing, and the NLP row's creation stays as in
//! C (only when the NLP is constructed).

use std::any::Any;

use crate::{
    api,
    cons::*,
    cvec::CVec,
    event::*,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var,
    types::{BoundType, ConsId, ConshdlrId, EventhdlrId, ScipResultCode, SolId, VarId, VarStatus},
    var::{BdChgIdx, NONE},
};

/* constraint handler properties */
pub const CONSHDLR_NAME: &str = "and";
const CONSHDLR_DESC: &str = "constraint handler for AND-constraints: r = and(x1, ..., xn)";
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 850100;
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -850100;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -850100;
/// Frequency for separating cuts; zero means to separate only in the root
/// node.
const CONSHDLR_SEPAFREQ: i32 = 1;
/// Frequency for propagating domains; zero means only preprocessing
/// propagation.
const CONSHDLR_PROPFREQ: i32 = 1;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0 for
/// first only.
const CONSHDLR_EAGERFREQ: i32 = 100;
/// Maximal number of presolving rounds the constraint handler participates
/// in (-1: no limit).
const CONSHDLR_MAXPREROUNDS: i32 = -1;
/// Should separation method be delayed, if other separators found cuts?
const CONSHDLR_DELAYSEPA: bool = false;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const CONSHDLR_DELAYPROP: bool = false;
/// Should the constraint handler be skipped, if no constraints are available?
const CONSHDLR_NEEDSCONS: bool = true;

const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST | PRESOLTIMING_EXHAUSTIVE;
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

const EVENTHDLR_NAME: &str = "and";
const EVENTHDLR_DESC: &str = "bound change event handler for AND-constraints";

/// Should pairwise constraint comparison be performed in presolving?
const DEFAULT_PRESOLPAIRWISE: bool = true;
/// Should constraint get linearized and removed?
const DEFAULT_LINEARIZE: bool = false;
/// Should cuts be separated during LP enforcing?
const DEFAULT_ENFORCECUTS: bool = true;
/// Should an aggregated linearization be used?
const DEFAULT_AGGRLINEARIZATION: bool = false;
/// Should implied integrality of resultant variables be detected?
const DEFAULT_UPGRRESULTANT: bool = false;
/// Should dual presolving be performed?
const DEFAULT_DUALPRESOLVING: bool = true;

/// Minimal size of hash table in and constraint tables.
const HASHSIZE_ANDCONS: i32 = 500;
/// Should hash table be used for detecting redundant constraints in advance.
const DEFAULT_PRESOLUSEHASHING: bool = true;
/// Number for minimal pairwise presolving comparisons.
const NMINCOMPARISONS: i32 = 200000;
/// Minimal gain per minimal pairwise presolving comparisons to repeat
/// pairwise comparison round.
const MINGAINPERNMINCOMPARISONS: f64 = 1e-06;

/// Events caught on watched operator variables.
const WATCHED_EVENTS: EventType = EVENTTYPE_LBTIGHTENED | EVENTTYPE_UBRELAXED;
/// Events caught on all operator variables.
const OPERATOR_EVENTS: EventType = EVENTTYPE_UBTIGHTENED | EVENTTYPE_LBRELAXED;

/*
 * Data structures
 */

/// `SCIP_ConsData`: constraint data for AND-constraints.
#[derive(Default)]
pub struct ConsData {
    /// Variables in the AND-constraint.
    pub vars: CVec<VarId>,
    /// Resultant variable.
    pub resvar: VarId,
    /// Rows for linear relaxation of AND-constraint (`NULL`: empty).
    pub rows: Vec<crate::types::RowId>,
    /// Aggregated row for linear relaxation of AND-constraint.
    pub aggrrow: crate::types::RowId,
    /// Row for representation in nonlinear relaxation.
    pub nlrow: usize,
    /// Number of variables in AND-constraint.
    pub nvars: i32,
    /// Size of vars array.
    pub varssize: i32,
    /// Position of first watched operator variable.
    pub watchedvar1: i32,
    /// Position of second watched operator variable.
    pub watchedvar2: i32,
    /// Event filter position of first watched operator variable.
    pub filterpos1: i32,
    /// Event filter position of second watched operator variable.
    pub filterpos2: i32,
    /// Is constraint already preprocessed/propagated?
    pub propagated: bool,
    /// Is none of the operator variables fixed to FALSE?
    pub nofixedzero: bool,
    /// Were the implications of the constraint already added?
    pub impladded: bool,
    /// Was the implication for 2 operands with fixed resultant added?
    pub opimpladded: bool,
    /// Are the constraint's variables sorted?
    pub sorted: bool,
    /// Was constraint changed since last pair preprocessing round?
    pub changed: bool,
    /// Are the constraint's equal variables already merged?
    pub merged: bool,
}

/// `SCIP_ConshdlrData`: constraint handler data.
pub struct ConshdlrData {
    /// Event handler for bound change events on watched variables.
    pub eventhdlr: EventhdlrId,
    /// Should pairwise constraint comparison be performed in presolving?
    pub presolpairwise: bool,
    /// Should hash table be used for detecting redundant constraints in
    /// advance.
    pub presolusehashing: bool,
    /// Should constraint get linearized and removed?
    pub linearize: bool,
    /// Should cuts be separated during LP enforcing?
    pub enforcecuts: bool,
    /// Should an aggregated linearization be used?
    pub aggrlinearization: bool,
    /// Should implied integrality of resultant variables be detected?
    pub upgrresultant: bool,
    /// Should dual presolving be performed?
    pub dualpresolving: bool,
}

/// The constraint data of an AND-constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of an AND-constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/*
 * Propagation rules
 */

/// `PROPRULE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Proprule {
    /// Propagation was applied without a specific propagation rule.
    Invalid = 0,
    /// v_i = FALSE                                  =>  r   = FALSE
    Rule1 = 1,
    /// r   = TRUE                                   =>  v_i = TRUE for all i
    Rule2 = 2,
    /// v_i = TRUE for all i                         =>  r   = TRUE
    Rule3 = 3,
    /// r   = FALSE, v_i = TRUE for all i except j   =>  v_j = FALSE
    Rule4 = 4,
}

impl Proprule {
    fn from_i32(i: i32) -> Self {
        match i {
            1 => Proprule::Rule1,
            2 => Proprule::Rule2,
            3 => Proprule::Rule3,
            4 => Proprule::Rule4,
            _ => Proprule::Invalid,
        }
    }
}

/*
 * Local methods
 */

/// `lockRounding`: installs rounding locks for the given variable in the
/// given AND-constraint.
fn lock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    // Rounding in both directions may violate the constraint.
    api::lock_var_cons(scip, var, cons, true, true)
}

/// `unlockRounding`: removes rounding locks for the given variable in the
/// given AND-constraint.
fn unlock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    // Rounding in both directions may violate the constraint.
    api::unlock_var_cons(scip, var, cons, true, true)
}

/// `consdataCatchWatchedEvents`: catches events for the watched variable at
/// given position; the event data is the constraint; returns the event filter
/// position.
fn consdata_catch_watched_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult<i32> {
    // Catch tightening events for lower bound and relaxed events for upper
    // bounds on watched variable.
    let var = cd(scip, cons).vars[pos];
    let mut filterpos = -1;
    api::catch_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, Some(&mut filterpos))?;
    Ok(filterpos)
}

/// `consdataDropWatchedEvents`: drops events for the watched variable at
/// given position.
fn consdata_drop_watched_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32, filterpos: i32) -> ScipResult {
    // Drop tightening events for lower bound and relaxed events for upper
    // bounds on watched variable.
    let var = cd(scip, cons).vars[pos];
    api::drop_var_event(scip, var, WATCHED_EVENTS, eventhdlr, cons, filterpos)
}

/// `consdataCatchEvents`: catches needed events on all variables of
/// constraint, except the special ones for watched variables.
fn consdata_catch_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    // Catch bound change events for both bounds on resultant variable.
    let resvar = cd(scip, cons).resvar;
    api::catch_var_event(scip, resvar, EVENTTYPE_BOUNDCHANGED, eventhdlr, cons, None)?;

    // Catch tightening events for upper bound and relaxed events for lower
    // bounds on operator variables.
    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        api::catch_var_event(scip, var, OPERATOR_EVENTS, eventhdlr, cons, None)?;
    }

    Ok(())
}

/// `consdataDropEvents`: drops events on all variables of constraint,
/// except the special ones for watched variables.
fn consdata_drop_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    // Drop bound change events for both bounds on resultant variable.
    let resvar = cd(scip, cons).resvar;
    api::drop_var_event(scip, resvar, EVENTTYPE_BOUNDCHANGED, eventhdlr, cons, -1)?;

    // Drop tightening events for upper bound and relaxed events for lower
    // bounds on operator variables.
    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        api::drop_var_event(scip, var, OPERATOR_EVENTS, eventhdlr, cons, -1)?;
    }

    Ok(())
}

/// `consdataSwitchWatchedvars`: stores the given variable numbers as watched
/// variables, and updates the event processing.
fn consdata_switch_watchedvars(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, watchedvar1: i32, watchedvar2: i32) -> ScipResult {
    // If one watched variable is equal to the old other watched variable, just
    // switch positions.
    let c = cdm(scip, cons);
    if watchedvar1 == c.watchedvar2 || watchedvar2 == c.watchedvar1 {
        std::mem::swap(&mut c.watchedvar1, &mut c.watchedvar2);
        std::mem::swap(&mut c.filterpos1, &mut c.filterpos2);
    }

    // Drop events on old watched variables.
    let (w1, w2, f1, f2) = {
        let c = cd(scip, cons);
        (c.watchedvar1, c.watchedvar2, c.filterpos1, c.filterpos2)
    };
    if w1 != -1 && w1 != watchedvar1 {
        consdata_drop_watched_events(scip, cons, eventhdlr, w1, f1)?;
    }
    if w2 != -1 && w2 != watchedvar2 {
        consdata_drop_watched_events(scip, cons, eventhdlr, w2, f2)?;
    }

    // Catch events on new watched variables.
    if watchedvar1 != -1 && watchedvar1 != w1 {
        let filterpos = consdata_catch_watched_events(scip, cons, eventhdlr, watchedvar1)?;
        cdm(scip, cons).filterpos1 = filterpos;
    }
    if watchedvar2 != -1 && watchedvar2 != w2 {
        let filterpos = consdata_catch_watched_events(scip, cons, eventhdlr, watchedvar2)?;
        cdm(scip, cons).filterpos2 = filterpos;
    }

    // Set the new watched variables.
    let c = cdm(scip, cons);
    c.watchedvar1 = watchedvar1;
    c.watchedvar2 = watchedvar2;

    Ok(())
}

/// `consdataEnsureVarsSize`: ensures, that the vars array can store at least
/// num entries.
fn consdata_ensure_vars_size(scip: &mut Scip, cons: ConsId, num: i32) {
    if num > cd(scip, cons).varssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = cdm(scip, cons);
        c.vars.0.resize(newsize as usize, NONE);
        c.varssize = newsize;
    }
}

/// `consdataCreate`: creates constraint data for AND-constraint; the events
/// (C catches them here in the transformed problem) are caught by
/// [`consdata_catch_events`] once the constraint exists.
fn consdata_create(scip: &mut Scip, vars: &[VarId], resvar: VarId) -> ScipResult<ConsData> {
    let nvars = vars.len() as i32;
    let mut c = ConsData {
        vars: CVec(vars.to_vec()),
        resvar,
        rows: Vec::new(),
        aggrrow: NONE,
        nlrow: NONE,
        nvars,
        varssize: nvars,
        watchedvar1: -1,
        watchedvar2: -1,
        filterpos1: -1,
        filterpos2: -1,
        propagated: false,
        nofixedzero: false,
        impladded: false,
        opimpladded: false,
        sorted: false,
        changed: true,
        merged: false,
    };

    // Get transformed variables, if we are in the transformed problem.
    if api::is_transformed(scip) {
        api::get_transformed_vars(scip, &mut c.vars.0[..nvars as usize])?;
        c.resvar = api::get_transformed_var(scip, c.resvar)?;
    }

    // Note: currently, this constraint handler does not handle
    // multiaggregations (e.g. during propagation), hence we forbid
    // multiaggregation from the beginning for the involved variables.
    if scip.set.stage <= Stage::ExitPresolve {
        for v in 0..nvars {
            scip_var::mark_do_not_multaggr_var(scip, c.vars[v])?;
        }
        scip_var::mark_do_not_multaggr_var(scip, c.resvar)?;
    }

    // Capture vars.
    crate::var::var_capture(scip, c.resvar);
    for v in 0..nvars {
        crate::var::var_capture(scip, c.vars[v]);
    }

    Ok(c)
}

/// `consdataFreeRows`: releases LP rows of constraint data and frees rows
/// array.
fn consdata_free_rows(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let rows = std::mem::take(&mut cdm(scip, cons).rows);
    for mut r in rows {
        scip_lp::release_row(scip, &mut r)?;
    }

    let mut aggrrow = cd(scip, cons).aggrrow;
    if aggrrow != NONE {
        scip_lp::release_row(scip, &mut aggrrow)?;
        cdm(scip, cons).aggrrow = NONE;
    }

    Ok(())
}

/// `consdataFree`: frees constraint data for AND-constraint.
fn consdata_free(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    if api::is_transformed(scip) {
        // Drop events for watched variables.
        consdata_switch_watchedvars(scip, cons, eventhdlr, -1, -1)?;

        // Drop all other events on variables.
        consdata_drop_events(scip, cons, eventhdlr)?;
    }

    // Release and free the rows.
    consdata_free_rows(scip, cons)?;

    // Release the nlrow.
    let mut nlrow = cd(scip, cons).nlrow;
    if nlrow != NONE {
        crate::nlp::release_nl_row(scip, &mut nlrow)?;
        cdm(scip, cons).nlrow = NONE;
    }

    // Release vars.
    for v in 0..cd(scip, cons).nvars {
        let mut var = cd(scip, cons).vars[v];
        api::release_var(scip, &mut var)?;
    }
    let mut resvar = cd(scip, cons).resvar;
    api::release_var(scip, &mut resvar)?;

    Ok(())
}

/// `addCoef`: adds coefficient to AND-constraint.
fn add_coef(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, mut var: VarId) -> ScipResult {
    // Are we in the transformed problem?
    let transformed = scip.conss[cons].is_transformed();

    // Always use transformed variables in transformed constraints.
    if transformed {
        var = api::get_transformed_var(scip, var)?;
    }

    let n = cd(scip, cons).nvars;
    consdata_ensure_vars_size(scip, cons, n + 1);
    {
        let c = cdm(scip, cons);
        c.vars[c.nvars] = var;
        c.nvars += 1;
        c.sorted = c.nvars == 1;
        c.changed = true;
        c.merged = false;
    }

    // Capture variable.
    crate::var::var_capture(scip, var);

    // If we are in transformed problem, catch the variable's events.
    if transformed {
        // Catch bound change events of variable.
        api::catch_var_event(scip, var, OPERATOR_EVENTS, eventhdlr, cons, None)?;
    }

    // Install the rounding locks for the new variable.
    lock_rounding(scip, cons, var)?;

    // @todo update LP rows
    if !cd(scip, cons).rows.is_empty() {
        log::error!("cannot add coefficients to AND-constraint after LP relaxation was created");
        return Err(Retcode::InvalidCall);
    }

    Ok(())
}

/// `delCoefPos`: deletes coefficient at given position from AND-constraint
/// data.
fn del_coef_pos(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    // Remove the rounding locks of the variable.
    let var = cd(scip, cons).vars[pos];
    unlock_rounding(scip, cons, var)?;

    if scip.conss[cons].is_transformed() {
        // Drop bound change events of variable.
        api::drop_var_event(scip, var, OPERATOR_EVENTS, eventhdlr, cons, -1)?;
    }

    if scip.conss[cons].is_transformed() {
        // If the position is watched, stop watching the position.
        if cd(scip, cons).watchedvar1 == pos {
            let w2 = cd(scip, cons).watchedvar2;
            consdata_switch_watchedvars(scip, cons, eventhdlr, w2, -1)?;
        }
        if cd(scip, cons).watchedvar2 == pos {
            let w1 = cd(scip, cons).watchedvar1;
            consdata_switch_watchedvars(scip, cons, eventhdlr, w1, -1)?;
        }
    }

    // Release variable.
    let mut v = var;
    api::release_var(scip, &mut v)?;

    // Move the last variable to the free slot.
    let c = cdm(scip, cons);
    c.vars[pos] = c.vars[c.nvars - 1];
    c.nvars -= 1;

    // If the last variable (that moved) was watched, update the watched
    // position.
    if c.watchedvar1 == c.nvars {
        c.watchedvar1 = pos;
    }
    if c.watchedvar2 == c.nvars {
        c.watchedvar2 = pos;
    }

    c.propagated = false;
    c.sorted = false;
    c.changed = true;

    Ok(())
}

/// `consdataSort`: sorts AND-constraint's variables by non-decreasing
/// variable index.
fn consdata_sort(scip: &mut Scip, cons: ConsId) {
    if cd(scip, cons).sorted {
        return;
    }

    if cd(scip, cons).nvars <= 1 {
        cdm(scip, cons).sorted = true;
        return;
    }

    let mut var1 = NONE;
    let mut var2 = NONE;

    // Remember watch variables.
    {
        let c = cdm(scip, cons);
        if c.watchedvar1 != -1 {
            var1 = c.vars[c.watchedvar1];
            c.watchedvar1 = -1;
            if c.watchedvar2 != -1 {
                var2 = c.vars[c.watchedvar2];
                c.watchedvar2 = -1;
            }
        }
    }

    // Sort variables after index.
    let mut vars = std::mem::take(&mut cdm(scip, cons).vars);
    let n = cd(scip, cons).nvars as usize;
    crate::sort::sort(&mut vars.0[..n], (), |&a, &b| crate::var::var_comp(scip, a, b));

    // Correct watched variables.
    if var1 != NONE {
        let find = |v: VarId| match crate::sort::sortedvec_find(&vars.0[..n], |&a, &b| crate::var::var_comp(scip, a, b), v) {
            Ok(p) | Err(p) => p as i32,
        };
        let pos1 = find(var1);
        let pos2 = if var2 != NONE { Some(find(var2)) } else { None };
        let c = cdm(scip, cons);
        c.watchedvar1 = pos1;
        if let Some(p) = pos2 {
            c.watchedvar2 = p;
        }
    }

    let c = cdm(scip, cons);
    c.vars = vars;
    c.sorted = true;
}

/// `applyFixings`: deletes all one-fixed variables.
fn apply_fixings(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, nchgcoefs: &mut i32) -> ScipResult {
    let mut v = 0;
    while v < cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];

        if scip.vars[var].glbdom.lb > 0.5 {
            del_coef_pos(scip, cons, eventhdlr, v)?;
            *nchgcoefs += 1;
        } else {
            // Get binary representative of variable.
            let (repvar, _negated) = scip_var::get_binvar_representative(scip, var)?;

            // Check, if the variable should be replaced with the
            // representative.
            if repvar != var {
                // Delete old (aggregated) variable.
                del_coef_pos(scip, cons, eventhdlr, v)?;

                // Add representative instead.
                add_coef(scip, cons, eventhdlr, repvar)?;
            } else {
                v += 1;
            }
        }
    }

    Ok(())
}

/// `createRelaxation`: creates a linearization of the AND-constraint.
fn create_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let (nvars, resvar, vars) = {
        let c = cd(scip, cons);
        (c.nvars, c.resvar, c.vars.0[..c.nvars as usize].to_vec())
    };
    let name = scip.conss[cons].name.clone();
    let (local, modifiable, removable) = (scip.conss[cons].local, scip.conss[cons].modifiable, scip.conss[cons].removable);
    let infinity = scip.set.infinity();

    // Creates LP rows corresponding to AND-constraint:
    //   - one additional row:             resvar - v1 - ... - vn >= 1-n
    //   - for each operator variable vi:  resvar - vi            <= 0
    let mut rows = Vec::with_capacity(nvars as usize + 1);

    // Create additional row.
    let row = scip_lp::create_empty_row_cons(scip, cons, &format!("{name}_add"), -nvars as f64 + 1.0, infinity, local, modifiable, removable)?;
    api::add_var_to_row(scip, row, resvar, 1.0)?;
    scip_lp::add_vars_to_row_same_coef(scip, row, &vars, -1.0)?;
    rows.push(row);

    // Create operator rows.
    for (i, &v) in vars.iter().enumerate() {
        let row = scip_lp::create_empty_row_cons(scip, cons, &format!("{name}_{i}"), -infinity, 0.0, local, modifiable, removable)?;
        api::add_var_to_row(scip, row, resvar, 1.0)?;
        api::add_var_to_row(scip, row, v, -1.0)?;
        rows.push(row);
    }

    cdm(scip, cons).rows = rows;

    Ok(())
}

/// `SCIProwIsInLP`.
#[inline]
fn row_is_in_lp(scip: &Scip, row: crate::types::RowId) -> bool {
    scip.rows[row].lppos >= 0
}

/// `addRelaxation`: adds linear relaxation of AND-constraint to the LP;
/// returns whether an infeasibility was detected.
fn add_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    // In the root LP we only add the weaker relaxation which consists of two
    // rows:
    //   - one additional row:             resvar - v1 - ... - vn >= 1-n
    //   - aggregated row:               n*resvar - v1 - ... - vn <= 0.0
    //
    // during separation we separate the stronger relaxation which consists of
    // n+1 row:
    //   - one additional row:             resvar - v1 - ... - vn >= 1-n
    //   - for each operator variable vi:  resvar - vi            <= 0.0
    let mut infeasible = false;

    // Create the aggregated row.
    if cd(scip, cons).aggrrow == NONE {
        let name = format!("{}_operators", scip.conss[cons].name);
        let (local, modifiable, removable) = (scip.conss[cons].local, scip.conss[cons].modifiable, scip.conss[cons].removable);
        let infinity = scip.set.infinity();
        let (nvars, resvar, vars) = {
            let c = cd(scip, cons);
            (c.nvars, c.resvar, c.vars.0[..c.nvars as usize].to_vec())
        };
        let row = scip_lp::create_empty_row_cons(scip, cons, &name, -infinity, 0.0, local, modifiable, removable)?;
        api::add_var_to_row(scip, row, resvar, nvars as f64)?;
        scip_lp::add_vars_to_row_same_coef(scip, row, &vars, -1.0)?;
        cdm(scip, cons).aggrrow = row;
    }

    // Insert aggregated LP row as cut.
    let aggrrow = cd(scip, cons).aggrrow;
    if !row_is_in_lp(scip, aggrrow) {
        infeasible = scip_lp::add_row(scip, aggrrow, false)?;
    }

    if !infeasible {
        if cd(scip, cons).rows.is_empty() {
            // Create the n+1 row relaxation.
            create_relaxation(scip, cons)?;
        }

        // Add additional row.
        let row0 = cd(scip, cons).rows[0];
        if !row_is_in_lp(scip, row0) {
            infeasible = scip_lp::add_row(scip, row0, false)?;
        }
    }

    Ok(infeasible)
}

/// `addNlrow`: adds constraint as row to the NLP, if not added yet.
fn add_nlrow(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // Skip deactivated, redundant, or local constraints (the NLP does not allow
    // for local rows at the moment).
    let c = &scip.conss[cons];
    if !c.is_active() || !c.check || c.local {
        return Ok(());
    }

    if cd(scip, cons).nlrow == NONE {
        let (vars, resvar) = {
            let c = cd(scip, cons);
            (c.vars.0[..c.nvars as usize].to_vec(), c.resvar)
        };
        let mut varexprs = Vec::with_capacity(vars.len());
        for &v in &vars {
            varexprs.push(crate::expr_var::create_expr_var(scip, v, None, 0)?);
        }
        let mut expr = crate::expr_product::create_expr_product(scip, &varexprs, 1.0, None, 0)?;

        let name = scip.conss[cons].name.clone();
        let nlrow = crate::nlp::create_nl_row(scip, &name, 0.0, &[resvar], &[-1.0], expr, 0.0, 0.0, crate::expr::ExprCurv::Unknown)?;
        cdm(scip, cons).nlrow = nlrow;

        crate::expr::expr_release(scip, &mut expr)?;
        for mut e in varexprs {
            crate::expr::expr_release(scip, &mut e)?;
        }
    }

    let nlrow = cd(scip, cons).nlrow;
    if scip.nlrows[nlrow].nlpindex < 0 {
        crate::nlp::add_nl_row(scip, nlrow)?;
    }

    Ok(())
}

/// `checkCons`: checks AND-constraint for feasibility of given solution;
/// returns whether the constraint is violated.
fn check_cons(scip: &mut Scip, cons: ConsId, sol: SolId, checklprows: bool, printreason: bool) -> ScipResult<bool> {
    let mut violated = false;

    // Check whether we can skip this feasibility check, because all rows are in
    // the LP and do not have to be checked.
    let mut mustcheck = checklprows;
    mustcheck = mustcheck || cd(scip, cons).rows.is_empty();
    if !mustcheck {
        for &r in &cd(scip, cons).rows {
            mustcheck = !row_is_in_lp(scip, r);
            if mustcheck {
                break;
            }
        }
    }

    // Check feasibility of constraint if necessary.
    if mustcheck {
        let mut minsolval = 1.0;
        let mut sumsolval = 0.0;

        // Increase age of constraint; age is reset to zero, if a violation was
        // found only in case we are in enforcement.
        if sol == NONE {
            api::inc_cons_age(scip, cons)?;
        }

        // Evaluate operator variables.
        let (vars, resvar, nvars) = {
            let c = cd(scip, cons);
            (c.vars.0[..c.nvars as usize].to_vec(), c.resvar, c.nvars)
        };
        for &v in &vars {
            let solval = api::get_sol_val(scip, sol, v);

            if minsolval > solval {
                minsolval = solval;
            }

            sumsolval += solval;
        }

        // The resultant must be at most as large as every operator and at least
        // as large as one minus the sum of negated operators.
        let solval = api::get_sol_val(scip, sol, resvar);
        let a = solval - minsolval;
        let b = sumsolval - (nvars as f64 - 1.0 + solval);
        // MAX3(0.0, a, b)
        let m = if 0.0 >= a { 0.0 } else { a };
        let viol = if m >= b { m } else { b };

        if scip.set.is_feas_positive(viol) {
            violated = true;

            // Only reset constraint age if we are in enforcement.
            if sol == NONE {
                api::reset_cons_age(scip, cons);
            }

            if printreason {
                // Printing is not ported.
            }
        }

        // Update constraint violation in solution.
        if sol != NONE {
            api::update_sol_cons_violation(scip, sol, viol, viol);
        }
    }

    Ok(violated)
}

/// `separateCons`: separates given primal solution; returns `(separated,
/// cutoff)`.
fn separate_cons(scip: &mut Scip, cons: ConsId, sol: SolId) -> ScipResult<(bool, bool)> {
    let mut separated = false;

    // Create all necessary rows for the linear relaxation.
    if cd(scip, cons).rows.is_empty() {
        create_relaxation(scip, cons)?;
    }

    // Test all rows for feasibility and add infeasible rows.
    let rows = cd(scip, cons).rows.clone();
    for r in rows {
        if !row_is_in_lp(scip, r) {
            let feasibility = scip_lp::get_row_sol_feasibility(scip, r, sol);
            if scip.set.is_feas_negative(feasibility) {
                let cutoff = scip_lp::add_row(scip, r, false)?;
                if cutoff {
                    return Ok((separated, true));
                }
                separated = true;
            }
        }
    }

    Ok((separated, false))
}

/// `analyzeConflictOne`: analyzes conflicting TRUE assignment to resultant of
/// given constraint, and adds conflict constraint to problem.
fn analyze_conflict_one(scip: &mut Scip, cons: ConsId, falsepos: i32) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it turned
    // on.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis, and add resultant and single operand
    // variable to conflict candidate queue.
    api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

    let (resvar, var) = (cd(scip, cons).resvar, cd(scip, cons).vars[falsepos]);
    api::add_conflict_binvar(scip, resvar)?;
    api::add_conflict_binvar(scip, var)?;

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `analyzeConflictZero`: analyzes conflicting FALSE assignment to resultant
/// of given constraint, and adds conflict constraint to problem.
fn analyze_conflict_zero(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // applicable.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis, and add all variables of infeasible
    // constraint to conflict candidate queue.
    api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

    let resvar = cd(scip, cons).resvar;
    api::add_conflict_binvar(scip, resvar)?;
    for v in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];
        api::add_conflict_binvar(scip, var)?;
    }

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `consdataFixResultantZero`: tries to fix the given resultant to zero.
fn consdata_fix_resultant_zero(scip: &mut Scip, cons: ConsId, resvar: VarId, pos: i32, cutoff: &mut bool, nfixedvars: &mut i32) -> ScipResult {
    let (infeasible, tightened) = scip_var::infer_binvar_cons(scip, resvar, false, cons, Proprule::Rule1 as i32)?;

    if infeasible {
        // Use conflict analysis to get a conflict constraint out of the
        // conflicting assignment.
        analyze_conflict_one(scip, cons, pos)?;
        api::reset_cons_age(scip, cons);
        *cutoff = true;
    } else {
        scip_cons::del_cons_local(scip, cons)?;
        if tightened {
            api::reset_cons_age(scip, cons);
            *nfixedvars += 1;
        }
    }

    Ok(())
}

/// `consdataFixOperandsOne`: fix all operands to one.
fn consdata_fix_operands_one(scip: &mut Scip, cons: ConsId, vars: &[VarId], cutoff: &mut bool, nfixedvars: &mut i32) -> ScipResult {
    let mut v = 0;
    while v < vars.len() && !*cutoff {
        let (infeasible, tightened) = scip_var::infer_binvar_cons(scip, vars[v], true, cons, Proprule::Rule2 as i32)?;

        if infeasible {
            // Use conflict analysis to get a conflict constraint out of the
            // conflicting assignment.
            analyze_conflict_one(scip, cons, v as i32)?;
            api::reset_cons_age(scip, cons);
            *cutoff = true;
        } else if tightened {
            api::reset_cons_age(scip, cons);
            *nfixedvars += 1;
        }
        v += 1;
    }

    if !*cutoff {
        scip_cons::del_cons_local(scip, cons)?;
    }

    Ok(())
}

/// The flags of a constraint, as `SCIPconsIsInitial(cons)`, ... pass them.
fn cons_flags(scip: &Scip, cons: ConsId) -> ConsFlags {
    let c = &scip.conss[cons];
    ConsFlags {
        initial: c.initial,
        separate: c.separate,
        enforce: c.enforce,
        check: c.check,
        propagate: c.propagate,
        local: c.local,
        modifiable: c.modifiable,
        dynamic: c.dynamic,
        removable: c.removable,
        stickingatnode: c.stickingatnode,
    }
}

/// `consdataLinearize`: linearize AND-constraint due to a globally to zero
/// fixed resultant; that is, creates, adds, and releases a logicor constraint
/// and remove the AND-constraint globally.
///
/// Since the resultant is fixed to zero the AND-constraint collapses to
/// linear constraint of the form:
///
/// - sum_{i=0}^{n-1} v_i <= n-1
///
/// This can be transformed into a logicor constraint of the form
///
/// - sum_{i=0}^{n-1} ~v_i >= 1
fn consdata_linearize(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32, nupgdconss: &mut i32) -> ScipResult {
    let nvars = cd(scip, cons).nvars as usize;
    let consvars = cd(scip, cons).vars.0[..nvars].to_vec();
    let mut conscreated = false;
    let mut lincons = NONE;

    // Allocate memory for variables for updated constraint.
    let mut vars = vec![NONE; nvars];

    // If we only have two variables, we prefer a set packing constraint instead
    // of a logicor constraint.
    if nvars == 2 && !scip.conss[cons].modifiable {
        // Get active representation.
        for v in 0..nvars {
            vars[v] = scip_var::get_binvar_representative(scip, consvars[v])?.0;
        }

        // If one of the two operators is globally fixed to one it follows that
        // the other has to be zero.
        if scip.vars[vars[0]].glbdom.lb > 0.5 {
            let (infeasible, tightened) = scip_var::fix_var(scip, vars[1], 0.0)?;

            if infeasible {
                *cutoff = true;
            } else if tightened {
                *nfixedvars += 1;
            }
        } else if scip.vars[vars[1]].glbdom.lb > 0.5 {
            let (infeasible, tightened) = scip_var::fix_var(scip, vars[0], 0.0)?;

            if infeasible {
                *cutoff = true;
            } else if tightened {
                *nfixedvars += 1;
            }
        } else if scip.vars[vars[0]].glbdom.ub > 0.5 && scip.vars[vars[1]].glbdom.ub > 0.5 {
            // Create, add, and release the setppc constraint.
            let name = scip.conss[cons].name.clone();
            let flags = cons_flags(scip, cons);
            lincons = crate::cons_setppc::create_cons_setpack(scip, &name, &vars, &flags)?;

            conscreated = true;
        }
    } else {
        // Collect negated variables.
        for v in 0..nvars {
            vars[v] = scip_var::get_negated_var(scip, consvars[v])?;
        }

        // Create, add, and release the logicor constraint.
        let name = scip.conss[cons].name.clone();
        let flags = cons_flags(scip, cons);
        lincons = crate::cons_logicor::create_cons_logicor(scip, &name, &vars, &flags)?;

        conscreated = true;
    }

    if conscreated {
        // Add and release new constraint.
        scip_cons::add_cons_upgrade(scip, cons, &mut lincons)?;
        *nupgdconss += 1;
    }

    // Remove the AND-constraint globally.
    scip_cons::del_cons(scip, cons)?;

    Ok(())
}

/// `analyzeZeroResultant`: the resultant is fixed to zero; in case all except
/// one operator are fixed to TRUE the last operator has to fixed to FALSE.
fn analyze_zero_resultant(scip: &mut Scip, cons: ConsId, watchedvar1: i32, watchedvar2: i32, cutoff: &mut bool, nfixedvars: &mut i32) -> ScipResult {
    if watchedvar2 == -1 {
        let var = cd(scip, cons).vars[watchedvar1];
        let (infeasible, tightened) = scip_var::infer_binvar_cons(scip, var, false, cons, Proprule::Rule4 as i32)?;

        if infeasible {
            // Use conflict analysis to get a conflict constraint out of the
            // conflicting assignment.
            analyze_conflict_zero(scip, cons)?;
            api::reset_cons_age(scip, cons);
            *cutoff = true;
        } else {
            scip_cons::del_cons_local(scip, cons)?;
            if tightened {
                api::reset_cons_age(scip, cons);
                *nfixedvars += 1;
            }
        }
    }

    Ok(())
}

/// `mergeMultiples`: replaces multiple occurrences of variables; `entries`
/// stores whether two positions in constraints represent the same variable.
#[allow(clippy::too_many_arguments)]
fn merge_multiples(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, entries: &mut Vec<u8>, nfixedvars: &mut i32, nchgcoefs: &mut i32, _ndelconss: &mut i32) -> ScipResult {
    if cd(scip, cons).merged {
        return Ok(());
    }

    // Nothing to merge.
    if cd(scip, cons).nvars <= 1 {
        cdm(scip, cons).merged = true;
        return Ok(());
    }

    let nvars = cd(scip, cons).nvars;
    let nentries = api::get_n_vars(scip) as usize;
    if entries.len() < nentries {
        entries.resize(nentries, 0);
    }

    let probvar_of = |scip: &Scip, var: VarId| if crate::var::var_is_active(scip, var) { var } else { scip.vars[var].negatedvar };

    // Initialize entries array.
    for v in (0..nvars).rev() {
        let var = cd(scip, cons).vars[v];
        let probvar = probvar_of(scip, var);
        let probidx = scip.vars[probvar].probindex;

        // Var is not active yet.
        entries[probidx as usize] = 0;
    }

    // Search for multiple variables; scan from back to front because deletion
    // doesn't affect the order of the front variables.
    // @note don't reorder variables because we would loose the watched
    // variables and filter position inforamtion
    let mut v = nvars - 1;
    while v >= 0 {
        let var = cd(scip, cons).vars[v];
        let probvar = probvar_of(scip, var);
        let probidx = scip.vars[probvar].probindex as usize;
        let active = crate::var::var_is_active(scip, var);

        // If var occurs first time in constraint init entries array.
        if entries[probidx] == 0 {
            entries[probidx] = if active { 1 } else { 2 };
        }
        // If var occurs second time in constraint, first time it was not
        // negated.
        else if (entries[probidx] == 1 && active) || (entries[probidx] == 2 && !active) {
            // Delete the multiple variable.
            del_coef_pos(scip, cons, eventhdlr, v)?;
            *nchgcoefs += 1;
        } else {
            // Negation of the variable is already present in the constraint:
            // fix resultant to zero.
            let resvar = cd(scip, cons).resvar;
            let (_infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
            if fixed {
                *nfixedvars += 1;
            }

            scip_cons::del_cons(scip, cons)?;
            break;
        }
        v -= 1;
    }

    cdm(scip, cons).merged = true;

    Ok(())
}

/// `propagateCons`: propagates constraint with the following rules:
///  (1) v_i = FALSE                                  =>  r   = FALSE
///  (2) r   = TRUE                                   =>  v_i = TRUE for all i
///  (3) v_i = TRUE for all i                         =>  r   = TRUE
///  (4) r   = FALSE, v_i = TRUE for all i except j   =>  v_j = FALSE
///
/// additional if the resultant is fixed to zero during presolving or in the
/// root node (globally), then the AND-constraint is collapsed to a linear
/// (logicor) constraint of the form
/// -> sum_{i=0}^{n-1} ~v_i >= 1
fn propagate_cons(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, cutoff: &mut bool, nfixedvars: &mut i32, nupgdconss: &mut i32) -> ScipResult {
    let (resvar, vars, nvars) = {
        let c = cd(scip, cons);
        (c.resvar, c.vars.0[..c.nvars as usize].to_vec(), c.nvars as usize)
    };

    // Don't process the constraint, if none of the operator variables was
    // fixed to FALSE, and if the watched variables and the resultant weren't
    // fixed to any value since last propagation call.
    if cd(scip, cons).propagated {
        return Ok(());
    }

    // Increase age of constraint; age is reset to zero, if a conflict or a
    // propagation was found.
    if !api::in_repropagation(scip) {
        api::inc_cons_age(scip, cons)?;
    }

    // If one of the operator variables was fixed to FALSE, the resultant can
    // be fixed to FALSE (rule (1)).
    if !cd(scip, cons).nofixedzero {
        // Search for operator fixed to zero.
        let mut i = 0;
        while i < nvars && scip.vars[vars[i]].locdom.ub > 0.5 {
            i += 1;
        }
        if i < nvars {
            // Fix resultant to zero.
            consdata_fix_resultant_zero(scip, cons, resvar, i as i32, cutoff, nfixedvars)?;
        } else {
            cdm(scip, cons).nofixedzero = true;
        }
    }

    // Check if resultant variables is globally fixed to zero.
    if !api::in_probing(scip) && scip.conss[cons].nupgradelocks == 0 && scip.vars[resvar].glbdom.ub < 0.5 {
        consdata_linearize(scip, cons, cutoff, nfixedvars, nupgdconss)?;

        if *cutoff && api::get_depth(scip) > 0 {
            // We are done with solving since a global bound change was
            // infeasible.
            let root = api::get_root_node(scip);
            api::cutoff_node(scip, root)?;
        }

        return Ok(());
    }

    // If the resultant and at least one operand are locally fixed to zero, the
    // constraint is locally redundant.
    if scip.vars[resvar].locdom.ub < 0.5 && !cd(scip, cons).nofixedzero {
        scip_cons::del_cons_local(scip, cons)?;
        return Ok(());
    }

    // If resultant is fixed to TRUE, all operator variables can be fixed to
    // TRUE (rule (2)).
    if scip.vars[resvar].locdom.lb > 0.5 {
        // Fix operands to one.
        consdata_fix_operands_one(scip, cons, &vars, cutoff, nfixedvars)?;

        return Ok(());
    }

    // Rules (3) and (4) can only be applied, if we know all operator
    // variables.
    if scip.conss[cons].modifiable {
        return Ok(());
    }

    // Rules (3) and (4) cannot be applied, if we have at least two unfixed
    // variables left; that means, we only have to watch (i.e. capture events)
    // of two variables, and switch to other variables if these ones get fixed.
    let mut watchedvar1 = cd(scip, cons).watchedvar1;
    let mut watchedvar2 = cd(scip, cons).watchedvar2;

    // Check, if watched variables are still unfixed.
    if watchedvar1 != -1 && scip.vars[vars[watchedvar1 as usize]].locdom.lb > 0.5 {
        watchedvar1 = -1;
    }
    if watchedvar2 != -1 && scip.vars[vars[watchedvar2 as usize]].locdom.lb > 0.5 {
        watchedvar2 = -1;
    }

    // If only one watched variable is still unfixed, make it the first one.
    if watchedvar1 == -1 {
        watchedvar1 = watchedvar2;
        watchedvar2 = -1;
    }

    // If the watched variables are invalid (fixed), find new ones if existing.
    if watchedvar2 == -1 {
        for (i, &var) in vars.iter().enumerate() {
            if scip.vars[var].locdom.lb < 0.5 {
                if watchedvar1 == -1 {
                    watchedvar1 = i as i32;
                } else if watchedvar1 != i as i32 {
                    watchedvar2 = i as i32;
                    break;
                }
            }
        }
    }

    // If all variables are fixed to TRUE, the resultant can also be fixed to
    // TRUE (rule (3)).
    if watchedvar1 == -1 {
        let (infeasible, tightened) = scip_var::infer_binvar_cons(scip, resvar, true, cons, Proprule::Rule3 as i32)?;

        if infeasible {
            // Use conflict analysis to get a conflict constraint out of the
            // conflicting assignment.
            analyze_conflict_zero(scip, cons)?;
            api::reset_cons_age(scip, cons);
            *cutoff = true;
        } else {
            scip_cons::del_cons_local(scip, cons)?;
            if tightened {
                api::reset_cons_age(scip, cons);
                *nfixedvars += 1;
            }
        }

        return Ok(());
    }

    // If resultant is fixed to FALSE, and only one operator variable is not
    // fixed to TRUE, this operator variable can be fixed to FALSE (rule (4)).
    if watchedvar2 == -1 && scip.vars[resvar].locdom.ub < 0.5 {
        analyze_zero_resultant(scip, cons, watchedvar1, watchedvar2, cutoff, nfixedvars)?;

        return Ok(());
    }

    // Switch to the new watched variables.
    consdata_switch_watchedvars(scip, cons, eventhdlr, watchedvar1, watchedvar2)?;

    // Mark the constraint propagated if we have an unfixed resultant or are
    // not in probing, it is necessary that a fixed resulting in probing mode
    // does not lead to a propagated constraint, because the constraint upgrade
    // needs to be performed.
    let resvar = cd(scip, cons).resvar;
    let propagated = !api::in_probing(scip) || (scip.vars[resvar].locdom.lb < 0.5 && scip.vars[resvar].locdom.ub > 0.5);
    cdm(scip, cons).propagated = propagated;

    Ok(())
}

/// `resolvePropagation`: resolves a conflict on the given variable by
/// supplying the variables needed for applying the corresponding propagation
/// rule (see propagateCons()).
fn resolve_propagation(scip: &mut Scip, cons: ConsId, infervar: VarId, proprule: Proprule, bdchgidx: Option<&BdChgIdx>, result: &mut ScipResultCode) -> ScipResult {
    let (vars, resvar) = {
        let c = cd(scip, cons);
        (c.vars.0[..c.nvars as usize].to_vec(), c.resvar)
    };

    match proprule {
        Proprule::Rule1 => {
            // The resultant was inferred to FALSE, because one operand variable
            // was FALSE.
            for &v in &vars {
                if api::get_var_ub_at_index(scip, v, bdchgidx, false) < 0.5 {
                    api::add_conflict_binvar(scip, v)?;
                    break;
                }
            }
            *result = ScipResultCode::Success;
        }
        Proprule::Rule2 => {
            // The operand variable was inferred to TRUE, because the resultant
            // was TRUE.
            api::add_conflict_binvar(scip, resvar)?;
            *result = ScipResultCode::Success;
        }
        Proprule::Rule3 => {
            // The resultant was inferred to TRUE, because all operand variables
            // were TRUE.
            for &v in &vars {
                api::add_conflict_binvar(scip, v)?;
            }
            *result = ScipResultCode::Success;
        }
        Proprule::Rule4 => {
            // The operand variable was inferred to FALSE, because the resultant
            // was FALSE and all other operands were TRUE.
            api::add_conflict_binvar(scip, resvar)?;
            for &v in &vars {
                if v != infervar {
                    api::add_conflict_binvar(scip, v)?;
                }
            }
            *result = ScipResultCode::Success;
        }
        Proprule::Invalid => {
            log::error!("invalid inference information {} in AND-constraint <{}>", proprule as i32, scip.conss[cons].name);
            return Err(Retcode::InvalidData);
        }
    }

    Ok(())
}

fn nlocksup(scip: &Scip, var: VarId) -> i32 {
    crate::var::var_get_n_locks_up_type(scip, var, crate::types::LockType::Model)
}

fn nlocksdown(scip: &Scip, var: VarId) -> i32 {
    crate::var::var_get_n_locks_down_type(scip, var, crate::types::LockType::Model)
}

fn aggregated_obj(scip: &Scip, var: VarId) -> ScipResult<f64> {
    let mut obj = 0.0;
    crate::var::var_get_aggregated_obj(scip, var, &mut obj)?;
    Ok(obj)
}

/// `dualPresolve`: perform dual presolving on AND-constraints.
#[allow(clippy::too_many_arguments)]
fn dual_presolve(
    scip: &mut Scip,
    conss: &[ConsId],
    eventhdlr: EventhdlrId,
    entries: &mut Vec<u8>,
    cutoff: &mut bool,
    nfixedvars: &mut i32,
    naggrvars: &mut i32,
    nchgcoefs: &mut i32,
    ndelconss: &mut i32,
    nupgdconss: &mut i32,
    naddconss: &mut i32,
) -> ScipResult {
    if conss.is_empty() {
        return Ok(());
    }

    let mut c = conss.len() as isize - 1;
    while c >= 0 && !*cutoff {
        let cons = conss[c as usize];
        c -= 1;

        {
            let co = &scip.conss[cons];
            if !co.is_active() || !co.check || co.modifiable {
                continue;
            }
        }

        // Propagate constraint.
        propagate_cons(scip, cons, eventhdlr, cutoff, nfixedvars, nupgdconss)?;

        if !scip.conss[cons].is_active() {
            continue;
        }

        if *cutoff {
            break;
        }

        apply_fixings(scip, cons, eventhdlr, nchgcoefs)?;

        // Merge multiple occurances of variables or variables with their
        // negated variables.
        merge_multiples(scip, cons, eventhdlr, entries, nfixedvars, nchgcoefs, ndelconss)?;

        if !scip.conss[cons].is_active() {
            continue;
        }

        let vars = cd(scip, cons).vars.0[..cd(scip, cons).nvars as usize].to_vec();
        let nvars = vars.len();

        if nvars == 0 {
            continue;
        }

        let resvar = cd(scip, cons).resvar;

        // Dual presolving does not apply to a fixed resultant.
        if scip.vars[resvar].glbdom.ub < 0.5 {
            continue;
        }

        if nlocksup(scip, resvar) == 1 && nlocksdown(scip, resvar) == 1 {
            let mut posobjsum = 0.0;
            let mut maxobj = -scip.set.infinity();
            let mut maxpos: isize = -1;
            let oldnfixedvars = *nfixedvars;
            let oldnaggrvars = *naggrvars;

            let mut impoperands = Vec::new();

            // Collect important operands.
            for &var in vars.iter().rev() {
                if nlocksup(scip, var) == 1 && nlocksdown(scip, var) == 1 {
                    impoperands.push(var);

                    // Get aggregated objective value of active variable.
                    let obj = aggregated_obj(scip, var)?;

                    // Add up all positive objective values of operands which have
                    // exactly one lock in both directions.
                    if obj > 0.0 {
                        posobjsum += obj;
                    }

                    // Memorize maximal objective value of operands and its
                    // position.
                    if obj > maxobj {
                        maxpos = impoperands.len() as isize - 1;
                        maxobj = obj;
                    }
                }
            }

            // No dual fixable variables found.
            let nimpoperands = impoperands.len();
            if nimpoperands == 0 {
                continue;
            }

            // Get aggregated objective value of active variable.
            let resobj = aggregated_obj(scip, resvar)?;

            // Resultant contributes to the objective with a negative value.
            if scip.set.is_le(resobj, 0.0) {
                let poscontissmall = scip.set.is_le(posobjsum, resobj.abs());

                // If all variables are only locked by this constraint and the
                // resultants contribution more then compensates the positive
                // contribution, we can fix all variables to 1.
                if nimpoperands == nvars && poscontissmall {
                    let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 1.0)?;

                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }

                    let mut v = nvars as isize - 1;
                    while v >= 0 && !*cutoff {
                        let (infeasible, fixed) = scip_var::fix_var(scip, vars[v as usize], 1.0)?;

                        *cutoff = *cutoff || infeasible;
                        if fixed {
                            *nfixedvars += 1;
                        }
                        v -= 1;
                    }

                    scip_cons::del_cons(scip, cons)?;
                    *ndelconss += 1;
                } else {
                    let mut aggregationperformed = false;
                    let mut zerofix = false;

                    let mut v = nimpoperands as isize - 1;
                    while v >= 0 && !*cutoff {
                        let op = impoperands[v as usize];
                        v -= 1;

                        // Get aggregated objective value of active variable.
                        let obj = aggregated_obj(scip, op)?;

                        if scip.set.is_le(obj, 0.0) {
                            let (infeasible, fixed) = scip_var::fix_var(scip, op, 1.0)?;

                            *cutoff = *cutoff || infeasible;
                            if fixed {
                                *nfixedvars += 1;
                            }
                        } else if !poscontissmall {
                            scip_var::fix_var(scip, op, 0.0)?;

                            *nfixedvars += 1;
                            zerofix = true;
                        } else {
                            // Aggregate resultant to operand.
                            let (_infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, resvar, op, 1.0, -1.0, 0.0)?;

                            if aggregated {
                                // Note that we cannot remove the aggregated operand
                                // because we do not know the position.
                                *naggrvars += 1;

                                aggregationperformed = true;
                            }
                        }
                    }

                    // Did we aggregate the resultant, then we can decide the
                    // value to fix it on the (aggregated) objective value since it
                    // was a independant variable.
                    if aggregationperformed || zerofix {
                        let fixval = if zerofix {
                            0.0
                        } else {
                            // Get aggregated objective value of active variable,
                            // that might be changed.
                            let obj = aggregated_obj(scip, resvar)?;

                            if scip.set.is_negative(obj) { 1.0 } else { 0.0 }
                        };

                        if fixval < 0.5 || (*nfixedvars - oldnfixedvars + *naggrvars - oldnaggrvars) as usize == nvars {
                            scip_var::fix_var(scip, resvar, fixval)?;

                            *nfixedvars += 1;

                            scip_cons::del_cons(scip, cons)?;
                            *ndelconss += 1;
                        }
                    }
                }
            }
            // Resultant contributes to the objective with a positive value.
            else {
                let mut zerofix = false;

                // If the smallest possible contribution is negative, but does not
                // compensate the positive contribution of the resultant we need to
                // fix this variable to 0.
                if nimpoperands == nvars && scip.set.is_le(maxobj, 0.0) {
                    let fixval = if scip.set.is_le(maxobj.abs(), resobj) { 0.0 } else { 1.0 };

                    let (infeasible, fixed) = scip_var::fix_var(scip, impoperands[maxpos as usize], fixval)?;
                    zerofix = fixval < 0.5;

                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }
                }

                let mut v = nimpoperands as isize - 1;
                while v >= 0 && !*cutoff {
                    let op = impoperands[v as usize];

                    // Get aggregated objective value of active variable.
                    let obj = aggregated_obj(scip, op)?;

                    let (infeasible, fixed) = if scip.set.is_le(obj, 0.0) {
                        if v == maxpos {
                            v -= 1;
                            continue;
                        }

                        scip_var::fix_var(scip, op, 1.0)?
                    } else {
                        zerofix = true;
                        scip_var::fix_var(scip, op, 0.0)?
                    };

                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }
                    v -= 1;
                }

                // Iff we have fixed all variables, all variables needed to be
                // stored in the impoperands array.
                if (*nfixedvars - oldnfixedvars) as usize == nvars {
                    let (infeasible, fixed) = scip_var::fix_var(scip, resvar, if zerofix { 0.0 } else { 1.0 })?;

                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }

                    scip_cons::del_cons(scip, cons)?;
                    *ndelconss += 1;
                }
            }
        }
        // Resultant is lock by another constraint (handler), check for operands
        // with only one down- and uplock.
        else {
            let mut maxobj = -scip.set.infinity();
            let mut linearize = false;
            let mut zerofix = false;

            // Get aggregated objective value of active variable.
            let resobj = aggregated_obj(scip, resvar)?;

            let resobjispos = scip.set.is_gt(resobj, 0.0);

            // We can only aggregate when the objective contribution of the
            // resultant is less or equal to 0.
            if !resobjispos {
                let mut goodvarsfound = false;

                for v in (0..nvars).rev() {
                    let var = vars[v];

                    // Get aggregated objective value of active variable.
                    let obj = aggregated_obj(scip, var)?;

                    // All operands which are only locked by this constraint, the
                    // objective contribution is greater or equal to 0 can be
                    // aggregated to the resultant.
                    if nlocksup(scip, var) == 1 && nlocksdown(scip, var) == 1 {
                        if !scip.set.is_negative(obj) {
                            // Aggregate resultant to operand.
                            let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, resvar, var, 1.0, -1.0, 0.0)?;

                            if aggregated && scip.conss[cons].nupgradelocks == 0 {
                                *naggrvars += 1;

                                linearize = true;

                                // Delete redundant entry from constraint.
                                del_coef_pos(scip, cons, eventhdlr, v as i32)?;
                                *nchgcoefs += 1;
                            }

                            *cutoff = *cutoff || infeasible;
                        } else {
                            goodvarsfound = true;
                        }
                    }
                }

                // If we aggregated an operands with the resultant we can also fix
                // "good" independant operands to 1, since the correctness of
                // "resultant = 0 => at least one operand = 0" in enforced by that
                // aggregation without an aggregation we cannot fix these
                // variables since it might lead to infeasibility, e.g.
                //
                //   obj(x3) = -1
                //   r = x1 * x2 * x3
                //   r = 0
                //   x1 = 1
                //   x2 = 1
                if !*cutoff && goodvarsfound && linearize {
                    // Fix good variables to 1 (C reads the shortened vars array
                    // through its alias).
                    for v in (0..cd(scip, cons).nvars).rev() {
                        let var = cd(scip, cons).vars[v];

                        if nlocksup(scip, var) == 1 && nlocksdown(scip, var) == 1 {
                            let (_infeasible, fixed) = scip_var::fix_var(scip, var, 1.0)?;

                            if fixed {
                                *nfixedvars += 1;
                            }
                        }
                    }
                }
            }
            // If the downlocks of the resultant are only from this constraint and
            // the objective contribution is positive, we can try to fix operands.
            else if nlocksdown(scip, resvar) == 1 {
                let mut locksareone = true;
                let mut maxpos = -1;

                for v in (0..nvars).rev() {
                    let var = vars[v];

                    // Check if all resultants are only locked by this constraint.
                    locksareone = locksareone && (nlocksup(scip, var) == 1 && nlocksdown(scip, var) == 1);

                    // Get aggregated objective value of active variable.
                    let obj = aggregated_obj(scip, var)?;

                    // Memorize maximal objective value of operands and its
                    // position.
                    if obj > maxobj {
                        maxpos = v as isize;
                        maxobj = obj;
                    }

                    // All operands which are only locked by this constraint, the
                    // objective contribution is greater or equal to 0, and the
                    // absolute value of the contribution of the resultant exceeds
                    // can be eliminated and aggregated to the resultant.
                    if nlocksup(scip, var) == 1 && nlocksdown(scip, var) == 1 && scip.set.is_ge(obj, 0.0) {
                        let (infeasible, fixed) = scip_var::fix_var(scip, var, 0.0)?;

                        *cutoff = *cutoff || infeasible;
                        if fixed {
                            *nfixedvars += 1;
                        }

                        zerofix = true;
                    }
                }

                // If constraint is still active and all operands are only lock by
                // this constraint, we check if we can fix the worst (in objective
                // contribution) operand to zero.
                if !zerofix && locksareone && scip.set.is_ge(resobj, maxobj.abs()) {
                    // Objective contribution needs to be negative, otherwise, the
                    // variable should already be fixed to 0.
                    let (infeasible, fixed) = scip_var::fix_var(scip, vars[maxpos as usize], 0.0)?;

                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }

                    zerofix = true;
                }

                // Fix the resultant if one operand was fixed to zero and delete
                // the constraint.
                if zerofix {
                    let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;

                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }

                    scip_cons::del_cons(scip, cons)?;
                    *ndelconss += 1;
                }
            }

            // We have to linearize the constraint, otherwise we might get wrong
            // propagations, since due to aggregations a resultant fixed to zero
            // is already fulfilling the constraint, and we must not ensure that
            // some remaining operand needs to be 0.
            if linearize {
                let consresvar = cd(scip, cons).resvar;
                let vals = [1.0, -1.0];
                let consname = scip.conss[cons].name.clone();
                let flags = cons_flags(scip, cons);
                let infinity = scip.set.infinity();

                // Create operator linear constraints.
                let n = cd(scip, cons).nvars;
                for v in (0..n).rev() {
                    let consvars = [consresvar, cd(scip, cons).vars[v]];

                    let mut newcons = crate::cons_linear::create_cons_linear(
                        scip,
                        &format!("{consname}_{v}"),
                        &consvars,
                        &vals,
                        -infinity,
                        0.0,
                        flags.initial,
                        flags.separate,
                        flags.enforce,
                        flags.check,
                        flags.propagate,
                        flags.local,
                        flags.modifiable,
                        flags.dynamic,
                        flags.removable,
                        flags.stickingatnode,
                    )?;

                    // Add constraint.
                    scip_cons::add_cons(scip, newcons)?;
                    scip_cons::release_cons(scip, &mut newcons)?;
                }
                *naddconss += n;

                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            }
            // If only one operand is leftover, aggregate it to the resultant.
            else if cd(scip, cons).nvars == 1 {
                // Aggregate resultant to operand.
                let var0 = cd(scip, cons).vars[0];
                let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, resvar, var0, 1.0, -1.0, 0.0)?;

                if aggregated {
                    *naggrvars += 1;
                }

                *cutoff = *cutoff || infeasible;

                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            }

            // If no operand is leftover delete the constraint.
            if scip.conss[cons].is_active() && cd(scip, cons).nvars == 0 {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            }
        }
    }

    Ok(())
}

/// `SCIPvarGetProbvarBinary` on a copy: the active binary variable and
/// whether it is negated.
fn probvar_binary(scip: &Scip, var: VarId) -> ScipResult<(VarId, bool)> {
    let mut v = var;
    let mut negated = false;
    crate::var::var_get_probvar_binary(scip, &mut v, &mut negated)?;
    Ok((v, negated))
}

/// `cliquePresolve`:
///
/// 1. check if at least two operands or one operand and the resultant are in
///    one clique, if so, we can fix the resultant to zero and in the former
///    case we can also delete this constraint but we need to extract the
///    clique information as constraint
///
///    x == AND(y, z) and clique(y,z) => x = 0, delete constraint and create y + z <= 1
///    x == AND(y, z) and clique(x,y) => x = 0
///
///    special handled cases are:
///    - if the resultant is a negation of an operand, in that case we fix the
///      resultant to 0
///    - if the resultant is equal to an operand, we will linearize this
///      constraint by adding all necessary set-packing constraints like
///      resultant + ~operand <= 1 and delete the old constraint
///
///    x == AND(~x, y) => x = 0
///    x == AND(x, y)  => add x + ~y <= 1 and delete the constraint
///
/// 2. check if one operand is in a clique with the negation of all other
///    operands, this means we can aggregate this operand to the resultant
///
///    r == AND(x,y,z) and clique(x,~y) and clique(x,~z) => r == x
///
/// 3. check if the resultant and the negations of all operands are in a
///    clique
///
///    r == AND(x,y) and clique(r, ~x,~y) => upgrade the constraint to a
///    set-partitioning constraint r + ~x + ~y = 1
///
/// @note We removed also fixed variables and propagate them, and if only one
/// operand is remaining due to removal, we will aggregate the resultant with
/// this operand
#[allow(clippy::too_many_arguments)]
fn clique_presolve(
    scip: &mut Scip,
    cons: ConsId,
    eventhdlr: EventhdlrId,
    cutoff: &mut bool,
    nfixedvars: &mut i32,
    naggrvars: &mut i32,
    nchgcoefs: &mut i32,
    ndelconss: &mut i32,
    naddconss: &mut i32,
) -> ScipResult {
    if !scip.conss[cons].is_active() || scip.conss[cons].modifiable {
        return Ok(());
    }

    let mut nvars = cd(scip, cons).nvars;

    // Remove fixed variables to be able to ask for cliques.
    //
    // if an operand is fixed to 0 fix the resultant to 0 and delete the
    // constraint if an operand is fixed to 1 remove it from the constraint
    for v in (0..nvars).rev() {
        let var = cd(scip, cons).vars[v];

        if scip.vars[var].glbdom.lb > 0.5 {
            // Because we loop from back to front we can delete the entry in the
            // consdata structure.
            del_coef_pos(scip, cons, eventhdlr, v)?;
            *nchgcoefs += 1;

            continue;
        } else if scip.vars[var].glbdom.ub < 0.5 {
            let resvar = cd(scip, cons).resvar;
            let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
            *cutoff = *cutoff || infeasible;
            if fixed {
                *nfixedvars += 1;
            }

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;

            return Ok(());
        }
    }

    // If we deleted some operands constraint might be redundant.
    if cd(scip, cons).nvars < nvars {
        // All operands fixed to one were removed, so if no operand is left this
        // means we can fix the resultant to 1 too.
        if cd(scip, cons).nvars == 0 {
            let resvar = cd(scip, cons).resvar;
            let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 1.0)?;
            *cutoff = *cutoff || infeasible;
            if fixed {
                *nfixedvars += 1;
            }

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;

            return Ok(());
        }
        // If only one not fixed operand is left, we can aggregate it to the
        // resultant.
        else if cd(scip, cons).nvars == 1 {
            // Aggregate resultant to last operand.
            let (resvar, var0) = (cd(scip, cons).resvar, cd(scip, cons).vars[0]);
            let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, resvar, var0, 1.0, -1.0, 0.0)?;

            if aggregated {
                *naggrvars += 1;
            }

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;

            *cutoff = *cutoff || infeasible;

            return Ok(());
        }

        nvars = cd(scip, cons).nvars;
    }

    let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
    let flags = cons_flags(scip, cons);
    let consname = scip.conss[cons].name.clone();

    // @todo when cliques are improved, we only need to collect all clique-ids
    // for all variables and check for doubled entries
    // Case 1 first part: check if two operands are in a clique.
    if scip.conss[cons].nupgradelocks == 0 {
        for v in (1..nvars as usize).rev() {
            let (var1, negated1) = probvar_binary(scip, vars[v])?;
            let value1 = !negated1;

            for v2 in (0..v).rev() {
                let (var2, negated2) = probvar_binary(scip, vars[v2])?;
                let value2 = !negated2;

                // If both variables are negated of each other or the same, this
                // will be handled in applyFixings();
                // @note if both variables are the same, then
                // SCIPvarsHaveCommonClique() will return TRUE, so we better
                // continue
                if var1 == var2 {
                    continue;
                }

                if crate::var::vars_have_common_clique(scip, var1, value1, var2, value2, true) {
                    let resvar = cd(scip, cons).resvar;
                    let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
                    *cutoff = *cutoff || infeasible;
                    if fixed {
                        *nfixedvars += 1;
                    }

                    // Create clique constraint which lead to the last fixing.
                    let name = format!("{consname}_clq_{v2}");

                    let consvars = [
                        if value1 { var1 } else { scip_var::get_negated_var(scip, var1)? },
                        if value2 { var2 } else { scip_var::get_negated_var(scip, var2)? },
                    ];

                    let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &consvars, &flags)?;
                    scip_cons::add_cons_upgrade(scip, cons, &mut cliquecons)?;
                    *naddconss += 1;

                    scip_cons::del_cons(scip, cons)?;
                    *ndelconss += 1;

                    return Ok(());
                }
            }
        }
    }

    let resvar = cd(scip, cons).resvar;
    let (var1, negated1) = probvar_binary(scip, resvar)?;

    // It may appear that we have a fixed resultant.
    if scip.vars[var1].varstatus == VarStatus::Fixed {
        // Resultant is fixed to 1, so fix all operands to 1.
        if scip.vars[resvar].glbdom.lb > 0.5 {
            // Fix all operands to 1.
            let mut v = nvars as isize - 1;
            while v >= 0 && !*cutoff {
                let (infeasible, fixed) = scip_var::fix_var(scip, vars[v as usize], 1.0)?;
                *cutoff = *cutoff || infeasible;

                if fixed {
                    *nfixedvars += 1;
                }
                v -= 1;
            }

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
        }
        // The upgrade to a linear constraint because of the to 0 fixed
        // resultant we do in propagateCons().

        return Ok(());
    }

    let value1 = !negated1;

    // Case 1 second part: check if one operands is in a clique with the
    // resultant.
    for v in (0..nvars as usize).rev() {
        let (var2, negated2) = probvar_binary(scip, vars[v])?;
        let value2 = !negated2;

        // If both variables are negated of each other or the same, this will be
        // handled in applyFixings();
        // @note if both variables are the same, then SCIPvarsHaveCommonClique()
        // will return TRUE, so we better continue
        if var1 == var2 {
            // x1 == AND(~x1, x2 ...) => x1 = 0
            if value1 != value2 {
                let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
                *cutoff = *cutoff || infeasible;

                if fixed {
                    *nfixedvars += 1;
                }

                return Ok(());
            }
            // x1 == AND(x1, x2 ...) => delete constraint and create all
            // set-packing constraints x1 + ~x2 <= 1, x1 + ~... <= 1
            else if scip.conss[cons].nupgradelocks == 0 {
                let mut consvars = [resvar, NONE];

                for v2 in (0..nvars as usize).rev() {
                    let (var2, negated2) = probvar_binary(scip, vars[v2])?;

                    // If the active representations of the resultant and an
                    // operand are different then we need to extract this as a
                    // clique constraint
                    //
                    // if the active representations of the resultant and an
                    // operand are equal then the clique constraint would look
                    // like x1 + ~x1 <= 1, which is redundant
                    //
                    // if the active representations of the resultant and an
                    // operand are negated of each other then the clique
                    // constraint would look like x1 + x1 <= 1, which will lead to
                    // a fixation of the resultant later on
                    if var1 == var2 {
                        if value1 == negated2 {
                            let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
                            *cutoff = *cutoff || infeasible;

                            if fixed {
                                *nfixedvars += 1;
                            }

                            break;
                        }
                    } else {
                        consvars[1] = scip_var::get_negated_var(scip, vars[v2])?;

                        let name = format!("{consname}_clq_{v2}");

                        let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &consvars, &flags)?;
                        scip_cons::add_cons(scip, cliquecons)?;
                        scip_cons::release_cons(scip, &mut cliquecons)?;
                        *naddconss += 1;
                    }
                }

                // Delete old constraint.
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;

                return Ok(());
            }
            // Due to SCIPvarsHaveCommonClique() returns on two same variables
            // that they are in a clique, we need to handle it explicitly.
            else {
                continue;
            }
        }

        // Fix resultant in operand clique.
        if crate::var::vars_have_common_clique(scip, var1, value1, var2, value2, true) {
            let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
            *cutoff = *cutoff || infeasible;
            if fixed {
                *nfixedvars += 1;
            }

            return Ok(());
        }
    }

    if !scip.conss[cons].is_active() {
        return Ok(());
    }

    let mut v2: isize = -1;
    // Check which operands have a negated variable.
    let mut v = nvars as isize - 1;
    while v >= 0 {
        let (var1, _) = probvar_binary(scip, vars[v as usize])?;

        if scip.vars[var1].negatedvar == NONE {
            if v2 >= 0 {
                break;
            }
            v2 = v;
        }
        v -= 1;
    }

    let mut allnegoperandsexist = false;

    let (vstart, vend): (isize, isize) = if v2 == -1 {
        // All operands have a negated variable, so we will check for all
        // possible negated ciques.
        allnegoperandsexist = true;
        (nvars as isize - 1, 0)
    } else if v2 >= 0 && v == -1 {
        // Exactly one operands has no negated variable, so only this variable can
        // be in a clique with all other negations.
        (v2, v2)
    } else {
        // At least two operands have no negated variable, so there is no
        // possible clique with negated variables.
        (-1, 0)
    };

    // Case 2: check for negated cliques in the operands.
    let mut v = vstart;
    while v >= vend {
        let (var1, negated1) = probvar_binary(scip, vars[v as usize])?;
        let value1 = !negated1;

        let mut v2 = nvars as isize - 1;
        while v2 >= 0 {
            if v2 == v {
                v2 -= 1;
                continue;
            }

            let (var2, negated2) = probvar_binary(scip, vars[v2 as usize])?;

            // Invert flag, because we want to check var 1 against all negations
            // of the other variables.
            let value2 = negated2;

            // Due to SCIPvarsHaveCommonClique() returns on two same variables
            // that they are in a clique, we need to handle it explicitly.
            if var1 == var2 && value1 == value2 {
                let (infeasible, fixed) = scip_var::fix_var(scip, resvar, 0.0)?;
                *cutoff = *cutoff || infeasible;
                if fixed {
                    *nfixedvars += 1;
                }

                return Ok(());
            }

            // Due to SCIPvarsHaveCommonClique() returns on two negated variables
            // that they are not in a clique, we need to handle it explicitly.
            if var1 == var2 && value1 != value2 {
                v2 -= 1;
                continue;
            }

            if !crate::var::vars_have_common_clique(scip, var1, value1, var2, value2, true) {
                break;
            }
            v2 -= 1;
        }

        if v2 == -1 {
            let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, resvar, vars[v as usize], 1.0, -1.0, 0.0)?;
            *cutoff = *cutoff || infeasible;

            if aggregated {
                *naggrvars += 1;
            }

            return Ok(());
        }
        v -= 1;
    }

    // Case 3: check if the resultant and the negations of the operands are in a
    // clique, then we can upgrade this constraint to a set-partitioning
    // constraint.
    if allnegoperandsexist && scip.conss[cons].is_active() && scip.conss[cons].nupgradelocks == 0 {
        let n = nvars as usize;
        let mut newvars = vec![NONE; n + 1];
        let mut negations = vec![false; n + 1];

        (newvars[n], negations[n]) = probvar_binary(scip, resvar)?;

        // Get active variables.
        for v in (0..n).rev() {
            (newvars[v], negations[v]) = probvar_binary(scip, vars[v])?;
        }

        let mut upgrade = true;

        // The resultant is in a clique with the negations of all operands, due
        // to this AND-constraint; only check if the negations of all operands
        // are in a clique.
        let mut v = n as isize - 1;
        while v >= 0 && upgrade {
            for v2 in (0..v as usize).rev() {
                // The resultant need to be in a clique with the negations of all
                // operands.
                if !crate::var::vars_have_common_clique(scip, newvars[v as usize], negations[v as usize], newvars[v2], negations[v2], true) {
                    upgrade = false;
                    break;
                }
            }
            v -= 1;
        }

        // All variables are in a clique, so upgrade thi AND-constraint.
        if upgrade {
            // Get new constraint variables.
            if negations[n] {
                // Negation does not need to be existing, so
                // SCIPvarGetNegatedVar() cannot be called.
                newvars[n] = scip_var::get_negated_var(scip, newvars[n])?;
            }

            for v in (0..n).rev() {
                if !negations[v] {
                    // Negation does not need to be existing, so
                    // SCIPvarGetNegatedVar() cannot be called.
                    newvars[v] = scip_var::get_negated_var(scip, newvars[v])?;
                }
            }

            let name = format!("{consname}_clqeq");

            let mut cliquecons = crate::cons_setppc::create_cons_setpart(scip, &name, &newvars, &flags)?;
            scip_cons::add_cons_upgrade(scip, cons, &mut cliquecons)?;
            *naddconss += 1;

            // Delete old constraint.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
        }
    }

    Ok(())
}

/// The hash table operations of `detectRedundantConstraints`
/// (`hashGetKeyAndcons`, `hashKeyEqAndcons`, `hashKeyValAndcons`); C's
/// key-eq sorts both constraints, which are sorted before every query and
/// insertion here.
struct AndconsHash<'a> {
    scip: &'a Scip,
}

impl crate::hash::HashOps<ConsId> for AndconsHash<'_> {
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
/// constraints for possible redundancy and removes or changes constraint
/// accordingly; in contrast to removeRedundantConstraints(), it uses a hash
/// table.
fn detect_redundant_constraints(scip: &mut Scip, conss: &[ConsId], firstchange: &mut i32, cutoff: &mut bool, naggrvars: &mut i32, ndelconss: &mut i32) -> ScipResult {
    // Create a hash table for the constraint set.
    let mut hashtablesize = conss.len() as i32;
    hashtablesize = if hashtablesize >= HASHSIZE_ANDCONS { hashtablesize } else { HASHSIZE_ANDCONS };
    let mut hashtable = crate::hash::HashTable::<ConsId>::new(hashtablesize);

    *cutoff = false;

    // Check all constraints in the given set for redundancy.
    for &cons0 in conss {
        if !scip.conss[cons0].is_active() || scip.conss[cons0].modifiable {
            continue;
        }

        // Sort the constraint.
        consdata_sort(scip, cons0);

        // Get constraint from current hash table with same variables as cons0.
        let cons1 = hashtable.retrieve(&AndconsHash { scip }, &cons0);

        if let Some(cons1) = cons1 {
            let redundant;

            let (res0, res1) = (cd(scip, cons0).resvar, cd(scip, cons1).resvar);
            if res0 != res1 {
                // Aggregate resultants.
                let (infeasible, red, aggregated) = scip_var::aggregate_vars(scip, res0, res1, 1.0, -1.0, 0.0)?;
                *cutoff = infeasible;
                redundant = red;

                if aggregated {
                    *naggrvars += 1;
                }
                if *cutoff {
                    return Ok(());
                }
            } else {
                redundant = true;
            }

            // Delete consdel.
            if redundant {
                // Update flags of constraint which caused the redundancy s.t.
                // nonredundant information doesn't get lost.
                scip_cons::update_cons_flags(scip, cons1, cons0)?;

                // Delete constraint.
                scip_cons::del_cons(scip, cons0)?;
                *ndelconss += 1;
            }

            // Update the first changed constraint to begin the next aggregation
            // round with.
            if cd(scip, cons0).changed && scip.conss[cons1].consspos < *firstchange {
                *firstchange = scip.conss[cons1].consspos;
            }
        } else {
            // No such constraint in current hash table: insert cons0 into hash
            // table.
            hashtable.insert(&AndconsHash { scip }, cons0);
        }
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce constraints.
fn enforce_constraint(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let enforcecuts = hd(scip, conshdlr).enforcecuts;

    *result = ScipResultCode::Feasible;

    // Method is called only for integral solutions, because the enforcing
    // priority is negative.
    for &cons in conss {
        let violated = check_cons(scip, cons, sol, false, false)?;
        if !violated {
            continue;
        }

        if !enforcecuts {
            *result = ScipResultCode::Infeasible;
            return Ok(());
        }

        let (separated, cutoff) = separate_cons(scip, cons, sol)?;
        if cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        } else if separated {
            *result = ScipResultCode::Separated;
        } else if *result == ScipResultCode::Feasible {
            // Do not change result separated to infeasible.
            *result = ScipResultCode::Infeasible;
        }
    }

    Ok(())
}

/// `preprocessConstraintPairs`: compares constraint with all prior
/// constraints for possible redundancy or aggregation, and removes or changes
/// constraint accordingly.
#[allow(clippy::too_many_arguments)]
fn preprocess_constraint_pairs(
    scip: &mut Scip,
    conss: &[ConsId],
    firstchange: i32,
    chkind: i32,
    cutoff: &mut bool,
    naggrvars: &mut i32,
    nbdchgs: &mut i32,
    ndelconss: &mut i32,
) -> ScipResult {
    // Get the constraint to be checked against all prior constraints.
    let cons0 = conss[chkind as usize];

    // Sort the constraint.
    consdata_sort(scip, cons0);

    // Check constraint against all prior constraints.
    let cons0changed = cd(scip, cons0).changed;

    if scip.conss[cons0].is_active() {
        let mut c = if cons0changed { 0 } else { firstchange };
        while c < chkind && !*cutoff {
            if c % 1000 == 0 && api::is_stopped(scip) {
                break;
            }

            let cons1 = conss[c as usize];
            c += 1;

            // Ignore inactive and modifiable constraints.
            if !scip.conss[cons1].is_active() || scip.conss[cons1].modifiable {
                continue;
            }

            // If both constraints were not changed since last round, we can
            // ignore the pair.
            if !cons0changed && !cd(scip, cons1).changed {
                continue;
            }

            // Sort the constraint.
            consdata_sort(scip, cons1);

            // Check consdata0 against consdata1:
            // - if they consist of the same operands, the resultants can be
            //   aggregated
            // - if one operand list is a subset of the other, add implication
            //   r0 = 1 -> r1 = 1, or r1 = 1 -> r0 = 1
            let (mut cons0superset, mut cons1superset) = (true, true);
            {
                let (c0, c1) = (cd(scip, cons0), cd(scip, cons1));
                let (mut v0, mut v1) = (0, 0);
                while (v0 < c0.nvars || v1 < c1.nvars) && (cons0superset || cons1superset) {
                    // Test, if variable appears in only one or in both
                    // constraints.
                    let varcmp = if v0 < c0.nvars && v1 < c1.nvars {
                        crate::var::var_compare(scip, c0.vars[v0], c1.vars[v1])
                    } else if v0 < c0.nvars {
                        -1
                    } else {
                        1
                    };

                    match varcmp {
                        -1 => {
                            // Variable doesn't appear in consdata1.
                            cons1superset = false;
                            v0 += 1;
                        }
                        1 => {
                            // Variable doesn't appear in consdata0.
                            cons0superset = false;
                            v1 += 1;
                        }
                        0 => {
                            // Variable appears in both constraints.
                            v0 += 1;
                            v1 += 1;
                        }
                        _ => {
                            log::error!("invalid comparison result");
                            return Err(Retcode::InvalidData);
                        }
                    }
                }
            }

            let (res0, res1) = (cd(scip, cons0).resvar, cd(scip, cons1).resvar);

            // Check for equivalence and domination.
            if cons0superset && cons1superset {
                // Constraints are equivalent: aggregate resultants.
                let (infeasible, redundant, aggregated) = scip_var::aggregate_vars(scip, res0, res1, 1.0, -1.0, 0.0)?;

                if aggregated {
                    *naggrvars += 1;
                }

                if redundant {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons0, cons1)?;

                    // Delete constraint.
                    scip_cons::del_cons(scip, cons1)?;
                    *ndelconss += 1;
                }

                *cutoff = *cutoff || infeasible;
            } else if cons0superset {
                // The conjunction of cons0 is a superset of the conjunction of
                // cons1: add implication.
                let (infeasible, nboundchgs) = scip_var::add_var_implication(scip, res0, true, res1, BoundType::Lower, 1.0)?;
                *cutoff = *cutoff || infeasible;
                *nbdchgs += nboundchgs;
            } else if cons1superset {
                // The conjunction of cons1 is a superset of the conjunction of
                // cons0: add implication.
                let (infeasible, nboundchgs) = scip_var::add_var_implication(scip, res1, true, res0, BoundType::Lower, 1.0)?;
                *cutoff = *cutoff || infeasible;
                *nbdchgs += nboundchgs;
            }
        }
    }
    cdm(scip, cons0).changed = false;

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `conshdlrCopyAnd`: copy method for constraint handler plugins.
fn conshdlr_copy_and(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_and(scip)?;
    *valid = true;
    Ok(())
}

/// `consFreeAnd`: destructor of constraint handler to free constraint
/// handler data.
fn cons_free_and(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consInitpreAnd`: presolving initialization method of constraint
/// handler.
fn cons_initpre_and(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let (linearize, aggrlinearization) = (hd(scip, conshdlr).linearize, hd(scip, conshdlr).aggrlinearization);

    if linearize {
        // Linearize all AND-constraints and remove the AND-constraints.
        let infinity = scip.set.infinity();
        for &cons in conss {
            // Only added constraints can be upgraded.
            if !scip.conss[cons].is_added() || scip.conss[cons].nupgradelocks >= 1 {
                continue;
            }

            let (nvars, consvars, resvar) = {
                let c = cd(scip, cons);
                (c.nvars as usize, c.vars.0[..c.nvars as usize].to_vec(), c.resvar)
            };
            let consname = scip.conss[cons].name.clone();
            let f = cons_flags(scip, cons);
            let add_linear = |scip: &mut Scip, name: String, vars: &[VarId], vals: &[f64], lhs: f64, rhs: f64| -> ScipResult {
                let mut newcons = crate::cons_linear::create_cons_linear(
                    scip,
                    &name,
                    vars,
                    vals,
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

                // Add constraint.
                scip_cons::add_cons(scip, newcons)?;
                scip_cons::release_cons(scip, &mut newcons)
            };

            if !aggrlinearization {
                // Create operator linear constraints.
                for (v, &var) in consvars.iter().enumerate() {
                    add_linear(scip, format!("{consname}_{v}"), &[resvar, var], &[1.0, -1.0], -infinity, 0.0)?;
                }
            }

            let mut vars = consvars.clone();
            vars.push(resvar);
            let mut vals = vec![-1.0; nvars + 1];

            if aggrlinearization {
                // Create additional linear constraint.
                vals[nvars] = nvars as f64;
                add_linear(scip, format!("{consname}_operators"), &vars, &vals, -infinity, 0.0)?;
            }

            // Create additional linear constraint.
            vals[nvars] = 1.0;
            add_linear(scip, format!("{consname}_add"), &vars, &vals, -(nvars as f64) + 1.0, infinity)?;

            // Delete constraint.
            scip_cons::del_cons(scip, cons)?;
        }
    }

    Ok(())
}

/// `consInitsolAnd`: solving process initialization method of constraint
/// handler.
fn cons_initsol_and(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    // Add nlrow representation to NLP, if NLP had been constructed.
    if crate::nlp::is_nlp_constructed(scip) {
        for &cons in conss {
            add_nlrow(scip, cons)?;
        }
    }

    Ok(())
}

/// `consExitsolAnd`: solving process deinitialization method of constraint
/// handler.
fn cons_exitsol_and(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], _restart: bool) -> ScipResult {
    // Release and free the rows and nlrow of all constraints.
    for &cons in conss {
        consdata_free_rows(scip, cons)?;

        let mut nlrow = cd(scip, cons).nlrow;
        if nlrow != NONE {
            crate::nlp::release_nl_row(scip, &mut nlrow)?;
            cdm(scip, cons).nlrow = NONE;
        }
    }

    Ok(())
}

/// `consDeleteAnd`: frees specific constraint data; the data is put back on
/// the constraint while it is freed.
fn cons_delete_and(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    scip.conss[cons].consdata = consdata.take();
    let ret = consdata_free(scip, cons, eventhdlr);
    scip.conss[cons].consdata = None;

    ret
}

/// `consTransAnd`: transforms constraint data into data belonging to the
/// transformed problem.
fn cons_trans_and(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let (vars, resvar) = {
        let s = cd(scip, sourcecons);
        (s.vars.0[..s.nvars as usize].to_vec(), s.resvar)
    };

    // Create target constraint data.
    let targetdata = consdata_create(scip, &vars, resvar)?;

    // Create target constraint.
    let name = scip.conss[sourcecons].name.clone();
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    // Catch needed events on variables (C does it in consdataCreate).
    consdata_catch_events(scip, *targetcons, eventhdlr)
}

/// `consInitlpAnd`: LP initialization method of constraint handler.
fn cons_initlp_and(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let mut i = 0;
    while i < conss.len() && !*infeasible {
        *infeasible = add_relaxation(scip, conss[i])?;
        i += 1;
    }

    Ok(())
}

/// The shared body of `consSepalpAnd` and `consSepasolAnd`.
fn sepa_and(scip: &mut Scip, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    // Separate all useful constraints.
    for &cons in &conss[..nusefulconss as usize] {
        let (separated, cutoff) = separate_cons(scip, cons, sol)?;
        if cutoff {
            *result = ScipResultCode::Cutoff;
        } else if separated {
            *result = ScipResultCode::Separated;
        }
    }

    // Combine constraints to get more cuts.
    // @todo combine constraints to get further cuts

    Ok(())
}

/// `consSepalpAnd`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_and(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    sepa_and(scip, conss, nusefulconss, NONE, result)
}

/// `consSepasolAnd`: separation method of constraint handler for arbitrary
/// primal solutions.
fn cons_sepasol_and(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    sepa_and(scip, conss, nusefulconss, sol, result)
}

/// `consEnfolpAnd`: constraint enforcing method of constraint handler for LP
/// solutions.
fn cons_enfolp_and(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, NONE, result)
}

/// `consEnforelaxAnd`: constraint enforcing method of constraint handler for
/// relaxation solutions.
fn cons_enforelax_and(scip: &mut Scip, sol: SolId, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, sol, result)
}

/// `consEnfopsAnd`: constraint enforcing method of constraint handler for
/// pseudo solutions.
fn cons_enfops_and(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    // Method is called only for integral solutions, because the enforcing
    // priority is negative.
    for &cons in conss {
        if check_cons(scip, cons, NONE, true, false)? {
            *result = ScipResultCode::Infeasible;
            return Ok(());
        }
    }
    *result = ScipResultCode::Feasible;

    Ok(())
}

/// `consCheckAnd`: feasibility check method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_check_and(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    checklprows: bool,
    printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let mut i = 0;
    while i < conss.len() && (*result == ScipResultCode::Feasible || completely) {
        if check_cons(scip, conss[i], sol, checklprows, printreason)? {
            *result = ScipResultCode::Infeasible;
        }
        i += 1;
    }

    Ok(())
}

/// `consPropAnd`: domain propagation method of constraint handler.
fn cons_prop_and(
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
    let mut nfixedvars = 0;
    let mut nupgdconss = 0;

    // Propagate all useful constraints.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff {
        propagate_cons(scip, conss[c], eventhdlr, &mut cutoff, &mut nfixedvars, &mut nupgdconss)?;
        c += 1;
    }

    // Return the correct result.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if nfixedvars > 0 || nupgdconss > 0 {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consPresolAnd`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_and(
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

    let oldnfixedvars = counts.nfixedvars;
    let mut oldnaggrvars = counts.naggrvars;
    let mut oldnchgbds = counts.nchgbds;
    let mut oldndelconss = counts.ndelconss;
    let oldnupgdconss = counts.nupgdconss;

    let nentries = (api::get_n_vars(scip) - api::get_n_cont_vars(scip)).max(0) as usize;
    let mut entries = vec![0u8; nentries];

    let h = hd(scip, conshdlr);
    let (eventhdlr, dualpresolving, presolusehashing, presolpairwise) = (h.eventhdlr, h.dualpresolving, h.presolusehashing, h.presolpairwise);

    // Process constraints.
    let mut cutoff = false;
    let mut firstchange = i32::MAX;
    let mut c = 0;
    while c < nconss && !cutoff && (c % 1000 != 0 || !api::is_stopped(scip)) {
        let cons = conss[c as usize];

        // Force presolving the constraint in the initial round.
        if nrounds == 0 {
            cdm(scip, cons).propagated = false;
        }

        // Remember the first changed constraint to begin the next aggregation
        // round with.
        if firstchange == i32::MAX && cd(scip, cons).changed {
            firstchange = c;
        }

        // Propagate constraint.
        propagate_cons(scip, cons, eventhdlr, &mut cutoff, &mut counts.nfixedvars, &mut counts.nupgdconss)?;

        // Remove all variables that are fixed to one; merge multiple entries of
        // the same variable; fix resultant to zero if a pair of negated
        // variables is contained in the operand variables.
        if !cutoff && !scip.conss[cons].deleted {
            apply_fixings(scip, cons, eventhdlr, &mut counts.nchgcoefs)?;

            // Merge multiple occurances of variables or variables with their
            // negated variables.
            merge_multiples(scip, cons, eventhdlr, &mut entries, &mut counts.nfixedvars, &mut counts.nchgcoefs, &mut counts.ndelconss)?;
        }

        if !cutoff && !scip.conss[cons].deleted && !scip.conss[cons].modifiable {
            // If only one variable is left, the resultant has to be equal to this
            // single variable.
            if cd(scip, cons).nvars == 1 {
                // Aggregate variables: resultant - operand == 0.
                let (resvar, var0) = (cd(scip, cons).resvar, cd(scip, cons).vars[0]);
                let (infeasible, redundant, aggregated) = scip_var::aggregate_vars(scip, resvar, var0, 1.0, -1.0, 0.0)?;
                cutoff = infeasible;

                if aggregated {
                    counts.naggrvars += 1;
                }

                if redundant {
                    // Delete constraint.
                    scip_cons::del_cons(scip, cons)?;
                    counts.ndelconss += 1;
                }
            } else if !cd(scip, cons).impladded {
                // Add implications: resultant == 1 -> all operands == 1.
                let mut i = 0;
                while i < cd(scip, cons).nvars && !cutoff {
                    let (resvar, var) = (cd(scip, cons).resvar, cd(scip, cons).vars[i]);
                    let (infeasible, nimplbdchgs) = scip_var::add_var_implication(scip, resvar, true, var, BoundType::Lower, 1.0)?;
                    cutoff = infeasible;
                    counts.nchgbds += nimplbdchgs;
                    i += 1;
                }
                cdm(scip, cons).impladded = true;
            }

            // If in r = x and y, the resultant is fixed to zero, add implication
            // x = 1 -> y = 0.
            if !cutoff && scip.conss[cons].is_active() && cd(scip, cons).nvars == 2 && !cd(scip, cons).opimpladded && scip.vars[cd(scip, cons).resvar].glbdom.ub < 0.5 {
                let (var0, var1) = (cd(scip, cons).vars[0], cd(scip, cons).vars[1]);
                let (infeasible, nimplbdchgs) = scip_var::add_var_implication(scip, var0, true, var1, BoundType::Upper, 0.0)?;
                cutoff = infeasible;
                counts.nchgbds += nimplbdchgs;
                cdm(scip, cons).opimpladded = true;
            }
        }
        c += 1;
    }

    // Perform dual presolving on AND-constraints.
    if dualpresolving && !cutoff && !api::is_stopped(scip) && scip_var::allow_strong_dual_reds(scip) {
        dual_presolve(
            scip,
            conss,
            eventhdlr,
            &mut entries,
            &mut cutoff,
            &mut counts.nfixedvars,
            &mut counts.naggrvars,
            &mut counts.nchgcoefs,
            &mut counts.ndelconss,
            &mut counts.nupgdconss,
            &mut counts.naddconss,
        )?;
    }

    // Check for cliques inside the AND constraint.
    if (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
        let mut c = 0;
        while c < conss.len() && !cutoff && !api::is_stopped(scip) {
            let cons = conss[c];
            c += 1;

            if !scip.conss[cons].is_active() {
                continue;
            }

            // cliquePresolve() may aggregate variables which need to be removed
            // from other constraints, we also need to make sure that we remove
            // fixed variables by calling propagateCons() to make sure that
            // applyFixing() and mergeMultiples() work.
            propagate_cons(scip, cons, eventhdlr, &mut cutoff, &mut counts.nfixedvars, &mut counts.nupgdconss)?;

            if !cutoff && !scip.conss[cons].deleted {
                // Remove all variables that are fixed to one; merge multiple
                // entries of the same variable; fix resultant to zero if a pair
                // of negated variables is contained in the operand variables.
                apply_fixings(scip, cons, eventhdlr, &mut counts.nchgcoefs)?;
                merge_multiples(scip, cons, eventhdlr, &mut entries, &mut counts.nfixedvars, &mut counts.nchgcoefs, &mut counts.ndelconss)?;

                // Check if at least two operands are in one clique.
                clique_presolve(
                    scip,
                    cons,
                    eventhdlr,
                    &mut cutoff,
                    &mut counts.nfixedvars,
                    &mut counts.naggrvars,
                    &mut counts.nchgcoefs,
                    &mut counts.ndelconss,
                    &mut counts.naddconss,
                )?;
            }
        }
    }

    // Process pairs of constraints: check them for equal operands in order to
    // aggregate resultants; only apply this expensive procedure, if the single
    // constraint preprocessing did not find any reductions (otherwise, we delay
    // the presolving to be called again next time).
    if !cutoff
        && presolusehashing
        && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0
        && counts.nfixedvars == oldnfixedvars
        && counts.naggrvars == oldnaggrvars
        && firstchange < nconss
    {
        // Detect redundant constraints; fast version with hash table instead of
        // pairwise comparison.
        detect_redundant_constraints(scip, conss, &mut firstchange, &mut cutoff, &mut counts.naggrvars, &mut counts.ndelconss)?;
        oldnaggrvars = counts.naggrvars;
    }

    if !cutoff && presolpairwise && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 && counts.nfixedvars == oldnfixedvars && counts.naggrvars == oldnaggrvars {
        let mut npaircomparisons: i64 = 0;
        oldndelconss = counts.ndelconss;

        let mut c = firstchange;
        while c < nconss && !cutoff && !api::is_stopped(scip) {
            let cons = conss[c as usize];
            if scip.conss[cons].is_active() && !scip.conss[cons].modifiable {
                npaircomparisons += if cd(scip, cons).changed { c as i64 } else { c as i64 - firstchange as i64 };

                preprocess_constraint_pairs(scip, conss, firstchange, c, &mut cutoff, &mut counts.naggrvars, &mut counts.nchgbds, &mut counts.ndelconss)?;

                if npaircomparisons > NMINCOMPARISONS as i64 {
                    if ((counts.ndelconss - oldndelconss) as f64 + (counts.naggrvars - oldnaggrvars) as f64 + (counts.nchgbds - oldnchgbds) as f64 / 2.0)
                        / (npaircomparisons as f64)
                        < MINGAINPERNMINCOMPARISONS
                    {
                        break;
                    }
                    oldndelconss = counts.ndelconss;
                    oldnaggrvars = counts.naggrvars;
                    oldnchgbds = counts.nchgbds;

                    npaircomparisons = 0;
                }
            }
            c += 1;
        }
    }

    // Return the correct result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if counts.nfixedvars > oldnfixedvars
        || counts.naggrvars > oldnaggrvars
        || counts.nchgbds > oldnchgbds
        || counts.ndelconss > oldndelconss
        || counts.nupgdconss > oldnupgdconss
    {
        ScipResultCode::Success
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consRespropAnd`: propagation conflict resolving method of constraint
/// handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_and(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    infervar: VarId,
    inferinfo: i32,
    _boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    resolve_propagation(scip, cons, infervar, Proprule::from_i32(inferinfo), bdchgidx.as_ref(), result)
}

/// `consLockAnd`: variable rounding lock method of constraint handler.
fn cons_lock_and(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: crate::types::LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    // Resultant variable.
    let resvar = cd(scip, cons).resvar;
    api::add_var_locks_type(scip, resvar, locktype, nlockspos + nlocksneg, nlockspos + nlocksneg)?;

    // Operand variables.
    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        api::add_var_locks_type(scip, var, locktype, nlockspos + nlocksneg, nlockspos + nlocksneg)?;
    }

    Ok(())
}

/// `consActiveAnd`: constraint activation notification method of constraint
/// handler.
fn cons_active_and(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    if scip.set.stage == Stage::Solving && crate::nlp::is_nlp_constructed(scip) {
        add_nlrow(scip, cons)?;
    }

    Ok(())
}

/// `consDeactiveAnd`: constraint deactivation notification method of
/// constraint handler.
fn cons_deactive_and(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    // Remove row from NLP, if still in solving if we are in exitsolve, the
    // whole NLP will be freed anyway.
    let nlrow = cd(scip, cons).nlrow;
    if scip.set.stage == Stage::Solving && nlrow != NONE {
        crate::nlp::del_nl_row(scip, nlrow)?;
    }

    Ok(())
}

/// `consCopyAnd`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_and(
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
    *valid = true;

    let sourceresvar = get_resultant_and(sourcescip, sourcecons);

    // Map resultant to active variable of the target SCIP.
    let (resvar, v) = crate::copy::get_var_copy(sourcescip, scip, sourceresvar, Some(maps), global)?;
    *valid = v;

    // We do not copy, if a variable is missing.
    if !*valid {
        return Ok(());
    }

    // Map operand variables to active variables of the target SCIP.
    let sourcevars = get_vars_and(sourcescip, sourcecons).to_vec();

    let mut vars = Vec::with_capacity(sourcevars.len());
    for &sv in &sourcevars {
        let (var, v) = crate::copy::get_var_copy(sourcescip, scip, sv, Some(maps), global)?;
        *valid = v;

        // We do not copy, if a variable is missing.
        if !*valid {
            return Ok(());
        }
        vars.push(var);
    }

    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);

    // Creates and captures a AND-constraint.
    *cons = create_cons_and(scip, &consname, resvar, &vars, flags)?;

    Ok(())
}

/// `consGetVarsAnd`: constraint method of constraint handler which returns
/// the variables (the callers size the array by `consGetNVarsAnd`).
fn cons_get_vars_and(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.extend_from_slice(&c.vars.0[..c.nvars as usize]);
    vars.push(c.resvar);
    *success = true;
    Ok(())
}

/// `consGetNVarsAnd`: constraint method of constraint handler which returns
/// the number of variables (if possible).
fn cons_get_n_vars_and(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = cd(scip, cons).nvars + 1;
    *success = true;
    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecAnd`: the event data is the constraint.
fn event_exec_and(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let c = cdm(scip, eventdata);

    // Check, if the variable was fixed to zero.
    if event.eventtype == EVENTTYPE_UBTIGHTENED {
        c.nofixedzero = false;
    }

    c.propagated = false;

    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrAnd`: creates the handler for AND-constraints and
/// includes it in SCIP.
pub fn include_conshdlr_and(scip: &mut Scip) -> ScipResult {
    use crate::paramset::paramset_add_bool;

    // Create event handler for events on variables.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_and, None)?;

    // Create constraint handler data.
    let conshdlrdata = ConshdlrData {
        eventhdlr,
        presolpairwise: false,
        presolusehashing: false,
        linearize: false,
        enforcecuts: false,
        aggrlinearization: false,
        upgrresultant: false,
        dualpresolving: false,
    };

    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_and),
        Some(cons_enfops_and),
        Some(cons_check_and),
        Some(cons_lock_and),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_and);
    h.conscopy = Some(cons_copy_and);
    h.consactive = Some(cons_active_and);
    h.consdeactive = Some(cons_deactive_and);
    h.consdelete = Some(cons_delete_and);
    h.consinitsol = Some(cons_initsol_and);
    h.consexitsol = Some(cons_exitsol_and);
    h.consfree = Some(cons_free_and);
    h.consgetvars = Some(cons_get_vars_and);
    h.consgetnvars = Some(cons_get_n_vars_and);
    h.consinitpre = Some(cons_initpre_and);
    h.consinitlp = Some(cons_initlp_and);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_and), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_and), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_and);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_and),
        Some(cons_sepasol_and),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_and);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_and);

    // Add AND-constraint handler parameters.
    macro_rules! t {
        ($f:ident) => {
            crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, $f)
        };
    }
    paramset_add_bool(
        scip,
        "constraints/and/presolpairwise",
        "should pairwise constraint comparison be performed in presolving?",
        t!(presolpairwise),
        true,
        DEFAULT_PRESOLPAIRWISE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/and/presolusehashing",
        "should hash table be used for detecting redundant constraints in advance",
        t!(presolusehashing),
        true,
        DEFAULT_PRESOLUSEHASHING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/and/linearize",
        "should the AND-constraint get linearized and removed (in presolving)?",
        t!(linearize),
        true,
        DEFAULT_LINEARIZE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/and/enforcecuts",
        "should cuts be separated during LP enforcing?",
        t!(enforcecuts),
        true,
        DEFAULT_ENFORCECUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/and/aggrlinearization",
        "should an aggregated linearization be used?",
        t!(aggrlinearization),
        true,
        DEFAULT_AGGRLINEARIZATION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/and/upgraderesultant",
        "should implied integrality of resultant variables be detected?",
        t!(upgrresultant),
        true,
        DEFAULT_UPGRRESULTANT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/and/dualpresolving",
        "should dual presolving be performed?",
        t!(dualpresolving),
        true,
        DEFAULT_DUALPRESOLVING,
        None,
    )?;

    Ok(())
}

/// `SCIPcreateConsAnd`: creates and captures an AND-constraint.
pub fn create_cons_and(scip: &mut Scip, name: &str, resvar: VarId, vars: &[VarId], flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the AND-constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        log::error!("AND-constraint handler not found");
        return Err(Retcode::PluginNotFound);
    }

    // Check whether resultant variable is binary.
    if !crate::var::var_is_binary(scip, resvar) {
        log::error!("resultant <{}> is not binary", scip.vars[resvar].name);
        return Err(Retcode::InvalidData);
    }

    // Check whether all variables are binary.
    for &v in vars {
        if !crate::var::var_is_binary(scip, v) {
            log::error!("operand <{}> is not binary", scip.vars[v].name);
            return Err(Retcode::InvalidData);
        }
    }

    // Upgrade binary resultant variable to an implicit binary variable.
    if hd(scip, conshdlr).upgrresultant && !crate::var::var_is_implied_integral(scip, resvar) {
        unimplemented!("constraints/and/upgraderesultant");
    }

    // Create constraint data.
    let consdata = consdata_create(scip, vars, resvar)?;
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Create constraint.
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)?;

    // Catch needed events on variables, in the transformed problem (C does it
    // in consdataCreate).
    if api::is_transformed(scip) {
        consdata_catch_events(scip, cons, eventhdlr)?;
    }

    Ok(cons)
}

/// `SCIPcreateConsBasicAnd`: creates and captures an AND-constraint with all
/// constraint flags set to their default values.
pub fn create_cons_basic_and(scip: &mut Scip, name: &str, resvar: VarId, vars: &[VarId]) -> ScipResult<ConsId> {
    create_cons_and(scip, name, resvar, vars, &ConsFlags::default())
}

/// `SCIPgetNVarsAnd`: gets number of variables in AND-constraint.
pub fn get_n_vars_and(scip: &Scip, cons: ConsId) -> i32 {
    cd(scip, cons).nvars
}

/// `SCIPgetVarsAnd`: gets array of variables in AND-constraint.
pub fn get_vars_and(scip: &Scip, cons: ConsId) -> &[VarId] {
    let c = cd(scip, cons);
    &c.vars.0[..c.nvars as usize]
}

/// `SCIPgetResultantAnd`: gets the resultant variable in AND-constraint.
pub fn get_resultant_and(scip: &Scip, cons: ConsId) -> VarId {
    cd(scip, cons).resvar
}

/// `SCIPisAndConsSorted`: return if the variables of the AND-constraint are
/// sorted with respect to their indices.
pub fn is_and_cons_sorted(scip: &Scip, cons: ConsId) -> bool {
    cd(scip, cons).sorted
}

/// `SCIPsortAndCons`: sort the variables of the AND-constraint with respect
/// to their indices.
pub fn sort_and_cons(scip: &mut Scip, cons: ConsId) {
    consdata_sort(scip, cons);
}
