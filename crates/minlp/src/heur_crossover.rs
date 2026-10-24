//! Port of `heur_crossover.c`: LNS heuristic that fixes the integer
//! variables on which a few of the best solutions agree and solves the rest
//! in a sub-SCIP.
//!
//! C compares the solutions it remembers by pointer; here by id, which
//! differs only where C reuses the memory of a freed solution.

use std::any::Any;

use crate::{
    api,
    copy::CopyMaps,
    event::{EVENTTYPE_LPSOLVED, Event},
    heur::{HEURTIMING_AFTERNODE, HeurTiming},
    misc::RandNumGen,
    paramset::{ParamSetting, paramset_set_bool, paramset_set_char, paramset_set_int, paramset_set_longint},
    retcode::ScipResult,
    scip::Scip,
    types::{EventhdlrId, HeurId, ScipResultCode, SolId, VarId},
    var::NONE,
};

const HEUR_NAME: &str = "crossover";
const HEUR_DESC: &str = "LNS heuristic that fixes all variables that are identic in a couple of solutions";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1104000;
const HEUR_FREQ: i32 = 15;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i64 = 5000;
/// Factor by which Crossover should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.01;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i64 = 50;
/// Minimum percentage of integer variables that have to be fixed.
const DEFAULT_MINFIXINGRATE: f64 = 0.666;
/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i64 = 500;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.1;
/// Factor by which the limit on the number of LP depends on the node limit.
const DEFAULT_LPLIMFAC: f64 = 2.0;
/// Number of solutions that will be taken into account.
const DEFAULT_NUSEDSOLS: i32 = 3;
/// Number of nodes without incumbent change heuristic should wait.
const DEFAULT_NWAITINGNODES: i64 = 200;
/// Should the choice which sols to take be randomized?
const DEFAULT_RANDOMIZATION: bool = true;
/// Should the nwaitingnodes parameter be ignored at the root node?
const DEFAULT_DONTWAITATROOT: bool = false;
/// Should subproblem be created out of the rows in the LP rows?
const DEFAULT_USELPROWS: bool = false;
/// If DEFAULT_USELPROWS is FALSE, then should all active cuts from the
/// cutpool of the original scip be copied to constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Should the subproblem be permuted to increase diversification?
const DEFAULT_PERMUTE: bool = false;
/// Limit on number of improving incumbent solutions in sub-CIP.
const DEFAULT_BESTSOLLIMIT: i32 = -1;
/// Should uct node selection be used at the beginning of the search?
const DEFAULT_USEUCT: bool = false;
/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 7;

const EVENTHDLR_NAME: &str = "Crossover";
const EVENTHDLR_DESC: &str = "LP event handler for crossover heuristic";

/// `SOLTUPLE`: an n-tuple of solutions and its hash key.
struct SolTuple {
    indices: Vec<i32>,
    key: u32,
}

/// `SCIP_HeurData`.
struct HeurData {
    prevlastsol: SolId,
    prevbestsol: SolId,
    prevnsols: i32,
    maxnodes: i64,
    minnodes: i64,
    nodesofs: i64,
    usednodes: i64,
    nodesquot: f64,
    nusedsols: i32,
    nwaitingnodes: i64,
    nfailures: u32,
    nextnodenumber: i64,
    minfixingrate: f64,
    minimprove: f64,
    nodelimit: f64,
    lplimfac: f64,
    randomization: bool,
    dontwaitatroot: bool,
    randnumgen: Option<RandNumGen>,
    /// The hash table of tried solution tuples; C finds a tuple by its key
    /// and then its indices, which is a search for equal indices.
    hashtable: Vec<SolTuple>,
    uselprows: bool,
    copycuts: bool,
    permute: bool,
    bestsollimit: i32,
    useuct: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut().unwrap()
}

/*
 * Local methods
 */

/// `calculateHashKey`: the hash key (x1+1) * (x2+1) * ... * (xn+1) + x1 +
/// x2 + ... + xn of a tuple of solution indices.
fn calculate_hash_key(indices: &[i32]) -> u32 {
    let mut hashkey: u32 = 1;
    for &i in indices {
        hashkey = hashkey.wrapping_mul((i as u32).wrapping_add(1));
    }
    for &i in indices {
        hashkey = hashkey.wrapping_add(i as u32);
    }
    hashkey
}

/// `sortArray`: insertion sort for a small int array.
fn sort_array(a: &mut [i32]) {
    for i in 1..a.len() {
        let tmp = a[i];
        let mut j = i as isize - 1;
        while j >= 0 && a[j as usize] > tmp {
            a[(j + 1) as usize] = a[j as usize];
            j -= 1;
        }
        a[(j + 1) as usize] = tmp;
    }
}

/// `createSolTuple`: a new tuple of the given solution indices. As in C, the
/// tuple keeps the indices as given and `indices` is sorted afterwards.
fn create_sol_tuple(indices: &mut [i32]) -> SolTuple {
    let copy = indices.to_vec();

    // Data input.
    sort_array(indices);
    let key = calculate_hash_key(&copy);
    SolTuple { indices: copy, key }
}

/// `SCIPhashtableExists` on the tried tuples.
fn tuple_exists(hashtable: &[SolTuple], elem: &SolTuple) -> bool {
    hashtable.iter().any(|t| t.key == elem.key && t.indices == elem.indices)
}

/// `solHasNewSource`: whether the new solution was not found at the same
/// node by the same heuristic as an already selected one.
fn sol_has_new_source(scip: &Scip, sols: &[SolId], selection: &[i32], newsol: i32) -> bool {
    let n = sols[newsol as usize];
    for &s in selection {
        let s = sols[s as usize];
        if crate::sol::sol_get_heur(scip, s) == crate::sol::sol_get_heur(scip, n) && scip.sols[s].nodenum == scip.sols[n].nodenum {
            return false;
        }
    }
    true
}

/// `selectSolsRandomized`: randomly selects the solutions crossover will use
/// from the pool of all solutions found so far; returns success.
fn select_sols_randomized(scip: &mut Scip, heur: HeurId, selection: &mut [i32]) -> bool {
    // Initialization.
    let nusedsols = hd(scip, heur).nusedsols;
    let mut lastsol = api::get_n_sols(scip);
    let sols = api::get_sols(scip).to_vec();
    let mut i = 0;
    let mut success = false;

    // Perform at maximum 10 restarts and stop as soon as a new set of
    // solutions is found.
    while !success && i < 10 {
        let mut validtuple = true;
        let mut j = 0;
        while j < nusedsols && validtuple {
            let mut k = hdm(scip, heur).randnumgen.as_mut().unwrap().get_int(nusedsols - j - 1, lastsol - 1);

            // Ensure that the solution does not have a similar source as the
            // others.
            while k >= nusedsols - j - 1 && !sol_has_new_source(scip, &sols, &selection[..j as usize], k) {
                k -= 1;
            }

            validtuple = k >= nusedsols - j - 1;
            selection[j as usize] = k;
            lastsol = k;
            j += 1;
        }

        if validtuple {
            // Creates an object ready to be inserted into the hashtable.
            let elem = create_sol_tuple(&mut selection[..nusedsols as usize]);

            // Check whether the randomized set is already in the hashtable, if
            // not, insert it.
            let d = hdm(scip, heur);
            if !tuple_exists(&d.hashtable, &elem) {
                d.hashtable.push(elem);
                success = true;
            }
        }
        i += 1;
    }
    success
}

/// `fixVariables`: the fixings for the CROSSOVER subproblem; returns
/// `(fixedvars, fixedvals, success)`.
fn fix_variables(scip: &Scip, heur: HeurId, selection: &[i32]) -> (Vec<VarId>, Vec<f64>, bool) {
    let sols = api::get_sols(scip);
    let nusedsols = hd(scip, heur).nusedsols as usize;

    // Get required data of the original problem.
    let vars = api::get_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    let mut fixedvars = Vec::new();
    let mut fixedvals = Vec::new();

    // Go through discrete variables and collect potential fixings.
    for &var in &vars[..nbinvars + nintvars] {
        let mut fixable = true;
        let solval = api::get_sol_val(scip, sols[selection[0] as usize], var);

        // Check, whether variable's value is identical for each selected
        // solution.
        for &s in &selection[1..nusedsols] {
            let varsolval = api::get_sol_val(scip, sols[s as usize], var);
            if (solval - varsolval).abs() > 0.5 {
                fixable = false;
                break;
            }
        }

        // Original solval can be outside transformed global bounds.
        let v = &scip.vars[var];
        fixable = fixable && v.glbdom.lb <= solval && solval <= v.glbdom.ub;

        // If solutions' values are equal, variable should be fixed in the
        // subproblem.
        if fixable {
            fixedvars.push(var);
            fixedvals.push(solval);
        }
    }

    let fixingrate = fixedvars.len() as f64 / crate::def::max((nbinvars + nintvars) as f64, 1.0);

    // If all variables would be fixed or amount of fixed variables is
    // insufficient, skip subproblem creation and abort immediately.
    let success = fixedvars.len() < nbinvars + nintvars && fixingrate >= hd(scip, heur).minfixingrate;
    (fixedvars, fixedvals, success)
}

/// `determineVariableFixings`: selects the solutions and fixes the variables
/// of the subproblem; returns `(fixedvars, fixedvals, success)`.
fn determine_variable_fixings(scip: &mut Scip, heur: HeurId, selection: &mut [i32]) -> (Vec<VarId>, Vec<f64>, bool) {
    // Get solutions' data.
    let nsols = api::get_n_sols(scip);
    let sols = api::get_sols(scip).to_vec();
    let nusedsols = hd(scip, heur).nusedsols;
    let mut success;

    // Use nusedsols best solutions if randomization is deactivated or there
    // are only nusedsols solutions at hand or the best solution is not part
    // of the selection.
    let d = hd(scip, heur);
    if !d.randomization || nsols == nusedsols || d.prevlastsol != sols[(nusedsols - 1) as usize] {
        for i in 0..nusedsols {
            selection[i as usize] = i;
        }
        let elem = create_sol_tuple(&mut selection[..nusedsols as usize]);

        let solheur = crate::sol::sol_get_heur(scip, sols[0]);
        let solnodenum = scip.sols[sols[0]].nodenum;
        let mut allsame = true;

        // Check, whether all solutions have been found by the same heuristic
        // at the same node; in this case we do not run crossover, since the
        // solutions are probably not diverse enough.
        for &s in &sols[1..nusedsols as usize] {
            if crate::sol::sol_get_heur(scip, s) != solheur || scip.sols[s].nodenum != solnodenum {
                allsame = false;
            }
        }

        let d = hdm(scip, heur);
        let exists = tuple_exists(&d.hashtable, &elem);
        success = !allsame && !exists;

        // Check, whether solution tuple has already been tried.
        if !exists {
            d.hashtable.push(elem);
        }

        // If solution tuple has already been tried, randomization is allowed
        // and enough solutions are at hand, try to randomize another tuple.
        // E.g., this can happen if the last crossover solution was among the
        // best ones.
        if !success && d.randomization && nsols > nusedsols {
            success = select_sols_randomized(scip, heur, selection);
        }
    }
    // Otherwise randomize the set of solutions.
    else {
        success = select_sols_randomized(scip, heur, selection);
    }

    // No acceptable solution tuple could be created.
    if !success {
        return (Vec::new(), Vec::new(), false);
    }

    // Set up the variables of the subproblem.
    fix_variables(scip, heur, selection)
}

/// `updateFailureStatistic`: updates heurdata after a run of crossover.
fn update_failure_statistic(scip: &mut Scip, heur: HeurId) {
    let nnodes = api::get_n_nodes(scip);
    let d = hdm(scip, heur);

    // Increase number of failures, calculate next node at which crossover
    // should be called and update actual solutions.
    d.nfailures += 1;
    d.nextnodenumber = if d.nfailures <= 25 { nnodes + 100 * (2i64 << d.nfailures) } else { i64::MAX };
}

/// The data the sub-SCIP's LP event handler reads: C passes it the
/// heuristic's data, here it keeps its own copy of the two fields.
struct EventhdlrData {
    lplimfac: f64,
    nodelimit: f64,
}

/// `eventExecCrossover`: interrupts the sub-SCIP once it solved too many
/// LPs.
fn event_exec_crossover(scip: &mut Scip, eventhdlr: EventhdlrId, _event: &Event, _eventdata: usize) -> ScipResult {
    let d: &EventhdlrData = scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref().unwrap();

    // Interrupt solution process of sub-SCIP.
    if api::get_n_lps(scip) as f64 > d.lplimfac * d.nodelimit {
        crate::scip_solve::interrupt_solve(scip);
    }
    Ok(())
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyCrossover`: copy method for primal heuristic plugins (called
/// when SCIP copies plugins).
fn heur_copy_crossover(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_crossover(scip)
}

/// `setupAndSolveSubscipCrossover`: sets up and solves the subproblem.
#[allow(clippy::too_many_arguments)]
fn setup_and_solve_subscip_crossover(
    scip: &mut Scip,
    subscip: &mut Scip,
    heur: HeurId,
    vars: &[VarId],
    fixedvars: &[VarId],
    fixedvals: &[f64],
    nstallnodes: i64,
    result: &mut ScipResultCode,
    selection: &mut [i32],
    nusedsols: i32,
) -> ScipResult {
    let nvars = vars.len();

    // Create the variable mapping hash map.
    let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

    // Create a copy of the transformed problem to be used by the heuristic.
    let (uselprows, copycuts) = (hd(scip, heur).uselprows, hd(scip, heur).copycuts);
    crate::copy::copy_large_neighborhood_search(scip, subscip, &mut maps, "crossover", fixedvars, fixedvals, uselprows, copycuts)?;

    // Create event handler for LP events.
    let lplimfac = hd(scip, heur).lplimfac;
    let eventhdlr = api::include_eventhdlr_basic(
        subscip,
        EVENTHDLR_NAME,
        EVENTHDLR_DESC,
        event_exec_crossover,
        Some(Box::new(EventhdlrData { lplimfac, nodelimit: 0.0 })),
    )?;

    // Store copied variables in the order in which they appear in the main
    // SCIP.
    let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable statistic timing inside sub SCIP and output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    // Check whether there is enough time and memory left (C sets the best
    // solution limit before SCIPcopyLimits resets it).
    paramset_set_int(subscip, "limits/bestsol", hd(scip, heur).bestsollimit)?;

    // Set limits for the subproblem.
    crate::copy::copy_limits(scip, subscip)?;
    hdm(scip, heur).nodelimit = nstallnodes as f64;
    subscip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_mut()
        .unwrap()
        .downcast_mut::<EventhdlrData>()
        .unwrap()
        .nodelimit = nstallnodes as f64;
    paramset_set_longint(subscip, "limits/nodes", nstallnodes)?;

    // Forbid recursive call of heuristics and separators solving subMIPs.
    crate::paramset::set_subscips_off(subscip, true)?;

    // Disable cutting plane separation.
    crate::paramset::set_separating(subscip, ParamSetting::Off, true)?;

    // Disable expensive presolving.
    crate::paramset::set_presolving(subscip, ParamSetting::Fast, true)?;

    // Use best estimate node selection.
    if crate::set::set_find_nodesel(subscip, "estimate") != NONE && !crate::paramset::paramset_is_fixed(subscip, "nodeselection/estimate/stdpriority")? {
        paramset_set_int(subscip, "nodeselection/estimate/stdpriority", i32::MAX / 4)?;
    }

    // Activate uct node selection at the top of the tree.
    if hd(scip, heur).useuct && crate::set::set_find_nodesel(subscip, "uct") != NONE && !crate::paramset::paramset_is_fixed(subscip, "nodeselection/uct/stdpriority")? {
        paramset_set_int(subscip, "nodeselection/uct/stdpriority", i32::MAX / 2)?;
    }

    // Use inference branching.
    if crate::set::set_find_branchrule(subscip, "inference") != NONE && !crate::paramset::paramset_is_fixed(subscip, "branching/inference/priority")? {
        paramset_set_int(subscip, "branching/inference/priority", i32::MAX / 4)?;
    }

    // Enable conflict analysis, disable analysis of boundexceeding LPs, and
    // restrict conflict pool.
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
    let minimprove = hd(scip, heur).minimprove;
    let upperbound = api::get_upperbound(scip) - scip.set.sumepsilon();
    let cutoff = if !scip.set.is_infinity(-1.0 * api::get_lowerbound(scip)) {
        (1.0 - minimprove) * api::get_upperbound(scip) + minimprove * api::get_lowerbound(scip)
    } else if api::get_upperbound(scip) >= 0.0 {
        (1.0 - minimprove) * api::get_upperbound(scip)
    } else {
        (1.0 + minimprove) * api::get_upperbound(scip)
    };
    let cutoff = crate::def::min(upperbound, cutoff);
    api::set_objlimit(subscip, cutoff)?;

    // Permute the subproblem to increase diversification.
    if hd(scip, heur).permute {
        unimplemented!("SCIPpermuteProb (heuristics/crossover/permute, off by default)");
    }

    // Catch LP events of sub-SCIP.
    crate::scip_solve::transform_prob(subscip)?;
    api::catch_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, None)?;

    // Solve the subproblem.
    crate::scip_solve::solve(subscip)?;

    // Drop LP events of sub-SCIP.
    api::drop_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, -1)?;

    hdm(scip, heur).usednodes += api::get_n_nodes(subscip);

    // Merge histories of the subscip-variables to the SCIP variables.
    crate::copy::merge_variable_statistics(subscip, scip, &subvars, vars)?;

    // Check, whether a solution was found.
    if api::get_n_sols(subscip) > 0 {
        // Check, whether a solution was found; due to numerics, it might
        // happen that not all solutions are feasible -> try all solutions
        // until one was accepted.
        let (success, solindex) = crate::copy::translate_sub_sols(scip, subscip, heur, &subvars)?;

        if success {
            *result = ScipResultCode::FoundSol;

            // Insert all crossings of the new solution and (nusedsols-1) of
            // its parents into the hashtable in order to avoid that the same
            // solutions are combined again.
            for i in 0..nusedsols as usize {
                let tmp = selection[i];
                selection[i] = solindex;
                let elem = create_sol_tuple(&mut selection[..nusedsols as usize]);
                hdm(scip, heur).hashtable.push(elem);
                selection[i] = tmp;
            }

            // If solution was among the best ones, crossover should not be
            // called until another good solution was found.
            if !hd(scip, heur).randomization {
                let bestsol = api::get_best_sol(scip);
                let lastsol = api::get_sols(scip)[(hd(scip, heur).nusedsols - 1) as usize];
                let d = hdm(scip, heur);
                d.prevbestsol = bestsol;
                d.prevlastsol = lastsol;
            }
        }

        // If solution is not better than incumbent or could not be added to
        // problem => run is counted as a failure.
        if !success || solindex != scip.sols[api::get_best_sol(scip)].index {
            update_failure_statistic(scip, heur);
        }
    } else {
        // If no new solution was found, run was a failure.
        update_failure_statistic(scip, heur);
    }

    Ok(())
}

/// `heurInitCrossover`: initialization method of primal heuristic (called
/// after problem was transformed).
fn heur_init_crossover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);

    // Initialize data.
    let d = hdm(scip, heur);
    d.usednodes = 0;
    d.prevlastsol = NONE;
    d.prevbestsol = NONE;
    d.nfailures = 0;
    d.prevnsols = 0;
    d.nextnodenumber = 0;
    d.randnumgen = Some(RandNumGen::new(seed));

    // Initialize hash table.
    d.hashtable = Vec::new();
    Ok(())
}

/// `heurExitCrossover`: deinitialization method of primal heuristic (called
/// before transformed problem is freed).
fn heur_exit_crossover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free all soltuples, the random number generator and the hash table.
    let d = hdm(scip, heur);
    d.hashtable = Vec::new();
    d.randnumgen = None;
    Ok(())
}

/// `heurExecCrossover`: execution method of primal heuristic.
fn heur_exec_crossover(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let nusedsols = hd(scip, heur).nusedsols;

    *result = ScipResultCode::Delayed;

    // Only call heuristic, if enough solutions are at hand.
    if api::get_n_sols(scip) < nusedsols {
        return Ok(());
    }

    let sols = api::get_sols(scip).to_vec();

    // If one good solution was found, heuristic should not be delayed any
    // longer.
    if sols[(nusedsols - 1) as usize] != hd(scip, heur).prevlastsol {
        let nnodes = api::get_n_nodes(scip);
        let d = hdm(scip, heur);
        d.nextnodenumber = nnodes;
        if sols[0] != d.prevbestsol {
            d.nfailures = 0;
        }
    }
    // In nonrandomized mode: only recall heuristic, if at least one new good
    // solution was found in the meantime.
    else if !hd(scip, heur).randomization {
        return Ok(());
    }

    // If heuristic should be delayed, wait until certain number of nodes is
    // reached.
    if api::get_n_nodes(scip) < hd(scip, heur).nextnodenumber {
        return Ok(());
    }

    // Only call heuristic, if enough nodes were processed since last
    // incumbent.
    let bestsol = api::get_best_sol(scip);
    if api::get_n_nodes(scip) - scip.sols[bestsol].nodenum < hd(scip, heur).nwaitingnodes && (api::get_depth(scip) > 0 || !hd(scip, heur).dontwaitatroot) {
        return Ok(());
    }

    *result = ScipResultCode::DidNotRun;

    let d = hd(scip, heur);
    let h = &scip.heurs[heur];

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut nstallnodes = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Reward Crossover if it succeeded often.
    nstallnodes = (nstallnodes as f64 * (1.0 + 2.0 * (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0))) as i64;

    // Count the setup costs for the sub-MIP as 100 nodes.
    nstallnodes -= 100 * h.ncalls;
    nstallnodes += d.nodesofs;

    // Determine the node limit for the current process.
    nstallnodes -= d.usednodes;
    nstallnodes = if nstallnodes <= d.maxnodes { nstallnodes } else { d.maxnodes };

    // Check whether we have enough nodes left to call subproblem solving.
    if nstallnodes < d.minnodes {
        return Ok(());
    }

    // Consider time and memory limits of the main SCIP.
    if !crate::copy::check_copy_limits(scip)? {
        return Ok(());
    }

    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Get variable information.
    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);

    // Check whether discrete variables are available.
    if nbinvars == 0 && nintvars == 0 {
        return Ok(());
    }

    let mut selection = vec![0i32; nusedsols as usize];

    // Determine fixings of variables with same value in a certain set of
    // solutions.
    let (fixedvars, fixedvals, success) = determine_variable_fixings(scip, heur, &mut selection);

    let bestsol = api::get_best_sol(scip);
    {
        let d = hdm(scip, heur);
        d.prevbestsol = bestsol;
        d.prevlastsol = sols[(d.nusedsols - 1) as usize];
    }

    // If creation of sub-SCIP was aborted (e.g. due to number of fixings),
    // free sub-SCIP and abort.
    if !success {
        // This run will be counted as a failure since no new solution tuple
        // could be generated or the neighborhood of the solution was not
        // fruitful in the sense that it was too big.
        update_failure_statistic(scip, heur);
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Initializing the subproblem.
    let mut subscip = Scip::create()?;

    // Setup and solve the subproblem and catch the return code.
    setup_and_solve_subscip_crossover(scip, &mut subscip, heur, &vars, &fixedvars, &fixedvals, nstallnodes, result, &mut selection, nusedsols)
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurCrossover`: creates the crossover primal heuristic and
/// includes it in SCIP.
pub fn include_heur_crossover(scip: &mut Scip) -> ScipResult {
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_bool, paramset_add_int, paramset_add_longint, paramset_add_real},
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
        heur_exec_crossover,
        Some(Box::new(HeurData {
            prevlastsol: NONE,
            prevbestsol: NONE,
            prevnsols: 0,
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            nodesofs: DEFAULT_NODESOFS,
            usednodes: 0,
            nodesquot: DEFAULT_NODESQUOT,
            nusedsols: DEFAULT_NUSEDSOLS,
            nwaitingnodes: DEFAULT_NWAITINGNODES,
            nfailures: 0,
            nextnodenumber: 0,
            minfixingrate: DEFAULT_MINFIXINGRATE,
            minimprove: DEFAULT_MINIMPROVE,
            nodelimit: 0.0,
            lplimfac: DEFAULT_LPLIMFAC,
            randomization: DEFAULT_RANDOMIZATION,
            dontwaitatroot: DEFAULT_DONTWAITATROOT,
            randnumgen: None,
            hashtable: Vec::new(),
            uselprows: DEFAULT_USELPROWS,
            copycuts: DEFAULT_COPYCUTS,
            permute: DEFAULT_PERMUTE,
            bestsollimit: DEFAULT_BESTSOLLIMIT,
            useuct: DEFAULT_USEUCT,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_crossover);
    h.heurinit = Some(heur_init_crossover);
    h.heurexit = Some(heur_exit_crossover);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add crossover primal heuristic parameters.
    paramset_add_longint(
        scip,
        "heuristics/crossover/nodesofs",
        "number of nodes added to the contingent of the total nodes",
        t!(i64, nodesofs),
        false,
        DEFAULT_NODESOFS,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/crossover/maxnodes",
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
        "heuristics/crossover/minnodes",
        "minimum number of nodes required to start the subproblem",
        t!(i64, minnodes),
        true,
        DEFAULT_MINNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/crossover/nusedsols",
        "number of solutions to be taken into account",
        t!(i32, nusedsols),
        false,
        DEFAULT_NUSEDSOLS,
        2,
        i32::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/crossover/nwaitingnodes",
        "number of nodes without incumbent change that heuristic should wait",
        t!(i64, nwaitingnodes),
        true,
        DEFAULT_NWAITINGNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/crossover/nodesquot",
        "contingent of sub problem nodes in relation to the number of nodes of the original problem",
        t!(f64, nodesquot),
        false,
        DEFAULT_NODESQUOT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/crossover/minfixingrate",
        "minimum percentage of integer variables that have to be fixed",
        t!(f64, minfixingrate),
        false,
        DEFAULT_MINFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/crossover/minimprove",
        "factor by which Crossover should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/crossover/lplimfac",
        "factor by which the limit on the number of LP depends on the node limit",
        t!(f64, lplimfac),
        true,
        DEFAULT_LPLIMFAC,
        1.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/crossover/randomization",
        "should the choice which sols to take be randomized?",
        t!(bool, randomization),
        true,
        DEFAULT_RANDOMIZATION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/crossover/dontwaitatroot",
        "should the nwaitingnodes parameter be ignored at the root node?",
        t!(bool, dontwaitatroot),
        true,
        DEFAULT_DONTWAITATROOT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/crossover/uselprows",
        "should subproblem be created out of the rows in the LP rows?",
        t!(bool, uselprows),
        true,
        DEFAULT_USELPROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/crossover/copycuts",
        "if uselprows == FALSE, should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/crossover/permute",
        "should the subproblem be permuted to increase diversification?",
        t!(bool, permute),
        true,
        DEFAULT_PERMUTE,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/crossover/bestsollimit",
        "limit on number of improving incumbent solutions in sub-CIP",
        t!(i32, bestsollimit),
        false,
        DEFAULT_BESTSOLLIMIT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/crossover/useuct",
        "should uct node selection be used at the beginning of the search?",
        t!(bool, useuct),
        true,
        DEFAULT_USEUCT,
        None,
    )?;

    Ok(())
}
