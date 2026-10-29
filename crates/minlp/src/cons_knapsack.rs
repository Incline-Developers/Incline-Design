//! SCIP's knapsack constraint handler (`cons_knapsack.c`): a^T x <= b with
//! binary x and nonnegative integral weights; propagation, presolving
//! (coefficient tightening, clique extraction, disaggregation), separation
//! of lifted cover inequalities, and the upgrade of linear constraints.
//!
//! Like the linear handler, the constraint data is read through [`cd`] and
//! [`cdm`] afresh after any call that may trigger events, and the event data
//! lives in the event handler's arena. The cardinality upgrade
//! (`WITH_CARDINALITY_UPGRADE`) is not compiled in the reference build.

use std::any::Any;

use crate::{
    api,
    arena::Arena,
    cons::{ConsFlags, *},
    cvec::CVec,
    event::*,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var, sort,
    types::{BoundType, ConsId, ConshdlrId, EventhdlrId, LockType, ScipResultCode, SolId, VarId, VarStatus},
    var::{BdChgIdx, NONE, var_compare, var_is_binary},
};

/* constraint handler properties */
pub const CONSHDLR_NAME: &str = "knapsack";
const CONSHDLR_DESC: &str = "knapsack constraint of the form  a^T x <= b, x binary and a >= 0";
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 600000;
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -600000;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -600000;
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

const EVENTHDLR_NAME: &str = "knapsack";
const EVENTHDLR_DESC: &str = "bound change event handler for knapsack constraints";
/// Variable events that should be caught by the event handler.
const EVENTTYPE_KNAPSACK: EventType = EVENTTYPE_LBCHANGED | EVENTTYPE_UBTIGHTENED | EVENTTYPE_VARFIXED | EVENTTYPE_VARDELETED | EVENTTYPE_IMPLADDED;

/// Priority of the constraint handler for upgrading of linear constraints.
const LINCONSUPGD_PRIORITY: i32 = 100000;

/// Maximal number of items in knapsack where clique information is used.
const MAX_USECLIQUES_SIZE: i32 = 1000;
/// Maximal number of items to store in the zero list in preprocessing.
const MAX_ZEROITEMS_SIZE: i32 = 10000;

/// Maximal allowed rounding distance for scaling in knapsack relaxation.
const KNAPSACKRELAX_MAXDELTA: f64 = 0.1;
/// Maximal allowed denominator in knapsack rational relaxation.
const KNAPSACKRELAX_MAXDNOM: i64 = 1000;
/// Maximal allowed scaling factor in knapsack rational relaxation.
const KNAPSACKRELAX_MAXSCALE: f64 = 1000.0;

/// Multiplier on separation frequency, how often knapsack cuts are
/// separated.
const DEFAULT_SEPACARDFREQ: i32 = 1;
/// Maximal number of separation rounds per node (-1: unlimited).
const DEFAULT_MAXROUNDS: i32 = 5;
/// Maximal number of separation rounds in the root node (-1: unlimited).
const DEFAULT_MAXROUNDSROOT: i32 = -1;
/// Maximal number of cuts separated per separation round.
const DEFAULT_MAXSEPACUTS: i32 = 50;
/// Maximal number of cuts separated per separation round in the root node.
const DEFAULT_MAXSEPACUTSROOT: i32 = 200;
/// Maximal relative distance from current node's dual bound to primal bound
/// compared to best node's dual bound for separating knapsack cuts.
const DEFAULT_MAXCARDBOUNDDIST: f64 = 0.0;
/// Should disaggregation of knapsack constraints be allowed in
/// preprocessing?
const DEFAULT_DISAGGREGATION: bool = true;
/// Should presolving try to simplify knapsacks.
const DEFAULT_SIMPLIFYINEQUALITIES: bool = true;
/// Should negated clique information be used in solving process.
const DEFAULT_NEGATEDCLIQUE: bool = true;

/// Maximal absolute coefficient in variable bounds used for knapsack
/// relaxation.
const MAXABSVBCOEF: f64 = 1e+5;
/// Should lifted minimal cover inequalities using superadditive up-lifting
/// be separated in addition.
const USESUPADDLIFT: bool = false;

/// Should hash table be used for detecting redundant constraints in
/// advance.
const DEFAULT_PRESOLUSEHASHING: bool = true;
/// Minimal size of hash table in linear constraint tables.
const HASHSIZE_KNAPSACKCONS: i32 = 500;

/// Should pairwise constraint comparison be performed in presolving?
const DEFAULT_PRESOLPAIRWISE: bool = true;
/// Number for minimal pairwise presolving comparisons.
const NMINCOMPARISONS: i32 = 200000;
/// Minimal gain per minimal pairwise presolving comparisons to repeat
/// pairwise comparison round.
const MINGAINPERNMINCOMPARISONS: f64 = 1e-06;

/// Should dual presolving steps be performed?
const DEFAULT_DUALPRESOLVING: bool = true;
/// Should presolving try to detect constraints parallel to the objective
/// function defining an upper bound and prevent these constraints from
/// entering the LP.
const DEFAULT_DETECTCUTOFFBOUND: bool = true;
/// Should presolving try to detect constraints parallel to the objective
/// function defining a lower bound and prevent these constraints from
/// entering the LP.
const DEFAULT_DETECTLOWERBOUND: bool = true;
/// Lower clique size limit for greedy clique extraction algorithm (relative
/// to largest clique).
const DEFAULT_CLIQUEEXTRACTFACTOR: f64 = 0.5;
/// Maximal size for which LEWI are iteratively separated by reducing the
/// feasible set.
const MAXCOVERSIZEITERLEWI: i32 = 1000;

/// Should GUB information be used for separation?
const DEFAULT_USEGUBS: bool = false;
/// Factor on the growth of global cliques to decide when to update a
/// previous (negated) clique partition (used only if
/// updatecliquepartitions is set to TRUE).
const DEFAULT_CLQPARTUPDATEFAC: f64 = 1.5;
/// Should clique partition information be updated when old partition seems
/// outdated?
const DEFAULT_UPDATECLIQUEPARTITIONS: bool = false;

/*
 * Data structures
 */

/// `SCIP_ConshdlrData`: constraint handler data.
#[derive(Default)]
pub struct ConshdlrData {
    /// Cleared memory array, all entries are set to zero in initpre, if you
    /// use this you have to clear it at the end, exists only in presolving
    /// stage.
    pub ints1: CVec<i32>,
    pub ints2: CVec<i32>,
    pub longints1: CVec<i64>,
    pub longints2: CVec<i64>,
    pub bools1: CVec<bool>,
    pub bools2: CVec<bool>,
    pub bools3: CVec<bool>,
    pub bools4: CVec<bool>,
    /// Cleared memory array, all entries are set to zero in consinit, if you
    /// use this you have to clear it at the end.
    pub reals1: CVec<f64>,
    pub ints1size: i32,
    pub ints2size: i32,
    pub longints1size: i32,
    pub longints2size: i32,
    pub bools1size: i32,
    pub bools2size: i32,
    pub bools3size: i32,
    pub bools4size: i32,
    pub reals1size: i32,
    /// Cleared memory array with default values -1; used for clique
    /// partitions.
    pub probtoidxmap: CVec<i32>,
    pub probtoidxmapsize: i32,
    /// Event handler for bound change events.
    pub eventhdlr: EventhdlrId,
    /// Maximal relative distance from current node's dual bound to primal
    /// bound compared to best node's dual bound for separating knapsack cuts.
    pub maxcardbounddist: f64,
    /// Multiplier on separation frequency, how often knapsack cuts are
    /// separated.
    pub sepacardfreq: i32,
    /// Maximal number of separation rounds per node (-1: unlimited).
    pub maxrounds: i32,
    /// Maximal number of separation rounds in the root node (-1: unlimited).
    pub maxroundsroot: i32,
    /// Maximal number of cuts separated per separation round.
    pub maxsepacuts: i32,
    /// Maximal number of cuts separated per separation round in the root
    /// node.
    pub maxsepacutsroot: i32,
    /// Should disaggregation of knapsack constraints be allowed in
    /// preprocessing?
    pub disaggregation: bool,
    /// Should presolving try to cancel down or delete coefficients in
    /// inequalities.
    pub simplifyinequalities: bool,
    /// Should negated clique information be used in solving process.
    pub negatedclique: bool,
    /// Should pairwise constraint comparison be performed in presolving?
    pub presolpairwise: bool,
    /// Should hash table be used for detecting redundant constraints in
    /// advance.
    pub presolusehashing: bool,
    /// Should dual presolving steps be performed?
    pub dualpresolving: bool,
    /// Should GUB information be used for separation?
    pub usegubs: bool,
    /// Should presolving try to detect constraints parallel to the objective
    /// function defining an upper bound and prevent these constraints from
    /// entering the LP.
    pub detectcutoffbound: bool,
    /// Should presolving try to detect constraints parallel to the objective
    /// function defining a lower bound and prevent these constraints from
    /// entering the LP.
    pub detectlowerbound: bool,
    /// Should clique partition information be updated when old partition
    /// seems outdated?
    pub updatecliquepartitions: bool,
    /// Lower clique size limit for greedy clique extraction algorithm
    /// (relative to largest clique).
    pub cliqueextractfactor: f64,
    /// Factor on the growth of global cliques to decide when to update a
    /// previous (negated) clique partition (used only if
    /// updatecliquepartitions is set to TRUE).
    pub clqpartupdatefac: f64,
}

/// `SCIP_ConsData`: constraint data for knapsack constraints.
#[derive(Clone, Debug, Default)]
pub struct ConsData {
    /// Variables in knapsack constraint.
    pub vars: CVec<VarId>,
    /// Weights of variables in knapsack constraint.
    pub weights: CVec<i64>,
    /// Event data for bound change events of the variables (ids in the event
    /// handler's arena); `None` outside the transformed problem.
    pub eventdata: Option<CVec<usize>>,
    /// Clique indices of the clique partition.
    pub cliquepartition: Option<CVec<i32>>,
    /// Clique indices of the negated clique partition.
    pub negcliquepartition: Option<CVec<i32>>,
    /// Corresponding LP row.
    pub row: usize,
    /// Number of variables in knapsack constraint.
    pub nvars: i32,
    /// Size of vars, weights, and eventdata arrays.
    pub varssize: i32,
    /// Number of cliques in the clique partition.
    pub ncliques: i32,
    /// Number of cliques in the negated clique partition.
    pub nnegcliques: i32,
    /// Number of global cliques the last time a negated clique partition was
    /// computed.
    pub ncliqueslastnegpart: i32,
    /// Number of global cliques the last time a clique partition was
    /// computed.
    pub ncliqueslastpart: i32,
    /// Capacity of knapsack.
    pub capacity: i64,
    /// Sum of all weights.
    pub weightsum: i64,
    /// Sum of weights of variables fixed to one.
    pub onesweightsum: i64,
    /// Max level in which the knapsack constraint is already presolved.
    pub presolvedtiming: PresolTiming,
    /// Are the knapsack items sorted by weight?
    pub sorted: bool,
    /// Is the clique partition valid?
    pub cliquepartitioned: bool,
    /// Is the negated clique partition valid?
    pub negcliquepartitioned: bool,
    /// Are the constraint's equal variables already merged?
    pub merged: bool,
    /// Were the cliques of the knapsack already added to clique table?
    pub cliquesadded: bool,
    /// Were variables deleted after last cleanup?
    pub varsdeleted: bool,
    /// Does this constraint contain multi-aggregations.
    pub existmultaggr: bool,
}

/// `SCIP_EventData`: event data for bound changes events.
#[derive(Clone, Copy, Debug)]
pub struct EventData {
    /// Knapsack constraint to process the bound change for.
    pub cons: ConsId,
    /// Weight of variable.
    pub weight: i64,
    /// Position of event in variable's event filter.
    pub filterpos: i32,
}

/// The data of the knapsack event handler: the event data of all
/// constraints.
#[derive(Default)]
pub struct EventhdlrData {
    pub eventdatas: Arena<EventData>,
}

/// `SORTKEYPAIR`: data structure to combine two sorting key values.
#[derive(Clone, Copy, Debug, Default)]
struct SortKeyPair {
    /// First sort key value.
    key1: f64,
    /// Second sort key value.
    key2: f64,
}

/// The constraint data of a knapsack constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a knapsack constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the knapsack constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/// The constraint handler data of the knapsack constraint handler, mutably.
#[inline]
fn hdm(scip: &mut Scip, h: ConshdlrId) -> &mut ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_mut().unwrap().downcast_mut::<ConshdlrData>().unwrap()
}

/// The event data arena of the knapsack event handler.
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

/// The event data `id` of the knapsack event handler.
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

/// Runs `f` on the constraint data taken out of the constraint; for the
/// methods that only read SCIP while changing the data.
fn with_cd<R>(scip: &mut Scip, cons: ConsId, f: impl FnOnce(&mut Scip, &mut ConsData) -> R) -> R {
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let r = f(scip, data.downcast_mut::<ConsData>().unwrap());
    scip.conss[cons].consdata = Some(data);
    r
}

/// The event handler of the knapsack constraint handler of `cons`.
fn cons_eventhdlr(scip: &Scip, cons: ConsId) -> EventhdlrId {
    hd(scip, scip.conss[cons].conshdlr).eventhdlr
}

/*
 * Local methods
 */

/// `compSortkeypairs`: comparison method for two sorting key pairs.
fn comp_sortkeypairs(sortkeypair1: &SortKeyPair, sortkeypair2: &SortKeyPair) -> i32 {
    if sortkeypair1.key1 < sortkeypair2.key1 {
        -1
    } else if sortkeypair1.key1 > sortkeypair2.key1 {
        1
    } else if sortkeypair1.key2 < sortkeypair2.key2 {
        -1
    } else if sortkeypair1.key2 > sortkeypair2.key2 {
        1
    } else {
        0
    }
}

/// `sortItems`: sorts items in knapsack with nonincreasing weights.
fn sort_items(scip: &Scip, c: &mut ConsData) {
    if !c.sorted {
        let n = c.nvars as usize;
        let eventdata = c.eventdata.as_mut().unwrap();
        let cliquepartition = c.cliquepartition.as_mut().unwrap();
        let negcliquepartition = c.negcliquepartition.as_mut().unwrap();

        // Sort of five joint arrays of Long/pointer/pointer/ints/ints, sorted by
        // first array in non-increasing order via sort template.
        sort::sort_down(
            &mut c.weights.0[..n],
            (&mut c.vars.0[..n], &mut eventdata.0[..n], &mut cliquepartition.0[..n], &mut negcliquepartition.0[..n]),
            sort::cmp_long,
        );

        // Sort all items with same weight according to their variable index,
        // used for hash value for fast pairwise comparison of all constraints.
        let mut v = c.nvars - 1;
        while v >= 0 {
            let mut w = v - 1;
            while w >= 0 && c.weights[v] == c.weights[w] {
                w -= 1;
            }
            if v - w > 1 {
                // Sort all corresponding parts of arrays for which the weights are
                // equal by using the variable index.
                let (a, b) = ((w + 1) as usize, (v + 1) as usize);
                sort::sort(
                    &mut c.vars.0[a..b],
                    (&mut eventdata.0[a..b], &mut cliquepartition.0[a..b], &mut negcliquepartition.0[a..b]),
                    |&x, &y| var_compare(scip, x, y),
                );
            }
            v = w;
        }

        // We need to make sure that our clique numbers of our normal clique will
        // be in increasing order without gaps.
        if c.cliquepartitioned {
            let mut lastcliquenum = 0;
            for pos in 0..n {
                // If the clique number in the normal clique at position pos is
                // greater than the last found clique number the partition is
                // invalid.
                if cliquepartition[pos] > lastcliquenum {
                    c.cliquepartitioned = false;
                    break;
                } else if cliquepartition[pos] == lastcliquenum {
                    lastcliquenum += 1;
                }
            }
        }
        // We need to make sure that our clique numbers of our negated clique
        // will be in increasing order without gaps.
        if c.negcliquepartitioned {
            let mut lastcliquenum = 0;
            for pos in 0..n {
                // If the clique number in the negated clique at position pos is
                // greater than the last found clique number the partition is
                // invalid.
                if negcliquepartition[pos] > lastcliquenum {
                    c.negcliquepartitioned = false;
                    break;
                } else if negcliquepartition[pos] == lastcliquenum {
                    lastcliquenum += 1;
                }
            }
        }

        c.sorted = true;
    }
}

/// `calcCliquepartition`: calculates a partition of the variables into
/// cliques.
fn calc_cliquepartition(scip: &mut Scip, conshdlr: ConshdlrId, c: &mut ConsData, normalclique: bool, negatedclique: bool) -> ScipResult {
    let (updatecliquepartitions, clqpartupdatefac) = (hd(scip, conshdlr).updatecliquepartitions, hd(scip, conshdlr).clqpartupdatefac);

    // Rerun eventually if number of global cliques increased considerably
    // since last partition.
    let ispartitionoutdated = updatecliquepartitions && c.ncliques > 1 && scip_var::get_n_cliques(scip) >= (clqpartupdatefac * c.ncliqueslastpart as f64) as i32;

    if normalclique && (!c.cliquepartitioned || ispartitionoutdated) {
        let n = c.nvars as usize;
        let mut map = std::mem::take(&mut hdm(scip, conshdlr).probtoidxmap);
        let mut mapsize = hd(scip, conshdlr).probtoidxmapsize;
        c.ncliques = scip_var::calc_clique_partition(scip, &c.vars.0[..n], &mut map, &mut mapsize, &mut c.cliquepartition.as_mut().unwrap().0[..n])?;
        let h = hdm(scip, conshdlr);
        h.probtoidxmap = map;
        h.probtoidxmapsize = mapsize;
        c.cliquepartitioned = true;
        c.ncliqueslastpart = scip_var::get_n_cliques(scip);
    }

    // Rerun eventually if number of global cliques increased considerably
    // since last negated partition.
    let isnegpartitionoutdated = updatecliquepartitions && c.nnegcliques > 1 && scip_var::get_n_cliques(scip) >= (clqpartupdatefac * c.ncliqueslastnegpart as f64) as i32;

    if negatedclique && (!c.negcliquepartitioned || isnegpartitionoutdated) {
        let n = c.nvars as usize;
        let mut map = std::mem::take(&mut hdm(scip, conshdlr).probtoidxmap);
        let mut mapsize = hd(scip, conshdlr).probtoidxmapsize;
        c.nnegcliques = scip_var::calc_negated_clique_partition(scip, &c.vars.0[..n], &mut map, &mut mapsize, &mut c.negcliquepartition.as_mut().unwrap().0[..n])?;
        let h = hdm(scip, conshdlr);
        h.probtoidxmap = map;
        h.probtoidxmapsize = mapsize;
        c.negcliquepartitioned = true;
        c.ncliqueslastnegpart = scip_var::get_n_cliques(scip);
    }

    Ok(())
}

/// `lockRounding`: installs rounding locks for the given variable in the
/// given knapsack constraint.
fn lock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    api::lock_var_cons(scip, var, cons, false, true)
}

/// `unlockRounding`: removes rounding locks for the given variable in the
/// given knapsack constraint.
fn unlock_rounding(scip: &mut Scip, cons: ConsId, var: VarId) -> ScipResult {
    api::unlock_var_cons(scip, var, cons, false, true)
}

/// `catchEvents`: catches bound change events for variables in knapsack.
fn catch_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    for i in 0..cd(scip, cons).nvars {
        let (var, weight) = (cd(scip, cons).vars[i], cd(scip, cons).weights[i]);
        let id = eds(scip, eventhdlr).push(EventData { cons, weight, filterpos: -1 });
        cdm(scip, cons).eventdata.as_mut().unwrap()[i] = id;
        let mut filterpos = -1;
        api::catch_var_event(scip, var, EVENTTYPE_KNAPSACK, eventhdlr, id, Some(&mut filterpos))?;
        eds(scip, eventhdlr)[id].filterpos = filterpos;
    }
    Ok(())
}

/// `dropEvents`: drops bound change events for variables in knapsack.
fn drop_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        let id = cd(scip, cons).eventdata.as_ref().unwrap()[i];
        let filterpos = ed(scip, eventhdlr, id).filterpos;
        api::drop_var_event(scip, var, EVENTTYPE_KNAPSACK, eventhdlr, id, filterpos)?;
        eds(scip, eventhdlr).remove(id);
    }
    Ok(())
}

/// `consdataEnsureVarsSize`: ensures, that vars and vals arrays can store at
/// least num entries.
fn consdata_ensure_vars_size(scip: &Scip, c: &mut ConsData, num: i32, transformed: bool) {
    if num > c.varssize {
        let newsize = scip.set.calc_mem_grow_size(num) as usize;
        c.vars.0.resize(newsize, NONE);
        c.weights.0.resize(newsize, 0);
        if transformed {
            c.eventdata.as_mut().unwrap().0.resize(newsize, NONE);
            c.cliquepartition.as_mut().unwrap().0.resize(newsize, 0);
            c.negcliquepartition.as_mut().unwrap().0.resize(newsize, 0);
        }
        c.varssize = newsize as i32;
    }
}

/// `updateWeightSums`: updates all weight sums for fixed and unfixed
/// variables.
fn update_weight_sums(scip: &Scip, c: &mut ConsData, var: VarId, weightdelta: i64) {
    c.weightsum += weightdelta;

    if scip.vars[var].locdom.lb > 0.5 {
        c.onesweightsum += weightdelta;
    }
}

/// `consdataCreate`: creates knapsack constraint data.
fn consdata_create(scip: &mut Scip, vars: &[VarId], weights: &[i64], capacity: i64) -> ScipResult<ConsData> {
    let mut constant: i64 = 0;
    let mut c = ConsData { row: NONE, ..ConsData::default() };

    if !vars.is_empty() {
        let mut varsbuffer = Vec::with_capacity(vars.len());
        let mut weightsbuffer = Vec::with_capacity(vars.len());

        for v in 0..vars.len() {
            // All weight have to be non negative.
            if weights[v] > 0 {
                let x = &scip.vars[vars[v]];
                // Treat fixed variables as constants if problem compression is
                // enabled.
                if api::is_cons_compression_enabled(scip) && x.glbdom.lb > x.glbdom.ub - 0.5 {
                    // Only if the variable is fixed to 1, we add its weight to the
                    // constant.
                    if x.glbdom.ub > 0.5 {
                        constant += weights[v];
                    }
                } else {
                    varsbuffer.push(vars[v]);
                    weightsbuffer.push(weights[v]);
                }
            }
        }

        // Copy the active variables and weights into the constraint data
        // structure.
        c.nvars = varsbuffer.len() as i32;
        c.vars = CVec(varsbuffer);
        c.weights = CVec(weightsbuffer);
    }

    c.varssize = c.nvars;
    c.capacity = capacity - constant;
    c.ncliqueslastpart = -1;
    c.ncliqueslastnegpart = -1;

    // Get transformed variables, if we are in the transformed problem.
    if api::is_transformed(scip) {
        let n = c.nvars as usize;
        api::get_transformed_vars(scip, &mut c.vars.0[..n])?;

        for v in 0..n {
            let var = crate::var::var_get_probvar(scip, c.vars[v]);
            c.existmultaggr = c.existmultaggr || scip.vars[var].varstatus == VarStatus::MultAggr;
        }

        // Allocate memory for additional data structures.
        c.eventdata = Some(CVec(vec![NONE; n]));
        c.cliquepartition = Some(CVec(vec![0; n]));
        c.negcliquepartition = Some(CVec(vec![0; n]));
    }

    // Calculate sum of weights and capture variables.
    for v in 0..c.nvars as usize {
        // Calculate sum of weights.
        let (var, weight) = (c.vars[v], c.weights[v]);
        update_weight_sums(scip, &mut c, var, weight);

        // Capture variables.
        crate::var::var_capture(scip, var);
    }

    Ok(c)
}

/// `consdataFree`: frees knapsack constraint data; the data must be in the
/// constraint while its events are dropped.
fn consdata_free(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    let row = cd(scip, cons).row;
    if row != NONE {
        crate::lp::row_release(scip, row)?;
        cdm(scip, cons).row = NONE;
    }
    if cd(scip, cons).eventdata.is_some() {
        drop_events(scip, cons, eventhdlr)?;
        cdm(scip, cons).eventdata = None;
    }

    // Release variables.
    for v in 0..cd(scip, cons).nvars {
        let mut var = cd(scip, cons).vars[v];
        api::release_var(scip, &mut var)?;
        cdm(scip, cons).vars[v] = var;
    }

    Ok(())
}

/// `consdataChgWeight`: changes a single weight in knapsack constraint data.
fn consdata_chg_weight(scip: &mut Scip, cons: ConsId, item: i32, newweight: i64) {
    let oldweight = cd(scip, cons).weights[item];
    let weightdiff = newweight - oldweight;
    let var = cd(scip, cons).vars[item];
    with_cd(scip, cons, |scip, c| {
        c.weights[item] = newweight;

        // Update weight sums for all and fixed variables.
        update_weight_sums(scip, c, var, weightdiff);
    });

    if let Some(e) = cd(scip, cons).eventdata.as_ref() {
        let id = e[item];
        let eventhdlr = cons_eventhdlr(scip, cons);
        eds(scip, eventhdlr)[id].weight = newweight;
    }

    let c = cdm(scip, cons);
    c.presolvedtiming = 0;
    c.sorted = false;

    // Recalculate cliques extraction after a weight was increased.
    if oldweight < newweight {
        c.cliquesadded = false;
    }
}

/// `createRelaxation`: creates LP row corresponding to knapsack constraint.
fn create_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let k = &scip.conss[cons];
    let (name, local, modifiable, removable) = (k.name.clone(), k.is_local(), k.modifiable, k.is_removable());
    let capacity = cd(scip, cons).capacity;
    let inf = scip.set.infinity();

    let row = scip_lp::create_empty_row_cons(scip, cons, &name, -inf, capacity as f64, local, modifiable, removable)?;
    cdm(scip, cons).row = row;

    scip_lp::cache_row_extensions(scip, row);
    for i in 0..cd(scip, cons).nvars {
        let (var, weight) = (cd(scip, cons).vars[i], cd(scip, cons).weights[i]);
        api::add_var_to_row(scip, row, var, weight as f64)?;
    }
    scip_lp::flush_row_extensions(scip, row)?;

    Ok(())
}

/// `addRelaxation`: adds linear relaxation of knapsack constraint to the LP;
/// returns whether a cutoff has been detected.
fn add_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let mut cutoff = false;

    if cd(scip, cons).row == NONE {
        create_relaxation(scip, cons)?;
    }

    // Insert LP row as cut.
    let row = cd(scip, cons).row;
    if scip.rows[row].lppos < 0 {
        cutoff = scip_lp::add_row(scip, row, false)?;
    }

    Ok(cutoff)
}

/// `checkCons`: checks knapsack constraint for feasibility of given
/// solution (`NONE` for the current one); returns whether the constraint is
/// violated. Printing the reason is not ported.
fn check_cons(scip: &mut Scip, cons: ConsId, sol: SolId, checklprows: bool, _printreason: bool) -> ScipResult<bool> {
    let mut violated = false;

    let row = cd(scip, cons).row;
    if checklprows || row == NONE || scip.rows[row].lppos < 0 {
        let mut normsum = 0.0;
        let mut hugesum = 0.0;

        // Increase age of constraint; age is reset to zero, if a violation was
        // found only in case we are in enforcement.
        if sol == NONE {
            api::inc_cons_age(scip, cons)?;
        }

        // Sum separately over normal and huge weight contributions in order to
        // reduce numerical cancellation.
        let mut v = cd(scip, cons).nvars - 1;
        while v >= 0 {
            let (weight, var) = (cd(scip, cons).weights[v], cd(scip, cons).vars[v]);
            if scip.set.is_huge_value(weight as f64) {
                hugesum += weight as f64 * api::get_sol_val(scip, sol, var);
            } else {
                normsum += weight as f64 * api::get_sol_val(scip, sol, var);
            }
            v -= 1;
        }

        // Calculate constraint violation and update it in solution.
        normsum += hugesum;
        let capacity = cd(scip, cons).capacity as f64;
        let (absviol, relviol) = if normsum > capacity {
            (normsum - capacity, crate::misc::rel_diff(normsum, capacity))
        } else {
            (0.0, 0.0)
        };

        if sol != NONE {
            api::update_sol_lp_cons_violation(scip, sol, absviol, relviol);
        }

        if scip.set.is_feas_positive(absviol) {
            violated = true;

            // Only reset constraint age if we are in enforcement.
            if sol == NONE {
                api::reset_cons_age(scip, cons);
            }
        }
    }

    Ok(violated)
}

/// The items in and out of a knapsack solution, as C's `solitems` and
/// `nonsolitems` arrays.
#[derive(Clone, Debug, Default)]
pub struct KnapsackItems {
    pub solitems: Vec<i32>,
    pub nonsolitems: Vec<i32>,
}

/// `SCIPsolveKnapsackExactly`: solves knapsack problem in maximization form
/// exactly using dynamic programming; fills `sol` (cleared first) with the
/// selected and the not selected items if given, and adds the optimal value
/// to `solval` (set to zero first); returns `success` (false if the table
/// would get too large).
///
/// The algorithm will first compute a greedy solution and terminate if the
/// greedy solution is proven to be optimal. The dynamic programming
/// algorithm runs with a time and space complexity of O(nitems * capacity).
#[allow(clippy::too_many_arguments)]
pub fn solve_knapsack_exactly(
    scip: &Scip,
    weights: &[i64],
    profits: &[f64],
    mut capacity: i64,
    items: &[i32],
    mut sol: Option<&mut KnapsackItems>,
    mut solval: Option<&mut f64>,
) -> bool {
    let nitems = weights.len();
    let mut success = true;

    // Initializing solution value.
    if let Some(v) = solval.as_deref_mut() {
        *v = 0.0;
    }

    // Init solution information.
    if let Some(s) = sol.as_deref_mut() {
        s.solitems.clear();
        s.nonsolitems.clear();
    }

    // Allocate temporary memory.
    let mut myweights: Vec<i64> = Vec::with_capacity(nitems);
    let mut myprofits: Vec<f64> = Vec::with_capacity(nitems);
    let mut myitems: Vec<i32> = Vec::with_capacity(nitems);
    let mut weightsum: i64 = 0;
    let mut minweight = i64::MAX;
    let mut maxweight: i64 = 0;

    // Remove unnecessary items.
    for j in 0..nitems {
        // Item does not fit; item is not profitable.
        if weights[j] > capacity || profits[j] <= 0.0 {
            if let Some(s) = sol.as_deref_mut() {
                s.nonsolitems.push(items[j]);
            }
        }
        // Item always fits.
        else if weights[j] == 0 {
            if let Some(s) = sol.as_deref_mut() {
                s.solitems.push(items[j]);
            }

            if let Some(v) = solval.as_deref_mut() {
                *v += profits[j];
            }
        }
        // All important items.
        else {
            myweights.push(weights[j]);
            myprofits.push(profits[j]);
            myitems.push(items[j]);

            // Remember smallest item.
            if weights[j] < minweight {
                minweight = weights[j];
            }

            // Remember bigest item.
            if weights[j] > maxweight {
                maxweight = weights[j];
            }

            weightsum += weights[j];
        }
    }
    let nmyitems = myitems.len();

    // Check if all profits are integer to strengthen the upper bound on the
    // greedy solution.
    let mut intprofits = true;
    let mut j = 0;
    while j < nmyitems && intprofits {
        intprofits = intprofits && scip.set.is_integral(myprofits[j]);
        j += 1;
    }

    // If no item is left then goto end.
    if nmyitems == 0 {
        return success;
    }

    // If all items fit, we also do not need to do the expensive stuff later
    // on.
    if weightsum > 0 && weightsum <= capacity {
        for j in (0..nmyitems).rev() {
            if let Some(s) = sol.as_deref_mut() {
                s.solitems.push(myitems[j]);
            }
            if let Some(v) = solval.as_deref_mut() {
                *v += myprofits[j];
            }
        }

        return success;
    }

    // Make weights relatively prime.
    let mut eqweights = true;
    if maxweight > 1 {
        // Determine greatest common divisor.
        let mut gcd = myweights[nmyitems - 1];
        let mut j = nmyitems as isize - 2;
        while j >= 0 && gcd >= 2 {
            gcd = crate::misc::calc_gre_com_div(gcd, myweights[j as usize]);
            j -= 1;
        }

        // Divide by greatest common divisor.
        if gcd > 1 {
            for j in (0..nmyitems).rev() {
                myweights[j] /= gcd;
                eqweights = eqweights && myweights[j] == 1;
            }
            capacity /= gcd;
            minweight /= gcd;
        } else {
            eqweights = false;
        }
    }

    // If only one item fits, then take the best.
    if minweight > capacity / 2 {
        let mut p = nmyitems - 1;

        // Find best item.
        for j in (0..nmyitems - 1).rev() {
            if myprofits[j] > myprofits[p] {
                p = j;
            }
        }

        // Update solution information.
        if let Some(s) = sol.as_deref_mut() {
            s.solitems.push(myitems[p]);
            for j in (0..nmyitems).rev() {
                if j != p {
                    s.nonsolitems.push(myitems[j]);
                }
            }
        }

        // Update solution value.
        if let Some(v) = solval.as_deref_mut() {
            *v += myprofits[p];
        }

        return success;
    }

    // If all items have the same weight, then take the best.
    if eqweights {
        let mut addval = 0.0;

        sort::sort_down(&mut myprofits[..], (&mut myitems[..], &mut myweights[..]), sort::cmp_real);

        // Update solution information.
        if sol.is_some() || solval.is_some() {
            // If all items would fit we had handled this case before; take the
            // first best items into the solution.
            let mut i = capacity - 1;
            while i >= 0 {
                if let Some(s) = sol.as_deref_mut() {
                    s.solitems.push(myitems[i as usize]);
                }
                addval += myprofits[i as usize];
                i -= 1;
            }

            if let Some(s) = sol.as_deref_mut() {
                // The rest are not in the solution.
                let mut i = nmyitems as i64 - 1;
                while i >= capacity {
                    s.nonsolitems.push(myitems[i as usize]);
                    i -= 1;
                }
            }
        }

        // Update solution value.
        if let Some(v) = solval.as_deref_mut() {
            *v += addval;
        }

        return success;
    }

    // Sort myitems (plus corresponding arrays myweights and myprofits) such
    // that p_1/w_1 >= p_2/w_2 >= ... >= p_n/w_n, this is only used for the
    // greedy solution.
    let mut tempsort: Vec<f64> = (0..nmyitems).map(|j| myprofits[j] / myweights[j] as f64).collect();
    let mut realweights: Vec<f64> = (0..nmyitems).map(|j| myweights[j] as f64).collect();
    let greedymedianpos = sort::select_weighted_down(
        &mut tempsort,
        (&mut myweights[..], &mut myprofits[..], &mut myitems[..]),
        sort::cmp_real,
        Some(&mut realweights),
        capacity as f64,
    )
    .unwrap();

    // Initialize values for greedy solution information.
    let mut greedysolweight: i64 = 0;
    let mut greedysolvalue = 0.0;

    // Determine greedy solution.
    let mut j = 0;
    while j < greedymedianpos {
        // Update greedy solution weight and value.
        greedysolweight += myweights[j];
        greedysolvalue += myprofits[j];
        j += 1;
    }

    // If the greedy solution is optimal by comparing to the LP solution, we
    // take this solution. This happens if:
    // - the greedy solution reaches the capacity, because then the LP
    //   solution is integral;
    // - the greedy solution has an objective that is at least the LP value
    //   rounded down in case that all profits are integer, too.
    let mut greedyupperbound = greedysolvalue + myprofits[j] * (capacity - greedysolweight) as f64 / myweights[j] as f64;
    if intprofits {
        greedyupperbound = scip.set.floor(greedyupperbound);
    }
    if greedysolweight == capacity || scip.set.is_ge(greedysolvalue, greedyupperbound) {
        // Update solution information.
        if let Some(s) = sol.as_deref_mut() {
            // Collect items.
            for myitem in &myitems[..j] {
                s.solitems.push(*myitem);
            }
            for myitem in &myitems[j..nmyitems] {
                s.nonsolitems.push(*myitem);
            }
        }

        // Update solution value.
        if let Some(v) = solval.as_deref_mut() {
            *v += greedysolvalue;
        }

        return success;
    }

    // In the following table we do not need the first minweight columns.
    capacity -= minweight - 1;

    // We can only handle integers.
    if capacity >= i32::MAX as i64 {
        success = false;
        return success;
    }

    let intcap = capacity as i32;

    // This condition checks whether we will try to allocate a correct number
    // of bytes and do not have an overflow, while computing the size for the
    // allocation.
    let size = std::mem::size_of::<f64>();
    if intcap < 0 || (intcap > 0 && (nmyitems > usize::MAX / intcap as usize / size || nmyitems * intcap as usize * size > i32::MAX as usize)) {
        success = false;
        return success;
    }

    // Allocate temporary memory.
    let intcap = intcap as usize;
    let idx = |j: usize, d: usize| j * intcap + d;
    let mut optvalues = vec![0.0; nmyitems * intcap];

    // We memorize at each step the current minimal weight to later on know
    // which value in our optvalues matrix is valid; each value entries of the
    // j-th row of optvalues is valid if the index is >= allcurrminweight[j],
    // otherwise it is invalid; a second possibility would be to clear the
    // whole optvalues, which should be more expensive than storing 'nmyitem'
    // values.
    let mut allcurrminweight = vec![0i32; nmyitems];
    let mut currminweight = (myweights[0] - minweight) as i32;
    allcurrminweight[0] = currminweight;

    // Fills first row of dynamic programming table with optimal values.
    for optvalue in &mut optvalues[currminweight as usize..intcap] {
        *optvalue = myprofits[0];
    }

    // Fills dynamic programming table with optimal values.
    for j in 1..nmyitems {
        // Compute important part of weight, which will be represented in the
        // table.
        let intweight = (myweights[j] - minweight) as i32;

        // Copy all nonzeros from row above.
        let mut d = currminweight;
        while d < intweight && (d as usize) < intcap {
            optvalues[idx(j, d as usize)] = optvalues[idx(j - 1, d as usize)];
            d += 1;
        }

        // Update corresponding row.
        for d in intweight as usize..intcap {
            // If index d < current minweight then optvalues[IDX(j-1,d)] is not
            // initialized, i.e. should be 0.
            if (d as i32) < currminweight {
                optvalues[idx(j, d)] = myprofits[j];
            } else {
                let sumprofit = if (d as i64) - myweights[j] < currminweight as i64 {
                    myprofits[j]
                } else {
                    optvalues[idx(j - 1, (d as i64 - myweights[j]) as usize)] + myprofits[j]
                };

                optvalues[idx(j, d)] = crate::def::max(sumprofit, optvalues[idx(j - 1, d)]);
            }
        }

        // Update currminweight.
        if intweight < currminweight {
            currminweight = intweight;
        }

        allcurrminweight[j] = currminweight;
    }

    // Update optimal solution by following the table.
    if let Some(s) = sol {
        let mut d = intcap as i32 - 1;

        // Insert all items in (non-) solution vector.
        let mut j = nmyitems - 1;
        while j > 0 {
            // If the following condition holds this means all remaining items
            // does not fit anymore.
            if d < allcurrminweight[j] {
                // We cannot have exceeded our capacity.
                break;
            }

            // Collect solution items; the first condition means that no further
            // item can fit anymore, but this does.
            if d < allcurrminweight[j - 1] || optvalues[idx(j, d as usize)] > optvalues[idx(j - 1, d as usize)] {
                s.solitems.push(myitems[j]);

                // Check that we do not have an underflow.
                d = (d as i64 - myweights[j]) as i32;
            }
            // Collect non-solution items.
            else {
                s.nonsolitems.push(myitems[j]);
            }
            j -= 1;
        }

        // Insert remaining items.
        if d >= allcurrminweight[j] {
            s.solitems.push(myitems[j]);
        } else {
            let mut j = j as isize;
            while j >= 0 {
                s.nonsolitems.push(myitems[j as usize]);
                j -= 1;
            }
        }
    }

    // Update solution value.
    if let Some(v) = solval {
        *v += optvalues[idx(nmyitems - 1, intcap - 1)];
    }

    success
}

/// `SCIPsolveKnapsackApproximately`: solves knapsack problem in
/// maximization form approximately by solving the LP-relaxation of the
/// problem using Dantzig's method and rounding down the solution; fills
/// `sol` and adds to `solval` like [`solve_knapsack_exactly`]. The item
/// arrays are partially sorted in place.
pub fn solve_knapsack_approximately(weights: &mut [i64], profits: &mut [f64], capacity: i64, items: &mut [i32], mut sol: Option<&mut KnapsackItems>, mut solval: Option<&mut f64>) {
    let nitems = weights.len();

    if let Some(s) = sol.as_deref_mut() {
        s.solitems.clear();
        s.nonsolitems.clear();
    }

    if let Some(v) = solval.as_deref_mut() {
        *v = 0.0;
    }

    // Initialize data for median search.
    let mut tempsort = vec![0.0; nitems];
    let mut realweights = vec![0.0; nitems];
    for j in (0..nitems).rev() {
        tempsort[j] = profits[j] / weights[j] as f64;
        realweights[j] = weights[j] as f64;
    }

    // Partially sort indices such that all elements that are larger than the
    // break item appear first.
    sort::select_weighted_down(
        &mut tempsort,
        (&mut weights[..], &mut profits[..], &mut items[..]),
        sort::cmp_real,
        Some(&mut realweights),
        capacity as f64,
    );

    // Selects items as long as they fit into the knapsack.
    let mut solitemsweight: i64 = 0;
    let mut j = 0;
    while j < nitems && solitemsweight + weights[j] <= capacity {
        if let Some(s) = sol.as_deref_mut() {
            s.solitems.push(items[j]);
        }

        if let Some(v) = solval.as_deref_mut() {
            *v += profits[j];
        }

        solitemsweight += weights[j];
        j += 1;
    }

    if let Some(s) = sol {
        while j < nitems {
            s.nonsolitems.push(items[j]);
            j += 1;
        }
    }
}

/// The result of [`get_cover`].
#[derive(Clone, Debug, Default)]
struct Cover {
    /// Cover variables (positions in the knapsack).
    covervars: Vec<i32>,
    /// Noncover variables.
    noncovervars: Vec<i32>,
    /// Weight of cover.
    coverweight: i64,
    /// Whether a cover was found.
    found: bool,
    /// Number of variables with tightened upper bound.
    ntightened: i32,
    /// Whether the LP sol for knapsack vars is fractional.
    fractional: bool,
}

/// `getCover`: gets a cover C for the knapsack constraint (if one exists)
/// by solving the (modified) transformed separation problem
/// approximately.
#[allow(clippy::too_many_arguments)]
fn get_cover(scip: &mut Scip, vars: &[VarId], weights: &[i64], capacity: i64, solvals: &[f64], modtransused: bool) -> ScipResult<Cover> {
    let nvars = vars.len();
    let mut cover = Cover {
        fractional: true,
        ..Cover::default()
    };

    // Gets the following sets
    //  N_1 = {j in N : x*_j = 1} (fixedones),
    //  N_0 = {j in N : x*_j = 0} (fixedzeros) and
    //  N\(N_0 & N_1) (items),
    // where x*_j is the solution value of variable x_j.
    let mut fixedones: Vec<i32> = Vec::with_capacity(nvars);
    let mut fixedzeros: Vec<i32> = Vec::with_capacity(nvars);
    let mut items: Vec<i32> = Vec::with_capacity(nvars);
    let mut fixedonesweight: i64 = 0;
    let mut itemsweight: i64 = 0;
    for j in 0..nvars {
        // Tightens upper bound of x_j if weight of x_j is greater than capacity
        // of knapsack.
        if weights[j] > capacity {
            scip_var::tighten_var_ub(scip, vars[j], 0.0, false)?;
            cover.ntightened += 1;
            continue;
        }

        // Variable x_j has solution value one.
        if scip.set.is_feas_eq(solvals[j], 1.0) {
            fixedones.push(j as i32);
            fixedonesweight += weights[j];
        }
        // Variable x_j has solution value zero.
        else if scip.set.is_feas_eq(solvals[j], 0.0) {
            fixedzeros.push(j as i32);
        }
        // Variable x_j has fractional solution value.
        else {
            items.push(j as i32);
            itemsweight += weights[j];
        }
    }

    // Sets whether the LP solution x* for the knapsack variables is
    // fractional; if it is not fractional we stop the separation routine.
    if items.is_empty() {
        cover.fractional = false;
        return Ok(cover);
    }

    // Transforms the traditional separation problem (under consideration of
    // the following fixing: z_j = 1 for all j in N_1, z_j = 0 for all j in
    // N_0)
    //
    //  min sum_{j in N\(N_0 & N_1)} (1 - x*_j) z_j
    //     sum_{j in N\(N_0 & N_1)} a_j z_j >= (a_0 + 1) - sum_{j in N_1} a_j
    //                        z_j in {0,1}, j in N\(N_0 & N_1)
    //
    // to a knapsack problem in maximization form by complementing the
    // variables
    //
    // sum_{j in N\(N_0 & N_1)} (1 - x*_j) -
    //  max sum_{j in N\(N_0 & N_1)} (1 - x*_j) z_j
    //     sum_{j in N\(N_0 & N_1)} a_j z_j <= sum_{j in N\N_0} a_j - (a_0 + 1)
    //                        z_j in {0,1}, j in N\(N_0 & N_1)

    // Gets weight and profit of variables in transformed knapsack problem.
    let mut transweights: Vec<i64> = items.iter().map(|&j| weights[j as usize]).collect();
    let mut transprofits: Vec<f64> = items.iter().map(|&j| 1.0 - solvals[j as usize]).collect();

    // Gets capacity of transformed knapsack problem.
    let transcapacity = fixedonesweight + itemsweight - capacity - 1;

    // If capacity of transformed knapsack problem is less than zero, there is
    // no cover (when variables fixed to zero are not used).
    if transcapacity < 0 {
        return Ok(cover);
    }

    if modtransused {
        // Transforms the modified separation problem (under consideration of
        // the following fixing: z_j = 1 for all j in N_1, z_j = 0 for all j in
        // N_0)
        //
        //  min sum_{j in N\(N_0 & N_1)} (1 - x*_j) a_j z_j
        //     sum_{j in N\(N_0 & N_1)} a_j z_j >= (a_0 + 1) - sum_{j in N_1} a_j
        //                        z_j in {0,1}, j in N\(N_0 & N_1)
        //
        // to a knapsack problem in maximization form by complementing the
        // variables
        //
        // sum_{j in N\(N_0 & N_1)} (1 - x*_j) a_j -
        //  max sum_{j in N\(N_0 & N_1)} (1 - x*_j) a_j z_j
        //     sum_{j in N\(N_0 & N_1)} a_j z_j <= sum_{j in N\N_0} a_j - (a_0 + 1)
        //                        z_j in {0,1}, j in N\(N_0 & N_1)

        // Gets weight and profit of variables in modified transformed knapsack
        // problem.
        for j in 0..items.len() {
            transprofits[j] *= weights[items[j] as usize] as f64;
        }
    }

    // Solves (modified) transformed knapsack problem approximately by solving
    // the LP-relaxation of the (modified) transformed knapsack problem using
    // Dantzig's method and rounding down the solution. Let z* be the solution,
    // then
    //  j in C,       if z*_j = 0 and
    //  i in N\C,      if z*_j = 1.
    let mut sol = KnapsackItems::default();
    solve_knapsack_approximately(&mut transweights, &mut transprofits, transcapacity, &mut items, Some(&mut sol), None);
    cover.noncovervars = sol.solitems;
    cover.covervars = sol.nonsolitems;

    // Constructs cover C (sum_{j in C} a_j > a_0).
    for &j in &cover.covervars {
        cover.coverweight += weights[j as usize];
    }

    // Adds all variables from N_1 to C.
    for &j in &fixedones {
        cover.covervars.push(j);
        cover.coverweight += weights[j as usize];
    }

    // Adds all variables from N_0 to N\C.
    for &j in &fixedzeros {
        cover.noncovervars.push(j);
    }

    cover.found = true;

    Ok(cover)
}

/// `getPartitionCovervars`: gets partition (C_1,C_2) of minimal cover C,
/// i.e. C_1 & C_2 = C and C_1 cap C_2 = emptyset, with C_1 not empty;
/// chooses partition as follows C_2 = { j in C : x*_j = 1 } and C_1 = C \
/// C_2.
fn get_partition_covervars(scip: &Scip, solvals: &[f64], covervars: &[i32], vars_c1: &mut Vec<i32>, vars_c2: &mut Vec<i32>) {
    vars_c1.clear();
    vars_c2.clear();
    for &j in covervars {
        // Variable has solution value one.
        if scip.set.is_ge(solvals[j as usize], 1.0) {
            vars_c2.push(j);
        }
        // Variable has solution value less than one.
        else {
            vars_c1.push(j);
        }
    }
}

/// `changePartitionCovervars`: changes given partition (C_1,C_2) of minimal
/// cover C, if |C1| = 1, by moving one and two (if possible) variables from
/// C2 to C1 if |C1| = 1 and |C1| = 0, respectively.
fn change_partition_covervars(weights: &[i64], vars_c1: &mut Vec<i32>, vars_c2: &mut Vec<i32>) {
    // Sorts variables in C2 such that a_1 >= .... >= a_|C2|.
    let mut sortkeys_c2: Vec<f64> = vars_c2.iter().map(|&j| weights[j as usize] as f64).collect();
    sort::sort_down(&mut sortkeys_c2[..], (&mut vars_c2[..],), sort::cmp_real);

    // Adds one or two variable from C2 with smallest weight to C1 and removes
    // them from C2.
    while vars_c1.len() < 2 && !vars_c2.is_empty() {
        vars_c1.push(vars_c2.pop().unwrap());
    }
}

/// `changePartitionFeasiblesetvars`: changes given partition (C_1,C_2) of
/// feasible set C, if |C1| = 1, by moving one variable from C2 to C1.
fn change_partition_feasiblesetvars(weights: &[i64], vars_c1: &mut Vec<i32>, vars_c2: &mut Vec<i32>) {
    // Sorts variables in C2 such that a_1 >= .... >= a_|C2|.
    let mut sortkeys_c2: Vec<f64> = vars_c2.iter().map(|&j| weights[j as usize] as f64).collect();
    sort::sort_down(&mut sortkeys_c2[..], (&mut vars_c2[..],), sort::cmp_real);

    // Adds variable from C2 with smallest weight to C1 and removes it from C2.
    vars_c1.push(vars_c2.pop().unwrap());
}

/// `getPartitionNoncovervars`: gets partition (F,R) of N \ C where C is a
/// minimal cover, i.e. F & R = N \ C and F cap R = emptyset; chooses
/// partition as follows R = { j in N \ C : x*_j = 0 } and F = (N \ C) \ F.
fn get_partition_noncovervars(scip: &Scip, solvals: &[f64], noncovervars: &[i32], vars_f: &mut Vec<i32>, vars_r: &mut Vec<i32>) {
    vars_f.clear();
    vars_r.clear();
    for &j in noncovervars {
        // Variable has solution value zero.
        if scip.set.is_feas_eq(solvals[j as usize], 0.0) {
            vars_r.push(j);
        }
        // Variable has solution value greater than zero.
        else {
            vars_f.push(j);
        }
    }
}

/// `getLiftingSequence`: sorts variables in F, C_2, and R according to the
/// second level lifting sequence that will be used in the sequential lifting
/// procedure.
fn get_lifting_sequence(solvals: &[f64], weights: &[i64], vars_f: &mut [i32], vars_c2: &mut [i32], vars_r: &mut [i32]) {
    // Gets sorting key for variables in F corresponding to the following
    // lifting sequence
    //  sequence 1: non-increasing absolute difference between x*_j and the
    //              value the variable is fixed to, i.e. x*_1 >= x*_2 >= ... >=
    //              x*_|F|
    // in case of equality uses
    //  sequence 4: non-increasing a_j, i.e. a_1 >= a_2 >= ... >= a_|C_2|
    let mut sortkeypairs_f: Vec<SortKeyPair> = vars_f
        .iter()
        .map(|&j| SortKeyPair {
            key1: solvals[j as usize],
            key2: weights[j as usize] as f64,
        })
        .collect();

    // Gets sorting key for variables in C_2 corresponding to the following
    // lifting sequence
    //  sequence 4: non-increasing a_j, i.e. a_1 >= a_2 >= ... >= a_|C_2|
    let mut sortkeys_c2: Vec<f64> = vars_c2.iter().map(|&j| weights[j as usize] as f64).collect();

    // Gets sorting key for variables in R corresponding to the following
    // lifting sequence
    //  sequence 4: non-increasing a_j, i.e. a_1 >= a_2 >= ... >= a_|R|
    let mut sortkeys_r: Vec<f64> = vars_r.iter().map(|&j| weights[j as usize] as f64).collect();

    // Sorts F, C2 and R.
    if !vars_f.is_empty() {
        sort::sort_down(&mut sortkeypairs_f[..], (vars_f,), comp_sortkeypairs);
    }
    if !vars_c2.is_empty() {
        sort::sort_down(&mut sortkeys_c2[..], (vars_c2,), sort::cmp_real);
    }
    if !vars_r.is_empty() {
        sort::sort_down(&mut sortkeys_r[..], (vars_r,), sort::cmp_real);
    }
}

/// `enlargeMinweights`: enlarges minweight table to at least the given
/// length.
fn enlarge_minweights(scip: &Scip, minweights: &mut Vec<i64>, minweightslen: &mut usize, newlen: usize) {
    if newlen > minweights.len() {
        // Reallocate table memory.
        let newsize = scip.set.calc_mem_grow_size(newlen as i32) as usize;
        minweights.resize(newsize, 0);
    }

    // Initialize new elements.
    for minweight in &mut minweights[*minweightslen..newlen] {
        *minweight = i64::MAX;
    }
    *minweightslen = newlen;
}

/// The result of [`sequential_up_and_down_lifting`].
struct Lifted {
    /// Activity of lifted valid inequality.
    cutact: f64,
    /// Right hand side of the lifted valid inequality.
    liftrhs: i32,
}

/// The minweight table update after lifting a variable with coefficient
/// `liftcoef` and weight `weight` (C computes `minweights[w - liftcoef] +
/// weight` in wrapping arithmetic).
fn update_minweights(minweights: &mut [i64], top: usize, liftcoef: i32, weight: i64) {
    // Updates minweight table: minweight_i+1[w] =
    //  min{ minweights_i[w], a_{j_i}},                      if w <  alpha_j_i
    //  min{ minweights_i[w], minweights_i[w - alpha_j_i] + a_j_i},    if w >= alpha_j_i
    let mut w = top as i32;
    while w >= 0 {
        let wu = w as usize;
        if w < liftcoef {
            minweights[wu] = crate::def::min(minweights[wu], weight);
        } else {
            minweights[wu] = crate::def::min(minweights[wu], minweights[(w - liftcoef) as usize].wrapping_add(weight));
        }
        w -= 1;
    }
}

/// `sequentialUpAndDownLifting`: lifts given inequality
///   sum_{j in M_1} x_j <= alpha_0
///  valid for
///   S^0 = { x in {0,1}^|M_1| : sum_{j in M_1} a_j x_j <= a_0 - sum_{j in M_2} a_j }
///  to a valid inequality
///   sum_{j in M_1} x_j + sum_{j in F} alpha_j x_j + sum_{j in M_2} alpha_j x_j + sum_{j in R} alpha_j x_j
///   <= alpha_0 + sum_{j in M_2} alpha_j
///  for
///   S = { x in {0,1}^|N| : sum_{j in N} a_j x_j <= a_0 };
/// uses sequential up-lifting for the variables in F, sequential
/// down-lifting for the variable in M_2, and sequential up-lifting for the
/// variables in R; procedure can be used to strengthen minimal cover
/// inequalities and extended weight inequalities. Writes the lifting
/// coefficients to `liftcoefs` (zeroed first) and sorts `vars_m1`.
#[allow(clippy::too_many_arguments)]
fn sequential_up_and_down_lifting(
    scip: &Scip,
    weights: &[i64],
    capacity: i64,
    solvals: &[f64],
    vars_m1: &mut [i32],
    vars_m2: &[i32],
    vars_f: &[i32],
    vars_r: &[i32],
    alpha0: i32,
    liftcoefs: &mut [i32],
) -> Lifted {
    let nvars_m1 = vars_m1.len();

    // Allocates temporary memory.
    let mut minweights: Vec<i64> = vec![0; nvars_m1 + 1];

    // Initializes data structures.
    for c in liftcoefs.iter_mut() {
        *c = 0;
    }
    let mut cutact = 0.0;

    // Sets lifting coefficient of variables in M1, sorts variables in M1 such
    // that a_1 <= a_2 <= ... <= a_|M1| and calculates activity of the current
    // valid inequality.
    let mut sortkeys: Vec<f64> = vec![0.0; nvars_m1];
    for j in 0..nvars_m1 {
        liftcoefs[vars_m1[j] as usize] = 1;
        sortkeys[j] = weights[vars_m1[j] as usize] as f64;
        cutact += solvals[vars_m1[j] as usize];
    }
    sort::sort(&mut sortkeys[..], (&mut vars_m1[..],), sort::cmp_real);

    // Initializes (i = 1) the minweight table, defined as: minweights_i[w] =
    //  min  sum_{j in M_1} a_j x_j + sum_{k=1}^{i-1} a_{j_k}    x_{j_k}
    //  s.t.  sum_{j in M_1}    x_j + sum_{k=1}^{i-1} alpha_{j_k} x_{j_k} >= w
    //                        x_j in {0,1} for j in M_1 & {j_i,...,j_i-1},
    // for i = 1,...,t with t = |N\M1| and w = 0,...,|M1| +
    // sum_{k=1}^{i-1} alpha_{j_k}.
    minweights[0] = 0;
    for w in 1..=nvars_m1 {
        minweights[w] = minweights[w - 1] + weights[vars_m1[w - 1] as usize];
    }
    let mut minweightslen = nvars_m1 + 1;

    // Gets sum of weights of variables fixed to one, i.e. sum of weights of
    // variables in M_2.
    let mut fixedonesweight: i64 = 0;
    for &j in vars_m2 {
        fixedonesweight += weights[j as usize];
    }

    // Initializes right hand side of lifted valid inequality.
    let mut liftrhs = alpha0;

    // Sequentially up-lifts all variables in F.
    for &liftvar in vars_f {
        let weight = weights[liftvar as usize];

        let z = if capacity - fixedonesweight - weight < 0 {
            // Knapsack problem is infeasible: sets z = 0.
            0
        } else if minweights[liftrhs as usize] <= capacity - fixedonesweight - weight {
            // Knapsack problem is feasible: sets z = max { w : 0 <= w <= liftrhs,
            // minweights_i[w] <= a_0 - fixedonesweight - a_{j_i} } = liftrhs, if
            // minweights_i[liftrhs] <= a_0 - fixedonesweight - a_{j_i}.
            liftrhs
        } else {
            // Knapsack problem is feasible: uses binary search to find z = max {
            // w : 0 <= w <= liftrhs, minweights_i[w] <= a_0 - fixedonesweight -
            // a_{j_i} }.
            let mut left = 0;
            let mut right = liftrhs + 1;
            while left < right - 1 {
                let middle = (left + right) / 2;
                if minweights[middle as usize] <= capacity - fixedonesweight - weight {
                    left = middle;
                } else {
                    right = middle;
                }
            }

            // Now z = left.
            left
        };

        // Calculates lifting coefficients alpha_{j_i} = liftrhs - z.
        let liftcoef = liftrhs - z;
        liftcoefs[liftvar as usize] = liftcoef;

        // Minweight table and activity of current valid inequality will not
        // change, if alpha_{j_i} = 0.
        if liftcoef == 0 {
            continue;
        }

        // Updates activity of current valid inequality.
        cutact += liftcoef as f64 * solvals[liftvar as usize];

        // Enlarges current minweight table: from minweightlen = |M1| +
        // sum_{k=1}^{i-1} alpha_{j_k} + 1 entries to |M1| + sum_{k=1}^{i}
        // alpha_{j_k} + 1 entries and sets minweights_i[w] = infinity for w =
        // |M1| + sum_{k=1}^{i-1} alpha_{j_k} + 1 , ... , |M1| + sum_{k=1}^{i}
        // alpha_{j_k}.
        let newlen = minweightslen + liftcoef as usize;
        enlarge_minweights(scip, &mut minweights, &mut minweightslen, newlen);

        update_minweights(&mut minweights, minweightslen - 1, liftcoef, weight);
    }

    // Sequentially down-lifts all variables in M_2.
    for &liftvar in vars_m2 {
        let weight = weights[liftvar as usize];

        // Uses binary search to find z = max { w : 0 <= w <= |M_1| +
        // sum_{k=1}^{i-1} alpha_{j_k}, minweights_[w] <= a_0 - fixedonesweight +
        // a_{j_i}}.
        let mut left: i32 = 0;
        let mut right = minweightslen as i32;
        while left < right - 1 {
            let middle = (left + right) / 2;
            if minweights[middle as usize] <= capacity - fixedonesweight + weight {
                left = middle;
            } else {
                right = middle;
            }
        }

        // Now z = left.
        let z = left;

        // Calculates lifting coefficients alpha_{j_i} = z - liftrhs.
        let liftcoef = z - liftrhs;
        liftcoefs[liftvar as usize] = liftcoef;

        // Updates sum of weights of variables fixed to one.
        fixedonesweight -= weight;

        // Updates right-hand side of current valid inequality.
        liftrhs += liftcoef;

        // Minweight table and activity of current valid inequality will not
        // change, if alpha_{j_i} = 0.
        if liftcoef == 0 {
            continue;
        }

        // Updates activity of current valid inequality.
        cutact += liftcoef as f64 * solvals[liftvar as usize];

        // Enlarges current minweight table.
        let newlen = minweightslen + liftcoef as usize;
        enlarge_minweights(scip, &mut minweights, &mut minweightslen, newlen);

        update_minweights(&mut minweights, minweightslen - 1, liftcoef, weight);
    }

    // Sequentially up-lifts all variables in R.
    for &liftvar in vars_r {
        let weight = weights[liftvar as usize];

        let z = if minweights[liftrhs as usize] <= capacity - weight {
            // Sets z = max { w : 0 <= w <= liftrhs, minweights_i[w] <= a_0 -
            // a_{j_i} } = liftrhs, if minweights_i[liftrhs] <= a_0 - a_{j_i}.
            liftrhs
        } else {
            // Uses binary search to find z = max { w : 0 <= w <= liftrhs,
            // minweights_i[w] <= a_0 - a_{j_i} }.
            let mut left = 0;
            let mut right = liftrhs + 1;
            while left < right - 1 {
                let middle = (left + right) / 2;
                if minweights[middle as usize] <= capacity - weight {
                    left = middle;
                } else {
                    right = middle;
                }
            }

            // Now z = left.
            left
        };

        // Calculates lifting coefficients alpha_{j_i} = liftrhs - z.
        let liftcoef = liftrhs - z;
        liftcoefs[liftvar as usize] = liftcoef;

        // Minweight table and activity of current valid inequality will not
        // change, if alpha_{j_i} = 0.
        if liftcoef == 0 {
            continue;
        }

        // Updates activity of current valid inequality.
        cutact += liftcoef as f64 * solvals[liftvar as usize];

        update_minweights(&mut minweights, liftrhs as usize, liftcoef, weight);
    }

    Lifted { cutact, liftrhs }
}

/// Which origin the rows of a lifted cut get when a constraint is given:
/// the constraint (minimal cover cuts) or its handler (extended weight
/// cuts).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowOrigin {
    Cons,
    Conshdlr,
}

/// Creates the row of a lifted cut `sum liftcoefs_j x_j <= liftrhs` of the
/// knapsack variables in the given sets (the first with coefficient one,
/// the others with their positive lifting coefficients), and adds it if it
/// is efficacious (the end of the C separation routines; `tag` names the
/// cut type).
#[allow(clippy::too_many_arguments)]
fn add_lifted_cut(
    scip: &mut Scip,
    cons: ConsId,
    sepa: crate::types::SepaId,
    origin: RowOrigin,
    tag: &str,
    vars: &[VarId],
    first: &[i32],
    others: [&[i32]; 3],
    liftcoefs: &[i32],
    liftrhs: i32,
    sol: SolId,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    let inf = scip.set.infinity();

    // Creates LP row.
    let mut row = if cons != NONE {
        let conshdlr = scip.conss[cons].conshdlr;
        let k = &scip.conss[cons];
        let name = format!("{}_{}{}", k.name, tag, scip.conshdlrs[conshdlr].ncutsfound);
        let (local, removable) = (k.is_local(), k.is_removable());
        match origin {
            RowOrigin::Cons => scip_lp::create_empty_row_cons(scip, cons, &name, -inf, liftrhs as f64, local, false, removable)?,
            RowOrigin::Conshdlr => scip_lp::create_empty_row_conshdlr(scip, conshdlr, &name, -inf, liftrhs as f64, local, false, removable)?,
        }
    } else if sepa != NONE {
        let name = format!("{}_{}_{}", scip.sepas[sepa].name, tag, scip.sepas[sepa].ncutsfound);
        scip_lp::create_empty_row_sepa(scip, sepa, &name, -inf, liftrhs as f64, false, false, true)?
    } else {
        let name = format!("nn_{}_{}", tag, *ncuts);
        scip_lp::create_empty_row_unspec(scip, &name, -inf, liftrhs as f64, false, false, true)?
    };

    // Adds all variables in the knapsack constraint with calculated lifting
    // coefficient to the cut.
    scip_lp::cache_row_extensions(scip, row);
    for &j in first {
        api::add_var_to_row(scip, row, vars[j as usize], 1.0)?;
    }
    for set in others {
        for &j in set {
            if liftcoefs[j as usize] > 0 {
                api::add_var_to_row(scip, row, vars[j as usize], liftcoefs[j as usize] as f64)?;
            }
        }
    }
    scip_lp::flush_row_extensions(scip, row)?;

    // Checks if cut is violated enough.
    if scip_lp::is_cut_efficacious(scip, sol, row) {
        if cons != NONE {
            api::reset_cons_age(scip, cons);
        }
        *cutoff = scip_lp::add_row(scip, row, false)?;
        *ncuts += 1;
    }
    scip_lp::release_row(scip, &mut row)
}

/// `separateSequLiftedMinimalCoverInequality`: separates lifted minimal
/// cover inequalities using sequential up- and down-lifting for given
/// knapsack problem (without GUB information; GUB lifting is not ported).
#[allow(clippy::too_many_arguments)]
fn separate_sequ_lifted_minimal_cover_inequality(
    scip: &mut Scip,
    cons: ConsId,
    sepa: crate::types::SepaId,
    vars: &[VarId],
    weights: &[i64],
    capacity: i64,
    solvals: &[f64],
    mincovervars: &[i32],
    nonmincovervars: &[i32],
    sol: SolId,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    *cutoff = false;

    // Allocates temporary memory.
    let (mut vars_c1, mut vars_c2, mut vars_f, mut vars_r) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut liftcoefs = vec![0i32; vars.len()];

    // Gets partition (C_1,C_2) of C, i.e. C_1 & C_2 = C and C_1 cap C_2 =
    // emptyset, with C_1 not empty; chooses partition as follows
    //  C_2 = { j in C : x*_j = 1 } and
    //  C_1 = C\C_2
    get_partition_covervars(scip, solvals, mincovervars, &mut vars_c1, &mut vars_c2);

    // Changes partition (C_1,C_2) of minimal cover C, if |C1| = 1, by moving
    // one variable from C2 to C1.
    if vars_c1.len() < 2 && !vars_c2.is_empty() {
        change_partition_covervars(weights, &mut vars_c1, &mut vars_c2);
    }

    // Gets partition (F,R) of N\C, i.e. F & R = N\C and F cap R = emptyset;
    // chooses partition as follows
    //  R = { j in N\C : x*_j = 0 } and
    //  F = (N\C)\F
    get_partition_noncovervars(scip, solvals, nonmincovervars, &mut vars_f, &mut vars_r);

    // Lift cuts without GUB information: sorts variables in F, C_2, R
    // according to the second level lifting sequence that will be used in the
    // sequential lifting procedure.
    get_lifting_sequence(solvals, weights, &mut vars_f, &mut vars_c2, &mut vars_r);

    // Lifts minimal cover inequality sum_{j in C_1} x_j <= |C_1| - 1 valid for
    //
    //   S^0 = { x in {0,1}^|C_1| : sum_{j in C_1} a_j x_j <= a_0 - sum_{j in C_2} a_j }
    //
    // to a valid inequality sum_{j in C_1} x_j + sum_{j in N\C_1} alpha_j x_j
    // <= |C_1| - 1 + sum_{j in C_2} alpha_j for
    //
    //    S = { x in {0,1}^|N|  : sum_{j in N}  a_j x_j <= a_0 },
    //
    // uses sequential up-lifting for the variables in F, sequential
    // down-lifting for the variable in C_2 and sequential up-lifting for the
    // variables in R according to the second level lifting sequence.
    let alpha0 = vars_c1.len() as i32 - 1;
    let lifted = sequential_up_and_down_lifting(scip, weights, capacity, solvals, &mut vars_c1, &vars_c2, &vars_f, &vars_r, alpha0, &mut liftcoefs);

    // Checks if lifting yielded a violated cut.
    if api::is_efficacious(scip, (lifted.cutact - lifted.liftrhs as f64) / (crate::def::max(lifted.liftrhs, 1) as f64).sqrt()) {
        add_lifted_cut(
            scip,
            cons,
            sepa,
            RowOrigin::Cons,
            "mcseq",
            vars,
            &vars_c1,
            [&vars_c2, &vars_f, &vars_r],
            &liftcoefs,
            lifted.liftrhs,
            sol,
            cutoff,
            ncuts,
        )?;
    }

    Ok(())
}

/// `separateSequLiftedExtendedWeightInequality`: separates lifted extended
/// weight inequalities using sequential up- and down-lifting for given
/// knapsack problem.
#[allow(clippy::too_many_arguments)]
fn separate_sequ_lifted_extended_weight_inequality(
    scip: &mut Scip,
    cons: ConsId,
    sepa: crate::types::SepaId,
    vars: &[VarId],
    weights: &[i64],
    capacity: i64,
    solvals: &[f64],
    feassetvars: &[i32],
    nonfeassetvars: &[i32],
    sol: SolId,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    *cutoff = false;

    // Allocates temporary memory.
    let (mut vars_t1, mut vars_t2, mut vars_f, mut vars_r) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut liftcoefs = vec![0i32; vars.len()];

    // Gets partition (T_1,T_2) of T, i.e. T_1 & T_2 = T and T_1 cap T_2 =
    // emptyset, with T_1 not empty; chooses partition as follows
    //  T_2 = { j in T : x*_j = 1 } and
    //  T_1 = T\T_2
    get_partition_covervars(scip, solvals, feassetvars, &mut vars_t1, &mut vars_t2);

    // Changes partition (T_1,T_2) of feasible set T, if |T1| = 0, by moving
    // one variable from T2 to T1.
    if vars_t1.is_empty() && !vars_t2.is_empty() {
        change_partition_feasiblesetvars(weights, &mut vars_t1, &mut vars_t2);
    }

    // Gets partition (F,R) of N\T, i.e. F & R = N\T and F cap R = emptyset;
    // chooses partition as follows
    //  R = { j in N\T : x*_j = 0 } and
    //  F = (N\T)\F
    get_partition_noncovervars(scip, solvals, nonfeassetvars, &mut vars_f, &mut vars_r);

    // Sorts variables in F, T_2, and R according to the second level lifting
    // sequence that will be used in the sequential lifting procedure (the
    // variable removed last from the initial cover does not have to be lifted
    // first, therefore it is included in the sorting routine).
    get_lifting_sequence(solvals, weights, &mut vars_f, &mut vars_t2, &mut vars_r);

    // Lifts extended weight inequality sum_{j in T_1} x_j <= |T_1| valid for
    //
    //   S^0 = { x in {0,1}^|T_1| : sum_{j in T_1} a_j x_j <= a_0 - sum_{j in T_2} a_j }
    //
    // to a valid inequality sum_{j in T_1} x_j + sum_{j in N\T_1} alpha_j x_j
    // <= |T_1| + sum_{j in T_2} alpha_j for
    //
    //    S = { x in {0,1}^|N|  : sum_{j in N}  a_j x_j <= a_0 },
    //
    // uses sequential up-lifting for the variables in F, sequential
    // down-lifting for the variable in T_2 and sequential up-lifting for the
    // variabels in R according to the second level lifting sequence.
    let alpha0 = vars_t1.len() as i32;
    let lifted = sequential_up_and_down_lifting(scip, weights, capacity, solvals, &mut vars_t1, &vars_t2, &vars_f, &vars_r, alpha0, &mut liftcoefs);

    // Checks if lifting yielded a violated cut.
    if api::is_efficacious(scip, (lifted.cutact - lifted.liftrhs as f64) / (crate::def::max(lifted.liftrhs, 1) as f64).sqrt()) {
        add_lifted_cut(
            scip,
            cons,
            sepa,
            RowOrigin::Conshdlr,
            "ewseq",
            vars,
            &vars_t1,
            [&vars_t2, &vars_f, &vars_r],
            &liftcoefs,
            lifted.liftrhs,
            sol,
            cutoff,
            ncuts,
        )?;
    }

    Ok(())
}

/// `makeCoverMinimal`: converts given cover C to a minimal cover by
/// removing variables in the reverse order in which the variables were
/// chosen to be in C, i.e. in the order of non-increasing (1 - x*_j)/a_j,
/// if the transformed separation problem was used to find C and in the
/// order of non-increasing (1 - x*_j), if the modified transformed
/// separation problem was used to find C; note that all variables with x*_j
/// = 1 will be removed last from C.
fn make_cover_minimal(weights: &[i64], capacity: i64, solvals: &[f64], cover: &mut Cover, modtransused: bool) {
    let covervars = &mut cover.covervars;

    // Sorts C in the reverse order in which the variables were chosen to be in
    // the cover, i.e.
    //  such that (1 - x*_1)/a_1 >= ... >= (1 - x*_|C|)/a_|C|,  if trans
    //  separation problem was used to find C
    //  such that (1 - x*_1)    >= ... >= (1 - x*_|C|),      if modified trans
    //  separation problem was used to find C
    // note that all variables with x*_j = 1 are in the end of the sorted C, so
    // they will be removed last from C.
    let mut sortkeypairs: Vec<SortKeyPair> = if modtransused {
        covervars
            .iter()
            .map(|&j| SortKeyPair {
                key1: solvals[j as usize],
                key2: weights[j as usize] as f64,
            })
            .collect()
    } else {
        covervars
            .iter()
            .map(|&j| SortKeyPair {
                key1: (solvals[j as usize] - 1.0) / weights[j as usize] as f64,
                key2: (-weights[j as usize]) as f64,
            })
            .collect()
    };

    sort::sort(&mut sortkeypairs[..], (&mut covervars[..],), comp_sortkeypairs);

    // Gets j' with a_j' = min{ a_j : j in C }.
    let mut minweightidx = 0;
    let mut minweight = weights[covervars[minweightidx] as usize];
    for j in 1..covervars.len() {
        if weights[covervars[j] as usize] <= minweight {
            minweightidx = j;
            minweight = weights[covervars[minweightidx] as usize];
        }
    }

    let mut j = 0;
    // Removes variables from C until the remaining variables form a minimal
    // cover.
    while j < covervars.len() && cover.coverweight - minweight > capacity {
        // If sum_{i in C} a_i - a_j <= a_0, j cannot be removed from C.
        if cover.coverweight - weights[covervars[j] as usize] <= capacity {
            j += 1;
            continue;
        }

        // Adds j to N\C.
        cover.noncovervars.push(covervars[j]);

        // Removes j from C.
        cover.coverweight -= weights[covervars[j] as usize];
        covervars.remove(j);

        // Updates j' with a_j' = min{ a_j : j in C }.
        if j == minweightidx {
            minweightidx = 0;
            minweight = weights[covervars[minweightidx] as usize];
            for k in 1..covervars.len() {
                if weights[covervars[k] as usize] <= minweight {
                    minweightidx = k;
                    minweight = weights[covervars[minweightidx] as usize];
                }
            }
        } else {
            minweightidx -= 1;
        }

        // j needs to stay the same.
    }
}

/// `getFeasibleSet`: converts given initial cover C_init to a feasible set
/// by removing variables in the reverse order in which they were chosen to
/// be in C_init:
///  non-increasing (1 - x*_j)/a_j,  if transformed separation problem was
///  used to find C_init
///  non-increasing (1 - x*_j),     if modified transformed separation problem
///  was used to find C_init.
/// separates lifted extended weight inequalities using sequential up- and
/// down-lifting for this feasible set and all subsequent feasible sets.
#[allow(clippy::too_many_arguments)]
fn get_feasible_set(
    scip: &mut Scip,
    cons: ConsId,
    sepa: crate::types::SepaId,
    vars: &[VarId],
    weights: &[i64],
    capacity: i64,
    solvals: &[f64],
    cover: &mut Cover,
    modtransused: bool,
    sol: SolId,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    *cutoff = false;

    // Sorts C in the reverse order in which the variables were chosen to be in
    // the cover, i.e.
    //  such that (1 - x*_1)/a_1 >= ... >= (1 - x*_|C|)/a_|C|,  if trans
    //  separation problem was used to find C
    //  such that (1 - x*_1)    >= ... >= (1 - x*_|C|),      if modified trans
    //  separation problem was used to find C
    // note that all variables with x*_j = 1 are in the end of the sorted C, so
    // they will be removed last from C.
    let mut sortkeys: Vec<f64> = if modtransused {
        cover.covervars.iter().map(|&j| solvals[j as usize]).collect()
    } else {
        cover.covervars.iter().map(|&j| (solvals[j as usize] - 1.0) / weights[j as usize] as f64).collect()
    };
    sort::sort(&mut sortkeys[..], (&mut cover.covervars[..],), sort::cmp_real);

    // Removes variables from C_init and separates lifted extended weight
    // inequalities using sequential up- and down-lifting; in addition to an
    // extended weight inequality this gives cardinality inequalities.
    while cover.covervars.len() >= 2 {
        // Adds first element of C_init to N\C_init.
        cover.noncovervars.push(cover.covervars[0]);

        // Removes first element from C_init.
        cover.coverweight -= weights[cover.covervars[0] as usize];
        cover.covervars.remove(0);

        if cover.coverweight <= capacity {
            separate_sequ_lifted_extended_weight_inequality(
                scip,
                cons,
                sepa,
                vars,
                weights,
                capacity,
                solvals,
                &cover.covervars,
                &cover.noncovervars,
                sol,
                cutoff,
                ncuts,
            )?;
        }

        // Stop if cover is too large.
        if cover.covervars.len() as i32 >= MAXCOVERSIZEITERLEWI {
            break;
        }
    }

    Ok(())
}

/// `SCIPseparateKnapsackCuts`: separates different classes of valid
/// inequalities for the 0-1 knapsack problem. GUB information
/// (`constraints/knapsack/usegubs`, off by default) is not ported.
#[allow(clippy::too_many_arguments)]
pub fn separate_knapsack_cuts(
    scip: &mut Scip,
    cons: ConsId,
    sepa: crate::types::SepaId,
    vars: &[VarId],
    weights: &[i64],
    capacity: i64,
    sol: SolId,
    usegubs: bool,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    *cutoff = false;

    if vars.is_empty() {
        return Ok(());
    }

    // Increase age of constraint (age is reset to zero, if a cut was found).
    if cons != NONE {
        api::inc_cons_age(scip, cons)?;
    }

    // Gets solution values of all problem variables.
    let solvals: Vec<f64> = vars.iter().map(|&v| api::get_sol_val(scip, sol, v)).collect();

    // LMCI1 (lifted minimal cover inequalities using sequential up- and
    // down-lifting) using GUB information.
    if usegubs {
        unimplemented!("knapsack separation with GUB information");
    } else {
        // LMCI1 (lifted minimal cover inequalities using sequential up- and
        // down-lifting) (and LMCI2 (lifted minimal cover inequalities using
        // superadditive up-lifting))

        // Gets a most violated initial cover C_init ( sum_{j in C_init} a_j >
        // a_0 ) by using the MODIFIED transformed separation problem and taking
        // into account the following fixing:
        //  j in C_init,        if j in N_1 = {j in N : x*_j = 1} and
        //  j in N\C_init,      if j in N_0 = {j in N : x*_j = 0},
        // if one exists.
        let modtransused = true;
        let mut cover = get_cover(scip, vars, weights, capacity, &solvals, modtransused)?;

        // If x* is not fractional we stop the separation routine.
        if !cover.fractional {
            return Ok(());
        }

        // If no cover was found we stop the separation routine for lifted
        // minimal cover inequality.
        if cover.found {
            // Converts initial cover C_init to a minimal cover C by removing
            // variables in the reverse order in which the variables were chosen
            // to be in C_init; note that variables with x*_j = 1 will be removed
            // last.
            make_cover_minimal(weights, capacity, &solvals, &mut cover, modtransused);

            // Separates lifted minimal cover inequalities using sequential up-
            // and down-lifting.
            separate_sequ_lifted_minimal_cover_inequality(
                scip,
                cons,
                sepa,
                vars,
                weights,
                capacity,
                &solvals,
                &cover.covervars,
                &cover.noncovervars,
                sol,
                cutoff,
                ncuts,
            )?;

            // Separating lifted minimal cover inequalities using superadditive
            // up-lifting is off (USESUPADDLIFT).
            const _: () = assert!(!USESUPADDLIFT);
        }
    }

    // LEWI (lifted extended weight inequalities using sequential up- and
    // down-lifting).
    if !*cutoff {
        // Gets a most violated initial cover C_init ( sum_{j in C_init} a_j >
        // a_0 ) by using the transformed separation problem and taking into
        // account the following fixing:
        //  j in C_init,        if j in N_1 = {j in N : x*_j = 1} and
        //  j in N\C_init,      if j in N_0 = {j in N : x*_j = 0},
        // if one exists.
        let modtransused = false;
        let mut cover = get_cover(scip, vars, weights, capacity, &solvals, modtransused)?;

        // If no cover was found we stop the separation routine.
        if cover.found {
            // Converts initial cover C_init to a feasible set by removing
            // variables in the reverse order in which they were chosen to be in
            // C_init and separates lifted extended weight inequalities using
            // sequential up- and down-lifting for this feasible set and all
            // subsequent feasible sets.
            get_feasible_set(scip, cons, sepa, vars, weights, capacity, &solvals, &mut cover, modtransused, sol, cutoff, ncuts)?;
        }
    }

    Ok(())
}

/// The variable bounds `(vars, coefs, constants)` of a [`crate::var::VBounds`].
fn vbounds(vb: &Option<Box<crate::var::VBounds>>) -> (Vec<VarId>, Vec<f64>, Vec<f64>) {
    match vb {
        Some(b) => {
            let n = b.len as usize;
            (b.vars.0[..n].to_vec(), b.coefs.0[..n].to_vec(), b.constants.0[..n].to_vec())
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// `SCIPseparateRelaxedKnapsack`: relaxes given general linear constraint
/// into a knapsack constraint and separates lifted knapsack cover
/// inequalities.
#[allow(clippy::too_many_arguments)]
pub fn separate_relaxed_knapsack(
    scip: &mut Scip,
    cons: ConsId,
    sepa: crate::types::SepaId,
    knapvars: &[VarId],
    knapvals: &[f64],
    valscale: f64,
    mut rhs: f64,
    sol: SolId,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    let binvars: Vec<VarId> = api::get_vars(scip).to_vec();

    // All variables which are of integral type can be potentially of binary
    // type; this can be checked via the method SCIPvarIsBinary(var).
    let nbinvars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip)) as usize;

    *cutoff = false;

    if nbinvars == 0 {
        return Ok(());
    }

    // Set up data structures.
    let mut consvars: Vec<VarId> = Vec::with_capacity(nbinvars);
    let mut consvals: Vec<i64> = Vec::with_capacity(nbinvars);

    // Get conshdlrdata to use cleared memory.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    let noknapsackconshdlr;
    let usegubs;
    let mut binvals: CVec<f64>;
    let mut tmpindices: Vec<i32> = Vec::new();
    if conshdlr == NONE {
        noknapsackconshdlr = true;
        usegubs = DEFAULT_USEGUBS;
        binvals = CVec(vec![0.0; nbinvars]);
    } else {
        noknapsackconshdlr = false;
        let h = hdm(scip, conshdlr);
        usegubs = h.usegubs;

        // Increase array size to avoid an endless loop in the next block; this
        // might happen if continuous variables change their types to
        // SCIP_VARTYPE_BINARY during presolving.
        if h.reals1size == 0 {
            h.reals1 = CVec(vec![0.0; 1]);
            h.reals1size = 1;
        }

        // Next if condition should normally not be true, because it means that
        // presolving has created more binary variables than binary + integer
        // variables existed at the constraint initialization method, but for
        // example if you would transform all integers into their binary
        // representation then it maybe happens.
        if (h.reals1size as usize) < nbinvars {
            h.reals1size = nbinvars as i32;
            h.reals1.0.resize(nbinvars, 0.0);
        }

        binvals = std::mem::take(&mut h.reals1);
    }

    let mut tmp = 0;

    'terminate: {
        // Relax continuous knapsack constraint:
        // 1. make all variables binary:
        //   if x_j is continuous or integer variable substitute:
        //    - a_j < 0: x_j = lb  or  x_j = b*z + d with variable lower bound
        //      b*z + d with binary variable z
        //    - a_j > 0: x_j = ub  or  x_j = b*z + d with variable upper bound
        //      b*z + d with binary variable z
        // 2. convert coefficients of all variables to positive integers:
        //    - scale all coefficients a_j to a~_j integral
        //    - substitute  x~_j = 1 - x_j if a~_j < 0

        // Replace integer and continuous variables with binary variables.
        for i in 0..knapvars.len() {
            let var = knapvars[i];

            if var_is_binary(scip, var) && crate::var::var_is_active(scip, var) {
                let solval = api::get_sol_val(scip, sol, var);

                // Knapsack relaxation assumes solution values between 0.0 and 1.0
                // for binary variables.
                if scip.set.is_feas_lt(solval, 0.0) || scip.set.is_feas_gt(solval, 1.0) {
                    break 'terminate;
                }

                let probindex = scip.vars[var].probindex;
                binvals[probindex] += valscale * knapvals[i];
                if !noknapsackconshdlr {
                    tmpindices.push(probindex);
                    tmp += 1;
                }
            } else if valscale * knapvals[i] > 0.0 {
                // a_j > 0: substitution with lb or vlb.
                let (zvlb, bvlb, dvlb) = vbounds(&scip.vars[var].vlbs);

                // Search for lb or vlb with maximal bound value.
                let mut bestlbsol = scip.vars[var].glbdom.lb;
                let mut bestlbtype: i32 = -1;
                for j in 0..zvlb.len() {
                    // Use only numerical stable vlb with binary variable z.
                    if var_is_binary(scip, zvlb[j]) && crate::var::var_is_active(scip, zvlb[j]) && bvlb[j].abs() <= MAXABSVBCOEF {
                        let z = &scip.vars[zvlb[j]];
                        let ub = scip.vars[var].locdom.ub;
                        if (bvlb[j] >= 0.0 && scip.set.is_gt(bvlb[j] * z.locdom.lb + dvlb[j], ub)) || (bvlb[j] <= 0.0 && scip.set.is_gt(bvlb[j] * z.locdom.ub + dvlb[j], ub)) {
                            *cutoff = true;
                            break 'terminate;
                        }

                        let vlbsol = bvlb[j] * api::get_sol_val(scip, sol, zvlb[j]) + dvlb[j];
                        if scip.set.is_ge(vlbsol, bestlbsol) {
                            bestlbsol = vlbsol;
                            bestlbtype = j as i32;
                        }
                    }
                }

                // If no lb or vlb with binary variable was found, we have to abort.
                if scip.set.is_infinity(-bestlbsol) {
                    break 'terminate;
                }

                if bestlbtype == -1 {
                    rhs -= valscale * knapvals[i] * bestlbsol;
                } else {
                    let b = bestlbtype as usize;
                    rhs -= valscale * knapvals[i] * dvlb[b];
                    let probindex = scip.vars[zvlb[b]].probindex;
                    binvals[probindex] += valscale * knapvals[i] * bvlb[b];

                    if scip.set.is_infinity(binvals[probindex].abs()) {
                        break 'terminate;
                    }

                    if !noknapsackconshdlr {
                        tmpindices.push(probindex);
                        tmp += 1;
                    }
                }
            } else {
                // a_j < 0: substitution with ub or vub.
                let (zvub, bvub, dvub) = vbounds(&scip.vars[var].vubs);

                // Search for ub or vub with minimal bound value.
                let mut bestubsol = scip.vars[var].glbdom.ub;
                let mut bestubtype: i32 = -1;
                for j in 0..zvub.len() {
                    // Use only numerical stable vub with active binary variable z.
                    if var_is_binary(scip, zvub[j]) && crate::var::var_is_active(scip, zvub[j]) && bvub[j].abs() <= MAXABSVBCOEF {
                        let z = &scip.vars[zvub[j]];
                        let lb = scip.vars[var].locdom.lb;
                        if (bvub[j] >= 0.0 && scip.set.is_lt(bvub[j] * z.locdom.ub + dvub[j], lb)) || (bvub[j] <= 0.0 && scip.set.is_lt(bvub[j] * z.locdom.lb + dvub[j], lb)) {
                            *cutoff = true;
                            break 'terminate;
                        }

                        let vubsol = bvub[j] * api::get_sol_val(scip, sol, zvub[j]) + dvub[j];
                        if scip.set.is_le(vubsol, bestubsol) {
                            bestubsol = vubsol;
                            bestubtype = j as i32;
                        }
                    }
                }

                // If no ub or vub with binary variable was found, we have to abort.
                if scip.set.is_infinity(bestubsol) {
                    break 'terminate;
                }

                if bestubtype == -1 {
                    rhs -= valscale * knapvals[i] * bestubsol;
                } else {
                    let b = bestubtype as usize;
                    rhs -= valscale * knapvals[i] * dvub[b];
                    let probindex = scip.vars[zvub[b]].probindex;
                    binvals[probindex] += valscale * knapvals[i] * bvub[b];

                    if scip.set.is_infinity(binvals[probindex].abs()) {
                        break 'terminate;
                    }

                    if !noknapsackconshdlr {
                        tmpindices.push(probindex);
                        tmp += 1;
                    }
                }
            }
        }

        // Convert coefficients of all (now binary) variables to positive
        // integers:
        //  - make all coefficients integral
        //  - make all coefficients positive (substitute negated variable)

        // Calculate scalar which makes all coefficients integral in relative
        // allowed difference in between -SCIPepsilon(scip) and
        // KNAPSACKRELAX_MAXDELTA; if coefficients cannot be made integral, we
        // have to use a scalar of 1.0 and only round fractional coefficients
        // down.
        let intscalar = crate::misc::calc_integral_scalar(
            &binvals.0[..nbinvars],
            -scip.set.epsilon(),
            KNAPSACKRELAX_MAXDELTA,
            KNAPSACKRELAX_MAXDNOM,
            KNAPSACKRELAX_MAXSCALE,
        )
        .unwrap_or(1.0);

        // Make all coefficients integral and positive:
        //  - scale a~_j = a_j * intscalar
        //  - substitute x~_j = 1 - x_j if a~_j < 0
        rhs *= intscalar;

        let mut minact: i64 = 0;
        let mut maxact: i64 = 0;
        for i in 0..nbinvars {
            let mut val = scip.set.floor(binvals[i] * intscalar) as i64;
            if val == 0 {
                continue;
            }

            let var = if val > 0 {
                binvars[i]
            } else {
                let var = scip_var::get_negated_var(scip, binvars[i])?;
                val = -val;
                rhs += val as f64;
                var
            };

            if scip.vars[var].locdom.lb > 0.5 {
                minact += val;
            }
            if scip.vars[var].locdom.ub > 0.5 {
                maxact += val;
            }

            consvals.push(val);
            consvars.push(var);
        }

        if !consvars.is_empty() {
            let capacity = scip.set.feas_floor(rhs) as i64;

            if minact > capacity {
                *cutoff = true;
                break 'terminate;
            }

            if maxact > capacity {
                // Separate lifted cut from relaxed knapsack constraint.
                separate_knapsack_cuts(scip, cons, sepa, &consvars, &consvals, capacity, sol, usegubs, cutoff, ncuts)?;
            }
        }
    }

    // Free data structures.
    if !noknapsackconshdlr {
        // Clear binvals.
        let mut t = tmp - 1;
        while t >= 0 {
            binvals[tmpindices[t as usize]] = 0.0;
            t -= 1;
        }
        hdm(scip, conshdlr).reals1 = binvals;
    }

    Ok(())
}

/// `separateCons`: separates given knapsack constraint.
fn separate_cons(scip: &mut Scip, cons: ConsId, sol: SolId, sepacuts: bool, usegubs: bool, cutoff: &mut bool, ncuts: &mut i32) -> ScipResult {
    *cutoff = false;

    // Check knapsack constraint itself for feasibility.
    let violated = check_cons(scip, cons, sol, sol != NONE, false)?;

    if violated {
        // Add knapsack constraint as LP row to the LP.
        *cutoff = add_relaxation(scip, cons)?;
        *ncuts += 1;
    } else if sepacuts {
        let c = cd(scip, cons);
        let n = c.nvars as usize;
        let (vars, weights, capacity) = (c.vars.0[..n].to_vec(), c.weights.0[..n].to_vec(), c.capacity);
        separate_knapsack_cuts(scip, cons, NONE, &vars, &weights, capacity, sol, usegubs, cutoff, ncuts)?;
    }

    Ok(())
}

/// `addCoef`: adds coefficient to constraint data.
fn add_coef(scip: &mut Scip, cons: ConsId, mut var: VarId, weight: i64) -> ScipResult {
    // Add the new coefficient to the LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        api::add_var_to_row(scip, row, var, weight as f64)?;
    }

    // Check for fixed variable.
    if scip.vars[var].glbdom.lb > 0.5 {
        // Variable is fixed to one: reduce capacity.
        cdm(scip, cons).capacity -= weight;
    } else if scip.vars[var].glbdom.ub > 0.5 {
        // Get binary representative of variable.
        (var, _) = scip_var::get_binvar_representative(scip, var)?;

        // Insert coefficient.
        let transformed = api::cons_is_transformed(scip, cons);
        with_cd(scip, cons, |scip, c| {
            consdata_ensure_vars_size(scip, c, c.nvars + 1, transformed);
            let n = c.nvars;
            c.vars[n] = var;
            c.weights[n] = weight;
            c.nvars += 1;
        });

        // Capture variable.
        crate::var::var_capture(scip, var);

        // Install the rounding locks of variable.
        lock_rounding(scip, cons, var)?;

        // Catch events.
        if transformed {
            let eventhdlr = cons_eventhdlr(scip, cons);
            let last = cd(scip, cons).nvars - 1;
            let id = eds(scip, eventhdlr).push(EventData { cons, weight, filterpos: -1 });
            cdm(scip, cons).eventdata.as_mut().unwrap()[last] = id;
            let mut filterpos = -1;
            api::catch_var_event(scip, var, EVENTTYPE_KNAPSACK, eventhdlr, id, Some(&mut filterpos))?;
            eds(scip, eventhdlr)[id].filterpos = filterpos;

            let probvar = crate::var::var_get_probvar(scip, var);
            if !cd(scip, cons).existmultaggr && scip.vars[probvar].varstatus == VarStatus::MultAggr {
                cdm(scip, cons).existmultaggr = true;
            }

            // Mark constraint to be propagated and presolved.
            cons_mark_propagate(scip, cons);
            let c = cdm(scip, cons);
            c.presolvedtiming = 0;
            // New coefficient might lead to larger cliques.
            c.cliquesadded = false;
        }

        // Update weight sums.
        with_cd(scip, cons, |scip, c| update_weight_sums(scip, c, var, weight));

        let c = cdm(scip, cons);
        c.sorted = false;
        c.cliquepartitioned = false;
        c.negcliquepartitioned = false;
        c.merged = false;
    }

    Ok(())
}

/// The clique partition repair of [`del_coef_pos`] for one partition: the
/// entry of the last item moves to `pos`; returns `(partitioned,
/// ncliques)`.
fn repair_clique_partition(partition: &mut [i32], nvars: i32, pos: i32, mut partitioned: bool, mut ncliques: i32) -> (bool, i32) {
    // If the clique number is equal to the number of variables we have only
    // cliques with one element, so we don't change the clique number.
    if partition[(nvars - 1) as usize] != nvars - 1 {
        let oldcliqenum = partition[pos as usize];
        partition[pos as usize] = partition[(nvars - 1) as usize];

        // The following if and else cases assure that we have increasing clique
        // numbers.
        if partition[pos as usize] > pos {
            // Recalculate the clique partition after a coefficient was removed.
            partitioned = false;
        } else {
            // If the old clique number was greater than the new one we have to
            // check that before a bigger clique number occurs the same as the
            // old one is still in the cliquepartition.
            if oldcliqenum > partition[pos as usize] {
                let mut i = 0;
                while i < nvars {
                    if oldcliqenum == partition[i as usize] {
                        break;
                    } else if oldcliqenum < partition[i as usize] {
                        // Recalculate the clique partition after a coefficient was
                        // removed.
                        partitioned = false;
                        break;
                    }
                    i += 1;
                }

                // If we reached the end in the for loop, it means we have deleted
                // the last element of the clique with the biggest index, so
                // decrease the number of cliques.
                if i == nvars {
                    ncliques -= 1;
                }
            }
            // If the old clique number was smaller than the new one we have to
            // check the front for an element with clique number minus 1.
            else if oldcliqenum < partition[pos as usize] {
                let cliquenumbefore = partition[pos as usize] - 1;
                let mut i = pos - 1;
                while i >= 0 && i >= cliquenumbefore && partition[i as usize] < cliquenumbefore {
                    i -= 1;
                }

                if i < cliquenumbefore {
                    // Recalculate the clique partition after a coefficient was
                    // removed.
                    partitioned = false;
                }
            }
            // If we deleted the last element of the clique with biggest index, we
            // have to decrease the clique number.
            else if pos == nvars - 1 {
                let cliquenumbefore = partition[pos as usize];
                let mut i = pos - 1;
                while i >= 0 && i >= cliquenumbefore && partition[i as usize] < cliquenumbefore {
                    i -= 1;
                }

                if i < cliquenumbefore {
                    ncliques -= 1;
                }
            }
            // If the old clique number is equal to the new one the
            // cliquepartition should be ok.
        }
    } else {
        ncliques -= 1;
    }
    (partitioned, ncliques)
}

/// `delCoefPos`: deletes coefficient at given position from constraint
/// data.
fn del_coef_pos(scip: &mut Scip, cons: ConsId, pos: i32) -> ScipResult {
    let mut var = cd(scip, cons).vars[pos];

    // Delete the coefficient from the LP row.
    let row = cd(scip, cons).row;
    if row != NONE {
        let w = cd(scip, cons).weights[pos];
        api::add_var_to_row(scip, row, var, -(w as f64))?;
    }

    // Remove the rounding locks of variable.
    unlock_rounding(scip, cons, var)?;

    // Drop events and mark constraint to be propagated and presolved.
    if api::cons_is_transformed(scip, cons) {
        let eventhdlr = cons_eventhdlr(scip, cons);
        let id = cd(scip, cons).eventdata.as_ref().unwrap()[pos];
        let filterpos = ed(scip, eventhdlr, id).filterpos;
        api::drop_var_event(scip, var, EVENTTYPE_KNAPSACK, eventhdlr, id, filterpos)?;
        eds(scip, eventhdlr).remove(id);
        cons_mark_propagate(scip, cons);
        let c = cdm(scip, cons);
        c.presolvedtiming = 0;
        c.sorted = c.sorted && pos == c.nvars - 1;
    }

    with_cd(scip, cons, |scip, c| {
        // Decrease weight sums.
        let w = c.weights[pos];
        update_weight_sums(scip, c, var, -w);

        // Move the last variable to the free slot.
        let last = c.nvars - 1;
        c.vars[pos] = c.vars[last];
        c.weights[pos] = c.weights[last];
        if let Some(e) = c.eventdata.as_mut() {
            e[pos] = e[last];
        }
    });

    // Release variable.
    api::release_var(scip, &mut var)?;

    let c = cdm(scip, cons);

    // Try to use old clique partitions.
    if c.cliquepartitioned {
        let (partitioned, ncliques) = repair_clique_partition(&mut c.cliquepartition.as_mut().unwrap().0, c.nvars, pos, c.cliquepartitioned, c.ncliques);
        c.cliquepartitioned = partitioned;
        c.ncliques = ncliques;
    }
    if c.negcliquepartitioned {
        let (partitioned, nnegcliques) = repair_clique_partition(&mut c.negcliquepartition.as_mut().unwrap().0, c.nvars, pos, c.negcliquepartitioned, c.nnegcliques);
        c.negcliquepartitioned = partitioned;
        c.nnegcliques = nnegcliques;
    }

    c.nvars -= 1;

    Ok(())
}

/// `removeZeroWeights`: removes all items with weight zero from knapsack
/// constraint.
fn remove_zero_weights(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let mut v = cd(scip, cons).nvars - 1;
    while v >= 0 {
        if cd(scip, cons).weights[v] == 0 {
            del_coef_pos(scip, cons, v)?;
        }
        v -= 1;
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

/// `mergeMultiples`: replaces multiple occurrences of a variable or its
/// negation by a single coefficient; returns whether the node can be cut
/// off.
fn merge_multiples(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    if cd(scip, cons).merged {
        return Ok(false);
    }

    if cd(scip, cons).nvars <= 1 {
        cdm(scip, cons).merged = true;
        return Ok(false);
    }

    // Sorting array after indices of variables, that's only for faster
    // merging.
    with_cd(scip, cons, |scip, c| {
        let n = c.nvars as usize;
        let eventdata = c.eventdata.as_mut().unwrap();
        let cliquepartition = c.cliquepartition.as_mut().unwrap();
        let negcliquepartition = c.negcliquepartition.as_mut().unwrap();
        sort::sort(
            &mut c.vars.0[..n],
            (&mut eventdata.0[..n], &mut c.weights.0[..n], &mut cliquepartition.0[..n], &mut negcliquepartition.0[..n]),
            |&x, &y| crate::var::var_compare_active_and_negated(scip, x, y),
        );

        // Knapsack-sorting (decreasing weights) now lost.
        c.sorted = false;
    });

    let negation = |scip: &Scip, var: VarId| -> (VarId, bool) {
        if scip.vars[var].varstatus == VarStatus::Negated {
            (scip.vars[var].negatedvar, true)
        } else {
            (var, false)
        }
    };

    let mut v = cd(scip, cons).nvars - 1;
    let mut prev = v - 1;
    // Loop backwards through the items: deletion only affects rear items.
    while prev >= 0 {
        let (var1, negated1) = negation(scip, cd(scip, cons).vars[v]);
        let (var2, negated2) = negation(scip, cd(scip, cons).vars[prev]);

        if var1 == var2 {
            let (wv, wprev) = (cd(scip, cons).weights[v], cd(scip, cons).weights[prev]);

            // Both variables are either active or negated.
            if negated1 == negated2 {
                // Variables var1 and var2 are equal: add weight of var1 to var2,
                // and delete var1.
                consdata_chg_weight(scip, cons, prev, wv + wprev);
                del_coef_pos(scip, cons, v)?;
            }
            // Variables var1 and var2 are opposite: subtract smaller weight from
            // larger weight, reduce capacity, and delete item of smaller weight.
            else if wv == wprev {
                // Both variables eliminate themselves: w*x + w*(1-x) == w.
                cdm(scip, cons).capacity -= wv;
                // This does not affect var2, because var2 stands before var1.
                del_coef_pos(scip, cons, v)?;
                del_coef_pos(scip, cons, prev)?;

                prev -= 1;
            } else if wv < wprev {
                cdm(scip, cons).capacity -= wv;
                consdata_chg_weight(scip, cons, prev, wprev - wv);
                // This does not affect var2, because var2 stands before var1.
                del_coef_pos(scip, cons, v)?;
            } else {
                cdm(scip, cons).capacity -= wprev;
                consdata_chg_weight(scip, cons, v, wv - wprev);
                // Attention: normally we lose our order.
                del_coef_pos(scip, cons, prev)?;

                // Restore order iff necessary; otherwise the order still stands.
                if cd(scip, cons).nvars != v {
                    // Either that was the last pair or both, the negated and
                    // "normal" variable in front doesn't match var1, so the order is
                    // irrelevant.
                    let front = if prev == 0 { NONE } else { cd(scip, cons).vars[prev - 1] };
                    if prev == 0 || (var1 != front && var1 != scip.vars[front].negatedvar) {
                        prev -= 1;
                    }
                    // We need to let v at the same position.
                    else {
                        // Reduced capacity might lead to larger cliques.
                        cdm(scip, cons).cliquesadded = false;
                        // Don't decrease v, the same variable may exist up front.
                        prev -= 1;
                        continue;
                    }
                }
            }
            // Reduced capacity might lead to larger cliques.
            cdm(scip, cons).cliquesadded = false;
        }

        v = prev;
        prev -= 1;
    }

    cdm(scip, cons).merged = true;

    // Check infeasibility.
    let c = cd(scip, cons);
    Ok(c.onesweightsum > c.capacity)
}

/// `dualPresolving`: in case the knapsack constraint is independent of
/// every else, solve the knapsack problem (exactly) and apply the fixings
/// (dual reductions); returns whether the constraint is deleted.
fn dual_presolving(scip: &mut Scip, cons: ConsId, nfixedvars: &mut i32, ndelconss: &mut i32) -> ScipResult<bool> {
    use crate::var::{var_get_n_locks_down_type as locksdown, var_get_n_locks_up_type as locksup};

    // Constraints for which the check flag is set to FALSE, did not contribute
    // to the lock numbers; therefore, we cannot use the locks to decide for a
    // dual reduction using this constraint; for example after a restart the
    // cuts which are added to the problems have the check flag set to FALSE.
    if !scip.conss[cons].is_checked() {
        return Ok(false);
    }

    let c = cd(scip, cons);
    let nvars = c.nvars as usize;
    let vars = c.vars.0[..nvars].to_vec();
    let weights = c.weights.0[..nvars].to_vec();
    let capacity = c.capacity;

    let mut profits = vec![0.0; nvars];
    let mut items = vec![0i32; nvars];

    let mut applicable = true;

    // Check if we can apply the dual reduction; this can be done if the
    // knapsack has the only locks on this constraint; collect object values
    // which are the profits of the knapsack problem.
    for v in 0..nvars {
        let mut var = vars[v];

        // The variable should not be (globally) fixed.
        if locksdown(scip, var, crate::types::LockType::Model) > 0 || locksup(scip, var, crate::types::LockType::Model) > 1 {
            applicable = false;
            break;
        }

        let mut negated = false;

        // Get the active variable.
        crate::var::var_get_probvar_binary(scip, &mut var, &mut negated)?;

        profits[v] = if negated { scip.vars[var].obj } else { -scip.vars[var].obj };

        items[v] = v as i32;
    }

    if applicable {
        // Solve knapsack problem exactly.
        let mut sol = KnapsackItems::default();
        let mut solval = 0.0;
        let success = solve_knapsack_exactly(scip, &weights, &profits, capacity, &items, Some(&mut sol), Some(&mut solval));

        if success {
            // Apply solution of the knapsack as dual reductions.
            for &j in &sol.solitems {
                let var = vars[j as usize];
                scip_var::tighten_var_lb(scip, var, 1.0, true)?;
                *nfixedvars += 1;
            }
            for &j in &sol.nonsolitems {
                let var = vars[j as usize];
                scip_var::tighten_var_ub(scip, var, 0.0, true)?;
                *nfixedvars += 1;
            }

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
            return Ok(true);
        }
    }

    Ok(false)
}

/// `checkParallelObjective`: check if the knapsack constraint is parallel
/// to objective function; if so update the cutoff bound and avoid that the
/// constraint enters the LP by setting the initial and separated flag to
/// FALSE.
fn check_parallel_objective(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let conshdlr = scip.conss[cons].conshdlr;
    let nvars = cd(scip, cons).nvars;
    let nobjvars = api::get_n_obj_vars(scip);

    // Check if the knapsack constraints has the same number of variables as
    // the objective function and if the initial and/or separated flag is set
    // to FALSE.
    if nvars != nobjvars || (!scip.conss[cons].is_initial() && !scip.conss[cons].is_separated()) {
        return Ok(());
    }

    // There are no variables in the ojective function and in the constraint.
    // Thus, the constraint is redundant. Since we have a pure feasibility
    // problem, we do not want to set a cutoff or lower bound.
    if nobjvars == 0 {
        return Ok(());
    }

    let mut applicable = true;
    let mut offset = 0.0;
    let mut scale = 1.0;

    let mut v = 0;
    while v < nvars && applicable {
        let mut negated = false;
        let mut var = cd(scip, cons).vars[v];

        if crate::var::var_is_negated(scip, var) {
            negated = true;
            var = scip.vars[var].negatedvar;
        }

        let objval = scip.vars[var].obj;

        // If a variable has a zero objective coefficient the knapsack
        // constraint is not parallel to objective function.
        if scip.set.is_zero(objval) {
            applicable = false;
        } else {
            let weight = cd(scip, cons).weights[v] as f64;

            if negated {
                if v == 0 {
                    // The first variable defines the scale.
                    scale = weight / -objval;

                    offset += weight;
                } else if scip.set.is_eq(-objval * scale, weight) {
                    offset += weight;
                } else {
                    applicable = false;
                }
            } else if v == 0 {
                // The first variable define the scale.
                scale = weight / objval;
            } else if !scip.set.is_eq(objval * scale, weight) {
                applicable = false;
            }
        }
        v += 1;
    }

    if applicable {
        let capacity = cd(scip, cons).capacity as f64;
        if scip.set.is_positive(scale) && hd(scip, conshdlr).detectcutoffbound {
            // Avoid that the knapsack constraint enters the LP since it is
            // parallel to the objective function.
            scip_cons::set_cons_initial(scip, cons, false);
            scip_cons::set_cons_separated(scip, cons, false);

            let mut cutoffbound = (capacity - offset) / scale;

            // Increase the cutoff bound value by an epsilon to ensue that
            // solution with the value of the cutoff bound are still excepted.
            cutoffbound += scip.set.cutoffbounddelta();

            if cutoffbound < api::get_cutoffbound(scip) {
                api::update_cutoffbound(scip, cutoffbound)?;
            } else {
                // In case the cutoff bound is worse then currently known one we
                // avoid additionaly enforcement and propagation.
                scip_cons::set_cons_enforced(scip, cons, false);
                scip_cons::set_cons_propagated(scip, cons, false);
            }
        } else if scip.set.is_negative(scale) && hd(scip, conshdlr).detectlowerbound {
            // Avoid that the knapsack constraint enters the LP since it is
            // parallel to the objective function.
            scip_cons::set_cons_initial(scip, cons, false);
            scip_cons::set_cons_separated(scip, cons, false);

            let lowerbound = (capacity - offset) / scale;

            api::update_local_lowerbound(scip, lowerbound)?;
        }
    }

    Ok(())
}

/// `stableSort`: sort the variables and weights w.r.t. the clique
/// partition; thereby ensure the current order of the variables when a
/// weight of one variable is greater or equal another weight and both
/// variables are in the same cliques; fills `vars`, `weights` and the
/// starting position of each clique (and the end, at index `ncliques`).
fn stable_sort(c: &ConsData, vars: &mut [VarId], weights: &mut [i64], cliquestartposs: &mut [i32], usenegatedclique: bool) {
    let norigvars = c.nvars as usize;

    if norigvars == 0 {
        return;
    }

    let (cliquepartition, ncliques) = if usenegatedclique {
        (c.negcliquepartition.as_ref().unwrap(), c.nnegcliques as usize)
    } else {
        (c.cliquepartition.as_ref().unwrap(), c.ncliques as usize)
    };

    // We first count all clique items and alloc temporary memory for a bucket
    // sort.
    let mut cliquecount = vec![0usize; ncliques];

    // First we count for each clique the number of elements.
    for v in (0..norigvars).rev() {
        cliquecount[cliquepartition[v] as usize] += 1;
    }

    let mut varpointers = vec![0usize; ncliques];
    let mut nextpos = 0;

    // Now we initialize all start pointers for each clique, so they will be
    // ordered: to reach the goal that all variables of each clique will be
    // standing next to each other we will initialize the starting pointers for
    // each clique by adding the number of each clique to the last clique
    // starting pointer, e.g. clique1 has 4 elements and clique2 has 3 elements
    // the the starting pointer for clique1 will be the pointer to vars[0], the
    // starting pointer to clique2 will be the pointer to vars[4] and to
    // clique3 it will be vars[7].
    for c in 0..ncliques {
        varpointers[c] = nextpos;
        cliquestartposs[c] = nextpos as i32;
        nextpos += cliquecount[c];
    }
    cliquestartposs[ncliques] = nextpos as i32;

    // Now we copy all variable and weights to the right order.
    for v in 0..norigvars {
        let p = cliquepartition[v] as usize;
        vars[varpointers[p]] = c.vars[v];
        weights[varpointers[p]] = c.weights[v];
        varpointers[p] += 1;
    }
}

/// `applyFixings`: deletes all fixed variables from knapsack constraint, and
/// replaces variables with binary representatives. With `want_cutoff` (C's
/// `cutoff != NULL`) stops at the first infeasibility and returns it;
/// otherwise applies all fixings.
fn apply_fixings(scip: &mut Scip, cons: ConsId, want_cutoff: bool) -> ScipResult<bool> {
    let mut cutoff = false;

    // Check infeasibility.
    if cd(scip, cons).onesweightsum > cd(scip, cons).capacity {
        return Ok(want_cutoff);
    }

    // All multi-aggregations should be resolved.
    cdm(scip, cons).existmultaggr = false;

    let mut v = 0;
    while v < cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[v];

        if scip.vars[var].glbdom.lb > 0.5 {
            let w = cd(scip, cons).weights[v];
            cdm(scip, cons).capacity -= w;
            del_coef_pos(scip, cons, v)?;
            // Reduced capacity might lead to larger cliques.
            cdm(scip, cons).cliquesadded = false;
        } else if scip.vars[var].glbdom.ub < 0.5 {
            del_coef_pos(scip, cons, v)?;
        } else {
            let mut weight = cd(scip, cons).weights[v];

            // Get binary representative of variable.
            let (repvar, _) = scip_var::get_binvar_representative(scip, var)?;

            // Check for multi-aggregation.
            let (workvar, negated) = if crate::var::var_is_negated(scip, repvar) {
                (scip.vars[repvar].negatedvar, true)
            } else {
                (repvar, false)
            };

            // If repvar is not negated so workvar = repvar, otherwise workvar =
            // 1 - repvar. This means,
            // weight * workvar = weight * (a_1*y_1 + ... + a_n*y_n + c)
            //
            // The explanation for  the following block:
            // 1a) If repvar is a multi-aggregated variable weight * repvar should
            //     be replaced by weight * (a_1*y_1 + ... + a_n*y_n + c).
            // 1b) If repvar is a negated variable of a multi-aggregated variable
            //     weight * repvar should be replaced by weight - weight * (a_1*y_1
            //     + ... + a_n*y_n + c), for better further use here we switch the
            //     sign of weight so now we have the replacement -weight + weight *
            //     (a_1*y_1 + ... + a_n*y_n + c).
            // 2)  For all replacement variable we check:
            // 2a) weight * a_i < 0 than we add -weight * a_i * y_i_neg to the
            //     constraint and adjust the capacity through capacity -= weight *
            //     a_i caused by the negation of y_i.
            // 2b) weight * a_i >= 0 than we add weight * a_i * y_i to the
            //     constraint.
            // 3a) If repvar was not negated we need to subtract weight * c from
            //     capacity.
            // 3b) If repvar was negated we need to subtract weight * (c - 1) from
            //     capacity(note we switched the sign of weight in this case.
            if scip.vars[workvar].varstatus == VarStatus::MultAggr {
                crate::var::var_flatten_aggregation_graph(scip, workvar)?;
                let m = scip.vars[workvar].data.multaggr.clone();
                let aggrconst = m.constant;

                if !scip.set.is_integral(weight as f64 * aggrconst) {
                    // Try to resolve a multi-aggregation with a non-integral value
                    // for weight*aggrconst.
                    return Err(Retcode::Error);
                }

                // If workvar was negated, we have to flip the weight.
                if negated {
                    weight *= -1;
                }

                let mut i = m.nvars - 1;
                while i >= 0 {
                    let (aggrvar, aggrscalar) = (m.vars[i], m.scalars[i]);
                    if !var_is_binary(scip, aggrvar) {
                        // Try to resolve a multi-aggregation with a non-binary
                        // variable.
                        return Err(Retcode::Error);
                    }
                    if !scip.set.is_integral(weight as f64 * aggrscalar) {
                        // Try to resolve a multi-aggregation with a non-integral
                        // value for weight*aggrscalars.
                        return Err(Retcode::Error);
                    }

                    // If the new coefficient is smaller than zero, we need to add
                    // the negated variable instead and adjust the capacity.
                    if scip.set.is_negative(weight as f64 * aggrscalar) {
                        let negvar = scip_var::get_negated_var(scip, aggrvar)?;
                        let w = scip.set.floor(-(weight as f64) * aggrscalar + 0.5) as i64;
                        add_coef(scip, cons, negvar, w)?;
                        let dec = scip.set.floor(weight as f64 * aggrscalar + 0.5) as i64;
                        cdm(scip, cons).capacity -= dec;
                    } else {
                        let w = scip.set.floor(weight as f64 * aggrscalar + 0.5) as i64;
                        add_coef(scip, cons, aggrvar, w)?;
                    }
                    i -= 1;
                }

                // Delete old coefficient.
                del_coef_pos(scip, cons, v)?;

                // Adjust the capacity with the aggregation constant and if necessary
                // the extra weight through the negation.
                let dec = if negated {
                    scip.set.floor(weight as f64 * (aggrconst - 1.0) + 0.5) as i64
                } else {
                    scip.set.floor(weight as f64 * aggrconst + 0.5) as i64
                };
                cdm(scip, cons).capacity -= dec;

                if cd(scip, cons).capacity < 0 && want_cutoff {
                    cutoff = true;
                    break;
                }
            }
            // Check, if the variable should be replaced with the representative.
            else if repvar != var {
                // Delete old (aggregated) variable.
                del_coef_pos(scip, cons, v)?;

                // Add representative instead.
                add_coef(scip, cons, repvar, weight)?;
            } else {
                v += 1;
            }
        }
    }

    // If aggregated variables have been replaced, multiple entries of the
    // same variable are possible and we have to clean up the constraint.
    if want_cutoff && !cutoff {
        cutoff = merge_multiples(scip, cons)?;
    }

    Ok(cutoff)
}

/// `propagateCons`: propagation method for knapsack constraints; returns
/// `(cutoff, redundant)`.
fn propagate_cons(scip: &mut Scip, cons: ConsId, nfixedvars: &mut i32, mut usenegatedclique: bool) -> ScipResult<(bool, bool)> {
    let mut cutoff = false;
    let mut redundant = false;
    let conshdlr = scip.conss[cons].conshdlr;

    // Increase age of constraint; age is reset to zero, if a conflict or a
    // propagation was found.
    if !crate::tree::tree_in_repropagation(scip) {
        api::inc_cons_age(scip, cons)?;
    }

    usenegatedclique = usenegatedclique && cd(scip, cons).merged;

    let mut minweightsum: i64 = 0;
    let nvars = cd(scip, cons).nvars;

    // Make sure, the items are sorted by non-increasing weight.
    with_cd(scip, cons, |scip, c| sort_items(scip, c));

    'block: {
        let mut localminweightsum: i64 = 0;

        // (1) compute the minimum weight of the knapsack constraint using
        //     negated clique information; a negated clique means, that at most
        //     one of the clique variables can be zero
        //     - minweightsum = sum_{negated cliques C} ( sum(wi : i \in C) -
        //       W_max(C) ), where W_max(C) is the maximal weight of C
        //
        //     if for i \in C (a negated clique) oneweightsum + minweightsum - wi
        //     + W_max(C) > capacity => xi = 1 since replacing i with the element
        //     of maximal weight leads to infeasibility
        if usenegatedclique && nvars > 0 {
            // Compute clique partitions.
            let mut data = scip.conss[cons].consdata.take().unwrap();
            let r = calc_cliquepartition(scip, conshdlr, data.downcast_mut::<ConsData>().unwrap(), false, true);
            scip.conss[cons].consdata = Some(data);
            r?;
            let nnegcliques = cd(scip, cons).nnegcliques;

            // If we have no real negated cliques we can stop here.
            if nnegcliques == nvars {
                // Run the standard algorithm that does not involve cliques.
                usenegatedclique = false;
                break 'block;
            }

            // Allocate temporary memory and initialize it.
            let n = nvars as usize;
            let nn = nnegcliques as usize;
            let mut myvars = vec![NONE; n];
            let mut myweights = vec![0i64; n];
            let mut cliquestartposs = vec![0i32; nn + 1];
            let mut cliqueendposs = vec![0i32; nn];
            let mut secondmaxweights = vec![0i64; nn];

            // Resort variables to avoid quadratic algorithm later on.
            stable_sort(cd(scip, cons), &mut myvars, &mut myweights, &mut cliquestartposs, true);

            // Save the end positions of the cliques because start positions are
            // moved in the following loop.
            for c in 0..nn {
                cliqueendposs[c] = cliquestartposs[c + 1] - 1;
            }

            let mut c: i32 = 0;
            let mut foundmax = false;
            let mut i: i32 = 0;

            while i < nvars {
                // Ignore variables of the negated clique which are fixed to one
                // since these are counted in consdata->onesweightsum.

                // If there are only one variable negated cliques left we can stop.
                if nnegcliques - c == nvars - i {
                    minweightsum += localminweightsum;
                    localminweightsum = 0;
                    break;
                }

                // For summing up the minimum active weights due to cliques we have
                // to omit the biggest weights of each clique, we can only skip this
                // clique if this variables is not fixed to zero, otherwise we have
                // to fix all other clique variables to one.
                if cliquestartposs[c as usize] == i {
                    c += 1;
                    minweightsum += localminweightsum;
                    localminweightsum = 0;
                    foundmax = true;

                    if scip.vars[myvars[i as usize]].locdom.lb > 0.5 {
                        foundmax = false;
                    }

                    if scip.vars[myvars[i as usize]].locdom.ub > 0.5 {
                        i += 1;
                        continue;
                    }
                }

                let x = myvars[i as usize];
                if scip.vars[x].locdom.lb < 0.5 {
                    if scip.vars[x].locdom.ub > 0.5 {
                        if !foundmax {
                            foundmax = true;

                            // Overwrite cliquestartpos to the position of the first
                            // unfixed variable in this clique.
                            cliquestartposs[(c - 1) as usize] = i;
                            i += 1;
                            continue;
                        }
                        // Memorize second max weight for each clique.
                        if secondmaxweights[(c - 1) as usize] == 0 {
                            secondmaxweights[(c - 1) as usize] = myweights[i as usize];
                        }
                        localminweightsum += myweights[i as usize];
                    }
                    // We found a fixed variable to zero so all other variables in
                    // this negated clique have to be fixed to one.
                    else {
                        // Fix all other variables of the negated clique to 1.
                        let mut v = cliquestartposs[(c - 1) as usize];
                        while v < cliquestartposs[c as usize] {
                            let y = myvars[v as usize];
                            if v != i && scip.vars[y].locdom.lb < 0.5 {
                                let index = scip.vars[x].index;
                                let (infeasible, _) = scip_var::infer_binvar_cons(scip, y, true, cons, index)?;

                                if infeasible {
                                    // Analyze the infeasibility if conflict analysis is
                                    // applicable.
                                    if api::is_conflict_analysis_applicable(scip) {
                                        // Initialize the conflict analysis.
                                        api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

                                        // Add the two variables which are fixed to zero
                                        // within a negated clique.
                                        api::add_conflict_binvar(scip, x)?;
                                        api::add_conflict_binvar(scip, y)?;

                                        // Start the conflict analysis.
                                        api::analyze_conflict_cons(scip, cons)?;
                                    }
                                    cutoff = true;
                                    break;
                                }
                                *nfixedvars += 1;
                                api::reset_cons_age(scip, cons);
                            }
                            v += 1;
                        }

                        // Reset local minweightsum for clique because all fixed to
                        // one variables are now counted in consdata->onesweightsum.
                        localminweightsum = 0;

                        // We can jump to the end of this clique.
                        i = cliqueendposs[(c - 1) as usize];

                        if cutoff {
                            break;
                        }
                    }
                }
                i += 1;
            }

            // Add last clique minweightsum.
            minweightsum += localminweightsum;

            // Check, if weights of fixed variables don't exceeds knapsack
            // capacity.
            if !cutoff && cd(scip, cons).capacity >= minweightsum + cd(scip, cons).onesweightsum {
                // Loop over cliques.
                for c in 0..nn {
                    let endvarposclique = cliqueendposs[c];
                    let startvarposclique = cliquestartposs[c];

                    let maxvar = myvars[startvarposclique as usize];

                    // No need to process this negated clique because all variables
                    // are already fixed (which we detect from a fixed maxvar).
                    if scip.vars[maxvar].locdom.ub - scip.vars[maxvar].locdom.lb < 0.5 {
                        continue;
                    }

                    let maxcliqueweight = myweights[startvarposclique as usize];
                    let mut maxvarfixed = false;

                    let (onesweightsum, capacity) = (cd(scip, cons).onesweightsum, cd(scip, cons).capacity);

                    // If the sum of all weights of fixed variables to one plus the
                    // minimalweightsum (minimal weight which is already used in this
                    // knapsack due to negated cliques) plus any weight minus the
                    // second largest weight in this clique exceeds the capacity the
                    // maximum weight variable can be fixed to zero.
                    if onesweightsum + minweightsum + (maxcliqueweight - secondmaxweights[c]) > capacity {
                        api::reset_cons_age(scip, cons);
                        scip_var::infer_binvar_cons(scip, maxvar, false, cons, cliquestartposs[c])?;
                        *nfixedvars += 1;
                        maxvarfixed = true;
                    }
                    // Stop when the remaining cliques are singletons such that all
                    // subsequent variables have a weight that fits into the knapsack,
                    // or when no further variable fixings are possible:
                    //
                    // the gain in any of the following negated cliques (the
                    // additional term if the maximum weight variable was set to 1, and
                    // the second largest was set to 0) does not suffice to infer
                    // additional variable fixings because
                    //
                    // - the cliques are sorted by decreasing maximum weight -> for all
                    //   c' >= c: maxweights[c'] <= maxcliqueweight
                    // - their second largest elements are at least as large as the
                    //   smallest weight of the knapsack
                    else if nnegcliques - c as i32 == nvars - startvarposclique
                        || onesweightsum + minweightsum + (maxcliqueweight - cd(scip, cons).weights[nvars - 1]) <= capacity
                    {
                        break;
                    }

                    // Loop over items with non-maximal weight (omitting the first
                    // position).
                    let mut i = endvarposclique;
                    while i > startvarposclique {
                        let y = myvars[i as usize];
                        // Only check variables of negated cliques for which no
                        // variable is locally fixed.
                        if scip.vars[y].locdom.lb < 0.5 {
                            let (onesweightsum, capacity) = (cd(scip, cons).onesweightsum, cd(scip, cons).capacity);

                            // We fix the members of this clique with non-maximal weight
                            // in two cases to 1:
                            //
                            // the maxvar was already fixed to 0 because it has a huge
                            // gain.
                            //
                            // if for i \in C (a negated clique) onesweightsum - wi +
                            // W_max(c) > capacity  => xi = 1 since replacing i with the
                            // element of maximal weight leads to infeasibility.
                            if maxvarfixed || onesweightsum + minweightsum - myweights[i as usize] + maxcliqueweight > capacity {
                                scip_var::infer_binvar_cons(scip, y, true, cons, -i)?;
                                *nfixedvars += 1;
                                api::reset_cons_age(scip, cons);

                                // Update minweightsum because now the variable is fixed
                                // to one and its weight is counted by
                                // consdata->onesweightsum.
                                minweightsum -= myweights[i as usize];
                            } else {
                                break;
                            }
                        }
                        i -= 1;
                    }
                }
            }
        }
    }

    // Check, if weights of fixed variables already exceed knapsack capacity.
    if cd(scip, cons).capacity < minweightsum + cd(scip, cons).onesweightsum {
        api::reset_cons_age(scip, cons);
        cutoff = true;

        // Analyze the cutoff in SOLVING stage and if conflict analysis is
        // turned on.
        if (scip.set.stage == Stage::Solving || api::in_probing(scip)) && api::is_conflict_analysis_applicable(scip) {
            // Start conflict analysis with the fixed-to-one variables, add only
            // as many as needed to exceed the capacity.
            let mut weight: i64 = 0;

            api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

            let mut i = 0;
            while i < nvars && weight <= cd(scip, cons).capacity {
                let var = cd(scip, cons).vars[i];
                if scip.vars[var].locdom.lb > 0.5 {
                    api::add_conflict_binvar(scip, var)?;
                    weight += cd(scip, cons).weights[i];
                }
                i += 1;
            }

            api::analyze_conflict_cons(scip, cons)?;
        }

        return Ok((cutoff, redundant));
    }

    // The algorithm below is a special case of propagation involving negated
    // cliques.
    if !usenegatedclique {
        let residualcapacity = cd(scip, cons).capacity - cd(scip, cons).onesweightsum;

        // Fix all variables to zero, that don't fit into the knapsack anymore.
        let mut i = 0;
        while i < nvars && cd(scip, cons).weights[i] > residualcapacity {
            let var = cd(scip, cons).vars[i];
            // If all weights of fixed variables to one plus any weight exceeds the
            // capacity the variables have to be fixed to zero.
            if scip.vars[var].locdom.lb < 0.5 && scip.vars[var].locdom.ub > 0.5 {
                api::reset_cons_age(scip, cons);
                scip_var::infer_binvar_cons(scip, var, false, cons, i)?;
                *nfixedvars += 1;
            }
            i += 1;
        }
    }

    // Check if the knapsack is now redundant.
    if !scip.conss[cons].modifiable {
        let mut unfixedweightsum = cd(scip, cons).onesweightsum;

        // Sum up the weights of all unfixed variables, plus the weight sum of all
        // variables fixed to one already.
        for i in 0..nvars {
            let var = cd(scip, cons).vars[i];
            if scip.vars[var].locdom.lb + 0.5 < scip.vars[var].locdom.ub {
                unfixedweightsum += cd(scip, cons).weights[i];

                // The weight sum is larger than the capacity, so the constraint is
                // not redundant.
                if unfixedweightsum > cd(scip, cons).capacity {
                    return Ok((cutoff, redundant));
                }
            }
        }

        // We summed up all (unfixed and fixed to one) weights and did not exceed
        // the capacity, so the constraint is redundant.
        scip_cons::del_cons_local(scip, cons)?;
        redundant = true;
    }

    Ok((cutoff, redundant))
}

/// `upgradeCons`: all but one variable fit into the knapsack constraint, so
/// we can upgrade this constraint to an logicor constraint containing all
/// negated variables of this knapsack constraint.
fn upgrade_cons(scip: &mut Scip, cons: ConsId, ndelconss: &mut i32, naddconss: &mut i32) -> ScipResult {
    let flags = cons_flags(scip, cons);
    let name = scip.conss[cons].name.clone();
    let n = cd(scip, cons).nvars as usize;
    let vars = cd(scip, cons).vars.0[..n].to_vec();

    // If the knapsack constraint consists only of two variables, we can
    // upgrade it to a set-packing constraint.
    let mut newcons = if n == 2 {
        crate::cons_setppc::create_cons_setpack(scip, &name, &vars, &flags)?
    }
    // If the knapsack constraint consists of at least three variables, we can
    // upgrade it to a logicor constraint containing all negated variables of
    // the knapsack.
    else {
        let mut consvars = vec![NONE; n];
        scip_var::get_negated_vars(scip, &vars, &mut consvars)?;
        crate::cons_logicor::create_cons_logicor(scip, &name, &consvars, &flags)?
    };

    // Add the upgraded constraint to the problem.
    scip_cons::add_cons_upgrade(scip, cons, &mut newcons)?;
    *naddconss += 1;

    // Remove the underlying constraint from the problem.
    scip_cons::del_cons(scip, cons)?;
    *ndelconss += 1;

    Ok(())
}

/// Divides the weights of the items up to `splitpos` and the capacity by
/// the greatest common divisor of those weights (shared tail of the
/// branches of [`delete_redundant_vars`]).
fn divide_front_by_gcd(scip: &mut Scip, cons: ConsId, splitpos: i32, nvars: i32, nchgcoefs: &mut i32, nchgsides: &mut i32) {
    // Division by greatest common divisor.
    let mut w = splitpos;
    let mut gcd = cd(scip, cons).weights[w];
    while w >= 0 && gcd > 1 {
        gcd = crate::misc::calc_gre_com_div(gcd, cd(scip, cons).weights[w]);
        w -= 1;
    }

    // Normalize if possible.
    if gcd > 1 {
        let mut w = splitpos;
        while w >= 0 {
            let weight = cd(scip, cons).weights[w];
            consdata_chg_weight(scip, cons, w, weight / gcd);
            w -= 1;
        }
        *nchgcoefs += nvars;

        cdm(scip, cons).capacity /= gcd;
        *nchgsides += 1;
    }
}

/// `deleteRedundantVars`: delete redundant variables, i.e.
/// 5x1 + 5x2 + 5x3 + 2x4 + 1x5 <= 13  =>  x4, x5 always fits into the
/// knapsack, so we can delete them;
/// 5x1 + 5x2 + 5x3 + 2x4 + 1x5 <= 8 and we have the cliqueinformation
/// (x1,x2,x3) is a clique  =>  x4, x5 always fits into the knapsack, so we
/// can delete them;
/// 5x1 + 5x2 + 5x3 + 1x4 + 1x5 <= 6 and we have the cliqueinformation
/// (x1,x2,x3) is a clique and (x4,x5) too  =>  we create the set
/// partitioning constraint x4 + x5 <= 1 and delete them in this knapsack.
#[allow(clippy::too_many_arguments)]
fn delete_redundant_vars(scip: &mut Scip, cons: ConsId, frontsum: i64, splitpos: i32, nchgcoefs: &mut i32, nchgsides: &mut i32, naddconss: &mut i32) -> ScipResult {
    let conshdlr = scip.conss[cons].conshdlr;
    let nvars = cd(scip, cons).nvars;
    let capacity = cd(scip, cons).capacity;

    // If there are no variables rear to splitpos, the constraint has no
    // redundant variables.
    if nvars - 1 == splitpos {
        return Ok(());
    }

    // Detect redundant variables.
    if cd(scip, cons).weightsum - cd(scip, cons).weights[splitpos] <= capacity {
        // All rear items are redundant, because leaving one item in front and
        // incl. of splitpos out the rear itmes always fit.

        // Delete items and update capacity.
        let mut w = nvars - 1;
        while w > splitpos {
            let weight = cd(scip, cons).weights[w];
            cdm(scip, cons).capacity -= weight;
            del_coef_pos(scip, cons, w)?;
            w -= 1;
        }
        *nchgsides += 1;
        *nchgcoefs += nvars - splitpos;

        divide_front_by_gcd(scip, cons, splitpos, nvars, nchgcoefs, nchgsides);
    }
    // Rear items can only be redundant, when the sum is smaller to the weight
    // at splitpos and all rear items would always fit into the knapsack,
    // therefor the item directly after splitpos needs to be smaller than the
    // one at splitpos and needs to fit into the knapsack.
    else if hd(scip, conshdlr).disaggregation && frontsum + cd(scip, cons).weights[splitpos + 1] <= capacity {
        let len = (nvars - (splitpos + 1)) as usize;
        let rear = cd(scip, cons).vars.0[(splitpos + 1) as usize..nvars as usize].to_vec();

        // Calculate clique partition.
        let mut clqpart = vec![0i32; len];
        let mut map = std::mem::take(&mut hdm(scip, conshdlr).probtoidxmap);
        let mut mapsize = hd(scip, conshdlr).probtoidxmapsize;
        let nclq = scip_var::calc_clique_partition(scip, &rear, &mut map, &mut mapsize, &mut clqpart);
        let h = hdm(scip, conshdlr);
        h.probtoidxmap = map;
        h.probtoidxmapsize = mapsize;
        let nclq = nclq? as usize;

        // Check if we found at least one clique.
        if nclq < len {
            let mut maxactduetoclq: i64 = 0;
            let mut cliquenum = 0;

            // Calculate maximum activity due to cliques.
            for (w, clqpart_w) in clqpart[..len].iter().enumerate() {
                if *clqpart_w == cliquenum {
                    maxactduetoclq += cd(scip, cons).weights[w + (splitpos + 1) as usize];
                    cliquenum += 1;
                }
            }

            // All rear items are redundant due to clique information, if
            // maxactduetoclq is smaller than the weight before, so delete them
            // and create for all clique the corresponding clique constraints and
            // update the capacity.
            if frontsum + maxactduetoclq <= capacity {
                let flags = cons_flags(scip, cons);
                for c in 0..nclq as i32 {
                    let clqvars: Vec<VarId> = (0..len).filter(|&w| clqpart[w] == c).map(|w| rear[w]).collect();

                    // We found a real clique so extract this constraint, because we
                    // do not know who this information generated so.
                    if clqvars.len() > 1 {
                        let name = format!("{}_clq_{}_{}", scip.conss[cons].name, capacity, c);
                        let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &clqvars, &flags)?;

                        // Add the special constraint to the problem.
                        scip_cons::add_cons(scip, cliquecons)?;
                        scip_cons::release_cons(scip, &mut cliquecons)?;
                        *naddconss += 1;
                    }
                }

                // Delete items and update capacity.
                let mut w = nvars - 1;
                while w > splitpos {
                    del_coef_pos(scip, cons, w)?;
                    *nchgcoefs += 1;
                    w -= 1;
                }
                cdm(scip, cons).capacity -= maxactduetoclq;
                *nchgsides += 1;

                divide_front_by_gcd(scip, cons, splitpos, nvars, nchgcoefs, nchgsides);
            }
        }
    }

    Ok(())
}

/// `detectRedundantVars`: detect redundant variables which always fits into
/// the knapsack (see [`delete_redundant_vars`]).
fn detect_redundant_vars(scip: &mut Scip, cons: ConsId, ndelconss: &mut i32, nchgcoefs: &mut i32, nchgsides: &mut i32, naddconss: &mut i32) -> ScipResult {
    let noldchgcoefs = *nchgcoefs;
    let nvars = cd(scip, cons).nvars;
    let capacity = cd(scip, cons).capacity;
    let mut sum: i64 = 0;

    // Search for maximal fitting items.
    let mut v = 0;
    while v < nvars && sum + cd(scip, cons).weights[v] <= capacity {
        sum += cd(scip, cons).weights[v];
        v += 1;
    }

    // All but one variable fit into the knapsack, so we can upgrade this
    // constraint to a logicor.
    if scip.conss[cons].nupgradelocks == 0 && v == nvars - 1 {
        upgrade_cons(scip, cons, ndelconss, naddconss)?;
        return Ok(());
    }

    if v < nvars - 1 {
        // Try to delete variables.
        delete_redundant_vars(scip, cons, sum, v, nchgcoefs, nchgsides, naddconss)?;

        // All but one variable fit into the knapsack, so we can upgrade this
        // constraint to a logicor.
        if scip.conss[cons].nupgradelocks == 0 && v == cd(scip, cons).nvars - 1 {
            upgrade_cons(scip, cons, ndelconss, naddconss)?;
        }

        return Ok(());
    }

    // If we already found some redundant variables, stop here.
    if *nchgcoefs > noldchgcoefs {
        return Ok(());
    }

    let conshdlr = scip.conss[cons].conshdlr;

    // Calculate clique partition.
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let r = calc_cliquepartition(scip, conshdlr, data.downcast_mut::<ConsData>().unwrap(), true, false);
    scip.conss[cons].consdata = Some(data);
    r?;

    // Check for real existing cliques.
    let clqpart = cd(scip, cons).cliquepartition.as_ref().unwrap().0[..nvars as usize].to_vec();
    if clqpart[v as usize] < v {
        let mut sumfront: i64 = 0;
        let mut maxactduetoclqfront: i64 = 0;
        let mut cliquenum = 0;

        // Calculate maximal activity due to cliques.
        let mut w = 0;
        while w < nvars {
            let weight = cd(scip, cons).weights[w];
            if clqpart[w as usize] == cliquenum {
                if maxactduetoclqfront + weight <= capacity {
                    maxactduetoclqfront += weight;
                    cliquenum += 1;
                } else {
                    break;
                }
            }
            sumfront += weight;
            w += 1;
        }

        // If all items fit, then delete the whole constraint but create clique
        // constraints which led to this information.
        if hd(scip, conshdlr).disaggregation && scip.conss[cons].nupgradelocks == 0 && w == nvars {
            let ncliques = cd(scip, cons).ncliques;
            let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
            let flags = cons_flags(scip, cons);

            for c in 0..ncliques {
                let clqvars: Vec<VarId> = (0..nvars as usize).filter(|&w| clqpart[w] == c).map(|w| vars[w]).collect();

                // We found a real clique so extract this constraint, because we do
                // not know who this information generated so.
                if clqvars.len() > 1 {
                    let name = format!("{}_clq_{}_{}", scip.conss[cons].name, capacity, c);
                    let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &clqvars, &flags)?;

                    // Add the special constraint to the problem.
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

        if w > v && w < nvars - 1 {
            // Try to delete variables.
            delete_redundant_vars(scip, cons, sumfront, w, nchgcoefs, nchgsides, naddconss)?;
        }
    }

    Ok(())
}

/// `normalizeWeights`: divides weights by their greatest common divisor and
/// divides capacity by the same value, rounding down the result.
fn normalize_weights(scip: &mut Scip, cons: ConsId, nchgcoefs: &mut i32, nchgsides: &mut i32) {
    // Sort items, because we can stop earlier if the smaller weights are
    // evaluated first.
    with_cd(scip, cons, |scip, c| sort_items(scip, c));

    let nvars = cd(scip, cons).nvars;
    let mut gcd = cd(scip, cons).weights[nvars - 1];
    let mut i = nvars - 2;
    while i >= 0 && gcd >= 2 {
        gcd = crate::misc::calc_gre_com_div(gcd, cd(scip, cons).weights[i]);
        i -= 1;
    }

    if gcd >= 2 {
        for i in 0..nvars {
            let weight = cd(scip, cons).weights[i];
            consdata_chg_weight(scip, cons, i, weight / gcd);
        }
        cdm(scip, cons).capacity /= gcd;
        *nchgcoefs += nvars;
        *nchgsides += 1;

        // Weight should still be sorted, because the reduction preserves this.
        cdm(scip, cons).sorted = true;
    }
}

/// Reduces the weights of the first `vbig` items to `newweight` and all
/// others to one, and sets the capacity to `newweight` (the case 2 endings
/// of [`dual_weights_tightening`]).
fn reduce_to_big_and_ones(scip: &mut Scip, cons: ConsId, vbig: i32, nvars: i32, newweight: i64, nchgcoefs: &mut i32) {
    // Reduce big weights.
    let mut v = 0;
    while v < vbig {
        if cd(scip, cons).weights[v] > newweight {
            consdata_chg_weight(scip, cons, v, newweight);
            *nchgcoefs += 1;
        }
        v += 1;
    }

    // Reduce small weights.
    while v < nvars {
        if cd(scip, cons).weights[v] > 1 {
            consdata_chg_weight(scip, cons, v, 1);
            *nchgcoefs += 1;
        }
        v += 1;
    }

    cdm(scip, cons).capacity = newweight;
}

/// The middle weight setting of [`dual_weights_tightening`] when twice the
/// sum of the small items fits into the dual capacity: sets the middle
/// weights to half the dual capacity, or (odd dual capacity) to the dual
/// capacity while doubling all others.
#[allow(clippy::too_many_arguments)]
fn set_middle_weights(scip: &mut Scip, cons: ConsId, v: &mut i32, end: i32, nvars: i32, dualcapacity: &mut i64, reductionsum: &mut i64, nchgcoefs: &mut i32, nchgsides: &mut i32) {
    let wt = |scip: &Scip, i: i32| cd(scip, cons).weights[i];

    // Dualcapacity is even, we can set the middle weights to dualcapacity/2.
    if (*dualcapacity & 1) == 0 {
        let newweight = *dualcapacity / 2;

        // Set all middle coefficients.
        while *v <= end {
            if wt(scip, *v) > newweight {
                *reductionsum += wt(scip, *v) - newweight;
                consdata_chg_weight(scip, cons, *v, newweight);
                *nchgcoefs += 1;
            }
            *v += 1;
        }
    }
    // Dualcapacity is odd, we can set the middle weights to dualcapacity but
    // therefor need to multiply all other coefficients by 2.
    else {
        // Correct the reductionsum.
        *reductionsum *= 2;

        // Multiply big coefficients by 2.
        for w in 0..*v {
            let weight = wt(scip, w);
            consdata_chg_weight(scip, cons, w, weight * 2);
        }

        let newweight = *dualcapacity;
        // Set all middle coefficients.
        while *v <= end {
            *reductionsum += 2 * wt(scip, *v) - newweight;
            consdata_chg_weight(scip, cons, *v, newweight);
            *v += 1;
        }

        // Multiply small coefficients by 2.
        for w in end + 1..nvars {
            let weight = wt(scip, w);
            consdata_chg_weight(scip, cons, w, weight * 2);
        }
        *nchgcoefs += nvars;

        *dualcapacity *= 2;
        cdm(scip, cons).capacity *= 2;
        *nchgsides += 1;
    }
}

/// `dualWeightsTightening`: dual weights tightening for knapsack
/// constraints
///
/// 1. a) check if all two pairs exceed the capacity, then we can upgrade
///    this constraint to a set-packing constraint
///    b) check if all but the smallest weight fit into the knapsack, then we
///    can upgrade this constraint to a logicor constraint
///
/// 2. check if besides big coefficients, that fit only by itself, for a
///    certain amount of variables all combination of these are a minimal
///    cover, then might reduce the weights and the capacity, e.g.
///
///    +219y1 + 180y2 + 74x1 + 70x2 + 63x3 + 62x4 + 53x5 <= 219  <=>  3y1 +
///    3y2 + x1 + x2 + x3 + x4 + x5 <= 3
///
/// 3. use the duality between a^Tx <= capacity  <=>  a^T~x >= weightsum -
///    capacity to tighten weights, e.g.
///
///    11x1 + 10x2 + 7x3 + 7x4 + 5x5 <= 27   <=>  11~x1 + 10~x2 + 7~x3 + 7~x4
///    + 5~x5 >= 13
///
///    the above constraint can be changed to     8~x1 + 8~x2 + 6.5~x3 +
///    6.5~x4 + 5~x5 >= 13
///
///    16~x1 + 16~x2 + 13~x3 + 13~x4 + 10~x5 >= 26  <=>  16x1 + 16x2 + 13x3 +
///    13x4 + 10x5 <= 42
fn dual_weights_tightening(scip: &mut Scip, cons: ConsId, ndelconss: &mut i32, nchgcoefs: &mut i32, nchgsides: &mut i32, naddconss: &mut i32) -> ScipResult {
    let wt = |scip: &Scip, i: i32| cd(scip, cons).weights[i];

    if scip.conss[cons].nupgradelocks >= 1 {
        return Ok(());
    }

    // Constraint should be merged.
    let nvars = cd(scip, cons).nvars;
    let capacity = cd(scip, cons).capacity;
    let oldnchgcoefs = *nchgcoefs;

    // Case 1.
    if wt(scip, nvars - 1) + wt(scip, nvars - 2) > capacity {
        // Two variable are enough to exceed the constraint, so we can update it
        // to a set-packing
        //
        // e.g. 5x1 + 4x2 + 3x3 <= 5  <=>   x1 + x2 + x3 <= 1
        let flags = cons_flags(scip, cons);
        let name = scip.conss[cons].name.clone();
        let vars = cd(scip, cons).vars.0[..nvars as usize].to_vec();
        let mut newcons = crate::cons_setppc::create_cons_setpack(scip, &name, &vars, &flags)?;

        // Add the upgraded constraint to the problem.
        scip_cons::add_cons_upgrade(scip, cons, &mut newcons)?;
        *naddconss += 1;

        // Remove the underlying constraint from the problem.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;

        return Ok(());
    }

    // All but one variable fit into the knapsack, so we can upgrade this
    // constraint to a logicor.
    if cd(scip, cons).weightsum - wt(scip, nvars - 1) <= cd(scip, cons).capacity {
        upgrade_cons(scip, cons, ndelconss, naddconss)?;
        return Ok(());
    }

    // Early termination, if the pair with biggest coeffcients together does
    // not exceed the dualcapacity.
    if cd(scip, cons).weightsum - capacity > wt(scip, 0) + wt(scip, 1) {
        return Ok(());
    }

    // Case 2.
    let mut v = 0;

    // The following is done without looking at the dualcapacity; it is enough
    // to check whether for a certain amount of variables each combination is a
    // minimal cover, some examples
    //
    // +74x1 + 70x2 + 63x3 + 62x4 + 53x5 <= 219    <=>   74~x1 + 70~x2 + 63~x3
    //                                + 62~x4 + 53~x5 >= 103
    //                                <=>    ~x1 +  ~x2 +  ~x3 +  ~x4 +  ~x5 >= 2
    //                                <=>     x1 +   x2 +   x3 +   x4 +   x5 <= 3
    //
    // +219y1 + 180y_2 +74x1 + 70x2 + 63x3 + 62x4 + 53x5 <= 219  <=>  3y1 + 3y2
    // + x1 + x2 + x3 + x4 + x5 <= 3

    // Determine big weights that fit only by itself.
    while v < nvars && wt(scip, v) + wt(scip, nvars - 1) > capacity {
        v += 1;
    }

    let vbig = v;
    let mut exceedsum: i64 = 0;

    // Determine the amount needed to exceed the capacity.
    while v < nvars && exceedsum <= capacity {
        exceedsum += wt(scip, v);
        v += 1;
    }

    // If we exceeded the capacity we might reduce the weights.
    if exceedsum > capacity {
        // All small weights were needed to exceed the capacity.
        if v == nvars {
            let newweight = nvars as i64 - vbig as i64 - 1;
            reduce_to_big_and_ones(scip, cons, vbig, nvars, newweight, nchgcoefs);
            return Ok(());
        }
        // A certain amount of small variables exceed the capacity, so check if
        // this holds for all combinations of the small weights.
        else {
            let mut exceedsumback: i64 = 0;
            let nexceed = v - vbig;

            // Determine weightsum of the same amount as before but of the smallest
            // weight.
            let mut w = nvars - 1;
            while w >= nvars - nexceed {
                exceedsumback += wt(scip, w);
                w -= 1;
            }

            // If the same amount but with the smallest possible weights also
            // exceed the capacity, it holds for all combinations of all small
            // weights.
            if exceedsumback > capacity {
                let newweight = nexceed as i64 - 1;

                // Taking out the smallest element needs to fit.
                reduce_to_big_and_ones(scip, cons, vbig, nvars, newweight, nchgcoefs);
                return Ok(());
            }
        }
    } else {
        // Either choose a big coefficients or all other variables
        //
        // 973x1 + 189x2 + 189x3 + 145x4 + 110x5 + 104x6 + 93x7 + 71x8 + 68x9 +
        // 10x10 <= 979
        //
        // either choose x1, or all other variables (weightsum of x2 to x10 is 979
        // above), so we can tighten this constraint to
        //
        // 9x1 + x2 + x3 + x4 + x5 + x6 + x7 + x8 + x9 + x10 <= 9
        if wt(scip, vbig - 1) > nvars as i64 - vbig as i64 || wt(scip, vbig) > 1 {
            let newweight = nvars as i64 - vbig as i64;
            reduce_to_big_and_ones(scip, cons, vbig, nvars, newweight, nchgcoefs);
            return Ok(());
        }
    }

    // Case 3.
    let mut dualcapacity = cd(scip, cons).weightsum - capacity;
    let mut reductionsum: i64 = 0;
    v = 0;

    'terminate: {
        // Reduce big weights
        //
        // e.g. 11x0 + 11x1 + 10x2 + 10x3 <= 32  <=>   11~x0 + 11~x1 + 10~x2 +
        //      10~x3 >= 10
        //                           <=>   10~x0 + 10~x1 + 10~x2 + 10~x3 >= 10
        //                           <=>     x0 +   x1 +   x2 +   x3 <= 3
        while wt(scip, v) > dualcapacity {
            reductionsum += wt(scip, v) - dualcapacity;
            consdata_chg_weight(scip, cons, v, dualcapacity);
            v += 1;
        }
        *nchgcoefs += v;

        // Skip weights equal to the dualcapacity, because we cannot change them.
        while v < nvars && wt(scip, v) == dualcapacity {
            v += 1;
        }

        // Any negated variable out of the first n - 1 items is enough to fulfill
        // the constraint, so we can update it to a logicor after a possible
        // removal of the last, redundant item
        //
        // e.g. 10x1 + 10x2 + 10x3 <= 20  <=>   10~x1 + 10~x2 + 10~x3 >= 10  <=>
        //      ~x1 + ~x2 + ~x3 >= 1
        if v >= nvars - 1 {
            // The last weight is not enough to satisfy the dual capacity -> remove
            // this redundant item.
            if v == nvars - 1 {
                del_coef_pos(scip, cons, nvars - 1)?;
            }
            upgrade_cons(scip, cons, ndelconss, naddconss)?;
            return Ok(());
        }
        // At least two items with weight smaller than the dual capacity.
        else if wt(scip, nvars - 1) + wt(scip, nvars - 2) >= dualcapacity {
            // We have a dual-knapsack constraint where we either need to choose
            // one variable out of a subset (big coefficients) of all or two
            // variables of the rest
            //
            // e.g. 9x1 + 9x2 + 6x3 + 4x4 <= 19  <=>   9~x1 + 9~x2 + 6~x3 + 4~x4 >= 9
            //                        <=>   2~x1 + 2~x2 +  ~x3 +  ~x4 >= 2
            //                        <=>   2x1  +  2x2 +  x3 +  x4 <= 4
            //
            //    3x1 + 3x2 + 2x3 + 2x4 + 2x5 + 2x6 + x7 <= 12  <=>  3~x1 + 3~x2 +
            //    2~x3 + 2~x4 + 2~x5 + 2~x6 + ~x7 >= 3
            //                                    <=>  2~x1 + 2~x2 +  ~x3 +  ~x4 +
            //                                    ~x5 +  ~x6 + ~x7 >= 2
            //                                    <=>  2 x1 + 2 x2 +  x3 +  x4 +
            //                                    x5 +  x6 +  x7 <= 7
            if v > 0 && wt(scip, nvars - 2) > 1 {
                let mut ncoefchg = 0;

                // Reduce all bigger weights.
                for w in 0..v {
                    if wt(scip, w) > 2 {
                        consdata_chg_weight(scip, cons, w, 2);
                        ncoefchg += 1;
                    } else {
                        break;
                    }
                }

                // Reduce all smaller weights.
                for w in v..nvars {
                    if wt(scip, w) > 1 {
                        consdata_chg_weight(scip, cons, w, 1);
                        ncoefchg += 1;
                    }
                }
                *nchgcoefs += ncoefchg;

                // Correct the capacity.
                cdm(scip, cons).capacity = (-2 + v * 2 + nvars - v) as i64;

                // Reset the reductionsum.
                reductionsum = 0;
            }
        } else {
            let minweight0 = wt(scip, nvars - 1);
            let mut newweight = dualcapacity - minweight0;
            let mut restsumweights: i64 = 0;
            let mut sumcoefcase = false;
            let mut startv = v;

            // Reduce big weights of pairs that exceed the dualcapacity
            //
            // e.g. 9x1 + 9x2 + 6x3 + 4x4 + 4x5 + 4x6 <= 27  <=>   9~x1 + 9~x2 + 6~x3
            //      + 4~x4 + 4~x5 + 4~x6 >= 9
            //                                <=>   9~x1 + 9~x2 + 5~x3 + 4~x4 + 4~x5
            //                                + 4~x6 >= 9
            //                                <=>   9x1  + 9x2  + 5x3  + 4x4  + 4x5
            //                                + 4x6  <= 27
            while wt(scip, v) > newweight {
                reductionsum += wt(scip, v) - newweight;
                consdata_chg_weight(scip, cons, v, newweight);
                v += 1;
            }
            *nchgcoefs += v - startv;

            // Skip equal weights.
            while wt(scip, v) == newweight {
                v += 1;
            }

            if v > 0 {
                for w in v..nvars {
                    restsumweights += wt(scip, w);
                }
            } else {
                restsumweights = cd(scip, cons).weightsum;
            }

            if restsumweights < dualcapacity {
                // We found redundant variables, which does not influence the
                // feasibility of any integral solution, e.g.
                //
                // +61x1  + 61x2  + 61x3  + 61x4  + 61x5  + 61x6  + 35x7  + 10x8 <=
                // 350  <=>
                // +61~x1 + 61~x2 + 61~x3 + 61~x4 + 61~x5 + 61~x6 + 35~x7 + 10~x8 >=
                // 61
                if startv == v {
                    // Remove redundant variables.
                    let mut w = nvars - 1;
                    while w >= v {
                        del_coef_pos(scip, cons, v)?;
                        *nchgcoefs += 1;
                        w -= 1;
                    }

                    // For performance reasons we do not update the capacity(, i.e.
                    // reduce it by reductionsum) and directly upgrade this
                    // constraint.
                    upgrade_cons(scip, cons, ndelconss, naddconss)?;
                    return Ok(());
                }

                // Special case where we have three different coefficient types
                //
                // e.g. 9x1 + 9x2 + 6x3 + 6x4 + 4x5 + 4x6 <= 29  <=>   9~x1 + 9~x2 +
                //      6~x3 + 6~x4 + 4~x5 + 4~x6 >= 9
                //                                <=>   9~x1 + 9~x2 + 5~x3 + 5~x4 +
                //                                4~x5 + 4~x6 >= 9
                //                                <=>   3~x1 + 3~x2 + 2~x3 + 2~x4 +
                //                                ~x5 +  ~x6 >= 3
                //                                <=>   3x1  + 3x2  + 2x3  + 2x4  +
                //                                x5 +  x6 <= 9
                if wt(scip, v) > 1 || wt(scip, startv) > nvars as i64 - v as i64 || (startv > 0 && wt(scip, 0) == nvars as i64 - v as i64 + 1) {
                    // Adjust smallest coefficients, which all together do not exceed
                    // the dualcapacity.
                    let mut w = nvars - 1;
                    while w >= v {
                        if wt(scip, w) > 1 {
                            consdata_chg_weight(scip, cons, w, 1);
                            *nchgcoefs += 1;
                        }
                        w -= 1;
                    }

                    // Adjust middle sized coefficients, which when choosing also one
                    // small coefficients exceed the dualcapacity.
                    newweight = nvars as i64 - v as i64;
                    while w >= startv {
                        if wt(scip, w) > newweight {
                            consdata_chg_weight(scip, cons, w, newweight);
                            *nchgcoefs += 1;
                        }
                        w -= 1;
                    }

                    // Adjust big sized coefficients, where each of them exceeds the
                    // dualcapacity by itself.
                    newweight += 1;
                    while w >= 0 {
                        if wt(scip, w) > newweight {
                            consdata_chg_weight(scip, cons, w, newweight);
                            *nchgcoefs += 1;
                        }
                        w -= 1;
                    }

                    // Update the capacity.
                    let newcap = (startv as i64 - 1) * newweight + (v as i64 - startv as i64) * (newweight - 1) + (nvars as i64 - v as i64);
                    if cd(scip, cons).capacity > newcap {
                        cdm(scip, cons).capacity = newcap;
                        *nchgsides += 1;
                    }
                }

                return Ok(());
            }

            // Check if all rear items have the same weight as the last one, so we
            // cannot tighten the constraint further.
            let mut end = nvars - 2;
            while end >= 0 && wt(scip, end) == wt(scip, end + 1) {
                end -= 1;
            }

            if v >= end {
                break 'terminate;
            }

            end = nvars - 2;

            // Can we stop early, another special reduction case might exist.
            if 2 * wt(scip, end) > dualcapacity {
                restsumweights = 0;

                // Determine capacity of the small items.
                for w in end + 1..nvars {
                    restsumweights += wt(scip, w);
                }

                if restsumweights * 2 <= dualcapacity {
                    // Check for further posssible reductions in the middle.
                    while v < end && restsumweights + wt(scip, v) >= dualcapacity {
                        v += 1;
                    }

                    if v >= end {
                        break 'terminate;
                    }

                    set_middle_weights(scip, cons, &mut v, end, nvars, &mut dualcapacity, &mut reductionsum, nchgcoefs, nchgsides);
                }
                break 'terminate;
            }

            // Further reductions using the next possible coefficient sum
            //
            // e.g. 9x1 + 8x2 + 7x3 + 3x4 + x5 <= 19  <=>   9~x1 + 8~x2 + 7~x3 + 3~x4
            //      + ~x5 >= 9
            //                            <=>   9~x1 + 8~x2 + 6~x3 + 3~x4 + ~x5 >= 9
            //                            <=>   9x1  + 8x2  + 6x3  + 3x4  + x5  <= 18
            for k in 0..4 {
                // Determine next minimal coefficient sum.
                let sumcoef = match k {
                    0 => wt(scip, nvars - 1) + wt(scip, nvars - 2),
                    1 => wt(scip, nvars - 1) + wt(scip, nvars - 3),
                    2 => {
                        if wt(scip, nvars - 1) + wt(scip, nvars - 4) < wt(scip, nvars - 2) + wt(scip, nvars - 3) {
                            sumcoefcase = true;
                            wt(scip, nvars - 1) + wt(scip, nvars - 4)
                        } else {
                            sumcoefcase = false;
                            wt(scip, nvars - 2) + wt(scip, nvars - 3)
                        }
                    }
                    3 => {
                        if sumcoefcase {
                            crate::def::min(wt(scip, nvars - 1) + wt(scip, nvars - 5), wt(scip, nvars - 2) + wt(scip, nvars - 3))
                        } else {
                            crate::def::min(wt(scip, nvars - 1) + wt(scip, nvars - 4), wt(scip, nvars - 1) + wt(scip, nvars - 2) + wt(scip, nvars - 3))
                        }
                    }
                    _ => return Err(Retcode::Error),
                };

                // Tighten next coefficients that, pair with the current small
                // coefficient, exceed the dualcapacity.
                let mut minweight = wt(scip, end);
                while minweight <= sumcoef {
                    newweight = dualcapacity - minweight;
                    startv = v;

                    // Shrink big coefficients.
                    while wt(scip, v) + minweight > dualcapacity && 2 * minweight <= dualcapacity {
                        reductionsum += wt(scip, v) - newweight;
                        consdata_chg_weight(scip, cons, v, newweight);
                        v += 1;
                    }
                    *nchgcoefs += v - startv;

                    // Skip unchangable weights.
                    while wt(scip, v) + minweight == dualcapacity {
                        v += 1;
                    }

                    end -= 1;
                    // Skip same end weights.
                    while end >= 0 && wt(scip, end) == wt(scip, end + 1) {
                        end -= 1;
                    }

                    if v >= end {
                        break 'terminate;
                    }

                    minweight = wt(scip, end);
                }

                if v >= end {
                    break 'terminate;
                }

                // Now check if a combination of small coefficients allows us to
                // tighten big coefficients further.
                if sumcoef < minweight {
                    minweight = sumcoef;
                    newweight = dualcapacity - minweight;
                    startv = v;

                    // Shrink big coefficients.
                    while wt(scip, v) + minweight > dualcapacity && 2 * minweight <= dualcapacity {
                        reductionsum += wt(scip, v) - newweight;
                        consdata_chg_weight(scip, cons, v, newweight);
                        v += 1;
                    }
                    *nchgcoefs += v - startv;

                    // Skip unchangable weights.
                    while wt(scip, v) + minweight == dualcapacity {
                        v += 1;
                    }
                }

                if v >= end {
                    break 'terminate;
                }

                // Can we stop early, another special reduction case might exist.
                if 2 * wt(scip, end) > dualcapacity {
                    restsumweights = 0;

                    // Determine capacity of the small items.
                    for w in end + 1..nvars {
                        restsumweights += wt(scip, w);
                    }

                    if restsumweights * 2 <= dualcapacity {
                        // Check for further posssible reductions in the middle.
                        while v < end && restsumweights + wt(scip, v) >= dualcapacity {
                            v += 1;
                        }

                        if v >= end {
                            break 'terminate;
                        }

                        set_middle_weights(scip, cons, &mut v, end, nvars, &mut dualcapacity, &mut reductionsum, nchgcoefs, nchgsides);
                    }
                    break 'terminate;
                }

                // Cannot tighten any further.
                if 2 * sumcoef > dualcapacity {
                    break 'terminate;
                }
            }
        }
    }

    // Correct capacity.
    if reductionsum > 0 {
        cdm(scip, cons).capacity -= reductionsum;
        *nchgsides += 1;
    }

    if oldnchgcoefs < *nchgcoefs {
        // It might be that we can divide the weights by their greatest common
        // divisor.
        normalize_weights(scip, cons, nchgcoefs, nchgsides);
    }

    Ok(())
}

/// `prepareCons`: fixes variables with weights bigger than the capacity and
/// delete redundant constraints, also sort weights.
fn prepare_cons(scip: &mut Scip, cons: ConsId, nfixedvars: &mut i32, ndelconss: &mut i32, nchgcoefs: &mut i32) -> ScipResult {
    let nvars = cd(scip, cons).nvars;

    // No variables left, then delete constraint.
    if nvars == 0 {
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok(());
    }

    // Sort items.
    with_cd(scip, cons, |scip, c| sort_items(scip, c));

    let capacity = cd(scip, cons).capacity;

    let mut v = 0;

    // Check for weights bigger than the capacity.
    while v < nvars && cd(scip, cons).weights[v] > capacity {
        let var = cd(scip, cons).vars[v];
        let (_infeasible, fixed) = scip_var::fix_var(scip, var, 0.0)?;
        if fixed {
            *nfixedvars += 1;
        }
        v += 1;
    }

    // If we fixed at least one variable we need to delete them from the
    // constraint.
    if v > 0 {
        if v == nvars {
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
            return Ok(());
        }

        // Delete all position from back to front.
        v -= 1;
        while v >= 0 {
            del_coef_pos(scip, cons, v)?;
            *nchgcoefs += 1;
            v -= 1;
        }

        // Sort items again because of deletion.
        with_cd(scip, cons, |scip, c| sort_items(scip, c));
    }

    if !scip.set.is_huge_value(capacity as f64) && cd(scip, cons).weightsum <= capacity {
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
    }

    Ok(())
}

/// `simplifyInequalities`: tries to simplify weights and delete redundant
/// variables in knapsack a^Tx <= capacity
///
/// 1. use the duality between a^Tx <= capacity  <=>  -a^T~x <= capacity -
///    weightsum to tighten weights, e.g.
///
///    11x1 + 10x2 + 7x3 + 5x4 + 5x5 <= 25   <=>  -10~x1 - 10~x2 - 7~x3 - 5~x4
///    - 5~x5 <= -13
///
///    the above constraint can be changed to
///
///    -8~x1 - 8~x2 - 7~x3 - 5~x4 - 5~x5 <= -12  <=>  8x1 + 8x2 + 7x3 + 5x4 +
///    5x5 <= 20
///
/// 2. if variables in a constraint do not affect the (in-)feasibility of the
///    constraint, we can delete them, e.g.
///
///    7x1 + 6x2 + 5x3 + 5x4 + x5 + x6 <= 20 => x5 and x6 are redundant and
///    can be removed
///
/// 3. Tries to use gcd information an all but one weight to change this
///    not-included weight and normalize the constraint further, e.g.
///
///    9x1 + 6x2 + 6x3 + 5x4 <= 13  =>  9x1 + 6x2 + 6x3 + 6x4 <= 12  =>  3x1 +
///    2x2 + 2x3 + 2x4 <= 4  =>  4x1 + 2x2 + 2x3 + 2x4 <= 4
///    =>
///    2x1 + x2 + x3 + x4 <= 2
///    9x1 + 6x2 + 6x3 + 7x4 <= 13  =>  9x1 + 6x2 + 6x3 + 6x4 <= 12  =>  see
///    above
///
/// Returns whether the node can be cut off.
#[allow(clippy::too_many_arguments)]
fn simplify_inequalities(
    scip: &mut Scip,
    cons: ConsId,
    nfixedvars: &mut i32,
    ndelconss: &mut i32,
    nchgcoefs: &mut i32,
    nchgsides: &mut i32,
    naddconss: &mut i32,
) -> ScipResult<bool> {
    let wt = |scip: &Scip, i: i32| cd(scip, cons).weights[i];

    // Remove double enties and also combinations of active and negated
    // variables.
    if merge_multiples(scip, cons)? {
        return Ok(true);
    }

    // Fix variables with big coefficients and remove redundant constraints,
    // sort weights.
    prepare_cons(scip, cons, nfixedvars, ndelconss, nchgcoefs)?;

    if scip.conss[cons].deleted {
        return Ok(false);
    }

    if !scip.set.is_huge_value(cd(scip, cons).capacity as f64) {
        // 1. dual weights tightening
        dual_weights_tightening(scip, cons, ndelconss, nchgcoefs, nchgsides, naddconss)?;

        if scip.conss[cons].deleted {
            return Ok(false);
        }

        // 2. delete redundant variables
        detect_redundant_vars(scip, cons, ndelconss, nchgcoefs, nchgsides, naddconss)?;

        if scip.conss[cons].deleted {
            return Ok(false);
        }
    }

    let mut nvars = cd(scip, cons).nvars;

    // Determine greatest common divisor.
    let mut gcd = wt(scip, nvars - 1);
    let mut v = nvars - 2;
    while v >= 0 && gcd > 1 {
        gcd = crate::misc::calc_gre_com_div(gcd, wt(scip, v));
        v -= 1;
    }

    // Divide the constraint by their greatest common divisor.
    if gcd >= 2 {
        let mut v = nvars - 1;
        while v >= 0 {
            let weight = wt(scip, v);
            consdata_chg_weight(scip, cons, v, weight / gcd);
            v -= 1;
        }
        *nchgcoefs += nvars;

        cdm(scip, cons).capacity /= gcd;
        *nchgsides += 1;
    }

    // 3. start gcd procedure for all variables
    loop {
        nvars = cd(scip, cons).nvars;

        // Stop if we have two coefficients which are one in absolute value.
        if wt(scip, nvars - 1) == 1 && wt(scip, nvars - 2) == 1 {
            return Ok(false);
        }

        let mut v = 0;
        // Determine coefficients as big as the capacity, these we do not need to
        // take into account when calculating the gcd.
        while wt(scip, v) == cd(scip, cons).capacity {
            v += 1;
        }

        // All but one variable are as big as the capacity, this is handled
        // elsewhere.
        if v == nvars - 1 {
            return Ok(false);
        }

        let offsetv = v;

        let mut gcd: i64 = -1;
        let mut candpos: i32 = -1;
        let mut candpos2: i32 = -1;

        // Calculate greatest common divisor over all integer and binary
        // variables and determine the candidate where we might change the
        // coefficient.
        let mut v = nvars - 1;
        while v >= offsetv {
            let weight = wt(scip, v);

            let oldgcd = gcd;

            if gcd == -1 {
                gcd = wt(scip, v);
            } else {
                // Calculate greatest common divisor for all variables.
                gcd = crate::misc::calc_gre_com_div(gcd, weight);
            }

            // If the greatest commmon divisor has become 1, we might have found
            // the possible coefficient to change or we can terminate.
            if gcd == 1 {
                // Found candidate.
                if candpos == -1 {
                    gcd = oldgcd;
                    candpos = v;

                    // If both first coefficients have a gcd of 1, both are
                    // candidates for the coefficient change.
                    if v == nvars - 2 {
                        candpos2 = v + 1;
                    }
                }
                // Two different variables lead to a gcd of one, so we cannot change
                // a coefficient.
                else if candpos == v + 1 && candpos2 == v + 2 {
                    // Take new candidates.
                    candpos = candpos2;

                    // Recalculate gcd from scratch.
                    gcd = wt(scip, v + 1);

                    // Calculate greatest common divisor for variables.
                    gcd = crate::misc::calc_gre_com_div(gcd, wt(scip, v));
                    if gcd == 1 {
                        return Ok(false);
                    }
                } else {
                    // Cannot determine a possible coefficient for reduction.
                    return Ok(false);
                }
            }
            v -= 1;
        }

        // We should have found one coefficient, that led to a gcd of 1,
        // otherwise we could normalize the constraint further.

        // Determine the remainder of the capacity and the gcd.
        let rest = cd(scip, cons).capacity % gcd;

        'cont: {
            if candpos == -1 {
                // We assume that the constraint was normalized; replace old with
                // new capacity.
                cdm(scip, cons).capacity -= rest;
                *nchgsides += 1;

                // Replace old big coefficients with new capacity.
                for v in 0..offsetv {
                    let capacity = cd(scip, cons).capacity;
                    consdata_chg_weight(scip, cons, v, capacity);
                }

                *nchgcoefs += offsetv;
                break 'cont;
            }

            // Determine the remainder of the coefficient candidate and the gcd.
            let restweight = wt(scip, candpos) % gcd;

            // Calculate new coefficient.
            let newweight = if restweight > rest {
                wt(scip, candpos) - restweight + gcd
            } else {
                wt(scip, candpos) - restweight
            };

            // Must not change weights and capacity if one variable would be
            // removed and we have a big coefficient, e.g., 11x1 + 6x2 + 6x3 + 5x4
            // <= 11 => gcd = 6, offsetv = 1 => newweight = 0, but we would lose x1
            // = 1 => x4 = 0.
            if newweight == 0 && offsetv > 0 {
                return Ok(false);
            }

            if rest > 0 {
                // Replace old with new capacity.
                cdm(scip, cons).capacity -= rest;
                *nchgsides += 1;

                // Replace old big coefficients with new capacity.
                for v in 0..offsetv {
                    let capacity = cd(scip, cons).capacity;
                    consdata_chg_weight(scip, cons, v, capacity);
                }

                *nchgcoefs += offsetv;
            }

            if newweight == 0 {
                // Delete redundant coefficient.
                del_coef_pos(scip, cons, candpos)?;
                nvars -= 1;
            } else {
                // Replace old with new coefficient.
                consdata_chg_weight(scip, cons, candpos, newweight);
            }
            *nchgcoefs += 1;
        }

        // Now constraint can be normalized, dividing it by the gcd.
        let mut v = nvars - 1;
        while v >= 0 {
            let weight = wt(scip, v);
            consdata_chg_weight(scip, cons, v, weight / gcd);
            v -= 1;
        }
        *nchgcoefs += nvars;

        cdm(scip, cons).capacity /= gcd;
        *nchgsides += 1;

        if nvars < 2 {
            break;
        }
    }

    Ok(false)
}

/// The list of binary zero implications of [`tighten_weights_lift`]:
/// `zeroitems` and `nextidxs` with C's buffer size, plus the per
/// variable/value lists.
struct Zerolist {
    /// Binary variables that have at least one entry in zeroitems.
    liftcands: [Vec<i32>; 2],
    /// Number of lifting candidates.
    nliftcands: [usize; 2],
    /// First index in zeroitems for each binary variable/value pair, or zero
    /// for empty list.
    firstidxs: [CVec<i32>; 2],
    /// Sums of weights of the implied-to-zero items.
    zeroweightsums: [CVec<i64>; 2],
    /// Item number in knapsack that is implied to zero.
    zeroitems: Vec<i32>,
    /// Next index in zeroitems for the same binary variable, or zero for end of
    /// list.
    nextidxs: Vec<i32>,
    zeroitemssize: i32,
    nzeroitems: i32,
}

/// `insertZerolist`: inserts an element into the list of binary zero
/// implications.
fn insert_zerolist(z: &mut Zerolist, probindex: i32, value: bool, knapsackidx: i32, knapsackweight: i64, memlimitreached: &mut bool) {
    let nzeros = z.nzeroitems;

    // Allocate enough memory.
    if nzeros == z.zeroitemssize {
        // We explicitly construct the complete implication graph where the
        // knapsack variables are involved; this can be too huge - abort on
        // memory limit.
        if z.zeroitemssize >= MAX_ZEROITEMS_SIZE {
            *memlimitreached = true;
            return;
        }
        z.zeroitemssize *= 2;
        z.zeroitemssize = crate::def::min(z.zeroitemssize, MAX_ZEROITEMS_SIZE);
        z.zeroitems.resize(z.zeroitemssize as usize, 0);
        z.nextidxs.resize(z.zeroitemssize as usize, 0);
    }

    if *memlimitreached {
        *memlimitreached = false;
    }

    // Insert element.
    let v = value as usize;
    let p = probindex as usize;
    z.zeroitems[nzeros as usize] = knapsackidx;
    z.nextidxs[nzeros as usize] = z.firstidxs[v][p];
    if z.firstidxs[v][p] == 0 {
        z.liftcands[v][z.nliftcands[v]] = probindex;
        z.nliftcands[v] += 1;
    }
    z.firstidxs[v][p] = nzeros;
    z.nzeroitems += 1;
    z.zeroweightsums[v][p] += knapsackweight;
}

/// Maximal clique length that is used in [`tighten_weights_lift`].
const MAX_CLIQUELENGTH: i32 = 50;

/// Grows a cleared scratch array of the handler data to `n` entries.
fn ensure_cleared<T: Copy + Default>(array: &mut CVec<T>, size: &mut i32, n: i32) {
    if *size < n {
        *size = n;
        array.0.resize(n as usize, T::default());
    }
}

/// `tightenWeightsLift`: applies rule (3) of the weight tightening
/// procedure, which can lift other variables into the knapsack:
/// (3) for a clique C let C(xi == v) := C \ {j: xi == v -> xj == 0}), let
///     cliqueweightsum(xi == v) := sum(W(C(xi == v)))
///     if cliqueweightsum(xi == v) < capacity:
///     - fixing variable xi to v would make the knapsack constraint
///       redundant
///     - the weight of the variable or its negation (depending on v) can be
///       increased as long as it has the same redundancy effect:
///        wi'     := capacity - cliqueweightsum(xi == v)
///     this rule can also be applied to binary variables not in the
///     knapsack!
///
/// Returns whether the node can be cut off.
fn tighten_weights_lift(scip: &mut Scip, cons: ConsId, nchgcoefs: &mut i32) -> ScipResult<bool> {
    let mut cutoff = false;
    let nvars = cd(scip, cons).nvars;

    // Check if the knapsack has too many items/cliques for applying this
    // costly method.
    if (!cd(scip, cons).cliquepartitioned && nvars > MAX_USECLIQUES_SIZE) || cd(scip, cons).ncliques > MAX_USECLIQUES_SIZE {
        return Ok(false);
    }

    // Sort items, s.t. the heaviest one is in the first position.
    with_cd(scip, cons, |scip, c| sort_items(scip, c));

    if !cd(scip, cons).cliquepartitioned && nvars > MAX_USECLIQUES_SIZE {
        return Ok(false);
    }

    // We have to consider all integral variables since even integer and
    // implicit integer variables can have binary bounds.
    let nbinvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip);
    let binvars: Vec<VarId> = api::get_vars(scip)[..nbinvars as usize].to_vec();

    // Get conshdlrdata to use cleared memory.
    let conshdlr = scip.conss[cons].conshdlr;

    // Next if conditions should normally not be true, because it means that
    // presolving has created more binary variables than binary + integer
    // variables existed at the presolving initialization method, but for
    // example if you would transform all integers into their binary
    // representation then it maybe happens.
    let h = hdm(scip, conshdlr);
    ensure_cleared(&mut h.ints1, &mut h.ints1size, nbinvars);
    ensure_cleared(&mut h.ints2, &mut h.ints2size, nbinvars);
    ensure_cleared(&mut h.longints1, &mut h.longints1size, nbinvars);
    ensure_cleared(&mut h.longints2, &mut h.longints2size, nbinvars);

    // Allocate temporary memory for the list of implied to zero variables.
    // Initial size of zeroitems buffer.
    let zeroitemssize = crate::def::min(nbinvars, MAX_ZEROITEMS_SIZE);
    let mut z = Zerolist {
        liftcands: [vec![0; nbinvars as usize], vec![0; nbinvars as usize]],
        nliftcands: [0, 0],
        firstidxs: [std::mem::take(&mut h.ints1), std::mem::take(&mut h.ints2)],
        zeroweightsums: [std::mem::take(&mut h.longints1), std::mem::take(&mut h.longints2)],
        zeroitems: vec![0; zeroitemssize as usize],
        nextidxs: vec![0; zeroitemssize as usize],
        zeroitemssize,
        nzeroitems: 1,
    };
    // Dummy element.
    z.zeroitems[0] = -1;
    z.nextidxs[0] = -1;

    // Next if conditions should normally not be true (see above).
    ensure_cleared(&mut h.bools1, &mut h.bools1size, nbinvars);
    ensure_cleared(&mut h.bools2, &mut h.bools2size, nbinvars);
    let mut zeroiteminserted = [std::mem::take(&mut h.bools1), std::mem::take(&mut h.bools2)];

    let n2 = 2 * nbinvars as usize;
    let mut tmpboolindices3 = vec![false; nvars as usize];
    let mut tmpboolindices2 = vec![false; n2];
    let mut tmpindices3 = vec![0i32; nvars as usize];
    let mut tmpindices2 = vec![0i32; n2];
    let mut tmpindices = vec![0i32; n2];
    let mut tmpboolindices = vec![false; n2];

    let mut tmp2 = 0;
    let mut tmp3 = 0;

    let mut memlimitreached = false;
    let mut i = 0;
    while i < cd(scip, cons).nvars && !memlimitreached {
        let mut tmp = 0;

        // Get corresponding active problem variable.
        let mut var = cd(scip, cons).vars[i];
        let weight = cd(scip, cons).weights[i];
        let mut value = true;
        crate::var::var_get_probvar_binary(scip, &mut var, &mut value)?;
        let varprobindex = scip.vars[var].probindex;

        // Update the zeroweightsum.
        z.zeroweightsums[!value as usize][varprobindex] += weight;
        tmpboolindices3[tmp3] = !value;
        tmpindices3[tmp3] = varprobindex;
        tmp3 += 1;

        // Initialize the arrays of inserted zero items; first add the
        // implications (~x == 1 -> x == 0).
        {
            let probindex = scip.vars[var].probindex;
            let implvalue = !value;

            // Insert the item into the list of the implied variable/value.
            if z.firstidxs[implvalue as usize][probindex] == 0 {
                tmpboolindices2[tmp2] = implvalue;
                tmpindices2[tmp2] = probindex;
                tmp2 += 1;
            }

            insert_zerolist(&mut z, probindex, implvalue, i, weight, &mut memlimitreached);
            zeroiteminserted[implvalue as usize][probindex] = true;
            tmpboolindices[tmp] = implvalue;
            tmpindices[tmp] = probindex;
            tmp += 1;
        }

        // Get the cliques where the knapsack item is member of with value 1.
        let cliques: Vec<usize> = match scip.vars[var].cliquelist.as_ref() {
            Some(l) => l.cliques[value as usize].0[..l.ncliques[value as usize] as usize].to_vec(),
            None => Vec::new(),
        };
        let mut j = 0;
        while j < cliques.len() && !memlimitreached {
            let q = &scip.cliques[cliques[j]];
            let ncliquevars = q.nvars;

            // Discard big cliques.
            if ncliquevars > MAX_CLIQUELENGTH {
                j += 1;
                continue;
            }

            let cliquevars = q.vars.0[..ncliquevars as usize].to_vec();
            let cliquevalues = q.values.0[..ncliquevars as usize].to_vec();

            for k in (0..ncliquevars as usize).rev() {
                if var == cliquevars[k] {
                    continue;
                }

                let probindex = scip.vars[cliquevars[k]].probindex;
                if probindex == -1 {
                    continue;
                }

                let implvalue = cliquevalues[k];

                // Insert the item into the list of the clique variable/value.
                if !zeroiteminserted[implvalue as usize][probindex] {
                    if z.firstidxs[implvalue as usize][probindex] == 0 {
                        tmpboolindices2[tmp2] = implvalue;
                        tmpindices2[tmp2] = probindex;
                        tmp2 += 1;
                    }

                    insert_zerolist(&mut z, probindex, implvalue, i, weight, &mut memlimitreached);
                    zeroiteminserted[implvalue as usize][probindex] = true;
                    tmpboolindices[tmp] = implvalue;
                    tmpindices[tmp] = probindex;
                    tmp += 1;

                    if memlimitreached {
                        break;
                    }
                }
            }
            j += 1;
        }

        // Clear zeroiteminserted.
        while tmp > 0 {
            tmp -= 1;
            zeroiteminserted[tmpboolindices[tmp] as usize][tmpindices[tmp]] = false;
        }
        i += 1;
    }

    // Calculate the clique partition and the maximal sum of weights using the
    // clique information.
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let r = calc_cliquepartition(scip, conshdlr, data.downcast_mut::<ConsData>().unwrap(), true, false);
    scip.conss[cons].consdata = Some(data);
    r?;

    // Next if condition should normally not be true, because it means that
    // presolving has created more binary variables in one constraint than
    // binary + integer variables existed in the whole problem at the
    // presolving initialization method, but for example if you would
    // transform all integers into their binary representation then it maybe
    // happens.
    let cnvars = cd(scip, cons).nvars;
    let h = hdm(scip, conshdlr);
    ensure_cleared(&mut h.bools3, &mut h.bools3size, cnvars);
    let mut cliqueused = std::mem::take(&mut h.bools3);

    let mut maxcliqueweightsum: i64 = 0;
    let mut tmp = 0;

    // Calculates maximal weight of cliques.
    for i in 0..cnvars {
        let cliquenum = cd(scip, cons).cliquepartition.as_ref().unwrap()[i];
        if !cliqueused[cliquenum] {
            maxcliqueweightsum += cd(scip, cons).weights[i];
            cliqueused[cliquenum] = true;
            tmpindices[tmp] = cliquenum;
            tmp += 1;
        }
    }

    // Clear cliqueused (C clears the first entries, which are the clique
    // numbers used).
    while tmp > 0 {
        tmp -= 1;
        cliqueused[tmp] = false;
    }

    // Next if condition should normally not be true (see above).
    let h = hdm(scip, conshdlr);
    ensure_cleared(&mut h.bools4, &mut h.bools4size, cnvars);
    let mut itemremoved = std::mem::take(&mut h.bools4);

    // For each binary variable xi and each fixing v, calculate the
    // cliqueweightsum and update the weight of the variable in the knapsack
    // (this is sequence-dependent because the new or modified weights have to
    // be included in subsequent cliqueweightsum calculations).
    let mut addvars: Vec<VarId> = Vec::with_capacity(n2);
    let mut addweights: Vec<i64> = Vec::with_capacity(n2);
    let mut addweightsum: i64 = 0;
    let capacity = cd(scip, cons).capacity;
    let mut val = 0;
    while val < 2 && addweightsum < capacity {
        let mut i = 0;
        while i < z.nliftcands[val] && addweightsum < capacity {
            let mut tmp = 0;
            let probindex = z.liftcands[val][i] as usize;

            // Ignore empty zero lists and variables that cannot be lifted
            // anyways.
            if z.firstidxs[val][probindex] == 0 || maxcliqueweightsum - z.zeroweightsums[val][probindex] + addweightsum >= capacity {
                i += 1;
                continue;
            }

            // Mark the items that are implied to zero by setting the current
            // variable to the current value.
            let mut idx = z.firstidxs[val][probindex];
            while idx != 0 {
                itemremoved[z.zeroitems[idx as usize]] = true;
                idx = z.nextidxs[idx as usize];
            }

            // Calculate the residual cliqueweight sum; the previously added items
            // are single-element cliques.
            let mut cliqueweightsum = addweightsum;
            for j in 0..cnvars {
                let cliquenum = cd(scip, cons).cliquepartition.as_ref().unwrap()[j];
                if !itemremoved[j] {
                    if !cliqueused[cliquenum] {
                        cliqueweightsum += cd(scip, cons).weights[j];
                        cliqueused[cliquenum] = true;
                        tmpindices[tmp] = cliquenum;
                        tmp += 1;
                    }

                    if cliqueweightsum >= capacity {
                        break;
                    }
                }
            }

            // Check if the weight of the variable/value can be increased.
            if cliqueweightsum < capacity {
                // Insert the variable (with value TRUE) in the list of additional
                // items.
                let mut var = binvars[probindex];
                if val == 0 {
                    var = scip_var::get_negated_var(scip, var)?;
                }
                let weight = capacity - cliqueweightsum;
                addvars.push(var);
                addweights.push(weight);
                addweightsum += weight;
            }

            // Clear itemremoved.
            let mut idx = z.firstidxs[val][probindex];
            while idx != 0 {
                itemremoved[z.zeroitems[idx as usize]] = false;
                idx = z.nextidxs[idx as usize];
            }

            // Clear cliqueused.
            while tmp > 0 {
                tmp -= 1;
                cliqueused[tmpindices[tmp]] = false;
            }
            i += 1;
        }
        val += 1;
    }

    // Clear part of zeroweightsums.
    while tmp3 > 0 {
        tmp3 -= 1;
        z.zeroweightsums[tmpboolindices3[tmp3] as usize][tmpindices3[tmp3]] = 0;
    }

    // Clear rest of zeroweightsums and firstidxs.
    while tmp2 > 0 {
        tmp2 -= 1;
        z.zeroweightsums[tmpboolindices2[tmp2] as usize][tmpindices2[tmp2]] = 0;
        z.firstidxs[tmpboolindices2[tmp2] as usize][tmpindices2[tmp2]] = 0;
    }

    // Put the cleared scratch arrays back.
    let h = hdm(scip, conshdlr);
    let [f0, f1] = z.firstidxs;
    let [w0, w1] = z.zeroweightsums;
    let [b1, b2] = zeroiteminserted;
    h.ints1 = f0;
    h.ints2 = f1;
    h.longints1 = w0;
    h.longints2 = w1;
    h.bools1 = b1;
    h.bools2 = b2;
    h.bools3 = cliqueused;
    h.bools4 = itemremoved;

    // Add all additional item weights.
    for i in 0..addvars.len() {
        add_coef(scip, cons, addvars[i], addweights[i])?;
    }
    *nchgcoefs += addvars.len() as i32;

    if !addvars.is_empty() {
        // If new items were added, multiple entries of the same variable are
        // possible and we have to clean up the constraint.
        cutoff = merge_multiples(scip, cons)?;
    }

    Ok(cutoff)
}

/// `tightenWeights`: tightens item weights and capacity in presolving:
/// given a knapsack sum(wi*xi) <= capacity
/// (1) let weightsum := sum(wi)
///     if weightsum - wi < capacity:
///     - not using item i would make the knapsack constraint redundant
///     - wi and capacity can be changed to have the same redundancy effect and
///       the same results for fixing xi to zero or one, but with a reduced
///       wi and tightened capacity to tighten the LP relaxation
///     - change coefficients:
///         wi'       := weightsum - capacity
///         capacity' := capacity - (wi - wi')
/// (2) increase weights from front to back(sortation is necessary) if there
///     is no space left for another weight
///     - determine the four(can be adjusted) minimal weightsums of the
///       knapsack, i.e. in increasing order
///       weights[nvars - 1], weights[nvars - 2], MIN(weights[nvars - 3],
///       weights[nvars - 1] + weights[nvars - 2]), MIN(MAX(weights[nvars - 3],
///       weights[nvars - 1] + weights[nvars - 2]), weights[nvars - 4]), note
///       that there can be multiple times the same weight, this can be
///       improved
///     - check if summing up a minimal weightsum with a big weight exceeds
///       the capacity, then we can increase the big weight, to capacity -
///       lastmininmalweightsum, e.g. :
///           19x1 + 15x2 + 10x3 + 5x4 + 5x5 <= 19
///       ->  minimal weightsums: 5, 5, 10, 10
///       ->  15 + 5 > 19 => increase 15 to 19 - 0 = 19
///       ->  10 + 10 > 19 => increase 10 to 19 - 5 = 14, resulting in
///           19x1 + 19x2 + 14x3 + 5x4 + 5x5 <= 19
/// (3) let W(C) be the maximal weight of clique C,
///       cliqueweightsum := sum(W(C))
///     if cliqueweightsum - W(C) < capacity:
///     - not using any item of C would make the knapsack constraint redundant
///     - weights wi, i in C, and capacity can be changed to have the same
///       redundancy effect and the same results for fixing xi, i in C, to
///       zero or one, but with a reduced wi and tightened capacity to tighten
///       the LP relaxation
///     - let D := W(C) - (capacity - cliqueweightsum), then change
///       coefficients:
///         wi'       := max(wi - D, 0)
///         capacity' := capacity - D
/// (4) for a clique C let C(xi == v) := C \ {j: xi == v -> xj == 0}),
///     let cliqueweightsum(xi == v) := sum(W(C(xi == v)))
///     if cliqueweightsum(xi == v) < capacity:
///     - fixing variable xi to v would make the knapsack constraint redundant
///     - the weight of the variable or its negation (depending on v) can be
///       increased as long as it has the same redundancy effect:
///        wi'     := capacity - cliqueweightsum(xi == v)
///     This rule can also be applied to binary variables not in the
///     knapsack!
/// (5) if min{w} + wi > capacity:
///     - using item i would force to fix other items to zero
///     - wi can be increased to the capacity
///
/// Returns whether the node can be cut off.
#[allow(clippy::too_many_arguments)]
fn tighten_weights(
    scip: &mut Scip,
    cons: ConsId,
    presoltiming: PresolTiming,
    nchgcoefs: &mut i32,
    nchgsides: &mut i32,
    naddconss: &mut i32,
    ndelconss: &mut i32,
) -> ScipResult<bool> {
    let conshdlr = scip.conss[cons].conshdlr;
    let mut sumcoefcase = false;

    let cutoff = merge_multiples(scip, cons)?;
    if cutoff {
        return Ok(true);
    }

    // Apply rule (1).
    if (presoltiming & PRESOLTIMING_FAST) != 0 {
        loop {
            // Sort items, s.t. the heaviest one is in the first position.
            with_cd(scip, cons, |scip, c| sort_items(scip, c));

            for i in 0..cd(scip, cons).nvars {
                let weight = cd(scip, cons).weights[i];
                let c = cd(scip, cons);
                if c.weightsum - weight < c.capacity {
                    let newweight = c.weightsum - c.capacity;
                    consdata_chg_weight(scip, cons, i, newweight);
                    cdm(scip, cons).capacity -= weight - newweight;
                    *nchgcoefs += 1;
                    *nchgsides += 1;
                } else {
                    break;
                }
            }
            let c = cd(scip, cons);
            if !(!c.sorted && c.weightsum > c.capacity) {
                break;
            }
        }
    }

    // Check for redundancy.
    if cd(scip, cons).weightsum <= cd(scip, cons).capacity {
        return Ok(false);
    }

    let mut pos: i32 = 0;
    while pos < cd(scip, cons).nvars && cd(scip, cons).weights[pos] == cd(scip, cons).capacity {
        pos += 1;
    }

    let mut sumcoef: i64 = 0;
    let nvars = cd(scip, cons).nvars;
    let capacity = cd(scip, cons).capacity;
    let w = |scip: &Scip, i: i32| cd(scip, cons).weights[i];

    // C reads weights[pos + 1] even at pos == nvars - 1, where the loop below
    // breaks at once whatever it holds; read the spare slot when it exists.
    let wnext = |scip: &Scip, i: i32| cd(scip, cons).weights.0.get(i as usize).copied().unwrap_or(0);
    if (presoltiming & (PRESOLTIMING_FAST | PRESOLTIMING_MEDIUM)) != 0 && pos < nvars && w(scip, pos) + wnext(scip, pos + 1) > capacity {
        // Further reductions using the next possible coefficient sum
        //
        // e.g. 19x1 + 15x2 + 10x3 + 5x4 + 5x5 <= 19  <=>  19x1 + 19x2 + 14x3 + 5x4 + 5x5  <= 19
        for k in 0..4 {
            let mut newweight = capacity - sumcoef;
            let backpos;

            // Determine next minimal coefficient sum.
            match k {
                0 => {
                    sumcoef = w(scip, nvars - 1);
                    backpos = nvars - 1;
                }
                1 => {
                    sumcoef = w(scip, nvars - 2);
                    backpos = nvars - 2;
                }
                2 => {
                    if w(scip, nvars - 3) < w(scip, nvars - 1) + w(scip, nvars - 2) {
                        sumcoefcase = true;
                        sumcoef = w(scip, nvars - 3);
                        backpos = nvars - 3;
                    } else {
                        sumcoefcase = false;
                        sumcoef = w(scip, nvars - 1) + w(scip, nvars - 2);
                        backpos = nvars - 2;
                    }
                }
                _ => {
                    if sumcoefcase {
                        if w(scip, nvars - 4) < w(scip, nvars - 1) + w(scip, nvars - 2) {
                            sumcoef = w(scip, nvars - 4);
                            backpos = nvars - 4;
                        } else {
                            sumcoef = w(scip, nvars - 1) + w(scip, nvars - 2);
                            backpos = nvars - 2;
                        }
                    } else {
                        sumcoef = w(scip, nvars - 3);
                        backpos = nvars - 3;
                    }
                }
            }

            if backpos <= pos {
                break;
            }

            // Tighten next coefficients that, paired with the current small
            // coefficient, exceed the capacity.
            let mut maxweight = w(scip, pos);
            let startpos = pos;
            while 2 * maxweight > capacity && maxweight + sumcoef > capacity {
                consdata_chg_weight(scip, cons, pos, newweight);
                pos += 1;
                maxweight = w(scip, pos);

                if backpos <= pos {
                    break;
                }
            }
            *nchgcoefs += pos - startpos;

            // Skip unchangable weights.
            while pos < nvars && w(scip, pos) + sumcoef == capacity {
                pos += 1;
            }

            // Check special case were there is only one weight left to tighten
            //
            // e.g.  95x1 + 59x2 + 37x3 + 36x4 <= 95 (37 > 36)
            //
            //  =>  95x1 + 59x2 + 59x3 + 36x4 <= 95
            //
            //     197x1 + 120x2 + 77x3 + 10x4 <= 207 (here we cannot tighten the coefficient further)
            if pos + 1 == backpos && w(scip, pos) > sumcoef && ((k == 0) || (k == 1 && w(scip, nvars - 1) + sumcoef + w(scip, pos) > capacity)) {
                newweight = capacity - sumcoef;
                consdata_chg_weight(scip, cons, pos, newweight);
                break;
            }

            if backpos <= pos {
                break;
            }
        }
    }

    // Apply rule (2) (don't apply, if the knapsack has too many items for
    // applying this costly method).
    if (presoltiming & PRESOLTIMING_MEDIUM) != 0 {
        let c = cd(scip, cons);
        if hd(scip, conshdlr).disaggregation
            && scip.conss[cons].get_n_upgrade_locks() == 0
            && c.nvars - pos <= MAX_USECLIQUES_SIZE
            && c.nvars >= 2
            && pos > 0
            && (c.nvars - pos) as i64 <= c.capacity
            && c.weights[pos - 1] == c.capacity
            && (pos == c.nvars || c.weights[pos] == 1)
        {
            let flags = cons_flags(scip, cons);
            if pos == cd(scip, cons).nvars {
                let name = scip.conss[cons].name.clone();
                let vars = cd(scip, cons).vars.0[..pos as usize].to_vec();
                let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &vars, &flags)?;

                // Add the upgraded constraint to the problem.
                scip_cons::add_cons(scip, cliquecons)?;
                scip_cons::release_cons(scip, &mut cliquecons)?;
                *naddconss += 1;

                // Delete old constraint.
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;

                return Ok(false);
            }

            let len = (cd(scip, cons).nvars - pos) as usize;

            // Calculate clique partition.
            let rear = cd(scip, cons).vars.0[pos as usize..pos as usize + len].to_vec();
            let mut clqpart = vec![0i32; len];
            let mut map = std::mem::take(&mut hdm(scip, conshdlr).probtoidxmap);
            let mut mapsize = hd(scip, conshdlr).probtoidxmapsize;
            let nclq = scip_var::calc_clique_partition(scip, &rear, &mut map, &mut mapsize, &mut clqpart);
            let h = hdm(scip, conshdlr);
            h.probtoidxmap = map;
            h.probtoidxmapsize = mapsize;
            let nclq = nclq?;

            // Copy corresponding variables with big coefficients.
            let front = cd(scip, cons).vars.0[..pos as usize].to_vec();

            // Create for each clique a set-packing constraint.
            for c in 0..nclq {
                let mut clqvars = front.clone();
                for w in c as usize..len {
                    if clqpart[w] == c {
                        clqvars.push(rear[w]);
                    }
                }

                let name = format!("{}_clq_{}_{}", scip.conss[cons].name, cd(scip, cons).capacity, c);
                let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &clqvars, &flags)?;

                // Add the special constraint to the problem.
                scip_cons::add_cons(scip, cliquecons)?;
                scip_cons::release_cons(scip, &mut cliquecons)?;
                *naddconss += 1;
            }

            // Delete old constraint.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;

            return Ok(false);
        } else if c.nvars <= MAX_USECLIQUES_SIZE || (c.cliquepartitioned && c.ncliques <= MAX_USECLIQUES_SIZE) {
            let n = c.nvars as usize;
            let mut maxcliqueweights = vec![0i64; n];
            let mut newweightvals = vec![0i64; n];
            let mut newweightidxs = vec![0i32; n];

            // Repeat as long as changes have been applied.
            loop {
                // Sort items, s.t. the heaviest one is in the first position.
                with_cd(scip, cons, |scip, c| sort_items(scip, c));

                // Calculate a clique partition.
                let mut data = scip.conss[cons].consdata.take().unwrap();
                let r = calc_cliquepartition(scip, conshdlr, data.downcast_mut::<ConsData>().unwrap(), true, false);
                scip.conss[cons].consdata = Some(data);
                r?;

                // If there are only single element cliques, rule (2) is
                // equivalent to rule (1).
                let nv = cd(scip, cons).nvars;
                if cd(scip, cons).cliquepartition.as_ref().unwrap()[nv - 1] == nv - 1 {
                    break;
                }

                // Calculate the maximal weight of the cliques and store the clique
                // type.
                let mut cliqueweightsum: i64 = 0;
                let mut ncliques = 0;

                for i in 0..nv {
                    let cliquenum = cd(scip, cons).cliquepartition.as_ref().unwrap()[i];
                    let weight = cd(scip, cons).weights[i];

                    if cliquenum == ncliques {
                        maxcliqueweights[ncliques as usize] = weight;
                        cliqueweightsum += weight;
                        ncliques += 1;
                    }
                }

                // Apply rule on every clique.
                let mut zeroweights = false;
                for i in 0..ncliques {
                    let delta = cd(scip, cons).capacity - (cliqueweightsum - maxcliqueweights[i as usize]);
                    if delta > 0 {
                        let newcapacity = cd(scip, cons).capacity - delta;
                        let mut forceclique = false;
                        let mut nnewweights = 0usize;

                        for j in i..cd(scip, cons).nvars {
                            if cd(scip, cons).cliquepartition.as_ref().unwrap()[j] == i {
                                let mut newweight = cd(scip, cons).weights[j] - delta;
                                newweight = crate::def::max(newweight, 0);

                                // Cache the new weight.
                                newweightvals[nnewweights] = newweight;
                                newweightidxs[nnewweights] = j;
                                nnewweights += 1;
                            }
                        }

                        // Check if our clique information results out of this
                        // knapsack constraint and if so check if we would loose the
                        // clique information.
                        if nnewweights > 1 {
                            let mut newminweightsuminclique = newweightvals[nnewweights - 2];
                            newminweightsuminclique += newweightvals[nnewweights - 1];

                            // Check if these new two minimal weights both fit into the
                            // knapsack; if this is true, we have to add a clique
                            // constraint in order to enforce the clique (otherwise, the
                            // knapsack might have been one of the reasons for the
                            // clique, and the weight reduction might be infeasible,
                            // i.e., allows additional solutions).
                            if newminweightsuminclique <= newcapacity {
                                forceclique = true;
                            }
                        }

                        // Check if we really want to apply the change.
                        if hd(scip, conshdlr).disaggregation || !forceclique {
                            cdm(scip, cons).capacity = newcapacity;
                            *nchgsides += 1;

                            for k in 0..nnewweights {
                                let j = newweightidxs[k];

                                // Apply the weight change.
                                consdata_chg_weight(scip, cons, j, newweightvals[k]);
                                *nchgcoefs += 1;
                                zeroweights = zeroweights || (newweightvals[k] == 0);
                            }

                            // If before the weight update at least one pair of
                            // weights did not fit into the knapsack and now fits, we
                            // have to make sure, the clique is enforced - the clique
                            // might have been constructed partially from this
                            // constraint, and by reducing the weights, this clique
                            // information is not contained anymore in the knapsack
                            // constraint.
                            if forceclique {
                                let cliquevars: Vec<VarId> = (0..nnewweights).map(|k| cd(scip, cons).vars[newweightidxs[k]]).collect();
                                let name = format!("{}_clq_{}_{}", scip.conss[cons].name, cd(scip, cons).capacity, i);
                                let flags = cons_flags(scip, cons);
                                let mut cliquecons = crate::cons_setppc::create_cons_setpack(scip, &name, &cliquevars, &flags)?;

                                // Add the special constraint to the problem.
                                scip_cons::add_cons(scip, cliquecons)?;
                                scip_cons::release_cons(scip, &mut cliquecons)?;
                                *naddconss += 1;
                            }
                        }
                    }
                }
                if zeroweights {
                    remove_zero_weights(scip, cons)?;
                }

                let c = cd(scip, cons);
                if !(!c.sorted && c.weightsum > c.capacity) {
                    break;
                }
            }

            // Check for redundancy.
            if cd(scip, cons).weightsum <= cd(scip, cons).capacity {
                return Ok(false);
            }
        }
    }

    // Apply rule (3).
    if (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 && tighten_weights_lift(scip, cons, nchgcoefs)? {
        // C keeps going after a cutoff here, only reporting it.
        return tighten_weights_tail(scip, cons, presoltiming, nchgcoefs).map(|()| true);
    }

    tighten_weights_tail(scip, cons, presoltiming, nchgcoefs).map(|()| false)
}

/// Rules (4) and (5) of [`tighten_weights`], after its redundancy check.
fn tighten_weights_tail(scip: &mut Scip, cons: ConsId, presoltiming: PresolTiming, nchgcoefs: &mut i32) -> ScipResult {
    // Check for redundancy.
    if cd(scip, cons).weightsum <= cd(scip, cons).capacity {
        return Ok(());
    }

    if (presoltiming & PRESOLTIMING_FAST) != 0 {
        // Apply rule (4) (all but smallest weight).
        with_cd(scip, cons, |scip, c| sort_items(scip, c));
        let nvars = cd(scip, cons).nvars;
        let minweight = cd(scip, cons).weights[nvars - 1];
        for i in 0..nvars - 1 {
            let weight = cd(scip, cons).weights[i];
            let capacity = cd(scip, cons).capacity;
            if minweight + weight > capacity {
                if weight < capacity {
                    consdata_chg_weight(scip, cons, i, capacity); // this does not destroy the weight order!
                    cdm(scip, cons).sorted = true;
                    *nchgcoefs += 1;
                }
            } else {
                break;
            }
        }

        // Apply rule (5) (smallest weight).
        if cd(scip, cons).nvars >= 2 {
            let nvars = cd(scip, cons).nvars;
            let minweight = cd(scip, cons).weights[nvars - 2];
            let weight = cd(scip, cons).weights[nvars - 1];
            let capacity = cd(scip, cons).capacity;
            if minweight + weight > capacity && weight < capacity {
                consdata_chg_weight(scip, cons, nvars - 1, capacity); // this does not destroy the weight order!
                cdm(scip, cons).sorted = true;
                *nchgcoefs += 1;
            }
        }
    }

    Ok(())
}

/// `addNegatedCliques`: adds negated cliques of the knapsack constraint to
/// the global clique table. Returns whether the node can be cut off.
fn add_negated_cliques(scip: &mut Scip, cons: ConsId, nbdchgs: &mut i32) -> ScipResult<bool> {
    let nvars = cd(scip, cons).nvars;

    // Check whether the cliques have already been added.
    if cd(scip, cons).cliquesadded || nvars == 0 {
        return Ok(false);
    }

    // Make sure, the items are merged.
    if merge_multiples(scip, cons)? {
        return Ok(true);
    }

    // Make sure, items are sorted by non-increasing weight.
    with_cd(scip, cons, |scip, c| sort_items(scip, c));

    let conshdlr = scip.conss[cons].conshdlr;

    // Calculate a clique partition.
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let r = calc_cliquepartition(scip, conshdlr, data.downcast_mut::<ConsData>().unwrap(), false, true);
    scip.conss[cons].consdata = Some(data);
    r?;
    let mut nnegcliques = cd(scip, cons).nnegcliques;

    // If we have no negated cliques, stop.
    if nnegcliques == nvars {
        return Ok(false);
    }

    // Get temporary memory.
    let n = nvars as usize;
    let mut poscliquevars = vec![NONE; n];
    let mut cliquevars = vec![NONE; n];
    let mut gainweights = vec![0i64; n];
    let mut gaincliquepartition = vec![0i32; n];
    let mut maxweights = vec![0i64; nnegcliques as usize];
    let mut cliqueused = vec![false; nnegcliques as usize];

    nnegcliques = 0;
    let mut minactduetonegcliques: i64 = 0;

    // Determine maximal weights for all negated cliques and calculate minimal
    // weightsum due to negated cliques.
    for v in 0..nvars {
        let c = cd(scip, cons);
        let part = c.negcliquepartition.as_ref().unwrap()[v];
        if part == nnegcliques {
            nnegcliques += 1;
            maxweights[part as usize] = c.weights[v];
        } else {
            minactduetonegcliques += c.weights[v];
        }
    }

    let mut nposcliquevars = 0usize;

    // Add cliques, using negated cliques information.
    if minactduetonegcliques > 0 {
        // Free capacity is the rest of not used capacity if the smallest
        // amount of weights due to negated cliques are used.
        let freecapacity = cd(scip, cons).capacity - minactduetonegcliques;

        // Calculate possible gain by switching chosen items in negated cliques.
        {
            let c = cd(scip, cons);
            let part = c.negcliquepartition.as_ref().unwrap();
            for v in 0..nvars {
                if !cliqueused[part[v] as usize] {
                    cliqueused[part[v] as usize] = true;
                    for w in v + 1..nvars {
                        // If we would take the biggest weight instead of another what
                        // would we gain, take weight[v] instead of weight[w] (which are
                        // both in a negated clique).
                        if part[v] == part[w] && c.weights[v] > c.weights[w] {
                            poscliquevars[nposcliquevars] = c.vars[w];
                            gainweights[nposcliquevars] = maxweights[part[v] as usize] - c.weights[w];
                            gaincliquepartition[nposcliquevars] = part[v];
                            nposcliquevars += 1;
                        }
                    }
                }
            }
        }

        // Try to create negated cliques.
        if nposcliquevars > 0 {
            // Sort possible gain per substitution of the clique members.
            sort::sort_down(
                &mut gainweights[..nposcliquevars],
                (&mut poscliquevars[..nposcliquevars], &mut gaincliquepartition[..nposcliquevars]),
                sort::cmp_long,
            );

            for v in 0..nposcliquevars {
                cliquevars[0] = scip_var::get_negated_var(scip, poscliquevars[v])?;
                let mut ncliquevars = 1;
                let mut lastweight = gainweights[v];
                let mut beforelastweight = -1;
                let mut lastcliqueused = gaincliquepartition[v];

                // Clear cliqueused to get an unused array.
                cliqueused[..nnegcliques as usize].fill(false);
                cliqueused[gaincliquepartition[v] as usize] = true;

                // Taking bigger weights make the knapsack redundant so we will
                // create cliques, only take items which are not in the same negated
                // clique and by taking two of them would exceed the free capacity.
                let mut w = v + 1;
                while w < nposcliquevars && !cliqueused[gaincliquepartition[w] as usize] && gainweights[w] + lastweight > freecapacity {
                    beforelastweight = lastweight;
                    lastweight = gainweights[w];
                    lastcliqueused = gaincliquepartition[w];
                    cliqueused[gaincliquepartition[w] as usize] = true;
                    cliquevars[ncliquevars] = scip_var::get_negated_var(scip, poscliquevars[w])?;
                    ncliquevars += 1;
                    w += 1;
                }

                if ncliquevars > 1 {
                    // Add the clique to the clique table; this really happens,
                    // e.g., on enigma.mps from the short test set.
                    let (cutoff, thisnbdchgs) = scip_var::add_clique(scip, &cliquevars[..ncliquevars], &[], false)?;
                    if cutoff {
                        return Ok(true);
                    }

                    *nbdchgs += thisnbdchgs;

                    // Reset last used clique to get slightly different cliques.
                    cliqueused[lastcliqueused as usize] = false;

                    // Try to replace the last item in the clique by a different item
                    // to obtain a slightly different clique.
                    w += 1;
                    while w < nposcliquevars && !cliqueused[gaincliquepartition[w] as usize] && beforelastweight + gainweights[w] > freecapacity {
                        cliquevars[ncliquevars - 1] = scip_var::get_negated_var(scip, poscliquevars[w])?;
                        let (cutoff, thisnbdchgs) = scip_var::add_clique(scip, &cliquevars[..ncliquevars], &[], false)?;
                        if cutoff {
                            return Ok(true);
                        }
                        *nbdchgs += thisnbdchgs;
                        w += 1;
                    }
                }
            }
        }
    }

    Ok(false)
}

/// `greedyCliqueAlgorithm`: greedy clique detection by considering weights
/// and capacity
///
/// Greedily detects cliques by first sorting the items by decreasing weights
/// (optional) and then collecting greedily
/// 1) neighboring items which exceed the capacity together => one clique
/// 2) looping through the remaining items and finding the largest set of
///    preceding items to build a clique => possibly many more cliques
///
/// Returns whether the node can be cut off.
#[allow(clippy::too_many_arguments)]
fn greedy_clique_algorithm(
    scip: &mut Scip,
    items: &mut [VarId],
    weights: &mut [i64],
    capacity: i64,
    sorteditems: bool,
    cliqueextractfactor: f64,
    nbdchgs: &mut i32,
) -> ScipResult<bool> {
    let nitems = items.len();
    let mut cutoff = false;

    if nitems <= 1 {
        return Ok(false);
    }

    // Sort possible gain per substitution of the clique members.
    if !sorteditems {
        sort::sort_down(weights, (&mut *items,), sort::cmp_long);
    }

    let mut ncliquevars = 1usize;
    let mut lastweight = weights[0];

    // Taking these two weights together violates the knapsack => include into
    // clique.
    let mut i = 1;
    while i < nitems && weights[i] + lastweight > capacity {
        lastweight = weights[i];
        ncliquevars += 1;
        i += 1;
    }

    if ncliquevars > 1 {
        // Add the clique to the clique table.
        let (c, thisnbdchgs) = scip_var::add_clique(scip, &items[..ncliquevars], &[], false)?;
        if c {
            return Ok(true);
        }

        *nbdchgs += thisnbdchgs;
        let mut nnzadded = ncliquevars;

        // No more cliques to be found (don't know if this can actually happen,
        // since the knapsack could be replaced by a set-packing constraint).
        if ncliquevars == nitems {
            return Ok(false);
        }

        // Copy items in order into buffer array and deduce more cliques.
        let mut cliquevars = items[..ncliquevars].to_vec();

        // Try to replace the last item in the clique by a different item to
        // obtain a slightly different clique; loop over remaining, smaller
        // items and compare each item backwards against larger weights,
        // starting with the second smallest weight.
        let mut compareweightidx = ncliquevars as i32 - 2;

        // Determine minimum clique size for the following loop.
        let mut minclqsize = (cliqueextractfactor * ncliquevars as f64) as i32;
        minclqsize = crate::def::max(minclqsize, 2);

        // Loop over the remaining variables and the larger items of the first
        // clique until we find another clique or reach the size limit.
        while compareweightidx >= 0
            && i < nitems
            && !cutoff
            && ncliquevars as i32 >= minclqsize // stop at a given minimum clique size
            && nnzadded <= 2 * nitems
        // stop if enough nonzeros were added to the cliquetable
        {
            let compareweight = weights[compareweightidx as usize];

            // Include this item together with all items that have a weight at
            // least as large as the compare weight in a clique.
            if compareweight + weights[i] > capacity {
                cliquevars[ncliquevars - 1] = items[i];
                let (c, thisnbdchgs) = scip_var::add_clique(scip, &cliquevars[..ncliquevars], &[], false)?;
                cutoff = c;

                nnzadded += ncliquevars;

                // Stop when there is a cutoff.
                if !cutoff {
                    *nbdchgs += thisnbdchgs;
                }

                // Go to next smaller item.
                i += 1;
            } else {
                // Choose a preceding, larger weight to compare small items against.
                // Clique size is reduced by 1 simultaneously.
                compareweightidx -= 1;
                ncliquevars -= 1;
            }
        }
    }

    Ok(cutoff)
}

/// `addCliques`: adds cliques of the knapsack constraint to the global
/// clique table. Returns whether the node can be cut off.
fn add_cliques(scip: &mut Scip, cons: ConsId, cliqueextractfactor: f64, nbdchgs: &mut i32) -> ScipResult<bool> {
    let nvars = cd(scip, cons).nvars;

    // Check whether the cliques have already been added.
    if cd(scip, cons).cliquesadded || nvars == 0 {
        return Ok(false);
    }

    // Make sure, the items are merged.
    if merge_multiples(scip, cons)? {
        return Ok(true);
    }

    // Make sure, the items are sorted by non-increasing weight.
    with_cd(scip, cons, |scip, c| sort_items(scip, c));

    let conshdlr = scip.conss[cons].conshdlr;

    // Calculate a clique partition.
    let mut data = scip.conss[cons].consdata.take().unwrap();
    let r = calc_cliquepartition(scip, conshdlr, data.downcast_mut::<ConsData>().unwrap(), false, true);
    scip.conss[cons].consdata = Some(data);
    r?;
    let mut nnegcliques = cd(scip, cons).nnegcliques;

    let cutoff = add_cliques_body(scip, cons, cliqueextractfactor, nbdchgs, nvars, &mut nnegcliques)?;

    // Mark the constraint.
    cdm(scip, cons).cliquesadded = true;

    Ok(cutoff)
}

/// The part of `addCliques` that ends at its `TERMINATE` label.
fn add_cliques_body(scip: &mut Scip, cons: ConsId, cliqueextractfactor: f64, nbdchgs: &mut i32, nvars: i32, nnegcliques: &mut i32) -> ScipResult<bool> {
    // Get temporary memory.
    let n = nvars as usize;
    let mut poscliquevars = vec![NONE; n];
    let mut gainweights = vec![0i64; n];
    let mut secondmaxweights = vec![0i64; *nnegcliques as usize];

    let mut minactduetonegcliques: i64 = 0;

    // Calculate minimal activity due to negated cliques, and determine second
    // maximal weight in each clique.
    if *nnegcliques < nvars {
        *nnegcliques = 0;

        let c = cd(scip, cons);
        for i in 0..nvars {
            let cliquenum = c.negcliquepartition.as_ref().unwrap()[i];
            let weight = c.weights[i];

            if cliquenum == *nnegcliques {
                *nnegcliques += 1;
            } else {
                minactduetonegcliques += weight;
                if secondmaxweights[cliquenum as usize] == 0 {
                    secondmaxweights[cliquenum as usize] = weight;
                }
            }
        }
    }

    // Add cliques, using negated cliques information.
    if minactduetonegcliques > 0 {
        // Free capacity is the rest of not used capacity if the smallest
        // amount of weights due to negated cliques are used.
        let freecapacity = cd(scip, cons).capacity - minactduetonegcliques;

        // Create negated cliques out of negated cliques, if we do not take the
        // smallest weight of a cliques ...
        if add_negated_cliques(scip, cons, nbdchgs)? {
            return Ok(true);
        }

        let mut nposcliquevars = 0usize;

        for i in (0..nvars).rev() {
            // If we would take the biggest weight instead of the second biggest.
            let c = cd(scip, cons);
            let cliquenum = c.negcliquepartition.as_ref().unwrap()[i];
            if c.weights[i] > secondmaxweights[cliquenum as usize] {
                poscliquevars[nposcliquevars] = c.vars[i];
                gainweights[nposcliquevars] = c.weights[i] - secondmaxweights[cliquenum as usize];
                nposcliquevars += 1;
            }
        }

        // Use the gain weights and free capacity to derive greedily cliques.
        if nposcliquevars > 1
            && greedy_clique_algorithm(
                scip,
                &mut poscliquevars[..nposcliquevars],
                &mut gainweights[..nposcliquevars],
                freecapacity,
                false,
                cliqueextractfactor,
                nbdchgs,
            )?
        {
            return Ok(true);
        }
    }

    // Build cliques by using the items with the maximal weights (they are
    // sorted, so the call leaves them in place).
    let capacity = cd(scip, cons).capacity;
    let mut vars = cd(scip, cons).vars.0[..n].to_vec();
    let mut weights = cd(scip, cons).weights.0[..n].to_vec();
    greedy_clique_algorithm(scip, &mut vars, &mut weights, capacity, true, cliqueextractfactor, nbdchgs)
}

/// The knapsack hash table callbacks (`hashGetKeyKnapsackcons`,
/// `hashKeyEqKnapsackcons`, `hashKeyValKnapsackcons`). C's key value sorts
/// the constraint's items first; the hash table keeps the hash of every
/// stored element, so the callers sort the queried constraint beforehand
/// with [`sort_for_hash`] instead.
struct KnapsackconsHash<'a> {
    scip: &'a Scip,
}

impl crate::hash::HashOps<ConsId> for KnapsackconsHash<'_> {
    type Key = ConsId;

    // The key is the element itself.
    fn get_key(&self, element: &ConsId) -> ConsId {
        *element
    }

    // Returns TRUE iff both keys are equal; two constraints are equal if they
    // have the same variables and the same coefficients.
    fn key_eq(&self, key1: &ConsId, key2: &ConsId) -> bool {
        let consdata1 = cd(self.scip, *key1);
        let consdata2 = cd(self.scip, *key2);

        // Checks trivial case.
        if consdata1.nvars != consdata2.nvars {
            return false;
        }

        for i in (0..consdata1.nvars).rev() {
            // Tests if variables are equal.
            if consdata1.vars[i] != consdata2.vars[i] {
                return false;
            }

            // Tests if weights are equal too.
            if consdata1.weights[i] != consdata2.weights[i] {
                return false;
            }
        }

        true
    }

    // Returns the hash value of the key.
    fn key_val(&self, key: &ConsId) -> u64 {
        let c = cd(self.scip, *key);
        let minidx = self.scip.vars[c.vars[0usize]].index;
        let mididx = self.scip.vars[c.vars[c.nvars / 2]].index;
        let maxidx = self.scip.vars[c.vars[c.nvars - 1]].index;

        // Hash value depends on vectors of variable indices.
        let firstweight = c.weights[0usize] as u64;
        crate::misc::hash_six(c.nvars as u32, minidx as u32, mididx as u32, maxidx as u32, (firstweight >> 32) as u32, firstweight as u32) as u64
    }
}

/// The `sortItems` call at the head of `hashKeyValKnapsackcons`.
fn sort_for_hash(scip: &mut Scip, cons: ConsId) {
    with_cd(scip, cons, |scip, c| sort_items(scip, c));
}

/// `detectRedundantConstraints`: compares each constraint with all other
/// constraints for possible redundancy and removes or changes constraint
/// accordingly; in contrast to preprocessConstraintPairs(), it uses a hash
/// table. Returns whether the problem is infeasible.
fn detect_redundant_constraints(scip: &mut Scip, conss: &[ConsId], ndelconss: &mut i32) -> ScipResult<bool> {
    // Create a hash table for the constraint set.
    let mut hashtablesize = conss.len() as i32;
    hashtablesize = crate::def::max(hashtablesize, HASHSIZE_KNAPSACKCONS);
    let mut hashtable = crate::hash::HashTable::<ConsId>::new(hashtablesize);

    // Check all constraints in the given set for redundancy.
    for &cons0 in conss.iter().rev() {
        if !scip.conss[cons0].is_active() || scip.conss[cons0].modifiable {
            continue;
        }

        if cd(scip, cons0).nvars == 0 {
            if cd(scip, cons0).capacity < 0 {
                return Ok(true);
            } else {
                scip_cons::del_cons(scip, cons0)?;
                *ndelconss += 1;
                continue;
            }
        }

        // Get constraint from current hash table with same variables and same
        // weights as cons0.
        sort_for_hash(scip, cons0);
        let cons1 = hashtable.retrieve(&KnapsackconsHash { scip }, &cons0);

        if let Some(cons1) = cons1 {
            // Constraint found: create a new constraint with same coefficients and
            // best left and right hand side; delete old constraints afterwards.

            // Check which constraint has to stay.
            let (consstay, consdel) = if cd(scip, cons0).capacity < cd(scip, cons1).capacity {
                // Exchange consdel with consstay in hashtable.
                sort_for_hash(scip, cons1);
                hashtable.remove(&KnapsackconsHash { scip }, &cons1);
                hashtable.insert(&KnapsackconsHash { scip }, cons0);
                (cons0, cons1)
            } else {
                (cons1, cons0)
            };

            // Update flags of constraint which caused the redundancy s.t.
            // nonredundant information doesn't get lost.
            scip_cons::update_cons_flags(scip, consstay, consdel)?;

            // Delete consdel.
            scip_cons::del_cons(scip, consdel)?;
            *ndelconss += 1;
        } else {
            // No such constraint in current hash table: insert cons0 into hash
            // table.
            hashtable.insert(&KnapsackconsHash { scip }, cons0);
        }
    }

    Ok(false)
}

/// `preprocessConstraintPairs`: compares constraint with all prior
/// constraints for possible redundancy or aggregation, and removes or
/// changes constraint accordingly.
fn preprocess_constraint_pairs(scip: &mut Scip, conss: &[ConsId], firstchange: i32, chkind: i32, ndelconss: &mut i32) -> ScipResult {
    // Get the constraint to be checked against all prior constraints.
    let cons0 = conss[chkind as usize];

    // Sort the constraint.
    sort_for_hash(scip, cons0);

    // See #2970.
    if cd(scip, cons0).capacity == 0 {
        return Ok(());
    }

    // Check constraint against all prior constraints.
    let start = if cd(scip, cons0).presolvedtiming == PRESOLTIMING_EXHAUSTIVE { firstchange } else { 0 };
    for c in start..chkind {
        let cons1 = conss[c as usize];
        if !scip.conss[cons1].is_active() || scip.conss[cons1].modifiable {
            continue;
        }

        // If both constraints didn't change since last pair processing, we can
        // ignore the pair.
        if cd(scip, cons0).presolvedtiming >= PRESOLTIMING_EXHAUSTIVE && cd(scip, cons1).presolvedtiming >= PRESOLTIMING_EXHAUSTIVE {
            continue;
        }

        // Sort the constraint.
        sort_for_hash(scip, cons1);

        // See #2970.
        if cd(scip, cons1).capacity == 0 {
            continue;
        }

        let consdata0 = cd(scip, cons0);
        let consdata1 = cd(scip, cons1);
        let quotient = (consdata0.capacity as f64) / (consdata1.capacity as f64);

        let mut iscons0incons1contained;
        let mut iscons1incons0contained;
        let mut v;
        if consdata0.nvars > consdata1.nvars {
            iscons0incons1contained = false;
            iscons1incons0contained = true;
            v = consdata1.nvars - 1;
        } else if consdata0.nvars < consdata1.nvars {
            iscons0incons1contained = true;
            iscons1incons0contained = false;
            v = consdata0.nvars - 1;
        } else {
            iscons0incons1contained = true;
            iscons1incons0contained = true;
            v = consdata0.nvars - 1;
        }

        // Check consdata0 against consdata1:
        // 1. if all variables var_i of cons1 are in cons0 and for each of these
        //    variables (consdata0->weights[i] / quotient) >= consdata1->weights[i]
        //    cons1 is redundant
        // 2. if all variables var_i of cons0 are in cons1 and for each of these
        //    variables (consdata0->weights[i] / quotient) <= consdata1->weights[i]
        //    cons0 is redundant
        let mut v0 = consdata0.nvars - 1;
        let mut v1 = consdata1.nvars - 1;

        while v >= 0 {
            // Now there are more variables in cons1 left.
            if v1 > v0 {
                iscons1incons0contained = false;
                if !iscons0incons1contained {
                    break;
                }
            }
            // Now there are more variables in cons0 left.
            else if v1 < v0 {
                iscons0incons1contained = false;
                if !iscons1incons0contained {
                    break;
                }
            }

            // Both variables are the same.
            if consdata0.vars[v0] == consdata1.vars[v1] {
                // If cons1 is possible contained in cons0 (consdata0->weights[v0] /
                // quotient) must be greater equals consdata1->weights[v1].
                if iscons1incons0contained && scip.set.is_lt((consdata0.weights[v0] as f64) / quotient, consdata1.weights[v1] as f64) {
                    iscons1incons0contained = false;
                    if !iscons0incons1contained {
                        break;
                    }
                }
                // If cons0 is possible contained in cons1 (consdata0->weight[v0] /
                // quotient) must be less equals consdata1->weight[v1].
                else if iscons0incons1contained && scip.set.is_gt((consdata0.weights[v0] as f64) / quotient, consdata1.weights[v1] as f64) {
                    iscons0incons1contained = false;
                    if !iscons1incons0contained {
                        break;
                    }
                }
                v0 -= 1;
                v1 -= 1;
                v -= 1;
            } else {
                // Both constraints have a variables which is not part of the other
                // constraint, so stop.
                if iscons0incons1contained && iscons1incons0contained {
                    iscons0incons1contained = false;
                    iscons1incons0contained = false;
                    break;
                }

                // Continue to the next variable.
                if iscons0incons1contained {
                    v1 -= 1;
                } else {
                    v0 -= 1;
                }
            }
        }

        // Neither one constraint was contained in another or we checked all
        // variables of one constraint against the other.
        if iscons1incons0contained {
            // Update flags of constraint which caused the redundancy s.t.
            // nonredundant information doesn't get lost.
            scip_cons::update_cons_flags(scip, cons0, cons1)?;
            scip_cons::del_cons(scip, cons1)?;
            *ndelconss += 1;
        } else if iscons0incons1contained {
            // Update flags of constraint which caused the redundancy s.t.
            // nonredundant information doesn't get lost.
            scip_cons::update_cons_flags(scip, cons1, cons0)?;
            scip_cons::del_cons(scip, cons0)?;
            *ndelconss += 1;
            break;
        }
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce constraints.
fn enforce_constraint(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let mut cutoff = false;
    let mut ncuts = 0;

    *result = ScipResultCode::Feasible;

    // Get maximal number of cuts per round.
    let h = hd(scip, conshdlr);
    let maxncuts = if api::get_depth(scip) == 0 { h.maxsepacutsroot } else { h.maxsepacuts };

    // Search for violated useful knapsack constraints.
    let mut i = 0;
    while i < nusefulconss as usize && ncuts < maxncuts && !cutoff {
        let violated = check_cons(scip, conss[i], sol, false, false)?;
        if violated {
            // Add knapsack constraint as LP row to the relaxation.
            cutoff = add_relaxation(scip, conss[i])?;
            ncuts += 1;
        }
        i += 1;
    }

    // As long as no violations were found, search for violated obsolete
    // knapsack constraints.
    let mut i = nusefulconss as usize;
    while i < conss.len() && ncuts == 0 && !cutoff {
        let violated = check_cons(scip, conss[i], sol, false, false)?;
        if violated {
            // Add knapsack constraint as LP row to the relaxation.
            cutoff = add_relaxation(scip, conss[i])?;
            ncuts += 1;
        }
        i += 1;
    }

    // Adjust the result code.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/*
 * Linear constraint upgrading
 */

/// `createNormalizedKnapsack`: creates and captures a knapsack constraint
/// out of a linear inequality.
fn create_normalized_knapsack(scip: &mut Scip, name: &str, vars: &[VarId], vals: &[f64], lhs: f64, rhs: f64, flags: &ConsFlags) -> ScipResult<ConsId> {
    let nvars = vars.len();

    // Get temporary memory.
    let mut transvars = vec![NONE; nvars];
    let mut weights = vec![0i64; nvars];

    // If the right hand side is non-infinite, we have to negate all variables
    // with negative coefficient; otherwise, we have to negate all variables
    // with positive coefficient and multiply the row with -1.
    let mult: i64;
    let mut capacity: i64;
    if scip.set.is_infinity(rhs) {
        mult = -1;
        capacity = scip.set.feas_floor(-lhs) as i64;
    } else {
        mult = 1;
        capacity = scip.set.feas_floor(rhs) as i64;
    }

    // Negate positive or negative variables.
    for v in 0..nvars {
        let weight = mult * scip.set.feas_floor(vals[v]) as i64;
        if weight > 0 {
            transvars[v] = vars[v];
            weights[v] = weight;
        } else {
            transvars[v] = scip_var::get_negated_var(scip, vars[v])?;
            weights[v] = -weight;
            capacity -= weight;
        }
    }

    // Create the constraint.
    create_cons_knapsack(scip, name, &transvars, &weights, capacity, flags)
}

/// `linconsUpgdKnapsack`: tries to upgrade a linear constraint into a
/// knapsack constraint.
fn lincons_upgd_knapsack(scip: &mut Scip, cons: ConsId, s: &crate::cons_linear::LinConsStats, upgdcons: &mut ConsId) -> ScipResult {
    let nvars = s.vars.len() as i32;

    // Check, if linear constraint can be upgraded to a knapsack constraint
    // - all variables must be binary
    // - all coefficients must be integral
    // - exactly one of the sides must be infinite
    // note that this includes the case of negative capacity, which has been
    // observed to occur, e.g., when upgrading a conflict constraint.
    let upgrade = (s.nposbin + s.nnegbin + s.nposimplbin + s.nnegimplbin == nvars)
        && (s.ncoeffspone + s.ncoeffsnone + s.ncoeffspint + s.ncoeffsnint == nvars)
        && (scip.set.is_infinity(-s.lhs) != scip.set.is_infinity(s.rhs));

    if upgrade {
        // Create the knapsack constraint (an automatically upgraded constraint is
        // always unmodifiable).
        let name = scip.conss[cons].name.clone();
        let flags = cons_flags(scip, cons);
        *upgdcons = create_normalized_knapsack(scip, &name, &s.vars, &s.vals, s.lhs, s.rhs, &flags)?;
    }

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `consFreeKnapsack`: destructor of constraint handler to free constraint
/// handler data (called when SCIP is exiting).
fn cons_free_knapsack(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    // Free constraint handler data.
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consInitKnapsack`: initialization method of constraint handler (called
/// after problem was transformed).
fn cons_init_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    // All variables which are of integral type can be binary; this can be
    // checked via the method SCIPvarIsBinary(var).
    let nvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip);

    let h = hdm(scip, conshdlr);
    h.reals1 = CVec(vec![0.0; nvars as usize]);
    h.reals1size = nvars;

    Ok(())
}

/// `consExitKnapsack`: deinitialization method of constraint handler
/// (called before transformed problem is freed).
fn cons_exit_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    let h = hdm(scip, conshdlr);
    h.reals1 = CVec::default();
    h.reals1size = 0;

    Ok(())
}

/// `consInitpreKnapsack`: presolving initialization method of constraint
/// handler (called when presolving is about to begin).
fn cons_initpre_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    // All variables which are of integral type can be binary; this can be
    // checked via the method SCIPvarIsBinary(var).
    let nvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip);
    let n = nvars as usize;

    let h = hdm(scip, conshdlr);
    h.ints1 = CVec(vec![0; n]);
    h.ints2 = CVec(vec![0; n]);
    h.longints1 = CVec(vec![0; n]);
    h.longints2 = CVec(vec![0; n]);
    h.bools1 = CVec(vec![false; n]);
    h.bools2 = CVec(vec![false; n]);
    h.bools3 = CVec(vec![false; n]);
    h.bools4 = CVec(vec![false; n]);
    h.ints1size = nvars;
    h.ints2size = nvars;
    h.longints1size = nvars;
    h.longints2size = nvars;
    h.bools1size = nvars;
    h.bools2size = nvars;
    h.bools3size = nvars;
    h.bools4size = nvars;

    Ok(())
}

/// `consExitpreKnapsack`: presolving deinitialization method of constraint
/// handler (called after presolving has been finished).
fn cons_exitpre_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    for &cons in conss {
        if !scip.conss[cons].deleted {
            // Since we are not allowed to detect infeasibility in the exitpre
            // stage, we dont give an infeasible pointer.
            apply_fixings(scip, cons, false)?;
        }
    }

    let h = hdm(scip, conshdlr);
    h.ints1 = CVec::default();
    h.ints2 = CVec::default();
    h.longints1 = CVec::default();
    h.longints2 = CVec::default();
    h.bools1 = CVec::default();
    h.bools2 = CVec::default();
    h.bools3 = CVec::default();
    h.bools4 = CVec::default();
    h.ints1size = 0;
    h.ints2size = 0;
    h.longints1size = 0;
    h.longints2size = 0;
    h.bools1size = 0;
    h.bools2size = 0;
    h.bools3size = 0;
    h.bools4size = 0;

    Ok(())
}

/// `consInitsolKnapsack`: solving process initialization method of
/// constraint handler; adds the nlrows to the NLP if it had been
/// constructed, which the port never does (the NLP is not ported).
fn cons_initsol_knapsack(_scip: &mut Scip, _conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    Ok(())
}

/// `consExitsolKnapsack`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_knapsack(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], _restart: bool) -> ScipResult {
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

/// `consDeleteKnapsack`: frees specific constraint data.
fn cons_delete_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    // Get event handler.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Free knapsack constraint.
    scip.conss[cons].consdata = consdata.take();
    let r = consdata_free(scip, cons, eventhdlr);
    scip.conss[cons].consdata = None;
    r
}

/// `consTransKnapsack`: transforms constraint data into data belonging to
/// the transformed problem.
fn cons_trans_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let s = cd(scip, sourcecons);
    let n = s.nvars as usize;
    let vars = s.vars.0[..n].to_vec();
    let weights = s.weights.0[..n].to_vec();
    let capacity = s.capacity;

    // Get event handler.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Create target constraint data.
    let targetdata = consdata_create(scip, &vars, &weights, capacity)?;

    // Create target constraint.
    let name = scip.conss[sourcecons].name.clone();
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    // Catch events for variables.
    catch_events(scip, *targetcons, eventhdlr)
}

/// `consInitlpKnapsack`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
fn cons_initlp_knapsack(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let mut i = 0;
    while i < conss.len() && !*infeasible {
        *infeasible = add_relaxation(scip, conss[i])?;
        i += 1;
    }

    Ok(())
}

/// `consSepalpKnapsack`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    let depth = api::get_depth(scip);
    let nrounds = api::get_n_sepa_rounds(scip);

    // Only call the separator a given number of times at each node.
    let h = hd(scip, conshdlr);
    if (depth == 0 && h.maxroundsroot >= 0 && nrounds >= h.maxroundsroot) || (depth > 0 && h.maxrounds >= 0 && nrounds >= h.maxrounds) {
        return Ok(());
    }

    // Check, if we should additionally separate knapsack cuts.
    let sepafreq = scip.conshdlrs[conshdlr].sepafreq;
    let sepacardfreq = sepafreq * h.sepacardfreq;
    let mut sepacardinality = (h.sepacardfreq >= 0) && ((sepacardfreq == 0 && depth == 0) || (sepacardfreq >= 1 && (depth % sepacardfreq == 0)));

    // Check dual bound to see if we want to produce knapsack cuts at this node.
    let loclowerbound = api::get_local_lowerbound(scip);
    let glblowerbound = api::get_lowerbound(scip);
    let cutoffbound = api::get_cutoffbound(scip);
    let maxbound = glblowerbound + h.maxcardbounddist * (cutoffbound - glblowerbound);

    // Get the maximal number of cuts allowed in a separation round.
    let maxsepacuts = if depth == 0 { h.maxsepacutsroot } else { h.maxsepacuts };
    let usegubs = h.usegubs;

    sepacardinality = sepacardinality && scip.set.is_le(loclowerbound, maxbound);
    sepacardinality = sepacardinality && (api::get_n_lp_branch_cands(scip) > 0);

    *result = ScipResultCode::DidNotFind;
    let mut ncuts = 0;
    let mut cutoff = false;

    // Separate useful constraints.
    let mut i = 0;
    while i < nusefulconss as usize && ncuts < maxsepacuts && !crate::solve::solve_is_stopped(scip, false) {
        separate_cons(scip, conss[i], NONE, sepacardinality, usegubs, &mut cutoff, &mut ncuts)?;
        i += 1;
    }

    // Adjust return value.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/// `consSepasolKnapsack`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    let depth = api::get_depth(scip);
    let nrounds = api::get_n_sepa_rounds(scip);

    // Only call the separator a given number of times at each node.
    let h = hd(scip, conshdlr);
    if (depth == 0 && h.maxroundsroot >= 0 && nrounds >= h.maxroundsroot) || (depth > 0 && h.maxrounds >= 0 && nrounds >= h.maxrounds) {
        return Ok(());
    }

    // Check, if we should additionally separate knapsack cuts.
    let sepafreq = scip.conshdlrs[conshdlr].sepafreq;
    let sepacardfreq = sepafreq * h.sepacardfreq;
    let sepacardinality = (h.sepacardfreq >= 0) && ((sepacardfreq == 0 && depth == 0) || (sepacardfreq >= 1 && (depth % sepacardfreq == 0)));

    // Get the maximal number of cuts allowed in a separation round.
    let maxsepacuts = if depth == 0 { h.maxsepacutsroot } else { h.maxsepacuts };
    let usegubs = h.usegubs;

    *result = ScipResultCode::DidNotFind;
    let mut ncuts = 0;
    let mut cutoff = false;

    // Separate useful constraints.
    let mut i = 0;
    while i < nusefulconss as usize && ncuts < maxsepacuts && !crate::solve::solve_is_stopped(scip, false) {
        separate_cons(scip, conss[i], sol, sepacardinality, usegubs, &mut cutoff, &mut ncuts)?;
        i += 1;
    }

    // Adjust return value.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/// `consEnfolpKnapsack`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_knapsack(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, nusefulconss, NONE, result)
}

/// `consEnforelaxKnapsack`: constraint enforcing method of constraint
/// handler for relaxation solutions.
fn cons_enforelax_knapsack(
    scip: &mut Scip,
    sol: SolId,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, nusefulconss, sol, result)
}

/// `consEnfopsKnapsack`: constraint enforcing method of constraint handler
/// for pseudo solutions.
#[allow(clippy::too_many_arguments)]
fn cons_enfops_knapsack(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    for &cons in conss {
        let violated = check_cons(scip, cons, NONE, true, false)?;
        if violated {
            *result = ScipResultCode::Infeasible;
            return Ok(());
        }
    }
    *result = ScipResultCode::Feasible;

    Ok(())
}

/// `consCheckKnapsack`: feasibility check method of constraint handler for
/// integral solutions.
#[allow(clippy::too_many_arguments)]
fn cons_check_knapsack(
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
        let violated = check_cons(scip, conss[i], sol, checklprows, printreason)?;
        if violated {
            *result = ScipResultCode::Infeasible;
        }
        i += 1;
    }

    Ok(())
}

/// `consPropKnapsack`: domain propagation method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_prop_knapsack(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut cutoff = false;
    let mut nfixedvars = 0;

    let negatedclique = hd(scip, conshdlr).negatedclique;
    let inpresolve = scip.set.stage < Stage::InitSolve;

    // Process useful constraints.
    let mut i = 0;
    while i < nmarkedconss as usize && !cutoff {
        // Do not propagate constraints with multi-aggregated variables, which
        // should only happen in probing mode, otherwise the multi-aggregation
        // should be resolved.
        if inpresolve && cd(scip, conss[i]).existmultaggr {
            i += 1;
            continue;
        }

        (cutoff, _) = propagate_cons(scip, conss[i], &mut nfixedvars, negatedclique)?;

        // Unmark the constraint to be propagated.
        cons_unmark_propagate(scip, conss[i]);
        i += 1;
    }

    // Adjust result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if nfixedvars > 0 {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consPresolKnapsack`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_knapsack(
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

    // Remember old preprocessing counters.
    let mut cutoff = false;
    let oldnfixedvars = counts.nfixedvars;
    let oldnchgbds = counts.nchgbds;
    let mut oldndelconss = counts.ndelconss;
    let oldnaddconss = counts.naddconss;
    let mut oldnchgcoefs = counts.nchgcoefs;
    let mut oldnchgsides = counts.nchgsides;
    let mut firstchange = i32::MAX;

    let newchanges = nrounds == 0 || newcounts.nfixedvars > 0 || newcounts.naggrvars > 0 || newcounts.nchgbds > 0 || newcounts.nupgdconss > 0;

    let (cliqueextractfactor, simplifyinequalities, dualpresolving, presolusehashing, presolpairwise) = {
        let h = hd(scip, conshdlr);
        (h.cliqueextractfactor, h.simplifyinequalities, h.dualpresolving, h.presolusehashing, h.presolpairwise)
    };

    let mut c = 0;
    while c < nconss && !crate::solve::solve_is_stopped(scip, false) {
        let cons = conss[c as usize];

        // Update data structures; todo if UBTIGHTENED events were caught, we
        // could move this block after the continue.
        if newchanges || counts.nfixedvars > oldnfixedvars || counts.nchgbds > oldnchgbds {
            cutoff = apply_fixings(scip, cons, true)?;
            if cutoff {
                break;
            }
        }

        // Force presolving the constraint in the initial round.
        if nrounds == 0 {
            cdm(scip, cons).presolvedtiming = 0;
        } else if cd(scip, cons).presolvedtiming >= presoltiming {
            c += 1;
            continue;
        }

        cdm(scip, cons).presolvedtiming = presoltiming;

        let mut thisnfixedvars = counts.nfixedvars;
        let thisnchgbds = counts.nchgbds;

        // Merge constraint, so propagation works better.
        cutoff = merge_multiples(scip, cons)?;
        if cutoff {
            break;
        }

        // Add cliques in the knapsack to the clique table.
        if (presoltiming & PRESOLTIMING_MEDIUM) != 0 {
            cutoff = add_cliques(scip, cons, cliqueextractfactor, &mut counts.nchgbds)?;
            if cutoff {
                break;
            }
        }

        // Propagate constraint.
        if presoltiming < PRESOLTIMING_EXHAUSTIVE {
            let redundant;
            (cutoff, redundant) = propagate_cons(scip, cons, &mut counts.nfixedvars, (presoltiming & PRESOLTIMING_MEDIUM) != 0)?;

            if cutoff {
                break;
            }
            if redundant {
                counts.ndelconss += 1;
                c += 1;
                continue;
            }
        }

        // Remove again all fixed variables, if further fixings were found.
        if counts.nfixedvars > thisnfixedvars || counts.nchgbds > thisnchgbds {
            cutoff = apply_fixings(scip, cons, true)?;
            if cutoff {
                break;
            }

            thisnfixedvars = counts.nfixedvars;
        }

        if !scip.conss[cons].modifiable {
            // Check again for redundancy (applyFixings() might have decreased
            // weightsum due to fixed-to-zero vars).
            if cd(scip, cons).weightsum <= cd(scip, cons).capacity {
                scip_cons::del_cons_local(scip, cons)?;
                c += 1;
                continue;
            }

            // Divide weights by their greatest common divisor.
            normalize_weights(scip, cons, &mut counts.nchgcoefs, &mut counts.nchgsides);

            // Try to simplify inequalities.
            if simplifyinequalities && (presoltiming & PRESOLTIMING_FAST) != 0 {
                cutoff = simplify_inequalities(
                    scip,
                    cons,
                    &mut counts.nfixedvars,
                    &mut counts.ndelconss,
                    &mut counts.nchgcoefs,
                    &mut counts.nchgsides,
                    &mut counts.naddconss,
                )?;
                if cutoff {
                    break;
                }

                if scip.conss[cons].deleted {
                    c += 1;
                    continue;
                }

                // Remove again all fixed variables, if further fixings were found.
                if counts.nfixedvars > thisnfixedvars {
                    cutoff = apply_fixings(scip, cons, true)?;
                    if cutoff {
                        break;
                    }
                }
            }

            // Tighten capacity and weights.
            cutoff = tighten_weights(
                scip,
                cons,
                presoltiming,
                &mut counts.nchgcoefs,
                &mut counts.nchgsides,
                &mut counts.naddconss,
                &mut counts.ndelconss,
            )?;
            if cutoff {
                break;
            }

            if scip.conss[cons].is_active() {
                if dualpresolving && scip_var::allow_strong_dual_reds(scip) && (presoltiming & PRESOLTIMING_MEDIUM) != 0 {
                    // In case the knapsack constraints is independent of everything
                    // else, solve the knapsack and apply the dual reduction.
                    let redundant = dual_presolving(scip, cons, &mut counts.nchgbds, &mut counts.ndelconss)?;
                    if redundant {
                        c += 1;
                        continue;
                    }
                }

                // Check if knapsack constraint is parallel to objective function.
                check_parallel_objective(scip, cons)?;
            }
        }

        // Remember the first changed constraint to begin the next aggregation
        // round with.
        if firstchange == i32::MAX && cd(scip, cons).presolvedtiming != PRESOLTIMING_EXHAUSTIVE {
            firstchange = c;
        }
        c += 1;
    }

    // Preprocess pairs of knapsack constraints.
    if !cutoff && presolusehashing && (presoltiming & PRESOLTIMING_MEDIUM) != 0 {
        // Detect redundant constraints; fast version with hash table instead of
        // pairwise comparison.
        cutoff = detect_redundant_constraints(scip, conss, &mut counts.ndelconss)?;
    }

    let mut success = (counts.ndelconss != oldndelconss) || (counts.nchgsides != oldnchgsides) || (counts.nchgcoefs != oldnchgcoefs) || (counts.naddconss != oldnaddconss);

    if !cutoff && firstchange < nconss && presolpairwise && (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
        let mut npaircomparisons: i64 = 0;
        oldndelconss = counts.ndelconss;
        oldnchgsides = counts.nchgsides;
        oldnchgcoefs = counts.nchgcoefs;

        let mut c = firstchange;
        while c < nconss && !crate::solve::solve_is_stopped(scip, false) {
            let cons = conss[c as usize];
            if !scip.conss[cons].is_active() || scip.conss[cons].modifiable {
                c += 1;
                continue;
            }

            npaircomparisons += if cd(scip, cons).presolvedtiming < PRESOLTIMING_EXHAUSTIVE {
                c as i64
            } else {
                c as i64 - firstchange as i64
            };

            preprocess_constraint_pairs(scip, conss, firstchange, c, &mut counts.ndelconss)?;

            if npaircomparisons > NMINCOMPARISONS as i64 {
                if (counts.ndelconss != oldndelconss) || (counts.nchgsides != oldnchgsides) || (counts.nchgcoefs != oldnchgcoefs) {
                    success = true;
                }
                if ((counts.ndelconss - oldndelconss) as f64 + ((counts.nchgsides - oldnchgsides) as f64) / 2.0 + ((counts.nchgcoefs - oldnchgcoefs) as f64) / 10.0)
                    / (npaircomparisons as f64)
                    < MINGAINPERNMINCOMPARISONS
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

    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if success || counts.nfixedvars > oldnfixedvars || counts.nchgbds > oldnchgbds {
        ScipResultCode::Success
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consRespropKnapsack`: propagation conflict resolving method of
/// constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_knapsack(
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
    // Check if we fixed a binary variable to one (due to negated clique).
    if inferinfo >= 0 && scip.vars[infervar].locdom.lb > 0.5 {
        for i in 0..cd(scip, cons).nvars {
            if scip.vars[cd(scip, cons).vars[i]].index == inferinfo {
                let var = cd(scip, cons).vars[i];
                api::add_conflict_binvar(scip, var)?;
                break;
            }
        }
    } else {
        // According to negated cliques the minweightsum and all variables which
        // are fixed to one which led to a fixing of another negated clique
        // variable to one, the inferinfo was chosen to be the negative of the
        // position in the knapsack constraint, see one above call of
        // SCIPinferBinvarCons.
        let mut capsum: i64;
        if inferinfo < 0 {
            capsum = 0;
        } else {
            // Locate the inference variable and calculate the capacity that has to
            // be used up to conclude infervar == 0; inferinfo stores the position
            // of the inference variable (but maybe the variables were re-sorted).
            let c = cd(scip, cons);
            if inferinfo < c.nvars && c.vars[inferinfo] == infervar {
                capsum = c.weights[inferinfo];
            } else {
                let mut i = 0;
                while i < c.nvars && c.vars[i] != infervar {
                    i += 1;
                }
                capsum = c.weights[i];
            }
        }

        // Add fixed-to-one variables up to the point, that their weight plus the
        // weight of the conflict variable exceeds the capacity.
        if capsum <= cd(scip, cons).capacity {
            for i in 0..cd(scip, cons).nvars {
                let var = cd(scip, cons).vars[i];
                if api::get_var_lb_at_index(scip, var, bdchgidx.as_ref(), false) > 0.5 {
                    api::add_conflict_binvar(scip, var)?;
                    capsum += cd(scip, cons).weights[i];
                    if capsum > cd(scip, cons).capacity {
                        break;
                    }
                }
            }
        }
    }

    // NOTE: It might be the case that capsum < consdata->capacity. This is
    // due the fact that the fixing of the variable to zero can included
    // negated clique information. A negated clique means, that at most one of
    // the clique variables can be zero. These information can be used to
    // compute a minimum activity of the constraint and used to fix variables
    // to zero.
    //
    // Even if capsum < consdata->capacity we still reported a complete reason
    // since the minimum activity is based on global variable bounds. It might
    // even be the case that we reported to many variables which are fixed to
    // one.
    *result = ScipResultCode::Success;

    Ok(())
}

/// `consLockKnapsack`: variable rounding lock method of constraint handler.
fn cons_lock_knapsack(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    for i in 0..cd(scip, cons).nvars {
        let var = cd(scip, cons).vars[i];
        api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
    }

    Ok(())
}

/// `consActiveKnapsack`: constraint activation notification method of
/// constraint handler; adds the nlrow if the NLP is constructed, which the
/// port never does.
fn cons_active_knapsack(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId) -> ScipResult {
    Ok(())
}

/// `consDeactiveKnapsack`: constraint deactivation notification method of
/// constraint handler; removes the nlrow, which the port never has.
fn cons_deactive_knapsack(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId) -> ScipResult {
    Ok(())
}

/// `consDelvarsKnapsack`: variable deletion method of constraint handler.
fn cons_delvars_knapsack(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if !conss.is_empty() {
        perform_var_deletions(scip, conss)?;
    }
    Ok(())
}

/// `consGetVarsKnapsack`: constraint method of constraint handler which
/// returns the variables (the vector always fits here).
fn cons_get_vars_knapsack(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.extend_from_slice(&c.vars.0[..c.nvars as usize]);
    *success = true;
    Ok(())
}

/// `consGetNVarsKnapsack`: constraint method of constraint handler which
/// returns the number of variables.
fn cons_get_n_vars_knapsack(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = cd(scip, cons).nvars;
    *success = true;
    Ok(())
}

/*
 * Event handler
 */

/// `eventExecKnapsack`: execution method of bound change event handler.
fn event_exec_knapsack(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let edata = ed(scip, eventhdlr, eventdata);
    let cons = edata.cons;

    match event.eventtype {
        EVENTTYPE_LBTIGHTENED => {
            let c = cdm(scip, cons);
            c.onesweightsum += edata.weight;
            c.presolvedtiming = 0;
            cons_mark_propagate(scip, cons);
        }
        EVENTTYPE_LBRELAXED => {
            cdm(scip, cons).onesweightsum -= edata.weight;
        }
        EVENTTYPE_UBTIGHTENED => {
            cdm(scip, cons).presolvedtiming = 0;
            cons_mark_propagate(scip, cons);
        }
        EVENTTYPE_VARFIXED | EVENTTYPE_IMPLADDED => {
            // The variable should be removed from the constraint in presolving.
            if event.eventtype == EVENTTYPE_VARFIXED && !cd(scip, cons).existmultaggr {
                let var = event.data.var;
                let status = scip.vars[var].varstatus;

                // If the variable was aggregated or multiaggregated, we must signal
                // to propagation that we are no longer merged.
                if status == VarStatus::MultAggr {
                    let c = cdm(scip, cons);
                    c.existmultaggr = true;
                    c.merged = false;
                } else if status == VarStatus::Aggregated || (status == VarStatus::Negated && scip.vars[scip.vars[var].negatedvar].varstatus == VarStatus::Aggregated) {
                    cdm(scip, cons).merged = false;
                }
            }

            // Further preprocessing might be possible due to additional
            // implications.
            cdm(scip, cons).presolvedtiming = 0;
        }
        EVENTTYPE_VARDELETED => {
            cdm(scip, cons).varsdeleted = true;
        }
        _ => {
            // Invalid event type.
            return Err(Retcode::InvalidData);
        }
    }

    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrKnapsack`: creates the handler for knapsack
/// constraints and includes it in SCIP. Sub-SCIP copying, parsing, printing
/// and the symmetry graphs are added with those modules.
pub fn include_conshdlr_knapsack(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Include event handler for bound change events.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_knapsack, Some(Box::new(EventhdlrData::default())))?;

    // Create knapsack constraint handler data.
    let conshdlrdata = ConshdlrData {
        eventhdlr,
        ..ConshdlrData::default()
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
        Some(cons_enfolp_knapsack),
        Some(cons_enfops_knapsack),
        Some(cons_check_knapsack),
        Some(cons_lock_knapsack),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_knapsack);
    h.conscopy = Some(cons_copy_knapsack);
    h.consactive = Some(cons_active_knapsack);
    h.consdeactive = Some(cons_deactive_knapsack);
    h.consdelete = Some(cons_delete_knapsack);
    h.consdelvars = Some(cons_delvars_knapsack);
    h.consexit = Some(cons_exit_knapsack);
    h.consexitpre = Some(cons_exitpre_knapsack);
    h.consinitsol = Some(cons_initsol_knapsack);
    h.consexitsol = Some(cons_exitsol_knapsack);
    h.consfree = Some(cons_free_knapsack);
    h.consgetvars = Some(cons_get_vars_knapsack);
    h.consgetnvars = Some(cons_get_n_vars_knapsack);
    h.consinit = Some(cons_init_knapsack);
    h.consinitpre = Some(cons_initpre_knapsack);
    h.consinitlp = Some(cons_initlp_knapsack);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_knapsack), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_knapsack), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_knapsack);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_knapsack),
        Some(cons_sepasol_knapsack),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_knapsack);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_knapsack);

    if crate::set::set_find_conshdlr(scip, "linear") != NONE {
        // Include the linear constraint to knapsack constraint upgrade in the
        // linear constraint handler.
        crate::cons_linear::include_lincons_upgrade(scip, lincons_upgd_knapsack, LINCONSUPGD_PRIORITY, CONSHDLR_NAME)?;
    }

    // Add knapsack constraint handler parameters.
    paramset_add_int(
        scip,
        "constraints/knapsack/sepacardfreq",
        "multiplier on separation frequency, how often knapsack cuts are separated (-1: never, 0: only at root)",
        crate::data_target!(i32, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, sepacardfreq),
        true,
        DEFAULT_SEPACARDFREQ,
        -1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/knapsack/maxcardbounddist",
        "maximal relative distance from current node's dual bound to primal bound compared to best node's dual bound for separating knapsack cuts",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxcardbounddist),
        true,
        DEFAULT_MAXCARDBOUNDDIST,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/knapsack/cliqueextractfactor",
        "lower clique size limit for greedy clique extraction algorithm (relative to largest clique)",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, cliqueextractfactor),
        true,
        DEFAULT_CLIQUEEXTRACTFACTOR,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/knapsack/maxrounds",
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
        "constraints/knapsack/maxroundsroot",
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
        "constraints/knapsack/maxsepacuts",
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
        "constraints/knapsack/maxsepacutsroot",
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
        "constraints/knapsack/disaggregation",
        "should disaggregation of knapsack constraints be allowed in preprocessing?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, disaggregation),
        true,
        DEFAULT_DISAGGREGATION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/simplifyinequalities",
        "should presolving try to simplify knapsacks",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, simplifyinequalities),
        true,
        DEFAULT_SIMPLIFYINEQUALITIES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/negatedclique",
        "should negated clique information be used in solving process",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, negatedclique),
        true,
        DEFAULT_NEGATEDCLIQUE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/presolpairwise",
        "should pairwise constraint comparison be performed in presolving?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolpairwise),
        true,
        DEFAULT_PRESOLPAIRWISE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/presolusehashing",
        "should hash table be used for detecting redundant constraints in advance",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolusehashing),
        true,
        DEFAULT_PRESOLUSEHASHING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/dualpresolving",
        "should dual presolving steps be performed?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, dualpresolving),
        true,
        DEFAULT_DUALPRESOLVING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/usegubs",
        "should GUB information be used for separation?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, usegubs),
        true,
        DEFAULT_USEGUBS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/detectcutoffbound",
        "should presolving try to detect constraints parallel to the objective function defining an upper bound and prevent these constraints from entering the LP?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, detectcutoffbound),
        true,
        DEFAULT_DETECTCUTOFFBOUND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/detectlowerbound",
        "should presolving try to detect constraints parallel to the objective function defining a lower bound and prevent these constraints from entering the LP?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, detectlowerbound),
        true,
        DEFAULT_DETECTLOWERBOUND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/knapsack/updatecliquepartitions",
        "should clique partition information be updated when old partition seems outdated?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, updatecliquepartitions),
        true,
        DEFAULT_UPDATECLIQUEPARTITIONS,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/knapsack/clqpartupdatefac",
        "factor on the growth of global cliques to decide when to update a previous (negated) clique partition (used only if updatecliquepartitions is set to TRUE)",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, clqpartupdatefac),
        true,
        DEFAULT_CLQPARTUPDATEFAC,
        1.0,
        10.0,
        None,
    )?;

    Ok(())
}

/// `SCIPcreateConsKnapsack`: creates and captures a knapsack constraint.
pub fn create_cons_knapsack(scip: &mut Scip, name: &str, vars: &[VarId], weights: &[i64], capacity: i64, flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the knapsack constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Knapsack constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Check whether all variables are binary.
    for &var in vars {
        if !var_is_binary(scip, var) {
            // Item is not binary.
            return Err(Retcode::InvalidData);
        }
    }

    // Get event handler.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Create constraint data.
    let consdata = consdata_create(scip, vars, weights, capacity)?;

    // Create constraint.
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)?;

    // Catch events for variables.
    if api::is_transformed(scip) {
        catch_events(scip, cons, eventhdlr)?;
    }

    Ok(cons)
}

/// `SCIPcreateConsBasicKnapsack`: creates and captures a knapsack
/// constraint in its most basic version, i. e., all constraint flags are set
/// to their basic value as explained for the method
/// SCIPcreateConsKnapsack().
pub fn create_cons_basic_knapsack(scip: &mut Scip, name: &str, vars: &[VarId], weights: &[i64], capacity: i64) -> ScipResult<ConsId> {
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
    create_cons_knapsack(scip, name, vars, weights, capacity, &flags)
}

/// Whether the constraint belongs to the knapsack constraint handler.
fn is_knapsack(scip: &Scip, cons: ConsId) -> bool {
    scip.conshdlrs[scip.conss[cons].conshdlr].name == CONSHDLR_NAME
}

/// `SCIPaddCoefKnapsack`: adds new item to knapsack constraint.
pub fn add_coef_knapsack(scip: &mut Scip, cons: ConsId, var: VarId, weight: i64) -> ScipResult {
    if !is_knapsack(scip, cons) {
        // Constraint is not a knapsack constraint.
        return Err(Retcode::InvalidData);
    }

    add_coef(scip, cons, var, weight)
}

/// `SCIPgetCapacityKnapsack`: gets the capacity of the knapsack constraint.
pub fn get_capacity_knapsack(scip: &Scip, cons: ConsId) -> i64 {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack constraint");
    cd(scip, cons).capacity
}

/// `SCIPchgCapacityKnapsack`: changes capacity of the knapsack constraint;
/// only during problem creation stage (SCIP_STAGE_PROBLEM).
pub fn chg_capacity_knapsack(scip: &mut Scip, cons: ConsId, capacity: i64) -> ScipResult {
    if !is_knapsack(scip, cons) {
        // Constraint is not a knapsack constraint.
        return Err(Retcode::InvalidData);
    }

    if scip.set.stage != Stage::Problem {
        // Method can only be called during problem creation stage.
        return Err(Retcode::InvalidData);
    }

    cdm(scip, cons).capacity = capacity;
    Ok(())
}

/// `SCIPgetNVarsKnapsack`: gets the number of items in the knapsack
/// constraint.
pub fn get_n_vars_knapsack(scip: &Scip, cons: ConsId) -> i32 {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack constraint");
    cd(scip, cons).nvars
}

/// `SCIPgetVarsKnapsack`: gets the array of variables in the knapsack
/// constraint.
pub fn get_vars_knapsack(scip: &Scip, cons: ConsId) -> &[VarId] {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack constraint");
    let c = cd(scip, cons);
    &c.vars.0[..c.nvars as usize]
}

/// `SCIPgetWeightsKnapsack`: gets the array of weights in the knapsack
/// constraint.
pub fn get_weights_knapsack(scip: &Scip, cons: ConsId) -> &[i64] {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack constraint");
    let c = cd(scip, cons);
    &c.weights.0[..c.nvars as usize]
}

/// `SCIPgetDualsolKnapsack`: gets the dual solution of the knapsack
/// constraint in the current LP.
pub fn get_dualsol_knapsack(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack constraint");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualsol(scip, row) } else { 0.0 }
}

/// `SCIPgetDualfarkasKnapsack`: gets the dual Farkas value of the knapsack
/// constraint in the current infeasible LP.
pub fn get_dualfarkas_knapsack(scip: &Scip, cons: ConsId) -> f64 {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack constraint");
    let row = cd(scip, cons).row;
    if row != NONE { scip_lp::row_get_dualfarkas(scip, row) } else { 0.0 }
}

/// `SCIPgetRowKnapsack`: returns the linear relaxation of the given knapsack
/// constraint; may return `NONE` if no LP row was yet created.
pub fn get_row_knapsack(scip: &Scip, cons: ConsId) -> usize {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack");
    cd(scip, cons).row
}

/// `SCIPcreateRowKnapsack`: creates and returns the row of the given
/// knapsack constraint.
pub fn create_row_knapsack(scip: &mut Scip, cons: ConsId) -> ScipResult {
    assert!(is_knapsack(scip, cons), "constraint is not a knapsack");
    create_relaxation(scip, cons)
}

/// `SCIPcleanupConssKnapsack`: cleans up (multi-)aggregations and fixings
/// from knapsack constraints; returns whether the problem was detected to be
/// infeasible.
pub fn cleanup_conss_knapsack(scip: &mut Scip, onlychecked: bool, ndelconss: &mut i32) -> ScipResult<bool> {
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

/// `conshdlrCopyKnapsack`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_knapsack(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_knapsack(scip)?;
    *valid = true;
    Ok(())
}

/// `consCopyKnapsack`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_knapsack(
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
    let sourcevars = get_vars_knapsack(sourcescip, sourcecons).to_vec();
    let coefs: Vec<f64> = get_weights_knapsack(sourcescip, sourcecons).iter().map(|&w| w as f64).collect();
    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);

    // Copy the logic using the linear constraint copy method.
    let lhs = -scip.set.infinity();
    let rhs = get_capacity_knapsack(sourcescip, sourcecons) as f64;
    (*cons, *valid) = crate::cons_linear::copy_cons_linear(scip, sourcescip, &consname, &sourcevars, Some(&coefs), lhs, rhs, maps, flags, global)?;
    Ok(())
}
