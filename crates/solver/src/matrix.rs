//! The constraint matrix as the simplex solver uses it: column-wise, and
//! row-wise partitioned into nonbasic then basic columns (HiGHS's
//! `HighsSparseMatrix`, the parts the simplex solver calls).

use crate::{
    factor::ColMatrix,
    hvector::{HVector, TINY, ZERO},
    util::CDouble,
};

/// Row-wise PRICE switches to a dense result above this density.
pub(crate) const HYPER_PRICE_DENSITY: f64 = 0.1;

/// Column `j`'s rows are `index[start[j]..start[j + 1]]`. Columns at or
/// beyond `num_col` are logicals: unit columns, row `j - num_col`.
#[derive(Clone, Debug, Default)]
pub(crate) struct ColWise {
    pub num_col: usize,
    pub num_row: usize,
    pub start: Vec<usize>,
    pub index: Vec<usize>,
    pub value: Vec<f64>,
}

impl ColWise {
    /// HiGHS's column-wise `productTransposeQuad`: the nonzeros of
    /// `columnᵀ A` in column order, in compensated arithmetic.
    pub fn product_transpose_quad(&self, column: &HVector) -> (Vec<f64>, Vec<usize>) {
        let mut values = Vec::with_capacity(self.num_col);
        let mut index = Vec::with_capacity(self.num_col);
        for col in 0..self.num_col {
            let mut value = CDouble::new(0.0);
            for el in self.start[col]..self.start[col + 1] {
                value += column.array[self.index[el]] * self.value[el];
            }
            let abs = if value.value() < 0.0 { -value } else { value };
            if (abs - TINY).value() > 0.0 {
                values.push(value.value());
                index.push(col);
            }
        }
        (values, index)
    }

    pub fn as_factor_matrix(&self) -> ColMatrix<'_> {
        ColMatrix {
            start: &self.start,
            index: &self.index,
            value: &self.value,
        }
    }

    /// Add `multiplier` times column `col` (structural or logical) into
    /// `column`, keeping its index.
    pub fn collect_aj(&self, column: &mut HVector, col: usize, multiplier: f64) {
        let mut add = |row: usize, value: f64| {
            let value0 = column.array[row];
            let value1 = value0 + multiplier * value;
            if value0 == 0.0 {
                column.index[column.count as usize] = row;
                column.count += 1;
            }
            column.array[row] = if value1.abs() < TINY { ZERO } else { value1 };
        };
        if col < self.num_col {
            for el in self.start[col]..self.start[col + 1] {
                add(self.index[el], self.value[el]);
            }
        } else {
            add(col - self.num_col, 1.0);
        }
    }

    pub fn compute_dot(&self, array: &[f64], col: usize) -> f64 {
        if col < self.num_col {
            (self.start[col]..self.start[col + 1]).map(|el| array[self.index[el]] * self.value[el]).sum()
        } else {
            array[col - self.num_col]
        }
    }

    /// `result = columnᵀ A`, by columns.
    pub fn price_by_column(&self, quad_precision: bool, result: &mut HVector, column: &HVector) {
        result.count = 0;
        for col in 0..self.num_col {
            let value = if quad_precision {
                let mut quad = CDouble::new(0.0);
                for el in self.start[col]..self.start[col + 1] {
                    quad += column.array[self.index[el]] * self.value[el];
                }
                quad.value()
            } else {
                let mut value = 0.0;
                for el in self.start[col]..self.start[col + 1] {
                    value += column.array[self.index[el]] * self.value[el];
                }
                value
            };
            if value.abs() > TINY {
                result.array[col] = value;
                result.index[result.count as usize] = col;
                result.count += 1;
            }
        }
    }

    /// Scale column `col` and every row by the given factors.
    pub fn apply_scale(&mut self, col_scale: &[f64], row_scale: &[f64]) {
        for col in 0..self.num_col {
            for el in self.start[col]..self.start[col + 1] {
                self.value[el] *= col_scale[col] * row_scale[self.index[el]];
            }
        }
    }
}

/// The matrix row-wise, each row's entries for nonbasic columns first
/// (`start[i]..p_end[i]`) and basic ones after (`p_end[i]..start[i + 1]`).
#[derive(Clone, Debug, Default)]
pub(crate) struct RowPartitioned {
    pub num_col: usize,
    pub num_row: usize,
    pub start: Vec<usize>,
    pub p_end: Vec<usize>,
    pub index: Vec<usize>,
    pub value: Vec<f64>,
}

impl RowPartitioned {
    /// Build from `matrix`, with the columns flagged nonbasic first.
    pub fn create(matrix: &ColWise, nonbasic_flag: &[i8]) -> Self {
        let num_col = matrix.num_col;
        let num_row = matrix.num_row;
        let num_nz = matrix.index.len();
        let mut start = vec![0; num_row + 1];
        let mut p_end = vec![0; num_row];
        let mut end = vec![0; num_row];
        for col in 0..num_col {
            let counts = if nonbasic_flag[col] != 0 { &mut p_end } else { &mut end };
            for el in matrix.start[col]..matrix.start[col + 1] {
                counts[matrix.index[el]] += 1;
            }
        }
        for row in 0..num_row {
            start[row + 1] = start[row] + p_end[row] + end[row];
        }
        for row in 0..num_row {
            end[row] = start[row] + p_end[row];
            p_end[row] = start[row];
        }
        let mut index = vec![0; num_nz];
        let mut value = vec![0.0; num_nz];
        for col in 0..num_col {
            let next = if nonbasic_flag[col] != 0 { &mut p_end } else { &mut end };
            for el in matrix.start[col]..matrix.start[col + 1] {
                let row = matrix.index[el];
                let to = next[row];
                next[row] += 1;
                index[to] = col;
                value[to] = matrix.value[el];
            }
        }
        Self {
            num_col,
            num_row,
            start,
            p_end,
            index,
            value,
        }
    }

    /// Move `var_in` to the basic part and `var_out` to the nonbasic part.
    pub fn update(&mut self, var_in: usize, var_out: usize, matrix: &ColWise) {
        if var_in < self.num_col {
            for el in matrix.start[var_in]..matrix.start[var_in + 1] {
                let row = matrix.index[el];
                let mut find = self.start[row];
                self.p_end[row] -= 1;
                let swap = self.p_end[row];
                while self.index[find] != var_in {
                    find += 1;
                }
                self.index.swap(find, swap);
                self.value.swap(find, swap);
            }
        }
        if var_out < self.num_col {
            for el in matrix.start[var_out]..matrix.start[var_out + 1] {
                let row = matrix.index[el];
                let mut find = self.p_end[row];
                let swap = self.p_end[row];
                self.p_end[row] += 1;
                while self.index[find] != var_out {
                    find += 1;
                }
                self.index.swap(find, swap);
                self.value.swap(find, swap);
            }
        }
    }

    /// `result = columnᵀ A` over the nonbasic columns, row-wise and
    /// hyper-sparse, going dense once `result` passes `switch_density`.
    #[allow(clippy::mut_range_bound, reason = "the range is fixed; next_index records how far the loop got")]
    pub fn price_by_row_with_switch(&self, quad_precision: bool, result: &mut HVector, column: &HVector, expected_density: f64, from_index: usize, switch_density: f64) {
        let mut sum = SparseVectorSum::default();
        if quad_precision {
            sum.set_dimension(self.num_col);
        }
        let mut next_index = from_index;
        let column_count = column.count.max(0) as usize;
        if expected_density <= HYPER_PRICE_DENSITY {
            let inv_num_col = 1.0 / self.num_col as f64;
            for ix in next_index..column_count {
                let row = column.index[ix];
                let to_el = self.p_end[row];
                let row_num_nz = to_el - self.start[row];
                let local_density = result.count as f64 * inv_num_col;
                let switch_to_dense = result.count as usize + row_num_nz >= self.num_col || local_density > switch_density;
                if switch_to_dense {
                    break;
                }
                let multiplier = column.array[row];
                if multiplier != 0.0 {
                    if quad_precision {
                        for el in self.start[row]..to_el {
                            sum.add(self.index[el], multiplier * self.value[el]);
                        }
                    } else {
                        for el in self.start[row]..to_el {
                            let col = self.index[el];
                            let value0 = result.array[col];
                            let value1 = value0 + multiplier * self.value[el];
                            if value0 == 0.0 {
                                result.index[result.count as usize] = col;
                                result.count += 1;
                            }
                            result.array[col] = if value1.abs() < TINY { ZERO } else { value1 };
                        }
                    }
                }
                next_index = ix + 1;
            }
        }
        if quad_precision {
            sum.cleanup(|x| x.abs() <= TINY);
        }
        if next_index < column_count {
            if quad_precision {
                let mut result_array = sum.values.clone();
                self.price_by_row_dense_result_quad(&mut result_array, column, next_index);
                result.count = 0;
                for col in 0..self.num_col {
                    let value1 = result_array[col].value();
                    if value1.abs() < TINY {
                        result.array[col] = 0.0;
                    } else {
                        result.array[col] = value1;
                        result.index[result.count as usize] = col;
                        result.count += 1;
                    }
                }
            } else {
                self.price_by_row_dense_result(&mut result.array, column, next_index);
                result.count = 0;
                for col in 0..self.num_col {
                    if result.array[col].abs() < TINY {
                        result.array[col] = 0.0;
                    } else {
                        result.index[result.count as usize] = col;
                        result.count += 1;
                    }
                }
            }
        } else if quad_precision {
            result.count = sum.nonzero.len() as isize;
            for (k, &col) in sum.nonzero.iter().enumerate() {
                result.index[k] = col;
                result.array[col] = sum.values[col].value();
            }
        } else {
            result.tight();
        }
    }

    pub fn price_by_row(&self, quad_precision: bool, result: &mut HVector, column: &HVector) {
        self.price_by_row_with_switch(quad_precision, result, column, f64::NEG_INFINITY, 0, f64::INFINITY);
    }

    fn price_by_row_dense_result(&self, result: &mut [f64], column: &HVector, from_index: usize) {
        for ix in from_index..column.count as usize {
            let row = column.index[ix];
            let multiplier = column.array[row];
            for el in self.start[row]..self.p_end[row] {
                let col = self.index[el];
                let value1 = result[col] + multiplier * self.value[el];
                result[col] = if value1.abs() < TINY { ZERO } else { value1 };
            }
        }
    }

    fn price_by_row_dense_result_quad(&self, result: &mut [CDouble], column: &HVector, from_index: usize) {
        for ix in from_index..column.count as usize {
            let row = column.index[ix];
            let multiplier = column.array[row];
            for el in self.start[row]..self.p_end[row] {
                let col = self.index[el];
                let value1 = result[col] + multiplier * self.value[el];
                result[col] = if value1.value().abs() < TINY { CDouble::new(ZERO) } else { value1 };
            }
        }
    }

    /// The nonzeros of `columnᵀ A` over all columns, in compensated
    /// arithmetic.
    pub fn product_transpose_quad(&self, column: &HVector) -> (Vec<f64>, Vec<usize>) {
        let mut sum = SparseVectorSum::default();
        sum.set_dimension(self.num_col);
        for row in 0..self.num_row {
            let multiplier = column.array[row];
            for el in self.start[row]..self.start[row + 1] {
                sum.add(self.index[el], multiplier * self.value[el]);
            }
        }
        sum.cleanup(|x| x.abs() <= TINY);
        let values = sum.nonzero.iter().map(|&col| sum.values[col].value()).collect();
        (values, sum.nonzero)
    }

    pub fn max_abs_row_value(&self, row: usize) -> f64 {
        self.value[self.start[row]..self.start[row + 1]].iter().fold(-1.0f64, |max, value| max.max(value.abs()))
    }
}

/// HiGHS's `HighsSparseVectorSum`.
#[derive(Clone, Debug, Default)]
pub(crate) struct SparseVectorSum {
    pub values: Vec<CDouble>,
    pub nonzero: Vec<usize>,
}

impl SparseVectorSum {
    pub fn set_dimension(&mut self, dimension: usize) {
        self.values.resize(dimension, CDouble::default());
        self.nonzero.reserve(dimension);
    }

    pub fn add(&mut self, index: usize, value: f64) {
        if self.values[index].value() != 0.0 {
            self.values[index] += value;
        } else {
            self.values[index] = CDouble::new(value);
            self.nonzero.push(index);
        }
        if self.values[index].value() == 0.0 {
            self.values[index] = CDouble::new(f64::MIN_POSITIVE);
        }
    }

    pub fn cleanup(&mut self, is_zero: impl Fn(f64) -> bool) {
        let mut num_nz = self.nonzero.len();
        for i in (0..self.nonzero.len()).rev() {
            let pos = self.nonzero[i];
            if is_zero(self.values[pos].value()) {
                self.values[pos] = CDouble::default();
                num_nz -= 1;
                self.nonzero.swap(num_nz, i);
            }
        }
        self.nonzero.truncate(num_nz);
    }
}
