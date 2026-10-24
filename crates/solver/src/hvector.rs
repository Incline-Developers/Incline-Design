//! HiGHS's `HVector`: a full-length array with the indices of its
//! (possible) nonzeros, so sparse vectors are cheap to clear and traverse.

/// Values no larger than this are treated as zero.
pub(crate) const TINY: f64 = 1e-14;
/// What a value that cancels to [`TINY`] or less is set to, so that it
/// stays distinct from an untouched zero while its index is listed.
pub(crate) const ZERO: f64 = 1e-50;

#[derive(Clone, Debug, Default)]
pub(crate) struct HVector {
    pub size: usize,
    /// Number of listed indices, or -1 when they are not known and the
    /// array must be treated as dense.
    pub count: isize,
    pub index: Vec<usize>,
    pub array: Vec<f64>,
    /// Synthetic clock for the operations on this vector.
    pub synthetic_tick: f64,
    /// Work space for hyper-sparse solves and the update.
    pub cwork: Vec<u8>,
    pub iwork: Vec<usize>,
    /// Whether [`HVector::pack`] should copy the nonzeros out: the
    /// Forrest-Tomlin update reads the partially solved vector from there.
    pub pack_flag: bool,
    pub pack_count: usize,
    pub pack_index: Vec<usize>,
    pub pack_value: Vec<f64>,
}

impl HVector {
    pub fn new(size: usize) -> Self {
        let mut vector = Self::default();
        vector.setup(size);
        vector
    }

    pub fn setup(&mut self, size: usize) {
        self.size = size;
        self.count = 0;
        self.index = vec![0; size];
        self.array = vec![0.0; size];
        // Room for the factor's pivots and up to 6400 updates' worth.
        self.cwork = vec![0; size + 6400];
        self.iwork = vec![0; size * 4];
        self.pack_count = 0;
        self.pack_index = vec![0; size];
        self.pack_value = vec![0.0; size];
        self.pack_flag = false;
        self.synthetic_tick = 0.0;
    }

    pub fn clear(&mut self) {
        let dense_clear = self.count < 0 || self.count as f64 > self.size as f64 * 0.3;
        if dense_clear {
            self.array.fill(0.0);
        } else {
            for &i in &self.index[..self.count as usize] {
                self.array[i] = 0.0;
            }
        }
        self.clear_scalars();
    }

    pub fn clear_scalars(&mut self) {
        self.pack_flag = false;
        self.count = 0;
        self.synthetic_tick = 0.0;
    }

    /// Zero the values no larger than [`TINY`], keeping the index.
    pub fn tight(&mut self) {
        if self.count < 0 {
            for value in &mut self.array {
                if value.abs() < TINY {
                    *value = 0.0;
                }
            }
            return;
        }
        let mut kept = 0;
        for k in 0..self.count as usize {
            let i = self.index[k];
            if self.array[i].abs() >= TINY {
                self.index[kept] = i;
                kept += 1;
            } else {
                self.array[i] = 0.0;
            }
        }
        self.count = kept as isize;
    }

    /// Copy the nonzeros into the pack arrays, if the pack flag is set.
    pub fn pack(&mut self) {
        if !self.pack_flag {
            return;
        }
        self.pack_flag = false;
        self.pack_count = 0;
        for k in 0..self.count as usize {
            let i = self.index[k];
            self.pack_index[self.pack_count] = i;
            self.pack_value[self.pack_count] = self.array[i];
            self.pack_count += 1;
        }
    }

    /// Rebuild the index from the array when it is not short.
    pub fn re_index(&mut self) {
        if self.count >= 0 && self.count as f64 <= self.size as f64 * 0.1 {
            return;
        }
        let mut count = 0;
        let index = &mut self.index[..self.size];
        for (i, &value) in self.array[..self.size].iter().enumerate() {
            if value != 0.0 {
                index[count] = i;
                count += 1;
            }
        }
        self.count = count as isize;
    }

    pub fn copy_from(&mut self, from: &HVector) {
        self.clear();
        self.synthetic_tick = from.synthetic_tick;
        self.count = from.count;
        for k in 0..from.count as usize {
            let i = from.index[k];
            self.index[k] = i;
            self.array[i] = from.array[i];
        }
    }

    pub fn norm2(&self) -> f64 {
        self.index[..self.count as usize].iter().map(|&i| self.array[i] * self.array[i]).sum()
    }

    /// The listed indices.
    pub fn indices(&self) -> &[usize] {
        &self.index[..self.count.max(0) as usize]
    }
}
