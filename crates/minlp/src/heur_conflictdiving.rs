//! Port of `heur_conflictdiving.c`: LP diving heuristic that chooses
//! fixings w.r.t. conflict locks.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DIVETYPE_SOS1VARIABLE, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, LockType, ScipResultCode, VarId},
    var::{NONE, var_get_n_locks_down_type, var_get_n_locks_up_type, var_is_binary},
};

const HEUR_NAME: &str = "conflictdiving";
const HEUR_DESC: &str = "LP diving heuristic that chooses fixings w.r.t. conflict locks";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1000100;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;
/// Bit mask that represents all supported dive types.
const DIVESET_DIVETYPES: DiveType = DIVETYPE_INTEGRALITY | DIVETYPE_SOS1VARIABLE;
/// Is this dive set publicly available (ie., can be used by other primal
/// heuristics?).
const DIVESET_ISPUBLIC: bool = false;

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
const DEFAULT_RANDSEED: u32 = 151;
/// Weight factor to combine conflict and variable locks.
const DEFAULT_LOCKWEIGHT: f64 = 0.75;
/// Use the same rounding strategy like coefficent diving.
const DEFAULT_LIKECOEF: bool = false;
/// Rounding into potentially infeasible direction.
const DEFAULT_MAXVIOL: bool = true;
/// Threshold for penalizing the score.
const DEFAULT_MINCONFLICTLOCKS: i32 = 5;
const MIN_RAND: f64 = 1e-06;
const MAX_RAND: f64 = 1e-05;

use crate::heur_fracdiving::PROBINGSCORE_PENALTYRATIO;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: crate::types::SolId,
    /// Weight factor to combine conflict and variable locks.
    lockweight: f64,
    /// Use the same rounding strategy like coefficent diving.
    likecoefdiving: bool,
    /// Rounding into potentially infeasible direction.
    maxviol: bool,
    /// Threshold for penalizing the score.
    minconflictlocks: i32,
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

/// `heurCopyConflictdiving`: copy method for primal heuristic plugins.
fn heur_copy_conflictdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_conflictdiving(scip)
}

/// `heurInitConflictdiving`: initialization method of primal heuristic.
fn heur_init_conflictdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitConflictdiving`: deinitialization method of primal heuristic.
fn heur_exit_conflictdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecConflictdiving`: execution method of primal heuristic.
fn heur_exec_conflictdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::Delayed;

    // Don't run if no conflict constraints where found.
    if api::get_n_conflict_conss_found(scip) == 0 {
        return Ok(());
    }

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// The variable's (up, down) conflict and model locks.
fn locks(scip: &Scip, cand: VarId) -> (i32, i32, i32, i32) {
    (
        var_get_n_locks_up_type(scip, cand, LockType::Conflict),
        var_get_n_locks_down_type(scip, cand, LockType::Conflict),
        var_get_n_locks_up_type(scip, cand, LockType::Model),
        var_get_n_locks_down_type(scip, cand, LockType::Model),
    )
}

/// `getScoreLikeCoefdiving`: calculate score variant 1: use rounding
/// strategy like coefficent diving.
#[allow(clippy::too_many_arguments)]
fn get_score_like_coefdiving(
    scip: &mut Scip,
    lockweight: f64,
    diveset: DivesetId,
    divetype: DiveType,
    cand: VarId,
    candsol: f64,
    mut candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    // Get conflict and variable locks.
    let (nconflictlocksup, nconflictlocksdown, nlocksup, nlocksdown) = locks(scip, cand);

    // Combine conflict and variable locks.
    let upweight = lockweight * nconflictlocksup as f64 + (1.0 - lockweight) * nlocksup as f64;
    let downweight = lockweight * nconflictlocksdown as f64 + (1.0 - lockweight) * nlocksdown as f64;

    // Check whether there exists a direction w/o any locks.
    let mayrounddown = scip.set.is_zero(upweight);
    let mayroundup = scip.set.is_zero(downweight);

    if mayrounddown || mayroundup {
        // Choose rounding direction:
        // - if variable may be rounded in both directions, round
        //   corresponding to the fractionality
        // - otherwise, round in the infeasible direction
        if mayrounddown && mayroundup {
            // Try to avoid variability; decide randomly if the LP solution
            // can contain some noise.
            *roundup = if scip.set.is_eq(candsfrac, 0.5) {
                scip.divesets[diveset].randnumgen.get_int(0, 1) == 0
            } else {
                candsfrac > 0.5
            };
        } else {
            *roundup = mayrounddown;
        }
    } else {
        // The candidate may not be rounded.
        *roundup = scip.set.is_gt(downweight, upweight) || (scip.set.is_eq(downweight, upweight) && candsfrac > 0.5);
    }

    if *roundup {
        match divetype {
            DIVETYPE_INTEGRALITY => candsfrac = 1.0 - candsfrac,
            DIVETYPE_SOS1VARIABLE => {
                if scip.set.is_feas_positive(candsol) {
                    candsfrac = 1.0 - candsfrac;
                }
            }
            _ => return Err(crate::retcode::Retcode::InvalidData),
        }

        // Add some noise to avoid ties.
        *score = upweight + scip.divesets[diveset].randnumgen.get_real(MIN_RAND, MAX_RAND);
    } else {
        if divetype == DIVETYPE_SOS1VARIABLE && scip.set.is_feas_negative(candsol) {
            candsfrac = 1.0 - candsfrac;
        }

        // Add some noise to avoid ties.
        *score = downweight + scip.divesets[diveset].randnumgen.get_real(MIN_RAND, MAX_RAND);
    }

    // Penalize too small fractions.
    if scip.set.is_eq(candsfrac, 0.01) {
        // Try to avoid variability; decide randomly if the LP solution can
        // contain some noise; use a 1:SCIP_PROBINGSCORE_PENALTYRATIO chance
        // for scaling the score.
        if scip.divesets[diveset].randnumgen.get_int(0, PROBINGSCORE_PENALTYRATIO) == 0 {
            *score *= 0.01;
        }
    } else if candsfrac < 0.01 {
        *score *= 0.1;
    }

    // Prefer decisions on binary variables.
    if !var_is_binary(scip, cand) {
        *score = -1.0 / *score;
    }

    Ok(())
}

/// `getScore`: calculate score variant 2: use a rounding strategy that
/// tends towards infeasibility.
#[allow(clippy::too_many_arguments)]
fn get_score(
    scip: &mut Scip,
    heur: HeurId,
    diveset: DivesetId,
    divetype: DiveType,
    cand: VarId,
    candsol: f64,
    mut candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let (lockweight, maxviol, minconflictlocks) = {
        let h = hd(scip, heur);
        (h.lockweight, h.maxviol, h.minconflictlocks)
    };

    // Get conflict and variable locks.
    let (nconflictlocksup, nconflictlocksdown, nlocksup, nlocksdown) = locks(scip, cand);
    let conflictlocksum = (nconflictlocksup + nconflictlocksdown) as f64;

    // Combine conflict and variable locks.
    let upweight = lockweight * nconflictlocksup as f64 + (1.0 - lockweight) * nlocksup as f64;
    let downweight = lockweight * nconflictlocksdown as f64 + (1.0 - lockweight) * nlocksdown as f64;

    // Check whether there exists a rounding direction w/o any locks.
    let mayrounddown = scip.set.is_zero(upweight);
    let mayroundup = scip.set.is_zero(downweight);

    // Variable can be rounded in exactly one direction and we try to go into
    // the feasible direction.
    if mayrounddown || mayroundup {
        // Choose rounding direction:
        // - if variable may be rounded in both directions, round
        //   corresponding to the fractionality
        // - otherwise, round in the feasible direction
        if mayrounddown && mayroundup {
            // Try to avoid variability; decide randomly if the LP solution
            // can contain some noise.
            *roundup = if scip.set.is_eq(candsfrac, 0.5) {
                scip.divesets[diveset].randnumgen.get_int(0, 1) == 0
            } else {
                candsfrac > 0.5
            };
        } else {
            *roundup = mayroundup;
        }
    } else {
        // Both rounding directions have a different amount of locks.
        if !scip.set.is_eq(upweight, downweight) {
            *roundup = if maxviol {
                scip.set.is_gt(upweight, downweight)
            } else {
                scip.set.is_lt(upweight, downweight)
            };
        }
        // Break ties with lp fractionality != 0.5.
        else if !scip.set.is_eq(candsfrac, 0.5) {
            *roundup = candsfrac > 0.5;
        }
        // Break tie randomly.
        else {
            *roundup = scip.divesets[diveset].randnumgen.get_int(0, 1) == 1;
        }
    }

    if *roundup {
        match divetype {
            DIVETYPE_INTEGRALITY => candsfrac = 1.0 - candsfrac,
            DIVETYPE_SOS1VARIABLE => {
                if scip.set.is_feas_positive(candsol) {
                    candsfrac = 1.0 - candsfrac;
                }
            }
            _ => return Err(crate::retcode::Retcode::InvalidData),
        }

        // Add some noise to avoid ties.
        *score = upweight + scip.divesets[diveset].randnumgen.get_real(MIN_RAND, MAX_RAND);
    } else {
        if divetype == DIVETYPE_SOS1VARIABLE && scip.set.is_feas_negative(candsol) {
            candsfrac = 1.0 - candsfrac;
        }

        // Add some noise to avoid ties.
        *score = downweight + scip.divesets[diveset].randnumgen.get_real(MIN_RAND, MAX_RAND);
    }

    // Penalize too few conflict locks.
    if conflictlocksum > 0.0 && conflictlocksum < minconflictlocks as f64 {
        *score *= 0.1;
    }

    // Penalize if no conflict locks exist at all.
    if conflictlocksum == 0.0 {
        *score *= 0.01;
    }

    // Penalize too small fractions.
    if scip.set.is_eq(candsfrac, 0.01) {
        // Try to avoid variability; decide randomly if the LP solution can
        // contain some noise; use a 1:SCIP_PROBINGSCORE_PENALTYRATIO chance
        // for scaling the score.
        if scip.divesets[diveset].randnumgen.get_int(0, PROBINGSCORE_PENALTYRATIO) == 0 {
            *score *= 0.01;
        }
    } else if candsfrac < 0.01 {
        *score *= 0.01;
    }

    // Prefer decisions on binary variables.
    if !var_is_binary(scip, cand) {
        *score = -1.0 / *score;
    }

    Ok(())
}

/// `divesetGetScoreConflictdiving`: returns a score for the given
/// candidate -- the best candidate maximizes the diving score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_conflictdiving(
    scip: &mut Scip,
    diveset: DivesetId,
    divetype: DiveType,
    cand: VarId,
    candsol: f64,
    candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let heur = scip.divesets[diveset].heur;

    if hd(scip, heur).likecoefdiving {
        let lockweight = hd(scip, heur).lockweight;
        get_score_like_coefdiving(scip, lockweight, diveset, divetype, cand, candsol, candsfrac, score, roundup)
    } else {
        get_score(scip, heur, diveset, divetype, cand, candsol, candsfrac, score, roundup)
    }
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurConflictdiving`: creates the conflictdiving heuristic and
/// includes it in SCIP.
pub fn include_heur_conflictdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_conflictdiving,
        Some(Box::new(HeurData {
            sol: NONE,
            lockweight: DEFAULT_LOCKWEIGHT,
            likecoefdiving: DEFAULT_LIKECOEF,
            maxviol: DEFAULT_MAXVIOL,
            minconflictlocks: DEFAULT_MINCONFLICTLOCKS,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_conflictdiving);
    h.heurinit = Some(heur_init_conflictdiving);
    h.heurexit = Some(heur_exit_conflictdiving);

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
        diveset_get_score_conflictdiving,
        None,
    )?;

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};
    paramset_add_bool(
        scip,
        "heuristics/conflictdiving/maxviol",
        "try to maximize the violation",
        t!(bool, maxviol),
        true,
        DEFAULT_MAXVIOL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/conflictdiving/likecoef",
        "perform rounding like coefficient diving",
        t!(bool, likecoefdiving),
        true,
        DEFAULT_LIKECOEF,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/conflictdiving/minconflictlocks",
        "minimal number of conflict locks per variable",
        t!(i32, minconflictlocks),
        true,
        DEFAULT_MINCONFLICTLOCKS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/conflictdiving/lockweight",
        "weight used in a convex combination of conflict and variable locks",
        t!(f64, lockweight),
        true,
        DEFAULT_LOCKWEIGHT,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
