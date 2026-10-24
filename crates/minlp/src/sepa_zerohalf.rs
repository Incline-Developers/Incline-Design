//! Port of `sepa_zerohalf.c`: {0,1/2}-cuts separator.
//!
//! The mod 2 rows and columns live in arenas indexed by id; a column's set
//! of rows is a `BTreeSet` of row ids where C has a `SCIP_HASHSET` of row
//! pointers. C iterates that set in an order that depends on memory
//! addresses, but every use is independent of the order (signatures OR
//! the rows, equality tests the sets, and row additions and unlinkings
//! change each row on its own), so any set gives the same results.

use std::{any::Any, collections::BTreeSet};

use crate::{
    api,
    def::{REAL_MAX, max},
    hash::{HashOps, HashTable},
    misc::{RandNumGen, hash_signature64, rel_diff},
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{ColId, RowId, ScipResultCode, SepaId, SolId, VarId},
    var::NONE,
};

const SEPA_NAME: &str = "zerohalf";
const SEPA_DESC: &str = "{0,1/2}-cuts separator";
const SEPA_PRIORITY: i32 = -6000;
const SEPA_FREQ: i32 = 10;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
const SEPA_USESSUBSCIP: bool = false;
const SEPA_DELAY: bool = false;

const DEFAULT_MAXROUNDS: i32 = 5;
const DEFAULT_MAXROUNDSROOT: i32 = 20;
const DEFAULT_MAXSEPACUTS: i32 = 20;
const DEFAULT_MAXSEPACUTSROOT: i32 = 100;
const DEFAULT_MAXCUTCANDS: i32 = 2000;
const DEFAULT_MAXSLACK: f64 = 0.0;
const DEFAULT_MAXSLACKROOT: f64 = 0.0;
const DEFAULT_GOODSCORE: f64 = 1.0;
const DEFAULT_BADSCORE: f64 = 0.5;
const DEFAULT_MINVIOL: f64 = 0.1;
const DEFAULT_DYNAMICCUTS: bool = true;
const DEFAULT_MAXROWDENSITY: f64 = 0.05;
const DEFAULT_DENSITYOFFSET: i32 = 100;
const DEFAULT_INITSEED: i32 = 0x5EED;
const DEFAULT_OBJPARALWEIGHT: f64 = 0.0;
const DEFAULT_EFFICACYWEIGHT: f64 = 1.0;
const DEFAULT_DIRCUTOFFDISTWEIGHT: f64 = 0.0;
const DEFAULT_GOODMAXPARALL: f64 = 0.1;
const DEFAULT_MAXPARALL: f64 = 0.1;

// SCIPcalcRowIntegralScalar parameters.
const MAXDNOM: i64 = 1000;
const MAXSCALE: f64 = 1000.0;

/// Maximum number of rounds to perform reductions on the mod 2 system.
const MAXREDUCTIONROUNDS: i32 = 100;
/// Threshold for bound switching.
const BOUNDSWITCH: f64 = 0.5;

/// `MAXAGGRLEN(nvars)`.
fn maxaggrlen(nvars: i32) -> i32 {
    (0.1 * nvars as f64 + 1000.0) as i32
}

/// `SCIP_DEFAULT_EPSILON`.
const DEFAULT_EPSILON: f64 = 1e-09;

// Types of row indices in `RowIndex`.
const ORIG_RHS: u32 = 0;
const ORIG_LHS: u32 = 1;
const TRANSROW: u32 = 2;

/// `ROWINDEX`.
#[derive(Clone, Copy)]
struct RowIndex {
    /// Type of row index; 0 means lp row using the right hand side, 1 means
    /// lp row using the left hand side, and 2 means a transformed integral
    /// row.
    typ: u32,
    /// Lp position of original row, or index of transformed integral row.
    index: u32,
}

/// `UNIQUE_INDEX(rowind)`.
fn unique_index(r: RowIndex) -> usize {
    (3 * r.index + r.typ) as usize
}

/// `TRANSINTROW`: a transformed integral row obtained by relaxing an lp
/// row.
struct TransIntRow {
    slack: f64,
    rhs: f64,
    vals: Vec<f64>,
    varinds: Vec<i32>,
    len: usize,
    rank: i32,
    local: bool,
}

/// `MOD2_ROW`.
struct Mod2Row {
    /// Index set of rows associated with the mod 2 row.
    rowinds: Vec<RowIndex>,
    /// Sorted array of non-zero mod 2 columns in this mod 2 row.
    nonzcols: Vec<usize>,
    slack: f64,
    maxsolval: f64,
    index: i32,
    pos: i32,
    rhs: i32,
}

/// `MOD2_COL`.
struct Mod2Col {
    /// The set of rows that contain this column.
    nonzrows: BTreeSet<usize>,
    solval: f64,
    pos: i32,
    index: i32,
}

/// `MOD2_MATRIX`.
#[derive(Default)]
struct Mod2Matrix {
    /// The columns of the matrix (ids into `colarena`).
    cols: Vec<usize>,
    /// The rows of the matrix (ids into `rowarena`).
    rows: Vec<usize>,
    colarena: Vec<Mod2Col>,
    rowarena: Vec<Mod2Row>,
    transintrows: Vec<TransIntRow>,
    nzeroslackrows: i32,
}

/// Data of separator.
#[derive(Default)]
struct SepaData {
    randnumgen: Option<RandNumGen>,
    minviol: f64,
    maxslack: f64,
    maxslackroot: f64,
    maxrowdensity: f64,
    goodscore: f64,
    badscore: f64,
    objparalweight: f64,
    efficacyweight: f64,
    dircutoffdistweight: f64,
    goodmaxparall: f64,
    maxparall: f64,
    /// Infeasibility was detected after adding a zerohalf cut.
    infeasible: bool,
    dynamiccuts: bool,
    maxrounds: i32,
    maxroundsroot: i32,
    maxsepacuts: i32,
    maxsepacutsroot: i32,
    maxcutcands: i32,
    densityoffset: i32,
    initseed: i32,
    /// Number of reductions to the mod 2 system found so far.
    nreductions: i32,
}

fn data(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn data_mut(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/// `compareColIndex`: compare to mod 2 columns by there index.
fn compare_col_index(m: &Mod2Matrix, col1: usize, col2: usize) -> i32 {
    let (i1, i2) = (m.colarena[col1].index, m.colarena[col2].index);
    if i1 < i2 {
        -1
    } else if i2 < i1 {
        1
    } else {
        0
    }
}

/// `compareRowSlack`: comparison function for slack of mod 2 rows.
fn compare_row_slack(m: &Mod2Matrix, row1: usize, row2: usize) -> i32 {
    let (r1, r2) = (&m.rowarena[row1], &m.rowarena[row2]);
    let slack1iszero = r1.slack.abs() <= DEFAULT_EPSILON;
    let slack2iszero = r2.slack.abs() <= DEFAULT_EPSILON;

    // Zero slack comes first.
    if slack1iszero && !slack2iszero {
        return -1;
    }
    if slack2iszero && !slack1iszero {
        return 1;
    }
    if !slack1iszero && !slack2iszero {
        return 0;
    }

    // Prefer rows that contain columns with large solution value.
    if r1.maxsolval > r2.maxsolval {
        return -1;
    }
    if r2.maxsolval > r1.maxsolval {
        return 1;
    }

    // Rows with less non-zeros come first rows.
    if r1.nonzcols.len() < r2.nonzcols.len() {
        return -1;
    }
    if r2.nonzcols.len() < r1.nonzcols.len() {
        return 1;
    }

    0
}

/// `mod2`: take integral real value modulo 2.
fn mod2(scip: &Scip, mut val: f64) -> i32 {
    val *= 0.5;
    if (scip.set.round(val) - val).abs() > 0.1 { 1 } else { 0 }
}

/// `getIntegralScalar`: returns `(sval, intval)`, the scaled value and the
/// scaled integral value, see SCIPcalcIntegralScalar().
fn get_integral_scalar(val: f64, scalar: f64, mindelta: f64, maxdelta: f64) -> (f64, f64) {
    let sval = val * scalar;
    let downval = sval.floor();
    let upval = sval.ceil();

    let downviol = rel_diff(sval, downval) - maxdelta;
    let upviol = mindelta - rel_diff(sval, upval);

    if downviol < upviol { (sval, downval) } else { (sval, upval) }
}

/// `transformNonIntegralRow`: tries to transform a non-integral row into an
/// integral row that can be used in zerohalf separation.
#[allow(clippy::too_many_arguments)]
fn transform_non_integral_row(
    scip: &mut Scip,
    sol: SolId,
    allowlocal: bool,
    maxslack: f64,
    sign: i32,
    mut local: bool,
    rank: i32,
    rowvals: &[f64],
    rowcols: &[ColId],
    rhs: f64,
    intvarpos: &mut [i32],
) -> ScipResult<Option<TransIntRow>> {
    let rowlen = rowvals.len();
    let mut transrowvars = vec![0i32; rowlen];
    let mut transrowvals = vec![0.0; rowlen];
    let mut transrowlen = 0;
    let mut transrowrhs = rhs;
    let sign = sign as f64;

    // First add all integral variables to the transformed row and remember
    // their positions in the row.
    for i in 0..rowlen {
        if !scip.cols[rowcols[i]].integral {
            continue;
        }

        let probindex = scip.cols[rowcols[i]].var_probindex;
        transrowvars[transrowlen] = probindex;
        transrowvals[transrowlen] = sign * rowvals[i];
        transrowlen += 1;
        intvarpos[probindex as usize] = transrowlen as i32;
    }

    // Now loop over the non-integral columns of the row and project them out
    // using simple or variable bounds.
    let mut success = true;

    for i in 0..rowlen {
        if scip.cols[rowcols[i]].integral {
            continue;
        }

        let mut localbound = false;
        let colvar = scip.cols[rowcols[i]].var;
        let val = sign * rowvals[i];
        let mut closestvbd = 0.0;
        let mut closestvbdind = 0;
        let mut vbd: Option<(f64, VarId, f64)> = None;

        // If the value is positive we need to use a lower bound constraint.
        let mut closestbound;
        if val > 0.0 {
            // Retrieve simple variable bound.
            closestbound = scip.vars[colvar].glbdom.lb;
            if allowlocal && scip.set.is_sum_gt(scip.vars[colvar].locdom.lb, closestbound) {
                // Only use local bound if it is better thatn the global bound.
                closestbound = scip.vars[colvar].locdom.lb;
                localbound = true;
            }

            // Retrieve closest variable bound.
            crate::var::var_get_closest_vlb(scip, colvar, NONE, &mut closestvbd, &mut closestvbdind);

            // If a suitable variable bound exists which is at least as good as a
            // local simple bound or better than a global simple bound we use it.
            if closestvbdind >= 0 && (scip.set.is_gt(closestvbd, closestbound) || (localbound && scip.set.is_sum_eq(closestvbd, closestbound))) {
                let vb = scip.vars[colvar].vlbs.as_ref().unwrap();
                let k = closestvbdind as usize;
                vbd = Some((vb.coefs[k], vb.vars[k], vb.constants[k]));
                closestbound = closestvbd;
            }
        } else {
            // Retrieve simple variable bound.
            closestbound = scip.vars[colvar].glbdom.ub;
            if allowlocal && scip.set.is_sum_lt(scip.vars[colvar].locdom.ub, closestbound) {
                closestbound = scip.vars[colvar].locdom.ub;
                localbound = true;
            }

            // Retrieve closest variable bound.
            crate::var::var_get_closest_vub(scip, colvar, NONE, &mut closestvbd, &mut closestvbdind);

            // If a suitable variable bound exists which is at least as good as a
            // local simple bound or better than a global simple bound we use it.
            if closestvbdind >= 0 && (scip.set.is_lt(closestvbd, closestbound) || (localbound && scip.set.is_sum_eq(closestvbd, closestbound))) {
                let vb = scip.vars[colvar].vubs.as_ref().unwrap();
                let k = closestvbdind as usize;
                vbd = Some((vb.coefs[k], vb.vars[k], vb.constants[k]));
                closestbound = closestvbd;
            }
        }

        if let Some((vbdcoef, vbdvar, vbdconst)) = vbd {
            let coef = val * vbdcoef;
            transrowrhs -= val * vbdconst;

            let probindex = scip.vars[vbdvar].probindex;
            let pos = intvarpos[probindex as usize] - 1;
            if pos >= 0 {
                transrowvals[pos as usize] += coef;
            } else {
                transrowvars[transrowlen] = probindex;
                transrowvals[transrowlen] = coef;
                transrowlen += 1;
                intvarpos[probindex as usize] = transrowlen as i32;
            }
        } else if !scip.set.is_infinity(closestbound.abs()) {
            local = local || localbound;
            transrowrhs -= val * closestbound;
        } else {
            success = false;
            break;
        }
    }

    let mut i = 0;
    while i < transrowlen {
        intvarpos[transrowvars[i] as usize] = 0;
        if scip.set.is_zero(transrowvals[i]) {
            transrowlen -= 1;
            transrowvals[i] = transrowvals[transrowlen];
            transrowvars[i] = transrowvars[transrowlen];
        } else {
            i += 1;
        }
    }

    if transrowlen <= 1 {
        success = false;
    }

    if !success {
        return Ok(None);
    }

    let vars = api::get_vars(scip).to_vec();
    let mut nchgcoefs = 0;
    let mut len = transrowlen as i32;
    success = !crate::cuts::cuts_tighten_coefficients(scip, local, &mut transrowvals, &mut transrowrhs, &mut transrowvars, &mut len, &mut nchgcoefs);
    let transrowlen = len as usize;

    let mindelta = -scip.set.epsilon();
    let maxdelta = scip.set.sumepsilon();

    if !success {
        return Ok(None);
    }

    let Some(intscalar) = crate::misc::calc_integral_scalar(&transrowvals[..transrowlen], mindelta, maxdelta, MAXDNOM, MAXSCALE) else {
        return Ok(None);
    };

    transrowrhs *= intscalar;

    // Slack is initialized to zero since the transrowrhs can still change due
    // to bound usage in the loop below; the floored right hand side is then
    // added afterwards.
    let mut slack = 0.0;
    for i in 0..transrowlen {
        let var = vars[transrowvars[i] as usize];
        let solval = api::get_sol_val(scip, sol, var);
        let (newval, intval) = get_integral_scalar(transrowvals[i], intscalar, mindelta, maxdelta);

        if !scip.set.is_eq(intval, newval) {
            if intval < newval {
                let lb = if local { scip.vars[var].locdom.lb } else { scip.vars[var].glbdom.lb };

                if scip.set.is_infinity(-lb) {
                    return Ok(None);
                }

                transrowrhs += (intval - newval) * lb;
            } else {
                let ub = if local { scip.vars[var].locdom.ub } else { scip.vars[var].glbdom.ub };

                if scip.set.is_infinity(ub) {
                    return Ok(None);
                }

                transrowrhs += (intval - newval) * ub;
            }
        }

        slack -= solval * intval;
        transrowvals[i] = intval;
    }

    let floorrhs = scip.set.feas_floor(transrowrhs);
    slack += floorrhs;

    if slack <= maxslack {
        let mut introw = TransIntRow {
            rhs: floorrhs,
            slack,
            vals: transrowvals,
            varinds: transrowvars,
            len: transrowlen,
            local,
            rank,
        };
        if !scip.set.is_eq(floorrhs, transrowrhs) {
            introw.rank += 1;
        }
        Ok(Some(introw))
    } else {
        Ok(None)
    }
}

/// The LP rows (`SCIPgetLPRowsData`, `SCIPgetLPRows`).
fn lp_rows(scip: &Scip) -> Vec<RowId> {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `SCIPgetNLPCols`.
fn n_lp_cols(scip: &Scip) -> i32 {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        scip.lp.as_ref().unwrap().ncols
    } else {
        0
    }
}

/// `mod2MatrixTransformContRows`: tries to transform non-integral rows into
/// an integral form by using simple and variable bounds.
fn mod2_matrix_transform_cont_rows(scip: &mut Scip, sol: SolId, maxrowdensity: f64, densityoffset: i32, m: &mut Mod2Matrix, allowlocal: bool, maxslack: f64) -> ScipResult {
    let rows = lp_rows(scip);
    m.transintrows = Vec::with_capacity(2 * rows.len());

    let mut intvarpos = vec![0i32; api::get_n_vars(scip) as usize];
    let maxnonzeros = (n_lp_cols(scip) as f64 * maxrowdensity) as i32 + densityoffset;

    for &row in &rows {
        // Skip integral rows and rows not suitable for generating cuts.
        let r = &scip.rows[row];
        if r.modifiable || r.integral || (r.local && !allowlocal) || r.len > maxnonzeros {
            continue;
        }

        let (rowlhs, rowrhs, constant) = (r.lhs, r.rhs, r.constant);
        let lhs = rowlhs - constant;
        let rhs = rowrhs - constant;

        let activity = api::get_row_sol_activity(scip, row, sol) - constant;

        // Compute lhsslack: activity - lhs.
        let lhsslack = if scip.set.is_infinity(-rowlhs) { scip.set.infinity() } else { activity - lhs };

        // Compute rhsslack: rhs - activity.
        let rhsslack = if scip.set.is_infinity(rowrhs) { scip.set.infinity() } else { rhs - activity };

        if rhsslack > maxslack && lhsslack > maxslack {
            continue;
        }

        let r = &scip.rows[row];
        let rowlen = r.nlpcols as usize;
        let rowvals = r.vals.0[..rowlen].to_vec();
        let rowcols = r.cols.0[..rowlen].to_vec();
        let (local, rank) = (r.local, r.rank);

        if rhsslack <= maxslack {
            if let Some(introw) = transform_non_integral_row(scip, sol, allowlocal, maxslack, 1, local, rank, &rowvals, &rowcols, rhs, &mut intvarpos)? {
                m.transintrows.push(introw);
            }
        }

        if lhsslack <= maxslack {
            if let Some(introw) = transform_non_integral_row(scip, sol, allowlocal, maxslack, -1, local, rank, &rowvals, &rowcols, -lhs, &mut intvarpos)? {
                m.transintrows.push(introw);
            }
        }
    }

    Ok(())
}

/// The mod 2 column and right hand side offset a problem variable maps to
/// (C's `COLINFO`).
type ColInfo = (Option<usize>, i32);

/// `mod2MatrixAddCol`: adds new column to the mod 2 matrix.
fn mod2_matrix_add_col(scip: &Scip, m: &mut Mod2Matrix, origvar2col: &mut std::collections::HashMap<VarId, ColInfo>, origvar: VarId, solval: f64, rhsoffset: i32) {
    let id = m.colarena.len();
    m.colarena.push(Mod2Col {
        nonzrows: BTreeSet::new(),
        solval,
        pos: m.cols.len() as i32,
        index: scip.vars[origvar].probindex,
    });

    // Add column to mod 2 matrix.
    m.cols.push(id);

    // Create mapping of problem variable to mod 2 column with its right hand
    // side offset.
    origvar2col.insert(origvar, (Some(id), rhsoffset));
}

/// `mod2colLinkRow`: links row to mod 2 column.
fn mod2col_link_row(m: &mut Mod2Matrix, col: usize, row: usize) {
    m.colarena[col].nonzrows.insert(row);
    let solval = m.colarena[col].solval;
    let r = &mut m.rowarena[row];
    r.maxsolval = max(solval, r.maxsolval);
}

/// `mod2colUnlinkRow`: unlinks row from mod 2 column.
fn mod2col_unlink_row(m: &mut Mod2Matrix, col: usize, row: usize) {
    m.colarena[col].nonzrows.remove(&row);
}

/// `mod2rowUnlinkCol`: unlinks row from mod 2 column.
fn mod2row_unlink_col(m: &mut Mod2Matrix, row: usize, col: usize) {
    let i = {
        let r = &m.rowarena[row];
        crate::sort::sortedvec_find(&r.nonzcols, |a, b| compare_col_index(m, *a, *b), col).unwrap_or_else(|i| i)
    };

    m.rowarena[row].nonzcols.remove(i);

    if m.colarena[col].solval >= m.rowarena[row].maxsolval {
        let mut maxsolval = 0.0;
        for &c in &m.rowarena[row].nonzcols {
            maxsolval = max(m.colarena[c].solval, maxsolval);
        }
        m.rowarena[row].maxsolval = maxsolval;
    }
}

/// The new mod 2 row both `mod2MatrixAddOrigRow` and
/// `mod2MatrixAddTransRow` start from.
fn mod2_matrix_new_row(scip: &Scip, m: &mut Mod2Matrix, slack: f64, rhs: i32, rowind: RowIndex) -> usize {
    let id = m.rowarena.len();
    let index = m.rows.len() as i32;
    m.rowarena.push(Mod2Row {
        rowinds: vec![rowind],
        nonzcols: Vec::new(),
        slack: max(0.0, slack),
        maxsolval: 0.0,
        index,
        pos: 0,
        rhs,
    });
    m.rows.push(id);

    if scip.set.is_zero(m.rowarena[id].slack) {
        m.nzeroslackrows += 1;
    }
    id
}

/// Adds the mod 2 columns of the given integral coefficients to a new mod 2
/// row (the loop shared by `mod2MatrixAddOrigRow` and
/// `mod2MatrixAddTransRow`).
fn mod2_row_add_entries(scip: &Scip, m: &mut Mod2Matrix, origcol2col: &std::collections::HashMap<VarId, ColInfo>, row: usize, entries: &[(VarId, f64)]) {
    for &(var, val) in entries {
        if mod2(scip, val) == 1 {
            let (col, rhsoffset) = origcol2col.get(&var).copied().unwrap_or((None, 0));

            // Extract the righthand side offset from the colinfo and update the
            // righthand side.
            let r = &mut m.rowarena[row];
            r.rhs = (r.rhs + rhsoffset) % 2;

            // Extract the column pointer from the colinfo.
            if let Some(col) = col {
                m.rowarena[row].nonzcols.push(col);
                mod2col_link_row(m, col, row);
            }
        }
    }

    let mut nonzcols = std::mem::take(&mut m.rowarena[row].nonzcols);
    crate::sort::sort(&mut nonzcols[..], (), |a, b| compare_col_index(m, *a, *b));
    m.rowarena[row].nonzcols = nonzcols;
}

/// `mod2MatrixAddOrigRow`: adds a SCIP_ROW to the mod 2 matrix.
#[allow(clippy::too_many_arguments)]
fn mod2_matrix_add_orig_row(scip: &Scip, m: &mut Mod2Matrix, origcol2col: &std::collections::HashMap<VarId, ColInfo>, origrow: RowId, slack: f64, side: u32, rhsmod2: i32) {
    let row = mod2_matrix_new_row(
        scip,
        m,
        slack,
        rhsmod2,
        RowIndex {
            typ: side,
            index: scip.rows[origrow].lppos as u32,
        },
    );

    let r = &scip.rows[origrow];
    let rowlen = r.len as usize;
    let entries: Vec<(VarId, f64)> = (0..rowlen).map(|i| (scip.cols[r.cols[i]].var, r.vals[i])).collect();
    mod2_row_add_entries(scip, m, origcol2col, row, &entries);
}

/// `mod2MatrixAddTransRow`: adds a transformed integral row to the mod 2
/// matrix.
fn mod2_matrix_add_trans_row(scip: &Scip, m: &mut Mod2Matrix, origcol2col: &std::collections::HashMap<VarId, ColInfo>, transrowind: usize) {
    let vars = api::get_vars(scip);
    let (slack, rhs) = (m.transintrows[transrowind].slack, m.transintrows[transrowind].rhs);
    let row = mod2_matrix_new_row(
        scip,
        m,
        slack,
        mod2(scip, rhs),
        RowIndex {
            typ: TRANSROW,
            index: transrowind as u32,
        },
    );

    let introw = &m.transintrows[transrowind];
    let entries: Vec<(VarId, f64)> = (0..introw.len).map(|i| (vars[introw.varinds[i] as usize], introw.vals[i])).collect();
    mod2_row_add_entries(scip, m, origcol2col, row, &entries);
}

/// `buildMod2Matrix`: build the modulo 2 matrix from all integral rows in
/// the LP, and non-integral rows if the transformation to an integral row
/// succeeds.
fn build_mod2_matrix(scip: &mut Scip, sol: SolId, maxrowdensity: f64, densityoffset: i32, allowlocal: bool, maxslack: f64) -> ScipResult<Mod2Matrix> {
    let rows = lp_rows(scip);
    let nintvars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip)) as usize;
    let vars = api::get_vars(scip).to_vec();

    // Initialize fields.
    let mut m = Mod2Matrix::default();

    let mut origcol2col: std::collections::HashMap<VarId, ColInfo> = std::collections::HashMap::new();

    // Add all integral vars if they are not at their bound.
    for &var in &vars[..nintvars] {
        let primsol = api::get_sol_val(scip, sol, var);

        let lb = if allowlocal { scip.vars[var].locdom.lb } else { scip.vars[var].glbdom.lb };
        let lbsol = max(0.0, primsol - lb);
        if scip.set.is_zero(lbsol) {
            origcol2col.insert(var, (None, mod2(scip, lb)));
            continue;
        }

        let ub = if allowlocal { scip.vars[var].locdom.ub } else { scip.vars[var].glbdom.ub };
        let ubsol = max(0.0, ub - primsol);
        if scip.set.is_zero(ubsol) {
            origcol2col.insert(var, (None, mod2(scip, ub)));
            continue;
        }

        let useub = if scip.set.is_infinity(ub) {
            // If there is no ub, use lb.
            false
        } else if scip.set.is_infinity(-lb) {
            // If there is no lb, use ub.
            true
        } else {
            !scip.set.is_lt(primsol, (1.0 - BOUNDSWITCH) * lb + BOUNDSWITCH * ub)
        };

        if useub {
            mod2_matrix_add_col(scip, &mut m, &mut origcol2col, var, ubsol, mod2(scip, ub));
        } else {
            mod2_matrix_add_col(scip, &mut m, &mut origcol2col, var, lbsol, mod2(scip, lb));
        }
    }

    let maxnonzeros = (n_lp_cols(scip) as f64 * maxrowdensity) as i32 + densityoffset;

    // Add all integral rows using the created columns.
    for &row in &rows {
        // Skip non-integral rows and rows not suitable for generating cuts.
        let r = &scip.rows[row];
        if r.modifiable || !r.integral || (r.local && !allowlocal) || r.len > maxnonzeros {
            continue;
        }

        let mut lhsmod2 = 0;
        let mut rhsmod2 = 0;
        let (rowlhs, rowrhs, constant) = (r.lhs, r.rhs, r.constant);
        let activity = api::get_row_sol_activity(scip, row, sol) - constant;

        // Since row is integral we can ceil/floor the lhs/rhs after subtracting
        // the constant.
        let lhs = scip.set.feas_ceil(rowlhs - constant);
        let rhs = scip.set.feas_floor(rowrhs - constant);

        // Compute lhsslack: activity - lhs.
        let lhsslack = if scip.set.is_infinity(-rowlhs) {
            scip.set.infinity()
        } else {
            lhsmod2 = mod2(scip, lhs);
            activity - lhs
        };

        // Compute rhsslack: rhs - activity.
        let rhsslack = if scip.set.is_infinity(rowrhs) {
            scip.set.infinity()
        } else {
            rhsmod2 = mod2(scip, rhs);
            rhs - activity
        };

        if rhsslack <= maxslack && lhsslack <= maxslack {
            if lhsmod2 == rhsmod2 {
                // Maxslack < 1 implies rhs - lhs = rhsslack + lhsslack < 2.
                // Therefore lhs = rhs (mod2) can only hold if they are equal;
                // use rhs.
                mod2_matrix_add_orig_row(scip, &mut m, &origcol2col, row, rhsslack, ORIG_RHS, rhsmod2);
            } else {
                // Use both.
                mod2_matrix_add_orig_row(scip, &mut m, &origcol2col, row, lhsslack, ORIG_LHS, lhsmod2);
                mod2_matrix_add_orig_row(scip, &mut m, &origcol2col, row, rhsslack, ORIG_RHS, rhsmod2);
            }
        } else if rhsslack <= maxslack {
            // Use rhs.
            mod2_matrix_add_orig_row(scip, &mut m, &origcol2col, row, rhsslack, ORIG_RHS, rhsmod2);
        } else if lhsslack <= maxslack {
            // Use lhs.
            mod2_matrix_add_orig_row(scip, &mut m, &origcol2col, row, lhsslack, ORIG_LHS, lhsmod2);
        }
    }

    // Transform non-integral rows.
    mod2_matrix_transform_cont_rows(scip, sol, maxrowdensity, densityoffset, &mut m, allowlocal, maxslack)?;

    // Add all transformed integral rows using the created columns.
    for i in 0..m.transintrows.len() {
        mod2_matrix_add_trans_row(scip, &mut m, &origcol2col, i);
    }

    Ok(m)
}

/// The column hash table callbacks (`columnsEqual`, `columnGetSignature`).
struct ColumnOps<'a>(&'a Mod2Matrix);

impl HashOps<usize> for ColumnOps<'_> {
    type Key = usize;
    fn get_key(&self, element: &usize) -> usize {
        *element
    }
    /// `columnsEqual`: compare two mod 2 columns for equality.
    fn key_eq(&self, key1: &usize, key2: &usize) -> bool {
        let (c1, c2) = (&self.0.colarena[*key1], &self.0.colarena[*key2]);
        c1.nonzrows.len() == c2.nonzrows.len() && c1.nonzrows.iter().all(|r| c2.nonzrows.contains(r))
    }
    /// `columnGetSignature`: compute a signature of the rows in a mod 2
    /// matrix as hash value.
    fn key_val(&self, key: &usize) -> u64 {
        let mut signature = 0u64;
        for &r in &self.0.colarena[*key].nonzrows {
            signature |= hash_signature64(self.0.rowarena[r].index as u32);
        }
        signature
    }
}

/// The row hash table callbacks (`rowsEqual`, `rowGetSignature`).
struct RowOps<'a>(&'a Mod2Matrix);

impl HashOps<usize> for RowOps<'_> {
    type Key = usize;
    fn get_key(&self, element: &usize) -> usize {
        *element
    }
    /// `rowsEqual`: compare two mod 2 rows for equality.
    fn key_eq(&self, key1: &usize, key2: &usize) -> bool {
        let (r1, r2) = (&self.0.rowarena[*key1], &self.0.rowarena[*key2]);
        r1.nonzcols.len() == r2.nonzcols.len() && r1.rhs == r2.rhs && r1.nonzcols == r2.nonzcols
    }
    /// `rowGetSignature`: compute a signature of a mod 2 row as hash value.
    fn key_val(&self, key: &usize) -> u64 {
        let r = &self.0.rowarena[*key];
        let mut signature = r.rhs as u32 as u64;
        for &c in &r.nonzcols {
            signature |= hash_signature64(self.0.colarena[c].index as u32);
        }
        signature
    }
}

/// `mod2matrixRemoveRow`: removes a row from the mod 2 matrix.
fn mod2matrix_remove_row(scip: &Scip, m: &mut Mod2Matrix, row: usize) {
    let position = m.rowarena[row].pos as usize;

    // Update counter for zero slack rows.
    if scip.set.is_zero(m.rowarena[row].slack) {
        m.nzeroslackrows -= 1;
    }

    // Remove the row from the array.
    let last = m.rows.pop().unwrap();
    if position < m.rows.len() {
        m.rows[position] = last;
    }
    m.rowarena[last].pos = position as i32;

    // Unlink columns from row.
    let nonzcols = m.rowarena[row].nonzcols.clone();
    for c in nonzcols {
        mod2col_unlink_row(m, c, row);
    }
}

/// `mod2matrixRemoveCol`: removes a column from the mod 2 matrix.
fn mod2matrix_remove_col(m: &mut Mod2Matrix, col: usize) {
    let position = m.colarena[col].pos as usize;

    // Remove column from arrays.
    let last = m.cols.pop().unwrap();
    if position < m.cols.len() {
        m.cols[position] = last;
    }
    m.colarena[last].pos = position as i32;

    // Adjust rows of column.
    let rows: Vec<usize> = m.colarena[col].nonzrows.iter().copied().collect();
    for r in rows {
        mod2row_unlink_col(m, r, col);
    }
}

/// `mod2matrixPreprocessColumns`: remove columns that are (Prop3 iii) zero
/// (Prop3 iv) identify indentical columns (Prop3 v) unit vector columns.
fn mod2matrix_preprocess_columns(scip: &Scip, m: &mut Mod2Matrix, nreductions: &mut i32) {
    let mut columntable: HashTable<usize> = HashTable::new(m.cols.len() as i32);

    let mut i = 0;
    while i < m.cols.len() {
        let col = m.cols[i];
        let nnonzrows = m.colarena[col].nonzrows.len();
        if nnonzrows == 0 {
            // Prop3 iii.
            mod2matrix_remove_col(m, col);
        } else if nnonzrows == 1 {
            // Prop3 v.
            let row = *m.colarena[col].nonzrows.iter().next().unwrap();

            // Column is unit vector, so add its solution value to the rows slack
            // and remove it.
            if scip.set.is_zero(m.rowarena[row].slack) {
                m.nzeroslackrows -= 1;
            }

            m.rowarena[row].slack += m.colarena[col].solval;

            mod2matrix_remove_col(m, col);
            *nreductions += 1;
        } else {
            let identicalcol = columntable.retrieve(&ColumnOps(m), &col);
            if let Some(identicalcol) = identicalcol {
                // Column is identical to other column so add its solution value to
                // the other one and then remove and free it.
                m.colarena[identicalcol].solval += m.colarena[col].solval;

                // Also adjust the solval of the removed column so that the
                // maxsolval of each row is properly updated.
                m.colarena[col].solval = m.colarena[identicalcol].solval;

                mod2matrix_remove_col(m, col);
            } else {
                columntable.insert(&ColumnOps(m), col);
                i += 1;
            }
        }
    }
}

/// `NONZERO(x)`: `COPYSIGN(1e-100, x) + x`.
fn nonzero(x: f64) -> f64 {
    1e-100f64.copysign(x) + x
}

/// The aggregation `generateZerohalfCut` builds.
struct Aggregation {
    tmpcoefs: Vec<f64>,
    nonzeroinds: Vec<i32>,
    cutrhs: f64,
    cutrank: i32,
    cutislocal: bool,
}

/// `addOrigRow`: add original row to aggregation with weight 0.5.
fn add_orig_row(scip: &Scip, a: &mut Aggregation, row: RowId, sign: i32) {
    let weight = 0.5 * sign as f64;
    let r = &scip.rows[row];

    for i in 0..r.len as usize {
        let probindex = scip.cols[r.cols[i]].var_probindex;
        let mut val = a.tmpcoefs[probindex as usize];

        if val == 0.0 {
            a.nonzeroinds.push(probindex);
        }

        val += weight * r.vals[i];
        a.tmpcoefs[probindex as usize] = nonzero(val);
    }

    if sign == 1 {
        a.cutrhs += weight * scip.set.feas_floor(r.rhs - r.constant);
    } else {
        a.cutrhs += weight * scip.set.feas_ceil(r.lhs - r.constant);
    }

    if r.rank > a.cutrank {
        a.cutrank = r.rank;
    }
    a.cutislocal = a.cutislocal || r.local;
}

/// `addTransRow`: add transformed integral row to aggregation with weight
/// 0.5.
fn add_trans_row(a: &mut Aggregation, introw: &TransIntRow) {
    for i in 0..introw.len {
        let probindex = introw.varinds[i];
        let mut val = a.tmpcoefs[probindex as usize];

        if val == 0.0 {
            a.nonzeroinds.push(probindex);
        }

        val += 0.5 * introw.vals[i];
        a.tmpcoefs[probindex as usize] = nonzero(val);
    }

    a.cutrhs += 0.5 * introw.rhs;

    a.cutrank = max(a.cutrank, introw.rank);
    a.cutislocal = a.cutislocal || introw.local;
}

/// `calcEfficacy`: calculates the cuts efficacy of cut.
fn calc_efficacy(scip: &Scip, sol: SolId, cutcoefs: &[f64], cutrhs: f64, cutinds: &[i32]) -> f64 {
    let norm = api::get_vector_efficacy_norm(scip, cutcoefs);
    let vars = api::get_vars(scip);
    let mut activity = 0.0;
    for i in 0..cutcoefs.len() {
        activity += cutcoefs[i] * api::get_sol_val(scip, sol, vars[cutinds[i] as usize]);
    }

    (activity - cutrhs) / max(1e-6, norm)
}

/// `computeMaxViolation`: computes maximal violation that can be achieved
/// for zerohalf cuts where this row particiaptes.
fn compute_max_violation(row: &Mod2Row) -> f64 {
    let mut viol = 1.0 - row.slack;
    viol *= 0.5;
    viol
}

/// `computeViolation`: computes violation of zerohalf cut generated from
/// given mod 2 row.
fn compute_violation(m: &Mod2Matrix, row: usize) -> f64 {
    let r = &m.rowarena[row];
    let mut viol = 1.0 - r.slack;

    for &c in &r.nonzcols {
        viol -= m.colarena[c].solval;
    }

    viol *= 0.5;
    viol
}

/// The separation state that the generation and reduction steps share.
struct Round {
    minviol: f64,
    maxcutcands: i32,
    dynamiccuts: bool,
    cuts: Vec<RowId>,
    nreductions: i32,
    infeasible: bool,
}

/// `generateZerohalfCut`: generate a zerohalf cut from a given mod 2 row,
/// i.e., try if aggregations of rows of the mod2 matrix give violated cuts.
fn generate_zerohalf_cut(scip: &mut Scip, sol: SolId, m: &Mod2Matrix, sepa: SepaId, st: &mut Round, allowlocal: bool, row: usize) -> ScipResult {
    if compute_violation(m, row) < st.minviol {
        return Ok(());
    }

    let rows = lp_rows(scip);
    let nvars = api::get_n_vars(scip) as usize;
    let vars = api::get_vars(scip).to_vec();
    let maxaggrlen = maxaggrlen(n_lp_cols(scip));

    // Right hand side must be odd, otherwise no cut can be generated.

    // Perform the summation of the rows defined by the mod 2 row; the right
    // hand side of the zerohalf cut will be rounded down by 0.5 thus we can
    // add 0.5 to it already.
    let mut a = Aggregation {
        tmpcoefs: vec![0.0; nvars],
        nonzeroinds: Vec::new(),
        cutrhs: -0.5,
        cutrank: 0,
        cutislocal: false,
    };

    // Compute the aggregation of the rows with weight 0.5.
    for &ri in &m.rowarena[row].rowinds {
        match ri.typ {
            ORIG_RHS => add_orig_row(scip, &mut a, rows[ri.index as usize], 1),
            ORIG_LHS => add_orig_row(scip, &mut a, rows[ri.index as usize], -1),
            _ => add_trans_row(&mut a, &m.transintrows[ri.index as usize]),
        }
    }

    let mut cutinds = a.nonzeroinds;
    let mut cutnnz = cutinds.len() as i32;

    // Abort if aggregation is too long.
    if cutnnz > maxaggrlen {
        return Ok(());
    }

    let mut cutcoefs = vec![0.0; nvars];
    let mut cutrhs = a.cutrhs;
    let mut cutislocal = a.cutislocal;

    // Compute the cut coefficients and update right handside due to
    // complementation if necessary.
    let mut i = 0;
    while i < cutnnz as usize {
        let k = cutinds[i] as usize;
        let coef = a.tmpcoefs[k];
        let mut floorcoef = scip.set.feas_floor(coef);
        a.tmpcoefs[k] = 0.0;

        // Only check complementation if the coefficient was rounded down.
        if (coef - floorcoef).abs() > 0.1 {
            // Complement with closest bound.
            let primsol = api::get_sol_val(scip, sol, vars[k]);
            let mut lb = scip.vars[vars[k]].glbdom.lb;
            let mut ub = scip.vars[vars[k]].glbdom.ub;
            let mut loclb = false;
            let mut locub = false;

            // Use local bounds if better.
            if allowlocal {
                if scip.set.is_gt(scip.vars[vars[k]].locdom.lb, lb) {
                    loclb = true;
                    lb = scip.vars[vars[k]].locdom.lb;
                }

                if scip.set.is_lt(scip.vars[vars[k]].locdom.ub, ub) {
                    locub = true;
                    ub = scip.vars[vars[k]].locdom.ub;
                }
            }

            let useub = if scip.set.is_infinity(ub) {
                // If there is no ub, use lb.
                false
            } else if scip.set.is_infinity(-lb) {
                // If there is no lb, use ub.
                true
            } else {
                !scip.set.is_lt(primsol, (1.0 - BOUNDSWITCH) * lb + BOUNDSWITCH * ub)
            };

            if useub {
                // Set local flag if local bound was used.
                if locub {
                    cutislocal = true;
                }

                // Upper bound was used so floor was the wrong direction to round,
                // coefficient must be ceiled instead.
                floorcoef += 1.0;

                // Add delta of complementing then rounding by 0.5 and complementing
                // back to the right hand side.
                cutrhs += 0.5 * ub;
            } else {
                // Set local flag if local bound was used.
                if loclb {
                    cutislocal = true;
                }

                // Add delta of complementing then rounding by 0.5 and complementing
                // back to the right hand side.
                cutrhs -= 0.5 * lb;
            }
        }

        // Make coefficient exactly integral.
        floorcoef = scip.set.feas_round(floorcoef);

        // If coefficient is zero remove entry, otherwise set to floorcoef.
        if floorcoef == 0.0 {
            cutnnz -= 1;
            cutinds[i] = cutinds[cutnnz as usize];
        } else {
            cutcoefs[i] = floorcoef;
            i += 1;
        }
    }

    // Make right hand side exactly integral.
    cutrhs = scip.set.feas_round(cutrhs);

    let mut nchgcoefs = 0;
    if !crate::cuts::cuts_tighten_coefficients(scip, cutislocal, &mut cutcoefs, &mut cutrhs, &mut cutinds, &mut cutnnz, &mut nchgcoefs) {
        // Calculate efficacy.
        let n = cutnnz as usize;
        let cutefficacy = calc_efficacy(scip, sol, &cutcoefs[..n], cutrhs, &cutinds[..n]);

        if api::is_efficacious(scip, cutefficacy) {
            // Increase rank by 1.
            let cutrank = a.cutrank + 1;

            // Create the cut.
            let cutname = format!("zerohalf{}_x{}", scip.stat.nlps, m.rowarena[row].index);
            let inf = scip.set.infinity();
            let mut cut = scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -inf, cutrhs, cutislocal, false, st.dynamiccuts)?;

            scip.rows[cut].rank = cutrank;

            // Cache the row extension and only flush them if the cut gets added.
            scip_lp::cache_row_extensions(scip, cut);

            // Collect all non-zero coefficients.
            for v in 0..n {
                api::add_var_to_row(scip, cut, vars[cutinds[v] as usize], cutcoefs[v])?;
            }

            // Flush all changes before adding the cut.
            scip_lp::flush_row_extensions(scip, cut)?;

            if crate::cutpool::cutpool_is_cut_new(scip, crate::cutpool::GLOBALCUTPOOL, cut) {
                st.cuts.push(cut);
            } else {
                // Release the row.
                scip_lp::release_row(scip, &mut cut)?;
            }
        }
    }

    Ok(())
}

/// `mod2matrixPreprocessRows`: remove rows that are (a) zero (b) identical
/// to other rows (keep the one with smallest slack) (c) have slack greater
/// than 1 (d) for zero rows with 1 as rhs and slack less than 1, we
/// directly generate a cut and remove the row (Lemma 4).
fn mod2matrix_preprocess_rows(scip: &mut Scip, sol: SolId, m: &mut Mod2Matrix, sepa: SepaId, st: &mut Round, allowlocal: bool) -> ScipResult {
    let mut rowtable: HashTable<usize> = HashTable::new(m.rows.len() as i32);

    let mut i = 0;
    while i < m.rows.len() {
        let row = m.rows[i];
        m.rowarena[row].pos = i as i32;

        let (nnonzcols, rhs) = (m.rowarena[row].nonzcols.len(), m.rowarena[row].rhs);
        if (nnonzcols == 0 && rhs == 0) || compute_max_violation(&m.rowarena[row]) < st.minviol {
            // (a) and (c).
            st.nreductions += nnonzcols as i32;
            mod2matrix_remove_row(scip, m, row);
        } else if nnonzcols > 0 {
            // (b).
            let identicalrow = rowtable.retrieve(&RowOps(m), &row);
            if let Some(identicalrow) = identicalrow {
                // Row is identical to other row; only keep the one with smaller
                // slack.
                if m.rowarena[identicalrow].slack <= m.rowarena[row].slack {
                    mod2matrix_remove_row(scip, m, row);
                } else {
                    rowtable.remove(&RowOps(m), &identicalrow);
                    rowtable.insert(&RowOps(m), row);

                    let (rpos, ipos) = (m.rowarena[row].pos as usize, m.rowarena[identicalrow].pos as usize);
                    m.rows.swap(rpos, ipos);
                    m.rowarena[row].pos = ipos as i32;
                    m.rowarena[identicalrow].pos = rpos as i32;

                    mod2matrix_remove_row(scip, m, identicalrow);
                }
            } else {
                rowtable.insert(&RowOps(m), row);
                i += 1;
            }
        } else {
            // (d).
            generate_zerohalf_cut(scip, sol, m, sepa, st, allowlocal, row)?;

            if st.infeasible {
                break;
            }

            mod2matrix_remove_row(scip, m, row);
            i += 1;
        }
    }

    Ok(())
}

/// `mod2rowAddRow`: add a mod2 row to another one.
fn mod2row_add_row(scip: &Scip, m: &mut Mod2Matrix, row: usize, rowtoadd: usize) {
    let nlprows = if scip.tree.as_ref().unwrap().focuslpconstructed {
        scip.lp.as_ref().unwrap().nrows
    } else {
        0
    };

    m.rowarena[row].rhs ^= m.rowarena[rowtoadd].rhs;

    let newslack = m.rowarena[row].slack + m.rowarena[rowtoadd].slack;

    if scip.set.is_zero(m.rowarena[row].slack) && !scip.set.is_zero(newslack) {
        m.nzeroslackrows -= 1;
    }

    m.rowarena[row].slack = newslack;

    // The maximum index return by the UNIQUE_INDEX macro is 3 times the
    // maximum index value in the ROWINDEX struct. The index value could be
    // the lp position of an original row or the index of a transformed row.
    // Hence we need to allocate 3 times the maximum of these two possible
    // index types.
    let allocsize = 3 * max(nlprows as usize, m.transintrows.len());
    let mut contained = vec![false; allocsize];

    // Remember entries that are in the row to add.
    let addinds = m.rowarena[rowtoadd].rowinds.clone();
    for &ri in &addinds {
        contained[unique_index(ri)] = true;
    }

    // Remove the entries that are in both rows from the row (1 + 1 = 0 (mod
    // 2)).
    let r = &mut m.rowarena[row];
    let mut i = 0;
    while i < r.rowinds.len() {
        if contained[unique_index(r.rowinds[i])] {
            contained[unique_index(r.rowinds[i])] = false;
            r.rowinds.swap_remove(i);
        } else {
            i += 1;
        }
    }

    // Add remaining entries of row to add.
    for &ri in &addinds {
        if contained[unique_index(ri)] {
            contained[unique_index(ri)] = false;
            r.rowinds.push(ri);
        }
    }

    let rowcols = m.rowarena[row].nonzcols.clone();
    let addcols = m.rowarena[rowtoadd].nonzcols.clone();
    let mut newnonzcols = Vec::with_capacity(rowcols.len() + addcols.len());
    let (mut i, mut j) = (0, 0);
    m.rowarena[row].maxsolval = 0.0;

    // Since columns are sorted we can merge them.
    while i < rowcols.len() && j < addcols.len() {
        if rowcols[i] == addcols[j] {
            mod2col_unlink_row(m, rowcols[i], row);
            i += 1;
            j += 1;
        } else if m.colarena[rowcols[i]].index < m.colarena[addcols[j]].index {
            let r = &mut m.rowarena[row];
            r.maxsolval = max(r.maxsolval, m.colarena[rowcols[i]].solval);
            newnonzcols.push(rowcols[i]);
            i += 1;
        } else {
            mod2col_link_row(m, addcols[j], row);
            newnonzcols.push(addcols[j]);
            j += 1;
        }
    }

    while i < rowcols.len() {
        let r = &mut m.rowarena[row];
        r.maxsolval = max(r.maxsolval, m.colarena[rowcols[i]].solval);
        newnonzcols.push(rowcols[i]);
        i += 1;
    }

    while j < addcols.len() {
        mod2col_link_row(m, addcols[j], row);
        newnonzcols.push(addcols[j]);
        j += 1;
    }

    m.rowarena[row].nonzcols = newnonzcols;
}

/*
 * Callback methods of separator
 */

/// `sepaCopyZerohalf`: copy method for separator plugins.
fn sepa_copy_zerohalf(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_sepa_zerohalf(scip)
}

/// `sepaInitsolZerohalf`.
fn sepa_initsol_zerohalf(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Allocate random generator.
    let seed = scip.set.initialize_random_seed(data(scip, sepa).initseed as u32);
    data_mut(scip, sepa).randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `sepaExitsolZerohalf`.
fn sepa_exitsol_zerohalf(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Free random generator.
    data_mut(scip, sepa).randnumgen = None;
    Ok(())
}

/// `doSeparation`: perform the zerohalf cut separation.
fn do_separation(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    let d = data(scip, sepa);
    let ncalls = scip.sepas[sepa].ncallsatnode;

    // Only call the zerohalf cut separator a given number of times at each
    // node.
    if (depth == 0 && d.maxroundsroot >= 0 && ncalls >= d.maxroundsroot) || (depth > 0 && d.maxrounds >= 0 && ncalls >= d.maxrounds) {
        return Ok(());
    }

    let maxsepacuts = if depth == 0 { d.maxsepacutsroot } else { d.maxsepacuts };
    let mut maxslack = if depth == 0 { d.maxslackroot } else { d.maxslack };
    maxslack += 2.0 * scip.set.feastol();

    *result = ScipResultCode::DidNotFind;

    let (maxrowdensity, densityoffset) = (d.maxrowdensity, d.densityoffset);
    let mut st = Round {
        minviol: d.minviol,
        maxcutcands: d.maxcutcands,
        dynamiccuts: d.dynamiccuts,
        cuts: Vec::new(),
        nreductions: 0,
        infeasible: false,
    };

    let mut m = build_mod2_matrix(scip, sol, maxrowdensity, densityoffset, allowlocal, maxslack)?;

    for _ in 0..MAXREDUCTIONROUNDS {
        st.nreductions = 0;
        mod2matrix_preprocess_rows(scip, sol, &mut m, sepa, &mut st, allowlocal)?;

        if m.rows.is_empty() {
            break;
        }

        if st.cuts.len() as i32 >= st.maxcutcands {
            break;
        }

        mod2matrix_preprocess_columns(scip, &mut m, &mut st.nreductions);

        let mut ncancel = m.rows.len();
        let mut rows = std::mem::take(&mut m.rows);
        if ncancel > 100 {
            ncancel = 100;
            crate::sort::select(&mut rows[..], (), |a, b| compare_row_slack(&m, *a, *b), ncancel);
        }

        crate::sort::sort(&mut rows[..ncancel], (), |a, b| compare_row_slack(&m, *a, *b));
        m.rows = rows;

        if m.cols.is_empty() {
            break;
        }

        // Apply Prop5.
        for i in 0..ncancel {
            let row = m.rows[i];

            if scip.set.is_positive(m.rowarena[row].slack) || m.rowarena[row].nonzcols.is_empty() {
                continue;
            }

            let maxsolval = m.rowarena[row].maxsolval;
            let col = *m.rowarena[row].nonzcols.iter().find(|&&c| m.colarena[c].solval == maxsolval).unwrap();

            st.nreductions += 1;

            let nonzrows: Vec<usize> = m.colarena[col].nonzrows.iter().copied().filter(|&r| r != row).collect();

            for r in nonzrows {
                mod2row_add_row(scip, &mut m, r, row);
            }

            m.rowarena[row].slack = m.colarena[col].solval;
            m.nzeroslackrows -= 1;

            mod2matrix_remove_col(&mut m, col);
        }

        if st.nreductions == 0 {
            break;
        }
    }

    let mut i = 0;
    while i < m.rows.len() && (st.cuts.len() as i32) < st.maxcutcands {
        let row = m.rows[i];

        if compute_max_violation(&m.rowarena[row]) < st.minviol {
            break;
        }

        if m.rowarena[row].rhs == 0 {
            i += 1;
            continue;
        }

        generate_zerohalf_cut(scip, sol, &m, sepa, &mut st, allowlocal, row)?;
        i += 1;
    }

    // If cuts where found we apply a filtering procedure using the scores and
    // the orthogonalities, similar to the sepastore. We only add the cuts that
    // make it through this process and discard the rest.
    if !st.cuts.is_empty() {
        let d = data(scip, sepa);
        let (goodscore, badscore, goodmaxparall, maxparall, dircutoffdistweight, efficacyweight, objparalweight) = (
            d.goodscore,
            d.badscore,
            d.goodmaxparall,
            d.maxparall,
            d.dircutoffdistweight,
            d.efficacyweight,
            d.objparalweight,
        );
        let mut randnumgen = data_mut(scip, sepa).randnumgen.take();
        let mut nselectedcuts = 0;
        let r = crate::cutsel_hybrid::select_cuts_hybrid(
            scip,
            &mut st.cuts,
            &[],
            randnumgen.as_mut(),
            goodscore,
            badscore,
            goodmaxparall,
            maxparall,
            dircutoffdistweight,
            efficacyweight,
            objparalweight,
            0.0,
            maxsepacuts,
            &mut nselectedcuts,
        );
        data_mut(scip, sepa).randnumgen = randnumgen;
        r?;

        for i in 0..st.cuts.len() {
            if (i as i32) < nselectedcuts {
                // If selected, add global cuts to the pool and local cuts to the
                // sepastore.
                if scip.rows[st.cuts[i]].local {
                    st.infeasible = scip_lp::add_row(scip, st.cuts[i], false)?;
                } else {
                    api::add_pool_cut(scip, st.cuts[i])?;
                }
            }

            // Release current cut.
            scip_lp::release_row(scip, &mut st.cuts[i])?;
        }

        *result = if st.infeasible { ScipResultCode::Cutoff } else { ScipResultCode::Separated };
    }

    let d = data_mut(scip, sepa);
    d.infeasible = st.infeasible;
    d.nreductions = st.nreductions;

    Ok(())
}

/// `sepaExeclpZerohalf`: LP solution separation method of separator.
fn sepa_execlp_zerohalf(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call separator, if we are not close to terminating.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Only call separator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return Ok(());
    }

    // Only call separator, if there are fractional variables.
    if api::get_n_lp_branch_cands(scip) == 0 {
        return Ok(());
    }

    do_separation(scip, sepa, NONE, result, allowlocal, depth)
}

/// `sepaExecsolZerohalf`: custom solution separation method of separator.
fn sepa_execsol_zerohalf(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call separator, if we are not close to terminating.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    do_separation(scip, sepa, sol, result, allowlocal, depth)
}

/// `SCIPincludeSepaZerohalf`: creates the zerohalf separator and includes
/// it in SCIP.
pub fn include_sepa_zerohalf(scip: &mut Scip) -> ScipResult {
    // Create zerohalf separator data.
    let sepadata = SepaData::default();

    // Include separator.
    let sepa = api::include_sepa_basic(
        scip,
        SEPA_NAME,
        SEPA_DESC,
        SEPA_PRIORITY,
        SEPA_FREQ,
        SEPA_MAXBOUNDDIST,
        SEPA_USESSUBSCIP,
        SEPA_DELAY,
        Some(sepa_execlp_zerohalf),
        Some(sepa_execsol_zerohalf),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    let s = &mut scip.sepas[sepa];
    s.sepacopy = Some(sepa_copy_zerohalf);
    s.sepainitsol = Some(sepa_initsol_zerohalf);
    s.sepaexitsol = Some(sepa_exitsol_zerohalf);

    // Add zerohalf separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    let name = |p: &str| format!("separating/{SEPA_NAME}/{p}");
    paramset_add_int(
        scip,
        &name("maxrounds"),
        "maximal number of zerohalf separation rounds per node (-1: unlimited)",
        target!(i32, maxrounds),
        false,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxroundsroot"),
        "maximal number of zerohalf separation rounds in the root node (-1: unlimited)",
        target!(i32, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxsepacuts"),
        "maximal number of zerohalf cuts separated per separation round",
        target!(i32, maxsepacuts),
        false,
        DEFAULT_MAXSEPACUTS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("initseed"),
        "initial seed used for random tie-breaking in cut selection",
        target!(i32, initseed),
        false,
        DEFAULT_INITSEED,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxsepacutsroot"),
        "maximal number of zerohalf cuts separated per separation round in the root node",
        target!(i32, maxsepacutsroot),
        false,
        DEFAULT_MAXSEPACUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxcutcands"),
        "maximal number of zerohalf cuts considered per separation round",
        target!(i32, maxcutcands),
        false,
        DEFAULT_MAXCUTCANDS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxslack"),
        "maximal slack of rows to be used in aggregation",
        target!(f64, maxslack),
        true,
        DEFAULT_MAXSLACK,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxslackroot"),
        "maximal slack of rows to be used in aggregation in the root node",
        target!(f64, maxslackroot),
        true,
        DEFAULT_MAXSLACKROOT,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("goodscore"),
        "threshold for score of cut relative to best score to be considered good, so that less strict filtering is applied",
        target!(f64, goodscore),
        true,
        DEFAULT_GOODSCORE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("badscore"),
        "threshold for score of cut relative to best score to be discarded",
        target!(f64, badscore),
        true,
        DEFAULT_BADSCORE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("objparalweight"),
        "weight of objective parallelism in cut score calculation",
        target!(f64, objparalweight),
        true,
        DEFAULT_OBJPARALWEIGHT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("efficacyweight"),
        "weight of efficacy in cut score calculation",
        target!(f64, efficacyweight),
        true,
        DEFAULT_EFFICACYWEIGHT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("dircutoffdistweight"),
        "weight of directed cutoff distance in cut score calculation",
        target!(f64, dircutoffdistweight),
        true,
        DEFAULT_DIRCUTOFFDISTWEIGHT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("goodmaxparall"),
        "maximum parallelism for good cuts",
        target!(f64, goodmaxparall),
        true,
        DEFAULT_GOODMAXPARALL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxparall"),
        "maximum parallelism for non-good cuts",
        target!(f64, maxparall),
        true,
        DEFAULT_MAXPARALL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("minviol"),
        "minimal violation to generate zerohalfcut for",
        target!(f64, minviol),
        true,
        DEFAULT_MINVIOL,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("dynamiccuts"),
        "should generated cuts be removed from the LP if they are no longer tight?",
        target!(bool, dynamiccuts),
        false,
        DEFAULT_DYNAMICCUTS,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxrowdensity"),
        "maximal density of row to be used in aggregation",
        target!(f64, maxrowdensity),
        true,
        DEFAULT_MAXROWDENSITY,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("densityoffset"),
        "additional number of variables allowed in row on top of density",
        target!(i32, densityoffset),
        true,
        DEFAULT_DENSITYOFFSET,
        0,
        i32::MAX,
        None,
    )?;

    Ok(())
}
