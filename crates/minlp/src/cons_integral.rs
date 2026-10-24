//! Port of `cons_integral.c`: constraint handler for the integrality
//! constraint. Exact solving is not ported.

use crate::{
    api,
    cons::*,
    retcode::ScipResult,
    scip::Scip,
    types::{BranchDir, ConsId, ConshdlrId, LockType, LpSolStat, ScipResultCode, SolId},
};

pub const CONSHDLR_NAME: &str = "integral";
const CONSHDLR_DESC: &str = "integrality constraint";
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = 0;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = 0;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0
/// for first only.
const CONSHDLR_EAGERFREQ: i32 = -1;
/// Should the constraint handler be skipped, if no constraints are
/// available?
const CONSHDLR_NEEDSCONS: bool = false;

/*
 * Callback methods
 */

/// `consEnfolpIntegral`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_integral(scip: &mut Scip, _conshdlr: ConshdlrId, _conss: &[ConsId], _nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // If the root LP is unbounded, we want to terminate with UNBOUNDED or
    // INFORUNBOUNDED, depending on whether we are able to construct an integral
    // solution; in any case we do not want to branch.
    if api::get_lp_solstat(scip) == LpSolStat::UnboundedRay {
        *result = if api::get_n_lp_branch_cands(scip) == 0 {
            ScipResultCode::Feasible
        } else {
            ScipResultCode::Infeasible
        };
        return Ok(());
    }

    // Call branching methods.
    *result = api::branch_lp(scip)?;

    // If no branching was done, the LP solution was not fractional.
    if *result == ScipResultCode::DidNotRun {
        *result = ScipResultCode::Feasible;
    }

    Ok(())
}

/// `consEnforelaxIntegral`: constraint enforcing method of constraint
/// handler for relaxation solutions.
fn cons_enforelax_integral(
    scip: &mut Scip,
    sol: SolId,
    _conshdlr: ConshdlrId,
    _conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);
    let nintegers = nbinvars + nintvars + api::get_n_bin_impl_vars(scip) + api::get_n_int_impl_vars(scip);
    let vars = api::get_vars(scip)[..nintegers as usize].to_vec();

    for &var in &vars {
        let solval = api::get_sol_val(scip, sol, var);
        if !scip.set.is_feas_integral(solval) {
            let x = &scip.vars[var];
            if scip.set.is_feas_eq(x.locdom.lb, x.locdom.ub) {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            } else {
                // @todo better way to handle this would be a BRANCHEXECRELAX callback
                // that could also implement pseudo costs for relaxation solutions
                // instead of using the enforelaxcallback which is mainly intended for
                // spatial branching.
                api::add_extern_branch_cand(scip, var, 0.2, solval);
                *result = ScipResultCode::Infeasible;
            }
        }
    }

    // If we have found a branching candidate, immediately branch to be able to
    // return SCIP_BRANCHED and stop the enforcement loop.
    if *result == ScipResultCode::Infeasible {
        // Call branching methods for external candidates.
        *result = api::branch_extern(scip)?;
    }

    Ok(())
}

/// `consCheckIntegral`: feasibility check method of constraint handler for
/// integral solutions.
#[allow(clippy::too_many_arguments)]
fn cons_check_integral(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _conss: &[ConsId],
    sol: SolId,
    checkintegrality: bool,
    _checklprows: bool,
    _printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    if !checkintegrality {
        return Ok(());
    }

    let p = &scip.probs[api::sol_vars_prob(scip, sol)];
    let nintegers = p.nvars - p.ncontimplvars - p.ncontvars;
    let vars = p.vars.0[..nintegers as usize].to_vec();

    for &var in &vars {
        let solval = api::get_sol_val(scip, sol, var);

        if sol != crate::var::NONE {
            let feastol = scip.set.feastol();
            api::update_sol_integrality_violation(scip, sol, crate::def::epsfrac(solval, feastol));
        }

        if !scip.set.is_feas_integral(solval) {
            *result = ScipResultCode::Infeasible;

            if !completely {
                break;
            }
        }
    }

    Ok(())
}

/// `consLockIntegral`: variable rounding lock method of constraint handler.
fn cons_lock_integral(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId, _locktype: LockType, _nlockspos: i32, _nlocksneg: i32) -> ScipResult {
    Ok(())
}

/// `consGetDiveBdChgsIntegral`: constraint handler method to suggest dive
/// bound changes during the generic diving algorithm.
fn cons_get_dive_bd_chgs_integral(scip: &mut Scip, _conshdlr: ConshdlrId, diveset: usize, sol: SolId, success: &mut bool, _infeasible: &mut bool) -> ScipResult {
    let p = &scip.probs[api::sol_vars_prob(scip, sol)];
    let nintegers = p.nvars - p.nbinimplvars - p.nintimplvars - p.ncontimplvars - p.ncontvars;
    let vars = p.vars.0[..p.nvars as usize].to_vec();

    let mut bestscore = crate::def::REAL_MIN;
    let mut bestcandidx: i32 = -1;
    *success = false;
    let mut bestroundup = false;

    // Loop over solution values and get score of fractional variables.
    for v in 0..nintegers {
        let var = vars[v as usize];
        let solval = api::get_sol_val(scip, sol, var);

        // Skip variable if solution value disagrees with the local bounds.
        let x = &scip.vars[var];
        if !scip.set.is_feas_integral(solval) && scip.set.is_ge(solval, x.locdom.lb) && scip.set.is_le(solval, x.locdom.ub) {
            let frac = solval - scip.set.floor(solval);
            let (score, roundup) = crate::heur::diveset_get_score(scip, diveset, crate::heur::DIVETYPE_INTEGRALITY, var, solval, frac)?;

            // We search for candidates with maximum score.
            if score > bestscore {
                bestcandidx = v;
                bestscore = score;
                bestroundup = roundup;
                *success = true;
            }
        }
    }

    if *success {
        let var = vars[bestcandidx as usize];
        let solval = api::get_sol_val(scip, sol, var);

        // If we want to round up the best candidate, it is added as the preferred
        // bound change.
        let (ceil, floor) = (scip.set.ceil(solval), scip.set.floor(solval));
        api::add_dive_bound_change(scip, var, BranchDir::Upwards, ceil, bestroundup);
        api::add_dive_bound_change(scip, var, BranchDir::Downwards, floor, !bestroundup);
    }

    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrIntegral`: creates the handler for integrality
/// constraint and includes it in SCIP.
pub fn include_conshdlr_integral(scip: &mut Scip) -> ScipResult {
    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_integral),
        None,
        Some(cons_check_integral),
        Some(cons_lock_integral),
        None,
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    scip.conshdlrs[conshdlr].consgetdivebdchgs = Some(cons_get_dive_bd_chgs_integral);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_integral);
    scip.conshdlrs[conshdlr].conshdlrcopy = Some(conshdlr_copy_integral);

    Ok(())
}

/// `conshdlrCopyIntegral`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_integral(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_integral(scip)?;
    *valid = true;
    Ok(())
}
