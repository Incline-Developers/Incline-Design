//! Port of `heur_feaspump.c`: objective feasibility pump 2.0.
//!
//! The round-and-propagate scheme (`usefp20`) and the local branching
//! sub-MIP (`stage3`) need a copied SCIP and are not ported; both are off
//! by default and return `NotImplemented` when switched on.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPPLUNGE, HEURTIMING_DURINGLPLOOP, HeurTiming},
    misc::RandNumGen,
    prob::TRANSPROB,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_lp,
    types::{HeurId, LpSolStat, ScipResultCode, SolId, VarId},
    var::{NONE, var_get_lp_sol},
};

const HEUR_NAME: &str = "feaspump";
const HEUR_DESC: &str = "objective feasibility pump 2.0";
const HEUR_DISPCHAR: u8 = b'o';
const HEUR_PRIORITY: i32 = -1000000;
const HEUR_FREQ: i32 = 20;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Maximal fraction of diving LP iterations compared to node LP iterations.
const DEFAULT_MAXLPITERQUOT: f64 = 0.05;
/// Additional number of allowed LP iterations.
const DEFAULT_MAXLPITEROFS: i32 = 1000;
/// Total number of feasible solutions found up to which heuristic is called
/// (-1: no limit).
const DEFAULT_MAXSOLS: i32 = 10;
/// Maximal number of pumping rounds (-1: no limit).
const DEFAULT_MAXLOOPS: i32 = 10000;
/// Maximal number of pumping rounds without fractionality improvement (-1:
/// no limit).
const DEFAULT_MAXSTALLLOOPS: i32 = 10;
/// Minimum number of random variables to flip, if a 1-cycle is encountered.
const DEFAULT_MINFLIPS: i32 = 10;
/// Maximum length of cycles to be checked explicitly in each round.
const DEFAULT_CYCLELENGTH: i32 = 3;
/// Number of iterations until a random perturbation is forced.
const DEFAULT_PERTURBFREQ: i32 = 100;
/// Factor by which the regard of the objective is decreased in each round,
/// 1.0 for dynamic, depending on solutions already found.
const DEFAULT_OBJFACTOR: f64 = 0.1;
/// Initial weight of the objective function in the convex combination.
const DEFAULT_ALPHA: f64 = 1.0;
/// Threshold difference for the convex parameter to perform perturbation.
const DEFAULT_ALPHADIFF: f64 = 1.0;
/// Should the feasibility pump be called at root node before cut
/// separation?
const DEFAULT_BEFORECUTS: bool = true;
/// Should an iterative round-and-propagate scheme be used to find the
/// integral points?
const DEFAULT_USEFP20: bool = false;
/// Should a random perturbation be performed if a feasible solution was
/// found?
const DEFAULT_PERTSOLFOUND: bool = true;
/// Should we solve a local branching sub-MIP if no solution could be found?
const DEFAULT_STAGE3: bool = false;
/// Radius of the neighborhood to be searched in stage 3.
const DEFAULT_NEIGHBORHOODSIZE: i32 = 18;
/// Should all active cuts from the cutpool of the original SCIP be copied to
/// constraints of the subscip.
const DEFAULT_COPYCUTS: bool = true;

/// Minimal number of LP iterations allowed in each LP solving call.
const MINLPITER: i64 = 5000;

/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 13;

/// Primal heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    /// Rounded solution.
    roundedsol: SolId,
    /// Number of LP iterations used in this heuristic.
    nlpiterations: i64,
    /// Maximal fraction of diving LP iterations compared to node LP
    /// iterations.
    maxlpiterquot: f64,
    /// Factor by which the regard of the objective is decreased in each
    /// round, 1.0 for dynamic, depending on solutions already found.
    objfactor: f64,
    /// Initial weight of the objective function in the convex combination.
    alpha: f64,
    /// Threshold difference for the convex parameter to perform
    /// perturbation.
    alphadiff: f64,

    /// Additional number of allowed LP iterations.
    maxlpiterofs: i32,
    /// Total number of feasible solutions found up to which heuristic is
    /// called (-1: no limit).
    maxsols: i32,
    /// Maximum number of loops (-1: no limit).
    maxloops: i32,
    /// Maximal number of pumping rounds without fractionality improvement
    /// (-1: no limit).
    maxstallloops: i32,
    /// Minimum number of random variables to flip, if a 1-cycle is
    /// encountered.
    minflips: i32,
    /// Maximum length of cycles to be checked explicitly in each round.
    cyclelength: i32,
    /// Number of iterations until a random perturbation is forced.
    perturbfreq: i32,
    /// Number of runs that produced at least one feasible solution.
    nsuccess: i32,
    /// Radius of the neighborhood to be searched in stage 3.
    neighborhoodsize: i32,

    /// Random number generator.
    randnumgen: Option<RandNumGen>,
    /// Should the feasibility pump be called at root node before cut
    /// separation?
    beforecuts: bool,
    /// Should an iterative round-and-propagate scheme be used to find the
    /// integral points?
    usefp20: bool,
    /// Should a random perturbation be performed if a feasible solution was
    /// found?
    pertsolfound: bool,
    /// Should we solve a local branching sub-MIP if no solution could be
    /// found?
    stage3: bool,
    /// Should all active cuts from cutpool be copied to constraints in
    /// subproblem?
    copycuts: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

fn rng(scip: &mut Scip, heur: HeurId) -> &mut RandNumGen {
    hdm(scip, heur).randnumgen.as_mut().unwrap()
}

/// `insertFlipCand`: checks whether a variable is one of the currently most
/// fractional ones.
fn insert_flip_cand(mostfracvars: &mut [VarId], mostfracvals: &mut [f64], nflipcands: &mut i32, maxnflipcands: i32, var: VarId, mut frac: f64) {
    // Instead of the fractional value use the fractionality.
    if frac > 0.5 {
        frac = 1.0 - frac;
    }

    // If there are already enough candidates and the variable is less
    // fractional, return, else reserve the last entry.
    if *nflipcands >= maxnflipcands {
        if frac <= mostfracvals[(*nflipcands - 1) as usize] {
            return;
        } else {
            *nflipcands -= 1;
        }
    }

    // Shifting var and frac through the (sorted) arrays.
    let mut i = *nflipcands as usize;
    while i > 0 && mostfracvals[i - 1] < frac {
        mostfracvars[i] = mostfracvars[i - 1];
        mostfracvals[i] = mostfracvals[i - 1];
        i -= 1;
    }

    // Insert the variable and its fractionality.
    mostfracvars[i] = var;
    mostfracvals[i] = frac;

    // We've found another candidate.
    *nflipcands += 1;
}

/// `updateVariableRounding`: set solution value in rounded solution and
/// update objective coefficient accordingly.
fn update_variable_rounding(scip: &mut Scip, heur: HeurId, var: VarId, solval: f64, alpha: f64, scalingfactor: f64) -> ScipResult {
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    // Update rounded solution.
    let roundedsol = hd(scip, heur).roundedsol;
    api::set_sol_val(scip, roundedsol, var, solval)?;

    // Modify objective towards rounded solution value if it is at one of the
    // variable bounds.
    let orgobjcoeff = scip.vars[var].obj;
    let newobjcoeff = if scip.set.is_eq(solval, lb) {
        (1.0 - alpha) / scalingfactor + alpha * orgobjcoeff
    } else if scip.set.is_eq(solval, ub) {
        -(1.0 - alpha) / scalingfactor + alpha * orgobjcoeff
    } else {
        alpha * orgobjcoeff
    };

    scip_lp::chg_var_obj_dive(scip, var, newobjcoeff)
}

/// `handle1Cycle`: flips the roundings of the most fractional variables, if
/// a 1-cycle was found.
fn handle_1_cycle(scip: &mut Scip, heur: HeurId, mostfracvars: &[VarId], nflipcands: i32, alpha: f64, scalingfactor: f64) -> ScipResult {
    // Flip rounding to the opposite side.
    for &var in &mostfracvars[..nflipcands as usize] {
        let mut solval = var_get_lp_sol(scip, var);
        let roundedsolval = api::get_sol_val(scip, hd(scip, heur).roundedsol, var);

        // Flip to the opposite rounded solution value.
        if roundedsolval > solval {
            solval = scip.set.feas_floor(solval);
        } else {
            solval = scip.set.feas_ceil(solval);
        }

        update_variable_rounding(scip, heur, var, solval, alpha, scalingfactor)?;
    }
    Ok(())
}

/// `handleCycle`: flips the roundings of randomly chosen fractional
/// variables, preferring highly fractional ones, if a longer cycle was
/// found.
fn handle_cycle(scip: &mut Scip, heur: HeurId, vars: &[VarId], nbinandintvars: usize, alpha: f64, scalingfactor: f64) -> ScipResult {
    // Flip variables randomized biased on their fractionality.
    for &var in &vars[..nbinandintvars] {
        let mut solval = var_get_lp_sol(scip, var);

        // Skip variables with integral solution values.
        if scip.set.is_feas_integral(solval) {
            continue;
        }

        let frac = scip.set.feas_frac(solval);
        let flipprob = rng(scip, heur).get_real(-0.3, 0.7);

        // Flip, iff the sum of the randomized number and the fractionality
        // is big enough.
        let minfrac = if frac <= 1.0 - frac { frac } else { 1.0 - frac };
        let maxprob = if flipprob >= 0.0 { flipprob } else { 0.0 };
        if minfrac + maxprob > 0.5 {
            let roundedsolval = api::get_sol_val(scip, hd(scip, heur).roundedsol, var);

            // Flip the solution to the opposite side.
            if roundedsolval > solval {
                solval = scip.set.floor(solval);
            } else {
                solval = scip.set.ceil(solval);
            }

            // Update rounded solution value and objective coefficient.
            update_variable_rounding(scip, heur, var, solval, alpha, scalingfactor)?;
        }
    }

    Ok(())
}

/// `heurCopyFeaspump`: copy method for primal heuristic plugins.
fn heur_copy_feaspump(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_feaspump(scip)
}

/// `heurInitFeaspump`: initialization method of primal heuristic.
fn heur_init_feaspump(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    let roundedsol = api::create_sol(scip, heur)?;

    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    let h = hdm(scip, heur);
    h.sol = sol;
    h.roundedsol = roundedsol;

    // Initialize data.
    h.nlpiterations = 0;
    h.nsuccess = 0;

    h.randnumgen = Some(RandNumGen::new(seed));

    Ok(())
}

/// `heurExitFeaspump`: deinitialization method of primal heuristic.
fn heur_exit_feaspump(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let (sol, roundedsol) = (hd(scip, heur).sol, hd(scip, heur).roundedsol);
    api::free_sol(scip, sol)?;
    api::free_sol(scip, roundedsol)?;
    let h = hdm(scip, heur);
    h.sol = NONE;
    h.roundedsol = NONE;

    // Free random number generator.
    h.randnumgen = None;

    Ok(())
}

/// `heurInitsolFeaspump`: solving process initialization method of primal
/// heuristic.
fn heur_initsol_feaspump(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // If the heuristic is called at the root node, we may want to be called
    // directly after the initial root LP solve.
    if hd(scip, heur).beforecuts && scip.heurs[heur].freqofs == 0 {
        scip.heurs[heur].timingmask = HEURTIMING_DURINGLPLOOP;
    }

    Ok(())
}

/// `heurExitsolFeaspump`: solving process deinitialization method of primal
/// heuristic.
fn heur_exitsol_feaspump(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset the timing mask to its default value.
    scip.heurs[heur].timingmask = HEUR_TIMING;

    Ok(())
}

/// `adjustedMaxNLPIterations`: calculates an adjusted maximal number of LP
/// iterations.
fn adjusted_max_nlp_iterations(maxnlpiterations: i64, nsolsfound: i64, nstallloops: i32) -> i64 {
    if nstallloops <= 1 {
        if nsolsfound == 0 { 4 * maxnlpiterations } else { 2 * maxnlpiterations }
    } else {
        maxnlpiterations
    }
}

/// `heurExecFeaspump`: execution method of primal heuristic.
fn heur_exec_feaspump(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
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

    *result = ScipResultCode::DidNotRun;

    // Don't dive two times at the same node.
    if scip.stat.lastdivenode == api::get_n_nodes(scip) && api::get_depth(scip) > 0 {
        return Ok(());
    }

    // Only call feaspump once at the root.
    if api::get_depth(scip) == 0 && scip.heurs[heur].ncalls > 0 {
        return Ok(());
    }

    // Reset the timing mask to its default value (at the root node it could
    // be different).
    scip.heurs[heur].timingmask = HEUR_TIMING;

    // Only call the heuristic before cutting planes if we do not have an
    // incumbent and no pricer exists (no pricers are ported, so their number
    // is 0).
    let npricers = 0;
    if heurtiming == HEURTIMING_DURINGLPLOOP && api::get_n_sols_found(scip) > 0 && npricers == 0 {
        return Ok(());
    }

    // Only apply heuristic, if only a few solutions have been found and no
    // pricer exists.
    let maxsols = hd(scip, heur).maxsols;
    if maxsols >= 0 && api::get_n_sols_found(scip) > maxsols as i64 && npricers == 0 {
        return Ok(());
    }

    // Get all variables of LP and number of fractional variables in LP
    // solution that should be integral.
    let vars = api::get_vars(scip).to_vec();
    let nvars = api::get_n_vars(scip) as usize;
    let nenfovars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip) - api::get_n_cont_impl_vars(scip)) as usize;
    let mut nfracs = api::get_n_lp_branch_cands(scip);
    if nfracs == 0 {
        return Ok(());
    }

    // Calculate the maximal number of LP iterations until heuristic is
    // aborted.
    let nlpiterations = api::get_n_node_lp_iterations(scip);
    let ncalls = scip.heurs[heur].ncalls;
    let mut nsolsfound = 10 * scip.heurs[heur].nbestsolsfound + hd(scip, heur).nsuccess as i64;
    let h = hd(scip, heur);
    let mut maxnlpiterations = (((nsolsfound as f64 + 1.0) / (ncalls as f64 + 1.0)) * h.maxlpiterquot * nlpiterations as f64) as i64;
    maxnlpiterations += h.maxlpiterofs as i64;

    // Don't try to dive, if we took too many LP iterations during diving.
    if h.nlpiterations >= maxnlpiterations {
        return Ok(());
    }

    // At the first root call, allow more iterations if there is no feasible
    // solution yet.
    if scip.heurs[heur].ncalls == 0 && api::get_n_sols_found(scip) == 0 && api::get_depth(scip) == 0 {
        maxnlpiterations += nlpiterations;
    }

    // Allow at least a certain number of LP iterations in this dive.
    let h = hd(scip, heur);
    maxnlpiterations = if maxnlpiterations >= h.nlpiterations + MINLPITER {
        maxnlpiterations
    } else {
        h.nlpiterations + MINLPITER
    };

    // Calculate maximal number of flips and loops.
    let maxflips = 3 * h.minflips;
    let maxloops = if h.maxloops == -1 { i32::MAX } else { h.maxloops };
    let maxstallloops = if h.maxstallloops == -1 { i32::MAX } else { h.maxstallloops };

    *result = ScipResultCode::DidNotFind;

    if h.usefp20 {
        // The round-and-propagate scheme solves a copied SCIP.
        log::error!("heuristics/feaspump/usefp20 needs a copied SCIP, which is not ported");
        return Err(Retcode::NotImplemented);
    }
    let stage3 = h.stage3;
    if stage3 {
        // Stage 3 solves a local branching sub-MIP in a copied SCIP.
        log::error!("heuristics/feaspump/stage3 needs a copied SCIP, which is not ported");
        return Err(Retcode::NotImplemented);
    }

    // Memory allocation.
    let cyclelength = h.cyclelength as usize;
    let (minflips, perturbfreq, pertsolfound, alphadiff, objfactorparam, alphaparam) = (h.minflips, h.perturbfreq, h.pertsolfound, h.alphadiff, h.objfactor, h.alpha);
    let mut mostfracvars = vec![NONE; maxflips as usize];
    let mut mostfracvals = vec![0.0f64; maxflips as usize];
    let mut lastroundedsols = Vec::with_capacity(cyclelength);
    let mut lastalphas = vec![0.0f64; cyclelength];
    let mut cycles = vec![false; cyclelength];

    for _ in 0..cyclelength {
        lastroundedsols.push(api::create_sol(scip, heur)?);
    }

    // Start diving.
    scip_lp::start_dive(scip)?;

    // LP was solved optimal.
    let mut lperror = false;
    let mut lpsolstat = LpSolStat::Optimal;

    // Pumping rounds.
    nsolsfound = api::get_n_best_sols_found(scip);
    let objfactor = if objfactorparam == 1.0 {
        let v = 1.0 - 0.1 / (1 + nsolsfound) as f64;
        if v <= 0.999 { v } else { 0.999 }
    } else {
        objfactorparam
    };

    // Scale distance function and original objective to the same norm.
    let mut objnorm = api::get_obj_norm(scip);
    objnorm = if objnorm >= 1.0 { objnorm } else { 1.0 };
    let scalingfactor = (nenfovars as f64).sqrt() / objnorm;

    // Data initialization.
    let mut alpha = alphaparam;
    let mut nloops = 0;
    let mut nstallloops = 0;
    let mut nbestsolsfound = api::get_n_best_sols_found(scip);
    let mut bestnfracs = i32::MAX;

    // Pumping loop.
    while nfracs > 0
        && hd(scip, heur).nlpiterations < adjusted_max_nlp_iterations(maxnlpiterations, nsolsfound, nstallloops)
        && nloops < maxloops
        && nstallloops < maxstallloops
        && !crate::solve::solve_is_stopped(scip, false)
    {
        // Decrease convex combination scalar.
        nloops += 1;
        alpha *= objfactor;

        let roundedsol = hd(scip, heur).roundedsol;
        crate::sol::sol_link_lp_sol(scip, roundedsol, TRANSPROB)?;

        // Randomly choose maximum number of variables to flip in current
        // pumping round in case of a 1-cycle.
        let lo = if nfracs / 2 < minflips { nfracs / 2 + 1 } else { minflips };
        let hi = if nfracs <= maxflips { nfracs } else { maxflips };
        let maxnflipcands = rng(scip, heur).get_int(lo, hi);
        let mut nflipcands = 0;

        // Get all unfixed integer variables.
        let (npseudocands, _) = crate::branch::branchcand_get_pseudo_cands(scip);
        let pseudocands: Vec<VarId> = scip.branchcand.pseudocands.0[..npseudocands as usize].to_vec();

        for &var in &pseudocands {
            // Round the LP solution.
            let mut solval = var_get_lp_sol(scip, var);
            let frac = scip.set.feas_frac(solval);

            // Round randomly if the value is close to 0.5.
            if scip.set.is_eq(frac, 0.5) {
                if rng(scip, heur).get_real(0.0, 1.0) <= 0.5 {
                    solval = scip.set.floor(solval);
                } else {
                    solval = scip.set.ceil(solval);
                }
            } else {
                solval = scip.set.floor(solval + 0.5);
            }

            update_variable_rounding(scip, heur, var, solval, alpha, scalingfactor)?;

            // Check whether the variable is one of the most fractionals and
            // label if so.
            if scip.set.is_feas_positive(frac) {
                insert_flip_cand(&mut mostfracvars, &mut mostfracvals, &mut nflipcands, maxnflipcands, var, frac);
            }
        }

        // Change objective coefficients for continuous variables.
        for &var in &vars[nenfovars..nvars] {
            let newobj = alpha * scip.vars[var].obj;
            scip_lp::chg_var_obj_dive(scip, var, newobj)?;
        }

        // Initialize cycle check.
        let cl = cyclelength as i32;
        let mut minimum = if cl < nloops { cl } else { nloops - 1 };
        for j in 0..cyclelength {
            cycles[j] = nloops > j as i32 + 1 && (lastalphas[j] - alpha).abs() < alphadiff;
        }

        // Check for j-cycles.
        let roundedsol = hd(scip, heur).roundedsol;
        for &var in &vars[..nenfovars] {
            let solval = api::get_sol_val(scip, roundedsol, var);

            // Cycles exist, iff all solution values are equal.
            for j in 0..minimum as usize {
                let oldsolval = api::get_sol_val(scip, lastroundedsols[j], var);
                cycles[j] = cycles[j] && scip.set.is_feas_eq(solval, oldsolval);
            }
        }

        // Force to flip variables at random after a couple of pumping
        // rounds, or if a new best solution in the current region has been
        // found.
        if nloops % perturbfreq == 0 || (pertsolfound && api::get_n_best_sols_found(scip) > nbestsolsfound) {
            handle_cycle(scip, heur, &vars, nenfovars, alpha, scalingfactor)?;
            nbestsolsfound = api::get_n_best_sols_found(scip);
        } else {
            minimum = if cl < nloops { cl } else { nloops - 1 };

            for (j, cycle) in cycles[..minimum as usize].iter().enumerate() {
                // If we got the same rounded solution as in some step before,
                // we have to flip some variables.
                if *cycle {
                    // 1-cycles have a special flipping rule (flip most
                    // fractional variables).
                    if j == 0 {
                        handle_1_cycle(scip, heur, &mostfracvars, nflipcands, alpha, scalingfactor)?;
                    } else {
                        handle_cycle(scip, heur, &vars, nenfovars, alpha, scalingfactor)?;
                    }
                    break;
                }
            }
        }

        // The LP with the new (distance) objective is solved.
        let nlpiterations = scip.stat.nlpiterations;
        let nlpiterationsleft = adjusted_max_nlp_iterations(maxnlpiterations, nsolsfound, nstallloops) - hd(scip, heur).nlpiterations;
        let iterlimit = if nlpiterationsleft as i32 >= MINLPITER as i32 {
            nlpiterationsleft as i32
        } else {
            MINLPITER as i32
        };

        // Errors in the LP solver should not kill the overall solving
        // process, if the LP is just needed for a heuristic (C catches the
        // return code in optimized mode and warns).
        match scip_lp::solve_dive_lp(scip, iterlimit) {
            Ok((e, _)) => lperror = e,
            Err(retcode) => {
                log::warn!("Error while solving LP in Feaspump heuristic; LP solve terminated with code <{retcode:?}>");
                log::warn!("This does not affect the remaining solution procedure --> continue");
            }
        }
        lpsolstat = api::get_lp_solstat(scip);

        // Update iteration count.
        hdm(scip, heur).nlpiterations += scip.stat.nlpiterations - nlpiterations;

        // Check whether LP was solved optimal.
        if lperror || lpsolstat != LpSolStat::Optimal {
            break;
        }

        // Swap the last solutions.
        let roundedsol = hd(scip, heur).roundedsol;
        api::unlink_sol(scip, roundedsol)?;
        let tmpsol = lastroundedsols[cyclelength - 1];
        for j in (1..cyclelength).rev() {
            lastroundedsols[j] = lastroundedsols[j - 1];
            lastalphas[j] = lastalphas[j - 1];
        }
        lastroundedsols[0] = roundedsol;
        lastalphas[0] = alpha;
        hdm(scip, heur).roundedsol = tmpsol;

        // Check for improvement in number of fractionals.
        nfracs = api::get_n_lp_branch_cands(scip);
        if nfracs < bestnfracs {
            bestnfracs = nfracs;
            nstallloops = 0;
        } else {
            nstallloops += 1;
        }
    }

    // Try final solution, if no more fractional variables are left.
    if nfracs == 0 && !lperror && lpsolstat == LpSolStat::Optimal {
        let sol = hd(scip, heur).sol;
        crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

        let success = api::try_sol(scip, sol, false, false, false, false, false)?;
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

    // Free memory.
    for sol in lastroundedsols {
        api::free_sol(scip, sol)?;
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurFeaspump`: creates the feaspump primal heuristic and
/// includes it in SCIP.
pub fn include_heur_feaspump(scip: &mut Scip) -> ScipResult {
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
        heur_exec_feaspump,
        Some(Box::new(HeurData {
            sol: NONE,
            roundedsol: NONE,
            nlpiterations: 0,
            maxlpiterquot: DEFAULT_MAXLPITERQUOT,
            objfactor: DEFAULT_OBJFACTOR,
            alpha: DEFAULT_ALPHA,
            alphadiff: DEFAULT_ALPHADIFF,
            maxlpiterofs: DEFAULT_MAXLPITEROFS,
            maxsols: DEFAULT_MAXSOLS,
            maxloops: DEFAULT_MAXLOOPS,
            maxstallloops: DEFAULT_MAXSTALLLOOPS,
            minflips: DEFAULT_MINFLIPS,
            cyclelength: DEFAULT_CYCLELENGTH,
            perturbfreq: DEFAULT_PERTURBFREQ,
            nsuccess: 0,
            neighborhoodsize: DEFAULT_NEIGHBORHOODSIZE,
            randnumgen: None,
            beforecuts: DEFAULT_BEFORECUTS,
            usefp20: DEFAULT_USEFP20,
            pertsolfound: DEFAULT_PERTSOLFOUND,
            stage3: DEFAULT_STAGE3,
            copycuts: DEFAULT_COPYCUTS,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_feaspump);
    h.heurinit = Some(heur_init_feaspump);
    h.heurexit = Some(heur_exit_feaspump);
    h.heurinitsol = Some(heur_initsol_feaspump);
    h.heurexitsol = Some(heur_exitsol_feaspump);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add feaspump primal heuristic parameters.
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    };
    paramset_add_real(
        scip,
        "heuristics/feaspump/maxlpiterquot",
        "maximal fraction of diving LP iterations compared to node LP iterations",
        t!(f64, maxlpiterquot),
        false,
        DEFAULT_MAXLPITERQUOT,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/feaspump/objfactor",
        "factor by which the regard of the objective is decreased in each round, 1.0 for dynamic",
        t!(f64, objfactor),
        false,
        DEFAULT_OBJFACTOR,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/feaspump/alpha",
        "initial weight of the objective function in the convex combination",
        t!(f64, alpha),
        false,
        DEFAULT_ALPHA,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/feaspump/alphadiff",
        "threshold difference for the convex parameter to perform perturbation",
        t!(f64, alphadiff),
        false,
        DEFAULT_ALPHADIFF,
        0.0,
        1.0,
        None,
    )?;

    paramset_add_int(
        scip,
        "heuristics/feaspump/maxlpiterofs",
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
        "heuristics/feaspump/maxsols",
        "total number of feasible solutions found up to which heuristic is called (-1: no limit)",
        t!(i32, maxsols),
        true,
        DEFAULT_MAXSOLS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/feaspump/maxloops",
        "maximal number of pumping loops (-1: no limit)",
        t!(i32, maxloops),
        true,
        DEFAULT_MAXLOOPS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/feaspump/maxstallloops",
        "maximal number of pumping rounds without fractionality improvement (-1: no limit)",
        t!(i32, maxstallloops),
        true,
        DEFAULT_MAXSTALLLOOPS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/feaspump/minflips",
        "minimum number of random variables to flip, if a 1-cycle is encountered",
        t!(i32, minflips),
        true,
        DEFAULT_MINFLIPS,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/feaspump/cyclelength",
        "maximum length of cycles to be checked explicitly in each round",
        t!(i32, cyclelength),
        true,
        DEFAULT_CYCLELENGTH,
        1,
        100,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/feaspump/perturbfreq",
        "number of iterations until a random perturbation is forced",
        t!(i32, perturbfreq),
        true,
        DEFAULT_PERTURBFREQ,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/feaspump/neighborhoodsize",
        "radius (using Manhattan metric) of the neighborhood to be searched in stage 3",
        t!(i32, neighborhoodsize),
        false,
        DEFAULT_NEIGHBORHOODSIZE,
        1,
        i32::MAX,
        None,
    )?;

    paramset_add_bool(
        scip,
        "heuristics/feaspump/beforecuts",
        "should the feasibility pump be called at root node before cut separation?",
        t!(bool, beforecuts),
        false,
        DEFAULT_BEFORECUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/feaspump/usefp20",
        "should an iterative round-and-propagate scheme be used to find the integral points?",
        t!(bool, usefp20),
        false,
        DEFAULT_USEFP20,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/feaspump/pertsolfound",
        "should a random perturbation be performed if a feasible solution was found?",
        t!(bool, pertsolfound),
        false,
        DEFAULT_PERTSOLFOUND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/feaspump/stage3",
        "should we solve a local branching sub-MIP if no solution could be found?",
        t!(bool, stage3),
        false,
        DEFAULT_STAGE3,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/feaspump/copycuts",
        "should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;

    Ok(())
}
