//! HiGHS's `HighsGFkSolve`: Gaussian elimination over GF(k), k a small
//! prime, for the mod-k separator: it finds combinations of rows whose
//! sum is 0 mod k except for a right hand side of 1, walking several
//! basic solutions.

use std::{cmp::Reverse, collections::BinaryHeap};

use super::{cpp_heap_pop, cpp_heap_push};
use crate::presolve::matrix::Trees;

const NONE: usize = usize::MAX;

/// HiGHS's `HighsGFk<k>::powk`, in wrapping 32-bit arithmetic.
fn powk(k: u32, a: u32) -> u32 {
    match k {
        2 => a.wrapping_mul(a),
        3 => a.wrapping_mul(a).wrapping_mul(a),
        _ if k & 1 == 0 => powk(2, powk(k / 2, a)),
        _ => powk(k - 1, a).wrapping_mul(a),
    }
}

/// HiGHS's `HighsGFk<k>::inverse`.
fn inverse(k: u32, a: u32) -> u32 {
    match k {
        2 => 1,
        3 => a,
        _ => powk(k - 2, a) % k,
    }
}

/// HiGHS's `SolutionEntry`: a weight for a column of the system (a row
/// of the LP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SolutionEntry {
    pub index: usize,
    pub weight: u32,
}

#[derive(Default)]
pub(crate) struct GfkSolve {
    num_col: usize,
    num_row: usize,
    a_row: Vec<usize>,
    a_col: Vec<usize>,
    a_value: Vec<u32>,
    rowsize: Vec<usize>,
    colsize: Vec<usize>,
    colhead: Vec<usize>,
    a_next: Vec<usize>,
    a_prev: Vec<usize>,
    rowroot: Vec<usize>,
    ar_left: Vec<usize>,
    ar_right: Vec<usize>,
    rhs: Vec<u32>,
    factor_col_perm: Vec<usize>,
    factor_row_perm: Vec<usize>,
    col_basis_status: Vec<i8>,
    row_used: Vec<bool>,
    iterstack: Vec<usize>,
    rowpositions: Vec<usize>,
    rowpos_colsizes: Vec<usize>,
    freeslots: BinaryHeap<Reverse<usize>>,
}

impl GfkSolve {
    fn trees(&mut self) -> Trees<'_> {
        Trees {
            left: &mut self.ar_left,
            right: &mut self.ar_right,
            key: &self.a_col,
        }
    }

    fn link(&mut self, pos: usize) {
        let col = self.a_col[pos];
        self.a_next[pos] = self.colhead[col];
        self.a_prev[pos] = NONE;
        self.colhead[col] = pos;
        if self.a_next[pos] != NONE {
            let next = self.a_next[pos];
            self.a_prev[next] = pos;
        }
        self.colsize[col] += 1;
        let row = self.a_row[pos];
        let root = self.rowroot[row];
        self.rowroot[row] = self.trees().link(pos, root);
        self.rowsize[row] += 1;
    }

    fn unlink(&mut self, pos: usize) {
        let next = self.a_next[pos];
        let prev = self.a_prev[pos];
        if next != NONE {
            self.a_prev[next] = prev;
        }
        if prev != NONE {
            self.a_next[prev] = next;
        } else {
            self.colhead[self.a_col[pos]] = next;
        }
        self.colsize[self.a_col[pos]] -= 1;
        let row = self.a_row[pos];
        let root = self.rowroot[row];
        self.rowroot[row] = self.trees().unlink(pos, root);
        self.rowsize[row] -= 1;
        self.a_value[pos] = 0;
        self.freeslots.push(Reverse(pos));
    }

    fn drop_if_zero(&mut self, pos: usize) {
        if self.a_value[pos] == 0 {
            self.unlink(pos);
        }
    }

    fn store_row_positions(&mut self, pos: usize) {
        if pos == NONE {
            return;
        }
        self.iterstack.push(pos);
        while let Some(pos) = self.iterstack.pop() {
            self.rowpositions.push(pos);
            self.rowpos_colsizes.push(self.colsize[self.a_col[pos]]);
            if self.ar_left[pos] != NONE {
                self.iterstack.push(self.ar_left[pos]);
            }
            if self.ar_right[pos] != NONE {
                self.iterstack.push(self.ar_right[pos]);
            }
        }
    }

    /// HiGHS's `findNonzero`.
    pub fn find_nonzero(&mut self, row: usize, col: usize) -> Option<usize> {
        if self.rowroot[row] == NONE {
            return None;
        }
        let root = self.rowroot[row];
        self.rowroot[row] = self.trees().splay(col, root);
        (self.a_col[self.rowroot[row]] == col).then_some(self.rowroot[row])
    }

    fn add_nonzero(&mut self, row: usize, col: usize, val: u32) {
        let pos = match self.freeslots.pop() {
            None => {
                let pos = self.a_value.len();
                self.a_value.push(val);
                self.a_row.push(row);
                self.a_col.push(col);
                self.a_next.push(NONE);
                self.a_prev.push(NONE);
                self.ar_left.push(NONE);
                self.ar_right.push(NONE);
                pos
            }
            Some(Reverse(pos)) => {
                self.a_value[pos] = val;
                self.a_row[pos] = row;
                self.a_col[pos] = col;
                self.a_prev[pos] = NONE;
                pos
            }
        };
        self.link(pos);
    }

    /// HiGHS's `fromCSC<k>`: the system whose columns are given
    /// column-wise, entries mod `k`.
    pub fn load_csc(&mut self, k: u32, a_val: &[i64], a_index: &[usize], a_start: &[usize], num_row: usize) {
        self.a_value.clear();
        self.a_col.clear();
        self.a_row.clear();
        self.freeslots.clear();
        self.num_col = a_start.len() - 1;
        self.num_row = num_row;
        self.colhead = vec![NONE; self.num_col];
        self.colsize = vec![0; self.num_col];
        self.rhs = vec![0; num_row];
        self.rowroot = vec![NONE; num_row];
        self.rowsize = vec![0; num_row];
        for i in 0..self.num_col {
            for j in a_start[i]..a_start[i + 1] {
                let mut val = a_val[j] % k as i64;
                if val == 0 {
                    continue;
                }
                if val < 0 {
                    val += k as i64;
                }
                self.a_value.push(val as u32);
                self.a_col.push(i);
                self.a_row.push(a_index[j]);
            }
        }
        let nnz = self.a_value.len();
        self.a_next = vec![0; nnz];
        self.a_prev = vec![0; nnz];
        self.ar_left = vec![0; nnz];
        self.ar_right = vec![0; nnz];
        for pos in 0..nnz {
            self.link(pos);
        }
    }

    /// HiGHS's `setRhs<k>`.
    pub fn set_rhs(&mut self, k: u32, row: usize, val: i64) {
        self.rhs[row] = (val.unsigned_abs() as u32) % k;
    }

    /// HiGHS's `solve<k>`: report each basic solution walked.
    pub fn solve(&mut self, k: u32, mut report_solution: impl FnMut(&mut Vec<SolutionEntry>)) {
        let less = |a: &(usize, usize), b: &(usize, usize)| a.0 > b.0;
        let mut pqueue: Vec<(usize, usize)> = Vec::new();
        for i in 0..self.num_col {
            cpp_heap_push(&mut pqueue, (self.colsize[i], i), less);
        }
        let max_pivot = self.num_row.min(self.num_col);
        self.factor_col_perm.clear();
        self.factor_row_perm.clear();
        self.factor_col_perm.reserve(max_pivot);
        self.factor_row_perm.reserve(max_pivot);
        self.col_basis_status = vec![0; self.num_col];
        self.row_used = vec![false; self.num_row];
        let mut num_pivot = 0;
        while let Some((old_col_size, pivot_col)) = cpp_heap_pop(&mut pqueue, less) {
            if self.colsize[pivot_col] == 0 {
                continue;
            }
            if self.colsize[pivot_col] != old_col_size {
                cpp_heap_push(&mut pqueue, (self.colsize[pivot_col], pivot_col), less);
                continue;
            }
            let mut pivot = NONE;
            let mut pivot_row = NONE;
            let mut pivot_row_len = usize::MAX;
            let mut coliter = self.colhead[pivot_col];
            while coliter != NONE {
                let row = self.a_row[coliter];
                if !self.row_used[row] && self.rowsize[row] < pivot_row_len {
                    pivot_row_len = self.rowsize[row];
                    pivot_row = row;
                    pivot = coliter;
                }
                coliter = self.a_next[coliter];
            }
            let pivot_inverse = inverse(k, self.a_value[pivot]);
            self.rowpositions.clear();
            self.rowpos_colsizes.clear();
            self.store_row_positions(self.rowroot[pivot_row]);
            let mut coliter = self.colhead[pivot_col];
            while coliter != NONE {
                let next = self.a_next[coliter];
                if coliter == pivot {
                    coliter = next;
                    continue;
                }
                let row = self.a_row[coliter];
                if self.row_used[row] {
                    coliter = next;
                    continue;
                }
                let pivot_row_scale = pivot_inverse.wrapping_mul(k - self.a_value[coliter]);
                self.rhs[row] = self.rhs[row].wrapping_add(pivot_row_scale.wrapping_mul(self.rhs[pivot_row])) % k;
                for idx in 0..self.rowpositions.len() {
                    let pivot_row_pos = self.rowpositions[idx];
                    let target_col = self.a_col[pivot_row_pos];
                    match self.find_nonzero(self.a_row[coliter], target_col) {
                        None => {
                            let val = pivot_row_scale.wrapping_mul(self.a_value[pivot_row_pos]) % k;
                            if val != 0 {
                                self.add_nonzero(row, target_col, val);
                            }
                        }
                        Some(nonzero_pos) => {
                            self.a_value[nonzero_pos] = self.a_value[nonzero_pos].wrapping_add(pivot_row_scale.wrapping_mul(self.a_value[pivot_row_pos])) % k;
                            self.drop_if_zero(nonzero_pos);
                        }
                    }
                }
                coliter = next;
            }
            num_pivot += 1;
            self.factor_col_perm.push(pivot_col);
            self.factor_row_perm.push(pivot_row);
            self.col_basis_status[pivot_col] = 1;
            self.row_used[pivot_row] = true;
            if num_pivot == max_pivot {
                break;
            }
            for i in 0..pivot_row_len {
                let col = self.a_col[self.rowpositions[i]];
                let oldsize = self.rowpos_colsizes[i];
                self.colsize[col] -= 1;
                if self.colsize[col] == 0 {
                    continue;
                }
                if self.colsize[col] < oldsize {
                    cpp_heap_push(&mut pqueue, (self.colsize[col], col), less);
                }
            }
        }

        for i in 0..self.num_row {
            if self.row_used[i] {
                continue;
            }
            if self.rhs[i] != 0 {
                return;
            }
        }
        let mut solution: Vec<SolutionEntry> = Vec::with_capacity(self.num_col);
        let num_factor_rows = self.factor_row_perm.len();
        let mut basis_swaps: Vec<(usize, usize)> = Vec::new();
        for i in (0..num_factor_rows).rev() {
            let row = self.factor_row_perm[i];
            self.iterstack.push(self.rowroot[row]);
            while let Some(rowpos) = self.iterstack.pop() {
                if self.ar_left[rowpos] != NONE {
                    self.iterstack.push(self.ar_left[rowpos]);
                }
                if self.ar_right[rowpos] != NONE {
                    self.iterstack.push(self.ar_right[rowpos]);
                }
                let col = self.a_col[rowpos];
                if self.col_basis_status[col] != 0 {
                    continue;
                }
                self.col_basis_status[col] = -1;
                basis_swaps.push((i, col));
            }
        }
        let mut basis_swap_pos = 0;
        loop {
            let mut performed_basis_swap = false;
            solution.clear();
            for i in (0..num_factor_rows).rev() {
                let row = self.factor_row_perm[i];
                let mut solval: u32 = 0;
                for e in 0..solution.len() {
                    let entry = solution[e];
                    if let Some(pos) = self.find_nonzero(row, entry.index) {
                        solval = solval.wrapping_add(self.a_value[pos].wrapping_mul(entry.weight));
                    }
                }
                solval = self.rhs[row].wrapping_add(k).wrapping_sub(solval % k);
                let col = self.factor_col_perm[i];
                let pos = self.find_nonzero(row, col).expect("the pivot");
                let col_val_inverse = inverse(k, self.a_value[pos]);
                solval = solval.wrapping_mul(col_val_inverse) % k;
                if solval != 0 {
                    solution.push(SolutionEntry { index: col, weight: solval });
                }
            }
            report_solution(&mut solution);
            if basis_swap_pos < basis_swaps.len() {
                let (basis_index, entering_col) = basis_swaps[basis_swap_pos];
                let leaving_col = self.factor_col_perm[basis_index];
                self.factor_col_perm[basis_index] = entering_col;
                self.col_basis_status[entering_col] = 1;
                self.col_basis_status[leaving_col] = 0;
                performed_basis_swap = true;
                basis_swap_pos += 1;
            }
            if !performed_basis_swap {
                break;
            }
        }
    }
}
