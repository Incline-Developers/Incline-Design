//! HiGHS's `HighsObjectiveFunction`: the objective's nonzeros, binaries
//! first, with their clique partition and integral scale.

use super::{MipModel, cpp_partition};
use crate::{hash_table::hash_i32, integers::integral_scale};

#[derive(Clone, Debug, Default)]
pub(crate) struct ObjectiveFunction {
    obj_int_scale: f64,
    num_integral: usize,
    num_binary: usize,
    objective_nonzeros: Vec<usize>,
    objective_vals: Vec<f64>,
    clique_partition_start: Vec<usize>,
    /// Each column's position in a clique partition, if it is in one.
    col_to_partition: Vec<Option<usize>>,
}

impl ObjectiveFunction {
    pub fn new(model: &MipModel) -> Self {
        let mut objective_nonzeros: Vec<usize> = (0..model.num_col).filter(|&col| model.col_cost[col] != 0.0).collect();
        let col_to_partition = vec![None; model.num_col];
        let clique_partition_start = vec![0];
        if objective_nonzeros.is_empty() {
            return Self {
                obj_int_scale: 1.0,
                objective_nonzeros,
                col_to_partition,
                clique_partition_start,
                ..Self::default()
            };
        }
        let num_integral = cpp_partition(&mut objective_nonzeros, |&col| !model.is_col_continuous(col));
        let num_binary = if num_integral == 0 {
            0
        } else {
            cpp_partition(&mut objective_nonzeros[..num_integral], |&col| model.col_lower[col] == 0.0 && model.col_upper[col] == 1.0)
        };
        let objective_vals = objective_nonzeros.iter().map(|&col| model.col_cost[col]).collect();
        Self {
            obj_int_scale: 0.0,
            num_integral,
            num_binary,
            objective_nonzeros,
            objective_vals,
            clique_partition_start,
            col_to_partition,
        }
    }

    /// HiGHS's `setupCliquePartition`, given the clique partition of the
    /// binaries (`partition_start` over `clqvars`, as the clique table's
    /// `cliquePartition` leaves them: `(col, val)` pairs reordered).
    pub fn setup_clique_partition(&mut self, model: &MipModel, partition: impl FnOnce(&[f64], &mut Vec<(usize, bool)>, &mut Vec<usize>)) {
        if self.num_binary <= 1 {
            return;
        }
        let mut clqvars: Vec<(usize, bool)> = self.objective_nonzeros[..self.num_binary].iter().map(|&col| (col, model.col_cost[col] < 0.0)).collect();
        partition(&model.col_cost, &mut clqvars, &mut self.clique_partition_start);
        let num_partitions = self.clique_partition_start.len() - 1;
        if num_partitions == self.num_binary {
            self.clique_partition_start.truncate(1);
            return;
        }
        let mut p = 0;
        let mut k = 0;
        for i in 0..num_partitions {
            if self.clique_partition_start[i + 1] - self.clique_partition_start[i] == 1 {
                continue;
            }
            let (start, end) = (self.clique_partition_start[i], self.clique_partition_start[i + 1]);
            self.clique_partition_start[p] = k;
            for j in start..end {
                self.col_to_partition[clqvars[j].0] = Some(k);
                k += 1;
            }
            p += 1;
        }
        self.clique_partition_start[p] = k;
        self.clique_partition_start.truncate(p + 1);
        let col_to_partition = &self.col_to_partition;
        // HiGHS orders by the partition position as an unsigned integer, so
        // columns in no partition (-1) come last.
        self.objective_nonzeros[..self.num_binary].sort_by_key(|&col| (col_to_partition[col].map_or(u32::MAX, |pos| pos as u32), hash_i32(col as i32)));
        for i in 0..self.num_binary {
            self.objective_vals[i] = model.col_cost[self.objective_nonzeros[i]];
        }
    }

    pub fn check_integrality(&mut self, epsilon: f64) {
        if self.num_integral == self.objective_nonzeros.len() {
            if self.num_integral > 0 {
                self.obj_int_scale = integral_scale(&self.objective_vals, epsilon, epsilon);
                if self.obj_int_scale * super::TINY > epsilon {
                    self.obj_int_scale = 0.0;
                }
            } else {
                self.obj_int_scale = 1.0;
            }
        }
    }

    /// The columns with a nonzero cost, binaries first.
    pub fn objective_nonzeros(&self) -> &[usize] {
        &self.objective_nonzeros
    }

    pub fn objective_values_packed(&self) -> &[f64] {
        &self.objective_vals
    }

    pub fn num_binaries_in_objective(&self) -> usize {
        self.num_binary
    }

    pub fn clique_partition_starts(&self) -> &[usize] {
        &self.clique_partition_start
    }

    pub fn num_clique_partitions(&self) -> usize {
        self.clique_partition_start.len() - 1
    }

    pub fn col_clique_partition(&self, col: usize) -> Option<usize> {
        self.col_to_partition[col]
    }

    pub fn integral_scale(&self) -> f64 {
        self.obj_int_scale
    }

    pub fn is_integral(&self) -> bool {
        self.obj_int_scale != 0.0
    }

    pub fn is_zero(&self) -> bool {
        self.objective_nonzeros.is_empty()
    }
}
