//! Port of `heur_fracdiving.c`: LP diving heuristic that chooses fixings
//! w.r.t. the fractionalities.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DIVETYPE_SOS1VARIABLE, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, VarId},
    var::{NONE, var_is_binary, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "fracdiving";
const HEUR_DESC: &str = "LP diving heuristic that chooses fixings w.r.t. the fractionalities";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1003000;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 3;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;
/// Bit mask that represents all supported dive types.
const DIVESET_DIVETYPES: DiveType = DIVETYPE_INTEGRALITY | DIVETYPE_SOS1VARIABLE;
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
const DEFAULT_MAXDIVEUBQUOTNOSOL: f64 = 0.1;
const DEFAULT_MAXDIVEAVGQUOTNOSOL: f64 = 0.0;
const DEFAULT_BACKTRACK: bool = true;
const DEFAULT_LPRESOLVEDOMCHGQUOT: f64 = 0.15;
const DEFAULT_LPSOLVEFREQ: i32 = 0;
const DEFAULT_ONLYLPBRANCHCANDS: bool = false;
const DEFAULT_RANDSEED: u32 = 89;

/// `SCIP_PROBINGSCORE_PENALTYRATIO` (`def.h`): ratio for penalizing too
/// small fractionalities in diving heuristics.
pub(crate) const PROBINGSCORE_PENALTYRATIO: i32 = 2;

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

/// `heurCopyFracdiving`: copy method for primal heuristic plugins.
fn heur_copy_fracdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_fracdiving(scip)
}

/// `heurInitFracdiving`: initialization method of primal heuristic.
fn heur_init_fracdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitFracdiving`: deinitialization method of primal heuristic.
fn heur_exit_fracdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecFracdiving`: execution method of primal heuristic.
fn heur_exec_fracdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::DidNotRun;

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// `divesetGetScoreFracdiving`: calculate score and preferred rounding
/// direction for the candidate variable; the best candidate maximizes the
/// score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_fracdiving(
    scip: &mut Scip,
    diveset: DivesetId,
    divetype: DiveType,
    cand: VarId,
    candsol: f64,
    mut candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    // Score fractionality if candidate is an SOS1 variable.
    if divetype == DIVETYPE_SOS1VARIABLE {
        *score = candsfrac;

        // 'round' in nonzero direction, i.e., fix the candidates neighbors in
        // the conflict graph to zero.
        *roundup = scip.set.is_feas_positive(candsol);

        return Ok(());
    }

    let mayrounddown = var_may_round_down(scip, cand);
    let mayroundup = var_may_round_up(scip, cand);

    // Choose rounding direction:
    // - if variable may be rounded in either both or neither direction,
    //   round corresponding to the fractionality
    // - otherwise, round in the infeasible direction, because feasible
    //   direction is tried by rounding the current fractional solution
    if mayrounddown != mayroundup {
        *roundup = mayrounddown;
    }
    // Try to avoid variability; decide randomly if the LP solution can contain
    // some noise.
    else if scip.set.is_eq(candsfrac, 0.5) {
        *roundup = scip.divesets[diveset].randnumgen.get_int(0, 1) == 0;
    } else {
        *roundup = candsfrac > 0.5;
    }

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
    if scip.set.is_eq(candsfrac, 0.01) {
        // Try to avoid variability; decide randomly if the LP solution can
        // contain some noise; use a 1:SCIP_PROBINGSCORE_PENALTYRATIO chance
        // for increasing the fractionality, i.e., the score.
        if scip.divesets[diveset].randnumgen.get_int(0, PROBINGSCORE_PENALTYRATIO) == 0 {
            candsfrac += 10.0;
        }
    } else if candsfrac < 0.01 {
        candsfrac += 10.0;
    }

    // Prefer decisions on binary variables.
    if !var_is_binary(scip, cand) {
        candsfrac *= 1000.0;
    }

    // Prefer variables which cannot be rounded by scoring their
    // fractionality.
    *score = if !(mayrounddown || mayroundup) { -candsfrac } else { -2.0 - objgain };

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurFracdiving`: creates the fracdiving heuristic and
/// includes it in SCIP.
pub fn include_heur_fracdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_fracdiving,
        Some(Box::new(HeurData { sol: NONE }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_fracdiving);
    h.heurinit = Some(heur_init_fracdiving);
    h.heurexit = Some(heur_exit_fracdiving);

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
        diveset_get_score_fracdiving,
        None,
    )?;

    Ok(())
}
