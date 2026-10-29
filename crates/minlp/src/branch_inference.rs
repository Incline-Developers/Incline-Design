//! Port of `branch_inference.c`: inference history branching rule, which
//! sub-SCIPs of LNS heuristics branch with.

use crate::{
    api,
    def::{INVALID, UNKNOWN},
    history::History,
    retcode::ScipResult,
    scip::Scip,
    types::{BranchDir, BranchruleId, ScipResultCode, VarId},
    var::NONE,
};

const BRANCHRULE_NAME: &str = "inference";
const BRANCHRULE_DESC: &str = "inference history branching";
const BRANCHRULE_PRIORITY: i32 = 1000;
const BRANCHRULE_MAXDEPTH: i32 = -1;
const BRANCHRULE_MAXBOUNDDIST: f64 = 1.0;

const DEFAULT_CONFLICTWEIGHT: f64 = 1000.0;
const DEFAULT_CUTOFFWEIGHT: f64 = 1.0;
const DEFAULT_INFERENCEWEIGHT: f64 = 1.0;
const DEFAULT_RELIABLESCORE: f64 = 0.001;
const DEFAULT_FRACTIONALS: bool = true;
const DEFAULT_USEWEIGHTEDSUM: bool = true;
const DEFAULT_CONFLICTPRIO: i32 = 1;
const DEFAULT_CUTOFFPRIO: i32 = 1;

/// `SCIP_BranchruleData`.
#[derive(Default)]
struct BranchruleData {
    conflictweight: f64,
    cutoffweight: f64,
    inferenceweight: f64,
    reliablescore: f64,
    fractionals: bool,
    useweightedsum: bool,
    conflictprio: i32,
    cutoffprio: i32,
}

fn bd(scip: &Scip, branchrule: BranchruleId) -> &BranchruleData {
    scip.branchrules[branchrule].branchruledata.as_ref().unwrap().downcast_ref().unwrap()
}

/// The weights and settings one branching call uses.
#[derive(Clone, Copy)]
struct Weights {
    conflictweight: f64,
    inferenceweight: f64,
    cutoffweight: f64,
    reliablescore: f64,
}

/// `evaluateValueCand`: evaluates the given candidate with the given score
/// against the currently best known candidate, tiebreaking included.
#[allow(clippy::too_many_arguments)]
fn evaluate_value_cand(
    scip: &Scip,
    cand: VarId,
    score: f64,
    branchpoint: f64,
    branchdir: BranchDir,
    bestcand: &mut VarId,
    bestscore: &mut f64,
    bestbranchpoint: &mut f64,
    bestbranchdir: &mut BranchDir,
) {
    // Evaluate the candidate against the currently best candidate.
    if *bestscore < score {
        // The score of the candidate is better than the currently best known
        // candidate.
        *bestscore = score;
        *bestcand = cand;
        *bestbranchpoint = branchpoint;
        *bestbranchdir = branchdir;
    } else if *bestscore == score {
        let bestobj = scip.vars[*bestcand].obj.abs();
        let candobj = scip.vars[cand].obj.abs();

        // The candidate has the same score as the best known candidate;
        // therefore we use a second and third criteria to detect a unique best
        // candidate.
        if bestobj < candobj || (bestobj == candobj && scip.vars[*bestcand].index < scip.vars[cand].index) {
            *bestcand = cand;
            *bestbranchpoint = branchpoint;
            *bestbranchdir = branchdir;
        }
    }
}

/// `evaluateAggrCand`: evaluates the given candidate with the given score
/// against the currently best known candidate.
#[allow(clippy::too_many_arguments)]
fn evaluate_aggr_cand(
    scip: &Scip,
    cand: VarId,
    score: f64,
    val: f64,
    bestcand: &mut VarId,
    bestscore: &mut f64,
    bestval: &mut f64,
    bestcands: &mut [VarId],
    nbestcands: &mut usize,
) {
    // Evaluate the candidate against the currently best candidate.
    if *bestscore < score {
        // The score of the candidate is better than the currently best known
        // candidate, so it should be the first candidate in bestcands.
        *bestscore = score;
        *bestcand = cand;
        *bestval = val;
        *nbestcands = 1;
        bestcands[0] = cand;
    } else if scip.set.is_eq(*bestscore, score) {
        // The score of the candidate is comparable to the currently known
        // best, so we add it to bestcands.
        bestcands[*nbestcands] = cand;
        *nbestcands += 1;
    }
}

/// `tiebreakAggrCand`: chooses a singular best candidate from `bestcands`
/// and moves it to the beginning of the candidate array.
fn tiebreak_aggr_cand(scip: &Scip, bestcands: &mut [VarId], nbestcands: usize) {
    for c in 0..nbestcands {
        let bestobj = scip.vars[bestcands[0]].obj.abs();
        let candobj = scip.vars[bestcands[c]].obj.abs();

        // The candidate has the same score as the best known candidate;
        // therefore we use a second and third criteria to detect a unique best
        // candidate.
        if bestobj < candobj || (bestobj == candobj && scip.vars[bestcands[0]].index < scip.vars[bestcands[c]].index) {
            bestcands[0] = bestcands[c];
        }
    }
}

/// `checkValueScore`: checks if the score for the given domain value and
/// variable domain value is better than the current best known one.
#[allow(clippy::too_many_arguments)]
fn check_value_score(
    value: f64,
    history: &History,
    dir: BranchDir,
    conflictweight: f64,
    cutoffweight: f64,
    reliablescore: f64,
    bestscore: &mut f64,
    branchpoint: &mut f64,
    branchdir: &mut BranchDir,
) {
    let mut conflictscore = history.get_vsids(dir);
    let mut cutoffscore = history.get_cutoff_sum(dir);

    // In case the conflict score is below the reliable score we set it to
    // zero since it is seen to be unreliable.
    if conflictscore < reliablescore {
        conflictscore = 0.0;
    }

    // In case the cutoff score is below the reliable score we set it to zero
    // since it is seen to be unreliable.
    if cutoffscore < reliablescore {
        cutoffscore = 0.0;
    }

    // Compute weight score.
    let score = conflictweight * conflictscore + cutoffweight * cutoffscore;

    if score > *bestscore {
        *bestscore = score;
        *branchpoint = value;
        *branchdir = dir;
    }
}

/// `getAggrScore`: an aggregated score for the given variable using the
/// conflict score and cutoff score.
fn get_aggr_score(scip: &Scip, var: VarId, w: Weights) -> f64 {
    let mut conflictscore = crate::scip_var::get_var_conflict_score(scip, var);
    let mut cutoffscore = get_var_avg_inference_cutoff_score(scip, var, w.cutoffweight);

    // In case the conflict score is below the reliable score we set it to
    // zero since it is seen to be unreliable.
    if conflictscore < w.reliablescore {
        conflictscore = 0.0;
    }

    // In case the cutoff score is below the reliable score we set it to zero
    // since it is seen to be unreliable.
    if cutoffscore < w.reliablescore {
        cutoffscore = 0.0;
    }

    // Compute weighted score for the candidate.
    w.conflictweight * conflictscore + w.inferenceweight * cutoffscore
}

/// `SCIPgetVarAvgInferenceCutoffScore` (`scip_var.c`): the variable's
/// average inference/cutoff score value, weighting the cutoffs of the
/// variable with the given factor.
pub fn get_var_avg_inference_cutoff_score(scip: &Scip, var: VarId, cutoffweight: f64) -> f64 {
    let avginferdown = scip.stat.glbhistory.get_avg_inferences(BranchDir::Downwards);
    let avginferup = scip.stat.glbhistory.get_avg_inferences(BranchDir::Upwards);
    let avginfer = (avginferdown + avginferup) / 2.0;
    let inferdown = crate::var::var_get_avg_inferences(scip, var, BranchDir::Downwards);
    let inferup = crate::var::var_get_avg_inferences(scip, var, BranchDir::Upwards);
    let cutoffdown = crate::var::var_get_avg_cutoffs(scip, var, BranchDir::Downwards);
    let cutoffup = crate::var::var_get_avg_cutoffs(scip, var, BranchDir::Upwards);

    crate::branch::branch_get_score(scip, var, inferdown + cutoffweight * avginfer * cutoffdown, inferup + cutoffweight * avginfer * cutoffup)
}

/// `getValueScore`: the best domain value score of the given variable,
/// with its branching point and direction.
fn get_value_score(scip: &Scip, var: VarId, conflictweight: f64, cutoffweight: f64, reliablescore: f64, branchpoint: &mut f64, branchdir: &mut BranchDir) -> f64 {
    *branchpoint = UNKNOWN;
    *branchdir = BranchDir::Upwards;

    let mut bestscore = 0.0;

    if let Some(valuehistory) = scip.vars[var].valuehistory.as_ref() {
        let v = &scip.vars[var];
        for (value, history) in valuehistory.values.iter().zip(valuehistory.histories.iter()) {
            let value = *value;

            // Skip all domain values which are smaller or equal to the lower
            // bound.
            if value <= v.locdom.lb {
                continue;
            }

            // Skip all domain values which are larger or equal to the upper
            // bound.
            if value >= v.locdom.ub {
                break;
            }

            // Check var <= value.
            check_value_score(
                value,
                history,
                BranchDir::Downwards,
                conflictweight,
                cutoffweight,
                reliablescore,
                &mut bestscore,
                branchpoint,
                branchdir,
            );

            // Check var >= value.
            check_value_score(
                value,
                history,
                BranchDir::Upwards,
                conflictweight,
                cutoffweight,
                reliablescore,
                &mut bestscore,
                branchpoint,
                branchdir,
            );
        }
    }

    bestscore
}

/// `selectBestCands`: the candidates of best aggregated score.
fn select_best_cands(scip: &Scip, cands: &[VarId], candsols: &[f64], w: Weights, bestcands: &mut [VarId], nbestcands: &mut usize) {
    let mut bestaggrcand = cands[0];
    let mut bestval = candsols[0];
    bestcands[0] = cands[0];
    *nbestcands = 1;

    // Get aggregated score for the first candidate.
    let mut bestaggrscore = get_aggr_score(scip, cands[0], w);

    for c in 1..cands.len() {
        let cand = cands[c];
        let val = candsols[c];

        // Get score for the candidate.
        let aggrscore = get_aggr_score(scip, cand, w);

        // Evaluate the candidate against the currently best candidate w.r.t.
        // aggregated score.
        evaluate_aggr_cand(scip, cand, aggrscore, val, &mut bestaggrcand, &mut bestaggrscore, &mut bestval, bestcands, nbestcands);
    }
}

/// `performBranchingSol`: selects a variable out of the given candidate
/// array and performs the branching.
#[allow(clippy::too_many_arguments)]
fn perform_branching_sol(
    scip: &mut Scip,
    cands: &[VarId],
    candsols: &[f64],
    mut w: Weights,
    useweightedsum: bool,
    result: &mut ScipResultCode,
    mut conflictprio: i32,
    mut cutoffprio: i32,
) -> ScipResult {
    let ncands = cands.len();
    *result = ScipResultCode::DidNotFind;

    // Check if conflict score, inferences, and cutoff score should be used in
    // combination; otherwise just use inference.
    if !useweightedsum {
        conflictprio = 0;
        cutoffprio = 0;
        w.conflictweight = 0.0;
        w.inferenceweight = 1.0;
        w.cutoffweight = 0.0;
    }

    let mut bestcands = vec![NONE; ncands];
    let mut nbestcands = 0;

    let first = |cw: f64, iw: f64, cuw: f64| Weights {
        conflictweight: cw,
        inferenceweight: iw,
        cutoffweight: cuw,
        reliablescore: w.reliablescore,
    };
    if conflictprio > cutoffprio {
        // Select the best candidates w.r.t. the first criterion.
        select_best_cands(scip, cands, candsols, first(w.conflictweight, 0.0, 0.0), &mut bestcands, &mut nbestcands);

        // Select the best candidates w.r.t. the second criterion; we use
        // bestcands and nbestcands as input and output.
        if nbestcands > 1 {
            let input = bestcands[..nbestcands].to_vec();
            select_best_cands(scip, &input, candsols, first(0.0, w.inferenceweight, w.cutoffweight), &mut bestcands, &mut nbestcands);
        }
    } else if conflictprio == cutoffprio {
        // Select the best candidates w.r.t. weighted sum of both criteria.
        select_best_cands(scip, cands, candsols, w, &mut bestcands, &mut nbestcands);
    } else {
        // Select the best candidates w.r.t. the first criterion.
        select_best_cands(scip, cands, candsols, first(0.0, w.inferenceweight, w.cutoffweight), &mut bestcands, &mut nbestcands);

        // Select the best candidates w.r.t. the second criterion; we use
        // bestcands and nbestcands as input and output.
        if nbestcands > 1 {
            let input = bestcands[..nbestcands].to_vec();
            select_best_cands(scip, &input, candsols, first(w.conflictweight, 0.0, 0.0), &mut bestcands, &mut nbestcands);
        }
    }

    // Final tie breaking.
    if nbestcands > 1 {
        tiebreak_aggr_cand(scip, &mut bestcands, nbestcands);
    }

    let bestaggrcand = bestcands[0];
    let mut bestval = -INVALID;

    // Loop over cands, find bestcands[0], and store corresponding candsols
    // value in bestval.
    for c in 0..ncands {
        if bestaggrcand == cands[c] {
            bestval = candsols[c];
            break;
        }
    }

    // Perform the branching.
    let point = api::get_branching_point(scip, bestaggrcand, bestval);
    let children = api::branch_var_val(scip, bestaggrcand, point)?;

    if children.downchild != NONE || children.eqchild != NONE || children.upchild != NONE {
        *result = ScipResultCode::Branched;
    } else {
        // If there are no children, then variable should have been fixed by
        // SCIPbranchVar(Val).
        *result = ScipResultCode::ReducedDom;
    }
    Ok(())
}

/// `performBranchingNoSol`: selects a variable out of the given candidate
/// array and performs the branching.
fn perform_branching_no_sol(scip: &mut Scip, cands: &[VarId], w: Weights, useweightedsum: bool, result: &mut ScipResultCode) -> ScipResult {
    let ncands = cands.len();
    let mut bestbranchpoint = UNKNOWN;
    let mut bestbranchdir = BranchDir::Downwards;
    let mut bestvaluecand = NONE;

    *result = ScipResultCode::DidNotFind;

    let mut bestcands = vec![NONE; ncands];
    let mut nbestcands = 0;

    let mut bestaggrcand = cands[0];
    let mut bestval = UNKNOWN;

    // Check if the weighted sum between the average inferences and conflict
    // score should be used.
    if useweightedsum {
        bestvaluecand = cands[0];

        // Get domain value score for the first candidate.
        let mut bestvaluescore = get_value_score(scip, cands[0], w.conflictweight, w.cutoffweight, w.reliablescore, &mut bestbranchpoint, &mut bestbranchdir);

        // Get aggregated score for the first candidate.
        let mut bestaggrscore = get_aggr_score(scip, cands[0], w);

        for &cand in &cands[1..] {
            let val = UNKNOWN;

            // Get domain value score for the candidate.
            let mut branchpoint = 0.0;
            let mut branchdir = BranchDir::Upwards;
            let valuescore = get_value_score(scip, cand, w.conflictweight, w.cutoffweight, w.reliablescore, &mut branchpoint, &mut branchdir);

            // Evaluate the candidate against the currently best candidate
            // w.r.t. domain value score.
            evaluate_value_cand(
                scip,
                cand,
                valuescore,
                branchpoint,
                branchdir,
                &mut bestvaluecand,
                &mut bestvaluescore,
                &mut bestbranchpoint,
                &mut bestbranchdir,
            );

            // Get aggregated score for the candidate.
            let aggrscore = get_aggr_score(scip, cand, w);

            // Evaluate the candidate against the currently best candidate
            // w.r.t. aggregated score.
            evaluate_aggr_cand(
                scip,
                cand,
                aggrscore,
                val,
                &mut bestaggrcand,
                &mut bestaggrscore,
                &mut bestval,
                &mut bestcands,
                &mut nbestcands,
            );
        }
    } else {
        let mut bestaggrscore = crate::scip_var::get_var_avg_inference_score(scip, cands[0]);

        // Search for variable with best score w.r.t. average inferences per
        // branching.
        for &cand in &cands[1..] {
            let val = UNKNOWN;

            let mut aggrscore = crate::scip_var::get_var_avg_inference_score(scip, cand);

            // In case the average inferences score is below the reliable score
            // we set it to zero since it is seen to be unreliable.
            if aggrscore < w.reliablescore {
                aggrscore = 0.0;
            }

            // Evaluate the candidate against the currently best candidate.
            evaluate_aggr_cand(
                scip,
                cand,
                aggrscore,
                val,
                &mut bestaggrcand,
                &mut bestaggrscore,
                &mut bestval,
                &mut bestcands,
                &mut nbestcands,
            );
        }
    }

    let children = if bestbranchpoint == UNKNOWN {
        api::branch_var(scip, bestaggrcand)?
    } else {
        // Perform the branching.
        let mut downprio = 0.0;
        let mut upprio = 0.0;
        let (downub, uplb);
        if bestbranchdir == BranchDir::Downwards {
            downprio = 1.0;
            downub = bestbranchpoint;
            uplb = bestbranchpoint + 1.0;
        } else {
            upprio = 1.0;
            downub = bestbranchpoint - 1.0;
            uplb = bestbranchpoint;
        }

        // Calculate the child estimate and create down child; change upper
        // bound in down child.
        let estimate = api::calc_child_estimate(scip, bestvaluecand, downub);
        let downchild = api::create_child(scip, downprio, estimate)?;
        crate::scip_var::chg_var_ub_node(scip, downchild, bestvaluecand, downub)?;

        // Calculate the child estimate and create up child; change lower bound
        // in up child.
        let estimate = api::calc_child_estimate(scip, bestvaluecand, uplb);
        let upchild = api::create_child(scip, upprio, estimate)?;
        crate::scip_var::chg_var_lb_node(scip, upchild, bestvaluecand, uplb)?;

        crate::tree::BranchChildren {
            downchild,
            eqchild: NONE,
            upchild,
        }
    };

    if children.downchild != NONE || children.eqchild != NONE || children.upchild != NONE {
        *result = ScipResultCode::Branched;
    } else {
        // If there are no children, then variable should have been fixed by
        // SCIPbranchVar(Val).
        *result = ScipResultCode::ReducedDom;
    }
    Ok(())
}

/*
 * Callback methods
 */

/// `branchCopyInference`: copy method for branchrule plugins (called when
/// SCIP copies plugins).
fn branch_copy_inference(scip: &mut Scip, _sourcescip: &Scip, _branchrule: BranchruleId) -> ScipResult {
    // Call inclusion method of branchrule.
    include_branchrule_inference(scip)
}

fn weights(d: &BranchruleData) -> Weights {
    Weights {
        conflictweight: d.conflictweight,
        inferenceweight: d.inferenceweight,
        cutoffweight: d.cutoffweight,
        reliablescore: d.reliablescore,
    }
}

/// `branchExeclpInference`: branching execution method for fractional LP
/// solutions.
fn branch_execlp_inference(scip: &mut Scip, branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    let d = bd(scip, branchrule);
    let (w, useweightedsum, fractionals) = (weights(d), d.useweightedsum, d.fractionals);

    let cands = if fractionals {
        // Get LP candidates (fractional integer variables) with maximal
        // priority.
        let (lpcands, _, _, _, npriolpcands, _) = api::get_lp_branch_cands(scip)?;
        lpcands[..npriolpcands as usize].to_vec()
    } else {
        // Get pseudo candidates (non-fixed integer variables) with maximal
        // priority.
        pseudo_branch_cands(scip)
    };

    // Perform the branching.
    perform_branching_no_sol(scip, &cands, w, useweightedsum, result)
}

/// The pseudo branching candidates of maximal priority
/// (`SCIPgetPseudoBranchCands` with `npriopseudocands`).
fn pseudo_branch_cands(scip: &Scip) -> Vec<VarId> {
    let (_, npriopseudocands) = crate::branch::branchcand_get_pseudo_cands(scip);
    scip.branchcand.pseudocands.0[..npriopseudocands as usize].to_vec()
}

/// `branchExecextInference`: branching execution method for external
/// candidates.
fn branch_execext_inference(scip: &mut Scip, branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    let d = bd(scip, branchrule);
    let (w, useweightedsum, conflictprio, cutoffprio) = (weights(d), d.useweightedsum, d.conflictprio, d.cutoffprio);

    // Get branching candidates.
    let (cands, candsols, _) = api::get_extern_branch_cands(scip);

    // Perform the branching.
    perform_branching_sol(scip, &cands, &candsols, w, useweightedsum, result, conflictprio, cutoffprio)
}

/// `branchExecpsInference`: branching execution method for not completely
/// fixed pseudo solutions.
fn branch_execps_inference(scip: &mut Scip, branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    let d = bd(scip, branchrule);
    let (w, useweightedsum) = (weights(d), d.useweightedsum);

    // Get pseudo candidates (non-fixed integer variables).
    let cands = pseudo_branch_cands(scip);

    // Perform the branching.
    perform_branching_no_sol(scip, &cands, w, useweightedsum, result)
}

/*
 * branching specific interface methods
 */

/// `SCIPincludeBranchruleInference`: creates the inference history
/// branching rule and includes it in SCIP.
pub fn include_branchrule_inference(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Include branching rule.
    let methods = crate::branch::BranchruleMethods {
        branchcopy: Some(branch_copy_inference),
        branchexeclp: Some(branch_execlp_inference),
        branchexecext: Some(branch_execext_inference),
        branchexecps: Some(branch_execps_inference),
        ..Default::default()
    };
    let b = api::include_branchrule_basic(
        scip,
        BRANCHRULE_NAME,
        BRANCHRULE_DESC,
        BRANCHRULE_PRIORITY,
        BRANCHRULE_MAXDEPTH,
        BRANCHRULE_MAXBOUNDDIST,
        methods,
        Some(Box::new(BranchruleData::default())),
    )?;

    macro_rules! t {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, branchrules, b, branchruledata, BranchruleData, $field)
        };
    }

    // Inference branching rule parameters.
    paramset_add_real(
        scip,
        "branching/inference/conflictweight",
        "weight in score calculations for conflict score",
        t!(f64, conflictweight),
        true,
        DEFAULT_CONFLICTWEIGHT,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/inference/inferenceweight",
        "weight in score calculations for inference score",
        t!(f64, inferenceweight),
        true,
        DEFAULT_INFERENCEWEIGHT,
        crate::def::REAL_MIN,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/inference/cutoffweight",
        "weight in score calculations for cutoff score",
        t!(f64, cutoffweight),
        true,
        DEFAULT_CUTOFFWEIGHT,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/inference/fractionals",
        "should branching on LP solution be restricted to the fractional variables?",
        t!(bool, fractionals),
        true,
        DEFAULT_FRACTIONALS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/inference/useweightedsum",
        "should a weighted sum of inference, conflict and cutoff weights be used?",
        t!(bool, useweightedsum),
        false,
        DEFAULT_USEWEIGHTEDSUM,
        None,
    )?;

    // Inference branching rule parameters.
    paramset_add_real(
        scip,
        "branching/inference/reliablescore",
        "weight in score calculations for conflict score",
        t!(f64, reliablescore),
        true,
        DEFAULT_RELIABLESCORE,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;

    // Parameters for lexicographical ordering.
    paramset_add_int(
        scip,
        "branching/inference/conflictprio",
        "priority value for using conflict weights in lex. order",
        t!(i32, conflictprio),
        false,
        DEFAULT_CONFLICTPRIO,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "branching/inference/cutoffprio",
        "priority value for using cutoff weights in lex. order",
        t!(i32, cutoffprio),
        false,
        DEFAULT_CUTOFFPRIO,
        0,
        i32::MAX,
        None,
    )?;

    Ok(())
}
