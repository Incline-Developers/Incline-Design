//! Port of `scip_solve.c` (transforming, presolving and solving the
//! problem) and the problem creation of `scip_prob.c`.
//!
//! Not ported: reoptimization, concurrent solving, exact solving and
//! certificates, the NLP (the reference creates it whenever nonlinear
//! constraints enable it, but has no NLP solver), visualization, the
//! decomposition storage, Benders and pricers, and the CTRL-C capture.

use crate::{
    api,
    cons::{PRESOLTIMING_EXHAUSTIVE, PRESOLTIMING_FAST, PRESOLTIMING_FINAL, PRESOLTIMING_MEDIUM, PresolCounts, PresolTiming},
    event::{EVENTTYPE_DUALBOUNDIMPROVED, EVENTTYPE_PRESOLVEROUND, Event, EventData},
    prob::{ORIGPROB, TRANSPROB},
    retcode::{Retcode, ScipResult},
    scip::{GLOBALEVENTFILTER, ORIGPRIMAL, Scip, Stage, TRANSPRIMAL},
    types::{ObjSense, ScipResultCode, Status, VarStatus},
    var::NONE,
};

/*
 * Problem creation (scip_prob.c)
 */

/// `SCIPcreateProbBasic` (`scip_prob.c`): creates empty problem and
/// initializes all solving data structures (problem data callbacks are not
/// ported).
pub fn create_prob(scip: &mut Scip, name: &str) -> ScipResult {
    free_prob(scip)?;

    scip.set.stage = Stage::Problem;

    crate::stat::stat_create(scip);
    crate::prob::prob_create(scip, ORIGPROB, name, false);
    scip.primals.insert_at(ORIGPRIMAL, crate::primal::primal_create());
    crate::conflictstore::conflictstore_create(scip)?;

    Ok(())
}

/// `SCIPsetObjsense` (`scip_prob.c`): sets objective sense of problem.
pub fn set_objsense(scip: &mut Scip, objsense: ObjSense) -> ScipResult {
    crate::prob::prob_set_objsense(scip, ORIGPROB, objsense);
    Ok(())
}

/// `SCIPfreeProb` (`scip_prob.c`): frees problem and solution process data.
pub fn free_prob(scip: &mut Scip) -> ScipResult {
    let transsolorig = scip.set.misc_transsolsorig;
    scip.set.misc_transsolsorig = false;

    free_transform_public(scip)?;

    scip.set.misc_transsolsorig = transsolorig;

    if scip.set.stage == Stage::Problem {
        crate::conflictstore::conflictstore_free(scip)?;
        crate::primal::primal_clear(scip, ORIGPRIMAL)?;
        scip.primals.remove(ORIGPRIMAL);
        crate::prob::prob_free(scip, ORIGPROB)?;

        scip.set.stage = Stage::Init;
    }

    Ok(())
}

/*
 * Local methods
 */

/// `calcNonZeros`: calculates number of nonzeros in problem; returns
/// `(nchecknonzeros, nactivenonzeros, approxchecknonzeros,
/// approxactivenonzeros)`.
fn calc_non_zeros(scip: &mut Scip) -> ScipResult<(i64, i64, bool, bool)> {
    let mut nchecknonzeros: i64 = 0;
    let mut nactivenonzeros: i64 = 0;
    let mut approxchecknonzeros = false;
    let mut approxactivenonzeros = false;

    // Computes number of non-zeros over all active constraints.
    for h in (0..scip.set.nconshdlrs as usize).rev() {
        let conshdlr = scip.set.conshdlrs[h];
        let hd = &scip.conshdlrs[conshdlr];
        let nconss = hd.nactiveconss as usize;
        if nconss > 0 {
            let conss = hd.conss.0[..nconss].to_vec();

            // Calculate all active constraints.
            for &cons in conss.iter().rev() {
                let (nvars, success) = crate::cons::cons_get_n_vars(scip, cons)?;
                let ischeck = scip.conss[cons].is_checked();

                if !success {
                    approxactivenonzeros = true;
                    if ischeck {
                        approxchecknonzeros = true;
                    }
                } else {
                    nactivenonzeros += nvars as i64;
                    if ischeck {
                        nchecknonzeros += nvars as i64;
                    }
                }
            }
        }

        // Add nonzeros on inactive check constraints.
        let hd = &scip.conshdlrs[conshdlr];
        let nconss = hd.ncheckconss as usize;
        if nconss > 0 {
            let conss = hd.checkconss.0[..nconss].to_vec();
            for &cons in conss.iter().rev() {
                if !scip.conss[cons].is_active() {
                    let (nvars, success) = crate::cons::cons_get_n_vars(scip, cons)?;

                    if !success {
                        approxchecknonzeros = true;
                    } else {
                        nchecknonzeros += nvars as i64;
                    }
                }
            }
        }
    }

    Ok((nchecknonzeros, nactivenonzeros, approxchecknonzeros, approxactivenonzeros))
}

/// Creates the global event filter at its fixed id.
fn create_global_eventfilter(scip: &mut Scip) {
    let id = crate::event::eventfilter_create(scip);
    if id != GLOBALEVENTFILTER {
        let f = scip.eventfilters.remove(id);
        scip.eventfilters.insert_at(GLOBALEVENTFILTER, f);
    }
}

/// `SCIPtransformProb`: initializes solving data structures and transforms
/// problem.
pub fn transform_prob(scip: &mut Scip) -> ScipResult {
    // Check, if the problem was already transformed.
    if scip.set.stage >= Stage::Transformed {
        return Ok(());
    }

    // Check, if a node selector exists.
    let nodesel = crate::nodesel::set_get_nodesel(scip);
    if nodesel == NONE {
        // No node selector available.
        return Err(Retcode::PluginNotFound);
    }

    // Remember number of constraints.
    crate::prob::prob_mark_nconss(scip, ORIGPROB);

    // Switch stage to TRANSFORMING.
    scip.set.stage = Stage::Transforming;

    // Mark statistics before solving.
    crate::stat::stat_mark(scip);

    // Init solve data structures.
    create_global_eventfilter(scip);
    scip.eventqueue = Default::default();
    scip.branchcand = crate::branch::branchcand_create();
    let name = scip.probs[ORIGPROB].name.clone();
    crate::lp::lp_create(scip, &name)?;
    scip.primals.insert_at(TRANSPRIMAL, crate::primal::primal_create());
    crate::tree::tree_create(scip, nodesel)?;
    scip.relaxation = Default::default();
    scip.conflict = Some(crate::conflict::conflict_create(scip));
    crate::implics::cliquetable_create(scip);

    // Copy problem in solve memory.
    crate::prob::prob_transform(scip, ORIGPROB, TRANSPROB)?;

    // Switch stage to TRANSFORMED.
    scip.set.stage = Stage::Transformed;

    // Check, whether objective value is always integral by inspecting the
    // problem, if it is the case adjust the cutoff bound if primal solution is
    // already known.
    crate::prob::prob_check_obj_integral(scip, TRANSPROB, ORIGPROB)?;

    // If possible, scale objective function such that it becomes integral with
    // gcd 1.
    crate::prob::prob_scale_obj(scip, TRANSPROB, ORIGPROB)?;

    // Check solution of solution candidate storage.
    let mut oldnsolsfound = 0;

    // Update upper bound and cutoff bound due to objective limit in primal
    // data.
    crate::primal::primal_update_objlimit(scip, TRANSPRIMAL, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;

    // Do not consider original solutions of a benders decomposition because
    // their cost information is incomplete.
    if !scip.set.reopt_enable && scip.set.nactivebenders == 0 {
        oldnsolsfound = scip.primals[TRANSPRIMAL].nsolsfound;
        for s in (0..scip.primals[ORIGPRIMAL].nsols).rev() {
            let sol = scip.primals[ORIGPRIMAL].sols[s];

            // Recompute objective function, since the objective might have changed
            // in the meantime.
            api::recompute_sol_obj(scip, sol);

            // SCIPprimalTrySol() can only be called on transformed solutions;
            // therefore check solutions in original problem including modifiable
            // constraints.
            let feasible = crate::sol::sol_check_orig(scip, sol, ORIGPROB, ORIGPRIMAL, false, false, true, true, true, true)?;

            if feasible {
                let bestsol = api::get_best_sol(scip);

                // Add primal solution to solution storage by copying it.
                let stored = crate::primal::primal_add_sol(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER, sol)?;

                if stored && bestsol != api::get_best_sol(scip) {
                    api::store_solution_gap(scip);
                }
            }

            crate::sol::sol_free(scip, sol, ORIGPRIMAL)?;
            scip.primals[ORIGPRIMAL].nsols -= 1;
        }
    }

    scip.stat.nexternalsolsfound += scip.primals[TRANSPRIMAL].nsolsfound - oldnsolsfound;

    // Determine number of non-zeros.
    let (_nchecknonzeros, nactivenonzeros, _, _) = calc_non_zeros(scip)?;
    let nconss = api::get_n_conss(scip);
    scip.stat.nnz = nactivenonzeros;
    scip.stat.avgnnz = if nconss == 0 { 0.0 } else { nactivenonzeros as f64 / nconss as f64 };

    // Call initialization methods of plugins.
    crate::set::set_init_plugins(scip)?;

    // In case the permutation seed is different to 0, permute the transformed
    // problem.
    if scip.set.random_permutationseed > 0 {
        unimplemented!("permuting the transformed problem (randomization/permutationseed)");
    }

    if scip.set.misc_estimexternmem {
        // The following formula was estimated empirically using linear
        // regression.
        let nvars = api::get_n_vars(scip);
        let est = crate::def::max(1.0, 8.5e-04 * nconss as f64 + 7.6e-04 * nvars as f64 + 3.5e-05 * scip.stat.nnz as f64);
        scip.stat.externmemestim = (est * 1048576.0) as i64;
    }

    Ok(())
}

/// `initPresolve`: initializes presolving.
fn init_presolve(scip: &mut Scip) -> ScipResult {
    // Reset statistics for presolving and current branch and bound run.
    crate::stat::stat_reset_presolving(scip);

    // Increase number of branch and bound runs.
    scip.stat.nruns += 1;

    // Remember problem size of previous run.
    scip.stat.prevrunnvars = scip.probs[TRANSPROB].nvars;

    // Switch stage to INITPRESOLVE.
    scip.set.stage = Stage::InitPresolve;

    // Create temporary presolving root node.
    crate::tree::tree_create_presolving_root(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;

    // Retransform all existing solutions to original problem space, because
    // the transformed problem space may get modified in presolving and the
    // solutions may become invalid for the transformed problem.
    crate::primal::primal_retransform_solutions(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER)?;

    // Initialize lower bound of the presolving root node if a valid dual bound
    // is at hand.
    if scip.probs[TRANSPROB].dualbound != crate::def::INVALID {
        let dualbound = scip.probs[TRANSPROB].dualbound;
        let lb = crate::prob::prob_intern_objval(scip, TRANSPROB, ORIGPROB, dualbound);
        let root = treer!(scip).root;
        scip.nodes[root].lowerbound = lb;
        scip.nodes[root].estimate = lb;
        scip.stat.rootlowerbound = lb;
    }

    // Inform plugins that the presolving is abound to begin.
    crate::set::set_initpre_plugins(scip)?;

    // Delete the variables from the problems that were marked to be deleted.
    crate::prob::prob_perform_var_deletions(scip, TRANSPROB)?;

    // Switch stage to PRESOLVING.
    scip.set.stage = Stage::Presolving;

    Ok(())
}

/// `exitPresolve`: deinitializes presolving; returns whether the clique
/// clean up detects an infeasibility.
fn exit_presolve(scip: &mut Scip, solved: bool) -> ScipResult<bool> {
    let mut infeasible = false;

    // Switch stage to EXITPRESOLVE.
    scip.set.stage = Stage::ExitPresolve;

    // Inform plugins that the presolving is finished, and perform final
    // modifications.
    crate::set::set_exitpre_plugins(scip)?;

    // Remove empty and single variable cliques from the clique table, and
    // convert all two variable cliques into implications; delete the variables
    // from the problems that were marked to be deleted.
    if !solved {
        let mut nlocalbdchgs = 0;

        crate::prob::prob_perform_var_deletions(scip, TRANSPROB)?;

        crate::implics::cliquetable_cleanup(scip, TRANSPROB, ORIGPROB, &mut nlocalbdchgs, &mut infeasible)?;
    }

    // Exit presolving.
    crate::prob::prob_exit_presolve(scip, TRANSPROB)?;

    if !solved {
        // Check, whether objective value is always integral by inspecting the
        // problem, if it is the case adjust the cutoff bound if primal solution
        // is already known.
        crate::prob::prob_check_obj_integral(scip, TRANSPROB, ORIGPROB)?;

        // If possible, scale objective function such that it becomes integral
        // with gcd 1.
        crate::prob::prob_scale_obj(scip, TRANSPROB, ORIGPROB)?;
    }

    // Free temporary presolving root node.
    crate::tree::tree_free_presolving_root(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;

    // Switch stage to PRESOLVED.
    scip.set.stage = Stage::Presolved;

    Ok(infeasible)
}

/// The presolving counters of the statistics, as the counts the presolving
/// callbacks update.
fn stat_presol_counts(scip: &Scip) -> PresolCounts {
    let s = &scip.stat;
    PresolCounts {
        nfixedvars: s.npresolfixedvars,
        naggrvars: s.npresolaggrvars,
        nchgvartypes: s.npresolchgvartypes,
        nchgbds: s.npresolchgbds,
        naddholes: s.npresoladdholes,
        ndelconss: s.npresoldelconss,
        naddconss: s.npresoladdconss,
        nupgdconss: s.npresolupgdconss,
        nchgcoefs: s.npresolchgcoefs,
        nchgsides: s.npresolchgsides,
    }
}

/// Stores counts back into the presolving counters of the statistics.
fn set_stat_presol_counts(scip: &mut Scip, c: &PresolCounts) {
    let s = &mut scip.stat;
    s.npresolfixedvars = c.nfixedvars;
    s.npresolaggrvars = c.naggrvars;
    s.npresolchgvartypes = c.nchgvartypes;
    s.npresolchgbds = c.nchgbds;
    s.npresoladdholes = c.naddholes;
    s.npresoldelconss = c.ndelconss;
    s.npresoladdconss = c.naddconss;
    s.npresolupgdconss = c.nupgdconss;
    s.npresolchgcoefs = c.nchgcoefs;
    s.npresolchgsides = c.nchgsides;
}

/// `SCIPpropPresol` on the statistics' presolving counters.
fn run_prop_presol(scip: &mut Scip, prop: usize, timing: PresolTiming) -> ScipResult<ScipResultCode> {
    let mut counts = stat_presol_counts(scip);
    let nrounds = scip.stat.npresolrounds;
    let result = crate::prop::prop_presol(scip, prop, timing, nrounds, &mut counts);
    set_stat_presol_counts(scip, &counts);
    result
}

/// `SCIPpresolExec` on the statistics' presolving counters.
fn run_presol_exec(scip: &mut Scip, presol: usize, timing: PresolTiming) -> ScipResult<ScipResultCode> {
    let mut counts = stat_presol_counts(scip);
    let nrounds = scip.stat.npresolrounds;
    let result = crate::presol::presol_exec(scip, presol, timing, nrounds, &mut counts);
    set_stat_presol_counts(scip, &counts);
    result
}

/// `SCIPconshdlrPresolve` on the statistics' presolving counters.
fn run_conshdlr_presolve(scip: &mut Scip, conshdlr: usize, timing: PresolTiming) -> ScipResult<ScipResultCode> {
    let mut counts = stat_presol_counts(scip);
    let nrounds = scip.stat.npresolrounds;
    let result = crate::cons::conshdlr_presolve(scip, conshdlr, timing, nrounds, &mut counts);
    set_stat_presol_counts(scip, &counts);
    result
}

/// The state of a presolving round loop (`presolstart` etc. of
/// `presolveRound`).
struct RoundStarts {
    presolstart: i32,
    propstart: i32,
    consstart: i32,
}

/// `presolveRound`: applies one round of presolving with the given
/// presolving timing
///
/// This method will always be called with presoltiming fast first. It
/// iterates over all presolvers, propagators, and constraint handlers and
/// calls their presolving callbacks with timing fast. If enough reductions
/// are found, it returns and the next presolving round will be started
/// (again with timing fast). If the fast presolving does not find enough
/// reductions, this methods calls itself recursively with presoltiming
/// medium. Again, it calls the presolving callbacks of all presolvers,
/// propagators, and constraint handlers with timing medium. If enough
/// reductions are found, it returns and the next presolving round will be
/// started (with timing fast). Otherwise, it is called recursively with
/// presoltiming exhaustive. In exhaustive presolving, presolvers,
/// propagators, and constraint handlers are called w.r.t. their priority,
/// but this time, we stop as soon as enough reductions were found and do not
/// necessarily call all presolving methods. If we stop, we return and
/// another presolving round is started with timing fast.
#[allow(clippy::too_many_arguments)]
fn presolve_round(
    scip: &mut Scip,
    timing: &mut PresolTiming,
    unbounded: &mut bool,
    infeasible: &mut bool,
    lastround: bool,
    st: &mut RoundStarts,
    presolend: i32,
    propend: i32,
    consend: i32,
) -> ScipResult {
    let mut result;
    let mut aborted = false;
    let mut lastranpresol;

    *unbounded = false;
    *infeasible = false;

    let mut i;
    let mut j;
    let mut k;
    if *timing == PRESOLTIMING_EXHAUSTIVE {
        // In exhaustive presolving, we continue the loop where we stopped last
        // time to avoid calling the same (possibly ineffective) presolving step
        // again and again. If we reach the end of the arrays of presolvers,
        // propagators, and constraint handlers without having made enough
        // reductions, we start again from the beginning.
        i = st.presolstart;
        j = st.propstart;
        k = st.consstart;
        if i >= presolend && j >= propend && k >= consend {
            return Ok(());
        }

        if i == 0 && j == 0 && k == 0 {
            scip.stat.npresolroundsext += 1;
        }
    } else {
        // In fast and medium presolving, we always iterate over all presolvers,
        // propagators, and constraint handlers.
        i = 0;
        j = 0;
        k = 0;

        if *timing == PRESOLTIMING_FAST {
            scip.stat.npresolroundsfast += 1;
        }
        if *timing == PRESOLTIMING_MEDIUM {
            scip.stat.npresolroundsmed += 1;
        }
    }

    let presol_prio = |scip: &Scip, i: i32| scip.presols[scip.set.presols[i as usize]].priority;
    let prop_prio = |scip: &Scip, j: i32| scip.props[scip.set.props_presol[j as usize]].presolpriority;

    // Call included presolvers with nonnegative priority.
    while !*unbounded && !*infeasible && !aborted && (i < presolend || j < propend) {
        let priopresol = if i < presolend { presol_prio(scip, i) } else { -1 };
        let prioprop = if j < propend { prop_prio(scip, j) } else { -1 };

        // Call next propagator.
        if prioprop >= priopresol {
            // Only presolving methods which have non-negative priority will be
            // called before constraint handlers.
            if prioprop < 0 {
                break;
            }

            let prop = scip.set.props_presol[j as usize];
            result = run_prop_presol(scip, prop, *timing)?;
            lastranpresol = false;
            j += 1;
        }
        // Call next presolver.
        else {
            // Only presolving methods which have non-negative priority will be
            // called before constraint handlers.
            if priopresol < 0 {
                break;
            }

            let presol = scip.set.presols[i as usize];
            result = run_presol_exec(scip, presol, *timing)?;
            lastranpresol = true;
            i += 1;
        }

        if result == ScipResultCode::Cutoff {
            *infeasible = true;
        } else if result == ScipResultCode::Unbounded {
            *unbounded = true;
        }

        // Delete the variables from the problems that were marked to be deleted.
        crate::prob::prob_perform_var_deletions(scip, TRANSPROB)?;

        // If we work off the exhaustive presolvers, we stop immediately if a
        // reduction was found.
        if *timing == PRESOLTIMING_EXHAUSTIVE && !lastround && !api::is_presolve_finished(scip) {
            if lastranpresol {
                st.presolstart = i + 1;
                st.propstart = j;
            } else {
                st.presolstart = i;
                st.propstart = j + 1;
            }
            aborted = true;

            break;
        }
    }

    // Call presolve methods of constraint handlers.
    while k < consend && !*unbounded && !*infeasible && !aborted {
        let conshdlr = scip.set.conshdlrs[k as usize];
        result = run_conshdlr_presolve(scip, conshdlr, *timing)?;
        k += 1;

        if result == ScipResultCode::Cutoff {
            *infeasible = true;
        } else if result == ScipResultCode::Unbounded {
            *unbounded = true;
        }

        // Delete the variables from the problems that were marked to be deleted.
        crate::prob::prob_perform_var_deletions(scip, TRANSPROB)?;

        // If we work off the exhaustive presolvers, we stop immediately if a
        // reduction was found.
        if *timing == PRESOLTIMING_EXHAUSTIVE && !lastround && !api::is_presolve_finished(scip) {
            st.presolstart = i;
            st.propstart = j;
            st.consstart = k + 1;
            aborted = true;

            break;
        }
    }

    // Call included presolvers with negative priority.
    while !*unbounded && !*infeasible && !aborted && (i < presolend || j < propend) {
        let priopresol = if i < scip.set.npresols { presol_prio(scip, i) } else { -i32::MAX };
        let prioprop = if j < scip.set.nprops { prop_prio(scip, j) } else { -i32::MAX };

        // Choose presolving.
        if prioprop >= priopresol {
            let prop = scip.set.props_presol[j as usize];
            result = run_prop_presol(scip, prop, *timing)?;
            lastranpresol = false;
            j += 1;
        } else {
            let presol = scip.set.presols[i as usize];
            result = run_presol_exec(scip, presol, *timing)?;
            lastranpresol = true;
            i += 1;
        }

        if result == ScipResultCode::Cutoff {
            *infeasible = true;
        } else if result == ScipResultCode::Unbounded {
            *unbounded = true;
        }

        // Delete the variables from the problems that were marked to be deleted.
        crate::prob::prob_perform_var_deletions(scip, TRANSPROB)?;

        // If we work off the exhaustive presolvers, we stop immediately if a
        // reduction was found.
        if *timing == PRESOLTIMING_EXHAUSTIVE && !lastround && !api::is_presolve_finished(scip) {
            if lastranpresol {
                st.presolstart = i + 1;
                st.propstart = j;
            } else {
                st.presolstart = i;
                st.propstart = j + 1;
            }
            st.consstart = k;

            break;
        }
    }

    // Remove empty and single variable cliques from the clique table.
    if !*unbounded && !*infeasible {
        let mut nlocalbdchgs = 0;

        crate::implics::cliquetable_cleanup(scip, TRANSPROB, ORIGPROB, &mut nlocalbdchgs, infeasible)?;

        scip.stat.npresolfixedvars += nlocalbdchgs;

        // Do not call heuristics during presolving on a benders decomposition
        // because the cost information of the retransformed original solutions
        // would be incomplete.
        if !*infeasible && scip.set.nheurs > 0 && scip.set.nactivebenders == 0 {
            // Call primal heuristics that are applicable during presolving.
            let (_foundsol, unbd) = crate::solve::primal_heuristics(scip, TRANSPROB, TRANSPRIMAL, false, false, NONE, crate::heur::HEURTIMING_DURINGPRESOLLOOP, false)?;
            *unbounded = unbd;
        }
    }

    if !*unbounded && !*infeasible {
        // Call more expensive presolvers.
        if api::is_presolve_finished(scip) || lastround {
            if *timing != PRESOLTIMING_FINAL {
                // Increase timing.
                *timing = if *timing == PRESOLTIMING_FAST {
                    PRESOLTIMING_MEDIUM
                } else if *timing == PRESOLTIMING_MEDIUM {
                    PRESOLTIMING_EXHAUSTIVE
                } else {
                    PRESOLTIMING_FINAL
                };

                // Computational experiments showed that always starting the loop of
                // exhaustive presolvers from the beginning performs better than
                // continuing from the last processed presolver. Therefore, we start
                // from 0, but keep the mechanisms to possibly change this back later.
                st.presolstart = 0;
                st.propstart = 0;
                st.consstart = 0;

                presolve_round(scip, timing, unbounded, infeasible, lastround, st, presolend, propend, consend)?;
            }
        }
    }

    // Issue PRESOLVEROUND event.
    let event = Event {
        data: EventData::default(),
        eventtype: EVENTTYPE_PRESOLVEROUND,
    };
    crate::event::event_process(scip, &event, GLOBALEVENTFILTER)?;

    Ok(())
}

/// `presolve`: loops through the included presolvers and constraint's
/// presolve methods, until changes are too few; returns `(unbounded,
/// infeasible, vanished)`.
fn presolve(scip: &mut Scip) -> ScipResult<(bool, bool, bool)> {
    let mut unbounded = false;
    let mut vanished;
    let mut st = RoundStarts {
        presolstart: 0,
        propstart: 0,
        consstart: 0,
    };

    // Switch status to unknown.
    scip.stat.status = Status::Unknown;

    // Update upper bound and cutoff bound due to objective limit in primal
    // data.
    crate::primal::primal_update_objlimit(scip, TRANSPRIMAL, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;

    // Start presolving timer.
    let enabled = scip.set.time_enabled;
    scip.stat.presolvingtime.start(enabled);
    scip.stat.presolvingtimeoverall.start(enabled);

    // Initialize presolving.
    if scip.set.stage == Stage::Transformed {
        init_presolve(scip)?;
    }

    // Call primal heuristics that are applicable before presolving but not on
    // a benders decomposition because the cost information of the
    // retransformed original solutions would be incomplete.
    if scip.set.nheurs > 0 && scip.set.nactivebenders == 0 {
        let (_foundsol, unbd) = crate::solve::primal_heuristics(scip, TRANSPROB, TRANSPRIMAL, false, false, NONE, crate::heur::HEURTIMING_BEFOREPRESOL, false)?;
        unbounded = unbd;
    }

    let mut infeasible = false;
    if !unbounded && api::get_n_sols(scip) > 0 {
        let best = api::get_best_sol(scip);
        let obj = api::get_sol_orig_obj(scip, best);
        unbounded = scip.set.is_infinity(-obj);
    }
    let t = &scip.probs[TRANSPROB];
    vanished = t.nvars == 0 && t.nconss == 0 && scip.set.nactivepricers == 0;

    let mut finished =
        (scip.set.presol_maxrounds != -1 && scip.stat.npresolrounds >= scip.set.presol_maxrounds) || unbounded || vanished || (scip.set.reopt_enable && scip.stat.nreoptruns >= 1);

    // Abort if time limit was reached or user interrupted.
    let mut stopped = crate::solve::solve_is_stopped(scip, false);

    // Perform presolving rounds.
    while !finished && !stopped {
        // Store current number of reductions.
        let s = &mut scip.stat;
        s.lastnpresolfixedvars = s.npresolfixedvars;
        s.lastnpresolaggrvars = s.npresolaggrvars;
        s.lastnpresolchgvartypes = s.npresolchgvartypes;
        s.lastnpresolchgbds = s.npresolchgbds;
        s.lastnpresoladdholes = s.npresoladdholes;
        s.lastnpresoldelconss = s.npresoldelconss;
        s.lastnpresoladdconss = s.npresoladdconss;
        s.lastnpresolupgdconss = s.npresolupgdconss;
        s.lastnpresolchgcoefs = s.npresolchgcoefs;
        s.lastnpresolchgsides = s.npresolchgsides;

        // Set presolving flag.
        s.performpresol = true;

        // Sort propagators.
        crate::prop::set_sort_props_presol(scip);

        // Sort presolvers by priority.
        crate::presol::set_sort_presols(scip);

        // Check if this will be the last presolving round (in that case, we want
        // to run all presolvers).
        let lastround = if scip.set.presol_maxrounds == -1 {
            false
        } else {
            scip.stat.npresolrounds + 1 >= scip.set.presol_maxrounds
        };

        let mut presoltiming = PRESOLTIMING_FAST;

        // Perform the presolving round by calling the presolvers, propagators,
        // and constraint handlers.
        let (npresols, nprops, nconshdlrs) = (scip.set.npresols, scip.set.nprops, scip.set.nconshdlrs);
        presolve_round(scip, &mut presoltiming, &mut unbounded, &mut infeasible, lastround, &mut st, npresols, nprops, nconshdlrs)?;

        // Check, if we should abort presolving due to not enough changes in the
        // last round.
        finished = api::is_presolve_finished(scip) || presoltiming == PRESOLTIMING_FINAL;

        // Check whether problem is infeasible or unbounded or vanished.
        let t = &scip.probs[TRANSPROB];
        vanished = t.nvars == 0 && t.nconss == 0 && scip.set.nactivepricers == 0;
        finished = finished || unbounded || infeasible || vanished;

        // Increase round number.
        scip.stat.npresolrounds += 1;

        // Abort if time limit was reached or user interrupted.
        stopped = crate::solve::solve_is_stopped(scip, false);
    }

    // Flatten aggregation graph in order to avoid complicated multi-aggregated
    // variables (has to happen before checking the empty solution below,
    // because there exist instances for which all variable are
    // multi-aggregated, but in a very complicated manner).
    if !infeasible && !unbounded {
        for v in (0..scip.probs[TRANSPROB].nfixedvars).rev() {
            let var = scip.probs[TRANSPROB].fixedvars[v];
            if scip.vars[var].varstatus == VarStatus::MultAggr {
                crate::var::var_flatten_aggregation_graph(scip, var)?;
            }
        }
    }

    // First change status of scip, so that all plugins in their exitpre
    // callbacks can ask SCIP for the correct status.
    if infeasible {
        // Switch status to OPTIMAL or INFEASIBLE.
        scip.stat.status = if scip.primals[TRANSPRIMAL].nlimsolsfound > 0 {
            Status::Optimal
        } else {
            Status::Infeasible
        };
    } else if unbounded {
        // Switch status to UNBOUNDED or INFORUNBD.
        scip.stat.status = if scip.primals[TRANSPRIMAL].nsols >= 1 { Status::Unbounded } else { Status::InfOrUnbd };
    }
    // If no variables and constraints are present, we try to add the empty
    // solution (constraint handlers with needscons flag FALSE could
    // theoretically reject it); if no active pricers could create variables
    // later, we conclude optimality or infeasibility.
    else if scip.probs[TRANSPROB].nvars == 0 && scip.probs[TRANSPROB].nconss == 0 {
        let sol = api::create_sol(scip, NONE)?;
        api::try_sol_free(scip, sol, false, false, false, false, false)?;

        if scip.set.nactivepricers == 0 {
            scip.stat.status = if scip.primals[TRANSPRIMAL].nlimsolsfound > 0 {
                Status::Optimal
            } else {
                Status::Infeasible
            };
        }
    }

    // Deinitialize presolving.
    if finished && (!stopped || unbounded || infeasible || vanished) {
        let infeas = exit_presolve(scip, unbounded || infeasible || vanished)?;
        infeasible = infeasible || infeas;

        // Resort variables if we are not already done (unless variable
        // permutation was explicitly activated).
        if !scip.set.random_permutevars && !infeasible && !unbounded && !vanished {
            // (Re)Sort the variables, which appear in the four categories (binary,
            // integer, implicit, continuous) after presolve with respect to their
            // original index (within their categories). Adjust the problem index
            // afterwards which is supposed to reflect the position in the variable
            // array. This additional (re)sorting is supposed to get more robust
            // against the order presolving fixed variables. (We also reobtain a
            // possible block structure induced by the user model)
            crate::prob::prob_resort_vars(scip, TRANSPROB);
        }

        // Determine number of non-zeros.
        let (_nchecknonzeros, nactivenonzeros, _, _) = calc_non_zeros(scip)?;
        scip.stat.nnz = nactivenonzeros;
    }

    // Stop presolving time.
    let enabled = scip.set.time_enabled;
    scip.stat.presolvingtime.stop(enabled);
    scip.stat.presolvingtimeoverall.stop(enabled);

    // Remember number of constraints.
    crate::prob::prob_mark_nconss(scip, TRANSPROB);

    Ok((unbounded, infeasible, vanished))
}

/// `transformSols`: tries to transform original solutions to the
/// transformed problem space.
fn transform_sols(scip: &mut Scip) -> ScipResult {
    let nsols = api::get_n_sols(scip);
    let oldnsolsfound = scip.primals[TRANSPRIMAL].nsolsfound;

    // No solution to transform.
    if nsols == 0 {
        return Ok(());
    }

    let mut naddedsols = 0;

    // It might happen, that the added transferred solution does not equal the
    // corresponding original one, which might result in the array of solutions
    // being changed. Thus we temporarily copy the array and traverse it in
    // reverse order to ensure that the regarded solution in the copied array
    // was not already freed when new solutions were added and the worst
    // solutions were freed.
    let sols = api::get_sols(scip).to_vec();

    for &sol in sols.iter().rev() {
        // It might happen that a transferred original solution has a better
        // objective than its original counterpart (e.g., because
        // multi-aggregated variables get another value, but the solution is still
        // feasible); in this case, it might happen that the solution is not an
        // original one and we just skip this solution.
        if !crate::sol::sol_is_original(scip, sol) {
            continue;
        }

        let added = crate::primal::primal_transform_sol(scip, TRANSPRIMAL, sol, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER)?;

        if added {
            naddedsols += 1;
        }
    }

    if naddedsols > 0 {
        scip.stat.nexternalsolsfound += scip.primals[TRANSPRIMAL].nsolsfound - oldnsolsfound;
    }

    Ok(())
}

/// `initSolve`: initializes solution process data structures.
fn init_solve(scip: &mut Scip, solved: bool) -> ScipResult {
    // If problem has been solved, several time consuming tasks must not be
    // performed.
    if !solved {
        // Reset statistics for current branch and bound run.
        crate::stat::stat_reset_current_run(scip, solved);
        crate::stat::stat_enforce_lp_updates(scip);

        // LP is empty anyway; mark empty LP to be solved and update validsollp
        // counter.
        crate::lp::lp_reset(scip, TRANSPROB)?;

        // Update upper bound and cutoff bound due to objective limit in primal
        // data.
        crate::primal::primal_update_objlimit(scip, TRANSPRIMAL, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;
    }

    // Switch stage to INITSOLVE.
    scip.set.stage = Stage::InitSolve;

    // Initialize NLP if there are nonlinearities.
    if scip.probs[TRANSPROB].nlpenabled && !scip.set.nlp_disable {
        let name = scip.probs[TRANSPROB].name.clone();
        crate::nlp::nlp_create(scip, &name)?;

        let nvars = scip.probs[TRANSPROB].nvars as usize;
        let vars = scip.probs[TRANSPROB].vars.0[..nvars].to_vec();
        crate::nlp::nlp_add_vars_public(scip, &vars)?;

        // Adjust estimation of external memory: SCIPtransformProb() estimated the
        // memory used for the LP-solver. As a very crude approximation just double
        // this number. Only do this once in the first run.
        if scip.set.misc_estimexternmem && scip.stat.nruns <= 1 {
            scip.stat.externmemestim *= 2;
        }
    }

    // Initialize solution process data structures.
    crate::pricestore::pricestore_create(scip);
    scip.sepastores = vec![crate::sepastore::sepastore_create(scip), crate::sepastore::sepastore_create(scip)];
    let agelimit = scip.set.sepa_cutagelimit;
    scip.cutpools = vec![crate::cutpool::cutpool_create(scip, agelimit, true), crate::cutpool::cutpool_create(scip, agelimit, false)];
    crate::tree::tree_create_root(scip, GLOBALEVENTFILTER)?;

    // Try to transform original solutions to the transformed problem space.
    if scip.set.misc_transorigsols {
        transform_sols(scip)?;
    }

    // Restore lower bound of the root node if a valid dual bound is at hand.
    if scip.probs[TRANSPROB].dualbound != crate::def::INVALID {
        let dualbound = scip.probs[TRANSPROB].dualbound;
        let lb = crate::prob::prob_intern_objval(scip, TRANSPROB, ORIGPROB, dualbound);
        let root = treer!(scip).root;
        scip.nodes[root].lowerbound = lb;
        scip.nodes[root].estimate = lb;
        scip.stat.rootlowerbound = lb;

        // Throw improvement event.
        let event = Event {
            data: EventData::default(),
            eventtype: EVENTTYPE_DUALBOUNDIMPROVED,
        };
        crate::event::event_process(scip, &event, GLOBALEVENTFILTER)?;

        // Update primal-dual integrals.
        if scip.set.misc_calcintegral {
            let inf = scip.set.infinity();
            crate::stat::stat_update_primal_dual_integrals(scip, TRANSPROB, ORIGPROB, inf, lb);
        } else {
            scip.stat.lastlowerbound = lb;
        }
    }

    // Inform the transformed problem that the branch and bound process starts
    // now.
    crate::prob::prob_init_solve(scip, TRANSPROB)?;

    // Inform plugins that the branch and bound process starts now.
    crate::set::set_initsol_plugins(scip)?;

    // Remember number of constraints.
    crate::prob::prob_mark_nconss(scip, TRANSPROB);

    // If all variables are known, calculate a trivial primal bound by setting
    // all variables to their worst bound.
    if scip.set.nactivepricers == 0 {
        let mut objbound = 0.0;
        let mut v = 0;
        while v < scip.probs[TRANSPROB].nvars && !scip.set.is_infinity(objbound) {
            let var = scip.probs[TRANSPROB].vars[v];
            let obj = scip.vars[var].obj;
            if !scip.set.is_zero(obj) {
                let bd = crate::var::var_get_worst_bound_global(scip, var);
                if scip.set.is_infinity(bd.abs()) {
                    objbound = scip.set.infinity();
                } else {
                    objbound += obj * bd;
                }
            }
            v += 1;
        }

        // Adjust primal bound, such that solution with worst bound may be found.
        let delta = scip.set.cutoffbounddelta();
        if objbound + delta != objbound {
            objbound += delta;
        }
        // If objbound is very large, adding the cutoffbounddelta may not change
        // the number; in this case, we are using SCIPnextafter to ensure that the
        // cutoffbound is really larger than the best possible solution value.
        else {
            objbound = crate::misc::nextafter(objbound, crate::def::REAL_MAX);
        }

        // Update cutoff bound.
        if !scip.set.is_infinity(objbound) && scip.set.is_lt(objbound, scip.primals[TRANSPRIMAL].cutoffbound) {
            // Adjust cutoff bound.
            crate::primal::primal_set_cutoffbound(scip, TRANSPRIMAL, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, objbound, false)?;
        }
    }

    // Switch stage to SOLVING.
    scip.set.stage = Stage::Solving;

    Ok(())
}

/// `freeSolve`: frees solution process data structures.
fn free_solve(scip: &mut Scip, restart: bool) -> ScipResult {
    // Mark that we are currently restarting.
    if restart {
        scip.stat.inrestart = true;

        // Copy the current dual bound into the problem data structure such that
        // it can be used initialize the new search tree.
        let dualbound = api::get_dualbound(scip);
        crate::prob::prob_update_dualbound(scip, TRANSPROB, dualbound);
    }

    // Remove focus from the current focus node.
    if crate::tree::tree_get_focus_node(scip) != NONE {
        let mut node = NONE;
        let mut cutoff = false;

        crate::tree::node_focus(scip, &mut node, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, &mut cutoff, false, true)?;
    }

    // Switch stage to EXITSOLVE.
    scip.set.stage = Stage::ExitSolve;

    // Cleanup the conflict storage.
    crate::conflictstore::conflictstore_clean(scip, TRANSPROB)?;

    // Inform plugins that the branch and bound process is finished.
    crate::set::set_exitsol_plugins(scip, restart)?;

    // Free the NLP, if there is one, and reset the flags indicating
    // nonlinearity.
    if scip.nlp.is_some() {
        crate::nlp::nlp_free(scip)?;
    }
    scip.probs[TRANSPROB].nlpenabled = false;

    // Clear the LP, and flush the changes to clear the LP of the solver.
    crate::lp::lp_reset(scip, TRANSPROB)?;
    crate::lp::lp_invalidate_root_objval(scip);

    // Clear all row references in internal data structures.
    crate::cutpool::cutpool_clear(scip, 0)?;
    crate::cutpool::cutpool_clear(scip, 1)?;

    // We have to clear the tree prior to the problem deinitialization, because
    // the rows stored in the forks and subroots have to be released.
    crate::tree::tree_clear(scip, GLOBALEVENTFILTER)?;

    // Deinitialize transformed problem.
    crate::prob::prob_exit_solve(scip, TRANSPROB, restart)?;

    // Free solution process data structures.
    crate::cutpool::cutpool_clear(scip, 0)?;
    crate::cutpool::cutpool_clear(scip, 1)?;
    scip.cutpools.clear();
    scip.sepastores.clear();
    scip.pricestore = None;

    // Reset statistics for current branch and bound run.
    let st = scip.stat.status;
    let solved = st == Status::Infeasible || st == Status::Optimal || st == Status::Unbounded || st == Status::InfOrUnbd;
    crate::stat::stat_reset_current_run(scip, solved);

    // Switch stage to TRANSFORMED.
    scip.set.stage = Stage::Transformed;

    // Restart finished.
    scip.stat.inrestart = false;

    Ok(())
}

/// `freeTransform`: frees transformed problem.
fn free_transform(scip: &mut Scip) -> ScipResult {
    // Call exit methods of plugins.
    crate::set::set_exit_plugins(scip)?;

    // Copy best primal solutions to original solution candidate list but not
    // for a benders decomposition because their cost information would be
    // incomplete.
    if !scip.set.reopt_enable && scip.set.limit_maxorigsol > 0 && scip.set.misc_transsolsorig && scip.set.nactivebenders == 0 {
        let nsols = scip.primals[TRANSPRIMAL].nsols;
        let maxsols = scip.set.limit_maxorigsol;
        let mut s = 0;

        // Iterate over all solutions as long as the original solution candidate
        // store size limit is not reached.
        while s < nsols && scip.primals[ORIGPRIMAL].nsols < maxsols {
            let sol = scip.primals[TRANSPRIMAL].sols[s];

            let mut hasinfval = if !crate::sol::sol_is_original(scip, sol) {
                // Retransform solution into the original problem space.
                crate::sol::sol_retransform(scip, sol, ORIGPROB, TRANSPROB)?
            } else {
                false
            };

            // Removing infinite fixings is turned off by the corresponding
            // parameter.
            if !scip.set.misc_finitesolstore {
                hasinfval = false;
            }

            if !hasinfval {
                // Add solution to original candidate solution storage.
                crate::primal::primal_add_orig_sol(scip, ORIGPRIMAL, ORIGPROB, sol)?;
            } else {
                unimplemented!("SCIPcreateFiniteSolCopy (misc/finitesolutionstore)");
            }

            s += 1;
        }
    }

    // Switch stage to FREETRANS.
    scip.set.stage = Stage::FreeTrans;

    // Clear the conflict store; since the conflict store can contain
    // transformed constraints we need to remove them. The store will be
    // finally freed in SCIPfreeProb().
    crate::conflictstore::conflictstore_clear(scip)?;

    // Free transformed problem data structures.
    crate::prob::prob_free(scip, TRANSPROB)?;
    crate::implics::cliquetable_free(scip);
    scip.conflict = None;
    scip.relaxation = Default::default();
    crate::tree::tree_free(scip, GLOBALEVENTFILTER)?;
    crate::primal::primal_clear(scip, TRANSPRIMAL)?;
    scip.primals.remove(TRANSPRIMAL);
    crate::lp::lp_free(scip)?;
    scip.branchcand = Default::default();
    crate::event::eventfilter_free(scip, GLOBALEVENTFILTER)?;
    scip.eventqueue = Default::default();

    if scip.set.misc_resetstat {
        // Reset statistics to the point before the problem was transformed.
        crate::stat::stat_reset(scip);
    } else {
        // Even if statistics are not completely reset, a partial reset of the
        // primal-dual integral is necessary.
        crate::stat::stat_reset_primal_dual_integrals(scip, true);
    }

    // Switch stage to PROBLEM.
    scip.set.stage = Stage::Problem;

    // Reset objective limit.
    api::set_objlimit(scip, crate::def::INVALID)?;

    // Reset original variable's local and global bounds to their original
    // values.
    crate::prob::prob_reset_bounds(scip, ORIGPROB)?;

    Ok(())
}

/*
 * Public methods
 */

/// `SCIPpresolve`: transforms and presolves problem.
pub fn presolve_prob(scip: &mut Scip) -> ScipResult {
    // Start solving timer.
    let enabled = scip.set.time_enabled;
    scip.stat.solvingtime.start(enabled);
    scip.stat.solvingtimeoverall.start(enabled);

    // Reset the user interrupt flag.
    scip.stat.userinterrupt = false;
    api::interrupt_lp(scip, false)?;

    match scip.set.stage {
        Stage::Problem | Stage::Transformed | Stage::Presolving => {
            if scip.set.stage == Stage::Problem {
                // Initialize solving data structures and transform problem.
                transform_prob(scip)?;
            }

            // Presolve problem.
            let (unbounded, infeasible, vanished) = presolve(scip)?;

            if infeasible || unbounded || vanished {
                // Initialize solving process data structures to be able to switch to
                // SOLVED stage.
                init_solve(scip, true)?;

                // Switch stage to SOLVED.
                scip.set.stage = Stage::Solved;

                match scip.stat.status {
                    Status::Optimal => {
                        // Remove the root node from the tree, s.t. the lower bound is set to
                        // +infinity (see initSolve()).
                        crate::tree::tree_clear(scip, GLOBALEVENTFILTER)?;
                    }
                    Status::Infeasible | Status::Unbounded | Status::InfOrUnbd => {}
                    _ => {
                        // Note that this is in an internal SCIP error since the status is
                        // corrupted.
                        return Err(Retcode::Error);
                    }
                }
            }
        }
        Stage::Presolved | Stage::Solved => {}
        _ => {
            // Invalid SCIP stage.
            return Err(Retcode::InvalidCall);
        }
    }

    // Stop solving timer.
    let enabled = scip.set.time_enabled;
    scip.stat.solvingtime.stop(enabled);
    scip.stat.solvingtimeoverall.stop(enabled);

    Ok(())
}

/// `SCIPsolve`: transforms, presolves, and solves problem.
pub fn solve(scip: &mut Scip) -> ScipResult {
    let mut cutpoolncutsfoundbeforerestart: i64 = 0;
    let mut cutpoolncutsaddedbeforerestart: i64 = 0;
    let mut cutpoolncallsbeforerestart: i64 = 0;
    let mut cutpoolnrootcallsbeforerestart: i64 = 0;
    let mut cutpoolmaxncutsbeforerestart: i64 = 0;
    let mut cutpooltimebeforerestart = 0.0;
    let mut transferstatistics = false;

    // If the stage is already SCIP_STAGE_SOLVED do nothing.
    if scip.set.stage == Stage::Solved {
        return Ok(());
    }

    let st = scip.stat.status;
    if st == Status::Infeasible || st == Status::Optimal || st == Status::Unbounded || st == Status::InfOrUnbd {
        log::warn!("SCIPsolve() was called, but problem is already solved");
        return Ok(());
    }

    // Check, if a node selector exists.
    if crate::nodesel::set_get_nodesel(scip) == NONE {
        // No node selector available.
        return Err(Retcode::PluginNotFound);
    }

    // Check, if an integrality constraint handler exists if there are integral
    // variables.
    if crate::set::set_find_conshdlr(scip, "integral") == NONE {
        log::warn!("integrality constraint handler not available");
    }

    // Initialize presolving flag (may be modified in SCIPpresolve()).
    scip.stat.performpresol = false;

    // Start solving timer.
    let enabled = scip.set.time_enabled;
    scip.stat.solvingtime.start(enabled);
    scip.stat.solvingtimeoverall.start(enabled);

    // Reset the user interrupt flag.
    scip.stat.userinterrupt = false;
    api::interrupt_lp(scip, false)?;

    // Automatic restarting loop.
    let mut restart = scip.stat.userrestart;

    loop {
        if restart {
            transferstatistics = true;
            let cp = &scip.cutpools[0];
            cutpoolncutsfoundbeforerestart = cp.ncutsfound;
            cutpoolncutsaddedbeforerestart = cp.ncutsadded;
            cutpooltimebeforerestart = cp.poolclock.get_time();
            cutpoolncallsbeforerestart = cp.ncalls;
            cutpoolnrootcallsbeforerestart = cp.nrootcalls;
            cutpoolmaxncutsbeforerestart = cp.maxncuts;

            // Reset relaxation solution, so that the objective value is recomputed
            // from scratch next time, using the new transformed problem.
            api::clear_relax_sol_vals(scip, NONE)?;

            // Free the solving process data in order to restart.
            free_solve(scip, true)?;
        }
        restart = false;
        scip.stat.userrestart = false;

        let mut presolved_now = false;
        match scip.set.stage {
            Stage::Problem | Stage::Transformed | Stage::Presolving => {
                // Initialize solving data structures, transform and problem.
                presolve_prob(scip)?;

                if scip.set.stage == Stage::Solved || scip.set.stage == Stage::Presolving {
                    if scip.set.reopt_enable {
                        unimplemented!("reoptimization");
                    }
                } else if !crate::solve::solve_is_stopped(scip, true) {
                    // Abort if a node limit was reached; otherwise fall through.
                    presolved_now = true;
                }
            }
            _ => {}
        }

        let stage = scip.set.stage;
        if presolved_now || stage == Stage::Presolved || stage == Stage::Solving {
            if presolved_now || stage == Stage::Presolved {
                // Check if reoptimization is enabled and global constraints are saved.
                if scip.set.reopt_enable {
                    unimplemented!("reoptimization");
                }

                // Initialize solving process data structures.
                init_solve(scip, false)?;
            }

            // Reset display.
            crate::stat::stat_reset_display(scip);

            // Remember cutpool statistics after restart.
            if transferstatistics {
                let cp = &mut scip.cutpools[0];
                cp.ncutsfound += cutpoolncutsfoundbeforerestart;
                cp.ncutsadded += cutpoolncutsaddedbeforerestart;
                cp.poolclock.set_time(cutpooltimebeforerestart);
                cp.ncalls += cutpoolncallsbeforerestart;
                cp.nrootcalls += cutpoolnrootcallsbeforerestart;
                cp.maxncuts += cutpoolmaxncutsbeforerestart;
            }

            // Continue solution process.
            restart = crate::solve::solve_cip(scip)?;

            // Detect, whether problem is solved.
            if crate::tree::tree_get_n_nodes(scip) == 0 && crate::tree::tree_get_current_node(scip) == NONE {
                // Tree is empty, and no current node exists -> problem is solved.
                scip.set.stage = Stage::Solved;
            }
        } else if stage != Stage::Solved && !matches!(stage, Stage::Problem | Stage::Transformed | Stage::Presolving) {
            // Invalid SCIP stage.
            return Err(Retcode::InvalidCall);
        }

        if !(restart && !crate::solve::solve_is_stopped(scip, true)) {
            break;
        }
    }

    // Stop solving timer.
    let enabled = scip.set.time_enabled;
    scip.stat.solvingtime.stop(enabled);
    scip.stat.solvingtimeoverall.stop(enabled);

    Ok(())
}

/// `SCIPfreeSolve`: frees branch and bound tree and all solution process
/// data; statistics, presolving data and transformed problem is preserved.
pub fn free_solve_public(scip: &mut Scip, restart: bool) -> ScipResult {
    match scip.set.stage {
        Stage::Init | Stage::Transformed | Stage::Problem => Ok(()),
        Stage::Presolving | Stage::Presolved => {
            if scip.set.stage == Stage::Presolving {
                // Exit presolving.
                exit_presolve(scip, false)?;
            }

            // Switch stage to TRANSFORMED.
            scip.set.stage = Stage::Transformed;
            Ok(())
        }
        Stage::Solving | Stage::Solved => {
            // Free solution process data structures.
            free_solve(scip, restart)
        }
        _ => {
            // Invalid SCIP stage.
            Err(Retcode::InvalidCall)
        }
    }
}

/// `SCIPfreeTransform`: frees all solution process data including
/// presolving and transformed problem, only original problem is kept.
pub fn free_transform_public(scip: &mut Scip) -> ScipResult {
    match scip.set.stage {
        Stage::Init | Stage::Problem => Ok(()),
        Stage::Presolving | Stage::Presolved | Stage::Solving | Stage::Solved | Stage::Transformed => {
            if scip.set.stage == Stage::Presolving {
                // Exit presolving.
                exit_presolve(scip, false)?;
            }

            if scip.set.stage != Stage::Transformed {
                // Free solution process data.
                free_solve_public(scip, false)?;
            }

            // Free transformed problem data structures.
            free_transform(scip)
        }
        Stage::Transforming => unimplemented!("freeTransforming (an error during transformation)"),
        _ => {
            // Invalid SCIP stage.
            Err(Retcode::InvalidCall)
        }
    }
}

/// `SCIPinterruptSolve`: informs SCIP that the solving process should be
/// interrupted as soon as possible (e.g., after the current node has been
/// solved).
pub fn interrupt_solve(scip: &mut Scip) {
    // Set the userinterrupt flag.
    scip.stat.userinterrupt = true;
}

/// `SCIPisSolveInterrupted`: indicates whether SCIP has been informed that
/// the solving process should be interrupted as soon as possible.
pub fn is_solve_interrupted(scip: &Scip) -> bool {
    scip.stat.userinterrupt
}

/// `SCIPrestartSolve`: informs SCIP that the solving process should be
/// restarted as soon as possible (e.g., after the current node has been
/// solved).
pub fn restart_solve(scip: &mut Scip) {
    // Set the userrestart flag.
    scip.stat.userrestart = true;
}
