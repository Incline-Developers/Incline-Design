//! Port of `presol_dualcomp.c`: dual compensation presolver: compensate
//! single up-/downlocks by singleton continuous variables.
//!
//! The presolver looks for a variable with a single up-/downlock and tries
//! to fix it at its bound, if singleton continuous variables in the
//! constraint causing the lock can compensate any infeasibility while
//! preserving optimality.

use std::any::Any;

use crate::{
    api,
    cons::*,
    matrix::{Matrix, matrix_create},
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_var,
    types::{LockType, PresolId, ScipResultCode},
    var::{var_get_n_locks_down_type, var_get_n_locks_up_type, var_is_integral},
};

const PRESOL_NAME: &str = "dualcomp";
const PRESOL_DESC: &str = "compensate single up-/downlocks by singleton continuous variables";

/* we need singleton continuous variables for the lock compensation, thus it
 * is presumably a good idea to call this presolver before stuffing, which
 * fixes singleton continuous variables */
/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = -50;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_EXHAUSTIVE;

/// Should only discrete variables be compensated?
const DEFAULT_COMP_ONLY_DIS_VARS: bool = false;

/*
 * Data structures
 */

/// `SCIP_PresolData`: control parameters.
pub struct PresolData {
    /// Flag indicating if only discrete variables should be compensated.
    pub componlydisvars: bool,
}

/// `FIXINGDIRECTION`: type of fixing direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fixingdirection {
    /// Fix variable at lower bound.
    FixAtLb = -1,
    /// Do not fix variable.
    NoFix = 0,
    /// Fix variable at upper bound.
    FixAtUb = 1,
}

/// `LOCKCOMPENSATION`: type of variable lock compensation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lockcompensation {
    CompensateDownlock = 0,
    CompensateUplock = 1,
}

/*
 * Local methods
 */

/// Whether column `colidx` is a valid compensation candidate: a singleton
/// continuous variable whose locks are all in the matrix.
fn is_compensation_candidate(scip: &Scip, matrix: &Matrix, colidx: usize) -> bool {
    let var = matrix.vars[colidx];
    matrix.col_nnonzs(colidx) == 1
        && !var_is_integral(scip, var)
        && var_get_n_locks_up_type(scip, var, LockType::Model) == matrix.nuplocks[colidx]
        && var_get_n_locks_down_type(scip, var, LockType::Model) == matrix.ndownlocks[colidx]
}

/// `compensateVarLock`: try to compensate a variable with a single opposite
/// lock by using singleton continuous variables.
#[allow(clippy::too_many_arguments)]
fn compensate_var_lock(
    scip: &Scip,
    matrix: &Matrix,
    col: usize,
    row: usize,
    val: f64,
    twosides: bool,
    compensation: Lockcompensation,
    varstofix: &mut [Fixingdirection],
    nfixings: &mut i32,
) {
    // The variable for compensation should not be a compensation variable
    // itself; try lock compensation only if minimum one singleton continuous
    // variable is present.
    let singleton = matrix.row_range(row).any(|k| is_compensation_candidate(scip, matrix, matrix.rowmatind[k] as usize));

    // Return if no compensation variable is available.
    if !singleton {
        return;
    }

    // We perform the following transformations afterwards: lhs <= a1 x1 + a2
    // x2 + ... an xn <= rhs with a1, a2, ..., an >= 0. For the downlock case
    // we multiply the constraint in thought by (-1) if the corresponding
    // coefficient is negative. We attribute the uplock case to the downlock
    // case by multiplying in thought the corresponding column by (-1).
    let mut multrowbyminusone = false;
    if compensation == Lockcompensation::CompensateDownlock {
        if scip.set.is_lt(val, 0.0) {
            multrowbyminusone = true;
        }
    } else {
        // In the uplock case we multiply the column in thought by (-1) and thus
        // we need to multiply the constraint by (-1) to get a positive
        // coefficient.
        if scip.set.is_gt(val, 0.0) {
            multrowbyminusone = true;
        }
    }

    // We need the objective coefficient and constraint coefficient ratio to
    // later preserve optimality. Further we need to consider multiplications
    // of the constraint by (-1). For ranged rows and equalities we switch to
    // the rhs.
    let mut lhs = matrix.lhs[row];
    let mut ratio = scip.vars[matrix.vars[col]].obj / val;
    if multrowbyminusone {
        if twosides {
            lhs = -matrix.rhs[row];
        } else {
            lhs = -lhs;
        }

        ratio = -ratio;
    }

    let mut offset = 0.0;
    let mut trytofix = true;
    let mut delta = 0.0;
    let mut deltaisinf = false;

    for k in matrix.row_range(row) {
        let colidx = matrix.rowmatind[k] as usize;
        let coef = matrix.rowmatval[k];
        let var = matrix.vars[colidx];
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;
        let obj = scip.vars[var].obj;

        if colidx == col {
            // This is the variable which we want to compensate.
            if compensation == Lockcompensation::CompensateDownlock {
                if scip.set.is_infinity(-lb) {
                    trytofix = false;
                    break;
                } else if multrowbyminusone {
                    offset += (-coef) * lb;
                } else {
                    offset += coef * lb;
                }
            } else if scip.set.is_infinity(ub) {
                trytofix = false;
                break;
            } else {
                // For the uplock case we have opposed sign for the coefficient as
                // in the downlock case. The multiplication of the column results
                // in swapping the negative bounds.
                if multrowbyminusone {
                    offset += coef * (-ub);
                } else {
                    offset += (-coef) * (-ub);
                }
            }
        } else if is_compensation_candidate(scip, matrix, colidx) {
            // This is singleton continuous variable and thus a valid compensation
            // candidate.
            if scip.set.is_lt(coef, 0.0) {
                // coef < 0
                if multrowbyminusone {
                    if scip.set.is_infinity(-lb) {
                        trytofix = false;
                        break;
                    }

                    // We have a negative coefficient and the row is multiplied by
                    // (-1) thus actually we have a positive coefficient.
                    offset += (-coef) * lb;

                    // Only consider singleton continuous variables with a better or
                    // the same obj/coef ratio for preserving optimality.
                    if scip.set.is_le(obj / (-coef), ratio) {
                        if scip.set.is_infinity(ub) {
                            deltaisinf = true;
                            break;
                        }

                        // Calculate the contribution to the compensation value.
                        delta += (-coef) * (ub - lb);
                    }
                } else {
                    if scip.set.is_infinity(ub) {
                        trytofix = false;
                        break;
                    }

                    // We have a negative coefficient and hence need to multiply the
                    // column by (-1). This means the bounds swap and change the
                    // sign.
                    offset += (-coef) * (-ub);

                    // Only consider singleton continuous variables with a better or
                    // the same obj/coef ratio for preserving optimality.
                    if scip.set.is_le(obj / coef, ratio) {
                        if scip.set.is_infinity(-lb) {
                            deltaisinf = true;
                            break;
                        }

                        // Calculate the contribution to the compensation value.
                        delta += (-coef) * (ub - lb);
                    }
                }
            } else {
                // coef >= 0
                if multrowbyminusone {
                    // We have a positive or zero coefficient and the row is
                    // multiplied by (-1).
                    if scip.set.is_infinity(ub) {
                        trytofix = false;
                        break;
                    }

                    // We have a positive or zero coefficient and multiply in thought
                    // the constraint by (-1) thus we have actually a negative
                    // coefficient and multiply the column by (-1). Therefore the sign
                    // of the coefficient does not change but the bounds swap and
                    // change the sign.
                    offset += coef * (-ub);

                    // We have a positive or zero coefficient and multiply in thought
                    // the constraint by (-1) which delivers the ratio. A further
                    // multiplication of the column does not change anything.
                    if scip.set.is_le(obj / (-coef), ratio) {
                        if scip.set.is_infinity(-lb) {
                            deltaisinf = true;
                            break;
                        }

                        // Calculate the contribution to the compensation value.
                        delta += coef * (ub - lb);
                    }
                } else {
                    if scip.set.is_infinity(-lb) {
                        trytofix = false;
                        break;
                    }

                    // We have positive coefficient and do not need to multiply
                    // anything by (-1).
                    offset += coef * lb;

                    if scip.set.is_le(obj / coef, ratio) {
                        if scip.set.is_infinity(ub) {
                            deltaisinf = true;
                            break;
                        }

                        // Calculate the contribution to the compensation value.
                        delta += coef * (ub - lb);
                    }
                }
            }
        } else {
            // Remaining variables; the reasons for the following signs are the
            // same as for the singleton continuous variables.
            if scip.set.is_lt(coef, 0.0) {
                if multrowbyminusone {
                    if scip.set.is_infinity(-lb) {
                        trytofix = false;
                        break;
                    }

                    offset += (-coef) * lb;
                } else {
                    if scip.set.is_infinity(ub) {
                        trytofix = false;
                        break;
                    }

                    offset += (-coef) * (-ub);
                }
            } else if multrowbyminusone {
                if scip.set.is_infinity(ub) {
                    trytofix = false;
                    break;
                }

                offset += coef * (-ub);
            } else {
                if scip.set.is_infinity(-lb) {
                    trytofix = false;
                    break;
                }

                offset += coef * lb;
            }
        }
    }

    // Avoid fixings to infinite values or fixings of already fixed variables.
    if trytofix && varstofix[col] == Fixingdirection::NoFix {
        // Feasibility is secured if the compensation value delta is large enough
        // to compensate the value lhs-offset.
        if deltaisinf || scip.set.is_le(lhs - offset, delta) {
            let var = matrix.vars[col];
            if compensation == Lockcompensation::CompensateUplock {
                if !scip.set.is_infinity(scip.vars[var].glbdom.ub) {
                    varstofix[col] = Fixingdirection::FixAtUb;
                    *nfixings += 1;
                }
            } else if !scip.set.is_infinity(-scip.vars[var].glbdom.lb) {
                varstofix[col] = Fixingdirection::FixAtLb;
                *nfixings += 1;
            }
        }
    }
}

/// The row causing the single lock of column `i`, searched as the reference
/// does: an equation or ranged row, or a `>=` row whose coefficient has the
/// sign `badsign` (negative for the uplock, positive for the downlock).
/// Returns `(inspect, twosides, row, val)`.
fn find_lock_row(scip: &Scip, matrix: &Matrix, i: usize, uplock: bool) -> (bool, bool, usize, f64) {
    let mut row = usize::MAX;
    let mut val = 0.0;

    for k in matrix.col_range(i) {
        row = matrix.colmatind[k] as usize;
        val = matrix.colmatval[k];
        let lhs = matrix.lhs[row];
        let rhs = matrix.rhs[row];

        if scip.set.is_eq(lhs, rhs) {
            // Equation.
            return (true, true, row, val);
        } else if matrix.isrhsinfinite[row] {
            // >=
            if (uplock && scip.set.is_lt(val, 0.0)) || (!uplock && scip.set.is_gt(val, 0.0)) {
                return (true, false, row, val);
            }
        } else if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(rhs) {
            // Ranged row.
            return (true, true, row, val);
        }
    }

    (false, false, row, val)
}

/*
 * Callback methods of presolver
 */

/// `presolExecDualcomp`: execution method of presolver.
fn presol_exec_dualcomp(
    scip: &mut Scip,
    presol: PresolId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if scip.set.stage != Stage::Presolving || api::in_probing(scip) || api::is_nlp_enabled(scip) {
        return Ok(());
    }

    if crate::solve::solve_is_stopped(scip, false) || api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // Don't run if no compensation variables are present.
    if api::get_n_cont_vars(scip) == 0 {
        return Ok(());
    }

    if !scip_var::allow_strong_dual_reds(scip) {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let componlydisvars = scip.presols[presol].presoldata.as_ref().unwrap().downcast_ref::<PresolData>().unwrap().componlydisvars;

    let created = matrix_create(
        scip,
        true,
        &mut counts.naddconss,
        &mut counts.ndelconss,
        &mut counts.nchgcoefs,
        &mut counts.nchgbds,
        &mut counts.nfixedvars,
    )?;

    // If infeasibility was detected during matrix creation, return here.
    if created.infeasible {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // We only work on pure MIPs currently.
    if let (Some(matrix), true) = (created.matrix.as_ref(), created.complete) {
        let ncols = matrix.ncols as usize;
        let mut nfixings = 0;
        let mut varstofix = vec![Fixingdirection::NoFix; ncols];

        for i in 0..ncols {
            let var = matrix.vars[i];

            // Exclude compensation variables itself for compensation.
            if !var_is_integral(scip, var) && matrix.col_nnonzs(i) == 1 {
                continue;
            }

            // If requested exclude continuous variables for compensation.
            if componlydisvars && !var_is_integral(scip, var) {
                continue;
            }

            let obj = scip.vars[var].obj;

            // Verifiy that this variable has one uplock and that the uplocks are
            // consistent.
            if var_get_n_locks_up_type(scip, var, LockType::Model) == 1 && matrix.nuplocks[i] == 1 && scip.set.is_le(obj, 0.0) {
                // Search row which causes the uplock.
                let (inspect, twosides, row, val) = find_lock_row(scip, matrix, i, true);

                if inspect {
                    // Try to fix variable i at the upper bound.
                    compensate_var_lock(scip, matrix, i, row, val, twosides, Lockcompensation::CompensateUplock, &mut varstofix, &mut nfixings);
                }
            }
            // Verifiy that this variable has one downlock and that the downlocks
            // are consistent.
            else if var_get_n_locks_down_type(scip, var, LockType::Model) == 1 && matrix.ndownlocks[i] == 1 && scip.set.is_ge(obj, 0.0) {
                // Search row which causes the downlock.
                let (inspect, twosides, row, val) = find_lock_row(scip, matrix, i, false);

                if inspect {
                    // Try to fix variable i at the lower bound.
                    compensate_var_lock(scip, matrix, i, row, val, twosides, Lockcompensation::CompensateDownlock, &mut varstofix, &mut nfixings);
                }
            }
        }

        if nfixings > 0 {
            let oldnfixedvars = counts.nfixedvars;
            let vars = matrix.vars.clone();

            // Look for fixable variables.
            for v in (0..ncols).rev() {
                let var = vars[v];

                let fixval = match varstofix[v] {
                    // Fix at lower bound (avoiding fixings to infinite values).
                    Fixingdirection::FixAtLb => scip.vars[var].glbdom.lb,
                    // Fix at upper bound.
                    Fixingdirection::FixAtUb => scip.vars[var].glbdom.ub,
                    Fixingdirection::NoFix => continue,
                };

                let (infeasible, _fixed) = scip_var::fix_var(scip, var, fixval)?;
                if infeasible {
                    *result = ScipResultCode::Cutoff;
                    break;
                }
                counts.nfixedvars += 1;
            }

            if *result != ScipResultCode::Cutoff && counts.nfixedvars > oldnfixedvars {
                *result = ScipResultCode::Success;
            }
        }
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `presolFreeDualcomp`: destructor of presolver to free user data (called
/// when SCIP is exiting).
fn presol_free_dualcomp(scip: &mut Scip, presol: PresolId) -> ScipResult {
    // Free presolver data.
    scip.presols[presol].presoldata = None;
    Ok(())
}

/// `presolCopyDualcomp`: copy method for presolver plugins (called when SCIP
/// copies plugins).
fn presol_copy_dualcomp(scip: &mut Scip, _sourcescip: &Scip, _presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver.
    include_presol_dualcomp(scip)?;
    Ok(())
}

/// `SCIPincludePresolDualcomp`: creates the dualcomp presolver and includes
/// it in SCIP.
pub fn include_presol_dualcomp(scip: &mut Scip) -> ScipResult {
    // Create dualcomp presolver data.
    let presoldata = PresolData {
        componlydisvars: DEFAULT_COMP_ONLY_DIS_VARS,
    };

    // Include presolver.
    let presol = api::include_presol_basic(
        scip,
        PRESOL_NAME,
        PRESOL_DESC,
        PRESOL_PRIORITY,
        PRESOL_MAXROUNDS,
        PRESOL_TIMING,
        presol_exec_dualcomp,
        Some(Box::new(presoldata) as Box<dyn Any>),
    )?;
    scip.presols[presol].presolcopy = Some(presol_copy_dualcomp);

    scip.presols[presol].presolfree = Some(presol_free_dualcomp);

    crate::paramset::paramset_add_bool(
        scip,
        "presolving/dualcomp/componlydisvars",
        "should only discrete variables be compensated?",
        crate::data_target!(bool, presols, presol, presoldata, PresolData, componlydisvars),
        false,
        DEFAULT_COMP_ONLY_DIS_VARS,
        None,
    )?;

    Ok(())
}
