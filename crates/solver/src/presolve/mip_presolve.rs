//! HiGHS's `HPresolve` (`presolve/HPresolve.cpp`) as the MIP solver runs
//! it: the presolve over the solver's model, with its integer reductions
//! (rounding, implied integers, probing, the clique table, dominated
//! columns, coefficient strengthening and sparsification). It changes the
//! solver's data in place as HiGHS's does: the model, the global domain,
//! the clique table and the implications. The LP presolve
//! (`hpresolve.rs`) is the same code without the integer branches.

use std::{
    collections::{BTreeSet, HashMap},
    ops::Bound::{Excluded, Unbounded},
};

use super::{
    NONE,
    matrix::{self, Trees},
    max, min,
    mip_postsolve::{MipPostsolveStack, RowType},
    sum_bounds::{Bounds, SumBounds, impl_var_lower, impl_var_upper},
};
use crate::{
    hash_table::hash_i32,
    integers::{fractionality, gcd, integral_scale, mod_f64, mod_i64, modular_inverse},
    mip::{
        BoundType, DomainChange, VarType,
        clique_table::{CliqueVar, NO_ORIGIN},
        conflict_pool::ConflictPool,
        cut_pool::CutPool,
        data::MipData,
        domain::{Domain, Reason},
        objective_function::ObjectiveFunction,
    },
    pdqsort::pdqsort,
    util::{CDouble, Random, hash},
};

/// HiGHS's `kHighsTiny`.
const TINY: f64 = 1e-14;
const INF: f64 = f64::INFINITY;

/// HiGHS's `HPresolve::Result`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Ok,
    PrimalInfeasible,
    DualInfeasible,
    Stopped,
}

/// HiGHS's `HighsModelStatus` as presolve's `run` returns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PresolveStatus {
    NotReduced,
    Reduced,
    ReducedToEmpty,
    Infeasible,
    UnboundedOrInfeasible,
}

/// HiGHS's `StatusResult`: a flag, or the outcome that ended the check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StatusResult {
    flag: bool,
    result: Outcome,
}

impl StatusResult {
    fn flag(flag: bool) -> Self {
        Self { flag, result: Outcome::Ok }
    }

    fn result(result: Outcome) -> Self {
        Self {
            flag: result == Outcome::Ok,
            result,
        }
    }
}

/// The options presolve reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    /// HiGHS's `mip_feasibility_tolerance`.
    pub primal_feasibility_tolerance: f64,
    pub dual_feasibility_tolerance: f64,
    pub small_matrix_value: f64,
    /// HiGHS's `presolve_pivot_threshold`.
    pub pivot_threshold: f64,
    /// HiGHS's `presolve_substitution_maxfillin`.
    pub max_fillin: i64,
    /// The solve's time limit, in seconds.
    pub time_limit: f64,
    pub random_seed: u32,
    pub mip_pool_age_limit: usize,
    pub mip_pool_soft_limit: usize,
}

impl Settings {
    pub fn from_options(options: &crate::mip::data::MipOptions) -> Self {
        Self {
            primal_feasibility_tolerance: options.mip_feasibility_tolerance,
            dual_feasibility_tolerance: 1e-7,
            small_matrix_value: options.small_matrix_value,
            pivot_threshold: 0.01,
            max_fillin: 10,
            time_limit: options.time_limit,
            random_seed: options.random_seed,
            mip_pool_age_limit: options.mip_pool_age_limit,
            mip_pool_soft_limit: options.mip_pool_soft_limit,
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
            lower: &$s.mip.model.col_lower,
            upper: &$s.mip.model.col_upper,
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

pub(crate) struct MipPresolve<'a> {
    /// The solver's data; presolve works on its model (HiGHS's
    /// `presolvedModel`).
    pub mip: &'a mut MipData,
    /// The solver's postsolve stack (HiGHS's `mipdata_->postSolveStack`),
    /// held here while presolve runs and handed back when it is dropped.
    pub ps: MipPostsolveStack,
    /// The original model's row count (HiGHS's `orig_model_->num_row_`).
    orig_num_row: usize,
    settings: Settings,
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
    rowsize_integer: Vec<usize>,
    rowsize_impl_int: Vec<usize>,
    colsize: Vec<usize>,
    /// HiGHS's `rowpositions`: the stored row is its first
    /// `rowpositions_len` entries. A shorter row stored later leaves the
    /// tail as it was, as the C++ vector's memory does, which a stored row
    /// slice taken before still reads.
    rowpositions: Vec<usize>,
    rowpositions_len: usize,
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
    // HiGHS's `liftingOpportunities` only fill when
    // `mip_lifting_for_probing` is on, which it is not by default.
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
    num_probes: Vec<u16>,
    probing_contingent: i64,
    probing_num_del_col: usize,
    num_probed: usize,
    num_deleted_rows: usize,
    num_deleted_cols: usize,
    old_num_col: usize,
    old_num_row: usize,
    probing_early_abort: bool,
    /// Scratch for a row's positions.
    row_buffer: Vec<usize>,
}

impl Drop for MipPresolve<'_> {
    fn drop(&mut self) {
        std::mem::swap(&mut self.mip.postsolve, &mut self.ps);
    }
}

impl<'a> MipPresolve<'a> {
    /// HiGHS's `okSetInput(mipsolver, presolve_reduction_limit)`: presolve
    /// over the solver's model, whose bounds the global domain's replace
    /// when presolve has run on it before.
    pub fn new(mip: &'a mut MipData, orig_num_row: usize, first_presolve: bool, presolve_reduction_limit: i64) -> Self {
        if !first_presolve {
            mip.model.col_lower = mip.domain.col_lower.clone();
            mip.model.col_upper = mip.domain.col_upper.clone();
        }
        let settings = Settings::from_options(&mip.options);
        let ps = std::mem::take(&mut mip.postsolve);
        let num_col = mip.model.num_col;
        let num_row = mip.model.num_row;
        let mut row_dual_lower = vec![-INF; num_row];
        let mut row_dual_upper = vec![INF; num_row];
        for i in 0..num_row {
            if mip.model.row_lower[i] == -INF {
                row_dual_upper[i] = 0.0;
            }
            if mip.model.row_upper[i] == INF {
                row_dual_lower[i] = 0.0;
            }
        }
        let mut presolve = MipPresolve {
            mip,
            ps,
            orig_num_row,
            settings,
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
            rowsize_integer: Vec::new(),
            rowsize_impl_int: Vec::new(),
            colsize: Vec::new(),
            rowpositions: Vec::new(),
            rowpositions_len: 0,
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
            reduction_limit: if presolve_reduction_limit < 0 { usize::MAX } else { presolve_reduction_limit as usize },
            singleton_rows: Vec::new(),
            singleton_columns: Vec::new(),
            row_deleted: vec![false; num_row],
            col_deleted: vec![false; num_col],
            single_equation_checked: vec![false; num_row],
            num_probes: vec![0; num_col],
            probing_contingent: 1000,
            probing_num_del_col: 0,
            num_probed: 0,
            num_deleted_rows: 0,
            num_deleted_cols: 0,
            old_num_col: 0,
            old_num_row: 0,
            probing_early_abort: false,
            row_buffer: Vec::new(),
        };
        let (start, index, value) = (
            std::mem::take(&mut presolve.mip.model.a_start),
            std::mem::take(&mut presolve.mip.model.a_index),
            std::mem::take(&mut presolve.mip.model.a_value),
        );
        presolve.load_csc(&value, &index, &start);
        presolve.mip.model.a_start = start;
        presolve.mip.model.a_index = index;
        presolve.mip.model.a_value = value;
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

    fn row_coefficients_integral(&mut self, row: usize, scale: f64) -> bool {
        for pos in self.row_vector(row) {
            if fractionality(self.a_value[pos] * scale) > self.settings.small_matrix_value {
                return false;
            }
        }
        true
    }

    // ---- Queries -------------------------------------------------------

    fn is_lower_implied(&self, col: usize) -> bool {
        self.mip.model.col_lower[col] == -INF || self.impl_col_lower[col] >= self.mip.model.col_lower[col] - self.primal_feastol
    }

    fn is_lower_strictly_implied(&self, col: usize, tolerance: f64) -> bool {
        self.mip.model.col_lower[col] == -INF || self.impl_col_lower[col] > self.mip.model.col_lower[col] + tolerance
    }

    fn is_upper_implied(&self, col: usize) -> bool {
        self.mip.model.col_upper[col] == INF || self.impl_col_upper[col] <= self.mip.model.col_upper[col] + self.primal_feastol
    }

    fn is_upper_strictly_implied(&self, col: usize, tolerance: f64) -> bool {
        self.mip.model.col_upper[col] == INF || self.impl_col_upper[col] < self.mip.model.col_upper[col] - tolerance
    }

    fn is_implied_free(&self, col: usize) -> bool {
        self.is_lower_implied(col) && self.is_upper_implied(col)
    }

    fn is_dual_implied_free(&self, row: usize) -> bool {
        let tol = self.settings.dual_feasibility_tolerance;
        self.is_equation(row)
            || (self.mip.model.row_upper[row] != INF && self.impl_row_dual_upper[row] <= tol)
            || (self.mip.model.row_lower[row] != -INF && self.impl_row_dual_lower[row] >= -tol)
    }

    fn dual_implied_free_get_rhs_and_row_type(&mut self, row: usize, relax_row_dual_bounds: bool) -> (f64, RowType) {
        let tol = self.settings.dual_feasibility_tolerance;
        if self.is_equation(row) {
            (self.mip.model.row_upper[row], RowType::Eq)
        } else if self.mip.model.row_upper[row] != INF && self.impl_row_dual_upper[row] <= tol {
            let rhs = self.mip.model.row_upper[row];
            if relax_row_dual_bounds {
                self.change_row_dual_upper(row, INF);
            }
            (rhs, RowType::Leq)
        } else {
            let rhs = self.mip.model.row_lower[row];
            if relax_row_dual_bounds {
                self.change_row_dual_lower(row, -INF);
            }
            (rhs, RowType::Geq)
        }
    }

    fn is_equation(&self, row: usize) -> bool {
        self.mip.model.row_lower[row] == self.mip.model.row_upper[row]
    }

    fn is_ranged(&self, row: usize) -> bool {
        self.mip.model.row_lower[row] != -INF && self.mip.model.row_upper[row] != INF
    }

    fn is_redundant(&self, row: usize) -> bool {
        self.implied_row_bounds.sum_lower(row, CDouble::default()) >= self.mip.model.row_lower[row] - self.primal_feastol
            && self.implied_row_bounds.sum_upper(row, CDouble::default()) <= self.mip.model.row_upper[row] + self.primal_feastol
    }

    fn yields_implied_lower_bound(&self, row: usize, val: f64) -> bool {
        (val < 0.0 && self.mip.model.row_upper[row] != INF) || (val > 0.0 && self.mip.model.row_lower[row] != -INF)
    }

    fn yields_implied_upper_bound(&self, row: usize, val: f64) -> bool {
        self.yields_implied_lower_bound(row, -val)
    }

    // Only on a side the row has (HiGHS #3385; the other case is dual
    // infeasible).
    fn is_implied_equation_at_lower(&self, row: usize) -> bool {
        self.mip.model.row_lower[row] != -INF && self.impl_row_dual_lower[row] > self.settings.dual_feasibility_tolerance
    }

    fn is_implied_equation_at_upper(&self, row: usize) -> bool {
        self.mip.model.row_upper[row] != INF && self.impl_row_dual_upper[row] < -self.settings.dual_feasibility_tolerance
    }

    /// HiGHS's `isImpliedIntegral`: whether the model implies an integer
    /// column's integrality, rounding the sides of the rows that show it.
    fn is_implied_integral(&mut self, col: usize) -> StatusResult {
        let mut run_dual_detection = true;
        for (row, val) in self.col_entries(col) {
            if self.rowsize[row] < 2 || self.rowsize_integer[row] < self.rowsize[row] {
                run_dual_detection = false;
                continue;
            }
            let row_lower = if self.is_implied_equation_at_upper(row) {
                self.mip.model.row_upper[row]
            } else {
                self.mip.model.row_lower[row]
            };
            let row_upper = if self.is_implied_equation_at_lower(row) {
                self.mip.model.row_lower[row]
            } else {
                self.mip.model.row_upper[row]
            };
            if row_upper == row_lower {
                run_dual_detection = false;
                let scale = 1.0 / val;
                if !self.row_coefficients_integral(row, scale) {
                    continue;
                }
                if fractionality(row_lower * scale) > self.primal_feastol {
                    return StatusResult::result(Outcome::PrimalInfeasible);
                }
                return StatusResult::flag(true);
            }
        }
        if !run_dual_detection {
            return StatusResult::flag(false);
        }
        for (row, val) in self.col_entries(col) {
            let scale = 1.0 / val;
            if !self.row_coefficients_integral(row, scale) {
                return StatusResult::flag(false);
            }
            if self.mip.model.row_upper[row] != INF {
                let r_upper = val.abs() * (self.mip.model.row_upper[row] * scale.abs() + self.primal_feastol).floor();
                if (self.mip.model.row_upper[row] - r_upper).abs() > self.settings.small_matrix_value {
                    self.mip.model.row_upper[row] = r_upper;
                    self.mark_changed_row(row);
                }
            }
            if self.mip.model.row_lower[row] != -INF {
                let r_lower = val.abs() * (self.mip.model.row_lower[row] * scale.abs() - self.primal_feastol).ceil();
                if (self.mip.model.row_lower[row] - r_lower).abs() > self.settings.small_matrix_value {
                    self.mip.model.row_lower[row] = r_lower;
                    self.mark_changed_row(row);
                }
            }
        }
        StatusResult::flag(true)
    }

    /// HiGHS's `isImpliedInteger`: whether a continuous column only takes
    /// integer values.
    fn is_implied_integer(&mut self, col: usize) -> StatusResult {
        let mut run_dual_detection = true;
        for (row, val) in self.col_entries(col) {
            if self.rowsize[row] < 2 || self.rowsize_integer[row] + self.rowsize_impl_int[row] + 1 < self.rowsize[row] {
                run_dual_detection = false;
                continue;
            }
            let row_lower = if self.is_implied_equation_at_upper(row) {
                self.mip.model.row_upper[row]
            } else {
                self.mip.model.row_lower[row]
            };
            let row_upper = if self.is_implied_equation_at_lower(row) {
                self.mip.model.row_lower[row]
            } else {
                self.mip.model.row_upper[row]
            };
            if row_upper == row_lower {
                run_dual_detection = false;
                let scale = 1.0 / val;
                if fractionality(row_lower * scale) > self.primal_feastol {
                    continue;
                }
                if !self.row_coefficients_integral(row, scale) {
                    continue;
                }
                return StatusResult::flag(true);
            }
        }
        if !run_dual_detection {
            return StatusResult::flag(false);
        }
        let small = self.settings.small_matrix_value;
        let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
        if (lower != -INF && fractionality(lower) > small) || (upper != INF && fractionality(upper) > small) {
            return StatusResult::flag(false);
        }
        for (row, val) in self.col_entries(col) {
            let scale = 1.0 / val;
            if self.mip.model.row_upper[row] != INF && fractionality(self.mip.model.row_upper[row] * scale) > self.primal_feastol {
                return StatusResult::flag(false);
            }
            if self.mip.model.row_lower[row] != -INF && fractionality(self.mip.model.row_lower[row] * scale) > self.primal_feastol {
                return StatusResult::flag(false);
            }
            if !self.row_coefficients_integral(row, scale) {
                return StatusResult::flag(false);
            }
        }
        StatusResult::flag(true)
    }

    /// HiGHS's `convertImpliedInteger`: make a continuous column implied
    /// integer, counting it in `row` only or in all its rows.
    fn convert_implied_integer(&mut self, col: usize, row: usize, skip_input_checks: bool) -> StatusResult {
        if self.col_deleted[col] {
            return StatusResult::flag(false);
        }
        if !skip_input_checks {
            if self.mip.model.integrality[col] != VarType::Continuous {
                return StatusResult::flag(false);
            }
            let implied_integer = self.is_implied_integer(col);
            if !implied_integer.flag {
                return implied_integer;
            }
        }
        self.mip.model.integrality[col] = VarType::ImplicitInteger;
        if row != NONE {
            self.rowsize_impl_int[row] += 1;
        } else {
            let mut pos = self.colhead[col];
            while pos != NONE {
                self.rowsize_impl_int[self.a_row[pos]] += 1;
                pos = self.a_next[pos];
            }
        }
        // Strengthen the bounds from the implied ones first: a fractional
        // implied bound tighter than the rounded one breaks rules that
        // assume integrality (HiGHS #3276).
        let (mut lower, mut upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
        if self.impl_col_lower[col] > lower + self.primal_feastol {
            lower = self.impl_col_lower[col];
        }
        if self.impl_col_upper[col] < upper - self.primal_feastol {
            upper = self.impl_col_upper[col];
        }
        StatusResult::result(self.change_col_bounds(col, lower, upper))
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
        self.rowpositions[..self.rowpositions_len].iter().map(|&pos| (self.a_col[pos], self.a_value[pos])).collect()
    }

    /// HiGHS's `storeRow`: keep the row's positions in column order.
    fn store_row(&mut self, row: usize) {
        let mut sorted = std::mem::take(&mut self.row_buffer);
        matrix::in_order(self.rowroot[row], &self.ar_left, &self.ar_right, &mut sorted);
        let n = sorted.len();
        if self.rowpositions.len() < n {
            self.rowpositions.resize(n, NONE);
        }
        self.rowpositions[..n].copy_from_slice(&sorted);
        self.rowpositions_len = n;
        self.row_buffer = sorted;
    }

    fn load_csc(&mut self, value: &[f64], index: &[usize], start: &[usize]) {
        let num_col = self.mip.model.num_col;
        let num_row = self.mip.model.num_row;
        self.a_value.clear();
        self.a_col.clear();
        self.a_row.clear();
        self.freeslots.clear();
        self.colhead = vec![NONE; num_col];
        self.rowroot = vec![NONE; num_row];
        self.colsize = vec![0; num_col];
        self.rowsize = vec![0; num_row];
        self.rowsize_integer = vec![0; num_row];
        self.rowsize_impl_int = vec![0; num_row];
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
        self.mip.model.a_start = start;
        self.mip.model.a_index = index;
        self.mip.model.a_value = value;
    }

    /// HiGHS's `toCSC` then `okFromCSC`: rebuild the storage compactly.
    fn rebuild_storage(&mut self) {
        self.write_csc();
        let (start, index, value) = (
            std::mem::take(&mut self.mip.model.a_start),
            std::mem::take(&mut self.mip.model.a_index),
            std::mem::take(&mut self.mip.model.a_value),
        );
        self.load_csc(&value, &index, &start);
        self.mip.model.a_start = start;
        self.mip.model.a_index = index;
        self.mip.model.a_value = value;
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
        match self.mip.model.integrality[col] {
            VarType::Integer => self.rowsize_integer[row] += 1,
            VarType::ImplicitInteger => self.rowsize_impl_int[row] += 1,
            _ => {}
        }
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
        match self.mip.model.integrality[col] {
            VarType::Integer => self.rowsize_integer[row] -= 1,
            VarType::ImplicitInteger => self.rowsize_impl_int[row] -= 1,
            _ => {}
        }
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
        self.single_equation_checked[row] = false;
    }

    fn mark_changed_col(&mut self, col: usize) {
        if !self.changed_col_flag[col] {
            self.changed_col_indices.push(col);
            self.changed_col_flag[col] = true;
        }
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

    fn change_col_upper(&mut self, col: usize, mut new_upper: f64) -> Outcome {
        if self.mip.model.integrality[col] != VarType::Continuous {
            new_upper = (new_upper + self.primal_feastol).floor();
            if new_upper == self.mip.model.col_upper[col] {
                return Outcome::Ok;
            }
        }
        let old_upper = self.mip.model.col_upper[col];
        self.mip.model.col_upper[col] = new_upper;
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

    fn change_col_lower(&mut self, col: usize, mut new_lower: f64) -> Outcome {
        if self.mip.model.integrality[col] != VarType::Continuous {
            new_lower = (new_lower - self.primal_feastol).ceil();
            if new_lower == self.mip.model.col_lower[col] {
                return Outcome::Ok;
            }
        }
        let old_lower = self.mip.model.col_lower[col];
        self.mip.model.col_lower[col] = new_lower;
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
        if new_lower > self.mip.model.col_upper[col] {
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
        let bound_diff = self.mip.model.col_upper[col] - self.mip.model.col_lower[col];
        if bound_diff <= self.primal_feastol && (bound_diff <= self.settings.small_matrix_value || self.get_max_abs_col_val(col) * bound_diff <= self.primal_feastol) {
            if bound_diff < -self.primal_feastol {
                return (Outcome::PrimalInfeasible, false);
            }
            if self.mip.model.col_lower[col].abs() == INF {
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
        let upper = self.mip.model.col_upper[col];
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
        let lower = self.mip.model.col_lower[col];
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
        let new_dual_implied = !self.is_dual_implied_free(row) && self.mip.model.row_upper[row] != INF && new_upper <= tol;
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
        let new_dual_implied = !self.is_dual_implied_free(row) && self.mip.model.row_lower[row] != -INF && new_lower >= -tol;
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
            self.mip.model.col_cost[col]
        } else {
            -INF
        };
        let dual_row_upper = if self.is_upper_strictly_implied(col, implied_margin) {
            self.mip.model.col_cost[col]
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
            self.mip.model.row_upper[row]
        } else {
            self.mip.model.row_lower[row]
        };
        let row_upper = if self.is_implied_equation_at_lower(row) {
            self.mip.model.row_lower[row]
        } else {
            self.mip.model.row_upper[row]
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
        let check_implied_bound = |s: &mut Self, row_bnd: f64, residual_act: f64, direction: f64| -> Outcome {
            if direction * residual_act <= -INF {
                return Outcome::Ok;
            }
            let implied_bound = ((CDouble::new(row_bnd) - residual_act) / val).value();
            if implied_bound.abs() * TINY > s.primal_feastol {
                return Outcome::Ok;
            }
            // A row that is not the model's (a cut presolve added) can go,
            // so its bounds are kept as column bounds instead.
            let use_impl_bound = s.ps.orig_row_index(row) < s.orig_num_row;
            let integral = s.mip.model.integrality[col] != VarType::Continuous;
            if direction * val > 0.0 {
                let update_col_bound =
                    (integral && implied_bound < s.mip.model.col_upper[col] - s.primal_feastol) || (!use_impl_bound && implied_bound < s.mip.model.col_upper[col] - threshold);
                if use_impl_bound && implied_bound < s.impl_col_upper[col] - threshold {
                    s.change_impl_col_upper(col, implied_bound, row);
                }
                if update_col_bound {
                    check!(s.change_col_upper(col, implied_bound));
                }
            } else {
                let update_col_bound =
                    (integral && implied_bound > s.mip.model.col_lower[col] + s.primal_feastol) || (!use_impl_bound && implied_bound > s.mip.model.col_lower[col] + threshold);
                if use_impl_bound && implied_bound > s.impl_col_lower[col] + threshold {
                    s.change_impl_col_lower(col, implied_bound, row);
                }
                if update_col_bound {
                    check!(s.change_col_lower(col, implied_bound));
                }
            }
            Outcome::Ok
        };
        if row_upper != INF {
            let residual = self.implied_row_bounds.residual_sum_lower_orig(col_bounds!(self), row, col, val, NONE, -INF, -INF);
            check!(check_implied_bound(self, row_upper, residual, 1.0));
        }
        if row_lower != -INF {
            let residual = self.implied_row_bounds.residual_sum_upper_orig(col_bounds!(self), row, col, val, NONE, -INF, -INF);
            check!(check_implied_bound(self, row_lower, residual, -1.0));
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

    fn shrink_problem(&mut self) {
        let old_num_col = self.mip.model.num_col;
        self.mip.model.num_col = 0;
        let mut new_col_index = vec![NONE; old_num_col];
        for i in 0..old_num_col {
            if self.col_deleted[i] {
                continue;
            }
            let to = self.mip.model.num_col;
            self.mip.model.num_col += 1;
            new_col_index[i] = to;
            if to < i {
                self.mip.model.col_cost[to] = self.mip.model.col_cost[i];
                self.mip.model.col_lower[to] = self.mip.model.col_lower[i];
                self.mip.model.col_upper[to] = self.mip.model.col_upper[i];
                self.mip.model.integrality[to] = self.mip.model.integrality[i];
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
        let num_col = self.mip.model.num_col;
        self.col_deleted = vec![false; num_col];
        self.mip.model.col_cost.truncate(num_col);
        self.mip.model.col_lower.truncate(num_col);
        self.mip.model.col_upper.truncate(num_col);
        self.mip.model.integrality.truncate(num_col);
        self.impl_col_lower.truncate(num_col);
        self.impl_col_upper.truncate(num_col);
        self.col_lower_source.truncate(num_col);
        self.col_upper_source.truncate(num_col);
        self.impl_row_dual_source_by_col.truncate(num_col);
        self.colhead.truncate(num_col);
        self.colsize.truncate(num_col);
        self.changed_col_flag.truncate(num_col);
        self.num_deleted_cols = 0;

        let old_num_row = self.mip.model.num_row;
        self.mip.model.num_row = 0;
        let mut new_row_index = vec![NONE; old_num_row];
        for i in 0..old_num_row {
            if self.row_deleted[i] {
                continue;
            }
            let to = self.mip.model.num_row;
            self.mip.model.num_row += 1;
            new_row_index[i] = to;
            if to < i {
                self.mip.model.row_lower[to] = self.mip.model.row_lower[i];
                self.mip.model.row_upper[to] = self.mip.model.row_upper[i];
                self.row_dual_lower[to] = self.row_dual_lower[i];
                self.row_dual_upper[to] = self.row_dual_upper[i];
                self.impl_row_dual_lower[to] = self.impl_row_dual_lower[i];
                self.impl_row_dual_upper[to] = self.impl_row_dual_upper[i];
                self.row_dual_lower_source[to] = self.row_dual_lower_source[i];
                self.row_dual_upper_source[to] = self.row_dual_upper_source[i];
                self.col_impl_source_by_row[to] = self.col_impl_source_by_row[i].clone();
                self.rowroot[to] = self.rowroot[i];
                self.rowsize[to] = self.rowsize[i];
                self.rowsize_integer[to] = self.rowsize_integer[i];
                self.rowsize_impl_int[to] = self.rowsize_impl_int[i];
                self.changed_row_flag[to] = self.changed_row_flag[i];
                self.single_equation_checked[to] = self.single_equation_checked[i];
            }
        }
        let num_row = self.mip.model.num_row;
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
        self.mip.model.row_lower.truncate(num_row);
        self.mip.model.row_upper.truncate(num_row);
        self.row_dual_lower.truncate(num_row);
        self.row_dual_upper.truncate(num_row);
        self.impl_row_dual_lower.truncate(num_row);
        self.impl_row_dual_upper.truncate(num_row);
        self.row_dual_lower_source.truncate(num_row);
        self.row_dual_upper_source.truncate(num_row);
        self.col_impl_source_by_row.truncate(num_row);
        self.rowroot.truncate(num_row);
        self.rowsize.truncate(num_row);
        self.rowsize_integer.truncate(num_row);
        self.rowsize_impl_int.truncate(num_row);
        self.changed_row_flag.truncate(num_row);
        self.single_equation_checked.truncate(num_row);
        self.num_deleted_rows = 0;

        let new_row_map: Vec<Option<usize>> = new_row_index.iter().map(|&row| (row != NONE).then_some(row)).collect();
        let new_col_map: Vec<Option<usize>> = new_col_index.iter().map(|&col| (col != NONE).then_some(col)).collect();
        self.ps.compress_index_maps(&new_row_map, &new_col_map);
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

        // The solver's data over the shrunk model.
        let mip = &mut *self.mip;
        mip.row_matrix_set = false;
        mip.objective_function = ObjectiveFunction::new(&mip.model);
        mip.domain = Domain::new(&mip.model);
        mip.domain.set_global(true);
        let ps = &self.ps;
        mip.cliquetable
            .rebuild(num_col, &new_col_map, &|col| ps.is_col_linearly_transformable(col), &mip.domain, &mip.model);
        mip.implications
            .rebuild(num_col, &new_col_map, &|col| ps.is_col_linearly_transformable(col), &mip.domain, &mip.model, mip.feastol);
        mip.cutpool = CutPool::new(num_col, self.settings.mip_pool_age_limit, self.settings.mip_pool_soft_limit);
        mip.conflictpool = ConflictPool::new(5 * self.settings.mip_pool_age_limit, self.settings.mip_pool_soft_limit);
        for i in 0..old_num_col {
            if new_col_index[i] != NONE {
                self.num_probes[new_col_index[i]] = self.num_probes[i];
            }
        }
        self.num_probes.truncate(num_col);
    }

    fn remove_row(&mut self, row: usize) {
        self.mark_row_deleted(row);
        self.store_row(row);
        for k in 0..self.rowpositions_len {
            let pos = self.rowpositions[k];
            self.unlink(pos);
        }
    }

    fn remove_fixed_col(&mut self, col: usize) {
        let fixval = self.mip.model.col_lower[col];
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
            if self.mip.model.row_lower[colrow] != -INF {
                self.mip.model.row_lower[colrow] -= colval * fixval;
            }
            if self.mip.model.row_upper[colrow] != INF {
                self.mip.model.row_upper[colrow] -= colval * fixval;
            }
            self.unlink(colpos);
            self.reinsert_equation(colrow);
        }
        self.mip.model.offset += self.mip.model.col_cost[col] * fixval;
        self.mip.model.col_cost[col] = 0.0;
    }

    fn fix_col_to_lower(&mut self, col: usize) -> Outcome {
        let fixval = self.mip.model.col_lower[col];
        if fixval == -INF {
            return Outcome::DualInfeasible;
        }
        self.ps.fixed_col(col, fixval);
        self.remove_fixed_col_at(col, fixval);
        Outcome::Ok
    }

    fn fix_col_to_upper(&mut self, col: usize) -> Outcome {
        let fixval = self.mip.model.col_upper[col];
        if fixval == INF {
            return Outcome::DualInfeasible;
        }
        self.ps.fixed_col(col, fixval);
        self.remove_fixed_col_at(col, fixval);
        Outcome::Ok
    }

    fn fix_col_to_zero(&mut self, col: usize) {
        self.ps.fixed_col(col, 0.0);
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
            if self.mip.model.row_lower[colrow] != -INF {
                self.mip.model.row_lower[colrow] += scale * rhs;
            }
            if self.mip.model.row_upper[colrow] != INF {
                self.mip.model.row_upper[colrow] += scale * rhs;
            }
            for k in 0..self.rowpositions_len {
                let rowiter = self.rowpositions[k];
                if self.a_col[rowiter] != col {
                    self.add_to_matrix(colrow, self.a_col[rowiter], scale * self.a_value[rowiter]);
                }
            }
            self.reinsert_equation(colrow);
        }
        if self.mip.model.col_cost[col] != 0.0 {
            let objscale = CDouble::new(self.mip.model.col_cost[col]) * substrowscale;
            self.mip.model.offset = (-(objscale * rhs) + self.mip.model.offset).value();
            for k in 0..self.rowpositions_len {
                let rowiter = self.rowpositions[k];
                let c = self.a_col[rowiter];
                self.mip.model.col_cost[c] = (objscale * self.a_value[rowiter] + self.mip.model.col_cost[c]).value();
                if self.mip.model.col_cost[c].abs() <= self.settings.small_matrix_value {
                    self.mip.model.col_cost[c] = 0.0;
                }
            }
            self.mip.model.col_cost[col] = 0.0;
        }
        for k in 0..self.rowpositions_len {
            let pos = self.rowpositions[k];
            self.unlink(pos);
        }
    }

    /// Substitute `substcol = offset + scale * staycol` everywhere.
    fn substitute_cols(&mut self, substcol: usize, staycol: usize, offset: f64, scale: f64) {
        // An implied integer standing in for an integer column becomes one,
        // so integrality is not lost (HiGHS #3268).
        if self.mip.model.integrality[substcol] == VarType::Integer && self.mip.model.integrality[staycol] == VarType::ImplicitInteger {
            self.mip.model.integrality[staycol] = VarType::Integer;
            let mut pos = self.colhead[staycol];
            while pos != NONE {
                let row = self.a_row[pos];
                self.rowsize_integer[row] += 1;
                self.rowsize_impl_int[row] -= 1;
                pos = self.a_next[pos];
            }
        }
        let mut coliter = self.colhead[substcol];
        while coliter != NONE {
            let colrow = self.a_row[coliter];
            let colval = self.a_value[coliter];
            let colpos = coliter;
            coliter = self.a_next[coliter];
            self.unlink(colpos);
            if self.mip.model.row_lower[colrow] != -INF {
                self.mip.model.row_lower[colrow] -= colval * offset;
            }
            if self.mip.model.row_upper[colrow] != INF {
                self.mip.model.row_upper[colrow] -= colval * offset;
            }
            self.add_to_matrix(colrow, staycol, scale * colval);
            self.reinsert_equation(colrow);
        }
        if self.mip.model.col_cost[substcol] != 0.0 {
            self.mip.model.offset += self.mip.model.col_cost[substcol] * offset;
            self.mip.model.col_cost[staycol] += scale * self.mip.model.col_cost[substcol];
            if self.mip.model.col_cost[staycol].abs() <= self.settings.small_matrix_value {
                self.mip.model.col_cost[staycol] = 0.0;
            }
            self.mip.model.col_cost[substcol] = 0.0;
        }
    }

    fn substitute_free_col(&mut self, row: usize, col: usize, relax_row_dual_bounds: bool) {
        let (rhs, _row_type) = self.dual_implied_free_get_rhs_and_row_type(row, relax_row_dual_bounds);
        let row_vec = self.stored_row_entries();
        self.ps.free_col_substitution(row, col, rhs, row_vec);
        self.substitute_row_col(row, col, rhs);
    }

    // ---- Limits ----------------------------------------------------------

    fn check_time_limit(&self) -> Outcome {
        if self.settings.time_limit < INF && self.mip.elapsed() >= self.settings.time_limit {
            return Outcome::Stopped;
        }
        Outcome::Ok
    }

    fn check_limits(&self) -> Outcome {
        let num_reductions = self.ps.num_reductions();
        if (num_reductions & 1023) == 0 {
            check!(self.check_time_limit());
        }
        if num_reductions >= self.reduction_limit { Outcome::Stopped } else { Outcome::Ok }
    }

    fn store_current_problem_size(&mut self) {
        self.old_num_col = self.mip.model.num_col - self.num_deleted_cols;
        self.old_num_row = self.mip.model.num_row - self.num_deleted_rows;
    }

    fn problem_size_reduction(&self) -> f64 {
        let col_reduction = 100.0 * (self.old_num_col as f64 - (self.mip.model.num_col - self.num_deleted_cols) as f64) / self.old_num_col as f64;
        let row_reduction = 100.0 * (self.old_num_row as f64 - (self.mip.model.num_row - self.num_deleted_rows) as f64) / self.old_num_row as f64;
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

impl MipPresolve<'_> {
    fn doubleton_eq(&mut self, row: usize, row_type: RowType) -> Outcome {
        let nz_pos1 = self.rowroot[row];
        let nz_pos2 = if self.ar_right[nz_pos1] != NONE {
            self.ar_right[nz_pos1]
        } else {
            self.ar_left[nz_pos1]
        };
        let small = self.settings.small_matrix_value;
        let col_at_pos1_better = {
            let (col1, col2) = (self.a_col[nz_pos1], self.a_col[nz_pos2]);
            let int1 = self.mip.model.integrality[col1] == VarType::Integer;
            let int2 = self.mip.model.integrality[col2] == VarType::Integer;
            if int1 {
                if int2 {
                    // Both integer: the smaller coefficient, or the sparser
                    // column if they are equal.
                    if self.a_value[nz_pos1].abs() < self.a_value[nz_pos2].abs() - small {
                        true
                    } else if self.a_value[nz_pos2].abs() < self.a_value[nz_pos1].abs() - small {
                        false
                    } else {
                        self.colsize[col1] < self.colsize[col2]
                    }
                } else {
                    false
                }
            } else if int2 {
                true
            } else {
                let col1_size = self.colsize[col1];
                if col1_size == 1 {
                    true
                } else {
                    let col2_size = self.colsize[col2];
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
            }
        };
        let (substcol, staycol, substcoef, mut staycoef) = if col_at_pos1_better {
            (self.a_col[nz_pos1], self.a_col[nz_pos2], self.a_value[nz_pos1], self.a_value[nz_pos2])
        } else {
            (self.a_col[nz_pos2], self.a_col[nz_pos1], self.a_value[nz_pos2], self.a_value[nz_pos1])
        };
        let mut rhs = self.mip.model.row_upper[row];
        if self.mip.model.integrality[substcol] == VarType::Integer && self.mip.model.integrality[staycol] == VarType::Integer {
            let round_coef = (staycoef / substcoef).round() * substcoef;
            if (round_coef - staycoef).abs() > small {
                return Outcome::Ok;
            }
            staycoef = round_coef;
            let round_rhs = (rhs / substcoef).round() * substcoef;
            if (rhs - round_rhs).abs() > self.primal_feastol {
                return Outcome::PrimalInfeasible;
            }
            rhs = round_rhs;
        }
        let old_stay_lower = self.mip.model.col_lower[staycol];
        let old_stay_upper = self.mip.model.col_upper[staycol];
        let subst_lower = self.mip.model.col_lower[substcol];
        let subst_upper = self.mip.model.col_upper[substcol];
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
        let _ = (row_type, lower_tightened, upper_tightened);
        self.ps.doubleton_equation(substcol, staycol, substcoef, staycoef, rhs);
        self.mark_col_deleted(substcol);
        self.remove_row(row);
        self.substitute_cols(substcol, staycol, rhs / substcoef, -staycoef / substcoef);
        check!(self.remove_row_singletons());
        self.check_limits()
    }

    fn singleton_row(&mut self, row: usize) -> Outcome {
        let nz_pos = self.rowroot[row];
        let col = self.a_col[nz_pos];
        let val = self.a_value[nz_pos];
        self.mark_row_deleted(row);
        self.unlink(nz_pos);
        let feastol = self.primal_feastol;
        let (col_lower, col_upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
        let (row_lower, row_upper) = (self.mip.model.row_lower[row], self.mip.model.row_upper[row]);
        let redundant = if val > 0.0 {
            col_upper * val <= row_upper + feastol && col_lower * val >= row_lower - feastol
        } else {
            col_lower * val <= row_upper + feastol && col_upper * val >= row_lower - feastol
        };
        if redundant {
            self.ps.redundant_row();
            return self.check_limits();
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
        let is_integral = self.mip.model.integrality[col] != VarType::Continuous;
        let mut lb = if lower_tightened {
            if is_integral {
                new_col_lower = (new_col_lower - bound_tol).ceil();
            }
            new_col_lower
        } else {
            col_lower
        };
        let mut ub = if upper_tightened {
            if is_integral {
                new_col_upper = (new_col_upper + bound_tol).floor();
            }
            new_col_upper
        } else {
            col_upper
        };
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
        self.ps.singleton_row();
        if lower_tightened {
            check!(self.change_col_lower(col, lb));
        }
        if ub == lb {
            self.ps.fixed_col(col, lb);
            self.remove_fixed_col(col);
        } else if upper_tightened {
            check!(self.change_col_upper(col, ub));
        }
        if !self.col_deleted[col] && self.colsize[col] == 0 {
            return self.empty_col(col);
        }
        self.check_limits()
    }

    fn singleton_col(&mut self, col: usize) -> Outcome {
        let nz_pos = self.colhead[col];
        let row = self.a_row[nz_pos];
        let col_coef = self.a_value[nz_pos];
        if self.rowsize[row] == 1 {
            check!(self.singleton_row(row));
            if !self.col_deleted[col] {
                return self.empty_col(col);
            }
            return Outcome::Ok;
        }
        check!(self.detect_dominated_col(col, false));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.convert_implied_integer(col, row, false).result);
        check!(self.dual_fixing(col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.singleton_col_stuffing(col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.update_col_implied_bounds(row, col, col_coef));
        if self.mip.model.integrality[col] != VarType::Integer {
            self.update_row_dual_implied_bounds(row, col, col_coef);
        }
        if self.is_dual_implied_free(row) && self.is_implied_free(col) {
            if self.mip.model.integrality[col] == VarType::Integer {
                let implied_integral = self.is_implied_integral(col);
                check!(implied_integral.result);
                if !implied_integral.flag {
                    return Outcome::Ok;
                }
            }
            self.store_row(row);
            self.substitute_free_col(row, col, false);
            return self.check_limits();
        }
        Outcome::Ok
    }

    fn check_row_redundant(&mut self, row: usize) -> Outcome {
        if self.is_redundant(row) {
            self.ps.redundant_row();
            self.remove_row(row);
            return self.check_limits();
        }
        Outcome::Ok
    }

    fn row_presolve(&mut self, row: usize) -> Outcome {
        let feastol = self.primal_feastol;
        if self.implied_row_bounds.sum_lower(row, CDouble::default()) > self.mip.model.row_upper[row] + feastol
            || self.implied_row_bounds.sum_upper(row, CDouble::default()) < self.mip.model.row_lower[row] - feastol
        {
            return Outcome::PrimalInfeasible;
        }
        // A row dual bounded away from zero on a side the row lacks
        // (HiGHS #3392).
        let dual_tol = self.settings.dual_feasibility_tolerance;
        if (self.impl_row_dual_lower[row] > dual_tol && self.mip.model.row_lower[row] == -INF)
            || (self.impl_row_dual_upper[row] < -dual_tol && self.mip.model.row_upper[row] == INF)
        {
            return Outcome::DualInfeasible;
        }
        if self.rowsize[row] == 1 {
            return self.singleton_row(row);
        }
        check!(self.check_row_redundant(row));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }

        if self.rowsize_integer[row] != 0 || self.rowsize_impl_int[row] != 0 {
            // Setting a binary to its other bound than the one the activity
            // bound uses must keep the row feasible (Suhl and Szymanski).
            for pos in self.row_vector(row) {
                let col = self.a_col[pos];
                let val = self.a_value[pos];
                if self.mip.model.integrality[col] == VarType::Continuous || self.mip.model.col_upper[col] != self.mip.model.col_lower[col] + 1.0 {
                    continue;
                }
                let compute_offset = |s: &Self| val.abs() * (CDouble::new(s.mip.model.col_upper[col]) - CDouble::new(s.mip.model.col_lower[col]));
                let degree1_tests = |s: &mut Self, direction: f64, row_activity_bound: f64, row_bound: f64| -> Outcome {
                    if direction * row_activity_bound >= direction * row_bound - s.primal_feastol {
                        return Outcome::Ok;
                    }
                    if direction * val > 0.0 {
                        let lower = s.mip.model.col_lower[col] + 1.0;
                        check!(s.change_col_lower(col, lower));
                    } else {
                        let upper = s.mip.model.col_upper[col] - 1.0;
                        check!(s.change_col_upper(col, upper));
                    }
                    Outcome::Ok
                };
                let bound = self.implied_row_bounds.sum_upper_orig(row, -compute_offset(self));
                let row_lower = self.mip.model.row_lower[row];
                check!(degree1_tests(self, 1.0, bound, row_lower));
                let bound = self.implied_row_bounds.sum_lower_orig(row, compute_offset(self));
                let row_upper = self.mip.model.row_upper[row];
                check!(degree1_tests(self, -1.0, bound, row_upper));
            }
        }

        check!(self.check_row_redundant(row));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }

        let orig_row_upper = self.mip.model.row_upper[row];
        let orig_row_lower = self.mip.model.row_lower[row];
        if !self.is_equation(row) {
            // HiGHS checks the singleton columns' redundant bounds here for
            // an LP only.
            if self.is_implied_equation_at_lower(row) {
                self.mip.model.row_upper[row] = self.mip.model.row_lower[row];
                self.change_row_dual_lower(row, -INF);
            } else if self.is_implied_equation_at_upper(row) {
                self.mip.model.row_lower[row] = self.mip.model.row_upper[row];
                self.change_row_dual_upper(row, INF);
            }
        }
        let row_upper = self.mip.model.row_upper[row];
        let row_lower = self.mip.model.row_lower[row];
        if self.rowsize[row] == 2 && row_lower == row_upper {
            let row_type = if orig_row_lower == orig_row_upper {
                RowType::Eq
            } else if orig_row_upper != INF {
                RowType::Leq
            } else {
                RowType::Geq
            };
            return self.doubleton_eq(row, row_type);
        }

        if self.rowsize_integer[row] != 0 || self.rowsize_impl_int[row] != 0 {
            if row_lower == row_upper {
                check!(self.integer_equation_presolve(row, row_upper));
                if self.row_deleted[row] {
                    return Outcome::Ok;
                }
            } else {
                check!(self.integer_inequality_presolve(row));
                if self.row_deleted[row] {
                    return Outcome::Ok;
                }
            }
        }

        // Forcing rows.
        self.store_row(row);
        let dynamism = Self::compute_dynamism(self.rowpositions[..self.rowpositions_len].iter().map(|&pos| self.a_value[pos]));
        let side = self.mip.model.row_lower[row];
        let bound = self.implied_row_bounds.sum_upper_orig(row, CDouble::default());
        check!(self.check_forcing_row(row, 1.0, side, bound, dynamism, RowType::Geq));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }
        let side = self.mip.model.row_upper[row];
        let bound = self.implied_row_bounds.sum_lower_orig(row, CDouble::default());
        check!(self.check_forcing_row(row, -1.0, side, bound, dynamism, RowType::Leq));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }
        check!(self.update_col_implied_bounds_of_row(row));
        self.extract_var_bounds(row);
        self.check_limits()
    }

    /// `rowPresolve` on an equation with integer columns: simple probing
    /// (section 3.6 of Achterberg et al.), then scaling a continuous column
    /// to an implied integer, or substituting a column to make the gcd of
    /// the coefficients one. A deleted row has been dealt with; HiGHS
    /// returns from `rowPresolve` there, as the caller does.
    fn integer_equation_presolve(&mut self, row: usize, row_upper: f64) -> Outcome {
        let small = self.settings.small_matrix_value;
        let implied_row_lower = self.implied_row_bounds.sum_lower(row, CDouble::default());
        let implied_row_upper = self.implied_row_bounds.sum_upper(row, CDouble::default());
        if implied_row_lower != -INF && implied_row_upper != INF && (implied_row_lower + implied_row_upper - 2.0 * row_upper).abs() <= small {
            let mut bin_col = NONE;
            let mut bin_coef = (implied_row_upper - row_upper).abs();
            self.store_row(row);
            for k in 0..self.rowpositions_len {
                let pos = self.rowpositions[k];
                let (col, value) = (self.a_col[pos], self.a_value[pos]);
                if (value.abs() - bin_coef).abs() <= small
                    && self.mip.model.integrality[col] == VarType::Integer
                    && (self.mip.model.col_upper[col] - self.mip.model.col_lower[col] - 1.0).abs() <= self.primal_feastol
                {
                    bin_col = col;
                    bin_coef = value;
                    break;
                }
            }
            if bin_col != NONE {
                // The bounds as they stand now: removing or substituting a
                // column changes them, which would make later substitutions
                // invalid (HiGHS #3260).
                let snapshot: Vec<(f64, f64)> = (0..self.rowpositions_len)
                    .map(|k| {
                        let col = self.a_col[self.rowpositions[k]];
                        (impl_var_lower(col_bounds!(self), row, col), impl_var_upper(col_bounds!(self), row, col))
                    })
                    .collect();
                for k in 0..self.rowpositions_len {
                    let rowiter = self.rowpositions[k];
                    let col = self.a_col[rowiter];
                    if col == bin_col {
                        continue;
                    }
                    let (col_lower, col_upper) = snapshot[k];
                    if self.mip.model.col_lower[col] == self.mip.model.col_upper[col] {
                        self.ps.fixed_col(col, self.mip.model.col_lower[col]);
                        self.remove_fixed_col(col);
                        continue;
                    }
                    let direction = if bin_coef.is_sign_negative() == self.a_value[rowiter].is_sign_negative() {
                        1.0
                    } else {
                        -1.0
                    };
                    let bound = if direction >= 0.0 { col_upper } else { col_lower };
                    let scale = direction * (col_lower - col_upper);
                    let offset = bound - self.mip.model.col_lower[bin_col] * scale;
                    self.ps.doubleton_equation(col, bin_col, 1.0, -scale, offset);
                    self.substitute_cols(col, bin_col, offset, scale);
                }
                self.remove_row(row);
                check!(self.check_limits());
                return self.remove_row_singletons();
            }
        }

        if self.rowsize_integer[row] + self.rowsize_impl_int[row] + 1 >= self.rowsize[row] {
            let mut continuous_col = NONE;
            let mut continuous_coef = 0.0;
            let mut row_coefs_int = Vec::with_capacity(self.rowsize[row]);
            self.store_row(row);
            for k in 0..self.rowpositions_len {
                let pos = self.rowpositions[k];
                let (col, value) = (self.a_col[pos], self.a_value[pos]);
                if self.mip.model.integrality[col] == VarType::Continuous {
                    continuous_coef = value;
                    continuous_col = col;
                    continue;
                }
                row_coefs_int.push(value);
            }
            if continuous_coef != 0.0 {
                row_coefs_int.push(row_upper);
                let int_scale = integral_scale(&row_coefs_int, small, small);
                if int_scale != 0.0 && int_scale <= 1e3 {
                    let scale = 1.0 / (continuous_coef * int_scale).abs();
                    if scale != 1.0 {
                        check!(self.transform_column(continuous_col, scale, 0.0));
                        check!(self.convert_implied_integer(continuous_col, NONE, true).result);
                        if int_scale != 1.0 {
                            self.scale_stored_row(row, int_scale, true);
                        }
                    }
                }
            } else {
                let mut int_scale = integral_scale(&row_coefs_int, small, small);
                if int_scale != 0.0 && int_scale <= 1e3 {
                    let mut rhs = row_upper * int_scale;
                    if fractionality(rhs) > self.primal_feastol {
                        return Outcome::PrimalInfeasible;
                    }
                    rhs = rhs.round();
                    let mut x1_cand = NONE;
                    let mut d: i64 = 0;
                    for i in 0..self.rowpositions_len {
                        let coef = (int_scale * self.a_value[self.rowpositions[i]]).round().abs();
                        let newgcd = if d == 0 { coef as i64 } else { gcd(coef as i64, d) };
                        if newgcd == 1 {
                            // This column would make the gcd one, so it is
                            // the candidate to substitute; a second one
                            // leaves only the scaling.
                            if x1_cand != NONE {
                                x1_cand = NONE;
                                break;
                            }
                            x1_cand = i;
                        } else {
                            d = newgcd;
                        }
                    }
                    if x1_cand != NONE {
                        let x1_pos = self.rowpositions[x1_cand];
                        let x1 = self.a_col[x1_pos];
                        let rhs2 = rhs / d as f64;
                        if fractionality(rhs2) <= self.mip.epsilon {
                            // x1 = d * z.
                            check!(self.transform_column(x1, d as f64, 0.0));
                        } else {
                            // x1 = d * z + b, with b = a1^-1 rhs (mod d).
                            let mut a1 = (int_scale * self.a_value[x1_pos]).round() as i64;
                            a1 = mod_i64(a1, d);
                            let a1_inverse = modular_inverse(a1, d);
                            let b = mod_f64(a1_inverse as f64 * rhs, d as f64);
                            let z_lower = ((self.mip.model.col_lower[x1] - b) / d as f64 - self.primal_feastol).ceil();
                            let z_upper = ((self.mip.model.col_upper[x1] - b) / d as f64 + self.primal_feastol).floor();
                            if z_lower == z_upper {
                                let fix_val = z_lower * d as f64 + b;
                                check!(self.change_col_bounds(x1, fix_val, fix_val));
                                self.ps.fixed_col(x1, fix_val);
                                self.remove_fixed_col(x1);
                                // `rowpositions.erase`: the tail moves down
                                // and the last slot keeps its value.
                                let len = self.rowpositions_len;
                                self.rowpositions.copy_within(x1_cand + 1..len, x1_cand);
                                self.rowpositions_len -= 1;
                            } else {
                                check!(self.transform_column(x1, d as f64, b));
                            }
                        }
                        int_scale /= d as f64;
                    }
                    if int_scale != 1.0 {
                        self.scale_stored_row(row, int_scale, true);
                    }
                }
            }
        }
        Outcome::Ok
    }

    /// `rowPresolve` on an inequality or ranged row with integer columns:
    /// scale an all-integer row to integral coefficients, or round it by
    /// Chvátal-Gomory strengthening (section 3.4 of Achterberg et al.),
    /// then strengthen the integer coefficients.
    fn integer_inequality_presolve(&mut self, row: usize) -> Outcome {
        let small = self.settings.small_matrix_value;
        let feastol = self.primal_feastol;
        self.store_row(row);
        if self.rowsize[row] == self.rowsize_integer[row] + self.rowsize_impl_int[row] {
            let mut row_coefs = Vec::with_capacity(self.rowsize[row]);
            let mut row_index = Vec::with_capacity(self.rowsize[row]);
            for k in 0..self.rowpositions_len {
                let pos = self.rowpositions[k];
                row_coefs.push(self.a_value[pos]);
                row_index.push(self.a_col[pos]);
            }
            let int_scale = integral_scale(
                &row_coefs,
                if self.mip.model.row_lower[row] == -INF { feastol } else { small },
                if self.mip.model.row_upper[row] == INF { feastol } else { small },
            );

            // Round a side: (rounded, fraction, tightened).
            let round_rhs = |rhs: CDouble, min_rhs_tightening: f64, direction: f64| -> (CDouble, CDouble, bool) {
                let rounded_rhs = direction * (direction * rhs + feastol).floor();
                let fraction_rhs = direction * (rhs - rounded_rhs);
                let rhs_tightened = fraction_rhs >= min_rhs_tightening - small;
                (rounded_rhs, fraction_rhs, rhs_tightened)
            };

            // Scale the row back to `rounded / scalar`.
            let scale_row = |s: &mut Self, row_coefs: &[f64], rounded_lhs: CDouble, rounded_rhs: CDouble, scalar: f64, check_delta: bool| {
                if rounded_lhs != -INF {
                    s.mip.model.row_lower[row] = (rounded_lhs / scalar).value();
                }
                if rounded_rhs != INF {
                    s.mip.model.row_upper[row] = (rounded_rhs / scalar).value();
                }
                for i in 0..row_coefs.len() {
                    let delta = (CDouble::new(row_coefs[i]) / scalar - s.a_value[s.rowpositions[i]]).value();
                    if !check_delta || delta.abs() > small {
                        s.add_to_matrix(row, row_index[i], delta);
                    }
                }
            };

            if int_scale != 0.0 {
                // HiGHS's `checkScaleRow`.
                let mut lhs = CDouble::new(self.mip.model.row_lower[row]);
                let mut rhs = CDouble::new(self.mip.model.row_upper[row]);
                let lhs_finite = lhs != -INF;
                let rhs_finite = rhs != INF;
                if lhs_finite {
                    lhs *= int_scale;
                }
                if rhs_finite {
                    rhs *= int_scale;
                }
                let mut rounded_lhs = CDouble::new(-INF);
                let mut rounded_rhs = CDouble::new(INF);
                let mut fraction_lhs = CDouble::new(0.0);
                let mut fraction_rhs = CDouble::new(0.0);
                let mut min_rhs_tightening = 0.0;
                let mut min_lhs_tightening = 0.0;
                let mut max_val = 0.0;
                let mut lhs_tightened = false;
                let mut rhs_tightened = false;
                let mut scalable = true;
                for i in 0..row_coefs.len() {
                    let scale_coef = CDouble::new(row_coefs[i]) * int_scale;
                    let int_coef = (scale_coef + 0.5).floor();
                    let coef_delta = int_coef - scale_coef;
                    row_coefs[i] = int_coef.value();
                    max_val = max(row_coefs[i].abs(), max_val);
                    let ub = self.mip.model.col_upper[row_index[i]];
                    if coef_delta < -small {
                        if lhs_finite {
                            if ub == INF {
                                scalable = false;
                                break;
                            }
                            lhs += ub * coef_delta;
                        }
                        min_rhs_tightening = max(-coef_delta.value(), min_rhs_tightening);
                    } else if coef_delta > small {
                        if rhs_finite {
                            if ub == INF {
                                scalable = false;
                                break;
                            }
                            rhs += ub * coef_delta;
                        }
                        min_lhs_tightening = max(coef_delta.value(), min_lhs_tightening);
                    }
                }
                if scalable {
                    if lhs_finite {
                        (rounded_lhs, fraction_lhs, lhs_tightened) = round_rhs(lhs, min_lhs_tightening, -1.0);
                    }
                    if rhs_finite {
                        (rounded_rhs, fraction_rhs, rhs_tightened) = round_rhs(rhs, min_rhs_tightening, 1.0);
                    }
                    let is_infeasible = lhs_finite && rhs_finite && rounded_rhs < rounded_lhs - 0.5;
                    if is_infeasible {
                        return Outcome::PrimalInfeasible;
                    }
                    let ranged_or_equation_row = lhs_tightened && rhs_tightened;
                    if ranged_or_equation_row || (lhs_tightened && self.mip.model.row_upper[row] == INF) || (rhs_tightened && self.mip.model.row_lower[row] == -INF) {
                        // HiGHS's `scaleRowIntVals`.
                        if !(max_val > 1000.0 && int_scale > 100.0) {
                            scale_row(self, &row_coefs, rounded_lhs, rounded_rhs, 1.0, false);
                        } else if ranged_or_equation_row {
                            rounded_lhs /= int_scale;
                            rounded_rhs /= int_scale;
                            if rounded_rhs < self.mip.model.row_upper[row] - feastol {
                                self.mip.model.row_upper[row] = rounded_rhs.value();
                            }
                            if rounded_lhs > self.mip.model.row_lower[row] + feastol {
                                self.mip.model.row_lower[row] = rounded_lhs.value();
                            }
                        } else if (rhs_tightened && fraction_rhs < min_rhs_tightening - feastol) || (lhs_tightened && fraction_lhs < min_lhs_tightening - feastol) {
                            scale_row(self, &row_coefs, rounded_lhs, rounded_rhs, int_scale, true);
                        }
                    }
                }
            } else if !self.is_ranged(row) {
                check!(self.chvatal_gomory_strengthening(row, &row_coefs, &row_index));
            }
        }

        check!(self.check_row_redundant(row));
        if self.row_deleted[row] {
            return Outcome::Ok;
        }

        if self.mip.model.row_lower[row] == -INF {
            let implied_row_upper = self.implied_row_bounds.sum_upper(row, CDouble::default());
            if implied_row_upper != INF {
                let mut rhs = CDouble::new(self.mip.model.row_upper[row]);
                let max_abs_coef_value = CDouble::new(implied_row_upper) - self.mip.model.row_upper[row];
                self.strengthen_coefs(row, &mut rhs, 1.0, max_abs_coef_value);
                self.mip.model.row_upper[row] = rhs.value();
            }
        }
        if self.mip.model.row_upper[row] == INF {
            let implied_row_lower = self.implied_row_bounds.sum_lower(row, CDouble::default());
            if implied_row_lower != -INF {
                let mut rhs = CDouble::new(self.mip.model.row_lower[row]);
                let max_abs_coef_value = self.mip.model.row_lower[row] - CDouble::new(implied_row_lower);
                self.strengthen_coefs(row, &mut rhs, -1.0, max_abs_coef_value);
                self.mip.model.row_lower[row] = rhs.value();
            }
        }
        Outcome::Ok
    }

    /// `rowPresolve`'s `strengthenCoefs`: cap the integer coefficients at
    /// what the row's activity bound leaves.
    fn strengthen_coefs(&mut self, row: usize, rhs: &mut CDouble, direction: f64, max_abs_coef_value: CDouble) {
        for k in 0..self.rowpositions_len {
            let rowiter = self.rowpositions[k];
            let col = self.a_col[rowiter];
            let val = direction * self.a_value[rowiter];
            let col_lower = impl_var_lower(col_bounds!(self), row, col);
            let col_upper = impl_var_upper(col_bounds!(self), row, col);
            if self.mip.model.integrality[col] == VarType::Continuous {
                continue;
            }
            if val > max_abs_coef_value + self.primal_feastol {
                let delta = direction * (max_abs_coef_value - val);
                self.add_to_matrix(row, col, delta.value());
                *rhs += delta * col_upper;
            } else if val < -max_abs_coef_value - self.primal_feastol {
                let delta = -direction * (max_abs_coef_value + val);
                self.add_to_matrix(row, col, delta.value());
                *rhs += delta * col_lower;
            }
        }
    }

    /// `rowPresolve`'s Chvátal-Gomory strengthening of an all-integer
    /// inequality that cannot be scaled to integers.
    fn chvatal_gomory_strengthening(&mut self, row: usize, row_coefs: &[f64], row_index: &[usize]) -> Outcome {
        const MAX_DYNAMISM: f64 = 1e5;
        let small = self.settings.small_matrix_value;
        let feastol = self.primal_feastol;
        let mut rounded_row_coefs = vec![0.0; self.rowsize[row]];
        let mut scalars: Vec<f64> = vec![1.0];
        let model = &self.mip.model;

        // Direction -1 is a <= row, 1 a >= row.
        let direction = if model.row_upper[row] != INF { -1.0 } else { 1.0 };
        let mut rhs = CDouble::new(if direction < 0.0 { -model.row_upper[row] } else { model.row_lower[row] });

        // Complement or shift the columns to lower bounds of zero.
        let mut min_abs_coef = INF;
        let mut max_abs_coef = 0.0;
        for i in 0..row_coefs.len() {
            let col = row_index[i];
            let val = direction * row_coefs[i];
            let absval = val.abs();
            min_abs_coef = min(min_abs_coef, absval);
            max_abs_coef = max(max_abs_coef, absval);
            if val < 0.0 && model.col_upper[col] != INF {
                rhs -= val * CDouble::new(model.col_upper[col]);
            } else if val > 0.0 && model.col_lower[col] != -INF {
                rhs -= val * CDouble::new(model.col_lower[col]);
            } else {
                return Outcome::Ok;
            }
        }
        let dynamism = max_abs_coef / min_abs_coef;
        if dynamism <= MAX_DYNAMISM {
            for t in 1..=5 {
                let t = t as f64;
                scalars.push(t / max_abs_coef);
                scalars.push(t / min_abs_coef);
                scalars.push((2.0 * t - 1.0) / (2.0 * min_abs_coef));
            }
        }
        // `std::set<double>`: ascending, without repeats.
        scalars.sort_by(|a, b| a.partial_cmp(b).unwrap());
        scalars.dedup();

        let mut rounded_rhs = CDouble::new(0.0);
        let mut tightened = false;
        for &s in &scalars {
            // HiGHS's `roundRow`.
            let scalar = CDouble::new(s);
            rounded_rhs = (rhs * scalar - feastol).ceil();
            if rounded_rhs <= feastol {
                continue;
            }
            let rhs_ratio = rhs / rounded_rhs;
            let mut accept = false;
            let mut weaker = false;
            for i in 0..row_coefs.len() {
                let abs_coef = row_coefs[i].abs();
                rounded_row_coefs[i] = (abs_coef * scalar - TINY).ceil().value();
                let threshold = (rounded_row_coefs[i] * rhs_ratio).value();
                if abs_coef < threshold - small {
                    weaker = true;
                    break;
                }
                accept = accept || (abs_coef > threshold + small);
            }
            if !weaker && accept {
                tightened = true;
                break;
            }
        }
        if !tightened {
            return Outcome::Ok;
        }
        // Undo the shifting and complementing.
        for i in 0..row_coefs.len() {
            let col = row_index[i];
            let val = direction * row_coefs[i];
            if val < 0.0 && model.col_upper[col] != INF {
                rounded_row_coefs[i] = -rounded_row_coefs[i];
                rounded_rhs += rounded_row_coefs[i] * CDouble::new(model.col_upper[col]);
            } else if val > 0.0 && model.col_lower[col] != -INF {
                rounded_rhs += rounded_row_coefs[i] * CDouble::new(model.col_lower[col]);
            }
            rounded_row_coefs[i] *= direction;
        }
        rounded_rhs *= direction;
        // Replace the row by the rounded one.
        if direction < 0.0 {
            self.mip.model.row_upper[row] = rounded_rhs.value();
        } else {
            self.mip.model.row_lower[row] = rounded_rhs.value();
        }
        for i in 0..row_coefs.len() {
            let delta = (CDouble::new(rounded_row_coefs[i]) - row_coefs[i]).value();
            if delta.abs() > small {
                self.add_to_matrix(row, row_index[i], delta);
            }
        }
        Outcome::Ok
    }

    /// HiGHS's `extractVarBounds`: variable bounds a row with one binary
    /// gives its other columns.
    fn extract_var_bounds(&mut self, row: usize) {
        if self.rowsize[row] <= 1 || self.rowsize_integer[row] == 0 {
            return;
        }
        let num_inf_sum_lower = self.implied_row_bounds.num_inf_sum_lower(row);
        let num_inf_sum_upper = self.implied_row_bounds.num_inf_sum_upper(row);
        let mut use_lhs = self.mip.model.row_lower[row] != -INF && num_inf_sum_upper <= 1;
        let mut use_rhs = self.mip.model.row_upper[row] != INF && num_inf_sum_lower <= 1;
        if !use_lhs && !use_rhs {
            return;
        }
        let positions = self.row_vector(row);
        let mut bin_col = NONE;
        let mut bin_coef = 0.0;
        for &pos in &positions {
            let col = self.a_col[pos];
            if self.mip.model.col_lower[col] == self.mip.model.col_upper[col] {
                continue;
            }
            if self.is_binary(col) {
                if bin_col != NONE {
                    return;
                }
                bin_col = col;
                bin_coef = self.a_value[pos];
            }
        }
        if bin_col == NONE {
            return;
        }
        for &pos in &positions {
            let col = self.a_col[pos];
            let value = self.a_value[pos];
            if self.mip.model.col_lower[col] == self.mip.model.col_upper[col] || col == bin_col {
                continue;
            }
            let mut vlb_constant = -INF;
            if use_lhs {
                let residual = self.implied_row_bounds.residual_sum_upper(col_bounds!(self), row, col, value, bin_col, bin_coef, 0.0);
                if residual != INF {
                    vlb_constant = ((CDouble::new(self.mip.model.row_lower[row]) - residual) / value.abs()).value();
                    use_lhs = num_inf_sum_upper == 0;
                }
            }
            let mut vub_constant = INF;
            if use_rhs {
                let residual = self.implied_row_bounds.residual_sum_lower(col_bounds!(self), row, col, value, bin_col, bin_coef, 0.0);
                if residual != -INF {
                    vub_constant = ((CDouble::new(self.mip.model.row_upper[row]) - residual) / value.abs()).value();
                    use_rhs = num_inf_sum_lower == 0;
                }
            }
            if value < 0.0 {
                vlb_constant *= -1.0;
                vub_constant *= -1.0;
                std::mem::swap(&mut vlb_constant, &mut vub_constant);
            }
            let vb_coef = -bin_coef / value;
            let integral = self.mip.model.integrality[col] != VarType::Continuous;
            let feastol = self.mip.feastol;
            if vlb_constant != -INF {
                let lower = self.mip.model.col_lower[col];
                self.mip.implications.add_vlb_with(col, bin_col, vb_coef, vlb_constant, lower, integral, feastol);
            }
            if vub_constant != INF {
                let upper = self.mip.model.col_upper[col];
                self.mip.implications.add_vub_with(col, bin_col, vb_coef, vub_constant, upper, integral, feastol);
            }
            if !use_lhs && !use_rhs {
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_forcing_row(&mut self, row: usize, direction: f64, row_side: f64, implied_row_bound: f64, dynamism: CDouble, row_type: RowType) -> Outcome {
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
                if self.mip.model.col_upper[col] <= self.impl_col_upper[col] {
                    nfixings += 1;
                }
            } else if self.mip.model.col_lower[col] >= self.impl_col_lower[col] {
                nfixings += 1;
            }
        }
        if nfixings != self.rowsize[row] {
            return Outcome::Ok;
        }
        let _ = row_type;
        self.ps.forcing_row();
        self.mark_row_deleted(row);
        for &(col, value) in &row_vector {
            if direction * value > 0.0 {
                check!(self.fix_col_to_upper(col));
            } else {
                check!(self.fix_col_to_lower(col));
            }
        }
        self.ps.redundant_row();
        check!(self.remove_row_singletons());
        self.check_limits()
    }

    fn empty_col(&mut self, col: usize) -> Outcome {
        let cost = self.mip.model.col_cost[col];
        if (cost > 0.0 && self.mip.model.col_lower[col] == -INF) || (cost < 0.0 && self.mip.model.col_upper[col] == INF) {
            if cost.abs() <= self.settings.dual_feasibility_tolerance {
                self.mip.model.col_cost[col] = 0.0;
            } else {
                return Outcome::DualInfeasible;
            }
        }
        let cost = self.mip.model.col_cost[col];
        if cost > 0.0 {
            check!(self.fix_col_to_lower(col));
        } else if cost < 0.0 || self.mip.model.col_upper[col].abs() < self.mip.model.col_lower[col].abs() {
            check!(self.fix_col_to_upper(col));
        } else if self.mip.model.col_lower[col] != -INF {
            check!(self.fix_col_to_lower(col));
        } else {
            self.fix_col_to_zero(col);
        }
        self.check_limits()
    }

    fn col_presolve(&mut self, col: usize) -> Outcome {
        let (outcome, is_fixed) = self.check_col_bounds(col);
        check!(outcome);
        if is_fixed {
            self.ps.fixed_col(col, self.mip.model.col_lower[col]);
            self.remove_fixed_col(col);
            return self.check_limits();
        }
        match self.colsize[col] {
            0 => return self.empty_col(col),
            1 => return self.singleton_col(col),
            _ => {}
        }
        check!(self.detect_dominated_col(col, true));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }

        // Integer columns cannot tighten the bounds of row duals.
        let modify_implied_row_dual_bound = |s: &mut Self, row: usize, direction: f64, is_bound_implied: bool, num_inf: i32| {
            if is_bound_implied && row != NONE && num_inf == 1 && direction * s.mip.model.col_cost[col] >= 0.0 && !s.is_ranged(row) {
                let nz_pos = s.find_nonzero(row, col);
                let value = s.a_value[nz_pos];
                if s.mip.model.integrality[col] != VarType::Integer || (s.rowsize_integer[row] == s.rowsize[row] && s.row_coefficients_integral(row, 1.0 / value)) {
                    if direction * value > 0.0 {
                        s.change_impl_row_dual_lower(row, 0.0, col);
                    } else {
                        s.change_impl_row_dual_upper(row, 0.0, col);
                    }
                }
            }
        };
        let (source, implied, num_inf) = (
            self.col_lower_source[col],
            self.is_lower_implied(col),
            self.implied_dual_row_bounds.num_inf_sum_upper_orig(col),
        );
        modify_implied_row_dual_bound(self, source, 1.0, implied, num_inf);
        let (source, implied, num_inf) = (
            self.col_upper_source[col],
            self.is_upper_implied(col),
            self.implied_dual_row_bounds.num_inf_sum_lower_orig(col),
        );
        modify_implied_row_dual_bound(self, source, -1.0, implied, num_inf);

        check!(self.convert_implied_integer(col, NONE, false).result);

        // Shift integer columns to a lower bound of zero.
        let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
        if self.mip.model.integrality[col] != VarType::Continuous && lower != 0.0 && (lower != -INF || upper != INF) && upper - lower > 0.5 {
            if upper.abs() > lower.abs() {
                if lower.abs() < 1000.5 {
                    check!(self.transform_column(col, 1.0, lower));
                }
            } else if upper.abs() < 1000.5 {
                check!(self.transform_column(col, -1.0, upper));
            }
        }

        check!(self.dual_fixing(col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        check!(self.singleton_col_stuffing(col));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        if self.mip.model.integrality[col] != VarType::Integer {
            self.update_row_dual_implied_bounds_of_col(col);
        }
        Outcome::Ok
    }

    fn detect_dominated_col(&mut self, col: usize, handle_singleton_rows: bool) -> Outcome {
        let cost = CDouble::new(-self.mip.model.col_cost[col]);
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

        let fix = |s: &mut Self, direction: f64| -> Outcome {
            if direction > 0.0 {
                check!(s.fix_col_to_lower(col));
            } else {
                check!(s.fix_col_to_upper(col));
            }
            if handle_singleton_rows {
                check!(s.remove_row_singletons());
            }
            s.check_limits()
        };
        let dominated_col = |s: &mut Self, dual_bound: f64, bound: f64, direction: f64| -> Outcome {
            if direction * dual_bound <= tol {
                return Outcome::Ok;
            }
            if direction * bound == -INF {
                return Outcome::DualInfeasible;
            }
            fix(s, direction)
        };
        let weakly_dominated_col = |s: &mut Self, dual_bound: f64, bound: f64, other_bound: f64, direction: f64| -> Outcome {
            if direction * dual_bound < -tol {
                return Outcome::Ok;
            }
            if direction * bound != -INF {
                return fix(s, direction);
            }
            // A forcing column.
            let cost = CDouble::new(-s.mip.model.col_cost[col]);
            let bound_on_col_dual = if direction > 0.0 {
                -s.implied_dual_row_bounds.sum_upper_orig(col, cost)
            } else {
                -s.implied_dual_row_bounds.sum_lower_orig(col, cost)
            };
            if bound_on_col_dual.abs() <= (CDouble::new(tol) / dynamism).value().max(f64::EPSILON) {
                let col_integral = s.mip.model.integrality[col] == VarType::Integer;
                let entries = s.col_entries(col);
                s.ps.forcing_column(col, entries, other_bound, direction < 0.0, col_integral);
                s.mark_col_deleted(col);
                let mut coliter = s.colhead[col];
                while coliter != NONE {
                    let row = s.a_row[coliter];
                    let rhs = if direction * s.a_value[coliter] > 0.0 {
                        s.mip.model.row_upper[row]
                    } else {
                        s.mip.model.row_lower[row]
                    };
                    coliter = s.a_next[coliter];
                    let entries = s.row_entries(row);
                    s.ps.forcing_column_removed_row(col, row, rhs, entries);
                    s.remove_row(row);
                }
                return s.check_limits();
            }
            Outcome::Ok
        };

        let lower = self.mip.model.col_lower[col];
        check!(dominated_col(self, col_dual_lower, lower, 1.0));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        let upper = self.mip.model.col_upper[col];
        check!(dominated_col(self, col_dual_upper, upper, -1.0));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
        check!(weakly_dominated_col(self, col_dual_lower, lower, upper, 1.0));
        if self.col_deleted[col] {
            return Outcome::Ok;
        }
        let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
        weakly_dominated_col(self, col_dual_upper, upper, lower, -1.0)
    }

    fn dual_fixing(&mut self, col: usize) -> Outcome {
        if self.mip.model.col_lower[col] == self.mip.model.col_upper[col] {
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
            let cost = self.mip.model.col_cost[col];
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
                if self.mip.model.col_cost[col] > tol || self.mip.model.col_lower[col] > -INF {
                    check!(self.fix_col_to_lower(col));
                }
            } else if self.mip.model.col_cost[col] < -tol || self.mip.model.col_upper[col] < INF {
                check!(self.fix_col_to_upper(col));
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
                check!(self.handle_single_equation(equation_row));
                self.single_equation_checked[equation_row] = true;
                if self.col_deleted[col] {
                    return Outcome::Ok;
                }
            } else if self.mip.model.col_lower[col] != -INF && self.mip.model.col_upper[col] != INF {
                if has_single_down_lock {
                    let (upper, lower) = (self.mip.model.col_upper[col], self.mip.model.col_lower[col]);
                    check!(self.dual_fixing_substitute_col(col, down_lock_row, 1.0, upper, lower));
                    if self.col_deleted[col] {
                        return Outcome::Ok;
                    }
                }
                if has_single_up_lock {
                    let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
                    check!(self.dual_fixing_substitute_col(col, up_lock_row, -1.0, lower, upper));
                    if self.col_deleted[col] {
                        return Outcome::Ok;
                    }
                }
            }
        }
        let integral = self.mip.model.integrality[col] != VarType::Continuous;
        if let Some(new_bound) = self.has_tighter_bound(col, 1.0, self.mip.model.col_upper[col]) {
            let new_bound = max(new_bound, self.mip.model.col_lower[col]);
            if new_bound < self.mip.model.col_upper[col] - self.primal_feastol {
                if new_bound == self.mip.model.col_lower[col] {
                    check!(self.fix_col_to_lower(col));
                } else if integral {
                    check!(self.change_col_upper(col, new_bound));
                }
            }
        } else if let Some(new_bound) = self.has_tighter_bound(col, -1.0, self.mip.model.col_lower[col]) {
            let new_bound = min(new_bound, self.mip.model.col_upper[col]);
            if new_bound > self.mip.model.col_lower[col] + self.primal_feastol {
                if new_bound == self.mip.model.col_upper[col] {
                    check!(self.fix_col_to_upper(col));
                } else if integral {
                    check!(self.change_col_lower(col, new_bound));
                }
            }
        }
        Outcome::Ok
    }

    /// `dualFixing`'s `substituteCol`: substitute `col` by a binary in its
    /// one locking row whose upper bound makes the row redundant and
    /// implies the column's bound.
    fn dual_fixing_substitute_col(&mut self, col: usize, row: usize, direction: f64, col_bound: f64, other_col_bound: f64) -> Outcome {
        let lhs_finite = self.mip.model.row_lower[row] != -INF;
        let rhs_finite = self.mip.model.row_upper[row] != INF;
        self.store_row(row);
        for k in 0..self.rowpositions_len {
            let pos = self.rowpositions[k];
            let (nz_col, nz_val) = (self.a_col[pos], self.a_value[pos]);
            if nz_col == col {
                continue;
            }
            if self.mip.model.integrality[nz_col] != VarType::Integer || self.mip.model.col_lower[nz_col] != 0.0 || self.mip.model.col_upper[nz_col] != 1.0 {
                continue;
            }
            if (rhs_finite
                && self.implied_row_bounds.residual_sum_upper_orig(col_bounds!(self), row, nz_col, nz_val, NONE, -INF, -INF) > self.mip.model.row_upper[row] + self.primal_feastol)
                || (lhs_finite
                    && self.implied_row_bounds.residual_sum_lower_orig(col_bounds!(self), row, nz_col, nz_val, NONE, -INF, -INF)
                        < self.mip.model.row_lower[row] - self.primal_feastol)
            {
                continue;
            }
            let nz_upper = self.mip.model.col_upper[nz_col];
            let best_bound = if direction > 0.0 {
                self.compute_implied_lower_bound(col, nz_col, nz_upper, 0)
            } else {
                -self.compute_implied_upper_bound(col, nz_col, nz_upper, 0)
            };
            if best_bound >= direction * col_bound - self.primal_feastol {
                let offset = other_col_bound;
                let scale = col_bound - other_col_bound;
                self.ps.doubleton_equation(col, nz_col, 1.0, -scale, offset);
                self.mark_col_deleted(col);
                self.substitute_cols(col, nz_col, offset, scale);
                check!(self.check_limits());
                break;
            }
        }
        Outcome::Ok
    }

    /// HiGHS's `hasTighterBound` lambda in `dualFixing`.
    fn has_tighter_bound(&mut self, col: usize, direction: f64, current_bound: f64) -> Option<f64> {
        if direction * self.mip.model.col_cost[col] < 0.0 {
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
    fn handle_single_equation(&mut self, row: usize) -> Outcome {
        struct EquationNonzero {
            col: usize,
            val: f64,
            mark: i32,
        }
        let check_column = |s: &Self, col: usize, col_direction: f64| -> bool {
            if col_direction * s.mip.model.col_cost[col] < 0.0 {
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
                let row_direction = if s.mip.model.row_lower[r] == -INF && s.mip.model.row_upper[r] != INF {
                    1.0
                } else {
                    -1.0
                };
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
            let bound = if direction * val > 0.0 {
                s.mip.model.col_upper[col]
            } else {
                s.mip.model.col_lower[col]
            };
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
            let integral = self.mip.model.integrality[nz.col] != VarType::Continuous;
            if nz.mark <= 0 || integral {
                compute_activity(self, nz.col, nz.val, &mut activity_t_plus, &mut t_plus_finite, 1.0);
            } else {
                compute_activity(self, nz.col, nz.val, &mut activity_sc_plus, &mut sc_plus_finite, -1.0);
            }
            if nz.mark >= 0 || integral {
                compute_activity(self, nz.col, nz.val, &mut activity_t_minus, &mut t_minus_finite, -1.0);
            } else {
                compute_activity(self, nz.col, nz.val, &mut activity_sc_minus, &mut sc_minus_finite, 1.0);
            }
            if (num_s_minus == 0 || !t_plus_finite || !sc_plus_finite) && (num_s_plus == 0 || !t_minus_finite || !sc_minus_finite) {
                break;
            }
        }
        let fix_cols = |s: &mut Self, direction: i32| -> Outcome {
            for nz in &equation_row_vector {
                if direction * nz.mark >= 0 {
                    continue;
                }
                if direction as f64 * nz.val > 0.0 {
                    check!(s.fix_col_to_upper(nz.col));
                } else {
                    check!(s.fix_col_to_lower(nz.col));
                }
            }
            Outcome::Ok
        };
        let row_lower = self.mip.model.row_lower[row];
        if num_s_minus > 0 && t_plus_finite && sc_plus_finite && (activity_t_plus + activity_sc_plus).value() <= row_lower + self.primal_feastol {
            check!(fix_cols(self, 1));
        } else if num_s_plus > 0 && t_minus_finite && sc_minus_finite && (activity_t_minus + activity_sc_minus).value() >= row_lower - self.primal_feastol {
            check!(fix_cols(self, -1));
        }
        Outcome::Ok
    }

    fn is_stuffing_singleton(&self, col: usize) -> bool {
        !self.col_deleted[col] && self.colsize[col] == 1 && self.mip.model.col_lower[col] != self.mip.model.col_upper[col]
    }

    /// `singletonColStuffing`'s `computeCandidates`: false when both
    /// activity bounds are lost.
    fn stuffing_candidates(&mut self, row: usize, direction: f64, st: &mut StuffingState, allow_integer_candidates: bool) -> bool {
        st.candidates.clear();
        st.candidates.reserve(self.rowsize[row]);
        st.sum_lower = CDouble::new(0.0);
        st.sum_upper = CDouble::new(0.0);
        st.sum_lower_finite = true;
        st.sum_upper_finite = true;
        st.num_integer_candidates = 0;
        st.min_weight = INF;
        st.max_weight = -INF;
        for (j, value) in self.row_entries(row) {
            let aj = direction * value;
            let cj = self.mip.model.col_cost[j];
            let mut sum_lower_bound = self.mip.model.col_lower[j];
            let mut sum_upper_bound = self.mip.model.col_upper[j];
            let is_candidate = allow_integer_candidates || self.mip.model.integrality[j] != VarType::Integer;
            let mut add_candidate = |multiplier: f64| {
                if self.mip.model.integrality[j] == VarType::Integer {
                    st.num_integer_candidates += 1;
                }
                st.min_weight = min(st.min_weight, multiplier * aj);
                st.max_weight = max(st.max_weight, multiplier * aj);
                st.candidates.push(StuffingCandidate { col: j, val: aj, multiplier });
            };
            if self.is_stuffing_singleton(j) {
                if aj > 0.0 {
                    if cj >= 0.0 {
                        sum_upper_bound = sum_lower_bound;
                    } else if is_candidate {
                        sum_upper_bound = sum_lower_bound;
                        add_candidate(1.0);
                    }
                } else if cj <= 0.0 {
                    sum_lower_bound = sum_upper_bound;
                } else if is_candidate {
                    sum_lower_bound = sum_upper_bound;
                    add_candidate(-1.0);
                }
            }
            if aj < 0.0 {
                std::mem::swap(&mut sum_lower_bound, &mut sum_upper_bound);
            }
            st.sum_lower_finite = st.sum_lower_finite && sum_lower_bound.abs() != INF;
            st.sum_upper_finite = st.sum_upper_finite && sum_upper_bound.abs() != INF;
            if st.sum_lower_finite {
                st.sum_lower += aj * CDouble::new(sum_lower_bound);
            }
            if st.sum_upper_finite {
                st.sum_upper += aj * CDouble::new(sum_upper_bound);
            }
            if !st.sum_lower_finite && !st.sum_upper_finite {
                return false;
            }
        }
        true
    }

    /// HiGHS's `singletonColStuffing` (Gamrath et al., "Progress in
    /// presolving for mixed integer programming").
    fn singleton_col_stuffing(&mut self, col: usize) -> Outcome {
        if !self.is_stuffing_singleton(col) {
            return Outcome::Ok;
        }
        let row = self.a_row[self.colhead[col]];
        if self.rowsize[row] <= 1 || self.is_ranged(row) {
            return Outcome::Ok;
        }
        let check_row = |s: &mut Self, rhs: f64, direction: f64| -> Outcome {
            if direction * rhs == INF {
                return Outcome::Ok;
            }
            let mut st = StuffingState::default();
            // HiGHS's `checkCandidates`.
            let usable = 'check: {
                if s.stuffing_candidates(row, direction, &mut st, true) {
                    if st.num_integer_candidates == 0 {
                        break 'check true;
                    }
                    if st.num_integer_candidates == st.candidates.len() {
                        break 'check st.min_weight == st.max_weight;
                    }
                }
                st.num_integer_candidates > 0 && s.stuffing_candidates(row, direction, &mut st, false)
            };
            if !usable {
                return Outcome::Ok;
            }
            {
                let cost = &s.mip.model.col_cost;
                pdqsort(&mut st.candidates, |c1, c2| cost[c1.col] / c1.val < cost[c2.col] / c2.val);
            }
            for t in &st.candidates {
                if s.mip.model.col_lower[t.col] == -INF || s.mip.model.col_upper[t.col] == INF {
                    break;
                }
                let delta = (t.multiplier * t.val) * (CDouble::new(s.mip.model.col_upper[t.col]) - CDouble::new(s.mip.model.col_lower[t.col]));
                if st.sum_upper_finite && delta <= direction * rhs - st.sum_upper + s.primal_feastol {
                    if t.multiplier < 0.0 {
                        check!(s.fix_col_to_lower(t.col));
                    } else {
                        check!(s.fix_col_to_upper(t.col));
                    }
                } else if st.sum_lower_finite && delta <= st.sum_lower - direction * rhs + s.primal_feastol {
                    if -t.multiplier < 0.0 {
                        check!(s.fix_col_to_lower(t.col));
                    } else {
                        check!(s.fix_col_to_upper(t.col));
                    }
                }
                if st.sum_lower_finite {
                    st.sum_lower += delta;
                }
                if st.sum_upper_finite {
                    st.sum_upper += delta;
                }
            }
            Outcome::Ok
        };
        let rhs = self.mip.model.row_upper[row];
        check!(check_row(self, rhs, 1.0));
        let rhs = self.mip.model.row_lower[row];
        check!(check_row(self, rhs, -1.0));
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
        let compute_bound = |s: &Self, t: &Triplet, rhs: f64, direction: f64, is_lower: bool, worst_case: bool| -> f64 {
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
            let bound = ((CDouble::new(rhs) - CDouble::new(residual)) / t.jval).value();
            // An integer column's bound is rounded here (HiGHS #3411). An
            // implied bound holds for every feasible point, so the primal
            // tolerance is safe; a worst-case bound is where the column may
            // be fixed, and moving past it by delta violates the row by
            // |jval| delta, so its tolerance is divided by |jval|.
            if s.mip.model.integrality[col] != VarType::Continuous {
                let tol = if worst_case { s.primal_feastol / max(1.0, t.jval.abs()) } else { s.primal_feastol };
                return if is_lower { (bound - tol).ceil() } else { (bound + tol).floor() };
            }
            bound
        };
        for t in &nzs {
            for (rhs, direction) in [(self.mip.model.row_upper[t.row], 1.0), (self.mip.model.row_lower[t.row], -1.0)] {
                if direction * rhs == INF {
                    continue;
                }
                if direction * t.jval < 0.0 {
                    if want[0] {
                        lower_bound = max(lower_bound, compute_bound(self, t, rhs, direction, true, false));
                    }
                    if want[2] && worst_case_lower_bound != INF {
                        worst_case_lower_bound = max(worst_case_lower_bound, compute_bound(self, t, rhs, direction, true, true));
                    }
                } else {
                    if want[1] {
                        upper_bound = min(upper_bound, compute_bound(self, t, rhs, direction, false, false));
                    }
                    if want[3] && worst_case_upper_bound != -INF {
                        worst_case_upper_bound = min(worst_case_upper_bound, compute_bound(self, t, rhs, direction, false, true));
                    }
                }
            }
        }
        (lower_bound, upper_bound, worst_case_lower_bound, worst_case_upper_bound)
    }
}

// ---- Passes ----------------------------------------------------------------

impl MipPresolve<'_> {
    fn remove_row_singletons(&mut self) -> Outcome {
        let mut i = 0;
        while i < self.singleton_rows.len() {
            let row = self.singleton_rows[i];
            i += 1;
            if self.row_deleted[row] || self.rowsize[row] > 1 {
                continue;
            }
            check!(self.row_presolve(row));
        }
        self.singleton_rows.clear();
        Outcome::Ok
    }

    fn presolve_col_singletons(&mut self) -> Outcome {
        let mut i = 0;
        while i < self.singleton_columns.len() {
            let col = self.singleton_columns[i];
            i += 1;
            if self.col_deleted[col] {
                continue;
            }
            check!(self.col_presolve(col));
        }
        let (col_deleted, colsize) = (&self.col_deleted, &self.colsize);
        self.singleton_columns.retain(|&col| !(col_deleted[col] || colsize[col] > 1));
        Outcome::Ok
    }

    fn presolve_changed_rows(&mut self) -> Outcome {
        let mut changed_rows = Vec::with_capacity(self.mip.model.num_row - self.num_deleted_rows);
        std::mem::swap(&mut changed_rows, &mut self.changed_row_indices);
        for row in changed_rows {
            if self.row_deleted[row] {
                continue;
            }
            check!(self.row_presolve(row));
            self.changed_row_flag[row] = self.row_deleted[row];
        }
        Outcome::Ok
    }

    fn presolve_changed_cols(&mut self) -> Outcome {
        let mut changed_cols = Vec::with_capacity(self.mip.model.num_col - self.num_deleted_cols);
        std::mem::swap(&mut changed_cols, &mut self.changed_col_indices);
        for col in changed_cols {
            if self.col_deleted[col] {
                continue;
            }
            check!(self.col_presolve(col));
            self.changed_col_flag[col] = self.col_deleted[col];
        }
        Outcome::Ok
    }

    fn remove_doubleton_equations(&mut self) -> Outcome {
        let mut eq = self.equations.iter().next().copied();
        while let Some(key) = eq {
            let eqrow = key.1;
            if self.rowsize[eqrow] > 2 {
                return Outcome::Ok;
            }
            check!(self.row_presolve(eqrow));
            eq = if self.row_deleted[eqrow] {
                self.equations.iter().next().copied()
            } else {
                self.equations.range((Excluded(key), Unbounded)).next().copied()
            };
        }
        Outcome::Ok
    }

    fn initial_row_and_col_presolve(&mut self) -> Outcome {
        for row in 0..self.mip.model.num_row {
            if self.row_deleted[row] {
                continue;
            }
            check!(self.row_presolve(row));
            self.changed_row_flag[row] = false;
        }
        for col in 0..self.mip.model.num_col {
            if self.col_deleted[col] {
                continue;
            }
            if self.mip.model.integrality[col] != VarType::Continuous {
                let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
                check!(self.change_col_bounds(col, lower, upper));
            }
            check!(self.col_presolve(col));
            self.changed_col_flag[col] = false;
        }
        self.check_limits()
    }

    fn fast_presolve_loop(&mut self) -> Outcome {
        loop {
            self.store_current_problem_size();
            check!(self.remove_row_singletons());
            check!(self.presolve_changed_rows());
            check!(self.remove_doubleton_equations());
            check!(self.presolve_col_singletons());
            check!(self.presolve_changed_cols());
            // Not `<=`: a reduction of NaN (nothing left to reduce) stops it.
            if self.problem_size_reduction().partial_cmp(&0.01).is_none_or(std::cmp::Ordering::is_le) {
                return Outcome::Ok;
            }
        }
    }

    fn aggregator(&mut self) -> Outcome {
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
            if self.mip.model.integrality[col] == VarType::Integer {
                let implied_integral = self.is_implied_integral(col);
                check!(implied_integral.result);
                if !implied_integral.flag {
                    continue;
                }
            }
            if self.rowsize[row] == 2 || self.colsize[col] == 2 {
                self.store_row(row);
                self.substitute_free_col(row, col, true);
                self.substitution_opportunities[i - 1].0 = NONE;
                check!(self.remove_row_singletons());
                check!(self.check_limits());
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
            self.substitute_free_col(row, col, true);
            self.substitution_opportunities[i - 1].0 = NONE;
            check!(self.remove_row_singletons());
            check!(self.check_limits());
        }
        self.substitution_opportunities.retain(|&(row, _)| row != NONE);
        Outcome::Ok
    }

    fn count_fillin(&mut self, row: usize) -> i64 {
        let mut fillin = 0;
        for k in 0..self.rowpositions_len {
            let col = self.a_col[self.rowpositions[k]];
            if self.find_nonzero(row, col) == NONE {
                fillin += 1;
            }
        }
        fillin
    }

    fn detect_parallel_rows_and_cols(&mut self) -> Outcome {
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
        for i in 0..self.mip.model.num_col {
            if self.col_deleted[i] {
                continue;
            }
            if self.colsize[i] == 0 {
                check!(self.col_presolve(i));
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
                let col;
                let duplicate_col;
                let col_scale;
                let mut check_col_impl_bounds = true;
                let mut check_duplicate_col_impl_bounds = true;
                let int_i = self.mip.model.integrality[i] == VarType::Integer;
                let int_cand = self.mip.model.integrality[parallel_col_candidate] == VarType::Integer;
                if int_i && int_cand {
                    // Both integer: the scale must be integral, so the
                    // smaller largest value is col's.
                    if col_max[i].0.abs() < col_max[parallel_col_candidate].0.abs() {
                        col = i;
                        duplicate_col = parallel_col_candidate;
                    } else {
                        col = parallel_col_candidate;
                        duplicate_col = i;
                    }
                    let scale_cand = col_max[duplicate_col].0 / col_max[col].0;
                    col_scale = scale_cand.round();
                    if (scale_cand - col_scale).abs() > small {
                        continue;
                    }
                    // Scaled by more than one, col moves on a grid the
                    // duplicate cannot always make up for.
                    if col_scale != 1.0 {
                        check_duplicate_col_impl_bounds = false;
                    }
                } else if int_i {
                    col = i;
                    duplicate_col = parallel_col_candidate;
                    col_scale = col_max[duplicate_col].0 / col_max[col].0;
                    check_col_impl_bounds = false;
                } else {
                    col = parallel_col_candidate;
                    duplicate_col = i;
                    col_scale = col_max[duplicate_col].0 / col_max[col].0;
                    check_col_impl_bounds = !int_cand;
                }
                // A MIP needs no dual postsolve, so weakly redundant bounds
                // will do.
                let col_upper_inf = |s: &Self| check_col_impl_bounds && if col_scale > 0.0 { s.is_upper_implied(col) } else { s.is_lower_implied(col) };
                let col_lower_inf = |s: &Self| check_col_impl_bounds && if col_scale > 0.0 { s.is_lower_implied(col) } else { s.is_upper_implied(col) };
                let duplicate_col_upper_inf = |s: &Self| check_duplicate_col_impl_bounds && s.is_upper_implied(duplicate_col);
                let duplicate_col_lower_inf = |s: &Self| check_duplicate_col_impl_bounds && s.is_lower_implied(duplicate_col);
                let obj_diff = (CDouble::new(col_scale) * self.mip.model.col_cost[col] - self.mip.model.col_cost[duplicate_col]).value();
                const MERGE: u8 = 0;
                const COL_TO_UPPER: u8 = 1;
                const COL_TO_LOWER: u8 = 2;
                const DUPLICATE_TO_UPPER: u8 = 3;
                const DUPLICATE_TO_LOWER: u8 = 4;
                let (cl, cu, dl, du) = (
                    self.mip.model.col_lower[col],
                    self.mip.model.col_upper[col],
                    self.mip.model.col_lower[duplicate_col],
                    self.mip.model.col_upper[duplicate_col],
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
                if reduction_case == MERGE && self.mip.model.integrality[col] == VarType::Integer {
                    let illegal_scale = if self.mip.model.integrality[duplicate_col] != VarType::Integer {
                        // The scale must not exceed 1/(y_u - y_l).
                        (col_scale * (du - dl)).abs() < 1.0 - feastol
                    } else {
                        // Both integer: the scale must not exceed
                        // (x_u - x_l) + 1.
                        col_scale.abs() > cu - cl + 1.0 + feastol
                    };
                    if illegal_scale {
                        continue;
                    }
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
                        check!(self.fix_col_to_lower(duplicate_col));
                    }
                    DUPLICATE_TO_UPPER => {
                        del_col = duplicate_col;
                        unsingleton(self, duplicate_col);
                        check!(self.fix_col_to_upper(duplicate_col));
                    }
                    COL_TO_LOWER => {
                        del_col = col;
                        unsingleton(self, col);
                        check!(self.fix_col_to_lower(col));
                    }
                    COL_TO_UPPER => {
                        del_col = col;
                        unsingleton(self, col);
                        check!(self.fix_col_to_upper(col));
                    }
                    _ => {
                        // A merge the postsolve stack refuses would lose
                        // integrality (HiGHS 1.15 made it anyway; fixed in
                        // #3261).
                        let x_int = self.mip.model.integrality[col] == VarType::Integer;
                        let y_int = self.mip.model.integrality[duplicate_col] == VarType::Integer;
                        if !self
                            .ps
                            .duplicate_column(col_scale, cl, cu, dl, du, col, duplicate_col, x_int, y_int, self.settings.primal_feasibility_tolerance)
                        {
                            continue;
                        }
                        // A continuous column merged into an integer one
                        // makes it continuous.
                        let rowsize_int_reduction = !y_int && x_int;
                        if rowsize_int_reduction {
                            self.mip.model.integrality[col] = VarType::Continuous;
                        }
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
                            if rowsize_int_reduction {
                                self.rowsize_integer[colrow] -= 1;
                            }
                            coliter = self.a_next[coliter];
                            self.unlink(colpos);
                            self.reinsert_equation(colrow);
                        }
                        self.mip.model.col_cost[duplicate_col] = 0.0;
                        del_col = duplicate_col;
                        self.reset_col_implied_bounds(col, NONE);
                        if rowsize_int_reduction && self.mip.model.integrality[duplicate_col] == VarType::ImplicitInteger {
                            let implied_integer = self.is_implied_integer(col);
                            check!(implied_integer.result);
                            if implied_integer.flag {
                                self.convert_implied_integer(col, NONE, true);
                            }
                        }
                    }
                }
                break;
            }
            if del_col != NONE {
                if del_col != i {
                    buckets.get_mut(&col_hashes[i]).unwrap().remove(last);
                }
                check!(self.check_limits());
                check!(self.remove_row_singletons());
            } else {
                buckets.entry(col_hashes[i]).or_default().push(i);
            }
        }

        buckets.clear();
        let num_singletons = |map: &HashMap<usize, i64>, row: usize| map.get(&row).copied().unwrap_or(0);
        for i in 0..self.mip.model.num_row {
            if self.row_deleted[i] {
                continue;
            }
            if self.rowsize[i] <= 1 || (self.rowsize[i] == 2 && self.is_equation(i)) {
                check!(self.row_presolve(i));
                continue;
            }
            let num_singleton = num_singletons(&num_row_singletons, i);
            let candidates = buckets.get(&row_hashes[i]).cloned().unwrap_or_default();
            let mut del_row = NONE;
            let mut last = NONE;
            if !candidates.is_empty() {
                self.store_row(i);
            }
            for (k, &parallel_row_cand) in candidates.iter().enumerate() {
                last = k;
                let num_singleton_candidate = num_singletons(&num_row_singletons, parallel_row_cand);
                if self.rowsize[i] as i64 - num_singleton != self.rowsize[parallel_row_cand] as i64 - num_singleton_candidate {
                    continue;
                }
                if num_singleton_candidate > 1 || num_singleton > 1 {
                    // More than one extra singleton only when the other row
                    // has none and is an equation, which then sparsifies it.
                    if (num_singleton != 0 || !self.is_equation(i)) && (num_singleton_candidate != 0 || !self.is_equation(parallel_row_cand)) {
                        continue;
                    }
                } else if num_singleton_candidate != num_singleton && !self.is_equation(i) && !self.is_equation(parallel_row_cand) {
                    continue;
                }
                let row_scale = row_max[parallel_row_cand].0 / row_max[i].0;
                let mut parallel = true;
                for k in 0..self.rowpositions_len {
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
                if num_singleton != 0 || num_singleton_candidate != 0 {
                    if self.is_equation(i) {
                        // Add the equation to the other row.
                        let rowvector = self.stored_row_entries();
                        check!(self.equality_row_addition(i, parallel_row_cand, -row_scale, rowvector));
                        del_row = parallel_row_cand;
                    } else if self.is_equation(parallel_row_cand) {
                        let rowvector = self.row_entries(parallel_row_cand);
                        check!(self.equality_row_addition(parallel_row_cand, i, -row_max[i].0 / row_max[parallel_row_cand].0, rowvector));
                        del_row = i;
                    }
                    // HiGHS leaves two inequalities with one singleton each
                    // for later.
                    continue;
                }
                let mut row_lower_tightened = false;
                let mut row_upper_tightened = false;
                let (mut new_lower, mut new_upper) = if row_scale > 0.0 {
                    (self.mip.model.row_lower[i] * row_scale, self.mip.model.row_upper[i] * row_scale)
                } else {
                    (self.mip.model.row_upper[i] * row_scale, self.mip.model.row_lower[i] * row_scale)
                };
                let cand = parallel_row_cand;
                if new_upper < self.mip.model.row_upper[cand] {
                    if new_upper < self.mip.model.row_lower[cand] - feastol {
                        return Outcome::PrimalInfeasible;
                    }
                    if new_upper <= self.mip.model.row_lower[cand] + feastol {
                        new_upper = self.mip.model.row_lower[cand];
                    }
                    if new_upper < self.mip.model.row_upper[cand] {
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
                        self.mip.model.row_upper[cand] = new_upper;
                    }
                }
                if new_lower > self.mip.model.row_lower[cand] {
                    if new_lower > self.mip.model.row_upper[cand] + feastol {
                        return Outcome::PrimalInfeasible;
                    }
                    if new_lower >= self.mip.model.row_upper[cand] - feastol {
                        new_lower = self.mip.model.row_upper[cand];
                    }
                    if new_lower > self.mip.model.row_lower[cand] {
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
                        self.mip.model.row_lower[cand] = new_lower;
                    }
                }
                self.reset_row_dual_implied_bounds(cand, NONE);
                let _ = (row_upper_tightened, row_lower_tightened);
                self.ps.duplicate_row();
                del_row = i;
                self.mark_row_deleted(i);
                for k in 0..self.rowpositions_len {
                    let pos = self.rowpositions[k];
                    self.unlink(pos);
                }
                break;
            }
            if del_row != NONE {
                if del_row != i {
                    buckets.get_mut(&row_hashes[i]).unwrap().remove(last);
                }
                check!(self.check_limits());
            } else {
                buckets.entry(row_hashes[i]).or_default().push(i);
            }
        }
        Outcome::Ok
    }

    fn shrink_and_rebuild_if_halved(&mut self) {
        if self.shrink_problem_enabled && (self.num_deleted_cols >= self.mip.model.num_col / 2 || self.num_deleted_rows >= self.mip.model.num_row / 2) {
            self.shrink_problem();
            self.rebuild_storage();
        }
    }

    /// HiGHS's `presolve` for a MIP. The model is already a minimisation.
    fn presolve(&mut self) -> Outcome {
        self.mip.cliquetable.set_presolve_flag(true);
        check!(self.initial_row_and_col_presolve());
        let mut num_parallel_row_col_calls = 0;
        let mut try_sparsify = true;
        let mut try_probing = true;
        let mut num_cliques_before_probing: Option<usize> = None;
        let mut domcol_after_probing_called = false;
        // HiGHS removes dependent equations for an LP only.
        loop {
            check!(self.fast_presolve_loop());
            self.store_current_problem_size();
            // After a restart the clique table and implications may hold
            // substitutions, which are applied before the aggregator as
            // they may lose validity otherwise.
            let mut num_del_col = 0;
            check!(self.apply_conflict_graph_substitutions(&mut num_del_col));
            check!(self.aggregator());
            if self.problem_size_reduction() > 0.05 {
                continue;
            }
            if try_sparsify {
                let num_nz = self.num_nonzeros();
                check!(self.sparsify());
                let nz_reduction = 100.0 * (1.0 - (self.num_nonzeros() as f64 / num_nz as f64));
                if nz_reduction > 0.0 {
                    check!(self.fast_presolve_loop());
                }
                try_sparsify = false;
            }
            if num_parallel_row_col_calls < 5 {
                self.shrink_and_rebuild_if_halved();
                self.store_current_problem_size();
                check!(self.detect_parallel_rows_and_cols());
                num_parallel_row_col_calls += 1;
                if self.problem_size_reduction() > 0.05 {
                    continue;
                }
            }
            check!(self.fast_presolve_loop());
            let mut num_strengthened = 0;
            check!(self.strengthen_inequalities(&mut num_strengthened));
            check!(self.fast_presolve_loop());
            if num_cliques_before_probing.is_none() {
                num_cliques_before_probing = Some(self.mip.cliquetable.num_cliques());
                self.store_current_problem_size();
                check!(self.dominated_columns());
                if self.problem_size_reduction() > 0.0 {
                    check!(self.fast_presolve_loop());
                }
                if self.problem_size_reduction() > 0.05 {
                    continue;
                }
            }
            self.store_current_problem_size();
            check!(self.enumerate_solutions());
            if self.problem_size_reduction() > 0.05 {
                continue;
            }
            if try_probing {
                check!(self.detect_implied_integers());
                self.store_current_problem_size();
                check!(self.run_probing());
                try_probing = self.probing_contingent > self.num_probed as i64 && (self.problem_size_reduction() > 1.0 || self.probing_early_abort);
                try_sparsify = true;
                if self.problem_size_reduction() > 0.05 || try_probing {
                    continue;
                }
                check!(self.fast_presolve_loop());
            }
            if self.mip.cliquetable.num_cliques() > num_cliques_before_probing.unwrap_or(0) && !domcol_after_probing_called {
                domcol_after_probing_called = true;
                self.store_current_problem_size();
                check!(self.dominated_columns());
                if self.problem_size_reduction() > 0.0 {
                    check!(self.fast_presolve_loop());
                }
                if self.problem_size_reduction() > 0.05 {
                    continue;
                }
            }
            break;
        }
        self.scale_mip()
    }

    /// HiGHS's `HPresolve::run` for a MIP: presolve, compact the model,
    /// hand the global domain its pools and move cuts a restart kept in
    /// the model to the cut pool.
    pub fn run(&mut self) -> PresolveStatus {
        self.shrink_problem_enabled = true;
        match self.presolve() {
            Outcome::Ok | Outcome::Stopped => {}
            Outcome::PrimalInfeasible => return PresolveStatus::Infeasible,
            Outcome::DualInfeasible => return PresolveStatus::UnboundedOrInfeasible,
        }
        self.shrink_problem();

        self.mip.cliquetable.set_presolve_flag(false);
        let num_nonzeros = self.num_nonzeros();
        self.mip.cliquetable.set_max_entries(num_nonzeros);
        {
            let mip = &mut *self.mip;
            mip.domain.add_cutpool(&mip.cutpool);
            mip.domain.add_conflict_pool(&mip.conflictpool, mip.model.num_col);
        }
        if self.mip.num_restarts != 0 {
            let mut numcuts = 0;
            for i in (0..self.mip.model.num_row).rev() {
                if self.ps.orig_row_index(i) < self.orig_num_row {
                    break;
                }
                // The row is a cut: into the cut pool, out of the matrix.
                numcuts += 1;
                self.store_row(i);
                let mut cutinds: Vec<usize> = self.rowpositions[..self.rowpositions_len].iter().map(|&j| self.a_col[j]).collect();
                let mut cutvals: Vec<f64> = self.rowpositions[..self.rowpositions_len].iter().map(|&j| self.a_value[j]).collect();
                let integral = self.rowsize_integer[i] + self.rowsize_impl_int[i] == self.rowsize[i] && self.row_coefficients_integral(i, 1.0);
                let rhs = self.mip.model.row_upper[i];
                let (num_nonzero, num_row) = (self.mip.model.num_nonzero(), self.mip.model.num_row);
                self.mip.cutpool.add_cut(&mut cutinds, &mut cutvals, rhs, integral, true, false, num_nonzero, num_row);
                self.mark_row_deleted(i);
                for k in 0..self.rowpositions_len {
                    let j = self.rowpositions[k];
                    self.unlink(j);
                }
            }
            let m = &mut self.mip.model;
            m.num_row -= numcuts;
            m.row_lower.truncate(m.num_row);
            m.row_upper.truncate(m.num_row);
        }
        self.write_csc();

        if self.mip.model.num_col == 0 {
            if self.mip.model.offset > self.mip.upper_limit {
                return PresolveStatus::Infeasible;
            }
            self.mip.lower_bound = 0.0;
            let feasible = (0..self.mip.model.num_row).all(|i| !(self.mip.model.row_lower[i] > self.primal_feastol || self.mip.model.row_upper[i] < -self.primal_feastol));
            return if feasible { PresolveStatus::ReducedToEmpty } else { PresolveStatus::Infeasible };
        }
        if self.ps.num_reductions() > 0 {
            PresolveStatus::Reduced
        } else {
            PresolveStatus::NotReduced
        }
    }
}

// ---- MIP reductions ------------------------------------------------------

#[derive(Clone, Copy)]
struct StuffingCandidate {
    col: usize,
    val: f64,
    multiplier: f64,
}

/// What `singletonColStuffing`'s candidate search leaves.
#[derive(Default)]
struct StuffingState {
    candidates: Vec<StuffingCandidate>,
    sum_lower: CDouble,
    sum_upper: CDouble,
    sum_lower_finite: bool,
    sum_upper_finite: bool,
    num_integer_candidates: usize,
    min_weight: f64,
    max_weight: f64,
}

/// The counts `dominatedColumns` keeps across columns.
#[derive(Default)]
struct DominationStats {
    num_dom_checks: usize,
    num_dom_checks_pred_bnd_analysis: usize,
    num_fixed_cols: usize,
    num_fixed_cols_pred_bnd_analysis: usize,
    num_modified_bnds_pred_bnd_analysis: usize,
}

impl MipPresolve<'_> {
    fn compute_implied_lower_bound(&mut self, col: usize, bound_col: usize, bound_col_value: f64, bound_col_coeff_pattern: i32) -> f64 {
        self.compute_col_bounds(col, bound_col, bound_col_value, bound_col_coeff_pattern, [true, false, false, false])
            .0
    }

    fn compute_implied_upper_bound(&mut self, col: usize, bound_col: usize, bound_col_value: f64, bound_col_coeff_pattern: i32) -> f64 {
        self.compute_col_bounds(col, bound_col, bound_col_value, bound_col_coeff_pattern, [false, true, false, false])
            .1
    }

    fn compute_worst_case_lower_bound(&mut self, col: usize, bound_col: usize, bound_col_value: f64, bound_col_coeff_pattern: i32) -> f64 {
        self.compute_col_bounds(col, bound_col, bound_col_value, bound_col_coeff_pattern, [false, false, true, false])
            .2
    }

    fn compute_worst_case_upper_bound(&mut self, col: usize, bound_col: usize, bound_col_value: f64, bound_col_coeff_pattern: i32) -> f64 {
        self.compute_col_bounds(col, bound_col, bound_col_value, bound_col_coeff_pattern, [false, false, false, true])
            .3
    }

    fn is_binary(&self, i: usize) -> bool {
        self.mip.model.integrality[i] == VarType::Integer && self.mip.model.col_lower[i] == 0.0 && self.mip.model.col_upper[i] == 1.0
    }

    fn check_domination_non_zero(&self, row: usize, mut aj: f64, mut ak: f64) -> bool {
        if self.is_ranged(row) {
            return (aj - ak).abs() <= self.settings.small_matrix_value;
        }
        if self.mip.model.row_upper[row] == INF {
            aj = -aj;
            ak = -ak;
        }
        aj <= ak + self.settings.small_matrix_value
    }

    fn check_domination(&mut self, signatures: &[(u32, u32)], stats: &mut DominationStats, scalj: f64, j: usize, scalk: f64, k: usize) -> bool {
        stats.num_dom_checks += 1;
        if self.mip.model.integrality[j] == VarType::Integer && self.mip.model.integrality[k] == VarType::Continuous {
            return false;
        }
        let (mut sj_minus, mut sj_plus) = signatures[j];
        if scalj == -1.0 {
            std::mem::swap(&mut sj_plus, &mut sj_minus);
        }
        let (mut sk_minus, mut sk_plus) = signatures[k];
        if scalk == -1.0 {
            std::mem::swap(&mut sk_plus, &mut sk_minus);
        }
        if (!sj_minus & sk_minus) != 0 {
            return false;
        }
        if (sj_plus & !sk_plus) != 0 {
            return false;
        }
        if scalj * self.mip.model.col_cost[j] > scalk * self.mip.model.col_cost[k] + self.settings.small_matrix_value {
            return false;
        }
        let mut pos = self.colhead[j];
        while pos != NONE {
            let row = self.a_row[pos];
            let val = self.a_value[pos];
            let ak_pos = self.find_nonzero(row, k);
            if !self.check_domination_non_zero(row, scalj * val, scalk * if ak_pos == NONE { 0.0 } else { self.a_value[ak_pos] }) {
                return false;
            }
            pos = self.a_next[pos];
        }
        let mut pos = self.colhead[k];
        while pos != NONE {
            let row = self.a_row[pos];
            let val = self.a_value[pos];
            pos = self.a_next[pos];
            if self.find_nonzero(row, j) != NONE {
                continue;
            }
            if !self.check_domination_non_zero(row, 0.0, scalk * val) {
                return false;
            }
        }
        true
    }

    fn dominated_fix_col(&mut self, stats: &mut DominationStats, col: usize, direction: f64) -> Outcome {
        stats.num_fixed_cols += 1;
        if direction > 0.0 {
            check!(self.fix_col_to_upper(col));
        } else {
            check!(self.fix_col_to_lower(col));
        }
        check!(self.remove_row_singletons());
        check!(self.remove_doubleton_equations());
        Outcome::Ok
    }

    /// `dominatedColumns`'s predictive bound analysis (Theorem 3 of Gamrath
    /// et al.).
    #[allow(clippy::too_many_arguments)]
    fn dominated_tighten_bounds(
        &mut self,
        stats: &mut DominationStats,
        col: usize,
        col_bound: f64,
        col_is_at_upper: bool,
        other_col: usize,
        other_col_bound: f64,
        other_col_coeff_pattern: i32,
    ) -> Outcome {
        if self.mip.model.col_lower[col] == self.mip.model.col_upper[col] {
            return Outcome::Ok;
        }
        let integral = |s: &Self| s.mip.model.integrality[col] != VarType::Continuous;
        let mut lower_bound;
        let mut upper_bound;
        if col_is_at_upper {
            upper_bound = self.compute_implied_upper_bound(col, other_col, other_col_bound, other_col_coeff_pattern);
            lower_bound = min(col_bound, self.compute_implied_lower_bound(col, other_col, other_col_bound, other_col_coeff_pattern));
            if self.mip.model.col_cost[col] <= 0.0 {
                let worst_case_upper = self.compute_worst_case_upper_bound(col, other_col, other_col_bound, other_col_coeff_pattern);
                lower_bound = max(lower_bound, min(col_bound, worst_case_upper));
            }
        } else {
            lower_bound = self.compute_implied_lower_bound(col, other_col, other_col_bound, other_col_coeff_pattern);
            upper_bound = max(col_bound, self.compute_implied_upper_bound(col, other_col, other_col_bound, other_col_coeff_pattern));
            if self.mip.model.col_cost[col] >= 0.0 {
                let worst_case_lower = self.compute_worst_case_lower_bound(col, other_col, other_col_bound, other_col_coeff_pattern);
                upper_bound = min(upper_bound, max(col_bound, worst_case_lower));
            }
        }
        // Infinite bounds are no tightening (HiGHS #3285); integer bounds
        // come rounded (#3411).
        if lower_bound < INF && lower_bound > self.mip.model.col_lower[col] + self.primal_feastol {
            if lower_bound == self.mip.model.col_upper[col] {
                stats.num_fixed_cols_pred_bnd_analysis += 1;
                // Done once fixed (HiGHS #3290).
                return self.dominated_fix_col(stats, col, 1.0);
            } else if integral(self) {
                stats.num_modified_bnds_pred_bnd_analysis += 1;
                check!(self.change_col_lower(col, lower_bound));
            }
        }
        if upper_bound > -INF && upper_bound < self.mip.model.col_upper[col] - self.primal_feastol {
            if upper_bound == self.mip.model.col_lower[col] {
                stats.num_fixed_cols_pred_bnd_analysis += 1;
                return self.dominated_fix_col(stats, col, -1.0);
            } else if integral(self) {
                stats.num_modified_bnds_pred_bnd_analysis += 1;
                check!(self.change_col_upper(col, upper_bound));
            }
        }
        Outcome::Ok
    }

    #[allow(clippy::too_many_arguments)]
    fn dominated_check_cols(
        &mut self,
        signatures: &[(u32, u32)],
        stats: &mut DominationStats,
        col: usize,
        k: usize,
        direction: f64,
        multiplier: f64,
        bound_implied: bool,
        has_cliques: bool,
        same_var_type: bool,
    ) -> Outcome {
        let direction_k = multiplier * direction;
        let dominating_bound = if direction > 0.0 {
            self.mip.model.col_upper[col]
        } else {
            self.mip.model.col_lower[col]
        };
        let dominated_bound = if direction_k > 0.0 { self.mip.model.col_lower[k] } else { self.mip.model.col_upper[k] };
        let is_dominating_bound_finite = direction * dominating_bound != INF;
        let is_dominated_bound_finite = direction_k * dominated_bound != -INF;
        let try_to_fix = is_dominated_bound_finite && (bound_implied || has_cliques);
        let try_to_strengthen_bounds = (is_dominating_bound_finite || is_dominated_bound_finite) && same_var_type;
        if try_to_fix || try_to_strengthen_bounds {
            if !try_to_fix {
                stats.num_dom_checks_pred_bnd_analysis += 1;
            }
            if self.check_domination(signatures, stats, direction, col, direction_k, k) {
                let current_bound_implied = if direction > 0.0 { self.is_upper_implied(col) } else { self.is_lower_implied(col) };
                if try_to_fix
                    && (current_bound_implied
                        || self.mip.cliquetable.have_common_clique_counted(
                            CliqueVar::new(col, if direction > 0.0 { 1 } else { 0 }),
                            CliqueVar::new(k, if direction_k > 0.0 { 1 } else { 0 }),
                        ))
                {
                    check!(self.dominated_fix_col(stats, k, -direction_k));
                } else if try_to_strengthen_bounds {
                    let pattern = (direction * direction_k) as i32;
                    if is_dominated_bound_finite {
                        check!(self.dominated_tighten_bounds(stats, col, dominating_bound, direction > 0.0, k, dominated_bound, pattern));
                    }
                    if !self.col_deleted[col] && is_dominating_bound_finite {
                        check!(self.dominated_tighten_bounds(stats, k, dominated_bound, direction_k < 0.0, col, dominating_bound, pattern));
                    }
                }
            }
        }
        Outcome::Ok
    }

    fn vars_have_same_type(&self, col1: usize, col2: usize) -> bool {
        let t = &self.mip.model.integrality;
        (t[col1] != VarType::Continuous && t[col2] != VarType::Continuous) || (t[col1] != VarType::Integer && t[col2] != VarType::Integer)
    }

    #[allow(clippy::too_many_arguments)]
    fn dominated_check_row(
        &mut self,
        signatures: &[(u32, u32)],
        stats: &mut DominationStats,
        row: usize,
        col: usize,
        direction: f64,
        best_val: f64,
        bound_implied: bool,
        has_cliques: bool,
    ) -> Outcome {
        // The row's own copy of its positions (HiGHS #3472): checking the
        // columns can change the row and the stored one.
        if self.row_deleted[row] {
            return Outcome::Ok;
        }
        let mut positions = Vec::new();
        matrix::in_order(self.rowroot[row], &self.ar_left, &self.ar_right, &mut positions);
        let only_pred_bnd_analysis = !bound_implied && !has_cliques;
        for pos in positions {
            if self.a_value[pos] == 0.0 || self.a_row[pos] != row {
                continue;
            }
            let k = self.a_col[pos];
            if k == col || self.col_deleted[k] {
                continue;
            }
            let ak = self.a_value[pos];
            let same_var_type = self.vars_have_same_type(col, k);
            if only_pred_bnd_analysis && !same_var_type {
                continue;
            }
            if self.check_domination_non_zero(row, direction * best_val, direction * ak) {
                check!(self.dominated_check_cols(signatures, stats, col, k, direction, 1.0, bound_implied, has_cliques, same_var_type));
                if self.col_deleted[col] || self.row_deleted[row] {
                    break;
                }
            }
            if !self.col_deleted[k] && self.a_value[pos] != 0.0 && self.a_row[pos] == row && self.check_domination_non_zero(row, direction * best_val, -direction * ak) {
                check!(self.dominated_check_cols(signatures, stats, col, k, direction, -1.0, bound_implied, has_cliques, same_var_type));
                if self.col_deleted[col] || self.row_deleted[row] {
                    break;
                }
            }
        }
        Outcome::Ok
    }

    /// HiGHS's `dominatedColumns`: section 6.4 of Achterberg et al.,
    /// "Presolve Reductions in Mixed Integer Programming", with the
    /// predictive bound analysis of Gamrath et al.
    fn dominated_columns(&mut self) -> Outcome {
        let num_col = self.mip.model.num_col;
        let mut signatures = vec![(0u32, 0u32); num_col];
        let mut stats = DominationStats::default();
        for i in 0..self.a_value.len() {
            if self.a_value[i] == 0.0 {
                continue;
            }
            let row = self.a_row[i];
            let col = self.a_col[i];
            let row_lower_finite = (self.mip.model.row_lower[row] != -INF) as u32;
            let row_upper_finite = (self.mip.model.row_upper[row] != INF) as u32;
            let row_hashed_pos = (hash_i32(row as i32) >> 59) as u32;
            let (first, second) = if self.a_value[i] > 0.0 {
                (row_lower_finite, row_upper_finite)
            } else {
                (row_upper_finite, row_lower_finite)
            };
            signatures[col].0 |= first << row_hashed_pos;
            signatures[col].1 |= second << row_hashed_pos;
        }

        let mut num_cols = 0usize;
        const MAX_AVERAGE_NUM_DOM_CHECKS_PRED_BND_ANALYSIS: usize = 10000;
        const MIN_AVERAGE_NUM_REDS_PRED_BND_ANALYSIS: f64 = 1e-2;
        let mut allow_pred_bnd_analysis = true;

        for j in 0..self.mip.model.num_col {
            if self.col_deleted[j] {
                continue;
            }
            num_cols += 1;
            let mut best_row_plus = NONE;
            let mut best_row_plus_len = usize::MAX;
            let mut aj_best_row_plus = 0.0;
            let mut best_row_minus = NONE;
            let mut best_row_minus_len = usize::MAX;
            let mut aj_best_row_minus = 0.0;
            let mut pos = self.colhead[j];
            while pos != NONE {
                let row = self.a_row[pos];
                let scale = if self.mip.model.row_upper[row] != INF { 1.0 } else { -1.0 };
                let val = scale * self.a_value[pos];
                if val > 0.0 && self.rowsize[row] < best_row_plus_len {
                    best_row_plus = row;
                    best_row_plus_len = self.rowsize[row];
                    aj_best_row_plus = self.a_value[pos];
                }
                if val < 0.0 && self.rowsize[row] < best_row_minus_len {
                    best_row_minus = row;
                    best_row_minus_len = self.rowsize[row];
                    aj_best_row_minus = self.a_value[pos];
                }
                pos = self.a_next[pos];
            }

            let lower_implied = self.is_lower_implied(j);
            let upper_implied = self.is_upper_implied(j);
            let has_neg_cliques = self.is_binary(j) && self.mip.cliquetable.num_cliques_col(j, false) > 0;
            let has_pos_cliques = self.is_binary(j) && self.mip.cliquetable.num_cliques_col(j, true) > 0;

            if best_row_minus != NONE && (allow_pred_bnd_analysis || lower_implied || has_neg_cliques) {
                check!(self.dominated_check_row(&signatures, &mut stats, best_row_minus, j, -1.0, aj_best_row_minus, lower_implied, has_neg_cliques));
            }
            if !self.col_deleted[j] && best_row_plus != NONE && (allow_pred_bnd_analysis || upper_implied || has_pos_cliques) {
                check!(self.dominated_check_row(&signatures, &mut stats, best_row_plus, j, 1.0, aj_best_row_plus, upper_implied, has_pos_cliques));
            }

            let average_num_dom_checks_pred_bnd_analysis = stats.num_dom_checks_pred_bnd_analysis / num_cols;
            let average_num_reds_pred_bnd_analysis = (stats.num_fixed_cols_pred_bnd_analysis + stats.num_modified_bnds_pred_bnd_analysis) as f64 / num_cols as f64;
            allow_pred_bnd_analysis = allow_pred_bnd_analysis
                && (stats.num_dom_checks_pred_bnd_analysis <= 30 * MAX_AVERAGE_NUM_DOM_CHECKS_PRED_BND_ANALYSIS
                    || (average_num_dom_checks_pred_bnd_analysis <= MAX_AVERAGE_NUM_DOM_CHECKS_PRED_BND_ANALYSIS
                        && average_num_reds_pred_bnd_analysis >= MIN_AVERAGE_NUM_REDS_PRED_BND_ANALYSIS));
        }
        Outcome::Ok
    }
}

impl MipPresolve<'_> {
    /// HiGHS's `prepareProbing`: shrink and rebuild the problem, tighten
    /// bounds to their implied values, set up the global domain and the
    /// clique table. Returns whether this is the first call.
    fn prepare_probing(&mut self, first_call: &mut bool) -> Outcome {
        self.shrink_problem();
        self.rebuild_storage();
        let num_nonzeros = self.num_nonzeros();
        self.mip.cliquetable.set_max_entries(num_nonzeros);

        let huge_bound = self.primal_feastol / TINY;
        for i in 0..self.mip.model.num_col {
            if self.impl_col_lower[i].abs() <= huge_bound && self.impl_col_lower[i] > self.mip.model.col_lower[i] {
                check!(self.change_col_lower(i, self.impl_col_lower[i]));
            }
            if self.impl_col_upper[i].abs() <= huge_bound && self.impl_col_upper[i] < self.mip.model.col_upper[i] {
                check!(self.change_col_upper(i, self.impl_col_upper[i]));
            }
        }

        // The solver's postsolve stack is presolve's while it runs; the
        // pseudocosts read it.
        std::mem::swap(&mut self.mip.postsolve, &mut self.ps);
        self.mip.setup_domain_propagation();
        std::mem::swap(&mut self.mip.postsolve, &mut self.ps);
        *first_call = !self.mip.cliques_extracted;

        let ps = &self.ps;
        let orig_num_row = self.orig_num_row;
        let mip = &mut *self.mip;
        mip.parts().on_domain(|dom, env| dom.propagate(env));
        if mip.domain.infeasible() {
            return Outcome::PrimalInfeasible;
        }
        if *first_call {
            mip.cliques_extracted = true;
            mip.parts()
                .on_cliques(|ct, dom, cc| ct.extract_cliques(cc.cx.model, dom, true, &|row| ps.orig_row_index(row), orig_num_row, cc));
            if mip.domain.infeasible() {
                return Outcome::PrimalInfeasible;
            }
            // Presolve keeps the objective limit without the current
            // offset.
            if mip.upper_limit != INF {
                let tmp_limit = mip.upper_limit;
                mip.upper_limit = tmp_limit - mip.model.offset;
                mip.parts().on_cliques(|ct, dom, cc| ct.extract_obj_cliques(cc.cx.model, dom, cc));
                mip.upper_limit = tmp_limit;
                if mip.domain.infeasible() {
                    return Outcome::PrimalInfeasible;
                }
            }
            mip.parts().on_domain(|dom, env| dom.propagate(env));
            if mip.domain.infeasible() {
                return Outcome::PrimalInfeasible;
            }
        }
        mip.parts().on_cliques(|ct, dom, cc| ct.cleanup_fixed(dom, cc));
        if mip.domain.infeasible() {
            return Outcome::PrimalInfeasible;
        }
        Outcome::Ok
    }

    /// HiGHS's `finaliseProbing`: apply what probing found: redundant
    /// clique rows, lifted clique entries, fixings, tightened bounds and
    /// substitutions.
    fn finalise_probing(
        &mut self,
        first_call: bool,
        num_vars_fixed: &mut usize,
        num_bnds_tightened: &mut usize,
        num_vars_substituted: &mut usize,
        lifted_non_zeros: &mut i64,
    ) -> Outcome {
        {
            let ps = &self.ps;
            let orig_num_row = self.orig_num_row;
            let mip = &mut *self.mip;
            mip.parts().on_cliques(|ct, dom, cc| ct.cleanup_fixed(dom, cc));
            if !first_call {
                mip.parts()
                    .on_cliques(|ct, dom, cc| ct.extract_cliques(cc.cx.model, dom, false, &|row| ps.orig_row_index(row), orig_num_row, cc));
            }
            mip.parts().on_cliques(|ct, dom, cc| ct.run_clique_merging(dom, cc));
        }

        let deleted_rows = std::mem::take(self.mip.cliquetable.deleted_rows());
        for delrow in deleted_rows {
            let delrow = delrow as usize;
            if !self.row_deleted[delrow] {
                self.remove_row(delrow);
            }
        }

        let extensionvars = std::mem::take(self.mip.cliquetable.clique_extensions());
        *lifted_non_zeros += extensionvars.len() as i64;
        for (row, var) in extensionvars {
            let row = row as usize;
            if self.row_deleted[row] {
                *lifted_non_zeros -= 1;
                continue;
            }
            let mut val = 1.0;
            if var.val() == 0 {
                self.mip.model.row_lower[row] -= 1.0;
                self.mip.model.row_upper[row] -= 1.0;
                val = -1.0;
            }
            self.add_to_matrix(row, var.col32() as usize, val);
        }

        for i in 0..self.mip.model.num_col {
            if self.col_deleted[i] {
                continue;
            }
            let new_lower_bnd = self.mip.model.col_lower[i] < self.mip.domain.col_lower[i];
            let new_upper_bnd = self.mip.model.col_upper[i] > self.mip.domain.col_upper[i];
            if new_lower_bnd {
                check!(self.change_col_lower(i, self.mip.domain.col_lower[i]));
            }
            if new_upper_bnd {
                check!(self.change_col_upper(i, self.mip.domain.col_upper[i]));
            }
            if self.mip.domain.is_fixed(i) {
                *num_vars_fixed += 1;
                self.ps.fixed_col(i, self.mip.model.col_lower[i]);
                self.remove_fixed_col(i);
            } else {
                if new_lower_bnd {
                    *num_bnds_tightened += 1;
                }
                if new_upper_bnd {
                    *num_bnds_tightened += 1;
                }
            }
            // No limit check here: the clique table has deleted rows
            // without postsolve entries, relying on every fixing being
            // applied (HiGHS #3220).
        }

        check!(self.apply_conflict_graph_substitutions(num_vars_substituted));
        self.check_limits()
    }

    /// HiGHS's `computeProbingScore`.
    fn compute_probing_score(&self, col: usize) -> (i64, i64) {
        let implics_up = self.mip.cliquetable.num_implications_val(col, true) as i64;
        let implics_down = self.mip.cliquetable.num_implications_val(col, false) as i64;
        (
            (5000i64).min(implics_up * implics_down) / (1 + self.num_probes[col] as i64),
            (100i64).min(implics_up + implics_down),
        )
    }

    /// HiGHS's `runProbing`: probe the binaries with the most implications
    /// first, within budgets on time, deletions and clique table work.
    fn run_probing(&mut self) -> Outcome {
        self.probing_early_abort = false;
        let old_num_probed = self.num_probed;
        let mut first_call = false;
        check!(self.prepare_probing(&mut first_call));

        let mut binaries: Vec<(i64, i64, usize, usize)> = Vec::new();
        if !self.mip.cliquetable.is_full() {
            binaries.reserve(self.mip.model.num_col);
            let mut random = Random::new(self.settings.random_seed);
            for i in 0..self.mip.model.num_col {
                if self.mip.domain.is_binary(&self.mip.model, i) {
                    let probing_score = self.compute_probing_score(i);
                    binaries.push((-probing_score.0, -probing_score.1, random.integer_any(), i));
                }
            }
        }
        if binaries.is_empty() {
            return self.check_limits();
        }
        // The entries are distinct (each has its column), so any sort
        // orders them as HiGHS's pdqsort does.
        binaries.sort_unstable();

        let mut num_changed_cols = 0;
        while self.mip.domain.changed_cols().len() != num_changed_cols {
            let col = self.mip.domain.changed_cols()[num_changed_cols];
            num_changed_cols += 1;
            if self.mip.domain.is_fixed(col) {
                self.probing_num_del_col += 1;
            }
        }

        let num_cliques_start = self.mip.cliquetable.num_cliques() as i64;
        let num_implics_start = self.mip.implications.num_implications();
        let num_del_start = self.probing_num_del_col as i64;
        let calc_num_del = |s: &Self| s.probing_num_del_col as i64 - num_del_start + (s.mip.implications.substitutions.len() + s.mip.cliquetable.substitutions().len()) as i64;
        let mut num_del = calc_num_del(self);

        let num_nonzeros = self.num_nonzeros() as i64;
        let mut splay_contingent = self.mip.cliquetable.num_neighbourhood_queries + (if self.mip.submip { 0 } else { 100000 }).max(10 * num_nonzeros);
        let mut num_fail: i64 = 0;
        // Lifting opportunities are only collected with
        // `mip_lifting_for_probing` on.

        for &(_, _, _, i) in &binaries {
            if self.mip.cliquetable.substitution(i).is_some() || !self.mip.domain.is_binary(&self.mip.model, i) {
                continue;
            }
            if self.mip.elapsed() > self.settings.time_limit {
                return Outcome::Stopped;
            }
            let tighten_limits = self.num_probed - old_num_probed >= 2500;
            let size_limit = ((self.mip.model.num_row + self.mip.model.num_col) / 20) as i64;
            self.probing_early_abort = if !tighten_limits {
                num_del > (1000i64).max(size_limit)
            } else {
                num_del > (1000i64).min(size_limit)
            };
            if self.probing_early_abort {
                break;
            }
            let num_nonzeros = self.num_nonzeros() as i64;
            if self.mip.cliquetable.is_full()
                || self.mip.cliquetable.num_cliques() as i64 - num_cliques_start > (1000000i64).max(2 * num_nonzeros)
                || self.mip.implications.num_implications() - num_implics_start > (1000000i64).max(2 * num_nonzeros)
            {
                break;
            }
            if self.mip.cliquetable.num_neighbourhood_queries > splay_contingent {
                break;
            }
            if self.probing_contingent - (self.num_probed as i64) < 0 {
                break;
            }

            let mut num_bound_chgs = 0usize;
            let mut num_new_cliques = -(self.mip.cliquetable.num_cliques() as i64);
            let probing_result = {
                let mut parts = self.mip.parts();
                parts
                    .implications
                    .run_probing(i, &mut num_bound_chgs, parts.domain, parts.cliquetable, &mut parts.cx, parts.pseudocost)
            };
            if !probing_result {
                continue;
            }
            self.probing_contingent += num_bound_chgs as i64;
            num_new_cliques += self.mip.cliquetable.num_cliques() as i64;
            num_new_cliques = num_new_cliques.max(0);
            while self.mip.domain.changed_cols().len() != num_changed_cols {
                let col = self.mip.domain.changed_cols()[num_changed_cols];
                num_changed_cols += 1;
                if self.mip.domain.is_fixed(col) {
                    self.probing_num_del_col += 1;
                }
            }
            let new_num_del = calc_num_del(self);
            if new_num_del > num_del {
                self.probing_contingent += num_del;
                if !self.mip.submip {
                    splay_contingent += 100 * (new_num_del + num_del_start);
                    splay_contingent += 1000 * num_new_cliques;
                }
                num_del = new_num_del;
                num_fail = 0;
            } else if self.mip.submip || num_new_cliques == 0 {
                splay_contingent -= (if tighten_limits { 250 } else { 100 }) * num_fail;
                num_fail += 1;
            } else {
                splay_contingent += 1000 * num_new_cliques;
                num_fail = 0;
            }
            self.num_probed += 1;
            self.num_probes[i] = self.num_probes[i].wrapping_add(1);
            if self.mip.domain.infeasible() {
                return Outcome::PrimalInfeasible;
            }
        }

        let mut num_vars_fixed = 0;
        let mut num_bnds_tightened = 0;
        let mut num_vars_substituted = 0;
        let mut lifted_nonzeros = 0;
        check!(self.finalise_probing(first_call, &mut num_vars_fixed, &mut num_bnds_tightened, &mut num_vars_substituted, &mut lifted_nonzeros));
        self.probing_num_del_col += num_vars_substituted;
        self.check_limits()
    }

    /// HiGHS's `applyConflictGraphSubstitutions`: the substitutions the
    /// implications and the clique table found.
    fn apply_conflict_graph_substitutions(&mut self, num_del_col: &mut usize) -> Outcome {
        for k in 0..self.mip.implications.substitutions.len() {
            let substitution = self.mip.implications.substitutions[k];
            if self.col_deleted[substitution.substcol] || self.col_deleted[substitution.staycol] {
                continue;
            }
            *num_del_col += 1;
            self.ps
                .doubleton_equation(substitution.substcol, substitution.staycol, 1.0, -substitution.scale, substitution.offset);
            self.mark_col_deleted(substitution.substcol);
            self.substitute_cols(substitution.substcol, substitution.staycol, substitution.offset, substitution.scale);
            check!(self.check_limits());
        }
        self.mip.implications.substitutions.clear();

        for k in 0..self.mip.cliquetable.substitutions().len() {
            let subst = self.mip.cliquetable.substitutions()[k];
            let replace_col = subst.replace.col32() as usize;
            if self.col_deleted[subst.substcol] || self.col_deleted[replace_col] {
                continue;
            }
            let (scale, offset) = if subst.replace.val() == 0 { (-1.0, 1.0) } else { (1.0, 0.0) };
            *num_del_col += 1;
            self.ps.doubleton_equation(subst.substcol, replace_col, 1.0, -scale, offset);
            self.mark_col_deleted(subst.substcol);
            self.substitute_cols(subst.substcol, replace_col, offset, scale);
            check!(self.check_limits());
        }
        self.mip.cliquetable.clear_substitutions();
        Outcome::Ok
    }
}

impl MipPresolve<'_> {
    /// HiGHS's `transformColumn`: replace `x` by `scale * x' + constant`.
    fn transform_column(&mut self, col: usize, scale: f64, constant: f64) -> Outcome {
        self.mip.implications.column_transformed(col, scale, constant);
        self.ps.linear_transform(col, scale, constant);
        if constant != 0.0 {
            let old_lower = self.mip.model.col_lower[col];
            let old_upper = self.mip.model.col_upper[col];
            self.mip.model.col_upper[col] -= constant;
            self.mip.model.col_lower[col] -= constant;
            let mut pos = self.colhead[col];
            while pos != NONE {
                let row = self.a_row[pos];
                let val = self.a_value[pos];
                self.implied_row_bounds.updated_var_lower(col_bounds!(self), row, col, val, old_lower);
                self.implied_row_bounds.updated_var_upper(col_bounds!(self), row, col, val, old_upper);
                pos = self.a_next[pos];
            }
            let old_impl_lower = self.impl_col_lower[col];
            let old_impl_upper = self.impl_col_upper[col];
            self.impl_col_lower[col] -= constant;
            self.impl_col_upper[col] -= constant;
            let mut pos = self.colhead[col];
            while pos != NONE {
                let row = self.a_row[pos];
                let val = self.a_value[pos];
                let (lower_source, upper_source) = (self.col_lower_source[col], self.col_upper_source[col]);
                self.implied_row_bounds
                    .updated_impl_var_lower(col_bounds!(self), row, col, val, old_impl_lower, lower_source);
                self.implied_row_bounds
                    .updated_impl_var_upper(col_bounds!(self), row, col, val, old_impl_upper, upper_source);
                pos = self.a_next[pos];
            }
        }
        self.implied_dual_row_bounds.sum_scaled(col, scale);
        let bound_scale = 1.0 / scale;
        self.mip.model.col_lower[col] *= bound_scale;
        self.mip.model.col_upper[col] *= bound_scale;
        self.impl_col_lower[col] *= bound_scale;
        self.impl_col_upper[col] *= bound_scale;
        if scale < 0.0 {
            let m = &mut self.mip.model;
            std::mem::swap(&mut m.col_lower[col], &mut m.col_upper[col]);
            std::mem::swap(&mut self.impl_col_lower[col], &mut self.impl_col_upper[col]);
            std::mem::swap(&mut self.col_lower_source[col], &mut self.col_upper_source[col]);
        }
        self.mip.model.offset += self.mip.model.col_cost[col] * constant;
        self.mip.model.col_cost[col] *= scale;
        let mut coliter = self.colhead[col];
        while coliter != NONE {
            let val = self.a_value[coliter];
            self.a_value[coliter] *= scale;
            let row = self.a_row[coliter];
            let row_constant = val * constant;
            if self.mip.model.row_lower[row] != -INF {
                self.mip.model.row_lower[row] -= row_constant;
            }
            if self.mip.model.row_upper[row] != INF {
                self.mip.model.row_upper[row] -= row_constant;
            }
            coliter = self.a_next[coliter];
        }
        if self.mip.model.integrality[col] != VarType::Continuous {
            let (lower, upper) = (self.mip.model.col_lower[col], self.mip.model.col_upper[col]);
            check!(self.change_col_bounds(col, lower, upper));
        }
        self.mark_changed_col(col);
        Outcome::Ok
    }

    /// HiGHS's `scaleStoredRow`: scale the stored row, rounding its sides
    /// if it is `integral`.
    fn scale_stored_row(&mut self, row: usize, scale: f64, integral: bool) {
        self.mip.model.row_upper[row] *= scale;
        self.mip.model.row_lower[row] *= scale;
        self.impl_row_dual_lower[row] /= scale;
        self.impl_row_dual_upper[row] /= scale;
        if integral {
            if self.mip.model.row_upper[row] != INF {
                self.mip.model.row_upper[row] = self.mip.model.row_upper[row].round();
            }
            if self.mip.model.row_lower[row] != -INF {
                self.mip.model.row_lower[row] = self.mip.model.row_lower[row].round();
            }
        }
        let mut j = 0;
        while j < self.rowpositions_len {
            let pos = self.rowpositions[j];
            self.a_value[pos] *= scale;
            if self.a_value[pos].abs() <= self.settings.small_matrix_value {
                self.unlink(pos);
            }
            j += 1;
        }
        self.implied_row_bounds.sum_scaled(row, scale);
        if scale < 0.0 {
            std::mem::swap(&mut self.row_dual_lower[row], &mut self.row_dual_upper[row]);
            std::mem::swap(&mut self.impl_row_dual_lower[row], &mut self.impl_row_dual_upper[row]);
            std::mem::swap(&mut self.row_dual_lower_source[row], &mut self.row_dual_upper_source[row]);
            let m = &mut self.mip.model;
            std::mem::swap(&mut m.row_lower[row], &mut m.row_upper[row]);
        }
    }

    /// HiGHS's `scaleMIP`: scale rows with continuous columns and the
    /// continuous columns by powers of two.
    fn scale_mip(&mut self) -> Outcome {
        for i in 0..self.mip.model.num_row {
            if self.row_deleted[i] || self.rowsize[i] < 1 || self.rowsize_integer[i] + self.rowsize_impl_int[i] == self.rowsize[i] {
                continue;
            }
            self.store_row(i);
            let mut max_abs_val = 0.0;
            for j in 0..self.rowpositions_len {
                let nz_pos = self.rowpositions[j];
                if self.mip.model.integrality[self.a_col[nz_pos]] != VarType::Continuous {
                    continue;
                }
                max_abs_val = max(self.a_value[nz_pos].abs(), max_abs_val);
            }
            let mut scale = (-f64::log2(max_abs_val)).round().exp2();
            if scale == 1.0 {
                continue;
            }
            if self.mip.model.row_upper[i] == INF {
                scale = -scale;
            }
            self.scale_stored_row(i, scale, false);
        }
        for i in 0..self.mip.model.num_col {
            if self.col_deleted[i] || self.colsize[i] < 1 || self.mip.model.integrality[i] != VarType::Continuous {
                continue;
            }
            let max_abs_val = self.get_max_abs_col_val(i);
            let scale = (-f64::log2(max_abs_val)).round().exp2();
            if scale == 1.0 {
                continue;
            }
            check!(self.transform_column(i, scale, 0.0));
        }
        Outcome::Ok
    }
}

/// `enumerateSolutions`' candidate rows: score, second score, a random
/// tie-break, the row and its column signature.
type CandidateRow = (f64, f64, usize, usize, u32);

impl MipPresolve<'_> {
    /// `enumerateSolutions`' `getBinaryRow`: the row's unfixed columns,
    /// sorted, if they are all binary and at most `MAX_ROW_SIZE`.
    fn enumeration_binary_row(&self, row: usize, binvars: &mut [usize], numnzs: &mut usize) -> bool {
        const MAX_ROW_SIZE: usize = 8;
        *numnzs = 0;
        let model = &self.mip.model;
        let domain = &self.mip.domain;
        for j in model.ar_start[row]..model.ar_start[row + 1] {
            let col = model.ar_index[j];
            if domain.is_fixed(col) {
                continue;
            }
            if !domain.is_binary(model, col) || *numnzs >= MAX_ROW_SIZE {
                return false;
            }
            binvars[*numnzs] = col;
            *numnzs += 1;
        }
        if *numnzs == 0 {
            return false;
        }
        binvars[..*numnzs].sort_unstable();
        true
    }

    /// HiGHS's `enumerateSolutions`: enumerate the solutions of small pure
    /// binary rows, for fixings, bounds, cliques and substitutions they
    /// all agree on.
    fn enumerate_solutions(&mut self) -> Outcome {
        let mut first_call = false;
        check!(self.prepare_probing(&mut first_call));

        const MAX_ROW_SIZE: usize = 8;
        const MAX_NUM_ROWS_CHECKED: usize = 400;
        const MAX_NUM_SOLUTIONS: usize = 1 << MAX_ROW_SIZE;
        const MAX_PERCENTAGE_ROW_OVERLAP: usize = 50;
        const MAX_NUM_FAILS: i32 = 6;

        // Compile the candidate rows.
        let mut rows: Vec<CandidateRow> = Vec::with_capacity(self.mip.model.num_row);
        {
            let mut binvars = vec![0usize; MAX_ROW_SIZE];
            let mut numnzs = 0;
            let mut random = Random::new(self.settings.random_seed);
            for i in 0..self.mip.model.num_row {
                if self.mip.domain.is_redundant_row(i, &self.mip.model, self.mip.feastol) {
                    continue;
                }
                if !self.enumeration_binary_row(i, &mut binvars, &mut numnzs) {
                    continue;
                }
                let mut score: i64 = 0;
                let mut score2: i64 = 0;
                for &var in &binvars[..numnzs] {
                    let probing_score = self.compute_probing_score(var);
                    score += probing_score.0;
                    score2 += probing_score.1;
                }
                let mut signature: u32 = 0;
                for &var in &binvars[..numnzs] {
                    signature |= 1 << (hash_i32(var as i32) >> 59);
                }
                rows.push((-(score as f64 / numnzs as f64), -(score2 as f64 / numnzs as f64), random.integer_any(), i, signature));
            }
        }
        // The rows are distinct, so any sort orders them as pdqsort does.
        rows.sort_unstable_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap()
                .then(a.1.partial_cmp(&b.1).unwrap())
                .then(a.2.cmp(&b.2))
                .then(a.3.cmp(&b.3))
                .then(a.4.cmp(&b.4))
        });

        // Remove rows that overlap much with a better one.
        if rows.len() > 1 {
            let mut binvars = vec![0usize; MAX_ROW_SIZE];
            let mut binvars2 = vec![0usize; MAX_ROW_SIZE];
            let mut numnzs = 0;
            let mut numnzs2 = 0;
            let mut num_rows_accepted = 0;
            let mut num_rows_removed: i64 = 0;
            for i in 0..rows.len() - 1 {
                let r = rows[i].3;
                if r == NONE {
                    continue;
                }
                num_rows_accepted += 1;
                if num_rows_accepted >= MAX_NUM_ROWS_CHECKED {
                    break;
                }
                self.enumeration_binary_row(r, &mut binvars, &mut numnzs);
                let mut num_rows_active: i64 = 0;
                let old_num_rows_removed = num_rows_removed;
                for ii in i + 1..rows.len() {
                    let r2 = rows[ii].3;
                    if r2 == NONE {
                        continue;
                    }
                    num_rows_active += 1;
                    if (rows[i].4 & rows[ii].4) == 0 {
                        continue;
                    }
                    self.enumeration_binary_row(r2, &mut binvars2, &mut numnzs2);
                    let (mut ir, mut ir2, mut overlap) = (0, 0, 0usize);
                    while ir < numnzs && ir2 < numnzs2 {
                        if binvars[ir] < binvars2[ir2] {
                            ir += 1;
                        } else if binvars[ir] > binvars2[ir2] {
                            ir2 += 1;
                        } else {
                            ir += 1;
                            ir2 += 1;
                            overlap += 1;
                        }
                    }
                    if (100 * overlap) / numnzs.min(numnzs2) > MAX_PERCENTAGE_ROW_OVERLAP {
                        num_rows_removed += 1;
                        rows[ii].3 = NONE;
                    }
                }
                if num_rows_active - num_rows_removed + old_num_rows_removed <= 1 {
                    break;
                }
            }
            if num_rows_removed > 0 {
                rows.retain(|p| p.3 != NONE);
            }
            rows.truncate(MAX_NUM_ROWS_CHECKED);
        }

        let num_col = self.mip.model.num_col;
        let mut solutions = vec![[0i32; MAX_NUM_SOLUTIONS]; MAX_ROW_SIZE];
        let mut vars = vec![0usize; MAX_ROW_SIZE];
        // (domain changes, changed columns) when each branch was made.
        let mut branches = [(0usize, 0usize); MAX_ROW_SIZE];
        let mut worst_case_bounds = vec![0usize; num_col];
        let mut worst_case_lower_bound = vec![INF; num_col];
        let mut worst_case_upper_bound = vec![-INF; num_col];
        let mut col_lower = self.mip.domain.col_lower.clone();
        let mut col_upper = self.mip.domain.col_upper.clone();

        let find_branch_var = |domain: &Domain, vars: &[usize], num_vars: usize| vars[..num_vars].iter().copied().find(|&v| !domain.is_fixed(v));

        let mut num_fails = 0;
        for r in &rows {
            let row = r.3;
            if self.mip.domain.is_redundant_row(row, &self.mip.model, self.mip.feastol) {
                continue;
            }
            let mut num_vars = 0;
            if !self.enumeration_binary_row(row, &mut vars, &mut num_vars) {
                continue;
            }

            let mut num_branches: isize = -1;
            let mut num_worst_case_bounds = 0usize;
            let mut num_solutions = 0usize;
            let mut min_num_active_cols = num_vars;
            let mut max_num_active_cols = 0usize;
            loop {
                let mut backtrack = self.mip.domain.infeasible();
                if !backtrack {
                    backtrack = find_branch_var(&self.mip.domain, &vars, num_vars).is_none();
                    if backtrack {
                        // HiGHS's `handleSolution`.
                        self.mip.parts().on_domain(|dom, env| dom.propagate(env));
                        let domain = &self.mip.domain;
                        if !domain.infeasible() {
                            let update_worst_case_bounds = |col: usize, lower: &mut [f64], upper: &mut [f64]| {
                                lower[col] = min(lower[col], domain.col_lower[col]);
                                upper[col] = max(upper[col], domain.col_upper[col]);
                                lower[col] <= col_lower[col] && upper[col] >= col_upper[col]
                            };
                            let remove_worst_case_bounds = |pos: usize, n: &mut usize, bounds: &mut [usize], lower: &mut [f64], upper: &mut [f64]| {
                                lower[bounds[pos]] = INF;
                                upper[bounds[pos]] = -INF;
                                bounds[pos] = bounds[*n - 1];
                                bounds[*n - 1] = 0;
                                *n -= 1;
                            };
                            if num_solutions == 0 {
                                for &col in domain.changed_cols() {
                                    worst_case_bounds[num_worst_case_bounds] = col;
                                    num_worst_case_bounds += 1;
                                    update_worst_case_bounds(col, &mut worst_case_lower_bound, &mut worst_case_upper_bound);
                                }
                            } else {
                                let mut i = 0;
                                while i < num_worst_case_bounds {
                                    let col = worst_case_bounds[i];
                                    if !domain.is_changed_col(col) || update_worst_case_bounds(col, &mut worst_case_lower_bound, &mut worst_case_upper_bound) {
                                        remove_worst_case_bounds(
                                            i,
                                            &mut num_worst_case_bounds,
                                            &mut worst_case_bounds,
                                            &mut worst_case_lower_bound,
                                            &mut worst_case_upper_bound,
                                        );
                                    } else {
                                        i += 1;
                                    }
                                }
                            }
                            let mut num_active_cols = 0;
                            for i in 0..num_vars {
                                let sol_value = if domain.col_lower[vars[i]] == 0.0 { 0 } else { 1 };
                                solutions[i][num_solutions] = sol_value;
                                if sol_value != 0 {
                                    num_active_cols += 1;
                                }
                            }
                            min_num_active_cols = min_num_active_cols.min(num_active_cols);
                            max_num_active_cols = max_num_active_cols.max(num_active_cols);
                            num_solutions += 1;
                        }
                    }
                }
                if !backtrack {
                    // HiGHS's `doBranch`: the down branch.
                    let branchvar = find_branch_var(&self.mip.domain, &vars, num_vars).unwrap();
                    num_branches += 1;
                    branches[num_branches as usize] = (self.mip.domain.domain_change_stack().len(), self.mip.domain.changed_cols().len());
                    self.mip
                        .parts()
                        .on_domain(|dom, env| dom.change_bound(DomainChange::new(0.0, branchvar, BoundType::Upper), Reason::branching(), env));
                } else {
                    // HiGHS's `doBacktrack`.
                    while num_branches >= 0 {
                        let (num_domain_changes, num_changed_cols) = branches[num_branches as usize];
                        let domchg = self.mip.domain.domain_change_stack()[num_domain_changes];
                        self.mip.parts().on_domain(|dom, env| {
                            dom.backtrack(env);
                        });
                        self.mip.domain.clear_changed_cols_from(num_changed_cols);
                        if domchg.boundtype == BoundType::Upper {
                            self.mip
                                .parts()
                                .on_domain(|dom, env| dom.change_bound(DomainChange::new(1.0, domchg.column, BoundType::Lower), Reason::branching(), env));
                            break;
                        }
                        branches[num_branches as usize] = (0, 0);
                        num_branches -= 1;
                    }
                    if num_branches < 0 {
                        break;
                    }
                }
            }

            if num_solutions == 0 {
                return Outcome::PrimalInfeasible;
            }

            let old_num_changed_cols = self.mip.domain.changed_cols().len();
            let old_num_cliques = self.mip.cliquetable.num_cliques();
            let old_num_substitutions = self.mip.cliquetable.substitutions().len();

            if max_num_active_cols == 1 || min_num_active_cols == num_vars - 1 {
                let mut clique: Vec<CliqueVar> = vars[..num_vars].iter().map(|&v| CliqueVar::new(v, if max_num_active_cols == 1 { 1 } else { 0 })).collect();
                let equality = min_num_active_cols == max_num_active_cols;
                self.mip
                    .parts()
                    .on_cliques(|ct, dom, cc| ct.add_clique(cc.cx.model, dom, &mut clique, equality, NO_ORIGIN, cc));
                if self.mip.domain.infeasible() {
                    return Outcome::PrimalInfeasible;
                }
            }

            for worst_case_bound in &mut worst_case_bounds[..num_worst_case_bounds] {
                let col = *worst_case_bound;
                if worst_case_lower_bound[col] > self.mip.domain.col_lower[col] {
                    let bound = worst_case_lower_bound[col];
                    self.mip
                        .parts()
                        .on_domain(|dom, env| dom.change_bound(DomainChange::new(bound, col, BoundType::Lower), Reason::unspecified(), env));
                    if self.mip.domain.infeasible() {
                        return Outcome::PrimalInfeasible;
                    }
                }
                if worst_case_upper_bound[col] < self.mip.domain.col_upper[col] {
                    let bound = worst_case_upper_bound[col];
                    self.mip
                        .parts()
                        .on_domain(|dom, env| dom.change_bound(DomainChange::new(bound, col, BoundType::Upper), Reason::unspecified(), env));
                    if self.mip.domain.infeasible() {
                        return Outcome::PrimalInfeasible;
                    }
                }
                worst_case_lower_bound[col] = INF;
                worst_case_upper_bound[col] = -INF;
                *worst_case_bound = 0;
            }

            for i in 0..num_vars - 1 {
                let col = vars[i];
                for ii in i + 1..num_vars {
                    let col2 = vars[ii];
                    let identical = (0..num_solutions).all(|sol| solutions[i][sol] == solutions[ii][sol]);
                    let complementary = !identical && (0..num_solutions).all(|sol| solutions[i][sol] == 1 - solutions[ii][sol]);
                    if identical || complementary {
                        let mut clique = [CliqueVar::new(col, 0), CliqueVar::new(col2, if identical { 1 } else { 0 })];
                        self.mip.parts().on_cliques(|ct, dom, cc| ct.add_clique(cc.cx.model, dom, &mut clique, true, NO_ORIGIN, cc));
                        if self.mip.domain.infeasible() {
                            return Outcome::PrimalInfeasible;
                        }
                    }
                }
            }

            let num_changed_cols = self.mip.domain.changed_cols().len();
            for &col in self.mip.domain.changed_cols() {
                col_lower[col] = self.mip.domain.col_lower[col];
                col_upper[col] = self.mip.domain.col_upper[col];
            }
            self.mip.domain.clear_changed_cols();

            if num_changed_cols != old_num_changed_cols
                || self.mip.cliquetable.num_cliques() != old_num_cliques
                || self.mip.cliquetable.substitutions().len() != old_num_substitutions
            {
                num_fails = 0;
            } else {
                num_fails += 1;
                if num_fails > MAX_NUM_FAILS {
                    break;
                }
            }
        }

        let mut num_vars_fixed = 0;
        let mut num_bnds_tightened = 0;
        let mut num_vars_substituted = 0;
        let mut lifted_nonzeros = 0;
        check!(self.finalise_probing(first_call, &mut num_vars_fixed, &mut num_bnds_tightened, &mut num_vars_substituted, &mut lifted_nonzeros));
        self.check_limits()
    }
}

impl MipPresolve<'_> {
    /// HiGHS's `detectImpliedIntegers`.
    fn detect_implied_integers(&mut self) -> Outcome {
        for col in 0..self.mip.model.num_col {
            check!(self.convert_implied_integer(col, NONE, false).result);
        }
        Outcome::Ok
    }

    /// HiGHS's `strengthenInequalities`: tighten the integer coefficients
    /// of inequalities by covers of their maximal violation.
    fn strengthen_inequalities(&mut self, num_strengthened: &mut usize) -> Outcome {
        let mut complementation: Vec<i8> = Vec::new();
        // In double-double throughout, as HiGHS since #3234.
        let mut reducedcost: Vec<CDouble> = Vec::new();
        let mut upper: Vec<CDouble> = Vec::new();
        let cmin = |a: CDouble, b: CDouble| if a < b { a } else { b };
        let cmax = |a: CDouble, b: CDouble| if a > b { a } else { b };
        let mut indices: Vec<usize> = Vec::new();
        let mut positions: Vec<usize> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut coefs: Vec<f64> = Vec::new();
        let mut cover: Vec<usize> = Vec::new();
        let feastol = self.primal_feastol;
        *num_strengthened = 0;
        const CHECK_TIME_FREQUENCY: usize = 100;

        for row in 0..self.mip.model.num_row {
            if self.rowsize[row] <= 1 || self.is_ranged(row) {
                continue;
            }
            let rowsize_limit = 1000usize.max((self.mip.model.num_col - self.num_deleted_cols) / 20);
            if self.rowsize[row] > rowsize_limit {
                continue;
            }
            let mut maxviolation;
            let mut continuouscontribution = CDouble::new(0.0);
            // The most that negligible terms, left out of covers, contribute
            // (HiGHS #3468): a term is negligible if that is at most feastol.
            let mut smallcontribution = CDouble::new(0.0);
            let scale;
            if self.mip.model.row_lower[row] != -INF {
                maxviolation = CDouble::new(self.mip.model.row_lower[row]);
                scale = -1.0;
            } else {
                maxviolation = CDouble::new(-self.mip.model.row_upper[row]);
                scale = 1.0;
            }
            complementation.clear();
            reducedcost.clear();
            upper.clear();
            indices.clear();
            positions.clear();
            stack.push(self.rowroot[row]);
            let mut skiprow = false;
            while let Some(pos) = stack.pop() {
                if self.ar_right[pos] != NONE {
                    stack.push(self.ar_right[pos]);
                }
                if self.ar_left[pos] != NONE {
                    stack.push(self.ar_left[pos]);
                }
                let col = self.a_col[pos];
                skiprow = self.mip.model.col_lower[col] == -INF || self.mip.model.col_upper[col] == INF;
                if skiprow {
                    break;
                }
                let comp: i8;
                let mut weight = self.a_value[pos] * scale;
                let ub = CDouble::new(self.mip.model.col_upper[col]) - CDouble::new(self.mip.model.col_lower[col]);
                if weight > 0.0 {
                    comp = 1;
                    maxviolation += CDouble::new(self.mip.model.col_upper[col]) * weight;
                } else {
                    comp = -1;
                    maxviolation += CDouble::new(self.mip.model.col_lower[col]) * weight;
                    weight = -weight;
                }
                if self.mip.model.integrality[col] == VarType::Continuous {
                    continuouscontribution += weight * ub;
                    continue;
                }
                if ub * weight <= feastol {
                    smallcontribution += ub * weight;
                    continue;
                }
                indices.push(reducedcost.len());
                positions.push(pos);
                reducedcost.push(CDouble::new(weight));
                complementation.push(comp);
                upper.push(ub);
            }
            if (row & CHECK_TIME_FREQUENCY) == 0 || 10 * self.rowsize[row] > rowsize_limit {
                check!(self.check_time_limit());
            }
            if skiprow {
                stack.clear();
                continue;
            }
            if maxviolation <= feastol {
                check!(self.row_presolve(row));
                continue;
            }
            let small_val = max(100.0 * feastol, feastol * maxviolation.value());
            loop {
                if maxviolation - continuouscontribution - smallcontribution <= small_val || indices.is_empty() {
                    break;
                }
                indices.sort_unstable_by(|&i1, &i2| reducedcost[i2].partial_cmp(&reducedcost[i1]).unwrap().then(i2.cmp(&i1)));
                let mut lambda = maxviolation - continuouscontribution - smallcontribution;
                cover.clear();
                for i in (1..=indices.len()).rev() {
                    let index = indices[i - 1];
                    let delta = upper[index] * reducedcost[index];
                    if upper[index] <= 1000.0 && reducedcost[index] > small_val && lambda - delta <= small_val {
                        cover.push(index);
                    } else {
                        lambda -= delta;
                    }
                }
                if cover.is_empty() || lambda <= small_val {
                    break;
                }
                // `std::min_element` with HiGHS's comparison.
                let less = |i1: usize, i2: usize| {
                    if reducedcost[i1] <= 1e-3 || reducedcost[i2] <= 1e-3 {
                        reducedcost[i1] > reducedcost[i2]
                    } else {
                        reducedcost[i1] < reducedcost[i2]
                    }
                };
                let mut alpos = cover[0];
                for &c in &cover[1..] {
                    if less(c, alpos) {
                        alpos = c;
                    }
                }
                let al = reducedcost[alpos];
                coefs.resize(cover.len(), 0.0);
                let coverrhs = cmax((lambda / al - feastol).ceil(), CDouble::new(1.0));
                let mut slackupper = -coverrhs;
                let mut step = CDouble::new(INF);
                for i in 0..cover.len() {
                    coefs[i] = (cmin(reducedcost[cover[i]], lambda) / al - self.settings.small_matrix_value).ceil().value();
                    slackupper += upper[cover[i]] * coefs[i];
                    step = cmin(step, reducedcost[cover[i]] / coefs[i]);
                }
                step = cmin(step, maxviolation / coverrhs);
                maxviolation -= step * coverrhs;
                // Only the cover's reduced costs change, so only they can
                // become negligible: their most goes to smallcontribution and
                // their reduced cost to zero, removing them below.
                for i in 0..cover.len() {
                    let index = cover[i];
                    reducedcost[index] -= step * coefs[i];
                    if reducedcost[index] * upper[index] <= feastol {
                        if reducedcost[index] > 0.0 {
                            smallcontribution += reducedcost[index] * upper[index];
                        }
                        reducedcost[index] = CDouble::new(0.0);
                    }
                }
                indices.retain(|&i| reducedcost[i] > 0.0);
                if step * slackupper <= feastol {
                    smallcontribution += step * slackupper;
                } else {
                    indices.push(reducedcost.len());
                    reducedcost.push(step);
                    upper.push(slackupper);
                }
            }
            let threshold = (maxviolation + feastol).value();
            indices.retain(|&i| !(i >= positions.len() || reducedcost[i].abs() <= threshold));
            if indices.is_empty() {
                continue;
            }
            let direction = if scale < 0.0 { -1.0 } else { 1.0 };
            let mut rhs = CDouble::new(if scale < 0.0 { self.mip.model.row_lower[row] } else { self.mip.model.row_upper[row] });
            for &i in &indices {
                let coefdelta = direction * (reducedcost[i] - maxviolation);
                let col = self.a_col[positions[i]];
                if complementation[i] == -1 {
                    rhs += coefdelta * self.mip.model.col_lower[col];
                    self.add_to_matrix(row, col, coefdelta.value());
                } else {
                    rhs -= coefdelta * self.mip.model.col_upper[col];
                    self.add_to_matrix(row, col, (-coefdelta).value());
                }
            }
            if scale < 0.0 {
                self.mip.model.row_lower[row] = rhs.value();
            } else {
                self.mip.model.row_upper[row] = rhs.value();
            }
            *num_strengthened += indices.len();
        }
        Outcome::Ok
    }
}

impl MipPresolve<'_> {
    /// HiGHS's `equalityRowAddition`: add `scale` times equation
    /// `stayrow` to `removerow`, cancelling their common entries.
    fn equality_row_addition(&mut self, stayrow: usize, removerow: usize, scale: f64, rowvector: Vec<(usize, f64)>) -> Outcome {
        let mut stay_rowpositions = Vec::new();
        matrix::in_order(self.rowroot[stayrow], &self.ar_left, &self.ar_right, &mut stay_rowpositions);
        let _ = rowvector;
        self.ps.equality_row_addition();
        for &rowiter in &stay_rowpositions {
            let pos = self.find_nonzero(removerow, self.a_col[rowiter]);
            if pos != NONE {
                self.unlink(pos);
            } else {
                self.add_to_matrix(removerow, self.a_col[rowiter], scale * self.a_value[rowiter]);
            }
        }
        if self.mip.model.row_upper[removerow] != INF {
            self.mip.model.row_upper[removerow] = (CDouble::new(scale) * self.mip.model.row_upper[stayrow] + self.mip.model.row_upper[removerow]).value();
        }
        if self.mip.model.row_lower[removerow] != -INF {
            self.mip.model.row_lower[removerow] = (CDouble::new(scale) * self.mip.model.row_upper[stayrow] + self.mip.model.row_lower[removerow]).value();
        }
        check!(self.row_presolve(removerow));
        Outcome::Ok
    }
}

/// `sparsify`'s `std::map<double, HighsInt>` of candidate scales.
#[derive(Default)]
struct ScaleMap(Vec<(f64, i64)>);

impl ScaleMap {
    fn clear(&mut self) {
        self.0.clear();
    }

    /// `std::map::lower_bound`: the first key not less than `key`.
    fn lower_bound(&self, key: f64) -> usize {
        self.0.partition_point(|&(k, _)| k < key)
    }

    fn emplace(&mut self, key: f64, value: i64) {
        let at = self.lower_bound(key);
        if at < self.0.len() && key.partial_cmp(&self.0[at].0).is_none_or(std::cmp::Ordering::is_ge) {
            return;
        }
        self.0.insert(at, (key, value));
    }

    /// Count `scale` for the entry `value` against `cand_row_val`. Returns
    /// whether the scale was dismissed as numerically bad (only the second
    /// scan stops looking at the entry then; it is the last thing either
    /// does).
    fn record(&mut self, scale: f64, scale_tolerance: f64, value: f64, cand_row_val: f64, small: f64) {
        let it = self.lower_bound(scale - scale_tolerance);
        if it != self.0.len() && (self.0[it].0 - scale).abs() <= scale_tolerance {
            // A scale this close could leave a tiny value here: it counts
            // only if the value cancels, and is dismissed otherwise.
            if self.0[it].1 == -1 {
                return;
            }
            if (self.0[it].0 * value + cand_row_val).abs() <= small {
                self.0[it].1 += 1;
            } else {
                self.0[it].1 = -1;
            }
        } else {
            self.emplace(scale, 1);
        }
    }

    /// The scale cancelling the most entries (more than `misses`), the
    /// smaller one on ties.
    fn best(&self, misses: i64) -> (f64, i64) {
        let mut num_cancel = 0;
        let mut scale = 0.0f64;
        for &(k, count) in &self.0 {
            if count <= misses {
                continue;
            }
            if count > num_cancel || (count == num_cancel && k.abs() < scale.abs()) {
                scale = k;
                num_cancel = count;
            }
        }
        (scale, num_cancel)
    }
}

impl MipPresolve<'_> {
    /// HiGHS's `sparsify`: add multiples of equations to rows where they
    /// cancel more entries than they fill.
    fn sparsify(&mut self) -> Outcome {
        let small = self.settings.small_matrix_value;
        let mut sparsify_rows: Vec<(usize, f64)> = Vec::new();
        check!(self.remove_row_singletons());
        check!(self.remove_doubleton_equations());
        let min_nonzero_val = self.primal_feastol.sqrt();
        let tmp_equations: Vec<usize> = self.equations.iter().map(|&(_, row)| row).collect();
        let mut possible_scales = ScaleMap::default();
        for eqrow in tmp_equations {
            if self.row_deleted[eqrow] {
                continue;
            }
            self.store_row(eqrow);
            let stored: Vec<usize> = self.rowpositions[..self.rowpositions_len].to_vec();
            let mut sparsest_col = NONE;
            let mut second_sparsest_col = NONE;
            let mut sparsest_col_len = usize::MAX;
            let mut second_sparsest_col_len = usize::MAX;
            for &nz_pos in &stored {
                let col = self.a_col[nz_pos];
                if self.colsize[col] < sparsest_col_len {
                    second_sparsest_col = sparsest_col;
                    second_sparsest_col_len = sparsest_col_len;
                    sparsest_col = col;
                    sparsest_col_len = self.colsize[col];
                } else if self.colsize[col] < second_sparsest_col_len {
                    second_sparsest_col = col;
                    second_sparsest_col_len = self.colsize[col];
                }
            }
            sparsify_rows.clear();

            for (cand_row, cand_val) in self.col_entries(sparsest_col) {
                if cand_row == eqrow {
                    continue;
                }
                possible_scales.clear();
                let mut misses: i64 = 0;
                // No fillin when a continuous row would cancel entries of a
                // row with integers, which can spoil the cuts.
                let mut max_misses: i64 = 1;
                if self.rowsize_integer[eqrow] == 0 && self.rowsize_integer[cand_row] != 0 {
                    max_misses -= 1;
                }
                for &pos in &stored {
                    let (col, value) = (self.a_col[pos], self.a_value[pos]);

                    let cand_row_val = if col == sparsest_col {
                        cand_val
                    } else {
                        let nz_pos = self.find_nonzero(cand_row, col);
                        if nz_pos == NONE {
                            if self.mip.model.integrality[col] == VarType::Integer && self.mip.model.col_upper[col] - self.mip.model.col_lower[col] > 1.5 {
                                // No fillin of general integers.
                                misses = 2;
                                break;
                            }
                            misses += 1;
                            if misses > max_misses {
                                break;
                            }
                            continue;
                        }
                        self.a_value[nz_pos]
                    };
                    let scale = -cand_row_val / value;
                    if scale.abs() > 1e3 {
                        continue;
                    }
                    let scale_tolerance = min_nonzero_val / value.abs();
                    possible_scales.record(scale, scale_tolerance, value, cand_row_val, small);
                }
                if misses > max_misses || possible_scales.0.is_empty() {
                    continue;
                }
                let (scale, num_cancel) = possible_scales.best(misses);
                if num_cancel > misses {
                    sparsify_rows.push((cand_row, scale));
                }
            }

            if self.mip.model.integrality[sparsest_col] != VarType::Integer || (self.mip.model.col_upper[sparsest_col] - self.mip.model.col_lower[sparsest_col]) < 1.5 {
                // Rows with all the other columns but not the sparsest.
                for (cand_row, cand_val) in self.col_entries(second_sparsest_col) {
                    if cand_row == eqrow {
                        continue;
                    }
                    if self.rowsize_integer[eqrow] == 0 && self.rowsize_integer[cand_row] != 0 {
                        continue;
                    }
                    if self.find_nonzero(cand_row, sparsest_col) != NONE {
                        continue;
                    }
                    possible_scales.clear();
                    let mut skip = false;
                    for &pos in &stored {
                        let (col, value) = (self.a_col[pos], self.a_value[pos]);

                        let cand_row_val = if col == second_sparsest_col {
                            cand_val
                        } else {
                            let nz_pos = self.find_nonzero(cand_row, col);
                            skip = nz_pos == NONE;
                            if skip {
                                break;
                            }
                            self.a_value[nz_pos]
                        };
                        let scale = -cand_row_val / value;
                        if scale.abs() > 1e3 {
                            continue;
                        }
                        let scale_tolerance = min_nonzero_val / value.abs();
                        possible_scales.record(scale, scale_tolerance, value, cand_row_val, small);
                    }
                    if skip || possible_scales.0.is_empty() {
                        continue;
                    }
                    let (scale, num_cancel) = possible_scales.best(1);
                    if num_cancel > 1 {
                        sparsify_rows.push((cand_row, scale));
                    }
                }
            }

            if sparsify_rows.is_empty() {
                continue;
            }
            self.ps.equality_row_additions();
            let rhs = self.mip.model.row_lower[eqrow];
            for &(row, scale) in &sparsify_rows {
                if self.mip.model.row_lower[row] != -INF {
                    self.mip.model.row_lower[row] += scale * rhs;
                }
                if self.mip.model.row_upper[row] != INF {
                    self.mip.model.row_upper[row] += scale * rhs;
                }
                for k in 0..self.rowpositions_len {
                    let pos = self.rowpositions[k];
                    self.add_to_matrix(row, self.a_col[pos], scale * self.a_value[pos]);
                }
                self.reinsert_equation(row);
            }
            check!(self.check_limits());
            check!(self.remove_row_singletons());
            check!(self.remove_doubleton_equations());
        }
        Outcome::Ok
    }
}
