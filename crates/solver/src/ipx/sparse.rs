//! IPX's sparse matrix, indexed vector and vector utilities
//! (`sparse_matrix`, `indexed_vector`, `utils`, `sparse_utils`).
//!
//! IPX's `Int` is HiGHS's `HighsInt`, 32 bits (BASICLU's `lu_int` too),
//! signed since its index arrays carry negative markers.

use crate::pdqsort::pdqsort;

pub(crate) type Int = i32;

#[inline(always)]
pub(crate) fn u(x: Int) -> usize {
    x as usize
}

/// A vector is treated sparse if it has no more than this fraction of
/// its dimension in nonzeros (IPX's `kHypersparseThreshold`).
pub(crate) const HYPERSPARSE_THRESHOLD: f64 = 0.1;

/// A dense vector with an optional list of its nonzeros' positions.
#[derive(Clone, Debug, Default)]
pub(crate) struct IndexedVector {
    pub elements: Vec<f64>,
    pub pattern: Vec<Int>,
    /// The number of entries in `pattern`, or -1 when it is not known.
    pub nnz: Int,
}

impl IndexedVector {
    pub fn new(dim: usize) -> Self {
        Self {
            elements: vec![0.0; dim],
            pattern: vec![0; dim],
            nnz: 0,
        }
    }

    pub fn dim(&self) -> usize {
        self.elements.len()
    }

    pub fn sparse(&self) -> bool {
        self.nnz >= 0 && self.nnz as f64 <= HYPERSPARSE_THRESHOLD * self.dim() as f64
    }

    pub fn invalidate_pattern(&mut self) {
        self.nnz = -1;
    }

    pub fn set_to_zero(&mut self) {
        if self.sparse() {
            for p in 0..u(self.nnz) {
                self.elements[u(self.pattern[p])] = 0.0;
            }
        } else {
            self.elements.fill(0.0);
        }
        self.nnz = 0;
    }

    /// IPX's `for_each_nonzero`: each (index, value), by the pattern if the
    /// vector is sparse, otherwise every entry.
    pub fn for_each_nonzero(&self, mut f: impl FnMut(usize, f64)) {
        if self.sparse() {
            for p in 0..u(self.nnz) {
                let i = u(self.pattern[p]);
                f(i, self.elements[i]);
            }
        } else {
            for (i, &x) in self.elements.iter().enumerate() {
                f(i, x);
            }
        }
    }
}

impl IndexedVector {
    /// `for_each_nonzero` with each entry mutable.
    pub fn for_each_nonzero_mut(&mut self, mut f: impl FnMut(usize, &mut f64)) {
        if self.sparse() {
            for p in 0..u(self.nnz) {
                let i = u(self.pattern[p]);
                f(i, &mut self.elements[i]);
            }
        } else {
            for (i, x) in self.elements.iter_mut().enumerate() {
                f(i, x);
            }
        }
    }
}

pub(crate) fn dot_indexed(x: &IndexedVector, y: &[f64]) -> f64 {
    let mut d = 0.0;
    x.for_each_nonzero(|p, f| d += y[p] * f);
    d
}

/// IPX's column-wise sparse matrix, with a queue for building a column.
#[derive(Clone, Debug)]
pub(crate) struct SparseMatrix {
    nrow: Int,
    pub colptr: Vec<Int>,
    pub rowidx: Vec<Int>,
    pub values: Vec<f64>,
    rowidx_queue: Vec<Int>,
    values_queue: Vec<f64>,
}

impl Default for SparseMatrix {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl SparseMatrix {
    pub fn new(nrow: Int, ncol: Int) -> Self {
        Self::with_capacity(nrow, ncol, 0)
    }

    pub fn with_capacity(nrow: Int, ncol: Int, min_capacity: Int) -> Self {
        let mut a = Self {
            nrow: 0,
            colptr: Vec::new(),
            rowidx: Vec::new(),
            values: Vec::new(),
            rowidx_queue: Vec::new(),
            values_queue: Vec::new(),
        };
        a.resize(nrow, ncol, min_capacity);
        a
    }

    pub fn rows(&self) -> Int {
        self.nrow
    }
    pub fn cols(&self) -> Int {
        self.colptr.len() as Int - 1
    }
    pub fn entries(&self) -> Int {
        *self.colptr.last().unwrap()
    }
    pub fn entries_of(&self, j: Int) -> Int {
        self.end(j) - self.begin(j)
    }
    pub fn capacity(&self) -> Int {
        self.rowidx.len() as Int
    }
    pub fn begin(&self, j: Int) -> Int {
        self.colptr[u(j)]
    }
    pub fn end(&self, j: Int) -> Int {
        self.colptr[u(j) + 1]
    }
    pub fn index(&self, pos: Int) -> Int {
        self.rowidx[u(pos)]
    }
    pub fn value(&self, pos: Int) -> f64 {
        self.values[u(pos)]
    }

    pub fn reserve(&mut self, min_capacity: Int) {
        if min_capacity > self.capacity() {
            self.rowidx.resize(u(min_capacity), 0);
            self.values.resize(u(min_capacity), 0.0);
        }
    }

    pub fn resize(&mut self, nrow: Int, ncol: Int, min_capacity: Int) {
        self.nrow = nrow;
        self.colptr.clear();
        self.colptr.resize(u(ncol) + 1, 0);
        self.rowidx.clear();
        self.rowidx.resize(u(min_capacity), 0);
        self.values.clear();
        self.values.resize(u(min_capacity), 0.0);
    }

    pub fn clear(&mut self) {
        self.resize(0, 0, 0);
    }

    /// The columns `[abegin[j], aend[j])` of `ai`/`ax`, without zeros,
    /// with each column's indices sorted.
    pub fn load_from_arrays(&mut self, nrow: Int, ncol: Int, abegin: &[Int], aend: &[Int], ai: &[Int], ax: &[f64]) {
        let mut nz = 0;
        for j in 0..u(ncol) {
            nz += aend[j] - abegin[j];
        }
        self.resize(nrow, ncol, nz);
        let mut put = 0;
        for j in 0..u(ncol) {
            self.colptr[j] = put;
            for p in abegin[j]..aend[j] {
                if ax[u(p)] != 0.0 {
                    self.rowidx[u(put)] = ai[u(p)];
                    self.values[u(put)] = ax[u(p)];
                    put += 1;
                }
            }
        }
        self.colptr[u(ncol)] = put;
        self.sort_indices();
    }

    pub fn sort_indices(&mut self) {
        if self.is_sorted() {
            return;
        }
        let mut work: Vec<(Int, f64)> = Vec::new();
        for j in 0..self.cols() {
            let (begin, end) = (u(self.begin(j)), u(self.end(j)));
            work.clear();
            work.extend((begin..end).map(|p| (self.rowidx[p], self.values[p])));
            // Indices within a column are distinct, so any sort agrees.
            work.sort_unstable_by_key(|entry| entry.0);
            for (k, p) in (begin..end).enumerate() {
                self.rowidx[p] = work[k].0;
                self.values[p] = work[k].1;
            }
        }
    }

    fn is_sorted(&self) -> bool {
        for j in 0..self.cols() {
            let (begin, end) = (u(self.begin(j)), u(self.end(j)));
            for p in begin..end.saturating_sub(1).max(begin) {
                if self.rowidx[p] > self.rowidx[p + 1] {
                    return false;
                }
            }
        }
        true
    }

    pub fn push_back(&mut self, i: Int, x: f64) {
        self.rowidx_queue.push(i);
        self.values_queue.push(x);
    }

    pub fn queue_size(&self) -> Int {
        self.rowidx_queue.len() as Int
    }

    pub fn add_column(&mut self) {
        let nz = self.entries();
        let nznew = nz + self.queue_size();
        self.reserve(nznew);
        self.rowidx[u(nz)..u(nznew)].copy_from_slice(&self.rowidx_queue);
        self.values[u(nz)..u(nznew)].copy_from_slice(&self.values_queue);
        self.colptr.push(nznew);
        self.clear_queue();
    }

    pub fn clear_queue(&mut self) {
        self.rowidx_queue.clear();
        self.values_queue.clear();
    }
}

pub(crate) fn transpose(a: &SparseMatrix) -> SparseMatrix {
    let mut at = SparseMatrix::default();
    transpose_into(a, &mut at);
    at
}

pub(crate) fn transpose_into(a: &SparseMatrix, at: &mut SparseMatrix) {
    let m = u(a.rows());
    let n = a.cols();
    let nz = a.entries();
    at.resize(n, m as Int, nz);
    let mut work = vec![0 as Int; m];
    for p in 0..u(nz) {
        work[u(a.rowidx[p])] += 1;
    }
    let mut sum = 0;
    for i in 0..m {
        at.colptr[i] = sum;
        sum += work[i];
        work[i] = at.colptr[i];
    }
    at.colptr[m] = sum;
    for j in 0..n {
        for p in a.begin(j)..a.end(j) {
            let i = u(a.rowidx[u(p)]);
            let put = u(work[i]);
            work[i] += 1;
            at.rowidx[put] = j;
            at.values[put] = a.values[u(p)];
        }
    }
}

pub(crate) fn copy_columns(a: &SparseMatrix, cols: &[Int]) -> SparseMatrix {
    let mut a2 = SparseMatrix::new(a.rows(), 0);
    for &j in cols {
        for p in a.begin(j)..a.end(j) {
            a2.push_back(a.index(p), a.value(p));
        }
        a2.add_column();
    }
    a2
}

pub(crate) fn permute_rows(a: &mut SparseMatrix, perm: &[Int]) {
    for p in 0..u(a.entries()) {
        a.rowidx[p] = perm[u(a.rowidx[p])];
    }
}

pub(crate) fn scale_column(a: &mut SparseMatrix, j: Int, s: f64) {
    for p in a.begin(j)..a.end(j) {
        a.values[u(p)] *= s;
    }
}

/// Remove the diagonal entries of the square `a`, storing them in `diag`
/// if given; returns how many there were.
pub(crate) fn remove_diagonal(a: &mut SparseMatrix, mut diag: Option<&mut [f64]>) -> Int {
    let ncol = u(a.cols());
    let mut get = 0usize;
    let mut put = 0usize;
    for j in 0..ncol {
        if let Some(diag) = diag.as_deref_mut() {
            diag[j] = 0.0;
        }
        let end = u(a.colptr[j + 1]);
        a.colptr[j] = put as Int;
        while get < end {
            if a.rowidx[get] == j as Int {
                if let Some(diag) = diag.as_deref_mut() {
                    diag[j] = a.values[get];
                }
            } else {
                a.rowidx[put] = a.rowidx[get];
                a.values[put] = a.values[get];
                put += 1;
            }
            get += 1;
        }
    }
    a.colptr[ncol] = put as Int;
    (get - put) as Int
}

/// Column `j`'s row indices and values.
#[inline(always)]
fn column(a: &SparseMatrix, j: Int) -> (&[Int], &[f64]) {
    let range = u(a.colptr[u(j)])..u(a.colptr[u(j) + 1]);
    (&a.rowidx[range.clone()], &a.values[range])
}

/// `sum x[index[k]] * value[k]` in order.
#[inline(always)]
fn gather_dot(index: &[Int], value: &[f64], x: &[f64]) -> f64 {
    let mut d = 0.0;
    for (&i, &v) in index.iter().zip(value) {
        d += x[u(i)] * v;
    }
    d
}

/// `x[index[k]] -= value[k] * temp`.
#[inline(always)]
fn scatter_sub(index: &[Int], value: &[f64], temp: f64, x: &mut [f64]) {
    for (&i, &v) in index.iter().zip(value) {
        x[u(i)] -= v * temp;
    }
}

pub(crate) fn dot_column(a: &SparseMatrix, j: Int, rhs: &[f64]) -> f64 {
    let (index, value) = column(a, j);
    gather_dot(index, value, rhs)
}

pub(crate) fn scatter_column(a: &SparseMatrix, j: Int, alpha: f64, lhs: &mut [f64]) {
    let (index, value) = column(a, j);
    for (&i, &v) in index.iter().zip(value) {
        lhs[u(i)] += alpha * v;
    }
}

/// `lhs += alpha * op(A) * rhs`, `op` the transpose with `trans` 'T'.
pub(crate) fn multiply_add(a: &SparseMatrix, rhs: &[f64], alpha: f64, lhs: &mut [f64], trans: char) {
    let n = a.cols();
    if trans == 't' || trans == 'T' {
        for j in 0..n {
            lhs[u(j)] += alpha * dot_column(a, j, rhs);
        }
    } else {
        for j in 0..n {
            scatter_column(a, j, alpha * rhs[u(j)], lhs);
        }
    }
}

/// `lhs += A * D^2 * A' * rhs`, `D` the identity if not given.
pub(crate) fn add_normal_product(a: &SparseMatrix, d: Option<&[f64]>, rhs: &[f64], lhs: &mut [f64]) {
    for j in 0..a.cols() {
        let mut temp = dot_column(a, j, rhs);
        if let Some(d) = d {
            temp *= d[u(j)] * d[u(j)];
        }
        scatter_column(a, j, temp, lhs);
    }
}

/// Solve with the triangular `a` in place; `upper` for an upper factor,
/// `unitdiag` when its diagonal is implicit (otherwise each column's
/// diagonal entry is its last if upper, first if lower). Returns the
/// number of nonzeros in the solution.
pub(crate) fn triangular_solve(a: &SparseMatrix, x: &mut [f64], trans: char, upper: bool, unitdiag: bool) -> Int {
    let ncol = a.cols();
    let ap = &a.colptr;
    let ai = &a.rowidx;
    let ax = &a.values;
    let unit = Int::from(unitdiag);
    let mut nz = 0;
    if trans == 't' || trans == 'T' {
        if upper {
            for i in 0..ncol {
                let begin = ap[u(i)];
                let end = ap[u(i) + 1] - (1 - unit);
                let d = gather_dot(&ai[u(begin)..u(end)], &ax[u(begin)..u(end)], x);
                x[u(i)] -= d;
                if !unitdiag {
                    x[u(i)] /= ax[u(end)];
                }
                if x[u(i)] != 0.0 {
                    nz += 1;
                }
            }
        } else {
            for i in (0..ncol).rev() {
                let begin = ap[u(i)] + (1 - unit);
                let end = ap[u(i) + 1];
                let d = gather_dot(&ai[u(begin)..u(end)], &ax[u(begin)..u(end)], x);
                x[u(i)] -= d;
                if !unitdiag {
                    x[u(i)] /= ax[u(begin - 1)];
                }
                if x[u(i)] != 0.0 {
                    nz += 1;
                }
            }
        }
    } else if upper {
        for j in (0..ncol).rev() {
            let begin = ap[u(j)];
            let end = ap[u(j) + 1] - (1 - unit);
            if !unitdiag {
                x[u(j)] /= ax[u(end)];
            }
            let temp = x[u(j)];
            if temp != 0.0 {
                scatter_sub(&ai[u(begin)..u(end)], &ax[u(begin)..u(end)], temp, x);
                nz += 1;
            }
        }
    } else {
        for j in 0..ncol {
            let begin = ap[u(j)] + (1 - unit);
            let end = ap[u(j) + 1];
            if !unitdiag {
                x[u(j)] /= ax[u(begin - 1)];
            }
            let temp = x[u(j)];
            if temp != 0.0 {
                scatter_sub(&ai[u(begin)..u(end)], &ax[u(begin)..u(end)], temp, x);
                nz += 1;
            }
        }
    }
    nz
}

pub(crate) fn forward_solve(l: &SparseMatrix, uf: &SparseMatrix, x: &mut [f64]) {
    triangular_solve(l, x, 'n', false, true);
    triangular_solve(uf, x, 'n', true, false);
}

pub(crate) fn backward_solve(l: &SparseMatrix, uf: &SparseMatrix, x: &mut [f64]) {
    triangular_solve(uf, x, 't', true, false);
    triangular_solve(l, x, 't', false, true);
}

pub(crate) fn onenorm_matrix(a: &SparseMatrix) -> f64 {
    let mut norm: f64 = 0.0;
    for j in 0..a.cols() {
        let mut colsum = 0.0;
        for p in a.begin(j)..a.end(j) {
            colsum += a.value(p).abs();
        }
        norm = norm.max(colsum);
    }
    norm
}

pub(crate) fn infnorm_matrix(a: &SparseMatrix) -> f64 {
    let mut rowsum = vec![0.0; u(a.rows())];
    for j in 0..a.cols() {
        for p in a.begin(j)..a.end(j) {
            rowsum[u(a.index(p))] += a.value(p).abs();
        }
    }
    infnorm(&rowsum)
}

/// An estimate of the 1-norm of the inverse of the triangular `a`.
pub(crate) fn normest_inverse(a: &SparseMatrix, upper: bool, unitdiag: bool) -> f64 {
    let m = a.rows();
    let mut x = vec![0.0; u(m)];
    if upper {
        for j in 0..m {
            let begin = a.begin(j);
            let mut end = a.end(j);
            if !unitdiag {
                end -= 1;
            }
            let mut temp = 0.0;
            for p in begin..end {
                temp -= x[u(a.index(p))] * a.value(p);
            }
            temp += if temp >= 0.0 { 1.0 } else { -1.0 };
            if !unitdiag {
                temp /= a.value(end);
            }
            x[u(j)] = temp;
        }
    } else {
        for j in (0..m).rev() {
            let mut begin = a.begin(j);
            let end = a.end(j);
            if !unitdiag {
                begin += 1;
            }
            let mut temp = 0.0;
            for p in begin..end {
                temp -= x[u(a.index(p))] * a.value(p);
            }
            temp += if temp >= 0.0 { 1.0 } else { -1.0 };
            if !unitdiag {
                temp /= a.value(begin - 1);
            }
            x[u(j)] = temp;
        }
    }
    let x1norm = onenorm(&x);
    let xinfnorm = infnorm(&x);
    triangular_solve(a, &mut x, 'n', upper, unitdiag);
    let y1norm = onenorm(&x);
    (y1norm / x1norm).max(xinfnorm)
}

// ---- Vectors ---------------------------------------------------------------

pub(crate) fn all_finite(x: &[f64]) -> bool {
    x.iter().all(|xi| xi.is_finite())
}

pub(crate) fn onenorm(x: &[f64]) -> f64 {
    let mut norm = 0.0;
    for xi in x {
        norm += xi.abs();
    }
    norm
}

pub(crate) fn twonorm(x: &[f64]) -> f64 {
    let mut norm = 0.0;
    for xi in x {
        norm += xi * xi;
    }
    norm.sqrt()
}

/// `std::max` over |x|, which keeps the larger unless NaN comes second.
pub(crate) fn infnorm(x: &[f64]) -> f64 {
    let mut norm: f64 = 0.0;
    for xi in x {
        let v = xi.abs();
        if norm < v {
            norm = v;
        }
    }
    norm
}

pub(crate) fn dot(x: &[f64], y: &[f64]) -> f64 {
    let mut d = 0.0;
    for i in 0..x.len() {
        d += x[i] * y[i];
    }
    d
}

pub(crate) fn find_max_abs(x: &[f64]) -> Int {
    let mut xmax = 0.0;
    let mut imax = 0;
    for (i, xi) in x.iter().enumerate() {
        if xi.abs() > xmax {
            xmax = xi.abs();
            imax = i;
        }
    }
    imax as Int
}

/// `lhs[permuted_index[i]] = rhs[i]`.
pub(crate) fn permute(permuted_index: &[Int], rhs: &[f64], lhs: &mut [f64]) {
    for (i, &to) in permuted_index.iter().enumerate() {
        lhs[u(to)] = rhs[i];
    }
}

/// `lhs[i] = rhs[permuted_index[i]]`.
pub(crate) fn permute_back(permuted_index: &[Int], rhs: &[f64], lhs: &mut [f64]) {
    for (i, &from) in permuted_index.iter().enumerate() {
        lhs[i] = rhs[u(from)];
    }
}

pub(crate) fn inverse_perm(perm: &[Int]) -> Vec<Int> {
    let mut invperm = vec![0; perm.len()];
    for (i, &p) in perm.iter().enumerate() {
        invperm[u(p)] = i as Int;
    }
    invperm
}

/// The indices `0..m` sorted by `values` (then index), descending with
/// `reverse`; the identity if there are no values.
pub(crate) fn sortperm(m: Int, values: Option<&[f64]>, reverse: bool) -> Vec<Int> {
    let mut perm: Vec<Int> = (0..m).collect();
    let Some(values) = values else {
        return perm;
    };
    // `std::pair`'s ordering, NaNs included.
    let less = |a: (f64, Int), b: (f64, Int)| a.0 < b.0 || (b.0.partial_cmp(&a.0).is_none_or(std::cmp::Ordering::is_ge) && a.1 < b.1);
    if reverse {
        pdqsort(&mut perm, |&i, &j| less((values[u(j)], j), (values[u(i)], i)));
    } else {
        pdqsort(&mut perm, |&i, &j| less((values[u(i)], i), (values[u(j)], j)));
    }
    perm
}

// ---- Graph searches ----------------------------------------------------------

/// IPX's `DepthFirstSearch` from `istart` through the columns `colmap`
/// maps rows to (a row mapping to a negative column has no edges),
/// writing finished nodes to `istack[..top]`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn depth_first_search(
    istart: Int,
    ap: &[Int],
    ai: &[Int],
    colmap: Option<&[Int]>,
    mut top: Int,
    istack: &mut [Int],
    marked: &mut [Int],
    marker: Int,
    pstack: &mut [Int],
) -> Int {
    let mut head: Int = 0;
    istack[0] = istart;
    while head >= 0 {
        let i = istack[u(head)];
        let j = colmap.map_or(i, |colmap| colmap[u(i)]);
        if marked[u(i)] != marker {
            marked[u(i)] = marker;
            pstack[u(head)] = if j >= 0 { ap[u(j)] } else { 0 };
        }
        let mut done = true;
        let pbeg = pstack[u(head)];
        let pend = if j >= 0 { ap[u(j) + 1] } else { 0 };
        for p in pbeg..pend {
            let inext = ai[u(p)];
            if marked[u(inext)] == marker {
                continue;
            }
            pstack[u(head)] = p + 1;
            head += 1;
            istack[u(head)] = inext;
            done = false;
            break;
        }
        if done {
            head -= 1;
            top -= 1;
            istack[u(top)] = i;
        }
    }
    top
}

/// IPX's `AugmentingPath` for a matching of columns to rows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn augmenting_path(
    jstart: Int,
    ap: &[Int],
    ai: &[Int],
    jmatch: &mut [Int],
    cheap: &mut [Int],
    marked: &mut [Int],
    istack: &mut [Int],
    jstack: &mut [Int],
    pstack: &mut [Int],
) -> bool {
    let mut found = false;
    let mut head: Int = 0;
    jstack[0] = jstart;
    while head >= 0 {
        let j = jstack[u(head)];
        if marked[u(j)] != jstart {
            marked[u(j)] = jstart;
            let mut i = 0;
            let mut p = cheap[u(j)];
            while p < ap[u(j) + 1] && !found {
                i = ai[u(p)];
                found = jmatch[u(i)] == -1;
                p += 1;
            }
            cheap[u(j)] = p;
            if found {
                istack[u(head)] = i;
                break;
            }
            pstack[u(head)] = ap[u(j)];
        }
        let pbeg = pstack[u(head)];
        let pend = ap[u(j) + 1];
        let mut p = pbeg;
        while p < pend {
            let i = ai[u(p)];
            if jmatch[u(i)] < -1 {
                p += 1;
                continue;
            }
            if marked[u(jmatch[u(i)])] == jstart {
                p += 1;
                continue;
            }
            pstack[u(head)] = p + 1;
            istack[u(head)] = i;
            head += 1;
            jstack[u(head)] = jmatch[u(i)];
            break;
        }
        if p == ap[u(j) + 1] {
            head -= 1;
        }
    }
    if found {
        let mut p = head;
        while p >= 0 {
            jmatch[u(istack[u(p)])] = jstack[u(p)];
            p -= 1;
        }
    }
    found
}
