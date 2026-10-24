//! SCIP's primal data (`primal.c`, `struct_primal.h`): the incumbent and
//! the solution storage.

use crate::{cvec::CVec, types::SolId};

/// `SCIP_PRIMAL`.
#[derive(Clone, Debug, Default)]
pub struct Primal {
    pub nsolsfound: i64,
    pub nlimsolsfound: i64,
    pub nbestsolsfound: i64,
    pub nlimbestsolsfound: i64,
    pub upperbound: f64,
    pub cutoffbound: f64,
    pub sols: CVec<SolId>,
    pub partialsols: CVec<SolId>,
    pub existingsols: CVec<SolId>,
    pub currentsol: SolId,
    pub primalray: SolId,
    pub solssize: i32,
    pub partialsolssize: i32,
    pub nsols: i32,
    pub npartialsols: i32,
    pub existingsolssize: i32,
    pub nexistingsols: i32,
    pub updateviolations: bool,
}

use crate::{
    event, lp, prob,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    sol,
    types::{HeurId, ObjSense, SolOrigin, VarId},
    var::NONE,
};

/*
 * memory growing methods for dynamically allocated arrays
 */

/// `ensureSolsSize`: ensures, that sols array can store at least `num`
/// entries.
fn ensure_sols_size(scip: &mut Scip, p: usize, num: i32) {
    if num > scip.primals[p].solssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.primals[p].sols.0.resize(newsize as usize, NONE);
        scip.primals[p].solssize = newsize;
    }
}

/// `ensurePartialsolsSize`: ensures, that partialsols array can store at
/// least `num` entries.
fn ensure_partialsols_size(scip: &mut Scip, p: usize, num: i32) {
    if num > scip.primals[p].partialsolssize {
        let newsize = scip.set.calc_mem_grow_size(num).min(scip.set.limit_maxorigsol);
        scip.primals[p].partialsols.0.resize(newsize as usize, NONE);
        scip.primals[p].partialsolssize = newsize;
    }
}

/// `ensureExistingsolsSize`: ensures, that existingsols array can store at
/// least `num` entries.
fn ensure_existingsols_size(scip: &mut Scip, p: usize, num: i32) {
    if num > scip.primals[p].existingsolssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.primals[p].existingsols.0.resize(newsize as usize, NONE);
        scip.primals[p].existingsolssize = newsize;
    }
}

/// `SCIPprimalCreate`: creates primal data.
pub fn primal_create() -> Primal {
    Primal {
        sols: CVec::new(),
        partialsols: CVec::new(),
        existingsols: CVec::new(),
        currentsol: NONE,
        primalray: NONE,
        solssize: 0,
        partialsolssize: 0,
        nsols: 0,
        npartialsols: 0,
        existingsolssize: 0,
        nexistingsols: 0,
        nsolsfound: 0,
        nlimsolsfound: 0,
        nbestsolsfound: 0,
        nlimbestsolsfound: 0,
        upperbound: crate::def::INVALID,
        cutoffbound: crate::def::INVALID,
        updateviolations: true,
    }
}

/// `SCIPprimalClear`: clears primal data.
pub fn primal_clear(scip: &mut Scip, p: usize) -> ScipResult {
    // Free temporary solution for storing current solution.
    let currentsol = scip.primals[p].currentsol;
    if currentsol != NONE {
        sol::sol_free(scip, currentsol, p)?;
    }

    // Free solution for storing primal ray.
    let primalray = scip.primals[p].primalray;
    if primalray != NONE {
        sol::sol_free(scip, primalray, p)?;
    }

    // Free feasible primal CIP solutions.
    for s in 0..scip.primals[p].nsols {
        let sl = scip.primals[p].sols[s];
        sol::sol_free(scip, sl, p)?;
    }

    // Free partial CIP solutions.
    for s in 0..scip.primals[p].npartialsols {
        let sl = scip.primals[p].partialsols[s];
        sol::sol_free(scip, sl, p)?;
    }

    let pr = &mut scip.primals[p];
    pr.currentsol = NONE;
    pr.primalray = NONE;
    pr.nsols = 0;
    pr.nsolsfound = 0;
    pr.npartialsols = 0;
    pr.nlimsolsfound = 0;
    pr.nbestsolsfound = 0;
    pr.nlimbestsolsfound = 0;
    pr.upperbound = crate::def::INVALID;
    pr.cutoffbound = crate::def::INVALID;
    pr.updateviolations = true;
    Ok(())
}

/// `sortPrimalSols`: sorts primal solutions by objective value.
fn sort_primal_sols(scip: &mut Scip, p: usize, origprob: usize, transprob: usize) {
    for i in 1..scip.primals[p].nsols {
        let s = scip.primals[p].sols[i];
        let objval = sol::sol_get_obj(scip, s, transprob, origprob);
        let mut j = i;
        while j > 0 && objval < sol::sol_get_obj(scip, scip.primals[p].sols[j - 1], transprob, origprob) {
            let prev = scip.primals[p].sols[j - 1];
            scip.primals[p].sols[j] = prev;
            j -= 1;
        }
        scip.primals[p].sols[j] = s;
    }
}

/// `primalSetCutoffbound`: sets the cutoff bound in primal data and in LP
/// solver.
fn primal_set_cutoffbound_internal(scip: &mut Scip, p: usize, prob: usize, eventfilter: usize, cutoffbound: f64) -> ScipResult {
    let pr = &mut scip.primals[p];
    pr.cutoffbound = crate::def::min(cutoffbound, pr.upperbound); // get rid of numerical issues
    let cutoffbound = pr.cutoffbound;

    // Set cut off value in LP solver.
    lp::lp_set_cutoffbound(scip, prob, cutoffbound)?;

    // Cut off leaves of the tree.
    crate::tree::tree_cutoff(scip, eventfilter, cutoffbound)?;
    Ok(())
}

/// `SCIPprimalSetCutoffbound`: sets the cutoff bound in primal data and in
/// LP solver; `useforobjlimit` says whether the cutoff bound should be
/// used to update the objective limit, if better.
pub fn primal_set_cutoffbound(scip: &mut Scip, p: usize, transprob: usize, origprob: usize, eventfilter: usize, cutoffbound: f64, useforobjlimit: bool) -> ScipResult {
    if cutoffbound < scip.primals[p].cutoffbound {
        if useforobjlimit {
            let objval = prob::prob_extern_objval(scip, transprob, origprob, cutoffbound);

            if objval < prob::prob_get_objlim(scip, origprob) {
                prob::prob_set_objlim(scip, origprob, objval);
                prob::prob_set_objlim(scip, transprob, objval);
            }
        }

        // Update cutoff bound.
        primal_set_cutoffbound_internal(scip, p, transprob, eventfilter, cutoffbound)?;
    } else if cutoffbound > scip.primals[p].cutoffbound {
        log::error!("invalid increase in cutoff bound");
        return Err(Retcode::InvalidData);
    }
    Ok(())
}

/// `primalSetUpperbound`: sets upper bound in primal data and in LP
/// solver.
fn primal_set_upperbound_internal(scip: &mut Scip, p: usize, prob: usize, eventfilter: usize, upperbound: f64) -> ScipResult {
    scip.primals[p].upperbound = upperbound;

    // If objective value is always integral, the cutoff bound can be reduced
    // to nearly the previous integer number.
    let cutoffbound = if scip.probs[prob].objisintegral && !scip.set.is_infinity(upperbound) {
        let delta = scip.set.cutoffbounddelta();
        let c = scip.set.feas_ceil(upperbound) - (1.0 - delta);
        // SCIPsetFeasCeil() can increase bound by almost 1.0 due to numerics
        // and very large upperbound value.
        crate::def::min(c, upperbound)
    } else {
        upperbound
    };

    // Update cutoff bound.
    if cutoffbound < scip.primals[p].cutoffbound {
        primal_set_cutoffbound_internal(scip, p, prob, eventfilter, cutoffbound)?;
    }
    Ok(())
}

/// `SCIPprimalSetUpperbound`: sets upper bound in primal data and in LP
/// solver.
pub fn primal_set_upperbound(scip: &mut Scip, p: usize, prob: usize, eventfilter: usize, upperbound: f64) -> ScipResult {
    if upperbound < scip.primals[p].upperbound {
        // Update primal bound.
        primal_set_upperbound_internal(scip, p, prob, eventfilter, upperbound)?;
    } else if upperbound > scip.primals[p].upperbound {
        log::error!("invalid increase in upper bound");
        return Err(Retcode::InvalidData);
    }
    Ok(())
}

/// `SCIPprimalUpdateObjlimit`: updates upper bound and cutoff bound in
/// primal data after a tightening of the problem's objective limit.
pub fn primal_update_objlimit(scip: &mut Scip, p: usize, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    // Get internal objective limit.
    let objlim = prob::prob_get_objlim(scip, origprob);
    let objlimit = crate::def::min(prob::prob_intern_objval(scip, transprob, origprob, objlim), scip.set.infinity());

    // Update the cutoff bound.
    if objlimit < scip.primals[p].cutoffbound {
        primal_set_cutoffbound_internal(scip, p, transprob, eventfilter, objlimit)?;
    }

    // Set new upper bound (and decrease cutoff bound, if objective value is
    // always integral).
    if objlimit < scip.primals[p].upperbound {
        primal_set_upperbound_internal(scip, p, transprob, eventfilter, objlimit)?;
    }
    Ok(())
}

/// `SCIPprimalUpdateObjoffset`: recalculates upper bound and cutoff bound
/// in primal data after a change of the problem's objective offset.
pub fn primal_update_objoffset(scip: &mut Scip, p: usize, transprob: usize, origprob: usize) -> ScipResult {
    let eventfilter = crate::scip::GLOBALEVENTFILTER;

    // Recalculate internal objective limit.
    let objlim = prob::prob_get_objlim(scip, origprob);
    let mut upperbound = crate::def::min(prob::prob_intern_objval(scip, transprob, origprob, objlim), scip.set.infinity());

    // Resort current primal solutions.
    sort_primal_sols(scip, p, origprob, transprob);

    // Compare objective limit to currently best solution.
    if scip.primals[p].nsols > 0 {
        let obj = sol::sol_get_obj(scip, scip.primals[p].sols[0_usize], transprob, origprob);
        upperbound = crate::def::min(upperbound, obj);
    }

    // Invalidate old upper bound.
    let inf = scip.set.infinity();
    primal_set_upperbound_internal(scip, p, transprob, eventfilter, inf)?;

    // Reset the cutoff bound.
    //
    // We might need to relax the bound since in presolving the objective
    // correction of an aggregation is still in progress.
    primal_set_cutoffbound_internal(scip, p, transprob, eventfilter, upperbound)?;

    // Set new upper bound (and decrease cutoff bound, if objective value is
    // always integral).
    primal_set_upperbound_internal(scip, p, transprob, eventfilter, upperbound)?;
    Ok(())
}

/// `SCIPprimalAddOrigObjoffset`: adds additional objective offset in
/// original space to all existing solution (in original space).
pub fn primal_add_orig_objoffset(scip: &mut Scip, p: usize, addval: f64) {
    // Check current order of primal solutions.
    for i in 0..scip.primals[p].nexistingsols {
        let s = scip.primals[p].existingsols[i];
        scip.sols[s].obj += addval;
    }
}

/// `SCIPprimalUpperboundIsSol`: returns whether the current primal bound
/// is justified with a feasible primal solution; if not, the primal bound
/// was set from the user as objective limit.
pub fn primal_upperbound_is_sol(scip: &Scip, p: usize, transprob: usize, origprob: usize) -> bool {
    let pr = &scip.primals[p];
    pr.nsols > 0 && scip.set.is_eq(pr.upperbound, sol::sol_get_obj(scip, pr.sols[0_usize], transprob, origprob))
}

/// `SCIPprimalUpdateRay`: update the primal ray thats proves
/// unboundedness.
pub fn primal_update_ray(scip: &mut Scip, p: usize, primalray: usize) -> ScipResult {
    // Clear previously stored primal ray, if any.
    let old = scip.primals[p].primalray;
    if old != NONE {
        sol::sol_free(scip, old, p)?;
    }

    let copy = sol::sol_copy(scip, p, primalray)?;
    scip.primals[p].primalray = copy;
    Ok(())
}

/// `primalAddSol`: adds primal solution to solution storage at given
/// position; the solution moves into the storage.
#[allow(clippy::too_many_arguments)]
fn primal_add_sol_internal(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize, solptr: usize, insertpos: i32, replace: bool) -> ScipResult {
    let mut s = solptr;

    // If the solution is added during presolving and it is not defined on
    // original variables, presolving operations will destroy its validity,
    // so we retransform it to the original space.
    if scip.set.stage < Stage::Presolved && !sol::sol_is_original(scip, s) {
        sol::sol_unlink(scip, s, transprob)?;
        sol::sol_retransform(scip, s, origprob, transprob)?;
    }

    let obj = sol::sol_get_obj(scip, s, transprob, origprob);

    // Make sure that the primal bound is at least the lower bound.
    let lowerbound = crate::api::get_lowerbound(scip);
    if !scip.set.is_infinity(obj) && !scip.set.is_infinity(-lowerbound) && scip.set.is_feas_gt(lowerbound, obj) {
        let lb = prob::prob_extern_objval(scip, transprob, origprob, lowerbound);
        let o = prob::prob_extern_objval(scip, transprob, origprob, obj);
        if scip.probs[origprob].objsense == ObjSense::Minimize {
            log::warn!("Dual bound {lb} is larger than the objective of the primal solution {o}. The solution might not be optimal.");
        } else {
            log::warn!("Dual bound {lb} is smaller than the objective of the primal solution {o}. The solution might not be optimal.");
        }
    }

    // Completely fill the solution's own value array to unlink it from the
    // LP or pseudo solution.
    sol::sol_unlink(scip, s, transprob)?;

    // Allocate memory for solution storage.
    let maxsol = scip.set.limit_maxsol;
    ensure_sols_size(scip, p, maxsol);

    // If set->limit_maxsol was decreased in the meantime, free all solutions
    // exceeding the limit.
    for pos in maxsol..scip.primals[p].nsols {
        let sl = scip.primals[p].sols[pos];
        sol::sol_free(scip, sl, p)?;
    }
    scip.primals[p].nsols = scip.primals[p].nsols.min(maxsol);

    // If the solution should replace an existing one, free this solution,
    // otherwise, free the last solution if the solution storage is full.
    if replace {
        let existing = scip.primals[p].sols[insertpos];
        sol::sol_transform(scip, existing, s, p)?;
        s = existing;
    } else {
        if scip.primals[p].nsols == maxsol {
            let last = scip.primals[p].sols[maxsol - 1];
            sol::sol_free(scip, last, p)?;
        } else {
            scip.primals[p].nsols += 1;
        }

        // Move all solutions with worse objective value than the new
        // solution.
        let pr = &mut scip.primals[p];
        let mut pos = pr.nsols - 1;
        while pos > insertpos {
            pr.sols[pos] = pr.sols[pos - 1];
            pos -= 1;
        }

        // Insert solution at correct position.
        pr.sols[insertpos] = s;
        pr.nsolsfound += 1;

        // Check if solution is better than objective limit.
        let objlim = prob::prob_get_objlim(scip, origprob);
        if scip.set.is_feas_le(obj, prob::prob_intern_objval(scip, transprob, origprob, objlim)) {
            scip.primals[p].nlimsolsfound += 1;
        }
    }

    // If its the first primal solution, store the relevant statistics.
    if scip.primals[p].nsolsfound == 1 {
        let sl = &scip.sols[s];
        let (nodenum, runnum, time, depth) = (sl.nodenum, sl.runnum, sl.time, sl.depth);
        let heur = sol::sol_get_heur(scip, s);
        let firstbound = prob::prob_extern_objval(scip, transprob, origprob, obj);
        let stat = &mut scip.stat;
        stat.nnodesbeforefirst = nodenum;
        stat.nrunsbeforefirst = runnum;
        stat.firstprimalheur = heur;
        stat.firstprimaltime = time;
        stat.firstprimaldepth = depth;
        stat.firstprimalbound = firstbound;
    }

    // Update the solution value sums in variables.
    if !sol::sol_is_original(scip, s) {
        let nsols = scip.primals[p].nsols;
        sol::sol_update_varsum(scip, s, transprob, (nsols - insertpos) as f64 / (2.0 * nsols as f64 - 1.0));
    }

    // Check, if the global upper bound has to be updated.
    if obj < scip.primals[p].cutoffbound && insertpos == 0 {
        // Issue BESTSOLFOUND event.
        let ev = event::event_create_sol(event::EVENTTYPE_BESTSOLFOUND, s);
        event::event_process(scip, &ev, eventfilter)?;

        // Update the upper bound.
        primal_set_upperbound(scip, p, transprob, eventfilter, obj)?;

        scip.primals[p].nbestsolsfound += 1;
        scip.stat.bestsolnode = scip.stat.nnodes;
    } else {
        // Issue POORSOLFOUND event.
        let ev = event::event_create_sol(event::EVENTTYPE_POORSOLFOUND, s);
        event::event_process(scip, &ev, eventfilter)?;
    }

    // If an original solution was added during solving, try to transfer it
    // to the transformed space.
    if sol::sol_is_original(scip, s) && scip.set.stage == Stage::Solving && scip.set.misc_transorigsols {
        primal_transform_sol(scip, p, s, origprob, transprob, eventfilter)?;
    }
    Ok(())
}

/// `primalAddOrigSol`: adds primal solution to solution storage at given
/// position.
fn primal_add_orig_sol_internal(scip: &mut Scip, p: usize, prob: usize, s: usize, insertpos: i32) -> ScipResult {
    let maxorigsol = scip.set.limit_maxorigsol;

    // Allocate memory for solution storage.
    ensure_sols_size(scip, p, maxorigsol);

    // If the solution storage is full, free the last solution(s); more than
    // one solution may be freed, if set->limit_maxorigsol was decreased in
    // the meantime.
    for pos in maxorigsol - 1..scip.primals[p].nsols {
        let sl = scip.primals[p].sols[pos];
        sol::sol_free(scip, sl, p)?;
    }

    // Insert solution at correct position.
    let pr = &mut scip.primals[p];
    pr.nsols = (pr.nsols + 1).min(maxorigsol);
    let mut pos = pr.nsols - 1;
    while pos > insertpos {
        pr.sols[pos] = pr.sols[pos - 1];
        pos -= 1;
    }

    pr.sols[insertpos] = s;
    pr.nsolsfound += 1;

    // Check if solution is better than objective limit.
    if scip.set.is_feas_le(scip.sols[s].obj, prob::prob_get_objlim(scip, prob)) {
        scip.primals[p].nlimsolsfound += 1;
    }
    Ok(())
}

/// `primalAddOrigPartialSol`: adds primal solution to solution storage.
fn primal_add_orig_partial_sol(scip: &mut Scip, p: usize, s: usize) -> ScipResult {
    if scip.primals[p].npartialsols >= scip.set.limit_maxorigsol {
        log::error!("Cannot add partial solution to storage: limit reached.");
        return Err(Retcode::InvalidCall);
    }

    // Allocate memory for solution storage.
    let n = scip.primals[p].npartialsols;
    ensure_partialsols_size(scip, p, n + 1);

    scip.primals[p].partialsols[n] = s;
    scip.primals[p].npartialsols += 1;
    Ok(())
}

/// `primalSearchSolPos`: uses binary search to find position in solution
/// storage.
fn primal_search_sol_pos(scip: &Scip, p: usize, transprob: usize, origprob: usize, s: usize) -> i32 {
    let pr = &scip.primals[p];
    let obj = sol::sol_get_obj(scip, s, transprob, origprob);
    let sols = &pr.sols;

    let mut left = -1;
    let mut right = pr.nsols;
    while left < right - 1 {
        let middle = (left + right) / 2;
        let middleobj = sol::sol_get_obj(scip, sols[middle], transprob, origprob);
        if obj < middleobj {
            right = middle;
        } else {
            left = middle;
        }
    }

    // Prefer solutions that live in the transformed space.
    if !sol::sol_is_original(scip, s) {
        while right > 0 && sol::sol_is_original(scip, sols[right - 1]) && scip.set.is_eq(sol::sol_get_obj(scip, sols[right - 1], transprob, origprob), obj) {
            right -= 1;
        }
    }
    right
}

/// `primalSearchOrigSolPos`: uses binary search to find position in
/// solution storage.
fn primal_search_orig_sol_pos(scip: &Scip, p: usize, s: usize) -> i32 {
    let pr = &scip.primals[p];
    let obj = scip.sols[s].obj;

    let mut left = -1;
    let mut right = pr.nsols;
    while left < right - 1 {
        let middle = (left + right) / 2;
        let middleobj = scip.sols[pr.sols[middle]].obj;
        if obj < middleobj {
            right = middle;
        } else {
            left = middle;
        }
    }
    right
}

/// `primalExistsSol`: returns whether the given primal solution is already
/// existent in the solution storage; `insertpos` might be changed if an
/// existing solution should be replaced.
fn primal_exists_sol(scip: &Scip, p: usize, origprob: usize, transprob: usize, s: usize, insertpos: &mut i32, replace: &mut bool) -> bool {
    let pr = &scip.primals[p];
    let obj = sol::sol_get_obj(scip, s, transprob, origprob);

    // Search in the better solutions.
    let mut i = *insertpos - 1;
    while i >= 0 {
        let solobj = sol::sol_get_obj(scip, pr.sols[i], transprob, origprob);

        // Due to transferring the objective value of transformed solutions to
        // the original space, small numerical errors might occur which can
        // lead to SCIPsetIsLE() failing in case of high absolute numbers.
        if scip.set.is_lt(solobj, obj) {
            break;
        }

        if sol::sols_are_equal(scip, s, pr.sols[i], origprob, transprob) {
            if scip.set.stage >= Stage::Presolved && sol::sol_is_original(scip, pr.sols[i]) && !sol::sol_is_original(scip, s) {
                *insertpos = i;
                *replace = true;
            }
            return true;
        }
        i -= 1;
    }

    // Search in the worse solutions.
    for i in *insertpos..pr.nsols {
        let solobj = sol::sol_get_obj(scip, pr.sols[i], transprob, origprob);

        // Due to transferring the objective value of transformed solutions to
        // the original space, small numerical errors might occur which can
        // lead to SCIPsetIsLE() failing in case of high absolute numbers.
        if scip.set.is_gt(solobj, obj) {
            break;
        }

        if sol::sols_are_equal(scip, s, pr.sols[i], origprob, transprob) {
            if scip.set.stage >= Stage::Presolved && sol::sol_is_original(scip, pr.sols[i]) && !sol::sol_is_original(scip, s) {
                *insertpos = i;
                *replace = true;
            }
            return true;
        }
    }
    false
}

/// `primalExistsOrigSol`: returns whether the given primal solution is
/// already existent in the original solution candidate storage.
fn primal_exists_orig_sol(scip: &Scip, p: usize, prob: usize, s: usize, insertpos: i32) -> bool {
    let pr = &scip.primals[p];
    let obj = scip.sols[s].obj;

    // Search in the better solutions.
    let mut i = insertpos - 1;
    while i >= 0 {
        let solobj = scip.sols[pr.sols[i]].obj;
        if scip.set.is_lt(solobj, obj) {
            break;
        }
        if sol::sols_are_equal(scip, s, pr.sols[i], prob, NONE) {
            return true;
        }
        i -= 1;
    }

    // Search in the worse solutions.
    for i in insertpos..pr.nsols {
        let solobj = scip.sols[pr.sols[i]].obj;
        if scip.set.is_gt(solobj, obj) {
            break;
        }
        if sol::sols_are_equal(scip, s, pr.sols[i], prob, NONE) {
            return true;
        }
    }
    false
}

/// `solOfInterest`: check if we are willing to check the solution for
/// feasibility; returns `(interesting, insertpos, replace)`, `replace`
/// saying whether the solution at `insertpos` should be replaced (e.g.,
/// because it lives in the original space).
fn sol_of_interest(scip: &Scip, p: usize, origprob: usize, transprob: usize, s: usize) -> (bool, i32, bool) {
    let mut insertpos = -1;
    let mut replace = false;

    // In real solving mode, we need to compare the real objective value with
    // the real cutoff bound.
    let obj = sol::sol_get_obj(scip, s, transprob, origprob);
    let solisacceptable = if !scip.set.is_infinity(obj) {
        !scip.set.misc_improvingsols || obj < scip.primals[p].cutoffbound
    } else {
        log::warn!("Ignoring primal solution candidate with objective value beyond infinity threshold.");
        false
    };

    // A solution is acceptable if its objective is finite and either not cut
    // off or non-improving solutions enabled.
    if solisacceptable {
        // Find insert position for the solution.
        insertpos = primal_search_sol_pos(scip, p, transprob, origprob, s);
        replace = false;

        // The solution should be added, if the insertpos is smaller than the
        // maximum number of solutions to be stored and it does not already
        // exist or it does exist, but the existing solution should be
        // replaced by the new one.
        if insertpos < scip.set.limit_maxsol && (!primal_exists_sol(scip, p, origprob, transprob, s, &mut insertpos, &mut replace) || replace) {
            return (true, insertpos, replace);
        }
    }
    (false, insertpos, replace)
}

/// `origsolOfInterest`: check if we are willing to store the solution
/// candidate for later checking; returns the insert position too.
fn origsol_of_interest(scip: &Scip, p: usize, origprob: usize, s: usize) -> (bool, i32) {
    // Find insert position for the solution.
    let insertpos = primal_search_orig_sol_pos(scip, p, s);

    let interesting = !scip.set.reopt_enable && insertpos < scip.set.limit_maxorigsol && !primal_exists_orig_sol(scip, p, origprob, s, insertpos);
    (interesting, insertpos)
}

/// `SCIPprimalAddSol`: adds primal solution to solution storage by copying
/// it; returns whether it was stored.
pub fn primal_add_sol(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize, s: usize) -> ScipResult<bool> {
    let (interesting, insertpos, replace) = sol_of_interest(scip, p, origprob, transprob, s);
    if interesting {
        // Create a copy of the solution.
        let solcopy = sol::sol_copy(scip, p, s)?;

        // Insert copied solution into solution storage.
        primal_add_sol_internal(scip, p, origprob, transprob, eventfilter, solcopy, insertpos, replace)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// `SCIPprimalAddSolFree`: adds primal solution to solution storage, frees
/// the solution afterwards; returns whether it was stored.
pub fn primal_add_sol_free(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize, s: usize) -> ScipResult<bool> {
    let (interesting, insertpos, replace) = sol_of_interest(scip, p, origprob, transprob, s);
    if interesting {
        // Insert solution into solution storage.
        primal_add_sol_internal(scip, p, origprob, transprob, eventfilter, s, insertpos, replace)?;
        Ok(true)
    } else {
        // The solution is too bad -> free it immediately.
        sol::sol_free(scip, s, p)?;
        Ok(false)
    }
}

/// `SCIPprimalAddOrigSol`: adds primal solution to solution candidate
/// storage of original problem space.
pub fn primal_add_orig_sol(scip: &mut Scip, p: usize, prob: usize, s: usize) -> ScipResult<bool> {
    if sol::sol_is_partial(scip, s) {
        // Create a copy of the solution.
        let solcopy = sol::sol_copy(scip, p, s)?;
        primal_add_orig_partial_sol(scip, p, solcopy)?;
        return Ok(true);
    }
    let (interesting, insertpos) = origsol_of_interest(scip, p, prob, s);
    if interesting {
        // Create a copy of the solution.
        let solcopy = sol::sol_copy(scip, p, s)?;

        // Insert solution into solution storage.
        primal_add_orig_sol_at(scip, p, prob, solcopy, insertpos)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn primal_add_orig_sol_at(scip: &mut Scip, p: usize, prob: usize, s: usize, insertpos: i32) -> ScipResult {
    primal_add_orig_sol_internal(scip, p, prob, s, insertpos)
}

/// `SCIPprimalAddOrigSolFree`: adds primal solution to solution candidate
/// storage of original problem space, frees the solution afterwards.
pub fn primal_add_orig_sol_free(scip: &mut Scip, p: usize, prob: usize, s: usize) -> ScipResult<bool> {
    if sol::sol_is_partial(scip, s) {
        // Insert solution into solution storage.
        primal_add_orig_partial_sol(scip, p, s)?;
        return Ok(true);
    }
    let (interesting, insertpos) = origsol_of_interest(scip, p, prob, s);
    if interesting {
        // Insert solution into solution storage.
        primal_add_orig_sol_at(scip, p, prob, s, insertpos)?;
        Ok(true)
    } else {
        // The solution is too bad -> free it immediately.
        sol::sol_free(scip, s, p)?;
        Ok(false)
    }
}

/// `primalLinkCurrentSol`: links temporary solution of primal data to
/// current solution.
fn primal_link_current_sol(scip: &mut Scip, p: usize, prob: usize, heur: HeurId) -> ScipResult {
    let currentsol = scip.primals[p].currentsol;
    if currentsol == NONE {
        let s = sol::sol_create_current_sol(scip, prob, p, heur)?;
        scip.primals[p].currentsol = s;
    } else {
        sol::sol_link_current_sol(scip, currentsol, prob)?;
        sol::sol_set_heur(scip, currentsol, heur);
    }
    Ok(())
}

/// `SCIPprimalAddCurrentSol`: adds current LP/pseudo solution to solution
/// storage.
pub fn primal_add_current_sol(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize, heur: HeurId) -> ScipResult<bool> {
    // Link temporary solution to current solution.
    primal_link_current_sol(scip, p, transprob, heur)?;

    // Add solution to solution storage.
    let currentsol = scip.primals[p].currentsol;
    primal_add_sol(scip, p, origprob, transprob, eventfilter, currentsol)
}

/// The check flags `SCIPprimalTrySol` passes to `SCIPsolCheck`.
#[derive(Clone, Copy, Debug)]
pub struct CheckFlags {
    pub printreason: bool,
    pub completely: bool,
    pub checkbounds: bool,
    pub checkintegrality: bool,
    pub checklprows: bool,
}

/// `SCIPprimalTrySol`: checks primal solution; if feasible, adds it to
/// storage by copying it; returns whether it was stored.
pub fn primal_try_sol(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize, s: usize, f: CheckFlags) -> ScipResult<bool> {
    let (interesting, insertpos, replace) = sol_of_interest(scip, p, origprob, transprob, s);

    // Check solution for feasibility.
    let feasible = interesting && sol::sol_check(scip, s, transprob, f.printreason, f.completely, f.checkbounds, f.checkintegrality, f.checklprows)?;

    if feasible {
        // Create a copy of the solution.
        let solcopy = sol::sol_copy(scip, p, s)?;

        // Insert copied solution into solution storage.
        primal_add_sol_internal(scip, p, origprob, transprob, eventfilter, solcopy, insertpos, replace)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// `SCIPprimalTrySolFree`: checks primal solution; if feasible, adds it to
/// storage; solution is freed afterwards.
pub fn primal_try_sol_free(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize, s: usize, f: CheckFlags) -> ScipResult<bool> {
    let (interesting, insertpos, replace) = sol_of_interest(scip, p, origprob, transprob, s);

    // Check solution for feasibility.
    let feasible = interesting && sol::sol_check(scip, s, transprob, f.printreason, f.completely, f.checkbounds, f.checkintegrality, f.checklprows)?;

    if feasible {
        // Insert solution into solution storage.
        primal_add_sol_internal(scip, p, origprob, transprob, eventfilter, s, insertpos, replace)?;
        Ok(true)
    } else {
        // The solution is too bad or infeasible -> free it immediately.
        sol::sol_free(scip, s, p)?;
        Ok(false)
    }
}

/// `SCIPprimalTryCurrentSol`: checks current LP/pseudo solution; if
/// feasible, adds it to storage.
#[allow(clippy::too_many_arguments)]
pub fn primal_try_current_sol(
    scip: &mut Scip,
    p: usize,
    origprob: usize,
    transprob: usize,
    eventfilter: usize,
    heur: HeurId,
    printreason: bool,
    completely: bool,
    checkintegrality: bool,
    checklprows: bool,
) -> ScipResult<bool> {
    // Link temporary solution to current solution.
    primal_link_current_sol(scip, p, transprob, heur)?;

    // Add solution to solution storage.
    let currentsol = scip.primals[p].currentsol;
    let f = CheckFlags {
        printreason,
        completely,
        checkbounds: false,
        checkintegrality,
        checklprows,
    };
    primal_try_sol(scip, p, origprob, transprob, eventfilter, currentsol, f)
}

/// `SCIPprimalSolCreated`: inserts solution into the global array of all
/// existing primal solutions.
pub fn primal_sol_created(scip: &mut Scip, p: usize, s: usize) {
    // Allocate memory for solution storage.
    let n = scip.primals[p].nexistingsols;
    ensure_existingsols_size(scip, p, n + 1);

    // Append solution.
    scip.sols[s].primalindex = n;
    scip.primals[p].existingsols[n] = s;
    scip.primals[p].nexistingsols += 1;
}

/// `SCIPprimalSolFreed`: removes solution from the global array of all
/// existing primal solutions.
pub fn primal_sol_freed(scip: &mut Scip, p: usize, s: usize) {
    // Remove solution.
    let idx = scip.sols[s].primalindex;
    let pr = &mut scip.primals[p];
    if idx < pr.nexistingsols - 1 {
        let moved = pr.existingsols[pr.nexistingsols - 1];
        pr.existingsols[idx] = moved;
        scip.sols[moved].primalindex = idx;
    }
    scip.primals[p].nexistingsols -= 1;
}

/// `SCIPprimalUpdateVarObj`: updates all existing primal solutions after a
/// change in a variable's objective value.
pub fn primal_update_var_obj(scip: &mut Scip, p: usize, var: VarId, oldobj: f64, newobj: f64) {
    for i in 0..scip.primals[p].nexistingsols {
        let s = scip.primals[p].existingsols[i];
        if !sol::sol_is_original(scip, s) {
            sol::sol_update_var_obj(scip, s, var, oldobj, newobj);
        }
    }
}

/// `SCIPprimalRetransformSolutions`: retransforms all existing solutions
/// to original problem space; as a side effect, the objective value of the
/// solutions can change (numerical errors) so we update the objective
/// cutoff value and upper bound accordingly.
pub fn primal_retransform_solutions(scip: &mut Scip, p: usize, origprob: usize, transprob: usize, eventfilter: usize) -> ScipResult {
    for i in 0..scip.primals[p].nsols {
        let s = scip.primals[p].sols[i];
        if scip.sols[s].solorigin == SolOrigin::Zero {
            sol::sol_retransform(scip, s, origprob, transprob)?;
        }
    }

    sort_primal_sols(scip, p, origprob, transprob);

    // Check if the global upper bound has to be updated; we do not inform
    // anybody about this change; if this leads to some problem, a possible
    // solution is to issue a BESTSOLFOUND event.
    if scip.primals[p].nsols > 0 {
        let obj = sol::sol_get_obj(scip, scip.primals[p].sols[0_usize], transprob, origprob);
        if obj < scip.primals[p].cutoffbound {
            // Update the upper bound.
            primal_set_upperbound(scip, p, transprob, eventfilter, obj)?;
        }
    }
    Ok(())
}

/// `SCIPprimalTransformSol`: tries to transform original solution to the
/// transformed problem space; returns whether the solution was added.
pub fn primal_transform_sol(scip: &mut Scip, p: usize, s: usize, origprob: usize, transprob: usize, eventfilter: usize) -> ScipResult<bool> {
    let norigvars = scip.probs[origprob].nvars;
    let ntransvars = scip.probs[transprob].nvars as usize;

    let mut localsolvals = vec![0.0; ntransvars];
    let mut localsolvalset = vec![false; ntransvars];
    let mut feasible = true;

    // For each original variable, get the corresponding active, fixed or
    // multi-aggregated variable; if it resolves to an active variable, we
    // set its solution value or check whether an already stored solution
    // value is consistent; if it resolves to a fixed variable, we check that
    // the fixing matches the original solution value; multi-aggregated
    // variables are skipped, because their value is defined by setting
    // solution values for the active variables, anyway.
    let mut v = 0;
    while v < norigvars && feasible {
        let mut var = scip.probs[origprob].vars[v];
        v += 1;
        let solval = sol::sol_get_val(scip, s, var);

        // Get corresponding active, fixed, or multi-aggregated variable.
        let mut scalar = 1.0;
        let mut constant = 0.0;
        crate::var::var_get_probvar_sum(scip, &mut var, &mut scalar, &mut constant)?;

        // Check whether the fixing corresponds to the solution value of the
        // original variable.
        if scalar == 0.0 {
            if !scip.set.is_eq(solval, constant) {
                feasible = false;
            }
        } else if crate::var::var_is_active(scip, var) {
            let pi = scip.vars[var].probindex as usize;

            // If we already assigned a solution value to the transformed
            // variable, check that it corresponds to the value obtained from
            // the currently regarded original variable.
            if localsolvalset[pi] {
                if !scip.set.is_eq(solval, scalar * localsolvals[pi] + constant) {
                    feasible = false;
                }
            }
            // Assign solution value to the transformed variable.
            else {
                localsolvals[pi] = (solval - constant) / scalar;
                localsolvalset[pi] = true;
            }
        }
    }

    // If the solution values of fixed and active variables lead to no
    // contradiction, construct solution and try it.
    if feasible {
        let heur = sol::sol_get_heur(scip, s);
        let transsol = sol::sol_create(scip, p, heur)?;

        // Set solution values for variables to which we assigned a value.
        for v in 0..ntransvars {
            if localsolvalset[v] {
                let tv = scip.probs[transprob].vars[v];
                sol::sol_set_val(scip, transsol, tv, localsolvals[v])?;
            }
        }

        let f = CheckFlags {
            printreason: false,
            completely: false,
            checkbounds: true,
            checkintegrality: true,
            checklprows: true,
        };
        primal_try_sol_free(scip, p, origprob, transprob, eventfilter, transsol, f)
    } else {
        Ok(false)
    }
}
