//! Port of `heur_vbounds.c`: LNS heuristic that fixes the integer variables
//! along the variable bound graph (in topological order) to one of their
//! bounds, then solves the LP and a sub-MIP of what is left.

use std::any::Any;

use crate::{
    api,
    copy::CopyMaps,
    heur::{HEURTIMING_BEFORENODE, HeurTiming},
    paramset::{ParamSetting, paramset_set_bool, paramset_set_int, paramset_set_longint},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_probing,
    types::{BoundType, HeurId, LpSolStat, ScipResultCode, VarId},
    var::NONE,
};

const VBOUNDVARIANT_NOOBJ: u32 = 0x001;
const VBOUNDVARIANT_BESTBOUND: u32 = 0x002;
const VBOUNDVARIANT_WORSTBOUND: u32 = 0x004;

const HEUR_NAME: &str = "vbounds";
const HEUR_DESC: &str = "LNS heuristic uses the variable lower and upper bounds to determine the search neighborhood";
const HEUR_DISPCHAR: u8 = b'p';
const HEUR_PRIORITY: i32 = 2500;
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
/// Factor by which vbounds heuristic should at least improve the
/// incumbent.
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
/// Which variants of the vbounds heuristic that try to stay feasible should
/// be called?
const DEFAULT_FEASVARIANT: u32 = VBOUNDVARIANT_BESTBOUND | VBOUNDVARIANT_WORSTBOUND;
/// Which tightening variants of the vbounds heuristic should be called?
const DEFAULT_TIGHTENVARIANT: u32 = VBOUNDVARIANT_NOOBJ | VBOUNDVARIANT_BESTBOUND | VBOUNDVARIANT_WORSTBOUND;

/// `SCIP_HeurData`.
struct HeurData {
    vbvars: Vec<VarId>,
    vbbounds: Vec<BoundType>,
    maxnodes: i64,
    minnodes: i64,
    nodesofs: i64,
    usednodes: i64,
    minintfixingrate: f64,
    minmipfixingrate: f64,
    minimprove: f64,
    nodesquot: f64,
    cutoffbound: f64,
    maxproprounds: i32,
    maxbacktracks: i32,
    feasvariant: i32,
    tightenvariant: i32,
    initialized: bool,
    applicable: bool,
    copycuts: bool,
    uselockfixings: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut().unwrap()
}

fn get_lb_index(idx: i32) -> i32 {
    2 * idx
}

fn get_ub_index(idx: i32) -> i32 {
    2 * idx + 1
}

fn get_var_index(idx: i32) -> usize {
    (idx / 2) as usize
}

fn get_boundtype(idx: i32) -> BoundType {
    if idx % 2 == 0 { BoundType::Lower } else { BoundType::Upper }
}

fn is_index_lowerbound(idx: i32) -> bool {
    idx % 2 == 0
}

fn get_other_bound_index(idx: i32) -> i32 {
    if idx % 2 == 0 { idx + 1 } else { idx - 1 }
}

/// `heurdataReset`: resets the heuristic data structure.
fn heurdata_reset(d: &mut HeurData) {
    d.vbvars = Vec::new();
    d.vbbounds = Vec::new();
    d.initialized = false;
    d.applicable = false;
}

/// `SCIPvarGetCliques(var, value)`.
fn var_cliques(scip: &Scip, var: VarId, value: bool) -> Vec<usize> {
    match scip.vars[var].cliquelist.as_ref() {
        Some(l) => l.cliques[value as usize].0[..l.ncliques[value as usize] as usize].to_vec(),
        None => Vec::new(),
    }
}

/// `dfs`: performs depth-first-search in the implicitly given directed graph
/// from the given start index.
#[allow(clippy::too_many_arguments)]
fn dfs(
    scip: &Scip,
    startnode: i32,
    visited: &mut [bool],
    dfsstack: &mut [i32],
    stacknextedge: &mut [i32],
    stacknextcliquevar: &mut [i32],
    cliqueexit: &mut [i32],
    dfsnodes: &mut Vec<i32>,
) {
    let vars = api::get_vars(scip);

    // Put start node on the stack.
    dfsstack[0] = startnode;
    stacknextcliquevar[0] = 0;
    stacknextedge[0] = 0;
    let mut maxstacksize = 1;
    let mut stacksize = 1usize;
    let mut idx: i32 = -1;

    // We run until no more bounds indices are on the stack.
    'outer: while stacksize > 0 {
        // Get next node from stack.
        let curridx = dfsstack[stacksize - 1];

        // Mark current node as visited.
        visited[curridx as usize] = true;
        let mut found = false;

        let startvar = vars[get_var_index(curridx)];
        let lower = is_index_lowerbound(curridx);

        if stacknextedge[stacksize - 1] >= 0 {
            // Go over edges corresponding to varbounds.
            let vb = if lower { scip.vars[startvar].vlbs.as_ref() } else { scip.vars[startvar].vubs.as_ref() };
            let (vbvars, coefs): (&[VarId], &[f64]) = match vb {
                Some(vb) => (&vb.vars.0[..vb.len as usize], &vb.coefs.0[..vb.len as usize]),
                None => (&[], &[]),
            };
            let nvbvars = vbvars.len() as i32;

            // Iterate over all vbounds for the given bound.
            let mut i = stacknextedge[stacksize - 1];
            while i < nvbvars {
                let iu = i as usize;
                if !crate::var::var_is_active(scip, vbvars[iu]) {
                    i += 1;
                    continue;
                }

                let probindex = scip.vars[vbvars[iu]].probindex;
                idx = if scip.set.is_positive(coefs[iu]) == lower {
                    get_lb_index(probindex)
                } else {
                    get_ub_index(probindex)
                };

                // Break when the first unvisited node is reached.
                if !visited[idx as usize] {
                    break;
                }
                i += 1;
            }

            // We stopped because we found an unhandled node and not because we
            // reached the end of the list.
            if i < nvbvars {
                // Put the adjacent node onto the stack.
                dfsstack[stacksize] = idx;
                stacknextedge[stacksize] = 0;
                stacknextcliquevar[stacksize] = 0;
                stacknextedge[stacksize - 1] = i + 1;
                stacksize += 1;

                // Restart while loop, get next index from stack.
                continue 'outer;
            }
        }

        stacknextedge[stacksize - 1] = -1;

        // Treat cliques.
        if crate::var::var_is_binary(scip, startvar) {
            let cliques = var_cliques(scip, startvar, !lower);
            let ncliques = cliques.len() as i32;

            // Iterate over all not yet handled cliques and search for an
            // unvisited node.
            let mut j = -stacknextedge[stacksize - 1] - 1;
            while j < ncliques {
                let clique = cliques[j as usize];
                let cliqueindex = scip.cliques[clique].index as usize;

                // The first time we evaluate this clique for the current node.
                if stacknextcliquevar[stacksize - 1] == 0 {
                    if cliqueexit[cliqueindex] > 0 {
                        if !visited[(cliqueexit[cliqueindex] - 1) as usize] && cliqueexit[cliqueindex] - 1 != curridx {
                            stacknextedge[stacksize - 1] = -j - 2;
                            stacknextcliquevar[stacksize - 1] = 0;
                            idx = cliqueexit[cliqueindex] - 1;
                            cliqueexit[cliqueindex] = -1;
                            found = true;
                        } else {
                            j += 1;
                            continue;
                        }
                    } else if cliqueexit[cliqueindex] == 0 {
                        cliqueexit[cliqueindex] = get_other_bound_index(curridx) + 1;
                    } else {
                        j += 1;
                        continue;
                    }
                }

                if !found {
                    let c = &scip.cliques[clique];
                    let ncliquevars = c.nvars as usize;
                    for i in 0..ncliquevars {
                        let cv = c.vars[i];
                        if cv == startvar {
                            continue;
                        }

                        let probindex = scip.vars[cv].probindex;
                        idx = if c.values[i] { get_lb_index(probindex) } else { get_ub_index(probindex) };

                        // Break when the first unvisited node is reached.
                        if idx >= 0 && !visited[idx as usize] {
                            if i < ncliquevars - 1 {
                                stacknextedge[stacksize - 1] = -j - 1;
                                stacknextcliquevar[stacksize - 1] = i as i32 + 1;
                            } else {
                                stacknextedge[stacksize - 1] = -j - 2;
                                stacknextcliquevar[stacksize - 1] = 0;
                            }
                            found = true;
                            break;
                        }
                    }
                }

                if found {
                    // Put the adjacent node onto the stack.
                    dfsstack[stacksize] = idx;
                    stacknextedge[stacksize] = 0;
                    stacknextcliquevar[stacksize] = 0;
                    stacksize += 1;
                    break;
                }
                j += 1;
            }

            // Restart while loop, get next index from stack.
            if found {
                continue 'outer;
            }
        }

        maxstacksize = if maxstacksize >= stacksize { maxstacksize } else { stacksize };

        // The current node was completely handled, remove it from the stack.
        stacksize -= 1;

        if maxstacksize > 1 && crate::var::var_is_integral(scip, startvar) {
            // Store node in the sorted nodes array.
            dfsnodes.push(curridx);
        } else {
            visited[curridx as usize] = false;
        }
    }
}

/// `topologicalSort`: sorts the bounds of variables topologically.
fn topological_sort(scip: &Scip) -> Vec<i32> {
    let nbounds = 2 * api::get_n_vars(scip) as usize;
    let mut dfsstack = vec![0i32; nbounds];
    let mut stacknextedge = vec![0i32; nbounds];
    let mut stacknextcliquevar = vec![0i32; nbounds];
    let mut cliqueexit = vec![0i32; crate::scip_var::get_n_cliques(scip) as usize];
    let mut visited = vec![false; nbounds];
    let mut vbvars = Vec::new();

    // While there are unvisited nodes, run dfs on the inverse graph starting
    // from one of these nodes; the dfs orders are stored in the topoorder
    // array, later dfs calls are just appended after the stacks of previous
    // dfs calls, which gives us a topological order.
    for i in 0..nbounds {
        if !visited[i] {
            dfs(
                scip,
                i as i32,
                &mut visited,
                &mut dfsstack,
                &mut stacknextedge,
                &mut stacknextcliquevar,
                &mut cliqueexit,
                &mut vbvars,
            );
        }
    }

    vbvars
}

/// The objective cutoff the heuristic's sub-SCIPs use (`cutoffbound` in
/// `initializeCandsLists` and `setupAndSolveSubscip`).
fn heur_cutoffbound(scip: &Scip, minimprove: f64, lowerbound: f64) -> f64 {
    let upperbound = api::get_upperbound(scip) - scip.set.sumepsilon();
    let cutoffbound = if !scip.set.is_infinity(-lowerbound) {
        (1.0 - minimprove) * api::get_upperbound(scip) + minimprove * lowerbound
    } else if api::get_upperbound(scip) >= 0.0 {
        (1.0 - minimprove) * api::get_upperbound(scip)
    } else {
        (1.0 + minimprove) * api::get_upperbound(scip)
    };
    crate::def::min(upperbound, cutoffbound)
}

/// `initializeCandsLists`: initializes the candidate lists.
fn initialize_cands_lists(scip: &mut Scip, heur: HeurId) -> ScipResult {
    let nvars = api::get_n_int_vars(scip) + api::get_n_bin_vars(scip) + api::get_n_impl_vars(scip);

    // Initialize data.
    {
        let d = hdm(scip, heur);
        d.usednodes = 0;
        d.initialized = true;
    }

    if nvars == 0 {
        return Ok(());
    }

    // Create the topological sorted variable array with respect to the
    // variable bounds.
    let vbs = topological_sort(scip);
    let nvbs = vbs.len();

    // Check if the candidate list contains enough candidates.
    if nvbs > 0 && nvbs as f64 >= 0.1 * hd(scip, heur).minintfixingrate * nvars as f64 {
        let vars = api::get_vars(scip).to_vec();
        let mut vbvars = Vec::with_capacity(nvbs);
        let mut vbbounds = Vec::with_capacity(nvbs);

        // Capture variable candidate list.
        for &v in &vbs {
            let var = vars[get_var_index(v)];
            vbvars.push(var);
            vbbounds.push(get_boundtype(v));
            crate::var::var_capture(scip, var);
        }

        let d = hdm(scip, heur);
        d.vbvars = vbvars;
        d.vbbounds = vbbounds;
        d.applicable = true;
    }

    // If there is already a solution, add an objective cutoff.
    let cutoffbound = if api::get_n_sols(scip) > 0 {
        heur_cutoffbound(scip, hd(scip, heur).minimprove, api::get_lowerbound(scip))
    } else {
        scip.set.infinity()
    };
    hdm(scip, heur).cutoffbound = cutoffbound;

    Ok(())
}

/// `applyVboundsFixings`: applies variable bound fixing during probing;
/// returns `infeasible`.
#[allow(clippy::too_many_arguments)]
fn apply_vbounds_fixings(
    scip: &mut Scip,
    heur: HeurId,
    vars: &[VarId],
    tighten: bool,
    obj: i32,
    allobj1: &mut bool,
    allobj2: &mut bool,
    backtracked: &mut bool,
) -> ScipResult<bool> {
    let mut infeasible = false;
    let mut nbacktracks = 0;
    let (maxproprounds, maxbacktracks) = (hd(scip, heur).maxproprounds, hd(scip, heur).maxbacktracks);
    let vbbounds = hd(scip, heur).vbbounds.clone();
    let mut lastvar;
    let mut lastfixval;
    let mut lastfixedlb;

    // Loop over variables in topological order.
    let mut v = 0;
    while v < vars.len() && !infeasible {
        let var = vars[v];
        let bound = vbbounds[v];

        // Only check integer or binary variables.
        if !crate::var::var_is_integral(scip, var) {
            v += 1;
            continue;
        }

        // Skip variables which are already fixed.
        if scip.vars[var].locdom.lb + 0.5 > scip.vars[var].locdom.ub {
            v += 1;
            continue;
        }

        // There are two cases for tighten: 1) tighten == TRUE: we go through
        // the list of variables and fix variables to force propagation; this
        // is be obtained by fixing the variable to the other bound (which
        // means that the current bound is changed and therefore, propagation
        // is triggered); 2) tighten == FALSE: we go through the list of
        // variables and fix variables to the current bound (and therefore,
        // propagation is not triggered).
        let fixtolower = tighten == (bound == BoundType::Upper);

        // If we want to take into account the objective function
        // coefficients, we only perform a fixing if the variable would take
        // its preferred value.
        if (scip.vars[var].obj >= 0.0) != fixtolower {
            if obj == 1 {
                v += 1;
                continue;
            } else {
                *allobj1 = false;
            }
        }

        // If we want to take into account the objective function
        // coefficients but reverted, we only perform a fixing if the
        // variable would take its non-preferred value.
        if (scip.vars[var].obj >= 0.0) == fixtolower {
            if obj == 2 {
                v += 1;
                continue;
            } else {
                *allobj2 = false;
            }
        }

        lastvar = var;

        // Fix the variable to its bound.
        if fixtolower {
            // We cannot fix to infinite bounds.
            if scip.set.is_infinity(-scip.vars[var].locdom.lb) {
                v += 1;
                continue;
            }

            // Only open a new probing node if we will not exceed the maximal
            // tree depth.
            if crate::def::MAXTREEDEPTH > api::get_depth(scip) {
                scip_probing::new_probing_node(scip)?;
            }

            // Fix variable to lower bound.
            let lb = scip.vars[var].locdom.lb;
            scip_probing::fix_var_probing(scip, var, lb)?;
            lastfixedlb = true;
            lastfixval = scip.vars[var].locdom.lb;
        } else {
            // We cannot fix to infinite bounds.
            if scip.set.is_infinity(scip.vars[var].locdom.ub) {
                v += 1;
                continue;
            }

            // Only open a new probing node if we will not exceed the maximal
            // tree depth.
            if crate::def::MAXTREEDEPTH > api::get_depth(scip) {
                scip_probing::new_probing_node(scip)?;
            }

            // Fix variable to upper bound.
            let ub = scip.vars[var].locdom.ub;
            scip_probing::fix_var_probing(scip, var, ub)?;
            lastfixedlb = false;
            lastfixval = scip.vars[var].locdom.ub;
        }

        // Check if problem is already infeasible.
        infeasible = scip_probing::propagate_probing(scip, maxproprounds)?.0;

        // Probing detected infeasibility: backtrack.
        if infeasible {
            let depth = scip_probing::get_probing_depth(scip);
            scip_probing::backtrack_probing(scip, depth - 1)?;

            nbacktracks += 1;
            infeasible = false;

            // Increase the lower bound of the variable which caused the
            // infeasibility.
            if lastfixedlb && lastfixval + 0.5 < scip.vars[lastvar].locdom.ub {
                if lastfixval + 0.5 > scip.vars[lastvar].locdom.lb {
                    scip_probing::chg_var_lb_probing(scip, lastvar, lastfixval + 1.0)?;
                }
            } else if !lastfixedlb && lastfixval - 0.5 > scip.vars[lastvar].locdom.lb {
                if lastfixval - 0.5 < scip.vars[lastvar].locdom.ub {
                    scip_probing::chg_var_ub_probing(scip, lastvar, lastfixval - 1.0)?;
                }
            }
            // Because of the limited number of propagation rounds, it may
            // happen that conflict analysis finds a valid global bound for the
            // last fixed variable that conflicts with applying the reverse
            // bound change after backtracking; in that case, we ran into a
            // deadend and stop.
            else {
                infeasible = true;
            }

            if !infeasible {
                // Propagate fixings.
                infeasible = scip_probing::propagate_probing(scip, maxproprounds)?.0;
            }

            if infeasible || nbacktracks > maxbacktracks {
                break;
            }
        }
        v += 1;
    }

    *backtracked = nbacktracks > 0;

    Ok(infeasible)
}

/// `setupAndSolveSubscip`: copies the problem to the sub-SCIP, solves it,
/// and adds its solutions.
fn setup_and_solve_subscip(scip: &mut Scip, subscip: &mut Scip, heur: HeurId, vars: &[VarId], nstallnodes: i64, lowerbound: f64, result: &mut ScipResultCode) -> ScipResult {
    let nvars = vars.len();

    // Create the variable mapping hash map.
    let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

    crate::copy::copy_cons_compression(scip, subscip, &mut maps, "_vbounds", &[], &[], false, false)?;

    if hd(scip, heur).copycuts {
        // Copies all active cuts from cutpool of sourcescip to linear
        // constraints in targetscip.
        crate::copy::copy_cuts_public(scip, subscip, &mut maps, false)?;
    }

    let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable statistic timing inside sub SCIP and output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    // Set limits for the subproblem.
    crate::copy::copy_limits(scip, subscip)?;
    paramset_set_longint(subscip, "limits/stallnodes", nstallnodes)?;
    paramset_set_longint(subscip, "limits/nodes", hd(scip, heur).maxnodes)?;

    // Speed up sub-SCIP by not checking dual LP feasibility.
    paramset_set_bool(subscip, "lp/checkdualfeas", false)?;

    // Forbid call of heuristics and separators solving sub-CIPs.
    crate::paramset::set_subscips_off(subscip, true)?;

    // Disable cutting plane separation.
    crate::paramset::set_separating(subscip, ParamSetting::Off, true)?;

    // Disable expensive presolving.
    crate::paramset::set_presolving(subscip, ParamSetting::Fast, true)?;

    // Use inference branching.
    if crate::set::set_find_branchrule(subscip, "inference") != NONE && !crate::paramset::paramset_is_fixed(subscip, "branching/inference/priority")? {
        paramset_set_int(subscip, "branching/inference/priority", i32::MAX / 4)?;
    }

    // Set a cutoff bound.
    if api::get_n_sols(scip) > 0 {
        let cutoffbound = heur_cutoffbound(scip, hd(scip, heur).minimprove, lowerbound);
        hdm(scip, heur).cutoffbound = cutoffbound;
    }

    let cutoffbound = hd(scip, heur).cutoffbound;
    if !scip.set.is_infinity(cutoffbound) {
        api::set_objlimit(subscip, cutoffbound)?;
    }

    // Solve the subproblem.
    crate::scip_solve::presolve_prob(subscip)?;

    // After presolving, we should have at least reached a certain fixing rate
    // over ALL variables (including continuous) to ensure that not only
    // continuous variables are left.
    if (nvars as i32 - api::get_n_vars(subscip)) as f64 / nvars as f64 >= hd(scip, heur).minmipfixingrate {
        crate::scip_solve::solve(subscip)?;

        // Check, whether a solution was found; due to numerics, it might
        // happen that not all solutions are feasible -> try all solutions
        // until one was accepted.
        let (wasfeas, _) = crate::copy::translate_sub_sols(scip, subscip, heur, &subvars)?;
        if wasfeas {
            *result = ScipResultCode::FoundSol;
        }
    }
    Ok(())
}

/// `applyVbounds`: main procedure of the vbounds heuristic; returns
/// `(skipobj1, skipobj2)`.
fn apply_vbounds(scip: &mut Scip, heur: HeurId, tighten: bool, obj: i32, result: &mut ScipResultCode) -> ScipResult<(bool, bool)> {
    let vbvars = hd(scip, heur).vbvars.clone();
    let nvbvars = vbvars.len();
    let mut skipobj1 = false;
    let mut skipobj2 = false;
    let mut allobj1 = true;
    let mut allobj2 = true;
    let mut backtracked = true;

    if (nvbvars as f64) < api::get_n_vars(scip) as f64 * hd(scip, heur).minintfixingrate {
        return Ok((skipobj1, skipobj2));
    }

    if *result == ScipResultCode::DidNotRun {
        *result = ScipResultCode::DidNotFind;
    }

    let mut lowerbound = api::get_lowerbound(scip);

    let oldnpscands = api::get_n_pseudo_branch_cands(scip);

    let d = hd(scip, heur);
    let h = &scip.heurs[heur];

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut nstallnodes = (d.nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Reward variable bounds heuristic if it succeeded often.
    nstallnodes = (nstallnodes as f64 * 3.0 * (h.nbestsolsfound as f64 + 1.0) / (h.ncalls as f64 + 1.0)) as i64;
    // Count the setup costs for the sub-MIP as 100 nodes.
    nstallnodes -= 100 * h.ncalls;
    nstallnodes += d.nodesofs;

    // Determine the node limit for the current process.
    nstallnodes -= d.usednodes;
    nstallnodes = if nstallnodes <= d.maxnodes { nstallnodes } else { d.maxnodes };

    // Check whether we have enough nodes left to call subproblem solving.
    if nstallnodes < d.minnodes {
        return Ok((skipobj1, skipobj2));
    }

    if crate::solve::solve_is_stopped(scip, false) {
        return Ok((skipobj1, skipobj2));
    }

    'terminate: {
        // Check whether the LP should be solved at the current node in the
        // tree to determine whether the heuristic is allowed to solve an LP.
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

        // Get variable data of original problem.
        let vars = api::get_vars(scip).to_vec();

        // Start probing.
        scip_probing::start_probing(scip)?;

        // Apply the variable fixings.
        let mut cutoff = apply_vbounds_fixings(scip, heur, &vbvars, tighten, obj, &mut allobj1, &mut allobj2, &mut backtracked)?;

        skipobj1 = allobj1;
        skipobj2 = allobj2;

        if cutoff || crate::solve::solve_is_stopped(scip, false) {
            break 'terminate;
        }

        // Check that we had enough fixings.
        let mut npscands = api::get_n_pseudo_branch_cands(scip);
        let minintfixingrate = hd(scip, heur).minintfixingrate;

        // Check fixing rate.
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
                Err(retstat) => log::warn!("Error while solving LP in vbound heuristic; LP solve terminated with code <{retstat:?}>"),
            }

            lpstatus = api::get_lp_solstat(scip);
        }

        // Check if this is a feasible solution.
        if lpstatus == LpSolStat::Optimal && !lperror {
            lowerbound = api::get_lp_objval(scip);

            // Copy the current LP solution to the working solution.
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

        // If no solution has been found --> fix all other variables by
        // subscip if necessary.
        if !lperror && lpstatus != LpSolStat::Infeasible && lpstatus != LpSolStat::ObjLimit {
            // Check whether there is enough time and memory left.
            if !crate::copy::check_copy_limits(scip)? {
                break 'terminate;
            }

            // Create subproblem.
            let mut subscip = Scip::create()?;

            setup_and_solve_subscip(scip, &mut subscip, heur, &vars, nstallnodes, lowerbound, result)?;
        }
    }

    // Exit probing mode.
    if api::in_probing(scip) {
        scip_probing::end_probing(scip)?;
    }

    Ok((skipobj1, skipobj2))
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyVbounds`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_vbounds(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of heuristic.
    include_heur_vbounds(scip)
}

/// `heurExitsolVbounds`: solving process deinitialization method of primal
/// heuristic (called before branch and bound process data is freed).
fn heur_exitsol_vbounds(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Release all variables.
    let vbvars = std::mem::take(&mut hdm(scip, heur).vbvars);
    for mut var in vbvars {
        api::release_var(scip, &mut var)?;
    }

    // Reset heuristic data structure.
    heurdata_reset(hdm(scip, heur));
    Ok(())
}

/// `heurExecVbounds`: execution method of primal heuristic.
fn heur_exec_vbounds(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if api::get_n_pseudo_branch_cands(scip) == 0 {
        return Ok(());
    }

    if !hd(scip, heur).initialized {
        initialize_cands_lists(scip, heur)?;
    }

    if !hd(scip, heur).applicable {
        return Ok(());
    }

    let feasvariant = hd(scip, heur).feasvariant as u32;
    let tightenvariant = hd(scip, heur).tightenvariant as u32;

    // Try variable bounds.
    let (mut skipobj1, mut skipobj2) = (false, false);
    if feasvariant & VBOUNDVARIANT_NOOBJ != 0 {
        (skipobj1, skipobj2) = apply_vbounds(scip, heur, false, 0, result)?;
    }
    if !skipobj1 && feasvariant & VBOUNDVARIANT_BESTBOUND != 0 {
        apply_vbounds(scip, heur, false, 1, result)?;
    }
    if !skipobj2 && feasvariant & VBOUNDVARIANT_WORSTBOUND != 0 {
        apply_vbounds(scip, heur, false, 2, result)?;
    }

    let (mut skipobj1, mut skipobj2) = (false, false);
    if tightenvariant & VBOUNDVARIANT_NOOBJ != 0 {
        (skipobj1, skipobj2) = apply_vbounds(scip, heur, true, 0, result)?;
    }
    if !skipobj1 && tightenvariant & VBOUNDVARIANT_BESTBOUND != 0 {
        apply_vbounds(scip, heur, true, 1, result)?;
    }
    if !skipobj2 && tightenvariant & VBOUNDVARIANT_WORSTBOUND != 0 {
        apply_vbounds(scip, heur, true, 2, result)?;
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurVbounds`: creates the vbounds primal heuristic and
/// includes it in SCIP.
pub fn include_heur_vbounds(scip: &mut Scip) -> ScipResult {
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
        heur_exec_vbounds,
        Some(Box::new(HeurData {
            vbvars: Vec::new(),
            vbbounds: Vec::new(),
            maxnodes: DEFAULT_MAXNODES,
            minnodes: DEFAULT_MINNODES,
            nodesofs: DEFAULT_NODESOFS,
            usednodes: 0,
            minintfixingrate: DEFAULT_MININTFIXINGRATE,
            minmipfixingrate: DEFAULT_MINMIPFIXINGRATE,
            minimprove: DEFAULT_MINIMPROVE,
            nodesquot: DEFAULT_NODESQUOT,
            cutoffbound: 0.0,
            maxproprounds: DEFAULT_MAXPROPROUNDS,
            maxbacktracks: DEFAULT_MAXBACKTRACKS,
            feasvariant: DEFAULT_FEASVARIANT as i32,
            tightenvariant: DEFAULT_TIGHTENVARIANT as i32,
            initialized: false,
            applicable: false,
            copycuts: DEFAULT_COPYCUTS,
            uselockfixings: DEFAULT_USELOCKFIXINGS,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_vbounds);
    h.heurexitsol = Some(heur_exitsol_vbounds);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add variable bounds primal heuristic parameters.
    paramset_add_real(
        scip,
        "heuristics/vbounds/minintfixingrate",
        "minimum percentage of integer variables that have to be fixed",
        t!(f64, minintfixingrate),
        false,
        DEFAULT_MININTFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/vbounds/minmipfixingrate",
        "minimum percentage of variables that have to be fixed within sub-SCIP (integer and continuous)",
        t!(f64, minmipfixingrate),
        false,
        DEFAULT_MINMIPFIXINGRATE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/vbounds/maxnodes",
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
        "heuristics/vbounds/nodesofs",
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
        "heuristics/vbounds/minnodes",
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
        "heuristics/vbounds/nodesquot",
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
        "heuristics/vbounds/minimprove",
        "factor by which vbounds heuristic should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/vbounds/maxproprounds",
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
        "heuristics/vbounds/copycuts",
        "should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/vbounds/uselockfixings",
        "should more variables be fixed based on variable locks if the fixing rate was not reached?",
        t!(bool, uselockfixings),
        true,
        DEFAULT_USELOCKFIXINGS,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/vbounds/maxbacktracks",
        "maximum number of backtracks during the fixing process",
        t!(i32, maxbacktracks),
        true,
        DEFAULT_MAXBACKTRACKS,
        -1,
        i32::MAX / 4,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/vbounds/feasvariant",
        "which variants of the vbounds heuristic that try to stay feasible should be called? (0: off, 1: w/o looking at obj, 2: only fix to best bound, 4: only fix to worst bound",
        t!(i32, feasvariant),
        true,
        DEFAULT_FEASVARIANT as i32,
        0,
        7,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/vbounds/tightenvariant",
        "which tightening variants of the vbounds heuristic should be called? (0: off, 1: w/o looking at obj, 2: only fix to best bound, 4: only fix to worst bound",
        t!(i32, tightenvariant),
        true,
        DEFAULT_TIGHTENVARIANT as i32,
        0,
        7,
        None,
    )?;

    Ok(())
}
