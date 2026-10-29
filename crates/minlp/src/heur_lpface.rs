//! Port of `heur_lpface.c`: LNS heuristic that searches the optimal face of
//! the LP inside a sub-MIP, fixing the variables with nonzero reduced costs
//! and requiring the objective to equal the lower bound.

use std::any::Any;

use crate::{
    api,
    copy::CopyMaps,
    event::{EVENTTYPE_LPSOLVED, Event},
    heur::{HEURTIMING_AFTERLPNODE, HeurTiming},
    paramset::{ParamSetting, paramset_set_bool, paramset_set_char, paramset_set_int, paramset_set_longint, paramset_set_real},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    types::{BranchDir, EventhdlrId, HeurId, LpSolStat, ScipResultCode, Status, VarId, VarStatus},
    var::NONE,
};

const HEUR_NAME: &str = "lpface";
const HEUR_DESC: &str = "LNS heuristic that searches the optimal LP face inside a sub-MIP";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1104010;
const HEUR_FREQ: i32 = 15;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i64 = 5000;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i64 = 50;
/// Required percentage of fixed integer variables in sub-MIP to run.
const DEFAULT_MINFIXINGRATE: f64 = 0.1;
/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i64 = 200;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.1;
/// Factor by which the limit on the number of LP depends on the node limit.
const DEFAULT_LPLIMFAC: f64 = 2.0;
/// Should subproblem be created out of the rows in the LP rows, otherwise,
/// the copy constructors of the constraints handlers are used.
const DEFAULT_USELPROWS: bool = true;
/// If uselprows == FALSE, should all active cuts from cutpool be copied to
/// constraints in subproblem?
const DEFAULT_COPYCUTS: bool = true;
/// Should the dually nonbasic rows be turned into equations?
const DEFAULT_DUALBASISEQUATIONS: bool = false;
/// Should the heuristic continue solving the same sub-SCIP?
const DEFAULT_KEEPSUBSCIP: bool = false;
/// The minimum active search tree path length along which the lower bound
/// hasn't changed before heuristic becomes active.
const DEFAULT_MINPATHLEN: i32 = 5;

const EVENTHDLR_NAME: &str = "Lpface";
const EVENTHDLR_DESC: &str = "LP event handler for lpface heuristic";

/// `SUBSCIPDATA`: the sub-SCIP kept across runs.
struct SubscipData {
    subscip: Option<Box<Scip>>,
    subvars: Vec<VarId>,
    objbound: f64,
}

/// `SCIP_HeurData`.
struct HeurData {
    maxnodes: i64,
    minnodes: i64,
    nodesofs: i64,
    usednodes: i64,
    nodesquot: f64,
    nfailures: u32,
    nextnodenumber: i64,
    lastlpobjinfeas: f64,
    minfixingrate: f64,
    nodelimit: f64,
    lplimfac: f64,
    uselprows: bool,
    copycuts: bool,
    dualbasisequations: bool,
    keepsubscip: bool,
    subscipobjective: u8,
    submipstatus: Status,
    submipnlpiters: i64,
    submippresoltime: f64,
    nvarsfixed: i32,
    minpathlen: i32,
    subscipdata: Option<SubscipData>,
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

/// `determineVariableFixings`: the variable fixings for the sub-SCIP based
/// on reduced costs; returns `(fixvars, fixvals, success)`.
fn determine_variable_fixings(scip: &mut Scip, heur: HeurId) -> (Vec<VarId>, Vec<f64>, bool) {
    // Get required data of the main scip problem.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    let mut fixingcounter = 0;
    let mut fixvars = Vec::new();
    let mut fixvals = Vec::new();

    // Loop over problem variables and fix all with nonzero reduced costs to
    // their solution value.
    for &var in &vars {
        // Skip non-column variables.
        if scip.vars[var].varstatus != VarStatus::Column {
            continue;
        }

        // Skip relaxation only variables.
        if scip.vars[var].relaxationonly {
            continue;
        }

        let solval = api::get_sol_val(scip, NONE, var);
        let col = scip.vars[var].data.col;
        let redcost = api::get_col_redcost(scip, col);
        let lbglobal = scip.vars[var].glbdom.lb;
        let ubglobal = scip.vars[var].glbdom.ub;

        // Fix the variable to its solution value if variable is nonbasic
        // (i.e., at one of its bounds) with nonzero reduced costs.
        if !scip.set.is_dualfeas_zero(redcost) {
            // Fix variable based on reduced cost information, respecting
            // global bounds.
            if (redcost > 0.0 && scip.set.is_feas_eq(solval, lbglobal)) || (redcost < 0.0 && scip.set.is_feas_eq(solval, ubglobal)) {
                fixvars.push(var);
                fixvals.push(solval);

                if crate::var::var_is_integral(scip, var) {
                    fixingcounter += 1;
                }
            }
        }
    }

    let fixingrate = fixingcounter as f64 / crate::def::max((nbinvars + nintvars) as f64, 1.0);
    hdm(scip, heur).nvarsfixed = fixingcounter as i32;

    // If all variables were fixed or amount of fixed variables is
    // insufficient, skip residual part of subproblem creation and abort
    // immediately.
    let success = fixingcounter < nvars && fixingrate >= hd(scip, heur).minfixingrate;
    (fixvars, fixvals, success)
}

/// `createRows`: creates the rows of the subproblem.
fn create_rows(scip: &mut Scip, subscip: &mut Scip, subvars: &[VarId], dualbasisequations: bool) -> ScipResult {
    // Get the rows and their number.
    let rows = api::get_lp_rows(scip);

    // Copy all global rows to linear constraints, unless they contain
    // relaxation-only variables.
    for row in rows {
        // Ignore rows that are only locally valid.
        if scip.rows[row].local {
            continue;
        }

        // Get the row's data.
        let r = &scip.rows[row];
        let constant = r.constant;
        let nnonz = r.len as usize;
        let vals = r.vals.0[..nnonz].to_vec();
        let cols = r.cols.0[..nnonz].to_vec();

        // Only subtract constant if left hand side is not infinite.
        let mut lhs = r.lhs;
        if !scip.set.is_infinity(-lhs) {
            lhs -= constant;
        }

        // Only subtract constant if right hand side is not infinite.
        let mut rhs = r.rhs;
        if !scip.set.is_infinity(rhs) {
            rhs -= constant;
        }

        // Fill the corresponding subproblem variables; skip row if not all
        // variables are in sub-SCIP, i.e., relaxation-only variables.
        let consvars: Vec<VarId> = cols.iter().map(|&c| subvars[scip.vars[scip.cols[c].var].probindex as usize]).collect();
        if consvars.contains(&NONE) {
            continue;
        }

        let dualsol = scip.rows[row].dualsol;
        let rowsolactivity = if crate::tree::tree_has_current_node_lp(scip) {
            crate::lp::row_get_lp_activity(scip, row)
        } else {
            crate::lp::row_get_pseudo_activity(scip, row)
        };

        // Transform into equation if the row is sharp and has a nonzero dual
        // solution.
        if dualbasisequations && !scip.set.is_dualfeas_zero(dualsol) {
            if dualsol > 0.0 && scip.set.is_feas_eq(rowsolactivity, lhs) {
                rhs = lhs;
            } else if dualsol < 0.0 && scip.set.is_feas_eq(rowsolactivity, rhs) {
                lhs = rhs;
            }
        }

        // Create a new linear constraint and add it to the subproblem.
        let name = scip.rows[row].name.clone();
        let mut cons = crate::cons_linear::create_cons_linear(subscip, &name, &consvars, &vals, lhs, rhs, true, true, true, true, true, false, false, true, true, false)?;
        crate::scip_cons::add_cons(subscip, cons)?;
        crate::scip_cons::release_cons(subscip, &mut cons)?;
    }
    Ok(())
}

/// `setupSubproblem`: creates the LP face subproblem constraints.
fn setup_subproblem(scip: &mut Scip, subscip: &mut Scip, subvars: &[VarId], heur: HeurId) -> ScipResult {
    let vars = api::get_vars(scip).to_vec();

    // We copy the rows of the LP, if enough variables could be fixed and we
    // work on the MIP relaxation of the problem.
    if hd(scip, heur).uselprows {
        let dualbasisequations = hd(scip, heur).dualbasisequations;
        create_rows(scip, subscip, subvars, dualbasisequations)?;
    }

    // Add an equation that the objective function must be equal to the
    // lower bound.
    let lowerbound = api::get_lowerbound(scip);
    let mut origobjcons = crate::cons_linear::create_cons_linear(
        subscip,
        "objbound_of_origscip",
        &[],
        &[],
        lowerbound,
        lowerbound,
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
        if !subscip.set.is_zero(obj) {
            crate::cons_linear::add_coef_linear(subscip, origobjcons, subvars[i], obj)?;
        }
    }

    crate::scip_cons::add_cons(subscip, origobjcons)?;
    crate::scip_cons::release_cons(subscip, &mut origobjcons)?;
    Ok(())
}

/// `updateFailureStatistic`: updates heurdata after an unsuccessful run of
/// lpface.
fn update_failure_statistic(scip: &mut Scip, heur: HeurId) {
    let nnodes = api::get_n_nodes(scip);
    let d = hdm(scip, heur);

    // Increase number of failures, calculate next node at which lpface
    // should be called and update actual solutions.
    d.nfailures += 1;
    d.nextnodenumber = if d.nfailures <= 25 { nnodes + 100 * (2i64 << d.nfailures) } else { i64::MAX };
}

/// `calcNodeLimit`: a node limit based on node limiting parameters of the
/// heuristic.
fn calc_node_limit(scip: &Scip, heur: HeurId, keptsubscipnodes: Option<i64>) -> i64 {
    let d = hd(scip, heur);

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut nodelimit = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Count the setup costs for the sub-MIP as 100 nodes.
    nodelimit -= 100 * scip.heurs[heur].ncalls;

    // Add the offset.
    nodelimit += d.nodesofs;

    // Subtract previously used nodes.
    nodelimit -= d.usednodes;

    // Do not use more than the maximum number of allowed nodes in one run.
    nodelimit = if nodelimit <= d.maxnodes { nodelimit } else { d.maxnodes };

    // If the subscip has been kept from a previous run, add the number of
    // already processed nodes.
    if let Some(n) = keptsubscipnodes {
        nodelimit += n;
    }

    nodelimit
}

/// The number of nodes of the kept sub-SCIP, if there is one.
fn kept_subscip_nodes(scip: &Scip, heur: HeurId) -> Option<i64> {
    hd(scip, heur).subscipdata.as_ref().and_then(|s| s.subscip.as_ref()).map(|s| api::get_n_nodes(s))
}

/// `setSubscipLimits`: sets node, time, and memory limit according to the
/// parameter settings of the heuristic; returns success.
fn set_subscip_limits(scip: &mut Scip, subscip: &mut Scip, heur: HeurId, keptsubscipnodes: Option<i64>) -> ScipResult<bool> {
    // Check whether there is enough time and memory left.
    let mut timelimit = crate::paramset::paramset_get_real(scip, "limits/time")?;
    let memorylimit = crate::paramset::paramset_get_real(scip, "limits/memory")?;

    if !scip.set.is_infinity(timelimit) {
        timelimit -= api::get_solving_time(scip);
    }

    // Subtract the memory already used by the main SCIP and the estimated
    // memory usage of external software: SCIP's memory accounting is not
    // ported, both count as zero.

    // Abort if no time is left or not enough memory (we don't abort in this
    // case if misc_avoidmemout == FALSE) to create a copy of SCIP, including
    // external memory usage.
    let avoidmemout = crate::paramset::paramset_get_bool(scip, "misc/avoidmemout")?;
    if timelimit <= 0.0 || (avoidmemout && memorylimit <= 0.0) {
        return Ok(false);
    }

    // Calculate node limit for the subproblem.
    let nodelimit = calc_node_limit(scip, heur, keptsubscipnodes);

    paramset_set_longint(subscip, "limits/nodes", nodelimit)?;
    hdm(scip, heur).nodelimit = nodelimit as f64;

    // Set also the other two limits.
    paramset_set_real(subscip, "limits/time", timelimit)?;
    paramset_set_real(subscip, "limits/memory", memorylimit)?;

    // Disable bound limits.
    paramset_set_real(subscip, "limits/primal", crate::def::INVALID)?;
    paramset_set_real(subscip, "limits/dual", crate::def::INVALID)?;

    Ok(true)
}

/// `setSubscipParameters`: sets all one-time parameter settings like search
/// strategy, but no limits.
fn set_subscip_parameters(subscip: &mut Scip) -> ScipResult {
    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable statistic timing inside sub SCIP.
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    // Forbid recursive call of heuristics and separators solving subMIPs.
    crate::paramset::set_subscips_off(subscip, true)?;

    // Disable expensive cutting plane separation.
    crate::paramset::set_separating(subscip, ParamSetting::Fast, true)?;

    // Disable expensive presolving.
    crate::paramset::set_presolving(subscip, ParamSetting::Fast, true)?;

    // Use restart depth first node selection.
    if crate::set::set_find_nodesel(subscip, "restartdfs") != NONE && !crate::paramset::paramset_is_fixed(subscip, "nodeselection/restartdfs/stdpriority")? {
        paramset_set_int(subscip, "nodeselection/restartdfs/stdpriority", i32::MAX / 4)?;
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
    Ok(())
}

/// `changeSubvariableObjective`: sets the objective coefficient of a
/// sub-SCIP variable by the user's choice of objective.
fn change_subvariable_objective(scip: &Scip, subscip: &mut Scip, var: VarId, subvar: VarId, subscipobjective: u8) -> ScipResult {
    let objcoeff = match subscipobjective {
        // Use zero as objective function.
        b'z' => 0.0,
        // Use current LP fractionality as objective.
        b'f' => {
            let lpsolval = crate::var::var_get_lp_sol(scip, var);
            let downfrac = scip.set.frac(lpsolval);
            let upfrac = 1.0 - downfrac;
            downfrac - upfrac
        }
        // Use root LP solution difference.
        b'r' => {
            let lpsolval = crate::var::var_get_lp_sol(scip, var);
            let rootlpsolval = crate::var::var_get_root_sol(scip, var);
            rootlpsolval - lpsolval
        }
        // Use average inferences.
        b'i' => crate::var::var_get_avg_inferences(scip, var, BranchDir::Downwards) - crate::var::var_get_avg_inferences(scip, var, BranchDir::Upwards),
        // Use original objective function.
        b'o' => scip.vars[var].obj,
        _ => 0.0,
    };

    crate::scip_var::chg_var_obj(subscip, subvar, objcoeff)
}

/// The data the sub-SCIP's LP event handler reads: C passes it the
/// heuristic's data, here it keeps its own copy of the two fields.
struct EventhdlrData {
    lplimfac: f64,
    nodelimit: f64,
}

/// `eventExecLpface`: execution callback of the event handler for Lpface
/// sub-SCIP.
fn event_exec_lpface(scip: &mut Scip, eventhdlr: EventhdlrId, _event: &Event, _eventdata: usize) -> ScipResult {
    let d: &EventhdlrData = scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref().unwrap();

    // Interrupt solution process of sub-SCIP.
    if api::get_n_lps(scip) as f64 > d.lplimfac * d.nodelimit {
        crate::scip_solve::interrupt_solve(scip);
    }
    Ok(())
}

/// `setupSubscipLpface`: sets up the subproblem; returns the sub-SCIP
/// variables in the order of the main SCIP's.
fn setup_subscip_lpface(scip: &mut Scip, subscip: &mut Scip, heur: HeurId, vars: &[VarId], fixvars: &[VarId], fixvals: &[f64]) -> ScipResult<Vec<VarId>> {
    let nvars = vars.len();

    // Create the variable hash map.
    let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

    if hd(scip, heur).uselprows {
        // Copy all plugins.
        crate::copy::copy_plugins(scip, subscip, true, true, true, true, true, true, true, true, true, true, true)?;

        // Copy parameter settings.
        crate::copy::copy_param_settings(scip, subscip)?;

        // Get name of the original problem and add the string "_lpfacesub".
        let probname = format!("{}_lpfacesub", scip.probs[crate::prob::ORIGPROB].name);

        // Create the subproblem.
        crate::scip_solve::create_prob(subscip, &probname)?;
        subscip.stat.subscipdepth = crate::copy::get_subscip_depth(scip) + 1;

        // Copy all variables.
        crate::copy::copy_vars_public(scip, subscip, &mut maps, fixvars, fixvals, true)?;
    } else {
        crate::copy::copy_cons_compression(scip, subscip, &mut maps, "lpface", fixvars, fixvals, true, false)?;

        if hd(scip, heur).copycuts {
            // Copies all active cuts from cutpool of sourcescip to linear
            // constraints in targetscip.
            crate::copy::copy_cuts_public(scip, subscip, &mut maps, true)?;
        }
    }

    // Fill subvars array with mapping from original variables and set the
    // objective coefficient to the desired value.
    let subscipobjective = hd(scip, heur).subscipobjective;
    let mut subvars = vec![NONE; nvars];
    for i in 0..nvars {
        subvars[i] = maps.var_image(vars[i]);
        if subvars[i] != NONE {
            change_subvariable_objective(scip, subscip, vars[i], subvars[i], subscipobjective)?;
        }
    }

    // Disable output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;

    // Fix variables that are at their bounds and have nonzero reduced costs.
    setup_subproblem(scip, subscip, &subvars, heur)?;

    // Set up sub-SCIP parameters.
    set_subscip_parameters(subscip)?;

    Ok(subvars)
}

/// `solveSubscipLpface`: solves the subproblem; returns whether the
/// sub-SCIP should be kept.
fn solve_subscip_lpface(
    scip: &mut Scip,
    subscip: &mut Scip,
    heur: HeurId,
    subvars: &[VarId],
    result: &mut ScipResultCode,
    focusnodelb: f64,
    keptsubscipnodes: Option<i64>,
    mut keepthisscip: bool,
) -> ScipResult<bool> {
    // Create event handler for LP events.
    let lplimfac = hd(scip, heur).lplimfac;
    let eventhdlr = api::include_eventhdlr_basic(
        subscip,
        EVENTHDLR_NAME,
        EVENTHDLR_DESC,
        event_exec_lpface,
        Some(Box::new(EventhdlrData { lplimfac, nodelimit: 0.0 })),
    )?;

    // Determine node, memory, and time limits for the sub-SCIP. Both node
    // and time limit change with every call to the heuristic.
    let success = set_subscip_limits(scip, subscip, heur, keptsubscipnodes)?;

    // If we did not succeed to set the limits of the subscip to let it run,
    // we won't keep it any longer.
    if !success {
        return Ok(false);
    }
    let nodelimit = hd(scip, heur).nodelimit;
    subscip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_mut()
        .unwrap()
        .downcast_mut::<EventhdlrData>()
        .unwrap()
        .nodelimit = nodelimit;

    // Catch LP events of sub-SCIP.
    crate::scip_solve::transform_prob(subscip)?;
    api::catch_event(subscip, EVENTTYPE_LPSOLVED, eventhdlr, 0, None)?;

    // Solve the subproblem.
    crate::scip_solve::solve(subscip)?;

    // Save useful information regarding the subscip runs.
    let d = hdm(scip, heur);
    d.usednodes += api::get_n_nodes(subscip);
    d.submipnlpiters += subscip.stat.nlpiterations;
    d.submippresoltime += subscip.stat.presolvingtime.get_time();
    d.submipstatus = api::get_status(subscip);

    // Store the focus node lower bound as infeasible to avoid running on this
    // face again.
    if d.submipstatus == Status::Infeasible {
        d.lastlpobjinfeas = focusnodelb;
        keepthisscip = false;
    } else if api::get_n_sols(subscip) > 0 {
        // Check, whether a solution was found; due to numerics, it might
        // happen that not all solutions are feasible -> try all solutions
        // until one was accepted.
        let (success, solindex) = crate::copy::translate_sub_sols(scip, subscip, heur, subvars)?;

        // We found an optimal solution and are done. Thus, we free the subscip
        // immediately.
        if success {
            keepthisscip = false;
            *result = ScipResultCode::FoundSol;
        }

        // If solution could not be added to problem => run is counted as a
        // failure.
        if !success || solindex != scip.sols[api::get_best_sol(scip)].index {
            update_failure_statistic(scip, heur);
        }
    } else {
        // If no new solution was found, run was a failure.
        update_failure_statistic(scip, heur);
    }

    Ok(keepthisscip)
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyLpface`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_lpface(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_lpface(scip)
}

/// `heurInitLpface`: initialization method of primal heuristic (called
/// after problem was transformed).
fn heur_init_lpface(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Initialize data.
    let d = hdm(scip, heur);
    d.usednodes = 0;
    d.nfailures = 0;
    d.nextnodenumber = 0;
    d.submipstatus = Status::Unknown;
    d.submipnlpiters = -1;
    d.submippresoltime = 0.0;
    d.nvarsfixed = -1;
    Ok(())
}

/// `heurInitsolLpface`: solving process initialization method of primal
/// heuristic (called when branch and bound process is about to begin).
fn heur_initsol_lpface(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset the last infeasible objective because it lives in transformed
    // space and must be invalidated at every restart.
    let inf = scip.set.infinity();
    let d = hdm(scip, heur);
    d.lastlpobjinfeas = -inf;

    // Variable order might have changed since the last run, reinitialize
    // sub-SCIP data.
    d.subscipdata = Some(SubscipData {
        subscip: None,
        subvars: Vec::new(),
        objbound: crate::def::INVALID,
    });
    Ok(())
}

/// `heurExitsolLpface`: solving process deinitialization method of primal
/// heuristic (called before branch and bound process is exiting).
fn heur_exitsol_lpface(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Variable order might change after restart, free the heuristic
    // subscipdata (and a kept sub-SCIP).
    hdm(scip, heur).subscipdata = None;
    Ok(())
}

/// `heurExecLpface`: execution method of primal heuristic.
fn heur_exec_lpface(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Delayed;

    // We skip infeasible nodes.
    if nodeinfeasible {
        return Ok(());
    }

    // Do not run heuristic on nodes that were not solved to optimality.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // LP face requires that the LP defines a valid lower bound for the
    // current node.
    if !crate::lp::lp_is_relax(scip) || !crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) {
        return Ok(());
    }

    let currentnode = crate::tree::tree_get_current_node(scip);
    let focusnodelb = scip.nodes[currentnode].lowerbound;

    // Do not run if the current focus node already has a lower bound higher
    // than the LP value at the node, for example, due to strong branching.
    let lpobjval = api::get_lp_objval(scip);
    if scip.set.is_gt(focusnodelb, lpobjval) {
        return Ok(());
    }

    // Only run at lower bound defining nodes.
    if scip.set.is_gt(focusnodelb, api::get_lowerbound(scip)) {
        return Ok(());
    }

    // Delay heuristic if the active search tree path is not deep enough.
    if api::get_depth(scip) < hd(scip, heur).minpathlen - 1 {
        return Ok(());
    }

    // The node number to run the heuristic again was not yet reached.
    if api::get_n_nodes(scip) < hd(scip, heur).nextnodenumber {
        return Ok(());
    }

    // Only run if lower bound has increased since last LP objective where the
    // sub-MIP was solved to infeasibility.
    if scip.set.is_eq(hd(scip, heur).lastlpobjinfeas, focusnodelb) {
        return Ok(());
    }

    // Make the reasoning stronger if the objective value must be integral.
    if api::is_obj_integral(scip) && (!scip.set.is_integral(focusnodelb) || scip.set.is_lt(focusnodelb, hd(scip, heur).lastlpobjinfeas + 1.0)) {
        return Ok(());
    }

    let rootlb = scip.stat.rootlowerbound;

    // If the lower bound hasn't changed since the root node, we want to run
    // anyway, otherwise we base our decision on the total path length since
    // the last lower bound improvement.
    if scip.set.is_lt(rootlb, focusnodelb) {
        let minpathlen = hd(scip, heur).minpathlen;

        // The length of the current path (in edges) along which the lower
        // bound stayed the same.
        let mut nonimprovingpathlen = 0;
        let mut parent = scip.nodes[currentnode].parent;

        // Count the path length along which the dual bound has not changed.
        while scip.set.is_eq(scip.nodes[parent].lowerbound, focusnodelb) && nonimprovingpathlen < minpathlen {
            nonimprovingpathlen += 1;

            // We cannot hit the root node because the root lower bound is
            // strictly smaller.
            parent = scip.nodes[parent].parent;
        }

        // We return if the nonimproving path is too short measured by the
        // heuristic frequency.
        if nonimprovingpathlen < minpathlen {
            // We do not delay the heuristic if the path has length zero,
            // otherwise it may be called at children so that the
            // nonimproving path length reaches the threshold.
            if nonimprovingpathlen == 0 {
                *result = ScipResultCode::DidNotRun;
            }
            return Ok(());
        }
    }

    *result = ScipResultCode::DidNotRun;

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted; check whether we have enough nodes left to call subproblem
    // solving.
    let nodelimit = calc_node_limit(scip, heur, kept_subscip_nodes(scip, heur));
    if nodelimit < hd(scip, heur).minnodes {
        return Ok(());
    }

    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);

    // Check whether discrete variables are present.
    if nbinvars == 0 && nintvars == 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let keepthisscip = hd(scip, heur).keepsubscip;

    // Check if variable number increased since last call to the sub-SCIP;
    // or if the lower bound has increased, the kept sub-SCIP is no longer
    // useful.
    {
        let sd = hd(scip, heur).subscipdata.as_ref().unwrap();
        if sd.subscip.is_some() && (sd.subvars.len() != nvars || scip.set.is_gt(focusnodelb, sd.objbound)) {
            let sd = hdm(scip, heur).subscipdata.as_mut().unwrap();
            *sd = SubscipData {
                subscip: None,
                subvars: Vec::new(),
                objbound: crate::def::INVALID,
            };
        }
    }

    // Retrieve the sub-SCIP from the heuristic data structure.
    let kept = hdm(scip, heur).subscipdata.as_mut().unwrap().subscip.take();
    let wasrestored = kept.is_some();
    let keptsubscipnodes = kept.as_ref().map(|s| api::get_n_nodes(s));
    let (mut subscip, subvars) = match kept {
        Some(subscip) => {
            let subvars = hd(scip, heur).subscipdata.as_ref().unwrap().subvars.clone();
            (subscip, subvars)
        }
        None => {
            let (fixvars, fixvals, success) = determine_variable_fixings(scip, heur);

            if !success {
                *result = ScipResultCode::DidNotRun;
                return Ok(());
            }

            // Initialize the subproblem.
            let mut subscip = Scip::create()?;
            let subvars = setup_subscip_lpface(scip, &mut subscip, heur, &vars, &fixvars, &fixvals)?;
            (subscip, subvars)
        }
    };

    let keepthisscip = solve_subscip_lpface(scip, &mut subscip, heur, &subvars, result, focusnodelb, keptsubscipnodes, keepthisscip)?;

    // Free subproblem or store it for the next run of the heuristic.
    let sd = hdm(scip, heur).subscipdata.as_mut().unwrap();
    if !keepthisscip {
        *sd = SubscipData {
            subscip: None,
            subvars: Vec::new(),
            objbound: crate::def::INVALID,
        };
    } else if wasrestored {
        sd.subscip = Some(subscip);
    } else {
        // If the subscip has not yet been stored, we copy the subscip into the
        // heuristic data to keep it for the next run.
        let objbound = scip.nodes[currentnode].lowerbound;
        let sd = hdm(scip, heur).subscipdata.as_mut().unwrap();
        sd.subscip = Some(subscip);
        sd.subvars = subvars;
        sd.objbound = objbound;
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurLpface`: creates the lpface primal heuristic and
/// includes it in SCIP.
pub fn include_heur_lpface(scip: &mut Scip) -> ScipResult {
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
        heur_exec_lpface,
        Some(Box::new(HeurData {
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            nodesofs: DEFAULT_NODESOFS,
            usednodes: 0,
            nodesquot: DEFAULT_NODESQUOT,
            nfailures: 0,
            nextnodenumber: 0,
            lastlpobjinfeas: 0.0,
            minfixingrate: DEFAULT_MINFIXINGRATE,
            nodelimit: 0.0,
            lplimfac: DEFAULT_LPLIMFAC,
            uselprows: DEFAULT_USELPROWS,
            copycuts: DEFAULT_COPYCUTS,
            dualbasisequations: DEFAULT_DUALBASISEQUATIONS,
            keepsubscip: DEFAULT_KEEPSUBSCIP,
            subscipobjective: b'z',
            submipstatus: Status::Unknown,
            submipnlpiters: -1,
            submippresoltime: 0.0,
            nvarsfixed: -1,
            minpathlen: DEFAULT_MINPATHLEN,
            subscipdata: None,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_lpface);
    h.heurinit = Some(heur_init_lpface);
    h.heurinitsol = Some(heur_initsol_lpface);
    h.heurexitsol = Some(heur_exitsol_lpface);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add lpface primal heuristic parameters.
    paramset_add_longint(
        scip,
        "heuristics/lpface/nodesofs",
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
        "heuristics/lpface/maxnodes",
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
        "heuristics/lpface/minnodes",
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
        "heuristics/lpface/nodesquot",
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
        "heuristics/lpface/minfixingrate",
        "required percentage of fixed integer variables in sub-MIP to run",
        t!(f64, minfixingrate),
        false,
        DEFAULT_MINFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/lpface/lplimfac",
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
        "heuristics/lpface/uselprows",
        "should subproblem be created out of the rows in the LP rows?",
        t!(bool, uselprows),
        true,
        DEFAULT_USELPROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/lpface/dualbasisequations",
        "should dually nonbasic rows be turned into equations?",
        t!(bool, dualbasisequations),
        true,
        DEFAULT_DUALBASISEQUATIONS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/lpface/keepsubscip",
        "should the heuristic continue solving the same sub-SCIP?",
        t!(bool, keepsubscip),
        true,
        DEFAULT_KEEPSUBSCIP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/lpface/copycuts",
        "if uselprows == FALSE, should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/lpface/subscipobjective",
        "objective function in the sub-SCIP: (z)ero, (r)oot-LP-difference, (i)nference, LP (f)ractionality, (o)riginal",
        t!(u8, subscipobjective),
        true,
        b'z',
        Some("forzi"),
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/lpface/minpathlen",
        "the minimum active search tree path length along which lower bound hasn't changed before heuristic becomes active",
        t!(i32, minpathlen),
        true,
        DEFAULT_MINPATHLEN,
        0,
        65531,
        None,
    )?;

    Ok(())
}
