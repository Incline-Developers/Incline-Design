//! HiGHS's `HPresolve` (`presolve/HPresolve.cpp`), the LP presolve: the
//! reductions it applies to a continuous LP whose postsolve must give a
//! basis, in the order HiGHS applies them. The MIP-only reductions
//! (probing, clique and implied-integer detection, sparsification,
//! dominated columns and coefficient strengthening) are left out, as are
//! HiGHS's options for logging and switching rules off. So is the
//! dominated (forcing) column rule 1.15 applied: its basis postsolve is
//! wrong, and HiGHS has used it only without one since #3348.

use std::{
    collections::{BTreeSet, HashMap},
    ops::Bound::{Excluded, Unbounded},
};

use super::{
    NONE,
    matrix::{self, Trees},
    max, min,
    postsolve::{PostsolveStack, RowType},
    sum_bounds::{Bounds, SumBounds},
};
use crate::{
    pdqsort::pdqsort,
    util::{CDouble, hash},
};

/// HiGHS's `kHighsTiny`.
const TINY: f64 = 1e-14;
const INF: f64 = f64::INFINITY;

/// An LP as presolve sees it: minimised, column-wise.
#[derive(Clone, Debug, Default)]
pub(crate) struct Lp {
    pub num_col: usize,
    pub num_row: usize,
    pub col_cost: Vec<f64>,
    pub col_lower: Vec<f64>,
    pub col_upper: Vec<f64>,
    pub row_lower: Vec<f64>,
    pub row_upper: Vec<f64>,
    pub offset: f64,
    pub start: Vec<usize>,
    pub index: Vec<usize>,
    pub value: Vec<f64>,
}

/// HiGHS's `HPresolve::Result`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Ok,
    PrimalInfeasible,
    DualInfeasible,
    Stopped,
}

/// HiGHS's `HighsPresolveStatus`, as far as LP presolve sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PresolveStatus {
    NotReduced,
    Reduced,
    ReducedToEmpty,
    Infeasible,
    UnboundedOrInfeasible,
    /// Stopped at the time limit (`kTimeout`).
    Timeout,
}

/// The options presolve reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    pub primal_feasibility_tolerance: f64,
    pub dual_feasibility_tolerance: f64,
    pub small_matrix_value: f64,
    /// HiGHS's `presolve_pivot_threshold`.
    pub pivot_threshold: f64,
    /// HiGHS's `presolve_substitution_maxfillin`.
    pub max_fillin: i64,
    /// The solve's time limit, in seconds.
    pub time_limit: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            primal_feasibility_tolerance: 1e-7,
            dual_feasibility_tolerance: 1e-7,
            small_matrix_value: 1e-9,
            pivot_threshold: 0.01,
            max_fillin: 10,
            time_limit: f64::INFINITY,
        }
    }
}

macro_rules! check {
    ($call:expr) => {
        match $call {
            Outcome::Ok => {}
            other => return other,
        }
    };
}

/// The column bounds row activity bounds are over.
macro_rules! col_bounds {
    ($s:ident) => {
        Bounds {
            lower: &$s.model.col_lower,
            upper: &$s.model.col_upper,
            impl_lower: &$s.impl_col_lower,
            impl_upper: &$s.impl_col_upper,
            impl_lower_source: &$s.col_lower_source,
            impl_upper_source: &$s.col_upper_source,
        }
    };
}

/// The row dual bounds column dual activity bounds are over.
macro_rules! dual_bounds {
    ($s:ident) => {
        Bounds {
            lower: &$s.row_dual_lower,
            upper: &$s.row_dual_upper,
            impl_lower: &$s.impl_row_dual_lower,
            impl_upper: &$s.impl_row_dual_upper,
            impl_lower_source: &$s.row_dual_lower_source,
            impl_upper_source: &$s.row_dual_upper_source,
        }
    };
}

macro_rules! trees {
    ($s:ident) => {
        Trees {
            left: &mut $s.ar_left,
            right: &mut $s.ar_right,
            key: &$s.a_col,
        }
    };
}

pub(crate) struct Presolve {
    pub model: Lp,
    settings: Settings,
    /// When presolve began, for its time limit.
    started: web_time::Instant,
    primal_feastol: f64,

    a_value: Vec<f64>,
    a_row: Vec<usize>,
    a_col: Vec<usize>,
    colhead: Vec<usize>,
    a_next: Vec<usize>,
    a_prev: Vec<usize>,
    rowroot: Vec<usize>,
    ar_left: Vec<usize>,
    ar_right: Vec<usize>,
    rowsize: Vec<usize>,
    colsize: Vec<usize>,
    rowpositions: Vec<usize>,
    freeslots: Vec<usize>,

    impl_col_lower: Vec<f64>,
    impl_col_upper: Vec<f64>,
    col_lower_source: Vec<usize>,
    col_upper_source: Vec<usize>,
    row_dual_lower: Vec<f64>,
    row_dual_upper: Vec<f64>,
    impl_row_dual_lower: Vec<f64>,
    impl_row_dual_upper: Vec<f64>,
    row_dual_lower_source: Vec<usize>,
    row_dual_upper_source: Vec<usize>,
    col_impl_source_by_row: Vec<BTreeSet<usize>>,
    impl_row_dual_source_by_col: Vec<BTreeSet<usize>>,
    implied_row_bounds: SumBounds,
    implied_dual_row_bounds: SumBounds,

    changed_row_indices: Vec<usize>,
    changed_row_flag: Vec<bool>,
    changed_col_indices: Vec<usize>,
    changed_col_flag: Vec<bool>,
    /// Row and column pairs that may allow a free column substitution;
    /// a row of `NONE` marks one used up.
    substitution_opportunities: Vec<(usize, usize)>,
    /// The equations by size, then row.
    equations: BTreeSet<(usize, usize)>,
    /// The size each equation is filed under in `equations`, if it is.
    eq_key: Vec<Option<usize>>,
    shrink_problem_enabled: bool,
    reduction_limit: usize,
    singleton_rows: Vec<usize>,
    singleton_columns: Vec<usize>,
    row_deleted: Vec<bool>,
    col_deleted: Vec<bool>,
    single_equation_checked: Vec<bool>,
    /// How many rows are flagged in `single_equation_checked` (HiGHS #3341).
    num_single_equation_checked: usize,
    num_deleted_rows: usize,
    num_deleted_cols: usize,
    old_num_col: usize,
    old_num_row: usize,
    /// Scratch for a row's positions.
    row_buffer: Vec<usize>,
}

impl Presolve {
    /// HiGHS's `okSetInput` for an LP.
    pub fn new(model: Lp, settings: Settings) -> Self {
        let num_col = model.num_col;
        let num_row = model.num_row;
        let mut row_dual_lower = vec![-INF; num_row];
        let mut row_dual_upper = vec![INF; num_row];
        for i in 0..num_row {
            if model.row_lower[i] == -INF {
                row_dual_upper[i] = 0.0;
            }
            if model.row_upper[i] == INF {
                row_dual_lower[i] = 0.0;
            }
        }
        let mut presolve = Presolve {
            model,
            settings,
            started: web_time::Instant::now(),
            primal_feastol: settings.primal_feasibility_tolerance,
            a_value: Vec::new(),
            a_row: Vec::new(),
            a_col: Vec::new(),
            colhead: Vec::new(),
            a_next: Vec::new(),
            a_prev: Vec::new(),
            rowroot: Vec::new(),
            ar_left: Vec::new(),
            ar_right: Vec::new(),
            rowsize: Vec::new(),
            colsize: Vec::new(),
            rowpositions: Vec::new(),
            freeslots: Vec::new(),
            impl_col_lower: vec![-INF; num_col],
            impl_col_upper: vec![INF; num_col],
            col_lower_source: vec![NONE; num_col],
            col_upper_source: vec![NONE; num_col],
            row_dual_lower,
            row_dual_upper,
            impl_row_dual_lower: vec![-INF; num_row],
            impl_row_dual_upper: vec![INF; num_row],
            row_dual_lower_source: vec![NONE; num_row],
            row_dual_upper_source: vec![NONE; num_row],
            col_impl_source_by_row: vec![BTreeSet::new(); num_row],
            impl_row_dual_source_by_col: vec![BTreeSet::new(); num_col],
            implied_row_bounds: SumBounds::default(),
            implied_dual_row_bounds: SumBounds::default(),
            changed_row_indices: Vec::with_capacity(num_row),
            changed_row_flag: vec![true; num_row],
            changed_col_indices: Vec::with_capacity(num_col),
            changed_col_flag: vec![true; num_col],
            substitution_opportunities: Vec::new(),
            equations: BTreeSet::new(),
            eq_key: Vec::new(),
            shrink_problem_enabled: true,
            reduction_limit: usize::MAX,
            singleton_rows: Vec::new(),
            singleton_columns: Vec::new(),
            row_deleted: vec![false; num_row],
            col_deleted: vec![false; num_col],
            single_equation_checked: vec![false; num_row],
            num_single_equation_checked: 0,
            num_deleted_rows: 0,
            num_deleted_cols: 0,
            old_num_col: 0,
            old_num_row: 0,
            row_buffer: Vec::new(),
        };
        let (start, index, value) = (
            std::mem::take(&mut presolve.model.start),
            std::mem::take(&mut presolve.model.index),
            std::mem::take(&mut presolve.model.value),
        );
        presolve.load_csc(&value, &index, &start);
        for row in 0..num_row {
            if !presolve.is_dual_implied_free(row) {
                continue;
            }
            for pos in presolve.row_vector(row) {
                if presolve.is_implied_free(presolve.a_col[pos]) {
                    presolve.substitution_opportunities.push((row, presolve.a_col[pos]));
                }
            }
        }
        presolve
    }

    // ---- Queries -------------------------------------------------------

    fn is_lower_implied(&self, col: usize) -> bool {
        self.model.col_lower[col] == -INF || self.impl_col_lower[col] >= self.model.col_lower[col] - self.primal_feastol
    }

    fn is_lower_strictly_implied(&self, col: usize, tolerance: f64) -> bool {
        self.model.col_lower[col] == -INF || self.impl_col_lower[col] > self.model.col_lower[col] + tolerance
    }

    fn is_upper_implied(&self, col: usize) -> bool {
        self.model.col_upper[col] == INF || self.impl_col_upper[col] <= self.model.col_upper[col] + self.primal_feastol
    }

    fn is_upper_strictly_implied(&self, col: usize, tolerance: f64) -> bool {
        self.model.col_upper[col] == INF || self.impl_col_upper[col] < self.model.col_upper[col] - tolerance
    }

    fn is_implied_free(&self, col: usize) -> bool {
        self.is_lower_implied(col) && self.is_upper_implied(col)
    }

    fn is_dual_implied_free(&self, row: usize) -> bool {
        let tol = self.settings.dual_feasibility_tolerance;
        self.is_equation(row)
            || (self.model.row_upper[row] != INF && self.impl_row_dual_upper[row] <= tol)
            || (self.model.row_lower[row] != -INF && self.impl_row_dual_lower[row] >= -tol)
    }

    fn dual_implied_free_get_rhs_and_row_type(&mut self, row: usize, relax_row_dual_bounds: bool) -> (f64, RowType) {
        let tol = self.settings.dual_feasibility_tolerance;
        if self.is_equation(row) {
            (self.model.row_upper[row], RowType::Eq)
        } else if self.model.row_upper[row] != INF && self.impl_row_dual_upper[row] <= tol {
            let rhs = self.model.row_upper[row];
            if relax_row_dual_bounds {
                self.change_row_dual_upper(row, INF);
            }
            (rhs, RowType::Leq)
        } else {
            let rhs = self.model.row_lower[row];
            if relax_row_dual_bounds {
                self.change_row_dual_lower(row, -INF);
            }
            (rhs, RowType::Geq)
        }
    }

    fn is_equation(&self, row: usize) -> bool {
        self.model.row_lower[row] == self.model.row_upper[row]
    }

    fn is_ranged(&self, row: usize) -> bool {
        self.model.row_lower[row] != -INF && self.model.row_upper[row] != INF
    }

    fn is_redundant(&self, row: usize) -> bool {
        self.implied_row_bounds.sum_lower(row, CDouble::default()) >= self.model.row_lower[row] - self.primal_feastol
            && self.implied_row_bounds.sum_upper(row, CDouble::default()) <= self.model.row_upper[row] + self.primal_feastol
    }

    fn yields_implied_lower_bound(&self, row: usize, val: f64) -> bool {
        (val < 0.0 && self.model.row_upper[row] != INF) || (val > 0.0 && self.model.row_lower[row] != -INF)
    }

    fn yields_implied_upper_bound(&self, row: usize, val: f64) -> bool {
        self.yields_implied_lower_bound(row, -val)
    }

    // Only on a side the row has (HiGHS #3385; the other case is dual
    // infeasible).
    fn is_implied_equation_at_lower(&self, row: usize) -> bool {
        self.model.row_lower[row] != -INF && self.impl_row_dual_lower[row] > self.settings.dual_feasibility_tolerance
    }

    fn is_implied_equation_at_upper(&self, row: usize) -> bool {
        self.model.row_upper[row] != INF && self.impl_row_dual_upper[row] < -self.settings.dual_feasibility_tolerance
    }

    // ---- Matrix storage ------------------------------------------------

    /// A column's (row, value) entries, in list order.
    fn col_entries(&self, col: usize) -> Vec<(usize, f64)> {
        let mut entries = Vec::with_capacity(self.colsize[col]);
        let mut pos = self.colhead[col];
        while pos != NONE {
            entries.push((self.a_row[pos], self.a_value[pos]));
            pos = self.a_next[pos];
        }
        entries
    }

    /// A row's positions in pre-order (HiGHS's `getRowVector`).
    fn row_vector(&mut self, row: usize) -> Vec<usize> {
        let mut out = std::mem::take(&mut self.row_buffer);
        matrix::pre_order(self.rowroot[row], &self.ar_left, &self.ar_right, &mut out);
        let positions = out.clone();
        self.row_buffer = out;
        positions
    }

    fn row_entries(&mut self, row: usize) -> Vec<(usize, f64)> {
        self.row_vector(row).into_iter().map(|pos| (self.a_col[pos], self.a_value[pos])).collect()
    }

    fn stored_row_entries(&self) -> Vec<(usize, f64)> {
        self.rowpositions.iter().map(|&pos| (self.a_col[pos], self.a_value[pos])).collect()
    }

    /// HiGHS's `storeRow`: keep the row's positions in column order.
    fn store_row(&mut self, row: usize) {
        matrix::in_order(self.rowroot[row], &self.ar_left, &self.ar_right, &mut self.rowpositions);
    }

    fn load_csc(&mut self, value: &[f64], index: &[usize], start: &[usize]) {
        let num_col = self.model.num_col;
        let num_row = self.model.num_row;
        self.a_value.clear();
        self.a_col.clear();
        self.a_row.clear();
        self.freeslots.clear();
        self.colhead = vec![NONE; num_col];
        self.rowroot = vec![NONE; num_row];
        self.colsize = vec![0; num_col];
        self.rowsize = vec![0; num_row];
        self.implied_row_bounds.clear();
        self.implied_row_bounds.set_num_sums(num_row);
        self.implied_dual_row_bounds.clear();
        self.implied_dual_row_bounds.set_num_sums(num_col);
        let nnz = value.len();
        self.a_value = value.to_vec();
        self.a_col.reserve(nnz);
        self.a_row.reserve(nnz);
        for i in 0..num_col {
            let len = start[i + 1] - start[i];
            self.a_col.extend(std::iter::repeat_n(i, len));
            self.a_row.extend_from_slice(&index[start[i]..start[i + 1]]);
        }
        self.a_next = vec![0; nnz];
        self.a_prev = vec![0; nnz];
        self.ar_left = vec![0; nnz];
        self.ar_right = vec![0; nnz];
        for pos in 0..nnz {
            self.link(pos);
        }
        if self.equations.is_empty() {
            self.eq_key = vec![None; num_row];
            for i in 0..num_row {
                if self.is_equation(i) {
                    self.equations.insert((self.rowsize[i], i));
                    self.eq_key[i] = Some(self.rowsize[i]);
                }
            }
        }
    }

    /// HiGHS's `toCSC`: the matrix into `model`, column-wise, leaving the
    /// column sizes counted down to 0.
    fn write_csc(&mut self) {
        let num_col = self.colsize.len();
        let mut start = vec![0usize; num_col + 1];
        let mut nnz = 0;
        for i in 0..num_col {
            start[i] = nnz;
            nnz += self.colsize[i];
        }
        start[num_col] = nnz;
        let mut value = vec![0.0; nnz];
        let mut index = vec![0usize; nnz];
        for i in 0..self.a_value.len() {
            if self.a_value[i] == 0.0 {
                continue;
            }
            let col = self.a_col[i];
            let pos = start[col + 1] - self.colsize[col];
            self.colsize[col] -= 1;
            value[pos] = self.a_value[i];
            index[pos] = self.a_row[i];
        }
        self.model.start = start;
        self.model.index = index;
        self.model.value = value;
    }

    /// HiGHS's `toCSC` then `okFromCSC`: rebuild the storage compactly.
    fn rebuild_storage(&mut self) {
        self.write_csc();
        let (start, index, value) = (
            std::mem::take(&mut self.model.start),
            std::mem::take(&mut self.model.index),
            std::mem::take(&mut self.model.value),
        );
        self.load_csc(&value, &index, &start);
        self.model.start = start;
        self.model.index = index;
        self.model.value = value;
    }

    fn link(&mut self, pos: usize) {
        let col = self.a_col[pos];
        let row = self.a_row[pos];
        self.a_next[pos] = self.colhead[col];
        self.a_prev[pos] = NONE;
        self.colhead[col] = pos;
        if self.a_next[pos] != NONE {
            let next = self.a_next[pos];
            self.a_prev[next] = pos;
        }
        self.colsize[col] += 1;
        self.ar_left[pos] = NONE;
        self.ar_right[pos] = NONE;
        let root = self.rowroot[row];
        self.rowroot[row] = trees!(self).link(pos, root);
        let value = self.a_value[pos];
        self.implied_row_bounds.add(col_bounds!(self), row, col, value);
        self.implied_dual_row_bounds.add(dual_bounds!(self), col, row, value);
        self.rowsize[row] += 1;
    }

    fn unlink(&mut self, pos: usize) {
        let next = self.a_next[pos];
        let prev = self.a_prev[pos];
        let col = self.a_col[pos];
        let row = self.a_row[pos];
        let value = self.a_value[pos];
        if next != NONE {
            self.a_prev[next] = prev;
        }
        if prev != NONE {
            self.a_next[prev] = next;
        } else {
            self.colhead[col] = next;
        }
        self.colsize[col] -= 1;
        if !self.col_deleted[col] {
            if self.colsize[col] == 1 {
                self.singleton_columns.push(col);
            } else {
                self.mark_changed_col(col);
            }
            self.implied_dual_row_bounds.remove(dual_bounds!(self), col, row, value);
        }
        let root = self.rowroot[row];
        self.rowroot[row] = trees!(self).unlink(pos, root);
        self.rowsize[row] -= 1;
        if !self.row_deleted[row] {
            if self.rowsize[row] == 1 {
                self.singleton_rows.push(row);
            } else {
                self.mark_changed_row(row);
            }
            self.implied_row_bounds.remove(col_bounds!(self), row, col, value);
        }
        self.reset_row_dual_implied_bounds_derived_from_col(col);
        self.reset_col_implied_bounds_derived_from_row(row);
        self.a_value[pos] = 0.0;
        self.freeslots.push(pos);
    }

    fn mark_changed_row(&mut self, row: usize) {
        if !self.changed_row_flag[row] {
            self.changed_row_indices.push(row);
            self.changed_row_flag[row] = true;
        }
        self.reset_single_equation_checked(row);
    }

    fn mark_changed_col(&mut self, col: usize) {
        if !self.changed_col_flag[col] {
            self.changed_col_indices.push(col);
            self.changed_col_flag[col] = true;
        }
        // Its rows' single equations are worth checking again (HiGHS #3232),
        // until no row is flagged (#3341).
        let mut pos = self.colhead[col];
        while pos != NONE {
            if !self.reset_single_equation_checked(self.a_row[pos]) {
                break;
            }
            pos = self.a_next[pos];
        }
    }

    /// Clear `row`'s flag; whether any row is still flagged.
    fn reset_single_equation_checked(&mut self, row: usize) -> bool {
        if self.single_equation_checked[row] {
            self.single_equation_checked[row] = false;
            self.num_single_equation_checked -= 1;
        }
        self.num_single_equation_checked > 0
    }

    fn get_max_abs_col_val(&self, col: usize) -> f64 {
        let mut max_val = 0.0;
        let mut pos = self.colhead[col];
        while pos != NONE {
            max_val = max(self.a_value[pos].abs(), max_val);
            pos = self.a_next[pos];
        }
        max_val
    }

    fn get_max_abs_row_val(&mut self, row: usize) -> f64 {
        let mut max_val = 0.0;
        for pos in self.row_vector(row) {
            max_val = max(self.a_value[pos].abs(), max_val);
        }
        max_val
    }

    fn find_nonzero(&mut self, row: usize, col: usize) -> usize {
        if self.rowroot[row] == NONE {
            return NONE;
        }
        let root = self.rowroot[row];
        self.rowroot[row] = trees!(self).splay(col, root);
        if self.a_col[self.rowroot[row]] == col { self.rowroot[row] } else { NONE }
    }

    fn add_to_matrix(&mut self, row: usize, col: usize, val: f64) {
        let mut pos = self.find_nonzero(row, col);
        self.mark_changed_row(row);
        self.mark_changed_col(col);
        if pos == NONE {
            if let Some(free) = self.freeslots.pop() {
                pos = free;
                self.a_value[pos] = val;
                self.a_row[pos] = row;
                self.a_col[pos] = col;
                self.a_prev[pos] = NONE;
            } else {
                pos = self.a_value.len();
                self.a_value.push(val);
                self.a_row.push(row);
                self.a_col.push(col);
                self.a_next.push(NONE);
                self.a_prev.push(NONE);
                self.ar_left.push(NONE);
                self.ar_right.push(NONE);
            }
            self.link(pos);
            self.reset_row_dual_implied_bounds_derived_from_col(col);
            self.reset_col_implied_bounds_derived_from_row(row);
        } else {
            let sum = self.a_value[pos] + val;
            if sum.abs() <= self.settings.small_matrix_value {
                self.unlink(pos);
            } else {
                self.reset_row_dual_implied_bounds_derived_from_col(col);
                self.reset_col_implied_bounds_derived_from_row(row);
                let old = self.a_value[pos];
                self.implied_row_bounds.remove(col_bounds!(self), row, col, old);
                self.implied_dual_row_bounds.remove(dual_bounds!(self), col, row, old);
                self.a_value[pos] = sum;
                self.implied_row_bounds.add(col_bounds!(self), row, col, sum);
                self.implied_dual_row_bounds.add(dual_bounds!(self), col, row, sum);
            }
        }
    }

    fn mark_row_deleted(&mut self, row: usize) {
        if self.is_equation(row)
            && let Some(size) = self.eq_key[row].take()
        {
            self.equations.remove(&(size, row));
        }
        self.changed_row_flag[row] = true;
        self.row_deleted[row] = true;
        self.num_deleted_rows += 1;
        self.reset_single_equation_checked(row);
    }

    fn mark_col_deleted(&mut self, col: usize) {
        self.changed_col_flag[col] = true;
        self.col_deleted[col] = true;
        self.num_deleted_cols += 1;
    }

    fn reinsert_equation(&mut self, row: usize) {
        if self.is_equation(row)
            && let Some(size) = self.eq_key[row]
            && size != self.rowsize[row]
        {
            self.equations.remove(&(size, row));
            self.equations.insert((self.rowsize[row], row));
            self.eq_key[row] = Some(self.rowsize[row]);
        }
    }

    // ---- Bounds ----------------------------------------------------------

    fn change_col_upper(&mut self, col: usize, new_upper: f64) -> Outcome {
        let old_upper = self.model.col_upper[col];
        self.model.col_upper[col] = new_upper;
        check!(self.check_col_bounds(col).0);
        let mut pos = self.colhead[col];
        while pos != NONE {
            let row = self.a_row[pos];
            self.implied_row_bounds.updated_var_upper(col_bounds!(self), row, col, self.a_value[pos], old_upper);
            self.mark_changed_row(row);
            pos = self.a_next[pos];
        }
        Outcome::Ok
    }

    fn change_col_lower(&mut self, col: usize, new_lower: f64) -> Outcome {
        let old_lower = self.model.col_lower[col];
        self.model.col_lower[col] = new_lower;
        check!(self.check_col_bounds(col).0);
        let mut pos = self.colhead[col];
        while pos != NONE {
            let row = self.a_row[pos];
            self.implied_row_bounds.updated_var_lower(col_bounds!(self), row, col, self.a_value[pos], old_lower);
            self.mark_changed_row(row);
            pos = self.a_next[pos];
        }
        Outcome::Ok
    }

    fn change_col_bounds(&mut self, col: usize, new_lower: f64, new_upper: f64) -> Outcome {
        if new_lower > self.model.col_upper[col] {
            check!(self.change_col_upper(col, new_upper));
            check!(self.change_col_lower(col, new_lower));
        } else {
            check!(self.change_col_lower(col, new_lower));
            check!(self.change_col_upper(col, new_upper));
        }
        Outcome::Ok
    }

    /// The outcome of checking a column's bounds, and whether they fix it.
    fn check_col_bounds(&self, col: usize) -> (Outcome, bool) {
        let bound_diff = self.model.col_upper[col] - self.model.col_lower[col];
        if bound_diff <= self.primal_feastol && (bound_diff <= self.settings.small_matrix_value || self.get_max_abs_col_val(col) * bound_diff <= self.primal_feastol) {
            if bound_diff < -self.primal_feastol {
                return (Outcome::PrimalInfeasible, false);
            }
            if self.model.col_lower[col].abs() == INF {
                return (Outcome::DualInfeasible, false);
            }
            return (Outcome::Ok, true);
        }
        (Outcome::Ok, false)
    }

    fn change_row_dual_upper(&mut self, row: usize, new_upper: f64) {
        let old_upper = self.row_dual_upper[row];
        self.row_dual_upper[row] = new_upper;
        for pos in self.row_vector(row) {
            let col = self.a_col[pos];
            self.implied_dual_row_bounds.updated_var_upper(dual_bounds!(self), col, row, self.a_value[pos], old_upper);
            self.mark_changed_col(col);
        }
    }

    fn change_row_dual_lower(&mut self, row: usize, new_lower: f64) {
        let old_lower = self.row_dual_lower[row];
        self.row_dual_lower[row] = new_lower;
        for pos in self.row_vector(row) {
            let col = self.a_col[pos];
            self.implied_dual_row_bounds.updated_var_lower(dual_bounds!(self), col, row, self.a_value[pos], old_lower);
            self.mark_changed_col(col);
        }
    }

    fn change_impl_col_upper(&mut self, col: usize, new_upper: f64, origin_row: usize) {
        let old_impl_upper = self.impl_col_upper[col];
        let old_upper_source = self.col_upper_source[col];
        let upper = self.model.col_upper[col];
        if old_impl_upper >= upper - self.primal_feastol && new_upper < upper - self.primal_feastol {
            self.mark_changed_col(col);
        }
        let new_implied_free = self.is_lower_implied(col) && old_impl_upper > upper + self.primal_feastol && new_upper <= upper + self.primal_feastol;
        if old_upper_source != origin_row {
            if old_upper_source != NONE && old_upper_source != self.col_lower_source[col] {
                self.col_impl_source_by_row[old_upper_source].remove(&col);
            }
            if origin_row != NONE {
                self.col_impl_source_by_row[origin_row].insert(col);
            }
            self.col_upper_source[col] = origin_row;
        }
        self.impl_col_upper[col] = new_upper;
        if !new_implied_free && min(old_impl_upper, new_upper) >= upper {
            return;
        }
        let mut pos = self.colhead[col];
        while pos != NONE {
            let row = self.a_row[pos];
            self.implied_row_bounds
                .updated_impl_var_upper(col_bounds!(self), row, col, self.a_value[pos], old_impl_upper, old_upper_source);
            if new_implied_free && self.is_dual_implied_free(row) {
                self.substitution_opportunities.push((row, col));
            }
            self.mark_changed_row(row);
            pos = self.a_next[pos];
        }
    }

    fn change_impl_col_lower(&mut self, col: usize, new_lower: f64, origin_row: usize) {
        let old_impl_lower = self.impl_col_lower[col];
        let old_lower_source = self.col_lower_source[col];
        let lower = self.model.col_lower[col];
        if old_impl_lower <= lower + self.primal_feastol && new_lower > lower + self.primal_feastol {
            self.mark_changed_col(col);
        }
        let new_implied_free = self.is_upper_implied(col) && old_impl_lower < lower - self.primal_feastol && new_lower >= lower - self.primal_feastol;
        if old_lower_source != origin_row {
            if old_lower_source != NONE && old_lower_source != self.col_upper_source[col] {
                self.col_impl_source_by_row[old_lower_source].remove(&col);
            }
            if origin_row != NONE {
                self.col_impl_source_by_row[origin_row].insert(col);
            }
            self.col_lower_source[col] = origin_row;
        }
        self.impl_col_lower[col] = new_lower;
        if !new_implied_free && max(old_impl_lower, new_lower) <= lower {
            return;
        }
        let mut pos = self.colhead[col];
        while pos != NONE {
            let row = self.a_row[pos];
            self.implied_row_bounds
                .updated_impl_var_lower(col_bounds!(self), row, col, self.a_value[pos], old_impl_lower, old_lower_source);
            if new_implied_free && self.is_dual_implied_free(row) {
                self.substitution_opportunities.push((row, col));
            }
            self.mark_changed_row(row);
            pos = self.a_next[pos];
        }
    }

    fn change_impl_row_dual_upper(&mut self, row: usize, new_upper: f64, origin_col: usize) {
        let tol = self.settings.dual_feasibility_tolerance;
        let old_impl_upper = self.impl_row_dual_upper[row];
        let old_upper_source = self.row_dual_upper_source[row];
        if old_impl_upper >= -tol && new_upper < -tol {
            self.mark_changed_row(row);
        }
        // Ranged rows included: the row's dual is implied non-positive.
        let new_dual_implied = !self.is_dual_implied_free(row) && self.model.row_upper[row] != INF && new_upper <= tol;
        if old_upper_source != origin_col {
            if old_upper_source != NONE && old_upper_source != self.row_dual_lower_source[row] {
                self.impl_row_dual_source_by_col[old_upper_source].remove(&row);
            }
            if origin_col != NONE {
                self.impl_row_dual_source_by_col[origin_col].insert(row);
            }
            self.row_dual_upper_source[row] = origin_col;
        }
        self.impl_row_dual_upper[row] = new_upper;
        if !new_dual_implied && min(old_impl_upper, new_upper) >= self.row_dual_upper[row] {
            return;
        }
        for pos in self.row_vector(row) {
            let col = self.a_col[pos];
            self.implied_dual_row_bounds
                .updated_impl_var_upper(dual_bounds!(self), col, row, self.a_value[pos], old_impl_upper, old_upper_source);
            self.mark_changed_col(col);
            if new_dual_implied && self.is_implied_free(col) {
                self.substitution_opportunities.push((row, col));
            }
        }
    }

    fn change_impl_row_dual_lower(&mut self, row: usize, new_lower: f64, origin_col: usize) {
        let tol = self.settings.dual_feasibility_tolerance;
        let old_impl_lower = self.impl_row_dual_lower[row];
        let old_lower_source = self.row_dual_lower_source[row];
        if old_impl_lower <= tol && new_lower > tol {
            self.mark_changed_row(row);
        }
        // Ranged rows included: the row's dual is implied non-negative.
        let new_dual_implied = !self.is_dual_implied_free(row) && self.model.row_lower[row] != -INF && new_lower >= -tol;
        if old_lower_source != origin_col {
            if old_lower_source != NONE && old_lower_source != self.row_dual_upper_source[row] {
                self.impl_row_dual_source_by_col[old_lower_source].remove(&row);
            }
            if origin_col != NONE {
                self.impl_row_dual_source_by_col[origin_col].insert(row);
            }
            self.row_dual_lower_source[row] = origin_col;
        }
        self.impl_row_dual_lower[row] = new_lower;
        if !new_dual_implied && max(old_impl_lower, new_lower) <= self.row_dual_lower[row] {
            return;
        }
        for pos in self.row_vector(row) {
            let col = self.a_col[pos];
            self.implied_dual_row_bounds
                .updated_impl_var_lower(dual_bounds!(self), col, row, self.a_value[pos], old_impl_lower, old_lower_source);
            self.mark_changed_col(col);
            if new_dual_implied && self.is_implied_free(col) {
                self.substitution_opportunities.push((row, col));
            }
        }
    }

    // ---- Implied bounds --------------------------------------------------

    /// Whether the column's dual constraint can imply row dual bounds,
    /// and the dual row's sides.
    fn check_update_row_dual_implied_bounds(&self, col: usize) -> (bool, f64, f64) {
        let implied_margin = if self.colsize[col] != 1 { self.primal_feastol } else { -self.primal_feastol };
        let dual_row_lower = if self.is_lower_strictly_implied(col, implied_margin) {
            self.model.col_cost[col]
        } else {
            -INF
        };
        let dual_row_upper = if self.is_upper_strictly_implied(col, implied_margin) {
            self.model.col_cost[col]
        } else {
            INF
        };
        let ok = (dual_row_lower != -INF && self.implied_dual_row_bounds.num_inf_sum_upper_orig(col) <= 1)
            || (dual_row_upper != INF && self.implied_dual_row_bounds.num_inf_sum_lower_orig(col) <= 1);
        (ok, dual_row_lower, dual_row_upper)
    }

    fn update_row_dual_implied_bounds(&mut self, row: usize, col: usize, val: f64) {
        let (ok, dual_row_lower, dual_row_upper) = self.check_update_row_dual_implied_bounds(col);
        if !ok {
            return;
        }
        let threshold = 1000.0 * self.settings.dual_feasibility_tolerance;
        let check_implied_bound = |s: &mut Self, dual_row_bnd: f64, residual_act: f64, direction: f64| {
            if direction * residual_act <= -INF {
                return;
            }
            let implied_bound = ((CDouble::new(dual_row_bnd) - residual_act) / val).value();
            if implied_bound.abs() * TINY > s.settings.dual_feasibility_tolerance {
                return;
            }
            if direction * val > 0.0 {
                if implied_bound < s.impl_row_dual_upper[row] - threshold {
                    s.change_impl_row_dual_upper(row, implied_bound, col);
                }
            } else if implied_bound > s.impl_row_dual_lower[row] + threshold {
                s.change_impl_row_dual_lower(row, implied_bound, col);
            }
        };
        if dual_row_upper != INF {
            let residual = self.implied_dual_row_bounds.residual_sum_lower_orig(dual_bounds!(self), col, row, val, NONE, -INF, -INF);
            check_implied_bound(self, dual_row_upper, residual, 1.0);
        }
        if dual_row_lower != -INF {
            let residual = self.implied_dual_row_bounds.residual_sum_upper_orig(dual_bounds!(self), col, row, val, NONE, -INF, -INF);
            check_implied_bound(self, dual_row_lower, residual, -1.0);
        }
    }

    fn update_row_dual_implied_bounds_of_col(&mut self, col: usize) {
        if !self.check_update_row_dual_implied_bounds(col).0 {
            return;
        }
        let mut pos = self.colhead[col];
        while pos != NONE {
            self.update_row_dual_implied_bounds(self.a_row[pos], col, self.a_value[pos]);
            pos = self.a_next[pos];
        }
    }

    /// Whether the row can imply column bounds, and its sides.
    fn check_update_col_implied_bounds(&self, row: usize) -> (bool, f64, f64) {
        let row_lower = if self.is_implied_equation_at_upper(row) {
            self.model.row_upper[row]
        } else {
            self.model.row_lower[row]
        };
        let row_upper = if self.is_implied_equation_at_lower(row) {
            self.model.row_lower[row]
        } else {
            self.model.row_upper[row]
        };
        let ok = (row_lower != -INF && self.implied_row_bounds.num_inf_sum_upper_orig(row) <= 1) || (row_upper != INF && self.implied_row_bounds.num_inf_sum_lower_orig(row) <= 1);
        (ok, row_lower, row_upper)
    }

    fn update_col_implied_bounds(&mut self, row: usize, col: usize, val: f64) -> Outcome {
        let (ok, row_lower, row_upper) = self.check_update_col_implied_bounds(row);
        if !ok {
            return Outcome::Ok;
        }
        let threshold = 1000.0 * self.primal_feastol;
        let check_implied_bound = |s: &mut Self, row_bnd: f64, residual_act: f64, direction: f64| {
            if direction * residual_act <= -INF {
                return;
            }
            let implied_bound = ((CDouble::new(row_bnd) - residual_act) / val).value();
            if implied_bound.abs() * TINY > s.primal_feastol {
                return;
            }
            if direction * val > 0.0 {
                if implied_bound < s.impl_col_upper[col] - threshold {
                    s.change_impl_col_upper(col, implied_bound, row);
                }
            } else if implied_bound > s.impl_col_lower[col] + threshold {
                s.change_impl_col_lower(col, implied_bound, row);
            }
        };
        if row_upper != INF {
            let residual = self.implied_row_bounds.residual_sum_lower_orig(col_bounds!(self), row, col, val, NONE, -INF, -INF);
            check_implied_bound(self, row_upper, residual, 1.0);
        }
        if row_lower != -INF {
            let residual = self.implied_row_bounds.residual_sum_upper_orig(col_bounds!(self), row, col, val, NONE, -INF, -INF);
            check_implied_bound(self, row_lower, residual, -1.0);
        }
        Outcome::Ok
    }

    fn update_col_implied_bounds_of_row(&mut self, row: usize) -> Outcome {
        if !self.check_update_col_implied_bounds(row).0 {
            return Outcome::Ok;
        }
        for pos in self.row_vector(row) {
            check!(self.update_col_implied_bounds(row, self.a_col[pos], self.a_value[pos]));
        }
        Outcome::Ok
    }

    fn reset_col_implied_bounds(&mut self, col: usize, row: usize) {
        if !self.col_deleted[col] {
            if self.col_lower_source[col] != NONE && (row == NONE || self.col_lower_source[col] == row) {
                self.change_impl_col_lower(col, -INF, NONE);
            }
            if self.col_upper_source[col] != NONE && (row == NONE || self.col_upper_source[col] == row) {
                self.change_impl_col_upper(col, INF, NONE);
            }
        } else if row != NONE && !self.row_deleted[row] {
            self.col_impl_source_by_row[row].remove(&col);
        }
    }

    fn reset_row_dual_implied_bounds(&mut self, row: usize, col: usize) {
        if !self.row_deleted[row] {
            if self.row_dual_lower_source[row] != NONE && (col == NONE || self.row_dual_lower_source[row] == col) {
                self.change_impl_row_dual_lower(row, -INF, NONE);
            }
            if self.row_dual_upper_source[row] != NONE && (col == NONE || self.row_dual_upper_source[row] == col) {
                self.change_impl_row_dual_upper(row, INF, NONE);
            }
        } else if col != NONE && !self.col_deleted[col] {
            self.impl_row_dual_source_by_col[col].remove(&row);
        }
    }

    fn reset_col_implied_bounds_derived_from_row(&mut self, row: usize) {
        if self.col_impl_source_by_row[row].is_empty() {
            return;
        }
        let affected = std::mem::take(&mut self.col_impl_source_by_row[row]);
        for col in affected {
            self.reset_col_implied_bounds(col, row);
        }
    }

    fn reset_row_dual_implied_bounds_derived_from_col(&mut self, col: usize) {
        if self.impl_row_dual_source_by_col[col].is_empty() {
            return;
        }
        let affected = std::mem::take(&mut self.impl_row_dual_source_by_col[col]);
        for row in affected {
            self.reset_row_dual_implied_bounds(row, col);
        }
    }

    // ---- Removing rows and columns ---------------------------------------

    fn shrink_problem(&mut self, postsolve: &mut PostsolveStack) {
        let old_num_col = self.model.num_col;
        self.model.num_col = 0;
        let mut new_col_index = vec![NONE; old_num_col];
        for i in 0..old_num_col {
            if self.col_deleted[i] {
                continue;
            }
            let to = self.model.num_col;
            self.model.num_col += 1;
            new_col_index[i] = to;
            if to < i {
                self.model.col_cost[to] = self.model.col_cost[i];
                self.model.col_lower[to] = self.model.col_lower[i];
                self.model.col_upper[to] = self.model.col_upper[i];
                self.impl_col_lower[to] = self.impl_col_lower[i];
                self.impl_col_upper[to] = self.impl_col_upper[i];
                self.col_lower_source[to] = self.col_lower_source[i];
                self.col_upper_source[to] = self.col_upper_source[i];
                self.impl_row_dual_source_by_col[to] = self.impl_row_dual_source_by_col[i].clone();
                self.colhead[to] = self.colhead[i];
                self.colsize[to] = self.colsize[i];
                self.changed_col_flag[to] = self.changed_col_flag[i];
            }
        }
        let num_col = self.model.num_col;
        self.col_deleted = vec![false; num_col];
        self.model.col_cost.truncate(num_col);
        self.model.col_lower.truncate(num_col);
        self.model.col_upper.truncate(num_col);
        self.impl_col_lower.truncate(num_col);
        self.impl_col_upper.truncate(num_col);
        self.col_lower_source.truncate(num_col);
        self.col_upper_source.truncate(num_col);
        self.impl_row_dual_source_by_col.truncate(num_col);
        self.colhead.truncate(num_col);
        self.colsize.truncate(num_col);
        self.changed_col_flag.truncate(num_col);
        self.num_deleted_cols = 0;

        let old_num_row = self.model.num_row;
        self.model.num_row = 0;
        let mut new_row_index = vec![NONE; old_num_row];
        for i in 0..old_num_row {
            if self.row_deleted[i] {
                continue;
            }
            let to = self.model.num_row;
            self.model.num_row += 1;
            new_row_index[i] = to;
            if to < i {
                self.model.row_lower[to] = self.model.row_lower[i];
                self.model.row_upper[to] = self.model.row_upper[i];
                self.row_dual_lower[to] = self.row_dual_lower[i];
                self.row_dual_upper[to] = self.row_dual_upper[i];
                self.impl_row_dual_lower[to] = self.impl_row_dual_lower[i];
                self.impl_row_dual_upper[to] = self.impl_row_dual_upper[i];
                self.row_dual_lower_source[to] = self.row_dual_lower_source[i];
                self.row_dual_upper_source[to] = self.row_dual_upper_source[i];
                self.col_impl_source_by_row[to] = self.col_impl_source_by_row[i].clone();
                self.rowroot[to] = self.rowroot[i];
                self.rowsize[to] = self.rowsize[i];
                self.changed_row_flag[to] = self.changed_row_flag[i];
                self.single_equation_checked[to] = self.single_equation_checked[i];
            }
        }
        let num_row = self.model.num_row;
        for i in 0..num_col {
            if self.col_lower_source[i] != NONE {
                self.col_lower_source[i] = new_row_index[self.col_lower_source[i]];
            }
            if self.col_upper_source[i] != NONE {
                self.col_upper_source[i] = new_row_index[self.col_upper_source[i]];
            }
        }
        for i in 0..num_row {
            if self.row_dual_lower_source[i] != NONE {
                self.row_dual_lower_source[i] = new_col_index[self.row_dual_lower_source[i]];
            }
            if self.row_dual_upper_source[i] != NONE {
                self.row_dual_upper_source[i] = new_col_index[self.row_dual_upper_source[i]];
            }
        }
        for sources in &mut self.impl_row_dual_source_by_col[..num_col] {
            *sources = sources.iter().filter_map(|&row| (new_row_index[row] != NONE).then_some(new_row_index[row])).collect();
        }
        for sources in &mut self.col_impl_source_by_row[..num_row] {
            *sources = sources.iter().filter_map(|&col| (new_col_index[col] != NONE).then_some(new_col_index[col])).collect();
        }
        self.row_deleted = vec![false; num_row];
        self.model.row_lower.truncate(num_row);
        self.model.row_upper.truncate(num_row);
        self.row_dual_lower.truncate(num_row);
        self.row_dual_upper.truncate(num_row);
        self.impl_row_dual_lower.truncate(num_row);
        self.impl_row_dual_upper.truncate(num_row);
        self.row_dual_lower_source.truncate(num_row);
        self.row_dual_upper_source.truncate(num_row);
        self.col_impl_source_by_row.truncate(num_row);
        self.rowroot.truncate(num_row);
        self.rowsize.truncate(num_row);
        self.changed_row_flag.truncate(num_row);
        self.single_equation_checked.truncate(num_row);
        self.num_deleted_rows = 0;

        postsolve.compress_index_maps(&new_row_index, &new_col_index);
        self.implied_row_bounds.shrink(&new_row_index, num_row);
        self.implied_dual_row_bounds.shrink(&new_col_index, num_col);

        for i in 0..self.a_value.len() {
            if self.a_value[i] == 0.0 {
                continue;
            }
            self.a_col[i] = new_col_index[self.a_col[i]];
            self.a_row[i] = new_row_index[self.a_row[i]];
        }

        for col in &mut self.singleton_columns {
            *col = new_col_index[*col];
        }
        self.singleton_columns.retain(|&col| col != NONE);
        for col in &mut self.changed_col_indices {
            *col = new_col_index[*col];
        }
        self.changed_col_indices.retain(|&col| col != NONE);
        for row in &mut self.singleton_rows {
            *row = new_row_index[*row];
        }
        self.singleton_rows.retain(|&row| row != NONE);
        for row in &mut self.changed_row_indices {
            *row = new_row_index[*row];
        }
        self.changed_row_indices.retain(|&row| row != NONE);
        for pair in &mut self.substitution_opportunities {
            if pair.0 == NONE {
                continue;
            }
            pair.0 = new_row_index[pair.0];
            pair.1 = new_col_index[pair.1];
        }
        self.substitution_opportunities.retain(|&(row, col)| row != NONE && col != NONE);

        self.equations.clear();
        self.eq_key = vec![None; num_row];
        for i in 0..num_row {
            if self.is_equation(i) {
                self.equations.insert((self.rowsize[i], i));
                self.eq_key[i] = Some(self.rowsize[i]);
            }
        }
    }

    fn remove_row(&mut self, row: usize) {
        self.mark_row_deleted(row);
        self.store_row(row);
        for k in 0..self.rowpositions.len() {
            let pos = self.rowpositions[k];
            self.unlink(pos);
        }
    }

    fn remove_fixed_col(&mut self, col: usize) {
        let fixval = self.model.col_lower[col];
        self.remove_fixed_col_at(col, fixval);
    }

    fn remove_fixed_col_at(&mut self, col: usize, fixval: f64) {
        self.mark_col_deleted(col);
        let mut coliter = self.colhead[col];
        while coliter != NONE {
            let colrow = self.a_row[coliter];
            let colval = self.a_value[coliter];
            let colpos = coliter;
            coliter = self.a_next[coliter];
            if self.model.row_lower[colrow] != -INF {
                self.model.row_lower[colrow] -= colval * fixval;
            }
            if self.model.row_upper[colrow] != INF {
                self.model.row_upper[colrow] -= colval * fixval;
            }
            self.unlink(colpos);
            self.reinsert_equation(colrow);
        }
        self.model.offset += self.model.col_cost[col] * fixval;
        self.model.col_cost[col] = 0.0;
    }

    fn fix_col_to_lower(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        let fixval = self.model.col_lower[col];
        if fixval == -INF {
            return Outcome::DualInfeasible;
        }
        postsolve.fixed_col_at_lower(col, fixval, self.model.col_cost[col], self.col_entries(col));
        self.remove_fixed_col_at(col, fixval);
        Outcome::Ok
    }

    fn fix_col_to_upper(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        let fixval = self.model.col_upper[col];
        if fixval == INF {
            return Outcome::DualInfeasible;
        }
        postsolve.fixed_col_at_upper(col, fixval, self.model.col_cost[col], self.col_entries(col));
        self.remove_fixed_col_at(col, fixval);
        Outcome::Ok
    }

    fn fix_col_to_zero(&mut self, postsolve: &mut PostsolveStack, col: usize) {
        postsolve.fixed_col_at_zero(col, self.model.col_cost[col], self.col_entries(col));
        self.remove_fixed_col_at(col, 0.0);
    }

    /// Substitute `col` out using row `row` (whose positions are stored):
    /// `col = (rhs - rest of row) / coefficient`.
    fn substitute_row_col(&mut self, row: usize, col: usize, rhs: f64) {
        let pos = self.find_nonzero(row, col);
        let substrowscale = -1.0 / self.a_value[pos];
        self.mark_row_deleted(row);
        self.mark_col_deleted(col);
        let mut coliter = self.colhead[col];
        while coliter != NONE {
            let colrow = self.a_row[coliter];
            let colval = self.a_value[coliter];
            let colpos = coliter;
            coliter = self.a_next[coliter];
            if row == colrow {
                continue;
            }
            self.unlink(colpos);
            let scale = colval * substrowscale;
            if self.model.row_lower[colrow] != -INF {
                self.model.row_lower[colrow] += scale * rhs;
            }
            if self.model.row_upper[colrow] != INF {
                self.model.row_upper[colrow] += scale * rhs;
            }
            for k in 0..self.rowpositions.len() {
                let rowiter = self.rowpositions[k];
                if self.a_col[rowiter] != col {
                    self.add_to_matrix(colrow, self.a_col[rowiter], scale * self.a_value[rowiter]);
                }
            }
            self.reinsert_equation(colrow);
        }
        if self.model.col_cost[col] != 0.0 {
            let objscale = CDouble::new(self.model.col_cost[col]) * substrowscale;
            self.model.offset = (-(objscale * rhs) + self.model.offset).value();
            for k in 0..self.rowpositions.len() {
                let rowiter = self.rowpositions[k];
                let c = self.a_col[rowiter];
                self.model.col_cost[c] = (objscale * self.a_value[rowiter] + self.model.col_cost[c]).value();
                if self.model.col_cost[c].abs() <= self.settings.small_matrix_value {
                    self.model.col_cost[c] = 0.0;
                }
            }
            self.model.col_cost[col] = 0.0;
        }
        for k in 0..self.rowpositions.len() {
            let pos = self.rowpositions[k];
            self.unlink(pos);
        }
    }

    /// Substitute `substcol = offset + scale * staycol` everywhere.
    fn substitute_cols(&mut self, substcol: usize, staycol: usize, offset: f64, scale: f64) {
        let mut coliter = self.colhead[substcol];
        while coliter != NONE {
            let colrow = self.a_row[coliter];
            let colval = self.a_value[coliter];
            let colpos = coliter;
            coliter = self.a_next[coliter];
            self.unlink(colpos);
            if self.model.row_lower[colrow] != -INF {
                self.model.row_lower[colrow] -= colval * offset;
            }
            if self.model.row_upper[colrow] != INF {
                self.model.row_upper[colrow] -= colval * offset;
            }
            self.add_to_matrix(colrow, staycol, scale * colval);
            self.reinsert_equation(colrow);
        }
        if self.model.col_cost[substcol] != 0.0 {
            self.model.offset += self.model.col_cost[substcol] * offset;
            self.model.col_cost[staycol] += scale * self.model.col_cost[substcol];
            if self.model.col_cost[staycol].abs() <= self.settings.small_matrix_value {
                self.model.col_cost[staycol] = 0.0;
            }
            self.model.col_cost[substcol] = 0.0;
        }
    }

    fn substitute_free_col(&mut self, postsolve: &mut PostsolveStack, row: usize, col: usize, relax_row_dual_bounds: bool) {
        let (rhs, row_type) = self.dual_implied_free_get_rhs_and_row_type(row, relax_row_dual_bounds);
        postsolve.free_col_substitution(row, col, rhs, self.model.col_cost[col], row_type, self.stored_row_entries(), self.col_entries(col));
        self.substitute_row_col(row, col, rhs);
    }

    // ---- Limits ----------------------------------------------------------

    fn check_limits(&self, postsolve: &PostsolveStack) -> Outcome {
        // HiGHS reads its clock every 1024 reductions.
        if postsolve.num_reductions() & 1023 == 0 && self.timed_out() {
            return Outcome::Stopped;
        }
        if postsolve.num_reductions() >= self.reduction_limit {
            Outcome::Stopped
        } else {
            Outcome::Ok
        }
    }

    fn store_current_problem_size(&mut self) {
        self.old_num_col = self.model.num_col - self.num_deleted_cols;
        self.old_num_row = self.model.num_row - self.num_deleted_rows;
    }

    fn problem_size_reduction(&self) -> f64 {
        let col_reduction = 100.0 * (self.old_num_col as f64 - (self.model.num_col - self.num_deleted_cols) as f64) / self.old_num_col as f64;
        let row_reduction = 100.0 * (self.old_num_row as f64 - (self.model.num_row - self.num_deleted_rows) as f64) / self.old_num_row as f64;
        max(row_reduction, col_reduction)
    }

    fn num_nonzeros(&self) -> usize {
        self.a_value.len() - self.freeslots.len()
    }

    fn compute_dynamism(values: impl IntoIterator<Item = f64>) -> CDouble {
        let mut min_abs_coef = INF;
        let mut max_abs_coef = -INF;
        for value in values {
            let abs_coef = value.abs();
            min_abs_coef = min(min_abs_coef, abs_coef);
            max_abs_coef = max(max_abs_coef, abs_coef);
        }
        CDouble::new(max_abs_coef) / CDouble::new(min_abs_coef)
    }
}

// ---- Reductions ------------------------------------------------------------

impl Presolve {
    fn doubleton_eq(&mut self, postsolve: &mut PostsolveStack, row: usize, row_type: RowType) -> Outcome {
        let nz_pos1 = self.rowroot[row];
        let nz_pos2 = if self.ar_right[nz_pos1] != NONE {
            self.ar_right[nz_pos1]
        } else {
            self.ar_left[nz_pos1]
        };
        let col_at_pos1_better = {
            let col1_size = self.colsize[self.a_col[nz_pos1]];
            if col1_size == 1 {
                true
            } else {
                let col2_size = self.colsize[self.a_col[nz_pos2]];
                if col2_size == 1 {
                    false
                } else {
                    let abs1_val = self.a_value[nz_pos1].abs();
                    let abs2_val = self.a_value[nz_pos2].abs();
                    if col1_size != col2_size && max(abs1_val, abs2_val) <= 2.0 * min(abs1_val, abs2_val) {
                        col1_size < col2_size
                    } else {
                        abs1_val > abs2_val
                    }
                }
            }
        };
        let (substcol, staycol, substcoef, staycoef) = if col_at_pos1_better {
            (self.a_col[nz_pos1], self.a_col[nz_pos2], self.a_value[nz_pos1], self.a_value[nz_pos2])
        } else {
            (self.a_col[nz_pos2], self.a_col[nz_pos1], self.a_value[nz_pos2], self.a_value[nz_pos1])
        };
        let rhs = self.model.row_upper[row];
        let old_stay_lower = self.model.col_lower[staycol];
        let old_stay_upper = self.model.col_upper[staycol];
        let subst_lower = self.model.col_lower[substcol];
        let subst_upper = self.model.col_upper[substcol];
        let implied = |bound: f64| ((CDouble::new(rhs) - substcoef * bound) / staycoef).value();
        let (stay_impl_lower, stay_impl_upper) = if substcoef.is_sign_negative() != staycoef.is_sign_negative() {
            (
                if subst_lower == -INF { -INF } else { implied(subst_lower) },
                if subst_upper == INF { INF } else { implied(subst_upper) },
            )
        } else {
            (
                if subst_upper == INF { -INF } else { implied(subst_upper) },
                if subst_lower == -INF { INF } else { implied(subst_lower) },
            )
        };
        let lower_tightened = stay_impl_lower > old_stay_lower + self.primal_feastol;
        if lower_tightened {
            check!(self.change_col_lower(staycol, stay_impl_lower));
        }
        let upper_tightened = stay_impl_upper < old_stay_upper - self.primal_feastol;
        if upper_tightened {
            check!(self.change_col_upper(staycol, stay_impl_upper));
        }
        postsolve.doubleton_equation(
            row,
            substcol,
            staycol,
            substcoef,
            staycoef,
            rhs,
            self.model.col_cost[substcol],
            lower_tightened,
            upper_tightened,
            row_type,
            self.col_entries(substcol),
        );
        self.mark_col_deleted(substcol);
        self.remove_row(row);
        self.substitute_cols(substcol, staycol, rhs / substcoef, -staycoef / substcoef);
        check!(self.remove_row_singletons(postsolve));
        self.check_limits(postsolve)
    }

    fn singleton_row(&mut self, postsolve: &mut PostsolveStack, row: usize) -> Outcome {
        let nz_pos = self.rowroot[row];
        let col = self.a_col[nz_pos];
        let val = self.a_value[nz_pos];
        self.mark_row_deleted(row);
        self.unlink(nz_pos);
        let feastol = self.primal_feastol;
        let (col_lower, col_upper) = (self.model.col_lower[col], self.model.col_upper[col]);
        let (row_lower, row_upper) = (self.model.row_lower[row], self.model.row_upper[row]);
        let redundant = if val > 0.0 {
            col_upper * val <= row_upper + feastol && col_lower * val >= row_lower - feastol
        } else {
            col_lower * val <= row_upper + feastol && col_upper * val >= row_lower - feastol
        };
        if redundant {
            postsolve.redundant_row(row);
            return self.check_limits(postsolve);
        }
        let mut new_col_upper = INF;
        let mut new_col_lower = -INF;
        if val > 0.0 {
            if row_upper != INF {
                new_col_upper = row_upper / val;
            }
            if row_lower != -INF {
                new_col_lower = row_lower / val;
            }
        } else {
            if row_upper != INF {
                new_col_lower = row_upper / val;
            }
            if row_lower != -INF {
                new_col_upper = row_lower / val;
            }
        }
        // At least machine epsilon (HiGHS #3224).
        let bound_tol = (feastol / max(1.0, val.abs())).max(f64::EPSILON);
        let mut lower_tightened = new_col_lower > col_lower + bound_tol;
        let mut upper_tightened = new_col_upper < col_upper - bound_tol;
        let mut lb = if lower_tightened { new_col_lower } else { col_lower };
        let mut ub = if upper_tightened { new_col_upper } else { col_upper };
        if ub <= lb + feastol {
            if ub < lb - feastol {
                return Outcome::PrimalInfeasible;
            }
            if ub < lb || (ub > lb && (ub - lb) * max(val.abs(), self.get_max_abs_col_val(col)) <= feastol) {
                if lower_tightened && upper_tightened {
                    ub = 0.5 * (ub + lb);
                    lb = ub;
                    lower_tightened = lb > col_lower;
                    upper_tightened = ub < col_upper;
                } else if lower_tightened {
                    lb = ub;
                    lower_tightened = lb > col_lower;
                } else {
                    ub = lb;
                    upper_tightened = ub < col_upper;
                }
            }
        }
        postsolve.singleton_row(row, col, val, lower_tightened, upper_tightened);
        if lower_tightened {
            check!(self.change_col_lower(col, lb));
        }
        if ub == lb {
            postsolve.removed_fixed_col(col, lb, self.model.col_cost[col], self.col_entries(col));
            self.remove_fixed_col(col);
        } else if upper_tightened {
            check!(self.change_col_upper(col, ub));
        }
        if !self.col_deleted[col] && self.colsize[col] == 0 {
            return self.empty_col(postsolve, col);
        }
        self.check_limits(postsolve)
    }

    fn singleton_col(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        let nz_pos = self.colhead[col];
        let row = self.a_row[nz_pos];
        let col_coef = self.a_value[nz_pos];
        if self.rowsize[row] == 1 {
            check!(self.singleton_row(postsolve, row));
            if !self.col_deleted[col] {
                return self.empty_col(postsolve, col);
            }
            return Outcome::Ok;
        }
        check!(self.detect_dominated_col(postsolve, col, false));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.dual_fixing(postsolve, col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.singleton_col_stuffing(postsolve, col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.update_col_implied_bounds(row, col, col_coef));
        self.update_row_dual_implied_bounds(row, col, col_coef);
        if self.is_dual_implied_free(row) && self.is_implied_free(col) {
            self.store_row(row);
            self.substitute_free_col(postsolve, row, col, false);
            return self.check_limits(postsolve);
        }
        Outcome::Ok
    }

    fn row_presolve(&mut self, postsolve: &mut PostsolveStack, row: usize) -> Outcome {
        let feastol = self.primal_feastol;
        if self.implied_row_bounds.sum_lower(row, CDouble::default()) > self.model.row_upper[row] + feastol
            || self.implied_row_bounds.sum_upper(row, CDouble::default()) < self.model.row_lower[row] - feastol
        {
            return Outcome::PrimalInfeasible;
        }
        // A row dual bounded away from zero on a side the row lacks
        // (HiGHS #3392).
        let dual_tol = self.settings.dual_feasibility_tolerance;
        if (self.impl_row_dual_lower[row] > dual_tol && self.model.row_lower[row] == -INF) || (self.impl_row_dual_upper[row] < -dual_tol && self.model.row_upper[row] == INF) {
            return Outcome::DualInfeasible;
        }
        if self.rowsize[row] == 1 {
            return self.singleton_row(postsolve, row);
        }
        // HiGHS checks for redundancy twice, around reductions on integer
        // columns; an LP has none, so the second check finds nothing new.
        for _ in 0..2 {
            if self.is_redundant(row) {
                postsolve.redundant_row(row);
                self.remove_row(row);
                check!(self.check_limits(postsolve));
            }
            if self.row_deleted[row] {
                return Outcome::Ok;
            }
        }

        let orig_row_upper = self.model.row_upper[row];
        let orig_row_lower = self.model.row_lower[row];
        if !self.is_equation(row) {
            if self.is_implied_equation_at_lower(row) {
                self.model.row_upper[row] = self.model.row_lower[row];
                self.change_row_dual_lower(row, -INF);
                check!(self.check_redundant_bounds(self.row_dual_lower_source[row], row));
            } else if self.is_implied_equation_at_upper(row) {
                self.model.row_lower[row] = self.model.row_upper[row];
                self.change_row_dual_upper(row, INF);
                check!(self.check_redundant_bounds(self.row_dual_upper_source[row], row));
            }
        }
        let row_upper = self.model.row_upper[row];
        let row_lower = self.model.row_lower[row];
        if self.rowsize[row] == 2 && row_lower == row_upper {
            let row_type = if orig_row_lower == orig_row_upper {
                RowType::Eq
            } else if orig_row_upper != INF {
                RowType::Leq
            } else {
                RowType::Geq
            };
            return self.doubleton_eq(postsolve, row, row_type);
        }

        // Forcing rows.
        self.store_row(row);
        let dynamism = Self::compute_dynamism(self.rowpositions.iter().map(|&pos| self.a_value[pos]));
        let side = self.model.row_lower[row];
        let bound = self.implied_row_bounds.sum_upper_orig(row, CDouble::default());
        check!(self.check_forcing_row(postsolve, row, 1.0, side, bound, dynamism, RowType::Geq));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }
        let side = self.model.row_upper[row];
        let bound = self.implied_row_bounds.sum_lower_orig(row, CDouble::default());
        check!(self.check_forcing_row(postsolve, row, -1.0, side, bound, dynamism, RowType::Leq));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }
        check!(self.update_col_implied_bounds_of_row(row));
        self.check_limits(postsolve)
    }

    /// A column singleton whose bound the row implies loses that bound,
    /// once the row has become an equation by complementary slackness.
    fn check_redundant_bounds(&mut self, col: usize, row: usize) -> Outcome {
        let _ = row;
        if self.colsize[col] != 1 {
            return Outcome::Ok;
        }
        if self.model.col_cost[col] > 0.0 {
            if self.model.col_lower[col] > self.impl_col_lower[col] - self.primal_feastol {
                check!(self.change_col_lower(col, -INF));
            }
        } else if self.model.col_upper[col] < self.impl_col_upper[col] + self.primal_feastol {
            check!(self.change_col_upper(col, INF));
        }
        Outcome::Ok
    }

    #[allow(clippy::too_many_arguments)]
    fn check_forcing_row(
        &mut self,
        postsolve: &mut PostsolveStack,
        row: usize,
        direction: f64,
        row_side: f64,
        implied_row_bound: f64,
        dynamism: CDouble,
        row_type: RowType,
    ) -> Outcome {
        if direction * row_side == -INF
            || direction * implied_row_bound == INF
            || (CDouble::new(row_side) - CDouble::new(implied_row_bound)).abs().value() > (CDouble::new(self.primal_feastol) / dynamism).value().max(f64::EPSILON)
        {
            return Outcome::Ok;
        }
        let row_vector = self.stored_row_entries();
        let mut nfixings = 0;
        for &(col, value) in &row_vector {
            if direction * value > 0.0 {
                if self.model.col_upper[col] <= self.impl_col_upper[col] {
                    nfixings += 1;
                }
            } else if self.model.col_lower[col] >= self.impl_col_lower[col] {
                nfixings += 1;
            }
        }
        if nfixings != self.rowsize[row] {
            return Outcome::Ok;
        }
        postsolve.forcing_row(row, row_vector.iter().copied(), row_type);
        self.mark_row_deleted(row);
        for &(col, value) in &row_vector {
            if direction * value > 0.0 {
                check!(self.fix_col_to_upper(postsolve, col));
            } else {
                check!(self.fix_col_to_lower(postsolve, col));
            }
        }
        postsolve.redundant_row(row);
        check!(self.remove_row_singletons(postsolve));
        self.check_limits(postsolve)
    }

    fn empty_col(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        let cost = self.model.col_cost[col];
        if (cost > 0.0 && self.model.col_lower[col] == -INF) || (cost < 0.0 && self.model.col_upper[col] == INF) {
            if cost.abs() <= self.settings.dual_feasibility_tolerance {
                self.model.col_cost[col] = 0.0;
            } else {
                return Outcome::DualInfeasible;
            }
        }
        let cost = self.model.col_cost[col];
        if cost > 0.0 {
            check!(self.fix_col_to_lower(postsolve, col));
        } else if cost < 0.0 || self.model.col_upper[col].abs() < self.model.col_lower[col].abs() {
            check!(self.fix_col_to_upper(postsolve, col));
        } else if self.model.col_lower[col] != -INF {
            check!(self.fix_col_to_lower(postsolve, col));
        } else {
            self.fix_col_to_zero(postsolve, col);
        }
        self.check_limits(postsolve)
    }

    fn col_presolve(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        let (outcome, is_fixed) = self.check_col_bounds(col);
        check!(outcome);
        if is_fixed {
            postsolve.removed_fixed_col(col, self.model.col_lower[col], self.model.col_cost[col], self.col_entries(col));
            self.remove_fixed_col(col);
            return self.check_limits(postsolve);
        }
        match self.colsize[col] {
            0 => return self.empty_col(postsolve, col),
            1 => return self.singleton_col(postsolve, col),
            _ => {}
        }
        check!(self.detect_dominated_col(postsolve, col, true));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.dual_fixing(postsolve, col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.singleton_col_stuffing(postsolve, col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        self.update_row_dual_implied_bounds_of_col(col);
        Outcome::Ok
    }

    fn detect_dominated_col(&mut self, postsolve: &mut PostsolveStack, col: usize, handle_singleton_rows: bool) -> Outcome {
        let cost = CDouble::new(-self.model.col_cost[col]);
        let col_dual_upper = -self.implied_dual_row_bounds.sum_lower(col, cost);
        let col_dual_lower = -self.implied_dual_row_bounds.sum_upper(col, cost);
        let mut values = Vec::with_capacity(self.colsize[col]);
        let mut pos = self.colhead[col];
        while pos != NONE {
            values.push(self.a_value[pos]);
            pos = self.a_next[pos];
        }
        let dynamism = Self::compute_dynamism(values);
        let tol = self.settings.dual_feasibility_tolerance;

        let fix = |s: &mut Self, postsolve: &mut PostsolveStack, direction: f64| -> Outcome {
            if direction > 0.0 {
                check!(s.fix_col_to_lower(postsolve, col));
            } else {
                check!(s.fix_col_to_upper(postsolve, col));
            }
            if handle_singleton_rows {
                check!(s.remove_row_singletons(postsolve));
            }
            s.check_limits(postsolve)
        };
        let dominated_col = |s: &mut Self, postsolve: &mut PostsolveStack, dual_bound: f64, bound: f64, direction: f64| -> Outcome {
            if direction * dual_bound <= tol {
                return Outcome::Ok;
            }
            if direction * bound == -INF {
                return Outcome::DualInfeasible;
            }
            fix(s, postsolve, direction)
        };
        let weakly_dominated_col = |s: &mut Self, postsolve: &mut PostsolveStack, dual_bound: f64, bound: f64, other_bound: f64, direction: f64| -> Outcome {
            if direction * dual_bound < -tol {
                return Outcome::Ok;
            }
            if direction * bound != -INF {
                return fix(s, postsolve, direction);
            }
            // A forcing column.
            let cost = CDouble::new(-s.model.col_cost[col]);
            let bound_on_col_dual = if direction > 0.0 {
                -s.implied_dual_row_bounds.sum_upper_orig(col, cost)
            } else {
                -s.implied_dual_row_bounds.sum_lower_orig(col, cost)
            };
            if bound_on_col_dual.abs() <= (CDouble::new(tol) / dynamism).value().max(f64::EPSILON) {
                postsolve.forcing_column(col, s.col_entries(col), other_bound, direction < 0.0);
                s.mark_col_deleted(col);
                let mut coliter = s.colhead[col];
                while coliter != NONE {
                    let row = s.a_row[coliter];
                    let rhs = if direction * s.a_value[coliter] > 0.0 {
                        s.model.row_upper[row]
                    } else {
                        s.model.row_lower[row]
                    };
                    coliter = s.a_next[coliter];
                    let entries = s.row_entries(row);
                    postsolve.forcing_column_removed_row(col, row, rhs, entries);
                    s.remove_row(row);
                }
                return s.check_limits(postsolve);
            }
            Outcome::Ok
        };

        let lower = self.model.col_lower[col];
        check!(dominated_col(self, postsolve, col_dual_lower, lower, 1.0));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        let upper = self.model.col_upper[col];
        check!(dominated_col(self, postsolve, col_dual_upper, upper, -1.0));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        let (lower, upper) = (self.model.col_lower[col], self.model.col_upper[col]);
        check!(weakly_dominated_col(self, postsolve, col_dual_lower, lower, upper, 1.0));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        let (lower, upper) = (self.model.col_lower[col], self.model.col_upper[col]);
        weakly_dominated_col(self, postsolve, col_dual_upper, upper, lower, -1.0)
    }

    fn dual_fixing(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        if self.model.col_lower[col] == self.model.col_upper[col] {
            return Outcome::Ok;
        }
        // Locks: the objective, then each row that bounds the column.
        let mut num_down_locks = 0;
        let mut num_up_locks = 0;
        let mut down_lock_row = NONE;
        let mut up_lock_row = NONE;
        'locks: {
            let mut lock = |row: usize, has_down_lock: bool, has_up_lock: bool| {
                if has_up_lock {
                    num_up_locks += 1;
                    up_lock_row = row;
                }
                if has_down_lock {
                    num_down_locks += 1;
                    down_lock_row = row;
                }
                num_down_locks > 1 && num_up_locks > 1
            };
            let cost = self.model.col_cost[col];
            if cost < 0.0 {
                if lock(NONE, true, false) {
                    break 'locks;
                }
            } else if cost > 0.0 && lock(NONE, false, true) {
                break 'locks;
            }
            let mut pos = self.colhead[col];
            while pos != NONE {
                let (row, value) = (self.a_row[pos], self.a_value[pos]);
                let has_down_lock = self.yields_implied_lower_bound(row, value);
                let has_up_lock = self.yields_implied_upper_bound(row, value);
                if (has_down_lock || has_up_lock) && lock(row, has_down_lock, has_up_lock) {
                    break;
                }
                pos = self.a_next[pos];
            }
        }

        // With no down (up) locks the column can go to its lower (upper)
        // bound. A cost beyond the tolerance forces it there, and an
        // infinite bound then means unboundedness; a column of about zero
        // cost is dual feasible anywhere, so it is fixed only at a finite
        // bound.
        if num_down_locks == 0 || num_up_locks == 0 {
            let tol = self.settings.dual_feasibility_tolerance;
            if num_down_locks == 0 {
                if self.model.col_cost[col] > tol || self.model.col_lower[col] > -INF {
                    check!(self.fix_col_to_lower(postsolve, col));
                }
            } else if self.model.col_cost[col] < -tol || self.model.col_upper[col] < INF {
                check!(self.fix_col_to_upper(postsolve, col));
            }
            return Outcome::Ok;
        }
        let has_single_down_lock = num_down_locks == 1 && down_lock_row != NONE;
        let has_single_up_lock = num_up_locks == 1 && up_lock_row != NONE;
        if has_single_down_lock || has_single_up_lock {
            let equation_row = if has_single_down_lock && self.is_equation(down_lock_row) {
                down_lock_row
            } else if has_single_up_lock && self.is_equation(up_lock_row) {
                up_lock_row
            } else {
                NONE
            };
            if equation_row != NONE && !self.single_equation_checked[equation_row] {
                check!(self.handle_single_equation(postsolve, equation_row));
                self.single_equation_checked[equation_row] = true;
                self.num_single_equation_checked += 1;
                if self.col_deleted[col] {
                    return Outcome::Ok;
                }
            }
        }
        // Tighter bounds can only fix a continuous column here.
        if let Some(new_bound) = self.has_tighter_bound(col, 1.0, self.model.col_upper[col]) {
            let new_bound = max(new_bound, self.model.col_lower[col]);
            if new_bound < self.model.col_upper[col] - self.primal_feastol && new_bound == self.model.col_lower[col] {
                check!(self.fix_col_to_lower(postsolve, col));
            }
        } else if let Some(new_bound) = self.has_tighter_bound(col, -1.0, self.model.col_lower[col]) {
            let new_bound = min(new_bound, self.model.col_upper[col]);
            if new_bound > self.model.col_lower[col] + self.primal_feastol && new_bound == self.model.col_upper[col] {
                check!(self.fix_col_to_upper(postsolve, col));
            }
        }
        Outcome::Ok
    }

    /// HiGHS's `hasTighterBound` lambda in `dualFixing`.
    fn has_tighter_bound(&mut self, col: usize, direction: f64, current_bound: f64) -> Option<f64> {
        if direction * self.model.col_cost[col] < 0.0 {
            return None;
        }
        let huge_bound = self.primal_feastol / TINY;
        let new_bound = if direction > 0.0 {
            self.compute_col_bounds(col, NONE, INF, 0, [false, false, true, false]).2
        } else {
            -self.compute_col_bounds(col, NONE, INF, 0, [false, false, false, true]).3
        };
        if new_bound != -INF && (new_bound >= direction * current_bound - self.primal_feastol || new_bound.abs() > huge_bound) {
            return None;
        }
        Some(new_bound * direction)
    }

    /// HiGHS's single-equation extension of dual fixing.
    fn handle_single_equation(&mut self, postsolve: &mut PostsolveStack, row: usize) -> Outcome {
        struct EquationNonzero {
            col: usize,
            val: f64,
            mark: i32,
        }
        let check_column = |s: &Self, col: usize, col_direction: f64| -> bool {
            if col_direction * s.model.col_cost[col] < 0.0 {
                return false;
            }
            let mut pos = s.colhead[col];
            while pos != NONE {
                let (r, value) = (s.a_row[pos], s.a_value[pos]);
                pos = s.a_next[pos];
                if r == row || s.is_redundant(r) {
                    continue;
                }
                if s.is_ranged(r) {
                    return false;
                }
                let row_direction = if s.model.row_lower[r] == -INF && s.model.row_upper[r] != INF { 1.0 } else { -1.0 };
                if col_direction * row_direction * value < 0.0 {
                    return false;
                }
            }
            true
        };
        let mut equation_row_vector = Vec::with_capacity(self.rowsize[row]);
        let mut num_s_plus = 0;
        let mut num_s_minus = 0;
        for (col, val) in self.row_entries(row) {
            let col_direction = 1.0f64.copysign(val);
            let mut mark = 0;
            if check_column(self, col, col_direction) {
                num_s_plus += 1;
                mark = 1;
            } else if check_column(self, col, -col_direction) {
                num_s_minus += 1;
                mark = -1;
            }
            equation_row_vector.push(EquationNonzero { col, val, mark });
        }
        if num_s_plus == 0 && num_s_minus == 0 {
            return Outcome::Ok;
        }
        let compute_activity = |s: &Self, col: usize, val: f64, activity: &mut CDouble, finite: &mut bool, direction: f64| {
            let bound = if direction * val > 0.0 { s.model.col_upper[col] } else { s.model.col_lower[col] };
            *finite = *finite && bound.abs() != INF;
            if *finite {
                *activity += CDouble::new(val) * bound;
            }
        };
        let mut activity_t_plus = CDouble::new(0.0);
        let mut activity_t_minus = CDouble::new(0.0);
        let mut activity_sc_plus = CDouble::new(0.0);
        let mut activity_sc_minus = CDouble::new(0.0);
        let mut t_plus_finite = true;
        let mut t_minus_finite = true;
        let mut sc_plus_finite = true;
        let mut sc_minus_finite = true;
        for nz in &equation_row_vector {
            if nz.mark <= 0 {
                compute_activity(self, nz.col, nz.val, &mut activity_t_plus, &mut t_plus_finite, 1.0);
            } else {
                compute_activity(self, nz.col, nz.val, &mut activity_sc_plus, &mut sc_plus_finite, -1.0);
            }
            if nz.mark >= 0 {
                compute_activity(self, nz.col, nz.val, &mut activity_t_minus, &mut t_minus_finite, -1.0);
            } else {
                compute_activity(self, nz.col, nz.val, &mut activity_sc_minus, &mut sc_minus_finite, 1.0);
            }
            if (num_s_minus == 0 || !t_plus_finite || !sc_plus_finite) && (num_s_plus == 0 || !t_minus_finite || !sc_minus_finite) {
                break;
            }
        }
        let fix_cols = |s: &mut Self, postsolve: &mut PostsolveStack, direction: i32| -> Outcome {
            for nz in &equation_row_vector {
                if direction * nz.mark >= 0 {
                    continue;
                }
                if direction as f64 * nz.val > 0.0 {
                    check!(s.fix_col_to_upper(postsolve, nz.col));
                } else {
                    check!(s.fix_col_to_lower(postsolve, nz.col));
                }
            }
            Outcome::Ok
        };
        let row_lower = self.model.row_lower[row];
        if num_s_minus > 0 && t_plus_finite && sc_plus_finite && (activity_t_plus + activity_sc_plus).value() <= row_lower + self.primal_feastol {
            check!(fix_cols(self, postsolve, 1));
        } else if num_s_plus > 0 && t_minus_finite && sc_minus_finite && (activity_t_minus + activity_sc_minus).value() >= row_lower - self.primal_feastol {
            check!(fix_cols(self, postsolve, -1));
        }
        Outcome::Ok
    }

    fn singleton_col_stuffing(&mut self, postsolve: &mut PostsolveStack, col: usize) -> Outcome {
        #[derive(Clone, Copy)]
        struct Candidate {
            col: usize,
            val: f64,
            multiplier: f64,
        }
        let is_singleton = |s: &Self, col: usize| !s.col_deleted[col] && s.colsize[col] == 1 && s.model.col_lower[col] != s.model.col_upper[col];
        if !is_singleton(self, col) {
            return Outcome::Ok;
        }
        let row = self.a_row[self.colhead[col]];
        if self.rowsize[row] <= 1 || self.is_ranged(row) {
            return Outcome::Ok;
        }
        let check_row = |s: &mut Self, postsolve: &mut PostsolveStack, rhs: f64, direction: f64| -> Outcome {
            if direction * rhs == INF {
                return Outcome::Ok;
            }
            // With continuous columns only, HiGHS's candidate search runs
            // once, and its result stands whenever both activity bounds
            // were not lost.
            let mut candidates: Vec<Candidate> = Vec::with_capacity(s.rowsize[row]);
            let mut sum_lower = CDouble::new(0.0);
            let mut sum_upper = CDouble::new(0.0);
            let mut sum_lower_finite = true;
            let mut sum_upper_finite = true;
            let mut complete = true;
            for (j, value) in s.row_entries(row) {
                let aj = direction * value;
                let cj = s.model.col_cost[j];
                let mut sum_lower_bound = s.model.col_lower[j];
                let mut sum_upper_bound = s.model.col_upper[j];
                if is_singleton(s, j) {
                    if aj > 0.0 {
                        sum_upper_bound = sum_lower_bound;
                        if cj < 0.0 {
                            candidates.push(Candidate { col: j, val: aj, multiplier: 1.0 });
                        }
                    } else {
                        sum_lower_bound = sum_upper_bound;
                        if cj > 0.0 {
                            candidates.push(Candidate {
                                col: j,
                                val: aj,
                                multiplier: -1.0,
                            });
                        }
                    }
                }
                if aj < 0.0 {
                    std::mem::swap(&mut sum_lower_bound, &mut sum_upper_bound);
                }
                sum_lower_finite = sum_lower_finite && sum_lower_bound.abs() != INF;
                sum_upper_finite = sum_upper_finite && sum_upper_bound.abs() != INF;
                if sum_lower_finite {
                    sum_lower += CDouble::new(sum_lower_bound) * aj;
                }
                if sum_upper_finite {
                    sum_upper += CDouble::new(sum_upper_bound) * aj;
                }
                if !sum_lower_finite && !sum_upper_finite {
                    complete = false;
                    break;
                }
            }
            if !complete {
                return Outcome::Ok;
            }
            {
                let cost = &s.model.col_cost;
                pdqsort(&mut candidates, |c1, c2| cost[c1.col] / c1.val < cost[c2.col] / c2.val);
            }
            for t in &candidates {
                if s.model.col_lower[t.col] == -INF || s.model.col_upper[t.col] == INF {
                    break;
                }
                let delta = (CDouble::new(s.model.col_upper[t.col]) - CDouble::new(s.model.col_lower[t.col])) * (t.multiplier * t.val);
                if sum_upper_finite && delta.value() <= (-sum_upper + direction * rhs + s.primal_feastol).value() {
                    if t.multiplier < 0.0 {
                        check!(s.fix_col_to_lower(postsolve, t.col));
                    } else {
                        check!(s.fix_col_to_upper(postsolve, t.col));
                    }
                } else if sum_lower_finite && delta.value() <= (sum_lower - direction * rhs + s.primal_feastol).value() {
                    if -t.multiplier < 0.0 {
                        check!(s.fix_col_to_lower(postsolve, t.col));
                    } else {
                        check!(s.fix_col_to_upper(postsolve, t.col));
                    }
                }
                if sum_lower_finite {
                    sum_lower += delta;
                }
                if sum_upper_finite {
                    sum_upper += delta;
                }
            }
            Outcome::Ok
        };
        let rhs = self.model.row_upper[row];
        check!(check_row(self, postsolve, rhs, 1.0));
        let rhs = self.model.row_lower[row];
        check!(check_row(self, postsolve, rhs, -1.0));
        Outcome::Ok
    }

    /// HiGHS's `computeColBounds`: the implied bounds on `col` from its
    /// rows that `want` asks for (lower, upper, worst-case lower,
    /// worst-case upper), with `bound_col` at `bound_col_value` if there
    /// is one.
    fn compute_col_bounds(&mut self, col: usize, bound_col: usize, bound_col_value: f64, bound_col_coeff_pattern: i32, want: [bool; 4]) -> (f64, f64, f64, f64) {
        struct Triplet {
            row: usize,
            jval: f64,
            kval: f64,
        }
        let lower_requested = want[0] || want[2];
        let upper_requested = want[1] || want[3];
        let mut nzs: Vec<Triplet> = Vec::with_capacity(self.colsize[col]);
        let skip_non_zero = |s: &Self, row: usize, val: f64| {
            let has_lower = s.yields_implied_lower_bound(row, val);
            let has_upper = s.yields_implied_upper_bound(row, val);
            (!upper_requested && lower_requested && !has_lower)
                || (!lower_requested && upper_requested && !has_upper)
                || (upper_requested && lower_requested && !has_lower && !has_upper)
        };
        let store_triplet = |nzs: &mut Vec<Triplet>, row: usize, jval: f64, kval: f64| {
            if bound_col_coeff_pattern != 0 && jval.is_sign_negative() != (bound_col_coeff_pattern as f64 * kval).is_sign_negative() {
                return;
            }
            nzs.push(Triplet { row, jval, kval });
        };
        if bound_col != NONE {
            if self.colsize[col] < self.colsize[bound_col] {
                let mut pos = self.colhead[col];
                while pos != NONE {
                    let (row, value) = (self.a_row[pos], self.a_value[pos]);
                    pos = self.a_next[pos];
                    if skip_non_zero(self, row, value) {
                        continue;
                    }
                    let nz_pos = self.find_nonzero(row, bound_col);
                    if nz_pos == NONE {
                        continue;
                    }
                    store_triplet(&mut nzs, row, value, self.a_value[nz_pos]);
                }
            } else {
                let mut pos = self.colhead[bound_col];
                while pos != NONE {
                    let (row, value) = (self.a_row[pos], self.a_value[pos]);
                    pos = self.a_next[pos];
                    let nz_pos = self.find_nonzero(row, col);
                    if nz_pos == NONE {
                        continue;
                    }
                    if skip_non_zero(self, row, self.a_value[nz_pos]) {
                        continue;
                    }
                    store_triplet(&mut nzs, row, self.a_value[nz_pos], value);
                }
            }
        } else {
            let mut pos = self.colhead[col];
            while pos != NONE {
                let (row, value) = (self.a_row[pos], self.a_value[pos]);
                pos = self.a_next[pos];
                if skip_non_zero(self, row, value) {
                    continue;
                }
                nzs.push(Triplet { row, jval: value, kval: -INF });
            }
        }
        let mut lower_bound = -INF;
        let mut upper_bound = INF;
        let mut worst_case_lower_bound = -INF;
        let mut worst_case_upper_bound = INF;
        let compute_bound = |s: &Self, t: &Triplet, rhs: f64, direction: f64, worst_case: bool| -> f64 {
            let b = col_bounds!(s);
            let residual;
            if (direction > 0.0 && !worst_case) || (direction < 0.0 && worst_case) {
                residual = if worst_case {
                    s.implied_row_bounds.residual_sum_lower(b, t.row, col, t.jval, bound_col, t.kval, bound_col_value)
                } else {
                    s.implied_row_bounds.residual_sum_lower_orig(b, t.row, col, t.jval, bound_col, t.kval, bound_col_value)
                };
                if residual == -INF {
                    return INF.copysign(t.jval);
                }
            } else {
                residual = if worst_case {
                    s.implied_row_bounds.residual_sum_upper(b, t.row, col, t.jval, bound_col, t.kval, bound_col_value)
                } else {
                    s.implied_row_bounds.residual_sum_upper_orig(b, t.row, col, t.jval, bound_col, t.kval, bound_col_value)
                };
                if residual == INF {
                    return -INF.copysign(t.jval);
                }
            }
            ((CDouble::new(rhs) - CDouble::new(residual)) / t.jval).value()
        };
        for t in &nzs {
            for (rhs, direction) in [(self.model.row_upper[t.row], 1.0), (self.model.row_lower[t.row], -1.0)] {
                if direction * rhs == INF {
                    continue;
                }
                if direction * t.jval < 0.0 {
                    if want[0] {
                        lower_bound = max(lower_bound, compute_bound(self, t, rhs, direction, false));
                    }
                    if want[2] && worst_case_lower_bound != INF {
                        worst_case_lower_bound = max(worst_case_lower_bound, compute_bound(self, t, rhs, direction, true));
                    }
                } else {
                    if want[1] {
                        upper_bound = min(upper_bound, compute_bound(self, t, rhs, direction, false));
                    }
                    if want[3] && worst_case_upper_bound != -INF {
                        worst_case_upper_bound = min(worst_case_upper_bound, compute_bound(self, t, rhs, direction, true));
                    }
                }
            }
        }
        (lower_bound, upper_bound, worst_case_lower_bound, worst_case_upper_bound)
    }
}

// ---- Passes ----------------------------------------------------------------

impl Presolve {
    fn remove_row_singletons(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let mut i = 0;
        while i < self.singleton_rows.len() {
            let row = self.singleton_rows[i];
            i += 1;
            if self.row_deleted[row] || self.rowsize[row] > 1 {
                continue;
            }
            check!(self.row_presolve(postsolve, row));
        }
        self.singleton_rows.clear();
        Outcome::Ok
    }

    fn presolve_col_singletons(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let mut i = 0;
        while i < self.singleton_columns.len() {
            let col = self.singleton_columns[i];
            i += 1;
            if self.col_deleted[col] {
                continue;
            }
            check!(self.col_presolve(postsolve, col));
        }
        let (col_deleted, colsize) = (&self.col_deleted, &self.colsize);
        self.singleton_columns.retain(|&col| !(col_deleted[col] || colsize[col] > 1));
        Outcome::Ok
    }

    fn presolve_changed_rows(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let mut changed_rows = Vec::with_capacity(self.model.num_row - self.num_deleted_rows);
        std::mem::swap(&mut changed_rows, &mut self.changed_row_indices);
        for row in changed_rows {
            if self.row_deleted[row] {
                continue;
            }
            check!(self.row_presolve(postsolve, row));
            self.changed_row_flag[row] = self.row_deleted[row];
        }
        Outcome::Ok
    }

    fn presolve_changed_cols(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let mut changed_cols = Vec::with_capacity(self.model.num_col - self.num_deleted_cols);
        std::mem::swap(&mut changed_cols, &mut self.changed_col_indices);
        for col in changed_cols {
            if self.col_deleted[col] {
                continue;
            }
            check!(self.col_presolve(postsolve, col));
            self.changed_col_flag[col] = self.col_deleted[col];
        }
        Outcome::Ok
    }

    fn remove_doubleton_equations(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let mut eq = self.equations.iter().next().copied();
        while let Some(key) = eq {
            let eqrow = key.1;
            if self.rowsize[eqrow] > 2 {
                return Outcome::Ok;
            }
            check!(self.row_presolve(postsolve, eqrow));
            eq = if self.row_deleted[eqrow] {
                self.equations.iter().next().copied()
            } else {
                self.equations.range((Excluded(key), Unbounded)).next().copied()
            };
        }
        Outcome::Ok
    }

    fn initial_row_and_col_presolve(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        for row in 0..self.model.num_row {
            if self.row_deleted[row] {
                continue;
            }
            check!(self.row_presolve(postsolve, row));
            self.changed_row_flag[row] = false;
        }
        for col in 0..self.model.num_col {
            if self.col_deleted[col] {
                continue;
            }
            check!(self.col_presolve(postsolve, col));
            self.changed_col_flag[col] = false;
        }
        self.check_limits(postsolve)
    }

    fn fast_presolve_loop(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        loop {
            self.store_current_problem_size();
            check!(self.remove_row_singletons(postsolve));
            check!(self.presolve_changed_rows(postsolve));
            check!(self.remove_doubleton_equations(postsolve));
            check!(self.presolve_col_singletons(postsolve));
            check!(self.presolve_changed_cols(postsolve));
            // Not `<=`: a reduction of NaN (nothing left to reduce) stops it.
            if self.problem_size_reduction().partial_cmp(&0.01).is_none_or(std::cmp::Ordering::is_le) {
                return Outcome::Ok;
            }
        }
    }

    fn aggregator(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let mut opportunities = std::mem::take(&mut self.substitution_opportunities);
        opportunities.retain(|&(row, col)| !(self.row_deleted[row] || self.col_deleted[col] || !self.is_implied_free(col) || !self.is_dual_implied_free(row)));
        {
            let (rowsize, colsize) = (&self.rowsize, &self.colsize);
            let key = |&(row, col): &(usize, usize)| (hash::pair_hash(1, row as u32, col as u32) ^ (hash::pair_hash(0, row as u32, col as u32) >> 32), row, col);
            pdqsort(&mut opportunities, |nz1, nz2| {
                let min_len1 = rowsize[nz1.0].min(colsize[nz1.1]);
                let min_len2 = rowsize[nz2.0].min(colsize[nz2.1]);
                if min_len1 == 2 && min_len2 != 2 {
                    return true;
                }
                if min_len2 == 2 && min_len1 != 2 {
                    return false;
                }
                let size_prod1 = rowsize[nz1.0] as i64 * colsize[nz1.1] as i64;
                let size_prod2 = rowsize[nz2.0] as i64 * colsize[nz2.1] as i64;
                if size_prod1 < size_prod2 {
                    return true;
                }
                if size_prod2 < size_prod1 {
                    return false;
                }
                if min_len1 < min_len2 {
                    return true;
                }
                if min_len2 < min_len1 {
                    return false;
                }
                key(nz1) < key(nz2)
            });
        }
        self.substitution_opportunities = opportunities;

        let mut nfail = 0;
        let mut i = 0;
        while i < self.substitution_opportunities.len() {
            let (row, col) = self.substitution_opportunities[i];
            i += 1;
            if self.row_deleted[row] || self.col_deleted[col] || !self.is_implied_free(col) || !self.is_dual_implied_free(row) {
                self.substitution_opportunities[i - 1].0 = NONE;
                continue;
            }
            let nz_pos = self.find_nonzero(row, col);
            if nz_pos == NONE {
                self.substitution_opportunities[i - 1].0 = NONE;
                continue;
            }
            if self.rowsize[row] == 2 || self.colsize[col] == 2 {
                self.store_row(row);
                self.substitute_free_col(postsolve, row, col, true);
                self.substitution_opportunities[i - 1].0 = NONE;
                check!(self.remove_row_singletons(postsolve));
                check!(self.check_limits(postsolve));
                continue;
            }
            let row_shorter = self.rowsize[row] < self.colsize[col];
            let mut max_val = if row_shorter { self.get_max_abs_row_val(row) } else { self.get_max_abs_col_val(col) };
            if self.a_value[nz_pos].abs() < max_val * self.settings.pivot_threshold {
                max_val = if row_shorter { self.get_max_abs_col_val(col) } else { self.get_max_abs_row_val(row) };
                if self.a_value[nz_pos].abs() < max_val * self.settings.pivot_threshold {
                    self.substitution_opportunities[i - 1].0 = NONE;
                    continue;
                }
            }
            self.store_row(row);
            let mut fillin = -((self.rowsize[row] + self.colsize[col]) as i64 - 1);
            let mut pos = self.colhead[col];
            while pos != NONE {
                let other = self.a_row[pos];
                pos = self.a_next[pos];
                if other == row {
                    continue;
                }
                fillin += self.count_fillin(other);
                if fillin > self.settings.max_fillin {
                    break;
                }
            }
            if fillin > self.settings.max_fillin {
                nfail += 1;
                if nfail == 3 {
                    break;
                }
                continue;
            }
            nfail = 0;
            self.substitute_free_col(postsolve, row, col, true);
            self.substitution_opportunities[i - 1].0 = NONE;
            check!(self.remove_row_singletons(postsolve));
            check!(self.check_limits(postsolve));
        }
        self.substitution_opportunities.retain(|&(row, _)| row != NONE);
        Outcome::Ok
    }

    fn count_fillin(&mut self, row: usize) -> i64 {
        let mut fillin = 0;
        for k in 0..self.rowpositions.len() {
            let col = self.a_col[self.rowpositions[k]];
            if self.find_nonzero(row, col) == NONE {
                fillin += 1;
            }
        }
        fillin
    }

    fn detect_parallel_rows_and_cols(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        let small = self.settings.small_matrix_value;
        let tol = self.settings.dual_feasibility_tolerance;
        let feastol = self.primal_feastol;
        let mut row_hashes: Vec<u64> = self.rowsize.iter().map(|&size| size as u64).collect();
        let mut col_hashes: Vec<u64> = self.colsize.iter().map(|&size| size as u64).collect();
        let mut row_max: Vec<(f64, usize)> = vec![(0.0, 0); self.rowsize.len()];
        let mut col_max: Vec<(f64, usize)> = vec![(0.0, 0); self.colsize.len()];
        let mut num_row_singletons: HashMap<usize, i64> = HashMap::new();
        let nnz = self.a_value.len();

        for i in 0..nnz {
            if self.a_value[i] == 0.0 {
                continue;
            }
            let (row, col, value) = (self.a_row[i], self.a_col[i], self.a_value[i]);
            if self.colsize[col] == 1 {
                col_max[col].0 = value;
                row_hashes[row] = row_hashes[row].wrapping_sub(1);
                *num_row_singletons.entry(row).or_insert(0) += 1;
                continue;
            }
            let abs_val = value.abs();
            let abs_row_max = row_max[row].0.abs();
            if abs_val >= abs_row_max - small && (abs_val > abs_row_max + small || col < row_max[row].1) {
                row_max[row] = (value, col);
            }
            let abs_col_max = col_max[col].0.abs();
            if abs_val >= abs_col_max - small && (abs_val > abs_col_max + small || row < col_max[col].1) {
                col_max[col] = (value, row);
            }
        }
        for i in 0..nnz {
            if self.a_value[i] == 0.0 {
                continue;
            }
            let (row, col, value) = (self.a_row[i], self.a_col[i], self.a_value[i]);
            if self.colsize[col] == 1 {
                col_hashes[col] = row as u64;
            } else {
                hash::sparse_combine_value(&mut row_hashes[row], col, hash::double_hash_code(value / row_max[row].0) as u64);
                hash::sparse_combine_value(&mut col_hashes[col], row, hash::double_hash_code(value / col_max[col].0) as u64);
            }
        }

        // HiGHS's unordered_multimap of hashes: equal hashes in the order
        // they were filed.
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        for i in 0..self.model.num_col {
            if self.col_deleted[i] {
                continue;
            }
            if self.colsize[i] == 0 {
                check!(self.col_presolve(postsolve, i));
                continue;
            }
            let candidates = buckets.get(&col_hashes[i]).cloned().unwrap_or_default();
            let mut del_col = NONE;
            let mut last = NONE;
            for (k, &parallel_col_candidate) in candidates.iter().enumerate() {
                last = k;
                if self.colsize[i] != self.colsize[parallel_col_candidate] {
                    continue;
                }
                let col = parallel_col_candidate;
                let duplicate_col = i;
                let col_scale = col_max[duplicate_col].0 / col_max[col].0;
                let col_upper_inf = |s: &Self| {
                    if col_scale > 0.0 {
                        s.is_upper_strictly_implied(col, feastol)
                    } else {
                        s.is_lower_strictly_implied(col, feastol)
                    }
                };
                let col_lower_inf = |s: &Self| {
                    if col_scale > 0.0 {
                        s.is_lower_strictly_implied(col, feastol)
                    } else {
                        s.is_upper_strictly_implied(col, feastol)
                    }
                };
                let duplicate_col_upper_inf = |s: &Self| s.is_upper_strictly_implied(duplicate_col, feastol);
                let duplicate_col_lower_inf = |s: &Self| s.is_lower_strictly_implied(duplicate_col, feastol);
                let obj_diff = (CDouble::new(col_scale) * self.model.col_cost[col] - self.model.col_cost[duplicate_col]).value();
                const MERGE: u8 = 0;
                const COL_TO_UPPER: u8 = 1;
                const COL_TO_LOWER: u8 = 2;
                const DUPLICATE_TO_UPPER: u8 = 3;
                const DUPLICATE_TO_LOWER: u8 = 4;
                let (cl, cu, dl, du) = (
                    self.model.col_lower[col],
                    self.model.col_upper[col],
                    self.model.col_lower[duplicate_col],
                    self.model.col_upper[duplicate_col],
                );
                let scaled_to = |up: bool| if (col_scale > 0.0) == up { COL_TO_UPPER } else { COL_TO_LOWER };
                let reduction_case;
                if obj_diff < -tol {
                    if col_upper_inf(self) && dl != -INF {
                        reduction_case = DUPLICATE_TO_LOWER;
                    } else if duplicate_col_lower_inf(self) && (col_scale < 0.0 || cu != INF) && (col_scale > 0.0 || cl != -INF) {
                        reduction_case = scaled_to(true);
                    } else {
                        continue;
                    }
                } else if obj_diff > tol {
                    if col_lower_inf(self) && du != INF {
                        reduction_case = DUPLICATE_TO_UPPER;
                    } else if duplicate_col_upper_inf(self) && (col_scale < 0.0 || cl != -INF) && (col_scale > 0.0 || cu != INF) {
                        reduction_case = scaled_to(false);
                    } else {
                        continue;
                    }
                } else if col_upper_inf(self) && dl != -INF {
                    reduction_case = DUPLICATE_TO_LOWER;
                } else if col_lower_inf(self) && du != INF {
                    reduction_case = DUPLICATE_TO_UPPER;
                } else if duplicate_col_upper_inf(self) && (col_scale < 0.0 || cl != -INF) && (col_scale > 0.0 || cu != INF) {
                    reduction_case = scaled_to(false);
                } else if duplicate_col_lower_inf(self) && (col_scale < 0.0 || cu != INF) && (col_scale > 0.0 || cl != -INF) {
                    reduction_case = scaled_to(true);
                } else {
                    reduction_case = MERGE;
                }

                let mut parallel = true;
                let mut pos = self.colhead[col];
                while pos != NONE {
                    let (row, value) = (self.a_row[pos], self.a_value[pos]);
                    pos = self.a_next[pos];
                    let duplicate_pos = self.find_nonzero(row, duplicate_col);
                    parallel = duplicate_pos != NONE;
                    if !parallel {
                        break;
                    }
                    parallel = (CDouble::new(self.a_value[duplicate_pos]) - CDouble::new(col_scale) * value).value().abs() <= small;
                    if !parallel {
                        break;
                    }
                }
                if !parallel {
                    continue;
                }
                let mut unsingleton = |s: &Self, c: usize| {
                    if s.colsize[c] == 1 {
                        *num_row_singletons.entry(s.a_row[s.colhead[c]]).or_insert(0) -= 1;
                    }
                };
                match reduction_case {
                    DUPLICATE_TO_LOWER => {
                        del_col = duplicate_col;
                        unsingleton(self, duplicate_col);
                        check!(self.fix_col_to_lower(postsolve, duplicate_col));
                    }
                    DUPLICATE_TO_UPPER => {
                        del_col = duplicate_col;
                        unsingleton(self, duplicate_col);
                        check!(self.fix_col_to_upper(postsolve, duplicate_col));
                    }
                    COL_TO_LOWER => {
                        del_col = col;
                        unsingleton(self, col);
                        check!(self.fix_col_to_lower(postsolve, col));
                    }
                    COL_TO_UPPER => {
                        del_col = col;
                        unsingleton(self, col);
                        check!(self.fix_col_to_upper(postsolve, col));
                    }
                    _ => {
                        // HiGHS merges even when the postsolve stack
                        // refuses the merge (it only reports that in debug).
                        postsolve.duplicate_column(col_scale, cl, cu, dl, du, col, duplicate_col);
                        self.mark_changed_col(col);
                        unsingleton(self, duplicate_col);
                        let (merge_lower, merge_upper) = if col_scale > 0.0 {
                            (cl + col_scale * dl, cu + col_scale * du)
                        } else {
                            (cl + col_scale * du, cu + col_scale * dl)
                        };
                        check!(self.change_col_bounds(col, merge_lower, merge_upper));
                        self.mark_col_deleted(duplicate_col);
                        let mut coliter = self.colhead[duplicate_col];
                        while coliter != NONE {
                            let colpos = coliter;
                            let colrow = self.a_row[coliter];
                            coliter = self.a_next[coliter];
                            self.unlink(colpos);
                            self.reinsert_equation(colrow);
                        }
                        self.model.col_cost[duplicate_col] = 0.0;
                        del_col = duplicate_col;
                        self.reset_col_implied_bounds(col, NONE);
                    }
                }
                break;
            }
            if del_col != NONE {
                if del_col != i {
                    buckets.get_mut(&col_hashes[i]).unwrap().remove(last);
                }
                check!(self.check_limits(postsolve));
                check!(self.remove_row_singletons(postsolve));
            } else {
                buckets.entry(col_hashes[i]).or_default().push(i);
            }
        }

        buckets.clear();
        let num_singletons = |map: &HashMap<usize, i64>, row: usize| map.get(&row).copied().unwrap_or(0);
        for i in 0..self.model.num_row {
            if self.row_deleted[i] {
                continue;
            }
            if self.rowsize[i] <= 1 || (self.rowsize[i] == 2 && self.is_equation(i)) {
                check!(self.row_presolve(postsolve, i));
                continue;
            }
            // Rows with extra singletons need reductions whose postsolve
            // gives no basis.
            let num_singleton = num_singletons(&num_row_singletons, i);
            if num_singleton != 0 {
                continue;
            }
            let candidates = buckets.get(&row_hashes[i]).cloned().unwrap_or_default();
            let mut del_row = NONE;
            let mut last = NONE;
            if !candidates.is_empty() {
                self.store_row(i);
            }
            for (k, &parallel_row_cand) in candidates.iter().enumerate() {
                last = k;
                let num_singleton_candidate = num_singletons(&num_row_singletons, parallel_row_cand);
                if num_singleton_candidate != 0 {
                    continue;
                }
                if self.rowsize[i] as i64 - num_singleton != self.rowsize[parallel_row_cand] as i64 - num_singleton_candidate {
                    continue;
                }
                let row_scale = row_max[parallel_row_cand].0 / row_max[i].0;
                let mut parallel = true;
                for k in 0..self.rowpositions.len() {
                    let pos = self.rowpositions[k];
                    let (col, value) = (self.a_col[pos], self.a_value[pos]);
                    if self.colsize[col] == 1 {
                        continue;
                    }
                    let nz_pos = self.find_nonzero(parallel_row_cand, col);
                    parallel = nz_pos != NONE;
                    if !parallel {
                        break;
                    }
                    parallel = (CDouble::new(self.a_value[nz_pos]) - CDouble::new(row_scale) * value).value().abs() <= small;
                    if !parallel {
                        break;
                    }
                }
                if !parallel {
                    continue;
                }
                let mut row_lower_tightened = false;
                let mut row_upper_tightened = false;
                let (mut new_lower, mut new_upper) = if row_scale > 0.0 {
                    (self.model.row_lower[i] * row_scale, self.model.row_upper[i] * row_scale)
                } else {
                    (self.model.row_upper[i] * row_scale, self.model.row_lower[i] * row_scale)
                };
                let cand = parallel_row_cand;
                if new_upper < self.model.row_upper[cand] {
                    if new_upper < self.model.row_lower[cand] - feastol {
                        return Outcome::PrimalInfeasible;
                    }
                    if new_upper <= self.model.row_lower[cand] + feastol {
                        new_upper = self.model.row_lower[cand];
                    }
                    if new_upper < self.model.row_upper[cand] {
                        row_upper_tightened = true;
                        if row_scale > 0.0 {
                            let tmp = self.row_dual_lower[i] / row_scale;
                            self.row_dual_lower[i] = self.row_dual_lower[cand] * row_scale;
                            self.row_dual_lower[cand] = tmp;
                        } else {
                            let tmp = self.row_dual_upper[i] / row_scale;
                            self.row_dual_upper[i] = self.row_dual_lower[cand] * row_scale;
                            self.row_dual_lower[cand] = tmp;
                        }
                        self.model.row_upper[cand] = new_upper;
                    }
                }
                if new_lower > self.model.row_lower[cand] {
                    if new_lower > self.model.row_upper[cand] + feastol {
                        return Outcome::PrimalInfeasible;
                    }
                    if new_lower >= self.model.row_upper[cand] - feastol {
                        new_lower = self.model.row_upper[cand];
                    }
                    if new_lower > self.model.row_lower[cand] {
                        row_lower_tightened = true;
                        if row_scale > 0.0 {
                            let tmp = self.row_dual_upper[i] / row_scale;
                            self.row_dual_upper[i] = self.row_dual_upper[cand] * row_scale;
                            self.row_dual_upper[cand] = tmp;
                        } else {
                            let tmp = self.row_dual_lower[i] / row_scale;
                            self.row_dual_lower[i] = self.row_dual_upper[cand] * row_scale;
                            self.row_dual_upper[cand] = tmp;
                        }
                        self.model.row_lower[cand] = new_lower;
                    }
                }
                self.reset_row_dual_implied_bounds(cand, NONE);
                postsolve.duplicate_row(cand, row_upper_tightened, row_lower_tightened, i, row_scale);
                del_row = i;
                self.mark_row_deleted(i);
                for k in 0..self.rowpositions.len() {
                    let pos = self.rowpositions[k];
                    self.unlink(pos);
                }
                break;
            }
            if del_row != NONE {
                if del_row != i {
                    buckets.get_mut(&row_hashes[i]).unwrap().remove(last);
                }
                check!(self.check_limits(postsolve));
            } else {
                buckets.entry(row_hashes[i]).or_default().push(i);
            }
        }
        Outcome::Ok
    }

    /// HiGHS's `removeDependentEquations`: factor the equations, each
    /// with its right-hand side, as columns; those left without a pivot
    /// depend on the others and are removed.
    fn remove_dependent_equations(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        if self.equations.is_empty() {
            return Outcome::Ok;
        }
        let num_eq = self.equations.len();
        let rhs_row = self.model.num_col;
        let capacity = self.num_nonzeros() + num_eq;
        let mut start = Vec::with_capacity(num_eq + 1);
        let mut index = Vec::with_capacity(capacity);
        let mut value = Vec::with_capacity(capacity);
        start.push(0);
        let eq_set: Vec<usize> = self.equations.iter().map(|&(_, eq)| eq).collect();
        for &eq in &eq_set {
            for pos in self.row_vector(eq) {
                value.push(self.a_value[pos]);
                index.push(self.a_col[pos]);
            }
            if self.model.row_lower[eq] != 0.0 {
                value.push(self.model.row_lower[eq]);
                index.push(rhs_row);
            }
            start.push(value.len());
        }
        let mut col_set: Vec<usize> = (0..num_eq).collect();
        let mut factor = crate::factor::Factor::default();
        factor.setup_general(
            num_eq,
            self.model.num_col + 1,
            num_eq,
            crate::factor::DEFAULT_PIVOT_THRESHOLD,
            crate::factor::DEFAULT_PIVOT_TOLERANCE,
        );
        // HiGHS bounds the search's time, and skips it when the factor
        // expects to take longer.
        factor.time_limit = Some(1.0f64.max((0.01 * self.settings.time_limit).min(1000.0)));
        let rank_deficiency = factor.build(
            crate::factor::ColMatrix {
                start: &start,
                index: &index,
                value: &value,
            },
            &mut col_set,
        );
        if factor.timed_out {
            return Outcome::Ok;
        }
        for var_with_no_pivot_k in &factor.var_with_no_pivot[..rank_deficiency] {
            let var = *var_with_no_pivot_k;
            if var != NONE {
                let redundant_row = eq_set[var];
                postsolve.redundant_row(redundant_row);
                self.remove_row(redundant_row);
            }
        }
        Outcome::Ok
    }

    fn shrink_and_rebuild_if_halved(&mut self, postsolve: &mut PostsolveStack) {
        if self.shrink_problem_enabled && (self.num_deleted_cols >= self.model.num_col / 2 || self.num_deleted_rows >= self.model.num_row / 2) {
            self.shrink_problem(postsolve);
            self.rebuild_storage();
        }
    }

    /// HiGHS's `presolve` for an LP.
    fn presolve(&mut self, postsolve: &mut PostsolveStack) -> Outcome {
        check!(self.initial_row_and_col_presolve(postsolve));
        let mut num_parallel_row_col_calls = 0;
        let mut dependent_equations_called = false;
        loop {
            check!(self.fast_presolve_loop(postsolve));
            self.store_current_problem_size();
            check!(self.aggregator(postsolve));
            if self.problem_size_reduction() > 0.05 {
                continue;
            }
            if num_parallel_row_col_calls < 5 {
                self.shrink_and_rebuild_if_halved(postsolve);
                self.store_current_problem_size();
                check!(self.detect_parallel_rows_and_cols(postsolve));
                num_parallel_row_col_calls += 1;
                if self.problem_size_reduction() > 0.05 {
                    continue;
                }
            }
            check!(self.fast_presolve_loop(postsolve));
            check!(self.fast_presolve_loop(postsolve));
            if !dependent_equations_called {
                self.shrink_and_rebuild_if_halved(postsolve);
                self.store_current_problem_size();
                check!(self.remove_dependent_equations(postsolve));
                dependent_equations_called = true;
                if self.problem_size_reduction() > 0.05 {
                    continue;
                }
            }
            break;
        }
        Outcome::Ok
    }

    fn timed_out(&self) -> bool {
        self.settings.time_limit < f64::INFINITY && self.started.elapsed().as_secs_f64() >= self.settings.time_limit
    }

    /// HiGHS's `HPresolve::run`: presolve, then compact the reduced LP
    /// into `model`.
    pub fn run(&mut self, postsolve: &mut PostsolveStack) -> PresolveStatus {
        self.shrink_problem_enabled = true;
        match self.presolve(postsolve) {
            Outcome::Stopped if self.timed_out() => return PresolveStatus::Timeout,
            Outcome::Ok | Outcome::Stopped => {}
            Outcome::PrimalInfeasible => return PresolveStatus::Infeasible,
            Outcome::DualInfeasible => return PresolveStatus::UnboundedOrInfeasible,
        }
        self.shrink_problem(postsolve);
        self.write_csc();
        if self.model.num_col == 0 {
            PresolveStatus::ReducedToEmpty
        } else if postsolve.num_reductions() > 0 {
            PresolveStatus::Reduced
        } else {
            PresolveStatus::NotReduced
        }
    }
}
