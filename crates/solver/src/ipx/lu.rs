//! IPX's `BasicLu` (`basiclu_wrapper`) behind its `LuUpdate` interface:
//! BASICLU with the memory it asks for, and IPX's reading of its
//! stability measures.

use super::{
    basiclu::{ERROR_SINGULAR_UPDATE, Lu},
    sparse::{IndexedVector, Int, SparseMatrix, remove_diagonal},
};

/// Columns of the active submatrix whose largest entry is no larger are
/// dropped at once when the LU factorization is used for rank detection.
pub(crate) const LU_DEPENDENCY_TOL: f64 = 1e-3;
/// A fresh factorization this inaccurate is unstable.
const LU_STABILITY_THRESHOLD: f64 = 1e-12;
/// An update whose new diagonal entry of U errs relatively by more is
/// unstable.
const FT_DIAG_ERROR_TOL: f64 = 1e-8;
const REALLOC_FACTOR: f64 = 1.5;

/// `(L, U, rowperm, colperm, dependent_cols)`, as [`BasicLu::get_factors`]
/// returns them.
pub(crate) type LuFactors = (Option<SparseMatrix>, Option<SparseMatrix>, Vec<Int>, Vec<Int>, Vec<Int>);

#[derive(Clone, Debug)]
pub(crate) struct BasicLu {
    lu: Lu,
    fill_factor: f64,
    /// Updates since the last factorization.
    updates: Int,
}

impl BasicLu {
    pub fn new(dim: Int) -> Self {
        Self {
            lu: Lu::new(dim),
            fill_factor: 0.0,
            updates: 0,
        }
    }

    /// Factor the columns `[bbegin[j], bend[j])`; returns 1 if the
    /// factorization is unstable, plus 2 if the matrix is singular.
    pub fn factorize(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], bx: &[f64], strict_abs_pivottol: bool) -> Int {
        self.updates = 0;
        self.lu.abstol = if strict_abs_pivottol { LU_DEPENDENCY_TOL } else { 1e-14 };
        let mut ncall = 0;
        let status = loop {
            let status = self.lu.factorize(bbegin, bend, bi, bx, ncall > 0);
            if status != 1 {
                break status;
            }
            self.reallocate();
            ncall += 1;
        };
        assert!(status == 0 || status == 2, "basiclu_factorize failed with status {status}");
        let matrix_nz = self.lu.matrix_nz;
        let dim = self.lu.m;
        self.fill_factor = (self.lu.lnz + self.lu.unz + dim) as f64 / matrix_nz as f64;
        let mut ret = 0;
        if self.lu.residual_test > LU_STABILITY_THRESHOLD {
            ret |= 1;
        }
        if status == 2 {
            ret |= 2;
        }
        ret
    }

    /// L (unit diagonal removed) and U, the row and column permutations,
    /// and the positions of the dependent columns (at the end of the
    /// pivot sequence).
    pub fn get_factors(&mut self, want_l: bool, want_u: bool) -> LuFactors {
        let dim = self.lu.m;
        let (rowperm, colperm, l, uf) = self.lu.get_factors(want_l, want_u);
        let to_matrix = |(colptr, rowidx, values): (Vec<Int>, Vec<Int>, Vec<f64>)| {
            let mut a = SparseMatrix::new(dim, dim);
            a.colptr = colptr;
            a.rowidx = rowidx;
            a.values = values;
            a
        };
        let l = l.map(|factors| {
            let mut l = to_matrix(factors);
            remove_diagonal(&mut l, None);
            l
        });
        let uf = uf.map(to_matrix);
        let dependent_cols = (self.lu.rank..dim).collect();
        (l, uf, rowperm, colperm, dependent_cols)
    }

    pub fn solve_dense(&mut self, rhs: &[f64], lhs: &mut [f64], trans: char) {
        let status = self.lu.solve_dense(rhs, lhs, trans);
        assert_eq!(status, 0, "basiclu_solve_dense failed");
    }

    pub fn solve_dense_in_place(&mut self, lhs: &mut [f64], trans: char) {
        let status = self.lu.solve_dense_in_place(lhs, trans);
        assert_eq!(status, 0, "basiclu_solve_dense failed");
    }

    pub fn solve_dense_vec(&mut self, lhs: &mut Vec<f64>, trans: char) {
        let status = self.lu.solve_dense_vec(lhs, trans);
        assert_eq!(status, 0, "basiclu_solve_dense failed");
    }

    /// FTRAN of the column `bi`/`bx` entering the basis, keeping what the
    /// update needs, and its solution in `lhs` if given.
    pub fn ftran_for_update(&mut self, nzrhs: Int, bi: &[Int], bx: &[f64], lhs: Option<&mut IndexedVector>) {
        match lhs {
            None => loop {
                let status = self.lu.solve_for_update(nzrhs, bi, bx, None, 'N');
                if status != 1 {
                    assert_eq!(status, 0, "basiclu_solve_for_update (ftran without lhs) failed");
                    return;
                }
                self.reallocate();
            },
            Some(lhs) => {
                lhs.set_to_zero();
                let mut nzlhs = 0;
                loop {
                    let status = self.lu.solve_for_update(nzrhs, bi, bx, Some((&mut nzlhs, &mut lhs.pattern, &mut lhs.elements)), 'N');
                    if status != 1 {
                        assert_eq!(status, 0, "basiclu_solve_for_update (ftran with lhs) failed");
                        break;
                    }
                    self.reallocate();
                }
                lhs.nnz = nzlhs;
            }
        }
    }

    /// BTRAN of the unit row `j` of the column leaving the basis, keeping
    /// what the update needs, and its solution in `lhs` if given.
    pub fn btran_for_update(&mut self, j: Int, lhs: Option<&mut IndexedVector>) {
        match lhs {
            None => loop {
                let status = self.lu.solve_for_update(0, &[j], &[], None, 'T');
                if status != 1 {
                    assert_eq!(status, 0, "basiclu_solve_for_update (btran without lhs) failed");
                    return;
                }
                self.reallocate();
            },
            Some(lhs) => {
                lhs.set_to_zero();
                let mut nzlhs = 0;
                loop {
                    let status = self.lu.solve_for_update(0, &[j], &[], Some((&mut nzlhs, &mut lhs.pattern, &mut lhs.elements)), 'T');
                    if status != 1 {
                        assert_eq!(status, 0, "basiclu_solve_for_update (btran with lhs) failed");
                        break;
                    }
                    self.reallocate();
                }
                lhs.nnz = nzlhs;
            }
        }
    }

    /// Update the factors after the FTRAN and BTRAN; returns -1 for a
    /// singular update, 1 for an unstable one, 0 otherwise.
    pub fn update(&mut self, pivot: f64) -> Int {
        self.updates += 1;
        let status = loop {
            let status = self.lu.update(pivot);
            if status != 1 {
                break status;
            }
            self.reallocate();
        };
        assert!(status == 0 || status == ERROR_SINGULAR_UPDATE, "basiclu_update failed with status {status}");
        if status == ERROR_SINGULAR_UPDATE {
            return -1;
        }
        if self.lu.pivot_error > FT_DIAG_ERROR_TOL {
            return 1;
        }
        0
    }

    pub fn need_fresh_factorization(&self) -> bool {
        self.lu.nforrest == self.lu.m || self.lu.update_cost() > 1.0
    }

    pub fn fill_factor(&self) -> f64 {
        self.fill_factor
    }

    pub fn pivottol(&self) -> f64 {
        self.lu.reltol
    }

    pub fn set_pivottol(&mut self, new_pivottol: f64) {
        self.lu.reltol = new_pivottol;
    }

    pub fn updates(&self) -> Int {
        self.updates
    }

    fn reallocate(&mut self) {
        let grow = |len: usize, add: Int| ((len as i64 + i64::from(add)) as f64 * REALLOC_FACTOR) as usize;
        if self.lu.addmem_l > 0 {
            let new_size = grow(self.lu.li.len(), self.lu.addmem_l);
            self.lu.li.resize(new_size, 0);
            self.lu.lx.resize(new_size, 0.0);
        }
        if self.lu.addmem_u > 0 {
            let new_size = grow(self.lu.ui.len(), self.lu.addmem_u);
            self.lu.ui.resize(new_size, 0);
            self.lu.ux.resize(new_size, 0.0);
        }
        if self.lu.addmem_w > 0 {
            let new_size = grow(self.lu.wi.len(), self.lu.addmem_w);
            self.lu.wi.resize(new_size, 0);
            self.lu.wx.resize(new_size, 0.0);
        }
    }
}
