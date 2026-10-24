//! HiGHS's `HighsPostsolveStack` for MIP presolve: every reduction the MIP
//! presolve records, undone for primal values only (the MIP solver's
//! `undoPrimal`), with the index maps and the cut bookkeeping the solver
//! needs across restarts.

use crate::{integers::fractionality, util::CDouble};

const INF: f64 = f64::INFINITY;

/// HiGHS's `RowType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowType {
    Geq,
    Leq,
    Eq,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Nonzero {
    pub index: usize,
    pub value: f64,
}

#[derive(Clone, Copy, Debug)]
struct DuplicateColumn {
    col_scale: f64,
    col_lower: f64,
    col_upper: f64,
    duplicate_col_lower: f64,
    duplicate_col_upper: f64,
    col: usize,
    duplicate_col: usize,
    col_integral: bool,
    duplicate_col_integral: bool,
}

#[derive(Clone, Debug)]
enum Reduction {
    LinearTransform {
        scale: f64,
        constant: f64,
        col: usize,
    },
    FreeColSubstitution {
        rhs: f64,
        row: usize,
        col: usize,
        row_values: Vec<Nonzero>,
    },
    DoubletonEquation {
        coef: f64,
        coef_subst: f64,
        rhs: f64,
        col_subst: usize,
        col: usize,
    },
    EqualityRowAddition,
    EqualityRowAdditions,
    SingletonRow,
    FixedCol {
        fix_value: f64,
        col: usize,
    },
    RedundantRow,
    ForcingRow,
    ForcingColumn {
        col_bound: f64,
        col: usize,
        at_infinite_upper: bool,
        col_integral: bool,
        col_values: Vec<Nonzero>,
    },
    ForcingColumnRemovedRow {
        rhs: f64,
        row: usize,
        row_values: Vec<Nonzero>,
    },
    DuplicateRow,
    DuplicateColumn(DuplicateColumn),
}

/// The tolerances postsolve reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tolerances {
    pub primal_feasibility_tolerance: f64,
    pub mip_feasibility_tolerance: f64,
}

/// HiGHS's `HighsPostsolveStack`.
#[derive(Clone, Debug, Default)]
pub(crate) struct MipPostsolveStack {
    reductions: Vec<Reduction>,
    orig_col_index: Vec<usize>,
    orig_row_index: Vec<usize>,
    linearly_transformable: Vec<bool>,
    orig_num_col: usize,
    orig_num_row: usize,
}

impl MipPostsolveStack {
    pub fn orig_row_index(&self, row: usize) -> usize {
        self.orig_row_index[row]
    }

    pub fn orig_col_index(&self, col: usize) -> usize {
        self.orig_col_index[col]
    }

    pub fn orig_num_row(&self) -> usize {
        self.orig_num_row
    }

    pub fn orig_num_col(&self) -> usize {
        self.orig_num_col
    }

    pub fn num_reductions(&self) -> usize {
        self.reductions.len()
    }

    /// HiGHS's `appendCutsToModel`.
    pub fn append_cuts_to_model(&mut self, num_cuts: usize) {
        for _ in 0..num_cuts {
            self.orig_row_index.push(self.orig_num_row);
            self.orig_num_row += 1;
        }
    }

    /// HiGHS's `removeCutsFromModel`.
    pub fn remove_cuts_from_model(&mut self, num_cuts: usize) {
        self.orig_num_row -= num_cuts;
        let mut size = self.orig_row_index.len();
        for i in (1..=self.orig_row_index.len()).rev() {
            if self.orig_row_index[i - 1] < self.orig_num_row {
                break;
            }
            size -= 1;
        }
        self.orig_row_index.truncate(size);
    }

    /// HiGHS's `initializeIndexMaps`.
    pub fn initialize_index_maps(&mut self, num_row: usize, num_col: usize) {
        self.orig_num_row = num_row;
        self.orig_num_col = num_col;
        self.orig_row_index = (0..num_row).collect();
        self.orig_col_index = (0..num_col).collect();
        self.linearly_transformable.resize(num_col, true);
    }

    /// HiGHS's `compressIndexMaps`, `None` marking deleted rows and columns.
    pub fn compress_index_maps(&mut self, new_row_index: &[Option<usize>], new_col_index: &[Option<usize>]) {
        let mut num_row = self.orig_row_index.len();
        for (i, new) in new_row_index.iter().enumerate() {
            match new {
                None => num_row -= 1,
                Some(new) => self.orig_row_index[*new] = self.orig_row_index[i],
            }
        }
        self.orig_row_index.truncate(num_row);
        let mut num_col = self.orig_col_index.len();
        for (i, new) in new_col_index.iter().enumerate() {
            match new {
                None => num_col -= 1,
                Some(new) => self.orig_col_index[*new] = self.orig_col_index[i],
            }
        }
        self.orig_col_index.truncate(num_col);
    }

    fn orig_rows(&self, entries: impl IntoIterator<Item = (usize, f64)>) -> Vec<Nonzero> {
        entries
            .into_iter()
            .map(|(row, value)| Nonzero {
                index: self.orig_row_index[row],
                value,
            })
            .collect()
    }

    fn orig_cols(&self, entries: impl IntoIterator<Item = (usize, f64)>) -> Vec<Nonzero> {
        entries
            .into_iter()
            .map(|(col, value)| Nonzero {
                index: self.orig_col_index[col],
                value,
            })
            .collect()
    }

    pub fn linear_transform(&mut self, col: usize, scale: f64, constant: f64) {
        self.reductions.push(Reduction::LinearTransform {
            scale,
            constant,
            col: self.orig_col_index[col],
        });
    }

    pub fn free_col_substitution(&mut self, row: usize, col: usize, rhs: f64, row_vec: impl IntoIterator<Item = (usize, f64)>) {
        let row_values = self.orig_cols(row_vec);
        self.reductions.push(Reduction::FreeColSubstitution {
            rhs,
            row: self.orig_row_index[row],
            col: self.orig_col_index[col],
            row_values,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn doubleton_equation(&mut self, col_subst: usize, col: usize, coef_subst: f64, coef: f64, rhs: f64) {
        self.reductions.push(Reduction::DoubletonEquation {
            coef,
            coef_subst,
            rhs,
            col_subst: self.orig_col_index[col_subst],
            col: self.orig_col_index[col],
        });
    }

    pub fn equality_row_addition(&mut self) {
        self.reductions.push(Reduction::EqualityRowAddition);
    }

    pub fn equality_row_additions(&mut self) {
        self.reductions.push(Reduction::EqualityRowAdditions);
    }

    pub fn singleton_row(&mut self) {
        self.reductions.push(Reduction::SingletonRow);
    }

    /// HiGHS's `fixedColAtLower`, `fixedColAtUpper`, `fixedColAtZero` and
    /// `removedFixedCol`, which postsolve the same way for primal values.
    pub fn fixed_col(&mut self, col: usize, fix_value: f64) {
        self.reductions.push(Reduction::FixedCol {
            fix_value,
            col: self.orig_col_index[col],
        });
    }

    pub fn redundant_row(&mut self) {
        self.reductions.push(Reduction::RedundantRow);
    }

    pub fn forcing_row(&mut self) {
        self.reductions.push(Reduction::ForcingRow);
    }

    pub fn forcing_column(&mut self, col: usize, col_vec: impl IntoIterator<Item = (usize, f64)>, bound_val: f64, at_infinite_upper: bool, col_integral: bool) {
        let col_values = self.orig_rows(col_vec);
        self.reductions.push(Reduction::ForcingColumn {
            col_bound: bound_val,
            col: self.orig_col_index[col],
            at_infinite_upper,
            col_integral,
            col_values,
        });
    }

    pub fn forcing_column_removed_row(&mut self, forcing_col: usize, row: usize, rhs: f64, row_vec: impl IntoIterator<Item = (usize, f64)>) {
        let row_values = self.orig_cols(row_vec.into_iter().filter(|&(col, _)| col != forcing_col));
        self.reductions.push(Reduction::ForcingColumnRemovedRow {
            rhs,
            row: self.orig_row_index[row],
            row_values,
        });
    }

    pub fn duplicate_row(&mut self) {
        self.reductions.push(Reduction::DuplicateRow);
    }

    /// HiGHS's `duplicateColumn`: false (and nothing recorded) if the
    /// merge could not be undone.
    #[allow(clippy::too_many_arguments)]
    pub fn duplicate_column(
        &mut self,
        col_scale: f64,
        col_lower: f64,
        col_upper: f64,
        duplicate_col_lower: f64,
        duplicate_col_upper: f64,
        col: usize,
        duplicate_col: usize,
        col_integral: bool,
        duplicate_col_integral: bool,
        ok_merge_tolerance: f64,
    ) -> bool {
        let orig_col = self.orig_col_index[col];
        let orig_duplicate_col = self.orig_col_index[duplicate_col];
        let values = DuplicateColumn {
            col_scale,
            col_lower,
            col_upper,
            duplicate_col_lower,
            duplicate_col_upper,
            col: orig_col,
            duplicate_col: orig_duplicate_col,
            col_integral,
            duplicate_col_integral,
        };
        if !values.ok_merge(ok_merge_tolerance) {
            return false;
        }
        self.reductions.push(Reduction::DuplicateColumn(values));
        self.linearly_transformable[orig_col] = false;
        self.linearly_transformable[orig_duplicate_col] = false;
        true
    }

    /// HiGHS's `getReducedPrimalSolution`.
    pub fn reduced_primal_solution(&self, orig_primal_solution: &[f64]) -> Vec<f64> {
        let mut reduced = orig_primal_solution.to_vec();
        for reduction in &self.reductions {
            match reduction {
                Reduction::DuplicateColumn(d) => reduced[d.col] += d.col_scale * reduced[d.duplicate_col],
                Reduction::LinearTransform { scale, constant, col } => {
                    reduced[*col] -= constant;
                    reduced[*col] /= scale;
                }
                _ => {}
            }
        }
        let reduced_num_col = self.orig_col_index.len();
        for i in 0..reduced_num_col {
            reduced[i] = reduced[self.orig_col_index[i]];
        }
        reduced.truncate(reduced_num_col);
        reduced
    }

    pub fn is_col_linearly_transformable(&self, col: usize) -> bool {
        self.linearly_transformable[self.orig_col_index[col]]
    }

    /// HiGHS's `undoIterateBackwards`.
    fn undo_iterate_backwards(values: &mut Vec<f64>, index: &[usize], orig_size: usize) {
        values.resize(orig_size, 0.0);
        for i in (1..=index.len()).rev() {
            values[index[i - 1]] = values[i - 1];
        }
    }

    /// HiGHS's `undoPrimal`: the original model's column values (and the
    /// partial row values HiGHS keeps alongside) for the reduced ones.
    pub fn undo_primal(&self, tol: Tolerances, col_value: &mut Vec<f64>, row_value: &mut Vec<f64>) {
        Self::undo_iterate_backwards(col_value, &self.orig_col_index, self.orig_num_col);
        Self::undo_iterate_backwards(row_value, &self.orig_row_index, self.orig_num_row);
        for reduction in self.reductions.iter().rev() {
            let is_model_row = |row: usize, row_value: &Vec<f64>| row < row_value.len();
            match reduction {
                Reduction::LinearTransform { scale, constant, col } => {
                    col_value[*col] *= scale;
                    col_value[*col] += constant;
                }
                Reduction::FreeColSubstitution { rhs, row, col, row_values } => {
                    let mut col_coef = 0.0;
                    let mut row_val = CDouble::new(0.0);
                    for rv in row_values {
                        if rv.index == *col {
                            col_coef = rv.value;
                        } else {
                            row_val += rv.value * col_value[rv.index];
                        }
                    }
                    if is_model_row(*row, row_value) {
                        row_value[*row] = (row_val + col_coef * col_value[*col]).value();
                    }
                    col_value[*col] = ((CDouble::new(*rhs) - row_val) / col_coef).value();
                }
                Reduction::DoubletonEquation {
                    coef,
                    coef_subst,
                    rhs,
                    col_subst,
                    col,
                } => {
                    col_value[*col_subst] = ((CDouble::new(*rhs) - CDouble::new(*coef) * col_value[*col]) / *coef_subst).value();
                }
                Reduction::FixedCol { fix_value, col } => col_value[*col] = *fix_value,
                Reduction::ForcingColumn {
                    col_bound,
                    col,
                    at_infinite_upper,
                    col_integral,
                    col_values,
                } => {
                    let direction = if *at_infinite_upper { 1.0 } else { -1.0 };
                    let mut nonbasic_row = false;
                    let mut col_val_from_nonbasic_row = *col_bound;
                    for cv in col_values {
                        if is_model_row(cv.index, row_value) {
                            let col_val_from_row = row_value[cv.index] / cv.value;
                            if direction * col_val_from_row > direction * col_val_from_nonbasic_row {
                                nonbasic_row = true;
                                col_val_from_nonbasic_row = col_val_from_row;
                            }
                        }
                    }
                    if nonbasic_row && *col_integral {
                        col_val_from_nonbasic_row = direction * (direction * col_val_from_nonbasic_row - tol.mip_feasibility_tolerance).ceil();
                    }
                    col_value[*col] = col_val_from_nonbasic_row;
                }
                Reduction::ForcingColumnRemovedRow { rhs, row, row_values } => {
                    if !is_model_row(*row, row_value) {
                        continue;
                    }
                    let mut val = CDouble::new(*rhs);
                    for rv in row_values {
                        val -= rv.value * col_value[rv.index];
                    }
                    row_value[*row] = val.value();
                }
                Reduction::DuplicateColumn(d) => d.undo_primal(tol, col_value),
                Reduction::EqualityRowAddition
                | Reduction::EqualityRowAdditions
                | Reduction::SingletonRow
                | Reduction::RedundantRow
                | Reduction::ForcingRow
                | Reduction::DuplicateRow => {}
            }
        }
    }
}

impl DuplicateColumn {
    /// HiGHS's `okMerge`.
    fn ok_merge(&self, tolerance: f64) -> bool {
        let scale = self.col_scale;
        let x_int = self.col_integral;
        let y_int = self.duplicate_col_integral;
        let x_lo = if x_int { (self.col_lower - tolerance).ceil() } else { self.col_lower };
        let x_up = if x_int { (self.col_upper + tolerance).floor() } else { self.col_upper };
        let y_lo = if y_int {
            (self.duplicate_col_lower - tolerance).ceil()
        } else {
            self.duplicate_col_lower
        };
        let y_up = if y_int {
            (self.duplicate_col_upper + tolerance).floor()
        } else {
            self.duplicate_col_upper
        };
        let x_len = x_up - x_lo;
        let y_len = y_up - y_lo;
        let mut ok_merge = true;
        if scale == 0.0 {
            ok_merge = false;
        }
        let abs_scale = scale.abs();
        if x_int {
            if y_int {
                if fractionality(scale) > tolerance {
                    ok_merge = false;
                }
                let scale_limit = x_len + 1.0 + tolerance;
                if abs_scale > scale_limit {
                    ok_merge = false;
                }
            } else if y_len == 0.0 {
                ok_merge = false;
            } else {
                let scale_limit = 1.0 / y_len;
                if abs_scale < scale_limit {
                    ok_merge = false;
                }
            }
        } else if y_int {
            let scale_limit = x_len;
            if abs_scale > scale_limit {
                ok_merge = false;
            }
        }
        ok_merge
    }

    /// `DuplicateColumn::undo` without a basis.
    fn undo_primal(&self, tol: Tolerances, col_value: &mut [f64]) {
        let (col, dup) = (self.col, self.duplicate_col);
        let merge_val = col_value[col];
        let primal_tol = tol.primal_feasibility_tolerance;
        let mip_tol = tol.mip_feasibility_tolerance;
        let ok_residual = |x: f64, y: f64| ((x + self.col_scale * y) - merge_val).abs() <= primal_tol;
        col_value[col] = if self.col_lower != -INF { self.col_lower } else { 0.0f64.min(self.col_upper) };
        col_value[dup] = ((CDouble::new(merge_val) - col_value[col]) / self.col_scale).value();
        let mut recompute_col = false;
        if col_value[dup] > self.duplicate_col_upper {
            col_value[dup] = self.duplicate_col_upper;
            recompute_col = true;
        } else if col_value[dup] < self.duplicate_col_lower {
            col_value[dup] = self.duplicate_col_lower;
            recompute_col = true;
        } else if self.duplicate_col_integral && fractionality(col_value[dup]) > mip_tol {
            col_value[dup] = col_value[dup].floor();
            recompute_col = true;
        }
        if recompute_col {
            col_value[col] = merge_val - self.col_scale * col_value[dup];
            if !self.duplicate_col_integral && self.col_integral {
                col_value[col] = (col_value[col] - mip_tol).ceil();
                col_value[dup] = ((CDouble::new(merge_val) - col_value[col]) / self.col_scale).value();
            }
        }
        let has_error = |col_value: &[f64]| {
            col_value[dup] < self.duplicate_col_lower - mip_tol
                || col_value[dup] > self.duplicate_col_upper + mip_tol
                || col_value[col] < self.col_lower - mip_tol
                || col_value[col] > self.col_upper + mip_tol
                || !ok_residual(col_value[col], col_value[dup])
        };
        if has_error(col_value) {
            self.undo_fix(tol, col_value, merge_val);
        }
    }

    /// HiGHS's `undoFix`.
    fn undo_fix(&self, tol: Tolerances, col_value: &mut [f64], merge_value: f64) {
        let mip_tol = tol.mip_feasibility_tolerance;
        let primal_tol = tol.primal_feasibility_tolerance;
        let is_integer = |v: f64| fractionality(v) <= mip_tol;
        let is_feasible = |l: f64, v: f64, u: f64| v >= l - primal_tol && v <= u + primal_tol;
        let scale = self.col_scale;
        let x_int = self.col_integral;
        let y_int = self.duplicate_col_integral;
        let x_lo = if x_int { (self.col_lower - mip_tol).ceil() } else { self.col_lower };
        let x_up = if x_int { (self.col_upper + mip_tol).floor() } else { self.col_upper };
        let y_lo = if y_int { (self.duplicate_col_lower - mip_tol).ceil() } else { self.duplicate_col_lower };
        let y_up = if y_int {
            (self.duplicate_col_upper + mip_tol).floor()
        } else {
            self.duplicate_col_upper
        };
        let mut x_v = merge_value;
        let mut y_v = -INF;
        let check_int_var = |z_lo: f64, z_up: f64| -> (f64, f64, f64) {
            const VALUE_MAX: f64 = 1000.0;
            if z_lo <= -INF {
                if z_up >= INF { (0.0, 1.0, VALUE_MAX) } else { (z_up, -1.0, -VALUE_MAX) }
            } else {
                (z_lo, 1.0, if z_up >= INF { VALUE_MAX } else { z_up })
            }
        };
        let compute_value = |value: f64| ((CDouble::new(merge_value) - value) / scale).value();
        let compute_inv_value = |value: f64| (CDouble::new(merge_value) - CDouble::new(value) * scale).value();
        let find_value = |z_value: &mut f64, z_0: f64, z_1: f64, z_delta: f64, other_value: &mut f64, other_lower: f64, other_upper: f64, other_int: bool| -> bool {
            const EPS: f64 = 1e-8;
            *z_value = z_0;
            loop {
                *other_value = compute_value(*z_value);
                if is_feasible(other_lower, *other_value, other_upper) && (!other_int || is_integer(*other_value)) {
                    return true;
                }
                if z_delta > 0.0 && *z_value + z_delta >= z_1 + EPS {
                    return false;
                }
                if z_delta < 0.0 && *z_value + z_delta <= z_1 - EPS {
                    return false;
                }
                *z_value += z_delta;
            }
        };
        if x_int {
            let (x_0, x_d, x_1) = check_int_var(x_lo, x_up);
            find_value(&mut x_v, x_0, x_1, x_d, &mut y_v, y_lo, y_up, y_int);
        } else if y_int {
            let (y_0, y_d, y_1) = check_int_var(y_lo, y_up);
            find_value(&mut y_v, y_0, y_1, y_d, &mut x_v, x_lo, x_up, x_int);
        } else {
            let set_x = |value: f64, x_v: &mut f64, y_v: &mut f64| {
                *x_v = value;
                *y_v = compute_value(value);
            };
            let set_y = |value: f64, x_v: &mut f64, y_v: &mut f64| {
                *y_v = value;
                *x_v = compute_inv_value(value);
            };
            if scale > 0.0 {
                if y_up < INF {
                    set_y(y_up, &mut x_v, &mut y_v);
                    if x_v < x_lo - primal_tol {
                        set_x(x_lo, &mut x_v, &mut y_v);
                        if y_v < y_lo - primal_tol {
                            set_x(x_lo - primal_tol, &mut x_v, &mut y_v);
                        }
                    }
                } else if y_lo > -INF {
                    set_y(y_lo, &mut x_v, &mut y_v);
                    if x_v > x_up + primal_tol {
                        set_x(x_up, &mut x_v, &mut y_v);
                        if y_v > y_up + primal_tol {
                            set_x(x_up + primal_tol, &mut x_v, &mut y_v);
                        }
                    }
                } else {
                    set_x(0.0f64.max(x_lo), &mut x_v, &mut y_v);
                }
            } else if y_lo > -INF {
                set_y(y_lo, &mut x_v, &mut y_v);
                if x_v < x_lo - primal_tol {
                    set_x(x_lo, &mut x_v, &mut y_v);
                    if y_v > y_up + primal_tol {
                        set_x(x_lo - primal_tol, &mut x_v, &mut y_v);
                    }
                }
            } else if y_up < INF {
                set_y(y_up, &mut x_v, &mut y_v);
                if x_v > x_up + primal_tol {
                    set_x(x_up, &mut x_v, &mut y_v);
                    if y_v < y_lo - primal_tol {
                        set_x(x_up + primal_tol, &mut x_v, &mut y_v);
                    }
                }
            } else {
                set_x(0.0f64.max(x_lo), &mut x_v, &mut y_v);
            }
        }
        const RESIDUAL_TOLERANCE: f64 = 1e-12;
        let residual = (CDouble::new(x_v) - compute_inv_value(y_v)).value().abs();
        let x_y_ok = is_feasible(x_lo, x_v, x_up)
            && is_feasible(y_lo, y_v, y_up)
            && (!x_int || is_integer(x_v))
            && (!y_int || is_integer(y_v))
            && x_v.abs() < INF
            && y_v.abs() < INF
            && residual <= RESIDUAL_TOLERANCE;
        if x_y_ok {
            col_value[self.col] = x_v;
            col_value[self.duplicate_col] = y_v;
        }
    }
}
