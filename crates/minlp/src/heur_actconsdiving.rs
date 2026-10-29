//! Port of `heur_actconsdiving.c`: LP diving heuristic that chooses
//! fixings w.r.t. the active constraints.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, VarId, VarStatus},
    var::{NONE, var_is_binary, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "actconsdiving";
const HEUR_DESC: &str = "LP diving heuristic that chooses fixings w.r.t. the active constraints";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1003700;
const HEUR_FREQ: i32 = -1;
const HEUR_FREQOFS: i32 = 5;
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
const DEFAULT_ONLYLPBRANCHCANDS: bool = true;
const DEFAULT_RANDSEED: u32 = 149;

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
 * local methods
 */

/// `getNActiveConsScore`: returns a score value for the given variable
/// based on the active constraints that the variable appears in, with the
/// scores for branching downwards and upwards.
fn get_n_active_cons_score(scip: &mut Scip, sol: crate::types::SolId, var: VarId) -> (f64, f64, f64) {
    if scip.vars[var].varstatus != VarStatus::Column {
        return (0.0, 0.0, 0.0);
    }

    let col = scip.vars[var].data.col;
    let nrows = scip.cols[col].nlprows as usize;

    let mut nactrows = 0;
    let mut downcoefsum = 0.0;
    let mut upcoefsum = 0.0;

    for r in 0..nrows {
        let row = scip.cols[col].rows[r];

        // Calculate number of active constraint sides, i.e., count equations
        // as two.
        let lhs = scip.rows[row].lhs;
        let rhs = scip.rows[row].rhs;

        let activity = api::get_row_sol_activity(scip, row, sol);
        let dualsol = scip.rows[row].dualsol;
        if scip.set.is_feas_eq(activity, lhs) {
            nactrows += 1;
            let coef = scip.cols[col].vals[r] / crate::lp::row_get_norm(&scip.rows[row]);
            if scip.set.is_feas_positive(dualsol) {
                if coef > 0.0 {
                    downcoefsum += coef;
                } else {
                    upcoefsum -= coef;
                }
            }
        } else if scip.set.is_feas_eq(activity, rhs) {
            nactrows += 1;
            let coef = scip.cols[col].vals[r] / crate::lp::row_get_norm(&scip.rows[row]);
            if scip.set.is_feas_negative(dualsol) {
                if coef > 0.0 {
                    upcoefsum += coef;
                } else {
                    downcoefsum -= coef;
                }
            }
        }
    }

    // Use the number of LP rows for normalization.
    let nlprows = api::get_n_lp_rows(scip) as f64;
    upcoefsum /= nlprows;
    downcoefsum /= nlprows;

    // Calculate the score using SCIP's branch score. Pass NULL as variable
    // to not have the var branch factor influence the result.
    let score = nactrows as f64 / nlprows + crate::branch::branch_get_score(scip, NONE, downcoefsum, upcoefsum);

    (score, downcoefsum, upcoefsum)
}

/*
 * Callback methods
 */

/// `heurCopyActconsdiving`: copy method for primal heuristic plugins.
fn heur_copy_actconsdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_actconsdiving(scip)
}

/// `heurInitActconsdiving`: initialization method of primal heuristic.
fn heur_init_actconsdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitActconsdiving`: deinitialization method of primal heuristic.
fn heur_exit_actconsdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurExecActconsdiving`: execution method of primal heuristic.
fn heur_exec_actconsdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let sol = hd(scip, heur).sol;
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::DidNotRun;

    // If there are no integer variables, stop execution.
    if api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip) == api::get_n_vars(scip) {
        return Ok(());
    }

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)
}

/// `divesetGetScoreActconsdiving`: calculate score and preferred rounding
/// direction for the candidate variable; the best candidate maximizes the
/// score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_actconsdiving(
    scip: &mut Scip,
    diveset: DivesetId,
    _divetype: DiveType,
    cand: VarId,
    _candsol: f64,
    mut candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let mayrounddown = var_may_round_down(scip, cand);
    let mayroundup = var_may_round_up(scip, cand);

    // First, calculate the variable score.
    let worksol = scip.divesets[diveset].sol;
    let (s, downscore, upscore) = get_n_active_cons_score(scip, worksol, cand);
    *score = s;

    // Get the rounding direction: prefer an unroundable direction.
    if mayrounddown && mayroundup {
        // Try to avoid variability; decide randomly if the LP solution can
        // contain some noise.
        if scip.set.is_eq(candsfrac, 0.5) {
            *roundup = scip.divesets[diveset].randnumgen.get_int(0, 1) == 0;
        } else {
            *roundup = candsfrac > 0.5;
        }
    } else if mayrounddown || mayroundup {
        *roundup = mayrounddown;
    } else {
        *roundup = downscore > upscore;
    }

    if *roundup {
        candsfrac = 1.0 - candsfrac;
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
        *score *= 0.01;
    }

    // Penalize variable if it may be rounded.
    if mayrounddown || mayroundup {
        *score -= 3.0;
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurActconsdiving`: creates the actconsdiving heuristic and
/// includes it in SCIP.
pub fn include_heur_actconsdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_actconsdiving,
        Some(Box::new(HeurData { sol: NONE }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_actconsdiving);
    h.heurinit = Some(heur_init_actconsdiving);
    h.heurexit = Some(heur_exit_actconsdiving);

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
        diveset_get_score_actconsdiving,
        None,
    )?;

    Ok(())
}
