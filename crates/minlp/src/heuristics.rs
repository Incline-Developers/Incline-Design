//! Port of `heuristics.c`: methods commonly used by primal heuristics, here
//! the generic diving algorithm (`SCIPperformGenericDivingAlgorithm`). The
//! large neighborhood search helpers need sub-SCIP copies and are not
//! ported.

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DiveContext, DivesetId},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_probing,
    types::{BranchDir, HeurId, LpSolStat, ScipResultCode, SolId, VarId},
    var::{NONE, var_may_round_down, var_may_round_up},
};

/// Minimal number of LP iterations allowed in each LP solving call.
const MINLPITER: i64 = 10000;

/// `SCIPdivesetGetNLPIterations`.
fn diveset_get_n_lp_iterations(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> i64 {
    scip.divesets[diveset].divesetstats[divecontext as usize].nlpiterations
}

/// `SCIPdivesetGetNCalls`.
fn diveset_get_n_calls(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> i32 {
    scip.divesets[diveset].divesetstats[divecontext as usize].ncalls
}

/// `solveLP`: solve probing LP; returns `(lperror, cutoff)`.
fn solve_lp(scip: &mut Scip, diveset: DivesetId, maxnlpiterations: i64, divecontext: DiveContext) -> ScipResult<(bool, bool)> {
    let nlpiterations = scip.stat.nlpiterations;

    // Allow at least MINLPITER more iterations so as not to run out of LP
    // iterations during this solve.
    let mut lpiterationlimit = (maxnlpiterations - diveset_get_n_lp_iterations(scip, diveset, divecontext)) as i32;
    lpiterationlimit = crate::def::max(lpiterationlimit, MINLPITER as i32);

    // Errors in the LP solver should not kill the overall solving process,
    // if the LP is just needed for a heuristic (C catches the return code in
    // optimized mode; the out-pointers keep their values).
    let (lperror, cutoff) = scip_probing::solve_probing_lp(scip, lpiterationlimit).unwrap_or((false, false));

    // Update iteration count.
    let niters = scip.stat.nlpiterations - nlpiterations;
    crate::heur::diveset_update_lp_stats(scip, diveset, niters, divecontext);

    Ok((lperror, cutoff))
}

/// `selectNextDiving`: select the next variable and type of diving; returns
/// `(enfosuccess, infeasible)` and sets `nviollpcands`.
#[allow(clippy::too_many_arguments)]
fn select_next_diving(
    scip: &mut Scip,
    diveset: DivesetId,
    worksol: SolId,
    onlylpbranchcands: bool,
    storelpcandscores: bool,
    lpcands: &[VarId],
    lpcandssol: &[f64],
    lpcandsfrac: &[f64],
    lpcandsscores: &mut [f64],
    lpcandroundup: &mut [bool],
    nviollpcands: &mut i32,
    nlpcands: usize,
) -> ScipResult<(bool, bool)> {
    *nviollpcands = 0;
    // We use diving solution enforcement provided by the constraint
    // handlers.
    if !onlylpbranchcands {
        return api::get_dive_bound_changes(scip, diveset, worksol);
    }

    let mut bestscore = crate::def::REAL_MIN;
    let mut bestcandidx: i32 = -1;

    crate::tree::tree_clear_dive_bound_changes(scip);

    // Search for the candidate that maximizes the dive set score function
    // and whose solution value is still feasible.
    for c in 0..nlpcands {
        // Scores are kept in arrays for faster reuse.
        if storelpcandscores {
            (lpcandsscores[c], lpcandroundup[c]) = crate::heur::diveset_get_score(scip, diveset, DIVETYPE_INTEGRALITY, lpcands[c], lpcandssol[c], lpcandsfrac[c])?;
        }

        let score = lpcandsscores[c];
        // Update the best candidate if it has a higher score and a solution
        // value which does not violate one of the local bounds.
        let v = &scip.vars[lpcands[c]];
        if scip.set.is_le(v.locdom.lb, lpcandssol[c]) && scip.set.is_ge(v.locdom.ub, lpcandssol[c]) {
            if score > bestscore {
                bestcandidx = c as i32;
                bestscore = score;
            }
        } else {
            *nviollpcands += 1;
        }
    }

    // There is no guarantee that a candidate is found since local bounds
    // might render all solution values infeasible.
    let enfosuccess = bestcandidx >= 0;
    if enfosuccess {
        let b = bestcandidx as usize;
        // If we want to round up the best candidate, it is added as the
        // preferred bound change.
        let ceil = scip.set.ceil(lpcandssol[b]);
        let floor = scip.set.floor(lpcandssol[b]);
        api::add_dive_bound_change(scip, lpcands[b], BranchDir::Upwards, ceil, lpcandroundup[b]);
        api::add_dive_bound_change(scip, lpcands[b], BranchDir::Downwards, floor, !lpcandroundup[b]);
    }
    Ok((enfosuccess, false))
}

/// `getDivesetIterLimit`: return the LP iteration budget suggestion for
/// this dive set.
fn get_diveset_iter_limit(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> i64 {
    let d = &scip.divesets[diveset];
    let mut iterlimit = (d.maxlpiterquot * (crate::heur::diveset_get_sol_success(scip, diveset, divecontext) as f64 + 1.0)
        / (diveset_get_n_calls(scip, diveset, divecontext) as f64 + 1.0)
        * api::get_n_node_lp_iterations(scip) as f64) as i64;
    iterlimit += d.maxlpiterofs as i64;
    iterlimit -= diveset_get_n_lp_iterations(scip, diveset, divecontext);

    iterlimit
}

/// `SCIPperformGenericDivingAlgorithm`: performs a diving within the limits
/// of the diveset parameters. SCIP enters probing mode, selects candidates
/// maximizing the diveset score whose solution value is still feasible,
/// propagates their bound changes, resolves the LP when the frequency or
/// the number of domain changes says so, and optionally backtracks. The
/// SOS1 polishing step is skipped (that constraint handler is not ported).
#[allow(clippy::too_many_arguments)]
pub fn perform_generic_diving_algorithm(
    scip: &mut Scip,
    diveset: DivesetId,
    worksol: SolId,
    heur: HeurId,
    result: &mut ScipResultCode,
    nodeinfeasible: bool,
    iterlim: i64,
    nodelimit: i32,
    mut lpresolvedomchgquot: f64,
    divecontext: DiveContext,
) -> ScipResult {
    let _ = heur;
    *result = ScipResultCode::Delayed;

    // Do not call heuristic in node that was already detected to be
    // infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Only call heuristic, if an optimal LP solution is at hand.
    if !crate::tree::tree_has_current_node_lp(scip) || api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP solution is basic (which allows fast
    // resolve in diving).
    if !crate::lp::lp_is_sol_basic(scip) {
        return Ok(());
    }

    // When heuristic was called by scheduler, be less restrictive.
    if divecontext != DiveContext::Scheduler {
        // Don't dive two times at the same node.
        if scip.stat.lastdivenode == api::get_n_nodes(scip) && api::get_depth(scip) > 0 {
            return Ok(());
        }

        // Only call heuristic, if the LP objective value is smaller than the
        // cutoff bound.
        let lpobj = api::get_lp_objval(scip);
        if scip.set.is_ge(lpobj, api::get_cutoffbound(scip)) {
            return Ok(());
        }
    }

    *result = ScipResultCode::DidNotRun;

    // Only try to dive, if we are in the correct part of the tree, given by
    // minreldepth and maxreldepth.
    let depth = api::get_depth(scip);
    let mut maxdepth = scip.stat.maxdepth;
    maxdepth = crate::def::max(maxdepth, 30);
    let (minreldepth, maxreldepth) = (scip.divesets[diveset].minreldepth, scip.divesets[diveset].maxreldepth);
    if divecontext != DiveContext::Scheduler && ((depth as f64) < minreldepth * maxdepth as f64 || depth as f64 > maxreldepth * maxdepth as f64) {
        return Ok(());
    }

    // Calculate the maximal number of LP iterations.
    let mut maxnlpiterations = if iterlim < 0 {
        diveset_get_n_lp_iterations(scip, diveset, divecontext) + get_diveset_iter_limit(scip, diveset, divecontext)
    } else {
        diveset_get_n_lp_iterations(scip, diveset, divecontext) + iterlim
    };

    // Don't try to dive, if we took too many LP iterations during diving.
    if diveset_get_n_lp_iterations(scip, diveset, divecontext) >= maxnlpiterations {
        return Ok(());
    }

    // Allow at least a certain number of LP iterations in this dive.
    if diveset_get_n_lp_iterations(scip, diveset, divecontext) + MINLPITER > maxnlpiterations {
        maxnlpiterations = diveset_get_n_lp_iterations(scip, diveset, divecontext) + MINLPITER;
    }

    // These constraint handlers are required for polishing an LP relaxation
    // solution beyond rounding (SOS1 is not ported, so it is absent).
    let indconshdlr = crate::set::set_find_conshdlr(scip, "indicator");

    let (mut lpcands, mut lpcandssol, mut lpcandsfrac, nlpcands0, _, _) = api::get_lp_branch_cands(scip)?;
    let mut nlpcands = nlpcands0 as usize;

    let onlylpbranchcands = scip.divesets[diveset].onlylpbranchcands;
    // Don't try to dive, if there are no diving candidates.
    if onlylpbranchcands && nlpcands == 0 {
        return Ok(());
    }

    // Calculate the objective search bound.
    let d = &scip.divesets[diveset];
    let (ubquot, avgquot) = if api::get_n_sols_found(scip) == 0 {
        (d.maxdiveubquotnosol, d.maxdiveavgquotnosol)
    } else {
        (d.maxdiveubquot, d.maxdiveavgquot)
    };

    let searchubbound = if ubquot > 0.0 {
        api::get_lowerbound(scip) + ubquot * (api::get_cutoffbound(scip) - api::get_lowerbound(scip))
    } else {
        scip.set.infinity()
    };

    let searchavgbound = if avgquot > 0.0 {
        api::get_lowerbound(scip) + avgquot * (api::get_avg_lowerbound(scip) - api::get_lowerbound(scip))
    } else {
        scip.set.infinity()
    };

    let mut searchbound = crate::def::min(searchubbound, searchavgbound);

    if api::is_obj_integral(scip) {
        searchbound = scip.set.ceil(searchbound);
    }

    // Calculate the maximal diving depth: 10 * min{number of integer
    // variables, max depth}.
    let mut maxdivedepth = api::get_n_vars(scip) - api::get_n_cont_vars(scip) - api::get_n_cont_impl_vars(scip);
    maxdivedepth = crate::def::min(maxdivedepth, maxdepth);
    maxdivedepth *= 10;

    // If lpresolvedomchgquot is not explicitly given (so -1), then use the
    // default for the current diveset.
    if lpresolvedomchgquot < 0.0 {
        lpresolvedomchgquot = scip.divesets[diveset].lpresolvedomchgquot;
    }
    let _ = lpresolvedomchgquot;

    *result = ScipResultCode::DidNotFind;

    // Start probing mode.
    scip_probing::start_probing(scip)?;

    // Enables collection of variable statistics during probing.
    crate::stat::stat_enable_var_history(scip);

    // Buffer storage for previous candidates and their branching values for
    // pseudo cost updates.
    let lpsolvefreq = scip.divesets[diveset].lpsolvefreq;
    let mut previouscands: Vec<VarId> = vec![NONE; crate::def::max(1, lpsolvefreq) as usize];
    let mut previousvals: Vec<f64> = vec![0.0; crate::def::max(1, lpsolvefreq) as usize];

    // Keep some statistics.
    let mut lperror = false;
    let mut cutoff = false;
    let mut lastlpdepth: i32 = -1;
    let startndivecands = nlpcands as i32;
    let mut totalnbacktracks = 0;
    let mut totalnprobingnodes = 0;
    let oldnsolsfound = api::get_n_sols_found(scip);
    let oldnbestsolsfound = api::get_n_best_sols_found(scip);
    let oldnconflictsfound = api::get_n_conflict_conss_found(scip);

    // Link the working solution to the dive set.
    scip.divesets[diveset].sol = worksol;

    let (mut lpcandsscores, mut lpcandroundup): (Vec<f64>, Vec<bool>) = if onlylpbranchcands {
        (vec![0.0; nlpcands], vec![false; nlpcands])
    } else {
        (Vec::new(), Vec::new())
    };

    let mut enfosuccess = true;
    let mut leafsol = false;
    let mut nviollpcands = 0;

    let ret = (|| -> ScipResult<bool> {
        // LP loop; every time a new LP was solved, conditions are checked;
        // dive as long we are in the given objective, depth and iteration
        // limits and fractional variables exist, but
        // - if possible, we dive at least with the depth 10
        // - if the number of fractional variables decreased at least with 1
        //   variable per 2 dive depths, we continue diving
        loop {
            let pdepth = scip_probing::get_probing_depth(scip);
            let keep = !lperror
                && !cutoff
                && api::get_lp_solstat(scip) == LpSolStat::Optimal
                && enfosuccess
                && (nodelimit == -1 || totalnprobingnodes < nodelimit)
                && (pdepth < 10
                    || nlpcands as i32 <= startndivecands - pdepth / 2
                    || (pdepth < maxdivedepth && diveset_get_n_lp_iterations(scip, diveset, divecontext) < maxnlpiterations && api::get_lp_objval(scip) < searchbound))
                && !crate::solve::solve_is_stopped(scip, false);
            if !keep {
                break;
            }

            // Remember the last LP depth.
            lastlpdepth = scip_probing::get_probing_depth(scip);
            let mut domreds: i64 = 0;

            // Loop over candidates and determine if they are roundable.
            let mut allroundable = true;
            let mut c = 0;
            while allroundable && c < nlpcands {
                allroundable = var_may_round_down(scip, lpcands[c]) || var_may_round_up(scip, lpcands[c]);
                c += 1;
            }

            // If all candidates are roundable, try to round the solution.
            if allroundable {
                // Working solution must be linked to LP solution.
                crate::sol::sol_link_lp_sol(scip, worksol, TRANSPROB)?;
                // Create solution from diving LP and try to round it.
                let success = api::round_sol(scip, worksol)?;

                // Succesfully rounded solutions are tried for primal
                // feasibility.
                if success {
                    let mut changed = false;

                    // Adjust indicator constraints.
                    if indconshdlr != NONE {
                        changed = crate::cons_indicator::make_indicators_feasible(scip, indconshdlr, worksol)?;
                    }

                    // Try to add solution to SCIP.
                    let stored = api::try_sol(scip, worksol, false, false, false, false, changed)?;

                    // Check, if solution was feasible and good enough.
                    if stored {
                        *result = ScipResultCode::FoundSol;
                        leafsol = nlpcands == 0;

                        // The rounded solution found above led to a cutoff of
                        // the node LP solution.
                        if api::get_lp_solstat(scip) == LpSolStat::ObjLimit {
                            cutoff = true;
                            break;
                        }
                    }
                }
            }

            // Working solution must be linked to LP solution.
            let lastlpobjval = api::get_lp_objval(scip);
            crate::sol::sol_link_lp_sol(scip, worksol, TRANSPROB)?;

            // We must explicitly store the solution values by unlinking the
            // solution, otherwise, the working solution may contain wrong
            // entries, if, e.g., a backtrack occurred after an intermediate
            // LP resolve or the LP was resolved during conflict analysis.
            api::unlink_sol(scip, worksol)?;

            // Ensure array sizes for the diving on the fractional variables.
            if onlylpbranchcands && nlpcands > lpcandsscores.len() {
                lpcandsscores.resize(nlpcands, 0.0);
                lpcandroundup.resize(nlpcands, false);
            }

            // Select the next diving action by selecting appropriate dive
            // bound changes for the preferred and alternative child.
            let store = scip_probing::get_probing_depth(scip) == lastlpdepth;
            let (es, _infeasible) = select_next_diving(
                scip,
                diveset,
                worksol,
                onlylpbranchcands,
                store,
                &lpcands,
                &lpcandssol,
                &lpcandsfrac,
                &mut lpcandsscores,
                &mut lpcandroundup,
                &mut nviollpcands,
                nlpcands,
            )?;
            enfosuccess = es;

            // If we did not succeed finding an enforcement, the solution is
            // potentially feasible and we break immediately.
            if !enfosuccess {
                break;
            }

            let mut prevcandsinsertidx: i32 = -1;

            // Start propagating candidate variables
            //   - until the desired targetdepth is reached,
            //   - or there is no further candidate variable left because of
            //     intermediate bound changes,
            //   - or a cutoff is detected.
            loop {
                let mut localdomreds: i64 = 0;

                // Ensure that a new candidate was successfully determined
                // (usually at the end of the previous loop iteration).
                let mut bdchgvar = NONE;
                let mut bdchgvalue = crate::def::INVALID;
                let mut bdchgdir = BranchDir::Auto;
                let mut nbdchanges;

                let mut backtracked = false;
                loop {
                    // Get the bound change information stored in the dive
                    // set.
                    let (bdchgvars, bdchgdirs, bdchgvals) = {
                        let (v, d, x) = crate::tree::tree_get_dive_bound_change_data(scip, !backtracked);
                        (v.to_vec(), d.to_vec(), x.to_vec())
                    };
                    nbdchanges = bdchgvars.len();

                    // Return if we reached the depth limit.
                    if crate::def::MAXTREEDEPTH <= api::get_depth(scip) {
                        return Ok(true);
                    }

                    // Return if we reached the node limit.
                    if nodelimit != -1 && totalnprobingnodes >= nodelimit {
                        return Ok(true);
                    }

                    // Dive deeper into the tree.
                    scip_probing::new_probing_node(scip)?;
                    totalnprobingnodes += 1;

                    // Apply all suggested domain changes of the variables.
                    for dd in 0..nbdchanges {
                        // Get the next bound change data: variable, direction,
                        // and value.
                        bdchgvar = bdchgvars[dd];
                        bdchgvalue = bdchgvals[dd];
                        bdchgdir = bdchgdirs[dd];

                        let lblocal = scip.vars[bdchgvar].locdom.lb;
                        let ublocal = scip.vars[bdchgvar].locdom.ub;

                        let mut infeasbdchange = false;
                        // Tighten the lower and/or upper bound depending on
                        // the bound change type.
                        match bdchgdir {
                            BranchDir::Upwards => {
                                // Test if bound change is possible, otherwise
                                // propagation might have deduced the same
                                // bound already or numerical troubles might
                                // have occurred.
                                if scip.set.is_feas_le(bdchgvalue, lblocal) || scip.set.is_feas_gt(bdchgvalue, ublocal) {
                                    infeasbdchange = true;
                                } else {
                                    // Round variable up.
                                    scip_probing::chg_var_lb_probing(scip, bdchgvar, bdchgvalue)?;
                                }
                            }
                            BranchDir::Downwards => {
                                if scip.set.is_feas_ge(bdchgvalue, ublocal) || scip.set.is_feas_lt(bdchgvalue, lblocal) {
                                    infeasbdchange = true;
                                } else {
                                    // Round variable down.
                                    scip_probing::chg_var_ub_probing(scip, bdchgvar, bdchgvalue)?;
                                }
                            }
                            BranchDir::Fixed => {
                                if scip.set.is_feas_lt(bdchgvalue, lblocal) || scip.set.is_feas_gt(bdchgvalue, ublocal) || (scip.set.is_feas_eq(lblocal, ublocal) && nbdchanges < 2)
                                {
                                    infeasbdchange = true;
                                } else {
                                    // Fix variable to the bound change value.
                                    if scip.set.is_feas_lt(lblocal, bdchgvalue) {
                                        scip_probing::chg_var_lb_probing(scip, bdchgvar, bdchgvalue)?;
                                    }
                                    if scip.set.is_feas_gt(ublocal, bdchgvalue) {
                                        scip_probing::chg_var_ub_probing(scip, bdchgvar, bdchgvalue)?;
                                    }
                                }
                            }
                            BranchDir::Auto => {
                                return Err(crate::retcode::Retcode::InvalidData);
                            }
                        }
                        // If the variable domain has been shrunk in the
                        // meantime, numerical troubles may have occured or
                        // variable was fixed by propagation while
                        // backtracking => Abort diving!
                        if infeasbdchange {
                            cutoff = true;
                            break;
                        }
                    }
                    // Break loop immediately if we detected a cutoff.
                    if cutoff {
                        break;
                    }

                    // Apply domain propagation.
                    let (co, ld) = scip_probing::propagate_probing(scip, 0)?;
                    cutoff = co;
                    localdomreds = ld;

                    // Add the number of bound changes we applied by ourselves
                    // after propagation, otherwise the counter would have
                    // been reset.
                    localdomreds += nbdchanges as i64;

                    // Resolve the diving LP if the diving resolve frequency is
                    // reached or a sufficient number of intermediate bound
                    // changes was reached.
                    let quot = scip.divesets[diveset].lpresolvedomchgquot;
                    if !cutoff
                        && ((lpsolvefreq > 0 && ((scip_probing::get_probing_depth(scip) - lastlpdepth) % lpsolvefreq) == 0)
                            || ((domreds + localdomreds) as f64 > quot * api::get_n_vars(scip) as f64)
                            || (onlylpbranchcands && nviollpcands > (quot * nlpcands as f64) as i32))
                    {
                        (lperror, cutoff) = solve_lp(scip, diveset, maxnlpiterations, divecontext)?;

                        // LP errors lead to early termination.
                        if lperror {
                            cutoff = true;
                            break;
                        }
                    }

                    // Perform backtracking if a cutoff was detected.
                    let backtrack;
                    if cutoff && !backtracked && scip.divesets[diveset].backtrack {
                        let pd = scip_probing::get_probing_depth(scip);
                        scip_probing::backtrack_probing(scip, pd - 1)?;
                        totalnbacktracks += 1;
                        backtracked = true;
                        backtrack = true;
                        cutoff = false;
                    } else {
                        backtrack = false;
                    }
                    if !backtrack {
                        break;
                    }
                }

                // We add the domain reductions from the last evaluated node.
                domreds += localdomreds;

                // Store candidate for pseudo cost update and choose next
                // candidate only if no cutoff was detected.
                if !cutoff {
                    if nbdchanges == 1 && (bdchgdir == BranchDir::Upwards || bdchgdir == BranchDir::Downwards) {
                        prevcandsinsertidx += 1;

                        // Extend array in case of a dynamic, domain change
                        // based LP resolve strategy.
                        if prevcandsinsertidx as usize >= previouscands.len() {
                            let n = previouscands.len() * 2;
                            previouscands.resize(n, NONE);
                            previousvals.resize(n, 0.0);
                        }

                        // Store candidate for pseudo cost update.
                        previouscands[prevcandsinsertidx as usize] = bdchgvar;
                        previousvals[prevcandsinsertidx as usize] = bdchgvalue;
                    }

                    // Choose next candidate variable and resolve the LP if
                    // none is found.
                    if api::get_lp_solstat(scip) == LpSolStat::NotSolved {
                        // Select the next diving action.
                        let store = scip_probing::get_probing_depth(scip) == lastlpdepth;
                        let (es, _infeasible) = select_next_diving(
                            scip,
                            diveset,
                            worksol,
                            onlylpbranchcands,
                            store,
                            &lpcands,
                            &lpcandssol,
                            &lpcandsfrac,
                            &mut lpcandsscores,
                            &mut lpcandroundup,
                            &mut nviollpcands,
                            nlpcands,
                        )?;
                        enfosuccess = es;

                        // In case of an unsuccesful candidate search, we solve
                        // the node LP.
                        if !enfosuccess {
                            (lperror, cutoff) = solve_lp(scip, diveset, maxnlpiterations, divecontext)?;

                            // Check for an LP error and terminate in this
                            // case, cutoffs lead to termination anyway.
                            if lperror {
                                cutoff = true;
                            }

                            // enfosuccess must be set to TRUE for entering the
                            // main LP loop again.
                            enfosuccess = true;
                        }
                    }
                }
                if !(!cutoff && api::get_lp_solstat(scip) == LpSolStat::NotSolved) {
                    break;
                }
            }

            // Check new LP candidates and use the LP Objective gain to update
            // pseudo cost information.
            if !cutoff && api::get_lp_solstat(scip) == LpSolStat::Optimal {
                let (c, s, f, n, _, _) = api::get_lp_branch_cands(scip)?;
                (lpcands, lpcandssol, lpcandsfrac) = (c, s, f);
                nlpcands = n as usize;

                // Distribute the gain equally over all variables that we
                // rounded since the last LP.
                let mut gain = api::get_lp_objval(scip) - lastlpobjval;
                gain = crate::def::max(gain, 0.0);
                gain /= 1.0 * (scip_probing::get_probing_depth(scip) - lastlpdepth) as f64;

                // Loop over previously fixed candidates and share gain
                // improvement.
                for v in 0..(prevcandsinsertidx + 1) as usize {
                    let cand = previouscands[v];
                    let val = previousvals[v];
                    let solval = api::get_sol_val(scip, worksol, cand);

                    // It may happen that a variable had an integral solution
                    // value beforehand, e.g., for indicator variables.
                    if !scip.set.is_zero(val - solval) {
                        crate::scip_var::update_var_pseudocost(scip, cand, val - solval, gain, 1.0)?;
                    }
                }
            } else {
                nlpcands = 0;
            }
        }

        // Check if a solution has been found.
        if !enfosuccess && !lperror && !cutoff && api::get_lp_solstat(scip) == LpSolStat::Optimal {
            // Create solution from diving LP.
            crate::sol::sol_link_lp_sol(scip, worksol, TRANSPROB)?;

            // Try to add solution to SCIP.
            let success = api::try_sol(scip, worksol, false, false, false, false, false)?;

            // Check, if solution was feasible and good enough.
            if success {
                *result = ScipResultCode::FoundSol;
                leafsol = true;
            }
        }

        let depth = api::get_depth(scip);
        let nsols = api::get_n_sols_found(scip) - oldnsolsfound;
        let nbest = api::get_n_best_sols_found(scip) - oldnbestsolsfound;
        let nconf = api::get_n_conflict_conss_found(scip) - oldnconflictsfound;
        crate::heur::diveset_update_stats(scip, diveset, depth, totalnprobingnodes, totalnbacktracks, nsols, nbest, nconf, leafsol, divecontext);

        Ok(false)
    })();

    // TERMINATE: end probing mode.
    let r = ret.map(|_| ());
    let e = scip_probing::end_probing(scip);
    scip.divesets[diveset].sol = NONE;
    r?;
    e
}
