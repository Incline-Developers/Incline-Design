//! A linear program: columns with a cost and bounds, rows with bounds over
//! a sparse matrix.

/// Whether the objective is minimised or maximised.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sense {
    #[default]
    Minimise,
    Maximise,
}

/// `minimise / maximise  cost·x + offset`  subject to
/// `row_lower ≤ A·x ≤ row_upper` and `col_lower ≤ x ≤ col_upper`.
///
/// Bounds may be infinite. The matrix is held row-wise, the way programs
/// are usually built; the solver takes its own column-wise copy.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Problem {
    pub sense: Sense,
    pub offset: f64,
    pub col_cost: Vec<f64>,
    pub col_lower: Vec<f64>,
    pub col_upper: Vec<f64>,
    /// Whether each column must take an integer value. Empty for a pure LP.
    pub integrality: Vec<bool>,
    pub row_lower: Vec<f64>,
    pub row_upper: Vec<f64>,
    /// Row `i`'s entries are `row_index[row_start[i]..row_start[i + 1]]`
    /// with values `row_value[..]` alike.
    pub row_start: Vec<usize>,
    pub row_index: Vec<usize>,
    pub row_value: Vec<f64>,
}

impl Problem {
    pub fn new(sense: Sense) -> Self {
        Self {
            sense,
            row_start: vec![0],
            ..Self::default()
        }
    }

    pub fn num_cols(&self) -> usize {
        self.col_cost.len()
    }

    pub fn num_rows(&self) -> usize {
        self.row_lower.len()
    }

    pub fn num_nonzeros(&self) -> usize {
        self.row_index.len()
    }

    /// Add a column, returning its index.
    pub fn add_col(&mut self, cost: f64, lower: f64, upper: f64) -> usize {
        self.col_cost.push(cost);
        self.col_lower.push(lower);
        self.col_upper.push(upper);
        if !self.integrality.is_empty() {
            self.integrality.push(false);
        }
        self.col_cost.len() - 1
    }

    /// Add a row over existing columns, returning its index.
    pub fn add_row(&mut self, lower: f64, upper: f64, entries: impl IntoIterator<Item = (usize, f64)>) -> usize {
        if self.row_start.is_empty() {
            self.row_start.push(0);
        }
        for (col, value) in entries {
            debug_assert!(col < self.num_cols(), "row entry for column {col} of {}", self.num_cols());
            self.row_index.push(col);
            self.row_value.push(value);
        }
        self.row_start.push(self.row_index.len());
        self.row_lower.push(lower);
        self.row_upper.push(upper);
        self.row_lower.len() - 1
    }

    /// Row `row`'s entries as `(column, value)`.
    pub fn row(&self, row: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let range = self.row_start[row]..self.row_start[row + 1];
        self.row_index[range.clone()].iter().copied().zip(self.row_value[range].iter().copied())
    }

    /// The objective at `col_value`, offset included.
    pub fn objective_at(&self, col_value: &[f64]) -> f64 {
        self.offset + self.col_cost.iter().zip(col_value).map(|(cost, value)| cost * value).sum::<f64>()
    }

    /// The matrix column-wise: `(start, index, value)` with column `j`'s
    /// rows at `index[start[j]..start[j + 1]]`, in increasing order.
    pub fn col_wise(&self) -> (Vec<usize>, Vec<usize>, Vec<f64>) {
        let num_cols = self.num_cols();
        let mut start = vec![0usize; num_cols + 1];
        for &col in &self.row_index {
            start[col + 1] += 1;
        }
        for col in 0..num_cols {
            start[col + 1] += start[col];
        }
        let mut next = start.clone();
        let mut index = vec![0usize; self.row_index.len()];
        let mut value = vec![0.0; self.row_index.len()];
        for row in 0..self.num_rows() {
            for (col, entry) in self.row(row) {
                let at = next[col];
                index[at] = row;
                value[at] = entry;
                next[col] += 1;
            }
        }
        (start, index, value)
    }
}
