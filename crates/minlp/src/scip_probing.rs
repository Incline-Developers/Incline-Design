//! Port of `scip_probing.c`: public methods for the probing mode. Pricing
//! during probing, relaxators and the diving set statistics are not ported.

use crate::{
    api,
    prob::{ORIGPROB, TRANSPROB},
    retcode::{Retcode, ScipResult},
    scip::{GLOBALEVENTFILTER, Scip, Stage},
    sepastore::{SEPASTORE, SEPASTOREPROBING},
    tree::{tree_get_current_node, tree_get_probing_depth, tree_probing},
    types::{BoundType, LpSolStat, VarId},
    var,
};

/// `SCIPstartProbing`: initiates probing, making methods `SCIPnewProbingNode()`,
/// `SCIPbacktrackProbing()`, `SCIPchgVarLbProbing()`, ... available.
pub fn start_probing(scip: &mut Scip) -> ScipResult {
    if tree_probing(scip) {
        // Already in probing mode.
        return Err(Retcode::InvalidCall);
    }

    if scip.lp.is_some() && crate::lp::lp_diving(scip) {
        // Cannot start probing while in diving mode.
        return Err(Retcode::InvalidCall);
    }

    // Use a different separation storage for probing mode; otherwise SCIP will
    // remove the cuts that are currently in the separation storage after
    // solving an LP in probing mode.
    // Before initSolve both stores are NULL in C (swapping them is a no-op).
    if scip.sepastores.len() > SEPASTOREPROBING {
        scip.sepastores.swap(SEPASTORE, SEPASTOREPROBING);
    }

    crate::tree::tree_start_probing(scip, TRANSPROB, false)?;

    // Disables the collection of any statistic for a variable.
    crate::stat::stat_disable_var_history(scip);

    Ok(())
}

/// `SCIPnewProbingNode`: creates a new probing sub node, whose changes can
/// be undone by backtracking to a higher node in the probing path.
pub fn new_probing_node(scip: &mut Scip) -> ScipResult {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    // A MaxDepthLevel error is only warned about in C before passing it on.
    crate::tree::tree_create_probing_node_pub(scip)
}

/// `SCIPgetProbingDepth`: returns the current probing depth.
pub fn get_probing_depth(scip: &Scip) -> i32 {
    if !tree_probing(scip) {
        // Not in probing mode.
        return -1;
    }

    tree_get_probing_depth(scip)
}

/// `SCIPbacktrackProbing`: undoes all changes to the problem applied in
/// probing up to the given probing depth.
pub fn backtrack_probing(scip: &mut Scip, probingdepth: i32) -> ScipResult {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    if probingdepth < 0 || probingdepth > tree_get_probing_depth(scip) {
        // Backtracking probing depth out of current probing range.
        return Err(Retcode::InvalidData);
    }

    crate::tree::tree_backtrack_probing_pub(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, probingdepth)
}

/// `SCIPendProbing`: quits probing and resets bounds and constraints to the
/// focus node's environment.
pub fn end_probing(scip: &mut Scip) -> ScipResult {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    // Switch back from probing to normal operation mode and restore variables
    // and constraints to focus node.
    crate::tree::tree_end_probing(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;

    // Enables the collection of statistics for a variable.
    crate::stat::stat_enable_var_history(scip);

    // Switch to the original separation storage.
    // Before initSolve both stores are NULL in C (swapping them is a no-op).
    if scip.sepastores.len() > SEPASTOREPROBING {
        scip.sepastores.swap(SEPASTORE, SEPASTOREPROBING);
    }

    Ok(())
}

/// The probing bound change (`SCIPnodeAddBoundchg` at the current node
/// with `probingchange` set).
fn add_probing_boundchg(scip: &mut Scip, var: VarId, newbound: f64, boundtype: BoundType) -> ScipResult {
    let node = tree_get_current_node(scip);
    crate::tree::node_add_boundchg(scip, node, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, var, newbound, boundtype, true)
}

/// `SCIPchgVarLbProbing`: injects a change of variable's lower bound into
/// current probing node; the same can also be achieved with a call to
/// `SCIPchgVarLb()`, but in this case, the bound change would be treated
/// like a deduction instead of a branching decision.
pub fn chg_var_lb_probing(scip: &mut Scip, v: VarId, mut newbound: f64) -> ScipResult {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    var::var_adjust_lb(scip, v, &mut newbound);

    // Ignore tightenings of lower bounds to +infinity during solving process.
    if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
        return Ok(());
    }

    add_probing_boundchg(scip, v, newbound, BoundType::Lower)
}

/// `SCIPchgVarUbProbing`: the upper bound counterpart of
/// [`chg_var_lb_probing`].
pub fn chg_var_ub_probing(scip: &mut Scip, v: VarId, mut newbound: f64) -> ScipResult {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    var::var_adjust_ub(scip, v, &mut newbound);

    // Ignore tightenings of upper bounds to -infinity during solving process.
    if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
        return Ok(());
    }

    add_probing_boundchg(scip, v, newbound, BoundType::Upper)
}

/// `SCIPgetVarObjProbing`: gets variable's objective value in current
/// probing.
pub fn get_var_obj_probing(scip: &Scip, v: VarId) -> f64 {
    if !tree_probing(scip) {
        // Not in probing mode.
        return crate::def::INVALID;
    }

    var::var_get_obj_lp(scip, v)
}

/// `SCIPchgVarObjProbing`: changes (column) variable's objective value
/// during probing mode.
pub fn chg_var_obj_probing(scip: &mut Scip, v: VarId, newobj: f64) -> ScipResult {
    if !tree_probing(scip) {
        log::error!("not in probing mode");
        return Err(Retcode::InvalidCall);
    }

    // Get current probing node.
    let node = tree_get_current_node(scip);

    // Get old objective function value.
    let oldobj = scip.vars[v].obj;

    if scip.set.is_eq(oldobj, newobj) {
        return Ok(());
    }

    {
        let pn = scip.nodes[node].data.probingnode.as_mut().unwrap();
        pn.origobjvars.0.push(v);
        pn.origobjvals.0.push(oldobj);
        pn.nchgdobjs += 1;
    }
    scip.tree.as_mut().unwrap().probingsumchgdobjs += 1;

    // Inform tree and LP that the objective was changed and invalidate the
    // LP's cutoff bound, since this has nothing to do with the current
    // objective value anymore; the cutoff bound is reset in SCIPendProbing().
    if !scip.tree.as_ref().unwrap().probingobjchanged {
        let infinity = scip.set.infinity();
        crate::lp::lp_set_cutoffbound(scip, TRANSPROB, infinity)?;

        crate::tree::tree_mark_probing_obj_changed(scip);
        crate::lp::lp_mark_diving_obj_changed(scip);
    }

    // Perform the objective change.
    var::var_chg_obj(scip, v, TRANSPROB, newobj)
}

/// `SCIPaddRowProbing`: adds a row to the LP in current probing node.
pub fn add_row_probing(scip: &mut Scip, row: crate::types::RowId) -> ScipResult {
    if !tree_probing(scip) {
        log::error!("not in probing mode");
        return Err(Retcode::InvalidCall);
    }

    // Get depth of current node.
    let node = tree_get_current_node(scip);
    let depth = scip.nodes[node].depth as i32;

    crate::lp::lp_add_row(scip, row, depth)
}

/// `SCIPfixVarProbing`: injects a change of variable's bounds into current
/// probing node to fix the variable to the specified value.
pub fn fix_var_probing(scip: &mut Scip, v: VarId, fixedval: f64) -> ScipResult {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    // We adjust the fixing value here and compare the old bound with the
    // adjusted values because otherwise, it might happen that the unadjusted
    // value is better and we add the boundchange, but within
    // SCIPnodeAddBoundchg() the bounds are adjusted - using the feasibility
    // epsilon for integer variables - and it is asserted, that the bound is
    // still better than the old one which might then be incorrect.
    let mut fixlb = fixedval;
    let mut fixub = fixedval;
    var::var_adjust_lb(scip, v, &mut fixlb);
    var::var_adjust_ub(scip, v, &mut fixub);

    if scip.set.is_gt(fixlb, scip.vars[v].locdom.lb) {
        add_probing_boundchg(scip, v, fixlb, BoundType::Lower)?;
    }
    if scip.set.is_lt(fixub, scip.vars[v].locdom.ub) {
        add_probing_boundchg(scip, v, fixub, BoundType::Upper)?;
    }

    Ok(())
}

/// `SCIPisObjChangedProbing`: returns whether the objective function has
/// changed during probing mode.
pub fn is_obj_changed_probing(scip: &Scip) -> bool {
    scip.tree.is_some() && api::in_probing(scip) && crate::tree::tree_probing_obj_changed(scip)
}

/// `SCIPpropagateProbing`: applies domain propagation on the probing sub
/// problem, that was changed after SCIPstartProbing() was called; returns
/// `(cutoff, ndomredsfound)`.
pub fn propagate_probing(scip: &mut Scip, maxproprounds: i32) -> ScipResult<(bool, i64)> {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    let mut objchgs: Vec<(VarId, f64)> = Vec::new();
    let mut changedobj = false;

    // Undo objective changes if we want to propagate during probing.
    if scip.tree.as_ref().unwrap().probingobjchanged {
        let vars = api::get_vars(scip).to_vec();

        for v in vars {
            let unchanged = scip.vars[v].unchangedobj;
            let obj = get_var_obj_probing(scip, v);
            if !scip.set.is_eq(unchanged, obj) {
                objchgs.push((v, obj));
                var::var_chg_obj(scip, v, TRANSPROB, unchanged)?;
            }
        }

        crate::lp::lp_unmark_diving_obj_changed(scip);
        scip.tree.as_mut().unwrap().probingobjchanged = false;
        changedobj = true;
    }

    let s = &scip.stat;
    let mut ndomredsfound = -(s.nprobboundchgs + s.nprobholechgs);

    let mut cutoff = false;
    let depth = api::get_depth(scip);
    crate::solve::propagate_domains(
        scip,
        TRANSPROB,
        ORIGPROB,
        GLOBALEVENTFILTER,
        depth,
        maxproprounds,
        crate::cons::PROPTIMING_ALWAYS,
        &mut cutoff,
    )?;

    let s = &scip.stat;
    ndomredsfound += s.nprobboundchgs + s.nprobholechgs;

    // Restore old objective function.
    if changedobj {
        crate::lp::lp_mark_diving_obj_changed(scip);
        scip.tree.as_mut().unwrap().probingobjchanged = true;

        for (v, obj) in objchgs {
            var::var_chg_obj(scip, v, TRANSPROB, obj)?;
        }
    }

    Ok((cutoff, ndomredsfound))
}

/// `SCIPpropagateProbingImplications`: applies domain propagation on the
/// probing sub problem, that was changed after SCIPstartProbing() was
/// called; only propagations of the binary variables fixed at the current
/// probing node that are triggered by the implication graph and the clique
/// table are applied; returns whether the probing node can be cut off.
pub fn propagate_probing_implications(scip: &mut Scip) -> ScipResult<bool> {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    let node = tree_get_current_node(scip);
    crate::tree::node_propagate_implics(scip, node, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)
}

/// `solveProbingLP` without pricing: solves the LP at the current probing
/// node (cannot be applied at preprocessing stage); returns `(lperror,
/// cutoff)`.
fn solve_probing_lp_impl(scip: &mut Scip, itlim: i32) -> ScipResult<(bool, bool)> {
    if !tree_probing(scip) {
        // Not in probing mode.
        return Err(Retcode::InvalidCall);
    }

    let initcutoff = crate::solve::init_conss_lp(scip, SEPASTORE, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, false, false)?;

    if initcutoff {
        return Ok((false, true));
    }
    let mut cutoff = false;

    // Load the LP state (if necessary).
    crate::tree::tree_load_probing_lp_state(scip, TRANSPROB)?;

    // The LP is a relaxation if and only if the objective has not been
    // changed.
    let objchanged = scip.tree.as_ref().unwrap().probingobjchanged;
    crate::lp::lp_set_is_relax(scip, !objchanged);

    // Solve probing LP.
    let lperror = crate::lp::lp_solve_and_eval(scip, TRANSPROB, itlim as i64, false, false, false, false)?;

    // Mark the probing node to have a solved LP.
    if !lperror {
        crate::tree::tree_mark_probing_node_has_lp(scip)?;
    }

    // Remember that probing might have changed the LPi state; this holds even
    // if solving returned with an LP error.
    scip.tree.as_mut().unwrap().probingsolvedlp = true;

    // The LP is infeasible or the objective limit was reached.
    if !lperror {
        let solstat = crate::lp::lp_get_solstat(scip);
        let objchanged = scip.tree.as_ref().unwrap().probingobjchanged;
        let exceeded = solstat == LpSolStat::Optimal && !objchanged && {
            let objval = api::get_lp_objval(scip);
            scip.set.is_ge(objval, api::get_cutoffbound(scip))
        };
        if solstat == LpSolStat::Infeasible || solstat == LpSolStat::ObjLimit || exceeded {
            // Analyze the infeasible LP (only if all columns are in the LP and no
            // external pricers exist).
            if crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) && !scip.tree.as_ref().unwrap().probingobjchanged {
                crate::conflict::conflict_analyze_lp_public(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;
            }

            cutoff = true;
        }
    }

    Ok((lperror, cutoff))
}

/// `SCIPsolveProbingLP`: solves the LP at the current probing node (cannot
/// be applied at preprocessing stage); no separation or pricing is applied;
/// `itlim` -1 for no limit; returns `(lperror, cutoff)`.
pub fn solve_probing_lp(scip: &mut Scip, itlim: i32) -> ScipResult<(bool, bool)> {
    solve_probing_lp_impl(scip, itlim)
}
