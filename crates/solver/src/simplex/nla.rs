//! HiGHS's `HSimplexNla`: the factor, used in the scaled space whatever
//! space the simplex solver works in. When the LP being solved is unscaled
//! but has scale factors, vectors are scaled on the way in and out.

use crate::{factor::Factor, hvector::HVector};

/// Column and row scale factors: the scaled matrix is `R A C`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scale {
    pub col: Vec<f64>,
    pub row: Vec<f64>,
}

/// Loop over a vector's indices when it has few enough, else densely.
const DENSITY_FOR_INDEXING: f64 = 0.4;

pub(crate) fn sparse_loop_style(count: isize, dim: usize) -> (bool, usize) {
    let use_indices = count >= 0 && (count as f64) < DENSITY_FOR_INDEXING * dim as f64;
    (use_indices, if use_indices { count as usize } else { dim })
}

#[derive(Clone, Default)]
pub(crate) struct Nla {
    pub factor: Factor,
    pub num_col: usize,
    pub num_row: usize,
    /// Present while the LP being solved is unscaled but has scaling.
    pub scale: Option<Scale>,
    pub build_synthetic_tick: f64,
}

impl Nla {
    pub fn variable_scale_factor(&self, var: usize) -> f64 {
        match &self.scale {
            None => 1.0,
            Some(scale) => {
                if var < self.num_col {
                    scale.col[var]
                } else {
                    1.0 / scale.row[var - self.num_col]
                }
            }
        }
    }

    pub fn basic_col_scale_factor(&self, row: usize, basic_index: &[usize]) -> f64 {
        if self.scale.is_none() {
            return 1.0;
        }
        self.variable_scale_factor(basic_index[row])
    }

    pub fn pivot_in_scaled_space(&self, aq: &HVector, variable_in: usize, row_out: usize, basic_index: &[usize]) -> f64 {
        aq.array[row_out] * self.variable_scale_factor(variable_in) / self.variable_scale_factor(basic_index[row_out])
    }

    pub fn apply_basis_matrix_row_scale(&self, rhs: &mut HVector) {
        let Some(scale) = &self.scale else { return };
        let (use_indices, to_entry) = sparse_loop_style(rhs.count, self.num_row);
        for entry in 0..to_entry {
            let row = if use_indices { rhs.index[entry] } else { entry };
            rhs.array[row] *= scale.row[row];
        }
    }

    pub fn apply_basis_matrix_col_scale(&self, rhs: &mut HVector, basic_index: &[usize]) {
        let Some(scale) = &self.scale else { return };
        let (use_indices, to_entry) = sparse_loop_style(rhs.count, self.num_row);
        for entry in 0..to_entry {
            let col = if use_indices { rhs.index[entry] } else { entry };
            let var = basic_index[col];
            if var < self.num_col {
                rhs.array[col] *= scale.col[var];
            } else {
                rhs.array[col] /= scale.row[var - self.num_col];
            }
        }
    }

    pub fn unapply_basis_matrix_row_scale(&self, rhs: &mut HVector) {
        let Some(scale) = &self.scale else { return };
        let (use_indices, to_entry) = sparse_loop_style(rhs.count, self.num_row);
        for entry in 0..to_entry {
            let row = if use_indices { rhs.index[entry] } else { entry };
            rhs.array[row] /= scale.row[row];
        }
    }

    pub fn ftran(&self, rhs: &mut HVector, expected_density: f64, basic_index: &[usize]) {
        self.apply_basis_matrix_row_scale(rhs);
        self.factor.ftran(rhs, expected_density);
        self.apply_basis_matrix_col_scale(rhs, basic_index);
    }

    pub fn btran(&self, rhs: &mut HVector, expected_density: f64, basic_index: &[usize]) {
        self.apply_basis_matrix_col_scale(rhs, basic_index);
        self.factor.btran(rhs, expected_density);
        self.apply_basis_matrix_row_scale(rhs);
    }

    pub fn ftran_in_scaled_space(&self, rhs: &mut HVector, expected_density: f64) {
        self.factor.ftran(rhs, expected_density);
    }

    pub fn btran_in_scaled_space(&self, rhs: &mut HVector, expected_density: f64) {
        self.factor.btran(rhs, expected_density);
    }

    pub fn row_ep_2norm_in_scaled_space(&self, row: usize, row_ep: &HVector, basic_index: &[usize]) -> f64 {
        let Some(scale) = &self.scale else {
            return row_ep.norm2();
        };
        let col_scale_value = self.basic_col_scale_factor(row, basic_index);
        let (use_indices, to_entry) = sparse_loop_style(row_ep.count, self.num_row);
        let mut norm = 0.0;
        for entry in 0..to_entry {
            let i = if use_indices { row_ep.index[entry] } else { entry };
            let value = row_ep.array[i] / (scale.row[i] * col_scale_value);
            norm += value * value;
        }
        norm
    }

    /// Bring the update's packed vectors into the scaled space.
    pub fn transform_for_update(&self, aq: &mut HVector, ep: &mut HVector, variable_in: usize, row_out: usize, basic_index: &[usize]) {
        if self.scale.is_none() {
            return;
        }
        let cq_scale_factor = self.variable_scale_factor(variable_in);
        for value in &mut aq.pack_value[..aq.pack_count] {
            *value *= cq_scale_factor;
        }
        aq.array[row_out] *= cq_scale_factor;
        let cp_scale_factor = self.basic_col_scale_factor(row_out, basic_index);
        aq.array[row_out] /= cp_scale_factor;
        for value in &mut ep.pack_value[..ep.pack_count] {
            *value /= cp_scale_factor;
        }
    }
}
