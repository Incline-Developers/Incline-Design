//! Port of `branch_mostinf.c`: most infeasible LP branching rule.

use crate::{
    api,
    def::INVALID,
    retcode::ScipResult,
    scip::Scip,
    scip_var::compute_var_lb_local,
    types::{BranchruleId, ScipResultCode, VarId, VarStatus},
    var::{NONE, var_get_probvar},
};

const BRANCHRULE_NAME: &str = "mostinf";
const BRANCHRULE_DESC: &str = "most infeasible branching";
const BRANCHRULE_PRIORITY: i32 = 100;
const BRANCHRULE_MAXDEPTH: i32 = -1;
const BRANCHRULE_MAXBOUNDDIST: f64 = 1.0;

/*
 * Local methods
 */

/// The best candidate of [`update_best_candidate`].
struct Best {
    var: VarId,
    score: f64,
    obj: f64,
    sol: f64,
}

/// `updateBestCandidate`: compares the so far best branching candidate with
/// a new candidate and updates best candidate, if new candidate is better.
fn update_best_candidate(scip: &Scip, best: &mut Best, mut cand: VarId, mut candscore: f64, candsol: f64) {
    use crate::scip_var::compute_var_ub_local;

    if scip.vars[var_get_probvar(scip, cand)].varstatus == VarStatus::MultAggr {
        // For a multi-aggregated variable, we call updateBestCandidate function
        // recursively with all variables in the multi-aggregation.
        cand = var_get_probvar(scip, cand);
        let m = &scip.vars[cand].data.multaggr;
        let nmultvars = m.nvars as usize;
        let multvars = m.vars.0[..nmultvars].to_vec();

        // If we have a candidate branching point, then first register only
        // aggregation variables for which we can compute a corresponding
        // branching point too (see also comments below); if this fails, then
        // register all (unfixed) aggregation variables, thereby forgetting about
        // candsol.
        let mut success = false;
        if candsol != INVALID {
            let multscalars = m.scalars.0[..nmultvars].to_vec();

            // For computing the branching point, we need the current bounds of the
            // multi-aggregated variable.
            let minact = compute_var_lb_local(scip, cand);
            let maxact = compute_var_ub_local(scip, cand);

            for i in 0..nmultvars {
                // Skip fixed variables.
                let multvarlb = compute_var_lb_local(scip, multvars[i]);
                let multvarub = compute_var_ub_local(scip, multvars[i]);
                if scip.set.is_eq(multvarlb, multvarub) {
                    continue;
                }

                // We cannot ensure that both the upper bound in the left node and the
                // lower bound in the right node will be candsol by a clever choice for
                // the branching point of multvars[i], but we can try to ensure that at
                // least one of them will be at candsol.
                let (aggrvarsol1, aggrvarsol2) = if multscalars[i] > 0.0 {
                    //   cand >= candsol
                    // if multvars[i] >= (candsol - (maxact - multscalars[i] * ub(multvars[i]))) / multscalars[i]
                    //            = (candsol - maxact) / multscalars[i] + ub(multvars[i])
                    //    cand <= candsol
                    // if multvars[i] <= (candsol - (minact - multscalar[i] * lb(multvars[i]))) / multscalars[i]
                    //            = (candsol - minact) / multscalars[i] + lb(multvars[i])
                    ((candsol - maxact) / multscalars[i] + multvarub, (candsol - minact) / multscalars[i] + multvarlb)
                } else {
                    //   cand >= candsol
                    // if multvars[i] <= (candsol - (maxact - multscalars[i] * lb(multvars[i]))) / multscalars[i]
                    //            = (candsol - maxact) / multscalars[i] + lb(multvars[i])
                    //   cand <= candsol
                    // if multvars[i] >= (candsol - (minact - multscalar[i] * ub(multvars[i]))) / multscalars[i]
                    //            = (candsol - minact) / multscalars[i] + ub(multvars[i])
                    ((candsol - minact) / multscalars[i] + multvarub, (candsol - maxact) / multscalars[i] + multvarlb)
                };

                // By the above choice, aggrvarsol1 <= ub(multvars[i]) and aggrvarsol2
                // >= lb(multvars[i]); if aggrvarsol1 <= lb(multvars[i]) or aggrvarsol2
                // >= ub(multvars[i]), then choose the other one; if both are out of
                // bounds, then give up; if both are inside bounds, then choose the one
                // closer to 0.0 (someone has better idea???).
                let aggrvarsol = if scip.set.is_feas_le(aggrvarsol1, multvarlb) {
                    if scip.set.is_feas_ge(aggrvarsol2, multvarub) {
                        continue;
                    } else {
                        aggrvarsol2
                    }
                } else if scip.set.is_feas_ge(aggrvarsol2, multvarub) {
                    aggrvarsol1
                } else if aggrvarsol1.abs() < aggrvarsol2.abs() {
                    aggrvarsol1
                } else {
                    aggrvarsol2
                };
                success = true;

                update_best_candidate(scip, best, multvars[i], candscore, aggrvarsol);
            }
        }

        if !success {
            for &multvar in &multvars {
                // Skip fixed variables.
                let multvarlb = compute_var_lb_local(scip, multvar);
                let multvarub = compute_var_ub_local(scip, multvar);
                if scip.set.is_eq(multvarlb, multvarub) {
                    continue;
                }

                update_best_candidate(scip, best, multvar, candscore, INVALID);
            }
        }
        return;
    }

    candscore *= scip.vars[cand].branchfactor;
    let obj = scip.vars[cand].obj.abs();
    let set = &scip.set;
    if set.is_infinity(candscore) || (!set.is_infinity(best.score) && (set.is_gt(candscore, best.score) || (set.is_ge(candscore, best.score) && obj > best.obj))) {
        best.var = cand;
        best.score = candscore;
        best.obj = obj;
        best.sol = candsol;
    }
}

/*
 * Callback methods
 */

/// `branchExeclpMostinf`: branching execution method for fractional LP
/// solutions.
fn branch_execlp_mostinf(scip: &mut Scip, _branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    // Get branching candidates.
    let (lpcands, _, lpcandsfrac, nlpcands, _, _) = api::get_lp_branch_cands(scip)?;

    // Search the most infeasible candidate.
    let mut bestscore = crate::def::REAL_MIN;
    let mut bestobj = 0.0;
    let mut bestcand: i32 = -1;
    for i in 0..nlpcands as usize {
        let mut infeasibility = lpcandsfrac[i];
        infeasibility = crate::def::min(infeasibility, 1.0 - infeasibility);
        let mut score = infeasibility;
        score *= scip.vars[lpcands[i]].branchfactor;
        let obj = scip.vars[lpcands[i]].obj.abs();
        if scip.set.is_gt(score, bestscore) || (scip.set.is_ge(score, bestscore) && obj > bestobj) {
            bestscore = score;
            bestobj = obj;
            bestcand = i as i32;
        }
    }

    // Perform the branching.
    api::branch_var(scip, lpcands[bestcand as usize])?;
    *result = ScipResultCode::Branched;

    Ok(())
}

/// `branchExecextMostinf`: branching execution method for external
/// candidates.
fn branch_execext_mostinf(scip: &mut Scip, _branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    // Get branching candidates.
    let (externcands, externcandssol, externcandsscore) = api::get_extern_branch_cands(scip);

    // Search the most infeasible candidate.
    let mut best = Best {
        var: NONE,
        score: crate::def::REAL_MIN,
        obj: 0.0,
        sol: INVALID,
    };
    for i in 0..externcands.len() {
        update_best_candidate(scip, &mut best, externcands[i], externcandsscore[i], externcandssol[i]);
    }

    if best.var == NONE {
        // branchExecextMostinf failed to select a branching variable.
        log::error!("branchExecextMostinf failed to select a branching variable from {} candidates", externcands.len());
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    let brpoint = api::get_branching_point(scip, best.var, best.sol);

    // Perform the branching.
    let c = api::branch_var_val(scip, best.var, brpoint)?;

    if c.downchild != NONE || c.eqchild != NONE || c.upchild != NONE {
        *result = ScipResultCode::Branched;
    } else {
        // If there are no children, then variable should have been fixed by
        // SCIPbranchVarVal.
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/*
 * branching specific interface methods
 */

/// `branchCopyMostinf`: copy method for branchrule plugins (called when SCIP
/// copies plugins).
fn branch_copy_mostinf(scip: &mut Scip, _sourcescip: &Scip, _branchrule: BranchruleId) -> ScipResult {
    // Call inclusion method of branchrule.
    include_branchrule_mostinf(scip)?;
    Ok(())
}

/// `SCIPincludeBranchruleMostinf`: creates the most infeasible LP branching
/// rule and includes it in SCIP.
pub fn include_branchrule_mostinf(scip: &mut Scip) -> ScipResult {
    let methods = crate::branch::BranchruleMethods {
        branchexeclp: Some(branch_execlp_mostinf),
        branchexecext: Some(branch_execext_mostinf),
        ..Default::default()
    };
    let branchrule = api::include_branchrule_basic(
        scip,
        BRANCHRULE_NAME,
        BRANCHRULE_DESC,
        BRANCHRULE_PRIORITY,
        BRANCHRULE_MAXDEPTH,
        BRANCHRULE_MAXBOUNDDIST,
        methods,
        None,
    )?;
    scip.branchrules[branchrule].branchcopy = Some(branch_copy_mostinf);

    Ok(())
}
