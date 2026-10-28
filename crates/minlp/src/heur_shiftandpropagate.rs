//! Port of `heur_shiftandpropagate.c`: pre-root heuristic to expand an
//! auxiliary branch-and-bound tree and apply propagation techniques.
//!
//! The constraint matrix, the violated row bookkeeping and the LP column
//! order live in [`State`] inside the heuristic data while the heuristic
//! runs, because the event handler updates them whenever propagation
//! changes a bound. Code that changes the state takes it out of the
//! heuristic data and puts it back before calling into SCIP.

use std::any::Any;

use crate::{
    api,
    event::{EVENTTYPE_BOUNDCHANGED, EVENTTYPE_GBDCHANGED, Event, EventType},
    heur::{HEURTIMING_BEFORENODE, HeurTiming},
    misc::RandNumGen,
    prob::TRANSPROB,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_probing,
    types::{ColId, EventhdlrId, HeurId, LockType, LpSolStat, ScipResultCode, VarId, VarType},
    var::{NONE, var_get_n_locks_down_type, var_get_n_locks_up_type, var_is_binary, var_is_implied_integral, var_is_integral},
};

const HEUR_NAME: &str = "shiftandpropagate";
const HEUR_DESC: &str = "Pre-root heuristic to expand an auxiliary branch-and-bound tree and apply propagation techniques";
const HEUR_DISPCHAR: u8 = b'p';
const HEUR_PRIORITY: i32 = 1000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_BEFORENODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// The heuristic row weight for inequalities.
const DEFAULT_WEIGHT_INEQUALITY: i32 = 1;
/// The heuristic row weight for equations.
const DEFAULT_WEIGHT_EQUALITY: i32 = 3;
/// Should continuous variables be relaxed from the problem?
const DEFAULT_RELAX: bool = true;
/// Is propagation of solution values enabled?
const DEFAULT_PROBING: bool = true;
/// Should heuristic only be executed if no primal solution was found, yet?
const DEFAULT_ONLYWITHOUTSOL: bool = true;
/// The default number of propagation rounds for each propagation used.
const DEFAULT_NPROPROUNDS: i32 = 10;
/// Fixed maximum number of propagations.
const DEFAULT_PROPBREAKER: i32 = 65000;
/// Fixed maximum number of allowed cutoffs before the heuristic stops.
const DEFAULT_CUTOFFBREAKER: i32 = 15;
/// The default random seed for random number generation.
const DEFAULT_RANDSEED: u32 = 29;
/// The default key for variable sorting.
const DEFAULT_SORTKEY: u8 = b'v';
/// Should variables be processed in sorted order?
const DEFAULT_SORTVARS: bool = true;
/// Should variable statistics be collected during probing?
const DEFAULT_COLLECTSTATS: bool = true;
/// Should the heuristic stop calculating optimal shift values when no more
/// rows are violated?
const DEFAULT_STOPAFTERFEASIBLE: bool = true;
/// Should binary variables be shifted first?
const DEFAULT_PREFERBINARIES: bool = true;
/// Should the heuristic choose the best candidate in every round? (set to
/// FALSE for static order)?
const DEFAULT_SELECTBEST: bool = false;
/// Maximum percentage of allowed cutoffs before stopping the heuristic.
const DEFAULT_MAXCUTOFFQUOT: f64 = 0.0;
/// Options sorting key: (n)orms down, norms (u)p, (v)iolated rows
/// decreasing, viola(t)ed rows increasing, or (r)andom.
const SORTKEYS: &str = "nrtuv";
/// Should variables with a zero shifting value be delayed instead of being
/// fixed?
const DEFAULT_NOZEROFIXING: bool = false;
/// Should binary variables with no locks in one direction be fixed to that
/// direction?
const DEFAULT_FIXBINLOCKS: bool = true;
/// Should binary variables with no locks be preferred in the ordering?
const DEFAULT_BINLOCKSFIRST: bool = false;
/// Should coefficients and left/right hand sides be normalized by max row
/// coeff?
const DEFAULT_NORMALIZE: bool = true;
/// Should row weight be increased every time the row is violated?
const DEFAULT_UPDATEWEIGHTS: bool = false;
/// Should implicit integer variables be treated as continuous variables?
const DEFAULT_IMPLISCONTINUOUS: bool = true;
/// Minimum fixing rate over all variables (including continuous) to solve
/// LP.
const DEFAULT_MINFIXINGRATELP: f64 = 0.0;

const EVENTHDLR_NAME: &str = "eventhdlrshiftandpropagate";
const EVENTHDLR_DESC: &str = "event handler to catch bound changes";
const EVENTTYPE_SHIFTANDPROPAGATE: EventType = EVENTTYPE_BOUNDCHANGED | EVENTTYPE_GBDCHANGED;

/*
 * Data structures
 */

/// Status of a variable in heuristic transformation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransformStatus {
    /// Variable has not been transformed yet.
    None = 0,
    /// Variable has been shifted by using lower bound (x-lb).
    Lb = 1,
    /// Variable has been negated by using upper bound (ub-x).
    Neg = 2,
    /// Variable does not have to be shifted.
    Free = 3,
}

/// Information about the matrix after its heuristic transformation.
#[derive(Default)]
struct ConstraintMatrix {
    /// Matrix coefficients row by row.
    rowmatvals: Vec<f64>,
    /// The indices of the corresponding variables.
    rowmatind: Vec<i32>,
    /// The starting indices of each row.
    rowmatbegin: Vec<i32>,
    /// Matrix coefficients column by column.
    colmatvals: Vec<f64>,
    /// The indices of the corresponding rows for each coefficient.
    colmatind: Vec<i32>,
    /// The starting indices of each column.
    colmatbegin: Vec<i32>,
    /// The number of violated rows for every variable.
    violrows: Vec<i32>,
    /// Information about transform status of every discrete variable.
    transformstatus: Vec<TransformStatus>,
    /// Left hand side vector after normalization.
    lhs: Vec<f64>,
    /// Right hand side vector after normalization.
    rhs: Vec<f64>,
    /// Vector norms of all discrete problem variables after normalization.
    colnorms: Vec<f64>,
    /// The upper bounds of every non-continuous variable after
    /// transformation.
    upperbounds: Vec<f64>,
    /// Values by which original discrete variable bounds were shifted.
    transformshiftvals: Vec<f64>,
    /// Number of nonzero column entries.
    nnonzs: i32,
    /// Number of rows of matrix.
    nrows: i32,
    /// The number of columns in matrix (including continuous vars).
    ncols: i32,
    /// Number of discrete problem variables.
    ndiscvars: i32,
    /// Indicates if the matrix data has already been normalized.
    normalized: bool,
}

/// What the heuristic and its event handler share while it runs (the C
/// heuristic data's column and weight arrays and the event handler data).
#[derive(Default)]
struct State {
    /// Stores lp columns with discrete variables before cont. variables.
    lpcols: Vec<ColId>,
    /// The number of lp columns.
    nlpcols: i32,
    /// Row weight storage.
    rowweights: Vec<i32>,
    /// The constraint matrix of the heuristic.
    matrix: ConstraintMatrix,
    /// All currently violated LP rows.
    violatedrows: Vec<i32>,
    /// Position in violatedrows array for every row.
    violatedrowpos: Vec<i32>,
    /// The total number of currently violated rows.
    nviolatedrows: i32,
    /// Copies of the parameters the state's methods read.
    updateweights: bool,
    normalize: bool,
    impliscontinuous: bool,
}

/// Primal heuristic data.
struct HeurData {
    /// Random number generation.
    randnumgen: Option<RandNumGen>,
    /// Should continuous variables be relaxed from the problem.
    relax: bool,
    /// Should probing be executed?
    probing: bool,
    /// Should heuristic only be executed if no primal solution was found,
    /// yet?
    onlywithoutsol: bool,
    /// The default number of propagation rounds for each propagation used.
    nproprounds: i32,
    /// The number of cutoffs before heuristic execution is stopped, or -1
    /// for no limit.
    cutoffbreaker: i32,
    /// Event handler to register and process variable bound changes.
    eventhdlr: EventhdlrId,

    /// Maximum percentage of allowed cutoffs before stopping the heuristic.
    maxcutoffquot: f64,
    /// Minimum fixing rate over all variables (including continuous) to
    /// solve LP.
    minfixingratelp: f64,
    /// The key by which variables are sorted.
    sortkey: u8,
    /// Should variables be processed in sorted order?
    sortvars: bool,
    /// Should variable statistics be collected during probing?
    collectstats: bool,
    /// Should the heuristic stop calculating optimal shift values when no
    /// more rows are violated?
    stopafterfeasible: bool,
    /// Should binary variables be shifted first?
    preferbinaries: bool,
    /// Should variables with a zero shifting value be delayed instead of
    /// being fixed?
    nozerofixing: bool,
    /// Should binary variables with no locks in one direction be fixed to
    /// that direction?
    fixbinlocks: bool,
    /// Should binary variables with no locks be preferred in the ordering?
    binlocksfirst: bool,
    /// Should coefficients be normalized by max row coeff for col norm?
    normalize: bool,
    /// Should row weight be increased every time the row is violated?
    updateweights: bool,
    /// Should implicit integer variables be treated as continuous
    /// variables?
    impliscontinuous: bool,
    /// Should the heuristic choose the best candidate in every round? (set
    /// to FALSE for static order)?
    selectbest: bool,

    /// The running heuristic's state; `matrix_set` says whether the event
    /// handler may update it (C: `eventhdlrdata->matrix != NULL`).
    state: Option<State>,
    matrix_set: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

fn take_state(scip: &mut Scip, heur: HeurId) -> State {
    hdm(scip, heur).state.take().unwrap()
}

fn put_state(scip: &mut Scip, heur: HeurId, st: State) {
    hdm(scip, heur).state = Some(st);
}

/*
 * Local methods
 */

/// `varIsDiscrete`: returns whether a given variable is counted as
/// discrete, depending on the parameter impliscontinuous.
fn var_is_discrete(scip: &Scip, var: VarId, impliscontinuous: bool) -> bool {
    var_is_integral(scip, var) && (!impliscontinuous || !var_is_implied_integral(scip, var))
}

/// `colIsDiscrete`: returns whether a given column is counted as discrete,
/// depending on the parameter impliscontinuous.
fn col_is_discrete(scip: &Scip, col: ColId, impliscontinuous: bool) -> bool {
    scip.cols[col].integral && (!impliscontinuous || !scip.cols[col].impliedintegral)
}

impl ConstraintMatrix {
    /// `getRowData`: returns the position of the row's first nonzero and
    /// its number of nonzeros.
    fn row_range(&self, rowindex: usize) -> (usize, usize) {
        let arrayposition = self.rowmatbegin[rowindex] as usize;
        let nrowvals = if rowindex as i32 == self.nrows - 1 {
            self.nnonzs as usize - arrayposition
        } else {
            self.rowmatbegin[rowindex + 1] as usize - arrayposition
        };
        (arrayposition, nrowvals)
    }

    /// `getColumnData`: returns the position of the column's first nonzero
    /// and its number of nonzeros.
    fn col_range(&self, colindex: usize) -> (usize, usize) {
        let arrayposition = self.colmatbegin[colindex] as usize;
        let ncolvals = if colindex as i32 == self.ncols - 1 {
            self.nnonzs as usize - arrayposition
        } else {
            self.colmatbegin[colindex + 1] as usize - arrayposition
        };
        (arrayposition, ncolvals)
    }
}

/// `relaxVar`: relaxes a continuous variable from all its rows, which has
/// influence on both the left and right hand side of the constraint.
fn relax_var(scip: &Scip, var: VarId, matrix: &mut ConstraintMatrix) {
    let varcol = scip.vars[var].data.col;

    // Get nonzero values and corresponding rows of variable.
    let ncolvals = scip.cols[varcol].nlprows as usize;

    let ub = scip.vars[var].glbdom.ub;
    let lb = scip.vars[var].glbdom.lb;

    debug_assert!(matrix.normalized);
    // Relax variable from all its constraints.
    for r in 0..ncolvals {
        let colrow = scip.cols[varcol].rows[r];
        let rowindex = scip.rows[colrow].lppos;

        if rowindex == -1 {
            break;
        }
        let rowindex = rowindex as usize;

        let colval = scip.cols[varcol].vals[r];

        // Variables bound influence the lhs and rhs of current row depending
        // on the sign of the variables coefficient.
        let (lhsvarbound, rhsvarbound) = if scip.set.is_feas_positive(colval) {
            (ub, lb)
        } else if scip.set.is_feas_negative(colval) {
            (lb, ub)
        } else {
            continue;
        };

        // Relax variable from the current row.
        if !scip.set.is_infinity(-matrix.lhs[rowindex]) && !scip.set.is_infinity(lhsvarbound.abs()) {
            matrix.lhs[rowindex] -= colval * lhsvarbound;
        } else {
            matrix.lhs[rowindex] = -scip.set.infinity();
        }

        if !scip.set.is_infinity(matrix.rhs[rowindex]) && !scip.set.is_infinity(rhsvarbound.abs()) {
            matrix.rhs[rowindex] -= colval * rhsvarbound;
        } else {
            matrix.rhs[rowindex] = scip.set.infinity();
        }
    }
}

/// `transformVariable`: transforms bounds of a given variable s.t. its
/// lower bound equals zero afterwards.
///
/// If the variable already has lower bound zero, the variable is not
/// transformed, if not, the variable's bounds are changed w.r.t. the
/// smaller absolute value of its bounds in order to avoid numerical
/// inaccuracies. If both lower and upper bound of the variable differ from
/// infinity, there are two cases. If |lb| <= |ub|, the bounds are shifted
/// by -lb, else a new variable ub - x replaces x. The transformation is
/// memorized by the transform status of the variable s.t. retransformation
/// is possible.
fn transform_variable(scip: &Scip, st: &mut State, colpos: usize) {
    let col = st.lpcols[colpos];
    let var = scip.cols[col].var;
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;
    let matrix = &mut st.matrix;
    let set = &scip.set;

    let mut negatecoeffs = false;
    let deltashift;
    // If both lower and upper bound are -infinity and infinity, resp., this
    // is reflected by a free transform status. If the lower bound is already
    // zero, this is reflected by identity transform status. In both cases,
    // none of the corresponding rows needs to be modified.
    if set.is_infinity(-lb) && set.is_infinity(ub) {
        if matrix.transformstatus[colpos] == TransformStatus::Neg {
            negatecoeffs = true;
        }

        deltashift = matrix.transformshiftvals[colpos];
        matrix.transformshiftvals[colpos] = 0.0;
        matrix.transformstatus[colpos] = TransformStatus::Free;
    } else if set.is_le(lb.abs(), ub.abs()) {
        matrix.transformstatus[colpos] = TransformStatus::Lb;
        deltashift = lb;
        matrix.transformshiftvals[colpos] = lb;
    } else {
        if matrix.transformstatus[colpos] != TransformStatus::Neg {
            negatecoeffs = true;
        }
        matrix.transformstatus[colpos] = TransformStatus::Neg;
        deltashift = ub;
        matrix.transformshiftvals[colpos] = ub;
    }

    // Determine the upper bound for this variable in heuristic
    // transformation (lower bound is implicit; always 0).
    if !set.is_infinity(ub) && !set.is_infinity(lb) {
        let d = ub - lb;
        matrix.upperbounds[colpos] = if d <= set.infinity() { d } else { set.infinity() };
    } else {
        matrix.upperbounds[colpos] = set.infinity();
    }

    // A real transformation is necessary. The variable x is either shifted
    // by -lb or replaced by ub - x, depending on the smaller absolute of lb
    // and ub.
    if !set.is_feas_zero(deltashift) || negatecoeffs {
        // Get nonzero values and corresponding rows of column.
        let (begin, nrows) = matrix.col_range(colpos);

        // Go through rows and modify its lhs, rhs and the variable
        // coefficient, if necessary.
        for i in begin..begin + nrows {
            let rowpos = matrix.colmatind[i] as usize;

            if !set.is_infinity(-matrix.lhs[rowpos]) {
                matrix.lhs[rowpos] -= matrix.colmatvals[i] * deltashift;
            }

            if !set.is_infinity(matrix.rhs[rowpos]) {
                matrix.rhs[rowpos] -= matrix.colmatvals[i] * deltashift;
            }

            if negatecoeffs {
                matrix.colmatvals[i] = -matrix.colmatvals[i];
            }
        }
    }
}

/// `initMatrix`: initializes copy of the original coefficient matrix and
/// applies heuristic specific adjustments: normalizing row vectors,
/// transforming variable domains such that lower bound is zero, and
/// relaxing continuous variables. Returns (nmaxrows, initialized,
/// infeasible).
fn init_matrix(scip: &mut Scip, st: &mut State, colposs: &[i32], relax: bool) -> (i32, bool, bool) {
    // Get LP row data; column data is already initialized in heurdata.
    let (lprows, nrows) = if crate::tree::tree_is_focus_node_lp_constructed(scip) {
        let lp = scip.lp.as_ref().unwrap();
        (lp.rows.0[..lp.nrows as usize].to_vec(), lp.nrows)
    } else {
        (Vec::new(), 0)
    };
    let ncols = st.nlpcols as usize;
    let impliscontinuous = st.impliscontinuous;

    st.matrix.nrows = nrows;
    st.matrix.nnonzs = 0;
    st.matrix.normalized = false;
    st.matrix.ndiscvars = 0;
    let mut nmaxrows = 0;

    // Count the number of nonzeros of the LP constraint matrix.
    for j in 0..ncols {
        let col = st.lpcols[j];
        if col_is_discrete(scip, col, impliscontinuous) {
            st.matrix.nnonzs += scip.cols[col].nlprows;
            st.matrix.ndiscvars += 1;
        }
    }

    st.matrix.ncols = st.matrix.ndiscvars;

    if st.matrix.nnonzs == 0 {
        return (nmaxrows, false, false);
    }

    // Allocate memory for the members of heuristic matrix.
    let m = &mut st.matrix;
    let (nnonzs, mnrows, mncols, ndisc) = (m.nnonzs as usize, m.nrows as usize, m.ncols as usize, m.ndiscvars as usize);
    m.rowmatvals = vec![0.0; nnonzs];
    m.rowmatind = vec![0; nnonzs];
    m.colmatvals = vec![0.0; nnonzs];
    m.colmatind = vec![0; nnonzs];
    m.rowmatbegin = vec![0; mnrows];
    m.colmatbegin = vec![0; mncols];
    m.lhs = vec![0.0; mnrows];
    m.rhs = vec![0.0; mnrows];
    m.colnorms = vec![0.0; mncols];
    m.violrows = vec![0; mncols];
    m.transformstatus = vec![TransformStatus::None; ndisc];
    m.upperbounds = vec![0.0; ndisc];
    m.transformshiftvals = vec![0.0; ndisc];

    let mut currentpointer = 0usize;
    let mut infeasible = false;

    // Initialize the rows vector of the heuristic matrix together with its
    // corresponding lhs, rhs.
    for i in 0..nrows as usize {
        // Get LP row information.
        let row = lprows[i];
        let nrowlpnonz = scip.rows[row].nlpcols as usize;
        let constant = scip.rows[row].constant;
        let rowlhs = scip.rows[row].lhs;
        let rowrhs = scip.rows[row].rhs;

        let m = &mut st.matrix;
        m.rowmatbegin[i] = currentpointer as i32;

        // Modify the lhs and rhs w.r.t to the rows constant.
        m.lhs[i] = if !scip.set.is_infinity(-rowlhs) { rowlhs - constant } else { -scip.set.infinity() };
        m.rhs[i] = if !scip.set.is_infinity(rowrhs) { rowrhs - constant } else { scip.set.infinity() };

        // In case of empty rows with a 0 < lhs <= 0.0 or 0.0 <= rhs < 0 we
        // deduce the infeasibility of the problem.
        if nrowlpnonz == 0 && (scip.set.is_feas_positive(m.lhs[i]) || scip.set.is_feas_negative(m.rhs[i])) {
            infeasible = true;
            break;
        }

        // Row coefficients are normalized and copied to heuristic matrix.
        for j in 0..nrowlpnonz {
            let col = scip.rows[row].cols[j];
            if !col_is_discrete(scip, col, impliscontinuous) {
                continue;
            }

            let m = &mut st.matrix;
            m.rowmatvals[currentpointer] = scip.rows[row].vals[j];
            m.rowmatind[currentpointer] = colposs[scip.cols[col].lppos as usize];

            currentpointer += 1;
        }
    }

    st.matrix.normalized = true;

    if infeasible {
        return (nmaxrows, false, true);
    }

    debug_assert!(currentpointer as i32 == st.matrix.nnonzs);

    currentpointer = 0;

    // Copy the nonzero coefficient data column by column to heuristic
    // matrix.
    for j in 0..st.matrix.ncols as usize {
        let currentcol = st.lpcols[j];

        let ncolnonz = scip.cols[currentcol].nlprows as usize;
        st.matrix.colnorms[j] = ncolnonz as f64;

        nmaxrows = if nmaxrows >= ncolnonz as i32 { nmaxrows } else { ncolnonz as i32 };

        // Loop over all rows with nonzero coefficients in the column,
        // transform them and add them to the heuristic matrix.
        st.matrix.colmatbegin[j] = currentpointer as i32;

        for i in 0..ncolnonz {
            let colval = scip.cols[currentcol].vals[i];
            let row = scip.cols[currentcol].rows[i];
            let mut normval = colval;

            st.matrix.colmatvals[currentpointer] = colval;
            st.matrix.colmatind[currentpointer] = scip.rows[row].lppos;

            if st.normalize {
                normval /= crate::lp::row_get_maxval(scip, row);
            }

            // Update the column norm for maximum normalized rows.
            st.matrix.colnorms[j] += if normval >= 0.0 { normval } else { -normval };
            currentpointer += 1;
        }
    }

    // Each variable is either transformed, if it supposed to be integral, or
    // relaxed.
    let nloop = if relax { ncols } else { st.matrix.ndiscvars as usize };
    for j in 0..nloop {
        let col = st.lpcols[j];
        if col_is_discrete(scip, col, impliscontinuous) {
            st.matrix.transformshiftvals[j] = 0.0;
            transform_variable(scip, st, j);
        } else {
            let var = scip.cols[col].var;
            debug_assert!(!var_is_discrete(scip, var, impliscontinuous));
            relax_var(scip, var, &mut st.matrix);
        }
    }

    (nmaxrows, true, false)
}

/// `checkRowViolation`: updates the information about a row whenever
/// violation status changes.
fn check_row_violation(scip: &Scip, st: &mut State, rowindex: usize) {
    let set = &scip.set;
    let (begin, ncols) = st.matrix.row_range(rowindex);
    let mut violadd = 0;

    // Row is now violated. Enqueue it in the set of violated rows.
    if st.violatedrowpos[rowindex] == -1 && (set.is_feas_gt(st.matrix.lhs[rowindex], 0.0) || set.is_feas_lt(st.matrix.rhs[rowindex], 0.0)) {
        let n = st.nviolatedrows as usize;
        st.violatedrows[n] = rowindex as i32;
        st.violatedrowpos[rowindex] = st.nviolatedrows;
        st.nviolatedrows += 1;
        if st.updateweights {
            st.rowweights[rowindex] += 1;
        }

        violadd = 1;
    }
    // Row is now feasible. Remove it from the set of violated rows.
    else if st.violatedrowpos[rowindex] >= 0 && set.is_feas_le(st.matrix.lhs[rowindex], 0.0) && set.is_feas_ge(st.matrix.rhs[rowindex], 0.0) {
        // Swap the row with last violated row.
        if st.violatedrowpos[rowindex] != st.nviolatedrows - 1 {
            let last = st.violatedrows[(st.nviolatedrows - 1) as usize];
            st.violatedrows[st.violatedrowpos[rowindex] as usize] = last;
            st.violatedrowpos[last as usize] = st.violatedrowpos[rowindex];
        }

        // Unlink the row from its position in the array and decrease number
        // of violated rows.
        st.violatedrowpos[rowindex] = -1;
        st.nviolatedrows -= 1;
        violadd = -1;
    }

    // Increase or decrease the column violation counter.
    for c in begin..begin + ncols {
        let col = st.matrix.rowmatind[c] as usize;
        st.matrix.violrows[col] += violadd;
    }
}

/// `checkViolations`: collects the necessary information about row
/// violations for the zero-solution. That is, all solution values in
/// heuristic transformation are zero. `colidx` is a column index for a
/// specific column, or -1 for all rows.
fn check_violations(scip: &Scip, st: &mut State, colidx: i32) {
    // Check if we requested an update for a single variable, or if we want
    // to (re)-initialize the whole violation info.
    let (begin, nrows) = if colidx >= 0 {
        st.matrix.col_range(colidx as usize)
    } else {
        let nrows = st.matrix.nrows as usize;
        st.nviolatedrows = 0;

        // Reinitialize the violated rows.
        for i in 0..nrows {
            st.violatedrowpos[i] = -1;
        }

        // Clear the violated row counters for all variables.
        for v in 0..st.matrix.ndiscvars as usize {
            st.matrix.violrows[v] = 0;
        }
        (0, nrows)
    };

    // Loop over rows and check if it is violated.
    for i in 0..nrows {
        let rowpos = if colidx >= 0 { st.matrix.colmatind[begin + i] as usize } else { i };

        // Check, if zero solution violates this row.
        check_row_violation(scip, st, rowpos);
    }
}

/// `retransformVariable`: retransforms solution values of variables
/// according to their transformation status.
fn retransform_variable(matrix: &ConstraintMatrix, varindex: usize, solvalue: f64) -> f64 {
    let status = matrix.transformstatus[varindex];
    debug_assert!(status != TransformStatus::None);

    // Check if original variable has different bounds and transform
    // solution value correspondingly.
    if status == TransformStatus::Lb {
        solvalue + matrix.transformshiftvals[varindex]
    } else if status == TransformStatus::Neg {
        matrix.transformshiftvals[varindex] - solvalue
    } else {
        solvalue
    }
}

/// `getOptimalShiftingValue`: determines the best shifting value of a
/// variable; sets `beststep` and `rowviolations` (the new weighted sum of
/// row violations, i.e, v - f).
#[allow(clippy::too_many_arguments)]
fn get_optimal_shifting_value(
    scip: &Scip,
    st: &State,
    varindex: usize,
    direction: i32,
    steps: &mut [f64],
    violationchange: &mut [i32],
    beststep: &mut f64,
    rowviolations: &mut i32,
) {
    let set = &scip.set;
    let matrix = &st.matrix;
    let upperbound = matrix.upperbounds[varindex];

    // Get nonzero values and corresponding rows of variable.
    let (begin, nrows) = matrix.col_range(varindex);

    // Loop over rows and calculate, which is the minimum shift to make this
    // row feasible or the minimum shift to violate this row.
    let mut allzero = true;
    let mut slacksurplus = 0.0;
    for i in 0..nrows {
        // Get the row data.
        let rowpos = matrix.colmatind[begin + i] as usize;
        let lhs = matrix.lhs[rowpos];
        let rhs = matrix.rhs[rowpos];
        let rowweight = st.rowweights[rowpos];
        let val = direction as f64 * matrix.colmatvals[begin + i];

        // Determine if current row is violated or not.
        let rowisviolated = set.is_feas_lt(rhs, 0.0) || set.is_feas_lt(-lhs, 0.0);

        // For a feasible row, determine the minimum integer value within the
        // bounds of the variable by which it has to be shifted to make row
        // infeasible.
        if !rowisviolated {
            let mut maxfeasshift = set.infinity();

            // Feasibility can only be violated if the variable has a lock in
            // the corresponding direction, i.e. a positive coefficient for a
            // "<="-constraint, a negative coefficient for a ">="-constraint.
            // If the variable has no lock in the current row, it can still
            // help to increase the slack of this row; we measure slack
            // increase for shifting by one.
            if val < 0.0 {
                if !set.is_infinity(-lhs) {
                    maxfeasshift = set.feas_floor((if lhs <= 0.0 { lhs } else { 0.0 }) / val);
                } else {
                    slacksurplus -= val;
                }
            } else if !set.is_infinity(rhs) {
                maxfeasshift = set.feas_floor((if rhs >= 0.0 { rhs } else { 0.0 }) / val);
            } else {
                slacksurplus += val;
            }

            // Check if the least violating shift lies within variable bounds
            // and set corresponding array values.
            if !set.is_infinity(maxfeasshift) && set.is_feas_le(maxfeasshift + 1.0, upperbound) {
                steps[i] = maxfeasshift + 1.0;
                violationchange[i] = rowweight;
                allzero = false;
            } else {
                steps[i] = upperbound;
                violationchange[i] = 0;
            }
        }
        // For a violated row, determine the minimum integral value within the
        // bounds of the variable by which it has to be shifted to make row
        // feasible.
        else {
            let mut minfeasshift = set.infinity();

            // If coefficient has the right sign to make row feasible,
            // determine the minimum integer to shift variable to obtain
            // feasibility.
            if val < 0.0 {
                if set.is_feas_lt(rhs, 0.0) {
                    minfeasshift = set.feas_ceil((if rhs <= val { rhs } else { val }) / val);
                }
            } else if set.is_feas_lt(-lhs, 0.0) {
                minfeasshift = set.feas_ceil((if lhs >= val { lhs } else { val }) / val);
            }

            // Check if the minimum feasibility recovery shift lies within
            // variable bounds and set corresponding array values.
            if !set.is_infinity(minfeasshift) && set.is_feas_le(minfeasshift, upperbound) {
                steps[i] = minfeasshift;
                violationchange[i] = -rowweight;
                allzero = false;
            } else {
                steps[i] = upperbound;
                violationchange[i] = 0;
            }
        }
    }

    // In case that the variable cannot affect the feasibility of any row, in
    // particular it cannot violate a single row, but we can add slack to
    // already feasible rows, we will do this.
    if allzero {
        if !set.is_infinity(upperbound) && set.is_gt(slacksurplus, 0.0) {
            *beststep = direction as f64 * upperbound;
        } else {
            *beststep = 0.0;
        }

        return;
    }

    // Sorts rows by increasing value of steps.
    crate::sort::sort(&mut steps[..nrows], (&mut violationchange[..nrows],), crate::sort::cmp_real);

    *beststep = 0.0;
    *rowviolations = 0;
    let mut sum = 0;

    // Best shifting step is calculated by summing up the violation changes
    // for each relevant step and taking the one which leads to the minimum
    // sum. This sum measures the balance of feasibility recovering and
    // violating changes which will be obtained by shifting the variable by
    // this step; note, the sums for smaller steps have to be taken into
    // account for all bigger steps, i.e., the sums can be computed
    // iteratively.
    let mut i = 0;
    while i < nrows && !set.is_infinity(steps[i]) {
        sum += violationchange[i];

        // If we reached the last entry for the current step value, we have
        // finished computing its sum and update the step defining the
        // minimum sum.
        if (i == nrows - 1 || steps[i + 1] > steps[i]) && sum < *rowviolations {
            *rowviolations = sum;
            *beststep = direction as f64 * steps[i];
        }
        i += 1;
    }
}

/// `updateTransformation`: updates transformation of a given variable by
/// taking into account current local bounds. If the bounds have changed
/// since last update, updating the heuristic specific upper bound of the
/// variable, its current transformed solution value and all affected rows
/// is necessary.
fn update_transformation(scip: &Scip, st: &mut State, varindex: usize, lb: f64, ub: f64) -> ScipResult {
    let set = &scip.set;

    // deltashift is the difference between the old and new transformation
    // value.
    let mut deltashift = 0.0;
    let status = st.matrix.transformstatus[varindex];

    let mut checkviolations = false;
    // Depending on the variable status, deltashift is calculated
    // differently.
    match status {
        TransformStatus::Lb => {
            if set.is_infinity(-lb) {
                transform_variable(scip, st, varindex);
                checkviolations = true;
            } else {
                let m = &mut st.matrix;
                deltashift = lb - m.transformshiftvals[varindex];
                m.transformshiftvals[varindex] = lb;
                m.upperbounds[varindex] = if !set.is_infinity(ub) { ub - lb } else { set.infinity() };
            }
        }
        TransformStatus::Neg => {
            if set.is_infinity(ub) {
                transform_variable(scip, st, varindex);
                checkviolations = true;
            } else {
                let m = &mut st.matrix;
                deltashift = m.transformshiftvals[varindex] - ub;
                m.transformshiftvals[varindex] = ub;

                if !set.is_infinity(-lb) {
                    let d = ub - lb;
                    m.upperbounds[varindex] = if d <= set.infinity() { d } else { set.infinity() };
                } else {
                    m.upperbounds[varindex] = set.infinity();
                }
            }
        }
        TransformStatus::Free => {
            // In case of a free transform status, if one of the bounds has
            // become finite, we want to transform this variable to a variable
            // with a lowerbound or a negated transform status.
            if !set.is_infinity(-lb) || !set.is_infinity(ub) {
                transform_variable(scip, st, varindex);

                // Violations have to be rechecked for rows in which variable
                // appears.
                checkviolations = true;
            }
        }
        TransformStatus::None => {
            log::error!("Error: Invalid variable status <{}> in shift and propagagate heuristic, aborting!", status as i32);
            return Err(Retcode::InvalidData);
        }
    }

    // If the bound, by which the variable was shifted, has changed,
    // deltashift is different from zero, which requires an update of all
    // affected rows.
    if !set.is_feas_zero(deltashift) {
        // Get nonzero values and corresponding rows of variable.
        let m = &mut st.matrix;
        let (begin, nrows) = m.col_range(varindex);

        // Go through rows, update the rows w.r.t. the influence of the
        // changed transformation of the variable.
        for i in begin..begin + nrows {
            let r = m.colmatind[i] as usize;
            if !set.is_infinity(-m.lhs[r]) {
                m.lhs[r] -= m.colmatvals[i] * deltashift;
            }

            if !set.is_infinity(m.rhs[r]) {
                m.rhs[r] -= m.colmatvals[i] * deltashift;
            }
        }
        checkviolations = true;
    }

    // Check and update information about violated rows, if necessary.
    if checkviolations {
        check_violations(scip, st, varindex as i32);
    }

    Ok(())
}

/// `heurSortColsShiftandpropagate`: comparison method for columns; binary <
/// integer < implicit < continuous variables.
fn heur_sort_cols(scip: &Scip, col1: ColId, col2: ColId) -> i32 {
    let var1 = scip.cols[col1].var;
    let var2 = scip.cols[col2].var;

    let vartype1 = if var_is_implied_integral(scip, var1) {
        VarType::ImplInt
    } else {
        scip.vars[var1].vartype
    } as i32;
    let vartype2 = if var_is_implied_integral(scip, var2) {
        VarType::ImplInt
    } else {
        scip.vars[var2].vartype
    } as i32;

    if vartype1 < vartype2 {
        return -1;
    }
    if vartype1 > vartype2 {
        return 1;
    }

    0
}

/*
 * Callback methods of primal heuristic
 */

/// `heurExitShiftandpropagate`: deinitialization method of primal
/// heuristic.
fn heur_exit_shiftandpropagate(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free random number generator.
    hdm(scip, heur).randnumgen = None;
    Ok(())
}

/// `heurInitShiftandpropagate`: initialization method of primal heuristic.
fn heur_init_shiftandpropagate(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    hdm(scip, heur).randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `heurCopyShiftandpropagate`: copy method for primal heuristic plugins.
fn heur_copy_shiftandpropagate(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_shiftandpropagate(scip)
}

/// `SCIPrandomPermuteIntArray` on `array[offset..]`, whose `begin` may
/// reach below `offset` as in C (pointer arithmetic on `&permutation[n]`).
fn permute_int_array(rng: &mut RandNumGen, array: &mut [i32], offset: usize, begin: i32, mut end: i32) {
    // Loop backwards through all elements and always swap the current last
    // element to a random position.
    while end > begin + 1 {
        end -= 1;

        // Get a random position into which the last entry should be
        // shuffled.
        let i = rng.get_int(begin, end);

        // Swap the last element and the random element.
        let a = (offset as i64 + i as i64) as usize;
        let b = (offset as i64 + end as i64) as usize;
        array.swap(a, b);
    }
}

/// `heurExecShiftandpropagate`: execution method of primal heuristic.
fn heur_exec_shiftandpropagate(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let eventhdlr = hd(scip, heur).eventhdlr;

    *result = ScipResultCode::DidNotRun;

    // Heuristic is obsolete if there are only continuous variables.
    if api::get_n_vars(scip) - api::get_n_cont_vars(scip) == 0 {
        return Ok(());
    }

    // Stop execution method if there is already a primarily feasible
    // solution at hand.
    if api::get_best_sol(scip) != NONE && hd(scip, heur).onlywithoutsol {
        return Ok(());
    }

    // Stop if there is no LP available.
    if !api::has_current_node_lp(scip) {
        return Ok(());
    }

    if !crate::tree::tree_is_focus_node_lp_constructed(scip) {
        // Note that this call can have the side effect that variables are
        // created.
        let cutoff = api::construct_lp(scip)?;

        // Manually cut off the node if the LP construction detected
        // infeasibility (heuristics cannot return such a result).
        if cutoff {
            let node = crate::tree::tree_get_current_node(scip);
            api::cutoff_node(scip, node)?;
            return Ok(());
        }

        api::flush_lp(scip)?;
    }

    let nlprows = api::get_n_lp_rows(scip);

    let lpcols = api::get_lp_cols(scip);
    let nlpcols = lpcols.len();

    // We need an LP.
    if nlprows == 0 || nlpcols == 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let (impliscontinuous, updateweights, normalize) = {
        let h = hd(scip, heur);
        (h.impliscontinuous, h.updateweights, h.normalize)
    };

    // Copy and sort the columns by their variable types (binary before
    // integer before implicit integer before continuous).
    let mut st = State {
        lpcols,
        nlpcols: nlpcols as i32,
        updateweights,
        normalize,
        impliscontinuous,
        ..Default::default()
    };
    crate::sort::sort(&mut st.lpcols[..], (), |&a, &b| heur_sort_cols(scip, a, b));

    let mut colposs = vec![0i32; nlpcols];

    // We have to collect the number of different variable types before we
    // start probing since during probing variable can be created (e.g.,
    // cons_xor.c).
    let mut ndiscvars = 0;
    let mut nbinvars = 0;
    for c in 0..nlpcols {
        let col = st.lpcols[c];
        let colvar = scip.cols[col].var;

        if var_is_discrete(scip, colvar, impliscontinuous) {
            ndiscvars += 1;
        }
        if scip.vars[colvar].vartype == VarType::Binary && !var_is_implied_integral(scip, colvar) {
            nbinvars += 1;
        }

        // Save the position of this column in the array such that it can be
        // accessed as the "true" column position.
        colposs[scip.cols[col].lppos as usize] = c as i32;
    }

    // Start probing mode.
    scip_probing::start_probing(scip)?;

    // Enables collection of variable statistics during probing.
    if hd(scip, heur).collectstats {
        crate::stat::stat_enable_var_history(scip);
    } else {
        crate::stat::stat_disable_var_history(scip);
    }

    scip_probing::new_probing_node(scip)?;

    // Initialize heuristic matrix and working solution.
    let relax = hd(scip, heur).relax;
    let (nmaxrows, initialized, infeasible) = init_matrix(scip, &mut st, &colposs, relax);
    put_state(scip, heur, st);

    // Could not initialize matrix; the number of discrete LP column
    // variables can be less than the actual number of variables, if, e.g.,
    // there are nonlinearities in the problem. The heuristic execution can
    // be terminated in that case.
    if initialized && !infeasible && hd(scip, heur).state.as_ref().unwrap().matrix.ndiscvars >= ndiscvars {
        exec_main(scip, heur, eventhdlr, result, ndiscvars, nbinvars, nmaxrows, nlpcols)?;
    }

    // TERMINATE: terminate probing mode and free the remaining memory.
    scip_probing::end_probing(scip)?;
    let h = hdm(scip, heur);
    h.state = None;
    h.matrix_set = false;

    Ok(())
}

/// The part of `heurExecShiftandpropagate` between a successful matrix
/// initialization and the `TERMINATE2` label (whose cleanup it runs).
#[allow(clippy::too_many_arguments)]
fn exec_main(scip: &mut Scip, heur: HeurId, eventhdlr: EventhdlrId, result: &mut ScipResultCode, ndiscvars: i32, nbinvars: i32, nmaxrows: i32, nlpcols: usize) -> ScipResult {
    let ndiscvars_u = ndiscvars as usize;
    let nbinvars_u = nbinvars as usize;
    let impliscontinuous = hd(scip, heur).impliscontinuous;

    hdm(scip, heur).matrix_set = true;

    let sol = api::create_sol(scip, heur)?;
    crate::sol::sol_set_heur(scip, sol, heur);

    // Allocate arrays for execution method.
    let mut st = take_state(scip, heur);
    let mnrows = st.matrix.nrows as usize;
    let mut permutation: Vec<i32> = (0..ndiscvars).collect();
    st.rowweights = vec![0; mnrows];

    // Allocate necessary memory for best shift search.
    let mut steps = vec![0.0f64; nmaxrows as usize];
    let mut violationchange = vec![0i32; nmaxrows as usize];

    // Allocate arrays to store information about infeasible rows.
    st.violatedrows = vec![0; mnrows];
    st.violatedrowpos = vec![0; mnrows];

    // Initialize row weights.
    for r in 0..mnrows {
        if !scip.set.is_infinity(-st.matrix.lhs[r]) && !scip.set.is_infinity(st.matrix.rhs[r]) {
            st.rowweights[r] = DEFAULT_WEIGHT_EQUALITY;
        } else {
            st.rowweights[r] = DEFAULT_WEIGHT_INEQUALITY;
        }
    }

    // Check rows for infeasibility.
    check_violations(scip, &mut st, -1);

    let (sortvars, sortkey, preferbinaries) = {
        let h = hd(scip, heur);
        (h.sortvars, h.sortkey, h.preferbinaries)
    };

    // Allocate memory for violatedvarrows array only if variable ordering
    // relies on it.
    let mut violatedvarrows: Option<Vec<i32>> = if sortvars && (sortkey == b't' || sortkey == b'v') {
        Some(st.matrix.violrows[..ndiscvars_u].to_vec())
    } else {
        None
    };

    // Sort variables w.r.t. the sorting key parameter. Sorting is indirect,
    // all matrix column data stays in place, but permutation array gives
    // access to the sorted order of variables.
    if sortvars {
        use crate::sort::{cmp_int, cmp_real, sort, sort_down};
        let nb = nbinvars_u;
        let nd = ndiscvars_u;
        match sortkey {
            b'n' => {
                // Variable ordering w.r.t. column norms nonincreasing.
                let colnorms = &mut st.matrix.colnorms;
                if preferbinaries {
                    if nb > 0 {
                        sort_down(&mut colnorms[..nb], (&mut permutation[..nb],), cmp_real);
                    }
                    if nb < nd {
                        sort_down(&mut colnorms[nb..nd], (&mut permutation[nb..nd],), cmp_real);
                    }
                } else {
                    sort_down(&mut colnorms[..nd], (&mut permutation[..nd],), cmp_real);
                }
            }
            b'u' => {
                // Variable ordering w.r.t. column norms nondecreasing.
                let colnorms = &mut st.matrix.colnorms;
                if preferbinaries {
                    if nb > 0 {
                        sort(&mut colnorms[..nb], (&mut permutation[..nb],), cmp_real);
                    }
                    if nb < nd {
                        sort(&mut colnorms[nb..nd], (&mut permutation[nb..nd],), cmp_real);
                    }
                } else {
                    sort(&mut colnorms[..nd], (&mut permutation[..nd],), cmp_real);
                }
            }
            b'v' => {
                // Variable ordering w.r.t. nonincreasing number of violated
                // rows.
                let vvr = violatedvarrows.as_mut().unwrap();
                if preferbinaries {
                    if nb > 0 {
                        sort_down(&mut vvr[..nb], (&mut permutation[..nb],), cmp_int);
                    }
                    if nb < nd {
                        sort_down(&mut vvr[nb..nd], (&mut permutation[nb..nd],), cmp_int);
                    }
                } else {
                    sort_down(&mut vvr[..nd], (&mut permutation[..nd],), cmp_int);
                }
            }
            b't' => {
                // Variable ordering w.r.t. nondecreasing number of violated
                // rows.
                let vvr = violatedvarrows.as_mut().unwrap();
                if preferbinaries {
                    if nb > 0 {
                        sort(&mut vvr[..nb], (&mut permutation[..nb],), cmp_int);
                    }
                    if nb < nd {
                        sort(&mut vvr[nb..nd], (&mut permutation[nb..nd],), cmp_int);
                    }
                } else {
                    sort(&mut vvr[..nd], (&mut permutation[..nd],), cmp_int);
                }
            }
            b'r' => {
                // Random sorting.
                let rng = hdm(scip, heur).randnumgen.as_mut().unwrap();
                if preferbinaries {
                    if nb > 0 {
                        permute_int_array(rng, &mut permutation, 0, 0, nbinvars - 1);
                    }
                    if nb < nd {
                        permute_int_array(rng, &mut permutation, nb, nbinvars - 1, ndiscvars - nbinvars - 1);
                    }
                } else {
                    permute_int_array(rng, &mut permutation, 0, 0, ndiscvars - 1);
                }
            }
            _ => {}
        }
    }
    drop(violatedvarrows);

    // Should binary variables without locks be treated first?
    if hd(scip, heur).binlocksfirst {
        let nolocks = |scip: &Scip, var: VarId| var_get_n_locks_up_type(scip, var, LockType::Model) == 0 || var_get_n_locks_down_type(scip, var, LockType::Model) == 0;
        let mut nbinwithoutlocks = 0;

        // Count number of binaries without locks.
        if preferbinaries {
            for c in 0..nbinvars_u {
                let var = scip.cols[st.lpcols[permutation[c] as usize]].var;
                if nolocks(scip, var) {
                    nbinwithoutlocks += 1;
                }
            }
        } else {
            for c in 0..ndiscvars_u {
                let var = scip.cols[st.lpcols[permutation[c] as usize]].var;
                if var_is_binary(scip, var) && nolocks(scip, var) {
                    nbinwithoutlocks += 1;
                }
            }
        }

        if nbinwithoutlocks > 0 {
            let mut b = 1usize;
            let mut c = 0usize;
            let colvar = |scip: &Scip, st: &State, p: i32| scip.cols[st.lpcols[p as usize]].var;

            // If c reaches nbinwithoutlocks, then all binary variables
            // without locks were sorted to the beginning of the array.
            while c < nbinwithoutlocks && b < ndiscvars_u {
                let mut var = colvar(scip, &st, permutation[c]);
                let mut binvar = colvar(scip, &st, permutation[b]);

                // Search for next variable which is not a binary variable
                // without locks.
                while var_is_binary(scip, var) && nolocks(scip, var) {
                    c += 1;
                    if c >= nbinwithoutlocks {
                        break;
                    }
                    var = colvar(scip, &st, permutation[c]);
                }
                if c >= nbinwithoutlocks {
                    break;
                }

                // Search for next binary variable without locks (with
                // position > c).
                if b <= c {
                    b = c + 1;
                    binvar = colvar(scip, &st, permutation[b]);
                }
                while !var_is_binary(scip, binvar) || (var_get_n_locks_up_type(scip, binvar, LockType::Model) > 0 && var_get_n_locks_down_type(scip, binvar, LockType::Model) > 0) {
                    b += 1;
                    binvar = colvar(scip, &st, permutation[b]);
                }

                // Swap the two variables.
                permutation.swap(b, c);

                // Increase counters.
                c += 1;
                b += 1;
            }
        }
    }

    // Initialize variable events to catch bound changes during propagation.
    let ndiscmatrix = st.matrix.ndiscvars as usize;
    let discvars: Vec<VarId> = (0..ndiscmatrix).map(|c| scip.cols[st.lpcols[c]].var).collect();
    put_state(scip, heur, st);
    for (c, &var) in discvars.iter().enumerate() {
        api::catch_var_event(scip, var, EVENTTYPE_SHIFTANDPROPAGATE, eventhdlr, c, None)?;
    }

    let mut cutoff = false;

    let mut lastindexofsusp: i32 = -1;
    let mut probing = hd(scip, heur).probing;
    let mut ncutoffs = 0;
    let mut nprobings = 0;

    let (heurprobing, fixbinlocks, stopafterfeasible, nozerofixing, selectbest, nproprounds, cutoffbreaker, maxcutoffquot) = {
        let h = hd(scip, heur);
        (
            h.probing,
            h.fixbinlocks,
            h.stopafterfeasible,
            h.nozerofixing,
            h.selectbest,
            h.nproprounds,
            h.cutoffbreaker,
            h.maxcutoffquot,
        )
    };

    'terminate2: {
        // Loop over variables, shift them according to shifting criteria and
        // try to reduce the global infeasibility.
        for c in 0..ndiscvars_u {
            if selectbest {
                // Search for best candidate.
                let st = hd(scip, heur).state.as_ref().unwrap();
                let mut j = c + 1;
                let mut perm = permutation.clone();
                while j < ndiscvars_u {
                    // Run through remaining variables and search for best
                    // candidate.
                    if st.matrix.violrows[perm[c] as usize] < st.matrix.violrows[perm[j] as usize] {
                        perm.swap(c, j);
                    }
                    j += 1;
                }
                permutation = perm;
            }
            let permutedvarindex = permutation[c] as usize;
            let mut optimalshiftvalue = 0.0;
            let mut nviolations = 0;
            let var = scip.cols[hd(scip, heur).state.as_ref().unwrap().lpcols[permutedvarindex]].var;
            let lb = scip.vars[var].locdom.lb;
            let ub = scip.vars[var].locdom.ub;

            // Check whether we hit some limit, e.g. the time limit, in
            // between; since the check itself consumes some time, we only do
            // it every tenth iteration.
            if c % 10 == 0 && crate::solve::solve_is_stopped(scip, false) {
                break 'terminate2;
            }

            // If propagation is enabled, check if propagation has changed the
            // variables bounds and update the transformed upper bound
            // correspondingly.
            if heurprobing {
                let mut st = take_state(scip, heur);
                let r = update_transformation(scip, &mut st, permutedvarindex, lb, ub);
                put_state(scip, heur, st);
                r?;
            }

            // Ignore variable if propagation fixed it (lb and ub will be
            // zero).
            if scip.set.is_feas_zero(hd(scip, heur).state.as_ref().unwrap().matrix.upperbounds[permutedvarindex]) {
                api::set_sol_val(scip, sol, var, ub)?;

                continue;
            }

            let mut marksuspicious = false;
            let origsolval;

            // Check whether the variable is binary and has no locks in one
            // direction, so that we want to fix it to the respective bound
            // (only enabled by parameter).
            if fixbinlocks && var_is_binary(scip, var) && (var_get_n_locks_up_type(scip, var, LockType::Model) == 0 || var_get_n_locks_down_type(scip, var, LockType::Model) == 0) {
                if var_get_n_locks_up_type(scip, var, LockType::Model) == 0 {
                    origsolval = scip.vars[var].locdom.ub;
                } else {
                    origsolval = scip.vars[var].locdom.lb;
                }
            } else {
                let st = hd(scip, heur).state.as_ref().unwrap();
                // Only apply the computationally expensive best shift
                // selection, if there is a violated row left.
                if !stopafterfeasible || st.nviolatedrows > 0 {
                    // Compute optimal shift value for variable.
                    get_optimal_shifting_value(scip, st, permutedvarindex, 1, &mut steps, &mut violationchange, &mut optimalshiftvalue, &mut nviolations);

                    // Variables with FREE transform have to be dealt with
                    // twice.
                    if st.matrix.transformstatus[permutedvarindex] == TransformStatus::Free {
                        let mut downshiftvalue = 0.0;
                        let mut ndownviolations = 0;
                        get_optimal_shifting_value(scip, st, permutedvarindex, -1, &mut steps, &mut violationchange, &mut downshiftvalue, &mut ndownviolations);

                        // Compare to positive direction and select the
                        // direction which makes more rows feasible.
                        if ndownviolations < nviolations {
                            optimalshiftvalue = downshiftvalue;
                        }
                    }
                } else {
                    optimalshiftvalue = 0.0;
                }

                // If zero optimal shift values are forbidden by the user
                // parameter, delay the variable by marking it suspicious.
                if nozerofixing && nviolations > 0 && scip.set.is_feas_zero(optimalshiftvalue) {
                    marksuspicious = true;
                }

                // Retransform the solution value from the heuristic
                // transformation space.
                debug_assert!(var_is_discrete(scip, var, impliscontinuous));
                origsolval = retransform_variable(&st.matrix, permutedvarindex, optimalshiftvalue);
            }

            // Check if propagation should still be performed.
            if nprobings > DEFAULT_PROPBREAKER {
                probing = false;
            }

            // If propagation is enabled, fix the variable to the new solution
            // value and propagate the fixation (to fix other variables and to
            // find out early whether solution is already infeasible).
            if !marksuspicious && probing {
                scip_probing::new_probing_node(scip)?;
                scip_probing::fix_var_probing(scip, var, origsolval)?;

                (cutoff, _) = scip_probing::propagate_probing(scip, nproprounds)?;

                nprobings += 1;
            }

            // Propagation led to an empty domain, hence we backtrack and
            // postpone the variable.
            if cutoff {
                ncutoffs += 1;

                // Only continue heuristic if number of cutoffs occured so far
                // is reasonably small.
                if cutoffbreaker >= 0 && ncutoffs as f64 >= (maxcutoffquot * scip_probing::get_probing_depth(scip) as f64) + cutoffbreaker as f64 {
                    break;
                }

                cutoff = false;

                // Backtrack to the parent of the current node.
                let depth = scip_probing::get_probing_depth(scip);
                scip_probing::backtrack_probing(scip, depth - 1)?;

                // If the variable upper and lower bound are equal to the
                // solution value to which we tried to fix the variable, we are
                // trapped at an infeasible node and break; this can only
                // happen due to an intermediate global bound change of the
                // variable, I guess.
                let (vlb, vub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
                if scip.set.is_feas_eq(vub, origsolval) && scip.set.is_feas_eq(vlb, origsolval) {
                    cutoff = true;
                    break;
                } else if scip.set.is_feas_eq(vlb, origsolval) && origsolval.abs() < 1.0 / scip.set.epsilon() {
                    // If the variable was set to one of its bounds,
                    // repropagate by tightening this bound by 1.0 into the
                    // direction of the other bound, if possible; if the bound
                    // is too large (in abs value) do not even bother.
                    scip_probing::new_probing_node(scip)?;
                    scip_probing::chg_var_lb_probing(scip, var, origsolval + 1.0)?;
                    (cutoff, _) = scip_probing::propagate_probing(scip, nproprounds)?;
                } else if scip.set.is_feas_eq(vub, origsolval) && origsolval.abs() < 1.0 / scip.set.epsilon() {
                    // If the variable was set to one of its bounds,
                    // repropagate by tightening this bound by 1.0 into the
                    // direction of the other bound, if possible; if the bound
                    // is too large (in abs value) do not even bother.
                    scip_probing::new_probing_node(scip)?;
                    scip_probing::chg_var_ub_probing(scip, var, origsolval - 1.0)?;
                    (cutoff, _) = scip_probing::propagate_probing(scip, nproprounds)?;
                }

                // If the tightened bound again leads to a cutoff, both
                // subproblems are proven infeasible and the heuristic can be
                // stopped.
                if cutoff {
                    break;
                } else {
                    // Since repropagation was successful, we indicate that
                    // this variable led to a cutoff in one direction.
                    marksuspicious = true;
                }
            }

            if marksuspicious {
                // Mark the variable as suspicious.
                lastindexofsusp += 1;

                permutation[c] = permutation[lastindexofsusp as usize];
                permutation[lastindexofsusp as usize] = permutedvarindex as i32;
            } else {
                // Update solution.
                api::set_sol_val(scip, sol, var, origsolval)?;

                // Only to ensure that some assertions can be made later on.
                if !probing {
                    scip_probing::fix_var_probing(scip, var, origsolval)?;
                }
            }
        }

        // If constructed solution might be feasible, go through the queue of
        // suspicious variables and set the solution values.
        if hd(scip, heur).state.as_ref().unwrap().nviolatedrows == 0 && !cutoff {
            for v in 0..=lastindexofsusp {
                // Get the column position of the variable.
                let permutedvarindex = permutation[v as usize] as usize;
                let var = scip.cols[hd(scip, heur).state.as_ref().unwrap().lpcols[permutedvarindex]].var;

                // Update the transformation of the variable, since the bound
                // might have changed after the last update.
                if heurprobing {
                    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
                    let mut st = take_state(scip, heur);
                    let r = update_transformation(scip, &mut st, permutedvarindex, lb, ub);
                    put_state(scip, heur, st);
                    r?;
                }

                // Retransform the solution value from the heuristic
                // transformed space, set the solution value accordingly.
                let origsolval = retransform_variable(&hd(scip, heur).state.as_ref().unwrap().matrix, permutedvarindex, 0.0);
                api::set_sol_val(scip, sol, var, origsolval)?;
                // Only to ensure that some assertions can be made later.
                scip_probing::fix_var_probing(scip, var, origsolval)?;
            }

            // Fixing of remaining variables led to infeasibility.
            if hd(scip, heur).state.as_ref().unwrap().nviolatedrows > 0 {
                break 'terminate2;
            }

            let mut trysol = true;
            let solvelp;

            // Check if enough variables have been fixed (including
            // continuous) to solve the remaining LP.
            if nlpcols as i32 != hd(scip, heur).state.as_ref().unwrap().matrix.ndiscvars {
                let vars = api::get_vars(scip).to_vec();
                let nvars = api::get_n_vars(scip) as usize;
                let nminfixings = scip.set.ceil(hd(scip, heur).minfixingratelp * nvars as f64) as i32;
                let mut nfixedvars = ndiscvars;

                // Count fixed variables.
                let mut v = ndiscvars_u;
                while v < nvars && nfixedvars < nminfixings {
                    if scip.set.is_eq(scip.vars[vars[v]].locdom.lb, scip.vars[vars[v]].locdom.ub) {
                        nfixedvars += 1;
                    }
                    v += 1;
                }

                solvelp = nfixedvars >= nminfixings;
                trysol = solvelp;
            } else {
                // No need to solve an LP.
                solvelp = false;
            }

            // If the constructed solution might still be extendable to a
            // feasible solution, try this by solving the remaining LP.
            if solvelp {
                // Solve LP; errors in the LP solver should not kill the
                // overall solving process, if the LP is just needed for a
                // heuristic. Hence in optimized mode, the return code is
                // caught and a warning is printed.
                let lperror = match scip_probing::solve_probing_lp(scip, -1) {
                    Ok((e, _)) => e,
                    Err(retstat) => {
                        log::warn!("Error while solving LP in SHIFTANDPROPAGATE heuristic; LP solve terminated with code <{retstat:?}>");
                        false
                    }
                };

                // Check if this is a feasible solution.
                if !lperror && api::get_lp_solstat(scip) == LpSolStat::Optimal {
                    // Copy the current LP solution to the working solution.
                    crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;
                } else {
                    trysol = false;
                }
            }

            // Check solution for feasibility, and add it to solution store if
            // possible. None of integrality, feasibility of LP rows, variable
            // bounds have to be checked, because they are guaranteed by the
            // heuristic at this stage.
            if trysol {
                // We once also checked the variable bounds which should not
                // be necessary.
                let stored = api::try_sol(scip, sol, false, false, false, false, false)?;

                if stored {
                    *result = ScipResultCode::FoundSol;
                }
            }
        }
    }

    // TERMINATE2: free allocated memory in reverse order of allocation.
    for c in (0..discvars.len()).rev() {
        api::drop_var_event(scip, discvars[c], EVENTTYPE_SHIFTANDPROPAGATE, eventhdlr, c, -1)?;
    }

    api::free_sol(scip, sol)?;

    Ok(())
}

/// `eventExecShiftandpropagate`: event handler execution method for the
/// heuristic which catches all events in which a lower or upper bound were
/// tightened.
fn event_exec_shiftandpropagate(scip: &mut Scip, eventhdlr: EventhdlrId, _event: &Event, eventdata: usize) -> ScipResult {
    let heur: HeurId = *scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref::<HeurId>().unwrap();
    debug_assert!(hd(scip, heur).matrix_set);

    let colpos = eventdata;

    let mut st = take_state(scip, heur);
    let col = st.lpcols[colpos];
    let var = scip.cols[col].var;

    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    let r = update_transformation(scip, &mut st, colpos, lb, ub);
    put_state(scip, heur, st);
    r
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurShiftandpropagate`: creates the shiftandpropagate primal
/// heuristic and includes it in SCIP.
pub fn include_heur_shiftandpropagate(scip: &mut Scip) -> ScipResult {
    // The event handler's data (the heuristic) is set once the heuristic is
    // included.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_shiftandpropagate, None)?;

    // Include primal heuristic.
    let heur = api::include_heur_basic(
        scip,
        HEUR_NAME,
        HEUR_DESC,
        HEUR_DISPCHAR,
        HEUR_PRIORITY,
        HEUR_FREQ,
        HEUR_FREQOFS,
        HEUR_MAXDEPTH,
        HEUR_TIMING,
        HEUR_USESSUBSCIP,
        heur_exec_shiftandpropagate,
        Some(Box::new(HeurData {
            randnumgen: None,
            relax: DEFAULT_RELAX,
            probing: DEFAULT_PROBING,
            onlywithoutsol: DEFAULT_ONLYWITHOUTSOL,
            nproprounds: DEFAULT_NPROPROUNDS,
            cutoffbreaker: DEFAULT_CUTOFFBREAKER,
            eventhdlr,
            maxcutoffquot: DEFAULT_MAXCUTOFFQUOT,
            minfixingratelp: DEFAULT_MINFIXINGRATELP,
            sortkey: DEFAULT_SORTKEY,
            sortvars: DEFAULT_SORTVARS,
            collectstats: DEFAULT_COLLECTSTATS,
            stopafterfeasible: DEFAULT_STOPAFTERFEASIBLE,
            preferbinaries: DEFAULT_PREFERBINARIES,
            nozerofixing: DEFAULT_NOZEROFIXING,
            fixbinlocks: DEFAULT_FIXBINLOCKS,
            binlocksfirst: DEFAULT_BINLOCKSFIRST,
            normalize: DEFAULT_NORMALIZE,
            updateweights: DEFAULT_UPDATEWEIGHTS,
            impliscontinuous: DEFAULT_IMPLISCONTINUOUS,
            selectbest: DEFAULT_SELECTBEST,
            state: None,
            matrix_set: false,
        }) as Box<dyn Any>),
    )?;
    scip.eventhdlrs[eventhdlr].eventhdlrdata = Some(Box::new(heur) as Box<dyn Any>);

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_shiftandpropagate);
    h.heurinit = Some(heur_init_shiftandpropagate);
    h.heurexit = Some(heur_exit_shiftandpropagate);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add shiftandpropagate primal heuristic parameters.
    use crate::paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_real};
    paramset_add_int(
        scip,
        "heuristics/shiftandpropagate/nproprounds",
        "The number of propagation rounds used for each propagation",
        t!(i32, nproprounds),
        true,
        DEFAULT_NPROPROUNDS,
        -1,
        1000,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/relax",
        "Should continuous variables be relaxed?",
        t!(bool, relax),
        true,
        DEFAULT_RELAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/probing",
        "Should domains be reduced by probing?",
        t!(bool, probing),
        true,
        DEFAULT_PROBING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/onlywithoutsol",
        "Should heuristic only be executed if no primal solution was found, yet?",
        t!(bool, onlywithoutsol),
        true,
        DEFAULT_ONLYWITHOUTSOL,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/shiftandpropagate/cutoffbreaker",
        "The number of cutoffs before heuristic stops",
        t!(i32, cutoffbreaker),
        true,
        DEFAULT_CUTOFFBREAKER,
        -1,
        1000000,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/shiftandpropagate/sortkey",
        "the key for variable sorting: (n)orms down, norms (u)p, (v)iolations down, viola(t)ions up, or (r)andom",
        t!(u8, sortkey),
        true,
        DEFAULT_SORTKEY,
        Some(SORTKEYS),
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/sortvars",
        "Should variables be sorted for the heuristic?",
        t!(bool, sortvars),
        true,
        DEFAULT_SORTVARS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/collectstats",
        "should variable statistics be collected during probing?",
        t!(bool, collectstats),
        true,
        DEFAULT_COLLECTSTATS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/stopafterfeasible",
        "Should the heuristic stop calculating optimal shift values when no more rows are violated?",
        t!(bool, stopafterfeasible),
        true,
        DEFAULT_STOPAFTERFEASIBLE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/preferbinaries",
        "Should binary variables be shifted first?",
        t!(bool, preferbinaries),
        true,
        DEFAULT_PREFERBINARIES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/nozerofixing",
        "should variables with a zero shifting value be delayed instead of being fixed?",
        t!(bool, nozerofixing),
        true,
        DEFAULT_NOZEROFIXING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/fixbinlocks",
        "should binary variables with no locks in one direction be fixed to that direction?",
        t!(bool, fixbinlocks),
        true,
        DEFAULT_FIXBINLOCKS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/binlocksfirst",
        "should binary variables with no locks be preferred in the ordering?",
        t!(bool, binlocksfirst),
        true,
        DEFAULT_BINLOCKSFIRST,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/normalize",
        "should coefficients be normalized by max row coeff for col norm?",
        t!(bool, normalize),
        true,
        DEFAULT_NORMALIZE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/updateweights",
        "should row weight be increased every time the row is violated?",
        t!(bool, updateweights),
        true,
        DEFAULT_UPDATEWEIGHTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/impliscontinuous",
        "should implicit integer variables be treated as continuous variables?",
        t!(bool, impliscontinuous),
        true,
        DEFAULT_IMPLISCONTINUOUS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/shiftandpropagate/selectbest",
        "should the heuristic choose the best candidate in every round? (set to FALSE for static order)?",
        t!(bool, selectbest),
        true,
        DEFAULT_SELECTBEST,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/shiftandpropagate/maxcutoffquot",
        "maximum percentage of allowed cutoffs before stopping the heuristic",
        t!(f64, maxcutoffquot),
        true,
        DEFAULT_MAXCUTOFFQUOT,
        0.0,
        2.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/shiftandpropagate/minfixingratelp",
        "minimum fixing rate over all variables (including continuous) to solve LP",
        t!(f64, minfixingratelp),
        true,
        DEFAULT_MINFIXINGRATELP,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
