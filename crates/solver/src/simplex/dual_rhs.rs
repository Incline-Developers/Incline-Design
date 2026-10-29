//! HiGHS's `HEkkDualRHS`: the primal infeasibilities of the basic
//! variables, and the choice of leaving row by them (CHUZR).

use super::ekk::{EXCESSIVE_PRIMAL_VALUE, Ekk};
use crate::hvector::{HVector, ZERO};

#[derive(Clone, Debug, Default)]
pub(crate) struct DualRhs {
    /// Rows with infeasibility above this (relative to their weight) are
    /// listed; zero lists every infeasible row.
    pub work_cutoff: f64,
    /// Number of listed rows, or minus the number of rows when every row
    /// is to be searched.
    pub work_count: isize,
    pub work_mark: Vec<u8>,
    pub work_index: Vec<usize>,
    /// Each row's primal infeasibility, squared unless LiDSE is in use.
    pub work_infeasibility: Vec<f64>,
}

fn infeasibility(value: f64, lower: f64, upper: f64, tolerance: f64) -> f64 {
    if value < lower - tolerance {
        lower - value
    } else if value > upper + tolerance {
        value - upper
    } else {
        0.0
    }
}

impl DualRhs {
    pub fn setup(&mut self, num_row: usize) {
        self.work_mark = vec![0; num_row];
        self.work_index = vec![0; num_row];
        self.work_infeasibility = vec![0.0; num_row];
    }

    pub fn choose_normal(&mut self, ekk: &mut Ekk) -> Option<usize> {
        if self.work_count == 0 {
            return None;
        }
        let edge_weight = &ekk.dual_edge_weight;
        if self.work_count < 0 {
            let num_row = (-self.work_count) as usize;
            let random_start = ekk.random.integer(num_row);
            let mut best_merit = 0.0;
            let mut best_index = None;
            for (start, end) in [(random_start, num_row), (0, random_start)] {
                for row in start..end {
                    let infeas = self.work_infeasibility[row];
                    if infeas > ZERO {
                        let weight = edge_weight[row];
                        if best_merit * weight < infeas {
                            best_merit = infeas / weight;
                            best_index = Some(row);
                        }
                    }
                }
            }
            best_index
        } else {
            let count = self.work_count as usize;
            let random_start = ekk.random.integer(count);
            let mut best_merit = 0.0;
            let mut best_index = None;
            for (start, end) in [(random_start, count), (0, random_start)] {
                for i in start..end {
                    let row = self.work_index[i];
                    let infeas = self.work_infeasibility[row];
                    if infeas > ZERO {
                        let weight = edge_weight[row];
                        if best_merit * weight < infeas {
                            best_merit = infeas / weight;
                            best_index = Some(row);
                        }
                    }
                }
            }
            let create_list_again = match best_index {
                None => self.work_cutoff > 0.0,
                Some(_) => best_merit <= self.work_cutoff * 0.99,
            };
            if create_list_again {
                self.create_infeas_list(ekk, 0.0);
                return self.choose_normal(ekk);
            }
            best_index
        }
    }

    /// Subtract `theta` times `column` from the basic values, updating
    /// their infeasibilities. False if any value becomes excessive.
    pub fn update_primal(&mut self, ekk: &mut Ekk, column: &HVector, theta: f64) -> bool {
        let num_row = ekk.lp.num_row;
        let tolerance = ekk.options.primal_feasibility_tolerance;
        let squared = ekk.info.store_squared_primal_infeasibility;
        let dense = column.count < 0 || column.count as f64 > 0.4 * num_row as f64;
        let to_entry = if dense { num_row } else { column.count as usize };
        let mut num_excessive = 0;
        let info = &mut ekk.info;
        for entry in 0..to_entry {
            let row = if dense { entry } else { column.index[entry] };
            info.base_value[row] -= theta * column.array[row];
            let value = info.base_value[row];
            let infeas = infeasibility(value, info.base_lower[row], info.base_upper[row], tolerance);
            self.work_infeasibility[row] = if squared { infeas * infeas } else { infeas.abs() };
            if value <= -EXCESSIVE_PRIMAL_VALUE || value >= EXCESSIVE_PRIMAL_VALUE {
                num_excessive += 1;
            }
        }
        num_excessive == 0
    }

    pub fn update_pivots(&mut self, ekk: &mut Ekk, row: usize, value: f64) {
        let tolerance = ekk.options.primal_feasibility_tolerance;
        ekk.info.base_value[row] = value;
        let infeas = infeasibility(value, ekk.info.base_lower[row], ekk.info.base_upper[row], tolerance);
        self.work_infeasibility[row] = if ekk.info.store_squared_primal_infeasibility { infeas * infeas } else { infeas.abs() };
    }

    /// List the rows of `column` that have become infeasible enough.
    pub fn update_infeas_list(&mut self, ekk: &Ekk, column: &HVector) {
        if self.work_count < 0 {
            return;
        }
        let edge_weight = &ekk.dual_edge_weight;
        for &row in column.indices() {
            if self.work_mark[row] != 0 {
                continue;
            }
            let listed = if self.work_cutoff <= 0.0 {
                self.work_infeasibility[row] != 0.0
            } else {
                self.work_infeasibility[row] > edge_weight[row] * self.work_cutoff
            };
            if listed {
                self.work_index[self.work_count as usize] = row;
                self.work_count += 1;
                self.work_mark[row] = 1;
            }
        }
    }

    pub fn create_array_of_primal_infeasibilities(&mut self, ekk: &Ekk) {
        let tolerance = ekk.options.primal_feasibility_tolerance;
        let squared = ekk.info.store_squared_primal_infeasibility;
        for row in 0..ekk.lp.num_row {
            let infeas = infeasibility(ekk.info.base_value[row], ekk.info.base_lower[row], ekk.info.base_upper[row], tolerance);
            self.work_infeasibility[row] = if squared { infeas * infeas } else { infeas.abs() };
        }
    }

    /// List the infeasible rows; when there are many and columns are
    /// sparse, only those with the largest merit.
    pub fn create_infeas_list(&mut self, ekk: &mut Ekk, column_density: f64) {
        let num_row = ekk.lp.num_row;
        self.work_mark.fill(0);
        self.work_count = 0;
        self.work_cutoff = 0.0;
        for row in 0..num_row {
            if self.work_infeasibility[row] != 0.0 {
                self.work_mark[row] = 1;
                self.work_index[self.work_count as usize] = row;
                self.work_count += 1;
            }
        }
        let edge_weight = &ekk.dual_edge_weight;
        let work_count = self.work_count as usize;
        if work_count as f64 > (num_row as f64 * 0.01).max(500.0) && column_density < 0.05 {
            let icutoff = (work_count as f64 * 0.001).max(500.0) as usize;
            let mut max_merit: f64 = 0.0;
            let dwork = &mut ekk.scattered_dual_edge_weight;
            let mut put = 0;
            for row in 0..num_row {
                if self.work_mark[row] != 0 {
                    let merit = self.work_infeasibility[row] / edge_weight[row];
                    max_merit = max_merit.max(merit);
                    dwork[put] = -merit;
                    put += 1;
                }
            }
            dwork[..work_count].select_nth_unstable_by(icutoff, |a, b| a.total_cmp(b));
            let cut_merit = -dwork[icutoff];
            self.work_cutoff = (max_merit * 0.99999).min(cut_merit * 1.00001);

            self.work_mark.fill(0);
            self.work_count = 0;
            for row in 0..num_row {
                if self.work_infeasibility[row] >= edge_weight[row] * self.work_cutoff {
                    self.work_index[self.work_count as usize] = row;
                    self.work_count += 1;
                    self.work_mark[row] = 1;
                }
            }
            if self.work_count as f64 > icutoff as f64 * 1.5 {
                let full_count = self.work_count as usize;
                self.work_count = icutoff as isize;
                for i in icutoff..full_count {
                    let row = self.work_index[i];
                    if self.work_infeasibility[row] > edge_weight[row] * cut_merit {
                        self.work_index[self.work_count as usize] = row;
                        self.work_count += 1;
                    } else {
                        self.work_mark[row] = 0;
                    }
                }
            }
        }
        if self.work_count as f64 > 0.2 * num_row as f64 {
            self.work_count = -(num_row as isize);
            self.work_cutoff = 0.0;
        }
    }
}
