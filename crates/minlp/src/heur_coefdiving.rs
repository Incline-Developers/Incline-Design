//! Port of `heur_coefdiving.c`: LP diving heuristic that chooses fixings
//! w.r.t. the matrix coefficients.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DIVETYPE_SOS1VARIABLE, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, LockType, ScipResultCode, VarId},
    var::{NONE, var_get_n_locks_down_type, var_get_n_locks_up_type, var_is_binary, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "coefdiving";
const HEUR_DESC: &str = "LP diving heuristic that chooses fixings w.r.t. the matrix coefficients";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1001000;
const HEUR_FREQ: i32 = -1;
const HEUR_FREQOFS: i32 = 1;
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
const DEFAULT_RANDSEED: u32 = 83;

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

/// `heurCopyCoefdiving`: copy method for primal heuristic plugins.
fn heur_copy_coefdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_coefdiving(scip)
}

/// `heurInitCoefdiving`: initialization method of primal heuristic.
fn heur_init_coefdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitCoefdiving`: deinitialization method of primal heuristic.
fn heur_exit_coefdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecCoefdiving`: execution method of primal heuristic.
fn heur_exec_coefdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// `divesetGetScoreCoefdiving`: returns a score for the given candidate --
/// the best candidate maximizes the diving score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_coefdiving(
    scip: &mut Scip,
    diveset: DivesetId,
    divetype: DiveType,
    cand: VarId,
    candsol: f64,
    mut candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let mayrounddown = var_may_round_down(scip, cand);
    let mayroundup = var_may_round_up(scip, cand);

    if mayrounddown || mayroundup {
        // Choose rounding direction:
        // - if variable may be rounded in both directions, round
        //   corresponding to the fractionality
        // - otherwise, round in the infeasible direction
        if mayrounddown && mayroundup {
            debug_assert!(divetype != DIVETYPE_SOS1VARIABLE);

            // Try to avoid variability; decide randomly if the LP solution
            // can contain some noise.
            if scip.set.is_eq(candsfrac, 0.5) {
                *roundup = scip.divesets[diveset].randnumgen.get_int(0, 1) == 0;
            } else {
                *roundup = candsfrac > 0.5;
            }
        } else {
            *roundup = mayrounddown;
        }
    } else {
        // The candidate may not be rounded.
        let nlocksdown = var_get_n_locks_down_type(scip, cand, LockType::Model);
        let nlocksup = var_get_n_locks_up_type(scip, cand, LockType::Model);
        *roundup = nlocksdown > nlocksup || (nlocksdown == nlocksup && candsfrac > 0.5);
    }

    if *roundup {
        match divetype {
            DIVETYPE_INTEGRALITY => candsfrac = 1.0 - candsfrac,
            DIVETYPE_SOS1VARIABLE => {
                if scip.set.is_feas_positive(candsol) {
                    candsfrac = 1.0 - candsfrac;
                }
            }
            _ => {
                log::error!("Error: Unsupported diving type");
                return Err(crate::retcode::Retcode::InvalidData);
            }
        }
        *score = var_get_n_locks_up_type(scip, cand, LockType::Model) as f64;
    } else {
        if divetype == DIVETYPE_SOS1VARIABLE && scip.set.is_feas_negative(candsol) {
            candsfrac = 1.0 - candsfrac;
        }
        *score = var_get_n_locks_down_type(scip, cand, LockType::Model) as f64;
    }

    // Penalize too small fractions.
    if scip.set.is_eq(candsfrac, 0.01) {
        // Try to avoid variability; decide randomly if the LP solution can
        // contain some noise. Use a 1:SCIP_PROBINGSCORE_PENALTYRATIO chance
        // for scaling the score.
        if scip.divesets[diveset].randnumgen.get_int(0, crate::heur_fracdiving::PROBINGSCORE_PENALTYRATIO) == 0 {
            *score *= 0.01;
        }
    } else if candsfrac < 0.01 {
        *score *= 0.01;
    }

    // Prefer decisions on binary variables.
    if !var_is_binary(scip, cand) {
        *score *= 0.1;
    }

    // Penalize the variable if it may be rounded.
    if mayrounddown || mayroundup {
        *score -= api::get_n_lp_rows(scip) as f64;
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurCoefdiving`: creates the coefdiving heuristic and
/// includes it in SCIP.
pub fn include_heur_coefdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_coefdiving,
        Some(Box::new(HeurData { sol: NONE }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_coefdiving);
    h.heurinit = Some(heur_init_coefdiving);
    h.heurexit = Some(heur_exit_coefdiving);

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
        diveset_get_score_coefdiving,
        None,
    )?;

    Ok(())
}
