//! Port of `heur_oneopt.c`: 1-opt heuristic which tries to improve setting
//! of single integer variables.
//!
//! Shifts integer variables of the incumbent in their objective direction as
//! far as the global LP rows allow; for MIPs the continuous variables are
//! then recomputed by an LP in diving mode. The sub-SCIP variant run before
//! presolving (`heuristics/oneopt/beforepresol`, default off) is not ported.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERNODE, HEURTIMING_BEFORENODE, HEURTIMING_BEFOREPRESOL, HEURTIMING_DURINGLPLOOP, HeurTiming},
    prob::TRANSPROB,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_lp,
    types::{HeurId, LpSolStat, RowId, ScipResultCode, VarId, VarStatus},
    var::NONE,
};

const HEUR_NAME: &str = "oneopt";
const HEUR_DESC: &str = "1-opt heuristic which tries to improve setting of single integer variables";
const HEUR_DISPCHAR: u8 = b'i';
const HEUR_PRIORITY: i32 = -20000;
const HEUR_FREQ: i32 = 1;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_BEFOREPRESOL | HEURTIMING_AFTERNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Should the objective be weighted with the potential shifting value when
/// sorting the shifting candidates?
const DEFAULT_WEIGHTEDOBJ: bool = true;
/// Should the heuristic be called before and during the root node?
const DEFAULT_DURINGROOT: bool = true;
/// Should the heuristic be called before presolving.
const DEFAULT_BEFOREPRESOL: bool = false;
/// Should the construction of the LP be forced even if LP solving is
/// deactivated?
const DEFAULT_FORCELPCONSTRUCTION: bool = false;
/// Should the heuristic continue to run as long as improvements are found?
const DEFAULT_USELOOP: bool = true;

/// Primal heuristic data.
struct HeurData {
    /// Index of the last solution for which oneopt was performed.
    lastsolindex: i32,
    weightedobj: bool,
    duringroot: bool,
    forcelpconstruction: bool,
    beforepresol: bool,
    useloop: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/*
 * Local methods
 */

/// `calcShiftVal`: compute value by which the solution of variable `var`
/// can be shifted.
fn calc_shift_val(scip: &Scip, var: VarId, solval: f64, activities: &[f64]) -> f64 {
    // Get variable's solution value, global bounds and objective coefficient.
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;
    let obj = scip.vars[var].obj;
    let mut shiftdown = true;

    // Determine shifting direction and maximal possible shifting w.r.t.
    // corresponding bound.
    let mut newsolval = if obj > 0.0 {
        scip.set.feas_ceil(lb)
    } else if obj < 0.0 {
        shiftdown = false;
        scip.set.feas_floor(ub)
    } else {
        solval
    };

    // newsolval might be bounding solval -> avoid numerical shifting.
    if (shiftdown && scip.set.is_feas_ge(newsolval, solval)) || (!shiftdown && scip.set.is_feas_le(newsolval, solval)) {
        return 0.0;
    }

    // Get data of LP column.
    let col = &scip.cols[scip.vars[var].data.col];
    let ncolrows = col.nlprows as usize;

    // Find maximal shift value, st. all rows stay valid.
    for i in 0..ncolrows {
        let row = col.rows[i];
        let rowpos = scip.rows[row].lppos;

        // Only global rows need to be valid.
        if rowpos >= 0 && !scip.rows[row].local {
            let rowpos = rowpos as usize;
            let val = col.vals[i];
            let left = shiftdown == (val > 0.0);
            let side = if left { scip.rows[row].lhs } else { scip.rows[row].rhs };

            // Only finite bounds need to be considered.
            if !scip.set.is_infinity(if left { -side } else { side }) {
                let newsolvalrow = solval + (side - activities[rowpos]) / val;

                // Update shifting value.
                if (shiftdown && newsolvalrow > newsolval) || (!shiftdown && newsolvalrow < newsolval) {
                    let activity = activities[rowpos] + val * ((if shiftdown { newsolvalrow.floor() } else { newsolvalrow.ceil() }) - solval);

                    // Ensure that shifting preserves feasibility.
                    newsolval = if shiftdown == ((left && scip.set.is_feas_ge(activity, side)) || (!left && scip.set.is_feas_le(activity, side))) {
                        newsolvalrow.floor()
                    } else {
                        newsolvalrow.ceil()
                    };

                    // newsolval might have reached solval -> avoid numerical
                    // shifting.
                    if (shiftdown && scip.set.is_feas_ge(newsolval, solval)) || (!shiftdown && scip.set.is_feas_le(newsolval, solval)) {
                        return 0.0;
                    }
                }
            }
        }
    }

    // We must not shift variables to infinity.
    if scip.set.is_infinity(if shiftdown { -newsolval } else { newsolval }) {
        0.0
    } else {
        newsolval - solval
    }
}

/// `updateRowActivities`: update row activities after a variable's solution
/// value changed.
fn update_row_activities(scip: &Scip, activities: &mut [f64], var: VarId, shiftval: f64) {
    // Get data of column associated to variable.
    let col = &scip.cols[scip.vars[var].data.col];
    let ncolrows = col.nlprows as usize;

    // Enumerate all rows with nonzero entry in this column.
    for i in 0..ncolrows {
        let row = col.rows[i];
        let rowpos = scip.rows[row].lppos;

        // Update row activity, only regard global rows in the LP.
        if rowpos >= 0 && !scip.rows[row].local {
            let rowpos = rowpos as usize;
            activities[rowpos] += shiftval * col.vals[i];

            if scip.set.is_infinity(activities[rowpos]) {
                activities[rowpos] = scip.set.infinity();
            } else if scip.set.is_infinity(-activities[rowpos]) {
                activities[rowpos] = -scip.set.infinity();
            }
        }
    }
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyOneopt`: copy method for primal heuristic plugins.
fn heur_copy_oneopt(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_oneopt(scip)
}

/// `heurInitsolOneopt`: solving process initialization method of primal
/// heuristic.
fn heur_initsol_oneopt(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // If the heuristic is called at the root node, we may want to be called
    // during the cut-and-price loop and even before the first LP solve.
    if hd(scip, heur).duringroot && scip.heurs[heur].freqofs == 0 {
        scip.heurs[heur].timingmask = HEURTIMING_DURINGLPLOOP | HEURTIMING_BEFORENODE;
    }
    Ok(())
}

/// `heurExitsolOneopt`: solving process deinitialization method of primal
/// heuristic.
fn heur_exitsol_oneopt(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset the timing mask to its default value.
    scip.heurs[heur].timingmask = HEUR_TIMING;
    Ok(())
}

/// `heurInitOneopt`: initialization method of primal heuristic.
fn heur_init_oneopt(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Initialize last solution index.
    hdm(scip, heur).lastsolindex = -1;
    Ok(())
}

/// The LP rows (`SCIPgetLPRowsData`).
fn lp_rows(scip: &Scip) -> Vec<RowId> {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `heurExecOneopt`: execution method of primal heuristic.
fn heur_exec_oneopt(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Delayed;

    // We only want to process each solution once.
    let bestsol = api::get_best_sol(scip);
    if bestsol == NONE || hd(scip, heur).lastsolindex == scip.sols[bestsol].index {
        return Ok(());
    }

    // Reset the timing mask to its default value (at the root node it could
    // be different).
    if api::get_n_nodes(scip) > 1 {
        scip.heurs[heur].timingmask = HEUR_TIMING;
    }

    // Get problem variables.
    let mut vars = api::get_vars(scip).to_vec();
    let mut nvars = vars.len();
    let mut nenfovars = nvars - (api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip)) as usize;

    // Do not run if there are no discrete variables.
    if nenfovars == 0 {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    if heurtiming == HEURTIMING_BEFOREPRESOL {
        if !hd(scip, heur).beforepresol {
            return Ok(());
        }

        // The sub-SCIP variant (setupAndSolveSubscipOneopt) needs SCIPcopy,
        // which is not ported.
        return Err(Retcode::NotImplemented);
    }

    // We can only work on solutions valid in the transformed space.
    if crate::sol::sol_is_original(scip, bestsol) {
        return Ok(());
    }

    if heurtiming == HEURTIMING_BEFORENODE && (crate::tree::tree_has_current_node_lp(scip) || hd(scip, heur).forcelpconstruction) {
        let cutoff = api::construct_lp(scip)?;

        // Manually cut off the node if the LP construction detected
        // infeasibility (heuristics cannot return such a result).
        if cutoff {
            let node = crate::tree::tree_get_current_node(scip);
            api::cutoff_node(scip, node)?;
            return Ok(());
        }

        api::flush_lp(scip)?;

        // Get problem variables again, SCIPconstructLP() might have added new
        // variables.
        vars = api::get_vars(scip).to_vec();
        nvars = vars.len();
        nenfovars = nvars - (api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip)) as usize;
    }

    // We need an LP.
    let lprows = lp_rows(scip);
    if lprows.is_empty() {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    hdm(scip, heur).lastsolindex = scip.sols[bestsol].index;
    let worksol = api::create_sol_copy(scip, bestsol)?;
    crate::sol::sol_set_heur(scip, worksol, heur);

    let ret = (|| -> ScipResult {
        // Change solution values due to possible global bound changes first.
        for i in (0..nvars).rev() {
            let var = vars[i];
            let lb = scip.vars[var].glbdom.lb;
            let ub = scip.vars[var].glbdom.ub;

            let solval = api::get_sol_val(scip, worksol, var);
            // Old solution value is smaller than the actual lower bound.
            if scip.set.is_feas_lt(solval, lb) {
                // Set the solution value to the global lower bound.
                api::set_sol_val(scip, worksol, var, lb)?;
            }
            // Old solution value is greater than the actual upper bound.
            else if scip.set.is_feas_gt(solval, scip.vars[var].glbdom.ub) {
                // Set the solution value to the global upper bound.
                api::set_sol_val(scip, worksol, var, ub)?;
            }
        }

        let nlprows = lprows.len();
        let mut activities = vec![0.0f64; nlprows];

        // Initialize LP row activities.
        for i in 0..nlprows {
            let row = lprows[i];

            if !scip.rows[row].local {
                activities[i] = api::get_row_sol_activity(scip, row, worksol);
                if scip.set.is_feas_lt(activities[i], scip.rows[row].lhs) || scip.set.is_feas_gt(activities[i], scip.rows[row].rhs) {
                    // @todo try to correct lp rows
                    return Ok(());
                }
            }
        }

        let (useloop, weightedobj) = {
            let h = hd(scip, heur);
            (h.useloop, h.weightedobj)
        };

        // Buffer storage for possible shift candidates.
        let mut shiftcands: Vec<VarId> = Vec::with_capacity(8);
        let mut shiftvals: Vec<f64> = Vec::with_capacity(8);
        let mut nsuccessfulshifts = 0;
        loop {
            // Initialize data.
            let mut shifted = false;
            shiftcands.clear();
            shiftvals.clear();

            // Enumerate all integer variables and find out which of them are
            // shiftable.
            for &var in &vars[..nenfovars] {
                if scip.vars[var].varstatus == VarStatus::Column {
                    // Find out whether the variable can be shifted.
                    let solval = api::get_sol_val(scip, worksol, var);
                    let shiftval = calc_shift_val(scip, var, solval, &activities);

                    // Insert the variable into the list of shifting
                    // candidates.
                    if !scip.set.is_feas_zero(shiftval) {
                        shiftcands.push(var);
                        shiftvals.push(shiftval);
                    }
                }
            }

            // If at least one variable can be shifted, shift variables sorted
            // by their objective.
            let nshiftcands = shiftcands.len();
            if nshiftcands > 0 {
                // The case that exactly one variable can be shifted is
                // slightly easier.
                if nshiftcands == 1 {
                    let var = shiftcands[0];
                    let solval = api::get_sol_val(scip, worksol, var);
                    let shiftval = shiftvals[0];
                    api::set_sol_val(scip, worksol, var, solval + shiftval)?;
                    update_row_activities(scip, &mut activities, var, shiftval);
                    nsuccessfulshifts += 1;
                } else {
                    // Sort the variables by their objective, optionally
                    // weighted with the shiftval.
                    let mut objcoeffs: Vec<f64> = if weightedobj {
                        (0..nshiftcands).map(|i| scip.vars[shiftcands[i]].obj * shiftvals[i]).collect()
                    } else {
                        (0..nshiftcands).map(|i| scip.vars[shiftcands[i]].obj).collect()
                    };

                    // Sort arrays with respect to the first one.
                    crate::sort::sort(&mut objcoeffs[..], (&mut shiftcands[..],), crate::sort::cmp_real);

                    // Try to shift each variable -> Activities have to be
                    // updated.
                    for i in 0..nshiftcands {
                        let var = shiftcands[i];
                        let solval = api::get_sol_val(scip, worksol, var);
                        let shiftval = calc_shift_val(scip, var, solval, &activities);

                        // Update data structures for nonzero shift value.
                        if !scip.set.is_feas_zero(shiftval) {
                            api::set_sol_val(scip, worksol, var, solval + shiftval)?;
                            update_row_activities(scip, &mut activities, var, shiftval);
                            nsuccessfulshifts += 1;
                        }
                    }
                }
                shifted = true;
            }
            if !(useloop && shifted) {
                break;
            }
        }

        if nsuccessfulshifts > 0 {
            // If the problem is a pure IP, try to install the solution, if it
            // is a MIP, solve LP again to set the continuous variables to the
            // best possible value.
            if nvars == nenfovars || !crate::tree::tree_has_current_node_lp(scip) || api::get_lp_solstat(scip) != LpSolStat::Optimal {
                // Since activities are maintained iteratively, we cannot
                // guarantee the feasibility of the shiftings and have to set
                // the checklprows flag to TRUE.
                if api::try_sol(scip, worksol, false, false, false, false, true)? {
                    *result = ScipResultCode::FoundSol;
                }
            } else {
                // Start diving to calculate the LP relaxation.
                scip_lp::start_dive(scip)?;

                // Set the bounds of the variables: fixed for integers, global
                // bounds for continuous.
                for &var in &vars[..nvars] {
                    if scip.vars[var].varstatus == VarStatus::Column {
                        let (lb, ub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
                        scip_lp::chg_var_lb_dive(scip, var, lb)?;
                        scip_lp::chg_var_ub_dive(scip, var, ub)?;
                    }
                }
                // Apply this after global bounds to not cause an error with
                // intermediate empty domains.
                for &var in &vars[..nenfovars] {
                    if scip.vars[var].varstatus == VarStatus::Column {
                        let solval = api::get_sol_val(scip, worksol, var);
                        scip_lp::chg_var_lb_dive(scip, var, solval)?;
                        scip_lp::chg_var_ub_dive(scip, var, solval)?;
                    }
                }

                // Solve LP; errors in the LP solver should not kill the
                // overall solving process (C catches the return code in
                // optimized mode).
                let lperror = scip_lp::solve_dive_lp(scip, -1).map(|(e, _)| e).unwrap_or(true);

                // Check if this is a feasible solution.
                if !lperror && api::get_lp_solstat(scip) == LpSolStat::Optimal {
                    // Copy the current LP solution to the working solution.
                    crate::sol::sol_link_lp_sol(scip, worksol, TRANSPROB)?;

                    // Check solution for feasibility.
                    if api::try_sol(scip, worksol, false, false, false, false, false)? {
                        *result = ScipResultCode::FoundSol;
                    }
                }

                // Terminate the diving.
                if scip_lp::in_dive(scip) {
                    scip_lp::end_dive(scip)?;
                }
            }
        }

        // Heuristic should not rerun on this incumbent because the heuristic
        // loop finishes only after no further improvements of the incumbent
        // solution are possible.
        if useloop {
            let best = api::get_best_sol(scip);
            hdm(scip, heur).lastsolindex = scip.sols[best].index;
        }

        Ok(())
    })();

    api::free_sol(scip, worksol)?;

    ret
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurOneopt`: creates the oneopt primal heuristic and includes
/// it in SCIP.
pub fn include_heur_oneopt(scip: &mut Scip) -> ScipResult {
    use crate::paramset::paramset_add_bool;

    // Create Oneopt primal heuristic data.
    let heurdata = HeurData {
        lastsolindex: -1,
        weightedobj: DEFAULT_WEIGHTEDOBJ,
        duringroot: DEFAULT_DURINGROOT,
        forcelpconstruction: DEFAULT_FORCELPCONSTRUCTION,
        beforepresol: DEFAULT_BEFOREPRESOL,
        useloop: DEFAULT_USELOOP,
    };

    // Include primal heuristic.
    let heur = api::include_heur_basic(
        scip,
        HEUR_NAME,
        HEUR_DESC,
        HEUR_DISPCHAR,
        HEUR_PRIORITY,
        HEUR_FREQ,
        HEUR_FREQOFS,
        HEUR_MAXDEPTH,
        HEUR_TIMING,
        HEUR_USESSUBSCIP,
        heur_exec_oneopt,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_oneopt);
    h.heurinitsol = Some(heur_initsol_oneopt);
    h.heurexitsol = Some(heur_exitsol_oneopt);
    h.heurinit = Some(heur_init_oneopt);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add oneopt primal heuristic parameters.
    paramset_add_bool(
        scip,
        "heuristics/oneopt/weightedobj",
        "should the objective be weighted with the potential shifting value when sorting the shifting candidates?",
        t!(bool, weightedobj),
        true,
        DEFAULT_WEIGHTEDOBJ,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/oneopt/duringroot",
        "should the heuristic be called before and during the root node?",
        t!(bool, duringroot),
        true,
        DEFAULT_DURINGROOT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/oneopt/forcelpconstruction",
        "should the construction of the LP be forced even if LP solving is deactivated?",
        t!(bool, forcelpconstruction),
        true,
        DEFAULT_FORCELPCONSTRUCTION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/oneopt/beforepresol",
        "should the heuristic be called before presolving?",
        t!(bool, beforepresol),
        true,
        DEFAULT_BEFOREPRESOL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/oneopt/useloop",
        "should the heuristic continue to run as long as improvements are found?",
        t!(bool, useloop),
        true,
        DEFAULT_USELOOP,
        None,
    )?;

    Ok(())
}
