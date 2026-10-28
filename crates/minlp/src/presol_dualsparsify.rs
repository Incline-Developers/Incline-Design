//! Port of `presol_dualsparsify.c`: eliminate non-zero coefficients.
//!
//! Cancels non-zeros of a column with a scaled (implied free) column that
//! shares a pair of constraints with it, by substituting a new variable
//! y = weight * x_cancel + x_hashing for the hashing variable
//! (multi-aggregation).

use std::any::Any;

use crate::{
    api,
    cons::*,
    hash::{HashOps, HashTable},
    matrix::{Matrix, matrix_create, matrix_downlock_conflict, matrix_remove_column_bounds, matrix_uplock_conflict},
    retcode::ScipResult,
    scip::Scip,
    scip_var,
    types::{ImplintType, PresolId, ScipResultCode, VarId, VarType},
    var::{var_is_binary, var_is_implied_integral, var_is_integral},
};

const PRESOL_NAME: &str = "dualsparsify";
const PRESOL_DESC: &str = "eliminate non-zero coefficients";

/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = -240000;
/// Maximal number of presolving rounds the presolver participates in (-1: no
/// limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_EXHAUSTIVE;

/// Should dualsparsify presolver be copied to sub-SCIPs?
const DEFAULT_ENABLECOPY: bool = true;
/// Should we forbid cancellations that destroy integer coefficients?
const DEFAULT_PRESERVEINTCOEFS: bool = false;
/// Should we preserve good locked properties of variables (at most one lock
/// in one direction)?
const DEFAULT_PRESERVEGOODLOCKS: bool = false;
/// Default value for the maximal fillin for continuous variables.
const DEFAULT_MAX_CONT_FILLIN: i32 = 1;
/// Default value for the maximal fillin for binary variables.
const DEFAULT_MAX_BIN_FILLIN: i32 = 1;
/// Default value for the maximal fillin for integer variables (including
/// binary).
const DEFAULT_MAX_INT_FILLIN: i32 = 1;
/// Maximal number of considered nonzeros within one column (-1: no limit).
const DEFAULT_MAXCONSIDEREDNONZEROS: i32 = 70;
/// Minimal eleminated nonzeros within one column if we need to add a
/// constraint to the problem.
const DEFAULT_MINELIMINATEDNONZEROS: i32 = 100;
/// Limit on the number of useless vs. useful hashtable retrieves as a
/// multiple of the number of constraints.
const DEFAULT_MAXRETRIEVEFAC: f64 = 100.0;
/// Number of calls to wait until next execution as a multiple of the number
/// of useless calls.
const DEFAULT_WAITINGFAC: f64 = 2.0;

/// Maximal allowed scale for cancelling nonzeros.
const MAXSCALE: f64 = 1000.0;

/// Presolver data.
#[derive(Default)]
struct PresolData {
    nfailures: i32,
    nwaitingcalls: i32,
    naggregated: i32,
    maxcontfillin: i32,
    maxintfillin: i32,
    maxbinfillin: i32,
    maxconsiderednonzeros: i32,
    mineliminatednonzeros: i32,
    maxretrievefac: f64,
    waitingfac: f64,
    enablecopy: bool,
    preserveintcoefs: bool,
    preservegoodlocks: bool,
}

fn pd(scip: &Scip, presol: PresolId) -> &PresolData {
    scip.presols[presol].presoldata.as_ref().unwrap().downcast_ref::<PresolData>().unwrap()
}

fn pdm(scip: &mut Scip, presol: PresolId) -> &mut PresolData {
    scip.presols[presol].presoldata.as_mut().unwrap().downcast_mut::<PresolData>().unwrap()
}

/// `COLCONSPAIR`: a pair of constraints in a column; used for lookup in a
/// hashtable.
#[derive(Clone, Copy, Debug, Default)]
struct ColConsPair {
    colindex: i32,
    consindex1: i32,
    consindex2: i32,
    conscoef1: f64,
    conscoef2: f64,
}

/// The pair table's callbacks: elements are indices into the pair array.
struct PairOps<'a> {
    scip: &'a Scip,
    conspairs: &'a [ColConsPair],
}

impl HashOps<usize> for PairOps<'_> {
    type Key = ColConsPair;

    fn get_key(&self, element: &usize) -> ColConsPair {
        self.conspairs[*element]
    }

    /// `consPairsEqual`: returns TRUE iff both keys are equal.
    fn key_eq(&self, conspair1: &ColConsPair, conspair2: &ColConsPair) -> bool {
        if conspair1.consindex1 != conspair2.consindex1 {
            return false;
        }

        if conspair1.consindex2 != conspair2.consindex2 {
            return false;
        }

        let ratio1 = conspair1.conscoef2 / conspair1.conscoef1;
        let ratio2 = conspair2.conscoef2 / conspair2.conscoef1;

        self.scip.set.is_eq(ratio1, ratio2)
    }

    /// `consPairHashval`: returns the hash value of the key.
    fn key_val(&self, conspair: &ColConsPair) -> u64 {
        crate::misc::hash_three(
            conspair.consindex1 as u32,
            conspair.consindex2 as u32,
            crate::misc::real_hash_code(conspair.conscoef2 / conspair.conscoef1),
        ) as u64
    }
}

/// `getMaxActivitySingleRowWithoutCol`: calculate maximal activity of one
/// row without one specific column.
fn get_max_activity_single_row_without_col(matrix: &Matrix, row: usize, col: i32) -> f64 {
    let mut maxactivity = 0.0;

    for k in matrix.row_range(row) {
        let c = matrix.rowmatind[k];
        let val = matrix.rowmatval[k];

        if c == col {
            continue;
        }

        if val > 0.0 {
            maxactivity += val * matrix.ub[c as usize];
        } else if val < 0.0 {
            maxactivity += val * matrix.lb[c as usize];
        }
    }

    maxactivity
}

/// `getMinActivitySingleRowWithoutCol`: calculate minimal activity of one
/// row without one specific column.
fn get_min_activity_single_row_without_col(matrix: &Matrix, row: usize, col: i32) -> f64 {
    let mut minactivity = 0.0;

    for k in matrix.row_range(row) {
        let c = matrix.rowmatind[k];
        let val = matrix.rowmatval[k];

        if c == col {
            continue;
        }

        if val > 0.0 {
            minactivity += val * matrix.lb[c as usize];
        } else if val < 0.0 {
            minactivity += val * matrix.ub[c as usize];
        }
    }

    minactivity
}

/// `getMinMaxActivityResiduals`: get minimal and maximal residual activity
/// without one specified column; returns (minresactivity, maxresactivity,
/// isminsettoinfinity, ismaxsettoinfinity).
fn get_min_max_activity_residuals(scip: &Scip, matrix: &Matrix, col: usize, row: usize, val: f64) -> (f64, f64, bool, bool) {
    let lb = matrix.lb[col];
    let ub = matrix.ub[col];
    let inf = scip.set.infinity();

    let mut isminsettoinfinity = false;
    let mut ismaxsettoinfinity = false;

    let nmaxactneginf = matrix.maxactivityneginf[row];
    let nmaxactposinf = matrix.maxactivityposinf[row];
    let nminactneginf = matrix.minactivityneginf[row];
    let nminactposinf = matrix.minactivityposinf[row];

    let maxactivity = matrix.maxactivity[row];
    let minactivity = matrix.minactivity[row];

    let maxresactivity;
    let minresactivity;
    let c = col as i32;

    if val >= 0.0 {
        if scip.set.is_infinity(ub) {
            if nmaxactposinf == 1 && nmaxactneginf == 0 {
                maxresactivity = get_max_activity_single_row_without_col(matrix, row, c);
            } else {
                maxresactivity = inf;
                ismaxsettoinfinity = true;
            }
        } else if (nmaxactneginf + nmaxactposinf) > 0 {
            maxresactivity = inf;
            ismaxsettoinfinity = true;
        } else {
            maxresactivity = maxactivity - val * ub;
        }

        if scip.set.is_infinity(-lb) {
            if nminactneginf == 1 && nminactposinf == 0 {
                minresactivity = get_min_activity_single_row_without_col(matrix, row, c);
            } else {
                minresactivity = -inf;
                isminsettoinfinity = true;
            }
        } else if (nminactneginf + nminactposinf) > 0 {
            minresactivity = -inf;
            isminsettoinfinity = true;
        } else {
            minresactivity = minactivity - val * lb;
        }
    } else {
        if scip.set.is_infinity(-lb) {
            if nmaxactneginf == 1 && nmaxactposinf == 0 {
                maxresactivity = get_max_activity_single_row_without_col(matrix, row, c);
            } else {
                maxresactivity = inf;
                ismaxsettoinfinity = true;
            }
        } else if (nmaxactneginf + nmaxactposinf) > 0 {
            maxresactivity = inf;
            ismaxsettoinfinity = true;
        } else {
            maxresactivity = maxactivity - val * lb;
        }

        if scip.set.is_infinity(ub) {
            if nminactposinf == 1 && nminactneginf == 0 {
                minresactivity = get_min_activity_single_row_without_col(matrix, row, c);
            } else {
                minresactivity = -inf;
                isminsettoinfinity = true;
            }
        } else if (nminactneginf + nminactposinf) > 0 {
            minresactivity = -inf;
            isminsettoinfinity = true;
        } else {
            minresactivity = minactivity - val * ub;
        }
    }

    (minresactivity, maxresactivity, isminsettoinfinity, ismaxsettoinfinity)
}

/// `getVarBoundsOfRow`: calculate the upper and lower bound of one variable
/// from one row; returns (rowub, ubfound, rowlb, lbfound).
fn get_var_bounds_of_row(scip: &Scip, matrix: &Matrix, col: usize, row: usize, val: f64) -> (f64, bool, f64, bool) {
    let mut rowub = scip.set.infinity();
    let mut ubfound = false;
    let mut rowlb = -scip.set.infinity();
    let mut lbfound = false;

    let (minresactivity, maxresactivity, isminsettoinfinity, ismaxsettoinfinity) = get_min_max_activity_residuals(scip, matrix, col, row, val);

    let lhs = matrix.lhs[row];
    let rhs = matrix.rhs[row];

    if val > 0.0 {
        if !isminsettoinfinity && !scip.set.is_infinity(rhs) {
            rowub = (rhs - minresactivity) / val;
            ubfound = true;
        }

        if !ismaxsettoinfinity && !scip.set.is_infinity(-lhs) {
            rowlb = (lhs - maxresactivity) / val;
            lbfound = true;
        }
    } else {
        if !ismaxsettoinfinity && !scip.set.is_infinity(-lhs) {
            rowub = (lhs - maxresactivity) / val;
            ubfound = true;
        }

        if !isminsettoinfinity && !scip.set.is_infinity(rhs) {
            rowlb = (rhs - minresactivity) / val;
            lbfound = true;
        }
    }

    (rowub, ubfound, rowlb, lbfound)
}

/// `getImpliedBounds`: detect implied variable bounds; returns (ubimplied,
/// lbimplied).
fn get_implied_bounds(scip: &Scip, matrix: &Matrix, col: usize) -> (bool, bool) {
    let mut impliedub = scip.set.infinity();
    let mut impliedlb = -scip.set.infinity();

    let ub = matrix.ub[col];
    let lb = matrix.lb[col];

    for k in matrix.col_range(col) {
        let (rowub, ubfound, rowlb, lbfound) = get_var_bounds_of_row(scip, matrix, col, matrix.colmatind[k] as usize, matrix.colmatval[k]);

        if ubfound && rowub < impliedub {
            impliedub = rowub;
        }

        if lbfound && rowlb > impliedlb {
            impliedlb = rowlb;
        }
    }

    // We consider +/-inf bounds as implied bounds.
    let ubimplied = scip.set.is_infinity(ub) || (!scip.set.is_infinity(ub) && scip.set.is_le(impliedub, ub));
    let lbimplied = scip.set.is_infinity(-lb) || (!scip.set.is_infinity(-lb) && scip.set.is_ge(impliedlb, lb));

    (ubimplied, lbimplied)
}

/// `aggregation`: y = weight1 * var[colidx1] + var[colidx2].
#[allow(clippy::too_many_arguments)]
fn aggregation(scip: &mut Scip, matrix: &mut Matrix, presol: PresolId, vars: &mut [VarId], colidx1: usize, colidx2: usize, isimpliedfree: bool, weight1: f64) -> ScipResult {
    pdm(scip, presol).naggregated += 1;
    let naggregated = pd(scip, presol).naggregated;
    let aggregatedvar = vars[colidx2];

    // If the variable is implied free, we make sure that the columns bounds
    // are removed, so that subsequent checks for implied bounds do not
    // interfere with the exploitation of this variables implied bounds.
    if isimpliedfree {
        matrix_remove_column_bounds(scip, matrix, colidx2);
    }

    let newvarname = format!("dualsparsifyvar_{naggregated}");

    let constant = 0.0;

    let inf = scip.set.infinity();
    let (lb1, ub1) = (scip.vars[vars[colidx1]].glbdom.lb, scip.vars[vars[colidx1]].glbdom.ub);
    let (lb2, ub2) = (scip.vars[vars[colidx2]].glbdom.lb, scip.vars[vars[colidx2]].glbdom.ub);
    let newlb;
    let newub;
    if weight1 > 0.0 {
        newlb = if scip.set.is_infinity(-lb1) || scip.set.is_infinity(-lb2) {
            -inf
        } else {
            weight1 * lb1 + lb2
        };
        newub = if scip.set.is_infinity(ub1) || scip.set.is_infinity(ub2) {
            inf
        } else {
            weight1 * ub1 + ub2
        };
    } else {
        newlb = if scip.set.is_infinity(ub1) || scip.set.is_infinity(-lb2) {
            -inf
        } else {
            weight1 * ub1 + lb2
        };
        newub = if scip.set.is_infinity(lb1) || scip.set.is_infinity(ub2) {
            inf
        } else {
            weight1 * lb1 + ub2
        };
    }

    let newvartype = if var_is_integral(scip, aggregatedvar) { VarType::Integer } else { VarType::Continuous };
    let newvarimpltype = if var_is_implied_integral(scip, aggregatedvar) {
        ImplintType::Weak
    } else {
        ImplintType::None
    };

    let lhs = lb2;
    let rhs = ub2;

    let (initial, removable) = (scip.vars[aggregatedvar].initial, scip.vars[aggregatedvar].removable);
    let mut newvar = scip_var::create_var_impl(scip, &newvarname, newlb, newub, 0.0, newvartype, newvarimpltype, initial, removable)?;
    scip_var::add_var(scip, newvar)?;

    let tmpvars = [vars[colidx1], newvar];
    let coefs = [-weight1, 1.0];

    let (_infeasible, _aggregated) = scip_var::multiaggregate_var(scip, aggregatedvar, &tmpvars, &coefs, constant)?;

    vars[colidx2] = newvar;

    // Create a linear constraint that ensures that var[colidx2].lb <= y -
    // weight1 * var[colidx1] <= var[colidx2].ub; note that it might happen
    // that vars[colidx2] is not implied free even though it has infinite
    // bounds because getImpliedBounds() considers infinite bounds to be
    // implied.
    if !isimpliedfree && (!scip.set.is_infinity(rhs) || !scip.set.is_infinity(-lhs)) {
        let newconsname = format!("dualsparsifycons_{naggregated}");

        let mut newcons = crate::cons_linear::create_cons_linear(
            scip,
            &newconsname,
            &tmpvars,
            &coefs,
            lhs,
            rhs,
            true,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
        )?;
        crate::scip_cons::add_cons(scip, newcons)?;

        crate::scip_cons::release_cons(scip, &mut newcons)?;
    }

    api::release_var(scip, &mut newvar)?;

    Ok(())
}

/// Counters `cancelCol` updates.
struct CancelCounts<'a> {
    nuseless: &'a mut i64,
    nchgcoefs: &'a mut i32,
    ncanceled: &'a mut i32,
    nfillin: &'a mut i32,
}

/// `cancelCol`: try nonzero cancellation for given column.
#[allow(clippy::too_many_arguments)]
fn cancel_col(
    scip: &mut Scip,
    matrix: &mut Matrix,
    presol: PresolId,
    pairtable: &HashTable<usize>,
    conspairs: &[ColConsPair],
    ishashingcols: &mut [bool],
    vars: &mut [VarId],
    isblockedvar: &mut [bool],
    colidx: usize,
    maxcontfillin: i32,
    maxintfillin: i32,
    maxbinfillin: i32,
    maxconsiderednonzeros: i32,
    preserveintcoefs: bool,
    counts: CancelCounts,
    isaddedcons: bool,
) -> ScipResult {
    let (preservegoodlocks, mineliminatednonzeros) = {
        let p = pd(scip, presol);
        (p.preservegoodlocks, p.mineliminatednonzeros)
    };
    let mut swapped = false;

    let ncols = matrix.ncols as f64;
    let colishashing = ishashingcols[colidx];
    let mut cancelcollen = matrix.col_nnonzs(colidx) as usize;
    let cancelvar = vars[colidx];

    let maxfillin = if var_is_integral(scip, cancelvar) {
        if var_is_binary(scip, cancelvar) { maxbinfillin } else { maxintfillin }
    } else {
        maxcontfillin
    };

    let r = matrix.col_range(colidx);
    let mut cancelcolinds: Vec<i32> = matrix.colmatind[r.clone()].to_vec();
    let mut cancelcolvals: Vec<f64> = matrix.colmatval[r].to_vec();
    let mut tmpinds = vec![0i32; cancelcollen];
    let mut tmpvals = vec![0.0f64; cancelcollen];
    let mut scores = vec![0.0f64; cancelcollen];

    let mut nchgcoef = 0;
    let mut nretrieves: i64 = 0;
    loop {
        let mut bestcand: i32 = -1;
        let mut bestnfillin = 0;
        let mut bestscale = 1.0;
        let mut bestcancelrate = 0.0;

        // Sort the rows non-decreasingly by number of nonzeros; if the number
        // of nonzeros, we use the colindex as tie-breaker.
        for i in 0..cancelcollen {
            tmpinds[i] = i as i32;
            scores[i] = -matrix.row_nnonzs(cancelcolinds[i] as usize) as f64 - 1.0 * cancelcolinds[i] as f64 / ncols;
        }
        crate::sort::sort(&mut scores[..cancelcollen], (&mut tmpinds[..cancelcollen],), crate::sort::cmp_real);

        let mut maxlen = cancelcollen;
        if maxconsiderednonzeros >= 0 {
            maxlen = crate::def::min(cancelcollen, maxconsiderednonzeros as usize);
        }

        for i in 0..maxlen {
            // C's `break`s below leave only this inner loop.
            'jloop: for j in (i + 1)..maxlen {
                let i1 = tmpinds[i] as usize;
                let i2 = tmpinds[j] as usize;

                let colconspair = if cancelcolinds[i1] < cancelcolinds[i2] {
                    ColConsPair {
                        colindex: 0,
                        consindex1: cancelcolinds[i1],
                        consindex2: cancelcolinds[i2],
                        conscoef1: cancelcolvals[i1],
                        conscoef2: cancelcolvals[i2],
                    }
                } else {
                    ColConsPair {
                        colindex: 0,
                        consindex1: cancelcolinds[i2],
                        consindex2: cancelcolinds[i1],
                        conscoef1: cancelcolvals[i2],
                        conscoef2: cancelcolvals[i1],
                    }
                };

                let hashing = pairtable.retrieve(&PairOps { scip, conspairs }, &colconspair);
                nretrieves += 1;

                let Some(hashing) = hashing else { continue };
                let hashingcolconspair = conspairs[hashing];
                let hcol = hashingcolconspair.colindex as usize;
                if hcol == colidx || isblockedvar[hcol] {
                    continue;
                }

                // If the column we want to cancel is a hashing column (which
                // we stored for canceling other columns), we will only use the
                // hashing columns for canceling with less nonzeros and if the
                // number of nonzeros is equal we use the colindex as
                // tie-breaker to avoid cyclic nonzero cancellation.
                let hashingcollen = matrix.col_nnonzs(hcol) as usize;
                if colishashing && (cancelcollen < hashingcollen || (cancelcollen == hashingcollen && colidx < hcol)) {
                    continue;
                }

                let hr = matrix.col_range(hcol);
                let hashingcolvals = &matrix.colmatval[hr.clone()];
                let hashingcolinds = &matrix.colmatind[hr];
                let hashingcolvar = vars[hcol];
                let hashingcolisbin = var_is_binary(scip, hashingcolvar);

                // @todo do more reduction if knapsack constraint handler
                // supports downgrading constraint, i.e., converting into a
                // linear constraint.
                if hashingcolisbin {
                    continue;
                }

                let mut scale = -colconspair.conscoef1 / hashingcolconspair.conscoef1;

                if scale.abs() > MAXSCALE {
                    continue;
                }

                if !scip_var::is_var_aggr_coef_acceptable(scip, hashingcolvar, scale) {
                    continue;
                }

                if var_is_integral(scip, hashingcolvar) {
                    if var_is_integral(scip, cancelvar) {
                        // Skip if the hashing variable is an integer variable
                        // and the canceled variable is an implicit integer
                        // variable.
                        if !var_is_implied_integral(scip, hashingcolvar) && var_is_implied_integral(scip, cancelvar) {
                            continue;
                        }

                        // Skip if the scale is non-integral.
                        if !scip.set.is_integral(scale) {
                            continue;
                        }

                        // Round scale to be exactly integral.
                        scale = (scale + 0.5).floor();
                    }
                    // Skip if the canceled variable is a continuous variable.
                    else {
                        continue;
                    }
                }

                let mut a = 0;
                let mut b = 0;
                let mut ncancel = 0;
                let mut ntotfillin = 0;
                let mut abortpair = false;

                while a < cancelcollen && b < hashingcollen {
                    if cancelcolinds[a] == hashingcolinds[b] {
                        let newcoef = cancelcolvals[a] + scale * hashingcolvals[b];

                        // Check if coefficient is canceled.
                        if scip.set.is_zero(newcoef) {
                            ncancel += 1;
                        }
                        // Otherwise, check if integral coefficients are
                        // preserved if the column is integral.
                        else if preserveintcoefs && var_is_integral(scip, cancelvar) && scip.set.is_integral(cancelcolvals[a]) && !scip.set.is_integral(newcoef) {
                            abortpair = true;
                            break;
                        }
                        // Finally, check if locks could be modified in a bad
                        // way due to flipped signs.
                        else if 1.0f64.copysign(newcoef) != 1.0f64.copysign(cancelcolvals[a]) {
                            // Do not flip signs for non-canceled coefficients
                            // if this adds a lock to a variable that had at
                            // most one lock in that direction before, except
                            // if the other direction gets unlocked.
                            let rowrhs = matrix.rhs[cancelcolinds[a] as usize];
                            let rowlhs = matrix.lhs[cancelcolinds[a] as usize];
                            if ((cancelcolvals[a] > 0.0 && !scip.set.is_infinity(rowrhs)) || (cancelcolvals[a] < 0.0 && !scip.set.is_infinity(-rowlhs)))
                                && preservegoodlocks
                                && (matrix.nuplocks[colidx] > 1 && matrix.ndownlocks[colidx] <= 1)
                            {
                                abortpair = true;
                                break;
                            }

                            if ((cancelcolvals[a] < 0.0 && !scip.set.is_infinity(rowrhs)) || (cancelcolvals[a] > 0.0 && !scip.set.is_infinity(-rowlhs)))
                                && preservegoodlocks
                                && (matrix.ndownlocks[colidx] > 1 && matrix.nuplocks[colidx] <= 1)
                            {
                                abortpair = true;
                                break;
                            }
                        }

                        a += 1;
                        b += 1;
                    } else if cancelcolinds[a] < hashingcolinds[b] {
                        a += 1;
                    } else {
                        let newcoef = scale * hashingcolvals[b];
                        let rowrhs = matrix.rhs[hashingcolinds[b] as usize];
                        let rowlhs = matrix.lhs[hashingcolinds[b] as usize];

                        if ((newcoef > 0.0 && !scip.set.is_infinity(rowrhs)) || (newcoef < 0.0 && !scip.set.is_infinity(-rowlhs)))
                            && preservegoodlocks
                            && matrix.nuplocks[colidx] <= 1
                        {
                            abortpair = true;
                            b += 1;
                            break;
                        }

                        if ((newcoef < 0.0 && !scip.set.is_infinity(rowrhs)) || (newcoef > 0.0 && !scip.set.is_infinity(-rowlhs)))
                            && preservegoodlocks
                            && matrix.ndownlocks[colidx] <= 1
                        {
                            abortpair = true;
                            b += 1;
                            break;
                        }

                        b += 1;

                        ntotfillin += 1;
                        if ntotfillin > maxfillin {
                            abortpair = true;
                            break;
                        }
                    }
                }

                if abortpair {
                    continue;
                }

                while b < hashingcollen {
                    b += 1;

                    ntotfillin += 1;
                    if ntotfillin > maxfillin {
                        break;
                    }
                }

                // CHECKFILLINAGAIN:
                loop {
                    if ntotfillin > maxfillin || ntotfillin >= ncancel {
                        break;
                    }

                    let cancelrate = (ncancel - ntotfillin) as f64 / cancelcollen as f64;

                    // If a linear constraint is needed to keep the validity,
                    // we require a large nonzero cancellation.
                    if isaddedcons && (ncancel - ntotfillin < mineliminatednonzeros) {
                        break;
                    }

                    if cancelrate > bestcancelrate {
                        if ishashingcols[hcol] {
                            // Recompute whether a variable is still implied
                            // free; after some previous multi-aggregations of
                            // some variables, it might be that other
                            // variables that are contained in the same linear
                            // rows of the matrix are not implied free anymore
                            // (see #2971).
                            let (ubimplied, lbimplied) = get_implied_bounds(scip, matrix, hcol);

                            if !lbimplied || !ubimplied {
                                ishashingcols[hcol] = false;
                                ntotfillin += 2;
                                continue;
                            }
                        }

                        bestnfillin = ntotfillin;
                        bestcand = hcol as i32;
                        bestscale = scale;
                        bestcancelrate = cancelrate;

                        // Stop looking if the current candidate does not
                        // create any fill-in or alter coefficients.
                        if cancelrate == 1.0 {
                            break 'jloop;
                        }
                    }

                    // We accept the best candidate immediately if it does not
                    // create any fill-in or alter coefficients.
                    if bestcand != -1 && bestcancelrate == 1.0 {
                        break 'jloop;
                    }
                    break;
                }
            }
        }

        if bestcand != -1 {
            let bc = bestcand as usize;
            let hr = matrix.col_range(bc);
            let hashingcolvals = matrix.colmatval[hr.clone()].to_vec();
            let hashingcolinds = matrix.colmatind[hr].to_vec();
            let hashingcollen = hashingcolinds.len();

            let mut a = 0;
            let mut b = 0;
            let mut tmpcollen = 0;

            // The merged column may be longer than the canceled one.
            if tmpinds.len() < cancelcollen + hashingcollen {
                tmpinds.resize(cancelcollen + hashingcollen, 0);
                tmpvals.resize(cancelcollen + hashingcollen, 0.0);
            }

            while a < cancelcollen && b < hashingcollen {
                if cancelcolinds[a] == hashingcolinds[b] {
                    let val = cancelcolvals[a] + bestscale * hashingcolvals[b];

                    if !scip.set.is_zero(val) {
                        tmpinds[tmpcollen] = cancelcolinds[a];
                        tmpvals[tmpcollen] = val;
                        tmpcollen += 1;
                    }
                    nchgcoef += 1;

                    a += 1;
                    b += 1;
                } else if cancelcolinds[a] < hashingcolinds[b] {
                    tmpinds[tmpcollen] = cancelcolinds[a];
                    tmpvals[tmpcollen] = cancelcolvals[a];
                    tmpcollen += 1;
                    a += 1;
                } else {
                    tmpinds[tmpcollen] = hashingcolinds[b];
                    tmpvals[tmpcollen] = hashingcolvals[b] * bestscale;
                    nchgcoef += 1;
                    tmpcollen += 1;
                    b += 1;
                }
            }

            while a < cancelcollen {
                tmpinds[tmpcollen] = cancelcolinds[a];
                tmpvals[tmpcollen] = cancelcolvals[a];
                tmpcollen += 1;
                a += 1;
            }

            while b < hashingcollen {
                tmpinds[tmpcollen] = hashingcolinds[b];
                tmpvals[tmpcollen] = hashingcolvals[b] * bestscale;
                nchgcoef += 1;
                tmpcollen += 1;
                b += 1;
            }

            // Update fill-in counter.
            *counts.nfillin += bestnfillin;

            // Swap the temporary arrays so that the cancelcolinds and
            // cancelcolvals arrays contain the new changed column, and the
            // tmpinds and tmpvals arrays can be overwritten in the next
            // iteration.
            std::mem::swap(&mut tmpinds, &mut cancelcolinds);
            std::mem::swap(&mut tmpvals, &mut cancelcolvals);
            swapped = !swapped;
            cancelcollen = tmpcollen;
            if scores.len() < cancelcollen {
                scores.resize(cancelcollen, 0.0);
            }
            if tmpinds.len() < cancelcollen {
                tmpinds.resize(cancelcollen, 0);
                tmpvals.resize(cancelcollen, 0.0);
            }
            let isimpliedfree = ishashingcols[bc];
            aggregation(scip, matrix, presol, vars, colidx, bc, isimpliedfree, -bestscale)?;

            // The newly created variable is now at the position bestcand and
            // is assumed to have the same coefficients. This is not the case
            // if the variable is not implied free since then a new constraint
            // was added and the nonzero fillin would not be counted correctly
            // if we do not block this variable.
            if !ishashingcols[bc] {
                isblockedvar[bc] = true;
            }
        } else {
            break;
        }
    }
    let _ = swapped;

    if nchgcoef != 0 {
        // Update counters.
        *counts.nchgcoefs += nchgcoef;
        *counts.ncanceled += matrix.col_nnonzs(colidx) - cancelcollen as i32;

        isblockedvar[colidx] = true;

        // If successful, decrease the useless hashtable retrieves counter.
        *counts.nuseless -= nretrieves;
        *counts.nuseless = crate::def::max(*counts.nuseless, 0);
    } else {
        // If not successful, increase useless hashtable retrieves counter.
        *counts.nuseless += nretrieves;
    }

    Ok(())
}

/// `updateFailureStatistic`: updates failure counter after one execution.
fn update_failure_statistic(presoldata: &mut PresolData, success: bool) {
    if success {
        presoldata.nfailures = 0;
        presoldata.nwaitingcalls = 0;
    } else {
        presoldata.nfailures += 1;
        presoldata.nwaitingcalls = (presoldata.waitingfac * presoldata.nfailures as f64) as i32;
    }
}

/*
 * Callback methods of presolver
 */

/// `presolCopyDualsparsify`: copy method for presolver plugins.
fn presol_copy_dualsparsify(scip: &mut Scip, sourcescip: &Scip, presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver if copying is enabled.
    if pd(sourcescip, presol).enablecopy {
        include_presol_dualsparsify(scip)?;
    }
    Ok(())
}

/// Collects the constraint pairs of column `c` (`nnonz` considered
/// non-zeros, sorted by row length), shifted by `failshift`.
fn collect_conspairs(
    scip: &Scip,
    matrix: &Matrix,
    c: usize,
    mut nnonz: usize,
    maxconsiderednonzeros: i32,
    failshift: i32,
    scores: &mut [f64],
    perm: &mut [i32],
    conspairs: &mut Vec<ColConsPair>,
) {
    let cr = matrix.col_range(c);
    let colinds = &matrix.colmatind[cr.clone()];
    let colvals = &matrix.colmatval[cr];
    let ncols = matrix.ncols;

    // Sort the rows non-decreasingly by number of nonzeros; if the number of
    // nonzeros is equal, we use the colindex as tie-breaker.
    for i in 0..nnonz {
        perm[i] = i as i32;
        scores[i] = -matrix.row_nnonzs(colinds[i] as usize) as f64 - 1.0 * colinds[i] as f64 / ncols as f64;
    }
    crate::sort::sort(&mut scores[..nnonz], (&mut perm[..nnonz],), crate::sort::cmp_real);

    if maxconsiderednonzeros >= 0 {
        nnonz = crate::def::min(nnonz, maxconsiderednonzeros as usize);
    }

    let _ = scip;
    for i in 0..nnonz {
        for j in (i + 1)..nnonz {
            let i1 = perm[((i as i32 + failshift) % nnonz as i32) as usize] as usize;
            let i2 = perm[((j as i32 + failshift) % nnonz as i32) as usize] as usize;

            let pair = if colinds[i1] < colinds[i2] {
                ColConsPair {
                    colindex: c as i32,
                    consindex1: colinds[i1],
                    consindex2: colinds[i2],
                    conscoef1: colvals[i1],
                    conscoef2: colvals[i2],
                }
            } else {
                ColConsPair {
                    colindex: c as i32,
                    consindex1: colinds[i2],
                    consindex2: colinds[i1],
                    conscoef1: colvals[i2],
                    conscoef2: colvals[i1],
                }
            };
            conspairs.push(pair);
        }
    }
}

/// Inserts the constraint pairs into the hash table, keeping for equal keys
/// the pair whose column has fewer non-zeros.
fn insert_conspairs(scip: &Scip, matrix: &Matrix, pairtable: &mut HashTable<usize>, conspairs: &[ColConsPair]) {
    let ops = PairOps { scip, conspairs };
    for c in 0..conspairs.len() {
        let mut insert = true;

        // Check if this pair is already contained in the hash table; the loop
        // is required due to the non-transitivity of the hash functions.
        while let Some(other) = pairtable.retrieve(&ops, &conspairs[c]) {
            // If the previous constraint pair has fewer or the same number of
            // nonzeros in the attached column we keep that pair and skip this
            // one.
            if matrix.col_nnonzs(conspairs[other].colindex as usize) <= matrix.col_nnonzs(conspairs[c].colindex as usize) {
                insert = false;
                break;
            }

            // This pairs column has fewer nonzeros, so remove the other pair
            // from the hash table and loop.
            pairtable.remove(&ops, &other);
        }

        if insert {
            pairtable.insert(&ops, c);
        }
    }
}

/// `presolExecDualsparsify`: execution method of presolver.
fn presol_exec_dualsparsify(
    scip: &mut Scip,
    presol: PresolId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if scip_var::do_not_aggr(scip) {
        return Ok(());
    }

    // If restart is performed, some cuts will be tranformed into linear
    // constraints. However, SCIPmatrixCreate() only collects the original
    // constraints (not the constraints transformed from cuts). For this
    // reason, we only perform this method in the first run of
    // branch-and-cut.
    if api::get_n_runs(scip) > 1 {
        return Ok(());
    }

    if pd(scip, presol).nwaitingcalls > 0 {
        pdm(scip, presol).nwaitingcalls -= 1;
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let created = matrix_create(
        scip,
        true,
        &mut counts.naddconss,
        &mut counts.ndelconss,
        &mut counts.nchgcoefs,
        &mut counts.nchgbds,
        &mut counts.nfixedvars,
    )?;

    // If infeasibility was detected during matrix creation or matrix creation
    // is incomplete, return here.
    if created.infeasible || !created.complete {
        if created.infeasible {
            *result = ScipResultCode::Cutoff;
        }
        return Ok(());
    }

    let Some(mut matrix) = created.matrix else { return Ok(()) };

    let ncols = matrix.ncols as usize;
    let nrows = matrix.nrows as usize;

    // Sort columns by row indices.
    for i in 0..ncols {
        let r = matrix.col_range(i);
        crate::sort::sort(&mut matrix.colmatind[r.clone()], (&mut matrix.colmatval[r],), crate::sort::cmp_int);
    }

    let mut scores = vec![0.0f64; nrows];
    let mut perm = vec![0i32; nrows];
    let mut ishashingcols = vec![false; ncols];
    let mut vars = vec![crate::var::NONE; ncols];
    let mut isblockedvar = vec![false; ncols];

    let (maxconsiderednonzeros, nfailures, maxretrievefac, mineliminatednonzeros, preserveintcoefs, maxcontfillin, maxintfillin, maxbinfillin) = {
        let p = pd(scip, presol);
        let unlimited = |v: i32| if v == -1 { i32::MAX } else { v };
        (
            p.maxconsiderednonzeros,
            p.nfailures,
            p.maxretrievefac,
            p.mineliminatednonzeros,
            p.preserveintcoefs,
            unlimited(p.maxcontfillin),
            unlimited(p.maxintfillin),
            unlimited(p.maxbinfillin),
        )
    };

    // Loop over all columns and create cons pairs.
    let mut conspairs: Vec<ColConsPair> = Vec::new();
    let mut pairtable: HashTable<usize> = HashTable::new(1);

    // If we are called after one or more failures, i.e., executions without
    // finding cancellations, then we shift the section of nonzeros
    // considered.
    let failshift = nfailures.wrapping_mul(maxconsiderednonzeros);

    // Collect implied free variables and their number of nonzeros.
    for c in 0..ncols {
        vars[c] = matrix.vars[c];

        // If the locks do not match do not consider the column for
        // sparsification.
        if matrix_downlock_conflict(scip, &matrix, c) || matrix_uplock_conflict(scip, &matrix, c) {
            isblockedvar[c] = true;
            ishashingcols[c] = false;
            continue;
        }

        // Skip if the variable is not allowed to be multi-aggregated.
        if scip_var::do_not_multaggr_var(scip, vars[c]) {
            isblockedvar[c] = true;
            ishashingcols[c] = false;
            continue;
        }

        let nnonz = matrix.col_nnonzs(c) as usize;

        let (ubimplied, lbimplied) = get_implied_bounds(scip, &matrix, c);

        ishashingcols[c] = lbimplied && ubimplied;

        isblockedvar[c] = false;

        // Only consider implied free variables; skip singleton variables,
        // because either the constraint is redundant or the variables can be
        // canceled by variable substitution.
        if nnonz >= 2 && (lbimplied && ubimplied) {
            collect_conspairs(scip, &matrix, c, nnonz, maxconsiderednonzeros, failshift, &mut scores, &mut perm, &mut conspairs);
        }
    }

    // Insert conspairs into hash table.
    insert_conspairs(scip, &matrix, &mut pairtable, &conspairs);

    // Sort cols according to decreasing sparsity.
    let mut colidxsorted: Vec<i32> = (0..ncols as i32).collect();
    let mut colsparsity: Vec<i32> = (0..ncols).map(|c| -matrix.col_nnonzs(c)).collect();
    crate::sort::sort(&mut colsparsity[..], (&mut colidxsorted[..],), crate::sort::cmp_int);

    // Loop over the columns and cancel nonzeros until maximum number of
    // retrieves is reached.
    let mut maxuseless = (maxretrievefac * ncols as f64) as i64;
    let mut nuseless: i64 = 0;
    let mut numcancel = 0;
    let mut nfillin = 0;
    let mut c = 0;
    while c < ncols && nuseless <= maxuseless && !crate::solve::solve_is_stopped(scip, false) {
        let colidx = colidxsorted[c] as usize;
        c += 1;

        if isblockedvar[colidx] {
            continue;
        }

        let cc = CancelCounts {
            nuseless: &mut nuseless,
            nchgcoefs: &mut counts.nchgcoefs,
            ncanceled: &mut numcancel,
            nfillin: &mut nfillin,
        };
        cancel_col(
            scip,
            &mut matrix,
            presol,
            &pairtable,
            &conspairs,
            &mut ishashingcols,
            &mut vars,
            &mut isblockedvar,
            colidx,
            maxcontfillin,
            maxintfillin,
            maxbinfillin,
            maxconsiderednonzeros,
            preserveintcoefs,
            cc,
            false,
        )?;
    }

    if numcancel > 0 {
        *result = ScipResultCode::Success;
    } else {
        // Do reductions on variables that contain larger nonzero entries.
        pairtable.remove_all();
        conspairs.clear();

        // Collect large nonzero entries variables and their number of
        // nonzeros.
        for c in 0..ncols {
            let nnonz = matrix.col_nnonzs(c) as usize;
            vars[c] = matrix.vars[c];

            // If the locks do not match do not consider the column for
            // sparsification; also skip if the variable is not allowed to be
            // multi-aggregated.
            if matrix_downlock_conflict(scip, &matrix, c) || matrix_uplock_conflict(scip, &matrix, c) || scip_var::do_not_multaggr_var(scip, vars[c]) {
                isblockedvar[c] = true;
                ishashingcols[c] = false;
                continue;
            }

            isblockedvar[c] = false;

            // Only consider nonimplied free variables, i.e., non-hashing
            // columns in the previous step, with large nonzero entries.
            if nnonz as i32 >= mineliminatednonzeros && !ishashingcols[c] {
                ishashingcols[c] = true;
                collect_conspairs(scip, &matrix, c, nnonz, maxconsiderednonzeros, failshift, &mut scores, &mut perm, &mut conspairs);
            } else {
                ishashingcols[c] = false;
            }
        }

        // Insert conspairs into hash table.
        insert_conspairs(scip, &matrix, &mut pairtable, &conspairs);

        // Sort rows according to decreasingly sparsity.
        for c in 0..ncols {
            colidxsorted[c] = c as i32;
            colsparsity[c] = -matrix.col_nnonzs(c);
        }
        crate::sort::sort(&mut colsparsity[..], (&mut colidxsorted[..],), crate::sort::cmp_int);

        // Loop over the columns and cancel nonzeros until maximum number of
        // retrieves is reached.
        maxuseless = (maxretrievefac * ncols as f64) as i64;
        nuseless = 0;
        let mut c = 0;
        while c < ncols && nuseless <= maxuseless {
            let colidx = colidxsorted[c] as usize;
            c += 1;
            let nnonz = matrix.col_nnonzs(colidx);

            if isblockedvar[colidx] || nnonz < mineliminatednonzeros {
                continue;
            }

            let cc = CancelCounts {
                nuseless: &mut nuseless,
                nchgcoefs: &mut counts.nchgcoefs,
                ncanceled: &mut numcancel,
                nfillin: &mut nfillin,
            };
            cancel_col(
                scip,
                &mut matrix,
                presol,
                &pairtable,
                &conspairs,
                &mut ishashingcols,
                &mut vars,
                &mut isblockedvar,
                colidx,
                maxcontfillin,
                maxintfillin,
                maxbinfillin,
                maxconsiderednonzeros,
                preserveintcoefs,
                cc,
                true,
            )?;
        }

        if numcancel > 0 {
            *result = ScipResultCode::Success;
        }
    }

    update_failure_statistic(pdm(scip, presol), numcancel > 0);

    Ok(())
}

/// `presolInitDualsparsify`: initialization method of presolver.
fn presol_init_dualsparsify(scip: &mut Scip, presol: PresolId) -> ScipResult {
    // Set the counters in the init (and not in the initpre) callback such
    // that they persist across restarts.
    let p = pdm(scip, presol);
    p.nfailures = 0;
    p.nwaitingcalls = 0;
    p.naggregated = 0;
    Ok(())
}

/// `SCIPincludePresolDualsparsify`: creates the dualsparsify presolver and
/// includes it in SCIP.
pub fn include_presol_dualsparsify(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // Include presolver.
    let presol = api::include_presol_basic(
        scip,
        PRESOL_NAME,
        PRESOL_DESC,
        PRESOL_PRIORITY,
        PRESOL_MAXROUNDS,
        PRESOL_TIMING,
        presol_exec_dualsparsify,
        Some(Box::new(PresolData::default()) as Box<dyn Any>),
    )?;

    scip.presols[presol].presolcopy = Some(presol_copy_dualsparsify);
    scip.presols[presol].presolinit = Some(presol_init_dualsparsify);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, presols, presol, presoldata, PresolData, $f)
        };
    }

    paramset_add_bool(
        scip,
        "presolving/dualsparsify/enablecopy",
        "should dualsparsify presolver be copied to sub-SCIPs?",
        t!(bool, enablecopy),
        true,
        DEFAULT_ENABLECOPY,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/dualsparsify/preserveintcoefs",
        "should we forbid cancellations that destroy integer coefficients?",
        t!(bool, preserveintcoefs),
        true,
        DEFAULT_PRESERVEINTCOEFS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/dualsparsify/preservegoodlocks",
        "should we preserve good locked properties of variables (at most one lock in one direction)?",
        t!(bool, preservegoodlocks),
        true,
        DEFAULT_PRESERVEGOODLOCKS,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/dualsparsify/maxcontfillin",
        "maximal fillin for continuous variables (-1: unlimited)",
        t!(i32, maxcontfillin),
        false,
        DEFAULT_MAX_CONT_FILLIN,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/dualsparsify/maxbinfillin",
        "maximal fillin for binary variables (-1: unlimited)",
        t!(i32, maxbinfillin),
        false,
        DEFAULT_MAX_BIN_FILLIN,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/dualsparsify/maxintfillin",
        "maximal fillin for integer variables including binaries (-1: unlimited)",
        t!(i32, maxintfillin),
        false,
        DEFAULT_MAX_INT_FILLIN,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/dualsparsify/maxconsiderednonzeros",
        "maximal number of considered nonzeros within one column (-1: no limit)",
        t!(i32, maxconsiderednonzeros),
        true,
        DEFAULT_MAXCONSIDEREDNONZEROS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/dualsparsify/mineliminatednonzeros",
        "minimal eliminated nonzeros within one column if we need to add a constraint to the problem",
        t!(i32, mineliminatednonzeros),
        false,
        DEFAULT_MINELIMINATEDNONZEROS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/dualsparsify/maxretrievefac",
        "limit on the number of useless vs. useful hashtable retrieves as a multiple of the number of constraints",
        t!(f64, maxretrievefac),
        true,
        DEFAULT_MAXRETRIEVEFAC,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/dualsparsify/waitingfac",
        "number of calls to wait until next execution as a multiple of the number of useless calls",
        t!(f64, waitingfac),
        true,
        DEFAULT_WAITINGFAC,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;

    Ok(())
}
