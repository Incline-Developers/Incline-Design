//! Port of `branch_pscost.c`: pseudo costs branching rule.

use crate::{
    api,
    def::INVALID,
    misc::RandNumGen,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_var::{adjusted_var_lb, adjusted_var_ub, compute_var_lb_local, compute_var_ub_local},
    types::{BranchruleId, ScipResultCode, VarId, VarStatus, VarType},
    var::{NONE, var_get_probvar},
};

const BRANCHRULE_NAME: &str = "pscost";
const BRANCHRULE_DESC: &str = "branching on pseudo cost values";
const BRANCHRULE_PRIORITY: i32 = 2000;
const BRANCHRULE_MAXDEPTH: i32 = -1;
const BRANCHRULE_MAXBOUNDDIST: f64 = 1.0;

/// Possible pseudo cost multiplication strategies for branching on external
/// candidates.
const BRANCHRULE_STRATEGIES: &str = "dsuv";
/// Default pseudo cost multiplication strategy.
const BRANCHRULE_STRATEGY_DEFAULT: u8 = b'u';
/// Default weight for minimum of scores of a branching candidate.
const BRANCHRULE_SCOREMINWEIGHT_DEFAULT: f64 = 0.8;
/// Default weight for maximum of scores of a branching candidate.
const BRANCHRULE_SCOREMAXWEIGHT_DEFAULT: f64 = 1.3;
/// Default weight for sum of scores of a branching candidate.
const BRANCHRULE_SCORESUMWEIGHT_DEFAULT: f64 = 0.1;
/// Default number of children in n-ary branching.
const BRANCHRULE_NCHILDREN_DEFAULT: i32 = 2;
/// Default maximal depth where to do n-ary branching.
const BRANCHRULE_NARYMAXDEPTH_DEFAULT: i32 = -1;
/// Default minimal domain width in children when doing n-ary branching.
const BRANCHRULE_NARYMINWIDTH_DEFAULT: f64 = 0.001;
/// Default factor of domain width in n-ary branching.
const BRANCHRULE_NARYWIDTHFAC_DEFAULT: f64 = 2.0;
/// Initial random seed.
const BRANCHRULE_RANDSEED_DEFAULT: u32 = 47;
/// Default discount factor for discounted pseudo costs.
const BRANCHRULE_DISCOUNTFACTOR: f64 = 0.2;

/// `SCIP_BranchruleData`: branching rule data.
struct BranchruleData {
    /// Random number generator.
    randnumgen: RandNumGen,
    /// Strategy for computing score of external candidates.
    strategy: u8,
    /// Weight for minimum of scores of a branching candidate.
    scoreminweight: f64,
    /// Weight for maximum of scores of a branching candidate.
    scoremaxweight: f64,
    /// Weight for sum of scores of a branching candidate.
    scoresumweight: f64,
    /// Strategy used to update pseudo costs of continuous variables.
    updatestrategy: u8,
    /// Targeted number of children in n-ary branching.
    nchildren: i32,
    /// Maximal depth where to do n-ary branching, -1 to turn off.
    narymaxdepth: i32,
    /// Minimal domain width in children when doing n-ary branching, relative
    /// to global bounds.
    naryminwidth: f64,
    /// Factor of domain width in n-ary branching.
    narywidthfactor: f64,
    /// Discount factor for discounted pseudo costs.
    discountfactor: f64,
}

fn bd(scip: &Scip, branchrule: BranchruleId) -> &BranchruleData {
    scip.branchrules[branchrule].branchruledata.as_ref().unwrap().downcast_ref::<BranchruleData>().unwrap()
}

fn bdm(scip: &mut Scip, branchrule: BranchruleId) -> &mut BranchruleData {
    scip.branchrules[branchrule].branchruledata.as_mut().unwrap().downcast_mut::<BranchruleData>().unwrap()
}

/// `WEIGHTEDSCORING`.
fn weighted_scoring(d: &BranchruleData, min: f64, max: f64, sum: f64) -> f64 {
    d.scoreminweight * min + d.scoremaxweight * max + d.scoresumweight * sum
}

/*
 * Local methods
 */

/// The best candidate found so far.
struct Best {
    var: VarId,
    brpoint: f64,
    score: f64,
    rndscore: f64,
}

/// `updateBestCandidate`: checks if a given branching candidate is better
/// than a previous one and updates the best branching candidate accordingly.
#[allow(clippy::too_many_arguments)]
fn update_best_candidate(
    scip: &mut Scip,
    branchrule: BranchruleId,
    best: &mut Best,
    mut cand: VarId,
    candscoremin: f64,
    candscoremax: f64,
    candscoresum: f64,
    candrndscore: f64,
    candsol: f64,
) -> ScipResult {
    // A branching variable candidate should either be an active problem
    // variable or a multi-aggregated variable.
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
            let multscalars = scip.vars[cand].data.multaggr.scalars.0[..nmultvars].to_vec();

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
                    ((candsol - maxact) / multscalars[i] + multvarub, (candsol - minact) / multscalars[i] + multvarlb)
                } else {
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
                } else if scip.set.is_feas_ge(aggrvarsol2, multvarub) || aggrvarsol1.abs() < aggrvarsol2.abs() {
                    aggrvarsol1
                } else {
                    aggrvarsol2
                };
                success = true;

                update_best_candidate(scip, branchrule, best, multvars[i], candscoremin, candscoremax, candscoresum, candrndscore, aggrvarsol)?;
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

                update_best_candidate(scip, branchrule, best, multvar, candscoremin, candscoremax, candscoresum, candrndscore, INVALID)?;
            }
        }

        return Ok(());
    }

    // Select branching point for this variable.
    let candbrpoint = api::get_branching_point(scip, cand, candsol);

    // We cannot branch on a huge value for a discrete variable, because we
    // simply cannot enumerate such huge integer values in floating point
    // arithmetics.
    let integral = crate::var::var_is_integral(scip, cand);
    if integral && (scip.set.is_huge_value(candbrpoint) || scip.set.is_huge_value(-candbrpoint)) {
        return Ok(());
    }

    let d = bd(scip, branchrule);
    let strategy = if !integral {
        if d.strategy == b'u' { d.updatestrategy } else { d.strategy }
    } else if d.strategy == b'u' {
        b'l'
    } else {
        d.strategy
    };

    let set = &scip.set;
    let (lb, ub) = (scip.vars[cand].locdom.lb, scip.vars[cand].locdom.ub);
    let weighted = |d: &BranchruleData| {
        if set.is_infinity(candscoremax) {
            set.infinity()
        } else {
            weighted_scoring(d, candscoremin, candscoremax, candscoresum)
        }
    };
    let deltaminus;
    let deltaplus;
    match strategy {
        b'l' => {
            let solval = api::get_sol_val(scip, NONE, cand);
            deltaminus = if set.is_infinity(solval) || solval <= adjusted_var_ub(scip, cand, candbrpoint) {
                0.0
            } else {
                solval - adjusted_var_ub(scip, cand, candbrpoint)
            };
            deltaplus = if set.is_infinity(-solval) || solval >= adjusted_var_lb(scip, cand, candbrpoint) {
                0.0
            } else {
                adjusted_var_lb(scip, cand, candbrpoint) - solval
            };
        }
        b'd' => {
            deltaminus = if set.is_infinity(-lb) {
                weighted(d)
            } else {
                adjusted_var_ub(scip, cand, candbrpoint) - lb
            };
            deltaplus = if set.is_infinity(ub) {
                weighted(d)
            } else {
                ub - adjusted_var_lb(scip, cand, candbrpoint)
            };
        }
        b's' => {
            deltaplus = if set.is_infinity(-lb) {
                weighted(d)
            } else {
                adjusted_var_ub(scip, cand, candbrpoint) - lb
            };
            deltaminus = if set.is_infinity(ub) {
                weighted(d)
            } else {
                ub - adjusted_var_lb(scip, cand, candbrpoint)
            };
        }
        b'v' => {
            deltaplus = weighted(d);
            deltaminus = deltaplus;
        }
        _ => {
            log::error!("branching strategy {} unknown", strategy as char);
            return Err(Retcode::InvalidData);
        }
    }

    let mut branchscore;
    if set.is_infinity(deltaminus) || set.is_infinity(deltaplus) {
        branchscore = set.infinity();
    } else {
        let pscostdown = crate::var::var_get_pseudocost(scip, cand, -deltaminus);
        let pscostup = crate::var::var_get_pseudocost(scip, cand, deltaplus);
        branchscore = crate::branch::branch_get_score(scip, cand, pscostdown, pscostup);
    }

    if set.is_infinity(branchscore) {
        branchscore = 0.9 * set.infinity();
    }

    let take = |best: &mut Best| {
        best.score = branchscore;
        best.rndscore = candrndscore;
        best.var = cand;
        best.brpoint = candbrpoint;
    };

    if set.is_sum_gt(branchscore, best.score) {
        take(best);
        return Ok(());
    }

    // If score of candidate is worse than bestscore, stay with best candidate.
    if !set.is_sum_eq(branchscore, best.score) {
        return Ok(());
    }

    let (blb, bub) = (scip.vars[best.var].locdom.lb, scip.vars[best.var].locdom.ub);
    if set.is_infinity(-blb) && set.is_infinity(bub) {
        // Best candidate is unbounded -> we prefer to branch on it.
        if set.is_infinity(-lb) && set.is_infinity(ub) && bdm(scip, branchrule).randnumgen.get_real(0.0, 1.0) <= 0.5 {
            // But if the new candidate is also unbounded (thus as good as best
            // candidate), then switch to the candidate with 50% probability to
            // reduce performance variability.
            take(best);
        }
        return Ok(());
    }

    // Best candidate has a finite lower or upper bound -> consider taking the
    // other candidate.
    if (set.is_infinity(-lb) || set.is_infinity(ub)) && (set.is_infinity(-blb) || set.is_infinity(bub)) {
        // Both candidates are unbounded, but one side may be finite (for
        // bestcand we know there is one); take the candidate with the larger
        // bound on the bounded side (hope that this avoids branching on always
        // the same variable); this will also prefer unbounded variables over
        // bounded ones.
        if ub > bub || lb < blb {
            // Cand is better than bestvar.
            take(best);
            return Ok(());
        }

        if bub > ub || blb < lb {
            // Bestvar is better than cand.
            return Ok(());
        }

        // Both are equally good.
    }

    // @TODO: handle implied integrality like in relpscost.
    let vtype = |v: VarId| {
        if crate::var::var_is_implied_integral(scip, v) {
            VarType::ImplInt as i32
        } else {
            scip.vars[v].vartype as i32
        }
    };
    let besttype = vtype(best.var);
    let candtype = vtype(cand);

    if besttype == candtype {
        // If both have the same type, take the one with larger relative diameter.
        if crate::misc::rel_diff(bub, blb) < crate::misc::rel_diff(ub, lb) {
            // Cand has larger diameter than bestvar.
            take(best);
            return Ok(());
        }

        if crate::misc::rel_diff(bub, blb) > crate::misc::rel_diff(ub, lb) {
            // Bestvar has larger diameter than cand.
            return Ok(());
        }
    }

    // Take the one with better type ("more discrete").
    if besttype > candtype {
        // Cand is more discrete than bestvar.
        take(best);
        return Ok(());
    }

    if besttype < candtype {
        // Bestvar is more discrete than cand.
        return Ok(());
    }

    // Cand seems to be as good as the currently best one (bestvar); use the
    // random score as a final tie-breaker.
    if candrndscore >= best.rndscore {
        take(best);
    }

    Ok(())
}

/// `selectBranchVar`: selects the branching variable from given candidate
/// array; returns the variable (`NONE` if none) and its branching point.
fn select_branch_var(scip: &mut Scip, branchrule: BranchruleId, cands: &[VarId], candssol: &[f64], candsscore: &[f64]) -> ScipResult<(VarId, f64)> {
    let ncands = cands.len();
    let mut best = Best {
        var: NONE,
        brpoint: INVALID,
        score: -1.0,
        rndscore: -1.0,
    };

    if ncands == 0 {
        return Ok((NONE, INVALID));
    }

    // Sort branching candidates (in a copy), such that same variables are on
    // consecutive positions.
    let mut candssorted = cands.to_vec();
    let mut candsorigidx: Vec<i32> = (0..ncands as i32).collect();
    {
        let s: &Scip = scip;
        crate::sort::sort(&mut candssorted, (&mut candsorigidx[..],), |&a, &b| crate::var::var_comp(s, a, b));
    }

    let mut i = 0;
    while i < ncands {
        let cand = candssorted[i];

        // Compute min, sum, and max of all registered scores for this variables;
        // set candsol to a valid value, if someone registered one.
        let mut scoremin = candsscore[candsorigidx[i] as usize];
        let mut scoresum = scoremin;
        let mut scoremax = scoremin;
        let mut candsol = candssol[candsorigidx[i] as usize];
        let mut j = i + 1;
        while j < ncands && crate::var::var_compare(scip, candssorted[j], cand) == 0 {
            let s = candsscore[candsorigidx[j] as usize];
            scoresum += s;
            if s < scoremin {
                scoremin = s;
            } else if s > scoremax {
                scoremax = s;
            }

            // @todo if there are two valid externcandssol available for the same
            // variable, should we take the one closer to the middle of the domain?
            if scip.set.is_infinity(candsol.abs()) {
                candsol = candssol[candsorigidx[j] as usize];
            }
            j += 1;
        }

        // Set i to last occurrence of cand in candssorted (instead of first one as
        // before), so in next round we look at another variable.
        i = j - 1;

        // Check if new candidate is better than previous candidate (if any).
        let rnd = bdm(scip, branchrule).randnumgen.get_real(0.0, 1.0);
        update_best_candidate(scip, branchrule, &mut best, cand, scoremin, scoremax, scoresum, rnd, candsol)?;

        i += 1;
    }

    // There were candidates, but no variable was selected; this can only
    // happen if the branching points are huge values for all non-continuous
    // variables on which we cannot branch.
    // @todo delay the node?
    if best.var == NONE {
        log::error!("no branching could be created: all external candidates have huge bounds");
        return Err(Retcode::BranchError);
    }

    Ok((best.var, best.brpoint))
}

/*
 * Callback methods
 */

/// `branchCopyPscost`: copy method for branchrule plugins.
fn branch_copy_pscost(scip: &mut Scip, _sourcescip: &Scip, _branchrule: BranchruleId) -> ScipResult {
    include_branchrule_pscost(scip)
}

/// `branchFreePscost`: destructor of branching rule to free user data.
fn branch_free_pscost(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    scip.branchrules[branchrule].branchruledata = None;
    Ok(())
}

/// `branchInitPscost`: initialization method of branching rule.
fn branch_init_pscost(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    // Initialize branching rule data.
    let seed = scip.set.initialize_random_seed(BRANCHRULE_RANDSEED_DEFAULT);
    bdm(scip, branchrule).randnumgen.set_seed(seed);
    Ok(())
}

/// `branchExeclpPscost`: branching execution method for fractional LP
/// solutions.
fn branch_execlp_pscost(scip: &mut Scip, branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    let discountfactor = bd(scip, branchrule).discountfactor;

    // Get branching candidates.
    let (lpcands, lpcandssol, _, nlpcands, _, _) = api::get_lp_branch_cands(scip)?;

    let mut bestcand: i32 = -1;
    let mut bestscore = -scip.set.infinity();
    let mut bestrootdiff = 0.0;
    for c in 0..nlpcands as usize {
        let score = crate::scip_var::get_var_dpseudocost_score(scip, lpcands[c], lpcandssol[c], discountfactor);
        let rootsolval = crate::var::var_get_root_sol(scip, lpcands[c]);
        let rootdiff = (lpcandssol[c] - rootsolval).abs();
        if scip.set.is_sum_gt(score, bestscore) || (scip.set.is_sum_eq(score, bestscore) && rootdiff > bestrootdiff) {
            bestcand = c as i32;
            bestscore = score;
            bestrootdiff = rootdiff;
        }
    }

    // Perform the branching.
    api::branch_var(scip, lpcands[bestcand as usize])?;
    *result = ScipResultCode::Branched;

    Ok(())
}

/// `branchExecextPscost`: branching execution method for external
/// candidates.
/// @todo extend discounted pseudocosts for non-linear problems
fn branch_execext_pscost(scip: &mut Scip, branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    // Get branching candidates (the ones of maximal priority).
    let (externcands, externcandssol, externcandsscore) = api::get_extern_branch_cands(scip);
    let nprioexterncands = scip.branchcand.nprioexterncands as usize;

    // Get current update strategy for pseudo costs, if our multiplier rule is
    // 'u'.
    if bd(scip, branchrule).strategy == b'u' {
        let u = crate::paramset::paramset_get_char(scip, "branching/lpgainnormalize")?;
        bdm(scip, branchrule).updatestrategy = u;
    }

    // Select branching variable.
    let (brvar, brpoint) = select_branch_var(
        scip,
        branchrule,
        &externcands[..nprioexterncands],
        &externcandssol[..nprioexterncands],
        &externcandsscore[..nprioexterncands],
    )?;

    if brvar == NONE {
        // Can happen if all candidates were non-continous variables with huge
        // bounds.
        *result = ScipResultCode::DidNotFind;
        return Ok(());
    }

    let (nchildrenparam, narymaxdepth) = (bd(scip, branchrule).nchildren, bd(scip, branchrule).narymaxdepth);
    let depth = scip.nodes[crate::tree::tree_get_current_node(scip)].depth as i32;
    if nchildrenparam > 2 && depth <= narymaxdepth {
        // Do n-ary branching.
        unimplemented!("branch_pscost n-ary branching (branching/pscost/narymaxdepth, default -1)");
    }

    // Do binary branching (SCIPbranchVarValNary with n = 2 delegates to
    // SCIPtreeBranchVar).
    let c = api::branch_var_val(scip, brvar, brpoint)?;
    let nchildren = (c.downchild != NONE) as i32 + (c.eqchild != NONE) as i32 + (c.upchild != NONE) as i32;

    if nchildren > 1 {
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

/// `SCIPincludeBranchrulePscost`: creates the pseudo cost branching rule and
/// includes it in SCIP.
pub fn include_branchrule_pscost(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_char, paramset_add_int, paramset_add_real};

    // Create a random number generator.
    let seed = scip.set.initialize_random_seed(BRANCHRULE_RANDSEED_DEFAULT);

    // Create pscost branching rule data.
    let data = BranchruleData {
        randnumgen: RandNumGen::new(seed),
        strategy: 0,
        scoreminweight: 0.0,
        scoremaxweight: 0.0,
        scoresumweight: 0.0,
        updatestrategy: 0,
        nchildren: 0,
        narymaxdepth: 0,
        naryminwidth: 0.0,
        narywidthfactor: 0.0,
        discountfactor: 0.0,
    };

    let methods = crate::branch::BranchruleMethods {
        branchexeclp: Some(branch_execlp_pscost),
        branchexecext: Some(branch_execext_pscost),
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
        Some(Box::new(data)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let b = &mut scip.branchrules[branchrule];
    b.branchcopy = Some(branch_copy_pscost);
    b.branchfree = Some(branch_free_pscost);
    b.branchinit = Some(branch_init_pscost);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, branchrules, branchrule, branchruledata, BranchruleData, $f)
        };
    }
    let infinity = scip.set.infinity();
    paramset_add_char(
        scip,
        "branching/pscost/strategy",
        "strategy for utilizing pseudo-costs of external branching candidates (multiply as in pseudo costs 'u'pdate rule, or by 'd'omain reduction, or by domain reduction of 's'ibling, or by 'v'ariable score)",
        t!(u8, strategy),
        false,
        BRANCHRULE_STRATEGY_DEFAULT,
        Some(BRANCHRULE_STRATEGIES),
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/pscost/minscoreweight",
        "weight for minimum of scores of a branching candidate when building weighted sum of min/max/sum of scores",
        t!(f64, scoreminweight),
        true,
        BRANCHRULE_SCOREMINWEIGHT_DEFAULT,
        -infinity,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/pscost/maxscoreweight",
        "weight for maximum of scores of a branching candidate when building weighted sum of min/max/sum of scores",
        t!(f64, scoremaxweight),
        true,
        BRANCHRULE_SCOREMAXWEIGHT_DEFAULT,
        -infinity,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/pscost/sumscoreweight",
        "weight for sum of scores of a branching candidate when building weighted sum of min/max/sum of scores",
        t!(f64, scoresumweight),
        true,
        BRANCHRULE_SCORESUMWEIGHT_DEFAULT,
        -infinity,
        infinity,
        None,
    )?;
    paramset_add_int(
        scip,
        "branching/pscost/nchildren",
        "number of children to create in n-ary branching",
        t!(i32, nchildren),
        false,
        BRANCHRULE_NCHILDREN_DEFAULT,
        2,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "branching/pscost/narymaxdepth",
        "maximal depth where to do n-ary branching, -1 to turn off",
        t!(i32, narymaxdepth),
        false,
        BRANCHRULE_NARYMAXDEPTH_DEFAULT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/pscost/naryminwidth",
        "minimal domain width in children when doing n-ary branching, relative to global bounds",
        t!(f64, naryminwidth),
        false,
        BRANCHRULE_NARYMINWIDTH_DEFAULT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/pscost/narywidthfactor",
        "factor of domain width in n-ary branching when creating nodes with increasing distance from branching value",
        t!(f64, narywidthfactor),
        false,
        BRANCHRULE_NARYWIDTHFAC_DEFAULT,
        1.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/pscost/discountfactor",
        "discount factor for ancestral pseudo costs (0.0: disable discounted pseudo costs)",
        t!(f64, discountfactor),
        false,
        BRANCHRULE_DISCOUNTFACTOR,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
