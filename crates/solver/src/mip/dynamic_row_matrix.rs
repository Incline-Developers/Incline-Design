//! HiGHS's `HighsDynamicRowMatrix`: rows added and removed over time, their
//! storage reused, with each column's positive and negative entries linked
//! for the rows that propagate.

use std::collections::BTreeSet;

const NONE: usize = usize::MAX;

#[derive(Clone, Debug, Default)]
pub(crate) struct DynamicRowMatrix {
    /// Each row's range in `ar_index` and `ar_value`; `(NONE, NONE)` for a
    /// deleted row.
    ar_range: Vec<(usize, usize)>,
    ar_index: Vec<usize>,
    ar_value: Vec<f64>,
    ar_row_index: Vec<usize>,
    a_next_pos: Vec<usize>,
    a_prev_pos: Vec<usize>,
    a_next_neg: Vec<usize>,
    a_prev_neg: Vec<usize>,
    a_head_pos: Vec<usize>,
    a_head_neg: Vec<usize>,
    cols_linked: Vec<bool>,
    /// Free storage as `(length, start)`.
    free_spaces: BTreeSet<(usize, usize)>,
    deleted_rows: Vec<usize>,
}

impl DynamicRowMatrix {
    pub fn new(num_col: usize) -> Self {
        Self {
            a_head_pos: vec![NONE; num_col],
            a_head_neg: vec![NONE; num_col],
            ..Self::default()
        }
    }

    pub fn columns_linked(&self, row: usize) -> bool {
        self.cols_linked[row]
    }

    /// Add a row, reusing storage and a deleted row's index when it can;
    /// returns the row's index.
    pub fn add_row(&mut self, index: &[usize], value: &[f64], link_cols: bool) -> usize {
        let len = index.len();
        let found = self.free_spaces.range((len, 0)..).next().copied();
        let (start, end) = match found {
            None => {
                let start = self.ar_index.len();
                let end = start + len;
                self.ar_index.resize(end, 0);
                self.ar_value.resize(end, 0.0);
                self.ar_row_index.resize(end, 0);
                self.a_prev_pos.resize(end, NONE);
                self.a_next_pos.resize(end, NONE);
                self.a_prev_neg.resize(end, NONE);
                self.a_next_neg.resize(end, NONE);
                (start, end)
            }
            Some(slot) => {
                self.free_spaces.remove(&slot);
                let start = slot.1;
                let end = start + len;
                if slot.0 > len {
                    self.free_spaces.insert((slot.0 - len, end));
                }
                (start, end)
            }
        };
        let row = match self.deleted_rows.pop() {
            None => {
                self.ar_range.push((start, end));
                self.cols_linked.push(link_cols);
                self.ar_range.len() - 1
            }
            Some(row) => {
                self.ar_range[row] = (start, end);
                self.cols_linked[row] = link_cols;
                row
            }
        };
        for i in start..end {
            self.ar_index[i] = index[i - start];
            self.ar_value[i] = value[i - start];
            self.ar_row_index[i] = row;
        }
        if !link_cols {
            return row;
        }
        for i in start..end {
            let col = self.ar_index[i];
            if self.ar_value[i] > 0.0 {
                self.a_prev_pos[i] = NONE;
                let head = self.a_head_pos[col];
                self.a_head_pos[col] = i;
                self.a_next_pos[i] = head;
                if head != NONE {
                    self.a_prev_pos[head] = i;
                }
            } else {
                self.a_prev_neg[i] = NONE;
                let head = self.a_head_neg[col];
                self.a_head_neg[col] = i;
                self.a_next_neg[i] = head;
                if head != NONE {
                    self.a_prev_neg[head] = i;
                }
            }
        }
        row
    }

    fn unlink_entry(&mut self, i: usize) {
        let col = self.ar_index[i];
        if self.ar_value[i] > 0.0 {
            let prev = self.a_prev_pos[i];
            let next = self.a_next_pos[i];
            if next != NONE {
                self.a_prev_pos[next] = prev;
            }
            if prev != NONE {
                self.a_next_pos[prev] = next;
            } else {
                self.a_head_pos[col] = next;
            }
        } else {
            let prev = self.a_prev_neg[i];
            let next = self.a_next_neg[i];
            if next != NONE {
                self.a_prev_neg[next] = prev;
            }
            if prev != NONE {
                self.a_next_neg[prev] = next;
            } else {
                self.a_head_neg[col] = next;
            }
        }
    }

    /// Stop propagating `row`: take its entries out of the column lists.
    pub fn unlink_columns(&mut self, row: usize) {
        if !self.cols_linked[row] {
            return;
        }
        self.cols_linked[row] = false;
        let (start, end) = self.ar_range[row];
        for i in start..end {
            self.unlink_entry(i);
        }
    }

    /// Delete `row`; its index and storage may be reused.
    pub fn remove_row(&mut self, row: usize) {
        let (start, end) = self.ar_range[row];
        if self.cols_linked[row] {
            for i in start..end {
                self.unlink_entry(i);
            }
        }
        self.deleted_rows.push(row);
        self.free_spaces.insert((end - start, start));
        self.ar_range[row] = (NONE, NONE);
    }

    pub fn nonzero_capacity(&self) -> usize {
        self.ar_value.len()
    }

    /// Visit column `col`'s positive entries of propagating rows, as
    /// `(row, value)`, until `f` returns false.
    pub fn for_each_positive_column_entry(&self, col: usize, mut f: impl FnMut(usize, f64) -> bool) {
        let mut iter = self.a_head_pos[col];
        while iter != NONE {
            if !f(self.ar_row_index[iter], self.ar_value[iter]) {
                break;
            }
            iter = self.a_next_pos[iter];
        }
    }

    pub fn for_each_negative_column_entry(&self, col: usize, mut f: impl FnMut(usize, f64) -> bool) {
        let mut iter = self.a_head_neg[col];
        while iter != NONE {
            if !f(self.ar_row_index[iter], self.ar_value[iter]) {
                break;
            }
            iter = self.a_next_neg[iter];
        }
    }

    pub fn num_rows(&self) -> usize {
        self.ar_range.len()
    }

    pub fn num_deleted_rows(&self) -> usize {
        self.deleted_rows.len()
    }

    pub fn row_start(&self, row: usize) -> usize {
        self.ar_range[row].0
    }

    pub fn row_end(&self, row: usize) -> usize {
        self.ar_range[row].1
    }

    pub fn ar_index(&self) -> &[usize] {
        &self.ar_index
    }

    pub fn ar_value(&self) -> &[f64] {
        &self.ar_value
    }

    /// Row `row`'s entries.
    pub fn row(&self, row: usize) -> (&[usize], &[f64]) {
        let (start, end) = self.ar_range[row];
        (&self.ar_index[start..end], &self.ar_value[start..end])
    }
}
