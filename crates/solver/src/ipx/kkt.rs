//! IPX's KKT solvers: the normal equations `AI*W*AI' y = r` solved by the
//! (preconditioned) conjugate residuals method, with a diagonal
//! preconditioner in the first iterations (`KKTSolverDiag`) and one from a
//! basis afterwards (`KKTSolverBasis`, with its `SplittedNormalMatrix`).

use super::{
    IPX_ERROR_CR_INF_OR_NAN, IPX_ERROR_CR_ITER_LIMIT, IPX_ERROR_CR_MATRIX_NOT_POSDEF, IPX_ERROR_CR_NO_PROGRESS, IPX_ERROR_CR_PRECOND_NOT_POSDEF, Info,
    basis::{BasicStatus, Basis},
    control::Control,
    iterate::Iterate,
    maxvolume::Maxvolume,
    model::Model,
    sparse::{
        IndexedVector, Int, SparseMatrix, add_normal_product, backward_solve, copy_columns, dot, dot_column, forward_solve, infnorm, inverse_perm, permute_rows, scale_column,
        scatter_column, u,
    },
};

const INF: f64 = f64::INFINITY;

/// IPX's `LinearOperator`: `lhs = op(rhs)`, returning `rhs'lhs` when
/// asked for.
pub(crate) trait LinearOperator {
    fn apply(&mut self, rhs: &[f64], lhs: &mut [f64], want_dot: bool) -> f64;
}

/// `AI*W*AI'`, or `AI*AI'` without weights.
pub(crate) struct NormalMatrix<'a> {
    model: &'a Model,
    w: Option<Vec<f64>>,
}

impl<'a> NormalMatrix<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self { model, w: None }
    }

    pub fn prepare(&mut self, w: Option<&[f64]>) {
        self.w = w.map(|w| w.to_vec());
    }
}

impl LinearOperator for NormalMatrix<'_> {
    fn apply(&mut self, rhs: &[f64], lhs: &mut [f64], want_dot: bool) -> f64 {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let ai = self.model.ai();
        let (ap, ai_idx, ax) = (&ai.colptr, &ai.rowidx, &ai.values);
        if let Some(w) = &self.w {
            for i in 0..m {
                lhs[i] = rhs[i] * w[n + i];
            }
            for j in 0..n {
                let (index, value) = (&ai_idx[u(ap[j])..u(ap[j + 1])], &ax[u(ap[j])..u(ap[j + 1])]);
                let mut d = 0.0;
                for (&i, &v) in index.iter().zip(value) {
                    d += rhs[u(i)] * v;
                }
                d *= w[j];
                for (&i, &v) in index.iter().zip(value) {
                    lhs[u(i)] += d * v;
                }
            }
        } else {
            lhs[..m].fill(0.0);
            for j in 0..n {
                let (index, value) = (&ai_idx[u(ap[j])..u(ap[j + 1])], &ax[u(ap[j])..u(ap[j + 1])]);
                let mut d = 0.0;
                for (&i, &v) in index.iter().zip(value) {
                    d += rhs[u(i)] * v;
                }
                for (&i, &v) in index.iter().zip(value) {
                    lhs[u(i)] += d * v;
                }
            }
        }
        if want_dot { dot(rhs, lhs) } else { 0.0 }
    }
}

/// The diagonal of the normal matrix as its preconditioner.
pub(crate) struct DiagonalPrecond<'a> {
    model: &'a Model,
    diagonal: Vec<f64>,
}

impl<'a> DiagonalPrecond<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self {
            model,
            diagonal: vec![0.0; u(model.rows())],
        }
    }

    pub fn factorize(&mut self, w: Option<&[f64]>) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let ai = self.model.ai();
        if let Some(w) = w {
            self.diagonal[..m].copy_from_slice(&w[n..n + m]);
            for j in 0..n {
                let wj = w[j];
                for p in ai.begin(j as Int)..ai.end(j as Int) {
                    self.diagonal[u(ai.index(p))] += ai.value(p) * wj * ai.value(p);
                }
            }
        } else {
            self.diagonal.fill(0.0);
            for j in 0..n {
                for p in ai.begin(j as Int)..ai.end(j as Int) {
                    self.diagonal[u(ai.index(p))] += ai.value(p) * ai.value(p);
                }
            }
        }
    }
}

impl LinearOperator for DiagonalPrecond<'_> {
    fn apply(&mut self, rhs: &[f64], lhs: &mut [f64], _want_dot: bool) -> f64 {
        let mut rldot = 0.0;
        for i in 0..self.diagonal.len() {
            lhs[i] = rhs[i] / self.diagonal[i];
            rldot += lhs[i] * rhs[i];
        }
        rldot
    }
}

/// IPX's `ConjugateResiduals`.
pub(crate) struct ConjugateResiduals<'c> {
    control: &'c Control,
    pub errflag: Int,
    pub iter: Int,
}

impl<'c> ConjugateResiduals<'c> {
    pub fn new(control: &'c Control) -> Self {
        Self { control, errflag: 0, iter: 0 }
    }

    fn resnorm(residual: &[f64], resscale: Option<&[f64]>) -> f64 {
        match resscale {
            Some(resscale) => {
                let mut resnorm: f64 = 0.0;
                for i in 0..residual.len() {
                    let v = (resscale[i] * residual[i]).abs();
                    if resnorm < v {
                        resnorm = v;
                    }
                }
                resnorm
            }
            None => infnorm(residual),
        }
    }

    /// Solve `C lhs = rhs` from `lhs` as given.
    pub fn solve(&mut self, c: &mut dyn LinearOperator, rhs: &[f64], tol: f64, resscale: Option<&[f64]>, mut maxiter: Int, lhs: &mut [f64]) {
        let m = rhs.len();
        let mut residual = vec![0.0; m];
        let mut step = vec![0.0; m];
        let mut cresidual = vec![0.0; m];
        let mut cstep = vec![0.0; m];
        self.errflag = 0;
        self.iter = 0;
        if maxiter < 0 {
            maxiter = m as Int + 100;
        }
        if infnorm(lhs) == 0.0 {
            residual.copy_from_slice(rhs);
        } else {
            c.apply(lhs, &mut residual, false);
            for i in 0..m {
                residual[i] = rhs[i] - residual[i];
            }
        }
        let mut cdot = c.apply(&residual, &mut cresidual, true);
        step.copy_from_slice(&residual);
        cstep.copy_from_slice(&cresidual);
        loop {
            let resnorm = Self::resnorm(&residual, resscale);
            if resnorm <= tol {
                break;
            }
            if self.iter == maxiter {
                self.errflag = IPX_ERROR_CR_ITER_LIMIT;
                break;
            }
            if cdot <= 0.0 {
                self.errflag = IPX_ERROR_CR_MATRIX_NOT_POSDEF;
                break;
            }
            let denom = dot(&cstep, &cstep);
            let alpha = cdot / denom;
            if !alpha.is_finite() {
                self.errflag = IPX_ERROR_CR_INF_OR_NAN;
                break;
            }
            for i in 0..m {
                lhs[i] += alpha * step[i];
                residual[i] -= alpha * cstep[i];
            }
            let cdotnew = c.apply(&residual, &mut cresidual, true);
            let beta = cdotnew / cdot;
            for i in 0..m {
                step[i] = residual[i] + beta * step[i];
                cstep[i] = cresidual[i] + beta * cstep[i];
            }
            cdot = cdotnew;
            self.iter += 1;
            self.errflag = self.control.interrupt_check();
            if self.errflag != 0 {
                break;
            }
        }
    }

    /// Solve `C lhs = rhs` with the preconditioner `P`.
    #[allow(clippy::too_many_arguments)]
    pub fn solve_preconditioned(
        &mut self,
        c: &mut dyn LinearOperator,
        p: &mut dyn LinearOperator,
        rhs: &[f64],
        tol: f64,
        resscale: Option<&[f64]>,
        mut maxiter: Int,
        lhs: &mut [f64],
    ) {
        let m = rhs.len();
        let mut residual = vec![0.0; m];
        let mut sresidual = vec![0.0; m];
        let mut step = vec![0.0; m];
        let mut csresidual = vec![0.0; m];
        let mut cstep = vec![0.0; m];
        self.errflag = 0;
        self.iter = 0;
        if maxiter < 0 {
            maxiter = m as Int + 100;
        }
        if infnorm(lhs) == 0.0 {
            residual.copy_from_slice(rhs);
        } else {
            c.apply(lhs, &mut residual, false);
            for i in 0..m {
                residual[i] = rhs[i] - residual[i];
            }
        }
        let mut resnorm_precond_system = p.apply(&residual, &mut sresidual, true);
        let mut cdot = c.apply(&sresidual, &mut csresidual, true);
        step.copy_from_slice(&sresidual);
        cstep.copy_from_slice(&csresidual);
        loop {
            let resnorm = Self::resnorm(&residual, resscale);
            if resnorm <= tol {
                break;
            }
            if self.iter == maxiter {
                self.errflag = IPX_ERROR_CR_ITER_LIMIT;
                break;
            }
            if cdot <= 0.0 {
                self.errflag = IPX_ERROR_CR_MATRIX_NOT_POSDEF;
                break;
            }
            // `precond_cstep` lives in `csresidual`, as in IPX.
            let pdot = p.apply(&cstep, &mut csresidual, true);
            if pdot <= 0.0 {
                self.errflag = IPX_ERROR_CR_PRECOND_NOT_POSDEF;
                break;
            }
            let alpha = cdot / pdot;
            if !alpha.is_finite() {
                self.errflag = IPX_ERROR_CR_INF_OR_NAN;
                break;
            }
            for i in 0..m {
                lhs[i] += alpha * step[i];
                residual[i] -= alpha * cstep[i];
                sresidual[i] -= alpha * csresidual[i];
            }
            let cdotnew = c.apply(&sresidual, &mut csresidual, true);
            let beta = cdotnew / cdot;
            for i in 0..m {
                step[i] = sresidual[i] + beta * step[i];
                cstep[i] = csresidual[i] + beta * cstep[i];
            }
            cdot = cdotnew;
            self.iter += 1;
            if self.iter % 5 == 0 {
                let rsdot = p.apply(&residual, &mut sresidual, true);
                if rsdot >= resnorm_precond_system {
                    self.errflag = IPX_ERROR_CR_NO_PROGRESS;
                    break;
                }
                resnorm_precond_system = rsdot;
            }
            self.errflag = self.control.interrupt_check();
            if self.errflag != 0 {
                break;
            }
        }
    }
}

/// IPX's `KKTSolver` interface.
pub(crate) trait KktSolver {
    /// Prepare for solves at `iterate` (or the identity weights).
    fn factorize(&mut self, iterate: Option<&mut Iterate<'_>>, info: &mut Info);
    /// Solve the KKT system `[W^-1 AI'; AI 0] [x; y] = [a; b]` to `tol`.
    fn solve(&mut self, a: &[f64], b: &[f64], tol: f64, x: &mut [f64], y: &mut [f64], info: &mut Info);
    fn iter(&self) -> Int;
    fn basis_changes(&self) -> Int {
        0
    }
}

pub(crate) struct KktSolverDiag<'a, 'c> {
    control: &'c Control,
    model: &'a Model,
    normal_matrix: NormalMatrix<'a>,
    precond: DiagonalPrecond<'a>,
    w: Vec<f64>,
    resscale: Vec<f64>,
    factorized: bool,
    pub maxiter: Int,
    iter: Int,
}

impl<'a, 'c> KktSolverDiag<'a, 'c> {
    pub fn new(control: &'c Control, model: &'a Model) -> Self {
        let m = u(model.rows());
        let n = u(model.cols());
        Self {
            control,
            model,
            normal_matrix: NormalMatrix::new(model),
            precond: DiagonalPrecond::new(model),
            w: vec![0.0; n + m],
            resscale: vec![0.0; m],
            factorized: false,
            maxiter: -1,
            iter: 0,
        }
    }
}

impl KktSolver for KktSolverDiag<'_, '_> {
    fn factorize(&mut self, iterate: Option<&mut Iterate<'_>>, info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        self.iter = 0;
        self.factorized = false;
        if let Some(pt) = iterate {
            let mut regval = pt.mu();
            for j in 0..n + m {
                let g = pt.zl[j] / pt.xl[j] + pt.zu[j] / pt.xu[j];
                if g != 0.0 && g < regval {
                    regval = g;
                }
                self.w[j] = 1.0 / g;
            }
            for j in 0..n + m {
                if self.w[j].is_infinite() {
                    self.w[j] = 1.0 / regval;
                }
            }
        } else {
            self.w.fill(1.0);
        }
        for i in 0..m {
            self.resscale[i] = 1.0 / self.w[n + i].sqrt();
        }
        self.normal_matrix.prepare(Some(&self.w));
        self.precond.factorize(Some(&self.w));
        if info.errflag != 0 {
            return;
        }
        self.factorized = true;
    }

    fn solve(&mut self, a: &[f64], b: &[f64], tol: f64, x: &mut [f64], y: &mut [f64], info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let ai = self.model.ai();
        let mut rhs: Vec<f64> = b.iter().map(|&bi| -bi).collect();
        for j in 0..n + m {
            scatter_column(ai, j as Int, self.w[j] * a[j], &mut rhs);
        }
        y.fill(0.0);
        let mut cr = ConjugateResiduals::new(self.control);
        cr.solve_preconditioned(&mut self.normal_matrix, &mut self.precond, &rhs, tol, Some(&self.resscale), self.maxiter, y);
        info.errflag = cr.errflag;
        info.kktiter1 += cr.iter;
        self.iter += cr.iter;
        x[n..n + m].copy_from_slice(&b[..m]);
        for j in 0..n {
            let aty = dot_column(ai, j as Int, y);
            x[j] = self.w[j] * (a[j] - aty);
            for p in ai.begin(j as Int)..ai.end(j as Int) {
                let i = u(ai.index(p));
                x[n + i] -= x[j] * ai.value(p);
            }
        }
    }

    fn iter(&self) -> Int {
        self.iter
    }
}

/// `N*N'` preconditioned by the basis: `I + B^-1 N N' B^-T` in IPX's
/// row and column permuted form.
pub(crate) struct SplittedNormalMatrix<'a> {
    model: &'a Model,
    l: SparseMatrix,
    uf: SparseMatrix,
    n: SparseMatrix,
    free_positions: Vec<Int>,
    pub colperm: Vec<Int>,
    rowperm_inv: Vec<Int>,
    work: Vec<f64>,
}

impl<'a> SplittedNormalMatrix<'a> {
    pub fn new(model: &'a Model) -> Self {
        let m = u(model.rows());
        Self {
            model,
            l: SparseMatrix::default(),
            uf: SparseMatrix::default(),
            n: SparseMatrix::default(),
            free_positions: Vec::new(),
            colperm: vec![0; m],
            rowperm_inv: vec![0; m],
            work: vec![0.0; m],
        }
    }

    pub fn prepare(&mut self, basis: &mut Basis<'_>, colscale: &[f64]) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let ai = self.model.ai();
        self.n.clear();
        let (l, uf, rowperm, colperm) = basis.get_lu_factors();
        self.l = l;
        self.uf = uf;
        self.rowperm_inv = inverse_perm(&rowperm);
        self.colperm = colperm;
        for k in 0..m {
            let p = u(self.colperm[k]);
            let j = basis.at(p);
            if basis.status_of(j) == BasicStatus::Basic {
                scale_column(&mut self.uf, k as Int, colscale[u(j)]);
            }
        }
        let nonbasic_vars: Vec<Int> = (0..(n + m) as Int).filter(|&j| basis.status_of(j) == BasicStatus::Nonbasic).collect();
        self.n = copy_columns(ai, &nonbasic_vars);
        permute_rows(&mut self.n, &self.rowperm_inv);
        for (k, &j) in nonbasic_vars.iter().enumerate() {
            scale_column(&mut self.n, k as Int, colscale[u(j)]);
        }
        self.free_positions.clear();
        for k in 0..m {
            let p = u(self.colperm[k]);
            let j = basis.at(p);
            if basis.status_of(j) == BasicStatus::BasicFree {
                self.free_positions.push(k as Int);
            }
        }
    }
}

impl LinearOperator for SplittedNormalMatrix<'_> {
    fn apply(&mut self, rhs: &[f64], lhs: &mut [f64], want_dot: bool) -> f64 {
        self.work.copy_from_slice(rhs);
        backward_solve(&self.l, &self.uf, &mut self.work);
        lhs.fill(0.0);
        add_normal_product(&self.n, None, &self.work, lhs);
        forward_solve(&self.l, &self.uf, lhs);
        for i in 0..lhs.len() {
            lhs[i] += rhs[i];
        }
        for &i in &self.free_positions {
            lhs[u(i)] = 0.0;
        }
        if want_dot { dot(rhs, lhs) } else { 0.0 }
    }
}

const PIVOT_ZERO_TOL: f64 = 1e-7;

pub(crate) struct KktSolverBasis<'b, 'a, 'c> {
    control: &'c Control,
    model: &'a Model,
    basis: &'b mut Basis<'a>,
    splitted_normal_matrix: SplittedNormalMatrix<'a>,
    colscale: Vec<f64>,
    factorized: bool,
    pub maxiter: Int,
    iter: Int,
    basis_changes: Int,
}

impl<'b, 'a, 'c> KktSolverBasis<'b, 'a, 'c> {
    pub fn new(control: &'c Control, basis: &'b mut Basis<'a>) -> Self {
        let model = basis.model();
        let m = u(model.rows());
        let n = u(model.cols());
        Self {
            control,
            model,
            basis,
            splitted_normal_matrix: SplittedNormalMatrix::new(model),
            colscale: vec![0.0; n + m],
            factorized: false,
            maxiter: -1,
            iter: 0,
            basis_changes: 0,
        }
    }

    fn drop_primal(&mut self, iterate: &mut Iterate<'_>, info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let mut btran = IndexedVector::new(m);
        let mut row = IndexedVector::new(n + m);
        let drop_primal = self.control.params.ipm_drop_primal;
        let volume_tol = 2.0;
        info.errflag = 0;
        let mut candidates: Vec<Int> = Vec::new();
        for p in 0..m {
            let jb = self.basis.at(p);
            if self.basis.status_of(jb) != BasicStatus::Basic {
                continue;
            }
            let jbu = u(jb);
            let (xj, zj) = if iterate.xl[jbu] <= iterate.xu[jbu] {
                (iterate.xl[jbu], iterate.zl[jbu])
            } else {
                (iterate.xu[jbu], iterate.zu[jbu])
            };
            if xj < 0.01 * zj && xj <= drop_primal {
                candidates.push(jb);
            }
        }
        if candidates.is_empty() {
            return;
        }
        let mut invscale_basic = vec![0.0; m];
        for p in 0..m {
            invscale_basic[p] = 1.0 / self.colscale[u(self.basis.at(p))];
        }
        while let Some(&jb) = candidates.last() {
            let p = u(self.basis.position_of(jb));
            let s = invscale_basic[p];
            self.basis.tableau_row(jb, &mut btran, &mut row, true);
            let mut jmax: Int = -1;
            let mut vmax = volume_tol;
            row.for_each_nonzero(|j, pivot| {
                let pivot = pivot.abs();
                if pivot > PIVOT_ZERO_TOL {
                    let v = pivot * self.colscale[j] * s;
                    if v > vmax {
                        vmax = v;
                        jmax = j as Int;
                    }
                }
            });
            if jmax >= 0 {
                let pivot = row.elements[u(jmax)];
                let mut exchanged = false;
                info.errflag = self.basis.exchange_if_stable(jb, jmax, pivot, 1, &mut exchanged);
                if info.errflag != 0 {
                    return;
                }
                if !exchanged {
                    continue;
                }
                invscale_basic[p] = 1.0 / self.colscale[u(jmax)];
                info.updates_ipm += 1;
                self.basis_changes += 1;
            } else {
                let jbu = u(jb);
                if iterate.zl[jbu] / iterate.xl[jbu] > iterate.zu[jbu] / iterate.xu[jbu] {
                    iterate.make_implied_lb(jbu);
                } else {
                    iterate.make_implied_ub(jbu);
                }
                self.basis.free_basic_variable(jb);
                invscale_basic[p] = 0.0;
                self.colscale[jbu] = INF;
                info.primal_dropped += 1;
            }
            candidates.pop();
        }
    }

    fn drop_dual(&mut self, iterate: &mut Iterate<'_>, info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let mut ftran = IndexedVector::new(m);
        let drop_dual = self.control.params.ipm_drop_dual;
        let volume_tol = 2.0;
        info.errflag = 0;
        let mut candidates: Vec<Int> = Vec::new();
        for jn in 0..n + m {
            if self.basis.status_of(jn as Int) != BasicStatus::Nonbasic {
                continue;
            }
            let (xj, zj) = if iterate.zl[jn] >= iterate.zu[jn] {
                (iterate.xl[jn], iterate.zl[jn])
            } else {
                (iterate.xu[jn], iterate.zu[jn])
            };
            if zj < 0.01 * xj && zj <= drop_dual {
                candidates.push(jn as Int);
            }
        }
        if candidates.is_empty() {
            return;
        }
        let mut invscale_basic = vec![0.0; m];
        for p in 0..m {
            invscale_basic[p] = 1.0 / self.colscale[u(self.basis.at(p))];
        }
        while let Some(&jn) = candidates.last() {
            let s = self.colscale[u(jn)];
            self.basis.solve_for_update_into(jn, &mut ftran);
            let mut pmax: Int = -1;
            let mut vmax = volume_tol;
            ftran.for_each_nonzero(|p, pivot| {
                let pivot = pivot.abs();
                if pivot > PIVOT_ZERO_TOL {
                    let v = pivot * invscale_basic[p] * s;
                    if v > vmax {
                        vmax = v;
                        pmax = p as Int;
                    }
                }
            });
            if pmax >= 0 {
                let pivot = ftran.elements[u(pmax)];
                let jb = self.basis.at(u(pmax));
                let mut exchanged = false;
                info.errflag = self.basis.exchange_if_stable(jb, jn, pivot, -1, &mut exchanged);
                if info.errflag != 0 {
                    return;
                }
                if !exchanged {
                    continue;
                }
                invscale_basic[u(pmax)] = 1.0 / self.colscale[u(jn)];
                info.updates_ipm += 1;
                self.basis_changes += 1;
            } else {
                iterate.make_fixed(u(jn));
                self.basis.fix_nonbasic_variable(jn);
                self.colscale[u(jn)] = 0.0;
                info.dual_dropped += 1;
            }
            candidates.pop();
        }
    }
}

impl KktSolver for KktSolverBasis<'_, '_, '_> {
    fn factorize(&mut self, iterate: Option<&mut Iterate<'_>>, info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let iterate = iterate.expect("the basis KKT solver factors at an iterate");
        info.errflag = 0;
        self.factorized = false;
        self.iter = 0;
        self.basis_changes = 0;
        for j in 0..n + m {
            self.colscale[j] = iterate.scaling_factor(j);
        }
        if iterate.pobjective() >= iterate.dobjective() {
            self.drop_primal(iterate, info);
            if info.errflag != 0 {
                return;
            }
            self.drop_dual(iterate, info);
            if info.errflag != 0 {
                return;
            }
        }
        let mut maxvol = Maxvolume::new(self.control);
        info.errflag = if self.control.params.update_heuristic == 0 {
            maxvol.run_sequential(Some(&self.colscale), self.basis)
        } else {
            maxvol.run_heuristic(Some(&self.colscale), self.basis)
        };
        info.updates_ipm += maxvol.updates;
        self.basis_changes += maxvol.updates;
        if info.errflag != 0 {
            return;
        }
        if !self.basis.factorization_is_fresh() {
            info.errflag = self.basis.factorize();
            if info.errflag != 0 {
                return;
            }
        }
        self.splitted_normal_matrix.prepare(self.basis, &self.colscale);
        self.factorized = true;
    }

    fn solve(&mut self, a: &[f64], b: &[f64], tol: f64, x: &mut [f64], y: &mut [f64], info: &mut Info) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let ai = self.model.ai();
        let mut rhs = vec![0.0; m];
        let mut work = vec![0.0; m];
        info.errflag = 0;
        let mut num_free = 0;
        for p in 0..m {
            let j = self.basis.at(p);
            if self.basis.status_of(j) == BasicStatus::BasicFree {
                work[p] = a[u(j)];
                num_free += 1;
            }
        }
        if num_free > 0 {
            let input = work.clone();
            self.basis.solve_dense(&input, &mut work, 'T');
        }
        if num_free > 0 {
            for j in 0..n + m {
                if self.basis.status_of(j as Int) == BasicStatus::Nonbasic {
                    let d2 = self.colscale[j] * self.colscale[j];
                    let mut alpha = a[j] - dot_column(ai, j as Int, &work);
                    alpha *= d2;
                    scatter_column(ai, j as Int, alpha, &mut rhs);
                }
            }
        } else {
            for j in 0..n + m {
                if self.basis.status_of(j as Int) == BasicStatus::Nonbasic {
                    let d2 = self.colscale[j] * self.colscale[j];
                    let alpha = d2 * a[j];
                    scatter_column(ai, j as Int, alpha, &mut rhs);
                }
            }
        }
        let input = rhs.clone();
        self.basis.solve_dense(&input, &mut rhs, 'N');
        self.basis.solve_dense(b, &mut work, 'N');
        for p in 0..m {
            let j = self.basis.at(p);
            if self.basis.status_of(j) == BasicStatus::Basic {
                let d = self.colscale[u(j)];
                rhs[p] = (rhs[p] - work[p]) / d + a[u(j)] * d;
            } else {
                rhs[p] = 0.0;
            }
        }
        let colperm = &self.splitted_normal_matrix.colperm;
        for k in 0..m {
            work[k] = rhs[u(colperm[k])];
        }
        let mut lhs = rhs;
        lhs.fill(0.0);
        let mut cr = ConjugateResiduals::new(self.control);
        cr.solve(&mut self.splitted_normal_matrix, &work, tol, None, self.maxiter, &mut lhs);
        info.errflag = cr.errflag;
        info.kktiter2 += cr.iter;
        self.iter += cr.iter;
        let colperm = &self.splitted_normal_matrix.colperm;
        for k in 0..m {
            y[u(colperm[k])] = lhs[k];
        }
        for p in 0..m {
            let j = self.basis.at(p);
            if self.basis.status_of(j) == BasicStatus::Basic {
                y[p] /= self.colscale[u(j)];
            } else {
                y[p] = a[u(j)];
            }
        }
        let input = y.to_vec();
        self.basis.solve_dense(&input, y, 'T');
        work.copy_from_slice(&b[..m]);
        for j in 0..n + m {
            let mut xj = 0.0;
            if self.basis.status_of(j as Int) == BasicStatus::Nonbasic {
                xj = a[j] - dot_column(ai, j as Int, y);
                xj *= self.colscale[j] * self.colscale[j];
                scatter_column(ai, j as Int, -xj, &mut work);
            }
            x[j] = xj;
        }
        let input = work.clone();
        self.basis.solve_dense(&input, &mut work, 'N');
        for p in 0..m {
            x[u(self.basis.at(p))] = work[p];
        }
    }

    fn iter(&self) -> Int {
        self.iter
    }

    fn basis_changes(&self) -> Int {
        self.basis_changes
    }
}
