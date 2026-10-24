//! SCIP's LP relaxation (`lp.c`, `struct_lp.h`): columns, rows, and the
//! LP that keeps them in step with the LP solver.

use crate::{
    cvec::CVec,
    types::{ColId, LpAlgo, LpSolStat, RowId, RowOriginType, SideType, SolId, VarId},
};

/// `SCIP_COLSOLVALS`.
#[derive(Clone, Debug, Default)]
pub struct ColSolVals {
    pub primsol: f64,
    pub redcost: f64,
    pub basisstatus: u32,
}
/// `SCIP_ROWSOLVALS`.
#[derive(Clone, Debug, Default)]
pub struct RowSolVals {
    pub dualsol: f64,
    pub activity: f64,
    pub basisstatus: u32,
}
/// `SCIP_LPSOLVALS`.
#[derive(Clone, Debug, Default)]
pub struct LpSolVals {
    pub lpsolstat: LpSolStat,
    pub lpobjval: f64,
    pub primalfeasible: bool,
    pub primalchecked: bool,
    pub dualfeasible: bool,
    pub dualchecked: bool,
    pub solisbasic: bool,
    pub lpissolved: bool,
    pub hasprovedboundexact: bool,
}
/// `SCIP_COL`.
#[derive(Clone, Debug)]
pub struct Col {
    pub obj: f64,
    pub lb: f64,
    pub ub: f64,
    pub unchangedobj: f64,
    pub lazylb: f64,
    pub lazyub: f64,
    pub flushedobj: f64,
    pub flushedlb: f64,
    pub flushedub: f64,
    pub primsol: f64,
    pub redcost: f64,
    pub farkascoef: f64,
    pub minprimsol: f64,
    pub maxprimsol: f64,
    pub sbdown: f64,
    pub sbup: f64,
    pub sbsolval: f64,
    pub sblpobjval: f64,
    pub sbnode: i64,
    pub obsoletenode: i64,
    pub storedsolvals: Option<Box<ColSolVals>>,
    pub var: VarId,
    pub rows: CVec<RowId>,
    pub vals: CVec<f64>,
    pub validredcostlp: i64,
    pub validfarkaslp: i64,
    pub validsblp: i64,
    pub linkpos: CVec<i32>,
    pub index: i32,
    pub size: i32,
    pub len: i32,
    pub nlprows: i32,
    pub nunlinked: i32,
    pub lppos: i32,
    pub lpipos: i32,
    pub lpdepth: i32,
    pub sbitlim: i32,
    pub nsbcalls: i32,
    pub age: i32,
    pub var_probindex: i32,
    pub basisstatus: u32,
    pub lprowssorted: bool,
    pub nonlprowssorted: bool,
    pub objchanged: bool,
    pub lbchanged: bool,
    pub ubchanged: bool,
    pub coefchanged: bool,
    pub integral: bool,
    pub impliedintegral: bool,
    pub removable: bool,
    pub sbdownvalid: bool,
    pub sbupvalid: bool,
}
/// `SCIP_ROW`.
#[derive(Clone, Debug)]
pub struct Row {
    pub constant: f64,
    pub lhs: f64,
    pub rhs: f64,
    pub flushedlhs: f64,
    pub flushedrhs: f64,
    pub sqrnorm: f64,
    pub sumnorm: f64,
    pub objprod: f64,
    pub maxval: f64,
    pub minval: f64,
    pub dualsol: f64,
    pub activity: f64,
    pub dualfarkas: f64,
    pub pseudoactivity: f64,
    pub minactivity: f64,
    pub maxactivity: f64,
    pub validpsactivitydomchg: i64,
    pub validactivitybdsdomchg: i64,
    pub obsoletenode: i64,
    pub activeinlpcounter: i64,
    pub nlpsaftercreation: i64,
    pub storedsolvals: Option<Box<RowSolVals>>,
    pub origin: usize,
    pub name: String,
    pub cols: CVec<ColId>,
    pub cols_index: CVec<i32>,
    pub vals: CVec<f64>,
    pub linkpos: CVec<i32>,
    pub eventfilter: usize,
    pub validactivitylp: i64,
    pub index: i32,
    pub size: i32,
    pub len: i32,
    pub nlpcols: i32,
    pub nunlinked: i32,
    pub nuses: i32,
    pub lppos: i32,
    pub lpipos: i32,
    pub lpdepth: i32,
    pub minidx: i32,
    pub maxidx: i32,
    pub numintcols: i32,
    pub numimplintcols: i32,
    pub nummaxval: i32,
    pub numminval: i32,
    pub age: i32,
    pub rank: i32,
    pub fromcutpool: bool,
    pub basisstatus: u32,
    pub lpcolssorted: bool,
    pub nonlpcolssorted: bool,
    pub delaysort: bool,
    pub validminmaxidx: bool,
    pub lhschanged: bool,
    pub rhschanged: bool,
    pub coefchanged: bool,
    pub integral: bool,
    pub local: bool,
    pub modifiable: bool,
    pub removable: bool,
    pub inglobalcutpool: bool,
    pub normunreliable: bool,
    pub nlocks: u32,
    pub origintype: RowOriginType,
}
/// `SCIP_LP`.
pub struct Lp {
    pub lpobjval: f64,
    pub looseobjval: f64,
    pub rellooseobjval: f64,
    pub glbpseudoobjval: f64,
    pub relglbpseudoobjval: f64,
    pub pseudoobjval: f64,
    pub relpseudoobjval: f64,
    pub rootlpobjval: f64,
    pub rootlooseobjval: f64,
    pub cutoffbound: f64,
    pub feastol: f64,
    pub lpiobjlim: f64,
    pub lpifeastol: f64,
    pub lpidualfeastol: f64,
    pub lpibarrierconvtol: f64,
    pub lpiconditionlimit: f64,
    pub lpimarkowitz: f64,
    pub objsqrnorm: f64,
    pub objsumnorm: f64,
    pub degeneracy: f64,
    pub varconsratio: f64,
    pub lpi: Option<Box<crate::lpi::Lpi>>,
    pub lpicols: CVec<ColId>,
    pub lpirows: CVec<RowId>,
    pub chgcols: CVec<ColId>,
    pub chgrows: CVec<RowId>,
    pub cols: CVec<ColId>,
    pub lazycols: CVec<ColId>,
    pub rows: CVec<RowId>,
    pub soldirection: CVec<f64>,
    pub divelpistate: Option<Box<crate::lpi::LpiState>>,
    pub divechgsides: CVec<f64>,
    pub divechgsidetypes: CVec<SideType>,
    pub divechgrows: CVec<RowId>,
    pub storedsolvals: Option<Box<LpSolVals>>,
    pub validsoldirsol: SolId,
    pub validsollp: i64,
    pub validfarkaslp: i64,
    pub validsoldirlp: i64,
    pub validdegeneracylp: i64,
    pub divenolddomchgs: i64,
    pub lpicolssize: i32,
    pub nlpicols: i32,
    pub lpifirstchgcol: i32,
    pub lpirowssize: i32,
    pub nlpirows: i32,
    pub lpifirstchgrow: i32,
    pub chgcolssize: i32,
    pub nchgcols: i32,
    pub chgrowssize: i32,
    pub nchgrows: i32,
    pub colssize: i32,
    pub soldirectionsize: i32,
    pub ncols: i32,
    pub lazycolssize: i32,
    pub nlazycols: i32,
    pub nremovablecols: i32,
    pub firstnewcol: i32,
    pub rowssize: i32,
    pub nrows: i32,
    pub nremovablerows: i32,
    pub firstnewrow: i32,
    pub looseobjvalinf: i32,
    pub nloosevars: i32,
    pub glbpseudoobjvalinf: i32,
    pub pseudoobjvalinf: i32,
    pub ndivingrows: i32,
    pub ndivechgsides: i32,
    pub divechgsidessize: i32,
    pub divinglpiitlim: i32,
    pub lpiitlim: i32,
    pub lpifastmip: i32,
    pub lpithreads: i32,
    pub lpitiming: i32,
    pub lpirandomseed: i32,
    pub lpiscaling: i32,
    pub lpirefactorinterval: i32,
    pub lpipricing: crate::lpi::Pricing,
    pub lpsolstat: LpSolStat,
    pub lastlpalgo: LpAlgo,
    pub objsqrnormunreliable: bool,
    pub lpisolutionpolishing: bool,
    pub looseobjvalid: bool,
    pub glbpseudoobjvalid: bool,
    pub pseudoobjvalid: bool,
    pub flushdeletedcols: bool,
    pub flushaddedcols: bool,
    pub flushdeletedrows: bool,
    pub flushaddedrows: bool,
    pub updateintegrality: bool,
    pub flushed: bool,
    pub solved: bool,
    pub primalfeasible: bool,
    pub primalchecked: bool,
    pub dualfeasible: bool,
    pub dualchecked: bool,
    pub solisbasic: bool,
    pub rootlpisrelax: bool,
    pub isrelax: bool,
    pub installing: bool,
    pub strongbranching: bool,
    pub probing: bool,
    pub strongbranchprobing: bool,
    pub diving: bool,
    pub divingobjchg: bool,
    pub divinglazyapplied: bool,
    pub resolvelperror: bool,
    pub adjustlpval: bool,
    pub lpifromscratch: bool,
    pub lpipresolving: bool,
    pub lpilpinfo: bool,
    pub lpihasfeastol: bool,
    pub lpihasdualfeastol: bool,
    pub lpihasbarrierconvtol: bool,
    pub lpihasfastmip: bool,
    pub lpihasscaling: bool,
    pub lpihaspresolving: bool,
    pub lpihasrowrep: bool,
    pub lpihaspolishing: bool,
    pub lpihasrefactor: bool,
    pub lpirowrepswitch: f64,
    pub divelpwasprimfeas: bool,
    pub divelpwasprimchecked: bool,
    pub divelpwasdualfeas: bool,
    pub divelpwasdualchecked: bool,
    pub hasprovedbound: bool,
}

use crate::{
    def::INVALID,
    lpi::{self, BASESTAT_BASIC, BASESTAT_ZERO, LpParam, Pricing},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    sort,
    types::VarStatus,
};

/*
 * memory growing methods for dynamically allocated arrays
 */

/// Grows one of the LP's arrays like `ensureChgcolsSize` and its siblings.
macro_rules! lp_ensure {
    ($name:ident, $arr:ident, $size:ident) => {
        fn $name(scip: &mut Scip, num: i32) {
            if num > lp!(scip).$size {
                let newsize = scip.set.calc_mem_grow_size(num);
                let lp = &mut lp!(scip);
                lp.$arr.realloc(newsize);
                lp.$size = newsize;
            }
        }
    };
}

lp_ensure!(ensure_chgcols_size, chgcols, chgcolssize);
lp_ensure!(ensure_chgrows_size, chgrows, chgrowssize);
lp_ensure!(ensure_lpicols_size, lpicols, lpicolssize);
lp_ensure!(ensure_lpirows_size, lpirows, lpirowssize);
lp_ensure!(ensure_cols_size, cols, colssize);
lp_ensure!(ensure_lazycols_size, lazycols, lazycolssize);
lp_ensure!(ensure_rows_size, rows, rowssize);

/// `ensureSoldirectionSize`: frees and reallocates without growth.
fn ensure_soldirection_size(scip: &mut Scip, num: i32) {
    let lp = &mut lp!(scip);
    if num > lp.soldirectionsize {
        lp.soldirection = CVec::with_size(num);
        lp.soldirectionsize = num;
    }
}

/// `colEnsureSize`.
fn col_ensure_size(scip: &mut Scip, col: ColId, num: i32) {
    if num > scip.cols[col].size {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = &mut scip.cols[col];
        c.rows.realloc(newsize);
        c.vals.realloc(newsize);
        c.linkpos.realloc(newsize);
        c.size = newsize;
    }
}

/// `lpStoreSolVals`: saves the current LP solution status.
fn lp_store_sol_vals(scip: &mut Scip) {
    let lp = &mut lp!(scip);
    let storedsolvals = lp.storedsolvals.get_or_insert_with(Default::default);
    storedsolvals.lpsolstat = lp.lpsolstat;
    storedsolvals.lpobjval = lp.lpobjval;
    storedsolvals.primalfeasible = lp.primalfeasible;
    storedsolvals.primalchecked = lp.primalchecked;
    storedsolvals.dualfeasible = lp.dualfeasible;
    storedsolvals.dualchecked = lp.dualchecked;
    storedsolvals.solisbasic = lp.solisbasic;
    storedsolvals.lpissolved = lp.solved;
    storedsolvals.hasprovedboundexact = lp.hasprovedbound;
}

/// `lpRestoreSolVals`: restores the LP solution status saved by
/// [`lp_store_sol_vals`].
fn lp_restore_sol_vals(scip: &mut Scip, validlp: i64) {
    let lp = &mut lp!(scip);
    if let Some(storedsolvals) = lp.storedsolvals.as_ref() {
        lp.solved = storedsolvals.lpissolved;
        lp.validsollp = validlp;
        lp.lpsolstat = storedsolvals.lpsolstat;
        lp.lpobjval = storedsolvals.lpobjval;
        lp.primalfeasible = storedsolvals.primalfeasible;
        lp.primalchecked = storedsolvals.primalchecked;
        lp.dualfeasible = storedsolvals.dualfeasible;
        lp.dualchecked = storedsolvals.dualchecked;
        lp.solisbasic = storedsolvals.solisbasic;
        lp.hasprovedbound = storedsolvals.hasprovedboundexact;
    } else {
        lp.solved = false;
        lp.validsollp = -1;
        lp.lpsolstat = LpSolStat::NotSolved;
        lp.lpobjval = INVALID;
        lp.primalfeasible = false;
        lp.primalchecked = false;
        lp.dualfeasible = false;
        lp.dualchecked = false;
        lp.solisbasic = false;
        lp.validfarkaslp = -1;
    }
    lp.validdegeneracylp = -1;
}

/// `colStoreSolVals`.
fn col_store_sol_vals(scip: &mut Scip, col: ColId) {
    let c = &mut scip.cols[col];
    let storedsolvals = c.storedsolvals.get_or_insert_with(Default::default);
    storedsolvals.primsol = c.primsol;
    storedsolvals.redcost = c.redcost;
    storedsolvals.basisstatus = c.basisstatus;
}

/// `colRestoreSolVals`.
fn col_restore_sol_vals(scip: &mut Scip, col: ColId, validlp: i64, freebuffer: bool) {
    let c = &mut scip.cols[col];
    if let Some(storedsolvals) = c.storedsolvals.as_ref() {
        c.primsol = storedsolvals.primsol;
        c.redcost = storedsolvals.redcost;
        c.validredcostlp = validlp;
        c.basisstatus = storedsolvals.basisstatus;

        // We do not save the farkas coefficient, since it can be
        // recomputed; thus, we invalidate it here.
        c.validfarkaslp = -1;
    } else {
        // If the column was created after performing the storage (possibly
        // during probing), we treat it as implicitly zero.
        c.primsol = 0.0;
        c.validredcostlp = -1;
        c.validfarkaslp = -1;
        c.basisstatus = BASESTAT_ZERO;
    }
    if freebuffer {
        c.storedsolvals = None;
    }
}

/// `rowStoreSolVals`.
fn row_store_sol_vals(scip: &mut Scip, row: RowId, infeasible: bool) {
    let r = &mut scip.rows[row];
    let storedsolvals = r.storedsolvals.get_or_insert_with(Default::default);
    if infeasible {
        storedsolvals.dualsol = r.dualfarkas;
        storedsolvals.activity = INVALID;
        storedsolvals.basisstatus = BASESTAT_BASIC;
    } else {
        storedsolvals.dualsol = r.dualsol;
        storedsolvals.activity = r.activity;
        storedsolvals.basisstatus = r.basisstatus;
    }
}

/// `rowRestoreSolVals`.
fn row_restore_sol_vals(scip: &mut Scip, row: RowId, validlp: i64, freebuffer: bool, infeasible: bool) {
    let r = &mut scip.rows[row];
    if let Some(storedsolvals) = r.storedsolvals.as_ref() {
        if infeasible {
            r.dualfarkas = storedsolvals.dualsol;
        } else {
            r.dualsol = storedsolvals.dualsol;
        }
        r.activity = storedsolvals.activity;
        r.validactivitylp = validlp;
        r.basisstatus = storedsolvals.basisstatus;
    } else {
        // If the row was created after performing the storage (possibly
        // during probing), we treat it as basic with zero dual value.
        r.dualsol = 0.0;
        r.dualfarkas = 0.0;
        r.activity = INVALID;
        r.validactivitylp = -1;
        r.basisstatus = BASESTAT_BASIC;
    }
    if freebuffer {
        r.storedsolvals = None;
    }
}

/// `SCIProwEnsureSize`.
pub fn row_ensure_size(scip: &mut Scip, row: RowId, num: i32) {
    if num > scip.rows[row].size {
        let newsize = scip.set.calc_mem_grow_size(num);
        let r = &mut scip.rows[row];
        r.cols.realloc(newsize);
        r.cols_index.realloc(newsize);
        r.vals.realloc(newsize);
        r.linkpos.realloc(newsize);
        r.size = newsize;
    }
}

/*
 * Local methods for pseudo and loose objective values
 */

/// `recomputeLooseObjectiveValue` (exact solving is off).
fn recompute_loose_objective_value(scip: &mut Scip, prob: usize) {
    lp!(scip).looseobjval = 0.0;
    let nvars = scip.probs[prob].nvars;
    for v in 0..nvars {
        let var = scip.probs[prob].vars[v];
        let x = &scip.vars[var];
        if x.varstatus == VarStatus::Loose {
            // Skip variables with infinite best bound.
            let obj = x.obj;
            if scip.set.is_positive(obj) && !scip.set.is_infinity(-x.locdom.lb) {
                lp!(scip).looseobjval += obj * x.locdom.lb;
            } else if scip.set.is_negative(obj) && !scip.set.is_infinity(x.locdom.ub) {
                lp!(scip).looseobjval += obj * x.locdom.ub;
            }
        }
    }

    // The recomputed value is reliable.
    let lp = &mut lp!(scip);
    lp.rellooseobjval = lp.looseobjval;
    lp.looseobjvalid = true;
}

/// `recomputePseudoObjectiveValue` (exact solving is off).
fn recompute_pseudo_objective_value(scip: &mut Scip, prob: usize) {
    lp!(scip).pseudoobjval = 0.0;
    let nvars = scip.probs[prob].nvars;
    for v in 0..nvars {
        let x = &scip.vars[scip.probs[prob].vars[v]];
        // Skip variables with infinite best bound.
        if scip.set.is_positive(x.obj) && !scip.set.is_infinity(-x.locdom.lb) {
            lp!(scip).pseudoobjval += x.obj * x.locdom.lb;
        } else if scip.set.is_negative(x.obj) && !scip.set.is_infinity(x.locdom.ub) {
            lp!(scip).pseudoobjval += x.obj * x.locdom.ub;
        }
    }

    // The recomputed value is reliable.
    let lp = &mut lp!(scip);
    lp.relpseudoobjval = lp.pseudoobjval;
    lp.pseudoobjvalid = true;
}

/// `recomputeGlbPseudoObjectiveValue`.
fn recompute_glb_pseudo_objective_value(scip: &mut Scip, prob: usize) {
    lp!(scip).glbpseudoobjval = 0.0;
    let nvars = scip.probs[prob].nvars;
    for v in 0..nvars {
        let x = &scip.vars[scip.probs[prob].vars[v]];
        // Skip variables with infinite best bound.
        if scip.set.is_positive(x.obj) && !scip.set.is_infinity(-x.glbdom.lb) {
            lp!(scip).glbpseudoobjval += x.obj * x.glbdom.lb;
        } else if scip.set.is_negative(x.obj) && !scip.set.is_infinity(x.glbdom.ub) {
            lp!(scip).glbpseudoobjval += x.obj * x.glbdom.ub;
        }
    }

    // The recomputed value is reliable.
    let lp = &mut lp!(scip);
    lp.relglbpseudoobjval = lp.glbpseudoobjval;
    lp.glbpseudoobjvalid = true;
}

/// `getFiniteLooseObjval`.
fn get_finite_loose_objval(scip: &mut Scip, prob: usize) -> f64 {
    if !lp!(scip).looseobjvalid {
        recompute_loose_objective_value(scip, prob);
    }
    lp!(scip).looseobjval
}

/// `getFinitePseudoObjval`.
fn get_finite_pseudo_objval(scip: &mut Scip, prob: usize) -> f64 {
    if !lp!(scip).pseudoobjvalid {
        recompute_pseudo_objective_value(scip, prob);
    }
    lp!(scip).pseudoobjval
}

/*
 * Sorting and searching rows and columns
 */

/// `SCIProwComp` on the rows' indices.
pub fn row_comp(scip: &Scip, row1: RowId, row2: RowId) -> i32 {
    let (i1, i2) = (scip.rows[row1].index, scip.rows[row2].index);
    (i1 > i2) as i32 - (i1 < i2) as i32
}

/// Sorts `col`'s entries `start..end` by row index, then repairs the rows'
/// links back to them (`colSortLP` and `colSortNonLP`).
fn col_sort_part(scip: &mut Scip, col: ColId, start: i32, end: i32) {
    let Col { rows, vals, linkpos, .. } = &mut scip.cols[col];
    let srows = &scip.rows;
    let (s, e) = (start as usize, end as usize);
    sort::sort(&mut rows.0[s..e], (&mut vals.0[s..e], &mut linkpos.0[s..e]), |a, b| {
        let (i1, i2) = (srows[*a].index, srows[*b].index);
        (i1 > i2) as i32 - (i1 < i2) as i32
    });

    // Update links.
    for i in start..end {
        let c = &scip.cols[col];
        if c.linkpos[i] >= 0 {
            let (r, lp) = (c.rows[i], c.linkpos[i]);
            scip.rows[r].linkpos[lp] = i;
        }
    }
}

/// `colSortLP`: sorts column entries of LP rows by row index.
fn col_sort_lp(scip: &mut Scip, col: ColId) {
    if scip.cols[col].lprowssorted {
        return;
    }
    let n = scip.cols[col].nlprows;
    col_sort_part(scip, col, 0, n);
    scip.cols[col].lprowssorted = true;
}

/// `colSortNonLP`: sorts column entries of non-LP rows by row index.
fn col_sort_non_lp(scip: &mut Scip, col: ColId) {
    if scip.cols[col].nonlprowssorted {
        return;
    }
    let (s, e) = (scip.cols[col].nlprows, scip.cols[col].len);
    col_sort_part(scip, col, s, e);
    scip.cols[col].nonlprowssorted = true;
}

/// Sorts `row`'s entries `start..end` by column index, then repairs the
/// columns' links back to them (`rowSortLP` and `rowSortNonLP`).
fn row_sort_part(scip: &mut Scip, row: RowId, start: i32, end: i32) {
    let Row {
        cols_index, cols, linkpos, vals, ..
    } = &mut scip.rows[row];
    let (s, e) = (start as usize, end as usize);
    sort::sort(&mut cols_index.0[s..e], (&mut cols.0[s..e], &mut linkpos.0[s..e], &mut vals.0[s..e]), sort::cmp_int);

    // Update links.
    for i in start..end {
        let r = &scip.rows[row];
        if r.linkpos[i] >= 0 {
            let (c, lp) = (r.cols[i], r.linkpos[i]);
            scip.cols[c].linkpos[lp] = i;
        }
    }
}

/// `rowSortLP`: sorts row entries of LP columns by column index.
fn row_sort_lp(scip: &mut Scip, row: RowId) {
    let r = &scip.rows[row];
    if r.lpcolssorted || r.delaysort {
        return;
    }
    let n = r.nlpcols;
    row_sort_part(scip, row, 0, n);
    scip.rows[row].lpcolssorted = true;
}

/// `rowSortNonLP`: sorts row entries of non-LP columns by column index.
fn row_sort_non_lp(scip: &mut Scip, row: RowId) {
    let r = &scip.rows[row];
    if r.nonlpcolssorted || r.delaysort {
        return;
    }
    let (s, e) = (r.nlpcols, r.len);
    row_sort_part(scip, row, s, e);
    scip.rows[row].nonlpcolssorted = true;
}

/// `colSearchCoefPart`: binary search in `minpos..=maxpos`; -1 if absent.
fn col_search_coef_part(scip: &Scip, col: ColId, row: RowId, mut minpos: i32, mut maxpos: i32) -> i32 {
    let searchidx = scip.rows[row].index;
    let c = &scip.cols[col];
    while minpos <= maxpos {
        let pos = (minpos + maxpos) / 2;
        let idx = scip.rows[c.rows[pos]].index;
        if searchidx == idx {
            return pos;
        } else if searchidx < idx {
            maxpos = pos - 1;
        } else {
            minpos = pos + 1;
        }
    }
    -1
}

/// `colSearchCoef`: the position of `row` in `col`'s vectors, or -1.
fn col_search_coef(scip: &mut Scip, col: ColId, row: RowId) -> i32 {
    let mut pos = -1;

    // Search in the linked LP rows.
    if scip.rows[row].lppos >= 0 {
        col_sort_lp(scip, col);
        pos = col_search_coef_part(scip, col, row, 0, scip.cols[col].nlprows - 1);
        if pos >= 0 {
            return pos;
        }
    }

    // Search in the non-LP/unlinked rows.
    if scip.rows[row].lppos == -1 || scip.cols[col].nunlinked > 0 {
        col_sort_non_lp(scip, col);
        pos = col_search_coef_part(scip, col, row, scip.cols[col].nlprows, scip.cols[col].len - 1);
    }
    pos
}

/// `rowSearchCoefPart`: binary search in `minpos..=maxpos`; -1 if absent.
fn row_search_coef_part(scip: &Scip, row: RowId, col: ColId, mut minpos: i32, mut maxpos: i32) -> i32 {
    let searchidx = scip.cols[col].index;
    let r = &scip.rows[row];
    while minpos <= maxpos {
        let pos = (minpos + maxpos) / 2;
        let idx = r.cols_index[pos];
        if searchidx == idx {
            return pos;
        } else if searchidx < idx {
            maxpos = pos - 1;
        } else {
            minpos = pos + 1;
        }
    }
    -1
}

/// `rowSearchCoef`: the position of `col` in `row`'s vectors, or -1 (also
/// while sorting is delayed).
fn row_search_coef(scip: &mut Scip, row: RowId, col: ColId) -> i32 {
    if scip.rows[row].delaysort {
        return -1;
    }
    let mut pos = -1;

    // Search in the linked LP columns.
    if scip.cols[col].lppos >= 0 {
        row_sort_lp(scip, row);
        pos = row_search_coef_part(scip, row, col, 0, scip.rows[row].nlpcols - 1);
    }

    // Search in the non-LP/unlinked columns.
    if pos == -1 && (scip.cols[col].lppos == -1 || scip.rows[row].nunlinked > 0) {
        row_sort_non_lp(scip, row);
        pos = row_search_coef_part(scip, row, col, scip.rows[row].nlpcols, scip.rows[row].len - 1);
    }
    pos
}

/// `colMoveCoef`: moves a coefficient in a column to a different place,
/// and updates all corresponding data structures.
fn col_move_coef(scip: &mut Scip, col: ColId, oldpos: i32, newpos: i32) {
    if oldpos == newpos {
        return;
    }
    let c = &mut scip.cols[col];
    c.rows[newpos] = c.rows[oldpos];
    c.vals[newpos] = c.vals[oldpos];
    c.linkpos[newpos] = c.linkpos[oldpos];

    // Update link position in row.
    let (row, linkpos) = (c.rows[newpos], c.linkpos[newpos]);
    if linkpos >= 0 {
        scip.rows[row].linkpos[linkpos] = newpos;
    }

    // Update sorted flags.
    let c = &mut scip.cols[col];
    if scip.rows[row].lppos >= 0 && linkpos >= 0 {
        c.lprowssorted = false;
    } else {
        c.nonlprowssorted = false;
    }
}

/// `colSwapCoefs`: swaps two coefficients in a column, and updates all
/// corresponding data structures.
fn col_swap_coefs(scip: &mut Scip, col: ColId, pos1: i32, pos2: i32) {
    if pos1 == pos2 {
        return;
    }

    // Swap coefficients.
    let c = &mut scip.cols[col];
    let tmprow = c.rows[pos2];
    let tmpval = c.vals[pos2];
    let tmplinkpos = c.linkpos[pos2];
    c.rows[pos2] = c.rows[pos1];
    c.vals[pos2] = c.vals[pos1];
    c.linkpos[pos2] = c.linkpos[pos1];
    c.rows[pos1] = tmprow;
    c.vals[pos1] = tmpval;
    c.linkpos[pos1] = tmplinkpos;

    // Update link position in rows.
    let (row1, link1, row2, link2) = (c.rows[pos1], c.linkpos[pos1], c.rows[pos2], c.linkpos[pos2]);
    if link1 >= 0 {
        scip.rows[row1].linkpos[link1] = pos1;
    }
    if link2 >= 0 {
        scip.rows[row2].linkpos[link2] = pos2;
    }

    // Update sorted flags.
    let (lp1, lp2) = (scip.rows[row1].lppos, scip.rows[row2].lppos);
    let c = &mut scip.cols[col];
    if lp1 >= 0 && link1 >= 0 {
        c.lprowssorted = false;
    } else {
        c.nonlprowssorted = false;
    }
    if lp2 >= 0 && link2 >= 0 {
        c.lprowssorted = false;
    } else {
        c.nonlprowssorted = false;
    }
}

/// `rowMoveCoef`: moves a coefficient in a row to a different place, and
/// updates all corresponding data structures.
fn row_move_coef(scip: &mut Scip, row: RowId, oldpos: i32, newpos: i32) {
    if oldpos == newpos {
        return;
    }
    let r = &mut scip.rows[row];
    r.cols[newpos] = r.cols[oldpos];
    r.cols_index[newpos] = r.cols_index[oldpos];
    r.vals[newpos] = r.vals[oldpos];
    r.linkpos[newpos] = r.linkpos[oldpos];

    // Update link position in column.
    let (col, linkpos) = (r.cols[newpos], r.linkpos[newpos]);
    if linkpos >= 0 {
        scip.cols[col].linkpos[linkpos] = newpos;
    }

    // Update sorted flags.
    let r = &mut scip.rows[row];
    if scip.cols[col].lppos >= 0 && linkpos >= 0 {
        r.lpcolssorted = false;
    } else {
        r.nonlpcolssorted = false;
    }
}

/// `rowSwapCoefs`: swaps two coefficients in a row, and updates all
/// corresponding data structures.
fn row_swap_coefs(scip: &mut Scip, row: RowId, pos1: i32, pos2: i32) {
    if pos1 == pos2 {
        return;
    }

    // Swap coefficients.
    let r = &mut scip.rows[row];
    let tmpcol = r.cols[pos2];
    let tmpindex = r.cols_index[pos2];
    let tmpval = r.vals[pos2];
    let tmplinkpos = r.linkpos[pos2];
    r.cols[pos2] = r.cols[pos1];
    r.cols_index[pos2] = r.cols_index[pos1];
    r.vals[pos2] = r.vals[pos1];
    r.linkpos[pos2] = r.linkpos[pos1];
    r.cols[pos1] = tmpcol;
    r.cols_index[pos1] = tmpindex;
    r.vals[pos1] = tmpval;
    r.linkpos[pos1] = tmplinkpos;

    // Update link position in columns.
    let (col1, link1, col2, link2) = (r.cols[pos1], r.linkpos[pos1], r.cols[pos2], r.linkpos[pos2]);
    if link1 >= 0 {
        scip.cols[col1].linkpos[link1] = pos1;
    }
    if link2 >= 0 {
        scip.cols[col2].linkpos[link2] = pos2;
    }

    // Update sorted flags.
    let (lp1, lp2) = (scip.cols[col1].lppos, scip.cols[col2].lppos);
    let r = &mut scip.rows[row];
    if lp1 >= 0 && link1 >= 0 {
        r.lpcolssorted = false;
    } else {
        r.nonlpcolssorted = false;
    }
    if lp2 >= 0 && link2 >= 0 {
        r.lpcolssorted = false;
    } else {
        r.nonlpcolssorted = false;
    }
}

/// `rowEventCoefChanged`: issues a ROWCOEFCHANGED event if the row is
/// tracked for coefficient changes.
fn row_event_coef_changed(scip: &mut Scip, row: RowId, col: ColId, oldval: f64, newval: f64) -> ScipResult {
    use crate::event::*;
    let filter = scip.rows[row].eventfilter;
    let f = &scip.eventfilters[filter];
    if f.len > 0 && (f.eventmask & EVENTTYPE_ROWCOEFCHANGED) != 0 {
        let event = event_create_row_coef_changed(row, col, oldval, newval);
        eventqueue_add_filter(scip, filter, event)?;
    }
    Ok(())
}

/// `rowEventConstantChanged`: issues a ROWCONSTCHANGED event if the row is
/// tracked for coefficient changes.
fn row_event_constant_changed(scip: &mut Scip, row: RowId, oldval: f64, newval: f64) -> ScipResult {
    use crate::event::*;
    let filter = scip.rows[row].eventfilter;
    let f = &scip.eventfilters[filter];
    if f.len > 0 && (f.eventmask & EVENTTYPE_ROWCONSTCHANGED) != 0 {
        let event = event_create_row_const_changed(row, oldval, newval);
        eventqueue_add_filter(scip, filter, event)?;
    }
    Ok(())
}

/// `rowEventSideChanged`: issues a ROWSIDECHANGED event. The C tests the
/// mask with a stray negation (issuing the event only while the filter's
/// mask lacks ROWSIDECHANGED); kept as it is.
fn row_event_side_changed(scip: &mut Scip, row: RowId, side: SideType, oldval: f64, newval: f64) -> ScipResult {
    use crate::event::*;
    let filter = scip.rows[row].eventfilter;
    let f = &scip.eventfilters[filter];
    if f.len > 0 && (f.eventmask & EVENTTYPE_ROWSIDECHANGED) == 0 {
        let event = event_create_row_side_changed(row, side, oldval, newval);
        eventqueue_add_filter(scip, filter, event)?;
    }
    Ok(())
}

/// `coefChanged`: notifies the LP that a coefficient of `row` and `col`
/// changed, and invalidates the row's cached activities.
fn coef_changed(scip: &mut Scip, row: RowId, col: ColId) {
    let (rowlpipos, collpipos) = (scip.rows[row].lpipos, scip.cols[col].lpipos);
    if rowlpipos >= 0 && collpipos >= 0 {
        // We have to remember the change only in the row or in the column,
        // because the readdition of one vector would change the other
        // automatically.
        let lp = &mut lp!(scip);
        if rowlpipos >= lp.lpifirstchgrow {
            scip.rows[row].coefchanged = true;
        } else if collpipos >= lp.lpifirstchgcol {
            scip.cols[col].coefchanged = true;
        } else if lp.lpifirstchgrow - rowlpipos <= lp.lpifirstchgcol - collpipos {
            scip.rows[row].coefchanged = true;
            lp.lpifirstchgrow = rowlpipos;
        } else {
            scip.cols[col].coefchanged = true;
            lp.lpifirstchgcol = collpipos;
        }
        lp.flushed = false;
    }
    let r = &mut scip.rows[row];
    r.pseudoactivity = INVALID;
    r.minactivity = INVALID;
    r.maxactivity = INVALID;
    r.validpsactivitydomchg = -1;
    r.validactivitybdsdomchg = -1;
}

/// `colAddCoef`: adds a previously non existing coefficient to an LP
/// column; `linkpos` is the column's position in the row, or -1.
fn col_add_coef(scip: &mut Scip, col: ColId, row: RowId, mut val: f64, mut linkpos: i32) -> ScipResult {
    let len = scip.cols[col].len;
    col_ensure_size(scip, col, len + 1);
    let mut pos = scip.cols[col].len;
    scip.cols[col].len += 1;

    // If the row is in current LP and is linked to the column, we have to
    // insert it at the end of the linked LP rows part of the column's
    // arrays.
    if scip.rows[row].lppos >= 0 && linkpos >= 0 {
        // Move the first non-LP/not linked row to the end.
        let nlprows = scip.cols[col].nlprows;
        if nlprows < pos {
            col_move_coef(scip, col, nlprows, pos);
            pos = nlprows;
        }
        scip.cols[col].nlprows += 1;
    }

    // In case the coefficient is integral w.r.t. numerics we explicitly
    // round the coefficient to an integral value.
    val = if scip.set.is_integral(val) { scip.set.round(val) } else { val };

    // Insert the row at the correct position and update the links.
    let c = &mut scip.cols[col];
    c.rows[pos] = row;
    c.vals[pos] = val;
    c.linkpos[pos] = linkpos;
    if linkpos == -1 {
        c.nunlinked += 1;

        // If the column is in current LP, we have to link it to the row,
        // because otherwise, the primal information of the row is not
        // complete.
        if c.lppos >= 0 {
            // This call might swap the current row with the first
            // non-LP/not linked row, s.t. insertion position has to be
            // updated.
            row_add_coef(scip, row, col, val, pos)?;
            if scip.rows[row].lppos >= 0 {
                pos = scip.cols[col].nlprows - 1;
            }
            linkpos = scip.cols[col].linkpos[pos];
        }
    } else {
        scip.rows[row].linkpos[linkpos] = pos;
        scip.rows[row].nunlinked -= 1;

        // If the column is in current LP, now both conditions hold, so we
        // have to move the column to the linked LP-cols part of the row's
        // cols array.
        if scip.cols[col].lppos >= 0 {
            scip.rows[row].nlpcols += 1;
            let n = scip.rows[row].nlpcols;
            row_swap_coefs(scip, row, linkpos, n - 1);

            // If no swap was necessary, mark nonlpcols to be unsorted.
            if linkpos == n - 1 {
                scip.rows[row].lpcolssorted = false;
            }
        }
    }

    // Update the sorted flags.
    let rowindex = scip.rows[row].index;
    if scip.rows[row].lppos >= 0 && linkpos >= 0 {
        let c = &scip.cols[col];
        if c.nlprows > 1 {
            let sorted = c.lprowssorted && scip.rows[c.rows[c.nlprows - 2]].index < rowindex;
            scip.cols[col].lprowssorted = sorted;
        }
    } else {
        let c = &scip.cols[col];
        if c.len - c.nlprows > 1 {
            let sorted = c.nonlprowssorted && scip.rows[c.rows[c.len - 2]].index < rowindex;
            scip.cols[col].nonlprowssorted = sorted;
        }
    }
    coef_changed(scip, row, col);
    Ok(())
}

/// `colDelCoefPos`: deletes the coefficient at `pos` from a column.
fn col_del_coef_pos(scip: &mut Scip, col: ColId, mut pos: i32) {
    let row = scip.cols[col].rows[pos];
    if scip.cols[col].linkpos[pos] == -1 {
        scip.cols[col].nunlinked -= 1;
    }

    // If row is a linked LP row, move last linked LP coefficient to
    // position of empty slot (deleted coefficient).
    if pos < scip.cols[col].nlprows {
        let n = scip.cols[col].nlprows;
        col_move_coef(scip, col, n - 1, pos);
        scip.cols[col].nlprows -= 1;
        pos = scip.cols[col].nlprows;
    }

    // Move last coefficient to position of empty slot.
    let len = scip.cols[col].len;
    col_move_coef(scip, col, len - 1, pos);
    scip.cols[col].len -= 1;
    coef_changed(scip, row, col);
}

/// `colChgCoefPos`: changes a coefficient at `pos` of a column.
fn col_chg_coef_pos(scip: &mut Scip, col: ColId, pos: i32, mut val: f64) {
    // In case the coefficient is integral w.r.t. numerics we explicitly
    // round the coefficient to an integral value.
    val = if scip.set.is_integral(val) { scip.set.round(val) } else { val };
    if scip.set.is_zero(val) {
        // Delete existing coefficient.
        col_del_coef_pos(scip, col, pos);
    } else if !scip.set.is_eq(scip.cols[col].vals[pos], val) {
        // Change existing coefficient.
        scip.cols[col].vals[pos] = val;
        let row = scip.cols[col].rows[pos];
        coef_changed(scip, row, col);
    }
}

/*
 * Local methods for row norms
 */

/// `rowAddNorms`: updates the row norms after addition of a coefficient.
fn row_add_norms(scip: &mut Scip, row: RowId, col: ColId, val: f64, updateidxvals: bool) {
    let absval = val.abs();
    let (lppos, unchangedobj, colindex) = (scip.cols[col].lppos, scip.cols[col].unchangedobj, scip.cols[col].index);
    let set = &scip.set;
    let r = &mut scip.rows[row];

    // Update LP row norms.
    if lppos >= 0 {
        r.sqrnorm += absval * absval;
        r.sumnorm += absval;
        r.objprod += val * unchangedobj;
    }
    if updateidxvals {
        // Update min/maxidx.
        r.minidx = crate::def::min(r.minidx, colindex);
        r.maxidx = crate::def::max(r.maxidx, colindex);

        // Update maximal and minimal non-zero value.
        if r.nummaxval > 0 {
            if set.is_gt(absval, r.maxval) {
                r.maxval = absval;
                r.nummaxval = 1;
            } else if set.is_ge(absval, r.maxval) {
                // Make sure the maxval is always exactly the same.
                r.maxval = crate::def::max(absval, r.maxval);
                r.nummaxval += 1;
            }
        }
        if r.numminval > 0 {
            if set.is_lt(absval, r.minval) {
                r.minval = absval;
                r.numminval = 1;
            } else if set.is_le(absval, r.minval) {
                // Make sure the minval is always exactly the same.
                r.minval = crate::def::min(absval, r.minval);
                r.numminval += 1;
            }
        }
    }
}

/// `rowDelNorms`: updates the row norms after deletion of a coefficient.
fn row_del_norms(scip: &mut Scip, row: RowId, col: ColId, val: f64, forcenormupdate: bool, updateindex: bool, updateval: bool) {
    let absval = val.abs();
    let (lppos, unchangedobj, colindex) = (scip.cols[col].lppos, scip.cols[col].unchangedobj, scip.cols[col].index);
    let set = &scip.set;
    let r = &mut scip.rows[row];

    // Invalidate min/maxidx values.
    if updateindex && (colindex == r.minidx || colindex == r.maxidx) {
        r.validminmaxidx = false;
    }

    // Update LP row norms.
    if forcenormupdate || lppos >= 0 {
        r.sqrnorm -= absval * absval;
        r.sqrnorm = crate::def::max(r.sqrnorm, 0.0);
        r.sumnorm -= absval;
        r.sumnorm = crate::def::max(r.sumnorm, 0.0);
        r.objprod -= val * unchangedobj;
    }
    if updateval {
        // Update maximal and minimal non-zero value.
        if r.nummaxval > 0 && set.is_ge(absval, r.maxval) {
            r.nummaxval -= 1;
        }
        if r.numminval > 0 && set.is_le(absval, r.minval) {
            r.numminval -= 1;
        }
    }
}

/// `rowAddCoef`: adds a previously non existing coefficient to an LP row;
/// `linkpos` is the row's position in the column, or -1.
fn row_add_coef(scip: &mut Scip, row: RowId, col: ColId, mut val: f64, mut linkpos: i32) -> ScipResult {
    if scip.rows[row].nlocks > 0 {
        log::error!("cannot add a coefficient to the locked unmodifiable row <{}>", scip.rows[row].name);
        return Err(crate::retcode::Retcode::InvalidData);
    }
    let len = scip.rows[row].len;
    row_ensure_size(scip, row, len + 1);
    let mut pos = scip.rows[row].len;
    scip.rows[row].len += 1;

    // If the column is in current LP and is linked to the row, we have to
    // insert it at the end of the linked LP columns part of the row's
    // arrays.
    if scip.cols[col].lppos >= 0 && linkpos >= 0 {
        // Move the first non-LP/not linked column to the end.
        let nlpcols = scip.rows[row].nlpcols;
        if nlpcols < pos {
            row_move_coef(scip, row, nlpcols, pos);
            pos = nlpcols;
        }
        scip.rows[row].nlpcols += 1;
    }

    // In case the coefficient is integral w.r.t. numerics we explicitly
    // round the coefficient to an integral value.
    val = if scip.set.is_integral(val) { scip.set.round(val) } else { val };

    // Insert the column at the correct position and update the links.
    let (colindex, colintegral) = (scip.cols[col].index, scip.cols[col].integral);
    let valintegral = scip.set.is_integral(val);
    let r = &mut scip.rows[row];
    r.cols[pos] = col;
    r.cols_index[pos] = colindex;
    r.vals[pos] = val;
    r.linkpos[pos] = linkpos;
    r.integral = r.integral && colintegral && valintegral;
    if linkpos == -1 {
        r.nunlinked += 1;

        // If the row is in current LP, we have to link it to the column,
        // because otherwise, the dual information of the column is not
        // complete.
        if r.lppos >= 0 {
            // This call might swap the current column with the first
            // non-LP/not linked column, s.t. insertion position has to be
            // updated.
            col_add_coef(scip, col, row, val, pos)?;
            if scip.cols[col].lppos >= 0 {
                pos = scip.rows[row].nlpcols - 1;
            }
            linkpos = scip.rows[row].linkpos[pos];
        }
    } else {
        scip.cols[col].linkpos[linkpos] = pos;
        scip.cols[col].nunlinked -= 1;

        // If the row is in current LP, now both conditions hold, so we
        // have to move the row to the linked LP-rows part of the column's
        // rows array.
        if scip.rows[row].lppos >= 0 {
            scip.cols[col].nlprows += 1;
            let n = scip.cols[col].nlprows;
            col_swap_coefs(scip, col, linkpos, n - 1);

            // If no swap was necessary, mark lprows to be unsorted.
            if linkpos == n - 1 {
                scip.cols[col].lprowssorted = false;
            }
        }
    }

    // Update the sorted flags.
    let r = &mut scip.rows[row];
    if scip.cols[col].lppos >= 0 && linkpos >= 0 {
        if r.nlpcols > 1 {
            r.lpcolssorted = r.lpcolssorted && r.cols_index[r.nlpcols - 2] < colindex;
        }
    } else if r.len - r.nlpcols > 1 {
        r.nonlpcolssorted = r.nonlpcolssorted && r.cols_index[r.len - 2] < colindex;
    }
    row_add_norms(scip, row, col, val, true);
    coef_changed(scip, row, col);

    // Issue row coefficient changed event.
    row_event_coef_changed(scip, row, col, 0.0, val)
}

/// `rowDelCoefPos`: deletes the coefficient at `pos` from a row.
fn row_del_coef_pos(scip: &mut Scip, row: RowId, mut pos: i32) -> ScipResult {
    let col = scip.rows[row].cols[pos];
    let val = scip.rows[row].vals[pos];
    if scip.rows[row].nlocks > 0 {
        log::error!("cannot delete a coefficient from the locked unmodifiable row <{}>", scip.rows[row].name);
        return Err(crate::retcode::Retcode::InvalidData);
    }
    if scip.rows[row].linkpos[pos] == -1 {
        scip.rows[row].nunlinked -= 1;
    }

    // If column is a linked LP column, move last linked LP coefficient to
    // position of empty slot (deleted coefficient).
    if pos < scip.rows[row].nlpcols {
        let n = scip.rows[row].nlpcols;
        row_move_coef(scip, row, n - 1, pos);
        scip.rows[row].nlpcols -= 1;
        pos = scip.rows[row].nlpcols;
    }

    // Move last coefficient to position of empty slot.
    let len = scip.rows[row].len;
    row_move_coef(scip, row, len - 1, pos);
    scip.rows[row].len -= 1;

    // Update norms.
    row_del_norms(scip, row, col, val, false, true, true);
    coef_changed(scip, row, col);

    // Issue row coefficient changed event.
    row_event_coef_changed(scip, row, col, val, 0.0)
}

/// `rowChgCoefPos`: changes a coefficient at `pos` of a row.
fn row_chg_coef_pos(scip: &mut Scip, row: RowId, pos: i32, mut val: f64) -> ScipResult {
    if scip.rows[row].nlocks > 0 {
        log::error!("cannot change a coefficient of the locked unmodifiable row <{}>", scip.rows[row].name);
        return Err(crate::retcode::Retcode::InvalidData);
    }

    // In case the coefficient is integral w.r.t. numerics we explicitly
    // round the coefficient to an integral value.
    val = if scip.set.is_integral(val) { scip.set.round(val) } else { val };
    let col = scip.rows[row].cols[pos];
    if scip.set.is_zero(val) {
        // Delete existing coefficient.
        row_del_coef_pos(scip, row, pos)?;
    } else if !scip.set.is_eq(scip.rows[row].vals[pos], val) {
        // Change existing coefficient.
        let oldval = scip.rows[row].vals[pos];
        row_del_norms(scip, row, col, oldval, false, false, true);
        let valintegral = scip.set.is_integral(val);
        let colintegral = scip.cols[col].integral;
        let r = &mut scip.rows[row];
        r.vals[pos] = val;
        r.integral = r.integral && colintegral && valintegral;
        row_add_norms(scip, row, col, val, true);
        coef_changed(scip, row, col);

        // Issue row coefficient changed event.
        row_event_coef_changed(scip, row, col, oldval, val)?;
    }
    Ok(())
}

/// `rowSideChanged`: notifies the LP that a row side changed.
fn row_side_changed(scip: &mut Scip, row: RowId, sidetype: SideType) {
    if scip.rows[row].lpipos >= 0 {
        if !scip.rows[row].lhschanged && !scip.rows[row].rhschanged {
            let n = lp!(scip).nchgrows;
            ensure_chgrows_size(scip, n + 1);
            let lp = &mut lp!(scip);
            lp.chgrows[lp.nchgrows] = row;
            lp.nchgrows += 1;
        }

        // Mark side change in the row.
        match sidetype {
            SideType::Left => scip.rows[row].lhschanged = true,
            SideType::Right => scip.rows[row].rhschanged = true,
        }
        lp!(scip).flushed = false;
    }
}

/*
 * double linked coefficient matrix methods
 */

/// `colLink`: inserts the column into the rows that don't know it yet.
fn col_link(scip: &mut Scip, col: ColId) -> ScipResult {
    if scip.cols[col].nunlinked > 0 {
        // Unlinked rows can only be in the non-LP/unlinked rows part of
        // the rows array.
        let mut i = scip.cols[col].nlprows;
        while i < scip.cols[col].len {
            if scip.cols[col].linkpos[i] == -1 {
                // This call might swap the current row with the first
                // non-LP/not linked row, but this is of no harm.
                let (r, v) = (scip.cols[col].rows[i], scip.cols[col].vals[i]);
                row_add_coef(scip, r, col, v, i)?;
            }
            i += 1;
        }
    }
    Ok(())
}

/// `colUnlink`: removes the column from the rows that know it.
fn col_unlink(scip: &mut Scip, col: ColId) -> ScipResult {
    if scip.cols[col].nunlinked < scip.cols[col].len {
        let mut i = 0;
        while i < scip.cols[col].len {
            if scip.cols[col].linkpos[i] >= 0 {
                let (r, lp) = (scip.cols[col].rows[i], scip.cols[col].linkpos[i]);
                row_del_coef_pos(scip, r, lp)?;
                scip.cols[col].linkpos[i] = -1;
                scip.cols[col].nunlinked += 1;
            }
            i += 1;
        }
    }
    Ok(())
}

/// `rowLink`: inserts the row into the columns that don't know it yet.
pub fn row_link(scip: &mut Scip, row: RowId) -> ScipResult {
    if scip.rows[row].nunlinked > 0 {
        // Unlinked columns can only be in the non-LP/unlinked columns part
        // of the cols array.
        let mut i = scip.rows[row].nlpcols;
        while i < scip.rows[row].len {
            if scip.rows[row].linkpos[i] == -1 {
                // This call might swap the current column with the first
                // non-LP/not linked column, but this is of no harm.
                let (c, v) = (scip.rows[row].cols[i], scip.rows[row].vals[i]);
                col_add_coef(scip, c, row, v, i)?;
            }
            i += 1;
        }
    }
    Ok(())
}

/// `rowUnlink`: removes the row from the columns that know it.
fn row_unlink(scip: &mut Scip, row: RowId) {
    if scip.rows[row].nunlinked < scip.rows[row].len {
        let mut i = 0;
        while i < scip.rows[row].len {
            if scip.rows[row].linkpos[i] >= 0 {
                let (c, lp) = (scip.rows[row].cols[i], scip.rows[row].linkpos[i]);
                col_del_coef_pos(scip, c, lp);
                scip.rows[row].nunlinked += 1;
            }
            i += 1;
        }
    }
}

/*
 * local LP parameter methods
 */

/// The LP solver of the current LP, `lp->lpi`, as a place.
macro_rules! lpi {
    ($scip:expr) => {
        (**lp!($scip).lpi.as_mut().unwrap())
    };
}

/// `lpSetIntpar`: sets parameter of type int in LP solver, ignoring unknown
/// parameters; returns whether the parameter was set.
fn lp_set_intpar(scip: &mut Scip, lpparam: LpParam, value: i32) -> ScipResult<bool> {
    match lpi::lpi_set_intpar(&mut lpi!(scip), lpparam, value) {
        Err(Retcode::ParameterUnknown) => Ok(false),
        Err(e) => Err(e),
        Ok(()) => Ok(true),
    }
}

/// `lpSetBoolpar`.
fn lp_set_boolpar(scip: &mut Scip, lpparam: LpParam, value: bool) -> ScipResult<bool> {
    lp_set_intpar(scip, lpparam, value as i32)
}

/// `lpSetRealpar`: sets parameter of type real in LP solver, ignoring
/// unknown parameters; returns whether the parameter was set.
fn lp_set_realpar(scip: &mut Scip, lpparam: LpParam, value: f64) -> ScipResult<bool> {
    match lpi::lpi_set_realpar(&mut lpi!(scip), lpparam, value) {
        Err(Retcode::ParameterUnknown) => Ok(false),
        Err(e) => Err(e),
        Ok(()) => Ok(true),
    }
}

/// `lpCutoffDisabled`.
fn lp_cutoff_disabled(scip: &Scip, prob: usize) -> bool {
    scip.set.lp_disablecutoff == 1 || (scip.set.lp_disablecutoff == 2 && !crate::prob::prob_all_cols_in_lp(scip, prob))
}

/// Marks the LP unsolved after a parameter change that may change its
/// solution.
fn lp_mark_unsolved_primal(lp: &mut Lp) {
    lp.solved = false;
    lp.primalfeasible = false;
    lp.primalchecked = false;
    lp.lpobjval = INVALID;
    lp.lpsolstat = LpSolStat::NotSolved;
}

/// `lpSetObjlim`: sets the objective limit of the LP solver; returns
/// whether it changed.
fn lp_set_objlim(scip: &mut Scip, prob: usize, mut objlim: f64) -> ScipResult<bool> {
    let mut success = false;

    // If the objective limit is disabled or SCIP infinity, make sure that
    // the LP objective limit is deactivated by setting it to the LP
    // solver's infinity.
    if lp_cutoff_disabled(scip, prob) || scip.set.is_infinity(objlim) {
        objlim = lpi::lpi_infinity(&lpi!(scip));
    }
    if objlim != lp!(scip).lpiobjlim {
        success = lp_set_realpar(scip, LpParam::ObjLim, objlim)?;
        if success {
            // Check for accuracy issues in LP solver (e.g. CPLEX, if
            // objlim is close to 1e+75).
            let actualobjlim = lpi::lpi_get_realpar(&lpi!(scip), LpParam::ObjLim)?;
            let lp = &mut lp!(scip);
            if actualobjlim != lp.lpiobjlim {
                // Mark the current solution invalid.
                lp_mark_unsolved_primal(lp);
            }
            lp.lpiobjlim = actualobjlim;
        }
    }
    Ok(success)
}

/// `lpSetFeastol`: sets the feasibility tolerance of the LP solver;
/// returns whether it was tightened.
fn lp_set_feastol(scip: &mut Scip, feastol: f64) -> ScipResult<bool> {
    let mut success = false;
    if feastol != lp!(scip).lpifeastol {
        success = lp_set_realpar(scip, LpParam::Feastol, feastol)?;
        if success {
            let actualfeastol = lpi::lpi_get_realpar(&lpi!(scip), LpParam::Feastol)?;
            let lp = &mut lp!(scip);
            if lp.nrows > 0 && actualfeastol < lp.lpifeastol {
                // Mark the current solution invalid.
                lp_mark_unsolved_primal(lp);
            } else {
                success = false;
            }
            lp.lpifeastol = actualfeastol;
        }
    }
    Ok(success)
}

/// `lpSetDualfeastol`: sets the reduced costs feasibility tolerance of the
/// LP solver; returns whether it was tightened.
fn lp_set_dualfeastol(scip: &mut Scip, dualfeastol: f64) -> ScipResult<bool> {
    let mut success = false;
    if dualfeastol != lp!(scip).lpidualfeastol {
        success = lp_set_realpar(scip, LpParam::DualFeastol, dualfeastol)?;
        if success {
            let actualdualfeastol = lpi::lpi_get_realpar(&lpi!(scip), LpParam::DualFeastol)?;
            let lp = &mut lp!(scip);
            if lp.nrows > 0 && actualdualfeastol < lp.lpidualfeastol {
                // Mark the current solution invalid.
                lp.solved = false;
                lp.dualfeasible = false;
                lp.dualchecked = false;
                lp.lpobjval = INVALID;
                lp.lpsolstat = LpSolStat::NotSolved;
            } else {
                success = false;
            }
            lp.lpidualfeastol = actualdualfeastol;
        }
    }
    Ok(success)
}

/// `lpSetBarrierconvtol`: sets the convergence tolerance used in the
/// barrier algorithm; returns whether it was tightened.
fn lp_set_barrierconvtol(scip: &mut Scip, barrierconvtol: f64) -> ScipResult<bool> {
    let mut success = false;
    if barrierconvtol != lp!(scip).lpibarrierconvtol {
        success = lp_set_realpar(scip, LpParam::BarrierConvTol, barrierconvtol)?;
        if success {
            let actualbarrierconvtol = lpi::lpi_get_realpar(&lpi!(scip), LpParam::BarrierConvTol)?;
            let lp = &mut lp!(scip);
            if lp.nrows > 0 && actualbarrierconvtol < lp.lpibarrierconvtol && (lp.lastlpalgo == LpAlgo::Barrier || lp.lastlpalgo == LpAlgo::BarrierCrossover) {
                // Mark the current solution invalid.
                lp.solved = false;
                lp.dualfeasible = false;
                lp.dualchecked = false;
                lp.lpobjval = INVALID;
                lp.lpsolstat = LpSolStat::NotSolved;
            } else {
                success = false;
            }
            lp.lpibarrierconvtol = actualbarrierconvtol;
        }
    }
    Ok(success)
}

/// `lpSetFromscratch`.
fn lp_set_fromscratch(scip: &mut Scip, fromscratch: bool) -> ScipResult<bool> {
    let mut success = false;
    if fromscratch != lp!(scip).lpifromscratch {
        success = lp_set_boolpar(scip, LpParam::FromScratch, fromscratch)?;
        if success {
            lp!(scip).lpifromscratch = fromscratch;
        }
    }
    Ok(success)
}

/// `lpSetFastmip`.
fn lp_set_fastmip(scip: &mut Scip, fastmip: i32) -> ScipResult<bool> {
    let mut success = false;
    if fastmip != lp!(scip).lpifastmip {
        success = lp_set_intpar(scip, LpParam::FastMip, fastmip)?;
        if success {
            let lp = &mut lp!(scip);
            lp.lpifastmip = fastmip;
            lp.solved = false;
        }
    }
    Ok(success)
}

/// `lpSetScaling`.
fn lp_set_scaling(scip: &mut Scip, scaling: i32) -> ScipResult<bool> {
    let mut success = false;
    if scaling != lp!(scip).lpiscaling {
        success = lp_set_intpar(scip, LpParam::Scaling, scaling)?;
        if success {
            lp!(scip).lpiscaling = scaling;
        }
    }
    Ok(success)
}

/// `lpSetThreads`.
fn lp_set_threads(scip: &mut Scip, threads: i32) -> ScipResult<bool> {
    let mut success = false;
    if threads != lp!(scip).lpithreads {
        success = lp_set_intpar(scip, LpParam::Threads, threads)?;
        if success {
            lp!(scip).lpithreads = threads;
        }
    }
    Ok(success)
}

/// `lpSetPresolving`.
fn lp_set_presolving(scip: &mut Scip, presolving: bool) -> ScipResult<bool> {
    let mut success = false;
    if presolving != lp!(scip).lpipresolving {
        success = lp_set_boolpar(scip, LpParam::Presolving, presolving)?;
        if success {
            lp!(scip).lpipresolving = presolving;
        }
    }
    Ok(success)
}

/// `lpSetRowrepswitch`.
fn lp_set_rowrepswitch(scip: &mut Scip, rowrepswitch: f64) -> ScipResult<bool> {
    let mut success = false;
    if rowrepswitch != lp!(scip).lpirowrepswitch {
        success = lp_set_realpar(scip, LpParam::RowRepSwitch, rowrepswitch)?;
        if success {
            lp!(scip).lpirowrepswitch = rowrepswitch;
        }
    }
    Ok(success)
}

/// `lpSetIterationLimit`: -1 for no limit.
fn lp_set_iteration_limit(scip: &mut Scip, mut itlim: i32) -> ScipResult {
    if itlim == -1 {
        itlim = i32::MAX;
    }
    if itlim != lp!(scip).lpiitlim && lp_set_intpar(scip, LpParam::LpItLim, itlim)? {
        let lp = &mut lp!(scip);
        if itlim > lp.lpiitlim {
            // Mark the current solution invalid.
            lp.solved = false;
            lp.lpobjval = INVALID;
            lp.lpsolstat = LpSolStat::NotSolved;
        }
        lp.lpiitlim = itlim;
    }
    Ok(())
}

/// `lpSetPricing`.
fn lp_set_pricing(scip: &mut Scip, pricing: Pricing) -> ScipResult {
    if pricing != lp!(scip).lpipricing && lp_set_intpar(scip, LpParam::Pricing, pricing as i32)? {
        lp!(scip).lpipricing = pricing;
    }
    Ok(())
}

/// `lpSetPricingChar`.
fn lp_set_pricing_char(scip: &mut Scip, pricingchar: u8) -> ScipResult {
    let pricing = match pricingchar {
        b'l' => Pricing::LpiDefault,
        b'a' => Pricing::Auto,
        b'f' => Pricing::Full,
        b'p' => Pricing::Partial,
        b's' => Pricing::Steep,
        b'q' => Pricing::SteepQStart,
        b'd' => Pricing::Devex,
        _ => {
            log::error!("invalid LP pricing parameter <{}>", pricingchar as char);
            return Err(Retcode::InvalidData);
        }
    };
    lp_set_pricing(scip, pricing)
}

/// `lpSetLPInfo`.
fn lp_set_lp_info(scip: &mut Scip, lpinfo: bool) -> ScipResult {
    if lpinfo != lp!(scip).lpilpinfo && lp_set_boolpar(scip, LpParam::LpInfo, lpinfo)? {
        lp!(scip).lpilpinfo = lpinfo;
    }
    Ok(())
}

/// `lpSetConditionLimit`.
fn lp_set_condition_limit(scip: &mut Scip, condlimit: f64) -> ScipResult<bool> {
    let mut success = false;
    if condlimit != lp!(scip).lpiconditionlimit {
        success = lp_set_realpar(scip, LpParam::ConditionLimit, condlimit)?;
        if success {
            lp!(scip).lpiconditionlimit = condlimit;
        }
    }
    Ok(success)
}

/// `lpSetMarkowitz`.
fn lp_set_markowitz(scip: &mut Scip, threshhold: f64) -> ScipResult<bool> {
    let mut success = false;
    if threshhold != lp!(scip).lpimarkowitz {
        success = lp_set_realpar(scip, LpParam::Markowitz, threshhold)?;
        if success {
            lp!(scip).lpimarkowitz = threshhold;
        }
    }
    Ok(success)
}

/// `lpSetTiming`: `timing` is the clock type.
fn lp_set_timing(scip: &mut Scip, timing: i32, enabled: bool) -> ScipResult<bool> {
    let mut success = false;
    let lptiming = if !enabled { 0 } else { timing };
    if lptiming != lp!(scip).lpitiming {
        success = lp_set_intpar(scip, LpParam::Timing, lptiming)?;
        if success {
            lp!(scip).lpitiming = lptiming;
        }
    }
    Ok(success)
}

/// `lpSetRandomseed`.
fn lp_set_randomseed(scip: &mut Scip, randomseed: i32) -> ScipResult<bool> {
    let mut success = false;

    // We don't check this parameter because SoPlex will always return its
    // current random seed, not the initial one.
    if randomseed == 0 {
        lp!(scip).lpirandomseed = randomseed;
        success = true;
    } else if randomseed != lp!(scip).lpirandomseed {
        success = lp_set_intpar(scip, LpParam::RandomSeed, randomseed)?;
        if success {
            lp!(scip).lpirandomseed = randomseed;
        }
    }
    Ok(success)
}

/// `lpSetSolutionPolishing`.
fn lp_set_solution_polishing(scip: &mut Scip, polishing: bool) -> ScipResult<bool> {
    let mut success = false;
    if polishing != lp!(scip).lpisolutionpolishing {
        success = lp_set_intpar(scip, LpParam::Polishing, if polishing { 1 } else { 0 })?;
        if success {
            lp!(scip).lpisolutionpolishing = polishing;
        }
    }
    Ok(success)
}

/// `lpSetRefactorInterval`.
fn lp_set_refactor_interval(scip: &mut Scip, refactor: i32) -> ScipResult<bool> {
    let mut success = false;
    if refactor != lp!(scip).lpirefactorinterval {
        success = lp_set_intpar(scip, LpParam::Refactor, refactor)?;
        if success {
            lp!(scip).lpirefactorinterval = refactor;
        }
    }
    Ok(success)
}

/*
 * Column methods
 */

/// `SCIPcolCreate`: creates an LP column for `var` with the given entries.
pub fn col_create(scip: &mut Scip, var: VarId, rows: &[RowId], vals: &[f64], removable: bool) -> ScipResult<ColId> {
    let len = rows.len() as i32;
    let x = &scip.vars[var];
    let lb = x.locdom.lb;
    let ub = x.locdom.ub;
    let col = Col {
        rows: CVec(rows.to_vec()),
        vals: CVec(vals.to_vec()),
        linkpos: CVec(vec![-1; len as usize]),
        var,
        obj: x.obj,
        unchangedobj: x.unchangedobj,
        lb,
        ub,
        flushedobj: 0.0,
        flushedlb: 0.0,
        flushedub: 0.0,
        index: scip.stat.ncolidx,
        size: len,
        len,
        nlprows: 0,
        nunlinked: len,
        lppos: -1,
        lpipos: -1,
        lpdepth: -1,
        primsol: 0.0,
        redcost: INVALID,
        farkascoef: INVALID,
        minprimsol: ub,
        maxprimsol: lb,
        sbdown: INVALID,
        sbup: INVALID,
        sbsolval: INVALID,
        sblpobjval: INVALID,
        sbnode: -1,
        validredcostlp: -1,
        validfarkaslp: -1,
        validsblp: -1,
        sbitlim: -1,
        nsbcalls: 0,
        age: 0,
        obsoletenode: -1,
        var_probindex: x.probindex,
        basisstatus: BASESTAT_ZERO,
        lprowssorted: true,
        nonlprowssorted: len <= 1,
        objchanged: false,
        lbchanged: false,
        ubchanged: false,
        coefchanged: false,
        integral: crate::var::var_is_integral(scip, var),
        impliedintegral: crate::var::var_is_implied_integral(scip, var),
        removable,
        sbdownvalid: false,
        sbupvalid: false,
        lazylb: x.lazylb,
        lazyub: x.lazyub,
        storedsolvals: None,
    };
    scip.stat.ncolidx += 1;
    Ok(scip.cols.push(col))
}

/// `SCIPcolFree`: unlinks and frees the column.
pub fn col_free(scip: &mut Scip, col: ColId) -> ScipResult {
    // Remove column indices from corresponding rows.
    col_unlink(scip, col)?;
    scip.cols.remove(col);
    Ok(())
}

/// `SCIPcolSort`: sorts column entries such that LP rows precede non-LP
/// rows and inside both parts lower row indices precede higher ones.
pub fn col_sort(scip: &mut Scip, col: ColId) {
    col_sort_lp(scip, col);
    col_sort_non_lp(scip, col);
}

/// `SCIPcolAddCoef`: adds a previously non existing coefficient to an LP
/// column.
pub fn col_add_coef_extern(scip: &mut Scip, col: ColId, row: RowId, val: f64) -> ScipResult {
    col_add_coef(scip, col, row, val, -1)
}

/// `SCIPcolDelCoef`: deletes existing coefficient from column.
pub fn col_del_coef(scip: &mut Scip, col: ColId, row: RowId) -> ScipResult {
    let pos = col_search_coef(scip, col, row);
    if pos == -1 {
        log::error!(
            "coefficient for row <{}> doesn't exist in column <{}>",
            scip.rows[row].name,
            scip.vars[scip.cols[col].var].name
        );
        return Err(Retcode::InvalidData);
    }

    // If row knows of the column, remove the column from the row's col
    // vector.
    let linkpos = scip.cols[col].linkpos[pos];
    if linkpos >= 0 {
        row_del_coef_pos(scip, row, linkpos)?;
    }

    // Delete the row from the column's row vector.
    col_del_coef_pos(scip, col, pos);
    Ok(())
}

/// `SCIPcolChgCoef`: changes or adds a coefficient to an LP column.
pub fn col_chg_coef(scip: &mut Scip, col: ColId, row: RowId, val: f64) -> ScipResult {
    let pos = col_search_coef(scip, col, row);
    if pos == -1 {
        // Add previously not existing coefficient.
        col_add_coef(scip, col, row, val, -1)?;
    } else {
        // Modify already existing coefficient.
        let linkpos = scip.cols[col].linkpos[pos];
        if linkpos >= 0 {
            row_chg_coef_pos(scip, row, linkpos, val)?;
        }
        col_chg_coef_pos(scip, col, pos, val);
    }
    Ok(())
}

/// `SCIPcolIncCoef`: increases value of an existing or non-existing
/// coefficient in an LP column.
pub fn col_inc_coef(scip: &mut Scip, col: ColId, row: RowId, incval: f64) -> ScipResult {
    if scip.set.is_zero(incval) {
        return Ok(());
    }
    let pos = col_search_coef(scip, col, row);
    if pos == -1 {
        // Add previously not existing coefficient.
        col_add_coef(scip, col, row, incval, -1)?;
    } else {
        // Modify already existing coefficient.
        let linkpos = scip.cols[col].linkpos[pos];
        if linkpos >= 0 {
            let v = scip.cols[col].vals[pos] + incval;
            row_chg_coef_pos(scip, row, linkpos, v)?;
        }
        let v = scip.cols[col].vals[pos] + incval;
        col_chg_coef_pos(scip, col, pos, v);
    }
    Ok(())
}

/// `insertColChgcols`: insert column in the chgcols list (if not already
/// there).
fn insert_col_chgcols(scip: &mut Scip, col: ColId) {
    let c = &scip.cols[col];
    if !c.objchanged && !c.lbchanged && !c.ubchanged {
        let n = lp!(scip).nchgcols;
        ensure_chgcols_size(scip, n + 1);
        let lp = &mut lp!(scip);
        lp.chgcols[lp.nchgcols] = col;
        lp.nchgcols += 1;
    }

    // Mark the current LP unflushed.
    lp!(scip).flushed = false;
}

/// `isNewValueUnreliable`: is the new value reliable or may we have
/// cancellation?
fn is_new_value_unreliable(set: &crate::set::Set, newvalue: f64, oldvalue: f64) -> bool {
    let quotient = (newvalue.abs() + 1.0) / (oldvalue.abs() + 1.0);
    set.is_zero(quotient)
}

/// `lpUpdateObjNorms`: update norms of objective function vector.
fn lp_update_obj_norms(scip: &mut Scip, oldobj: f64, newobj: f64) {
    if newobj.abs() != oldobj.abs() {
        let set = &scip.set;
        let lp = &mut lp!(scip);
        if !lp.objsqrnormunreliable {
            let oldvalue = lp.objsqrnorm;
            lp.objsqrnorm += newobj * newobj - oldobj * oldobj;

            // Due to numerical cancellations, we recalculate objsqrnorm
            // using all variables.
            if set.is_lt(lp.objsqrnorm, 0.0) || is_new_value_unreliable(set, lp.objsqrnorm, oldvalue) {
                lp.objsqrnormunreliable = true;
            } else {
                // Due to numerical troubles it still can appear that
                // lp->objsqrnorm is a little bit smaller than 0.
                lp.objsqrnorm = crate::def::max(lp.objsqrnorm, 0.0);
            }
        }
        lp.objsumnorm += newobj.abs() - oldobj.abs();
        lp.objsumnorm = crate::def::max(lp.objsumnorm, 0.0);
    }
}

/// `SCIPcolChgObj`: changes objective value of column.
pub fn col_chg_obj(scip: &mut Scip, col: ColId, newobj: f64) -> ScipResult {
    let c = &scip.cols[col];
    if !scip.set.is_eq(c.obj, newobj) {
        // Insert column in the chgcols list (if not already there).
        if c.lpipos >= 0 {
            insert_col_chgcols(scip, col);

            // Mark objective value change in the column.
            scip.cols[col].objchanged = true;
        }
        // In any case, when the sign of the objective (and thereby the
        // best bound) changes, the variable has to enter the LP and the LP
        // has to be flushed.
        else if (c.obj < 0.0 && newobj >= 0.0 && scip.set.is_zero(c.ub)) || (c.obj >= 0.0 && newobj < 0.0 && scip.set.is_zero(c.lb)) {
            // Mark the LP unflushed.
            lp!(scip).flushed = false;
        }
    }

    // Store new objective function value.
    scip.cols[col].obj = newobj;

    // Update original objective value, as long as we are not in diving or
    // probing and changed objective values.
    if !lp!(scip).divingobjchg {
        let oldobj = scip.cols[col].unchangedobj;
        scip.cols[col].unchangedobj = newobj;
        lp_update_obj_norms(scip, oldobj, newobj);
    }
    Ok(())
}

/// `SCIPcolChgLb`: changes lower bound of column.
pub fn col_chg_lb(scip: &mut Scip, col: ColId, newlb: f64) -> ScipResult {
    let c = &scip.cols[col];
    if !scip.set.is_eq(c.lb, newlb) {
        // Insert column in the chgcols list (if not already there).
        if c.lpipos >= 0 {
            insert_col_chgcols(scip, col);

            // Mark bound change in the column.
            scip.cols[col].lbchanged = true;
        }
        // In any case, when the best bound is zero and gets changed, the
        // variable has to enter the LP and the LP has to be flushed.
        else if c.obj >= 0.0 && scip.set.is_zero(c.lb) {
            // Mark the LP unflushed.
            lp!(scip).flushed = false;
        }
    }
    scip.cols[col].lb = newlb;
    Ok(())
}

/// `SCIPcolChgUb`: changes upper bound of column.
pub fn col_chg_ub(scip: &mut Scip, col: ColId, newub: f64) -> ScipResult {
    let c = &scip.cols[col];
    if !scip.set.is_eq(c.ub, newub) {
        // Insert column in the chgcols list (if not already there).
        if c.lpipos >= 0 {
            insert_col_chgcols(scip, col);

            // Mark bound change in the column.
            scip.cols[col].ubchanged = true;
        }
        // In any case, when the best bound is zero and gets changed, the
        // variable has to enter the LP and the LP has to be flushed.
        else if c.obj < 0.0 && scip.set.is_zero(c.ub) {
            // Mark the LP unflushed.
            lp!(scip).flushed = false;
        }
    }
    scip.cols[col].ub = newub;
    Ok(())
}

/// The sum of `col`'s coefficients times `rowval` of each LP row, first
/// the linked LP rows, then any unlinked rows in the LP (the loops of
/// `SCIPcolCalcRedcost` and its siblings).
fn col_lp_product(scip: &Scip, col: ColId, mut acc: f64, sign: f64, rowval: impl Fn(&Row) -> f64) -> f64 {
    let c = &scip.cols[col];
    for i in 0..c.nlprows {
        let row = &scip.rows[c.rows[i]];
        acc += sign * (c.vals[i] * rowval(row));
    }
    if c.nunlinked > 0 {
        for i in c.nlprows..c.len {
            let row = &scip.rows[c.rows[i]];
            if row.lppos >= 0 {
                acc += sign * (c.vals[i] * rowval(row));
            }
        }
    }
    acc
}

/// `SCIPcolCalcRedcost`: calculates the reduced costs of a column using
/// the given dual solution vector.
pub fn col_calc_redcost(scip: &Scip, col: ColId, dualsol: &[f64]) -> f64 {
    col_lp_product(scip, col, scip.cols[col].obj, -1.0, |row| dualsol[row.lppos as usize])
}

/// `colCalcInternalRedcost`: calculates the reduced costs of a column
/// using the dual solution stored in the rows.
fn col_calc_internal_redcost(scip: &Scip, col: ColId) -> f64 {
    col_lp_product(scip, col, scip.cols[col].obj, -1.0, |row| row.dualsol)
}

/// `SCIPcolGetRedcost`: gets the reduced costs of a column in last LP or
/// after recalculation.
pub fn col_get_redcost(scip: &mut Scip, col: ColId) -> f64 {
    if scip.cols[col].validredcostlp < scip.stat.lpcount {
        let redcost = col_calc_internal_redcost(scip, col);
        let c = &mut scip.cols[col];
        c.redcost = redcost;
        c.validredcostlp = scip.stat.lpcount;
    }
    scip.cols[col].redcost
}

/// `SCIPcolGetFeasibility`: gets the feasibility of (the dual row of) a
/// column in last LP or after recalculation.
pub fn col_get_feasibility(scip: &mut Scip, col: ColId) -> f64 {
    // A column's reduced cost is defined as redcost = obj - activity,
    // activity = y^T * col; the column's feasibility is the feasibility of
    // the corresponding row in the dual LP, whose sides depend on the
    // bounds of the column.
    let (lb, ub) = (scip.cols[col].lb, scip.cols[col].ub);
    if scip.set.is_eq(lb, ub) {
        // Dual row is free.
        scip.set.infinity()
    } else {
        let redcost = col_get_redcost(scip, col);
        if !scip.set.is_negative(lb) {
            // Dual row is activity <= obj <=> redcost >= 0.
            redcost
        } else if scip.set.is_positive(ub) {
            // Dual row is activity == obj <=> redcost == 0.
            -redcost.abs()
        } else {
            // Dual row is activity >= obj <=> redcost <= 0.
            -redcost
        }
    }
}

/// `SCIPcolCalcFarkasCoef`: calculates the Farkas coefficient y^T A_i of a
/// column i using the given dual Farkas vector y.
pub fn col_calc_farkas_coef(scip: &Scip, col: ColId, dualfarkas: &[f64]) -> f64 {
    col_lp_product(scip, col, 0.0, 1.0, |row| dualfarkas[row.lppos as usize])
}

/// `colCalcInternalFarkasCoef`: calculates the Farkas coefficient y^T A_i
/// of a column i using the dual Farkas solution stored in the rows.
fn col_calc_internal_farkas_coef(scip: &Scip, col: ColId) -> f64 {
    col_lp_product(scip, col, 0.0, 1.0, |row| row.dualfarkas)
}

/// `SCIPcolGetFarkasCoef`: gets the Farkas coefficient y^T A_i of a column
/// i in last LP (which must be infeasible).
pub fn col_get_farkas_coef(scip: &mut Scip, col: ColId) -> f64 {
    if scip.cols[col].validfarkaslp < scip.stat.lpcount {
        let farkascoef = col_calc_internal_farkas_coef(scip, col);
        let c = &mut scip.cols[col];
        c.farkascoef = farkascoef;
        c.validfarkaslp = scip.stat.lpcount;
    }
    scip.cols[col].farkascoef
}

/// `SCIPcolGetFarkasValue`: gets the Farkas value of a column in last LP
/// (which must be infeasible).
pub fn col_get_farkas_value(scip: &mut Scip, col: ColId) -> f64 {
    let farkascoef = col_get_farkas_coef(scip, col);
    if farkascoef > 0.0 {
        scip.cols[col].ub * farkascoef
    } else {
        scip.cols[col].lb * farkascoef
    }
}

/// `SCIPlpStartStrongbranch`: marks the LP to be in strong branching mode.
pub fn lp_start_strongbranch(scip: &mut Scip) -> ScipResult {
    lp!(scip).strongbranching = true;
    lpi::lpi_start_strongbranch(&mut lpi!(scip))
}

/// `SCIPlpEndStrongbranch`.
pub fn lp_end_strongbranch(scip: &mut Scip) -> ScipResult {
    lp!(scip).strongbranching = false;
    lpi::lpi_end_strongbranch(&mut lpi!(scip))
}

/// `SCIPcolSetStrongbranchData`: sets strong branching information for a
/// column variable.
pub fn col_set_strongbranch_data(scip: &mut Scip, col: ColId, lpobjval: f64, primsol: f64, sbdown: f64, sbup: f64, sbdownvalid: bool, sbupvalid: bool, iter: i64, itlim: i32) {
    let cutoffbound = lp!(scip).cutoffbound;
    let stat = &mut scip.stat;
    let c = &mut scip.cols[col];
    c.sblpobjval = lpobjval;
    c.sbsolval = primsol;
    c.validsblp = stat.nlps;
    c.sbnode = stat.nnodes;
    c.sbitlim = itlim;
    c.nsbcalls += 1;
    c.sbdown = crate::def::min(sbdown, cutoffbound);
    c.sbup = crate::def::min(sbup, cutoffbound);
    c.sbdownvalid = sbdownvalid;
    c.sbupvalid = sbupvalid;
    stat.nstrongbranchs += 1;
    stat.nsblpiterations += iter;
    if stat.nnodes == 1 {
        stat.nrootstrongbranchs += 1;
        stat.nrootsblpiterations += iter;
    }
}

/// `SCIPcolInvalidateStrongbranchData`: invalidates strong branching
/// information for a column variable.
pub fn col_invalidate_strongbranch_data(scip: &mut Scip, col: ColId) {
    let c = &mut scip.cols[col];
    c.sbdown = INVALID;
    c.sbup = INVALID;
    c.sbdownvalid = false;
    c.sbupvalid = false;
    c.validsblp = -1;
    c.sbsolval = INVALID;
    c.sblpobjval = INVALID;
    c.sbnode = -1;
    c.sbitlim = -1;
}

/// The strong branching iterations to count when the LP solver doesn't
/// report them (`iter == -1` in `SCIPcolGetStrongbranch`).
fn estimated_sb_iterations(stat: &crate::stat::Stat, itlim: i32) -> i32 {
    let mut iter = if stat.ndualresolvelps > 0 {
        (2 * stat.ndualresolvelpiterations / stat.ndualresolvelps) as i32
    } else if stat.nduallps > 0 {
        ((stat.nduallpiterations / stat.nduallps) / 5) as i32
    } else if stat.nprimalresolvelps > 0 {
        (2 * stat.nprimalresolvelpiterations / stat.nprimalresolvelps) as i32
    } else if stat.nprimallps > 0 {
        ((stat.nprimallpiterations / stat.nprimallps) / 5) as i32
    } else {
        0
    };
    if iter / 2 >= itlim {
        iter = 2 * itlim;
    }
    iter
}

/// The results of a strong branching call on one column.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrongbranchResult {
    pub down: f64,
    pub up: f64,
    pub downvalid: bool,
    pub upvalid: bool,
    pub lperror: bool,
}

/// `SCIPcolGetStrongbranch`: gets strong branching information on a column
/// variable. `wantdown`/`wantup` stand for C's non-NULL `down`/`up`.
#[allow(clippy::too_many_arguments)]
pub fn col_get_strongbranch(
    scip: &mut Scip,
    col: ColId,
    integral: bool,
    prob: usize,
    itlim: i32,
    updatecol: bool,
    updatestat: bool,
    wantdown: bool,
    wantup: bool,
) -> ScipResult<StrongbranchResult> {
    let mut lperror = false;
    let mut sbitlim = scip.cols[col].sbitlim;
    let mut nsbcalls = scip.cols[col].nsbcalls;
    let mut validsblp = scip.stat.nlps;
    let mut sbsolval = scip.cols[col].primsol;
    let mut sblpobjval = lp_get_objval(scip, prob);
    let mut sbnode = scip.stat.nnodes;
    let mut sbdown;
    let mut sbup;
    let mut sbdownvalid;
    let mut sbupvalid;

    // If a loose variable has an infinite best bound, the LP bound is -infinity
    // and no gain can be achieved by strong branching.
    if lp!(scip).looseobjvalinf > 0 {
        sbdown = -scip.set.infinity();
        sbup = -scip.set.infinity();
        sbdownvalid = false;
        sbupvalid = false;
    } else {
        let mut iter = 0;
        let time_enabled = scip.set.time_enabled;
        scip.stat.strongbranchtime.start(time_enabled);

        // Call LPI strong branching.
        sbitlim = itlim;
        nsbcalls += 1;
        sbdown = lp!(scip).lpobjval;
        sbup = lp!(scip).lpobjval;
        sbdownvalid = scip.cols[col].sbdownvalid;
        sbupvalid = scip.cols[col].sbupvalid;
        let (lpipos, primsol) = (scip.cols[col].lpipos, scip.cols[col].primsol);
        let down = if wantdown { Some(&mut sbdown) } else { None };
        let up = if wantup { Some(&mut sbup) } else { None };
        let retcode = if integral {
            lpi::lpi_strongbranch_int(&mut lpi!(scip), lpipos, primsol, itlim, down, up, &mut sbdownvalid, &mut sbupvalid, &mut iter)
        } else {
            lpi::lpi_strongbranch_frac(&mut lpi!(scip), lpipos, primsol, itlim, down, up, &mut sbdownvalid, &mut sbupvalid, &mut iter)
        };

        // Check return code for errors.
        if retcode == Err(Retcode::LpError) {
            lperror = true;
            sbdown = INVALID;
            sbup = INVALID;
            sbdownvalid = false;
            sbupvalid = false;
            validsblp = -1;
            sbsolval = INVALID;
            sblpobjval = INVALID;
            sbnode = -1;
        } else {
            retcode?;
            let looseobjval = get_finite_loose_objval(scip, prob);
            let cutoffbound = lp!(scip).cutoffbound;
            sbdown = crate::def::min(sbdown + looseobjval, cutoffbound);
            sbup = crate::def::min(sbup + looseobjval, cutoffbound);

            // Update strong branching statistics.
            if updatestat {
                if iter == -1 {
                    // Calculate average iteration number.
                    iter = estimated_sb_iterations(&scip.stat, itlim);
                }
                let stat = &mut scip.stat;
                stat.nstrongbranchs += 1;
                stat.nsblpiterations += iter as i64;
                if stat.nnodes == 1 {
                    stat.nrootstrongbranchs += 1;
                    stat.nrootsblpiterations += iter as i64;
                }
            }
        }
        scip.stat.strongbranchtime.stop(time_enabled);
    }
    if updatecol {
        let c = &mut scip.cols[col];
        c.sbdown = sbdown;
        c.sbup = sbup;
        c.sbdownvalid = sbdownvalid;
        c.sbupvalid = sbupvalid;
        c.validsblp = validsblp;
        c.sbsolval = sbsolval;
        c.sblpobjval = sblpobjval;
        c.sbnode = sbnode;
        c.sbitlim = sbitlim;
        c.nsbcalls = nsbcalls;
    }
    Ok(StrongbranchResult {
        down: sbdown,
        up: sbup,
        downvalid: sbdownvalid,
        upvalid: sbupvalid,
        lperror,
    })
}

/// `SCIPcolGetStrongbranches`: gets strong branching information on column
/// variables; returns whether an unresolved LP error occurred.
#[allow(clippy::too_many_arguments)]
pub fn col_get_strongbranches(
    scip: &mut Scip,
    cols: &[ColId],
    integral: bool,
    prob: usize,
    itlim: i32,
    down: &mut [f64],
    up: &mut [f64],
    mut downvalid: Option<&mut [bool]>,
    mut upvalid: Option<&mut [bool]>,
) -> ScipResult<bool> {
    let ncols = cols.len();
    if ncols == 0 {
        return Ok(false);
    }
    let time_enabled = scip.set.time_enabled;
    scip.stat.strongbranchtime.start(time_enabled);
    let mut subcols = Vec::with_capacity(ncols);
    let mut subidx = Vec::with_capacity(ncols);
    let mut lpipos = Vec::with_capacity(ncols);
    let mut primsols = Vec::with_capacity(ncols);
    for (j, &col) in cols.iter().enumerate() {
        let objval = lp_get_objval(scip, prob);
        let neginf = -scip.set.infinity();
        let looseobjvalinf = lp!(scip).looseobjvalinf;
        let c = &mut scip.cols[col];
        c.validsblp = scip.stat.nlps;
        c.sbsolval = c.primsol;
        c.sblpobjval = objval;
        c.sbnode = scip.stat.nnodes;

        // If a loose variable has an infinite best bound, the LP bound is
        // -infinity and no gain can be achieved by strong branching.
        if looseobjvalinf > 0 {
            c.sbdown = neginf;
            c.sbup = neginf;
            c.sbdownvalid = false;
            c.sbupvalid = false;
            down[j] = c.sbdown;
            up[j] = c.sbup;
            if let Some(dv) = downvalid.as_deref_mut() {
                dv[j] = c.sbdownvalid;
            }
            if let Some(uv) = upvalid.as_deref_mut() {
                uv[j] = c.sbupvalid;
            }
        } else {
            c.sbitlim = itlim;
            c.nsbcalls += 1;
            lpipos.push(c.lpipos);
            primsols.push(c.primsol);
            subidx.push(j);
            subcols.push(col);
        }
    }
    let nsubcols = subcols.len();
    let mut sbdown = vec![0.0; ncols];
    let mut sbup = vec![0.0; ncols];
    let mut sbdownvalid = vec![false; ncols];
    let mut sbupvalid = vec![false; ncols];
    let mut iter = 0;

    // Compute strong branching information.
    let retcode = if integral {
        lpi::lpi_strongbranches_int(
            &mut lpi!(scip),
            &lpipos,
            &primsols,
            itlim,
            &mut sbdown,
            &mut sbup,
            &mut sbdownvalid,
            &mut sbupvalid,
            &mut iter,
        )
    } else {
        lpi::lpi_strongbranches_frac(
            &mut lpi!(scip),
            &lpipos,
            &primsols,
            itlim,
            &mut sbdown,
            &mut sbup,
            &mut sbdownvalid,
            &mut sbupvalid,
            &mut iter,
        )
    };

    // Check return code for errors.
    let lperror = retcode == Err(Retcode::LpError);
    if lperror {
        for j in 0..nsubcols {
            let (col, idx) = (subcols[j], subidx[j]);
            let c = &mut scip.cols[col];
            c.sbdown = INVALID;
            c.sbup = INVALID;
            c.sbdownvalid = false;
            c.sbupvalid = false;
            c.validsblp = -1;
            c.sbsolval = INVALID;
            c.sblpobjval = INVALID;
            c.sbnode = -1;
            down[idx] = c.sbdown;
            up[idx] = c.sbup;
            if let Some(dv) = downvalid.as_deref_mut() {
                dv[idx] = c.sbdownvalid;
            }
            if let Some(uv) = upvalid.as_deref_mut() {
                uv[idx] = c.sbupvalid;
            }
        }
    } else {
        retcode?;
        let looseobjval = get_finite_loose_objval(scip, prob);
        let cutoffbound = lp!(scip).cutoffbound;
        for j in 0..nsubcols {
            let (col, idx) = (subcols[j], subidx[j]);
            let c = &mut scip.cols[col];
            c.sbdown = crate::def::min(sbdown[j] + looseobjval, cutoffbound);
            c.sbup = crate::def::min(sbup[j] + looseobjval, cutoffbound);
            c.sbdownvalid = sbdownvalid[j];
            c.sbupvalid = sbupvalid[j];
            down[idx] = c.sbdown;
            up[idx] = c.sbup;
            if let Some(dv) = downvalid.as_deref_mut() {
                dv[idx] = c.sbdownvalid;
            }
            if let Some(uv) = upvalid.as_deref_mut() {
                uv[idx] = c.sbupvalid;
            }
        }

        // Update strong branching statistics.
        if iter == -1 {
            // Calculate average iteration number.
            iter = estimated_sb_iterations(&scip.stat, itlim);
        }
        let stat = &mut scip.stat;
        stat.nstrongbranchs += ncols as i64;
        stat.nsblpiterations += iter as i64;
        if stat.nnodes == 1 {
            stat.nrootstrongbranchs += ncols as i64;
            stat.nrootsblpiterations += iter as i64;
        }
    }
    scip.stat.strongbranchtime.stop(time_enabled);
    Ok(lperror)
}

/// `SCIPcolGetStrongbranchLast`: the last strong branching values (down,
/// up, downvalid, upvalid, solval, lpobjval).
pub fn col_get_strongbranch_last(scip: &Scip, col: ColId) -> (f64, f64, bool, bool, f64, f64) {
    let c = &scip.cols[col];
    (c.sbdown, c.sbup, c.sbdownvalid, c.sbupvalid, c.sbsolval, c.sblpobjval)
}

/// `SCIPcolGetStrongbranchLPAge`: number of LPs solved since the last
/// strong branching on this column at the current node.
pub fn col_get_strongbranch_lp_age(scip: &Scip, col: ColId) -> i64 {
    let c = &scip.cols[col];
    if c.sbnode != scip.stat.nnodes { i64::MAX } else { scip.stat.nlps - c.validsblp }
}

/// `SCIPcolMarkNotRemovableLocal`: marks a column to be not removable from
/// the LP in the current node.
pub fn col_mark_not_removable_local(scip: &mut Scip, col: ColId) {
    scip.cols[col].obsoletenode = scip.stat.nnodes;
}

/*
 * Row methods
 */

/// `rowCalcNorms`: calculates row norms and min/maxidx from scratch, and
/// checks the sorting.
fn row_calc_norms(scip: &mut Scip, row: RowId) {
    let infinity = scip.set.infinity();
    let r = &mut scip.rows[row];
    r.sqrnorm = 0.0;
    r.sumnorm = 0.0;
    r.objprod = 0.0;
    r.maxval = 0.0;
    r.nummaxval = 1;
    r.minval = infinity;
    r.numminval = 1;
    r.minidx = i32::MAX;
    r.maxidx = i32::MIN;
    r.validminmaxidx = true;
    r.lpcolssorted = true;
    r.nonlpcolssorted = true;

    // Check, if row is sorted; calculate sqrnorm, sumnorm, maxval, minval,
    // minidx, and maxidx.
    let (nlpcols, len) = (r.nlpcols, r.len);
    for i in 0..nlpcols {
        let (c, v) = (scip.rows[row].cols[i], scip.rows[row].vals[i]);
        row_add_norms(scip, row, c, v, true);
        if i > 0 {
            let r = &mut scip.rows[row];
            r.lpcolssorted = r.lpcolssorted && r.cols_index[i - 1] < r.cols_index[i];
        }
    }
    for i in nlpcols..len {
        let (c, v) = (scip.rows[row].cols[i], scip.rows[row].vals[i]);
        row_add_norms(scip, row, c, v, true);
        if i > nlpcols {
            let r = &mut scip.rows[row];
            r.nonlpcolssorted = r.nonlpcolssorted && r.cols_index[i - 1] < r.cols_index[i];
        }
    }
}

/// `rowCalcIdxsAndVals`: calculates min/maxval and min/maxidx from
/// scratch.
fn row_calc_idxs_and_vals(scip: &mut Scip, row: RowId) {
    let set = &scip.set;
    let r = &mut scip.rows[row];
    r.maxval = 0.0;
    r.nummaxval = 1;
    r.numintcols = 0;
    r.numimplintcols = 0;
    r.minval = set.infinity();
    r.numminval = 1;
    r.minidx = i32::MAX;
    r.maxidx = i32::MIN;
    r.validminmaxidx = true;

    // Calculate maxval, minval, minidx, and maxidx.
    for i in 0..r.len {
        let col = &scip.cols[r.cols[i]];
        let absval = r.vals[i].abs();
        r.minidx = crate::def::min(r.minidx, col.index);
        r.maxidx = crate::def::max(r.maxidx, col.index);
        if col.integral {
            r.numintcols += 1;
            if col.impliedintegral {
                r.numimplintcols += 1;
            }
        }

        // Update maximal and minimal non-zero value.
        if r.nummaxval > 0 {
            if set.is_gt(absval, r.maxval) {
                r.maxval = absval;
                r.nummaxval = 1;
            } else if set.is_ge(absval, r.maxval) {
                // Make sure the maxval is always exactly the same.
                r.maxval = crate::def::max(absval, r.maxval);
                r.nummaxval += 1;
            }
        }
        if r.numminval > 0 {
            if set.is_lt(absval, r.minval) {
                r.minval = absval;
                r.numminval = 1;
            } else if set.is_le(absval, r.minval) {
                // Make sure the minval is always exactly the same.
                r.minval = crate::def::min(absval, r.minval);
                r.numminval += 1;
            }
        }
    }
}

/// `isIntegralScalar`: checks whether the given scalar scales the given
/// value to an integral number with error in the given bounds; returns the
/// scaled integral value.
fn is_integral_scalar(val: f64, scalar: f64, mindelta: f64, maxdelta: f64) -> Option<f64> {
    let sval = val * scalar;
    let downval = sval.floor();
    let upval = sval.ceil();
    if crate::misc::rel_diff(sval, downval) <= maxdelta {
        Some(downval)
    } else if crate::misc::rel_diff(sval, upval) >= mindelta {
        Some(upval)
    } else {
        None
    }
}

/// `rowScale`: scales row with given factor, and rounds coefficients to
/// integers if close enough; the constant is automatically moved to the
/// sides.
#[allow(clippy::too_many_arguments)]
fn row_scale(scip: &mut Scip, row: RowId, scaleval: f64, integralcontvars: bool, minrounddelta: f64, maxrounddelta: f64) -> ScipResult {
    let mut mindelta = 0.0;
    let mut maxdelta = 0.0;
    let mut mindeltainf = false;
    let mut maxdeltainf = false;
    let mut oldlen = scip.rows[row].len;

    // Scale the row coefficients, thereby recalculating whether the row's
    // activity is always integral; if the row coefficients are rounded to
    // the nearest integer value, calculate the maximal activity difference
    // this rounding can lead to.
    scip.rows[row].integral = true;
    let mut c = 0;
    while c < scip.rows[row].len {
        let col = scip.rows[row].cols[c];
        let val = scip.rows[row].vals[c];

        // Get local or global bounds for column, depending on the local or
        // global feasibility of the row.
        let (lb, ub) = if scip.rows[row].local {
            (scip.cols[col].lb, scip.cols[col].ub)
        } else {
            let v = &scip.vars[scip.cols[col].var];
            (v.glbdom.lb, v.glbdom.ub)
        };

        // Calculate scaled coefficient.
        let mut newval = val * scaleval;
        if integralcontvars || scip.cols[col].integral || scip.set.is_integral(newval) {
            if let Some(intval) = is_integral_scalar(val, scaleval, minrounddelta, maxrounddelta) {
                if !scip.set.is_eq(intval, newval) {
                    if intval < newval {
                        mindelta += (intval - newval) * ub;
                        maxdelta += (intval - newval) * lb;
                        mindeltainf = mindeltainf || scip.set.is_infinity(ub);
                        maxdeltainf = maxdeltainf || scip.set.is_infinity(-lb);
                    } else {
                        mindelta += (intval - newval) * lb;
                        maxdelta += (intval - newval) * ub;
                        mindeltainf = mindeltainf || scip.set.is_infinity(-lb);
                        maxdeltainf = maxdeltainf || scip.set.is_infinity(ub);
                    }
                }
                newval = intval;
            }
        }
        if !scip.set.is_eq(val, newval) {
            // If column knows of the row, change the corresponding
            // coefficient in the column.
            let linkpos = scip.rows[row].linkpos[c];
            if linkpos >= 0 {
                col_chg_coef_pos(scip, col, linkpos, newval);
            }

            // Change the coefficient in the row, and update the norms and
            // integrality status.
            row_chg_coef_pos(scip, row, c, newval)?;

            // Current coefficient has been deleted from the row because it
            // was almost zero.
            if oldlen != scip.rows[row].len {
                c -= 1;
                oldlen = scip.rows[row].len;
            }
        } else {
            let valintegral = scip.set.is_integral(val);
            let r = &mut scip.rows[row];
            r.integral = r.integral && scip.cols[col].integral && valintegral;
        }
        c += 1;
    }

    // Scale the row sides, and move the constant to the sides; relax the
    // sides with accumulated delta in order to not destroy feasibility due
    // to rounding.
    let (lhs, rhs, constant) = (scip.rows[row].lhs, scip.rows[row].rhs, scip.rows[row].constant);
    if !scip.set.is_infinity(-lhs) {
        let newval = if mindeltainf {
            -scip.set.infinity()
        } else {
            let mut newval = (lhs - constant) * scaleval + mindelta;
            if scip.set.is_integral(newval) || (scip.rows[row].integral && !scip.rows[row].modifiable) {
                newval = scip.set.sum_ceil(newval);
            }
            newval
        };
        row_chg_lhs(scip, row, newval)?;
    }
    if !scip.set.is_infinity(rhs) {
        let newval = if maxdeltainf {
            scip.set.infinity()
        } else {
            let mut newval = (rhs - constant) * scaleval + maxdelta;
            if scip.set.is_integral(newval) || (scip.rows[row].integral && !scip.rows[row].modifiable) {
                newval = scip.set.sum_floor(newval);
            }
            newval
        };
        row_chg_rhs(scip, row, newval)?;
    }

    // Clear the row constant.
    row_chg_constant(scip, row, 0.0)?;
    scip.rows[row].validactivitylp = -1;
    Ok(())
}

/// `SCIProwCreate`: creates and captures an LP row. `origin` is the
/// creating constraint, handler or separator (an id), or `NONE`.
#[allow(clippy::too_many_arguments)]
pub fn row_create(
    scip: &mut Scip,
    name: &str,
    cols: &[ColId],
    vals: &[f64],
    lhs: f64,
    rhs: f64,
    origintype: RowOriginType,
    origin: usize,
    local: bool,
    modifiable: bool,
    removable: bool,
) -> ScipResult<RowId> {
    let len = cols.len() as i32;
    let mut integral = true;
    let mut rvals = CVec(vals.to_vec());
    let mut cols_index = CVec::with_size(len);
    for i in 0..len {
        let var = scip.cols[cols[i as usize]].var;
        cols_index[i] = scip.cols[cols[i as usize]].index;
        if scip.set.is_integral(rvals[i]) {
            rvals[i] = scip.set.round(rvals[i]);
            integral = integral && crate::var::var_is_integral(scip, var);
        } else {
            integral = false;
        }
    }
    let infinity = scip.set.infinity();
    let r = Row {
        cols: CVec(cols.to_vec()),
        vals: rvals,
        cols_index,
        linkpos: CVec(vec![-1; len as usize]),
        integral,
        name: name.to_string(),
        constant: 0.0,
        lhs,
        rhs,
        flushedlhs: -infinity,
        flushedrhs: infinity,
        sqrnorm: 0.0,
        sumnorm: 0.0,
        objprod: 0.0,
        maxval: 0.0,
        minval: infinity,
        dualsol: 0.0,
        activity: INVALID,
        dualfarkas: 0.0,
        pseudoactivity: INVALID,
        minactivity: INVALID,
        maxactivity: INVALID,
        origin,
        eventfilter: crate::var::NONE,
        index: scip.stat.nrowidx,
        size: len,
        len,
        nlpcols: 0,
        nunlinked: len,
        nuses: 0,
        lppos: -1,
        lpipos: -1,
        lpdepth: -1,
        minidx: i32::MAX,
        maxidx: i32::MIN,
        nummaxval: 0,
        numminval: 0,
        numintcols: -1,
        numimplintcols: -1,
        validactivitylp: -1,
        validpsactivitydomchg: -1,
        validactivitybdsdomchg: -1,
        nlpsaftercreation: 0,
        activeinlpcounter: 0,
        age: 0,
        rank: 0,
        obsoletenode: -1,
        fromcutpool: false,
        basisstatus: BASESTAT_BASIC,
        lpcolssorted: true,
        nonlpcolssorted: len <= 1,
        delaysort: false,
        validminmaxidx: false,
        lhschanged: false,
        rhschanged: false,
        coefchanged: false,
        local,
        modifiable,
        nlocks: 0,
        origintype,
        removable,
        inglobalcutpool: false,
        normunreliable: false,
        storedsolvals: None,
    };
    scip.stat.nrowidx += 1;
    let row = scip.rows.push(r);

    // Calculate row norms and min/maxidx, and check if row is sorted.
    row_calc_norms(scip, row);

    // Capture the row.
    row_capture(scip, row);

    // Create event filter.
    scip.rows[row].eventfilter = crate::event::eventfilter_create(scip);

    // Capture origin constraint if it is a constraint.
    if origintype == RowOriginType::Cons {
        crate::cons::cons_capture(scip, origin);
    }
    Ok(row)
}

/// `SCIProwFree`: frees an LP row.
fn row_free(scip: &mut Scip, row: RowId) -> ScipResult {
    // Release constraint that has been used for creating the row.
    if scip.rows[row].origintype == RowOriginType::Cons {
        let cons = scip.rows[row].origin;
        crate::cons::cons_release(scip, cons)?;
    }

    // Remove column indices from corresponding columns.
    row_unlink(scip, row);

    // Free event filter.
    let filter = scip.rows[row].eventfilter;
    crate::event::eventfilter_free(scip, filter)?;
    scip.rows.remove(row);
    Ok(())
}

/// `SCIProwCapture`: increases usage counter of LP row.
pub fn row_capture(scip: &mut Scip, row: RowId) {
    scip.rows[row].nuses += 1;
}

/// `SCIProwRelease`: decreases usage counter of LP row, and frees memory
/// if necessary.
pub fn row_release(scip: &mut Scip, row: RowId) -> ScipResult {
    scip.rows[row].nuses -= 1;
    if scip.rows[row].nuses == 0 {
        row_free(scip, row)?;
    }
    Ok(())
}

/// `SCIProwLock`: locks an unmodifiable row, which forbids further changes.
pub fn row_lock(scip: &mut Scip, row: RowId) {
    let r = &mut scip.rows[row];
    if !r.modifiable {
        r.nlocks += 1;
    }
}

/// `SCIProwUnlock`: unlocks a lock of an unmodifiable row.
pub fn row_unlock(scip: &mut Scip, row: RowId) {
    let r = &mut scip.rows[row];
    if !r.modifiable {
        r.nlocks -= 1;
    }
}

/// `SCIProwAddCoef`: adds a previously non existing coefficient to an LP
/// row.
pub fn row_add_coef_extern(scip: &mut Scip, row: RowId, col: ColId, val: f64) -> ScipResult {
    row_add_coef(scip, row, col, val, -1)
}

/// `SCIProwDelCoef`: deletes coefficient from row.
pub fn row_del_coef(scip: &mut Scip, row: RowId, col: ColId) -> ScipResult {
    let pos = row_search_coef(scip, row, col);
    if pos == -1 {
        log::error!(
            "coefficient for column <{}> doesn't exist in row <{}>",
            scip.vars[scip.cols[col].var].name,
            scip.rows[row].name
        );
        return Err(Retcode::InvalidData);
    }

    // If column knows of the row, remove the row from the column's row
    // vector.
    let linkpos = scip.rows[row].linkpos[pos];
    if linkpos >= 0 {
        col_del_coef_pos(scip, col, linkpos);
    }

    // Delete the column from the row's col vector.
    row_del_coef_pos(scip, row, pos)
}

/// `SCIProwChgCoef`: changes or adds a coefficient to an LP row.
pub fn row_chg_coef(scip: &mut Scip, row: RowId, col: ColId, val: f64) -> ScipResult {
    let pos = row_search_coef(scip, row, col);
    if pos == -1 {
        // Add previously not existing coefficient.
        row_add_coef(scip, row, col, val, -1)?;
    } else {
        // Modify already existing coefficient.
        let linkpos = scip.rows[row].linkpos[pos];
        if linkpos >= 0 {
            col_chg_coef_pos(scip, col, linkpos, val);
        }
        row_chg_coef_pos(scip, row, pos, val)?;
    }
    Ok(())
}

/// `SCIProwIncCoef`: increases value of an existing or non-existing
/// coefficient in an LP row.
pub fn row_inc_coef(scip: &mut Scip, row: RowId, col: ColId, incval: f64) -> ScipResult {
    if scip.set.is_zero(incval) {
        return Ok(());
    }
    let pos = row_search_coef(scip, row, col);
    if pos == -1 {
        // Add previously not existing coefficient.
        row_add_coef(scip, row, col, incval, -1)?;
    } else {
        // Modify already existing coefficient.
        let linkpos = scip.rows[row].linkpos[pos];
        if linkpos >= 0 {
            let v = scip.rows[row].vals[pos] + incval;
            col_chg_coef_pos(scip, col, linkpos, v);
        }
        let v = scip.rows[row].vals[pos] + incval;
        row_chg_coef_pos(scip, row, pos, v)?;
    }

    // Invalid the activity.
    scip.rows[row].validactivitylp = -1;
    Ok(())
}

/// `SCIProwChgConstant`: changes constant value of a row.
pub fn row_chg_constant(scip: &mut Scip, row: RowId, constant: f64) -> ScipResult {
    if !scip.set.is_eq(constant, scip.rows[row].constant) {
        let domchgcount = scip.stat.domchgcount;
        let r = &mut scip.rows[row];
        if r.validpsactivitydomchg == domchgcount {
            r.pseudoactivity += constant - r.constant;
        }
        if r.validactivitybdsdomchg == domchgcount {
            r.minactivity += constant - r.constant;
            r.maxactivity += constant - r.constant;
        }
        let (lhs, rhs) = (r.lhs, r.rhs);
        if !scip.set.is_infinity(-lhs) {
            row_side_changed(scip, row, SideType::Left);
        }
        if !scip.set.is_infinity(rhs) {
            row_side_changed(scip, row, SideType::Right);
        }
        let oldconstant = scip.rows[row].constant;
        scip.rows[row].constant = constant;

        // Issue row constant changed event.
        row_event_constant_changed(scip, row, oldconstant, constant)?;
    }
    Ok(())
}

/// `SCIProwAddConstant`: add constant value to a row.
pub fn row_add_constant(scip: &mut Scip, row: RowId, addval: f64) -> ScipResult {
    if !scip.set.is_zero(addval) {
        let c = scip.rows[row].constant + addval;
        row_chg_constant(scip, row, c)?;
    }
    Ok(())
}

/// `SCIProwChgLhs`: changes left hand side of LP row.
pub fn row_chg_lhs(scip: &mut Scip, row: RowId, lhs: f64) -> ScipResult {
    if !scip.set.is_eq(scip.rows[row].lhs, lhs) {
        let oldlhs = scip.rows[row].lhs;
        scip.rows[row].lhs = lhs;
        row_side_changed(scip, row, SideType::Left);
        if !lp!(scip).diving {
            // Issue row side changed event.
            row_event_side_changed(scip, row, SideType::Left, oldlhs, lhs)?;
        }
    }
    Ok(())
}

/// `SCIProwChgRhs`: changes right hand side of LP row.
pub fn row_chg_rhs(scip: &mut Scip, row: RowId, rhs: f64) -> ScipResult {
    if !scip.set.is_eq(scip.rows[row].rhs, rhs) {
        let oldrhs = scip.rows[row].rhs;
        scip.rows[row].rhs = rhs;
        row_side_changed(scip, row, SideType::Right);
        if !lp!(scip).diving {
            // Issue row side changed event.
            row_event_side_changed(scip, row, SideType::Right, oldrhs, rhs)?;
        }
    }
    Ok(())
}

/// `SCIProwChgLocal`: changes the local flag of LP row.
pub fn row_chg_local(scip: &mut Scip, row: RowId, local: bool) {
    scip.rows[row].local = local;
}

/// Scalars tried by `SCIProwCalcIntegralScalar` besides powers of two.
const SCALARS: [f64; 9] = [3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0, 17.0, 19.0];

/// Scales `scaleval` up until it makes `val` integral (`absval * scaleval
/// >= 0.5`), by the odd scalars first and powers of two otherwise (the
/// inner loops of `SCIProwCalcIntegralScalar`).
fn grow_integral_scaleval(mut scaleval: f64, val: f64, mindelta: f64, maxdelta: f64, maxscale: f64) -> f64 {
    let absval = val.abs();
    while scaleval <= maxscale && (absval * scaleval < 0.5 || is_integral_scalar(val, scaleval, mindelta, maxdelta).is_none()) {
        match SCALARS.iter().find(|&&s| is_integral_scalar(val, scaleval * s, mindelta, maxdelta).is_some()) {
            Some(&s) => scaleval *= s,
            None => scaleval *= 2.0,
        }
    }
    scaleval
}

/// `SCIProwCalcIntegralScalar`: tries to find a value, such that all row
/// coefficients, if scaled with this value become integral; returns the
/// scalar if one was found.
pub fn row_calc_integral_scalar(scip: &Scip, row: RowId, mindelta: f64, maxdelta: f64, maxdnom: i64, maxscale: f64, usecontvars: bool) -> Option<f64> {
    let r = &scip.rows[row];

    // Get minimal absolute non-zero value.
    let mut minval = f64::MAX;
    for c in 0..r.len {
        let val = r.vals[c];
        if val < mindelta || val > maxdelta {
            let absval = val.abs();
            minval = crate::def::min(minval, absval);
        }
    }
    if minval == f64::MAX {
        // All coefficients are zero (inside tolerances).
        return Some(1.0);
    }

    // Try, if row coefficients can be made integral by multiplying them
    // with the reciprocal of the smallest coefficient and a power of 2.
    let mut scaleval = 1.0 / minval;
    let mut scalable = scaleval <= maxscale;
    let mut c = 0;
    while c < r.len && scalable {
        // Don't look at continuous variables, if we don't have to.
        if usecontvars || scip.cols[r.cols[c]].integral {
            // Check, if the coefficient can be scaled with a simple scalar.
            scaleval = grow_integral_scaleval(scaleval, r.vals[c], mindelta, maxdelta, maxscale);
            scalable = scaleval <= maxscale;
        }
        c += 1;
    }
    if scalable {
        // Make row coefficients integral by dividing them by the smallest
        // coefficient (and multiplying them with a power of 2).
        return Some(scaleval);
    }

    // Try, if row coefficients can be made integral by multiplying them by
    // a power of 2.
    let mut twomultval = 1.0;
    let mut twomult = twomultval <= maxscale;
    let mut c = 0;
    while c < r.len && twomult {
        // Don't look at continuous variables, if we don't have to.
        if usecontvars || scip.cols[r.cols[c]].integral {
            // Check, if the coefficient can be scaled with a simple scalar.
            twomultval = grow_integral_scaleval(twomultval, r.vals[c], mindelta, maxdelta, maxscale);
            twomult = twomultval <= maxscale;
        }
        c += 1;
    }
    if twomult {
        // Make row coefficients integral by multiplying them with a power
        // of 2.
        return Some(twomultval);
    }

    // Convert each coefficient into a rational number, calculate the
    // greatest common divisor of the numerators and the smallest common
    // multiple of the denominators.
    let mut gcd: i64 = 1;
    let mut scm: i64 = 1;
    let mut rational = maxdnom > 1;

    // First coefficient (to initialize gcd).
    let mut c = 0;
    while c < r.len && rational {
        if usecontvars || scip.cols[r.cols[c]].integral {
            let val = r.vals[c];
            let mut numerator = 0;
            let mut denominator = 0;
            rational = crate::misc::real_to_rational(val, mindelta, maxdelta, maxdnom, &mut numerator, &mut denominator);
            if rational && numerator != 0 {
                gcd = numerator.abs();
                scm = denominator;
                rational = (scm as f64) / (gcd as f64) <= maxscale;
                break;
            }
        }
        c += 1;
    }

    // Remaining coefficients.
    c += 1;
    while c < r.len && rational {
        if usecontvars || scip.cols[r.cols[c]].integral {
            let val = r.vals[c];
            let mut numerator = 0;
            let mut denominator = 0;
            rational = crate::misc::real_to_rational(val, mindelta, maxdelta, maxdnom, &mut numerator, &mut denominator);
            if rational && numerator != 0 {
                gcd = crate::misc::calc_gre_com_div(gcd, numerator.abs());
                scm *= denominator / crate::misc::calc_gre_com_div(scm, denominator);
                rational = (scm as f64) / (gcd as f64) <= maxscale;
            }
        }
        c += 1;
    }
    if rational {
        // Make row coefficients integral by multiplying them with the
        // smallest common multiple of the denominators.
        return Some((scm as f64) / (gcd as f64));
    }
    None
}

/// `SCIProwMakeIntegral`: tries to scale row, s.t. all coefficients (of
/// integer variables) become integral; returns whether it could.
#[allow(clippy::too_many_arguments)]
pub fn row_make_integral(scip: &mut Scip, row: RowId, mindelta: f64, maxdelta: f64, maxdnom: i64, maxscale: f64, usecontvars: bool) -> ScipResult<bool> {
    // Calculate scalar to make coefficients integral.
    match row_calc_integral_scalar(scip, row, mindelta, maxdelta, maxdnom, maxscale, usecontvars) {
        Some(intscalar) => {
            // Scale the row.
            row_scale(scip, row, intscalar, usecontvars, mindelta, maxdelta)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// `SCIProwSort`: sorts row entries such that LP columns precede non-LP
/// columns and inside both parts lower column indices precede higher ones.
pub fn row_sort(scip: &mut Scip, row: RowId) {
    row_sort_lp(scip, row);
    row_sort_non_lp(scip, row);
}

/// `rowMerge`: sorts row, and merges equal column entries (resulting
/// from lazy sorting and adding) into a single entry; removes zero
/// entries. Works only for rows not yet in the LP.
fn row_merge(scip: &mut Scip, row: RowId) {
    let r = &scip.rows[row];
    if r.len > 0 && (!r.lpcolssorted || !r.nonlpcolssorted) {
        // Make sure, the row is sorted.
        row_sort(scip, row);

        // Merge equal columns, thereby recalculating whether the row's
        // activity is always integral.
        let set = &scip.set;
        let r = &mut scip.rows[row];
        let mut t = 0;
        r.integral = true;
        let mut s = 1;
        while s < r.len {
            if r.cols[s] == r.cols[t] {
                // Merge entries with equal column.
                r.vals[t] += r.vals[s];
            } else {
                // Go to the next entry, overwriting current entry if
                // coefficient is zero.
                if !set.is_zero(r.vals[t]) {
                    // In case the coefficient is integral w.r.t. numerics we
                    // explicitly round the coefficient to an integral value.
                    r.vals[t] = if set.is_integral(r.vals[t]) { set.round(r.vals[t]) } else { r.vals[t] };
                    r.integral = r.integral && scip.cols[r.cols[t]].integral && set.is_integral(r.vals[t]);
                    t += 1;
                }
                r.cols[t] = r.cols[s];
                r.cols_index[t] = r.cols_index[s];
                r.vals[t] = r.vals[s];
            }
            s += 1;
        }
        if !set.is_zero(r.vals[t]) {
            r.integral = r.integral && scip.cols[r.cols[t]].integral && set.is_integral(r.vals[t]);
            t += 1;
        }
        r.len = t;
        r.nunlinked = t;

        // If equal entries were merged, we have to recalculate the norms,
        // since the squared Euclidean norm is wrong.
        if t < s {
            row_calc_norms(scip, row);
        }
    }
}

/// `SCIProwDelaySort`: enables delaying of row sorting.
pub fn row_delay_sort(scip: &mut Scip, row: RowId) {
    scip.rows[row].delaysort = true;
}

/// `SCIProwForceSort`: disables delaying of row sorting, sorts row and
/// merges coefficients with equal columns.
pub fn row_force_sort(scip: &mut Scip, row: RowId) {
    scip.rows[row].delaysort = false;
    row_merge(scip, row);
}

/// `SCIProwRecalcNorms`: recalculates the current activity of a row.
pub fn row_recalc_norms(scip: &mut Scip, row: RowId) {
    row_calc_norms(scip, row);
}

/// `SCIProwGetNorm`: the Euclidean norm of the row's LP part (the NDEBUG
/// macro).
#[inline]
pub fn row_get_norm(row: &Row) -> f64 {
    row.sqrnorm.sqrt()
}

/// `SCIPcolGetBestBound`: the bound with the better objective (the NDEBUG
/// macro).
#[inline]
pub fn col_get_best_bound(col: &Col) -> f64 {
    if col.obj >= 0.0 { col.lb } else { col.ub }
}

/// The activity of `row` with the column values `colval` (the row constant
/// plus its LP columns, linked or not), the loops of
/// `SCIProwRecalcLPActivity` and its siblings.
fn row_lp_activity(scip: &Scip, row: RowId, colval: impl Fn(&Col) -> f64) -> f64 {
    let r = &scip.rows[row];
    let mut activity = r.constant;
    for c in 0..r.nlpcols {
        let col = &scip.cols[r.cols[c]];
        activity += r.vals[c] * colval(col);
    }
    if r.nunlinked > 0 {
        for c in r.nlpcols..r.len {
            let col = &scip.cols[r.cols[c]];
            if col.lppos >= 0 {
                activity += r.vals[c] * colval(col);
            }
        }
    }
    activity
}

/// Clamps an activity to [-infinity, infinity].
fn clamp_activity(set: &crate::set::Set, mut activity: f64) -> f64 {
    let inf = set.infinity();
    activity = crate::def::max(activity, -inf);
    crate::def::min(activity, inf)
}

/// `SCIProwRecalcLPActivity`: recalculates the current activity of a row.
pub fn row_recalc_lp_activity(scip: &mut Scip, row: RowId) {
    let activity = row_lp_activity(scip, row, |col| col.primsol);
    let r = &mut scip.rows[row];
    r.activity = activity;
    r.validactivitylp = scip.stat.lpcount;
}

/// `SCIProwGetLPActivity`: returns the activity of a row in the current LP
/// solution.
pub fn row_get_lp_activity(scip: &mut Scip, row: RowId) -> f64 {
    if scip.rows[row].validactivitylp != scip.stat.lpcount {
        row_recalc_lp_activity(scip, row);
    }
    clamp_activity(&scip.set, scip.rows[row].activity)
}

/// `SCIProwGetLPFeasibility`: returns the feasibility of a row in the
/// current LP solution: negative value means infeasibility.
pub fn row_get_lp_feasibility(scip: &mut Scip, row: RowId) -> f64 {
    let activity = row_get_lp_activity(scip, row);
    let r = &scip.rows[row];
    crate::def::min(r.rhs - activity, activity - r.lhs)
}

/// `SCIProwGetRelaxFeasibility`: returns the feasibility of a row in the
/// current relaxed solution: negative value means infeasibility.
pub fn row_get_relax_feasibility(scip: &Scip, row: RowId) -> f64 {
    let activity = row_lp_activity(scip, row, |col| crate::var::var_get_relax_sol(scip, col.var));
    let activity = clamp_activity(&scip.set, activity);
    let r = &scip.rows[row];
    crate::def::min(r.rhs - activity, activity - r.lhs)
}

/// `SCIProwGetNLPFeasibility`: returns the feasibility of a row in the
/// current NLP solution: negative value means infeasibility.
pub fn row_get_nlp_feasibility(scip: &Scip, row: RowId) -> f64 {
    let activity = row_lp_activity(scip, row, |col| crate::var::var_get_nlp_sol(scip, col.var));
    let activity = clamp_activity(&scip.set, activity);
    let r = &scip.rows[row];
    crate::def::min(r.rhs - activity, activity - r.lhs)
}

/// `SCIProwRecalcPseudoActivity`: calculates the current pseudo activity
/// of a row.
pub fn row_recalc_pseudo_activity(scip: &mut Scip, row: RowId) {
    let r = &scip.rows[row];
    let mut pseudoactivity = r.constant;
    for i in 0..r.len {
        pseudoactivity += col_get_best_bound(&scip.cols[r.cols[i]]) * r.vals[i];
    }
    let r = &mut scip.rows[row];
    r.pseudoactivity = pseudoactivity;
    r.validpsactivitydomchg = scip.stat.domchgcount;
}

/// `SCIProwGetPseudoActivity`: returns the pseudo activity of a row in the
/// current pseudo solution.
pub fn row_get_pseudo_activity(scip: &mut Scip, row: RowId) -> f64 {
    if scip.rows[row].validpsactivitydomchg != scip.stat.domchgcount {
        row_recalc_pseudo_activity(scip, row);
    }
    clamp_activity(&scip.set, scip.rows[row].pseudoactivity)
}

/// `SCIProwGetPseudoFeasibility`: returns the pseudo feasibility of a row
/// in the current pseudo solution: negative value means infeasibility.
pub fn row_get_pseudo_feasibility(scip: &mut Scip, row: RowId) -> f64 {
    let pseudoactivity = row_get_pseudo_activity(scip, row);
    let r = &scip.rows[row];
    crate::def::min(r.rhs - pseudoactivity, pseudoactivity - r.lhs)
}

/// `SCIProwGetSolActivity`: returns the activity of a row for a given
/// solution.
pub fn row_get_sol_activity(scip: &mut Scip, row: RowId, sol: SolId) -> f64 {
    let mut activity = scip.rows[row].constant;
    let len = scip.rows[row].len;
    for i in 0..len {
        let col = scip.rows[row].cols[i];
        let var = scip.cols[col].var;
        let mut solval = crate::sol::sol_get_val(scip, sol, var);
        let r = &scip.rows[row];
        let c = &scip.cols[col];
        if solval == crate::def::UNKNOWN {
            if scip.set.is_infinity(-r.lhs) {
                solval = if r.vals[i] >= 0.0 { c.lb } else { c.ub };
            } else if scip.set.is_infinity(r.rhs) {
                solval = if r.vals[i] >= 0.0 { c.ub } else { c.lb };
            } else {
                solval = (c.lb + c.ub) / 2.0;
            }
        }
        activity += r.vals[i] * solval;
    }
    clamp_activity(&scip.set, activity)
}

/// `SCIProwGetSolFeasibility`: returns the feasibility of a row for the
/// given solution.
pub fn row_get_sol_feasibility(scip: &mut Scip, row: RowId, sol: SolId) -> f64 {
    let activity = row_get_sol_activity(scip, row, sol);
    let r = &scip.rows[row];
    crate::def::min(r.rhs - activity, activity - r.lhs)
}

/// `rowCalcActivityBounds`: calculates minimal and maximal activity of row
/// w.r.t. the column's bounds.
fn row_calc_activity_bounds(scip: &mut Scip, row: RowId) {
    let set = &scip.set;
    let r = &mut scip.rows[row];
    let mut mininfinite = false;
    let mut maxinfinite = false;

    // Calculate activity bounds.
    r.minactivity = r.constant;
    r.maxactivity = r.constant;
    let mut i = 0;
    while i < r.len && (!mininfinite || !maxinfinite) {
        let col = &scip.cols[r.cols[i]];
        let val = r.vals[i];
        if val >= 0.0 {
            mininfinite = mininfinite || set.is_infinity(-col.lb);
            maxinfinite = maxinfinite || set.is_infinity(col.ub);
            if !mininfinite {
                r.minactivity += val * col.lb;
            }
            if !maxinfinite {
                r.maxactivity += val * col.ub;
            }
        } else {
            mininfinite = mininfinite || set.is_infinity(col.ub);
            maxinfinite = maxinfinite || set.is_infinity(-col.lb);
            if !mininfinite {
                r.minactivity += val * col.ub;
            }
            if !maxinfinite {
                r.maxactivity += val * col.lb;
            }
        }
        i += 1;
    }
    if mininfinite {
        r.minactivity = -set.infinity();
    }
    if maxinfinite {
        r.maxactivity = set.infinity();
    }
    r.validactivitybdsdomchg = scip.stat.domchgcount;
}

/// `SCIProwGetMinActivity`: returns the minimal activity of a row w.r.t.
/// the column's bounds.
pub fn row_get_min_activity(scip: &mut Scip, row: RowId) -> f64 {
    if scip.rows[row].validactivitybdsdomchg != scip.stat.domchgcount {
        row_calc_activity_bounds(scip, row);
    }
    scip.rows[row].minactivity
}

/// `SCIProwGetMaxActivity`: returns the maximal activity of a row w.r.t.
/// the column's bounds.
pub fn row_get_max_activity(scip: &mut Scip, row: RowId) -> f64 {
    if scip.rows[row].validactivitybdsdomchg != scip.stat.domchgcount {
        row_calc_activity_bounds(scip, row);
    }
    scip.rows[row].maxactivity
}

/// `SCIProwIsRedundant`: returns whether the row is unmodifiable and
/// redundant w.r.t. the columns' bounds.
pub fn row_is_redundant(scip: &mut Scip, row: RowId) -> bool {
    if scip.rows[row].modifiable {
        return false;
    }
    let lhs = scip.rows[row].lhs;
    if !scip.set.is_infinity(-lhs) {
        let minactivity = row_get_min_activity(scip, row);
        if scip.set.is_feas_lt(minactivity, lhs) {
            return false;
        }
    }
    let rhs = scip.rows[row].rhs;
    if !scip.set.is_infinity(rhs) {
        let maxactivity = row_get_max_activity(scip, row);
        if scip.set.is_feas_gt(maxactivity, rhs) {
            return false;
        }
    }
    true
}

/// `SCIProwGetMaxval`: gets maximal absolute value of row vector
/// coefficients.
pub fn row_get_maxval(scip: &mut Scip, row: RowId) -> f64 {
    if scip.rows[row].nummaxval == 0 {
        row_calc_idxs_and_vals(scip, row);
    }
    scip.rows[row].maxval
}

/// `SCIProwGetMinval`: gets minimal absolute value of row vector's
/// non-zero coefficients.
pub fn row_get_minval(scip: &mut Scip, row: RowId) -> f64 {
    if scip.rows[row].numminval == 0 {
        row_calc_idxs_and_vals(scip, row);
    }
    scip.rows[row].minval
}

/// `SCIProwGetMaxidx`: gets maximal column index of row entries.
pub fn row_get_maxidx(scip: &mut Scip, row: RowId) -> i32 {
    if !scip.rows[row].validminmaxidx {
        row_calc_idxs_and_vals(scip, row);
    }
    scip.rows[row].maxidx
}

/// `SCIProwGetMinidx`: gets minimal column index of row entries.
pub fn row_get_minidx(scip: &mut Scip, row: RowId) -> i32 {
    if !scip.rows[row].validminmaxidx {
        row_calc_idxs_and_vals(scip, row);
    }
    scip.rows[row].minidx
}

/// `SCIProwGetNumIntCols`: gets number of integral columns in row.
pub fn row_get_num_int_cols(scip: &mut Scip, row: RowId) -> i32 {
    if scip.rows[row].numintcols == -1 {
        row_calc_idxs_and_vals(scip, row);
    }
    scip.rows[row].numintcols
}

/// `SCIProwGetNumImpliedIntCols`: gets number of implied integral columns
/// in row.
pub fn row_get_num_implied_int_cols(scip: &mut Scip, row: RowId) -> i32 {
    if scip.rows[row].numimplintcols == -1 {
        row_calc_idxs_and_vals(scip, row);
    }
    scip.rows[row].numimplintcols
}

/// `SCIProwGetLPSolCutoffDistance`: returns row's cutoff distance in the
/// direction of the given primal solution.
pub fn row_get_lp_sol_cutoff_distance(scip: &mut Scip, row: RowId, sol: SolId) -> f64 {
    if lp!(scip).validsoldirlp != scip.stat.lpcount || lp!(scip).validsoldirsol != sol {
        let mut scale: f64 = 0.0;
        lp!(scip).validsoldirlp = scip.stat.lpcount;
        lp!(scip).validsoldirsol = sol;
        let ncols = lp!(scip).ncols;
        ensure_soldirection_size(scip, ncols);
        for k in 0..ncols {
            let col = lp!(scip).cols[k];
            let var = scip.cols[col].var;
            let d = crate::sol::sol_get_val(scip, sol, var) - scip.cols[col].primsol;
            lp!(scip).soldirection[k] = d;
            scale += d * d;
        }
        if scale > 0.0 {
            scale = 1.0 / scale.sqrt();
            for k in 0..ncols {
                lp!(scip).soldirection[k] *= scale;
            }
        }
    }
    let mut solcutoffdist = 0.0;
    {
        let r = &scip.rows[row];
        let lp = scip.lp.as_ref().unwrap();
        for k in 0..r.nlpcols {
            solcutoffdist += r.vals[k] * lp.soldirection[scip.cols[r.cols[k]].lppos];
        }
        for k in r.nlpcols..r.len {
            let lppos = scip.cols[r.cols[k]].lppos;
            if lppos >= 0 {
                solcutoffdist += r.vals[k] * lp.soldirection[lppos];
            }
        }
    }
    if scip.set.is_sum_zero(solcutoffdist) {
        solcutoffdist = scip.set.num_sumepsilon;
    }
    -row_get_lp_feasibility(scip, row) / solcutoffdist.abs()
}

/// The norm `separating/efficacynorm` selects, at least the sum epsilon
/// (the shared head of `SCIProwGetLPEfficacy` and its siblings).
fn row_efficacy_norm(scip: &mut Scip, row: RowId) -> f64 {
    let norm = match scip.set.sepa_efficacynorm {
        b'e' => row_get_norm(&scip.rows[row]),
        b'm' => row_get_maxval(scip, row),
        b's' => scip.rows[row].sumnorm,
        b'd' => {
            if scip.rows[row].len == 0 {
                0.0
            } else {
                1.0
            }
        }
        c => panic!("invalid efficacy norm parameter '{}'", c as char),
    };
    let eps = scip.set.sumepsilon();
    crate::def::max(norm, eps)
}

/// `SCIProwGetLPEfficacy`: returns the efficacy of the row in the current
/// LP solution: the violation in the norm of the row.
pub fn row_get_lp_efficacy(scip: &mut Scip, row: RowId) -> f64 {
    let norm = row_efficacy_norm(scip, row);
    let feasibility = row_get_lp_feasibility(scip, row);
    -feasibility / norm
}

/// `SCIProwIsLPEfficacious`: returns whether the row's efficacy with
/// respect to the current LP solution is greater than the minimal cut
/// efficacy.
pub fn row_is_lp_efficacious(scip: &mut Scip, row: RowId, root: bool) -> bool {
    let efficacy = row_get_lp_efficacy(scip, row);
    scip.set.is_efficacious(root, efficacy)
}

/// `SCIProwGetSolEfficacy`: returns the efficacy of the row with respect
/// to the given primal solution.
pub fn row_get_sol_efficacy(scip: &mut Scip, row: RowId, sol: SolId) -> f64 {
    let norm = row_efficacy_norm(scip, row);
    let feasibility = row_get_sol_feasibility(scip, row, sol);
    -feasibility / norm
}

/// `SCIProwIsSolEfficacious`: returns whether the row's efficacy with
/// respect to the given primal solution is greater than the minimal cut
/// efficacy.
pub fn row_is_sol_efficacious(scip: &mut Scip, row: RowId, sol: SolId, root: bool) -> bool {
    let efficacy = row_get_sol_efficacy(scip, row, sol);
    scip.set.is_efficacious(root, efficacy)
}

/// `SCIProwGetRelaxEfficacy`: returns the efficacy of the row with respect
/// to the relaxed solution.
pub fn row_get_relax_efficacy(scip: &mut Scip, row: RowId) -> f64 {
    let norm = row_efficacy_norm(scip, row);
    let feasibility = row_get_relax_feasibility(scip, row);
    -feasibility / norm
}

/// `SCIProwGetNLPEfficacy`: returns the efficacy of the row with respect
/// to the NLP solution.
pub fn row_get_nlp_efficacy(scip: &mut Scip, row: RowId) -> f64 {
    let norm = row_efficacy_norm(scip, row);
    let feasibility = row_get_nlp_feasibility(scip, row);
    -feasibility / norm
}

/// `SCIProwGetScalarProduct`: returns the scalar product of the coefficient
/// vectors of the two given rows, over the columns in the LP.
pub fn row_get_scalar_product(scip: &mut Scip, row1: RowId, row2: RowId) -> f64 {
    // Sort the column indices of both rows. The columns in a row are
    // divided into two parts, LP columns and non-LP columns, sorted within
    // each part; rows not yet linked to all their columns need up to four
    // partitions compared at once.
    row_sort(scip, row1);
    row_sort(scip, row2);
    let (mut r1, mut r2) = (&scip.rows[row1], &scip.rows[row2]);
    let cols = &scip.cols;
    let mut scalarprod;
    if (r1.nunlinked == 0 && r2.nunlinked == 0)
        || ((r1.nlpcols == r1.len || r1.nunlinked == r1.len) && (r2.nlpcols == r2.len || r2.nunlinked == r2.len) && (r1.nunlinked == 0 || r2.nunlinked == 0))
    {
        // Set the iterators to the last column we want to regard in the
        // row: nunlinked is either 0 or row->len, therefore, we get nlpcols
        // if nunlinked is 0 and row->len if the row is completely unlinked.
        let mut i1 = crate::def::max(r1.nlpcols, r1.nunlinked) - 1;
        let mut i2 = crate::def::max(r2.nlpcols, r2.nunlinked) - 1;
        scalarprod = 0.0;

        // Calculate the scalar product.
        while i1 >= 0 && i2 >= 0 {
            if r1.cols_index[i1] < r2.cols_index[i2] {
                i2 -= 1;
            } else if r1.cols_index[i1] > r2.cols_index[i2] {
                i1 -= 1;
            } else {
                scalarprod += r1.vals[i1] * r2.vals[i2];
                i1 -= 1;
                i2 -= 1;
            }
        }
    } else {
        scalarprod = 0.0;
        let mut ilp1 = 0;
        let mut ilp2 = 0;
        let mut inlp1 = if r1.nunlinked > 0 { r1.nlpcols } else { r1.len };
        let mut inlp2 = if r2.nunlinked > 0 { r2.nlpcols } else { r2.len };

        // Handle the case of four partitions (case 3) until one partition
        // is finished.
        while ilp1 < r1.nlpcols && inlp1 < r1.len && ilp2 < r2.nlpcols && inlp2 < r2.len {
            let (c1, c2) = (&r1.cols_index, &r2.cols_index);
            if c1[ilp1] == c2[ilp2] {
                scalarprod += r1.vals[ilp1] * r2.vals[ilp2];
                ilp1 += 1;
                ilp2 += 1;
            } else if c1[ilp1] == c2[inlp2] {
                scalarprod += r1.vals[ilp1] * r2.vals[inlp2];
                ilp1 += 1;
                inlp2 += 1;
            } else if c1[inlp1] == c2[ilp2] {
                scalarprod += r1.vals[inlp1] * r2.vals[ilp2];
                inlp1 += 1;
                ilp2 += 1;
            } else if c1[inlp1] == c2[inlp2] && cols[r1.cols[inlp1]].lppos >= 0 {
                scalarprod += r1.vals[inlp1] * r2.vals[inlp2];
                inlp1 += 1;
                inlp2 += 1;
            } else if c1[ilp1] < c1[inlp1] {
                if c2[ilp2] < c2[inlp2] {
                    if c1[ilp1] < c2[ilp2] {
                        ilp1 += 1;
                    } else {
                        ilp2 += 1;
                    }
                } else if c1[ilp1] < c2[inlp2] {
                    ilp1 += 1;
                } else {
                    inlp2 += 1;
                }
            } else if c2[ilp2] < c2[inlp2] {
                if c1[inlp1] < c2[ilp2] {
                    inlp1 += 1;
                } else {
                    ilp2 += 1;
                }
            } else if c1[inlp1] < c2[inlp2] {
                inlp1 += 1;
            } else {
                inlp2 += 1;
            }
        }

        // One partition was completely handled; if necessary, swap the
        // rows so that row1 is the one with only one remaining partition.
        if ilp1 != r1.nlpcols && inlp1 != r1.len {
            std::mem::swap(&mut r1, &mut r2);
            std::mem::swap(&mut ilp1, &mut ilp2);
            std::mem::swap(&mut inlp1, &mut inlp2);
        }

        // Determine section of row 1 that we want to look at (this merges
        // cases 4a and 4b).
        let (mut i1, end1, mut lpcols) = if ilp1 == r1.nlpcols { (inlp1, r1.len, false) } else { (ilp1, r1.nlpcols, true) };

        // Handle the case of three partitions (case 4) until one partition
        // is finished.
        while i1 < end1 && ilp2 < r2.nlpcols && inlp2 < r2.len {
            let (c1, c2) = (&r1.cols_index, &r2.cols_index);
            if c1[i1] == c2[ilp2] {
                scalarprod += r1.vals[i1] * r2.vals[ilp2];
                i1 += 1;
                ilp2 += 1;
            } else if c1[i1] == c2[inlp2] && (lpcols || cols[r1.cols[i1]].lppos >= 0) {
                scalarprod += r1.vals[i1] * r2.vals[inlp2];
                i1 += 1;
                inlp2 += 1;
            } else if c2[ilp2] < c2[inlp2] {
                if c1[i1] < c2[ilp2] {
                    i1 += 1;
                } else {
                    ilp2 += 1;
                }
            } else if c1[i1] < c2[inlp2] {
                i1 += 1;
            } else {
                inlp2 += 1;
            }
        }

        // If the second section of row 1 was finished, we can stop;
        // otherwise, we have to consider the remaining parts of the two
        // rows (the C leaves `lpcols` as it was for an LP section of row 2).
        if i1 < end1 {
            let (mut i2, end2) = if ilp2 == r2.nlpcols {
                lpcols = false;
                (inlp2, r2.len)
            } else {
                (ilp2, r2.nlpcols)
            };

            // Handle the case of two partitions (standard case 5, or case 1
            // or 2 due to the swapping).
            while i1 < end1 && i2 < end2 {
                if r1.cols_index[i1] == r2.cols_index[i2] && (lpcols || cols[r1.cols[i1]].lppos >= 0) {
                    scalarprod += r1.vals[i1] * r2.vals[i2];
                    i1 += 1;
                    i2 += 1;
                } else if r1.cols_index[i1] < r2.cols_index[i2] {
                    i1 += 1;
                } else {
                    i2 += 1;
                }
            }
        }
    }
    scalarprod
}

/// `SCIProwGetParallelism`: returns the degree of parallelism between the
/// hyperplanes defined by the two row vectors v, w: |v*w|/(|v|*|w|).
pub fn row_get_parallelism(scip: &mut Scip, row1: RowId, row2: RowId, orthofunc: u8) -> f64 {
    match orthofunc {
        b'e' => {
            let scalarprod = row_get_scalar_product(scip, row1, row2);
            if scalarprod == 0.0 {
                return 0.0;
            }

            // Norms may be wrong (bug 520, issue 44); if one is even 0,
            // reevaluate it here by hand.
            for row in [row1, row2] {
                if row_get_norm(&scip.rows[row]) == 0.0 {
                    let r = &scip.rows[row];
                    let mut sqrnorm = r.sqrnorm;
                    for i in 0..r.len {
                        if scip.cols[r.cols[i]].lppos >= 0 {
                            sqrnorm += r.vals[i] * r.vals[i];
                        }
                    }
                    scip.rows[row].sqrnorm = sqrnorm;
                }
            }
            scalarprod.abs() / (row_get_norm(&scip.rows[row1]) * row_get_norm(&scip.rows[row2]))
        }
        b'd' => todo!("SCIProwGetDiscreteScalarProduct: separating/orthofunc = d"),
        _ => panic!("invalid orthogonality function parameter '{}'", orthofunc as char),
    }
}

/// `SCIProwGetOrthogonality`: returns the degree of orthogonality between
/// the hyperplanes defined by the two row vectors v, w.
pub fn row_get_orthogonality(scip: &mut Scip, row1: RowId, row2: RowId, orthofunc: u8) -> f64 {
    1.0 - row_get_parallelism(scip, row1, row2, orthofunc)
}

/// `SCIProwGetObjParallelism`: gets parallelism of row with objective
/// function: if the returned value is 1, the row is parallel to the
/// objective function, if the value is 0, it is orthogonal.
pub fn row_get_obj_parallelism(scip: &mut Scip, row: RowId) -> f64 {
    if lp!(scip).objsqrnormunreliable {
        lp_recalculate_obj_sqr_norm(scip);
    }
    let r = &scip.rows[row];
    let prod = r.sqrnorm * lp!(scip).objsqrnorm;
    let mut parallelism = if scip.set.is_positive(prod) { r.objprod.abs() / prod.sqrt() } else { 0.0 };
    parallelism = crate::def::min(parallelism, 1.0);
    crate::def::max(parallelism, 0.0)
}

/// `SCIProwCatchEvent`: includes event handler with given data in row's
/// event filter.
pub fn row_catch_event(
    scip: &mut Scip,
    row: RowId,
    eventtype: crate::event::EventType,
    eventhdlr: crate::types::EventhdlrId,
    eventdata: usize,
    filterpos: Option<&mut i32>,
) -> ScipResult {
    let filter = scip.rows[row].eventfilter;
    crate::event::eventfilter_add(scip, filter, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIProwDropEvent`: deletes event handler with given data from row's
/// event filter.
pub fn row_drop_event(scip: &mut Scip, row: RowId, eventtype: crate::event::EventType, eventhdlr: crate::types::EventhdlrId, eventdata: usize, filterpos: i32) -> ScipResult {
    let filter = scip.rows[row].eventfilter;
    crate::event::eventfilter_del(scip, filter, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIProwMarkNotRemovableLocal`: marks a row to be not removable from
/// the LP in the current node.
pub fn row_mark_not_removable_local(scip: &mut Scip, row: RowId) {
    scip.rows[row].obsoletenode = scip.stat.nnodes;
}

/*
 * LP solver data update
 */

/// `markColDeleted`: resets column data to represent a column not in the
/// LP solver.
fn mark_col_deleted(col: &mut Col) {
    col.lpipos = -1;
    col.primsol = 0.0;
    col.redcost = INVALID;
    col.farkascoef = INVALID;
    col.sbdown = INVALID;
    col.sbup = INVALID;
    col.sbdownvalid = false;
    col.sbupvalid = false;
    col.validredcostlp = -1;
    col.validfarkaslp = -1;
    col.sbitlim = -1;
    col.basisstatus = BASESTAT_ZERO;
}

/// `lpFlushDelCols`: applies all cached column removals to the LP solver.
fn lp_flush_del_cols(scip: &mut Scip) -> ScipResult {
    // Find the first column to change.
    loop {
        let lp = scip.lp.as_ref().unwrap();
        let f = lp.lpifirstchgcol;
        if f < lp.nlpicols && f < lp.ncols && scip.cols[lp.cols[f]].lpipos == f && !scip.cols[lp.cols[f]].coefchanged {
            lp!(scip).lpifirstchgcol += 1;
        } else {
            break;
        }
    }

    // Shrink LP to the part which didn't change.
    let (first, nlpicols) = (lp!(scip).lpifirstchgcol, lp!(scip).nlpicols);
    if first < nlpicols {
        lpi::lpi_del_cols(&mut lpi!(scip), first, nlpicols - 1)?;
        for i in first..nlpicols {
            let col = lp!(scip).lpicols[i];
            mark_col_deleted(&mut scip.cols[col]);
        }
        let lp = &mut lp!(scip);
        lp.nlpicols = first;
        lp.flushdeletedcols = true;
        lp.updateintegrality = true;

        // Mark the LP unsolved.
        lp_mark_unsolved_primal(lp);
    }
    Ok(())
}

/// `computeLPBounds`: computes for the given column the lower and upper
/// bound that should be flushed into the LP depending on lazy bounds and
/// diving mode.
fn compute_lp_bounds(set: &crate::set::Set, lp: &Lp, col: &Col, lpiinf: f64) -> (f64, f64) {
    // Get the correct new lower bound: if lazy lower bound exists and is
    // larger than lower bound, set lower bound to infinity; if we are in
    // diving mode, ignore lazy bounds and always take the lower bound.
    let lb = if set.is_infinity(-col.lb) || (set.is_le(col.lb, col.lazylb) && !lp.diving) {
        -lpiinf
    } else {
        col.lb
    };

    // Get the correct new upper bound likewise.
    let ub = if set.is_infinity(col.ub) || (set.is_ge(col.ub, col.lazyub) && !lp.diving) {
        lpiinf
    } else {
        col.ub
    };
    (lb, ub)
}

/// `lpFlushAddCols`: applies all cached column additions to the LP solver.
fn lp_flush_add_cols(scip: &mut Scip) -> ScipResult {
    // If there are no columns to add, we are ready.
    let (ncols, nlpicols) = (lp!(scip).ncols, lp!(scip).nlpicols);
    if ncols == nlpicols {
        return Ok(());
    }

    // Add the additional columns.
    ensure_lpicols_size(scip, ncols);
    let lpiinf = lpi::lpi_infinity(&lpi!(scip));
    let naddcols = (ncols - nlpicols) as usize;
    let mut obj = Vec::with_capacity(naddcols);
    let mut lb = Vec::with_capacity(naddcols);
    let mut ub = Vec::with_capacity(naddcols);
    let mut beg = Vec::with_capacity(naddcols);
    let mut ind = Vec::new();
    let mut val = Vec::new();

    // Fill temporary memory with column data.
    for c in nlpicols..ncols {
        let col = lp!(scip).cols[c];

        // Because the column becomes a member of the LP solver, it now can
        // take values different from zero. That means, we have to include
        // the column in the corresponding row vectors.
        col_link(scip, col)?;
        lp!(scip).lpicols[c] = col;
        let (clb, cub) = compute_lp_bounds(&scip.set, scip.lp.as_ref().unwrap(), &scip.cols[col], lpiinf);
        let co = &mut scip.cols[col];
        co.lpipos = c;
        co.primsol = INVALID;
        co.redcost = INVALID;
        co.farkascoef = INVALID;
        co.sbdown = INVALID;
        co.sbup = INVALID;
        co.sbdownvalid = false;
        co.sbupvalid = false;
        co.validredcostlp = -1;
        co.validfarkaslp = -1;
        co.sbitlim = -1;
        co.objchanged = false;
        co.lbchanged = false;
        co.ubchanged = false;
        co.coefchanged = false;
        obj.push(co.obj);
        lb.push(clb);
        ub.push(cub);
        beg.push(ind.len() as i32);
        co.flushedobj = co.obj;
        co.flushedlb = clb;
        co.flushedub = cub;
        let co = &scip.cols[col];
        for i in 0..co.nlprows {
            let lpipos = scip.rows[co.rows[i]].lpipos;
            if lpipos >= 0 {
                ind.push(lpipos);
                val.push(co.vals[i]);
            }
        }
    }

    // Call LP interface.
    lpi::lpi_add_cols(&mut lpi!(scip), &obj, &lb, &ub, &beg, &ind, &val)?;
    let lp = &mut lp!(scip);
    lp.nlpicols = lp.ncols;
    lp.lpifirstchgcol = lp.nlpicols;
    lp.flushaddedcols = true;
    lp.updateintegrality = true;

    // Mark the LP unsolved.
    lp.solved = false;
    lp.dualfeasible = false;
    lp.dualchecked = false;
    lp.lpobjval = INVALID;
    lp.lpsolstat = LpSolStat::NotSolved;
    Ok(())
}

/// `markRowDeleted`: resets row data to represent a row not in the LP
/// solver.
fn mark_row_deleted(row: &mut Row) {
    row.lpipos = -1;
    row.dualsol = 0.0;
    row.activity = INVALID;
    row.dualfarkas = 0.0;
    row.basisstatus = BASESTAT_BASIC;
    row.validactivitylp = -1;
}

/// `lpFlushDelRows`: applies all cached row removals to the LP solver.
fn lp_flush_del_rows(scip: &mut Scip) -> ScipResult {
    // Find the first row to change.
    loop {
        let lp = scip.lp.as_ref().unwrap();
        let f = lp.lpifirstchgrow;
        if f < lp.nlpirows && f < lp.nrows && scip.rows[lp.rows[f]].lpipos == f && !scip.rows[lp.rows[f]].coefchanged {
            lp!(scip).lpifirstchgrow += 1;
        } else {
            break;
        }
    }

    // Shrink LP to the part which didn't change.
    let (first, nlpirows) = (lp!(scip).lpifirstchgrow, lp!(scip).nlpirows);
    if first < nlpirows {
        lpi::lpi_del_rows(&mut lpi!(scip), first, nlpirows - 1)?;
        for i in first..nlpirows {
            let row = lp!(scip).lpirows[i];
            mark_row_deleted(&mut scip.rows[row]);
            row_release(scip, row)?;
            lp!(scip).lpirows[i] = crate::var::NONE;
        }
        let lp = &mut lp!(scip);
        lp.nlpirows = first;
        lp.flushdeletedrows = true;

        // Mark the LP unsolved.
        lp.solved = false;
        lp.dualfeasible = false;
        lp.dualchecked = false;
        lp.lpobjval = INVALID;
        lp.lpsolstat = LpSolStat::NotSolved;
    }
    Ok(())
}

/// `lpFlushAddRows`: applies all cached row additions and removals to the
/// LP solver.
fn lp_flush_add_rows(scip: &mut Scip) -> ScipResult {
    // If there are no rows to add, we are ready.
    let (nrows, nlpirows) = (lp!(scip).nrows, lp!(scip).nlpirows);
    if nrows == nlpirows {
        return Ok(());
    }

    // Add the additional rows.
    ensure_lpirows_size(scip, nrows);
    let lpiinf = lpi::lpi_infinity(&lpi!(scip));
    let naddrows = (nrows - nlpirows) as usize;
    let mut lhs = Vec::with_capacity(naddrows);
    let mut rhs = Vec::with_capacity(naddrows);
    let mut beg = Vec::with_capacity(naddrows);
    let mut ind = Vec::new();
    let mut val = Vec::new();

    // Fill temporary memory with row data.
    for r in nlpirows..nrows {
        let row = lp!(scip).rows[r];

        // Because the row becomes a member of the LP solver, its dual
        // variable now can take values different from zero. That means, we
        // have to include the row in the corresponding column vectors.
        row_link(scip, row)?;
        row_capture(scip, row);
        lp!(scip).lpirows[r] = row;
        let set = &scip.set;
        let ro = &mut scip.rows[row];
        ro.lpipos = r;
        ro.dualsol = INVALID;
        ro.activity = INVALID;
        ro.dualfarkas = INVALID;
        ro.validactivitylp = -1;
        ro.lhschanged = false;
        ro.rhschanged = false;
        ro.coefchanged = false;
        let l = if set.is_infinity(-ro.lhs) { -lpiinf } else { ro.lhs - ro.constant };
        let h = if set.is_infinity(ro.rhs) { lpiinf } else { ro.rhs - ro.constant };
        lhs.push(l);
        rhs.push(h);
        beg.push(ind.len() as i32);
        ro.flushedlhs = l;
        ro.flushedrhs = h;
        let ro = &scip.rows[row];
        for i in 0..ro.nlpcols {
            let lpipos = scip.cols[ro.cols[i]].lpipos;
            if lpipos >= 0 {
                ind.push(lpipos);
                val.push(ro.vals[i]);
            }
        }
    }

    // Call LP interface.
    lpi::lpi_add_rows(&mut lpi!(scip), &lhs, &rhs, &beg, &ind, &val)?;
    let lp = &mut lp!(scip);
    lp.nlpirows = lp.nrows;
    lp.lpifirstchgrow = lp.nlpirows;
    lp.flushaddedrows = true;

    // Mark the LP unsolved.
    lp_mark_unsolved_primal(lp);
    Ok(())
}

/// `lpFlushChgCols`: applies all cached column bound and objective
/// changes to the LP.
fn lp_flush_chg_cols(scip: &mut Scip) -> ScipResult {
    if lp!(scip).nchgcols == 0 {
        return Ok(());
    }

    // Get the LP solver's infinity value.
    let lpiinf = lpi::lpi_infinity(&lpi!(scip));
    let mut objind = Vec::new();
    let mut obj = Vec::new();
    let mut bdind = Vec::new();
    let mut lb = Vec::new();
    let mut ub = Vec::new();

    // Collect all cached bound and objective changes.
    let nchgcols = lp!(scip).nchgcols;
    for i in 0..nchgcols {
        let col = lp!(scip).chgcols[i];
        if scip.cols[col].lpipos >= 0 {
            if scip.cols[col].objchanged {
                let co = &mut scip.cols[col];
                let newobj = co.obj;
                if co.flushedobj != newobj {
                    objind.push(co.lpipos);
                    obj.push(newobj);
                    co.flushedobj = newobj;
                }
                co.objchanged = false;
            }
            if scip.cols[col].lbchanged || scip.cols[col].ubchanged {
                let (newlb, newub) = compute_lp_bounds(&scip.set, scip.lp.as_ref().unwrap(), &scip.cols[col], lpiinf);
                let co = &mut scip.cols[col];
                if co.flushedlb != newlb || co.flushedub != newub {
                    bdind.push(co.lpipos);
                    lb.push(newlb);
                    ub.push(newub);
                    co.flushedlb = newlb;
                    co.flushedub = newub;
                }
                co.lbchanged = false;
                co.ubchanged = false;
            }
        }
    }

    // Change objective values in LP.
    if !objind.is_empty() {
        lpi::lpi_chg_obj(&mut lpi!(scip), &objind, &obj)?;

        // Mark the LP unsolved.
        let lp = &mut lp!(scip);
        lp.solved = false;
        lp.dualfeasible = false;
        lp.dualchecked = false;
        lp.lpobjval = INVALID;
        lp.lpsolstat = LpSolStat::NotSolved;
    }

    // Change bounds in LP.
    if !bdind.is_empty() {
        lpi::lpi_chg_bounds(&mut lpi!(scip), &bdind, &lb, &ub)?;

        // Mark the LP unsolved.
        lp_mark_unsolved_primal(&mut lp!(scip));
    }
    lp!(scip).nchgcols = 0;
    Ok(())
}

/// `lpFlushChgRows`: applies all cached row side changes to the LP.
fn lp_flush_chg_rows(scip: &mut Scip) -> ScipResult {
    if lp!(scip).nchgrows == 0 {
        return Ok(());
    }

    // Get the LP solver's infinity value.
    let lpiinf = lpi::lpi_infinity(&lpi!(scip));
    let mut ind = Vec::new();
    let mut lhs = Vec::new();
    let mut rhs = Vec::new();

    // Collect all cached left and right hand side changes.
    let nchgrows = lp!(scip).nchgrows;
    for i in 0..nchgrows {
        let row = lp!(scip).chgrows[i];
        let set = &scip.set;
        let ro = &mut scip.rows[row];
        if ro.lpipos >= 0 && (ro.lhschanged || ro.rhschanged) {
            let newlhs = if set.is_infinity(-ro.lhs) { -lpiinf } else { ro.lhs - ro.constant };
            let newrhs = if set.is_infinity(ro.rhs) { lpiinf } else { ro.rhs - ro.constant };
            if ro.flushedlhs != newlhs || ro.flushedrhs != newrhs {
                ind.push(ro.lpipos);
                lhs.push(newlhs);
                rhs.push(newrhs);
                ro.flushedlhs = newlhs;
                ro.flushedrhs = newrhs;
            }
            ro.lhschanged = false;
            ro.rhschanged = false;
        }
    }

    // Change left and right hand sides in LP.
    if !ind.is_empty() {
        lpi::lpi_chg_sides(&mut lpi!(scip), &ind, &lhs, &rhs)?;

        // Mark the LP unsolved.
        lp_mark_unsolved_primal(&mut lp!(scip));
    }
    lp!(scip).nchgrows = 0;
    Ok(())
}

/// `lpCopyIntegrality`: copy integrality information to the LP.
fn lp_copy_integrality(scip: &mut Scip) -> ScipResult {
    let ncols = lp!(scip).ncols;
    let mut integer_info = vec![0; ncols as usize];
    let mut nintegers = 0;
    for i in 0..ncols {
        let var = scip.cols[lp!(scip).cols[i]].var;
        if crate::var::var_is_integral(scip, var) || crate::var::var_is_binary(scip, var) {
            integer_info[i as usize] = 1;
            nintegers += 1;
        }
    }
    if nintegers > 0 {
        lpi::lpi_set_integrality_information(&mut lpi!(scip), &integer_info)?;
    } else {
        lpi::lpi_set_integrality_information(&mut lpi!(scip), &[])?;
    }
    lp!(scip).updateintegrality = false;
    Ok(())
}

/// `SCIPlpFlush`: applies all cached changes to the LP solver.
pub fn lp_flush(scip: &mut Scip, prob: usize) -> ScipResult {
    if !lp!(scip).flushed {
        let lp = &mut lp!(scip);
        lp.flushdeletedcols = false;
        lp.flushaddedcols = false;
        lp.flushdeletedrows = false;
        lp.flushaddedrows = false;
        lp_flush_del_cols(scip)?;
        lp_flush_del_rows(scip)?;
        lp_flush_chg_cols(scip)?;
        lp_flush_chg_rows(scip)?;
        lp_flush_add_cols(scip)?;
        lp_flush_add_rows(scip)?;
        lp!(scip).flushed = true;
    }

    // If the cutoff bound was changed in between and it is not disabled
    // (e.g. for column generation), we want to re-optimize the LP even if
    // nothing else has changed.
    if !lp_cutoff_disabled(scip, prob) && lp!(scip).cutoffbound != lp!(scip).lpiobjlim && lp!(scip).ncols > 0 {
        let lp = &mut lp!(scip);
        lp.solved = false;
        lp.lpsolstat = LpSolStat::NotSolved;
    }
    Ok(())
}

/// `SCIPlpMarkFlushed`: marks the LP to be flushed, even if the LP thinks
/// it is not flushed.
pub fn lp_mark_flushed(scip: &mut Scip) {
    let lp = &mut lp!(scip);
    lp.lpifirstchgcol = lp.nlpicols;
    lp.lpifirstchgrow = lp.nlpirows;

    // Reset LP changed flags.
    for i in 0..lp.nchgcols {
        let col = &mut scip.cols[lp.chgcols[i]];
        if col.lpipos >= 0 {
            col.objchanged = false;
            col.lbchanged = false;
            col.ubchanged = false;
        }
    }
    lp.nchgcols = 0;
    for i in 0..lp.nchgrows {
        let row = &mut scip.rows[lp.chgrows[i]];
        if row.lpipos >= 0 {
            row.lhschanged = false;
            row.rhschanged = false;
        }
    }
    lp.nchgrows = 0;
    lp.flushed = true;
}

/*
 * LP methods
 */

/// `colUpdateAddLP`: updates link data after addition of column to LP.
fn col_update_add_lp(scip: &mut Scip, col: ColId) {
    // Update column arrays of all linked rows.
    for i in 0..scip.cols[col].len {
        let pos = scip.cols[col].linkpos[i];
        if pos >= 0 {
            let row = scip.cols[col].rows[i];
            scip.rows[row].nlpcols += 1;
            let n = scip.rows[row].nlpcols;
            row_swap_coefs(scip, row, pos, n - 1);

            // If no swap was necessary, mark lpcols to be unsorted.
            if pos == n - 1 {
                scip.rows[row].lpcolssorted = false;
            }

            // Update norms.
            let v = scip.rows[row].vals[n - 1];
            row_add_norms(scip, row, col, v, false);
        }
    }
}

/// `rowUpdateAddLP`: updates link data after addition of row to LP.
fn row_update_add_lp(scip: &mut Scip, row: RowId) {
    // Update row arrays of all linked columns.
    for i in 0..scip.rows[row].len {
        let pos = scip.rows[row].linkpos[i];
        if pos >= 0 {
            let col = scip.rows[row].cols[i];
            scip.cols[col].nlprows += 1;
            let n = scip.cols[col].nlprows;
            col_swap_coefs(scip, col, pos, n - 1);

            // If no swap was necessary, mark lprows to be unsorted.
            if pos == n - 1 {
                scip.cols[col].lprowssorted = false;
            }
        }
    }
}

/// `colUpdateDelLP`: updates link data after removal of column from LP.
fn col_update_del_lp(scip: &mut Scip, col: ColId) {
    // Update column arrays of all linked rows.
    for i in 0..scip.cols[col].len {
        let pos = scip.cols[col].linkpos[i];
        if pos >= 0 {
            let row = scip.cols[col].rows[i];

            // Update norms.
            let v = scip.rows[row].vals[pos];
            row_del_norms(scip, row, col, v, true, false, false);
            scip.rows[row].nlpcols -= 1;
            let n = scip.rows[row].nlpcols;
            row_swap_coefs(scip, row, pos, n);

            // If no swap was necessary, mark nonlpcols to be unsorted.
            if pos == n {
                scip.rows[row].nonlpcolssorted = false;
            }
        }
    }
}

/// `rowUpdateDelLP`: updates link data after removal of row from LP.
fn row_update_del_lp(scip: &mut Scip, row: RowId) {
    // Update row arrays of all linked columns.
    for i in 0..scip.rows[row].len {
        let pos = scip.rows[row].linkpos[i];
        if pos >= 0 {
            let col = scip.rows[row].cols[i];
            scip.cols[col].nlprows -= 1;
            let n = scip.cols[col].nlprows;
            col_swap_coefs(scip, col, pos, n);

            // If no swap was necessary, mark lprows to be unsorted.
            if pos == n {
                scip.cols[col].nonlprowssorted = false;
            }
        }
    }
}

/// `reallocDiveChgSideArrays`: grows the arrays that store the row sides
/// changed during diving.
fn realloc_dive_chg_side_arrays(lp: &mut Lp, minsize: i32, growfact: f64) {
    if minsize <= lp.divechgsidessize {
        return;
    }
    lp.divechgsidessize = crate::def::max(minsize, (lp.divechgsidessize as f64 * growfact) as i32);
    lp.divechgsides.realloc(lp.divechgsidessize);
    lp.divechgsidetypes.realloc(lp.divechgsidessize);
    lp.divechgrows.realloc(lp.divechgsidessize);
}

/// `DIVESTACKINITSIZE`.
const DIVESTACKINITSIZE: i32 = 100;

/// `SCIPlpCreate`: creates empty LP data object.
pub fn lp_create(scip: &mut Scip, name: &str) -> ScipResult {
    let set = &scip.set;
    let lpi = lpi::lpi_create(name, lpi::OBJSEN_MINIMIZE)?;
    let lpiinf = lpi::lpi_infinity(&lpi);
    let mut lp = Lp {
        lpi: Some(Box::new(lpi)),
        lpicols: CVec::new(),
        lpirows: CVec::new(),
        chgcols: CVec::new(),
        chgrows: CVec::new(),
        cols: CVec::new(),
        soldirection: CVec::new(),
        lazycols: CVec::new(),
        rows: CVec::new(),
        lpobjval: 0.0,
        glbpseudoobjval: 0.0,
        relglbpseudoobjval: 0.0,
        glbpseudoobjvalid: true,
        glbpseudoobjvalinf: 0,
        pseudoobjval: 0.0,
        relpseudoobjval: 0.0,
        pseudoobjvalid: true,
        pseudoobjvalinf: 0,
        looseobjval: 0.0,
        rellooseobjval: 0.0,
        looseobjvalid: true,
        looseobjvalinf: 0,
        nloosevars: 0,
        rootlpobjval: INVALID,
        rootlooseobjval: INVALID,
        cutoffbound: set.infinity(),
        feastol: INVALID,
        degeneracy: 0.0,
        varconsratio: 0.0,
        validdegeneracylp: -1,
        objsqrnorm: 0.0,
        objsumnorm: 0.0,
        lpicolssize: 0,
        nlpicols: 0,
        lpirowssize: 0,
        nlpirows: 0,
        lpifirstchgcol: 0,
        lpifirstchgrow: 0,
        colssize: 0,
        soldirectionsize: 0,
        ncols: 0,
        lazycolssize: 0,
        nlazycols: 0,
        rowssize: 0,
        nrows: 0,
        chgcolssize: 0,
        nchgcols: 0,
        chgrowssize: 0,
        nchgrows: 0,
        firstnewcol: 0,
        firstnewrow: 0,
        nremovablecols: 0,
        nremovablerows: 0,
        // The initial (empty) LP is solved with primal and dual solution of
        // zero.
        validsollp: scip.stat.lpcount,
        validfarkaslp: -1,
        validsoldirlp: -1,
        validsoldirsol: crate::var::NONE,
        objsqrnormunreliable: false,
        flushdeletedcols: false,
        flushaddedcols: false,
        flushdeletedrows: false,
        flushaddedrows: false,
        updateintegrality: true,
        flushed: true,
        lpsolstat: LpSolStat::Optimal,
        solved: true,
        primalfeasible: true,
        primalchecked: true,
        dualfeasible: true,
        dualchecked: true,
        solisbasic: false,
        rootlpisrelax: true,
        isrelax: true,
        installing: false,
        strongbranching: false,
        strongbranchprobing: false,
        probing: false,
        diving: false,
        divingobjchg: false,
        divinglazyapplied: false,
        divelpistate: None,
        divelpwasprimfeas: true,
        divelpwasprimchecked: true,
        divelpwasdualfeas: true,
        divelpwasdualchecked: true,
        hasprovedbound: false,
        divechgsides: CVec::with_size(DIVESTACKINITSIZE),
        divechgsidetypes: CVec::with_size(DIVESTACKINITSIZE),
        divechgrows: CVec::with_size(DIVESTACKINITSIZE),
        ndivechgsides: 0,
        divechgsidessize: DIVESTACKINITSIZE,
        ndivingrows: 0,
        divinglpiitlim: i32::MAX,
        resolvelperror: false,
        divenolddomchgs: 0,
        adjustlpval: false,
        lpiobjlim: lpiinf,
        lpifeastol: 0.0,
        lpidualfeastol: set.dualfeastol(),
        lpibarrierconvtol: set.barrierconvtol(),
        lpifromscratch: false,
        lpifastmip: set.lp_fastmip,
        lpiscaling: set.lp_scaling,
        lpipresolving: set.lp_presolving,
        lpilpinfo: set.disp_lpinfo,
        lpirowrepswitch: set.lp_rowrepswitch,
        lpisolutionpolishing: set.lp_solutionpolishing > 0,
        lpirefactorinterval: set.lp_refactorinterval,
        lpiconditionlimit: set.lp_conditionlimit,
        lpimarkowitz: set.lp_markowitz,
        lpiitlim: i32::MAX,
        lpipricing: Pricing::Auto,
        lastlpalgo: LpAlgo::DualSimplex,
        lpithreads: set.lp_threads,
        lpitiming: set.time_clocktype,
        lpirandomseed: set.random_randomseed,
        storedsolvals: None,
        lpihasfeastol: false,
        lpihasdualfeastol: false,
        lpihasbarrierconvtol: false,
        lpihasfastmip: false,
        lpihasscaling: false,
        lpihaspresolving: false,
        lpihasrowrep: false,
        lpihaspolishing: false,
        lpihasrefactor: false,
    };
    lp_reset_feastol_of(&mut lp, set);
    lp.lpifeastol = lp.feastol;
    scip.lp = Some(Box::new(lp));

    // Set default parameters in LP solver.
    let objlim = lp!(scip).lpiobjlim;
    if !lp_set_realpar(scip, LpParam::ObjLim, objlim)? {
        log::info!("LP Solver: objective limit cannot be set -- can lead to unnecessary simplex iterations");
    }
    let v = lp!(scip).lpifeastol;
    lp!(scip).lpihasfeastol = lp_set_realpar(scip, LpParam::Feastol, v)?;
    let v = lp!(scip).lpidualfeastol;
    lp!(scip).lpihasdualfeastol = lp_set_realpar(scip, LpParam::DualFeastol, v)?;
    let v = lp!(scip).lpibarrierconvtol;
    lp!(scip).lpihasbarrierconvtol = lp_set_realpar(scip, LpParam::BarrierConvTol, v)?;
    let v = lp!(scip).lpifromscratch;
    lp_set_boolpar(scip, LpParam::FromScratch, v)?;
    let v = lp!(scip).lpifastmip;
    lp!(scip).lpihasfastmip = lp_set_intpar(scip, LpParam::FastMip, v)?;
    let v = lp!(scip).lpiscaling;
    lp!(scip).lpihasscaling = lp_set_intpar(scip, LpParam::Scaling, v)?;
    let v = lp!(scip).lpipresolving;
    lp!(scip).lpihaspresolving = lp_set_boolpar(scip, LpParam::Presolving, v)?;
    let v = lp!(scip).lpitiming;
    lp_set_intpar(scip, LpParam::Timing, v)?;
    let v = lp!(scip).lpiitlim;
    lp_set_intpar(scip, LpParam::LpItLim, v)?;
    let v = lp!(scip).lpipricing as i32;
    lp_set_intpar(scip, LpParam::Pricing, v)?;
    let v = lp!(scip).lpilpinfo;
    lp_set_boolpar(scip, LpParam::LpInfo, v)?;
    let v = lp!(scip).lpirowrepswitch;
    lp!(scip).lpihasrowrep = lp_set_realpar(scip, LpParam::RowRepSwitch, v)?;
    let v = if lp!(scip).lpisolutionpolishing { 1 } else { 0 };
    lp!(scip).lpihaspolishing = lp_set_intpar(scip, LpParam::Polishing, v)?;
    let v = lp!(scip).lpirefactorinterval;
    lp!(scip).lpihasrefactor = lp_set_intpar(scip, LpParam::Refactor, v)?;
    let v = lp!(scip).lpiconditionlimit;
    lp_set_realpar(scip, LpParam::ConditionLimit, v)?;
    let v = lp!(scip).lpimarkowitz;
    lp_set_realpar(scip, LpParam::Markowitz, v)?;
    let v = lp!(scip).lpithreads;
    lp_set_intpar(scip, LpParam::Threads, v)?;
    let v = lp!(scip).lpirandomseed;
    if v != 0 {
        lp_set_intpar(scip, LpParam::RandomSeed, v)?;
    }

    // Check that infinity value of LP-solver is at least as large as the
    // one used in SCIP.
    if scip.set.num_infinity > lpiinf {
        log::error!("The infinity value of the LP solver has to be at least as large as the one of SCIP.");
        return Err(Retcode::ParameterWrongVal);
    }
    Ok(())
}

/// `SCIPlpFree`: frees LP data object.
pub fn lp_free(scip: &mut Scip) -> ScipResult {
    lp_clear(scip)?;
    for i in 0..lp!(scip).nlpirows {
        let row = lp!(scip).lpirows[i];
        row_release(scip, row)?;
    }
    scip.lp = None;
    Ok(())
}

/// `SCIPlpReset`: resets the LP to the empty LP by removing all columns
/// and rows from LP, releasing all rows, and flushing the changes to the
/// LP solver.
pub fn lp_reset(scip: &mut Scip, prob: usize) -> ScipResult {
    lp_clear(scip)?;
    lp_flush(scip, prob)?;

    // Mark the empty LP to be solved.
    let lpcount = scip.stat.lpcount;
    let lp = &mut lp!(scip);
    lp.lpsolstat = LpSolStat::Optimal;
    lp.lpobjval = 0.0;
    lp.validsollp = lpcount;
    lp.validfarkaslp = -1;
    lp.validdegeneracylp = -1;
    lp.validsoldirlp = -1;
    lp.validsoldirsol = crate::var::NONE;
    lp.solved = true;
    lp.primalfeasible = true;
    lp.primalchecked = true;
    lp.dualfeasible = true;
    lp.dualchecked = true;
    lp.solisbasic = false;
    lp.lastlpalgo = LpAlgo::DualSimplex;
    Ok(())
}

/// `SCIPlpAddCol`: adds a column to the LP.
pub fn lp_add_col(scip: &mut Scip, col: ColId, depth: i32) -> ScipResult {
    let n = lp!(scip).ncols;
    ensure_cols_size(scip, n + 1);
    let lp = &mut lp!(scip);
    lp.cols[lp.ncols] = col;
    let c = &mut scip.cols[col];
    c.lppos = lp.ncols;
    c.lpdepth = depth;
    c.age = 0;
    lp.ncols += 1;
    if c.removable {
        lp.nremovablecols += 1;
    }
    let (lazylb, lazyub) = (c.lazylb, c.lazyub);
    if !scip.set.is_infinity(-lazylb) || !scip.set.is_infinity(lazyub) {
        let n = lp!(scip).nlazycols;
        ensure_lazycols_size(scip, n + 1);
        let lp = &mut lp!(scip);
        lp.lazycols[lp.nlazycols] = col;
        lp.nlazycols += 1;
    }

    // Mark the current LP unflushed.
    lp!(scip).flushed = false;

    // Update column arrays of all linked rows.
    col_update_add_lp(scip, col);

    // Update the objective function vector norms.
    let unchangedobj = scip.cols[col].unchangedobj;
    lp_update_obj_norms(scip, 0.0, unchangedobj);
    Ok(())
}

/// `SCIPlpAddRow`: adds a row to the LP and captures it.
pub fn lp_add_row(scip: &mut Scip, row: RowId, depth: i32) -> ScipResult {
    row_capture(scip, row);
    row_lock(scip, row);
    let n = lp!(scip).nrows;
    ensure_rows_size(scip, n + 1);
    let lp = &mut lp!(scip);
    lp.rows[lp.nrows] = row;
    let r = &mut scip.rows[row];
    r.lppos = lp.nrows;
    r.lpdepth = depth;
    r.age = 0;
    lp.nrows += 1;
    if r.removable {
        lp.nremovablerows += 1;
    }

    // Mark the current LP unflushed.
    lp.flushed = false;

    // Update row arrays of all linked columns.
    row_update_add_lp(scip, row);
    row_calc_norms(scip, row);

    // Check, if row addition to LP events are tracked; if so, issue
    // ROWADDEDLP event.
    let f = &scip.eventfilters[crate::scip::GLOBALEVENTFILTER];
    if f.len > 0 && (f.eventmask & crate::event::EVENTTYPE_ROWADDEDLP) != 0 {
        let event = crate::event::event_create_row_added_lp(row);
        crate::event::eventqueue_add(scip, event)?;
    }
    Ok(())
}

/// `SCIPlpShrinkCols`: removes all columns after the given number of
/// columns from the LP.
pub fn lp_shrink_cols(scip: &mut Scip, newncols: i32) -> ScipResult {
    if newncols < lp!(scip).ncols {
        let mut c = lp!(scip).ncols - 1;
        while c >= newncols {
            let col = lp!(scip).cols[c];
            let co = &mut scip.cols[col];
            co.lppos = -1;
            co.lpdepth = -1;
            let removable = co.removable;
            let lp = &mut lp!(scip);
            lp.ncols -= 1;
            if removable {
                lp.nremovablecols -= 1;
            }
            col_update_del_lp(scip, col);
            let unchangedobj = scip.cols[col].unchangedobj;
            lp_update_obj_norms(scip, unchangedobj, 0.0);
            c -= 1;
        }
        let lp = &mut lp!(scip);
        lp.lpifirstchgcol = crate::def::min(lp.lpifirstchgcol, newncols);

        // Remove columns which are deleted from the lazy column array.
        let mut c = 0;
        while c < lp.nlazycols {
            if scip.cols[lp.lazycols[c]].lppos < 0 {
                lp.lazycols[c] = lp.lazycols[lp.nlazycols - 1];
                lp.nlazycols -= 1;
            } else {
                c += 1;
            }
        }

        // Mark LP to be unflushed.
        lp.flushed = false;
    }
    Ok(())
}

/// `SCIPlpShrinkRows`: removes and releases all rows after the given
/// number of rows from the LP.
pub fn lp_shrink_rows(scip: &mut Scip, newnrows: i32) -> ScipResult {
    if newnrows < lp!(scip).nrows {
        let mut r = lp!(scip).nrows - 1;
        while r >= newnrows {
            let row = lp!(scip).rows[r];
            let ro = &mut scip.rows[row];
            ro.lppos = -1;
            ro.lpdepth = -1;
            let removable = ro.removable;
            let lp = &mut lp!(scip);
            lp.nrows -= 1;
            if removable {
                lp.nremovablerows -= 1;
            }
            row_update_del_lp(scip, row);
            row_unlock(scip, row);

            // Check, if row deletion events are tracked; if so, issue
            // ROWDELETEDLP event.
            let f = &scip.eventfilters[crate::scip::GLOBALEVENTFILTER];
            if f.len > 0 && (f.eventmask & crate::event::EVENTTYPE_ROWDELETEDLP) != 0 {
                let event = crate::event::event_create_row_deleted_lp(row);
                crate::event::eventqueue_add(scip, event)?;
            }
            row_release(scip, row)?;
            lp!(scip).rows[r] = crate::var::NONE;
            r -= 1;
        }
        let lp = &mut lp!(scip);
        lp.lpifirstchgrow = crate::def::min(lp.lpifirstchgrow, newnrows);

        // Mark LP to be unflushed.
        lp.flushed = false;
    }
    Ok(())
}

/// `SCIPlpClear`: removes all columns and rows from LP, releases all rows.
pub fn lp_clear(scip: &mut Scip) -> ScipResult {
    lp_shrink_cols(scip, 0)?;
    lp_shrink_rows(scip, 0)
}

/// `SCIPlpMarkSize`: remembers number of columns and rows to track the
/// newly added ones.
pub fn lp_mark_size(scip: &mut Scip) {
    let lp = &mut lp!(scip);
    lp.firstnewrow = lp.nrows;
    lp.firstnewcol = lp.ncols;
}

/// `SCIPlpSetSizeMark`: sets the remembered number of columns and rows to
/// the given values.
pub fn lp_set_size_mark(scip: &mut Scip, nrows: i32, ncols: i32) {
    let lp = &mut lp!(scip);
    lp.firstnewrow = nrows;
    lp.firstnewcol = ncols;
}

/// `SCIPlpGetBasisInd`: gets all indices of basic columns and rows: index
/// i >= 0 corresponds to column i, index i < 0 to row -i-1.
pub fn lp_get_basis_ind(scip: &mut Scip, basisind: &mut [i32]) -> ScipResult {
    lpi::lpi_get_basis_ind(&mut lpi!(scip), basisind)
}

/// `SCIPlpGetBase`: gets current basis status for columns and rows.
pub fn lp_get_base(scip: &mut Scip, cstat: Option<&mut [i32]>, rstat: Option<&mut [i32]>) -> ScipResult {
    lpi::lpi_get_base(&mut lpi!(scip), cstat, rstat)
}

/// `SCIPlpGetBInvRow`: gets a row from the inverse basis matrix B^-1.
pub fn lp_get_binv_row(scip: &mut Scip, r: i32, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    lpi::lpi_get_binv_row(&mut lpi!(scip), r, coef, inds, ninds)
}

/// `SCIPlpGetBInvCol`: gets a column from the inverse basis matrix B^-1.
pub fn lp_get_binv_col(scip: &mut Scip, c: i32, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    lpi::lpi_get_binv_col(&mut lpi!(scip), c, coef, inds, ninds)
}

/// `SCIPlpGetBInvARow`: gets a row from the product of inverse basis
/// matrix B^-1 and coefficient matrix A (as used in the simplex).
pub fn lp_get_binv_a_row(scip: &mut Scip, r: i32, binvrow: Option<&[f64]>, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    lpi::lpi_get_binv_a_row(&mut lpi!(scip), r, binvrow, coef, inds, ninds)
}

/// `SCIPlpGetBInvACol`: gets a column from the product of inverse basis
/// matrix B^-1 and coefficient matrix A (as used in the simplex).
pub fn lp_get_binv_a_col(scip: &mut Scip, c: i32, coef: &mut [f64], inds: Option<&mut [i32]>, ninds: Option<&mut i32>) -> ScipResult {
    lpi::lpi_get_binv_a_col(&mut lpi!(scip), c, coef, inds, ninds)
}

/// `SCIPlpSumRows`: calculates a weighted sum of all LP rows; for negative
/// weights, the left and right hand side of the corresponding LP row are
/// swapped in the summation. Returns (sumlhs, sumrhs).
pub fn lp_sum_rows(scip: &mut Scip, prob: usize, weights: &[f64], sumcoef: &mut crate::misc::DynArray<f64>) -> (f64, f64) {
    sumcoef.clear();
    sumcoef.extend(0, scip.probs[prob].nvars - 1);

    // Calculate the row summation.
    let mut sumlhs = 0.0;
    let mut sumrhs = 0.0;
    let mut lhsinfinite = false;
    let mut rhsinfinite = false;
    let lp = scip.lp.as_ref().unwrap();
    let set = &scip.set;
    for r in 0..lp.nrows {
        let w = weights[r as usize];
        if !set.is_zero(w) {
            let row = &scip.rows[lp.rows[r]];
            for i in 0..row.len {
                let idx = scip.cols[row.cols[i]].var_probindex;
                sumcoef.inc_val(idx, w * row.vals[i]);
            }
            if w > 0.0 {
                lhsinfinite = lhsinfinite || set.is_infinity(-row.lhs);
                if !lhsinfinite {
                    sumlhs += w * (row.lhs - row.constant);
                }
                rhsinfinite = rhsinfinite || set.is_infinity(row.rhs);
                if !rhsinfinite {
                    sumrhs += w * (row.rhs - row.constant);
                }
            } else {
                lhsinfinite = lhsinfinite || set.is_infinity(row.rhs);
                if !lhsinfinite {
                    sumlhs += w * (row.rhs - row.constant);
                }
                rhsinfinite = rhsinfinite || set.is_infinity(-row.lhs);
                if !rhsinfinite {
                    sumrhs += w * (row.lhs - row.constant);
                }
            }
        }
    }
    if lhsinfinite {
        sumlhs = -set.infinity();
    }
    if rhsinfinite {
        sumrhs = set.infinity();
    }
    (sumlhs, sumrhs)
}

/// `SCIPlpGetState`: stores LP state (like basis information) into LP
/// state object; `None` for the empty LP.
pub fn lp_get_state(scip: &mut Scip) -> ScipResult<Option<Box<lpi::LpiState>>> {
    if lp!(scip).nlpicols == 0 && lp!(scip).nlpirows == 0 {
        Ok(None)
    } else {
        Ok(Some(Box::new(lpi::lpi_get_state(&mut lpi!(scip))?)))
    }
}

/// `SCIPlpSetState`: loads LP state (like basis information) into solver.
pub fn lp_set_state(
    scip: &mut Scip,
    prob: usize,
    lpistate: Option<&lpi::LpiState>,
    wasprimfeas: bool,
    wasprimchecked: bool,
    wasdualfeas: bool,
    wasdualchecked: bool,
) -> ScipResult {
    // Flush changes to the LP solver.
    lp_flush(scip, prob)?;

    // Set LPI state in the LP solver.
    if lp!(scip).solved && lp!(scip).solisbasic {
        return Ok(());
    }
    match lpistate {
        None => lp!(scip).solisbasic = false,
        Some(state) => {
            lpi::lpi_set_state(&mut lpi!(scip), state)?;
            lp!(scip).solisbasic = lpi::lpi_has_state_basis(&lpi!(scip), state);
        }
    }

    // Setting feasibility to TRUE might be wrong because in probing mode,
    // the state is even saved when the LP was flushed and solved, also,
    // e.g., when we hit the iteration limit.
    let lp = &mut lp!(scip);
    lp.primalfeasible = wasprimfeas;
    lp.primalchecked = wasprimchecked;
    lp.dualfeasible = wasdualfeas;
    lp.dualchecked = wasdualchecked;
    Ok(())
}

/// `SCIPlpInterrupt`: interrupts or disables the interrupt of the
/// currently ongoing lp solve.
pub fn lp_interrupt(scip: &mut Scip, interrupt: bool) -> ScipResult {
    match lp!(scip).lpi.as_mut() {
        None => Ok(()),
        Some(lpi) => lpi::lpi_interrupt(lpi, interrupt),
    }
}

/// `SCIPlpGetNorms`: stores pricing norms into LP norms object; `None` for
/// the empty LP.
pub fn lp_get_norms(scip: &mut Scip) -> ScipResult<Option<Box<lpi::LpiNorms>>> {
    if lp!(scip).nlpicols == 0 && lp!(scip).nlpirows == 0 {
        Ok(None)
    } else {
        Ok(lpi::lpi_get_norms(&mut lpi!(scip))?.map(Box::new))
    }
}

/// `SCIPlpSetNorms`: loads pricing norms from LP norms object into solver.
pub fn lp_set_norms(scip: &mut Scip, lpinorms: Option<&lpi::LpiNorms>) -> ScipResult {
    if let Some(norms) = lpinorms {
        lpi::lpi_set_norms(&mut lpi!(scip), norms)?;
    }
    Ok(())
}

/// `SCIPlpGetCutoffbound`: return the current cutoff bound of the LP.
pub fn lp_get_cutoffbound(scip: &Scip) -> f64 {
    scip.lp.as_ref().unwrap().cutoffbound
}

/// `SCIPlpSetCutoffbound`: sets the upper objective limit of the LP solver.
pub fn lp_set_cutoffbound(scip: &mut Scip, prob: usize, cutoffbound: f64) -> ScipResult {
    // If the objective function was changed in diving, the cutoff bound
    // has no meaning (it will be set correctly in SCIPendDive()).
    if lp!(scip).divingobjchg {
        return Ok(());
    }

    // If the cutoff bound is increased, and the LP was proved to exceed the
    // old cutoff, it is no longer solved.
    if lp_get_solstat(scip) == LpSolStat::ObjLimit && cutoffbound > lp!(scip).cutoffbound {
        // Mark the current solution invalid.
        let lp = &mut lp!(scip);
        lp.solved = false;
        lp.lpobjval = INVALID;
        lp.lpsolstat = LpSolStat::NotSolved;
    }
    // If the cutoff bound is decreased below the current optimal value, the
    // LP now exceeds the objective limit; if the objective limit in the LP
    // solver was disabled, the solution status of the LP is not changed.
    else if !lp_cutoff_disabled(scip, prob) && lp_get_solstat(scip) == LpSolStat::Optimal && lp_get_objval(scip, prob) >= cutoffbound {
        lp!(scip).lpsolstat = LpSolStat::ObjLimit;
    }
    lp!(scip).cutoffbound = cutoffbound;
    Ok(())
}

/// `SCIPlpGetFeastol`: gets current primal feasibility tolerance of LP.
pub fn lp_get_feastol(scip: &Scip) -> f64 {
    scip.lp.as_ref().unwrap().feastol
}

/// `SCIPlpSetFeastol`: sets primal feasibility tolerance of LP.
fn lp_set_feastol_of(lp: &mut Lp, newfeastol: f64) {
    if newfeastol < lp.feastol {
        lp.solved = false;
        lp.lpsolstat = LpSolStat::NotSolved;
    }
    lp.feastol = newfeastol;
}

/// `SCIPlpSetFeastol`.
pub fn lp_set_feastol_extern(scip: &mut Scip, newfeastol: f64) {
    lp_set_feastol_of(&mut lp!(scip), newfeastol);
}

/// `SCIPlpResetFeastol` on a given LP.
fn lp_reset_feastol_of(lp: &mut Lp, set: &crate::set::Set) {
    if set.relaxfeastol() != INVALID {
        lp_set_feastol_of(lp, crate::def::min(set.relaxfeastol(), set.lp_feastol_factor() * set.feastol()));
    } else {
        lp_set_feastol_of(lp, set.lp_feastol_factor() * set.feastol());
    }
}

/// `SCIPlpResetFeastol`: resets primal feasibility tolerance of LP: sets
/// it to the minimum of the relaxation feasibility tolerance (if set) and
/// the LP feasibility tolerance factor times the feasibility tolerance.
pub fn lp_reset_feastol(scip: &mut Scip) {
    let lp = &mut **scip.lp.as_mut().unwrap();
    lp_reset_feastol_of(lp, &scip.set);
}

/// `lpalgoName`: returns the name of the given LP algorithm.
fn lpalgo_name(lpalgo: LpAlgo) -> &'static str {
    match lpalgo {
        LpAlgo::PrimalSimplex => "primal simplex",
        LpAlgo::DualSimplex => "dual simplex",
        LpAlgo::Barrier => "barrier",
        LpAlgo::BarrierCrossover => "barrier/crossover",
    }
}

/// Runs one LP solve with `lpalgo` and counts it (the shared body of
/// `lpPrimalSimplex`, `lpDualSimplex` and `lpBarrier`); returns whether an
/// unresolved LP error occurred.
fn lp_run_algorithm(scip: &mut Scip, lpalgo: LpAlgo, resolve: bool, keepsol: bool, instable: bool) -> ScipResult<bool> {
    let mut lperror = false;
    let time_enabled = scip.set.time_enabled;
    let divingorprobing = lp!(scip).diving || lp!(scip).probing;
    let strongbranchprobing = lp!(scip).strongbranchprobing;
    let barrier = matches!(lpalgo, LpAlgo::Barrier | LpAlgo::BarrierCrossover);

    // Start timing.
    let mut timedelta = 0.0;
    {
        let stat = &mut scip.stat;
        if divingorprobing {
            if strongbranchprobing {
                stat.strongbranchtime.start(time_enabled);
            } else {
                stat.divinglptime.start(time_enabled);
            }
        } else {
            let clock = match lpalgo {
                LpAlgo::PrimalSimplex => &mut stat.primallptime,
                LpAlgo::DualSimplex => &mut stat.duallptime,
                _ => &mut stat.barrierlptime,
            };
            clock.start(time_enabled);
            timedelta = -clock.get_time();
        }
        if instable && !barrier {
            stat.resolveinstablelptime.start(time_enabled);
        }
    }

    // Call LP solver.
    let retcode = match lpalgo {
        LpAlgo::PrimalSimplex => lpi::lpi_solve_primal(&mut lpi!(scip)),
        LpAlgo::DualSimplex => lpi::lpi_solve_dual(&mut lpi!(scip)),
        LpAlgo::Barrier => lpi::lpi_solve_barrier(&mut lpi!(scip), false),
        LpAlgo::BarrierCrossover => lpi::lpi_solve_barrier(&mut lpi!(scip), true),
    };
    if retcode == Err(Retcode::LpError) {
        lperror = true;
    } else {
        retcode?;
    }
    lp!(scip).lastlpalgo = lpalgo;
    lp!(scip).solisbasic = lpalgo != LpAlgo::Barrier;

    // Stop timing.
    {
        let stat = &mut scip.stat;
        if divingorprobing {
            if strongbranchprobing {
                stat.strongbranchtime.stop(time_enabled);
            } else {
                stat.divinglptime.stop(time_enabled);
            }
        } else {
            let clock = match lpalgo {
                LpAlgo::PrimalSimplex => &mut stat.primallptime,
                LpAlgo::DualSimplex => &mut stat.duallptime,
                _ => &mut stat.barrierlptime,
            };
            if barrier {
                clock.stop(time_enabled);
                timedelta += clock.get_time();
            } else {
                timedelta += clock.get_time();
                clock.stop(time_enabled);
            }
        }
        if instable && !barrier {
            stat.resolveinstablelptime.stop(time_enabled);
        }
    }

    // Count number of iterations.
    scip.stat.lpcount += 1;
    let iterations = lp_get_iterations(scip)? as i64;
    let lpifromscratch = lp!(scip).lpifromscratch;
    let stat = &mut scip.stat;
    if iterations > 0 {
        // Don't count the resolves after removing unused columns/rows.
        if !strongbranchprobing {
            stat.nlps += 1;
            stat.nlpiterations += iterations;
        }
        if !barrier && resolve && !lpifromscratch && stat.nlps > 1 {
            if lpalgo == LpAlgo::PrimalSimplex {
                stat.nprimalresolvelps += 1;
                stat.nprimalresolvelpiterations += iterations;
            } else {
                stat.ndualresolvelps += 1;
                stat.ndualresolvelpiterations += iterations;
            }
        }
        if instable && !barrier {
            stat.nresolveinstablelps += 1;
            stat.nresolveinstablelpiters += iterations;
        }
        if divingorprobing {
            if strongbranchprobing {
                stat.nsbdivinglps += 1;
                stat.nsbdivinglpiterations += iterations;
            } else {
                stat.lastdivenode = stat.nnodes;
                stat.ndivinglps += 1;
                stat.ndivinglpiterations += iterations;
            }
        } else {
            match lpalgo {
                LpAlgo::PrimalSimplex => {
                    stat.nprimallps += 1;
                    stat.nprimallpiterations += iterations;
                }
                LpAlgo::DualSimplex => {
                    stat.nduallps += 1;
                    stat.nduallpiterations += iterations;
                }
                _ => {
                    stat.nbarrierlps += 1;
                    stat.nbarrierlpiterations += iterations;
                }
            }
        }
    } else {
        if !divingorprobing {
            match lpalgo {
                LpAlgo::PrimalSimplex => {
                    stat.nprimalzeroitlps += 1;
                    stat.primalzeroittime += timedelta;
                }
                LpAlgo::DualSimplex => {
                    stat.ndualzeroitlps += 1;
                    stat.dualzeroittime += timedelta;
                }
                _ => {
                    stat.nbarrierzeroitlps += 1;
                    stat.barrierzeroittime += timedelta;
                }
            }
        }
        if keepsol && !lperror {
            let lpcount = stat.lpcount;
            let lp = &mut lp!(scip);
            if lp.validsollp == lpcount - 1 {
                lp.validsollp = lpcount;
            }
            if lp.validfarkaslp == lpcount - 1 {
                lp.validfarkaslp = lpcount;
            }
        }
    }
    Ok(lperror)
}

/// `lpAlgorithm`: solves the LP with the given algorithm; returns
/// (timelimit, lperror).
fn lp_algorithm(scip: &mut Scip, lpalgo: LpAlgo, resolve: bool, keepsol: bool, instable: bool) -> ScipResult<(bool, bool)> {
    // Check if a time limit is set, and set time limit for LP solver
    // accordingly.
    let mut lptimelimit = lpi::lpi_infinity(&lpi!(scip));
    if scip.set.istimelimitfinite {
        lptimelimit = scip.set.limit_time - scip.stat.solvingtime.get_time();
    }
    let mut success = false;
    if lptimelimit > 0.0 {
        success = lp_set_realpar(scip, LpParam::LpTiLim, lptimelimit)?;
    }
    if lptimelimit <= 0.0 || !success {
        return Ok((true, lptimelimit > 0.0));
    }
    let lperror = match lpalgo {
        LpAlgo::DualSimplex => {
            let set = &scip.set;
            if set.lp_lexdualalgo && (!set.lp_lexdualrootonly || scip.stat.maxdepth == 0) && (!set.lp_lexdualstalling || lp!(scip).installing) {
                todo!("lpLexDualSimplex: lp/lexdualalgo")
            } else {
                lp_run_algorithm(scip, lpalgo, resolve, keepsol, instable)?
            }
        }
        LpAlgo::PrimalSimplex => lp_run_algorithm(scip, lpalgo, resolve, keepsol, instable)?,
        LpAlgo::Barrier | LpAlgo::BarrierCrossover => lp_run_algorithm(scip, lpalgo, false, keepsol, false)?,
    };

    // Check for primal and dual feasibility.
    if !lperror {
        let (primalfeasible, dualfeasible) = lpi::lpi_get_sol_feasibility(&mut lpi!(scip))?;
        let lp = &mut lp!(scip);
        lp.primalfeasible = primalfeasible;
        lp.dualfeasible = dualfeasible;
    }
    Ok((false, lperror))
}

/// `MAXNUMTROUBLELPMSGS`: maximal number of numerical trouble messages
/// shown below verblevel full.
const MAXNUMTROUBLELPMSGS: i64 = 10;

/// `SCIP_VERBLEVEL_HIGH` and `SCIP_VERBLEVEL_FULL`.
const VERBLEVEL_HIGH: i32 = 4;
const VERBLEVEL_FULL: i32 = 5;

/// `lpNumericalTroubleMessage`: prints the message, counting those shown
/// below verblevel full.
fn lp_numerical_trouble_message(scip: &mut Scip, verblevel: i32, msg: std::fmt::Arguments) {
    if scip.set.disp_verblevel < VERBLEVEL_FULL {
        if verblevel <= VERBLEVEL_HIGH {
            if scip.stat.nnumtroublelpmsgs > MAXNUMTROUBLELPMSGS {
                return;
            }
            scip.stat.nnumtroublelpmsgs += 1;
        }
        if verblevel > scip.set.disp_verblevel {
            return;
        }
    }
    log::info!("(node {}) numerical troubles in LP {} -- {}", scip.stat.nnodes, scip.stat.nlps, msg);
}

/// `ignoreInstability`: calls the LP solver's instability ignore; returns
/// whether instability was successfully ignored.
fn ignore_instability(scip: &mut Scip, lpalgo: LpAlgo) -> ScipResult<bool> {
    let success = lpi::lpi_ignore_instability(&mut lpi!(scip))?;
    if success {
        lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("ignoring instability of {}", lpalgo_name(lpalgo)));
        if !scip.set.lp_checkdualfeas {
            lp!(scip).dualfeasible = true;
        }
        if !scip.set.lp_checkprimfeas {
            lp!(scip).primalchecked = true;
        }
    }
    Ok(success)
}

/// `FEASTOLTIGHTFAC`: factor to tighten the feasibility tolerances in a
/// resolve.
const FEASTOLTIGHTFAC: f64 = 0.001;

/// The test after each solve in `lpSolveStable`: stop on a time limit or a
/// stable solution (with the iteration limit test while `checkitlim`),
/// else try to ignore the instability if stability isn't checked.
fn lp_solve_stable_done(scip: &mut Scip, lpalgo: LpAlgo, timelimit: bool, lperror: bool, checkitlim: bool, itlimishard: bool) -> ScipResult<bool> {
    let lpi = &lpi!(scip);
    if timelimit || (!lperror && lpi::lpi_is_stable(lpi) && (!checkitlim || itlimishard || !lpi::lpi_is_iterlim_exc(lpi))) {
        return Ok(true);
    }
    if !scip.set.lp_checkstability && ignore_instability(scip, lpalgo)? {
        return Ok(true);
    }
    Ok(false)
}

/// `lpSolveStable`: solves the LP with the given LP algorithm, and tries
/// to resolve numerical problems; returns (timelimit, lperror).
#[allow(clippy::too_many_arguments)]
fn lp_solve_stable(
    scip: &mut Scip,
    prob: usize,
    mut lpalgo: LpAlgo,
    itlim: i32,
    harditlim: i32,
    mut resolve: bool,
    fastmip: i32,
    tightprimfeastol: bool,
    tightdualfeastol: bool,
    fromscratch: bool,
    scaling: i32,
    keepsol: bool,
) -> ScipResult<(bool, bool)> {
    // Solving the LP when loose variables with infinite best bound are
    // present is not implemented.
    if lp!(scip).looseobjvalinf > 0 {
        log::error!("cannot solve LP when loose variable with infinite best bound is present");
        return Err(Retcode::Error);
    }

    // Check, whether we solve with a simplex algorithm.
    let simplex = lpalgo == LpAlgo::PrimalSimplex || lpalgo == LpAlgo::DualSimplex;

    // Check whether the iteration limit is a hard one.
    let itlimishard = itlim == harditlim;

    // Check whether solution polishing should be used.
    let polishing = scip.set.lp_solutionpolishing;
    let usepolishing = if lp!(scip).lpihaspolishing
        && (polishing == 2
            || (polishing == 1 && scip.stat.nnodes == 1 && !lp!(scip).probing)
            || (polishing == 3 && ((lp!(scip).probing && !lp!(scip).strongbranchprobing) || lp!(scip).diving)))
    {
        if lp!(scip).updateintegrality {
            lp_copy_integrality(scip)?;
        }
        true
    } else {
        false
    };

    // Solve with given settings (usually fast but imprecise).
    let cutoffbound = lp!(scip).cutoffbound;
    if scip.set.is_infinity(cutoffbound) {
        lp_set_objlim(scip, prob, cutoffbound)?;
    } else {
        let looseobjval = get_finite_loose_objval(scip, prob);
        lp_set_objlim(scip, prob, cutoffbound - looseobjval)?;
    }
    lp_set_iteration_limit(scip, itlim)?;
    let feastol = lp!(scip).feastol;
    lp_set_feastol(scip, if tightprimfeastol { FEASTOLTIGHTFAC * feastol } else { feastol })?;
    let dualfeastol = scip.set.dualfeastol();
    lp_set_dualfeastol(scip, if tightdualfeastol { FEASTOLTIGHTFAC * dualfeastol } else { dualfeastol })?;
    let barrierconvtol = scip.set.barrierconvtol();
    lp_set_barrierconvtol(
        scip,
        if tightprimfeastol || tightdualfeastol {
            FEASTOLTIGHTFAC * barrierconvtol
        } else {
            barrierconvtol
        },
    )?;
    lp_set_fromscratch(scip, fromscratch)?;
    lp_set_fastmip(scip, fastmip)?;
    lp_set_scaling(scip, scaling)?;
    lp_set_presolving(scip, scip.set.lp_presolving)?;
    lp_set_rowrepswitch(scip, scip.set.lp_rowrepswitch)?;
    lp_set_pricing_char(scip, scip.set.lp_pricing)?;
    lp_set_threads(scip, scip.set.lp_threads)?;
    lp_set_lp_info(scip, scip.set.disp_lpinfo)?;
    lp_set_condition_limit(scip, scip.set.lp_conditionlimit)?;
    lp_set_markowitz(scip, scip.set.lp_markowitz)?;
    lp_set_timing(scip, scip.set.time_clocktype, scip.set.time_enabled)?;
    let seed = (scip.set.initialize_random_seed(scip.set.random_randomseed as u32) % i32::MAX as u32) as i32;
    lp_set_randomseed(scip, seed)?;
    lp_set_solution_polishing(scip, usepolishing)?;
    lp_set_refactor_interval(scip, scip.set.lp_refactorinterval)?;
    let (mut timelimit, mut lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, false)?;

    // After the first solve, do not use starting basis, since otherwise
    // the solver will probably think the basis is optimal without
    // performing scaling/change tolerances/presolving.
    resolve = false;

    // Check for stability; iteration limit exceeded is also treated like
    // instability if the iteration limit is soft.
    if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, true, itlimishard)? {
        return Ok((timelimit, lperror));
    }

    // In the following, whenever the LP iteration limit is exceeded in an
    // LP solving call, we leave out the remaining resolving calls with
    // changed settings and go directly to solving the LP from scratch.

    // If FASTMIP is turned on, solve again without FASTMIP (starts from
    // the solution of the last LP solving call); do this only if the
    // iteration limit was not exceeded in the last LP solving call.
    if fastmip > 0 && simplex && (lperror || !lpi::lpi_is_iterlim_exc(&lpi!(scip))) && lp_set_fastmip(scip, 0)? {
        lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("solve again with {} without FASTMIP", lpalgo_name(lpalgo)));
        (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
        if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, true, itlimishard)? {
            return Ok((timelimit, lperror));
        }
    }

    // If the iteration limit was exceeded in the last LP solving call, we
    // leave out the remaining resolving calls with changed settings and go
    // directly to solving the LP from scratch.
    if (lperror || !lpi::lpi_is_iterlim_exc(&lpi!(scip))) && lp_set_scaling(scip, if scaling > 0 { 0 } else { 1 })? {
        // Solve again with opposite scaling setting (starts from the
        // solution of the last LP solving call).
        lp_numerical_trouble_message(
            scip,
            VERBLEVEL_FULL,
            format_args!("solve again with {} {} scaling", lpalgo_name(lpalgo), if scaling == 0 { "with" } else { "without" }),
        );
        (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
        if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, true, itlimishard)? {
            return Ok((timelimit, lperror));
        }

        // Reset scaling.
        lp_set_scaling(scip, scaling)?;
    }

    // If the iteration limit was exceeded in the last LP solving call, we
    // leave out the remaining resolving calls with changed settings and go
    // directly to solving the LP from scratch.
    let presolving = scip.set.lp_presolving;
    if (lperror || !lpi::lpi_is_iterlim_exc(&lpi!(scip))) && lp_set_presolving(scip, !presolving)? {
        // Solve again with opposite presolving setting (starts from the
        // solution of the last LP solving call).
        lp_numerical_trouble_message(
            scip,
            VERBLEVEL_FULL,
            format_args!("solve again with {} {} presolving", lpalgo_name(lpalgo), if !presolving { "with" } else { "without" }),
        );
        (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
        if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, true, itlimishard)? {
            return Ok((timelimit, lperror));
        }

        // Reset presolving.
        lp_set_presolving(scip, presolving)?;
    }

    // Solve again with a tighter feasibility tolerance (starts from the
    // solution of the last LP solving call); do this only if the iteration
    // limit was not exceeded in the last LP solving call.
    if ((simplex && (!tightprimfeastol || !tightdualfeastol)) || (!tightprimfeastol && !tightdualfeastol)) && (lperror || !lpi::lpi_is_iterlim_exc(&lpi!(scip))) {
        let mut success = false;
        if !tightprimfeastol {
            let feastol = lp!(scip).feastol;
            success = lp_set_feastol(scip, FEASTOLTIGHTFAC * feastol)?;
        }
        let mut success2 = false;
        if !tightdualfeastol {
            success2 = lp_set_dualfeastol(scip, FEASTOLTIGHTFAC * scip.set.dualfeastol())?;
        }
        let mut success3 = false;
        if !simplex && !tightprimfeastol && !tightdualfeastol {
            success3 = lp_set_barrierconvtol(scip, FEASTOLTIGHTFAC * scip.set.barrierconvtol())?;
        }

        // Only resolve if at least one of the parameters was actually
        // changed in the LP solver.
        if success || success2 || success3 {
            lp_numerical_trouble_message(
                scip,
                VERBLEVEL_FULL,
                format_args!("solve again with {} with tighter primal and dual feasibility tolerance", lpalgo_name(lpalgo)),
            );
            (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
            if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, true, itlimishard)? {
                return Ok((timelimit, lperror));
            }

            // Reset feasibility tolerance.
            if !tightprimfeastol {
                let feastol = lp!(scip).feastol;
                lp_set_feastol(scip, feastol)?;
            }
            if !tightdualfeastol {
                lp_set_dualfeastol(scip, scip.set.dualfeastol())?;
            }
            if !simplex && !tightprimfeastol && !tightdualfeastol {
                lp_set_barrierconvtol(scip, scip.set.barrierconvtol())?;
            }
        }
    }

    // All LPs solved after this point are solved from scratch, so set the
    // LP iteration limit to the hard limit; the given iteration limit
    // might be a soft one to restrict resolving calls only.
    lp_set_iteration_limit(scip, harditlim)?;

    // If not already done, solve again from scratch.
    if !fromscratch && simplex && lp_set_fromscratch(scip, true)? {
        lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("solve again from scratch with {}", lpalgo_name(lpalgo)));
        (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
        if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, false, itlimishard)? {
            return Ok((timelimit, lperror));
        }
    }

    // Solve again, use other simplex this time.
    if simplex {
        lpalgo = if lpalgo == LpAlgo::PrimalSimplex { LpAlgo::DualSimplex } else { LpAlgo::PrimalSimplex };
        lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("solve again from scratch with {}", lpalgo_name(lpalgo)));
        (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
        if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, false, itlimishard)? {
            return Ok((timelimit, lperror));
        }

        // Solve again with opposite scaling and other simplex.
        if lp_set_scaling(scip, if scaling > 0 { 0 } else { 1 })? {
            lp_numerical_trouble_message(
                scip,
                VERBLEVEL_FULL,
                format_args!(
                    "solve again from scratch with {} {} scaling",
                    lpalgo_name(lpalgo),
                    if scaling == 0 { "with" } else { "without" }
                ),
            );
            (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
            if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, false, itlimishard)? {
                return Ok((timelimit, lperror));
            }

            // Reset scaling.
            lp_set_scaling(scip, scaling)?;
        }

        // Solve again with opposite presolving and other simplex.
        let presolving = scip.set.lp_presolving;
        if lp_set_presolving(scip, !presolving)? {
            lp_numerical_trouble_message(
                scip,
                VERBLEVEL_FULL,
                format_args!(
                    "solve again from scratch with {} {} presolving",
                    lpalgo_name(lpalgo),
                    if !presolving { "with" } else { "without" }
                ),
            );
            (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
            if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, false, itlimishard)? {
                return Ok((timelimit, lperror));
            }

            // Reset presolving.
            lp_set_presolving(scip, presolving)?;
        }

        // Solve again with tighter feasibility tolerance, use other simplex
        // this time.
        if !tightprimfeastol || !tightdualfeastol {
            let mut success = false;
            if !tightprimfeastol {
                let feastol = lp!(scip).feastol;
                success = lp_set_feastol(scip, FEASTOLTIGHTFAC * feastol)?;
            }
            let mut success2 = false;
            if !tightdualfeastol {
                success2 = lp_set_dualfeastol(scip, FEASTOLTIGHTFAC * scip.set.dualfeastol())?;
            }

            // Only resolve if at least one of the parameters was actually
            // changed in the LP solver.
            if success || success2 {
                lp_numerical_trouble_message(
                    scip,
                    VERBLEVEL_FULL,
                    format_args!("solve again from scratch with {} with tighter feasibility tolerance", lpalgo_name(lpalgo)),
                );
                (timelimit, lperror) = lp_algorithm(scip, lpalgo, resolve, keepsol, true)?;
                if lp_solve_stable_done(scip, lpalgo, timelimit, lperror, false, itlimishard)? {
                    return Ok((timelimit, lperror));
                }

                // Reset feasibility tolerance.
                if !tightprimfeastol {
                    let feastol = lp!(scip).feastol;
                    lp_set_feastol(scip, feastol)?;
                }
                if !tightdualfeastol {
                    lp_set_dualfeastol(scip, scip.set.dualfeastol())?;
                }
            }
        }
    }

    // Nothing worked -- exit with an LPERROR.
    lp_numerical_trouble_message(scip, VERBLEVEL_HIGH, format_args!("unresolved"));
    Ok((timelimit, true))
}

/// `adjustLPobjval`: adjust the LP objective value if it is greater/less
/// than +/- SCIPsetInfinity(); `messagehdlr` is C's non-NULL message
/// handler, which warns once.
fn adjust_lp_objval(scip: &mut Scip, messagehdlr: bool) {
    let inf = scip.set.infinity();
    let lp = &mut lp!(scip);
    if scip.set.is_infinity(lp.lpobjval) && lp.lpobjval != inf {
        if !lp.adjustlpval && messagehdlr {
            log::warn!("LP solution value is above SCIP's infinity value");
            lp.adjustlpval = true;
        }
        lp.lpobjval = inf;
    } else if scip.set.is_infinity(-lp.lpobjval) && lp.lpobjval != -inf {
        if !lp.adjustlpval && messagehdlr {
            log::warn!("LP solution value is below SCIP's -infinity value");
            lp.adjustlpval = true;
        }
        lp.lpobjval = -inf;
    }
}

/// `lpSolve`: solves the LP with the given algorithm and evaluates return
/// status; returns whether an unresolved LP error occurred.
#[allow(clippy::too_many_arguments)]
fn lp_solve(
    scip: &mut Scip,
    prob: usize,
    mut lpalgo: LpAlgo,
    resolveitlim: i32,
    harditlim: i32,
    needprimalray: bool,
    needdualray: bool,
    mut resolve: bool,
    fastmip: i32,
    tightprimfeastol: bool,
    tightdualfeastol: bool,
    fromscratch: bool,
    scaling: i32,
    keepsol: bool,
) -> ScipResult<bool> {
    let mut solvedprimal = false;
    let mut solveddual = false;
    let itlim = if resolve { resolveitlim } else { harditlim };
    loop {
        let (timelimit, lperror) = lp_solve_stable(
            scip,
            prob,
            lpalgo,
            itlim,
            harditlim,
            resolve,
            fastmip,
            tightprimfeastol,
            tightdualfeastol,
            fromscratch,
            scaling,
            keepsol,
        )?;

        // Only the first solve should be counted as resolving call.
        resolve = false;
        solvedprimal = solvedprimal || lp!(scip).lastlpalgo == LpAlgo::PrimalSimplex;
        solveddual = solveddual || lp!(scip).lastlpalgo == LpAlgo::DualSimplex;

        // Check for error.
        if lperror {
            let lp = &mut lp!(scip);
            lp.solved = false;
            lp.lpsolstat = LpSolStat::NotSolved;
            return Ok(true);
        }

        // Evaluate solution status.
        if timelimit {
            let inf = scip.set.infinity();
            let lp = &mut lp!(scip);
            lp.solved = true;
            lp.lpsolstat = LpSolStat::TimeLimit;
            lp.lpobjval = -inf;
            return Ok(false);
        }
        let inf = scip.set.infinity();
        if lpi::lpi_is_optimal(&lpi!(scip)) {
            lp!(scip).lpsolstat = LpSolStat::Optimal;
            let objval = lpi::lpi_get_objval(&mut lpi!(scip))?;
            lp!(scip).lpobjval = objval;
            adjust_lp_objval(scip, true);
            let lp = &mut lp!(scip);
            if !scip.set.is_infinity(lp.lpiobjlim) && scip.set.is_ge(lp.lpobjval, lp.lpiobjlim) {
                // The solver may return the optimal value, even if this is
                // greater or equal than the upper bound.
                lp.lpsolstat = LpSolStat::ObjLimit;
                lp.lpobjval = inf;
            }
        } else if lpi::lpi_is_objlim_exc(&lpi!(scip)) {
            let lp = &mut lp!(scip);
            lp.lpsolstat = LpSolStat::ObjLimit;
            lp.lpobjval = inf;
        } else if lpi::lpi_is_primal_infeasible(&lpi!(scip)) {
            // Make sure that we always have a dual ray if requested.
            if needdualray && !lpi::lpi_has_dual_ray(&lpi!(scip)) && !solveddual && lpalgo != LpAlgo::DualSimplex {
                lpalgo = LpAlgo::DualSimplex;
                continue;
            }
            let lp = &mut lp!(scip);
            lp.lpsolstat = LpSolStat::Infeasible;
            lp.lpobjval = inf;
        } else if lpi::lpi_exists_primal_ray(&lpi!(scip)) {
            // Make sure that we always have a primal ray if requested.
            if needprimalray && !lpi::lpi_is_primal_unbounded(&lpi!(scip)) && !solvedprimal && lpalgo != LpAlgo::PrimalSimplex {
                lpalgo = LpAlgo::PrimalSimplex;
                continue;
            }
            let lp = &mut lp!(scip);
            lp.lpsolstat = LpSolStat::UnboundedRay;
            lp.lpobjval = -inf;
        } else if lpi::lpi_is_iterlim_exc(&lpi!(scip)) {
            let objval = lpi::lpi_get_objval(&mut lpi!(scip))?;
            lp!(scip).lpobjval = objval;

            // The lpobjval might be infinite, e.g. if the LP solver was not
            // able to produce a valid bound while reaching the iteration
            // limit. In this case, we avoid the warning in adjustLPobjval().
            let lpiinf = lpi::lpi_infinity(&lpi!(scip));
            adjust_lp_objval(scip, objval.abs() != lpiinf);
            lp!(scip).lpsolstat = LpSolStat::IterLimit;
        } else if lpi::lpi_is_timelim_exc(&lpi!(scip)) {
            let lp = &mut lp!(scip);
            lp.lpobjval = -inf;
            lp.lpsolstat = LpSolStat::TimeLimit;
        } else if !solveddual && lpalgo != LpAlgo::DualSimplex {
            lpalgo = LpAlgo::DualSimplex;
            log::info!(
                "(node {}) solution status of LP {} could not be proven (internal status:{}) -- solve again with {}",
                scip.stat.nnodes,
                scip.stat.nlps,
                lpi::lpi_get_internal_status(&lpi!(scip)),
                lpalgo_name(lpalgo)
            );
            continue;
        } else if !solvedprimal && lpalgo != LpAlgo::PrimalSimplex {
            lpalgo = LpAlgo::PrimalSimplex;
            log::info!(
                "(node {}) solution status of LP {} could not be proven (internal status:{}) -- solve again with {}",
                scip.stat.nnodes,
                scip.stat.nlps,
                lpi::lpi_get_internal_status(&lpi!(scip)),
                lpalgo_name(lpalgo)
            );
            continue;
        } else {
            log::error!(
                "(node {}) error or unknown return status of {} in LP {} (internal status: {})",
                scip.stat.nnodes,
                lpalgo_name(lp!(scip).lastlpalgo),
                scip.stat.nlps,
                lpi::lpi_get_internal_status(&lpi!(scip))
            );
            lp!(scip).lpsolstat = LpSolStat::Error;
            return Err(Retcode::LpError);
        }
        lp!(scip).solved = true;
        return Ok(false);
    }
}

/// `lpFlushAndSolve`: flushes the LP and solves it with the primal or dual
/// simplex algorithm, depending on the current basis feasibility; returns
/// whether an unresolved LP error occurred.
#[allow(clippy::too_many_arguments)]
fn lp_flush_and_solve(
    scip: &mut Scip,
    prob: usize,
    resolveitlim: i32,
    harditlim: i32,
    needprimalray: bool,
    needdualray: bool,
    mut fastmip: i32,
    tightprimfeastol: bool,
    tightdualfeastol: bool,
    fromscratch: bool,
    scaling: i32,
    keepsol: bool,
) -> ScipResult<bool> {
    // Flush changes to the LP solver.
    lp_flush(scip, prob)?;

    // Turn off FASTMIP if columns were changed.
    let lp = &mut lp!(scip);
    fastmip = if !lp.flushaddedcols && !lp.flushdeletedcols { fastmip } else { 0 };

    // Select LP algorithm to apply.
    let resolve = lp.solisbasic && (lp.dualfeasible || lp.primalfeasible) && !fromscratch;
    let algo = if resolve { scip.set.lp_resolvealgorithm } else { scip.set.lp_initalgorithm };
    lp.hasprovedbound = false;
    let lpalgo = match algo {
        b's' => {
            // Select simplex method.
            if lp.dualfeasible || !lp.primalfeasible {
                LpAlgo::DualSimplex
            } else {
                LpAlgo::PrimalSimplex
            }
        }
        b'p' => LpAlgo::PrimalSimplex,
        b'd' => LpAlgo::DualSimplex,
        b'b' => LpAlgo::Barrier,
        b'c' => LpAlgo::BarrierCrossover,
        _ => {
            log::error!("invalid parameter setting <{}> for LP algorithm", algo as char);
            return Err(Retcode::ParameterWrongVal);
        }
    };
    lp_solve(
        scip,
        prob,
        lpalgo,
        resolveitlim,
        harditlim,
        needprimalray,
        needdualray,
        resolve,
        fastmip,
        tightprimfeastol,
        tightdualfeastol,
        fromscratch,
        scaling,
        keepsol,
    )
}

/// `updateLazyBounds`: marks all lazy columns to be changed; this is
/// needed for reloading/removing bounds of these columns before and after
/// diving.
fn update_lazy_bounds(scip: &mut Scip) {
    // Return, if we are in diving, and bounds were already applied, or if
    // we are not in diving and bounds were not applied.
    if lp!(scip).diving == lp!(scip).divinglazyapplied {
        return;
    }
    for c in 0..lp!(scip).nlazycols {
        let col = lp!(scip).lazycols[c];

        // If the column has a lazy lower bound, mark its lower bounds as
        // changed.
        if !scip.set.is_infinity(-scip.cols[col].lazylb) {
            insert_col_chgcols(scip, col);
            scip.cols[col].lbchanged = true;
        }

        // If the column has a lazy upper bound, mark its upper bounds as
        // changed.
        if !scip.set.is_infinity(scip.cols[col].lazyub) {
            insert_col_chgcols(scip, col);
            scip.cols[col].ubchanged = true;
        }
    }

    // Update lp->divinglazyapplied flag: if we are in diving mode, we just
    // applied the lazy bounds, if not, we just removed them.
    let lp = &mut lp!(scip);
    lp.divinglazyapplied = lp.diving;
}

/// `lpGetResolveItlim`: returns the iteration limit for an LP resolving
/// call.
fn lp_get_resolve_itlim(scip: &Scip, mut itlim: i32) -> i32 {
    let (set, stat) = (&scip.set, &scip.stat);
    if set.lp_resolveiterfac == -1.0 || stat.nlps - stat.nrootlps < 5 {
        return itlim;
    }
    if itlim == -1 {
        itlim = i32::MAX;
    }
    let avg = set.lp_resolveiterfac * (stat.nlpiterations - stat.nrootlpiterations) as f64 / (stat.nlps - stat.nrootlps) as f64;
    let mx = crate::def::max(set.lp_resolveitermin as f64, avg);
    crate::def::min(itlim as f64, mx) as i32
}

/// Whether `lpalgo` is a simplex algorithm.
fn is_simplex(lpalgo: LpAlgo) -> bool {
    lpalgo == LpAlgo::PrimalSimplex || lpalgo == LpAlgo::DualSimplex
}

/// `SCIPlpSolveAndEval`: solves the LP with simplex algorithm, and copy
/// the solution into the column's data; returns whether an unresolved LP
/// error occurred.
#[allow(clippy::too_many_arguments)]
pub fn lp_solve_and_eval(scip: &mut Scip, prob: usize, itlim: i64, limitresolveiters: bool, mut aging: bool, keepsol: bool, forcedlpsolve: bool) -> ScipResult<bool> {
    let mut retcode: ScipResult = Ok(());
    let mut lperror = false;
    if lp!(scip).flushed && lp!(scip).solved {
        return Ok(false);
    }

    // Check whether we need a proof of unboundedness or infeasibility by a
    // primal or dual ray.
    let needprimalray = true;
    let needdualray = !crate::prob::prob_all_cols_in_lp(scip, prob) || (scip.set.conf_enable && scip.set.conf_useinflp != b'o');

    // Compute the limit for the number of LP resolving iterations, if
    // needed (i.e. if limitresolveiters == TRUE).
    let harditlim = crate::def::min(itlim, i32::MAX as i64) as i32;
    let resolveitlim = if limitresolveiters { lp_get_resolve_itlim(scip, harditlim) } else { harditlim };

    // If there are lazy bounds, check whether the bounds should explicitly
    // be put into the LP (diving was started) or removed from the LP
    // (diving was ended).
    if lp!(scip).nlazycols > 0 {
        update_lazy_bounds(scip);
    }

    // Flush changes to the LP solver.
    lp_flush(scip, prob)?;

    // If the time limit was reached in the last call and the LP did not
    // change, lp->solved is set to TRUE, but we want to run again anyway,
    // since there seems to be some time left / the time limit was
    // increased.
    if !lp!(scip).solved || (lp!(scip).lpsolstat == LpSolStat::TimeLimit && scip.stat.status != crate::types::Status::TimeLimit) {
        let mut primalfeasible;
        let mut dualfeasible;
        let mut farkasvalid;
        let mut rayfeasible;
        let mut tightprimfeastol = false;
        let mut tightdualfeastol = false;
        let mut fromscratch = false;
        let wasfromscratch = scip.stat.nlps == 0;
        let mut scaling = scip.set.lp_scaling;

        // Set initial LP solver settings.
        let lp = &lp!(scip);
        let mut fastmip = if lp.lpihasfastmip && !lp.flushaddedcols && !lp.flushdeletedcols && scip.stat.nnodes > 1 {
            scip.set.lp_fastmip
        } else {
            0
        };
        'solveagain: loop {
            let oldnlps = scip.stat.nlps;
            lperror = lp_flush_and_solve(
                scip,
                prob,
                resolveitlim,
                harditlim,
                needprimalray,
                needdualray,
                fastmip,
                tightprimfeastol,
                tightdualfeastol,
                fromscratch,
                scaling,
                keepsol,
            )?;
            if lperror {
                break 'solveagain;
            }

            // Evaluate solution status.
            match lp_get_solstat(scip) {
                LpSolStat::Optimal => {
                    // Get LP solution and possibly check the solution's
                    // feasibility again.
                    let checkprim = scip.set.lp_checkprimfeas;
                    let checkdual = scip.set.lp_checkdualfeas;
                    lp!(scip).primalchecked = checkprim;
                    lp!(scip).dualchecked = checkdual;
                    let (pf, df) = lp_get_sol(scip, checkprim, checkdual)?;
                    primalfeasible = if checkprim { pf } else { true };
                    dualfeasible = if checkdual { df } else { true };

                    // Update ages and remove obsolete columns and rows from
                    // LP.
                    if primalfeasible && dualfeasible && aging && !lp!(scip).diving && scip.stat.nlps > oldnlps {
                        lp_update_ages(scip)?;
                        if scip.stat.nlps % ((scip.set.lp_rowagelimit + 1) / 2 + 1) as i64 == 0 {
                            lp_remove_new_obsoletes(scip)?;
                        }
                        if !lp!(scip).solved {
                            // Resolve LP after removing obsolete columns
                            // and rows; to prevent infinite loops, no more
                            // aging.
                            aging = false;
                            continue 'solveagain;
                        }
                    }
                    if !primalfeasible || !dualfeasible {
                        let simplex = is_simplex(lp!(scip).lastlpalgo);
                        if fastmip > 0 && simplex {
                            // Solution is infeasible (this can happen due to
                            // numerical problems): solve again without
                            // FASTMIP.
                            log::info!(
                                "(node {}) solution of LP {} not optimal (pfeas={}, dfeas={}) -- solving again without FASTMIP",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                dualfeasible as u32
                            );
                            fastmip = 0;
                            continue 'solveagain;
                        } else if (!primalfeasible && !tightprimfeastol) || (!dualfeasible && !tightdualfeastol) {
                            // Solution is infeasible (this can happen due to
                            // numerical problems): solve again with tighter
                            // feasibility tolerance.
                            log::info!(
                                "(node {}) solution of LP {} not optimal (pfeas={}, dfeas={}) -- solving again with tighter feasibility tolerance",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                dualfeasible as u32
                            );
                            tightprimfeastol = tightprimfeastol || !primalfeasible;
                            tightdualfeastol = tightdualfeastol || !dualfeasible;
                            continue 'solveagain;
                        } else if !fromscratch && !wasfromscratch && simplex {
                            // Solution is infeasible (this can happen due to
                            // numerical problems): solve again from scratch.
                            log::info!(
                                "(node {}) solution of LP {} not optimal (pfeas={}, dfeas={}) -- solving again from scratch",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                dualfeasible as u32
                            );
                            fromscratch = true;
                            continue 'solveagain;
                        } else {
                            lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("unresolved"));
                            let lp = &mut lp!(scip);
                            lp.solved = false;
                            lp.lpsolstat = LpSolStat::NotSolved;
                            lperror = true;
                        }
                    }
                }
                LpSolStat::Infeasible => {
                    if scip.set.lp_checkfarkas || scip.set.lp_alwaysgetduals || !crate::prob::prob_all_cols_in_lp(scip, prob) {
                        if lpi::lpi_has_dual_ray(&lpi!(scip)) {
                            farkasvalid = lp_get_dualfarkas(scip, forcedlpsolve, true)?;
                        }
                        // It might happen that we have no infeasibility
                        // proof for the current LP (e.g. if the LP was
                        // always solved with the primal simplex due to
                        // numerical problems) - treat this case like an LP
                        // error.
                        else {
                            log::info!("(node {}) infeasibility of LP {} could not be proven by dual ray", scip.stat.nnodes, scip.stat.nlps);
                            let lp = &mut lp!(scip);
                            lp.solved = false;
                            lp.lpsolstat = LpSolStat::NotSolved;
                            farkasvalid = false;
                            lperror = true;
                        }
                    } else {
                        farkasvalid = true;
                    }

                    // If the LP solver does not provide a Farkas proof we
                    // don't want to resolve the LP.
                    if !farkasvalid && !lperror {
                        let simplex = is_simplex(lp!(scip).lastlpalgo);
                        if fastmip > 0 && simplex {
                            // The Farkas proof does not prove infeasibility
                            // (this can happen due to numerical problems):
                            // solve again without FASTMIP.
                            log::info!(
                                "(node {}) proof of infeasible LP {} not valid -- solving again without FASTMIP",
                                scip.stat.nnodes,
                                scip.stat.nlps
                            );
                            fastmip = 0;
                            continue 'solveagain;
                        } else if !tightdualfeastol {
                            // The Farkas proof does not prove infeasibility:
                            // solve again with tighter feasibility tolerance.
                            log::info!(
                                "(node {}) proof of infeasible LP {} not valid -- solving again with tighter dual feasibility tolerance",
                                scip.stat.nnodes,
                                scip.stat.nlps
                            );
                            tightdualfeastol = true;
                            continue 'solveagain;
                        } else if !fromscratch && simplex {
                            // The Farkas proof does not prove infeasibility:
                            // solve again from scratch.
                            log::info!(
                                "(node {}) proof of infeasible LP {} not valid -- solving again from scratch",
                                scip.stat.nnodes,
                                scip.stat.nlps
                            );
                            fromscratch = true;
                            continue 'solveagain;
                        } else {
                            // Nothing helped: forget about the LP at this node
                            // and mark it to be unsolved.
                            lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("unresolved, LP infeasible"));
                            let lp = &mut lp!(scip);
                            lp.solved = false;
                            lp.lpsolstat = LpSolStat::NotSolved;
                            lperror = true;
                        }
                    }
                }
                LpSolStat::UnboundedRay => {
                    if scip.set.lp_checkprimfeas {
                        (primalfeasible, rayfeasible) = lp_get_unbounded_sol(scip, true)?;
                        lp!(scip).primalchecked = true;
                    } else {
                        lp_get_unbounded_sol(scip, false)?;
                        primalfeasible = true;
                        rayfeasible = true;
                        lp!(scip).primalchecked = false;
                    }
                    if !primalfeasible || !rayfeasible {
                        let simplex = is_simplex(lp!(scip).lastlpalgo);
                        if fastmip > 0 && simplex {
                            // Unbounded solution is infeasible (this can
                            // happen due to numerical problems): solve again
                            // without FASTMIP.
                            log::info!(
                                "(node {}) solution of unbounded LP {} not optimal (pfeas={}, rfeas={}) -- solving again without FASTMIP",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                rayfeasible as u32
                            );
                            fastmip = 0;
                            continue 'solveagain;
                        } else if !tightprimfeastol {
                            // Solve again with tighter feasibility tolerance.
                            log::info!(
                                "(node {}) solution of unbounded LP {} not optimal (pfeas={}, rfeas={}) -- solving again with tighter primal feasibility tolerance",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                rayfeasible as u32
                            );
                            tightprimfeastol = true;
                            continue 'solveagain;
                        } else if !fromscratch && simplex {
                            // Solve again from scratch.
                            log::info!(
                                "(node {}) solution of unbounded LP {} not optimal (pfeas={}, rfeas={}) -- solving again from scratch",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                rayfeasible as u32
                            );
                            fromscratch = true;
                            continue 'solveagain;
                        } else if scaling > 0 {
                            // Solve again without scaling.
                            log::info!(
                                "(node {}) solution of unbounded LP {} not optimal (pfeas={}, rfeas={}) -- solving without scaling",
                                scip.stat.nnodes,
                                scip.stat.nlps,
                                primalfeasible as u32,
                                rayfeasible as u32
                            );
                            scaling = 0;
                            continue 'solveagain;
                        } else {
                            // Nothing helped: forget about the LP at this node
                            // and mark it to be unsolved.
                            lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("unresolved, LP unbounded"));
                            let lp = &mut lp!(scip);
                            lp.solved = false;
                            lp.lpsolstat = LpSolStat::NotSolved;
                            lperror = true;
                        }
                    }
                }
                LpSolStat::ObjLimit => {
                    // Some LP solvers do not apply the final pivot to reach
                    // the dual solution exceeding the objective limit; in
                    // branch-and-price we must make sure that a dual
                    // feasible solution exists that exceeds it, so continue
                    // with one steepest edge dual iteration without the
                    // limit, then without FASTMIP.
                    if !crate::prob::prob_all_cols_in_lp(scip, prob) {
                        let mut objval = lpi::lpi_get_objval(&mut lpi!(scip))?;
                        if scip.set.is_lt(objval, lp!(scip).lpiobjlim) {
                            fromscratch = false;

                            // Temporarily disable cutoffbound, which also
                            // disables the objective limit.
                            let tmpcutoff = lp!(scip).cutoffbound;
                            lp!(scip).cutoffbound = lpi::lpi_infinity(&lpi!(scip));

                            // Set lp pricing strategy to steepest edge.
                            let tmppricingchar = crate::paramset::paramset_get_char(scip, "lp/pricing")?;
                            crate::paramset::paramset_set_char(scip, "lp/pricing", b's')?;

                            // Resolve LP with an iteration limit of 1.
                            lperror = lp_solve(
                                scip,
                                prob,
                                LpAlgo::DualSimplex,
                                1,
                                1,
                                false,
                                false,
                                true,
                                fastmip,
                                tightprimfeastol,
                                tightdualfeastol,
                                fromscratch,
                                scaling,
                                keepsol,
                            )?;

                            // Reinstall old cutoff bound and lp pricing
                            // strategy.
                            lp!(scip).cutoffbound = tmpcutoff;
                            crate::paramset::paramset_set_char(scip, "lp/pricing", tmppricingchar)?;

                            // Get objective value.
                            objval = lpi::lpi_get_objval(&mut lpi!(scip))?;

                            // Get solution status for the lp.
                            let mut solstat = lp_get_solstat(scip);

                            // The solution is still not exceeding the
                            // objective limit and the solving process was
                            // stopped due to time or iteration limit, solve
                            // again with fastmip turned off.
                            if solstat == LpSolStat::IterLimit {
                                let cutoff = lp!(scip).cutoffbound - get_finite_loose_objval(scip, prob);
                                if scip.set.is_lt(objval, cutoff) {
                                    let simplex = is_simplex(lp!(scip).lastlpalgo);
                                    if !lperror && fastmip > 0 && simplex {
                                        fastmip = 0;
                                        lperror = lp_solve(
                                            scip,
                                            prob,
                                            LpAlgo::DualSimplex,
                                            -1,
                                            -1,
                                            false,
                                            false,
                                            true,
                                            fastmip,
                                            tightprimfeastol,
                                            tightdualfeastol,
                                            fromscratch,
                                            scaling,
                                            keepsol,
                                        )?;

                                        // Get objective value.
                                        objval = lpi::lpi_get_objval(&mut lpi!(scip))?;

                                        // Get solution status for the lp.
                                        solstat = lp_get_solstat(scip);
                                    }
                                }
                            }

                            // Check for lp errors.
                            if lperror || solstat == LpSolStat::Error || solstat == LpSolStat::NotSolved {
                                let lp = &mut lp!(scip);
                                lp.solved = false;
                                lp.lpsolstat = LpSolStat::NotSolved;
                                if !lperror {
                                    retcode = Err(Retcode::LpError);
                                }
                                break 'solveagain;
                            }
                            lp!(scip).solved = true;

                            // Optimal solution / objective limit / iteration
                            // limit or time limit with cutoff.
                            let cutoffge = |scip: &mut Scip, objval: f64| {
                                let cutoff = lp!(scip).cutoffbound - get_finite_loose_objval(scip, prob);
                                scip.set.is_ge(objval, cutoff)
                            };
                            if solstat == LpSolStat::Optimal
                                || solstat == LpSolStat::ObjLimit
                                || ((solstat == LpSolStat::IterLimit || solstat == LpSolStat::TimeLimit) && cutoffge(scip, objval))
                            {
                                // Get LP solution and possibly check the
                                // solution's feasibility again.
                                let checkprim = scip.set.lp_checkprimfeas;
                                let checkdual = scip.set.lp_checkdualfeas;
                                lp!(scip).primalchecked = checkprim;
                                lp!(scip).dualchecked = checkdual;
                                let (pf, df) = lp_get_sol(scip, checkprim, checkdual)?;
                                primalfeasible = if checkprim { pf } else { true };
                                dualfeasible = if checkdual { df } else { true };

                                // If objective value is larger than the
                                // cutoff bound, set solution status to
                                // objective limit reached and objective
                                // value to infinity.
                                if cutoffge(scip, objval) {
                                    let inf = scip.set.infinity();
                                    let lp = &mut lp!(scip);
                                    lp.lpsolstat = LpSolStat::ObjLimit;
                                    lp.lpobjval = inf;
                                }

                                // LP solution is not feasible or objective
                                // limit was reached without the LP value
                                // really exceeding the cutoffbound; mark the
                                // LP to be unsolved.
                                if !primalfeasible || !dualfeasible || (solstat == LpSolStat::ObjLimit && !cutoffge(scip, objval)) {
                                    lp_numerical_trouble_message(scip, VERBLEVEL_HIGH, format_args!("unresolved"));
                                    let lp = &mut lp!(scip);
                                    lp.solved = false;
                                    lp.lpsolstat = LpSolStat::NotSolved;
                                    lperror = true;
                                }
                            }
                            // Infeasible solution.
                            else if solstat == LpSolStat::Infeasible {
                                if scip.set.lp_checkfarkas || !crate::prob::prob_all_cols_in_lp(scip, prob) {
                                    if lpi::lpi_has_dual_ray(&lpi!(scip)) {
                                        farkasvalid = lp_get_dualfarkas(scip, forcedlpsolve, true)?;
                                    }
                                    // It might happen that we have no
                                    // infeasibility proof for the current LP -
                                    // treat this case like an LP error.
                                    else {
                                        log::info!("(node {}) infeasibility of LP {} could not be proven by dual ray", scip.stat.nnodes, scip.stat.nlps);
                                        let lp = &mut lp!(scip);
                                        lp.solved = false;
                                        lp.lpsolstat = LpSolStat::NotSolved;
                                        farkasvalid = false;
                                        lperror = true;
                                    }
                                } else {
                                    farkasvalid = true;
                                }
                                if !farkasvalid {
                                    let simplex = is_simplex(lp!(scip).lastlpalgo);
                                    if !tightprimfeastol {
                                        // Solve again with tighter
                                        // feasibility tolerance.
                                        log::info!(
                                            "(node {}) proof of infeasible LP {} not valid -- solving again with tighter primal feasibility tolerance",
                                            scip.stat.nnodes,
                                            scip.stat.nlps
                                        );
                                        tightprimfeastol = true;
                                        continue 'solveagain;
                                    } else if simplex {
                                        // Solve again from scratch.
                                        log::info!(
                                            "(node {}) proof of infeasible LP {} not valid -- solving again from scratch",
                                            scip.stat.nnodes,
                                            scip.stat.nlps
                                        );
                                        fromscratch = true;
                                        continue 'solveagain;
                                    } else {
                                        // Nothing helped: forget about the LP
                                        // at this node and mark it to be
                                        // unsolved.
                                        lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("unresolved, LP infeasible"));
                                        let lp = &mut lp!(scip);
                                        lp.solved = false;
                                        lp.lpsolstat = LpSolStat::NotSolved;
                                        lperror = true;
                                    }
                                }
                            }
                            // Unbounded solution.
                            else if solstat == LpSolStat::UnboundedRay {
                                if scip.set.lp_checkprimfeas {
                                    (primalfeasible, rayfeasible) = lp_get_unbounded_sol(scip, true)?;
                                    lp!(scip).primalchecked = true;
                                } else {
                                    lp_get_unbounded_sol(scip, false)?;
                                    primalfeasible = true;
                                    rayfeasible = true;
                                    lp!(scip).primalchecked = false;
                                }
                                if !primalfeasible || !rayfeasible {
                                    // Unbounded solution is infeasible: forget
                                    // about the LP at this node and mark it to
                                    // be unsolved.
                                    lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("unresolved, unbounded LP"));
                                    let lp = &mut lp!(scip);
                                    lp.solved = false;
                                    lp.lpsolstat = LpSolStat::NotSolved;
                                    lperror = true;
                                }
                            }
                        } else {
                            lp_get_sol(scip, false, false)?;
                        }
                    }
                }
                LpSolStat::IterLimit => {}
                LpSolStat::TimeLimit => {
                    // Make sure that we evaluate the time limit exactly in
                    // order to avoid erroneous warning.
                    scip.stat.nclockskipsleft = 0;
                    if !scip.stat.userinterrupt && !crate::solve::solve_is_stopped(scip, false) {
                        log::warn!("LP solver reached time limit, but SCIP time limit is not exceeded yet; you might consider switching the clock type of SCIP");
                        scip.stat.status = crate::types::Status::TimeLimit;
                    }
                }
                LpSolStat::Error | LpSolStat::NotSolved => {
                    log::error!("error in LP solver");
                    retcode = Err(Retcode::LpError);
                }
            }
            break 'solveagain;
        }
    }

    // If the LP had to be solved from scratch, we have to reset this flag
    // since it is stored in the LPI; otherwise it may happen that we
    // continue to solve from scratch during strong branching.
    if lp!(scip).lpifromscratch {
        let _ = lp_set_fromscratch(scip, false);
    }
    retcode.map(|()| lperror)
}

/// `SCIPlpGetSolstat`: gets solution status of current LP.
pub fn lp_get_solstat(scip: &Scip) -> LpSolStat {
    let lp = scip.lp.as_ref().unwrap();
    if lp.flushed { lp.lpsolstat } else { LpSolStat::NotSolved }
}

/// `SCIPlpGetObjval`: gets objective value of current LP; for an LP with
/// unbounded loose variables, -infinity.
pub fn lp_get_objval(scip: &mut Scip, prob: usize) -> f64 {
    let lp = &lp!(scip);
    if !lp.flushed {
        INVALID
    } else if scip.set.is_infinity(lp.lpobjval) || scip.set.is_infinity(-lp.lpobjval) {
        lp.lpobjval
    } else if lp.looseobjvalinf > 0 {
        -scip.set.infinity()
    } else {
        // Recalculate the loose objective value, if needed.
        if !lp.looseobjvalid {
            recompute_loose_objective_value(scip, prob);
        }
        let lp = &lp!(scip);
        lp.lpobjval + lp.looseobjval
    }
}

/// `SCIPlpGetColumnObjval`: gets part of objective value of current LP
/// that results from COLUMN variables only.
pub fn lp_get_column_objval(scip: &Scip) -> f64 {
    let lp = scip.lp.as_ref().unwrap();
    if lp.flushed { lp.lpobjval } else { INVALID }
}

/// `SCIPlpGetLooseObjval`: gets part of objective value of current LP that
/// results from LOOSE variables only.
pub fn lp_get_loose_objval(scip: &mut Scip, prob: usize) -> f64 {
    if !lp!(scip).flushed {
        INVALID
    } else if lp!(scip).looseobjvalinf > 0 {
        -scip.set.infinity()
    } else {
        get_finite_loose_objval(scip, prob)
    }
}

/// `SCIPlpStoreRootObjval`: remembers the current LP objective value as
/// root solution value.
pub fn lp_store_root_objval(scip: &mut Scip, prob: usize) {
    let col = lp_get_column_objval(scip);
    let loose = lp_get_loose_objval(scip, prob);
    let lp = &mut lp!(scip);
    lp.rootlpobjval = col;
    lp.rootlooseobjval = loose;
}

/// `SCIPlpInvalidateRootObjval`: invalidates the root LP solution value.
pub fn lp_invalidate_root_objval(scip: &mut Scip) {
    let lp = &mut lp!(scip);
    lp.rootlpobjval = INVALID;
    lp.rootlooseobjval = INVALID;
}

/// `SCIPlpRecomputeLocalAndGlobalPseudoObjval`: recomputes local and
/// global pseudo objective values.
pub fn lp_recompute_local_and_global_pseudo_objval(scip: &mut Scip, prob: usize) {
    let set = &scip.set;
    let lp = &mut **scip.lp.as_mut().unwrap();
    lp.glbpseudoobjvalinf = 0;
    lp.glbpseudoobjval = 0.0;
    lp.pseudoobjvalinf = 0;
    lp.pseudoobjval = 0.0;
    let p = &scip.probs[prob];
    for v in 0..p.nvars {
        let x = &scip.vars[p.vars[v]];
        let obj = x.obj;

        // Update the pseudo objective value.
        if set.is_positive(obj) {
            // Update the global pseudo objective value.
            if set.is_infinity(-x.glbdom.lb) {
                lp.glbpseudoobjvalinf += 1;
            } else {
                lp.glbpseudoobjval += obj * x.glbdom.lb;
            }
            if set.is_infinity(-x.locdom.lb) {
                lp.pseudoobjvalinf += 1;
            } else {
                lp.pseudoobjval += obj * x.locdom.lb;
            }
        }
        if set.is_negative(obj) {
            // Update the global pseudo objective value.
            if set.is_infinity(x.glbdom.ub) {
                lp.glbpseudoobjvalinf += 1;
            } else {
                lp.glbpseudoobjval += obj * x.glbdom.ub;
            }
            if set.is_infinity(x.locdom.ub) {
                lp.pseudoobjvalinf += 1;
            } else {
                lp.pseudoobjval += obj * x.locdom.ub;
            }
        }
    }

    // The recomputed values are reliable.
    lp.relglbpseudoobjval = lp.glbpseudoobjval;
    lp.glbpseudoobjvalid = true;
    lp.relpseudoobjval = lp.pseudoobjval;
    lp.pseudoobjvalid = true;
}

/// `SCIPlpGetGlobalPseudoObjval`: gets the global pseudo objective value;
/// that is all variables set to their best (w.r.t. the objective function)
/// global bound.
pub fn lp_get_global_pseudo_objval(scip: &mut Scip, prob: usize) -> f64 {
    let inf = scip.set.infinity();
    if lp!(scip).glbpseudoobjvalinf > 0 || scip.set.nactivepricers > 0 {
        return -inf;
    }

    // Recalculate the global pseudo solution value, if needed.
    if !lp!(scip).glbpseudoobjvalid {
        recompute_glb_pseudo_objective_value(scip, prob);
    }

    // If the global pseudo objective value is smaller than -infinity, we
    // just return -infinity.
    let v = lp!(scip).glbpseudoobjval;
    if scip.set.is_infinity(-v) {
        return -inf;
    }
    if scip.set.is_infinity(v) {
        return inf;
    }
    v
}

/// `SCIPlpGetPseudoObjval`: gets the pseudo objective value for the
/// current search node; that is all variables set to their best (w.r.t.
/// the objective function) local bound.
pub fn lp_get_pseudo_objval(scip: &mut Scip, prob: usize) -> f64 {
    let inf = scip.set.infinity();
    if lp!(scip).pseudoobjvalinf > 0 || scip.set.nactivepricers > 0 {
        return -inf;
    }

    // Recalculate the pseudo solution value, if needed.
    if !lp!(scip).pseudoobjvalid {
        recompute_pseudo_objective_value(scip, prob);
    }

    // If the pseudo objective value is smaller than -infinity, we just
    // return -infinity.
    let v = lp!(scip).pseudoobjval;
    if scip.set.is_infinity(-v) {
        return -inf;
    }
    if scip.set.is_infinity(v) {
        return inf;
    }
    v
}

/// `SCIPlpGetModifiedPseudoObjval`: gets pseudo objective value, if a
/// bound of the given variable would be modified in the given way.
pub fn lp_get_modified_pseudo_objval(scip: &mut Scip, prob: usize, var: VarId, oldbound: f64, newbound: f64, boundtype: crate::types::BoundType) -> f64 {
    let mut pseudoobjval = get_finite_pseudo_objval(scip, prob);
    let mut pseudoobjvalinf = lp!(scip).pseudoobjvalinf;
    let obj = scip.vars[var].obj;
    if !scip.set.is_zero(obj) && boundtype == crate::var::var_get_best_bound_type(scip, var) {
        if scip.set.is_infinity(oldbound.abs()) {
            pseudoobjvalinf -= 1;
        } else {
            pseudoobjval -= oldbound * obj;
        }
        if scip.set.is_infinity(newbound.abs()) {
            pseudoobjvalinf += 1;
        } else {
            pseudoobjval += newbound * obj;
        }
    }
    if pseudoobjvalinf > 0 || scip.set.nactivepricers > 0 {
        -scip.set.infinity()
    } else {
        pseudoobjval
    }
}

/// `getObjvalDeltaObj`: compute the objective delta due the new objective
/// coefficient; returns (deltaval, deltainf).
fn get_objval_delta_obj(set: &crate::set::Set, oldobj: f64, newobj: f64, lb: f64, ub: f64) -> (f64, i32) {
    let mut deltaval = 0.0;
    let mut deltainf = 0;
    if set.is_positive(oldobj) {
        // Sign of objective did not change.
        if set.is_positive(newobj) {
            // If the bound is finite, calculate the deltaval.
            if !set.is_infinity(-lb) {
                deltaval = lb * (newobj - oldobj);
            }
        }
        // Sign of objective did change, so the best bound does change.
        else if set.is_negative(newobj) {
            if set.is_infinity(-lb) {
                // Old best bound was infinite while new one is not.
                if !set.is_infinity(ub) {
                    deltainf = -1;
                    deltaval = ub * newobj;
                }
            } else {
                // New best bound is infinite while old one was not.
                if set.is_infinity(ub) {
                    deltainf = 1;
                    deltaval = -lb * oldobj;
                }
                // Neither the new best bound nor the old one is infinite.
                else {
                    deltaval = (ub * newobj) - (lb * oldobj);
                }
            }
        }
        // New objective is 0.0.
        else if set.is_infinity(-lb) {
            deltainf = -1;
        } else {
            deltaval = -lb * oldobj;
        }
    } else if set.is_negative(oldobj) {
        // Sign of objective did not change.
        if set.is_negative(newobj) {
            // If the bound is finite, calculate the deltaval.
            if !set.is_infinity(ub) {
                deltaval = ub * (newobj - oldobj);
            }
        }
        // Sign of objective did change, so the best bound does change.
        else if set.is_positive(newobj) {
            if set.is_infinity(ub) {
                // Old best bound was infinite while new one is not.
                if !set.is_infinity(-lb) {
                    deltainf = -1;
                    deltaval = lb * newobj;
                }
            } else {
                // New best bound is infinite while old one was not.
                if set.is_infinity(-lb) {
                    deltainf = 1;
                    deltaval = -ub * oldobj;
                }
                // Neither the new best bound nor the old one is infinite.
                else {
                    deltaval = (lb * newobj) - (ub * oldobj);
                }
            }
        }
        // New objective is 0.0.
        else if set.is_infinity(ub) {
            deltainf = -1;
        } else {
            deltaval = -ub * oldobj;
        }
    }
    // Old objective was 0.0.
    else if set.is_negative(newobj) {
        if set.is_infinity(ub) {
            deltainf = 1;
        } else {
            deltaval = ub * newobj;
        }
    } else if set.is_positive(newobj) {
        if set.is_infinity(-lb) {
            deltainf = 1;
        } else {
            deltaval = lb * newobj;
        }
    }
    (deltaval, deltainf)
}

/// `getObjvalDeltaLb`: compute the objective delta due the new lower
/// bound; returns (deltaval, deltainf).
fn get_objval_delta_lb(set: &crate::set::Set, obj: f64, oldlb: f64, newlb: f64) -> (f64, i32) {
    if set.is_infinity(-newlb) {
        (-obj * oldlb, 1)
    } else if set.is_infinity(-oldlb) {
        (obj * newlb, -1)
    } else {
        (obj * (newlb - oldlb), 0)
    }
}

/// `getObjvalDeltaUb`: compute the objective delta due the new upper
/// bound; returns (deltaval, deltainf).
fn get_objval_delta_ub(set: &crate::set::Set, obj: f64, oldub: f64, newub: f64) -> (f64, i32) {
    if set.is_infinity(newub) {
        (-obj * oldub, 1)
    } else if set.is_infinity(oldub) {
        (obj * newub, -1)
    } else {
        (obj * (newub - oldub), 0)
    }
}

/// `lpUpdateObjval`: updates current pseudo and loose objective values
/// for a change in a variable's objective value or bounds.
fn lp_update_objval(scip: &mut Scip, var: VarId, deltaval: f64, deltainf: i32, local: bool, mut loose: bool, global: bool) {
    let set = &scip.set;
    let lp = &mut **scip.lp.as_mut().unwrap();
    if local {
        // Update the pseudo objective value.
        lp.pseudoobjvalinf += deltainf;
        if lp.pseudoobjvalid {
            lp.pseudoobjval += deltaval;

            // If the absolute value was increased, this is regarded as
            // reliable, otherwise, we check whether we can still trust the
            // updated value.
            if lp.relpseudoobjval.abs() < lp.pseudoobjval.abs() {
                lp.relpseudoobjval = lp.pseudoobjval;
            } else if set.is_update_unreliable(lp.pseudoobjval, lp.relpseudoobjval) {
                lp.pseudoobjvalid = false;
            }
        }

        // After changing a local bound on a LOOSE variable, we have to
        // update the loose objective value, too.
        if scip.vars[var].varstatus == VarStatus::Loose {
            loose = true;
        }
    }
    if loose {
        // Update the loose objective value.
        lp.looseobjvalinf += deltainf;
        if deltaval != 0.0 && lp.looseobjvalid {
            lp.looseobjval += deltaval;

            // If the absolute value was increased, this is regarded as
            // reliable, otherwise, we check whether we can still trust the
            // updated value.
            if lp.rellooseobjval.abs() < lp.looseobjval.abs() {
                lp.rellooseobjval = lp.looseobjval;
            } else if set.is_update_unreliable(lp.looseobjval, lp.rellooseobjval) {
                lp.looseobjvalid = false;
            }
        }
    }
    if global {
        // Update the global pseudo objective value.
        lp.glbpseudoobjvalinf += deltainf;
        if lp.glbpseudoobjvalid {
            lp.glbpseudoobjval += deltaval;

            // If the absolute value was increased, this is regarded as
            // reliable, otherwise, we check whether we can still trust the
            // updated value.
            if lp.relglbpseudoobjval.abs() < lp.glbpseudoobjval.abs() {
                lp.relglbpseudoobjval = lp.glbpseudoobjval;
            } else if set.is_update_unreliable(lp.glbpseudoobjval, lp.relglbpseudoobjval) {
                lp.glbpseudoobjvalid = false;
            }
        }
    }
}

/// `SCIPlpUpdateVarObj`: updates current pseudo and loose objective value
/// for a change in a variable's objective coefficient.
pub fn lp_update_var_obj(scip: &mut Scip, var: VarId, oldobj: f64, newobj: f64) -> ScipResult {
    if !scip.set.is_eq(oldobj, newobj) {
        // The objective coefficient can only be changed during presolving,
        // that implies that the global and local domain of the variable are
        // the same.
        let x = &scip.vars[var];
        let (lloc, uloc, lglb, uglb) = (x.locdom.lb, x.locdom.ub, x.glbdom.lb, x.glbdom.ub);

        // Update the pseudo objective value.
        let (deltaval, deltainf) = get_objval_delta_obj(&scip.set, oldobj, newobj, lloc, uloc);
        lp_update_objval(scip, var, deltaval, deltainf, true, false, false);

        // Update the global pseudo objective value.
        let (deltaval, deltainf) = get_objval_delta_obj(&scip.set, oldobj, newobj, lglb, uglb);
        lp_update_objval(scip, var, deltaval, deltainf, false, false, true);
    }
    Ok(())
}

/// `SCIPlpUpdateVarLbGlobal`: updates current root pseudo objective value
/// for a global change in a variable's lower bound.
pub fn lp_update_var_lb_global(scip: &mut Scip, var: VarId, oldlb: f64, newlb: f64) -> ScipResult {
    let obj = scip.vars[var].obj;
    if !scip.set.is_eq(oldlb, newlb) && scip.set.is_positive(obj) {
        let (deltaval, deltainf) = get_objval_delta_lb(&scip.set, obj, oldlb, newlb);
        lp_update_objval(scip, var, deltaval, deltainf, false, false, true);
    }
    Ok(())
}

/// `SCIPlpUpdateVarLb`: updates current pseudo and loose objective value
/// for a change in a variable's lower bound.
pub fn lp_update_var_lb(scip: &mut Scip, var: VarId, oldlb: f64, newlb: f64) -> ScipResult {
    let obj = scip.vars[var].obj;
    if !scip.set.is_eq(oldlb, newlb) && scip.set.is_positive(obj) {
        let (deltaval, deltainf) = get_objval_delta_lb(&scip.set, obj, oldlb, newlb);
        lp_update_objval(scip, var, deltaval, deltainf, true, false, false);
    }
    Ok(())
}

/// `SCIPlpUpdateVarUbGlobal`: updates current root pseudo objective value
/// for a global change in a variable's upper bound.
pub fn lp_update_var_ub_global(scip: &mut Scip, var: VarId, oldub: f64, newub: f64) -> ScipResult {
    let obj = scip.vars[var].obj;
    if !scip.set.is_eq(oldub, newub) && scip.set.is_negative(obj) {
        let (deltaval, deltainf) = get_objval_delta_ub(&scip.set, obj, oldub, newub);
        lp_update_objval(scip, var, deltaval, deltainf, false, false, true);
    }
    Ok(())
}

/// `SCIPlpUpdateVarUb`: updates current pseudo objective value for a
/// change in a variable's upper bound.
pub fn lp_update_var_ub(scip: &mut Scip, var: VarId, oldub: f64, newub: f64) -> ScipResult {
    let obj = scip.vars[var].obj;
    if !scip.set.is_eq(oldub, newub) && scip.set.is_negative(obj) {
        let (deltaval, deltainf) = get_objval_delta_ub(&scip.set, obj, oldub, newub);
        lp_update_objval(scip, var, deltaval, deltainf, true, false, false);
    }
    Ok(())
}

/// `SCIPlpUpdateAddVar`: informs LP, that given variable was added to the
/// problem.
pub fn lp_update_add_var(scip: &mut Scip, var: VarId) -> ScipResult {
    // Add the variable to the objective value sums.
    let obj = scip.vars[var].obj;
    lp_update_var_obj(scip, var, 0.0, obj)?;

    // Update the loose variables counter.
    if scip.vars[var].varstatus == VarStatus::Loose {
        lp!(scip).nloosevars += 1;
    }
    Ok(())
}

/// `SCIPlpUpdateDelVar`: informs LP, that given variable is to be deleted
/// from the problem.
pub fn lp_update_del_var(scip: &mut Scip, var: VarId) -> ScipResult {
    // Subtract the variable from the objective value sums.
    let obj = scip.vars[var].obj;
    lp_update_var_obj(scip, var, obj, 0.0)?;

    // Update the loose variables counter.
    if scip.vars[var].varstatus == VarStatus::Loose {
        lp_dec_nloosevars(scip);
    }
    Ok(())
}

/// `SCIPlpUpdateVarColumn`: informs LP, that given formerly loose
/// problem variable is now a column variable.
pub fn lp_update_var_column(scip: &mut Scip, var: VarId) -> ScipResult {
    let x = &scip.vars[var];
    let obj = x.obj;
    let (lb, ub) = (x.locdom.lb, x.locdom.ub);

    // Update loose objective value.
    if scip.set.is_positive(obj) {
        if scip.set.is_infinity(-lb) {
            lp!(scip).looseobjvalinf -= 1;
        } else {
            lp_update_objval(scip, var, -lb * obj, 0, false, true, false);
        }
    } else if scip.set.is_negative(obj) {
        if scip.set.is_infinity(ub) {
            lp!(scip).looseobjvalinf -= 1;
        } else {
            lp_update_objval(scip, var, -ub * obj, 0, false, true, false);
        }
    }
    lp_dec_nloosevars(scip);
    Ok(())
}

/// `SCIPlpUpdateVarLoose`: informs LP, that given formerly column problem
/// variable is now again a loose variable.
pub fn lp_update_var_loose(scip: &mut Scip, var: VarId) -> ScipResult {
    let x = &scip.vars[var];
    let obj = x.obj;
    let (lb, ub) = (x.locdom.lb, x.locdom.ub);

    // Update loose objective value corresponding to the addition of
    // variable.
    if scip.set.is_positive(obj) {
        if scip.set.is_infinity(-lb) {
            lp!(scip).looseobjvalinf += 1;
        } else {
            lp_update_objval(scip, var, lb * obj, 0, false, true, false);
        }
    } else if scip.set.is_negative(obj) {
        if scip.set.is_infinity(ub) {
            lp!(scip).looseobjvalinf += 1;
        } else {
            lp_update_objval(scip, var, ub * obj, 0, false, true, false);
        }
    }
    lp!(scip).nloosevars += 1;
    Ok(())
}

/// `SCIPlpDecNLoosevars`: decrease the number of loose variables by one.
pub fn lp_dec_nloosevars(scip: &mut Scip) {
    let lp = &mut lp!(scip);
    lp.nloosevars -= 1;

    // Get rid of numerical problems: set loose objective value explicitly
    // to zero, if no loose variables remain.
    if lp.nloosevars == 0 {
        lp.looseobjval = 0.0;
    }
}

/// `SCIPlpIsFeasEQ`: checks if two values are equal w.r.t. the LP's
/// feasibility tolerance.
pub fn lp_is_feas_eq(lp: &Lp, val1: f64, val2: f64) -> bool {
    crate::def::eps_eq(val1, val2, lp.feastol)
}

/// `SCIPlpIsFeasLT`.
pub fn lp_is_feas_lt(lp: &Lp, val1: f64, val2: f64) -> bool {
    crate::def::eps_lt(val1, val2, lp.feastol)
}

/// `SCIPlpIsFeasLE`.
pub fn lp_is_feas_le(lp: &Lp, val1: f64, val2: f64) -> bool {
    crate::def::eps_le(val1, val2, lp.feastol)
}

/// `SCIPlpIsFeasGT`.
pub fn lp_is_feas_gt(lp: &Lp, val1: f64, val2: f64) -> bool {
    crate::def::eps_gt(val1, val2, lp.feastol)
}

/// `SCIPlpIsFeasGE`.
pub fn lp_is_feas_ge(lp: &Lp, val1: f64, val2: f64) -> bool {
    crate::def::eps_ge(val1, val2, lp.feastol)
}

/// `SCIPlpIsFeasZero`.
pub fn lp_is_feas_zero(lp: &Lp, val: f64) -> bool {
    crate::def::eps_z(val, lp.feastol)
}

/// `SCIPlpIsFeasPositive`.
pub fn lp_is_feas_positive(lp: &Lp, val: f64) -> bool {
    crate::def::eps_p(val, lp.feastol)
}

/// `SCIPlpIsFeasNegative`.
pub fn lp_is_feas_negative(lp: &Lp, val: f64) -> bool {
    crate::def::eps_n(val, lp.feastol)
}

/// `SCIPlpGetSol`: stores the LP solution in the columns and rows data
/// structure. `checkprimal`/`checkdual` stand for C's non-NULL feasibility
/// pointers; returns (primalfeasible, dualfeasible), meaningful for the
/// checked ones.
pub fn lp_get_sol(scip: &mut Scip, checkprimal: bool, checkdual: bool) -> ScipResult<(bool, bool)> {
    // Initialize return and feasibility flags; if primal or dual
    // feasibility shall not be checked, we set the corresponding flag
    // immediately to FALSE to skip all checks.
    let mut stillprimalfeasible = checkprimal;
    let mut stilldualfeasible = checkdual;

    // Check if the values are already calculated.
    if lp!(scip).validsollp == scip.stat.lpcount {
        return Ok((true, true));
    }
    lp!(scip).validsollp = scip.stat.lpcount;
    let nlpicols = lp!(scip).nlpicols as usize;
    let nlpirows = lp!(scip).nlpirows as usize;
    let lpcount = scip.stat.lpcount;

    // Get temporary memory.
    let mut primsol = vec![0.0; nlpicols];
    let mut dualsol = vec![0.0; nlpirows];
    let mut redcost = vec![0.0; nlpicols];
    let mut cstat = vec![0; nlpicols];
    let mut rstat = vec![0; nlpirows];
    lpi::lpi_get_sol(&mut lpi!(scip), None, Some(&mut primsol), Some(&mut dualsol), None, Some(&mut redcost))?;
    if lp!(scip).solisbasic {
        lpi::lpi_get_base(&mut lpi!(scip), Some(&mut cstat), Some(&mut rstat))?;
    }

    // Copy primal solution and reduced costs into columns.
    let mut primalbound = 0.0;
    let mut dualbound = 0.0;
    {
        let set = &scip.set;
        let lp = &**scip.lp.as_ref().unwrap();
        let barrier = lp.lastlpalgo == LpAlgo::Barrier;
        for c in 0..nlpicols {
            let col = &mut scip.cols[lp.lpicols[c as i32]];
            col.primsol = primsol[c];
            if !col.primsol.is_finite() {
                // Calculating with nan or +/-inf can have many unexpected
                // effects; thus change the solution here to a reasonable
                // value (0.0) and declare it as neither primal nor dual
                // feasible; this should trigger a resolve of the LP, or a
                // stop with an LP error.
                stillprimalfeasible = false;
                stilldualfeasible = false;
                col.primsol = 0.0;
            }
            col.minprimsol = crate::def::min(col.minprimsol, primsol[c]);
            col.maxprimsol = crate::def::max(col.maxprimsol, primsol[c]);
            col.redcost = redcost[c];
            col.basisstatus = cstat[c] as u32;
            col.validredcostlp = lpcount;
            if stillprimalfeasible {
                stillprimalfeasible = (set.is_infinity(-col.lb) || lp_is_feas_ge(lp, col.primsol, col.lb)) && (set.is_infinity(col.ub) || lp_is_feas_le(lp, col.primsol, col.ub));
                primalbound += col.primsol * col.obj;
            }
            if barrier {
                // Complementary slackness in barrier solutions is measured
                // as product of primal slack and dual multiplier; we use a
                // slack of at most 1.
                if stilldualfeasible {
                    let compslack = crate::def::min(col.primsol - col.lb, 1.0) * col.redcost;
                    stilldualfeasible = !set.is_dualfeas_positive(compslack);
                }
                if stilldualfeasible {
                    let compslack = crate::def::min(col.ub - col.primsol, 1.0) * col.redcost;
                    stilldualfeasible = !set.is_dualfeas_negative(compslack);
                }
            } else {
                // Nonbasic variables at one of their bounds can have reduced
                // costs; basic ones get zero reduced costs when not checked.
                if !checkdual && col.basisstatus == BASESTAT_BASIC {
                    col.redcost = 0.0;
                }

                // Complementary slackness means that if a variable is not at
                // its lower or upper bound, its reduced costs must be
                // non-positive or non-negative, respectively.
                if stilldualfeasible && (set.is_infinity(-col.lb) || lp_is_feas_gt(lp, col.primsol, col.lb)) {
                    stilldualfeasible = !set.is_dualfeas_positive(col.redcost);
                }
                if stilldualfeasible && (set.is_infinity(col.ub) || lp_is_feas_lt(lp, col.primsol, col.ub)) {
                    stilldualfeasible = !set.is_dualfeas_negative(col.redcost);
                }
            }

            // We intentionally use an exact positive/negative check because
            // ignoring small reduced cost values may lead to a wrong bound
            // value.
            if stilldualfeasible {
                if col.redcost > 0.0 && !set.is_infinity(-col.lb) {
                    dualbound += col.redcost * col.lb;
                } else if col.redcost < 0.0 && !set.is_infinity(col.ub) {
                    dualbound += col.redcost * col.ub;
                }
            }
        }
    }

    // Copy dual solution and activities into rows.
    for r in 0..nlpirows {
        let row = lp!(scip).lpirows[r as i32];
        scip.rows[row].dualsol = dualsol[r];
        if scip.rows[row].validactivitylp != scip.stat.lpcount {
            row_recalc_lp_activity(scip, row);
        }
        let set = &scip.set;
        let lp = &**scip.lp.as_ref().unwrap();
        let ro = &mut scip.rows[row];
        ro.basisstatus = rstat[r] as u32;
        ro.validactivitylp = lpcount;
        if stillprimalfeasible {
            stillprimalfeasible = (set.is_infinity(-ro.lhs) || lp_is_feas_ge(lp, ro.activity, ro.lhs)) && (set.is_infinity(ro.rhs) || lp_is_feas_le(lp, ro.activity, ro.rhs));
        }
        if lp.lastlpalgo == LpAlgo::Barrier {
            // Complementary slackness in barrier solutions is measured as
            // product of primal slack and dual multiplier.
            if stilldualfeasible {
                let compslack = crate::def::min(ro.activity - ro.lhs, 1.0) * ro.dualsol;
                stilldualfeasible = !set.is_dualfeas_positive(compslack);
            }
            if stilldualfeasible {
                let compslack = crate::def::min(ro.rhs - ro.activity, 1.0) * ro.dualsol;
                stilldualfeasible = !set.is_dualfeas_negative(compslack);
            }
        } else {
            // Complementary slackness means that if the activity of a row
            // is not at its left-hand or right-hand side, its dual
            // multiplier must be non-positive or non-negative, respectively.
            if stilldualfeasible && (set.is_infinity(-ro.lhs) || lp_is_feas_gt(lp, ro.activity, ro.lhs)) {
                stilldualfeasible = !set.is_dualfeas_positive(ro.dualsol);
            }
            if stilldualfeasible && (set.is_infinity(ro.rhs) || lp_is_feas_lt(lp, ro.activity, ro.rhs)) {
                stilldualfeasible = !set.is_dualfeas_negative(ro.dualsol);
            }
        }

        // We intentionally use an exact positive/negative check because
        // ignoring small dual multipliers may lead to a wrong bound value.
        if stilldualfeasible {
            if ro.dualsol > 0.0 && !set.is_infinity(-ro.lhs) {
                dualbound += ro.dualsol * (ro.lhs - ro.constant);
            } else if ro.dualsol < 0.0 && !set.is_infinity(ro.rhs) {
                dualbound += ro.dualsol * (ro.rhs - ro.constant);
            }
        }
    }

    // If the objective value returned by the LP solver is smaller than the
    // internally computed primal bound, then we declare the solution primal
    // infeasible; we assume primalbound and lp->lpobjval to be equal if
    // they are both +/- infinity.
    let set = &scip.set;
    let lpobjval = lp!(scip).lpobjval;
    if stillprimalfeasible && !(set.is_infinity(primalbound) && set.is_infinity(lpobjval)) && !(set.is_infinity(-primalbound) && set.is_infinity(-lpobjval)) {
        stillprimalfeasible = set.is_feas_le(primalbound, lpobjval);
    }

    // If the objective value returned by the LP solver is smaller than the
    // internally computed dual bound, we declare the solution dual
    // infeasible; we assume dualbound and lp->lpobjval to be equal if they
    // are both +/- infinity.
    if stilldualfeasible && !(set.is_infinity(dualbound) && set.is_infinity(lpobjval)) && !(set.is_infinity(-dualbound) && set.is_infinity(-lpobjval)) {
        stilldualfeasible = set.is_feas_ge(dualbound, lpobjval);
    }
    Ok((stillprimalfeasible, stilldualfeasible))
}

/// `SCIPlpGetUnboundedSol`: stores LP solution with infinite objective
/// value in the columns and rows. `check` stands for C's non-NULL
/// feasibility pointers; returns (primalfeasible, rayfeasible).
pub fn lp_get_unbounded_sol(scip: &mut Scip, check: bool) -> ScipResult<(bool, bool)> {
    let mut primalfeasible = true;
    let mut rayfeasible = true;

    // Check if the values are already calculated.
    if lp!(scip).validsollp == scip.stat.lpcount {
        return Ok((true, true));
    }
    lp!(scip).validsollp = scip.stat.lpcount;

    // Check if the LP solver is able to provide a primal unbounded ray.
    if !lpi::lpi_has_primal_ray(&lpi!(scip)) {
        log::error!("LP solver has no primal ray to prove unboundedness.");
        return Err(Retcode::LpError);
    }
    let nlpicols = lp!(scip).nlpicols as usize;
    let nlpirows = lp!(scip).nlpirows as usize;
    let mut primsol = vec![0.0; nlpicols];
    let mut activity = vec![0.0; nlpirows];
    let mut ray = vec![0.0; nlpicols];

    // Get primal unbounded ray.
    lpi::lpi_get_primal_ray(&mut lpi!(scip), &mut ray)?;
    let lpcount = scip.stat.lpcount;

    // Calculate the objective value decrease of the ray and heuristically
    // try to construct primal solution.
    let mut rayobjval = 0.0;
    {
        let set = &scip.set;
        let lp = &**scip.lp.as_ref().unwrap();
        for c in 0..nlpicols {
            let col = &scip.cols[lp.lpicols[c as i32]];

            // Check if the ray is valid w.r.t. the bounds of the column.
            if check {
                rayfeasible = rayfeasible && (!set.is_negative(ray[c]) || set.is_infinity(-col.lb)) && (!set.is_positive(ray[c]) || set.is_infinity(col.ub));
            }
            if !set.is_zero(ray[c]) {
                rayobjval += ray[c] * col.obj;
            }

            // Many LP solvers cannot directly provide a feasible solution if
            // they detected unboundedness. We therefore first heuristically
            // try to construct a primal solution.
            primsol[c] = 0.0;
            if set.is_feas_zero(ray[c]) {
                // If the ray component is 0, we try to satisfy as many rows
                // as possible.
                if crate::var::var_get_nlocks_down(scip, col.var) == 0 && !set.is_infinity(-col.lb) {
                    primsol[c] = col.lb;
                } else if crate::var::var_get_nlocks_up(scip, col.var) == 0 && !set.is_infinity(col.ub) {
                    primsol[c] = col.ub;
                }
            }

            // Make sure we respect the bounds.
            primsol[c] = crate::def::max(primsol[c], col.lb);
            primsol[c] = crate::def::min(primsol[c], col.ub);
        }
    }

    // Check feasibility of heuristic primal solution.
    let mut r = 0;
    {
        let set = &scip.set;
        let lp = &**scip.lp.as_ref().unwrap();
        while r < nlpirows {
            let row = &scip.rows[lp.lpirows[r as i32]];
            let mut act = row.constant;
            for c in 0..row.nlpcols {
                act += row.vals[c] * primsol[scip.cols[row.cols[c]].lppos as usize];
            }
            if row.nunlinked > 0 {
                for c in row.nlpcols..row.len {
                    let lppos = scip.cols[row.cols[c]].lppos;
                    if lppos >= 0 {
                        act += row.vals[c] * primsol[lppos as usize];
                    }
                }
            }

            // Check feasibility.
            if (!set.is_infinity(-row.lhs) && lp_is_feas_lt(lp, act, row.lhs)) || (!set.is_infinity(row.rhs) && lp_is_feas_gt(lp, act, row.rhs)) {
                break;
            }
            r += 1;
        }
    }

    // Did not find a primal feasible solution.
    if r < nlpirows {
        // Get primal feasible point.
        lpi::lpi_get_sol(&mut lpi!(scip), None, Some(&mut primsol), None, None, None)?;

        // Check bounds of primal solution.
        if check {
            let set = &scip.set;
            let lp = &**scip.lp.as_ref().unwrap();
            for c in 0..nlpicols {
                let col = &scip.cols[lp.lpicols[c as i32]];

                // Check whether primal solution satisfies the bounds; note
                // that we also ensure that the primal solution is within
                // SCIP's infinity bounds.
                if set.is_infinity(primsol[c].abs()) || lp_is_feas_lt(lp, primsol[c], col.lb) || lp_is_feas_gt(lp, primsol[c], col.ub) {
                    primalfeasible = false;
                    break;
                }
            }
        }
    }

    // Compute activity and check feasibility of primal solution and ray.
    {
        let set = &scip.set;
        let lp = &**scip.lp.as_ref().unwrap();
        for r in 0..nlpirows {
            let row = &scip.rows[lp.lpirows[r as i32]];
            let mut primact = row.constant;
            let mut rayact = 0.0;
            for c in 0..row.nlpcols {
                let lppos = scip.cols[row.cols[c]].lppos as usize;
                primact += row.vals[c] * primsol[lppos];
                rayact += row.vals[c] * ray[lppos];
            }
            if row.nunlinked > 0 {
                for c in row.nlpcols..row.len {
                    let lppos = scip.cols[row.cols[c]].lppos;
                    if lppos >= 0 {
                        primact += row.vals[c] * primsol[lppos as usize];
                        rayact += row.vals[c] * ray[lppos as usize];
                    }
                }
            }

            // Check feasibility of primal solution.
            if check
                && primalfeasible
                && ((!set.is_infinity(-row.lhs) && lp_is_feas_lt(lp, primact, row.lhs)) || (!set.is_infinity(row.rhs) && lp_is_feas_gt(lp, primact, row.rhs)))
            {
                primalfeasible = false;
            }

            // Check feasibility of ray.
            if check && rayfeasible && ((!set.is_infinity(-row.lhs) && lp_is_feas_lt(lp, rayact, 0.0)) || (!set.is_infinity(row.rhs) && lp_is_feas_gt(lp, rayact, 0.0))) {
                rayfeasible = false;
            }

            // Store the activity of the primal solution.
            activity[r] = primact;
        }
    }

    // Compute the scale of the ray, such that the resulting solution has
    // infinite objective value.
    let set = &scip.set;
    let rayscale = if check && !primalfeasible {
        0.0
    } else if check && !rayfeasible {
        0.0
    } else if !set.is_negative(rayobjval) {
        // Due to numerical problems, the objective of the ray might be
        // nonnegative.
        if check {
            rayfeasible = false;
        }
        0.0
    } else {
        // Scale the ray, such that the resulting point has infinite
        // objective value; it should not exceed the bounds of the columns.
        let lp = &**scip.lp.as_ref().unwrap();
        let mut rayscale = -2.0 * set.infinity() / rayobjval;
        for c in 0..nlpicols {
            let col = &scip.cols[lp.lpicols[c as i32]];
            if set.is_positive(ray[c]) {
                if !set.is_infinity(primsol[c]) {
                    rayscale = crate::def::min(rayscale, (col.ub - primsol[c]) / ray[c]);
                }
                // If the primal solution is infinite, set ray scale such
                // that the resulting point is within bounds.
                else {
                    rayscale = crate::def::min(rayscale, 1.0 / ray[c]);
                }
            } else if set.is_negative(ray[c]) {
                if !set.is_infinity(-primsol[c]) {
                    rayscale = crate::def::min(rayscale, (col.lb - primsol[c]) / ray[c]);
                }
                // If the primal solution is infinite, set ray scale such
                // that the resulting point is within bounds.
                else {
                    rayscale = crate::def::min(rayscale, -1.0 / ray[c]);
                }
            }
        }
        rayscale
    };

    // Calculate primal solution and update columns: it is not guaranteed
    // to be feasible because of the typically large values in scaling.
    let inf = set.infinity();
    let lp = &**scip.lp.as_ref().unwrap();
    for c in 0..nlpicols {
        let col = &mut scip.cols[lp.lpicols[c as i32]];
        if set.is_zero(ray[c]) {
            col.primsol = primsol[c];
        } else {
            let primsolval = primsol[c] + rayscale * ray[c];
            col.primsol = crate::def::max(-inf, crate::def::min(inf, primsolval));
        }
        col.redcost = INVALID;
        col.validredcostlp = -1;
    }

    // Update rows.
    for r in 0..nlpirows {
        let row = &mut scip.rows[lp.lpirows[r as i32]];
        row.dualsol = INVALID;
        row.activity = activity[r] + row.constant;
        row.validactivitylp = lpcount;
    }
    Ok((primalfeasible, rayfeasible))
}

/// `SCIPlpGetPrimalRay`: returns primal ray proving the unboundedness of
/// the current LP, w.r.t. the problem index of the variables.
pub fn lp_get_primal_ray(scip: &mut Scip, ray: &mut [f64]) -> ScipResult {
    // Check if the LP solver is able to provide a primal unbounded ray.
    if !lpi::lpi_has_primal_ray(&lpi!(scip)) {
        log::error!("LP solver has no primal ray for unbounded LP");
        return Err(Retcode::LpError);
    }
    let nlpicols = lp!(scip).nlpicols as usize;
    let mut lpiray = vec![0.0; nlpicols];

    // Get primal unbounded ray.
    lpi::lpi_get_primal_ray(&mut lpi!(scip), &mut lpiray)?;

    // Store the ray values of active problem variables.
    let lp = &**scip.lp.as_ref().unwrap();
    for c in 0..nlpicols {
        let var = scip.cols[lp.lpicols[c as i32]].var;
        ray[scip.vars[var].probindex as usize] = lpiray[c];
    }
    Ok(())
}

/// `SCIPlpGetDualfarkas`: stores the dual Farkas multipliers for
/// infeasibility proof in rows; besides, the proof is checked for
/// numerical stability if lp/checkfarkas is set and `checkvalid` (C's
/// non-NULL `valid`). Returns whether the Farkas proof is valid.
pub fn lp_get_dualfarkas(scip: &mut Scip, forcedlpsolve: bool, checkvalid: bool) -> ScipResult<bool> {
    let mut valid = true;

    // Check if the values are already calculated.
    if lp!(scip).validfarkaslp == scip.stat.lpcount {
        return Ok(true);
    }
    lp!(scip).validfarkaslp = scip.stat.lpcount;
    let mut maxactivity = 0.0;
    let mut farkaslhs = 0.0;
    let checkfarkas = scip.set.lp_checkfarkas && checkvalid;
    let nlpicols = lp!(scip).nlpicols as usize;
    let nlpirows = lp!(scip).nlpirows as usize;
    let mut dualfarkas = vec![0.0; nlpirows];
    let mut farkascoefs = if checkfarkas { vec![0.0; nlpicols] } else { Vec::new() };

    // Get dual Farkas infeasibility proof.
    lpi::lpi_get_dualfarkas(&mut lpi!(scip), &mut dualfarkas)?;
    let set = &scip.set;
    let lp = &**scip.lp.as_ref().unwrap();

    // Store infeasibility proof in rows.
    'terminate: {
        for r in 0..nlpirows {
            let row = &mut scip.rows[lp.lpirows[r as i32]];
            row.dualfarkas = dualfarkas[r];
            row.dualsol = INVALID;
            row.activity = 0.0;
            row.validactivitylp = -1;
            row.basisstatus = BASESTAT_BASIC;
            if checkfarkas && dualfarkas[r] != 0.0 {
                // The infeasibility proof would be invalid if (i)
                // dualfarkas[r] > 0 and lhs = -inf, (ii) dualfarkas[r] < 0 and
                // rhs = inf; however, due to numerics we accept slightly
                // negative / positive values.
                if (set.is_dualfeas_gt(dualfarkas[r], 0.0) && set.is_infinity(-row.lhs)) || (set.is_dualfeas_lt(dualfarkas[r], 0.0) && set.is_infinity(row.rhs)) {
                    valid = false;
                    break 'terminate;
                }

                // Dual multipliers, for which the corresponding row side in
                // infinite, are treated as zero if they are zero within
                // tolerances (see above) but slightly positive / negative.
                if (dualfarkas[r] > 0.0 && set.is_infinity(-row.lhs)) || (dualfarkas[r] < 0.0 && set.is_infinity(row.rhs)) {
                    continue;
                }

                // Iterate over all columns and scale with dual solution.
                for c in 0..row.len {
                    let pos = scip.cols[row.cols[c]].lppos;
                    if pos == -1 {
                        continue;
                    }
                    farkascoefs[pos as usize] += dualfarkas[r] * row.vals[c];
                }

                // The row contributes with its left-hand side to the proof.
                if dualfarkas[r] > 0.0 {
                    farkaslhs += dualfarkas[r] * row.lhs;
                }
                // The row contributes with its right-hand side to the proof.
                else if dualfarkas[r] < 0.0 {
                    farkaslhs += dualfarkas[r] * row.rhs;
                }
                maxactivity += dualfarkas[r] * row.constant;
            }
        }

        // Set columns as invalid.
        for c in 0..nlpicols {
            let col = &mut scip.cols[lp.lpicols[c as i32]];
            col.primsol = INVALID;
            col.redcost = INVALID;
            col.validredcostlp = -1;
            col.validfarkaslp = -1;
            if checkfarkas {
                if set.is_dualfeas_zero(farkascoefs[c]) {
                    continue;
                }

                // The multiplier is positive: the upper bound counts.
                if farkascoefs[c] > 0.0 {
                    maxactivity += farkascoefs[c] * col.ub;
                    if set.is_infinity(col.ub) {
                        valid = false;
                        break 'terminate;
                    }
                }
                // The multiplier is negative: the lower bound counts.
                else {
                    maxactivity += farkascoefs[c] * col.lb;
                    if set.is_infinity(-col.lb) {
                        valid = false;
                        break 'terminate;
                    }
                }
            }
        }

        // Check whether the farkasproof is valid for relative epsilon
        // tolerance and the maxactivity is not infinite.
        if checkfarkas && (set.is_infinity(maxactivity) || set.is_infinity(-farkaslhs) || set.is_rel_ge(maxactivity, farkaslhs)) {
            if forcedlpsolve && set.is_lt(maxactivity, farkaslhs) {
                log::warn!("Unreliable farkas proof forced valid, result might not be optimal.");
            } else {
                valid = false;
            }
        }
    }
    Ok(valid)
}

/// `SCIPlpGetIterations`: get number of iterations used in last LP solve.
pub fn lp_get_iterations(scip: &mut Scip) -> ScipResult<i32> {
    lpi::lpi_get_iterations(&mut lpi!(scip))
}

/// `SCIPlpUpdateAges`: increases age of columns with solution value 0.0
/// and basic rows with activity not at its bounds, resets age of
/// non-zero columns and sharp rows.
pub fn lp_update_ages(scip: &mut Scip) -> ScipResult {
    let lp = &**scip.lp.as_ref().unwrap();
    for c in 0..lp.nlpicols {
        let col = &mut scip.cols[lp.lpicols[c]];
        // Non-basic columns to remove are exactly at 0.0.
        if col.primsol == 0.0 {
            col.age += 1;
        } else {
            col.age = 0;
        }
    }
    for r in 0..lp.nlpirows {
        let row = &mut scip.rows[lp.lpirows[r]];
        row.nlpsaftercreation += 1;
        // Basic rows to remove are exactly at 0.0.
        if row.dualsol == 0.0 {
            row.age += 1;
        } else {
            row.activeinlpcounter += 1;
            row.age = 0;
        }
    }
    Ok(())
}

/// `lpDelColset`: deletes the marked columns from the LP and the LP
/// interface; `coldstat` is 1 for columns to delete, and receives their
/// new positions (-1 for deleted ones).
fn lp_del_colset(scip: &mut Scip, coldstat: &mut [i32]) -> ScipResult {
    let ncols = lp!(scip).ncols;

    // Delete columns in LP solver.
    lpi::lpi_del_colset(&mut lpi!(scip), coldstat)?;

    // Update LP data respectively.
    for c in 0..ncols {
        let col = lp!(scip).cols[c];
        scip.cols[col].lppos = coldstat[c as usize];
        if coldstat[c as usize] == -1 {
            // Mark column to be deleted from the LPI, update column arrays
            // of all linked rows, and update the objective function vector
            // norms.
            mark_col_deleted(&mut scip.cols[col]);
            col_update_del_lp(scip, col);
            let unchangedobj = scip.cols[col].unchangedobj;
            lp_update_obj_norms(scip, unchangedobj, 0.0);
            scip.cols[col].lpdepth = -1;
            let lp = &mut lp!(scip);
            lp.cols[c] = crate::var::NONE;
            lp.lpicols[c] = crate::var::NONE;
            lp.ncols -= 1;
            lp.nremovablecols -= 1;
            lp.nlpicols -= 1;
        } else if coldstat[c as usize] < c {
            let n = coldstat[c as usize];
            let lp = &mut lp!(scip);
            lp.cols[n] = col;
            lp.lpicols[n] = col;
            scip.cols[col].lppos = n;
            scip.cols[col].lpipos = n;
            lp.cols[c] = crate::var::NONE;
            lp.lpicols[c] = crate::var::NONE;
        }
    }

    // Remove columns which are deleted from the lazy column array.
    let lp = &mut lp!(scip);
    let mut c = 0;
    while c < lp.nlazycols {
        if scip.cols[lp.lazycols[c]].lpipos < 0 {
            lp.lazycols[c] = lp.lazycols[lp.nlazycols - 1];
            lp.nlazycols -= 1;
        } else {
            c += 1;
        }
    }

    // Mark LP to be unsolved.
    if lp.ncols < ncols {
        lp.lpifirstchgcol = lp.nlpicols;
        lp_mark_unsolved_primal(lp);
    }
    Ok(())
}

/// `lpDelRowset`: deletes the marked rows from the LP and the LP
/// interface; `rowdstat` is 1 for rows to delete, and receives their new
/// positions (-1 for deleted ones).
fn lp_del_rowset(scip: &mut Scip, rowdstat: &mut [i32]) -> ScipResult {
    let nrows = lp!(scip).nrows;

    // Delete rows in LP solver.
    lpi::lpi_del_rowset(&mut lpi!(scip), rowdstat)?;

    // Update LP data respectively.
    for r in 0..nrows {
        let row = lp!(scip).rows[r];
        scip.rows[row].lppos = rowdstat[r as usize];
        if rowdstat[r as usize] == -1 {
            if scip.rows[row].removable {
                lp!(scip).nremovablerows -= 1;
            }

            // Mark row to be deleted from the LPI and update row arrays of
            // all linked columns.
            mark_row_deleted(&mut scip.rows[row]);
            row_update_del_lp(scip, row);
            scip.rows[row].lpdepth = -1;

            // Check, if row deletion events are tracked; if so, issue
            // ROWDELETEDLP event.
            let f = &scip.eventfilters[crate::scip::GLOBALEVENTFILTER];
            if f.len > 0 && (f.eventmask & crate::event::EVENTTYPE_ROWDELETEDLP) != 0 {
                let event = crate::event::event_create_row_deleted_lp(row);
                crate::event::eventqueue_add(scip, event)?;
            }
            let lpirow = lp!(scip).lpirows[r];
            row_release(scip, lpirow)?;
            lp!(scip).lpirows[r] = crate::var::NONE;
            row_unlock(scip, row);
            row_release(scip, row)?;
            let lp = &mut lp!(scip);
            lp.rows[r] = crate::var::NONE;
            lp.nrows -= 1;
            lp.nlpirows -= 1;
        } else if rowdstat[r as usize] < r {
            let n = rowdstat[r as usize];
            let lp = &mut lp!(scip);
            lp.rows[n] = row;
            lp.lpirows[n] = row;
            scip.rows[row].lppos = n;
            scip.rows[row].lpipos = n;
            lp.rows[r] = crate::var::NONE;
            lp.lpirows[r] = crate::var::NONE;
        }
    }

    // Mark LP to be unsolved.
    let lp = &mut lp!(scip);
    if lp.nrows < nrows {
        lp.lpifirstchgrow = lp.nlpirows;
        lp.solved = false;
        lp.dualfeasible = false;
        lp.dualchecked = false;
        lp.lpobjval = INVALID;
        lp.lpsolstat = LpSolStat::NotSolved;
    }
    Ok(())
}

/// `lpRemoveObsoleteCols`: removes all non-basic columns, that are too
/// old, beginning with the given firstcol.
fn lp_remove_obsolete_cols(scip: &mut Scip, firstcol: i32) -> ScipResult {
    let lp = &lp!(scip);
    if lp.nremovablecols == 0 || scip.set.lp_colagelimit == -1 || !lp.solisbasic {
        return Ok(());
    }
    let ncols = lp.ncols;
    let mut coldstat = vec![0; ncols as usize];
    let mut ndelcols = 0;

    // Mark obsolete columns to be deleted.
    let nnodes = scip.stat.nnodes;
    for c in firstcol..ncols {
        let col = &mut scip.cols[lp.cols[c]];
        if col.removable
            // Don't remove column a second time from same node (avoid
            // cycling), or a first time if marked nonremovable locally.
            && col.obsoletenode != nnodes
            && col.age > scip.set.lp_colagelimit
            && col.basisstatus != BASESTAT_BASIC
            // bestbd != 0 -> column would be priced in next time.
            && scip.set.is_zero(col_get_best_bound(col))
        {
            coldstat[c as usize] = 1;
            ndelcols += 1;
            col.obsoletenode = nnodes;
        }
    }

    // Delete the marked columns in the LP solver interface, update the LP
    // respectively.
    if ndelcols > 0 {
        lp_del_colset(scip, &mut coldstat)?;
    }
    Ok(())
}

/// `lpRemoveObsoleteRows`: removes all basic rows, that are too old,
/// beginning with the given firstrow.
fn lp_remove_obsolete_rows(scip: &mut Scip, firstrow: i32) -> ScipResult {
    let lp = &lp!(scip);
    if lp.nremovablerows == 0 || scip.set.lp_rowagelimit == -1 || !lp.solisbasic {
        return Ok(());
    }
    let nrows = lp.nrows;
    let mut rowdstat = vec![0; nrows as usize];
    let mut ndelrows = 0;

    // Mark obsolete rows to be deleted.
    let nnodes = scip.stat.nnodes;
    for r in firstrow..nrows {
        let row = &mut scip.rows[lp.rows[r]];
        if row.removable
            // Don't remove row a second time from same node (avoid
            // cycling), or a first time if marked nonremovable locally.
            && row.obsoletenode != nnodes
            && row.age > scip.set.lp_rowagelimit
            && row.basisstatus == BASESTAT_BASIC
        {
            rowdstat[r as usize] = 1;
            ndelrows += 1;
            row.obsoletenode = nnodes;
        }
    }

    // Delete the marked rows in the LP solver interface, update the LP
    // respectively.
    if ndelrows > 0 {
        lp_del_rowset(scip, &mut rowdstat)?;
    }
    Ok(())
}

/// `SCIPlpRemoveNewObsoletes`: removes all non-basic columns and basic
/// rows in the part of the LP created at the current node, that are too
/// old.
pub fn lp_remove_new_obsoletes(scip: &mut Scip) -> ScipResult {
    let (firstnewcol, ncols) = (lp!(scip).firstnewcol, lp!(scip).ncols);
    if firstnewcol < ncols {
        lp_remove_obsolete_cols(scip, firstnewcol)?;
    }
    let (firstnewrow, nrows) = (lp!(scip).firstnewrow, lp!(scip).nrows);
    if firstnewrow < nrows {
        lp_remove_obsolete_rows(scip, firstnewrow)?;
    }
    Ok(())
}

/// `SCIPlpRemoveAllObsoletes`: removes all non-basic columns and basic
/// rows in whole LP, that are too old.
pub fn lp_remove_all_obsoletes(scip: &mut Scip) -> ScipResult {
    if 0 < lp!(scip).ncols {
        lp_remove_obsolete_cols(scip, 0)?;
    }
    if 0 < lp!(scip).nrows {
        lp_remove_obsolete_rows(scip, 0)?;
    }
    Ok(())
}

/// `lpCleanupCols`: removes all non-basic columns at 0.0 beginning with
/// the given firstcol.
fn lp_cleanup_cols(scip: &mut Scip, firstcol: i32) -> ScipResult {
    let lp = &lp!(scip);
    if lp.nremovablecols == 0 || !lp.solisbasic {
        return Ok(());
    }
    let ncols = lp.ncols;
    let mut coldstat = vec![0; ncols as usize];
    let mut ndelcols = 0;

    // Mark unused columns to be deleted.
    for c in firstcol..ncols {
        let lpicol = &scip.cols[lp.lpicols[c]];
        if lpicol.removable
            && lpicol.basisstatus != BASESTAT_BASIC
            // Non-basic columns to remove are exactly at 0.0.
            && lpicol.primsol == 0.0
            // bestbd != 0 -> column would be priced in next time.
            && scip.set.is_zero(col_get_best_bound(&scip.cols[lp.cols[c]]))
        {
            coldstat[c as usize] = 1;
            ndelcols += 1;
        }
    }

    // Delete the marked columns in the LP solver interface, update the LP
    // respectively.
    if ndelcols > 0 {
        lp_del_colset(scip, &mut coldstat)?;
    }
    Ok(())
}

/// `lpCleanupRows`: removes all basic rows beginning with the given
/// firstrow.
fn lp_cleanup_rows(scip: &mut Scip, firstrow: i32) -> ScipResult {
    let lp = &lp!(scip);
    if lp.nremovablerows == 0 || !lp.solisbasic {
        return Ok(());
    }
    let nrows = lp.nrows;
    let mut rowdstat = vec![0; nrows as usize];
    let mut ndelrows = 0;

    // Mark unused rows to be deleted.
    for r in firstrow..nrows {
        let lpirow = &scip.rows[lp.lpirows[r]];
        if lpirow.removable && lpirow.basisstatus == BASESTAT_BASIC {
            rowdstat[r as usize] = 1;
            ndelrows += 1;
        }
    }

    // Delete the marked rows in the LP solver interface, update the LP
    // respectively.
    if ndelrows > 0 {
        lp_del_rowset(scip, &mut rowdstat)?;
    }
    Ok(())
}

/// `SCIPlpCleanupNew`: removes all non-basic columns at 0.0 and basic
/// rows in the part of the LP created at the current node.
pub fn lp_cleanup_new(scip: &mut Scip, root: bool) -> ScipResult {
    let cleanupcols = if root { scip.set.lp_cleanupcolsroot } else { scip.set.lp_cleanupcols };
    let cleanuprows = if root { scip.set.lp_cleanuprowsroot } else { scip.set.lp_cleanuprows };
    let (firstnewcol, ncols) = (lp!(scip).firstnewcol, lp!(scip).ncols);
    if cleanupcols && firstnewcol < ncols {
        lp_cleanup_cols(scip, firstnewcol)?;
    }
    let (firstnewrow, nrows) = (lp!(scip).firstnewrow, lp!(scip).nrows);
    if cleanuprows && firstnewrow < nrows {
        lp_cleanup_rows(scip, firstnewrow)?;
    }
    Ok(())
}

/// `SCIPlpCleanupAll`: removes all non-basic columns at 0.0 and basic rows
/// in the whole LP.
pub fn lp_cleanup_all(scip: &mut Scip, root: bool) -> ScipResult {
    let cleanupcols = if root { scip.set.lp_cleanupcolsroot } else { scip.set.lp_cleanupcols };
    let cleanuprows = if root { scip.set.lp_cleanuprowsroot } else { scip.set.lp_cleanuprows };
    if cleanupcols && 0 < lp!(scip).ncols {
        lp_cleanup_cols(scip, 0)?;
    }
    if cleanuprows && 0 < lp!(scip).nrows {
        lp_cleanup_rows(scip, 0)?;
    }
    Ok(())
}

/// `SCIPlpRemoveRedundantRows`: removes all redundant rows that were
/// added at the current node.
pub fn lp_remove_redundant_rows(scip: &mut Scip) -> ScipResult {
    let (firstnewrow, nrows) = (lp!(scip).firstnewrow, lp!(scip).nrows);
    if firstnewrow == nrows {
        return Ok(());
    }
    let mut rowdstat = vec![0; nrows as usize];
    let mut ndelrows = 0;

    // Mark redundant basic rows to be deleted.
    for r in firstnewrow..nrows {
        let lpirow = lp!(scip).lpirows[r];
        if (!lp!(scip).solisbasic || scip.rows[lpirow].basisstatus == BASESTAT_BASIC) && row_is_redundant(scip, lpirow) {
            rowdstat[r as usize] = 1;
            ndelrows += 1;
        }
    }

    // Delete the marked rows in the LP solver interface, update the LP
    // respectively.
    if ndelrows > 0 {
        lp_del_rowset(scip, &mut rowdstat)?;
    }
    Ok(())
}

/// `SCIPlpStartDive`: initiates LP diving, making methods
/// SCIPchgVarObjDive(), SCIPchgVarLbDive(), and SCIPchgVarUbDive()
/// available.
pub fn lp_start_dive(scip: &mut Scip) -> ScipResult {
    // Save current LPI state (basis information).
    let state = lpi::lpi_get_state(&mut lpi!(scip))?;
    let lp = &mut lp!(scip);
    lp.divelpistate = Some(Box::new(state));
    lp.divelpwasprimfeas = lp.primalfeasible;
    lp.divelpwasdualfeas = lp.dualfeasible;
    lp.divelpwasprimchecked = lp.primalchecked;
    lp.divelpwasdualchecked = lp.dualchecked;

    // Save current LP values dependent on the solution.
    lp_store_sol_vals(scip);

    // Store LP solution values in columns and rows, if the LP is solved
    // and restoring by resolving is off.
    if !scip.set.lp_resolverestore && lp!(scip).solved {
        let store = match lp!(scip).lpsolstat {
            LpSolStat::Optimal => {
                lp_get_sol(scip, false, false)?;
                true
            }
            LpSolStat::UnboundedRay => {
                lp_get_unbounded_sol(scip, false)?;
                true
            }
            LpSolStat::ObjLimit | LpSolStat::IterLimit | LpSolStat::TimeLimit => {
                lp_get_sol(scip, false, false)?;
                true
            }
            LpSolStat::Infeasible => {
                lp_get_dualfarkas(scip, false, false)?;
                true
            }
            LpSolStat::NotSolved | LpSolStat::Error => false,
        };
        if store {
            for c in 0..lp!(scip).ncols {
                let col = lp!(scip).cols[c];
                col_store_sol_vals(scip, col);
            }
            let infeasible = lp!(scip).storedsolvals.as_ref().unwrap().lpsolstat == LpSolStat::Infeasible;
            for r in 0..lp!(scip).nrows {
                let row = lp!(scip).rows[r];
                row_store_sol_vals(scip, row, infeasible);
            }
        }
    }

    // Store LPI iteration limit.
    let itlim = lpi::lpi_get_intpar(&lpi!(scip), LpParam::LpItLim)?;
    let domchgcount = scip.stat.domchgcount;
    let lp = &mut lp!(scip);
    lp.divinglpiitlim = itlim;

    // Remember the number of domain changes.
    lp.divenolddomchgs = domchgcount;

    // Store current number of rows.
    lp.ndivingrows = lp.nrows;

    // Switch to diving mode.
    lp.diving = true;
    Ok(())
}

/// `SCIPlpEndDive`: quits LP diving and resets bounds and objective
/// values of columns to the current node's values.
pub fn lp_end_dive(scip: &mut Scip, prob: usize, vars: &[VarId]) -> ScipResult {
    // Reset all columns' objective values and bounds to its original
    // values.
    for &var in vars {
        if scip.vars[var].varstatus == VarStatus::Column {
            let x = &scip.vars[var];
            let (col, obj, lb, ub) = (x.data.col, x.obj, x.locdom.lb, x.locdom.ub);
            col_chg_obj(scip, col, obj)?;
            col_chg_lb(scip, col, lb)?;
            col_chg_ub(scip, col, ub)?;
        }
    }

    // Remove rows which were added in diving mode.
    let ndivingrows = lp!(scip).ndivingrows;
    lp_shrink_rows(scip, ndivingrows)?;

    // Undo changes to left hand sides and right hand sides.
    while lp!(scip).ndivechgsides > 0 {
        let lp = &mut lp!(scip);
        lp.ndivechgsides -= 1;
        let oldside = lp.divechgsides[lp.ndivechgsides];
        let sidetype = lp.divechgsidetypes[lp.ndivechgsides];
        let row = lp.divechgrows[lp.ndivechgsides];
        if sidetype == SideType::Left {
            row_chg_lhs(scip, row, oldside)?;
        } else {
            row_chg_rhs(scip, row, oldside)?;
        }
    }

    // Restore LPI iteration limit.
    let itlim = lp!(scip).divinglpiitlim;
    lp_set_iteration_limit(scip, itlim)?;

    // Reload LPI state saved at start of diving and free it afterwards; it
    // may be NULL, in which case simply nothing happens.
    let state = lp!(scip).divelpistate.take();
    let lp = &lp!(scip);
    let (pf, pc, df, dc) = (lp.divelpwasprimfeas, lp.divelpwasprimchecked, lp.divelpwasdualfeas, lp.divelpwasdualchecked);
    lp_set_state(scip, prob, state.as_deref(), pf, pc, df, dc)?;
    let lp = &mut lp!(scip);
    lp.divelpwasprimfeas = true;
    lp.divelpwasdualfeas = true;
    lp.divelpwasprimchecked = true;
    lp.divelpwasdualchecked = true;

    // Switch to standard (non-diving) mode.
    lp.diving = false;
    lp.divingobjchg = false;

    // If the LP was solved before starting the dive, but not to
    // optimality (or unboundedness), then we need to solve the LP again to
    // reset the solution; restoring by solving the LP again in either case
    // can be forced by setting the parameter resolverestore to TRUE;
    // restoring an unbounded ray after solve does not seem to work
    // currently (bug 631), so we resolve also in this case.
    let stored = lp.storedsolvals.as_ref().unwrap();
    let (lpissolved, storedstat) = (stored.lpissolved, stored.lpsolstat);
    if lpissolved && (scip.set.lp_resolverestore || storedstat != LpSolStat::Optimal || lp.divenolddomchgs < scip.stat.domchgcount) {
        let lperror = lp_solve_and_eval(scip, prob, -1, false, false, false, false)?;
        if lperror {
            lp_numerical_trouble_message(scip, VERBLEVEL_FULL, format_args!("unresolved when resolving LP after diving"));
            lp!(scip).resolvelperror = true;
        } else {
            let solstat = lp_get_solstat(scip);
            if solstat != LpSolStat::Optimal && solstat != LpSolStat::Infeasible && solstat != LpSolStat::UnboundedRay && solstat != LpSolStat::ObjLimit {
                log::info!("LP was not resolved to a sufficient status after diving");
                lp!(scip).resolvelperror = true;
            }
        }
    }
    // Otherwise, we can just reload the buffered LP solution values at
    // start of diving; this has the advantage that we are guaranteed to
    // continue with the same LP status as before diving.
    else {
        if lp!(scip).nlazycols > 0 {
            update_lazy_bounds(scip);
            lp_flush(scip, prob)?;
        }

        // Increment lp counter to ensure that we do not use solution
        // values from the last solved diving lp.
        scip.stat.lpcount += 1;
        let lpcount = scip.stat.lpcount;
        if lpissolved
            && matches!(
                storedstat,
                LpSolStat::Optimal | LpSolStat::UnboundedRay | LpSolStat::ObjLimit | LpSolStat::IterLimit | LpSolStat::TimeLimit | LpSolStat::Infeasible
            )
        {
            lp_restore_sol_vals(scip, lpcount);
            let freebuffer = scip.set.lp_freesolvalbuffers;
            for c in 0..lp!(scip).ncols {
                let col = lp!(scip).cols[c];
                col_restore_sol_vals(scip, col, lpcount, freebuffer);
            }
            let infeasible = storedstat == LpSolStat::Infeasible;
            for r in 0..lp!(scip).nrows {
                let row = lp!(scip).rows[r];
                row_restore_sol_vals(scip, row, lpcount, freebuffer, infeasible);
            }
        } else {
            lp_restore_sol_vals(scip, -1);
        }
    }
    Ok(())
}

/// `DIVESTACKGROWFACT`.
const DIVESTACKGROWFACT: f64 = 1.5;

/// `SCIPlpRecordOldRowSideDive`: records a current row side such that any
/// change will be undone after diving.
pub fn lp_record_old_row_side_dive(scip: &mut Scip, row: RowId, sidetype: SideType) {
    let side = if sidetype == SideType::Left { scip.rows[row].lhs } else { scip.rows[row].rhs };
    let lp = &mut lp!(scip);

    // Realloc if more memory is needed.
    if lp.ndivechgsides == lp.divechgsidessize {
        let n = lp.divechgsidessize + 1;
        realloc_dive_chg_side_arrays(lp, n, DIVESTACKGROWFACT);
    }

    // Store old side, side type, and row.
    lp.divechgsides[lp.ndivechgsides] = side;
    lp.divechgsidetypes[lp.ndivechgsides] = sidetype;
    lp.divechgrows[lp.ndivechgsides] = row;
    lp.ndivechgsides += 1;
}

/// `SCIPlpStartProbing`: informs the LP that probing mode was initiated.
pub fn lp_start_probing(scip: &mut Scip) {
    lp!(scip).probing = true;
}

/// `SCIPlpEndProbing`: informs the LP that probing mode was finished.
pub fn lp_end_probing(scip: &mut Scip) {
    lp!(scip).probing = false;
}

/// `SCIPlpStartStrongbranchProbing`: informs the LP that probing mode is
/// now used for strongbranching.
pub fn lp_start_strongbranch_probing(scip: &mut Scip) {
    lp!(scip).strongbranchprobing = true;
}

/// `SCIPlpEndStrongbranchProbing`: informs the LP that probing mode is
/// not used for strongbranching anymore.
pub fn lp_end_strongbranch_probing(scip: &mut Scip) {
    lp!(scip).strongbranchprobing = false;
}

/// `SCIPlpRecalculateObjSqrNorm`: recalculates Euclidean norm of
/// objective function vector of column variables if it have gotten
/// unreliable during calculation.
pub fn lp_recalculate_obj_sqr_norm(scip: &mut Scip) {
    let lp = &mut **scip.lp.as_mut().unwrap();
    if lp.objsqrnormunreliable {
        lp.objsqrnorm = 0.0;
        let mut c = lp.ncols - 1;
        while c >= 0 {
            let unchangedobj = scip.cols[lp.cols[c]].unchangedobj;
            lp.objsqrnorm += unchangedobj * unchangedobj;
            c -= 1;
        }

        // Due to numerical troubles it still can appear that
        // lp->objsqrnorm is a little bit smaller than 0.
        lp.objsqrnorm = crate::def::max(lp.objsqrnorm, 0.0);
        lp.objsqrnormunreliable = false;
    }
}

/// `SCIPlpGetNUnfixedCols`: gets the current number of unfixed LP columns.
pub fn lp_get_n_unfixed_cols(scip: &Scip, eps: f64) -> i32 {
    let lp = scip.lp.as_ref().unwrap();
    let mut nunfixedcols = 0;
    for c in 0..lp.ncols {
        let col = &scip.cols[lp.cols[c]];
        if col.ub - col.lb > eps {
            nunfixedcols += 1;
        }
    }
    nunfixedcols
}

/// `SCIPlpGetObjNorm`: gets Euclidean norm of objective function vector
/// of column variables, only use this method if lp->objsqrnormunreliable
/// == FALSE.
pub fn lp_get_obj_norm(scip: &Scip) -> f64 {
    scip.lp.as_ref().unwrap().objsqrnorm.sqrt()
}

/// `SCIPlpGetRootObjval`: gets objective value of root LP.
pub fn lp_get_root_objval(scip: &Scip) -> f64 {
    let lp = scip.lp.as_ref().unwrap();
    crate::def::min(lp.rootlpobjval + lp.rootlooseobjval, INVALID)
}

/// `SCIPlpGetDualDegeneracy`: compute the dual degeneracy of the current
/// LP solution and the variable-constraint ratio of the optimal face;
/// returns (degeneracy, varconsratio).
pub fn lp_get_dual_degeneracy(scip: &mut Scip) -> ScipResult<(f64, f64)> {
    if lp!(scip).validdegeneracylp != scip.stat.nlps {
        lp!(scip).validdegeneracylp = scip.stat.nlps;

        // If the LP was solved to optimality, we compute the dual
        // degeneracy.
        if lp_get_solstat(scip) == LpSolStat::Optimal {
            let ncols = lp!(scip).ncols;
            let nrows = lp!(scip).nrows;
            let mut nfixedcols = 0;
            let mut nalreadyfixedcols = 0;
            let mut nfixedrows = 0;
            let mut nineq = 0;
            let mut nbasicequalities = 0;

            // Count number of columns that will be fixed by reduced cost
            // fixing, if the LP was solved to optimality.
            let mut c = ncols - 1;
            while c >= 0 {
                let col = lp!(scip).cols[c];
                if scip.cols[col].basisstatus != BASESTAT_BASIC {
                    let redcost = col_get_redcost(scip, col);
                    if !scip.set.is_zero(redcost) {
                        nfixedcols += 1;
                    } else if scip.set.is_eq(scip.cols[col].lb, scip.cols[col].ub) {
                        nalreadyfixedcols += 1;
                    }
                }
                c -= 1;
            }

            // Count number of rows that are tight (the dual solution is
            // non-zero) and inequalities.
            let mut r = nrows - 1;
            while r >= 0 {
                let row = lp!(scip).rows[r];
                let (lhs, rhs) = (scip.rows[row].lhs, scip.rows[row].rhs);
                if !scip.set.is_eq(lhs, rhs) {
                    let dualsol = scip.rows[row].get_dualsol();
                    nineq += 1;
                    if scip.rows[row].basisstatus != BASESTAT_BASIC && !scip.set.is_zero(dualsol) {
                        let activity = row_get_lp_activity(scip, row);
                        if scip.set.is_eq(lhs, activity) || scip.set.is_eq(rhs, activity) {
                            nfixedrows += 1;
                        }
                    }
                } else if scip.rows[row].basisstatus == BASESTAT_BASIC {
                    nbasicequalities += 1;
                }
                r -= 1;
            }

            // Compute the dual degeneracy and the variable-constraint ratio.
            let lp = &mut lp!(scip);
            let denom = ncols + nineq - nrows + nbasicequalities - nalreadyfixedcols;
            lp.degeneracy = if denom > 0 { 1.0 - 1.0 * (nfixedcols + nfixedrows) as f64 / denom as f64 } else { 0.0 };
            lp.varconsratio = if nrows > 0 {
                1.0 * (ncols + nineq + nbasicequalities - nfixedcols - nfixedrows - nalreadyfixedcols) as f64 / nrows as f64
            } else {
                1.0
            };
        } else {
            let lp = &mut lp!(scip);
            lp.degeneracy = 0.0;
            lp.varconsratio = 0.0;
        }
    }
    let lp = &lp!(scip);
    Ok((lp.degeneracy, lp.varconsratio))
}

impl Col {
    /// `SCIPcolGetPrimsol`: the primal LP solution value, 0 for columns not
    /// in the LP (the NDEBUG macro).
    #[inline]
    pub fn get_primsol(&self) -> f64 {
        if self.lppos >= 0 { self.primsol } else { 0.0 }
    }

    /// `SCIPcolIsInLP`.
    #[inline]
    pub fn is_in_lp(&self) -> bool {
        self.lppos >= 0
    }
}

impl Row {
    /// `SCIProwGetDualsol`: the dual LP solution value, 0 for rows not in
    /// the LP (the NDEBUG macro).
    #[inline]
    pub fn get_dualsol(&self) -> f64 {
        if self.lppos >= 0 { self.dualsol } else { 0.0 }
    }

    /// `SCIProwGetDualfarkas` (the NDEBUG macro).
    #[inline]
    pub fn get_dualfarkas(&self) -> f64 {
        if self.lppos >= 0 { self.dualfarkas } else { 0.0 }
    }

    /// `SCIProwIsInLP`.
    #[inline]
    pub fn is_in_lp(&self) -> bool {
        self.lppos >= 0
    }

    /// `SCIProwGetOriginCons`: the creating constraint, or `NONE`.
    #[inline]
    pub fn get_origin_cons(&self) -> usize {
        if self.origintype == RowOriginType::Cons { self.origin } else { crate::var::NONE }
    }

    /// `SCIProwGetOriginSepa`: the creating separator, or `NONE`.
    #[inline]
    pub fn get_origin_sepa(&self) -> usize {
        if self.origintype == RowOriginType::Sepa { self.origin } else { crate::var::NONE }
    }
}

/// `SCIProwGetOriginConshdlr`: the constraint handler that created the
/// row, or `NONE`.
pub fn row_get_origin_conshdlr(scip: &Scip, row: RowId) -> usize {
    let r = &scip.rows[row];
    match r.origintype {
        RowOriginType::Conshdlr => r.origin,
        RowOriginType::Cons => scip.conss[r.origin].conshdlr,
        _ => crate::var::NONE,
    }
}

/// `SCIPcolGetBasisStatus` (the NDEBUG macro).
pub fn col_get_basis_status(scip: &Scip, col: ColId) -> u32 {
    scip.cols[col].basisstatus
}

/// `SCIPcolGetPrimsol` (the NDEBUG macro).
pub fn col_get_primsol(scip: &Scip, col: ColId) -> f64 {
    scip.cols[col].get_primsol()
}

/// `SCIPlpSetRootLPIsRelax`: sets whether the root LP is a relaxation of
/// the problem and its optimal objective value is a global lower bound.
pub fn lp_set_root_lpis_relax(scip: &mut Scip, isrelax: bool) {
    lp!(scip).rootlpisrelax = isrelax;
}

/// `SCIPlpIsRootLPRelax`.
pub fn lp_is_root_lp_relax(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().rootlpisrelax
}

/// `SCIPlpSetIsRelax`: sets whether the current LP is a relaxation of the
/// current problem and its optimal objective value is a local lower bound.
pub fn lp_set_is_relax(scip: &mut Scip, relax: bool) {
    lp!(scip).isrelax = relax;
}

/// `SCIPlpIsRelax`.
pub fn lp_is_relax(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().isrelax
}

/// `SCIPlpIsSolved`.
pub fn lp_is_solved(scip: &Scip) -> bool {
    let lp = scip.lp.as_ref().unwrap();
    lp.flushed && lp.solved
}

/// `SCIPlpIsPrimalReliable`.
pub fn lp_is_primal_reliable(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().primalchecked
}

/// `SCIPlpIsDualReliable`.
pub fn lp_is_dual_reliable(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().dualchecked
}

/// `SCIPlpIsSolBasic`.
pub fn lp_is_sol_basic(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().solisbasic
}

/// `SCIPlpDiving`.
pub fn lp_diving(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().diving
}

/// `SCIPlpDivingObjChanged`.
pub fn lp_diving_obj_changed(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().divingobjchg
}

/// `SCIPlpMarkDivingObjChanged`.
pub fn lp_mark_diving_obj_changed(scip: &mut Scip) {
    lp!(scip).divingobjchg = true;
}

/// `SCIPlpUnmarkDivingObjChanged`.
pub fn lp_unmark_diving_obj_changed(scip: &mut Scip) {
    lp!(scip).divingobjchg = false;
}

/// `SCIPlpDivingRowsChanged`.
pub fn lp_diving_rows_changed(scip: &Scip) -> bool {
    scip.lp.as_ref().unwrap().ndivechgsides > 0
}

/// `SCIPlpGetNCols`.
pub fn lp_get_n_cols(scip: &Scip) -> i32 {
    scip.lp.as_ref().unwrap().ncols
}

/// `SCIPlpGetNRows`.
pub fn lp_get_n_rows(scip: &Scip) -> i32 {
    scip.lp.as_ref().unwrap().nrows
}

/// `SCIPlpGetNewcols`: the columns created at the current node.
pub fn lp_get_newcols(scip: &Scip) -> &[ColId] {
    let lp = scip.lp.as_ref().unwrap();
    &lp.cols.as_slice()[lp.firstnewcol as usize..lp.ncols as usize]
}

/// `SCIPlpGetNNewcols`.
pub fn lp_get_n_newcols(scip: &Scip) -> i32 {
    let lp = scip.lp.as_ref().unwrap();
    lp.ncols - lp.firstnewcol
}

/// `SCIPlpGetNewrows`: the rows created at the current node.
pub fn lp_get_newrows(scip: &Scip) -> &[RowId] {
    let lp = scip.lp.as_ref().unwrap();
    &lp.rows.as_slice()[lp.firstnewrow as usize..lp.nrows as usize]
}

/// `SCIPlpGetNNewrows`.
pub fn lp_get_n_newrows(scip: &Scip) -> i32 {
    let lp = scip.lp.as_ref().unwrap();
    lp.nrows - lp.firstnewrow
}

/// `SCIProwGetOriginSepa`: the separator that created the row, or `NONE`.
pub fn row_get_origin_sepa(scip: &Scip, row: RowId) -> usize {
    let r = &scip.rows[row];
    if r.origintype == RowOriginType::Sepa { r.origin } else { crate::var::NONE }
}
