//! Port of `cons_setppc.c`: constraint handler for the set partitioning /
//! packing / covering constraints `1^T x == 1`, `1^T x <= 1` or
//! `1^T x >= 1`.
//!
//! Not ported: variable usage counting (`VARUSES`) and LP branching
//! (`BRANCHLP`), both compiled out in the reference; the NLP rows (the NLP
//! is not ported); printing and parsing.

use std::any::Any;

use crate::{
    api,
    cons::{ConsFlags, *},
    cvec::CVec,
    event::*,
    misc::RandNumGen,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var, sort,
    types::{BoundType, ConsId, ConshdlrId, EventhdlrId, LockType, NLOCKTYPES, ScipResultCode, SolId, VarId, VarStatus},
    var::{BdChgIdx, NONE, var_compare},
};

/* constraint handler properties */
pub const CONSHDLR_NAME: &str = "setppc";
const CONSHDLR_DESC: &str = "set partitioning / packing / covering constraints";
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 700000;
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -700000;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -700000;
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
const LINCONSUPGD_PRIORITY: i32 = 700000;
/// Priority of the constraint handler for upgrading of nonlinear
/// constraints.
#[allow(dead_code)]
const NONLINCONSUPGD_PRIORITY: i32 = 700000;

const EVENTHDLR_NAME: &str = "setppc";
const EVENTHDLR_DESC: &str = "bound change event handler for set partitioning / packing / covering constraints";

const CONFLICTHDLR_NAME: &str = "setppc";
const CONFLICTHDLR_DESC: &str = "conflict handler creating set covering constraints";
const CONFLICTHDLR_PRIORITY: i32 = LINCONSUPGD_PRIORITY;

/// Should pairwise constraint comparison be performed in presolving?
const DEFAULT_PRESOLPAIRWISE: bool = true;

/// Minimal size of hash table in setppc constraint tables.
const HASHSIZE_SETPPCCONS: i32 = 500;
/// Should hash table be used for detecting redundant constraints in
/// advance.
const DEFAULT_PRESOLUSEHASHING: bool = true;
/// Number for minimal pairwise presolving comparisons.
const NMINCOMPARISONS: i32 = 200000;
/// Minimal gain per minimal pairwise presolving comparisons to repeat
/// pairwise comparison round.
const MINGAINPERNMINCOMPARISONS: f64 = 1e-06;

const DEFAULT_RANDSEED: u32 = 3;

/// Number of children created in pseudo branching (0: disable branching).
const DEFAULT_NPSEUDOBRANCHES: i32 = 2;
/// Should dual presolving steps be performed?
const DEFAULT_DUALPRESOLVING: bool = true;

/// Should we try to lift variables into other clique constraints, fix
/// variables, aggregate them, and also shrink the amount of variables in
/// clique constraints.
const DEFAULT_CLIQUELIFTING: bool = false;
/// Should we try to generate extra clique constraint out of all binary
/// variables to hopefully fasten the detection of redundant clique
/// constraints.
const DEFAULT_ADDVARIABLESASCLIQUES: bool = false;
/// Should we try to shrink the number of variables in a clique constraints,
/// by replacing more than one variable by only one.
const DEFAULT_CLIQUESHRINKING: bool = true;

/*
 * Data structures
 */

/// `SCIP_SETPPCTYPE`: type of setppc constraint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetppcType {
    /// Constraint is a set partitioning constraint: sum(x) == 1.
    #[default]
    Partitioning = 0,
    /// Constraint is a set packing constraint: sum(x) <= 1.
    Packing = 1,
    /// Constraint is a set covering constraint: sum(x) >= 1.
    Covering = 2,
}

/// `SCIP_ConshdlrData`: constraint handler data.
pub struct ConshdlrData {
    /// Event handler for bound change events.
    pub eventhdlr: EventhdlrId,
    /// Linear constraint handler or `NONE` if not included.
    pub conshdlrlinear: ConshdlrId,
    /// Number of set partitioning constraints in transformed problem.
    pub nsetpart: i64,
    /// Number of children created in pseudo branching (0 to disable
    /// branching).
    pub npseudobranches: i32,
    /// Number of fixed variables after last clique lifting run.
    pub noldfixedvars: i32,
    /// Number of implication before last clique lifting run.
    pub noldimpls: i32,
    /// Number of cliques before last clique lifting run.
    pub noldcliques: i32,
    /// Number of setppc constraints since the last clique lifting run.
    pub noldupgrs: i32,
    /// Number of setppc clique lifting runs.
    pub nclqpresolve: i32,
    /// Remember whether we upgraded a constraint type.
    pub updatedsetppctype: bool,
    /// Should we perform the clique lifting procedure.
    pub cliquelifting: bool,
    /// Check whether we have enough changes to run the lifting procedure
    /// again.
    pub enablecliquelifting: bool,
    /// Should we try to shrink the number of variables in a clique
    /// constraints, by replacing more than one variable by only one.
    pub cliqueshrinking: bool,
    /// Should we try to generate extra clique constraint out of all binary
    /// variables to hopefully fasten the detection of redundant clique
    /// constraints.
    pub addvariablesascliques: bool,
    /// Random number generator.
    pub randnumgen: RandNumGen,
    /// Should pairwise constraint comparison be performed in presolving?
    pub presolpairwise: bool,
    /// Should hash table be used for detecting redundant constraints in
    /// advance.
    pub presolusehashing: bool,
    /// Should dual presolving steps be performed?
    pub dualpresolving: bool,
    /// Cleared memory array with default values -1; used for clique
    /// partitions.
    pub probtoidxmap: CVec<i32>,
    /// Size of probtoidxmap.
    pub probtoidxmapsize: i32,
}

/// `SCIP_ConsData`: constraint data for set partitioning / packing /
/// covering constraints.
#[derive(Default)]
pub struct ConsData {
    /// Bit signature of vars array.
    pub signature: u64,
    /// LP row, if constraint is already stored in LP row format.
    pub row: usize,
    /// Variables of the constraint.
    pub vars: CVec<VarId>,
    /// Size of vars array.
    pub varssize: i32,
    /// Number of variables in the constraint.
    pub nvars: i32,
    /// Current number of variables fixed to zero in the constraint.
    pub nfixedzeros: i32,
    /// Current number of variables fixed to one in the constraint.
    pub nfixedones: i32,
    /// Type of constraint: set partitioning, packing or covering.
    pub setppctype: SetppcType,
    /// Are the constraint's variables sorted?
    pub sorted: bool,
    /// Was the set partitioning / packing constraint already added as
    /// clique?
    pub cliqueadded: bool,
    /// Is the bit signature valid?
    pub validsignature: bool,
    /// Was constraint changed since last redundancy round in preprocessing?
    pub changed: bool,
    /// Were variables deleted after last cleanup?
    pub varsdeleted: bool,
    /// Are the constraint's equal/negated variables already merged?
    pub merged: bool,
    /// Was the constraint already propagated in presolving w.r.t. the
    /// current domains?
    pub presolpropagated: bool,
    /// Does this constraint contain aggregations.
    pub existmultaggr: bool,
    /// Are events installed for this constraint?
    pub catchevents: bool,
}

/// The constraint data of a setppc constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a setppc constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the setppc constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/// The constraint handler data of the setppc constraint handler, mutably.
#[inline]
fn hdm(scip: &mut Scip, h: ConshdlrId) -> &mut ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_mut().unwrap().downcast_mut::<ConshdlrData>().unwrap()
}

/// The event handler of the setppc constraint handler of `cons`.
fn cons_eventhdlr(scip: &Scip, cons: ConsId) -> EventhdlrId {
    hd(scip, scip.conss[cons].conshdlr).eventhdlr
}

/*
 * Local methods
 */

/// `setppcCompare`: compares two active constraints of type set
/// partitioning or set packing such that a "-1" is return if
///  1. the first constraint is a set partitioning constraint and the second
///     is a set packing or
///  2. both constraints are set partitioning constraints and the second has
///     more! variables than the first or
///  3. both constraints are set packing constraints and the second has
///     less! variables than the first
///     a "0" is return if
///  1. both constraint are of the same type and have the amount of
///     variables or
///     and a "1" is returned otherwise.
#[allow(dead_code)] // used by the clique lifting only
fn setppc_compare(scip: &Scip, cons1: ConsId, cons2: ConsId) -> i32 {
    // The partitioning type should be the smallest value and the packing the
    // second smallest.
    let consdata1 = cd(scip, cons1);
    let consdata2 = cd(scip, cons2);

    if consdata1.setppctype < consdata2.setppctype
        || (consdata1.setppctype == SetppcType::Partitioning && consdata1.nvars < consdata2.nvars)
        || (consdata2.setppctype == SetppcType::Packing && consdata1.nvars > consdata2.nvars)
    {
        -1
    } else if consdata1.setppctype == consdata2.setppctype && consdata1.nvars == consdata2.nvars {
        0
    } else {
        1
    }
}

/// `setppcCompare2`: compares two setppc constraints such that a "-1" is
/// return if the first constraint is active and
///  1. the second constraint is deleted
///  2. the first constraint is a set partitioning constraint and the second
///     is a set packing or
///  3. both constraints are set partitioning constraints and the second has
///     more! variables than the first or
///  4. both constraints are set packing constraints and the second has
///     less! variables than the first
///     a "0" is return if
///  1. both constraint are set-covering constraints
///  2. both constraint are of the same type and have the amount of
///     variables or
///     and a "1" is returned otherwise.
fn setppc_compare2(scip: &Scip, cons1: ConsId, cons2: ConsId) -> i32 {
    if scip.conss[cons1].deleted {
        if scip.conss[cons2].deleted {
            return 0;
        } else {
            return 1;
        }
    } else if scip.conss[cons2].deleted {
        return -1;
    }

    let consdata1 = cd(scip, cons1);
    let consdata2 = cd(scip, cons2);

    // The partitioning type should be the smallest value and the packing the
    // second smallest.
    if consdata1.setppctype < consdata2.setppctype
        || (consdata1.setppctype != SetppcType::Covering
            && ((consdata1.setppctype == SetppcType::Partitioning && consdata1.nvars < consdata2.nvars)
                || (consdata2.setppctype == SetppcType::Packing && consdata1.nvars > consdata2.nvars)))
    {
        -1
    } else if consdata2.setppctype == SetppcType::Covering || (consdata1.setppctype == consdata2.setppctype && consdata1.nvars == consdata2.nvars) {
        0
    } else {
        1
    }
}

/// `lockRounding`: installs rounding locks for the given variable in the
/// given setppc constraint.
fn lock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    match cd(scip, cons).setppctype {
        SetppcType::Partitioning => api::lock_var_cons(scip, var, cons, true, true),
        SetppcType::Packing => api::lock_var_cons(scip, var, cons, false, true),
        SetppcType::Covering => api::lock_var_cons(scip, var, cons, true, false),
    }
}

/// `unlockRounding`: removes rounding locks for the given variable in the
/// given setppc constraint.
fn unlock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    match cd(scip, cons).setppctype {
        SetppcType::Partitioning => api::unlock_var_cons(scip, var, cons, true, true),
        SetppcType::Packing => api::unlock_var_cons(scip, var, cons, false, true),
        SetppcType::Covering => api::unlock_var_cons(scip, var, cons, true, false),
    }
}

/// `conshdlrdataCreate`: creates constraint handler data for set
/// partitioning / packing / covering constraint handler.
fn conshdlrdata_create(scip: &Scip, eventhdlr: EventhdlrId) -> ConshdlrData {
    ConshdlrData {
        npseudobranches: DEFAULT_NPSEUDOBRANCHES,
        // Set event handler for bound change events.
        eventhdlr,
        nsetpart: 0,
        probtoidxmap: CVec::default(),
        probtoidxmapsize: 0,
        // Create a random number generator.
        randnumgen: RandNumGen::new(scip.set.initialize_random_seed(DEFAULT_RANDSEED)),
        conshdlrlinear: NONE,
        noldfixedvars: 0,
        noldimpls: 0,
        noldcliques: 0,
        noldupgrs: 0,
        nclqpresolve: 0,
        updatedsetppctype: false,
        cliquelifting: false,
        enablecliquelifting: false,
        cliqueshrinking: false,
        addvariablesascliques: false,
        presolpairwise: false,
        presolusehashing: false,
        dualpresolving: false,
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

/// `consdataCreate`: creates a set partitioning / packing / covering
/// constraint data object.
fn consdata_create(scip: &mut Scip, vars: &[VarId], setppctype: SetppcType) -> ScipResult<ConsData> {
    let nvars = vars.len() as i32;
    let mut c = ConsData { row: NONE, ..ConsData::default() };

    if nvars > 0 {
        // @todo the setppc constraint handler does not remove fixed variables
        // from its var array; removing those variables is only possible if we
        // consider the values of nfixedones and nfixedzeros in all propagation
        // methods.

        // For uncompressed copies, simply duplicate the whole array.
        c.vars = CVec(vars.to_vec());
        c.varssize = nvars;
        c.nvars = nvars;

        if api::is_transformed(scip) {
            // Get transformed variables.
            api::get_transformed_vars(scip, &mut c.vars.0[..])?;

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
    }

    c.setppctype = setppctype;
    c.sorted = nvars <= 1;
    c.cliqueadded = false;
    c.validsignature = false;
    c.changed = true;
    c.varsdeleted = false;
    c.merged = false;
    c.presolpropagated = false;

    Ok(c)
}

/// `consdataCreateTransformed`: creates a transformed set partitioning /
/// packing / covering constraint data object.
fn consdata_create_transformed(scip: &mut Scip, vars: &[VarId], setppctype: SetppcType) -> ScipResult<ConsData> {
    // Create constraint data.
    let mut c = consdata_create(scip, vars, setppctype)?;

    // Transform the variables.
    let n = c.nvars as usize;
    api::get_transformed_vars(scip, &mut c.vars.0[..n])?;

    Ok(c)
}

/// `consdataFree`: frees a set partitioning / packing / covering
/// constraint data.
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

/// `consdataGetSignature`: returns the bit signature of the given
/// constraint data.
fn consdata_get_signature(scip: &Scip, c: &mut ConsData) -> u64 {
    if !c.validsignature {
        c.signature = 0;
        for i in 0..c.nvars {
            c.signature |= crate::misc::hash_signature64(scip.vars[c.vars[i]].index as u32);
        }
        c.validsignature = true;
    }
    c.signature
}

/// `consdataSort`: sorts setppc constraint's variables by non-decreasing
/// variable index.
fn consdata_sort(scip: &Scip, c: &mut ConsData) {
    if !c.sorted {
        if c.nvars <= 1 {
            c.sorted = true;
        } else {
            let n = c.nvars as usize;
            sort::sort(&mut c.vars.0[..n], (), |&a, &b| var_compare(scip, a, b));
            c.sorted = true;
        }
    }
}

/// [`consdata_sort`] on the data of `cons`.
fn consdata_sort_cons(scip: &mut Scip, cons: ConsId) {
    let mut data = scip.conss[cons].consdata.take().unwrap();
    consdata_sort(scip, data.downcast_mut::<ConsData>().unwrap());
    scip.conss[cons].consdata = Some(data);
}

/// [`consdata_get_signature`] on the data of `cons`.
fn cons_get_signature(scip: &mut Scip, cons: ConsId) -> u64 {
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let s = consdata_get_signature(scip, data.downcast_mut::<ConsData>().unwrap());
    scip.conss[cons].consdata = Some(data);
    s
}

/// `setSetppcType`: changes the type of a setppc constraint.
fn set_setppc_type(scip: &mut Scip, cons: ConsId, setppctype: SetppcType) -> ScipResult {
    if cd(scip, cons).setppctype == setppctype {
        return Ok(());
    }

    // Remove rounding locks.
    let mut locked = false;
    let mut i = 0;
    while i < NLOCKTYPES && !locked {
        locked = api::cons_is_locked_type(scip, cons, i);
        i += 1;
    }

    if locked {
        for i in 0..cd(scip, cons).nvars {
            let var = cd(scip, cons).vars[i];
            unlock_rounding(scip, cons, var)?;
        }
    }

    let conshdlr = scip.conss[cons].conshdlr;

    if api::is_transformed(scip) {
        if setppctype == SetppcType::Partitioning {
            hdm(scip, conshdlr).nsetpart += 1;
        } else if cd(scip, cons).setppctype == SetppcType::Partitioning {
            hdm(scip, conshdlr).nsetpart -= 1;
        }
    }

    // Change the constraint type.
    cdm(scip, cons).setppctype = setppctype;

    // Reinstall rounding locks again.
    if locked {
        for i in 0..cd(scip, cons).nvars {
            let var = cd(scip, cons).vars[i];
            lock_rounding(scip, cons, var)?;
        }
    }

    // Remember that we changed a constraint type for clique lifting
    // procedure.
    if setppctype != SetppcType::Covering {
        hdm(scip, conshdlr).updatedsetppctype = true;
    }

    Ok(())
}

/// The events a setppc constraint catches on each of its variables.
const SETPPC_EVENTS: EventType = EVENTTYPE_BOUNDCHANGED | EVENTTYPE_VARDELETED | EVENTTYPE_VARFIXED;

/// `catchEvent`: catches events for variable at given position; the event
/// data is the constraint.
fn catch_event(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    let var = cd(scip, cons).vars[pos];

    // We are catching the following events:
    //
    // - SCIP_EVENTTYPE_BOUNDCHANGED: Is used to count the number of variable
    //   fixed locally to zero and one. That helps to speed up the propagation
    //
    // - SCIP_EVENTTYPE_VARDELETED: Is caught to remove a deleted variable from
    //   the constraint
    //
    // - SCIP_EVENTTYPE_VARFIXED: Is used to get informed if a variable of the
    //   constraint was aggregated which means was detected to be equal or a
    //   negated variable of on other variable. in case of a negation this could
    //   lead to a redundant constraint if the (other) active variable is also
    //   part of the constraint.

    // Catch bound change events on variable.
    api::catch_var_event(scip, var, SETPPC_EVENTS, eventhdlr, cons, None)?;

    // Update the fixed variables counters for this variable.
    if scip.set.is_eq(scip.vars[var].locdom.ub, 0.0) {
        cdm(scip, cons).nfixedzeros += 1;

        // During presolving, we may fix the last unfixed variable or do an
        // aggregation if there are two unfixed variables.
        let c = cd(scip, cons);
        if scip.conss[cons].is_active() && (scip.set.stage < Stage::InitSolve && c.nfixedzeros >= c.nvars - 2) {
            cdm(scip, cons).presolpropagated = false;

            // During solving, we only propagate again if there is only one unfixed
            // variable left.
            let c = cd(scip, cons);
            if c.nfixedzeros >= c.nvars - 1 {
                cons_mark_propagate(scip, cons);
            }
        }
    } else if scip.set.is_eq(scip.vars[var].locdom.lb, 1.0) {
        cdm(scip, cons).nfixedones += 1;

        if scip.conss[cons].is_active() {
            cdm(scip, cons).presolpropagated = false;
            cons_mark_propagate(scip, cons);
        }
    }

    Ok(())
}

/// `dropEvent`: drops events for variable at given position.
fn drop_event(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    let var = cd(scip, cons).vars[pos];

    // Drop events on variable.
    api::drop_var_event(scip, var, SETPPC_EVENTS, eventhdlr, cons, -1)?;

    // Update the fixed variables counters for this variable.
    if scip.set.is_eq(scip.vars[var].locdom.ub, 0.0) {
        cdm(scip, cons).nfixedzeros -= 1;
    } else if scip.set.is_eq(scip.vars[var].locdom.lb, 1.0) {
        cdm(scip, cons).nfixedones -= 1;
    }

    Ok(())
}

/// `catchAllEvents`: catches bound change events for all variables in
/// transformed setppc constraint.
fn catch_all_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    if cd(scip, cons).catchevents {
        return Ok(());
    }

    // Catch event for every single variable.
    for i in 0..cd(scip, cons).nvars {
        catch_event(scip, cons, eventhdlr, i)?;
    }

    cdm(scip, cons).catchevents = true;

    Ok(())
}

/// `dropAllEvents`: drops bound change events for all variables in
/// transformed setppc constraint.
fn drop_all_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    if !cd(scip, cons).catchevents {
        return Ok(());
    }

    // Drop event of every single variable.
    for i in 0..cd(scip, cons).nvars {
        drop_event(scip, cons, eventhdlr, i)?;
    }

    cdm(scip, cons).catchevents = false;

    Ok(())
}

/// `addCoef`: adds coefficient in setppc constraint.
fn add_coef(scip: &mut Scip, cons: ConsId, mut var: VarId) -> ScipResult {
    // Are we in the transformed problem?
    let transformed = api::cons_is_transformed(scip, cons);

    // Always use transformed variables in transformed constraints.
    if transformed {
        var = api::get_transformed_var(scip, var)?;
    }

    let nvars = cd(scip, cons).nvars;
    let mut data = scip.conss[cons].consdata.take().unwrap();
    consdata_ensure_vars_size(scip, data.downcast_mut::<ConsData>().unwrap(), nvars + 1);
    scip.conss[cons].consdata = Some(data);

    let index = scip.vars[var].index;
    let c = cdm(scip, cons);
    c.vars[c.nvars] = var;
    c.nvars += 1;
    if c.validsignature {
        c.signature |= crate::misc::hash_signature64(index as u32);
    }
    c.sorted = c.nvars == 1;
    c.changed = true;

    // Capture the variable.
    crate::var::var_capture(scip, var);

    // If we are in transformed problem, catch the variable's events.
    if transformed {
        // Get event handler.
        let eventhdlr = cons_eventhdlr(scip, cons);

        // Catch bound change events of variable.
        if cd(scip, cons).catchevents {
            let pos = cd(scip, cons).nvars - 1;
            catch_event(scip, cons, eventhdlr, pos)?;
        }

        let probvar = crate::var::var_get_probvar(scip, var);
        if !cd(scip, cons).existmultaggr && scip.vars[probvar].varstatus == VarStatus::MultAggr {
            cdm(scip, cons).existmultaggr = true;
        }
    }

    // Install the rounding locks for the new variable.
    lock_rounding(scip, cons, var)?;

    // Add the new coefficient to the LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        api::add_var_to_row(scip, row, var, 1.0)?;
    }

    let c = cdm(scip, cons);
    c.merged = false;
    c.cliqueadded = false;

    Ok(())
}

/// `delCoefPos`: deletes coefficient at given position from setppc
/// constraint data.
fn del_coef_pos(scip: &mut Scip, cons: ConsId, pos: i32) -> ScipResult {
    let mut var = cd(scip, cons).vars[pos];

    // Remove the rounding locks for the deleted variable.
    unlock_rounding(scip, cons, var)?;

    // If we are in transformed problem, delete the event data of the variable.
    if api::cons_is_transformed(scip, cons) {
        // Get event handler.
        let eventhdlr = cons_eventhdlr(scip, cons);

        // Drop bound change events of variable.
        if cd(scip, cons).catchevents {
            drop_event(scip, cons, eventhdlr, pos)?;
        }

        // The last variable of the constraint was deleted; mark it for
        // propagation (so that it can be deleted).
        if cd(scip, cons).nvars == 1 {
            cdm(scip, cons).presolpropagated = false;
        }
    }

    // Delete coefficient from the LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        api::add_var_to_row(scip, row, var, -1.0)?;
    }

    // Move the last variable to the free slot.
    let c = cdm(scip, cons);
    if pos != c.nvars - 1 {
        c.vars[pos] = c.vars[c.nvars - 1];
        c.sorted = false;
    }
    c.nvars -= 1;
    c.validsignature = false;
    c.changed = true;

    // Release variable.
    api::release_var(scip, &mut var)?;

    Ok(())
}

/// `dualPresolving`: preform dual presolving
///
/// In case a part (more than one variable) in the setppc constraint is
/// independent of everything else (is locked only by this constraint), we
/// can perform dual reductions:
///
/// (1) set covering
///
///   - fix all independent variables with negative object coefficient to
///     one
///   - fix all remaining independent variables to zero
///
///   (i) all variables are independent and the constraint is not
///       modifiable
///
///       - fix the variable with the smallest object coefficient to one
///
///   (ii) a variable x has exactly 0 uplocks and arbitrary downlocks and a
///        variable y has exactly 1 downlock and arbitrary uplocks and obj(x)
///        <= obj(y) and obj(y) >= 0
///
///       - fix y to 0, because it is dominated by x
///
/// (2) set partitioning
///
///   (i) all variables are independent and the constraint is not
///       modifiable
///
///       - fix the variable with the smallest object coefficient to one
///       - fix all remaining independent variables to zero
///
///   (ii) a variable x has exactly 1 uplock and arbitrary downlocks and a
///        variable y has exactly 1 downlock and arbitrary uplocks and obj(x)
///        <= obj(y)
///
///       - fix y to 0, because it is dominated by x
///
/// (3) set packing
///
///   (i) all variables are independent and the constraint is not
///       modifiable
///
///       - fix the variable with the smallest object coefficient to one if
///         the object coefficient is negative or zero
///       - fix all remaining independent variables to zero
///
///   (ii) a variable x has exactly 1 uplock and arbitrary downlocks and a
///        variable y has exactly 0 downlocks and arbitrary uplocks and obj(x)
///        <= obj(y)
///
///       - fix y to 0, because it is dominated by x
///
///
/// Note: the following dual reduction for set covering and set packing
/// constraints is already performed by the presolver "dualfix"
///   (1) in case of a set covering constraint the following dual reduction
///       can be performed:
///       - if a variable in a set covering constraint is only locked by that
///         constraint and has negative or zero objective coefficient than it
///         can be fixed to one
///   (2) in case of a set packing constraint the following dual reduction
///       can be performed:
///       - if a variable in a set packing constraint is only locked by that
///         constraint and has positive or zero objective coefficient than it
///         can be fixed to zero
///
/// Note: all dual reduction (ii) could also be performed by the "domcol"
/// presolver, but because the pairwise comparison of columns is only done
/// heuristically (and here it should be even cheaper) we perform them here
/// (too).
///
/// Moreover, if there exists a variable that is only locked by a covering
/// or packing constraint with two variables, one can aggregate variables.
///
/// Returns whether presolving was performed (C's `SCIP_SUCCESS`).
fn dual_presolving(scip: &mut Scip, cons: ConsId, nfixedvars: &mut i32, ndelconss: &mut i32, naggrvars: &mut i32) -> ScipResult<bool> {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};
    const M: LockType = LockType::Model;

    // Constraints for which the check flag is set to FALSE, did not contribute
    // to the lock numbers; therefore, we cannot use the locks to decide for a
    // dual reduction using this constraint; for example after a restart the
    // cuts which are added to the problems have the check flag set to FALSE.
    if !scip.conss[cons].is_checked() {
        return Ok(false);
    }

    // Modifiable non-covering constraints cannot be deleted if one variable is
    // fixed to one, because the propagation for newly inserted variables must
    // be considered later.
    if cd(scip, cons).nfixedones == 1 && scip.conss[cons].modifiable {
        return Ok(false);
    }

    // All fixed variables should be removed at that point.
    let nvars = cd(scip, cons).nvars;

    // We don't want to consider small constraints (note that the constraints
    // can be modifiable, so we can't delete this constraint).
    if nvars < 2 {
        return Ok(false);
    }

    let setppctype = cd(scip, cons).setppctype;
    let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
    let mut idx: i32 = -1;
    let mut indepidx: i32 = -1;
    let mut bestobjval = crate::def::INVALID;

    // Collect the rounding locks depending on the setppc type.
    let (nlockdowns, nlockups, objsign) = match setppctype {
        SetppcType::Partitioning => (1, 1, 0.0),
        SetppcType::Packing => (0, 1, -1.0),
        SetppcType::Covering => (1, 0, 1.0),
    };

    // The objective of the variable, seen through its active variable.
    let active_obj = |scip: &Scip, var: VarId| -> ScipResult<(VarId, f64)> {
        let mut activevar = var;
        let mut negated = false;

        // Get the active variable.
        crate::var::var_get_probvar_binary(scip, &mut activevar, &mut negated)?;

        let objval = if negated { -scip.vars[activevar].obj } else { scip.vars[activevar].obj };
        Ok((activevar, objval))
    };

    let mut nposfixings = 0;

    // Check if we can apply the dual reduction; therefore count the number of
    // variables where the setppc has the only locks on this constraint.
    for v in 0..nvars {
        let var = vars[v as usize];

        // The variable should not be (globally) fixed.
        if locksdown(scip, var, M) >= nlockdowns && locksup(scip, var, M) == nlockups {
            let (_, objval) = active_obj(scip, var)?;

            // Check if the current variable has a smaller objective coefficient.
            if idx == -1 || objval < bestobjval {
                idx = v;
                bestobjval = objval;
            }

            // Determine independent variable, i.e., only locked by the current
            // constraint.
            if locksdown(scip, var, M) == nlockdowns {
                // Store variables that have the right objective sign.
                if objval * objsign >= 0.0 {
                    indepidx = v;
                }
            }
        }

        // In case another constraint has also downlocks on that variable we
        // cannot perform a dual reduction on these variables.
        if locksdown(scip, var, M) == nlockdowns && locksup(scip, var, M) >= nlockups {
            nposfixings += 1;
        }
    }

    if idx == -1 || nposfixings == 0 {
        return Ok(false);
    }

    let noldfixed = *nfixedvars;

    // In the special case of two variables, where one variable is independent
    // and will be minimized for covering or maximized for packing or does not
    // appear in the objective, we can aggregate variables:
    //  - Covering: var1 + var2 >= 1 and the objective of var1 is non-negative.
    //  - Packing:  var1 + var2 <= 1 and the objective of var1 is non-positive.
    // In both cases, var1 + var2 = 1 holds in every optimal solution.
    if setppctype != SetppcType::Partitioning && nvars == 2 && indepidx >= 0 {
        let idx2 = 1 - indepidx;

        scip_var::aggregate_vars(scip, vars[indepidx as usize], vars[idx2 as usize], 1.0, 1.0, 1.0)?;
        *naggrvars += 1;

        // Remove constraint since it is redundant.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;

        return Ok(true);
    }

    // In case of set packing and set partitioning we fix the dominated
    // variables to zero.
    if setppctype != SetppcType::Covering {
        // First part of all variables.
        for v in (0..nvars).rev() {
            if v == idx {
                continue;
            }

            let var = vars[v as usize];

            // In case another constraint has also downlocks on that variable we
            // cannot perform a dual reduction on these variables.
            if locksdown(scip, var, M) == nlockdowns && locksup(scip, var, M) >= nlockups {
                let (_, objval) = active_obj(scip, var)?;

                if objval >= bestobjval {
                    scip_var::fix_var(scip, var, 0.0)?;
                    *nfixedvars += 1;
                }
            }
        }
    }
    // If we got a set covering constraint and not all variables are locked
    // from this constraint it might not get redundant (which is case if it is
    // not possible to fix at least one variable to one), we fix all redundant
    // variables to their best bound.
    else {
        // First part of all variables.
        for v in (0..nvars).rev() {
            if v == idx {
                continue;
            }

            let var = vars[v as usize];

            // In case another constraint has also downlocks on that variable we
            // cannot perform a dual reduction on these variables.
            if locksdown(scip, var, M) == nlockdowns && locksup(scip, var, M) >= nlockups {
                let (_, objval) = active_obj(scip, var)?;

                let fixval = if objval > 0.0 { 0.0 } else { 1.0 };

                // If variables has a negative objective contribution, and is uplocked
                // by another constraint we cannot fix the variables to 1.
                if (fixval == 1.0 && locksup(scip, var, M) > nlockups) || objval < bestobjval {
                    continue;
                }

                scip_var::fix_var(scip, var, fixval)?;
                *nfixedvars += 1;
            }
        }
    }

    // If all variables but the domination variable is fixed and the
    // constraint is not modifiable or the constraint is a covering constraint
    // and the bestobjval is less than or equal to zero, we can fix the
    // domination variable (with best objective coefficient) and the constraint
    // gets redundant.
    if ((*nfixedvars - noldfixed == nvars - 1) && !scip.conss[cons].modifiable) || (setppctype == SetppcType::Covering && bestobjval <= 0.0) {
        // In case of a set packing constraint with positive objective values,
        // all variables can be fixed to zero; in all other cases the variable
        // with the smallest objective values is fixed to one.
        if (setppctype == SetppcType::Packing && bestobjval > 0.0 && locksdown(scip, vars[idx as usize], M) == 0) || setppctype != SetppcType::Packing || bestobjval <= 0.0 {
            let fixval = if setppctype == SetppcType::Packing && bestobjval > 0.0 { 0.0 } else { 1.0 };

            scip_var::fix_var(scip, vars[idx as usize], fixval)?;
            *nfixedvars += 1;
        }

        // Remove constraint since it is redundant.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
    }

    // Set result pointer to SCIP_SUCCESS, if variables could be fixed.
    Ok(*nfixedvars != noldfixed)
}

/// `mergeMultiples`: find pairs of negated variables in constraint:
/// partitioning/packing: all other variables must be zero, constraint is
/// redundant
/// covering: constraint is redundant
///
/// find sets of equal variables in constraint:
/// partitioning/packing: variable must be zero
/// covering: multiple entries of variable can be replaced by single entry
///
/// Returns whether a fixing leads to a cutoff.
fn merge_multiples(scip: &mut Scip, cons: ConsId, nfixedvars: &mut i32, ndelconss: &mut i32, nchgcoefs: &mut i32) -> ScipResult<bool> {
    if cd(scip, cons).merged || scip.conss[cons].deleted {
        return Ok(false);
    }

    if cd(scip, cons).nvars <= 1 {
        cdm(scip, cons).merged = true;
        return Ok(false);
    }

    // Sorting array after indices of variables, that's only for faster
    // merging.
    let mut data = scip.conss[cons].consdata.take().unwrap();
    {
        let c = data.downcast_mut::<ConsData>().unwrap();
        let n = c.nvars as usize;
        sort::sort(&mut c.vars.0[..n], (), |&a, &b| crate::var::var_compare_active_and_negated(scip, a, b));

        // Setppc sorting now lost.
        c.sorted = false;
    }
    scip.conss[cons].consdata = Some(data);

    // Loop backwards through the items: deletion only affects rear items.
    let mut v = cd(scip, cons).nvars - 1;
    while v > 0 {
        let mut negated1 = false;
        let mut negated2 = false;

        let mut var1 = cd(scip, cons).vars[v];
        if scip.vars[var1].varstatus == VarStatus::Negated {
            var1 = scip.vars[var1].negatedvar;
            negated1 = true;
        }
        let mut var2 = cd(scip, cons).vars[v - 1];
        if scip.vars[var2].varstatus == VarStatus::Negated {
            var2 = scip.vars[var2].negatedvar;
            negated2 = true;
        }

        if var1 == var2 {
            // One variables is active and the other is the same negated variable.
            if negated1 != negated2 {
                // All other variable have to be zero if it's a partitioning or
                // packing constraint.
                if cd(scip, cons).setppctype != SetppcType::Covering {
                    for i in (0..cd(scip, cons).nvars).rev() {
                        if i != v && i != (v - 1) {
                            let var = cd(scip, cons).vars[i];
                            let (infeasible, fixed) = scip_var::fix_var(scip, var, 0.0)?;
                            if infeasible {
                                return Ok(true);
                            }

                            if fixed {
                                *nfixedvars += 1;
                            }
                        }
                    }
                }

                // All setppc-type constraints are redundant.
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
                return Ok(false);
            }
            // Both variables are either active or negated.
            else {
                // This variable can be fixed to zero if it's a partitioning or
                // packing constraint.
                if cd(scip, cons).setppctype != SetppcType::Covering {
                    let (infeasible, fixed) = scip_var::fix_var(scip, var1, if negated1 { 1.0 } else { 0.0 })?;
                    if infeasible {
                        return Ok(true);
                    }

                    if fixed {
                        *nfixedvars += 1;
                    }
                }
                // Multiple entries of variable can be replaced by single entry.
                else {
                    del_coef_pos(scip, cons, v)?; // only some changed behind position v-1, so it's okay
                    *nchgcoefs += 1;
                }
            }
            cdm(scip, cons).changed = true;
        }
        v -= 1;
    }
    cdm(scip, cons).merged = true;

    Ok(false)
}

/// `applyFixings`: deletes all zero-fixed variables and replace aggregated
/// variables. `counts` is `(naddconss, ndelconss, nfixedvars)`, or `None`
/// indicating we can not resolve multi-aggregations (C passes all four
/// pointers or none). Returns whether a fixing leads to a cutoff.
fn apply_fixings(scip: &mut Scip, cons: ConsId, mut counts: Option<(&mut i32, &mut i32, &mut i32)>) -> ScipResult<bool> {
    let mut cutoff = false;

    // All multi-aggregations should be resolved.
    cdm(scip, cons).existmultaggr = false;

    let mut v = 0;
    'outer: while v < cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];

        if scip.vars[var].glbdom.ub < 0.5 {
            del_coef_pos(scip, cons, v)?;
        } else {
            // Get binary representative of variable.
            let (repvar, _negated) = scip_var::get_binvar_representative(scip, var)?;

            // Resolve multi-aggregation.
            let rstatus = scip.vars[repvar].varstatus;
            if rstatus == VarStatus::MultAggr || (rstatus == VarStatus::Negated && scip.vars[scip.vars[repvar].negatedvar].varstatus == VarStatus::MultAggr) {
                let mut constant = 0.0;
                let mut requiredsize = 0;
                let mut consvarssize = 10;

                let mut nconsvars = 1;
                let mut consvars = CVec(vec![NONE; consvarssize as usize]);
                let mut consvals = CVec(vec![0.0; consvarssize as usize]);
                consvars[0usize] = repvar;
                consvals[0usize] = 1.0;

                // Get active variables for new constraint.
                crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, consvarssize, &mut constant, &mut requiredsize)?;
                // If space was not enough we need to resize the buffers.
                if requiredsize > consvarssize {
                    consvarssize = requiredsize;
                    consvars.0.resize(consvarssize as usize, NONE);
                    consvals.0.resize(consvarssize as usize, 0.0);

                    crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, consvarssize, &mut constant, &mut requiredsize)?;
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
                } else if scip.set.is_feas_eq(constant, 1.0) {
                    // Check for another multi-aggregation.
                    let mut v2 = cd(scip, cons).nvars - 1;
                    while v2 > v {
                        let probvar = crate::var::var_get_probvar(scip, cd(scip, cons).vars[v]);
                        if scip.vars[probvar].varstatus == VarStatus::MultAggr {
                            break;
                        }
                        v2 -= 1;
                    }

                    // Constraint is redundant.
                    if v2 == v && nconsvars == 0 {
                        // We can fix.
                        if cd(scip, cons).nvars > 1
                            && cd(scip, cons).setppctype != SetppcType::Covering
                            && let Some((_, _, nfixedvars)) = counts.as_mut()
                        {
                            for v2 in (0..cd(scip, cons).nvars).rev() {
                                let var2 = cd(scip, cons).vars[v2];
                                if var2 != var {
                                    // Fix all remaining variables to zero, constraint is
                                    // already feasible or infeasible.
                                    let fixed;
                                    (cutoff, fixed) = scip_var::fix_var(scip, var2, 0.0)?;
                                    if cutoff {
                                        break 'outer;
                                    }

                                    if fixed {
                                        **nfixedvars += 1;
                                    }
                                }
                            }
                        }

                        // C also asks for nfixedvars != NULL, a single variable or a
                        // covering constraint; nfixedvars is given whenever ndelconss is.
                        if let Some((_, ndelconss, _)) = counts.as_mut() {
                            // Delete old constraint.
                            scip_cons::del_cons(scip, cons)?;
                            **ndelconss += 1;
                        }

                        break 'outer;
                    }
                }

                // We can easily add the coefficients and still have a setppc
                // constraint.
                if easycase {
                    // Delete old (multi-aggregated) variable.
                    del_coef_pos(scip, cons, v)?;

                    // Add active representation.
                    for v2 in (0..nconsvars).rev() {
                        add_coef(scip, cons, consvars[v2])?;
                    }
                }
                // We need to degrade this setppc constraint to a linear constraint.
                else if counts.is_some() || scip.conss[cons].is_added() {
                    // It might happen that there are more than one multi-aggregated
                    // variable, so we need to get the whole probvar sum over all
                    // variables.
                    let mut size = crate::def::max(nconsvars, 1) + cd(scip, cons).nvars - 1;

                    // Memory needed is at least old number of variables - 1 + number of
                    // variables in first multi-aggregation.
                    consvars.0.resize(size as usize, NONE);
                    consvals.0.resize(size as usize, 0.0);

                    nconsvars = cd(scip, cons).nvars;

                    // Add constraint variables to new linear variables.
                    for k in (0..cd(scip, cons).nvars).rev() {
                        consvars[k] = cd(scip, cons).vars[k];
                        consvals[k] = 1.0;
                    }

                    constant = 0.0;

                    // Get active variables for new constraint.
                    crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, size, &mut constant, &mut requiredsize)?;

                    // If space was not enough (we found another multi-aggregation), we
                    // need to resize the buffers.
                    if requiredsize > size {
                        size = requiredsize;
                        consvars.0.resize(size as usize, NONE);
                        consvals.0.resize(size as usize, 0.0);

                        crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, size, &mut constant, &mut requiredsize)?;
                    }

                    // Compute sides.
                    let inf = scip.set.infinity();
                    let (lhs, rhs) = match cd(scip, cons).setppctype {
                        SetppcType::Packing => (-inf, 1.0 - constant),
                        SetppcType::Partitioning => (1.0 - constant, 1.0 - constant),
                        SetppcType::Covering => (1.0 - constant, inf),
                    };

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
                    if let Some((naddconss, ndelconss, _)) = counts.as_mut() {
                        **ndelconss += 1;
                        **naddconss += 1;
                    }

                    break 'outer;
                }
                // We need to degrade this setppc constraint to a linear constraint.
                else {
                    // Check, if the variable should be replaced with the representative.
                    if repvar != var {
                        // Delete old (aggregated) variable.
                        del_coef_pos(scip, cons, v)?;

                        // Add representative instead.
                        add_coef(scip, cons, repvar)?;
                    }

                    log::warn!(
                        "setppc constraint <{}> has a multi-aggregated variable, which was not resolved and therefore could lead to aborts",
                        scip.conss[cons].name
                    );
                    v += 1;
                }
            } else {
                // Check, if the variable should be replaced with the representative.
                if repvar != var {
                    // Delete old (aggregated) variable.
                    del_coef_pos(scip, cons, v)?;

                    // Add representative instead.
                    add_coef(scip, cons, repvar)?;
                } else {
                    v += 1;
                }
            }
        }
    }

    // All multi-aggregations should be resolved.
    cdm(scip, cons).existmultaggr = false;

    Ok(cutoff)
}

/// `analyzeConflictZero`: analyzes conflicting assignment on given
/// constraint where all of the variables where assigned to zero, and adds
/// conflict constraint to problem.
fn analyze_conflict_zero(scip: &mut Scip, cons: ConsId) -> ScipResult {
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

/// `analyzeConflictOne`: analyzes conflicting assignment on given
/// constraint where two of the variables where assigned to one, and adds
/// conflict constraint to problem.
fn analyze_conflict_one(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // applicable.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis, and add the two variables assigned to one
    // to conflict candidate queue.
    api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;
    let mut n = 0;
    let mut v = 0;
    while v < cd(scip, cons).nvars && n < 2 {
        let var = cd(scip, cons).vars[v];
        if scip.vars[var].locdom.lb > 0.5 {
            api::add_conflict_binvar(scip, var)?;
            n += 1;
        }
        v += 1;
    }

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `processFixings`: checks constraint for violation only looking at the
/// fixed variables, applies further fixings if possible; returns `(addcut,
/// mustcheck)`: whether this constraint must be added as a cut, and whether
/// it must be checked for feasibility.
fn process_fixings(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32) -> ScipResult<(bool, bool)> {
    let mut addcut = false;
    let mut mustcheck = true;

    if cd(scip, cons).nfixedones == 1 {
        // Exactly one variable is fixed to 1:
        // - a set covering constraint is feasible anyway and can be disabled
        // - all other variables in a set partitioning or packing constraint must be
        //   zero
        if cd(scip, cons).setppctype == SetppcType::Covering {
            scip_cons::del_cons_local(scip, cons)?;
        } else {
            if cd(scip, cons).nfixedzeros < cd(scip, cons).nvars - 1 {
                let mut oneidx = -1;

                // Unfixed variables exist: fix them to zero; this could result in
                // additional variables fixed to one due to aggregations; in this case,
                // the constraint is infeasible in local bounds.
                let nvars = cd(scip, cons).nvars;
                let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
                let mut v = 0;
                while v < nvars && cd(scip, cons).nfixedones == 1 {
                    let var = vars[v as usize];
                    if scip.vars[var].locdom.lb < 0.5 {
                        let (_infeasible, tightened) = scip_var::infer_binvar_cons(scip, var, false, cons, oneidx)?;
                        if tightened {
                            *nfixedvars += 1;
                        }
                    } else {
                        oneidx = v;
                    }
                    v += 1;
                }

                // The fixed to one variable must have been found, and at least one
                // variable must have been fixed.
                api::reset_cons_age(scip, cons);
            }

            // Now all other variables are fixed to zero: the constraint is
            // feasible, and if it's not modifiable, it is redundant.
            if !scip.conss[cons].modifiable && cd(scip, cons).nfixedones == 1 {
                scip_cons::del_cons_local(scip, cons)?;
            }
        }
        mustcheck = false;
    }

    let c = cd(scip, cons);
    if c.nfixedones >= 2 {
        // At least two variables are fixed to 1:
        // - a set covering constraint is feasible anyway and can be disabled
        // - a set partitioning or packing constraint is infeasible
        if c.setppctype == SetppcType::Covering {
            scip_cons::del_cons_local(scip, cons)?;
        } else {
            api::reset_cons_age(scip, cons);

            // Use conflict analysis to get a conflict constraint out of the
            // conflicting assignment.
            analyze_conflict_one(scip, cons)?;

            *cutoff = true;
        }
        mustcheck = false;
    } else if c.nfixedzeros == c.nvars {
        // All variables are fixed to zero:
        // - a set packing constraint is feasible anyway, and if it's
        //   unmodifiable, it can be disabled
        // - a set partitioning or covering constraint is infeasible, and if it's
        //   unmodifiable, the node can be cut off -- otherwise, the constraint
        //   must be added as a cut and further pricing must be performed
        if c.setppctype == SetppcType::Packing {
            if !scip.conss[cons].modifiable {
                scip_cons::del_cons_local(scip, cons)?;
            }
        } else {
            api::reset_cons_age(scip, cons);
            if scip.conss[cons].modifiable {
                addcut = true;
            } else {
                // Use conflict analysis to get a conflict constraint out of the
                // conflicting assignment.
                analyze_conflict_zero(scip, cons)?;

                *cutoff = true;
            }
        }
        mustcheck = false;
    } else if c.nfixedzeros == c.nvars - 1 && c.nfixedones == 0 {
        // All variables except one are fixed to zero:
        // - a set packing constraint is feasible anyway, and if it's
        //   unmodifiable, it can be disabled
        // - an unmodifiable set partitioning or covering constraint is feasible
        //   and can be disabled after the remaining variable is fixed to one
        // - a modifiable set partitioning or covering constraint must be checked
        //   manually
        if c.setppctype == SetppcType::Packing {
            if !scip.conss[cons].modifiable {
                scip_cons::del_cons_local(scip, cons)?;
            }
            mustcheck = false;
        } else if !scip.conss[cons].modifiable {
            // Search the single variable that can be fixed.
            let nvars = c.nvars;
            let vars = c.vars.0[..nvars as usize].to_vec();
            for &var in &vars {
                if scip.vars[var].locdom.ub > 0.5 {
                    scip_var::infer_binvar_cons(scip, var, true, cons, 0)?;
                    *nfixedvars += 1;
                    break;
                }
            }
            scip_cons::del_cons_local(scip, cons)?;
            mustcheck = false;
        }
    }

    Ok((addcut, mustcheck))
}

/// `checkCons`: checks constraint for violation, returns TRUE iff
/// constraint is feasible.
fn check_cons(scip: &mut Scip, cons: ConsId, sol: SolId) -> bool {
    // Calculate the constraint's activity.
    let c = cd(scip, cons);
    let nvars = c.nvars;
    let setppctype = c.setppctype;
    let mut sum = 0.0;
    let sumbound = if setppctype == SetppcType::Covering { 1.0 } else { 1.0 + 2.0 * scip.set.feastol() };
    let mut v = 0;
    while v < nvars && sum < sumbound {
        // If sum >= sumbound, the feasibility is clearly decided.
        let solval = api::get_sol_val(scip, sol, cd(scip, cons).vars[v]);
        sum += solval;
        v += 1;
    }

    let mut absviol = sum - 1.0;
    let mut relviol = crate::misc::rel_diff(sum, 1.0);
    let check = match setppctype {
        SetppcType::Partitioning => {
            // In case of partitioning, the violation is equal to the absolute
            // difference between sum and 1.
            absviol = absviol.abs();
            relviol = relviol.abs();
            scip.set.is_feas_eq(sum, 1.0)
        }
        // In case of packing, the violation is equal to how much sum exceeds 1.
        SetppcType::Packing => scip.set.is_feas_le(sum, 1.0),
        SetppcType::Covering => {
            // In case of covering, the violation is equal to how much 1 exceeds
            // sum.
            absviol = -absviol;
            relviol = -relviol;
            scip.set.is_feas_ge(sum, 1.0)
        }
    };

    if sol != NONE {
        api::update_sol_lp_cons_violation(scip, sol, absviol, relviol);
    }

    check
}

/// `createRow`: creates an LP row in a set partitioning / packing /
/// covering constraint data object.
fn create_row(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let inf = scip.set.infinity();
    let (lhs, rhs) = match cd(scip, cons).setppctype {
        SetppcType::Partitioning => (1.0, 1.0),
        SetppcType::Packing => (-inf, 1.0),
        SetppcType::Covering => (1.0, inf),
    };

    let k = &scip.conss[cons];
    let (name, local, modifiable, removable) = (k.name.clone(), k.is_local(), k.modifiable, k.is_removable());
    let row = scip_lp::create_empty_row_cons(scip, cons, &name, lhs, rhs, local, modifiable, removable)?;
    cdm(scip, cons).row = row;

    let n = cd(scip, cons).nvars as usize;
    let vars = cd(scip, cons).vars.0[..n].to_vec();
    scip_lp::add_vars_to_row_same_coef(scip, row, &vars, 1.0)?;

    Ok(())
}

/// `addCut`: adds setppc constraint as cut to the LP; returns whether a
/// cutoff has been detected.
fn add_cut(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let mut cutoff = false;

    if cd(scip, cons).row == NONE {
        // Convert set partitioning constraint data into LP row.
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
fn separate_cons(scip: &mut Scip, cons: ConsId, sol: SolId, lpfeas: bool, cutoff: &mut bool, separated: &mut bool, reduceddom: &mut bool) -> ScipResult {
    let mut addcut;
    let mustcheck;

    *cutoff = false;

    // Skip constraints already in the LP.
    let row = cd(scip, cons).row;
    if lpfeas && row != NONE && scip.rows[row].lppos >= 0 {
        return Ok(());
    }

    // Check constraint for violation only looking at the fixed variables,
    // apply further fixings if possible.
    if lpfeas {
        let mut nfixedvars = 0;

        (addcut, mustcheck) = process_fixings(scip, cons, cutoff, &mut nfixedvars)?;

        *reduceddom = nfixedvars > 0;
    } else {
        mustcheck = true;
        addcut = false;
    }

    if mustcheck {
        // Variable's fixings didn't give us any information -> we have to check
        // the constraint.
        let row = cd(scip, cons).row;
        if lpfeas && row != NONE {
            let feasibility = scip_lp::get_row_sol_feasibility(scip, row, sol);
            addcut = scip.set.is_feas_negative(feasibility);
        } else {
            addcut = !check_cons(scip, cons, sol);
        }

        if !addcut {
            // Constraint was feasible -> increase age.
            api::inc_cons_age(scip, cons)?;
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
fn enforce_pseudo(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, infeasible: &mut bool, reduceddom: &mut bool, solvelp: &mut bool) -> ScipResult {
    let mut nfixedvars = 0;

    // Check constraint for violation only looking at the fixed variables,
    // apply further fixings if possible.
    let (addcut, mustcheck) = process_fixings(scip, cons, cutoff, &mut nfixedvars)?;

    *reduceddom = nfixedvars > 0;

    if mustcheck {
        if check_cons(scip, cons, NONE) {
            // Constraint was feasible -> increase age.
            api::inc_cons_age(scip, cons)?;
        } else {
            // Constraint was infeasible -> reset age.
            api::reset_cons_age(scip, cons);
            *infeasible = true;
        }
    }

    if addcut {
        // A cut must be added to the LP -> we have to solve the LP immediately.
        api::reset_cons_age(scip, cons);
        *solvelp = true;
    }

    Ok(())
}

/// The setppc hash table callbacks (`hashGetKeySetppccons`,
/// `hashKeyEqSetppccons`, `hashKeyValSetppccons`). C's key value sorts the
/// constraint's variables first; the hash table keeps the hash of every
/// stored element, so the callers sort the queried constraint beforehand
/// with [`consdata_sort_cons`] instead.
struct SetppcconsHash<'a> {
    scip: &'a Scip,
}

impl crate::hash::HashOps<ConsId> for SetppcconsHash<'_> {
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

/// `presolvePropagateCons`: check if constraint is already redundant or
/// infeasible due to fixings, fix or aggregate left over variables if
/// possible. Memorizing the aggregations instead (`aggregate == FALSE`)
/// belongs to the clique lifting, which is not ported.
fn presolve_propagate_cons(scip: &mut Scip, cons: ConsId, aggregate: bool, nfixedvars: &mut i32, naggrvars: &mut i32, ndelconss: &mut i32, cutoff: &mut bool) -> ScipResult {
    if !scip.conss[cons].is_active() {
        return Ok(());
    }

    if cd(scip, cons).presolpropagated {
        return Ok(());
    }

    cdm(scip, cons).presolpropagated = true;

    let c = cd(scip, cons);
    let nvars = c.nvars;
    let vars = c.vars.0[..nvars as usize].to_vec();
    let setppctype = c.setppctype;
    let modifiable = scip.conss[cons].modifiable;

    // No variables left.
    if nvars == 0 && !modifiable {
        if setppctype == SetppcType::Partitioning || setppctype == SetppcType::Covering {
            *cutoff = true;
            return Ok(());
        } else {
            // Delete constraint.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
            return Ok(());
        }
    }

    // More then two variables are fixed.
    if cd(scip, cons).nfixedones > 1 {
        // At least two variables are fixed to 1:
        // - a set covering constraint is feasible anyway and can be deleted
        // - a set partitioning or packing constraint is infeasible
        if setppctype == SetppcType::Covering {
            // Delete constraint.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
            return Ok(());
        }

        *cutoff = true;
        return Ok(());
    }

    if cd(scip, cons).nfixedones == 1 {
        // Exactly one variable is fixed to 1:
        // - a set covering constraint is feasible anyway and can be disabled
        // - all other variables in a set partitioning or packing constraint must be
        //   zero
        if setppctype != SetppcType::Covering && cd(scip, cons).nfixedzeros < nvars - 1 {
            for v in (0..nvars as usize).rev() {
                let x = &scip.vars[vars[v]];
                if x.locdom.lb + 0.5 < x.locdom.ub {
                    // Fix all remaining variables to zero, constraint is already
                    // feasible or infeasible.
                    (*cutoff, _) = scip_var::fix_var(scip, vars[v], 0.0)?;
                    if *cutoff {
                        return Ok(());
                    }

                    *nfixedvars += 1;
                }
            }
        }

        if !modifiable || setppctype == SetppcType::Covering {
            // Delete constraint.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
        }

        return Ok(());
    }

    // Other propagations can only be done on not modifiable constraints.
    if modifiable {
        return Ok(());
    }

    // All variables were fixed to zero then either delete the constraint or
    // stop with infeasibility.
    if cd(scip, cons).nfixedzeros == nvars {
        // All variables are fixed to zero:
        // - a set packing constraint is feasible anyway and can be deleted
        // - a set partitioning or covering constraint is infeasible, and so is the
        //   whole problem
        if setppctype == SetppcType::Partitioning || setppctype == SetppcType::Covering {
            *cutoff = true;
            return Ok(());
        }

        // Delete constraint.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok(());
    }

    // All but one variable were fixed to zero then delete the constraint and
    // for setpartition fix the remaining variable to 1.
    if cd(scip, cons).nfixedzeros + 1 == nvars {
        // All variables except one are fixed to zero:
        // - a set packing constraint is feasible anyway, and can be deleted
        // - a set partitioning or covering constraint is feasible and can be
        //   deleted after the remaining variable is fixed to one
        if setppctype == SetppcType::Partitioning || setppctype == SetppcType::Covering {
            for v in (0..nvars as usize).rev() {
                if scip.vars[vars[v]].locdom.ub > 0.5 {
                    // Fix the remaining set partition variable.
                    (*cutoff, _) = scip_var::fix_var(scip, vars[v], 1.0)?;
                    if *cutoff {
                        return Ok(());
                    }

                    *nfixedvars += 1;
                    break;
                }
            }
        }

        // Delete constraint.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok(());
    }

    // All but two variable were fixed to zero in a setpartitioning constraint
    // then delete the constraint and aggregate the remaining two variables.
    if setppctype == SetppcType::Partitioning && cd(scip, cons).nfixedzeros + 2 == nvars {
        let mut var = NONE;
        for v in (0..nvars as usize).rev() {
            if scip.vars[vars[v]].locdom.ub > 0.5 {
                if var == NONE {
                    var = vars[v];
                } else {
                    if !aggregate {
                        unimplemented!("memorizing setppc aggregations (clique lifting) is not ported");
                    }

                    // Aggregate last remaining variables in the set partitioning
                    // constraint.
                    let (infeasible, redundant, aggregated) = scip_var::aggregate_vars(scip, var, vars[v], 1.0, 1.0, 1.0)?;
                    *cutoff = infeasible;
                    if *cutoff {
                        return Ok(());
                    }

                    if aggregated {
                        *naggrvars += 1;
                    }

                    if redundant {
                        // Delete constraint.
                        scip_cons::del_cons(scip, cons)?;
                        *ndelconss += 1;
                    }

                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// `addCliques`: add cliques to SCIP.
#[allow(clippy::too_many_arguments)]
fn add_cliques(scip: &mut Scip, conss: &[ConsId], firstclique: i32, lastclique: i32, naddconss: &mut i32, ndelconss: &mut i32, nchgbds: &mut i32, cutoff: &mut bool) -> ScipResult {
    // Add clique and implication information.
    for c in firstclique..lastclique {
        let cons = conss[c as usize];

        // Ignore deleted constraints.
        if !scip.conss[cons].is_active() {
            continue;
        }

        let mut nlocalbdchgs = 0;
        *cutoff = apply_fixings(scip, cons, Some((naddconss, ndelconss, &mut nlocalbdchgs)))?;
        *nchgbds += nlocalbdchgs;

        if *cutoff {
            return Ok(());
        }

        if scip.conss[cons].deleted {
            continue;
        }

        if !cd(scip, cons).cliqueadded && cd(scip, cons).nvars >= 2 {
            let setppctype = cd(scip, cons).setppctype;

            // Add a set partitioning / packing constraint as clique.
            if setppctype == SetppcType::Partitioning || setppctype == SetppcType::Packing {
                let n = cd(scip, cons).nvars as usize;
                let vars = cd(scip, cons).vars.0[..n].to_vec();
                let (infeasible, nlocalbdchgs) = scip_var::add_clique(scip, &vars, &[], setppctype == SetppcType::Partitioning)?;
                *nchgbds += nlocalbdchgs;

                if infeasible {
                    *cutoff = true;
                    return Ok(());
                }
            } else if cd(scip, cons).nvars == 2 && !scip.conss[cons].modifiable {
                // A two-variable set covering constraint x + y >= 1 yields the
                // implication x == 0 -> y == 1.
                let (v0, v1) = (cd(scip, cons).vars[0usize], cd(scip, cons).vars[1usize]);
                let (infeasible, nlocalbdchgs) = scip_var::add_var_implication(scip, v0, false, v1, BoundType::Lower, 1.0)?;
                *nchgbds += nlocalbdchgs;

                if infeasible {
                    *cutoff = true;
                    return Ok(());
                }
            }
            cdm(scip, cons).cliqueadded = true;
        }
    }

    Ok(())
}

/// `multiAggregateBinvar`: perform multi-aggregation on variables resulting
/// from a set-partitioning/-packing constraint; returns `(infeasible,
/// aggregated)`.
fn multi_aggregate_binvar(scip: &mut Scip, linearconshdlrexist: bool, vars: &[VarId], pos: usize) -> ScipResult<(bool, bool)> {
    let nvars = vars.len();

    if !linearconshdlrexist && nvars > 2 {
        return Ok((false, false));
    }

    // If the first variable will be multi-aggregated, we do not need to copy
    // the variables.
    let tmpvars: Vec<VarId> = if pos == 0 {
        vars[1..].to_vec()
    }
    // If the last variable will be multi-aggregated, we do not need to copy
    // the variables.
    else if pos == nvars - 1 {
        vars[..nvars - 1].to_vec()
    }
    // Copy variables for aggregation.
    else {
        let mut t = vars[..nvars - 1].to_vec();
        t[pos] = vars[nvars - 1];
        t
    };

    // Initialize scalars.
    let scalars = vec![-1.0; nvars - 1];

    // Perform multi-aggregation.
    scip_var::multiaggregate_var(scip, vars[pos], &tmpvars, &scalars, 1.0)
}

/// `removeDoubleAndSingletonsAndPerformDualpresolve`: determine singleton
/// variables in set-partitioning/-packing constraints, or doubleton
/// variables (active and negated) in any combination of set-partitioning and
/// set-packing constraints
///
/// we can multi-aggregate the variable and either change the
/// set-partitioning constraint to a set-packing constraint or even delete it
///
/// 1. c1: x + y + z = 1,  uplocks(x) = 1, downlocks(x) = 1
///    =>  x = 1 - y - z and change c1 to y + z <= 1
///
/// 2. c2: x + y + z <= 1,  uplocks(x) = 1, downlocks(x) = 0, obj(x) < 0
///    =>  x = 1 - y - z and change c2 to y + z <= 1
///
/// 3. d1: x + y + z <= 1 and d2: ~x + u + v <= 1, uplocks(x) = 1,
///    downlocks(x) = 1
///    a)  obj(x) <= 0  =>  x = 1 - y - z and delete d1
///    b)  obj(x) > 0   => ~x = 1 - u - v and delete d2
///
/// 4. e1: x + y + z == 1 and e2: ~x + u + v (<= or ==) 1, uplocks(x) = (1
///    or 2), downlocks(x) = 2
///    =>  x = 1 - y - z and delete e1
///
/// we can also aggregate a variable in a set-packing constraint with only
/// two variables when the uplocks are equal to one and then delete this
/// constraint
///
/// 5. f1: x + y <= 1,  uplocks(x) = 1, obj(x) <= 0
///    =>  x = 1 - y and delete f1
///
/// @todo might want to multi-aggregate variables even with more locks, when
/// the fill in is still smaller or equal to the old number of non-zeros,
/// e.g.
///
///   x + y + z = 1
///   ~x + u + v <=/= 1
///   ~x + w <= 1
#[allow(clippy::too_many_arguments)]
fn remove_double_and_singletons_and_perform_dualpresolve(
    scip: &mut Scip,
    conss: &[ConsId],
    dualpresolvingenabled: bool,
    linearconshdlrexist: bool,
    nfixedvars: &mut i32,
    naggrvars: &mut i32,
    ndelconss: &mut i32,
    nchgcoefs: &mut i32,
    nchgsides: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};
    const M: LockType = LockType::Model;

    let nconss = conss.len();
    let nbinvars = api::get_n_bin_vars(scip);
    let nposbinvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip);
    let binvars: Vec<VarId> = api::get_vars(scip)[..nposbinvars as usize].to_vec();

    // Determine number for possible multi-aggregations.
    let mut nposvars = 0;
    for v in (0..nposbinvars).rev() {
        let var = binvars[v as usize];
        if v < nbinvars || crate::var::var_is_binary(scip, var) {
            let nuplocks = locksup(scip, var, M);
            let ndownlocks = locksdown(scip, var, M);

            if (nuplocks == 1 && ndownlocks <= 1) || (nuplocks <= 1 && ndownlocks == 1) || (nuplocks <= 2 && ndownlocks <= 2 && scip.vars[var].negatedvar != NONE) {
                nposvars += 1;
            }
        }
    }

    if nposvars == 0 {
        return Ok(());
    }

    // A hashmap from var to index when found in a set-partitioning constraint.
    let mut vartoindex = crate::hash::HashMap::<i32>::new(nposvars);

    // Get temporary memory.
    let mut chgtype = vec![false; nconss];
    let mut considxs = vec![0i32; nposbinvars as usize];
    let mut posincons = vec![0i32; nposbinvars as usize];
    let mut usefulconss = conss.to_vec();

    // Sort constraints.
    sort::sort(&mut usefulconss[..], (), |&a, &b| setppc_compare2(scip, a, b));

    let mut nhashmapentries: i32 = 0;
    let mut ndecs = 0;

    let donotaggr = scip_var::do_not_aggr(scip);
    let donotmultaggr = scip_var::do_not_multaggr(scip);

    // Determine singleton variables in set-partitioning/-packing constraints,
    // or doubleton variables (active and negated) in any combination of
    // set-partitioning and set-packing constraints
    //
    // we can multi-aggregate the variable and either change the
    // set-partitioning constraint to a set-packing constraint or even delete it.
    for c in 0..nconss {
        let mut nlocaladdconss = 0;

        let cons = usefulconss[c];

        if scip.conss[cons].deleted {
            continue;
        }

        // If we cannot find any constraint to perform a useful
        // multi-aggregation, stop.
        if cd(scip, cons).setppctype == SetppcType::Covering {
            break;
        }

        if !scip.conss[cons].is_checked() {
            continue;
        }

        if scip.conss[cons].modifiable {
            continue;
        }

        // Update the variables.
        *cutoff = apply_fixings(scip, cons, Some((&mut nlocaladdconss, &mut *ndelconss, &mut *nfixedvars)))?;
        if *cutoff {
            break;
        }

        // Due to resolving multi-aggregations a constraint can become deleted.
        if scip.conss[cons].deleted {
            continue;
        }

        process_fixings(scip, cons, cutoff, nfixedvars)?;
        if *cutoff {
            break;
        }

        if scip.conss[cons].deleted {
            continue;
        }

        let oldnfixedvars = *nfixedvars;

        // Merging unmerged constraints.
        *cutoff = merge_multiples(scip, cons, nfixedvars, ndelconss, nchgcoefs)?;
        if *cutoff {
            break;
        }

        if scip.conss[cons].deleted {
            continue;
        }

        if oldnfixedvars < *nfixedvars {
            // Update the variables.
            *cutoff = apply_fixings(scip, cons, Some((&mut nlocaladdconss, &mut *ndelconss, &mut *nfixedvars)))?;

            if scip.conss[cons].deleted {
                continue;
            }
        }

        // If the constraint was not merged and consists of a variable with its
        // negation, the constraint is redundant.
        if cd(scip, cons).nvars < 2 {
            // Deleting redundant set-packing constraint.
            if cd(scip, cons).setppctype == SetppcType::Packing {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;

                continue;
            } else {
                if cd(scip, cons).nvars == 0 {
                    *cutoff = true;

                    break;
                }

                let var = cd(scip, cons).vars[0usize];
                let (_infeasible, fixed) = scip_var::fix_var(scip, var, 1.0)?;
                if fixed {
                    *nfixedvars += 1;
                }

                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;

                continue;
            }
        }

        // Perform dualpresolve on set-packing constraints with exactly two
        // variables.
        if !donotaggr && cd(scip, cons).nvars == 2 && dualpresolvingenabled && cd(scip, cons).setppctype == SetppcType::Packing {
            let mut objval = 0.0;

            let var = cd(scip, cons).vars[0usize];
            crate::var::var_get_aggregated_obj(scip, var, &mut objval)?;

            let nuplocks = locksup(scip, var, M);

            if nuplocks == 1 && objval <= 0.0 {
                // Perform aggregation on variables resulting from a set-packing
                // constraint.
                let other = cd(scip, cons).vars[1usize];
                let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, var, other, 1.0, 1.0, 1.0)?;

                if infeasible {
                    *cutoff = true;
                    break;
                }

                if aggregated {
                    *naggrvars += 1;

                    scip_cons::del_cons(scip, cons)?;
                    *ndelconss += 1;
                }

                continue;
            } else {
                let var = cd(scip, cons).vars[1usize];
                crate::var::var_get_aggregated_obj(scip, var, &mut objval)?;

                let nuplocks = locksup(scip, var, M);

                if nuplocks == 1 && objval <= 0.0 {
                    // Perform aggregation on variables resulting from a set-packing
                    // constraint.
                    let other = cd(scip, cons).vars[0usize];
                    let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, var, other, 1.0, 1.0, 1.0)?;

                    if infeasible {
                        *cutoff = true;
                        break;
                    }

                    if aggregated {
                        *naggrvars += 1;

                        scip_cons::del_cons(scip, cons)?;
                        *ndelconss += 1;
                    }

                    continue;
                }
            }
        } else if !donotaggr && cd(scip, cons).nvars == 2 && cd(scip, cons).setppctype == SetppcType::Partitioning {
            // Perform aggregation on variables resulting from a set-partitioning
            // constraint.
            let (v0, v1) = (cd(scip, cons).vars[0usize], cd(scip, cons).vars[1usize]);
            let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, v0, v1, 1.0, 1.0, 1.0)?;

            if infeasible {
                *cutoff = true;
                break;
            }

            if aggregated {
                *naggrvars += 1;

                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            }

            continue;
        }

        // We already found all possible variables for multi-aggregation.
        if ndecs >= nposvars {
            continue;
        }

        // No multi aggregation is allowed, so we can continue.
        if donotmultaggr {
            continue;
        }

        // Reset aggregation information.
        let mut aggregated = false;
        let mut aggrconsindex: i32 = -1;
        let mut aggrallyindex: i32 = -1;

        // Search for possible variables for multi-aggregation.
        let mut v = cd(scip, cons).nvars - 1;
        while v >= 0 {
            let var = cd(scip, cons).vars[v];

            let nuplocks = locksup(scip, var, M);
            let ndownlocks = locksdown(scip, var, M);
            let setppctype = cd(scip, cons).setppctype;

            if dualpresolvingenabled && setppctype == SetppcType::Packing && nuplocks <= 1 && nuplocks + ndownlocks <= 2 {
                // We found a redundant variable in a set-partitioning constraint.
                if ndownlocks == 0 {
                    let mut objval = 0.0;

                    crate::var::var_get_aggregated_obj(scip, var, &mut objval)?;
                    ndecs += 1;

                    // If the objective value is >= 0 the fixing is normally done by the
                    // dualfix presolver.
                    if !scip.set.is_negative(objval) {
                        scip_var::fix_var(scip, var, 0.0)?;
                        *nfixedvars += 1;
                    } else {
                        // Perform aggregation on variables resulting from a set-packing
                        // constraint.
                        let vars = cd(scip, cons).vars.0[..cd(scip, cons).nvars as usize].to_vec();
                        let (infeasible, fixed) = multi_aggregate_binvar(scip, linearconshdlrexist, &vars, v as usize)?;

                        if infeasible {
                            *cutoff = true;
                            break;
                        }

                        if fixed {
                            *naggrvars += 1;

                            del_coef_pos(scip, cons, v)?;
                            *nchgcoefs += 1;
                        }
                    }
                } else if ndownlocks == 1 && scip.vars[var].negatedvar != NONE {
                    let negvar = scip.vars[var].negatedvar;

                    // If we found a new variable add it to the data.
                    if !vartoindex.exists(negvar as u64) {
                        nhashmapentries += 1;
                        let _ = vartoindex.insert(var as u64, nhashmapentries);

                        considxs[(nhashmapentries - 1) as usize] = c as i32;
                        posincons[(nhashmapentries - 1) as usize] = v;

                        v -= 1;
                        continue;
                    }

                    let image = vartoindex.get_image(negvar as u64).unwrap();

                    let consindex = considxs[(image - 1) as usize];

                    // If the following assert fails, the constraint was not merged, or
                    // something really strange happened.
                    ndecs += 1;

                    let mut varindex = posincons[(image - 1) as usize];
                    considxs[(image - 1) as usize] = -1;
                    posincons[(image - 1) as usize] = -1;
                    vartoindex.remove(negvar as u64);

                    // This constraint can already be required for a multi-aggregation or
                    // the other constraint removed.
                    let aggrcons = usefulconss[consindex as usize];
                    if aggregated || scip.conss[aggrcons].deleted {
                        v -= 1;
                        continue;
                    }

                    // Determine active variable and constraint that corresponds to.
                    let (activevar, multaggridx) = if scip.vars[var].varstatus == VarStatus::Negated {
                        (negvar, consindex)
                    } else {
                        (var, c as i32)
                    };
                    let objval = scip.vars[activevar].obj;

                    // Perform aggregation on variables resulting from a set-packing
                    // constraint.
                    let infeasible;
                    if (objval < 0.0) == (multaggridx == c as i32) {
                        let vars = cd(scip, cons).vars.0[..cd(scip, cons).nvars as usize].to_vec();
                        (infeasible, aggregated) = multi_aggregate_binvar(scip, linearconshdlrexist, &vars, v as usize)?;
                        aggrconsindex = c as i32;
                        aggrallyindex = consindex;
                    } else {
                        // We need to find the variable again if other multi-aggregations
                        // invalidated the position.
                        let a = cd(scip, aggrcons);
                        if varindex >= a.nvars || a.vars[varindex] != negvar {
                            // If the following assert is raised, then the constraint is
                            // redundant and we do not need to aggregate anymore and can
                            // delete this constraint.
                            let mut v2 = a.nvars - 1;
                            while v2 >= 0 {
                                if a.vars[v2] == negvar {
                                    break;
                                }
                                v2 -= 1;
                            }
                            varindex = v2;
                        }

                        let vars = a.vars.0[..a.nvars as usize].to_vec();
                        (infeasible, aggregated) = multi_aggregate_binvar(scip, linearconshdlrexist, &vars, varindex as usize)?;

                        aggrconsindex = consindex;
                        aggrallyindex = c as i32;
                    }

                    if infeasible {
                        *cutoff = true;
                        break;
                    }
                }
            }
            // We found a redundant variable in a set-partitioning constraint.
            else if setppctype == SetppcType::Partitioning && nuplocks == 1 && ndownlocks == 1 {
                // This constraint can already be required for a multi-aggregation.
                if aggregated {
                    v -= 1;
                    continue;
                }

                // Perform aggregation on variables resulting from a set-partitioning
                // constraint.
                let vars = cd(scip, cons).vars.0[..cd(scip, cons).nvars as usize].to_vec();
                let infeasible;
                (infeasible, aggregated) = multi_aggregate_binvar(scip, linearconshdlrexist, &vars, v as usize)?;
                ndecs += 1;

                if infeasible {
                    *cutoff = true;
                    break;
                }

                if aggregated {
                    *naggrvars += 1;

                    del_coef_pos(scip, cons, v)?;
                    *nchgcoefs += 1;

                    set_setppc_type(scip, cons, SetppcType::Packing)?;
                    *nchgsides += 1;

                    chgtype[c] = true;

                    aggregated = false;
                    break;
                }
            }
            // We might have found a redundant variable.
            else if ndownlocks <= 2 && nuplocks <= 2 && scip.vars[var].negatedvar != NONE {
                let negvar = scip.vars[var].negatedvar;

                // If we have two times the same variable in a set-partitioning
                // constraint, we cannot aggregate this.
                if vartoindex.exists(var as u64) {
                    let image = vartoindex.get_image(var as u64).unwrap();

                    considxs[(image - 1) as usize] = -1;
                    posincons[(image - 1) as usize] = -1;

                    vartoindex.remove(var as u64);

                    v -= 1;
                    continue;
                } else if setppctype == SetppcType::Partitioning {
                    // If we found a new variable add it to the data.
                    if !vartoindex.exists(negvar as u64) {
                        nhashmapentries += 1;
                        let _ = vartoindex.insert(var as u64, nhashmapentries);

                        considxs[(nhashmapentries - 1) as usize] = c as i32;
                        posincons[(nhashmapentries - 1) as usize] = v;

                        v -= 1;
                        continue;
                    }
                } else {
                    // If the negated variable occurs in an additional packing
                    // constraint, then we are no longer in a doubleton or singleton
                    // case; this case of two packing and one partitioning constraint
                    // (thus, 2 down- and 2 uplocks) cannot be handled by this routine,
                    // so the variable cannot be aggregated, see also #3752 and !3832;
                    // this situation is characterized by having 2 downlocks (one from
                    // the partitioning constraint, one from the negated variable in the
                    // other packing constraint; the current packing constraint (cons)
                    // does not contribute a downlock).
                    if ndownlocks >= 2 {
                        v -= 1;
                        continue;
                    }

                    // If the negated variable did not occur in a set partitioning
                    // constraint (those will be iterated over first), we cannot
                    // aggregate this variable.
                    if !vartoindex.exists(negvar as u64) {
                        v -= 1;
                        continue;
                    }
                }

                let image = vartoindex.get_image(negvar as u64).unwrap();

                let consindex = considxs[(image - 1) as usize];

                // If the following assert fails, the constraint was not merged, or
                // something really strange happened.
                ndecs += 1;

                let mut varindex = posincons[(image - 1) as usize];
                considxs[(image - 1) as usize] = -1;
                posincons[(image - 1) as usize] = -1;
                vartoindex.remove(negvar as u64);

                // This constraint can already be required for a multi-aggregation or
                // the other constraint removed.
                let aggrcons = usefulconss[consindex as usize];
                if aggregated || scip.conss[aggrcons].deleted {
                    v -= 1;
                    continue;
                }

                // Must not multi-aggregate variables that are locked more then twice
                // by all setppc constraints.
                if setppctype == SetppcType::Packing && cd(scip, aggrcons).setppctype == SetppcType::Packing {
                    v -= 1;
                    continue;
                }

                // Perform aggregation on variables resulting from a set-partitioning
                // constraint.
                let infeasible;
                if chgtype[consindex as usize] {
                    let vars = cd(scip, cons).vars.0[..cd(scip, cons).nvars as usize].to_vec();
                    (infeasible, aggregated) = multi_aggregate_binvar(scip, linearconshdlrexist, &vars, v as usize)?;

                    aggrconsindex = c as i32;
                    aggrallyindex = consindex;
                } else {
                    // @note it might have happened that we have a variable at hand which
                    // exists actually in a set-packing constraint and due to some other
                    // aggregation we increased the number of locks and reached this part
                    // of the code, where we would expect only set-partitioning
                    // constraints in general, so in such a strange case we cannot
                    // aggregate anything.
                    if cd(scip, aggrcons).setppctype != SetppcType::Partitioning {
                        v -= 1;
                        continue;
                    }

                    // We need to find the variable again if other multi-aggregations
                    // invalidated the position.
                    let a = cd(scip, aggrcons);
                    if varindex >= a.nvars || a.vars[varindex] != negvar {
                        // If the following assert is raised, then the constraint is
                        // redundant and we do not need to aggregate anymore and can delete
                        // this constraint.
                        let mut v2 = a.nvars - 1;
                        while v2 >= 0 {
                            if a.vars[v2] == negvar {
                                break;
                            }
                            v2 -= 1;
                        }
                        varindex = v2;
                    }

                    let vars = a.vars.0[..a.nvars as usize].to_vec();
                    (infeasible, aggregated) = multi_aggregate_binvar(scip, linearconshdlrexist, &vars, varindex as usize)?;

                    aggrconsindex = consindex;
                    aggrallyindex = c as i32;
                }

                if infeasible {
                    *cutoff = true;
                    break;
                }
            }
            v -= 1;
        }

        if *cutoff {
            break;
        }

        if aggregated {
            *naggrvars += 1;

            let cons = usefulconss[aggrallyindex as usize];

            // Update the variables.
            *cutoff = apply_fixings(scip, cons, Some((&mut nlocaladdconss, &mut *ndelconss, &mut *nfixedvars)))?;
            if *cutoff {
                break;
            }

            // Merging unmerged constraints.
            *cutoff = merge_multiples(scip, cons, nfixedvars, ndelconss, nchgcoefs)?;
            if *cutoff {
                break;
            }

            // Update hashmap information.
            if !scip.conss[cons].deleted {
                let aggrcons = usefulconss[aggrconsindex as usize];
                for v in (0..cd(scip, aggrcons).nvars).rev() {
                    let var = cd(scip, aggrcons).vars[v];
                    if vartoindex.exists(var as u64) {
                        // Variable moved to ally constraint to unknown value position.
                        let image = vartoindex.get_image(var as u64).unwrap();
                        considxs[(image - 1) as usize] = aggrallyindex;
                        posincons[(image - 1) as usize] = 0;
                    }
                }
            }

            scip_cons::del_cons(scip, usefulconss[aggrconsindex as usize])?;
            *ndelconss += 1;
        }
    }

    Ok(())
}

/// `detectRedundantConstraints`: compares each constraint with all other
/// constraints for possible redundancy and removes or changes constraint
/// accordingly; in contrast to removeRedundantConstraints(), it uses a hash
/// table.
fn detect_redundant_constraints(scip: &mut Scip, conss: &[ConsId], firstchange: &mut i32, ndelconss: &mut i32, nchgsides: &mut i32) -> ScipResult {
    if conss.is_empty() {
        return Ok(());
    }

    // Create a hash table for the constraint set.
    let mut hashtablesize = conss.len() as i32;
    hashtablesize = crate::def::max(hashtablesize, HASHSIZE_SETPPCCONS);
    let mut hashtable = crate::hash::HashTable::<ConsId>::new(hashtablesize);

    // Check all constraints in the given set for redundancy.
    for &cons0 in conss {
        if !scip.conss[cons0].is_active() || scip.conss[cons0].modifiable {
            continue;
        }

        // Get constraint from current hash table with same variables as cons0 and
        // with coefficients either equal or negated to the ones of cons0.
        consdata_sort_cons(scip, cons0);
        let cons1 = hashtable.retrieve(&SetppcconsHash { scip }, &cons0);

        if let Some(cons1) = cons1 {
            // Constraint found: create a new constraint with same coefficients and
            // best left and right hand side; delete old constraints afterwards.

            // If necessary change type of setppc constraint.
            let (type0, type1) = (cd(scip, cons0).setppctype, cd(scip, cons1).setppctype);
            if type1 != SetppcType::Partitioning && type0 != type1 {
                // Change the type of cons0.
                set_setppc_type(scip, cons1, SetppcType::Partitioning)?;
                *nchgsides += 1;
            }

            // Update flags of constraint which caused the redundancy s.t.
            // nonredundant information doesn't get lost.
            scip_cons::update_cons_flags(scip, cons1, cons0)?;

            // Delete cons0.
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
            hashtable.insert(&SetppcconsHash { scip }, cons0);
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

/// `fixAdditionalVars`: for cons0 contained in cons1, fixes variables of
/// cons1 that are not in cons0 to zero.
fn fix_additional_vars(scip: &mut Scip, cons0: ConsId, cons1: ConsId, cutoff: &mut bool, nfixedvars: &mut i32) -> ScipResult {
    *cutoff = false;

    let idx = |scip: &Scip, cons: ConsId, v: i32| scip.vars[cd(scip, cons).vars[v]].index;

    // Fix variables in the range of cons0.
    let mut v0 = 0;
    let mut v1 = 0;
    while v0 < cd(scip, cons0).nvars && !*cutoff {
        let index0 = idx(scip, cons0, v0);
        while idx(scip, cons1, v1) < index0 && !*cutoff {
            // Fix variable to zero.
            let var = cd(scip, cons1).vars[v1];
            let fixed;
            (*cutoff, fixed) = scip_var::fix_var(scip, var, 0.0)?;
            if fixed {
                *nfixedvars += 1;
            }
            v1 += 1;
        }
        v0 += 1;
        v1 += 1;
    }

    // Fix remaining variables of cons1.
    while v1 < cd(scip, cons1).nvars && !*cutoff {
        // Fix variable to zero.
        let var = cd(scip, cons1).vars[v1];
        let fixed;
        (*cutoff, fixed) = scip_var::fix_var(scip, var, 0.0)?;
        if fixed {
            *nfixedvars += 1;
        }
        v1 += 1;
    }

    Ok(())
}

/// `processContainedCons`: applies reductions for cons0 being contained in
/// cons1.
fn process_contained_cons(scip: &mut Scip, cons0: ConsId, cons1: ConsId, cutoff: &mut bool, nfixedvars: &mut i32, ndelconss: &mut i32, nchgsides: &mut i32) -> ScipResult {
    *cutoff = false;

    match (cd(scip, cons0).setppctype, cd(scip, cons1).setppctype) {
        (SetppcType::Partitioning, SetppcType::Partitioning | SetppcType::Packing) => {
            // cons0: partitioning, cons1: partitioning or packing
            // -> fix additional variables in cons1 to zero, remove cons1
            fix_additional_vars(scip, cons0, cons1, cutoff, nfixedvars)?;
            remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
        }
        (SetppcType::Partitioning, SetppcType::Covering) => {
            // cons0: partitioning, cons1: covering
            // -> remove cons1
            remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
        }
        (SetppcType::Packing, SetppcType::Partitioning | SetppcType::Packing) => {
            // cons0: packing, cons1: partitioning or packing
            // -> remove cons0
            remove_redundant_cons(scip, cons1, cons0, ndelconss)?;
        }
        (SetppcType::Packing, SetppcType::Covering) => {
            // cons0: packing, cons1: covering
            // -> nothing can be deduced
        }
        (SetppcType::Covering, SetppcType::Partitioning | SetppcType::Packing) => {
            // cons0: covering, cons1: partitioning or packing
            // -> fix additional variables in cons1 to zero, remove cons1, convert
            //    cons0 into partitioning
            fix_additional_vars(scip, cons0, cons1, cutoff, nfixedvars)?;
            set_setppc_type(scip, cons0, SetppcType::Partitioning)?;
            remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
            *nchgsides += 1;
        }
        (SetppcType::Covering, SetppcType::Covering) => {
            // cons0: covering, cons1: covering
            // -> remove cons1
            remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
        }
    }

    Ok(())
}

/// `removeRedundantConstraints`: deletes redundant constraints.
#[allow(clippy::too_many_arguments)]
fn remove_redundant_constraints(
    scip: &mut Scip,
    conss: &[ConsId],
    firstchange: i32,
    chkind: i32,
    cutoff: &mut bool,
    nfixedvars: &mut i32,
    ndelconss: &mut i32,
    nchgsides: &mut i32,
) -> ScipResult {
    *cutoff = false;

    // Get the constraint to be checked against all prior constraints.
    let cons0 = conss[chkind as usize];

    // Sort the constraint cons0.
    consdata_sort_cons(scip, cons0);

    // Get the bit signature of the constraint.
    let signature0 = cons_get_signature(scip, cons0);

    // Check constraint against all prior constraints.
    let cons0changed = cd(scip, cons0).changed;
    cdm(scip, cons0).changed = false;
    let mut c = if cons0changed { 0 } else { firstchange };
    while c < chkind && !*cutoff && scip.conss[cons0].is_active() {
        let cons1 = conss[c as usize];
        c += 1;

        // Ignore inactive and modifiable constraints.
        if !scip.conss[cons1].is_active() || scip.conss[cons1].modifiable {
            continue;
        }

        // Sort the constraint cons1.
        consdata_sort_cons(scip, cons1);

        // Get the bit signature of cons1.
        let signature1 = cons_get_signature(scip, cons1);

        // Check (based on signature) if the two constraints are not included in
        // each other.
        let jointsignature = signature0 | signature1;
        if jointsignature != signature0 && jointsignature != signature1 {
            continue;
        }

        // Check whether one constraint is really a subset of the other.
        let consdata0 = cd(scip, cons0);
        let consdata1 = cd(scip, cons1);
        let idx0 = |v: i32| scip.vars[consdata0.vars[v]].index;
        let idx1 = |v: i32| scip.vars[consdata1.vars[v]].index;
        let mut cons0iscontained = consdata0.nvars <= consdata1.nvars;
        let mut cons1iscontained = consdata1.nvars <= consdata0.nvars;
        let mut v0 = 0;
        let mut v1 = 0;
        while v0 < consdata0.nvars && v1 < consdata1.nvars {
            let index0 = idx0(v0);
            let index1 = idx1(v1);
            if index0 < index1 {
                cons0iscontained = false;
                if !cons1iscontained {
                    break;
                }
                v0 += 1;
                while v0 < consdata0.nvars && idx0(v0) < index1 {
                    v0 += 1;
                }
            } else if index1 < index0 {
                cons1iscontained = false;
                if !cons0iscontained {
                    break;
                }
                v1 += 1;
                while v1 < consdata1.nvars && idx1(v1) < index0 {
                    v1 += 1;
                }
            } else {
                v0 += 1;
                v1 += 1;
            }
        }
        cons0iscontained = cons0iscontained && (v0 == consdata0.nvars);
        cons1iscontained = cons1iscontained && (v1 == consdata1.nvars);
        let (type0, type1) = (consdata0.setppctype, consdata1.setppctype);

        if cons0iscontained && cons1iscontained {
            // Both constraints consists of the same variables.
            if type0 == type1 {
                // Both constraints are equal: update flags in cons0 and delete cons1.
                remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
            } else if type0 == SetppcType::Partitioning {
                // The set partitioning constraint is stronger: remove the other one.
                remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
            } else if type1 == SetppcType::Partitioning {
                // The set partitioning constraint is stronger: remove the other one.
                remove_redundant_cons(scip, cons1, cons0, ndelconss)?;
            } else {
                // One is a covering, the other one a packing constraint: replace them
                // by a single partitioning constraint.

                // Change the type of cons0.
                set_setppc_type(scip, cons0, SetppcType::Partitioning)?;
                *nchgsides += 1;

                // Delete cons1.
                remove_redundant_cons(scip, cons0, cons1, ndelconss)?;
            }
        } else if cons0iscontained {
            // cons0 is contained in cons1.
            process_contained_cons(scip, cons0, cons1, cutoff, nfixedvars, ndelconss, nchgsides)?;
        } else if cons1iscontained {
            // cons1 is contained in cons1.
            process_contained_cons(scip, cons1, cons0, cutoff, nfixedvars, ndelconss, nchgsides)?;
        }
    }

    Ok(())
}

/// `performVarDeletions`: perform deletion of variables in all constraints
/// of the constraint handler.
fn perform_var_deletions(scip: &mut Scip, conss: &[ConsId]) -> ScipResult {
    // Iterate over all constraints.
    for &cons in conss {
        // Constraint is marked, that some of its variables were deleted.
        if cd(scip, cons).varsdeleted {
            // Iterate over all variables of the constraint and delete marked
            // variables.
            for v in (0..cd(scip, cons).nvars).rev() {
                if scip.vars[cd(scip, cons).vars[v]].deleted {
                    del_coef_pos(scip, cons, v)?;
                }
            }
            cdm(scip, cons).varsdeleted = false;
        }
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce constraints.
fn enforce_constraint(scip: &mut Scip, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let mut cutoff = false;
    let mut separated = false;
    let mut reduceddom = false;

    // Check all useful set partitioning / packing / covering constraints for
    // feasibility.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff && !reduceddom {
        separate_cons(scip, conss[c], sol, true, &mut cutoff, &mut separated, &mut reduceddom)?;
        c += 1;
    }

    // Check all obsolete set partitioning / packing / covering constraints for
    // feasibility.
    let mut c = nusefulconss as usize;
    while c < conss.len() && !cutoff && !separated && !reduceddom {
        separate_cons(scip, conss[c], sol, true, &mut cutoff, &mut separated, &mut reduceddom)?;
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
 * upgrading of linear constraints
 */

/// `createConsSetppc`: creates and captures a set partitioning / packing /
/// covering constraint.
fn create_cons_setppc(scip: &mut Scip, name: &str, vars: &[VarId], setppctype: SetppcType, flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the set partitioning constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Set partitioning / packing / covering constraint handler not found.
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
    let consdata = if scip.set.stage == Stage::Problem {
        // Create constraint in original problem.
        consdata_create(scip, vars, setppctype)?
    } else {
        // Create constraint in transformed problem.
        consdata_create_transformed(scip, vars, setppctype)?
    };

    // Create constraint.
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)?;

    if api::is_transformed(scip) && setppctype == SetppcType::Partitioning {
        hdm(scip, conshdlr).nsetpart += 1;
    }

    if scip.set.stage != Stage::Problem {
        // Get event handler.
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        // Catch bound change events of variables.
        catch_all_events(scip, cons, eventhdlr)?;
    }

    Ok(cons)
}

/// `createNormalizedSetppc`: creates and captures a normalized (with all
/// coefficients +1) setppc constraint.
fn create_normalized_setppc(scip: &mut Scip, name: &str, vars: &[VarId], vals: &[f64], mult: i32, setppctype: SetppcType, flags: &ConsFlags) -> ScipResult<ConsId> {
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
    create_cons_setppc(scip, name, &transvars, setppctype, flags)
}

/// `linconsUpgdSetppc`: check, if linear constraint can be upgraded to set
/// partitioning, packing, or covering constraint
/// - all set partitioning / packing / covering constraints consist only of
///   binary variables with a coefficient of +1.0 or -1.0 (variables with -1.0
///   coefficients can be negated):
///   lhs    <= x1 + ... + xp - y1 - ... - yn <= rhs
/// - negating all variables y = (1-Y) with negative coefficients gives:
///   lhs + n <= x1 + ... + xp + Y1 + ... + Yn <= rhs + n
/// - negating all variables x = (1-X) with positive coefficients and
///   multiplying with -1 gives:
///   p - rhs <= X1 + ... + Xp + y1 + ... + yn <= p - lhs
/// - a set partitioning constraint has left hand side of +1.0, and right
///   hand side of +1.0 : x(S) == 1.0
///   -> without negations:  lhs == rhs == 1 - n  or  lhs == rhs == p - 1
/// - a set packing constraint has left hand side of -infinity, and right
///   hand side of +1.0 : x(S) <= 1.0
///   -> without negations:  (lhs == -inf  and  rhs == 1 - n)  or  (lhs == p
///   - 1  and  rhs = +inf)
/// - a set covering constraint has left hand side of +1.0, and right hand
///   side of +infinity: x(S) >= 1.0
///   -> without negations:  (lhs == 1 - n  and  rhs == +inf)  or  (lhs ==
///   -inf  and  rhs = p - 1)
fn lincons_upgd_setppc(scip: &mut Scip, cons: ConsId, s: &crate::cons_linear::LinConsStats, upgdcons: &mut ConsId) -> ScipResult {
    let nvars = s.vars.len() as i32;
    let (lhs, rhs) = (s.lhs, s.rhs);
    let ncoeffsnone = s.ncoeffsnone as f64;
    let ncoeffspone = s.ncoeffspone as f64;

    if s.nposbin + s.nnegbin + s.nposimplbin + s.nnegimplbin == nvars && s.ncoeffspone + s.ncoeffsnone == nvars {
        let set = &scip.set;
        let (setppctype, mult) = if set.is_eq(lhs, rhs) && (set.is_eq(lhs, 1.0 - ncoeffsnone) || set.is_eq(lhs, ncoeffspone - 1.0)) {
            // Check, if we have to multiply with -1 (negate the positive vars) or
            // with +1 (negate the negative vars).
            (SetppcType::Partitioning, if set.is_eq(lhs, 1.0 - ncoeffsnone) { 1 } else { -1 })
        } else if (set.is_infinity(-lhs) && set.is_eq(rhs, 1.0 - ncoeffsnone)) || (set.is_eq(lhs, ncoeffspone - 1.0) && set.is_infinity(rhs)) {
            (SetppcType::Packing, if set.is_infinity(-lhs) { 1 } else { -1 })
        } else if (set.is_eq(lhs, 1.0 - ncoeffsnone) && set.is_infinity(rhs)) || (set.is_infinity(-lhs) && set.is_eq(rhs, ncoeffspone - 1.0)) {
            (SetppcType::Covering, if set.is_infinity(rhs) { 1 } else { -1 })
        } else {
            return Ok(());
        };

        // Create the set partitioning / packing / covering constraint (an
        // automatically upgraded constraint is always unmodifiable).
        let name = scip.conss[cons].name.clone();
        let flags = cons_flags(scip, cons);
        *upgdcons = create_normalized_setppc(scip, &name, &s.vars, &s.vals, mult, setppctype, &flags)?;
    }

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `consFreeSetppc`: destructor of constraint handler to free constraint
/// handler data (called when SCIP is exiting).
fn cons_free_setppc(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    // Free constraint handler data.
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consInitSetppc`: initialization method of constraint handler (called
/// after problem was transformed).
fn cons_init_setppc(scip: &mut Scip, conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    let h = hdm(scip, conshdlr);
    h.noldfixedvars = 0;
    h.noldimpls = 0;
    h.noldcliques = 0;
    h.noldupgrs = 0;
    h.nclqpresolve = 0;
    h.updatedsetppctype = false;
    h.enablecliquelifting = true;

    Ok(())
}

/// `consExitpreSetppc`: presolving deinitialization method of constraint
/// handler (called after presolving has been finished).
fn cons_exitpre_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    for &cons in conss {
        if !scip.conss[cons].deleted {
            // We are not allowed to detect infeasibility in the exitpre stage.
            apply_fixings(scip, cons, None)?;
        }
    }

    Ok(())
}

/// `consInitsolSetppc`: adds the nlrows to the NLP if it had been
/// constructed, which the port never does (the NLP is not ported).
fn cons_initsol_setppc(_scip: &mut Scip, _conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    Ok(())
}

/// `consExitsolSetppc`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], _restart: bool) -> ScipResult {
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

/// `consDeleteSetppc`: frees specific constraint data.
fn cons_delete_setppc(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    // Get event handler.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    scip.conss[cons].consdata = consdata.take();

    if api::is_transformed(scip) && cd(scip, cons).setppctype == SetppcType::Partitioning {
        hdm(scip, conshdlr).nsetpart -= 1;
    }

    // If constraint belongs to transformed problem space, drop bound change
    // events on variables.
    if cd(scip, cons).nvars > 0 && crate::var::var_is_transformed(scip, cd(scip, cons).vars[0usize]) {
        drop_all_events(scip, cons, eventhdlr)?;
    }

    // Free setppc constraint data.
    let mut data = scip.conss[cons].consdata.take().unwrap();
    consdata_free(scip, data.downcast_mut::<ConsData>().unwrap())
}

/// `consTransSetppc`: transforms constraint data into data belonging to the
/// transformed problem.
fn cons_trans_setppc(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    // Get event handler.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let s = cd(scip, sourcecons);
    let vars = s.vars.0[..s.nvars as usize].to_vec();
    let setppctype = s.setppctype;

    // Create constraint data for target constraint.
    let targetdata = consdata_create_transformed(scip, &vars, setppctype)?;

    // Create target constraint.
    let name = scip.conss[sourcecons].name.clone();
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    if setppctype == SetppcType::Partitioning {
        hdm(scip, conshdlr).nsetpart += 1;
    }

    // Catch bound change events of variables.
    catch_all_events(scip, *targetcons, eventhdlr)
}

/// `consInitlpSetppc`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
fn cons_initlp_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let mut c = 0;
    while c < conss.len() && !*infeasible {
        *infeasible = add_cut(scip, conss[c])?;
        c += 1;
    }

    Ok(())
}

/// The shared body of `consSepalpSetppc` and `consSepasolSetppc`.
fn sepa_setppc(scip: &mut Scip, conss: &[ConsId], nusefulconss: i32, sol: SolId, lpfeas: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    let mut cutoff = false;
    let mut separated = false;
    let mut reduceddom = false;

    // Check all useful set partitioning / packing / covering constraints for
    // feasibility.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff {
        separate_cons(scip, conss[c], sol, lpfeas, &mut cutoff, &mut separated, &mut reduceddom)?;
        c += 1;
    }

    // Combine set partitioning / packing / covering constraints to get more
    // cuts; @todo further cuts of set partitioning / packing / covering
    // constraints.

    // Return the correct result.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if reduceddom {
        *result = ScipResultCode::ReducedDom;
    } else if separated {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/// `consSepalpSetppc`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    sepa_setppc(scip, conss, nusefulconss, NONE, true, result)
}

/// `consSepasolSetppc`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    sepa_setppc(scip, conss, nusefulconss, sol, false, result)
}

/// `consEnfolpSetppc`: constraint enforcing method of constraint handler for
/// LP solutions.
fn cons_enfolp_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conss, nusefulconss, NONE, result)
}

/// `consEnforelaxSetppc`: constraint enforcing method of constraint handler
/// for relaxation solutions.
fn cons_enforelax_setppc(scip: &mut Scip, sol: SolId, _conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conss, nusefulconss, sol, result)
}

/// `consEnfopsSetppc`: constraint enforcing method of constraint handler
/// for pseudo solutions (pseudo branching needs `VARUSES`, compiled out).
#[allow(clippy::too_many_arguments)]
fn cons_enfops_setppc(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let mut cutoff = false;
    let mut infeasible = false;
    let mut reduceddom = false;
    let mut solvelp = false;

    // Check all set partitioning / packing / covering constraints for
    // feasibility.
    let mut c = 0;
    while c < conss.len() && !cutoff && !reduceddom && !solvelp {
        enforce_pseudo(scip, conss[c], &mut cutoff, &mut infeasible, &mut reduceddom, &mut solvelp)?;
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

/// `consCheckSetppc`: feasibility check method of constraint handler for
/// integral solutions; printing the reason is not ported.
#[allow(clippy::too_many_arguments)]
fn cons_check_setppc(
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

    // Check all set partitioning / packing / covering constraints for
    // feasibility.
    let mut c = 0;
    while c < conss.len() && (*result == ScipResultCode::Feasible || completely) {
        let cons = conss[c];
        let row = cd(scip, cons).row;
        if (checklprows || row == NONE || scip.rows[row].lppos < 0) && !check_cons(scip, cons, sol) {
            // Constraint is violated.
            *result = ScipResultCode::Infeasible;
        }
        c += 1;
    }

    Ok(())
}

/// `consPropSetppc`: domain propagation method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_prop_setppc(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut nfixedvars = 0;

    *result = ScipResultCode::DidNotFind;

    let mut cutoff = false;
    let inpresolve = scip.set.stage < Stage::InitSolve;

    // Propagate all marked set partitioning / packing / covering constraints.
    let mut c = nmarkedconss - 1;
    while c >= 0 && !cutoff {
        let cons = conss[c as usize];

        // During presolving, we do not want to propagate constraints with
        // multiaggregated variables. After presolving, we want to resolve the
        // multiaggregation to have a clean data structure; All initial
        // constraints should not have multiaggregated variables, but this is not
        // true for constraints that were introduced during solving.
        if cd(scip, cons).existmultaggr {
            if inpresolve {
                c -= 1;
                continue;
            }

            let mut naddconss = 0;
            let mut ndelconss = 0;
            cutoff = apply_fixings(scip, cons, Some((&mut naddconss, &mut ndelconss, &mut nfixedvars)))?;

            if cutoff {
                break;
            }
        }

        // All multiaggregations should be resolved at here.
        process_fixings(scip, cons, &mut cutoff, &mut nfixedvars)?;

        cons_unmark_propagate(scip, cons);
        c -= 1;
    }

    // Return the correct result.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if nfixedvars > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `consPresolSetppc`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_setppc(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nrounds: i32,
    presoltiming: PresolTiming,
    newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    let nconss = conss.len() as i32;

    *result = ScipResultCode::DidNotFind;

    let mut oldnfixedvars = counts.nfixedvars;
    let mut oldndelconss = counts.ndelconss;
    let oldnaggrvars = counts.naggrvars;
    let oldnchgcoefs = counts.nchgcoefs;
    let mut cutoff = false;

    // Determine whether we want to run the clique lifting procedure.
    let nfixed = api::get_n_fixed_vars(scip);
    let nimpls = api::get_n_implications(scip);
    let ncliques = scip_var::get_n_cliques(scip);
    let h = hdm(scip, conshdlr);
    h.enablecliquelifting =
        h.enablecliquelifting || h.updatedsetppctype || h.noldfixedvars != nfixed || h.noldimpls != nimpls || h.noldcliques != ncliques || h.noldupgrs != nconss;

    // Remember old values.
    let startdelconss = counts.ndelconss;
    h.noldimpls = nimpls;
    h.noldcliques = ncliques;
    h.updatedsetppctype = false;

    let (dualpresolving, presolusehashing, presolpairwise, cliquelifting) = (h.dualpresolving, h.presolusehashing, h.presolpairwise, h.cliquelifting);

    // Process constraints.
    let mut firstchange = i32::MAX;
    let mut firstclique = i32::MAX;
    let mut lastclique = -1;
    let mut c = 0;
    while c < nconss && !crate::solve::solve_is_stopped(scip, false) {
        let cons = conss[c as usize];
        c += 1;

        // Remove all variables that are fixed to zero and replace all aggregated
        // variables.
        if cd(scip, cons).nfixedzeros > 0
            || newcounts.naggrvars > 0
            || newcounts.naddconss > 0
            || newcounts.nupgdconss > 0
            || counts.naggrvars > oldnaggrvars
            || (nrounds == 0 && api::get_n_runs(scip) > 1)
        {
            cutoff = apply_fixings(scip, cons, Some((&mut counts.naddconss, &mut counts.ndelconss, &mut counts.nfixedvars)))?;

            if cutoff {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }

            if scip.conss[cons].deleted {
                continue;
            }
        }

        // Find pairs of negated variables in constraint:
        // partitioning/packing: all other variables must be zero, constraint is
        // redundant
        // covering: constraint is redundant
        //
        // find sets of equal variables in constraint:
        // partitioning/packing: variable must be zero
        // covering: multiple entries of variable can be replaced by single entry
        cutoff = merge_multiples(scip, cons, &mut counts.nfixedvars, &mut counts.ndelconss, &mut counts.nchgcoefs)?;

        if cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }

        // If constraint was deleted while merging, go to the next constraint.
        if !scip.conss[cons].is_active() {
            continue;
        }

        // Remove fixings found by merging.
        if cd(scip, cons).nfixedzeros > 0 {
            cutoff = apply_fixings(scip, cons, Some((&mut counts.naddconss, &mut counts.ndelconss, &mut counts.nfixedvars)))?;

            if cutoff {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }

            if scip.conss[cons].deleted {
                continue;
            }
        }

        // Check if constraint is already redundant or infeasible due to fixings,
        // fix or aggregate left over variables if possible.
        presolve_propagate_cons(scip, cons, true, &mut counts.nfixedvars, &mut counts.naggrvars, &mut counts.ndelconss, &mut cutoff)?;

        if cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }

        // If constraint was deleted while propagation, go to the next constraint.
        if !scip.conss[cons].is_active() {
            continue;
        }

        // Remove fixings found by presolvePropagateCons().
        if cd(scip, cons).nfixedzeros > 0 {
            cutoff = apply_fixings(scip, cons, Some((&mut counts.naddconss, &mut counts.ndelconss, &mut counts.nfixedvars)))?;

            if cutoff {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }

            if scip.conss[cons].deleted {
                continue;
            }
        }

        // Perform dual reductions.
        if dualpresolving && scip_var::allow_strong_dual_reds(scip) {
            if dual_presolving(scip, cons, &mut counts.nfixedvars, &mut counts.ndelconss, &mut counts.naggrvars)? {
                *result = ScipResultCode::Success;
            }

            // If dual reduction deleted the constraint we take the next.
            if !scip.conss[cons].is_active() {
                continue;
            }
        }

        // Remember the first changed constraint to begin the next redundancy
        // round with.
        if firstchange == i32::MAX && cd(scip, cons).changed {
            firstchange = c - 1;
        }

        // Remember the first and last constraints for which we have to add the
        // clique information.
        if !cd(scip, cons).cliqueadded && cd(scip, cons).nvars >= 2 {
            if firstclique == i32::MAX {
                firstclique = c - 1;
            }
            lastclique = c - 1;
        }
    }

    // Update result pointer.
    if oldnfixedvars < counts.nfixedvars || oldnaggrvars < counts.naggrvars || oldndelconss < counts.ndelconss || oldnchgcoefs < counts.nchgcoefs {
        *result = ScipResultCode::Success;
    }

    if firstchange < nconss && presolusehashing {
        // Detect redundant constraints; fast version with hash table instead of
        // pairwise comparison.
        detect_redundant_constraints(scip, conss, &mut firstchange, &mut counts.ndelconss, &mut counts.nchgsides)?;
        if oldndelconss < counts.ndelconss {
            *result = ScipResultCode::Success;
        }
    }

    // Determine singleton variables in set-partitioning/-packing constraints,
    // or doubleton variables (active and negated) in any combination of
    // set-partitioning and set-packing constraints.
    let h = hd(scip, conshdlr);
    let (nsetpart, haslinear) = (h.nsetpart, h.conshdlrlinear != NONE);
    if nconss > 1
        && (presoltiming & PRESOLTIMING_MEDIUM) != 0
        && ((nsetpart > 0 && !scip_var::do_not_multaggr(scip) && haslinear)
            || (dualpresolving && scip_var::allow_strong_dual_reds(scip) && nsetpart < nconss as i64 && !scip_var::do_not_aggr(scip)))
    {
        let dualenabled = dualpresolving && scip_var::allow_strong_dual_reds(scip);
        remove_double_and_singletons_and_perform_dualpresolve(
            scip,
            conss,
            dualenabled,
            haslinear,
            &mut counts.nfixedvars,
            &mut counts.naggrvars,
            &mut counts.ndelconss,
            &mut counts.nchgcoefs,
            &mut counts.nchgsides,
            &mut cutoff,
        )?;

        if cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        } else if oldnfixedvars < counts.nfixedvars || oldnaggrvars < counts.naggrvars || oldndelconss < counts.ndelconss {
            *result = ScipResultCode::Success;
        }
    }

    // Clique lifting.
    if cliquelifting && hd(scip, conshdlr).enablecliquelifting && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
        unimplemented!("setppc clique lifting (constraints/setppc/cliquelifting) is not ported");
    }

    if oldndelconss == counts.ndelconss && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
        // Check constraints for redundancy.
        if presolpairwise {
            let mut npaircomparisons: i64 = 0;

            oldnfixedvars = counts.nfixedvars;

            let mut c = firstchange;
            while c < nconss && !crate::solve::solve_is_stopped(scip, false) {
                let cons = conss[c as usize];
                if scip.conss[cons].is_active() && !scip.conss[cons].modifiable {
                    npaircomparisons += if cd(scip, cons).changed { c as i64 } else { c as i64 - firstchange as i64 };

                    remove_redundant_constraints(
                        scip,
                        conss,
                        firstchange,
                        c,
                        &mut cutoff,
                        &mut counts.nfixedvars,
                        &mut counts.ndelconss,
                        &mut counts.nchgsides,
                    )?;
                    if cutoff {
                        *result = ScipResultCode::Cutoff;
                        return Ok(());
                    }

                    if npaircomparisons > NMINCOMPARISONS as i64 {
                        if ((counts.ndelconss - oldndelconss + counts.nfixedvars - oldnfixedvars) as f64) / (npaircomparisons as f64) < MINGAINPERNMINCOMPARISONS {
                            break;
                        }
                        oldndelconss = counts.ndelconss;
                        oldnfixedvars = counts.nfixedvars;
                        npaircomparisons = 0;
                        *result = ScipResultCode::Success;
                    }
                }
                c += 1;
            }
        }
    }

    // Add cliques after lifting variables.
    add_cliques(
        scip,
        conss,
        crate::def::min(firstclique, nconss),
        crate::def::min(lastclique, nconss),
        &mut counts.naddconss,
        &mut counts.ndelconss,
        &mut counts.nchgbds,
        &mut cutoff,
    )?;

    if cutoff {
        *result = ScipResultCode::Cutoff;
    }

    hdm(scip, conshdlr).noldupgrs = nconss - (counts.ndelconss - startdelconss);

    Ok(())
}

/// `consRespropSetppc`: propagation conflict resolving method of constraint
/// handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_setppc(
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
    let setppctype = cd(scip, cons).setppctype;
    if setppctype == SetppcType::Covering || (setppctype == SetppcType::Partitioning && api::get_var_lb_at_index(scip, infervar, bdchgidx.as_ref(), true) > 0.5) {
        // The inference constraint is a set partitioning or covering constraint
        // with the inference variable inferred to 1.0: the reason for the
        // deduction is the assignment of 0.0 to all other variables.
        for v in 0..cd(scip, cons).nvars {
            let var = cd(scip, cons).vars[v];
            if var != infervar {
                // The reason variable must be assigned to zero.
                api::add_conflict_binvar(scip, var)?;
            }
        }
    } else {
        // The inference constraint is a set partitioning or packing constraint
        // with the inference variable inferred to 0.0: the reason for the
        // deduction is the assignment of 1.0 to a single variable.
        if inferinfo >= 0 {
            let var = cd(scip, cons).vars[inferinfo];
            api::add_conflict_binvar(scip, var)?;
        } else {
            for v in 0..cd(scip, cons).nvars {
                let var = cd(scip, cons).vars[v];
                if api::get_var_lb_at_index(scip, var, bdchgidx.as_ref(), false) > 0.5 {
                    api::add_conflict_binvar(scip, var)?;
                    break;
                }
            }
        }
    }

    *result = ScipResultCode::Success;

    Ok(())
}

/// `consLockSetppc`: variable rounding lock method of constraint handler.
fn cons_lock_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let (nlocksdown, nlocksup) = match cd(scip, cons).setppctype {
        SetppcType::Partitioning => (nlockspos + nlocksneg, nlockspos + nlocksneg),
        SetppcType::Packing => (nlocksneg, nlockspos),
        SetppcType::Covering => (nlockspos, nlocksneg),
    };

    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        api::add_var_locks_type(scip, var, locktype, nlocksdown, nlocksup)?;
    }

    Ok(())
}

/// `consActiveSetppc`: constraint activation notification method of
/// constraint handler (the nlrow part is not ported).
fn cons_active_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    // We only might add the constraint to the propagation list, when we are
    // not activating it in probing mode.
    if scip.set.stage > Stage::Transforming {
        let c = cd(scip, cons);
        if c.nfixedones >= 1 || c.nfixedzeros >= c.nvars - 1 {
            cons_mark_propagate(scip, cons);
        }
    }

    Ok(())
}

/// `consDeactiveSetppc`: constraint deactivation notification method of
/// constraint handler.
fn cons_deactive_setppc(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    if scip.conss[cons].deleted {
        // Get event handler.
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        // If constraint belongs to transformed problem space, drop bound change
        // events on variables.
        if cd(scip, cons).nvars > 0 && crate::var::var_is_transformed(scip, cd(scip, cons).vars[0usize]) {
            drop_all_events(scip, cons, eventhdlr)?;
        }
    }

    // Remove row from NLP, if still in solving (the NLP is not ported).
    Ok(())
}

/// `consDelvarsSetppc`: variable deletion method of constraint handler.
fn cons_delvars_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if !conss.is_empty() {
        perform_var_deletions(scip, conss)?;
    }
    Ok(())
}

/// `consGetVarsSetppc`: constraint method of constraint handler which
/// returns the variables (the vector always fits here).
fn cons_get_vars_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.extend_from_slice(&c.vars.0[..c.nvars as usize]);
    *success = true;
    Ok(())
}

/// `consGetNVarsSetppc`: constraint method of constraint handler which
/// returns the number of variables.
fn cons_get_n_vars_setppc(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = cd(scip, cons).nvars;
    *success = true;
    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecSetppc`: execution method of bound change event handler; the
/// event data is the constraint.
fn event_exec_setppc(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let cons: ConsId = eventdata;
    let eventtype = event.eventtype;

    match eventtype {
        EVENTTYPE_LBTIGHTENED => cdm(scip, cons).nfixedones += 1,
        EVENTTYPE_LBRELAXED => cdm(scip, cons).nfixedones -= 1,
        EVENTTYPE_UBTIGHTENED => cdm(scip, cons).nfixedzeros += 1,
        EVENTTYPE_UBRELAXED => cdm(scip, cons).nfixedzeros -= 1,
        EVENTTYPE_VARDELETED => cdm(scip, cons).varsdeleted = true,
        EVENTTYPE_VARFIXED => {
            let var = event.data.var;
            if cd(scip, cons).merged {
                // This event should only arise during the presolving stage; one
                // variable was changed to a negated or aggregated variable, so maybe
                // we can merge again.
                let x = &scip.vars[var];
                if x.varstatus != VarStatus::Fixed && x.glbdom.lb < 0.5 && x.glbdom.ub > 0.5 {
                    cdm(scip, cons).merged = false;
                }
            }

            if !cd(scip, cons).existmultaggr {
                let probvar = crate::var::var_get_probvar(scip, var);
                if scip.vars[probvar].varstatus == VarStatus::MultAggr {
                    cdm(scip, cons).existmultaggr = true;
                }
            }
        }
        _ => {
            // Invalid event type.
            return Err(Retcode::InvalidData);
        }
    }

    if (eventtype & EVENTTYPE_BOUNDTIGHTENED) != 0 {
        let c = cd(scip, cons);
        if c.nfixedones >= 1 || c.nfixedzeros >= c.nvars - 1 {
            cdm(scip, cons).presolpropagated = false;
            cons_mark_propagate(scip, cons);
        } else if scip.set.stage < Stage::InitSolve && c.nfixedzeros >= c.nvars - 2 {
            cdm(scip, cons).presolpropagated = false;
        }
    }

    Ok(())
}

/*
 * Callback methods of conflict handler
 */

/// `conflictExecSetppc`: conflict processing method of conflict handler
/// (called when conflict was found).
fn conflict_exec_setppc(scip: &mut Scip, _conflicthdlr: crate::types::ConflicthdlrId, args: &crate::conflict::ConflictExecArgs, result: &mut ScipResultCode) -> ScipResult {
    // Don't process already resolved conflicts.
    if args.resolved {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let nbdchginfos = args.bdchginfos.len();
    let conflict_name = |scip: &Scip| {
        let napplied = scip.conflict.as_ref().map_or(0, |c| c.nappliedglbconss + c.nappliedlocconss);
        format!("cf{}_{}", api::get_n_runs(scip), napplied)
    };

    // For two (binary) variables we will create a set packing constraint and
    // add the clique information of the conflict is global.
    if nbdchginfos == 2 {
        let mut twovars = [NONE; 2];
        for k in 0..2 {
            let info = crate::conflict::bdchginfo(scip, args.bdchginfos[k]);
            twovars[k] = info.var;

            // We can only treat binary variables.
            if !crate::var::var_is_binary(scip, twovars[k]) {
                return Ok(());
            }

            // If the variable is fixed to zero in the conflict set, we have to use
            // its negation.
            if info.newbound < 0.5 {
                twovars[k] = scip_var::get_negated_var(scip, twovars[k])?;
            }
        }

        // Create a constraint out of the conflict set.
        let consname = conflict_name(scip);
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
        let mut cons = create_cons_setpack(scip, &consname, &twovars, &flags)?;

        // If the constraint gets globally added, we also add the clique
        // information.
        if !scip.conss[cons].is_local() {
            scip_var::add_clique(scip, &twovars, &[], false)?;
        }

        // Add conflict to SCIP.
        scip_cons::add_conflict(scip, args.node, &mut cons, args.validnode, args.conftype, args.cutoffinvolved)?;
        *result = ScipResultCode::ConsAdded;
        return Ok(());
    }

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
        let consname = conflict_name(scip);
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
        let mut cons = create_cons_setcover(scip, &consname, &vars, &flags)?;
        scip_cons::add_cons_node(scip, args.node, cons, args.validnode)?;
        scip_cons::release_cons(scip, &mut cons)?;
        *result = ScipResultCode::ConsAdded;
    }

    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `nonlinUpgdSetppc`: tries to upgrade a nonlinear constraint to a
/// setpacking constraint.
fn nonlin_upgd_setppc(scip: &mut Scip, cons: ConsId, nvarexprs: i32, nupgdconss: &mut i32, upgdconss: &mut Vec<ConsId>, _upgdconsssize: i32) -> ScipResult {
    *nupgdconss = 0;

    // Need exactly two variables.
    if nvarexprs != 2 {
        return Ok(());
    }

    // Left and right hand side need to be equal.
    // @todo we could also handle inequalities
    let mut rhs = crate::cons_nonlinear::get_rhs_nonlinear(scip, cons);
    if scip.set.is_infinity(rhs) || !scip.set.is_eq(crate::cons_nonlinear::get_lhs_nonlinear(scip, cons), rhs) {
        return Ok(());
    }

    // Check whether constraint is quadratic.
    let isquadratic = crate::cons_nonlinear::check_quadratic_nonlinear(scip, cons)?;
    if !isquadratic {
        return Ok(());
    }

    let expr = crate::cons_nonlinear::get_expr_nonlinear(scip, cons);
    let (constant, nlinexprs, nquadexprs, nbilinexprterms) = {
        let q = crate::expr::expr_get_quadratic_data(scip, expr);
        (q.constant, q.linexprs.len(), q.quadexprterms.len(), q.bilinexprterms.len())
    };

    // Adjust rhs.
    rhs -= constant;

    // Cannot currently handle linear part.
    if nlinexprs > 0 {
        return Ok(());
    }

    // Need only one bilinear term.
    if nbilinexprterms != 1 {
        return Ok(());
    }

    // Need exactly two quadratic variables.
    if nquadexprs != 2 {
        return Ok(());
    }

    // Get bilinear term.
    let (expr1, expr2, bilincoef) = {
        let b = &crate::expr::expr_get_quadratic_data(scip, expr).bilinexprterms[0];
        (b.expr1, b.expr2, b.coef)
    };
    let bilinvars = [crate::expr_var::get_var_expr_var(scip, expr1), crate::expr_var::get_var_expr_var(scip, expr2)];

    if scip.set.is_zero(bilincoef) {
        return Ok(());
    }

    // Check variable types.
    for &v in &bilinvars {
        if scip.vars[v].vartype != crate::types::VarType::Binary || crate::var::var_is_implied_integral(scip, v) {
            return Ok(());
        }
    }

    // Get data of quadratic terms; for binary variables, we can treat sqr coef
    // as lin coef.
    let (expr1, mut coefx, expr2, mut coefy) = {
        let q = crate::expr::expr_get_quadratic_data(scip, expr);
        let (t0, t1) = (&q.quadexprterms[0], &q.quadexprterms[1]);
        (t0.expr, t0.lincoef + t0.sqrcoef, t1.expr, t1.lincoef + t1.sqrcoef)
    };

    // Divide constraint by coefficient of x*y.
    coefx /= bilincoef;
    coefy /= bilincoef;
    rhs /= bilincoef;

    // Constraint is now of the form coefx * x + coefy * y + x * y == rhs; we
    // can rewrite as (x + coefy) * (y + coefx) == rhs + coefx * coefy.

    // We can only upgrade if coefx and coefy are 0 or -1 and rhs == -coefx *
    // coefy.
    if !scip.set.is_zero(coefx) && !scip.set.is_eq(coefx, -1.0) {
        return Ok(());
    }
    if !scip.set.is_zero(coefy) && !scip.set.is_eq(coefy, -1.0) {
        return Ok(());
    }
    if !scip.set.is_eq(rhs, -coefx * coefy) {
        return Ok(());
    }

    let mut vars = [NONE, NONE];
    if scip.set.is_zero(coefy) {
        vars[0] = crate::expr_var::get_var_expr_var(scip, expr1);
    } else {
        // x - 1 = -(1-x) = -(~x)
        let v = crate::expr_var::get_var_expr_var(scip, expr1);
        vars[0] = crate::scip_var::get_negated_var(scip, v)?;
    }
    if scip.set.is_zero(coefx) {
        vars[1] = crate::expr_var::get_var_expr_var(scip, expr2);
    } else {
        // y - 1 = -(1 - y) = -(~y)
        let v = crate::expr_var::get_var_expr_var(scip, expr2);
        vars[1] = crate::scip_var::get_negated_var(scip, v)?;
    }

    // Constraint is now of the form vars[0] * vars[1] == 0.

    // vars[0] + vars[1] <= 1
    let flags = crate::cons::cons_flags(scip, cons);
    let name = scip.conss[cons].name.clone();
    upgdconss.push(create_cons_setpack(scip, &name, &vars, &flags)?);

    *nupgdconss += 1;

    Ok(())
}

/// `SCIPincludeConshdlrSetppc`: creates the handler for set partitioning /
/// packing / covering constraints and includes it in SCIP. Sub-SCIP copying,
/// parsing, printing, the symmetry graphs and the nonlinear upgrade are added
/// with those modules.
pub fn include_conshdlr_setppc(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // Create event handler for bound change events.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_setppc, None)?;

    // Create conflict handler for setppc constraints.
    api::include_conflicthdlr_basic(scip, CONFLICTHDLR_NAME, CONFLICTHDLR_DESC, CONFLICTHDLR_PRIORITY, conflict_exec_setppc, None)?;

    // Create constraint handler data.
    let conshdlrdata = conshdlrdata_create(scip, eventhdlr);

    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_setppc),
        Some(cons_enfops_setppc),
        Some(cons_check_setppc),
        Some(cons_lock_setppc),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_setppc);
    h.conscopy = Some(cons_copy_setppc);
    h.consactive = Some(cons_active_setppc);
    h.consdeactive = Some(cons_deactive_setppc);
    h.consdelete = Some(cons_delete_setppc);
    h.consdelvars = Some(cons_delvars_setppc);
    h.consexitpre = Some(cons_exitpre_setppc);
    h.consinitsol = Some(cons_initsol_setppc);
    h.consexitsol = Some(cons_exitsol_setppc);
    h.consfree = Some(cons_free_setppc);
    h.consgetvars = Some(cons_get_vars_setppc);
    h.consgetnvars = Some(cons_get_n_vars_setppc);
    h.consinit = Some(cons_init_setppc);
    h.consinitlp = Some(cons_initlp_setppc);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_setppc), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_setppc), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_setppc);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_setppc),
        Some(cons_sepasol_setppc),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_setppc);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_setppc);

    let conshdlrlinear = crate::set::set_find_conshdlr(scip, "linear");
    hdm(scip, conshdlr).conshdlrlinear = conshdlrlinear;
    if conshdlrlinear != NONE {
        // Include the linear constraint to setppc constraint upgrade in the
        // linear constraint handler.
        crate::cons_linear::include_lincons_upgrade(scip, lincons_upgd_setppc, LINCONSUPGD_PRIORITY, CONSHDLR_NAME)?;
    }
    if crate::set::set_find_conshdlr(scip, "nonlinear") != NONE {
        // Notify function that upgrades quadratic constraint to setpacking.
        crate::cons_nonlinear::include_cons_upgrade_nonlinear(scip, nonlin_upgd_setppc, NONLINCONSUPGD_PRIORITY, true, CONSHDLR_NAME)?;
    }

    // Set partitioning constraint handler parameters.
    paramset_add_int(
        scip,
        "constraints/setppc/npseudobranches",
        "number of children created in pseudo branching (0: disable pseudo branching)",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, npseudobranches),
        true,
        DEFAULT_NPSEUDOBRANCHES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/setppc/presolpairwise",
        "should pairwise constraint comparison be performed in presolving?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolpairwise),
        true,
        DEFAULT_PRESOLPAIRWISE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/setppc/presolusehashing",
        "should hash table be used for detecting redundant constraints in advance",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolusehashing),
        true,
        DEFAULT_PRESOLUSEHASHING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/setppc/dualpresolving",
        "should dual presolving steps be performed?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, dualpresolving),
        true,
        DEFAULT_DUALPRESOLVING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/setppc/cliquelifting",
        " should we try to lift variables into other clique constraints, fix variables, aggregate them, and also shrink the amount of variables in clique constraints",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, cliquelifting),
        true,
        DEFAULT_CLIQUELIFTING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/setppc/addvariablesascliques",
        "should we try to generate extra cliques out of all binary variables to maybe fasten redundant constraint detection",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, addvariablesascliques),
        true,
        DEFAULT_ADDVARIABLESASCLIQUES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/setppc/cliqueshrinking",
        "should we try to shrink the number of variables in a clique constraints, by replacing more than one variable by only one",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, cliqueshrinking),
        true,
        DEFAULT_CLIQUESHRINKING,
        None,
    )?;

    Ok(())
}

/// The flags of `SCIPcreateConsBasic*`.
const BASIC_FLAGS: ConsFlags = ConsFlags {
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

/// `SCIPcreateConsSetpart`: creates and captures a set partitioning
/// constraint.
pub fn create_cons_setpart(scip: &mut Scip, name: &str, vars: &[VarId], flags: &ConsFlags) -> ScipResult<ConsId> {
    create_cons_setppc(scip, name, vars, SetppcType::Partitioning, flags)
}

/// `SCIPcreateConsBasicSetpart`.
pub fn create_cons_basic_setpart(scip: &mut Scip, name: &str, vars: &[VarId]) -> ScipResult<ConsId> {
    create_cons_setpart(scip, name, vars, &BASIC_FLAGS)
}

/// `SCIPcreateConsSetpack`: creates and captures a set packing constraint.
pub fn create_cons_setpack(scip: &mut Scip, name: &str, vars: &[VarId], flags: &ConsFlags) -> ScipResult<ConsId> {
    create_cons_setppc(scip, name, vars, SetppcType::Packing, flags)
}

/// `SCIPcreateConsBasicSetpack`.
pub fn create_cons_basic_setpack(scip: &mut Scip, name: &str, vars: &[VarId]) -> ScipResult<ConsId> {
    create_cons_setpack(scip, name, vars, &BASIC_FLAGS)
}

/// `SCIPcreateConsSetcover`: creates and captures a set covering
/// constraint.
pub fn create_cons_setcover(scip: &mut Scip, name: &str, vars: &[VarId], flags: &ConsFlags) -> ScipResult<ConsId> {
    create_cons_setppc(scip, name, vars, SetppcType::Covering, flags)
}

/// `SCIPcreateConsBasicSetcover`.
pub fn create_cons_basic_setcover(scip: &mut Scip, name: &str, vars: &[VarId]) -> ScipResult<ConsId> {
    create_cons_setcover(scip, name, vars, &BASIC_FLAGS)
}

/// Whether the constraint belongs to the setppc constraint handler.
fn is_setppc(scip: &Scip, cons: ConsId) -> bool {
    scip.conshdlrs[scip.conss[cons].conshdlr].name == CONSHDLR_NAME
}

/// `SCIPaddCoefSetppc`: adds coefficient in set partitioning / packing /
/// covering constraint.
pub fn add_coef_setppc(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    if !is_setppc(scip, cons) {
        // Constraint is not a set partitioning / packing / covering constraint.
        return Err(Retcode::InvalidData);
    }

    add_coef(scip, cons, var)
}

/// `SCIPgetNVarsSetppc`: gets number of variables in set partitioning /
/// packing / covering constraint.
pub fn get_n_vars_setppc(scip: &Scip, cons: ConsId) -> i32 {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    cd(scip, cons).nvars
}

/// `SCIPgetVarsSetppc`: gets array of variables in set partitioning /
/// packing / covering constraint.
pub fn get_vars_setppc(scip: &Scip, cons: ConsId) -> &[VarId] {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    let c = cd(scip, cons);
    &c.vars.0[..c.nvars as usize]
}

/// `SCIPgetTypeSetppc`: gets type of set partitioning / packing / covering
/// constraint.
pub fn get_type_setppc(scip: &Scip, cons: ConsId) -> SetppcType {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    cd(scip, cons).setppctype
}

/// `SCIPgetDualsolSetppc`: gets the dual solution of the set partitioning /
/// packing / covering constraint in the current LP.
pub fn get_dualsol_setppc(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualsol(scip, row) } else { 0.0 }
}

/// `SCIPgetDualfarkasSetppc`: gets the dual Farkas value of the set
/// partitioning / packing / covering constraint in the current infeasible LP.
pub fn get_dualfarkas_setppc(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualfarkas(scip, row) } else { 0.0 }
}

/// `SCIPgetRowSetppc`: returns the linear relaxation of the given set
/// partitioning / packing / covering constraint; may return `NONE` if no LP
/// row was yet created.
pub fn get_row_setppc(scip: &Scip, cons: ConsId) -> usize {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    cd(scip, cons).row
}

/// `SCIPcreateRowSetppc`: creates and returns the row of the given set
/// partitioning / packing / covering constraint.
pub fn create_row_setppc(scip: &mut Scip, cons: ConsId) -> ScipResult {
    if !is_setppc(scip, cons) {
        return Err(Retcode::Error);
    }
    create_row(scip, cons)
}

/// `SCIPgetNFixedonesSetppc`: returns current number of variables fixed to
/// one in the constraint.
pub fn get_n_fixedones_setppc(scip: &Scip, cons: ConsId) -> i32 {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    cd(scip, cons).nfixedones
}

/// `SCIPgetNFixedzerosSetppc`: returns current number of variables fixed to
/// zero in the constraint.
pub fn get_n_fixedzeros_setppc(scip: &Scip, cons: ConsId) -> i32 {
    assert!(is_setppc(scip, cons), "constraint is not a set partitioning / packing / covering constraint");
    cd(scip, cons).nfixedzeros
}

/// `SCIPcleanupConssSetppc`: cleans up (multi-)aggregations and fixings from
/// setppc constraints; returns whether problem was detected to be
/// infeasible.
pub fn cleanup_conss_setppc(scip: &mut Scip, onlychecked: bool, naddconss: &mut i32, ndelconss: &mut i32, nchgcoefs: &mut i32, nfixedvars: &mut i32) -> ScipResult<bool> {
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        return Ok(false);
    }

    let mut infeasible = false;

    let h = &scip.conshdlrs[conshdlr];
    let conss: Vec<ConsId> = if onlychecked {
        h.checkconss.0[..h.ncheckconss as usize].to_vec()
    } else {
        h.conss.0[..h.nactiveconss as usize].to_vec()
    };

    // Loop backwards since then deleted constraints do not interfere with the
    // loop.
    for &cons in conss.iter().rev() {
        infeasible = apply_fixings(scip, cons, Some((&mut *naddconss, &mut *ndelconss, &mut *nfixedvars)))?;
        if infeasible {
            break;
        }

        if scip.conss[cons].deleted {
            continue;
        }

        // Merging unmerged constraints.
        infeasible = merge_multiples(scip, cons, nfixedvars, ndelconss, nchgcoefs)?;
        if infeasible {
            break;
        }
    }

    Ok(infeasible)
}

/// `conshdlrCopySetppc`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_setppc(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_setppc(scip)?;
    *valid = true;
    Ok(())
}

/// `consCopySetppc`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_setppc(
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
    let sourcevars = get_vars_setppc(sourcescip, sourcecons).to_vec();

    // Get setppc type.
    let (lhs, rhs) = match get_type_setppc(sourcescip, sourcecons) {
        SetppcType::Partitioning => (1.0, 1.0),
        SetppcType::Packing => (-scip.set.infinity(), 1.0),
        SetppcType::Covering => (1.0, scip.set.infinity()),
    };
    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);

    // Copy the logic using the linear constraint copy method.
    (*cons, *valid) = crate::cons_linear::copy_cons_linear(scip, sourcescip, &consname, &sourcevars, None, lhs, rhs, maps, flags, global)?;
    Ok(())
}
