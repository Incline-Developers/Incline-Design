//! Port of `heur_veclendiving.c`: LP diving heuristic that rounds variables with long column vectors.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, VarId},
    var::NONE,
};

const HEUR_NAME: &str = "veclendiving";
const HEUR_DESC: &str = "LP diving heuristic that rounds variables with long column vectors";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1003100;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 4;
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
const DEFAULT_MAXDIVEUBQUOTNOSOL: f64 = 0.1;
const DEFAULT_MAXDIVEAVGQUOTNOSOL: f64 = 0.0;
const DEFAULT_BACKTRACK: bool = true;
const DEFAULT_LPRESOLVEDOMCHGQUOT: f64 = 0.15;
const DEFAULT_LPSOLVEFREQ: i32 = 0;
const DEFAULT_ONLYLPBRANCHCANDS: bool = false;
const DEFAULT_RANDSEED: u32 = 113;

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

/// `heurCopyVeclendiving`: copy method for primal heuristic plugins.
fn heur_copy_veclendiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_veclendiving(scip)
}

/// `heurInitVeclendiving`: initialization method of primal heuristic.
fn heur_init_veclendiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitVeclendiving`: deinitialization method of primal heuristic.
fn heur_exit_veclendiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecVeclendiving`: execution method of primal heuristic.
fn heur_exec_veclendiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::DidNotRun;

    // Terminate if there are no integer variables (note that, e.g., SOS1
    // variables may be present).
    if api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip) == api::get_n_vars(scip) {
        return Ok(());
    }

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// `divesetGetScoreVeclendiving`: calculate score and preferred rounding
/// direction for the candidate variable; the best candidate maximizes the
/// score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_veclendiving(
    scip: &mut Scip,
    _diveset: DivesetId,
    _divetype: DiveType,
    cand: VarId,
    _candsol: f64,
    candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let obj = scip.vars[cand].obj;
    *roundup = obj >= 0.0;
    let objdelta = if *roundup { (1.0 - candsfrac) * obj } else { -candsfrac * obj };

    let colveclen = if scip.vars[cand].varstatus == crate::types::VarStatus::Column {
        scip.cols[scip.vars[cand].data.col].len as f64
    } else {
        0.0
    };

    // Larger score is better.
    *score = (colveclen + 1.0) / (objdelta + scip.set.sumepsilon());

    // Prefer decisions on binary variables.
    if scip.vars[cand].vartype != crate::types::VarType::Binary || crate::var::var_is_implied_integral(scip, cand) {
        *score *= 0.001;
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurVeclendiving`: creates the veclendiving heuristic and
/// includes it in SCIP.
pub fn include_heur_veclendiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_veclendiving,
        Some(Box::new(HeurData { sol: NONE }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_veclendiving);
    h.heurinit = Some(heur_init_veclendiving);
    h.heurexit = Some(heur_exit_veclendiving);

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
        diveset_get_score_veclendiving,
        None,
    )?;

    Ok(())
}
