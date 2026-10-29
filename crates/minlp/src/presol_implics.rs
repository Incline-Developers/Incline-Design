//! Port of `presol_implics.c`: implics presolver: the implication graph
//! aggregator.

use crate::{
    api,
    cons::*,
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    scip_var,
    types::{BoundType, PresolId, ScipResultCode, VarId, VarStatus},
};

const PRESOL_NAME: &str = "implics";
const PRESOL_DESC: &str = "implication graph aggregator";
/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = -10000;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_MEDIUM;

/*
 * Callback methods of presolver
 */

/// `presolExecImplics`: execution method of presolver.
fn presol_exec_implics(
    scip: &mut Scip,
    _presol: PresolId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    // Initialize fixing and aggregation storages.
    let mut bdchgvars: Vec<VarId> = Vec::new();
    let mut bdchgtypes: Vec<BoundType> = Vec::new();
    let mut bdchgvals: Vec<f64> = Vec::new();
    let mut aggrvars: Vec<VarId> = Vec::new();
    let mut aggraggvars: Vec<VarId> = Vec::new();
    let mut aggrcoefs: Vec<f64> = Vec::new();
    let mut aggrconsts: Vec<f64> = Vec::new();

    // Get active binary problem variables.
    let nbinvars = api::get_n_bin_vars(scip) as usize;

    // Look for variable implications in x == 0 and x == 1 with the same implied
    // variable:
    //  x = 0 -> y = lb, and x = 1 -> y = lb: fix y to lb
    //  x = 0 -> y = lb, and x = 1 -> y = ub: aggregate y == lb + (ub-lb)x
    //  x = 0 -> y = ub, and x = 1 -> y = lb: aggregate y == ub - (ub-lb)x
    //  x = 0 -> y = ub, and x = 1 -> y = ub: fix y to ub
    // The fixings and aggregations are stored in a buffer and applied
    // afterwards, because fixing and aggregation would modify the vars array
    // and the implication arrays.
    for v in 0..nbinvars {
        let var = scip.probs[TRANSPROB].vars[v];

        // Don't perform presolving operations on deleted variables.
        if scip.vars[var].deleted {
            continue;
        }

        // Get implications for given variable.
        let (implvars, impltypes, implbounds, nimpls) = match scip.vars[var].implics.as_ref() {
            Some(im) => (
                [im.vars[0].0.clone(), im.vars[1].0.clone()],
                [im.types[0].0.clone(), im.types[1].0.clone()],
                [im.bounds[0].0.clone(), im.bounds[1].0.clone()],
                [im.nimpls[0] as usize, im.nimpls[1] as usize],
            ),
            None => Default::default(),
        };

        // Scan implication arrays for equal variables.
        let mut i0 = 0;
        let mut i1 = 0;
        while i0 < nimpls[0] && i1 < nimpls[1] {
            // Scan the binary or non-binary part of the implication arrays.
            let mut index0 = scip.vars[implvars[0][i0]].index;
            let mut index1 = scip.vars[implvars[1][i1]].index;
            while index0 < index1 {
                i0 += 1;
                if i0 == nimpls[0] {
                    index0 = -1;
                    break;
                }
                index0 = scip.vars[implvars[0][i0]].index;
            }
            while index1 < index0 {
                i1 += 1;
                if i1 == nimpls[1] {
                    index1 = -1;
                    break;
                }
                index1 = scip.vars[implvars[1][i1]].index;
            }

            if index0 == index1 {
                let implvar = implvars[0][i0];

                // Multiaggregated variables cannot be aggregated or their bounds
                // tightened.
                if scip.vars[implvar].varstatus != VarStatus::MultAggr {
                    if impltypes[0][i0] == impltypes[1][i1] {
                        // Found implication x = 0 -> y >= b / y <= b  and  x = 1 -> y >= c
                        // / y <= c  =>  change bound y >= min(b,c) / y <= max(b,c).
                        bdchgvars.push(implvar);
                        bdchgtypes.push(impltypes[0][i0]);
                        if impltypes[0][i0] == BoundType::Lower {
                            bdchgvals.push(crate::def::min(implbounds[0][i0], implbounds[1][i1]));
                        } else {
                            bdchgvals.push(crate::def::max(implbounds[0][i0], implbounds[1][i1]));
                        }
                    } else {
                        let implvarlb = scip.vars[implvar].glbdom.lb;
                        let implvarub = scip.vars[implvar].glbdom.ub;

                        if impltypes[0][i0] == BoundType::Upper && scip.set.is_eq(implbounds[0][i0], implvarlb) && scip.set.is_eq(implbounds[1][i1], implvarub) {
                            // Found implication x = 0 -> y = lb and x = 1 -> y = ub  =>
                            // aggregate y = lb + (ub-lb) * x.
                            aggrvars.push(implvar);
                            aggraggvars.push(var);
                            aggrcoefs.push(implvarub - implvarlb);
                            aggrconsts.push(implvarlb);
                        } else if impltypes[0][i0] == BoundType::Lower && scip.set.is_eq(implbounds[0][i0], implvarub) && scip.set.is_eq(implbounds[1][i1], implvarlb) {
                            // Found implication x = 0 -> y = ub and x = 1 -> y = lb  =>
                            // aggregate y = ub - (ub-lb) * x.
                            aggrvars.push(implvar);
                            aggraggvars.push(var);
                            aggrcoefs.push(implvarlb - implvarub);
                            aggrconsts.push(implvarub);
                        }
                    }
                }

                // Process the next implications.
                i0 += 1;
                i1 += 1;
            }
        }
    }

    // Perform the bound changes; we cannot assume y to be active (see var.c:
    // varRemoveImplicsVbs()), but it should not cause any troubles as this case
    // seems to be handled correctly in SCIPtightenVarLb/Ub(), unless the
    // variable is multiaggregated, but this has been excluded above.
    let mut v = 0;
    while v < bdchgvars.len() && *result != ScipResultCode::Cutoff {
        let (infeasible, tightened) = if bdchgtypes[v] == BoundType::Lower {
            scip_var::tighten_var_lb(scip, bdchgvars[v], bdchgvals[v], false)?
        } else {
            scip_var::tighten_var_ub(scip, bdchgvars[v], bdchgvals[v], false)?
        };

        if infeasible {
            *result = ScipResultCode::Cutoff;
        } else if tightened {
            counts.nchgbds += 1;
            *result = ScipResultCode::Success;
        }
        v += 1;
    }

    // Perform the aggregations (see above).
    let mut v = 0;
    while v < aggrvars.len() && *result != ScipResultCode::Cutoff {
        // Aggregation y = const + coef * x  =>  y - coef * x = const.
        let (infeasible, _redundant, aggregated) = scip_var::aggregate_vars(scip, aggrvars[v], aggraggvars[v], 1.0, -aggrcoefs[v], aggrconsts[v])?;
        if infeasible {
            *result = ScipResultCode::Cutoff;
        } else if aggregated {
            counts.naggrvars += 1;
            *result = ScipResultCode::Success;
        }
        v += 1;
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `presolCopyImplics`: copy method for presolver plugins (called when SCIP
/// copies plugins).
fn presol_copy_implics(scip: &mut Scip, _sourcescip: &Scip, _presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver.
    include_presol_implics(scip)?;
    Ok(())
}

/// `SCIPincludePresolImplics`: creates the implics presolver and includes it
/// in SCIP.
pub fn include_presol_implics(scip: &mut Scip) -> ScipResult {
    // Include presolver.
    let presol = api::include_presol_basic(scip, PRESOL_NAME, PRESOL_DESC, PRESOL_PRIORITY, PRESOL_MAXROUNDS, PRESOL_TIMING, presol_exec_implics, None)?;
    scip.presols[presol].presolcopy = Some(presol_copy_implics);

    Ok(())
}
