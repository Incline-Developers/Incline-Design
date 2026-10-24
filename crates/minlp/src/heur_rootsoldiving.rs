//! Port of `heur_rootsoldiving.c`: LP diving heuristic that changes
//! variable's objective values using root LP solution as guide.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{HeurId, LpSolStat, ScipResultCode},
    var::{NONE, var_get_lb_lp, var_get_lp_sol, var_get_obj_lp, var_get_root_sol, var_get_ub_lp},
};

const HEUR_NAME: &str = "rootsoldiving";
const HEUR_DESC: &str = "LP diving heuristic that changes variable's objective values using root LP solution as guide";
const HEUR_DISPCHAR: u8 = b'o';
const HEUR_PRIORITY: i32 = -1005000;
const HEUR_FREQ: i32 = 20;
const HEUR_FREQOFS: i32 = 5;
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
/// Maximal fraction of diving LP iterations compared to node LP iterations.
const DEFAULT_MAXLPITERQUOT: f64 = 0.01;
/// Additional number of allowed LP iterations.
const DEFAULT_MAXLPITEROFS: i32 = 1000;
/// Total number of feasible solutions found up to which heuristic is called
/// (-1: no limit).
const DEFAULT_MAXSOLS: i32 = -1;
/// Maximal diving depth: number of binary/integer variables times depthfac.
const DEFAULT_DEPTHFAC: f64 = 0.5;
/// Maximal diving depth factor if no feasible solution was found yet.
const DEFAULT_DEPTHFACNOSOL: f64 = 2.0;

/// Minimal number of LP iterations allowed in each LP solving call.
const MINLPITER: i64 = 10000;
/// Soft rounding factor to fade out objective coefficients.
const DEFAULT_ALPHA: f64 = 0.9;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: crate::types::SolId,
    /// Minimal relative depth to start diving.
    minreldepth: f64,
    /// Maximal relative depth to start diving.
    maxreldepth: f64,
    /// Maximal fraction of diving LP iterations compared to node LP
    /// iterations.
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
    /// Soft rounding factor to fade out objective coefficients.
    alpha: f64,
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
 * Callback methods
 */

/// `heurCopyRootsoldiving`: copy method for primal heuristic plugins.
fn heur_copy_rootsoldiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_rootsoldiving(scip)
}

/// `heurInitRootsoldiving`: initialization method of primal heuristic.
fn heur_init_rootsoldiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    let h = hdm(scip, heur);
    h.sol = sol;

    // Initialize data.
    h.nlpiterations = 0;
    h.nsuccess = 0;

    Ok(())
}

/// `heurExitRootsoldiving`: deinitialization method of primal heuristic.
fn heur_exit_rootsoldiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;

    Ok(())
}

/// `heurExecRootsoldiving`: execution method of primal heuristic.
fn heur_exec_rootsoldiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
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

    // Get number of fractional variables, that should be integral.
    let mut nlpcands = api::get_n_lp_branch_cands(scip);

    // Don't try to dive, if there are no fractional variables.
    if nlpcands == 0 {
        return Ok(());
    }

    // Get all variables of LP.
    let vars = api::get_vars(scip).to_vec();
    let nvars = api::get_n_vars(scip) as usize;
    let nenfovars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip) - api::get_n_cont_impl_vars(scip)) as usize;

    // Calculate the maximal diving depth.
    let h = hd(scip, heur);
    let mut maxdivedepth = if api::get_n_sols_found(scip) == 0 {
        (h.depthfacnosol * nenfovars as f64) as i32
    } else {
        (h.depthfac * nenfovars as f64) as i32
    };
    maxdivedepth = if maxdivedepth >= 10 { maxdivedepth } else { 10 };

    *result = ScipResultCode::DidNotFind;

    // Get root solution value of all binary and integer variables.
    let rootsol: Vec<f64> = vars[..nenfovars].iter().map(|&v| var_get_root_sol(scip, v)).collect();

    // Get current LP objective value, and calculate length of a single step
    // in an objective coefficient.
    let mut absstartobjval = api::get_lp_objval(scip);
    absstartobjval = if absstartobjval >= 0.0 { absstartobjval } else { -absstartobjval };
    absstartobjval = if absstartobjval >= 1.0 { absstartobjval } else { 1.0 };
    let objstep = absstartobjval / 10.0;

    // Initialize array storing the preferred soft rounding directions and
    // counting the integral value rounds.
    let mut softroundings = vec![0i32; nenfovars];
    let mut intvalrounds = vec![0i32; nenfovars];

    // Allocate temporary memory for buffering objective changes.
    let mut objchgvals = vec![0.0f64; nenfovars];

    // Start diving.
    scip_lp::start_dive(scip)?;

    let mut lperror = false;
    let mut divedepth = 0;
    let mut lpsolstat = LpSolStat::Optimal;
    let alpha = hd(scip, heur).alpha;
    let mut ncycles = 0;
    let mut lpsolchanged = true;
    let startnlpcands = nlpcands;
    let sol = hd(scip, heur).sol;
    while !lperror
        && lpsolstat == LpSolStat::Optimal
        && nlpcands > 0
        && ncycles < 10
        && (divedepth < 10 || nlpcands <= startnlpcands - divedepth / 2 || (divedepth < maxdivedepth && hd(scip, heur).nlpiterations < maxnlpiterations))
        && !crate::solve::solve_is_stopped(scip, false)
    {
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

        divedepth += 1;
        let mut hardroundingidx: i32 = -1;
        let mut hardroundingdir = 0;
        let mut hardroundingoldbd = 0.0;
        let mut hardroundingnewbd = 0.0;
        let mut boundschanged = false;

        // Round solution x* from diving LP:
        //   - x~_j = down(x*_j)    if x*_j is integer or binary variable and
        //     x*_j <= root solution_j
        //   - x~_j = up(x*_j)      if x*_j is integer or binary variable and
        //     x*_j  > root solution_j
        //   - x~_j = x*_j          if x*_j is continuous variable
        // change objective function in diving LP:
        //   - if x*_j is integral, or j is a continuous variable, set
        //     obj'_j = alpha * obj_j
        //   - otherwise, set obj'_j = alpha * obj_j + sign(x*_j - x~_j)
        for i in 0..nenfovars {
            let var = vars[i];
            let oldobj = var_get_obj_lp(scip, var);
            let mut newobj = oldobj;

            let mut solval = var_get_lp_sol(scip, var);
            if scip.set.is_feas_integral(solval) {
                // If the variable became integral after a soft rounding,
                // count the rounds; after a while, fix it to its current
                // integral value; otherwise, fade out the objective value.
                if softroundings[i] != 0 && lpsolchanged {
                    intvalrounds[i] += 1;
                    if intvalrounds[i] == 5 && var_get_lb_lp(scip, var) < var_get_ub_lp(scip, var) - 0.5 {
                        // Use exact integral value, if the variable is only
                        // integral within numerical tolerances.
                        solval = scip.set.floor(solval + 0.5);
                        scip_lp::chg_var_lb_dive(scip, var, solval)?;
                        scip_lp::chg_var_ub_dive(scip, var, solval)?;
                        boundschanged = true;
                    }
                } else {
                    newobj = alpha * oldobj;
                }
            } else if solval <= rootsol[i] {
                // If the variable was soft rounded most of the time
                // downwards, round it downwards by changing the bounds;
                // otherwise, apply soft rounding by changing the objective
                // value.
                softroundings[i] -= 1;
                if softroundings[i] <= -10 && hardroundingidx == -1 {
                    hardroundingidx = i as i32;
                    hardroundingdir = -1;
                    hardroundingoldbd = var_get_ub_lp(scip, var);
                    hardroundingnewbd = scip.set.feas_floor(solval);
                    scip_lp::chg_var_ub_dive(scip, var, hardroundingnewbd)?;
                    boundschanged = true;
                } else {
                    newobj = alpha * oldobj + objstep;
                }
            } else {
                // If the variable was soft rounded most of the time upwards,
                // round it upwards by changing the bounds; otherwise, apply
                // soft rounding by changing the objective value.
                softroundings[i] += 1;
                if softroundings[i] >= 10 && hardroundingidx == -1 {
                    hardroundingidx = i as i32;
                    hardroundingdir = 1;
                    hardroundingoldbd = var_get_lb_lp(scip, var);
                    hardroundingnewbd = scip.set.feas_ceil(solval);
                    scip_lp::chg_var_lb_dive(scip, var, hardroundingnewbd)?;
                    boundschanged = true;
                } else {
                    newobj = alpha * oldobj - objstep;
                }
            }

            // Remember the objective change.
            objchgvals[i] = newobj;
        }

        // Apply objective changes if there was no bound change.
        if !boundschanged {
            // Apply cached changes on integer variables.
            for i in 0..nenfovars {
                scip_lp::chg_var_obj_dive(scip, vars[i], objchgvals[i])?;
            }

            // Fade out the objective values of the continuous variables.
            for &var in &vars[nenfovars..nvars] {
                let oldobj = var_get_obj_lp(scip, var);
                let newobj = alpha * oldobj;
                scip_lp::chg_var_obj_dive(scip, var, newobj)?;
            }
        }

        // SOLVEAGAIN:
        loop {
            // Resolve the diving LP.
            let nlpiterations = scip.stat.nlpiterations;
            let itlim = (maxnlpiterations - hd(scip, heur).nlpiterations) as i32;
            let itlim = if itlim >= MINLPITER as i32 { itlim } else { MINLPITER as i32 };
            // Errors in the LP solver should not kill the overall solving
            // process, if the LP is just needed for a heuristic (C catches
            // the return code in optimized mode and warns).
            match scip_lp::solve_dive_lp(scip, itlim) {
                Ok((e, _)) => lperror = e,
                Err(retcode) => {
                    log::warn!("Error while solving LP in Rootsoldiving heuristic; LP solve terminated with code <{retcode:?}>");
                    log::warn!("This does not affect the remaining solution procedure --> continue");
                }
            }
            lpsolstat = api::get_lp_solstat(scip);

            if lperror {
                break;
            }

            // Update iteration count.
            hdm(scip, heur).nlpiterations += scip.stat.nlpiterations - nlpiterations;

            // If no LP iterations were performed, we stayed at the same
            // solution -> count this cycling.
            lpsolchanged = scip.stat.nlpiterations != nlpiterations;
            if lpsolchanged {
                ncycles = 0;
            } else if !boundschanged {
                // Do not count if integral variables have been fixed.
                ncycles += 1;
            }

            // Get LP solution status and number of fractional variables, that
            // should be integral.
            if lpsolstat == LpSolStat::Infeasible && hardroundingidx != -1 {
                let var = vars[hardroundingidx as usize];

                // Round the hard rounded variable to the opposite direction
                // and resolve the LP.
                if hardroundingdir == -1 {
                    scip_lp::chg_var_ub_dive(scip, var, hardroundingoldbd)?;
                    scip_lp::chg_var_lb_dive(scip, var, hardroundingnewbd + 1.0)?;
                } else {
                    scip_lp::chg_var_lb_dive(scip, var, hardroundingoldbd)?;
                    scip_lp::chg_var_ub_dive(scip, var, hardroundingnewbd - 1.0)?;
                }
                hardroundingidx = -1;
                continue;
            }
            break;
        }

        if lperror {
            break;
        }

        if lpsolstat == LpSolStat::Optimal {
            nlpcands = api::get_n_lp_branch_cands(scip);
        }
    }

    // Check if a solution has been found.
    if nlpcands == 0 && !lperror && lpsolstat == LpSolStat::Optimal {
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

/// `SCIPincludeHeurRootsoldiving`: creates the rootsoldiving heuristic and
/// includes it in SCIP.
pub fn include_heur_rootsoldiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_rootsoldiving,
        Some(Box::new(HeurData {
            sol: NONE,
            minreldepth: DEFAULT_MINRELDEPTH,
            maxreldepth: DEFAULT_MAXRELDEPTH,
            maxlpiterquot: DEFAULT_MAXLPITERQUOT,
            maxlpiterofs: DEFAULT_MAXLPITEROFS,
            maxsols: DEFAULT_MAXSOLS,
            depthfac: DEFAULT_DEPTHFAC,
            depthfacnosol: DEFAULT_DEPTHFACNOSOL,
            alpha: DEFAULT_ALPHA,
            nlpiterations: 0,
            nsuccess: 0,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_rootsoldiving);
    h.heurinit = Some(heur_init_rootsoldiving);
    h.heurexit = Some(heur_exit_rootsoldiving);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Rootsoldiving heuristic parameters.
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_int, paramset_add_real},
    };
    paramset_add_real(
        scip,
        "heuristics/rootsoldiving/minreldepth",
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
        "heuristics/rootsoldiving/maxreldepth",
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
        "heuristics/rootsoldiving/maxlpiterquot",
        "maximal fraction of diving LP iterations compared to node LP iterations",
        t!(f64, maxlpiterquot),
        false,
        DEFAULT_MAXLPITERQUOT,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/rootsoldiving/maxlpiterofs",
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
        "heuristics/rootsoldiving/maxsols",
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
        "heuristics/rootsoldiving/depthfac",
        "maximal diving depth: number of binary/integer variables times depthfac",
        t!(f64, depthfac),
        true,
        DEFAULT_DEPTHFAC,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rootsoldiving/depthfacnosol",
        "maximal diving depth factor if no feasible solution was found yet",
        t!(f64, depthfacnosol),
        true,
        DEFAULT_DEPTHFACNOSOL,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/rootsoldiving/alpha",
        "soft rounding factor to fade out objective coefficients",
        t!(f64, alpha),
        true,
        DEFAULT_ALPHA,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
