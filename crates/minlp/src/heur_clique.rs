//! Port of `heur_clique.c`: LNS heuristic that fixes, in each clique, the
//! variable of best objective to one and the others to zero, propagates,
//! and solves the LP and then a sub-MIP of what is left.

use std::any::Any;

use crate::{
    api,
    cons::ConsFlags,
    copy::CopyMaps,
    heur::{HEURTIMING_BEFORENODE, HeurTiming},
    paramset::{ParamSetting, paramset_set_bool, paramset_set_int, paramset_set_longint},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_probing,
    types::{ConfType, HeurId, LpSolStat, ScipResultCode, Status, VarId},
    var::NONE,
};

const HEUR_NAME: &str = "clique";
const HEUR_DESC: &str = "LNS heuristic using a clique partition to restrict the search neighborhood";
const HEUR_DISPCHAR: u8 = b'p';
const HEUR_PRIORITY: i32 = 5000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_BEFORENODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i64 = 5000;
/// Minimum percentage of integer variables that have to be fixed.
const DEFAULT_MININTFIXINGRATE: f64 = 0.65;
/// Minimum percentage of variables that have to be fixed within sub-SCIP
/// (integer and continuous).
const DEFAULT_MINMIPFIXINGRATE: f64 = 0.65;
/// Factor by which clique heuristic should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.01;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i64 = 500;
/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i64 = 500;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.1;
/// Maximum number of propagation rounds during probing.
const DEFAULT_MAXPROPROUNDS: i32 = 2;
/// Maximum number of backtracks during the fixing process.
const DEFAULT_MAXBACKTRACKS: i32 = 10;
/// Should all active cuts from the cutpool of the original scip be copied
/// to constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Should more variables be fixed based on variable locks if the fixing
/// rate was not reached?
const DEFAULT_USELOCKFIXINGS: bool = false;

/// `SCIP_HeurData`.
struct HeurData {
    maxnodes: i64,
    minnodes: i64,
    nodesofs: i64,
    usednodes: i64,
    minintfixingrate: f64,
    minmipfixingrate: f64,
    minimprove: f64,
    nodesquot: f64,
    maxproprounds: i32,
    maxbacktracks: i32,
    copycuts: bool,
    uselockfixings: bool,
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

/// `getCliqueUnfixedVars`: the number of unfixed variables of a clique.
fn get_clique_unfixed_vars(scip: &Scip, clique: usize) -> i32 {
    let cl = &scip.cliques[clique];
    let mut nunfixed = 0;
    for &var in &cl.vars.0[..cl.nvars as usize] {
        // Is variable unfixed?
        if scip.vars[var].locdom.ub > scip.vars[var].locdom.lb + 0.5 {
            nunfixed += 1;
        }
    }
    nunfixed
}

/// The flags of the heuristic's own conflict constraints.
fn conflict_flags() -> ConsFlags {
    ConsFlags {
        initial: false,
        separate: true,
        enforce: false,
        check: false,
        propagate: true,
        local: true,
        modifiable: false,
        dynamic: true,
        removable: true,
        stickingatnode: false,
    }
}

/// Creates the heuristic's own conflict, a logicor on the variables fixed
/// to one (negated) or zero.
fn create_conflict(scip: &mut Scip, onefixvars: &mut [VarId], onefixvals: &[bool]) -> ScipResult<crate::types::ConsId> {
    // Create own conflict.
    let consname = format!("conf{}", api::get_n_nodes(scip));

    // Get variables for the conflict.
    for i in 0..onefixvars.len() {
        // If the variable was fixed to 1 by the heuristic, get its negated
        // variable.
        if onefixvals[i] {
            onefixvars[i] = crate::scip_var::get_negated_var(scip, onefixvars[i])?;
        }
    }

    // Create conflict constraint.
    crate::cons_logicor::create_cons_logicor(scip, &consname, onefixvars, &conflict_flags())
}

/// `applyCliqueFixings`: applies clique fixings using probing; returns
/// whether the propagation stopped with infeasibility.
fn apply_clique_fixings(scip: &mut Scip, heur: HeurId, enabledconflicts: bool, onefixvars: &mut Vec<VarId>, onefixvals: &mut Vec<bool>) -> ScipResult<bool> {
    let (maxproprounds, maxbacktracks) = (hd(scip, heur).maxproprounds, hd(scip, heur).maxbacktracks);
    let cliques = crate::scip_var::get_cliques(scip).to_vec();
    let ncliques = cliques.len();

    let mut nbacktracks = 0;
    let mut propagated = vec![false; ncliques];

    let mut cliquesizes = vec![0i32; ncliques];
    for c in (0..ncliques).rev() {
        cliquesizes[c] = scip.cliques[cliques[c]].nvars;
    }

    let mut permutation: Vec<i32> = (0..ncliques as i32).collect();
    crate::sort::sort(&mut permutation, (), |&a, &b| cliquesizes[b as usize] - cliquesizes[a as usize]);
    let perm = |i: usize| permutation[i] as usize;

    let mut cutoff = false;
    let mut probingdepthofonefix = 0;
    let mut firstclique = 0usize;

    scip_probing::new_probing_node(scip)?;

    for _c in 0..ncliques {
        let mut bestpos: i32 = -1;
        let mut bestobj = scip.set.infinity();
        let mut alreadyone = false;
        let mut newnode = false;
        let mut bestclique = firstclique;

        if bestclique >= ncliques {
            break;
        }

        let mut bestcliquesize = get_clique_unfixed_vars(scip, cliques[perm(bestclique)]);

        for i in firstclique + 1..ncliques {
            if cliquesizes[perm(i)] < bestcliquesize {
                break;
            }

            if propagated[perm(i)] {
                continue;
            }

            let cliquesize = get_clique_unfixed_vars(scip, cliques[perm(i)]);

            if cliquesize > bestcliquesize {
                bestclique = i;
                bestcliquesize = cliquesize;
            } else if cliquesize == 0 {
                propagated[perm(i)] = true;
            }
        }
        let clique = cliques[perm(bestclique)];
        propagated[perm(bestclique)] = true;

        while firstclique < ncliques && propagated[perm(firstclique)] {
            firstclique += 1;
        }

        let ncliquevars = scip.cliques[clique].nvars as usize;
        let cliquevars: Vec<VarId> = scip.cliques[clique].vars.0[..ncliquevars].to_vec();
        let cliquevals: Vec<bool> = scip.cliques[clique].values.0[..ncliquevars].to_vec();

        let mut v = 0;
        while v < ncliquevars {
            let var = cliquevars[v];

            // Variable is already fixed.
            if scip.vars[var].locdom.ub < scip.vars[var].locdom.lb + 0.5 {
                // Clique variable is fixed to 1.
                if cliquevals[v] == (scip.vars[var].locdom.lb > 0.5) {
                    alreadyone = true;
                    break;
                }
                v += 1;
                continue;
            }

            let obj = if cliquevals[v] { scip.vars[var].obj } else { -scip.vars[var].obj };

            if obj < bestobj {
                // Variable is not the best one in the clique anymore, fix it to 0.
                if bestpos >= 0 {
                    let b = bestpos as usize;
                    scip_probing::fix_var_probing(scip, cliquevars[b], if cliquevals[b] { 0.0 } else { 1.0 })?;
                    newnode = true;
                }

                bestobj = obj;
                bestpos = v as i32;
            } else {
                // Variable is not the best one in the clique, fix it to 0.
                scip_probing::fix_var_probing(scip, var, if cliquevals[v] { 0.0 } else { 1.0 })?;
                newnode = true;
            }
            v += 1;
        }

        // We found a variable in the clique which is already fixed to 1.
        if alreadyone {
            // Fix (so far) best candidate to 0.
            if bestpos >= 0 {
                let b = bestpos as usize;
                scip_probing::fix_var_probing(scip, cliquevars[b], if cliquevals[b] { 0.0 } else { 1.0 })?;
                newnode = true;
            }

            // Fix all variables not yet processed to 0.
            while v < ncliquevars {
                let var = cliquevars[v];

                if scip.vars[var].locdom.ub < scip.vars[var].locdom.lb + 0.5 {
                    v += 1;
                    continue;
                }

                scip_probing::fix_var_probing(scip, var, if cliquevals[v] { 0.0 } else { 1.0 })?;
                newnode = true;
                v += 1;
            }
        }
        // Fix the best variable to 1.
        else if bestpos >= 0 {
            let b = bestpos as usize;
            onefixvars.push(cliquevars[b]);
            probingdepthofonefix = scip_probing::get_probing_depth(scip);

            if cliquevals[b] {
                scip_probing::fix_var_probing(scip, cliquevars[b], 1.0)?;
                onefixvals.push(true);
            } else {
                scip_probing::fix_var_probing(scip, cliquevars[b], 0.0)?;
                onefixvals.push(false);
            }
            newnode = true;
        }

        if newnode {
            // Propagate fixings.
            cutoff = scip_probing::propagate_probing(scip, maxproprounds)?.0;

            if crate::solve::solve_is_stopped(scip, false) {
                break;
            }

            // Stop if we reached the depth limit.
            if crate::def::MAXTREEDEPTH <= api::get_depth(scip) {
                break;
            }

            // Probing detected infeasibility: backtrack.
            if cutoff {
                if !onefixvars.is_empty() {
                    if probingdepthofonefix > 0 {
                        scip_probing::backtrack_probing(scip, probingdepthofonefix - 1)?;
                        probingdepthofonefix = 0;
                        nbacktracks += 1;

                        // Because of the limited number of propagation rounds,
                        // it may happen that conflict analysis finds a valid
                        // global bound for the last fixed variable that
                        // conflicts with applying the reverse fixing after
                        // backtracking; in that case, we ran into a deadend and
                        // stop.
                        let last = onefixvars.len() - 1;
                        let lastval = if onefixvals[last] { 1.0 } else { 0.0 };
                        let lastvar = onefixvars[last];
                        if scip.vars[lastvar].locdom.lb < 1.5 - lastval && scip.vars[lastvar].locdom.ub > 0.5 - lastval {
                            // Fix the last variable, which was fixed to 1 and led
                            // to the cutoff, to 0.
                            scip_probing::fix_var_probing(scip, lastvar, 1.0 - lastval)?;
                            onefixvars.pop();
                            onefixvals.pop();

                            // Propagate fixings.
                            cutoff = scip_probing::propagate_probing(scip, maxproprounds)?.0;
                        }
                    }

                    if cutoff {
                        if enabledconflicts {
                            let mut conflictcons = create_conflict(scip, onefixvars, onefixvals)?;
                            let focusnode = crate::tree::tree_get_focus_node(scip);
                            crate::scip_cons::add_conflict(scip, focusnode, &mut conflictcons, NONE, ConfType::Propagation, false)?;
                        }
                        break;
                    } else if nbacktracks > maxbacktracks {
                        break;
                    }
                }
                // We had a cutoff without a single one-fixing, so the current
                // problem seems to be infeasible already.
                else {
                    break;
                }
            }

            scip_probing::new_probing_node(scip)?;
        }
    }

    Ok(cutoff)
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyClique`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_clique(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_clique(scip)
}

/// `heurInitClique`: initialization method of primal heuristic (called
/// after problem was transformed).
fn heur_init_clique(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset heuristic data.
    hdm(scip, heur).usednodes = 0;
    Ok(())
}

/// `heurExecClique`: execution method of primal heuristic.
fn heur_exec_clique(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    let nbinvars = api::get_n_bin_vars(scip);
    if nbinvars < 2 {
        return Ok(());
    }

    // Check for necessary information to apply this heuristic.
    if crate::scip_var::get_n_cliques(scip) == 0 {
        return Ok(());
    }

    let mut lowerbound = api::get_lowerbound(scip);

    let d = hd(scip, heur);
    let h = &scip.heurs[heur];

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut nstallnodes = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Reward clique heuristic if it succeeded often.
    nstallnodes = (nstallnodes as f64 * 3.0 * (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0)) as i64;
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

    let oldnpscands = api::get_n_pseudo_branch_cands(scip);
    let mut onefixvars: Vec<VarId> = Vec::new();
    let mut onefixvals: Vec<bool> = Vec::new();

    // Disable conflict analysis, because we can it better than SCIP itself,
    // cause we have more information.
    let enabledconflicts = crate::paramset::paramset_get_bool(scip, "conflict/enable")?;
    if !crate::paramset::paramset_is_fixed(scip, "conflict/enable")? {
        paramset_set_bool(scip, "conflict/enable", false)?;
    }

    'terminate: {
        let solvelp = api::has_current_node_lp(scip);

        if !crate::tree::tree_is_focus_node_lp_constructed(scip) && solvelp {
            let cutoff = api::construct_lp(scip)?;

            // Manually cut off the node if the LP construction detected
            // infeasibility (heuristics cannot return such a result).
            if cutoff {
                let node = crate::tree::tree_get_current_node(scip);
                api::cutoff_node(scip, node)?;
                break 'terminate;
            }

            api::flush_lp(scip)?;
        }

        *result = ScipResultCode::DidNotFind;

        // Start probing.
        scip_probing::start_probing(scip)?;

        // Apply fixings due to clique information.
        let mut cutoff = apply_clique_fixings(scip, heur, enabledconflicts, &mut onefixvars, &mut onefixvals)?;

        if cutoff || crate::solve::solve_is_stopped(scip, false) {
            break 'terminate;
        }

        // Check that we had enough fixings.
        let mut npscands = api::get_n_pseudo_branch_cands(scip);
        let minintfixingrate = hd(scip, heur).minintfixingrate;

        if npscands as f64 > oldnpscands as f64 * (1.0 - minintfixingrate) {
            if hd(scip, heur).uselockfixings && npscands as f64 <= 2.0 * oldnpscands as f64 * (1.0 - minintfixingrate) {
                let allrowsfulfilled;
                (cutoff, allrowsfulfilled) = crate::heur_locks::apply_lock_fixings(scip, NONE)?;

                if cutoff || crate::solve::solve_is_stopped(scip, false) {
                    break 'terminate;
                }

                npscands = api::get_n_pseudo_branch_cands(scip);

                if !allrowsfulfilled && npscands as f64 > oldnpscands as f64 * (1.0 - minintfixingrate) {
                    break 'terminate;
                }
            } else {
                break 'terminate;
            }
        }

        // Probing LP solving.
        let mut lpstatus = LpSolStat::Error;
        let mut lperror = false;

        // Solve lp only if the problem is still feasible.
        if solvelp {
            // Solve LP; errors in the LP solver should not kill the overall
            // solving process, if the LP is just needed for a heuristic.
            match scip_probing::solve_probing_lp(scip, -1) {
                Ok((e, _)) => lperror = e,
                Err(retstat) => log::warn!("Error while solving LP in clique heuristic; LP solve terminated with code <{retstat:?}>"),
            }

            lpstatus = api::get_lp_solstat(scip);
        }

        // Check if this is a feasible solution.
        if lpstatus == LpSolStat::Optimal && !lperror {
            lowerbound = api::get_lp_objval(scip);

            // Create a solution from the current LP solution.
            let sol = api::create_sol(scip, heur)?;
            crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

            let success = api::round_sol(scip, sol)?;
            if success {
                // Check solution for feasibility, and add it to solution store
                // if possible; neither integrality nor feasibility of LP rows
                // has to be checked, because they are guaranteed by the
                // heuristic at this stage.
                let stored = api::try_sol(scip, sol, false, false, true, false, false)?;
                if stored {
                    *result = ScipResultCode::FoundSol;
                }

                api::free_sol(scip, sol)?;

                // We found a solution, so we are done.
                break 'terminate;
            }

            api::free_sol(scip, sol)?;
        }

        // Create conflict.
        if enabledconflicts && crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) && (lpstatus == LpSolStat::Infeasible || lpstatus == LpSolStat::ObjLimit) {
            let mut conflictcons = create_conflict(scip, &mut onefixvars, &onefixvals)?;
            let focusnode = crate::tree::tree_get_focus_node(scip);
            crate::scip_cons::add_conflict(scip, focusnode, &mut conflictcons, NONE, ConfType::InfeasLp, false)?;
            break 'terminate;
        }

        // Start subscip solving: no solution has been found yet and the
        // subproblem is still feasible, so fix all other variables by subscip
        // if necessary.
        if !lperror {
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

            crate::copy::copy_cons_compression(scip, &mut subscip, &mut maps, "_clique", &[], &[], false, false)?;

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

            // Speed up sub-SCIP by not checking dual LP feasibility.
            paramset_set_bool(&mut subscip, "lp/checkdualfeas", false)?;

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

            // If there is already a solution, add an objective cutoff.
            if api::get_n_sols(scip) > 0 {
                let minimprove = hd(scip, heur).minimprove;
                let upperbound = api::get_upperbound(scip) - scip.set.sumepsilon();

                let cutoffbound = if !scip.set.is_infinity(-1.0 * lowerbound) {
                    (1.0 - minimprove) * api::get_upperbound(scip) + minimprove * lowerbound
                } else if api::get_upperbound(scip) >= 0.0 {
                    (1.0 - minimprove) * api::get_upperbound(scip)
                } else {
                    (1.0 + minimprove) * api::get_upperbound(scip)
                };
                let cutoffbound = crate::def::min(upperbound, cutoffbound);
                api::set_objlimit(&mut subscip, cutoffbound)?;
            }

            // Solve the subproblem.
            crate::scip_solve::presolve_prob(&mut subscip)?;

            // After presolving, we should have at least reached a certain
            // fixing rate over ALL variables (including continuous) to ensure
            // that not only continuous variables are left.
            if (nvars as i32 - api::get_n_vars(&subscip)) as f64 / nvars as f64 >= hd(scip, heur).minmipfixingrate {
                crate::scip_solve::solve(&mut subscip)?;

                // Check, whether a solution was found; due to numerics, it
                // might happen that not all solutions are feasible -> try all
                // solutions until one was accepted.
                let (success, _) = crate::copy::translate_sub_sols(scip, &subscip, heur, &subvars)?;
                if success {
                    *result = ScipResultCode::FoundSol;
                }

                // If subscip was infeasible, add a conflict.
                if api::get_status(&subscip) == Status::Infeasible {
                    let mut conflictcons = create_conflict(scip, &mut onefixvars, &onefixvals)?;
                    let focusnode = crate::tree::tree_get_focus_node(scip);
                    crate::scip_cons::add_cons_node(scip, focusnode, conflictcons, NONE)?;
                    crate::scip_cons::release_cons(scip, &mut conflictcons)?;
                }
            }
        }
    }

    // Reset the conflict analysis.
    if !crate::paramset::paramset_is_fixed(scip, "conflict/enable")? {
        paramset_set_bool(scip, "conflict/enable", enabledconflicts)?;
    }

    // End probing.
    if api::in_probing(scip) {
        scip_probing::end_probing(scip)?;
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurClique`: creates the clique primal heuristic and
/// includes it in SCIP.
pub fn include_heur_clique(scip: &mut Scip) -> ScipResult {
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
        heur_exec_clique,
        Some(Box::new(HeurData {
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            nodesofs: DEFAULT_NODESOFS,
            usednodes: 0,
            minintfixingrate: DEFAULT_MININTFIXINGRATE,
            minmipfixingrate: DEFAULT_MINMIPFIXINGRATE,
            minimprove: DEFAULT_MINIMPROVE,
            nodesquot: DEFAULT_NODESQUOT,
            maxproprounds: DEFAULT_MAXPROPROUNDS,
            maxbacktracks: DEFAULT_MAXBACKTRACKS,
            copycuts: DEFAULT_COPYCUTS,
            uselockfixings: DEFAULT_USELOCKFIXINGS,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_clique);
    h.heurinit = Some(heur_init_clique);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add clique primal heuristic parameters.
    paramset_add_real(
        scip,
        "heuristics/clique/minintfixingrate",
        "minimum percentage of integer variables that have to be fixable",
        t!(f64, minintfixingrate),
        false,
        DEFAULT_MININTFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/clique/minmipfixingrate",
        "minimum percentage of fixed variables in the sub-MIP",
        t!(f64, minmipfixingrate),
        false,
        DEFAULT_MINMIPFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/clique/maxnodes",
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
        "heuristics/clique/nodesofs",
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
        "heuristics/clique/minnodes",
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
        "heuristics/clique/nodesquot",
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
        "heuristics/clique/minimprove",
        "factor by which clique heuristic should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/clique/maxproprounds",
        "maximum number of propagation rounds during probing (-1 infinity)",
        t!(i32, maxproprounds),
        true,
        DEFAULT_MAXPROPROUNDS,
        -1,
        i32::MAX / 4,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/clique/copycuts",
        "should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/clique/uselockfixings",
        "should more variables be fixed based on variable locks if the fixing rate was not reached?",
        t!(bool, uselockfixings),
        true,
        DEFAULT_USELOCKFIXINGS,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/clique/maxbacktracks",
        "maximum number of backtracks during the fixing process",
        t!(i32, maxbacktracks),
        true,
        DEFAULT_MAXBACKTRACKS,
        -1,
        i32::MAX / 4,
        None,
    )?;

    Ok(())
}
