//! IPX's `Basis` (`basis.cc`), with the crash basis it starts from
//! (`guess_basis.cc`) and the starting basis of the main IPM
//! (`starting_basis.cc`).

use super::{
    IPX_ERROR_BASIS_SINGULAR, IPX_ERROR_BASIS_TOO_ILL_CONDITIONED, Info,
    control::Control,
    iterate::{Iterate, State},
    lu::{BasicLu, LU_DEPENDENCY_TOL},
    model::Model,
    sparse::{
        HYPERSPARSE_THRESHOLD, IndexedVector, Int, SparseMatrix, all_finite, augmenting_path, depth_first_search, dot, dot_column, find_max_abs, scatter_column, sortperm, twonorm,
        u,
    },
};

const BASIS_REPAIR_THRESHOLD: f64 = 1e5;
const MAX_BASIS_REPAIR: Int = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BasicStatus {
    NonbasicFixed,
    Nonbasic,
    Basic,
    BasicFree,
}

pub(crate) struct Basis<'a> {
    control: &'a Control,
    model: &'a Model,
    basis: Vec<Int>,
    /// Position in the basis (plus `m` if free), -1 if nonbasic, -2 if
    /// nonbasic and fixed.
    map2basis: Vec<Int>,
    lu: BasicLu,
    factorization_is_fresh: bool,
    pub num_factorizations: Int,
    pub num_updates: Int,
}

impl<'a> Basis<'a> {
    pub fn new(control: &'a Control, model: &'a Model) -> Self {
        let m = model.rows();
        let n = model.cols();
        let mut lu = BasicLu::new(m);
        lu.set_pivottol(control.params.lu_pivottol);
        let mut basis = Basis {
            control,
            model,
            basis: vec![0; u(m)],
            map2basis: vec![0; u(n + m)],
            lu,
            factorization_is_fresh: false,
            num_factorizations: 0,
            num_updates: 0,
        };
        basis.set_to_slack_basis();
        basis
    }

    pub fn model(&self) -> &'a Model {
        self.model
    }

    pub fn at(&self, p: usize) -> Int {
        self.basis[p]
    }

    pub fn position_of(&self, j: Int) -> Int {
        let m = self.model.rows();
        let p = self.map2basis[u(j)];
        if p < 0 {
            -1
        } else if p < m {
            p
        } else {
            p - m
        }
    }

    pub fn status_of(&self, j: Int) -> BasicStatus {
        let m = self.model.rows();
        let p = self.map2basis[u(j)];
        if p < 0 {
            if p == -1 { BasicStatus::Nonbasic } else { BasicStatus::NonbasicFixed }
        } else if p < m {
            BasicStatus::Basic
        } else {
            BasicStatus::BasicFree
        }
    }

    pub fn is_basic(&self, j: Int) -> bool {
        matches!(self.status_of(j), BasicStatus::Basic | BasicStatus::BasicFree)
    }

    pub fn is_nonbasic(&self, j: Int) -> bool {
        matches!(self.status_of(j), BasicStatus::Nonbasic | BasicStatus::NonbasicFixed)
    }

    pub fn fix_nonbasic_variable(&mut self, j: Int) {
        if self.status_of(j) == BasicStatus::NonbasicFixed {
            return;
        }
        self.map2basis[u(j)] = -2;
    }

    pub fn free_basic_variable(&mut self, j: Int) {
        if self.status_of(j) == BasicStatus::BasicFree {
            return;
        }
        self.map2basis[u(j)] += self.model.rows();
    }

    pub fn unfix_variables(&mut self) {
        for p in &mut self.map2basis {
            if *p == -2 {
                *p = -1;
            }
        }
    }

    pub fn unfree_variables(&mut self) {
        let m = self.model.rows();
        for p in &mut self.map2basis {
            if *p >= m {
                *p -= m;
            }
        }
    }

    pub fn set_to_slack_basis(&mut self) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        for (i, basic) in self.basis[..m].iter_mut().enumerate() {
            *basic = (n + i) as Int;
        }
        for map in &mut self.map2basis[..n] {
            *map = -1;
        }
        for i in 0..m {
            self.map2basis[n + i] = i as Int;
        }
        let err = self.factorize();
        debug_assert_eq!(err, 0);
    }

    pub fn factorize(&mut self) -> Int {
        let m = u(self.model.rows());
        let ai = self.model.ai();
        let mut begin = vec![0; m];
        let mut end = vec![0; m];
        for i in 0..m {
            begin[i] = ai.begin(self.basis[i]);
            end[i] = ai.end(self.basis[i]);
        }
        let mut err = 0;
        loop {
            let flag = self.lu.factorize(&begin, &end, &ai.rowidx, &ai.values, false);
            self.num_factorizations += 1;
            if flag & 2 != 0 {
                self.adapt_to_singular_factorization();
                err = IPX_ERROR_BASIS_SINGULAR;
                break;
            }
            if flag & 1 != 0 && self.tighten_lu_pivot_tol() {
                continue;
            }
            break;
        }
        self.factorization_is_fresh = true;
        err
    }

    pub fn factorization_is_fresh(&self) -> bool {
        self.factorization_is_fresh
    }

    /// The fresh factorization's L and U and its row and column orders.
    pub fn get_lu_factors(&mut self) -> (SparseMatrix, SparseMatrix, Vec<Int>, Vec<Int>) {
        let (l, uf, rowperm, colperm, _) = self.lu.get_factors(true, true);
        (l.unwrap(), uf.unwrap(), rowperm, colperm)
    }

    pub fn solve_dense(&mut self, rhs: &[f64], lhs: &mut [f64], trans: char) {
        self.lu.solve_dense(rhs, lhs, trans);
    }

    pub fn solve_dense_in_place(&mut self, lhs: &mut [f64], trans: char) {
        self.lu.solve_dense_in_place(lhs, trans);
    }

    pub fn solve_dense_vec(&mut self, lhs: &mut Vec<f64>, trans: char) {
        self.lu.solve_dense_vec(lhs, trans);
    }

    /// FTRAN of nonbasic column `j`, or BTRAN of basic `j`'s row, with
    /// the solution in `lhs`, kept for an update.
    pub fn solve_for_update_into(&mut self, j: Int, lhs: &mut IndexedVector) {
        let p = self.position_of(j);
        if p < 0 {
            let ai = self.model.ai();
            let (begin, end) = (u(ai.begin(j)), u(ai.end(j)));
            self.lu.ftran_for_update((end - begin) as Int, &ai.rowidx[begin..end], &ai.values[begin..end], Some(lhs));
        } else {
            self.lu.btran_for_update(p, Some(lhs));
        }
    }

    /// The same, without the solution.
    pub fn solve_for_update(&mut self, j: Int) {
        let p = self.position_of(j);
        if p < 0 {
            let ai = self.model.ai();
            let (begin, end) = (u(ai.begin(j)), u(ai.end(j)));
            self.lu.ftran_for_update((end - begin) as Int, &ai.rowidx[begin..end], &ai.values[begin..end], None);
        } else {
            self.lu.btran_for_update(p, None);
        }
    }

    /// Row `jb` of the simplex tableau over the nonbasic columns (fixed
    /// ones left out with `ignore_fixed`), via its BTRAN in `btran`.
    pub fn tableau_row(&mut self, jb: Int, btran: &mut IndexedVector, row: &mut IndexedVector, ignore_fixed: bool) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        self.solve_for_update_into(jb, btran);
        let mut is_sparse = btran.sparse();
        let ait = self.model.ait();
        if is_sparse {
            let mut nz = 0;
            for pattern_k in &btran.pattern[..u(btran.nnz)] {
                let i = *pattern_k;
                nz += ait.end(i) - ait.begin(i);
            }
            nz /= 2;
            if nz as f64 > HYPERSPARSE_THRESHOLD * n as f64 {
                is_sparse = false;
            }
        }
        if is_sparse {
            row.set_to_zero();
            let mut nz = 0usize;
            for k in 0..u(btran.nnz) {
                let i = btran.pattern[k];
                let temp = btran.elements[u(i)];
                for p in ait.begin(i)..ait.end(i) {
                    let j = u(ait.rowidx[u(p)]);
                    if self.map2basis[j] == -1 || (self.map2basis[j] == -2 && !ignore_fixed) {
                        self.map2basis[j] -= 2;
                        row.pattern[nz] = j as Int;
                        nz += 1;
                    }
                    if self.map2basis[j] < -2 {
                        row.elements[j] += temp * ait.values[u(p)];
                    }
                }
            }
            for k in 0..nz {
                self.map2basis[u(row.pattern[k])] += 2;
            }
            row.nnz = nz as Int;
        } else {
            let ai = self.model.ai();
            for j in 0..n + m {
                let mut result = 0.0;
                if self.map2basis[j] == -1 || (self.map2basis[j] == -2 && !ignore_fixed) {
                    for p in ai.begin(j as Int)..ai.end(j as Int) {
                        result += ai.values[u(p)] * btran.elements[u(ai.rowidx[u(p)])];
                    }
                }
                row.elements[j] = result;
            }
            row.invalidate_pattern();
        }
    }

    /// Exchange `jb` out and `jn` into the basis, after the FTRAN (`sys`
    /// > 0) or BTRAN (`sys` < 0) still to be done, if the update is
    /// >    stable; otherwise refactor and leave `exchanged` false.
    pub fn exchange_if_stable(&mut self, jb: Int, jn: Int, tableau_entry: f64, sys: i32, exchanged: &mut bool) -> Int {
        if sys > 0 {
            self.solve_for_update(jn);
        }
        if sys < 0 {
            self.solve_for_update(jb);
        }
        *exchanged = false;
        let err = self.lu.update(tableau_entry);
        if err != 0 {
            if self.factorization_is_fresh && !self.tighten_lu_pivot_tol() {
                return IPX_ERROR_BASIS_TOO_ILL_CONDITIONED;
            }
            return self.factorize();
        }
        let ib = self.position_of(jb);
        self.basis[u(ib)] = jn;
        self.map2basis[u(jn)] = ib;
        self.map2basis[u(jb)] = -1;
        self.num_updates += 1;
        self.factorization_is_fresh = false;
        *exchanged = true;
        if self.lu.need_fresh_factorization() {
            return self.factorize();
        }
        0
    }

    /// The basic solution: `x` basic from the nonbasic `x`, and `y`, `z`
    /// with the basic `z` as given.
    pub fn compute_basic_solution(&mut self, x: &mut [f64], y: &mut [f64], z: &mut [f64]) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let (b, c, ai) = (self.model.b(), self.model.c(), self.model.ai());
        y.copy_from_slice(b);
        for (j, x_j) in x[..n + m].iter().enumerate() {
            if self.is_nonbasic(j as Int) {
                scatter_column(ai, j as Int, -*x_j, y);
            }
        }
        self.solve_dense_in_place(y, 'N');
        for p in 0..m {
            x[u(self.basis[p])] = y[p];
        }
        for p in 0..m {
            y[p] = c[u(self.basis[p])] - z[u(self.basis[p])];
        }
        self.solve_dense_in_place(y, 'T');
        for j in 0..n + m {
            if self.is_nonbasic(j as Int) {
                z[j] = c[j] - dot_column(ai, j as Int, y);
            }
        }
    }

    /// A basis preferring columns with large `colscale`: a crash basis
    /// repaired, or the slack basis, then free columns in and fixed ones
    /// out.
    pub fn construct_basis_from_weights(&mut self, colscale: &[f64], info: &mut Info) {
        info.errflag = 0;
        info.dependent_rows = 0;
        info.dependent_cols = 0;
        if self.control.params.crash_basis != 0 {
            self.crash_basis(colscale);
            // IPX logs this, so it is computed (and its solves run).
            let _ = self.min_singular_value();
            self.repair(info);
            if info.basis_repairs < 0 {
                self.set_to_slack_basis();
            } else if info.basis_repairs > 0 {
                let _ = self.min_singular_value();
            }
        } else {
            self.set_to_slack_basis();
        }
        self.pivot_free_variables_into_basis(colscale, info);
        if info.errflag != 0 {
            return;
        }
        self.pivot_fixed_variables_out_of_basis(colscale, info);
    }

    /// An estimate of the basis matrix's smallest singular value by the
    /// power method on its inverse.
    pub fn min_singular_value(&mut self) -> f64 {
        let m = u(self.model.rows());
        let mut v = vec![0.0; m];
        let lambda = power_method(
            |x, fx| {
                self.solve_dense(x, fx, 'N');
                self.solve_dense_in_place(fx, 'T');
            },
            &mut v,
        );
        (1.0 / lambda).sqrt()
    }

    fn adapt_to_singular_factorization(&mut self) -> Int {
        let n = self.model.cols();
        let (_, _, rowperm, colperm, dependent_cols) = self.lu.get_factors(false, false);
        for &k in &dependent_cols {
            let p = u(colperm[u(k)]);
            let i = rowperm[u(k)];
            let jb = self.basis[p];
            let jn = n + i;
            self.basis[p] = jn;
            self.map2basis[u(jn)] = p as Int;
            if jb >= 0 {
                self.map2basis[u(jb)] = -1;
            }
        }
        dependent_cols.len() as Int
    }

    fn tighten_lu_pivot_tol(&mut self) -> bool {
        let tol = self.lu.pivottol();
        if tol <= 0.05 {
            self.lu.set_pivottol(0.1);
        } else if tol <= 0.25 {
            self.lu.set_pivottol(0.3);
        } else if tol <= 0.5 {
            self.lu.set_pivottol(0.9);
        } else {
            return false;
        }
        true
    }

    fn crash_basis(&mut self, colweights: &[f64]) {
        let cols_guessed = guess_basis(self.model, colweights);
        self.basis.fill(-1);
        self.map2basis.fill(-1);
        for (k, &j) in cols_guessed.iter().enumerate() {
            self.basis[k] = j;
            self.map2basis[u(j)] = k as Int;
        }
        self.crash_factorize();
    }

    fn repair(&mut self, info: &mut Info) {
        let m = u(self.model.rows());
        let n = self.model.cols();
        let mut work = vec![0.0; m];
        info.basis_repairs = 0;
        loop {
            let (pmax, imax, pivot) = self.inverse_search(&mut work);
            if pmax < 0 || imax < 0 || !pivot.is_finite() {
                info.basis_repairs = -1;
                break;
            }
            if pivot.abs() < BASIS_REPAIR_THRESHOLD {
                break;
            }
            let jb = self.basis[u(pmax)];
            let jn = n + imax;
            if !self.is_nonbasic(jn) {
                info.basis_repairs = -2;
                break;
            }
            if info.basis_repairs >= MAX_BASIS_REPAIR {
                info.basis_repairs = -3;
                break;
            }
            self.solve_for_update(jb);
            self.solve_for_update(jn);
            self.crash_exchange(jb, jn, pivot, 0);
            info.basis_repairs += 1;
        }
    }

    /// IPX's `InverseSearch`: a large entry of the basis inverse.
    fn inverse_search(&mut self, work: &mut [f64]) -> (Int, Int, f64) {
        let m = work.len();
        let mut inverse_max = 0.0;
        for (i, work_i) in work[..m].iter_mut().enumerate() {
            *work_i = 1.0 / (i + 1) as f64;
        }
        loop {
            self.solve_dense_in_place(work, 'N');
            if !all_finite(work) {
                break;
            }
            let pmax = find_max_abs(work);
            work.fill(0.0);
            work[u(pmax)] = 1.0;
            self.solve_dense_in_place(work, 'T');
            if !all_finite(work) {
                break;
            }
            let imax = find_max_abs(work);
            let inverse_entry = work[u(imax)];
            if inverse_entry.abs() <= 2.0 * inverse_max {
                return (pmax, imax, inverse_entry);
            }
            inverse_max = inverse_entry.abs();
            work.fill(0.0);
            work[u(imax)] = 1.0;
        }
        (-1, -1, f64::INFINITY)
    }

    fn crash_factorize(&mut self) -> Int {
        let m = u(self.model.rows());
        let ai = self.model.ai();
        let mut begin = vec![0; m];
        let mut end = vec![0; m];
        for i in 0..m {
            if self.basis[i] >= 0 {
                begin[i] = ai.begin(self.basis[i]);
                end[i] = ai.end(self.basis[i]);
            }
        }
        let flag = self.lu.factorize(&begin, &end, &ai.rowidx, &ai.values, true);
        self.num_factorizations += 1;
        let mut ndropped = 0;
        if flag & 2 != 0 {
            ndropped = self.adapt_to_singular_factorization();
        }
        self.factorization_is_fresh = true;
        ndropped
    }

    fn crash_exchange(&mut self, jb: Int, jn: Int, tableau_entry: f64, sys: i32) {
        if sys > 0 {
            self.solve_for_update(jn);
        }
        if sys < 0 {
            self.solve_for_update(jb);
        }
        let ib = self.position_of(jb);
        self.basis[u(ib)] = jn;
        self.map2basis[u(jn)] = ib;
        self.map2basis[u(jb)] = -1;
        self.num_updates += 1;
        self.factorization_is_fresh = false;
        let err = self.lu.update(tableau_entry);
        if err != 0 || self.lu.need_fresh_factorization() {
            self.crash_factorize();
        }
    }

    fn pivot_free_variables_into_basis(&mut self, colweights: &[f64], info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let mut ftran = IndexedVector::new(m);
        let dependency_tol = self.control.params.dependency_tol.max(0.0);
        info.errflag = 0;
        info.dependent_cols = 0;
        let mut remaining: Vec<Int> = (0..n + m).filter(|&j| colweights[j].is_infinite() && self.map2basis[j] < 0).map(|j| j as Int).collect();
        while let Some(&jn) = remaining.last() {
            info.errflag = self.control.interrupt_check();
            if info.errflag != 0 {
                return;
            }
            self.solve_for_update_into(jn, &mut ftran);
            let mut pmax: Int = -1;
            let mut pmax_nonfree: Int = -1;
            let mut fmax = 0.0;
            let mut fmax_nonfree = 0.0;
            ftran.for_each_nonzero(|p, f| {
                let f = f.abs();
                if f > fmax {
                    fmax = f;
                    pmax = p as Int;
                }
                if !colweights[u(self.basis[p])].is_infinite() && f > fmax_nonfree {
                    fmax_nonfree = f;
                    pmax_nonfree = p as Int;
                }
            });
            if fmax > 4.0 && fmax_nonfree < 1.0 {
                let jb = self.basis[u(pmax)];
                let mut exchanged = false;
                info.errflag = self.exchange_if_stable(jb, jn, ftran.elements[u(pmax)], -1, &mut exchanged);
                if info.errflag != 0 {
                    return;
                }
                if !exchanged {
                    continue;
                }
                remaining.pop();
                remaining.push(jb);
                info.updates_start += 1;
            } else if fmax_nonfree <= dependency_tol {
                if !info.cols_inconsistent {
                    let c = self.model.c();
                    let mut delta_obj = c[u(jn)];
                    ftran.for_each_nonzero(|p, f| {
                        let j = u(self.basis[p]);
                        if colweights[j].is_infinite() {
                            delta_obj -= c[j] * f;
                        }
                    });
                    if delta_obj.abs() > dependency_tol {
                        info.cols_inconsistent = true;
                    }
                }
                info.dependent_cols += 1;
                remaining.pop();
            } else {
                let jb = self.basis[u(pmax_nonfree)];
                let mut exchanged = false;
                info.errflag = self.exchange_if_stable(jb, jn, ftran.elements[u(pmax_nonfree)], -1, &mut exchanged);
                if info.errflag != 0 {
                    return;
                }
                if !exchanged {
                    continue;
                }
                remaining.pop();
                info.updates_start += 1;
            }
        }
    }

    fn pivot_fixed_variables_out_of_basis(&mut self, colweights: &[f64], info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let (ai, lb, ub) = (self.model.ai(), self.model.lb(), self.model.ub());
        let mut btran = IndexedVector::new(m);
        let mut row = IndexedVector::new(n + m);
        let dependency_tol = self.control.params.dependency_tol.max(0.0);
        info.errflag = 0;
        info.dependent_rows = 0;
        let mut b_minus_fixed_columns = self.model.b().to_vec();
        for j in 0..n + m {
            if lb[j] == ub[j] && lb[j] != 0.0 {
                scatter_column(ai, j as Int, -lb[j], &mut b_minus_fixed_columns);
            }
        }
        let mut remaining: Vec<Int> = (n..n + m).filter(|&j| colweights[j] == 0.0 && self.map2basis[j] >= 0).map(|j| j as Int).collect();
        while let Some(&jb) = remaining.last() {
            info.errflag = self.control.interrupt_check();
            if info.errflag != 0 {
                return;
            }
            self.tableau_row(jb, &mut btran, &mut row, false);
            let mut jmax: Int = -1;
            let mut jmax_nonfixed: Int = -1;
            let mut rmax = 0.0;
            let mut rmax_nonfixed = 0.0;
            row.for_each_nonzero(|j, r| {
                if j >= n || colweights[j] != 0.0 {
                    let r = r.abs();
                    if r > rmax {
                        rmax = r;
                        jmax = j as Int;
                    }
                    if colweights[j] != 0.0 && r > rmax_nonfixed {
                        rmax_nonfixed = r;
                        jmax_nonfixed = j as Int;
                    }
                }
            });
            let _ = jmax_nonfixed;
            if rmax > 4.0 && rmax_nonfixed < 1.0 {
                let mut exchanged = false;
                info.errflag = self.exchange_if_stable(jb, jmax, row.elements[u(jmax)], 1, &mut exchanged);
                if info.errflag != 0 {
                    return;
                }
                if !exchanged {
                    continue;
                }
                remaining.pop();
                remaining.push(jmax);
                info.updates_start += 1;
            } else if rmax_nonfixed <= dependency_tol {
                if !info.rows_inconsistent {
                    let delta_obj = super::sparse::dot_indexed(&btran, &b_minus_fixed_columns);
                    if delta_obj.abs() > dependency_tol {
                        info.rows_inconsistent = true;
                    }
                }
                info.dependent_rows += 1;
                remaining.pop();
            } else {
                let mut jmax_scaled: Int = -1;
                let mut rmax_scaled = 0.0;
                row.for_each_nonzero(|j, r| {
                    let r = r.abs();
                    let rscaled = r * colweights[j];
                    if r >= 0.1 * rmax_nonfixed && rscaled > rmax_scaled {
                        rmax_scaled = rscaled;
                        jmax_scaled = j as Int;
                    }
                });
                let pivot = row.elements[u(jmax_scaled)];
                let mut exchanged = false;
                info.errflag = self.exchange_if_stable(jb, jmax_scaled, pivot, 1, &mut exchanged);
                if info.errflag != 0 {
                    return;
                }
                if !exchanged {
                    continue;
                }
                remaining.pop();
                info.updates_start += 1;
            }
        }
    }
}

/// `x` at the basic positions.
pub(crate) fn copy_basic(x: &[f64], basis: &Basis) -> Vec<f64> {
    (0..u(basis.model().rows())).map(|p| x[u(basis.at(p))]).collect()
}

/// IPX's `PowerMethod`: the largest eigenvalue of the operator `func`.
fn power_method(mut func: impl FnMut(&[f64], &mut [f64]), v: &mut [f64]) -> f64 {
    const MAXITER: Int = 100;
    const TOL: f64 = 1e-3;
    let dim = v.len();
    let mut fv = vec![0.0; dim];
    for (i, v_i) in v[..dim].iter_mut().enumerate() {
        *v_i = 1.0 + 1.0 / (i + 1) as f64;
    }
    let norm = twonorm(v);
    for vi in v.iter_mut() {
        *vi /= norm;
    }
    let mut lambda = 0.0;
    let mut iter = 0;
    loop {
        let more = iter < MAXITER;
        iter += 1;
        if !more {
            break;
        }
        func(v, &mut fv);
        let lambda_old = lambda;
        lambda = twonorm(&fv);
        for i in 0..dim {
            v[i] = fv[i] / lambda;
        }
        if (lambda - lambda_old).abs() <= TOL * lambda {
            break;
        }
    }
    lambda
}

// ---- GuessBasis ------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn compute_pattern(l: &SparseMatrix, ai: &SparseMatrix, j: Int, rownumber: &[Int], pattern: &mut [Int], pstack: &mut [Int], marked: &mut [Int], marker: Int) -> Int {
    let m = l.rows();
    let mut top = m;
    for p in ai.begin(j)..ai.end(j) {
        let i = ai.index(p);
        if marked[u(i)] != marker {
            top = depth_first_search(i, &l.colptr, &l.rowidx, Some(rownumber), top, pattern, marked, marker, pstack);
        }
    }
    top
}

fn compute_values(l: &SparseMatrix, ai: &SparseMatrix, j: Int, rownumber: &[Int], pattern: &[Int], top: Int, lhs: &mut [f64]) -> Int {
    let m = u(l.rows());
    for t in u(top)..m {
        lhs[u(pattern[t])] = 0.0;
    }
    for p in ai.begin(j)..ai.end(j) {
        lhs[u(ai.index(p))] = ai.value(p);
    }
    let mut lhsmax = 0.0;
    let mut imax: Int = -1;
    for pattern_t in &pattern[u(top)..m] {
        let i = u(*pattern_t);
        let temp = lhs[i];
        let k = rownumber[i];
        if temp != 0.0 {
            if k >= 0 {
                for p in l.colptr[u(k)]..l.colptr[u(k) + 1] {
                    lhs[u(l.rowidx[u(p)])] -= l.values[u(p)] * temp;
                }
            } else if temp.abs() > lhsmax {
                lhsmax = temp.abs();
                imax = i as Int;
            }
        }
    }
    imax
}

/// IPX's `GuessBasis`: free columns first, then singletons, then a
/// matching by weight, completed with slacks.
fn guess_basis(model: &Model, colweights: &[f64]) -> Vec<Int> {
    let m = u(model.rows());
    let n = u(model.cols());
    let ai = model.ai();
    let ait = model.ait();
    let mut basis: Vec<Int> = Vec::new();
    let mut rownumber = vec![-1 as Int; m];
    let mut active = vec![true; n + m];

    // Free columns.
    {
        let mut pattern = vec![0; m];
        let mut work = vec![0; m];
        let mut marked = vec![-1 as Int; m];
        let mut lhs = vec![0.0; m];
        let mut l = SparseMatrix::new(m as Int, 0);
        for j in 0..n + m {
            if colweights[j] != f64::INFINITY {
                continue;
            }
            let top = compute_pattern(&l, ai, j as Int, &rownumber, &mut pattern, &mut work, &mut marked, j as Int);
            let imax = compute_values(&l, ai, j as Int, &rownumber, &pattern, top, &mut lhs);
            let pivot = if imax >= 0 { lhs[u(imax)] } else { 0.0 };
            if pivot.abs() > LU_DEPENDENCY_TOL {
                rownumber[u(imax)] = basis.len() as Int;
                basis.push(j as Int);
                for t in ai.begin(j as Int)..ai.end(j as Int) {
                    let i = u(ai.index(t));
                    if rownumber[i] < 0 && lhs[i] != 0.0 {
                        l.push_back(i as Int, lhs[i] / pivot);
                    }
                }
                l.add_column();
            }
            active[j] = false;
        }
    }

    // Singletons.
    for (i, rownumber_i) in rownumber[..m].iter_mut().enumerate() {
        if *rownumber_i >= 0 {
            continue;
        }
        let mut rowmax: f64 = 0.0;
        let mut max_singleton = 0.0;
        let mut jsingleton: Int = -1;
        for p in ait.begin(i as Int)..ait.end(i as Int) {
            let j = u(ait.index(p));
            if !active[j] {
                continue;
            }
            let a = ait.value(p).abs() * colweights[j];
            if rowmax < a {
                rowmax = a;
            }
            if a > max_singleton && ai.end(j as Int) == ai.begin(j as Int) + 1 {
                max_singleton = a;
                jsingleton = j as Int;
            }
        }
        if max_singleton > 0.0 && max_singleton >= 0.5 * rowmax {
            *rownumber_i = basis.len() as Int;
            basis.push(jsingleton);
            active[u(jsingleton)] = false;
        }
    }

    // The rest, by weight.
    {
        let colperm = sortperm((n + m) as Int, Some(colweights), true);
        let mut jmatch = vec![-1 as Int; m];
        for i in 0..m {
            if rownumber[i] >= 0 {
                jmatch[i] = -2;
            }
        }
        let mut marked = vec![-1 as Int; n + m];
        let ap = &ai.colptr;
        let mut cheap: Vec<Int> = ap[..n + m].to_vec();
        let mut work = vec![0; m];
        let mut work2 = vec![0; m + 1];
        let mut work3 = vec![0; m + 1];
        let mut num_failed = 0;
        for &j in &colperm {
            if !active[u(j)] {
                continue;
            }
            if colweights[u(j)] == 0.0 {
                break;
            }
            let matched = augmenting_path(j, ap, &ai.rowidx, &mut jmatch, &mut cheap, &mut marked, &mut work, &mut work2, &mut work3);
            if matched {
                basis.push(j);
            } else {
                num_failed += 1;
            }
            if num_failed >= 10 * (m as Int - basis.len() as Int) {
                break;
            }
        }
        for i in 0..m {
            if jmatch[i] >= 0 {
                rownumber[i] = m as Int;
            }
        }
    }
    for (i, rownumber_i) in rownumber[..m].iter().enumerate() {
        if *rownumber_i < 0 {
            basis.push((n + i) as Int);
        }
    }
    basis
}

// ---- StartingBasis ---------------------------------------------------------

/// IPX's `StartingBasis`: the basis from the iterate's scaling factors,
/// with variables it fixes or frees, and dependent rows and columns made
/// consistent.
pub(crate) fn starting_basis(iterate: &mut Iterate<'_>, basis: &mut Basis<'_>, info: &mut Info) {
    let model = iterate.model();
    let m = u(model.rows());
    let n = u(model.cols());
    let (lb, ub) = (model.lb(), model.ub());
    let mut colscale = vec![0.0; n + m];
    info.errflag = 0;
    for j in 0..n + m {
        colscale[j] = iterate.scaling_factor(j);
        if lb[j] == ub[j] {
            colscale[j] = 0.0;
        }
    }
    basis.construct_basis_from_weights(&colscale, info);
    if info.errflag != 0 {
        return;
    }
    for (j, colscale_j) in colscale[..n + m].iter().enumerate() {
        if *colscale_j == 0.0 || colscale_j.is_infinite() {
            if basis.is_basic(j as Int) {
                basis.free_basic_variable(j as Int);
            } else {
                basis.fix_nonbasic_variable(j as Int);
            }
        }
    }
    for j in 0..n + m {
        if lb[j] == ub[j] && basis.status_of(j as Int) == BasicStatus::NonbasicFixed {
            iterate.make_fixed_at(j, lb[j]);
        }
    }
    postprocess_dependencies(iterate, basis, info);
}

fn postprocess_dependencies(iterate: &mut Iterate<'_>, basis: &mut Basis<'_>, info: &mut Info) {
    let model = iterate.model();
    let m = u(model.rows());
    let n = u(model.cols());
    let (ai, lb, ub) = (model.ai(), model.lb(), model.ub());
    let mut dependent_rows: Vec<usize> = Vec::new();
    let mut dependent_cols: Vec<usize> = Vec::new();
    let mut dx = vec![0.0; n + m];
    let mut dy = vec![0.0; m];
    if info.dependent_cols > 0 {
        let mut dxbasic = vec![0.0; m];
        for j in 0..n {
            if lb[j].is_infinite() && ub[j].is_infinite() && basis.is_nonbasic(j as Int) {
                debug_assert_eq!(iterate.state_of(j), State::Free);
                dx[j] = -iterate.x[j];
                scatter_column(ai, j as Int, iterate.x[j], &mut dxbasic);
                dependent_cols.push(j);
            }
        }
        basis.solve_dense_vec(&mut dxbasic, 'N');
        for p in 0..m {
            dx[u(basis.at(p))] = dxbasic[p];
        }
    }
    if info.dependent_rows > 0 {
        for (p, dy_p) in dy[..m].iter_mut().enumerate() {
            let j = u(basis.at(p));
            if j >= n && lb[j] == ub[j] {
                *dy_p = -iterate.y[j - n];
                dependent_rows.push(j - n);
            }
        }
        basis.solve_dense_vec(&mut dy, 'T');
        for &i in &dependent_rows {
            dy[i] = -iterate.y[i];
        }
    }
    iterate.update(1.0, Some(&dx), None, None, 1.0, Some(&dy), None, None);
    for &j in &dependent_cols {
        iterate.make_fixed_at(j, 0.0);
    }
    for &i in &dependent_rows {
        iterate.make_implied_eq(n + i);
    }
    let _ = dot;
}
