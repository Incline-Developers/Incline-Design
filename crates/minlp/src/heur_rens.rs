//! Port of `heur_rens.c`: LNS heuristic that finds the optimal rounding to
//! a given point, by solving a sub-SCIP with the integral variables of the
//! LP solution fixed.
//!
//! The NLP relaxation as start solution (`startsol = 'n'`) is not ported:
//! without a constructed NLP the C falls back to the LP solution too.

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

const HEUR_NAME: &str = "rens";
const HEUR_DESC: &str = "LNS exploring fractional neighborhood of relaxation's optimum";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1100000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Should general integers get binary bounds [floor(.),ceil(.)]?
const DEFAULT_BINARYBOUNDS: bool = true;
/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i64 = 5000;
/// Minimum percentage of integer variables that have to be fixed.
const DEFAULT_MINFIXINGRATE: f64 = 0.5;
/// Factor by which RENS should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.01;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i64 = 50;
/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i64 = 500;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.1;
/// Factor by which the limit on the number of LP depends on the node limit.
const DEFAULT_LPLIMFAC: f64 = 2.0;
/// Solution that is used for fixing values.
const DEFAULT_STARTSOL: u8 = b'l';
/// Possible values for startsol ('l'p relaxation, 'n'lp relaxation).
const STARTSOL_CHOICES: &str = "nl";
/// Should subproblem be created out of the rows in the LP rows?
const DEFAULT_USELPROWS: bool = false;
/// If DEFAULT_USELPROWS is FALSE, then should all active cuts from the
/// cutpool of the original scip be copied to constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Should the RENS sub-CIP get its own full time limit?
const DEFAULT_EXTRATIME: bool = false;
/// Should all subproblem solutions be added to the original SCIP?
const DEFAULT_ADDALLSOLS: bool = false;
/// Should the RENS sub-CIP be solved with full-scale SCIP settings?
const DEFAULT_FULLSCALE: bool = false;
/// Limit on number of improving incumbent solutions in sub-CIP.
const DEFAULT_BESTSOLLIMIT: i32 = -1;
/// Should uct node selection be used at the beginning of the search?
const DEFAULT_USEUCT: bool = false;

const EVENTHDLR_NAME: &str = "Rens";
const EVENTHDLR_DESC: &str = "LP event handler for rens heuristic";

/// `SCIP_HeurData`.
struct HeurData {
    maxnodes: i64,
    minnodes: i64,
    nodesofs: i64,
    usednodes: i64,
    minfixingrate: f64,
    minimprove: f64,
    nodesquot: f64,
    nodelimit: f64,
    lplimfac: f64,
    startsol: u8,
    binarybounds: bool,
    uselprows: bool,
    copycuts: bool,
    extratime: bool,
    addallsols: bool,
    fullscale: bool,
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

/// `computeFixingrate`: the variables to fix and their values; returns
/// `(fixedvars, fixedvals, fixingrate, success)`.
fn compute_fixingrate(scip: &Scip, minfixingrate: f64, startsol: &mut u8) -> (Vec<VarId>, Vec<f64>, f64, bool) {
    let mut fixingrate = 1.0;
    let mut fixedvars = Vec::new();
    let mut fixedvals = Vec::new();

    // If there is no NLP relaxation available (e.g., because the presolved
    // problem is linear), use LP relaxation.
    if !api::is_nlp_constructed(scip) {
        *startsol = b'l';
    }

    // Get required variable data.
    let vars = api::get_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    // Try to solve NLP relaxation.
    if *startsol == b'n' {
        unimplemented!("RENS on the NLP relaxation");
    }

    // Count the number of variables with integral solution values in the
    // current NLP or LP solution.
    for &var in &vars[..nbinvars + nintvars] {
        // Get solution value in the relaxation in question.
        let solval = crate::var::var_get_lp_sol(scip, var);

        // Append variable to the buffer storage for integer variables with
        // integer solution values.
        if scip.set.is_feas_integral(solval) {
            // Fix variables to current LP/NLP solution if it is integral, use
            // exact integral value, if the variable is only integral within
            // numerical tolerances.
            let solval = scip.set.floor(solval + 0.5);
            fixedvars.push(var);
            fixedvals.push(solval);
        }
    }

    // Abort, if all integer variables were fixed (which should not happen for
    // MIP), but frequently happens for MINLPs using an LP relaxation.
    if fixedvars.len() == nbinvars + nintvars {
        return (fixedvars, fixedvals, fixingrate, false);
    }

    fixingrate = fixedvars.len() as f64 / crate::def::max((nbinvars + nintvars) as f64, 1.0);

    // Abort, if the amount of fixed variables is insufficient.
    if fixingrate < minfixingrate {
        return (fixedvars, fixedvals, fixingrate, false);
    }

    (fixedvars, fixedvals, fixingrate, true)
}

/// `restrictToBinaryBounds`: restricts the bounds of the general integer
/// variables of the subproblem to the nearest integers of their solution
/// values.
fn restrict_to_binary_bounds(scip: &Scip, subscip: &mut Scip, subvars: &[VarId], startsol: u8) -> ScipResult {
    // Get required variable data.
    let vars = api::get_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    // Change bounds of integer variables of the subproblem.
    for i in nbinvars..nbinvars + nintvars {
        if subvars[i] == NONE {
            continue;
        }

        // Get the current LP/NLP solution for each variable.
        debug_assert_eq!(startsol, b'l');
        let solval = crate::var::var_get_lp_sol(scip, vars[i]);

        // Restrict bounds to nearest integers if the solution value is not
        // already integer.
        if !scip.set.is_feas_integral(solval) {
            let lb = scip.set.feas_floor(solval);
            let ub = scip.set.feas_ceil(solval);

            // Perform the bound change.
            crate::scip_var::chg_var_lb_global(subscip, subvars[i], lb)?;
            crate::scip_var::chg_var_ub_global(subscip, subvars[i], ub)?;
        }
    }
    Ok(())
}

/// The data the sub-SCIP's LP event handler reads: C passes it the
/// heuristic's data, here it keeps its own copy of the two fields.
struct EventhdlrData {
    lplimfac: f64,
    nodelimit: f64,
}

/// `eventExecRens`: interrupts the sub-SCIP once it solved too many LPs.
fn event_exec_rens(scip: &mut Scip, eventhdlr: EventhdlrId, _event: &Event, _eventdata: usize) -> ScipResult {
    let d: &EventhdlrData = scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref().unwrap();

    // Interrupt solution process of sub-SCIP.
    if api::get_n_lps(scip) as f64 > d.lplimfac * d.nodelimit {
        crate::scip_solve::interrupt_solve(scip);
    }
    Ok(())
}

/// `setupAndSolveSubscip`: copies the problem into `subscip` with the
/// fixings, solves it and transfers its solutions.
#[allow(clippy::too_many_arguments)]
fn setup_and_solve_subscip(
    scip: &mut Scip,
    subscip: &mut Scip,
    result: &mut ScipResultCode,
    heur: HeurId,
    fixedvars: &[VarId],
    fixedvals: &[f64],
    minfixingrate: f64,
    minimprove: f64,
    maxnodes: i64,
    nstallnodes: i64,
    startsol: u8,
    binarybounds: bool,
    uselprows: bool,
) -> ScipResult {
    // Get variable data.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Create the variable mapping hash map.
    let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

    // Create a problem copy as sub SCIP.
    let copycuts = hd(scip, heur).copycuts;
    crate::copy::copy_large_neighborhood_search(scip, subscip, &mut maps, "rens", fixedvars, fixedvals, uselprows, copycuts)?;

    // Create event handler for LP events.
    let lplimfac = hd(scip, heur).lplimfac;
    let eventhdlr = api::include_eventhdlr_basic(
        subscip,
        EVENTHDLR_NAME,
        EVENTHDLR_DESC,
        event_exec_rens,
        Some(Box::new(EventhdlrData { lplimfac, nodelimit: 0.0 })),
    )?;

    // Copy subproblem variables into the same order as the source SCIP
    // variables.
    let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

    // Restrict the integer variables to binary bounds.
    if binarybounds {
        restrict_to_binary_bounds(scip, subscip, &subvars, startsol)?;
    }

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable statistic timing inside sub SCIP and output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    // Set limits for the subproblem.
    crate::copy::copy_limits(scip, subscip)?;
    hdm(scip, heur).nodelimit = maxnodes as f64;
    subscip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_mut()
        .unwrap()
        .downcast_mut::<EventhdlrData>()
        .unwrap()
        .nodelimit = maxnodes as f64;
    paramset_set_longint(subscip, "limits/stallnodes", nstallnodes)?;
    paramset_set_longint(subscip, "limits/nodes", maxnodes)?;
    paramset_set_int(subscip, "limits/bestsol", hd(scip, heur).bestsollimit)?;

    // Forbid recursive call of heuristics and separators solving sub-SCIPs.
    crate::paramset::set_subscips_off(subscip, true)?;

    // Disable expensive techniques that merely work on the dual bound.
    if !hd(scip, heur).fullscale {
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

        // Enable conflict analysis, disable analysis of boundexceeding LPs,
        // and restrict conflict pool.
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
    }

    // If there is already a solution, add an objective cutoff.
    if api::get_n_sols(scip) > 0 {
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
    }

    // Presolve the subproblem.
    crate::scip_solve::presolve_prob(subscip)?;

    let norigvars = subscip.probs[crate::prob::ORIGPROB].nvars;
    let mut allfixingrate = (norigvars - api::get_n_vars(subscip)) as f64 / norigvars as f64;

    // Additional variables added in presolving may lead to the subSCIP
    // having more variables than the original.
    allfixingrate = crate::def::max(allfixingrate, 0.0);

    // After presolving, we should have at least reached a certain fixing rate
    // over ALL variables (including continuous) to ensure that not only
    // continuous variables are left.
    if allfixingrate >= minfixingrate / 2.0 {
        // Catch LP events of sub-SCIP.
        crate::scip_solve::transform_prob(subscip)?;
        api::catch_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, None)?;

        // Solve the subproblem.
        crate::scip_solve::solve(subscip)?;

        // Drop LP events of sub-SCIP.
        api::drop_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, -1)?;

        // Transfer variable statistics from sub-SCIP.
        crate::copy::merge_variable_statistics(subscip, scip, &subvars, &vars)?;

        // Check, whether a solution was found; due to numerics, it might
        // happen that not all solutions are feasible -> try all solutions
        // until one was accepted.
        let subsols = api::get_sols(subscip).to_vec();
        let addallsols = hd(scip, heur).addallsols;
        let mut success = false;
        let mut i = 0;
        while i < subsols.len() && (!success || addallsols) {
            let newsol = crate::copy::translate_sub_sol(scip, subscip, subsols[i], heur, &subvars)?;

            success = api::try_sol_free(scip, newsol, false, false, true, true, true)?;
            if success {
                *result = ScipResultCode::FoundSol;
            }
            i += 1;
        }
    }
    Ok(())
}

/// `SCIPapplyRens`: main procedure of the RENS heuristic, creates and
/// solves a sub-SCIP.
#[allow(clippy::too_many_arguments)]
pub fn apply_rens(
    scip: &mut Scip,
    heur: HeurId,
    result: &mut ScipResultCode,
    minfixingrate: f64,
    minimprove: f64,
    maxnodes: i64,
    nstallnodes: i64,
    mut startsol: u8,
    binarybounds: bool,
    uselprows: bool,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Compute the number of initial fixings and check if the fixing rate
    // exceeds the minimum fixing rate.
    let (fixedvars, fixedvals, _intfixingrate, success) = compute_fixingrate(scip, minfixingrate, &mut startsol);
    if !success {
        return Ok(());
    }

    // Check whether there is enough time and memory left.
    if !crate::copy::check_copy_limits(scip)? {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Initialize the subproblem.
    let mut subscip = Scip::create()?;

    setup_and_solve_subscip(
        scip,
        &mut subscip,
        result,
        heur,
        &fixedvars,
        &fixedvals,
        minfixingrate,
        minimprove,
        maxnodes,
        nstallnodes,
        startsol,
        binarybounds,
        uselprows,
    )
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyRens`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_rens(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_rens(scip)
}

/// `heurInitRens`: initialization method of primal heuristic (called after
/// problem was transformed).
fn heur_init_rens(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Initialize data.
    hdm(scip, heur).usednodes = 0;
    Ok(())
}

/// `heurExecRens`: execution method of primal heuristic.
fn heur_exec_rens(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Delayed;

    // Do not call heuristic of node was already detected to be infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    let startsol = hd(scip, heur).startsol;

    // Only call heuristic, if an optimal LP solution is at hand.
    if startsol == b'l' && api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    if startsol == b'l' {
        let lpobjval = api::get_lp_objval(scip);
        if scip.set.is_ge(lpobjval, api::get_cutoffbound(scip)) {
            return Ok(());
        }
    }

    // Only continue with some fractional variables.
    if startsol == b'l' && api::get_n_lp_branch_cands(scip) == 0 {
        return Ok(());
    }

    // Do not proceed, when we should use the NLP relaxation, but there is no
    // NLP solver included in SCIP (none is ported).
    if startsol == b'n' {
        return Ok(());
    }

    *result = ScipResultCode::DidNotRun;

    let d = hd(scip, heur);
    let h = &scip.heurs[heur];

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut nstallnodes = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Reward RENS if it succeeded often.
    nstallnodes = (nstallnodes as f64 * 3.0 * (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0)) as i64;
    // Count the setup costs for the sub-SCIP as 100 nodes.
    nstallnodes -= 100 * h.ncalls;
    nstallnodes += d.nodesofs;

    // Determine the node limit for the current process.
    nstallnodes -= d.usednodes;
    nstallnodes = if nstallnodes <= d.maxnodes { nstallnodes } else { d.maxnodes };

    // Check whether we have enough nodes left to call subproblem solving.
    if nstallnodes < d.minnodes {
        return Ok(());
    }

    let extratime = d.extratime;
    if crate::solve::solve_is_stopped(scip, false) && !extratime {
        return Ok(());
    }

    let d = hd(scip, heur);
    let (minfixingrate, minimprove, maxnodes, binarybounds, uselprows) = (d.minfixingrate, d.minimprove, d.maxnodes, d.binarybounds, d.uselprows);
    apply_rens(scip, heur, result, minfixingrate, minimprove, maxnodes, nstallnodes, startsol, binarybounds, uselprows)
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurRens`: creates the rens primal heuristic and includes it
/// in SCIP.
pub fn include_heur_rens(scip: &mut Scip) -> ScipResult {
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_longint, paramset_add_real},
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
        heur_exec_rens,
        Some(Box::new(HeurData {
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            nodesofs: DEFAULT_NODESOFS,
            usednodes: 0,
            minfixingrate: DEFAULT_MINFIXINGRATE,
            minimprove: DEFAULT_MINIMPROVE,
            nodesquot: DEFAULT_NODESQUOT,
            nodelimit: 0.0,
            lplimfac: DEFAULT_LPLIMFAC,
            startsol: DEFAULT_STARTSOL,
            binarybounds: DEFAULT_BINARYBOUNDS,
            uselprows: DEFAULT_USELPROWS,
            copycuts: DEFAULT_COPYCUTS,
            extratime: DEFAULT_EXTRATIME,
            addallsols: DEFAULT_ADDALLSOLS,
            fullscale: DEFAULT_FULLSCALE,
            bestsollimit: DEFAULT_BESTSOLLIMIT,
            useuct: DEFAULT_USEUCT,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_rens);
    h.heurinit = Some(heur_init_rens);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add rens primal heuristic parameters.
    paramset_add_real(
        scip,
        "heuristics/rens/minfixingrate",
        "minimum percentage of integer variables that have to be fixable",
        t!(f64, minfixingrate),
        false,
        DEFAULT_MINFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/rens/maxnodes",
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
        "heuristics/rens/nodesofs",
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
        "heuristics/rens/minnodes",
        "minimum number of nodes required to start the subproblem",
        t!(i64, minnodes),
        true,
        DEFAULT_MINNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rens/nodesquot",
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
        "heuristics/rens/minimprove",
        "factor by which RENS should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rens/lplimfac",
        "factor by which the limit on the number of LP depends on the node limit",
        t!(f64, lplimfac),
        true,
        DEFAULT_LPLIMFAC,
        1.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/rens/startsol",
        "solution that is used for fixing values ('l'p relaxation, 'n'lp relaxation)",
        t!(u8, startsol),
        false,
        DEFAULT_STARTSOL,
        Some(STARTSOL_CHOICES),
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rens/binarybounds",
        "should general integers get binary bounds [floor(.),ceil(.)] ?",
        t!(bool, binarybounds),
        true,
        DEFAULT_BINARYBOUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rens/uselprows",
        "should subproblem be created out of the rows in the LP rows?",
        t!(bool, uselprows),
        true,
        DEFAULT_USELPROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rens/copycuts",
        "if uselprows == FALSE, should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rens/extratime",
        "should the RENS sub-CIP get its own full time limit? This is only for testing and not recommended!",
        t!(bool, extratime),
        true,
        DEFAULT_EXTRATIME,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rens/addallsols",
        "should all subproblem solutions be added to the original SCIP?",
        t!(bool, addallsols),
        true,
        DEFAULT_ADDALLSOLS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rens/fullscale",
        "should the RENS sub-CIP be solved with cuts, conflicts, strong branching,... This is only for testing and not recommended!",
        t!(bool, fullscale),
        true,
        DEFAULT_FULLSCALE,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/rens/bestsollimit",
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
        "heuristics/rens/useuct",
        "should uct node selection be used at the beginning of the search?",
        t!(bool, useuct),
        true,
        DEFAULT_USEUCT,
        None,
    )?;

    Ok(())
}
