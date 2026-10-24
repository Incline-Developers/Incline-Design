//! Port of `presol_sparsify.c`: cancel non-zeros of the constraint matrix
//! based on the linear constraints: add scaled equalities to the other
//! constraints to cancel non-zeros.

use std::any::Any;

use crate::{
    api,
    cons::*,
    hash::{HashOps, HashTable},
    matrix::{Matrix, matrix_create},
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_cons,
    types::{PresolId, ScipResultCode},
    var::{var_is_binary, var_is_integral},
};

const PRESOL_NAME: &str = "sparsify";
const PRESOL_DESC: &str = "eliminate non-zero coefficients";

/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = -24000;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_EXHAUSTIVE;

/// Should sparsify presolver be copied to sub-SCIPs?
const DEFAULT_ENABLECOPY: bool = true;
/// Should we cancel nonzeros in constraints of the linear constraint handler?
const DEFAULT_CANCELLINEAR: bool = true;
/// Should we forbid cancellations that destroy integer coefficients?
const DEFAULT_PRESERVEINTCOEFS: bool = true;
/// Default value for the maximal fillin for continuous variables.
const DEFAULT_MAX_CONT_FILLIN: i32 = 0;
/// Default value for the maximal fillin for binary variables.
const DEFAULT_MAX_BIN_FILLIN: i32 = 0;
/// Default value for the maximal fillin for integer variables (including
/// binary).
const DEFAULT_MAX_INT_FILLIN: i32 = 0;
/// Maximal support of one equality to be used for cancelling (-1: no limit).
const DEFAULT_MAXNONZEROS: i32 = -1;
/// Maximal number of considered non-zeros within one row (-1: no limit).
const DEFAULT_MAXCONSIDEREDNONZEROS: i32 = 70;
/// Order in which to process inequalities ('n'o sorting, 'i'ncreasing
/// nonzeros, 'd'ecreasing nonzeros).
const DEFAULT_ROWSORT: u8 = b'd';
/// Limit on the number of useless vs. useful hashtable retrieves as a
/// multiple of the number of constraints.
const DEFAULT_MAXRETRIEVEFAC: f64 = 100.0;
/// Number of calls to wait until next execution as a multiple of the number
/// of useless calls.
const DEFAULT_WAITINGFAC: f64 = 2.0;
/// Maximal allowed scale for cancelling non-zeros.
const MAXSCALE: f64 = 1000.0;

/*
 * Data structures
 */

/// `SCIP_PresolData`: presolver data.
pub struct PresolData {
    /// Total number of canceled nonzeros (net value, i.e., removed minus added
    /// nonzeros).
    pub ncancels: i32,
    /// Total number of added nonzeros.
    pub nfillin: i32,
    /// Number of calls to presolver without success.
    pub nfailures: i32,
    /// Number of presolver calls until next real execution.
    pub nwaitingcalls: i32,
    /// Maximal fillin for continuous variables.
    pub maxcontfillin: i32,
    /// Maximal fillin for integer variables.
    pub maxintfillin: i32,
    /// Maximal fillin for binary variables.
    pub maxbinfillin: i32,
    /// Maximal support of one equality to be used for cancelling (-1: no
    /// limit).
    pub maxnonzeros: i32,
    /// Maximal number of considered non-zeros within one row (-1: no limit).
    pub maxconsiderednonzeros: i32,
    /// Limit on the number of useless vs. useful hashtable retrieves as a
    /// multiple of the number of constraints.
    pub maxretrievefac: f64,
    /// Number of calls to wait until next execution as a multiple of the
    /// number of useless calls.
    pub waitingfac: f64,
    /// Order in which to process inequalities ('n'o sorting, 'i'ncreasing
    /// nonzeros, 'd'ecreasing nonzeros).
    pub rowsort: u8,
    /// Should sparsify presolver be copied to sub-SCIPs?
    pub enablecopy: bool,
    /// Should we cancel nonzeros in constraints of the linear constraint
    /// handler?
    pub cancellinear: bool,
    /// Should we forbid cancellations that destroy integer coefficients?
    pub preserveintcoefs: bool,
}

/// `ROWVARPAIR`: structure representing a pair of variables in a row; used
/// for lookup in a hashtable.
#[derive(Clone, Copy, Debug, Default)]
struct RowVarPair {
    rowindex: i32,
    varindex1: i32,
    varindex2: i32,
    varcoef1: f64,
    varcoef2: f64,
}

/// The pair table's callbacks: elements are indices into the pair array.
struct PairOps<'a> {
    scip: &'a Scip,
    varpairs: &'a [RowVarPair],
}

impl HashOps<usize> for PairOps<'_> {
    type Key = RowVarPair;

    fn get_key(&self, element: &usize) -> RowVarPair {
        self.varpairs[*element]
    }

    /// `varPairsEqual`: returns TRUE iff both keys are equal.
    fn key_eq(&self, varpair1: &RowVarPair, varpair2: &RowVarPair) -> bool {
        if varpair1.varindex1 != varpair2.varindex1 {
            return false;
        }

        if varpair1.varindex2 != varpair2.varindex2 {
            return false;
        }

        let ratio1 = varpair1.varcoef2 / varpair1.varcoef1;
        let ratio2 = varpair2.varcoef2 / varpair2.varcoef1;

        self.scip.set.is_eq(ratio1, ratio2)
    }

    /// `varPairHashval`: returns the hash value of the key.
    fn key_val(&self, varpair: &RowVarPair) -> u64 {
        crate::misc::hash_three(
            varpair.varindex1 as u32,
            varpair.varindex2 as u32,
            crate::misc::real_hash_code(varpair.varcoef2 / varpair.varcoef1),
        ) as u64
    }
}

/// `cancelRow`: try non-zero cancellation for given row.
#[allow(clippy::too_many_arguments)]
fn cancel_row(
    scip: &mut Scip,
    matrix: &Matrix,
    pairtable: &HashTable<usize>,
    varpairs: &[RowVarPair],
    rowidx: usize,
    maxcontfillin: i32,
    maxintfillin: i32,
    maxbinfillin: i32,
    maxconsiderednonzeros: i32,
    preserveintcoefs: bool,
    nuseless: &mut i64,
    nchgcoefs: &mut i32,
    ncanceled: &mut i32,
    nfillin: &mut i32,
) -> ScipResult {
    let rowiseq = scip.set.is_eq(matrix.lhs[rowidx], matrix.rhs[rowidx]);
    let rowrange = matrix.row_range(rowidx);
    let mut cancelrowlen = matrix.row_nnonzs(rowidx) as usize;
    let cancelcons = matrix.cons[rowidx];

    let mut mincancelrate = 0.0;

    // For set packing and logicor constraints, only accept equalities where
    // all modified coefficients are cancelled.
    let hdlr = scip.conss[cancelcons].conshdlr;
    if hdlr == crate::set::set_find_conshdlr(scip, "setppc") || hdlr == crate::set::set_find_conshdlr(scip, "logicor") {
        mincancelrate = 1.0;
    }

    let mut cancelrowinds: Vec<i32> = matrix.rowmatind[rowrange.clone()].to_vec();
    let mut cancelrowvals: Vec<f64> = matrix.rowmatval[rowrange].to_vec();
    let mut tmpinds = vec![0i32; cancelrowlen];
    let mut tmpvals = vec![0.0f64; cancelrowlen];
    let mut locks = vec![0i32; cancelrowlen];

    let mut cancellhs = matrix.lhs[rowidx];
    let mut cancelrhs = matrix.rhs[rowidx];

    let mut nchgcoef = 0;
    let mut nretrieves: i64 = 0;

    let ops = PairOps { scip, varpairs };

    loop {
        let mut bestscale = 1.0;
        let mut bestcand: i32 = -1;
        let mut bestnfillin = 0;
        let mut bestcancelrate = 0.0;

        // The row grows by fill-in: keep the work arrays as long as the row.
        if tmpinds.len() < cancelrowlen {
            tmpinds.resize(cancelrowlen, 0);
            tmpvals.resize(cancelrowlen, 0.0);
            locks.resize(cancelrowlen, 0);
        }

        for i in 0..cancelrowlen {
            tmpinds[i] = i as i32;
            let c = cancelrowinds[i] as usize;
            locks[i] = matrix.ndownlocks[c] + matrix.nuplocks[c];
        }

        crate::sort::sort(&mut locks[..cancelrowlen], (&mut tmpinds[..cancelrowlen],), crate::sort::cmp_int);

        let mut maxlen = cancelrowlen;
        if maxconsiderednonzeros >= 0 {
            maxlen = crate::def::min(cancelrowlen, maxconsiderednonzeros as usize);
        }

        for i in 0..maxlen {
            for j in i + 1..maxlen {
                let i1 = tmpinds[i] as usize;
                let i2 = tmpinds[j] as usize;

                let rowvarpair = if cancelrowinds[i1] < cancelrowinds[i2] {
                    RowVarPair {
                        rowindex: 0,
                        varindex1: cancelrowinds[i1],
                        varindex2: cancelrowinds[i2],
                        varcoef1: cancelrowvals[i1],
                        varcoef2: cancelrowvals[i2],
                    }
                } else {
                    RowVarPair {
                        rowindex: 0,
                        varindex1: cancelrowinds[i2],
                        varindex2: cancelrowinds[i1],
                        varcoef1: cancelrowvals[i2],
                        varcoef2: cancelrowvals[i1],
                    }
                };

                let found = pairtable.retrieve(&ops, &rowvarpair);
                nretrieves += 1;

                let Some(found) = found else { continue };
                let eqrowvarpair = varpairs[found];
                if eqrowvarpair.rowindex as usize == rowidx {
                    continue;
                }
                let eqrow = eqrowvarpair.rowindex as usize;

                // If the row we want to cancel is an equality, we will only use
                // equalities for canceling with less non-zeros and if the number of
                // non-zeros is equal we use the rowindex as tie-breaker to avoid
                // cyclic non-zero cancellation.
                let eqrowlen = matrix.row_nnonzs(eqrow) as usize;
                if rowiseq && (cancelrowlen < eqrowlen || (cancelrowlen == eqrowlen && rowidx < eqrow)) {
                    continue;
                }

                let eqbeg = matrix.rowmatbeg[eqrow] as usize;
                let eqrowvals = &matrix.rowmatval[eqbeg..eqbeg + eqrowlen];
                let eqrowinds = &matrix.rowmatind[eqbeg..eqbeg + eqrowlen];

                let scale = -rowvarpair.varcoef1 / eqrowvarpair.varcoef1;

                if scale.abs() > MAXSCALE {
                    continue;
                }

                let mut a = 0;
                let mut b = 0;
                let mut ncancel = 0;
                let mut ncontfillin = 0;
                let mut nintfillin = 0;
                let mut nbinfillin = 0;
                let mut abortpair = false;
                while a < cancelrowlen && b < eqrowlen {
                    if cancelrowinds[a] == eqrowinds[b] {
                        let newcoef = cancelrowvals[a] + scale * eqrowvals[b];
                        let col = cancelrowinds[a] as usize;

                        // Check if coefficient is cancelled.
                        if scip.set.is_zero(newcoef) {
                            ncancel += 1;
                        }
                        // Otherwise, check if integral coefficients are preserved if the
                        // column is integral.
                        else if preserveintcoefs && var_is_integral(scip, matrix.vars[col]) && scip.set.is_integral(cancelrowvals[a]) && !scip.set.is_integral(newcoef) {
                            abortpair = true;
                            break;
                        }
                        // Finally, check if locks could be modified in a bad way due to
                        // flipped signs.
                        else if (scip.set.is_infinity(cancelrhs) || scip.set.is_infinity(-cancellhs)) && 1.0f64.copysign(newcoef) != 1.0f64.copysign(cancelrowvals[a]) {
                            // Do not flip signs for non-canceled coefficients if this adds a
                            // lock to a variable that had at most one lock in that direction
                            // before, except if the other direction gets unlocked.
                            if (cancelrowvals[a] > 0.0 && !scip.set.is_infinity(cancelrhs)) || (cancelrowvals[a] < 0.0 && !scip.set.is_infinity(-cancellhs)) {
                                // If we get into this case the variable had a positive
                                // coefficient in a <= constraint or a negative coefficient in a
                                // >= constraint, e.g. an uplock. If this was the only uplock we
                                // do not abort their cancelling, otherwise we abort if we had a
                                // single or no downlock and add one.
                                if matrix.nuplocks[col] > 1 && matrix.ndownlocks[col] <= 1 {
                                    abortpair = true;
                                    break;
                                }
                            }

                            if (cancelrowvals[a] < 0.0 && !scip.set.is_infinity(cancelrhs)) || (cancelrowvals[a] > 0.0 && !scip.set.is_infinity(-cancellhs)) {
                                // Symmetric case where the variable had a downlock.
                                if matrix.ndownlocks[col] > 1 && matrix.nuplocks[col] <= 1 {
                                    abortpair = true;
                                    break;
                                }
                            }
                        }

                        a += 1;
                        b += 1;
                    } else if cancelrowinds[a] < eqrowinds[b] {
                        a += 1;
                    } else {
                        let col = eqrowinds[b] as usize;
                        let var = matrix.vars[col];
                        let newcoef = scale * eqrowvals[b];

                        if (newcoef > 0.0 && !scip.set.is_infinity(cancelrhs)) || (newcoef < 0.0 && !scip.set.is_infinity(-cancellhs)) {
                            if matrix.nuplocks[col] <= 1 {
                                abortpair = true;
                                b += 1;
                                break;
                            }
                        }

                        if (newcoef < 0.0 && !scip.set.is_infinity(cancelrhs)) || (newcoef > 0.0 && !scip.set.is_infinity(-cancellhs)) {
                            if matrix.ndownlocks[col] <= 1 {
                                abortpair = true;
                                b += 1;
                                break;
                            }
                        }

                        b += 1;

                        if var_is_integral(scip, var) {
                            if var_is_binary(scip, var) {
                                nbinfillin += 1;
                                if nbinfillin > maxbinfillin {
                                    abortpair = true;
                                    break;
                                }
                            }

                            nintfillin += 1;
                            if nintfillin > maxintfillin {
                                abortpair = true;
                                break;
                            }
                        } else {
                            ncontfillin += 1;
                            if ncontfillin > maxcontfillin {
                                abortpair = true;
                                break;
                            }
                        }
                    }
                }

                if abortpair {
                    continue;
                }

                while b < eqrowlen {
                    let var = matrix.vars[eqrowinds[b] as usize];
                    b += 1;
                    if var_is_integral(scip, var) {
                        if var_is_binary(scip, var) {
                            nbinfillin += 1;
                            if nbinfillin > maxbinfillin {
                                break;
                            }
                        }
                        nintfillin += 1;
                        if nintfillin > maxintfillin {
                            break;
                        }
                    } else {
                        ncontfillin += 1;
                        if ncontfillin > maxcontfillin {
                            break;
                        }
                    }
                }

                if ncontfillin > maxcontfillin || nbinfillin > maxbinfillin || nintfillin > maxintfillin {
                    continue;
                }

                let ntotfillin = nintfillin + ncontfillin;

                if ntotfillin >= ncancel {
                    continue;
                }

                let cancelrate = (ncancel - ntotfillin) as f64 / eqrowlen as f64;

                if cancelrate < mincancelrate {
                    continue;
                }

                if cancelrate > bestcancelrate {
                    bestnfillin = ntotfillin;
                    bestcand = eqrow as i32;
                    bestscale = scale;
                    bestcancelrate = cancelrate;

                    // Stop looking if the current candidate does not create any
                    // fill-in or alter coefficients.
                    if cancelrate == 1.0 {
                        break;
                    }
                }

                // We accept the best candidate immediately if it does not create any
                // fill-in or alter coefficients.
                if bestcand != -1 && bestcancelrate == 1.0 {
                    break;
                }
            }
            // The `break`s above leave only the inner loop, as in C.
        }

        if bestcand != -1 {
            let eqrow = bestcand as usize;
            let eqrowlen = matrix.row_nnonzs(eqrow) as usize;
            let eqbeg = matrix.rowmatbeg[eqrow] as usize;
            let eqrowvals = &matrix.rowmatval[eqbeg..eqbeg + eqrowlen];
            let eqrowinds = &matrix.rowmatind[eqbeg..eqbeg + eqrowlen];
            let eqrhs = matrix.rhs[eqrow];

            let mut a = 0;
            let mut b = 0;
            let mut tmprowlen = 0;

            if !scip.set.is_zero(eqrhs) {
                if !scip.set.is_infinity(-cancellhs) {
                    cancellhs += bestscale * eqrhs;
                }
                if !scip.set.is_infinity(cancelrhs) {
                    cancelrhs += bestscale * eqrhs;
                }
            }

            // The new row has at most cancelrowlen + eqrowlen entries.
            if tmpinds.len() < cancelrowlen + eqrowlen {
                tmpinds.resize(cancelrowlen + eqrowlen, 0);
                tmpvals.resize(cancelrowlen + eqrowlen, 0.0);
            }

            while a < cancelrowlen && b < eqrowlen {
                if cancelrowinds[a] == eqrowinds[b] {
                    let val = cancelrowvals[a] + bestscale * eqrowvals[b];

                    if !scip.set.is_zero(val) {
                        tmpinds[tmprowlen] = cancelrowinds[a];
                        tmpvals[tmprowlen] = val;
                        tmprowlen += 1;
                    }
                    nchgcoef += 1;

                    a += 1;
                    b += 1;
                } else if cancelrowinds[a] < eqrowinds[b] {
                    tmpinds[tmprowlen] = cancelrowinds[a];
                    tmpvals[tmprowlen] = cancelrowvals[a];
                    tmprowlen += 1;
                    a += 1;
                } else {
                    tmpinds[tmprowlen] = eqrowinds[b];
                    tmpvals[tmprowlen] = eqrowvals[b] * bestscale;
                    nchgcoef += 1;
                    tmprowlen += 1;
                    b += 1;
                }
            }

            while a < cancelrowlen {
                tmpinds[tmprowlen] = cancelrowinds[a];
                tmpvals[tmprowlen] = cancelrowvals[a];
                tmprowlen += 1;
                a += 1;
            }

            while b < eqrowlen {
                tmpinds[tmprowlen] = eqrowinds[b];
                tmpvals[tmprowlen] = eqrowvals[b] * bestscale;
                nchgcoef += 1;
                tmprowlen += 1;
                b += 1;
            }

            // Update fill-in counter.
            *nfillin += bestnfillin;

            // Swap the temporary arrays so that the cancelrowinds and
            // cancelrowvals arrays, contain the new changed row, and the tmpinds
            // and tmpvals arrays can be overwritten in the next iteration.
            std::mem::swap(&mut tmpinds, &mut cancelrowinds);
            std::mem::swap(&mut tmpvals, &mut cancelrowvals);
            cancelrowlen = tmprowlen;
        } else {
            break;
        }
    }

    if nchgcoef != 0 {
        let consvars: Vec<_> = cancelrowinds[..cancelrowlen].iter().map(|&c| matrix.vars[c as usize]).collect();

        // Create sparsified constraint and add it to scip.
        let name = scip.conss[cancelcons].name.clone();
        let mut cons = crate::cons_linear::create_cons_linear(
            scip,
            &name,
            &consvars,
            &cancelrowvals[..cancelrowlen],
            cancellhs,
            cancelrhs,
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
        scip_cons::del_cons(scip, cancelcons)?;
        scip_cons::add_cons(scip, cons)?;

        scip_cons::release_cons(scip, &mut cons)?;

        // Update counters.
        *nchgcoefs += nchgcoef;
        *ncanceled += matrix.row_nnonzs(rowidx) - cancelrowlen as i32;

        // If successful, decrease the useless hashtable retrieves counter; the
        // rationale here is that we want to keep going if, after many useless
        // calls that almost exceeded the budget, we finally reach a useful
        // section; but we don't allow a negative build-up for the case that the
        // useful section is all at the beginning and we just want to quit
        // quickly afterwards.
        *nuseless -= nretrieves;
        *nuseless = crate::def::max(*nuseless, 0);
    } else {
        // If not successful, increase useless hashtable retrieves counter.
        *nuseless += nretrieves;
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

#[inline]
fn pdm(scip: &mut Scip, presol: PresolId) -> &mut PresolData {
    scip.presols[presol].presoldata.as_mut().unwrap().downcast_mut::<PresolData>().unwrap()
}

/*
 * Callback methods of presolver
 */

/// `presolExecSparsify`: execution method of presolver.
fn presol_exec_sparsify(
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

    if pdm(scip, presol).nwaitingcalls > 0 {
        pdm(scip, presol).nwaitingcalls -= 1;
        return Ok(());
    }

    // If we want to cancel only from specialized constraints according to the
    // parameter, then we can skip execution if only linear constraints are
    // present.
    let linearhdlr = crate::set::set_find_conshdlr(scip, "linear");
    if !pdm(scip, presol).cancellinear && linearhdlr != crate::var::NONE && scip.conshdlrs[linearhdlr].nconss >= api::get_n_conss(scip) {
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

    // If infeasibility was detected during matrix creation, return here.
    if created.infeasible {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // We only work on pure MIPs currently.
    match (created.matrix, created.complete) {
        (Some(mut matrix), true) => {
            let p = pdm(scip, presol);
            let (maxnonzeros, maxconsiderednonzeros, nfailures, rowsort, maxretrievefac, cancellinear, preserveintcoefs) = (
                p.maxnonzeros,
                p.maxconsiderednonzeros,
                p.nfailures,
                p.rowsort,
                p.maxretrievefac,
                p.cancellinear,
                p.preserveintcoefs,
            );
            let unlimited = |v: i32| if v == -1 { i32::MAX } else { v };
            let (maxcontfillin, maxintfillin, maxbinfillin) = (unlimited(p.maxcontfillin), unlimited(p.maxintfillin), unlimited(p.maxbinfillin));

            let nrows = matrix.nrows as usize;

            // Sort rows by column indices.
            for i in 0..nrows {
                let r = matrix.row_range(i);
                crate::sort::sort(&mut matrix.rowmatind[r.clone()], (&mut matrix.rowmatval[r],), crate::sort::cmp_int);
            }

            let ncols = matrix.ncols as usize;
            let mut locks = vec![0i32; ncols];
            let mut perm = vec![0i32; ncols];

            // Loop over all rows and create var pairs.
            let mut numcancel = 0;
            let mut nfillin = 0;
            let mut varpairs: Vec<RowVarPair> = Vec::new();

            // Collect equalities and their number of non-zeros.
            for r in 0..nrows {
                let mut nnonz = matrix.row_nnonzs(r) as usize;

                // Consider equalities with support at most maxnonzeros; skip
                // singleton equalities, because these are faster processed by
                // trivial presolving.
                if nnonz >= 2 && (maxnonzeros < 0 || nnonz <= maxnonzeros as usize) && scip.set.is_eq(matrix.rhs[r], matrix.lhs[r]) {
                    let beg = matrix.rowmatbeg[r] as usize;
                    let rowinds = &matrix.rowmatind[beg..beg + nnonz];
                    let rowvals = &matrix.rowmatval[beg..beg + nnonz];

                    for i in 0..nnonz {
                        perm[i] = i as i32;
                        let c = rowinds[i] as usize;
                        locks[i] = matrix.ndownlocks[c] + matrix.nuplocks[c];
                    }

                    crate::sort::sort(&mut locks[..nnonz], (&mut perm[..nnonz],), crate::sort::cmp_int);

                    if maxconsiderednonzeros >= 0 {
                        nnonz = crate::def::min(nnonz, maxconsiderednonzeros as usize);
                    }

                    // If we are called after one or more failures, i.e., executions
                    // without finding cancellations, then we shift the section of
                    // nonzeros considered; in the case that the maxconsiderednonzeros
                    // limit is hit, this results in different variable pairs being
                    // tried and avoids trying the same useless cancellations
                    // repeatedly.
                    let failshift = (nfailures * maxconsiderednonzeros) as usize;

                    for i in 0..nnonz {
                        for j in i + 1..nnonz {
                            let i1 = perm[(i + failshift) % nnonz] as usize;
                            let i2 = perm[(j + failshift) % nnonz] as usize;
                            let pair = if rowinds[i1] < rowinds[i2] {
                                RowVarPair {
                                    rowindex: r as i32,
                                    varindex1: rowinds[i1],
                                    varindex2: rowinds[i2],
                                    varcoef1: rowvals[i1],
                                    varcoef2: rowvals[i2],
                                }
                            } else {
                                RowVarPair {
                                    rowindex: r as i32,
                                    varindex1: rowinds[i2],
                                    varindex2: rowinds[i1],
                                    varcoef1: rowvals[i2],
                                    varcoef2: rowvals[i1],
                                }
                            };
                            varpairs.push(pair);
                        }
                    }
                }
            }

            let mut pairtable: HashTable<usize> = HashTable::new(1);
            {
                let ops = PairOps { scip, varpairs: &varpairs };

                // Insert varpairs into hash table.
                for r in 0..varpairs.len() {
                    let mut insert = true;

                    // Check if this pair is already contained in the hash table; the
                    // loop is required due to the non-transitivity of the hash
                    // functions.
                    while let Some(other) = pairtable.retrieve(&ops, &varpairs[r]) {
                        // If the previous variable pair has fewer or the same number of
                        // non-zeros in the attached row we keep that pair and skip this
                        // one.
                        if matrix.row_nnonzs(varpairs[other].rowindex as usize) <= matrix.row_nnonzs(varpairs[r].rowindex as usize) {
                            insert = false;
                            break;
                        }

                        // This pairs row has fewer non-zeros, so remove the other pair
                        // from the hash table and loop.
                        pairtable.remove(&ops, &other);
                    }

                    if insert {
                        // Prevent the insertion of too many variable pairs into the
                        // hashtable; a safety margin of factor 4 is built into the
                        // 8=2*4.
                        if (pairtable.get_n_entries() as i64) * (8 * std::mem::size_of::<usize>() as i64) > i32::MAX as i64 {
                            break;
                        }

                        pairtable.insert(&ops, r);
                    }
                }
            }

            // Sort rows according to parameter value.
            let rowidxsorted: Option<Vec<i32>> = if rowsort == b'i' || rowsort == b'd' {
                let mut rowidxsorted: Vec<i32> = (0..nrows as i32).collect();
                let mut rowsparsity: Vec<i32> = (0..nrows).map(|r| if rowsort == b'i' { matrix.row_nnonzs(r) } else { -matrix.row_nnonzs(r) }).collect();
                crate::sort::sort(&mut rowsparsity, (&mut rowidxsorted[..],), crate::sort::cmp_int);
                Some(rowidxsorted)
            } else {
                None
            };

            // Loop over the rows and cancel non-zeros until maximum number of
            // retrieves is reached.
            let maxuseless = (maxretrievefac * nrows as f64) as i64;
            let mut nuseless: i64 = 0;
            let mut r = 0;
            while r < nrows && nuseless <= maxuseless && !crate::solve::solve_is_stopped(scip, false) {
                let rowidx = match &rowidxsorted {
                    Some(v) => v[r] as usize,
                    None => r,
                };
                r += 1;

                // Check whether we want to cancel only from specialized constraints;
                // one reasoning behind this may be that cancelling fractional
                // coefficients requires more numerical care than is currently
                // implemented in method cancelRow().
                if !cancellinear && scip.conss[matrix.cons[rowidx]].conshdlr == linearhdlr {
                    continue;
                }

                cancel_row(
                    scip,
                    &matrix,
                    &pairtable,
                    &varpairs,
                    rowidx,
                    maxcontfillin,
                    maxintfillin,
                    maxbinfillin,
                    maxconsiderednonzeros,
                    preserveintcoefs,
                    &mut nuseless,
                    &mut counts.nchgcoefs,
                    &mut numcancel,
                    &mut nfillin,
                )?;
            }

            // Update result.
            let p = pdm(scip, presol);
            p.ncancels += numcancel;
            p.nfillin += nfillin;

            if numcancel > 0 {
                *result = ScipResultCode::Success;
            }

            update_failure_statistic(pdm(scip, presol), numcancel > 0);
        }
        // If matrix construction fails once, we do not ever want to be called
        // again.
        _ => {
            let p = pdm(scip, presol);
            update_failure_statistic(p, false);
            p.nwaitingcalls = i32::MAX;
        }
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `presolFreeSparsify`: destructor of presolver to free user data (called
/// when SCIP is exiting).
fn presol_free_sparsify(scip: &mut Scip, presol: PresolId) -> ScipResult {
    // Free presolver data.
    scip.presols[presol].presoldata = None;
    Ok(())
}

/// `presolInitSparsify`: initialization method of presolver (called after
/// problem was transformed); the counters are set here (and not in the
/// initpre callback) such that they persist across restarts.
fn presol_init_sparsify(scip: &mut Scip, presol: PresolId) -> ScipResult {
    let p = pdm(scip, presol);
    p.ncancels = 0;
    p.nfillin = 0;
    p.nfailures = 0;
    p.nwaitingcalls = 0;

    Ok(())
}

/// `presolCopySparsify`: copy method for presolver plugins (called when SCIP
/// copies plugins).
fn presol_copy_sparsify(scip: &mut Scip, sourcescip: &Scip, presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver if copying is enabled.
    if sourcescip.presols[presol].presoldata.as_ref().unwrap().downcast_ref::<PresolData>().unwrap().enablecopy {
        include_presol_sparsify(scip)?;
    }
    Ok(())
}

/// `SCIPincludePresolSparsify`: creates the sparsify presolver and includes
/// it in SCIP.
pub fn include_presol_sparsify(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_real};

    // Create sparsify presolver data.
    let presoldata = PresolData {
        ncancels: 0,
        nfillin: 0,
        nfailures: 0,
        nwaitingcalls: 0,
        maxcontfillin: DEFAULT_MAX_CONT_FILLIN,
        maxintfillin: DEFAULT_MAX_INT_FILLIN,
        maxbinfillin: DEFAULT_MAX_BIN_FILLIN,
        maxnonzeros: DEFAULT_MAXNONZEROS,
        maxconsiderednonzeros: DEFAULT_MAXCONSIDEREDNONZEROS,
        maxretrievefac: DEFAULT_MAXRETRIEVEFAC,
        waitingfac: DEFAULT_WAITINGFAC,
        rowsort: DEFAULT_ROWSORT,
        enablecopy: DEFAULT_ENABLECOPY,
        cancellinear: DEFAULT_CANCELLINEAR,
        preserveintcoefs: DEFAULT_PRESERVEINTCOEFS,
    };

    // Include presolver.
    let presol = api::include_presol_basic(
        scip,
        PRESOL_NAME,
        PRESOL_DESC,
        PRESOL_PRIORITY,
        PRESOL_MAXROUNDS,
        PRESOL_TIMING,
        presol_exec_sparsify,
        Some(Box::new(presoldata) as Box<dyn Any>),
    )?;
    scip.presols[presol].presolcopy = Some(presol_copy_sparsify);

    scip.presols[presol].presolfree = Some(presol_free_sparsify);
    scip.presols[presol].presolinit = Some(presol_init_sparsify);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, presols, presol, presoldata, PresolData, $f)
        };
    }

    paramset_add_bool(
        scip,
        "presolving/sparsify/enablecopy",
        "should sparsify presolver be copied to sub-SCIPs?",
        t!(bool, enablecopy),
        true,
        DEFAULT_ENABLECOPY,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/sparsify/cancellinear",
        "should we cancel nonzeros in constraints of the linear constraint handler?",
        t!(bool, cancellinear),
        true,
        DEFAULT_CANCELLINEAR,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/sparsify/preserveintcoefs",
        "should we forbid cancellations that destroy integer coefficients?",
        t!(bool, preserveintcoefs),
        true,
        DEFAULT_PRESERVEINTCOEFS,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/sparsify/maxcontfillin",
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
        "presolving/sparsify/maxbinfillin",
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
        "presolving/sparsify/maxintfillin",
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
        "presolving/sparsify/maxnonzeros",
        "maximal support of one equality to be used for cancelling (-1: no limit)",
        t!(i32, maxnonzeros),
        true,
        DEFAULT_MAXNONZEROS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/sparsify/maxconsiderednonzeros",
        "maximal number of considered non-zeros within one row (-1: no limit)",
        t!(i32, maxconsiderednonzeros),
        true,
        DEFAULT_MAXCONSIDEREDNONZEROS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "presolving/sparsify/rowsort",
        "order in which to process inequalities ('n'o sorting, 'i'ncreasing nonzeros, 'd'ecreasing nonzeros)",
        t!(u8, rowsort),
        true,
        DEFAULT_ROWSORT,
        Some("nid"),
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/sparsify/maxretrievefac",
        "limit on the number of useless vs. useful hashtable retrieves as a multiple of the number of constraints",
        t!(f64, maxretrievefac),
        true,
        DEFAULT_MAXRETRIEVEFAC,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/sparsify/waitingfac",
        "number of calls to wait until next execution as a multiple of the number of useless calls",
        t!(f64, waitingfac),
        true,
        DEFAULT_WAITINGFAC,
        0.0,
        f64::MAX,
        None,
    )?;

    Ok(())
}
