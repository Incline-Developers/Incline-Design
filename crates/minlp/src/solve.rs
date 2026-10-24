//! SCIP's main solving loop (`solve.c`): stopping criteria, primal
//! heuristics, propagation, the node LP and its price-and-cut loop,
//! enforcement, and the branch-and-bound loop of `SCIPsolveCIP`.

use crate::{
    api,
    heur::*,
    retcode::ScipResult,
    scip::{Scip, Stage},
    types::{NodeId, NodeType, ScipResultCode, Status},
    var::NONE,
};

/// `MAXNLPERRORS`: maximal number of LP error loops in a single node.
const MAXNLPERRORS: i32 = 10;
/// `MAXNCLOCKSKIPS`: maximum number of SCIPsolveIsStopped() calls without
/// checking the clock.
const MAXNCLOCKSKIPS: i32 = 64;
/// `NINITCALLS`: minimum number of calls to SCIPsolveIsStopped() prior to
/// dynamic clock skips.
const NINITCALLS: i64 = 1000;
/// `SAFETYFACTOR`: the probability that SCIP skips the clock call after the
/// time limit has already been reached.
const SAFETYFACTOR: f64 = 1e-2;

/// `SCIPsolveIsStopped`: returns whether the solving process will be / was
/// stopped before proving optimality; if the solving process was stopped,
/// stores the reason as status in stat. `checknodelimits`: should the node
/// limits be involved in the check?
pub fn solve_is_stopped(scip: &mut Scip, checknodelimits: bool) -> bool {
    // Increase the number of calls to this method.
    scip.stat.nisstoppedcalls += 1;

    // In case lowerbound >= upperbound, we do not want to terminate with
    // SCIP_STATUS_GAPLIMIT but with the ordinary
    // SCIP_STATUS_OPTIMAL/INFEASIBLE/...
    if scip.set.stage >= Stage::Solving && scip.set.is_le(api::get_upperbound(scip), api::get_lowerbound(scip)) {
        return true;
    }

    // If some limit has been changed since the last call, we reset the status.
    if scip.set.limitchanged {
        scip.stat.status = Status::Unknown;
        scip.set.limitchanged = false;
    }

    if scip.interrupted || scip.stat.userinterrupt {
        scip.stat.status = Status::UserInterrupt;
        scip.stat.userinterrupt = false;

        // Only reset the interrupted counter if this is the main SCIP catching
        // CTRL-C.
        if scip.set.misc_catchctrlc {
            scip.interrupted = false;
        }
    }
    // Only measure the clock if a time limit is set.
    else if scip.set.istimelimitfinite {
        // Check if we have already called this function sufficiently often for
        // a valid estimation of its average call interval.
        if scip.stat.nclockskipsleft <= 0 || scip.stat.nisstoppedcalls < NINITCALLS {
            let currtime = scip.stat.solvingtime.get_time();

            // Use the measured time to update the average time interval between
            // two calls to this method.
            if scip.set.time_rareclockcheck && scip.stat.nisstoppedcalls >= NINITCALLS {
                let mut nclockskips = MAXNCLOCKSKIPS;

                let avgisstoppedfreq = currtime / scip.stat.nisstoppedcalls as f64;

                // If we are approaching the time limit, reset the number of clock
                // skips to 0.
                if (SAFETYFACTOR * (scip.set.limit_time - currtime) / (avgisstoppedfreq + 1e-6)) < nclockskips as f64 {
                    nclockskips = 0;
                }

                scip.stat.nclockskipsleft = nclockskips;
            } else {
                scip.stat.nclockskipsleft = 0;
            }

            // Set the status if the time limit was hit.
            if currtime >= scip.set.limit_time {
                scip.stat.status = Status::TimeLimit;
                return true;
            }
        } else if scip.stat.solvingtime.get_last_time() >= scip.set.limit_time {
            // Use information if clock has been updated more recently.
            scip.stat.status = Status::TimeLimit;
            return true;
        } else {
            scip.stat.nclockskipsleft -= 1;
        }
    }

    // The memory limit (SCIPgetConcurrentMemTotal) is not measured: Improve
    // keeps SCIP's default, which no instance reaches.
    let set = &scip.set;
    if api::get_n_lim_sols_found(scip) > 0
        && (set.is_lt(api::get_gap(scip), set.limit_gap) || set.is_lt((api::get_upperbound(scip) - api::get_lowerbound(scip)) * api::get_trans_objscale(scip), set.limit_absgap))
    {
        scip.stat.status = Status::GapLimit;
    } else if set.limit_primal != crate::def::INVALID && !set.is_positive(api::get_objsense(scip) as i32 as f64 * (api::get_primalbound(scip) - set.limit_primal)) {
        scip.stat.status = Status::PrimalLimit;
    } else if set.limit_dual != crate::def::INVALID && !set.is_negative(api::get_objsense(scip) as i32 as f64 * (api::get_dualbound(scip) - set.limit_dual)) {
        scip.stat.status = Status::DualLimit;
    } else if set.limit_solutions >= 0 && set.stage >= Stage::Presolving && api::get_n_lim_sols_found(scip) >= set.limit_solutions as i64 {
        scip.stat.status = Status::SolLimit;
    } else if set.limit_bestsol >= 0 && set.stage >= Stage::Presolving && api::get_n_best_sols_found(scip) >= set.limit_bestsol as i64 {
        scip.stat.status = Status::BestSolLimit;
    } else if checknodelimits && set.limit_nodes >= 0 && scip.stat.nnodes >= set.limit_nodes {
        scip.stat.status = Status::NodeLimit;
    } else if checknodelimits && set.limit_totalnodes >= 0 && scip.stat.ntotalnodes >= set.limit_totalnodes {
        scip.stat.status = Status::TotalNodeLimit;
    } else if checknodelimits && set.limit_stallnodes >= 0 && scip.stat.nnodes >= scip.stat.bestsolnode + set.limit_stallnodes {
        scip.stat.status = Status::StallNodeLimit;
    }

    // If stat->status was initialized to SCIP_STATUS_NODELIMIT or
    // SCIP_STATUS_STALLNODELIMIT due to a previous call to
    // SCIPsolveIsStopped(,,TRUE), in the case of checknodelimits == FALSE, we
    // do not want to report here that the solve will be stopped due to a
    // nodelimit.
    let status = scip.stat.status;
    if !checknodelimits {
        status != Status::Unknown && status != Status::NodeLimit && status != Status::TotalNodeLimit && status != Status::StallNodeLimit
    } else {
        status != Status::Unknown
    }
}

/// `SCIPprimalHeuristics`: calls primal heuristics; `intree` is false when
/// called during presolving (C passes no tree), `withlp` false when called
/// during presolving or propagation (no LP); `nextnode` is the next node
/// that will be processed, or `NONE` if no more nodes left (only needed when
/// calling after node heuristics). Returns `(foundsol, unbounded)`: whether
/// a solution has been found, and whether an unbounded ray was found in the
/// LP.
#[allow(clippy::too_many_arguments)]
pub fn primal_heuristics(
    scip: &mut Scip,
    prob: usize,
    primal: usize,
    intree: bool,
    withlp: bool,
    nextnode: NodeId,
    mut heurtiming: HeurTiming,
    nodeinfeasible: bool,
) -> ScipResult<(bool, bool)> {
    let mut unbounded = false;

    // Nothing to do, if no heuristics are available, or if the branch-and-bound
    // process is finished.
    if scip.set.nheurs == 0 || (heurtiming == HEURTIMING_AFTERNODE && nextnode == NONE) {
        return Ok((false, unbounded));
    }

    // Do not continue if we reached a time limit.
    if solve_is_stopped(scip, false) {
        return Ok((false, unbounded));
    }

    // Sort heuristics by priority, but move the delayed heuristics to the
    // front.
    set_sort_heurs(scip);

    // Specialize the AFTERNODE timing flag.
    if (heurtiming & HEURTIMING_AFTERNODE) == HEURTIMING_AFTERNODE {
        // Clear the AFTERNODE flags and replace them by the right ones.
        heurtiming &= !HEURTIMING_AFTERNODE;

        // We are in plunging mode iff the next node is a sibling or a child, and
        // no leaf.
        let plunging = nextnode != NONE && scip.nodes[nextnode].nodetype != NodeType::Leaf;
        let pseudonode = !crate::tree::tree_has_focus_node_lp(scip);

        // Call plunging heuristics also at root node.
        if plunging && crate::tree::tree_get_current_depth(scip) > 0 {
            if !pseudonode {
                heurtiming |= HEURTIMING_AFTERLPNODE;
            } else {
                heurtiming |= HEURTIMING_AFTERPSEUDONODE;
            }
        } else if !pseudonode {
            heurtiming |= HEURTIMING_AFTERLPPLUNGE | HEURTIMING_AFTERLPNODE;
        } else {
            heurtiming |= HEURTIMING_AFTERPSEUDOPLUNGE | HEURTIMING_AFTERPSEUDONODE;
        }
    }

    // Initialize the tree related data, if we are not in presolving.
    let (depth, lpstateforkdepth);
    if heurtiming == HEURTIMING_BEFOREPRESOL || heurtiming == HEURTIMING_DURINGPRESOLLOOP {
        depth = -1;
        lpstateforkdepth = -1;
    } else {
        depth = crate::tree::tree_get_focus_depth(scip);
        let f = treer!(scip).focuslpstatefork;
        lpstateforkdepth = if f != NONE { scip.nodes[f].depth as i32 } else { -1 };
    }

    // Call heuristics.
    let mut ndelayedheurs = 0;
    let oldnbestsolsfound = scip.primals[primal].nbestsolsfound;

    // Collect lower bound of current node.
    let lowerbound = if intree {
        scip.nodes[crate::tree::tree_get_focus_node(scip)].lowerbound
    } else if withlp {
        crate::lp::lp_get_pseudo_objval(scip, prob)
    } else {
        -scip.set.infinity()
    };

    let mut h = 0;
    while h < scip.set.nheurs {
        // It might happen that a diving heuristic renders the previously solved
        // node LP invalid such that additional calls to LP heuristics will fail;
        // better abort the loop in this case.
        if withlp && lp!(scip).resolvelperror {
            break;
        }

        let heur = scip.set.heurs[h];
        let result = heur_exec(scip, heur, primal, depth, lpstateforkdepth, heurtiming, nodeinfeasible, &mut ndelayedheurs)?;

        // If the new solution cuts off the current node due to a new primal
        // solution (via the cutoff bound) interrupt calling the remaining
        // heuristics.
        if solve_is_stopped(scip, false) || (result == ScipResultCode::FoundSol && scip.set.is_ge(lowerbound, scip.primals[primal].cutoffbound)) {
            break;
        }

        // Check if the problem is proven to be unbounded, currently this happens
        // only in reoptimization.
        if result == ScipResultCode::Unbounded {
            unbounded = true;
            break;
        }
        h += 1;
    }

    let foundsol = scip.primals[primal].nbestsolsfound > oldnbestsolsfound;

    Ok((foundsol, unbounded))
}

/// `propagationRound`: applies one round of propagation; `depth` is the
/// depth level to use for propagator frequency checks; returns `(delayed,
/// propagain)`: whether a propagator was delayed and whether propagation
/// should be applied again; updates `cutoff` and `postpone`.
#[allow(clippy::too_many_arguments)]
fn propagation_round(
    scip: &mut Scip,
    depth: i32,
    fullpropagation: bool,
    onlydelayed: bool,
    timingmask: crate::cons::PropTiming,
    cutoff: &mut bool,
    postpone: &mut bool,
) -> ScipResult<(bool, bool)> {
    use ScipResultCode as R;

    let mut delayed = false;
    let mut propagain = false;

    // Sort propagators.
    crate::prop::set_sort_props(scip);

    // Check if we want to abort on a cutoff; if we are not in the solving stage
    // (e.g., in presolving), we want to abort anyway.
    let abortoncutoff = scip.set.prop_abortoncutoff || scip.set.stage != Stage::Solving;

    // The checks after each propagator or constraint handler.
    let after = |scip: &mut Scip, result: R, delayed: &mut bool, propagain: &mut bool, cutoff: &mut bool, postpone: &mut bool| -> bool {
        *delayed = *delayed || result == R::Delayed;
        *propagain = *propagain || result == R::ReducedDom;

        // Beside the result pointer of the propagator we have to check if an
        // internal cutoff was detected; this can happen when a global bound
        // change was applied which is globally valid and leads locally (for the
        // current node and others) to an infeasible problem.
        *cutoff = *cutoff || result == R::Cutoff || treer!(scip).cutoffdepth <= crate::tree::tree_get_current_depth(scip);
        *postpone = result == R::DelayNode && !*cutoff;

        // If we work off the delayed propagators, we stop immediately if a
        // reduction was found.
        if onlydelayed && result == R::ReducedDom {
            *delayed = true;
            return true;
        }
        false
    };

    // Call additional propagators with nonnegative priority, then propagate
    // constraints, then call additional propagators with negative priority.
    for pass in 0..3 {
        if pass == 1 {
            let mut i = 0;
            while i < scip.set.nconshdlrs as usize && !*postpone && (!*cutoff || !abortoncutoff) {
                let h = scip.set.conshdlrs[i];
                i += 1;

                // Timing needs to fit.
                if (scip.conshdlrs[h].proptiming & timingmask) == 0 {
                    continue;
                }

                if onlydelayed && !scip.conshdlrs[h].propwasdelayed {
                    continue;
                }

                let sbprobing = treer!(scip).sbprobing;
                let result = crate::cons::conshdlr_propagate(scip, h, depth, fullpropagation, onlydelayed, sbprobing, timingmask)?;
                if after(scip, result, &mut delayed, &mut propagain, cutoff, postpone) {
                    return Ok((delayed, propagain));
                }
            }
            continue;
        }

        let mut i = 0;
        while i < scip.set.nprops as usize && !*postpone && (!*cutoff || !abortoncutoff) {
            let p = scip.set.props[i];
            i += 1;

            // Timing needs to fit.
            if (scip.props[p].timingmask & timingmask) == 0 {
                continue;
            }

            if (pass == 0) == (scip.props[p].priority < 0) {
                continue;
            }

            if onlydelayed && !scip.props[p].wasdelayed {
                continue;
            }

            let sbprobing = treer!(scip).sbprobing;
            let result = crate::prop::prop_exec(scip, p, depth, onlydelayed, sbprobing, timingmask)?;
            if after(scip, result, &mut delayed, &mut propagain, cutoff, postpone) {
                return Ok((delayed, propagain));
            }
        }
    }

    Ok((delayed, propagain))
}

/// `propagateDomains`: applies domain propagation on current node;
/// `maxproprounds` is the maximal number of propagation rounds (-1: no
/// limit, 0: parameter settings); returns `(cutoff, postpone)`.
fn propagate_domains_internal(scip: &mut Scip, depth: i32, mut maxproprounds: i32, mut fullpropagation: bool, timingmask: crate::cons::PropTiming) -> ScipResult<(bool, bool)> {
    let node = crate::tree::tree_get_current_node(scip);

    // Adjust maximal number of propagation rounds.
    if maxproprounds == 0 {
        maxproprounds = if depth == 0 { scip.set.prop_maxroundsroot } else { scip.set.prop_maxrounds };
    }
    if maxproprounds == -1 {
        maxproprounds = i32::MAX;
    }

    // Propagate as long new bound changes were found and the maximal number of
    // propagation rounds is not exceeded.
    let mut cutoff = false;
    let mut postpone = false;
    let mut propround = 0;
    let mut propagain = true;
    while propagain && !cutoff && !postpone && propround < maxproprounds && !solve_is_stopped(scip, false) {
        propround += 1;

        // Perform the propagation round by calling the propagators and
        // constraint handlers.
        let mut delayed;
        (delayed, propagain) = propagation_round(scip, depth, fullpropagation, false, timingmask, &mut cutoff, &mut postpone)?;

        // If the propagation will be terminated, call the delayed propagators.
        while delayed && (!propagain || propround >= maxproprounds) && !cutoff {
            // Call the delayed propagators and constraint handlers.
            (delayed, propagain) = propagation_round(scip, depth, fullpropagation, true, timingmask, &mut cutoff, &mut postpone)?;
        }

        // If a reduction was found, we want to do another full propagation round
        // (even if the propagator only claimed to have done a domain reduction
        // without applying a domain change).
        fullpropagation = true;
    }

    // Mark the node to be completely propagated in the current repropagation
    // subtree level.
    crate::tree::node_mark_propagated(scip, node);

    Ok((cutoff, postpone))
}

/// `SCIPpropagateDomains`: applies domain propagation on current node and
/// flushes the conflict store afterwards; `maxrounds` is the maximal number
/// of propagation rounds (-1: no limit, 0: parameter settings).
#[allow(clippy::too_many_arguments)]
pub fn propagate_domains(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    depth: i32,
    maxrounds: i32,
    timingmask: crate::cons::PropTiming,
    cutoff: &mut bool,
) -> ScipResult {
    // Apply domain propagation.
    (*cutoff, _) = propagate_domains_internal(scip, depth, maxrounds, true, timingmask)?;

    // Flush the conflict set storage.
    crate::conflict::conflict_flush_conss(scip, transprob, origprob, eventfilter)
}

/// `isPseudocostUpdateValid`: returns whether the given variable with the
/// old LP solution value should lead to an update of the pseudo cost entry.
fn is_pseudocost_update_valid(scip: &Scip, var: crate::types::VarId, oldlpsolval: f64, updateintegers: bool, updatecontinuous: bool) -> bool {
    let set = &scip.set;
    let integral = crate::var::var_is_integral(scip, var);
    let v = &scip.vars[var];

    if !updatecontinuous && !integral {
        return false;
    }

    if !updateintegers && integral {
        return false;
    }

    if !integral && set.branch_lpgainnorm != b'l' {
        // If the variable is fixed at +/- infinity or it has an unbounded
        // domain, then the domain-based update strategies will not work.
        if set.is_infinity(v.locdom.lb.abs()) || set.is_infinity(v.locdom.ub.abs()) {
            return false;
        }

        // @todo if set->branch_lpgainnorm == 's', then we would need to know
        // then domain before branching since this is difficult to get, we
        // don't check for unboundedness here and let the pscost update fail
        // later however, this makes the weights used to spread a pseudo cost
        // update over all domain changes inaccurate.
        return true;
    }

    // If the old LP solution value is unknown, the pseudo cost update cannot
    // be performed.
    if oldlpsolval >= crate::def::INVALID {
        return false;
    }

    // The bound change on the given variable was responsible for the gain in
    // the dual bound, if the variable's old solution value is outside the
    // current bounds, and the new solution value is equal to the bound closest
    // to the old solution value.

    // Find out, which of the current bounds is violated by the old LP solution
    // value.
    if set.is_lt(oldlpsolval, v.locdom.lb) {
        let newlpsolval = crate::var::var_get_lp_sol(scip, var);
        set.is_eq(newlpsolval, v.locdom.lb)
    } else if set.is_gt(oldlpsolval, v.locdom.ub) {
        let newlpsolval = crate::var::var_get_lp_sol(scip, var);
        set.is_eq(newlpsolval, v.locdom.ub)
    } else {
        false
    }
}

/// `PSEUDOCOSTFLAG`: pseudo cost flag stored in the variables to mark them
/// for the pseudo cost update; variable's bounds were not changed.
const PSEUDOCOST_NONE: u32 = 0;
/// Bound changes on variable should be ignored for pseudo cost updates.
const PSEUDOCOST_IGNORE: u32 = 1;
/// Pseudo cost value of variable should be updated.
const PSEUDOCOST_UPDATE: u32 = 2;

/// The LP objective value stored at an LP state fork: C reads
/// `data.fork->lpobjval` for forks and subroots alike, which share the
/// field's offset.
fn lpstatefork_lpobjval(scip: &Scip, node: NodeId) -> f64 {
    let n = &scip.nodes[node];
    if n.nodetype == NodeType::Subroot {
        n.data.subroot.as_ref().unwrap().lpobjval
    } else {
        n.data.fork.as_ref().unwrap().lpobjval
    }
}

/// Collects the branching bound changes on the path between depths `from`
/// and `to` (both inclusive) whose variables are not yet marked, marking
/// each `PSEUDOCOST_UPDATE` when `valid` holds and `PSEUDOCOST_IGNORE`
/// otherwise; returns the bound changes and the number of valid updates.
fn collect_pseudocost_updates(scip: &mut Scip, from: i32, to: i32, valid: impl Fn(&Scip, &crate::var::BoundChg) -> bool) -> (Vec<crate::var::BoundChg>, i32) {
    use crate::types::BoundchgType;

    let mut updates = Vec::new();
    let mut nvalidupdates = 0;

    for d in from..=to {
        let node = treer!(scip).path[d];
        let Some(domchg) = scip.nodes[node].domchg.as_ref() else {
            continue;
        };
        let nboundchgs = domchg.nboundchgs as usize;

        for i in 0..nboundchgs {
            let bc = scip.nodes[node].domchg.as_ref().unwrap().boundchgs[i];
            let var = bc.var;

            if bc.boundchgtype == BoundchgType::Branching && scip.vars[var].pseudocostflag == PSEUDOCOST_NONE {
                // Remember the bound change and mark the variable.
                updates.push(bc);

                if valid(scip, &bc) {
                    scip.vars[var].pseudocostflag = PSEUDOCOST_UPDATE;
                    nvalidupdates += 1;
                } else {
                    scip.vars[var].pseudocostflag = PSEUDOCOST_IGNORE;
                }
            }
        }
    }

    (updates, nvalidupdates)
}

/// `updatePseudocost`: updates the variable's pseudo cost values after the
/// node's initial LP was solved.
fn update_pseudocost(scip: &mut Scip, prob: usize, updateintegers: bool, updatecontinuous: bool) -> ScipResult {
    use crate::types::{BoundType, LpSolStat};

    let focusnode = crate::tree::tree_get_focus_node(scip);
    let actdepth = scip.nodes[focusnode].depth as i32;
    let fork = treer!(scip).focuslpstatefork;

    if !((updateintegers || updatecontinuous) && lp!(scip).solved && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal && fork != NONE) {
        return Ok(());
    }

    let forkdepth = scip.nodes[fork].depth as i32;

    // Search the nodes from LP fork down to current node for bound changes in
    // between; move in this direction, because the bound changes closer to the
    // LP fork are more likely to have a valid LP solution information
    // attached; collect the bound changes for pseudo cost value updates and
    // mark the corresponding variables such that they are not updated twice in
    // case of more than one bound change on the same variable.
    //
    // We even collect redundant bound changes, since they were not redundant
    // in the LP branching decision and therefore should be regarded in the
    // pseudocost updates; however, if the variable is continuous and we
    // normalize the pseudo costs by the domain reduction, then getting the
    // variable bound before the branching is not possible by looking at the
    // variables branching information (since redundant branchings are not
    // applied); thus, in this case we ignore the boundchange.
    let (updates, nvalidupdates) = collect_pseudocost_updates(scip, forkdepth + 1, actdepth, |scip, bc| {
        is_pseudocost_update_valid(scip, bc.var, bc.data.branchingdata.lpsolval, updateintegers, updatecontinuous)
            && (crate::var::var_is_integral(scip, bc.var) || !bc.redundant || scip.set.branch_lpgainnorm != b'd')
    });

    // Update the pseudo cost values and reset the variables' flags; assume,
    // that the responsibility for the dual gain is equally spread on all bound
    // changes that lead to valid pseudo cost updates.
    let weight = if nvalidupdates > 0 { 1.0 / nvalidupdates as f64 } else { 1.0 };
    let mut lpgain = (crate::lp::lp_get_objval(scip, prob) - lpstatefork_lpobjval(scip, fork)) * weight;
    lpgain = crate::def::max(lpgain, 0.0);

    for u in &updates {
        let var = u.var;
        if scip.vars[var].pseudocostflag == PSEUDOCOST_UPDATE {
            if crate::var::var_is_integral(scip, var) || scip.set.branch_lpgainnorm == b'l' {
                let delta = crate::var::var_get_lp_sol(scip, var) - u.data.branchingdata.lpsolval;
                crate::var::var_update_pseudocost(scip, var, delta, lpgain, weight)?;
            } else {
                // set->branch_lpgainnorm == 'd':
                // For continuous variables, we want to pseudocosts to be the
                // average of the gain in the LP value if the domain is reduced
                // from x% of its original width to y% of its original (e.g.,
                // global) width, i.e., to be the average of LPgain /
                // (oldwidth/origwidth - newwidth/origwidth) = LPgain * origwidth
                // / (oldwidth - newwidth). Then an expected improvement in the LP
                // value by a reduction of the domain width from x% to y% of its
                // original width can be computed by pseudocost * (oldwidth -
                // newwidth) / origwidth. Since the original width cancels out, we
                // can also define the pseudocosts as average of LPgain /
                // (oldwidth - newwidth) and compute the expected improvement as
                // pseudocost * (oldwidth - newwidth).
                //
                // Let var have bounds [a,c] before the branching and assume we
                // branched on some value b. b is given by updates[i]->newbound.
                //
                // If updates[i]->boundtype = upper, then node corresponds to the
                // child [a,b]. Thus, we have oldwidth = c-a, newwidth = b-a, and
                // oldwidth - newwidth = c-b. To get c (the previous upper bound),
                // we look into the var->ubchginfos array.
                //
                // If updates[i]->boundtype = lower, then node corresponds to the
                // child [b,c]. Thus, we have oldwidth = c-a, newwidth = c-b, and
                // oldwidth - newwidth = b-a. To get c (the previous lower bound),
                // we look into the var->lbchginfos array.
                let mut oldbound = crate::def::INVALID;
                let delta;
                let v = &scip.vars[var];
                let set = &scip.set;

                if set.branch_lpgainnorm == b'd' {
                    let upper = u.boundtype == BoundType::Upper;
                    let (infos, n) = if upper { (&v.ubchginfos, v.nubchginfos) } else { (&v.lbchginfos, v.nlbchginfos) };

                    // Walk backwards through bound change information array to
                    // find the bound change corresponding to branching in
                    // updates[i]; usually it will be the first one we look at.
                    let mut j = n - 1;
                    while j >= 0 {
                        let bdchginfo = &infos[j];
                        let beyond = if upper { bdchginfo.oldbound > u.newbound } else { bdchginfo.oldbound < u.newbound };
                        if beyond {
                            // First boundchange which bound is beyond the bound set by
                            // the branching in updates[i]; if it is a branching, then
                            // this should be exactly the bound change that we are
                            // looking for; otherwise this should be because the
                            // branching domain change has not been applied to the
                            // variable due to redundancy; in this case, i.e., if there
                            // was another boundchange coming from somewhere else, we
                            // are not sure whether oldbound is an accurate value to
                            // compute the old domain size, so we skip the pseudocosts
                            // update.
                            if bdchginfo.boundchgtype == crate::types::BoundchgType::Branching {
                                oldbound = bdchginfo.oldbound;
                            } else {
                                break;
                            }
                        }
                        j -= 1;
                    }

                    // If the bound change was redundant (e.g., due to a change in
                    // the global bound), then it was not applied, so there exists
                    // no corresponding bound change info; if it is not redundant,
                    // then we should have found at least one corresponding
                    // boundchange.
                    if oldbound != crate::def::INVALID {
                        // If the old bound is at infinity or the new bound at
                        // -infinity (mirrored for lower bounds), then we say the
                        // delta is infinity.
                        let inf = if upper {
                            set.is_infinity(oldbound) || set.is_infinity(-u.newbound)
                        } else {
                            set.is_infinity(-oldbound) || set.is_infinity(u.newbound)
                        };
                        delta = if inf { crate::def::INVALID } else { u.newbound - oldbound };
                    } else {
                        delta = crate::def::INVALID;
                    }
                } else {
                    // set->branch_lpgainnorm == 's': Here, we divide the LPgain by
                    // the reduction in the sibling node.
                    //
                    // If updates[i]->boundtype = upper, then node corresponds to the
                    // child [a,b]. Thus, we have oldwidth = c-a, newwidth = c-b, and
                    // oldwidth - newwidth = b-a. Conveniently, we just use the
                    // current lower bound for a (it may have been tightened,
                    // though).
                    //
                    // If updates[i]->boundtype = lower, then node corresponds to the
                    // child [b,a]. Thus, we have oldwidth = c-a, newwidth = b-a, and
                    // oldwidth - newwidth = c-b. Conveniently, we just use the
                    // current upper bound for c (it may have been tightened,
                    // though).
                    if u.boundtype == BoundType::Upper {
                        delta = if set.is_infinity(-u.newbound) || set.is_infinity(-v.locdom.lb) {
                            crate::def::INVALID
                        } else {
                            u.newbound - v.locdom.lb
                        };
                    } else {
                        delta = if set.is_infinity(u.newbound) || set.is_infinity(v.locdom.ub) {
                            crate::def::INVALID
                        } else {
                            -(v.locdom.ub - u.newbound)
                        };
                    }
                }

                if delta != crate::def::INVALID {
                    crate::var::var_update_pseudocost(scip, var, delta, lpgain, weight)?;
                }
            }
        }
        scip.vars[var].pseudocostflag = PSEUDOCOST_NONE;
    }

    // Update the ancestor pscost fields.
    if forkdepth >= 1 && scip.set.branch_collectancpscost {
        // If the last fork is not the root node, we have more ancestors.
        let mut ancestordepth = -1;
        let mut d = forkdepth - 1;
        while d >= 0 {
            let node = treer!(scip).path[d];
            if scip.nodes[node].nodetype == NodeType::Fork {
                ancestordepth = d;
                break;
            }
            d -= 1;
        }

        if ancestordepth >= 0 {
            // Ancestor pseudo cost updates for an ancestor; we even collect
            // redundant bound changes, since they were not redundant in the LP
            // branching decision and therefore should be regarded in the
            // ancestor pseudocost updates.
            let (updates, nvalidupdates) = collect_pseudocost_updates(scip, ancestordepth + 1, forkdepth, |scip, bc| {
                // Check for valid pseudocost updates for integer variables.
                is_pseudocost_update_valid(scip, bc.var, bc.data.branchingdata.lpsolval, updateintegers, updatecontinuous) && crate::var::var_is_integral(scip, bc.var)
            });

            // Update the ancestor pseudo cost values and reset the variables'
            // flags; assume, that the responsibility for the dual gain is equally
            // spread on all bound changes that lead to valid ancestor pseudo cost
            // updates.
            let weight = if nvalidupdates > 0 { 1.0 / nvalidupdates as f64 } else { 1.0 };

            // Lp gain is same as above but with different weight.
            let mut lpgain = (crate::lp::lp_get_objval(scip, prob) - lpstatefork_lpobjval(scip, fork)) * weight;
            lpgain = crate::def::max(lpgain, 0.0);

            for u in &updates {
                let var = u.var;
                if scip.vars[var].pseudocostflag == PSEUDOCOST_UPDATE {
                    // We use updates[i]->newbound as new lp value for measuring
                    // change in the LP value.
                    crate::var::var_update_anc_pseudocost(scip, var, u.newbound - u.data.branchingdata.lpsolval, lpgain, weight)?;
                }
                scip.vars[var].pseudocostflag = PSEUDOCOST_NONE;
            }
        }
    }

    Ok(())
}

/// `updateEstimate`: updates the estimated value of a primal feasible
/// solution for the focus node after the LP was solved.
fn update_estimate(scip: &mut Scip) {
    use crate::types::LpSolStat;

    // Estimate is only available if LP was solved to optimality.
    if !crate::tree::tree_has_focus_node_lp(scip) || crate::lp::lp_get_solstat(scip) != LpSolStat::Optimal || !crate::lp::lp_is_relax(scip) {
        return;
    }

    let focusnode = crate::tree::tree_get_focus_node(scip);

    // Get the fractional variables.
    let counts = crate::branch::branchcand_get_lp_cands(scip);

    // Calculate the estimate: lowerbound + sum(min{f_j * pscdown_j, (1-f_j) *
    // pscup_j}).
    let mut estimate = scip.nodes[focusnode].lowerbound;

    // An infinite lower bound implies an infinite estimate.
    if scip.set.is_infinity(estimate) {
        crate::tree::node_set_estimate(scip, focusnode, estimate);
        return;
    }

    for i in 0..counts.nlpcands as usize {
        let (cand, frac) = (scip.branchcand.lpcands[i], scip.branchcand.lpcandsfrac[i]);
        let pscdown = crate::var::var_get_pseudocost(scip, cand, 0.0 - frac);
        let pscup = crate::var::var_get_pseudocost(scip, cand, 1.0 - frac);
        estimate += crate::def::min(pscdown, pscup);
    }

    crate::tree::node_set_estimate(scip, focusnode, estimate);
}

/// `SCIPinitConssLP`: puts all constraints with initial flag TRUE into the
/// LP; `root`: is this the initial root LP? `firstsubtreeinit`: is this the
/// first call in the current subtree after jumping through the tree?
/// Returns whether the node can be cut off.
pub fn init_conss_lp(scip: &mut Scip, s: usize, transprob: usize, origprob: usize, eventfilter: usize, root: bool, firstsubtreeinit: bool) -> ScipResult<bool> {
    let mut cutoff = false;

    // Inform separation storage, that LP is now filled with initial data.
    crate::sepastore::sepastore_start_initial_lp(scip, s);

    // Add LP relaxations of all initial constraints to LP.
    let mut h = 0;
    while h < scip.set.nconshdlrs as usize && !cutoff {
        let hdlr = scip.set.conshdlrs[h];
        cutoff = crate::cons::conshdlr_init_lp(scip, hdlr, firstsubtreeinit)?;
        h += 1;
    }

    if !cutoff {
        // Apply cuts.
        cutoff = crate::sepastore::sepastore_apply_cuts(scip, s, transprob, origprob, eventfilter, root)?;
    } else {
        // The current node will be cut off; we clear the sepastore.
        crate::sepastore::sepastore_clear_cuts(scip, s, eventfilter)?;
    }

    // Inform separation storage, that initial LP setup is now finished.
    crate::sepastore::sepastore_end_initial_lp(scip, s);

    Ok(cutoff)
}

/// Adds the initial variables of the transformed problem that `pick` selects
/// to the LP; returns whether an empty domain was found.
fn add_initial_vars(scip: &mut Scip, transprob: usize, pick: impl Fn(&Scip, crate::types::VarId) -> bool) -> ScipResult<bool> {
    let mut cutoff = false;

    // Inform pricing storage, that LP is now filled with initial data.
    crate::pricestore::pricestore_start_initial_lp(scip);

    // Add all initial variables to LP.
    for v in 0..scip.probs[transprob].nvars as usize {
        let var = scip.probs[transprob].vars[v];
        if pick(scip, var) {
            crate::pricestore::pricestore_add_var(scip, var, 0.0, true)?;

            // Check for empty domains (necessary if no presolving was performed).
            if scip.set.is_gt(scip.vars[var].locdom.lb, scip.vars[var].locdom.ub) {
                cutoff = true;
                break;
            }
        }
    }

    crate::pricestore::pricestore_apply_vars(scip, transprob)?;

    // Inform pricing storage, that initial LP setup is now finished.
    crate::pricestore::pricestore_end_initial_lp(scip);

    Ok(cutoff)
}

/// `initLP`: constructs the initial LP of the current node; returns whether
/// the node can be cut off.
fn init_lp(scip: &mut Scip, s: usize, transprob: usize, origprob: usize, eventfilter: usize, root: bool) -> ScipResult<bool> {
    let mut oldnvars = 0;

    // At the root node, we have to add the initial variables as columns.
    if root {
        // Store number of variables for later.
        oldnvars = scip.probs[transprob].nvars;

        if add_initial_vars(scip, transprob, |scip, var| scip.vars[var].initial)? {
            return Ok(true);
        }
    }

    // Put all initial constraints into the LP.
    // @todo check whether we jumped through the tree
    if init_conss_lp(scip, s, transprob, origprob, eventfilter, root, true)? {
        return Ok(true);
    }

    // Putting all initial constraints into the LP might have added new
    // variables.
    if root && scip.probs[transprob].nvars > oldnvars {
        // Check all initial variables.
        let cutoff = add_initial_vars(scip, transprob, |scip, var| {
            let v = &scip.vars[var];
            v.initial && (v.varstatus != crate::types::VarStatus::Column || scip.cols[v.data.col].lppos < 0)
        })?;
        return Ok(cutoff);
    }

    Ok(false)
}

/// `SCIPconstructCurrentLP`: constructs the LP of the current node, but does
/// not load the LP state and warmstart information; `newinitconss`: do we
/// have to add new initial constraints? Returns whether the node can be cut
/// off.
pub fn construct_current_lp(scip: &mut Scip, s: usize, transprob: usize, origprob: usize, eventfilter: usize, newinitconss: bool) -> ScipResult<bool> {
    let mut cutoff = false;

    if !crate::tree::tree_is_focus_node_lp_constructed(scip) {
        // Inform separation storage, that LP is now filled with initial data.
        crate::sepastore::sepastore_start_initial_lp(scip, s);

        let correctlpdepth = treer!(scip).correctlpdepth;
        if correctlpdepth >= 0 {
            let mut i = treer!(scip).pathnlprows[correctlpdepth] as usize;
            while i < lp!(scip).nrows as usize {
                let row = lp!(scip).rows[i];

                // Keep all active global cuts that where applied in the previous
                // node in the lp.
                if !scip.rows[row].local && scip.rows[row].age == 0 {
                    // This has no effect inside initial LP, but is set for
                    // consistency.
                    scip.rows[row].fromcutpool = true;
                    let root = crate::tree::tree_get_current_depth(scip) == 0;
                    cutoff = crate::sepastore::sepastore_add_cut(scip, s, eventfilter, row, true, root)?;
                }
                i += 1;
            }
        }

        let mut initroot = false;
        if !cutoff {
            // Load the LP into the solver and load the LP state.
            initroot = crate::tree::tree_load_lp(scip)?;

            // Apply cuts.
            let root = crate::tree::tree_get_current_depth(scip) == 0;
            cutoff = crate::sepastore::sepastore_apply_cuts(scip, s, transprob, origprob, eventfilter, root)?;
        } else {
            // The current node will be cut off; we clear the sepastore.
            crate::sepastore::sepastore_clear_cuts(scip, s, eventfilter)?;
        }

        // Inform separation storage, that initial LP setup is now finished.
        crate::sepastore::sepastore_end_initial_lp(scip, s);

        if !cutoff {
            // Setup initial LP relaxation of node.
            cutoff = init_lp(scip, s, transprob, origprob, eventfilter, initroot)?;
        }
    } else if newinitconss {
        cutoff = init_conss_lp(scip, s, transprob, origprob, eventfilter, false, false)?;
    }

    Ok(cutoff)
}

/// `updatePrimalRay`: updates the primal ray stored in primal data; clears
/// previously stored primal ray, if existing and there was no LP error;
/// stores current primal ray, if LP is unbounded and there has been no
/// error.
fn update_primal_ray(scip: &mut Scip, prob: usize, primal: usize, lperror: bool) -> ScipResult {
    if lperror {
        return Ok(());
    }

    // Clear previously stored primal ray, if any.
    let ray = scip.primals[primal].primalray;
    if ray != NONE {
        crate::sol::sol_free(scip, ray, primal)?;
        scip.primals[primal].primalray = NONE;
    }

    // Store unbounded ray, if LP is unbounded.
    if crate::lp::lp_get_solstat(scip) == crate::types::LpSolStat::UnboundedRay {
        let nvars = scip.probs[prob].nvars as usize;

        // Get buffer memory for storing the ray and load the ray values into it.
        let mut ray = vec![0.0; nvars];
        crate::lp::lp_get_primal_ray(scip, &mut ray)?;

        // Create solution to store the primal ray in.
        let sol = crate::sol::sol_create(scip, primal, NONE)?;
        scip.primals[primal].primalray = sol;

        // Set values of all active variable in the solution that represents the
        // primal ray.
        for i in 0..nvars {
            let var = scip.probs[prob].vars[i];
            crate::sol::sol_set_val(scip, sol, var, ray[i])?;
        }
    }

    Ok(())
}

/// `solveNodeInitialLP`: load and solve the initial LP of a node;
/// `newinitconss`: do we have to add new initial constraints?
/// `forcedlpsolve`: would SCIP abort if the LP is not solved? Returns
/// `(cutoff, lperror)`: whether the node can be cut off, and whether an
/// unresolved error in LP solving occured.
fn solve_node_initial_lp(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    primal: usize,
    eventfilter: usize,
    newinitconss: bool,
    forcedlpsolve: bool,
) -> ScipResult<(bool, bool)> {
    use crate::types::LpSolStat;

    // Initializing variables for compiler warnings, which are not correct.
    let mut starttime = 0.0;
    let mut nlpiterations = 0;

    // Load the LP into the solver.
    let cutoff = construct_current_lp(scip, crate::sepastore::SEPASTORE, transprob, origprob, eventfilter, newinitconss)?;

    if cutoff {
        return Ok((true, false));
    }

    // Load the LP state.
    crate::tree::tree_load_lp_state(scip, transprob)?;

    let focusnode = crate::tree::tree_get_focus_node(scip);
    let rootnode = scip.nodes[focusnode].depth == 0;

    // Store current LP iteration count and solving time if we are at the root
    // node.
    if rootnode {
        nlpiterations = scip.stat.nlpiterations;
        starttime = scip.stat.solvingtime.get_time();
    }

    // Solve initial LP.
    let itlim = if rootnode { scip.set.lp_rootiterlim } else { scip.set.lp_iterlim };
    let lperror = crate::lp::lp_solve_and_eval(scip, transprob, itlim, true, true, false, forcedlpsolve)?;

    // Save time for very first LP in root node.
    if scip.stat.nnodelps == 0 && rootnode {
        scip.stat.firstlptime = scip.stat.solvingtime.get_time() - starttime;
    }

    // Remove previous primal ray, store new one if LP is unbounded.
    update_primal_ray(scip, transprob, primal, lperror)?;

    if !lperror {
        let solstat = crate::lp::lp_get_solstat(scip);
        if solstat != LpSolStat::IterLimit && solstat != LpSolStat::TimeLimit {
            // Issue FIRSTLPSOLVED event.
            let event = crate::event::event_create_node(crate::event::EVENTTYPE_FIRSTLPSOLVED, crate::tree::tree_get_focus_node(scip));
            crate::event::event_process(scip, &event, eventfilter)?;
        }

        // Update pseudo cost values for integer variables (always) and for
        // continuous variables (if not delayed).
        let delay = scip.set.branch_delaypscost;
        update_pseudocost(scip, transprob, true, !delay)?;

        // Update lower bound of current node w.r.t. initial lp.
        let solstat = crate::lp::lp_get_solstat(scip);
        if (solstat == LpSolStat::Optimal || solstat == LpSolStat::UnboundedRay || solstat == LpSolStat::ObjLimit)
            && crate::prob::prob_all_cols_in_lp(scip, transprob)
            && crate::lp::lp_is_relax(scip)
        {
            crate::tree::node_update_lowerbound_lp(scip, focusnode, transprob, origprob, eventfilter)?;

            // If this is the first LP solved at the root, store its iteration
            // count and solution value.
            if scip.stat.nnodelps == 0 && rootnode {
                scip.stat.nrootfirstlpiterations = scip.stat.nlpiterations - nlpiterations;
                let lowerbound = scip.nodes[focusnode].lowerbound;
                scip.stat.firstlpdualbound = crate::prob::prob_extern_objval(scip, transprob, origprob, lowerbound);
            }
        }
    }

    Ok((false, lperror))
}

/// `separationRoundResolveLP`: makes sure the LP is flushed and solved;
/// updates `lperror`, and sets `mustsepa` and `mustprice` if additional
/// separation and pricing rounds should be performed.
fn separation_round_resolve_lp(scip: &mut Scip, prob: usize, primal: usize, lperror: &mut bool, mustsepa: &mut bool, mustprice: &mut bool) -> ScipResult {
    // If bound changes were applied in the separation round, we have to
    // resolve the LP.
    if !lp!(scip).flushed {
        // Solve LP (with dual simplex).
        let itlim = scip.set.lp_iterlim;
        *lperror = crate::lp::lp_solve_and_eval(scip, prob, itlim, false, true, false, false)?;

        *mustsepa = true;
        *mustprice = true;

        // Remove previous primal ray, store new one if LP is unbounded.
        update_primal_ray(scip, prob, primal, *lperror)?;
    }

    Ok(())
}

/// The `enoughcuts` update after a separator or constraint handler ran.
fn update_enoughcuts(scip: &Scip, s: usize, root: bool, result: ScipResultCode, enoughcuts: &mut bool) {
    let set = &scip.set;
    let factor = set.get_sepa_maxcuts_gen_factor(root);
    if set.is_zero(factor * set.get_sepa_maxcuts(root) as f64) {
        *enoughcuts = true;
    } else if set.is_negative(factor) {
        *enoughcuts = *enoughcuts || result == ScipResultCode::NewRound;
    } else {
        *enoughcuts = *enoughcuts || scip.sepastores[s].ncuts as i64 >= set.ceil(factor * set.get_sepa_maxcuts(root) as f64) as i64 || result == ScipResultCode::NewRound;
    }
}

/// Whether a result found something, which stops working off delayed
/// separators.
#[inline]
fn sepa_found(result: ScipResultCode) -> bool {
    use ScipResultCode as R;
    result == R::ConsAdded || result == R::ReducedDom || result == R::Separated || result == R::NewRound
}

/// The flags one LP separation round reports.
#[derive(Default)]
struct SepaRoundLp {
    delayed: bool,
    enoughcuts: bool,
}

/// `separationRoundLP`: applies one round of LP separation; `bounddist` is
/// the current relative distance of local dual bound to global dual bound;
/// updates `cutoff`, `lperror`, `mustsepa` and `mustprice`.
#[allow(clippy::too_many_arguments)]
fn separation_round_lp(
    scip: &mut Scip,
    prob: usize,
    primal: usize,
    s: usize,
    actdepth: i32,
    bounddist: f64,
    allowlocal: bool,
    onlydelayed: bool,
    cutoff: &mut bool,
    lperror: &mut bool,
    mustsepa: &mut bool,
    mustprice: &mut bool,
) -> ScipResult<SepaRoundLp> {
    use ScipResultCode as R;

    use crate::types::LpSolStat;

    let root = actdepth == 0;
    let mut r = SepaRoundLp::default();

    {
        let set = &scip.set;
        let factor = set.get_sepa_maxcuts_gen_factor(root);
        r.enoughcuts = if set.is_zero(factor * set.get_sepa_maxcuts(root) as f64) {
            true
        } else if set.is_negative(factor) {
            false
        } else {
            scip.sepastores[s].ncuts as i64 >= set.ceil(factor * set.get_sepa_maxcuts(root) as f64) as i64
        };
    }

    *lperror = false;
    let mut consadded = false;

    // Sort separators by priority.
    crate::sepa::set_sort_sepas(scip);

    let lpok = |scip: &Scip| {
        let lp = scip.lp.as_ref().unwrap();
        lp.flushed && lp.solved && (lp.lpsolstat == LpSolStat::Optimal || lp.lpsolstat == LpSolStat::UnboundedRay)
    };

    // Call LP separators with nonnegative priority, try separating constraints
    // of the constraint handlers, then call LP separators with negative
    // priority.
    for pass in 0..3 {
        let n = if pass == 1 { scip.set.nconshdlrs } else { scip.set.nsepas } as usize;
        let mut i = 0;
        while i < n && !*cutoff && !*lperror && !r.enoughcuts && lpok(scip) {
            let result;
            if pass == 1 {
                let h = scip.set.conshdlrs_sepa[i];
                i += 1;

                if onlydelayed && !scip.conshdlrs[h].sepalpwasdelayed {
                    continue;
                }

                result = crate::cons::conshdlr_separate_lp(scip, h, actdepth, onlydelayed)?;
            } else {
                let sepa = scip.set.sepas[i];
                i += 1;

                if (pass == 0) == (scip.sepas[sepa].priority < 0) {
                    continue;
                }

                if onlydelayed && !scip.sepas[sepa].lpwasdelayed {
                    continue;
                }

                result = crate::sepa::sepa_exec_lp(scip, sepa, s, actdepth, bounddist, allowlocal, onlydelayed)?;
            }

            *cutoff = *cutoff || result == R::Cutoff;
            consadded = consadded || result == R::ConsAdded;
            update_enoughcuts(scip, s, root, result, &mut r.enoughcuts);
            r.delayed = r.delayed || result == R::Delayed;

            if !*cutoff {
                // Make sure the LP is solved (after adding bound changes, LP has to
                // be flushed and resolved).
                separation_round_resolve_lp(scip, prob, primal, lperror, mustsepa, mustprice)?;
            }

            // If we work off the delayed separators, we stop immediately if a cut
            // was found.
            if onlydelayed && sepa_found(result) {
                r.delayed = true;
                return Ok(r);
            }
        }
    }

    // Process the constraints that were added during this separation round.
    while consadded {
        consadded = false;

        let mut i = 0;
        while i < scip.set.nconshdlrs as usize && !*cutoff && !*lperror && !r.enoughcuts && lpok(scip) {
            let h = scip.set.conshdlrs_sepa[i];
            i += 1;

            let result = crate::cons::conshdlr_separate_lp(scip, h, actdepth, onlydelayed)?;
            *cutoff = *cutoff || result == R::Cutoff;
            consadded = consadded || result == R::ConsAdded;
            update_enoughcuts(scip, s, root, result, &mut r.enoughcuts);
            r.delayed = r.delayed || result == R::Delayed;

            if !*cutoff {
                // Make sure the LP is solved (after adding bound changes, LP has to
                // be flushed and resolved).
                separation_round_resolve_lp(scip, prob, primal, lperror, mustsepa, mustprice)?;
            }
        }
    }

    Ok(r)
}

/// `separationRoundSol`: applies one round of separation on the given
/// primal solution; returns `(delayed, enoughcuts)` and updates `cutoff`.
fn separation_round_sol(scip: &mut Scip, s: usize, sol: crate::types::SolId, actdepth: i32, allowlocal: bool, onlydelayed: bool, cutoff: &mut bool) -> ScipResult<(bool, bool)> {
    use ScipResultCode as R;

    let mut delayed = false;
    let mut enoughcuts = false;
    let mut consadded = false;
    let root = actdepth == 0;

    // Sort separators by priority.
    crate::sepa::set_sort_sepas(scip);

    // Call separators with nonnegative priority, try separating constraints of
    // the constraint handlers, then call separators with negative priority.
    for pass in 0..3 {
        let n = if pass == 1 { scip.set.nconshdlrs } else { scip.set.nsepas } as usize;
        let mut i = 0;
        while i < n && !*cutoff && !enoughcuts && !solve_is_stopped(scip, false) {
            let result;
            if pass == 1 {
                let h = scip.set.conshdlrs_sepa[i];
                i += 1;

                if onlydelayed && !scip.conshdlrs[h].sepasolwasdelayed {
                    continue;
                }

                result = crate::cons::conshdlr_separate_sol(scip, h, sol, actdepth, onlydelayed)?;
            } else {
                let sepa = scip.set.sepas[i];
                i += 1;

                if (pass == 0) == (scip.sepas[sepa].priority < 0) {
                    continue;
                }

                if onlydelayed && !scip.sepas[sepa].solwasdelayed {
                    continue;
                }

                result = crate::sepa::sepa_exec_sol(scip, sepa, s, sol, actdepth, allowlocal, onlydelayed)?;
            }

            *cutoff = *cutoff || result == R::Cutoff;
            consadded = consadded || result == R::ConsAdded;
            update_enoughcuts(scip, s, root, result, &mut enoughcuts);
            delayed = delayed || result == R::Delayed;

            // If we work off the delayed separators, we stop immediately if a cut
            // was found.
            if onlydelayed && sepa_found(result) {
                return Ok((true, enoughcuts));
            }
        }
    }

    // Process the constraints that were added during this separation round.
    while consadded {
        consadded = false;

        let mut i = 0;
        while i < scip.set.nconshdlrs as usize && !*cutoff && !enoughcuts && !solve_is_stopped(scip, false) {
            let h = scip.set.conshdlrs_sepa[i];
            i += 1;

            let result = crate::cons::conshdlr_separate_sol(scip, h, sol, actdepth, onlydelayed)?;
            *cutoff = *cutoff || result == R::Cutoff;
            consadded = consadded || result == R::ConsAdded;
            update_enoughcuts(scip, s, root, result, &mut enoughcuts);
            delayed = delayed || result == R::Delayed;
        }
    }

    Ok((delayed, enoughcuts))
}

/// `SCIPseparationRound`: applies one round of separation on the given
/// primal solution or on the LP solution (`sol` is `NONE`); returns
/// `(delayed, cutoff)`.
#[allow(clippy::too_many_arguments)]
pub fn separation_round(
    scip: &mut Scip,
    prob: usize,
    primal: usize,
    s: usize,
    sol: crate::types::SolId,
    actdepth: i32,
    allowlocal: bool,
    onlydelayed: bool,
) -> ScipResult<(bool, bool)> {
    let mut cutoff = false;
    let delayed;

    if sol == NONE {
        // Apply a separation round on the LP solution.
        let mut lperror = false;
        let mut mustsepa = false;
        let mut mustprice = false;
        let r = separation_round_lp(
            scip,
            prob,
            primal,
            s,
            actdepth,
            0.0,
            allowlocal,
            onlydelayed,
            &mut cutoff,
            &mut lperror,
            &mut mustsepa,
            &mut mustprice,
        )?;
        delayed = r.delayed;
    } else {
        // Apply a separation round on the given primal solution.
        (delayed, _) = separation_round_sol(scip, s, sol, actdepth, allowlocal, onlydelayed, &mut cutoff)?;
    }

    Ok((delayed, cutoff))
}

/// What `SCIPpriceLoop` reports.
pub struct PriceLoopResult {
    /// Number of column variables after problem vars were priced.
    pub npricedcolvars: i32,
    /// Whether an unresolved error in LP solving occured.
    pub lperror: bool,
    /// Whether the pricing was aborted and the lower bound must not be used.
    pub aborted: bool,
}

/// `SCIPpriceLoop`: solves the current LP completely with pricing in new
/// variables; `pretendroot`: should the pricers be called as if we are at
/// the root node? `maxpricerounds` is the maximal number of pricing rounds
/// (-1: no limit); a finite limit means that the LP might not be solved to
/// optimality! Sets `mustsepa` if a separation round should follow.
/// (`displayinfo` lines are not printed.)
#[allow(clippy::too_many_arguments)]
pub fn price_loop(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    primal: usize,
    eventfilter: usize,
    pretendroot: bool,
    mut maxpricerounds: i32,
    mustsepa: &mut bool,
) -> ScipResult<PriceLoopResult> {
    use crate::types::LpSolStat;

    let currentnode = crate::tree::tree_get_current_node(scip);
    let mut r = PriceLoopResult {
        npricedcolvars: scip.probs[transprob].ncolvars,
        lperror: false,
        aborted: false,
    };

    // If the LP is unbounded, we don't need to price.
    let solstat = crate::lp::lp_get_solstat(scip);
    let mut mustprice = solstat == LpSolStat::Optimal || solstat == LpSolStat::Infeasible || solstat == LpSolStat::ObjLimit;

    // If all the variables are already in the LP, we don't need to price.
    mustprice = mustprice && !crate::prob::prob_all_cols_in_lp(scip, transprob);

    // Check if infinite number of pricing rounds should be used.
    if maxpricerounds == -1 {
        maxpricerounds = i32::MAX;
    }

    // Pricing (has to be done completely to get a valid lower bound).
    let mut npricerounds = 0;
    while !r.lperror && mustprice && npricerounds < maxpricerounds {
        // Check if pricing loop should be aborted.
        if solve_is_stopped(scip, false) {
            // Do not print the warning message if we stopped because the problem
            // is solved.
            if !scip.set.is_le(api::get_upperbound(scip), api::get_lowerbound(scip)) {
                log::warn!("pricing has been interrupted -- LP of current node is invalid");
            }

            r.aborted = true;
            break;
        }

        // Call primal heuristics which are callable during pricing.
        primal_heuristics(scip, transprob, primal, true, true, NONE, HEURTIMING_DURINGPRICINGLOOP, false)?;

        // Price problem variables.
        crate::pricestore::pricestore_add_prob_vars(scip, transprob)?;
        r.npricedcolvars = scip.probs[transprob].ncolvars;

        // Call external pricers to create additional problem variables.
        let maxvars = scip.set.get_price_maxvars(pretendroot);
        let enoughvars = if maxvars == i32::MAX {
            false
        } else {
            scip.pricestore.as_ref().unwrap().nvars >= maxvars + 1
        };
        let stoppricing = false;
        if scip.set.nactivepricers > 0 && !enoughvars {
            unimplemented!("variable pricers");
        }

        // Apply the priced variables to the LP.
        crate::pricestore::pricestore_apply_vars(scip, transprob)?;
        mustprice = !lp!(scip).flushed || scip.probs[transprob].ncolvars != r.npricedcolvars;
        *mustsepa = *mustsepa || !lp!(scip).flushed;

        // After adding columns, the LP should be primal feasible such that the
        // primal simplex is applicable; if LP was infeasible, we have to use
        // dual simplex.
        r.lperror = crate::lp::lp_solve_and_eval(scip, transprob, -1, false, true, false, false)?;

        // Reset bounds temporarily set by pricer to their original values.
        crate::pricestore::pricestore_reset_bounds(scip)?;

        // Put all initial constraints into the LP.
        init_conss_lp(scip, crate::sepastore::SEPASTORE, transprob, origprob, eventfilter, false, false)?;
        mustprice = mustprice || !lp!(scip).flushed || scip.probs[transprob].ncolvars != r.npricedcolvars;
        *mustsepa = *mustsepa || !lp!(scip).flushed;

        // If all pricers wanted to stop pricing, do not do another pricing round
        // (LP value is no valid dual bound in this case).
        if stoppricing {
            mustprice = false;
            r.aborted = true;
        }

        // Solve LP again after resetting bounds and adding new initial
        // constraints (with dual simplex).
        r.lperror = crate::lp::lp_solve_and_eval(scip, transprob, -1, false, false, false, false)?;

        // Remove previous primal ray, store new one if LP is unbounded.
        update_primal_ray(scip, transprob, primal, r.lperror)?;

        // Increase pricing round counter.
        scip.stat.npricerounds += 1;
        npricerounds += 1;

        // If the LP is unbounded, we can stop pricing.
        let solstat = crate::lp::lp_get_solstat(scip);
        mustprice = mustprice && (solstat == LpSolStat::Optimal || solstat == LpSolStat::Infeasible || solstat == LpSolStat::ObjLimit);

        // If the lower bound is already higher than the cutoff bound, we can stop
        // pricing.
        mustprice = mustprice && scip.set.is_lt(scip.nodes[currentnode].lowerbound, scip.primals[primal].cutoffbound);
    }

    let solstat = crate::lp::lp_get_solstat(scip);
    r.aborted = r.aborted || r.lperror || solstat == LpSolStat::NotSolved || solstat == LpSolStat::Error || npricerounds == maxpricerounds;

    // Set information, whether the current lp is a valid relaxation of the
    // current problem.
    crate::lp::lp_set_is_relax(scip, !r.aborted);

    Ok(r)
}

/// `cutpoolSeparate`: separates cuts of the cut pool `p`;
/// `cutpoolisdelayed`: is the cutpool delayed (count cuts found)? Updates
/// `enoughcuts` and `cutoff`.
#[allow(clippy::too_many_arguments)]
fn cutpool_separate(
    scip: &mut Scip,
    p: usize,
    eventfilter: usize,
    s: usize,
    cutpoolisdelayed: bool,
    root: bool,
    actdepth: i32,
    enoughcuts: &mut bool,
    cutoff: &mut bool,
) -> ScipResult {
    let poolfreq = scip.set.sepa_poolfreq;
    if (poolfreq == 0 && actdepth == 0) || (poolfreq > 0 && actdepth % poolfreq == 0) {
        let result = crate::cutpool::cutpool_separate(scip, p, s, eventfilter, NONE, cutpoolisdelayed, root)?;
        *cutoff = *cutoff || result == ScipResultCode::Cutoff;
        update_enoughcuts(scip, s, root, result, enoughcuts);
    }
    Ok(())
}

/// The flags `priceAndCutLoop` reports.
#[derive(Default)]
struct PriceAndCut {
    /// Whether the node can be cut off.
    cutoff: bool,
    /// Whether an unbounded ray was found in the LP.
    unbounded: bool,
    /// Whether an unresolved error in LP solving occured.
    lperror: bool,
    /// Whether the pricing was aborted and the lower bound must not be used.
    pricingaborted: bool,
}

/// Updates the focus node's lower bound and estimate from the LP solution,
/// and the best root solution at the root.
fn update_lowerbound_from_lp(scip: &mut Scip, focusnode: NodeId, transprob: usize, origprob: usize, eventfilter: usize, root: bool) -> ScipResult {
    crate::tree::node_update_lowerbound_lp(scip, focusnode, transprob, origprob, eventfilter)?;

    // Update node estimate.
    update_estimate(scip);

    if root && crate::lp::lp_get_solstat(scip) == crate::types::LpSolStat::Optimal {
        crate::prob::prob_update_best_root_sol(scip, transprob);
    }
    Ok(())
}

/// `priceAndCutLoop`: solve the current LP of a node with a price-and-cut
/// loop; `fullseparation`: are we in the first prop-and-cut-and-price loop?
/// `forcedlpsolve`: would SCIP abort if the LP is not solved? Sets
/// `propagateagain` if we want to propagate again.
#[allow(clippy::too_many_arguments)]
fn price_and_cut_loop(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    primal: usize,
    eventfilter: usize,
    fullseparation: bool,
    forcedlpsolve: bool,
    propagateagain: &mut bool,
) -> ScipResult<PriceAndCut> {
    use crate::{
        cutpool::{DELAYEDCUTPOOL, GLOBALCUTPOOL},
        sepastore::SEPASTORE,
        types::LpSolStat,
    };

    let mut r = PriceAndCut::default();

    let focusnode = crate::tree::tree_get_focus_node(scip);
    let actdepth = scip.nodes[focusnode].depth as i32;
    let root = actdepth == 0;

    // Check, if we want to separate at this node.
    let loclowerbound = scip.nodes[focusnode].lowerbound;
    let glblowerbound = crate::tree::tree_get_lowerbound(scip);
    let bounddist = (loclowerbound - glblowerbound) / (scip.primals[primal].cutoffbound - glblowerbound);
    let allowlocal = scip.set.is_le(bounddist, scip.set.sepa_maxlocalbounddist);
    let separate = scip.set.sepa_maxruns == -1 || scip.stat.nruns < scip.set.sepa_maxruns;

    // Determine maximal number of separation rounds.
    let mut maxseparounds = if root { scip.set.sepa_maxroundsroot } else { scip.set.sepa_maxrounds };
    if maxseparounds == -1 {
        maxseparounds = i32::MAX;
    }
    if scip.stat.nruns > 1 && root && scip.set.sepa_maxroundsrootsubrun >= 0 {
        maxseparounds = maxseparounds.min(scip.set.sepa_maxroundsrootsubrun);
    }

    // Determine maximal number of partial rounds excluding delayed round.
    let mut maxsepapartialrounds = i32::MAX;
    if !fullseparation && scip.set.sepa_maxaddrounds >= 0 {
        maxsepapartialrounds = scip.stat.nseparounds + scip.set.sepa_maxaddrounds;
    }

    // Determine maximal number of stalling rounds.
    let mut maxsepastallrounds = if root { scip.set.sepa_maxstallroundsroot } else { scip.set.sepa_maxstallrounds };
    if maxsepastallrounds == -1 {
        maxsepastallrounds = i32::MAX;
    }

    // Solve initial LP of price-and-cut loop.
    let itlim = scip.set.lp_iterlim;
    r.lperror = crate::lp::lp_solve_and_eval(scip, transprob, itlim, false, true, false, forcedlpsolve)?;

    // Remove previous primal ray, store new one if LP is unbounded.
    update_primal_ray(scip, transprob, primal, r.lperror)?;

    // Price-and-cut loop.
    let mut npricedcolvars = scip.probs[transprob].ncolvars;
    let mut mustprice = true;
    let mut mustsepa = separate;
    let mut delayedsepa = false;
    r.cutoff = false;
    r.unbounded = crate::lp::lp_get_solstat(scip) == LpSolStat::UnboundedRay;
    let mut nsepastallrounds = 0;
    let mut stalllpsolstat = LpSolStat::NotSolved;
    let mut stalllpobjval = f64::MIN;
    let mut stallnfracs = i32::MAX;
    lp!(scip).installing = false;

    while !r.cutoff && !r.unbounded && !r.lperror && (mustprice || mustsepa) {
        // Solve the LP with pricing in new variables.
        while mustprice && !r.lperror {
            let p = price_loop(scip, transprob, origprob, primal, eventfilter, root, -1, &mut mustsepa)?;
            npricedcolvars = p.npricedcolvars;
            r.lperror = p.lperror;
            r.pricingaborted = p.aborted;

            mustprice = false;

            // Update lower bound w.r.t. the LP solution.
            if !r.lperror && !r.pricingaborted && crate::lp::lp_is_relax(scip) {
                update_lowerbound_from_lp(scip, focusnode, transprob, origprob, eventfilter, root)?;
            }

            if !r.lperror {
                // Call propagators that are applicable during LP solving loop only
                // if the node is not cut off.
                if scip.set.is_lt(scip.nodes[focusnode].lowerbound, scip.primals[primal].cutoffbound) {
                    let oldnboundchgs = scip.stat.nboundchgs;
                    let oldninitconssadded = scip.stat.ninitconssadded;

                    let depth = crate::tree::tree_get_current_depth(scip);
                    (r.cutoff, _) = propagate_domains_internal(scip, depth, 0, false, crate::cons::PROPTIMING_DURINGLPLOOP)?;

                    if scip.stat.ninitconssadded != oldninitconssadded {
                        r.cutoff = init_conss_lp(scip, SEPASTORE, transprob, origprob, eventfilter, false, false)?;
                    }

                    if !r.cutoff && !r.unbounded {
                        // If we found something, solve LP again.
                        if !lp!(scip).flushed {
                            // In the root node, remove redundant rows permanently from
                            // the LP.
                            if root {
                                crate::lp::lp_flush(scip, transprob)?;
                                crate::lp::lp_remove_redundant_rows(scip)?;
                            }

                            // Resolve LP.
                            let itlim = scip.set.lp_iterlim;
                            r.lperror = crate::lp::lp_solve_and_eval(scip, transprob, itlim, false, true, false, false)?;

                            // Remove previous primal ray, store new one if LP is
                            // unbounded.
                            update_primal_ray(scip, transprob, primal, r.lperror)?;

                            mustprice = true;
                            *propagateagain = true;
                        }
                        // Propagation might have changed the best bound of loose
                        // variables, thereby changing the loose objective value which
                        // is added to the LP value; because of the loose status, the LP
                        // might not be reoptimized, but the lower bound of the node
                        // needs to be updated.
                        else if scip.stat.nboundchgs > oldnboundchgs {
                            *propagateagain = true;

                            if lp!(scip).solved && crate::prob::prob_all_cols_in_lp(scip, transprob) && crate::lp::lp_is_relax(scip) {
                                update_lowerbound_from_lp(scip, focusnode, transprob, origprob, eventfilter, root)?;
                            }
                        }
                    }
                }
            }

            // Call primal heuristics that are applicable during node LP solving
            // loop.
            if !r.cutoff && !r.unbounded && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal {
                let (_, unbounded) = primal_heuristics(scip, transprob, primal, true, true, NONE, HEURTIMING_DURINGLPLOOP, false)?;
                r.unbounded = r.unbounded || unbounded;
                r.lperror = r.lperror || lp!(scip).resolvelperror;
            }
        }

        // If we are infeasible, unbounded, exceeded a separation round, the
        // objective, or a global performance limit, we don't need to separate
        // cuts (the global limits are only checked at the root node in order to
        // not query system time too often).
        let solstat = crate::lp::lp_get_solstat(scip);
        mustsepa = mustsepa
            && separate
            && !r.cutoff
            && !r.unbounded
            && scip.stat.nseparounds < maxseparounds
            && (delayedsepa || scip.stat.nseparounds < maxsepapartialrounds)
            && nsepastallrounds < maxsepastallrounds
            && (solstat == LpSolStat::Optimal || solstat == LpSolStat::UnboundedRay)
            && scip.set.is_lt(scip.nodes[focusnode].lowerbound, scip.primals[primal].cutoffbound)
            && (!root || !solve_is_stopped(scip, false));

        // Separation (needs not to be done completely, because we just want to
        // increase the lower bound).
        if mustsepa {
            let olddomchgcount = scip.stat.domchgcount;
            let oldninitconssadded = scip.stat.ninitconssadded;

            mustsepa = false;
            delayedsepa = delayedsepa && scip.stat.nseparounds >= maxsepapartialrounds;
            let mut enoughcuts = scip.set.is_zero(scip.set.get_sepa_maxcuts_gen_factor(root) * scip.set.get_sepa_maxcuts(root) as f64);

            // Global cut pool separation.
            if !enoughcuts && !delayedsepa {
                cutpool_separate(scip, GLOBALCUTPOOL, eventfilter, SEPASTORE, false, root, actdepth, &mut enoughcuts, &mut r.cutoff)?;
            }

            // Separate constraints and LP.
            if !r.cutoff && !enoughcuts {
                // Constraint and LP separation: apply a separation round.
                let s = separation_round_lp(
                    scip,
                    transprob,
                    primal,
                    SEPASTORE,
                    actdepth,
                    bounddist,
                    allowlocal,
                    delayedsepa,
                    &mut r.cutoff,
                    &mut r.lperror,
                    &mut mustsepa,
                    &mut mustprice,
                )?;
                delayedsepa = s.delayed;
                enoughcuts = s.enoughcuts;

                // If we are in the last separation or stall round, also call the
                // delayed separators.
                let solstat = crate::lp::lp_get_solstat(scip);
                if !r.cutoff
                    && !r.lperror
                    && lp!(scip).solved
                    && !enoughcuts
                    && delayedsepa
                    && (scip.stat.nseparounds + 1 >= maxseparounds || nsepastallrounds + 1 >= maxsepastallrounds)
                    && (solstat == LpSolStat::Optimal || solstat == LpSolStat::UnboundedRay)
                {
                    let s = separation_round_lp(
                        scip,
                        transprob,
                        primal,
                        SEPASTORE,
                        actdepth,
                        bounddist,
                        allowlocal,
                        delayedsepa,
                        &mut r.cutoff,
                        &mut r.lperror,
                        &mut mustsepa,
                        &mut mustprice,
                    )?;
                    delayedsepa = s.delayed;
                    enoughcuts = s.enoughcuts;
                }
            }

            // Call global cut pool separation again since separators may add cuts
            // to the pool instead of the sepastore.
            if !r.cutoff && !r.lperror && lp!(scip).solved && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal && !enoughcuts {
                cutpool_separate(scip, GLOBALCUTPOOL, eventfilter, SEPASTORE, false, root, actdepth, &mut enoughcuts, &mut r.cutoff)?;
            }

            // Delayed global cut pool separation.
            if !r.cutoff && !r.lperror && lp!(scip).solved && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal && scip.sepastores[SEPASTORE].ncuts == 0 && !enoughcuts {
                cutpool_separate(scip, DELAYEDCUTPOOL, eventfilter, SEPASTORE, true, root, actdepth, &mut enoughcuts, &mut r.cutoff)?;
            }

            // Delayed separation if no cuts where produced.
            if !r.cutoff && !r.lperror && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal && scip.sepastores[SEPASTORE].ncuts == 0 && delayedsepa {
                let s = separation_round_lp(
                    scip,
                    transprob,
                    primal,
                    SEPASTORE,
                    actdepth,
                    bounddist,
                    allowlocal,
                    delayedsepa,
                    &mut r.cutoff,
                    &mut r.lperror,
                    &mut mustsepa,
                    &mut mustprice,
                )?;
                delayedsepa = s.delayed;
                enoughcuts = s.enoughcuts;

                // Call delayed cut pool separation again, since separators may add
                // cuts to the pool instead of the sepastore.
                if !r.cutoff && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal {
                    cutpool_separate(scip, DELAYEDCUTPOOL, eventfilter, SEPASTORE, true, root, actdepth, &mut enoughcuts, &mut r.cutoff)?;
                }
            }

            let solstat = crate::lp::lp_get_solstat(scip);
            if r.cutoff || r.lperror || solstat == LpSolStat::Infeasible || solstat == LpSolStat::ObjLimit || solstat == LpSolStat::IterLimit || solstat == LpSolStat::TimeLimit {
                // The found cuts are of no use, because the node is infeasible
                // anyway (or we have an error in the LP).
                crate::sepastore::sepastore_clear_cuts(scip, SEPASTORE, eventfilter)?;
            } else {
                // Apply found cuts.
                r.cutoff = crate::sepastore::sepastore_apply_cuts(scip, SEPASTORE, transprob, origprob, eventfilter, root)?;

                if !r.cutoff {
                    mustprice = mustprice || !lp!(scip).flushed || scip.probs[transprob].ncolvars != npricedcolvars;
                    mustsepa = mustsepa || !lp!(scip).flushed;

                    // If a new bound change (e.g. a cut with only one column) was
                    // found, propagate domains again.
                    if scip.stat.domchgcount != olddomchgcount {
                        *propagateagain = true;

                        // In the root node, remove redundant rows permanently from the
                        // LP.
                        if root {
                            crate::lp::lp_flush(scip, transprob)?;
                            crate::lp::lp_remove_redundant_rows(scip)?;
                        }
                    }

                    if scip.stat.ninitconssadded != oldninitconssadded {
                        r.cutoff = init_conss_lp(scip, SEPASTORE, transprob, origprob, eventfilter, false, false)?;
                    }

                    if !r.cutoff {
                        // Solve LP (with dual simplex).
                        let itlim = scip.set.lp_iterlim;
                        r.lperror = crate::lp::lp_solve_and_eval(scip, transprob, itlim, false, true, false, false)?;

                        // Remove previous primal ray, store new one if LP is unbounded.
                        update_primal_ray(scip, transprob, primal, r.lperror)?;

                        if !r.lperror {
                            // Propagation might have changed the best bound of loose
                            // variables, thereby changing the loose objective value
                            // which is added to the LP value; because of the loose
                            // status, the LP might not be reoptimized, but the lower
                            // bound of the node needs to be updated.
                            if scip.stat.domchgcount != olddomchgcount
                                && (!mustprice || mustsepa)
                                && !r.cutoff
                                && crate::prob::prob_all_cols_in_lp(scip, transprob)
                                && crate::lp::lp_is_relax(scip)
                            {
                                update_lowerbound_from_lp(scip, focusnode, transprob, origprob, eventfilter, root)?;
                            }

                            // Check if we are stalling. If we have an LP solution, then
                            // we are stalling if we had an LP solution before and the LP
                            // value did not improve and the number of fractional
                            // variables did not decrease. If we do not have an LP
                            // solution, then we are stalling if the solution status of
                            // the LP did not change.
                            let stalling;
                            if crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal {
                                let nfracs = crate::branch::branchcand_get_lp_cands(scip).nlpcands;
                                let lpobjval = crate::lp::lp_get_objval(scip, transprob);
                                let objreldiff = crate::misc::rel_diff(lpobjval, stalllpobjval);

                                stalling =
                                    stalllpsolstat == LpSolStat::Optimal && objreldiff <= 1e-04 && nfracs as f64 >= (0.9 - 0.1 * nsepastallrounds as f64) * stallnfracs as f64;

                                stalllpobjval = lpobjval;
                                stallnfracs = nfracs;
                            } else {
                                stalling = stalllpsolstat == crate::lp::lp_get_solstat(scip);
                            }

                            if !stalling {
                                nsepastallrounds = 0;
                                lp!(scip).installing = false;
                            } else {
                                nsepastallrounds += 1;
                            }
                            stalllpsolstat = crate::lp::lp_get_solstat(scip);

                            // Tell LP that we are stalling.
                            if nsepastallrounds + 1 >= maxsepastallrounds {
                                lp!(scip).installing = true;
                            }
                        }
                    }
                }
            }

            // Increase separation round counter.
            scip.stat.nseparounds += 1;
        }
    }

    if root && nsepastallrounds >= maxsepastallrounds {
        log::info!("Truncate separation round because of stalling ({} stall rounds).", maxsepastallrounds);
    }

    if !r.lperror {
        // Update pseudo cost values for continuous variables, if it should be
        // delayed.
        let delay = scip.set.branch_delaypscost;
        update_pseudocost(scip, transprob, false, delay)?;
    }

    // Update lower bound w.r.t. the LP solution.
    if !r.cutoff && !r.lperror {
        if crate::lp::lp_is_relax(scip) {
            crate::tree::node_update_lowerbound_lp(scip, focusnode, transprob, origprob, eventfilter)?;
        }

        // Update node estimate.
        update_estimate(scip);

        // Issue LPSOLVED event.
        let solstat = crate::lp::lp_get_solstat(scip);
        if solstat != LpSolStat::IterLimit && solstat != LpSolStat::TimeLimit {
            let event = crate::event::event_create_node(crate::event::EVENTTYPE_LPSOLVED, focusnode);
            crate::event::event_process(scip, &event, eventfilter)?;
        }

        // If the LP is a relaxation and we are not solving exactly, then we may
        // analyze an infeasible or bound exceeding LP (not necessary in the root
        // node) and cut off the current node.
        let solstat = crate::lp::lp_get_solstat(scip);
        if !root && crate::lp::lp_is_relax(scip) && crate::prob::prob_all_cols_in_lp(scip, transprob) && (solstat == LpSolStat::Infeasible || solstat == LpSolStat::ObjLimit) {
            crate::conflict::conflict_analyze_lp_public(scip, transprob, origprob, eventfilter)?;
            r.cutoff = true;
        }
    }

    // Check for unboundedness.
    if !r.lperror {
        r.unbounded = crate::lp::lp_get_solstat(scip) == LpSolStat::UnboundedRay;
    }

    lp!(scip).installing = false;

    // Check for cutoff.
    if r.cutoff {
        crate::tree::node_cutoff(scip, focusnode, eventfilter, transprob, origprob)?;
    }

    Ok(r)
}

/// `applyBounding`: updates the current lower bound with the pseudo
/// objective value, cuts off node by bounding, and applies conflict analysis
/// if the pseudo objective lead to the cutoff; updates `cutoff`.
fn apply_bounding(scip: &mut Scip, transprob: usize, origprob: usize, primal: usize, eventfilter: usize, cutoff: &mut bool) -> ScipResult {
    if !*cutoff {
        // Get current focus node.
        let focusnode = crate::tree::tree_get_focus_node(scip);

        // Update lower bound w.r.t. the pseudo solution.
        let pseudoobjval = crate::lp::lp_get_pseudo_objval(scip, transprob);
        crate::tree::node_update_lowerbound(scip, focusnode, eventfilter, transprob, origprob, pseudoobjval)?;

        // Check for infeasible node by bounding.
        let cutoffbound = scip.primals[primal].cutoffbound;
        if scip.set.is_ge(scip.nodes[focusnode].lowerbound, cutoffbound) {
            *cutoff = true;

            // Call pseudo conflict analysis, if the node is cut off due to the
            // pseudo objective value.
            if !scip.set.is_infinity(-pseudoobjval) && !scip.set.is_infinity(cutoffbound) && scip.set.is_ge(pseudoobjval, cutoffbound) {
                crate::conflict::conflict_analyze_pseudo(scip, transprob, origprob, eventfilter)?;
            }
        }
    }
    Ok(())
}

/// `markRelaxsUnsolved`: marks all relaxators to be unsolved.
fn mark_relaxs_unsolved(scip: &mut Scip) {
    crate::relax::relaxation_set_sol_valid(scip, false, false);
    for r in 0..scip.set.nrelaxs as usize {
        let relax = scip.set.relaxs[r];
        crate::relax::relax_mark_unsolved(scip, relax);
    }
}

/// The flags `solveNodeLP` reports.
#[derive(Default)]
struct NodeLp {
    cutoff: bool,
    unbounded: bool,
    lperror: bool,
    pricingaborted: bool,
}

/// `solveNodeLP`: solves the current node's LP in a price-and-cut loop;
/// `initiallpsolved`: was the initial LP already solved? `fullseparation`:
/// are we in the first prop-and-cut-and-price loop? `newinitconss`: do we
/// have to add new initial constraints? `forcedlpsolve`: would SCIP abort if
/// the LP is not solved? Sets `propagateagain` and `solverelaxagain`.
#[allow(clippy::too_many_arguments)]
fn solve_node_lp(
    scip: &mut Scip,
    origprob: usize,
    transprob: usize,
    primal: usize,
    eventfilter: usize,
    initiallpsolved: bool,
    fullseparation: bool,
    newinitconss: bool,
    forcedlpsolve: bool,
    propagateagain: &mut bool,
    solverelaxagain: &mut bool,
    unbounded: &mut bool,
) -> ScipResult<NodeLp> {
    use crate::{sepastore::SEPASTORE, types::LpSolStat};

    let mut r = NodeLp {
        unbounded: *unbounded,
        ..NodeLp::default()
    };

    let nlps = scip.stat.nlps;
    let nzeroitlps = scip.stat.ndualzeroitlps + scip.stat.nprimalzeroitlps + scip.stat.nbarrierzeroitlps;
    let nlpiterations = scip.stat.nlpiterations;

    if !initiallpsolved {
        // Load and solve the initial LP of the node.
        (r.cutoff, r.lperror) = solve_node_initial_lp(scip, transprob, origprob, primal, eventfilter, newinitconss, forcedlpsolve)?;

        // Update initial LP iteration counter.
        scip.stat.ninitlps += scip.stat.nlps - nlps;
        scip.stat.ninitlpiterations += scip.stat.nlpiterations - nlpiterations;

        // In the root node, we try if the initial LP solution is feasible to
        // avoid expensive setup of data structures in separators; in case the
        // root LP is aborted, e.g., by hitting the time limit, we do not check
        // the LP solution since the corresponding data structures have not been
        // updated.
        let solstat = crate::lp::lp_get_solstat(scip);
        if crate::tree::tree_get_current_depth(scip) == 0
            && !r.cutoff
            && !r.lperror
            && (solstat == LpSolStat::Optimal || solstat == LpSolStat::UnboundedRay)
            && !solve_is_stopped(scip, false)
        {
            let oldnbestsolsfound = scip.primals[primal].nbestsolsfound;

            let sol = crate::sol::sol_create_lp_sol(scip, transprob, primal, NONE)?;

            let checklprows = crate::lp::lp_get_solstat(scip) != LpSolStat::UnboundedRay;

            let flags = crate::primal::CheckFlags {
                printreason: false,
                completely: false,
                checkbounds: true,
                checkintegrality: true,
                checklprows,
            };
            let stored = crate::primal::primal_try_sol_free(scip, primal, origprob, transprob, eventfilter, sol, flags)?;

            if stored {
                scip.stat.nlpsolsfound += 1;

                if scip.primals[primal].nbestsolsfound != oldnbestsolsfound {
                    scip.stat.nlpbestsolsfound += 1;
                    api::store_solution_gap(scip);
                }
            }

            if crate::lp::lp_get_solstat(scip) == LpSolStat::UnboundedRay {
                r.unbounded = true;
            }
        }
    } else {
        r.cutoff = init_conss_lp(scip, SEPASTORE, transprob, origprob, eventfilter, false, false)?;
    }

    // Check for infeasible node by bounding.
    apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut r.cutoff)?;

    if !r.cutoff && !r.lperror {
        let oldnpricedvars = scip.probs[transprob].ncolvars;
        let oldninitconssadded = scip.stat.ninitconssadded;
        let oldncutsapplied = scip.sepastores[SEPASTORE].ncutsapplied;
        let oldnboundchgs = scip.stat.nboundchgs;
        let olddomchgcount = scip.stat.domchgcount;

        // Solve the LP with price-and-cut.
        let p = price_and_cut_loop(scip, transprob, origprob, primal, eventfilter, fullseparation, forcedlpsolve, propagateagain)?;
        r.cutoff = p.cutoff;
        r.unbounded = p.unbounded;
        r.lperror = p.lperror;
        r.pricingaborted = p.pricingaborted;

        // Check if the problem was changed and the relaxation needs to be
        // resolved.
        if scip.probs[transprob].ncolvars != oldnpricedvars
            || scip.stat.ninitconssadded != oldninitconssadded
            || scip.sepastores[SEPASTORE].ncutsapplied != oldncutsapplied
            || scip.stat.nboundchgs != oldnboundchgs
            || scip.stat.domchgcount != olddomchgcount
        {
            *solverelaxagain = true;
            mark_relaxs_unsolved(scip);
        }
    }

    // If pricing was aborted while solving the LP of the node and the node
    // cannot be cut off due to the lower bound computed by the pricer, the
    // solving of the LP might be stopped due to the objective limit, but the
    // node may not be cut off, since the LP objective is not a feasible lower
    // bound for the solutions in the current subtree. In this case, the LP has
    // to be solved to optimality by temporarily removing the cutoff bound.
    let solstat = crate::lp::lp_get_solstat(scip);
    if r.pricingaborted && (solstat == LpSolStat::ObjLimit || solstat == LpSolStat::IterLimit) && !r.cutoff {
        // Temporarily disable cutoffbound, which also disables the objective
        // limit.
        let tmpcutoff = lp!(scip).cutoffbound;
        let inf = crate::lpi::lpi_infinity(lp!(scip).lpi.as_deref().unwrap());
        lp!(scip).cutoffbound = inf;

        lp!(scip).solved = false;
        r.lperror = crate::lp::lp_solve_and_eval(scip, transprob, -1, false, false, false, forcedlpsolve)?;

        // Reinstall old cutoff bound.
        lp!(scip).cutoffbound = tmpcutoff;

        // Lp solstat should not be objlimit, since the cutoff bound was removed
        // temporarily; lp solstat should not be unboundedray, since the lp was
        // dual feasible; there should be no primal ray, since the lp was dual
        // feasible.
        if crate::lp::lp_get_solstat(scip) == LpSolStat::Infeasible {
            r.cutoff = true;
        }
    }

    // Update node's LP iteration counter.
    let s = &mut scip.stat;
    s.nnodelps += s.nlps - nlps;
    s.nnodezeroitlps += s.ndualzeroitlps + s.nprimalzeroitlps + s.nbarrierzeroitlps - nzeroitlps;
    s.nnodelpiterations += s.nlpiterations - nlpiterations;

    // Update number of root node LPs and iterations if the root node was
    // processed.
    let focusnode = treer!(scip).focusnode;
    if scip.nodes[focusnode].depth == 0 {
        let s = &mut scip.stat;
        s.nrootlps += s.nlps - nlps;
        s.nrootlpiterations += s.nlpiterations - nlpiterations;
    }

    *unbounded = r.unbounded;
    Ok(r)
}

/// `solveNodeRelax`: calls relaxators; `beforelp`: should the relaxators
/// with non-negative or negative priority be called? Updates `cutoff`,
/// `propagateagain`, `solvelpagain`, `solverelaxagain`, and sets
/// `relaxcalled` if at least one relaxator was called.
#[allow(clippy::too_many_arguments)]
fn solve_node_relax(
    scip: &mut Scip,
    eventfilter: usize,
    transprob: usize,
    origprob: usize,
    depth: i32,
    beforelp: bool,
    cutoff: &mut bool,
    propagateagain: &mut bool,
    solvelpagain: &mut bool,
    solverelaxagain: &mut bool,
    relaxcalled: &mut bool,
) -> ScipResult {
    use ScipResultCode as R;

    // Sort by priority.
    crate::relax::set_sort_relaxs(scip);

    let mut r = 0;
    while r < scip.set.nrelaxs as usize && !*cutoff {
        let relax = scip.set.relaxs[r];
        r += 1;

        if beforelp != (scip.relaxs[relax].priority >= 0) {
            continue;
        }

        *relaxcalled = true;

        let mut lowerbound = -scip.set.infinity();

        let result = crate::relax::relax_exec(scip, relax, depth, &mut lowerbound)?;

        match result {
            R::Cutoff => {
                *cutoff = true;
                // @todo does it make sense to proceed if the node is proven to be
                // infeasible?
                return Ok(());
            }
            R::ConsAdded => {
                // The separation for new constraints should be called.
                *solvelpagain = true;
                // The propagation for new constraints should be called.
                *propagateagain = true;
            }
            R::ReducedDom => {
                *solvelpagain = true;
                *propagateagain = true;
            }
            R::Separated => *solvelpagain = true,
            R::Suspended => *solverelaxagain = true,
            R::Success | R::DidNotRun => {}
            _ => {
                log::error!("invalid result code <{:?}> of relaxator <{}>", result, scip.relaxs[relax].name);
                return Err(crate::retcode::Retcode::InvalidResult);
            }
        }

        if result != R::Cutoff && result != R::DidNotRun && result != R::Suspended {
            let focusnode = crate::tree::tree_get_focus_node(scip);

            // Update lower bound w.r.t. the lower bound given by the relaxator.
            crate::tree::node_update_lowerbound(scip, focusnode, eventfilter, transprob, origprob, lowerbound)?;
        }
    }

    Ok(())
}

/// The flags `enforceConstraints` reports and updates.
struct EnforceFlags<'a> {
    /// Whether a branching was created.
    branched: &'a mut bool,
    /// Whether the node can be cut off.
    cutoff: &'a mut bool,
    /// Whether the LP/pseudo solution is infeasible.
    infeasible: &'a mut bool,
    /// Whether domain propagation should be applied again.
    propagateagain: &'a mut bool,
    /// Whether the node's LP has to be solved again.
    solvelpagain: &'a mut bool,
    /// Whether the external relaxators should be called again.
    solverelaxagain: &'a mut bool,
}

/// `enforceConstraints`: enforces constraints by branching, separation, or
/// domain reduction; `forced`: should enforcement of pseudo solution be
/// forced?
fn enforce_constraints(scip: &mut Scip, prob: usize, primal: usize, f: EnforceFlags, forced: bool) -> ScipResult {
    use ScipResultCode as R;

    use crate::{sepastore::SEPASTORE, types::LpSolStat};

    *f.branched = false;

    // @todo avoid checking the same pseudosolution twice

    // Enforce (best) relaxation solution if the LP has a worse objective
    // value.
    let mut enforcerelaxsol = relax_sol_preferred(scip, prob);

    // Check if all constraint handlers implement the enforelax callback,
    // otherwise enforce the LP solution.
    let mut h = 0;
    while h < scip.set.nconshdlrs as usize && enforcerelaxsol {
        let c = &scip.conshdlrs[scip.set.conshdlrs_enfo[h]];
        if c.consenforelax.is_none() && (!c.needscons || c.nconss > 0) {
            enforcerelaxsol = false;

            if !scip.stat.disableenforelaxmsg && scip.set.disp_verblevel == 4 {
                // Remember that the disable relaxation enforcement message was
                // posted and only post it again if the verblevel is
                // SCIP_VERBLEVEL_FULL.
                scip.stat.disableenforelaxmsg = true;
            }
            log::info!(
                "Disable enforcement of relaxation solutions since constraint handler {} does not implement enforelax-callback",
                scip.conshdlrs[scip.set.conshdlrs_enfo[h]].name
            );
        }
        h += 1;
    }

    // Enforce constraints by branching, applying additional cutting planes (if
    // LP is being processed), introducing new constraints, or tighten the
    // domains.

    // Check, if the solution is infeasible anyway due to it's objective value.
    let objinfeasible = if crate::tree::tree_has_focus_node_lp(scip) || enforcerelaxsol {
        false
    } else {
        let pseudoobjval = crate::lp::lp_get_pseudo_objval(scip, prob);
        let focusnode = crate::tree::tree_get_focus_node(scip);
        scip.set.is_dualfeas_lt(pseudoobjval, scip.nodes[focusnode].lowerbound)
    };

    // During constraint enforcement, generated cuts should enter the LP in any
    // case; otherwise, a constraint handler would fail to enforce its
    // constraints if it relies on the modification of the LP relaxation.
    crate::sepastore::sepastore_start_force_cuts(scip, SEPASTORE);

    // Enforce constraints until a handler resolved an infeasibility with
    // cutting off the node, branching, reducing a domain, or separating a cut;
    // if a constraint handler introduced new constraints to enforce his
    // constraints, the newly added constraints have to be enforced themselves.
    let mut resolved = false;

    // In case the relaxation solution should be enforced, we need to create the
    // corresponding solution for the enforelax callbacks.
    let relaxsol = if enforcerelaxsol { crate::sol::sol_create_relax_sol(scip, primal, NONE)? } else { NONE };

    let mut h = 0;
    while h < scip.set.nconshdlrs as usize && !resolved {
        let hdlr = scip.set.conshdlrs_enfo[h];
        h += 1;

        // Enforce LP, pseudo, or relaxation solution.
        let result;
        if enforcerelaxsol {
            result = crate::cons::conshdlr_enforce_relax_sol(scip, hdlr, relaxsol, *f.infeasible)?;
        } else if crate::tree::tree_has_focus_node_lp(scip) {
            // NOTE: May be called with lp->primalfeasible=FALSE. But then
            // enforcement is called for infinite points?!
            result = crate::cons::conshdlr_enforce_lp_sol(scip, hdlr, *f.infeasible)?;
        } else {
            result = crate::cons::conshdlr_enforce_pseudo_sol(scip, hdlr, *f.infeasible, objinfeasible, forced)?;
            if scip.sepastores[SEPASTORE].ncuts != 0 {
                log::error!("pseudo enforcing method of constraint handler <{}> separated cuts", scip.conshdlrs[hdlr].name);
                return Err(crate::retcode::Retcode::InvalidResult);
            }
        }

        match result {
            R::Cutoff => {
                *f.cutoff = true;
                *f.infeasible = true;
                resolved = true;
            }
            R::ConsAdded => {
                *f.infeasible = true;
                // The propagation for new constraints should be called.
                *f.propagateagain = true;
                // The separation for new constraints should be called.
                *f.solvelpagain = true;
                *f.solverelaxagain = true;
                mark_relaxs_unsolved(scip);
                resolved = true;
            }
            R::ReducedDom => {
                *f.infeasible = true;
                *f.propagateagain = true;
                *f.solvelpagain = true;
                *f.solverelaxagain = true;
                mark_relaxs_unsolved(scip);
                resolved = true;
            }
            R::Separated => {
                *f.infeasible = true;
                *f.solvelpagain = true;
                *f.solverelaxagain = true;
                mark_relaxs_unsolved(scip);
                resolved = true;
            }
            R::Branched => {
                *f.infeasible = true;
                *f.branched = true;
                resolved = true;

                // Increase the number of internal nodes.
                scip.stat.ninternalnodes += 1;
                scip.stat.ntotalinternalnodes += 1;
            }
            R::SolveLp => {
                // Either LP was not solved, or it is not solved anymore (e.g.,
                // because feastol has been tightened by some constraint handler).
                *f.infeasible = true;
                *f.solvelpagain = true;
                resolved = true;
                // The node's LP must be solved.
                crate::tree::tree_set_focus_node_lp(scip, true);
            }
            R::Infeasible => *f.infeasible = true,
            R::Feasible => {}
            R::DidNotRun => *f.infeasible = true,
            _ => {
                log::error!(
                    "invalid result code <{:?}> from enforcing method of constraint handler <{}>",
                    result,
                    scip.conshdlrs[hdlr].name
                );
                return Err(crate::retcode::Retcode::InvalidResult);
            }
        }

        // The enforcement method may add a primal solution, after which the LP
        // status could be set to objective limit reached.
        if crate::tree::tree_has_focus_node_lp(scip) && crate::lp::lp_get_solstat(scip) == LpSolStat::ObjLimit {
            *f.cutoff = true;
            *f.infeasible = true;
            resolved = true;

            // If we used the probing mode during branching, it might happen that
            // we added a constraint or global bound and returned SCIP_CONSADDED
            // or SCIP_REDUCEDDOM, but when reoptimizing the LP after ending the
            // probing mode, this leads to hitting the objective limit. In this
            // case, we do not need to propagate or solve the LP again.
            *f.propagateagain = false;
            *f.solvelpagain = false;
        }
    }

    // In case the relaxation solution was enforced, free the created solution.
    if enforcerelaxsol {
        crate::sol::sol_free(scip, relaxsol, primal)?;
    }

    // Deactivate the cut forcing of the constraint enforcement.
    crate::sepastore::sepastore_end_force_cuts(scip, SEPASTORE);

    Ok(())
}

/// `applyCuts`: applies the cuts stored in the separation store, or clears
/// the store if the node can be cut off; `root`: is this the initial root
/// LP?
#[allow(clippy::too_many_arguments)]
fn apply_cuts(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    root: bool,
    cutoff: &mut bool,
    propagateagain: &mut bool,
    solvelpagain: &mut bool,
    solverelaxagain: &mut bool,
) -> ScipResult {
    use crate::sepastore::SEPASTORE;

    if *cutoff {
        // The found cuts are of no use, because the node is infeasible anyway
        // (or we have an error in the LP).
        crate::sepastore::sepastore_clear_cuts(scip, SEPASTORE, eventfilter)?;
    } else if scip.sepastores[SEPASTORE].ncuts > 0 {
        let olddomchgcount = scip.stat.domchgcount;
        let oldncutsapplied = scip.sepastores[SEPASTORE].ncutsapplied;

        *cutoff = crate::sepastore::sepastore_apply_cuts(scip, SEPASTORE, transprob, origprob, eventfilter, root)?;
        *propagateagain = *propagateagain || scip.stat.domchgcount != olddomchgcount;
        *solvelpagain = true;

        if scip.stat.domchgcount != olddomchgcount || scip.sepastores[SEPASTORE].ncutsapplied != oldncutsapplied {
            *solverelaxagain = true;
            mark_relaxs_unsolved(scip);
        }
    }
    Ok(())
}

/// `updateLoopStatus`: updates the cutoff, propagateagain, and
/// solverelaxagain status of the current solving loop.
fn update_loop_status(scip: &Scip, depth: i32, cutoff: &mut bool, propagateagain: &mut bool, solverelaxagain: &mut bool) {
    let tree = &treer!(scip);

    // Check, if the path was cutoff.
    *cutoff = *cutoff || tree.cutoffdepth <= depth;

    // Check if branching was already performed.
    if tree.nchildren == 0 {
        // Check, if the focus node should be repropagated.
        let focusnode = crate::tree::tree_get_focus_node(scip);
        *propagateagain = *propagateagain || scip.nodes[focusnode].reprop;

        // Check, if one of the external relaxations should be solved again.
        let mut r = 0;
        while r < scip.set.nrelaxs as usize && !*solverelaxagain {
            *solverelaxagain = *solverelaxagain || !crate::relax::relax_is_solved(scip, scip.set.relaxs[r]);
            r += 1;
        }
    } else {
        // If branching was performed, avoid another node loop iteration.
        *propagateagain = false;
        *solverelaxagain = false;
    }
}

/// The state `propAndSolve` carries between iterations of the node loop.
#[derive(Default)]
struct NodeLoop {
    /// The last LP count for which AFTERLP propagation was performed.
    afterlpproplps: i64,
    /// Timing for running heuristics after propagation call.
    heurtiming: HeurTiming,
    /// The number of lp errors.
    nlperrors: i32,
    /// Whether we want to do a fullpropagation next time.
    fullpropagation: bool,
    /// Whether we want to propagate again.
    propagateagain: bool,
    /// Whether the lp was solved.
    lpsolved: bool,
    /// Whether a relaxator was called; initialized with last loop's result.
    relaxcalled: bool,
    /// Whether we want to solve the lp again.
    solvelpagain: bool,
    /// Whether we want to solve the relaxation again.
    solverelaxagain: bool,
    /// Whether the node can be cut off.
    cutoff: bool,
    /// Whether the node should be postponed.
    postpone: bool,
    /// Whether the focus node is unbounded.
    unbounded: bool,
    /// Whether solving was interrupted.
    stopped: bool,
    /// Whether an unresolved error in LP solving occured.
    lperror: bool,
    /// Whether the pricing was aborted and the lower bound must not be used.
    pricingaborted: bool,
    /// Whether the enforcement of pseudo solution should be forced.
    forcedenforcement: bool,
}

/// `propAndSolve`: propagate domains and solve relaxation and lp;
/// `propagate`: should we propagate? `solvelp`: should we solve the lp?
/// `solverelax`: should we solve the relaxation? `forcedlpsolve`: would SCIP
/// abort if the LP is not solved? `initiallpsolved`: was the initial LP
/// already solved? `fullseparation`: are we in the first
/// prop-and-cut-and-price loop?
#[allow(clippy::too_many_arguments)]
fn prop_and_solve(
    scip: &mut Scip,
    origprob: usize,
    transprob: usize,
    primal: usize,
    eventfilter: usize,
    focusnode: NodeId,
    actdepth: i32,
    mut propagate: bool,
    mut solvelp: bool,
    mut solverelax: bool,
    forcedlpsolve: bool,
    initiallpsolved: bool,
    fullseparation: bool,
    l: &mut NodeLoop,
) -> ScipResult {
    use crate::{
        cons::{PROPTIMING_AFTERLPLOOP, PROPTIMING_BEFORELP},
        types::LpSolStat,
    };

    let mut newinitconss = false;

    if !l.cutoff && !l.postpone {
        let lpwasflushed = lp!(scip).flushed;
        let oldnboundchgs = scip.stat.nboundchgs;
        let oldninitconssadded = scip.stat.ninitconssadded;

        // Call after LP propagators.
        if (l.afterlpproplps < scip.stat.nnodelps && l.lpsolved) || l.relaxcalled {
            let depth = crate::tree::tree_get_current_depth(scip);
            (l.cutoff, l.postpone) = propagate_domains_internal(scip, depth, 0, l.fullpropagation, PROPTIMING_AFTERLPLOOP)?;

            // Check, if the path was cutoff.
            l.cutoff = l.cutoff || treer!(scip).cutoffdepth <= actdepth;
            l.afterlpproplps = scip.stat.nnodelps;
            propagate = propagate || scip.stat.nboundchgs > oldnboundchgs;
        }

        // Call before LP propagators.
        if propagate && !l.cutoff {
            let depth = crate::tree::tree_get_current_depth(scip);
            (l.cutoff, l.postpone) = propagate_domains_internal(scip, depth, 0, l.fullpropagation, PROPTIMING_BEFORELP)?;
        }

        newinitconss = scip.stat.ninitconssadded != oldninitconssadded;
        l.fullpropagation = false;

        // Check, if the path was cutoff.
        l.cutoff = l.cutoff || treer!(scip).cutoffdepth <= actdepth;

        // If the LP was flushed and is now no longer flushed, a bound change
        // occurred, and the LP has to be resolved; we also have to solve the LP
        // if new intial constraints were added which need to be added to the LP.
        solvelp = solvelp || (lpwasflushed && (!lp!(scip).flushed || newinitconss));
        solverelax = solverelax || newinitconss;

        // The number of bound changes was increased by the propagation call, thus
        // the relaxation should be solved again.
        if scip.stat.nboundchgs > oldnboundchgs {
            // Propagation might have changed the best bound of loose variables,
            // thereby changing the loose objective value which is added to the LP
            // value; because of the loose status, the LP might not be
            // reoptimized, but the lower bound of the node needs to be updated.
            if !solvelp && lp!(scip).flushed && lp!(scip).solved && crate::prob::prob_all_cols_in_lp(scip, transprob) && crate::lp::lp_is_relax(scip) {
                crate::tree::node_update_lowerbound_lp(scip, focusnode, transprob, origprob, eventfilter)?;

                if crate::tree::tree_has_focus_node_lp(scip) {
                    // Update node estimate.
                    update_estimate(scip);

                    if actdepth == 0 && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal {
                        crate::prob::prob_update_best_root_sol(scip, transprob);
                    }
                }
            }

            solverelax = true;
            mark_relaxs_unsolved(scip);
        }

        // Update lower bound with the pseudo objective value, and cut off node by
        // bounding.
        apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut l.cutoff)?;
    }

    if l.postpone {
        return Ok(());
    }

    // Call primal heuristics that are applicable after propagation loop before
    // lp solve; the first time we go here, we call the before node heuristics
    // instead.
    if !l.cutoff && !crate::tree::tree_probing(scip) {
        // If the heuristics find a new incumbent solution, propagate again.
        let unbounded;
        (l.propagateagain, unbounded) = primal_heuristics(scip, transprob, primal, true, true, NONE, l.heurtiming, false)?;
        l.unbounded = l.unbounded || unbounded;

        l.heurtiming = HEURTIMING_AFTERPROPLOOP;

        // Check if primal heuristics found a solution and we therefore reached a
        // solution limit.
        if solve_is_stopped(scip, false) {
            // We reached a solution limit and do not want to continue the
            // processing of the current node, but in order to allow restarting the
            // optimization process later, we need to create a "branching" with
            // only one child node that is a copy of the focusnode.
            crate::tree::tree_set_focus_node_lp(scip, false);
            let estimate = scip.nodes[focusnode].estimate;
            crate::tree::node_create_child(scip, 1.0, estimate)?;
            l.stopped = true;
            return Ok(());
        }

        // If diving produced an LP error, switch back to non-LP node.
        if lp!(scip).resolvelperror {
            crate::tree::tree_set_focus_node_lp(scip, false);
            lp!(scip).resolvelperror = false;
        }

        if l.propagateagain {
            l.solvelpagain = solvelp;
            l.solverelaxagain = solverelax;
            return Ok(());
        }
    }

    // Solve external relaxations with non-negative priority.
    l.relaxcalled = false;
    if solverelax && !l.cutoff {
        // Clear the storage of external branching candidates.
        crate::branch::branchcand_clear_extern_cands(scip);

        solve_node_relax(
            scip,
            eventfilter,
            transprob,
            origprob,
            actdepth,
            true,
            &mut l.cutoff,
            &mut l.propagateagain,
            &mut l.solvelpagain,
            &mut l.solverelaxagain,
            &mut l.relaxcalled,
        )?;

        // Check, if the path was cutoff.
        l.cutoff = l.cutoff || treer!(scip).cutoffdepth <= actdepth;

        // Apply found cuts.
        apply_cuts(
            scip,
            transprob,
            origprob,
            eventfilter,
            actdepth == 0,
            &mut l.cutoff,
            &mut l.propagateagain,
            &mut l.solvelpagain,
            &mut l.solverelaxagain,
        )?;

        // Update lower bound with the pseudo objective value, and cut off node by
        // bounding.
        apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut l.cutoff)?;
    }

    // Check, if we want to solve the LP at this node.
    if solvelp && !l.cutoff && crate::tree::tree_has_focus_node_lp(scip) {
        l.lperror = false;
        l.unbounded = false;

        // Solve the node's LP.
        let r = solve_node_lp(
            scip,
            origprob,
            transprob,
            primal,
            eventfilter,
            initiallpsolved,
            fullseparation,
            newinitconss,
            forcedlpsolve,
            &mut l.propagateagain,
            &mut l.solverelaxagain,
            &mut l.unbounded,
        )?;
        l.cutoff = r.cutoff;
        l.lperror = r.lperror;
        l.pricingaborted = r.pricingaborted;

        l.lpsolved = true;
        l.solvelpagain = false;

        // Check, if the path was cutoff.
        l.cutoff = l.cutoff || treer!(scip).cutoffdepth <= actdepth;

        // If an error occured during LP solving, switch to pseudo solution.
        if l.lperror {
            if forcedlpsolve {
                log::error!("(node {}) unresolved numerical troubles in LP {} cannot be dealt with", scip.stat.nnodes, scip.stat.nlps);
                return Err(crate::retcode::Retcode::LpError);
            }
            crate::tree::tree_set_focus_node_lp(scip, false);
            l.nlperrors += 1;
            log::info!(
                "(node {}) unresolved numerical troubles in LP {} -- using pseudo solution instead (loop {})",
                scip.stat.nnodes,
                scip.stat.nlps,
                l.nlperrors
            );
        }

        let solstat = crate::lp::lp_get_solstat(scip);
        if solstat == LpSolStat::TimeLimit || solstat == LpSolStat::IterLimit {
            crate::tree::tree_set_focus_node_lp(scip, false);
            l.forcedenforcement = true;
            log::info!(
                "(node {}) LP solver hit {} limit in LP {} -- using pseudo solution instead",
                scip.stat.nnodes,
                if solstat == LpSolStat::TimeLimit { "time" } else { "iteration" },
                scip.stat.nlps
            );
        }

        // If we solve exactly, and the LP claims to be infeasible but the
        // infeasibility could not be proved, we have to forget about the LP and
        // use the pseudo solution instead. this "not proved" condition is
        // monitored by the last check. normally if the LP is infeasible, the
        // lowerbound would be infinity.
        if !l.cutoff
            && !l.lperror
            && l.pricingaborted
            && crate::lp::lp_get_solstat(scip) == LpSolStat::Infeasible
            && scip.nodes[focusnode].lowerbound < scip.primals[primal].cutoffbound
        {
            if crate::branch::branchcand_get_n_pseudo_cands(scip) == 0 && scip.probs[transprob].ncontvars > 0 {
                log::error!(
                    "(node {}) could not prove infeasibility of LP {} (exactsolve=0, pricingaborted=1), all variables are fixed, {} continuous vars",
                    scip.stat.nnodes,
                    scip.stat.nlps,
                    scip.probs[transprob].ncontvars
                );
                return Err(crate::retcode::Retcode::LpError);
            } else {
                crate::tree::tree_set_focus_node_lp(scip, false);
                l.forcedenforcement = true;
            }
        }

        // Update lower bound with the pseudo objective value, and cut off node by
        // bounding.
        apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut l.cutoff)?;
    }

    // Reset solverelaxagain if no relaxations were solved up to this point
    // (the LP-changes are already included in relaxators called after the LP).
    l.solverelaxagain = l.solverelaxagain && l.relaxcalled;

    // Solve external relaxations with negative priority.
    if solverelax && !l.cutoff {
        solve_node_relax(
            scip,
            eventfilter,
            transprob,
            origprob,
            actdepth,
            false,
            &mut l.cutoff,
            &mut l.propagateagain,
            &mut l.solvelpagain,
            &mut l.solverelaxagain,
            &mut l.relaxcalled,
        )?;

        // Check, if the path was cutoff.
        l.cutoff = l.cutoff || treer!(scip).cutoffdepth <= actdepth;

        // Apply found cuts.
        apply_cuts(
            scip,
            transprob,
            origprob,
            eventfilter,
            actdepth == 0,
            &mut l.cutoff,
            &mut l.propagateagain,
            &mut l.solvelpagain,
            &mut l.solverelaxagain,
        )?;

        // Update lower bound with the pseudo objective value, and cut off node by
        // bounding.
        apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut l.cutoff)?;
    }

    Ok(())
}

/// `restartAllowed`: check if a restart can be performed.
fn restart_allowed(scip: &Scip) -> bool {
    scip.set.nactivepricers == 0 && !scip.set.reopt_enable && (scip.set.presol_maxrestarts == -1 || scip.stat.nruns <= scip.set.presol_maxrestarts) && !scip.set.exact_enable
}

/// The outcome of `solveNode`.
#[derive(Default)]
struct NodeResult {
    /// Whether the node can be cut off.
    cutoff: bool,
    /// Whether the node should be postponed.
    postpone: bool,
    /// Whether the focus node is unbounded.
    unbounded: bool,
    /// Whether the focus node's solution is infeasible.
    infeasible: bool,
    /// Should solving process be started again with presolving?
    restart: bool,
    /// Whether AFTERNODE heuristics were already called.
    afternodeheur: bool,
    /// Whether solving was interrupted.
    stopped: bool,
}

/// The check for an immediate restart in `solveNode`, with `restartfac`
/// the fraction of integer variables fixed in the root node.
fn restart_wanted(scip: &Scip, transprob: usize, restartfac: f64) -> bool {
    let p = &scip.probs[transprob];
    restart_allowed(scip)
        && (scip.stat.userrestart
            || (scip.stat.nrootintfixingsrun as f64 > restartfac * (p.nvars - p.ncontvars) as f64
                && (scip.stat.nruns == 1 || p.nvars as f64 <= (1.0 - scip.set.presol_restartminred) * scip.stat.prevrunnvars as f64)))
}

/// `solveNode`: solves the focus node.
fn solve_node(scip: &mut Scip, origprob: usize, transprob: usize, primal: usize, eventfilter: usize) -> ScipResult<NodeResult> {
    use ScipResultCode as R;

    use crate::types::LpSolStat;

    let mut n = NodeResult::default();
    let mut pricingaborted = false;

    let focusnode = crate::tree::tree_get_focus_node(scip);
    let actdepth = scip.nodes[focusnode].depth as i32;

    // Invalidate relaxation solution.
    crate::relax::relaxation_set_sol_valid(scip, false, false);

    // Clear the storage of external branching candidates.
    crate::branch::branchcand_clear_extern_cands(scip);

    // Check, if we want to solve the LP at the selected node:
    // - solve the LP, if the lp solve depth and frequency demand solving
    // - solve the root LP, if the LP solve frequency is set to 0
    // - solve the root LP, if there are continuous variables present
    // - don't solve the node if its cut off by the pseudo objective value anyway
    let set = &scip.set;
    let mut focusnodehaslp = set.lp_solvedepth == -1 || actdepth <= set.lp_solvedepth;
    focusnodehaslp = focusnodehaslp && actdepth >= set.lp_minsolvedepth;
    focusnodehaslp = focusnodehaslp && (set.lp_solvefreq >= 1 && actdepth % set.lp_solvefreq == 0);
    focusnodehaslp = focusnodehaslp || (actdepth == 0 && set.lp_solvefreq == 0);
    let pseudoobjval = crate::lp::lp_get_pseudo_objval(scip, transprob);
    focusnodehaslp = focusnodehaslp && scip.set.is_lt(pseudoobjval, scip.primals[primal].cutoffbound);
    crate::tree::tree_set_focus_node_lp(scip, focusnodehaslp);

    // External node solving loop:
    //  - propagate domains
    //  - solve SCIP_LP
    //  - enforce constraints
    // if a constraint handler adds constraints to enforce its own constraints,
    // both, propagation and LP solving is applied again (if applicable on
    // current node); however, if the new constraints don't have the enforce
    // flag set, it is possible, that the current infeasible solution is not
    // cut off; in this case, we have to declare the solution infeasible and
    // perform a branching.
    let mut lastdomchgcount = scip.stat.domchgcount;
    let mut lastlpcount = scip.stat.lpcount;
    let mut initiallpsolved = false;
    let mut fullseparation = true;
    scip.stat.npricerounds = 0;
    scip.stat.nseparounds = 0;
    let mut forcedlpsolve = false;
    let mut nloops = 0;

    let mut l = NodeLoop {
        heurtiming: HEURTIMING_BEFORENODE,
        solverelaxagain: true,
        solvelpagain: true,
        propagateagain: true,
        fullpropagation: true,
        ..NodeLoop::default()
    };

    while !n.cutoff && !n.postpone && (l.solverelaxagain || l.solvelpagain || l.propagateagain) && l.nlperrors < MAXNLPERRORS && !n.restart {
        n.unbounded = false;
        n.infeasible = false;
        let mut foundsol = false;

        nloops += 1;
        l.lperror = false;
        l.lpsolved = false;
        l.relaxcalled = false;
        l.forcedenforcement = false;
        l.afterlpproplps = -1;

        while !l.lperror && !n.cutoff && (l.propagateagain || l.solvelpagain || l.solverelaxagain || (l.afterlpproplps < scip.stat.nnodelps && l.lpsolved) || l.relaxcalled) {
            let solverelax = l.solverelaxagain;
            l.solverelaxagain = false;
            let solvelp = l.solvelpagain;
            l.solvelpagain = false;
            let propagate = l.propagateagain;
            l.propagateagain = false;

            // Update lower bound with the pseudo objective value, and cut off node
            // by bounding.
            apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut n.cutoff)?;

            // Propagate domains before lp solving and solve relaxation and lp.
            l.cutoff = n.cutoff;
            l.postpone = n.postpone;
            l.unbounded = n.unbounded;
            l.stopped = n.stopped;
            l.pricingaborted = pricingaborted;
            prop_and_solve(
                scip,
                origprob,
                transprob,
                primal,
                eventfilter,
                focusnode,
                actdepth,
                propagate,
                solvelp,
                solverelax,
                forcedlpsolve,
                initiallpsolved,
                fullseparation,
                &mut l,
            )?;
            n.cutoff = l.cutoff;
            n.postpone = l.postpone;
            n.unbounded = l.unbounded;
            n.stopped = l.stopped;
            pricingaborted = l.pricingaborted;

            initiallpsolved |= l.lpsolved;

            // Time or solution limit was hit and we already created a dummy child
            // node to terminate fast.
            if n.stopped {
                // Reset LP feastol to normal value, in case someone tightened it
                // during node solving.
                crate::lp::lp_reset_feastol(scip);
                return Ok(n);
            }
        }
        fullseparation = false;

        // Update the cutoff, propagateagain, and solverelaxagain status of current
        // solving loop.
        update_loop_status(scip, actdepth, &mut n.cutoff, &mut l.propagateagain, &mut l.solverelaxagain);

        // Call primal heuristics that should be applied after the LP relaxation of
        // the node was solved; if this is the first loop of the root node, call
        // also AFTERNODE heuristics already here, since they might help to
        // improve the primal bound, thereby producing additional reduced cost
        // strengthenings and strong branching bound fixings which also might lead
        // to a restart.
        if !n.postpone && (!n.cutoff || crate::tree::tree_get_n_nodes(scip) > 0) {
            if actdepth == 0 && !n.afternodeheur {
                let unbounded;
                (foundsol, unbounded) = primal_heuristics(scip, transprob, primal, true, true, NONE, HEURTIMING_AFTERLPLOOP | HEURTIMING_AFTERNODE, n.cutoff)?;
                n.unbounded = n.unbounded || unbounded;
                // The AFTERNODE heuristics should not be called again after the
                // node.
                n.afternodeheur = true;
            } else if l.lpsolved || crate::relax::relaxation_is_sol_valid(scip) {
                let unbounded;
                (foundsol, unbounded) = primal_heuristics(scip, transprob, primal, true, true, NONE, HEURTIMING_AFTERLPLOOP, n.cutoff)?;
                n.unbounded = n.unbounded || unbounded;
            }

            // Heuristics might have found a solution or set the cutoff bound such
            // that the current node is cut off.
            apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut n.cutoff)?;
        }

        // Check if heuristics leave us with an invalid LP.
        if lp!(scip).resolvelperror {
            if forcedlpsolve {
                log::error!("(node {}) unresolved numerical troubles in LP {} cannot be dealt with", scip.stat.nnodes, scip.stat.nlps);
                return Err(crate::retcode::Retcode::LpError);
            }
            crate::tree::tree_set_focus_node_lp(scip, false);
            lp!(scip).resolvelperror = false;
            l.nlperrors += 1;
            log::info!(
                "(node {}) unresolved numerical troubles in LP {} -- using pseudo solution instead (loop {})",
                scip.stat.nnodes,
                scip.stat.nlps,
                l.nlperrors
            );
        }

        if pricingaborted && !n.cutoff && crate::lp::lp_get_solstat(scip) != LpSolStat::Optimal {
            crate::tree::tree_set_focus_node_lp(scip, false);

            // If we just ran into the time limit this is not really a numerical
            // trouble; however, if this is not the case, we print messages about
            // numerical troubles in the current LP.
            if !solve_is_stopped(scip, false) {
                if forcedlpsolve {
                    log::error!("(node {}) unresolved numerical troubles in LP {} cannot be dealt with", scip.stat.nnodes, scip.stat.nlps);
                    return Err(crate::retcode::Retcode::LpError);
                }
                l.nlperrors += 1;
                log::info!(
                    "(node {}) unresolved numerical troubles in LP {} -- using pseudo solution instead (loop {})",
                    scip.stat.nnodes,
                    scip.stat.nlps,
                    l.nlperrors
                );
            }
        }

        // If an improved solution was found, propagate and solve the relaxations
        // again.
        if foundsol && !n.cutoff {
            l.propagateagain = true;
            l.solvelpagain = true;
            l.solverelaxagain = true;
            mark_relaxs_unsolved(scip);
        }

        // Check for immediate restart.
        n.restart = n.restart || (actdepth == 0 && restart_wanted(scip, transprob, scip.set.presol_immrestartfac));

        // Enforce constraints.
        let mut branched = false;
        if !n.postpone && !n.restart && !n.cutoff && !l.solverelaxagain && !l.solvelpagain && !l.propagateagain {
            // If the solution changed since the last enforcement, we have to
            // completely reenforce it; otherwise, we only have to enforce the
            // additional constraints added in the last enforcement, but keep the
            // infeasible flag TRUE in order to not declare the infeasible solution
            // feasible due to disregarding the already enforced constraints.
            if lastdomchgcount != scip.stat.domchgcount || lastlpcount != scip.stat.lpcount {
                lastdomchgcount = scip.stat.domchgcount;
                lastlpcount = scip.stat.lpcount;
                n.infeasible = false;
            }

            // Call constraint enforcement.
            let f = EnforceFlags {
                branched: &mut branched,
                cutoff: &mut n.cutoff,
                infeasible: &mut n.infeasible,
                propagateagain: &mut l.propagateagain,
                solvelpagain: &mut l.solvelpagain,
                solverelaxagain: &mut l.solverelaxagain,
            };
            enforce_constraints(scip, transprob, primal, f, l.forcedenforcement)?;

            // Apply found cuts.
            apply_cuts(
                scip,
                transprob,
                origprob,
                eventfilter,
                actdepth == 0,
                &mut n.cutoff,
                &mut l.propagateagain,
                &mut l.solvelpagain,
                &mut l.solverelaxagain,
            )?;

            // Update lower bound with the pseudo objective value, and cut off node
            // by bounding.
            apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut n.cutoff)?;

            // Update the cutoff, propagateagain, and solverelaxagain status of
            // current solving loop.
            update_loop_status(scip, actdepth, &mut n.cutoff, &mut l.propagateagain, &mut l.solverelaxagain);
        }

        // The enforcement detected no infeasibility, so, no branching was
        // performed, but the pricing was aborted and the current feasible
        // solution does not have to be the best solution in the current subtree
        // --> we have to do a pseudo branching, so we set infeasible TRUE and add
        // the current solution to the solution pool.
        if pricingaborted && !n.infeasible && !n.cutoff && !n.postpone && !n.restart {
            let oldnbestsolsfound = scip.primals[primal].nbestsolsfound;
            let flags = crate::primal::CheckFlags {
                printreason: false,
                completely: false,
                checkbounds: true,
                checkintegrality: true,
                checklprows: true,
            };

            // In case the relaxation was enforced add this solution, otherwise
            // decide between LP and pseudosol.
            if relax_sol_preferred(scip, transprob) {
                let relaxsol = crate::sol::sol_create_relax_sol(scip, primal, NONE)?;

                let stored = crate::primal::primal_try_sol(scip, primal, origprob, transprob, eventfilter, relaxsol, flags)?;

                crate::sol::sol_free(scip, relaxsol, primal)?;

                if stored {
                    scip.stat.nrelaxsolsfound += 1;

                    if scip.primals[primal].nbestsolsfound != oldnbestsolsfound {
                        scip.stat.nrelaxbestsolsfound += 1;
                        api::store_solution_gap(scip);
                    }
                }
            } else {
                let sol = crate::sol::sol_create_current_sol(scip, transprob, primal, NONE)?;
                let stored = crate::primal::primal_try_sol_free(scip, primal, origprob, transprob, eventfilter, sol, flags)?;

                if stored {
                    scip.stat.nlpsolsfound += 1;

                    if scip.primals[primal].nbestsolsfound != oldnbestsolsfound {
                        scip.stat.nlpbestsolsfound += 1;
                        api::store_solution_gap(scip);
                    }
                }
            }

            n.infeasible = true;
        }

        // If the node is infeasible, but no constraint handler could resolve the
        // infeasibility -> branch on LP, external candidates, or the pseudo
        // solution -> e.g. select non-fixed binary or integer variable x with
        // value x', create three sons: x <= x'-1, x = x', and x >= x'+1. In the
        // left and right branch, the current solution is cut off. In the middle
        // branch, the constraints can hopefully reduce domains of other variables
        // to cut off the current solution. In LP branching, we cannot allow
        // adding constraints, because this does not necessary change the LP and
        // can therefore lead to an infinite loop.
        let wasforcedlpsolve = forcedlpsolve;
        forcedlpsolve = false;
        if n.infeasible
            && !n.cutoff
            && !n.postpone
            && !n.restart
            && (!n.unbounded || crate::branch::branchcand_get_n_extern_cands(scip) > 0 || crate::branch::branchcand_get_n_pseudo_cands(scip) > 0)
            && !l.solverelaxagain
            && !l.solvelpagain
            && !l.propagateagain
            && !branched
        {
            let mut result = R::DidNotRun;
            let cutoffbound = scip.primals[primal].cutoffbound;
            let sepastore = crate::sepastore::SEPASTORE;

            let nlpcands = if crate::tree::tree_has_focus_node_lp(scip) {
                crate::branch::branchcand_get_lp_cands(scip).nlpcands
            } else {
                0
            };

            if nlpcands > 0 || crate::branch::branchcand_get_n_extern_cands(scip) > 0 {
                // If there are LP candidates and their maximal priority is at least
                // the maximal priority of the external candidates, then branch on
                // the LP candidates. Note that due to implicit integer variables,
                // SCIPbranchcandGetLPMaxPrio(branchcand) might be finite and
                // SCIPbranchcandGetNPrioLPCands(branchcand) > 0, but nlpcands == 0.
                if crate::branch::branchcand_get_lp_max_prio(scip) >= crate::branch::branchcand_get_extern_max_prio(scip) && nlpcands > 0 {
                    // Branch on LP solution.
                    result = crate::branch::branch_exec_lp(scip, transprob, origprob, eventfilter, sepastore, cutoffbound, false)?;
                } else {
                    // Branch on external candidates.
                    result = crate::branch::branch_exec_extern(scip, transprob, origprob, eventfilter, sepastore, cutoffbound, true)?;
                }
            }

            if result == R::DidNotRun || result == R::DidNotFind {
                // Branch on pseudo solution.
                result = crate::branch::branch_exec_pseudo(scip, transprob, origprob, eventfilter, cutoffbound, true)?;
            }

            // SCIP cannot guarantee convergence if it is necessary to branch on
            // unbounded variables.
            if result == R::Branched {
                let var = scip.stat.lastbranchvar;

                if var != NONE
                    && !scip.stat.branchedunbdvar
                    && !crate::var::var_is_integral(scip, var)
                    && (scip.set.is_infinity(-scip.vars[var].locdom.lb) || scip.set.is_infinity(scip.vars[var].locdom.ub))
                {
                    let v = &scip.vars[var];
                    log::info!(
                        "Starting spatial branch-and-bound on unbounded variable <{}> ([{},{}]) - cannot guarantee finite termination.",
                        v.name,
                        v.locdom.lb,
                        v.locdom.ub
                    );
                    scip.stat.branchedunbdvar = true;
                }
            }

            match result {
                R::Cutoff => n.cutoff = true,
                R::ConsAdded => {
                    if nlpcands > 0 {
                        log::error!("LP branching rule added constraint, which was not allowed this time");
                        return Err(crate::retcode::Retcode::InvalidResult);
                    }
                    l.propagateagain = true;
                    l.solvelpagain = true;
                    l.solverelaxagain = true;
                    mark_relaxs_unsolved(scip);
                }
                R::ReducedDom => {
                    l.propagateagain = true;
                    l.solvelpagain = true;
                    l.solverelaxagain = true;
                    mark_relaxs_unsolved(scip);
                }
                R::Separated => {
                    l.solvelpagain = true;
                    l.solverelaxagain = true;
                    mark_relaxs_unsolved(scip);
                }
                R::Branched => {
                    // Increase the number of internal nodes.
                    scip.stat.ninternalnodes += 1;
                    scip.stat.ntotalinternalnodes += 1;
                }
                R::DidNotFind | R::DidNotRun => {
                    // All integer variables in the infeasible solution are fixed,
                    // - if no continuous variables exist and all variables are known,
                    //   the infeasible pseudo solution is completely fixed, and the
                    //   node can be cut off
                    // - if at least one continuous variable exists or we do not know
                    //   all variables due to external pricers, we cannot resolve the
                    //   infeasibility by branching -> solve LP (and maybe price in
                    //   additional variables)
                    if scip.probs[transprob].ncontvars == 0 && scip.set.nactivepricers == 0 {
                        n.cutoff = true;
                    } else {
                        // Feasible LP solutions with all integers fixed must be
                        // feasible if also no external branching candidates were
                        // available.
                        let solstat = crate::lp::lp_get_solstat(scip);
                        if solstat == LpSolStat::TimeLimit || solstat == LpSolStat::IterLimit || solve_is_stopped(scip, false) {
                            // As we hit the time or iteration limit or another
                            // interrupt (e.g., gap limit), we do not want to solve the
                            // LP again. in order to terminate correctly, we create a
                            // "branching" with only one child node that is a copy of
                            // the focusnode.
                            let estimate = scip.nodes[focusnode].estimate;
                            crate::tree::node_create_child(scip, 1.0, estimate)?;
                        } else {
                            if pricingaborted {
                                log::error!("pricing was aborted, but no branching could be created!");
                                return Err(crate::retcode::Retcode::InvalidResult);
                            }

                            if wasforcedlpsolve {
                                log::error!("LP was solved, all integers fixed, some constraint still infeasible, but no branching could be created!");
                                return Err(crate::retcode::Retcode::InvalidResult);
                            }

                            if !treer!(scip).forcinglpmessage && scip.set.disp_verblevel == 4 {
                                // Remember that the forcing LP solving message was posted
                                // and do only post it again if the verblevel is
                                // SCIP_VERBLEVEL_FULL.
                                tree!(scip).forcinglpmessage = true;
                            }
                            log::info!("(node {}) forcing the solution of an LP (last LP {})...", scip.stat.nnodes, scip.stat.nlps);

                            // Solve the LP in the next loop.
                            crate::tree::tree_set_focus_node_lp(scip, true);
                            l.solvelpagain = true;
                            // This LP must be solved without error - otherwise we have
                            // to abort.
                            forcedlpsolve = true;
                        }
                    }
                }
                _ => {
                    log::error!("invalid result code <{:?}> from SCIPbranchLP(), SCIPbranchExt() or SCIPbranchPseudo()", result);
                    return Err(crate::retcode::Retcode::InvalidResult);
                }
            }

            // Apply found cuts.
            apply_cuts(
                scip,
                transprob,
                origprob,
                eventfilter,
                actdepth == 0,
                &mut n.cutoff,
                &mut l.propagateagain,
                &mut l.solvelpagain,
                &mut l.solverelaxagain,
            )?;

            // Update lower bound with the pseudo objective value, and cut off node by
            // bounding.
            apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut n.cutoff)?;

            // Update the cutoff, propagateagain, and solverelaxagain status of
            // current solving loop.
            update_loop_status(scip, actdepth, &mut n.cutoff, &mut l.propagateagain, &mut l.solverelaxagain);
        }

        // Check for immediate restart.
        n.restart = n.restart || (actdepth == 0 && restart_wanted(scip, transprob, scip.set.presol_immrestartfac));
    }
    let _ = nloops;

    // Flush the conflict set storage.
    crate::conflict::conflict_flush_conss(scip, transprob, origprob, eventfilter)?;

    // Check for too many LP errors.
    if l.nlperrors >= MAXNLPERRORS {
        log::error!("(node {}) unresolved numerical troubles in LP {} -- aborting", scip.stat.nnodes, scip.stat.nlps);
        return Err(crate::retcode::Retcode::LpError);
    }

    // Check for final restart.
    let mut restartfac = scip.set.presol_subrestartfac;
    if actdepth == 0 {
        restartfac = crate::def::min(restartfac, scip.set.presol_restartfac);
    }
    n.restart = n.restart || restart_wanted(scip, transprob, restartfac);

    // Remember the last root LP solution.
    if actdepth == 0 && !n.cutoff && !n.unbounded && !n.postpone {
        // The root pseudo objective value and pseudo objective value should be
        // equal in the root node.
        let haslp = crate::tree::tree_has_focus_node_lp(scip);
        crate::prob::prob_store_root_sol(scip, transprob, haslp);
    }

    // Check for cutoff.
    if n.cutoff {
        crate::tree::node_cutoff(scip, focusnode, eventfilter, transprob, origprob)?;

        // The LP might have been unbounded but not enforced, because the node is
        // cut off anyway.
        n.unbounded = false;
        n.infeasible = true;
    }
    // Update the regression statistic nlpbranchcands and LP objective value.
    else if !n.unbounded && crate::lp::lp_get_solstat(scip) == LpSolStat::Optimal && lp!(scip).looseobjvalinf == 0 {
        // Get number of LP candidate variables.
        let nlpcands = crate::branch::branchcand_get_lp_cands(scip).nlpcands;

        // Get LP objective value.
        let lpobjval = crate::lp::lp_get_objval(scip, transprob);

        // Add the observation to the regression.
        scip.stat.regressioncandsobjval.add_observation(nlpcands as f64, lpobjval);
    }

    // Reset LP feastol to normal value, in case someone tightened it during
    // node solving.
    crate::lp::lp_reset_feastol(scip);

    Ok(n)
}

/// `addCurrentSolution`: if feasible, adds current solution to the solution
/// storage; `checksol`: should the solution be checked?
fn add_current_solution(scip: &mut Scip, origprob: usize, transprob: usize, primal: usize, eventfilter: usize, checksol: bool) -> ScipResult {
    let oldnbestsolsfound = scip.primals[primal].nbestsolsfound;
    let te = scip.set.time_enabled;
    let flags = crate::primal::CheckFlags {
        printreason: false,
        completely: false,
        checkbounds: true,
        checkintegrality: true,
        checklprows: true,
    };

    // Adds the solution, checking it if wanted.
    let add = |scip: &mut Scip, sol| -> ScipResult<bool> {
        if checksol {
            crate::primal::primal_try_sol_free(scip, primal, origprob, transprob, eventfilter, sol, flags)
        } else {
            crate::primal::primal_add_sol_free(scip, primal, origprob, transprob, eventfilter, sol)
        }
    };

    // Found a feasible solution.
    if relax_sol_preferred(scip, transprob) {
        // Start clock for relaxation solutions.
        scip.stat.relaxsoltime.start(te);

        let sol = crate::sol::sol_create_relax_sol(scip, primal, NONE)?;
        let foundsol = add(scip, sol)?;

        if foundsol {
            scip.stat.nrelaxsolsfound += 1;

            if scip.primals[primal].nbestsolsfound != oldnbestsolsfound {
                scip.stat.nrelaxbestsolsfound += 1;
                api::store_solution_gap(scip);
            }
        }

        // Stop clock for relaxation solutions.
        scip.stat.relaxsoltime.stop(te);
    } else if crate::tree::tree_has_focus_node_lp(scip) {
        // Start clock for LP solutions.
        scip.stat.lpsoltime.start(te);

        // Add solution to storage.
        let sol = crate::sol::sol_create_lp_sol(scip, transprob, primal, NONE)?;
        let foundsol = add(scip, sol)?;

        if foundsol {
            scip.stat.nlpsolsfound += 1;

            if scip.primals[primal].nbestsolsfound != oldnbestsolsfound {
                scip.stat.nlpbestsolsfound += 1;
                api::store_solution_gap(scip);
            }
        }

        // Stop clock for LP solutions.
        scip.stat.lpsoltime.stop(te);
    } else {
        // Start clock for pseudo solutions.
        scip.stat.pseudosoltime.start(te);

        // Add solution to storage.
        let sol = crate::sol::sol_create_pseudo_sol(scip, transprob, primal, NONE)?;
        let foundsol = add(scip, sol)?;

        // Stop clock for pseudo solutions.
        scip.stat.pseudosoltime.stop(te);

        if foundsol {
            scip.stat.npssolsfound += 1;

            if scip.primals[primal].nbestsolsfound != oldnbestsolsfound {
                scip.stat.npsbestsolsfound += 1;
                api::store_solution_gap(scip);
            }
        }
    }

    Ok(())
}

/// `SCIPsolveCIP`: main solving loop; returns whether the solving process
/// should be started again with presolving.
pub fn solve_cip(scip: &mut Scip) -> ScipResult<bool> {
    use crate::{event::*, types::LpSolStat};

    let transprob = crate::prob::TRANSPROB;
    let origprob = crate::prob::ORIGPROB;
    let primal = crate::scip::TRANSPRIMAL;
    let eventfilter = crate::scip::GLOBALEVENTFILTER;

    // Check for immediate restart (if problem solving marked to be restarted
    // was aborted).
    let mut restartfac = scip.set.presol_subrestartfac;
    if crate::tree::tree_get_current_depth(scip) == 0 {
        restartfac = crate::def::min(restartfac, scip.set.presol_restartfac);
    }
    let mut restart = restart_wanted(scip, transprob, restartfac);

    // Calculate the number of successful conflict analysis calls that should
    // trigger a restart.
    let restartconfnum = if scip.set.conf_restartnum > 0 {
        let mut r = scip.set.conf_restartnum as f64;
        for _ in 0..scip.stat.nconfrestarts {
            r *= scip.set.conf_restartfac;
        }
        r
    } else {
        f64::MAX
    };

    // Switch status to UNKNOWN.
    scip.stat.status = Status::Unknown;

    let mut focusnode;
    let mut nextnode = NONE;
    let mut unbounded = false;
    let mut postpone;
    let mut cutoff = false;

    while !solve_is_stopped(scip, true) && !restart {
        let mut foundsol = false;
        let infeasible;
        let mut nodesel;

        loop {
            // Update the memory saving flag, switch algorithms respectively.
            crate::stat::stat_update_memsave_mode(scip);

            // Get the current node selector.
            nodesel = crate::nodesel::set_get_nodesel(scip);

            // Inform tree about the current node selector.
            crate::tree::tree_set_nodesel(scip, nodesel)?;

            // The next node was usually already selected in the previous solving
            // loop before the primal heuristics were called, because they need to
            // know, if the next node will be a child/sibling (plunging) or not; if
            // the heuristics found a new best solution that cut off some of the
            // nodes, the node selector must be called again, because the selected
            // next node may be invalid due to cut off.
            if nextnode == NONE {
                // Select next node to process.
                nextnode = crate::nodesel::nodesel_select(scip, nodesel)?;
            }
            focusnode = nextnode;
            nextnode = NONE;

            // Start node activation timer.
            let te = scip.set.time_enabled;
            scip.stat.nodeactivationtime.start(te);

            // Focus selected node.
            crate::tree::node_focus(scip, &mut focusnode, transprob, origprob, eventfilter, &mut cutoff, false, false)?;

            if cutoff {
                scip.stat.ndelayedcutoffs += 1;
            }

            // Stop node activation timer.
            scip.stat.nodeactivationtime.stop(te);

            // Select new node, if the current one was located in a cut off
            // subtree.
            if !cutoff {
                break;
            }
        }

        // If no more node was selected, we finished optimization.
        if focusnode == NONE {
            break;
        }

        // Update maxdepth and node count statistics.
        let depth = scip.nodes[focusnode].depth as i32;
        scip.stat.maxdepth = scip.stat.maxdepth.max(depth);
        scip.stat.maxtotaldepth = scip.stat.maxtotaldepth.max(depth);
        scip.stat.nnodes += 1;
        scip.stat.ntotalnodes += 1;

        // Update reference bound statistic, if available.
        if scip.set.is_ge(scip.nodes[focusnode].lowerbound, scip.stat.referencebound) {
            scip.stat.nnodesaboverefbound += 1;
        }

        // Issue NODEFOCUSED event.
        let event = event_create_node(EVENTTYPE_NODEFOCUSED, focusnode);
        event_process(scip, &event, eventfilter)?;

        // Solve focus node.
        let n = solve_node(scip, origprob, transprob, primal, eventfilter)?;
        cutoff = n.cutoff;
        postpone = n.postpone;
        unbounded = n.unbounded;
        infeasible = n.infeasible;
        restart = n.restart;
        let afternodeheur = n.afternodeheur;
        let mut stopped = n.stopped;

        let branched = treer!(scip).nchildren > 0;

        if stopped {
            break;
        }

        // Check for restart.
        if !restart && !postpone {
            // Check, if the current solution is feasible.
            if !infeasible {
                // In the unbounded case, we check the solution w.r.t. the original
                // problem, because we do not want to rely on the LP feasibility and
                // integrality is not checked for unbounded solutions, anyway.
                let feasible = if unbounded {
                    let sol = if relax_sol_preferred(scip, transprob) {
                        crate::sol::sol_create_relax_sol(scip, primal, NONE)?
                    } else if crate::tree::tree_has_focus_node_lp(scip) {
                        crate::sol::sol_create_lp_sol(scip, transprob, primal, NONE)?
                    } else {
                        crate::sol::sol_create_pseudo_sol(scip, transprob, primal, NONE)?
                    };
                    let feasible = api::check_sol_orig(scip, sol, false, false)?;
                    crate::sol::sol_free(scip, sol, primal)?;
                    feasible
                } else {
                    true
                };

                // Node solution is feasible: add it to the solution store.
                if feasible {
                    add_current_solution(scip, origprob, transprob, primal, eventfilter, false)?;

                    // Issue NODEFEASIBLE event.
                    let event = event_create_node(EVENTTYPE_NODEFEASIBLE, focusnode);
                    event_process(scip, &event, eventfilter)?;

                    // Update the cutoff pointer if the new solution made the cutoff
                    // bound equal to the lower bound.
                    apply_bounding(scip, transprob, origprob, primal, eventfilter, &mut cutoff)?;

                    // Increment number of feasible leaf nodes.
                    scip.stat.nfeasleaves += 1;
                }
            } else if !unbounded || branched {
                // Node solution is not feasible.
                let eventtype;
                if !branched {
                    // Issue NODEINFEASIBLE event.
                    eventtype = EVENTTYPE_NODEINFEASIBLE;

                    // We only increase the number of objective leaf nodes if we hit
                    // the LP objective limit; we might have also hit the objective
                    // limit at a node that is actually infeasible, or a dual
                    // reduction led to an infeasibility prior to LP solving such that
                    // the node will be marked as infeasible.
                    if crate::tree::tree_has_current_node_lp(scip) && crate::lp::lp_get_solstat(scip) == LpSolStat::ObjLimit {
                        scip.stat.nobjleaves += 1;
                    } else {
                        scip.stat.ninfeasleaves += 1;
                    }

                    // Increase the cutoff counter of the branching variable.
                    let var = scip.stat.lastbranchvar;
                    if var != NONE {
                        let (dir, value) = (scip.stat.lastbranchdir, scip.stat.lastbranchvalue);
                        crate::var::var_inc_cutoff_sum(scip, var, dir, value, 1.0)?;
                    }
                    // @todo if last branching variable is unknown, retrieve it from
                    // the nodes' boundchg arrays
                } else {
                    // Issue NODEBRANCHED event.
                    eventtype = EVENTTYPE_NODEBRANCHED;
                }
                let event = event_create_node(eventtype, focusnode);
                event_process(scip, &event, eventfilter)?;
            }

            // If no branching was created, the node was not cut off, but its lower
            // bound is still smaller than the cutoff bound, we have to branch on a
            // non-fixed variable; this can happen, if we want to solve exactly, the
            // current solution was declared feasible by the constraint
            // enforcement, but in exact solution checking it was found out to be
            // infeasible; in this case, no branching would have been generated by
            // the enforcement of constraints, but we have to further investigate
            // the current sub tree; note that we must not check tree->nchildren >
            // 0 here to determine whether we branched, we rather check it directly
            // after solveNode() and store the result, because an event handler
            // might impose a new cutoff bound (as is the case in ParaSCIP).
            if !cutoff && !unbounded && !branched && scip.nodes[focusnode].lowerbound < scip.primals[primal].cutoffbound {
                loop {
                    let mut result = ScipResultCode::DidNotRun;
                    if crate::branch::branchcand_get_n_pseudo_cands(scip) == 0 {
                        if scip.probs[transprob].ncontvars > 0 {
                            log::error!("cannot branch on all-fixed LP");
                        }
                    } else {
                        let cutoffbound = scip.primals[primal].cutoffbound;
                        result = crate::branch::branch_exec_pseudo(scip, transprob, origprob, eventfilter, cutoffbound, false)?;
                    }
                    if result != ScipResultCode::ReducedDom {
                        break;
                    }
                }
            }

            // Select node to process in next solving loop; the primal heuristics
            // need to know whether a child/sibling (plunging) will be selected as
            // next node or not.
            nextnode = crate::nodesel::nodesel_select(scip, nodesel)?;

            // Call primal heuristics that should be applied after the node was
            // solved.
            let nnodes = crate::tree::tree_get_n_nodes(scip);
            stopped = solve_is_stopped(scip, false);
            if !afternodeheur && (!cutoff || nnodes > 0) && !stopped {
                let unb;
                (foundsol, unb) = primal_heuristics(scip, transprob, primal, true, true, nextnode, HEURTIMING_AFTERNODE, cutoff)?;
                unbounded = unbounded || unb;
                stopped = solve_is_stopped(scip, false);
            }

            // If the heuristics found a new best solution that cut off some of the
            // nodes, the node selector must be called again, because the selected
            // next node may be invalid due to cut off.
            if nnodes != crate::tree::tree_get_n_nodes(scip) || stopped {
                nextnode = NONE;
            }
        } else if !infeasible && !postpone {
            // The current solution was not proven to be infeasible, but due to the
            // restart, this does not mean that it is feasible, we might just have
            // skipped the check. Thus, we try to add it to the solution store, but
            // check it again.
            add_current_solution(scip, origprob, transprob, primal, eventfilter, true)?;
        }
        // We want to put the focusnode back into the leaf queue if it was
        // postponed.
        else if postpone {
            let mut newfocusnode = NONE;

            // @todo should we re-install the old focus node in order to somehow set
            // the forks more clever?
            crate::tree::node_focus(scip, &mut newfocusnode, transprob, origprob, eventfilter, &mut cutoff, true, false)?;
        }

        // Compute number of successfully applied conflicts.
        let nsuccessconflicts = {
            let c = scip.conflict.as_ref().unwrap();
            c.npropsuccess + c.ninflpsuccess + c.nboundlpsuccess + c.nsbsuccess + c.npseudosuccess
        };

        // Trigger restart due to conflicts and the restart parameters allow
        // another restart.
        if nsuccessconflicts as f64 >= restartconfnum && restart_allowed(scip) {
            log::info!(
                "(run {}, node {}) restarting after {} successful conflict analysis calls",
                scip.stat.nruns,
                scip.stat.nnodes,
                nsuccessconflicts
            );
            restart = true;
            scip.stat.nconfrestarts += 1;
        }

        // Restart if the userrestart was set to true, we have still some nodes
        // left and the restart parameters allow another restart.
        restart = restart || (scip.stat.userrestart && crate::tree::tree_get_n_nodes(scip) > 0 && restart_allowed(scip));

        if restart_allowed(scip)
            && scip.set.limit_autorestartnodes as i64 == scip.stat.nnodes
            && scip.stat.ntotalnodes - scip.stat.nruns as i64 + 1 == scip.set.limit_autorestartnodes as i64
        {
            log::info!("(run {}, node {}) restarting: triggering parameter controlled restart)", scip.stat.nruns, scip.stat.nnodes);
            restart = true;
        }

        // If restart limit was exceeded, change the status; if status is
        // different from unknown, ie some other limit was hit, leave it
        // unchanged.
        if restart && scip.stat.status == Status::Unknown && scip.set.limit_restarts >= 0 && scip.stat.nruns > scip.set.limit_restarts {
            restart = false;
            scip.stat.status = Status::RestartLimit;
        }

        let _ = foundsol;
    }

    // Update the primal-dual integral if node or time limits were hit or an
    // interruption signal was called.
    if solve_is_stopped(scip, true) {
        let inf = scip.set.infinity();
        crate::stat::stat_update_primal_dual_integrals(scip, transprob, origprob, inf, -inf);
    }

    // Cuts off nodes with lower bound is not better than given cutoff bound,
    // manually; this necessary to ensure that SCIP terminates with a proper
    // solve stage.
    let cutoffbound = scip.primals[primal].cutoffbound;
    crate::tree::tree_cutoff(scip, eventfilter, cutoffbound)?;

    // If the current node is the only remaining node, and if its lower bound
    // exceeds the upper bound, we have to delete it manually in order to get to
    // the SOLVED stage instead of thinking, that only the gap limit was reached
    // (this may happen, if the current node is the one defining the global
    // lower bound and a feasible solution with the same value was found at this
    // node).
    let tfocus = treer!(scip).focusnode;
    if tfocus != NONE && crate::tree::tree_get_n_nodes(scip) == 0 && scip.set.is_ge(scip.nodes[tfocus].lowerbound, scip.primals[primal].cutoffbound) {
        focusnode = NONE;
        crate::tree::node_focus(scip, &mut focusnode, transprob, origprob, eventfilter, &mut cutoff, false, false)?;
    }

    // Check whether we finished solving.
    if crate::tree::tree_get_n_nodes(scip) == 0 && crate::tree::tree_get_current_node(scip) == NONE {
        // No restart necessary.
        restart = false;

        // Set the solution status.
        if unbounded || scip.set.is_infinity(-api::get_upperbound(scip)) {
            if scip.primals[primal].nsols > 0 {
                // Switch status to UNBOUNDED.
                scip.stat.status = Status::Unbounded;
            } else {
                // Switch status to INFORUNB.
                scip.stat.status = Status::InfOrUnbd;
            }
        } else if scip.primals[primal].nlimsolsfound == 0 {
            // Switch status to INFEASIBLE.
            scip.stat.status = Status::Infeasible;
        } else {
            // Switch status to OPTIMAL.
            scip.stat.status = Status::Optimal;
        }
    }

    Ok(restart)
}

/// Whether the relaxation solution is to be used instead of the LP one: it
/// is valid, includes the LP, and the LP is missing or has a worse
/// objective value.
fn relax_sol_preferred(scip: &mut Scip, prob: usize) -> bool {
    if !(crate::relax::relaxation_is_sol_valid(scip) && crate::relax::relaxation_is_lp_included_for_sol(scip)) {
        return false;
    }
    if !crate::tree::tree_has_focus_node_lp(scip) {
        return true;
    }
    let relaxobj = crate::relax::relaxation_get_sol_obj(scip);
    let lpobj = crate::lp::lp_get_objval(scip, prob);
    scip.set.is_gt(relaxobj, lpobj)
}
