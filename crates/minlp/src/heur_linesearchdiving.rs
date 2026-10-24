//! Port of `heur_linesearchdiving.c`: LP diving heuristic that chooses fixings following the line from root solution to current solution.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DIVETYPE_SOS1VARIABLE, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, VarId},
    var::{NONE, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "linesearchdiving";
const HEUR_DESC: &str = "LP diving heuristic that chooses fixings following the line from root solution to current solution";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1006000;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 6;
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
const DEFAULT_RANDSEED: u32 = 137;

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

/// `heurCopyLinesearchdiving`: copy method for primal heuristic plugins.
fn heur_copy_linesearchdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_linesearchdiving(scip)
}

/// `heurInitLinesearchdiving`: initialization method of primal heuristic.
fn heur_init_linesearchdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitLinesearchdiving`: deinitialization method of primal heuristic.
fn heur_exit_linesearchdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecLinesearchdiving`: execution method of primal heuristic.
fn heur_exec_linesearchdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::DidNotRun;

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// `divesetGetScoreLinesearchdiving`: calculate score and preferred
/// rounding direction for the candidate variable; the best candidate
/// maximizes the score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_linesearchdiving(
    scip: &mut Scip,
    _diveset: DivesetId,
    divetype: DiveType,
    cand: VarId,
    candsol: f64,
    candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let rootsolval = crate::var::var_get_root_sol(scip, cand);
    let sumeps = scip.set.sumepsilon();
    let mut distquot;

    // Preferred branching direction is further away from the root LP
    // solution.
    if scip.set.is_lt(candsol, rootsolval) {
        // Round down.
        *roundup = false;

        distquot = match divetype {
            DIVETYPE_INTEGRALITY => (candsfrac + sumeps) / (rootsolval - candsol),
            DIVETYPE_SOS1VARIABLE => {
                if scip.set.is_feas_positive(candsol) {
                    (candsfrac + sumeps) / (rootsolval - candsol)
                } else {
                    (1.0 - candsfrac + sumeps) / (candsol - rootsolval)
                }
            }
            _ => return Err(crate::retcode::Retcode::InvalidData),
        };

        // Avoid roundable candidates.
        if var_may_round_down(scip, cand) {
            distquot *= 1000.0;
        }
    } else if scip.set.is_gt(candsol, rootsolval) {
        // Round up.
        distquot = match divetype {
            DIVETYPE_INTEGRALITY => (1.0 - candsfrac + sumeps) / (candsol - rootsolval),
            DIVETYPE_SOS1VARIABLE => {
                if scip.set.is_feas_positive(candsol) {
                    (1.0 - candsfrac + sumeps) / (candsol - rootsolval)
                } else {
                    (candsfrac + sumeps) / (rootsolval - candsol)
                }
            }
            _ => return Err(crate::retcode::Retcode::InvalidData),
        };

        // Avoid roundable candidates.
        if var_may_round_up(scip, cand) {
            distquot *= 1000.0;
        }
        *roundup = true;
    } else {
        // If the solution values are equal, we arbitrarily select branching
        // downwards; candidates with equal LP solution values are penalized
        // with an infinite score.
        *roundup = false;
        distquot = scip.set.infinity();
    }

    *score = -distquot;

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurLinesearchdiving`: creates the linesearchdiving heuristic and
/// includes it in SCIP.
pub fn include_heur_linesearchdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_linesearchdiving,
        Some(Box::new(HeurData { sol: NONE }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_linesearchdiving);
    h.heurinit = Some(heur_init_linesearchdiving);
    h.heurexit = Some(heur_exit_linesearchdiving);

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
        diveset_get_score_linesearchdiving,
        None,
    )?;

    Ok(())
}
