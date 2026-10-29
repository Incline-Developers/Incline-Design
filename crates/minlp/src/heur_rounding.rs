//! Port of `heur_rounding.c`: LP rounding heuristic with infeasibility
//! recovering.
//!
//! Rounds the fractional variables one by one; while a row is violated it
//! rounds a variable that decreases the violation with the fewest locks,
//! otherwise the variable that is most crucial to round in the other
//! direction.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPNODE, HEURTIMING_DURINGLPLOOP, HeurTiming},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, LockType, LpSolStat, RowId, ScipResultCode, SolId, VarId, VarType},
    var::{NONE, var_get_n_locks_down_type, var_get_n_locks_up_type},
};

const HEUR_NAME: &str = "rounding";
const HEUR_DESC: &str = "LP rounding heuristic with infeasibility recovering";
const HEUR_DISPCHAR: u8 = b'r';
const HEUR_PRIORITY: i32 = -1000;
const HEUR_FREQ: i32 = 1;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_DURINGLPLOOP;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Number of calls per found solution that are considered as standard
/// success, a higher factor causes the heuristic to be called more often.
const DEFAULT_SUCCESSFACTOR: i32 = 100;
/// Should the heuristic only be called once per node?
const DEFAULT_ONCEPERNODE: bool = false;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    /// Last LP number where the heuristic was applied.
    lastlp: i64,
    successfactor: i32,
    oncepernode: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/*
 * local methods
 */

/// Row violation bookkeeping shared by the update functions.
pub(crate) struct Violations {
    pub violrows: Vec<RowId>,
    pub violrowpos: Vec<i32>,
    pub nviolrows: usize,
}

/// `updateViolations`: update row violation arrays after a row's activity
/// value changed.
pub(crate) fn update_violations(scip: &Scip, row: RowId, v: &mut Violations, oldactivity: f64, newactivity: f64) {
    let lhs = scip.rows[row].lhs;
    let rhs = scip.rows[row].rhs;
    let oldviol = scip.set.is_feas_lt(oldactivity, lhs) || scip.set.is_feas_gt(oldactivity, rhs);
    let newviol = scip.set.is_feas_lt(newactivity, lhs) || scip.set.is_feas_gt(newactivity, rhs);
    if oldviol != newviol {
        let rowpos = scip.rows[row].lppos as usize;

        if oldviol {
            // The row violation was repaired: remove row from violrows array,
            // decrease violation count.
            let violpos = v.violrowpos[rowpos] as usize;
            v.violrowpos[rowpos] = -1;
            if violpos != v.nviolrows - 1 {
                v.violrows[violpos] = v.violrows[v.nviolrows - 1];
                v.violrowpos[scip.rows[v.violrows[violpos]].lppos as usize] = violpos as i32;
            }
            v.nviolrows -= 1;
        } else {
            // The row is now violated: add row to violrows array, increase
            // violation count.
            v.violrows[v.nviolrows] = row;
            v.violrowpos[rowpos] = v.nviolrows as i32;
            v.nviolrows += 1;
        }
    }
}

/// `updateActivities`: update row activities after a variable's solution
/// value changed.
fn update_activities(scip: &Scip, activities: &mut [f64], v: &mut Violations, var: VarId, oldsolval: f64, newsolval: f64) {
    let delta = newsolval - oldsolval;
    let col = scip.vars[var].data.col;
    let c = &scip.cols[col];
    let ncolrows = c.nlprows as usize;

    for r in 0..ncolrows {
        let row = c.rows[r];
        let rowpos = scip.rows[row].lppos;

        if rowpos >= 0 && !scip.rows[row].local {
            let rowpos = rowpos as usize;

            // Update row activity.
            let oldactivity = activities[rowpos];
            if !scip.set.is_infinity(-oldactivity) && !scip.set.is_infinity(oldactivity) {
                let mut newactivity = oldactivity + delta * c.vals[r];
                if scip.set.is_infinity(newactivity) {
                    newactivity = scip.set.infinity();
                } else if scip.set.is_infinity(-newactivity) {
                    newactivity = -scip.set.infinity();
                }
                activities[rowpos] = newactivity;

                // Update row violation arrays.
                update_violations(scip, row, v, oldactivity, newactivity);
            }
        }
    }
}

/// `selectRounding`: returns a variable, that pushes activity of the row in
/// the given direction with minimal negative impact on other rows (NONE if
/// impossible), with its old and new solution values; if variables have
/// equal impact, chooses the one with best objective value improvement in
/// corresponding direction; rounding in a direction is forbidden, if this
/// forces the objective value over the upper bound.
fn select_rounding(scip: &Scip, sol: SolId, minobj: f64, row: RowId, direction: i32) -> (VarId, f64, f64) {
    let r = &scip.rows[row];
    let nrowcols = r.nlpcols as usize;
    let cutoffbound = api::get_cutoffbound(scip);

    // Select rounding variable.
    let mut minnlocks = i32::MAX;
    let mut bestdeltaobj = scip.set.infinity();
    let mut roundvar = NONE;
    let mut oldsolval = 0.0;
    let mut newsolval = 0.0;
    for c in 0..nrowcols {
        let var = scip.cols[r.cols[c]].var;

        if scip.vars[var].vartype != VarType::Continuous {
            let solval = api::get_sol_val(scip, sol, var);

            if !scip.set.is_feas_integral(solval) {
                let val = r.vals[c];
                let obj = scip.vars[var].obj;

                if direction as f64 * val < 0.0 {
                    // Rounding down.
                    let nlocks = var_get_n_locks_down_type(scip, var, LockType::Model);
                    if nlocks <= minnlocks {
                        let roundval = scip.set.feas_floor(solval);
                        let deltaobj = obj * (roundval - solval);
                        if (nlocks < minnlocks || deltaobj < bestdeltaobj) && minobj - obj < cutoffbound {
                            minnlocks = nlocks;
                            bestdeltaobj = deltaobj;
                            roundvar = var;
                            oldsolval = solval;
                            newsolval = roundval;
                        }
                    }
                } else {
                    // Rounding up.
                    let nlocks = var_get_n_locks_up_type(scip, var, LockType::Model);
                    if nlocks <= minnlocks {
                        let roundval = scip.set.feas_ceil(solval);
                        let deltaobj = obj * (roundval - solval);
                        if (nlocks < minnlocks || deltaobj < bestdeltaobj) && minobj + obj < cutoffbound {
                            minnlocks = nlocks;
                            bestdeltaobj = deltaobj;
                            roundvar = var;
                            oldsolval = solval;
                            newsolval = roundval;
                        }
                    }
                }
            }
        }
    }

    (roundvar, oldsolval, newsolval)
}

/// `selectEssentialRounding`: returns a fractional variable, that has most
/// impact on rows in opposite direction, i.e. that is most crucial to fix in
/// the other direction.
fn select_essential_rounding(scip: &Scip, sol: SolId, minobj: f64, lpcands: &[VarId], nlpcands: usize) -> (VarId, f64, f64) {
    let cutoffbound = api::get_cutoffbound(scip);

    // Select rounding variable.
    let mut maxnlocks = -1;
    let mut bestdeltaobj = scip.set.infinity();
    let mut roundvar = NONE;
    let mut oldsolval = 0.0;
    let mut newsolval = 0.0;
    for &var in &lpcands[..nlpcands] {
        let solval = api::get_sol_val(scip, sol, var);
        if !scip.set.is_feas_integral(solval) {
            let obj = scip.vars[var].obj;

            // Rounding down.
            let nlocks = var_get_n_locks_up_type(scip, var, LockType::Model);
            if nlocks >= maxnlocks {
                let roundval = scip.set.feas_floor(solval);
                let deltaobj = obj * (roundval - solval);
                if (nlocks > maxnlocks || deltaobj < bestdeltaobj) && minobj - obj < cutoffbound {
                    maxnlocks = nlocks;
                    bestdeltaobj = deltaobj;
                    roundvar = var;
                    oldsolval = solval;
                    newsolval = roundval;
                }
            }

            // Rounding up.
            let nlocks = var_get_n_locks_down_type(scip, var, LockType::Model);
            if nlocks >= maxnlocks {
                let roundval = scip.set.feas_ceil(solval);
                let deltaobj = obj * (roundval - solval);
                if (nlocks > maxnlocks || deltaobj < bestdeltaobj) && minobj + obj < cutoffbound {
                    maxnlocks = nlocks;
                    bestdeltaobj = deltaobj;
                    roundvar = var;
                    oldsolval = solval;
                    newsolval = roundval;
                }
            }
        }
    }

    (roundvar, oldsolval, newsolval)
}

/*
 * Callback methods
 */

/// `heurCopyRounding`: copy method for primal heuristic plugins.
fn heur_copy_rounding(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_rounding(scip)
}

/// `heurInitRounding`: initialization method of primal heuristic.
fn heur_init_rounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create heuristic data.
    let sol = api::create_sol(scip, heur)?;
    let h = hdm(scip, heur);
    h.sol = sol;
    h.lastlp = -1;
    Ok(())
}

/// `heurExitRounding`: deinitialization method of primal heuristic.
fn heur_exit_rounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free heuristic data.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurInitsolRounding`: solving process initialization method of primal
/// heuristic.
fn heur_initsol_rounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    hdm(scip, heur).lastlp = -1;

    // Change the heuristic's timingmask, if nit should be called only once
    // per node.
    if hd(scip, heur).oncepernode {
        scip.heurs[heur].timingmask = HEURTIMING_AFTERLPNODE;
    }
    Ok(())
}

/// `heurExitsolRounding`: solving process deinitialization method of primal
/// heuristic.
fn heur_exitsol_rounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset the timing mask to its default value.
    scip.heurs[heur].timingmask = HEUR_TIMING;
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

/// `heurExecRounding`: execution method of primal heuristic.
fn heur_exec_rounding(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call heuristic, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobj = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobj, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Don't call heuristic, if we have already processed the current LP
    // solution.
    let nlps = scip.stat.nlps;
    if nlps == hd(scip, heur).lastlp {
        return Ok(());
    }
    hdm(scip, heur).lastlp = nlps;

    // Don't call heuristic, if it was not successful enough in the past.
    let ncalls = scip.heurs[heur].ncalls;
    let nsolsfound = 10 * scip.heurs[heur].nbestsolsfound + scip.heurs[heur].nsolsfound;
    let nnodes = api::get_n_nodes(scip);
    let successfactor = hd(scip, heur).successfactor as i64;
    if nnodes % ((ncalls / successfactor) / (nsolsfound + 1) + 1) != 0 {
        return Ok(());
    }

    // Get fractional variables, that should be integral.
    let (lpcands, lpcandssol, _, nlpcands, _, _) = api::get_lp_branch_cands(scip)?;
    let nlpcands = nlpcands as usize;

    // Only call heuristic, if LP solution is fractional.
    if nlpcands == 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Get LP rows.
    let lprows = lp_rows(scip);
    let nlprows = lprows.len();

    // Get memory for activities, violated rows, and row violation positions.
    let mut nfrac = nlpcands;
    let mut activities = vec![0.0f64; nlprows];
    let mut v = Violations {
        violrows: vec![NONE; nlprows],
        violrowpos: vec![0; nlprows],
        nviolrows: 0,
    };

    // Get the activities for all globally valid rows; the rows should be
    // feasible, but due to numerical inaccuracies in the LP solver, they can
    // be violated.
    for r in 0..nlprows {
        let row = lprows[r];

        if !scip.rows[row].local {
            activities[r] = api::get_row_activity(scip, row);
            if scip.set.is_feas_lt(activities[r], scip.rows[row].lhs) || scip.set.is_feas_gt(activities[r], scip.rows[row].rhs) {
                v.violrows[v.nviolrows] = row;
                v.violrowpos[r] = v.nviolrows as i32;
                v.nviolrows += 1;
            } else {
                v.violrowpos[r] = -1;
            }
        }
    }

    // Get the working solution from heuristic's local data.
    let sol = hd(scip, heur).sol;

    // Copy the current LP solution to the working solution.
    crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

    // Calculate the minimal objective value possible after rounding
    // fractional variables.
    let mut minobj = api::get_sol_trans_obj(scip, sol);
    for c in 0..nlpcands {
        let obj = scip.vars[lpcands[c]].obj;
        let bestroundval = if obj > 0.0 {
            scip.set.feas_floor(lpcandssol[c])
        } else {
            scip.set.feas_ceil(lpcandssol[c])
        };
        minobj += obj * (bestroundval - lpcandssol[c]);
    }

    // Try to round remaining variables in order to become/stay feasible.
    while nfrac > 0 {
        // Choose next variable to process:
        //  - if a violated row exists, round a variable decreasing the
        //    violation, that has least impact on other rows
        //  - otherwise, round a variable, that has strongest devastating
        //    impact on rows in opposite direction
        let (roundvar, oldsolval, newsolval) = if v.nviolrows > 0 {
            let row = v.violrows[v.nviolrows - 1];
            let rowpos = scip.rows[row].lppos as usize;

            if scip.set.is_feas_lt(activities[rowpos], scip.rows[row].lhs) {
                // Lhs is violated: select a variable rounding, that increases
                // the activity.
                select_rounding(scip, sol, minobj, row, 1)
            } else {
                // Rhs is violated: select a variable rounding, that decreases
                // the activity.
                select_rounding(scip, sol, minobj, row, -1)
            }
        } else {
            select_essential_rounding(scip, sol, minobj, &lpcands, nlpcands)
        };

        // Check, whether rounding was possible.
        if roundvar == NONE {
            break;
        }

        // Update row activities of globally valid rows.
        update_activities(scip, &mut activities, &mut v, roundvar, oldsolval, newsolval);

        // Store new solution value and decrease fractionality counter.
        api::set_sol_val(scip, sol, roundvar, newsolval)?;
        nfrac -= 1;

        // Update minimal objective value possible after rounding remaining
        // variables.
        let obj = scip.vars[roundvar].obj;
        if obj > 0.0 && newsolval > oldsolval {
            minobj += obj;
        } else if obj < 0.0 && newsolval < oldsolval {
            minobj -= obj;
        }
    }

    // Check, if the new solution is feasible.
    if nfrac == 0 && v.nviolrows == 0 {
        // Check solution for feasibility, and add it to solution store if
        // possible; we better check feasibility of LP rows, because of
        // numerical problems with activity updating.
        let stored = api::try_sol(scip, sol, false, false, false, false, true)?;

        if stored {
            *result = ScipResultCode::FoundSol;
        }
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurRounding`: creates the rounding heuristic with
/// infeasibility recovering and includes it in SCIP.
pub fn include_heur_rounding(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // Create Rounding primal heuristic data.
    let heurdata = HeurData {
        sol: NONE,
        lastlp: -1,
        successfactor: DEFAULT_SUCCESSFACTOR,
        oncepernode: DEFAULT_ONCEPERNODE,
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
        heur_exec_rounding,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_rounding);
    h.heurinit = Some(heur_init_rounding);
    h.heurexit = Some(heur_exit_rounding);
    h.heurinitsol = Some(heur_initsol_rounding);
    h.heurexitsol = Some(heur_exitsol_rounding);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add rounding primal heuristic parameters.
    paramset_add_int(
        scip,
        "heuristics/rounding/successfactor",
        "number of calls per found solution that are considered as standard success, a higher factor causes the heuristic to be called more often",
        t!(i32, successfactor),
        true,
        DEFAULT_SUCCESSFACTOR,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/rounding/oncepernode",
        "should the heuristic only be called once per node?",
        t!(bool, oncepernode),
        true,
        DEFAULT_ONCEPERNODE,
        None,
    )?;

    Ok(())
}
