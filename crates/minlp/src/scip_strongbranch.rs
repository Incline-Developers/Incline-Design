//! The strong branching part of `scip_var.c`: starting and ending strong
//! branching, strong branching on fractional columns with and without
//! domain propagation, and the access to the last strong branching data.

use crate::{
    api,
    prob::{ORIGPROB, TRANSPROB},
    retcode::{Retcode, ScipResult},
    scip::{GLOBALEVENTFILTER, Scip},
    scip_probing,
    tree::tree_probing,
    types::{BoundType, BranchDir, LpSolStat, VarId, VarStatus},
};

/// `SCIPisStrongbranchDownFirst`: returns whether the down child should be
/// regarded first in strong branching on `var`.
pub fn is_strongbranch_down_first(scip: &Scip, var: VarId) -> bool {
    use crate::types::LockType;
    match scip.set.branch_firstsbchild {
        b'u' => false,
        b'd' => true,
        b'a' => crate::var::var_get_n_locks_down_type(scip, var, LockType::Model) > crate::var::var_get_n_locks_up_type(scip, var, LockType::Model),
        _ => crate::var::var_get_avg_cutoffs(scip, var, BranchDir::Downwards) > crate::var::var_get_avg_cutoffs(scip, var, BranchDir::Upwards),
    }
}

/// `SCIPstartStrongbranch`: start strong branching - call before any strong
/// branching.
pub fn start_strongbranch(scip: &mut Scip, enablepropagation: bool) -> ScipResult {
    // Start probing mode to allow propagation before solving the strong
    // branching LPs; if no propagation should be done, set the LP to strong
    // branching mode.
    if enablepropagation {
        if tree_probing(scip) {
            // Cannot start strong branching with propagation while in probing
            // mode.
            return Err(Retcode::InvalidCall);
        }

        if scip.lp.is_some() && crate::lp::lp_diving(scip) {
            // Cannot start strong branching with propagation while in diving
            // mode.
            return Err(Retcode::InvalidCall);
        }

        // Other then in SCIPstartProbing(), we do not disable collecting
        // variable statistics during strong branching; we also don't use a
        // separate separation storage.
        crate::tree::tree_start_probing(scip, TRANSPROB, true)?;

        // Inform the LP that the current probing mode is used for strong
        // branching.
        crate::lp::lp_start_strongbranch_probing(scip);
    } else {
        crate::lp::lp_start_strongbranch(scip)?;
    }

    // Reset local strong branching info.
    scip.stat.lastsblpsolstats = [LpSolStat::NotSolved, LpSolStat::NotSolved];

    Ok(())
}

/// `SCIPendStrongbranch`: end strong branching - call after any strong
/// branching.
pub fn end_strongbranch(scip: &mut Scip) -> ScipResult {
    // Depending on whether the strong branching mode was started with
    // propagation enabled or not, we end the strong branching probing mode or
    // the LP strong branching mode.
    if tree_probing(scip) {
        // Collect all bound changes deducted during probing, which were applied
        // at the probing root and apply them to the parent node.
        let node = crate::tree::tree_get_current_node(scip);
        let mut changes: Vec<(VarId, f64, BoundType)> = Vec::new();
        if let Some(domchg) = scip.nodes[node].domchg.as_ref() {
            for bc in domchg.boundchgs.iter().take(domchg.nboundchgs as usize) {
                // Ignore redundant bound changes.
                if bc.redundant {
                    continue;
                }
                changes.push((bc.var, bc.newbound, bc.boundtype));
            }
        }

        // Inform the LP that the probing mode is not used for strong branching
        // anymore.
        crate::lp::lp_end_strongbranch_probing(scip);

        // Switch back from probing to normal operation mode and restore
        // variables and constraints to focus node.
        crate::tree::tree_end_probing(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;

        // Apply the collected bound changes.
        for (var, bound, bt) in changes {
            if bt == BoundType::Lower {
                crate::scip_var::chg_var_lb(scip, var, bound)?;
            } else {
                crate::scip_var::chg_var_ub(scip, var, bound)?;
            }
        }
    } else {
        crate::lp::lp_end_strongbranch(scip)?;
    }

    Ok(())
}

/// `analyzeStrongbranch`: analyze the strong branching for the given
/// variable; that includes conflict analysis for infeasible branches and
/// storing of root reduced cost information; returns `(downinf, upinf,
/// downconflict, upconflict)`.
fn analyze_strongbranch(scip: &mut Scip, var: VarId) -> ScipResult<(bool, bool, bool, bool)> {
    let col = scip.vars[var].data.col;
    let cutoffbound = scip.lp.as_ref().unwrap().cutoffbound;
    let c = &scip.cols[col];
    let downcutoff = c.sbdownvalid && scip.set.is_ge(c.sbdown, cutoffbound);
    let upcutoff = c.sbupvalid && scip.set.is_ge(c.sbup, cutoffbound);
    let (downinf, upinf) = (downcutoff, upcutoff);
    let mut downconflict = false;
    let mut upconflict = false;

    // Analyze infeasible strong branching sub problems: because the strong
    // branching's bound change is necessary for infeasibility, it cannot be
    // undone; therefore, infeasible strong branchings on non-binary variables
    // will not produce a valid conflict constraint.
    if scip.set.conf_enable && scip.set.conf_usesb && scip.set.nconflicthdlrs > 0 && crate::var::var_is_binary(scip, var) && crate::tree::tree_get_current_depth(scip) > 0 {
        let c = &scip.cols[col];
        let (primsol, lb, ub) = (c.primsol, c.lb, c.ub);
        if (downcutoff && scip.set.feas_ceil(primsol - 1.0) >= lb - 0.5) || (upcutoff && scip.set.feas_floor(primsol + 1.0) <= ub + 0.5) {
            (downconflict, upconflict) = crate::conflict::conflict_analyze_strongbranch(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, col)?;
        }
    }

    // The strong branching results can be used to strengthen the root reduced
    // cost information which is used for example to propagate against the
    // cutoff bound.
    if crate::tree::tree_get_current_depth(scip) == 0 && crate::var::var_is_binary(scip, var) && crate::lp::lp_is_dual_reliable(scip) {
        let lpobjval = crate::lp::lp_get_objval(scip, TRANSPROB);
        let c = &scip.cols[col];
        let (primsol, lb, ub, sbdown, sbup, sbdownvalid, sbupvalid) = (c.primsol, c.lb, c.ub, c.sbdown, c.sbup, c.sbdownvalid, c.sbupvalid);

        if sbdownvalid && scip.set.feas_ceil(primsol - 1.0) >= lb - 0.5 && lpobjval < sbdown {
            let gub = scip.vars[var].glbdom.ub;
            crate::var::var_update_best_root_sol(scip, var, gub, -(sbdown - lpobjval), lpobjval);
        }
        if sbupvalid && scip.set.feas_floor(primsol + 1.0) <= ub + 0.5 && lpobjval < sbup {
            let glb = scip.vars[var].glbdom.lb;
            crate::var::var_update_best_root_sol(scip, var, glb, sbup - lpobjval, lpobjval);
        }
    }

    Ok((downinf, upinf, downconflict, upconflict))
}

/// The results of strong branching on one variable.
#[derive(Clone, Copy, Debug, Default)]
pub struct Strongbranch {
    /// Dual bound after branching column down.
    pub down: f64,
    /// Dual bound after branching column up.
    pub up: f64,
    /// Whether the down value is a valid dual bound; otherwise, it can only be
    /// used as an estimate value.
    pub downvalid: bool,
    /// Whether the up value is a valid dual bound.
    pub upvalid: bool,
    /// Whether the downwards branch is infeasible.
    pub downinf: bool,
    /// Whether the upwards branch is infeasible.
    pub upinf: bool,
    /// Whether a conflict constraint was created for an infeasible downwards
    /// branch.
    pub downconflict: bool,
    /// Whether a conflict constraint was created for an infeasible upwards
    /// branch.
    pub upconflict: bool,
    /// Whether an unresolved LP error occurred or the solving process should
    /// be stopped (e.g., due to a time limit).
    pub lperror: bool,
    /// The number of domain reductions down (with propagation).
    pub ndomredsdown: i64,
    /// The number of domain reductions up (with propagation).
    pub ndomredsup: i64,
}

/// `SCIPgetVarStrongbranchFrac`: gets strong branching information on
/// column variable with fractional value; before calling this method, the
/// strong branching mode must have been activated by calling
/// `SCIPstartStrongbranch()`.
pub fn get_var_strongbranch_frac(scip: &mut Scip, var: VarId, itlim: i32, idempotent: bool) -> ScipResult<Strongbranch> {
    let mut r = Strongbranch::default();
    let lpobjval = api::get_lp_objval(scip);

    if scip.vars[var].varstatus != VarStatus::Column {
        // Cannot get strong branching information on non-COLUMN variable.
        return Err(Retcode::InvalidData);
    }

    let col = scip.vars[var].data.col;

    if scip.cols[col].lppos < 0 {
        // Cannot get strong branching information on variable not in current LP.
        return Err(Retcode::InvalidData);
    }

    // Check if the solving process should be aborted.
    if crate::solve::solve_is_stopped(scip, false) {
        // Mark this as if the LP failed.
        r.lperror = true;
        return Ok(r);
    }

    // Call strong branching for column with fractional value.
    let sb = crate::lp::col_get_strongbranch(scip, col, false, TRANSPROB, itlim, !idempotent, !idempotent, true, true)?;
    let (mut localdown, mut localup, mut localdownvalid, mut localupvalid) = (sb.down, sb.up, sb.downvalid, sb.upvalid);
    r.lperror = sb.lperror;

    // Update lower bound by the higher pseudo objective value.
    if !scip.set.is_zero(scip.vars[var].obj) {
        let boundtype = crate::var::var_get_best_bound_type(scip, var);
        let lpsol = crate::var::var_get_lp_sol(scip, var);
        let (oldbound, newbound) = if boundtype == BoundType::Upper {
            (scip.vars[var].locdom.ub, scip.set.feas_floor(lpsol))
        } else {
            (scip.vars[var].locdom.lb, scip.set.feas_ceil(lpsol))
        };

        let pseudoobjval = crate::lp::lp_get_modified_pseudo_objval(scip, TRANSPROB, var, oldbound, newbound, boundtype);

        if pseudoobjval > lpobjval {
            if boundtype == BoundType::Upper {
                if !localdownvalid || localdown < pseudoobjval {
                    localdown = pseudoobjval;
                    localdownvalid = true;
                }
            } else if !localupvalid || localup < pseudoobjval {
                localup = pseudoobjval;
                localupvalid = true;
            }
        }
    }

    // Check, if the branchings are infeasible; in exact solving mode, we
    // cannot trust the strong branching enough to declare the sub problems
    // infeasible.
    if !r.lperror && crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) {
        if !idempotent {
            let (_, _, dc, uc) = analyze_strongbranch(scip, var)?;
            r.downconflict = dc;
            r.upconflict = uc;
        }

        let cutoffbound = scip.lp.as_ref().unwrap().cutoffbound;
        r.downinf = localdownvalid && scip.set.is_ge(localdown, cutoffbound);
        r.upinf = localupvalid && scip.set.is_ge(localup, cutoffbound);
    }

    r.down = localdown;
    r.up = localup;
    r.downvalid = localdownvalid;
    r.upvalid = localupvalid;

    Ok(r)
}

/// The outputs of one strong branching child that
/// `performStrongbranchWithPropagation` always writes.
#[derive(Default)]
struct ChildResult {
    lperror: bool,
    foundsol: bool,
    cutoff: bool,
}

/// `SCIPtryStrongbranchLPSol`: checks the solution of the current strong
/// branching LP for feasibility; returns `(foundsol, cutoff)` (cutoff only
/// set, never cleared, as in C).
pub fn try_strongbranch_lp_sol(scip: &mut Scip, foundsol: &mut bool, cutoff: &mut bool) -> ScipResult {
    if scip.set.branch_checksbsol {
        let mut rounded = true;
        let value = api::get_lp_objval(scip);
        let oldnbestsolsfound = scip.primals[crate::scip::TRANSPRIMAL].nbestsolsfound;

        // Start clock for strong branching solutions.
        let enabled = scip.set.time_enabled;
        scip.stat.sbsoltime.start(enabled);

        let sol = crate::sol::sol_create_lp_sol(scip, TRANSPROB, crate::scip::TRANSPRIMAL, crate::var::NONE)?;
        crate::sol::sol_set_strongbranching(scip, sol);

        // Try to round the strong branching solution.
        if scip.set.branch_roundsbsol {
            rounded = crate::sol::sol_round(scip, sol, TRANSPROB)?;
        }

        // Check the solution for feasibility if rounding worked well (or was not
        // tried).
        if rounded {
            *foundsol = api::try_sol_free(scip, sol, false, false, false, true, false)?;
        } else {
            api::free_sol(scip, sol)?;
        }

        if *foundsol {
            scip.stat.nsbsolsfound += 1;

            if scip.primals[crate::scip::TRANSPRIMAL].nbestsolsfound != oldnbestsolsfound {
                scip.stat.nsbbestsolsfound += 1;
            }

            if scip.set.is_ge(value, api::get_cutoffbound(scip)) {
                *cutoff = true;
            }
        }

        // Stop clock for strong branching solutions.
        scip.stat.sbsoltime.stop(enabled);
    }

    Ok(())
}

/// `performStrongbranchWithPropagation`: create, solve, and evaluate a single
/// strong branching child (for strong branching with propagation).
#[allow(clippy::too_many_arguments)]
fn perform_strongbranch_with_propagation(
    scip: &mut Scip,
    var: VarId,
    down: bool,
    firstchild: bool,
    propagate: bool,
    newbound: f64,
    itlim: i32,
    maxproprounds: i32,
    value: &mut f64,
    valid: &mut bool,
    ndomreductions: &mut i64,
    conflict: &mut bool,
    vars: &[VarId],
    mut newbounds: Option<(&mut [f64], &mut [f64])>,
) -> ScipResult<ChildResult> {
    let inf = scip.set.infinity();
    let mut r = ChildResult::default();

    // Check whether the strong branching child is already infeasible due to
    // the bound change; since this means that solval is not within the bounds,
    // this should only happen if previous strong branching calls on other
    // variables detected bound changes which are valid for and were already
    // applied at the probing root; bound changes are applied in
    // SCIPendStrongbranch(), which can be seen as a conflict constraint.
    if (down && newbound < scip.vars[var].locdom.lb - 0.5) || (!down && newbound > scip.vars[var].locdom.ub + 0.5) {
        *value = inf;
        *valid = true;
        *conflict = true;
        r.cutoff = true;
        return Ok(r);
    }

    // We need to ensure that we can create at least one new probing node
    // without exceeding the maximal tree depth.
    if crate::def::MAXTREEDEPTH > crate::tree::tree_get_probing_depth(scip) {
        // Create a new probing node for the strong branching child and apply the
        // new bound for the variable.
        scip_probing::new_probing_node(scip)?;

        if down {
            if scip.set.is_lt(newbound, scip.vars[var].locdom.ub) {
                scip_probing::chg_var_ub_probing(scip, var, newbound)?;
            }
        } else if scip.set.is_gt(newbound, scip.vars[var].locdom.lb) {
            scip_probing::chg_var_lb_probing(scip, var, newbound)?;
        }
    } else {
        *valid = false;
        r.cutoff = false;
        *conflict = false;
        return Ok(r);
    }

    // Propagate domains at the probing node.
    if propagate {
        // Start time measuring.
        let enabled = scip.set.time_enabled;
        scip.stat.strongpropclock.start(enabled);

        let (cutoff, ndomreds) = scip_probing::propagate_probing(scip, maxproprounds)?;
        r.cutoff = cutoff;

        // Store number of domain reductions in strong branching.
        if down {
            scip.stat.nsbdowndomchgs += ndomreds;
        } else {
            scip.stat.nsbupdomchgs += ndomreds;
        }

        *ndomreductions = ndomreds;

        // Stop time measuring.
        scip.stat.strongpropclock.stop(enabled);

        if r.cutoff {
            *value = inf;
            *valid = true;
        }
    }

    // If propagation did not already detect infeasibility, solve the probing
    // LP.
    if !r.cutoff {
        (r.lperror, r.cutoff) = scip_probing::solve_probing_lp(scip, itlim)?;

        if r.cutoff {
            *value = inf;
            *valid = true;
        } else if !r.lperror {
            // Save the lp solution status.
            let solstat = api::get_lp_solstat(scip);
            scip.stat.lastsblpsolstats[if down { 0 } else { 1 }] = solstat;

            match solstat {
                LpSolStat::Optimal => {
                    *value = api::get_lp_objval(scip);
                    *valid = true;

                    // Check the strong branching LP solution for feasibility.
                    try_strongbranch_lp_sol(scip, &mut r.foundsol, &mut r.cutoff)?;
                }
                LpSolStat::IterLimit | LpSolStat::TimeLimit => {
                    if solstat == LpSolStat::IterLimit {
                        scip.stat.nsbtimesiterlimhit += 1;
                    }

                    // Use LP value as estimate; we access the LPI directly, because
                    // when a time limit was hit, we cannot access objective value and
                    // dual feasibility using the SCIPlp... methods.
                    let lpi = scip.lp.as_deref_mut().unwrap().lpi.as_deref_mut().unwrap();
                    if crate::lpi::lpi_was_solved(lpi) {
                        let objval = crate::lpi::lpi_get_objval(lpi)?;
                        let dualfeasible = crate::lpi::lpi_is_dual_feasible(lpi);
                        let looseobjval = crate::lp::lp_get_loose_objval(scip, TRANSPROB);

                        // The infinity value in the LPI should not be smaller than SCIP's
                        // infinity value; we use SCIP's infinity value here because a
                        // value larger than this is counted as infeasible by SCIP.
                        *value = if scip.set.is_infinity(objval) {
                            inf
                        } else if scip.set.is_infinity(-looseobjval) {
                            -inf
                        } else {
                            objval + looseobjval
                        };

                        if dualfeasible {
                            *valid = true;

                            if scip.set.is_ge(*value, api::get_cutoffbound(scip)) {
                                r.cutoff = true;
                            }
                        }
                    }
                }
                LpSolStat::Error | LpSolStat::UnboundedRay => {
                    r.lperror = true;
                }
                _ => {
                    // Invalid LP solution status.
                    return Err(Retcode::InvalidData);
                }
            }
        }

        // If columns are missing in the LP, the cutoff flag may be wrong.
        // Therefore, we need to set it and the valid pointer to false here.
        if r.cutoff && !crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) {
            r.cutoff = false;
        }
    }

    // If the subproblem was feasible, we store the local bounds of the
    // variables after propagation and (possibly) conflict analysis.
    if !r.cutoff
        && let Some((newlbs, newubs)) = newbounds.as_mut()
    {
        if firstchild {
            // Initialize the newlbs and newubs to the current local bounds.
            for (v, &x) in vars.iter().enumerate() {
                newlbs[v] = scip.vars[x].locdom.lb;
                newubs[v] = scip.vars[x].locdom.ub;
            }
        } else {
            // Update newlbs and newubs: take the weaker of the already stored
            // bounds and the current local bounds.
            for (v, &x) in vars.iter().enumerate() {
                let lb = scip.vars[x].locdom.lb;
                let ub = scip.vars[x].locdom.ub;
                newlbs[v] = crate::def::min(newlbs[v], lb);
                newubs[v] = crate::def::max(newubs[v], ub);
            }
        }
    }

    // Revert all changes at the probing node.
    scip_probing::backtrack_probing(scip, 0)?;

    Ok(r)
}

/// `SCIPgetVarStrongbranchWithPropagation`: gets strong branching
/// information with previous domain propagation on column variable;
/// `maxproprounds` -1 for no limit, -2 for the parameter settings; the
/// valid bounds for all active variables are stored in `newbounds` if
/// given.
#[allow(clippy::too_many_arguments)]
pub fn get_var_strongbranch_with_propagation(
    scip: &mut Scip,
    var: VarId,
    solval: f64,
    lpobjval: f64,
    itlim: i32,
    mut maxproprounds: i32,
    mut newbounds: Option<(&mut [f64], &mut [f64])>,
) -> ScipResult<Strongbranch> {
    let inf = scip.set.infinity();

    // Check whether propagation should be performed.
    let propagate = maxproprounds != 0 && maxproprounds != -3;

    // Check, if all existing columns are in LP. If this is not the case, we may
    // still return that the up and down dual bounds are valid, because the
    // branching rule should not apply them otherwise. However, we must not set
    // the downinf or upinf pointers to TRUE based on the dual bound, because
    // we cannot guarantee that this node can be cut off.
    let allcolsinlp = crate::prob::prob_all_cols_in_lp(scip, TRANSPROB);

    // If maxproprounds is -2, change it to 0, which for the following calls
    // means using the parameter settings.
    if maxproprounds == -2 {
        maxproprounds = 0;
    }

    let mut r = Strongbranch {
        down: lpobjval,
        up: lpobjval,
        ..Default::default()
    };

    let vars = api::get_vars(scip).to_vec();

    scip.stat.lastsblpsolstats = [LpSolStat::NotSolved, LpSolStat::NotSolved];

    // Check if the solving process should be aborted.
    if crate::solve::solve_is_stopped(scip, false) {
        // Mark this as if the LP failed.
        r.lperror = true;
        return Ok(r);
    }

    if scip.vars[var].varstatus != VarStatus::Column {
        // Cannot get strong branching information on non-COLUMN variable.
        return Err(Retcode::InvalidData);
    }

    let col = scip.vars[var].data.col;

    if scip.cols[col].lppos < 0 {
        // Cannot get strong branching information on variable not in current LP.
        return Err(Retcode::InvalidData);
    }

    let newlb = scip.set.feas_floor(solval + 1.0);
    let newub = scip.set.feas_ceil(solval - 1.0);

    // The up branch is infeasible due to the branching bound change; since this
    // means that solval is not within the bounds, this should only happen if
    // previous strong branching calls on other variables detected bound changes
    // which are valid for and were already applied at the probing root; bound
    // changes are applied in SCIPendStrongbranch(), which can be seen as a
    // conflict constraint; we do not regard the down branch: its valid flag
    // stays false.
    if newlb > scip.vars[var].locdom.ub + 0.5 {
        r.up = inf;
        r.upinf = true;
        r.upvalid = true;
        r.upconflict = true;
        crate::lp::col_set_strongbranch_data(scip, col, lpobjval, solval, r.down, r.up, false, true, 0, i32::MAX);
        return Ok(r);
    }

    // The down branch is infeasible due to the branching bound change (see
    // above).
    if newub < scip.vars[var].locdom.lb - 0.5 {
        r.down = inf;
        r.downinf = true;
        r.downvalid = true;
        r.downconflict = true;
        crate::lp::col_set_strongbranch_data(scip, col, lpobjval, solval, r.down, r.up, true, false, 0, i32::MAX);
        return Ok(r);
    }

    // We now do strong branching by creating the two potential child nodes as
    // probing nodes and solving them one after the other. We will stop when
    // the first child is detected infeasible, saving the effort we would need
    // for the second child. Since empirically, the up child tends to be
    // infeasible more often, we do strongbranching first on the up branch.
    let oldniters = scip.stat.nsbdivinglpiterations;
    let mut firstchild = true;
    let mut cutoff = false;

    // Switch conflict analysis according to usesb parameter.
    let enabledconflict = scip.set.conf_enable;
    scip.set.conf_enable = scip.set.conf_enable && scip.set.conf_usesb;

    // @todo: decide the branch to look at first based on the cutoffs in
    // previous calls?
    let mut downchild = is_strongbranch_down_first(scip, var);

    let mut downvalidlocal = false;
    let mut upvalidlocal = false;

    let nconflicts = |scip: &Scip| scip.conflict.as_ref().unwrap().nconflictsets;

    loop {
        let oldnconflicts = nconflicts(scip);

        let reborrow = newbounds.as_mut().map(|(l, u)| (&mut **l, &mut **u));
        if downchild {
            let c = perform_strongbranch_with_propagation(
                scip,
                var,
                downchild,
                firstchild || cutoff,
                propagate,
                newub,
                itlim,
                maxproprounds,
                &mut r.down,
                &mut downvalidlocal,
                &mut r.ndomredsdown,
                &mut r.downconflict,
                &vars,
                reborrow,
            )?;
            r.lperror = c.lperror;
            cutoff = c.cutoff;

            // Check for infeasibility.
            if cutoff {
                r.downinf = true;
                if scip.vars[var].locdom.lb > newub + 0.5 || nconflicts(scip) > oldnconflicts {
                    r.downconflict = true;
                }
            }

            // Check whether new solutions rendered the other child infeasible.
            if c.foundsol && allcolsinlp && scip.set.is_ge(if upvalidlocal { r.up } else { lpobjval }, api::get_cutoffbound(scip)) {
                r.upinf = true;
                upvalidlocal = true;
                cutoff = true;
            }
        } else {
            let c = perform_strongbranch_with_propagation(
                scip,
                var,
                downchild,
                firstchild || cutoff,
                propagate,
                newlb,
                itlim,
                maxproprounds,
                &mut r.up,
                &mut upvalidlocal,
                &mut r.ndomredsup,
                &mut r.upconflict,
                &vars,
                reborrow,
            )?;
            r.lperror = c.lperror;
            cutoff = c.cutoff;

            // Check for infeasibility.
            if cutoff {
                r.upinf = true;
                if scip.vars[var].locdom.ub < newlb - 0.5 || nconflicts(scip) > oldnconflicts {
                    r.upconflict = true;
                }
            }

            // Check whether new solutions rendered the other child infeasible.
            if c.foundsol && allcolsinlp && scip.set.is_ge(if downvalidlocal { r.down } else { lpobjval }, api::get_cutoffbound(scip)) {
                r.downinf = true;
                downvalidlocal = true;
                cutoff = true;
            }
        }

        downchild = !downchild;
        firstchild = !firstchild;

        // If a child is cut off and forceall is FALSE, we do not regard the other
        // child.
        if !(!firstchild && (!cutoff || scip.set.branch_forceall)) {
            break;
        }
    }

    // Update lower bound by the higher pseudo objective value.
    if !r.downinf && !r.upinf && !scip.set.is_zero(scip.vars[var].obj) {
        let boundtype = crate::var::var_get_best_bound_type(scip, var);
        let (oldbound, newbound) = if boundtype == BoundType::Upper {
            (scip.vars[var].locdom.ub, newub)
        } else {
            (scip.vars[var].locdom.lb, newlb)
        };

        let pseudoobjval = crate::lp::lp_get_modified_pseudo_objval(scip, TRANSPROB, var, oldbound, newbound, boundtype);

        if pseudoobjval > lpobjval {
            if boundtype == BoundType::Upper {
                if !downvalidlocal || r.down < pseudoobjval {
                    r.down = pseudoobjval;
                    downvalidlocal = true;
                    if allcolsinlp && scip.set.is_ge(r.down, api::get_cutoffbound(scip)) {
                        r.downinf = true;
                    }
                }
            } else if !upvalidlocal || r.up < pseudoobjval {
                r.up = pseudoobjval;
                upvalidlocal = true;
                if allcolsinlp && scip.set.is_ge(r.up, api::get_cutoffbound(scip)) {
                    r.upinf = true;
                }
            }
        }
    }

    // Set strong branching information in column.
    if r.lperror {
        crate::lp::col_invalidate_strongbranch_data(scip, col);
    } else {
        let iters = scip.stat.nsbdivinglpiterations - oldniters;
        crate::lp::col_set_strongbranch_data(scip, col, lpobjval, solval, r.down, r.up, downvalidlocal, upvalidlocal, iters, itlim);
    }

    r.downvalid = downvalidlocal;
    r.upvalid = upvalidlocal;

    scip.set.conf_enable = enabledconflict;

    Ok(r)
}

/// `SCIPgetVarStrongbranchLast`: gets strong branching information on
/// column variable x with integral LP solution value (val); returns `(down,
/// up, downvalid, upvalid, solval, lpobjval)` of the last strong branching
/// call.
pub fn get_var_strongbranch_last(scip: &Scip, var: VarId) -> ScipResult<(f64, f64, bool, bool, f64, f64)> {
    if scip.vars[var].varstatus != VarStatus::Column {
        // Cannot get strong branching information on non-COLUMN variable.
        return Err(Retcode::InvalidData);
    }

    Ok(crate::lp::col_get_strongbranch_last(scip, scip.vars[var].data.col))
}

/// `SCIPgetVarStrongbranchNode`: gets node number of the last node in
/// current branch and bound run, where strong branching was used on the
/// given variable, or -1 if strong branching was never applied to the
/// variable in current run.
pub fn get_var_strongbranch_node(scip: &Scip, var: VarId) -> i64 {
    if scip.vars[var].varstatus != VarStatus::Column {
        return -1;
    }

    scip.cols[scip.vars[var].data.col].sbnode
}

/// `SCIPgetVarStrongbranchLPAge`: gets number of LPs solved since last
/// strong branching call on the given variable.
pub fn get_var_strongbranch_lp_age(scip: &Scip, var: VarId) -> i64 {
    if scip.vars[var].varstatus != VarStatus::Column {
        return i64::MAX;
    }

    crate::lp::col_get_strongbranch_lp_age(scip, scip.vars[var].data.col)
}
