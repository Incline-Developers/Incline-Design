//! Port of `branch_relpscost.c`: reliable pseudo costs branching rule.
//!
//! Not ported: the symmetry handling (`initOrbits`,
//! `filterSymmetricVariables`; `branching/relpscost/filtercandssym` is off
//! by default) and the Treemodel candidate selection (see
//! [`crate::treemodel`]).

use std::any::Any;

use crate::{
    api,
    def::{INVALID, max, min},
    misc::RandNumGen,
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_strongbranch as sb,
    treemodel::{Treemodel, treemodel_init, treemodel_is_enabled, treemodel_select_candidate},
    types::{BoundType, BoundchgType, BranchDir, BranchruleId, ConfidenceLevel, LpSolStat, ScipResultCode, VarId},
};

const BRANCHRULE_NAME: &str = "relpscost";
const BRANCHRULE_DESC: &str = "reliability branching on pseudo cost values";
const BRANCHRULE_PRIORITY: i32 = 10000;
const BRANCHRULE_MAXDEPTH: i32 = -1;
const BRANCHRULE_MAXBOUNDDIST: f64 = 1.0;

const DEFAULT_CONFLICTWEIGHT: f64 = 0.01;
const DEFAULT_CONFLENGTHWEIGHT: f64 = 0.0;
const DEFAULT_INFERENCEWEIGHT: f64 = 0.0001;
const DEFAULT_CUTOFFWEIGHT: f64 = 0.0001;
const DEFAULT_GMIAVGEFFWEIGHT: f64 = 0.0;
const DEFAULT_GMILASTEFFWEIGHT: f64 = 0.00001;
const DEFAULT_PSCOSTWEIGHT: f64 = 1.0;
const DEFAULT_NLSCOREWEIGHT: f64 = 0.1;
const DEFAULT_MINRELIABLE: f64 = 1.0;
const DEFAULT_MAXRELIABLE: f64 = 5.0;
const DEFAULT_SBITERQUOT: f64 = 0.5;
const DEFAULT_DYNAMICLOOKAHEADQUOT: f64 = 0.6;
const DEFAULT_SBITEROFS: i32 = 100000;
const DEFAULT_MAXLOOKAHEAD: i32 = 9;
const DEFAULT_INITCAND: i32 = 100;
const DEFAULT_INITITER: i32 = 0;
const DEFAULT_MAXBDCHGS: i32 = 5;
const DEFAULT_MAXPROPROUNDS: i32 = -2;
const DEFAULT_PROBINGBOUNDS: bool = true;
const DEFAULT_USERELERRORFORRELIABILITY: bool = false;
const DEFAULT_LOWERRORTOL: f64 = 0.05;
const DEFAULT_HIGHERRORTOL: f64 = 1.0;
const DEFAULT_USEHYPTESTFORRELIABILITY: bool = false;
const DEFAULT_USEDYNAMICCONFIDENCE: bool = false;
const DEFAULT_STORESEMIINITCOSTS: bool = false;
const DEFAULT_USESBLOCALINFO: bool = false;
const DEFAULT_CONFIDENCELEVEL: i32 = 2;
const DEFAULT_SKIPBADINITCANDS: bool = true;
const DEFAULT_STARTRANDSEED: i32 = 5;
const DEFAULT_DYNAMICLOOKAHEAD: bool = false;
const DEFAULT_MINSAMPLESIZE: i32 = 10;
const DEFAULT_DYNAMICLOOKDISTRIBUTION: i32 = 1;
const DEFAULT_RANDINITORDER: bool = false;
const DEFAULT_USESMALLWEIGHTSITLIM: bool = false;
const DEFAULT_DYNAMICWEIGHTS: bool = true;
const DEFAULT_DEGENERACYAWARE: i32 = 1;

// Symmetry handling.
const DEFAULT_FILTERCANDSSYM: bool = false;
const DEFAULT_TRANSSYMPSCOST: bool = false;

// Distribution to use for lookahead strong branching.
const EXPONENTIALDISTRIBUTION: i32 = 0;
const PARETODISTRIBUTION: i32 = 1;
const LOGNORMALDISTRIBUTION: i32 = 2;

/// Shift for geometric mean of left and right gains.
const GEOMMEANSHIFT: f64 = 0.01;

/// Maximum gain with which we update the estimated left and right dual
/// gains.
const MAXGAINTHRESHOLD: f64 = 1e15;

/// Minimum considered expected dual gain and probability for lookahead
/// strong branching.
const MINGAINTHRESHOLD: f64 = 1e-5;

/// Default discount factor for discounted pseudo costs.
const BRANCHRULE_DISCOUNTFACTOR: f64 = 0.2;

/// Branching rule data.
#[derive(Default)]
struct BranchruleData {
    conflictweight: f64,
    conflengthweight: f64,
    inferenceweight: f64,
    cutoffweight: f64,
    gmiavgeffweight: f64,
    gmilasteffweight: f64,
    pscostweight: f64,
    nlscoreweight: f64,
    minreliable: f64,
    maxreliable: f64,
    sbiterquot: f64,
    #[allow(dead_code)]
    meandualgain: f64,
    currmeandualgain: f64,
    maxmeangain: f64,
    minmeangain: f64,
    sumlogmeangains: f64,
    logstdevgain: f64,
    nzerogains: f64,
    currndualgains: i32,
    sbiterofs: i32,
    maxlookahead: i32,
    initcand: i32,
    inititer: i32,
    maxbdchgs: i32,
    maxproprounds: i32,
    probingbounds: bool,
    userelerrorforreliability: bool,
    lowerrortol: f64,
    higherrortol: f64,
    usehyptestforreliability: bool,
    usedynamicconfidence: bool,
    storesemiinitcosts: bool,
    usesblocalinfo: bool,
    skipbadinitcands: bool,
    dynamicweights: bool,
    dynamiclookaheadquot: f64,
    dynamiclookahead: bool,
    dynamiclookdistribution: i32,
    minsamplesize: i32,
    degeneracyaware: i32,
    confidencelevel: i32,
    /// Array to store nonlinear count values (C's `nlcount`, NULL when
    /// `None`).
    nlcount: Option<Vec<i32>>,
    nlcountsize: i32,
    nlcountmax: i32,
    randinitorder: bool,
    randnumgen: Option<RandNumGen>,
    startrandseed: i32,
    usesmallweightsitlim: bool,
    treemodel: Treemodel,
    filtercandssym: bool,
    transsympscost: bool,
    nosymmetry: bool,
    norbits: i32,
    npermvars: i32,
    discountfactor: f64,
}

fn data(scip: &Scip, branchrule: BranchruleId) -> &BranchruleData {
    scip.branchrules[branchrule].branchruledata.as_ref().unwrap().downcast_ref::<BranchruleData>().unwrap()
}

fn data_mut(scip: &mut Scip, branchrule: BranchruleId) -> &mut BranchruleData {
    scip.branchrules[branchrule].branchruledata.as_mut().unwrap().downcast_mut::<BranchruleData>().unwrap()
}

/*
 * local methods
 */

/// `SCIPupdateVarPseudocostSymmetric`: updates the pseudo costs of the
/// given variable and all its symmetric variables (the orbits are not
/// ported: `branchorbitidx` is always NULL).
#[allow(clippy::too_many_arguments)]
fn update_var_pseudocost_symmetric(
    scip: &mut Scip,
    branchrule: BranchruleId,
    branchvar: VarId,
    branchorbitidx: Option<&[i32]>,
    _branchvaridx: usize,
    solvaldelta: f64,
    objdelta: f64,
    weight: f64,
) -> ScipResult {
    // Update the discounted pseudo cost of the current node branched variable.
    let useancpscost = scip.set.branch_collectancpscost;
    if useancpscost {
        let currentnode = crate::tree::tree_get_focus_node(scip);
        if scip.nodes[currentnode].depth > 0 {
            let changes: Vec<(VarId, f64, f64)> = match scip.nodes[currentnode].domchg.as_ref() {
                Some(domchange) => domchange
                    .boundchgs
                    .iter()
                    .take(domchange.nboundchgs as usize)
                    .filter(|bc| bc.boundchgtype == BoundchgType::Branching)
                    .map(|bc| (bc.var, bc.newbound, bc.data.branchingdata.lpsolval))
                    .collect(),
                None => Vec::new(),
            };
            for (var, newbound, parentlpsolval) in changes {
                if crate::var::var_is_integral(scip, var) {
                    if parentlpsolval >= INVALID {
                        continue;
                    }
                    let parentsolvedelta = newbound - parentlpsolval;
                    crate::scip_var::update_var_anc_pseudocost(scip, var, parentsolvedelta, objdelta, weight)?;
                }
            }
        }
    }

    let d = data(scip, branchrule);
    if d.nosymmetry || !d.transsympscost || branchorbitidx.is_none() {
        // Use original update function.
        crate::scip_var::update_var_pseudocost(scip, branchvar, solvaldelta, objdelta, weight)?;
        return Ok(());
    }

    // The orbits are not ported.
    Err(Retcode::NotImplemented)
}

/// `countNonlinearities`: counts number of nonlinear constraints in which
/// each variable appears.
fn count_nonlinearities(scip: &mut Scip, nlcount: &mut [i32]) -> ScipResult<i32> {
    let nvars = api::get_n_vars(scip) as usize;
    debug_assert!(nlcount.len() >= nvars);

    // Get nonlinearity for constraints in NLP.
    if api::is_nlp_constructed(scip) {
        let counts = crate::nlp::get_nlp_vars_nonlinearity(scip)?;
        debug_assert_eq!(counts.len(), nvars);
        nlcount[..counts.len()].copy_from_slice(&counts);
    } else {
        nlcount[..nvars].fill(0);
    }

    // Increase counters for and constraints.
    let andconshdlr = crate::set::set_find_conshdlr(scip, "and");
    if andconshdlr != crate::var::NONE {
        for c in 0..scip.conshdlrs[andconshdlr].nactiveconss as usize {
            // Get constraint and variables.
            let andcons = scip.conshdlrs[andconshdlr].conss[c];
            let andvars = crate::cons_and::get_vars_and(scip, andcons).to_vec();
            let andres = crate::cons_and::get_resultant_and(scip, andcons);

            // Get active index of resultant; the resultant might be deleted.
            let probindex = scip.vars[crate::var::var_get_probvar(scip, andres)].probindex;
            if probindex >= 0 {
                nlcount[probindex as usize] += 1;
            }

            for &v in &andvars {
                // Get active index of operator; the operator might be deleted.
                let probindex = scip.vars[crate::var::var_get_probvar(scip, v)].probindex;
                if probindex >= 0 {
                    nlcount[probindex as usize] += 1;
                }
            }
        }
    }

    // Compute maximum count value.
    let mut nlcountmax = 1;
    for &n in &nlcount[..nvars] {
        if nlcountmax < n {
            nlcountmax = n;
        }
    }
    Ok(nlcountmax)
}

/// `branchruledataEnsureNlcount`.
fn branchruledata_ensure_nlcount(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    let nvars = api::get_n_vars(scip);

    if data(scip, branchrule).nlscoreweight > 0.0 {
        if data(scip, branchrule).nlcount.is_none() {
            let mut nlcount = vec![0; nvars as usize];
            let nlcountmax = count_nonlinearities(scip, &mut nlcount)?;
            let d = data_mut(scip, branchrule);
            d.nlcount = Some(nlcount);
            d.nlcountsize = nvars;
            d.nlcountmax = nlcountmax;
        } else if data(scip, branchrule).nlcountsize < nvars {
            let d = data_mut(scip, branchrule);
            d.nlcount.as_mut().unwrap().resize(nvars as usize, 0);
            d.nlcountsize = nvars;
        }
    } else {
        let d = data_mut(scip, branchrule);
        d.nlcount = None;
        d.nlcountsize = 0;
        d.nlcountmax = 1;
    }
    Ok(())
}

/// `calcNlscore`: calculates nlscore value between 0 and 1.
fn calc_nlscore(nlcount: Option<&[i32]>, nlcountmax: i32, probindex: i32) -> f64 {
    match nlcount {
        Some(nlcount) if nlcountmax >= 1 => nlcount[probindex as usize] as f64 / nlcountmax as f64,
        _ => 0.0,
    }
}

/// The weights `calcScore` reads from the branching rule data.
#[derive(Clone, Copy)]
struct Weights {
    dynamicweights: bool,
    conflictweight: f64,
    conflengthweight: f64,
    inferenceweight: f64,
    cutoffweight: f64,
    gmiavgeffweight: f64,
    gmilasteffweight: f64,
    pscostweight: f64,
    nlscoreweight: f64,
}

/// `calcScore`: calculates an overall score value for the given individual
/// score values.
#[allow(clippy::too_many_arguments)]
fn calc_score(
    scip: &Scip,
    w: &Weights,
    conflictscore: f64,
    avgconflictscore: f64,
    conflengthscore: f64,
    avgconflengthscore: f64,
    inferencescore: f64,
    avginferencescore: f64,
    cutoffscore: f64,
    avgcutoffscore: f64,
    gmieffscore: f64,
    lastgmieffscore: f64,
    pscostscore: f64,
    avgpscostscore: f64,
    nlscore: f64,
    frac: f64,
    degeneracyfactor: f64,
) -> f64 {
    let mut dynamicfactor = if w.dynamicweights {
        (scip.stat.ninfeasleaves as f64 + 1.0) / (scip.stat.nobjleaves as f64 + 1.0)
    } else {
        1.0
    };

    dynamicfactor *= degeneracyfactor;

    let mut score = dynamicfactor
        * (w.conflictweight * (1.0 - 1.0 / (1.0 + conflictscore / avgconflictscore))
            + w.conflengthweight * (1.0 - 1.0 / (1.0 + conflengthscore / avgconflengthscore))
            + w.inferenceweight * (1.0 - 1.0 / (1.0 + inferencescore / avginferencescore))
            + w.cutoffweight * (1.0 - 1.0 / (1.0 + cutoffscore / avgcutoffscore))
            + w.gmiavgeffweight * gmieffscore
            + w.gmilasteffweight * lastgmieffscore)
        + w.pscostweight / dynamicfactor * (1.0 - 1.0 / (1.0 + pscostscore / avgpscostscore))
        + w.nlscoreweight * nlscore;

    // Avoid close to integral variables.
    if min(frac, 1.0 - frac) < 10.0 * scip.set.feastol() {
        score *= 1e-6;
    }

    score
}

/// A bound change stored by `addBdchg`.
struct Bdchg {
    ind: usize,
    boundtype: BoundType,
    bound: f64,
}

/// `applyBdchgs`: applies bound changes stored in bound change arrays.
fn apply_bdchgs(scip: &mut Scip, vars: &[VarId], bdchgs: &[Bdchg], result: &mut ScipResultCode) -> ScipResult {
    for b in bdchgs {
        let v = b.ind;

        if b.boundtype == BoundType::Lower {
            // Change lower bound of variable to given bound.
            let (infeasible, _) = crate::scip_var::tighten_var_lb(scip, vars[v], b.bound, true)?;
            if infeasible {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }
        } else {
            // Change upper bound of variable to given bound.
            let (infeasible, _) = crate::scip_var::tighten_var_ub(scip, vars[v], b.bound, true)?;
            if infeasible {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }
        }
    }

    Ok(())
}

/// `updateMinMaxMeanGain`: update the min/max gain, and the mean of all
/// gains computed so far. This mean is used in the definition of the
/// exponential distribution.
fn update_min_max_mean_gain(scip: &mut Scip, branchrule: BranchruleId, downgain: f64, upgain: f64) {
    // In the case of very large gains, SCIP will already prioritize this
    // variable.
    if scip.set.is_rel_ge(downgain, MAXGAINTHRESHOLD) || scip.set.is_rel_ge(upgain, MAXGAINTHRESHOLD) {
        return;
    }

    let d = data_mut(scip, branchrule);

    // Initializing to avoid linter, value never used.
    let mut oldlogstdevgain = -1.0;
    let mut oldlogmeangain = -1.0;

    let meangain = ((downgain + GEOMMEANSHIFT) * (upgain + GEOMMEANSHIFT)).sqrt() - GEOMMEANSHIFT;

    if meangain < MINGAINTHRESHOLD {
        d.nzerogains += 1.0;
        return;
    }

    d.currmeandualgain = (d.currndualgains as f64 / (d.currndualgains + 1) as f64) * d.currmeandualgain + meangain / (d.currndualgains + 1) as f64;
    d.currndualgains += 1;

    if d.currndualgains > 1 {
        oldlogstdevgain = d.logstdevgain;
        oldlogmeangain = d.sumlogmeangains / (d.currndualgains - 1) as f64;
    }

    d.sumlogmeangains += meangain.ln();

    // Update the max mean gain.
    let logmeangain = d.sumlogmeangains / d.currndualgains as f64;
    let ngains = d.currndualgains;
    if ngains == 1 {
        d.logstdevgain = (meangain.ln() - logmeangain).powf(2.0);
    } else {
        d.logstdevgain = ((ngains - 2) as f64 * oldlogstdevgain + (ngains - 1) as f64 * (oldlogmeangain - logmeangain).powf(2.0) + (meangain.ln() - logmeangain).powf(2.0))
            / (ngains - 1) as f64;
    }

    d.maxmeangain = max(d.maxmeangain, meangain);
    d.minmeangain = min(d.minmeangain, meangain);
}

/// `strongBranchingDepth`: compute the depth of the tree with the assumption
/// that left and right dual gains are equal.
fn strong_branching_depth(gap: f64, maxmeangain: f64) -> f64 {
    // Using an epsilon value if maxmeangain is zero.
    (gap / max(maxmeangain, MINGAINTHRESHOLD)).ceil()
}

/// `strongBranchingTreeSize`: compute the size of the tree with the
/// assumption that left and right dual gains are equal.
fn strong_branching_tree_size(estimatedepth: f64) -> f64 {
    2.0f64.powf(estimatedepth + 1.0) - 1.0
}

/// `cdfProbability`: calculate the cumulative distribution function (CDF)
/// value for a mixture of a Dirac at zero and a continuous distribution
/// (depending on distributioncdf).
fn cdf_probability(rate: f64, zeroprob: f64, proposedgain: f64, mingain: f64, logmeangain: f64, logstdevgain: f64, distributioncdf: i32) -> f64 {
    if distributioncdf == PARETODISTRIBUTION {
        if proposedgain < mingain {
            0.0
        } else {
            zeroprob + (1.0 - zeroprob) * (1.0 - (mingain / proposedgain).powf(rate))
        }
    } else if distributioncdf == EXPONENTIALDISTRIBUTION {
        zeroprob + (1.0 - zeroprob) * (1.0 - (-rate * proposedgain).exp())
    } else {
        zeroprob + (1.0 - zeroprob) * 0.5 * libm_erfc(-(proposedgain.ln() - logmeangain) / (logstdevgain * 2.0f64.sqrt()))
    }
}

/// C's `erfc` (only reached with the log-normal dynamic lookahead
/// distribution, which needs `dynamiclookahead`, off by default).
fn libm_erfc(_x: f64) -> f64 {
    unimplemented!("erfc: the log-normal dynamic lookahead distribution is not ported")
}

/// `expectedTreeSize`: calculate the expected size of a tree with one more
/// iteration of strong branching.
#[allow(clippy::too_many_arguments)]
fn expected_tree_size(scip: &Scip, gap: f64, zeroprob: f64, currentdepth: f64, lambda: f64, mingain: f64, logmeangain: f64, logstdevgain: f64, distributioncdf: i32) -> f64 {
    let mut ptotal = 0.0;
    let mut totalimprovedtree = 0.0;
    let mut depth = currentdepth as i32;
    let currenttreesize = strong_branching_tree_size(currentdepth);
    let nexttreesize;
    let cdf = |g: f64| cdf_probability(lambda, zeroprob, g, mingain, logmeangain, logstdevgain, distributioncdf);

    if depth == 1 {
        nexttreesize = currenttreesize;
    }
    // Compute the expected size with one more strong branching iteration.
    else if depth == 2 {
        // Probability of finding a better variable that would reduce the depth
        // (smaller tree size).
        let p = 1.0 - cdf(gap);

        // Size of the improved tree.
        let improvedtree = strong_branching_tree_size(currentdepth - 1.0);
        nexttreesize = improvedtree * p + strong_branching_tree_size(currentdepth) * (1.0 - p) + 2.0;

        if p < MINGAINTHRESHOLD {
            return scip.set.infinity();
        }
    } else {
        // Probability of not finding a better variable that would reduce the
        // depth of the tree (size of the tree).
        let pnotbetter = cdf(gap / (depth - 1) as f64);
        // Probability of finding a better variable that would reduce the depth
        // (smaller tree size).
        let mut pbetter = 1.0 - cdf(gap / (depth - 1) as f64);

        if pbetter < MINGAINTHRESHOLD {
            return scip.set.infinity();
        }

        let mut p = 0.0;
        while pbetter >= MINGAINTHRESHOLD && ptotal + pnotbetter < 1.0 {
            if depth > 2 {
                p = cdf(gap / (depth - 2) as f64) - cdf(gap / (depth - 1) as f64);
                ptotal += p;
                pbetter = 1.0 - cdf(gap / (depth - 2) as f64);
            } else if depth == 2 {
                p = 1.0 - cdf(gap);
                ptotal += p;
                pbetter = 0.0;
            }

            let improvedtree = if ptotal + pnotbetter <= 1.0 {
                // Compute expected size of the improved tree based on improved depth,
                // considering the probability of this improvement.
                strong_branching_tree_size(currentdepth - 1.0) * p
            } else {
                0.0
            };

            depth -= 1;
            totalimprovedtree += improvedtree;
        }

        // Compute the expectation of the next tree size with one more iteration
        // of strong branching.
        nexttreesize = totalimprovedtree + pnotbetter * strong_branching_tree_size(currentdepth) + 2.0;
    }

    nexttreesize
}

/// `continueStrongBranchingLookahead`: decide if we continue strong
/// branching based based on lookahead.
#[allow(clippy::too_many_arguments)]
fn continue_strong_branching_lookahead(
    scip: &Scip,
    candidx: usize,
    ninitcands: usize,
    lookahead: f64,
    maxlookahead: f64,
    nbdchgs: i32,
    nbdconflicts: i32,
    maxbdchgs: i32,
    maxnsblpiterations: i64,
) -> bool {
    candidx < ninitcands
        && lookahead < maxlookahead
        && nbdchgs.saturating_add(nbdconflicts) < maxbdchgs
        && ((candidx as i32) < maxlookahead as i32 || api::get_n_strongbranch_lp_iterations(scip) < maxnsblpiterations)
}

/// `continueStrongBranchingTreeSizeEstimation`: decide if we continue
/// strong branching based on the estimation of the tree size given the
/// current gains.
fn continue_strong_branching_tree_size_estimation(scip: &Scip, branchrule: BranchruleId, lookahead: f64, maxlookahead: f64) -> bool {
    let d = data(scip, branchrule);

    // Default values never used.
    let mut logmeangain = 0.0;
    let mut logstdevgain = -1.0;

    if !d.dynamiclookahead {
        return true;
    }

    // Compute the expected tree size after reaching a min lookahaead.
    if lookahead < d.dynamiclookaheadquot * maxlookahead {
        return true;
    }

    // If we do not have a large enough sample to estimate the tree size we
    // continue with strong branching.
    if d.currndualgains < d.minsamplesize {
        return true;
    }

    let maxmeangain = d.maxmeangain;
    let minmeangain = d.minmeangain;

    // Compute the absolute gap at the current node. If no upper bound available
    // we continue strong branching as usual.
    let absdualgap = if !scip.set.is_infinity(api::get_upperbound(scip)) {
        api::get_upperbound(scip) - scip.nodes[crate::tree::tree_get_current_node(scip)].lowerbound
    } else {
        scip.set.infinity()
    };

    let gaptoclose = if !scip.set.is_infinity(absdualgap) && !scip.set.is_infinity(maxmeangain) && scip.set.is_gt(maxmeangain, 0.0) {
        absdualgap
    } else {
        return true;
    };

    let zeroprob = d.nzerogains / (d.nzerogains + d.currndualgains as f64);

    let lambda;
    if d.dynamiclookdistribution == PARETODISTRIBUTION {
        let sumlambda = d.sumlogmeangains - d.currndualgains as f64 * minmeangain.ln();
        if scip.set.is_zero(sumlambda) {
            return true;
        }
        lambda = d.currndualgains as f64 / sumlambda;
    } else if d.dynamiclookdistribution == EXPONENTIALDISTRIBUTION {
        lambda = 1.0 / d.currmeandualgain;
    } else {
        logmeangain = d.sumlogmeangains / d.currndualgains as f64;
        logstdevgain = d.logstdevgain;
        // Lambda value not used.
        lambda = -1.0;
    }

    // Continue strong branching since we do not have good enough gains in this
    // case.
    let currentdepth = strong_branching_depth(gaptoclose, maxmeangain);
    if currentdepth >= 50.0 {
        return true;
    }

    // Compute the tree size if we branch on the best variable so far, including
    // the strong branching already done.
    let currenttreesize = strong_branching_tree_size(currentdepth);

    // Compute the expected size of the tree with one more strong branching.
    let nexttreesize = expected_tree_size(
        scip,
        gaptoclose,
        zeroprob,
        currentdepth,
        lambda,
        minmeangain,
        logmeangain,
        logstdevgain,
        d.dynamiclookdistribution,
    );

    if scip.set.is_le(nexttreesize, currenttreesize - 1.0) {
        return true;
    }

    false
}

/// `needsStrongBranching`: determine if strong branching is needed on the
/// given candidate variable.
#[allow(clippy::too_many_arguments)]
fn needs_strong_branching(
    scip: &Scip,
    branchrule: BranchruleId,
    branchcand: VarId,
    branchcandfrac: f64,
    bestpscand: VarId,
    bestpscandfrac: f64,
    reliable: f64,
    relerrorthreshold: f64,
    clevel: ConfidenceLevel,
    useancpscost: bool,
) -> bool {
    use crate::var::{var_get_anc_pseudocost_count_current_run, var_get_pseudocost_count_current_run, var_is_pscost_relerror_reliable, var_significant_pscost_difference};

    let d = data(scip, branchrule);

    // Check, if the pseudo cost score and the discounted pseudocost score of the
    // variable is reliable.
    let pscostdownsize = var_get_pseudocost_count_current_run(scip, branchcand, BranchDir::Downwards);
    let pscostupsize = var_get_pseudocost_count_current_run(scip, branchcand, BranchDir::Upwards);
    let pscostsize = min(pscostdownsize, pscostupsize);

    let dpscostdownsize = var_get_anc_pseudocost_count_current_run(scip, branchcand, BranchDir::Downwards);
    let dpscostupsize = var_get_anc_pseudocost_count_current_run(scip, branchcand, BranchDir::Upwards);
    let dpscostsize = min(dpscostdownsize, dpscostupsize);

    // Determine if variable is considered reliable based on the current
    // reliability setting; check fixed number threshold (aka original)
    // reliability first.
    if pscostsize < reliable || (useancpscost && dpscostsize < reliable) {
        return true;
    }

    let significant = |dir: BranchDir| {
        if dir == BranchDir::Downwards {
            var_significant_pscost_difference(scip, bestpscand, bestpscandfrac, branchcand, branchcandfrac, BranchDir::Downwards, clevel, true)
        } else {
            var_significant_pscost_difference(scip, bestpscand, 1.0 - bestpscandfrac, branchcand, 1.0 - branchcandfrac, BranchDir::Upwards, clevel, true)
        }
    };

    if d.userelerrorforreliability && d.usehyptestforreliability {
        if !var_is_pscost_relerror_reliable(scip, branchcand, relerrorthreshold, clevel) && !significant(BranchDir::Downwards) && !significant(BranchDir::Upwards) {
            return true;
        }
    }

    // Check if relative error is tolerable.
    if d.userelerrorforreliability && !var_is_pscost_relerror_reliable(scip, branchcand, relerrorthreshold, clevel) {
        return true;
    }

    // Check if best pseudo-candidate is significantly better in both
    // directions, use strong-branching otherwise.
    if d.usehyptestforreliability && !significant(BranchDir::Downwards) && !significant(BranchDir::Upwards) {
        return true;
    }

    false
}

/// The per-candidate scores the loops of `execRelpscost` read.
struct CandScores {
    conflictscore: f64,
    conflengthscore: f64,
    inferencescore: f64,
    cutoffscore: f64,
    gmieffscore: f64,
    lastgmieffscore: f64,
    nlscore: f64,
}

fn cand_scores(scip: &Scip, branchrule: BranchruleId, var: VarId) -> CandScores {
    use crate::scip_var as sv;
    let d = data(scip, branchrule);
    CandScores {
        conflictscore: sv::get_var_conflict_score(scip, var),
        conflengthscore: sv::get_var_conflictlength_score(scip, var),
        inferencescore: sv::get_var_avg_inference_score(scip, var),
        cutoffscore: sv::get_var_avg_cutoff_score(scip, var),
        gmieffscore: crate::var::var_get_avg_gmiscore(scip, var),
        lastgmieffscore: crate::var::var_get_last_gmiscore(scip, var),
        nlscore: calc_nlscore(d.nlcount.as_deref(), d.nlcountmax, scip.vars[var].probindex),
    }
}

/// `execRelpscost`: execute reliability pseudo cost branching.
#[allow(clippy::too_many_arguments)]
fn exec_relpscost(
    scip: &mut Scip,
    branchrule: BranchruleId,
    branchcands: &[VarId],
    branchcandssol: &[f64],
    branchcandsfrac: &[f64],
    branchorbitidx: Option<&[i32]>,
    executebranch: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    use crate::scip_var as sv;

    let nbranchcands = branchcands.len();
    *result = ScipResultCode::DidNotRun;

    // Get current LP objective bound of the local sub problem and global cutoff
    // bound.
    let lpobjval = api::get_lp_objval(scip);

    // Check, if we want to solve the problem exactly, meaning that strong
    // branching information is not useful for cutting off sub problems and
    // improving lower bounds of children.
    let exactsolve = scip.set.exact_enable;

    // Check, if all existing columns are in LP, and thus the strong branching
    // results give lower bounds.
    let allcolsinlp = crate::prob::prob_all_cols_in_lp(scip, crate::prob::TRANSPROB);

    let mut bestcand: usize;
    let mut bestsbdown = INVALID;
    let mut bestsbup = INVALID;
    let mut bestsbdownvalid = false;
    let mut bestsbupvalid = false;
    let mut bestsbdowncutoff = false;
    let mut bestsbupcutoff = false;
    let mut provedbound = lpobjval;
    let mut ninitcands: usize = 0;

    // Allocate memory to store the lp bounds of the up and down children for
    // those of the variables that we performed sb on.
    let mut sbdown = vec![0.0; nbranchcands];
    let mut sbup = vec![0.0; nbranchcands];
    let mut sbdownvalid = vec![false; nbranchcands];
    let mut sbupvalid = vec![false; nbranchcands];

    if nbranchcands == 1 {
        // Only one candidate: nothing has to be done.
        bestcand = 0;
        sbdownvalid[0] = false;
        sbupvalid[0] = false;
    } else {
        let mut degeneracyfactor = 1.0;

        let mut useancpscost = scip.set.branch_collectancpscost;

        let d = data(scip, branchrule);
        let w = Weights {
            dynamicweights: d.dynamicweights,
            conflictweight: d.conflictweight,
            conflengthweight: d.conflengthweight,
            inferenceweight: d.inferenceweight,
            cutoffweight: d.cutoffweight,
            gmiavgeffweight: d.gmiavgeffweight,
            gmilasteffweight: d.gmilasteffweight,
            pscostweight: d.pscostweight,
            nlscoreweight: d.nlscoreweight,
        };
        let degeneracyaware = d.degeneracyaware;
        let discountfactor = d.discountfactor;

        // Get LP degeneracy information and compute a factor to change weighting of
        // pseudo cost score vs. other scores.
        if degeneracyaware > 0 && (api::get_depth(scip) > 0 || degeneracyaware > 1) {
            // Get LP degeneracy information.
            let (mut degeneracy, varconsratio) = crate::lp::lp_get_dual_degeneracy(scip)?;

            // Increase factor for a degeneracy >= 80%.
            if degeneracy >= 0.8 {
                degeneracy = 10.0 * (degeneracy - 0.7);
                degeneracyfactor *= 10.0f64.powf(degeneracy);
            }
            // Increase factor for a variable-constraint ratio >= 2.0.
            if varconsratio >= 2.0 {
                degeneracyfactor *= 10.0 * varconsratio;
            }
        }

        let vars = api::get_vars(scip).to_vec();
        let nvars = vars.len();

        let best_sol_index = |scip: &Scip| {
            let s = api::get_best_sol(scip);
            if s == crate::var::NONE { -1 } else { scip.sols[s].index }
        };
        let mut bestsolidx = best_sol_index(scip);

        // Get average conflict, inference, and pseudocost scores.
        let avgconflictscore = max(api::get_avg_conflict_score(scip), 0.1);
        let avgconflengthscore = max(api::get_avg_conflictlength_score(scip), 0.1);
        let avginferencescore = max(api::get_avg_inference_score(scip), 0.1);
        let avgcutoffscore = max(api::get_avg_cutoff_score(scip), 0.1);
        let mut avgpscostscore = max(api::get_avg_pseudocost_score(scip), 0.1);
        let avgdpscostscore = max(api::get_avg_dpseudocost_score(scip, discountfactor), 0.1);

        // Get nonlinear counts according to parameters.
        branchruledata_ensure_nlcount(scip, branchrule)?;

        let mut initstrongbranching = false;

        let d = data(scip, branchrule);

        // Check whether propagation should be performed.
        let propagate = d.maxproprounds != 0 && !scip.set.exact_enable;

        // Check whether valid bounds should be identified in probing-like fashion.
        let probingbounds = propagate && d.probingbounds;

        // Get maximal number of candidates to initialize with strong branching; if
        // the current solutions is not basic, we cannot warmstart the simplex
        // algorithm and therefore don't initialize any candidates.
        let mut maxninitcands = min(nbranchcands as i32, d.initcand);
        if !api::is_lp_sol_basic(scip) {
            maxninitcands = 0;
        }

        // Calculate maximal number of strong branching LP iterations; if we used
        // too many, don't apply strong branching any more; also, if degeneracy is
        // too high, don't run strong branching at this node.
        let nlpiterationsquot = (d.sbiterquot * api::get_n_node_lp_iterations(scip) as f64) as i64;
        let maxnsblpiterations = nlpiterationsquot + d.sbiterofs as i64 + scip.stat.nrootsblpiterations;
        let nsblpiterations = api::get_n_strongbranch_lp_iterations(scip);
        if nsblpiterations > maxnsblpiterations || degeneracyfactor >= 10.0 {
            maxninitcands = 0;
        }
        let maxninitcands = maxninitcands as usize;

        // Get buffer for storing the unreliable candidates (one additional slot
        // for convenience).
        let mut initcands = vec![0usize; maxninitcands + 1];
        let mut initcandscores = vec![0.0; maxninitcands + 1];
        ninitcands = 0;

        // Allocate memory for the down and up gains, and the computed pseudocost
        // scores.
        let mut mingains = vec![0.0; nbranchcands];
        let mut maxgains = vec![0.0; nbranchcands];
        let mut scores = vec![0.0; nbranchcands];
        let mut scoresfrompc = vec![0.0; nbranchcands];
        let mut scoresfromothers = vec![0.0; nbranchcands];

        // Get current node number.
        let nodenum = api::get_n_nodes(scip);

        // Initialize bound change arrays.
        let mut bdchgs: Vec<Bdchg> = Vec::new();
        let nbdconflicts = 0;
        let maxbdchgs = if d.maxbdchgs == -1 { i32::MAX } else { d.maxbdchgs };

        // Calculate value used as reliability.
        let mut prio = (maxnsblpiterations - nsblpiterations) as f64 / (nsblpiterations as f64 + 1.0);
        prio = min(prio, 1.0);
        prio = max(prio, (nlpiterationsquot - nsblpiterations) as f64 / (nsblpiterations as f64 + 1.0));
        let reliable = (1.0 - prio) * d.minreliable + prio * d.maxreliable;

        // Calculate the threshold for the relative error in the same way; low
        // tolerance is more strict than higher tolerance.
        let relerrorthreshold = (1.0 - prio) * d.higherrortol + prio * d.lowerrortol;

        let mut clevel = match d.confidencelevel {
            0 => ConfidenceLevel::Min,
            1 => ConfidenceLevel::Low,
            2 => ConfidenceLevel::Medium,
            3 => ConfidenceLevel::High,
            _ => ConfidenceLevel::Max,
        };
        // Determine the confidence level for hypothesis testing based on value of
        // prio.
        if d.usedynamicconfidence {
            // With decreasing priority, use a less strict confidence level.
            clevel = if prio >= 0.9 {
                ConfidenceLevel::Max
            } else if prio >= 0.7 {
                ConfidenceLevel::High
            } else if prio >= 0.5 {
                ConfidenceLevel::Medium
            } else if prio >= 0.3 {
                ConfidenceLevel::Low
            } else {
                ConfidenceLevel::Min
            };
        }

        let usehyptestforreliability = d.usehyptestforreliability;
        let randinitorder = d.randinitorder;
        let maxlookaheadparam = d.maxlookahead;
        let inititerparam = d.inititer;
        let skipbadinitcands = d.skipbadinitcands;
        let maxproprounds = d.maxproprounds;
        let dynamiclookahead = d.dynamiclookahead;
        let usesmallweightsitlim = d.usesmallweightsitlim;
        let storesemiinitcosts = d.storesemiinitcosts;
        let usesblocalinfo = d.usesblocalinfo;

        // Search for the best pseudo cost candidate, while remembering unreliable
        // candidates in a sorted buffer.
        let mut nuninitcands = 0;
        let mut bestpscand: i32 = -1;
        let mut bestpsscore = -scip.set.infinity();
        let mut bestpsfracscore = -scip.set.infinity();
        let mut bestpsdomainscore = -scip.set.infinity();

        // Search for the best candidate first.
        if usehyptestforreliability {
            for c in 0..nbranchcands {
                let cs = cand_scores(scip, branchrule, branchcands[c]);
                let mut pscostscore = sv::get_var_pseudocost_score(scip, branchcands[c], branchcandssol[c]);

                // Replace the pseudo cost score with the already calculated one.
                if sb::get_var_strongbranch_node(scip, branchcands[c]) == nodenum {
                    // Use the score of the strong branching call at the current node.
                    let (down, up, _, _, _, lastlpobjval) = sb::get_var_strongbranch_last(scip, branchcands[c])?;
                    let mut downgain = max(down - lastlpobjval, 0.0);
                    let mut upgain = max(up - lastlpobjval, 0.0);
                    if useancpscost {
                        // Add discounted gains as stored in dpscost.
                        let downsol = scip.set.feas_ceil(branchcandssol[c] - 1.0);
                        let upsol = scip.set.feas_floor(branchcandssol[c] + 1.0);
                        downgain += discountfactor * crate::var::var_get_anc_pseudocost(scip, branchcands[c], downsol - branchcandssol[c]);
                        upgain += discountfactor * crate::var::var_get_anc_pseudocost(scip, branchcands[c], upsol - branchcandssol[c]);
                        downgain /= 1.0 + discountfactor;
                        upgain /= 1.0 + discountfactor;
                    }

                    pscostscore = crate::branch::branch_get_score(scip, branchcands[c], downgain, upgain);
                }

                let score = calc_score(
                    scip,
                    &w,
                    cs.conflictscore,
                    avgconflictscore,
                    cs.conflengthscore,
                    avgconflengthscore,
                    cs.inferencescore,
                    avginferencescore,
                    cs.cutoffscore,
                    avgcutoffscore,
                    cs.gmieffscore,
                    cs.lastgmieffscore,
                    pscostscore,
                    avgpscostscore,
                    cs.nlscore,
                    branchcandsfrac[c],
                    degeneracyfactor,
                );

                // Check for better score of candidate.
                if scip.set.is_sum_ge(score, bestpsscore) {
                    let fracscore = min(branchcandsfrac[c], 1.0 - branchcandsfrac[c]);
                    let domainscore = -(scip.vars[branchcands[c]].locdom.ub - scip.vars[branchcands[c]].locdom.lb);
                    if scip.set.is_sum_gt(score, bestpsscore)
                        || scip.set.is_sum_gt(fracscore, bestpsfracscore)
                        || (scip.set.is_sum_ge(fracscore, bestpsfracscore) && domainscore > bestpsdomainscore)
                    {
                        bestpscand = c as i32;
                        bestpsscore = score;
                        bestpsfracscore = fracscore;
                        bestpsdomainscore = domainscore;
                    }
                }
            }
        }

        let bestps = |bestpscand: i32| {
            if bestpscand >= 0 {
                (branchcands[bestpscand as usize], branchcandsfrac[bestpscand as usize])
            } else {
                (crate::var::NONE, 0.0)
            }
        };

        // Use discounted pseudocosts only if all candidates are reliable.
        if useancpscost && maxninitcands > 0 {
            // Look for at least one unreliable candidate.
            for c in 0..nbranchcands {
                let (bv, bf) = bestps(bestpscand);
                if needs_strong_branching(
                    scip,
                    branchrule,
                    branchcands[c],
                    branchcandsfrac[c],
                    bv,
                    bf,
                    reliable,
                    relerrorthreshold,
                    clevel,
                    useancpscost,
                ) {
                    useancpscost = false;
                    break;
                }
            }
        }

        if useancpscost {
            avgpscostscore = avgdpscostscore;
        }

        for c in 0..nbranchcands {
            // Record the variables current pseudocosts. These may be overwritten if
            // strong branching is performed.
            sbdownvalid[c] = false;
            sbupvalid[c] = false;
            let fracpart = scip.set.feas_frac(crate::var::var_get_lp_sol(scip, branchcands[c]));
            let mut downgain = crate::var::var_get_pseudocost(scip, branchcands[c], 0.0 - fracpart);
            let mut upgain = crate::var::var_get_pseudocost(scip, branchcands[c], 1.0 - fracpart);
            mingains[c] = min(downgain, upgain);
            maxgains[c] = max(downgain, upgain);

            // Get conflict, inference, cutoff, nonlinear, and pseudo cost scores for
            // candidate.
            let cs = cand_scores(scip, branchrule, branchcands[c]);
            let mut pscostscore = sv::get_var_pseudocost_score(scip, branchcands[c], branchcandssol[c]);
            let mut usesb = false;
            if useancpscost {
                pscostscore = sv::get_var_dpseudocost_score(scip, branchcands[c], branchcandssol[c], discountfactor);
            }

            // Don't use strong branching on variables that have already been
            // initialized at the current node; instead replace the pseudo cost score
            // with the already calculated one.
            if sb::get_var_strongbranch_node(scip, branchcands[c]) == nodenum {
                // Use the score of the strong branching call at the current node.
                let (down, up, _, _, _, lastlpobjval) = sb::get_var_strongbranch_last(scip, branchcands[c])?;
                downgain = max(down - lastlpobjval, 0.0);
                upgain = max(up - lastlpobjval, 0.0);

                // Add discounted gains as stored in anspscost.
                if useancpscost {
                    // The anspscost must be reliable here.
                    let downsol = scip.set.feas_ceil(branchcandssol[c] - 1.0);
                    let upsol = scip.set.feas_floor(branchcandssol[c] + 1.0);
                    downgain += discountfactor * crate::var::var_get_anc_pseudocost(scip, branchcands[c], downsol - branchcandssol[c]);
                    upgain += discountfactor * crate::var::var_get_anc_pseudocost(scip, branchcands[c], upsol - branchcandssol[c]);
                    downgain /= 1.0 + discountfactor;
                    upgain /= 1.0 + discountfactor;
                }

                pscostscore = crate::branch::branch_get_score(scip, branchcands[c], downgain, upgain);
                mingains[c] = min(downgain, upgain);
                maxgains[c] = max(downgain, upgain);

                if dynamiclookahead {
                    update_min_max_mean_gain(scip, branchrule, downgain, upgain);
                }
            } else if maxninitcands > 0 {
                // Check, if the pseudo cost score of the variable is reliable.
                let downsize = crate::var::var_get_pseudocost_count_current_run(scip, branchcands[c], BranchDir::Downwards);
                let upsize = crate::var::var_get_pseudocost_count_current_run(scip, branchcands[c], BranchDir::Upwards);
                let size = min(downsize, upsize);

                let (bv, bf) = bestps(bestpscand);
                usesb = needs_strong_branching(scip, branchrule, branchcands[c], branchcandsfrac[c], bv, bf, reliable, relerrorthreshold, clevel, false);

                // Count the number of variables that are completely uninitialized.
                if size < 0.1 {
                    nuninitcands += 1;
                }
            }

            // Combine the five score values.
            scoresfrompc[c] = calc_score(
                scip,
                &w,
                0.0,
                avgconflictscore,
                0.0,
                avgconflengthscore,
                0.0,
                avginferencescore,
                0.0,
                avgcutoffscore,
                0.0,
                0.0,
                pscostscore,
                avgpscostscore,
                0.0,
                branchcandsfrac[c],
                degeneracyfactor,
            );
            scoresfromothers[c] = calc_score(
                scip,
                &w,
                cs.conflictscore,
                avgconflictscore,
                cs.conflengthscore,
                avgconflengthscore,
                cs.inferencescore,
                avginferencescore,
                cs.cutoffscore,
                avgcutoffscore,
                cs.gmieffscore,
                cs.lastgmieffscore,
                0.0,
                avgpscostscore,
                cs.nlscore,
                branchcandsfrac[c],
                degeneracyfactor,
            );
            let mut score = scoresfrompc[c] + scoresfromothers[c];
            scores[c] = score;

            if usesb {
                mingains[c] = 0.0;
                maxgains[c] = 0.0;
                scoresfrompc[c] = 0.0;
                scoresfromothers[c] = 0.0;

                // Assign a random score to this uninitialized candidate.
                if randinitorder {
                    score += data_mut(scip, branchrule).randnumgen.as_mut().unwrap().get_real(0.0, 1e-4);
                }

                // Pseudo cost of variable is not reliable: insert candidate in initcands
                // buffer.
                let mut j = ninitcands;
                while j > 0 && score > initcandscores[j - 1] {
                    initcands[j] = initcands[j - 1];
                    initcandscores[j] = initcandscores[j - 1];
                    j -= 1;
                }
                initcands[j] = c;
                initcandscores[j] = score;
                ninitcands += 1;
                ninitcands = min(ninitcands, maxninitcands);
            }
            // In the case of hypothesis reliability, the best pseudo candidate has
            // been determined already.
            else if !usehyptestforreliability {
                // Variable will keep its pseudo cost value: check for better score of
                // candidate.
                if scip.set.is_sum_ge(score, bestpsscore) {
                    let fracscore = min(branchcandsfrac[c], 1.0 - branchcandsfrac[c]);
                    let domainscore = -(scip.vars[branchcands[c]].locdom.ub - scip.vars[branchcands[c]].locdom.lb);
                    if scip.set.is_sum_gt(score, bestpsscore)
                        || scip.set.is_sum_gt(fracscore, bestpsfracscore)
                        || (scip.set.is_sum_ge(fracscore, bestpsfracscore) && domainscore > bestpsdomainscore)
                    {
                        bestpscand = c as i32;
                        bestpsscore = score;
                        bestpsfracscore = fracscore;
                        bestpsdomainscore = domainscore;
                    }
                }
            }
        }

        // In the special case that only the best pseudo candidate was selected for
        // strong branching, skip the strong branching.
        if usehyptestforreliability && ninitcands == 1 {
            ninitcands = 0;
        }

        // Initialize unreliable candidates with strong branching until
        // maxlookahead is reached, search best strong branching candidate.
        let maxlookahead = maxlookaheadparam as f64 * (1.0 + nuninitcands as f64 / nbranchcands as f64);
        let mut inititer = inititerparam;
        if inititer == 0 {
            // Iteration limit is set to twice the average number of iterations spent
            // to resolve a dual feasible SCIP_LP; at the first few nodes, this
            // average is not very exact, so we better increase the iteration limit on
            // these very important nodes.
            let mut nlpiterations = scip.stat.ndualresolvelpiterations;
            let mut nlps = scip.stat.ndualresolvelps;
            if nlps == 0 {
                nlpiterations = scip.stat.ninitlpiterations;
                nlps = scip.stat.ninitlps;
                if nlps == 0 {
                    nlpiterations = 1000;
                    nlps = 1;
                }
            }
            inititer = (2 * nlpiterations / nlps) as i32;
            inititer = (inititer as f64 * (1.0 + 20.0 / nodenum as f64)) as i32;
            inititer = max(inititer, 10);
            inititer = min(inititer, 500);
        }

        let mut bestsbcand: i32 = -1;
        let mut bestsbscore = -scip.set.infinity();
        let mut bestsbfracscore = -scip.set.infinity();
        let mut bestsbdomainscore = -scip.set.infinity();
        let mut bestuninitsbscore = -scip.set.infinity();
        let mut bestuninitsbcand: i32 = -1;
        let mut lookahead = 0.0;
        let mut newlbs: Vec<f64> = Vec::new();
        let mut newubs: Vec<f64> = Vec::new();

        let mut i = 0;
        while continue_strong_branching_lookahead(
            scip,
            i,
            ninitcands,
            lookahead,
            maxlookahead,
            bdchgs.len() as i32,
            nbdconflicts,
            maxbdchgs,
            maxnsblpiterations,
        ) && continue_strong_branching_tree_size_estimation(scip, branchrule, lookahead, maxlookahead)
        {
            // Get candidate number to initialize.
            let c = initcands[i];

            if skipbadinitcands {
                let mut skipsb = false;
                // If the current best candidate is a candidate found by strong branching,
                // determine if candidate pseudo-costs are significantly smaller in at
                // least one direction, in which case we safe the execution of
                // strong-branching for now.
                if bestsbscore > bestpsscore && bestsbscore > bestuninitsbscore && bestsbupvalid && bestsbdownvalid {
                    // Test if the variable is unlikely to produce a better gain than the
                    // currently best one. Skip strong-branching in such a case.
                    if crate::var::var_pscost_threshold_probability_test(scip, branchcands[c], branchcandsfrac[c], bestsbdown, BranchDir::Downwards, clevel)
                        || crate::var::var_pscost_threshold_probability_test(scip, branchcands[c], 1.0 - branchcandsfrac[c], bestsbup, BranchDir::Upwards, clevel)
                    {
                        skipsb = true;
                    }
                }
                // The currently best candidate is also a pseudo-candidate; apply
                // significance test and skip candidate if it is significantly worse in
                // at least one direction.
                else if bestpscand != -1 && bestpsscore > bestuninitsbscore {
                    let b = bestpscand as usize;
                    if crate::var::var_significant_pscost_difference(
                        scip,
                        branchcands[b],
                        branchcandsfrac[b],
                        branchcands[c],
                        branchcandsfrac[c],
                        BranchDir::Downwards,
                        clevel,
                        true,
                    ) || crate::var::var_significant_pscost_difference(
                        scip,
                        branchcands[b],
                        1.0 - branchcandsfrac[b],
                        branchcands[c],
                        1.0 - branchcandsfrac[c],
                        BranchDir::Upwards,
                        clevel,
                        true,
                    ) {
                        skipsb = true;
                    }
                }
                // Compare against the best init cand that has been skipped already.
                else if bestuninitsbcand != -1 {
                    let b = bestuninitsbcand as usize;
                    if crate::var::var_significant_pscost_difference(
                        scip,
                        branchcands[b],
                        branchcandsfrac[b],
                        branchcands[c],
                        branchcandsfrac[c],
                        BranchDir::Downwards,
                        clevel,
                        true,
                    ) || crate::var::var_significant_pscost_difference(
                        scip,
                        branchcands[b],
                        1.0 - branchcandsfrac[b],
                        branchcands[c],
                        1.0 - branchcandsfrac[c],
                        BranchDir::Upwards,
                        clevel,
                        true,
                    ) {
                        skipsb = true;
                    }
                }

                // Skip candidate, update the best score of an unitialized candidate.
                if skipsb {
                    if bestuninitsbcand == -1 {
                        bestuninitsbcand = c as i32;
                        bestuninitsbscore = initcandscores[i];
                    }
                    i += 1;
                    continue;
                }
            }

            // Use strong branching on candidate.
            if !initstrongbranching {
                initstrongbranching = true;

                sb::start_strongbranch(scip, propagate)?;

                // Create arrays for probing-like bound tightening.
                if probingbounds {
                    newlbs = vec![0.0; nvars];
                    newubs = vec![0.0; nvars];
                }
            }

            let r = if propagate {
                // Apply strong branching.
                let bounds = if probingbounds { Some((&mut newlbs[..], &mut newubs[..])) } else { None };
                sb::get_var_strongbranch_with_propagation(scip, branchcands[c], branchcandssol[c], lpobjval, inititer, maxproprounds, bounds)?
            } else {
                // Apply strong branching.
                let mut r = sb::get_var_strongbranch_frac(scip, branchcands[c], inititer, false)?;
                r.ndomredsdown = 0;
                r.ndomredsup = 0;
                r
            };
            let (mut down, mut up, downvalid, upvalid, downinf, upinf) = (r.down, r.up, r.downvalid, r.upvalid, r.downinf, r.upinf);

            // Check for an error in strong branching.
            if r.lperror {
                if !crate::solve::solve_is_stopped(scip, false) {
                    // SCIP_VERBLEVEL_HIGH.
                    if scip.set.disp_verblevel >= 4 {
                        log::info!(
                            "(node {}) error in strong branching call for variable <{}> with solution {}",
                            api::get_n_nodes(scip),
                            scip.vars[branchcands[c]].name,
                            branchcandssol[c]
                        );
                    }
                }
                break;
            }

            // Strong branching might have found a new primal solution which updated
            // the cutoff bound. In this case, the provedbound computed before can be
            // higher than the cutoffbound and the current node can be cut off.
            // Additionally, also if the value for the current best candidate is valid
            // and exceeds the new cutoff bound, we want to change the domain of this
            // variable rather than branching on it.
            if api::get_best_sol(scip) != crate::var::NONE && best_sol_index(scip) != bestsolidx {
                bestsolidx = best_sol_index(scip);

                // Proved bound for current node is larger than new cutoff bound -> cut
                // off current node.
                if scip.set.is_ge(provedbound, api::get_cutoffbound(scip)) {
                    *result = ScipResultCode::Cutoff;
                    // Terminate initialization loop, because node is infeasible.
                    break;
                }
                // Proved bound for down child of best candidate is larger than cutoff
                // bound -> increase lower bound of best candidate; we must only do this
                // if the LP is complete, i.e., we are not doing column generation.
                else if bestsbcand != -1 && allcolsinlp {
                    let b = bestsbcand as usize;
                    if !bestsbdowncutoff && bestsbdownvalid && scip.set.is_ge(bestsbdown, api::get_cutoffbound(scip)) {
                        bestsbdowncutoff = true;
                        bdchgs.push(Bdchg {
                            ind: scip.vars[branchcands[b]].probindex as usize,
                            boundtype: BoundType::Lower,
                            bound: scip.set.feas_ceil(branchcandssol[b]),
                        });
                    }
                    // Proved bound for up child of best candidate is larger than cutoff
                    // bound -> decrease upper bound of best candidate.
                    else if !bestsbupcutoff && bestsbupvalid && scip.set.is_ge(bestsbup, api::get_cutoffbound(scip)) {
                        bestsbupcutoff = true;
                        bdchgs.push(Bdchg {
                            ind: scip.vars[branchcands[b]].probindex as usize,
                            boundtype: BoundType::Upper,
                            bound: scip.set.feas_floor(branchcandssol[b]),
                        });
                    }
                }
            }

            // Evaluate strong branching.
            down = max(down, lpobjval);
            up = max(up, lpobjval);
            let downgain = down - lpobjval;
            let upgain = up - lpobjval;

            if dynamiclookahead {
                update_min_max_mean_gain(scip, branchrule, downgain, upgain);
            }

            // Depending on the user parameter choice of storesemiinitcosts, pseudo
            // costs are also updated in single directions, if the node in the other
            // direction was infeasible or cut off.
            if !downinf && (!upinf || storesemiinitcosts) {
                // Smaller weights are given if the strong branching hit the time limit
                // in the corresponding direction.
                let weight = if usesmallweightsitlim {
                    if scip.stat.lastsblpsolstats[0] != LpSolStat::IterLimit { 1.0 } else { 0.5 }
                } else {
                    1.0
                };

                // Update pseudo cost values.
                update_var_pseudocost_symmetric(scip, branchrule, branchcands[c], branchorbitidx, c, 0.0 - branchcandsfrac[c], downgain, weight)?;
            }
            if !upinf && (!downinf || storesemiinitcosts) {
                // Smaller weights are given if the strong branching hit the time limit
                // in the corresponding direction.
                let weight = if usesmallweightsitlim {
                    if scip.stat.lastsblpsolstats[1] != LpSolStat::IterLimit { 1.0 } else { 0.5 }
                } else {
                    1.0
                };

                // Update pseudo cost values.
                update_var_pseudocost_symmetric(scip, branchrule, branchcands[c], branchorbitidx, c, 1.0 - branchcandsfrac[c], upgain, weight)?;
            }

            // The minimal lower bound of both children is a proved lower bound of the
            // current subtree.
            if allcolsinlp && !exactsolve && downvalid && upvalid {
                let minbound = min(down, up);
                provedbound = max(provedbound, minbound);

                // Save probing-like bounds detected during strong branching.
                if probingbounds && (!downinf || !upinf) {
                    for v in 0..nvars {
                        if scip.set.is_gt(newlbs[v], scip.vars[vars[v]].locdom.lb) {
                            bdchgs.push(Bdchg {
                                ind: v,
                                boundtype: BoundType::Lower,
                                bound: newlbs[v],
                            });
                        }
                        if scip.set.is_lt(newubs[v], scip.vars[vars[v]].locdom.ub) {
                            bdchgs.push(Bdchg {
                                ind: v,
                                boundtype: BoundType::Upper,
                                bound: newubs[v],
                            });
                        }
                    }
                }
            }

            // Check if there are infeasible roundings.
            if (downinf || upinf) && !exactsolve {
                if downinf && upinf {
                    // Both roundings are infeasible -> node is infeasible.
                    *result = ScipResultCode::Cutoff;
                    // Terminate initialization loop, because node is infeasible.
                    break;
                } else {
                    // Rounding is infeasible in one direction -> round variable in other
                    // direction.
                    bdchgs.push(Bdchg {
                        ind: scip.vars[branchcands[c]].probindex as usize,
                        boundtype: if downinf { BoundType::Lower } else { BoundType::Upper },
                        bound: if downinf {
                            scip.set.feas_ceil(branchcandssol[c])
                        } else {
                            scip.set.feas_floor(branchcandssol[c])
                        },
                    });
                    if bdchgs.len() as i32 + nbdconflicts >= maxbdchgs {
                        // Terminate initialization loop, because enough roundings are
                        // performed.
                        break;
                    }
                }
            } else {
                mingains[c] = min(downgain, upgain);
                maxgains[c] = max(downgain, upgain);

                sbdown[c] = down;
                sbup[c] = up;
                sbdownvalid[c] = downvalid;
                sbupvalid[c] = upvalid;

                // Check for a better score.
                let conflictscore = sv::get_var_conflict_score(scip, branchcands[c]);
                let conflengthscore = sv::get_var_conflictlength_score(scip, branchcands[c]);
                let d = data(scip, branchrule);
                let nlscore = calc_nlscore(d.nlcount.as_deref(), d.nlcountmax, scip.vars[branchcands[c]].probindex);

                // Optionally, use only local information obtained via strong branching
                // for this candidate, i.e., local domain reductions and no cutoff score.
                let inferencescore = if usesblocalinfo {
                    crate::branch::branch_get_score(scip, branchcands[c], r.ndomredsdown as f64, r.ndomredsup as f64)
                } else {
                    sv::get_var_avg_inference_score(scip, branchcands[c])
                };
                let cutoffscore = if usesblocalinfo { 0.0 } else { sv::get_var_avg_cutoff_score(scip, branchcands[c]) };
                let gmieffscore = if usesblocalinfo { 0.0 } else { crate::var::var_get_avg_gmiscore(scip, branchcands[c]) };
                let lastgmieffscore = if usesblocalinfo { 0.0 } else { crate::var::var_get_last_gmiscore(scip, branchcands[c]) };
                let pscostscore = crate::branch::branch_get_score(scip, branchcands[c], downgain, upgain);

                scoresfrompc[c] = calc_score(
                    scip,
                    &w,
                    0.0,
                    avgconflictscore,
                    0.0,
                    avgconflengthscore,
                    0.0,
                    avginferencescore,
                    0.0,
                    avgcutoffscore,
                    0.0,
                    0.0,
                    pscostscore,
                    avgpscostscore,
                    0.0,
                    branchcandsfrac[c],
                    degeneracyfactor,
                );
                scoresfromothers[c] = calc_score(
                    scip,
                    &w,
                    conflictscore,
                    avgconflictscore,
                    conflengthscore,
                    avgconflengthscore,
                    inferencescore,
                    avginferencescore,
                    cutoffscore,
                    avgcutoffscore,
                    gmieffscore,
                    lastgmieffscore,
                    0.0,
                    avgpscostscore,
                    nlscore,
                    branchcandsfrac[c],
                    degeneracyfactor,
                );

                let score = scoresfrompc[c] + scoresfromothers[c];
                scores[c] = score;

                if scip.set.is_sum_ge(score, bestsbscore) {
                    let fracscore = min(branchcandsfrac[c], 1.0 - branchcandsfrac[c]);
                    let domainscore = -(scip.vars[branchcands[c]].locdom.ub - scip.vars[branchcands[c]].locdom.lb);
                    if scip.set.is_sum_gt(score, bestsbscore)
                        || scip.set.is_sum_gt(fracscore, bestsbfracscore)
                        || (scip.set.is_sum_ge(fracscore, bestsbfracscore) && domainscore > bestsbdomainscore)
                    {
                        bestsbcand = c as i32;
                        bestsbscore = score;
                        bestsbdown = down;
                        bestsbup = up;
                        bestsbdownvalid = downvalid;
                        bestsbupvalid = upvalid;
                        bestsbdowncutoff = false;
                        bestsbupcutoff = false;
                        bestsbfracscore = fracscore;
                        bestsbdomainscore = domainscore;
                        lookahead = 0.0;
                    } else {
                        lookahead += 0.5;
                    }
                } else {
                    lookahead += 1.0;
                }
            }

            i += 1;
        }

        if initstrongbranching {
            sb::end_strongbranch(scip)?;

            let solstat = api::get_lp_solstat(scip);
            if solstat == LpSolStat::ObjLimit || solstat == LpSolStat::Infeasible {
                *result = ScipResultCode::Cutoff;
            }
        }

        bestcand = 0;
        if *result != ScipResultCode::Cutoff {
            // Get the score of the best uninitialized strong branching candidate.
            if i < ninitcands && bestuninitsbcand == -1 {
                bestuninitsbscore = initcandscores[i];
            }

            // If the best pseudo cost candidate is better than the best uninitialized
            // strong branching candidate, compare it to the best initialized strong
            // branching candidate.
            if bestpsscore > bestuninitsbscore && scip.set.is_sum_gt(bestpsscore, bestsbscore) {
                bestcand = bestpscand as usize;
            } else if bestsbcand >= 0 {
                bestcand = bestsbcand as usize;
            } else {
                // No candidate was initialized, and the best score is the one of the
                // first candidate in the initialization queue.
                bestcand = initcands[0];
            }

            // Update lower bound of current node.
            if allcolsinlp && !exactsolve {
                api::update_local_lowerbound(scip, provedbound)?;
            }
        }

        // Apply domain reductions.
        if !bdchgs.is_empty() {
            if *result != ScipResultCode::Cutoff {
                apply_bdchgs(scip, &vars, &bdchgs, result)?;
                if *result != ScipResultCode::Cutoff {
                    *result = ScipResultCode::ReducedDom;
                }
            }
            bdchgs.clear();
        }

        // Apply the Treemodel branching rule to potentially select a better
        // branching candidate than the current one.
        if *result != ScipResultCode::Cutoff
            && *result != ScipResultCode::ReducedDom
            && *result != ScipResultCode::ConsAdded
            && treemodel_is_enabled(&data(scip, branchrule).treemodel)
        {
            let mut usetreemodel = true;

            // If the pseudocosts are zero, use SCIPs best variable since the
            // Treemodel is not applicable.
            if scip.set.is_zero(maxgains[bestcand]) {
                usetreemodel = false;
            }

            // If SCIPs best candidate was selected due to hybrid branching scores
            // rather than because of pseudocosts, then we keep it.
            let smallpscost = data(scip, branchrule).treemodel.smallpscost;
            if usetreemodel && avgpscostscore <= smallpscost {
                for cand in 0..nbranchcands {
                    if scoresfrompc[cand] > scoresfrompc[bestcand] {
                        usetreemodel = false;
                        break;
                    }
                }
            }

            if usetreemodel {
                let treemodel = std::mem::take(&mut data_mut(scip, branchrule).treemodel);
                let r = treemodel_select_candidate(scip, &treemodel, branchcands, &mingains, &maxgains, &scoresfromothers, &mut bestcand);
                data_mut(scip, branchrule).treemodel = treemodel;
                r?;
            }
        }
        let _ = scores;
    }

    // If no domain could be reduced, create the branching.
    if *result != ScipResultCode::Cutoff && *result != ScipResultCode::ReducedDom && *result != ScipResultCode::ConsAdded && executebranch {
        let var = branchcands[bestcand];
        let val = branchcandssol[bestcand];

        // Perform the branching.
        let children = api::branch_var_val(scip, var, val)?;

        // Update the lower bounds in the children.
        if allcolsinlp && !exactsolve {
            if sbdownvalid[bestcand] {
                api::update_node_lowerbound(scip, children.downchild, sbdown[bestcand])?;
            }
            if sbupvalid[bestcand] {
                api::update_node_lowerbound(scip, children.upchild, sbup[bestcand])?;
            }
        }

        *result = ScipResultCode::Branched;
    }
    let _ = ninitcands;

    Ok(())
}

/*
 * Callback methods
 */

/// `branchCopyRelpscost`: copy method for branchrule plugins.
fn branch_copy_relpscost(scip: &mut Scip, _sourcescip: &Scip, _branchrule: BranchruleId) -> ScipResult {
    // Call inclusion method of branchrule.
    include_branchrule_relpscost(scip)
}

/// `branchInitsolRelpscost`: solving process initialization method of
/// branching rule.
fn branch_initsol_relpscost(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    let seed = scip.set.initialize_random_seed(data(scip, branchrule).startrandseed as u32);
    let d = data_mut(scip, branchrule);

    // Initialize branching rule data.
    d.nlcount = None;
    d.nlcountsize = 0;
    d.nlcountmax = 1;

    // Create a random number generator.
    d.randnumgen = Some(RandNumGen::new(seed));

    Ok(())
}

/// `branchExitsolRelpscost`: solving process deinitialization method of
/// branching rule.
fn branch_exitsol_relpscost(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    let d = data_mut(scip, branchrule);

    // Free memory in branching rule data.
    d.nlcount = None;
    d.nlcountsize = 0;

    // Free random number generator.
    d.randnumgen = None;

    d.nosymmetry = false;
    d.norbits = 0;
    d.npermvars = 0;

    Ok(())
}

/// `branchExeclpRelpscost`: branching execution method for fractional LP
/// solutions.
fn branch_execlp_relpscost(scip: &mut Scip, branchrule: BranchruleId, _allowaddcons: bool, result: &mut ScipResultCode) -> ScipResult {
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    // Get branching candidates.
    let (lpcands, lpcandssol, lpcandsfrac, nlpcands, _, _) = api::get_lp_branch_cands(scip)?;
    let nlpcands = nlpcands as usize;

    let inf = scip.set.infinity();
    let d = data_mut(scip, branchrule);
    d.maxmeangain = -inf;
    d.minmeangain = inf;
    d.sumlogmeangains = 0.0;
    d.logstdevgain = 0.0;
    d.nzerogains = 0.0;

    // Determine whether we should run filtering.
    let runfiltering = !d.nosymmetry && d.filtercandssym && scip.stat.subscipdepth == 0 && !crate::lp::lp_diving(scip) && !api::in_probing(scip);

    // Init orbits if necessary (the symmetry handling is not ported).
    if runfiltering {
        return Err(Retcode::NotImplemented);
    }

    // No orbits available. Copy all (unfiltered) branching candidates, because
    // they will be updated w.r.t. the strong branching LP solution.
    let filteredlpcands = lpcands[..nlpcands].to_vec();
    let filteredlpcandssol = lpcandssol[..nlpcands].to_vec();
    let filteredlpcandsfrac = lpcandsfrac[..nlpcands].to_vec();

    // Execute branching rule.
    exec_relpscost(scip, branchrule, &filteredlpcands, &filteredlpcandssol, &filteredlpcandsfrac, None, true, result)
}

/*
 * branching specific interface methods
 */

macro_rules! target {
    ($ty:ty, $id:expr, $field:ident) => {
        crate::data_target!($ty, branchrules, $id, branchruledata, BranchruleData, $field)
    };
}

fn treemodel_get(scip: &Scip, branchrule: BranchruleId) -> &Treemodel {
    &data(scip, branchrule).treemodel
}

fn treemodel_get_mut(scip: &mut Scip, branchrule: BranchruleId) -> &mut Treemodel {
    &mut data_mut(scip, branchrule).treemodel
}

/// `SCIPincludeBranchruleRelpscost`: creates the reliable pseudo cost
/// branching rule and includes it in SCIP.
pub fn include_branchrule_relpscost(scip: &mut Scip) -> ScipResult {
    use crate::def::{REAL_MAX, REAL_MIN};

    // Create relpscost branching rule data.
    let inf = scip.set.infinity();
    let branchruledata = BranchruleData {
        maxmeangain: -inf,
        minmeangain: inf,
        ..Default::default()
    };

    // Include branching rule.
    let methods = crate::branch::BranchruleMethods {
        branchcopy: Some(branch_copy_relpscost),
        branchinitsol: Some(branch_initsol_relpscost),
        branchexitsol: Some(branch_exitsol_relpscost),
        branchexeclp: Some(branch_execlp_relpscost),
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
        Some(Box::new(branchruledata) as Box<dyn Any>),
    )?;

    // Relpscost branching rule parameters.
    let real = |scip: &mut Scip, name: &str, desc: &str, t, adv: bool, def: f64, lo: f64, hi: f64| {
        paramset_add_real(scip, &format!("branching/relpscost/{name}"), desc, t, adv, def, lo, hi, None).map(|_| ())
    };
    let int = |scip: &mut Scip, name: &str, desc: &str, t, adv: bool, def: i32, lo: i32, hi: i32| {
        paramset_add_int(scip, &format!("branching/relpscost/{name}"), desc, t, adv, def, lo, hi, None).map(|_| ())
    };
    let boolp =
        |scip: &mut Scip, name: &str, desc: &str, t, adv: bool, def: bool| paramset_add_bool(scip, &format!("branching/relpscost/{name}"), desc, t, adv, def, None).map(|_| ());

    real(
        scip,
        "conflictweight",
        "weight in score calculations for conflict score",
        target!(f64, b, conflictweight),
        true,
        DEFAULT_CONFLICTWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "conflictlengthweight",
        "weight in score calculations for conflict length score",
        target!(f64, b, conflengthweight),
        true,
        DEFAULT_CONFLENGTHWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "inferenceweight",
        "weight in score calculations for inference score",
        target!(f64, b, inferenceweight),
        true,
        DEFAULT_INFERENCEWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "cutoffweight",
        "weight in score calculations for cutoff score",
        target!(f64, b, cutoffweight),
        true,
        DEFAULT_CUTOFFWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "gmiavgeffweight",
        "weight in score calculations for average GMI cuts normalized efficacy",
        target!(f64, b, gmiavgeffweight),
        true,
        DEFAULT_GMIAVGEFFWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "gmilasteffweight",
        "weight in score calculations for last GMI cuts normalized efficacy",
        target!(f64, b, gmilasteffweight),
        true,
        DEFAULT_GMILASTEFFWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "pscostweight",
        "weight in score calculations for pseudo cost score",
        target!(f64, b, pscostweight),
        true,
        DEFAULT_PSCOSTWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "nlscoreweight",
        "weight in score calculations for nlcount score",
        target!(f64, b, nlscoreweight),
        true,
        DEFAULT_NLSCOREWEIGHT,
        REAL_MIN,
        REAL_MAX,
    )?;
    real(
        scip,
        "minreliable",
        "minimal value for minimum pseudo cost size to regard pseudo cost value as reliable",
        target!(f64, b, minreliable),
        true,
        DEFAULT_MINRELIABLE,
        0.0,
        REAL_MAX,
    )?;
    real(
        scip,
        "maxreliable",
        "maximal value for minimum pseudo cost size to regard pseudo cost value as reliable",
        target!(f64, b, maxreliable),
        true,
        DEFAULT_MAXRELIABLE,
        0.0,
        REAL_MAX,
    )?;
    real(
        scip,
        "sbiterquot",
        "maximal fraction of strong branching LP iterations compared to node relaxation LP iterations",
        target!(f64, b, sbiterquot),
        false,
        DEFAULT_SBITERQUOT,
        0.0,
        REAL_MAX,
    )?;
    real(
        scip,
        "dynamiclookaheadquot",
        "apply dynamic lookahead after this fraction maxlookahead is reached",
        target!(f64, b, dynamiclookaheadquot),
        true,
        DEFAULT_DYNAMICLOOKAHEADQUOT,
        0.0,
        1.0,
    )?;
    int(
        scip,
        "sbiterofs",
        "additional number of allowed strong branching LP iterations",
        target!(i32, b, sbiterofs),
        false,
        DEFAULT_SBITEROFS,
        0,
        i32::MAX,
    )?;
    int(
        scip,
        "maxlookahead",
        "maximal number of further variables evaluated without better score",
        target!(i32, b, maxlookahead),
        true,
        DEFAULT_MAXLOOKAHEAD,
        1,
        i32::MAX,
    )?;
    boolp(
        scip,
        "dynamiclookahead",
        "should we use a dynamic lookahead based on a tree size estimation of further strong branchings?",
        target!(bool, b, dynamiclookahead),
        true,
        DEFAULT_DYNAMICLOOKAHEAD,
    )?;
    int(
        scip,
        "initcand",
        "maximal number of candidates initialized with strong branching per node",
        target!(i32, b, initcand),
        false,
        DEFAULT_INITCAND,
        0,
        i32::MAX,
    )?;
    int(
        scip,
        "dynamiclookdistribution",
        "which distribution should be used for dynamic lookahead? 0=exponential, 1=Pareto, 2=log-normal?",
        target!(i32, b, dynamiclookdistribution),
        true,
        DEFAULT_DYNAMICLOOKDISTRIBUTION,
        EXPONENTIALDISTRIBUTION,
        LOGNORMALDISTRIBUTION,
    )?;
    int(
        scip,
        "inititer",
        "iteration limit for strong branching initializations of pseudo cost entries (0: auto)",
        target!(i32, b, inititer),
        false,
        DEFAULT_INITITER,
        0,
        i32::MAX,
    )?;
    int(
        scip,
        "maxbdchgs",
        "maximal number of bound tightenings before the node is reevaluated (-1: unlimited)",
        target!(i32, b, maxbdchgs),
        true,
        DEFAULT_MAXBDCHGS,
        -1,
        i32::MAX,
    )?;
    let maxproprounds = if scip.set.exact_enable { 0 } else { DEFAULT_MAXPROPROUNDS };
    int(
        scip,
        "maxproprounds",
        "maximum number of propagation rounds to be performed during strong branching before solving the LP (-1: no limit, -2: parameter settings)",
        target!(i32, b, maxproprounds),
        true,
        maxproprounds,
        -2,
        i32::MAX,
    )?;
    boolp(
        scip,
        "probingbounds",
        "should valid bounds be identified in a probing-like fashion during strong branching (only with propagation)?",
        target!(bool, b, probingbounds),
        true,
        DEFAULT_PROBINGBOUNDS,
    )?;
    boolp(
        scip,
        "userelerrorreliability",
        "should reliability be based on relative errors?",
        target!(bool, b, userelerrorforreliability),
        true,
        DEFAULT_USERELERRORFORRELIABILITY,
    )?;
    real(
        scip,
        "lowerrortol",
        "low relative error tolerance for reliability",
        target!(f64, b, lowerrortol),
        true,
        DEFAULT_LOWERRORTOL,
        0.0,
        REAL_MAX,
    )?;
    real(
        scip,
        "higherrortol",
        "high relative error tolerance for reliability",
        target!(f64, b, higherrortol),
        true,
        DEFAULT_HIGHERRORTOL,
        0.0,
        REAL_MAX,
    )?;
    boolp(
        scip,
        "storesemiinitcosts",
        "should strong branching result be considered for pseudo costs if the other direction was infeasible?",
        target!(bool, b, storesemiinitcosts),
        true,
        DEFAULT_STORESEMIINITCOSTS,
    )?;
    boolp(
        scip,
        "usesblocalinfo",
        "should the scoring function use only local cutoff and inference information obtained for strong branching candidates?",
        target!(bool, b, usesblocalinfo),
        true,
        DEFAULT_USESBLOCALINFO,
    )?;
    boolp(
        scip,
        "usehyptestforreliability",
        "should the strong branching decision be based on a hypothesis test?",
        target!(bool, b, usehyptestforreliability),
        true,
        DEFAULT_USEHYPTESTFORRELIABILITY,
    )?;
    boolp(
        scip,
        "usedynamicconfidence",
        "should the confidence level be adjusted dynamically?",
        target!(bool, b, usedynamicconfidence),
        true,
        DEFAULT_USEDYNAMICCONFIDENCE,
    )?;
    boolp(
        scip,
        "skipbadinitcands",
        "should branching rule skip candidates that have a low probability to be better than the best strong-branching or pseudo-candidate?",
        target!(bool, b, skipbadinitcands),
        true,
        DEFAULT_SKIPBADINITCANDS,
    )?;
    int(
        scip,
        "confidencelevel",
        "the confidence level for statistical methods, between 0 (Min) and 4 (Max).",
        target!(i32, b, confidencelevel),
        true,
        DEFAULT_CONFIDENCELEVEL,
        0,
        4,
    )?;
    boolp(
        scip,
        "randinitorder",
        "should candidates be initialized in randomized order?",
        target!(bool, b, randinitorder),
        true,
        DEFAULT_RANDINITORDER,
    )?;
    boolp(
        scip,
        "usesmallweightsitlim",
        "should smaller weights be used for pseudo cost updates after hitting the LP iteration limit?",
        target!(bool, b, usesmallweightsitlim),
        true,
        DEFAULT_USESMALLWEIGHTSITLIM,
    )?;
    boolp(
        scip,
        "dynamicweights",
        "should the weights of the branching rule be adjusted dynamically during solving based on objective and infeasible leaf counters?",
        target!(bool, b, dynamicweights),
        true,
        DEFAULT_DYNAMICWEIGHTS,
    )?;
    int(
        scip,
        "degeneracyaware",
        "should degeneracy be taken into account to update weights and skip strong branching? (0: off, 1: after root, 2: always)",
        target!(i32, b, degeneracyaware),
        true,
        DEFAULT_DEGENERACYAWARE,
        0,
        2,
    )?;
    int(
        scip,
        "startrandseed",
        "start seed for random number generation",
        target!(i32, b, startrandseed),
        true,
        DEFAULT_STARTRANDSEED,
        0,
        i32::MAX,
    )?;
    int(
        scip,
        "minsamplesize",
        "minimum sample size to estimate the tree size for dynamic lookahead",
        target!(i32, b, minsamplesize),
        true,
        DEFAULT_MINSAMPLESIZE,
        0,
        i32::MAX,
    )?;
    boolp(
        scip,
        "filtercandssym",
        "Use symmetry to filter branching candidates?",
        target!(bool, b, filtercandssym),
        true,
        DEFAULT_FILTERCANDSSYM,
    )?;
    boolp(
        scip,
        "transsympscost",
        "Transfer pscost information to symmetric variables?",
        target!(bool, b, transsympscost),
        true,
        DEFAULT_TRANSSYMPSCOST,
    )?;
    real(
        scip,
        "discountfactor",
        "discount factor for ancestral pseudo costs (0.0: disable discounted pseudo costs)",
        target!(f64, b, discountfactor),
        false,
        BRANCHRULE_DISCOUNTFACTOR,
        0.0,
        1.0,
    )?;

    // Relpcost is safe to use in exact solving mode (not ported).

    // Initialise the Treemodel parameters.
    treemodel_init(scip, b, treemodel_get, treemodel_get_mut)?;

    Ok(())
}

/// `SCIPexecRelpscostBranching`: execution reliability pseudo cost
/// branching with the given branching candidates.
pub fn exec_relpscost_branching(
    scip: &mut Scip,
    branchcands: &[VarId],
    branchcandssol: &[f64],
    branchcandsfrac: &[f64],
    executebranching: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    // Find branching rule.
    let branchrule = scip.set.branchrules.0[..scip.set.nbranchrules as usize]
        .iter()
        .copied()
        .find(|&b| scip.branchrules[b].name == BRANCHRULE_NAME)
        .unwrap();

    // Execute branching rule.
    exec_relpscost(scip, branchrule, branchcands, branchcandssol, branchcandsfrac, None, executebranching, result)
}
