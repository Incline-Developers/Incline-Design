//! Port of `heur_indicator.c`: indicator primal heuristic, which creates
//! feasible solutions from values for indicator variables (passed by the
//! indicator constraint handler's alternative LP, which is not ported, or
//! taken from the best solution with `improvesols`).

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_DURINGLPLOOP, HEURTIMING_NONE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    scip_probing,
    types::{ConsId, ConshdlrId, HeurId, LpSolStat, ScipResultCode, SolId},
    var::NONE,
};

const HEUR_NAME: &str = "indicator";
const HEUR_DESC: &str = "indicator heuristic to create feasible solutions from values for indicator variables";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -20200;
const HEUR_FREQ: i32 = 1;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_DURINGLPLOOP;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Whether the one-opt heuristic should be started.
const DEFAULT_ONEOPT: bool = false;
/// Try to improve other solutions by one-opt?
const DEFAULT_IMPROVESOLS: bool = false;

/// `SCIP_HeurData`: primal heuristic data.
struct HeurData {
    /// Number of indicator constraints.
    nindconss: i32,
    /// Indicator constraints.
    indconss: Option<Vec<ConsId>>,
    /// Bitset of indicator variables in solution candidate.
    solcand: Option<Vec<bool>>,
    /// Objective of previously stored solution.
    obj: f64,
    /// Whether the one-opt heuristic should be started.
    oneopt: bool,
    /// Indicator constraint handler.
    indicatorconshdlr: ConshdlrId,
    /// Last solution considered for improvement.
    lastsol: SolId,
    /// Try to improve other solutions by one-opt?
    improvesols: bool,
    /// Initial heuristic timing.
    inittiming: HeurTiming,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut().unwrap()
}

/*
 * Local methods
 */

/// `tryOneOpt`: try one-opt on given solution; returns the number of
/// solutions found.
fn try_one_opt(scip: &mut Scip, heur: HeurId, indconss: &[ConsId], solcand: &[bool]) -> ScipResult<i32> {
    let nindconss = indconss.len();
    let mut nfoundsols = 0;

    scip_probing::start_probing(scip)?;

    let mut i = 0;
    while i < nindconss && !crate::solve::solve_is_stopped(scip, false) {
        // Skip nonactive constraints.
        if !scip.conss[indconss[i]].is_active() {
            i += 1;
            continue;
        }

        let binvar = crate::cons_indicator::get_binary_var_indicator(scip, indconss[i]);

        // Skip constraints with fixed variables.
        if scip.vars[binvar].locdom.ub < 0.5 || scip.vars[binvar].locdom.lb > 0.5 {
            i += 1;
            continue;
        }

        // Return if the we would exceed the depth limit of the tree.
        if crate::def::MAXTREEDEPTH <= api::get_depth(scip) {
            break;
        }

        if solcand[i] {
            i += 1;
            continue;
        }

        // Get rid of all bound changes.
        scip_probing::new_probing_node(scip)?;

        // Fix variables.
        for c in 0..nindconss {
            // Skip nonactive constraints.
            if !scip.conss[indconss[c]].is_active() {
                continue;
            }

            let binvar = crate::cons_indicator::get_binary_var_indicator(scip, indconss[c]);

            // Fix variables according to solution candidate, except constraint i.
            let s = if c == i { !solcand[c] } else { solcand[c] };

            if !s {
                if scip.vars[binvar].locdom.lb < 0.5 && scip.vars[binvar].locdom.ub > 0.5 {
                    scip_probing::chg_var_lb_probing(scip, binvar, 1.0)?;
                }
            } else if scip.vars[binvar].locdom.ub > 0.5 && scip.vars[binvar].locdom.lb < 0.5 {
                scip_probing::chg_var_ub_probing(scip, binvar, 0.0)?;
            }
        }

        // Propagate variables.
        let (cutoff, _) = scip_probing::propagate_probing(scip, -1)?;
        if cutoff {
            scip_probing::backtrack_probing(scip, 0)?;
            i += 1;
            continue;
        }

        // Solve LP to move continuous variables.
        let (lperror, cutoff) = scip_probing::solve_probing_lp(scip, -1)?;

        // The LP often reaches the objective limit - we currently do not use such
        // solutions.
        if lperror || cutoff || api::get_lp_solstat(scip) != LpSolStat::Optimal {
            scip_probing::backtrack_probing(scip, 0)?;
            i += 1;
            continue;
        }

        // Create solution.
        let sol = api::create_sol(scip, heur)?;

        // Copy the current LP solution to the working solution.
        crate::sol::sol_link_lp_sol(scip, sol, crate::prob::TRANSPROB)?;

        // Check solution for feasibility: only check integrality, because we
        // solved an LP.
        let stored = api::try_sol_free(scip, sol, false, false, false, true, false)?;
        if stored {
            nfoundsols += 1;
        }

        scip_probing::backtrack_probing(scip, 0)?;
        i += 1;
    }
    scip_probing::end_probing(scip)?;

    Ok(nfoundsols)
}

/// `trySolCandidate`: try given solution; returns the number of solutions
/// found.
fn try_sol_candidate(scip: &mut Scip, heur: HeurId, indconss: &[ConsId], solcand: &[bool]) -> ScipResult<i32> {
    let mut nfoundsols = 0;

    scip_probing::start_probing(scip)?;

    // We can stop here if we have already reached the maximal depth.
    if crate::def::MAXTREEDEPTH <= api::get_depth(scip) {
        scip_probing::end_probing(scip)?;
        return Ok(0);
    }

    scip_probing::new_probing_node(scip)?;

    // Fix variables.
    for (c, &cons) in indconss.iter().enumerate() {
        // Skip nonactive constraints.
        if !scip.conss[cons].is_active() {
            continue;
        }

        let binvar = crate::cons_indicator::get_binary_var_indicator(scip, cons);

        // Fix binary variables not in cover to 1 and corresponding slack
        // variables to 0. The other binary variables are fixed to 0.
        if !solcand[c] {
            // To be sure, check for non-fixed variables.
            if scip.vars[binvar].locdom.lb < 0.5 && scip.vars[binvar].locdom.ub > 0.5 {
                scip_probing::chg_var_lb_probing(scip, binvar, 1.0)?;
            }
        } else if scip.vars[binvar].locdom.ub > 0.5 && scip.vars[binvar].locdom.lb < 0.5 {
            scip_probing::chg_var_ub_probing(scip, binvar, 0.0)?;
        }
    }

    // Propagate variables.
    let (cutoff, _) = scip_probing::propagate_probing(scip, -1)?;
    if cutoff {
        scip_probing::end_probing(scip)?;
        return Ok(0);
    }

    // Solve LP to move continuous variables.
    let (lperror, cutoff) = scip_probing::solve_probing_lp(scip, -1)?;

    // The LP often reaches the objective limit - we currently do not use such
    // solutions.
    if lperror || cutoff || api::get_lp_solstat(scip) != LpSolStat::Optimal {
        scip_probing::end_probing(scip)?;
        return Ok(0);
    }

    // Create solution.
    let sol = api::create_sol(scip, heur)?;

    // Copy the current LP solution to the working solution.
    crate::sol::sol_link_lp_sol(scip, sol, crate::prob::TRANSPROB)?;

    // Check solution for feasibility: only check integrality, because we
    // solved an LP.
    let stored = api::try_sol_free(scip, sol, false, false, false, true, false)?;
    if stored {
        nfoundsols += 1;
    }

    scip_probing::end_probing(scip)?;

    // Possibly perform one-opt.
    if stored && hd(scip, heur).oneopt {
        try_one_opt(scip, heur, indconss, solcand)?;
    }

    Ok(nfoundsols)
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyIndicator`: copy method for primal heuristic plugins (called
/// when SCIP copies plugins).
fn heur_copy_indicator(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_indicator(scip)
}

/// `heurInitsolIndicator`: solving process initialization method of primal
/// heuristic (called when branch and bound process is about to begin).
fn heur_initsol_indicator(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Find indicator handler.
    let indicatorconshdlr = crate::set::set_find_conshdlr(scip, "indicator");
    hdm(scip, heur).indicatorconshdlr = indicatorconshdlr;

    // Store indicator timing.
    let timingmask = scip.heurs[heur].timingmask;
    hdm(scip, heur).inittiming = timingmask;

    // Disable indicator heuristic, enabled when indicators are passed.
    if indicatorconshdlr == NONE || scip.conshdlrs[indicatorconshdlr].nconss == 0 || crate::copy::get_subscip_depth(scip) > 0 {
        scip.heurs[heur].timingmask = HEURTIMING_NONE;
    }

    Ok(())
}

/// `heurExitsolIndicator`: solving process deinitialization method of
/// primal heuristic (called before branch and bound process data is freed).
fn heur_exitsol_indicator(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset indicator timing.
    scip.heurs[heur].timingmask = hd(scip, heur).inittiming;
    Ok(())
}

/// `heurFreeIndicator`: destructor of primal heuristic to free user data
/// (called when SCIP is exiting).
fn heur_free_indicator(scip: &mut Scip, heur: HeurId) -> ScipResult {
    scip.heurs[heur].heurdata = None;
    Ok(())
}

/// `heurExecIndicator`: execution method of primal heuristic.
fn heur_exec_indicator(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Get heuristic's data.
    let indicatorconshdlr = hd(scip, heur).indicatorconshdlr;
    let nindconss = if hd(scip, heur).solcand.is_none() {
        scip.conshdlrs[indicatorconshdlr].nconss
    } else {
        hd(scip, heur).nindconss
    };

    // The heuristic will only be successful if there are indicator
    // constraints, no binary variables except for the indicator variables, and
    // no integral variables.
    if nindconss == 0 || api::get_n_bin_vars(scip) + api::get_n_bin_impl_vars(scip) > nindconss || api::get_n_int_vars(scip) + api::get_n_int_impl_vars(scip) >= 1 {
        return Ok(());
    }

    // Call heuristic if solution candidate is available.
    if hd(scip, heur).solcand.is_some() {
        let indconss = hd(scip, heur).indconss.clone().unwrap();
        let solcand = hd(scip, heur).solcand.clone().unwrap();
        let nfoundsols = try_sol_candidate(scip, heur, &indconss, &solcand)?;

        *result = if nfoundsols > 0 { ScipResultCode::FoundSol } else { ScipResultCode::DidNotFind };

        // Free memory.
        let d = hdm(scip, heur);
        d.solcand = None;
        d.indconss = None;
    }

    // Try to improve solutions generated by other heuristics.
    if hd(scip, heur).improvesols {
        // Check whether a new best solution has been found.
        let bestsol = api::get_best_sol(scip);
        if bestsol == hd(scip, heur).lastsol {
            return Ok(());
        }
        hdm(scip, heur).lastsol = bestsol;

        // Avoid solutions produced by this heuristic.
        if crate::sol::sol_get_heur(scip, bestsol) == heur {
            return Ok(());
        }

        let h = &scip.conshdlrs[indicatorconshdlr];
        let indconss = h.conss.0[..h.nconss as usize].to_vec();

        // Fill solution candidate.
        let mut solcand = vec![false; nindconss as usize];
        for i in 0..nindconss as usize {
            if scip.conss[indconss[i]].is_active() {
                let binvar = crate::cons_indicator::get_binary_var_indicator(scip, indconss[i]);
                let val = api::get_sol_val(scip, bestsol, binvar);

                if val > 0.5 {
                    solcand[i] = true;
                }
            }
        }

        // Try one-opt heuristic.
        let nfoundsols = try_one_opt(scip, heur, &indconss[..nindconss as usize], &solcand)?;

        *result = if nfoundsols > 0 { ScipResultCode::FoundSol } else { ScipResultCode::DidNotFind };
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurIndicator`: creates the indicator primal heuristic and
/// includes it in SCIP.
pub fn include_heur_indicator(scip: &mut Scip) -> ScipResult {
    // Create Indicator primal heuristic data.
    let heurdata = HeurData {
        nindconss: 0,
        indconss: None,
        solcand: None,
        lastsol: NONE,
        indicatorconshdlr: NONE,
        obj: scip.set.infinity(),
        inittiming: HEUR_TIMING,
        oneopt: false,
        improvesols: false,
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
        heur_exec_indicator,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_indicator);
    h.heurinitsol = Some(heur_initsol_indicator);
    h.heurexitsol = Some(heur_exitsol_indicator);
    h.heurfree = Some(heur_free_indicator);

    // Add parameters.
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }
    crate::paramset::paramset_add_bool(
        scip,
        &format!("heuristics/{HEUR_NAME}/oneopt"),
        "whether the one-opt heuristic should be started",
        t!(bool, oneopt),
        true,
        DEFAULT_ONEOPT,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        &format!("heuristics/{HEUR_NAME}/improvesols"),
        "Try to improve other solutions by one-opt?",
        t!(bool, improvesols),
        true,
        DEFAULT_IMPROVESOLS,
        None,
    )?;

    Ok(())
}

/// `SCIPheurPassIndicator`: pass partial solution for indicator variables
/// to heuristic.
pub fn heur_pass_indicator(scip: &mut Scip, heur: HeurId, indconss: &[ConsId], solcand: &[bool], obj: f64) -> ScipResult {
    // Get heuristic's data.
    if obj >= hd(scip, heur).obj {
        return Ok(());
    }

    // Copy indicator information and partial solution.
    let inittiming = hd(scip, heur).inittiming;
    let d = hdm(scip, heur);
    d.indconss = Some(indconss.to_vec());
    d.nindconss = indconss.len() as i32;
    d.solcand = Some(solcand.to_vec());
    d.obj = obj;

    // Enable indicator heuristic.
    scip.heurs[heur].timingmask = inittiming;

    Ok(())
}
