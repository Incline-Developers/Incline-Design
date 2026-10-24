//! HiGHS's `HighsPostsolveStack`: the reductions presolve made, in order,
//! each able to undo itself, turning a solution and basis of the reduced
//! LP into ones of the original.

use super::NONE;
use crate::util::CDouble;

/// HiGHS's `HighsBasisStatus`, with its `kNonbasic`: nonbasic at a bound
/// still to be chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Lower,
    Basic,
    Upper,
    Zero,
    Nonbasic,
}

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

#[derive(Clone, Debug, Default)]
pub(crate) struct Solution {
    pub col_value: Vec<f64>,
    pub col_dual: Vec<f64>,
    pub row_value: Vec<f64>,
    pub row_dual: Vec<f64>,
    pub dual_valid: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Basis {
    pub col_status: Vec<Status>,
    pub row_status: Vec<Status>,
    pub valid: bool,
}

#[derive(Clone, Debug)]
enum Reduction {
    FreeColSubstitution {
        rhs: f64,
        col_cost: f64,
        row: usize,
        col: usize,
        row_type: RowType,
        row_values: Vec<Nonzero>,
        col_values: Vec<Nonzero>,
    },
    DoubletonEquation(DoubletonEquation, Vec<Nonzero>),
    SingletonRow {
        coef: f64,
        row: usize,
        col: usize,
        col_lower_tightened: bool,
        col_upper_tightened: bool,
    },
    FixedCol {
        fix_value: f64,
        col_cost: f64,
        col: usize,
        fix_type: Status,
        col_values: Vec<Nonzero>,
    },
    RedundantRow {
        row: usize,
    },
    ForcingRow {
        row: usize,
        row_type: RowType,
        row_values: Vec<Nonzero>,
    },
    ForcingColumn {
        col_bound: f64,
        col: usize,
        at_infinite_upper: bool,
        col_values: Vec<Nonzero>,
    },
    ForcingColumnRemovedRow {
        rhs: f64,
        row: usize,
        row_values: Vec<Nonzero>,
    },
    DuplicateRow {
        duplicate_row_scale: f64,
        duplicate_row: usize,
        row: usize,
        row_lower_tightened: bool,
        row_upper_tightened: bool,
    },
    DuplicateColumn(DuplicateColumn),
}

#[derive(Clone, Copy, Debug)]
struct DoubletonEquation {
    coef: f64,
    coef_subst: f64,
    rhs: f64,
    subst_cost: f64,
    /// `NONE` for a substitution not from a model row.
    row: usize,
    col_subst: usize,
    col: usize,
    lower_tightened: bool,
    upper_tightened: bool,
    row_type: RowType,
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
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PostsolveStack {
    reductions: Vec<Reduction>,
    orig_col_index: Vec<usize>,
    orig_row_index: Vec<usize>,
    orig_num_col: usize,
    orig_num_row: usize,
}

/// Postsolve's tolerances, from the options.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tolerances {
    pub primal_feasibility: f64,
    pub dual_feasibility: f64,
}

impl PostsolveStack {
    pub fn initialize_index_maps(&mut self, num_row: usize, num_col: usize) {
        self.orig_num_row = num_row;
        self.orig_num_col = num_col;
        self.orig_row_index = (0..num_row).collect();
        self.orig_col_index = (0..num_col).collect();
    }

    pub fn compress_index_maps(&mut self, new_row_index: &[usize], new_col_index: &[usize]) {
        let mut num_row = self.orig_row_index.len();
        for (i, &to) in new_row_index.iter().enumerate() {
            if to == NONE {
                num_row -= 1;
            } else {
                self.orig_row_index[to] = self.orig_row_index[i];
            }
        }
        self.orig_row_index.truncate(num_row);
        let mut num_col = self.orig_col_index.len();
        for (i, &to) in new_col_index.iter().enumerate() {
            if to == NONE {
                num_col -= 1;
            } else {
                self.orig_col_index[to] = self.orig_col_index[i];
            }
        }
        self.orig_col_index.truncate(num_col);
    }

    pub fn num_reductions(&self) -> usize {
        self.reductions.len()
    }

    fn row_values(&self, row: impl IntoIterator<Item = (usize, f64)>) -> Vec<Nonzero> {
        row.into_iter()
            .map(|(col, value)| Nonzero {
                index: self.orig_col_index[col],
                value,
            })
            .collect()
    }

    fn col_values(&self, col: impl IntoIterator<Item = (usize, f64)>) -> Vec<Nonzero> {
        col.into_iter()
            .map(|(row, value)| Nonzero {
                index: self.orig_row_index[row],
                value,
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn free_col_substitution(
        &mut self,
        row: usize,
        col: usize,
        rhs: f64,
        col_cost: f64,
        row_type: RowType,
        row_vec: impl IntoIterator<Item = (usize, f64)>,
        col_vec: impl IntoIterator<Item = (usize, f64)>,
    ) {
        let row_values = self.row_values(row_vec);
        let col_values = self.col_values(col_vec);
        self.reductions.push(Reduction::FreeColSubstitution {
            rhs,
            col_cost,
            row: self.orig_row_index[row],
            col: self.orig_col_index[col],
            row_type,
            row_values,
            col_values,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn doubleton_equation(
        &mut self,
        row: usize,
        col_subst: usize,
        col: usize,
        coef_subst: f64,
        coef: f64,
        rhs: f64,
        subst_cost: f64,
        lower_tightened: bool,
        upper_tightened: bool,
        row_type: RowType,
        col_vec: impl IntoIterator<Item = (usize, f64)>,
    ) {
        let col_values = self.col_values(col_vec);
        let reduction = DoubletonEquation {
            coef,
            coef_subst,
            rhs,
            subst_cost,
            row: if row == NONE { NONE } else { self.orig_row_index[row] },
            col_subst: self.orig_col_index[col_subst],
            col: self.orig_col_index[col],
            lower_tightened,
            upper_tightened,
            row_type,
        };
        self.reductions.push(Reduction::DoubletonEquation(reduction, col_values));
    }

    pub fn singleton_row(&mut self, row: usize, col: usize, coef: f64, col_lower_tightened: bool, col_upper_tightened: bool) {
        self.reductions.push(Reduction::SingletonRow {
            coef,
            row: self.orig_row_index[row],
            col: self.orig_col_index[col],
            col_lower_tightened,
            col_upper_tightened,
        });
    }

    fn fixed_col(&mut self, col: usize, fix_value: f64, col_cost: f64, fix_type: Status, col_vec: impl IntoIterator<Item = (usize, f64)>) {
        let col_values = self.col_values(col_vec);
        self.reductions.push(Reduction::FixedCol {
            fix_value,
            col_cost,
            col: self.orig_col_index[col],
            fix_type,
            col_values,
        });
    }

    pub fn fixed_col_at_lower(&mut self, col: usize, fix_value: f64, col_cost: f64, col_vec: impl IntoIterator<Item = (usize, f64)>) {
        self.fixed_col(col, fix_value, col_cost, Status::Lower, col_vec);
    }

    pub fn fixed_col_at_upper(&mut self, col: usize, fix_value: f64, col_cost: f64, col_vec: impl IntoIterator<Item = (usize, f64)>) {
        self.fixed_col(col, fix_value, col_cost, Status::Upper, col_vec);
    }

    pub fn fixed_col_at_zero(&mut self, col: usize, col_cost: f64, col_vec: impl IntoIterator<Item = (usize, f64)>) {
        self.fixed_col(col, 0.0, col_cost, Status::Zero, col_vec);
    }

    pub fn removed_fixed_col(&mut self, col: usize, fix_value: f64, col_cost: f64, col_vec: impl IntoIterator<Item = (usize, f64)>) {
        self.fixed_col(col, fix_value, col_cost, Status::Nonbasic, col_vec);
    }

    pub fn redundant_row(&mut self, row: usize) {
        self.reductions.push(Reduction::RedundantRow { row: self.orig_row_index[row] });
    }

    pub fn forcing_row(&mut self, row: usize, row_vec: impl IntoIterator<Item = (usize, f64)>, row_type: RowType) {
        let row_values = self.row_values(row_vec);
        self.reductions.push(Reduction::ForcingRow {
            row: self.orig_row_index[row],
            row_type,
            row_values,
        });
    }

    pub fn forcing_column(&mut self, col: usize, col_vec: impl IntoIterator<Item = (usize, f64)>, bound_val: f64, at_infinite_upper: bool) {
        let col_values = self.col_values(col_vec);
        self.reductions.push(Reduction::ForcingColumn {
            col_bound: bound_val,
            col: self.orig_col_index[col],
            at_infinite_upper,
            col_values,
        });
    }

    pub fn forcing_column_removed_row(&mut self, forcing_col: usize, row: usize, rhs: f64, row_vec: impl IntoIterator<Item = (usize, f64)>) {
        let row_values = self.row_values(row_vec.into_iter().filter(|&(col, _)| col != forcing_col));
        self.reductions.push(Reduction::ForcingColumnRemovedRow {
            rhs,
            row: self.orig_row_index[row],
            row_values,
        });
    }

    pub fn duplicate_row(&mut self, row: usize, row_upper_tightened: bool, row_lower_tightened: bool, duplicate_row: usize, duplicate_row_scale: f64) {
        self.reductions.push(Reduction::DuplicateRow {
            duplicate_row_scale,
            duplicate_row: self.orig_row_index[duplicate_row],
            row: self.orig_row_index[row],
            row_lower_tightened,
            row_upper_tightened,
        });
    }

    /// Merge `duplicate_col` into `col` (both continuous, so the merge is
    /// always allowed: HiGHS's `okMerge` only rejects a zero scale).
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
    ) -> bool {
        if col_scale == 0.0 {
            return false;
        }
        self.reductions.push(Reduction::DuplicateColumn(DuplicateColumn {
            col_scale,
            col_lower,
            col_upper,
            duplicate_col_lower,
            duplicate_col_upper,
            col: self.orig_col_index[col],
            duplicate_col: self.orig_col_index[duplicate_col],
        }));
        true
    }

    /// Turn the reduced LP's solution and basis into the original's.
    pub fn undo(&self, tol: Tolerances, solution: &mut Solution, basis: &mut Basis) {
        undo_iterate_backwards(&mut solution.col_value, &self.orig_col_index, self.orig_num_col, 0.0);
        undo_iterate_backwards(&mut solution.row_value, &self.orig_row_index, self.orig_num_row, 0.0);
        if solution.dual_valid {
            undo_iterate_backwards(&mut solution.col_dual, &self.orig_col_index, self.orig_num_col, 0.0);
            undo_iterate_backwards(&mut solution.row_dual, &self.orig_row_index, self.orig_num_row, 0.0);
        }
        if basis.valid {
            // Resizing a C++ status vector fills it with kLower.
            undo_iterate_backwards(&mut basis.col_status, &self.orig_col_index, self.orig_num_col, Status::Lower);
            undo_iterate_backwards(&mut basis.row_status, &self.orig_row_index, self.orig_num_row, Status::Lower);
        }
        for reduction in self.reductions.iter().rev() {
            match reduction {
                Reduction::FreeColSubstitution {
                    rhs,
                    col_cost,
                    row,
                    col,
                    row_type,
                    row_values,
                    col_values,
                } => undo_free_col_substitution(*rhs, *col_cost, *row, *col, *row_type, row_values, col_values, solution, basis),
                Reduction::DoubletonEquation(reduction, col_values) => reduction.undo(tol, col_values, solution, basis),
                Reduction::SingletonRow {
                    coef,
                    row,
                    col,
                    col_lower_tightened,
                    col_upper_tightened,
                } => undo_singleton_row(tol, *coef, *row, *col, *col_lower_tightened, *col_upper_tightened, solution, basis),
                Reduction::FixedCol {
                    fix_value,
                    col_cost,
                    col,
                    fix_type,
                    col_values,
                } => undo_fixed_col(*fix_value, *col_cost, *col, *fix_type, col_values, solution, basis),
                Reduction::RedundantRow { row } => {
                    if solution.dual_valid {
                        solution.row_dual[*row] = 0.0;
                        if basis.valid {
                            basis.row_status[*row] = Status::Basic;
                        }
                    }
                }
                Reduction::ForcingRow { row, row_type, row_values } => undo_forcing_row(*row, *row_type, row_values, solution, basis),
                Reduction::ForcingColumn {
                    col_bound,
                    col,
                    at_infinite_upper,
                    col_values,
                } => undo_forcing_column(*col_bound, *col, *at_infinite_upper, col_values, solution, basis),
                Reduction::ForcingColumnRemovedRow { rhs, row, row_values } => {
                    let mut val = CDouble::new(*rhs);
                    for nz in row_values {
                        val -= nz.value * solution.col_value[nz.index];
                    }
                    solution.row_value[*row] = val.value();
                    if solution.dual_valid {
                        solution.row_dual[*row] = 0.0;
                    }
                    if basis.valid {
                        basis.row_status[*row] = Status::Basic;
                    }
                }
                Reduction::DuplicateRow {
                    duplicate_row_scale,
                    duplicate_row,
                    row,
                    row_lower_tightened,
                    row_upper_tightened,
                } => undo_duplicate_row(tol, *duplicate_row_scale, *duplicate_row, *row, *row_lower_tightened, *row_upper_tightened, solution, basis),
                Reduction::DuplicateColumn(reduction) => reduction.undo(tol, solution, basis),
            }
        }
    }
}

fn undo_iterate_backwards<T: Copy>(values: &mut Vec<T>, index: &[usize], orig_size: usize, fill: T) {
    values.resize(orig_size, fill);
    for i in (0..index.len()).rev() {
        values[index[i]] = values[i];
    }
}

fn compute_row_status(dual: f64, row_type: RowType) -> Status {
    match row_type {
        RowType::Eq => {
            if dual < 0.0 {
                Status::Upper
            } else {
                Status::Lower
            }
        }
        RowType::Geq => Status::Lower,
        RowType::Leq => Status::Upper,
    }
}

/// HiGHS's `computeStatus` with a basis: a clearly signed dual decides
/// the status, otherwise the basis's stands.
fn compute_status_in(dual: f64, status: &mut Status, dual_feasibility_tolerance: f64) -> Status {
    if dual > dual_feasibility_tolerance {
        *status = Status::Lower;
    } else if dual < -dual_feasibility_tolerance {
        *status = Status::Upper;
    }
    *status
}

fn compute_status(dual: f64, dual_feasibility_tolerance: f64) -> Status {
    if dual > dual_feasibility_tolerance {
        Status::Lower
    } else if dual < -dual_feasibility_tolerance {
        Status::Upper
    } else {
        Status::Basic
    }
}

#[allow(clippy::too_many_arguments)]
fn undo_free_col_substitution(
    rhs: f64,
    col_cost: f64,
    row: usize,
    col: usize,
    row_type: RowType,
    row_values: &[Nonzero],
    col_values: &[Nonzero],
    solution: &mut Solution,
    basis: &mut Basis,
) {
    let mut col_coef = 0.0;
    let mut row_value = CDouble::new(0.0);
    for nz in row_values {
        if nz.index == col {
            col_coef = nz.value;
        } else {
            row_value += nz.value * solution.col_value[nz.index];
        }
    }
    solution.row_value[row] = (row_value + col_coef * solution.col_value[col]).value();
    solution.col_value[col] = ((CDouble::new(rhs) - row_value) / col_coef).value();
    if !solution.dual_valid {
        return;
    }
    solution.row_dual[row] = 0.0;
    let mut dualval = CDouble::new(col_cost);
    for nz in col_values {
        dualval -= nz.value * solution.row_dual[nz.index];
    }
    solution.row_dual[row] = (dualval / col_coef).value();
    solution.col_dual[col] = 0.0;
    if !basis.valid {
        return;
    }
    basis.col_status[col] = Status::Basic;
    basis.row_status[row] = compute_row_status(solution.row_dual[row], row_type);
}

impl DoubletonEquation {
    fn undo(&self, tol: Tolerances, col_values: &[Nonzero], solution: &mut Solution, basis: &mut Basis) {
        let &DoubletonEquation {
            coef,
            coef_subst,
            rhs,
            subst_cost,
            row,
            col_subst,
            col,
            lower_tightened,
            upper_tightened,
            row_type,
        } = self;
        solution.col_value[col_subst] = ((CDouble::new(rhs) - CDouble::new(coef) * solution.col_value[col]) / coef_subst).value();
        if row == NONE || !solution.dual_valid {
            return;
        }
        let col_status = if !basis.valid {
            compute_status(solution.col_dual[col], tol.dual_feasibility)
        } else {
            compute_status_in(solution.col_dual[col], &mut basis.col_status[col], tol.dual_feasibility)
        };
        let mut row_dual = CDouble::new(0.0);
        solution.row_dual[row] = 0.0;
        for nz in col_values {
            row_dual -= nz.value * solution.row_dual[nz.index];
        }
        row_dual /= coef_subst;
        solution.row_dual[row] = row_dual.value();

        solution.col_dual[col_subst] = subst_cost;
        solution.col_dual[col] += subst_cost * coef / coef_subst;
        if (upper_tightened && col_status == Status::Upper) || (lower_tightened && col_status == Status::Lower) {
            let row_dual_delta = solution.col_dual[col] / coef;
            solution.row_dual[row] = (row_dual + row_dual_delta).value();
            solution.col_dual[col] = 0.0;
            solution.col_dual[col_subst] = (CDouble::new(solution.col_dual[col_subst]) - row_dual_delta * coef_subst).value();
            if basis.valid {
                let same_sign = coef.is_sign_negative() == coef_subst.is_sign_negative();
                basis.col_status[col_subst] = if (same_sign && basis.col_status[col] == Status::Upper) || (!same_sign && basis.col_status[col] == Status::Lower) {
                    Status::Lower
                } else {
                    Status::Upper
                };
                basis.col_status[col] = Status::Basic;
            }
        } else {
            let row_dual_delta = solution.col_dual[col_subst] / coef_subst;
            solution.row_dual[row] = (row_dual + row_dual_delta).value();
            solution.col_dual[col_subst] = 0.0;
            solution.col_dual[col] = (CDouble::new(solution.col_dual[col]) - row_dual_delta * coef).value();
            if basis.valid {
                basis.col_status[col_subst] = Status::Basic;
            }
        }
        if !basis.valid {
            return;
        }
        basis.row_status[row] = compute_row_status(solution.row_dual[row], row_type);
    }
}

#[allow(clippy::too_many_arguments)]
fn undo_singleton_row(tol: Tolerances, coef: f64, row: usize, col: usize, col_lower_tightened: bool, col_upper_tightened: bool, solution: &mut Solution, basis: &mut Basis) {
    if !solution.dual_valid {
        return;
    }
    let col_status = if !basis.valid {
        compute_status(solution.col_dual[col], tol.dual_feasibility)
    } else {
        compute_status_in(solution.col_dual[col], &mut basis.col_status[col], tol.dual_feasibility)
    };
    if (!col_lower_tightened || col_status != Status::Lower) && (!col_upper_tightened || col_status != Status::Upper) {
        if basis.valid {
            basis.row_status[row] = Status::Basic;
        }
        solution.row_dual[row] = 0.0;
        return;
    }
    solution.row_dual[row] = solution.col_dual[col] / coef;
    solution.col_dual[col] = 0.0;
    if !basis.valid {
        return;
    }
    basis.row_status[row] = match col_status {
        Status::Lower => {
            if coef > 0.0 {
                Status::Lower
            } else {
                Status::Upper
            }
        }
        _ => {
            if coef > 0.0 {
                Status::Upper
            } else {
                Status::Lower
            }
        }
    };
    basis.col_status[col] = Status::Basic;
}

fn undo_fixed_col(fix_value: f64, col_cost: f64, col: usize, fix_type: Status, col_values: &[Nonzero], solution: &mut Solution, basis: &mut Basis) {
    solution.col_value[col] = fix_value;
    if !solution.dual_valid {
        return;
    }
    let mut reduced_cost = CDouble::new(col_cost);
    for nz in col_values {
        reduced_cost -= nz.value * solution.row_dual[nz.index];
    }
    solution.col_dual[col] = reduced_cost.value();
    if basis.valid {
        basis.col_status[col] = fix_type;
        if basis.col_status[col] == Status::Nonbasic {
            basis.col_status[col] = if solution.col_dual[col] >= 0.0 { Status::Lower } else { Status::Upper };
        }
    }
}

fn undo_forcing_row(row: usize, row_type: RowType, row_values: &[Nonzero], solution: &mut Solution, basis: &mut Basis) {
    if !solution.dual_valid {
        return;
    }
    let mut basic_col = NONE;
    let mut dual_delta = 0.0;
    let direction = if row_type == RowType::Leq { 1.0 } else { -1.0 };
    for nz in row_values {
        let col_dual = solution.col_dual[nz.index] - nz.value * dual_delta;
        if direction * col_dual * nz.value < 0.0 {
            dual_delta = solution.col_dual[nz.index] / nz.value;
            basic_col = nz.index;
        }
    }
    if basic_col != NONE {
        solution.row_dual[row] += dual_delta;
        for nz in row_values {
            solution.col_dual[nz.index] = (CDouble::new(solution.col_dual[nz.index]) - CDouble::new(dual_delta) * nz.value).value();
        }
        solution.col_dual[basic_col] = 0.0;
        if basis.valid {
            basis.row_status[row] = if row_type == RowType::Geq { Status::Lower } else { Status::Upper };
            basis.col_status[basic_col] = Status::Basic;
        }
    }
}

fn undo_forcing_column(col_bound: f64, col: usize, at_infinite_upper: bool, col_values: &[Nonzero], solution: &mut Solution, basis: &mut Basis) {
    let mut nonbasic_row = NONE;
    let mut nonbasic_row_status = Status::Nonbasic;
    let mut col_val_from_nonbasic_row = col_bound;
    let direction = if at_infinite_upper { 1.0 } else { -1.0 };
    for nz in col_values {
        let col_val_from_row = solution.row_value[nz.index] / nz.value;
        if direction * col_val_from_row > direction * col_val_from_nonbasic_row {
            nonbasic_row = nz.index;
            col_val_from_nonbasic_row = col_val_from_row;
            nonbasic_row_status = if direction * nz.value > 0.0 { Status::Lower } else { Status::Upper };
        }
    }
    solution.col_value[col] = col_val_from_nonbasic_row;
    if !solution.dual_valid {
        return;
    }
    solution.col_dual[col] = 0.0;
    if !basis.valid {
        return;
    }
    if nonbasic_row == NONE {
        basis.col_status[col] = if at_infinite_upper { Status::Lower } else { Status::Upper };
    } else {
        basis.col_status[col] = Status::Basic;
        basis.row_status[nonbasic_row] = nonbasic_row_status;
    }
}

#[allow(clippy::too_many_arguments)]
fn undo_duplicate_row(
    tol: Tolerances,
    duplicate_row_scale: f64,
    duplicate_row: usize,
    row: usize,
    row_lower_tightened: bool,
    row_upper_tightened: bool,
    solution: &mut Solution,
    basis: &mut Basis,
) {
    if !solution.dual_valid {
        return;
    }
    if !row_upper_tightened && !row_lower_tightened {
        solution.row_dual[duplicate_row] = 0.0;
        if basis.valid {
            basis.row_status[duplicate_row] = Status::Basic;
        }
        return;
    }
    let row_status = if !basis.valid {
        compute_status(solution.row_dual[row], tol.dual_feasibility)
    } else {
        compute_status_in(solution.row_dual[row], &mut basis.row_status[row], tol.dual_feasibility)
    };
    let compute_row_dual_and_status = |tightened: bool, solution: &mut Solution, basis: &mut Basis| {
        if tightened {
            solution.row_dual[duplicate_row] = solution.row_dual[row] / duplicate_row_scale;
            if basis.valid {
                basis.row_status[duplicate_row] = if duplicate_row_scale > 0.0 { Status::Upper } else { Status::Lower };
            }
            solution.row_dual[row] = 0.0;
            if basis.valid {
                basis.row_status[row] = Status::Basic;
            }
        } else {
            solution.row_dual[duplicate_row] = 0.0;
            if basis.valid {
                basis.row_status[duplicate_row] = Status::Basic;
            }
        }
    };
    match row_status {
        Status::Basic => {
            solution.row_dual[duplicate_row] = 0.0;
            if basis.valid {
                basis.row_status[duplicate_row] = Status::Basic;
            }
        }
        Status::Upper => compute_row_dual_and_status(row_upper_tightened, solution, basis),
        Status::Lower => compute_row_dual_and_status(row_lower_tightened, solution, basis),
        _ => {}
    }
}

impl DuplicateColumn {
    fn undo(&self, tol: Tolerances, solution: &mut Solution, basis: &mut Basis) {
        let &DuplicateColumn {
            col_scale,
            col_lower,
            col_upper,
            duplicate_col_lower,
            duplicate_col_upper,
            col,
            duplicate_col,
        } = self;
        let merge_val = solution.col_value[col];
        let ok_residual = |x: f64, y: f64| (x + col_scale * y - merge_val).abs() <= tol.primal_feasibility;
        let is_at_bound = |value: f64, bound: f64| {
            if value < bound - tol.primal_feasibility {
                return false;
            }
            value <= bound + tol.primal_feasibility
        };
        if solution.dual_valid {
            solution.col_dual[duplicate_col] = solution.col_dual[col] * col_scale;
        }
        if basis.valid {
            match basis.col_status[col] {
                Status::Lower => {
                    solution.col_value[col] = col_lower;
                    if col_scale > 0.0 {
                        basis.col_status[duplicate_col] = Status::Lower;
                        solution.col_value[duplicate_col] = duplicate_col_lower;
                    } else {
                        basis.col_status[duplicate_col] = Status::Upper;
                        solution.col_value[duplicate_col] = duplicate_col_upper;
                    }
                    return;
                }
                Status::Upper => {
                    solution.col_value[col] = col_upper;
                    if col_scale > 0.0 {
                        basis.col_status[duplicate_col] = Status::Upper;
                        solution.col_value[duplicate_col] = duplicate_col_upper;
                    } else {
                        basis.col_status[duplicate_col] = Status::Lower;
                        solution.col_value[duplicate_col] = duplicate_col_lower;
                    }
                    return;
                }
                Status::Zero => {
                    solution.col_value[col] = 0.0;
                    basis.col_status[duplicate_col] = Status::Zero;
                    solution.col_value[duplicate_col] = 0.0;
                    return;
                }
                Status::Basic | Status::Nonbasic => {}
            }
        }
        solution.col_value[col] = if col_lower != f64::NEG_INFINITY { col_lower } else { 0.0f64.min(col_upper) };
        solution.col_value[duplicate_col] = ((CDouble::new(merge_val) - solution.col_value[col]) / col_scale).value();
        let mut recompute_col = false;
        if basis.valid {
            basis.col_status[duplicate_col] = Status::Nonbasic;
        }
        if solution.col_value[duplicate_col] > duplicate_col_upper {
            solution.col_value[duplicate_col] = duplicate_col_upper;
            recompute_col = true;
            if basis.valid {
                basis.col_status[duplicate_col] = Status::Upper;
            }
        } else if solution.col_value[duplicate_col] < duplicate_col_lower {
            solution.col_value[duplicate_col] = duplicate_col_lower;
            recompute_col = true;
            if basis.valid {
                basis.col_status[duplicate_col] = Status::Lower;
            }
        }
        if recompute_col {
            solution.col_value[col] = merge_val - col_scale * solution.col_value[duplicate_col];
        } else if basis.valid {
            basis.col_status[duplicate_col] = basis.col_status[col];
            if col_lower != f64::NEG_INFINITY {
                basis.col_status[col] = Status::Lower;
            } else if col_upper <= 0.0 {
                basis.col_status[col] = Status::Upper;
            } else {
                basis.col_status[col] = Status::Zero;
            }
        }
        let has_error = |solution: &Solution| {
            solution.col_value[duplicate_col] < duplicate_col_lower - MIP_FEASIBILITY_TOLERANCE
                || solution.col_value[duplicate_col] > duplicate_col_upper + MIP_FEASIBILITY_TOLERANCE
                || solution.col_value[col] < col_lower - MIP_FEASIBILITY_TOLERANCE
                || solution.col_value[col] > col_upper + MIP_FEASIBILITY_TOLERANCE
                || !ok_residual(solution.col_value[col], solution.col_value[duplicate_col])
        };
        if !has_error(solution) {
            return;
        }
        self.undo_fix(tol, solution, merge_val);
        if basis.valid {
            let mut duplicate_col_basic = false;
            if duplicate_col_lower == f64::NEG_INFINITY && duplicate_col_upper == f64::INFINITY {
                if solution.col_value[duplicate_col] == 0.0 {
                    basis.col_status[col] = Status::Basic;
                    basis.col_status[duplicate_col] = Status::Zero;
                } else {
                    duplicate_col_basic = true;
                }
            } else if is_at_bound(solution.col_value[duplicate_col], duplicate_col_lower) {
                basis.col_status[col] = Status::Basic;
                basis.col_status[duplicate_col] = Status::Lower;
            } else if is_at_bound(solution.col_value[duplicate_col], duplicate_col_upper) {
                basis.col_status[col] = Status::Basic;
                basis.col_status[duplicate_col] = Status::Upper;
            } else {
                duplicate_col_basic = true;
            }
            if duplicate_col_basic {
                basis.col_status[duplicate_col] = Status::Basic;
                basis.col_status[col] = if is_at_bound(solution.col_value[col], col_lower) {
                    Status::Lower
                } else if is_at_bound(solution.col_value[col], col_upper) {
                    Status::Upper
                } else {
                    Status::Nonbasic
                };
            }
        }
    }

    /// HiGHS's `undoFix` for two continuous columns: choose values of
    /// both within their bounds that give the merged value.
    fn undo_fix(&self, tol: Tolerances, solution: &mut Solution, merge_value: f64) {
        let ptol = tol.primal_feasibility;
        let is_feasible = |l: f64, v: f64, u: f64| v >= l - ptol && v <= u + ptol;
        let scale = self.col_scale;
        let (x_lo, x_up) = (self.col_lower, self.col_upper);
        let (y_lo, y_up) = (self.duplicate_col_lower, self.duplicate_col_upper);
        let compute_value = |value: f64| ((CDouble::new(merge_value) - value) / scale).value();
        let compute_inv_value = |value: f64| (CDouble::new(merge_value) - CDouble::new(value) * scale).value();
        let mut x_v = merge_value;
        let mut y_v = f64::NEG_INFINITY;
        let set_x_value = |value: f64, x_v: &mut f64, y_v: &mut f64| {
            *x_v = value;
            *y_v = compute_value(value);
        };
        let set_y_value = |value: f64, x_v: &mut f64, y_v: &mut f64| {
            *y_v = value;
            *x_v = compute_inv_value(value);
        };
        if scale > 0.0 {
            if y_up < f64::INFINITY {
                set_y_value(y_up, &mut x_v, &mut y_v);
                if x_v < x_lo - ptol {
                    set_x_value(x_lo, &mut x_v, &mut y_v);
                    if y_v < y_lo - ptol {
                        set_x_value(x_lo - ptol, &mut x_v, &mut y_v);
                    }
                }
            } else if y_lo > f64::NEG_INFINITY {
                set_y_value(y_lo, &mut x_v, &mut y_v);
                if x_v > x_up + ptol {
                    set_x_value(x_up, &mut x_v, &mut y_v);
                    if y_v > y_up + ptol {
                        set_x_value(x_up + ptol, &mut x_v, &mut y_v);
                    }
                }
            } else {
                set_x_value(0.0f64.max(x_lo), &mut x_v, &mut y_v);
            }
        } else if y_lo > f64::NEG_INFINITY {
            set_y_value(y_lo, &mut x_v, &mut y_v);
            if x_v < x_lo - ptol {
                set_x_value(x_lo, &mut x_v, &mut y_v);
                if y_v > y_up + ptol {
                    set_x_value(x_lo - ptol, &mut x_v, &mut y_v);
                }
            }
        } else if y_up < f64::INFINITY {
            set_y_value(y_up, &mut x_v, &mut y_v);
            if x_v > x_up + ptol {
                set_x_value(x_up, &mut x_v, &mut y_v);
                if y_v < y_lo - ptol {
                    set_x_value(x_up + ptol, &mut x_v, &mut y_v);
                }
            }
        } else {
            set_x_value(0.0f64.max(x_lo), &mut x_v, &mut y_v);
        }
        let residual = (CDouble::new(x_v) - compute_inv_value(y_v)).value().abs();
        let x_y_ok = is_feasible(x_lo, x_v, x_up) && is_feasible(y_lo, y_v, y_up) && x_v.abs() < f64::INFINITY && y_v.abs() < f64::INFINITY && residual <= 1e-12;
        if x_y_ok {
            solution.col_value[self.col] = x_v;
            solution.col_value[self.duplicate_col] = y_v;
        }
    }
}

/// HiGHS's default `mip_feasibility_tolerance`, which postsolve uses for
/// the bounds of merged columns even in an LP.
const MIP_FEASIBILITY_TOLERANCE: f64 = 1e-6;
