//! SCIP's variable methods of the public API (`scip_var.c`): bound changes
//! and tightenings in every stage, inferences of constraints and
//! propagators, type changes, fixings and aggregations, and the branching
//! statistics getters the plugins use.
//!
//! Exact mode, printing and parsing are not ported. Out parameters become
//! return values: `(infeasible, tightened)` for tightenings.

use crate::{
    api,
    cvec::CVec,
    prob::{ORIGPROB, TRANSPROB},
    retcode::{Retcode, ScipResult},
    scip::{GLOBALEVENTFILTER, Scip, Stage},
    tree::{node_add_boundchg, node_add_boundinfer, tree_get_current_depth, tree_get_current_node},
    types::{BoundType, BranchDir, ConsId, ImplintType, PropId, VarId, VarStatus, VarType},
    var::{self, NONE},
};

/// `SCIPnodeAddBoundchg` with SCIP's problems and global event filter.
fn add_boundchg(scip: &mut Scip, node: usize, var: VarId, newbound: f64, boundtype: BoundType) -> ScipResult {
    node_add_boundchg(scip, node, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, var, newbound, boundtype, false)
}

/// `SCIPnodeAddBoundinfer` with SCIP's problems and global event filter.
fn add_boundinfer(scip: &mut Scip, node: usize, var: VarId, newbound: f64, boundtype: BoundType, infercons: ConsId, inferprop: PropId, inferinfo: i32) -> ScipResult {
    node_add_boundinfer(
        scip,
        node,
        TRANSPROB,
        ORIGPROB,
        GLOBALEVENTFILTER,
        var,
        newbound,
        boundtype,
        infercons,
        inferprop,
        inferinfo,
        false,
    )
}

/// The root node of the tree.
fn root(scip: &Scip) -> usize {
    treer!(scip).root
}

/// After a bound change on the root in presolving: an integer variable that
/// became binary changes its type (`SCIPchgVarType`); returns its
/// infeasibility.
fn upgrade_to_binary(scip: &mut Scip, var: VarId) -> ScipResult<bool> {
    if scip.vars[var].vartype == VarType::Integer && var::var_is_binary(scip, var) {
        return chg_var_type(scip, var, VarType::Binary);
    }
    Ok(false)
}

/// `tightenBounds`: tightens the global bounds of a variable becoming
/// integral; returns infeasibility.
fn tighten_bounds(scip: &mut Scip, var: VarId, integral: bool) -> ScipResult<bool> {
    if !var::var_is_integral(scip, var) && integral {
        let lb = scip.vars[var].glbdom.lb;
        if !scip.set.is_feas_integral(lb)
            || (!scip.set.is_integral(lb) && lb < scip.set.feas_ceil(lb))
            || (!scip.set.is_eq(lb, scip.set.feas_ceil(lb)) && lb < scip.set.feas_ceil(lb))
        {
            let (infeasible, _) = tighten_var_lb_global(scip, var, scip.set.feas_ceil(lb), true)?;
            if infeasible {
                return Ok(true);
            }
        }

        let ub = scip.vars[var].glbdom.ub;
        if !scip.set.is_feas_integral(ub) || (!scip.set.is_integral(ub) && ub > scip.set.feas_floor(ub)) {
            let (infeasible, _) = tighten_var_ub_global(scip, var, scip.set.feas_floor(ub), true)?;
            if infeasible {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// `SCIPchgVarType`: changes type of variable in the problem; returns
/// whether infeasibility was detected while tightening the bounds.
pub fn chg_var_type(scip: &mut Scip, mut var: VarId, vartype: VarType) -> ScipResult<bool> {
    if var::var_is_negated(scip, var) {
        var = scip.vars[var].negatedvar;
    }

    let infeasible;
    match scip.set.stage {
        Stage::Problem => {
            infeasible = tighten_bounds(scip, var, var::var_is_integral(scip, var))?;
            if scip.vars[var].probindex >= 0 {
                crate::prob::prob_chg_var_type(scip, ORIGPROB, var, vartype, true)?;
            } else {
                var::var_chg_type(scip, var, vartype)?;
            }
        }
        Stage::Presolving => {
            if !var::var_is_transformed(scip, var) {
                let transvar = api::get_transformed_var(scip, var)?;
                return chg_var_type(scip, transvar, vartype);
            }

            infeasible = tighten_bounds(scip, var, var::var_is_integral(scip, var))?;
            if scip.vars[var].probindex >= 0 {
                crate::prob::prob_chg_var_type(scip, TRANSPROB, var, vartype, true)?;
            } else {
                var::var_chg_type(scip, var, vartype)?;
            }
        }
        _ => return Err(Retcode::InvalidCall),
    }
    Ok(infeasible)
}

/// `SCIPchgVarImplType`: changes implied integral type of variable in the
/// problem; returns whether infeasibility was detected.
pub fn chg_var_impl_type(scip: &mut Scip, mut var: VarId, impltype: ImplintType) -> ScipResult<bool> {
    if var::var_is_negated(scip, var) {
        var = scip.vars[var].negatedvar;
    }

    let infeasible;
    match scip.set.stage {
        Stage::Problem => {
            infeasible = tighten_bounds(scip, var, impltype != ImplintType::None)?;
            if scip.vars[var].probindex >= 0 {
                crate::prob::prob_chg_var_impl_type(scip, ORIGPROB, var, impltype, true)?;
            } else {
                var::var_chg_impl_type(scip, var, impltype)?;
            }
        }
        Stage::Presolving => {
            if !var::var_is_transformed(scip, var) {
                let transvar = api::get_transformed_var(scip, var)?;
                return chg_var_impl_type(scip, transvar, impltype);
            }

            infeasible = tighten_bounds(scip, var, impltype != ImplintType::None)?;
            if scip.vars[var].probindex >= 0 {
                crate::prob::prob_chg_var_impl_type(scip, TRANSPROB, var, impltype, true)?;
            } else {
                var::var_chg_impl_type(scip, var, impltype)?;
            }
        }
        _ => return Err(Retcode::InvalidCall),
    }
    Ok(infeasible)
}

/// `SCIPadjustedVarLb`: returns the adjusted (i.e. rounded, if the given
/// variable is of integral type) lower bound value.
pub fn adjusted_var_lb(scip: &Scip, var: VarId, mut lb: f64) -> f64 {
    var::var_adjust_lb(scip, var, &mut lb);
    lb
}

/// `SCIPadjustedVarUb`: returns the adjusted (i.e. rounded, if the given
/// variable is of integral type) upper bound value.
pub fn adjusted_var_ub(scip: &Scip, var: VarId, mut ub: f64) -> f64 {
    var::var_adjust_ub(scip, var, &mut ub);
    ub
}

/// `SCIPchgVarLb`: changes variable's lower bound in current problem.
pub fn chg_var_lb(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    var::var_adjust_lb(scip, var, &mut newbound);

    // Ignore tightenings of lower bounds to +infinity during solving process.
    if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
        return Ok(());
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_lb_original(scip, var, newbound)?;
            var::var_chg_lb_global(scip, var, newbound)?;
            var::var_chg_lb_local(scip, var, newbound)?;
        }
        Stage::Transforming | Stage::Presolved => {
            var::var_chg_lb_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
            upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, tree_get_current_node(scip), var, newbound, BoundType::Lower)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }
    Ok(())
}

/// `SCIPchgVarUb`: changes variable's upper bound in current problem.
pub fn chg_var_ub(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    var::var_adjust_ub(scip, var, &mut newbound);

    // Ignore tightenings of upper bounds to -infinity during solving process.
    if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
        return Ok(());
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_ub_original(scip, var, newbound)?;
            var::var_chg_ub_global(scip, var, newbound)?;
            var::var_chg_ub_local(scip, var, newbound)?;
        }
        Stage::Transforming | Stage::Presolved => {
            var::var_chg_ub_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
            upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, tree_get_current_node(scip), var, newbound, BoundType::Upper)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }
    Ok(())
}

/// `SCIPchgVarLbNode`: changes lower bound of variable in the given node;
/// `NONE` for the current node.
pub fn chg_var_lb_node(scip: &mut Scip, node: usize, var: VarId, mut newbound: f64) -> ScipResult {
    if node == NONE {
        chg_var_lb(scip, var, newbound)?;
    } else {
        var::var_adjust_lb(scip, var, &mut newbound);

        // Ignore tightenings of lower bounds to +infinity during solving
        // process.
        if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
            return Ok(());
        }

        add_boundchg(scip, node, var, newbound, BoundType::Lower)?;
    }
    Ok(())
}

/// `SCIPchgVarUbNode`: changes upper bound of variable in the given node;
/// `NONE` for the current node.
pub fn chg_var_ub_node(scip: &mut Scip, node: usize, var: VarId, mut newbound: f64) -> ScipResult {
    if node == NONE {
        chg_var_ub(scip, var, newbound)?;
    } else {
        var::var_adjust_ub(scip, var, &mut newbound);

        // Ignore tightenings of upper bounds to -infinity during solving
        // process.
        if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
            return Ok(());
        }

        add_boundchg(scip, node, var, newbound, BoundType::Upper)?;
    }
    Ok(())
}

/// `SCIPchgVarLbGlobal`: changes global lower bound of variable.
pub fn chg_var_lb_global(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    var::var_adjust_lb(scip, var, &mut newbound);

    // Ignore tightenings of lower bounds to +infinity during solving process.
    if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
        return Ok(());
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_lb_global(scip, var, newbound)?;
            var::var_chg_lb_local(scip, var, newbound)?;
            var::var_chg_lb_original(scip, var, newbound)?;
        }
        Stage::Transforming | Stage::Transformed => {
            var::var_chg_lb_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
            upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }
    Ok(())
}

/// `SCIPchgVarUbGlobal`: changes global upper bound of variable.
pub fn chg_var_ub_global(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    var::var_adjust_ub(scip, var, &mut newbound);

    // Ignore tightenings of upper bounds to -infinity during solving process.
    if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
        return Ok(());
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_ub_global(scip, var, newbound)?;
            var::var_chg_ub_local(scip, var, newbound)?;
            var::var_chg_ub_original(scip, var, newbound)?;
        }
        Stage::Transforming | Stage::Transformed => {
            var::var_chg_ub_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
            upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }
    Ok(())
}

/// `SCIPchgVarLbLazy`: changes lazy lower bound of the variable, this is
/// only possible if the variable is not in the LP yet.
pub fn chg_var_lb_lazy(scip: &mut Scip, var: VarId, lazylb: f64) -> ScipResult {
    if scip.set.is_gt(lazylb, scip.vars[var].glbdom.lb) {
        chg_var_lb_global(scip, var, lazylb)?;
    }
    var::var_chg_lb_lazy(scip, var, lazylb)
}

/// `SCIPchgVarUbLazy`: changes lazy upper bound of the variable, this is
/// only possible if the variable is not in the LP yet.
pub fn chg_var_ub_lazy(scip: &mut Scip, var: VarId, lazyub: f64) -> ScipResult {
    if scip.set.is_lt(lazyub, scip.vars[var].glbdom.ub) {
        chg_var_ub_global(scip, var, lazyub)?;
    }
    var::var_chg_ub_lazy(scip, var, lazyub)
}

/// `SCIPtightenVarLb`: changes lower bound of variable in preprocessing or
/// in the current node, if the new bound is tighter (w.r.t. bound
/// strengthening epsilon) than the current bound; returns `(infeasible,
/// tightened)`.
pub fn tighten_var_lb(scip: &mut Scip, var: VarId, mut newbound: f64, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_lb(scip, var, &mut newbound);

    // Ignore tightenings of lower bounds to +infinity during solving process.
    if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
        return Ok((false, false));
    }

    // Get current bounds.
    let lb = compute_var_lb_local(scip, var);
    let ub = compute_var_ub_local(scip, var);

    if scip.set.is_feas_gt(newbound, ub) {
        return Ok((true, false));
    }
    newbound = crate::def::min(newbound, ub);

    if (force && scip.set.is_le(newbound, lb)) || (!force && !scip.set.is_lb_better(newbound, lb, ub)) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_lb_global(scip, var, newbound)?;
            var::var_chg_lb_local(scip, var, newbound)?;
            var::var_chg_lb_original(scip, var, newbound)?;
        }
        Stage::Transformed => {
            var::var_chg_lb_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, tree_get_current_node(scip), var, newbound, BoundType::Lower)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    // Check whether the lower bound improved.
    let tightened = lb < compute_var_lb_local(scip, var);
    Ok((infeasible, tightened))
}

/// `SCIPtightenVarUb`: changes upper bound of variable in preprocessing or
/// in the current node, if the new bound is tighter (w.r.t. bound
/// strengthening epsilon) than the current bound; returns `(infeasible,
/// tightened)`.
pub fn tighten_var_ub(scip: &mut Scip, var: VarId, mut newbound: f64, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_ub(scip, var, &mut newbound);

    // Ignore tightenings of upper bounds to -infinity during solving process.
    if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
        return Ok((false, false));
    }

    // Get current bounds.
    let lb = compute_var_lb_local(scip, var);
    let ub = compute_var_ub_local(scip, var);

    if scip.set.is_feas_lt(newbound, lb) {
        return Ok((true, false));
    }
    newbound = crate::def::max(newbound, lb);

    if (force && scip.set.is_ge(newbound, ub)) || (!force && !scip.set.is_ub_better(newbound, lb, ub)) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_ub_global(scip, var, newbound)?;
            var::var_chg_ub_local(scip, var, newbound)?;
            var::var_chg_ub_original(scip, var, newbound)?;
        }
        Stage::Transformed => {
            var::var_chg_ub_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, tree_get_current_node(scip), var, newbound, BoundType::Upper)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    // Check whether the upper bound improved.
    let tightened = ub > compute_var_ub_local(scip, var);
    Ok((infeasible, tightened))
}

/// `SCIPinferVarFixCons`: fixes variable in preprocessing or in the current
/// node, if the new bound is tighter (w.r.t. bound strengthening epsilon)
/// than the current bound; the given inference constraint is stored, such
/// that the conflict analysis is able to find out the reason for the
/// deduction of the bound change; returns `(infeasible, tightened)`.
pub fn infer_var_fix_cons(scip: &mut Scip, var: VarId, fixedval: f64, infercons: ConsId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    if scip.set.stage == Stage::Presolving && tree_get_current_depth(scip) == 0 {
        let mut infeasible = false;
        let mut fixed = false;
        var::var_fix(scip, var, TRANSPROB, ORIGPROB, fixedval, &mut infeasible, &mut fixed)?;
        Ok((infeasible, fixed))
    } else {
        let (infeasible, lbtightened) = infer_var_lb_cons(scip, var, fixedval, infercons, inferinfo, force)?;
        if !infeasible {
            let (infeasible, tightened) = infer_var_ub_cons(scip, var, fixedval, infercons, inferinfo, force)?;
            Ok((infeasible, tightened | lbtightened))
        } else {
            Ok((infeasible, false))
        }
    }
}

/// The body of `SCIPinferVarLbCons`.
fn infer_var_lb(scip: &mut Scip, var: VarId, mut newbound: f64, infercons: ConsId, inferprop: PropId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_lb(scip, var, &mut newbound);

    // Get current bounds.
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    if scip.set.is_infinity(newbound) || scip.set.is_feas_gt(newbound, ub) {
        return Ok((true, false));
    }
    newbound = crate::def::min(newbound, ub);

    if (force && scip.set.is_le(newbound, lb)) || (!force && !scip.set.is_lb_better(newbound, lb, ub)) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_lb_global(scip, var, newbound)?;
            var::var_chg_lb_local(scip, var, newbound)?;
            var::var_chg_lb_original(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundinfer(scip, tree_get_current_node(scip), var, newbound, BoundType::Lower, infercons, inferprop, inferinfo)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    // Check whether the lower bound improved.
    let tightened = lb < compute_var_lb_local(scip, var);
    Ok((infeasible, tightened))
}

/// The body of `SCIPinferVarUbCons`.
fn infer_var_ub(scip: &mut Scip, var: VarId, mut newbound: f64, infercons: ConsId, inferprop: PropId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_ub(scip, var, &mut newbound);

    // Get current bounds.
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    if scip.set.is_infinity(-newbound) || scip.set.is_feas_lt(newbound, lb) {
        return Ok((true, false));
    }
    newbound = crate::def::max(newbound, lb);

    if (force && scip.set.is_ge(newbound, ub)) || (!force && !scip.set.is_ub_better(newbound, lb, ub)) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_ub_global(scip, var, newbound)?;
            var::var_chg_ub_local(scip, var, newbound)?;
            var::var_chg_ub_original(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundinfer(scip, tree_get_current_node(scip), var, newbound, BoundType::Upper, infercons, inferprop, inferinfo)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    // Check whether the upper bound improved.
    let tightened = ub > compute_var_ub_local(scip, var);
    Ok((infeasible, tightened))
}

/// `SCIPinferVarLbCons`: changes lower bound of variable in preprocessing
/// or in the current node, if the new bound is tighter (w.r.t. bound
/// strengthening epsilon) than the current bound; the given inference
/// constraint is stored, such that the conflict analysis is able to find
/// out the reason for the deduction of the bound change; returns
/// `(infeasible, tightened)`.
pub fn infer_var_lb_cons(scip: &mut Scip, var: VarId, newbound: f64, infercons: ConsId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    infer_var_lb(scip, var, newbound, infercons, NONE, inferinfo, force)
}

/// `SCIPinferVarUbCons`: the upper bound counterpart of
/// [`infer_var_lb_cons`].
pub fn infer_var_ub_cons(scip: &mut Scip, var: VarId, newbound: f64, infercons: ConsId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    infer_var_ub(scip, var, newbound, infercons, NONE, inferinfo, force)
}

/// `SCIPinferVarLbProp`: changes lower bound of variable in preprocessing
/// or in the current node, if the new bound is tighter (w.r.t. bound
/// strengthening epsilon) than the current bound; the given inference
/// propagator is stored, such that the conflict analysis is able to find
/// out the reason for the deduction of the bound change; returns
/// `(infeasible, tightened)`. Unlike the constraint version it ignores
/// tightenings to +infinity during solving, and it is a no-op for a new
/// bound not above the current one even when forced.
pub fn infer_var_lb_prop(scip: &mut Scip, var: VarId, mut newbound: f64, inferprop: PropId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_lb(scip, var, &mut newbound);

    if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
        return Ok((false, false));
    }

    // Get current bounds.
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    if scip.set.is_feas_gt(newbound, ub) {
        return Ok((true, false));
    }
    newbound = crate::def::min(newbound, ub);

    if (!force && !scip.set.is_lb_better(newbound, lb, ub)) || scip.set.is_le(newbound, lb) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_lb_global(scip, var, newbound)?;
            var::var_chg_lb_local(scip, var, newbound)?;
            var::var_chg_lb_original(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundinfer(scip, tree_get_current_node(scip), var, newbound, BoundType::Lower, NONE, inferprop, inferinfo)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    let tightened = lb < compute_var_lb_local(scip, var);
    Ok((infeasible, tightened))
}

/// `SCIPinferVarUbProp`: the upper bound counterpart of
/// [`infer_var_lb_prop`].
pub fn infer_var_ub_prop(scip: &mut Scip, var: VarId, mut newbound: f64, inferprop: PropId, inferinfo: i32, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_ub(scip, var, &mut newbound);

    if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
        return Ok((false, false));
    }

    // Get current bounds.
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    if scip.set.is_feas_lt(newbound, lb) {
        return Ok((true, false));
    }
    newbound = crate::def::max(newbound, lb);

    if (!force && !scip.set.is_ub_better(newbound, lb, ub)) || scip.set.is_ge(newbound, ub) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_ub_global(scip, var, newbound)?;
            var::var_chg_ub_local(scip, var, newbound)?;
            var::var_chg_ub_original(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundinfer(scip, tree_get_current_node(scip), var, newbound, BoundType::Upper, NONE, inferprop, inferinfo)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    let tightened = ub > compute_var_ub_local(scip, var);
    Ok((infeasible, tightened))
}

/// The shared body of `SCIPinferBinvarCons` and `SCIPinferBinvarProp`.
fn infer_binvar(scip: &mut Scip, var: VarId, fixedval: bool, infercons: ConsId, inferprop: PropId, inferinfo: i32) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    // Get current bounds.
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    // Check, if variable is already fixed.
    if lb > 0.5 || ub < 0.5 {
        return Ok((fixedval == (lb < 0.5), false));
    }

    // Apply the fixing.
    match scip.set.stage {
        Stage::Problem => {
            if fixedval {
                chg_var_lb(scip, var, 1.0)?;
            } else {
                chg_var_ub(scip, var, 0.0)?;
            }
        }
        Stage::Presolving if tree_get_current_depth(scip) == 0 => {
            let mut fixed = false;
            var::var_fix(scip, var, TRANSPROB, ORIGPROB, fixedval as i32 as f64, &mut infeasible, &mut fixed)?;
        }
        Stage::Presolving | Stage::Solving => {
            let node = tree_get_current_node(scip);
            if fixedval {
                add_boundinfer(scip, node, var, 1.0, BoundType::Lower, infercons, inferprop, inferinfo)?;
            } else {
                add_boundinfer(scip, node, var, 0.0, BoundType::Upper, infercons, inferprop, inferinfo)?;
            }
        }
        _ => return Err(Retcode::InvalidCall),
    }

    Ok((infeasible, true))
}

/// `SCIPinferBinvarCons`: depending on SCIP's stage, fixes binary variable
/// in the problem, in preprocessing, or in current node; the given
/// inference constraint is stored, such that the conflict analysis is able
/// to find out the reason for the deduction of the bound change; returns
/// `(infeasible, tightened)`.
pub fn infer_binvar_cons(scip: &mut Scip, var: VarId, fixedval: bool, infercons: ConsId, inferinfo: i32) -> ScipResult<(bool, bool)> {
    infer_binvar(scip, var, fixedval, infercons, NONE, inferinfo)
}

/// `SCIPtightenVarLbGlobal`: changes global lower bound of variable in
/// preprocessing or in the current node, if the new bound is tighter
/// (w.r.t. bound strengthening epsilon) than the current global bound;
/// returns `(infeasible, tightened)`.
pub fn tighten_var_lb_global(scip: &mut Scip, var: VarId, mut newbound: f64, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_lb(scip, var, &mut newbound);

    // Ignore tightenings of lower bounds to +infinity during solving process.
    if scip.set.is_infinity(newbound) && scip.set.stage == Stage::Solving {
        return Ok((false, false));
    }

    // Get current bounds.
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;

    if scip.set.is_feas_gt(newbound, ub) {
        return Ok((true, false));
    }
    newbound = crate::def::min(newbound, ub);

    // Bound changes of less than epsilon are ignored by SCIPvarChgLb or
    // raise an assert in SCIPnodeAddBoundinfer, so don't relax this check.
    if scip.set.is_eq(lb, newbound) || (!force && !scip.set.is_lb_better(newbound, lb, ub)) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_lb_global(scip, var, newbound)?;
            var::var_chg_lb_local(scip, var, newbound)?;
            var::var_chg_lb_original(scip, var, newbound)?;
        }
        Stage::Transforming => {
            var::var_chg_lb_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Lower)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    // Coverity: unreachable code.
    let tightened = lb < compute_var_lb_global(scip, var);
    Ok((infeasible, tightened))
}

/// `SCIPtightenVarUbGlobal`: the upper bound counterpart of
/// [`tighten_var_lb_global`].
pub fn tighten_var_ub_global(scip: &mut Scip, var: VarId, mut newbound: f64, force: bool) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;

    var::var_adjust_ub(scip, var, &mut newbound);

    // Ignore tightenings of upper bounds to -infinity during solving process.
    if scip.set.is_infinity(-newbound) && scip.set.stage == Stage::Solving {
        return Ok((false, false));
    }

    // Get current bounds.
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;

    if scip.set.is_feas_lt(newbound, lb) {
        return Ok((true, false));
    }
    newbound = crate::def::max(newbound, lb);

    // Bound changes of less than epsilon are ignored by SCIPvarChgUb or
    // raise an assert in SCIPnodeAddBoundinfer, so don't relax this check.
    if scip.set.is_eq(ub, newbound) || (!force && !scip.set.is_ub_better(newbound, lb, ub)) {
        return Ok((false, false));
    }

    match scip.set.stage {
        Stage::Problem => {
            var::var_chg_ub_global(scip, var, newbound)?;
            var::var_chg_ub_local(scip, var, newbound)?;
            var::var_chg_ub_original(scip, var, newbound)?;
        }
        Stage::Transforming => {
            var::var_chg_ub_global(scip, var, newbound)?;
        }
        Stage::Presolving if !api::in_probing(scip) => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
            infeasible = upgrade_to_binary(scip, var)?;
        }
        Stage::Presolving | Stage::Solving => {
            add_boundchg(scip, root(scip), var, newbound, BoundType::Upper)?;
        }
        _ => return Err(Retcode::InvalidCall),
    }

    let tightened = ub > compute_var_ub_global(scip, var);
    Ok((infeasible, tightened))
}

/// `SCIPcomputeVarLbGlobal`: for a multi-aggregated variable, returns the
/// global lower bound computed by adding the global bounds from all
/// aggregation variables; otherwise the global lower bound.
pub fn compute_var_lb_global(scip: &Scip, var: VarId) -> f64 {
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        var::var_get_multaggr_lb_global(scip, var)
    } else {
        scip.vars[var].glbdom.lb
    }
}

/// `SCIPcomputeVarUbGlobal`.
pub fn compute_var_ub_global(scip: &Scip, var: VarId) -> f64 {
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        var::var_get_multaggr_ub_global(scip, var)
    } else {
        scip.vars[var].glbdom.ub
    }
}

/// `SCIPcomputeVarLbLocal`.
pub fn compute_var_lb_local(scip: &Scip, var: VarId) -> f64 {
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        var::var_get_multaggr_lb_local(scip, var)
    } else {
        scip.vars[var].locdom.lb
    }
}

/// `SCIPcomputeVarUbLocal`.
pub fn compute_var_ub_local(scip: &Scip, var: VarId) -> f64 {
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        var::var_get_multaggr_ub_local(scip, var)
    } else {
        scip.vars[var].locdom.ub
    }
}

/// `SCIPfixVar`: in problem creation and solving stage, both bounds of the
/// variable are set to the given value; in presolving stage, the variable
/// is converted into a fixed variable, and bounds are changed respectively;
/// conversion into a fixed variable changes the vars array returned from
/// SCIPgetVars() and SCIPgetVarsData(); returns `(infeasible, fixed)`.
pub fn fix_var(scip: &mut Scip, var: VarId, fixedval: f64) -> ScipResult<(bool, bool)> {
    let mut infeasible = false;
    let mut fixed = false;

    // In the problem creation stage, modify the bounds as requested,
    // independently from the current bounds.
    if scip.set.stage != Stage::Problem {
        let x = &scip.vars[var];
        if x.varstatus == VarStatus::Fixed {
            return Ok((!scip.set.is_feas_eq(fixedval, x.locdom.lb), false));
        } else if (var::var_is_integral(scip, var) && !scip.set.is_feas_integral(fixedval))
            || scip.set.is_feas_lt(fixedval, x.locdom.lb)
            || scip.set.is_feas_gt(fixedval, x.locdom.ub)
        {
            return Ok((true, false));
        }
    }

    match scip.set.stage {
        Stage::Problem => {
            // In the problem creation stage, modify the bounds as requested,
            // independently from the current bounds.
            chg_var_lb(scip, var, fixedval)?;
            chg_var_ub(scip, var, fixedval)?;
            Ok((false, true))
        }
        Stage::Presolving if tree_get_current_depth(scip) == 0 => {
            var::var_fix(scip, var, TRANSPROB, ORIGPROB, fixedval, &mut infeasible, &mut fixed)?;
            Ok((infeasible, fixed))
        }
        Stage::Presolving | Stage::Solving => {
            if scip.set.is_feas_gt(fixedval, scip.vars[var].locdom.lb) {
                if scip.set.is_feas_gt(fixedval, scip.vars[var].locdom.ub) {
                    return Ok((true, fixed));
                } else {
                    chg_var_lb(scip, var, fixedval)?;
                    fixed = true;
                }
            }
            if scip.set.is_feas_lt(fixedval, scip.vars[var].locdom.ub) {
                if scip.set.is_feas_lt(fixedval, scip.vars[var].locdom.lb) {
                    return Ok((true, fixed));
                } else {
                    chg_var_ub(scip, var, fixedval)?;
                    fixed = true;
                }
            }
            Ok((false, fixed))
        }
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPaddVarImplication`: informs binary variable x about a globally
/// valid implication:  x == 0 or x == 1  ==>  y <= b  or  y >= b; the
/// implication must be non-redundant; returns `(infeasible, nbdchgs)`.
pub fn add_var_implication(scip: &mut Scip, var: VarId, varfixing: bool, implvar: VarId, impltype: BoundType, implbound: f64) -> ScipResult<(bool, i32)> {
    let mut infeasible = false;
    let mut nbdchgs = 0;

    if !var::var_is_binary(scip, var) {
        // Can't add implication for nonbinary variable.
        return Err(Retcode::InvalidData);
    }

    let implprobvar = var::var_get_probvar(scip, implvar);

    // Transform implication containing two binary variables to a clique; the
    // condition ensures that the active representative of implvar is binary.
    if var::var_is_binary(scip, implvar) && (var::var_is_active(scip, implvar) || (implprobvar != NONE && var::var_is_binary(scip, implprobvar))) {
        // Only add clique if implication is not redundant with respect to
        // global bounds of the implication variable.
        if (impltype == BoundType::Lower && scip.vars[implvar].glbdom.lb < 0.5) || (impltype == BoundType::Upper && scip.vars[implvar].glbdom.ub > 0.5) {
            let vars = [var, implvar];
            let vals = [varfixing, impltype == BoundType::Upper];
            return add_clique(scip, &vars, &vals, false);
        }

        return Ok((false, 0));
    }

    // The implication graph can only handle 'real' binary (SCIP_VARTYPE_BINARY)
    // variables, therefore we have to turn the implication into a variable
    // bound.
    if scip.vars[var].vartype != VarType::Binary {
        let lby = scip.vars[implvar].glbdom.lb;
        let uby = scip.vars[implvar].glbdom.ub;

        if varfixing {
            if impltype == BoundType::Lower {
                // We return if the lower bound is infinity.
                if scip.set.is_infinity(-lby) {
                    return Ok((false, 0));
                }

                var::var_add_vlb(scip, implvar, TRANSPROB, ORIGPROB, var, implbound - lby, lby, true, &mut infeasible, Some(&mut nbdchgs))?;
            } else {
                // We return if the upper bound is infinity.
                if scip.set.is_infinity(uby) {
                    return Ok((false, 0));
                }

                var::var_add_vub(scip, implvar, TRANSPROB, ORIGPROB, var, implbound - uby, uby, true, &mut infeasible, Some(&mut nbdchgs))?;
            }
        } else if impltype == BoundType::Lower {
            // We return if the lower bound is infinity.
            if scip.set.is_infinity(-lby) {
                return Ok((false, 0));
            }

            var::var_add_vlb(
                scip,
                implvar,
                TRANSPROB,
                ORIGPROB,
                var,
                lby - implbound,
                implbound,
                true,
                &mut infeasible,
                Some(&mut nbdchgs),
            )?;
        } else {
            // We return if the upper bound is infinity.
            if scip.set.is_infinity(uby) {
                return Ok((false, 0));
            }

            var::var_add_vub(
                scip,
                implvar,
                TRANSPROB,
                ORIGPROB,
                var,
                uby - implbound,
                implbound,
                true,
                &mut infeasible,
                Some(&mut nbdchgs),
            )?;
        }
    } else {
        var::var_add_implic(
            scip,
            var,
            TRANSPROB,
            ORIGPROB,
            varfixing,
            implvar,
            impltype,
            implbound,
            true,
            &mut infeasible,
            Some(&mut nbdchgs),
        )?;
    }

    Ok((infeasible, nbdchgs))
}

/// `SCIPaddClique`: adds a clique information to SCIP, stating that at most
/// one of the given binary variables can be set to 1; if a variable appears
/// twice in the same clique, the corresponding implications are performed;
/// `values` empty means all `true`; returns `(infeasible, nbdchgs)`.
pub fn add_clique(scip: &mut Scip, vars: &[VarId], values: &[bool], isequation: bool) -> ScipResult<(bool, i32)> {
    let mut infeasible = false;
    let mut nbdchgs = 0;

    if vars.len() > 1 {
        // Add the clique to the clique table.
        crate::implics::cliquetable_add(scip, TRANSPROB, ORIGPROB, vars, values, isequation, &mut infeasible, Some(&mut nbdchgs))?;
    }

    Ok((infeasible, nbdchgs))
}

/// `SCIPaggregateVars`: from a given equality a*x + b*y == c, aggregates
/// one of the variables and removes it from the set of active problem
/// variables; returns `(infeasible, redundant, aggregated)`.
#[allow(clippy::too_many_arguments)]
pub fn aggregate_vars(scip: &mut Scip, mut varx: VarId, mut vary: VarId, mut scalarx: f64, mut scalary: f64, mut rhs: f64) -> ScipResult<(bool, bool, bool)> {
    let mut infeasible = false;
    let redundant;
    let mut aggregated = false;

    if crate::tree::tree_probing(scip) {
        // Cannot aggregate variables during probing.
        return Err(Retcode::InvalidCall);
    }

    // Do not aggregate if aggregation is turned off.
    if scip.set.presol_donotaggr {
        return Ok((false, false, false));
    }

    // Get the corresponding equality in active problem variable space:
    // transform both expressions "a*x + 0" and "b*y + 0" into problem
    // variable space.
    let mut constantx = 0.0;
    let mut constanty = 0.0;
    var::var_get_probvar_sum(scip, &mut varx, &mut scalarx, &mut constantx)?;
    var::var_get_probvar_sum(scip, &mut vary, &mut scalary, &mut constanty)?;

    // We cannot aggregate multi-aggregated variables.
    if scip.vars[varx].varstatus == VarStatus::MultAggr || scip.vars[vary].varstatus == VarStatus::MultAggr {
        return Ok((false, false, false));
    }

    // Move the constant to the right hand side to acquire the form
    // "a'*x' + b'*y' == c'".
    rhs -= constantx + constanty;

    // If a scalar is zero, treat the variable as fixed-to-zero variable.
    if scip.set.is_zero(scalarx) {
        varx = NONE;
    }
    if scip.set.is_zero(scalary) {
        vary = NONE;
    }

    // Capture the special cases that less than two variables are left, due to
    // resolutions to a fixed variable or resolution of y to x.
    if varx == NONE && vary == NONE {
        // Both variables were resolved to fixed variables.
        infeasible = !scip.set.is_zero(rhs);
        redundant = true;
    } else if varx == NONE {
        // x was resolved to fixed variable: variable y can be fixed to
        // c'/b'.
        var::var_fix(scip, vary, TRANSPROB, ORIGPROB, rhs / scalary, &mut infeasible, &mut aggregated)?;
        redundant = true;
    } else if vary == NONE {
        // y was resolved to fixed variable: variable x can be fixed to
        // c'/a'.
        var::var_fix(scip, varx, TRANSPROB, ORIGPROB, rhs / scalarx, &mut infeasible, &mut aggregated)?;
        redundant = true;
    } else if varx == vary {
        // Both variables were resolved to the same active problem variable:
        // this variable can be fixed.
        scalarx += scalary;
        if scip.set.is_zero(scalarx) {
            // Left hand side of equality is zero: equality is potentially
            // infeasible.
            infeasible = !scip.set.is_zero(rhs);
        } else {
            // Sum of scalars is not zero: fix variable x' == y' to c'/(a'+b').
            var::var_fix(scip, varx, TRANSPROB, ORIGPROB, rhs / scalarx, &mut infeasible, &mut aggregated)?;
        }
        redundant = true;
    } else {
        // Both variables are different active problem variables, and both
        // scalars are non-zero: try to aggregate them.
        var::var_try_aggregate_vars(scip, TRANSPROB, ORIGPROB, varx, vary, scalarx, scalary, rhs, &mut infeasible, &mut aggregated)?;
        redundant = aggregated;
    }

    Ok((infeasible, redundant, aggregated))
}

/// `SCIPmultiaggregateVar`: converts variable into multi-aggregated
/// variable; returns `(infeasible, aggregated)`.
pub fn multiaggregate_var(scip: &mut Scip, var: VarId, aggvars: &[VarId], scalars: &[f64], constant: f64) -> ScipResult<(bool, bool)> {
    if crate::tree::tree_probing(scip) {
        // Cannot multi-aggregate variables during probing.
        return Err(Retcode::InvalidCall);
    }

    let mut infeasible = false;
    let mut aggregated = false;
    let aggvars = crate::cvec::CVec(aggvars.to_vec());
    let mut scalars = crate::cvec::CVec(scalars.to_vec());
    var::var_multiaggregate(
        scip,
        var,
        TRANSPROB,
        ORIGPROB,
        aggvars.0.len() as i32,
        &aggvars,
        &mut scalars,
        constant,
        &mut infeasible,
        &mut aggregated,
    )?;
    Ok((infeasible, aggregated))
}

/// `SCIPdoNotAggr`: returns whether aggregation of variables is not
/// allowed.
pub fn do_not_aggr(scip: &Scip) -> bool {
    scip.set.presol_donotaggr
}

/// `SCIPdoNotMultaggr`: returns whether multi-aggregation is disabled.
pub fn do_not_multaggr(scip: &Scip) -> bool {
    scip.set.presol_donotmultaggr
}

/// `SCIPdoNotAggrVar`: returns whether variable is not allowed to be
/// aggregated.
pub fn do_not_aggr_var(scip: &Scip, var: VarId) -> bool {
    scip.set.presol_donotaggr || var::var_do_not_aggr(scip, var)
}

/// `SCIPdoNotMultaggrVar`: returns whether variable is not allowed to be
/// multi-aggregated.
pub fn do_not_multaggr_var(scip: &Scip, var: VarId) -> bool {
    scip.set.presol_donotmultaggr || var::var_do_not_multaggr(scip, var)
}

/// `SCIPisVarAggrCoefAcceptable`: returns whether the given scalar is
/// acceptable as aggregation coefficient for the variable.
pub fn is_var_aggr_coef_acceptable(scip: &Scip, var: VarId, scalar: f64) -> bool {
    var::var_is_aggr_coef_acceptable(scip, var, scalar)
}

/// `SCIPallowStrongDualReds`: returns whether strong dual reductions are
/// allowed during propagation and presolving.
pub fn allow_strong_dual_reds(scip: &Scip) -> bool {
    !scip.set.reopt_enable && scip.set.misc_allowstrongdualreds
}

/// `SCIPallowWeakDualReds`: returns whether weak dual reductions are
/// allowed during propagation and presolving.
pub fn allow_weak_dual_reds(scip: &Scip) -> bool {
    !scip.set.reopt_enable && scip.set.misc_allowweakdualreds
}

/// `SCIPmarkDoNotAggrVar`: marks the variable that it must not be
/// aggregated.
pub fn mark_do_not_aggr_var(scip: &mut Scip, var: VarId) -> ScipResult {
    var::var_mark_do_not_aggr(scip, var)
}

/// `SCIPmarkDoNotMultaggrVar`: marks the variable that it must not be
/// multi-aggregated.
pub fn mark_do_not_multaggr_var(scip: &mut Scip, var: VarId) -> ScipResult {
    var::var_mark_do_not_multaggr(scip, var)
}

/// `SCIPcreateVarImpl`: creates and captures problem variable with
/// optional implied integral type; variable callbacks and data are not
/// ported.
#[allow(clippy::too_many_arguments)]
pub fn create_var_impl(scip: &mut Scip, name: &str, lb: f64, ub: f64, obj: f64, vartype: VarType, impltype: ImplintType, initial: bool, removable: bool) -> ScipResult<VarId> {
    // Check whether the objective coefficient is finite.
    if scip.set.is_infinity(obj.abs()) {
        // Invalid objective function value: value is infinite.
        return Err(Retcode::InvalidData);
    }

    if vartype == VarType::ImplInt {
        // Using SCIP_VARTYPE_IMPLINT deprecated, define impltype instead.
        return Err(Retcode::InvalidData);
    }

    match scip.set.stage {
        Stage::Problem => var::var_create_original(scip, Some(name), lb, ub, obj, vartype, impltype, initial, removable),
        Stage::Transforming | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::Solving => {
            var::var_create_transformed(scip, Some(name), lb, ub, obj, vartype, impltype, initial, removable)
        }
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPaddVar` (`scip_prob.c`): adds variable to the problem.
pub fn add_var(scip: &mut Scip, var: VarId) -> ScipResult {
    // Avoid inserting the same variable twice.
    if scip.vars[var].probindex != -1 {
        return Ok(());
    }

    // Insert the negation variable x instead of the negated variable x' in
    // x' = offset - x.
    if scip.vars[var].varstatus == VarStatus::Negated {
        let negationvar = scip.vars[var].negatedvar;
        return add_var(scip, negationvar);
    }

    match scip.set.stage {
        Stage::Problem => {
            if scip.vars[var].varstatus != VarStatus::Original {
                // Cannot add transformed variables to original problem.
                return Err(Retcode::InvalidData);
            }
            crate::prob::prob_add_var(scip, ORIGPROB, var)
        }
        Stage::Transforming | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::Solving => {
            let status = scip.vars[var].varstatus;
            if status == VarStatus::Original {
                // Cannot add original variables to transformed problem.
                return Err(Retcode::InvalidData);
            } else if status != VarStatus::Loose && status != VarStatus::Column {
                // Cannot add fixed or aggregated variables to transformed problem.
                return Err(Retcode::InvalidData);
            }
            crate::prob::prob_add_var(scip, TRANSPROB, var)
        }
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPchgVarObj`: changes variable's objective value.
pub fn chg_var_obj(scip: &mut Scip, var: VarId, newobj: f64) -> ScipResult {
    if scip.set.is_infinity(newobj.abs()) {
        // Invalid objective value: objective value is infinite.
        return Err(Retcode::InvalidData);
    }

    match scip.set.stage {
        Stage::Problem => var::var_chg_obj(scip, var, ORIGPROB, newobj),
        Stage::Transformed | Stage::Transforming | Stage::Presolving | Stage::Presolved => var::var_chg_obj(scip, var, TRANSPROB, newobj),
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPgetNCliques`: gets the number of cliques in the clique table.
pub fn get_n_cliques(scip: &Scip) -> i32 {
    scip.cliquetable.as_ref().unwrap().ncliques
}

/// `SCIPgetCliques` (`scip_var.c`): gets the array of cliques in the
/// clique table.
pub fn get_cliques(scip: &Scip) -> &[usize] {
    let t = scip.cliquetable.as_ref().unwrap();
    &t.cliques.0[..t.ncliques as usize]
}

/// The cliques `var == value` is in (`SCIPvarGetNCliques` and
/// `SCIPvarGetCliques`).
fn var_cliques(scip: &Scip, var: VarId, value: bool) -> &[usize] {
    match scip.vars[var].cliquelist.as_ref() {
        Some(l) => &l.cliques[value as usize].0[..l.ncliques[value as usize] as usize],
        None => &[],
    }
}

/// Maximal number of cliques of variable for addLargestCliquePart() to
/// search a suitable clique.
const MAXNUMEARCHCLIQUE: usize = 10000;

/// `addLargestCliquePart`: greedily adds the variables of the clique of
/// `var` with the smallest next index to the clique partition part `p`.
#[allow(clippy::too_many_arguments)]
fn add_largest_clique_part(
    scip: &Scip,
    var: VarId,
    value: bool,
    varidx: i32,
    idx: &[i32],
    values: &[bool],
    p: i32,
    nvars: i32,
    cliquepartition: &mut [i32],
    ncliqueparts: &mut [i32],
) {
    cliquepartition[varidx as usize] = p;
    ncliqueparts[p as usize] = 1;

    if var::var_is_active(scip, var) {
        let varcliques = var_cliques(scip, var, value);
        let nvarcliques = varcliques.len();

        if nvarcliques > 0 && nvarcliques < MAXNUMEARCHCLIQUE {
            let mut selectedidx: i32 = -1;
            let mut selectedsmallestidx = nvars + 1;

            // Loop through all cliques of the variable.
            for (l, &clique) in varcliques.iter().enumerate() {
                let q = &scip.cliques[clique];
                let mut smallestidx = nvars + 1;

                for k in 0..q.nvars as usize {
                    let othervar = q.vars[k];
                    if var::var_is_active(scip, othervar) {
                        let probidx = scip.vars[othervar].probindex;
                        let j = idx[probidx as usize];

                        if j >= 0 && q.values[k] == values[j as usize] && cliquepartition[j as usize] < 0 && j > varidx && j < smallestidx {
                            smallestidx = j;
                            if smallestidx == varidx + 1 {
                                break;
                            }
                        }
                    }
                }

                if smallestidx < selectedsmallestidx {
                    selectedidx = l as i32;
                    selectedsmallestidx = smallestidx;
                    if smallestidx == varidx + 1 {
                        break;
                    }
                }
            }

            if selectedidx >= 0 {
                let q = &scip.cliques[varcliques[selectedidx as usize]];

                for k in 0..q.nvars as usize {
                    let othervar = q.vars[k];
                    if var::var_is_active(scip, othervar) {
                        let probidx = scip.vars[othervar].probindex;
                        let j = idx[probidx as usize];

                        if j >= 0 && q.values[k] == values[j as usize] && cliquepartition[j as usize] < 0 {
                            cliquepartition[j as usize] = p;
                            ncliqueparts[p as usize] += 1;
                        }
                    }
                }
            }
        }
    }
}

/// `calcCliquePartitionGreedy`: calculates a partition of the given set of
/// binary variables into cliques; returns the number of cliques.
fn calc_clique_partition_greedy(scip: &mut Scip, vars: &[VarId], values: &[bool], probtoidxmap: &mut CVec<i32>, probtoidxmapsize: &mut i32, cliquepartition: &mut [i32]) -> i32 {
    let nvars = vars.len();
    let mut marked = vec![0i32; nvars];
    let mut nneigh = vec![0i32; nvars];
    let mut ncliqueparts = vec![0i32; nvars];

    let nbinvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip);

    if *probtoidxmapsize < nbinvars {
        probtoidxmap.0.resize(nbinvars as usize, 0);
        *probtoidxmapsize = nbinvars;
        for i in 0..nbinvars as usize {
            probtoidxmap[i] = -1;
        }
    }
    let idx = &mut probtoidxmap.0;

    // Clean up the array of variable indices.
    for i in 0..nvars {
        cliquepartition[i] = -1;

        let probidx = scip.vars[vars[i]].probindex;

        // Variable index has not been set yet.
        if probidx >= 0 && idx[probidx as usize] < 0 {
            idx[probidx as usize] = i as i32;
        }
    }

    // Add the first variable as the first clique part.
    add_largest_clique_part(scip, vars[0], values[0], 0, idx, values, 0, nvars as i32, cliquepartition, &mut ncliqueparts);
    let mut ncliques: i32 = 1;

    // Loop over all variables.
    for i in 1..nvars {
        // Variable already assigned to a clique part.
        if cliquepartition[i] >= 0 {
            continue;
        }

        let var = vars[i];
        let value = values[i];
        let varcliques = var_cliques(scip, var, value);
        let nvarcliques = varcliques.len();

        // If the variable is not active, or has no cliques, it becomes a
        // singleton part.
        if nvarcliques > 0 && var::var_is_active(scip, var) {
            let mut foundpart = false;

            for nneigh_p in &mut nneigh[..ncliques as usize] {
                *nneigh_p = 0;
            }

            // Loop over cliques of the variable.
            let mut l = 0;
            while l < nvarcliques && !foundpart {
                let q = &scip.cliques[varcliques[l]];

                for k in 0..q.nvars as usize {
                    let othervar = q.vars[k];

                    // Ignore inactive variables and the variable itself.
                    if !var::var_is_active(scip, othervar) || othervar == var {
                        continue;
                    }

                    let probidx = scip.vars[othervar].probindex;
                    let j = idx[probidx as usize];

                    // Check whether the other variable is in the set.
                    if j >= 0 {
                        let p = cliquepartition[j as usize];

                        // Count the other variable as neighbor if it belongs to a
                        // clique part and has the same clique value.
                        if p >= 0 && q.values[k] == values[j as usize] && marked[j as usize] != i as i32 {
                            nneigh[p as usize] += 1;
                            marked[j as usize] = i as i32;

                            // Check whether all variables of the part are neighbors.
                            if nneigh[p as usize] == ncliqueparts[p as usize] {
                                cliquepartition[i] = p;
                                ncliqueparts[p as usize] += 1;
                                foundpart = true;
                                break;
                            }
                        }
                    }
                }
                l += 1;
            }

            // Add variable to a new clique part.
            if !foundpart {
                add_largest_clique_part(scip, vars[i], values[i], i as i32, idx, values, ncliques, nvars as i32, cliquepartition, &mut ncliqueparts);
                ncliques += 1;
            }
        } else {
            ncliqueparts[ncliques as usize] = 1;
            cliquepartition[i] = ncliques;
            ncliques += 1;
        }
    }

    // Reset the index array.
    for i in 0..nvars {
        let probidx = scip.vars[vars[i]].probindex;
        if probidx >= 0 {
            idx[probidx as usize] = -1;
        }
    }

    ncliques
}

/// `SCIPcalcCliquePartition`: calculates a partition of the given set of
/// binary variables into cliques; returns the number of cliques.
pub fn calc_clique_partition(scip: &mut Scip, vars: &[VarId], probtoidxmap: &mut CVec<i32>, probtoidxmapsize: &mut i32, cliquepartition: &mut [i32]) -> ScipResult<i32> {
    let nvars = vars.len();

    // Check for trivial case.
    if nvars == 0 {
        return Ok(0);
    }

    // If we have no cliques, every variable is its own part.
    if get_n_cliques(scip) == 0 {
        for (i, part) in cliquepartition[..nvars].iter_mut().enumerate() {
            *part = i as i32;
        }
        return Ok(nvars as i32);
    }

    // Get active (or negated active) variables.
    let mut tmpvars = vars.to_vec();
    let mut tmpvalues = vec![true; nvars];
    var::vars_get_probvar_binary(scip, &mut tmpvars, &mut tmpvalues)?;

    Ok(calc_clique_partition_greedy(scip, &tmpvars, &tmpvalues, probtoidxmap, probtoidxmapsize, cliquepartition))
}

/// `SCIPgetNegatedVar`: gets negated variable x' = lb + ub - x of
/// variable x; negated variable is created, if not yet existing.
pub fn get_negated_var(scip: &mut Scip, var: VarId) -> ScipResult<VarId> {
    let mut negvar = NONE;
    var::var_negate(scip, var, &mut negvar)?;
    Ok(negvar)
}

/// `SCIPcalcNegatedCliquePartition`: calculates a partition of the given set
/// of binary variables into negated cliques; returns the number of cliques.
pub fn calc_negated_clique_partition(scip: &mut Scip, vars: &[VarId], probtoidxmap: &mut CVec<i32>, probtoidxmapsize: &mut i32, cliquepartition: &mut [i32]) -> ScipResult<i32> {
    // Check for trivial case.
    if vars.is_empty() {
        return Ok(0);
    }

    // Get negated variables.
    let mut negvars = vec![NONE; vars.len()];
    for v in (0..vars.len()).rev() {
        negvars[v] = get_negated_var(scip, vars[v])?;
    }

    // Calculate cliques on negated variables, which are "negated" cliques on
    // normal variables array.
    calc_clique_partition(scip, &negvars, probtoidxmap, probtoidxmapsize, cliquepartition)
}

/// `SCIPgetBinvarRepresentative`: gets a binary variable that is equal to
/// the given binary variable, and that is either active, fixed, or
/// multi-aggregated, or the negated variable of an active, fixed, or
/// multi-aggregated variable; returns `(repvar, negated)`.
pub fn get_binvar_representative(scip: &mut Scip, var: VarId) -> ScipResult<(VarId, bool)> {
    let mut repvar = var;
    let mut negated = false;

    // Get the active variable.
    var::var_get_probvar_binary(scip, &mut repvar, &mut negated)?;

    // Negate the variable if necessary.
    if negated {
        repvar = get_negated_var(scip, repvar)?;
    }

    Ok((repvar, negated))
}

/// `SCIPgetBinvarRepresentatives`: gets binary variables that are equal to
/// the given binary variables, see [`get_binvar_representative`].
pub fn get_binvar_representatives(scip: &mut Scip, vars: &[VarId], repvars: &mut [VarId], negated: &mut [bool]) -> ScipResult {
    if vars.is_empty() {
        return Ok(());
    }

    // Copy variables into representative array.
    repvars[..vars.len()].copy_from_slice(vars);
    for n in negated[..vars.len()].iter_mut() {
        *n = false;
    }

    // Get the active variables.
    var::vars_get_probvar_binary(scip, &mut repvars[..vars.len()], &mut negated[..vars.len()])?;

    // Negate the variables if necessary.
    for v in (0..vars.len()).rev() {
        if negated[v] {
            repvars[v] = get_negated_var(scip, repvars[v])?;
        }
    }

    Ok(())
}

/// `SCIPgetNegatedVars`: gets negated variables x' = lb + ub - x of
/// variables x; negated variables are created, if not yet existing.
pub fn get_negated_vars(scip: &mut Scip, vars: &[VarId], negvars: &mut [VarId]) -> ScipResult {
    for v in 0..vars.len() {
        negvars[v] = get_negated_var(scip, vars[v])?;
    }
    Ok(())
}

/// `SCIPcleanupCliques`: removes fixings, aggregations, and redundant
/// entries from the clique table; returns whether cleanup detected
/// infeasibility.
pub fn cleanup_cliques(scip: &mut Scip) -> ScipResult<bool> {
    let mut nlocalbdchgs = 0;
    let mut globalinfeasibility = false;

    crate::implics::cliquetable_cleanup(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, &mut nlocalbdchgs, &mut globalinfeasibility)?;

    if globalinfeasibility {
        scip.stat.status = crate::types::Status::Infeasible;
    }

    Ok(globalinfeasibility)
}

/// `SCIPaddVarVlb` (`scip_var.c`): informs variable x about a globally valid
/// variable lower bound x >= b*z + d with integer variable z; if x is not
/// continuous, the corresponding bound on z is added too. Returns
/// `(infeasible, nbdchgs)`.
pub fn add_var_vlb(scip: &mut Scip, var: VarId, vlbvar: VarId, vlbcoef: f64, vlbconstant: f64) -> ScipResult<(bool, i32)> {
    let mut infeasible = false;
    let mut nlocalbdchgs = 0;

    var::var_add_vlb(scip, var, TRANSPROB, ORIGPROB, vlbvar, vlbcoef, vlbconstant, true, &mut infeasible, Some(&mut nlocalbdchgs))?;

    let mut nbdchgs = nlocalbdchgs;

    // If x is not continuous we add a variable bound for z; do not add it if
    // cofficient would be too small or we already detected infeasibility.
    if !infeasible && var::var_is_integral(scip, var) && !scip.set.is_zero(1.0 / vlbcoef) {
        if vlbcoef > 0.0 {
            // If b > 0, we have a variable upper bound: x >= b*z + d  =>  z <=
            // (x-d)/b.
            var::var_add_vub(
                scip,
                vlbvar,
                TRANSPROB,
                ORIGPROB,
                var,
                1.0 / vlbcoef,
                -vlbconstant / vlbcoef,
                true,
                &mut infeasible,
                Some(&mut nlocalbdchgs),
            )?;
        } else {
            // If b < 0, we have a variable lower bound: x >= b*z + d  =>  z >=
            // (x-d)/b.
            var::var_add_vlb(
                scip,
                vlbvar,
                TRANSPROB,
                ORIGPROB,
                var,
                1.0 / vlbcoef,
                -vlbconstant / vlbcoef,
                true,
                &mut infeasible,
                Some(&mut nlocalbdchgs),
            )?;
        }
        nbdchgs += nlocalbdchgs;
    }

    Ok((infeasible, nbdchgs))
}

/// `SCIPaddVarVub` (`scip_var.c`): informs variable x about a globally valid
/// variable upper bound x <= b*z + d with integer variable z; if x is not
/// continuous, the corresponding bound on z is added too. Returns
/// `(infeasible, nbdchgs)`.
pub fn add_var_vub(scip: &mut Scip, var: VarId, vubvar: VarId, vubcoef: f64, vubconstant: f64) -> ScipResult<(bool, i32)> {
    let mut infeasible = false;
    let mut nlocalbdchgs = 0;

    var::var_add_vub(scip, var, TRANSPROB, ORIGPROB, vubvar, vubcoef, vubconstant, true, &mut infeasible, Some(&mut nlocalbdchgs))?;

    let mut nbdchgs = nlocalbdchgs;

    // If x is not continuous we add a variable bound for z; do not add it if
    // cofficient would be too small or we already detected infeasibility.
    if !infeasible && var::var_is_integral(scip, var) && !scip.set.is_zero(1.0 / vubcoef) {
        if vubcoef > 0.0 {
            // If b > 0, we have a variable lower bound: x <= b*z + d  =>  z >=
            // (x-d)/b.
            var::var_add_vlb(
                scip,
                vubvar,
                TRANSPROB,
                ORIGPROB,
                var,
                1.0 / vubcoef,
                -vubconstant / vubcoef,
                true,
                &mut infeasible,
                Some(&mut nlocalbdchgs),
            )?;
        } else {
            // If b < 0, we have a variable upper bound: x <= b*z + d  =>  z <=
            // (x-d)/b.
            var::var_add_vub(
                scip,
                vubvar,
                TRANSPROB,
                ORIGPROB,
                var,
                1.0 / vubcoef,
                -vubconstant / vubcoef,
                true,
                &mut infeasible,
                Some(&mut nlocalbdchgs),
            )?;
        }
        nbdchgs += nlocalbdchgs;
    }

    Ok((infeasible, nbdchgs))
}

/// `SCIPgetVarConflictScore`: returns the variable's conflict score value.
pub fn get_var_conflict_score(scip: &Scip, var: VarId) -> f64 {
    let downscore = crate::var::var_get_vsids(scip, var, BranchDir::Downwards);
    let upscore = crate::var::var_get_vsids(scip, var, BranchDir::Upwards);

    crate::branch::branch_get_score(scip, var, downscore, upscore)
}

/// `SCIPgetVarConflictlengthScore`: returns the variable's conflict length
/// score.
pub fn get_var_conflictlength_score(scip: &Scip, var: VarId) -> f64 {
    let downscore = crate::var::var_get_avg_conflictlength(scip, var, BranchDir::Downwards);
    let upscore = crate::var::var_get_avg_conflictlength(scip, var, BranchDir::Upwards);

    crate::branch::branch_get_score(scip, var, downscore, upscore)
}

/// `SCIPgetVarAvgInferenceScore`: returns the variable's average inference
/// score value.
pub fn get_var_avg_inference_score(scip: &Scip, var: VarId) -> f64 {
    let inferdown = crate::var::var_get_avg_inferences(scip, var, BranchDir::Downwards);
    let inferup = crate::var::var_get_avg_inferences(scip, var, BranchDir::Upwards);

    crate::branch::branch_get_score(scip, var, inferdown, inferup)
}

/// `SCIPgetVarAvgCutoffScore`: returns the variable's average cutoff score
/// value.
pub fn get_var_avg_cutoff_score(scip: &Scip, var: VarId) -> f64 {
    let cutoffdown = crate::var::var_get_avg_cutoffs(scip, var, BranchDir::Downwards);
    let cutoffup = crate::var::var_get_avg_cutoffs(scip, var, BranchDir::Upwards);

    crate::branch::branch_get_score(scip, var, cutoffdown, cutoffup)
}

/// `SCIPgetVarPseudocostScore`: gets the variable's pseudo cost score
/// value for the given LP solution value.
pub fn get_var_pseudocost_score(scip: &Scip, var: VarId, solval: f64) -> f64 {
    let downsol = scip.set.feas_ceil(solval - 1.0);
    let upsol = scip.set.feas_floor(solval + 1.0);
    let pscostdown = crate::var::var_get_pseudocost(scip, var, downsol - solval);
    let pscostup = crate::var::var_get_pseudocost(scip, var, upsol - solval);

    crate::branch::branch_get_score(scip, var, pscostdown, pscostup)
}

/// `SCIPgetVarDPseudocostScore`: gets the variable's (possible) discounted
/// pseudo cost score value for the given LP solution value.
pub fn get_var_dpseudocost_score(scip: &Scip, var: VarId, solval: f64, discountfac: f64) -> f64 {
    let downsol = scip.set.feas_ceil(solval - 1.0);
    let upsol = scip.set.feas_floor(solval + 1.0);
    let mut pscostdown = crate::var::var_get_pseudocost(scip, var, downsol - solval) + discountfac * crate::var::var_get_anc_pseudocost(scip, var, downsol - solval);
    let mut pscostup = crate::var::var_get_pseudocost(scip, var, upsol - solval) + discountfac * crate::var::var_get_anc_pseudocost(scip, var, upsol - solval);
    pscostdown /= 1.0 + discountfac;
    pscostup /= 1.0 + discountfac;

    crate::branch::branch_get_score(scip, var, pscostdown, pscostup)
}

/// `SCIPupdateVarPseudocost`: updates the pseudo costs of the given
/// variable and the global pseudo costs after a change of
/// `solvaldelta` in the variable's solution value and resulting change of
/// `objdelta` in the LP's objective value.
pub fn update_var_pseudocost(scip: &mut Scip, var: VarId, solvaldelta: f64, objdelta: f64, weight: f64) -> ScipResult {
    // Differences infinity - eps should also be treated as infinity.
    if !scip.set.is_infinity(2.0 * objdelta) && (scip.set.branch_divingpscost || (!crate::lp::lp_diving(scip) && !crate::tree::tree_probing(scip))) {
        crate::var::var_update_pseudocost(scip, var, solvaldelta, objdelta, weight)?;
    }

    Ok(())
}

/// `SCIPupdateVarAncPseudocost`: updates the ancestral pseudo costs of the
/// given variable and the global ancestral pseudo costs.
pub fn update_var_anc_pseudocost(scip: &mut Scip, var: VarId, solvaldelta: f64, objdelta: f64, weight: f64) -> ScipResult {
    // Differences infinity - eps should also be treated as infinity.
    if !scip.set.is_infinity(2.0 * objdelta) && (scip.set.branch_divingpscost || (!crate::lp::lp_diving(scip) && !crate::tree::tree_probing(scip))) {
        crate::var::var_update_anc_pseudocost(scip, var, solvaldelta, objdelta, weight)?;
    }

    Ok(())
}
