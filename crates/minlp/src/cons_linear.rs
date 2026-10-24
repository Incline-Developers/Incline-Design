//! SCIP's linear constraint handler (`cons_linear.c`): lhs <= a^T x <= rhs,
//! with activity-based propagation, presolving (aggregation, coefficient
//! tightening, dual reductions, pairwise comparison), separation of
//! knapsack cardinality cuts, upgrading to specialized constraint types,
//! and a conflict handler creating linear constraints.
//!
//! C keeps pointers between constraint data, event data and constraints;
//! here the constraint data is the constraint's `consdata`, read through
//! [`cd`]/[`cdm`] afresh after every call that may trigger events (which
//! update activities of this very constraint), and the event data lives in
//! the event handler's arena.

use std::any::Any;

use crate::{
    api,
    api::get_sol_val,
    arena::Arena,
    cons::*,
    cvec::CVec,
    def::INVALID,
    event::*,
    misc::hash_signature64,
    quad::{Quad, sum_qd},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var, sort,
    tree::tree_has_current_node_lp as has_current_node_lp,
    types::{BoundType, ConfType, ConsId, ConshdlrId, EventhdlrId, ImplintType, LockType, NLOCKTYPES, ScipResultCode, SolId, VarId, VarStatus, VarType},
    var::{BdChgIdx, NONE, var_capture, var_compare, var_is_binary, var_is_implied_integral, var_is_integral},
};

pub const CONSHDLR_NAME: &str = "linear";
const CONSHDLR_DESC: &str = "linear constraints of the form  lhs <= a^T x <= rhs";
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 100000;
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -1000000;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -1000000;
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

/// Presolving timing of the constraint handler (fast, medium, or
/// exhaustive).
const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST | PRESOLTIMING_EXHAUSTIVE;
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

const EVENTHDLR_NAME: &str = "linear";
const EVENTHDLR_DESC: &str = "bound change event handler for linear constraints";

const CONFLICTHDLR_NAME: &str = "linear";
const CONFLICTHDLR_DESC: &str = "conflict handler creating linear constraints";
const CONFLICTHDLR_PRIORITY: i32 = -1000000;

/// Multiplier on propagation frequency, how often the bounds are tightened.
const DEFAULT_TIGHTENBOUNDSFREQ: i32 = 1;
/// Maximal number of separation rounds per node (-1: unlimited).
const DEFAULT_MAXROUNDS: i32 = 5;
/// Maximal number of separation rounds in the root node (-1: unlimited).
const DEFAULT_MAXROUNDSROOT: i32 = -1;
/// Maximal number of cuts separated per separation round.
const DEFAULT_MAXSEPACUTS: i32 = 50;
/// Maximal number of cuts separated per separation round in root node.
const DEFAULT_MAXSEPACUTSROOT: i32 = 200;
/// Should pairwise constraint comparison be performed in presolving?
const DEFAULT_PRESOLPAIRWISE: bool = true;
/// Should hash table be used for detecting redundant constraints in
/// advance.
const DEFAULT_PRESOLUSEHASHING: bool = true;
/// Number for minimal pairwise presolving comparisons.
const DEFAULT_NMINCOMPARISONS: i32 = 200000;
/// Minimal gain per minimal pairwise presolving comparisons to repeat
/// pairwise comparison round.
const DEFAULT_MINGAINPERNMINCOMP: f64 = 1e-06;
/// Should variables be sorted after presolve w.r.t their coefficient
/// absolute for faster propagation?
const DEFAULT_SORTVARS: bool = true;
/// Should the violation for a constraint with side 0.0 be checked relative
/// to 1.0 (FALSE) or to the maximum absolute value in the activity (TRUE)?
const DEFAULT_CHECKRELMAXABS: bool = false;
/// Maximal allowed relative gain in maximum norm for constraint aggregation
/// (0.0: disable constraint aggregation).
const DEFAULT_MAXAGGRNORMSCALE: f64 = 0.0;
/// Maximum activity delta to run easy propagation on linear constraint.
const DEFAULT_MAXEASYACTIVITYDELTA: f64 = 1e6;
/// Maximal relative distance from current node's dual bound to primal bound
/// compared to best node's dual bound for separating knapsack cardinality
/// cuts.
const DEFAULT_MAXCARDBOUNDDIST: f64 = 0.0;
/// Should all constraints be subject to cardinality cut generation instead
/// of only the ones with non-zero dual value?
const DEFAULT_SEPARATEALL: bool = false;
/// Should presolving search for redundant variables in equations.
const DEFAULT_AGGREGATEVARIABLES: bool = true;
/// Should presolving try to simplify inequalities.
const DEFAULT_SIMPLIFYINEQUALITIES: bool = true;
/// Should dual presolving steps be performed?
const DEFAULT_DUALPRESOLVING: bool = true;
/// Should stuffing of singleton continuous variables be performed?
const DEFAULT_SINGLETONSTUFFING: bool = true;
/// Should single variable stuffing be performed, which tries to fulfill
/// constraints using the cheapest variable?
const DEFAULT_SINGLEVARSTUFFING: bool = false;
/// Should presolving try to detect constraints parallel to the objective
/// function defining an upper bound and prevent these constraints from
/// entering the LP.
const DEFAULT_DETECTCUTOFFBOUND: bool = true;
/// Should presolving try to detect constraints parallel to the objective
/// function defining a lower bound and prevent these constraints from
/// entering the LP.
const DEFAULT_DETECTLOWERBOUND: bool = true;
/// Should presolving try to detect subsets of constraints parallel to the
/// objective function.
const DEFAULT_DETECTPARTIALOBJECTIVE: bool = true;
/// Should we perform ranged row propagation.
const DEFAULT_RANGEDROWPROPAGATION: bool = true;
/// Should presolving and propagation extract sub-constraints from ranged
/// rows and equations?
const DEFAULT_RANGEDROWARTCONS: bool = true;
/// Maximum depth to apply ranged row propagation.
const DEFAULT_RANGEDROWMAXDEPTH: i32 = i32::MAX;
/// Frequency for applying ranged row propagation.
const DEFAULT_RANGEDROWFREQ: i32 = 1;
/// Should multi-aggregations only be performed if the constraint can be
/// removed afterwards?
const DEFAULT_MULTAGGRREMOVE: bool = false;
/// Maximum coefficient dynamism (ie. maxabsval / minabsval) for
/// multiaggregation.
const DEFAULT_MAXMULTAGGRQUOT: f64 = 1e+03;
/// Maximum coefficient dynamism (ie. maxabsval / minabsval) for
/// multiaggregation.
const DEFAULT_MAXDUALMULTAGGRQUOT: f64 = 1e+20;
/// Should cliques be extracted?
const DEFAULT_EXTRACTCLIQUES: bool = true;

/// Maximal denominator for simple rational fixed values.
const MAXDNOM: i64 = 10000;
/// Maximal coefficient value after scaling.
const MAXSCALEDCOEF: f64 = 0.0;
/// Maximal coefficient value after scaling if all variables are of integral
/// type.
const MAXSCALEDCOEFINTEGER: f64 = 0.0;
/// Maximal absolute value of full and partial activities such that
/// redundancy-based simplifications are allowed to be applied.
const MAXACTVAL: f64 = 1e+09;
/// Maximal abolsute value we trust without recomputing the activity.
const MAXVALRECOMP: f64 = 1e+06;
/// Minimal abolsute value we trust without recomputing the activity.
const MINVALRECOMP: f64 = 1e-05;
/// Priority of the constraint handler for upgrading of expressions
/// constraints.
const NONLINCONSUPGD_PRIORITY: i32 = 1000000;

/// `SCIP_ConsData`: constraint data for linear constraints.
#[derive(Clone, Debug, Default)]
pub struct ConsData {
    /// Left hand side of row (for ranged rows).
    pub lhs: f64,
    /// Right hand side of row.
    pub rhs: f64,
    /// Maximum absolute value of all coefficients.
    pub maxabsval: f64,
    /// Minimal absolute value of all coefficients.
    pub minabsval: f64,
    /// Minimal value w.r.t. the variable's local bounds for the constraint's
    /// activity, ignoring the coefficients contributing with infinite value.
    pub minactivity: Quad,
    /// Maximal value w.r.t. the variable's local bounds for the constraint's
    /// activity, ignoring the coefficients contributing with infinite value.
    pub maxactivity: Quad,
    /// Last minimal activity which was computed by complete summation over
    /// all contributing values.
    pub lastminactivity: f64,
    /// Last maximal activity which was computed by complete summation over
    /// all contributing values.
    pub lastmaxactivity: f64,
    /// Minimal value w.r.t. the variable's global bounds for the constraint's
    /// activity, ignoring the coefficients contributing with infinite value.
    pub glbminactivity: Quad,
    /// Maximal value w.r.t. the variable's global bounds for the constraint's
    /// activity, ignoring the coefficients contributing with infinite value.
    pub glbmaxactivity: Quad,
    /// Last global minimal activity which was computed by complete summation
    /// over all contributing values.
    pub lastglbminactivity: f64,
    /// Last global maximal activity which was computed by complete summation
    /// over all contributing values.
    pub lastglbmaxactivity: f64,
    /// Maximal activity contribution of a single variable, or SCIP_INVALID if
    /// invalid.
    pub maxactdelta: f64,
    /// Variable with maximal activity contribution, or `NONE` if invalid.
    pub maxactdeltavar: VarId,
    /// Bit signature of coefficients that may take a positive value.
    pub possignature: u64,
    /// Bit signature of coefficients that may take a negative value.
    pub negsignature: u64,
    /// LP row, if constraint is already stored in LP row format.
    pub row: usize,
    /// Variables of constraint entries.
    pub vars: CVec<VarId>,
    /// Coefficients of constraint entries.
    pub vals: CVec<f64>,
    /// Event data for bound change events of the variables (ids into the
    /// event handler's arena), or `None`.
    pub eventdata: Option<CVec<usize>>,
    pub minactivityneginf: i32,
    pub minactivityposinf: i32,
    pub maxactivityneginf: i32,
    pub maxactivityposinf: i32,
    pub minactivityneghuge: i32,
    pub minactivityposhuge: i32,
    pub maxactivityneghuge: i32,
    pub maxactivityposhuge: i32,
    pub glbminactivityneginf: i32,
    pub glbminactivityposinf: i32,
    pub glbmaxactivityneginf: i32,
    pub glbmaxactivityposinf: i32,
    pub glbminactivityneghuge: i32,
    pub glbminactivityposhuge: i32,
    pub glbmaxactivityneghuge: i32,
    pub glbmaxactivityposhuge: i32,
    /// Size of the vars- and vals-arrays.
    pub varssize: i32,
    /// Number of nonzeros in constraint.
    pub nvars: i32,
    /// The number of binary variables in the constraint, only valid after
    /// sorting in stage >= SCIP_STAGE_INITSOLVE.
    pub nbinvars: i32,
    /// Is constraint already propagated with bound tightening? (2 bits)
    pub boundstightened: u32,
    /// Did we perform ranged row propagation on this constraint? (0: no, 1:
    /// yes, 2: with potentially adding artificial constraint).
    pub rangedrowpropagated: u32,
    pub validmaxabsval: bool,
    pub validminabsval: bool,
    pub validactivities: bool,
    pub validminact: bool,
    pub validmaxact: bool,
    pub validglbminact: bool,
    pub validglbmaxact: bool,
    pub presolved: bool,
    pub removedfixings: bool,
    pub validsignature: bool,
    pub changed: bool,
    pub normalized: bool,
    pub upgradetried: bool,
    pub upgraded: bool,
    pub indexsorted: bool,
    pub merged: bool,
    pub cliquesadded: bool,
    pub implsadded: bool,
    pub coefsorted: bool,
    pub varsdeleted: bool,
    pub hascontvar: bool,
    pub hasnonbinvar: bool,
    pub hasnonbinvalid: bool,
    pub checkabsolute: bool,
}

/// `SCIP_EventData`: event data for bound change event.
#[derive(Clone, Copy, Debug)]
pub struct EventData {
    /// Linear constraint to process the bound change for.
    pub cons: ConsId,
    /// Position of variable in vars array.
    pub varpos: i32,
    /// Position of event in variable's event filter.
    pub filterpos: i32,
}

/// The event handler's data: the event data of all caught bound change
/// events.
#[derive(Default)]
pub struct EventhdlrData {
    pub eventdatas: Arena<EventData>,
}

/// `SCIP_DECL_LINCONSUPGD`: tries to upgrade a linear constraint into a
/// more specific constraint type; `(cons, nvars, vars, vals, lhs, rhs,
/// nposbin, nnegbin, nposint, nnegint, nposimpl, nnegimpl, nposimplbin,
/// nnegimplbin, nposcont, nnegcont, ncoeffspone, ncoeffsnone, ncoeffspint,
/// ncoeffsnint, ncoeffspfrac, ncoeffsnfrac, poscoeffsum, negcoeffsum,
/// integral, upgdcons)`.
pub type LinConsUpgd = fn(&mut Scip, ConsId, &LinConsStats, &mut ConsId) -> ScipResult;

/// The arguments `SCIP_DECL_LINCONSUPGD` receives besides the constraint.
pub struct LinConsStats {
    pub vars: Vec<VarId>,
    pub vals: Vec<f64>,
    pub lhs: f64,
    pub rhs: f64,
    pub nposbin: i32,
    pub nnegbin: i32,
    pub nposint: i32,
    pub nnegint: i32,
    pub nposimpl: i32,
    pub nnegimpl: i32,
    pub nposimplbin: i32,
    pub nnegimplbin: i32,
    pub nposcont: i32,
    pub nnegcont: i32,
    pub ncoeffspone: i32,
    pub ncoeffsnone: i32,
    pub ncoeffspint: i32,
    pub ncoeffsnint: i32,
    pub ncoeffspfrac: i32,
    pub ncoeffsnfrac: i32,
    pub poscoeffsum: f64,
    pub negcoeffsum: f64,
    pub integral: bool,
}

/// `SCIP_LinConsUpgrade`: linear constraint update method.
#[derive(Clone, Copy)]
pub struct LinConsUpgrade {
    /// Method to call for upgrading linear constraint.
    pub linconsupgd: LinConsUpgd,
    /// Priority of upgrading method.
    pub priority: i32,
    /// Is upgrading enabled.
    pub active: bool,
}

/// `SCIP_ConshdlrData`: constraint handler data.
#[derive(Default)]
pub struct ConshdlrData {
    /// Event handler for bound change events.
    pub eventhdlr: EventhdlrId,
    /// Linear constraint upgrade methods for specializing linear constraints.
    pub linconsupgrades: Vec<LinConsUpgrade>,
    /// Maximal allowed relative gain in maximum norm for constraint
    /// aggregation (0.0: disable constraint aggregation).
    pub maxaggrnormscale: f64,
    /// Maximal relative distance from current node's dual bound to primal
    /// bound compared to best node's dual bound for separating knapsack
    /// cardinality cuts.
    pub maxcardbounddist: f64,
    /// Minimal gain per minimal pairwise presolving comparisons to repeat
    /// pairwise comparison round.
    pub mingainpernmincomp: f64,
    /// Maximum activity delta to run easy propagation on linear constraint
    /// (faster, but numerically less stable).
    pub maxeasyactivitydelta: f64,
    /// Size of linconsupgrade array.
    pub linconsupgradessize: i32,
    /// Number of linear constraint upgrade methods.
    pub nlinconsupgrades: i32,
    /// Multiplier on propagation frequency, how often the bounds are
    /// tightened.
    pub tightenboundsfreq: i32,
    /// Maximal number of separation rounds per node (-1: unlimited).
    pub maxrounds: i32,
    /// Maximal number of separation rounds in the root node (-1: unlimited).
    pub maxroundsroot: i32,
    /// Maximal number of cuts separated per separation round.
    pub maxsepacuts: i32,
    /// Maximal number of cuts separated per separation round in root node.
    pub maxsepacutsroot: i32,
    /// Number for minimal pairwise presolving comparisons.
    pub nmincomparisons: i32,
    /// Number of added constraints.
    pub naddconss: i32,
    /// Should pairwise constraint comparison be performed in presolving?
    pub presolpairwise: bool,
    /// Should hash table be used for detecting redundant constraints in
    /// advance.
    pub presolusehashing: bool,
    /// Should all constraints be subject to cardinality cut generation
    /// instead of only the ones with non-zero dual value?
    pub separateall: bool,
    /// Should presolving search for redundant variables in equations.
    pub aggregatevariables: bool,
    /// Should presolving try to cancel down or delete coefficients in
    /// inequalities.
    pub simplifyinequalities: bool,
    /// Should dual presolving steps be performed?
    pub dualpresolving: bool,
    /// Should stuffing of singleton continuous variables be performed?
    pub singletonstuffing: bool,
    /// Should single variable stuffing be performed, which tries to fulfill
    /// constraints using the cheapest variable?
    pub singlevarstuffing: bool,
    /// Should binary variables be sorted for faster propagation?
    pub sortvars: bool,
    /// Should the violation for a constraint with side 0.0 be checked
    /// relative to 1.0 (FALSE) or to the maximum absolute value in the
    /// activity (TRUE)?
    pub checkrelmaxabs: bool,
    /// Should presolving try to detect constraints parallel to the objective
    /// function defining an upper bound and prevent these constraints from
    /// entering the LP.
    pub detectcutoffbound: bool,
    /// Should presolving try to detect constraints parallel to the objective
    /// function defining a lower bound and prevent these constraints from
    /// entering the LP.
    pub detectlowerbound: bool,
    /// Should presolving try to detect subsets of constraints parallel to
    /// the objective function.
    pub detectpartialobjective: bool,
    /// Should presolving and propagation try to improve bounds, detect
    /// infeasibility, and extract sub-constraints from ranged rows and
    /// equations.
    pub rangedrowpropagation: bool,
    /// Should presolving and propagation extract sub-constraints from ranged
    /// rows and equations?
    pub rangedrowartcons: bool,
    /// Maximum depth to apply ranged row propagation.
    pub rangedrowmaxdepth: i32,
    /// Frequency for applying ranged row propagation.
    pub rangedrowfreq: i32,
    /// Should multi-aggregations only be performed if the constraint can be
    /// removed afterwards?
    pub multaggrremove: bool,
    /// Maximum coefficient dynamism (ie. maxabsval / minabsval) for primal
    /// multiaggregation.
    pub maxmultaggrquot: f64,
    /// Maximum coefficient dynamism (ie. maxabsval / minabsval) for dual
    /// multiaggregation.
    pub maxdualmultaggrquot: f64,
    /// Should cliques be extracted?
    pub extractcliques: bool,
}

/// The constraint data of a linear constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a linear constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the linear constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/// The constraint handler data of the linear constraint handler, mutably.
#[inline]
fn hdm(scip: &mut Scip, h: ConshdlrId) -> &mut ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_mut().unwrap().downcast_mut::<ConshdlrData>().unwrap()
}

/// The event data arena of the linear event handler.
#[inline]
fn eds(scip: &mut Scip, eventhdlr: EventhdlrId) -> &mut Arena<EventData> {
    &mut scip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_mut()
        .unwrap()
        .downcast_mut::<EventhdlrData>()
        .unwrap()
        .eventdatas
}

/// The event data `ed` of the linear event handler.
#[inline]
fn ed(scip: &Scip, eventhdlr: EventhdlrId, id: usize) -> EventData {
    scip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_ref()
        .unwrap()
        .downcast_ref::<EventhdlrData>()
        .unwrap()
        .eventdatas[id]
}

/*
 * Propagation rules
 */

/// `PROPRULE`: activity residuals of all other variables tighten bounds of
/// single variable due to the right hand side of the inequality.
const PROPRULE_1_RHS: i32 = 1;
/// Activity residuals of all other variables tighten bounds of single
/// variable due to the left hand side of the inequality.
const PROPRULE_1_LHS: i32 = 2;
/// Fixed variables and gcd of all left variables tighten bounds of a single
/// variable in this reanged row.
const PROPRULE_1_RANGEDROW: i32 = 3;

/// `InferInfo` (`asbits.proprule:8`, `asbits.pos:24` in an int, low bits
/// first): returns the propagation rule stored in the inference
/// information.
#[inline]
fn infer_info_get_proprule(inferinfo: i32) -> i32 {
    (inferinfo as u32 & 0xff) as i32
}

/// `inferInfoGetPos`: returns the position stored in the inference
/// information.
#[inline]
fn infer_info_get_pos(inferinfo: i32) -> i32 {
    ((inferinfo as u32 >> 8) & 0x00ff_ffff) as i32
}

/// `getInferInt`: constructs an inference information out of a propagation
/// rule and a position number, returns info as int; in the inferinfo struct
/// only 24 bits for 'pos' are reserved.
#[inline]
fn get_infer_int(proprule: i32, pos: i32) -> i32 {
    ((proprule as u32 & 0xff) | ((pos as u32 & 0x00ff_ffff) << 8)) as i32
}

/*
 * Memory growing methods for dynamically allocated arrays
 */

/// `consdataEnsureVarsSize`: ensures, that vars and vals arrays can store
/// at least num entries.
fn consdata_ensure_vars_size(scip: &mut Scip, cons: ConsId, num: i32) {
    if num > cd(scip, cons).varssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = cdm(scip, cons);
        c.vars.0.resize(newsize as usize, NONE);
        c.vals.0.resize(newsize as usize, 0.0);
        if let Some(e) = c.eventdata.as_mut() {
            e.0.resize(newsize as usize, NONE);
        }
        c.varssize = newsize;
    }
}

/*
 * Local methods for managing linear constraint update methods
 */

/// `linconsupgradeCreate`: creates a linear constraint upgrade data object.
fn linconsupgrade_create(linconsupgd: LinConsUpgd, priority: i32) -> LinConsUpgrade {
    LinConsUpgrade {
        linconsupgd,
        priority,
        active: true,
    }
}

/// `conshdlrdataCreate`: creates constraint handler data for linear
/// constraint handler.
fn conshdlrdata_create(eventhdlr: EventhdlrId) -> ConshdlrData {
    // Set event handler for updating linear constraint activity bounds.
    ConshdlrData {
        eventhdlr,
        ..ConshdlrData::default()
    }
}

/// `conshdlrdataHasUpgrade`: checks whether the upgrade method is already
/// known.
fn conshdlrdata_has_upgrade(data: &ConshdlrData, linconsupgd: LinConsUpgd) -> bool {
    let mut i = data.nlinconsupgrades - 1;
    while i >= 0 {
        if std::ptr::fn_addr_eq(data.linconsupgrades[i as usize].linconsupgd, linconsupgd) {
            return true;
        }
        i -= 1;
    }
    false
}

/// `conshdlrdataIncludeUpgrade`: adds a linear constraint update method to
/// the constraint handler's data.
fn conshdlrdata_include_upgrade(scip: &Scip, data: &mut ConshdlrData, linconsupgrade: LinConsUpgrade) {
    // conshdlrdataEnsureLinconsupgradesSize.
    let num = data.nlinconsupgrades + 1;
    if num > data.linconsupgradessize {
        let newsize = scip.set.calc_mem_grow_size(num);
        data.linconsupgrades.resize(newsize as usize, linconsupgrade);
        data.linconsupgradessize = newsize;
    }

    let mut i = data.nlinconsupgrades as usize;
    while i > 0 && data.linconsupgrades[i - 1].priority < linconsupgrade.priority {
        data.linconsupgrades[i] = data.linconsupgrades[i - 1];
        i -= 1;
    }
    data.linconsupgrades[i] = linconsupgrade;
    data.nlinconsupgrades += 1;
}

/*
 * Local methods
 */

/// `lockRounding`: installs rounding locks for the given variable
/// associated to the given coefficient in the linear constraint.
fn lock_rounding(scip: &mut Scip, cons: ConsId, var: VarId, val: f64) -> ScipResult {
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    if val < 0.0 {
        let (down, up) = (!scip.set.is_infinity(rhs), !scip.set.is_infinity(-lhs));
        api::lock_var_cons(scip, var, cons, down, up)
    } else {
        let (down, up) = (!scip.set.is_infinity(-lhs), !scip.set.is_infinity(rhs));
        api::lock_var_cons(scip, var, cons, down, up)
    }
}

/// `unlockRounding`: removes rounding locks for the given variable
/// associated to the given coefficient in the linear constraint.
fn unlock_rounding(scip: &mut Scip, cons: ConsId, var: VarId, val: f64) -> ScipResult {
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    if val < 0.0 {
        let (down, up) = (!scip.set.is_infinity(rhs), !scip.set.is_infinity(-lhs));
        api::unlock_var_cons(scip, var, cons, down, up)
    } else {
        let (down, up) = (!scip.set.is_infinity(-lhs), !scip.set.is_infinity(rhs));
        api::unlock_var_cons(scip, var, cons, down, up)
    }
}

/// The events the linear constraints catch on their variables.
const CONS_EVENTS: EventType =
    EVENTTYPE_BOUNDCHANGED | EVENTTYPE_VARFIXED | EVENTTYPE_VARUNLOCKED | EVENTTYPE_GBDCHANGED | EVENTTYPE_VARDELETED | EVENTTYPE_TYPECHANGED | EVENTTYPE_IMPLTYPECHANGED;

/// `consCatchEvent`: creates event data for variable at given position, and
/// catches events.
fn cons_catch_event(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    let id = eds(scip, eventhdlr).push(EventData { cons, varpos: pos, filterpos: -1 });
    cdm(scip, cons).eventdata.as_mut().unwrap()[pos] = id;

    let var = cd(scip, cons).vars[pos];
    let mut filterpos = -1;
    api::catch_var_event(scip, var, CONS_EVENTS, eventhdlr, id, Some(&mut filterpos))?;
    eds(scip, eventhdlr)[id].filterpos = filterpos;

    let active = crate::var::var_is_active(scip, var);
    let c = cdm(scip, cons);
    c.removedfixings = c.removedfixings && active;
    Ok(())
}

/// `consDropEvent`: deletes event data for variable at given position, and
/// drops events.
fn cons_drop_event(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    let id = cd(scip, cons).eventdata.as_ref().unwrap()[pos];
    let var = cd(scip, cons).vars[pos];
    let filterpos = ed(scip, eventhdlr, id).filterpos;

    api::drop_var_event(scip, var, CONS_EVENTS, eventhdlr, id, filterpos)?;

    eds(scip, eventhdlr).remove(id);
    Ok(())
}

/// `consCatchAllEvents`: catches bound change events for all variables in
/// transformed linear constraint.
fn cons_catch_all_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    // Allocate eventdata array.
    let c = cdm(scip, cons);
    c.eventdata = Some(CVec(vec![NONE; c.varssize as usize]));

    // Catch event for every single variable.
    for i in 0..cd(scip, cons).nvars {
        cons_catch_event(scip, cons, eventhdlr, i)?;
    }
    Ok(())
}

/// `consDropAllEvents`: drops bound change events for all variables in
/// transformed linear constraint.
fn cons_drop_all_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    // Drop event of every single variable.
    let mut i = cd(scip, cons).nvars - 1;
    while i >= 0 {
        cons_drop_event(scip, cons, eventhdlr, i)?;
        i -= 1;
    }

    // Free eventdata array.
    cdm(scip, cons).eventdata = None;
    Ok(())
}

/// `consdataCreate`: creates a linear constraint data.
fn consdata_create(scip: &mut Scip, vars: &[VarId], vals: &[f64], mut lhs: f64, mut rhs: f64) -> ScipResult<ConsData> {
    let set = &scip.set;
    if set.is_infinity(rhs) {
        rhs = set.infinity();
    } else if set.is_infinity(-rhs) {
        rhs = -set.infinity();
    }

    if set.is_infinity(-lhs) {
        lhs = -set.infinity();
    } else if set.is_infinity(lhs) {
        lhs = set.infinity();
    }

    if set.is_gt(lhs, rhs) {
        log::warn!("left hand side of linear constraint greater than right hand side");
        log::warn!(" -> lhs={lhs}, rhs={rhs}");
    }

    let mut c = ConsData {
        varssize: 0,
        nvars: vars.len() as i32,
        hascontvar: false,
        hasnonbinvar: false,
        hasnonbinvalid: true,
        ..ConsData::default()
    };

    let mut constant = 0.0;
    if !vars.is_empty() {
        // Copy variables into temporary buffer.
        let mut varsbuffer = Vec::with_capacity(vars.len());
        let mut valsbuffer = Vec::with_capacity(vars.len());

        // Loop over variables and sort out fixed ones.
        let compression = api::is_cons_compression_enabled(scip);
        for v in 0..vars.len() {
            let var = vars[v];
            let val = vals[v];

            if !scip.set.is_zero(val) {
                let x = &scip.vars[var];
                // Treat fixed variable as a constant if problem compression is
                // enabled.
                if compression && scip.set.is_eq(x.glbdom.lb, x.glbdom.ub) {
                    constant += x.glbdom.lb * val;
                } else {
                    varsbuffer.push(var);
                    valsbuffer.push(val);

                    if !c.hascontvar && !crate::var::var_is_binary(scip, var) {
                        c.hasnonbinvar = true;

                        if !crate::var::var_is_integral(scip, var) {
                            c.hascontvar = true;
                        }
                    }
                }
            }
        }
        c.nvars = varsbuffer.len() as i32;

        if c.nvars > 0 {
            // Copy the possibly reduced buffer arrays into block.
            c.varssize = c.nvars;
            c.vars = CVec(varsbuffer);
            c.vals = CVec(valsbuffer);
        }
    }

    c.eventdata = None;

    // Due to compressed copying, we may have fixed variables contributing to
    // the left and right hand side.
    if !scip.set.is_zero(constant) {
        if !scip.set.is_infinity(lhs.abs()) {
            lhs -= constant;
        }

        if !scip.set.is_infinity(rhs.abs()) {
            rhs -= constant;
        }
    }

    let invalid = crate::def::INVALID;
    c.row = NONE;
    c.lhs = lhs;
    c.rhs = rhs;
    c.maxabsval = invalid;
    c.minabsval = invalid;
    c.minactivity = Quad::new(invalid);
    c.maxactivity = Quad::new(invalid);
    c.lastminactivity = invalid;
    c.lastmaxactivity = invalid;
    c.maxactdelta = invalid;
    c.maxactdeltavar = NONE;
    c.minactivityneginf = -1;
    c.minactivityposinf = -1;
    c.maxactivityneginf = -1;
    c.maxactivityposinf = -1;
    c.minactivityneghuge = -1;
    c.minactivityposhuge = -1;
    c.maxactivityneghuge = -1;
    c.maxactivityposhuge = -1;
    c.glbminactivity = Quad::new(invalid);
    c.glbmaxactivity = Quad::new(invalid);
    c.lastglbminactivity = invalid;
    c.lastglbmaxactivity = invalid;
    c.glbminactivityneginf = -1;
    c.glbminactivityposinf = -1;
    c.glbmaxactivityneginf = -1;
    c.glbmaxactivityposinf = -1;
    c.glbminactivityneghuge = -1;
    c.glbminactivityposhuge = -1;
    c.glbmaxactivityneghuge = -1;
    c.glbmaxactivityposhuge = -1;
    c.possignature = 0;
    c.negsignature = 0;
    c.validmaxabsval = false;
    c.validminabsval = false;
    c.validactivities = false;
    c.validminact = false;
    c.validmaxact = false;
    c.validglbminact = false;
    c.validglbmaxact = false;
    c.boundstightened = 0;
    c.presolved = false;
    c.removedfixings = false;
    c.validsignature = false;
    c.changed = true;
    c.normalized = false;
    c.upgradetried = false;
    c.upgraded = false;
    c.indexsorted = c.nvars <= 1;
    c.merged = c.nvars <= 1;
    c.cliquesadded = false;
    c.implsadded = false;
    c.coefsorted = false;
    c.nbinvars = -1;
    c.varsdeleted = false;
    c.rangedrowpropagated = 0;
    c.checkabsolute = false;

    if api::is_transformed(scip) {
        // Get transformed variables.
        let n = c.nvars as usize;
        api::get_transformed_vars(scip, &mut c.vars.0[..n])?;
    }

    // Capture variables.
    for v in 0..c.nvars as usize {
        // Likely implies a deleted variable.
        if c.vars[v] == NONE {
            return Err(Retcode::InvalidData);
        }
        crate::var::var_capture(scip, c.vars[v]);
    }

    Ok(c)
}

/// `consdataFree`: frees a linear constraint data.
fn consdata_free(scip: &mut Scip, c: &mut ConsData) -> ScipResult {
    // Release the row.
    if c.row != NONE {
        crate::lp::row_release(scip, c.row)?;
        c.row = NONE;
    }

    // Release variables.
    for v in 0..c.nvars as usize {
        let mut var = c.vars[v];
        api::release_var(scip, &mut var)?;
        c.vars[v] = var;
    }

    Ok(())
}

/// Runs `f` on the constraint data taken out of the constraint; for the
/// methods that only read SCIP while changing the data (they trigger no
/// events, so no handler looks for the data meanwhile).
fn with_cd<R>(scip: &mut Scip, cons: ConsId, f: impl FnOnce(&mut Scip, &mut ConsData) -> R) -> R {
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let r = f(scip, data.downcast_mut::<ConsData>().unwrap());
    scip.conss[cons].consdata = Some(data);
    r
}

/// `consdataInvalidateActivities`: invalidates activity bounds, such that
/// they are recalculated in next get.
fn consdata_invalidate_activities(c: &mut ConsData) {
    let invalid = crate::def::INVALID;
    c.validactivities = false;
    c.validminact = false;
    c.validmaxact = false;
    c.validglbminact = false;
    c.validglbmaxact = false;
    c.validmaxabsval = false;
    c.validminabsval = false;
    c.hasnonbinvalid = false;
    c.minactivity = Quad::new(invalid);
    c.maxactivity = Quad::new(invalid);
    c.lastminactivity = invalid;
    c.lastmaxactivity = invalid;
    c.maxabsval = invalid;
    c.minabsval = invalid;
    c.maxactdelta = invalid;
    c.maxactdeltavar = NONE;
    c.minactivityneginf = -1;
    c.minactivityposinf = -1;
    c.maxactivityneginf = -1;
    c.maxactivityposinf = -1;
    c.minactivityneghuge = -1;
    c.minactivityposhuge = -1;
    c.maxactivityneghuge = -1;
    c.maxactivityposhuge = -1;
    c.glbminactivity = Quad::new(invalid);
    c.glbmaxactivity = Quad::new(invalid);
    c.lastglbminactivity = invalid;
    c.lastglbmaxactivity = invalid;
    c.glbminactivityneginf = -1;
    c.glbminactivityposinf = -1;
    c.glbmaxactivityneginf = -1;
    c.glbmaxactivityposinf = -1;
    c.glbminactivityneghuge = -1;
    c.glbminactivityposhuge = -1;
    c.glbmaxactivityneghuge = -1;
    c.glbmaxactivityposhuge = -1;
}

/// `consdataComputePseudoActivity`: compute the pseudo activity of a
/// constraint.
fn consdata_compute_pseudo_activity(scip: &Scip, c: &ConsData) -> f64 {
    let mut pseudoactivity = 0.0;
    let mut pseudoactivityposinf = 0;
    let mut pseudoactivityneginf = 0;

    let mut i = c.nvars - 1;
    while i >= 0 {
        let bound = crate::var::var_get_best_bound_local(scip, c.vars[i]);
        let val = c.vals[i];
        if scip.set.is_infinity(-bound) {
            if val < 0.0 {
                pseudoactivityposinf += 1;
            } else {
                pseudoactivityneginf += 1;
            }
        } else if scip.set.is_infinity(bound) {
            if val < 0.0 {
                pseudoactivityneginf += 1;
            } else {
                pseudoactivityposinf += 1;
            }
        } else {
            pseudoactivity += val * bound;
        }
        i -= 1;
    }

    // Invalidate pseudo activity for contradicting contributions.
    if pseudoactivityneginf > 0 && pseudoactivityposinf > 0 {
        crate::def::INVALID
    } else if pseudoactivityneginf > 0 {
        -scip.set.infinity()
    } else if pseudoactivityposinf > 0 {
        scip.set.infinity()
    } else {
        pseudoactivity
    }
}

/// The activity sum of `consdataRecomputeMinactivity` and its relatives:
/// the coefficients times the bound `bound(var, val)` picks, leaving out
/// infinite and huge contributions.
fn consdata_recompute_activity(scip: &Scip, c: &ConsData, bound: impl Fn(&crate::var::Var, f64) -> f64) -> Quad {
    let mut act = Quad::new(0.0);
    let mut i = c.nvars - 1;
    while i >= 0 {
        let b = bound(&scip.vars[c.vars[i]], c.vals[i]);
        if !scip.set.is_infinity(b) && !scip.set.is_infinity(-b) && !scip.set.is_huge_value(c.vals[i] * b) && !scip.set.is_huge_value(-c.vals[i] * b) {
            act = sum_qd(act, c.vals[i] * b);
        }
        i -= 1;
    }
    act
}

/// `consdataRecomputeMinactivity`: recompute the minactivity of a
/// constraint.
fn consdata_recompute_minactivity(scip: &Scip, c: &mut ConsData) {
    c.minactivity = consdata_recompute_activity(scip, c, |v, val| if val > 0.0 { v.locdom.lb } else { v.locdom.ub });

    // The activity was just computed from scratch and is valid now.
    c.validminact = true;

    // The activity was just computed from scratch, mark it to be reliable.
    c.lastminactivity = c.minactivity.to_dbl();
}

/// `consdataRecomputeMaxactivity`: recompute the maxactivity of a
/// constraint.
fn consdata_recompute_maxactivity(scip: &Scip, c: &mut ConsData) {
    c.maxactivity = consdata_recompute_activity(scip, c, |v, val| if val > 0.0 { v.locdom.ub } else { v.locdom.lb });

    // The activity was just computed from scratch and is valid now.
    c.validmaxact = true;

    // The activity was just computed from scratch, mark it to be reliable.
    c.lastmaxactivity = c.maxactivity.to_dbl();
}

/// `consdataRecomputeGlbMinactivity`: recompute the global minactivity of a
/// constraint.
fn consdata_recompute_glb_minactivity(scip: &Scip, c: &mut ConsData) {
    c.glbminactivity = consdata_recompute_activity(scip, c, |v, val| if val > 0.0 { v.glbdom.lb } else { v.glbdom.ub });

    // The activity was just computed from scratch and is valid now.
    c.validglbminact = true;

    // The activity was just computed from scratch, mark it to be reliable.
    c.lastglbminactivity = c.glbminactivity.to_dbl();
}

/// `consdataRecomputeGlbMaxactivity`: recompute the global maxactivity of a
/// constraint.
fn consdata_recompute_glb_maxactivity(scip: &Scip, c: &mut ConsData) {
    c.glbmaxactivity = consdata_recompute_activity(scip, c, |v, val| if val > 0.0 { v.glbdom.ub } else { v.glbdom.lb });

    // The activity was just computed from scratch and is valid now.
    c.validglbmaxact = true;

    // The activity was just computed from scratch, mark it to be reliable.
    c.lastglbmaxactivity = c.glbmaxactivity.to_dbl();
}

/// `consdataCalcMaxAbsval`: calculates maximum absolute value of
/// coefficients.
fn consdata_calc_max_absval(c: &mut ConsData) {
    c.validmaxabsval = true;
    c.maxabsval = 0.0;
    for i in 0..c.nvars as usize {
        let absval = c.vals[i].abs();
        if absval > c.maxabsval {
            c.maxabsval = absval;
        }
    }
}

/// `consdataCalcMinAbsval`: calculates minimum absolute value of
/// coefficients.
fn consdata_calc_min_absval(c: &mut ConsData) {
    c.validminabsval = true;

    c.minabsval = if c.nvars > 0 { c.vals[0usize].abs() } else { 0.0 };

    for i in 1..c.nvars as usize {
        let absval = c.vals[i].abs();
        if absval < c.minabsval {
            c.minabsval = absval;
        }
    }
}

/// `consdataCheckNonbinvar`: checks the type of all variables of the
/// constraint and sets hasnonbinvar and hascontvar flags accordingly.
fn consdata_check_nonbinvar(scip: &Scip, c: &mut ConsData) {
    c.hasnonbinvar = false;
    c.hascontvar = false;

    let mut v = c.nvars - 1;
    while v >= 0 {
        if !crate::var::var_is_binary(scip, c.vars[v]) {
            c.hasnonbinvar = true;

            if !crate::var::var_is_integral(scip, c.vars[v]) {
                c.hascontvar = true;
                break;
            }
        }
        v -= 1;
    }
    c.hasnonbinvalid = true;
}

/// `consdataRecomputeMaxActivityDelta`: recompute maximal activity
/// contribution for a single variable.
fn consdata_recompute_max_activity_delta(scip: &Scip, c: &mut ConsData) {
    c.maxactdelta = 0.0;

    if !c.hasnonbinvalid {
        consdata_check_nonbinvar(scip, c);
    }

    // Easy case, the problem consists only of binary variables.
    if !c.hasnonbinvar {
        let mut v = c.nvars - 1;
        while v >= 0 {
            let x = &scip.vars[c.vars[v]];
            if x.locdom.lb < 0.5 && x.locdom.ub > 0.5 {
                let delta = c.vals[v].abs();

                if delta > c.maxactdelta {
                    c.maxactdelta = delta;
                    c.maxactdeltavar = c.vars[v];
                }
            }
            v -= 1;
        }
        return;
    }

    let mut v = c.nvars - 1;
    while v >= 0 {
        let x = &scip.vars[c.vars[v]];
        let lb = x.locdom.lb;
        let ub = x.locdom.ub;

        if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
            c.maxactdelta = scip.set.infinity();
            c.maxactdeltavar = c.vars[v];
            break;
        }

        let domain = ub - lb;
        let delta = c.vals[v].abs() * domain;

        if delta > c.maxactdelta {
            c.maxactdelta = delta;
            c.maxactdeltavar = c.vars[v];
        }
        v -= 1;
    }
}

/// `consdataUpdateActivities`: updates activities for a change in a bound;
/// `var` can be `NONE` for global bound changes.
#[allow(clippy::too_many_arguments)]
fn consdata_update_activities(scip: &Scip, c: &mut ConsData, _var: VarId, oldbound: f64, newbound: f64, val: f64, boundtype: BoundType, global: bool, checkreliability: bool) {
    let mut delta = Quad::new(0.0);

    // Depending on the boundtype and the coefficient, we choose the activity
    // to be updated:
    //   lower bound + pos. coef: update minactivity
    //   lower bound + neg. coef: update maxactivity, positive and negative
    //                            infinity counters have to be switched
    //   upper bound + pos. coef: update maxactivity
    //   upper bound + neg. coef: update minactivity, positive and negative
    //                            infinity counters have to be switched
    let minside = (boundtype == BoundType::Lower) == (val > 0.0);
    let swap = val <= 0.0;
    let ConsData {
        minactivity,
        maxactivity,
        lastminactivity,
        lastmaxactivity,
        glbminactivity,
        glbmaxactivity,
        lastglbminactivity,
        lastglbmaxactivity,
        minactivityneginf,
        minactivityposinf,
        maxactivityneginf,
        maxactivityposinf,
        minactivityneghuge,
        minactivityposhuge,
        maxactivityneghuge,
        maxactivityposhuge,
        glbminactivityneginf,
        glbminactivityposinf,
        glbmaxactivityneginf,
        glbmaxactivityposinf,
        glbminactivityneghuge,
        glbminactivityposhuge,
        glbmaxactivityneghuge,
        glbmaxactivityposhuge,
        validminact,
        validmaxact,
        validglbminact,
        validglbmaxact,
        ..
    } = c;
    let (activity, lastactivity, posinf, neginf, poshuge, neghuge, valid) = match (global, minside) {
        (true, true) => (
            glbminactivity,
            lastglbminactivity,
            glbminactivityposinf,
            glbminactivityneginf,
            glbminactivityposhuge,
            glbminactivityneghuge,
            validglbminact,
        ),
        (true, false) => (
            glbmaxactivity,
            lastglbmaxactivity,
            glbmaxactivityposinf,
            glbmaxactivityneginf,
            glbmaxactivityposhuge,
            glbmaxactivityneghuge,
            validglbmaxact,
        ),
        (false, true) => (
            minactivity,
            lastminactivity,
            minactivityposinf,
            minactivityneginf,
            minactivityposhuge,
            minactivityneghuge,
            validminact,
        ),
        (false, false) => (
            maxactivity,
            lastmaxactivity,
            maxactivityposinf,
            maxactivityneginf,
            maxactivityposhuge,
            maxactivityneghuge,
            validmaxact,
        ),
    };
    // With a negative coefficient the infinity counters are switched.
    let (activityposinf, activityneginf) = if swap { (neginf, posinf) } else { (posinf, neginf) };
    let (activityposhuge, activityneghuge) = (poshuge, neghuge);
    let validact = *valid;

    let oldcontribution = val * oldbound;
    let newcontribution = val * newbound;
    let hugevalnewcont = scip.set.is_huge_value(newcontribution.abs());
    let finitenewbound = !scip.set.is_infinity(newbound.abs());

    if scip.set.is_infinity(oldbound.abs()) {
        // Old bound was +infinity.
        if oldbound > 0.0 {
            // We only have to do something if the new bound is not again
            // +infinity.
            if finitenewbound || newbound < 0.0 {
                // Decrease the counter for positive infinite contributions.
                *activityposinf -= 1;

                // If the bound changed to -infinity, increase the counter for
                // negative infinite contributions.
                if !finitenewbound && newbound < 0.0 {
                    *activityneginf += 1;
                } else if hugevalnewcont {
                    // If the contribution of this variable is too large, increase
                    // the counter for huge values.
                    if newcontribution > 0.0 {
                        *activityposhuge += 1;
                    } else {
                        *activityneghuge += 1;
                    }
                }
                // "normal case": just add the contribution to the activity.
                else {
                    delta = Quad::new(newcontribution);
                }
            }
        }
        // Old bound was -infinity.
        else {
            // We only have to do something ig the new bound is not again
            // -infinity.
            if finitenewbound || newbound > 0.0 {
                // Decrease the counter for negative infinite contributions.
                *activityneginf -= 1;

                // If the bound changed to +infinity, increase the counter for
                // positive infinite contributions.
                if !finitenewbound && newbound > 0.0 {
                    *activityposinf += 1;
                } else if hugevalnewcont {
                    // If the contribution of this variable is too large, increase
                    // the counter for huge values.
                    if newcontribution > 0.0 {
                        *activityposhuge += 1;
                    } else {
                        *activityneghuge += 1;
                    }
                }
                // "normal case": just add the contribution to the activity.
                else {
                    delta = Quad::new(newcontribution);
                }
            }
        }
    } else if scip.set.is_huge_value(oldcontribution.abs()) {
        // Old contribution was too large and positive (negative): decrease the
        // counter for huge contributions; it might be increased again later,
        // but checking here that the bound is not huge again would not handle a
        // change from a huge to an infinite bound.
        if oldcontribution > 0.0 {
            *activityposhuge -= 1;
        } else {
            *activityneghuge -= 1;
        }

        if !finitenewbound {
            // If the bound changed to +infinity, increase the counter for
            // positive infinite contributions.
            if newbound > 0.0 {
                *activityposinf += 1;
            }
            // If the bound changed to -infinity, increase the counter for
            // negative infinite contributions.
            else {
                *activityneginf += 1;
            }
        } else if hugevalnewcont {
            // If the contribution of this variable is too large and positive,
            // increase the corresponding counter.
            if newcontribution > 0.0 {
                *activityposhuge += 1;
            }
            // If the contribution of this variable is too large and negative,
            // increase the corresponding counter.
            else {
                *activityneghuge += 1;
            }
        }
        // "normal case": just add the contribution to the activity.
        else {
            delta = Quad::new(newcontribution);
        }
    }
    // Old bound was finite and not too large.
    else if !finitenewbound {
        // If the new bound is +infinity (-infinity), the old contribution has to
        // be subtracted and the counter for positive (negative) infinite
        // contributions has to be increased.
        if newbound > 0.0 {
            *activityposinf += 1;
            delta = Quad::new(-oldcontribution);
        } else {
            *activityneginf += 1;
            delta = Quad::new(-oldcontribution);
        }
    }
    // If the contribution of this variable is too large, increase the counter
    // for huge values.
    else if hugevalnewcont {
        if newcontribution > 0.0 {
            *activityposhuge += 1;
            delta = Quad::new(-oldcontribution);
        } else {
            *activityneghuge += 1;
            delta = Quad::new(-oldcontribution);
        }
    }
    // "normal case": just update the activity.
    else {
        delta = Quad::new(newcontribution);
        delta = sum_qd(delta, -oldcontribution);
    }

    // Update the activity, if the current value is valid and there was a
    // change in the finite part.
    if validact && delta.to_dbl() != 0.0 {
        // If the absolute value of the activity is increased, this is regarded
        // as reliable, otherwise, we check whether we can still trust the
        // updated value.
        *activity = sum_qd(*activity, delta.to_dbl());

        let curractivity = activity.to_dbl();

        if lastactivity.abs() < curractivity.abs() {
            *lastactivity = curractivity;
        } else if checkreliability && scip.set.is_update_unreliable(curractivity, *lastactivity) {
            // Mark the activity that was just changed and is not reliable
            // anymore to be invalid.
            *valid = false;
        }
    }
}

/// `consdataUpdateActivitiesLb`: updates minimum and maximum activity for a
/// change in lower bound.
fn consdata_update_activities_lb(scip: &Scip, c: &mut ConsData, var: VarId, oldlb: f64, newlb: f64, val: f64, checkreliability: bool) {
    if c.validactivities {
        consdata_update_activities(scip, c, var, oldlb, newlb, val, BoundType::Lower, false, checkreliability);
    }
}

/// `consdataUpdateActivitiesUb`: updates minimum and maximum activity for a
/// change in upper bound.
fn consdata_update_activities_ub(scip: &Scip, c: &mut ConsData, var: VarId, oldub: f64, newub: f64, val: f64, checkreliability: bool) {
    if c.validactivities {
        consdata_update_activities(scip, c, var, oldub, newub, val, BoundType::Upper, false, checkreliability);
    }
}

/// `consdataUpdateActivitiesGlbLb`: updates minimum and maximum global
/// activity for a change in the global lower bound.
fn consdata_update_activities_glb_lb(scip: &Scip, c: &mut ConsData, oldlb: f64, newlb: f64, val: f64, checkreliability: bool) {
    if c.validactivities {
        consdata_update_activities(scip, c, NONE, oldlb, newlb, val, BoundType::Lower, true, checkreliability);
    }
}

/// `consdataUpdateActivitiesGlbUb`: updates minimum and maximum global
/// activity for a change in global upper bound.
fn consdata_update_activities_glb_ub(scip: &Scip, c: &mut ConsData, oldub: f64, newub: f64, val: f64, checkreliability: bool) {
    if c.validactivities {
        consdata_update_activities(scip, c, NONE, oldub, newub, val, BoundType::Upper, true, checkreliability);
    }
}

/// `consdataUpdateAddCoef`: updates minimum and maximum activity and
/// maximum absolute value for coefficient addition.
fn consdata_update_add_coef(scip: &Scip, c: &mut ConsData, var: VarId, val: f64, checkreliability: bool) {
    // Update maximum absolute value.
    if c.validmaxabsval {
        let absval = val.abs();
        c.maxabsval = crate::def::max(c.maxabsval, absval);
    }

    // Update minimum absolute value.
    if c.validminabsval {
        let absval = val.abs();
        c.minabsval = crate::def::min(c.minabsval, absval);
    }

    let x = &scip.vars[var];
    let (lb, ub, glb, gub) = (x.locdom.lb, x.locdom.ub, x.glbdom.lb, x.glbdom.ub);

    // Update minimum and maximum activity.
    if c.validactivities {
        consdata_update_activities_lb(scip, c, var, 0.0, lb, val, checkreliability);
        consdata_update_activities_ub(scip, c, var, 0.0, ub, val, checkreliability);
        consdata_update_activities_glb_lb(scip, c, 0.0, glb, val, checkreliability);
        consdata_update_activities_glb_ub(scip, c, 0.0, gub, val, checkreliability);
    }

    // Update maximum activity delta.
    if c.maxactdeltavar == NONE || !scip.set.is_infinity(c.maxactdelta) {
        if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
            c.maxactdelta = scip.set.infinity();
            c.maxactdeltavar = var;
        } else if c.maxactdeltavar != NONE {
            let domain = ub - lb;
            let delta = val.abs() * domain;

            if delta > c.maxactdelta {
                c.maxactdelta = delta;
                c.maxactdeltavar = var;
            }
        }
    }
}

/// `consdataUpdateDelCoef`: updates minimum and maximum activity for
/// coefficient deletion, invalidates maximum absolute value if necessary.
fn consdata_update_del_coef(scip: &Scip, c: &mut ConsData, var: VarId, val: f64, checkreliability: bool) {
    // Invalidate maximum absolute value, if this coefficient was the maximum.
    if c.validmaxabsval {
        let absval = val.abs();
        if scip.set.is_eq(absval, c.maxabsval) {
            c.validmaxabsval = false;
            c.maxabsval = crate::def::INVALID;
        }
    }

    // Invalidate minimum absolute value, if this coefficient was the minimum.
    if c.validminabsval {
        let absval = val.abs();
        if scip.set.is_eq(absval, c.minabsval) {
            c.validminabsval = false;
            c.minabsval = crate::def::INVALID;
        }
    }

    // Update minimum and maximum activity.
    if c.validactivities {
        let x = &scip.vars[var];
        let (lb, ub, glb, gub) = (x.locdom.lb, x.locdom.ub, x.glbdom.lb, x.glbdom.ub);
        consdata_update_activities_lb(scip, c, var, lb, 0.0, val, checkreliability);
        consdata_update_activities_ub(scip, c, var, ub, 0.0, val, checkreliability);
        consdata_update_activities_glb_lb(scip, c, glb, 0.0, val, checkreliability);
        consdata_update_activities_glb_ub(scip, c, gub, 0.0, val, checkreliability);
    }

    // Reset maximum activity delta so that it will be recalculated on the next
    // real propagation.
    if c.maxactdeltavar == var {
        c.maxactdelta = crate::def::INVALID;
        c.maxactdeltavar = NONE;
    }
}

/// `consdataUpdateChgCoef`: updates minimum and maximum activity for
/// coefficient change, invalidates maximum absolute value if necessary.
fn consdata_update_chg_coef(scip: &Scip, c: &mut ConsData, var: VarId, oldval: f64, newval: f64, checkreliability: bool) {
    // @todo do something more clever here, e.g. if oldval * newval >= 0, do
    // the update directly
    consdata_update_del_coef(scip, c, var, oldval, checkreliability);
    consdata_update_add_coef(scip, c, var, newval, checkreliability);
}

/// `consdataGetMaxAbsval`: returns the maximum absolute value of all
/// coefficients in the constraint.
fn consdata_get_max_absval(c: &mut ConsData) -> f64 {
    if !c.validmaxabsval {
        consdata_calc_max_absval(c);
    }
    c.maxabsval
}

/// `consdataGetMinAbsval`: returns the minimum absolute value of all
/// coefficients in the constraint.
fn consdata_get_min_absval(c: &mut ConsData) -> f64 {
    if !c.validminabsval {
        consdata_calc_min_absval(c);
    }
    c.minabsval
}

/// `consdataCalcActivities`: calculates minimum and maximum local and
/// global activity for constraint from scratch; additionally recalculates
/// maximum absolute value of coefficients.
fn consdata_calc_activities(scip: &Scip, c: &mut ConsData) {
    c.validmaxabsval = true;
    c.validminabsval = true;
    c.validactivities = true;
    c.validminact = true;
    c.validmaxact = true;
    c.validglbminact = true;
    c.validglbmaxact = true;
    c.maxabsval = 0.0;
    c.minabsval = if c.nvars == 0 { 0.0 } else { c.vals[0usize].abs() };
    c.minactivity = Quad::new(0.0);
    c.maxactivity = Quad::new(0.0);
    c.lastminactivity = 0.0;
    c.lastmaxactivity = 0.0;
    c.minactivityneginf = 0;
    c.minactivityposinf = 0;
    c.maxactivityneginf = 0;
    c.maxactivityposinf = 0;
    c.minactivityneghuge = 0;
    c.minactivityposhuge = 0;
    c.maxactivityneghuge = 0;
    c.maxactivityposhuge = 0;
    c.glbminactivity = Quad::new(0.0);
    c.glbmaxactivity = Quad::new(0.0);
    c.lastglbminactivity = 0.0;
    c.lastglbmaxactivity = 0.0;
    c.glbminactivityneginf = 0;
    c.glbminactivityposinf = 0;
    c.glbmaxactivityneginf = 0;
    c.glbmaxactivityposinf = 0;
    c.glbminactivityneghuge = 0;
    c.glbminactivityposhuge = 0;
    c.glbmaxactivityneghuge = 0;
    c.glbmaxactivityposhuge = 0;

    for i in 0..c.nvars as usize {
        let (var, val) = (c.vars[i], c.vals[i]);
        consdata_update_add_coef(scip, c, var, val, false);
    }

    c.lastminactivity = c.minactivity.to_dbl();
    c.lastmaxactivity = c.maxactivity.to_dbl();
    c.lastglbminactivity = c.glbminactivity.to_dbl();
    c.lastglbmaxactivity = c.glbmaxactivity.to_dbl();
}

/// An activity bound: its value, whether it is tight to the variable
/// bounds (i.e. the actual activity, otherwise a relaxation is provided),
/// and whether it was set to infinity rather than calculated.
#[derive(Clone, Copy, Debug, Default)]
struct ActBound {
    act: f64,
    istight: bool,
    issettoinfinity: bool,
}

/// `getMinActivity`: gets minimal activity for constraint and given values
/// of counters for infinite and huge contributions and (if needed) delta to
/// subtract from stored finite part of activity in case of a residual
/// activity; `goodrelax`: should a good relaxation be computed or are
/// relaxed acticities ignored, anyway?
#[allow(clippy::too_many_arguments)]
fn get_min_activity(scip: &Scip, c: &mut ConsData, posinf: i32, neginf: i32, poshuge: i32, neghuge: i32, delta: f64, global: bool, goodrelax: bool) -> ActBound {
    // If we have neg. infinite contributions, the minactivity is -infty.
    if neginf > 0 {
        ActBound {
            act: -scip.set.infinity(),
            issettoinfinity: true,
            istight: posinf == 0,
        }
    }
    // If we have pos. (and no neg.) infinite contributions, the minactivity is
    // +infty.
    else if posinf > 0 {
        ActBound {
            act: scip.set.infinity(),
            issettoinfinity: true,
            istight: true,
        }
    }
    // If we have neg. huge contributions or do not need a good relaxation, we
    // just return -infty as minactivity.
    else if neghuge > 0 || (poshuge > 0 && !goodrelax) {
        ActBound {
            act: -scip.set.infinity(),
            issettoinfinity: true,
            istight: false,
        }
    } else {
        // Recompute minactivity if it is not valid.
        let mut tmpactivity = if global {
            if !c.validglbminact {
                consdata_recompute_glb_minactivity(scip, c);
            }
            c.glbminactivity
        } else {
            if !c.validminact {
                consdata_recompute_minactivity(scip, c);
            }
            c.minactivity
        };

        // Calculate residual minactivity.
        tmpactivity = sum_qd(tmpactivity, -delta);

        // We have no infinite and no neg. huge contributions, but pos. huge
        // contributions; a feasible relaxation of the minactivity is given by
        // adding the number of positive huge contributions times the huge
        // value.
        let istight;
        if poshuge > 0 {
            tmpactivity = sum_qd(tmpactivity, poshuge as f64 * scip.set.get_huge_value());
            istight = false;
        }
        // All counters are zero, so the minactivity is tight.
        else {
            istight = true;
        }

        // Round residual minactivity.
        ActBound {
            act: tmpactivity.to_dbl(),
            istight,
            issettoinfinity: false,
        }
    }
}

/// `getMaxActivity`: gets maximal activity for constraint and given values
/// of counters for infinite and huge contributions and (if needed) delta to
/// subtract from stored finite part of activity in case of a residual
/// activity.
#[allow(clippy::too_many_arguments)]
fn get_max_activity(scip: &Scip, c: &mut ConsData, posinf: i32, neginf: i32, poshuge: i32, neghuge: i32, delta: f64, global: bool, goodrelax: bool) -> ActBound {
    // If we have pos. infinite contributions, the maxactivity is +infty.
    if posinf > 0 {
        ActBound {
            act: scip.set.infinity(),
            issettoinfinity: true,
            istight: neginf == 0,
        }
    }
    // If we have neg. (and no pos.) infinite contributions, the maxactivity is
    // -infty.
    else if neginf > 0 {
        ActBound {
            act: -scip.set.infinity(),
            issettoinfinity: true,
            istight: true,
        }
    }
    // If we have pos. huge contributions or do not need a good relaxation, we
    // just return +infty as maxactivity.
    else if poshuge > 0 || (neghuge > 0 && !goodrelax) {
        ActBound {
            act: scip.set.infinity(),
            issettoinfinity: true,
            istight: false,
        }
    } else {
        // Recompute maxactivity if it is not valid.
        let mut tmpactivity = if global {
            if !c.validglbmaxact {
                consdata_recompute_glb_maxactivity(scip, c);
            }
            c.glbmaxactivity
        } else {
            if !c.validmaxact {
                consdata_recompute_maxactivity(scip, c);
            }
            c.maxactivity
        };

        // Calculate residual maxactivity.
        tmpactivity = sum_qd(tmpactivity, -delta);

        // We have no infinite and no pos. huge contributions, but neg. huge
        // contributions; a feasible relaxation of the maxactivity is given by
        // subtracting the number of negative huge contributions times the huge
        // value.
        let istight;
        if neghuge > 0 {
            tmpactivity = sum_qd(tmpactivity, -neghuge as f64 * scip.set.get_huge_value());
            istight = false;
        }
        // All counters are zero, so the maxactivity is tight.
        else {
            istight = true;
        }

        // Round residual maxactivity.
        ActBound {
            act: tmpactivity.to_dbl(),
            istight,
            issettoinfinity: false,
        }
    }
}

/// `consdataGetActivityBounds`: gets activity bounds for constraint; returns
/// the minimal and the maximal activity.
fn consdata_get_activity_bounds(scip: &Scip, c: &mut ConsData, goodrelax: bool) -> (ActBound, ActBound) {
    if !c.validactivities {
        consdata_calc_activities(scip, c);
    }

    let (pi, ni, ph, nh) = (c.minactivityposinf, c.minactivityneginf, c.minactivityposhuge, c.minactivityneghuge);
    let min = get_min_activity(scip, c, pi, ni, ph, nh, 0.0, false, goodrelax);
    let (pi, ni, ph, nh) = (c.maxactivityposinf, c.maxactivityneginf, c.maxactivityposhuge, c.maxactivityneghuge);
    let max = get_max_activity(scip, c, pi, ni, ph, nh, 0.0, false, goodrelax);
    (min, max)
}

/// `consdataGetReliableResidualActivity`: calculates activity bounds for
/// constraint after setting variable to zero.
fn consdata_get_reliable_residual_activity(scip: &Scip, c: &ConsData, cancelvar: VarId, isminresact: bool, useglobalbounds: bool) -> f64 {
    let mut resactivity = 0.0;

    for v in 0..c.nvars as usize {
        let var = c.vars[v];
        if var == cancelvar {
            continue;
        }

        let val = c.vals[v];
        let x = &scip.vars[var];
        let (lb, ub) = if useglobalbounds { (x.glbdom.lb, x.glbdom.ub) } else { (x.locdom.lb, x.locdom.ub) };

        if val > 0.0 {
            if isminresact {
                resactivity += val * lb;
            } else {
                resactivity += val * ub;
            }
        } else if isminresact {
            resactivity += val * ub;
        } else {
            resactivity += val * lb;
        }
    }
    resactivity
}

/// `consdataGetActivityResiduals`: gets activity bounds for constraint
/// after setting variable to zero; returns the minimal and maximal residual
/// activity.
fn consdata_get_activity_residuals(scip: &Scip, c: &mut ConsData, var: VarId, val: f64, goodrelax: bool) -> (ActBound, ActBound) {
    // Get activity bounds of linear constraint.
    if !c.validactivities {
        consdata_calc_activities(scip, c);
    }

    let x = &scip.vars[var];
    let (minactbound, maxactbound, absval) = if val > 0.0 {
        (x.locdom.lb, x.locdom.ub, val)
    } else {
        (-x.locdom.ub, -x.locdom.lb, -val)
    };

    // Get/compute minactivity by calling getMinActivity() with updated counters
    // for infinite and huge values and contribution of variable set to zero
    // that has to be subtracted from finite part of activity.
    let (pi, ni, ph, nh) = (c.minactivityposinf, c.minactivityneginf, c.minactivityposhuge, c.minactivityneghuge);
    let min = if scip.set.is_infinity(minactbound) {
        get_min_activity(scip, c, pi - 1, ni, ph, nh, 0.0, false, goodrelax)
    } else if scip.set.is_infinity(-minactbound) {
        get_min_activity(scip, c, pi, ni - 1, ph, nh, 0.0, false, goodrelax)
    } else if scip.set.is_huge_value(minactbound * absval) {
        get_min_activity(scip, c, pi, ni, ph - 1, nh, 0.0, false, goodrelax)
    } else if scip.set.is_huge_value(-minactbound * absval) {
        get_min_activity(scip, c, pi, ni, ph, nh - 1, 0.0, false, goodrelax)
    } else {
        get_min_activity(scip, c, pi, ni, ph, nh, absval * minactbound, false, goodrelax)
    };

    // Get/compute maxactivity by calling getMaxActivity() with updated counters
    // for infinite and huge values and contribution of variable set to zero
    // that has to be subtracted from finite part of activity.
    let (pi, ni, ph, nh) = (c.maxactivityposinf, c.maxactivityneginf, c.maxactivityposhuge, c.maxactivityneghuge);
    let max = if scip.set.is_infinity(-maxactbound) {
        get_max_activity(scip, c, pi, ni - 1, ph, nh, 0.0, false, goodrelax)
    } else if scip.set.is_infinity(maxactbound) {
        get_max_activity(scip, c, pi - 1, ni, ph, nh, 0.0, false, goodrelax)
    } else if scip.set.is_huge_value(absval * maxactbound) {
        get_max_activity(scip, c, pi, ni, ph - 1, nh, 0.0, false, goodrelax)
    } else if scip.set.is_huge_value(-absval * maxactbound) {
        get_max_activity(scip, c, pi, ni, ph, nh - 1, 0.0, false, goodrelax)
    } else {
        get_max_activity(scip, c, pi, ni, ph, nh, absval * maxactbound, false, goodrelax)
    };

    (min, max)
}

/// `consdataGetGlbActivityBounds`: gets global activity bounds for
/// constraint.
/// Only the wanted sides are computed (C passes `NULL` for the others), as
/// computing one may recompute its stored activity.
fn consdata_get_glb_activity_bounds(scip: &Scip, c: &mut ConsData, goodrelax: bool, wantmin: bool, wantmax: bool) -> (ActBound, ActBound) {
    if !c.validactivities {
        consdata_calc_activities(scip, c);
    }

    let mut min = ActBound::default();
    let mut max = ActBound::default();
    if wantmin {
        let (pi, ni, ph, nh) = (c.glbminactivityposinf, c.glbminactivityneginf, c.glbminactivityposhuge, c.glbminactivityneghuge);
        min = get_min_activity(scip, c, pi, ni, ph, nh, 0.0, true, goodrelax);
    }
    if wantmax {
        let (pi, ni, ph, nh) = (c.glbmaxactivityposinf, c.glbmaxactivityneginf, c.glbmaxactivityposhuge, c.glbmaxactivityneghuge);
        max = get_max_activity(scip, c, pi, ni, ph, nh, 0.0, true, goodrelax);
    }
    (min, max)
}

/// `consdataGetGlbActivityResiduals`: gets global activity bounds for
/// constraint after setting variable to zero.
/// Only the wanted sides are computed, like [`consdata_get_glb_activity_bounds`].
fn consdata_get_glb_activity_residuals(scip: &Scip, c: &mut ConsData, var: VarId, val: f64, goodrelax: bool, wantmin: bool, wantmax: bool) -> (ActBound, ActBound) {
    // Get activity bounds of linear constraint.
    if !c.validactivities {
        consdata_calc_activities(scip, c);
    }

    let x = &scip.vars[var];
    let (minactbound, maxactbound, absval) = if val > 0.0 {
        (x.glbdom.lb, x.glbdom.ub, val)
    } else {
        (-x.glbdom.ub, -x.glbdom.lb, -val)
    };

    let (pi, ni, ph, nh) = (c.glbminactivityposinf, c.glbminactivityneginf, c.glbminactivityposhuge, c.glbminactivityneghuge);
    let min = if !wantmin {
        ActBound::default()
    } else if scip.set.is_infinity(minactbound) {
        get_min_activity(scip, c, pi - 1, ni, ph, nh, 0.0, true, goodrelax)
    } else if scip.set.is_infinity(-minactbound) {
        get_min_activity(scip, c, pi, ni - 1, ph, nh, 0.0, true, goodrelax)
    } else if scip.set.is_huge_value(minactbound * absval) {
        get_min_activity(scip, c, pi, ni, ph - 1, nh, 0.0, true, goodrelax)
    } else if scip.set.is_huge_value(-minactbound * absval) {
        get_min_activity(scip, c, pi, ni, ph, nh - 1, 0.0, true, goodrelax)
    } else {
        get_min_activity(scip, c, pi, ni, ph, nh, absval * minactbound, true, goodrelax)
    };

    let (pi, ni, ph, nh) = (c.glbmaxactivityposinf, c.glbmaxactivityneginf, c.glbmaxactivityposhuge, c.glbmaxactivityneghuge);
    let max = if !wantmax {
        ActBound::default()
    } else if scip.set.is_infinity(-maxactbound) {
        get_max_activity(scip, c, pi, ni - 1, ph, nh, 0.0, true, goodrelax)
    } else if scip.set.is_infinity(maxactbound) {
        get_max_activity(scip, c, pi - 1, ni, ph, nh, 0.0, true, goodrelax)
    } else if scip.set.is_huge_value(absval * maxactbound) {
        get_max_activity(scip, c, pi, ni, ph - 1, nh, 0.0, true, goodrelax)
    } else if scip.set.is_huge_value(-absval * maxactbound) {
        get_max_activity(scip, c, pi, ni, ph, nh - 1, 0.0, true, goodrelax)
    } else {
        get_max_activity(scip, c, pi, ni, ph, nh, absval * maxactbound, true, goodrelax)
    };

    (min, max)
}

/// `consdataGetActivity`: calculates the activity of the linear constraint
/// for given solution (`NONE` for the current solution).
fn consdata_get_activity(scip: &mut Scip, c: &ConsData, sol: SolId) -> f64 {
    let mut activity;

    if sol == NONE && !has_current_node_lp(scip) {
        activity = consdata_compute_pseudo_activity(scip, c);
    } else {
        activity = 0.0;
        let mut nposinf = 0;
        let mut nneginf = 0;

        for v in 0..c.nvars as usize {
            let solval = get_sol_val(scip, sol, c.vars[v]);
            let negsign = c.vals[v] < 0.0;

            if (scip.set.is_infinity(solval) && !negsign) || (scip.set.is_infinity(-solval) && negsign) {
                nposinf += 1;
            } else if (scip.set.is_infinity(solval) && negsign) || (scip.set.is_infinity(-solval) && !negsign) {
                nneginf += 1;
            } else {
                activity += c.vals[v] * solval;
            }
        }

        // Invalidate activity for contradicting contributions.
        if nposinf > 0 && nneginf > 0 {
            activity = INVALID;
        } else if nneginf > 0 {
            activity = -scip.set.infinity();
        } else if nposinf > 0 {
            activity = scip.set.infinity();
        }
    }

    if activity == INVALID {
        return activity;
    } else if activity < 0.0 {
        activity = crate::def::max(activity, -scip.set.infinity());
    } else {
        activity = crate::def::min(activity, scip.set.infinity());
    }

    activity
}

/// `consdataGetFeasibility`: calculates the feasibility of the linear
/// constraint for given solution.
fn consdata_get_feasibility(scip: &mut Scip, c: &ConsData, sol: SolId) -> f64 {
    let activity = consdata_get_activity(scip, c, sol);

    if activity == INVALID {
        return -scip.set.infinity();
    }

    crate::def::min(c.rhs - activity, activity - c.lhs)
}

/// `consdataUpdateSignatures`: updates bit signatures after adding a single
/// coefficient.
fn consdata_update_signatures(scip: &Scip, c: &mut ConsData, pos: usize) {
    let x = &scip.vars[c.vars[pos]];
    let varsignature = hash_signature64(x.index as u32);
    let lb = x.glbdom.lb;
    let ub = x.glbdom.ub;
    let val = c.vals[pos];
    if (val > 0.0 && ub > 0.0) || (val < 0.0 && lb < 0.0) {
        c.possignature |= varsignature;
    }
    if (val > 0.0 && lb < 0.0) || (val < 0.0 && ub > 0.0) {
        c.negsignature |= varsignature;
    }
}

/// `consdataCalcSignatures`: calculates the bit signatures of the given
/// constraint data.
fn consdata_calc_signatures(scip: &Scip, c: &mut ConsData) {
    if !c.validsignature {
        c.validsignature = true;
        c.possignature = 0;
        c.negsignature = 0;
        for i in 0..c.nvars as usize {
            consdata_update_signatures(scip, c, i);
        }
    }
}

/// The variable type ordering of the comparators: implied integral
/// variables as the deprecated `SCIP_VARTYPE_IMPLINT`.
fn comp_vartype(scip: &Scip, var: VarId) -> VarType {
    if var_is_implied_integral(scip, var) { VarType::ImplInt } else { scip.vars[var].vartype }
}

/// `EPSGT(x, y, eps)`.
#[inline]
fn eps_gt(x: f64, y: f64, eps: f64) -> bool {
    x - y > eps
}

/// `consdataCompVar`: index comparison method of linear constraints:
/// compares two indices of the variable set in the linear constraint.
fn consdata_comp_var(scip: &Scip, c: &ConsData, ind1: usize, ind2: usize) -> i32 {
    let var1 = c.vars[ind1];
    let var2 = c.vars[ind2];

    // Exactly one variable is binary.
    if var_is_binary(scip, var1) != var_is_binary(scip, var2) {
        if var_is_binary(scip, var1) { -1 } else { 1 }
    }
    // Both variables are binary.
    else if var_is_binary(scip, var1) {
        var_compare(scip, var1, var2)
    } else {
        let vartype1 = comp_vartype(scip, var1);
        let vartype2 = comp_vartype(scip, var2);

        if vartype1 < vartype2 {
            -1
        } else if vartype1 > vartype2 {
            1
        } else {
            var_compare(scip, var1, var2)
        }
    }
}

/// `consdataCompVarProp`: index comparison method of linear constraints for
/// propagation order.
fn consdata_comp_var_prop(scip: &Scip, c: &ConsData, ind1: usize, ind2: usize) -> i32 {
    let var1 = c.vars[ind1];
    let var2 = c.vars[ind2];
    let probindex = |v: VarId| scip.vars[v].probindex;

    // Exactly one variable is binary.
    if var_is_binary(scip, var1) != var_is_binary(scip, var2) {
        if var_is_binary(scip, var1) { -1 } else { 1 }
    }
    // Both variables are binary.
    else if var_is_binary(scip, var1) {
        let abscoef1 = c.vals[ind1].abs();
        let abscoef2 = c.vals[ind2].abs();

        if eps_gt(abscoef1, abscoef2, 1e-9) {
            -1
        } else if eps_gt(abscoef2, abscoef1, 1e-9) {
            1
        } else {
            probindex(var1) - probindex(var2)
        }
    } else {
        let vartype1 = comp_vartype(scip, var1);
        let vartype2 = comp_vartype(scip, var2);

        if vartype1 < vartype2 {
            -1
        } else if vartype1 > vartype2 {
            1
        } else {
            // Both variables are continuous.
            if !var_is_integral(scip, var1) {
                probindex(var1) - probindex(var2)
            } else {
                let x1 = &scip.vars[var1];
                let x2 = &scip.vars[var2];
                let abscont1 = (c.vals[ind1] * (x1.glbdom.ub - x1.glbdom.lb)).abs();
                let abscont2 = (c.vals[ind2] * (x2.glbdom.ub - x2.glbdom.lb)).abs();

                if eps_gt(abscont1, abscont2, 1e-9) {
                    -1
                } else if eps_gt(abscont2, abscont1, 1e-9) {
                    1
                } else {
                    probindex(var1) - probindex(var2)
                }
            }
        }
    }
}

/// `permSortConsdata`: permutes the constraint's variables according to a
/// given permutation (the event data positions with them).
fn perm_sort_consdata(scip: &mut Scip, eventhdlr: EventhdlrId, c: &mut ConsData, perm: &mut [i32], nvars: i32) {
    // Permute the variables in the linear constraint according to the target
    // permutation.
    let mut eventdatav = NONE;
    for v in 0..nvars {
        if perm[v as usize] != v {
            let varv = c.vars[v];
            let valv = c.vals[v];
            if let Some(e) = c.eventdata.as_ref() {
                eventdatav = e[v];
            }
            let mut i = v;
            loop {
                let pi = perm[i as usize];
                c.vars[i] = c.vars[pi];
                c.vals[i] = c.vals[pi];
                if let Some(e) = c.eventdata.as_mut() {
                    e[i] = e[pi];
                    eds(scip, eventhdlr)[e[i]].varpos = i;
                }
                std::mem::swap(&mut perm[i as usize], &mut i);
                if perm[i as usize] == v {
                    break;
                }
            }
            c.vars[i] = varv;
            c.vals[i] = valv;
            if let Some(e) = c.eventdata.as_mut() {
                e[i] = eventdatav;
                eds(scip, eventhdlr)[e[i]].varpos = i;
            }
            perm[i as usize] = i;
        }
    }
}

/// `consdataSort`: sorts linear constraint's variables depending on the
/// stage of the solving process:
/// - during PRESOLVING sorts variables by binary, integer, implied
///   integral, and continuous variables, and the variables of the same type
///   by non-decreasing variable index
/// - during SOLVING sorts variables of the remaining problem by binary,
///   integer, implied integral, and continuous variables, and binary and
///   integer variables by their global max activity delta (within each
///   group), ties within a group are broken by problem index of the
///   variable.
fn consdata_sort(scip: &mut Scip, cons: ConsId) {
    let eventhdlr = hd(scip, scip.conss[cons].conshdlr).eventhdlr;
    with_cd(scip, cons, |scip, c| {
        let presolving = scip.set.stage < Stage::InitSolve;
        // Check if there are variables for sorting.
        if c.nvars <= 1 {
            c.indexsorted = true;
            c.coefsorted = true;
            c.nbinvars = if c.nvars == 1 { var_is_binary(scip, c.vars[0]) as i32 } else { 0 };
        } else if (!c.indexsorted && presolving) || (!c.coefsorted && !presolving) {
            let mut perm: Vec<i32> = (0..c.nvars).collect();

            // Call sorting method.
            if presolving {
                sort::sort(&mut perm, (), |&a, &b| consdata_comp_var(scip, c, a as usize, b as usize));
            } else {
                sort::sort(&mut perm, (), |&a, &b| consdata_comp_var_prop(scip, c, a as usize, b as usize));
            }

            let nvars = c.nvars;
            perm_sort_consdata(scip, eventhdlr, c, &mut perm, nvars);

            if !presolving {
                c.indexsorted = false;
                c.coefsorted = true;

                // Count binary variables in the sorted vars array.
                c.nbinvars = 0;
                for v in 0..c.nvars {
                    if var_is_binary(scip, c.vars[v]) {
                        c.nbinvars += 1;
                    } else {
                        break;
                    }
                }
            } else {
                c.indexsorted = true;
                c.coefsorted = false;
            }
        }
    })
}

/*
 * local linear constraint handler methods
 */

/// Whether the constraint holds locks of any type.
fn cons_is_locked_any(scip: &Scip, cons: ConsId) -> bool {
    let mut locked = false;
    let mut i = 0;
    while i < NLOCKTYPES && !locked {
        locked = api::cons_is_locked_type(scip, cons, i);
        i += 1;
    }
    locked
}

/// `chgLhs`: sets left hand side of linear constraint.
fn chg_lhs(scip: &mut Scip, cons: ConsId, mut lhs: f64) -> ScipResult {
    // Adjust value to be not beyond infinity.
    if scip.set.is_infinity(-lhs) {
        lhs = -scip.set.infinity();
    } else if scip.set.is_infinity(lhs) {
        lhs = scip.set.infinity();
    }

    // Check whether the side is not changed.
    if scip.set.is_eq(cd(scip, cons).lhs, lhs) {
        return Ok(());
    }

    // Ensure that rhs >= lhs is satisfied without numerical tolerance.
    if scip.set.is_eq(lhs, cd(scip, cons).rhs) {
        cdm(scip, cons).rhs = lhs;
    }

    let locked = cons_is_locked_any(scip, cons);

    // If necessary, update the rounding locks of variables.
    if locked {
        let oldlhs = cd(scip, cons).lhs;
        if scip.set.is_infinity(-oldlhs) && !scip.set.is_infinity(-lhs) {
            // The left hand side switched from -infinity to a non-infinite value
            // -> install rounding locks.
            for v in 0..cd(scip, cons).nvars {
                let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                if scip.set.is_positive(val) {
                    api::lock_var_cons(scip, var, cons, true, false)?;
                } else {
                    api::lock_var_cons(scip, var, cons, false, true)?;
                }
            }
        } else if !scip.set.is_infinity(-oldlhs) && scip.set.is_infinity(-lhs) {
            // The left hand side switched from a non-infinite value to -infinity
            // -> remove rounding locks.
            for v in 0..cd(scip, cons).nvars {
                let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                if scip.set.is_positive(val) {
                    api::unlock_var_cons(scip, var, cons, true, false)?;
                } else {
                    api::unlock_var_cons(scip, var, cons, false, true)?;
                }
            }
        }
    }

    // Check whether the left hand side is increased, if and only if that's the
    // case we maybe can propagate, tighten and add more cliques.
    if !scip.set.is_infinity(lhs.abs()) && scip.set.is_gt(lhs, cd(scip, cons).lhs) {
        let c = cdm(scip, cons);
        c.boundstightened = 0;
        c.presolved = false;
        c.cliquesadded = false;
        c.implsadded = false;

        // Mark the constraint for propagation.
        if api::cons_is_transformed(scip, cons) {
            cons_mark_propagate(scip, cons);
        }
    }

    // Set new left hand side and update constraint data.
    let c = cdm(scip, cons);
    c.lhs = lhs;
    c.changed = true;
    c.normalized = false;
    c.upgradetried = false;
    c.rangedrowpropagated = 0;

    // Update the lhs of the LP row.
    let row = c.row;
    if row != NONE {
        crate::lp::row_chg_lhs(scip, row, lhs)?;
    }

    Ok(())
}

/// `chgRhs`: sets right hand side of linear constraint.
fn chg_rhs(scip: &mut Scip, cons: ConsId, mut rhs: f64) -> ScipResult {
    // Adjust value to be not beyond infinity.
    if scip.set.is_infinity(rhs) {
        rhs = scip.set.infinity();
    } else if scip.set.is_infinity(-rhs) {
        rhs = -scip.set.infinity();
    }

    // Check whether the side is not changed.
    if scip.set.is_eq(cd(scip, cons).rhs, rhs) {
        return Ok(());
    }

    // Ensure that rhs >= lhs is satisfied without numerical tolerance.
    if scip.set.is_eq(rhs, cd(scip, cons).lhs) {
        cdm(scip, cons).lhs = rhs;
    }

    let locked = cons_is_locked_any(scip, cons);

    // If necessary, update the rounding locks of variables.
    if locked {
        let oldrhs = cd(scip, cons).rhs;
        if scip.set.is_infinity(oldrhs) && !scip.set.is_infinity(rhs) {
            // The right hand side switched from infinity to a non-infinite value
            // -> install rounding locks.
            for v in 0..cd(scip, cons).nvars {
                let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                if scip.set.is_positive(val) {
                    api::lock_var_cons(scip, var, cons, false, true)?;
                } else {
                    api::lock_var_cons(scip, var, cons, true, false)?;
                }
            }
        } else if !scip.set.is_infinity(oldrhs) && scip.set.is_infinity(rhs) {
            // The right hand side switched from a non-infinite value to infinity
            // -> remove rounding locks.
            for v in 0..cd(scip, cons).nvars {
                let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                if scip.set.is_positive(val) {
                    api::unlock_var_cons(scip, var, cons, false, true)?;
                } else {
                    api::unlock_var_cons(scip, var, cons, true, false)?;
                }
            }
        }
    }

    // Check whether the right hand side is decreased, if and only if that's the
    // case we maybe can propagate, tighten and add more cliques.
    if !scip.set.is_infinity(rhs.abs()) && scip.set.is_lt(rhs, cd(scip, cons).rhs) {
        let c = cdm(scip, cons);
        c.boundstightened = 0;
        c.presolved = false;
        c.cliquesadded = false;
        c.implsadded = false;

        // Mark the constraint for propagation.
        if api::cons_is_transformed(scip, cons) {
            cons_mark_propagate(scip, cons);
        }
    }

    // Set new right hand side and update constraint data.
    let c = cdm(scip, cons);
    c.rhs = rhs;
    c.changed = true;
    c.normalized = false;
    c.upgradetried = false;
    c.rangedrowpropagated = 0;

    // Update the rhs of the LP row.
    let row = c.row;
    if row != NONE {
        crate::lp::row_chg_rhs(scip, row, rhs)?;
    }

    Ok(())
}

/// `addCoef`: adds coefficient in linear constraint.
fn add_coef(scip: &mut Scip, cons: ConsId, mut var: VarId, val: f64) -> ScipResult {
    // Are we in the transformed problem?
    let transformed = api::cons_is_transformed(scip, cons);

    // Always use transformed variables in transformed constraints.
    if transformed {
        var = api::get_transformed_var(scip, var)?;
    }

    let n = cd(scip, cons).nvars;
    consdata_ensure_vars_size(scip, cons, n + 1);
    let c = cdm(scip, cons);
    c.vars[n] = var;
    c.vals[n] = val;
    c.nvars += 1;

    // Capture variable.
    var_capture(scip, var);

    // If we are in transformed problem, the variable needs an additional
    // event data.
    if transformed {
        if cd(scip, cons).eventdata.is_some() {
            // Check for event handler.
            let eventhdlr = hd(scip, scip.conss[cons].conshdlr).eventhdlr;

            // Initialize eventdata array.
            let c = cdm(scip, cons);
            let last = c.nvars - 1;
            c.eventdata.as_mut().unwrap()[last] = NONE;

            // Catch bound change events of variable.
            cons_catch_event(scip, cons, eventhdlr, last)?;
        }

        // Update minimum and maximum activities.
        if !scip.set.is_zero(val) {
            with_cd(scip, cons, |scip, c| consdata_update_add_coef(scip, c, var, val, false));
        }
    }

    // Install rounding locks for new variable with non-zero coefficient.
    if !scip.set.is_zero(val) {
        lock_rounding(scip, cons, var, val)?;
    }

    // Mark the constraint for propagation.
    if transformed {
        cons_mark_propagate(scip, cons);
    }

    let active = crate::var::var_is_active(scip, var);
    with_cd(scip, cons, |scip, c| {
        c.boundstightened = 0;
        c.presolved = false;
        c.removedfixings = c.removedfixings && active;

        if c.validsignature {
            consdata_update_signatures(scip, c, c.nvars as usize - 1);
        }

        c.changed = true;
        c.normalized = false;
        c.upgradetried = false;
        c.cliquesadded = false;
        c.implsadded = false;
        c.rangedrowpropagated = 0;
        c.merged = false;

        if c.nvars == 1 {
            c.indexsorted = true;
            c.coefsorted = true;
        } else if scip.set.stage < Stage::InitSolve {
            c.indexsorted = c.indexsorted && consdata_comp_var(scip, c, c.nvars as usize - 2, c.nvars as usize - 1) <= 0;
            c.coefsorted = false;
        } else {
            c.indexsorted = false;
            c.coefsorted = c.coefsorted && consdata_comp_var_prop(scip, c, c.nvars as usize - 2, c.nvars as usize - 1) <= 0;
        }

        // Update hascontvar and hasnonbinvar flags.
        if c.hasnonbinvalid && !c.hascontvar && !var_is_binary(scip, var) {
            c.hasnonbinvar = true;
            if !var_is_integral(scip, var) {
                c.hascontvar = true;
            }
        }
    });

    // Add the new coefficient to the LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        api::add_var_to_row(scip, row, var, val)?;
    }

    Ok(())
}

/// `delCoefPos`: deletes coefficient at given position from linear
/// constraint data.
fn del_coef_pos(scip: &mut Scip, cons: ConsId, pos: i32) -> ScipResult {
    let mut var = cd(scip, cons).vars[pos];
    let val = cd(scip, cons).vals[pos];

    // Remove rounding locks for deleted variable with non-zero coefficient.
    if !scip.set.is_zero(val) {
        unlock_rounding(scip, cons, var, val)?;
    }

    let eventhdlr = hd(scip, scip.conss[cons].conshdlr).eventhdlr;

    // If we are in transformed problem, delete the event data of the variable.
    // Drop bound change events of variable.
    if api::cons_is_transformed(scip, cons) && cd(scip, cons).eventdata.is_some() {
        cons_drop_event(scip, cons, eventhdlr, pos)?;
    }

    with_cd(scip, cons, |scip, c| {
        // Move the last variable to the free slot.
        if pos != c.nvars - 1 {
            let last = c.nvars - 1;
            c.vars[pos] = c.vars[last];
            c.vals[pos] = c.vals[last];

            if let Some(e) = c.eventdata.as_mut() {
                e[pos] = e[last];
                eds(scip, eventhdlr)[e[pos]].varpos = pos;
            }

            c.indexsorted = c.indexsorted && pos + 2 >= c.nvars;
            c.coefsorted = c.coefsorted && pos + 2 >= c.nvars;
        }
        c.nvars -= 1;
    });

    // If at most one variable is left, the activities should be recalculated
    // (to correspond exactly to the bounds of the remaining variable, or give
    // exactly 0.0).
    let transformed = api::cons_is_transformed(scip, cons);
    if cd(scip, cons).nvars <= 1 {
        consdata_invalidate_activities(cdm(scip, cons));
    } else if transformed && !scip.set.is_zero(val) {
        // If we are in transformed problem, update minimum and maximum
        // activities.
        with_cd(scip, cons, |scip, c| consdata_update_del_coef(scip, c, var, val, true));
    }

    // Mark the constraint for propagation.
    if transformed {
        cons_mark_propagate(scip, cons);
    }

    let isbinary = var_is_binary(scip, var);
    let c = cdm(scip, cons);
    c.boundstightened = 0;
    c.presolved = false;
    c.validsignature = false;
    c.changed = true;
    c.normalized = false;
    c.upgradetried = false;
    c.cliquesadded = false;
    c.implsadded = false;
    c.rangedrowpropagated = 0;

    // Check if hasnonbinvar flag might be incorrect now.
    if c.hasnonbinvar && !isbinary {
        c.hasnonbinvalid = false;
    }

    // Delete coefficient from the LP row.
    let row = c.row;
    if row != NONE {
        api::add_var_to_row(scip, row, var, -val)?;
    }

    // Release variable.
    api::release_var(scip, &mut var)?;

    Ok(())
}

/// `chgCoefPos`: changes coefficient value at given position of linear
/// constraint data.
fn chg_coef_pos(scip: &mut Scip, cons: ConsId, pos: i32, newval: f64) -> ScipResult {
    let var = cd(scip, cons).vars[pos];
    let val = cd(scip, cons).vals[pos];

    let locked = cons_is_locked_any(scip, cons);

    // If necessary, update the rounding locks of the variable.
    if locked && (!scip.set.is_negative(val) || !scip.set.is_negative(newval)) && (!scip.set.is_positive(val) || !scip.set.is_positive(newval)) {
        // Remove rounding locks for variable with old non-zero coefficient.
        if !scip.set.is_zero(val) {
            unlock_rounding(scip, cons, var, val)?;
        }

        // Install rounding locks for variable with new non-zero coefficient.
        if !scip.set.is_zero(newval) {
            lock_rounding(scip, cons, var, newval)?;
        }
    }

    let transformed = api::cons_is_transformed(scip, cons);
    with_cd(scip, cons, |scip, c| {
        // Change the value.
        c.vals[pos] = newval;

        if c.coefsorted {
            if pos > 0 {
                c.coefsorted = consdata_comp_var_prop(scip, c, pos as usize - 1, pos as usize) <= 0;
            }
            if c.coefsorted && pos < c.nvars - 1 {
                c.coefsorted = consdata_comp_var_prop(scip, c, pos as usize, pos as usize + 1) <= 0;
            }
        }

        // Update minimum and maximum activities.
        if transformed {
            if scip.set.is_zero(val) {
                consdata_update_add_coef(scip, c, var, newval, true);
            } else if scip.set.is_zero(newval) {
                consdata_update_del_coef(scip, c, var, val, true);
            } else {
                consdata_update_chg_coef(scip, c, var, val, newval, true);
            }
        }
    });

    // Mark the constraint for propagation.
    if transformed {
        cons_mark_propagate(scip, cons);
    }

    let c = cdm(scip, cons);
    c.boundstightened = 0;
    c.presolved = false;
    c.validsignature = c.validsignature && newval * val > 0.0;
    c.changed = true;
    c.normalized = false;
    c.upgradetried = false;
    c.cliquesadded = false;
    c.implsadded = false;
    c.rangedrowpropagated = 0;

    Ok(())
}

/// `scaleCons`: scales a linear constraint with a constant scalar.
fn scale_cons(scip: &mut Scip, cons: ConsId, scalar: f64) -> ScipResult {
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    if (!scip.set.is_infinity(-lhs) && scip.set.is_infinity(-lhs * scalar)) || (!scip.set.is_infinity(rhs) && scip.set.is_infinity(rhs * scalar)) {
        // C warns: skipped scaling to avoid numerical troubles.
        return Ok(());
    }

    // Scale the coefficients.
    let mut i = cd(scip, cons).nvars - 1;
    while i >= 0 {
        let oldval = cd(scip, cons).vals[i];
        let mut newval = scalar * oldval;

        // Because SCIPisScalingIntegral uses another integrality check as
        // SCIPfeasFloor, we add an additional 0.5 before flooring down our new
        // value.
        if scip.set.is_scaling_integral(oldval, scalar) {
            newval = scip.set.feas_floor(newval + 0.5);
        }

        if scip.set.is_zero(newval) {
            // C warns: coefficient scaled to zero.
            del_coef_pos(scip, cons, i)?;
        } else {
            cdm(scip, cons).vals[i] = newval;
        }
        i -= 1;
    }

    let set = &scip.set;
    let c = scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap();

    // Scale the sides.
    if scalar < 0.0 {
        let lhs = c.lhs;
        c.lhs = -c.rhs;
        c.rhs = -lhs;
    }
    let absscalar = scalar.abs();
    if !set.is_infinity(-c.lhs) {
        let newval = absscalar * c.lhs;

        // Because SCIPisScalingIntegral uses another integrality check as
        // SCIPfeasFloor, we add an additional 0.5 before flooring down our new
        // value.
        if set.is_scaling_integral(c.lhs, absscalar) {
            c.lhs = set.feas_floor(newval + 0.5);
        } else {
            c.lhs = newval;
        }
    }
    if !set.is_infinity(c.rhs) {
        let newval = absscalar * c.rhs;

        // Because SCIPisScalingIntegral uses another integrality check as
        // SCIPfeasCeil, we subtract 0.5 before ceiling up our new value.
        if set.is_scaling_integral(c.rhs, absscalar) {
            c.rhs = set.feas_ceil(newval - 0.5);
        } else {
            c.rhs = newval;
        }
    }

    consdata_invalidate_activities(c);
    c.cliquesadded = false;
    c.implsadded = false;

    Ok(())
}

/// `performVarDeletions`: perform deletion of variables in all constraints
/// of the constraint handler.
fn perform_var_deletions(scip: &mut Scip, conss: &[ConsId]) -> ScipResult {
    // Iterate over all constraints.
    for &cons in conss {
        // Constraint is marked, that some of its variables were deleted.
        if cd(scip, cons).varsdeleted {
            // Iterate over all variables of the constraint and delete them from
            // the constraint.
            let mut v = cd(scip, cons).nvars - 1;
            while v >= 0 {
                if scip.vars[cd(scip, cons).vars[v]].deleted {
                    del_coef_pos(scip, cons, v)?;
                }
                v -= 1;
            }
            cdm(scip, cons).varsdeleted = false;
        }
    }
    Ok(())
}

/// `normalizeCons`: normalizes a linear constraint with the following rules:
/// - if all coefficients have them same absolute value, change them to
///   (-)1.0
/// - multiplication with +1 or -1: apply the following rules in the given
///   order, until the sign of the factor is determined (later rules only
///   apply, if the current rule doesn't determine the sign):
///     1. the right hand side must not be negative
///     2. the right hand side must not be infinite
///     3. the absolute value of the right hand side must be greater than
///        that of the left hand side
///     4. the number of positive coefficients must not be smaller than the
///        number of negative coefficients
///     5. multiply with +1
/// - rationals to integrals: try to identify a rational representation of
///   the fractional coefficients, and multiply all coefficients by the
///   smallest common multiple of all denominators to get integral
///   coefficients; forbid large denominators due to numerical stability
/// - division by greatest common divisor: if all coefficients are integral,
///   divide them by the greatest common divisor.
///
/// Returns whether infeasibility was detected.
fn normalize_cons(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    // We must not change a modifiable constraint in any way.
    if scip.conss[cons].modifiable {
        return Ok(false);
    }

    // Check, if the constraint is already normalized.
    if cd(scip, cons).normalized {
        return Ok(false);
    }

    if cd(scip, cons).nvars == 0 {
        cdm(scip, cons).normalized = true;
        return Ok(false);
    }

    // Get maximum and minimum absolute coefficient.
    let maxabsval = consdata_get_max_absval(cdm(scip, cons));
    let minabsval = consdata_get_min_absval(cdm(scip, cons));

    // Return if scaling by maxval will eliminate coefficients.
    if scip.set.is_zero(minabsval / maxabsval) {
        return Ok(false);
    }

    // Return if scaling by maxval will eliminate or generate non-zero sides.
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    if !scip.set.is_infinity(lhs) && scip.set.is_feas_zero(lhs) != scip.set.is_feas_zero(lhs / maxabsval) {
        return Ok(false);
    }
    if !scip.set.is_infinity(rhs) && scip.set.is_feas_zero(rhs) != scip.set.is_feas_zero(rhs / maxabsval) {
        return Ok(false);
    }

    // Check if not all absolute coefficients are near 1.0 but scaling could do.
    if scip.set.is_lt(minabsval, 1.0) != scip.set.is_gt(maxabsval, 1.0) {
        // Calculate scale of the average minimum and maximum absolute
        // coefficient to 1.0.
        let scalar = 2.0 / (minabsval + maxabsval);

        // Check if all scaled absolute coefficients are near 1.0; we can relax
        // EQ(x,1.0) to LE(x,1.0), as LT(x,1.0) is not possible.
        if scip.set.is_le(scalar * maxabsval, 1.0) {
            scale_cons(scip, cons, scalar)?;
        }
    }

    // nvars might have changed.
    if cd(scip, cons).nvars == 0 {
        cdm(scip, cons).normalized = true;
        return Ok(false);
    }

    // Calculate the maximal multiplier for common divisor calculation:
    //  |p/q - val| < epsilon  and  q < feastol/epsilon  =>  |p - q*val| < feastol
    // which means, a value of feastol/epsilon should be used as maximal
    // multiplier; additionally, we don't want to scale the constraint if this
    // would lead to too large coefficients.
    // Slightly decrease epsilon to be safe in rational conversion below.
    let epsilon = scip.set.epsilon() * 0.9;
    let feastol = scip.set.feastol();
    let mut maxmult = (feastol / epsilon + feastol) as i64;

    if !cd(scip, cons).hasnonbinvalid {
        with_cd(scip, cons, |scip, c| consdata_check_nonbinvar(scip, c));
    }

    // Get maximum absolute coefficient.
    let maxabsval = consdata_get_max_absval(cdm(scip, cons));

    // If all variables are of integral type we will allow a greater
    // multiplier.
    if !cd(scip, cons).hascontvar {
        maxmult = crate::def::min(maxmult, (MAXSCALEDCOEFINTEGER / crate::def::max(maxabsval, 1.0)) as i64);
    } else {
        maxmult = crate::def::min(maxmult, (MAXSCALEDCOEF / crate::def::max(maxabsval, 1.0)) as i64);
    }

    //
    // multiplication with +1 or -1
    //
    let mut mult = 0;
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

    // 1. the right hand side must not be negative
    if scip.set.is_positive(lhs) {
        mult = 1;
    } else if scip.set.is_negative(rhs) {
        mult = -1;
    }

    if mult == 0 {
        // 2. the right hand side must not be infinite
        if scip.set.is_infinity(-lhs) {
            mult = 1;
        } else if scip.set.is_infinity(rhs) {
            mult = -1;
        }
    }

    if mult == 0 {
        // 3. the absolute value of the right hand side must be greater than
        // that of the left hand side
        if scip.set.is_gt(rhs.abs(), lhs.abs()) {
            mult = 1;
        } else if scip.set.is_lt(rhs.abs(), lhs.abs()) {
            mult = -1;
        }
    }

    if mult == 0 {
        // 4. the number of positive coefficients must not be smaller than the
        // number of negative coefficients
        let c = cd(scip, cons);
        let mut nposcoeffs = 0;
        let mut nnegcoeffs = 0;
        for i in 0..c.nvars {
            if c.vals[i] > 0.0 {
                nposcoeffs += 1;
            } else {
                nnegcoeffs += 1;
            }
        }
        if nposcoeffs > nnegcoeffs {
            mult = 1;
        } else if nposcoeffs < nnegcoeffs {
            mult = -1;
        }
    }

    if mult == 0 {
        // 5. multiply with +1
        mult = 1;
    }

    if mult == -1 {
        // Scale the constraint with -1; scaleCons() can delete variables, but
        // scaling with -1 should not do that.
        scale_cons(scip, cons, -1.0)?;
    }

    //
    // rationals to integrals
    //
    let mut success = true;
    let mut scm: i64 = 1;
    {
        let c = cd(scip, cons);
        let mut i = 0;
        while i < c.nvars && success && scm <= maxmult {
            if !scip.set.is_integral(c.vals[i]) {
                let mut numerator = 0;
                let mut denominator = 0;
                // Epsilon has been slightly decreased above - to be on the safe
                // side.
                success = crate::misc::real_to_rational(c.vals[i], -epsilon, epsilon, maxmult, &mut numerator, &mut denominator);
                if success {
                    scm = crate::misc::calc_sma_com_mul(scm, denominator);
                }
            }
            i += 1;
        }
    }

    // It might be that we have really big coefficients, but all are integral,
    // in that case we want to divide them by their greatest common divisor.
    let mut onlyintegral = true;
    if scm == 1 {
        let c = cd(scip, cons);
        let mut i = c.nvars - 1;
        while i >= 0 {
            if !scip.set.is_integral(c.vals[i]) {
                onlyintegral = false;
                break;
            }
            i -= 1;
        }
    }

    success = success && (scm <= maxmult || (scm == 1 && onlyintegral));
    if success && scm != 1 {
        // Scale the constraint with the smallest common multiple of all
        // denominators.
        scale_cons(scip, cons, scm as f64)?;

        let set = &scip.set;
        let c = scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap();
        if c.validmaxabsval {
            c.maxabsval *= (scm as f64).abs();
            if !set.is_integral(c.maxabsval) {
                c.validmaxabsval = false;
                c.maxabsval = INVALID;
                consdata_calc_max_absval(c);
            }
        }

        if c.validminabsval {
            c.minabsval *= (scm as f64).abs();
            if !set.is_integral(c.minabsval) {
                c.validminabsval = false;
                c.minabsval = INVALID;
                consdata_calc_min_absval(c);
            }
        }
    }

    //
    // division by greatest common divisor
    //
    if success && cd(scip, cons).nvars >= 1 {
        // All coefficients are integral: divide them by their greatest common
        // divisor.
        let c = cd(scip, cons);
        let mut gcd = (c.vals[0usize].abs() + feastol) as i64;
        let mut i = 1;
        while i < c.nvars && gcd > 1 {
            gcd = crate::misc::calc_gre_com_div(gcd, (c.vals[i].abs() + feastol) as i64);
            i += 1;
        }

        if gcd > 1 {
            // Since the lhs/rhs is not respected for gcd calculation it can
            // happen that we detect infeasibility.
            if !c.hascontvar && onlyintegral && scip.set.is_eq(c.lhs, c.rhs) && !scip.set.is_feas_integral(c.rhs / gcd as f64) {
                return Ok(true);
            }

            // Divide the constraint by the greatest common divisor of the
            // coefficients.
            scale_cons(scip, cons, 1.0 / gcd as f64)?;

            let c = cdm(scip, cons);
            if c.validmaxabsval {
                c.maxabsval /= (gcd as f64).abs();
            }
            if c.validminabsval {
                c.minabsval /= (gcd as f64).abs();
            }
        }
    }

    // Mark constraint to be normalized.
    cdm(scip, cons).normalized = true;

    Ok(false)
}

/// `mergeMultiples`: replaces multiple occurrences of a variable by a single
/// non-zero coefficient.
fn merge_multiples(scip: &mut Scip, cons: ConsId) -> ScipResult {
    if cd(scip, cons).merged {
        return Ok(());
    }

    // Sort the constraint.
    consdata_sort(scip, cons);

    // Go backwards through the constraint looking for multiple occurrences of
    // the same variable; backward direction is necessary, since delCoefPos()
    // modifies the given position and the subsequent ones.
    let mut v = cd(scip, cons).nvars - 1;
    while v >= 0 {
        let var = cd(scip, cons).vars[v];
        let mut valsum = cd(scip, cons).vals[v];

        // Sum multiple occurrences.
        while v >= 1 && cd(scip, cons).vars[v - 1] == var {
            del_coef_pos(scip, cons, v)?;
            v -= 1;
            valsum += cd(scip, cons).vals[v];
        }

        // Modify the last existing occurrence of the variable.
        if scip.set.is_zero(valsum) {
            del_coef_pos(scip, cons, v)?;
        } else if valsum != cd(scip, cons).vals[v] {
            chg_coef_pos(scip, cons, v, valsum)?;
        }
        v -= 1;
    }

    cdm(scip, cons).merged = true;

    Ok(())
}

/// `applyFixings`: replaces all fixed and aggregated variables by their
/// non-fixed counterparts. With `want_infeasible` (C's `infeasible !=
/// NULL`) stops after the first infeasibility and returns it; otherwise
/// applies all fixings.
fn apply_fixings(scip: &mut Scip, cons: ConsId, want_infeasible: bool) -> ScipResult<bool> {
    if cd(scip, cons).eventdata.is_none() {
        let eventhdlr = hd(scip, scip.conss[cons].conshdlr).eventhdlr;

        // Catch bound change events of variables.
        cons_catch_all_events(scip, cons, eventhdlr)?;
    }

    if !cd(scip, cons).removedfixings {
        // If an unmodifiable row has been added to the LP, then we cannot apply
        // fixing anymore (cannot change a row); this should not happen, as
        // applyFixings is called in addRelaxation() before creating and adding
        // a row.
        let mut lhssubtrahend = 0.0;
        let mut rhssubtrahend = 0.0;

        let mut v = 0;
        while v < cd(scip, cons).nvars {
            let mut var = cd(scip, cons).vars[v];
            let mut scalar = cd(scip, cons).vals[v];
            let mut constant = 0.0;
            crate::var::var_get_probvar_sum(scip, &mut var, &mut scalar, &mut constant)?;

            let status = scip.vars[var].varstatus;
            let mut fixed = false;
            match status {
                VarStatus::Original => {
                    // Original variable in transformed linear constraint.
                    return Err(Retcode::InvalidData);
                }
                VarStatus::Aggregated | VarStatus::Negated => {
                    // Aggregated variable after resolving linear term.
                    return Err(Retcode::InvalidData);
                }
                VarStatus::Loose | VarStatus::Column => {
                    if var != cd(scip, cons).vars[v] {
                        add_coef(scip, cons, var, scalar)?;
                        del_coef_pos(scip, cons, v)?;

                        if !scip.set.is_infinity(-cd(scip, cons).lhs) {
                            lhssubtrahend += constant;
                        }
                        if !scip.set.is_infinity(cd(scip, cons).rhs) {
                            rhssubtrahend += constant;
                        }
                    }
                    v += 1;
                }
                VarStatus::MultAggr => {
                    if scalar != 0.0 {
                        crate::var::var_flatten_aggregation_graph(scip, var)?;
                        let m = scip.vars[var].data.multaggr.clone();
                        for i in 0..m.nvars {
                            add_coef(scip, cons, m.vars[i], scalar * m.scalars[i])?;
                        }
                        constant += scalar * m.constant;
                    }
                    fixed = true;
                }
                VarStatus::Fixed => fixed = true,
            }

            if fixed {
                if !scip.set.is_infinity(-cd(scip, cons).lhs) {
                    if scip.set.is_infinity(constant.abs()) {
                        // If lhs gets infinity it means that the problem is
                        // infeasible.
                        if constant < 0.0 {
                            chg_lhs(scip, cons, scip.set.infinity())?;

                            if want_infeasible {
                                return Ok(true);
                            }
                        } else {
                            chg_lhs(scip, cons, -scip.set.infinity())?;
                        }
                    } else {
                        lhssubtrahend += constant;
                    }
                }
                if !scip.set.is_infinity(cd(scip, cons).rhs) {
                    if scip.set.is_infinity(constant.abs()) {
                        // If rhs gets -infinity it means that the problem is
                        // infeasible.
                        if constant > 0.0 {
                            chg_rhs(scip, cons, -scip.set.infinity())?;

                            if want_infeasible {
                                return Ok(true);
                            }
                        } else {
                            chg_rhs(scip, cons, scip.set.infinity())?;
                        }
                    } else {
                        rhssubtrahend += constant;
                    }
                }
                del_coef_pos(scip, cons, v)?;
            }
        }

        let modifiable = scip.conss[cons].modifiable;
        let lhs = cd(scip, cons).lhs;
        if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(lhs) {
            // Check left hand side of unmodifiable empty constraint with former
            // feasibility tolerance.
            if !modifiable && cd(scip, cons).nvars == 0 {
                if scip.set.is_feas_lt(lhssubtrahend, lhs) {
                    chg_lhs(scip, cons, scip.set.infinity())?;

                    if want_infeasible {
                        return Ok(true);
                    }
                } else {
                    chg_lhs(scip, cons, -scip.set.infinity())?;
                }
            }
            // For normal numbers that are relatively equal, subtraction can lead
            // to cancellation, causing wrong fixings of other variables -->
            // better use a real zero here.
            else if scip.set.is_ge(lhs.abs(), 1.0) && scip.set.is_eq(lhssubtrahend, lhs) {
                chg_lhs(scip, cons, 0.0)?;
            } else {
                chg_lhs(scip, cons, lhs - lhssubtrahend)?;
            }
        }
        let rhs = cd(scip, cons).rhs;
        if !scip.set.is_infinity(rhs) && !scip.set.is_infinity(-rhs) {
            // Check right hand side of unmodifiable empty constraint with former
            // feasibility tolerance.
            if !modifiable && cd(scip, cons).nvars == 0 {
                if scip.set.is_feas_gt(rhssubtrahend, rhs) {
                    chg_rhs(scip, cons, -scip.set.infinity())?;

                    if want_infeasible {
                        return Ok(true);
                    }
                } else {
                    chg_rhs(scip, cons, scip.set.infinity())?;
                }
            }
            // For normal numbers that are relatively equal, subtraction can lead
            // to cancellation, causing wrong fixings of other variables -->
            // better use a real zero here.
            else if scip.set.is_ge(rhs.abs(), 1.0) && scip.set.is_eq(rhssubtrahend, rhs) {
                chg_rhs(scip, cons, 0.0)?;
            } else {
                chg_rhs(scip, cons, rhs - rhssubtrahend)?;
            }
        }
        cdm(scip, cons).removedfixings = true;

        // If aggregated variables have been replaced, multiple entries of the
        // same variable are possible and we have to clean up the constraint.
        merge_multiples(scip, cons)?;
    }

    Ok(false)
}

/// `addConflictBounds`: for each variable in the linear constraint, except
/// the inferred variable, adds one bound to the conflict analysis'
/// candidate store (bound depends on sign of coefficient and whether the
/// left or right hand side was the reason for the inference variable's
/// bound change); the conflict analysis can be initialized with the linear
/// constraint being the conflict detecting constraint by using `NONE` as
/// inferred variable.
fn add_conflict_bounds(scip: &mut Scip, cons: ConsId, infervar: VarId, bdchgidx: Option<&BdChgIdx>, inferpos: i32, reasonisrhs: bool) -> ScipResult {
    let nvars = cd(scip, cons).nvars;

    // For each variable, add the bound to the conflict queue, that is
    // responsible for the minimal or maximal residual value, depending on
    // whether the left or right hand side is responsible for the bound change:
    //  - if the right hand side is the reason, the minimal residual activity
    //    is responsible
    //  - if the left hand side is the reason, the maximal residual activity
    //    is responsible

    // If the variable is integral we only need to add reason bounds until the
    // propagation could be applied.
    if infervar == NONE || var_is_integral(scip, infervar) {
        let mut minresactivity = -scip.set.infinity();
        let mut maxresactivity = scip.set.infinity();
        let ismintight;
        let ismaxtight;
        let isminsettoinfinity;
        let ismaxsettoinfinity;

        // Calculate the minimal and maximal global activity of all other
        // variables involved in the constraint.
        let (min, max) = with_cd(scip, cons, |scip, c| {
            if infervar != NONE {
                let val = c.vals[inferpos];
                consdata_get_glb_activity_residuals(scip, c, infervar, val, false, reasonisrhs, !reasonisrhs)
            } else {
                consdata_get_glb_activity_bounds(scip, c, false, reasonisrhs, !reasonisrhs)
            }
        });
        if reasonisrhs {
            minresactivity = min.act;
            ismintight = min.istight;
            isminsettoinfinity = min.issettoinfinity;
            (ismaxtight, ismaxsettoinfinity) = (false, false);
        } else {
            maxresactivity = max.act;
            ismaxtight = max.istight;
            ismaxsettoinfinity = max.issettoinfinity;
            (ismintight, isminsettoinfinity) = (false, false);
        }

        // We can only do something clever, if the residual activity is finite
        // and not relaxed.
        if (reasonisrhs && !isminsettoinfinity && ismintight) || (!reasonisrhs && !ismaxsettoinfinity && ismaxtight) {
            let mut resactisinf = false;
            let mut rescap;

            // Calculate the residual capacity that would be left, if the
            // variable would be set to one more / one less than its inferred
            // bound.
            if infervar != NONE {
                let c = cd(scip, cons);
                if reasonisrhs {
                    if scip.set.is_update_unreliable(minresactivity, c.lastglbminactivity) {
                        minresactivity = consdata_get_reliable_residual_activity(scip, c, infervar, true, true);
                        if scip.set.is_infinity(-minresactivity) {
                            resactisinf = true;
                        }
                    }
                    rescap = c.rhs - minresactivity;
                } else {
                    if scip.set.is_update_unreliable(maxresactivity, c.lastglbmaxactivity) {
                        maxresactivity = consdata_get_reliable_residual_activity(scip, c, infervar, false, true);
                        if scip.set.is_infinity(maxresactivity) {
                            resactisinf = true;
                        }
                    }
                    rescap = c.lhs - maxresactivity;
                }

                let inferval = c.vals[inferpos];
                if reasonisrhs == (inferval > 0.0) {
                    rescap -= inferval * (api::get_var_ub_at_index(scip, infervar, bdchgidx, true) + 1.0);
                } else {
                    rescap -= inferval * (api::get_var_lb_at_index(scip, infervar, bdchgidx, true) - 1.0);
                }
            } else {
                let c = cd(scip, cons);
                rescap = if reasonisrhs { c.rhs - minresactivity } else { c.lhs - maxresactivity };
            }

            if !resactisinf {
                // Now add bounds as reasons until the residual capacity is
                // exceeded.
                for i in 0..nvars {
                    let (var, val) = (cd(scip, cons).vars[i], cd(scip, cons).vals[i]);

                    // Zero coefficients and the inferred variable can be ignored.
                    if var == infervar || scip.set.is_zero(val) {
                        continue;
                    }

                    // Check if the residual capacity is exceeded.
                    if (reasonisrhs && scip.set.is_feas_negative(rescap)) || (!reasonisrhs && scip.set.is_feas_positive(rescap)) {
                        break;
                    }

                    // Update the residual capacity due to the local bound of this
                    // variable.
                    if reasonisrhs == (val > 0.0) {
                        // rhs is reason and coeff is positive, or lhs is reason and
                        // coeff is negative -> lower bound
                        api::add_conflict_lb(scip, var, bdchgidx)?;
                        rescap -= val * (api::get_var_lb_at_index(scip, var, bdchgidx, false) - scip.vars[var].glbdom.lb);
                    } else {
                        // lhs is reason and coeff is positive, or rhs is reason and
                        // coeff is negative -> upper bound
                        api::add_conflict_ub(scip, var, bdchgidx)?;
                        rescap -= val * (api::get_var_ub_at_index(scip, var, bdchgidx, false) - scip.vars[var].glbdom.ub);
                    }
                }
                return Ok(());
            }
        }
    }

    // For a bound change on a continuous variable, all locally changed bounds
    // are responsible.
    for i in 0..nvars {
        let (var, val) = (cd(scip, cons).vars[i], cd(scip, cons).vals[i]);

        // Zero coefficients and the inferred variable can be ignored.
        if var == infervar || scip.set.is_zero(val) {
            continue;
        }

        if reasonisrhs == (val > 0.0) {
            // rhs is reason and coeff is positive, or lhs is reason and coeff is
            // negative -> lower bound is responsible
            api::add_conflict_lb(scip, var, bdchgidx)?;
        } else {
            // lhs is reason and coeff is positive, or rhs is reason and coeff is
            // negative -> upper bound is responsible
            api::add_conflict_ub(scip, var, bdchgidx)?;
        }
    }

    Ok(())
}

/// `addConflictFixedVars`: for each variable in the linear ranged row
/// constraint, except the inferred variable, adds the bounds of all fixed
/// variables to the conflict analysis' candidate store; the conflict
/// analysis can be initialized with the linear constraint being the
/// conflict detecting constraint by using `NONE` as inferred variable.
fn add_conflict_fixed_vars(scip: &mut Scip, cons: ConsId, infervar: VarId, bdchgidx: Option<&BdChgIdx>, _inferpos: i32) -> ScipResult {
    let nvars = cd(scip, cons).nvars;

    // Collect all fixed variables.
    let mut v = nvars - 1;
    while v >= 0 {
        let var = cd(scip, cons).vars[v];
        // Need to add old bounds before propagation of inferrence variable.
        if var == infervar {
            if !scip.set.is_eq(api::get_var_lb_at_index(scip, var, bdchgidx, false), scip.vars[var].glbdom.lb) {
                api::add_conflict_lb(scip, var, bdchgidx)?;
            }

            if !scip.set.is_eq(api::get_var_ub_at_index(scip, var, bdchgidx, false), scip.vars[var].glbdom.ub) {
                api::add_conflict_ub(scip, var, bdchgidx)?;
            }

            v -= 1;
            continue;
        }

        // Check for fixed variables.
        if scip
            .set
            .is_eq(api::get_var_lb_at_index(scip, var, bdchgidx, false), api::get_var_ub_at_index(scip, var, bdchgidx, false))
        {
            // Add all bounds of fixed variables which lead to the boundchange of
            // the given inference variable.
            api::add_conflict_lb(scip, var, bdchgidx)?;
            api::add_conflict_ub(scip, var, bdchgidx)?;
        }
        v -= 1;
    }

    Ok(())
}

/// `addConflictReasonVars`: add reasoning variables to conflict candidate
/// queue which led to the conflict.
fn add_conflict_reason_vars(scip: &mut Scip, vars: &[VarId], var: VarId, bound: f64) -> ScipResult {
    // Collect all variables for which the local bounds differ from their
    // global bounds.
    for v in (0..vars.len()).rev() {
        let x = &scip.vars[vars[v]];
        // Check for local bound changes variables.
        if !scip.set.is_eq(x.locdom.lb, x.glbdom.lb) {
            // Add conflict bound.
            api::add_conflict_lb(scip, vars[v], None)?;
        }

        let x = &scip.vars[vars[v]];
        if !scip.set.is_eq(x.locdom.ub, x.glbdom.ub) {
            api::add_conflict_ub(scip, vars[v], None)?;
        }
    }

    if var != NONE {
        if bound < scip.vars[var].locdom.lb {
            api::add_conflict_lb(scip, var, None)?;
        }

        if bound > scip.vars[var].locdom.ub {
            api::add_conflict_ub(scip, var, None)?;
        }
    }

    Ok(())
}

/// `resolvePropagation`: resolves a propagation on the given variable by
/// supplying the variables needed for applying the corresponding
/// propagation rule (see propagateCons()):
/// (1) activity residuals of all other variables tighten bounds of single
/// variable.
fn resolve_propagation(scip: &mut Scip, cons: ConsId, infervar: VarId, inferinfo: i32, _boundtype: BoundType, bdchgidx: Option<&BdChgIdx>) -> ScipResult<ScipResultCode> {
    let nvars = cd(scip, cons).nvars;

    // Get the position of the inferred variable in the vars array.
    let mut inferpos = infer_info_get_pos(inferinfo);
    if inferpos >= nvars || cd(scip, cons).vars[inferpos] != infervar {
        // Find inference variable in constraint.
        inferpos = 0;
        while inferpos < nvars && cd(scip, cons).vars[inferpos] != infervar {
            inferpos += 1;
        }
    }

    match infer_info_get_proprule(inferinfo) {
        // The bound of the variable was tightened, because the minimal or
        // maximal residual activity of the linear constraint (only taking the
        // other variables into account) didn't leave enough space for a larger
        // domain in order to not exceed the right hand side of the inequality.
        PROPRULE_1_RHS => add_conflict_bounds(scip, cons, infervar, bdchgidx, inferpos, true)?,
        // The bound of the variable was tightened, because the minimal or
        // maximal residual activity of the linear constraint (only taking the
        // other variables into account) didn't leave enough space for a larger
        // domain in order to not fall below the left hand side of the
        // inequality.
        PROPRULE_1_LHS => add_conflict_bounds(scip, cons, infervar, bdchgidx, inferpos, false)?,
        // The bound of the variable was tightened, because some variables were
        // already fixed and the leftover only allow the given inference
        // variable to their bounds in this given ranged row.
        PROPRULE_1_RANGEDROW => add_conflict_fixed_vars(scip, cons, infervar, bdchgidx, inferpos)?,
        _ => {
            // Invalid inference information.
            return Err(Retcode::InvalidData);
        }
    }

    Ok(ScipResultCode::Success)
}

/// `analyzeConflict`: analyzes conflicting bounds on given constraint, and
/// adds conflict constraint to problem.
fn analyze_conflict(scip: &mut Scip, cons: ConsId, reasonisrhs: bool) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // turned on.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis.
    api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

    // Add the conflicting bound for each variable of infeasible constraint to
    // conflict candidate queue.
    add_conflict_bounds(scip, cons, NONE, None, -1, reasonisrhs)?;

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `canTightenBounds`: check if there is any hope of tightening some bounds.
fn can_tighten_bounds(c: &ConsData) -> bool {
    let infcountmin = c.minactivityneginf + c.minactivityposinf + c.minactivityneghuge + c.minactivityposhuge;
    let infcountmax = c.maxactivityneginf + c.maxactivityposinf + c.maxactivityneghuge + c.maxactivityposhuge;

    !(infcountmin > 1 && infcountmax > 1)
}

/// `tightenVarUb`: tighten upper bound.
#[allow(clippy::too_many_arguments)]
fn tighten_var_ub(scip: &mut Scip, cons: ConsId, pos: i32, proprule: i32, mut newub: f64, oldub: f64, cutoff: &mut bool, nchgbds: &mut i32, force: bool) -> ScipResult {
    let var = cd(scip, cons).vars[pos];
    let lb = scip.vars[var].locdom.lb;
    newub = scip_var::adjusted_var_ub(scip, var, newub);

    if force || scip.set.is_ub_better(newub, lb, oldub) {
        let vartype = scip.vars[var].vartype;
        let impltype = scip.vars[var].varimpltype;

        // Tighten upper bound.
        let (infeasible, tightened) = scip_var::infer_var_ub_cons(scip, var, newub, cons, get_infer_int(proprule, pos), force)?;

        if infeasible {
            // Analyze conflict.
            analyze_conflict(scip, cons, true)?;

            *cutoff = true;
        } else if tightened {
            *nchgbds += 1;

            // If variable type was changed we might be able to upgrade the
            // constraint.
            if scip.vars[var].vartype != vartype || scip.vars[var].varimpltype != impltype {
                cdm(scip, cons).upgradetried = false;
            }
        }
    }
    Ok(())
}

/// `tightenVarLb`: tighten lower bound.
#[allow(clippy::too_many_arguments)]
fn tighten_var_lb(scip: &mut Scip, cons: ConsId, pos: i32, proprule: i32, mut newlb: f64, oldlb: f64, cutoff: &mut bool, nchgbds: &mut i32, force: bool) -> ScipResult {
    let var = cd(scip, cons).vars[pos];
    let ub = scip.vars[var].locdom.ub;
    newlb = scip_var::adjusted_var_lb(scip, var, newlb);

    if force || scip.set.is_lb_better(newlb, oldlb, ub) {
        let vartype = scip.vars[var].vartype;
        let impltype = scip.vars[var].varimpltype;

        // Tighten lower bound.
        let (infeasible, tightened) = scip_var::infer_var_lb_cons(scip, var, newlb, cons, get_infer_int(proprule, pos), force)?;

        if infeasible {
            // Analyze conflict.
            analyze_conflict(scip, cons, false)?;

            *cutoff = true;
        } else if tightened {
            *nchgbds += 1;

            // If variable type was changed we might be able to upgrade the
            // constraint.
            if scip.vars[var].vartype != vartype || scip.vars[var].varimpltype != impltype {
                cdm(scip, cons).upgradetried = false;
            }
        }
    }
    Ok(())
}

/// `tightenVarBoundsEasy`: tightens bounds of a single variable due to
/// activity bounds (easy case).
fn tighten_var_bounds_easy(scip: &mut Scip, cons: ConsId, pos: i32, cutoff: &mut bool, nchgbds: &mut i32, force: bool) -> ScipResult {
    // We cannot tighten variables' bounds, if the constraint may be not
    // complete.
    if scip.conss[cons].modifiable {
        return Ok(());
    }

    *cutoff = false;

    let var = cd(scip, cons).vars[pos];

    // We cannot tighten bounds of multi-aggregated variables.
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        return Ok(());
    }

    let val = cd(scip, cons).vals[pos];
    let lhs = cd(scip, cons).lhs;
    let rhs = cd(scip, cons).rhs;
    let mut lb = scip.vars[var].locdom.lb;
    let mut ub = scip.vars[var].locdom.ub;

    // Recompute activities if needed.
    with_cd(scip, cons, |scip, c| {
        if !c.validactivities {
            consdata_calc_activities(scip, c);
        }
        if !c.validminact {
            consdata_recompute_minactivity(scip, c);
        }
    });

    if val > 0.0 {
        // Check, if we can tighten the variable's upper bound.
        if !scip.set.is_infinity(rhs) {
            // Min activity should be valid at this point (if this is not true,
            // then some decisions might be wrong!).
            let minactivity = cd(scip, cons).minactivity.to_dbl();

            // If the minactivity is larger than the right hand side by
            // feasibility epsilon, the constraint is infeasible.
            if scip.set.is_feas_lt(rhs, minactivity) {
                *cutoff = true;
                return Ok(());
            }

            let mut slack = rhs - minactivity;

            // If the slack is zero in tolerances (or negative, but not enough to
            // make the constraint infeasible), we set it to zero.
            if !scip.set.is_positive(slack) {
                slack = 0.0;
            }

            let alpha = val * (ub - lb);

            if scip.set.is_sum_gt(alpha, slack) || (force && scip.set.is_gt(alpha, slack)) {
                // Compute new upper bound.
                let newub = lb + (slack / val);

                tighten_var_ub(scip, cons, pos, PROPRULE_1_RHS, newub, ub, cutoff, nchgbds, force)?;

                if *cutoff {
                    return Ok(());
                }

                // Collect the new upper bound which is needed for the lower bound
                // computation.
                ub = scip.vars[var].locdom.ub;
            }
        }

        // Check, if we can tighten the variable's lower bound.
        if !scip.set.is_infinity(-lhs) {
            // Make sure the max activity is reliable.
            if !cd(scip, cons).validmaxact {
                with_cd(scip, cons, |scip, c| consdata_recompute_maxactivity(scip, c));
            }
            let maxactivity = cd(scip, cons).maxactivity.to_dbl();

            // If the maxactivity is smaller than the left hand side by
            // feasibility epsilon, the constraint is infeasible.
            if scip.set.is_feas_lt(maxactivity, lhs) {
                *cutoff = true;
                return Ok(());
            }

            let mut slack = maxactivity - lhs;

            // If the slack is zero in tolerances (or negative, but not enough to
            // make the constraint infeasible), we set it to zero.
            if !scip.set.is_positive(slack) {
                slack = 0.0;
            }

            let alpha = val * (ub - lb);

            if scip.set.is_sum_gt(alpha, slack) || (force && scip.set.is_gt(alpha, slack)) {
                // Compute new lower bound.
                let newlb = ub - (slack / val);

                tighten_var_lb(scip, cons, pos, PROPRULE_1_LHS, newlb, lb, cutoff, nchgbds, force)?;

                if *cutoff {
                    return Ok(());
                }
            }
        }
    } else {
        // Check, if we can tighten the variable's lower bound.
        if !scip.set.is_infinity(rhs) {
            // Min activity should be valid at this point (if this is not true,
            // then some decisions might be wrong!).
            let minactivity = cd(scip, cons).minactivity.to_dbl();

            // If the minactivity is larger than the right hand side by
            // feasibility epsilon, the constraint is infeasible.
            if scip.set.is_feas_lt(rhs, minactivity) {
                *cutoff = true;
                return Ok(());
            }

            let mut slack = rhs - minactivity;

            // If the slack is zero in tolerances (or negative, but not enough to
            // make the constraint infeasible), we set it to zero.
            if !scip.set.is_positive(slack) {
                slack = 0.0;
            }

            let alpha = val * (lb - ub);

            if scip.set.is_sum_gt(alpha, slack) || (force && scip.set.is_gt(alpha, slack)) {
                // Compute new lower bound.
                let newlb = ub + slack / val;

                tighten_var_lb(scip, cons, pos, PROPRULE_1_RHS, newlb, lb, cutoff, nchgbds, force)?;

                if *cutoff {
                    return Ok(());
                }

                // Collect the new lower bound which is needed for the upper bound
                // computation.
                lb = scip.vars[var].locdom.lb;
            }
        }

        // Check, if we can tighten the variable's upper bound.
        if !scip.set.is_infinity(-lhs) {
            // Make sure the max activity is reliable.
            if !cd(scip, cons).validmaxact {
                with_cd(scip, cons, |scip, c| consdata_recompute_maxactivity(scip, c));
            }
            let maxactivity = cd(scip, cons).maxactivity.to_dbl();

            // If the maxactivity is smaller than the left hand side by
            // feasibility epsilon, the constraint is infeasible.
            if scip.set.is_feas_lt(maxactivity, lhs) {
                *cutoff = true;
                return Ok(());
            }

            let mut slack = maxactivity - lhs;

            // If the slack is zero in tolerances (or negative, but not enough to
            // make the constraint infeasible), we set it to zero.
            if !scip.set.is_positive(slack) {
                slack = 0.0;
            }

            let alpha = val * (lb - ub);

            if scip.set.is_sum_gt(alpha, slack) || (force && scip.set.is_gt(alpha, slack)) {
                // Compute new upper bound.
                let newub = lb - (slack / val);

                tighten_var_ub(scip, cons, pos, PROPRULE_1_LHS, newub, ub, cutoff, nchgbds, force)?;

                if *cutoff {
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// `analyzeConflictRangedRow`: analyzes conflicting bounds on given ranged
/// row constraint, and adds conflict constraint to problem; `var` is the
/// variable which was tried to fix/tighten (or `NONE`) to `bound` (or
/// `INVALID`).
fn analyze_conflict_ranged_row(scip: &mut Scip, cons: ConsId, vars: &[VarId], var: VarId, bound: f64) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // turned on.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis.
    api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

    // Add the conflicting fixed variables of this ranged row constraint to
    // conflict candidate queue.
    add_conflict_fixed_vars(scip, cons, NONE, None, -1)?;

    // Add reasoning variables to conflict candidate queue which led to the
    // conflict.
    add_conflict_reason_vars(scip, vars, var, bound)?;

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// Creates, adds locally and releases an artificial linear constraint of
/// [`ranged_row_propagation`].
fn add_artificial_cons(scip: &mut Scip, name: &str, vars: &[VarId], vals: &[f64], lhs: f64, rhs: f64) -> ScipResult {
    let mut newcons = create_cons_linear(scip, name, vars, vals, lhs, rhs, true, true, true, false, true, true, false, false, true, false)?;
    scip_cons::add_cons_local(scip, newcons, NONE)?;
    scip_cons::release_cons(scip, &mut newcons)
}

/// `rangedRowPropagation`: propagate ranged rows.
///
/// Check ranged rows for possible solutions, possibly detect infeasibility,
/// fix variables due to having only one possible solution, tighten bounds
/// if having only two possible solutions or add constraints which propagate
/// a subset of variables better.
///
/// Example:
/// c1: 12 x1 + 9  x2 - x3 = 0  with x1, x2 free and 1 <= x3 <= 2
///
/// x3 needs to be a multiple of 3, so the instance is infeasible.
///
/// Example:
/// c1: 12 x1 + 9  x2 - x3 = 1  with x1, x2 free and 1 <= x3 <= 2
///
/// The only possible value for x3 is 2, so the variable will be fixed.
fn ranged_row_propagation(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32, nchgbds: &mut i32, naddconss: &mut i32) -> ScipResult {
    // Modifiable constraint can be changed so we do not have all necessary
    // information.
    if scip.conss[cons].modifiable {
        return Ok(());
    }

    // We already did full ranged row propagation.
    if cd(scip, cons).rangedrowpropagated == 2 {
        return Ok(());
    }

    // At least three variables are needed.
    if cd(scip, cons).nvars < 3 {
        return Ok(());
    }

    // Do nothing on normal inequalities.
    if scip.set.is_infinity(-cd(scip, cons).lhs) || scip.set.is_infinity(cd(scip, cons).rhs) {
        return Ok(());
    }

    // Get constraint handler data.
    let conshdlr = scip.conss[cons].conshdlr;

    let addartconss = hd(scip, conshdlr).rangedrowartcons && api::get_depth(scip) < 1 && !api::in_probing(scip) && !crate::tree::tree_in_repropagation(scip);

    // We may add artificial constraints.
    if addartconss {
        cdm(scip, cons).rangedrowpropagated = 2;
    }
    // We are not allowed to add artificial constraints during propagation; if
    // nothing changed on this constraint since the last rangedrowpropagation,
    // we can stop; otherwise, we mark this constraint to be rangedrowpropagated
    // without artificial constraints.
    else {
        if cd(scip, cons).rangedrowpropagated > 0 {
            return Ok(());
        }

        cdm(scip, cons).rangedrowpropagated = 1;
    }
    let mut fixedact = 0.0;
    let mut nfixedconsvars = 0;

    let lb_ = |scip: &Scip, var: VarId| scip.vars[var].locdom.lb;
    let ub_ = |scip: &Scip, var: VarId| scip.vars[var].locdom.ub;
    let var_at = |scip: &Scip, v: i32| cd(scip, cons).vars[v];
    let val_at = |scip: &Scip, v: i32| cd(scip, cons).vals[v];
    let nvars = |scip: &Scip| cd(scip, cons).nvars;

    // Calculate fixed activity and number of fixed variables.
    let mut v = nvars(scip) - 1;
    while v >= 0 {
        // All zero coefficients should be eliminated.
        let var = var_at(scip, v);
        if scip.set.is_eq(lb_(scip, var), ub_(scip, var)) {
            fixedact += lb_(scip, var) * val_at(scip, v);
            nfixedconsvars += 1;
        }
        v -= 1;
    }

    // Do not work with huge fixed activities.
    if scip.set.is_huge_value(fixedact.abs()) {
        return Ok(());
    }

    // Compute lhs and rhs for unfixed variables only and get number of unfixed
    // variables.
    let lhs = cd(scip, cons).lhs - fixedact;
    let rhs = cd(scip, cons).rhs - fixedact;
    let nunfixedvars = nvars(scip) - nfixedconsvars;

    // Temporary memory for variables and coefficients which may lead to
    // infeasibility.
    let mut infcheckvars: Vec<VarId> = Vec::with_capacity(nunfixedvars.max(0) as usize);
    let mut infcheckvals: Vec<f64> = Vec::with_capacity(nunfixedvars.max(0) as usize);

    let mut absminbincoef = crate::def::REAL_MAX;
    let mut ncontvars = 0;
    let mut gcdisone = true;
    let mut possiblegcd = true;

    // We now partition all unfixed variables in two groups:
    //
    // the first one contains all integral variable with integral coefficient
    // so that all variables in this group will have a gcd greater than 1, this
    // group will be implicitly given
    //
    // the second group will contain all left unfixed variables and will be
    // saved as infcheckvars with corresponding coefficients as infcheckvals,
    // the order of these variables should be the same as in the consdata
    // object.

    // Find first integral variables with integral coefficient greater than 1,
    // thereby collecting all other unfixed variables.
    v = -1;
    let mut pos = -1;
    loop {
        v += 1;

        // Partition the variables, do not change the order of collection,
        // because it might be used later on.
        while v < nvars(scip) && {
            let (var, val) = (var_at(scip, v), val_at(scip, v));
            !var_is_integral(scip, var) || !scip.set.is_integral(val) || scip.set.is_eq(val.abs(), 1.0)
        } {
            let (var, val) = (var_at(scip, v), val_at(scip, v));
            if !scip.set.is_eq(lb_(scip, var), ub_(scip, var)) {
                if !var_is_integral(scip, var) {
                    ncontvars += 1;
                } else if var_is_binary(scip, var) {
                    let absval = val.abs();
                    if absminbincoef > absval {
                        absminbincoef = absval;
                    }
                }

                gcdisone = gcdisone && scip.set.is_eq(val.abs(), 1.0);
                possiblegcd = false;
                infcheckvars.push(var);
                infcheckvals.push(val);

                if pos == -1 {
                    pos = v;
                }
            }
            v += 1;
        }

        if !(v < nvars(scip) && {
            let var = var_at(scip, v);
            scip.set.is_eq(lb_(scip, var), ub_(scip, var))
        }) {
            break;
        }
    }

    'terminate: {
        // If the first group of variables is empty, we stop.
        if v == nvars(scip) {
            break 'terminate;
        }

        // We need at least two non-continuous variables.
        if ncontvars + 2 > nunfixedvars {
            break 'terminate;
        }

        let feastol = scip.set.feastol();

        let mut gcd = (val_at(scip, v).abs() + feastol) as i64;

        // Go on to partition the variables, do not change the order of
        // collection, because it might be used later on; calculate gcd over
        // the first part of variables.
        while v < nvars(scip) {
            let (var, val) = (var_at(scip, v), val_at(scip, v));
            if scip.set.is_eq(lb_(scip, var), ub_(scip, var)) {
                v += 1;
                continue;
            }

            if var_is_binary(scip, var) {
                let absval = val.abs();
                if absminbincoef > absval {
                    absminbincoef = absval;
                }
            }

            if !var_is_integral(scip, var) || !scip.set.is_integral(val) || scip.set.is_eq(val.abs(), 1.0) {
                if !var_is_integral(scip, var) {
                    ncontvars += 1;
                }

                gcdisone = gcdisone && scip.set.is_eq(val.abs(), 1.0);
                possiblegcd = false;
                infcheckvars.push(var);
                infcheckvals.push(val);

                if pos == -1 {
                    pos = v;
                }
            } else {
                let gcdtmp = crate::misc::calc_gre_com_div(gcd, (val.abs() + feastol) as i64);

                if gcdtmp == 1 {
                    infcheckvars.push(var);
                    infcheckvals.push(val);

                    if pos == -1 {
                        pos = v;
                    }
                } else {
                    gcd = gcdtmp;
                }
            }
            v += 1;
        }

        let ninfcheckvars = infcheckvars.len() as i32;

        // It should not happen that all variables are of integral type and have
        // a gcd >= 2, this should be done by normalizeCons().
        if ninfcheckvars == 0 {
            break 'terminate;
        }

        let mut minactinfvarsinvalid = false;
        let mut maxactinfvarsinvalid = false;
        let mut maxactinfvars = 0.0;
        let mut minactinfvars = 0.0;

        // Calculate activities over all infcheckvars.
        for v in (0..ninfcheckvars as usize).rev() {
            let lb = lb_(scip, infcheckvars[v]);
            let ub = ub_(scip, infcheckvars[v]);

            if scip.set.is_infinity(-lb) {
                if infcheckvals[v] < 0.0 {
                    maxactinfvarsinvalid = true;
                } else {
                    minactinfvarsinvalid = true;
                }
            } else if infcheckvals[v] < 0.0 {
                maxactinfvars += infcheckvals[v] * lb;
            } else {
                minactinfvars += infcheckvals[v] * lb;
            }

            if scip.set.is_infinity(ub) {
                if infcheckvals[v] > 0.0 {
                    maxactinfvarsinvalid = true;
                } else {
                    minactinfvarsinvalid = true;
                }
            } else if infcheckvals[v] > 0.0 {
                maxactinfvars += infcheckvals[v] * ub;
            } else {
                minactinfvars += infcheckvals[v] * ub;
            }

            // Better abort on to big values.
            if scip.set.is_huge_value(-minactinfvars) {
                minactinfvarsinvalid = true;
            }
            if scip.set.is_huge_value(maxactinfvars) {
                maxactinfvarsinvalid = true;
            }

            if minactinfvarsinvalid || maxactinfvarsinvalid {
                break 'terminate;
            }
        }

        let gcdf = gcd as f64;

        // Check if between left hand side and right hand side exist a feasible
        // point, if not the constraint leads to infeasibility.
        if !scip.set.is_integral((lhs - maxactinfvars) / gcdf) && scip.set.is_gt(scip.set.ceil((lhs - maxactinfvars) / gcdf) * gcdf, rhs - minactinfvars) {
            // Start conflict analysis.
            analyze_conflict_ranged_row(scip, cons, &infcheckvars, NONE, INVALID)?;

            *cutoff = true;
        } else if ncontvars == 0 {
            let mut gcdinfvars: i64 = -1;

            // Check for gcd over all infcheckvars.
            if possiblegcd {
                let mut v = ninfcheckvars - 1;
                gcdinfvars = (infcheckvals[v as usize].abs() + feastol) as i64;

                while v >= 0 && gcdinfvars >= 2 {
                    gcdinfvars = crate::misc::calc_gre_com_div(gcdinfvars, (infcheckvals[v as usize].abs() + feastol) as i64);
                    v -= 1;
                }
            } else if gcdisone {
                gcdinfvars = 1;
            }

            // Compute solutions for this ranged row, if all variables are of
            // integral type with integral coefficients.
            if gcdinfvars >= 1 {
                let gcdinfvarsf = gcdinfvars as f64;
                let mut minvalue = INVALID;
                let mut maxvalue = INVALID;
                let mut nsols = 0;

                let mut value = scip.set.ceil(minactinfvars - scip.set.feastol());

                // Check how many possible solutions exist.
                while scip.set.is_le(value, maxactinfvars) {
                    let mut value2 = value + gcdf * scip.set.ceil((lhs - value) / gcdf);

                    // value2 might violate lhs due to numerics, in this case take
                    // the next divisible number.
                    if !scip.set.is_ge(value2, lhs) {
                        value2 += gcdf;
                    }

                    if scip.set.is_le(value2, rhs) {
                        nsols += 1;

                        // Early termination if we found more than two solutions.
                        if nsols == 3 {
                            break;
                        }

                        if minvalue == INVALID {
                            minvalue = value;
                        }

                        maxvalue = value;
                    }
                    value += gcdinfvarsf;
                }

                // Determine last possible solution for better bounding.
                if nsols == 3 {
                    value = scip.set.floor(maxactinfvars + scip.set.feastol());

                    // Check how many possible solutions exist.
                    while scip.set.is_ge(value, minactinfvars) {
                        let mut value2 = value + gcdf * scip.set.floor((rhs - value) / gcdf);

                        // value2 might violate rhs due to numerics, in this case
                        // take the next divisible number.
                        if !scip.set.is_le(value2, rhs) {
                            value2 -= gcdf;
                        }

                        if scip.set.is_ge(value2, lhs) {
                            maxvalue = value;
                            break;
                        }
                        value -= gcdinfvarsf;
                    }
                }

                // No possible solution found.
                if nsols == 0 {
                    // Start conflict analysis.
                    analyze_conflict_ranged_row(scip, cons, &infcheckvars, NONE, INVALID)?;

                    *cutoff = true;
                }
                // If only one solution exist we can extract a new constraint or
                // fix variables.
                else if nsols == 1 {
                    // We can fix the only variable in our second set of variables.
                    if ninfcheckvars == 1 {
                        // Fix variable to only possible value.
                        let (infeasible, fixed) =
                            scip_var::infer_var_fix_cons(scip, infcheckvars[0], maxvalue / infcheckvals[0], cons, get_infer_int(PROPRULE_1_RANGEDROW, pos), true)?;
                        *cutoff = infeasible;

                        if *cutoff {
                            // Start conflict analysis.
                            analyze_conflict_ranged_row(scip, cons, &infcheckvars, NONE, INVALID)?;
                        }

                        if fixed {
                            *nfixedvars += 1;
                        }
                    } else {
                        // Check for exactly one unfixed variable which is not part
                        // of the infcheckvars.
                        if ninfcheckvars == nunfixedvars - 1 {
                            let mut foundvar = false;
                            let mut w = 0;

                            // Find variable which is not an infcheckvar and fix it.
                            let mut v = 0;
                            while v < nvars(scip) - 1 {
                                let var = var_at(scip, v);
                                if !scip.set.is_eq(lb_(scip, var), ub_(scip, var)) {
                                    if w >= ninfcheckvars || var != infcheckvars[w as usize] {
                                        foundvar = true;

                                        let val = val_at(scip, v);
                                        let bound = if val < 0.0 {
                                            scip.set.floor((lhs - maxvalue) / val)
                                        } else {
                                            scip.set.ceil((lhs - maxvalue) / val)
                                        };

                                        // Fix variable to only possible value.
                                        let (infeasible, fixed) = scip_var::infer_var_fix_cons(scip, var, bound, cons, get_infer_int(PROPRULE_1_RANGEDROW, v), true)?;
                                        *cutoff = infeasible;

                                        if *cutoff {
                                            // Start conflict analysis.
                                            analyze_conflict_ranged_row(scip, cons, &infcheckvars, var, bound)?;
                                        }

                                        if fixed {
                                            *nfixedvars += 1;
                                        }

                                        break;
                                    }

                                    w += 1;
                                }
                                v += 1;
                            }

                            // Maybe last variable was the not infcheckvar.
                            if !foundvar {
                                let (var, val) = (var_at(scip, v), val_at(scip, v));
                                let bound = if val < 0.0 {
                                    scip.set.floor((lhs - maxvalue) / val)
                                } else {
                                    scip.set.ceil((lhs - maxvalue) / val)
                                };

                                // Fix variable to only possible value.
                                let (infeasible, fixed) = scip_var::infer_var_fix_cons(scip, var, bound, cons, get_infer_int(PROPRULE_1_RANGEDROW, v), true)?;
                                *cutoff = infeasible;

                                if *cutoff {
                                    // Start conflict analysis.
                                    analyze_conflict_ranged_row(scip, cons, &infcheckvars, var, bound)?;
                                }

                                if fixed {
                                    *nfixedvars += 1;
                                }
                            }
                        } else if addartconss && (scip.set.is_gt(minvalue, minactinfvars) || scip.set.is_lt(maxvalue, maxactinfvars)) {
                            // Aggregation possible if we have two variables, but this
                            // will be done later on.
                            let name = format!("{}_artcons_{}", scip.conss[cons].name, hd(scip, conshdlr).naddconss);
                            hdm(scip, conshdlr).naddconss += 1;

                            // Create, add, and release new artificial constraint.
                            add_artificial_cons(scip, &name, &infcheckvars, &infcheckvals, maxvalue, maxvalue)?;
                            *naddconss += 1;
                        }
                    }
                }
                // At least two solutions.
                else {
                    // Only one variable in the second set, so we can bound this
                    // variables.
                    if ninfcheckvars == 1 {
                        let (newlb, newub) = if infcheckvals[0] < 0.0 {
                            (maxvalue / infcheckvals[0], minvalue / infcheckvals[0])
                        } else {
                            (minvalue / infcheckvals[0], maxvalue / infcheckvals[0])
                        };

                        if newlb > lb_(scip, infcheckvars[0]) {
                            // Tighten variable lower bound to minimal possible value.
                            let (infeasible, tightened) = scip_var::infer_var_lb_cons(scip, infcheckvars[0], newlb, cons, get_infer_int(PROPRULE_1_RANGEDROW, pos), true)?;
                            *cutoff = infeasible;

                            if *cutoff {
                                // Start conflict analysis.
                                analyze_conflict_ranged_row(scip, cons, &infcheckvars, NONE, INVALID)?;
                            }

                            if tightened {
                                *nchgbds += 1;
                            }
                        }

                        if newub < ub_(scip, infcheckvars[0]) {
                            // Tighten variable upper bound to maximal possible value.
                            let (infeasible, tightened) = scip_var::infer_var_ub_cons(scip, infcheckvars[0], newub, cons, get_infer_int(PROPRULE_1_RANGEDROW, pos), true)?;
                            *cutoff = infeasible;

                            if *cutoff {
                                // Start conflict analysis.
                                analyze_conflict_ranged_row(scip, cons, &infcheckvars, NONE, INVALID)?;
                            }

                            if tightened {
                                *nchgbds += 1;
                            }
                        }
                    }
                    // Check if we have only one not infcheckvars, if so we can
                    // tighten this variable.
                    else if ninfcheckvars == nunfixedvars - 1 {
                        let mut foundvar = false;
                        let mut w = 0;

                        // Find variable which is not an infcheckvar and fix it.
                        let mut v = 0;
                        while v < nvars(scip) - 1 {
                            let var = var_at(scip, v);
                            if !scip.set.is_eq(lb_(scip, var), ub_(scip, var)) {
                                if w >= ninfcheckvars || var != infcheckvars[w as usize] {
                                    foundvar = true;

                                    ranged_row_tighten_other(scip, cons, v, lhs, rhs, minvalue, maxvalue, &infcheckvars, cutoff, nchgbds)?;

                                    break;
                                }

                                w += 1;
                            }
                            v += 1;
                        }

                        // Maybe last variable was the not infcheckvar.
                        if !foundvar {
                            ranged_row_tighten_other(scip, cons, v, lhs, rhs, minvalue, maxvalue, &infcheckvars, cutoff, nchgbds)?;
                        }
                    }
                    // At least two solutions and more than one variable, so we add a
                    // new constraint which bounds the feasible region for our
                    // infcheckvars, if possible.
                    else if addartconss && (scip.set.is_gt(minvalue, minactinfvars) || scip.set.is_lt(maxvalue, maxactinfvars)) {
                        let newlhs = if scip.set.is_gt(minvalue, minactinfvars) { minvalue } else { -scip.set.infinity() };
                        let newrhs = if scip.set.is_lt(maxvalue, maxactinfvars) { maxvalue } else { scip.set.infinity() };

                        if !scip.set.is_infinity(-newlhs) || !scip.set.is_infinity(newrhs) {
                            let name = format!("{}_artcons1_{}", scip.conss[cons].name, hd(scip, conshdlr).naddconss);
                            hdm(scip, conshdlr).naddconss += 1;

                            // Create, add, and release new artificial constraint.
                            add_artificial_cons(scip, &name, &infcheckvars, &infcheckvals, newlhs, newrhs)?;
                            *naddconss += 1;
                        }
                    }
                }
            }
        } else if addartconss && ncontvars < ninfcheckvars {
            let mut maxact = 0.0;
            let mut minact = 0.0;
            let mut w = 0;

            // Compute activities of non-infcheckvars.
            let mut v = 0;
            while v < nvars(scip) {
                let (var, val) = (var_at(scip, v), val_at(scip, v));
                if w < ninfcheckvars && var == infcheckvars[w as usize] {
                    w += 1;
                    v += 1;
                    continue;
                }

                if !scip.set.is_eq(lb_(scip, var), ub_(scip, var)) {
                    if var_is_binary(scip, var) {
                        if val > 0.0 {
                            maxact += val;
                        } else {
                            minact += val;
                        }
                    } else if val > 0.0 {
                        let tmpval = val * lb_(scip, var);
                        if scip.set.is_huge_value(-tmpval) {
                            break;
                        }
                        minact += tmpval;

                        let tmpval = val * ub_(scip, var);
                        if scip.set.is_huge_value(tmpval) {
                            break;
                        }
                        maxact += tmpval;
                    } else {
                        let tmpval = val * ub_(scip, var);
                        if scip.set.is_huge_value(-tmpval) {
                            break;
                        }
                        minact += tmpval;

                        let tmpval = val * lb_(scip, var);
                        if scip.set.is_huge_value(tmpval) {
                            break;
                        }
                        maxact += tmpval;
                    }
                }
                v += 1;
            }
            if v == nvars(scip) && !scip.set.is_huge_value(-minact) && !scip.set.is_huge_value(maxact) {
                let newlhs = lhs - maxact;
                let newrhs = rhs - minact;

                let name = format!("{}_artcons2_{}", scip.conss[cons].name, hd(scip, conshdlr).naddconss);
                hdm(scip, conshdlr).naddconss += 1;

                // Create, add, and release new artificial constraint.
                add_artificial_cons(scip, &name, &infcheckvars, &infcheckvals, newlhs, newrhs)?;
                *naddconss += 1;
            }
        }
    }

    Ok(())
}

/// The tightening of the only unfixed variable outside the infcheckvars of
/// [`ranged_row_propagation`] with at least two solutions (C repeats it for
/// the found and the last variable).
#[allow(clippy::too_many_arguments)]
fn ranged_row_tighten_other(
    scip: &mut Scip,
    cons: ConsId,
    v: i32,
    lhs: f64,
    rhs: f64,
    minvalue: f64,
    maxvalue: f64,
    infcheckvars: &[VarId],
    cutoff: &mut bool,
    nchgbds: &mut i32,
) -> ScipResult {
    let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
    let (newlb, newub) = if val < 0.0 {
        (scip.set.floor((rhs - minvalue) / val), scip.set.floor((lhs - maxvalue) / val))
    } else {
        (scip.set.ceil((lhs - maxvalue) / val), scip.set.ceil((rhs - minvalue) / val))
    };

    if newlb > scip.vars[var].locdom.lb {
        // Tighten variable lower bound to minimal possible value.
        let (infeasible, tightened) = scip_var::infer_var_lb_cons(scip, var, newlb, cons, get_infer_int(PROPRULE_1_RANGEDROW, v), true)?;
        *cutoff = infeasible;

        if *cutoff {
            // Start conflict analysis.
            analyze_conflict_ranged_row(scip, cons, infcheckvars, var, newlb)?;
        }

        if tightened {
            *nchgbds += 1;
        }
    }

    // C reads the variable at position v again.
    let var = cd(scip, cons).vars[v];
    if newub < scip.vars[var].locdom.ub {
        // Tighten variable upper bound to maximal possible value.
        let (infeasible, tightened) = scip_var::infer_var_ub_cons(scip, var, newub, cons, get_infer_int(PROPRULE_1_RANGEDROW, v), true)?;
        *cutoff = infeasible;

        if *cutoff {
            // Start conflict analysis.
            analyze_conflict_ranged_row(scip, cons, infcheckvars, var, newub)?;
        }

        if tightened {
            *nchgbds += 1;
        }
    }
    Ok(())
}

/// `tightenVarBounds`: tightens bounds of a single variable due to activity
/// bounds.
fn tighten_var_bounds(scip: &mut Scip, cons: ConsId, pos: i32, cutoff: &mut bool, nchgbds: &mut i32, force: bool) -> ScipResult {
    // We cannot tighten variables' bounds, if the constraint may be not
    // complete.
    if scip.conss[cons].modifiable {
        return Ok(());
    }

    *cutoff = false;

    let var = cd(scip, cons).vars[pos];

    // We cannot tighten bounds of multi-aggregated variables.
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        return Ok(());
    }

    let val = cd(scip, cons).vals[pos];
    let lhs = cd(scip, cons).lhs;
    let rhs = cd(scip, cons).rhs;
    let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_residuals(scip, c, var, val, false));
    let mut minresactivity = min.act;
    let mut maxresactivity = max.act;
    let (ismintight, isminsettoinfinity) = (min.istight, min.issettoinfinity);
    let (ismaxtight, ismaxsettoinfinity) = (max.istight, max.issettoinfinity);
    let mut lb = scip.vars[var].locdom.lb;
    let mut ub = scip.vars[var].locdom.ub;
    let integral = var_is_integral(scip, var);

    if val > 0.0 {
        // Check, if we can tighten the variable's bounds reliably, therefore
        // only consider sides which are small or relatively different to the
        // residual activity bound to avoid cancellation leading to numerical
        // difficulties.
        if !isminsettoinfinity && !scip.set.is_infinity(rhs) && ismintight && (scip.set.is_lt(rhs.abs(), 1.0) || !scip.set.is_eq(minresactivity / rhs, 1.0)) {
            let mut newub = (rhs - minresactivity) / val;
            if !scip.set.is_infinity(newub) && ((force && scip.set.is_lt(newub, ub)) || (integral && scip.set.is_feas_lt(newub, ub)) || scip.set.is_ub_better(newub, lb, ub)) {
                let mut activityunreliable = scip.set.is_update_unreliable(minresactivity, cd(scip, cons).lastminactivity);

                // Check minresactivities for reliability.
                if activityunreliable {
                    minresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, true, false);
                    newub = (rhs - minresactivity) / val;
                    activityunreliable = scip.set.is_infinity(-minresactivity)
                        || (!scip.set.is_ub_better(newub, lb, ub) && (!scip.set.is_feas_lt(newub, ub) || !integral) && (!force || !scip.set.is_lt(newub, ub)));
                }

                if !activityunreliable {
                    // Tighten upper bound.
                    let (infeasible, tightened) = scip_var::infer_var_ub_cons(scip, var, newub, cons, get_infer_int(PROPRULE_1_RHS, pos), force)?;
                    if infeasible {
                        // Analyze conflict.
                        analyze_conflict(scip, cons, true)?;

                        *cutoff = true;
                        return Ok(());
                    }
                    if tightened {
                        // Get bound again: it may be additionally modified due to
                        // integrality.
                        ub = scip.vars[var].locdom.ub;
                        *nchgbds += 1;
                    }
                }
            }
        }

        if !ismaxsettoinfinity && !scip.set.is_infinity(-lhs) && ismaxtight && (scip.set.is_lt(lhs.abs(), 1.0) || !scip.set.is_eq(maxresactivity / lhs, 1.0)) {
            let mut newlb = (lhs - maxresactivity) / val;
            if !scip.set.is_infinity(-newlb) && ((force && scip.set.is_gt(newlb, lb)) || (integral && scip.set.is_feas_gt(newlb, lb)) || scip.set.is_lb_better(newlb, lb, ub)) {
                // Check maxresactivities for reliability.
                if scip.set.is_update_unreliable(maxresactivity, cd(scip, cons).lastmaxactivity) {
                    maxresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, false, false);
                    newlb = (lhs - maxresactivity) / val;

                    if scip.set.is_infinity(maxresactivity)
                        || (!scip.set.is_lb_better(newlb, lb, ub) && (!scip.set.is_feas_gt(newlb, lb) || !integral) && (!force || !scip.set.is_gt(newlb, lb)))
                    {
                        return Ok(());
                    }
                }

                // Tighten lower bound.
                let (infeasible, tightened) = scip_var::infer_var_lb_cons(scip, var, newlb, cons, get_infer_int(PROPRULE_1_LHS, pos), force)?;
                if infeasible {
                    // Analyze conflict.
                    analyze_conflict(scip, cons, false)?;

                    *cutoff = true;
                    return Ok(());
                }
                if tightened {
                    // Get bound again: it may be additionally modified due to
                    // integrality.
                    lb = scip.vars[var].locdom.lb;
                    *nchgbds += 1;
                }
            }
        }
    } else {
        // Check, if we can tighten the variable's bounds reliably, therefore
        // only consider sides which are small or relatively different to the
        // residual activity bound to avoid cancellation leading to numerical
        // difficulties.
        if !isminsettoinfinity && !scip.set.is_infinity(rhs) && ismintight && (scip.set.is_lt(rhs.abs(), 1.0) || !scip.set.is_eq(minresactivity / rhs, 1.0)) {
            let mut newlb = (rhs - minresactivity) / val;
            if !scip.set.is_infinity(-newlb) && ((force && scip.set.is_gt(newlb, lb)) || (integral && scip.set.is_feas_gt(newlb, lb)) || scip.set.is_lb_better(newlb, lb, ub)) {
                let mut activityunreliable = scip.set.is_update_unreliable(minresactivity, cd(scip, cons).lastminactivity);

                // Check minresactivities for reliability.
                if activityunreliable {
                    minresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, true, false);
                    newlb = (rhs - minresactivity) / val;

                    activityunreliable = scip.set.is_infinity(-minresactivity)
                        || (!scip.set.is_lb_better(newlb, lb, ub) && (!scip.set.is_feas_gt(newlb, lb) || !integral) && (!force || !scip.set.is_gt(newlb, lb)));
                }

                if !activityunreliable {
                    // Tighten lower bound.
                    let (infeasible, tightened) = scip_var::infer_var_lb_cons(scip, var, newlb, cons, get_infer_int(PROPRULE_1_RHS, pos), force)?;
                    if infeasible {
                        // Analyze conflict.
                        analyze_conflict(scip, cons, true)?;

                        *cutoff = true;
                        return Ok(());
                    }
                    if tightened {
                        // Get bound again: it may be additionally modified due to
                        // integrality.
                        lb = scip.vars[var].locdom.lb;
                        *nchgbds += 1;
                    }
                }
            }
        }

        if !ismaxsettoinfinity && !scip.set.is_infinity(-lhs) && ismaxtight && (scip.set.is_lt(lhs.abs(), 1.0) || !scip.set.is_eq(maxresactivity / lhs, 1.0)) {
            let mut newub = (lhs - maxresactivity) / val;
            if !scip.set.is_infinity(newub) && ((force && scip.set.is_lt(newub, ub)) || (integral && scip.set.is_feas_lt(newub, ub)) || scip.set.is_ub_better(newub, lb, ub)) {
                // Check maxresactivities for reliability.
                if scip.set.is_update_unreliable(maxresactivity, cd(scip, cons).lastmaxactivity) {
                    maxresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, false, false);
                    newub = (lhs - maxresactivity) / val;

                    // C has `&&` here where the other three cases have `||`.
                    if scip.set.is_infinity(maxresactivity)
                        || (!scip.set.is_ub_better(newub, lb, ub) && (!scip.set.is_feas_lt(newub, ub) && !integral) && (!force || !scip.set.is_lt(newub, ub)))
                    {
                        return Ok(());
                    }
                }

                // Tighten upper bound.
                let (infeasible, tightened) = scip_var::infer_var_ub_cons(scip, var, newub, cons, get_infer_int(PROPRULE_1_LHS, pos), force)?;
                if infeasible {
                    // Analyze conflict.
                    analyze_conflict(scip, cons, false)?;

                    *cutoff = true;
                    return Ok(());
                }
                if tightened {
                    // Get bound again: it may be additionally modified due to
                    // integrality.
                    ub = scip.vars[var].locdom.ub;
                    *nchgbds += 1;
                }
            }
        }
    }

    let _ = (lb, ub);
    Ok(())
}

const MAXTIGHTENROUNDS: i32 = 10;

/// `tightenBounds`: tightens bounds of variables in constraint due to
/// activity bounds.
fn tighten_bounds(scip: &mut Scip, cons: ConsId, maxeasyactivitydelta: f64, sortvars: bool, cutoff: &mut bool, nchgbds: &mut i32) -> ScipResult {
    *cutoff = false;

    // We cannot tighten variables' bounds, if the constraint may be not
    // complete.
    if scip.conss[cons].modifiable {
        return Ok(());
    }

    // If a constraint was created after presolve, then it may hold fixed
    // variables; if there are even multi-aggregated variables, then we cannot
    // do bound tightening on these; thus, ensure here again that variable
    // fixings have been applied.
    *cutoff = apply_fixings(scip, cons, true)?;
    if *cutoff {
        return Ok(());
    }

    // Check if constraint has any chances of tightening bounds.
    if !can_tighten_bounds(cd(scip, cons)) {
        return Ok(());
    }

    let nvars = cd(scip, cons).nvars;
    let force = nvars == 1 && !scip.conss[cons].modifiable;

    // We are at the root node or during presolving.
    let tightenmode: u32 = if api::get_depth(scip) < 1 { 2 } else { 1 };

    // Stop if we already tightened the constraint and the tightening is not
    // forced.
    if !force && cd(scip, cons).boundstightened >= tightenmode {
        return Ok(());
    }

    // Ensure that the variables are properly sorted.
    if sortvars && scip.set.stage >= Stage::InitSolve && !cd(scip, cons).coefsorted {
        consdata_sort(scip, cons);
    }

    // Update maximal activity delta if necessary.
    if cd(scip, cons).maxactdelta == INVALID {
        with_cd(scip, cons, |scip, c| consdata_recompute_max_activity_delta(scip, c));
    }

    // This may happen if all variables are fixed.
    if scip.set.is_feas_zero(cd(scip, cons).maxactdelta) {
        return Ok(());
    }

    if !scip.set.is_infinity(cd(scip, cons).maxactdelta) {
        // Use maximal activity delta to skip propagation (cannot deduce
        // anything).
        let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_bounds(scip, c, false));
        let c = cd(scip, cons);

        let slack = if scip.set.is_infinity(c.rhs) || min.issettoinfinity {
            scip.set.infinity()
        } else {
            c.rhs - min.act
        };
        let surplus = if scip.set.is_infinity(-c.lhs) || max.issettoinfinity {
            scip.set.infinity()
        } else {
            max.act - c.lhs
        };

        // Check if the constraint will propagate.
        if scip.set.is_le(c.maxactdelta, crate::def::min(slack, surplus)) {
            return Ok(());
        }
    }

    // Check if we can use fast implementation for easy and numerically well
    // behaved cases.
    let easycase = scip.set.is_lt(cd(scip, cons).maxactdelta, maxeasyactivitydelta);

    // As long as the bounds might be tightened again, try to tighten them;
    // abort after a maximal number of rounds.
    let mut lastchange = -1;

    let mut nrounds = 0;
    while (force || cd(scip, cons).boundstightened < tightenmode) && nrounds < MAXTIGHTENROUNDS {
        // Ensure that the variables are properly sorted.
        //
        // Note: it might happen that integer variables become binary during
        // bound tightening at the root node.
        if sortvars && scip.set.stage >= Stage::InitSolve && !cd(scip, cons).coefsorted {
            consdata_sort(scip, cons);
        }

        // Mark the constraint to have the variables' bounds tightened.
        cdm(scip, cons).boundstightened = tightenmode;

        // Try to tighten the bounds of each variable in the constraint. During
        // solving process, the binary variable sorting enables skipping
        // variables.
        let mut v = 0;
        while v < nvars && v != lastchange && !*cutoff {
            let oldnchgbds = *nchgbds;

            if easycase {
                tighten_var_bounds_easy(scip, cons, v, cutoff, nchgbds, force)?;
            } else {
                tighten_var_bounds(scip, cons, v, cutoff, nchgbds, force)?;
            }

            // If there was no progress, skip the rest of the binary variables.
            let c = cd(scip, cons);
            if *nchgbds > oldnchgbds {
                lastchange = v;
                v += 1;
            } else if c.coefsorted && v < c.nbinvars - 1 && !scip.set.is_feas_eq(scip.vars[c.vars[v]].locdom.ub, scip.vars[c.vars[v]].locdom.lb) {
                v = c.nbinvars;
            } else {
                v += 1;
            }
        }
        nrounds += 1;
    }

    Ok(())
}

/// `checkCons`: checks linear constraint for feasibility of given solution
/// or current solution (`NONE`); returns whether the constraint is
/// violated.
fn check_cons(scip: &mut Scip, cons: ConsId, sol: SolId, checklprows: bool, checkrelmaxabs: bool) -> ScipResult<bool> {
    let mut violated = false;

    let row = cd(scip, cons).row;
    let activity = if row != NONE {
        if !checklprows && scip.rows[row].lppos >= 0 {
            return Ok(false);
        } else if sol == NONE && !has_current_node_lp(scip) {
            consdata_compute_pseudo_activity(scip, cd(scip, cons))
        } else {
            api::get_row_sol_activity(scip, row, sol)
        }
    } else {
        with_cd(scip, cons, |scip, c| consdata_get_activity(scip, c, sol))
    };

    let (lhs, rhs, checkabsolute) = (cd(scip, cons).lhs, cd(scip, cons).rhs, cd(scip, cons).checkabsolute);

    // Calculate absolute and relative bound violations.
    let lhsviol = lhs - activity;
    let rhsviol = activity - rhs;

    let mut absviol = 0.0;
    let mut relviol = 0.0;
    if lhsviol > 0.0 && lhsviol > rhsviol {
        absviol = lhsviol;
        relviol = crate::misc::rel_diff(lhs, activity);
    } else if rhsviol > 0.0 {
        absviol = rhsviol;
        relviol = crate::misc::rel_diff(activity, rhs);
    }

    // The activity of pseudo solutions may be invalid if it comprises positive
    // and negative infinity contributions; we return infeasible for safety.
    if activity == INVALID {
        violated = true;
        // Set violation of invalid pseudo solutions.
        absviol = INVALID;
        relviol = INVALID;

        // Reset constraint age since we are in enforcement.
        api::reset_cons_age(scip, cons);
    }
    // Check with relative tolerances (the default).
    else if !checkabsolute && (scip.set.is_feas_lt(activity, lhs) || scip.set.is_feas_gt(activity, rhs)) {
        // The "normal" check: one of the two sides is violated.
        if !checkrelmaxabs {
            violated = true;

            // Only reset constraint age if we are in enforcement.
            if sol == NONE {
                api::reset_cons_age(scip, cons);
            }
        }
        // The (much) more complicated check: we try to disregard random noise
        // and violations of a 0.0 side which are small compared to the
        // absolute values occurring in the activity.
        else {
            let mut maxabs: f64 = 1.0;

            // Compute maximum absolute value.
            for v in 0..cd(scip, cons).nvars {
                let coef = cd(scip, cons).vals[v];
                let solval = get_sol_val(scip, sol, cd(scip, cons).vars[v]);
                let absval = (coef * solval).abs();
                maxabs = crate::def::max(maxabs, absval);
            }

            // Regard left hand side, first.
            if scip.set.is_feas_lt(activity, lhs) {
                // Check whether violation is random noise.
                if (lhs - activity) <= (1e-15 * maxabs) {
                    // Only increase constraint age if we are in enforcement.
                    if sol == NONE {
                        api::inc_cons_age(scip, cons)?;
                    }
                }
                // lhs is violated and lhs is 0.0: use relative tolerance w.r.t.
                // largest absolute value.
                else if scip.set.is_zero(lhs) {
                    if (lhs - activity) <= (scip.set.feastol() * maxabs) {
                        // Only increase constraint age if we are in enforcement.
                        if sol == NONE {
                            api::inc_cons_age(scip, cons)?;
                        }
                    } else {
                        violated = true;

                        // Only reset constraint age if we are in enforcement.
                        if sol == NONE {
                            api::reset_cons_age(scip, cons);
                        }
                    }
                } else {
                    violated = true;

                    // Only reset constraint age if we are in enforcement.
                    if sol == NONE {
                        api::reset_cons_age(scip, cons);
                    }
                }
            }

            // Now regard right hand side.
            if scip.set.is_feas_gt(activity, rhs) {
                // Check whether violation is random noise.
                if (activity - rhs) <= (1e-15 * maxabs) {
                    // Only increase constraint age if we are in enforcement.
                    if sol == NONE {
                        api::inc_cons_age(scip, cons)?;
                    }
                }
                // rhs is violated and rhs is 0.0, use relative tolerance w.r.t.
                // largest absolute value.
                else if scip.set.is_zero(rhs) {
                    if (activity - rhs) <= (scip.set.feastol() * maxabs) {
                        // Only increase constraint age if we are in enforcement.
                        if sol == NONE {
                            api::inc_cons_age(scip, cons)?;
                        }
                    } else {
                        violated = true;

                        // Only reset constraint age if we are in enforcement.
                        if sol == NONE {
                            api::reset_cons_age(scip, cons);
                        }
                    }
                } else {
                    violated = true;

                    // Only reset constraint age if we are in enforcement.
                    if sol == NONE {
                        api::reset_cons_age(scip, cons);
                    }
                }
            }
        }
    }
    // Check with absolute tolerances.
    else if checkabsolute
        && ((!scip.set.is_infinity(-lhs) && scip.set.is_gt(lhs - activity, scip.set.feastol()))
            || (!scip.set.is_infinity(rhs) && scip.set.is_gt(activity - rhs, scip.set.feastol())))
    {
        violated = true;

        // Only reset constraint age if we are in enforcement.
        if sol == NONE {
            api::reset_cons_age(scip, cons);
        }
    } else {
        // Only increase constraint age if we are in enforcement.
        if sol == NONE {
            api::inc_cons_age(scip, cons)?;
        }
    }

    // Update absolute and relative violation of the solution.
    if sol != NONE {
        api::update_sol_lp_cons_violation(scip, sol, absviol, relviol);
    }

    Ok(violated)
}

/// `createRow`: creates an LP row in a linear constraint data.
fn create_row(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let c = &scip.conss[cons];
    let (name, local, modifiable, removable) = (c.name.clone(), c.is_local(), c.modifiable, c.is_removable());
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

    let row = scip_lp::create_empty_row_cons(scip, cons, &name, lhs, rhs, local, modifiable, removable)?;
    cdm(scip, cons).row = row;

    let n = cd(scip, cons).nvars as usize;
    let vars = cd(scip, cons).vars.0[..n].to_vec();
    let vals = cd(scip, cons).vals.0[..n].to_vec();
    scip_lp::add_vars_to_row(scip, row, &vars, &vals)?;

    Ok(())
}

/// `addRelaxation`: adds linear constraint as cut to the LP; returns
/// whether a cutoff was found.
fn add_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let mut cutoff = false;

    if cd(scip, cons).row == NONE {
        if !scip.conss[cons].modifiable {
            // Replace all fixed variables by active counterparts, as we have no
            // chance to do this anymore after the row has been added to the LP;
            // removing this here will make test cons/linear/fixedvar.c fail (as
            // of 2018-12-03).
            cutoff = apply_fixings(scip, cons, true)?;
            if cutoff {
                return Ok(true);
            }
        }

        // Convert consdata object into LP row.
        create_row(scip, cons)?;
    }

    // Insert LP row as cut.
    let row = cd(scip, cons).row;
    if scip.rows[row].lppos < 0 {
        // If presolving is turned off, the row might be trivial.
        let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
        if !scip.set.is_infinity(-lhs) || !scip.set.is_infinity(rhs) {
            cutoff = scip_lp::add_row(scip, row, false)?;
        }
    }

    Ok(cutoff)
}

/// `separateCons`: separates linear constraint: adds linear constraint as
/// cut, if violated by given solution.
#[allow(clippy::too_many_arguments)]
fn separate_cons(scip: &mut Scip, cons: ConsId, checkrelmaxabs: bool, sol: SolId, separatecards: bool, separateall: bool, ncuts: &mut i32, cutoff: &mut bool) -> ScipResult {
    let oldncuts = *ncuts;
    *cutoff = false;

    let violated = check_cons(scip, cons, sol, sol != NONE, checkrelmaxabs)?;

    if violated {
        // Insert LP row as cut.
        *cutoff = add_relaxation(scip, cons)?;
        *ncuts += 1;
    } else if !scip.conss[cons].modifiable && separatecards && cd(scip, cons).nvars > 0 {
        let n = cd(scip, cons).nvars as usize;
        let vars = cd(scip, cons).vars.0[..n].to_vec();
        let vals = cd(scip, cons).vals.0[..n].to_vec();
        let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

        // Relax linear constraint into knapsack constraint and separate lifted
        // cardinality cuts.
        if !separateall && sol == NONE {
            // We only want to call the knapsack cardinality cut separator for
            // rows that have a non-zero dual solution.
            let row = cd(scip, cons).row;
            if row != NONE && scip.rows[row].lppos >= 0 {
                let dualsol = scip_lp::row_get_dualsol(scip, row);
                if scip.set.is_feas_negative(dualsol) {
                    if !scip.set.is_infinity(rhs) {
                        crate::cons_knapsack::separate_relaxed_knapsack(scip, cons, NONE, &vars, &vals, 1.0, rhs, sol, cutoff, ncuts)?;
                    }
                } else if scip.set.is_feas_positive(dualsol) && !scip.set.is_infinity(-lhs) {
                    crate::cons_knapsack::separate_relaxed_knapsack(scip, cons, NONE, &vars, &vals, -1.0, -lhs, sol, cutoff, ncuts)?;
                }
            }
        } else {
            if !scip.set.is_infinity(rhs) {
                crate::cons_knapsack::separate_relaxed_knapsack(scip, cons, NONE, &vars, &vals, 1.0, rhs, sol, cutoff, ncuts)?;
            }
            if !scip.set.is_infinity(-lhs) {
                crate::cons_knapsack::separate_relaxed_knapsack(scip, cons, NONE, &vars, &vals, -1.0, -lhs, sol, cutoff, ncuts)?;
            }
        }
    }

    if *ncuts > oldncuts {
        api::reset_cons_age(scip, cons);
    }

    Ok(())
}

/// `propagateCons`: propagation method for linear constraints.
#[allow(clippy::too_many_arguments)]
fn propagate_cons(
    scip: &mut Scip,
    cons: ConsId,
    tightenbounds: bool,
    rangedrowpropagation: bool,
    maxeasyactivitydelta: f64,
    sortvars: bool,
    cutoff: &mut bool,
    nchgbds: &mut i32,
) -> ScipResult {
    if cd(scip, cons).eventdata.is_none() {
        let eventhdlr = hd(scip, scip.conss[cons].conshdlr).eventhdlr;

        // Catch bound change events of variables.
        cons_catch_all_events(scip, cons, eventhdlr)?;
    }

    *cutoff = false;

    // We can only infer activity bounds of the linear constraint, if it is not
    // modifiable.
    if !scip.conss[cons].modifiable {
        // Increase age of constraint; age is reset to zero, if a conflict or a
        // propagation was found.
        if !crate::tree::tree_in_repropagation(scip) {
            api::inc_cons_age(scip, cons)?;
        }

        // Tighten the variable's bounds.
        if tightenbounds {
            let oldnchgbds = *nchgbds;

            tighten_bounds(scip, cons, maxeasyactivitydelta, sortvars, cutoff, nchgbds)?;

            if *nchgbds > oldnchgbds {
                api::reset_cons_age(scip, cons);
            }
        }

        // Propagate ranged rows.
        if rangedrowpropagation && tightenbounds && !*cutoff {
            let mut nfixedvars = 0;
            let mut naddconss = 0;

            ranged_row_propagation(scip, cons, cutoff, &mut nfixedvars, nchgbds, &mut naddconss)?;

            if nfixedvars > 0 {
                *nchgbds += 2 * nfixedvars;
            }
        }

        // Check constraint for infeasibility and redundancy.
        if !*cutoff {
            let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_bounds(scip, c, true));
            let (minactivity, maxactivity) = (min.act, max.act);
            let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

            if scip.set.is_feas_gt(minactivity, rhs) {
                // Analyze conflict.
                analyze_conflict(scip, cons, true)?;

                api::reset_cons_age(scip, cons);
                *cutoff = true;
            } else if scip.set.is_feas_lt(maxactivity, lhs) {
                // Analyze conflict.
                analyze_conflict(scip, cons, false)?;

                api::reset_cons_age(scip, cons);
                *cutoff = true;
            } else if scip.set.is_ge(minactivity, lhs) && scip.set.is_le(maxactivity, rhs) {
                // Remove the constraint locally unless it has become empty, in
                // which case it is removed globally.
                if cd(scip, cons).nvars > 0 {
                    scip_cons::del_cons_local(scip, cons)?;
                } else {
                    scip_cons::del_cons(scip, cons)?;
                }
            }
        }
    }

    Ok(())
}

/*
 * Presolving methods
 */

/// `fixVariables`: converts all variables with fixed domain into FIXED
/// variables.
fn fix_variables(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32) -> ScipResult {
    let mut v = 0;
    while v < cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];
        let varstatus = scip.vars[var].varstatus;

        if varstatus != VarStatus::Fixed {
            let lb = scip.vars[var].glbdom.lb;
            let ub = scip.vars[var].glbdom.ub;
            if scip.set.is_eq(lb, ub) {
                let fixval = crate::misc::select_simple_value(lb, ub, MAXDNOM);
                let (infeasible, fixed) = scip_var::fix_var(scip, var, fixval)?;
                if infeasible {
                    *cutoff = true;
                    return Ok(());
                }
                if fixed {
                    *nfixedvars += 1;
                }
            }
        }
        v += 1;
    }

    let infeasible = apply_fixings(scip, cons, true)?;
    if infeasible {
        *cutoff = true;
        return Ok(());
    }

    Ok(())
}

const MAX_CLIQUE_NONZEROS_PER_CONS: i32 = 1000000;

/// The finiteness flags of the global activities that
/// [`extract_cliques`] reads (C's `finitenegmaxact` reads the local
/// `maxactivityneghuge`).
struct CliqueFinite {
    lhs: bool,
    rhs: bool,
    negminact: bool,
    negmaxact: bool,
    posminact: bool,
    posmaxact: bool,
    minact: bool,
    maxact: bool,
}

fn clique_finite(scip: &Scip, c: &ConsData) -> CliqueFinite {
    let negminact = c.glbminactivityneginf == 0 && c.glbminactivityneghuge == 0;
    let negmaxact = c.glbmaxactivityneginf == 0 && c.maxactivityneghuge == 0;
    let posminact = c.glbminactivityposinf == 0 && c.glbminactivityposhuge == 0;
    let posmaxact = c.glbmaxactivityposinf == 0 && c.glbmaxactivityposhuge == 0;
    CliqueFinite {
        lhs: !scip.set.is_infinity(-c.lhs),
        rhs: !scip.set.is_infinity(c.rhs),
        negminact,
        negmaxact,
        posminact,
        posmaxact,
        minact: negminact && posminact,
        maxact: negmaxact && posmaxact,
    }
}

/// The binary variables of [`extract_cliques`] in constraint order (the
/// data is sorted by type): writes them to the front of `binvars` and
/// `binvarvals`, returns `(nposbinvars, nnegbinvars, allonebinary)`.
fn collect_clique_binvars(scip: &Scip, c: &ConsData, binvars: &mut Vec<VarId>, binvarvals: &mut Vec<f64>) -> (i32, i32, i32) {
    let mut nposbinvars = 0;
    let mut nnegbinvars = 0;
    let mut allonebinary = 0;
    for i in 0..c.nvars {
        let (var, val) = (c.vars[i], c.vals[i]);
        if var_is_binary(scip, var) {
            if scip.set.is_eq(val.abs(), 1.0) {
                allonebinary += 1;
            }

            let k = (nposbinvars + nnegbinvars) as usize;
            if k >= binvars.len() {
                binvars.resize(k + 1, NONE);
                binvarvals.resize(k + 1, 0.0);
            }
            binvars[k] = var;
            binvarvals[k] = val;

            if scip.set.is_positive(val) {
                nposbinvars += 1;
            } else {
                nnegbinvars += 1;
            }
        }
        // Stop searching for binary variables, because the constraint data is
        // sorted.
        else if !var_is_integral(scip, var) {
            break;
        }
    }
    (nposbinvars, nnegbinvars, allonebinary)
}

/// The bound change follow-up between the cases of [`extract_cliques`]:
/// removes fixings, tightens the bounds and collects the binary variables
/// again.
#[allow(clippy::too_many_arguments)]
fn extract_cliques_refresh(
    scip: &mut Scip,
    cons: ConsId,
    maxeasyactivitydelta: f64,
    sortvars: bool,
    nfixedvars: &mut i32,
    nchgbds: &mut i32,
    cutoff: &mut bool,
    binvars: &mut Vec<VarId>,
    binvarvals: &mut Vec<f64>,
    counts: &mut (i32, i32, i32),
    nvars: &mut i32,
) -> ScipResult {
    // Check for fixed variables.
    fix_variables(scip, cons, cutoff, nfixedvars)?;

    if !*cutoff {
        // Tighten variable's bounds.
        tighten_bounds(scip, cons, maxeasyactivitydelta, sortvars, cutoff, nchgbds)?;

        if !*cutoff {
            // Check for fixed variables.
            fix_variables(scip, cons, cutoff, nfixedvars)?;

            if !*cutoff {
                // Sort variables by variable type.
                consdata_sort(scip, cons);

                // Recompute activities if needed.
                with_cd(scip, cons, |scip, c| {
                    if !c.validactivities {
                        consdata_calc_activities(scip, c);
                    }
                });

                *nvars = cd(scip, cons).nvars;

                // Update binary variables.
                *counts = collect_clique_binvars(scip, cd(scip, cons), binvars, binvarvals);
            }
        }
    }
    Ok(())
}

/// `extractCliques`: extracts cliques of the constraint and adds them to
/// SCIP.
///
/// The following clique extraction mechanism are implemeneted
///
/// 1. collect binary variables and sort them in non increasing order, then
///
///    a) if the constraint has a finite right hand side and the negative
///       infinity counters for the minactivity are zero then add the
///       variables as a clique for which all successive pairs of
///       coefficients fullfill the following condition
///
///         minactivity + vals[i] + vals[i+1] > rhs
///
///       and also add the binary to binary implication also for
///       non-successive variables for which the same argument holds
///
///         minactivity + vals[i] + vals[j] > rhs
///
///       e.g. 5.3 x1 + 3.6 x2 + 3.3 x3 + 2.1 x4 <= 5.5 (all x are binary)
///       would lead to the clique (x1, x2, x3) and the binary to binary
///       implications x1 = 1 => x4 = 0 and x2 = 1 => x4 = 0
///
///    b) if the constraint has a finite left hand side and the positive
///       infinity counters for the maxactivity are zero then add the
///       variables as a clique for which all successive pairs of
///       coefficients fullfill the follwoing condition
///
///         maxactivity + vals[i] + vals[i-1] < lhs
///
///       and also add the binary to binary implication also for
///       non-successive variables for which the same argument holds
///
///         maxactivity + vals[i] + vals[j] < lhs
///
///       e.g. you could multiply the above example by -1
///
///    c) the constraint has a finite right hand side and a finite
///       minactivity then add the variables as a negated clique(clique on
///       the negated variables) for which all successive pairs of
///       coefficients fullfill the following condition
///
///         minactivity - vals[i] - vals[i-1] > rhs
///
///       and also add the binary to binary implication also for
///       non-successive variables for which the same argument holds
///
///         minactivity - vals[i] - vals[j] > rhs
///
///       e.g. -4 x1 -3 x2 - 2 x3 + 2 x4 <= -4 would lead to the (negated)
///       clique (~x1, ~x2) and the binary to binary implication x1 = 0 =>
///       x3 = 1
///
///    d) the constraint has a finite left hand side and a finite
///       maxactivity then add the variables as a negated clique(clique on
///       the negated variables) for which all successive pairs of
///       coefficients fullfill the following condition
///
///         maxactivity - vals[i] - vals[i+1] < lhs
///
///       and also add the binary to binary implication also for
///       non-successive variables for which the same argument holds
///
///         maxactivity - vals[i] - vals[j] < lhs
///
///       e.g. you could multiply the above example by -1
///
/// 2. if the linear constraint represents a set-packing or
///    set-partitioning constraint, the whole constraint is added as clique,
///    (this part is done at the end of the method)
#[allow(clippy::too_many_arguments)]
fn extract_cliques(scip: &mut Scip, cons: ConsId, maxeasyactivitydelta: f64, sortvars: bool, nfixedvars: &mut i32, nchgbds: &mut i32, cutoff: &mut bool) -> ScipResult {
    if cd(scip, cons).nvars < 2 {
        return Ok(());
    }

    // Add implications if possible
    //
    // for now we only add binary to non-binary implications, and this is only
    // done for the binary variable with the maximal absolute contribution and
    // also only if this variable would force all other variables to their
    // bound corresponding to the global minimal activity of the constraint.
    if !cd(scip, cons).implsadded {
        // Sort variables by variable type.
        consdata_sort(scip, cons);

        // Fast abort if no binaries seem to exist; "seem to", because there are
        // rare situations in which variables may actually not be sorted by
        // type, even though consdataSort has been called.
        if !var_is_binary(scip, cd(scip, cons).vars[0usize]) {
            return Ok(());
        }

        let nvars = cd(scip, cons).nvars;

        // Recompute activities if needed.
        with_cd(scip, cons, |scip, c| {
            if !c.validactivities {
                consdata_calc_activities(scip, c);
            }
        });

        let f = clique_finite(scip, cd(scip, cons));

        if (f.rhs || f.lhs) && (f.negminact || f.posminact || f.negmaxact || f.posmaxact) {
            let mut maxabscontrib = -1.0;
            let mut posval = false;
            let mut allbinary = true;
            let oldnchgbds = *nchgbds;
            let mut nimpls = 0;
            let mut position = -1;

            // We need a valid minimal/maximal activity to add cliques.
            with_cd(scip, cons, |scip, c| {
                if (f.negminact || f.posminact) && !c.validglbminact {
                    consdata_recompute_glb_minactivity(scip, c);
                }
                if (f.negmaxact || f.posmaxact) && !c.validglbmaxact {
                    consdata_recompute_glb_maxactivity(scip, c);
                }
            });

            // Determine maximal contribution to the activity.
            let mut v = nvars - 1;
            while v >= 0 {
                let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                if var_is_binary(scip, var) {
                    if val > 0.0 {
                        let value = val * scip.vars[var].glbdom.ub;

                        if value > maxabscontrib {
                            maxabscontrib = value;
                            position = v;
                            posval = true;
                        }
                    } else {
                        let value = (val * scip.vars[var].glbdom.lb).abs();

                        if value > maxabscontrib {
                            maxabscontrib = value;
                            position = v;
                            posval = false;
                        }
                    }
                } else {
                    allbinary = false;
                }
                v -= 1;
            }

            if !scip.set.is_eq(maxabscontrib, 1.0) && !allbinary {
                let posvar = cd(scip, cons).vars[position];

                // If the right hand side and the minimal activity are finite and
                // changing the variable with the biggest influence to their bound
                // forces all other variables to be at their minimal contribution,
                // we can add these implications.
                if f.rhs && f.minact && scip.set.is_eq(cd(scip, cons).glbminactivity.to_dbl(), cd(scip, cons).rhs - maxabscontrib) {
                    let mut v = nvars - 1;
                    while v >= 0 {
                        let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                        // Binary to binary implications will be collected when
                        // extrating cliques.
                        if !var_is_binary(scip, var) {
                            if v != position {
                                let (infeasible, nbdchgs) = if val > 0.0 {
                                    // Add implications.
                                    let lb = scip.vars[var].glbdom.lb;
                                    scip_var::add_var_implication(scip, posvar, posval, var, BoundType::Upper, lb)?
                                } else {
                                    // Add implications.
                                    let ub = scip.vars[var].glbdom.ub;
                                    scip_var::add_var_implication(scip, posvar, posval, var, BoundType::Lower, ub)?
                                };
                                nimpls += 1;
                                *nchgbds += nbdchgs;

                                if infeasible {
                                    *cutoff = true;
                                    break;
                                }
                            }
                        }
                        // Stop when reaching a 'real' binary variable because the
                        // variables are sorted after their type.
                        else if scip.vars[var].vartype == VarType::Binary && !var_is_implied_integral(scip, var) {
                            break;
                        }
                        v -= 1;
                    }
                }

                // If the left hand side and the maximal activity are finite and
                // changing the variable with the biggest influence to their bound
                // forces all other variables to be at their minimal contribution,
                // we can add these implications.
                if f.lhs && f.maxact && scip.set.is_eq(cd(scip, cons).glbmaxactivity.to_dbl(), cd(scip, cons).lhs - maxabscontrib) {
                    let mut v = nvars - 1;
                    while v >= 0 {
                        let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
                        // Binary to binary implications will be collected when
                        // extrating cliques.
                        if !var_is_binary(scip, var) {
                            if v != position {
                                let (infeasible, nbdchgs) = if val > 0.0 {
                                    // Add implications.
                                    let ub = scip.vars[var].glbdom.ub;
                                    scip_var::add_var_implication(scip, posvar, posval, var, BoundType::Lower, ub)?
                                } else {
                                    // Add implications.
                                    let lb = scip.vars[var].glbdom.lb;
                                    scip_var::add_var_implication(scip, posvar, posval, var, BoundType::Upper, lb)?
                                };
                                nimpls += 1;
                                *nchgbds += nbdchgs;

                                if infeasible {
                                    *cutoff = true;
                                    break;
                                }
                            }
                        }
                        // Stop when reaching a 'real' binary variable because the
                        // variables are sorted after their type.
                        else if scip.vars[var].vartype == VarType::Binary {
                            break;
                        }
                        v -= 1;
                    }
                }

                // Did we find some implications.
                if nimpls > 0 {
                    if *cutoff {
                        return Ok(());
                    }

                    // Did we find some boundchanges, then we need to remove fixings
                    // and tighten the bounds further.
                    if *nchgbds - oldnchgbds > 0 {
                        // Check for fixed variables.
                        fix_variables(scip, cons, cutoff, nfixedvars)?;
                        if *cutoff {
                            return Ok(());
                        }

                        // Tighten variable's bounds.
                        tighten_bounds(scip, cons, maxeasyactivitydelta, sortvars, cutoff, nchgbds)?;
                        if *cutoff {
                            return Ok(());
                        }

                        // Check for fixed variables.
                        fix_variables(scip, cons, cutoff, nfixedvars)?;
                        if *cutoff {
                            return Ok(());
                        }
                    }
                }
            }
        }

        cdm(scip, cons).implsadded = true;
    }

    // Check if we already added the cliques of this constraint.
    if cd(scip, cons).cliquesadded {
        return Ok(());
    }

    cdm(scip, cons).cliquesadded = true;
    let mut cliquenonzerosadded = 0;
    let mut stopped = false;

    // Sort variables by variable type.
    consdata_sort(scip, cons);

    let mut nvars = cd(scip, cons).nvars;

    // Recompute activities if needed.
    with_cd(scip, cons, |scip, c| {
        if !c.validactivities {
            consdata_calc_activities(scip, c);
        }
    });

    let f = clique_finite(scip, cd(scip, cons));

    // 1. we wheck whether some variables do not fit together into this
    // constraint and add the corresponding clique information.
    if (f.rhs || f.lhs) && (f.negminact || f.posminact || f.negmaxact || f.posmaxact) {
        let mut binvars: Vec<VarId> = vec![NONE; nvars as usize];
        let mut binvarvals: Vec<f64> = vec![0.0; nvars as usize];

        // Collect binary variables.
        let mut counts = collect_clique_binvars(scip, cd(scip, cons), &mut binvars, &mut binvarvals);

        // Setppc constraints will be handled later; we need at least two binary
        // variables with same sign to extract cliques.
        if counts.2 < nvars && (counts.0 >= 2 || counts.1 >= 2) {
            let mut oldnchgbds = *nchgbds;

            // We need a valid minimal/maximal activity to add cliques.
            with_cd(scip, cons, |scip, c| {
                if (f.negminact || f.posminact) && !c.validglbminact {
                    consdata_recompute_glb_minactivity(scip, c);
                }
                if (f.negmaxact || f.posmaxact) && !c.validglbmaxact {
                    consdata_recompute_glb_maxactivity(scip, c);
                }
            });

            // Sort coefficients non-increasing to be faster in the clique search.
            let n = (counts.0 + counts.1) as usize;
            sort::sort_down(&mut binvarvals[..n], (&mut binvars[..n],), sort::cmp_real);

            // Case a).
            let nposbinvars = counts.0;
            if f.rhs && f.negminact && nposbinvars >= 2 {
                // Compute value that needs to be exceeded.
                let threshold = cd(scip, cons).rhs - cd(scip, cons).glbminactivity.to_dbl();

                let mut j: i32 = 1;

                // Check if at least two variables are in a clique.
                if scip.set.is_feas_gt(binvarvals[0] + binvarvals[j as usize], threshold) {
                    j += 1;
                    // Check for extending the clique.
                    while j < nposbinvars {
                        if !scip.set.is_feas_gt(binvarvals[j as usize - 1] + binvarvals[j as usize], threshold) {
                            break;
                        }
                        j += 1;
                    }

                    // Add clique with at least two variables.
                    let (infeasible, nbdchgs) = scip_var::add_clique(scip, &binvars[..j as usize], &[], false)?;

                    if infeasible {
                        *cutoff = true;
                    }

                    *nchgbds += nbdchgs;

                    cliquenonzerosadded += j;
                    if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                        stopped = true;
                    }

                    // Exchange the last variable in the clique if possible and add
                    // all new ones.
                    if !stopped && !*cutoff && j < nposbinvars {
                        let mut lastfit = j - 2;

                        // Copy all 'main'-clique variables.
                        let mut clqvars = binvars[..j as usize].to_vec();

                        // Iterate up to the end with j and up to the front with
                        // lastfit, and check for different cliques.
                        while lastfit >= 0 && j < nposbinvars {
                            // Check if two variables are in a clique.
                            if scip.set.is_feas_gt(binvarvals[lastfit as usize] + binvarvals[j as usize], threshold) {
                                clqvars[lastfit as usize + 1] = binvars[j as usize];

                                // Add clique with at least two variables.
                                let (infeasible, nbdchgs) = scip_var::add_clique(scip, &clqvars[..lastfit as usize + 2], &[], false)?;

                                if infeasible {
                                    *cutoff = true;
                                    break;
                                }

                                *nchgbds += nbdchgs;

                                cliquenonzerosadded += lastfit + 2;
                                if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                                    stopped = true;
                                    break;
                                }

                                j += 1;
                            } else {
                                lastfit -= 1;
                            }
                        }
                    }
                }
            }

            // Did we find some boundchanges, then we need to remove fixings and
            // tighten the bounds further.
            if !stopped && !*cutoff && *nchgbds - oldnchgbds > 0 {
                extract_cliques_refresh(
                    scip,
                    cons,
                    maxeasyactivitydelta,
                    sortvars,
                    nfixedvars,
                    nchgbds,
                    cutoff,
                    &mut binvars,
                    &mut binvarvals,
                    &mut counts,
                    &mut nvars,
                )?;
                oldnchgbds = *nchgbds;
            }

            // Case b).
            let (nposbinvars, nnegbinvars) = (counts.0, counts.1);
            if !stopped && !*cutoff && f.lhs && f.posmaxact && nnegbinvars >= 2 {
                // Compute value that needs to be deceeded.
                let threshold = cd(scip, cons).lhs - cd(scip, cons).glbmaxactivity.to_dbl();

                let i = nposbinvars + nnegbinvars - 1;
                let mut j = i - 1;

                // Check if two variables are in a clique.
                if scip.set.is_feas_lt(binvarvals[i as usize] + binvarvals[j as usize], threshold) {
                    j -= 1;
                    // Check for extending the clique.
                    while j >= nposbinvars {
                        if !scip.set.is_feas_lt(binvarvals[j as usize + 1] + binvarvals[j as usize], threshold) {
                            break;
                        }
                        j -= 1;
                    }
                    let jstart = j;

                    // Add clique with at least two variables.
                    let (infeasible, nbdchgs) = scip_var::add_clique(scip, &binvars[(j + 1) as usize..(i + 1) as usize], &[], false)?;

                    if infeasible {
                        *cutoff = true;
                    }

                    *nchgbds += nbdchgs;

                    cliquenonzerosadded += i - j;
                    if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                        stopped = true;
                    }

                    // Exchange the last variable in the clique if possible and add
                    // all new ones.
                    if !stopped && !*cutoff && jstart >= nposbinvars {
                        let mut lastfit = jstart + 1;

                        // Copy all 'main'-clique variables.
                        let mut clqvars = binvars[lastfit as usize..(lastfit + i - j) as usize].to_vec();
                        lastfit += 1;

                        // Iterate up to the front with j and up to the end with
                        // lastfit, and check for different cliques.
                        while lastfit <= i && j >= nposbinvars {
                            // Check if two variables are in a clique.
                            if scip.set.is_feas_lt(binvarvals[lastfit as usize] + binvarvals[j as usize], threshold) {
                                let start = (lastfit - jstart - 2) as usize;
                                clqvars[start] = binvars[j as usize];

                                // Add clique with at least two variables.
                                let len = (i - lastfit + 2) as usize;
                                let (infeasible, nbdchgs) = scip_var::add_clique(scip, &clqvars[start..start + len], &[], false)?;

                                if infeasible {
                                    *cutoff = true;
                                    break;
                                }

                                *nchgbds += nbdchgs;

                                cliquenonzerosadded += i - lastfit + 2;
                                if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                                    stopped = true;
                                    break;
                                }

                                j -= 1;
                            } else {
                                lastfit += 1;
                            }
                        }
                    }
                }
            }

            // Did we find some boundchanges, then we need to remove fixings and
            // tighten the bounds further.
            if !stopped && !*cutoff && *nchgbds - oldnchgbds > 0 {
                extract_cliques_refresh(
                    scip,
                    cons,
                    maxeasyactivitydelta,
                    sortvars,
                    nfixedvars,
                    nchgbds,
                    cutoff,
                    &mut binvars,
                    &mut binvarvals,
                    &mut counts,
                    &mut nvars,
                )?;
                oldnchgbds = *nchgbds;
            }

            // Case c).
            let (nposbinvars, nnegbinvars) = (counts.0, counts.1);
            if !*cutoff && f.rhs && f.minact && nnegbinvars >= 2 {
                // Initialize clique values array for adding a negated clique.
                let values = vec![false; nnegbinvars as usize];

                // Compute value that needs to be exceeded.
                let threshold = cd(scip, cons).rhs - cd(scip, cons).glbminactivity.to_dbl();

                let i = nposbinvars + nnegbinvars - 1;
                let mut j = i - 1;

                if scip.set.is_feas_gt(-binvarvals[i as usize] - binvarvals[j as usize], threshold) {
                    j -= 1;
                    // Check for extending the clique.
                    while j >= nposbinvars {
                        if !scip.set.is_feas_gt(-binvarvals[j as usize + 1] - binvarvals[j as usize], threshold) {
                            break;
                        }
                        j -= 1;
                    }
                    let jstart = j;

                    // Add negated clique with at least two variables.
                    let len = (i - j) as usize;
                    let (infeasible, nbdchgs) = scip_var::add_clique(scip, &binvars[(j + 1) as usize..(i + 1) as usize], &values[..len], false)?;

                    if infeasible {
                        *cutoff = true;
                    }

                    *nchgbds += nbdchgs;

                    cliquenonzerosadded += i - j;
                    if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                        stopped = true;
                    }

                    // Exchange the last variable in the clique if possible and add
                    // all new ones.
                    if !stopped && !*cutoff && jstart >= nposbinvars {
                        let mut lastfit = j + 1;

                        // Copy all 'main'-clique variables.
                        let mut clqvars = binvars[lastfit as usize..(lastfit + i - j) as usize].to_vec();
                        lastfit += 1;

                        // Iterate up to the front with j and up to the end with
                        // lastfit, and check for different cliques.
                        while lastfit <= i && j >= nposbinvars {
                            // Check if two variables are in a negated clique.
                            if scip.set.is_feas_gt(-binvarvals[lastfit as usize] - binvarvals[j as usize], threshold) {
                                let start = (lastfit - jstart - 2) as usize;
                                clqvars[start] = binvars[j as usize];

                                // Add clique with at least two variables.
                                let len = (i - lastfit + 2) as usize;
                                let (infeasible, nbdchgs) = scip_var::add_clique(scip, &clqvars[start..start + len], &values[..len], false)?;

                                if infeasible {
                                    *cutoff = true;
                                    break;
                                }

                                *nchgbds += nbdchgs;

                                cliquenonzerosadded += i - lastfit + 2;
                                if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                                    stopped = true;
                                    break;
                                }

                                j -= 1;
                            } else {
                                lastfit += 1;
                            }
                        }
                    }
                }
            }

            // Did we find some boundchanges, then we need to remove fixings and
            // tighten the bounds further.
            if !stopped && !*cutoff && *nchgbds - oldnchgbds > 0 {
                extract_cliques_refresh(
                    scip,
                    cons,
                    maxeasyactivitydelta,
                    sortvars,
                    nfixedvars,
                    nchgbds,
                    cutoff,
                    &mut binvars,
                    &mut binvarvals,
                    &mut counts,
                    &mut nvars,
                )?;
            }

            // Case d).
            let nposbinvars = counts.0;
            if !stopped && !*cutoff && f.lhs && f.maxact && nposbinvars >= 2 {
                // Initialize clique values array for adding a negated clique.
                let values = vec![false; nposbinvars as usize];

                // Compute value that needs to be exceeded.
                let threshold = cd(scip, cons).lhs - cd(scip, cons).glbmaxactivity.to_dbl();

                let mut j: i32 = 1;

                if scip.set.is_feas_lt(-binvarvals[0] - binvarvals[j as usize], threshold) {
                    j += 1;
                    // Check for extending the clique.
                    while j < nposbinvars {
                        if !scip.set.is_feas_lt(-binvarvals[j as usize - 1] - binvarvals[j as usize], threshold) {
                            break;
                        }
                        j += 1;
                    }

                    // Add negated clique with at least two variables.
                    let (infeasible, nbdchgs) = scip_var::add_clique(scip, &binvars[..j as usize], &values[..j as usize], false)?;

                    if infeasible {
                        *cutoff = true;
                    }

                    *nchgbds += nbdchgs;

                    cliquenonzerosadded += j;
                    if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                        stopped = true;
                    }

                    // Exchange the last variable in the clique if possible and add
                    // all new ones.
                    if !stopped && !*cutoff && j < nposbinvars {
                        let mut lastfit = j - 2;

                        // Copy all 'main'-clique variables.
                        let mut clqvars = binvars[..j as usize].to_vec();

                        // Iterate up to the end with j and up to the front with
                        // lastfit, and check for different cliques.
                        while lastfit >= 0 && j < nposbinvars {
                            // Check if two variables are in a negated clique.
                            if scip.set.is_feas_lt(-binvarvals[lastfit as usize] - binvarvals[j as usize], threshold) {
                                clqvars[lastfit as usize + 1] = binvars[j as usize];

                                // Add clique with at least two variables.
                                let len = lastfit as usize + 2;
                                let (infeasible, nbdchgs) = scip_var::add_clique(scip, &clqvars[..len], &values[..len], false)?;

                                if infeasible {
                                    *cutoff = true;
                                    break;
                                }

                                *nchgbds += nbdchgs;

                                cliquenonzerosadded += lastfit + 2;
                                if cliquenonzerosadded >= MAX_CLIQUE_NONZEROS_PER_CONS {
                                    break;
                                }

                                j += 1;
                            } else {
                                lastfit -= 1;
                            }
                        }
                    }
                }
            }
        }

        if *cutoff {
            return Ok(());
        }
    }

    // 2. we only check if the constraint is a set packing / partitioning
    // constraint

    // Check if all variables are binary, if the coefficients are +1 or -1, and
    // if the right hand side is equal to 1 - number of negative coefficients,
    // or if the left hand side is equal to number of positive coefficients - 1.
    let mut nposcoefs = 0;
    let mut nnegcoefs = 0;
    for i in 0..nvars {
        let (var, val) = (cd(scip, cons).vars[i], cd(scip, cons).vals[i]);
        if !var_is_binary(scip, var) {
            return Ok(());
        } else if scip.set.is_eq(val, 1.0) {
            nposcoefs += 1;
        } else if scip.set.is_eq(val, -1.0) {
            nnegcoefs += 1;
        } else {
            return Ok(());
        }
    }

    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    let lhsclique = scip.set.is_eq(lhs, nposcoefs as f64 - 1.0);
    let rhsclique = scip.set.is_eq(rhs, 1.0 - nnegcoefs as f64);

    if lhsclique || rhsclique {
        let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
        let values: Vec<bool> = (0..nvars as usize).map(|i| rhsclique == (cd(scip, cons).vals[i] > 0.0)).collect();
        let isequation = scip.set.is_eq(lhs, rhs);
        let (infeasible, nbdchgs) = scip_var::add_clique(scip, &vars, &values, isequation)?;
        if infeasible {
            *cutoff = true;
        }
        *nchgbds += nbdchgs;
    }

    Ok(())
}

/// `tightenSides`: tightens left and right hand side of constraint due to
/// integrality; returns whether infeasibility was detected.
fn tighten_sides(scip: &mut Scip, cons: ConsId, nchgsides: &mut i32) -> ScipResult<bool> {
    let mut chglhs = false;
    let mut chgrhs = false;
    let mut newlhs = -scip.set.infinity();
    let mut newrhs = scip.set.infinity();

    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    if !scip.set.is_integral(lhs) || !scip.set.is_integral(rhs) {
        let mut integral = true;
        let mut i = 0;
        while i < cd(scip, cons).nvars && integral {
            integral = var_is_integral(scip, cd(scip, cons).vars[i]) && scip.set.is_integral(cd(scip, cons).vals[i]);
            i += 1;
        }
        if integral {
            if !scip.set.is_infinity(-lhs) && !scip.set.is_integral(lhs) {
                newlhs = scip.set.feas_ceil(lhs);
                chglhs = true;
            }
            if !scip.set.is_infinity(rhs) && !scip.set.is_integral(rhs) {
                newrhs = scip.set.feas_floor(rhs);
                chgrhs = true;
            }

            // Check whether rounding would lead to an unsatisfiable constraint.
            if scip.set.is_gt(newlhs, newrhs) {
                return Ok(true);
            }

            if chglhs {
                chg_lhs(scip, cons, newlhs)?;
                if !cd(scip, cons).upgraded {
                    *nchgsides += 1;
                }
            }
            if chgrhs {
                chg_rhs(scip, cons, newrhs)?;
                if !cd(scip, cons).upgraded {
                    *nchgsides += 1;
                }
            }
        }
    }

    Ok(false)
}

/// `consdataTightenCoefs`: tightens coefficients of binary, integer, and
/// implied integral variables due to activity bounds in presolving:
/// given an inequality  lhs <= a*x + ai*xi <= rhs, with a non-continuous
/// variable  li <= xi <= ui
/// let minact := min{a*x + ai*xi}, maxact := max{a*x + ai*xi}
/// (i) ai >= 0:
///    if  minact + ai >= lhs  and  maxact - ai <= rhs: (**)
///     - a deviation from the lower/upper bound of xi would make the
///       left/right hand side redundant
///     - ai, lhs and rhs can be changed to have the same redundancy effect
///       and the same results for xi fixed to its bounds, but with a reduced
///       ai and tightened sides to tighten the LP relaxation
///     - change coefficients:
///        ai'  := max(lhs - minact, maxact - rhs, 0)
///        lhs' := lhs - (ai - ai')*li
///        rhs' := rhs - (ai - ai')*ui
/// (ii) ai < 0:
///    if  minact - ai >= lhs  and  maxact + ai <= rhs: (***)
///     - a deviation from the upper/lower bound of xi would make the
///       left/right hand side redundant
///     - ai, lhs and rhs can be changed to have the same redundancy effect
///       and the same results for xi fixed to its bounds, but with a reduced
///       ai and tightened sides to tighten the LP relaxation
///     - change coefficients:
///        ai'  := min(rhs - maxact, minact - lhs, 0)
///        lhs' := lhs - (ai - ai')*ui
///        rhs' := rhs - (ai - ai')*li
///
/// We further try to remove variables from the constraint; variables which
/// fulfill conditions (**) or (***) are called relevant variables. A
/// deviation of only one from their bound makes the lhs/rhs feasible (i.e.,
/// redundant), even if all other variables are set to their "worst" bound.
/// If all variables which are not relevant cannot make the lhs/rhs
/// redundant, even if they are set to their "best" bound, they can be
/// removed from the constraint. E.g., for binary variables and an
/// inequality x_1 +x_2 +10y_1 +10y_2 >= 5, setting either of the y_i to one
/// suffices to fulfill the inequality, whereas the x_i do not contribute to
/// feasibility and can be removed.
fn consdata_tighten_coefs(scip: &mut Scip, cons: ConsId, nchgcoefs: &mut i32, nchgsides: &mut i32) -> ScipResult {
    use crate::def::{max3, min3};

    // Allocate relevance flags.
    let mut isvarrelevant = vec![false; cd(scip, cons).nvars as usize];

    // Get the minimal and maximal activity of the constraint.
    let bounds = |scip: &mut Scip| {
        let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_bounds(scip, c, true));
        (min.act, max.act, min.istight, max.istight, min.issettoinfinity, max.issettoinfinity)
    };
    let (mut minactivity, mut maxactivity, mut isminacttight, mut ismaxacttight, mut isminsettoinfinity, mut ismaxsettoinfinity) = bounds(scip);

    // Minimal and maximal activity without relevant variables.
    let mut minleftactivity = 0.0;
    let mut maxleftactivity = 0.0;

    // Try to tighten each coefficient.
    let mut i: i32 = 0;
    while i < cd(scip, cons).nvars {
        // Get coefficient and variable's bounds.
        let var = cd(scip, cons).vars[i];
        let val = cd(scip, cons).vals[i];
        let lb = scip.vars[var].locdom.lb;
        let ub = scip.vars[var].locdom.ub;
        let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

        if (i as usize) >= isvarrelevant.len() {
            isvarrelevant.resize(i as usize + 1, false);
        }

        // Check sign of coefficient.
        if val >= 0.0 {
            // Check, if a deviation from lower/upper bound would make lhs/rhs
            // redundant.
            isvarrelevant[i as usize] = var_is_integral(scip, var) && scip.set.is_ge(minactivity + val, lhs) && scip.set.is_le(maxactivity - val, rhs);

            if isvarrelevant[i as usize] {
                // Change coefficients:
                //  ai'  := max(lhs - minact, maxact - rhs)
                //  lhs' := lhs - (ai - ai')*li
                //  rhs' := rhs - (ai - ai')*ui
                let mut lval = lhs - minactivity;
                let mut rval = maxactivity - rhs;

                // Try to avoid cancellation, if there are only two variables.
                if cd(scip, cons).nvars == 2 {
                    let othervar = cd(scip, cons).vars[1 - i];
                    let otherval = cd(scip, cons).vals[1 - i];
                    let (olb, oub) = (scip.vars[othervar].locdom.lb, scip.vars[othervar].locdom.ub);

                    if !scip.set.is_infinity(-lhs) && !isminsettoinfinity {
                        lval = lhs - val * lb;
                        lval -= if otherval > 0.0 { otherval * olb } else { otherval * oub };
                    }

                    if !scip.set.is_infinity(rhs) && !ismaxsettoinfinity {
                        rval = val * ub - rhs;
                        rval += if otherval > 0.0 { otherval * oub } else { otherval * olb };
                    }
                }

                let newval = max3(lval, rval, 0.0);

                // Try to avoid cancellation in computation of lhs/rhs.
                let mut newlhs = lhs - val * lb;
                newlhs += newval * lb;
                let mut newrhs = rhs - val * ub;
                newrhs += newval * ub;

                if !scip.set.is_sum_rel_eq(newval, val) {
                    // Update the coefficient and the activity bounds.
                    if scip.set.is_zero(newval) {
                        del_coef_pos(scip, cons, i)?;
                        i -= 1;
                    } else {
                        chg_coef_pos(scip, cons, i, newval)?;
                    }
                    *nchgcoefs += 1;

                    // Get the new minimal and maximal activity of the constraint.
                    (minactivity, maxactivity, isminacttight, ismaxacttight, isminsettoinfinity, ismaxsettoinfinity) = bounds(scip);

                    let lhs = cd(scip, cons).lhs;
                    if !scip.set.is_infinity(-lhs) && !scip.set.is_eq(newlhs, lhs) {
                        chg_lhs(scip, cons, newlhs)?;
                        *nchgsides += 1;
                    }

                    let rhs = cd(scip, cons).rhs;
                    if !scip.set.is_infinity(rhs) && !scip.set.is_eq(newrhs, rhs) {
                        chg_rhs(scip, cons, newrhs)?;
                        *nchgsides += 1;
                    }
                }
            } else {
                if !scip.set.is_infinity(-minleftactivity) {
                    if scip.set.is_infinity(-lb) {
                        minleftactivity = -scip.set.infinity();
                    } else {
                        minleftactivity += val * lb;
                    }
                }

                if !scip.set.is_infinity(maxleftactivity) {
                    if scip.set.is_infinity(ub) {
                        maxleftactivity = scip.set.infinity();
                    } else {
                        maxleftactivity += val * ub;
                    }
                }
            }
        } else {
            // Check, if a deviation from lower/upper bound would make lhs/rhs
            // redundant.
            isvarrelevant[i as usize] = var_is_integral(scip, var) && scip.set.is_ge(minactivity - val, lhs) && scip.set.is_le(maxactivity + val, rhs);

            if isvarrelevant[i as usize] {
                // Change coefficients:
                //  ai'  := min(rhs - maxact, minact - lhs)
                //  lhs' := lhs - (ai - ai')*ui
                //  rhs' := rhs - (ai - ai')*li
                let mut lval = minactivity - lhs;
                let mut rval = rhs - maxactivity;

                // Try to avoid cancellation, if there are only two variables.
                if cd(scip, cons).nvars == 2 {
                    let othervar = cd(scip, cons).vars[1 - i];
                    let otherval = cd(scip, cons).vals[1 - i];
                    let (olb, oub) = (scip.vars[othervar].locdom.lb, scip.vars[othervar].locdom.ub);

                    if !scip.set.is_infinity(-lhs) && !isminsettoinfinity {
                        lval = val * ub - lhs;
                        lval += if otherval > 0.0 { otherval * olb } else { otherval * oub };
                    }

                    if !scip.set.is_infinity(rhs) && !ismaxsettoinfinity {
                        rval = rhs - val * lb;
                        rval -= if otherval > 0.0 { otherval * oub } else { otherval * olb };
                    }
                }

                let newval = min3(lval, rval, 0.0);

                // Try to avoid cancellation in computation of lhs/rhs.
                let mut newlhs = lhs - val * ub;
                newlhs += newval * ub;
                let mut newrhs = rhs - val * lb;
                newrhs += newval * lb;

                if !scip.set.is_sum_rel_eq(newval, val) {
                    // Update the coefficient and the activity bounds.
                    if scip.set.is_zero(newval) {
                        del_coef_pos(scip, cons, i)?;
                        i -= 1;
                    } else {
                        chg_coef_pos(scip, cons, i, newval)?;
                    }
                    *nchgcoefs += 1;

                    // Get the new minimal and maximal activity of the constraint.
                    (minactivity, maxactivity, isminacttight, ismaxacttight, isminsettoinfinity, ismaxsettoinfinity) = bounds(scip);

                    let lhs = cd(scip, cons).lhs;
                    if !scip.set.is_infinity(-lhs) && !scip.set.is_eq(newlhs, lhs) {
                        chg_lhs(scip, cons, newlhs)?;
                        *nchgsides += 1;
                    }

                    let rhs = cd(scip, cons).rhs;
                    if !scip.set.is_infinity(rhs) && !scip.set.is_eq(newrhs, rhs) {
                        chg_rhs(scip, cons, newrhs)?;
                        *nchgsides += 1;
                    }
                }
            } else {
                if !scip.set.is_infinity(-minleftactivity) {
                    if scip.set.is_infinity(ub) {
                        minleftactivity = -scip.set.infinity();
                    } else {
                        minleftactivity += val * ub;
                    }
                }

                if !scip.set.is_infinity(maxleftactivity) {
                    if scip.set.is_infinity(-lb) {
                        maxleftactivity = scip.set.infinity();
                    } else {
                        maxleftactivity += val * lb;
                    }
                }
            }
        }
        i += 1;
    }

    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

    // If the lhs is finite, we will check in the following whether the not
    // relevant variables can make lhs feasible; this is not valid, if the
    // minactivity is -\infty (aggrlhs would be minus infinity in the following
    // computation) or if huge values contributed to the minactivity, because
    // the minactivity is then just a relaxation (<= the exact minactivity),
    // and we might falsely remove coefficients in the following.
    if !scip.set.is_infinity(-lhs) && (scip.set.is_infinity(-minactivity) || !isminacttight) {
        return Ok(());
    }

    // If the rhs is finite, we will check in the following whether the not
    // relevant variables can make rhs feasible; this is not valid, if the
    // maxactivity is \infty (aggrrhs would be infinity in the following
    // computation) or if huge values contributed to the maxactivity, because
    // the maxactivity is then just a relaxation (>= the exact maxactivity),
    // and we might falsely remove coefficients in the following.
    if !scip.set.is_infinity(rhs) && (scip.set.is_infinity(maxactivity) || !ismaxacttight) {
        return Ok(());
    }

    // Correct lhs and rhs by min/max activity of relevant variables; relevant
    // variables are all those where a deviation from the bound makes the
    // lhs/rhs redundant.
    let aggrlhs = lhs - minactivity + minleftactivity;
    let aggrrhs = rhs - maxactivity + maxleftactivity;

    // Check if the constraint contains variables whose coefficient can be
    // removed. The reasoning is the following: Each relevant variable can make
    // the lhs/rhs feasible with a deviation of only one in the bound. If _all_
    // not relevant variables together cannot make lhs/rhs redundant, they can
    // be removed from the constraint. aggrrhs may contain some near-infinity
    // value, but only if rhs is infinity.
    if (scip.set.is_infinity(-lhs) || scip.set.is_feas_lt(maxleftactivity, aggrlhs)) && (scip.set.is_infinity(rhs) || scip.set.is_feas_gt(minleftactivity, aggrrhs)) {
        // Remove redundant variables from constraint.
        let mut i: i32 = 0;
        while i < cd(scip, cons).nvars {
            // Consider redundant variable.
            if !isvarrelevant[i as usize] {
                // Get coefficient and variable's bounds.
                let var = cd(scip, cons).vars[i];
                let val = cd(scip, cons).vals[i];
                let lb = scip.vars[var].locdom.lb;
                let ub = scip.vars[var].locdom.ub;

                // Check sign of coefficient.
                let (minleftactivitypart, maxleftactivitypart) = if val >= 0.0 { (val * lb, val * ub) } else { (val * ub, val * lb) };

                // Remove redundant variable.
                isvarrelevant[i as usize] = isvarrelevant[cd(scip, cons).nvars as usize - 1];
                del_coef_pos(scip, cons, i)?;
                i -= 1;

                // Adjust lhs and right hand side.
                let newlhs = cd(scip, cons).lhs - minleftactivitypart;
                let newrhs = cd(scip, cons).rhs - maxleftactivitypart;

                let lhs = cd(scip, cons).lhs;
                if !scip.set.is_infinity(-lhs) && !scip.set.is_eq(newlhs, lhs) {
                    chg_lhs(scip, cons, newlhs)?;
                    *nchgsides += 1;
                }

                let rhs = cd(scip, cons).rhs;
                if !scip.set.is_infinity(rhs) && !scip.set.is_eq(newrhs, rhs) {
                    chg_rhs(scip, cons, newrhs)?;
                    *nchgsides += 1;
                }
            }
            i += 1;
        }
    }

    Ok(())
}

/// `convertUnaryEquality`: processes equality with only one variable by
/// fixing the variable and deleting the constraint.
fn convert_unary_equality(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32, ndelconss: &mut i32) -> ScipResult {
    // Calculate the value to fix the variable to.
    let var = cd(scip, cons).vars[0usize];
    let val = cd(scip, cons).vals[0usize];
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    let fixval = crate::misc::select_simple_value(lhs / val - 0.9 * scip.set.epsilon(), rhs / val + 0.9 * scip.set.epsilon(), MAXDNOM);

    // Fix variable.
    let (infeasible, fixed) = scip_var::fix_var(scip, var, fixval)?;
    if infeasible {
        *cutoff = true;
        return Ok(());
    }
    if fixed {
        *nfixedvars += 1;
    }

    // Disable constraint.
    scip_cons::del_cons(scip, cons)?;
    if !cd(scip, cons).upgraded {
        *ndelconss += 1;
    }

    Ok(())
}

/// `convertBinaryEquality`: processes equality with exactly two variables
/// by aggregating one of the variables and deleting the constraint.
fn convert_binary_equality(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, naggrvars: &mut i32, ndelconss: &mut i32) -> ScipResult {
    let c = cd(scip, cons);
    let (x, y, a, b, rhs) = (c.vars[0usize], c.vars[1usize], c.vals[0usize], c.vals[1usize], c.rhs);

    // Aggregate the equality.
    let (infeasible, redundant, aggregated) = scip_var::aggregate_vars(scip, x, y, a, b, rhs)?;

    // Check for infeasibility of aggregation.
    if infeasible {
        *cutoff = true;
        return Ok(());
    }

    // Count the aggregation.
    if aggregated {
        *naggrvars += 1;
    }

    // Delete the constraint, if it is redundant.
    if redundant {
        scip_cons::del_cons(scip, cons)?;

        if !cd(scip, cons).upgraded {
            *ndelconss += 1;
        }
    }

    Ok(())
}

/// `getNewSidesAfterAggregation`: calculates the new lhs and rhs of the
/// constraint after the given variable is aggregated out; returns
/// `(newlhs, newrhs)`.
fn get_new_sides_after_aggregation(scip: &Scip, c: &ConsData, slackvar: VarId, slackcoef: f64) -> (f64, f64) {
    let slackvarlb = scip.vars[slackvar].glbdom.lb;
    let slackvarub = scip.vars[slackvar].glbdom.ub;
    let newlhs;
    let newrhs;
    if slackcoef > 0.0 {
        newrhs = if scip.set.is_infinity(-slackvarlb) {
            scip.set.infinity()
        } else {
            c.rhs - slackcoef * slackvarlb
        };
        newlhs = if scip.set.is_infinity(slackvarub) {
            -scip.set.infinity()
        } else {
            c.lhs - slackcoef * slackvarub
        };
    } else {
        newlhs = if scip.set.is_infinity(-slackvarlb) {
            -scip.set.infinity()
        } else {
            c.rhs - slackcoef * slackvarlb
        };
        newrhs = if scip.set.is_infinity(slackvarub) {
            scip.set.infinity()
        } else {
            c.lhs - slackcoef * slackvarub
        };
    }
    (newlhs, newrhs)
}

/// `convertLongEquality`: processes equality with more than two variables
/// by multi-aggregating one of the variables and converting the equality
/// into an inequality; if multi-aggregation is not possible, tries to
/// identify one continuous or integer variable that is implied integral by
/// this constraint.
#[allow(clippy::too_many_arguments)]
fn convert_long_equality(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, naggrvars: &mut i32, ndelconss: &mut i32, nchgvartypes: &mut i32) -> ScipResult {
    let conshdlr = scip.conss[cons].conshdlr;
    let maxmultaggrquot = hd(scip, conshdlr).maxmultaggrquot;
    let multaggrremove = hd(scip, conshdlr).multaggrremove;

    // We do not want to increase the total number of non-zeros due to the
    // multi-aggregation. Therefore, we have to restrict the number of locks of
    // a variable that is aggregated out.
    //  maxnlocksstay:  maximal sum of lock numbers if the constraint does not
    //                  become redundant after the aggregation
    //  maxnlocksremove: maximal sum of lock numbers if the constraint can be
    //                  deleted after the aggregation
    let lhs = cd(scip, cons).lhs;
    let rhs = cd(scip, cons).rhs;
    let nvars0 = cd(scip, cons).nvars;
    let mut maxnlocksstay = 0;
    let mut maxnlocksremove = if nvars0 == 3 {
        // If the constraint becomes redundant, 3 non-zeros are removed, and we
        // get 1 additional non-zero for each constraint the variable appears
        // in. Thus, the variable must appear in at most 3 other constraints.
        3
    } else if nvars0 == 4 {
        // If the constraint becomes redundant, 4 non-zeros are removed, and we
        // get 2 additional non-zeros for each constraint the variable appears
        // in. Thus, the variable must appear in at most 2 other constraints.
        2
    } else {
        // If the constraint is redundant but has more than 4 variables, we can
        // only accept one other constraint.
        1
    };

    // The locks on this constraint can be ignored.
    if scip.conss[cons].is_checked() {
        if !scip.set.is_infinity(-lhs) {
            maxnlocksstay += 1;
            maxnlocksremove += 1;
        }
        if !scip.set.is_infinity(rhs) {
            maxnlocksstay += 1;
            maxnlocksremove += 1;
        }
    }

    // Look for a slack variable s to convert a*x + s == b into
    // lhs <= a*x <= rhs.
    let mut bestslackpos: i32 = -1;
    let mut bestslacktype = VarType::Binary;
    let mut bestnlocks = i32::MAX;
    let mut bestremovescons = false;
    let mut bestslackdomrng = 0.0;
    let mut coefszeroone = true;
    let mut coefsintegral = true;
    let mut varsintegral = true;
    let mut ncontvars = 0;
    let mut contvarpos = -1;
    let mut nintvars = 0;
    let mut nweakimplvars = 0;
    let mut nimplvars = 0;
    let mut intvarpos = -1;
    let mut minabsval = scip.set.infinity();
    let mut maxabsval = -1.0;
    for v in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];
        let varlb = scip.vars[var].glbdom.lb;
        let varub = scip.vars[var].glbdom.ub;
        let val = cd(scip, cons).vals[v];
        let absval = val.abs();

        // Calculate minimal and maximal absolute value.
        if absval < minabsval {
            minabsval = absval;
        }
        if absval > maxabsval {
            maxabsval = absval;
        }

        // Do not try to multi aggregate, when numerical bad.
        if maxabsval / minabsval > maxmultaggrquot {
            return Ok(());
        }

        let impltype = scip.vars[var].varimpltype;
        let slacktype = if impltype != ImplintType::None { VarType::ImplInt } else { scip.vars[var].vartype };

        coefszeroone = coefszeroone && scip.set.is_eq(absval, 1.0);
        coefsintegral = coefsintegral && scip.set.is_integral(val);
        varsintegral = varsintegral && slacktype != VarType::Continuous;
        let iscont = slacktype == VarType::Continuous || slacktype == VarType::ImplInt;

        // Update candidates for continuous -> implint and integer -> implint
        // conversion.
        if slacktype == VarType::Continuous {
            ncontvars += 1;
            contvarpos = v;
        } else if slacktype == VarType::ImplInt {
            nimplvars += 1;
            if impltype == ImplintType::Weak {
                nweakimplvars += 1;
            }
        } else if slacktype == VarType::Integer {
            nintvars += 1;
            intvarpos = v;
        }

        // Check, if variable is already fixed or aggregated.
        if !crate::var::var_is_active(scip, var) {
            continue;
        }

        // Check, if variable is used in too many other constraints, even if this
        // constraint could be deleted.
        let nlocks = crate::var::var_get_n_locks_down_type(scip, var, LockType::Model) + crate::var::var_get_n_locks_up_type(scip, var, LockType::Model);

        if nlocks > maxnlocksremove {
            continue;
        }

        // Check, if variable can be used as a slack variable.
        if (iscont || (coefsintegral && varsintegral && scip.set.is_eq(absval, 1.0))) && !scip_var::do_not_multaggr_var(scip, var) {
            let slackdomrng = if scip.set.is_infinity(varub) || scip.set.is_infinity(-varlb) {
                scip.set.infinity()
            }
            // We do not want to perform multi-aggregation due to numerics, if the
            // bounds are huge.
            else if scip.set.is_huge_value(varub) || scip.set.is_huge_value(-varlb) {
                return Ok(());
            } else {
                (varub - varlb) * absval
            };
            let mut equal = false;

            // Continuous > implied > integer > binary.
            let mut better = slacktype > bestslacktype || bestslackpos == -1;
            if !better && slacktype == bestslacktype {
                better = nlocks < bestnlocks;
                if nlocks == bestnlocks && !bestremovescons {
                    better = scip.set.is_gt(slackdomrng, bestslackdomrng);
                    equal = !better && scip.set.is_ge(slackdomrng, bestslackdomrng);
                }
            }

            if better || equal {
                // Check if the constraint becomes redundant after
                // multi-aggregation.
                let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_residuals(scip, c, var, val, false));
                let (mut minresactivity, mut maxresactivity) = (min.act, max.act);

                // Do not perform the multi-aggregation due to numerics, if we have
                // huge contributions in the residual activity.
                if !min.istight || !max.istight {
                    continue;
                }

                let (newlhs, newrhs) = get_new_sides_after_aggregation(scip, cd(scip, cons), var, val);
                let mut removescons = scip.set.is_feas_le(newlhs, minresactivity) && scip.set.is_feas_le(maxresactivity, newrhs);

                // Check resactivities for reliability.
                if removescons {
                    let c = cd(scip, cons);
                    if !min.issettoinfinity && scip.set.is_update_unreliable(minresactivity, c.lastminactivity) {
                        minresactivity = consdata_get_reliable_residual_activity(scip, c, var, true, false);
                    }

                    if !max.issettoinfinity && scip.set.is_update_unreliable(maxresactivity, c.lastmaxactivity) && scip.set.is_feas_le(newlhs, minresactivity) {
                        maxresactivity = consdata_get_reliable_residual_activity(scip, c, var, false, false);
                    }

                    removescons = scip.set.is_feas_le(newlhs, minresactivity) && scip.set.is_feas_le(maxresactivity, newrhs);
                }

                // If parameter multaggrremove is set to TRUE, only aggregate when
                // this removes constraint.
                if multaggrremove && !removescons {
                    continue;
                }

                // If the constraint does not become redundant, only accept the
                // variable if it does not appear in other constraints.
                if !removescons && nlocks > maxnlocksstay {
                    continue;
                }

                // Prefer variables that make the constraints redundant unless
                // there is a continuous better slack.
                if !bestremovescons && removescons {
                    better = true;
                } else if bestremovescons && !removescons && (bestslacktype > VarType::Integer || slacktype <= VarType::Integer) {
                    better = false;
                }

                if better {
                    bestslackpos = v;
                    bestslacktype = slacktype;
                    bestnlocks = nlocks;
                    bestslackdomrng = slackdomrng;
                    bestremovescons = removescons;
                }
            }
        }
    }

    // If all coefficients and variables are integral, the right hand side must
    // also be integral.
    if coefsintegral && varsintegral && !scip.set.is_feas_integral(cd(scip, cons).rhs) {
        *cutoff = true;
        return Ok(());
    }

    // If the slack variable is of integer type, and the constraint itself may
    // take fractional values, we cannot aggregate the variable, because the
    // integrality condition would get lost. Similarly, if there are implied
    // integral variables, we cannot aggregate since we might loose the
    // integrality condition for this variable.
    if bestslackpos >= 0 && (bestslacktype == VarType::Continuous || bestslacktype == VarType::ImplInt || (coefsintegral && varsintegral && nimplvars == 0)) {
        // We found a slack variable that only occurs in at most one other
        // constraint:
        //  a_1*x_1 + ... + a_k*x_k + a'*s == rhs  ->  s == rhs - a_1/a'*x_1 - ... - a_k/a'*x_k

        // Do not multi aggregate binary variables.
        if var_is_binary(scip, cd(scip, cons).vars[bestslackpos]) {
            return Ok(());
        }

        // Convert equality into inequality by deleting the slack variable:
        //  x + a*s == b, l <= s <= u  ->  b - a*u <= x <= b - a*l
        let c = cd(scip, cons);
        let slackvar = c.vars[bestslackpos];
        let slackcoef = c.vals[bestslackpos];
        let aggrconst = c.rhs / slackcoef;
        let n = c.nvars as usize;

        // Set up the multi-aggregation.
        let mut aggrvars = vec![NONE; n - 1];
        let mut scalars = vec![0.0; n - 1];
        for v in 0..n - 1 {
            if v as i32 == bestslackpos {
                aggrvars[v] = c.vars[n - 1];
                scalars[v] = -c.vals[n - 1] / slackcoef;
            } else {
                aggrvars[v] = c.vars[v];
                scalars[v] = -c.vals[v] / slackcoef;
            }
        }

        // Perform the multi-aggregation.
        let (infeasible, aggregated) = scip_var::multiaggregate_var(scip, slackvar, &aggrvars, &scalars, aggrconst)?;

        // Check for infeasible aggregation.
        if infeasible {
            *cutoff = true;
            return Ok(());
        }

        // Check for applied aggregation.
        if !aggregated {
            return Ok(());
        }

        *naggrvars += 1;

        let (newlhs, newrhs) = get_new_sides_after_aggregation(scip, cd(scip, cons), slackvar, slackcoef);
        chg_lhs(scip, cons, newlhs)?;
        chg_rhs(scip, cons, newrhs)?;
        del_coef_pos(scip, cons, bestslackpos)?;

        // Delete the constraint if it became redundant.
        if bestremovescons {
            scip_cons::del_cons(scip, cons)?;

            if !cd(scip, cons).upgraded {
                *ndelconss += 1;
            }
        }
    } else if ncontvars == 1 {
        let var = cd(scip, cons).vars[contvarpos];
        let contval = cd(scip, cons).vals[contvarpos];

        if coefsintegral && scip.set.is_feas_integral(cd(scip, cons).rhs) {
            // Upgrade continuous variable to an implied integral one, if the
            // absolute value of the coefficient is one.
            if scip.set.is_eq(contval.abs(), 1.0) {
                // Convert the continuous variable with coefficient 1.0 into an
                // implied integral variable; if the integrality does not depend
                // on weak implied integrality, the variable becomes strongly
                // implied integral.
                let impltype = if nweakimplvars == 0 { ImplintType::Strong } else { ImplintType::Weak };
                let infeasible = scip_var::chg_var_impl_type(scip, var, impltype)?;
                *nchgvartypes += 1;
                if infeasible {
                    *cutoff = true;
                    return Ok(());
                }
            }
            // Aggregate continuous variable to an implied integral one if the
            // absolute coefficient is unequal to one.
            else if !scip_var::do_not_aggr(scip) {
                let absval = contval.abs();
                let newvarname = format!("{}_impl", scip.vars[var].name);
                let (initial, removable) = (scip.vars[var].initial, scip.vars[var].removable);

                // Create new implied integral variable for aggregation.
                let inf = scip.set.infinity();
                let mut newvar = scip_var::create_var_impl(scip, &newvarname, -inf, inf, 0.0, VarType::Continuous, ImplintType::Weak, initial, removable)?;

                // Add new variable to problem.
                scip_var::add_var(scip, newvar)?;

                // Aggregate continuous and implied integral variable.
                let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, var, newvar, absval, -1.0, 0.0)?;

                if infeasible {
                    *cutoff = true;

                    // Release implied integral variable.
                    api::release_var(scip, &mut newvar)?;
                    return Ok(());
                }

                // Release implied integral variable.
                api::release_var(scip, &mut newvar)?;

                if aggregated {
                    *naggrvars += 1;
                } else {
                    return Ok(());
                }
            }

            // We do not have any event on vartype changes, so we need to
            // manually force this constraint to be presolved again.
            let c = cdm(scip, cons);
            c.boundstightened = 0;
            c.rangedrowpropagated = 0;
            c.presolved = false;
        }
    } else if ncontvars == 0 && nimplvars == 0 && nintvars == 1 && !coefszeroone {
        // This seems to help for rococo instances, but does not for rout (where
        // all coefficients are +/- 1.0) -> we don't convert integers into
        // implints if the row is a 0/1-row.
        let var = cd(scip, cons).vars[intvarpos];
        let intval = cd(scip, cons).vals[intvarpos];

        if coefsintegral && scip.set.is_eq(intval.abs(), 1.0) && scip.set.is_feas_integral(cd(scip, cons).rhs) {
            // Convert the integer variable with coefficient 1.0 into an implied
            // integral variable.
            let infeasible = scip_var::chg_var_impl_type(scip, var, ImplintType::Strong)?;
            *nchgvartypes += 1;
            if infeasible {
                *cutoff = true;
                return Ok(());
            }
        }
    }

    Ok(())
}

/// `checkEqualObjective`: checks if the given variables and their
/// coefficient are equal (w.r.t. scaling factor) to the objective function;
/// updates the scaling factor between the constraint and the objective
/// function and the offset of the objective function resulting by this
/// constraint.
fn check_equal_objective(scip: &Scip, c: &ConsData, scale: &mut f64, offset: &mut f64) -> bool {
    for v in 0..c.nvars {
        let mut negated = false;
        let mut var = c.vars[v];

        if crate::var::var_is_negated(scip, var) {
            negated = true;
            var = scip.vars[var].negatedvar;
        }

        let objval = scip.vars[var].obj;

        // If a variable has a zero objective coefficient the linear constraint is
        // not a subset of the objective function.
        if scip.set.is_zero(objval) {
            return false;
        } else {
            let val = c.vals[v];

            if negated {
                if v == 0 {
                    // The first variable defines the scale.
                    *scale = val / -objval;

                    *offset += val;
                } else if scip.set.is_eq(-objval * *scale, val) {
                    *offset += val;
                } else {
                    return false;
                }
            } else if v == 0 {
                // The first variable defines the scale.
                *scale = val / objval;
            } else if !scip.set.is_eq(objval * *scale, val) {
                return false;
            }
        }
    }

    true
}

/// `checkPartialObjective`: check if the linear equality constraint is
/// equal to a subset of the objective function; if so we can remove the
/// objective coefficients and add an objective offset.
fn check_partial_objective(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let conshdlr = scip.conss[cons].conshdlr;
    let nvars = cd(scip, cons).nvars;
    let nobjvars = api::get_n_obj_vars(scip);

    // Check if the linear equality constraints does not have more variables
    // than the objective function.
    if nvars > nobjvars || nvars == 0 {
        return Ok(());
    }

    // Check for allowance of algorithm.
    let h = hd(scip, conshdlr);
    if (nvars < nobjvars && !h.detectpartialobjective) || (nvars == nobjvars && (!h.detectcutoffbound || !h.detectlowerbound)) {
        return Ok(());
    }

    let mut offset = cd(scip, cons).rhs;
    let mut scale = 1.0;

    // Checks if the variables and their coefficients are equal (w.r.t. scaling
    // factor) to the objective function.
    let applicable = check_equal_objective(scip, cd(scip, cons), &mut scale, &mut offset);

    if applicable {
        let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();

        offset /= scale;

        // Make equality a model constraint to ensure optimality in this
        // direction.
        scip_cons::set_cons_checked(scip, cons, true)?;
        scip_cons::set_cons_enforced(scip, cons, true);

        // Set all objective coefficient to zero.
        for var in vars {
            scip_var::chg_var_obj(scip, var, 0.0)?;
        }

        // Add an objective offset.
        api::add_objoffset(scip, offset)?;
    }

    Ok(())
}

/// `updateCutoffbound`: updates the cutoff if the given primal bound (which
/// is implied by the given constraint) is better.
fn update_cutoffbound(scip: &mut Scip, cons: ConsId, primalbound: f64) -> ScipResult {
    // Increase the cutoff bound value by an epsilon to ensue that solution with
    // the value of the cutoff bound are still accepted.
    let cutoffbound = primalbound + scip.set.cutoffbounddelta();

    if cutoffbound < api::get_cutoffbound(scip) {
        api::update_cutoffbound(scip, cutoffbound)?;
    } else {
        // We cannot disable the enforcement and propagation on ranged rows,
        // because the cutoffbound could only have resulted from one side.
        let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
        if scip.set.is_infinity(-lhs) || scip.set.is_infinity(rhs) {
            // In case the cutoff bound is worse then the currently known one, we
            // additionally avoid enforcement and propagation.
            scip_cons::set_cons_enforced(scip, cons, false);
            scip_cons::set_cons_propagated(scip, cons, false);
        }
    }

    Ok(())
}

/// `checkParallelObjective`: check if the linear constraint is parallel to
/// objective function; if so update the cutoff bound and avoid that the
/// constraint enters the LP by setting the initial and separated flag to
/// FALSE.
fn check_parallel_objective(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let conshdlr = scip.conss[cons].conshdlr;
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

    // Ignore equalities since these are covered by the method
    // checkPartialObjective().
    if scip.set.is_eq(lhs, rhs) {
        return Ok(());
    }

    let nvars = cd(scip, cons).nvars;
    let nobjvars = api::get_n_obj_vars(scip);

    // Check if the linear inequality constraints has the same number of
    // variables as the objective function and if the initial and/or separated
    // flag is set to FALSE.
    if nvars != nobjvars || (!scip.conss[cons].is_initial() && !scip.conss[cons].is_separated()) {
        return Ok(());
    }

    let mut offset = 0.0;
    let mut scale = 1.0;

    // There are no variables in the objective function and in the constraint.
    // Thus, the constraint is redundant or proves infeasibility. Since we have
    // a pure feasibility problem, we do not want to set a cutoff or lower
    // bound.
    if nobjvars == 0 {
        return Ok(());
    }

    // Checks if the variables and their coefficients are equal (w.r.t. scaling
    // factor) to the objective function.
    let applicable = check_equal_objective(scip, cd(scip, cons), &mut scale, &mut offset);

    if applicable {
        let rhsfinite = !scip.set.is_infinity(rhs);
        let lhsfinite = !scip.set.is_infinity(-lhs);
        let detectcutoffbound = hd(scip, conshdlr).detectcutoffbound;
        let detectlowerbound = hd(scip, conshdlr).detectlowerbound;

        if scale > 0.0 {
            if detectcutoffbound && rhsfinite {
                let primalbound = (rhs - offset) / scale;
                update_cutoffbound(scip, cons, primalbound)?;
            }

            if detectlowerbound && lhsfinite {
                let lowerbound = (lhs - offset) / scale;
                api::update_local_lowerbound(scip, lowerbound)?;
            }

            if (detectcutoffbound && (detectlowerbound || !lhsfinite)) || (detectlowerbound && !rhsfinite) {
                // Avoid that the linear constraint enters the LP since it is
                // parallel to the objective function.
                scip_cons::set_cons_initial(scip, cons, false);
                scip_cons::set_cons_separated(scip, cons, false);
            }
        } else {
            if detectlowerbound && rhsfinite {
                let lowerbound = (rhs - offset) / scale;
                api::update_local_lowerbound(scip, lowerbound)?;
            }

            if detectcutoffbound && lhsfinite {
                let primalbound = (lhs - offset) / scale;
                update_cutoffbound(scip, cons, primalbound)?;
            }

            if (detectcutoffbound && (detectlowerbound || !rhsfinite)) || (detectlowerbound && !lhsfinite) {
                // Avoid that the linear constraint enters the LP since it is
                // parallel to the objective function.
                scip_cons::set_cons_initial(scip, cons, false);
                scip_cons::set_cons_separated(scip, cons, false);
            }
        }
    }

    Ok(())
}

/// `convertEquality`: converts special equalities.
#[allow(clippy::too_many_arguments)]
fn convert_equality(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32, naggrvars: &mut i32, ndelconss: &mut i32, nchgvartypes: &mut i32) -> ScipResult {
    // Do nothing on inequalities.
    if !scip.set.is_eq(cd(scip, cons).lhs, cd(scip, cons).rhs) {
        return Ok(());
    }

    // Depending on the number of variables, call a special conversion method.
    let nvars = cd(scip, cons).nvars;
    if nvars == 1 {
        // Fix variable.
        convert_unary_equality(scip, cons, cutoff, nfixedvars, ndelconss)?;
    } else if nvars == 2 {
        // Aggregate one of the variables.
        convert_binary_equality(scip, cons, cutoff, naggrvars, ndelconss)?;
    } else {
        // Check if the equality is part of the objective function.
        check_partial_objective(scip, cons)?;

        // Try to multi-aggregate one of the variables.
        convert_long_equality(scip, cons, cutoff, naggrvars, ndelconss, nchgvartypes)?;
    }

    Ok(())
}

/// `consdataIsResidualIntegral`: returns whether the linear sum of all
/// variables/coefficients except the given one divided by the given value
/// is always integral.
fn consdata_is_residual_integral(scip: &Scip, c: &ConsData, pos: i32, val: f64) -> bool {
    for v in 0..c.nvars {
        if v != pos && (!var_is_integral(scip, c.vars[v]) || !scip.set.is_integral(c.vals[v] / val)) {
            return false;
        }
    }
    true
}

/// `calculateMinvalAndMaxval`: check if lhs/a_i - \sum_{j \neq i} a_j/a_i
/// x_j is always inside the bounds of x_i, check if rhs/a_i - \sum_{j \neq
/// i} a_j/a_i x_j is always inside the bounds of x_i; returns `(minval,
/// maxval)`.
fn calculate_minval_and_maxval(scip: &Scip, side: f64, val: f64, minresactivity: f64, maxresactivity: f64) -> (f64, f64) {
    if val > 0.0 {
        let minval = if scip.set.is_infinity(maxresactivity.abs()) {
            -maxresactivity
        } else {
            (side - maxresactivity) / val
        };
        let maxval = if scip.set.is_infinity(minresactivity.abs()) {
            -minresactivity
        } else {
            (side - minresactivity) / val
        };
        (minval, maxval)
    } else {
        let minval = if scip.set.is_infinity(minresactivity.abs()) {
            minresactivity
        } else {
            (side - minresactivity) / val
        };
        let maxval = if scip.set.is_infinity(maxresactivity.abs()) {
            maxresactivity
        } else {
            (side - maxresactivity) / val
        };
        (minval, maxval)
    }
}

/// `dualPresolve`: applies dual presolving for variables that are locked
/// only once in a direction, and this locking is due to a linear
/// inequality.
#[allow(clippy::too_many_arguments)]
fn dual_presolve(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, _nfixedvars: &mut i32, naggrvars: &mut i32, ndelconss: &mut i32, nchgvartypes: &mut i32) -> ScipResult {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};

    // Only process checked constraints (for which the locks are increased);
    // otherwise we would have to check for variables with nlocks == 0, and
    // these are already processed by the dualfix presolver.
    if !scip.conss[cons].is_checked() {
        return Ok(());
    }

    let conshdlr = scip.conss[cons].conshdlr;
    let maxdualmultaggrquot = hd(scip, conshdlr).maxdualmultaggrquot;

    let lhsexists = !scip.set.is_infinity(-cd(scip, cons).lhs);
    let rhsexists = !scip.set.is_infinity(cd(scip, cons).rhs);

    // Search for a single-locked variable which can be multi-aggregated; if a
    // valid continuous variable was found, we can use it safely for
    // aggregation and break the search loop.
    let mut bestpos: i32 = -1;
    let mut bestisint = true;
    let mut bestislhs = false;

    // We only want to multi-aggregate variables, if they appear in maximal one
    // additional constraint, everything else would produce fill-in.
    // Exceptions:
    // - If there are only two variables in the constraint from which the
    //   multi-aggregation arises, no fill-in will be produced.
    // - If there are three variables in the constraint, multi-aggregation in
    //   three additional constraints will remove six nonzeros (three from the
    //   constraint and the three entries of the multi-aggregated variable) and
    //   add six nonzeros (two variables per substitution).
    // - If there at most four variables in the constraint, multi-aggregation in
    //   two additional constraints will remove six nonzeros (four from the
    //   constraint and the two entries of the multi-aggregated variable) and
    //   add six nonzeros (three variables per substitution). God exists!
    let nvars = cd(scip, cons).nvars;
    let mut maxotherlocks = if nvars <= 2 {
        i32::MAX
    } else if nvars == 3 {
        3
    } else if nvars == 4 {
        2
    } else {
        1
    };

    // If this constraint has both sides, it also provides a lock for the other
    // side and thus we can allow one more lock.
    if lhsexists && rhsexists && maxotherlocks < i32::MAX {
        maxotherlocks += 1;
    }

    let mut minabsval = scip.set.infinity();
    let mut maxabsval = -1.0;
    let mut i = 0;
    while i < cd(scip, cons).nvars && bestisint {
        let val = cd(scip, cons).vals[i];
        let absval = val.abs();

        // Calculate minimal and maximal absolute value.
        if absval < minabsval {
            minabsval = absval;
        }
        if absval > maxabsval {
            maxabsval = absval;
        }

        // Do not try to multi aggregate, when numerical bad.
        if maxabsval / minabsval > maxdualmultaggrquot {
            return Ok(());
        }

        let var = cd(scip, cons).vars[i];
        let isint = crate::var::var_is_nonimplied_integral(scip, var);

        // If we already found a candidate, skip integers;
        // better do not multi-aggregate binary variables, since most plugins
        // rely on their binary variables to be either active, fixed, or
        // single-aggregated with another binary variable.
        if (bestpos >= 0 && isint) || (var_is_binary(scip, var) && cd(scip, cons).nvars > 2) || scip_var::do_not_multaggr_var(scip, var) {
            i += 1;
            continue;
        }

        let obj = scip.vars[var].obj;
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;

        // lhs <= a_0 * x_0 + a_1 * x_1 + ... + a_{n-1} * x_{n-1} <= rhs
        //
        // a_i >= 0, c_i >= 0, lhs exists, nlocksdown(x_i) == 1:
        //  - constraint is the only one that forbids fixing the variable to its
        //    lower bound
        //  - fix x_i to the smallest value for this constraint:
        //    x_i := lhs/a_i - \sum_{j \neq i} a_j/a_i * x_j
        //
        // a_i <= 0, c_i <= 0, lhs exists, nlocksup(x_i) == 1:
        //  - constraint is the only one that forbids fixing the variable to its
        //    upper bound
        //  - fix x_i to the largest value for this constraint:
        //    x_i := lhs/a_i - \sum_{j \neq i} a_j/a_i * x_j
        //
        // a_i >= 0, c_i <= 0, rhs exists, nlocksup(x_i) == 1:
        //  - constraint is the only one that forbids fixing the variable to its
        //    upper bound
        //  - fix x_i to the largest value for this constraint:
        //    x_i := rhs/a_i - \sum_{j \neq i} a_j/a_i * x_j
        //
        // a_i <= 0, c_i >= 0, rhs exists, nlocksdown(x_i) == 1:
        //  - constraint is the only one that forbids fixing the variable to its
        //    lower bound
        //  - fix x_i to the smallest value for this constraint:
        //    x_i := rhs/a_i - \sum_{j \neq i} a_j/a_i * x_j
        //
        // but: all this is only applicable, if the aggregated value is inside
        // x_i's bounds for all possible values of all x_j; furthermore: we only
        // want to apply this, if no fill-in will be produced.
        let down = locksdown(scip, var, LockType::Model);
        let up = locksup(scip, var, LockType::Model);
        let agglhs = lhsexists
            && ((val > 0.0 && !scip.set.is_negative(obj) && down == 1 && up <= maxotherlocks) || (val < 0.0 && !scip.set.is_positive(obj) && up == 1 && down <= maxotherlocks));
        let aggrhs = rhsexists
            && ((val > 0.0 && !scip.set.is_positive(obj) && up == 1 && down <= maxotherlocks) || (val < 0.0 && !scip.set.is_negative(obj) && down == 1 && up <= maxotherlocks));

        if agglhs || aggrhs {
            // Calculate bounds for \sum_{j \neq i} a_j * x_j.
            let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_residuals(scip, c, var, val, false));
            let (mut minresactivity, mut maxresactivity) = (min.act, max.act);
            let mut isminsettoinfinity = min.issettoinfinity;
            let mut ismaxsettoinfinity = max.issettoinfinity;

            // We called consdataGetActivityResiduals() saying that we do not need
            // a good relaxation, so whenever we have a relaxed activity, it should
            // be relaxed to +/- infinity. This is needed, because we do not want
            // to rely on relaxed finite resactivities.
            if agglhs {
                let lhs = cd(scip, cons).lhs;

                // Check if lhs/a_i - \sum_{j \neq i} a_j/a_i * x_j is always inside
                // the bounds of x_i.
                let (mut minval, mut maxval) = calculate_minval_and_maxval(scip, lhs, val, minresactivity, maxresactivity);

                if !scip.set.is_infinity(-minval) && scip.set.is_ge(minval, lb) && !scip.set.is_infinity(maxval) && scip.set.is_le(maxval, ub) {
                    let mut recalculated = false;
                    let oldmaxresactivity = maxresactivity;
                    let oldminresactivity = minresactivity;

                    // Check minresactivity for reliability.
                    if !isminsettoinfinity && scip.set.is_update_unreliable(minresactivity, cd(scip, cons).lastminactivity) {
                        minresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, true, false);
                        recalculated = !scip.set.is_eq(oldminresactivity, minresactivity);
                        // Here it means only that it was even calculated.
                        isminsettoinfinity = true;
                    }

                    // Check maxresactivity for reliability.
                    if !ismaxsettoinfinity && scip.set.is_update_unreliable(maxresactivity, cd(scip, cons).lastmaxactivity) {
                        maxresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, false, false);
                        recalculated = recalculated || !scip.set.is_eq(oldmaxresactivity, maxresactivity);
                        // Here it means only that it was even calculated.
                        ismaxsettoinfinity = true;
                    }

                    // minresactivity or maxresactivity wasn't reliable so recalculate
                    // min- and maxval.
                    if recalculated {
                        // Check again if lhs/a_i - \sum_{j \neq i} a_j/a_i * x_j is
                        // always inside the bounds of x_i.
                        (minval, maxval) = calculate_minval_and_maxval(scip, lhs, val, minresactivity, maxresactivity);
                    }

                    // If the variable is integer, we have to check whether the
                    // integrality condition would always be satisfied in the
                    // multi-aggregation.
                    if (!recalculated || (scip.set.is_feas_ge(minval, lb) && scip.set.is_feas_le(maxval, ub)))
                        && (!isint || (scip.set.is_integral(lhs / val) && consdata_is_residual_integral(scip, cd(scip, cons), i, val)))
                    {
                        bestpos = i;
                        bestisint = isint;
                        bestislhs = true;
                        // No need to also look at the right hand side.
                        i += 1;
                        continue;
                    }
                }
            }

            if aggrhs {
                let rhs = cd(scip, cons).rhs;

                // Check if rhs/a_i - \sum_{j \neq i} a_j/a_i * x_j is always inside
                // the bounds of x_i.
                let (mut minval, mut maxval) = calculate_minval_and_maxval(scip, rhs, val, minresactivity, maxresactivity);

                if !scip.set.is_infinity(-minval) && scip.set.is_ge(minval, lb) && !scip.set.is_infinity(maxval) && scip.set.is_le(maxval, ub) {
                    let mut recalculated = false;
                    let oldmaxresactivity = maxresactivity;
                    let oldminresactivity = minresactivity;

                    // Check minresactivity for reliability.
                    if !isminsettoinfinity && scip.set.is_update_unreliable(minresactivity, cd(scip, cons).lastminactivity) {
                        minresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, true, false);
                        recalculated = !scip.set.is_eq(oldminresactivity, minresactivity);
                    }

                    // Check maxresactivity for reliability.
                    if !ismaxsettoinfinity && scip.set.is_update_unreliable(maxresactivity, cd(scip, cons).lastmaxactivity) {
                        maxresactivity = consdata_get_reliable_residual_activity(scip, cd(scip, cons), var, false, false);
                        recalculated = recalculated || !scip.set.is_eq(oldmaxresactivity, maxresactivity);
                    }

                    // minresactivity or maxresactivity wasn't reliable so recalculate
                    // min- and maxval.
                    if recalculated {
                        // Check again if rhs/a_i - \sum_{j \neq i} a_j/a_i * x_j is
                        // always inside the bounds of x_i.
                        (minval, maxval) = calculate_minval_and_maxval(scip, rhs, val, minresactivity, maxresactivity);
                    }

                    // If the variable is integer, we have to check whether the
                    // integrality condition would always be satisfied in the
                    // multi-aggregation.
                    if (!recalculated || (scip.set.is_feas_ge(minval, lb) && scip.set.is_feas_le(maxval, ub)))
                        && (!isint || (scip.set.is_integral(rhs / val) && consdata_is_residual_integral(scip, cd(scip, cons), i, val)))
                    {
                        bestpos = i;
                        bestisint = isint;
                        bestislhs = false;
                    }
                }
            }
        }
        i += 1;
    }

    if bestpos >= 0 {
        let c = cd(scip, cons);
        let bestvar = c.vars[bestpos];
        let bestval = c.vals[bestpos];

        // Set up the multi-aggregation.
        let mut aggrvars = Vec::with_capacity(c.nvars as usize);
        let mut aggrcoefs = Vec::with_capacity(c.nvars as usize);
        for j in 0..c.nvars {
            if j != bestpos {
                let mut coef = -c.vals[j] / c.vals[bestpos];

                // Do not try to multi aggregate, when numerical bad.
                if scip.set.is_zero(coef) {
                    return Ok(());
                }

                if bestisint {
                    // Coefficient must be integral: round it to exact integral
                    // value.
                    coef = scip.set.floor(coef + 0.5);
                }
                aggrvars.push(c.vars[j]);
                aggrcoefs.push(coef);
            }
        }
        let mut aggrconst = if bestislhs { c.lhs / bestval } else { c.rhs / bestval };

        // Right hand side must be integral: round it to exact integral value.
        if bestisint {
            aggrconst = scip.set.floor(aggrconst + 0.5);
        }

        // Perform the multi-aggregation.
        let (infeasible, aggregated) = scip_var::multiaggregate_var(scip, bestvar, &aggrvars, &aggrcoefs, aggrconst)?;

        // If the multi-aggregated bestvar is enforced but not strongly implied
        // integral, we need to convert implied integral to integer variables
        // because integrality of the multi-aggregated variable must hold.
        if !infeasible && aggregated && scip.vars[bestvar].vartype != VarType::Continuous && scip.vars[bestvar].varimpltype != ImplintType::Strong {
            for &aggrvar in &aggrvars {
                // If the multi-aggregation was not infeasible, then setting implied
                // integral to integer should not lead to infeasibility.
                if scip.vars[aggrvar].vartype == VarType::Continuous || scip.vars[aggrvar].varimpltype != ImplintType::None {
                    if scip.vars[aggrvar].vartype == VarType::Continuous {
                        scip_var::chg_var_type(scip, aggrvar, VarType::Integer)?;
                    }
                    scip_var::chg_var_impl_type(scip, aggrvar, ImplintType::None)?;
                    *nchgvartypes += 1;
                }
            }
        }

        // Check for infeasible aggregation.
        if infeasible {
            *cutoff = true;
            return Ok(());
        }

        // Delete the constraint, if the aggregation was successful.
        if aggregated {
            scip_cons::del_cons(scip, cons)?;

            if !cd(scip, cons).upgraded {
                *ndelconss += 1;
            }
            *naggrvars += 1;
        }
    }

    Ok(())
}

/// `getVarWeight`: gets weight for variable in a "weighted number of
/// variables" sum.
fn get_var_weight(scip: &Scip, var: VarId) -> i32 {
    const BINWEIGHT: i32 = 1;
    const INTWEIGHT: i32 = 4;
    const CONTWEIGHT: i32 = 8;

    if var_is_implied_integral(scip, var) {
        return INTWEIGHT;
    }

    match scip.vars[var].vartype {
        VarType::Binary => BINWEIGHT,
        VarType::Integer => INTWEIGHT,
        VarType::Continuous => CONTWEIGHT,
        VarType::ImplInt => panic!("unknown variable type"),
    }
}

/// `aggregateVariables`: tries to aggregate variables in equations
/// a^Tx = lhs; in case there are at most two binary variables with an odd
/// coefficient and all other variables are not continuous and have an even
/// coefficient then:
/// - exactly one odd binary variables: this binary variables y can be fixed
///   to 0 if the lhs is even and to 1 if the lhs is odd
///    - lhs is odd ->  y = 1
///    - lhs is even -> y = 0
/// - exactly two odd binary variables: aggregate the two binary variables
///   with odd coefficient
///    - lhs is odd -> exactly one of the variable has to be 1 -> var1 + var2 = 1
///    - lhs is even -> both have to take the same value -> var1 - var2 = 0
fn aggregate_variables(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, nfixedvars: &mut i32, naggrvars: &mut i32) -> ScipResult {
    // Check if the linear constraint is an equation with integral right hand
    // side.
    if !scip.set.is_eq(cd(scip, cons).lhs, cd(scip, cons).rhs) || !scip.set.is_integral(cd(scip, cons).lhs) {
        return Ok(());
    }

    // Try to fix and aggregated variables until nothing is possible anymore.
    loop {
        let mut success = false;

        let lhs = cd(scip, cons).lhs;
        let nvars = cd(scip, cons).nvars;

        let mut var1 = NONE;
        let mut var2 = NONE;
        let mut noddvars = 0;

        // Search for binary variables with an odd coefficient.
        let mut v = 0;
        while v < nvars && noddvars < 3 {
            let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);

            // All coefficients and variables have to be integral.
            if !var_is_integral(scip, var) || !scip.set.is_integral(val) {
                return Ok(());
            }

            let val = scip.set.feas_floor(val) as i64;
            if val % 2 != 0 {
                // The odd values have to belong to binary variables.
                if !var_is_binary(scip, var) {
                    return Ok(());
                }

                if noddvars == 0 {
                    var1 = var;
                } else {
                    var2 = var;
                }

                noddvars += 1;
            }
            v += 1;
        }

        // Check lhs is odd or even.
        let lhsodd = (scip.set.feas_floor(lhs) as i64) % 2 != 0;

        if noddvars == 1 {
            let (infeasible, fixed) = scip_var::fix_var(scip, var1, if lhsodd { 1.0 } else { 0.0 })?;

            // Check for infeasibility of fixing.
            if infeasible {
                *cutoff = true;
                return Ok(());
            }

            if fixed {
                *nfixedvars += 1;
                success = true;
            }
        } else if noddvars == 2 {
            // Aggregate the two variables with odd coefficient
            // - lhs is odd -> exactly one of the variable has to be 1 -> var1 + var2 = 1
            // - lhs is even -> both have to take the same value -> var1 - var2 = 0
            let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, var1, var2, 1.0, if lhsodd { 1.0 } else { -1.0 }, if lhsodd { 1.0 } else { 0.0 })?;

            // Check for infeasibility of aggregation.
            if infeasible {
                *cutoff = true;
                return Ok(());
            }

            // Count the aggregation.
            if aggregated {
                *naggrvars += 1;
                success = true;
            }
        }

        if success {
            // Apply fixings and aggregation to successfully rerun this presolving
            // step.
            if apply_fixings(scip, cons, true)? {
                *cutoff = true;
                return Ok(());
            }

            if normalize_cons(scip, cons)? {
                *cutoff = true;
                return Ok(());
            }
        }

        if !(success && cd(scip, cons).nvars >= 1) {
            break;
        }
    }

    Ok(())
}

/// `consdataCompSim`: sorting method for constraint data, compares two
/// variables on given indices, continuous variables will be sorted to the
/// end and for all other variables the sortation will be in non-increasing
/// order of their absolute value of the coefficients.
fn consdata_comp_sim(scip: &Scip, c: &ConsData, ind1: usize, ind2: usize) -> i32 {
    let varcont1 = !var_is_integral(scip, c.vars[ind1]);
    let varcont2 = !var_is_integral(scip, c.vars[ind2]);

    if varcont1 {
        // Continuous variables will be sorted to the back.
        if varcont1 != varcont2 {
            return 1;
        }
        // Both variables are continuous.
        else {
            return 0;
        }
    }
    // Continuous variables will be sorted to the back.
    else if varcont2 {
        return -1;
    }

    let value = c.vals[ind2].abs() - c.vals[ind1].abs();

    // For all non-continuous variables, the variables are sorted after
    // decreasing absolute coefficients.
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// `rangedRowSimplify`: tries to simplify coefficients in ranged row of the
/// form lhs <= a^Tx <= rhs
///
/// 1. lhs <= a^Tx <= rhs, x binary, lhs > 0, forall a_i >= lhs, a_i <= rhs,
///    and forall pairs a_i + a_j > rhs, then we can change this constraint
///    to 1^Tx = 1
fn ranged_row_simplify(scip: &mut Scip, cons: ConsId, nchgcoefs: &mut i32, nchgsides: &mut i32) -> ScipResult {
    // We must not change a modifiable constraint in any way.
    if scip.conss[cons].modifiable || scip.conss[cons].deleted {
        return Ok(());
    }

    let nvars = cd(scip, cons).nvars;

    // Do not check empty or bound-constraints.
    if nvars < 2 {
        return Ok(());
    }

    let lhs = cd(scip, cons).lhs;
    let rhs = cd(scip, cons).rhs;

    // Sides must be positive and different to detect set partition.
    if !scip.set.is_positive(lhs) || !scip.set.is_lt(lhs, rhs) {
        return Ok(());
    }

    let mut minval = INVALID;
    let mut secondminval = INVALID;
    let mut maxval = -INVALID;

    let mut v = nvars - 1;
    while v >= 0 {
        let (var, val) = (cd(scip, cons).vars[v], cd(scip, cons).vals[v]);
        if var_is_binary(scip, var) {
            if minval > val || minval == INVALID {
                secondminval = minval;
                minval = val;
            } else if secondminval > val || secondminval == INVALID {
                secondminval = val;
            }

            if maxval < val || maxval == -INVALID {
                maxval = val;
            }
        } else {
            break;
        }
        v -= 1;
    }

    // Check if all variables are binary, we can choose one, and need to choose
    // at most one.
    if v == -1 && scip.set.is_ge(minval, lhs) && scip.set.is_le(maxval, rhs) && scip.set.is_gt(minval + secondminval, rhs) {
        // Change all coefficients to 1.0.
        let mut v = nvars - 1;
        while v >= 0 {
            chg_coef_pos(scip, cons, v, 1.0)?;
            v -= 1;
        }
        *nchgcoefs += nvars;

        // Replace old right and left hand side with 1.0.
        chg_rhs(scip, cons, 1.0)?;
        chg_lhs(scip, cons, 1.0)?;
        *nchgsides += 2;
    }

    Ok(())
}

/// `simplifyInequalities`: tries to simplify coefficients and delete
/// variables in constraints of the form lhs <= a^Tx <= rhs.
///
/// For both-sided constraints only [`ranged_row_simplify`] will be called.
///
/// For one-sided constraints there are several different coefficient
/// reduction steps which will be applied:
///
/// 1. We try to determine parts of the constraint which will not change
///    anything on (in-)feasibility of the constraint, e.g. 5x1 + 5x2 + 3z1
///    <= 8 => 3z1 is redundant if all x are binary and -2 < 3z1 <= 3
/// 2. We try to remove redundant fractional parts in a constraint, e.g.
///    5.2x1 + 5.1x2 + 3x3 <= 8.3  => will be changed to 5x1 + 5x2 + 3x3 <= 8
///    if all x are binary
/// 3. We are using the greatest common divisor for further reductions, e.g.
///    10x1 + 5y2 + 5x3 + 3x4 <= 15  => will be changed to 2x1 + y2 + x3 + x4
///    <= 3 if all xi are binary and y2 is integral
///
/// Returns whether infeasibility was detected.
fn simplify_inequalities(scip: &mut Scip, cons: ConsId, nchgcoefs: &mut i32, nchgsides: &mut i32) -> ScipResult<bool> {
    let vals = |scip: &Scip, i: i32| cd(scip, cons).vals[i];
    let vars = |scip: &Scip, i: i32| cd(scip, cons).vars[i];
    let glb = |scip: &Scip, var: VarId| (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);

    // We must not change a modifiable constraint in any way.
    if scip.conss[cons].modifiable || scip.conss[cons].deleted {
        return Ok(false);
    }

    let mut nvars = cd(scip, cons).nvars;

    // Do not check empty or bound-constraints.
    if nvars <= 2 {
        return Ok(false);
    }

    // Update maximal activity delta if necessary.
    if cd(scip, cons).maxactdelta == INVALID {
        with_cd(scip, cons, |scip, c| consdata_recompute_max_activity_delta(scip, c));
    }

    // Can only work with valid non-infinity activities per variable.
    if scip.set.is_infinity(cd(scip, cons).maxactdelta) {
        return Ok(false);
    }

    // Due to vartype changes, the status of the normalization can be wrong,
    // need an event but the eventsystem seems to be full.
    cdm(scip, cons).normalized = false;

    if normalize_cons(scip, cons)? {
        return Ok(true);
    }

    if !cd(scip, cons).normalized {
        return Ok(false);
    }

    let mut lhs = cd(scip, cons).lhs;
    let mut rhs = cd(scip, cons).rhs;

    let haslhs = !scip.set.is_infinity(-lhs);
    let hasrhs = !scip.set.is_infinity(rhs);

    // Extend both-sided simplification.
    if haslhs && hasrhs {
        ranged_row_simplify(scip, cons, nchgcoefs, nchgsides)?;

        return Ok(false);
    }

    // If we have a normalized inequality (not ranged) the one side should be
    // positive, @see normalizeCons().

    // Call sorting method, order continuous variables to the end and all other
    // variables after non-increasing absolute value of their coefficients.
    let eventhdlr = hd(scip, scip.conss[cons].conshdlr).eventhdlr;
    with_cd(scip, cons, |scip, c| {
        let mut perm: Vec<i32> = (0..nvars).collect();
        sort::sort(&mut perm, (), |&a, &b| consdata_comp_sim(scip, c, a as usize, b as usize));

        // Perform sorting after permutation array.
        perm_sort_consdata(scip, eventhdlr, c, &mut perm, nvars);
        c.indexsorted = false;
        c.coefsorted = false;
    });

    // Only check constraints with at least two non continuous variables.
    if !var_is_integral(scip, vars(scip, 1)) {
        return Ok(false);
    }

    // Do not process constraints when all coefficients are 1.0.
    if scip.set.is_eq(vals(scip, 0).abs(), 1.0) && ((hasrhs && scip.set.is_integral(rhs)) || (haslhs && scip.set.is_integral(lhs))) {
        return Ok(false);
    }

    let feastol = scip.set.feastol();

    // Get global activities.
    let (minb, maxb) = with_cd(scip, cons, |scip, c| consdata_get_glb_activity_bounds(scip, c, false, true, true));
    let (minact, maxact) = (minb.act, maxb.act);

    // Cannot work with infinite activities.
    if minb.issettoinfinity || maxb.issettoinfinity {
        return Ok(false);
    }

    let mut v: i32 = 0;
    let mut offsetv: i32 = -1;
    let side = if haslhs { lhs } else { rhs };
    let mut minactsub = minact;
    let mut maxactsub = maxact;
    let mut allcoefintegral;

    // We now determine coefficients as large as the side of the constraint to
    // retrieve a better reduction where we do not need to look at the large
    // coefficients
    //
    // e.g.  all x are binary, z are positive integer
    //     c1: +5x1 + 5x2 + 3x3 + 3x4 + x5 >= 5  (x5 is redundant and does not
    //         change (in-)feasibility of this constraint)
    //     c2: +4x1 + 4x2 + 3x3 + 3x4 + x5 >= 4  (gcd (without the coefficient
    //         of x5) after the large coefficients is 3
    //     c3: +30x1 + 29x2 + 14x3 + 14z1 + 7x5 + 7x6 <= 30 (gcd (without the
    //         coefficient of x2) after the large coefficients is 7
    //
    //     can be changed to
    //
    //     c1: +6x1 + 6x2 + 3x3 + 3x4 >= 6      (will be changed to c1: +2x1 +
    //         2x2 + x3 + x4 >= 2)
    //     c2: +6x1 + 6x2 + 3x3 + 3x4 + 3x5 >= 6  (will be changed to c2: +2x1
    //         + 2x2 + x3 + x4 + x5 >= 2)
    //     c3: +28x1 + 28x2 + 14x3 + 14z1 + 7x5 + 7x6 <= 28 (will be changed to
    //         c3: +4x1 + 4x2 + 2x3 + 2z1 + x5 + x6 <= 4)
    //
    // If the minimal activity is negative and we found more than one variable
    // with a coefficient bigger than the left hand side, we cannot apply the
    // extra reduction step and need to reset v
    //
    // e.g. 7x1 + 7x2 - 4x3 - 4x4 >= 7 => xi = 1 for all i is not a solution,
    //    but if we would do a change on the coefficients due to the gcd on the
    //    "small" coefficients we would get 8x1 + 8x2 - 4x3 - 4x4 >= 8 were xi
    //    = 1 for all i is a solution
    //
    // also redundancy of variables would not be correctly determined in such
    // a case.
    if nvars > 2 && scip.set.is_eq(vals(scip, 0), side) && !scip.set.is_negative(minactsub) {
        v = 1;

        while v < nvars && scip.set.is_eq(side, vals(scip, v)) {
            // If we have integer variable with "side"-coefficients but also with
            // a lower bound greater than 0 we stop this extra step, which might
            // have worked.
            if scip.vars[vars(scip, v)].glbdom.lb > 0.5 {
                v = 0;
                break;
            }

            v += 1;
        }

        // Easy and quick fix: if all coefficients were equal to the side, we
        // cannot apply further simplifications.
        if v == nvars {
            return Ok(false);
        }

        // Cannot work with continuous variables which have a big coefficient.
        if v > 0 && !var_is_integral(scip, vars(scip, v - 1)) {
            return Ok(false);
        }

        // Big negative coefficient, do not try to use the extra coefficient
        // reduction step.
        if scip.set.is_eq(side, -vals(scip, v)) {
            v = 0;
        }

        // All but one variable are processed or the next variable is continuous
        // we cannot perform the extra coefficient reduction.
        if v == nvars - 1 || !var_is_integral(scip, vars(scip, v)) {
            v = 0;
        }

        if v > 0 {
            offsetv = v - 1;

            for w in 0..v {
                let (lb, ub) = glb(scip, vars(scip, w));

                // Update residual activities.
                maxactsub -= ub * vals(scip, w);
                minactsub -= lb * vals(scip, w);
            }
        }
    }

    // Find and remove redundant variables which do not interact with the
    // (in-)feasibility of this constraint
    //
    // e.g. let all x are binary and y1 is continuous with bounds [-3,1] then we
    // can reduce
    //
    //      15x1 + 15x2 + 7x3 + 3x4 + y1 <= 26
    // to
    //      15x1 + 15x2 <= 26 <=> x1 + x2 <= 1
    if nvars > 2 && scip.set.is_integral(vals(scip, v)) {
        let mut redundant = false;

        let mut gcd = (vals(scip, v).abs() + feastol) as i64;

        if v == 0 {
            let (lb, ub) = glb(scip, vars(scip, 0));

            // Update residual activities.
            if vals(scip, 0) > 0.0 {
                maxactsub -= ub * vals(scip, 0);
                minactsub -= lb * vals(scip, 0);
            } else {
                maxactsub -= lb * vals(scip, 0);
                minactsub -= ub * vals(scip, 0);
            }
            v += 1;
        }

        let mut siderest = -INVALID;
        allcoefintegral = true;

        // Check if some variables always fit into the given constraint.
        while v < nvars - 1 {
            if !var_is_integral(scip, vars(scip, v)) {
                break;
            }

            if !scip.set.is_integral(vals(scip, v)) {
                allcoefintegral = false;
                break;
            }

            // Calculate greatest common divisor for all general and binary
            // variables.
            gcd = crate::misc::calc_gre_com_div(gcd, (vals(scip, v).abs() + feastol) as i64);

            if gcd == 1 {
                break;
            }

            let (lb, ub) = glb(scip, vars(scip, v));

            // Update residual activities.
            if vals(scip, v) > 0.0 {
                maxactsub -= ub * vals(scip, v);
                minactsub -= lb * vals(scip, v);
            } else {
                maxactsub -= lb * vals(scip, v);
                minactsub -= ub * vals(scip, v);
            }

            let gcdf = gcd as f64;
            if hasrhs {
                // Determine the remainder of the right hand side and the gcd.
                siderest = rhs - scip.set.feas_floor(rhs / gcdf) * gcdf;
            } else {
                // Determine the remainder of the left hand side and the gcd.
                siderest = lhs - scip.set.feas_floor(lhs / gcdf) * gcdf;
                if scip.set.is_zero(siderest) {
                    siderest = gcdf;
                }
            }

            let rredundant = hasrhs && maxactsub <= siderest && scip.set.is_feas_gt(minactsub, siderest - gcdf);
            let lredundant = haslhs && scip.set.is_feas_lt(maxactsub, siderest) && minactsub >= siderest - gcdf;

            // Early termination if the activities deceed the gcd.
            if offsetv == -1 && (rredundant || lredundant) {
                redundant = true;
                break;
            }
            v += 1;
        }

        if !redundant {
            let gcdf = gcd as f64;
            if hasrhs {
                // Determine the remainder of the right hand side and the gcd.
                siderest = rhs - scip.set.feas_floor(rhs / gcdf) * gcdf;
            } else {
                // Determine the remainder of the left hand side and the gcd.
                siderest = lhs - scip.set.feas_floor(lhs / gcdf) * gcdf;
                if scip.set.is_zero(siderest) {
                    siderest = gcdf;
                }
            }
        } else {
            v += 1;
        }

        // To avoid inconsistencies due to numerics, check that the full and
        // partial activities have reasonable absolute values.
        let numericsok = maxact.abs() < MAXACTVAL && maxactsub.abs() < MAXACTVAL && minact.abs() < MAXACTVAL && minactsub.abs() < MAXACTVAL;

        let gcdf = gcd as f64;
        let rredundant = hasrhs && maxactsub <= siderest && scip.set.is_feas_gt(minactsub, siderest - gcdf);
        let lredundant = haslhs && scip.set.is_feas_lt(maxactsub, siderest) && minactsub >= siderest - gcdf;

        // Check if we can remove redundant variables.
        if v < nvars && numericsok && (redundant || (offsetv == -1 && (rredundant || lredundant))) {
            // Remove redundant variables.
            let mut w = nvars - 1;
            while w >= v {
                del_coef_pos(scip, cons, w)?;
                w -= 1;
            }
            *nchgcoefs += nvars - v;

            let oldcoef = vals(scip, w);

            if normalize_cons(scip, cons)? {
                return Ok(true);
            }

            // Compute new greatest common divisor due to normalization.
            gcd = (gcd as f64 / (oldcoef / vals(scip, w)) + feastol) as i64;

            // Update side.
            if hasrhs {
                // Replace old with new right hand side.
                let newrhs = scip.set.feas_floor(cd(scip, cons).rhs);
                chg_rhs(scip, cons, newrhs)?;
                rhs = cd(scip, cons).rhs;
            } else if scip.set.is_feas_gt(oldcoef / vals(scip, w), 1.0) {
                let newlhs = scip.set.feas_ceil(cd(scip, cons).lhs);
                chg_lhs(scip, cons, newlhs)?;
                lhs = cd(scip, cons).lhs;
            }
            *nchgsides += 1;

            // Get new constraint data.
            nvars = cd(scip, cons).nvars;

            allcoefintegral = true;
        }

        let mut candpos: i32;

        // Try to find a better gcd, when having large coefficients.
        if offsetv >= 0 && gcd == 1 {
            // Calculate greatest common divisor for all general variables.
            gcd = (vals(scip, nvars - 1).abs() + feastol) as i64;

            if gcd > 1 {
                gcd = -1;
                candpos = -1;

                let mut v = nvars - 1;
                while v > offsetv {
                    if !var_is_integral(scip, vars(scip, v)) {
                        break;
                    }

                    if !scip.set.is_integral(vals(scip, v)) {
                        allcoefintegral = false;
                        break;
                    }

                    let oldgcd = gcd;

                    if gcd == -1 {
                        gcd = (vals(scip, v).abs() + feastol) as i64;
                    } else {
                        // Calculate greatest common divisor for all general and
                        // binary variables.
                        gcd = crate::misc::calc_gre_com_div(gcd, (vals(scip, v).abs() + feastol) as i64);
                    }

                    // If the greatest commmon divisor has become 1, we might have
                    // found the possible coefficient to change or we can stop
                    // searching.
                    if gcd == 1 {
                        if !var_is_binary(scip, vars(scip, v)) {
                            break;
                        }

                        // Found candidate.
                        if candpos == -1 {
                            gcd = oldgcd;
                            candpos = v;
                        }
                        // Two different binary variables lead to a gcd of one, so we
                        // cannot change a coefficient.
                        else {
                            break;
                        }
                    }
                    v -= 1;
                }
            } else {
                candpos = -1;
            }
        } else {
            candpos = nvars - 1;
        }

        // Check last coefficient for integrality.
        if gcd > 1 && allcoefintegral && !redundant && !scip.set.is_integral(vals(scip, nvars - 1)) {
            allcoefintegral = false;
        }

        // Check for further necessary coefficient adjustments.
        if offsetv >= 0 && gcd > 1 && allcoefintegral && var_is_integral(scip, vars(scip, candpos)) {
            let mut notchangable = false;
            let candval = vals(scip, candpos);
            let candvar = vars(scip, candpos);

            if candval > 0.0 && var_is_binary(scip, candvar) && crate::misc::calc_gre_com_div(gcd, (candval.abs() + feastol) as i64) < gcd {
                // Determine the remainder of the side and the gcd.
                let rest = if hasrhs { ((rhs + feastol) as i64) % gcd } else { ((lhs + feastol) as i64) % gcd };

                // Determine the remainder of the coefficient candidate and the gcd.
                let restcoef = ((candval + feastol) as i64) % gcd;

                let newcoef = if hasrhs {
                    // Calculate new coefficient.
                    if restcoef > rest {
                        candval - restcoef as f64 + gcd as f64
                    } else {
                        candval - restcoef as f64
                    }
                } else {
                    // Calculate new coefficient.
                    if rest == 0 || restcoef < rest {
                        candval - restcoef as f64
                    } else {
                        candval - restcoef as f64 + gcd as f64
                    }
                };

                // New coeffcient must not be zero if we would loose the
                // implication that a variable needs to be 0 if another with the
                // big coefficient was set to 1.
                if hasrhs && scip.set.is_zero(newcoef) {
                    notchangable = true;
                } else if scip.set.is_zero(newcoef) {
                    // Delete old redundant coefficient.
                    del_coef_pos(scip, cons, candpos)?;
                    *nchgcoefs += 1;
                } else {
                    // Replace old with new coefficient.
                    chg_coef_pos(scip, cons, candpos, newcoef)?;
                    *nchgcoefs += 1;
                }
            } else if candval < 0.0 || !var_is_binary(scip, candvar) {
                gcd = crate::misc::calc_gre_com_div(gcd, (candval.abs() + feastol) as i64);
            }

            // Correct side and big coefficients.
            let candabs = (vals(scip, candpos).abs() + feastol) as i64;
            if (!notchangable
                && hasrhs
                && ((!scip.set.is_feas_integral(rhs) || crate::misc::calc_gre_com_div(gcd, (rhs + feastol) as i64) < gcd) && crate::misc::calc_gre_com_div(gcd, candabs) == gcd))
                || (haslhs
                    && (!scip.set.is_feas_integral(lhs) || crate::misc::calc_gre_com_div(gcd, (lhs + feastol) as i64) < gcd)
                    && crate::misc::calc_gre_com_div(gcd, candabs) == gcd)
            {
                let gcdf = gcd as f64;
                let newcoef;
                if haslhs {
                    newcoef = ((scip.set.feas_ceil(lhs / gcdf) * gcdf + feastol) as i64) as f64;

                    chg_lhs(scip, cons, newcoef)?;
                    *nchgsides += 1;
                } else {
                    newcoef = ((scip.set.feas_floor(rhs / gcdf) * gcdf + feastol) as i64) as f64;

                    chg_rhs(scip, cons, newcoef)?;
                    *nchgsides += 1;
                }

                // Correct coefficients up front.
                let mut w = offsetv;
                while w >= 0 {
                    chg_coef_pos(scip, cons, w, newcoef)?;
                    w -= 1;
                }
                *nchgcoefs += offsetv + 1;
            }

            if !notchangable {
                if normalize_cons(scip, cons)? {
                    return Ok(true);
                }

                // Get new constraint data.
                nvars = cd(scip, cons).nvars;

                lhs = cd(scip, cons).lhs;
                rhs = cd(scip, cons).rhs;
            }
        }
    }

    // We still can remove continuous variables if they are redundant due to
    // the non-integrality argument.

    // No continuous variables are left over.
    if !var_is_integral(scip, vars(scip, nvars - 1)) {
        return Ok(false);
    }

    let mut onlybin = true;
    allcoefintegral = true;

    // Check if all variables are of binary type.
    let mut v = nvars - 1;
    while v >= 0 {
        if !var_is_binary(scip, vars(scip, v)) {
            onlybin = false;
        }
        if !scip.set.is_integral(vals(scip, v)) {
            allcoefintegral = false;
        }
        v -= 1;
    }

    // Check if the non-integrality part of all integral variables is smaller
    // than the non-inegrality part of the right hand side or bigger than the
    // left hand side respectively, so we can make all of them integral.
    if (hasrhs && !scip.set.is_feas_integral(rhs)) || (haslhs && !scip.set.is_feas_integral(lhs)) {
        let mut frac = 0.0;
        let mut found = false;

        if hasrhs {
            if allcoefintegral {
                // Replace old with new right hand side.
                chg_rhs(scip, cons, scip.set.floor(rhs))?;
                *nchgsides += 1;
            } else {
                let siderest = rhs - scip.set.floor(rhs);

                // Try to round down all non-integral coefficients.
                let mut v = nvars - 1;
                while v >= 0 {
                    let mut val = vals(scip, v);

                    // Add up all possible fractional parts.
                    if !scip.set.is_integral(val) {
                        let (lb, mut ub) = glb(scip, vars(scip, v));

                        // At least one bound need to be at zero.
                        if !onlybin && !scip.set.is_feas_zero(lb) && !scip.set.is_feas_zero(ub) {
                            return Ok(false);
                        }

                        // Swap bounds for 'standard' form.
                        if !scip.set.is_feas_zero(lb) {
                            ub = -lb;
                            val *= -1.0;
                        }

                        found = true;

                        frac += (val - scip.set.floor(val)) * ub;

                        // If we exceed the fractional part of the right hand side,
                        // we cannot tighten the coefficients
                        //
                        // e.g. 1.1x1 + 1.1x2 + 1.4x3 + 1.02x4 <= 2.4, here we cannot
                        //    floor all fractionals because x3, x4 set to 1 would be
                        //    infeasible but feasible after flooring.
                        if scip.set.is_gt(frac, siderest) {
                            return Ok(false);
                        }
                    }
                    v -= 1;
                }

                // Round rhs and coefficients to integral values.
                if found {
                    simplify_round_coefs(scip, cons, nvars, nchgcoefs)?;
                }

                // Replace old with new right hand side.
                chg_rhs(scip, cons, scip.set.floor(rhs))?;
                *nchgsides += 1;
            }
        } else if allcoefintegral {
            // Replace old with new left hand side.
            chg_lhs(scip, cons, scip.set.ceil(lhs))?;
            *nchgsides += 1;
        } else {
            // Cannot floor left hand side to zero.
            if scip.set.is_lt(lhs, 1.0) {
                return Ok(false);
            }

            let siderest = lhs - scip.set.floor(lhs);

            // Try to round down all non-integral coefficients.
            let mut v = nvars - 1;
            while v >= 0 {
                let mut val = vals(scip, v);

                // Add up all possible fractional parts.
                if !scip.set.is_integral(val) {
                    let (lb, mut ub) = glb(scip, vars(scip, v));

                    // At least one bound need to be at zero.
                    if !scip.set.is_feas_zero(lb) && !scip.set.is_feas_zero(ub) {
                        return Ok(false);
                    }

                    // Swap bounds for 'standard' form.
                    if !scip.set.is_feas_zero(lb) {
                        ub = -lb;
                        val *= -1.0;
                    }

                    // Cannot floor to zero.
                    if scip.set.is_lt(val, 1.0) {
                        return Ok(false);
                    }

                    // The fractional part on each variable need to exceed the
                    // fractional part on the left hand side.
                    if scip.set.is_lt(val - scip.set.floor(val), siderest) {
                        return Ok(false);
                    }

                    found = true;

                    frac += (val - scip.set.floor(val)) * ub;

                    // If we exceed the fractional part of the left hand side plus
                    // one by summing up all maximal fractional parts of the
                    // variables, we cannot tighten the coefficients
                    //
                    // e.g. 4.3x1 + 1.3x2 + 1.3x3 + 1.6x4 >= 4.2, here we cannot
                    //    floor all fractionals because x2-x4 set to 1 would be
                    //    feasible but not after flooring.
                    if scip.set.is_ge(frac, 1.0 + siderest) {
                        return Ok(false);
                    }
                }
                // All coefficients need to be integral, otherwise we might do an
                // invalid reduction.
                else {
                    return Ok(false);
                }
                v -= 1;
            }

            // Round lhs and coefficients to integral values.
            if found {
                simplify_round_coefs(scip, cons, nvars, nchgcoefs)?;
            }

            // Replace old with new left hand side.
            chg_lhs(scip, cons, scip.set.floor(lhs))?;
            *nchgsides += 1;
        }

        if normalize_cons(scip, cons)? {
            return Ok(true);
        }

        rhs = cd(scip, cons).rhs;
        lhs = cd(scip, cons).lhs;

        nvars = cd(scip, cons).nvars;
        if nvars < 2 {
            return Ok(false);
        }

        allcoefintegral = true;
    }

    // Following can also work on non integral coefficients, need more
    // investigation; only check constraints with integral coefficients on all
    // integral variables.
    if !allcoefintegral {
        return Ok(false);
    }

    // We want to avoid numerical troubles, therefore we do not change
    // non-integral sides.
    if (hasrhs && !scip.set.is_integral(rhs)) || (haslhs && !scip.set.is_integral(lhs)) {
        return Ok(false);
    }

    // Maximal absolute value of coefficients in constraint is one, so we cannot
    // tighten it further.
    if scip.set.is_eq(vals(scip, 0).abs(), 1.0) {
        return Ok(false);
    }

    // Stop if the last coeffcients is one in absolute value and the variable is
    // not binary.
    if !var_is_binary(scip, vars(scip, nvars - 1)) && scip.set.is_eq(vals(scip, nvars - 1).abs(), 1.0) {
        return Ok(false);
    }

    // Start gcd procedure for all variables.
    loop {
        // Stop if we have two coeffcients which are one in absolute value.
        if scip.set.is_eq(vals(scip, nvars - 1).abs(), 1.0) && scip.set.is_eq(vals(scip, nvars - 2).abs(), 1.0) {
            return Ok(false);
        }

        let mut gcd: i64 = -1;
        let foundbin: i32;

        // Calculate greatest common divisor over all integer variables; note
        // that the onlybin flag needs to be recomputed because coefficients of
        // non-binary variables might have changed to zero.
        if !onlybin {
            let mut fb = -1;
            onlybin = true;

            let mut v = nvars - 1;
            while v >= 0 {
                if var_is_binary(scip, vars(scip, v)) {
                    if fb == -1 {
                        fb = v;
                    }
                    v -= 1;
                    continue;
                } else {
                    onlybin = false;
                }

                let absval = vals(scip, v).abs();

                // Arithmetic precision can lead to the absolute value only being
                // integral up to feasibility tolerance, even though the value
                // itself is feasible up to epsilon, but since we add feastol
                // later, this is enough.
                if gcd == -1 {
                    gcd = (absval + feastol) as i64;
                } else {
                    // Calculate greatest common divisor for all general variables.
                    gcd = crate::misc::calc_gre_com_div(gcd, (absval + feastol) as i64);
                }
                if gcd == 1 {
                    break;
                }
                v -= 1;
            }
            foundbin = fb;
        } else {
            foundbin = nvars - 1;
        }

        // We need at least one binary variable and a gcd greater than 1 to try
        // to perform further coefficient changes.
        if gcd == 1 || foundbin == -1 {
            return Ok(false);
        }

        let mut candpos: i32 = -1;
        let mut candpos2: i32 = -1;

        // Calculate greatest common divisor over all integer and binary
        // variables and determine the candidate where we might change the
        // coefficient.
        let mut v = foundbin;
        while v >= 0 {
            if onlybin || var_is_binary(scip, vars(scip, v)) {
                // Arithmetic precision can lead to the absolute value only being
                // integral up to feasibility tolerance, even though the value
                // itself is feasible up to epsilon, but since we add feastol
                // later, this is enough.
                let oldgcd = gcd;

                if gcd == -1 {
                    gcd = (vals(scip, v).abs() + feastol) as i64;
                } else {
                    // Calculate greatest common divisor for all general and binary
                    // variables.
                    gcd = crate::misc::calc_gre_com_div(gcd, (vals(scip, v).abs() + feastol) as i64);
                }

                // If the greatest commmon divisor has become 1, we might have
                // found the possible coefficient to change or we can terminate.
                if gcd == 1 {
                    // Found candidate.
                    if candpos == -1 {
                        gcd = oldgcd;
                        candpos = v;

                        // If we have only binary variables and both first
                        // coefficients have a gcd of 1, both are candidates for the
                        // coefficient change.
                        if onlybin && v == foundbin - 1 {
                            candpos2 = foundbin;
                        }
                    }
                    // Two different binary variables lead to a gcd of one, so we
                    // cannot change a coefficient.
                    else if onlybin && candpos == v + 1 && candpos2 == v + 2 {
                        // Take new candidates.
                        candpos = candpos2;

                        // Recalculate gcd from scratch.
                        gcd = (vals(scip, v + 1).abs() + feastol) as i64;

                        // Calculate greatest common divisor for all general and
                        // binary variables.
                        gcd = crate::misc::calc_gre_com_div(gcd, (vals(scip, v).abs() + feastol) as i64);
                        if gcd == 1 {
                            return Ok(false);
                        }
                    } else {
                        // Cannot determine a possible coefficient for reduction.
                        return Ok(false);
                    }
                }
            }
            v -= 1;
        }

        // Determine the remainder of the side and the gcd.
        let rest = if hasrhs { ((rhs + feastol) as i64) % gcd } else { ((lhs + feastol) as i64) % gcd };

        // Determine the remainder of the coefficient candidate and the gcd.
        let candval = vals(scip, candpos);
        let restcoef = if candval < 0.0 {
            ((candval - feastol) as i64) % gcd + gcd
        } else {
            ((candval + feastol) as i64) % gcd
        };

        let newcoef;
        if hasrhs {
            if rest > 0 {
                // Replace old with new right hand side.
                chg_rhs(scip, cons, rhs - rest as f64)?;
                *nchgsides += 1;
            }

            // Calculate new coefficient.
            newcoef = if restcoef > rest {
                candval - restcoef as f64 + gcd as f64
            } else {
                candval - restcoef as f64
            };
        } else {
            if rest > 0 {
                // Replace old with new left hand side.
                chg_lhs(scip, cons, lhs - rest as f64 + gcd as f64)?;
                *nchgsides += 1;
            }

            // Calculate new coefficient.
            newcoef = if rest == 0 || restcoef < rest {
                candval - restcoef as f64
            } else {
                candval - restcoef as f64 + gcd as f64
            };
        }

        if scip.set.is_zero(newcoef) {
            // Delete redundant coefficient.
            del_coef_pos(scip, cons, candpos)?;
        } else {
            // Replace old with new coefficient.
            chg_coef_pos(scip, cons, candpos, newcoef)?;
        }
        *nchgcoefs += 1;

        // Now constraint can be normalized, might be directly done by dividing it
        // by the gcd.
        if normalize_cons(scip, cons)? {
            return Ok(true);
        }

        rhs = cd(scip, cons).rhs;
        lhs = cd(scip, cons).lhs;
        nvars = cd(scip, cons).nvars;

        if nvars < 2 {
            break;
        }
    }

    Ok(false)
}

/// The coefficient rounding of [`simplify_inequalities`]: floors (lower
/// bound zero) or ceils every non-integral coefficient, deleting those that
/// become zero.
fn simplify_round_coefs(scip: &mut Scip, cons: ConsId, nvars: i32, nchgcoefs: &mut i32) -> ScipResult {
    let mut v = nvars - 1;
    while v >= 0 {
        let val = cd(scip, cons).vals[v];

        // Add the whole fractional part.
        if !scip.set.is_integral(val) {
            let lb = scip.vars[cd(scip, cons).vars[v]].glbdom.lb;

            let newval = if scip.set.is_feas_zero(lb) { scip.set.floor(val) } else { scip.set.ceil(val) };

            if scip.set.is_zero(newval) {
                // Delete old redundant coefficient.
                del_coef_pos(scip, cons, v)?;
                *nchgcoefs += 1;
            } else {
                // Replace old with new coefficient.
                chg_coef_pos(scip, cons, v, newval)?;
                *nchgcoefs += 1;
            }
        }
        v -= 1;
    }
    Ok(())
}

/// The common and different variables of two constraints, as positions in
/// each, for [`aggregate_constraints`].
struct AggrIndices<'a> {
    commonidx0: &'a [i32],
    commonidx1: &'a [i32],
    diffidx0minus1: &'a [i32],
    diffidx1minus0: &'a [i32],
    nvarscommon: i32,
    commonidxweight: i32,
    diffidx0minus1weight: i32,
    diffidx1minus0weight: i32,
}

/// `aggregateConstraints`: tries to aggregate an (in)equality and an
/// equality in order to decrease the number of variables in the
/// (in)equality:
///  cons0 := a * cons0 + b * cons1,
/// where a = val1[v] and b = -val0[v] for common variable v which removes
/// most variable weight; for numerical stability, we will only accept
/// integral a and b; the variable weight is a weighted sum over all
/// included variables, where each binary variable weighs BINWEIGHT, each
/// integer or implied integral variable weighs INTWEIGHT and each
/// continuous variable weighs CONTWEIGHT.
///
/// Returns `(aggregated, infeasible)`.
fn aggregate_constraints(scip: &mut Scip, cons0: ConsId, cons1: ConsId, ix: &AggrIndices, maxaggrnormscale: f64, nchgcoefs: &mut i32) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;
    let mut aggregated = false;
    let nvarscommon = ix.nvarscommon as usize;

    // Search for the best common variable such that
    //  val1[var] * consdata0 - val0[var] * consdata1
    // has least weighted number of variables.
    let mut bestvarweight = ix.commonidxweight + ix.diffidx0minus1weight;
    let mut bestv: i32 = -1;
    let mut bestscalarsum = 0.0;
    let mut commonvarlindependent = true;
    {
        let c0 = cd(scip, cons0);
        let c1 = cd(scip, cons1);
        for v in 0..nvarscommon {
            let a = c1.vals[ix.commonidx1[v]];
            let b = -c0.vals[ix.commonidx0[v]];

            // Only try aggregation, if coefficients are integral (numerical
            // stability).
            if scip.set.is_integral(a) && scip.set.is_integral(b) {
                // Count the number of variables in the potential new constraint
                // a * consdata0 + b * consdata1.
                let mut varweight = ix.diffidx0minus1weight + ix.diffidx1minus0weight;
                let scalarsum = a.abs() + b.abs();
                let betterscalarsum = scalarsum < bestscalarsum;
                let mut i = 0;
                while i < nvarscommon && (varweight < bestvarweight || (varweight == bestvarweight && betterscalarsum)) {
                    let aggrcoef = a * c0.vals[ix.commonidx0[i]] + b * c1.vals[ix.commonidx1[i]];
                    if !scip.set.is_zero(aggrcoef) {
                        varweight += get_var_weight(scip, c0.vars[ix.commonidx0[i]]);
                    }
                    i += 1;
                }

                if varweight < bestvarweight || (varweight == bestvarweight && betterscalarsum) {
                    bestv = v as i32;
                    bestvarweight = varweight;
                    bestscalarsum = scalarsum;
                }
            }

            // Update commonvarlindependent flag, if still TRUE: v's common
            // coefficient in cons1 / v's common coefficient in cons0 should be
            // constant, i.e., equal 0's common coefficient in cons1 / 0's common
            // coefficient in cons0.
            if commonvarlindependent && v > 0 {
                commonvarlindependent = scip
                    .set
                    .is_eq(c1.vals[ix.commonidx1[v]] * c0.vals[ix.commonidx0[0]], c1.vals[ix.commonidx1[0]] * c0.vals[ix.commonidx0[v]]);
            }
        }
    }

    // If better aggregation was found, create new constraint and delete old
    // one.
    if (bestv != -1 || commonvarlindependent) && scip.conss[cons0].nupgradelocks == 0 {
        let c0 = cd(scip, cons0);
        let c1 = cd(scip, cons1);
        let mut a;
        let mut b;

        if bestv != -1 {
            let bv = bestv as usize;
            // Choose multipliers such that the multiplier for the (in)equality
            // cons0 is positive.
            if c1.vals[ix.commonidx1[bv]] > 0.0 {
                a = c1.vals[ix.commonidx1[bv]];
                b = -c0.vals[ix.commonidx0[bv]];
            } else {
                a = -c1.vals[ix.commonidx1[bv]];
                b = c0.vals[ix.commonidx0[bv]];
            }
        } else {
            if c1.vals[ix.commonidx1[0]] > 0.0 {
                a = c1.vals[ix.commonidx1[0]];
                b = -c0.vals[ix.commonidx0[0]];
            } else {
                a = -c1.vals[ix.commonidx1[0]];
                b = c0.vals[ix.commonidx0[0]];
            }

            // If a/b is integral, then we can easily choose integer multipliers.
            if scip.set.is_integral(a / b) {
                if a / b > 0.0 {
                    a /= b;
                    b = 1.0;
                } else {
                    a /= -b;
                    b = -1.0;
                }
            }
        }

        // Calculate the common coefficients, if we have not recognized linear
        // dependency.
        let mut newvars = Vec::new();
        let mut newvals = Vec::new();
        if !commonvarlindependent {
            for i in 0..nvarscommon {
                let aggrcoef = a * c0.vals[ix.commonidx0[i]] + b * c1.vals[ix.commonidx1[i]];
                if !scip.set.is_zero(aggrcoef) {
                    newvars.push(c0.vars[ix.commonidx0[i]]);
                    newvals.push(aggrcoef);
                }
            }
        }
        // Otherwise the aggregation coefficient should be 0.0 for every common
        // variable.

        // Calculate the coefficients appearing in cons0 but not in cons1.
        for i in 0..(c0.nvars as usize - nvarscommon) {
            newvars.push(c0.vars[ix.diffidx0minus1[i]]);
            newvals.push(a * c0.vals[ix.diffidx0minus1[i]]);
        }

        // Calculate the coefficients appearing in cons1 but not in cons0.
        for i in 0..(c1.nvars as usize - nvarscommon) {
            newvars.push(c1.vars[ix.diffidx1minus0[i]]);
            newvals.push(b * c1.vals[ix.diffidx1minus0[i]]);
        }

        // Calculate the new left and right hand side of the (in)equality.
        let newlhs = if scip.set.is_infinity(-c0.lhs) { -scip.set.infinity() } else { a * c0.lhs + b * c1.lhs };
        let newrhs = if scip.set.is_infinity(c0.rhs) { scip.set.infinity() } else { a * c0.rhs + b * c1.rhs };
        let upgraded0 = c0.upgraded;
        let nchg = c0.nvars + c1.nvars - ix.nvarscommon;

        // Create the new linear constraint.
        let k = &scip.conss[cons0];
        let name = k.name.clone();
        let flags = (
            k.is_initial(),
            k.is_separated(),
            k.is_enforced(),
            k.is_checked(),
            k.is_propagated(),
            k.is_local(),
            k.modifiable,
            k.is_dynamic(),
            k.is_removable(),
            k.is_sticking_at_node(),
        );
        let mut newcons = create_cons_linear(
            scip, &name, &newvars, &newvals, newlhs, newrhs, flags.0, flags.1, flags.2, flags.3, flags.4, flags.5, flags.6, flags.7, flags.8, flags.9,
        )?;

        // Copy the upgraded flag from the old cons0 to the new constraint.
        cdm(scip, newcons).upgraded = upgraded0;

        infeasible = normalize_cons(scip, newcons)?;

        // Check, if we really want to use the new constraint instead of the old
        // one: use the new one, if the maximum norm doesn't grow too much.
        let newmax = consdata_get_max_absval(cdm(scip, newcons));
        let oldmax = consdata_get_max_absval(cdm(scip, cons0));
        if !infeasible && newmax <= maxaggrnormscale * oldmax {
            // Update the statistics: we changed all coefficients.
            if !upgraded0 {
                *nchgcoefs += nchg;
            }
            aggregated = true;

            // Add the new linear constraint to the problem and delete the old
            // constraint.
            scip_cons::add_cons_upgrade(scip, cons0, &mut newcons)?;
            scip_cons::del_cons(scip, cons0)?;
        } else {
            scip_cons::release_cons(scip, &mut newcons)?;
        }
    }

    Ok((aggregated, infeasible))
}

/// The maximum absolute coefficient `consdataGetMaxAbsval` returns, without
/// caching it.
fn peek_max_absval(c: &ConsData) -> f64 {
    if c.validmaxabsval {
        return c.maxabsval;
    }
    let mut maxabsval = 0.0;
    for i in 0..c.nvars as usize {
        let absval = c.vals[i].abs();
        if absval > maxabsval {
            maxabsval = absval;
        }
    }
    maxabsval
}

/// The keys of the parallel constraint table: `hashGetKeyLinearcons`,
/// `hashKeyEqLinearcons`, `hashKeyValLinearcons`. C's comparison caches
/// the maximum absolute coefficients it computes; this records the
/// constraints so [`LinconsHash::cache`] can do the same afterwards.
struct LinconsHash<'a> {
    scip: &'a Scip,
    touched: std::cell::RefCell<Vec<ConsId>>,
}

impl LinconsHash<'_> {
    /// Caches the maximum absolute coefficients the comparisons computed.
    fn cache(touched: Vec<ConsId>, scip: &mut Scip) {
        for cons in touched {
            consdata_get_max_absval(cdm(scip, cons));
        }
    }
}

impl crate::hash::HashOps<ConsId> for LinconsHash<'_> {
    type Key = ConsId;

    // The key is the element itself.
    fn get_key(&self, element: &ConsId) -> ConsId {
        *element
    }

    // Returns TRUE iff both keys are equal; two constraints are equal if they
    // have the same variables and the coefficients are either equal or
    // negated.
    fn key_eq(&self, key1: &ConsId, key2: &ConsId) -> bool {
        let scip = self.scip;
        let mut consdata1 = cd(scip, *key1);
        let mut consdata2 = cd(scip, *key2);

        // If it is the same constraint we dont need to check anything.
        if key1 == key2 {
            return true;
        }

        // Checks trivial case.
        if consdata1.nvars != consdata2.nvars {
            return false;
        }

        // Tests if variables are equal.
        for i in 0..consdata1.nvars {
            if consdata1.vars[i] != consdata2.vars[i] {
                return false;
            }
        }

        // Order by maxabsval.
        self.touched.borrow_mut().extend([*key1, *key2]);
        if peek_max_absval(consdata1) > peek_max_absval(consdata2) {
            std::mem::swap(&mut consdata1, &mut consdata2);
        }

        // Initialize extremal scales.
        let mut minscale = scip.set.infinity();
        let mut maxscale = -scip.set.infinity();

        // Test if coefficient scales are equal.
        for i in 0..consdata1.nvars {
            let scale = consdata2.vals[i] / consdata1.vals[i];

            if minscale > scale {
                minscale = scale;

                if scip.set.is_lt(minscale, maxscale) {
                    return false;
                }
            }

            if maxscale < scale {
                maxscale = scale;

                if scip.set.is_lt(minscale, maxscale) {
                    return false;
                }
            }
        }

        true
    }

    // Returns the hash value of the key; using only the variable indices as
    // hash, since the values are compared by epsilon.
    fn key_val(&self, key: &ConsId) -> u64 {
        let c = cd(self.scip, *key);
        let minidx = self.scip.vars[c.vars[0usize]].index;
        let mididx = self.scip.vars[c.vars[c.nvars / 2]].index;
        let maxidx = self.scip.vars[c.vars[c.nvars - 1]].index;

        crate::misc::hash_four(c.nvars as u32, minidx as u32, mididx as u32, maxidx as u32) as u64
    }
}

/// `getParallelConsKey`: returns the key for deciding which of two parallel
/// constraints should be kept (smaller key should be kept); prefers
/// non-upgraded constraints and as second criterion the constraint with the
/// smallest position.
fn get_parallel_cons_key(scip: &Scip, cons: ConsId) -> u32 {
    ((cd(scip, cons).upgraded as u32) << 31).wrapping_add(scip.conss[cons].consspos as u32)
}

/// `retrieveParallelConstraints`: updates the hashtable such that out of
/// all constraints in the hashtable that are detected to be parallel to
/// `querycons`, only one is kept in the hashtable and stored into
/// `querycons`, and all others are removed from the hashtable and returned.
fn retrieve_parallel_constraints(scip: &mut Scip, hashtable: &mut crate::hash::HashTable<ConsId>, querycons: &mut ConsId) -> Vec<ConsId> {
    let mut parallelconss = Vec::new();
    let mut querykey = get_parallel_cons_key(scip, *querycons);

    loop {
        let ops = LinconsHash {
            scip,
            touched: Default::default(),
        };
        let found = hashtable.retrieve(&ops, querycons);
        let touched = ops.touched.into_inner();
        LinconsHash::cache(touched, scip);
        let Some(parallelcons) = found else { break };

        let conskey = get_parallel_cons_key(scip, parallelcons);

        if conskey < querykey {
            parallelconss.push(*querycons);
            *querycons = parallelcons;
            querykey = conskey;
        } else {
            parallelconss.push(parallelcons);
        }

        // If the constraint that just came out of the hash table is the one that
        // is kept, we do not need to look into the hashtable again, since the
        // invariant is that in the hashtable only pair-wise non-parallel
        // constraints are contained. For the original querycons, however,
        // multiple constraints that compare equal (=parallel) could be
        // contained due to non-transitivity of the equality comparison. Also we
        // can return immediately, since parallelcons is already contained in the
        // hashtable and we do not need to remove and reinsert it.
        if *querycons == parallelcons {
            return parallelconss;
        }

        // Remove parallelcons from the hashtable, since it will be replaced by
        // querycons.
        let ops = LinconsHash {
            scip,
            touched: Default::default(),
        };
        hashtable.remove(&ops, &parallelcons);
        let touched = ops.touched.into_inner();
        LinconsHash::cache(touched, scip);
    }

    let ops = LinconsHash {
        scip,
        touched: Default::default(),
    };
    hashtable.insert(&ops, *querycons);
    let touched = ops.touched.into_inner();
    LinconsHash::cache(touched, scip);

    parallelconss
}

/// `detectRedundantConstraints`: compares each constraint with all other
/// constraints for possible redundancy and removes or changes constraint
/// accordingly; in contrast to preprocessConstraintPairs(), it uses a hash
/// table.
fn detect_redundant_constraints(scip: &mut Scip, conss: &[ConsId], firstchange: &mut i32, cutoff: &mut bool, ndelconss: &mut i32, _nchgsides: &mut i32) -> ScipResult {
    // Create a hash table for the constraint set.
    let mut hashtable = crate::hash::HashTable::<ConsId>::new(conss.len() as i32);

    // Check all constraints in the given set for redundancy.
    for (c, &cons) in conss.iter().enumerate() {
        let mut cons0 = cons;

        if !scip.conss[cons0].is_active() || scip.conss[cons0].modifiable {
            continue;
        }

        // Do not check for parallel constraints if they should not be upgraded.
        if scip.conss[cons0].nupgradelocks > 0 {
            continue;
        }

        // Check for interuption.
        if c % 1000 == 0 && crate::solve::solve_is_stopped(scip, false) {
            break;
        }

        // Sorts the constraint.
        consdata_sort(scip, cons0);

        // Get constraints from current hash table with same variables as cons0
        // and with coefficients equal to the ones of cons0 when both are scaled
        // such that maxabsval is 1.0 and the coefficient of the first variable
        // is positive. Also inserts cons0 into the hashtable.
        let parallelconss = retrieve_parallel_constraints(scip, &mut hashtable, &mut cons0);

        if !parallelconss.is_empty() {
            // cons0 may have been changed in retrieveParallelConstraints().
            let mut lhs = cd(scip, cons0).lhs;
            let mut rhs = cd(scip, cons0).rhs;

            for &consdel in &parallelconss {
                // Do not delete constraint if it should not be upgraded.
                if scip.conss[consdel].nupgradelocks > 0 {
                    continue;
                }

                // Constraint found: create a new constraint with same coefficients
                // and best left and right hand side; delete old constraints
                // afterwards.
                let del = cd(scip, consdel);
                let scale = cd(scip, cons0).vals[0usize] / del.vals[0usize];
                let (dellhs, delrhs, delupgraded) = (del.lhs, del.rhs, del.upgraded);

                if scale > 0.0 {
                    // The coefficients of both constraints are parallel with a
                    // positive scale.
                    if !scip.set.is_infinity(-dellhs) {
                        lhs = crate::def::max(scale * dellhs, lhs);
                    }
                    if !scip.set.is_infinity(delrhs) {
                        rhs = crate::def::min(scale * delrhs, rhs);
                    }
                } else {
                    // The coefficients of both rows are negations.
                    if !scip.set.is_infinity(delrhs) {
                        lhs = crate::def::max(scale * delrhs, lhs);
                    }
                    if !scip.set.is_infinity(-dellhs) {
                        rhs = crate::def::min(scale * dellhs, rhs);
                    }
                }

                // Update flags of constraint which caused the redundancy s.t.
                // nonredundant information doesn't get lost.
                scip_cons::update_cons_flags(scip, cons0, consdel)?;

                // Delete consdel.
                scip_cons::del_cons(scip, consdel)?;
                if !delupgraded {
                    *ndelconss += 1;
                }
            }

            if scip.set.is_feas_lt(rhs, lhs) {
                *cutoff = true;
                break;
            }

            // Ensure that lhs <= rhs holds without tolerances as we only allow such
            // rows to enter the LP.
            if lhs > rhs {
                rhs = (lhs + rhs) / 2.0;
                lhs = rhs;
            }

            // Update lhs and rhs of cons0.
            chg_lhs(scip, cons0, lhs)?;
            chg_rhs(scip, cons0, rhs)?;

            // Update the first changed constraint to begin the next aggregation
            // round with.
            if cd(scip, cons0).changed && scip.conss[cons0].consspos < *firstchange {
                *firstchange = scip.conss[cons0].consspos;
            }
        }
    }

    Ok(())
}

/// `preprocessConstraintPairs`: compares constraint with all prior
/// constraints for possible redundancy or aggregation, and removes or
/// changes constraint accordingly; removed constraints become `NONE` in
/// `conss`.
#[allow(clippy::too_many_arguments)]
fn preprocess_constraint_pairs(
    scip: &mut Scip,
    conss: &mut [ConsId],
    firstchange: i32,
    chkind: i32,
    maxaggrnormscale: f64,
    cutoff: &mut bool,
    ndelconss: &mut i32,
    nchgsides: &mut i32,
    nchgcoefs: &mut i32,
) -> ScipResult {
    // Get the constraint to be checked against all prior constraints.
    let cons0 = conss[chkind as usize];
    let mut cons0isequality = scip.set.is_eq(cd(scip, cons0).lhs, cd(scip, cons0).rhs);

    // Sort the constraint.
    consdata_sort(scip, cons0);

    // Calculate bit signatures of cons0 for potentially positive and negative
    // coefficients.
    with_cd(scip, cons0, |scip, c| consdata_calc_signatures(scip, c));
    let possignature0 = cd(scip, cons0).possignature;
    let negsignature0 = cd(scip, cons0).negsignature;

    // Temporary memory for indices of common variables.
    let mut commonidx0: Vec<i32> = Vec::new();
    let mut commonidx1: Vec<i32> = Vec::new();
    let mut diffidx0minus1: Vec<i32> = Vec::new();
    let mut diffidx1minus0: Vec<i32> = Vec::new();

    let mut cons0lhs = cd(scip, cons0).lhs;
    let mut cons0rhs = cd(scip, cons0).rhs;
    let cons0upgraded = cd(scip, cons0).upgraded;

    // Check constraint against all prior constraints.
    let cons0changed = cd(scip, cons0).changed;
    cdm(scip, cons0).changed = false;
    let mut c = if cons0changed { 0 } else { firstchange };
    while c < chkind && !*cutoff && conss[chkind as usize] != NONE {
        let cons1 = conss[c as usize];

        // cons1 has become inactive during presolving of constraint pairs.
        if cons1 == NONE {
            c += 1;
            continue;
        }

        // If both constraints didn't change since last pair processing, we can
        // ignore the pair; if both constraints are already upgraded, skip the
        // pair; because changes on these constraints cannot be applied to the
        // instance anymore.
        if (!cons0changed && !cd(scip, cons1).changed) || (cons0upgraded && cd(scip, cons1).upgraded) {
            c += 1;
            continue;
        }

        // Sort the constraint.
        consdata_sort(scip, cons1);

        // Calculate bit signatures of cons1 for potentially positive and
        // negative coefficients.
        with_cd(scip, cons1, |scip, c| consdata_calc_signatures(scip, c));
        let possignature1 = cd(scip, cons1).possignature;
        let negsignature1 = cd(scip, cons1).negsignature;

        // The signatures give a quick test to check for domination and equality
        // of coefficients.
        let mut coefsequal = possignature0 == possignature1 && negsignature0 == negsignature1;
        let mut coefsnegated = possignature0 == negsignature1 && negsignature0 == possignature1;
        let (lhs1, rhs1) = (cd(scip, cons1).lhs, cd(scip, cons1).rhs);
        // possignature0 <= possignature1 (as bit vector), negsignature0 >=
        // negsignature1 (as bit vector), and so on.
        let mut cons0dominateslhs = scip.set.is_ge(cons0lhs, lhs1) && (possignature0 | possignature1) == possignature1 && (negsignature0 | negsignature1) == negsignature0;
        let mut cons1dominateslhs = scip.set.is_ge(lhs1, cons0lhs) && (possignature0 | possignature1) == possignature0 && (negsignature0 | negsignature1) == negsignature1;
        let mut cons0dominatesrhs = scip.set.is_le(cons0rhs, rhs1) && (possignature0 | possignature1) == possignature0 && (negsignature0 | negsignature1) == negsignature1;
        let mut cons1dominatesrhs = scip.set.is_le(rhs1, cons0rhs) && (possignature0 | possignature1) == possignature1 && (negsignature0 | negsignature1) == negsignature0;
        let mut cons1isequality = scip.set.is_eq(lhs1, rhs1);
        let tryaggregation = (cons0isequality || cons1isequality) && maxaggrnormscale > 0.0;
        if !cons0dominateslhs && !cons1dominateslhs && !cons0dominatesrhs && !cons1dominatesrhs && !coefsequal && !coefsnegated && !tryaggregation {
            c += 1;
            continue;
        }

        // Check consdata0 against consdata1:
        // - if lhs0 >= lhs1 and for each variable v and each solution value x_v
        //   val0[v]*x_v <= val1[v]*x_v, consdata0 dominates consdata1 w.r.t.
        //   left hand side
        // - if rhs0 <= rhs1 and for each variable v and each solution value x_v
        //   val0[v]*x_v >= val1[v]*x_v, consdata0 dominates consdata1 w.r.t.
        //   right hand side
        // - if val0[v] == -val1[v] for all variables v, the two inequalities can
        //   be replaced by a single ranged row (or equality)
        // - if at least one constraint is an equality, count the weighted number
        //   of common variables W_c and the weighted number of variable in the
        //   difference sets W_0 = w(V_0 \ V_1), W_1 = w(V_1 \ V_0), where the
        //   weight of each variable depends on its type, such that aggregations
        //   in order to remove the number of continuous and integer variables
        //   are preferred:
        //   - if W_c > W_1, try to aggregate  consdata0 := a * consdata0 + b *
        //     consdata1  in order to decrease the variable weight in consdata0,
        //     where a = +/- val1[v] and b = -/+ val0[v] for common v which leads
        //     to the smallest weight; for numerical stability, we will only
        //     accept integral a and b; the sign of a has to be positive to not
        //     switch the sense of the (in)equality cons0
        //   - if W_c > W_0, try to aggregate  consdata1 := a * consdata1 + b *
        //     consdata0  in order to decrease the variable weight in consdata1,
        //     where a = +/- val0[v] and b = -/+ val1[v] for common v which leads
        //     to the smallest weight; for numerical stability, we will only
        //     accept integral a and b; the sign of a has to be positive to not
        //     switch the sense of the (in)equality cons1

        // Check consdata0 against consdata1 for redundancy, or ranged row
        // accumulation.
        commonidx0.clear();
        commonidx1.clear();
        diffidx0minus1.clear();
        diffidx1minus0.clear();
        let mut commonidxweight = 0;
        let mut diffidx0minus1weight = 0;
        let mut diffidx1minus0weight = 0;
        let mut v0 = 0;
        let mut v1 = 0;
        {
            let c0 = cd(scip, cons0);
            let c1 = cd(scip, cons1);
            while (v0 < c0.nvars || v1 < c1.nvars)
                && (cons0dominateslhs || cons1dominateslhs || cons0dominatesrhs || cons1dominatesrhs || coefsequal || coefsnegated || tryaggregation)
            {
                // Test, if variable appears in only one or in both constraints.
                let varcmp = if v0 < c0.nvars && v1 < c1.nvars {
                    var_compare(scip, c0.vars[v0], c1.vars[v1])
                } else if v0 < c0.nvars {
                    -1
                } else {
                    1
                };

                let var;
                let val0;
                let val1;
                match varcmp {
                    -1 => {
                        // Variable doesn't appear in consdata1.
                        var = c0.vars[v0];
                        val0 = c0.vals[v0];
                        val1 = 0.0;
                        if tryaggregation {
                            diffidx0minus1.push(v0);
                            diffidx0minus1weight += get_var_weight(scip, var);
                        }
                        v0 += 1;
                        coefsequal = false;
                        coefsnegated = false;
                    }
                    1 => {
                        // Variable doesn't appear in consdata0.
                        var = c1.vars[v1];
                        val0 = 0.0;
                        val1 = c1.vals[v1];
                        if tryaggregation {
                            diffidx1minus0.push(v1);
                            diffidx1minus0weight += get_var_weight(scip, var);
                        }
                        v1 += 1;
                        coefsequal = false;
                        coefsnegated = false;
                    }
                    0 => {
                        // Variable appears in both constraints.
                        var = c0.vars[v0];
                        val0 = c0.vals[v0];
                        val1 = c1.vals[v1];
                        if tryaggregation {
                            commonidx0.push(v0);
                            commonidx1.push(v1);
                            commonidxweight += get_var_weight(scip, var);
                        }
                        v0 += 1;
                        v1 += 1;
                        coefsequal = coefsequal && scip.set.is_eq(val0, val1);
                        coefsnegated = coefsnegated && scip.set.is_eq(val0, -val1);
                    }
                    _ => panic!("invalid comparison result"),
                }

                // Update domination criteria w.r.t. the coefficient and the
                // variable's bounds.
                let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
                if scip.set.is_gt(val0, val1) {
                    if scip.set.is_negative(glb) {
                        cons0dominatesrhs = false;
                        cons1dominateslhs = false;
                    }
                    if scip.set.is_positive(gub) {
                        cons0dominateslhs = false;
                        cons1dominatesrhs = false;
                    }
                } else if scip.set.is_lt(val0, val1) {
                    if scip.set.is_negative(glb) {
                        cons0dominateslhs = false;
                        cons1dominatesrhs = false;
                    }
                    if scip.set.is_positive(gub) {
                        cons0dominatesrhs = false;
                        cons1dominateslhs = false;
                    }
                }
            }
        }

        // Check for disaggregated ranged rows.
        if coefsequal || coefsnegated {
            // The coefficients in both rows are either equal or negated: create a
            // new constraint with same coefficients and best left and right hand
            // sides; delete the old constraints afterwards.
            let (c0lhs, c0rhs) = (cd(scip, cons0).lhs, cd(scip, cons0).rhs);
            let (c1lhs, c1rhs) = (cd(scip, cons1).lhs, cd(scip, cons1).rhs);
            let (lhs, rhs) = if coefsequal {
                // The coefficients of both rows are equal.
                (crate::def::max(c0lhs, c1lhs), crate::def::min(c0rhs, c1rhs))
            } else {
                // The coefficients of both rows are negations.
                (crate::def::max(c0lhs, -c1rhs), crate::def::min(c0rhs, -c1lhs))
            };
            if scip.set.is_feas_lt(rhs, lhs) {
                *cutoff = true;
                break;
            }

            // Check which constraint has to stay; changes applied to an upgraded
            // constraint will not be considered in the instance.
            let (consstay, consdel, consinddel) = if cd(scip, cons0).upgraded { (cons1, cons0, chkind) } else { (cons0, cons1, c) };

            // Update the sides of consstay.
            chg_lhs(scip, consstay, lhs)?;
            chg_rhs(scip, consstay, rhs)?;
            if !cd(scip, cons0).upgraded {
                cons0lhs = cd(scip, cons0).lhs;
                cons0rhs = cd(scip, cons0).rhs;
            }

            // Update flags of constraint which caused the redundancy s.t.
            // nonredundant information doesn't get lost.
            scip_cons::update_cons_flags(scip, consstay, consdel)?;

            // Delete consdel.
            scip_cons::del_cons(scip, consdel)?;
            conss[consinddel as usize] = NONE;
            if !cd(scip, consdel).upgraded {
                *ndelconss += 1;
            }

            c += 1;
            continue;
        }

        // Check for domination: remove dominated sides, but don't touch
        // equalities as long as they are not totally redundant.
        if cons1dominateslhs && (!cons0isequality || cons1dominatesrhs || scip.set.is_infinity(cd(scip, cons0).rhs)) {
            // Left hand side is dominated by consdata1: delete left hand side of
            // consdata0; check for infeasibility.
            if scip.set.is_feas_gt(cd(scip, cons1).lhs, cd(scip, cons0).rhs) {
                *cutoff = true;
                break;
            }

            // Remove redundant left hand side.
            if !scip.set.is_infinity(-cd(scip, cons0).lhs) {
                chg_lhs(scip, cons0, -scip.set.infinity())?;
                cons0lhs = cd(scip, cons0).lhs;
                cons0isequality = false;
                if !cd(scip, cons0).upgraded {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons1, cons0)?;
                    *nchgsides += 1;
                }
            }
        } else if cons0dominateslhs && (!cons1isequality || cons0dominatesrhs || scip.set.is_infinity(cd(scip, cons1).rhs)) {
            // Left hand side is dominated by consdata0: delete left hand side of
            // consdata1; check for infeasibility.
            if scip.set.is_feas_gt(cd(scip, cons0).lhs, cd(scip, cons1).rhs) {
                *cutoff = true;
                break;
            }

            // Remove redundant left hand side.
            if !scip.set.is_infinity(-cd(scip, cons1).lhs) {
                chg_lhs(scip, cons1, -scip.set.infinity())?;
                cons1isequality = false;
                if !cd(scip, cons1).upgraded {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons0, cons1)?;
                    *nchgsides += 1;
                }
            }
        }
        if cons1dominatesrhs && (!cons0isequality || cons1dominateslhs || scip.set.is_infinity(-cd(scip, cons0).lhs)) {
            // Right hand side is dominated by consdata1: delete right hand side of
            // consdata0; check for infeasibility.
            if scip.set.is_feas_lt(cd(scip, cons1).rhs, cd(scip, cons0).lhs) {
                *cutoff = true;
                break;
            }

            // Remove redundant right hand side.
            if !scip.set.is_infinity(cd(scip, cons0).rhs) {
                chg_rhs(scip, cons0, scip.set.infinity())?;
                cons0rhs = cd(scip, cons0).rhs;
                cons0isequality = false;
                if !cd(scip, cons0).upgraded {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons1, cons0)?;
                    *nchgsides += 1;
                }
            }
        } else if cons0dominatesrhs && (!cons1isequality || cons0dominateslhs || scip.set.is_infinity(-cd(scip, cons1).lhs)) {
            // Right hand side is dominated by consdata0: delete right hand side of
            // consdata1; check for infeasibility.
            if scip.set.is_feas_lt(cd(scip, cons0).rhs, cd(scip, cons1).lhs) {
                *cutoff = true;
                break;
            }

            // Remove redundant right hand side.
            if !scip.set.is_infinity(cd(scip, cons1).rhs) {
                chg_rhs(scip, cons1, scip.set.infinity())?;
                cons1isequality = false;
                if !cd(scip, cons1).upgraded {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons0, cons1)?;
                    *nchgsides += 1;
                }
            }
        }

        // Check for now redundant constraints.
        if scip.set.is_infinity(-cd(scip, cons0).lhs) && scip.set.is_infinity(cd(scip, cons0).rhs) {
            // consdata0 became redundant.
            scip_cons::del_cons(scip, cons0)?;
            conss[chkind as usize] = NONE;
            if !cd(scip, cons0).upgraded {
                // Update flags of constraint which caused the redundancy s.t.
                // nonredundant information doesn't get lost.
                scip_cons::update_cons_flags(scip, cons1, cons0)?;
                *ndelconss += 1;
            }
            c += 1;
            continue;
        }
        if scip.set.is_infinity(-cd(scip, cons1).lhs) && scip.set.is_infinity(cd(scip, cons1).rhs) {
            // consdata1 became redundant.
            scip_cons::del_cons(scip, cons1)?;
            conss[c as usize] = NONE;
            if !cd(scip, cons1).upgraded {
                // Update flags of constraint which caused the redundancy s.t.
                // nonredundant information doesn't get lost.
                scip_cons::update_cons_flags(scip, cons0, cons1)?;
                *ndelconss += 1;
            }
            c += 1;
            continue;
        }

        // Check, if we want to aggregate an (in)equality with an equality:
        //  consdata0 := a * consdata0 + b * consdata1  or  consdata1 := a *
        //  consdata1 + b * consdata0
        if tryaggregation {
            let mut aggregated = false;
            let nvarscommon = commonidx0.len() as i32;

            if cons1isequality && !cd(scip, cons0).upgraded && commonidxweight > diffidx1minus0weight {
                // W_c > W_1: try to aggregate  consdata0 := a * consdata0 + b *
                // consdata1.
                let ix = AggrIndices {
                    commonidx0: &commonidx0,
                    commonidx1: &commonidx1,
                    diffidx0minus1: &diffidx0minus1,
                    diffidx1minus0: &diffidx1minus0,
                    nvarscommon,
                    commonidxweight,
                    diffidx0minus1weight,
                    diffidx1minus0weight,
                };
                let (agg, infeasible) = aggregate_constraints(scip, cons0, cons1, &ix, maxaggrnormscale, nchgcoefs)?;
                aggregated = agg;
                *cutoff = infeasible;

                if *cutoff {
                    break;
                }

                // Update array of active constraints.
                if aggregated {
                    conss[chkind as usize] = NONE;
                }
            }
            if !aggregated && cons0isequality && !cd(scip, cons1).upgraded && commonidxweight > diffidx0minus1weight {
                // W_c > W_0: try to aggregate  consdata1 := a * consdata1 + b *
                // consdata0.
                let ix = AggrIndices {
                    commonidx0: &commonidx1,
                    commonidx1: &commonidx0,
                    diffidx0minus1: &diffidx1minus0,
                    diffidx1minus0: &diffidx0minus1,
                    nvarscommon,
                    commonidxweight,
                    diffidx0minus1weight: diffidx1minus0weight,
                    diffidx1minus0weight: diffidx0minus1weight,
                };
                let (agg, infeasible) = aggregate_constraints(scip, cons1, cons0, &ix, maxaggrnormscale, nchgcoefs)?;
                aggregated = agg;
                *cutoff = infeasible;

                if *cutoff {
                    break;
                }

                // Update array of active constraints.
                if aggregated {
                    conss[c as usize] = NONE;
                }
            }
        }
        c += 1;
    }

    Ok(())
}

/// `presolStuffing`: do stuffing presolving on a single constraint.
#[allow(clippy::too_many_arguments)]
fn presol_stuffing(scip: &mut Scip, cons: ConsId, singletonstuffing: bool, singlevarstuffing: bool, cutoff: &mut bool, nfixedvars: &mut i32, nchgbds: &mut i32) -> ScipResult {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};

    // We only want to run for inequalities.
    if !scip.set.is_infinity(cd(scip, cons).rhs) && !scip.set.is_infinity(-cd(scip, cons).lhs) {
        return Ok(());
    }

    let mut minactivity;
    let mut maxactivity;
    let isminsettoinfinity;
    let mut ismaxsettoinfinity;
    if singlevarstuffing {
        let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_bounds(scip, c, false));
        minactivity = min.act;
        maxactivity = max.act;
        isminsettoinfinity = min.issettoinfinity;
        ismaxsettoinfinity = max.issettoinfinity;
    } else {
        minactivity = INVALID;
        maxactivity = INVALID;
        isminsettoinfinity = false;
        ismaxsettoinfinity = false;
    }
    let _ = &mut minactivity;

    // We want to have a <= constraint, if the rhs is infinite, we implicitly
    // multiply the constraint by -1, the new maxactivity is minus the old
    // minactivity then.
    let rhs;
    let factor;
    if scip.set.is_infinity(cd(scip, cons).rhs) {
        rhs = -cd(scip, cons).lhs;
        factor = -1.0;
        maxactivity = -minactivity;
        ismaxsettoinfinity = isminsettoinfinity;
    } else {
        rhs = cd(scip, cons).rhs;
        factor = 1.0;
    }

    let nvars = cd(scip, cons).nvars;
    let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
    let vals = cd(scip, cons).vals.0[..nvars as usize].to_vec();
    let nlocks = |scip: &Scip, var: VarId| locksup(scip, var, LockType::Model) + locksdown(scip, var, LockType::Model);

    // Check for continuous singletons.
    let mut v = nvars as usize;
    if singletonstuffing {
        v = 0;
        while v < nvars as usize {
            let var = vars[v];
            if !var_is_integral(scip, var) && nlocks(scip, var) == 1 {
                break;
            }
            v += 1;
        }
    }

    // A singleton was found -> perform singleton variable stuffing.
    if v < nvars as usize {
        let n = nvars as usize;
        let mut varpos = vec![0i32; n];
        let mut ratios = vec![0.0; n];
        let mut swapped = vec![false; n];
        let mut tryfixing = true;
        let mut nsingletons = 0;
        let mut mincondactivity = 0.0;
        let mut maxcondactivity = 0.0;

        for v in 0..n {
            let var = vars[v];
            let lb = scip.vars[var].glbdom.lb;
            let ub = scip.vars[var].glbdom.ub;
            let obj = scip.vars[var].obj;
            let val = factor * vals[v];

            // The variable is a singleton and continuous.
            if !var_is_integral(scip, var) && nlocks(scip, var) == 1 {
                if scip.set.is_negative(obj) && val > 0.0 {
                    // Case 1: obj < 0 and coef > 0.
                    if scip.set.is_infinity(-lb) {
                        tryfixing = false;
                        break;
                    }

                    maxcondactivity += val * lb;
                    mincondactivity += val * lb;
                    swapped[v] = false;
                    ratios[nsingletons] = obj / val;
                    varpos[nsingletons] = v as i32;
                    nsingletons += 1;
                } else if scip.set.is_positive(obj) && val < 0.0 {
                    // Case 2: obj > 0 and coef < 0.
                    if scip.set.is_infinity(ub) {
                        tryfixing = false;
                        break;
                    }

                    // Multiply column by (-1) to become case 1. Now bounds are
                    // swapped: ub := -lb, lb := -ub.
                    maxcondactivity += val * ub;
                    mincondactivity += val * ub;
                    swapped[v] = true;
                    ratios[nsingletons] = obj / val;
                    varpos[nsingletons] = v as i32;
                    nsingletons += 1;
                } else if val > 0.0 {
                    // Case 3: obj >= 0 and coef >= 0 is handled by duality fixing.
                    // We only consider the lower bound for the constants.
                    if scip.set.is_infinity(-lb) {
                        // Maybe unbounded.
                        tryfixing = false;
                        break;
                    }

                    maxcondactivity += val * lb;
                    mincondactivity += val * lb;
                } else {
                    // Case 4: obj <= 0 and coef <= 0 is also handled by duality
                    // fixing. We only consider the upper bound for the constants.
                    if scip.set.is_infinity(ub) {
                        // Maybe unbounded.
                        tryfixing = false;
                        break;
                    }

                    maxcondactivity += val * ub;
                    mincondactivity += val * ub;
                }
            } else {
                // Consider contribution of discrete variables, non-singleton
                // continuous variables and variables with more than one lock.
                if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
                    tryfixing = false;
                    break;
                }

                if val > 0.0 {
                    maxcondactivity += val * ub;
                    mincondactivity += val * lb;
                } else {
                    maxcondactivity += val * lb;
                    mincondactivity += val * ub;
                }
            }
        }
        if tryfixing && nsingletons > 0 && (scip.set.is_gt(rhs, maxcondactivity) || scip.set.is_le(rhs, mincondactivity)) {
            sort::sort(&mut ratios[..nsingletons], (&mut varpos[..nsingletons],), sort::cmp_real);

            // Verify which singleton continuous variables can be fixed.
            for v in 0..nsingletons {
                let idx = varpos[v] as usize;
                let var = vars[idx];
                let val = (factor * vals[idx]).abs();
                let lb = scip.vars[var].glbdom.lb;
                let ub = scip.vars[var].glbdom.ub;

                // Stop fixing if variable bounds are not finite.
                if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
                    break;
                }

                // Calculate the change in the row activities if this variable
                // changes its value from its worst to its best bound.
                let delta = if swapped[idx] { -(lb - ub) * val } else { (ub - lb) * val };

                if scip.set.is_le(delta, rhs - maxcondactivity) {
                    let (infeasible, tightened) = scip_var::fix_var(scip, var, if swapped[idx] { lb } else { ub })?;
                    *cutoff = infeasible;

                    if *cutoff {
                        break;
                    }
                    if tightened {
                        *nfixedvars += 1;
                    }
                }
                // We could in theory tighten the bound of the first singleton
                // variable which does not fall into the above case, since it
                // cannot be fully fixed. However, this is not needed and should be
                // done by activity-based bound tightening anyway after all other
                // continuous singleton columns were fixed; doing it here may
                // introduce numerical troubles in case of large bounds.
                else if scip.set.is_le(rhs, mincondactivity) {
                    let (infeasible, tightened) = scip_var::fix_var(scip, var, if swapped[idx] { ub } else { lb })?;
                    *cutoff = infeasible;

                    if *cutoff {
                        break;
                    }
                    if tightened {
                        *nfixedvars += 1;
                    }
                }

                maxcondactivity += delta;
                mincondactivity += delta;
            }
        }
    }

    // Perform single-variable stuffing:
    // for a linear inequality
    //  a_1 x_1 + a_2 x_2 + ... + a_n x_n <= b
    // with a_i > 0 and objective coefficients c_i < 0, setting all variables to
    // their upper bound (giving us the maximal activity of the constraint) is
    // worst w.r.t. feasibility of the constraint. On the other hand, this gives
    // the best objective function contribution of the variables contained in
    // the constraint. The maximal activity should be larger than the rhs,
    // otherwise the constraint is redundant. Now we are searching for a
    // variable x_k with maximal ratio c_k / a_k (note that all these ratios
    // are negative), so that by reducing the value of this variable we reduce
    // the activity of the constraint while having the smallest objective
    // deterioration per activity unit. If x_k has no downlocks, is continuous,
    // and can be reduced enough to render the constraint feasible, and ALL
    // other variables have only the one uplock installed by the current
    // constraint, we can reduce the upper bound of x_k such that the
    // maxactivity equals the rhs and fix all other variables to their upper
    // bound. Note that the others variables may have downlocks from other
    // constraints, which we do not need to care about since we are setting
    // them to the highest possible value. Also, they may be integer or binary,
    // because the computed ratio is still a lower bound on the change in the
    // objective caused by reducing those variable to reach constraint
    // feasibility. On the other hand, uplocks on x_k from other constraint do
    // no interfer with the method. With a slight adjustment, the procedure
    // even works for integral x_k. If (maxactivity - rhs)/val is integral, the
    // variable gets an integral value in order to fulfill the constraint
    // tightly, and we can just apply the procedure. If (maxactivity -
    // rhs)/val is fractional, we need to check, if overfulfilling the
    // constraint by setting x_k to ceil((maxactivity - rhs)/val) is still
    // better than setting x_k to ceil((maxactivity - rhs)/val) - 1 and filling
    // the remaining gap in the constraint with the next-best variable. For
    // this, we check that c_k * ceil((maxactivity - rhs)/val) is still better
    // than c_k * floor((maxactivity - rhs)/val) + c_j * ((maxactivity - rhs) -
    // (floor((maxactivity - rhs)/val) * val))/a_j. In this case, the upper
    // bound of x_k is decreased to ub_k - ceil(maxactivity - rhs). If there
    // are variables with a_i < 0 and c_i > 0, they are negated to obtain the
    // above form, variables with same sign of coefficients in constraint and
    // objective prevent the use of this method.
    if singlevarstuffing && !ismaxsettoinfinity {
        let mut bestratio = -scip.set.infinity();
        let mut secondbestratio = -scip.set.infinity();
        let mut bestindex: i32 = -1;
        let mut bestuplocks = 0;
        let mut bestdownlocks = 1;

        // Loop over all variables to identify the best and second-best ratio.
        for v in 0..nvars as usize {
            let var = vars[v];
            let obj = scip.vars[var].obj;
            let val = factor * vals[v];

            let ratio = obj / val;

            // If both objective and constraint push the variable to the same
            // direction, we can do nothing here.
            if !scip.set.is_negative(ratio) {
                bestindex = -1;
                break;
            }

            let (downlocks, uplocks) = if val > 0.0 {
                (locksdown(scip, var, LockType::Model), locksup(scip, var, LockType::Model))
            } else {
                (locksup(scip, var, LockType::Model), locksdown(scip, var, LockType::Model))
            };

            // Better ratio, update best candidate.
            if ratio > bestratio
                || (downlocks == 0 && ratio == bestratio && (bestdownlocks > 0 || (!var_is_integral(scip, var) && var_is_integral(scip, vars[bestindex as usize]))))
            {
                // Best index becomes second-best.
                if bestindex != -1 {
                    // Second-best index must not have more than 1 uplock.
                    if bestuplocks > 1 {
                        bestindex = -1;
                        break;
                    } else {
                        secondbestratio = bestratio;
                    }
                }
                bestdownlocks = downlocks;
                bestuplocks = uplocks;
                bestratio = ratio;
                bestindex = v as i32;

                // If this variable is the best in the end, we cannot do reductions
                // since it has a downlocks, if it is not the best, it has too many
                // uplocks -> not applicable.
                if bestdownlocks > 0 && bestuplocks > 1 {
                    bestindex = -1;
                    break;
                }
            } else {
                // Non-best index must not have more than 1 uplock.
                if uplocks > 1 {
                    bestindex = -1;
                    break;
                }
                // Update second-best ratio.
                if ratio > secondbestratio {
                    secondbestratio = ratio;
                }
            }
        }

        // Check if we can apply single variable stuffing.
        if bestindex != -1 && bestdownlocks == 0 {
            let mut tightened = false;
            let bi = bestindex as usize;
            let var = vars[bi];
            let obj = scip.vars[var].obj;
            let val = factor * vals[bi];
            let lb = scip.vars[var].glbdom.lb;
            let ub = scip.vars[var].glbdom.ub;
            let mut tryfixing = true;
            let bounddelta;

            if val < 0.0 {
                // The best variable is integer, and we need to overfulfill the
                // constraint when using just the variable.
                if var_is_integral(scip, var) && !scip.set.is_integral((maxactivity - rhs) / val) {
                    let bestvarfloor = scip.set.floor((maxactivity - rhs) / -val);
                    let activitydelta = (maxactivity - rhs) - (bestvarfloor * -val);
                    tryfixing = scip.set.is_le(obj, -activitydelta * secondbestratio);

                    bounddelta = scip.set.ceil((maxactivity - rhs) / -val);
                } else {
                    bounddelta = (maxactivity - rhs) / -val;
                }

                tryfixing = tryfixing && scip.set.is_le(bounddelta, ub - lb);

                if tryfixing {
                    if scip.set.is_eq(lb + bounddelta, ub) {
                        let (infeasible, t) = scip_var::fix_var(scip, var, lb + bounddelta)?;
                        *cutoff = infeasible;
                        tightened = t;
                    } else {
                        let (infeasible, t) = scip_var::tighten_var_lb(scip, var, lb + bounddelta, false)?;
                        *cutoff = infeasible;
                        tightened = t;
                    }
                }
            } else {
                // The best variable is integer, and we need to overfulfill the
                // constraint when using just the variable.
                if var_is_integral(scip, var) && !scip.set.is_integral((maxactivity - rhs) / val) {
                    let bestvarfloor = scip.set.floor((maxactivity - rhs) / val);
                    let activitydelta = (maxactivity - rhs) - (bestvarfloor * val);
                    tryfixing = scip.set.is_le(-obj, activitydelta * secondbestratio);

                    bounddelta = scip.set.ceil((maxactivity - rhs) / val);
                } else {
                    bounddelta = (maxactivity - rhs) / val;
                }

                tryfixing = tryfixing && scip.set.is_le(bounddelta, ub - lb);

                if tryfixing {
                    if scip.set.is_eq(ub - bounddelta, lb) {
                        let (infeasible, t) = scip_var::fix_var(scip, var, ub - bounddelta)?;
                        *cutoff = infeasible;
                        tightened = t;
                    } else {
                        let (infeasible, t) = scip_var::tighten_var_ub(scip, var, ub - bounddelta, false)?;
                        *cutoff = infeasible;
                        tightened = t;
                    }
                }
            }

            if *cutoff {
                return Ok(());
            }
            if tightened {
                if scip.set.is_eq(scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub) {
                    *nfixedvars += 1;
                } else {
                    *nchgbds += 1;
                }

                for v in 0..nvars as usize {
                    if v == bi {
                        continue;
                    }

                    let x = vars[v];
                    let fixval = if factor * vals[v] < 0.0 { scip.vars[x].glbdom.lb } else { scip.vars[x].glbdom.ub };
                    let (infeasible, t) = scip_var::fix_var(scip, x, fixval)?;
                    *cutoff = infeasible;

                    if *cutoff {
                        return Ok(());
                    }
                    if t {
                        *nfixedvars += 1;
                    }
                }
            }
        }
    }

    Ok(())
}

/// `fullDualPresolve`: applies full dual presolving on variables that only
/// appear in linear constraints.
fn full_dual_presolve(scip: &mut Scip, conss: &[ConsId], cutoff: &mut bool, nchgbds: &mut i32, nchgvartypes: &mut i32) -> ScipResult {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};

    // We calculate redundancy bounds with the following meaning:
    //  redlb[v] == k : if x_v >= k, we can always round x_v down to x_v == k
    //                  without violating any constraint
    //  redub[v] == k : if x_v <= k, we can always round x_v up to x_v == k
    //                  without violating any constraint
    // then:
    //  c_v >= 0 : x_v <= redlb[v] is feasible due to optimality
    //  c_v <= 0 : x_v >= redub[v] is feasible due to optimality
    //
    // Additionally, we detect continuous variables that are implied integral.
    // A continuous variable j is implied integral if it only has only +/-1
    // coefficients, and all constraints (including the bounds as trivial
    // constraints) in which:
    //  c_j > 0: the variable is down-locked,
    //  c_j < 0: the variable is up-locked,
    //  c_j = 0: the variable appears
    // have, apart from j, only integer variables with integral coefficients and
    // integral sides. This is because then, the value of the variable is either
    // determined by one of its bounds or by one of these constraints, and in
    // all cases, the value of the variable is integral.

    // Get active variables.
    let mut nvars = api::get_n_vars(scip);

    // If the problem is a pure binary program, nothing can be achieved by full
    // dual presolve.
    let nbinvars = api::get_n_bin_vars(scip);
    if nbinvars == nvars {
        return Ok(());
    }

    // Get number of continuous variables.
    let ncontvars = api::get_n_cont_vars(scip);
    let nintvars = nvars - ncontvars;

    // Copy the variable array since this array might change during the curse
    // of this algorithm.
    nvars -= nbinvars;
    let vars: Vec<VarId> = api::get_vars(scip)[nbinvars as usize..(nbinvars + nvars) as usize].to_vec();

    // Allocate temporary memory.
    let n = nvars as usize;
    let mut redlb = vec![0.0; n];
    let mut redub = vec![0.0; n];
    let mut nlocksdown = vec![0i32; n];
    let mut nlocksup = vec![0i32; n];
    let mut isimplint = vec![false; ncontvars as usize];
    let mut conscontvars: Vec<VarId> = Vec::with_capacity(ncontvars as usize);

    // Initialize redundancy bounds.
    for v in 0..n {
        redlb[v] = scip.vars[vars[v]].glbdom.lb;
        redub[v] = scip.vars[vars[v]].glbdom.ub;
    }

    // Initialize isimplint array: variable may be implied integral if rounded
    // to their best bound they are integral; we better not use
    // SCIPisFeasIntegral() in these checks.
    for v in 0..ncontvars as usize {
        let var = vars[v + (nintvars - nbinvars) as usize];
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;
        let obj = scip.vars[var].obj;
        isimplint[v] = if scip.set.is_zero(obj) {
            (scip.set.is_infinity(-lb) || scip.set.is_integral(lb)) && (scip.set.is_infinity(ub) || scip.set.is_integral(ub))
        } else if scip.set.is_positive(obj) {
            scip.set.is_infinity(-lb) || scip.set.is_integral(lb)
        } else {
            scip.set.is_infinity(ub) || scip.set.is_integral(ub)
        };
    }

    // Scan all constraints.
    for &cons in conss {
        // We only need to consider constraints that have been locked (i.e.,
        // checked constraints or constraints that are part of checked
        // disjunctions).
        if !scip.conss[cons].is_locked() {
            continue;
        }

        // Get number of times the constraint was locked.
        let nlockspos = scip.conss[cons].get_n_locks_pos();

        // We do not want to include constraints with locked negation (this would
        // be too weird).
        if scip.conss[cons].get_n_locks_neg() > 0 {
            // Mark all non-implied continuous variables.
            for i in 0..cd(scip, cons).nvars {
                let var = cd(scip, cons).vars[i];
                if !var_is_integral(scip, var) {
                    let contv = scip.vars[var].probindex - nintvars;
                    isimplint[contv as usize] = false;
                }
            }
            continue;
        }

        // Check for existing sides.
        let lhsexists = !scip.set.is_infinity(-cd(scip, cons).lhs);
        let rhsexists = !scip.set.is_infinity(cd(scip, cons).rhs);

        // Count locks and update redundancy bounds.
        let mut contvarpos = -1;
        conscontvars.clear();
        let mut hasimpliedpotential = false;
        let modifiable = scip.conss[cons].modifiable;
        let mut integralcoefs = !modifiable;

        for i in 0..cd(scip, cons).nvars {
            let var = cd(scip, cons).vars[i];
            let val = cd(scip, cons).vals[i];

            // Check if still all integer variables have integral coefficients.
            if var_is_integral(scip, var) {
                integralcoefs = integralcoefs && scip.set.is_integral(val);
            }

            // We do not need to process binary variables.
            if var_is_binary(scip, var) {
                continue;
            }

            let mut minresactivity;
            let mut maxresactivity;
            let isminsettoinfinity;
            let ismaxsettoinfinity;
            if modifiable {
                minresactivity = -scip.set.infinity();
                maxresactivity = scip.set.infinity();
                isminsettoinfinity = true;
                ismaxsettoinfinity = true;
            } else {
                // Calculate residual activity bounds if variable would be fixed to
                // zero.
                let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_glb_activity_residuals(scip, c, var, val, false, true, true));
                minresactivity = min.act;
                maxresactivity = max.act;
                isminsettoinfinity = min.issettoinfinity;
                ismaxsettoinfinity = max.issettoinfinity;

                // We called consdataGetGlbActivityResiduals() saying that we do not
                // need a good relaxation, so whenever we have a relaxed activity, it
                // should be relaxed to +/- infinity. This is needed, because we do
                // not want to rely on relaxed finite resactivities.

                // Check minresactivity for reliability.
                let c = cd(scip, cons);
                if !isminsettoinfinity && scip.set.is_update_unreliable(minresactivity, c.lastglbminactivity) {
                    minresactivity = consdata_get_reliable_residual_activity(scip, c, var, true, true);
                }

                // Check maxresactivity for reliability.
                if !ismaxsettoinfinity && scip.set.is_update_unreliable(maxresactivity, c.lastglbmaxactivity) {
                    maxresactivity = consdata_get_reliable_residual_activity(scip, c, var, false, true);
                }
            }

            let arrayindex = (scip.vars[var].probindex - nbinvars) as usize;

            let mut newredlb = redlb[arrayindex];
            let mut newredub = redub[arrayindex];
            let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
            if val > 0.0 {
                if lhsexists {
                    // lhs <= d*x + a*y, d > 0  ->  redundant in y if  x >= (lhs -
                    // min{a*y})/d.
                    nlocksdown[arrayindex] += nlockspos;
                    newredlb = if isminsettoinfinity { scip.set.infinity() } else { (lhs - minresactivity) / val };
                }
                if rhsexists {
                    // d*x + a*y <= rhs, d > 0  ->  redundant in y if  x <= (rhs -
                    // max{a*y})/d.
                    nlocksup[arrayindex] += nlockspos;
                    newredub = if ismaxsettoinfinity { -scip.set.infinity() } else { (rhs - maxresactivity) / val };
                }
            } else {
                if lhsexists {
                    // lhs <= d*x + a*y, d < 0  ->  redundant in y if  x <= (lhs -
                    // min{a*y})/d.
                    nlocksup[arrayindex] += nlockspos;
                    newredub = if isminsettoinfinity { -scip.set.infinity() } else { (lhs - minresactivity) / val };
                }
                if rhsexists {
                    // d*x + a*y <= rhs, d < 0  ->  redundant in y if  x >= (rhs -
                    // max{a*y})/d.
                    nlocksdown[arrayindex] += nlockspos;
                    newredlb = if ismaxsettoinfinity { scip.set.infinity() } else { (rhs - maxresactivity) / val };
                }
            }

            // If the variable is integer, we have to round the value to the next
            // integral value.
            if var_is_integral(scip, var) {
                if !scip.set.is_infinity(newredlb) {
                    newredlb = scip.set.ceil(newredlb);
                }
                if !scip.set.is_infinity(-newredub) {
                    newredub = scip.set.floor(newredub);
                }
            }

            // Update redundancy bounds.
            redlb[arrayindex] = crate::def::max(redlb[arrayindex], newredlb);
            redub[arrayindex] = crate::def::min(redub[arrayindex], newredub);

            // Collect the continuous variables of the constraint.
            if !var_is_integral(scip, var) {
                contvarpos = i;
                conscontvars.push(var);
                let contv = scip.vars[var].probindex - nintvars;
                hasimpliedpotential = hasimpliedpotential || isimplint[contv as usize];
            }
        }

        // Update implied integrality status of continuous variables.
        if hasimpliedpotential {
            if conscontvars.len() > 1 || !integralcoefs {
                // There is more than one continuous variable or the integer
                // variables have fractional coefficients: none of the continuous
                // variables is implied integral.
                for &var in &conscontvars {
                    let contv = scip.vars[var].probindex - nintvars;
                    isimplint[contv as usize] = false;
                }
            } else {
                // There is exactly one continuous variable and the integer
                // variables have integral coefficients: this is the interesting
                // case, and we have to check whether the coefficient is +/-1 and
                // the corresponding side(s) of the constraint is integral.
                let var = cd(scip, cons).vars[contvarpos];
                let val = cd(scip, cons).vals[contvarpos];
                let contv = (scip.vars[var].probindex - nintvars) as usize;
                let absval = val.abs();
                if !scip.set.is_eq(absval, 1.0) {
                    isimplint[contv] = false;
                } else {
                    let obj = scip.vars[var].obj;
                    if obj * val >= 0.0 && lhsexists {
                        // The variable may be blocked by the constraint's left hand
                        // side.
                        isimplint[contv] = isimplint[contv] && scip.set.is_integral(cd(scip, cons).lhs);
                    }
                    if obj * val <= 0.0 && rhsexists {
                        // The variable may be blocked by the constraint's left hand
                        // side.
                        isimplint[contv] = isimplint[contv] && scip.set.is_integral(cd(scip, cons).rhs);
                    }
                }
            }
        }
    }

    // Check if any bounds can be tightened due to optimality.
    for v in 0..n {
        let var = vars[v];
        let obj = scip.vars[var].obj;
        if !scip.set.is_positive(-obj) {
            // Making the variable as small as possible does not increase the
            // objective: check if all down locks of the variables are due to
            // linear constraints; if variable is cost neutral and only upper
            // bounded non-positively or negative largest bound to make
            // constraints redundant is huge, we better do nothing for numerical
            // reasons.
            let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
            if (scip.set.is_positive(obj) || scip.set.is_positive(gub) || !scip.set.is_infinity(-glb))
                && locksdown(scip, var, LockType::Model) == nlocksdown[v]
                && !scip.set.is_huge_value(-redlb[v])
                && redlb[v] < gub
            {
                // If x_v >= redlb[v], we can always round x_v down to x_v ==
                // redlb[v] without violating any constraint -> tighten upper bound
                // to x_v <= redlb[v].
                let (_infeasible, tightened) = scip_var::tighten_var_ub(scip, var, redlb[v], false)?;
                let ub = scip.vars[var].glbdom.ub;
                redub[v] = crate::def::min(redub[v], ub);
                if tightened {
                    *nchgbds += 1;
                }
            }
        }

        if !scip.set.is_positive(obj) {
            // Making the variable as large as possible does not increase the
            // objective: check if all up locks of the variables are due to linear
            // constraints; if variable is cost neutral and only lower bounded
            // non-negatively or positive smallest bound to make constraints
            // redundant is huge, we better do nothing for numerical reasons.
            let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
            if (scip.set.is_positive(-obj) || scip.set.is_positive(-glb) || !scip.set.is_infinity(gub))
                && locksup(scip, var, LockType::Model) == nlocksup[v]
                && !scip.set.is_huge_value(redub[v])
                && redub[v] > glb
            {
                // If x_v <= redub[v], we can always round x_v up to x_v == redub[v]
                // without violating any constraint -> tighten lower bound to x_v >=
                // redub[v].
                let (_infeasible, tightened) = scip_var::tighten_var_lb(scip, var, redub[v], false)?;
                let lb = scip.vars[var].glbdom.lb;
                redlb[v] = crate::def::max(redlb[v], lb);
                if tightened {
                    *nchgbds += 1;
                }
            }
        }
    }

    // Declare continuous variables implied integral.
    for v in (nintvars - nbinvars) as usize..n {
        let var = vars[v];

        // We can only conclude implied integrality if the variable appears in no
        // other constraint.
        if isimplint[v - (nintvars - nbinvars) as usize] && locksdown(scip, var, LockType::Model) == nlocksdown[v] && locksup(scip, var, LockType::Model) == nlocksup[v] {
            // Since we locally copied the variable array we can change the
            // variable type immediately.
            let infeasible = scip_var::chg_var_impl_type(scip, var, ImplintType::Weak)?;
            *nchgvartypes += 1;
            if infeasible {
                *cutoff = true;
                break;
            }
        }
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce constraints.
fn enforce_constraint(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let checkrelmaxabs = hd(scip, conshdlr).checkrelmaxabs;

    // Check for violated constraints: LP is processed at current node -> we
    // can add violated linear constraints to the SCIP_LP.
    *result = ScipResultCode::Feasible;

    // Check all useful linear constraints for feasibility.
    for &cons in &conss[..nusefulconss as usize] {
        let violated = check_cons(scip, cons, sol, false, checkrelmaxabs)?;

        if violated {
            // Insert LP row as cut.
            let cutoff = add_relaxation(scip, cons)?;
            *result = if cutoff { ScipResultCode::Cutoff } else { ScipResultCode::Separated };
        }
    }

    // Check all obsolete linear constraints for feasibility.
    let mut c = nusefulconss as usize;
    while c < conss.len() && *result == ScipResultCode::Feasible {
        let violated = check_cons(scip, conss[c], sol, false, checkrelmaxabs)?;

        if violated {
            // Insert LP row as cut.
            let cutoff = add_relaxation(scip, conss[c])?;
            *result = if cutoff { ScipResultCode::Cutoff } else { ScipResultCode::Separated };
        }
        c += 1;
    }

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `consFreeLinear`: destructor of constraint handler to free constraint
/// handler data (called when SCIP is exiting).
fn cons_free_linear(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    // Free constraint handler data.
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consInitLinear`: initialization method of constraint handler (called
/// after problem was transformed).
fn cons_init_linear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    // Check for event handler.
    hdm(scip, conshdlr).naddconss = 0;
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Catch events for the constraints.
    for &cons in conss {
        // Catch all events.
        cons_catch_all_events(scip, cons, eventhdlr)?;
    }

    Ok(())
}

/// `consExitLinear`: deinitialization method of constraint handler (called
/// before transformed problem is freed).
fn cons_exit_linear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    // Check for event handler.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Drop events for the constraints.
    for &cons in conss.iter().rev() {
        if cd(scip, cons).eventdata.is_some() {
            // Drop all events.
            cons_drop_all_events(scip, cons, eventhdlr)?;
        }
    }

    Ok(())
}

/// `consExitpreLinear`: presolving deinitialization method of constraint
/// handler (called after presolving has been finished).
fn cons_exitpre_linear(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    // Delete all linear constraints that were upgraded to a more specific
    // constraint type; make sure, only active variables remain in the
    // remaining constraints.
    for &cons in conss {
        if scip.conss[cons].deleted {
            continue;
        }

        if cd(scip, cons).upgraded {
            // This is no problem reduction, because the upgraded constraint was
            // added to the problem before, and the (redundant) linear constraint
            // was only kept in order to support presolving the the linear
            // constraint handler.
            scip_cons::del_cons(scip, cons)?;
        } else {
            // Since we are not allowed to detect infeasibility in the exitpre
            // stage, we dont give an infeasible pointer.
            apply_fixings(scip, cons, false)?;
        }
    }

    Ok(())
}

/// `consInitsolLinear`: solving process initialization method of
/// constraint handler; adds the nlrows to the NLP if it had been
/// constructed, which the port never does (the NLP is not ported).
fn cons_initsol_linear(_scip: &mut Scip, _conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    Ok(())
}

/// `consExitsolLinear`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_linear(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], restart: bool) -> ScipResult {
    // Release the rows and nlrows of all constraints.
    for &cons in conss {
        let row = cd(scip, cons).row;
        if row != NONE {
            crate::lp::row_release(scip, row)?;
            cdm(scip, cons).row = NONE;
        }
    }

    // If this is a restart, convert cutpool rows into linear constraints.
    if restart {
        // Create out of all active cuts in cutpool linear constraints; C
        // reports the number converted.
        scip_cons::convert_cuts_to_conss(scip)?;
    }

    Ok(())
}

/// `consDeactiveLinear`: constraint deactivation notification method of
/// constraint handler.
fn cons_deactive_linear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    if scip.conss[cons].deleted {
        // Check for event handler.
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        // Free event data.
        if cd(scip, cons).eventdata.is_some() {
            // Drop bound change events of variables.
            cons_drop_all_events(scip, cons, eventhdlr)?;
        }
    }

    // Remove row from NLP, if still in solving (the NLP is not ported).
    Ok(())
}

/// `consDeleteLinear`: frees specific constraint data.
fn cons_delete_linear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    let hasevents = consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap().eventdata.is_some();
    if hasevents {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;

        // Drop all events.
        scip.conss[cons].consdata = consdata.take();
        cons_drop_all_events(scip, cons, eventhdlr)?;
        *consdata = scip.conss[cons].consdata.take();
    }

    // Free linear constraint.
    let mut data = consdata.take().unwrap();
    consdata_free(scip, data.downcast_mut::<ConsData>().unwrap())?;

    Ok(())
}

/// `consTransLinear`: transforms constraint data into data belonging to the
/// transformed problem.
fn cons_trans_linear(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let s = cd(scip, sourcecons);
    let n = s.nvars as usize;
    let vars = s.vars.0[..n].to_vec();
    let vals = s.vals.0[..n].to_vec();
    let (lhs, rhs) = (s.lhs, s.rhs);

    // Create linear constraint data for target constraint.
    let targetdata = consdata_create(scip, &vars, &vals, lhs, rhs)?;

    // Create target constraint.
    let k = &scip.conss[sourcecons];
    let name = k.name.clone();
    let flags = crate::cons::ConsFlags {
        initial: k.is_initial(),
        separate: k.is_separated(),
        enforce: k.is_enforced(),
        check: k.is_checked(),
        propagate: k.is_propagated(),
        local: k.is_local(),
        modifiable: k.modifiable,
        dynamic: k.is_dynamic(),
        removable: k.is_removable(),
        stickingatnode: k.is_sticking_at_node(),
    };
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    Ok(())
}

/// `consInitlpLinear`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
fn cons_initlp_linear(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let mut c = 0;
    while c < conss.len() && !*infeasible {
        *infeasible = add_relaxation(scip, conss[c])?;
        c += 1;
    }

    Ok(())
}

/// `consSepalpLinear`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_linear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    let depth = api::get_depth(scip);
    let nrounds = api::get_n_sepa_rounds(scip);

    *result = ScipResultCode::DidNotRun;

    // Only call the separator a given number of times at each node.
    let h = hd(scip, conshdlr);
    if (depth == 0 && h.maxroundsroot >= 0 && nrounds >= h.maxroundsroot) || (depth > 0 && h.maxrounds >= 0 && nrounds >= h.maxrounds) {
        return Ok(());
    }

    // Get the maximal number of cuts allowed in a separation round.
    let maxsepacuts = if depth == 0 { h.maxsepacutsroot } else { h.maxsepacuts };
    let (maxcardbounddist, separateall, checkrelmaxabs) = (h.maxcardbounddist, h.separateall, h.checkrelmaxabs);

    // Check if we want to produce knapsack cardinality cuts at this node.
    let loclowerbound = api::get_local_lowerbound(scip);
    let glblowerbound = api::get_lowerbound(scip);
    let cutoffbound = api::get_cutoffbound(scip);
    let maxbound = glblowerbound + maxcardbounddist * (cutoffbound - glblowerbound);
    let mut separatecards = scip.set.is_le(loclowerbound, maxbound);
    separatecards = separatecards && api::get_n_lp_branch_cands(scip) > 0;

    *result = ScipResultCode::DidNotFind;
    let mut ncuts = 0;
    let mut cutoff = false;

    // Check all useful linear constraints for feasibility.
    let mut c = 0;
    while c < nusefulconss as usize && ncuts < maxsepacuts && !cutoff {
        separate_cons(scip, conss[c], checkrelmaxabs, NONE, separatecards, separateall, &mut ncuts, &mut cutoff)?;
        c += 1;
    }

    // Adjust return value.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/// `consSepasolLinear`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_linear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let depth = api::get_depth(scip);
    let nrounds = api::get_n_sepa_rounds(scip);

    *result = ScipResultCode::DidNotRun;

    // Only call the separator a given number of times at each node.
    let h = hd(scip, conshdlr);
    if (depth == 0 && h.maxroundsroot >= 0 && nrounds >= h.maxroundsroot) || (depth > 0 && h.maxrounds >= 0 && nrounds >= h.maxrounds) {
        return Ok(());
    }

    // Get the maximal number of cuts allowed in a separation round.
    let maxsepacuts = if depth == 0 { h.maxsepacutsroot } else { h.maxsepacuts };
    let (separateall, checkrelmaxabs) = (h.separateall, h.checkrelmaxabs);

    *result = ScipResultCode::DidNotFind;
    let mut ncuts = 0;
    let mut cutoff = false;

    // Check all useful linear constraints for feasibility.
    let mut c = 0;
    while c < nusefulconss as usize && ncuts < maxsepacuts && !cutoff {
        separate_cons(scip, conss[c], checkrelmaxabs, sol, true, separateall, &mut ncuts, &mut cutoff)?;
        c += 1;
    }

    // Adjust return value.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/// `consEnfolpLinear`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_linear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, nusefulconss, NONE, result)
}

/// `consEnforelaxLinear`: constraint enforcing method of constraint handler
/// for relaxation solutions.
fn cons_enforelax_linear(scip: &mut Scip, sol: SolId, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, nusefulconss, sol, result)
}

/// `consEnfopsLinear`: constraint enforcing method of constraint handler
/// for pseudo solutions.
#[allow(clippy::too_many_arguments)]
fn cons_enfops_linear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    let checkrelmaxabs = hd(scip, conshdlr).checkrelmaxabs;

    // If the solution is infeasible anyway due to objective value, skip the
    // enforcement.
    if objinfeasible {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    // Check all linear constraints for feasibility.
    let mut violated = false;
    let mut c = 0;
    while c < conss.len() && !violated {
        violated = check_cons(scip, conss[c], NONE, true, checkrelmaxabs)?;
        c += 1;
    }

    *result = if violated { ScipResultCode::Infeasible } else { ScipResultCode::Feasible };

    Ok(())
}

/// `consCheckLinear`: feasibility check method of constraint handler for
/// integral solutions; printing the reason is not ported.
#[allow(clippy::too_many_arguments)]
fn cons_check_linear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    checklprows: bool,
    _printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let checkrelmaxabs = hd(scip, conshdlr).checkrelmaxabs;

    // Check all linear constraints for feasibility.
    let mut c = 0;
    while c < conss.len() && (*result == ScipResultCode::Feasible || completely) {
        let violated = check_cons(scip, conss[c], sol, checklprows, checkrelmaxabs)?;

        if violated {
            *result = ScipResultCode::Infeasible;
        }
        c += 1;
    }

    Ok(())
}

/// `consPropLinear`: domain propagation method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_prop_linear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut rangedrowpropagation = false;
    let tightenbounds;

    // Check, if we want to tighten variable's bounds (in probing, we always want
    // to tighten the bounds).
    if api::in_probing(scip) {
        tightenbounds = true;
    } else {
        let depth = api::get_depth(scip);
        let propfreq = scip.conshdlrs[conshdlr].propfreq;
        let h = hd(scip, conshdlr);
        let tightenboundsfreq = propfreq * h.tightenboundsfreq;
        tightenbounds = h.tightenboundsfreq >= 0 && ((tightenboundsfreq == 0 && depth == 0) || (tightenboundsfreq >= 1 && depth % tightenboundsfreq == 0));

        // Check if we want to do ranged row propagation.
        rangedrowpropagation = h.rangedrowpropagation;
        rangedrowpropagation = rangedrowpropagation && !crate::tree::tree_in_repropagation(scip);
        rangedrowpropagation = rangedrowpropagation && depth <= h.rangedrowmaxdepth;
        let rangedrowfreq = propfreq * h.rangedrowfreq;
        rangedrowpropagation = rangedrowpropagation && h.rangedrowfreq >= 0 && ((rangedrowfreq == 0 && depth == 0) || (rangedrowfreq >= 1 && depth % rangedrowfreq == 0));
    }

    let (maxeasyactivitydelta, sortvars) = (hd(scip, conshdlr).maxeasyactivitydelta, hd(scip, conshdlr).sortvars);
    let mut cutoff = false;
    let mut nchgbds = 0;

    // Process constraints marked for propagation.
    let mut i = 0;
    while i < nmarkedconss as usize && !cutoff {
        cons_unmark_propagate(scip, conss[i]);
        propagate_cons(
            scip,
            conss[i],
            tightenbounds,
            rangedrowpropagation,
            maxeasyactivitydelta,
            sortvars,
            &mut cutoff,
            &mut nchgbds,
        )?;
        i += 1;
    }

    // Adjust result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if nchgbds > 0 {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// Handles a constraint that became empty in [`cons_presol_linear`]: it is
/// infeasible if its sides cross, otherwise redundant and deleted.
fn presol_empty_cons(scip: &mut Scip, cons: ConsId, cutoff: &mut bool, ndelconss: &mut i32) -> ScipResult {
    if scip.set.is_feas_gt(cd(scip, cons).lhs, cd(scip, cons).rhs) {
        *cutoff = true;
    } else {
        scip_cons::del_cons(scip, cons)?;
        if !cd(scip, cons).upgraded {
            *ndelconss += 1;
        }
    }
    Ok(())
}

const MAXCONSPRESOLROUNDS: i32 = 10;

/// `consPresolLinear`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_linear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nrounds: i32,
    presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    let nconss = conss.len() as i32;

    // Remember old preprocessing counters.
    let mut cutoff = false;
    let oldnfixedvars = counts.nfixedvars;
    let oldnaggrvars = counts.naggrvars;
    let oldnchgbds = counts.nchgbds;
    let mut oldndelconss = counts.ndelconss;
    let oldnupgdconss = counts.nupgdconss;
    let mut oldnchgcoefs = counts.nchgcoefs;
    let mut oldnchgsides = counts.nchgsides;

    // Get constraint handler data.
    let h = hd(scip, conshdlr);
    let eventhdlr = h.eventhdlr;
    let maxeasyactivitydelta = h.maxeasyactivitydelta;
    let sortvars = h.sortvars;
    let simplifyinequalities = h.simplifyinequalities;
    let aggregatevariables = h.aggregatevariables;
    let rangedrowpropagation = h.rangedrowpropagation;
    let extractcliques = h.extractcliques;
    let dualpresolving = h.dualpresolving;
    let singletonstuffing = h.singletonstuffing;
    let singlevarstuffing = h.singlevarstuffing;
    let presolusehashing = h.presolusehashing;
    let presolpairwise = h.presolpairwise;
    let maxaggrnormscale = h.maxaggrnormscale;
    let nmincomparisons = h.nmincomparisons;
    let mingainpernmincomp = h.mingainpernmincomp;

    // Process single constraints.
    let mut firstchange = i32::MAX;
    let mut firstupgradetry = i32::MAX;
    let mut c = 0;
    while c < nconss && !cutoff && !crate::solve::solve_is_stopped(scip, false) {
        let cons = conss[c as usize];

        // Ensure that rhs >= lhs is satisfied without numerical tolerance.
        if scip.set.is_eq(cd(scip, cons).rhs, cd(scip, cons).lhs) {
            let c = cdm(scip, cons);
            c.lhs = c.rhs;
        }

        if cd(scip, cons).eventdata.is_none() {
            // Catch bound change events of variables.
            cons_catch_all_events(scip, cons, eventhdlr)?;
        }

        // Incorporate fixings and aggregations in constraint.
        if apply_fixings(scip, cons, true)? {
            cutoff = true;
            break;
        }

        // We can only presolve linear constraints, that are not modifiable.
        if scip.conss[cons].modifiable {
            c += 1;
            continue;
        }

        // Remember the first changed constraint to begin the next aggregation
        // round with.
        if firstchange == i32::MAX && cd(scip, cons).changed {
            firstchange = c;
        }

        // Remember the first constraint that was not yet tried to be upgraded,
        // to begin the next upgrading round with.
        if firstupgradetry == i32::MAX && !cd(scip, cons).upgradetried {
            firstupgradetry = c;
        }

        // Check, if constraint is already preprocessed.
        if cd(scip, cons).presolved {
            c += 1;
            continue;
        }

        // Apply presolving as long as possible on the single constraint
        // (however, abort after a certain number of rounds to avoid nearly
        // infinite cycling due to very small bound changes).
        let mut npresolrounds = 0;
        while !cd(scip, cons).presolved && npresolrounds < MAXCONSPRESOLROUNDS && !crate::solve::solve_is_stopped(scip, false) {
            npresolrounds += 1;

            // Mark constraint being presolved and propagated.
            cdm(scip, cons).presolved = true;
            cons_unmark_propagate(scip, cons);

            if normalize_cons(scip, cons)? {
                cutoff = true;
                break;
            }

            // Tighten left and right hand side due to integrality.
            if tighten_sides(scip, cons, &mut counts.nchgsides)? {
                cutoff = true;
                break;
            }

            // Check bounds.
            if scip.set.is_feas_gt(cd(scip, cons).lhs, cd(scip, cons).rhs) {
                cutoff = true;
                break;
            }

            // Tighten variable's bounds.
            tighten_bounds(scip, cons, maxeasyactivitydelta, sortvars, &mut cutoff, &mut counts.nchgbds)?;
            if cutoff {
                break;
            }

            // Check for fixed variables.
            fix_variables(scip, cons, &mut cutoff, &mut counts.nfixedvars)?;
            if cutoff {
                break;
            }

            // If the maximal coefficient is large, recompute the activities before
            // infeasibility and redundancy checks.
            {
                let c = cd(scip, cons);
                if (c.validmaxabsval && c.maxabsval > MAXVALRECOMP) || (c.validminabsval && c.minabsval < MINVALRECOMP) {
                    with_cd(scip, cons, |scip, c| {
                        consdata_recompute_minactivity(scip, c);
                        consdata_recompute_maxactivity(scip, c);
                    });
                }
            }

            // Get activity bounds.
            let (min, max) = with_cd(scip, cons, |scip, c| consdata_get_activity_bounds(scip, c, true));
            let (minactivity, maxactivity) = (min.act, max.act);
            let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

            // Check constraint for infeasibility and redundancy.
            if scip.set.is_feas_gt(minactivity, rhs) || scip.set.is_feas_lt(maxactivity, lhs) {
                cutoff = true;
                break;
            } else if scip.set.is_ge(minactivity, lhs) && scip.set.is_le(maxactivity, rhs) {
                scip_cons::del_cons(scip, cons)?;
                if !cd(scip, cons).upgraded {
                    counts.ndelconss += 1;
                }
                break;
            } else if !scip.set.is_infinity(-lhs) && scip.set.is_ge(minactivity, lhs) {
                chg_lhs(scip, cons, -scip.set.infinity())?;
                if !cd(scip, cons).upgraded {
                    counts.nchgsides += 1;
                }
            } else if !scip.set.is_infinity(rhs) && scip.set.is_le(maxactivity, rhs) {
                chg_rhs(scip, cons, scip.set.infinity())?;
                if !cd(scip, cons).upgraded {
                    counts.nchgsides += 1;
                }
            }

            // Handle empty constraint.
            if cd(scip, cons).nvars == 0 {
                presol_empty_cons(scip, cons, &mut cutoff, &mut counts.ndelconss)?;
                break;
            }

            // Reduce big-M coefficients, that make the constraint redundant if the
            // variable is on a bound.
            consdata_tighten_coefs(scip, cons, &mut counts.nchgcoefs, &mut counts.nchgsides)?;

            // Try to simplify inequalities.
            if simplifyinequalities {
                cutoff = simplify_inequalities(scip, cons, &mut counts.nchgcoefs, &mut counts.nchgsides)?;
                if cutoff {
                    break;
                }
            }

            // Aggregation variable in equations.
            if aggregatevariables {
                aggregate_variables(scip, cons, &mut cutoff, &mut counts.nfixedvars, &mut counts.naggrvars)?;
                if cutoff {
                    break;
                }
            }
        }

        if !cutoff && !crate::solve::solve_is_stopped(scip, false) {
            // Perform ranged row propagation.
            if rangedrowpropagation {
                let lastnfixedvars = counts.nfixedvars;

                ranged_row_propagation(scip, cons, &mut cutoff, &mut counts.nfixedvars, &mut counts.nchgbds, &mut counts.naddconss)?;
                if !cutoff && lastnfixedvars < counts.nfixedvars {
                    cutoff = apply_fixings(scip, cons, true)?;
                }
            }

            // Extract cliques from constraint.
            if extractcliques && !cutoff && scip.conss[cons].is_active() {
                extract_cliques(scip, cons, maxeasyactivitydelta, sortvars, &mut counts.nfixedvars, &mut counts.nchgbds, &mut cutoff)?;

                // Check if the constraint got redundant or infeasible.
                if !cutoff && scip.conss[cons].is_active() && cd(scip, cons).nvars == 0 {
                    presol_empty_cons(scip, cons, &mut cutoff, &mut counts.ndelconss)?;
                }
            }

            // Convert special equalities.
            if !cutoff && scip.conss[cons].is_active() {
                convert_equality(
                    scip,
                    cons,
                    &mut cutoff,
                    &mut counts.nfixedvars,
                    &mut counts.naggrvars,
                    &mut counts.ndelconss,
                    &mut counts.nchgvartypes,
                )?;
            }

            // Apply dual presolving for variables that appear in only one
            // constraint.
            if !cutoff && scip.conss[cons].is_active() && dualpresolving && scip_var::allow_strong_dual_reds(scip) {
                dual_presolve(
                    scip,
                    cons,
                    &mut cutoff,
                    &mut counts.nfixedvars,
                    &mut counts.naggrvars,
                    &mut counts.ndelconss,
                    &mut counts.nchgvartypes,
                )?;
            }

            // Check if an inequality is parallel to the objective function.
            if !cutoff && scip.conss[cons].is_active() {
                check_parallel_objective(scip, cons)?;
            }

            // Remember the first changed constraint to begin the next aggregation
            // round with.
            if firstchange == i32::MAX && cd(scip, cons).changed {
                firstchange = c;
            }

            // Remember the first constraint that was not yet tried to be upgraded,
            // to begin the next upgrading round with.
            if firstupgradetry == i32::MAX && !cd(scip, cons).upgradetried {
                firstupgradetry = c;
            }
        }

        // Singleton column stuffing.
        if !cutoff && scip.conss[cons].is_active() && scip.conss[cons].is_checked() && (singletonstuffing || singlevarstuffing) && scip_var::allow_strong_dual_reds(scip) {
            presol_stuffing(scip, cons, singletonstuffing, singlevarstuffing, &mut cutoff, &mut counts.nfixedvars, &mut counts.nchgbds)?;

            // Handle empty constraint (C leaves the constraint loop here).
            if cd(scip, cons).nvars == 0 {
                presol_empty_cons(scip, cons, &mut cutoff, &mut counts.ndelconss)?;
                break;
            }
        }
        c += 1;
    }

    // Process pairs of constraints: check them for redundancy and try to
    // aggregate them; only apply this expensive procedure in exhaustive
    // presolving timing.
    if !cutoff && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 && (presolusehashing || presolpairwise) && !crate::solve::solve_is_stopped(scip, false) {
        if firstchange < nconss && presolusehashing {
            // Detect redundant constraints; fast version with hash table instead
            // of pairwise comparison.
            detect_redundant_constraints(scip, conss, &mut firstchange, &mut cutoff, &mut counts.ndelconss, &mut counts.nchgsides)?;
        }
        if firstchange < nconss && presolpairwise {
            let mut npaircomparisons: i64 = 0;
            oldndelconss = counts.ndelconss;
            oldnchgsides = counts.nchgsides;
            oldnchgcoefs = counts.nchgcoefs;

            // Allocate temporary memory.
            let mut usefulconss: Vec<ConsId> = Vec::with_capacity(conss.len());
            let mut firstchangenew = -1;
            for (c, &cons) in conss.iter().enumerate() {
                // Update firstchange.
                if c as i32 == firstchange {
                    firstchangenew = usefulconss.len() as i32;
                }

                // Ignore inactive and modifiable constraints.
                if !scip.conss[cons].is_active() || scip.conss[cons].modifiable {
                    continue;
                }

                usefulconss.push(cons);
            }
            firstchange = firstchangenew;
            let nusefulconss = usefulconss.len() as i32;
            let mut c = firstchange;
            while c < nusefulconss && !cutoff && !crate::solve::solve_is_stopped(scip, false) {
                // Constraint has become inactive or modifiable during pairwise
                // presolving.
                if usefulconss[c as usize] == NONE {
                    c += 1;
                    continue;
                }

                // C reads conss[c] here, not usefulconss[c].
                npaircomparisons += if cd(scip, conss[c as usize]).changed { c as i64 } else { (c - firstchange) as i64 };

                preprocess_constraint_pairs(
                    scip,
                    &mut usefulconss,
                    firstchange,
                    c,
                    maxaggrnormscale,
                    &mut cutoff,
                    &mut counts.ndelconss,
                    &mut counts.nchgsides,
                    &mut counts.nchgcoefs,
                )?;

                if npaircomparisons > nmincomparisons as i64 {
                    if ((counts.ndelconss - oldndelconss) as f64 + (counts.nchgsides - oldnchgsides) as f64 / 2.0 + (counts.nchgcoefs - oldnchgcoefs) as f64 / 10.0)
                        / (npaircomparisons as f64)
                        < mingainpernmincomp
                    {
                        break;
                    }
                    oldndelconss = counts.ndelconss;
                    oldnchgsides = counts.nchgsides;
                    oldnchgcoefs = counts.nchgcoefs;
                    npaircomparisons = 0;
                }
                c += 1;
            }
        }
    }

    // Before upgrading, check whether we can apply some additional dual
    // presolving, because a variable only appears in linear constraints and
    // we therefore have full information about it.
    if !cutoff
        && firstupgradetry < nconss
        && counts.nfixedvars == oldnfixedvars
        && counts.naggrvars == oldnaggrvars
        && counts.nchgbds == oldnchgbds
        && counts.ndelconss == oldndelconss
        && counts.nupgdconss == oldnupgdconss
        && counts.nchgcoefs == oldnchgcoefs
        && counts.nchgsides == oldnchgsides
        && dualpresolving
        && scip_var::allow_strong_dual_reds(scip)
        && !crate::solve::solve_is_stopped(scip, false)
    {
        full_dual_presolve(scip, conss, &mut cutoff, &mut counts.nchgbds, &mut counts.nchgvartypes)?;
    }

    // Try to upgrade constraints into a more specific constraint type; only
    // upgrade constraints, if no reductions were found in this round
    // (otherwise, the linear constraint handler may find additional reductions
    // before giving control away to other (less intelligent?) constraint
    // handlers).
    if !cutoff && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 && api::is_presolve_finished_with(scip, counts) {
        let mut c = firstupgradetry;
        while c < nconss && !crate::solve::solve_is_stopped(scip, false) {
            let cons = conss[c as usize];
            c += 1;

            // Don't upgrade modifiable constraints.
            if scip.conss[cons].modifiable {
                continue;
            }

            // Only upgrade completely presolved constraints, that changed since
            // the last upgrading call.
            if cd(scip, cons).upgradetried {
                continue;
            }
            if !cd(scip, cons).presolved {
                continue;
            }

            cdm(scip, cons).upgradetried = true;
            if scip.conss[cons].is_active() {
                let mut upgdcons = upgrade_cons_linear(scip, cons)?;
                if upgdcons != NONE {
                    // Add the upgraded constraint to the problem.
                    scip_cons::add_cons_upgrade(scip, cons, &mut upgdcons)?;
                    counts.nupgdconss += 1;

                    // Mark the linear constraint being upgraded and to be removed
                    // after presolving; don't delete it directly, because it may help
                    // to preprocess other linear constraints.
                    cdm(scip, cons).upgraded = true;

                    // Delete upgraded inequalities immediately; delete upgraded
                    // equalities, if we don't need it anymore for aggregation and
                    // redundancy checking.
                    if scip.set.is_lt(cd(scip, cons).lhs, cd(scip, cons).rhs) || !presolpairwise || maxaggrnormscale == 0.0 {
                        scip_cons::del_cons(scip, cons)?;
                    }
                }
            }
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
        || counts.nchgcoefs > oldnchgcoefs
        || counts.nchgsides > oldnchgsides
    {
        ScipResultCode::Success
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consRespropLinear`: propagation conflict resolving method of
/// constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_linear(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    infervar: VarId,
    inferinfo: i32,
    boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = resolve_propagation(scip, cons, infervar, inferinfo, boundtype, bdchgidx.as_ref())?;
    Ok(())
}

/// `consLockLinear`: variable rounding lock method of constraint handler.
fn cons_lock_linear(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let haslhs = !scip.set.is_infinity(-cd(scip, cons).lhs);
    let hasrhs = !scip.set.is_infinity(cd(scip, cons).rhs);

    // Update rounding locks of every single variable.
    for i in 0..cd(scip, cons).nvars {
        let (var, val) = (cd(scip, cons).vars[i], cd(scip, cons).vals[i]);
        if scip.set.is_positive(val) {
            if haslhs {
                api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
            }
            if hasrhs {
                api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
            }
        } else {
            if haslhs {
                api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
            }
            if hasrhs {
                api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
            }
        }
    }

    Ok(())
}

/// `consDelvarsLinear`: variable deletion method of constraint handler.
fn cons_delvars_linear(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if !conss.is_empty() {
        perform_var_deletions(scip, conss)?;
    }
    Ok(())
}

/// `consGetVarsLinear`: constraint method of constraint handler which
/// returns the variables (the vector always fits here).
fn cons_get_vars_linear(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.extend_from_slice(&c.vars.0[..c.nvars as usize]);
    *success = true;
    Ok(())
}

/// `consGetNVarsLinear`: constraint method of constraint handler which
/// returns the number of variables.
fn cons_get_n_vars_linear(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = cd(scip, cons).nvars;
    *success = true;
    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecLinear`: execution method of event handler.
fn event_exec_linear(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let edata = ed(scip, eventhdlr, eventdata);
    let cons = edata.cons;

    // We can skip events droped for deleted constraints.
    if scip.conss[cons].deleted {
        return Ok(());
    }

    let eventtype = event.eventtype;
    let var = event.data.var;

    if (eventtype & EVENTTYPE_BOUNDCHANGED) != EVENTTYPE_DISABLED {
        let varpos = edata.varpos;

        let oldbound = event.data.oldbound;
        let newbound = event.data.newbound;
        let val = cd(scip, cons).vals[varpos];

        // We only need to update the activities if the constraint is active,
        // otherwise we mark them to be invalid.
        if scip.conss[cons].is_active() {
            // Update the activity values.
            with_cd(scip, cons, |scip, c| {
                if (eventtype & EVENTTYPE_LBCHANGED) != EVENTTYPE_DISABLED {
                    consdata_update_activities_lb(scip, c, var, oldbound, newbound, val, true);
                } else {
                    consdata_update_activities_ub(scip, c, var, oldbound, newbound, val, true);
                }
            });
        } else {
            consdata_invalidate_activities(cdm(scip, cons));
        }

        let c = cdm(scip, cons);
        c.presolved = false;
        c.rangedrowpropagated = 0;

        // Bound change can turn the constraint infeasible or redundant only if
        // it was a tightening.
        if (eventtype & EVENTTYPE_BOUNDTIGHTENED) != EVENTTYPE_DISABLED {
            cons_mark_propagate(scip, cons);

            let infinity = scip.set.infinity();
            let c = cdm(scip, cons);

            // Reset maximal activity delta, so that it will be recalculated on the
            // next real propagation.
            if c.maxactdeltavar == var {
                c.maxactdelta = INVALID;
                c.maxactdeltavar = NONE;
            }

            // Check whether bound tightening might now be successful.
            if c.boundstightened > 0 {
                let rhsfinite = !(c.rhs >= infinity);
                let lhsfinite = !(-c.lhs >= infinity);
                match eventtype {
                    EVENTTYPE_LBTIGHTENED => {
                        if if val > 0.0 { rhsfinite } else { lhsfinite } {
                            c.boundstightened = 0;
                        }
                    }
                    EVENTTYPE_UBTIGHTENED => {
                        if if val > 0.0 { lhsfinite } else { rhsfinite } {
                            c.boundstightened = 0;
                        }
                    }
                    _ => return Err(Retcode::InvalidData),
                }
            }
        }
        // Update maximal activity delta if a bound was relaxed.
        else if !scip.set.is_infinity(cd(scip, cons).maxactdelta) {
            let lb = scip.vars[var].locdom.lb;
            let ub = scip.vars[var].locdom.ub;

            let domain = ub - lb;
            let delta = val.abs() * domain;

            let c = cdm(scip, cons);
            if delta > c.maxactdelta {
                c.maxactdelta = delta;
                c.maxactdeltavar = var;
            }
        }
    } else if (eventtype & EVENTTYPE_VARFIXED) != EVENTTYPE_DISABLED {
        // We want to remove the fixed variable.
        let c = cdm(scip, cons);
        c.presolved = false;
        c.removedfixings = false;
        c.rangedrowpropagated = 0;

        // Reset maximal activity delta, so that it will be recalculated on the
        // next real propagation.
        if c.maxactdeltavar == var {
            c.maxactdelta = INVALID;
            c.maxactdeltavar = NONE;
        }
    } else if (eventtype & EVENTTYPE_VARUNLOCKED) != EVENTTYPE_DISABLED {
        // There is only one lock left: we may multi-aggregate the variable as
        // slack of an equation.
        cdm(scip, cons).presolved = false;
    } else if (eventtype & EVENTTYPE_GBDCHANGED) != EVENTTYPE_DISABLED {
        let varpos = edata.varpos;

        let oldbound = event.data.oldbound;
        let newbound = event.data.newbound;
        let val = cd(scip, cons).vals[varpos];

        cdm(scip, cons).rangedrowpropagated = 0;

        // Update the activity values.
        with_cd(scip, cons, |scip, c| {
            if (eventtype & EVENTTYPE_GLBCHANGED) != EVENTTYPE_DISABLED {
                consdata_update_activities_glb_lb(scip, c, oldbound, newbound, val, true);
            } else {
                consdata_update_activities_glb_ub(scip, c, oldbound, newbound, val, true);
            }
        });

        // If the variable is binary but not fixed it had to become binary due to
        // this global change.
        if var_is_binary(scip, var) && scip.set.is_gt(scip.vars[var].glbdom.ub, scip.vars[var].glbdom.lb) {
            if scip.set.stage < Stage::InitSolve {
                cdm(scip, cons).indexsorted = false;
            } else {
                cdm(scip, cons).coefsorted = false;
            }
        }
    } else if (eventtype & EVENTTYPE_TYPECHANGED) != EVENTTYPE_DISABLED {
        let becomes = event.data.oldtype != VarType::Continuous || var_is_implied_integral(scip, var);
        let isbinary = var_is_binary(scip, var);
        let c = cdm(scip, cons);

        // For presolving it only matters if a variable becomes integral.
        c.presolved = c.presolved && becomes;

        // The ordering is preserved if the variable remains binary.
        c.indexsorted = c.indexsorted && isbinary && becomes;
    } else if (eventtype & EVENTTYPE_IMPLTYPECHANGED) != EVENTTYPE_DISABLED {
        let becomes = event.data.oldimpltype != ImplintType::None || scip.vars[var].vartype != VarType::Continuous;
        let isbinary = var_is_binary(scip, var);
        let c = cdm(scip, cons);

        // For presolving it only matters if a variable becomes integral.
        c.presolved = c.presolved && becomes;

        // The ordering is preserved if the variable remains binary.
        c.indexsorted = c.indexsorted && isbinary && becomes;
    } else {
        cdm(scip, cons).varsdeleted = true;
    }

    Ok(())
}

/*
 * Callback methods of conflict handler
 */

/// `conflictExecLinear`: conflict processing method of conflict handler
/// (called when conflict was found).
fn conflict_exec_linear(scip: &mut Scip, _conflicthdlr: crate::types::ConflicthdlrId, args: &crate::conflict::ConflictExecArgs, result: &mut ScipResultCode) -> ScipResult {
    // Don't process already resolved conflicts.
    if args.resolved {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Create array of variables and coefficients:
    // sum_{i \in P} x_i - sum_{i \in N} x_i >= 1 - |N|.
    let nbdchginfos = args.bdchginfos.len();
    let mut vars = Vec::with_capacity(nbdchginfos);
    let mut vals = Vec::with_capacity(nbdchginfos);
    let mut lhs = 1.0;
    let mut i = 0;
    while i < nbdchginfos {
        let info = crate::conflict::bdchginfo(scip, args.bdchginfos[i]);
        let var = info.var;
        let newbound = info.newbound;
        vars.push(var);

        // We can only treat binary variables.
        if !var_is_binary(scip, var) {
            break;
        }

        // Check whether the variable is fixed to zero (P) or one (N) in the
        // conflict set.
        if newbound < 0.5 {
            vals.push(1.0);
        } else {
            vals.push(-1.0);
            lhs -= 1.0;
        }
        i += 1;
    }

    if i == nbdchginfos {
        // Create a constraint out of the conflict set.
        let c = scip.conflict.as_ref().unwrap();
        let consname = format!("cf{}", c.nappliedglbconss + c.nappliedlocconss);
        let inf = scip.set.infinity();
        let mut cons = create_cons_linear(
            scip,
            &consname,
            &vars,
            &vals,
            lhs,
            inf,
            false,
            args.separate,
            false,
            false,
            true,
            args.local,
            false,
            args.dynamic,
            args.removable,
            false,
        )?;

        // Try to automatically convert a linear constraint into a more specific
        // and more specialized constraint.
        let upgdcons = upgrade_cons_linear(scip, cons)?;
        if upgdcons != NONE {
            scip_cons::release_cons(scip, &mut cons)?;
            cons = upgdcons;
        }

        // Add conflict to SCIP.
        scip_cons::add_conflict(scip, args.node, &mut cons, args.validnode, args.conftype, args.cutoffinvolved)?;

        *result = ScipResultCode::ConsAdded;
    }

    Ok(())
}

/*
 * constraint specific interface methods
 */

/*
 * Nonlinear constraint upgrading
 */

/// `upgradeConsNonlinear`: tries to upgrade a nonlinear constraint into a
/// linear constraint.
fn upgrade_cons_nonlinear(scip: &mut Scip, cons: ConsId, _nvarexprs: i32, nupgdconss: &mut i32, upgdconss: &mut Vec<ConsId>, _upgdconsssize: i32) -> ScipResult {
    *nupgdconss = 0;

    let expr = crate::cons_nonlinear::get_expr_nonlinear(scip, cons);

    // Not a linear constraint if the expression is not a sum (unless the
    // expression is a variable or a constant or a constant*variable, but these
    // are simplified away in cons_nonlinear).
    if !crate::expr::expr_is_sum(scip, expr) {
        return Ok(());
    }

    // If at least one child is not a variable, then not a linear constraint.
    if scip.exprs[expr].children.iter().any(|&c| !crate::expr::expr_is_var(scip, c)) {
        return Ok(());
    }

    // Consider constant part of the sum expression.
    let constant = crate::expr_sum::get_constant_expr_sum(scip, expr);
    let (clhs, crhs) = (crate::cons_nonlinear::get_lhs_nonlinear(scip, cons), crate::cons_nonlinear::get_rhs_nonlinear(scip, cons));
    let infinity = scip.set.infinity();
    let lhs = if scip.set.is_infinity(-clhs) { -infinity } else { clhs - constant };
    let rhs = if scip.set.is_infinity(crhs) { infinity } else { crhs - constant };

    let f = crate::cons::cons_flags(scip, cons);
    let name = scip.conss[cons].name.clone();
    let upgdcons = create_cons_linear(
        scip,
        &name,
        &[],
        &[],
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

    // Add linear terms.
    let children = scip.exprs[expr].children.clone();
    let coefs = crate::expr_sum::get_coefs_expr_sum(scip, expr).to_vec();
    for (i, &c) in children.iter().enumerate() {
        let var = crate::expr_var::get_var_expr_var(scip, c);
        add_coef_linear(scip, upgdcons, var, coefs[i])?;
    }

    // Check violation of this linear constraint with absolute tolerances, to be
    // consistent with the original nonlinear constraint.
    cdm(scip, upgdcons).checkabsolute = true;

    upgdconss.push(upgdcons);
    *nupgdconss = 1;

    Ok(())
}

/// `SCIPincludeConshdlrLinear`: creates the handler for linear constraints
/// and includes it in SCIP. Sub-SCIP copying, parsing, printing and the
/// symmetry graphs are added with those modules.
pub fn include_conshdlr_linear(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Create event handler for bound change events.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_linear, Some(Box::new(EventhdlrData::default())))?;

    // Create conflict handler for linear constraints.
    api::include_conflicthdlr_basic(scip, CONFLICTHDLR_NAME, CONFLICTHDLR_DESC, CONFLICTHDLR_PRIORITY, conflict_exec_linear, None)?;

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
        Some(cons_enfolp_linear),
        Some(cons_enfops_linear),
        Some(cons_check_linear),
        Some(cons_lock_linear),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.consdeactive = Some(cons_deactive_linear);
    h.consdelete = Some(cons_delete_linear);
    h.consdelvars = Some(cons_delvars_linear);
    h.consexit = Some(cons_exit_linear);
    h.consexitpre = Some(cons_exitpre_linear);
    h.consinitsol = Some(cons_initsol_linear);
    h.consexitsol = Some(cons_exitsol_linear);
    h.consfree = Some(cons_free_linear);
    h.consgetvars = Some(cons_get_vars_linear);
    h.consgetnvars = Some(cons_get_n_vars_linear);
    h.consinit = Some(cons_init_linear);
    h.consinitlp = Some(cons_initlp_linear);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_linear), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_linear), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_linear);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_linear),
        Some(cons_sepasol_linear),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_linear);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_linear);
    scip.conshdlrs[conshdlr].conshdlrcopy = Some(conshdlr_copy_linear);
    scip.conshdlrs[conshdlr].conscopy = Some(cons_copy_linear);

    if crate::set::set_find_conshdlr(scip, "nonlinear") != NONE {
        // Include the linear constraint upgrade in the nonlinear constraint
        // handler.
        crate::cons_nonlinear::include_cons_upgrade_nonlinear(scip, upgrade_cons_nonlinear, NONLINCONSUPGD_PRIORITY, true, CONSHDLR_NAME)?;
    }

    // Add linear constraint handler parameters.
    paramset_add_int(
        scip,
        "constraints/linear/tightenboundsfreq",
        "multiplier on propagation frequency, how often the bounds are tightened (-1: never, 0: only at root)",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, tightenboundsfreq),
        true,
        DEFAULT_TIGHTENBOUNDSFREQ,
        -1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/maxrounds",
        "maximal number of separation rounds per node (-1: unlimited)",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxrounds),
        false,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/maxroundsroot",
        "maximal number of separation rounds per node in the root node (-1: unlimited)",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/maxsepacuts",
        "maximal number of cuts separated per separation round",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxsepacuts),
        false,
        DEFAULT_MAXSEPACUTS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/maxsepacutsroot",
        "maximal number of cuts separated per separation round in the root node",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxsepacutsroot),
        false,
        DEFAULT_MAXSEPACUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/presolpairwise",
        "should pairwise constraint comparison be performed in presolving?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolpairwise),
        true,
        DEFAULT_PRESOLPAIRWISE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/presolusehashing",
        "should hash table be used for detecting redundant constraints in advance",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolusehashing),
        true,
        DEFAULT_PRESOLUSEHASHING,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/nmincomparisons",
        "number for minimal pairwise presolve comparisons",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, nmincomparisons),
        true,
        DEFAULT_NMINCOMPARISONS,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/linear/mingainpernmincomparisons",
        "minimal gain per minimal pairwise presolve comparisons to repeat pairwise comparison round",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, mingainpernmincomp),
        true,
        DEFAULT_MINGAINPERNMINCOMP,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/linear/maxaggrnormscale",
        "maximal allowed relative gain in maximum norm for constraint aggregation (0.0: disable constraint aggregation)",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxaggrnormscale),
        true,
        DEFAULT_MAXAGGRNORMSCALE,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/linear/maxeasyactivitydelta",
        "maximum activity delta to run easy propagation on linear constraint (faster, but numerically less stable)",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxeasyactivitydelta),
        true,
        DEFAULT_MAXEASYACTIVITYDELTA,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/linear/maxcardbounddist",
        "maximal relative distance from current node's dual bound to primal bound compared to best node's dual bound for separating knapsack cardinality cuts",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxcardbounddist),
        true,
        DEFAULT_MAXCARDBOUNDDIST,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/separateall",
        "should all constraints be subject to cardinality cut generation instead of only the ones with non-zero dual value?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, separateall),
        false,
        DEFAULT_SEPARATEALL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/aggregatevariables",
        "should presolving search for aggregations in equations",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, aggregatevariables),
        true,
        DEFAULT_AGGREGATEVARIABLES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/simplifyinequalities",
        "should presolving try to simplify inequalities",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, simplifyinequalities),
        true,
        DEFAULT_SIMPLIFYINEQUALITIES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/dualpresolving",
        "should dual presolving steps be performed?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, dualpresolving),
        true,
        DEFAULT_DUALPRESOLVING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/singletonstuffing",
        "should stuffing of singleton continuous variables be performed?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, singletonstuffing),
        true,
        DEFAULT_SINGLETONSTUFFING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/singlevarstuffing",
        "should single variable stuffing be performed, which tries to fulfill constraints using the cheapest variable?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, singlevarstuffing),
        true,
        DEFAULT_SINGLEVARSTUFFING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/sortvars",
        "apply binaries sorting in decr. order of coeff abs value?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, sortvars),
        true,
        DEFAULT_SORTVARS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/checkrelmaxabs",
        "should the violation for a constraint with side 0.0 be checked relative to 1.0 (FALSE) or to the maximum absolute value in the activity (TRUE)?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, checkrelmaxabs),
        true,
        DEFAULT_CHECKRELMAXABS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/detectcutoffbound",
        "should presolving try to detect constraints parallel to the objective function defining an upper bound and prevent these constraints from entering the LP?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, detectcutoffbound),
        true,
        DEFAULT_DETECTCUTOFFBOUND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/detectlowerbound",
        "should presolving try to detect constraints parallel to the objective function defining a lower bound and prevent these constraints from entering the LP?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, detectlowerbound),
        true,
        DEFAULT_DETECTLOWERBOUND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/detectpartialobjective",
        "should presolving try to detect subsets of constraints parallel to the objective function?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, detectpartialobjective),
        true,
        DEFAULT_DETECTPARTIALOBJECTIVE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/rangedrowpropagation",
        "should presolving and propagation try to improve bounds, detect infeasibility, and extract sub-constraints from ranged rows and equations?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, rangedrowpropagation),
        true,
        DEFAULT_RANGEDROWPROPAGATION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/rangedrowartcons",
        "should presolving and propagation extract sub-constraints from ranged rows and equations?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, rangedrowartcons),
        true,
        DEFAULT_RANGEDROWARTCONS,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/rangedrowmaxdepth",
        "maximum depth to apply ranged row propagation",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, rangedrowmaxdepth),
        true,
        DEFAULT_RANGEDROWMAXDEPTH,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/linear/rangedrowfreq",
        "frequency for applying ranged row propagation",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, rangedrowfreq),
        true,
        DEFAULT_RANGEDROWFREQ,
        1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/multaggrremove",
        "should multi-aggregations only be performed if the constraint can be removed afterwards?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, multaggrremove),
        true,
        DEFAULT_MULTAGGRREMOVE,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/linear/maxmultaggrquot",
        "maximum coefficient dynamism (ie. maxabsval / minabsval) for primal multiaggregation",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxmultaggrquot),
        true,
        DEFAULT_MAXMULTAGGRQUOT,
        1.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/linear/maxdualmultaggrquot",
        "maximum coefficient dynamism (ie. maxabsval / minabsval) for dual multiaggregation",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxdualmultaggrquot),
        true,
        DEFAULT_MAXDUALMULTAGGRQUOT,
        1.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/linear/extractcliques",
        "should Cliques be extracted?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, extractcliques),
        true,
        DEFAULT_EXTRACTCLIQUES,
        None,
    )?;

    Ok(())
}

/// `SCIPincludeLinconsUpgrade`: includes a linear constraint update method
/// into the linear constraint handler.
pub fn include_lincons_upgrade(scip: &mut Scip, linconsupgd: LinConsUpgd, priority: i32, conshdlrname: &str) -> ScipResult {
    // Find the linear constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Linear constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Check if linear constraint update method already exists in constraint
    // handler data.
    if !conshdlrdata_has_upgrade(hd(scip, conshdlr), linconsupgd) {
        // Create a linear constraint upgrade data object.
        let linconsupgrade = linconsupgrade_create(linconsupgd, priority);

        // Insert linear constraint update method into constraint handler data.
        let mut data = scip.conshdlrs[conshdlr].conshdlrdata.take().unwrap();
        conshdlrdata_include_upgrade(scip, data.downcast_mut::<ConshdlrData>().unwrap(), linconsupgrade);
        scip.conshdlrs[conshdlr].conshdlrdata = Some(data);

        // Adds parameter to turn on and off the upgrading step; the upgrade is
        // found by its method, since inserting sorts the list.
        let paramname = format!("constraints/linear/upgrade/{conshdlrname}");
        let paramdesc = format!("enable linear upgrading for constraint handler <{conshdlrname}>");
        let find = move |d: &ConshdlrData| d.linconsupgrades.iter().position(|u| std::ptr::fn_addr_eq(u.linconsupgd, linconsupgd)).unwrap();
        let target = std::rc::Rc::new(crate::paramset::Target::<bool> {
            get: Box::new(move |scip: &Scip| {
                let d = hd(scip, conshdlr);
                &d.linconsupgrades[find(d)].active
            }),
            get_mut: Box::new(move |scip: &mut Scip| {
                let d = hdm(scip, conshdlr);
                let i = find(d);
                &mut d.linconsupgrades[i].active
            }),
        });
        crate::paramset::paramset_add_bool(scip, &paramname, &paramdesc, Some(target), false, true, None)?;
    }

    Ok(())
}

/// `SCIPcreateConsLinear`: creates and captures a linear constraint.
#[allow(clippy::too_many_arguments)]
pub fn create_cons_linear(
    scip: &mut Scip,
    name: &str,
    vars: &[VarId],
    vals: &[f64],
    mut lhs: f64,
    mut rhs: f64,
    initial: bool,
    separate: bool,
    enforce: bool,
    check: bool,
    propagate: bool,
    local: bool,
    modifiable: bool,
    dynamic: bool,
    removable: bool,
    stickingatnode: bool,
) -> ScipResult<ConsId> {
    // Find the linear constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Linear constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Terminate if a coefficient is infinite.
    for &val in vals {
        if scip.set.is_infinity(val.abs()) {
            // Coefficient is infinite, consider adjusting the infinity threshold.
            return Err(Retcode::InvalidData);
        }
    }

    // For the solving process we need linear rows, containing only active
    // variables; therefore when creating a linear constraint after presolving
    // we have to ensure that it holds active variables.
    let consdata = if scip.set.stage >= Stage::ExitPresolve && !vars.is_empty() {
        let mut nconsvars = vars.len() as i32;
        let mut consvars = CVec(vars.to_vec());
        let mut consvals = CVec(vals.to_vec());
        let mut constant = 0.0;
        let mut requiredsize = 0;

        // Get active variables for new constraint.
        let varssize = nconsvars;
        crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, varssize, &mut constant, &mut requiredsize)?;

        // If space was not enough we need to resize the buffers.
        if requiredsize > nconsvars {
            consvars.0.resize(requiredsize as usize, NONE);
            consvals.0.resize(requiredsize as usize, 0.0);

            crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, requiredsize, &mut constant, &mut requiredsize)?;
        }

        // Adjust sides and check that we do not subtract infinity values.
        if scip.set.is_infinity(constant.abs()) {
            // Inactive variables lead to an infinite constant.
            return Err(Retcode::InvalidData);
        } else {
            if !scip.set.is_infinity(lhs.abs()) {
                lhs -= constant;
            }
            if !scip.set.is_infinity(rhs.abs()) {
                rhs -= constant;
            }

            if scip.set.is_infinity(-lhs) {
                lhs = -scip.set.infinity();
            } else if scip.set.is_infinity(lhs) {
                lhs = scip.set.infinity();
            }

            if scip.set.is_infinity(rhs) {
                rhs = scip.set.infinity();
            } else if scip.set.is_infinity(-rhs) {
                rhs = -scip.set.infinity();
            }
        }

        // Create constraint data.
        let n = nconsvars as usize;
        consdata_create(scip, &consvars.0[..n], &consvals.0[..n], lhs, rhs)?
    } else {
        // Create constraint data.
        consdata_create(scip, vars, vals, lhs, rhs)?
    };

    // Create constraint.
    let flags = crate::cons::ConsFlags {
        initial,
        separate,
        enforce,
        check,
        propagate,
        local,
        modifiable,
        dynamic,
        removable,
        stickingatnode,
    };
    scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), &flags)
}

/// `SCIPcreateConsBasicLinear`: creates and captures a linear constraint
/// in its most basic version, i. e., all constraint flags are set to their
/// basic value as explained for the method SCIPcreateConsLinear().
pub fn create_cons_basic_linear(scip: &mut Scip, name: &str, vars: &[VarId], vals: &[f64], lhs: f64, rhs: f64) -> ScipResult<ConsId> {
    create_cons_linear(scip, name, vars, vals, lhs, rhs, true, true, true, true, true, false, false, false, false, false)
}

/// Whether the constraint belongs to the linear constraint handler.
fn is_linear(scip: &Scip, cons: ConsId) -> bool {
    scip.conshdlrs[scip.conss[cons].conshdlr].name == CONSHDLR_NAME
}

/// `SCIPaddCoefLinear`: adds coefficient to linear constraint (if it is not
/// zero).
pub fn add_coef_linear(scip: &mut Scip, cons: ConsId, var: VarId, val: f64) -> ScipResult {
    if !is_linear(scip, cons) {
        // Constraint is not linear.
        return Err(Retcode::InvalidData);
    }

    // Terminate if coefficient is infinite.
    if scip.set.is_infinity(val.abs()) {
        return Err(Retcode::InvalidData);
    }

    // For the solving process we need linear rows, containing only active
    // variables; therefore when creating a linear constraint after presolving
    // we have to ensure that it holds active variables.
    if scip.set.stage >= Stage::ExitPresolve {
        let mut nconsvars = 1;
        let mut consvars = CVec(vec![var]);
        let mut consvals = CVec(vec![val]);
        let mut constant = 0.0;
        let mut requiredsize = 0;

        // Get active variables for new constraint.
        let varssize = nconsvars;
        crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, varssize, &mut constant, &mut requiredsize)?;

        // If space was not enough we need to resize the buffers.
        if requiredsize > nconsvars {
            consvars.0.resize(requiredsize as usize, NONE);
            consvals.0.resize(requiredsize as usize, 0.0);

            crate::var::var_get_active_representatives(scip, &mut consvars, &mut consvals, &mut nconsvars, requiredsize, &mut constant, &mut requiredsize)?;
        }

        let mut lhs = cd(scip, cons).lhs;
        let mut rhs = cd(scip, cons).rhs;

        // Adjust sides and check that we do not subtract infinity values.
        // Constant is infinite.
        if scip.set.is_infinity(constant.abs()) {
            if constant < 0.0 {
                if scip.set.is_infinity(lhs) || scip.set.is_infinity(rhs) {
                    // Adding the variable leads to an inconsistent constraint.
                    return Err(Retcode::InvalidData);
                }

                lhs = -scip.set.infinity();
                rhs = -scip.set.infinity();
            } else {
                if scip.set.is_infinity(-lhs) || scip.set.is_infinity(-rhs) {
                    // Adding the variable leads to an inconsistent constraint.
                    return Err(Retcode::InvalidData);
                }

                lhs = scip.set.infinity();
                rhs = scip.set.infinity();
            }
        }
        // Constant is not infinite.
        else {
            if !scip.set.is_infinity(lhs.abs()) {
                lhs -= constant;
            }
            if !scip.set.is_infinity(rhs.abs()) {
                rhs -= constant;
            }

            if scip.set.is_infinity(-lhs) {
                lhs = -scip.set.infinity();
            } else if scip.set.is_infinity(lhs) {
                lhs = scip.set.infinity();
            }

            if scip.set.is_infinity(rhs) {
                rhs = scip.set.infinity();
            } else if scip.set.is_infinity(-rhs) {
                rhs = -scip.set.infinity();
            }
        }

        // Add all active variables to constraint.
        let mut v = nconsvars - 1;
        while v >= 0 {
            if !scip.set.is_zero(consvals[v]) {
                add_coef(scip, cons, consvars[v], consvals[v])?;
            }
            v -= 1;
        }

        // Update left and right hand sides.
        chg_lhs(scip, cons, lhs)?;
        chg_rhs(scip, cons, rhs)?;
    } else if !scip.set.is_zero(val) {
        add_coef(scip, cons, var, val)?;
    }

    Ok(())
}

/// `SCIPchgCoefLinear`: changes coefficient of variable in linear
/// constraint; deletes the variable if coefficient is zero; adds variable if
/// not yet contained in the constraint. May only be called during problem
/// creation stage for an original constraint and variable.
pub fn chg_coef_linear(scip: &mut Scip, cons: ConsId, var: VarId, val: f64) -> ScipResult {
    if !is_linear(scip, cons) {
        // Constraint is not linear.
        return Err(Retcode::InvalidData);
    }

    if scip.set.stage > Stage::Problem || !scip.conss[cons].original || scip.vars[var].varstatus != VarStatus::Original {
        // Method may only be called during problem creation stage for original
        // constraints and variables.
        return Err(Retcode::InvalidData);
    }

    let mut found = false;
    let mut i = 0;
    while i < cd(scip, cons).nvars {
        if cd(scip, cons).vars[i] == var {
            if found || scip.set.is_zero(val) {
                del_coef_pos(scip, cons, i)?;

                // Decrease i by one since otherwise we would skip the coefficient
                // which has been switched to position i.
                i -= 1;
            } else {
                chg_coef_pos(scip, cons, i, val)?;
            }
            found = true;
        }
        i += 1;
    }

    if !found {
        add_coef_linear(scip, cons, var, val)?;
    }

    Ok(())
}

/// `SCIPdelCoefLinear`: deletes variable from linear constraint.
pub fn del_coef_linear(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    chg_coef_linear(scip, cons, var, 0.0)
}

/// `SCIPgetLhsLinear`: gets left hand side of linear constraint.
pub fn get_lhs_linear(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    cd(scip, cons).lhs
}

/// `SCIPgetRhsLinear`: gets right hand side of linear constraint.
pub fn get_rhs_linear(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    cd(scip, cons).rhs
}

/// `SCIPchgLhsLinear`: changes left hand side of linear constraint.
pub fn chg_lhs_linear(scip: &mut Scip, cons: ConsId, lhs: f64) -> ScipResult {
    if !is_linear(scip, cons) {
        // Constraint is not linear.
        return Err(Retcode::InvalidData);
    }
    chg_lhs(scip, cons, lhs)
}

/// `SCIPchgRhsLinear`: changes right hand side of linear constraint.
pub fn chg_rhs_linear(scip: &mut Scip, cons: ConsId, rhs: f64) -> ScipResult {
    if !is_linear(scip, cons) {
        // Constraint is not linear.
        return Err(Retcode::InvalidData);
    }
    chg_rhs(scip, cons, rhs)
}

/// `SCIPgetNVarsLinear`: gets the number of variables in the linear
/// constraint.
pub fn get_n_vars_linear(scip: &Scip, cons: ConsId) -> i32 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    cd(scip, cons).nvars
}

/// `SCIPgetVarsLinear`: gets the variables in the linear constraint.
pub fn get_vars_linear(scip: &Scip, cons: ConsId) -> &[VarId] {
    assert!(is_linear(scip, cons), "constraint is not linear");
    let c = cd(scip, cons);
    &c.vars.0[..c.nvars as usize]
}

/// `SCIPgetValsLinear`: gets the coefficient values in the linear
/// constraint.
pub fn get_vals_linear(scip: &Scip, cons: ConsId) -> &[f64] {
    assert!(is_linear(scip, cons), "constraint is not linear");
    let c = cd(scip, cons);
    &c.vals.0[..c.nvars as usize]
}

/// `SCIPgetActivityLinear`: gets the activity of the linear constraint in
/// the given solution (`NONE` for the current node's solution).
pub fn get_activity_linear(scip: &mut Scip, cons: ConsId, sol: SolId) -> f64 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    let row = cd(scip, cons).row;
    if row != NONE {
        api::get_row_sol_activity(scip, row, sol)
    } else {
        with_cd(scip, cons, |scip, c| consdata_get_activity(scip, c, sol))
    }
}

/// `SCIPgetFeasibilityLinear`: gets the feasibility of the linear
/// constraint in the given solution.
pub fn get_feasibility_linear(scip: &mut Scip, cons: ConsId, sol: SolId) -> f64 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    let row = cd(scip, cons).row;
    if row != NONE {
        scip_lp::get_row_sol_feasibility(scip, row, sol)
    } else {
        with_cd(scip, cons, |scip, c| consdata_get_feasibility(scip, c, sol))
    }
}

/// `SCIPgetDualsolLinear`: gets the dual solution of the linear constraint
/// in the current LP.
pub fn get_dualsol_linear(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualsol(scip, row) } else { 0.0 }
}

/// `SCIPgetDualfarkasLinear`: gets the dual Farkas value of the linear
/// constraint in the current infeasible LP.
pub fn get_dualfarkas_linear(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_linear(scip, cons), "constraint is not linear");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualfarkas(scip, row) } else { 0.0 }
}

/// `SCIPgetRowLinear`: returns the linear relaxation of the given linear
/// constraint; `NONE` if no LP row was yet created.
pub fn get_row_linear(scip: &Scip, cons: ConsId) -> crate::types::RowId {
    assert!(is_linear(scip, cons), "constraint is not linear");
    cd(scip, cons).row
}

/// `SCIPcreateRowLinear`: creates the row of the given linear constraint.
pub fn create_row_linear(scip: &mut Scip, cons: ConsId) -> ScipResult {
    if !is_linear(scip, cons) {
        return Err(Retcode::Error);
    }
    create_row(scip, cons)
}

/// `SCIPupgradeConsLinear`: tries to automatically convert a linear
/// constraint into a more specific and more specialized constraint; returns
/// the upgraded constraint, or `NONE` if not successful.
pub fn upgrade_cons_linear(scip: &mut Scip, cons: ConsId) -> ScipResult<ConsId> {
    let mut upgdcons = NONE;

    // We cannot upgrade a modifiable linear constraint, since we don't know
    // what additional coefficients to expect.
    if scip.conss[cons].modifiable {
        return Ok(NONE);
    }

    // Check for upgradability.
    if scip.conss[cons].nupgradelocks > 0 {
        return Ok(NONE);
    }

    // Get the constraint handler and check, if it's really a linear
    // constraint.
    if !is_linear(scip, cons) {
        return Err(Retcode::InvalidData);
    }
    let conshdlr = scip.conss[cons].conshdlr;

    // Check, if the constraint was already upgraded and will be deleted anyway
    // after preprocessing.
    if cd(scip, cons).upgraded {
        return Ok(NONE);
    }

    // Check, if the constraint is already stored as LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        if scip.rows[row].lppos >= 0 {
            // Cannot upgrade linear constraint that is already stored as row in
            // the LP.
            return Err(Retcode::InvalidData);
        } else {
            crate::lp::row_release(scip, row)?;
            cdm(scip, cons).row = NONE;
        }
    }

    // normalizeCons() can only detect infeasibility when scaling with the
    // gcd. in that case, the scaling was skipped and we hope that the
    // infeasibility gets detected later again.
    if normalize_cons(scip, cons)? {
        return Ok(NONE);
    }

    // Tighten sides.
    let mut nchgsides = 0;
    if tighten_sides(scip, cons, &mut nchgsides)? {
        return Ok(NONE);
    }

    //
    // calculate some statistics on linear constraint
    //
    let c = cd(scip, cons);
    let n = c.nvars as usize;
    let mut s = LinConsStats {
        vars: c.vars.0[..n].to_vec(),
        vals: c.vals.0[..n].to_vec(),
        lhs: c.lhs,
        rhs: c.rhs,
        nposbin: 0,
        nnegbin: 0,
        nposint: 0,
        nnegint: 0,
        nposimpl: 0,
        nnegimpl: 0,
        nposimplbin: 0,
        nnegimplbin: 0,
        nposcont: 0,
        nnegcont: 0,
        ncoeffspone: 0,
        ncoeffsnone: 0,
        ncoeffspint: 0,
        ncoeffsnint: 0,
        ncoeffspfrac: 0,
        ncoeffsnfrac: 0,
        poscoeffsum: 0.0,
        negcoeffsum: 0.0,
        integral: true,
    };
    for i in 0..n {
        let var = s.vars[i];
        let val = s.vals[i];
        let lb = scip.vars[var].locdom.lb;
        let ub = scip.vars[var].locdom.ub;

        if var_is_implied_integral(scip, var) {
            if var_is_binary(scip, var) {
                if val >= 0.0 {
                    s.nposimplbin += 1;
                } else {
                    s.nnegimplbin += 1;
                }
            }
            if !scip.set.is_zero(lb) || !scip.set.is_zero(ub) {
                s.integral = s.integral && scip.set.is_integral(val);
            }
            if val >= 0.0 {
                s.nposimpl += 1;
            } else {
                s.nnegimpl += 1;
            }
        } else {
            match scip.vars[var].vartype {
                VarType::Binary => {
                    if !scip.set.is_zero(lb) || !scip.set.is_zero(ub) {
                        s.integral = s.integral && scip.set.is_integral(val);
                    }
                    if val >= 0.0 {
                        s.nposbin += 1;
                    } else {
                        s.nnegbin += 1;
                    }
                }
                VarType::Integer => {
                    if !scip.set.is_zero(lb) || !scip.set.is_zero(ub) {
                        s.integral = s.integral && scip.set.is_integral(val);
                    }
                    if val >= 0.0 {
                        s.nposint += 1;
                    } else {
                        s.nnegint += 1;
                    }
                }
                VarType::Continuous => {
                    s.integral = s.integral && scip.set.is_eq(lb, ub) && scip.set.is_integral(val * lb);
                    if val >= 0.0 {
                        s.nposcont += 1;
                    } else {
                        s.nnegcont += 1;
                    }
                }
                _ => return Err(Retcode::InvalidData),
            }
        }
        if scip.set.is_eq(val, 1.0) {
            s.ncoeffspone += 1;
        } else if scip.set.is_eq(val, -1.0) {
            s.ncoeffsnone += 1;
        } else if scip.set.is_integral(val) {
            if scip.set.is_positive(val) {
                s.ncoeffspint += 1;
            } else {
                s.ncoeffsnint += 1;
            }
        } else if scip.set.is_positive(val) {
            s.ncoeffspfrac += 1;
        } else {
            s.ncoeffsnfrac += 1;
        }
        if scip.set.is_positive(val) {
            s.poscoeffsum += val;
        } else {
            s.negcoeffsum += val;
        }
    }

    //
    // call the upgrading methods
    //

    // Try all upgrading methods in priority order in case the upgrading step is
    // enable.
    let nupgrades = hd(scip, conshdlr).nlinconsupgrades as usize;
    let mut i = 0;
    while i < nupgrades && upgdcons == NONE {
        let u = hd(scip, conshdlr).linconsupgrades[i];
        if u.active {
            (u.linconsupgd)(scip, cons, &s, &mut upgdcons)?;
        }
        i += 1;
    }

    Ok(upgdcons)
}

/// `SCIPcleanupConssLinear`: cleans up (multi-)aggregations and fixings
/// from linear constraints; returns whether the problem was detected to be
/// infeasible.
pub fn cleanup_conss_linear(scip: &mut Scip, onlychecked: bool, ndelconss: &mut i32) -> ScipResult<bool> {
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
        infeasible = apply_fixings(scip, cons, true)?;

        if infeasible {
            break;
        }

        if cd(scip, cons).nvars >= 1 {
            continue;
        }

        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
    }

    Ok(infeasible)
}

/// `SCIPcopyConsLinear`: a linear constraint in `scip` copying `sourcevars`
/// (with `sourcecoefs`, all ones if `None`) of `sourcescip`, after
/// replacing them by their active representations; `NONE` if a variable
/// could not be copied. Returns `(cons, valid)`.
#[allow(clippy::too_many_arguments)]
pub fn copy_cons_linear(
    scip: &mut Scip,
    sourcescip: &mut Scip,
    name: &str,
    sourcevars: &[VarId],
    sourcecoefs: Option<&[f64]>,
    mut lhs: f64,
    mut rhs: f64,
    maps: &mut crate::copy::CopyMaps,
    flags: &ConsFlags,
    global: bool,
) -> ScipResult<(ConsId, bool)> {
    let f = flags;
    if scip.set.is_gt(lhs, rhs) {
        return Ok((NONE, false));
    }

    let mut nvars = sourcevars.len() as i32;
    if nvars == 0 {
        let cons = create_cons_linear(
            scip,
            name,
            &[],
            &[],
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
        return Ok((cons, true));
    }

    // Duplicate variable array and coefficient array.
    let mut vars = CVec(sourcevars.to_vec());
    let mut coefs = CVec(match sourcecoefs {
        Some(c) => c.to_vec(),
        None => vec![1.0; nvars as usize],
    });
    let mut constant = 0.0;

    // Transform source variable to active variables of the source SCIP since
    // only these can be mapped to variables of the target SCIP.
    if !crate::var::var_is_original(sourcescip, vars[0]) {
        let mut requiredsize = 0;
        let varssize = nvars;
        crate::var::var_get_active_representatives(sourcescip, &mut vars, &mut coefs, &mut nvars, varssize, &mut constant, &mut requiredsize)?;
        if requiredsize > nvars {
            vars.realloc(requiredsize);
            coefs.realloc(requiredsize);
            crate::var::var_get_active_representatives(sourcescip, &mut vars, &mut coefs, &mut nvars, requiredsize, &mut constant, &mut requiredsize)?;
        }
    } else {
        for v in 0..nvars {
            crate::var::var_get_origvar_sum(sourcescip, &mut vars[v], &mut coefs[v], &mut constant)?;
        }
    }

    // Map variables of the source constraint to variables of the target
    // SCIP.
    let mut success = true;
    let mut v = 0;
    while v < nvars && success {
        let (targetvar, s) = crate::copy::get_var_copy(sourcescip, scip, vars[v], Some(maps), global)?;
        vars[v] = targetvar;
        success = s;
        v += 1;
    }

    // Only create the target constraint, if all variables could be copied.
    if success {
        if !scip.set.is_infinity(-lhs) {
            lhs -= constant;
        }
        if !scip.set.is_infinity(rhs) {
            rhs -= constant;
        }
        let n = nvars as usize;
        let cons = create_cons_linear(
            scip,
            name,
            &vars.0[..n],
            &coefs.0[..n],
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
        Ok((cons, true))
    } else {
        Ok((NONE, false))
    }
}

/// `conshdlrCopyLinear`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_linear(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_linear(scip)?;
    *valid = true;
    Ok(())
}

/// `consCopyLinear`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_linear(
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
    let sourcevars = get_vars_linear(sourcescip, sourcecons).to_vec();
    let sourcecoefs = get_vals_linear(sourcescip, sourcecons).to_vec();
    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);
    let (lhs, rhs) = (get_lhs_linear(sourcescip, sourcecons), get_rhs_linear(sourcescip, sourcecons));

    (*cons, *valid) = copy_cons_linear(scip, sourcescip, &consname, &sourcevars, Some(&sourcecoefs), lhs, rhs, maps, flags, global)?;
    Ok(())
}
