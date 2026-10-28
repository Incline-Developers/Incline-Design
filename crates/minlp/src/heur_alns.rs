//! Port of `heur_alns.c`: adaptive large neighborhood search, which lets a
//! bandit algorithm choose among the neighborhoods RENS, RINS, mutation,
//! local branching, crossover, proximity, zero objective, DINS and trust
//! region, and solves the chosen one in a sub-SCIP.
//!
//! C's sub-SCIP event handler adds the sub-SCIP's new best solutions to the
//! main SCIP while the sub-SCIP solves. Here the handler lives in the
//! sub-SCIP and cannot reach the main SCIP, so it records the solution
//! values and the transfers are made, in order, right after the sub-SCIP
//! returns; nothing reads the main SCIP in between, so the result is the
//! same. The reward file (`rewardfilename`, "all rewards" mode), the
//! neighborhood statistics table and the trust region neighborhood (off by
//! default) are not ported.

use std::{any::Any, rc::Rc};

use crate::{
    api,
    bandit::Bandit,
    copy::CopyMaps,
    event::{EVENTTYPE_BESTSOLFOUND, EVENTTYPE_LPSOLVED, EVENTTYPE_SOLFOUND, Event, EventType},
    heur::{HEURTIMING_AFTERNODE, HEURTIMING_DURINGLPLOOP, HeurTiming, VGraph},
    misc::RandNumGen,
    paramset::{ParamSetting, Target, paramset_set_bool, paramset_set_char, paramset_set_int, paramset_set_longint, paramset_set_real},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{EventhdlrId, HeurId, LpSolStat, ScipResultCode, SolId, SolOrigin, Status, VarId, VarStatus},
    var::NONE,
};

const HEUR_NAME: &str = "alns";
const HEUR_DESC: &str = "Large neighborhood search heuristic that orchestrates the popular neighborhoods Local Branching, RINS, RENS, DINS etc.";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1100500;
const HEUR_FREQ: i32 = 20;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERNODE | HEURTIMING_DURINGLPLOOP;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

const NNEIGHBORHOODS: usize = 9;

/// Show statistics on neighborhoods?
const DEFAULT_SHOWNBSTATS: bool = false;
const DEFAULT_NODESQUOT: f64 = 0.1;
const DEFAULT_NODESQUOTMIN: f64 = 0.0;
const DEFAULT_NODESOFFSET: i64 = 500;
const DEFAULT_NSOLSLIM: i32 = 3;
const DEFAULT_MINNODES: i64 = 50;
const DEFAULT_MAXNODES: i64 = 5000;
/// Number of nodes since last incumbent solution that the heuristic should
/// wait.
const DEFAULT_WAITINGNODES: i64 = 25;
const DEFAULT_TARGETNODEFACTOR: f64 = 1.05;
/// Lower bound for learning rate for target nodes and minimum improvement.
const LRATEMIN: f64 = 0.01;
const LPLIMFAC: f64 = 4.0;
const DEFAULT_INITDURINGROOT: bool = false;
/// Number of allowed executions of the heuristic on the same incumbent
/// solution.
const DEFAULT_MAXCALLSSAMESOL: i32 = -1;
const DEFAULT_MINIMPROVELOW: f64 = 0.01;
const DEFAULT_MINIMPROVEHIGH: f64 = 0.01;
const MINIMPROVEFAC: f64 = 1.5;
const DEFAULT_STARTMINIMPROVE: f64 = 0.01;
const DEFAULT_ADJUSTMINIMPROVE: bool = false;
/// Should the target nodes be dynamically adjusted?
const DEFAULT_ADJUSTTARGETNODES: bool = true;
const DEFAULT_BESTSOLWEIGHT: i32 = 1;
/// The default bandit algorithm: (u)pper confidence bounds, (e)xp.3,
/// epsilon (g)reedy, exp.3-(i)x.
const DEFAULT_BANDITALGO: u8 = b'i';
/// Reward control to increase the weight of the simple solution indicator
/// and decrease the weight of the closed gap reward.
const DEFAULT_REWARDCONTROL: f64 = 0.8;
/// Should the reward be scaled by the effort?
const DEFAULT_SCALEBYEFFORT: bool = true;
/// Should the bandit algorithms be reset when a new problem is read?
const DEFAULT_RESETWEIGHTS: bool = true;
/// Should random seeds of sub-SCIPs be altered to increase diversification?
const DEFAULT_SUBSCIPRANDSEEDS: bool = false;
/// The reward baseline to separate successful and failed calls.
const DEFAULT_REWARDBASELINE: f64 = 0.5;
/// Tolerance by which the fixing rate may be missed without generic
/// fixing.
const DEFAULT_FIXTOL: f64 = 0.1;
/// Tolerance by which the fixing rate may be exceeded without generic
/// unfixing.
const DEFAULT_UNFIXTOL: f64 = 0.1;
/// Should local reduced costs be used for generic (un)fixing?
const DEFAULT_USELOCALREDCOST: bool = false;
/// Default reward offset between 0 and 1 at every observation for exp3.
const DEFAULT_BETA: f64 = 0.0;
/// Increase exploration in epsilon-greedy bandit algorithm.
const DEFAULT_EPS: f64 = 0.4685844;
/// Parameter to increase the confidence width in UCB.
const DEFAULT_ALPHA: f64 = 0.0016;
/// Default weight between uniform (gamma ~ 1) and weight driven (gamma ~ 0)
/// probability distribution for exp3.
const DEFAULT_GAMMA: f64 = 0.07041455;
/// Should reduced cost scores be used for variable priorization?
const DEFAULT_USEREDCOST: bool = true;
/// Should pseudo cost scores be used for variable priorization?
const DEFAULT_USEPSCOST: bool = true;
/// Should distances from fixed variables be used for variable priorization.
const DEFAULT_USEDISTANCES: bool = true;
/// Should the ALNS heuristic do more fixings by itself based on variable
/// prioritization until the target fixing rate is reached?
const DEFAULT_DOMOREFIXINGS: bool = true;
/// Should the heuristic adjust the target fixing rate based on the success?
const DEFAULT_ADJUSTFIXINGRATE: bool = true;
/// Should the heuristic activate other sub-SCIP heuristics during its
/// search?
const DEFAULT_USESUBSCIPHEURS: bool = false;
/// Should cutting planes be copied to the sub-SCIP?
const DEFAULT_COPYCUTS: bool = false;
/// File name to store all rewards and the selection of the bandit.
const DEFAULT_REWARDFILENAME: &str = "-";
const DEFAULT_SEED: i32 = 113;

/// Geometric decay for fixing rate adjustments.
const FIXINGRATE_DECAY: f64 = 0.75;
/// Initial increment value for fixing rate.
const FIXINGRATE_STARTINC: f64 = 0.2;

const MUTATIONSEED: u32 = 121;
const CROSSOVERSEED: u32 = 321;

/// Parameter for the number of solutions that crossover should combine.
const DEFAULT_NSOLS_CROSSOVER: i32 = 2;
/// Number of pool solutions where binary solution values must agree.
const DEFAULT_NPOOLSOLS_DINS: i32 = 5;
/// The penalty for violating the trust region.
const DEFAULT_VIOLPENALTY_TRUSTREGION: f64 = 100.0;

const EVENTHDLR_NAME: &str = "Alns";
const EVENTHDLR_DESC: &str = "LP event handler for alns heuristic";
const EVENTTYPE_ALNS: EventType = EVENTTYPE_LPSOLVED | EVENTTYPE_SOLFOUND | EVENTTYPE_BESTSOLFOUND;

/// Reward types of ALNS.
const REWARDTYPE_TOTAL: usize = 0;
const REWARDTYPE_BESTSOL: usize = 1;
const REWARDTYPE_CLOSEDGAP: usize = 2;
const REWARDTYPE_NOSOLPENALTY: usize = 3;
const NREWARDTYPES: usize = 4;

/// Sub-SCIP status codes of the statistics histogram.
const HIDX_OPT: usize = 0;
const HIDX_USR: usize = 1;
const HIDX_NODELIM: usize = 2;
const HIDX_STALLNODE: usize = 3;
const HIDX_INFEAS: usize = 4;
const HIDX_SOLLIM: usize = 5;
const HIDX_OTHER: usize = 6;
const NHISTENTRIES: usize = 7;

/// `NH_STATS`: statistics for a neighborhood.
#[derive(Clone, Copy, Debug, Default)]
struct NhStats {
    usednodes: i64,
    oldupperbound: f64,
    newupperbound: f64,
    nruns: i32,
    nrunsbestsol: i32,
    nsolsfound: i64,
    nbestsolsfound: i64,
    nfixings: i32,
    statushist: [i32; NHISTENTRIES],
}

/// `NH_FIXINGRATE`: fixing rate data structure to control the amount of
/// target fixings of a neighborhood.
#[derive(Clone, Copy, Debug, Default)]
struct NhFixingRate {
    minfixingrate: f64,
    targetfixingrate: f64,
    increment: f64,
    maxfixingrate: f64,
}

/// The neighborhoods, which select C's callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NhKind {
    Rens,
    Rins,
    Mutation,
    LocalBranching,
    Crossover,
    Proximity,
    ZeroObjective,
    Dins,
    TrustRegion,
}

/// `NH`: neighborhood data structure with statistics, fixing rate and its
/// specific data.
struct Nh {
    name: &'static str,
    fixingrate: NhFixingRate,
    stats: NhStats,
    kind: NhKind,
    active: bool,
    priority: f64,
    /// Mutation: random number generator.
    mutation_rng: Option<RandNumGen>,
    /// Crossover: the number of solutions that crossover should combine.
    crossover_nsols: i32,
    /// Crossover: random number generator to draw from the solution pool.
    crossover_rng: Option<RandNumGen>,
    /// Crossover: best selected solution by crossover as reference point.
    crossover_selsol: SolId,
    /// DINS: number of pool solutions where binary solution values must
    /// agree.
    dins_npoolsols: i32,
    /// Trust region: the penalty for violating the trust region.
    trustregion_violpenalty: f64,
}

/// `SCIP_HeurData`. The neighborhoods stay at fixed positions in `nhs`
/// (their parameters refer to them there); `neighborhoods` is C's pointer
/// array, which initsol permutes.
struct HeurData {
    nhs: Vec<Nh>,
    neighborhoods: Vec<usize>,
    bandit: Option<Bandit>,
    lastcallsol: SolId,
    rewardfilename: String,
    nodesoffset: i64,
    maxnodes: i64,
    targetnodes: i64,
    minnodes: i64,
    usednodes: i64,
    waitingnodes: i64,
    nodesquot: f64,
    nodesquotmin: f64,
    startminimprove: f64,
    minimprovelow: f64,
    minimprovehigh: f64,
    minimprove: f64,
    lplimfac: f64,
    exp3_gamma: f64,
    exp3_beta: f64,
    epsgreedy_eps: f64,
    ucb_alpha: f64,
    rewardcontrol: f64,
    targetnodefactor: f64,
    rewardbaseline: f64,
    fixtol: f64,
    unfixtol: f64,
    nactiveneighborhoods: i32,
    ninitneighborhoods: i32,
    nsolslim: i32,
    seed: i32,
    currneighborhood: i32,
    ndelayedcalls: i32,
    maxcallssamesol: i32,
    firstcallthissol: i64,
    banditalgo: u8,
    useredcost: bool,
    usedistances: bool,
    usepscost: bool,
    domorefixings: bool,
    adjustfixingrate: bool,
    usesubscipheurs: bool,
    adjustminimprove: bool,
    adjusttargetnodes: bool,
    resetweights: bool,
    subsciprandseeds: bool,
    scalebyeffort: bool,
    copycuts: bool,
    uselocalredcost: bool,
    initduringroot: bool,
    shownbstats: bool,
}

/// `SOLVELIMITS`: limits for the sub-SCIP solving process.
#[derive(Default)]
struct SolveLimits {
    nodelimit: i64,
    memorylimit: f64,
    timelimit: f64,
    stallnodes: i64,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut().unwrap()
}

/// The neighborhood at position `idx` of C's pointer array.
fn nh(scip: &Scip, heur: HeurId, idx: usize) -> &Nh {
    let d = hd(scip, heur);
    &d.nhs[d.neighborhoods[idx]]
}

fn nhm(scip: &mut Scip, heur: HeurId, idx: usize) -> &mut Nh {
    let d = hdm(scip, heur);
    let k = d.neighborhoods[idx];
    &mut d.nhs[k]
}

/*
 * Local methods
 */

/// `resetFixingRate`.
fn reset_fixing_rate(fixingrate: &mut NhFixingRate) {
    fixingrate.increment = FIXINGRATE_STARTINC;

    // Always start with the most conservative value.
    fixingrate.targetfixingrate = fixingrate.maxfixingrate;
}

/// `resetCurrentNeighborhood`.
fn reset_current_neighborhood(d: &mut HeurData) {
    d.currneighborhood = -1;
    d.ndelayedcalls = 0;
}

/// `updateFixingRateIncrement`.
fn update_fixing_rate_increment(fx: &mut NhFixingRate) {
    fx.increment *= FIXINGRATE_DECAY;
    fx.increment = crate::def::max(fx.increment, LRATEMIN);
}

/// `increaseFixingRate`: increases fixing rate; this method increases the
/// target fixing rate by the current increment.
fn increase_fixing_rate(fx: &mut NhFixingRate) {
    fx.targetfixingrate += fx.increment;
    fx.targetfixingrate = crate::def::min(fx.targetfixingrate, fx.maxfixingrate);
}

/// `decreaseFixingRate`.
fn decrease_fixing_rate(fx: &mut NhFixingRate) {
    fx.targetfixingrate -= fx.increment;
    fx.targetfixingrate = crate::def::max(fx.targetfixingrate, fx.minfixingrate);
}

/// `updateFixingRate`: updates the fixing rate based on the results of the
/// current run.
fn update_fixing_rate(neighborhood: &mut Nh, subscipstatus: Status, runstats: &NhStats) {
    let fx = &mut neighborhood.fixingrate;

    match subscipstatus {
        Status::Optimal | Status::Infeasible | Status::InfOrUnbd | Status::SolLimit | Status::BestSolLimit => {
            // Decrease the fixing rate (make subproblem harder).
            decrease_fixing_rate(fx);
        }
        Status::StallNodeLimit | Status::UserInterrupt | Status::Terminate | Status::NodeLimit => {
            // Increase the fixing rate (make the subproblem easier) only if no
            // solution was found.
            if runstats.nbestsolsfound <= 0 {
                increase_fixing_rate(fx);
            }
        }
        _ => {}
    }

    update_fixing_rate_increment(fx);
}

/// `increaseTargetNodeLimit`.
fn increase_target_node_limit(d: &mut HeurData) {
    d.targetnodes = (d.targetnodes as f64 * d.targetnodefactor) as i64 + 1;

    // Respect upper and lower parametrized bounds on targetnodes.
    if d.targetnodes > d.maxnodes {
        d.targetnodes = d.maxnodes;
    }
}

/// `resetTargetNodeLimit`.
fn reset_target_node_limit(d: &mut HeurData) {
    d.targetnodes = d.minnodes;
}

/// `updateTargetNodeLimit`: updates the target node limit based on the
/// current run results.
fn update_target_node_limit(d: &mut HeurData, runstats: &NhStats, subscipstatus: Status) {
    match subscipstatus {
        Status::StallNodeLimit | Status::NodeLimit => {
            // The subproblem could be explored more.
            if runstats.nbestsolsfound == 0 {
                increase_target_node_limit(d);
            }
        }
        _ => {}
    }
}

/// `resetMinimumImprovement`.
fn reset_minimum_improvement(d: &mut HeurData) {
    d.minimprove = d.startminimprove;
}

/// `increaseMinimumImprovement`.
fn increase_minimum_improvement(d: &mut HeurData) {
    d.minimprove *= MINIMPROVEFAC;
    d.minimprove = crate::def::min(d.minimprove, d.minimprovehigh);
}

/// `decreaseMinimumImprovement`.
fn decrease_minimum_improvement(d: &mut HeurData) {
    d.minimprove /= MINIMPROVEFAC;
    d.minimprove = crate::def::max(d.minimprove, d.minimprovelow);
}

/// `updateMinimumImprovement`: updates the minimum improvement based on the
/// status of the sub-SCIP.
fn update_minimum_improvement(d: &mut HeurData, subscipstatus: Status, runstats: &NhStats) {
    // If the sub-SCIP status was infeasible, we rather want to make the
    // sub-SCIP easier with a smaller minimum improvement.
    match subscipstatus {
        // Subproblem was infeasible, probably due to the minimum improvement ->
        // decrease minimum improvement.
        Status::Infeasible | Status::InfOrUnbd => decrease_minimum_improvement(d),
        // Subproblem could be optimally solved -> try higher minimum
        // improvement.
        Status::SolLimit | Status::BestSolLimit | Status::Optimal => increase_minimum_improvement(d),
        // Subproblem was too hard, decrease minimum improvement.
        Status::NodeLimit | Status::StallNodeLimit | Status::UserInterrupt => {
            if runstats.nbestsolsfound <= 0 {
                decrease_minimum_improvement(d);
            }
        }
        _ => {}
    }
}

/// `neighborhoodStatsReset`.
fn neighborhood_stats_reset(stats: &mut NhStats) {
    stats.nbestsolsfound = 0;
    stats.nruns = 0;
    stats.nrunsbestsol = 0;
    stats.nsolsfound = 0;
    stats.usednodes = 0;
    stats.nfixings = 0;
    stats.statushist = [0; NHISTENTRIES];
}

/// `neighborhoodInit`: initializes neighborhood specific data.
fn neighborhood_init(scip: &mut Scip, heur: HeurId, k: usize) {
    let nvars = api::get_n_vars(scip) as u32;
    match hd(scip, heur).nhs[k].kind {
        NhKind::Crossover => {
            // `nhInitCrossover`.
            let seed = scip.set.initialize_random_seed(CROSSOVERSEED + nvars);
            let n = &mut hdm(scip, heur).nhs[k];
            n.crossover_selsol = NONE;
            n.crossover_rng = Some(RandNumGen::new(seed));
        }
        NhKind::Mutation => {
            // `nhInitMutation`.
            let seed = scip.set.initialize_random_seed(MUTATIONSEED + nvars);
            hdm(scip, heur).nhs[k].mutation_rng = Some(RandNumGen::new(seed));
        }
        _ => {}
    }
}

/// `neighborhoodExit`: deinitializes neighborhood specific data.
fn neighborhood_exit(n: &mut Nh) {
    match n.kind {
        NhKind::Crossover => n.crossover_rng = None,
        NhKind::Mutation => n.mutation_rng = None,
        _ => {}
    }
}

/// The data of the sub-SCIP's event handler: C's `SCIP_EVENTDATA` without
/// the pointers into the main SCIP, plus the solutions to transfer.
struct EventhdlrData {
    /// The sub-SCIP variables in the order of the main SCIP's.
    subvars: Vec<VarId>,
    nodelimit: i64,
    lplimfac: f64,
    /// For each transfer, the values of the sub-SCIP's best solution at the
    /// main SCIP's variables (`None` for those without a copy).
    transfers: Vec<Vec<Option<f64>>>,
}

/// `eventExecAlns`: execution callback of the event handler. As in C, only
/// best solution events transfer, since an atomic event type never equals
/// the `SCIP_EVENTTYPE_SOLFOUND` mask.
fn event_exec_alns(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, _eventdata: usize) -> ScipResult {
    let eventtype = event.eventtype;
    if eventtype == EVENTTYPE_SOLFOUND || eventtype == EVENTTYPE_BESTSOLFOUND {
        // Try to transfer the solution to the original SCIP: record the values
        // of the best solution now.
        let subsol = api::get_best_sol(scip);
        let d: &EventhdlrData = scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref().unwrap();
        let values: Vec<Option<f64>> = d
            .subvars
            .iter()
            .map(|&sv| if sv != NONE { Some(api::get_sol_val(scip, subsol, sv)) } else { None })
            .collect();
        scip.eventhdlrs[eventhdlr]
            .eventhdlrdata
            .as_mut()
            .unwrap()
            .downcast_mut::<EventhdlrData>()
            .unwrap()
            .transfers
            .push(values);
    } else if eventtype == EVENTTYPE_LPSOLVED {
        // Interrupt solution process of sub-SCIP.
        let d: &EventhdlrData = scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref().unwrap();
        if api::get_n_lps(scip) as f64 > d.lplimfac * d.nodelimit as f64 {
            crate::scip_solve::interrupt_solve(scip);
        }
    }
    Ok(())
}

/// `transferSolution`: creates a new solution for the original problem from
/// recorded sub-SCIP solution values (`SCIPtranslateSubSol`) and tries to
/// add it.
fn transfer_solution(scip: &mut Scip, heur: HeurId, values: &[Option<f64>], runstats: &mut NhStats) -> ScipResult {
    let vars = api::get_vars(scip).to_vec();
    let solvals: Vec<f64> = vars
        .iter()
        .enumerate()
        .map(|(i, &var)| match values[i] {
            Some(v) => v,
            None => {
                let v = &scip.vars[var];
                crate::def::min(crate::def::max(0.0, v.locdom.lb), v.locdom.ub)
            }
        })
        .collect();
    let newsol = api::create_sol(scip, heur)?;
    api::set_sol_vals(scip, newsol, &vars, &solvals)?;

    let oldbestsol = api::get_best_sol(scip);

    // Try to add new solution to scip and free it immediately.
    let success = api::try_sol_free(scip, newsol, false, false, true, true, true)?;
    if success {
        runstats.nsolsfound += 1;
        if api::get_best_sol(scip) != oldbestsol {
            runstats.nbestsolsfound += 1;
        }
    }

    // Update new upper bound for reward later.
    runstats.newupperbound = api::get_upperbound(scip);
    Ok(())
}

/// `initRunStats`: initializes neighborhood statistics before the next run.
fn init_run_stats(scip: &Scip, stats: &mut NhStats) {
    stats.nbestsolsfound = 0;
    stats.nsolsfound = 0;
    stats.usednodes = 0;
    stats.nfixings = 0;
    stats.oldupperbound = api::get_upperbound(scip);
    stats.newupperbound = api::get_upperbound(scip);
}

/// `updateRunStats`: updates run stats after the sub SCIP was solved.
fn update_run_stats(stats: &mut NhStats, subscip: Option<&Scip>) {
    // Treat an untransformed subscip as if none was created.
    let subscip = subscip.filter(|s| s.set.stage >= crate::scip::Stage::Transformed);
    stats.usednodes = subscip.map_or(0, api::get_n_nodes);
}

/// `getHistIndex`: the histogram index for this status.
fn get_hist_index(subscipstatus: Status) -> usize {
    match subscipstatus {
        Status::Optimal => HIDX_OPT,
        Status::Infeasible => HIDX_INFEAS,
        Status::NodeLimit => HIDX_NODELIM,
        Status::StallNodeLimit => HIDX_STALLNODE,
        Status::SolLimit | Status::BestSolLimit => HIDX_SOLLIM,
        Status::UserInterrupt => HIDX_USR,
        _ => HIDX_OTHER,
    }
}

/// `updateNeighborhoodStats`: updates the statistics of the neighborhood
/// based on the current run.
fn update_neighborhood_stats(runstats: &NhStats, neighborhood: &mut Nh, subscipstatus: Status) {
    let stats = &mut neighborhood.stats;

    // Copy run statistics into neighborhood statistics.
    stats.nbestsolsfound += runstats.nbestsolsfound;
    stats.nsolsfound += runstats.nsolsfound;
    stats.usednodes += runstats.usednodes;
    stats.nruns += 1;

    if runstats.nbestsolsfound > 0 {
        stats.nrunsbestsol += DEFAULT_BESTSOLWEIGHT;
    } else if runstats.nsolsfound > 0 {
        stats.nrunsbestsol += 1;
    }

    // Update the counter for the subscip status.
    stats.statushist[get_hist_index(subscipstatus)] += 1;
}

/// `VARPRIO`: data used for variable prioritization for additional
/// fixings.
struct VarPrio<'a> {
    randscores: &'a [f64],
    distances: &'a [i32],
    redcostscores: &'a [f64],
    pscostscores: &'a [f64],
    useredcost: bool,
    usedistances: bool,
    usepscost: bool,
}

/// `sortIndCompAlns`: sort callback for variable indices using the ALNS
/// variable prioritization; the variable with a smaller score comes first.
fn sort_ind_comp_alns(varprio: &VarPrio, ind1: i32, ind2: i32) -> i32 {
    if ind1 == ind2 {
        return 0;
    }
    let (i1, i2) = (ind1 as usize, ind2 as usize);

    // Priority is on distances, if enabled. The variable which is closer in a
    // breadth-first search sense to the already fixed variables has
    // precedence.
    if varprio.usedistances {
        let mut dist1 = varprio.distances[i1];
        let mut dist2 = varprio.distances[i2];

        if dist1 < 0 {
            dist1 = i32::MAX;
        }
        if dist2 < 0 {
            dist2 = i32::MAX;
        }

        if dist1 < dist2 {
            return -1;
        } else if dist1 > dist2 {
            return 1;
        }
    }

    // If the indices tie considering distances or distances are disabled ->
    // use reduced cost information instead.
    if varprio.useredcost {
        if varprio.redcostscores[i1] < varprio.redcostscores[i2] {
            return -1;
        } else if varprio.redcostscores[i1] > varprio.redcostscores[i2] {
            return 1;
        }
    }

    // Use pseudo cost scores if reduced costs are disabled or a tie was found.
    if varprio.usepscost {
        // Prefer the variable with smaller pseudocost score.
        if varprio.pscostscores[i1] < varprio.pscostscores[i2] {
            return -1;
        } else if varprio.pscostscores[i1] > varprio.pscostscores[i2] {
            return 1;
        }
    }

    if varprio.randscores[i1] < varprio.randscores[i2] {
        return -1;
    } else if varprio.randscores[i1] > varprio.randscores[i2] {
        return 1;
    }

    ind1 - ind2
}

/// `getVariableRedcostScore`: the reduced cost score for this variable in
/// the reference solution.
fn get_variable_redcost_score(scip: &mut Scip, var: VarId, refsolval: f64, uselocalredcost: bool) -> f64 {
    // Prefer column variables.
    if scip.vars[var].varstatus != VarStatus::Column {
        return scip.set.infinity();
    }

    let (redcost, bestbound) = if !uselocalredcost {
        (crate::var::var_get_best_root_redcost(scip, var), crate::var::var_get_best_root_sol(scip, var))
    } else {
        // This can be safely asserted here, since the heuristic would not
        // reach this point, otherwise.
        (api::get_var_redcost(scip, var), crate::var::var_get_lp_sol(scip, var))
    };

    let mut score = redcost * (refsolval - bestbound);

    // Max out numerical inaccuracies from global scores.
    if !uselocalredcost {
        score = crate::def::max(score, 0.0);
    }

    score
}

/// `getVariablePscostScore`: the pseudo cost score of this variable with
/// respect to the reference solution.
fn get_variable_pscost_score(scip: &Scip, var: VarId, refsolval: f64, uselocallpsol: bool) -> f64 {
    // Variables that aren't LP columns have no pseudocost score.
    if scip.vars[var].varstatus != VarStatus::Column {
        return 0.0;
    }

    let soldiff = refsolval
        - if uselocallpsol {
            crate::var::var_get_lp_sol(scip, var)
        } else {
            crate::var::var_get_root_sol(scip, var)
        };

    // The score is 0.0 if the values are equal.
    if scip.set.is_feas_zero(soldiff) {
        0.0
    } else {
        crate::var::var_get_pseudocost(scip, var, soldiff)
    }
}

/// `tryAdd2variableBuffer`: adds a variable and solution value to the
/// fixings if the value is within the variable's global bounds.
fn try_add2variable_buffer(scip: &Scip, var: VarId, mut val: f64, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>, integer: bool) {
    // Round the value to its nearest integer.
    if integer {
        val = scip.set.floor(val + 0.5);
    }

    // Only add fixing if it is still valid within the global variable bounds.
    // Invalidity of this solution value can be due to a dual reduction that
    // affected the global bounds of the variable.
    if scip.vars[var].glbdom.lb <= val && val <= scip.vars[var].glbdom.ub {
        varbuf.push(var);
        valbuf.push(val);
    }
}

/// `neighborhoodGetRefsol`: queries the neighborhood for a reference
/// solution for further fixings.
fn neighborhood_get_refsol(scip: &Scip, n: &Nh) -> SolId {
    match n.kind {
        // `nhRefsolIncumbent`.
        NhKind::Rins | NhKind::Mutation | NhKind::LocalBranching | NhKind::Proximity | NhKind::ZeroObjective | NhKind::Dins | NhKind::TrustRegion => api::get_best_sol(scip),
        // `nhRefsolCrossover`.
        NhKind::Crossover => n.crossover_selsol,
        NhKind::Rens => NONE,
    }
}

/// `alnsFixMoreVariables`: fixes additional variables found in a feasible
/// reference solution if the ones that the neighborhood found were not
/// enough; returns success.
fn alns_fix_more_variables(scip: &mut Scip, heur: HeurId, refsol: SolId, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>, ntargetfixings: i32) -> ScipResult<bool> {
    let d = hd(scip, heur);

    // If the user parameter forbids more fixings, return immediately.
    if !d.domorefixings {
        return Ok(false);
    }

    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbinintvars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;

    if ntargetfixings as usize >= nbinintvars {
        return Ok(false);
    }

    // Determine the number of required additional fixings.
    let mut nvarstoadd = ntargetfixings - varbuf.len() as i32;
    if nvarstoadd == 0 {
        return Ok(false);
    }

    let usedistances = d.usedistances && !varbuf.is_empty();
    let (useredcost, usepscost, uselocalredcost) = (d.useredcost, d.usepscost, d.uselocalredcost);

    let mut randscores = vec![0.0; nbinintvars];
    let mut perm = vec![0i32; nbinintvars];
    let mut distances = vec![0i32; nvars];
    let mut redcostscores = vec![0.0; nbinintvars];
    let mut isfixed = vec![false; nbinintvars];
    let mut unfixedvars = vec![NONE; nbinintvars];
    let mut pscostscores = vec![0.0; nbinintvars];

    // Initialize variable graph distances from already fixed variables.
    if usedistances {
        VGraph::breadth_first(scip, None, varbuf, &mut distances, i32::MAX, i32::MAX, ntargetfixings)?;
    }

    // Mark binary and integer variables if they are fixed.
    for &v in varbuf.iter() {
        let probindex = scip.vars[v].probindex as usize;
        if probindex < nbinintvars {
            isfixed[probindex] = true;
        }
    }

    let mut solvals = vec![0.0; nbinintvars];
    api::get_sol_vals(scip, refsol, &vars[..nbinintvars], &mut solvals);

    // Assign scores to unfixed every discrete variable of the problem.
    let mut nunfixedvars = 0;
    for b in 0..nbinintvars {
        let var = vars[b];

        // Filter fixed variables.
        if isfixed[b] {
            continue;
        }

        // Filter variables with a solution value outside its global bounds.
        if solvals[b] < scip.vars[var].glbdom.lb - 0.5 || solvals[b] > scip.vars[var].glbdom.ub + 0.5 {
            continue;
        }

        // Filter variables with a fractional solution value (the reference
        // solution might be an LP solution).
        if !scip.set.is_feas_integral(solvals[b]) {
            continue;
        }

        redcostscores[nunfixedvars] = get_variable_redcost_score(scip, var, solvals[b], uselocalredcost);
        pscostscores[nunfixedvars] = get_variable_pscost_score(scip, var, solvals[b], uselocalredcost);

        unfixedvars[nunfixedvars] = var;
        perm[nunfixedvars] = nunfixedvars as i32;
        randscores[nunfixedvars] = hdm(scip, heur).bandit.as_mut().unwrap().rng().get_real(0.0, 1.0);

        // These assignments are based on the fact that nunfixedvars <= b.
        solvals[nunfixedvars] = solvals[b];
        distances[nunfixedvars] = distances[b];

        nunfixedvars += 1;
    }

    // Use selection algorithm (order of the variables does not matter) for
    // quickly completing the fixing.
    let varprio = VarPrio {
        randscores: &randscores,
        distances: &distances,
        redcostscores: &redcostscores,
        pscostscores: &pscostscores,
        useredcost,
        usedistances,
        usepscost,
    };

    // Select the first nvarstoadd many variables according to the score.
    if (nvarstoadd as usize) < nunfixedvars {
        crate::sort::select(&mut perm[..nunfixedvars], (), |&a, &b| sort_ind_comp_alns(&varprio, a, b), nvarstoadd as usize);
    } else {
        nvarstoadd = nunfixedvars as i32;
    }

    // Loop over the first elements of the selection defined in permutation.
    // They represent the best variables.
    for &permindex in &perm[..nvarstoadd as usize] {
        let p = permindex as usize;
        try_add2variable_buffer(scip, unfixedvars[p], solvals[p], varbuf, valbuf, true);
    }

    Ok(true)
}

/// `createBandit`: creates the bandit algorithm for the heuristic depending
/// on the user parameter.
fn create_bandit(scip: &Scip, d: &HeurData, priorities: &[f64], initseed: u32) -> ScipResult<Bandit> {
    match d.banditalgo {
        b'u' => crate::bandit::create_bandit_ucb(scip, Some(priorities), d.ucb_alpha, d.nactiveneighborhoods, initseed),
        b'e' => crate::bandit::create_bandit_exp3(scip, Some(priorities), d.exp3_gamma, d.exp3_beta, d.nactiveneighborhoods, initseed),
        b'i' => crate::bandit::create_bandit_exp3ix(scip, Some(priorities), d.nactiveneighborhoods, initseed),
        b'g' => crate::bandit::create_bandit_epsgreedy(scip, Some(priorities), d.epsgreedy_eps, false, false, 0.9, 0, d.nactiveneighborhoods, initseed),
        c => {
            log::error!("Unknown bandit parameter {}", c as char);
            Err(Retcode::InvalidData)
        }
    }
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyAlns`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_alns(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_alns(scip)
}

/// `alnsUnfixVariables`: unfixes some of the variables because there are
/// too many fixed; returns success.
fn alns_unfix_variables(scip: &mut Scip, heur: HeurId, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>, ntargetfixings: i32) -> ScipResult<bool> {
    let nbinintvars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;
    if nbinintvars == 0 {
        return Ok(false);
    }

    let nvars = api::get_n_vars(scip) as usize;
    let nfixings = varbuf.len();

    let mut isfixedvar = vec![false; nvars];
    let mut unfixedvars = Vec::with_capacity(nbinintvars);
    let mut distances = vec![0i32; nvars];
    let mut fixeddistances = vec![0i32; nfixings];
    let mut redcostscores = vec![0.0; nfixings];
    let mut randscores = vec![0.0; nfixings];
    let mut perm = vec![0i32; nfixings];
    let mut pscostscores = vec![0.0; nfixings];

    let varbufcpy = varbuf.clone();
    let valbufcpy = valbuf.clone();

    // Loop over fixed variables and mark their respective positions as fixed.
    for &v in varbuf.iter() {
        isfixedvar[scip.vars[v].probindex as usize] = true;
    }

    // Collect unfixed binary and integer variables.
    let vars = api::get_vars(scip).to_vec();
    for i in 0..nbinintvars {
        if !isfixedvar[i] {
            unfixedvars.push(vars[i]);
        }
    }

    let d = hd(scip, heur);
    let usedistances = d.usedistances && !unfixedvars.is_empty();
    let (useredcost, usepscost, uselocalredcost) = (d.useredcost, d.usepscost, d.uselocalredcost);

    // Collect distances of all fixed variables from those that are not fixed.
    if usedistances {
        VGraph::breadth_first(scip, None, &unfixedvars, &mut distances, i32::MAX, i32::MAX, i32::MAX)?;

        for i in 0..nfixings {
            let probindex = scip.vars[varbuf[i]].probindex;
            if probindex >= 0 {
                fixeddistances[i] = distances[probindex as usize];
            }
        }
    }

    // Collect reduced cost scores of the fixings and assign random scores.
    for i in 0..nfixings {
        let fixedvar = varbuf[i];
        let fixval = valbuf[i];

        // Use negative reduced cost and pseudo cost scores to prefer variable
        // fixings with small score.
        redcostscores[i] = -get_variable_redcost_score(scip, fixedvar, fixval, uselocalredcost);
        pscostscores[i] = -get_variable_pscost_score(scip, fixedvar, fixval, uselocalredcost);
        randscores[i] = hdm(scip, heur).bandit.as_mut().unwrap().rng().get_real(0.0, 1.0);
        perm[i] = i as i32;
    }

    let varprio = VarPrio {
        randscores: &randscores,
        distances: &fixeddistances,
        redcostscores: &redcostscores,
        pscostscores: &pscostscores,
        useredcost,
        usedistances,
        usepscost,
    };

    // Scores are assigned in such a way that variables with a smaller score
    // should be fixed last.
    crate::sort::select_down(&mut perm, (), |&a, &b| sort_ind_comp_alns(&varprio, a, b), ntargetfixings as usize);

    // Bring the desired variables to the front of the array.
    let nt = ntargetfixings as usize;
    for i in 0..nt {
        valbuf[i] = valbufcpy[perm[i] as usize];
        varbuf[i] = varbufcpy[perm[i] as usize];
    }
    varbuf.truncate(nt);
    valbuf.truncate(nt);

    Ok(true)
}

/// `neighborhoodFixVariables`: calls the variable fixing callback of this
/// neighborhood and orchestrates additional variable fixings, if necessary;
/// returns the result of the fixing operation.
fn neighborhood_fix_variables(scip: &mut Scip, heur: HeurId, idx: usize, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) -> ScipResult<ScipResultCode> {
    varbuf.clear();
    valbuf.clear();
    let mut result = ScipResultCode::DidNotRun;

    let targetfixingrate = nh(scip, heur, idx).fixingrate.targetfixingrate;
    let nbinintvars = api::get_n_bin_vars(scip) + api::get_n_int_vars(scip);
    let mut ntargetfixings = (targetfixingrate * nbinintvars as f64) as i32;

    if let Some(varfixings) = var_fixings_method(nh(scip, heur, idx).kind) {
        result = varfixings(scip, heur, idx, varbuf, valbuf)?;
        if result != ScipResultCode::Success {
            return Ok(result);
        }
    } else if ntargetfixings == 0 {
        return Ok(ScipResultCode::Success);
    }

    // Compute upper and lower target fixing limits using tolerance
    // parameters.
    let (fixtol, unfixtol) = (hd(scip, heur).fixtol, hd(scip, heur).unfixtol);
    ntargetfixings = (targetfixingrate * nbinintvars as f64) as i32;
    let mut nminfixings = ((targetfixingrate - fixtol) * nbinintvars as f64) as i32;
    nminfixings = if nminfixings >= 0 { nminfixings } else { 0 };
    let mut nmaxfixings = ((targetfixingrate + unfixtol) * nbinintvars as f64) as i32;
    nmaxfixings = if nmaxfixings <= nbinintvars { nmaxfixings } else { nbinintvars };

    // If too few fixings, use a strategy to select more variable fixings:
    // randomized, LP graph, ReducedCost based, mix.
    if (result == ScipResultCode::Success || result == ScipResultCode::DidNotRun) && (varbuf.len() as i32) < nminfixings {
        // Get reference solution from neighborhood.
        let refsol = neighborhood_get_refsol(scip, nh(scip, heur, idx));

        // Try to fix more variables based on the reference solution.
        let success = if refsol != NONE {
            alns_fix_more_variables(scip, heur, refsol, varbuf, valbuf, ntargetfixings)?
        } else {
            false
        };

        result = if success {
            ScipResultCode::Success
        } else if result == ScipResultCode::Success {
            ScipResultCode::DidNotFind
        } else {
            ScipResultCode::DidNotRun
        };
    } else if varbuf.len() as f64 > nmaxfixings as f64 {
        alns_unfix_variables(scip, heur, varbuf, valbuf, ntargetfixings)?;
        result = ScipResultCode::Success;
    }

    Ok(result)
}

/// `neighborhoodChangeSubscip`: changes the sub-SCIP by restricting
/// variable domains, changing objective coefficients, or adding
/// constraints; returns `(ndomchgs, nchgobjs, naddedconss, success)`.
fn neighborhood_change_subscip(sourcescip: &mut Scip, targetscip: &mut Scip, heur: HeurId, idx: usize, targetvars: &[VarId]) -> ScipResult<(i32, i32, i32, bool)> {
    let mut ndomchgs = 0;
    let mut nchgobjs = 0;
    let mut naddedconss = 0;
    let mut success = false;

    // Call the change sub-SCIP callback of the neighborhood.
    match nh(sourcescip, heur, idx).kind {
        NhKind::Rens => change_subscip_rens(sourcescip, targetscip, targetvars, &mut ndomchgs, &mut success)?,
        NhKind::LocalBranching => {
            let distance = (0.2 * api::get_n_bin_vars(sourcescip) as f64) as i32;
            add_local_branching_constraint(sourcescip, targetscip, targetvars, distance, &mut success, &mut naddedconss)?;
        }
        NhKind::Proximity => change_subscip_proximity(sourcescip, targetscip, targetvars, &mut nchgobjs, &mut success)?,
        NhKind::ZeroObjective => change_subscip_zeroobjective(sourcescip, targetscip, targetvars, &mut nchgobjs, &mut success)?,
        NhKind::Dins => change_subscip_dins(sourcescip, targetscip, targetvars, &mut ndomchgs, &mut naddedconss, &mut success)?,
        NhKind::TrustRegion => unimplemented!("the ALNS trust region neighborhood (SCIPaddTrustregionNeighborhoodConstraint, inactive by default)"),
        NhKind::Rins | NhKind::Mutation | NhKind::Crossover => success = true,
    }

    Ok((ndomchgs, nchgobjs, naddedconss, success))
}

/// `setLimits`: sets sub-SCIP solving limits.
fn set_limits(subscip: &mut Scip, solvelimits: &SolveLimits) -> ScipResult {
    paramset_set_longint(subscip, "limits/nodes", solvelimits.nodelimit)?;
    paramset_set_longint(subscip, "limits/stallnodes", solvelimits.stallnodes)?;
    paramset_set_real(subscip, "limits/time", solvelimits.timelimit)?;
    paramset_set_real(subscip, "limits/memory", solvelimits.memorylimit)?;
    Ok(())
}

/// `determineLimits`: determines limits for a sub-SCIP; returns whether
/// another sub-SCIP can be solved with them.
fn determine_limits(scip: &Scip, heur: HeurId, solvelimits: &mut SolveLimits, mut runagain: bool) -> ScipResult<bool> {
    let d = hd(scip, heur);
    let h = &scip.heurs[heur];

    // Check whether there is enough time and memory left.
    solvelimits.timelimit = crate::paramset::paramset_get_real(scip, "limits/time")?;
    if !scip.set.is_infinity(solvelimits.timelimit) {
        solvelimits.timelimit -= api::get_solving_time(scip);
    }
    solvelimits.memorylimit = crate::paramset::paramset_get_real(scip, "limits/memory")?;
    let avoidmemout = crate::paramset::paramset_get_bool(scip, "misc/avoidmemout")?;

    // Subtract the memory already used by the main SCIP and the estimated
    // memory usage of external software: SCIP's memory accounting is not
    // ported, both count as zero.

    // Abort if no time is left or not enough memory (we don't abort in this
    // case if misc_avoidmemout == FALSE) to create a copy of SCIP, including
    // external memory usage.
    if solvelimits.timelimit <= 0.0 || (avoidmemout && solvelimits.memorylimit <= 0.0) {
        runagain = false;
    }

    // If the heuristic is used to measure all rewards, it will always be
    // penalized here (the reward file is not ported).
    let mut nodesquot = d.nodesquot;
    nodesquot *= (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0);
    nodesquot = crate::def::max(nodesquot, d.nodesquotmin);

    // Calculate the search node limit of the heuristic.
    solvelimits.stallnodes = (nodesquot * api::get_n_nodes(scip) as f64) as i64;
    solvelimits.stallnodes += d.nodesoffset;
    solvelimits.stallnodes -= d.usednodes;
    solvelimits.stallnodes -= 100 * h.ncalls;
    solvelimits.stallnodes = if d.maxnodes <= solvelimits.stallnodes { d.maxnodes } else { solvelimits.stallnodes };

    // Use a smaller budget if not all neighborhoods have been initialized yet.
    let initfactor = (d.nactiveneighborhoods as f64 - d.ninitneighborhoods as f64 + 1.0) / (d.nactiveneighborhoods as f64 + 1.0);
    solvelimits.stallnodes = (solvelimits.stallnodes as f64 * initfactor) as i64;
    solvelimits.nodelimit = d.maxnodes;

    // Check whether we have enough nodes left to call subproblem solving.
    if solvelimits.stallnodes < d.targetnodes {
        runagain = false;
    }

    Ok(runagain)
}

/// `getReward`: calculates the reward based on the selected reward
/// measure.
fn get_reward(scip: &Scip, d: &HeurData, runstats: &NhStats) -> [f64; NREWARDTYPES] {
    let mut rewardptr = [0.0; NREWARDTYPES];
    let reward;

    let mut effort = runstats.usednodes as f64 / 100.0;

    let ndiscretevars = api::get_n_bin_vars(scip) + api::get_n_int_vars(scip);

    // Assume that every fixed variable linearly reduces the subproblem
    // complexity.
    if ndiscretevars > 0 {
        effort *= 1.0 - (runstats.nfixings as f64 / ndiscretevars as f64);
    }

    // A positive reward is only assigned if a new incumbent solution was
    // found.
    if runstats.nbestsolsfound > 0 {
        let rewardcontrol = d.rewardcontrol;

        // The indicator function is simply 1.0.
        rewardptr[REWARDTYPE_BESTSOL] = 1.0;
        rewardptr[REWARDTYPE_NOSOLPENALTY] = 1.0;

        let ub = runstats.newupperbound;
        let lb = api::get_lowerbound(scip);

        // Compute the closed gap reward.
        if scip.set.is_eq(ub, lb) || scip.set.is_infinity(runstats.oldupperbound) {
            rewardptr[REWARDTYPE_CLOSEDGAP] = 1.0;
        } else {
            rewardptr[REWARDTYPE_CLOSEDGAP] = (runstats.oldupperbound - ub) / (runstats.oldupperbound - lb);
        }

        // The reward is a convex combination of the best solution reward and
        // the reward for the closed gap.
        let mut r = rewardcontrol * rewardptr[REWARDTYPE_BESTSOL] + (1.0 - rewardcontrol) * rewardptr[REWARDTYPE_CLOSEDGAP];

        // Optionally, scale the reward by the involved effort.
        if d.scalebyeffort {
            r /= effort + 1.0;
        }

        // Add the baseline and rescale the reward into the interval
        // [baseline, 1.0].
        reward = d.rewardbaseline + (1.0 - d.rewardbaseline) * r;
    } else {
        // Linearly decrease the reward based on the number of nodes spent.
        let maxeffort = d.targetnodes as f64;
        let mut usednodes = runstats.usednodes as f64;

        if ndiscretevars > 0 {
            usednodes *= 1.0 - (runstats.nfixings as f64 / ndiscretevars as f64);
        }

        rewardptr[REWARDTYPE_NOSOLPENALTY] = 1.0 - (usednodes / maxeffort);
        rewardptr[REWARDTYPE_NOSOLPENALTY] = crate::def::max(0.0, rewardptr[REWARDTYPE_NOSOLPENALTY]);
        reward = d.rewardbaseline * rewardptr[REWARDTYPE_NOSOLPENALTY];
    }

    rewardptr[REWARDTYPE_TOTAL] = reward;
    rewardptr
}

/// `setupSubScip`: sets up the sub-SCIP parameters, objective cutoff, and
/// solution limits.
fn setup_sub_scip(scip: &Scip, subscip: &mut Scip, subvars: &[VarId], solvelimits: &SolveLimits, heur: HeurId, objchgd: bool) -> ScipResult {
    let d = hd(scip, heur);

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable output to console unless we are in debug mode.
    paramset_set_int(subscip, "display/verblevel", 0)?;

    // Disable statistic timing inside sub SCIP.
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    paramset_set_int(subscip, "limits/bestsol", d.nsolslim)?;

    // Forbid recursive call of heuristics and separators solving subMIPs.
    if !d.usesubscipheurs {
        crate::paramset::set_subscips_off(subscip, true)?;
    }

    // Disable cutting plane separation.
    crate::paramset::set_separating(subscip, ParamSetting::Off, true)?;

    // Disable expensive presolving.
    crate::paramset::set_presolving(subscip, ParamSetting::Fast, true)?;

    // Use best estimate node selection.
    if crate::set::set_find_nodesel(subscip, "estimate") != NONE && !crate::paramset::paramset_is_fixed(subscip, "nodeselection/estimate/stdpriority")? {
        paramset_set_int(subscip, "nodeselection/estimate/stdpriority", i32::MAX / 4)?;
    }

    // Use inference branching.
    if crate::set::set_find_branchrule(subscip, "inference") != NONE && !crate::paramset::paramset_is_fixed(subscip, "branching/inference/priority")? {
        paramset_set_int(subscip, "branching/inference/priority", i32::MAX / 4)?;
    }

    // Enable conflict analysis and restrict conflict pool.
    if !crate::paramset::paramset_is_fixed(subscip, "conflict/enable")? {
        paramset_set_bool(subscip, "conflict/enable", true)?;
    }
    if !crate::paramset::paramset_is_fixed(subscip, "conflict/useboundlp")? {
        paramset_set_char(subscip, "conflict/useboundlp", b'o')?;
    }
    if !crate::paramset::paramset_is_fixed(subscip, "conflict/maxstoresize")? {
        paramset_set_int(subscip, "conflict/maxstoresize", 100)?;
    }

    // Speed up sub-SCIP by not checking dual LP feasibility.
    paramset_set_bool(subscip, "lp/checkdualfeas", false)?;

    // Add an objective cutoff.
    if !scip.set.is_infinity(api::get_upperbound(scip)) {
        let upperbound = api::get_upperbound(scip) - scip.set.sumepsilon();
        let mut cutoff = if !scip.set.is_infinity(-1.0 * api::get_lowerbound(scip)) {
            (1.0 - d.minimprove) * api::get_upperbound(scip) + d.minimprove * api::get_lowerbound(scip)
        } else if api::get_upperbound(scip) >= 0.0 {
            (1.0 - d.minimprove) * api::get_upperbound(scip)
        } else {
            (1.0 + d.minimprove) * api::get_upperbound(scip)
        };
        cutoff = crate::def::min(upperbound, cutoff);

        if api::is_obj_integral(scip) {
            cutoff = scip.set.floor(cutoff);
        }

        // If the objective changed between the source and the target SCIP,
        // encode the cutoff as a constraint.
        if !objchgd {
            api::set_objlimit(subscip, cutoff)?;
        } else {
            let vars = api::get_vars(scip);
            let inf = subscip.set.infinity();
            let mut objcons = crate::cons_linear::create_cons_linear(
                subscip,
                "objbound_of_origscip",
                &[],
                &[],
                -inf,
                cutoff,
                true,
                true,
                true,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
            )?;
            for (i, &var) in vars.iter().enumerate() {
                let obj = scip.vars[var].obj;
                if !subscip.set.is_feas_zero(obj) {
                    crate::cons_linear::add_coef_linear(subscip, objcons, subvars[i], obj)?;
                }
            }
            crate::scip_cons::add_cons(subscip, objcons)?;
            crate::scip_cons::release_cons(subscip, &mut objcons)?;
        }
    }

    // Set solve limits for sub-SCIP.
    set_limits(subscip, solvelimits)?;

    // Change random seed of sub-SCIP.
    if d.subsciprandseeds {
        paramset_set_int(subscip, "randomization/randomseedshift", scip.heurs[heur].ncalls as i32)?;
    }

    Ok(())
}

/// `heurExecAlns`: execution method of primal heuristic.
fn heur_exec_alns(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let mut runstats = [NhStats::default(); NNEIGHBORHOODS];
    let mut subscipstatus = [Status::Unknown; NNEIGHBORHOODS];
    let mut rewards = [[0.0; NREWARDTYPES]; NNEIGHBORHOODS];

    *result = ScipResultCode::DidNotRun;

    if hd(scip, heur).nactiveneighborhoods == 0 {
        return Ok(());
    }

    // We only allow to run multiple times at a node during the root.
    if (heurtiming & HEURTIMING_DURINGLPLOOP) != 0 && (api::get_depth(scip) > 0 || !hd(scip, heur).initduringroot) {
        return Ok(());
    }

    // Update internal incumbent solution.
    let bestsol = api::get_best_sol(scip);
    if bestsol != hd(scip, heur).lastcallsol {
        let ncalls = scip.heurs[heur].ncalls;
        let d = hdm(scip, heur);
        d.lastcallsol = bestsol;
        d.firstcallthissol = ncalls;
    }

    // Do not run more than a user-defined number of times on each incumbent
    // (-1: no limit).
    let d = hd(scip, heur);
    if d.maxcallssamesol != -1 {
        let samesollimit = if d.maxcallssamesol > 0 {
            d.maxcallssamesol as i64
        } else {
            d.nactiveneighborhoods as i64
        };

        if scip.heurs[heur].ncalls - d.firstcallthissol >= samesollimit {
            return Ok(());
        }
    }

    // Wait for a sufficient number of nodes since last incumbent solution.
    if api::get_depth(scip) > 0 && bestsol != NONE && (api::get_n_nodes(scip) - scip.sols[bestsol].nodenum) < d.waitingnodes {
        return Ok(());
    }

    // Check if budget allows a run of the next selected neighborhood.
    let mut solvelimits = SolveLimits::default();
    let run = determine_limits(scip, heur, &mut solvelimits, true)?;

    if !run {
        return Ok(());
    }

    // Delay the heuristic if local reduced costs should be used for generic
    // variable unfixing.
    if hd(scip, heur).uselocalredcost && (nodeinfeasible || !api::has_current_node_lp(scip) || api::get_lp_solstat(scip) != LpSolStat::Optimal) {
        *result = ScipResultCode::Delayed;
        return Ok(());
    }

    // The all rewards mode (reward file) is not ported.

    // Use the neighborhood that requested a delay or select the next
    // neighborhood to run based on the selected bandit algorithm.
    let banditidx = if hd(scip, heur).currneighborhood >= 0 {
        hd(scip, heur).currneighborhood
    } else {
        hdm(scip, heur).bandit.as_mut().unwrap().select()
    };

    let mut neighborhoodidx = banditidx as usize;

    // Allocate memory for variable fixings buffer.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let mut varbuf = Vec::with_capacity(nvars);
    let mut valbuf = Vec::with_capacity(nvars);
    let mut subscip: Option<Box<Scip>> = None;

    // Initialize neighborhood statistics for a run.
    let mut ntries = 1;
    // The neighborhood of the last iteration (C's `neighborhood`).
    let mut neighborhood;
    loop {
        neighborhood = neighborhoodidx;

        let mut rs = NhStats::default();
        init_run_stats(scip, &mut rs);
        runstats[neighborhoodidx] = rs;
        rewards[neighborhoodidx][REWARDTYPE_TOTAL] = 0.0;
        subscipstatus[neighborhoodidx] = Status::Unknown;

        // Determine variable fixings and objective coefficients of this
        // neighborhood.
        let mut fixresult = neighborhood_fix_variables(scip, heur, neighborhoodidx, &mut varbuf, &mut valbuf)?;

        // Fixing was not successful, either because the fixing rate was not
        // reached (and no additional variable prioritization was used), or
        // the neighborhood requested a delay, e.g., because no LP relaxation
        // solution was available.
        if fixresult != ScipResultCode::Success {
            // Delay the heuristic along with the selected neighborhood. If the
            // neighborhood has been delayed for too many consecutive calls,
            // the delay is treated as a failure.
            if fixresult == ScipResultCode::Delayed {
                let freq = scip.heurs[heur].freq;
                let d = hdm(scip, heur);
                if d.ndelayedcalls > freq / 4 + 1 {
                    reset_current_neighborhood(d);

                    // Use SCIP_DIDNOTFIND to penalize the neighborhood with a bad
                    // reward.
                    fixresult = ScipResultCode::DidNotFind;
                } else if d.currneighborhood == -1 {
                    d.currneighborhood = neighborhoodidx as i32;
                    d.ndelayedcalls = 1;
                } else {
                    d.ndelayedcalls += 1;
                }
            }

            if fixresult == ScipResultCode::DidNotRun {
                if ntries < hd(scip, heur).nactiveneighborhoods {
                    let bandit = hdm(scip, heur).bandit.as_mut().unwrap();
                    bandit.update(neighborhoodidx as i32, 0.0);
                    neighborhoodidx = bandit.select() as usize;
                    ntries += 1;
                    if crate::solve::solve_is_stopped(scip, false) {
                        break;
                    }
                    continue;
                } else {
                    break;
                }
            }

            *result = fixresult;
            break;
        }

        *result = ScipResultCode::DidNotFind;

        nhm(scip, heur, neighborhoodidx).stats.nfixings += varbuf.len() as i32;
        runstats[neighborhoodidx].nfixings = varbuf.len() as i32;

        let mut sub = Scip::create()?;
        let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

        let probnamesuffix = format!("alns_{}", nh(scip, heur, neighborhoodidx).name);

        // Todo later: run global propagation for this set of fixings.
        let copycuts = hd(scip, heur).copycuts;
        crate::copy::copy_large_neighborhood_search(scip, &mut sub, &mut maps, &probnamesuffix, &varbuf, &valbuf, false, copycuts)?;

        // Store sub-SCIP variables in array for faster access.
        let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

        // Let the neighborhood add additional constraints, or restrict
        // domains.
        let (_ndomchgs, nchgobjs, _naddedconss, success) = neighborhood_change_subscip(scip, &mut sub, heur, neighborhoodidx, &subvars)?;

        if !success {
            subscip = Some(sub);
            break;
        }

        // Set up sub-SCIP parameters.
        setup_sub_scip(scip, &mut sub, &subvars, &solvelimits, heur, nchgobjs > 0)?;

        // Include an event handler to transfer solutions into the main SCIP.
        let eventdata = EventhdlrData {
            subvars: subvars.clone(),
            nodelimit: solvelimits.nodelimit,
            lplimfac: hd(scip, heur).lplimfac,
            transfers: Vec::new(),
        };
        let eventhdlr = api::include_eventhdlr_basic(&mut sub, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_alns, Some(Box::new(eventdata)))?;

        // Transform the problem before catching the events.
        crate::scip_solve::transform_prob(&mut sub)?;
        api::catch_event(&mut sub, EVENTTYPE_ALNS, eventhdlr, 0, None)?;

        // Set up sub-SCIP and run presolving.
        if let Err(retcode) = crate::scip_solve::presolve_prob(&mut sub) {
            log::warn!("Error while presolving subproblem in ALNS heuristic; sub-SCIP terminated with code <{retcode:?}>");
            subscip = Some(sub);
            break;
        }

        // Was presolving successful enough regarding fixings? otherwise,
        // terminate.
        let norigvars = sub.probs[crate::prob::ORIGPROB].nvars;
        let mut allfixingrate = (norigvars - api::get_n_vars(&sub)) as f64 / norigvars as f64;

        // Additional variables added in presolving may lead to the subSCIP
        // having more variables than the original.
        allfixingrate = crate::def::max(allfixingrate, 0.0);

        if allfixingrate >= nh(scip, heur, neighborhoodidx).fixingrate.targetfixingrate / 2.0 {
            // Run sub-SCIP for the given budget, and collect statistics.
            crate::scip_solve::solve(&mut sub)?;
        }

        // Make the solution transfers the event handler recorded.
        let transfers = std::mem::take(&mut sub.eventhdlrs[eventhdlr].eventhdlrdata.as_mut().unwrap().downcast_mut::<EventhdlrData>().unwrap().transfers);
        for values in &transfers {
            transfer_solution(scip, heur, values, &mut runstats[neighborhoodidx])?;
        }

        // Update statistics based on the sub-SCIP run results.
        update_run_stats(&mut runstats[neighborhoodidx], Some(&sub));
        subscipstatus[neighborhoodidx] = api::get_status(&sub);

        rewards[neighborhoodidx] = get_reward(scip, hd(scip, heur), &runstats[neighborhoodidx]);

        subscip = Some(sub);
        break;
    }

    // Free the sub-SCIP.
    drop(subscip);

    // Update bandit index that may have changed unless we are in all rewards
    // mode.
    let banditidx = neighborhoodidx;

    if *result != ScipResultCode::Delayed {
        // Decrease the number of neighborhoods that have not been initialized.
        if nh(scip, heur, neighborhood).stats.nruns == 0 {
            hdm(scip, heur).ninitneighborhoods -= 1;
        }

        hdm(scip, heur).usednodes += runstats[banditidx].usednodes;

        // Determine the success of this neighborhood, and update the target
        // fixing rate for the next time.
        let rs = runstats[banditidx];
        let status = subscipstatus[banditidx];
        update_neighborhood_stats(&rs, nhm(scip, heur, banditidx), status);

        // Adjust the fixing rate for this neighborhood; make no adjustments in
        // all rewards mode, because this only affects 1 of 8 heuristics.
        if hd(scip, heur).adjustfixingrate {
            update_fixing_rate(nhm(scip, heur, banditidx), status, &rs);
        }

        // Similarly, update the minimum improvement for the ALNS heuristic.
        if hd(scip, heur).adjustminimprove {
            update_minimum_improvement(hdm(scip, heur), status, &rs);
        }

        // Update the target node limit based on the status of the selected
        // algorithm.
        let ncalls = scip.heurs[heur].ncalls;
        if hd(scip, heur).adjusttargetnodes && ncalls >= hd(scip, heur).nactiveneighborhoods as i64 {
            update_target_node_limit(hdm(scip, heur), &rs, status);
        }

        // Update the bandit algorithm by the measured reward.
        let reward = rewards[banditidx][REWARDTYPE_TOTAL];
        let d = hdm(scip, heur);
        d.bandit.as_mut().unwrap().update(banditidx as i32, reward);

        reset_current_neighborhood(d);
    }

    Ok(())
}

/// The variable fixing callback of a neighborhood (C's `varfixings`).
type VarFixings = fn(&mut Scip, HeurId, usize, &mut Vec<VarId>, &mut Vec<f64>) -> ScipResult<ScipResultCode>;

fn var_fixings_method(kind: NhKind) -> Option<VarFixings> {
    match kind {
        NhKind::Rens => Some(var_fixings_rens),
        NhKind::Rins => Some(var_fixings_rins),
        NhKind::Mutation => Some(var_fixings_mutation),
        NhKind::Crossover => Some(var_fixings_crossover),
        NhKind::Dins => Some(var_fixings_dins),
        NhKind::LocalBranching | NhKind::Proximity | NhKind::ZeroObjective | NhKind::TrustRegion => None,
    }
}

/// `varFixingsRens`: callback to collect variable fixings of RENS.
fn var_fixings_rens(scip: &mut Scip, heur: HeurId, idx: usize, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) -> ScipResult<ScipResultCode> {
    if !api::has_current_node_lp(scip) {
        return Ok(ScipResultCode::Delayed);
    }
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(ScipResultCode::Delayed);
    }

    // Get variable information.
    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    // Return if no binary or integer variables are present.
    if nbinvars + nintvars == 0 {
        return Ok(ScipResultCode::DidNotRun);
    }

    let mut fracidx = Vec::with_capacity(nbinvars + nintvars);
    let mut frac = Vec::with_capacity(nbinvars + nintvars);

    // Loop over binary and integer variables; determine those that should be
    // fixed in the sub-SCIP.
    for (i, &var) in vars[..nbinvars + nintvars].iter().enumerate() {
        let lpsolval = crate::var::var_get_lp_sol(scip, var);

        // Fix all binary and integer variables with integer LP solution value.
        if scip.set.is_feas_integral(lpsolval) {
            try_add2variable_buffer(scip, var, lpsolval, varbuf, valbuf, true);
        } else {
            let f = scip.set.frac(lpsolval);
            frac.push(crate::def::min(f, 1.0 - f));
            fracidx.push(i as i32);
        }
    }

    // Do some additional fixing.
    let targetfixingrate = nh(scip, heur, idx).fixingrate.targetfixingrate;
    let target = targetfixingrate * (nbinvars + nintvars) as f64;
    if (varbuf.len() as f64) < target && !frac.is_empty() {
        crate::sort::sort_down(&mut frac, (&mut fracidx[..],), crate::sort::cmp_real);

        // Prefer variables that are almost integer.
        let mut i = 0;
        while i < frac.len() && (varbuf.len() as f64) < target {
            let var = vars[fracidx[i] as usize];
            let val = scip.set.round(crate::var::var_get_lp_sol(scip, var));
            try_add2variable_buffer(scip, var, val, varbuf, valbuf, true);
            i += 1;
        }
    }

    Ok(ScipResultCode::Success)
}

/// `changeSubscipRens`: callback for RENS subproblem changes.
fn change_subscip_rens(sourcescip: &Scip, targetscip: &mut Scip, subvars: &[VarId], ndomchgs: &mut i32, success: &mut bool) -> ScipResult {
    // Get variable information.
    let vars = api::get_vars(sourcescip);
    let nbinvars = api::get_n_bin_vars(sourcescip) as usize;
    let nintvars = api::get_n_int_vars(sourcescip) as usize;

    // Restrict bounds of integer variables with fractional solution value.
    for i in nbinvars..nbinvars + nintvars {
        let var = vars[i];
        let lpsolval = api::get_sol_val(sourcescip, NONE, var);

        if subvars[i] == NONE {
            continue;
        }

        if !sourcescip.set.is_feas_integral(lpsolval) {
            let newlb = sourcescip.set.floor(lpsolval);
            let newub = newlb + 1.0;

            // Only count this as a domain change if the new lower and upper
            // bound are a further restriction.
            let sv = &targetscip.vars[subvars[i]];
            if newlb > sv.glbdom.lb + 0.5 || newub < sv.glbdom.ub - 0.5 {
                crate::scip_var::chg_var_lb_global(targetscip, subvars[i], newlb)?;
                crate::scip_var::chg_var_ub_global(targetscip, subvars[i], newub)?;
                *ndomchgs += 1;
            }
        }
    }

    *success = true;
    Ok(())
}

/// `fixMatchingSolutionValues`: collects fixings by matching solution
/// values in a collection of solutions (`NONE` stands for the LP solution)
/// for the given variables (`None`: all binary and integer variables).
fn fix_matching_solution_values(scip: &Scip, sols: &[SolId], vars: Option<&[VarId]>, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) {
    let allvars;
    let vars = match vars {
        Some(v) => v,
        None => {
            let nbinintvars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;
            allvars = api::get_vars(scip)[..nbinintvars].to_vec();
            &allvars
        }
    };

    let firstsol = sols[0];

    // Loop over integer and binary variables and check if their solution
    // values match in all solutions.
    for &var in vars {
        let solval = api::get_sol_val(scip, firstsol, var);

        // Determine if solution values match in all given solutions.
        let mut s = 1;
        while s < sols.len() {
            if !scip.set.is_feas_zero(solval - api::get_sol_val(scip, sols[s], var)) {
                break;
            }
            s += 1;
        }

        // If we did not break early, all solutions agree on the solution value
        // of this variable.
        if s == sols.len() {
            try_add2variable_buffer(scip, var, solval, varbuf, valbuf, true);
        }
    }
}

/// `varFixingsRins`: callback to collect variable fixings of RINS.
fn var_fixings_rins(scip: &mut Scip, _heur: HeurId, _idx: usize, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) -> ScipResult<ScipResultCode> {
    if !api::has_current_node_lp(scip) {
        return Ok(ScipResultCode::Delayed);
    }
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(ScipResultCode::Delayed);
    }

    let incumbent = api::get_best_sol(scip);
    if incumbent == NONE {
        return Ok(ScipResultCode::DidNotRun);
    }

    if scip.sols[incumbent].solorigin == SolOrigin::Original {
        return Ok(ScipResultCode::DidNotRun);
    }

    // Get variable information.
    let nbinintvars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;

    // Return if no binary or integer variables are present.
    if nbinintvars == 0 {
        return Ok(ScipResultCode::DidNotRun);
    }

    // Incumbent is reference, the LP solution the other one.
    let vars = api::get_vars(scip)[..nbinintvars].to_vec();
    fix_matching_solution_values(scip, &[incumbent, NONE], Some(&vars), varbuf, valbuf);

    Ok(ScipResultCode::Success)
}

/// `varFixingsCrossover`: callback to collect variable fixings of
/// crossover.
fn var_fixings_crossover(scip: &mut Scip, heur: HeurId, idx: usize, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) -> ScipResult<ScipResultCode> {
    let k = hd(scip, heur).neighborhoods[idx];
    let mut nsols = hd(scip, heur).nhs[k].crossover_nsols;
    hdm(scip, heur).nhs[k].crossover_selsol = NONE;

    // Return if the pool has not enough solutions.
    if nsols > api::get_n_sols(scip) {
        return Ok(ScipResultCode::DidNotRun);
    }

    // Return if no binary or integer variables are present.
    if api::get_n_bin_vars(scip) + api::get_n_int_vars(scip) == 0 {
        return Ok(ScipResultCode::DidNotRun);
    }

    let mut lastdraw = api::get_n_sols(scip);
    let scipsols = api::get_sols(scip).to_vec();
    let mut sols = vec![NONE; nsols as usize];

    // Draw as many solutions from the pool as required by crossover, biased
    // towards better solutions; therefore, the sorting of the solutions by
    // objective is exploited.
    while nsols > 0 {
        if lastdraw == nsols {
            // No need for randomization anymore, exactly nsols many solutions
            // remain for the selection: fill the remaining slots 0,...,nsols -
            // 1 by the solutions at the same places.
            sols[..nsols as usize].copy_from_slice(&scipsols[..nsols as usize]);
            nsols = 0;
        } else {
            // Draw from the lastdraw - nsols many solutions nsols - 1, ...
            // lastdraw - 1 such that nsols many solution remain in the next
            // round.
            let nextdraw = hdm(scip, heur).nhs[k].crossover_rng.as_mut().unwrap().get_int(nsols - 1, lastdraw - 1);
            sols[(nsols - 1) as usize] = scipsols[nextdraw as usize];
            nsols -= 1;
            lastdraw = nextdraw;
        }
    }

    fix_matching_solution_values(scip, &sols, None, varbuf, valbuf);

    // Store best selected solution as reference solution.
    hdm(scip, heur).nhs[k].crossover_selsol = sols[0];

    Ok(ScipResultCode::Success)
}

/// `varFixingsMutation`: callback to collect variable fixings of mutation.
fn var_fixings_mutation(scip: &mut Scip, heur: HeurId, idx: usize, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) -> ScipResult<ScipResultCode> {
    let k = hd(scip, heur).neighborhoods[idx];

    // Get the problem variables.
    let nbinintvars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;
    if nbinintvars == 0 {
        return Ok(ScipResultCode::DidNotRun);
    }

    let incumbentsol = api::get_best_sol(scip);
    if incumbentsol == NONE {
        return Ok(ScipResultCode::DidNotRun);
    }

    let targetfixingrate = hd(scip, heur).nhs[k].fixingrate.targetfixingrate;
    let ntargetfixings = (targetfixingrate * nbinintvars as f64) as i32 + 1;

    // Don't continue if number of discrete variables is too small to reach
    // target fixing rate.
    if nbinintvars as i32 <= ntargetfixings {
        return Ok(ScipResultCode::DidNotRun);
    }

    let mut result = ScipResultCode::DidNotFind;

    // Copy variables into a buffer array.
    let mut varscpy = api::get_vars(scip)[..nbinintvars].to_vec();

    // Partially perturb the array until the number of target fixings is
    // reached.
    let mut i = 0;
    while (varbuf.len() as i32) < ntargetfixings && i < nbinintvars {
        let randint = hdm(scip, heur).nhs[k].mutation_rng.as_mut().unwrap().get_int(i as i32, nbinintvars as i32 - 1) as usize;
        if randint > i {
            varscpy.swap(i, randint);
        }

        // Copy the selected variables and their solution values into the
        // buffer.
        let val = api::get_sol_val(scip, incumbentsol, varscpy[i]);
        try_add2variable_buffer(scip, varscpy[i], val, varbuf, valbuf, true);
        i += 1;
    }

    // Not reaching the number of target fixings means that there is a
    // significant fraction (at least 1 - targetfixingrate) of variables for
    // which the incumbent solution value does not lie within the global
    // bounds anymore. This is a nonsuccess for the neighborhood (additional
    // fixings are not possible), which is okay because the incumbent solution
    // is significantly outdated.
    if varbuf.len() as i32 == ntargetfixings {
        result = ScipResultCode::Success;
    }

    Ok(result)
}

/// `addLocalBranchingConstraint`: adds the local branching constraint.
fn add_local_branching_constraint(sourcescip: &Scip, targetscip: &mut Scip, subvars: &[VarId], distance: i32, success: &mut bool, naddedconss: &mut i32) -> ScipResult {
    let nbinvars = api::get_n_bin_vars(sourcescip) as usize;
    let vars = api::get_vars(sourcescip);

    if nbinvars <= 3 {
        return Ok(());
    }

    let referencesol = api::get_best_sol(sourcescip);
    if referencesol == NONE {
        return Ok(());
    }

    let mut rhs = distance as f64;
    rhs = crate::def::max(rhs, 2.0);

    let mut consvals = vec![0.0; nbinvars];

    // Loop over binary variables and fill the local branching constraint.
    for i in 0..nbinvars {
        // Skip variables that are not present in sub-SCIP.
        if subvars[i] == NONE {
            continue;
        }

        if sourcescip.set.is_eq(api::get_sol_val(sourcescip, referencesol, vars[i]), 0.0) {
            consvals[i] = 1.0;
        } else {
            consvals[i] = -1.0;
            rhs -= 1.0;
        }
    }

    // Create the local branching constraint in the target scip.
    let inf = sourcescip.set.infinity();
    let mut localbranchcons = crate::cons_linear::create_cons_basic_linear(targetscip, "localbranch", &subvars[..nbinvars], &consvals, -inf, rhs)?;
    crate::scip_cons::add_cons(targetscip, localbranchcons)?;
    crate::scip_cons::release_cons(targetscip, &mut localbranchcons)?;

    *naddedconss = 1;
    *success = true;
    Ok(())
}

/// `changeSubscipProximity`: callback for proximity subproblem changes.
fn change_subscip_proximity(sourcescip: &Scip, targetscip: &mut Scip, subvars: &[VarId], nchgobjs: &mut i32, success: &mut bool) -> ScipResult {
    let vars = api::get_vars(sourcescip);
    let nvars = vars.len();
    let nbinvars = api::get_n_bin_vars(sourcescip) as usize;

    if nbinvars == 0 {
        return Ok(());
    }

    let referencesol = api::get_best_sol(sourcescip);
    if referencesol == NONE {
        return Ok(());
    }

    // Loop over binary variables, set objective coefficients based on
    // reference solution in a local branching fashion.
    let mut i = 0;
    while i < nbinvars {
        // Skip variables not present in sub-SCIP.
        if subvars[i] != NONE {
            let newobj = if api::get_sol_val(sourcescip, referencesol, vars[i]) < 0.5 { -1.0 } else { 1.0 };
            crate::scip_var::chg_var_obj(targetscip, subvars[i], newobj)?;
        }
        i += 1;
    }

    // Loop over the remaining variables and change their objective
    // coefficients to 0.
    while i < nvars {
        // Skip variables not present in sub-SCIP.
        if subvars[i] != NONE {
            crate::scip_var::chg_var_obj(targetscip, subvars[i], 0.0)?;
        }
        i += 1;
    }

    *nchgobjs = nvars as i32;
    *success = true;
    Ok(())
}

/// `changeSubscipZeroobjective`: callback for zeroobjective subproblem
/// changes.
fn change_subscip_zeroobjective(sourcescip: &Scip, targetscip: &mut Scip, subvars: &[VarId], nchgobjs: &mut i32, success: &mut bool) -> ScipResult {
    let nvars = api::get_n_vars(sourcescip) as usize;

    // Do not run if no objective variables are present.
    if api::get_n_obj_vars(sourcescip) == 0 {
        return Ok(());
    }

    // Loop over the variables and change their objective coefficients to 0.
    for &sv in &subvars[..nvars] {
        // Skip variables not present in sub-SCIP.
        if sv == NONE {
            continue;
        }
        crate::scip_var::chg_var_obj(targetscip, sv, 0.0)?;
    }

    *nchgobjs = nvars as i32;
    *success = true;
    Ok(())
}

/// `computeIntegerVariableBoundsDins`: computes tightened bounds for integer
/// variables depending on how much the LP and the incumbent solution values
/// differ; returns `(lb, ub)`.
fn compute_integer_variable_bounds_dins(scip: &Scip, var: VarId) -> (f64, f64) {
    // Get the bounds for each variable.
    let lbglobal = scip.vars[var].glbdom.lb;
    let ubglobal = scip.vars[var].glbdom.ub;

    // Get the current LP solution for each variable.
    let lpsol = crate::var::var_get_lp_sol(scip, var);

    // Get the current MIP solution for each variable.
    let bestsol = api::get_best_sol(scip);
    let mipsol = api::get_sol_val(scip, bestsol, var);

    let (mut lb, mut ub);

    // If the solution values differ by 0.5 or more, the variable is
    // rebounded, otherwise it is just copied.
    if (lpsol - mipsol).abs() >= 0.5 {
        lb = lbglobal;
        ub = ubglobal;

        // Create an equally sized range around lpsol for general integers:
        // bounds are lpsol +- (mipsol-lpsol).
        let mut range = 2.0 * lpsol - mipsol;

        if mipsol >= lpsol {
            range = scip.set.feas_ceil(range);
            lb = crate::def::max(lb, range);

            // When the bound new upper bound is equal to the current MIP
            // solution, we set both bounds to the integral bound (without
            // eps).
            if scip.set.is_feas_eq(mipsol, lb) {
                ub = lb;
            } else {
                ub = mipsol;
            }
        } else {
            range = scip.set.feas_floor(range);
            ub = crate::def::min(ub, range);

            // When the bound new upper bound is equal to the current MIP
            // solution, we set both bounds to the integral bound (without
            // eps).
            if scip.set.is_feas_eq(mipsol, ub) {
                lb = ub;
            } else {
                lb = mipsol;
            }
        }

        // The global domain of variables might have been reduced since
        // incumbent was found: adjust lb and ub accordingly.
        lb = crate::def::max(lb, lbglobal);
        ub = crate::def::min(ub, ubglobal);
    } else {
        // The global domain of variables might have been reduced since
        // incumbent was found: adjust it accordingly.
        lb = crate::def::max(mipsol, lbglobal);
        ub = crate::def::min(mipsol, ubglobal);
    }

    (lb, ub)
}

/// `varFixingsDins`: callback to collect variable fixings of DINS.
fn var_fixings_dins(scip: &mut Scip, heur: HeurId, idx: usize, varbuf: &mut Vec<VarId>, valbuf: &mut Vec<f64>) -> ScipResult<ScipResultCode> {
    let npoolsols = nh(scip, heur, idx).dins_npoolsols;
    let nmipsols = api::get_n_sols(scip);
    let nmipsols = if nmipsols <= npoolsols { nmipsols } else { npoolsols } as usize;

    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(ScipResultCode::Delayed);
    }

    if nmipsols == 0 {
        return Ok(ScipResultCode::DidNotRun);
    }

    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;
    if nbinvars + nintvars == 0 {
        return Ok(ScipResultCode::DidNotRun);
    }

    let rootlpsol = api::create_sol(scip, NONE)?;

    // Save root solution LP values in solution.
    for &var in &vars[..nbinvars + nintvars] {
        let rootsol = crate::var::var_get_root_sol(scip, var);
        api::set_sol_val(scip, rootlpsol, var, rootsol)?;
    }

    // Add the node and the root LP solution; the incumbent is reference.
    let mut sols: Vec<SolId> = api::get_sols(scip)[..nmipsols].to_vec();
    sols.push(NONE);
    sols.push(rootlpsol);

    // 1. Binary variables are fixed if their values agree in all the
    // solutions.
    if nbinvars > 0 {
        fix_matching_solution_values(scip, &sols, Some(&vars[..nbinvars]), varbuf, valbuf);
    }

    // 2. Integer variables are fixed if they have a very low distance between
    // the incumbent and the root LP solution (C loops from nbinvars to
    // nintvars).
    for &var in vars.iter().take(nintvars).skip(nbinvars) {
        let (lb, ub) = compute_integer_variable_bounds_dins(scip, var);
        if ub - lb < 0.5 {
            try_add2variable_buffer(scip, var, lb, varbuf, valbuf, true);
        }
    }

    api::free_sol(scip, rootlpsol)?;

    Ok(ScipResultCode::Success)
}

/// `changeSubscipDins`: callback for DINS subproblem changes.
fn change_subscip_dins(sourcescip: &Scip, targetscip: &mut Scip, subvars: &[VarId], ndomchgs: &mut i32, naddedconss: &mut i32, success: &mut bool) -> ScipResult {
    let vars = api::get_vars(sourcescip);
    let nbinvars = api::get_n_bin_vars(sourcescip) as usize;
    let nintvars = api::get_n_int_vars(sourcescip) as usize;

    // 1. Loop over integer variables and tighten the bounds (C loops from
    // nbinvars to nintvars).
    for v in nbinvars..nintvars {
        // Skip variables not present in sub-SCIP.
        if subvars[v] == NONE {
            continue;
        }

        let (lb, ub) = compute_integer_variable_bounds_dins(sourcescip, vars[v]);
        crate::scip_var::chg_var_lb_global(targetscip, subvars[v], lb)?;
        crate::scip_var::chg_var_ub_global(targetscip, subvars[v], ub)?;
        *ndomchgs += 1;
    }

    // 2. Add local branching constraint for binary variables.
    let distance = (0.1 * api::get_n_bin_vars(sourcescip) as f64) as i32;
    add_local_branching_constraint(sourcescip, targetscip, subvars, distance, success, naddedconss)?;

    *success = true;
    Ok(())
}

/// `nhDeactivate...`: whether a neighborhood is irrelevant for the problem.
fn nh_deactivate(scip: &Scip, kind: NhKind) -> bool {
    match kind {
        // `nhDeactivateDiscreteVars`: deactivate if no discrete variables are
        // present.
        NhKind::Rens | NhKind::Rins | NhKind::Mutation | NhKind::Crossover => api::get_n_bin_vars(scip) + api::get_n_int_vars(scip) == 0,
        // `nhDeactivateBinVars`.
        NhKind::LocalBranching | NhKind::Proximity | NhKind::Dins | NhKind::TrustRegion => api::get_n_bin_vars(scip) == 0,
        // `nhDeactivateObjVars`.
        NhKind::ZeroObjective => api::get_n_obj_vars(scip) == 0,
    }
}

/// `heurInitAlns`: initialization method of primal heuristic (called after
/// problem was transformed).
fn heur_init_alns(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reactivate all neighborhoods if a new problem is read in.
    let d = hdm(scip, heur);
    d.nactiveneighborhoods = d.neighborhoods.len() as i32;

    // Initialize neighborhoods for new problem.
    for i in 0..hd(scip, heur).neighborhoods.len() {
        let k = hd(scip, heur).neighborhoods[i];
        neighborhood_init(scip, heur, k);
        let n = &mut hdm(scip, heur).nhs[k];
        reset_fixing_rate(&mut n.fixingrate);
        neighborhood_stats_reset(&mut n.stats);
    }

    // Open reward file for reading.
    if hd(scip, heur).rewardfilename != DEFAULT_REWARDFILENAME {
        unimplemented!("heuristics/alns/rewardfilename (the all rewards mode)");
    }

    Ok(())
}

/// `heurInitsolAlns`: solving process initialization method of primal
/// heuristic (called when branch and bound process is about to begin).
fn heur_initsol_alns(scip: &mut Scip, heur: HeurId) -> ScipResult {
    let nneighborhoods = hd(scip, heur).neighborhoods.len();
    hdm(scip, heur).nactiveneighborhoods = nneighborhoods as i32;

    // Init neighborhoods for new problem by resetting their statistics and
    // fixing rate.
    for i in (0..nneighborhoods).rev() {
        let n = nh(scip, heur, i);
        let deactivate = nh_deactivate(scip, n.kind);

        // Disable inactive neighborhoods.
        if deactivate || !n.active {
            let d = hdm(scip, heur);
            if d.nactiveneighborhoods as usize - 1 > i {
                let last = d.nactiveneighborhoods as usize - 1;
                d.neighborhoods.swap(i, last);
            }
            d.nactiveneighborhoods -= 1;
        }
    }

    // Collect neighborhood priorities.
    let nactive = hd(scip, heur).nactiveneighborhoods as usize;
    let priorities: Vec<f64> = (0..nactive).map(|i| nh(scip, heur, i).priority).collect();

    let initseed = (hd(scip, heur).seed + api::get_n_vars(scip)) as u32;

    // Active neighborhoods might change between init calls, reset
    // functionality must take this into account.
    let d = hdm(scip, heur);
    if d.bandit.as_ref().is_some_and(|b| b.get_n_actions() != d.nactiveneighborhoods) {
        d.bandit = None;
    }

    if nactive > 0 {
        // Create or reset bandit algorithm.
        if hd(scip, heur).bandit.is_none() {
            let bandit = create_bandit(scip, hd(scip, heur), &priorities, initseed)?;
            let d = hdm(scip, heur);
            d.bandit = Some(bandit);
            reset_minimum_improvement(d);
            reset_target_node_limit(d);
        } else if hd(scip, heur).resetweights {
            // `SCIPresetBandit`.
            let seed = scip.set.initialize_random_seed(initseed);
            let d = hdm(scip, heur);
            d.bandit.as_mut().unwrap().reset(Some(&priorities), seed)?;
            reset_minimum_improvement(d);
            reset_target_node_limit(d);
        }
    }

    let d = hdm(scip, heur);
    d.usednodes = 0;
    d.ninitneighborhoods = d.nactiveneighborhoods;
    d.lastcallsol = NONE;
    d.firstcallthissol = 0;

    reset_current_neighborhood(d);

    Ok(())
}

/// `heurExitAlns`: deinitialization method of primal heuristic (called
/// before transformed problem is freed).
fn heur_exit_alns(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free neighborhood specific data.
    for n in hdm(scip, heur).nhs.iter_mut() {
        neighborhood_exit(n);
    }
    Ok(())
}

/// A neighborhood with its defaults (`alnsIncludeNeighborhood`).
fn new_neighborhood(name: &'static str, kind: NhKind, minfixingrate: f64, maxfixingrate: f64, active: bool, priority: f64) -> Nh {
    Nh {
        name,
        fixingrate: NhFixingRate {
            minfixingrate,
            targetfixingrate: 0.0,
            increment: 0.0,
            maxfixingrate,
        },
        stats: NhStats::default(),
        kind,
        active,
        priority,
        mutation_rng: None,
        crossover_nsols: DEFAULT_NSOLS_CROSSOVER,
        crossover_rng: None,
        crossover_selsol: NONE,
        dins_npoolsols: DEFAULT_NPOOLSOLS_DINS,
        trustregion_violpenalty: DEFAULT_VIOLPENALTY_TRUSTREGION,
    }
}

/// A [`Target`] on a field of neighborhood `k` of the heuristic `heur`.
macro_rules! nh_target {
    ($ty:ty, $heur:expr, $k:expr, $($field:tt)+) => {{
        let (heur, k) = ($heur, $k);
        Some(Rc::new(Target::<$ty> {
            get: Box::new(move |scip: &Scip| &hd(scip, heur).nhs[k].$($field)+),
            get_mut: Box::new(move |scip: &mut Scip| &mut hdm(scip, heur).nhs[k].$($field)+),
        }))
    }};
}

/// `includeNeighborhoods` with the parameters of `alnsIncludeNeighborhood`:
/// includes all neighborhoods.
fn include_neighborhoods(scip: &mut Scip, heur: HeurId) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    let defs: [(&'static str, NhKind, f64, f64, bool, f64); NNEIGHBORHOODS] = [
        ("rens", NhKind::Rens, 0.3, 0.9, true, 1.0),
        ("rins", NhKind::Rins, 0.3, 0.9, true, 1.0),
        ("mutation", NhKind::Mutation, 0.3, 0.9, true, 1.0),
        ("localbranching", NhKind::LocalBranching, 0.3, 0.9, true, 1.0),
        ("crossover", NhKind::Crossover, 0.3, 0.9, true, 1.0),
        ("proximity", NhKind::Proximity, 0.3, 0.9, true, 1.0),
        ("zeroobjective", NhKind::ZeroObjective, 0.3, 0.9, true, 1.0),
        ("dins", NhKind::Dins, 0.3, 0.9, true, 1.0),
        ("trustregion", NhKind::TrustRegion, 0.3, 0.9, false, 1.0),
    ];

    for (k, &(name, kind, minfixingrate, maxfixingrate, active, priority)) in defs.iter().enumerate() {
        let d = hdm(scip, heur);
        d.nhs.push(new_neighborhood(name, kind, minfixingrate, maxfixingrate, active, priority));
        d.neighborhoods.push(k);

        // Add parameters for this neighborhood.
        paramset_add_real(
            scip,
            &format!("heuristics/alns/{name}/minfixingrate"),
            "minimum fixing rate for this neighborhood",
            nh_target!(f64, heur, k, fixingrate.minfixingrate),
            true,
            minfixingrate,
            0.0,
            1.0,
            None,
        )?;
        paramset_add_real(
            scip,
            &format!("heuristics/alns/{name}/maxfixingrate"),
            "maximum fixing rate for this neighborhood",
            nh_target!(f64, heur, k, fixingrate.maxfixingrate),
            true,
            maxfixingrate,
            0.0,
            1.0,
            None,
        )?;
        paramset_add_bool(
            scip,
            &format!("heuristics/alns/{name}/active"),
            "is this neighborhood active?",
            nh_target!(bool, heur, k, active),
            true,
            active,
            None,
        )?;
        paramset_add_real(
            scip,
            &format!("heuristics/alns/{name}/priority"),
            "positive call priority to initialize bandit algorithms",
            nh_target!(f64, heur, k, priority),
            true,
            priority,
            1e-2,
            1.0,
            None,
        )?;

        // Add crossover neighborhood parameters.
        if kind == NhKind::Crossover {
            paramset_add_int(
                scip,
                "heuristics/alns/crossover/nsols",
                "the number of solutions that crossover should combine",
                nh_target!(i32, heur, k, crossover_nsols),
                true,
                DEFAULT_NSOLS_CROSSOVER,
                2,
                10,
                None,
            )?;
        }

        // Add DINS neighborhood parameters.
        if kind == NhKind::Dins {
            paramset_add_int(
                scip,
                "heuristics/alns/dins/npoolsols",
                "number of pool solutions where binary solution values must agree",
                nh_target!(i32, heur, k, dins_npoolsols),
                true,
                DEFAULT_NPOOLSOLS_DINS,
                1,
                100,
                None,
            )?;
        }

        // Add trust region neighborhood parameters.
        if kind == NhKind::TrustRegion {
            paramset_add_real(
                scip,
                "heuristics/alns/trustregion/violpenalty",
                "the penalty for each change in the binary variables from the candidate solution",
                nh_target!(f64, heur, k, trustregion_violpenalty),
                false,
                DEFAULT_VIOLPENALTY_TRUSTREGION,
                0.0,
                crate::def::REAL_MAX,
                None,
            )?;
        }
    }
    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurAlns`: creates the alns primal heuristic and includes it
/// in SCIP.
pub fn include_heur_alns(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_longint, paramset_add_real, paramset_add_string};

    // Create alns primal heuristic data (C clears it).
    let heurdata = HeurData {
        nhs: Vec::with_capacity(NNEIGHBORHOODS),
        neighborhoods: Vec::with_capacity(NNEIGHBORHOODS),
        bandit: None,
        lastcallsol: NONE,
        rewardfilename: String::new(),
        nodesoffset: 0,
        maxnodes: 0,
        targetnodes: 0,
        minnodes: 0,
        usednodes: 0,
        waitingnodes: 0,
        nodesquot: 0.0,
        nodesquotmin: 0.0,
        startminimprove: 0.0,
        minimprovelow: 0.0,
        minimprovehigh: 0.0,
        minimprove: 0.0,
        lplimfac: LPLIMFAC,
        exp3_gamma: 0.0,
        exp3_beta: 0.0,
        epsgreedy_eps: 0.0,
        ucb_alpha: 0.0,
        rewardcontrol: 0.0,
        targetnodefactor: 0.0,
        rewardbaseline: 0.0,
        fixtol: 0.0,
        unfixtol: 0.0,
        nactiveneighborhoods: 0,
        ninitneighborhoods: 0,
        nsolslim: 0,
        seed: 0,
        currneighborhood: 0,
        ndelayedcalls: 0,
        maxcallssamesol: 0,
        firstcallthissol: 0,
        banditalgo: 0,
        useredcost: false,
        usedistances: false,
        usepscost: false,
        domorefixings: false,
        adjustfixingrate: false,
        usesubscipheurs: false,
        adjustminimprove: false,
        adjusttargetnodes: false,
        resetweights: false,
        subsciprandseeds: false,
        scalebyeffort: false,
        copycuts: false,
        uselocalredcost: false,
        initduringroot: false,
        shownbstats: false,
    };

    // Include primal heuristic.
    let heur = api::include_heur_basic(
        scip,
        HEUR_NAME,
        HEUR_DESC,
        HEUR_DISPCHAR,
        HEUR_PRIORITY,
        HEUR_FREQ,
        HEUR_FREQOFS,
        HEUR_MAXDEPTH,
        HEUR_TIMING,
        HEUR_USESSUBSCIP,
        heur_exec_alns,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Include all neighborhoods.
    include_neighborhoods(scip, heur)?;

    // Set non fundamental callbacks via setter functions.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_alns);
    h.heurinit = Some(heur_init_alns);
    h.heurinitsol = Some(heur_initsol_alns);
    h.heurexit = Some(heur_exit_alns);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    paramset_add_bool(
        scip,
        "heuristics/alns/shownbstats",
        "show statistics on neighborhoods?",
        t!(bool, shownbstats),
        true,
        DEFAULT_SHOWNBSTATS,
        None,
    )?;

    // Add alns primal heuristic parameters.
    paramset_add_longint(
        scip,
        "heuristics/alns/maxnodes",
        "maximum number of nodes to regard in the subproblem",
        t!(i64, maxnodes),
        true,
        DEFAULT_MAXNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/alns/nodesofs",
        "offset added to the nodes budget",
        t!(i64, nodesoffset),
        false,
        DEFAULT_NODESOFFSET,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/alns/minnodes",
        "minimum number of nodes required to start a sub-SCIP",
        t!(i64, minnodes),
        true,
        DEFAULT_MINNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/alns/waitingnodes",
        "number of nodes since last incumbent solution that the heuristic should wait",
        t!(i64, waitingnodes),
        true,
        DEFAULT_WAITINGNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/nodesquot",
        "fraction of nodes compared to the main SCIP for budget computation",
        t!(f64, nodesquot),
        false,
        DEFAULT_NODESQUOT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/nodesquotmin",
        "lower bound fraction of nodes compared to the main SCIP for budget computation",
        t!(f64, nodesquotmin),
        false,
        DEFAULT_NODESQUOTMIN,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/startminimprove",
        "initial factor by which ALNS should at least improve the incumbent",
        t!(f64, startminimprove),
        true,
        DEFAULT_STARTMINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/minimprovelow",
        "lower threshold for the minimal improvement over the incumbent",
        t!(f64, minimprovelow),
        true,
        DEFAULT_MINIMPROVELOW,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/minimprovehigh",
        "upper bound for the minimal improvement over the incumbent",
        t!(f64, minimprovehigh),
        true,
        DEFAULT_MINIMPROVEHIGH,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/alns/nsolslim",
        "limit on the number of improving solutions in a sub-SCIP call",
        t!(i32, nsolslim),
        false,
        DEFAULT_NSOLSLIM,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/alns/banditalgo",
        "the bandit algorithm: (u)pper confidence bounds, (e)xp.3, epsilon (g)reedy, exp.3-(i)x",
        t!(u8, banditalgo),
        true,
        DEFAULT_BANDITALGO,
        Some("uegi"),
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/gamma",
        "weight between uniform (gamma ~ 1) and weight driven (gamma ~ 0) probability distribution for exp3",
        t!(f64, exp3_gamma),
        true,
        DEFAULT_GAMMA,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/beta",
        "reward offset between 0 and 1 at every observation for Exp.3",
        t!(f64, exp3_beta),
        true,
        DEFAULT_BETA,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/alpha",
        "parameter to increase the confidence width in UCB",
        t!(f64, ucb_alpha),
        true,
        DEFAULT_ALPHA,
        0.0,
        100.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/usedistances",
        "distances from fixed variables be used for variable prioritization",
        t!(bool, usedistances),
        true,
        DEFAULT_USEDISTANCES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/useredcost",
        "should reduced cost scores be used for variable prioritization?",
        t!(bool, useredcost),
        true,
        DEFAULT_USEREDCOST,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/domorefixings",
        "should the ALNS heuristic do more fixings by itself based on variable prioritization until the target fixing rate is reached?",
        t!(bool, domorefixings),
        true,
        DEFAULT_DOMOREFIXINGS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/adjustfixingrate",
        "should the heuristic adjust the target fixing rate based on the success?",
        t!(bool, adjustfixingrate),
        true,
        DEFAULT_ADJUSTFIXINGRATE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/usesubscipheurs",
        "should the heuristic activate other sub-SCIP heuristics during its search?",
        t!(bool, usesubscipheurs),
        true,
        DEFAULT_USESUBSCIPHEURS,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/rewardcontrol",
        "reward control to increase the weight of the simple solution indicator and decrease the weight of the closed gap reward",
        t!(f64, rewardcontrol),
        true,
        DEFAULT_REWARDCONTROL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/targetnodefactor",
        "factor by which target node number is eventually increased",
        t!(f64, targetnodefactor),
        true,
        DEFAULT_TARGETNODEFACTOR,
        1.0,
        1e+5,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/alns/seed",
        "initial random seed for bandit algorithms and random decisions by neighborhoods",
        t!(i32, seed),
        false,
        DEFAULT_SEED,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/alns/maxcallssamesol",
        "number of allowed executions of the heuristic on the same incumbent solution (-1: no limit, 0: number of active neighborhoods)",
        t!(i32, maxcallssamesol),
        true,
        DEFAULT_MAXCALLSSAMESOL,
        -1,
        100,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/adjustminimprove",
        "should the factor by which the minimum improvement is bound be dynamically updated?",
        t!(bool, adjustminimprove),
        true,
        DEFAULT_ADJUSTMINIMPROVE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/adjusttargetnodes",
        "should the target nodes be dynamically adjusted?",
        t!(bool, adjusttargetnodes),
        true,
        DEFAULT_ADJUSTTARGETNODES,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/eps",
        "increase exploration in epsilon-greedy bandit algorithm",
        t!(f64, epsgreedy_eps),
        true,
        DEFAULT_EPS,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/rewardbaseline",
        "the reward baseline to separate successful and failed calls",
        t!(f64, rewardbaseline),
        true,
        DEFAULT_REWARDBASELINE,
        0.0,
        0.99,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/resetweights",
        "should the bandit algorithms be reset when a new problem is read?",
        t!(bool, resetweights),
        true,
        DEFAULT_RESETWEIGHTS,
        None,
    )?;
    paramset_add_string(
        scip,
        "heuristics/alns/rewardfilename",
        "file name to store all rewards and the selection of the bandit",
        t!(String, rewardfilename),
        true,
        DEFAULT_REWARDFILENAME,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/subsciprandseeds",
        "should random seeds of sub-SCIPs be altered to increase diversification?",
        t!(bool, subsciprandseeds),
        true,
        DEFAULT_SUBSCIPRANDSEEDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/scalebyeffort",
        "should the reward be scaled by the effort?",
        t!(bool, scalebyeffort),
        true,
        DEFAULT_SCALEBYEFFORT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/copycuts",
        "should cutting planes be copied to the sub-SCIP?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/fixtol",
        "tolerance by which the fixing rate may be missed without generic fixing",
        t!(f64, fixtol),
        true,
        DEFAULT_FIXTOL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/alns/unfixtol",
        "tolerance by which the fixing rate may be exceeded without generic unfixing",
        t!(f64, unfixtol),
        true,
        DEFAULT_UNFIXTOL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/uselocalredcost",
        "should local reduced costs be used for generic (un)fixing?",
        t!(bool, uselocalredcost),
        true,
        DEFAULT_USELOCALREDCOST,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/usepscost",
        "should pseudo cost scores be used for variable priorization?",
        t!(bool, usepscost),
        true,
        DEFAULT_USEPSCOST,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/alns/initduringroot",
        "should the heuristic be executed multiple times during the root node?",
        t!(bool, initduringroot),
        true,
        DEFAULT_INITDURINGROOT,
        None,
    )?;

    Ok(())
}
