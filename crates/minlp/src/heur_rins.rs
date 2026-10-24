//! Port of `heur_rins.c`: LNS heuristic by Danna, Rothberg and Le Pape that
//! fixes the integer variables whose values agree in the LP solution and
//! the incumbent, and solves the rest in a sub-SCIP.

use std::any::Any;

use crate::{
    api,
    copy::CopyMaps,
    event::{EVENTTYPE_LPSOLVED, Event},
    heur::{HEURTIMING_AFTERLPNODE, HeurTiming},
    paramset::{ParamSetting, paramset_set_bool, paramset_set_char, paramset_set_int, paramset_set_longint},
    retcode::ScipResult,
    scip::Scip,
    types::{EventhdlrId, HeurId, LpSolStat, ScipResultCode, VarId},
    var::NONE,
};

const HEUR_NAME: &str = "rins";
const HEUR_DESC: &str = "relaxation induced neighborhood search by Danna, Rothberg, and Le Pape";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1101000;
const HEUR_FREQ: i32 = 15;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i32 = 500;
/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i32 = 5000;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i32 = 50;
/// Factor by which RINS should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.01;
/// Minimum percentage of integer variables that have to be fixed.
const DEFAULT_MINFIXINGRATE: f64 = 0.3;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.3;
/// Factor by which the limit on the number of LP depends on the node limit.
const DEFAULT_LPLIMFAC: f64 = 2.0;
/// Number of nodes without incumbent change that heuristic should wait.
const DEFAULT_NWAITINGNODES: i32 = 200;
/// Should subproblem be created out of the rows in the LP rows?
const DEFAULT_USELPROWS: bool = false;
/// If DEFAULT_USELPROWS is FALSE, then should all active cuts from the
/// cutpool of the original scip be copied to constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Should uct node selection be used at the beginning of the search?
const DEFAULT_USEUCT: bool = false;

const EVENTHDLR_NAME: &str = "Rins";
const EVENTHDLR_DESC: &str = "LP event handler for rins heuristic";

/// `SCIP_HeurData`.
struct HeurData {
    nodesofs: i32,
    maxnodes: i32,
    minnodes: i32,
    minfixingrate: f64,
    nwaitingnodes: i32,
    minimprove: f64,
    nodelimit: f64,
    lplimfac: f64,
    usednodes: i64,
    nodesquot: f64,
    uselprows: bool,
    copycuts: bool,
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

/// `determineFixings`: the variables with the same value in the incumbent
/// and the LP solution, and those values; returns `(fixedvars, fixedvals,
/// success)`.
fn determine_fixings(scip: &Scip, minfixingrate: f64) -> (Vec<VarId>, Vec<f64>, bool) {
    // Get required data of the original problem.
    let vars = api::get_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;
    let bestsol = api::get_best_sol(scip);

    let mut fixedvars = Vec::new();
    let mut fixedvals = Vec::new();

    // Determine variables to fix in the subproblem.
    for &var in &vars[..nbinvars + nintvars] {
        // Get the current LP solution and the incumbent solution for each
        // variable.
        let lpsolval = crate::var::var_get_lp_sol(scip, var);
        let solval = api::get_sol_val(scip, bestsol, var);

        // Iff both solutions are equal, variable is stored to be fixed.
        if scip.set.is_feas_eq(lpsolval, solval) {
            // Store the fixing and increase the number of fixed variables.
            fixedvars.push(var);
            fixedvals.push(solval);
        }
    }

    // Abort, if all variables should be fixed.
    let fixingcounter = fixedvars.len();
    if fixingcounter == nbinvars + nintvars {
        return (fixedvars, fixedvals, false);
    }
    let fixingrate = fixingcounter as f64 / crate::def::max((nbinvars + nintvars) as f64, 1.0);

    // Abort, if the amount of fixed variables is insufficient.
    if fixingrate < minfixingrate {
        return (fixedvars, fixedvals, false);
    }

    (fixedvars, fixedvals, true)
}

/// The data the sub-SCIP's LP event handler reads: C passes it the
/// heuristic's data, here it keeps its own copy of the two fields.
struct EventhdlrData {
    lplimfac: f64,
    nodelimit: f64,
}

/// `eventExecRins`: interrupts the sub-SCIP once it solved too many LPs.
fn event_exec_rins(scip: &mut Scip, eventhdlr: EventhdlrId, _event: &Event, _eventdata: usize) -> ScipResult {
    let d: &EventhdlrData = scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref().unwrap();

    // Interrupt solution process of sub-SCIP.
    if api::get_n_lps(scip) as f64 > d.lplimfac * d.nodelimit {
        crate::scip_solve::interrupt_solve(scip);
    }
    Ok(())
}

/// `wrapperRins`: the part of the heuristic that runs a sub-SCIP.
#[allow(clippy::too_many_arguments)]
fn wrapper_rins(scip: &mut Scip, subscip: &mut Scip, heur: HeurId, vars: &[VarId], fixedvars: &[VarId], fixedvals: &[f64], result: &mut ScipResultCode, nnodes: i64) -> ScipResult {
    let nvars = vars.len();

    // Create the variable mapping hash map.
    let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

    // Create a problem copy as sub SCIP.
    let (uselprows, copycuts) = (hd(scip, heur).uselprows, hd(scip, heur).copycuts);
    crate::copy::copy_large_neighborhood_search(scip, subscip, &mut maps, "rins", fixedvars, fixedvals, uselprows, copycuts)?;

    // Create event handler for LP events.
    let lplimfac = hd(scip, heur).lplimfac;
    let eventhdlr = api::include_eventhdlr_basic(
        subscip,
        EVENTHDLR_NAME,
        EVENTHDLR_DESC,
        event_exec_rins,
        Some(Box::new(EventhdlrData { lplimfac, nodelimit: 0.0 })),
    )?;

    // Copy subproblem variables from map to obtain the same order.
    let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable statistic timing inside sub SCIP and output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    // Set limits for the subproblem.
    crate::copy::copy_limits(scip, subscip)?;
    hdm(scip, heur).nodelimit = nnodes as f64;
    subscip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_mut()
        .unwrap()
        .downcast_mut::<EventhdlrData>()
        .unwrap()
        .nodelimit = nnodes as f64;
    paramset_set_longint(subscip, "limits/nodes", nnodes)?;
    paramset_set_longint(subscip, "limits/stallnodes", if 10 >= nnodes / 10 { 10 } else { nnodes / 10 })?;
    paramset_set_int(subscip, "limits/bestsol", 3)?;

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

    // Catch LP events of sub-SCIP.
    crate::scip_solve::transform_prob(subscip)?;
    api::catch_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, None)?;

    // Solve the subproblem.
    crate::scip_solve::solve(subscip)?;

    // Drop LP events of sub-SCIP.
    api::drop_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, -1)?;

    // We try to merge variable statistics with those of our main SCIP.
    crate::copy::merge_variable_statistics(subscip, scip, &subvars, vars)?;

    hdm(scip, heur).usednodes += api::get_n_nodes(subscip);

    let (success, _) = crate::copy::translate_sub_sols(scip, subscip, heur, &subvars)?;
    if success {
        *result = ScipResultCode::FoundSol;
    }
    Ok(())
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyRins`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_rins(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_rins(scip)
}

/// `heurInitRins`: initialization method of primal heuristic (called after
/// problem was transformed).
fn heur_init_rins(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Initialize data.
    hdm(scip, heur).usednodes = 0;
    Ok(())
}

/// `heurExecRins`: execution method of primal heuristic.
fn heur_exec_rins(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Delayed;

    // Do not call heuristic of node was already detected to be infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Only call heuristic, if an optimal LP solution and a feasible solution
    // are at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal || api::get_n_sols(scip) <= 0 {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobjval = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobjval, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Only call heuristic, if the best solution comes from transformed
    // problem.
    let bestsol = api::get_best_sol(scip);
    if crate::sol::sol_is_original(scip, bestsol) {
        return Ok(());
    }

    // Only call heuristic, if enough nodes were processed since last
    // incumbent.
    let d = hd(scip, heur);
    if api::get_n_nodes(scip) - scip.sols[bestsol].nodenum < d.nwaitingnodes as i64 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotRun;

    let h = &scip.heurs[heur];

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut nnodes = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Reward RINS if it succeeded often.
    nnodes = (nnodes as f64 * (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0)) as i64;
    // Count the setup costs for the sub-MIP as 100 nodes.
    nnodes -= (100.0 * h.ncalls as f64) as i64;
    nnodes += d.nodesofs as i64;

    // Determine the node limit for the current process.
    nnodes -= d.usednodes;
    nnodes = if nnodes <= d.maxnodes as i64 { nnodes } else { d.maxnodes as i64 };

    // Check whether we have enough nodes left to call subproblem solving.
    if nnodes < d.minnodes as i64 {
        return Ok(());
    }

    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);

    // Check whether discrete variables are available.
    if nbinvars == 0 && nintvars == 0 {
        return Ok(());
    }

    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Determine possible fixings for RINS: variables with same value in
    // bestsol and LP relaxation.
    let minfixingrate = hd(scip, heur).minfixingrate;
    let (fixedvars, fixedvals, success) = determine_fixings(scip, minfixingrate);

    // Too few variables could be fixed by the RINS scheme.
    if !success {
        return Ok(());
    }

    // Check whether there is enough time and memory left; abort if no time
    // is left or not enough memory to create a copy of SCIP.
    if !crate::copy::check_copy_limits(scip)? {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let mut subscip = Scip::create()?;
    wrapper_rins(scip, &mut subscip, heur, &vars, &fixedvars, &fixedvals, result, nnodes)
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurRins`: creates the RINS primal heuristic and includes it
/// in SCIP.
pub fn include_heur_rins(scip: &mut Scip) -> ScipResult {
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
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
        heur_exec_rins,
        Some(Box::new(HeurData {
            nodesofs: DEFAULT_NODESOFS,
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            minfixingrate: DEFAULT_MINFIXINGRATE,
            nwaitingnodes: DEFAULT_NWAITINGNODES,
            minimprove: DEFAULT_MINIMPROVE,
            nodelimit: 0.0,
            lplimfac: DEFAULT_LPLIMFAC,
            usednodes: 0,
            nodesquot: DEFAULT_NODESQUOT,
            uselprows: DEFAULT_USELPROWS,
            copycuts: DEFAULT_COPYCUTS,
            useuct: DEFAULT_USEUCT,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_rins);
    h.heurinit = Some(heur_init_rins);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add RINS primal heuristic parameters.
    paramset_add_int(
        scip,
        "heuristics/rins/nodesofs",
        "number of nodes added to the contingent of the total nodes",
        t!(i32, nodesofs),
        false,
        DEFAULT_NODESOFS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/rins/maxnodes",
        "maximum number of nodes to regard in the subproblem",
        t!(i32, maxnodes),
        true,
        DEFAULT_MAXNODES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/rins/minnodes",
        "minimum number of nodes required to start the subproblem",
        t!(i32, minnodes),
        true,
        DEFAULT_MINNODES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rins/nodesquot",
        "contingent of sub problem nodes in relation to the number of nodes of the original problem",
        t!(f64, nodesquot),
        false,
        DEFAULT_NODESQUOT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/rins/nwaitingnodes",
        "number of nodes without incumbent change that heuristic should wait",
        t!(i32, nwaitingnodes),
        true,
        DEFAULT_NWAITINGNODES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rins/minimprove",
        "factor by which rins should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rins/minfixingrate",
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
        "heuristics/rins/lplimfac",
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
        "heuristics/rins/uselprows",
        "should subproblem be created out of the rows in the LP rows?",
        t!(bool, uselprows),
        true,
        DEFAULT_USELPROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rins/copycuts",
        "if uselprows == FALSE, should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rins/useuct",
        "should uct node selection be used at the beginning of the search?",
        t!(bool, useuct),
        true,
        DEFAULT_USEUCT,
        None,
    )?;

    Ok(())
}
