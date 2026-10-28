//! IPX's `Maxvolume` (`maxvolume.cc`): basis updates that increase the
//! volume of the scaled basis matrix, which keep the basis
//! preconditioner good as the interior point iterates move.

use super::{
    basis::{BasicStatus, Basis},
    control::Control,
    sparse::{IndexedVector, Int, dot_column, sortperm, u},
};

const PIVOT_ZERO_TOL: f64 = 1e-7;

pub(crate) struct Maxvolume<'c> {
    control: &'c Control,
    pub updates: Int,
    pub skipped: Int,
    pub passes: Int,
    pub slices: Int,
    pub volinc: f64,
    pub tblnnz: Int,
    pub tblmax: f64,
    pub frobnorm_squared: f64,
}

struct Slice {
    colscale: Vec<f64>,
    invscale_basic: Vec<f64>,
    tblrow_used: Vec<bool>,
    colweights: Vec<f64>,
    lhs: IndexedVector,
    row: IndexedVector,
    work: Vec<f64>,
}

impl<'c> Maxvolume<'c> {
    pub fn new(control: &'c Control) -> Self {
        Self {
            control,
            updates: 0,
            skipped: 0,
            passes: 0,
            slices: 0,
            volinc: 0.0,
            tblnnz: 0,
            tblmax: 0.0,
            frobnorm_squared: 0.0,
        }
    }

    fn reset(&mut self) {
        self.updates = 0;
        self.skipped = 0;
        self.passes = 0;
        self.slices = 0;
        self.volinc = 0.0;
        self.tblnnz = 0;
        self.tblmax = 0.0;
        self.frobnorm_squared = 0.0;
    }

    pub fn run_sequential(&mut self, colscale: Option<&[f64]>, basis: &mut Basis<'_>) -> Int {
        let model = basis.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let mut ftran = IndexedVector::new(m);
        let mut errflag = 0;
        let maxpasses = self.control.params.maxpasses;
        let volumetol = self.control.params.volume_tol.max(1.0);
        let mut invscale_basic = vec![0.0; m];
        for p in 0..m {
            let j = basis.at(p);
            if basis.status_of(j) == BasicStatus::Basic {
                invscale_basic[p] = colscale.map_or(1.0, |colscale| 1.0 / colscale[u(j)]);
            }
        }
        self.reset();
        while self.passes < maxpasses || maxpasses < 0 {
            self.tblnnz = 0;
            self.tblmax = 0.0;
            self.frobnorm_squared = 0.0;
            let mut updates_last = 0;
            let mut candidates = sortperm((n + m) as Int, colscale, false);
            while let Some(&j) = candidates.last() {
                let dj = colscale.map_or(1.0, |colscale| colscale[u(j)]);
                if dj == 0.0 {
                    break;
                }
                if basis.status_of(j) != BasicStatus::Nonbasic {
                    candidates.pop();
                    continue;
                }
                errflag = self.control.interrupt_check();
                if errflag != 0 {
                    break;
                }
                basis.solve_for_update_into(j, &mut ftran);
                let mut pmax: Int = -1;
                let mut vmax = 0.0;
                let (tblnnz, frobnorm_squared) = (&mut self.tblnnz, &mut self.frobnorm_squared);
                ftran.for_each_nonzero(|p, x| {
                    let v = x.abs() * invscale_basic[p] * dj;
                    if v > vmax {
                        vmax = v;
                        pmax = p as Int;
                    }
                    *tblnnz += Int::from(v != 0.0);
                    *frobnorm_squared += v * v;
                });
                if self.tblmax < vmax {
                    self.tblmax = vmax;
                }
                if vmax <= volumetol {
                    self.skipped += 1;
                    candidates.pop();
                    continue;
                }
                let jb = basis.at(u(pmax));
                let mut exchanged = false;
                errflag = basis.exchange_if_stable(jb, j, ftran.elements[u(pmax)], -1, &mut exchanged);
                if errflag != 0 {
                    break;
                }
                if !exchanged {
                    continue;
                }
                invscale_basic[u(pmax)] = 1.0 / dj;
                updates_last += 1;
                self.volinc += vmax.log2();
                candidates.pop();
            }
            self.updates += updates_last;
            self.passes += 1;
            if updates_last == 0 || errflag != 0 {
                break;
            }
        }
        errflag
    }

    pub fn run_heuristic(&mut self, colscale: Option<&[f64]>, basis: &mut Basis<'_>) -> Int {
        let model = basis.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let mut slice = Slice {
            colscale: vec![0.0; n + m],
            invscale_basic: vec![0.0; m],
            tblrow_used: vec![false; m],
            colweights: vec![0.0; n + m],
            lhs: IndexedVector::new(m),
            row: IndexedVector::new(n + m),
            work: vec![0.0; m],
        };
        let mut errflag = 0;
        self.reset();
        let num_slices = (5 + (m as Int / self.control.params.rows_per_slice).max(0)).min(m as Int);
        for p in 0..m {
            let j = basis.at(p);
            if basis.status_of(j) == BasicStatus::Basic {
                slice.invscale_basic[p] = colscale.map_or(1.0, |colscale| 1.0 / colscale[u(j)]);
            }
        }
        for j in 0..n + m {
            if basis.status_of(j as Int) == BasicStatus::Nonbasic {
                slice.colscale[j] = colscale.map_or(1.0, |colscale| colscale[j]);
            }
        }
        let perm = sortperm(m as Int, Some(&slice.invscale_basic), false);
        for s in 0..num_slices {
            for i in 0..m {
                slice.tblrow_used[u(perm[i])] = i as Int % num_slices == s;
            }
            errflag = self.driver(basis, &mut slice);
            if errflag != 0 {
                break;
            }
        }
        self.passes = -1;
        self.slices = num_slices;
        errflag
    }

    fn driver(&mut self, basis: &mut Basis<'_>, slice: &mut Slice) -> Int {
        let model = basis.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let ai = model.ai();
        let mut errflag = 0;
        let volumetol = self.control.params.volume_tol.max(1.0);
        let maxskip = self.control.params.maxskip_updates;
        for p in 0..m {
            slice.work[p] = if slice.tblrow_used[p] { slice.invscale_basic[p] } else { 0.0 };
        }
        let input = slice.work.clone();
        basis.solve_dense(&input, &mut slice.work, 'T');
        for j in 0..n + m {
            if slice.colscale[j] != 0.0 {
                let sum = dot_column(ai, j as Int, &slice.work);
                slice.colweights[j] = sum * slice.colscale[j];
            } else {
                slice.colweights[j] = 0.0;
            }
        }
        let mut candidates: Vec<Int> = Vec::new();
        let mut skipped = 0;
        loop {
            if candidates.is_empty() {
                candidates = find_largest(&slice.colweights);
            }
            let jn = *candidates.last().unwrap();
            let weight = slice.colweights[u(jn)];
            if weight == 0.0 {
                break;
            }
            errflag = self.control.interrupt_check();
            if errflag != 0 {
                break;
            }
            basis.solve_for_update_into(jn, &mut slice.lhs);
            let pmax = scale_ftran(slice.colscale[u(jn)], &slice.invscale_basic, &mut slice.lhs);
            let scaled_pivot = slice.lhs.elements[u(pmax)];
            let vmax = scaled_pivot.abs();
            if vmax <= volumetol {
                slice.colweights[u(jn)] = 0.0;
                slice.colscale[u(jn)] = 0.0;
                candidates.pop();
                skipped += 1;
                if skipped > maxskip && maxskip >= 0 {
                    break;
                }
                continue;
            }
            let mut weight_recomp = 0.0;
            let tblrow_used = &slice.tblrow_used;
            slice.lhs.for_each_nonzero(|p, x| {
                if tblrow_used[p] {
                    weight_recomp += x;
                }
            });
            let jb = basis.at(u(pmax));
            basis.tableau_row(jb, &mut slice.lhs, &mut slice.row, true);
            let pivot = slice.row.elements[u(jn)];
            let mut exchanged = false;
            errflag = basis.exchange_if_stable(jb, jn, pivot, 0, &mut exchanged);
            if errflag != 0 {
                break;
            }
            if !exchanged {
                continue;
            }
            self.updates += 1;
            self.volinc += vmax.log2();
            let dn = slice.colscale[u(jn)];
            let dbinv = slice.invscale_basic[u(pmax)];
            slice.colscale[u(jb)] = 1.0 / slice.invscale_basic[u(pmax)];
            slice.invscale_basic[u(pmax)] = 1.0 / slice.colscale[u(jn)];
            slice.colscale[u(jn)] = 0.0;
            let used = if slice.tblrow_used[u(pmax)] { 1.0 } else { 0.0 };
            let alpha = (used - weight_recomp) / (dn * pivot);
            let (colweights, colscale) = (&mut slice.colweights, &slice.colscale);
            slice.row.for_each_nonzero(|j, x| {
                colweights[j] += alpha * x * colscale[j];
            });
            slice.colweights[u(jb)] = used + alpha / dbinv;
            slice.colweights[u(jn)] = 0.0;
            candidates.clear();
        }
        self.skipped += skipped;
        errflag
    }
}

/// The positions of the second largest and the largest |weight|.
fn find_largest(weights: &[f64]) -> Vec<Int> {
    let mut jmax = 0;
    let mut jmax2 = 0;
    let mut wmax = 0.0;
    let mut wmax2 = 0.0;
    // Only a weight above `wmax2` changes anything (`wmax2 <= wmax`), so a
    // chunk with none is skipped whole; the result is IPX's.
    const CHUNK: usize = 16;
    for (c, chunk) in weights.chunks(CHUNK).enumerate() {
        let largest = chunk.iter().fold(0.0f64, |largest, w| largest.max(w.abs()));
        if largest <= wmax2 {
            continue;
        }
        for (k, w) in chunk.iter().enumerate() {
            let (j, w) = (c * CHUNK + k, w.abs());
            if w > wmax {
                wmax2 = wmax;
                wmax = w;
                jmax2 = jmax;
                jmax = j;
            } else if w > wmax2 {
                wmax2 = w;
                jmax2 = j;
            }
        }
    }
    vec![jmax2 as Int, jmax as Int]
}

/// Scale the FTRAN result to the scaled tableau column; returns the
/// position of its largest entry that is not a tiny pivot.
fn scale_ftran(colscale_jn: f64, invscale_basic: &[f64], ftran: &mut IndexedVector) -> Int {
    let mut vmax = 0.0;
    let mut pmax = 0;
    ftran.for_each_nonzero_mut(|p, pivot| {
        let scaled_pivot = *pivot * colscale_jn * invscale_basic[p];
        let v = scaled_pivot.abs();
        if v > vmax && pivot.abs() > PIVOT_ZERO_TOL {
            vmax = v;
            pmax = p as Int;
        }
        *pivot = scaled_pivot;
    });
    pmax
}
