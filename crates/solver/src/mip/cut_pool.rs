//! HiGHS's `HighsCutPool`: the cuts found so far, aged out when unused,
//! separated into the LP when violated, and propagated by the domains.
//!
//! HiGHS runs one cut pool per worker; solver runs one worker, so there is
//! one pool and HiGHS's checks against the global pool from a worker's own
//! pool never apply. Extracting cliques from a cut of the global pool is
//! left to the caller of [`CutPool::add_cut`], after the cut is in.

use std::collections::{BTreeSet, HashMap};

use super::{change_log::ChangeLog, dynamic_row_matrix::DynamicRowMatrix};
use crate::{
    hash_table::{hash_u64, vector_hash_u32},
    util::{CDouble, hash::double_hash_code},
};

/// A change to the pool, for the domains propagating its cuts.
#[derive(Clone, Debug)]
pub(crate) enum CutEvent {
    /// `cut` was added; the domain computes its activity (and, when it
    /// propagates, marks it) from this copy of the row.
    Added {
        cut: usize,
        propagate: bool,
        index: Vec<usize>,
        value: Vec<f64>,
        rhs: f64,
    },
    Deleted {
        cut: usize,
        only_for_propagation: bool,
    },
}

/// HiGHS's `HighsCutSet`: cuts chosen for the LP, row-wise.
#[derive(Clone, Debug, Default)]
pub(crate) struct CutSet {
    pub cut_indices: Vec<usize>,
    pub ar_start: Vec<usize>,
    pub ar_index: Vec<usize>,
    pub ar_value: Vec<f64>,
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
}

impl CutSet {
    pub fn num_cuts(&self) -> usize {
        self.cut_indices.len()
    }

    pub fn resize(&mut self, nnz: usize) {
        let ncuts = self.num_cuts();
        self.lower.resize(ncuts, f64::NEG_INFINITY);
        self.upper.resize(ncuts, 0.0);
        self.ar_start.resize(ncuts + 1, 0);
        self.ar_index.resize(nnz, 0);
        self.ar_value.resize(nnz, 0.0);
    }

    pub fn clear(&mut self) {
        self.cut_indices.clear();
        self.upper.clear();
        self.ar_start.clear();
        self.ar_index.clear();
        self.ar_value.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.cut_indices.is_empty()
    }
}

/// HiGHS's `compute_cut_hash`.
fn compute_cut_hash(index: &[usize], value: &[f64], maxabscoef: f64) -> u64 {
    let scale = 1.0 / maxabscoef;
    let value_hash_codes = value.iter().map(|&value| double_hash_code(scale * value));
    vector_hash_u32(index.iter().map(|&index| index as u32)) ^ (vector_hash_u32(value_hash_codes) >> 32)
}

#[derive(Clone, Debug)]
pub(crate) struct CutPool {
    matrix: DynamicRowMatrix,
    rhs: Vec<f64>,
    ages: Vec<i16>,
    num_lps: Vec<i16>,
    row_normalization: Vec<f64>,
    max_abs_coef: Vec<f64>,
    row_integral: Vec<bool>,
    hash_to_cut: HashMap<u64, Vec<usize>>,
    /// The propagating cuts as `(age, cut)`.
    prop_rows: BTreeSet<(i16, usize)>,
    best_observed_score: f64,
    min_score_factor: f64,
    min_density_lim: f64,
    agelim: usize,
    softlimit: usize,
    num_lp_cuts: usize,
    num_prop_nzs: usize,
    num_prop_rows: usize,
    age_distribution: Vec<usize>,
    pub log: ChangeLog<CutEvent>,
}

impl CutPool {
    pub fn new(num_col: usize, agelim: usize, softlimit: usize) -> Self {
        Self {
            matrix: DynamicRowMatrix::new(num_col),
            rhs: Vec::new(),
            ages: Vec::new(),
            num_lps: Vec::new(),
            row_normalization: Vec::new(),
            max_abs_coef: Vec::new(),
            row_integral: Vec::new(),
            hash_to_cut: HashMap::new(),
            prop_rows: BTreeSet::new(),
            best_observed_score: 0.0,
            min_score_factor: 0.9,
            min_density_lim: 0.1 * num_col as f64,
            agelim,
            softlimit,
            num_lp_cuts: 0,
            num_prop_nzs: 0,
            num_prop_rows: 0,
            age_distribution: vec![0; agelim + 1],
            log: ChangeLog::default(),
        }
    }

    pub fn matrix(&self) -> &DynamicRowMatrix {
        &self.matrix
    }

    pub fn rhs(&self) -> &[f64] {
        &self.rhs
    }

    pub fn cut(&self, cut: usize) -> (&[usize], &[f64]) {
        self.matrix.row(cut)
    }

    pub fn row_length(&self, row: usize) -> usize {
        self.matrix.row_end(row) - self.matrix.row_start(row)
    }

    pub fn cut_is_integral(&self, cut: usize) -> bool {
        self.row_integral[cut]
    }

    pub fn num_cuts(&self) -> usize {
        self.matrix.num_rows() - self.matrix.num_deleted_rows()
    }

    pub fn num_available_cuts(&self) -> usize {
        self.num_cuts() - self.num_lp_cuts
    }

    pub fn max_abs_cut_coef(&self, cut: usize) -> f64 {
        self.max_abs_coef[cut]
    }

    pub fn row_normalization(&self, cut: usize) -> f64 {
        self.row_normalization[cut]
    }

    pub fn set_age_limit(&mut self, agelim: usize) {
        self.agelim = agelim;
        self.age_distribution.resize(agelim + 1, 0);
    }

    pub fn increase_num_lps(&mut self, cut: usize, n: i16) {
        self.num_lps[cut] += n;
    }

    pub fn reset_age(&mut self, cut: usize) {
        if self.ages[cut] > 0 {
            if self.matrix.columns_linked(cut) {
                self.prop_rows.remove(&(self.ages[cut], cut));
                self.prop_rows.insert((0, cut));
            }
            self.age_distribution[self.ages[cut] as usize] -= 1;
            self.age_distribution[0] += 1;
            self.ages[cut] = 0;
        }
    }

    fn is_duplicate(&self, hash: u64, norm: f64, index: &[usize], value: &[f64]) -> bool {
        let Some(rows) = self.hash_to_cut.get(&hash) else {
            return false;
        };
        let ar_value = self.matrix.ar_value();
        let ar_index = self.matrix.ar_index();
        for &row in rows {
            let start = self.matrix.row_start(row);
            let end = self.matrix.row_end(row);
            if end - start != index.len() {
                continue;
            }
            if index == &ar_index[start..end] {
                let mut dotprod = 0.0;
                for i in 0..index.len() {
                    dotprod += value[i] * ar_value[start + i];
                }
                let parallelism = dotprod * self.row_normalization[row] * norm;
                if parallelism >= 1.0 - 1e-6 {
                    return true;
                }
            }
        }
        false
    }

    /// HiGHS's `getParallelism` of two cuts of this pool.
    pub fn parallelism(&self, row1: usize, row2: usize) -> f64 {
        let mut i1 = self.matrix.row_start(row1);
        let end1 = self.matrix.row_end(row1);
        let mut i2 = self.matrix.row_start(row2);
        let end2 = self.matrix.row_end(row2);
        let ar_index = self.matrix.ar_index();
        let ar_value = self.matrix.ar_value();
        let mut dotprod = 0.0;
        while i1 != end1 && i2 != end2 {
            let col1 = ar_index[i1];
            let col2 = ar_index[i2];
            if col1 < col2 {
                i1 += 1;
            } else if col2 < col1 {
                i2 += 1;
            } else {
                dotprod += ar_value[i1] * ar_value[i2];
                i1 += 1;
                i2 += 1;
            }
        }
        dotprod * self.row_normalization[row1] * self.row_normalization[row2]
    }

    pub fn lp_cut_removed(&mut self, cut: usize) {
        let n = self.num_lps[cut];
        self.num_lps[cut] -= 1;
        if n > 1 {
            return;
        }
        if self.matrix.columns_linked(cut) {
            self.prop_rows.remove(&(self.ages[cut], cut));
            self.prop_rows.insert((1, cut));
        }
        self.ages[cut] = 1;
        self.num_lp_cuts -= 1;
        self.age_distribution[1] += 1;
    }

    fn delete_cut(&mut self, cut: usize, is_propagated: bool) {
        self.log.push(CutEvent::Deleted { cut, only_for_propagation: false });
        if is_propagated {
            self.num_prop_rows -= 1;
            self.num_prop_nzs -= self.row_length(cut);
        }
        self.matrix.remove_row(cut);
        self.ages[cut] = -1;
        self.rhs[cut] = f64::INFINITY;
    }

    pub fn perform_aging(&mut self) {
        let cut_index_end = self.matrix.num_rows();
        let mut agelim = self.agelim;
        let mut num_available_cuts = self.num_available_cuts() as isize;
        while agelim > 5 && num_available_cuts > self.softlimit as isize {
            num_available_cuts -= self.age_distribution[agelim] as isize;
            agelim -= 1;
        }
        for i in 0..cut_index_end {
            if self.num_lps[i] > 0 && self.ages[i] >= 0 {
                self.age_distribution[self.ages[i] as usize] -= 1;
                if self.matrix.columns_linked(i) {
                    self.prop_rows.remove(&(self.ages[i], i));
                    self.prop_rows.insert((-1, i));
                }
                self.ages[i] = -1;
                self.num_lp_cuts += 1;
            } else if self.num_lps[i] == 0 && self.ages[i] == -1 && self.rhs[i] != f64::INFINITY {
                if self.matrix.columns_linked(i) {
                    self.prop_rows.remove(&(self.ages[i], i));
                    self.prop_rows.insert((1, i));
                }
                self.ages[i] = 1;
                self.num_lp_cuts -= 1;
                self.age_distribution[1] += 1;
                continue;
            }
            if self.ages[i] < 0 {
                continue;
            }
            let is_propagated = self.matrix.columns_linked(i);
            if is_propagated {
                self.prop_rows.remove(&(self.ages[i], i));
            }
            self.age_distribution[self.ages[i] as usize] -= 1;
            self.ages[i] += 1;
            if self.ages[i] as usize > agelim {
                self.delete_cut(i, is_propagated);
            } else {
                if is_propagated {
                    self.prop_rows.insert((self.ages[i], i));
                }
                self.age_distribution[self.ages[i] as usize] += 1;
            }
        }
    }

    /// HiGHS's `separate`: the pool's cuts `sol` violates, best first and
    /// not too parallel to one another or to those already in `cutset`,
    /// appended to `cutset`. `col_lower`/`col_upper` are the domain's
    /// bounds.
    pub fn separate(&mut self, sol: &[f64], col_lower: &[f64], col_upper: &[f64], cutset: &mut CutSet, feastol: f64) {
        let nrows = self.matrix.num_rows();
        let mut efficacious_cuts: Vec<(f64, usize)> = Vec::new();
        let mut agelim = self.agelim;
        let mut num_cuts = self.num_cuts() as isize - self.num_lp_cuts as isize;
        while agelim > 1 && num_cuts > self.softlimit as isize {
            num_cuts -= self.age_distribution[agelim] as isize;
            agelim -= 1;
        }
        for i in 0..nrows {
            if self.ages[i] < 0 {
                continue;
            }
            let start = self.matrix.row_start(i);
            let end = self.matrix.row_end(i);
            let mut viol = -self.rhs[i];
            {
                let ar_index = self.matrix.ar_index();
                let ar_value = self.matrix.ar_value();
                for j in start..end {
                    viol += ar_value[j] * sol[ar_index[j]];
                }
            }
            let is_propagated = self.matrix.columns_linked(i);
            self.age_distribution[self.ages[i] as usize] -= 1;
            if is_propagated {
                self.prop_rows.remove(&(self.ages[i], i));
            }
            if viol <= feastol {
                self.ages[i] += 1;
                if self.ages[i] as usize >= agelim {
                    let hash = {
                        let (index, value) = self.matrix.row(i);
                        compute_cut_hash(index, value, self.max_abs_coef[i])
                    };
                    self.delete_cut(i, is_propagated);
                    if let Some(rows) = self.hash_to_cut.get_mut(&hash)
                        && let Some(at) = rows.iter().position(|&row| row == i)
                    {
                        rows.remove(at);
                        if rows.is_empty() {
                            self.hash_to_cut.remove(&hash);
                        }
                    }
                } else {
                    if is_propagated {
                        self.prop_rows.insert((self.ages[i], i));
                    }
                    self.age_distribution[self.ages[i] as usize] += 1;
                }
                continue;
            }
            // The norm counts only entries not at their bound in `sol`, so
            // a stronger cut is not judged weaker for its extra entries.
            let mut rownorm = CDouble::new(0.0);
            let mut num_active_nzs = 0usize;
            {
                let ar_index = self.matrix.ar_index();
                let ar_value = self.matrix.ar_value();
                for j in start..end {
                    let col = ar_index[j];
                    let solval = sol[col];
                    if ar_value[j] > 0.0 {
                        if solval > col_lower[col] + feastol {
                            rownorm += ar_value[j] * ar_value[j];
                            num_active_nzs += 1;
                        }
                    } else if solval < col_upper[col] - feastol {
                        rownorm += ar_value[j] * ar_value[j];
                        num_active_nzs += 1;
                    }
                }
            }
            self.ages[i] = 0;
            self.age_distribution[0] += 1;
            if is_propagated {
                self.prop_rows.insert((0, i));
            }
            let score = viol / (num_active_nzs as f64 * rownorm.value().sqrt());
            efficacious_cuts.push((score, i));
        }
        if efficacious_cuts.is_empty() {
            return;
        }
        let num = efficacious_cuts.len() as u64;
        let tie = |cut: usize| (hash_u64(((cut as u64) << 32).wrapping_add(num)), cut);
        efficacious_cuts.sort_by(|a, b| {
            if a.0 > b.0 {
                return std::cmp::Ordering::Less;
            }
            if a.0 < b.0 {
                return std::cmp::Ordering::Greater;
            }
            tie(b.1).cmp(&tie(a.1))
        });
        self.best_observed_score = super::cmax(efficacious_cuts[0].0, self.best_observed_score);
        let min_score = self.min_score_factor * self.best_observed_score;
        let mut numefficacious = efficacious_cuts.partition_point(|cut| !(min_score > cut.0));
        let lower_threshold = efficacious_cuts.len() / 20;
        let upper_threshold = efficacious_cuts.len() - 1;
        if numefficacious <= lower_threshold {
            numefficacious = (efficacious_cuts.len() / 2).max(1);
            self.min_score_factor = efficacious_cuts[numefficacious - 1].0 / self.best_observed_score;
        } else if numefficacious > upper_threshold {
            self.min_score_factor = efficacious_cuts[upper_threshold].0 / self.best_observed_score;
        }
        efficacious_cuts.truncate(numefficacious);
        let orignumcuts = cutset.num_cuts();
        let origselectednnz = cutset.ar_index.len();
        let mut selectednnz = origselectednnz;
        for &(_, cut) in &efficacious_cuts {
            let maxpar = 0.1;
            if cutset.cut_indices.iter().any(|&other| self.parallelism(other, cut) > maxpar) {
                continue;
            }
            self.num_lps[cut] += 1;
            self.age_distribution[self.ages[cut] as usize] -= 1;
            self.num_lp_cuts += 1;
            if self.matrix.columns_linked(cut) {
                self.prop_rows.remove(&(self.ages[cut], cut));
                self.prop_rows.insert((-1, cut));
            }
            self.ages[cut] = -1;
            cutset.cut_indices.push(cut);
            selectednnz += self.row_length(cut);
        }
        cutset.resize(selectednnz);
        let mut offset = origselectednnz;
        for i in orignumcuts..cutset.num_cuts() {
            cutset.ar_start[i] = offset;
            let cut = cutset.cut_indices[i];
            cutset.upper[i] = self.rhs[cut];
            let (index, value) = self.matrix.row(cut);
            for (&col, &val) in index.iter().zip(value) {
                cutset.ar_value[offset] = val;
                cutset.ar_index[offset] = col;
                offset += 1;
            }
        }
        let num_cuts = cutset.num_cuts();
        cutset.ar_start[num_cuts] = offset;
    }

    /// HiGHS's `separateLpCutsAfterRestart`: every cut, into the LP.
    pub fn separate_lp_cuts_after_restart(&mut self, cutset: &mut CutSet) {
        let numcuts = self.matrix.num_rows();
        cutset.cut_indices = (0..numcuts).collect();
        cutset.resize(self.matrix.nonzero_capacity());
        let mut offset = 0;
        for i in 0..cutset.num_cuts() {
            self.age_distribution[self.ages[i] as usize] -= 1;
            self.num_lp_cuts += 1;
            if self.matrix.columns_linked(i) {
                self.prop_rows.remove(&(self.ages[i], i));
                self.prop_rows.insert((-1, i));
            }
            self.num_lps[i] = 1;
            self.ages[i] = -1;
            cutset.ar_start[i] = offset;
            let cut = cutset.cut_indices[i];
            cutset.upper[i] = self.rhs[cut];
            let (index, value) = self.matrix.row(cut);
            for (&col, &val) in index.iter().zip(value) {
                cutset.ar_value[offset] = val;
                cutset.ar_index[offset] = col;
                offset += 1;
            }
        }
        let num_cuts = cutset.num_cuts();
        cutset.ar_start[num_cuts] = offset;
    }

    /// HiGHS's `addCut`: the cut `index`/`value` (sorted here by column)
    /// `<= rhs`, unless the pool has it already; returns its index.
    /// `num_nonzero` and `num_row` are the model's.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cut(
        &mut self,
        index: &mut [usize],
        value: &mut [f64],
        rhs: f64,
        integral: bool,
        mut propagate: bool,
        is_conflict: bool,
        num_nonzero: usize,
        num_row: usize,
    ) -> Option<usize> {
        let len = index.len();
        let mut norm = 0.0;
        let mut maxabscoef = 0.0f64;
        let mut sort_buffer: Vec<(usize, f64)> = Vec::with_capacity(len);
        for i in 0..len {
            norm += value[i] * value[i];
            maxabscoef = super::cmax(maxabscoef, value[i].abs());
            sort_buffer.push((index[i], value[i]));
        }
        sort_buffer.sort_unstable_by_key(|entry| entry.0);
        for (i, (col, val)) in sort_buffer.into_iter().enumerate() {
            index[i] = col;
            value[i] = val;
        }
        let hash = compute_cut_hash(index, value, maxabscoef);
        let normalization = 1.0 / norm.sqrt();
        if self.is_duplicate(hash, normalization, index, value) {
            return None;
        }
        if propagate {
            let new_prop_nzs = self.num_prop_nzs + len;
            let avg_model_nzs = num_nonzero as f64 / num_row as f64;
            let new_avg_prop_nzs = new_prop_nzs as f64 / (self.num_prop_rows + 1) as f64;
            let alpha = 2.0;
            if is_conflict {
                // Conflicts may raise the average density more.
                if new_avg_prop_nzs > super::cmax(alpha * avg_model_nzs, self.min_density_lim) {
                    propagate = false;
                } else {
                    self.num_prop_rows += 1;
                    self.num_prop_nzs = new_prop_nzs;
                }
            } else if len as f64 >= super::cmax(alpha * avg_model_nzs, self.min_density_lim) {
                propagate = false;
            } else {
                self.num_prop_rows += 1;
                self.num_prop_nzs = new_prop_nzs;
            }
        }
        // Past twice the model's nonzeros in propagating cuts, the oldest
        // stop propagating.
        let mut prop_row_excess_nzs = self.num_prop_nzs as isize - 2 * num_nonzero as isize;
        if prop_row_excess_nzs > 0 {
            let mut unlinked = Vec::new();
            for &(age, row) in self.prop_rows.iter().rev() {
                if prop_row_excess_nzs <= 0 {
                    break;
                }
                let len = self.row_length(row);
                prop_row_excess_nzs -= len as isize;
                self.num_prop_nzs -= len;
                self.num_prop_rows -= 1;
                unlinked.push((age, row));
            }
            for &(age, row) in &unlinked {
                self.matrix.unlink_columns(row);
                self.log.push(CutEvent::Deleted {
                    cut: row,
                    only_for_propagation: true,
                });
                self.prop_rows.remove(&(age, row));
            }
        }
        let row = self.matrix.add_row(index, value, propagate);
        self.hash_to_cut.entry(hash).or_default().push(row);
        if row == self.rhs.len() {
            self.rhs.push(0.0);
            self.ages.push(0);
            self.num_lps.push(0);
            self.row_normalization.push(0.0);
            self.max_abs_coef.push(0.0);
            self.row_integral.push(false);
        }
        self.rhs[row] = rhs;
        self.ages[row] = (self.agelim as i16 - 5).max(0);
        self.age_distribution[self.ages[row] as usize] += 1;
        self.row_integral[row] = integral;
        self.num_lps[row] = 0;
        if propagate {
            self.prop_rows.insert((self.ages[row], row));
        }
        self.row_normalization[row] = normalization;
        self.max_abs_coef[row] = maxabscoef;
        self.log.push(CutEvent::Added {
            cut: row,
            propagate,
            index: index.to_vec(),
            value: value.to_vec(),
            rhs,
        });
        Some(row)
    }
}
