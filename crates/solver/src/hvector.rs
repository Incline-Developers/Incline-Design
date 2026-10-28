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
        // Most vectors never need a hyper-sparse solve or a packed update.
        // Allocate that scratch space only when the operation needs it.
        self.cwork.clear();
        self.iwork.clear();
        self.pack_count = 0;
        self.pack_index.clear();
        self.pack_value.clear();
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
        let count = self.count as usize;
        if self.pack_index.len() < count {
            self.pack_index.resize(count, 0);
            self.pack_value.resize(count, 0.0);
        }
        for k in 0..count {
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
        let array = &self.array[..self.size];
        let end = self.size / 8 * 8;
        // Classify a block together, then visit only its nonzeros. The mask
        // keeps ascending row order without branching on each array entry.
        for (block, values) in array[..end].chunks_exact(8).enumerate() {
            let mut mask = u8::from(values[0] != 0.0)
                | (u8::from(values[1] != 0.0) << 1)
                | (u8::from(values[2] != 0.0) << 2)
                | (u8::from(values[3] != 0.0) << 3)
                | (u8::from(values[4] != 0.0) << 4)
                | (u8::from(values[5] != 0.0) << 5)
                | (u8::from(values[6] != 0.0) << 6)
                | (u8::from(values[7] != 0.0) << 7);
            while mask != 0 {
                index[count] = block * 8 + mask.trailing_zeros() as usize;
                count += 1;
                mask &= mask - 1;
            }
        }
        for (i, &value) in array.iter().enumerate().skip(end) {
            index[count] = i;
            count += usize::from(value != 0.0);
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
