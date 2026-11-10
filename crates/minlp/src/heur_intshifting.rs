//! Port of `heur_intshifting.c`: LP rounding heuristic with infeasibility
//! recovering and final LP solving.
//!
//! Shifts only integer variables, tracking for each row the activity range
//! over the continuous variables' global domains; a potentially feasible
//! assignment is completed by solving the LP over the continuous variables
//! in diving mode.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    misc::RandNumGen,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{HeurId, LockType, LpSolStat, RowId, ScipResultCode, SolId, VarId, VarStatus, VarType},
    var::{NONE, var_get_n_locks_down_type, var_get_n_locks_up_type},
};

const HEUR_NAME: &str = "intshifting";
const HEUR_DESC: &str = "LP rounding heuristic with infeasibility recovering and final LP solving";
const HEUR_DISPCHAR: u8 = b'r';
const HEUR_PRIORITY: i32 = -10000;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Maximal number of non improving shiftings.
const MAXSHIFTINGS: i32 = 50;
const WEIGHTFACTOR: f64 = 1.1;
const DEFAULT_RANDSEED: u32 = 17;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    /// Last LP number where the heuristic was applied.
    lastlp: i64,
    randnumgen: RandNumGen,
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

/// Row violation bookkeeping.
struct Violations {
    violrows: Vec<RowId>,
    violrowpos: Vec<i32>,
    nviolrows: i32,
    /// Number of violated rows with fractional candidates (kept at the front
    /// of `violrows`).
    nviolfracrows: i32,
    /// Number of fractional variables for every row.
    nfracsinrow: Vec<i32>,
}

/// Whether an activity range violates the row sides; `SCIPisFeasLT` cannot
/// handle comparing different infinities, so those are a separate case.
fn is_violated(scip: &Scip, minactivity: f64, maxactivity: f64, lhs: f64, rhs: f64) -> bool {
    if !(scip.set.is_infinity(maxactivity) || scip.set.is_infinity(-maxactivity) || scip.set.is_infinity(minactivity) || scip.set.is_infinity(-minactivity)) {
        scip.set.is_feas_lt(maxactivity, lhs) || scip.set.is_feas_gt(minactivity, rhs)
    } else {
        (scip.set.is_infinity(-maxactivity) && !scip.set.is_infinity(-lhs)) || (scip.set.is_infinity(minactivity) && !scip.set.is_infinity(rhs))
    }
}

/// `updateViolations`: update row violation arrays after a row's activity
/// value changed.
fn update_violations(scip: &Scip, row: RowId, v: &mut Violations, oldminactivity: f64, oldmaxactivity: f64, newminactivity: f64, newmaxactivity: f64) {
    let lhs = scip.rows[row].lhs;
    let rhs = scip.rows[row].rhs;

    let oldviol = is_violated(scip, oldminactivity, oldmaxactivity, lhs, rhs);
    let newviol = is_violated(scip, newminactivity, newmaxactivity, lhs, rhs);

    if oldviol != newviol {
        let rowpos = scip.rows[row].lppos as usize;
        let lppos = |r: RowId| scip.rows[r].lppos as usize;

        if oldviol {
            // The row violation was repaired: remove row from violrows array,
            // decrease violation count.
            let mut violpos = v.violrowpos[rowpos];
            v.violrowpos[rowpos] = -1;
            // First, move the row to the end of the subset of violated rows
            // with fractional variables.
            if v.nfracsinrow[rowpos] > 0 {
                // (C quirk kept: overwrite the slot with the last violated
                // row before the replacement below.)
                v.violrows[violpos as usize] = v.violrows[(v.nviolrows - 1) as usize];

                // Replace with last violated row containing fractional
                // variables.
                if violpos != v.nviolfracrows - 1 {
                    v.violrows[violpos as usize] = v.violrows[(v.nviolfracrows - 1) as usize];
                    v.violrowpos[lppos(v.violrows[violpos as usize])] = violpos;
                    violpos = v.nviolfracrows - 1;
                }
                v.nviolfracrows -= 1;
            }

            // Swap row at the end of the violated array to the position of
            // this row and decrease the counter.
            if violpos != v.nviolrows - 1 {
                v.violrows[violpos as usize] = v.violrows[(v.nviolrows - 1) as usize];
                v.violrowpos[lppos(v.violrows[violpos as usize])] = violpos;
            }
            v.nviolrows -= 1;
        } else {
            // The row is now violated: add row to violrows array, increase
            // violation count.
            v.violrows[v.nviolrows as usize] = row;
            v.violrowpos[rowpos] = v.nviolrows;
            v.nviolrows += 1;

            // If the row contains fractional variables, swap with the last
            // violated row that has no fractional variables at position
            // nviolfracrows.
            if v.nfracsinrow[rowpos] > 0 {
                if v.nviolfracrows < v.nviolrows - 1 {
                    let last = (v.nviolrows - 1) as usize;
                    v.violrows[last] = v.violrows[v.nviolfracrows as usize];
                    v.violrowpos[lppos(v.violrows[last])] = v.nviolrows - 1;

                    v.violrows[v.nviolfracrows as usize] = row;
                    v.violrowpos[rowpos] = v.nviolfracrows;
                }
                v.nviolfracrows += 1;
            }
        }
    }
}

/// `updateActivities`: update row activities after a variable's solution
/// value changed.
fn update_activities(scip: &Scip, minactivities: &mut [f64], maxactivities: &mut [f64], v: &mut Violations, var: VarId, oldsolval: f64, newsolval: f64) {
    let delta = newsolval - oldsolval;
    let col = scip.vars[var].data.col;
    let c = &scip.cols[col];
    let ncolrows = c.nlprows as usize;

    for r in 0..ncolrows {
        let row = c.rows[r];
        let rowpos = scip.rows[row].lppos;

        if rowpos >= 0 && !scip.rows[row].local {
            let rowpos = rowpos as usize;

            // Update row activities.
            let oldminactivity = minactivities[rowpos];
            let oldmaxactivity = maxactivities[rowpos];

            let newminactivity = if !scip.set.is_infinity(-oldminactivity) {
                let a = oldminactivity + delta * c.vals[r];
                minactivities[rowpos] = a;
                a
            } else {
                -scip.set.infinity()
            };
            let newmaxactivity = if !scip.set.is_infinity(oldmaxactivity) {
                let a = oldmaxactivity + delta * c.vals[r];
                maxactivities[rowpos] = a;
                a
            } else {
                scip.set.infinity()
            };

            // Update row violation arrays.
            update_violations(scip, row, v, oldminactivity, oldmaxactivity, newminactivity, newmaxactivity);
        }
    }
}

/// `selectShifting`: returns an integer variable, that pushes activity of
/// the row in the given direction with minimal negative impact on other rows
/// (NONE if impossible); if variables have equal impact, chooses the one
/// with best objective value improvement in corresponding direction; prefer
/// fractional integers over other variables in order to become integral
/// during the process.
#[allow(clippy::too_many_arguments)]
fn select_shifting(scip: &Scip, sol: SolId, row: RowId, rowactivity: f64, direction: i32, nincreases: &[f64], ndecreases: &[f64], increaseweight: f64) -> (VarId, f64, f64) {
    let r = &scip.rows[row];
    let nrowcols = r.nlpcols as usize;

    // Calculate how much the activity must be shifted in order to become
    // feasible.
    let activitydelta = if direction == 1 { r.lhs - rowactivity } else { r.rhs - rowactivity };

    // Select shifting variable.
    let mut bestshiftscore = crate::def::REAL_MAX;
    let mut bestdeltaobj = scip.set.infinity();
    let mut shiftvar = NONE;
    let mut newsolval = 0.0;
    let mut oldsolval = 0.0;
    for c in 0..nrowcols {
        let var = scip.cols[r.cols[c]].var;
        let val = r.vals[c];
        let solval = api::get_sol_val(scip, sol, var);

        // Only accept integer variables.
        if scip.vars[var].vartype == VarType::Continuous {
            continue;
        }

        let isfrac = !scip.set.is_feas_integral(solval);
        let increase = direction as f64 * val > 0.0;
        let probindex = scip.vars[var].probindex as usize;

        // Calculate the score of the shifting (prefer smaller values).
        let shiftscore = if isfrac {
            if increase {
                -1.0 / (var_get_n_locks_up_type(scip, var, LockType::Model) as f64 + 1.0)
            } else {
                -1.0 / (var_get_n_locks_down_type(scip, var, LockType::Model) as f64 + 1.0)
            }
        } else {
            let s = if increase {
                ndecreases[probindex] / increaseweight
            } else {
                nincreases[probindex] / increaseweight
            };
            s + 1.0
        };

        if shiftscore <= bestshiftscore {
            let shiftval = if !increase {
                // Shifting down.
                if isfrac {
                    scip.set.feas_floor(solval)
                } else {
                    let shiftval = scip.set.feas_floor(solval + activitydelta / val);
                    crate::def::max(shiftval, scip.vars[var].glbdom.lb)
                }
            } else {
                // Shifting up.
                if isfrac {
                    scip.set.feas_ceil(solval)
                } else {
                    let shiftval = scip.set.feas_ceil(solval + activitydelta / val);
                    crate::def::min(shiftval, scip.vars[var].glbdom.ub)
                }
            };

            if scip.set.is_eq(shiftval, solval) {
                continue;
            }

            let deltaobj = scip.vars[var].obj * (shiftval - solval);
            // Ignore candidates for which shiftval is too large.
            if (shiftscore < bestshiftscore || deltaobj < bestdeltaobj) && !scip.set.is_huge_value(shiftval.abs()) {
                bestshiftscore = shiftscore;
                bestdeltaobj = deltaobj;
                shiftvar = var;
                oldsolval = solval;
                newsolval = shiftval;
            }
        }
    }

    (shiftvar, oldsolval, newsolval)
}

/// `selectEssentialRounding`: returns a fractional variable, that has most
/// impact on rows in opposite direction, i.e. that is most crucial to fix in
/// the other direction.
fn select_essential_rounding(scip: &Scip, sol: SolId, minobj: f64, lpcands: &[VarId], nlpcands: usize) -> (VarId, f64, f64) {
    let cutoffbound = api::get_cutoffbound(scip);

    // Select shifting variable.
    let mut maxnlocks = -1;
    let mut bestdeltaobj = scip.set.infinity();
    let mut shiftvar = NONE;
    let mut oldsolval = 0.0;
    let mut newsolval = 0.0;
    for &var in &lpcands[..nlpcands] {
        let solval = api::get_sol_val(scip, sol, var);
        if !scip.set.is_feas_integral(solval) {
            let obj = scip.vars[var].obj;

            // Shifting down.
            let nlocks = var_get_n_locks_up_type(scip, var, LockType::Model);
            if nlocks >= maxnlocks {
                let shiftval = scip.set.feas_floor(solval);
                let deltaobj = obj * (shiftval - solval);
                if (nlocks > maxnlocks || deltaobj < bestdeltaobj) && minobj - obj < cutoffbound {
                    maxnlocks = nlocks;
                    bestdeltaobj = deltaobj;
                    shiftvar = var;
                    oldsolval = solval;
                    newsolval = shiftval;
                }
            }

            // Shifting up.
            let nlocks = var_get_n_locks_down_type(scip, var, LockType::Model);
            if nlocks >= maxnlocks {
                let shiftval = scip.set.feas_ceil(solval);
                let deltaobj = obj * (shiftval - solval);
                if (nlocks > maxnlocks || deltaobj < bestdeltaobj) && minobj + obj < cutoffbound {
                    maxnlocks = nlocks;
                    bestdeltaobj = deltaobj;
                    shiftvar = var;
                    oldsolval = solval;
                    newsolval = shiftval;
                }
            }
        }
    }

    (shiftvar, oldsolval, newsolval)
}

/// `addFracCounter`: adds a given value to the fractionality counters of the
/// rows in which the given variable appears.
fn add_frac_counter(scip: &Scip, v: &mut Violations, var: VarId, incval: i32) {
    let col = scip.vars[var].data.col;
    let c = &scip.cols[col];
    let nrows = c.nlprows as usize;
    for r in 0..nrows {
        let row = c.rows[r];
        let rowlppos = scip.rows[row].lppos as usize;

        if scip.rows[row].local {
            continue;
        }

        v.nfracsinrow[rowlppos] += incval;

        let theviolrowpos = v.violrowpos[rowlppos];

        // Swap positions in violrows array if fractionality has changed to 0.
        if theviolrowpos >= 0 {
            // First case: the number of fractional variables has become zero:
            // swap row in violrows array to the second part, containing only
            // violated rows without fractional variables.
            if v.nfracsinrow[rowlppos] == 0 {
                // Nothing to do if row is already at the end of the first
                // part, otherwise, swap it to the last position and decrease
                // the counter.
                if theviolrowpos < v.nviolfracrows - 1 {
                    let last = (v.nviolfracrows - 1) as usize;
                    v.violrows[theviolrowpos as usize] = v.violrows[last];
                    v.violrows[last] = row;

                    v.violrowpos[scip.rows[v.violrows[theviolrowpos as usize]].lppos as usize] = theviolrowpos;
                    v.violrowpos[rowlppos] = v.nviolfracrows - 1;
                }
                v.nviolfracrows -= 1;
            }
            // Second interesting case: the number of fractional variables was
            // zero before, swap it with the first violated row without
            // fractional variables.
            else if v.nfracsinrow[rowlppos] == incval {
                // Nothing to do if the row is exactly located at index
                // nviolfracrows.
                if theviolrowpos > v.nviolfracrows {
                    let first = v.nviolfracrows as usize;
                    v.violrows[theviolrowpos as usize] = v.violrows[first];
                    v.violrows[first] = row;

                    v.violrowpos[scip.rows[v.violrows[theviolrowpos as usize]].lppos as usize] = theviolrowpos;
                    v.violrowpos[rowlppos] = v.nviolfracrows;
                }
                v.nviolfracrows += 1;
            }
        }
    }
}

/*
 * Callback methods
 */

/// `heurCopyIntshifting`: copy method for primal heuristic plugins.
fn heur_copy_intshifting(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_intshifting(scip)
}

/// `heurInitIntshifting`: initialization method of primal heuristic.
fn heur_init_intshifting(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create heuristic data.
    let sol = api::create_sol(scip, heur)?;

    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    let heurdata = HeurData {
        sol,
        lastlp: -1,
        randnumgen: RandNumGen::new(seed),
    };
    scip.heurs[heur].heurdata = Some(Box::new(heurdata) as Box<dyn Any>);

    Ok(())
}

/// `heurExitIntshifting`: deinitialization method of primal heuristic.
fn heur_exit_intshifting(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free heuristic data.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;

    scip.heurs[heur].heurdata = None;

    Ok(())
}

/// `heurInitsolIntshifting`: solving process initialization method of
/// primal heuristic.
fn heur_initsol_intshifting(scip: &mut Scip, heur: HeurId) -> ScipResult {
    hdm(scip, heur).lastlp = -1;
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

/// `heurExecIntshifting`: execution method of primal heuristic.
fn heur_exec_intshifting(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do not call heuristic of node was already detected to be infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Don't call heuristic, if no continuous variables are present -> in this
    // case, it is equivalent to shifting heuristic.
    if api::get_n_cont_vars(scip) == 0 {
        return Ok(());
    }

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
    if nnodes % (ncalls / (nsolsfound + 1) + 1) != 0 {
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
    let nvars = api::get_n_vars(scip) as usize;
    let mut nfrac = nlpcands;
    let mut minactivities = vec![0.0f64; nlprows];
    let mut maxactivities = vec![0.0f64; nlprows];
    let mut v = Violations {
        violrows: vec![NONE; nlprows],
        violrowpos: vec![0; nlprows],
        nviolrows: 0,
        nviolfracrows: 0,
        nfracsinrow: vec![0; nlprows],
    };
    let mut nincreases = vec![0.0f64; nvars];
    let mut ndecreases = vec![0.0f64; nvars];

    // Get the minimal and maximal activity for all globally valid rows for
    // continuous variables in their full range; these are the values of a*x'
    // with x' being the LP solution for integer variables and the lower or
    // upper bound for the continuous variables.
    for r in 0..nlprows {
        let row = lprows[r];

        if !scip.rows[row].local {
            let mut mininf = false;
            let mut maxinf = false;
            minactivities[r] = 0.0;
            maxactivities[r] = 0.0;
            let nnonz = scip.rows[row].len as usize;
            let mut c = 0;
            while c < nnonz && !(mininf && maxinf) {
                let col = scip.rows[row].cols[c];
                let val = scip.rows[row].vals[c];
                let var = scip.cols[col].var;
                if scip.vars[var].vartype != VarType::Continuous {
                    let act = val * crate::lp::col_get_primsol(scip, col);
                    minactivities[r] += act;
                    maxactivities[r] += act;
                } else if val > 0.0 {
                    let lb = scip.vars[var].glbdom.lb;
                    let ub = scip.vars[var].glbdom.ub;
                    if scip.set.is_infinity(-lb) {
                        mininf = true;
                    } else {
                        minactivities[r] += val * lb;
                    }
                    if scip.set.is_infinity(ub) {
                        maxinf = true;
                    } else {
                        maxactivities[r] += val * ub;
                    }
                } else if val < 0.0 {
                    let lb = scip.vars[var].glbdom.lb;
                    let ub = scip.vars[var].glbdom.ub;
                    if scip.set.is_infinity(ub) {
                        mininf = true;
                    } else {
                        minactivities[r] += val * ub;
                    }
                    if scip.set.is_infinity(-lb) {
                        maxinf = true;
                    } else {
                        maxactivities[r] += val * lb;
                    }
                }

                if mininf {
                    minactivities[r] = -scip.set.infinity();
                }
                if maxinf {
                    maxactivities[r] = scip.set.infinity();
                }
                c += 1;
            }

            if scip.set.is_feas_lt(maxactivities[r], scip.rows[row].lhs) || scip.set.is_feas_gt(minactivities[r], scip.rows[row].rhs) {
                v.violrows[v.nviolrows as usize] = row;
                v.violrowpos[r] = v.nviolrows;
                v.nviolrows += 1;
            } else {
                v.violrowpos[r] = -1;
            }
        } else {
            // If row is a local row.
            v.violrowpos[r] = -1;
        }
    }

    // Calc the current number of fractional variables in rows.
    for &cand in &lpcands[..nlpcands] {
        add_frac_counter(scip, &mut v, cand, 1);
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
        let bestshiftval = if obj > 0.0 {
            scip.set.feas_floor(lpcandssol[c])
        } else {
            scip.set.feas_ceil(lpcandssol[c])
        };
        minobj += obj * (bestshiftval - lpcandssol[c]);
    }

    // Try to shift remaining variables in order to become/stay feasible.
    let mut nnonimprovingshifts = 0;
    let mut minnviolrows = i32::MAX;
    let mut increaseweight = 1.0;
    while (nfrac > 0 || v.nviolrows > 0) && nnonimprovingshifts < MAXSHIFTINGS && !crate::solve::solve_is_stopped(scip, false) {
        let nprevviolrows = v.nviolrows;

        // Choose next variable to process:
        //  - if a violated row exists, shift a variable decreasing the
        //    violation, that has least impact on other rows
        //  - otherwise, shift a variable, that has strongest devastating
        //    impact on rows in opposite direction
        let mut shiftvar = NONE;
        let mut oldsolval = 0.0;
        let mut newsolval = 0.0;
        if v.nviolrows > 0 && (nfrac == 0 || nnonimprovingshifts < MAXSHIFTINGS - 1) {
            // Violated rows containing fractional variables are preferred; if
            // such a row exists, choose the last one from the list.
            let rowidx = if v.nviolfracrows > 0 {
                v.nviolfracrows - 1
            } else {
                let n = v.nviolrows;
                hdm(scip, heur).randnumgen.get_int(0, n - 1)
            };

            let row = v.violrows[rowidx as usize];
            let rowpos = scip.rows[row].lppos as usize;

            // Get direction in which activity must be shifted.
            let direction = if scip.set.is_feas_lt(maxactivities[rowpos], scip.rows[row].lhs) { 1 } else { -1 };

            // Search an integer variable that can shift the activity in the
            // necessary direction.
            let rowactivity = if direction == 1 { maxactivities[rowpos] } else { minactivities[rowpos] };
            (shiftvar, oldsolval, newsolval) = select_shifting(scip, sol, row, rowactivity, direction, &nincreases, &ndecreases, increaseweight);
        }

        if shiftvar == NONE && nfrac > 0 {
            (shiftvar, oldsolval, newsolval) = select_essential_rounding(scip, sol, minobj, &lpcands, nlpcands);
        }

        // Check, whether shifting was possible.
        if shiftvar == NONE || scip.set.is_eq(oldsolval, newsolval) {
            break;
        }

        // Update row activities of globally valid rows.
        update_activities(scip, &mut minactivities, &mut maxactivities, &mut v, shiftvar, oldsolval, newsolval);

        if v.nviolrows >= nprevviolrows {
            nnonimprovingshifts += 1;
        } else if v.nviolrows < minnviolrows {
            minnviolrows = v.nviolrows;
            nnonimprovingshifts = 0;
        }

        // Store new solution value and decrease fractionality counter.
        api::set_sol_val(scip, sol, shiftvar, newsolval)?;

        // Update fractionality counter and minimal objective value possible
        // after shifting remaining variables.
        let oldsolvalisfrac = !scip.set.is_feas_integral(oldsolval);
        let obj = scip.vars[shiftvar].obj;
        if oldsolvalisfrac {
            nfrac -= 1;
            nnonimprovingshifts = 0;
            minnviolrows = i32::MAX;
            add_frac_counter(scip, &mut v, shiftvar, -1);

            // The rounding was already calculated into the minobj -> update
            // only if rounding in "wrong" direction.
            if obj > 0.0 && newsolval > oldsolval {
                minobj += obj;
            } else if obj < 0.0 && newsolval < oldsolval {
                minobj -= obj;
            }
        } else {
            // Update minimal possible objective value.
            minobj += obj * (newsolval - oldsolval);
        }

        // Update increase/decrease arrays.
        if !oldsolvalisfrac {
            let probindex = scip.vars[shiftvar].probindex as usize;
            increaseweight *= WEIGHTFACTOR;
            if newsolval < oldsolval {
                ndecreases[probindex] += increaseweight;
            } else {
                nincreases[probindex] += increaseweight;
            }
            if increaseweight >= 1e+09 {
                for i in 0..nvars {
                    nincreases[i] /= increaseweight;
                    ndecreases[i] /= increaseweight;
                }
                increaseweight = 1.0;
            }
        }
    }

    // Check, if the new solution is potentially feasible and solve the LP to
    // calculate values for the continuous variables.
    if nfrac == 0 && v.nviolrows == 0 {
        // Start diving to calculate the LP relaxation.
        scip_lp::start_dive(scip)?;

        // Set the bounds of the variables: fixed for binaries and integers,
        // global bounds for continuous.
        let vars = api::get_vars(scip).to_vec();
        let nintvars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip) - api::get_n_cont_impl_vars(scip)) as usize;
        for &var in &vars[..nvars] {
            if scip.vars[var].varstatus == VarStatus::Column {
                let (lb, ub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
                scip_lp::chg_var_lb_dive(scip, var, lb)?;
                scip_lp::chg_var_ub_dive(scip, var, ub)?;
            }
        }
        // Apply this after global bounds to not cause an error with
        // intermediate empty domains.
        for &var in &vars[..nintvars] {
            if scip.vars[var].varstatus == VarStatus::Column {
                let solval = api::get_sol_val(scip, sol, var);
                scip_lp::chg_var_lb_dive(scip, var, solval)?;
                scip_lp::chg_var_ub_dive(scip, var, solval)?;
            }
        }

        // Solve LP; errors in the LP solver should not kill the overall
        // solving process, if the LP is just needed for a heuristic (C
        // catches the return code in optimized mode).
        let lperror = scip_lp::solve_dive_lp(scip, -1).map(|(e, _)| e).unwrap_or(true);

        // Check if this is a feasible solution.
        if !lperror && api::get_lp_solstat(scip) == LpSolStat::Optimal {
            // Copy the current LP solution to the working solution.
            crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

            // Check solution for feasibility, and add it to solution store if
            // possible; neither integrality nor feasibility of LP rows has to
            // be checked, because this is already done in the intshifting
            // heuristic itself and due to the LP resolve.
            let stored = api::try_sol(scip, sol, false, false, false, false, false)?;

            if stored {
                *result = ScipResultCode::FoundSol;
            }
        }

        // Terminate the diving.
        if scip_lp::in_dive(scip) {
            scip_lp::end_dive(scip)?;
        }
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurIntshifting`: creates the intshifting heuristic with
/// infeasibility recovering and includes it in SCIP.
pub fn include_heur_intshifting(scip: &mut Scip) -> ScipResult {
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
        heur_exec_intshifting,
        None,
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_intshifting);
    h.heurinit = Some(heur_init_intshifting);
    h.heurexit = Some(heur_exit_intshifting);
    h.heurinitsol = Some(heur_initsol_intshifting);

    Ok(())
}
