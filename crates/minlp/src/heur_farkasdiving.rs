//! Port of `heur_farkasdiving.c`: LP diving heuristic that tries to
//! construct a Farkas-proof.

use std::any::Any;

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DIVETYPE_SOS1VARIABLE, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, VarId},
    var::NONE,
};

const HEUR_NAME: &str = "farkasdiving";
const HEUR_DESC: &str = "LP diving heuristic that tries to construct a Farkas-proof";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -900000;
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
const DEFAULT_LPSOLVEFREQ: i32 = 1;
const DEFAULT_ONLYLPBRANCHCANDS: bool = false;
const DEFAULT_RANDSEED: u32 = 151;
/// Maximal occurance factor of an objective coefficient.
const DEFAULT_MAXOBJOCC: f64 = 1.0;
/// Minimal objective dynamism (log).
const DEFAULT_OBJDYN: f64 = 0.0001;
/// Should diving candidates be checked before running?
const DEFAULT_CHECKCANDS: bool = false;
/// Should the score be scaled?
const DEFAULT_SCALESCORE: bool = true;
/// Run if successfull at root.
const DEFAULT_ROOTSUCCESS: bool = true;
/// Scale score by [f]ractionality or [i]mpact on farkasproof.
const DEFAULT_SCALETYPE: u8 = b'i';
const MIN_RAND: f64 = 1e-06;
const MAX_RAND: f64 = 1e-05;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: crate::types::SolId,
    /// Maximal occurance factor of an objective coefficient.
    maxobjocc: f64,
    /// Minimal objective dynamism (log).
    objdynamism: f64,
    /// Remember if the heuristic should not run at all.
    disabled: bool,
    /// Remember whether one global check was performed.
    glbchecked: bool,
    /// Should diving candidates be checked before running?
    checkcands: bool,
    /// Should score be scaled by fractionality.
    scalescore: bool,
    /// Run if successfull at root.
    rootsuccess: bool,
    /// Was a solution found at the root node?
    foundrootsol: bool,
    /// Scale score by [f]ractionality or [i]mpact on farkasproof.
    scaletype: u8,
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

/// `heurCopyFarkasdiving`: copy method for primal heuristic plugins.
fn heur_copy_farkasdiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_farkasdiving(scip)
}

/// `heurInitFarkasdiving`: initialization method of primal heuristic.
fn heur_init_farkasdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    let h = hdm(scip, heur);
    h.sol = sol;
    h.disabled = false;
    h.glbchecked = false;
    Ok(())
}

/// `heurInitsolFarkasdiving`: solving process initialization method of
/// primal heuristic.
fn heur_initsol_farkasdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    let h = hdm(scip, heur);
    h.glbchecked = false;
    h.disabled = false;
    h.foundrootsol = false;
    Ok(())
}

/// `heurExitFarkasdiving`: deinitialization method of primal heuristic.
fn heur_exit_farkasdiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `checkDivingCandidates`: check whether the diving candidates fulfill
/// requirements.
fn check_diving_candidates(scip: &Scip, heur: HeurId, divecandvars: &[VarId], ndivecands: usize) -> bool {
    let h = hd(scip, heur);
    let mut success = false;

    // Terminate without candidates.
    if ndivecands == 0 {
        return success;
    }

    // Terminate without objective.
    if scip.probs[crate::prob::TRANSPROB].nobjvars == 0 {
        return success;
    }

    // Collect and count all non-zero absolute values of objective
    // coefficients for diving candidates.
    let mut objcoefs: Vec<f64> = Vec::with_capacity(ndivecands);
    for &var in &divecandvars[..ndivecands] {
        let obj = scip.vars[var].obj;

        if scip.set.is_zero(obj) {
            continue;
        }

        objcoefs.push(obj.abs());
    }
    let nnzobjcoefs = objcoefs.len();

    // Terminate without objcands.
    if nnzobjcoefs == 0 {
        return success;
    }

    success = true;

    // Skip here if we are cheching the global properties and want to check
    // the local candidates, too.
    if !h.glbchecked && h.checkcands {
        return success;
    }

    // Sort in increasing order.
    crate::sort::sort(&mut objcoefs[..], (), crate::sort::cmp_real);

    let mut lastobjcoef = objcoefs[0];

    let objdynamism = (objcoefs[nnzobjcoefs - 1] / objcoefs[0]).log10();

    // CHECK#2: check objective dynamism.
    if objdynamism < h.objdynamism {
        return false;
    }

    // CHECK#4: the check would be always fulfilled for maxobjocc = 1.0.
    if h.maxobjocc < 1.0 {
        let mut tmpmaxfreq = 0;
        let mut maxfreq = 0;

        // Count number of different absolute objective values.
        for &coef in &objcoefs[1..nnzobjcoefs] {
            if scip.set.is_gt(coef, lastobjcoef) {
                if tmpmaxfreq > maxfreq {
                    maxfreq = tmpmaxfreq;
                }
                tmpmaxfreq = 0;

                lastobjcoef = coef;
            } else {
                tmpmaxfreq += 1;
            }
        }

        if maxfreq as f64 > h.maxobjocc * nnzobjcoefs as f64 {
            success = false;
        }
    }

    success
}

/// `heurExecFarkasdiving`: execution method of primal heuristic.
fn heur_exec_farkasdiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Terminate if the heuristic has been disabled.
    if hd(scip, heur).disabled {
        return Ok(());
    }

    // Check some simple global properties that are needed to run this
    // heuristic.
    let h = hd(scip, heur);
    let mut success = !h.rootsuccess || h.foundrootsol || api::get_depth(scip) < 1;
    if success && !h.glbchecked {
        let vars = api::get_vars(scip).to_vec();
        let n = (api::get_n_vars(scip) - api::get_n_cont_vars(scip) - api::get_n_cont_impl_vars(scip)) as usize;
        success = check_diving_candidates(scip, heur, &vars, n);
        hdm(scip, heur).glbchecked = true;
    }

    // Disable heuristic if requirements missing.
    if !success {
        hdm(scip, heur).disabled = true;
        return Ok(());
    }

    // Check diving candidates in detail.
    if hd(scip, heur).checkcands {
        // We can only access the branching candidates if the LP is solved to
        // optimality.
        if api::get_lp_solstat(scip) == crate::types::LpSolStat::Optimal {
            let (divecandvars, _, _, ndivecands, _, _) = api::get_lp_branch_cands(scip)?;

            success = check_diving_candidates(scip, heur, &divecandvars, ndivecands as usize);
        } else {
            success = false;
        }
    }

    if success {
        let diveset = scip.heurs[heur].divesets[0];
        let sol = hd(scip, heur).sol;

        crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)?;

        if hd(scip, heur).rootsuccess && api::get_depth(scip) == 0 && crate::heur::diveset_get_n_sols(scip, diveset, DiveContext::Single) > 0 {
            hdm(scip, heur).foundrootsol = true;
        }
    }

    Ok(())
}

/// `divesetGetScoreFarkasdiving`: calculate score and preferred rounding
/// direction for the candidate variable.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_farkasdiving(
    scip: &mut Scip,
    diveset: DivesetId,
    _divetype: DiveType,
    cand: VarId,
    candsol: f64,
    candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let heur = scip.divesets[diveset].heur;
    let (scalescore, scaletype) = {
        let h = hd(scip, heur);
        (h.scalescore, h.scaletype)
    };

    let obj = scip.vars[cand].obj;

    // Dive towards the pseudosolution, at the same time approximate the
    // contribution to a potentially Farkas-proof (infeasibility proof) by
    // y^TA_i = c_i.
    if scip.set.is_negative(obj) {
        *roundup = true;
    } else if scip.set.is_positive(obj) {
        *roundup = false;
    } else if scip.set.is_eq(candsfrac, 0.5) {
        *roundup = scip.divesets[diveset].randnumgen.get_int(0, 1) == 0;
    } else {
        *roundup = candsfrac > 0.5;
    }

    // Larger score is better.
    *score = obj.abs() + scip.divesets[diveset].randnumgen.get_real(MIN_RAND, MAX_RAND);

    if scalescore {
        if scaletype == b'f' {
            if *roundup {
                *score *= 1.0 - candsfrac;
            } else {
                *score *= candsfrac;
            }
        } else if *roundup {
            *score *= scip.set.ceil(candsol) - scip.vars[cand].locdom.lb;
        } else {
            *score *= scip.vars[cand].locdom.ub - scip.set.floor(candsol);
        }
    }

    // Prefer decisions on binary variables.
    if scip.vars[cand].vartype != crate::types::VarType::Binary || crate::var::var_is_implied_integral(scip, cand) {
        *score = -1.0 / *score;
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurFarkasdiving`: creates the farkasdiving heuristic and
/// includes it in SCIP.
pub fn include_heur_farkasdiving(scip: &mut Scip) -> ScipResult {
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
        heur_exec_farkasdiving,
        Some(Box::new(HeurData {
            sol: NONE,
            maxobjocc: DEFAULT_MAXOBJOCC,
            objdynamism: DEFAULT_OBJDYN,
            disabled: false,
            glbchecked: false,
            checkcands: DEFAULT_CHECKCANDS,
            scalescore: DEFAULT_SCALESCORE,
            rootsuccess: DEFAULT_ROOTSUCCESS,
            foundrootsol: false,
            scaletype: DEFAULT_SCALETYPE,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_farkasdiving);
    h.heurinit = Some(heur_init_farkasdiving);
    h.heurexit = Some(heur_exit_farkasdiving);
    h.heurinitsol = Some(heur_initsol_farkasdiving);

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
        diveset_get_score_farkasdiving,
        None,
    )?;

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    use crate::paramset::{paramset_add_bool, paramset_add_char, paramset_add_real};
    paramset_add_bool(
        scip,
        "heuristics/farkasdiving/checkcands",
        "should diving candidates be checked before running?",
        t!(bool, checkcands),
        true,
        DEFAULT_CHECKCANDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/farkasdiving/scalescore",
        "should the score be scaled?",
        t!(bool, scalescore),
        true,
        DEFAULT_SCALESCORE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/farkasdiving/rootsuccess",
        "should the heuristic only run within the tree if at least one solution was found at the root node?",
        t!(bool, rootsuccess),
        true,
        DEFAULT_ROOTSUCCESS,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/farkasdiving/maxobjocc",
        "maximal occurance factor of an objective coefficient",
        t!(f64, maxobjocc),
        true,
        DEFAULT_MAXOBJOCC,
        0.0,
        1.0,
        None,
    )?;
    let inf = scip.set.infinity();
    paramset_add_real(
        scip,
        "heuristics/farkasdiving/objdynamism",
        "minimal objective dynamism (log) to run",
        t!(f64, objdynamism),
        true,
        DEFAULT_OBJDYN,
        0.0,
        inf,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/farkasdiving/scaletype",
        "scale score by [f]ractionality or [i]mpact on farkasproof",
        t!(u8, scaletype),
        true,
        DEFAULT_SCALETYPE,
        Some("fi"),
        None,
    )?;

    Ok(())
}
