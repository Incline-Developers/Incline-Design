//! Port of `heur_randrounding.c`: fast LP rounding heuristic.
//!
//! Rounds the fractional variables in random order, each up with
//! probability equal to its fractional part; at the root node each rounding
//! is propagated in probing mode.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPNODE, HEURTIMING_DURINGLPLOOP, HEURTIMING_DURINGPRICINGLOOP, HeurTiming},
    misc::RandNumGen,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_probing,
    types::{HeurId, LpSolStat, ScipResultCode, SolId, VarId},
    var::{NONE, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "randrounding";
const HEUR_DESC: &str = "fast LP rounding heuristic";
const HEUR_DISPCHAR: u8 = b'r';
const HEUR_PRIORITY: i32 = -200;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_DURINGLPLOOP;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Should the heuristic only be called once per node?
const DEFAULT_ONCEPERNODE: bool = false;
/// Default random seed.
const DEFAULT_RANDSEED: u32 = 23;
/// Should the heuristic apply the variable lock strategy of simple rounding,
/// if possible?
const DEFAULT_USESIMPLEROUNDING: bool = false;
/// Limit of rounds for each propagation call.
const DEFAULT_MAXPROPROUNDS: i32 = 1;
/// Should the probing part of the heuristic be applied exclusively at the
/// root node.
const DEFAULT_PROPAGATEONLYROOT: bool = true;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    randnumgen: Option<RandNumGen>,
    /// Last LP number where the heuristic was applied.
    lastlp: i64,
    maxproprounds: i32,
    oncepernode: bool,
    usesimplerounding: bool,
    propagateonlyroot: bool,
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

/// `performRandRounding`: perform randomized rounding of the given
/// solution; domain propagation is optionally applied after every rounding
/// step.
fn perform_rand_rounding(scip: &mut Scip, heur: HeurId, sol: SolId, cands: &[VarId], ncands: usize, propagate: bool, result: &mut ScipResultCode) -> ScipResult {
    // Start probing tree before rounding begins.
    if propagate {
        scip_probing::start_probing(scip)?;
        crate::stat::stat_enable_var_history(scip);
    }

    // Copy and permute the candidate array.
    let mut permutedcands: Vec<VarId> = cands[..ncands].to_vec();
    hdm(scip, heur).randnumgen.as_mut().unwrap().permute(&mut permutedcands, 0, ncands);
    let mut cutoff = false;

    let (usesimplerounding, maxproprounds) = {
        let h = hd(scip, heur);
        (h.usesimplerounding, h.maxproprounds)
    };

    // Loop over candidates and perform randomized rounding and optionally
    // probing.
    let mut c = 0;
    while c < ncands && !cutoff {
        // Get next variable from permuted candidate array.
        let var = permutedcands[c];
        c += 1;
        let oldsolval = api::get_sol_val(scip, sol, var);
        let lb = scip.vars[var].locdom.lb;
        let ub = scip.vars[var].locdom.ub;

        let mayrounddown = var_may_round_down(scip, var);
        let mayroundup = var_may_round_up(scip, var);
        let ceilval = scip.set.feas_ceil(oldsolval);
        let floorval = scip.set.feas_floor(oldsolval);

        // Abort if rounded ceil and floor value lie outside the variable
        // domain. Otherwise, check if bounds allow only one rounding
        // direction, anyway.
        let newsolval = if lb > ceilval + 0.5 || ub < floorval - 0.5 {
            cutoff = true;
            break;
        } else if scip.set.is_feas_eq(lb, ceilval) {
            // Only rounding up possible.
            ceilval
        } else if scip.set.is_feas_eq(ub, floorval) {
            // Only rounding down possible.
            floorval
        } else if !usesimplerounding || !(mayroundup || mayrounddown) {
            // The standard randomized rounding.
            let randnumber = hdm(scip, heur).randnumgen.as_mut().unwrap().get_real(0.0, 1.0);
            if randnumber <= oldsolval - floorval { ceilval } else { floorval }
        }
        // Choose rounding direction, if possible, or use the only direction
        // guaranteed to be feasible.
        else if mayrounddown && mayroundup {
            // We can round in both directions: round in objective function
            // direction.
            if scip.vars[var].obj >= 0.0 { floorval } else { ceilval }
        } else if mayrounddown {
            floorval
        } else {
            ceilval
        };

        // If propagation is enabled, fix the candidate variable to its rounded
        // value and propagate the solution.
        if propagate {
            let lbadjust = scip.set.is_gt(newsolval, lb);
            let ubadjust = scip.set.is_lt(newsolval, ub);

            // Enter a new probing node if the variable was not already fixed
            // before.
            if lbadjust || ubadjust {
                if crate::solve::solve_is_stopped(scip, false) {
                    break;
                }

                // We only want to create a new probing node if we do not
                // exceeed the maximal tree depth, otherwise we finish at this
                // point.
                if api::get_depth(scip) < crate::def::MAXTREEDEPTH {
                    scip_probing::new_probing_node(scip)?;
                } else {
                    break;
                }

                // Tighten the bounds to fix the variable for the probing node.
                if lbadjust {
                    scip_probing::chg_var_lb_probing(scip, var, newsolval)?;
                }
                if ubadjust {
                    scip_probing::chg_var_ub_probing(scip, var, newsolval)?;
                }

                // Call propagation routines for the reduced problem.
                cutoff = scip_probing::propagate_probing(scip, maxproprounds)?.0;
            }
        }
        // Store new solution value.
        api::set_sol_val(scip, sol, var, newsolval)?;
    }

    // If no cutoff was detected, the solution is a candidate to be checked
    // for feasibility.
    if !cutoff && !crate::solve::solve_is_stopped(scip, false) {
        let stored = if crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) {
            // Neither integrality nor feasibility of LP rows has to be
            // checked, because all fractional variables were already moved in
            // feasible direction to the next integer.
            api::try_sol(scip, sol, false, false, false, false, true)?
        } else {
            // If there are variables which are not present in the LP, e.g.,
            // for column generation, we need to check their bounds.
            api::try_sol(scip, sol, false, false, true, false, true)?
        };

        if stored {
            *result = ScipResultCode::FoundSol;
        }
    }

    // Exit probing mode.
    if propagate {
        scip_probing::end_probing(scip)?;
    }

    Ok(())
}

/// `performLPRandRounding`: perform randomized LP-rounding.
fn perform_lp_rand_rounding(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, propagate: bool, result: &mut ScipResultCode) -> ScipResult {
    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobj = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobj, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Get fractional variables, that should be integral.
    let (lpcands, _, _, nlpcands, _, _) = api::get_lp_branch_cands(scip)?;

    // Only call heuristic, if LP solution is fractional; except we are called
    // during pricing, in this case we want to detect a (mixed) integer (LP)
    // solution which is primal feasible.
    if nlpcands == 0 && heurtiming != HEURTIMING_DURINGPRICINGLOOP {
        return Ok(());
    }

    // Get the working solution from heuristic's local data.
    let sol = hd(scip, heur).sol;

    // Copy the current LP solution to the working solution.
    crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

    // Don't call heuristic, if we have already processed the current LP
    // solution.
    let nlps = scip.stat.nlps;
    if nlps == hd(scip, heur).lastlp {
        return Ok(());
    }
    hdm(scip, heur).lastlp = nlps;

    *result = ScipResultCode::DidNotFind;

    // Perform random rounding.
    perform_rand_rounding(scip, heur, sol, &lpcands, nlpcands as usize, propagate, result)
}

/*
 * Callback methods
 */

/// `heurCopyRandrounding`: copy method for primal heuristic plugins.
fn heur_copy_randrounding(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_randrounding(scip)
}

/// `heurInitRandrounding`: initialization method of primal heuristic.
fn heur_init_randrounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create heuristic data.
    let sol = api::create_sol(scip, heur)?;
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    let h = hdm(scip, heur);
    h.sol = sol;
    h.lastlp = -1;

    // Create random number generator.
    h.randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `heurExitRandrounding`: deinitialization method of primal heuristic.
fn heur_exit_randrounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free heuristic data.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    let h = hdm(scip, heur);
    h.sol = NONE;

    // Free random number generator.
    h.randnumgen = None;
    Ok(())
}

/// `heurInitsolRandrounding`: solving process initialization method of
/// primal heuristic.
fn heur_initsol_randrounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    hdm(scip, heur).lastlp = -1;

    // Change the heuristic's timingmask, if it should be called only once per
    // node.
    if hd(scip, heur).oncepernode {
        scip.heurs[heur].timingmask = HEURTIMING_AFTERLPNODE;
    }
    Ok(())
}

/// `heurExitsolRandrounding`: solving process deinitialization method of
/// primal heuristic.
fn heur_exitsol_randrounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset the timing mask to its default value.
    scip.heurs[heur].timingmask = HEUR_TIMING;
    Ok(())
}

/// `heurExecRandrounding`: execution method of primal heuristic.
fn heur_exec_randrounding(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
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
    if scip.stat.nlps == hd(scip, heur).lastlp {
        return Ok(());
    }

    let propagate = !hd(scip, heur).propagateonlyroot || api::get_depth(scip) == 0;

    // Try to round LP solution.
    perform_lp_rand_rounding(scip, heur, heurtiming, propagate, result)
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurRandrounding`: creates the rand rounding heuristic and
/// includes it in SCIP.
pub fn include_heur_randrounding(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // Create heuristic data.
    let heurdata = HeurData {
        sol: NONE,
        randnumgen: None,
        lastlp: -1,
        maxproprounds: DEFAULT_MAXPROPROUNDS,
        oncepernode: DEFAULT_ONCEPERNODE,
        usesimplerounding: DEFAULT_USESIMPLEROUNDING,
        propagateonlyroot: DEFAULT_PROPAGATEONLYROOT,
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
        heur_exec_randrounding,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_randrounding);
    h.heurinit = Some(heur_init_randrounding);
    h.heurexit = Some(heur_exit_randrounding);
    h.heurinitsol = Some(heur_initsol_randrounding);
    h.heurexitsol = Some(heur_exitsol_randrounding);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    paramset_add_bool(
        scip,
        "heuristics/randrounding/oncepernode",
        "should the heuristic only be called once per node?",
        t!(bool, oncepernode),
        true,
        DEFAULT_ONCEPERNODE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/randrounding/usesimplerounding",
        "should the heuristic apply the variable lock strategy of simple rounding, if possible?",
        t!(bool, usesimplerounding),
        true,
        DEFAULT_USESIMPLEROUNDING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/randrounding/propagateonlyroot",
        "should the probing part of the heuristic be applied exclusively at the root node?",
        t!(bool, propagateonlyroot),
        true,
        DEFAULT_PROPAGATEONLYROOT,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/randrounding/maxproprounds",
        "limit of rounds for each propagation call",
        t!(i32, maxproprounds),
        true,
        DEFAULT_MAXPROPROUNDS,
        -1,
        i32::MAX,
        None,
    )?;

    Ok(())
}
