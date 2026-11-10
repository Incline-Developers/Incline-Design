//! Port of `heur_guideddiving.c`: LP diving heuristic that chooses fixings in direction of incumbent solutions.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, VarId},
    var::{NONE, var_is_binary, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "guideddiving";
const HEUR_DESC: &str = "LP diving heuristic that chooses fixings in direction of incumbent solutions";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1007000;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 7;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;
/// Bit mask that represents all supported dive types.
const DIVESET_DIVETYPES: DiveType = DIVETYPE_INTEGRALITY;
/// Is this dive set publicly available (ie., can be used by other primal
/// heuristics?).
const DIVESET_ISPUBLIC: bool = true;

/* Default parameter settings */

const DEFAULT_MINRELDEPTH: f64 = 0.0;
const DEFAULT_MAXRELDEPTH: f64 = 1.0;
const DEFAULT_MAXLPITERQUOT: f64 = 0.05;
const DEFAULT_MAXLPITEROFS: i32 = 1000;
const DEFAULT_MAXDIVEUBQUOT: f64 = 0.8;
const DEFAULT_MAXDIVEAVGQUOT: f64 = 0.0;
const DEFAULT_MAXDIVEUBQUOTNOSOL: f64 = 1.0;
const DEFAULT_MAXDIVEAVGQUOTNOSOL: f64 = 1.0;
const DEFAULT_BACKTRACK: bool = true;
const DEFAULT_LPRESOLVEDOMCHGQUOT: f64 = 0.15;
const DEFAULT_LPSOLVEFREQ: i32 = 0;
const DEFAULT_ONLYLPBRANCHCANDS: bool = false;
const DEFAULT_RANDSEED: u32 = 127;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: crate::types::SolId,
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

/// `heurCopyGuideddiving`: copy method for primal heuristic plugins.
fn heur_copy_guideddiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_guideddiving(scip)
}

/// `heurInitGuideddiving`: initialization method of primal heuristic.
fn heur_init_guideddiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitGuideddiving`: deinitialization method of primal heuristic.
fn heur_exit_guideddiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecGuideddiving`: execution method of primal heuristic.
fn heur_exec_guideddiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::DidNotRun;

    // Don't dive, if no feasible solutions exist.
    if api::get_n_sols(scip) == 0 {
        return Ok(());
    }

    // Get best solution that should guide the search; if this solution lives
    // in the original variable space, we cannot use it since it might
    // violate the global bounds of the current problem.
    if crate::sol::sol_is_original(scip, api::get_best_sol(scip)) {
        return Ok(());
    }

    // Terminate if there are no integer variables (note that, e.g., SOS1
    // variables may be present).
    if api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip) == api::get_n_vars(scip) {
        return Ok(());
    }

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// `divesetGetScoreGuideddiving`: calculate score and preferred rounding
/// direction for the candidate variable; the best candidate maximizes the
/// score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_guideddiving(
    scip: &mut Scip,
    _diveset: DivesetId,
    _divetype: DiveType,
    cand: VarId,
    candsol: f64,
    mut candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let bestsol = api::get_best_sol(scip);

    let bestsolval = api::get_sol_val(scip, bestsol, cand);

    // Variable should be rounded (guided) into the direction of its
    // incumbent solution value.
    *roundup = candsol < bestsolval;

    let mut obj = scip.vars[cand].obj;
    let objnorm = api::get_obj_norm(scip);

    // Divide by objective norm to normalize obj into [-1,1].
    if scip.set.is_positive(objnorm) {
        obj /= objnorm;
    }

    // Calculate objective gain and fractionality for the selected rounding
    // direction.
    let objgain = if *roundup {
        candsfrac = 1.0 - candsfrac;
        obj * candsfrac
    } else {
        -obj * candsfrac
    };

    // Penalize too small fractions.
    if candsfrac < 0.01 {
        candsfrac *= 0.1;
    }

    // Prefer decisions on binary variables.
    if !var_is_binary(scip, cand) {
        candsfrac *= 0.1;
    }

    // Prefer variables which cannot be rounded by scoring their
    // fractionality.
    *score = if !(var_may_round_down(scip, cand) || var_may_round_up(scip, cand)) {
        -candsfrac
    } else {
        -2.0 - objgain
    };

    Ok(())
}

/// `divesetAvailableGuideddiving`: don't dive with guided diving if no
/// feasible solutions exists or if this solution lives in the original
/// variable space, because it might violate the global bounds of the
/// current problem.
fn diveset_available_guideddiving(scip: &mut Scip, _diveset: DivesetId, available: &mut bool) -> ScipResult {
    *available = !(api::get_n_sols(scip) == 0 || crate::sol::sol_is_original(scip, api::get_best_sol(scip)));
    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurGuideddiving`: creates the guideddiving heuristic and
/// includes it in SCIP.
pub fn include_heur_guideddiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_guideddiving,
        Some(Box::new(HeurData { sol: NONE }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_guideddiving);
    h.heurinit = Some(heur_init_guideddiving);
    h.heurexit = Some(heur_exit_guideddiving);

    // Create a diveset (this will automatically install some additional
    // parameters for the heuristic).
    crate::heur::diveset_create(
        scip,
        heur,
        Some(HEUR_NAME),
        DEFAULT_MINRELDEPTH,
        DEFAULT_MAXRELDEPTH,
        DEFAULT_MAXLPITERQUOT,
        DEFAULT_MAXDIVEUBQUOT,
        DEFAULT_MAXDIVEAVGQUOT,
        DEFAULT_MAXDIVEUBQUOTNOSOL,
        DEFAULT_MAXDIVEAVGQUOTNOSOL,
        DEFAULT_LPRESOLVEDOMCHGQUOT,
        DEFAULT_LPSOLVEFREQ,
        DEFAULT_MAXLPITEROFS,
        DEFAULT_RANDSEED,
        DEFAULT_BACKTRACK,
        DEFAULT_ONLYLPBRANCHCANDS,
        DIVESET_ISPUBLIC,
        DIVESET_DIVETYPES,
        diveset_get_score_guideddiving,
        Some(diveset_available_guideddiving),
    )?;

    Ok(())
}
