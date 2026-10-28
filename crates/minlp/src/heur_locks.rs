//! Port of `heur_locks.c`: fixes binary variables to the bound with fewer
//! rounding locks, propagating after each fixing, then solves the LP of the
//! remaining problem and, if its solution does not round, a sub-MIP.

use std::any::Any;

use crate::{
    api,
    copy::CopyMaps,
    heur::{HEURTIMING_BEFORENODE, HeurTiming},
    misc::RandNumGen,
    paramset::{ParamSetting, paramset_set_bool, paramset_set_int, paramset_set_longint},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_probing,
    types::{HeurId, LockType, LpSolStat, RowId, ScipResultCode, VarId, VarStatus, VarType},
    var::NONE,
};

const HEUR_NAME: &str = "locks";
const HEUR_DESC: &str = "heuristic that fixes variables based on their rounding locks";
const HEUR_DISPCHAR: u8 = b'p';
const HEUR_PRIORITY: i32 = 3000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_BEFORENODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i64 = 5000;
/// Probability for rounding a variable up in case of ties.
const DEFAULT_ROUNDUPPROBABILITY: f64 = 0.67;
/// Minimum percentage of variables that have to be fixed.
const DEFAULT_MINFIXINGRATE: f64 = 0.65;
/// Factor by which locks heuristic should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.01;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i64 = 500;
/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i64 = 500;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.1;
/// Maximum number of propagation rounds during probing.
const DEFAULT_MAXPROPROUNDS: i32 = 2;
/// Should the locks be updated based on LP rows?
const DEFAULT_UPDATELOCKS: bool = true;
/// Should all active cuts from the cutpool of the original scip be copied
/// to constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Should a final sub-MIP be solved to construct a feasible solution if the
/// LP was not roundable?
const DEFAULT_USEFINALSUBMIP: bool = true;
/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 73;
/// Minimum fixing rate over all variables (including continuous) to solve
/// LP.
const DEFAULT_MINFIXINGRATELP: f64 = 0.0;

/// `SCIP_HeurData`.
pub struct HeurData {
    randnumgen: Option<RandNumGen>,
    maxnodes: i64,
    minnodes: i64,
    nodesofs: i64,
    usednodes: i64,
    roundupprobability: f64,
    minfixingrate: f64,
    minfixingratelp: f64,
    minimprove: f64,
    nodesquot: f64,
    maxproprounds: i32,
    updatelocks: bool,
    copycuts: bool,
    usefinalsubmip: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut().unwrap()
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyLocks`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_locks(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_locks(scip)
}

/// `heurInitLocks`: initialization method of primal heuristic (called after
/// problem was transformed).
fn heur_init_locks(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Initialize data and create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    let d = hdm(scip, heur);
    d.usednodes = 0;
    d.randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `heurExitLocks`: deinitialization method of primal heuristic (called
/// before transformed problem is freed).
fn heur_exit_locks(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free random number generator.
    hdm(scip, heur).randnumgen = None;
    Ok(())
}

/// Swaps positions `a` and `b` of the sorted variables and their locks,
/// keeping `varpos` up to date.
fn swap_vars(scip: &Scip, sortvars: &mut [VarId], nuplocks: &mut [i32], ndownlocks: &mut [i32], varpos: &mut Option<Vec<i32>>, a: usize, b: usize) {
    sortvars.swap(a, b);
    nuplocks.swap(a, b);
    ndownlocks.swap(a, b);
    if let Some(varpos) = varpos {
        varpos[scip.vars[sortvars[a]].probindex as usize] = a as i32;
        varpos[scip.vars[sortvars[b]].probindex as usize] = b as i32;
    }
}

/// `SCIPapplyLockFixings`: applies the fix-and-propagate scheme based on
/// variable locks (in probing mode); returns `(cutoff, allrowsfulfilled)`.
/// `heurdata` `None` uses the locks heuristic's own data.
pub fn apply_lock_fixings(scip: &mut Scip, heur: HeurId) -> ScipResult<(bool, bool)> {
    let heur = if heur == NONE { crate::set::set_find_heur(scip, HEUR_NAME) } else { heur };

    let mut cutoff = false;
    let mut allrowsfulfilled = false;

    let d = hd(scip, heur);
    let propagate = d.maxproprounds != 0;
    let maxproprounds = if d.maxproprounds == -2 { 0 } else { d.maxproprounds };
    let roundupprobability = d.roundupprobability;
    let updatelocks = d.updatelocks && crate::api::get_n_check_conss(scip) == api::get_n_lp_rows(scip);

    let vars = api::get_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip) as usize;

    // Allocate memory.
    let mut sortvars: Vec<VarId> = vars[..nbinvars].to_vec();
    let mut nuplocks = vec![0i32; nbinvars];
    let mut ndownlocks = vec![0i32; nbinvars];
    let mut varpos: Option<Vec<i32>> = None;

    // Get LP data.
    let lprows: Vec<RowId> = api::get_lp_rows(scip);
    let nlprows = lprows.len();
    let mut minact = vec![0.0; nlprows];
    let mut maxact = vec![0.0; nlprows];
    let mut fulfilled = vec![false; nlprows];

    let mut nglbfulfilledrows = 0usize;

    // Get locks of variables.
    for v in 0..nbinvars {
        let var = sortvars[v];
        nuplocks[v] = crate::var::var_get_n_locks_up_type(scip, var, LockType::Model);
        ndownlocks[v] = crate::var::var_get_n_locks_down_type(scip, var, LockType::Model);
    }

    // Get activities of rows.
    for r in 0..nlprows {
        let row = lprows[r];
        minact[r] = crate::lp::row_get_min_activity(scip, row);
        maxact[r] = crate::lp::row_get_max_activity(scip, row);
    }

    let mut propagated = true;
    let mut lastbestscore = i32::MAX;

    // Fix variables.
    let mut v = 0usize;
    while v < nbinvars {
        if crate::solve::solve_is_stopped(scip, false) {
            break;
        }

        let mut nfulfilledrows = 0;

        while v < nbinvars && (scip.vars[sortvars[v]].locdom.lb > 0.5 || scip.vars[sortvars[v]].locdom.ub < 0.5) {
            v += 1;
        }
        if v == nbinvars {
            break;
        }

        let mut bestpos = v;
        let mut bestscore = nuplocks[v] + ndownlocks[v];

        // Get best variable.
        if bestscore < lastbestscore {
            let mut i = v + 1;
            while i < nbinvars {
                let var = sortvars[i];

                // Variable is already fixed; move it to the front and
                // increment v to ignore it.
                if scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5 {
                    swap_vars(scip, &mut sortvars, &mut nuplocks, &mut ndownlocks, &mut varpos, i, v);
                    if bestpos == v {
                        bestpos = i;
                    }
                    v += 1;
                    i += 1;
                    continue;
                }

                let score = nuplocks[i] + ndownlocks[i];
                if score > bestscore {
                    bestscore = score;
                    bestpos = i;
                    if bestscore == lastbestscore {
                        break;
                    }
                }
                i += 1;
            }
            if v == nbinvars {
                break;
            }
        }
        lastbestscore = bestscore;

        // Move best variable to the front (at position v).
        if bestpos != v {
            swap_vars(scip, &mut sortvars, &mut nuplocks, &mut ndownlocks, &mut varpos, bestpos, v);
        }
        let var = sortvars[v];

        // All remaining variables are fixed, we can break the fix-and-propagate
        // loop.
        if scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5 {
            break;
        }

        // Stop if we reached the depth limit.
        if crate::def::MAXTREEDEPTH <= api::get_depth(scip) {
            break;
        }

        if propagated {
            scip_probing::new_probing_node(scip)?;
            propagated = false;
        }

        // Set variables to the bound with fewer locks, if tie choose an
        // average value.
        let mut lastfixval = if ndownlocks[v] > nuplocks[v] {
            1.0
        } else if ndownlocks[v] < nuplocks[v] {
            0.0
        } else if !scip.set.is_positive(scip.vars[var].obj) {
            // Prefer one-fixing if objective value is not positive.
            1.0
        } else {
            let randnumber = hdm(scip, heur).randnumgen.as_mut().unwrap().get_real(0.0, 1.0);

            // If a tie occurs, we randomly round the variable based on the
            // parameter 'roundupprobability'.
            if randnumber < roundupprobability { 1.0 } else { 0.0 }
        };

        let lastfixlocks = if lastfixval > 0.5 { nuplocks[v] } else { ndownlocks[v] };

        scip_probing::fix_var_probing(scip, var, lastfixval)?;

        if propagate && lastfixlocks > 0 {
            // Apply propagation.
            cutoff = scip_probing::propagate_probing(scip, maxproprounds)?.0;
            propagated = true;

            if cutoff {
                // Fix cutoff variable in other direction.
                let depth = scip_probing::get_probing_depth(scip);
                scip_probing::backtrack_probing(scip, depth - 1)?;
                cutoff = false;

                if lastfixval < 0.5 {
                    lastfixval = 1.0;

                    if scip.vars[var].locdom.ub > 0.5 {
                        scip_probing::fix_var_probing(scip, var, 1.0)?;
                    } else {
                        // Because of the limited number of propagation rounds,
                        // it may happen that conflict analysis finds a valid
                        // global bound for the last fixed variable that
                        // conflicts with applying the reverse bound change
                        // after backtracking; in that case, we ran into a
                        // deadend and stop.
                        cutoff = true;
                    }
                } else {
                    lastfixval = 0.0;

                    if scip.vars[var].locdom.lb < 0.5 {
                        scip_probing::fix_var_probing(scip, var, 0.0)?;
                    } else {
                        // See the comment above.
                        cutoff = true;
                    }
                }

                if !cutoff {
                    cutoff = scip_probing::propagate_probing(scip, maxproprounds)?.0;
                }
                if cutoff {
                    break;
                }
            }
        }

        if updatelocks {
            if scip.vars[var].varstatus != VarStatus::Column {
                v += 1;
                continue;
            }

            let col = scip.vars[var].data.col;
            let ncolrows = scip.cols[col].len as usize;
            let colrows: Vec<RowId> = scip.cols[col].rows.0[..ncolrows].to_vec();
            let colvals: Vec<f64> = scip.cols[col].vals.0[..ncolrows].to_vec();

            // Update activities.
            for r in 0..ncolrows {
                let row = colrows[r];
                let rowpos = scip.rows[row].lppos;

                // The row is not in the LP.
                if rowpos == -1 {
                    continue;
                }
                let rowpos = rowpos as usize;

                // We disregard cuts.
                if scip.rows[row].rank > 0 {
                    continue;
                }

                // The row is already fulfilled.
                if fulfilled[rowpos] {
                    continue;
                }

                let (lhs, rhs) = (scip.rows[row].lhs, scip.rows[row].rhs);
                let haslhs = !scip.set.is_infinity(-lhs);
                let hasrhs = !scip.set.is_infinity(rhs);

                if (colvals[r] > 0.0) == (lastfixval < 0.5) {
                    maxact[rowpos] -= colvals[r].abs();
                }
                if (colvals[r] > 0.0) == (lastfixval > 0.5) {
                    minact[rowpos] += colvals[r].abs();
                }

                // Check if the row cannot be violated anymore.
                if (!haslhs || scip.set.is_feas_ge(minact[rowpos], lhs)) && (!hasrhs || scip.set.is_feas_le(maxact[rowpos], rhs)) {
                    let varpos = varpos.get_or_insert_with(|| {
                        let mut p = vec![0i32; nbinvars];
                        for (pos, &sv) in sortvars.iter().enumerate() {
                            p[scip.vars[sv].probindex as usize] = pos as i32;
                        }
                        p
                    });

                    nfulfilledrows += 1;
                    fulfilled[rowpos] = true;

                    let ncols = scip.rows[row].len as usize;

                    // We assume that all rows are locking the variables.
                    for w in (0..ncols).rev() {
                        let colvar = scip.cols[scip.rows[row].cols[w]].var;
                        let val = scip.rows[row].vals[w];
                        if scip.vars[colvar].vartype == VarType::Binary && !crate::var::var_is_implied_integral(scip, colvar) && colvar != var {
                            let pos = varpos[scip.vars[colvar].probindex as usize] as usize;
                            if haslhs {
                                if val > 0.0 {
                                    ndownlocks[pos] -= 1;
                                } else {
                                    nuplocks[pos] -= 1;
                                }
                            }
                            if hasrhs {
                                if val > 0.0 {
                                    nuplocks[pos] -= 1;
                                } else {
                                    ndownlocks[pos] -= 1;
                                }
                            }
                        }
                    }
                    continue;
                } else if scip.set.is_feas_lt(maxact[rowpos], lhs) || scip.set.is_feas_gt(minact[rowpos], rhs) {
                    cutoff = true;
                    break;
                }
            }

            if cutoff {
                break;
            }

            nglbfulfilledrows += nfulfilledrows;

            if nglbfulfilledrows == nlprows {
                allrowsfulfilled = true;
                break;
            }
        }
        v += 1;
    }

    Ok((cutoff, allrowsfulfilled))
}

/// `heurExecLocks`: execution method of primal heuristic.
fn heur_exec_locks(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only run once.
    if api::get_n_runs(scip) > 1 {
        return Ok(());
    }

    if api::get_n_bin_vars(scip) == 0 {
        return Ok(());
    }

    // Only run if we are allowed to solve an LP at the current node in the
    // tree.
    if !api::has_current_node_lp(scip) {
        return Ok(());
    }

    if !crate::tree::tree_is_focus_node_lp_constructed(scip) {
        let cutoff = api::construct_lp(scip)?;

        // Manually cut off the node if the LP construction detected
        // infeasibility (heuristics cannot return such a result).
        if cutoff {
            let node = crate::tree::tree_get_current_node(scip);
            api::cutoff_node(scip, node)?;
            return Ok(());
        }

        api::flush_lp(scip)?;

        // We need an LP.
        if api::get_n_lp_rows(scip) == 0 {
            return Ok(());
        }
    }

    *result = ScipResultCode::DidNotFind;

    let mut lowerbound = api::get_lowerbound(scip);
    let oldnpscands = api::get_n_pseudo_branch_cands(scip);

    // Start probing mode.
    scip_probing::start_probing(scip)?;

    let (mut cutoff, allrowsfulfilled) = apply_lock_fixings(scip, heur)?;

    'terminate: {
        if cutoff || crate::solve::solve_is_stopped(scip, false) {
            break 'terminate;
        }

        // Check that we had enough fixings.
        let npscands = api::get_n_pseudo_branch_cands(scip);

        if !allrowsfulfilled && npscands as f64 > oldnpscands as f64 * (1.0 - hd(scip, heur).minfixingrate) {
            break 'terminate;
        }

        if api::get_n_cont_vars(scip) > 0 {
            let nvars = api::get_n_vars(scip);
            let nminfixings = scip.set.ceil(hd(scip, heur).minfixingratelp * nvars as f64) as i32;
            let mut nfixedvars = 0;

            // Count fixed variables.
            let vars = api::get_vars(scip);
            let mut i = 0;
            while i < nvars as usize && nfixedvars < nminfixings {
                let v = &scip.vars[vars[i]];
                if scip.set.is_eq(v.locdom.lb, v.locdom.ub) {
                    nfixedvars += 1;
                }
                i += 1;
            }

            if nfixedvars < nminfixings {
                break 'terminate;
            }
        }

        let mut lperror = false;

        // Solve LP; errors in the LP solver should not kill the overall
        // solving process, if the LP is just needed for a heuristic. Hence
        // in optimized mode, the return code is caught and a warning is
        // printed, only in debug mode, SCIP will stop.
        match scip_probing::solve_probing_lp(scip, -1) {
            Ok((e, c)) => {
                lperror = e;
                cutoff = c;
            }
            Err(retstat) => log::warn!("Error while solving LP in LOCKS heuristic; LP solve terminated with code <{retstat:?}>"),
        }

        let lpstatus = api::get_lp_solstat(scip);

        // Check if this is a feasible solution.
        if !lperror && lpstatus == LpSolStat::Optimal {
            lowerbound = api::get_lp_objval(scip);

            // Create a copy of the current LP solution.
            let sol = api::create_sol(scip, heur)?;
            crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

            let success = api::round_sol(scip, sol)?;

            if success {
                // Check solution for feasibility, and add it to solution
                // store if possible; neither integrality nor feasibility of
                // LP rows has to be checked, because they are guaranteed by
                // the heuristic at this stage.
                let stored = api::try_sol(scip, sol, false, false, false, false, false)?;
                if stored {
                    *result = ScipResultCode::FoundSol;
                }

                api::free_sol(scip, sol)?;

                // We found a solution, so we are done.
                break 'terminate;
            }

            api::free_sol(scip, sol)?;
        }

        if hd(scip, heur).usefinalsubmip && !cutoff && !lperror && lpstatus != LpSolStat::Infeasible && lpstatus != LpSolStat::ObjLimit {
            let d = hd(scip, heur);
            let h = &scip.heurs[heur];

            // Calculate the maximal number of branching nodes until heuristic
            // is aborted.
            let mut nstallnodes = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

            // Reward locks heuristic if it succeeded often.
            nstallnodes = (nstallnodes as f64 * 3.0 * (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0)) as i64;
            // Count the setup costs for the sub-MIP as 100 nodes.
            nstallnodes -= 100 * h.ncalls;
            nstallnodes += d.nodesofs;

            // Determine the node limit for the current process.
            nstallnodes -= d.usednodes;
            nstallnodes = if nstallnodes <= d.maxnodes { nstallnodes } else { d.maxnodes };

            // Check whether we have enough nodes left to call subproblem
            // solving.
            if nstallnodes < d.minnodes {
                break 'terminate;
            }

            // Check whether there is enough time and memory left.
            if !crate::copy::check_copy_limits(scip)? {
                break 'terminate;
            }

            // Get all variables.
            let vars = api::get_vars(scip).to_vec();
            let nvars = vars.len();

            // Create subproblem.
            let mut subscip = Scip::create()?;

            // Create the variable mapping hash map.
            let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

            crate::copy::copy(scip, &mut subscip, &mut maps, "_locks", false, false)?;

            if hd(scip, heur).copycuts {
                // Copies all active cuts from cutpool of sourcescip to linear
                // constraints in targetscip.
                crate::copy::copy_cuts_public(scip, &mut subscip, &mut maps, false)?;
            }

            let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

            // Do not abort subproblem on CTRL-C.
            paramset_set_bool(&mut subscip, "misc/catchctrlc", false)?;

            // Disable statistic timing inside sub SCIP and output to console.
            paramset_set_int(&mut subscip, "display/verblevel", 0)?;
            paramset_set_bool(&mut subscip, "timing/statistictiming", false)?;

            // Set limits for the subproblem.
            crate::copy::copy_limits(scip, &mut subscip)?;
            paramset_set_longint(&mut subscip, "limits/stallnodes", nstallnodes)?;
            paramset_set_longint(&mut subscip, "limits/nodes", hd(scip, heur).maxnodes)?;

            // Forbid call of heuristics and separators solving sub-CIPs.
            crate::paramset::set_subscips_off(&mut subscip, true)?;

            // Disable cutting plane separation.
            crate::paramset::set_separating(&mut subscip, ParamSetting::Off, true)?;

            // Disable expensive presolving.
            crate::paramset::set_presolving(&mut subscip, ParamSetting::Fast, true)?;

            // Use inference branching.
            if crate::set::set_find_branchrule(&subscip, "inference") != NONE && !crate::paramset::paramset_is_fixed(&subscip, "branching/inference/priority")? {
                paramset_set_int(&mut subscip, "branching/inference/priority", i32::MAX / 4)?;
            }

            // Speed up sub-SCIP by not checking dual LP feasibility.
            paramset_set_bool(&mut subscip, "lp/checkdualfeas", false)?;

            // If there is already a solution, add an objective cutoff.
            if api::get_n_sols(scip) > 0 {
                let minimprove = hd(scip, heur).minimprove;
                let upperbound = api::get_upperbound(scip) - scip.set.sumepsilon();

                let cutoffbound = if !scip.set.is_infinity(-lowerbound) {
                    (1.0 - minimprove) * api::get_upperbound(scip) + minimprove * lowerbound
                } else if api::get_upperbound(scip) >= 0.0 {
                    (1.0 - minimprove) * api::get_upperbound(scip)
                } else {
                    (1.0 + minimprove) * api::get_upperbound(scip)
                };
                let cutoffbound = crate::def::min(upperbound, cutoffbound);
                api::set_objlimit(&mut subscip, cutoffbound)?;
            }

            // Solve the subproblem; errors in the LP solver should not kill
            // the overall solving process.
            'freescip: {
                if let Err(retstat) = crate::scip_solve::presolve_prob(&mut subscip) {
                    log::warn!("Error while presolving subMIP in locks heuristic; sub-SCIP terminated with code <{retstat:?}>");
                    break 'freescip;
                }

                // After presolving, we should have at least reached a certain
                // fixing rate over ALL variables (including continuous) to
                // ensure that not only continuous variables are left.
                if (nvars as i32 - api::get_n_vars(&subscip)) as f64 / nvars as f64 >= hd(scip, heur).minfixingrate {
                    if let Err(retstat) = crate::scip_solve::solve(&mut subscip) {
                        log::warn!("Error while solving subMIP in locks heuristic; sub-SCIP terminated with code <{retstat:?}>");
                        break 'freescip;
                    }

                    // Check, whether a solution was found; due to numerics, it
                    // might happen that not all solutions are feasible -> try
                    // all solutions until one was accepted.
                    let (success, _) = crate::copy::translate_sub_sols(scip, &subscip, heur, &subvars)?;
                    if success {
                        *result = ScipResultCode::FoundSol;
                    }
                }

                hdm(scip, heur).usednodes += api::get_n_nodes(&subscip);
            }
        }
    }

    // Exit probing mode.
    scip_probing::end_probing(scip)?;

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurLocks`: creates the locks primal heuristic and includes
/// it in SCIP.
pub fn include_heur_locks(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_longint, paramset_add_real};

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
        heur_exec_locks,
        Some(Box::new(HeurData {
            randnumgen: None,
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            nodesofs: DEFAULT_NODESOFS,
            usednodes: 0,
            roundupprobability: DEFAULT_ROUNDUPPROBABILITY,
            minfixingrate: DEFAULT_MINFIXINGRATE,
            minfixingratelp: DEFAULT_MINFIXINGRATELP,
            minimprove: DEFAULT_MINIMPROVE,
            nodesquot: DEFAULT_NODESQUOT,
            maxproprounds: DEFAULT_MAXPROPROUNDS,
            updatelocks: DEFAULT_UPDATELOCKS,
            copycuts: DEFAULT_COPYCUTS,
            usefinalsubmip: DEFAULT_USEFINALSUBMIP,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_locks);
    h.heurinit = Some(heur_init_locks);
    h.heurexit = Some(heur_exit_locks);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add locks primal heuristic parameters.
    paramset_add_int(
        scip,
        "heuristics/locks/maxproprounds",
        "maximum number of propagation rounds to be performed in each propagation call (-1: no limit, -2: parameter settings)",
        t!(i32, maxproprounds),
        true,
        DEFAULT_MAXPROPROUNDS,
        -2,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/locks/minfixingrate",
        "minimum percentage of integer variables that have to be fixable",
        t!(f64, minfixingrate),
        false,
        DEFAULT_MINFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/locks/roundupprobability",
        "probability for rounding a variable up in case of ties",
        t!(f64, roundupprobability),
        false,
        DEFAULT_ROUNDUPPROBABILITY,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/locks/usefinalsubmip",
        "should a final sub-MIP be solved to costruct a feasible solution if the LP was not roundable?",
        t!(bool, usefinalsubmip),
        true,
        DEFAULT_USEFINALSUBMIP,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/locks/maxnodes",
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
        "heuristics/locks/nodesofs",
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
        "heuristics/locks/minnodes",
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
        "heuristics/locks/nodesquot",
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
        "heuristics/locks/minimprove",
        "factor by which locks heuristic should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/locks/copycuts",
        "should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/locks/updatelocks",
        "should the locks be updated based on LP rows?",
        t!(bool, updatelocks),
        true,
        DEFAULT_UPDATELOCKS,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/locks/minfixingratelp",
        "minimum fixing rate over all variables (including continuous) to solve LP",
        t!(f64, minfixingratelp),
        true,
        DEFAULT_MINFIXINGRATELP,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
