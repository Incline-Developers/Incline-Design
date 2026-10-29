//! Port of `heur_zirounding.c`: LP rounding heuristic as suggested by C.
//! Wallace taking row slacks and bounds into account.
//!
//! Shifts fractional variables within the slack of their rows to reduce
//! their fractionality (ZI value); equations need a continuous slack
//! variable that occurs in no other LP row.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPNODE, HeurTiming},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, LpSolStat, RowId, ScipResultCode, SolId, VarId, VarType},
    var::{NONE, var_is_implied_integral, var_is_integral},
};

const HEUR_NAME: &str = "zirounding";
const HEUR_DESC: &str = "LP rounding heuristic as suggested by C. Wallace taking row slacks and bounds into account";
const HEUR_DISPCHAR: u8 = b'r';
const HEUR_PRIORITY: i32 = -500;
const HEUR_FREQ: i32 = 1;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Delimits the number of main loops, can be set to -1 for no limit.
const DEFAULT_MAXROUNDINGLOOPS: i32 = 2;
/// Deactivation check is enabled by default.
const DEFAULT_STOPZIROUND: bool = true;
/// The tolerance percentage after which zirounding will not be executed
/// anymore.
const DEFAULT_STOPPERCENTAGE: f64 = 0.02;
/// Number of heuristic calls before deactivation check.
const DEFAULT_MINSTOPNCALLS: i32 = 1000;

/// Minimum shift value for every single step.
const MINSHIFT: f64 = 1e-4;

/// Primal heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    /// The number of the last LP for which ZIRounding was called.
    lastlp: i64,
    maxroundingloops: i32,
    stopziround: bool,
    stoppercentage: f64,
    minstopncalls: i32,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/// Shifting direction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

/*
 * Local methods
 */

/// `getZiValue`: returns the fractionality of a value x, which is calculated
/// as zivalue(x) = min(x-floor(x), ceil(x)-x).
fn get_zi_value(scip: &Scip, val: f64) -> f64 {
    let upgap = scip.set.feas_ceil(val) - val;
    let downgap = val - scip.set.feas_floor(val);
    crate::def::min(upgap, downgap)
}

/// `calculateBounds`: determines shifting bounds for variable; returns
/// whether a numerical error occurred.
fn calculate_bounds(scip: &Scip, var: VarId, currentvalue: f64, upperbound: &mut f64, lowerbound: &mut f64, upslacks: &[f64], downslacks: &[f64]) -> bool {
    // Get the column associated to the variable, the nonzero rows and the
    // nonzero coefficients.
    let col = &scip.cols[scip.vars[var].data.col];
    let ncolvals = col.nlprows as usize;

    // Only proceed, when variable has nonzero coefficients.
    if ncolvals == 0 {
        return false;
    }

    // Initialize the bounds on the shift to be the gap of the current
    // solution value to the bounds of the variable.
    let (lb, ub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
    *upperbound = if scip.set.is_infinity(ub) { scip.set.infinity() } else { ub - currentvalue };
    *lowerbound = if scip.set.is_infinity(-lb) { scip.set.infinity() } else { currentvalue - lb };

    // Go through every nonzero row coefficient corresponding to var to
    // determine bounds for shifting in such a way that shifting maintains
    // feasibility in every LP row.
    let mut i = 0;
    while i < ncolvals && crate::def::max(*lowerbound, *upperbound) >= MINSHIFT {
        let row = col.rows[i];
        let rowpos = scip.rows[row].lppos;
        i += 1;

        // The row might currently not be in the LP, ignore it!
        if rowpos == -1 {
            continue;
        }
        let rowpos = rowpos as usize;
        let val = col.vals[i - 1];

        // All bounds and slacks as they are calculated in zirounding always
        // have to be greater equal zero. It might however be due to
        // numerical issues, e.g. with scaling, that they are not. Better
        // abort in this case.
        if scip.set.is_feas_lt(*lowerbound, 0.0)
            || scip.set.is_feas_lt(*upperbound, 0.0)
            || scip.set.is_feas_lt(upslacks[rowpos], 0.0)
            || scip.set.is_feas_lt(downslacks[rowpos], 0.0)
        {
            return true;
        }

        // If coefficient > 0, rounding up might violate up slack and rounding
        // down might violate down slack; thus search for the minimum so that
        // no constraint is violated; vice versa for coefficient < 0.
        if val > 0.0 {
            if !scip.set.is_infinity(upslacks[rowpos]) {
                let upslack = crate::def::max(upslacks[rowpos], 0.0);
                *upperbound = crate::def::min(*upperbound, upslack / val);
            }

            if !scip.set.is_infinity(downslacks[rowpos]) {
                let downslack = crate::def::max(downslacks[rowpos], 0.0);
                *lowerbound = crate::def::min(*lowerbound, downslack / val);
            }
        } else {
            if !scip.set.is_infinity(upslacks[rowpos]) {
                let upslack = crate::def::max(upslacks[rowpos], 0.0);
                *lowerbound = crate::def::min(*lowerbound, -upslack / val);
            }

            if !scip.set.is_infinity(downslacks[rowpos]) {
                let downslack = crate::def::max(downslacks[rowpos], 0.0);
                *upperbound = crate::def::min(*upperbound, -downslack / val);
            }
        }
    }
    false
}

/// `updateSlacks`: when a variable is shifted, the activities and slacks of
/// all rows it appears in have to be updated.
#[allow(clippy::too_many_arguments)]
fn update_slacks(
    scip: &mut Scip,
    sol: SolId,
    var: VarId,
    shiftvalue: f64,
    upslacks: &mut [f64],
    downslacks: &mut [f64],
    activities: &mut [f64],
    slackvars: &[VarId],
    slackcoeffs: &[f64],
) -> ScipResult {
    let col = scip.vars[var].data.col;
    let nrows = scip.cols[col].nlprows as usize;

    // Go through all rows the shifted variable appears in.
    for i in 0..nrows {
        let row = scip.cols[col].rows[i];
        let rowpos = scip.rows[row].lppos;

        // If the row is in the LP, update its activity, up and down slack.
        if rowpos >= 0 {
            let rowpos = rowpos as usize;
            let val = scip.cols[col].vals[i] * shiftvalue;
            let lhs = scip.rows[row].lhs;
            let rhs = scip.rows[row].rhs;

            // If the row is an equation, we update its slack variable instead
            // of its activities.
            if scip.set.is_feas_eq(lhs, rhs) {
                let slackvarsolval = api::get_sol_val(scip, sol, slackvars[rowpos]);
                let slackvarshiftval = -val / slackcoeffs[rowpos];

                api::set_sol_val(scip, sol, slackvars[rowpos], slackvarsolval + slackvarshiftval)?;
            } else if !scip.set.is_infinity(activities[rowpos].abs()) {
                activities[rowpos] += val;
            }

            // The slacks of the row now can be updated independently of its
            // type.
            if !scip.set.is_infinity(upslacks[rowpos]) {
                upslacks[rowpos] -= val;
            }
            if !scip.set.is_infinity(-downslacks[rowpos]) {
                downslacks[rowpos] += val;
            }
        }
    }
    Ok(())
}

/// `rowFindSlackVar`: finds a continuous slack variable for an equation row
/// (NONE if none exists) and its coefficient.
fn row_find_slack_var(scip: &Scip, row: RowId) -> (VarId, f64) {
    let r = &scip.rows[row];
    let nrowvals = r.len as usize;

    // Iterate over the row variables. Stop after the first unfixed continuous
    // variable was found.
    for v in (0..nrowvals).rev() {
        let col = r.cols[v];
        if scip.cols[col].lppos == -1 {
            continue;
        }

        let colvar = scip.cols[col].var;

        if !var_is_integral(scip, colvar) && !scip.set.is_feas_eq(scip.vars[colvar].glbdom.lb, scip.vars[colvar].glbdom.ub) && scip.cols[col].nlprows == 1 {
            return (colvar, r.vals[v]);
        }
    }

    (NONE, 0.0)
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyZirounding`: copy method for primal heuristic plugins.
fn heur_copy_zirounding(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_zirounding(scip)
}

/// `heurInitZirounding`: initialization method of primal heuristic.
fn heur_init_zirounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitZirounding`: deinitialization method of primal heuristic.
fn heur_exit_zirounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurInitsolZirounding`: solving process initialization method of primal
/// heuristic.
fn heur_initsol_zirounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    hdm(scip, heur).lastlp = -1;
    Ok(())
}

/// The LP rows (`SCIPgetLPRows`).
fn lp_rows(scip: &Scip) -> Vec<RowId> {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `heurExecZirounding`: execution method of primal heuristic.
fn heur_exec_zirounding(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do not call heuristic of node was already detected to be infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Only call heuristic if an optimal LP-solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobj = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobj, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    let (stopziround, minstopncalls, stoppercentage, maxroundingloops) = {
        let h = hd(scip, heur);
        (h.stopziround, h.minstopncalls, h.stoppercentage, h.maxroundingloops)
    };

    // Do not call heuristic if deactivation check is enabled and percentage
    // of found solutions in relation to number of calls falls below
    // stoppercentage.
    let ncalls = scip.heurs[heur].ncalls;
    if stopziround && ncalls >= minstopncalls as i64 && (scip.heurs[heur].nsolsfound as f64 / ncalls as f64) < stoppercentage {
        return Ok(());
    }

    // Assure that heuristic has not already been called after the last LP
    // had been solved.
    let nlps = scip.stat.nlps;
    if nlps == hd(scip, heur).lastlp {
        return Ok(());
    }

    hdm(scip, heur).lastlp = nlps;

    // Get fractional variables.
    let (lpcands, lpcandssol, _, nlpcands, _, nimplfracs) = api::get_lp_branch_cands(scip)?;
    let nlpcands = (nlpcands + nimplfracs) as usize;

    // Make sure that there is at least one fractional variable that should be
    // integral.
    if nlpcands == 0 {
        return Ok(());
    }

    // Get LP rows data.
    let rows = lp_rows(scip);
    let nslacks = rows.len();

    // Cannot do anything if LP is empty.
    if nslacks == 0 {
        return Ok(());
    }

    // Get the working solution from heuristic's local data.
    let sol = hd(scip, heur).sol;

    *result = ScipResultCode::DidNotFind;

    // Copy the current LP solution to the working solution and allocate
    // memory for local data.
    crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;
    let mut solarray: Vec<f64> = lpcandssol[..nlpcands].to_vec();
    let mut zilpcands: Vec<VarId> = lpcands[..nlpcands].to_vec();

    // Allocate buffer data arrays.
    let mut slackvars = vec![NONE; nslacks];
    let mut upslacks = vec![0.0f64; nslacks];
    let mut downslacks = vec![0.0f64; nslacks];
    let mut slackvarcoeffs = vec![0.0f64; nslacks];
    let mut rowneedsslackvar = vec![false; nslacks];
    let mut activities = vec![0.0f64; nslacks];

    let mut nroundings = 0;

    // Loop over fractional variables and involved LP rows to find all rows
    // which require a slack variable.
    for &cand in &zilpcands {
        let col = &scip.cols[scip.vars[cand].data.col];
        let ncandrows = col.nlprows as usize;

        for r in 0..ncandrows {
            let row = col.rows[r];
            let rowpos = scip.rows[row].lppos;

            if rowpos >= 0 && scip.set.is_feas_eq(scip.rows[row].lhs, scip.rows[row].rhs) {
                rowneedsslackvar[rowpos as usize] = true;
            }
        }
    }

    // Calculate row slacks for every every row that belongs to the current LP
    // and ensure, that the current solution has no violated constraint -- if
    // any constraint is violated, i.e. a slack is significantly smaller than
    // zero, this will cause the termination of the heuristic because
    // Zirounding does not provide feasibility recovering.
    for i in 0..nslacks {
        let row = rows[i];

        let lhs = scip.rows[row].lhs;
        let rhs = scip.rows[row].rhs;

        // Get row activity.
        activities[i] = api::get_row_activity(scip, row);

        // In special case if LHS or RHS is (-)infinity slacks have to be
        // initialized as infinity.
        downslacks[i] = if scip.set.is_infinity(-lhs) { scip.set.infinity() } else { activities[i] - lhs };
        upslacks[i] = if scip.set.is_infinity(rhs) { scip.set.infinity() } else { rhs - activities[i] };

        // Row is an equation. Try to find a slack variable in the row, i.e., a
        // continuous variable which occurs only in this row. If no such
        // variable exists, there is no hope for an IP-feasible solution in
        // this round.
        if scip.set.is_feas_eq(lhs, rhs) && rowneedsslackvar[i] {
            (slackvars[i], slackvarcoeffs[i]) = row_find_slack_var(scip, row);

            if slackvars[i] == NONE {
                return Ok(());
            }

            let solvalslackvar = api::get_sol_val(scip, sol, slackvars[i]);
            let ubslackvar = scip.vars[slackvars[i]].glbdom.ub;
            let lbslackvar = scip.vars[slackvars[i]].glbdom.lb;

            let coeffslackvar = slackvarcoeffs[i];

            let ubgap = crate::def::max(0.0, ubslackvar - solvalslackvar);
            let lbgap = crate::def::max(0.0, solvalslackvar - lbslackvar);

            if scip.set.is_positive(coeffslackvar) {
                if !scip.set.is_infinity(lbslackvar) {
                    upslacks[i] += coeffslackvar * lbgap;
                } else {
                    upslacks[i] = scip.set.infinity();
                }
                if !scip.set.is_infinity(ubslackvar) {
                    downslacks[i] += coeffslackvar * ubgap;
                } else {
                    downslacks[i] = scip.set.infinity();
                }
            } else {
                if !scip.set.is_infinity(ubslackvar) {
                    upslacks[i] -= coeffslackvar * ubgap;
                } else {
                    upslacks[i] = scip.set.infinity();
                }
                if !scip.set.is_infinity(lbslackvar) {
                    downslacks[i] -= coeffslackvar * lbgap;
                } else {
                    downslacks[i] = scip.set.infinity();
                }
            }
        }
        // Due to numerical inaccuracies, the rows might be feasible, even if
        // the slacks are significantly smaller than zero -> terminate.
        if scip.set.is_feas_lt(upslacks[i], 0.0) || scip.set.is_feas_lt(downslacks[i], 0.0) {
            return Ok(());
        }
    }

    // Initialize number of remaining variables and flag to enter the main
    // loop.
    let mut currentlpcands = nlpcands as i32;
    let mut improvementfound = true;

    // Iterate over variables as long as there are fractional variables left.
    while currentlpcands > 0 && improvementfound && (maxroundingloops == -1 || nroundings < maxroundingloops) {
        improvementfound = false;
        nroundings += 1;

        // Check for every remaining fractional variable if a shifting
        // decreases ZI-value of the variable.
        let mut c: i32 = 0;
        while c < currentlpcands {
            let cu = c as usize;

            // Get values from local data.
            let oldsolval = solarray[cu];
            let var = zilpcands[cu];

            // Calculate bounds for variable and make sure that there are no
            // numerical inconsistencies.
            let mut upperbound = scip.set.infinity();
            let mut lowerbound = scip.set.infinity();
            if calculate_bounds(scip, var, oldsolval, &mut upperbound, &mut lowerbound, &upslacks, &downslacks) {
                return Ok(());
            }

            // Continue if only marginal shifts are possible.
            if crate::def::max(upperbound, lowerbound) < MINSHIFT {
                // Stop immediately if a variable has not been rounded during
                // the last round.
                if nroundings == maxroundingloops {
                    break;
                } else {
                    c += 1;
                    continue;
                }
            }

            // Calculate the possible values after shifting.
            let mut up = oldsolval + upperbound;
            let mut down = oldsolval - lowerbound;

            // If the variable is integer or implied integral, do not shift
            // further than the nearest integer.
            if scip.vars[var].vartype != VarType::Binary || var_is_implied_integral(scip, var) {
                let ceilx = scip.set.feas_ceil(oldsolval);
                let floorx = scip.set.feas_floor(oldsolval);
                up = crate::def::min(up, ceilx);
                down = crate::def::max(down, floorx);
            }

            // Calculate necessary values.
            let ziup = get_zi_value(scip, up);
            let zidown = get_zi_value(scip, down);
            let zicurrent = get_zi_value(scip, oldsolval);

            // Calculate the shifting direction that reduces ZI-value the most;
            // if both directions improve ZI-value equally, take the direction
            // which improves the objective.
            if scip.set.is_feas_lt(zidown, zicurrent) || scip.set.is_feas_lt(ziup, zicurrent) {
                let direction = if scip.set.is_feas_eq(ziup, zidown) {
                    if scip.set.is_feas_ge(scip.vars[var].obj, 0.0) { Direction::Down } else { Direction::Up }
                } else if scip.set.is_feas_lt(zidown, ziup) {
                    Direction::Down
                } else {
                    Direction::Up
                };

                // Once a possible shifting direction and value have been
                // found, variable value is updated.
                let mut shiftval = if direction == Direction::Up { up - oldsolval } else { down - oldsolval };

                // This improves numerical stability in some cases.
                shiftval = if direction == Direction::Up {
                    crate::def::min(shiftval, upperbound)
                } else {
                    crate::def::min(shiftval, lowerbound)
                };

                // Update the solution.
                solarray[cu] = if direction == Direction::Up { up } else { down };
                api::set_sol_val(scip, sol, var, solarray[cu])?;

                // Update the rows activities and slacks.
                update_slacks(scip, sol, var, shiftval, &mut upslacks, &mut downslacks, &mut activities, &slackvars, &slackvarcoeffs)?;

                // Since at least one improvement has been found, heuristic
                // will enter main loop for another time.
                improvementfound = true;
            }

            // If solution value of variable has become feasibly integral due
            // to rounding step, variable is put at the end of remaining
            // candidates array so as not to be considered in future loops.
            if scip.set.is_feas_integral(solarray[cu]) {
                let last = (currentlpcands - 1) as usize;
                zilpcands[cu] = zilpcands[last];
                solarray[cu] = solarray[last];
                currentlpcands -= 1;

                // Counter is decreased if end of candidates array has not
                // been reached yet.
                if c < currentlpcands {
                    c -= 1;
                }
            } else if nroundings == maxroundingloops {
                return Ok(());
            }
            c += 1;
        }
    }

    // In case that no candidate is left for rounding after the final main
    // loop the found solution has to be checked for feasibility in the
    // original problem.
    if currentlpcands == 0 {
        let stored = api::try_sol(scip, sol, false, false, false, true, false)?;
        if stored {
            *result = ScipResultCode::FoundSol;
        }
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurZirounding`: creates the zirounding primal heuristic and
/// includes it in SCIP.
pub fn include_heur_zirounding(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Create zirounding primal heuristic data.
    let heurdata = HeurData {
        sol: NONE,
        lastlp: -1,
        maxroundingloops: DEFAULT_MAXROUNDINGLOOPS,
        stopziround: DEFAULT_STOPZIROUND,
        stoppercentage: DEFAULT_STOPPERCENTAGE,
        minstopncalls: DEFAULT_MINSTOPNCALLS,
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
        heur_exec_zirounding,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_zirounding);
    h.heurinit = Some(heur_init_zirounding);
    h.heurexit = Some(heur_exit_zirounding);
    h.heurinitsol = Some(heur_initsol_zirounding);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add zirounding primal heuristic parameters.
    paramset_add_int(
        scip,
        "heuristics/zirounding/maxroundingloops",
        "determines maximum number of rounding loops",
        t!(i32, maxroundingloops),
        true,
        DEFAULT_MAXROUNDINGLOOPS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/zirounding/stopziround",
        "flag to determine if Zirounding is deactivated after a certain percentage of unsuccessful calls",
        t!(bool, stopziround),
        true,
        DEFAULT_STOPZIROUND,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/zirounding/stoppercentage",
        "if percentage of found solutions falls below this parameter, Zirounding will be deactivated",
        t!(f64, stoppercentage),
        true,
        DEFAULT_STOPPERCENTAGE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/zirounding/minstopncalls",
        "determines the minimum number of calls before percentage-based deactivation of Zirounding is applied",
        t!(i32, minstopncalls),
        true,
        DEFAULT_MINSTOPNCALLS,
        1,
        i32::MAX,
        None,
    )?;

    Ok(())
}
