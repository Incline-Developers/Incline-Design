//! HiGHS's `HEkkDualRow`: the pivotal row of the dual simplex method and
//! its bound-flipping ratio test (CHUZC).

use std::collections::BTreeSet;

use super::ekk::Ekk;
use crate::hvector::HVector;

const INITIAL_TOTAL_CHANGE: f64 = 1e-12;
const INITIAL_REMAIN_THETA: f64 = 1e100;
const MAX_SELECT_THETA: f64 = 1e18;

#[derive(Clone, Debug, Default)]
pub(crate) struct DualRow {
    /// Nonbasic free variables.
    pub free_list: BTreeSet<usize>,
    pub pack_count: usize,
    pub pack_index: Vec<usize>,
    pub pack_value: Vec<f64>,
    pub computed_edge_weight: f64,
    pub work_delta: f64,
    pub work_alpha: f64,
    pub work_theta: f64,
    /// The entering variable, if any.
    pub work_pivot: Option<usize>,
    pub work_count: usize,
    pub work_data: Vec<(usize, f64)>,
    pub work_group: Vec<usize>,
}

impl DualRow {
    pub fn setup(&mut self, num_tot: usize) {
        self.pack_count = 0;
        self.pack_index = vec![0; num_tot];
        self.pack_value = vec![0.0; num_tot];
        self.work_count = 0;
        self.work_data = vec![(0, 0.0); num_tot];
        self.free_list.clear();
    }

    pub fn clear(&mut self) {
        self.pack_count = 0;
        self.work_count = 0;
    }

    /// Pack `row`'s nonzeros, their indices offset by `offset`.
    pub fn choose_makepack(&mut self, row: &HVector, offset: usize) {
        for &index in row.indices() {
            self.pack_index[self.pack_count] = index + offset;
            self.pack_value[self.pack_count] = row.array[index];
            self.pack_count += 1;
        }
    }

    /// The candidates for CHUZC, with the Harris bound on the step.
    pub fn choose_possible(&mut self, ekk: &Ekk) {
        let update_count = ekk.info.update_count;
        let ta = if update_count < 10 {
            1e-9
        } else if update_count < 20 {
            3e-8
        } else {
            1e-6
        };
        let td = ekk.options.dual_feasibility_tolerance;
        let move_out = if self.work_delta < 0.0 { -1.0 } else { 1.0 };
        self.work_theta = f64::INFINITY;
        self.work_count = 0;
        for i in 0..self.pack_count {
            let col = self.pack_index[i];
            let mv = ekk.basis.nonbasic_move[col] as f64;
            let alpha = self.pack_value[i] * move_out * mv;
            if alpha > ta {
                self.work_data[self.work_count] = (col, alpha);
                self.work_count += 1;
                let relax = ekk.info.work_dual[col] * mv + td;
                if self.work_theta * alpha > relax {
                    self.work_theta = relax / alpha;
                }
            }
        }
    }

    /// Choose the entering variable by the bound-flipping ratio test,
    /// leaving the variables to flip in `work_data[..work_count]`. False
    /// when the test fails.
    pub fn choose_final(&mut self, ekk: &Ekk) -> bool {
        let work_move = &ekk.basis.nonbasic_move;
        let work_dual = &ekk.info.work_dual;
        let work_range = &ekk.info.work_range;
        let full_count = self.work_count;
        self.work_count = 0;
        let mut total_change = 0.0;
        let total_delta = self.work_delta.abs();
        let mut select_theta = 10.0 * self.work_theta + 1e-7;
        loop {
            for i in self.work_count..full_count {
                let (col, alpha) = self.work_data[i];
                let tight = work_move[col] as f64 * work_dual[col];
                if alpha * select_theta >= tight {
                    self.work_data.swap(self.work_count, i);
                    self.work_count += 1;
                    total_change += work_range[col] * alpha;
                }
            }
            select_theta *= 10.0;
            if total_change >= total_delta || self.work_count == full_count {
                break;
            }
        }
        if !self.choose_final_work_group_quad(ekk) {
            return false;
        }
        let (break_index, break_group) = self.choose_final_large_alpha(ekk);
        let Some(break_index) = break_index else {
            return false;
        };
        let move_out = if self.work_delta < 0.0 { -1.0 } else { 1.0 };
        let pivot = self.work_data[break_index].0;
        self.work_pivot = Some(pivot);
        self.work_alpha = self.work_data[break_index].1 * move_out * work_move[pivot] as f64;
        self.work_theta = if work_dual[pivot] * work_move[pivot] as f64 > 0.0 {
            work_dual[pivot] / self.work_alpha
        } else {
            0.0
        };
        self.work_count = 0;
        for i in 0..self.work_group[break_group] {
            let col = self.work_data[i].0;
            let mv = work_move[col] as f64;
            self.work_data[self.work_count] = (col, mv * work_range[col]);
            self.work_count += 1;
        }
        if self.work_theta == 0.0 {
            self.work_count = 0;
        }
        self.work_data[..self.work_count].sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        true
    }

    fn choose_final_work_group_quad(&mut self, ekk: &Ekk) -> bool {
        let work_move = &ekk.basis.nonbasic_move;
        let work_dual = &ekk.info.work_dual;
        let work_range = &ekk.info.work_range;
        let td = ekk.options.dual_feasibility_tolerance;
        let full_count = self.work_count;
        self.work_count = 0;
        let mut total_change = INITIAL_TOTAL_CHANGE;
        let mut select_theta = self.work_theta;
        let total_delta = self.work_delta.abs();
        self.work_group.clear();
        self.work_group.push(0);
        let mut prev_work_count = self.work_count;
        let mut prev_remain_theta = INITIAL_REMAIN_THETA;
        let mut prev_select_theta = select_theta;
        while select_theta < MAX_SELECT_THETA {
            let mut remain_theta = INITIAL_REMAIN_THETA;
            for i in self.work_count..full_count {
                let (col, value) = self.work_data[i];
                let dual = work_move[col] as f64 * work_dual[col];
                if dual <= select_theta * value {
                    self.work_data.swap(self.work_count, i);
                    self.work_count += 1;
                    total_change += value * work_range[col];
                } else if dual + td < remain_theta * value {
                    remain_theta = (dual + td) / value;
                }
            }
            self.work_group.push(self.work_count);
            select_theta = remain_theta;
            if self.work_count == prev_work_count && prev_select_theta == select_theta && prev_remain_theta == remain_theta {
                return false;
            }
            prev_work_count = self.work_count;
            prev_remain_theta = remain_theta;
            prev_select_theta = select_theta;
            if total_change >= total_delta || self.work_count == full_count {
                break;
            }
        }
        self.work_group.len() > 1
    }

    /// The group, from the last, whose largest pivot is large enough, and
    /// that pivot; ties go to the earlier in HiGHS's random permutation.
    fn choose_final_large_alpha(&self, ekk: &Ekk) -> (Option<usize>, usize) {
        let permutation = &ekk.info.num_tot_permutation;
        let mut final_compare: f64 = 0.0;
        for i in 0..self.work_count {
            final_compare = final_compare.max(self.work_data[i].1);
        }
        final_compare = (0.1 * final_compare).min(1.0);
        let count_group = self.work_group.len() - 1;
        for group in (0..count_group).rev() {
            let mut max_final = 0.0;
            let mut i_max_final: Option<usize> = None;
            for i in self.work_group[group]..self.work_group[group + 1] {
                if max_final < self.work_data[i].1 {
                    max_final = self.work_data[i].1;
                    i_max_final = Some(i);
                } else if max_final == self.work_data[i].1
                    && let Some(best) = i_max_final
                {
                    let j_col = self.work_data[best].0;
                    let i_col = self.work_data[i].0;
                    if permutation[i_col] < permutation[j_col] {
                        i_max_final = Some(i);
                    }
                }
            }
            if let Some(best) = i_max_final
                && self.work_data[best].1 > final_compare
            {
                return (Some(best), group);
            }
        }
        (None, 0)
    }

    /// Flip the chosen bounds, collecting the change in primal values.
    pub fn update_flip(&self, ekk: &mut Ekk, bfrt_column: &mut HVector) {
        let mut dual_objective_value_change = 0.0;
        bfrt_column.clear();
        for i in 0..self.work_count {
            let (col, change) = self.work_data[i];
            dual_objective_value_change += change * ekk.info.work_dual[col] * ekk.cost_scale;
            ekk.flip_bound(col);
            ekk.lp.a.collect_aj(bfrt_column, col, change);
        }
        ekk.info.updated_dual_objective_value += dual_objective_value_change;
    }

    pub fn update_dual(&self, ekk: &mut Ekk, theta: f64) {
        let mut dual_objective_value_change = 0.0;
        for i in 0..self.pack_count {
            let col = self.pack_index[i];
            ekk.info.work_dual[col] -= theta * self.pack_value[i];
            let delta_dual = theta * self.pack_value[i];
            let local_value = ekk.info.work_value[col];
            dual_objective_value_change += ekk.basis.nonbasic_flag[col] as f64 * (-local_value * delta_dual) * ekk.cost_scale;
        }
        ekk.info.updated_dual_objective_value += dual_objective_value_change;
    }

    pub fn create_freelist(&mut self, ekk: &Ekk) {
        self.free_list.clear();
        for var in 0..ekk.lp.num_col + ekk.lp.num_row {
            if ekk.basis.nonbasic_flag[var] != 0 && ekk.info.work_lower[var] == f64::NEG_INFINITY && ekk.info.work_upper[var] == f64::INFINITY {
                self.free_list.insert(var);
            }
        }
    }

    /// Give each nonbasic free variable a direction from the pivotal row.
    pub fn create_freemove(&self, ekk: &mut Ekk, row_ep: &HVector) {
        if self.free_list.is_empty() {
            return;
        }
        let update_count = ekk.info.update_count;
        let ta = if update_count < 10 {
            1e-9
        } else if update_count < 20 {
            3e-8
        } else {
            1e-6
        };
        let move_out = if self.work_delta < 0.0 { -1.0 } else { 1.0 };
        for &var in &self.free_list {
            let alpha = ekk.lp.a.compute_dot(&row_ep.array, var);
            if alpha.abs() > ta {
                ekk.basis.nonbasic_move[var] = if alpha * move_out > 0.0 { 1 } else { -1 };
            }
        }
    }

    pub fn delete_freemove(&self, ekk: &mut Ekk) {
        for &var in &self.free_list {
            ekk.basis.nonbasic_move[var] = 0;
        }
    }

    pub fn delete_freelist(&mut self, var: usize) {
        self.free_list.remove(&var);
    }

    pub fn compute_devex_weight(&mut self, ekk: &Ekk) {
        self.computed_edge_weight = 0.0;
        for el in 0..self.pack_count {
            let var = self.pack_index[el];
            if ekk.basis.nonbasic_flag[var] == 0 {
                continue;
            }
            let pv = ekk.info.devex_index[var] as f64 * self.pack_value[el];
            if pv != 0.0 {
                self.computed_edge_weight += pv * pv;
            }
        }
    }
}
