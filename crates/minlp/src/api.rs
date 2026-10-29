//! SCIP's public interface (`scip_*.c`): the functions plugins and the
//! solver itself call on the whole instance. Filled in as the solving
//! process that uses them is ported.

use crate::{
    scip::{Scip, Stage, TRANSPRIMAL},
    types::Status,
};

/// `SCIPgetStatus` (`scip_general.c`): gets solution status.
pub fn get_status(scip: &Scip) -> Status {
    scip.stat.status
}

/// `SCIPgetLowerbound` (`scip_solvingstats.c`): gets global lower (dual)
/// bound in transformed problem.
pub fn get_lowerbound(scip: &Scip) -> f64 {
    if scip.set.stage <= Stage::InitSolve {
        -scip.set.infinity()
    } else if get_status(scip) == Status::InfOrUnbd || get_status(scip) == Status::Unbounded {
        // In case we could not prove whether the problem is unbounded or
        // infeasible, we want to terminate with lower bound = -inf instead of
        // lower bound = upper bound = +inf also in case we prove that the
        // problem is unbounded, it seems to make sense to return with lower
        // bound = -inf, since -infinity is the only valid lower bound.
        -scip.set.infinity()
    } else if get_status(scip) == Status::Infeasible {
        // SCIPtreeGetLowerbound() should return +inf in the case of
        // infeasibility, but when infeasibility is detected during
        // presolving this does not seem to be the case; hence, we treat this
        // case explicitly.
        scip.set.infinity()
    } else {
        // It may happen that the remaining tree is empty or all open nodes
        // have a lower bound above the cutoff bound, but have not yet been
        // cut off, e.g., when the user calls SCIPgetDualbound() in some event
        // handler; in this case, the global lower bound is given by the upper
        // bound value.
        let treelowerbound = crate::tree::tree_get_lowerbound(scip);
        let upperbound = scip.primals[TRANSPRIMAL].upperbound;
        if treelowerbound < upperbound { treelowerbound } else { upperbound }
    }
}

/// `SCIPcatchEvent` (`scip_event.c`): catches a global (not variable or
/// row dependent) event.
pub fn catch_event(
    scip: &mut Scip,
    eventtype: crate::event::EventType,
    eventhdlr: crate::types::EventhdlrId,
    eventdata: usize,
    filterpos: Option<&mut i32>,
) -> crate::retcode::ScipResult {
    use crate::event::{EVENTTYPE_ROWCHANGED, EVENTTYPE_VARCHANGED};
    if eventtype & (EVENTTYPE_VARCHANGED | EVENTTYPE_ROWCHANGED) != 0 {
        log::error!("SCIPcatchEvent does not support variable or row change events. Use SCIPcatchVarEvent or SCIPcatchRowEvent!");
        return Err(crate::retcode::Retcode::InvalidData);
    }
    crate::event::eventfilter_add(scip, crate::scip::GLOBALEVENTFILTER, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIPdropEvent` (`scip_event.c`): drops a global event (stops to track
/// event).
pub fn drop_event(scip: &mut Scip, eventtype: crate::event::EventType, eventhdlr: crate::types::EventhdlrId, eventdata: usize, filterpos: i32) -> crate::retcode::ScipResult {
    crate::event::eventfilter_del(scip, crate::scip::GLOBALEVENTFILTER, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIPgetVarLbAtIndex` and `SCIPgetVarUbAtIndex` (`scip_var.c`): returns
/// lower or upper bound of variable directly before or after the bound
/// change given by the bound change index was applied.
pub fn get_var_bd_at_index(scip: &Scip, var: crate::types::VarId, boundtype: crate::types::BoundType, bdchgidx: Option<&crate::var::BdChgIdx>, after: bool) -> f64 {
    use crate::types::{BoundType, VarStatus};
    let upper = boundtype == BoundType::Upper;
    let opposite = if upper { BoundType::Lower } else { BoundType::Upper };
    let v = &scip.vars[var];
    // Bound of an attached variable y with x = a*y + c.
    let scaled = |y: crate::types::VarId, a: f64, c: f64| -> f64 {
        if a > 0.0 {
            // a > 0 -> get the same bound of y.
            let b = get_var_bd_at_index(scip, y, boundtype, bdchgidx, after);
            if scip.set.is_infinity(-b) {
                -scip.set.infinity()
            } else if scip.set.is_infinity(b) {
                scip.set.infinity()
            } else {
                a * b + c
            }
        } else if a < 0.0 {
            // a < 0 -> get the opposite bound of y.
            let b = get_var_bd_at_index(scip, y, opposite, bdchgidx, after);
            if scip.set.is_infinity(-b) {
                scip.set.infinity()
            } else if scip.set.is_infinity(b) {
                -scip.set.infinity()
            } else {
                a * b + c
            }
        } else {
            panic!("scalar is zero in aggregation");
        }
    };
    match v.varstatus {
        VarStatus::Original => get_var_bd_at_index(scip, v.data.original.transvar, boundtype, bdchgidx, after),
        VarStatus::Column | VarStatus::Loose => match bdchgidx {
            None => {
                if upper {
                    v.locdom.ub
                } else {
                    v.locdom.lb
                }
            }
            Some(_) => match crate::var::var_get_bdchg_info(scip, var, boundtype, bdchgidx, after) {
                Some(pos) => crate::var::var_bdchginfo(scip, var, boundtype, pos).newbound,
                None => {
                    if upper {
                        v.glbdom.ub
                    } else {
                        v.glbdom.lb
                    }
                }
            },
        },
        VarStatus::Fixed => {
            if upper {
                v.glbdom.ub
            } else {
                v.glbdom.lb
            }
        }
        // x = a*y + c  ->  y = (x-c)/a
        VarStatus::Aggregated => scaled(v.data.aggregate.var, v.data.aggregate.scalar, v.data.aggregate.constant),
        VarStatus::MultAggr => {
            // Handle multi-aggregated variables depending on one variable only
            // (possibly caused by SCIPvarFlattenAggregationGraph()).
            let m = &v.data.multaggr;
            if m.nvars == 1 {
                scaled(m.vars[0], m.scalars[0], m.constant)
            } else {
                panic!("cannot get the bounds of a multi-aggregated variable");
            }
        }
        // x' = offset - x  ->  x = offset - x'
        VarStatus::Negated => v.data.negate.constant - get_var_bd_at_index(scip, v.negatedvar, opposite, bdchgidx, after),
    }
}

/// `SCIPgetVarLbAtIndex` (`scip_var.c`).
pub fn get_var_lb_at_index(scip: &Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>, after: bool) -> f64 {
    get_var_bd_at_index(scip, var, crate::types::BoundType::Lower, bdchgidx, after)
}

/// `SCIPgetVarUbAtIndex` (`scip_var.c`).
pub fn get_var_ub_at_index(scip: &Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>, after: bool) -> f64 {
    get_var_bd_at_index(scip, var, crate::types::BoundType::Upper, bdchgidx, after)
}

/// `SCIPgetVarWasFixedAtIndex` (`scip_var.c`): returns whether the binary
/// variable was fixed at the time given by the bound change index.
pub fn get_var_was_fixed_at_index(scip: &Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>, after: bool) -> bool {
    // Check the current bounds first in order to decide at which bound change
    // information we have to look (which is expensive because we have to
    // follow the aggregation tree to the active variable).
    (scip.vars[var].locdom.lb > 0.5 && get_var_lb_at_index(scip, var, bdchgidx, after) > 0.5)
        || (scip.vars[var].locdom.ub < 0.5 && get_var_ub_at_index(scip, var, bdchgidx, after) < 0.5)
}

/// `SCIPgetSolVal` (`scip_sol.c`): gets value of variable in primal CIP
/// solution, or in current LP/pseudo solution (`sol` is `NONE`).
pub fn get_sol_val(scip: &Scip, sol: crate::types::SolId, var: crate::types::VarId) -> f64 {
    if sol != crate::var::NONE {
        return crate::sol::sol_get_val(scip, sol, var);
    }
    crate::var::var_get_sol(scip, var, crate::tree::tree_has_current_node_lp(scip))
}

/// The problem the `SCIPgetVars` family reads in the current stage: the
/// original one while in `SCIP_STAGE_PROBLEM`, the transformed one after.
#[inline]
fn stage_prob(scip: &Scip) -> &crate::prob::Prob {
    if scip.set.stage == Stage::Problem {
        &scip.probs[crate::prob::ORIGPROB]
    } else {
        &scip.probs[crate::prob::TRANSPROB]
    }
}

/// `SCIPgetVars` (`scip_prob.c`): gets array with active problem variables.
#[inline]
pub fn get_vars(scip: &Scip) -> &[crate::types::VarId] {
    let p = stage_prob(scip);
    &p.vars.0[..p.nvars as usize]
}

/// `SCIPgetNVars` (`scip_prob.c`): gets number of active problem variables.
#[inline]
pub fn get_n_vars(scip: &Scip) -> i32 {
    stage_prob(scip).nvars
}

/// `SCIPgetNContVars` (`scip_prob.c`): gets number of continuous active
/// problem variables.
#[inline]
pub fn get_n_cont_vars(scip: &Scip) -> i32 {
    stage_prob(scip).ncontvars
}

/// `SCIPcreateSol` (`scip_sol.c`): creates a primal solution, initialized
/// to zero; `heur` is the heuristic that found the solution (or `NONE` if
/// it's from the tree).
pub fn create_sol(scip: &mut Scip, heur: crate::types::HeurId) -> crate::retcode::ScipResult<crate::types::SolId> {
    use crate::retcode::Retcode;
    match scip.set.stage {
        Stage::Problem => crate::sol::sol_create_original(scip, crate::prob::ORIGPROB, crate::scip::ORIGPRIMAL, heur),
        Stage::Transforming | Stage::Transformed | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::InitSolve | Stage::Solving => {
            crate::sol::sol_create(scip, TRANSPRIMAL, heur)
        }
        s => {
            log::error!("invalid SCIP stage <{}>", s as i32);
            Err(Retcode::InvalidData)
        }
    }
}

/// `SCIPfreeSol` (`scip_sol.c`): frees primal CIP solution.
pub fn free_sol(scip: &mut Scip, sol: crate::types::SolId) -> crate::retcode::ScipResult {
    use crate::retcode::Retcode;
    match scip.set.stage {
        Stage::Problem => crate::sol::sol_free(scip, sol, crate::scip::ORIGPRIMAL),
        Stage::FreeTrans
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::Solving
        | Stage::Transforming
        | Stage::InitSolve
        | Stage::Solved
        | Stage::ExitSolve => crate::sol::sol_free(scip, sol, TRANSPRIMAL),
        s => {
            log::error!("invalid SCIP stage <{}>", s as i32);
            Err(Retcode::InvalidCall)
        }
    }
}

/// `SCIPgetNBinVars` (`scip_prob.c`): gets number of binary active problem
/// variables (without implied ones).
#[inline]
pub fn get_n_bin_vars(scip: &Scip) -> i32 {
    stage_prob(scip).nbinvars
}

/// `SCIPgetNIntVars` (`scip_prob.c`): gets number of integer active
/// problem variables (without implied ones).
#[inline]
pub fn get_n_int_vars(scip: &Scip) -> i32 {
    stage_prob(scip).nintvars
}

/// `SCIPgetNImplVars` (`scip_prob.c`): gets number of implied integral
/// active problem variables.
#[inline]
pub fn get_n_impl_vars(scip: &Scip) -> i32 {
    let p = stage_prob(scip);
    p.nbinimplvars + p.nintimplvars + p.ncontimplvars
}

/// `SCIPgetNBinImplVars` (`scip_prob.c`): gets number of implied integral
/// binary active problem variables.
#[inline]
pub fn get_n_bin_impl_vars(scip: &Scip) -> i32 {
    stage_prob(scip).nbinimplvars
}

/// `SCIPgetNIntImplVars` (`scip_prob.c`): gets number of implied integral
/// integer active problem variables.
#[inline]
pub fn get_n_int_impl_vars(scip: &Scip) -> i32 {
    stage_prob(scip).nintimplvars
}

/// `SCIPgetNContImplVars` (`scip_prob.c`): gets number of implied integral
/// continuous active problem variables.
#[inline]
pub fn get_n_cont_impl_vars(scip: &Scip) -> i32 {
    stage_prob(scip).ncontimplvars
}

/// `SCIPisEfficacious` (`scip_cut.c`): checks, if the given cut's efficacy
/// is larger than the minimal cut efficacy.
#[inline]
pub fn is_efficacious(scip: &Scip, efficacy: f64) -> bool {
    scip.set.is_efficacious(crate::tree::tree_get_current_depth(scip) == 0, efficacy)
}

/// `SCIPgetRowSolActivity` (`scip_lp.c`): returns the activity of a row for
/// the given primal solution (`NONE` for the current LP or pseudo
/// solution).
pub fn get_row_sol_activity(scip: &mut Scip, row: crate::types::RowId, sol: crate::types::SolId) -> f64 {
    if sol != crate::var::NONE {
        crate::lp::row_get_sol_activity(scip, row, sol)
    } else if crate::tree::tree_has_current_node_lp(scip) {
        crate::lp::row_get_lp_activity(scip, row)
    } else {
        crate::lp::row_get_pseudo_activity(scip, row)
    }
}

/// `SCIPgetUpperbound` (`scip_solvingstats.c`): gets global upper (primal)
/// bound in transformed problem (objective value of best solution or user
/// objective limit).
pub fn get_upperbound(scip: &Scip) -> f64 {
    if get_status(scip) == Status::Unbounded {
        -scip.set.infinity()
    } else {
        scip.primals[TRANSPRIMAL].upperbound
    }
}

/// `SCIPgetCutoffbound` (`scip_solvingstats.c`): gets global cutoff bound in
/// transformed problem.
pub fn get_cutoffbound(scip: &Scip) -> f64 {
    scip.primals[TRANSPRIMAL].cutoffbound
}

/// `SCIPgetPrimalbound` (`scip_solvingstats.c`): gets global primal bound
/// (objective value of best solution or user objective limit) for the
/// original problem.
pub fn get_primalbound(scip: &Scip) -> f64 {
    crate::prob::prob_extern_objval(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, get_upperbound(scip))
}

/// `SCIPgetDualbound` (`scip_solvingstats.c`): gets global dual bound.
pub fn get_dualbound(scip: &Scip) -> f64 {
    let t = &scip.probs[crate::prob::TRANSPROB];
    // In case we are in presolving we use the stored dual bound if it exits.
    if scip.set.stage <= Stage::InitSolve && t.dualbound < crate::def::INVALID {
        return t.dualbound;
    }
    crate::prob::prob_extern_objval(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, get_lowerbound(scip))
}

/// `SCIPgetGap` (`scip_solvingstats.c`): gets current gap |(primalbound -
/// dualbound)/min(|primalbound|,|dualbound|)| if both bounds have same sign,
/// or infinity, if they have opposite sign.
pub fn get_gap(scip: &Scip) -> f64 {
    // In case we could not prove whether the problem is unbounded or
    // infeasible, we want to terminate with gap = +inf; if the problem was
    // proven to be unbounded or proven to be infeasible we return gap = 0.
    if get_status(scip) == Status::InfOrUnbd {
        return scip.set.infinity();
    } else if get_status(scip) == Status::Infeasible || get_status(scip) == Status::Unbounded {
        return 0.0;
    }

    // The lowerbound is infinity, but SCIP may not have updated the status; in
    // this case, the problem was already solved so we return gap = 0.
    if scip.set.is_infinity(get_lowerbound(scip)) {
        return 0.0;
    }

    crate::misc::compute_gap(scip.set.epsilon(), scip.set.infinity(), get_primalbound(scip), get_dualbound(scip))
}

/// `SCIPgetNLimSolsFound` (`scip_solvingstats.c`): gets number of feasible
/// primal solutions respecting the objective limit found so far.
pub fn get_n_lim_sols_found(scip: &Scip) -> i64 {
    if scip.set.stage < Stage::Transformed {
        return 0;
    }
    scip.primals[TRANSPRIMAL].nlimsolsfound
}

/// `SCIPgetNBestSolsFound` (`scip_solvingstats.c`): gets number of feasible
/// primal solutions found so far, that improved the primal bound at the time
/// they were found.
pub fn get_n_best_sols_found(scip: &Scip) -> i64 {
    scip.primals[TRANSPRIMAL].nbestsolsfound
}

/// `SCIPgetTransObjscale` (`scip_prob.c`): returns the objective scale of the
/// transformed problem.
pub fn get_trans_objscale(scip: &Scip) -> f64 {
    scip.probs[crate::prob::TRANSPROB].objscale
}

/// `SCIPgetTransObjoffset` (`scip_prob.c`): returns the objective offset of
/// the transformed problem.
pub fn get_trans_objoffset(scip: &Scip) -> f64 {
    scip.probs[crate::prob::TRANSPROB].objoffset
}

/// `SCIPgetObjsense` (`scip_prob.c`): returns the objective sense of the
/// original problem.
pub fn get_objsense(scip: &Scip) -> crate::types::ObjSense {
    scip.probs[crate::prob::ORIGPROB].objsense
}

/// `SCIPstoreSolutionGap` (`scip_solvingstats.c`): stores the gap of the
/// current primal and dual bounds as the last (and, for the first solution,
/// first) solution gap.
pub fn store_solution_gap(scip: &mut Scip) {
    scip.stat.lastsolgap = crate::misc::compute_gap(scip.set.epsilon(), scip.set.infinity(), get_primalbound(scip), get_dualbound(scip));

    if scip.primals[TRANSPRIMAL].nsols == 1 {
        scip.stat.firstsolgap = scip.stat.lastsolgap;
    }

    if scip.set.misc_calcintegral {
        let upperbound = get_upperbound(scip);

        if upperbound < scip.stat.lastupperbound {
            let inf = scip.set.infinity();
            crate::stat::stat_update_primal_dual_integrals(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, upperbound, -inf);
        }
    }
}

/// `SCIPcheckSolOrig` (`scip_sol.c`): checks solution for feasibility in
/// original problem without adding it to the solution store; returns
/// whether the solution is feasible.
pub fn check_sol_orig(scip: &mut Scip, sol: crate::types::SolId, printreason: bool, mut completely: bool) -> crate::retcode::ScipResult<bool> {
    // Return immediately if the solution is of type partial.
    if scip.sols[sol].solorigin == crate::types::SolOrigin::Partial {
        log::error!("Cannot check feasibility of partial solutions.");
        return Err(crate::retcode::Retcode::InvalidData);
    }

    if !printreason {
        completely = false;
    }

    // Check solution in original problem; that includes bounds, integrality,
    // and non modifiable constraints.
    crate::sol::sol_check_orig(scip, sol, crate::prob::ORIGPROB, crate::scip::ORIGPRIMAL, printreason, completely, true, true, true, false)
}

/// The lock counts `SCIPlockVarCons` and `SCIPunlockVarCons` add per lock
/// type for a constraint's down and up locks.
fn cons_lock_counts(scip: &Scip, cons: crate::types::ConsId, lockdown: bool, lockup: bool) -> ([i32; crate::types::NLOCKTYPES], [i32; crate::types::NLOCKTYPES]) {
    let mut nlocksdown = [0; crate::types::NLOCKTYPES];
    let mut nlocksup = [0; crate::types::NLOCKTYPES];
    let c = &scip.conss[cons];
    for i in 0..crate::types::NLOCKTYPES {
        if c.nlockspos[i] > 0 {
            if lockdown {
                nlocksdown[i] += 1;
            }
            if lockup {
                nlocksup[i] += 1;
            }
        }
        if c.nlocksneg[i] > 0 {
            if lockdown {
                nlocksup[i] += 1;
            }
            if lockup {
                nlocksdown[i] += 1;
            }
        }
    }
    (nlocksdown, nlocksup)
}

/// Applies (or, `unlock`, removes) the lock counts of a constraint.
fn apply_cons_locks(
    scip: &mut Scip,
    var: crate::types::VarId,
    nlocksdown: [i32; crate::types::NLOCKTYPES],
    nlocksup: [i32; crate::types::NLOCKTYPES],
    unlock: bool,
) -> crate::retcode::ScipResult {
    match scip.set.stage {
        Stage::Problem
        | Stage::Transforming
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::InitSolve
        | Stage::Solving
        | Stage::ExitSolve
        | Stage::FreeTrans => {
            for i in 0..crate::types::NLOCKTYPES {
                if nlocksdown[i] == 0 && nlocksup[i] == 0 {
                    continue;
                }
                let lt = if i == 0 { crate::types::LockType::Model } else { crate::types::LockType::Conflict };
                if unlock {
                    crate::var::var_add_locks(scip, var, lt, -nlocksdown[i], -nlocksup[i])?;
                } else {
                    crate::var::var_add_locks(scip, var, lt, nlocksdown[i], nlocksup[i])?;
                }
            }
            Ok(())
        }
        s => {
            log::error!("invalid SCIP stage <{}>", s as i32);
            Err(crate::retcode::Retcode::InvalidCall)
        }
    }
}

/// `SCIPlockVarCons` (`scip_var.c`): add locks of type `locktype` of
/// variable with respect to the lock status of the constraint and its
/// negation; this method should be called whenever the lock status of a
/// variable in a constraint changes.
pub fn lock_var_cons(scip: &mut Scip, var: crate::types::VarId, cons: crate::types::ConsId, lockdown: bool, lockup: bool) -> crate::retcode::ScipResult {
    let (down, up) = cons_lock_counts(scip, cons, lockdown, lockup);
    apply_cons_locks(scip, var, down, up, false)
}

/// `SCIPunlockVarCons` (`scip_var.c`): remove locks of type `locktype` of
/// variable with respect to the lock status of the constraint and its
/// negation.
pub fn unlock_var_cons(scip: &mut Scip, var: crate::types::VarId, cons: crate::types::ConsId, lockdown: bool, lockup: bool) -> crate::retcode::ScipResult {
    let (down, up) = cons_lock_counts(scip, cons, lockdown, lockup);
    apply_cons_locks(scip, var, down, up, true)
}

/// `SCIPcatchVarEvent` (`scip_event.c`): catches an objective value or
/// domain change event on the given transformed variable.
pub fn catch_var_event(
    scip: &mut Scip,
    var: crate::types::VarId,
    eventtype: crate::event::EventType,
    eventhdlr: crate::types::EventhdlrId,
    eventdata: usize,
    filterpos: Option<&mut i32>,
) -> crate::retcode::ScipResult {
    if eventtype & crate::event::EVENTTYPE_VARCHANGED == 0 {
        log::error!("event does not operate on a single variable");
        return Err(crate::retcode::Retcode::InvalidData);
    }
    if crate::var::var_is_original(scip, var) {
        log::error!("cannot catch events on original variable <{}>", scip.vars[var].name);
        return Err(crate::retcode::Retcode::InvalidData);
    }
    crate::var::var_catch_event(scip, var, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIPdropVarEvent` (`scip_event.c`): drops an objective value or domain
/// change event (stops to track event) on the given transformed variable.
pub fn drop_var_event(
    scip: &mut Scip,
    var: crate::types::VarId,
    eventtype: crate::event::EventType,
    eventhdlr: crate::types::EventhdlrId,
    eventdata: usize,
    filterpos: i32,
) -> crate::retcode::ScipResult {
    if crate::var::var_is_original(scip, var) {
        log::error!("cannot drop events on original variable <{}>", scip.vars[var].name);
        return Err(crate::retcode::Retcode::InvalidData);
    }
    crate::var::var_drop_event(scip, var, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIPisConsCompressionEnabled` (`scip_copy.c`): is constraint
/// compression enabled for reductions? With constraint compression
/// enabled, fixed variables can be treated as constants by all
/// constraints that are copied after calling this method.
pub fn is_cons_compression_enabled(scip: &Scip) -> bool {
    scip.probs[crate::prob::ORIGPROB].conscompression && scip.set.stage == Stage::Problem
}

/// `SCIPisTransformed` (`scip_general.c`): returns whether the solution
/// process is in a stage after transformation.
pub fn is_transformed(scip: &Scip) -> bool {
    scip.set.stage as i32 >= Stage::Transforming as i32
}

/// `SCIPgetTransformedVars` (`scip_var.c`): gets corresponding transformed
/// variables for a given array of variables (in place).
pub fn get_transformed_vars(scip: &mut Scip, vars: &mut [crate::types::VarId]) -> crate::retcode::ScipResult {
    for v in vars.iter_mut() {
        if !crate::var::var_is_transformed(scip, *v) {
            let mut t = crate::var::NONE;
            crate::var::var_get_transformed(scip, *v, &mut t)?;
            *v = t;
        }
    }
    Ok(())
}

/// `SCIPreleaseVar` (`scip_var.c`): decreases usage counter of variable,
/// if the usage pointer reaches zero the variable gets freed.
pub fn release_var(scip: &mut Scip, var: &mut crate::types::VarId) -> crate::retcode::ScipResult {
    use crate::retcode::Retcode;
    match scip.set.stage {
        Stage::Problem => crate::var::var_release(scip, var),
        Stage::Transforming
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::Solved
        | Stage::ExitSolve
        | Stage::FreeTrans => {
            let v = &scip.vars[*var];
            if !crate::var::var_is_transformed(scip, *var) && v.nuses == 1 && v.data.original.transvar != crate::var::NONE {
                log::error!("cannot release last use of original variable while associated transformed variable exists");
                return Err(Retcode::InvalidCall);
            }
            crate::var::var_release(scip, var)
        }
        s => {
            log::error!("invalid SCIP stage <{}>", s as i32);
            Err(Retcode::InvalidCall)
        }
    }
}

/// `SCIPgetTransformedVar` (`scip_var.c`): gets corresponding transformed
/// variable of a given variable.
pub fn get_transformed_var(scip: &mut Scip, var: crate::types::VarId) -> crate::retcode::ScipResult<crate::types::VarId> {
    if crate::var::var_is_transformed(scip, var) {
        Ok(var)
    } else {
        let mut t = crate::var::NONE;
        crate::var::var_get_transformed(scip, var, &mut t)?;
        Ok(t)
    }
}

/// `SCIPconsIsLockedType` (`cons.c`): is constraint locked in either
/// direction for the given lock type?
pub fn cons_is_locked_type(scip: &Scip, cons: crate::types::ConsId, locktype: usize) -> bool {
    let c = &scip.conss[cons];
    c.nlockspos[locktype] > 0 || c.nlocksneg[locktype] > 0
}

/// `SCIPconsIsTransformed` (`cons.c`): returns TRUE iff constraint belongs
/// to the transformed problem.
pub fn cons_is_transformed(scip: &Scip, cons: crate::types::ConsId) -> bool {
    !scip.conss[cons].original
}

/// `SCIPaddVarToRow` (`scip_lp.c`): resolves variable to columns and adds
/// them with the coefficient to the row.
pub fn add_var_to_row(scip: &mut Scip, row: crate::types::RowId, var: crate::types::VarId, val: f64) -> crate::retcode::ScipResult {
    crate::var::var_add_to_row(scip, var, crate::prob::TRANSPROB, row, val)
}

/// `SCIPaddConflictLb` (`scip_conflict.c`): adds lower bound of variable at
/// the time of the given bound change index to the conflict analysis'
/// candidate storage.
pub fn add_conflict_lb(scip: &mut Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>) -> crate::retcode::ScipResult {
    crate::conflict::conflict_add_bound_public(scip, var, crate::types::BoundType::Lower, bdchgidx)
}

/// `SCIPaddConflictUb` (`scip_conflict.c`): adds upper bound of variable at
/// the time of the given bound change index to the conflict analysis'
/// candidate storage.
pub fn add_conflict_ub(scip: &mut Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>) -> crate::retcode::ScipResult {
    crate::conflict::conflict_add_bound_public(scip, var, crate::types::BoundType::Upper, bdchgidx)
}

/// `SCIPinitConflictAnalysis` (`scip_conflict.c`): initializes the conflict
/// analysis by clearing the conflict candidate queue.
pub fn init_conflict_analysis(scip: &mut Scip, conftype: crate::types::ConfType, iscutoffinvolved: bool) -> crate::retcode::ScipResult {
    crate::conflict::conflict_init(scip, crate::prob::TRANSPROB, conftype, iscutoffinvolved)
}

/// `SCIPanalyzeConflictCons` (`scip_conflict.c`): analyzes conflict bounds
/// that were added with calls to SCIPaddConflict...(), with the given
/// constraint as the conflict detecting constraint; returns whether a
/// conflict constraint was created. Generalized resolution
/// (`conflict/usegenres`) is not ported.
pub fn analyze_conflict_cons(scip: &mut Scip, cons: crate::types::ConsId) -> crate::retcode::ScipResult<bool> {
    let c = &scip.conss[cons];
    let validdepth = if c.is_global() {
        0
    } else if c.is_active() {
        c.get_valid_depth()
    } else {
        return Ok(false);
    };

    if scip.set.conf_usegenres {
        unimplemented!("generalized resolution conflict analysis");
    }

    crate::conflict::conflict_analyze_public(scip, crate::prob::TRANSPROB, validdepth)
}

/// `SCIPgetDepth` (`scip_tree.c`): gets depth of current node, or -1 if no
/// current node exists.
pub fn get_depth(scip: &Scip) -> i32 {
    crate::tree::tree_get_current_depth(scip)
}

/// `SCIPisConflictAnalysisApplicable` (`scip_conflict.c`): return TRUE if
/// conflict analysis is applicable.
pub fn is_conflict_analysis_applicable(scip: &Scip) -> bool {
    get_depth(scip) > 0 && crate::conflict::conflict_applicable(scip)
}

/// `SCIPinProbing` (`scip_probing.c`): returns whether we are in probing
/// mode.
pub fn in_probing(scip: &Scip) -> bool {
    crate::tree::tree_probing(scip)
}

/// `SCIPupdateSolLPConsViolation` (`scip_sol.c`): allows the constraint
/// handlers to update the LP row violations of the solution.
pub fn update_sol_lp_cons_violation(scip: &mut Scip, sol: crate::types::SolId, absviol: f64, relviol: f64) {
    if scip.primals[crate::scip::ORIGPRIMAL].updateviolations {
        crate::sol::sol_update_lp_cons_violation(scip, sol, absviol, relviol);
    }
}

/// `SCIPupdateSolConsViolation` (`scip_sol.c`): allows the constraint
/// handlers to update the constraint violations of the solution.
pub fn update_sol_cons_violation(scip: &mut Scip, sol: crate::types::SolId, absviol: f64, relviol: f64) {
    if scip.primals[crate::scip::ORIGPRIMAL].updateviolations {
        crate::sol::sol_update_cons_violation(scip, sol, absviol, relviol);
    }
}

/// `SCIPincConsAge` (`scip_cons.c`): increases age of constraint by 1.0.
pub fn inc_cons_age(scip: &mut Scip, cons: crate::types::ConsId) -> crate::retcode::ScipResult {
    crate::cons::cons_inc_age(scip, cons, crate::prob::TRANSPROB)
}

/// `SCIPresetConsAge` (`scip_cons.c`): resets age of constraint to zero.
pub fn reset_cons_age(scip: &mut Scip, cons: crate::types::ConsId) {
    crate::cons::cons_reset_age(scip, cons);
}

/// `SCIPgetNObjVars` (`scip_prob.c`): gets number of variables with a
/// non-zero objective coefficient.
pub fn get_n_obj_vars(scip: &Scip) -> i32 {
    match scip.set.stage {
        Stage::Problem => crate::prob::prob_get_nobj_vars(scip, crate::prob::ORIGPROB),
        Stage::Transformed | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::InitSolve | Stage::Solving | Stage::Solved => {
            crate::prob::prob_get_nobj_vars(scip, crate::prob::TRANSPROB)
        }
        _ => panic!("invalid SCIP stage"),
    }
}

/// `SCIPaddObjoffset` (`scip_prob.c`): adds offset of objective function
/// to transformed problem.
pub fn add_objoffset(scip: &mut Scip, addval: f64) -> crate::retcode::ScipResult {
    crate::prob::prob_add_objoffset(scip, crate::prob::TRANSPROB, addval);
    crate::primal::primal_update_objoffset(scip, TRANSPRIMAL, crate::prob::TRANSPROB, crate::prob::ORIGPROB)
}

/// `SCIPupdateNodeLowerbound` (`scip_prob.c`): if given value is larger
/// than the node's lower bound (in transformed problem), sets the node's
/// lower bound to the new value; cuts the node off if it exceeds the
/// cutoff bound.
pub fn update_node_lowerbound(scip: &mut Scip, node: crate::types::NodeId, newbound: f64) -> crate::retcode::ScipResult {
    use crate::{
        prob::{ORIGPROB, TRANSPROB},
        scip::GLOBALEVENTFILTER,
    };
    if scip.set.is_lt(newbound, scip.primals[TRANSPRIMAL].cutoffbound) {
        crate::tree::node_update_lowerbound(scip, node, GLOBALEVENTFILTER, TRANSPROB, ORIGPROB, newbound)
    } else {
        crate::tree::node_cutoff(scip, node, GLOBALEVENTFILTER, TRANSPROB, ORIGPROB)
    }
}

/// `SCIPupdateLocalLowerbound` (`scip_prob.c`): if given value is larger
/// than the current node's lower bound (in transformed problem), sets the
/// current node's lower bound to the new value.
pub fn update_local_lowerbound(scip: &mut Scip, newbound: f64) -> crate::retcode::ScipResult {
    use crate::prob::{ORIGPROB, TRANSPROB};
    match scip.set.stage {
        Stage::Presolving | Stage::Presolved => {
            let ext = crate::prob::prob_extern_objval(scip, TRANSPROB, ORIGPROB, newbound);
            crate::prob::prob_update_dualbound(scip, TRANSPROB, ext);
            Ok(())
        }
        Stage::Solving => {
            let node = crate::tree::tree_get_current_node(scip);
            update_node_lowerbound(scip, node, newbound)
        }
        _ => Err(crate::retcode::Retcode::InvalidCall),
    }
}

/// `SCIPupdateCutoffbound` (`scip_solvingstats.c`): updates the cutoff
/// bound.
pub fn update_cutoffbound(scip: &mut Scip, cutoffbound: f64) -> crate::retcode::ScipResult {
    use crate::prob::{ORIGPROB, TRANSPROB};
    crate::primal::primal_set_cutoffbound(scip, TRANSPRIMAL, TRANSPROB, ORIGPROB, crate::scip::GLOBALEVENTFILTER, cutoffbound, false)
}

/// `SCIPgetNSepaRounds` (`scip_solvingstats.c`): gets number of separation
/// rounds performed so far at the current node.
pub fn get_n_sepa_rounds(scip: &Scip) -> i32 {
    scip.stat.nseparounds
}

/// `SCIPgetLocalLowerbound` (`scip_prob.c`): gets lower bound of current
/// node in transformed problem.
pub fn get_local_lowerbound(scip: &Scip) -> f64 {
    let node = crate::tree::tree_get_current_node(scip);
    if node != crate::var::NONE { scip.nodes[node].lowerbound } else { crate::def::INVALID }
}

/// `SCIPgetNLPBranchCands` (`scip_branch.c`): gets number of branching
/// candidates for LP solution branching (number of fractional variables).
pub fn get_n_lp_branch_cands(scip: &mut Scip) -> i32 {
    use crate::types::LpSolStat;
    let solstat = crate::lp::lp_get_solstat(scip);
    if solstat != LpSolStat::Optimal && solstat != LpSolStat::UnboundedRay {
        panic!("LP not solved to optimality");
    }
    crate::branch::branchcand_get_lp_cands(scip).nlpcands
}

/// `SCIPisPresolveFinished` (`scip_general.c`): returns whether the
/// presolving process would be finished given no more presolving reductions
/// are found in this presolving round.
pub fn is_presolve_finished(scip: &Scip) -> bool {
    let st = &scip.stat;
    let counts = crate::cons::PresolCounts {
        nfixedvars: st.npresolfixedvars,
        naggrvars: st.npresolaggrvars,
        nchgvartypes: st.npresolchgvartypes,
        nchgbds: st.npresolchgbds,
        naddholes: st.npresoladdholes,
        ndelconss: st.npresoldelconss,
        naddconss: st.npresoladdconss,
        nupgdconss: st.npresolupgdconss,
        nchgcoefs: st.npresolchgcoefs,
        nchgsides: st.npresolchgsides,
    };
    is_presolve_finished_with(scip, &counts)
}

/// [`is_presolve_finished`] on the presolving counts a presolving callback
/// is updating: C's callbacks update the statistics' counters through
/// pointers, so `SCIPisPresolveFinished` inside a callback sees them.
pub fn is_presolve_finished_with(scip: &Scip, c: &crate::cons::PresolCounts) -> bool {
    let st = &scip.stat;
    let t = &scip.probs[crate::prob::TRANSPROB];

    // Get maximum number of presolving rounds.
    let mut maxnrounds = scip.set.presol_maxrounds;
    if maxnrounds == -1 {
        maxnrounds = i32::MAX;
    }

    // Don't abort, if enough changes were applied to the variables.
    let varchanges = (c.nfixedvars - st.lastnpresolfixedvars + c.naggrvars - st.lastnpresolaggrvars + c.nchgvartypes - st.lastnpresolchgvartypes) as f64
        + (c.nchgbds - st.lastnpresolchgbds) as f64 / 10.0
        + (c.naddholes - st.lastnpresoladdholes) as f64 / 10.0;
    let mut finished = t.nvars == 0 || varchanges <= scip.set.presol_abortfac * t.nvars as f64;

    // Don't abort, if enough changes were applied to the constraints.
    finished = finished
        && (t.nconss == 0
            || ((c.ndelconss - st.lastnpresoldelconss + c.naddconss - st.lastnpresoladdconss + c.nupgdconss - st.lastnpresolupgdconss + c.nchgsides - st.lastnpresolchgsides)
                as f64)
                <= scip.set.presol_abortfac * t.nconss as f64);

    // Don't abort, if enough changes were applied to the coefficients (assume a
    // 1% density of non-zero elements).
    finished = finished && (t.nvars == 0 || t.nconss == 0 || ((c.nchgcoefs - st.lastnpresolchgcoefs) as f64) <= scip.set.presol_abortfac * 0.01 * t.nvars as f64 * t.nconss as f64);

    // Abort if maximal number of presolving rounds is reached.
    finished || st.npresolrounds + 1 >= maxnrounds
}

/// `SCIPaddVarLocksType` (`scip_var.c`): adds given values to lock numbers
/// of type `locktype` of variable for rounding.
pub fn add_var_locks_type(scip: &mut Scip, var: crate::types::VarId, locktype: crate::types::LockType, nlocksdown: i32, nlocksup: i32) -> crate::retcode::ScipResult {
    match scip.set.stage {
        Stage::Problem
        | Stage::Transforming
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::ExitSolve
        | Stage::FreeTrans => crate::var::var_add_locks(scip, var, locktype, nlocksdown, nlocksup),
        _ => Err(crate::retcode::Retcode::InvalidCall),
    }
}

/// `SCIPincludeEventhdlrBasic` (`scip_event.c`): creates an event handler
/// and includes it in SCIP with its most fundamental callback.
pub fn include_eventhdlr_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    eventexec: crate::event::EventExec,
    eventhdlrdata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::EventhdlrId> {
    // Check whether event handler is already present.
    if crate::set::set_find_eventhdlr(scip, name) != crate::var::NONE {
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let h = crate::event::eventhdlr_create(name, desc, None, None, None, None, None, None, None, Some(eventexec), eventhdlrdata);
    let id = scip.eventhdlrs.push(h);
    crate::set::set_include_eventhdlr(scip, id);

    Ok(id)
}

/// `SCIPincludeConflicthdlrBasic` (`scip_conflict.c`): creates a conflict
/// handler and includes it in SCIP with its most fundamental callback.
pub fn include_conflicthdlr_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    conflictexec: crate::conflict::ConflicthdlrExec,
    conflicthdlrdata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::ConflicthdlrId> {
    // Check whether conflict handler is already present.
    if crate::set::set_find_conflicthdlr(scip, name) != crate::var::NONE {
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = crate::conflict::ConflicthdlrMethods {
        conflictexec: Some(conflictexec),
        ..Default::default()
    };
    let h = crate::conflict::conflicthdlr_create(scip, name, desc, priority, methods, conflicthdlrdata)?;
    crate::set::set_include_conflicthdlr(scip, h);

    Ok(h)
}

/// `SCIPaddConflictBinvar` (`scip_conflict.c`): adds changed bound of
/// fixed binary variable to the conflict analysis' candidate storage.
pub fn add_conflict_binvar(scip: &mut Scip, var: crate::types::VarId) -> crate::retcode::ScipResult {
    if scip.vars[var].locdom.lb > 0.5 {
        crate::conflict::conflict_add_bound_public(scip, var, crate::types::BoundType::Lower, None)?;
    } else if scip.vars[var].locdom.ub < 0.5 {
        crate::conflict::conflict_add_bound_public(scip, var, crate::types::BoundType::Upper, None)?;
    }
    Ok(())
}

/// `SCIPgetNFixedVars` (`scip_prob.c`): gets number of fixed or aggregated
/// problem variables (zero in `SCIP_STAGE_PROBLEM`).
pub fn get_n_fixed_vars(scip: &Scip) -> i32 {
    if scip.set.stage == Stage::Problem {
        0
    } else {
        scip.probs[crate::prob::TRANSPROB].nfixedvars
    }
}

/// `SCIPgetNImplications` (`scip_solvingstats.c`): gets total number of
/// implications between variables that are stored in the implication graph.
pub fn get_n_implications(scip: &Scip) -> i32 {
    scip.stat.nimplications
}

/// `SCIPgetNRuns` (`scip_solvingstats.c`): gets number of branch and bound
/// runs performed, including the current run.
pub fn get_n_runs(scip: &Scip) -> i32 {
    scip.stat.nruns
}

/// `SCIPaddConsAge` (`scip_cons.c`): adds given value to age of constraint,
/// but age can never become negative.
pub fn add_cons_age(scip: &mut Scip, cons: crate::types::ConsId, deltaage: f64) -> crate::retcode::ScipResult {
    crate::cons::cons_add_age(scip, cons, crate::prob::TRANSPROB, deltaage)
}

/// `SCIPgetBestSol` (`scip_sol.c`): gets best feasible primal solution
/// found so far, or `NONE` if no solution has been found.
pub fn get_best_sol(scip: &Scip) -> crate::types::SolId {
    use crate::scip::{ORIGPRIMAL, TRANSPRIMAL};
    match scip.set.stage {
        Stage::Init => crate::var::NONE,
        Stage::Problem => {
            let p = &scip.primals[ORIGPRIMAL];
            if p.nsols > 0 { p.sols[0usize] } else { crate::var::NONE }
        }
        Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::Solved
        | Stage::ExitSolve => {
            let p = &scip.primals[TRANSPRIMAL];
            if p.nsols > 0 { p.sols[0usize] } else { crate::var::NONE }
        }
        _ => {
            log::error!("invalid SCIP stage <{:?}>", scip.set.stage);
            crate::var::NONE
        }
    }
}

/// `SCIPgetNSols` (`scip_sol.c`): gets number of feasible primal solutions
/// stored in the solution storage.
pub fn get_n_sols(scip: &Scip) -> i32 {
    use crate::scip::{ORIGPRIMAL, TRANSPRIMAL};
    match scip.set.stage {
        Stage::Problem => scip.primals[ORIGPRIMAL].nsols,
        Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::Solved
        | Stage::ExitSolve => scip.primals[TRANSPRIMAL].nsols,
        _ => panic!("invalid SCIP stage <{:?}>", scip.set.stage),
    }
}

/// `SCIPgetSols` (`scip_sol.c`): gets array of feasible primal solutions
/// stored in the solution storage.
pub fn get_sols(scip: &Scip) -> &[crate::types::SolId] {
    use crate::scip::{ORIGPRIMAL, TRANSPRIMAL};
    let p = match scip.set.stage {
        Stage::Problem => &scip.primals[ORIGPRIMAL],
        Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::Solved
        | Stage::ExitSolve => &scip.primals[TRANSPRIMAL],
        _ => panic!("invalid SCIP stage <{:?}>", scip.set.stage),
    };
    &p.sols.0[..p.nsols as usize]
}

/// `SCIPgetSolOrigObj` (`scip_sol.c`): returns objective value of primal
/// CIP solution w.r.t. original problem, or current LP/pseudo objective
/// value (`sol == NONE`).
pub fn get_sol_orig_obj(scip: &mut Scip, sol: crate::types::SolId) -> f64 {
    use crate::prob::{ORIGPROB, TRANSPROB};
    if sol != crate::var::NONE && crate::sol::sol_is_original(scip, sol) {
        // SCIPsolGetOrigObj.
        return scip.sols[sol].obj;
    }
    if sol != crate::var::NONE {
        let obj = crate::sol::sol_get_obj(scip, sol, TRANSPROB, ORIGPROB);
        crate::prob::prob_extern_objval(scip, TRANSPROB, ORIGPROB, obj)
    } else if crate::tree::tree_has_current_node_lp(scip) {
        let obj = crate::lp::lp_get_objval(scip, TRANSPROB);
        crate::prob::prob_extern_objval(scip, TRANSPROB, ORIGPROB, obj)
    } else {
        let obj = crate::lp::lp_get_pseudo_objval(scip, TRANSPROB);
        crate::prob::prob_extern_objval(scip, TRANSPROB, ORIGPROB, obj)
    }
}

/// `SCIPtrySolFree` (`scip_sol.c`): checks primal solution; if feasible,
/// adds it to storage; solution is freed afterwards; returns whether it was
/// stored. Partial solutions are not ported.
pub fn try_sol_free(
    scip: &mut Scip,
    sol: crate::types::SolId,
    printreason: bool,
    mut completely: bool,
    checkbounds: bool,
    checkintegrality: bool,
    checklprows: bool,
) -> crate::retcode::ScipResult<bool> {
    use crate::{
        prob::{ORIGPROB, TRANSPROB},
        scip::{GLOBALEVENTFILTER, ORIGPRIMAL, TRANSPRIMAL},
    };

    let bestsol = get_best_sol(scip);

    if !printreason {
        completely = false;
    }

    let stored;
    if crate::sol::sol_is_original(scip, sol) {
        let feasible = crate::sol::sol_check_orig(scip, sol, ORIGPROB, ORIGPRIMAL, printreason, completely, checkbounds, checkintegrality, checklprows, true)?;

        if feasible {
            stored = crate::primal::primal_add_sol_free(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER, sol)?;

            if stored && bestsol != get_best_sol(scip) {
                store_solution_gap(scip);
            }
        } else {
            crate::sol::sol_free(scip, sol, TRANSPRIMAL)?;
            stored = false;
        }
    } else {
        let f = crate::primal::CheckFlags {
            printreason,
            completely,
            checkbounds,
            checkintegrality,
            checklprows,
        };
        stored = crate::primal::primal_try_sol_free(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER, sol, f)?;

        if stored && bestsol != get_best_sol(scip) {
            store_solution_gap(scip);
        }
    }

    Ok(stored)
}

/// `SCIPaddSolFree` (`scip_sol.c`): adds primal solution to solution
/// storage, frees the solution afterwards; returns whether it was good
/// enough to keep.
pub fn add_sol_free(scip: &mut Scip, sol: crate::types::SolId) -> crate::retcode::ScipResult<bool> {
    use crate::{
        prob::{ORIGPROB, TRANSPROB},
        scip::{GLOBALEVENTFILTER, ORIGPRIMAL, TRANSPRIMAL},
    };

    match scip.set.stage {
        Stage::Problem | Stage::FreeTrans => crate::primal::primal_add_orig_sol_free(scip, ORIGPRIMAL, ORIGPROB, sol),
        Stage::Transformed | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::Solving => {
            let bestsol = get_best_sol(scip);

            let stored = crate::primal::primal_add_sol_free(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER, sol)?;

            if stored && bestsol != get_best_sol(scip) {
                store_solution_gap(scip);
            }

            Ok(stored)
        }
        stage => {
            log::error!("invalid SCIP stage <{}>", stage as i32);
            Err(crate::retcode::Retcode::InvalidCall)
        }
    }
}

/// `SCIPrecomputeSolObj` (`scip_sol.c`): recomputes the objective value of
/// an original solution, e.g., when transferring solutions from the solution
/// pool (objective coefficients might have changed in the meantime).
pub fn recompute_sol_obj(scip: &mut Scip, sol: crate::types::SolId) {
    crate::sol::sol_recompute_obj(scip, sol, crate::prob::ORIGPROB);
}

/// `SCIPtransformObj` (`scip_sol.c`): maps original space objective value
/// into transformed objective value.
pub fn transform_obj(scip: &Scip, obj: f64) -> f64 {
    crate::prob::prob_intern_objval(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, obj)
}

/// `SCIPsetObjlimit` (`scip_prob.c`): sets limit on objective function,
/// such that only solutions better than this limit are accepted.
pub fn set_objlimit(scip: &mut Scip, objlimit: f64) -> crate::retcode::ScipResult {
    use crate::{
        prob::{ORIGPROB, TRANSPROB, prob_get_objlim, prob_intern_objval, prob_set_objlim},
        retcode::Retcode,
        scip::{GLOBALEVENTFILTER, TRANSPRIMAL},
    };

    match scip.set.stage {
        Stage::Problem => prob_set_objlim(scip, ORIGPROB, objlimit),
        Stage::Presolved | Stage::Transformed | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Solving => {
            let oldobjlimit = prob_get_objlim(scip, ORIGPROB);
            let relaxes = transform_obj(scip, objlimit) > prob_intern_objval(scip, TRANSPROB, ORIGPROB, oldobjlimit);
            if relaxes && (scip.set.stage != Stage::Presolved || !scip.set.reopt_enable) {
                // Cannot relax objective limit.
                return Err(Retcode::InvalidData);
            }
            prob_set_objlim(scip, ORIGPROB, objlimit);
            prob_set_objlim(scip, TRANSPROB, objlimit);
            crate::primal::primal_update_objlimit(scip, TRANSPRIMAL, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;
        }
        _ => {
            // Method is not callable in this SCIP stage.
            return Err(Retcode::InvalidCall);
        }
    }

    Ok(())
}

/// `SCIPinterruptLP` (`scip_lp.c`): interrupts or disables the interrupt of
/// the currently ongoing lp solve.
pub fn interrupt_lp(scip: &mut Scip, interrupt: bool) -> crate::retcode::ScipResult {
    if scip.lp.is_none() {
        return Ok(());
    }

    crate::lp::lp_interrupt(scip, interrupt)?;
    if interrupt {
        scip.stat.userinterrupt = true;
    }

    Ok(())
}

/// `SCIPgetNConss` (`scip_prob.c`): gets total number of globally valid
/// constraints currently in the problem.
pub fn get_n_conss(scip: &Scip) -> i32 {
    match scip.set.stage {
        Stage::Problem => scip.probs[crate::prob::ORIGPROB].nconss,
        Stage::Transformed | Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::InitSolve | Stage::Solving | Stage::Solved => {
            scip.probs[crate::prob::TRANSPROB].nconss
        }
        _ => panic!("invalid SCIP stage <{:?}>", scip.set.stage),
    }
}

/// `SCIPclearRelaxSolVals` (`scip_var.c`): sets the relaxation solution
/// values of all variables to zero.
pub fn clear_relax_sol_vals(scip: &mut Scip, relax: crate::types::RelaxId) -> crate::retcode::ScipResult {
    crate::relax::relaxation_set_sol_relax(scip, relax);

    if crate::relax::relaxation_is_sol_zero(scip) {
        return Ok(());
    }

    let vars = get_vars(scip).to_vec();
    for var in vars {
        crate::var::var_set_relax_sol(scip, var, 0.0, false)?;
    }

    crate::relax::relaxation_set_sol_obj(scip, 0.0);
    crate::relax::relaxation_set_sol_zero(scip, true);

    Ok(())
}

/// `SCIPgetLPSolstat` (`scip_lp.c`): gets solution status of current LP.
pub fn get_lp_solstat(scip: &Scip) -> crate::types::LpSolStat {
    if crate::tree::tree_is_focus_node_lp_constructed(scip) {
        crate::lp::lp_get_solstat(scip)
    } else {
        crate::types::LpSolStat::NotSolved
    }
}

/// `SCIPbranchLP` (`scip_branch.c`): calls branching rules to branch on an
/// LP solution; if no fractional variables exist, the result is
/// SCIP_DIDNOTRUN.
pub fn branch_lp(scip: &mut Scip) -> crate::retcode::ScipResult<crate::types::ScipResultCode> {
    let cutoffbound = scip.primals[TRANSPRIMAL].cutoffbound;
    crate::branch::branch_exec_lp(
        scip,
        crate::prob::TRANSPROB,
        crate::prob::ORIGPROB,
        crate::scip::GLOBALEVENTFILTER,
        crate::sepastore::SEPASTORE,
        cutoffbound,
        true,
    )
}

/// `SCIPbranchExtern` (`scip_branch.c`): calls branching rules to branch on
/// a external candidates; if no such candidates exist, the result is
/// SCIP_DIDNOTRUN.
pub fn branch_extern(scip: &mut Scip) -> crate::retcode::ScipResult<crate::types::ScipResultCode> {
    let cutoffbound = scip.primals[TRANSPRIMAL].cutoffbound;
    crate::branch::branch_exec_extern(
        scip,
        crate::prob::TRANSPROB,
        crate::prob::ORIGPROB,
        crate::scip::GLOBALEVENTFILTER,
        crate::sepastore::SEPASTORE,
        cutoffbound,
        true,
    )
}

/// `SCIPaddExternBranchCand` (`scip_branch.c`): insert variable, its score
/// and its solution value into the external branching candidate storage.
pub fn add_extern_branch_cand(scip: &mut Scip, var: crate::types::VarId, score: f64, solval: f64) {
    crate::branch::branchcand_add_extern_cand(scip, var, score, solval);
}

/// The problem a solution's values refer to (`SCIPgetSolVarsData`'s choice
/// of `origprob` or `transprob`).
pub fn sol_vars_prob(scip: &Scip, sol: crate::types::SolId) -> usize {
    if scip.set.stage == Stage::Problem || (sol != crate::var::NONE && crate::sol::sol_is_original(scip, sol)) {
        crate::prob::ORIGPROB
    } else {
        crate::prob::TRANSPROB
    }
}

/// `SCIPupdateSolIntegralityViolation` (`scip_sol.c`): update integrality
/// violation of a solution.
pub fn update_sol_integrality_violation(scip: &mut Scip, sol: crate::types::SolId, absviol: f64) {
    if scip.primals[crate::scip::ORIGPRIMAL].updateviolations {
        crate::sol::sol_update_integrality_violation(scip, sol, absviol);
    }
}

/// `SCIPaddDiveBoundChange` (`scip_heuristics.c`): stores the candidate
/// for the next dive bound change.
pub fn add_dive_bound_change(scip: &mut Scip, var: crate::types::VarId, dir: crate::types::BranchDir, value: f64, preferred: bool) {
    crate::tree::tree_add_dive_bound_change(scip, var, dir, value, preferred);
}

/// `SCIPincludeNodeselBasic` (`scip_nodesel.c`): creates a node selector
/// and includes it in SCIP with its most fundamental callbacks.
#[allow(clippy::too_many_arguments)]
pub fn include_nodesel_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    stdpriority: i32,
    memsavepriority: i32,
    nodeselselect: crate::nodesel::NodeselSelect,
    nodeselcomp: crate::nodesel::NodeselComp,
    nodeseldata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::NodeselId> {
    if scip.set.nodesels.0[..scip.set.nnodesels as usize].iter().any(|&n| scip.nodesels[n].name == name) {
        // Node selector already included.
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = crate::nodesel::NodeselMethods {
        nodeselselect: Some(nodeselselect),
        nodeselcomp: Some(nodeselcomp),
        ..Default::default()
    };
    let nodesel = crate::nodesel::nodesel_create(scip, name, desc, stdpriority, memsavepriority, methods, nodeseldata)?;
    crate::set::set_include_nodesel(scip, nodesel);

    Ok(nodesel)
}

/// `SCIPgetPrioChild` (`scip_tree.c`): gets the best child of the focus
/// node w.r.t. the node selection priority assigned by the branching rule.
pub fn get_prio_child(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_prio_child(scip)
}

/// `SCIPgetBestChild` (`scip_tree.c`): gets the best child of the focus
/// node w.r.t. the node selection strategy.
pub fn get_best_child(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_best_child(scip)
}

/// `SCIPgetPrioSibling` (`scip_tree.c`): gets the best sibling of the focus
/// node w.r.t. the node selection priority assigned by the branching rule.
pub fn get_prio_sibling(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_prio_sibling(scip)
}

/// `SCIPgetBestSibling` (`scip_tree.c`): gets the best sibling of the focus
/// node w.r.t. the node selection strategy.
pub fn get_best_sibling(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_best_sibling(scip)
}

/// `SCIPgetBestNode` (`scip_tree.c`): gets the best node from the tree
/// (child, sibling, or leaf) w.r.t. the node selection strategy.
pub fn get_best_node(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_best_node(scip)
}

/// `SCIPgetBestboundNode` (`scip_tree.c`): gets the node with smallest
/// lower bound from the tree (child, sibling, or leaf).
pub fn get_bestbound_node(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_lowerbound_node(scip)
}

/// `SCIPgetPlungeDepth` (`scip_tree.c`): gets current plunging depth
/// (successive times, a child was selected as next node).
pub fn get_plunge_depth(scip: &Scip) -> i32 {
    scip.stat.plungedepth
}

/// `SCIPgetMaxDepth` (`scip_solvingstats.c`): gets maximal depth of all
/// processed nodes in current branch and bound run (excluding probing
/// nodes).
pub fn get_max_depth(scip: &Scip) -> i32 {
    scip.stat.maxdepth
}

/// `SCIPgetNNodes` (`scip_solvingstats.c`): gets number of processed nodes
/// in current run, including the focus node.
pub fn get_n_nodes(scip: &Scip) -> i64 {
    scip.stat.nnodes
}

/// `SCIPgetNStrongbranchLPIterations` (`scip_solvingstats.c`): gets total
/// number of simplex iterations used so far in strong branching.
pub fn get_n_strongbranch_lp_iterations(scip: &Scip) -> i64 {
    scip.stat.nsblpiterations
}

/// `SCIPgetNNodeLPIterations` (`scip_solvingstats.c`): gets total number
/// of iterations used so far for node relaxations.
pub fn get_n_node_lp_iterations(scip: &Scip) -> i64 {
    scip.stat.nnodelpiterations
}

/// `SCIPgetNSolsFound` (`scip_solvingstats.c`): gets number of feasible
/// primal solutions found so far.
pub fn get_n_sols_found(scip: &Scip) -> i64 {
    scip.primals[TRANSPRIMAL].nsolsfound
}

/// `SCIPincludeBranchruleBasic` (`scip_branch.c`): creates a branching rule
/// and includes it in SCIP; the execution methods are set by the caller.
#[allow(clippy::too_many_arguments)]
pub fn include_branchrule_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    maxdepth: i32,
    maxbounddist: f64,
    methods: crate::branch::BranchruleMethods,
    branchruledata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::BranchruleId> {
    if scip.set.branchrules.0[..scip.set.nbranchrules as usize].iter().any(|&b| scip.branchrules[b].name == name) {
        // Branching rule already included.
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let branchrule = crate::branch::branchrule_create(scip, name, desc, priority, maxdepth, maxbounddist, methods, branchruledata)?;
    crate::set::set_include_branchrule(scip, branchrule);

    Ok(branchrule)
}

/// `(lpcands, lpcandssol, lpcandsfrac, nlpcands, npriolpcands,
/// nfracimplvars)`, as [`get_lp_branch_cands`] returns them.
pub type LpBranchCands = (Vec<crate::types::VarId>, Vec<f64>, Vec<f64>, i32, i32, i32);

/// `SCIPgetLPBranchCands` (`scip_branch.c`): gets branching candidates for
/// LP solution branching (fractional variables).
pub fn get_lp_branch_cands(scip: &mut Scip) -> crate::retcode::ScipResult<LpBranchCands> {
    use crate::types::LpSolStat;
    let solstat = crate::lp::lp_get_solstat(scip);
    if solstat != LpSolStat::Optimal && solstat != LpSolStat::UnboundedRay {
        // LP not solved to optimality.
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let c = crate::branch::branchcand_get_lp_cands(scip);
    let b = &scip.branchcand;
    // The arrays also hold the fractional implied integer variables behind
    // the candidates, as in C.
    let n = (c.nlpcands + c.nfracimplvars) as usize;
    Ok((
        b.lpcands.0[..n].to_vec(),
        b.lpcandssol.0[..n].to_vec(),
        b.lpcandsfrac.0[..n].to_vec(),
        c.nlpcands,
        c.npriolpcands,
        c.nfracimplvars,
    ))
}

/// `SCIPgetExternBranchCands` (`scip_branch.c`): gets external branching
/// candidates as `(externcands, externcandssol, externcandsscore)`.
pub fn get_extern_branch_cands(scip: &Scip) -> (Vec<crate::types::VarId>, Vec<f64>, Vec<f64>) {
    let b = &scip.branchcand;
    let n = b.nexterncands as usize;
    (b.externcands.0[..n].to_vec(), b.externcandssol.0[..n].to_vec(), b.externcandsscore.0[..n].to_vec())
}

/// `SCIPbranchVar` (`scip_branch.c`): branches on a non-continuous
/// variable v.
pub fn branch_var(scip: &mut Scip, var: crate::types::VarId) -> crate::retcode::ScipResult<crate::tree::BranchChildren> {
    if !crate::var::var_is_integral(scip, var) {
        // Cannot branch on continuous variable.
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let x = &scip.vars[var];
    if scip.set.is_eq(x.locdom.lb, x.locdom.ub) {
        // Cannot branch on variable with fixed domain.
        return Err(crate::retcode::Retcode::InvalidData);
    }

    crate::tree::tree_branch_var(
        scip,
        crate::prob::TRANSPROB,
        crate::prob::ORIGPROB,
        crate::scip::GLOBALEVENTFILTER,
        var,
        crate::def::INVALID,
    )
}

/// `SCIPbranchVarVal` (`scip_branch.c`): branches on a variable v; if
/// solution value x' is fractional, two child nodes will be created (x <=
/// floor(x'), x >= ceil(x')), if solution value is integral, three child
/// nodes will be created (x <= x'-1, x == x', x >= x'+1); if solution value
/// is equal to one of the bounds and the other bound is infinite, only two
/// child nodes will be created (the left one is x <= x' and the right one is
/// x >= x'+1).
pub fn branch_var_val(scip: &mut Scip, var: crate::types::VarId, val: f64) -> crate::retcode::ScipResult<crate::tree::BranchChildren> {
    let x = &scip.vars[var];
    if scip.set.is_eq(x.locdom.lb, x.locdom.ub) {
        // Cannot branch on variable with fixed domain.
        return Err(crate::retcode::Retcode::InvalidData);
    }

    crate::tree::tree_branch_var(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, crate::scip::GLOBALEVENTFILTER, var, val)
}

/// `SCIPgetBranchingPoint` (`scip_branch.c`): calculates the branching
/// point for a continuous or discrete variable.
pub fn get_branching_point(scip: &Scip, var: crate::types::VarId, suggestion: f64) -> f64 {
    crate::branch::branch_get_branching_point(scip, var, suggestion)
}

/// `SCIPaddConflictBd` (`scip_conflict.c`): adds lower or upper bound of
/// variable at the time of the given bound change index to the conflict
/// analysis' candidate storage.
pub fn add_conflict_bd(scip: &mut Scip, var: crate::types::VarId, boundtype: crate::types::BoundType, bdchgidx: Option<&crate::var::BdChgIdx>) -> crate::retcode::ScipResult {
    crate::conflict::conflict_add_bound_public(scip, var, boundtype, bdchgidx)
}

/// `SCIPcalcNodeselPriority` (`scip_branch.c`): calculates the node
/// selection priority for moving the given variable's value in the given
/// direction.
pub fn calc_nodesel_priority(scip: &Scip, var: crate::types::VarId, branchdir: crate::types::BranchDir, targetvalue: f64) -> f64 {
    crate::tree::tree_calc_nodesel_priority(scip, var, branchdir, targetvalue)
}

/// `SCIPcalcChildEstimate` (`scip_branch.c`): calculates an estimate for the
/// objective of the best feasible solution contained in the subtree after
/// applying the given branching.
pub fn calc_child_estimate(scip: &Scip, var: crate::types::VarId, targetvalue: f64) -> f64 {
    crate::tree::tree_calc_child_estimate(scip, var, targetvalue)
}

/// `SCIPcreateChild` (`scip_branch.c`): creates a child node of the focus
/// node.
pub fn create_child(scip: &mut Scip, nodeselprio: f64, estimate: f64) -> crate::retcode::ScipResult<crate::types::NodeId> {
    crate::tree::node_create_child(scip, nodeselprio, estimate)
}

/// `SCIPaddConflictRelaxedLb` (`scip_conflict.c`): adds lower bound of
/// variable at the time of the given bound change index to the conflict
/// analysis' candidate storage, relaxed to `relaxedlb` where possible.
pub fn add_conflict_relaxed_lb(scip: &mut Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>, relaxedlb: f64) -> crate::retcode::ScipResult {
    crate::conflict::conflict_add_relaxed_bound(scip, var, crate::types::BoundType::Lower, bdchgidx, relaxedlb)
}

/// `SCIPaddConflictRelaxedUb` (`scip_conflict.c`): adds upper bound of
/// variable at the time of the given bound change index to the conflict
/// analysis' candidate storage, relaxed to `relaxedub` where possible.
pub fn add_conflict_relaxed_ub(scip: &mut Scip, var: crate::types::VarId, bdchgidx: Option<&crate::var::BdChgIdx>, relaxedub: f64) -> crate::retcode::ScipResult {
    crate::conflict::conflict_add_relaxed_bound(scip, var, crate::types::BoundType::Upper, bdchgidx, relaxedub)
}

/// `SCIPgetConflictVarLb` (`scip_conflict.c`): the conflict lower bound if
/// the variable is present in the current conflict set, otherwise the
/// global lower bound.
pub fn get_conflict_var_lb(scip: &Scip, var: crate::types::VarId) -> f64 {
    crate::conflict::conflict_get_var_lb(scip, var)
}

/// `SCIPgetConflictVarUb` (`scip_conflict.c`): the conflict upper bound if
/// the variable is present in the current conflict set, otherwise the
/// global upper bound.
pub fn get_conflict_var_ub(scip: &Scip, var: crate::types::VarId) -> f64 {
    crate::conflict::conflict_get_var_ub(scip, var)
}

/// `SCIPincludePresolBasic` (`scip_presol.c`): creates a presolver and
/// includes it in SCIP with its most fundamental callbacks.
pub fn include_presol_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    maxrounds: i32,
    timing: crate::cons::PresolTiming,
    presolexec: crate::presol::PresolExec,
    presoldata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::PresolId> {
    // Check whether presolver is already present.
    if crate::set::set_find_presol(scip, name) != crate::var::NONE {
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = crate::presol::PresolMethods {
        presolexec: Some(presolexec),
        ..Default::default()
    };
    let p = crate::presol::presol_create(scip, name, desc, priority, maxrounds, timing, methods, presoldata)?;
    crate::set::set_include_presol(scip, p);

    Ok(p)
}

/// `SCIPincludePropBasic` (`scip_prop.c`): creates a propagator and
/// includes it in SCIP with its most fundamental callbacks.
#[allow(clippy::too_many_arguments)]
pub fn include_prop_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    freq: i32,
    delay: bool,
    timingmask: crate::cons::PropTiming,
    propexec: crate::prop::PropExec,
    propdata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::PropId> {
    // Check whether propagator is already present.
    if crate::set::set_find_prop(scip, name) != crate::var::NONE {
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = crate::prop::PropMethods {
        propexec: Some(propexec),
        ..Default::default()
    };
    let p = crate::prop::prop_create(
        scip,
        name,
        desc,
        priority,
        freq,
        delay,
        timingmask,
        0,
        -1,
        crate::cons::PRESOLTIMING_ALWAYS,
        methods,
        propdata,
    )?;
    crate::set::set_include_prop(scip, p);

    Ok(p)
}

/// `SCIPcutoffNode` (`scip_tree.c`): cuts off node and whole sub tree from
/// branch and bound tree.
pub fn cutoff_node(scip: &mut Scip, node: crate::types::NodeId) -> crate::retcode::ScipResult {
    crate::tree::node_cutoff(scip, node, crate::scip::GLOBALEVENTFILTER, crate::prob::TRANSPROB, crate::prob::ORIGPROB)
}

/// `SCIPgetRootNode` (`scip_tree.c`): gets the root node of the tree.
pub fn get_root_node(scip: &Scip) -> crate::types::NodeId {
    scip.tree.as_ref().unwrap().root
}

/// `SCIPgetLPRootObjval` (`scip_lp.c`): gets objective value of root LP.
pub fn get_lp_root_objval(scip: &Scip) -> f64 {
    crate::lp::lp_get_root_objval(scip)
}

/// `SCIPgetNActivePricers` (`scip_pricer.c`): returns the number of
/// currently active variable pricers.
pub fn get_n_active_pricers(scip: &Scip) -> i32 {
    scip.set.nactivepricers
}

/// `SCIPhasCurrentNodeLP` (`scip_lp.c`): returns whether the LP was or is
/// to be solved in the current node.
pub fn has_current_node_lp(scip: &Scip) -> bool {
    crate::tree::tree_has_current_node_lp(scip)
}

/// `SCIPisLPRelax` (`scip_lp.c`): returns whether the current LP is a
/// relaxation of the current problem and its optimal objective value is a
/// local lower bound.
pub fn is_lp_relax(scip: &Scip) -> bool {
    crate::lp::lp_is_relax(scip)
}

/// `SCIPisLPSolBasic` (`scip_lp.c`): returns whether the current LP solution
/// is basic, i.e. is defined by a valid simplex basis.
pub fn is_lp_sol_basic(scip: &Scip) -> bool {
    crate::lp::lp_is_sol_basic(scip)
}

/// `SCIPgetNPseudoBranchCands` (`scip_branch.c`): gets number of branching
/// candidates for pseudo solution branching (non-fixed variables).
pub fn get_n_pseudo_branch_cands(scip: &Scip) -> i32 {
    crate::branch::branchcand_get_n_pseudo_cands(scip)
}

/// `SCIPgetLPObjval` (`scip_lp.c`): gets objective value of current LP
/// (which is the sum of column and loose objective value).
pub fn get_lp_objval(scip: &mut Scip) -> f64 {
    crate::lp::lp_get_objval(scip, crate::prob::TRANSPROB)
}

/// `SCIPgetNLPRows` (`scip_lp.c`): gets the current number of LP rows (0 if
/// the focus node's LP is not constructed).
pub fn get_n_lp_rows(scip: &Scip) -> i32 {
    if crate::tree::tree_is_focus_node_lp_constructed(scip) {
        scip.lp.as_ref().unwrap().nrows
    } else {
        0
    }
}

/// `SCIPisNLPConstructed` (`scip_nlp.c`): returns whether the NLP relaxation
/// has been constructed (`scip->nlp != NULL`).
pub fn is_nlp_constructed(scip: &Scip) -> bool {
    crate::nlp::is_nlp_constructed(scip)
}

/// `SCIPgetLPCols` (`scip_lp.c`): gets current LP columns (empty if the
/// focus node's LP is not constructed).
pub fn get_lp_cols(scip: &Scip) -> Vec<crate::types::ColId> {
    if crate::tree::tree_is_focus_node_lp_constructed(scip) {
        let lp = scip.lp.as_ref().unwrap();
        lp.cols.0[..lp.ncols as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `SCIPgetColRedcost` (`scip_lp.c`): gets the reduced costs of a column in
/// the last (feasible) LP.
pub fn get_col_redcost(scip: &mut Scip, col: crate::types::ColId) -> f64 {
    if !crate::tree::tree_has_current_node_lp(scip) {
        // Cannot get reduced costs, because node LP is not processed.
        return 0.0;
    }
    crate::lp::col_get_redcost(scip, col)
}

/// `SCIPgetVarRedcost` (`scip_var.c`): returns the reduced costs of the
/// variable in the current node's LP relaxation.
pub fn get_var_redcost(scip: &mut Scip, var: crate::types::VarId) -> f64 {
    use crate::types::VarStatus;
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == crate::var::NONE {
                crate::def::INVALID
            } else {
                get_var_redcost(scip, transvar)
            }
        }
        VarStatus::Column => {
            let col = scip.vars[var].data.col;
            get_col_redcost(scip, col)
        }
        VarStatus::Loose => crate::def::INVALID,
        VarStatus::Fixed | VarStatus::Aggregated | VarStatus::MultAggr | VarStatus::Negated => 0.0,
    }
}

/// `SCIPgetVarImplRedcost` (`scip_var.c`): returns the implied reduced costs
/// of the variable in the current node's LP relaxation.
pub fn get_var_impl_redcost(scip: &mut Scip, var: crate::types::VarId, varfixing: bool) -> f64 {
    use crate::types::VarStatus;
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == crate::var::NONE {
                crate::def::INVALID
            } else {
                get_var_impl_redcost(scip, transvar, varfixing)
            }
        }
        VarStatus::Column => crate::var::var_get_impl_redcost(scip, var, varfixing, crate::prob::TRANSPROB),
        VarStatus::Loose => crate::def::INVALID,
        VarStatus::Fixed | VarStatus::Aggregated | VarStatus::MultAggr | VarStatus::Negated => 0.0,
    }
}

/// `SCIPisNLPEnabled` (`scip_nlp.c`): returns whether the NLP relaxation has
/// been enabled.
pub fn is_nlp_enabled(scip: &Scip) -> bool {
    scip.probs[crate::prob::TRANSPROB].nlpenabled
}

/// `SCIPgetGlobalPseudoObjval` (`scip_lp.c`): gets the global pseudo
/// objective value; that is all variables set to their best (w.r.t. the
/// objective function) global bound.
pub fn get_global_pseudo_objval(scip: &mut Scip) -> f64 {
    crate::lp::lp_get_global_pseudo_objval(scip, crate::prob::TRANSPROB)
}

/// `SCIPgetPseudoObjval` (`scip_lp.c`): gets the pseudo objective value for
/// the current search node; that is all variables set to their best (w.r.t.
/// the objective function) local bound.
pub fn get_pseudo_objval(scip: &mut Scip) -> f64 {
    crate::lp::lp_get_pseudo_objval(scip, crate::prob::TRANSPROB)
}

/// `SCIPanalyzeConflict` (`scip_conflict.c`): analyzes conflict bounds that
/// were added after a call to `SCIPinitConflictAnalysis()`; on success,
/// calls the conflict handlers to create a conflict constraint out of the
/// resulting conflict set; returns whether a conflict constraint was
/// created.
pub fn analyze_conflict(scip: &mut Scip, validdepth: i32) -> crate::retcode::ScipResult<bool> {
    if scip.set.conf_usegenres {
        unimplemented!("generalized resolution conflict analysis");
    }

    crate::conflict::conflict_analyze_public(scip, crate::prob::TRANSPROB, validdepth)
}

/// `SCIPgetAvgConflictScore`: gets the average conflict score value over
/// all variables.
pub fn get_avg_conflict_score(scip: &Scip) -> f64 {
    use crate::types::BranchDir;
    let scale = scip.probs[crate::prob::TRANSPROB].nvars as f64 * scip.stat.vsidsweight;
    let conflictscoredown = scip.stat.glbhistory.get_vsids(BranchDir::Downwards) / scale;
    let conflictscoreup = scip.stat.glbhistory.get_vsids(BranchDir::Upwards) / scale;

    crate::branch::branch_get_score(scip, crate::var::NONE, conflictscoredown, conflictscoreup)
}

/// `SCIPgetAvgConflictlengthScore`: gets the average conflictlength score
/// value over all variables.
pub fn get_avg_conflictlength_score(scip: &Scip) -> f64 {
    use crate::types::BranchDir;
    let conflictlengthdown = scip.stat.glbhistory.get_avg_conflictlength(BranchDir::Downwards);
    let conflictlengthup = scip.stat.glbhistory.get_avg_conflictlength(BranchDir::Upwards);

    crate::branch::branch_get_score(scip, crate::var::NONE, conflictlengthdown, conflictlengthup)
}

/// `SCIPgetAvgInferenceScore`: gets the average inference score value over
/// all variables.
pub fn get_avg_inference_score(scip: &Scip) -> f64 {
    use crate::types::BranchDir;
    let inferencesdown = scip.stat.glbhistory.get_avg_inferences(BranchDir::Downwards);
    let inferencesup = scip.stat.glbhistory.get_avg_inferences(BranchDir::Upwards);

    crate::branch::branch_get_score(scip, crate::var::NONE, inferencesdown, inferencesup)
}

/// `SCIPgetAvgCutoffScore`: gets the average cutoff score value over all
/// variables.
pub fn get_avg_cutoff_score(scip: &Scip) -> f64 {
    use crate::types::BranchDir;
    let cutoffsdown = scip.stat.glbhistory.get_avg_cutoffs(BranchDir::Downwards);
    let cutoffsup = scip.stat.glbhistory.get_avg_cutoffs(BranchDir::Upwards);

    crate::branch::branch_get_score(scip, crate::var::NONE, cutoffsdown, cutoffsup)
}

/// `SCIPgetAvgPseudocostScore`: gets the average pseudo cost score value
/// over all variables, assuming a fractionality of 0.5.
pub fn get_avg_pseudocost_score(scip: &Scip) -> f64 {
    let pscostdown = scip.stat.glbhistory.get_pseudocost(-0.5);
    let pscostup = scip.stat.glbhistory.get_pseudocost(0.5);

    crate::branch::branch_get_score(scip, crate::var::NONE, pscostdown, pscostup)
}

/// `SCIPgetAvgDPseudocostScore`: gets the average discounted pseudo cost
/// score value over all variables, assuming a fractionality of 0.5.
pub fn get_avg_dpseudocost_score(scip: &Scip, discountfac: f64) -> f64 {
    let h = &scip.stat.glbhistory;
    let mut pscostdown = h.get_pseudocost(-0.5) + discountfac * h.get_anc_pseudocost(-0.5);
    let mut pscostup = h.get_pseudocost(0.5) + discountfac * h.get_anc_pseudocost(0.5);
    pscostdown /= 1.0 + discountfac;
    pscostup /= 1.0 + discountfac;

    crate::branch::branch_get_score(scip, crate::var::NONE, pscostdown, pscostup)
}

/// `SCIPincludeSepaBasic`: creates a separator and includes it in SCIP
/// with its fundamental callbacks; the others are set on the returned
/// separator.
#[allow(clippy::too_many_arguments)]
pub fn include_sepa_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    freq: i32,
    maxbounddist: f64,
    usessubscip: bool,
    delay: bool,
    sepaexeclp: Option<crate::sepa::SepaExecLp>,
    sepaexecsol: Option<crate::sepa::SepaExecSol>,
    sepadata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::SepaId> {
    // Check whether separator is already present.
    if scip.set.sepas.0[..scip.set.nsepas as usize].iter().any(|&s| scip.sepas[s].name == name) {
        log::error!("separator <{name}> already included.");
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = crate::sepa::SepaMethods {
        sepaexeclp,
        sepaexecsol,
        ..Default::default()
    };
    let sepa = crate::sepa::sepa_create(scip, name, desc, priority, freq, maxbounddist, usessubscip, delay, methods, sepadata)?;
    crate::set::set_include_sepa(scip, sepa);
    Ok(sepa)
}

/// `SCIPaddPoolCut`: if not already existing, adds row to global cut pool.
pub fn add_pool_cut(scip: &mut Scip, row: crate::types::RowId) -> crate::retcode::ScipResult {
    crate::cutpool::cutpool_add_row(scip, crate::cutpool::GLOBALCUTPOOL, row)
}

/// `SCIPgetVarSols`: gets solution values of variables in current
/// LP/pseudo solution.
pub fn get_var_sols(scip: &Scip, vars: &[crate::types::VarId], vals: &mut [f64]) {
    if crate::tree::tree_has_current_node_lp(scip) {
        for (v, &var) in vars.iter().enumerate() {
            vals[v] = crate::var::var_get_lp_sol(scip, var);
        }
    } else {
        for (v, &var) in vars.iter().enumerate() {
            vals[v] = crate::var::var_get_pseudo_sol(scip, var);
        }
    }
}

/// `SCIPgetSolVals`: gets values of multiple variables in primal CIP
/// solution, or in current LP/pseudo solution (`sol` is `NONE`).
pub fn get_sol_vals(scip: &Scip, sol: crate::types::SolId, vars: &[crate::types::VarId], vals: &mut [f64]) {
    if sol != crate::var::NONE {
        for (v, &var) in vars.iter().enumerate() {
            vals[v] = crate::sol::sol_get_val(scip, sol, var);
        }
    } else {
        get_var_sols(scip, vars, vals);
    }
}

/// `SCIPgetVectorEfficacyNorm`: calculates the efficacy norm of the given
/// vector, which depends on the "separating/efficacynorm" parameter.
pub fn get_vector_efficacy_norm(scip: &Scip, vals: &[f64]) -> f64 {
    let mut norm = 0.0;
    match scip.set.sepa_efficacynorm {
        b'e' => {
            for &v in vals {
                norm += v * v;
            }
            norm = norm.sqrt();
        }
        b'm' => {
            for &v in vals {
                norm = crate::def::max(norm, v.abs());
            }
        }
        b's' => {
            for &v in vals {
                norm += v.abs();
            }
        }
        b'd' => {
            if vals.iter().any(|&v| !scip.set.is_zero(v)) {
                norm = 1.0;
            }
        }
        c => log::error!("invalid efficacy norm parameter '{}'", c as char),
    }
    norm
}

/// `SCIPincludeHeurBasic` (`scip_heur.c`): creates a primal heuristic with
/// its execution method and includes it in SCIP; the other callbacks are set
/// on the returned heuristic.
#[allow(clippy::too_many_arguments)]
pub fn include_heur_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    dispchar: u8,
    priority: i32,
    freq: i32,
    freqofs: i32,
    maxdepth: i32,
    timingmask: crate::heur::HeurTiming,
    usessubscip: bool,
    heurexec: crate::heur::HeurExec,
    heurdata: Option<Box<dyn std::any::Any>>,
) -> crate::retcode::ScipResult<crate::types::HeurId> {
    // Check whether heuristic is already present.
    if crate::set::set_find_heur(scip, name) != crate::var::NONE {
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = crate::heur::HeurMethods {
        heurexec: Some(heurexec),
        ..Default::default()
    };
    let heur = crate::heur::heur_create(scip, name, desc, dispchar, priority, freq, freqofs, maxdepth, timingmask, usessubscip, methods, heurdata)?;

    crate::set::set_include_heur(scip, heur);

    Ok(heur)
}

/// `SCIPsetSolVal` (`scip_sol.c`): sets value of variable in primal CIP
/// solution.
pub fn set_sol_val(scip: &mut Scip, sol: crate::types::SolId, var: crate::types::VarId, val: f64) -> crate::retcode::ScipResult {
    if crate::sol::sol_is_original(scip, sol) && crate::var::var_is_transformed(scip, var) {
        return Err(crate::retcode::Retcode::InvalidCall);
    }

    crate::sol::sol_set_val(scip, sol, var, val)
}

/// `SCIPtrySol` (`scip_sol.c`): checks solution for feasibility; if
/// possible, adds it to storage by copying; returns whether it was stored.
/// Partial solutions are not ported.
pub fn try_sol(
    scip: &mut Scip,
    sol: crate::types::SolId,
    printreason: bool,
    mut completely: bool,
    checkbounds: bool,
    checkintegrality: bool,
    checklprows: bool,
) -> crate::retcode::ScipResult<bool> {
    use crate::{
        prob::{ORIGPROB, TRANSPROB},
        scip::{GLOBALEVENTFILTER, ORIGPRIMAL, TRANSPRIMAL},
    };

    let bestsol = get_best_sol(scip);

    if !printreason {
        completely = false;
    }

    let stored;
    if crate::sol::sol_is_original(scip, sol) {
        // SCIPprimalTrySol() can only be called on transformed solutions;
        // therefore check solutions in original problem including modifiable
        // constraints.
        let feasible = crate::sol::sol_check_orig(scip, sol, ORIGPROB, ORIGPRIMAL, printreason, completely, checkbounds, checkintegrality, checklprows, true)?;
        if feasible {
            stored = crate::primal::primal_add_sol(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER, sol)?;

            if stored && bestsol != get_best_sol(scip) {
                store_solution_gap(scip);
            }
        } else {
            stored = false;
        }
    } else {
        let f = crate::primal::CheckFlags {
            printreason,
            completely,
            checkbounds,
            checkintegrality,
            checklprows,
        };
        stored = crate::primal::primal_try_sol(scip, TRANSPRIMAL, ORIGPROB, TRANSPROB, GLOBALEVENTFILTER, sol, f)?;

        if stored && bestsol != get_best_sol(scip) {
            store_solution_gap(scip);
        }
    }

    Ok(stored)
}

/// `SCIPgetSolTransObj` (`scip_sol.c`): returns transformed objective value
/// of primal CIP solution, or transformed current LP/pseudo objective value.
pub fn get_sol_trans_obj(scip: &mut Scip, sol: crate::types::SolId) -> f64 {
    use crate::prob::{ORIGPROB, TRANSPROB};
    if sol != crate::var::NONE {
        crate::sol::sol_get_obj(scip, sol, TRANSPROB, ORIGPROB)
    } else if crate::tree::tree_has_current_node_lp(scip) {
        crate::lp::lp_get_objval(scip, TRANSPROB)
    } else {
        crate::lp::lp_get_pseudo_objval(scip, TRANSPROB)
    }
}

/// `SCIPgetRowActivity` (`scip_lp.c`): returns the activity of a row in the
/// last LP or pseudo solution.
pub fn get_row_activity(scip: &mut Scip, row: crate::types::RowId) -> f64 {
    if crate::tree::tree_has_current_node_lp(scip) {
        crate::lp::row_get_lp_activity(scip, row)
    } else {
        crate::lp::row_get_pseudo_activity(scip, row)
    }
}

/// `SCIPconstructLP` (`scip_lp.c`): makes sure that the LP of the current
/// node is loaded and may be accessed through the LP information methods;
/// returns whether the node can be cut off.
pub fn construct_lp(scip: &mut Scip) -> crate::retcode::ScipResult<bool> {
    use crate::{
        prob::{ORIGPROB, TRANSPROB},
        scip::GLOBALEVENTFILTER,
    };
    crate::solve::construct_current_lp(scip, crate::sepastore::SEPASTORE, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER, false)
}

/// `SCIPflushLP` (`scip_lp.c`): makes sure that the LP of the current node
/// is flushed.
pub fn flush_lp(scip: &mut Scip) -> crate::retcode::ScipResult {
    crate::lp::lp_flush(scip, crate::prob::TRANSPROB)
}

/// `SCIPcreateSolCopy` (`scip_sol.c`) for a given source solution: creates
/// a copy of a primal solution.
pub fn create_sol_copy(scip: &mut Scip, sourcesol: crate::types::SolId) -> crate::retcode::ScipResult<crate::types::SolId> {
    crate::sol::sol_copy(scip, crate::scip::TRANSPRIMAL, sourcesol)
}

/// `SCIPgetNConflictConssFound` (`scip_solvingstats.c`): number of conflict
/// constraints found so far (generalized resolution is not ported, so it
/// contributes none).
pub fn get_n_conflict_conss_found(scip: &Scip) -> i64 {
    match scip.conflict.as_ref() {
        None => 0,
        Some(c) => {
            c.npropconfconss
                + c.npropreconvconss
                + c.ninflpconfconss
                + c.ninflpreconvconss
                + c.nboundlpconfconss
                + c.nboundlpreconvconss
                + c.nsbconfconss
                + c.nsbreconvconss
                + c.npseudoconfconss
                + c.npseudoreconvconss
                + c.ndualproofsbndglobal
                + c.ndualproofsinfglobal
        }
    }
}

/// `SCIPisObjIntegral` (`scip_prob.c`) in the transformed stages: whether
/// the objective value of every feasible solution is integral.
pub fn is_obj_integral(scip: &Scip) -> bool {
    scip.probs[crate::prob::TRANSPROB].objisintegral
}

/// `SCIPgetAvgLowerbound` (`scip_solvingstats.c`): average lower
/// (dual) bound of all unprocessed nodes in transformed problem.
pub fn get_avg_lowerbound(scip: &Scip) -> f64 {
    crate::tree::tree_get_avg_lowerbound(scip, scip.primals[TRANSPRIMAL].cutoffbound)
}

/// `SCIProundSol` (`scip_sol.c`): try to round given solution; returns
/// whether rounding was successful.
pub fn round_sol(scip: &mut Scip, sol: crate::types::SolId) -> crate::retcode::ScipResult<bool> {
    if crate::sol::sol_is_original(scip, sol) {
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    crate::sol::sol_round(scip, sol, crate::prob::TRANSPROB)
}

/// `SCIPunlinkSol` (`scip_sol.c`): stores solution values of variables in
/// solution's own array.
pub fn unlink_sol(scip: &mut Scip, sol: crate::types::SolId) -> crate::retcode::ScipResult {
    crate::sol::sol_unlink(scip, sol, crate::prob::TRANSPROB)
}

/// `SCIPgetDiveBoundChanges` (`scip_probing.c`): enforces a probing/diving
/// solution by suggesting bound changes that maximize the score w.r.t. the
/// current diving settings; returns `(success, infeasible)`.
pub fn get_dive_bound_changes(scip: &mut Scip, diveset: usize, sol: crate::types::SolId) -> crate::retcode::ScipResult<(bool, bool)> {
    let mut success = false;
    let mut infeasible = false;

    // We invalidate the previously stored bound changes.
    crate::tree::tree_clear_dive_bound_changes(scip);

    // Loop over constraint handlers until a constraint handler successfully
    // found a variable/value assignment for proceeding or a constraint
    // handler detected the infeasibility of the local node.
    let mut i = 0;
    while i < scip.set.nconshdlrs as usize && !(success || infeasible) {
        let h = scip.set.conshdlrs_enfo[i];
        (success, infeasible) = crate::cons::conshdlr_get_dive_bound_changes(scip, h, diveset, sol)?;
        i += 1;
    }

    Ok((success, infeasible))
}

/// `SCIPgetObjNorm` (`scip_prob.c`): returns the Euclidean norm of the
/// objective function vector (available only for transformed problem).
pub fn get_obj_norm(scip: &mut Scip) -> f64 {
    if scip.lp.as_ref().unwrap().objsqrnormunreliable {
        crate::lp::lp_recalculate_obj_sqr_norm(scip);
    }
    // SCIPlpGetObjNorm.
    scip.lp.as_ref().unwrap().objsqrnorm.sqrt()
}

/// `SCIPsetSolVals` (`scip_sol.c`): sets values of multiple variables in
/// primal CIP solution.
pub fn set_sol_vals(scip: &mut Scip, sol: crate::types::SolId, vars: &[crate::types::VarId], vals: &[f64]) -> crate::retcode::ScipResult {
    if crate::sol::sol_is_original(scip, sol) {
        for &var in vars {
            if crate::var::var_is_transformed(scip, var) {
                log::error!("cannot set value of transformed variable <{}> in original space solution", scip.vars[var].name);
                return Err(crate::retcode::Retcode::InvalidCall);
            }
        }
    }
    for (v, &var) in vars.iter().enumerate() {
        crate::sol::sol_set_val(scip, sol, var, vals[v])?;
    }
    Ok(())
}

/// `SCIPclearSol` (`scip_sol.c`): clears a primal solution.
pub fn clear_sol(scip: &mut Scip, sol: crate::types::SolId) -> crate::retcode::ScipResult {
    crate::sol::sol_clear(scip, sol);
    Ok(())
}

/// `SCIPcheckSol` (`scip_sol.c`): checks solution for feasibility without
/// adding it to the solution store.
pub fn check_sol(
    scip: &mut Scip,
    sol: crate::types::SolId,
    printreason: bool,
    mut completely: bool,
    checkbounds: bool,
    checkintegrality: bool,
    checklprows: bool,
) -> crate::retcode::ScipResult<bool> {
    // Return immediately if the solution is of type partial.
    if crate::sol::sol_is_partial(scip, sol) {
        log::error!("Cannot check feasibility of partial solutions.");
        return Err(crate::retcode::Retcode::InvalidData);
    }

    if !printreason {
        completely = false;
    }

    // SCIPsolCheck() can only be called on transformed solutions.
    if crate::sol::sol_is_original(scip, sol) {
        crate::sol::sol_check_orig(
            scip,
            sol,
            crate::prob::ORIGPROB,
            crate::scip::ORIGPRIMAL,
            printreason,
            completely,
            checkbounds,
            checkintegrality,
            checklprows,
            false,
        )
    } else {
        crate::sol::sol_check(scip, sol, crate::prob::TRANSPROB, printreason, completely, checkbounds, checkintegrality, checklprows)
    }
}

/// `SCIPgetSolvingTime` (`scip_timing.c`): gets the current solving time
/// in seconds.
pub fn get_solving_time(scip: &Scip) -> f64 {
    scip.stat.solvingtime.get_time()
}

/// `SCIPgetLPRows` (`scip_lp.c`): gets current LP rows (empty if the focus
/// node's LP is not constructed).
pub fn get_lp_rows(scip: &Scip) -> Vec<crate::types::RowId> {
    if crate::tree::tree_is_focus_node_lp_constructed(scip) {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `SCIPgetNLPs` (`scip_solvingstats.c`): gets total number of LPs solved
/// so far.
pub fn get_n_lps(scip: &Scip) -> i64 {
    scip.stat.nlps
}

/// `SCIPgetNCheckConss` (`scip_prob.c`): gets the number of checked
/// constraints over all constraint handlers.
pub fn get_n_check_conss(scip: &Scip) -> i32 {
    let mut ncheckconss = 0;
    for c in 0..scip.set.nconshdlrs {
        ncheckconss += scip.conshdlrs[scip.set.conshdlrs[c]].ncheckconss;
    }
    ncheckconss
}

/// `SCIPgetBestLeaf` (`scip_tree.c`): gets the best leaf from the node
/// queue w.r.t. the node selection strategy.
pub fn get_best_leaf(scip: &Scip) -> crate::types::NodeId {
    crate::tree::tree_get_best_leaf(scip)
}

/// `SCIPfindVar` (`scip_prob.c`): returns variable of given name in the
/// problem, or `NONE` if not existing.
pub fn find_var(scip: &Scip, name: &str) -> crate::types::VarId {
    match scip.set.stage {
        Stage::Problem => crate::prob::prob_find_var(scip, crate::prob::ORIGPROB, name),
        Stage::Transforming
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::Solved
        | Stage::ExitSolve
        | Stage::FreeTrans => {
            let var = crate::prob::prob_find_var(scip, crate::prob::TRANSPROB, name);
            if var == crate::var::NONE {
                crate::prob::prob_find_var(scip, crate::prob::ORIGPROB, name)
            } else {
                var
            }
        }
        _ => panic!("invalid SCIP stage <{:?}>", scip.set.stage),
    }
}

/// `SCIPparseReal`: extracts a value (including `+infinity`/`-infinity`)
/// from `s` at `pos`; returns it and the end position.
pub fn parse_real(scip: &Scip, s: &[u8], pos: usize) -> Option<(f64, usize)> {
    // Ignore white space.
    let localstr = crate::misc::skip_space(s, pos);

    // Test for a special infinity first.
    if s[localstr.min(s.len())..].starts_with(b"+infinity") {
        Some((scip.set.infinity(), localstr + 9))
    } else if s[localstr.min(s.len())..].starts_with(b"-infinity") {
        Some((-scip.set.infinity(), localstr + 9))
    } else {
        // Parse a finite value.
        crate::misc::str_to_real_value(s, localstr)
    }
}

/// `SCIPchgDualfeastol` (`scip_numerics.c`): sets the feasibility tolerance
/// for reduced costs.
pub fn chg_dualfeastol(scip: &mut Scip, dualfeastol: f64) -> crate::retcode::ScipResult {
    // Mark the LP unsolved, if the dual feasibility tolerance was tightened.
    if let Some(lp) = scip.lp.as_mut()
        && dualfeastol < scip.set.dualfeastol()
    {
        lp.solved = false;
        lp.lpsolstat = crate::types::LpSolStat::NotSolved;
    }

    // Change the settings (`SCIPsetSetDualfeastol`).
    scip.set.num_dualfeastol = dualfeastol;

    Ok(())
}

/// `SCIPchgRelaxfeastol` (`scip_numerics.c`, with `SCIPsetSetRelaxfeastol`):
/// sets the primal feasibility tolerance of relaxations; returns the old
/// one.
pub fn chg_relaxfeastol(scip: &mut Scip, relaxfeastol: f64) -> f64 {
    let oldval = scip.set.num_relaxfeastol;
    scip.set.num_relaxfeastol = relaxfeastol;
    oldval
}

/// `SCIPallColsInLP` (`scip_lp.c`): returns whether all columns of the
/// problem are in the LP.
pub fn all_cols_in_lp(scip: &Scip) -> bool {
    crate::prob::prob_all_cols_in_lp(scip, crate::prob::TRANSPROB)
}

/// `SCIPinRepropagation` (`scip_tree.c`).
pub fn in_repropagation(scip: &Scip) -> bool {
    crate::tree::tree_in_repropagation(scip)
}

/// `SCIPisStopped` (`scip_general.c`).
pub fn is_stopped(scip: &mut Scip) -> bool {
    crate::solve::solve_is_stopped(scip, false)
}

/// `SCIPgetNLPIterations` (`scip_solvingstats.c`).
pub fn get_n_lp_iterations(scip: &Scip) -> i64 {
    scip.stat.nlpiterations
}

/// `SCIPgetNRootLPIterations` (`scip_solvingstats.c`).
pub fn get_n_root_lp_iterations(scip: &Scip) -> i64 {
    scip.stat.nrootlpiterations
}
