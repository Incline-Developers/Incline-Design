//! Port of `heur_trysol.c`: try solution heuristic, which tries solutions
//! other plugins pass to it (and adds the ones they vouch for).

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_BEFORENODE, HEURTIMING_BEFOREPRESOL, HEURTIMING_DURINGLPLOOP, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ObjSense, ScipResultCode, SolId},
    var::NONE,
};

const HEUR_NAME: &str = "trysol";
const HEUR_DESC: &str = "try solution heuristic";
const HEUR_DISPCHAR: u8 = b't';
/// Should process after all other heuristics.
const HEUR_PRIORITY: i32 = -3000010;
const HEUR_FREQ: i32 = 1;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_DURINGLPLOOP | HEURTIMING_BEFOREPRESOL | HEURTIMING_BEFORENODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/*
 * Data structures
 */

/// Primal heuristic data.
struct HeurData {
    /// Storing solution passed to heuristic which has to tried (NONE if
    /// none).
    trysol: SolId,
    /// Storing solution passed to heuristic which can be added without
    /// checking (NONE if none).
    addsol: SolId,
    /// Whether we are within our own call.
    rec: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyTrySol`: copy method for primal heuristic plugins.
fn heur_copy_trysol(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_trysol(scip)
}

/// `heurExitTrySol`: deinitialization method of primal heuristic (C
/// declares it as an exitsol callback but registers it with
/// `SCIPsetHeurExit`).
fn heur_exit_trysol(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free solution if one is still present.
    let trysol = hd(scip, heur).trysol;
    if trysol != NONE {
        api::free_sol(scip, trysol)?;
        hdm(scip, heur).trysol = NONE;
    }

    // Free solution if one is still present.
    let addsol = hd(scip, heur).addsol;
    if addsol != NONE {
        api::free_sol(scip, addsol)?;
        hdm(scip, heur).addsol = NONE;
    }

    Ok(())
}

/// `heurExecTrySol`: execution method of primal heuristic.
fn heur_exec_trysol(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only run if solution present.
    if hd(scip, heur).addsol == NONE && hd(scip, heur).trysol == NONE {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    hdm(scip, heur).rec = true;

    let trysol = hd(scip, heur).trysol;
    if trysol != NONE {
        // Try solution and free it - check everything, because we are not
        // sure.
        hdm(scip, heur).trysol = NONE;
        let stored = api::try_sol_free(scip, trysol, false, false, true, true, true)?;

        if stored {
            *result = ScipResultCode::FoundSol;
        }
    }

    let addsol = hd(scip, heur).addsol;
    if addsol != NONE {
        hdm(scip, heur).addsol = NONE;
        let stored = api::add_sol_free(scip, addsol)?;

        if stored {
            *result = ScipResultCode::FoundSol;
        }
    }

    hdm(scip, heur).rec = false;

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurTrySol`: creates the trysol primal heuristic and
/// includes it in SCIP.
pub fn include_heur_trysol(scip: &mut Scip) -> ScipResult {
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
        heur_exec_trysol,
        Some(Box::new(HeurData {
            trysol: NONE,
            addsol: NONE,
            rec: false,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_trysol);
    h.heurexit = Some(heur_exit_trysol);

    Ok(())
}

/// Shared body of `SCIPheurPassSolTrySol` and `SCIPheurPassSolAddSol`:
/// keeps a copy of `sol` in the add or try slot if it is the first or
/// better than the one there.
fn pass_sol(scip: &mut Scip, heur: HeurId, sol: SolId, addslot: bool) -> ScipResult {
    // Only store solution if we are not within our own SCIPtrySol() call.
    if hd(scip, heur).rec {
        return Ok(());
    }

    let slot = |scip: &Scip| if addslot { hd(scip, heur).addsol } else { hd(scip, heur).trysol };
    let current = slot(scip);
    let replace = current == NONE || {
        let solobj = api::get_sol_orig_obj(scip, sol);
        let curobj = api::get_sol_orig_obj(scip, current);
        (api::get_objsense(scip) == ObjSense::Maximize && scip.set.is_gt(solobj, curobj)) || scip.set.is_lt(solobj, curobj)
    };

    if replace {
        if current != NONE {
            // Free previous solution.
            api::free_sol(scip, current)?;
        }

        let copy = api::create_sol_copy(scip, sol)?;
        api::unlink_sol(scip, copy)?;
        crate::sol::sol_set_heur(scip, copy, heur);
        if addslot {
            hdm(scip, heur).addsol = copy;
        } else {
            hdm(scip, heur).trysol = copy;
        }
    }

    Ok(())
}

/// `SCIPheurPassSolTrySol`: pass solution to trysol heuristic.
pub fn heur_pass_sol_try_sol(scip: &mut Scip, heur: HeurId, sol: SolId) -> ScipResult {
    pass_sol(scip, heur, sol, false)
}

/// `SCIPheurPassSolAddSol`: pass solution to trysol heuristic which just
/// gets added (without checking feasibility).
pub fn heur_pass_sol_add_sol(scip: &mut Scip, heur: HeurId, sol: SolId) -> ScipResult {
    pass_sol(scip, heur, sol, true)
}
