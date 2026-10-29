//! Port of `heur_objpscostdiving.c`: LP diving heuristic that changes
//! variable's objective value instead of bounds, using pseudo costs as
//! guide.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    misc::RandNumGen,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{HeurId, LpSolStat, ScipResultCode, VarId},
    var::{NONE, var_is_binary, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "objpscostdiving";
const HEUR_DESC: &str = "LP diving heuristic that changes variable's objective values instead of bounds, using pseudo costs as guide";
const HEUR_DISPCHAR: u8 = b'o';
const HEUR_PRIORITY: i32 = -1004000;
const HEUR_FREQ: i32 = 20;
const HEUR_FREQOFS: i32 = 4;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/*
 * Default parameter settings
 */

/// Minimal relative depth to start diving.
const DEFAULT_MINRELDEPTH: f64 = 0.0;
/// Maximal relative depth to start diving.
const DEFAULT_MAXRELDEPTH: f64 = 1.0;
/// Maximal fraction of diving LP iterations compared to total iteration
/// number.
const DEFAULT_MAXLPITERQUOT: f64 = 0.05;
/// Additional number of allowed LP iterations.
const DEFAULT_MAXLPITEROFS: i32 = 1000;
/// Total number of feasible solutions found up to which heuristic is called
/// (-1: no limit).
const DEFAULT_MAXSOLS: i32 = -1;
/// Maximal diving depth: number of binary/integer variables times depthfac.
const DEFAULT_DEPTHFAC: f64 = 0.5;
/// Maximal diving depth factor if no feasible solution was found yet.
const DEFAULT_DEPTHFACNOSOL: f64 = 2.0;
/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 139;

/// Minimal number of LP iterations allowed in each LP solving call.
const MINLPITER: i64 = 10000;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: crate::types::SolId,
    /// Random number generator.
    randnumgen: Option<RandNumGen>,
    /// Minimal relative depth to start diving.
    minreldepth: f64,
    /// Maximal relative depth to start diving.
    maxreldepth: f64,
    /// Maximal fraction of diving LP iterations compared to total iteration
    /// number.
    maxlpiterquot: f64,
    /// Additional number of allowed LP iterations.
    maxlpiterofs: i32,
    /// Total number of feasible solutions found up to which heuristic is
    /// called (-1: no limit).
    maxsols: i32,
    /// Maximal diving depth: number of binary/integer variables times
    /// depthfac.
    depthfac: f64,
    /// Maximal diving depth factor if no feasible solution was found yet.
    depthfacnosol: f64,
    /// LP iterations used in this heuristic.
    nlpiterations: i64,
    /// Number of runs that produced at least one feasible solution.
    nsuccess: i32,
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

/// `calcPscostQuot`: returns (pscostquot, roundup); rounddir -1: round
/// down, +1: round up, 0: select due to pseudo cost values.
fn calc_pscost_quot(scip: &mut Scip, heur: HeurId, var: VarId, primsol: f64, mut frac: f64, rounddir: i32) -> (f64, bool) {
    // Bound fractions to not prefer variables that are nearly integral.
    frac = if frac >= 0.1 { frac } else { 0.1 };
    frac = if frac <= 0.9 { frac } else { 0.9 };

    // Get pseudo cost quotient.
    let pscostdown = crate::var::var_get_pseudocost(scip, var, 0.0 - frac);
    let pscostup = crate::var::var_get_pseudocost(scip, var, 1.0 - frac);
    let rootsol = crate::var::var_get_root_sol(scip, var);

    // Choose rounding direction.
    //
    // To avoid performance variability caused by numerics we use random
    // numbers to decide whether we want to roundup or round down if the
    // values to compare are equal within tolerances.
    let set = &scip.set;
    let rng = scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap().randnumgen.as_mut().unwrap();
    let roundup = if rounddir == -1 {
        false
    } else if rounddir == 1 {
        true
    } else if set.is_lt(frac, 0.3) || (set.is_eq(frac, 0.3) && rng.get_int(0, 1) == 0) {
        false
    } else if set.is_gt(frac, 0.7) || (set.is_eq(frac, 0.7) && rng.get_int(0, 1) == 0) {
        true
    } else if set.is_lt(primsol, rootsol - 0.4) || (set.is_eq(primsol, rootsol - 0.4) && rng.get_int(0, 1) == 0) {
        false
    } else if set.is_gt(primsol, rootsol + 0.4) || (set.is_eq(primsol, rootsol + 0.4) && rng.get_int(0, 1) == 0) {
        true
    } else {
        !(set.is_lt(pscostdown, pscostup) || (set.is_eq(pscostdown, pscostup) && rng.get_int(0, 1) == 0))
    };

    // Calculate pseudo cost quotient.
    let mut pscostquot = if roundup {
        frac.sqrt() * (1.0 + pscostdown) / (1.0 + pscostup)
    } else {
        (1.0 - frac).sqrt() * (1.0 + pscostup) / (1.0 + pscostdown)
    };

    // Prefer decisions on binary variables.
    if var_is_binary(scip, var) {
        pscostquot *= 1000.0;
    }

    (pscostquot, roundup)
}

/*
 * Callback methods
 */

/// `heurCopyObjpscostdiving`: copy method for primal heuristic plugins.
fn heur_copy_objpscostdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_objpscostdiving(scip)
}

/// `heurInitObjpscostdiving`: initialization method of primal heuristic.
fn heur_init_objpscostdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;

    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    let h = hdm(scip, heur);
    h.sol = sol;
    h.randnumgen = Some(RandNumGen::new(seed));

    // Initialize data.
    h.nlpiterations = 0;
    h.nsuccess = 0;

    Ok(())
}

/// `heurExitObjpscostdiving`: deinitialization method of primal heuristic.
fn heur_exit_objpscostdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free random number generator.
    hdm(scip, heur).randnumgen = None;

    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;

    Ok(())
}

/// `heurExecObjpscostdiving`: execution method of primal heuristic.
fn heur_exec_objpscostdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Delayed;

    // Do not call heuristic of node was already detected to be infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Only call heuristic, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobjval = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobjval, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Only call heuristic, if the LP solution is basic (which allows fast
    // resolve in diving).
    if !api::is_lp_sol_basic(scip) {
        return Ok(());
    }

    // Don't dive two times at the same node.
    if scip.stat.lastdivenode == api::get_n_nodes(scip) && api::get_depth(scip) > 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotRun;

    // Only apply heuristic, if only a few solutions have been found.
    let maxsols = hd(scip, heur).maxsols;
    if maxsols >= 0 && api::get_n_sols_found(scip) >= maxsols as i64 {
        return Ok(());
    }

    // Only try to dive, if we are in the correct part of the tree, given by
    // minreldepth and maxreldepth.
    let depth = api::get_depth(scip);
    let mut maxdepth = api::get_max_depth(scip);
    maxdepth = if maxdepth >= 30 { maxdepth } else { 30 };
    let (minreldepth, maxreldepth) = (hd(scip, heur).minreldepth, hd(scip, heur).maxreldepth);
    if (depth as f64) < minreldepth * maxdepth as f64 || depth as f64 > maxreldepth * maxdepth as f64 {
        return Ok(());
    }

    // Calculate the maximal number of LP iterations until heuristic is
    // aborted.
    let nlpiterations = api::get_n_node_lp_iterations(scip);
    let ncalls = scip.heurs[heur].ncalls;
    let nsolsfound = 10 * scip.heurs[heur].nbestsolsfound + hd(scip, heur).nsuccess as i64;
    let h = hd(scip, heur);
    let mut maxnlpiterations = (((nsolsfound as f64 + 1.0) / (ncalls as f64 + 1.0)) * h.maxlpiterquot * nlpiterations as f64) as i64;
    maxnlpiterations += h.maxlpiterofs as i64;

    // Don't try to dive, if we took too many LP iterations during diving.
    if h.nlpiterations >= maxnlpiterations {
        return Ok(());
    }

    // Allow at least a certain number of LP iterations in this dive.
    maxnlpiterations = if maxnlpiterations >= h.nlpiterations + MINLPITER {
        maxnlpiterations
    } else {
        h.nlpiterations + MINLPITER
    };

    // Get fractional variables that should be integral.
    let (mut lpcands, mut lpcandssol, mut lpcandsfrac, mut nlpcands, _, _) = api::get_lp_branch_cands(scip)?;

    // Don't try to dive, if there are no fractional variables.
    if nlpcands == 0 {
        return Ok(());
    }

    // Calculate the maximal diving depth.
    let nvars = api::get_n_vars(scip) - api::get_n_cont_vars(scip) - api::get_n_cont_impl_vars(scip);
    let h = hd(scip, heur);
    let mut maxdivedepth = if api::get_n_sols_found(scip) == 0 {
        (h.depthfacnosol * nvars as f64) as i32
    } else {
        (h.depthfac * nvars as f64) as i32
    };
    maxdivedepth = if maxdivedepth <= 10 * maxdepth { maxdivedepth } else { 10 * maxdepth };

    *result = ScipResultCode::DidNotFind;

    // Get temporary memory for remembering the current soft roundings.
    let mut roundings = vec![0i32; nvars as usize];

    // Start diving.
    scip_lp::start_dive(scip)?;

    // Dive as long we are in the given diving depth and iteration limits and
    // fractional variables exist, but
    // - if the last objective change was in a direction, that corresponds to
    //   a feasible rounding, we continue in any case
    // - if possible, we dive at least with the depth 10
    // - if the number of fractional variables decreased at least with 1
    //   variable per 2 dive depths, we continue diving
    let mut lperror = false;
    let mut lpsolstat = LpSolStat::Optimal;
    let mut divedepth = 0;
    let startnlpcands = nlpcands;
    while !lperror
        && lpsolstat == LpSolStat::Optimal
        && nlpcands > 0
        && (divedepth < 10
            || nlpcands <= startnlpcands - divedepth / 2
            || (divedepth < maxdivedepth && nlpcands <= startnlpcands - divedepth / 10 && hd(scip, heur).nlpiterations < maxnlpiterations))
        && !crate::solve::solve_is_stopped(scip, false)
    {
        divedepth += 1;

        // Choose variable for objective change:
        // - prefer variables that may not be rounded without destroying LP
        //   feasibility:
        //   - of these variables, change objective value of variable with
        //     largest rel. difference of pseudo cost values
        // - if all remaining fractional variables may be rounded without
        //   destroying LP feasibility:
        //   - change objective value of variable with largest rel. difference
        //     of pseudo cost values
        let mut bestcand: i32 = -1;
        let mut bestpscostquot = -1.0;
        let mut bestcandmayrounddown = true;
        let mut bestcandmayroundup = true;
        let mut bestcandroundup = false;
        for c in 0..nlpcands as usize {
            let var = lpcands[c];
            let mayrounddown = var_may_round_down(scip, var);
            let mayroundup = var_may_round_up(scip, var);
            let primsol = lpcandssol[c];
            let frac = lpcandsfrac[c];
            if mayrounddown || mayroundup {
                // The candidate may be rounded: choose this candidate only, if
                // the best candidate may also be rounded.
                if bestcandmayrounddown || bestcandmayroundup {
                    // Choose rounding direction:
                    // - if variable may be rounded in both directions, round
                    //   corresponding to the pseudo cost values
                    // - otherwise, round in the infeasible direction, because
                    //   feasible direction is tried by rounding the current
                    //   fractional solution
                    let (mut pscostquot, roundup) = if mayrounddown && mayroundup {
                        calc_pscost_quot(scip, heur, var, primsol, frac, 0)
                    } else if mayrounddown {
                        calc_pscost_quot(scip, heur, var, primsol, frac, 1)
                    } else {
                        calc_pscost_quot(scip, heur, var, primsol, frac, -1)
                    };

                    // Prefer variables, that have already been soft rounded
                    // but failed to get integral.
                    let varidx = scip.vars[var].probindex as usize;
                    if roundings[varidx] != 0 {
                        pscostquot *= 1000.0;
                    }

                    // Check, if candidate is new best candidate.
                    if pscostquot > bestpscostquot {
                        bestcand = c as i32;
                        bestpscostquot = pscostquot;
                        bestcandmayrounddown = mayrounddown;
                        bestcandmayroundup = mayroundup;
                        bestcandroundup = roundup;
                    }
                }
            } else {
                // The candidate may not be rounded: calculate pseudo cost
                // quotient and preferred direction.
                let (mut pscostquot, roundup) = calc_pscost_quot(scip, heur, var, primsol, frac, 0);

                // Prefer variables, that have already been soft rounded but
                // failed to get integral.
                let varidx = scip.vars[var].probindex as usize;
                if roundings[varidx] != 0 {
                    pscostquot *= 1000.0;
                }

                // Check, if candidate is new best candidate: prefer
                // unroundable candidates in any case.
                if bestcandmayrounddown || bestcandmayroundup || pscostquot > bestpscostquot {
                    bestcand = c as i32;
                    bestpscostquot = pscostquot;
                    bestcandmayrounddown = false;
                    bestcandmayroundup = false;
                    bestcandroundup = roundup;
                }
            }
        }

        // If all candidates are roundable, try to round the solution.
        if bestcandmayrounddown || bestcandmayroundup {
            let sol = hd(scip, heur).sol;

            // Create solution from diving LP and try to round it.
            crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;
            let success = api::round_sol(scip, sol)?;

            if success {
                // Try to add solution to SCIP.
                let success = api::try_sol(scip, sol, false, false, false, false, false)?;

                // Check, if solution was feasible and good enough.
                if success {
                    *result = ScipResultCode::FoundSol;
                }
            }
        }

        let bestcand = bestcand as usize;
        let var = lpcands[bestcand];

        // Check, if the best candidate was already subject to soft rounding.
        let varidx = scip.vars[var].probindex as usize;
        if roundings[varidx] == 1 {
            // Variable was already soft rounded upwards: hard round it
            // downwards.
            let newub = scip.set.feas_floor(lpcandssol[bestcand]);
            scip_lp::chg_var_ub_dive(scip, var, newub)?;
        } else if roundings[varidx] == -1 {
            // Variable was already soft rounded downwards: hard round it
            // upwards.
            let newlb = scip.set.feas_ceil(lpcandssol[bestcand]);
            scip_lp::chg_var_lb_dive(scip, var, newlb)?;
        } else {
            // Apply soft rounding of best candidate via a change in the
            // objective value.
            let objscale = divedepth as f64 * 1000.0;
            let oldobj = crate::var::var_get_obj_lp(scip, var);
            let mut newobj;
            if bestcandroundup {
                // Soft round variable up: make objective value (more)
                // negative.
                newobj = if oldobj < 0.0 { objscale * oldobj } else { -objscale * oldobj };
                newobj = if newobj <= -objscale { newobj } else { -objscale };

                // Remember, that this variable was soft rounded upwards.
                roundings[varidx] = 1;
            } else {
                // Soft round variable down: make objective value (more)
                // positive.
                newobj = if oldobj > 0.0 { objscale * oldobj } else { -objscale * oldobj };
                newobj = if newobj >= objscale { newobj } else { objscale };

                // Remember, that this variable was soft rounded downwards.
                roundings[varidx] = -1;
            }
            scip_lp::chg_var_obj_dive(scip, var, newobj)?;
        }

        // Resolve the diving LP.
        let nlpiterations = scip.stat.nlpiterations;
        let itlim = (maxnlpiterations - hd(scip, heur).nlpiterations) as i32;
        let itlim = if itlim >= MINLPITER as i32 { itlim } else { MINLPITER as i32 };
        // Errors in the LP solver should not kill the overall solving
        // process, if the LP is just needed for a heuristic (C catches the
        // return code in optimized mode and warns).
        match scip_lp::solve_dive_lp(scip, itlim) {
            Ok((e, _)) => lperror = e,
            Err(retcode) => {
                log::warn!("Error while solving LP in Objpscostdiving heuristic; LP solve terminated with code <{retcode:?}>");
                log::warn!("This does not affect the remaining solution procedure --> continue");
            }
        }
        lpsolstat = api::get_lp_solstat(scip);

        if lperror {
            break;
        }

        // Update iteration count.
        hdm(scip, heur).nlpiterations += scip.stat.nlpiterations - nlpiterations;

        // Get LP solution status and fractional variables, that should be
        // integral.
        if lpsolstat == LpSolStat::Optimal {
            // Get new fractional variables.
            (lpcands, lpcandssol, lpcandsfrac, nlpcands, _, _) = api::get_lp_branch_cands(scip)?;
        }
    }

    // Check if a solution has been found.
    if nlpcands == 0 && !lperror && lpsolstat == LpSolStat::Optimal {
        let sol = hd(scip, heur).sol;

        // Create solution from diving LP.
        crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

        // Try to add solution to SCIP.
        let success = api::try_sol(scip, sol, false, false, false, false, false)?;

        // Check, if solution was feasible and good enough.
        if success {
            *result = ScipResultCode::FoundSol;
        }
    }

    // End diving.
    if scip_lp::in_dive(scip) {
        scip_lp::end_dive(scip)?;
    }

    if *result == ScipResultCode::FoundSol {
        hdm(scip, heur).nsuccess += 1;
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurObjpscostdiving`: creates the objpscostdiving heuristic
/// and includes it in SCIP.
pub fn include_heur_objpscostdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_objpscostdiving,
        Some(Box::new(HeurData {
            sol: NONE,
            randnumgen: None,
            minreldepth: DEFAULT_MINRELDEPTH,
            maxreldepth: DEFAULT_MAXRELDEPTH,
            maxlpiterquot: DEFAULT_MAXLPITERQUOT,
            maxlpiterofs: DEFAULT_MAXLPITEROFS,
            maxsols: DEFAULT_MAXSOLS,
            depthfac: DEFAULT_DEPTHFAC,
            depthfacnosol: DEFAULT_DEPTHFACNOSOL,
            nlpiterations: 0,
            nsuccess: 0,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_objpscostdiving);
    h.heurinit = Some(heur_init_objpscostdiving);
    h.heurexit = Some(heur_exit_objpscostdiving);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Objpscostdiving heuristic parameters.
    use crate::paramset::{paramset_add_int, paramset_add_real};
    paramset_add_real(
        scip,
        "heuristics/objpscostdiving/minreldepth",
        "minimal relative depth to start diving",
        t!(f64, minreldepth),
        true,
        DEFAULT_MINRELDEPTH,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/objpscostdiving/maxreldepth",
        "maximal relative depth to start diving",
        t!(f64, maxreldepth),
        true,
        DEFAULT_MAXRELDEPTH,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/objpscostdiving/maxlpiterquot",
        "maximal fraction of diving LP iterations compared to total iteration number",
        t!(f64, maxlpiterquot),
        false,
        DEFAULT_MAXLPITERQUOT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/objpscostdiving/maxlpiterofs",
        "additional number of allowed LP iterations",
        t!(i32, maxlpiterofs),
        false,
        DEFAULT_MAXLPITEROFS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/objpscostdiving/maxsols",
        "total number of feasible solutions found up to which heuristic is called (-1: no limit)",
        t!(i32, maxsols),
        true,
        DEFAULT_MAXSOLS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/objpscostdiving/depthfac",
        "maximal diving depth: number of binary/integer variables times depthfac",
        t!(f64, depthfac),
        true,
        DEFAULT_DEPTHFAC,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/objpscostdiving/depthfacnosol",
        "maximal diving depth factor if no feasible solution was found yet",
        t!(f64, depthfacnosol),
        true,
        DEFAULT_DEPTHFACNOSOL,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;

    Ok(())
}
