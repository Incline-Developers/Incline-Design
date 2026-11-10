//! SCIP's LP row and cut methods of the public API (`scip_lp.c`,
//! `scip_cut.c`) the plugins use.

use crate::{
    api,
    lp::{row_create, row_delay_sort, row_ensure_size, row_force_sort, row_get_max_activity, row_get_min_activity},
    prob::{TRANSPROB, prob_all_cols_in_lp},
    retcode::ScipResult,
    scip::{GLOBALEVENTFILTER, Scip},
    sepastore::{SEPASTORE, sepastore_add_cut},
    tree::tree_get_current_depth,
    types::{ConfType, ConsId, RowId, RowOriginType, VarId},
    var::var_add_to_row,
};

/// `SCIPcreateEmptyRowCons` (`scip_lp.c`): creates and captures an LP row
/// without any coefficients from a constraint.
#[allow(clippy::too_many_arguments)]
pub fn create_empty_row_cons(scip: &mut Scip, cons: ConsId, name: &str, lhs: f64, rhs: f64, local: bool, modifiable: bool, removable: bool) -> ScipResult<RowId> {
    row_create(scip, name, &[], &[], lhs, rhs, RowOriginType::Cons, cons, local, modifiable, removable)
}

/// `SCIPaddVarsToRow` (`scip_lp.c`): resolves variables to columns and adds
/// them with the coefficients to the row; this method caches the row
/// extensions and flushes them afterwards to gain better performance.
pub fn add_vars_to_row(scip: &mut Scip, row: RowId, vars: &[VarId], vals: &[f64]) -> ScipResult {
    // Resize the row to be able to store all variables (at least, if they
    // are COLUMN variables).
    let nnonz = scip.rows[row].len;
    row_ensure_size(scip, row, nnonz + vars.len() as i32);

    // Delay the row sorting.
    row_delay_sort(scip, row);

    // Add the variables to the row.
    for v in 0..vars.len() {
        var_add_to_row(scip, vars[v], TRANSPROB, row, vals[v])?;
    }

    // Force the row sorting.
    row_force_sort(scip, row);

    Ok(())
}

/// `SCIPaddVarsToRowSameCoef` (`scip_lp.c`): resolves variables to columns
/// and adds them with the same single coefficient to the row; this method
/// caches the row extensions and flushes them afterwards to gain better
/// performance.
pub fn add_vars_to_row_same_coef(scip: &mut Scip, row: RowId, vars: &[VarId], val: f64) -> ScipResult {
    // Resize the row to be able to store all variables (at least, if they
    // are COLUMN variables).
    let nnonz = scip.rows[row].len;
    row_ensure_size(scip, row, nnonz + vars.len() as i32);

    // Delay the row sorting.
    row_delay_sort(scip, row);

    // Add the variables to the row.
    for &var in vars {
        var_add_to_row(scip, var, TRANSPROB, row, val)?;
    }

    // Force the row sorting.
    row_force_sort(scip, row);

    Ok(())
}

/// `SCIProwGetDualsol` (`lp.c`): gets the dual LP solution of a row.
pub fn row_get_dualsol(scip: &Scip, row: RowId) -> f64 {
    let r = &scip.rows[row];
    if r.lppos >= 0 { r.dualsol } else { 0.0 }
}

/// `SCIPanalyzeConflict` (`scip_conflict.c`): analyzes conflict bounds that
/// were added after a call to SCIPinitConflictAnalysis() with calls to
/// SCIPaddConflict...(); returns whether a conflict constraint was created.
/// Generalized resolution (`conflict/usegenres`) is not ported.
pub fn analyze_conflict(scip: &mut Scip, validdepth: i32) -> ScipResult<bool> {
    if scip.set.conf_enable && scip.set.conf_usegenres {
        unimplemented!("generalized resolution conflict analysis");
    }
    crate::conflict::conflict_analyze_public(scip, TRANSPROB, validdepth)
}

/// `SCIPaddRow` (`scip_cut.c`): adds row to separation storage; returns
/// whether the row makes the node infeasible.
pub fn add_row(scip: &mut Scip, row: RowId, forcecut: bool) -> ScipResult<bool> {
    let root = tree_get_current_depth(scip) == 0;
    let infeasible = sepastore_add_cut(scip, SEPASTORE, GLOBALEVENTFILTER, row, forcecut, root)?;

    // Collect a conflict when the row alone cuts off the node.
    if infeasible && prob_all_cols_in_lp(scip, TRANSPROB) && api::is_conflict_analysis_applicable(scip) {
        api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

        let (lhs, rhs) = (scip.rows[row].lhs, scip.rows[row].rhs);
        if !scip.set.is_infinity(-lhs) {
            let act = row_get_max_activity(scip, row);
            if scip.set.is_lt(act, lhs) {
                let ncols = scip.rows[row].len;
                for j in 0..ncols {
                    let val = scip.rows[row].vals[j];
                    if !scip.set.is_zero(val) {
                        let var = scip.cols[scip.rows[row].cols[j]].var;
                        if val > 0.0 {
                            api::add_conflict_ub(scip, var, None)?;
                        } else {
                            api::add_conflict_lb(scip, var, None)?;
                        }
                    }
                }
            }
        } else if !scip.set.is_infinity(rhs) {
            let act = row_get_min_activity(scip, row);
            if scip.set.is_gt(act, rhs) {
                let ncols = scip.rows[row].len;
                for j in 0..ncols {
                    let val = scip.rows[row].vals[j];
                    if !scip.set.is_zero(val) {
                        let var = scip.cols[scip.rows[row].cols[j]].var;
                        if val > 0.0 {
                            api::add_conflict_lb(scip, var, None)?;
                        } else {
                            api::add_conflict_ub(scip, var, None)?;
                        }
                    }
                }
            }
        }

        let depth = api::get_depth(scip);
        analyze_conflict(scip, depth)?;
    }

    Ok(infeasible)
}

/// `SCIPgetRowSolFeasibility` (`scip_lp.c`): returns the feasibility of a
/// row for the given primal solution (`NONE` for the current one).
pub fn get_row_sol_feasibility(scip: &mut Scip, row: RowId, sol: crate::types::SolId) -> f64 {
    if sol != crate::var::NONE {
        crate::lp::row_get_sol_feasibility(scip, row, sol)
    } else if crate::tree::tree_has_current_node_lp(scip) {
        crate::lp::row_get_lp_feasibility(scip, row)
    } else {
        crate::lp::row_get_pseudo_feasibility(scip, row)
    }
}

/// `SCIProwGetDualfarkas` (`lp.c`): gets the dual Farkas coefficient of a
/// row in an infeasible LP.
pub fn row_get_dualfarkas(scip: &Scip, row: RowId) -> f64 {
    let r = &scip.rows[row];
    if r.lppos >= 0 { r.dualfarkas } else { 0.0 }
}

/// `SCIPcacheRowExtensions` (`scip_lp.c`): informs row that all subsequent
/// additions of variables to the row should be cached and not directly
/// applied.
pub fn cache_row_extensions(scip: &mut Scip, row: RowId) {
    row_delay_sort(scip, row);
}

/// `SCIPflushRowExtensions` (`scip_lp.c`): flushes all cached row
/// extensions after a call of SCIPcacheRowExtensions() and merges coefficients
/// with equal columns into a single coefficient.
pub fn flush_row_extensions(scip: &mut Scip, row: RowId) -> ScipResult {
    row_force_sort(scip, row);
    crate::lp::row_link(scip, row)
}

/// `SCIPcreateEmptyRowSepa` (`scip_lp.c`): creates and captures an LP row
/// without any coefficients from a separator.
#[allow(clippy::too_many_arguments)]
pub fn create_empty_row_sepa(scip: &mut Scip, sepa: crate::types::SepaId, name: &str, lhs: f64, rhs: f64, local: bool, modifiable: bool, removable: bool) -> ScipResult<RowId> {
    row_create(scip, name, &[], &[], lhs, rhs, RowOriginType::Sepa, sepa, local, modifiable, removable)
}

/// `SCIPcreateEmptyRowUnspec` (`scip_lp.c`): creates and captures an LP row
/// without any coefficients from an unspecified source.
pub fn create_empty_row_unspec(scip: &mut Scip, name: &str, lhs: f64, rhs: f64, local: bool, modifiable: bool, removable: bool) -> ScipResult<RowId> {
    row_create(scip, name, &[], &[], lhs, rhs, RowOriginType::Unspec, crate::var::NONE, local, modifiable, removable)
}

/// `SCIPisCutEfficacious` (`scip_cut.c`): returns whether the cut's
/// efficacy with respect to the given primal solution or the current LP
/// solution is greater than the minimal cut efficacy.
pub fn is_cut_efficacious(scip: &mut Scip, sol: crate::types::SolId, cut: RowId) -> bool {
    let root = tree_get_current_depth(scip) == 0;
    if sol == crate::var::NONE {
        crate::lp::row_is_lp_efficacious(scip, cut, root)
    } else {
        crate::lp::row_is_sol_efficacious(scip, cut, sol, root)
    }
}

/// `SCIPreleaseRow` (`scip_lp.c`): decreases usage counter of LP row, and
/// frees memory if necessary; sets `*row` to `NONE`.
pub fn release_row(scip: &mut Scip, row: &mut RowId) -> ScipResult {
    crate::lp::row_release(scip, *row)?;
    *row = crate::var::NONE;
    Ok(())
}

/// `SCIPcreateEmptyRowConshdlr` (`scip_lp.c`): creates and captures an LP
/// row without any coefficients from a constraint handler.
#[allow(clippy::too_many_arguments)]
pub fn create_empty_row_conshdlr(
    scip: &mut Scip,
    conshdlr: crate::types::ConshdlrId,
    name: &str,
    lhs: f64,
    rhs: f64,
    local: bool,
    modifiable: bool,
    removable: bool,
) -> ScipResult<RowId> {
    row_create(scip, name, &[], &[], lhs, rhs, RowOriginType::Conshdlr, conshdlr, local, modifiable, removable)
}

/// `SCIPstartDive` (`scip_lp.c`): initiates LP diving, making methods
/// `SCIPchgVarObjDive()`, `SCIPchgVarLbDive()`, and `SCIPchgVarUbDive()`
/// available.
pub fn start_dive(scip: &mut Scip) -> ScipResult {
    use crate::retcode::Retcode;
    if crate::lp::lp_diving(scip) {
        // Already in diving mode.
        return Err(Retcode::InvalidCall);
    }

    if crate::tree::tree_probing(scip) {
        // Cannot start diving while being in probing mode.
        return Err(Retcode::InvalidCall);
    }

    if !crate::tree::tree_is_focus_node_lp_constructed(scip) {
        // Cannot start diving if LP has not been constructed.
        return Err(Retcode::InvalidCall);
    }

    crate::lp::lp_start_dive(scip)?;

    // Remember the relaxation solution to reset it later.
    if crate::relax::relaxation_is_sol_valid(scip) {
        crate::tree::tree_store_relax_sol(scip, TRANSPROB)?;
    }

    Ok(())
}

/// `SCIPendDive` (`scip_lp.c`): quits LP diving and resets bounds and
/// objective values of columns to the current node's values.
pub fn end_dive(scip: &mut Scip) -> ScipResult {
    use crate::{prob::ORIGPROB, retcode::Retcode};
    if !crate::lp::lp_diving(scip) {
        // Not in diving mode.
        return Err(Retcode::InvalidCall);
    }

    // Unmark the diving flag in the LP and reset all variables' objective and
    // bound values.
    let nvars = scip.probs[TRANSPROB].nvars as usize;
    let vars: Vec<VarId> = scip.probs[TRANSPROB].vars.0[..nvars].to_vec();
    crate::lp::lp_end_dive(scip, TRANSPROB, &vars)?;

    // The lower bound may have changed slightly due to LP resolve in
    // SCIPlpEndDive() (exact solving is not ported).
    let focusnode = scip.tree.as_ref().unwrap().focusnode;
    if !scip.lp.as_ref().unwrap().resolvelperror && focusnode != crate::var::NONE && crate::lp::lp_is_relax(scip) && crate::lp::lp_is_solved(scip) {
        crate::tree::node_update_lowerbound_lp(scip, focusnode, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;
    }
    // Reset the probably changed LP's cutoff bound.
    let cutoffbound = scip.primals[crate::scip::TRANSPRIMAL].cutoffbound;
    crate::lp::lp_set_cutoffbound(scip, TRANSPROB, cutoffbound)?;

    // If a new best solution was created, the cutoff of the tree was delayed
    // due to diving; the cutoff has to be done now.
    if scip.tree.as_ref().unwrap().cutoffdelayed {
        let cutoffbound = scip.primals[crate::scip::TRANSPRIMAL].cutoffbound;
        crate::tree::tree_cutoff(scip, GLOBALEVENTFILTER, cutoffbound)?;
    }

    // If a relaxation was stored before diving, restore it now.
    if scip.tree.as_ref().unwrap().probdiverelaxstored {
        crate::tree::tree_restore_relax_sol(scip, TRANSPROB)?;
    }

    Ok(())
}

/// `SCIPchgVarObjDive` (`scip_lp.c`): changes variable's objective value in
/// current dive.
pub fn chg_var_obj_dive(scip: &mut Scip, var: VarId, newobj: f64) -> ScipResult {
    if !crate::lp::lp_diving(scip) {
        return Err(crate::retcode::Retcode::InvalidCall);
    }

    // Invalidate the LP's cutoff bound, since this has nothing to do with the
    // current objective value anymore; the cutoff bound is reset in
    // SCIPendDive().
    let inf = scip.set.infinity();
    crate::lp::lp_set_cutoffbound(scip, TRANSPROB, inf)?;

    // Mark the LP's objective function invalid.
    crate::lp::lp_mark_diving_obj_changed(scip);

    // Change the objective value of the variable in the diving LP.
    crate::var::var_chg_obj_dive(scip, var, newobj)
}

/// `SCIPchgVarLbDive` (`scip_lp.c`): changes variable's lower bound in
/// current dive.
pub fn chg_var_lb_dive(scip: &mut Scip, var: VarId, newbound: f64) -> ScipResult {
    if !crate::lp::lp_diving(scip) {
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    crate::var::var_chg_lb_dive(scip, var, newbound)
}

/// `SCIPchgVarUbDive` (`scip_lp.c`): changes variable's upper bound in
/// current dive.
pub fn chg_var_ub_dive(scip: &mut Scip, var: VarId, newbound: f64) -> ScipResult {
    if !crate::lp::lp_diving(scip) {
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    crate::var::var_chg_ub_dive(scip, var, newbound)
}

/// `SCIPsolveDiveLP` (`scip_lp.c`): solves the LP of the current dive; no
/// separation or pricing is applied; `itlim` -1 for no limit; returns
/// `(lperror, cutoff)`.
pub fn solve_dive_lp(scip: &mut Scip, itlim: i32) -> ScipResult<(bool, bool)> {
    use crate::{prob::ORIGPROB, types::LpSolStat};
    if !crate::lp::lp_diving(scip) {
        return Err(crate::retcode::Retcode::InvalidCall);
    }

    let mut cutoff = false;

    // Solve diving LP.
    let lperror = crate::lp::lp_solve_and_eval(scip, TRANSPROB, itlim as i64, false, false, false, false)?;

    // The LP is infeasible or the objective limit was reached.
    let solstat = crate::lp::lp_get_solstat(scip);
    if solstat == LpSolStat::Infeasible
        || solstat == LpSolStat::ObjLimit
        || (solstat == LpSolStat::Optimal && {
            let obj = api::get_lp_objval(scip);
            scip.set.is_ge(obj, api::get_cutoffbound(scip))
        })
    {
        // Analyze the infeasible LP (only if the objective was not changed,
        // all columns are in the LP, and no external pricers exist).
        if !(crate::lp::lp_diving_obj_changed(scip) || crate::lp::lp_diving_rows_changed(scip)) && prob_all_cols_in_lp(scip, TRANSPROB) {
            crate::conflict::conflict_analyze_lp_public(scip, TRANSPROB, ORIGPROB, GLOBALEVENTFILTER)?;
        }

        cutoff = true;
    }

    Ok((lperror, cutoff))
}

/// `SCIPinDive` (`scip_lp.c`): returns whether we are in diving mode.
pub fn in_dive(scip: &Scip) -> bool {
    crate::lp::lp_diving(scip)
}
