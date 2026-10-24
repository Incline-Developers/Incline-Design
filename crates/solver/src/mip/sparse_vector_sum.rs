//! HiGHS's `HighsSparseVectorSum`: a sparse vector summed in double-double
//! precision, remembering which entries were touched.

use crate::util::CDouble;

#[derive(Clone, Debug, Default)]
pub(crate) struct SparseVectorSum {
    pub values: Vec<CDouble>,
    pub nonzeroinds: Vec<usize>,
}

impl SparseVectorSum {
    pub fn new(dimension: usize) -> Self {
        let mut sum = Self::default();
        sum.set_dimension(dimension);
        sum
    }

    pub fn set_dimension(&mut self, dimension: usize) {
        self.values.resize(dimension, CDouble::new(0.0));
        self.nonzeroinds.reserve(dimension);
    }

    pub fn add(&mut self, index: usize, value: f64) {
        self.add_c(index, CDouble::new(value));
    }

    pub fn add_c(&mut self, index: usize, value: CDouble) {
        if self.values[index] != 0.0 {
            self.values[index] += value;
        } else {
            self.values[index] = value;
            self.nonzeroinds.push(index);
        }
        if self.values[index] == 0.0 {
            self.values[index] = CDouble::new(f64::MIN_POSITIVE);
        }
    }

    pub fn nonzeros(&self) -> &[usize] {
        &self.nonzeroinds
    }

    pub fn value(&self, index: usize) -> f64 {
        self.values[index].value()
    }

    pub fn clear(&mut self) {
        if 10 * self.nonzeroinds.len() < 3 * self.values.len() {
            for &i in &self.nonzeroinds {
                self.values[i] = CDouble::new(0.0);
            }
        } else {
            self.values.fill(CDouble::new(0.0));
        }
        self.nonzeroinds.clear();
    }

    /// HiGHS's `partition`, by `std::partition`.
    pub fn partition(&mut self, pred: impl FnMut(&usize) -> bool) -> usize {
        super::cpp_partition(&mut self.nonzeroinds, pred)
    }

    /// HiGHS's `cleanup`: drop the entries `is_zero` says are zero.
    pub fn cleanup(&mut self, mut is_zero: impl FnMut(usize, f64) -> bool) {
        let mut num_nz = self.nonzeroinds.len();
        for i in (0..num_nz).rev() {
            let pos = self.nonzeroinds[i];
            let val = self.values[pos].value();
            if is_zero(pos, val) {
                self.values[pos] = CDouble::new(0.0);
                num_nz -= 1;
                self.nonzeroinds.swap(num_nz, i);
            }
        }
        self.nonzeroinds.truncate(num_nz);
    }
}
