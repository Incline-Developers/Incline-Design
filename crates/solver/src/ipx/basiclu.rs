//! BASICLU (Lukas Schork's sparse LU factorization with Forrest-Tomlin
//! updates, `ipm/basiclu` in HiGHS), which IPX factors its bases with.
//!
//! Ported routine for routine. As in C, the integer workspace is one
//! array (`istore`) partitioned into regions, several of which the solve
//! and update routines reuse under other names once a factorization is
//! built; the offsets below are BASICLU's. Integers are [`Int`], BASICLU's
//! `lu_int` (32 bits, as HiGHS builds it), signed since the arrays hold -1
//! markers alongside indices.

use super::sparse::Int;

const OK: Int = 0;
const REALLOCATE: Int = 1;
const WARNING_SINGULAR_MATRIX: Int = 2;
const ERROR_INVALID_CALL: Int = -2;
const ERROR_INVALID_ARGUMENT: Int = -4;
const ERROR_MAXIMUM_UPDATES: Int = -5;
pub(crate) const ERROR_SINGULAR_UPDATE: Int = -6;

const NO_TASK: Int = 0;
const SINGLETONS: Int = 1;
const SETUP_BUMP: Int = 2;
const FACTORIZE_BUMP: Int = 3;
const BUILD_FACTORS: Int = 4;

const MAXROW_SMALL: Int = 64;
const GAP: Int = -1;

#[inline(always)]
fn u(x: Int) -> usize {
    x as usize
}

/// C's conversion of a double to an integer, toward zero.
#[inline(always)]
fn trunc(x: f64) -> Int {
    x as Int
}

#[inline(always)]
fn flip(i: Int) -> Int {
    -i - 1
}

/// Where a triangular solve writes the pattern of its result.
enum PatternOut<'a> {
    Istore(usize),
    Slice(&'a mut [Int]),
}

#[derive(Clone, Debug)]
pub(crate) struct Lu {
    // Parameters.
    pub droptol: f64,
    pub abstol: f64,
    pub reltol: f64,
    nzbias: Int,
    maxsearch: Int,
    pad: Int,
    stretch: f64,
    compress_thres: f64,
    sparse_thres: f64,
    search_rows: bool,

    // Readable.
    pub m: Int,
    pub addmem_l: Int,
    pub addmem_u: Int,
    pub addmem_w: Int,
    pub nupdate: Int,
    pub nforrest: Int,
    pub nfactorize: Int,
    nupdate_total: Int,
    nforrest_total: Int,
    nsymperm_total: Int,
    pub lnz: Int,
    pub unz: Int,
    rnz: Int,
    min_pivot: f64,
    max_pivot: f64,
    pub max_eta: f64,
    update_cost_numer: f64,
    update_cost_denom: f64,
    lflops: Int,
    uflops: Int,
    rflops: Int,
    condest_l: f64,
    condest_u: f64,
    norm_l: f64,
    norm_u: f64,
    pub normest_linv: f64,
    pub normest_uinv: f64,
    onenorm: f64,
    infnorm: f64,
    pub residual_test: f64,
    pub matrix_nz: Int,
    pub rank: Int,
    bump_size: Int,
    bump_nz: Int,
    nsearch_pivot: Int,
    nexpand: Int,
    ngarbage: Int,
    factor_flops: Int,
    pub pivot_error: f64,

    // Private.
    task: Int,
    pivot_row: Int,
    pivot_col: Int,
    ftran_for_update: Int,
    btran_for_update: Int,
    marker: Int,
    pivotlen: Int,
    rankdef: Int,
    min_colnz: Int,
    min_rownz: Int,

    // The user arrays, whose lengths are BASICLU's Lmem, Umem and Wmem.
    pub li: Vec<Int>,
    pub lx: Vec<f64>,
    pub ui: Vec<Int>,
    pub ux: Vec<f64>,
    pub wi: Vec<Int>,
    pub wx: Vec<f64>,

    is: Vec<Int>,
    colcount_flink: usize,
    colcount_blink: usize,
    rowcount_flink: usize,
    rowcount_blink: usize,
    wbegin: usize,
    wend: usize,
    wflink: usize,
    wblink: usize,
    pinv: usize,
    qinv: usize,
    lbegin_p: usize,
    ubegin: usize,
    iwork0: usize,

    work0: Vec<f64>,
    work1: Vec<f64>,
    col_pivot: Vec<f64>,
    row_pivot: Vec<f64>,
    /// BASICLU keeps the depth-first search stack in `work1` and the
    /// small pivot's cancellation masks in `row_pivot`; neither is read
    /// across those uses, so each has its own array here.
    pstack: Vec<Int>,
    /// BASICLU's `int64_t` bit masks, one bit per row of a small pivot.
    cancelled: Vec<i64>,
}

impl Lu {
    // Aliases for solves and updates.
    fn pivotcol(&self) -> usize {
        self.colcount_flink
    }
    fn pivotrow(&self) -> usize {
        self.colcount_blink
    }
    fn rbegin(&self) -> usize {
        self.rowcount_flink
    }
    fn eta_row(&self) -> usize {
        self.rowcount_flink + u(self.m) + 1
    }
    fn iwork1(&self) -> usize {
        self.rowcount_blink
    }
    fn lbegin(&self) -> usize {
        self.wbegin + u(self.m) + 1
    }
    fn ltbegin(&self) -> usize {
        self.wend + u(self.m) + 1
    }
    fn ltbegin_p(&self) -> usize {
        self.wflink + u(self.m) + 1
    }
    fn p(&self) -> usize {
        self.wblink + u(self.m) + 1
    }
    fn pmap(&self) -> usize {
        self.pinv
    }
    fn qmap(&self) -> usize {
        self.qinv
    }
    fn marked(&self) -> usize {
        self.iwork0
    }

    /// `basiclu_initialize` for dimension `m`, with BASICLU's defaults,
    /// and the user arrays at one element each as IPX starts them.
    pub fn new(m: Int) -> Self {
        let mu = u(m);
        let mut offset = 1;
        let mut take = |len: usize| {
            let at = offset;
            offset += len;
            at
        };
        let colcount_flink = take(2 * mu + 2);
        let colcount_blink = take(2 * mu + 2);
        let rowcount_flink = take(2 * mu + 2);
        let rowcount_blink = take(2 * mu + 2);
        let wbegin = take(2 * mu + 1);
        let wend = take(2 * mu + 1);
        let wflink = take(2 * mu + 1);
        let wblink = take(2 * mu + 1);
        let pinv = take(mu);
        let qinv = take(mu);
        let lbegin_p = take(mu + 1);
        let ubegin = take(mu + 1);
        let iwork0 = take(mu);
        let mut lu = Lu {
            droptol: 1e-20,
            abstol: 1e-14,
            reltol: 0.1,
            nzbias: 1,
            maxsearch: 3,
            pad: 4,
            stretch: 0.3,
            compress_thres: 0.5,
            sparse_thres: 0.05,
            search_rows: true,
            m,
            addmem_l: 0,
            addmem_u: 0,
            addmem_w: 0,
            nupdate: 0,
            nforrest: 0,
            nfactorize: 0,
            nupdate_total: 0,
            nforrest_total: 0,
            nsymperm_total: 0,
            lnz: 0,
            unz: 0,
            rnz: 0,
            min_pivot: 0.0,
            max_pivot: 0.0,
            max_eta: 0.0,
            update_cost_numer: 0.0,
            update_cost_denom: 1.0,
            lflops: 0,
            uflops: 0,
            rflops: 0,
            condest_l: 0.0,
            condest_u: 0.0,
            norm_l: 0.0,
            norm_u: 0.0,
            normest_linv: 0.0,
            normest_uinv: 0.0,
            onenorm: 0.0,
            infnorm: 0.0,
            residual_test: 0.0,
            matrix_nz: 0,
            rank: 0,
            bump_size: 0,
            bump_nz: 0,
            nsearch_pivot: 0,
            nexpand: 0,
            ngarbage: 0,
            factor_flops: 0,
            pivot_error: 0.0,
            task: NO_TASK,
            pivot_row: -1,
            pivot_col: -1,
            ftran_for_update: -1,
            btran_for_update: -1,
            marker: 0,
            pivotlen: 0,
            rankdef: 0,
            min_colnz: 1,
            min_rownz: 1,
            li: vec![0; 1],
            lx: vec![0.0; 1],
            ui: vec![0; 1],
            ux: vec![0.0; 1],
            wi: vec![0; 1],
            wx: vec![0.0; 1],
            is: vec![0; offset],
            colcount_flink,
            colcount_blink,
            rowcount_flink,
            rowcount_blink,
            wbegin,
            wend,
            wflink,
            wblink,
            pinv,
            qinv,
            lbegin_p,
            ubegin,
            iwork0,
            work0: vec![0.0; mu],
            work1: vec![0.0; mu],
            col_pivot: vec![0.0; mu],
            row_pivot: vec![0.0; mu],
            pstack: vec![0; mu],
            cancelled: vec![0; mu],
        };
        // lu_initialize loads with zero memory, then resets.
        lu.reset();
        lu
    }

    fn lmem(&self) -> Int {
        self.li.len() as Int
    }
    fn umem(&self) -> Int {
        self.ui.len() as Int
    }
    fn wmem(&self) -> Int {
        self.wi.len() as Int
    }

    /// What `lu_load` does on every call beyond copying fields.
    fn load(&mut self) {
        self.reltol = self.reltol.min(1.0);
        self.addmem_l = 0;
        self.addmem_u = 0;
        self.addmem_w = 0;
        let m = u(self.m);
        let wmem = self.wmem();
        if self.nupdate >= 0 {
            self.is[self.wend + m] = wmem;
        } else {
            self.is[self.wend + 2 * m] = wmem;
        }
    }

    fn reset(&mut self) {
        self.nupdate = -1;
        self.nforrest = 0;
        self.lnz = 0;
        self.unz = 0;
        self.rnz = 0;
        self.min_pivot = 0.0;
        self.max_pivot = 0.0;
        self.max_eta = 0.0;
        self.update_cost_numer = 0.0;
        self.update_cost_denom = 1.0;
        self.lflops = 0;
        self.uflops = 0;
        self.rflops = 0;
        self.condest_l = 0.0;
        self.condest_u = 0.0;
        self.norm_l = 0.0;
        self.norm_u = 0.0;
        self.normest_linv = 0.0;
        self.normest_uinv = 0.0;
        self.onenorm = 0.0;
        self.infnorm = 0.0;
        self.residual_test = 0.0;
        self.matrix_nz = 0;
        self.rank = 0;
        self.bump_size = 0;
        self.bump_nz = 0;
        self.nsearch_pivot = 0;
        self.nexpand = 0;
        self.ngarbage = 0;
        self.factor_flops = 0;
        self.pivot_error = 0.0;
        self.task = NO_TASK;
        self.pivot_row = -1;
        self.pivot_col = -1;
        self.ftran_for_update = -1;
        self.btran_for_update = -1;
        self.marker = 0;
        self.pivotlen = 0;
        self.rankdef = 0;
        self.min_colnz = 1;
        self.min_rownz = 1;
        let m = u(self.m);
        let wmem = self.wmem();
        self.is[self.wend + 2 * m] = wmem;
        let iwork0 = self.iwork0;
        self.is[iwork0..iwork0 + m].fill(0);
        self.work0.fill(0.0);
    }

    pub fn update_cost(&self) -> f64 {
        self.update_cost_numer / self.update_cost_denom
    }

    // ---- basiclu_factorize -----------------------------------------------

    /// `basiclu_factorize`; returns BASICLU's status, `REALLOCATE` asking
    /// for the user arrays to grow by `addmem_*` and the call to be made
    /// again with `c0ntinue`.
    pub fn factorize(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], bx: &[f64], c0ntinue: bool) -> Int {
        self.load();
        if !c0ntinue {
            self.reset();
            self.task = SINGLETONS;
        }
        if !(SINGLETONS..=BUILD_FACTORS).contains(&self.task) {
            return ERROR_INVALID_CALL;
        }
        if self.task <= SINGLETONS {
            self.task = SINGLETONS;
            let status = self.singletons(bbegin, bend, bi, bx);
            if status != OK {
                return status;
            }
        }
        if self.task <= SETUP_BUMP {
            self.task = SETUP_BUMP;
            let status = self.setup_bump(bbegin, bend, bi, bx);
            if status != OK {
                return status;
            }
        }
        if self.task <= FACTORIZE_BUMP {
            self.task = FACTORIZE_BUMP;
            let status = self.factorize_bump();
            if status != OK {
                return status;
            }
        }
        self.task = BUILD_FACTORS;
        let status = self.build_factors();
        if status != OK {
            return status;
        }
        self.task = NO_TASK;
        self.nupdate = 0;
        self.ftran_for_update = -1;
        self.btran_for_update = -1;
        self.nfactorize += 1;
        let m = u(self.m);
        let (lbegin, ubegin, p) = (self.lbegin(), self.ubegin, self.p());
        let (norm_l, normest_linv, condest_l) = condest(m, &self.is, lbegin, &self.li, &self.lx, None, Some(p), false, &mut self.work1);
        self.norm_l = norm_l;
        self.normest_linv = normest_linv;
        self.condest_l = condest_l;
        let (norm_u, normest_uinv, condest_u) = condest(m, &self.is, ubegin, &self.ui, &self.ux, Some(&self.row_pivot), Some(p), true, &mut self.work1);
        self.norm_u = norm_u;
        self.normest_uinv = normest_uinv;
        self.condest_u = condest_u;
        self.residual_test_(bbegin, bend, bi, bx);
        let factor_cost = 0.04 * self.m as f64 + 0.07 * self.matrix_nz as f64 + 0.20 * self.bump_nz as f64 + 0.20 * self.nsearch_pivot as f64 + 0.008 * self.factor_flops as f64;
        self.update_cost_denom = factor_cost * 250.0;
        if self.rank < self.m { WARNING_SINGULAR_MATRIX } else { OK }
    }

    fn singletons(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], bx: &[f64]) -> Int {
        let m = self.m;
        let mu = u(m);
        let mut bnz = 0;
        for j in 0..mu {
            if bend[j] < bbegin[j] {
                return ERROR_INVALID_ARGUMENT;
            }
            bnz += bend[j] - bbegin[j];
        }
        let mut ok = true;
        if self.lmem() < bnz {
            self.addmem_l = bnz - self.lmem();
            ok = false;
        }
        if self.umem() < bnz {
            self.addmem_u = bnz - self.umem();
            ok = false;
        }
        if self.wmem() < bnz {
            self.addmem_w = bnz - self.wmem();
            ok = false;
        }
        if !ok {
            return REALLOCATE;
        }
        let iwork1 = self.iwork1();
        let btp = self.wbegin;
        self.is[iwork1..iwork1 + mu].fill(0);
        for j in 0..mu {
            for pos in bbegin[j]..bend[j] {
                let i = bi[u(pos)];
                if i < 0 || i >= m {
                    return ERROR_INVALID_ARGUMENT;
                }
                self.is[iwork1 + u(i)] += 1;
            }
        }
        let mut put = 0;
        for i in 0..mu {
            self.is[btp + i] = put;
            put += self.is[iwork1 + i];
            self.is[iwork1 + i] = self.is[btp + i];
        }
        self.is[btp + mu] = put;
        let mut ok = true;
        for j in 0..mu {
            for pos in bbegin[j]..bend[j] {
                let i = u(bi[u(pos)]);
                let put = self.is[iwork1 + i];
                self.is[iwork1 + i] += 1;
                self.wi[u(put)] = j as Int;
                self.wx[u(put)] = bx[u(pos)];
                if put > self.is[btp + i] && self.wi[u(put) - 1] == j as Int {
                    ok = false;
                }
            }
        }
        if !ok {
            return ERROR_INVALID_ARGUMENT;
        }
        let (pinv, qinv) = (self.pinv, self.qinv);
        self.is[pinv..pinv + mu].fill(-1);
        self.is[qinv..qinv + mu].fill(-1);
        self.is[self.lbegin_p] = 0;
        self.is[self.ubegin] = 0;
        let mut rank = 0;
        if self.nzbias >= 0 {
            rank = self.singleton_cols(bbegin, bend, bi, bx, rank);
            rank = self.singleton_rows(bbegin, bend, bi, bx, rank);
        } else {
            rank = self.singleton_rows(bbegin, bend, bi, bx, rank);
            rank = self.singleton_cols(bbegin, bend, bi, bx, rank);
        }
        for i in 0..mu {
            if self.is[pinv + i] < 0 {
                self.is[pinv + i] = -1;
            }
            if self.is[qinv + i] < 0 {
                self.is[qinv + i] = -1;
            }
        }
        self.matrix_nz = bnz;
        self.rank = rank;
        OK
    }

    fn singleton_cols(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], _bx: &[f64], mut rank: Int) -> Int {
        let mu = u(self.m);
        let (btp, up, lp, pinv, qinv) = (self.wbegin, self.ubegin, self.lbegin_p, self.pinv, self.qinv);
        let iset = self.iwork1();
        let queue = iset + mu;
        let abstol = self.abstol;
        let rk0 = rank;
        let mut tail = 0usize;
        for j in 0..mu {
            if self.is[qinv + j] < 0 {
                let nz = bend[j] - bbegin[j];
                let mut i = 0;
                for pos in bbegin[j]..bend[j] {
                    i ^= bi[u(pos)];
                }
                self.is[iset + j] = i;
                self.is[qinv + j] = -nz - 1;
                if nz == 1 {
                    self.is[queue + tail] = j as Int;
                    tail += 1;
                }
            }
        }
        let mut put = self.is[up + u(rank)];
        let mut front = 0;
        while front < tail {
            let j = u(self.is[queue + front]);
            front += 1;
            if self.is[qinv + j] == -1 {
                continue;
            }
            let i = u(self.is[iset + j]);
            let end = self.is[btp + i + 1];
            let mut pos = self.is[btp + i];
            while self.wi[u(pos)] != j as Int {
                pos += 1;
            }
            let piv = self.wx[u(pos)];
            if piv == 0.0 || piv.abs() < abstol {
                continue;
            }
            self.is[qinv + j] = rank;
            self.is[pinv + i] = rank;
            for pos in self.is[btp + i]..end {
                let j2 = u(self.wi[u(pos)]);
                if self.is[qinv + j2] < 0 {
                    self.ui[u(put)] = j2 as Int;
                    self.ux[u(put)] = self.wx[u(pos)];
                    put += 1;
                    self.is[iset + j2] ^= i as Int;
                    self.is[qinv + j2] += 1;
                    if self.is[qinv + j2] == -2 {
                        self.is[queue + tail] = j2 as Int;
                        tail += 1;
                    }
                }
            }
            self.is[up + u(rank) + 1] = put;
            self.col_pivot[j] = piv;
            rank += 1;
        }
        let mut pos = self.is[lp + u(rk0)];
        for rk in rk0..rank {
            self.li[u(pos)] = -1;
            pos += 1;
            self.is[lp + u(rk) + 1] = pos;
        }
        rank
    }

    fn singleton_rows(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], bx: &[f64], mut rank: Int) -> Int {
        let mu = u(self.m);
        let (btp, up, lp, pinv, qinv) = (self.wbegin, self.ubegin, self.lbegin_p, self.pinv, self.qinv);
        let iset = self.iwork1();
        let queue = iset + mu;
        let abstol = self.abstol;
        let rk0 = rank;
        let mut tail = 0usize;
        for i in 0..mu {
            if self.is[pinv + i] < 0 {
                let nz = self.is[btp + i + 1] - self.is[btp + i];
                let mut j = 0;
                for pos in self.is[btp + i]..self.is[btp + i + 1] {
                    j ^= self.wi[u(pos)];
                }
                self.is[iset + i] = j;
                self.is[pinv + i] = -nz - 1;
                if nz == 1 {
                    self.is[queue + tail] = i as Int;
                    tail += 1;
                }
            }
        }
        let mut put = self.is[lp + u(rank)];
        let mut front = 0;
        while front < tail {
            let i = u(self.is[queue + front]);
            front += 1;
            if self.is[pinv + i] == -1 {
                continue;
            }
            let j = u(self.is[iset + i]);
            let end = bend[j];
            let mut pos = bbegin[j];
            while bi[u(pos)] != i as Int {
                pos += 1;
            }
            let piv = bx[u(pos)];
            if piv == 0.0 || piv.abs() < abstol {
                continue;
            }
            self.is[qinv + j] = rank;
            self.is[pinv + i] = rank;
            for pos in bbegin[j]..end {
                let i2 = u(bi[u(pos)]);
                if self.is[pinv + i2] < 0 {
                    self.li[u(put)] = i2 as Int;
                    self.lx[u(put)] = bx[u(pos)] / piv;
                    put += 1;
                    self.is[iset + i2] ^= j as Int;
                    self.is[pinv + i2] += 1;
                    if self.is[pinv + i2] == -2 {
                        self.is[queue + tail] = i2 as Int;
                        tail += 1;
                    }
                }
            }
            self.li[u(put)] = -1;
            put += 1;
            self.is[lp + u(rank) + 1] = put;
            self.col_pivot[j] = piv;
            rank += 1;
        }
        let pos = self.is[up + u(rk0)];
        for rk in rk0..rank {
            self.is[up + u(rk) + 1] = pos;
        }
        rank
    }

    fn setup_bump(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], bx: &[f64]) -> Int {
        let m = self.m;
        let mu = u(m);
        let rank = self.rank;
        let wmem = self.wmem();
        let bnz = self.matrix_nz;
        let lnz = self.is[self.lbegin_p + u(rank)] - rank;
        let unz = self.is[self.ubegin + u(rank)];
        let abstol = self.abstol;
        let pad = self.pad;
        let stretch = self.stretch;
        let (ccf, ccb, rcf, rcb) = (self.colcount_flink, self.colcount_blink, self.rowcount_flink, self.rowcount_blink);
        let (pinv, qinv, wbegin, wend, wflink, wblink, iwork0) = (self.pinv, self.qinv, self.wbegin, self.wend, self.wflink, self.wblink, self.iwork0);
        let wbegin2 = wbegin + mu;
        let wend2 = wend + mu;
        let mut bump_nz = bnz - lnz - unz - rank;
        let mut need = trunc(bump_nz as f64 + stretch * bump_nz as f64 + ((m - rank) * pad) as f64);
        need *= 2;
        if need > wmem {
            self.addmem_w = need - wmem;
            return REALLOCATE;
        }
        file_empty(&mut self.is, 2 * mu, wbegin, wend, wflink, wblink, wmem);
        let mut min_colnz = 0;
        list_init(&mut self.is, ccf, ccb, mu, mu + 2, Some(&mut min_colnz));
        let mut put: Int = 0;
        for j in 0..mu {
            if self.is[qinv + j] >= 0 {
                continue;
            }
            let mut cnz = 0;
            let mut cmx: f64 = 0.0;
            for pos in bbegin[j]..bend[j] {
                let i = u(bi[u(pos)]);
                if self.is[pinv + i] >= 0 {
                    continue;
                }
                cmx = cmx.max(bx[u(pos)].abs());
                cnz += 1;
            }
            if cmx == 0.0 || cmx < abstol {
                self.col_pivot[j] = 0.0;
                list_add(&mut self.is, ccf, ccb, j, 0, mu, Some(&mut min_colnz));
                bump_nz -= cnz;
            } else {
                self.col_pivot[j] = cmx;
                list_add(&mut self.is, ccf, ccb, j, u(cnz), mu, Some(&mut min_colnz));
                self.is[wbegin + j] = put;
                for pos in bbegin[j]..bend[j] {
                    let i = u(bi[u(pos)]);
                    if self.is[pinv + i] >= 0 {
                        continue;
                    }
                    self.wi[u(put)] = i as Int;
                    self.wx[u(put)] = bx[u(pos)];
                    put += 1;
                    self.is[iwork0 + i] += 1;
                }
                self.is[wend + j] = put;
                put = trunc(put as f64 + (stretch * cnz as f64 + pad as f64));
                list_move(&mut self.is, wflink, wblink, j, 0, 2 * mu, None);
            }
        }
        let mut min_rownz = 0;
        list_init(&mut self.is, rcf, rcb, mu, mu + 2, Some(&mut min_rownz));
        for i in 0..mu {
            if self.is[pinv + i] >= 0 {
                continue;
            }
            let rnz = self.is[iwork0 + i];
            self.is[iwork0 + i] = 0;
            list_add(&mut self.is, rcf, rcb, i, u(rnz), mu, Some(&mut min_rownz));
            self.is[wbegin2 + i] = put;
            self.is[wend2 + i] = put;
            put += rnz;
            list_move(&mut self.is, wflink, wblink, mu + i, 0, 2 * mu, None);
            put = trunc(put as f64 + (stretch * rnz as f64 + pad as f64));
        }
        for j in 0..mu {
            for pos in self.is[wbegin + j]..self.is[wend + j] {
                let i = u(self.wi[u(pos)]);
                let at = self.is[wend2 + i];
                self.is[wend2 + i] += 1;
                self.wi[u(at)] = j as Int;
            }
        }
        self.is[wbegin + 2 * mu] = put;
        self.bump_nz = bump_nz;
        self.bump_size = m - rank;
        self.min_colnz = min_colnz;
        self.min_rownz = min_rownz;
        OK
    }

    fn factorize_bump(&mut self) -> Int {
        let m = self.m;
        let (ccf, ccb, pinv, qinv) = (self.colcount_flink, self.colcount_blink, self.pinv, self.qinv);
        let mut status = OK;
        while self.rank + self.rankdef < m {
            if self.pivot_col < 0 {
                self.markowitz();
            }
            if self.pivot_row < 0 {
                list_remove(&mut self.is, ccf, ccb, u(self.pivot_col));
                self.pivot_col = -1;
                self.rankdef += 1;
            } else {
                status = self.pivot();
                if status != OK {
                    break;
                }
                self.is[pinv + u(self.pivot_row)] = self.rank;
                self.is[qinv + u(self.pivot_col)] = self.rank;
                self.pivot_col = -1;
                self.pivot_row = -1;
                self.rank += 1;
            }
        }
        status
    }

    fn markowitz(&mut self) {
        let m = self.m;
        let mu = u(m);
        let (wbegin, wend, ccf, rcf, rcb) = (self.wbegin, self.wend, self.colcount_flink, self.rowcount_flink, self.rowcount_blink);
        let abstol = self.abstol;
        let reltol = self.reltol;
        let maxsearch = self.maxsearch;
        let search_rows = self.search_rows;
        let nz_start = if search_rows { self.min_colnz.min(self.min_rownz) } else { self.min_colnz };
        let mut pivot_row: Int = -1;
        let mut pivot_col: Int = -1;
        // BASICLU's Markowitz costs are 64-bit (`int_least64_t`), since
        // m * m overflows `lu_int`.
        let big_m = i64::from(m);
        let mut mc_best = big_m * big_m;
        let mut nsearch = 0;
        let mut min_colnz: Int = -1;
        let mut min_rownz: Int = -1;
        'done: {
            if self.is[ccf + mu] != m {
                pivot_col = self.is[ccf + mu];
                break 'done;
            }
            for nz in nz_start..=m {
                let mut j = self.is[ccf + mu + u(nz)];
                while j < m {
                    let ju = u(j);
                    if min_colnz == -1 {
                        min_colnz = nz;
                    }
                    let cmx = self.col_pivot[ju];
                    if !(cmx == 0.0 || cmx < abstol) {
                        let tol = abstol.max(reltol * cmx);
                        for pos in self.is[wbegin + ju]..self.is[wend + ju] {
                            let x = self.wx[u(pos)].abs();
                            if x == 0.0 || x < tol {
                                continue;
                            }
                            let i = self.wi[u(pos)];
                            let nz1 = i64::from(nz);
                            let nz2 = i64::from(self.is[wend + mu + u(i)] - self.is[wbegin + mu + u(i)]);
                            let mc = (nz1 - 1) * (nz2 - 1);
                            if mc < mc_best {
                                mc_best = mc;
                                pivot_row = i;
                                pivot_col = j;
                                if search_rows && mc_best <= (nz1 - 1) * (nz1 - 1) {
                                    break 'done;
                                }
                            }
                        }
                        nsearch += 1;
                        if nsearch >= maxsearch {
                            break 'done;
                        }
                    }
                    j = self.is[ccf + ju];
                }
                if !search_rows {
                    continue;
                }
                let mut i = self.is[rcf + mu + u(nz)];
                while i < m {
                    let iu = u(i);
                    if min_rownz == -1 {
                        min_rownz = nz;
                    }
                    let inext = self.is[rcf + iu];
                    let mut cheap = false;
                    let mut found = false;
                    for pos in self.is[wbegin + mu + iu]..self.is[wend + mu + iu] {
                        let j = self.wi[u(pos)];
                        let ju = u(j);
                        let nz1 = i64::from(nz);
                        let nz2 = i64::from(self.is[wend + ju] - self.is[wbegin + ju]);
                        let mc = (nz1 - 1) * (nz2 - 1);
                        if mc >= mc_best {
                            continue;
                        }
                        cheap = true;
                        let cmx = self.col_pivot[ju];
                        if cmx == 0.0 || cmx < abstol {
                            continue;
                        }
                        let mut where_ = self.is[wbegin + ju];
                        while self.wi[u(where_)] != i {
                            where_ += 1;
                        }
                        let x = self.wx[u(where_)].abs();
                        if x >= abstol && x >= reltol * cmx {
                            found = true;
                            mc_best = mc;
                            pivot_row = i;
                            pivot_col = j;
                            if mc_best <= nz1 * (nz1 - 1) {
                                break 'done;
                            }
                        }
                    }
                    if cheap && !found {
                        list_move(&mut self.is, rcf, rcb, iu, mu + 1, mu, None);
                    } else {
                        nsearch += 1;
                        if nsearch >= maxsearch {
                            break 'done;
                        }
                    }
                    i = inext;
                }
            }
        }
        self.pivot_row = pivot_row;
        self.pivot_col = pivot_col;
        self.nsearch_pivot += nsearch;
        if min_colnz >= 0 {
            self.min_colnz = min_colnz;
        }
        if min_rownz >= 0 {
            self.min_rownz = min_rownz;
        }
    }

    fn pivot(&mut self) -> Int {
        let mu = u(self.m);
        let rank = u(self.rank);
        let pivot_col = u(self.pivot_col);
        let pivot_row = u(self.pivot_row);
        let (wbegin, wend) = (self.wbegin, self.wend);
        let nz_col = self.is[wend + pivot_col] - self.is[wbegin + pivot_col];
        let nz_row = self.is[wend + mu + pivot_row] - self.is[wbegin + mu + pivot_row];
        let mut status = OK;
        let room = self.lmem() - self.is[self.lbegin_p + rank];
        let need = nz_col;
        if room < need {
            self.addmem_l = need - room;
            status = REALLOCATE;
        }
        let room = self.umem() - self.is[self.ubegin + rank];
        let need = nz_row - 1;
        if room < need {
            self.addmem_u = need - room;
            status = REALLOCATE;
        }
        if status != OK {
            return status;
        }
        status = if nz_row == 1 {
            self.pivot_singleton_row()
        } else if nz_col == 1 {
            self.pivot_singleton_col()
        } else if nz_col == 2 {
            self.pivot_doubleton_col()
        } else if nz_col - 1 <= MAXROW_SMALL {
            self.pivot_small_or_any(true)
        } else {
            self.pivot_small_or_any(false)
        };
        if status == OK {
            let ubegin = self.ubegin;
            for pos in self.is[ubegin + rank]..self.is[ubegin + rank + 1] {
                let j = u(self.ui[u(pos)]);
                if self.col_pivot[j] == 0.0 || self.col_pivot[j] < self.abstol {
                    self.remove_col(j);
                }
            }
        }
        self.factor_flops += (nz_col - 1) * (nz_row - 1);
        status
    }

    /// `lu_pivot_any` and, with `small`, `lu_pivot_small`, which differ
    /// only in dropping cancelled entries from the row file.
    fn pivot_small_or_any(&mut self, small: bool) -> Int {
        let m = u(self.m);
        let rank = u(self.rank);
        let droptol = self.droptol;
        let pad = self.pad;
        let stretch = self.stretch;
        let pivot_col = u(self.pivot_col);
        let pivot_row = u(self.pivot_row);
        let (ccf, ccb, rcf, rcb) = (self.colcount_flink, self.colcount_blink, self.rowcount_flink, self.rowcount_blink);
        let (lbegin_p, ubegin, wbegin, wend, wflink, wblink, marked) = (self.lbegin_p, self.ubegin, self.wbegin, self.wend, self.wflink, self.wblink, self.iwork0);
        let mut cbeg = self.is[wbegin + pivot_col];
        let mut cend = self.is[wend + pivot_col];
        let mut rbeg = self.is[wbegin + m + pivot_row];
        let mut rend = self.is[wend + m + pivot_row];
        let cnz1 = cend - cbeg - 1;
        let rnz1 = rend - rbeg - 1;

        let mut grow: Int = 0;
        let mut where_: Int = -1;
        for pos in cbeg..cend {
            let i = self.wi[u(pos)];
            if i == pivot_row as Int {
                where_ = pos;
            } else {
                let nz = self.is[wend + m + u(i)] - self.is[wbegin + m + u(i)];
                grow = trunc(grow as f64 + ((nz + rnz1) as f64 + stretch * (nz + rnz1) as f64 + pad as f64));
            }
        }
        self.wi.swap(u(cbeg), u(where_));
        self.wx.swap(u(cbeg), u(where_));
        let pivot = self.wx[u(cbeg)];
        where_ = -1;
        for rpos in rbeg..rend {
            let j = self.wi[u(rpos)];
            if j == pivot_col as Int {
                where_ = rpos;
            } else {
                let nz = self.is[wend + u(j)] - self.is[wbegin + u(j)];
                grow = trunc(grow as f64 + ((nz + cnz1) as f64 + stretch * (nz + cnz1) as f64 + pad as f64));
            }
        }
        self.wi.swap(u(rbeg), u(where_));
        let mut room = self.is[wend + 2 * m] - self.is[wbegin + 2 * m];
        if grow > room {
            file_compress(&mut self.is, 2 * m, wbegin, wend, wflink, &mut self.wi, &mut self.wx, stretch, pad);
            cbeg = self.is[wbegin + pivot_col];
            cend = self.is[wend + pivot_col];
            rbeg = self.is[wbegin + m + pivot_row];
            rend = self.is[wend + m + pivot_row];
            room = self.is[wend + 2 * m] - self.is[wbegin + 2 * m];
            self.ngarbage += 1;
        }
        if grow > room {
            self.addmem_w = grow - room;
            return REALLOCATE;
        }
        let mut uput = self.is[ubegin + rank];
        let mut position = 1;
        for pos in cbeg + 1..cend {
            let i = u(self.wi[u(pos)]);
            self.is[marked + i] = position;
            position += 1;
        }
        // The pivot column's entries, which the eliminations read while
        // the file moves around them: HiGHS's `wi`/`wx` point at them.
        let wi_base = u(cbeg);
        let mut col_number = 0usize;
        for rpos in rbeg + 1..rend {
            let j = u(self.wi[u(rpos)]);
            let mut cmx: f64 = 0.0;
            where_ = -1;
            let pos1 = self.is[wbegin + j];
            let mut put = pos1;
            for pos in pos1..self.is[wend + j] {
                let i = self.wi[u(pos)];
                let position = self.is[marked + u(i)];
                if position > 0 {
                    self.work0[u(position)] = self.wx[u(pos)];
                } else {
                    if i == pivot_row as Int {
                        where_ = put;
                    } else {
                        let x = self.wx[u(pos)].abs();
                        if x > cmx {
                            cmx = x;
                        }
                    }
                    self.wi[u(put)] = self.wi[u(pos)];
                    self.wx[u(put)] = self.wx[u(pos)];
                    put += 1;
                }
            }
            self.is[wend + j] = put;
            self.wi.swap(u(pos1), u(where_));
            self.wx.swap(u(pos1), u(where_));
            let xrj = self.wx[u(pos1)];
            let mut room = self.is[wbegin + u(self.is[wflink + j])] - put;
            if room < cnz1 {
                let nz = self.is[wend + j] - self.is[wbegin + j];
                room = trunc(cnz1 as f64 + stretch * (nz + cnz1) as f64 + pad as f64);
                file_reappend(&mut self.is, j, 2 * m, wbegin, wend, wflink, wblink, &mut self.wi, &mut self.wx, room);
                put = self.is[wend + j];
                self.nexpand += 1;
            }
            let a = xrj / pivot;
            for pos in 1..=u(cnz1) {
                self.work0[pos] -= a * self.wx[wi_base + pos];
            }
            if small {
                let mut mask: i64 = 0;
                for pos in 1..=u(cnz1) {
                    let x = self.work0[pos].abs();
                    if x > droptol {
                        self.wi[u(put)] = self.wi[wi_base + pos];
                        self.wx[u(put)] = self.work0[pos];
                        put += 1;
                        if x > cmx {
                            cmx = x;
                        }
                    } else {
                        mask |= 1i64 << (pos - 1);
                    }
                    self.work0[pos] = 0.0;
                }
                self.cancelled[col_number] = mask;
            } else {
                for pos in 1..=u(cnz1) {
                    self.wi[u(put)] = self.wi[wi_base + pos];
                    self.wx[u(put)] = self.work0[pos];
                    put += 1;
                    let x = self.work0[pos].abs();
                    if x > cmx {
                        cmx = x;
                    }
                    self.work0[pos] = 0.0;
                }
            }
            self.is[wend + j] = put;
            if xrj.abs() > droptol {
                self.ui[u(uput)] = j as Int;
                self.ux[u(uput)] = xrj;
                uput += 1;
            }
            self.is[wbegin + j] += 1;
            let nz = self.is[wend + j] - self.is[wbegin + j];
            list_move(&mut self.is, ccf, ccb, j, u(nz), m, Some(&mut self.min_colnz));
            self.col_pivot[j] = cmx;
            col_number += 1;
        }
        for pos in cbeg + 1..cend {
            let i = u(self.wi[u(pos)]);
            self.is[marked + i] = 0;
        }
        for rpos in rbeg..rend {
            let j = u(self.wi[u(rpos)]);
            self.is[marked + j] = 1;
        }
        let mut mask: i64 = 1;
        for pos in cbeg + 1..cend {
            let i = u(self.wi[u(pos)]);
            let mut put = self.is[wbegin + m + i];
            for rpos in self.is[wbegin + m + i]..self.is[wend + m + i] {
                let j = self.wi[u(rpos)];
                if self.is[marked + u(j)] == 0 {
                    self.wi[u(put)] = j;
                    put += 1;
                }
            }
            self.is[wend + m + i] = put;
            let mut room = self.is[wbegin + u(self.is[wflink + m + i])] - put;
            if room < rnz1 {
                let nz = self.is[wend + m + i] - self.is[wbegin + m + i];
                room = trunc(rnz1 as f64 + stretch * (nz + rnz1) as f64 + pad as f64);
                file_reappend(&mut self.is, m + i, 2 * m, wbegin, wend, wflink, wblink, &mut self.wi, &mut self.wx, room);
                put = self.is[wend + m + i];
                self.nexpand += 1;
            }
            if small {
                let mut col_number = 0usize;
                for rpos in rbeg + 1..rend {
                    if self.cancelled[col_number] & mask == 0 {
                        self.wi[u(put)] = self.wi[u(rpos)];
                        put += 1;
                    }
                    col_number += 1;
                }
                mask <<= 1;
            } else {
                for rpos in rbeg + 1..rend {
                    self.wi[u(put)] = self.wi[u(rpos)];
                    put += 1;
                }
            }
            self.is[wend + m + i] = put;
            let nz = self.is[wend + m + i] - self.is[wbegin + m + i];
            list_move(&mut self.is, rcf, rcb, i, u(nz), m, Some(&mut self.min_rownz));
        }
        for rpos in rbeg..rend {
            let j = u(self.wi[u(rpos)]);
            self.is[marked + j] = 0;
        }
        let mut put = self.is[lbegin_p + rank];
        for pos in cbeg + 1..cend {
            let x = self.wx[u(pos)] / pivot;
            if x.abs() > droptol {
                self.li[u(put)] = self.wi[u(pos)];
                self.lx[u(put)] = x;
                put += 1;
            }
        }
        self.li[u(put)] = -1;
        put += 1;
        self.is[lbegin_p + rank + 1] = put;
        self.is[ubegin + rank + 1] = uput;
        self.col_pivot[pivot_col] = pivot;
        self.is[wend + pivot_col] = cbeg;
        self.is[wend + m + pivot_row] = rbeg;
        list_remove(&mut self.is, ccf, ccb, pivot_col);
        list_remove(&mut self.is, rcf, rcb, pivot_row);
        OK
    }

    fn pivot_singleton_row(&mut self) -> Int {
        let m = u(self.m);
        let rank = u(self.rank);
        let droptol = self.droptol;
        let pivot_col = u(self.pivot_col);
        let pivot_row = u(self.pivot_row);
        let (ccf, ccb, rcf, rcb) = (self.colcount_flink, self.colcount_blink, self.rowcount_flink, self.rowcount_blink);
        let (lbegin_p, ubegin, wbegin, wend) = (self.lbegin_p, self.ubegin, self.wbegin, self.wend);
        let cbeg = self.is[wbegin + pivot_col];
        let cend = self.is[wend + pivot_col];
        let rbeg = self.is[wbegin + m + pivot_row];
        let mut where_ = cbeg;
        while self.wi[u(where_)] != pivot_row as Int {
            where_ += 1;
        }
        let pivot = self.wx[u(where_)];
        let mut put = self.is[lbegin_p + rank];
        for pos in cbeg..cend {
            let x = self.wx[u(pos)] / pivot;
            if pos != where_ && x.abs() > droptol {
                self.li[u(put)] = self.wi[u(pos)];
                self.lx[u(put)] = x;
                put += 1;
            }
        }
        self.li[u(put)] = -1;
        put += 1;
        self.is[lbegin_p + rank + 1] = put;
        self.is[ubegin + rank + 1] = self.is[ubegin + rank];
        for pos in cbeg..cend {
            let i = u(self.wi[u(pos)]);
            if i == pivot_row {
                continue;
            }
            let mut where_ = self.is[wbegin + m + i];
            while self.wi[u(where_)] != pivot_col as Int {
                where_ += 1;
            }
            self.is[wend + m + i] -= 1;
            self.wi[u(where_)] = self.wi[u(self.is[wend + m + i])];
            let nz = self.is[wend + m + i] - self.is[wbegin + m + i];
            list_move(&mut self.is, rcf, rcb, i, u(nz), m, Some(&mut self.min_rownz));
        }
        self.col_pivot[pivot_col] = pivot;
        self.is[wend + pivot_col] = cbeg;
        self.is[wend + m + pivot_row] = rbeg;
        list_remove(&mut self.is, ccf, ccb, pivot_col);
        list_remove(&mut self.is, rcf, rcb, pivot_row);
        OK
    }

    fn pivot_singleton_col(&mut self) -> Int {
        let m = u(self.m);
        let rank = u(self.rank);
        let droptol = self.droptol;
        let pivot_col = u(self.pivot_col);
        let pivot_row = u(self.pivot_row);
        let (ccf, ccb, rcf, rcb) = (self.colcount_flink, self.colcount_blink, self.rowcount_flink, self.rowcount_blink);
        let (lbegin_p, ubegin, wbegin, wend) = (self.lbegin_p, self.ubegin, self.wbegin, self.wend);
        let cbeg = self.is[wbegin + pivot_col];
        let rbeg = self.is[wbegin + m + pivot_row];
        let rend = self.is[wend + m + pivot_row];
        let mut put = self.is[ubegin + rank];
        let pivot = self.wx[u(cbeg)];
        let mut xrj = 0.0;
        for rpos in rbeg..rend {
            let j = u(self.wi[u(rpos)]);
            if j == pivot_col {
                continue;
            }
            let mut where_: Int = -1;
            let mut cmx: f64 = 0.0;
            for pos in self.is[wbegin + j]..self.is[wend + j] {
                if self.wi[u(pos)] == pivot_row as Int {
                    where_ = pos;
                    xrj = self.wx[u(pos)];
                } else {
                    let x = self.wx[u(pos)].abs();
                    if x > cmx {
                        cmx = x;
                    }
                }
            }
            if f64::abs(xrj) > droptol {
                self.ui[u(put)] = j as Int;
                self.ux[u(put)] = xrj;
                put += 1;
            }
            self.is[wend + j] -= 1;
            let end = u(self.is[wend + j]);
            self.wi[u(where_)] = self.wi[end];
            self.wx[u(where_)] = self.wx[end];
            let nz = self.is[wend + j] - self.is[wbegin + j];
            list_move(&mut self.is, ccf, ccb, j, u(nz), m, Some(&mut self.min_colnz));
            self.col_pivot[j] = cmx;
        }
        self.is[ubegin + rank + 1] = put;
        let mut put = self.is[lbegin_p + rank];
        self.li[u(put)] = -1;
        put += 1;
        self.is[lbegin_p + rank + 1] = put;
        self.col_pivot[pivot_col] = pivot;
        self.is[wend + pivot_col] = cbeg;
        self.is[wend + m + pivot_row] = rbeg;
        list_remove(&mut self.is, ccf, ccb, pivot_col);
        list_remove(&mut self.is, rcf, rcb, pivot_row);
        OK
    }

    fn pivot_doubleton_col(&mut self) -> Int {
        let m = u(self.m);
        let rank = u(self.rank);
        let droptol = self.droptol;
        let pad = self.pad;
        let stretch = self.stretch;
        let pivot_col = u(self.pivot_col);
        let pivot_row = u(self.pivot_row);
        let (ccf, ccb, rcf, rcb) = (self.colcount_flink, self.colcount_blink, self.rowcount_flink, self.rowcount_blink);
        let (lbegin_p, ubegin, wbegin, wend, wflink, wblink, marked) = (self.lbegin_p, self.ubegin, self.wbegin, self.wend, self.wflink, self.wblink, self.iwork0);
        let mut cbeg = self.is[wbegin + pivot_col];
        let mut rbeg = self.is[wbegin + m + pivot_row];
        let mut rend = self.is[wend + m + pivot_row];
        let rnz1 = rend - rbeg - 1;
        if self.wi[u(cbeg)] != pivot_row as Int {
            self.wi.swap(u(cbeg), u(cbeg) + 1);
            self.wx.swap(u(cbeg), u(cbeg) + 1);
        }
        let pivot = self.wx[u(cbeg)];
        let other_row = u(self.wi[u(cbeg) + 1]);
        let other_value = self.wx[u(cbeg) + 1];
        let mut where_ = rbeg;
        while self.wi[u(where_)] != pivot_col as Int {
            where_ += 1;
        }
        self.wi.swap(u(rbeg), u(where_));
        let nz = self.is[wend + m + other_row] - self.is[wbegin + m + other_row];
        let grow = trunc((nz + rnz1) as f64 + stretch * (nz + rnz1) as f64 + pad as f64);
        let mut room = self.is[wend + 2 * m] - self.is[wbegin + 2 * m];
        if grow > room {
            file_compress(&mut self.is, 2 * m, wbegin, wend, wflink, &mut self.wi, &mut self.wx, stretch, pad);
            cbeg = self.is[wbegin + pivot_col];
            rbeg = self.is[wbegin + m + pivot_row];
            rend = self.is[wend + m + pivot_row];
            room = self.is[wend + 2 * m] - self.is[wbegin + 2 * m];
            self.ngarbage += 1;
        }
        if grow > room {
            self.addmem_w = grow - room;
            return REALLOCATE;
        }
        let mut uput = self.is[ubegin + rank];
        let mut put = rbeg + 1;
        let mut ncancelled = 0;
        for rpos in rbeg + 1..rend {
            let j = u(self.wi[u(rpos)]);
            let mut cmx: f64 = 0.0;
            let mut where_pivot: Int = -1;
            let mut where_other: Int = -1;
            let mut end = self.is[wend + j];
            for pos in self.is[wbegin + j]..end {
                let index = self.wi[u(pos)];
                if index == pivot_row as Int {
                    where_pivot = pos;
                } else if index == other_row as Int {
                    where_other = pos;
                } else {
                    let x = self.wx[u(pos)].abs();
                    if x > cmx {
                        cmx = x;
                    }
                }
            }
            let xrj = self.wx[u(where_pivot)];
            if self.wx[u(where_pivot)].abs() > droptol {
                self.ui[u(uput)] = j as Int;
                self.ux[u(uput)] = self.wx[u(where_pivot)];
                uput += 1;
            }
            if where_other == -1 {
                let x = -xrj * (other_value / pivot);
                let xabs = x.abs();
                if xabs > droptol {
                    self.wi[u(where_pivot)] = other_row as Int;
                    self.wx[u(where_pivot)] = x;
                    self.wi[u(put)] = j as Int;
                    put += 1;
                    if xabs > cmx {
                        cmx = xabs;
                    }
                } else {
                    self.is[wend + j] -= 1;
                    end = self.is[wend + j];
                    self.wi[u(where_pivot)] = self.wi[u(end)];
                    self.wx[u(where_pivot)] = self.wx[u(end)];
                    let nz = end - self.is[wbegin + j];
                    list_move(&mut self.is, ccf, ccb, j, u(nz), m, Some(&mut self.min_colnz));
                }
            } else {
                self.is[wend + j] -= 1;
                end = self.is[wend + j];
                self.wi[u(where_pivot)] = self.wi[u(end)];
                self.wx[u(where_pivot)] = self.wx[u(end)];
                if where_other == end {
                    where_other = where_pivot;
                }
                self.wx[u(where_other)] -= xrj * (other_value / pivot);
                let x = self.wx[u(where_other)].abs();
                if x <= droptol {
                    self.is[wend + j] -= 1;
                    end = self.is[wend + j];
                    self.wi[u(where_other)] = self.wi[u(end)];
                    self.wx[u(where_other)] = self.wx[u(end)];
                    self.is[marked + j] = 1;
                    ncancelled += 1;
                } else if x > cmx {
                    cmx = x;
                }
                let nz = self.is[wend + j] - self.is[wbegin + j];
                list_move(&mut self.is, ccf, ccb, j, u(nz), m, Some(&mut self.min_colnz));
            }
            self.col_pivot[j] = cmx;
        }
        rend = put;
        self.is[ubegin + rank + 1] = uput;
        if ncancelled > 0 {
            self.is[marked + pivot_col] = 1;
            let mut put = self.is[wbegin + m + other_row];
            let end = self.is[wend + m + other_row];
            #[allow(clippy::mut_range_bound, reason = "the range starts at put, read once as C's loop does")]
            for pos in put..end {
                let j = self.wi[u(pos)];
                if self.is[marked + u(j)] != 0 {
                    self.is[marked + u(j)] = 0;
                } else {
                    self.wi[u(put)] = j;
                    put += 1;
                }
            }
            self.is[wend + m + other_row] = put;
        } else {
            let mut where_ = self.is[wbegin + m + other_row];
            while self.wi[u(where_)] != pivot_col as Int {
                where_ += 1;
            }
            self.is[wend + m + other_row] -= 1;
            let end = self.is[wend + m + other_row];
            self.wi[u(where_)] = self.wi[u(end)];
        }
        let nfill = rend - (rbeg + 1);
        let room = self.is[wbegin + u(self.is[wflink + m + other_row])] - self.is[wend + m + other_row];
        if nfill > room {
            let nz = self.is[wend + m + other_row] - self.is[wbegin + m + other_row];
            let space = trunc(nfill as f64 + stretch * (nz + nfill) as f64 + pad as f64);
            file_reappend(&mut self.is, m + other_row, 2 * m, wbegin, wend, wflink, wblink, &mut self.wi, &mut self.wx, space);
            self.nexpand += 1;
        }
        let mut put = self.is[wend + m + other_row];
        for pos in rbeg + 1..rend {
            self.wi[u(put)] = self.wi[u(pos)];
            put += 1;
        }
        self.is[wend + m + other_row] = put;
        let nz = self.is[wend + m + other_row] - self.is[wbegin + m + other_row];
        list_move(&mut self.is, rcf, rcb, other_row, u(nz), m, Some(&mut self.min_rownz));
        let mut put = self.is[lbegin_p + rank];
        let x = other_value / pivot;
        if x.abs() > droptol {
            self.li[u(put)] = other_row as Int;
            self.lx[u(put)] = x;
            put += 1;
        }
        self.li[u(put)] = -1;
        put += 1;
        self.is[lbegin_p + rank + 1] = put;
        self.col_pivot[pivot_col] = pivot;
        self.is[wend + pivot_col] = cbeg;
        self.is[wend + m + pivot_row] = rbeg;
        list_remove(&mut self.is, ccf, ccb, pivot_col);
        list_remove(&mut self.is, rcf, rcb, pivot_row);
        OK
    }

    fn remove_col(&mut self, j: usize) {
        let m = u(self.m);
        let (ccf, ccb, rcf, rcb, wbegin, wend) = (self.colcount_flink, self.colcount_blink, self.rowcount_flink, self.rowcount_blink, self.wbegin, self.wend);
        let cbeg = self.is[wbegin + j];
        let cend = self.is[wend + j];
        for pos in cbeg..cend {
            let i = u(self.wi[u(pos)]);
            let mut where_ = self.is[wbegin + m + i];
            while self.wi[u(where_)] != j as Int {
                where_ += 1;
            }
            self.is[wend + m + i] -= 1;
            self.wi[u(where_)] = self.wi[u(self.is[wend + m + i])];
            let nz = self.is[wend + m + i] - self.is[wbegin + m + i];
            list_move(&mut self.is, rcf, rcb, i, u(nz), m, Some(&mut self.min_rownz));
        }
        self.col_pivot[j] = 0.0;
        self.is[wend + j] = cbeg;
        list_move(&mut self.is, ccf, ccb, j, 0, m, Some(&mut self.min_colnz));
    }

    fn build_factors(&mut self) -> Int {
        let m = self.m;
        let mu = u(m);
        let rank = self.rank;
        let pad = self.pad;
        let stretch = self.stretch;
        let (pinv, qinv, pmap, qmap) = (self.pinv, self.qinv, self.pmap(), self.qmap());
        let (pivotcol, pivotrow) = (self.pivotcol(), self.pivotrow());
        let (lbegin, lbegin_p, ltbegin, ltbegin_p, ubegin, rbegin) = (self.lbegin(), self.lbegin_p, self.ltbegin(), self.ltbegin_p(), self.ubegin, self.rbegin());
        let (wbegin, wend, wflink, wblink) = (self.wbegin, self.wend, self.wflink, self.wblink);
        let iwork1 = self.iwork1();
        let p = self.p();
        let mut lnz = self.is[lbegin_p + u(rank)];
        lnz -= rank;
        let mut unz = self.is[ubegin + u(rank)];
        let mut status = OK;
        let need = 2 * (lnz + m);
        if self.lmem() < need {
            self.addmem_l = need - self.lmem();
            status = REALLOCATE;
        }
        let need = unz + m + 1;
        if self.umem() < need {
            self.addmem_u = need - self.umem();
            status = REALLOCATE;
        }
        let need = trunc(unz as f64 + stretch * unz as f64 + (m * pad) as f64);
        if self.wmem() < need {
            self.addmem_w = need - self.wmem();
            status = REALLOCATE;
        }
        if status != OK {
            return status;
        }
        let mut lrank = rank;
        for i in 0..mu {
            if self.is[pinv + i] < 0 {
                self.is[pinv + i] = lrank;
                lrank += 1;
            }
            let at = u(self.is[pinv + i]);
            self.is[pivotrow + at] = i as Int;
        }
        lrank = rank;
        for j in 0..mu {
            if self.is[qinv + j] < 0 {
                self.is[qinv + j] = lrank;
                lrank += 1;
            }
            let at = u(self.is[qinv + j]);
            self.is[pivotcol + at] = j as Int;
        }
        for k in u(rank)..mu {
            self.col_pivot[u(self.is[pivotcol + k])] = 1.0;
        }
        let mut put = self.is[lbegin_p + u(rank)];
        for k in u(rank)..mu {
            self.li[u(put)] = -1;
            put += 1;
            self.is[lbegin_p + k + 1] = put;
        }
        for i in 0..mu {
            self.is[lbegin + i] = self.is[lbegin_p + u(self.is[pinv + i])];
        }
        self.is[iwork1..iwork1 + mu].fill(0);
        for get in 0..u(lnz + m) {
            let i = self.li[get];
            if i >= 0 {
                self.is[iwork1 + u(i)] += 1;
            }
        }
        let mut put = lnz + m;
        for k in 0..mu {
            let i = u(self.is[pivotrow + k]);
            self.is[ltbegin_p + k] = put;
            self.is[ltbegin + i] = put;
            put += self.is[iwork1 + i];
            self.li[u(put)] = -1;
            put += 1;
            self.is[iwork1 + i] = self.is[ltbegin_p + k];
        }
        for k in 0..mu {
            let ipivot = self.is[pivotrow + k];
            let mut get = self.is[lbegin_p + k];
            loop {
                let i = self.li[u(get)];
                if i < 0 {
                    break;
                }
                let put = self.is[iwork1 + u(i)];
                self.is[iwork1 + u(i)] += 1;
                self.li[u(put)] = ipivot;
                self.lx[u(put)] = self.lx[u(get)];
                get += 1;
            }
        }
        self.is[rbegin] = 2 * (lnz + m);
        let wmem = self.wmem();
        file_empty(&mut self.is, mu, wbegin, wend, wflink, wblink, wmem);
        self.is[iwork1..iwork1 + mu].fill(0);
        let mut put: Int = 0;
        if rank == m {
            for k in 0..mu {
                let jpivot = u(self.is[pivotcol + k]);
                self.is[wbegin + jpivot] = put;
                let mut nz = 0;
                for pos in self.is[ubegin + k]..self.is[ubegin + k + 1] {
                    let j = self.ui[u(pos)];
                    self.wi[u(put)] = j;
                    self.wx[u(put)] = self.ux[u(pos)];
                    put += 1;
                    self.is[iwork1 + u(j)] += 1;
                    nz += 1;
                }
                self.is[wend + jpivot] = put;
                put = trunc(put as f64 + (stretch * nz as f64 + pad as f64));
                list_move(&mut self.is, wflink, wblink, jpivot, 0, mu, None);
            }
        } else {
            unz = 0;
            for k in 0..u(rank) {
                let jpivot = u(self.is[pivotcol + k]);
                self.is[wbegin + jpivot] = put;
                let mut nz = 0;
                for pos in self.is[ubegin + k]..self.is[ubegin + k + 1] {
                    let j = self.ui[u(pos)];
                    if self.is[qinv + u(j)] < rank {
                        self.wi[u(put)] = j;
                        self.wx[u(put)] = self.ux[u(pos)];
                        put += 1;
                        self.is[iwork1 + u(j)] += 1;
                        nz += 1;
                    }
                }
                self.is[wend + jpivot] = put;
                put = trunc(put as f64 + (stretch * nz as f64 + pad as f64));
                list_move(&mut self.is, wflink, wblink, jpivot, 0, mu, None);
                unz += nz;
            }
            for k in u(rank)..mu {
                let jpivot = u(self.is[pivotcol + k]);
                self.is[wbegin + jpivot] = put;
                self.is[wend + jpivot] = put;
                put += pad;
                list_move(&mut self.is, wflink, wblink, jpivot, 0, mu, None);
            }
        }
        self.is[wbegin + mu] = put;
        self.ui[0] = -1;
        let mut put: Int = 1;
        for k in 0..mu {
            let j = u(self.is[pivotcol + k]);
            let i = u(self.is[pivotrow + k]);
            let nz = self.is[iwork1 + j];
            if nz == 0 {
                self.is[ubegin + i] = 0;
            } else {
                self.is[ubegin + i] = put;
                put += nz;
                self.ui[u(put)] = -1;
                put += 1;
            }
            self.is[iwork1 + j] = self.is[ubegin + i];
        }
        self.is[ubegin + mu] = put;
        for k in 0..mu {
            let jpivot = u(self.is[pivotcol + k]);
            let i = self.is[pivotrow + k];
            for pos in self.is[wbegin + jpivot]..self.is[wend + jpivot] {
                let j = u(self.wi[u(pos)]);
                let put = self.is[iwork1 + j];
                self.is[iwork1 + j] += 1;
                self.ui[u(put)] = i;
                self.ux[u(put)] = self.wx[u(pos)];
            }
        }
        for k in 0..mu {
            let i = self.is[pivotrow + k];
            let j = self.is[pivotcol + k];
            self.is[pmap + u(j)] = i;
            self.is[qmap + u(i)] = j;
        }
        let mut max_pivot: f64 = 0.0;
        let mut min_pivot = f64::INFINITY;
        for i in 0..mu {
            self.row_pivot[i] = self.col_pivot[u(self.is[qmap + i])];
            let pivot = self.row_pivot[i].abs();
            max_pivot = pivot.max(max_pivot);
            min_pivot = pivot.min(min_pivot);
        }
        self.is.copy_within(pivotrow..pivotrow + mu, p);
        self.min_pivot = min_pivot;
        self.max_pivot = max_pivot;
        self.pivotlen = m;
        self.lnz = lnz;
        self.unz = unz;
        self.rnz = 0;
        status
    }

    fn residual_test_(&mut self, bbegin: &[Int], bend: &[Int], bi: &[Int], bx: &[f64]) {
        let m = self.m;
        let mu = u(m);
        let rank = u(self.rank);
        let (p, pivotcol, pivotrow, lbegin_p, ltbegin_p, ubegin) = (self.p(), self.pivotcol(), self.pivotrow(), self.lbegin_p, self.ltbegin_p(), self.ubegin);
        let mut d;
        for k in 0..mu {
            d = 0.0;
            let mut pos = self.is[ltbegin_p + k];
            loop {
                let i = self.li[u(pos)];
                if i < 0 {
                    break;
                }
                d += self.work1[u(i)] * self.lx[u(pos)];
                pos += 1;
            }
            let ipivot = u(self.is[p + k]);
            self.work0[ipivot] = if d <= 0.0 { 1.0 } else { -1.0 };
            self.work1[ipivot] = self.work0[ipivot] - d;
        }
        for k in (0..mu).rev() {
            let ipivot = u(self.is[pivotrow + k]);
            self.work1[ipivot] /= self.row_pivot[ipivot];
            d = self.work1[ipivot];
            let mut pos = self.is[ubegin + ipivot];
            loop {
                let i = self.ui[u(pos)];
                if i < 0 {
                    break;
                }
                self.work1[u(i)] -= d * self.ux[u(pos)];
                pos += 1;
            }
        }
        for k in 0..rank {
            let ipivot = u(self.is[pivotrow + k]);
            let jpivot = u(self.is[pivotcol + k]);
            d = self.work1[ipivot];
            for pos in bbegin[jpivot]..bend[jpivot] {
                self.work0[u(bi[u(pos)])] -= d * bx[u(pos)];
            }
        }
        for k in rank..mu {
            let ipivot = u(self.is[pivotrow + k]);
            self.work0[ipivot] -= self.work1[ipivot];
        }
        let norm_ftran = onenorm(&self.work1);
        let norm_ftran_res = onenorm(&self.work0);
        for k in 0..mu {
            let ipivot = u(self.is[pivotrow + k]);
            d = 0.0;
            let mut pos = self.is[ubegin + ipivot];
            loop {
                let i = self.ui[u(pos)];
                if i < 0 {
                    break;
                }
                d += self.work1[u(i)] * self.ux[u(pos)];
                pos += 1;
            }
            self.work0[ipivot] = if d <= 0.0 { 1.0 } else { -1.0 };
            self.work1[ipivot] = (self.work0[ipivot] - d) / self.row_pivot[ipivot];
        }
        for k in (0..mu).rev() {
            d = 0.0;
            let mut pos = self.is[lbegin_p + k];
            loop {
                let i = self.li[u(pos)];
                if i < 0 {
                    break;
                }
                d += self.work1[u(i)] * self.lx[u(pos)];
                pos += 1;
            }
            self.work1[u(self.is[p + k])] -= d;
        }
        for k in 0..rank {
            let ipivot = u(self.is[pivotrow + k]);
            let jpivot = u(self.is[pivotcol + k]);
            d = 0.0;
            for pos in bbegin[jpivot]..bend[jpivot] {
                d += self.work1[u(bi[u(pos)])] * bx[u(pos)];
            }
            self.work0[ipivot] -= d;
        }
        for k in rank..mu {
            let ipivot = u(self.is[pivotrow + k]);
            self.work0[ipivot] -= self.work1[ipivot];
        }
        let norm_btran = onenorm(&self.work1);
        let norm_btran_res = onenorm(&self.work0);
        self.matrix_norm(bbegin, bend, bx, bi);
        self.residual_test = (norm_ftran_res / (m as f64 + self.onenorm * norm_ftran)).max(norm_btran_res / (m as f64 + self.infnorm * norm_btran));
        self.work0.fill(0.0);
    }

    fn matrix_norm(&mut self, bbegin: &[Int], bend: &[Int], bx: &[f64], bi: &[Int]) {
        let mu = u(self.m);
        let rank = u(self.rank);
        let (pivotcol, pivotrow) = (self.pivotcol(), self.pivotrow());
        self.work1.fill(0.0);
        let mut onenorm: f64 = 0.0;
        let mut infnorm: f64 = 0.0;
        for k in 0..rank {
            let jpivot = u(self.is[pivotcol + k]);
            let mut colsum = 0.0;
            for pos in bbegin[jpivot]..bend[jpivot] {
                colsum += bx[u(pos)].abs();
                self.work1[u(bi[u(pos)])] += bx[u(pos)].abs();
            }
            onenorm = onenorm.max(colsum);
        }
        for k in rank..mu {
            let ipivot = u(self.is[pivotrow + k]);
            self.work1[ipivot] += 1.0;
            onenorm = onenorm.max(1.0);
        }
        for i in 0..mu {
            infnorm = infnorm.max(self.work1[i]);
        }
        self.onenorm = onenorm;
        self.infnorm = infnorm;
    }

    // ---- basiclu_get_factors ---------------------------------------------

    /// `basiclu_get_factors`: the row and column pivot orders, and L and
    /// U column-wise with their unit and pivot diagonals first.
    #[allow(clippy::type_complexity)]
    pub fn get_factors(&mut self, want_l: bool, want_u: bool) -> (Vec<Int>, Vec<Int>, Option<(Vec<Int>, Vec<Int>, Vec<f64>)>, Option<(Vec<Int>, Vec<Int>, Vec<f64>)>) {
        self.load();
        let m = u(self.m);
        let (pivotrow, pivotcol) = (self.pivotrow(), self.pivotcol());
        let rowperm = self.is[pivotrow..pivotrow + m].to_vec();
        let colperm = self.is[pivotcol..pivotcol + m].to_vec();
        let l = want_l.then(|| {
            let (lbegin_p, ltbegin_p, p, colptr) = (self.lbegin_p, self.ltbegin_p(), self.p(), self.iwork1());
            let nnz = u(self.lnz) + m;
            let mut lcolptr = vec![0; m + 1];
            let mut lrowidx = vec![0; nnz];
            let mut lvalue = vec![0.0; nnz];
            let mut put = 0;
            for k in 0..m {
                lcolptr[k] = put;
                lrowidx[u(put)] = k as Int;
                lvalue[u(put)] = 1.0;
                put += 1;
                let at = u(self.is[p + k]);
                self.is[colptr + at] = put;
                put += self.is[lbegin_p + k + 1] - self.is[lbegin_p + k] - 1;
            }
            lcolptr[m] = put;
            for k in 0..m {
                let mut pos = self.is[ltbegin_p + k];
                loop {
                    let i = self.li[u(pos)];
                    if i < 0 {
                        break;
                    }
                    let put = self.is[colptr + u(i)];
                    self.is[colptr + u(i)] += 1;
                    lrowidx[u(put)] = k as Int;
                    lvalue[u(put)] = self.lx[u(pos)];
                    pos += 1;
                }
            }
            (lcolptr, lrowidx, lvalue)
        });
        let uf = want_u.then(|| {
            let (wbegin, wend, colptr) = (self.wbegin, self.wend, self.iwork1());
            let nnz = u(self.unz) + m;
            let mut ucolptr = vec![0; m + 1];
            let mut urowidx = vec![0; nnz];
            let mut uvalue = vec![0.0; nnz];
            self.is[colptr..colptr + m].fill(0);
            for j in 0..m {
                for pos in self.is[wbegin + j]..self.is[wend + j] {
                    self.is[colptr + u(self.wi[u(pos)])] += 1;
                }
            }
            let mut put = 0;
            for k in 0..m {
                let j = u(self.is[pivotcol + k]);
                ucolptr[k] = put;
                put += self.is[colptr + j];
                self.is[colptr + j] = ucolptr[k];
                urowidx[u(put)] = k as Int;
                uvalue[u(put)] = self.col_pivot[j];
                put += 1;
            }
            ucolptr[m] = put;
            for k in 0..m {
                let j = u(self.is[pivotcol + k]);
                for pos in self.is[wbegin + j]..self.is[wend + j] {
                    let row = u(self.wi[u(pos)]);
                    let put = self.is[colptr + row];
                    self.is[colptr + row] += 1;
                    urowidx[u(put)] = k as Int;
                    uvalue[u(put)] = self.wx[u(pos)];
                }
            }
            (ucolptr, urowidx, uvalue)
        });
        (rowperm, colperm, l, uf)
    }

    // ---- Solves ----------------------------------------------------------

    fn garbage_perm(&mut self) {
        let m = self.m;
        let pivotlen = self.pivotlen;
        let (pivotcol, pivotrow, marked) = (self.pivotcol(), self.pivotrow(), self.marked());
        if pivotlen > m {
            self.marker += 1;
            let marker = self.marker;
            let mut put = pivotlen;
            let mut get = pivotlen - 1;
            while get >= 0 {
                let j = self.is[pivotcol + u(get)];
                if self.is[marked + u(j)] != marker {
                    self.is[marked + u(j)] = marker;
                    put -= 1;
                    self.is[pivotcol + u(put)] = j;
                    self.is[pivotrow + u(put)] = self.is[pivotrow + u(get)];
                }
                get -= 1;
            }
            let mu = u(m);
            self.is.copy_within(pivotcol + u(put)..pivotcol + u(put) + mu, pivotcol);
            self.is.copy_within(pivotrow + u(put)..pivotrow + u(put) + mu, pivotrow);
            self.pivotlen = m;
        }
    }

    /// `basiclu_solve_dense`.
    pub fn solve_dense(&mut self, rhs: &[f64], lhs: &mut [f64], trans: char) -> Int {
        self.load();
        if self.nupdate < 0 {
            return ERROR_INVALID_CALL;
        }
        let mu = u(self.m);
        self.garbage_perm();
        let nforrest = u(self.nforrest);
        let (p, eta_row, pivotcol, pivotrow, lbegin_p, ltbegin_p, ubegin, rbegin, wbegin, wend) = (
            self.p(),
            self.eta_row(),
            self.pivotcol(),
            self.pivotrow(),
            self.lbegin_p,
            self.ltbegin_p(),
            self.ubegin,
            self.rbegin(),
            self.wbegin,
            self.wend,
        );
        let is = &self.is;
        let work1 = &mut self.work1;
        if trans == 't' || trans == 'T' {
            work1.copy_from_slice(&rhs[..mu]);
            for k in 0..mu {
                let jpivot = u(is[pivotcol + k]);
                let ipivot = u(is[pivotrow + k]);
                let x = work1[jpivot] / self.col_pivot[jpivot];
                for pos in is[wbegin + jpivot]..is[wend + jpivot] {
                    work1[u(self.wi[u(pos)])] -= x * self.wx[u(pos)];
                }
                lhs[ipivot] = x;
            }
            for t in (0..nforrest).rev() {
                let ipivot = u(is[eta_row + t]);
                let x = lhs[ipivot];
                for pos in is[rbegin + t]..is[rbegin + t + 1] {
                    let i = u(self.li[u(pos)]);
                    lhs[i] -= x * self.lx[u(pos)];
                }
            }
            for k in (0..mu).rev() {
                let mut x = 0.0;
                let mut pos = is[lbegin_p + k];
                loop {
                    let i = self.li[u(pos)];
                    if i < 0 {
                        break;
                    }
                    x += lhs[u(i)] * self.lx[u(pos)];
                    pos += 1;
                }
                lhs[u(is[p + k])] -= x;
            }
        } else {
            work1.copy_from_slice(&rhs[..mu]);
            for k in 0..mu {
                let mut x = 0.0;
                let mut pos = is[ltbegin_p + k];
                loop {
                    let i = self.li[u(pos)];
                    if i < 0 {
                        break;
                    }
                    x += work1[u(i)] * self.lx[u(pos)];
                    pos += 1;
                }
                work1[u(is[p + k])] -= x;
            }
            let mut pos = is[rbegin];
            for t in 0..nforrest {
                let ipivot = u(is[eta_row + t]);
                let mut x = 0.0;
                while pos < is[rbegin + t + 1] {
                    x += work1[u(self.li[u(pos)])] * self.lx[u(pos)];
                    pos += 1;
                }
                work1[ipivot] -= x;
            }
            for k in (0..mu).rev() {
                let jpivot = u(is[pivotcol + k]);
                let ipivot = u(is[pivotrow + k]);
                let x = work1[ipivot] / self.row_pivot[ipivot];
                let mut pos = is[ubegin + ipivot];
                loop {
                    let i = self.ui[u(pos)];
                    if i < 0 {
                        break;
                    }
                    work1[u(i)] -= x * self.ux[u(pos)];
                    pos += 1;
                }
                lhs[jpivot] = x;
            }
        }
        OK
    }

    /// `basiclu_solve_for_update`: FTRAN of a column entering the basis
    /// (`trans` 'N', `rhs` its entries) or BTRAN of the unit row of the
    /// column leaving (`trans` 'T', `irhs[0]` its index), each optionally
    /// returning the solution sparse in `lhs`.
    #[allow(clippy::too_many_arguments)]
    pub fn solve_for_update(&mut self, nrhs: Int, irhs: &[Int], xrhs: &[f64], want: Option<(&mut Int, &mut [Int], &mut [f64])>, trans: char) -> Int {
        self.load();
        if self.nupdate < 0 {
            return ERROR_INVALID_CALL;
        }
        if self.nforrest == self.m {
            return ERROR_MAXIMUM_UPDATES;
        }
        if trans == 't' || trans == 'T' {
            if !(irhs[0] >= 0 && irhs[0] < self.m) {
                return ERROR_INVALID_ARGUMENT;
            }
        } else {
            let mut ok = nrhs >= 0 && nrhs <= self.m;
            for n in 0..u(nrhs.max(0)) {
                ok = ok && irhs[n] >= 0 && irhs[n] < self.m;
            }
            if !ok {
                return ERROR_INVALID_ARGUMENT;
            }
        }
        self.solve_for_update_(nrhs, irhs, xrhs, want, trans)
    }

    fn solve_for_update_(&mut self, nrhs: Int, irhs: &[Int], xrhs: &[f64], want: Option<(&mut Int, &mut [Int], &mut [f64])>, trans: char) -> Int {
        let m = self.m;
        let mu = u(m);
        let nforrest = u(self.nforrest);
        let pivotlen = u(self.pivotlen);
        let nz_sparse = trunc(self.sparse_thres * m as f64);
        let droptol = self.droptol;
        let (p, pmap, qmap, eta_row, pivotcol, pivotrow, lbegin, ltbegin, ltbegin_p, ubegin, rbegin, wbegin, wend, marked) = (
            self.p(),
            self.pmap(),
            self.qmap(),
            self.eta_row(),
            self.pivotcol(),
            self.pivotrow(),
            self.lbegin(),
            self.ltbegin(),
            self.ltbegin_p(),
            self.ubegin,
            self.rbegin(),
            self.wbegin,
            self.wend,
            self.marked(),
        );
        let pattern_symb = self.iwork1();
        let pattern = pattern_symb + mu;
        let mut lflops = 0;
        let mut uflops = 0;
        let mut rflops = 0;
        if trans == 't' || trans == 'T' {
            let jpivot = u(irhs[0]);
            let mut ipivot = self.is[pmap + jpivot];
            let jbegin = self.is[wbegin + jpivot];
            let jend = self.is[wend + jpivot];
            self.marker += 1;
            let big_m = self.marker;
            let seeds: Vec<Int> = self.wi[u(jbegin)..u(jend)].to_vec();
            let top = solve_symbolic(m, &mut self.is, wbegin, Some(wend), &self.wi, &seeds, pattern_symb, &mut self.pstack, marked, big_m);
            let nz_symb = m - top;
            let room = self.lmem() - self.is[rbegin + nforrest];
            if room < nz_symb {
                self.addmem_l = nz_symb - room;
                return REALLOCATE;
            }
            for pos in jbegin..jend {
                self.work0[u(self.wi[u(pos)])] = self.wx[u(pos)];
            }
            solve_triangular(
                pattern_symb + u(top),
                pattern_symb + mu,
                &mut self.is,
                wbegin,
                Some(wend),
                &self.wi,
                &self.wx,
                Some(&self.col_pivot),
                0.0,
                &mut self.work0,
                PatternOut::Istore(pattern),
                &mut uflops,
            );
            let mut put = self.is[rbegin + nforrest];
            for t in u(top)..mu {
                let j = u(self.is[pattern_symb + t]);
                let i = self.is[pmap + j];
                self.li[u(put)] = i;
                self.lx[u(put)] = self.work0[j];
                put += 1;
                self.work0[j] = 0.0;
            }
            self.is[rbegin + nforrest + 1] = put;
            self.is[eta_row + nforrest] = ipivot;
            self.btran_for_update = jpivot as Int;
            if let Some((p_nlhs, ilhs, xlhs)) = want {
                self.marker += 1;
                let big_m = self.marker;
                self.is[pattern] = ipivot;
                self.is[marked + u(ipivot)] = big_m;
                let pivot = self.col_pivot[jpivot];
                xlhs[u(ipivot)] = 1.0 / pivot;
                let xdrop = droptol * pivot.abs();
                let mut nz: Int = 1;
                for pos in self.is[rbegin + nforrest]..self.is[rbegin + nforrest + 1] {
                    if self.lx[u(pos)].abs() > xdrop {
                        let i = self.li[u(pos)];
                        self.is[pattern + u(nz)] = i;
                        nz += 1;
                        self.is[marked + u(i)] = big_m;
                        xlhs[u(i)] = -self.lx[u(pos)] / pivot;
                    }
                }
                for t in (0..nforrest).rev() {
                    ipivot = self.is[eta_row + t];
                    if xlhs[u(ipivot)] != 0.0 {
                        let x = xlhs[u(ipivot)];
                        for pos in self.is[rbegin + t]..self.is[rbegin + t + 1] {
                            let i = self.li[u(pos)];
                            if self.is[marked + u(i)] != big_m {
                                self.is[marked + u(i)] = big_m;
                                self.is[pattern + u(nz)] = i;
                                nz += 1;
                            }
                            xlhs[u(i)] -= x * self.lx[u(pos)];
                            rflops += 1;
                        }
                    }
                }
                if nz <= nz_sparse {
                    self.marker += 1;
                    let big_m = self.marker;
                    let seeds: Vec<Int> = self.is[pattern..pattern + u(nz)].to_vec();
                    let top = solve_symbolic(m, &mut self.is, ltbegin, None, &self.li, &seeds, pattern_symb, &mut self.pstack, marked, big_m);
                    let nz = solve_triangular(
                        pattern_symb + u(top),
                        pattern_symb + mu,
                        &mut self.is,
                        ltbegin,
                        None,
                        &self.li,
                        &self.lx,
                        None,
                        droptol,
                        xlhs,
                        PatternOut::Slice(ilhs),
                        &mut lflops,
                    );
                    *p_nlhs = nz;
                } else {
                    let mut nz = 0;
                    for k in (0..mu).rev() {
                        let ipivot = u(self.is[p + k]);
                        if xlhs[ipivot] != 0.0 {
                            let x = xlhs[ipivot];
                            let mut pos = self.is[ltbegin_p + k];
                            loop {
                                let i = self.li[u(pos)];
                                if i < 0 {
                                    break;
                                }
                                xlhs[u(i)] -= x * self.lx[u(pos)];
                                lflops += 1;
                                pos += 1;
                            }
                            if x.abs() > droptol {
                                ilhs[u(nz)] = ipivot as Int;
                                nz += 1;
                            } else {
                                xlhs[ipivot] = 0.0;
                            }
                        }
                    }
                    *p_nlhs = nz;
                }
            }
        } else {
            self.marker += 1;
            let big_m = self.marker;
            let top = solve_symbolic(m, &mut self.is, lbegin, None, &self.li, &irhs[..u(nrhs)], pattern_symb, &mut self.pstack, marked, big_m);
            let nz_symb = m - top;
            for n in 0..u(nrhs) {
                self.work0[u(irhs[n])] = xrhs[n];
            }
            let mut nz = solve_triangular(
                pattern_symb + u(top),
                pattern_symb + mu,
                &mut self.is,
                lbegin,
                None,
                &self.li,
                &self.lx,
                None,
                droptol,
                &mut self.work0,
                PatternOut::Istore(pattern),
                &mut lflops,
            );
            if nz < nz_symb {
                let mut t = u(top);
                let mut n = 0;
                while n < nz {
                    let i = self.is[pattern_symb + t];
                    if i == self.is[pattern + u(n)] {
                        n += 1;
                    } else {
                        self.is[marked + u(i)] -= 1;
                    }
                    t += 1;
                }
                while t < mu {
                    let at = u(self.is[pattern_symb + t]);
                    self.is[marked + at] -= 1;
                    t += 1;
                }
            }
            let mut pos = self.is[rbegin];
            for t in 0..nforrest {
                let ipivot = self.is[eta_row + t];
                let mut x = 0.0;
                while pos < self.is[rbegin + t + 1] {
                    x += self.work0[u(self.li[u(pos)])] * self.lx[u(pos)];
                    pos += 1;
                }
                self.work0[u(ipivot)] -= x;
                if x != 0.0 && self.is[marked + u(ipivot)] != big_m {
                    self.is[marked + u(ipivot)] = big_m;
                    self.is[pattern + u(nz)] = ipivot;
                    nz += 1;
                }
            }
            rflops += self.is[rbegin + nforrest] - self.is[rbegin];
            let room = self.umem() - self.is[ubegin + mu];
            let need = nz + 1;
            if room < need {
                for n in 0..u(nz) {
                    self.work0[u(self.is[pattern + n])] = 0.0;
                }
                self.addmem_u = need - room;
                return REALLOCATE;
            }
            let want_solution = want.is_some();
            let mut put = self.is[ubegin + mu];
            for n in 0..u(nz) {
                let i = u(self.is[pattern + n]);
                self.ui[u(put)] = i as Int;
                self.ux[u(put)] = self.work0[i];
                put += 1;
                if !want_solution {
                    self.work0[i] = 0.0;
                }
            }
            self.ui[u(put)] = -1;
            self.ftran_for_update = 0;
            if let Some((p_nlhs, ilhs, xlhs)) = want {
                if nz <= nz_sparse {
                    self.marker += 1;
                    let big_m = self.marker;
                    let seeds: Vec<Int> = self.is[pattern..pattern + u(nz)].to_vec();
                    let top = solve_symbolic(m, &mut self.is, ubegin, None, &self.ui, &seeds, pattern_symb, &mut self.pstack, marked, big_m);
                    let nz = solve_triangular(
                        pattern_symb + u(top),
                        pattern_symb + mu,
                        &mut self.is,
                        ubegin,
                        None,
                        &self.ui,
                        &self.ux,
                        Some(&self.row_pivot),
                        droptol,
                        &mut self.work0,
                        PatternOut::Slice(ilhs),
                        &mut uflops,
                    );
                    for n in 0..u(nz) {
                        let i = u(ilhs[n]);
                        let j = self.is[qmap + i];
                        ilhs[n] = j;
                        xlhs[u(j)] = self.work0[i];
                        self.work0[i] = 0.0;
                    }
                    *p_nlhs = nz;
                } else {
                    let mut nz = 0;
                    for k in (0..pivotlen).rev() {
                        let ipivot = u(self.is[pivotrow + k]);
                        let jpivot = self.is[pivotcol + k];
                        if self.work0[ipivot] != 0.0 {
                            let x = self.work0[ipivot] / self.row_pivot[ipivot];
                            self.work0[ipivot] = 0.0;
                            let mut pos = self.is[ubegin + ipivot];
                            loop {
                                let i = self.ui[u(pos)];
                                if i < 0 {
                                    break;
                                }
                                self.work0[u(i)] -= x * self.ux[u(pos)];
                                uflops += 1;
                                pos += 1;
                            }
                            if x.abs() > droptol {
                                ilhs[u(nz)] = jpivot;
                                nz += 1;
                                xlhs[u(jpivot)] = x;
                            }
                        }
                    }
                    *p_nlhs = nz;
                }
            }
        }
        self.lflops += lflops;
        self.uflops += uflops;
        self.rflops += rflops;
        self.update_cost_numer += rflops as f64;
        OK
    }

    // ---- basiclu_update ----------------------------------------------------

    /// `basiclu_update` with `xtbl` the pivot element of the simplex
    /// tableau, after the FTRAN and BTRAN for the update.
    pub fn update(&mut self, xtbl: f64) -> Int {
        self.load();
        if self.nupdate < 0 || self.ftran_for_update < 0 || self.btran_for_update < 0 {
            return ERROR_INVALID_CALL;
        }
        self.update_(xtbl)
    }

    fn update_(&mut self, xtbl: f64) -> Int {
        let m = self.m;
        let mu = u(m);
        let nforrest = u(self.nforrest);
        let mut unz = self.unz;
        let pad = self.pad;
        let stretch = self.stretch;
        let (pmap, qmap, pivotcol, pivotrow, ubegin, rbegin, wbegin, wend, wflink, wblink, marked) = (
            self.pmap(),
            self.qmap(),
            self.pivotcol(),
            self.pivotrow(),
            self.ubegin,
            self.rbegin(),
            self.wbegin,
            self.wend,
            self.wflink,
            self.wblink,
            self.marked(),
        );
        let iwork1 = self.iwork1();
        let iwork2 = iwork1 + mu;
        let jpivot = self.btran_for_update;
        let ipivot = self.is[pmap + u(jpivot)];
        let oldpiv = self.col_pivot[u(jpivot)];

        // Move the spike to the end of U, its diagonal entry last.
        let mut spike_diag = 0.0;
        let mut have_diag = false;
        let mut put = self.is[ubegin + mu];
        let mut pos = put;
        loop {
            let i = self.ui[u(pos)];
            if i < 0 {
                break;
            }
            if i != ipivot {
                self.ui[u(put)] = i;
                self.ux[u(put)] = self.ux[u(pos)];
                put += 1;
            } else {
                spike_diag = self.ux[u(pos)];
                have_diag = true;
            }
            pos += 1;
        }
        if have_diag {
            self.ui[u(put)] = ipivot;
            self.ux[u(put)] = spike_diag;
        }
        let nz_spike = put - self.is[ubegin + mu];
        let nz_roweta = self.is[rbegin + nforrest + 1] - self.is[rbegin + nforrest];

        self.marker += 1;
        let big_m = self.marker;
        for pos in self.is[rbegin + nforrest]..self.is[rbegin + nforrest + 1] {
            let i = u(self.li[u(pos)]);
            self.is[marked + i] = big_m;
            self.work1[i] = self.lx[u(pos)];
        }
        let mut newpiv = spike_diag;
        let mut intersect = 0;
        for pos in self.is[ubegin + mu]..self.is[ubegin + mu] + nz_spike {
            let i = u(self.ui[u(pos)]);
            if self.is[marked + i] == big_m {
                newpiv -= self.ux[u(pos)] * self.work1[i];
                intersect += 1;
            }
        }
        if newpiv == 0.0 || newpiv.abs() < self.abstol {
            return ERROR_SINGULAR_UPDATE;
        }
        let piverr = (newpiv - xtbl * oldpiv).abs();

        let mut grow: Int = 0;
        for pos in self.is[ubegin + mu]..self.is[ubegin + mu] + nz_spike {
            let i = u(self.ui[u(pos)]);
            let j = u(self.is[qmap + i]);
            let jnext = u(self.is[wflink + j]);
            if self.is[wend + j] == self.is[wbegin + jnext] {
                let nz = self.is[wend + j] - self.is[wbegin + j];
                grow += nz + 1;
                grow = trunc(grow as f64 + (stretch * (nz + 1) as f64 + pad as f64));
            }
        }
        let room = self.is[wend + mu] - self.is[wbegin + mu];
        if grow > room {
            self.addmem_w = grow - room;
            return REALLOCATE;
        }

        // Replace the pivot column in U row-wise.
        let mut nz = 0;
        let mut pos = self.is[ubegin + u(ipivot)];
        loop {
            let i = self.ui[u(pos)];
            if i < 0 {
                break;
            }
            let j = u(self.is[qmap + u(i)]);
            let end = self.is[wend + j];
            self.is[wend + j] -= 1;
            let where_ = find(jpivot, &self.wi, self.is[wbegin + j], end);
            self.wi[u(where_)] = self.wi[u(end) - 1];
            self.wx[u(where_)] = self.wx[u(end) - 1];
            nz += 1;
            pos += 1;
        }
        unz -= nz;
        let mut pos = self.is[ubegin + u(ipivot)];
        while self.ui[u(pos)] >= 0 {
            self.ui[u(pos)] = GAP;
            pos += 1;
        }
        self.is[ubegin + u(ipivot)] = self.is[ubegin + mu];
        self.is[ubegin + mu] += nz_spike;
        let at = u(self.is[ubegin + mu]);
        self.ui[at] = GAP;
        self.is[ubegin + mu] += 1;
        let mut pos = self.is[ubegin + u(ipivot)];
        loop {
            let i = self.ui[u(pos)];
            if i < 0 {
                break;
            }
            let j = u(self.is[qmap + u(i)]);
            let jnext = u(self.is[wflink + j]);
            if self.is[wend + j] == self.is[wbegin + jnext] {
                let nz = self.is[wend + j] - self.is[wbegin + j];
                let room = trunc(1.0 + stretch * (nz + 1) as f64 + pad as f64);
                file_reappend(&mut self.is, j, mu, wbegin, wend, wflink, wblink, &mut self.wi, &mut self.wx, room);
            }
            let end = self.is[wend + j];
            self.is[wend + j] += 1;
            self.wi[u(end)] = jpivot;
            self.wx[u(end)] = self.ux[u(pos)];
            pos += 1;
        }
        unz += nz_spike;
        self.col_pivot[u(jpivot)] = spike_diag;
        self.row_pivot[u(ipivot)] = spike_diag;

        // Test whether the permuted U stays triangular.
        let mut istriangular;
        let mut reach: Option<(Vec<Int>, Vec<Int>)> = None;
        if have_diag {
            istriangular = intersect == 0;
            if istriangular {
                self.min_pivot = self.min_pivot.min(newpiv.abs());
                self.max_pivot = self.max_pivot.max(newpiv.abs());
                let nreach = u(nz_roweta) + 1;
                let mut row_reach = Vec::with_capacity(nreach);
                let mut col_reach = Vec::with_capacity(nreach);
                row_reach.push(ipivot);
                col_reach.push(jpivot);
                let mut pos = self.is[rbegin + nforrest];
                for _ in 1..nreach {
                    let i = self.li[u(pos)];
                    pos += 1;
                    row_reach.push(i);
                    col_reach.push(self.is[qmap + u(i)]);
                }
                self.nsymperm_total += 1;
                reach = Some((row_reach, col_reach));
            }
        } else {
            let path = iwork1;
            let top = self.bfs_path(u(jpivot), path, iwork2);
            istriangular = true;
            let mut rtop = m;
            self.marker += 1;
            let big_m = self.marker;
            let reach_at = iwork2;
            let mut t = top;
            while t < m - 1 && istriangular {
                let j = u(self.is[path + u(t)]);
                let jnext = self.is[path + u(t) + 1];
                let where_ = find(jnext, &self.wi, self.is[wbegin + j], self.is[wend + j]);
                self.wi[u(where_)] = j as Int;
                rtop = dfs(j as Int, &mut self.is, wbegin, Some(wend), &self.wi, rtop, reach_at, &mut self.pstack, marked, big_m);
                self.is[reach_at + u(rtop)] = jnext;
                self.wi[u(where_)] = jnext;
                istriangular = self.is[marked + u(jnext)] != big_m;
                t += 1;
            }
            if istriangular {
                let j = self.is[path + mu - 1];
                rtop = dfs(j, &mut self.is, wbegin, Some(wend), &self.wi, rtop, reach_at, &mut self.pstack, marked, big_m);
                self.is[reach_at + u(rtop)] = jpivot;
                self.is[marked + u(j)] -= 1;
                let mut pos = self.is[ubegin + u(ipivot)];
                loop {
                    let i = self.ui[u(pos)];
                    if i < 0 {
                        break;
                    }
                    if self.is[marked + u(self.is[qmap + u(i)])] == big_m {
                        istriangular = false;
                    }
                    pos += 1;
                }
                self.is[marked + u(j)] += 1;
            }
            if istriangular {
                let nswap = m - top - 1;
                let jlist: Vec<Int> = self.is[path + u(top)..path + u(top) + u(nswap) + 1].to_vec();
                self.permute(&jlist, nswap);
                unz -= 1;
                let col_reach: Vec<Int> = self.is[reach_at + u(rtop)..reach_at + mu].to_vec();
                let row_reach: Vec<Int> = col_reach.iter().map(|&j| self.is[pmap + u(j)]).collect();
                reach = Some((row_reach, col_reach));
            }
        }

        if !istriangular {
            // Forrest-Tomlin update: a row eta.
            let mut pos = self.is[wbegin + u(jpivot)];
            while pos < self.is[wend + u(jpivot)] {
                let j = u(self.wi[u(pos)]);
                let mut where_: Int = -1;
                let mut end = self.is[ubegin + u(self.is[pmap + j])];
                loop {
                    let i = self.ui[u(end)];
                    if i < 0 {
                        break;
                    }
                    if i == ipivot {
                        where_ = end;
                    }
                    end += 1;
                }
                self.ui[u(where_)] = self.ui[u(end) - 1];
                self.ux[u(where_)] = self.ux[u(end) - 1];
                self.ui[u(end) - 1] = -1;
                unz -= 1;
                pos += 1;
            }
            self.is[wend + u(jpivot)] = self.is[wbegin + u(jpivot)];
            self.col_pivot[u(jpivot)] = newpiv;
            self.row_pivot[u(ipivot)] = newpiv;
            self.min_pivot = self.min_pivot.min(newpiv.abs());
            self.max_pivot = self.max_pivot.max(newpiv.abs());
            let mut nz = 0;
            let mut put = self.is[rbegin + nforrest];
            let mut max_eta: f64 = 0.0;
            #[allow(clippy::mut_range_bound, reason = "the range starts at put, read once as C's loop does")]
            for pos in put..self.is[rbegin + nforrest + 1] {
                if self.lx[u(pos)] != 0.0 {
                    max_eta = max_eta.max(self.lx[u(pos)].abs());
                    self.li[u(put)] = self.li[u(pos)];
                    self.lx[u(put)] = self.lx[u(pos)];
                    put += 1;
                    nz += 1;
                }
            }
            self.is[rbegin + nforrest + 1] = put;
            self.rnz += nz;
            self.max_eta = self.max_eta.max(max_eta);
            reach = Some((vec![ipivot], vec![jpivot]));
            self.nforrest += 1;
            self.nforrest_total += 1;
        }
        let (row_reach, col_reach) = reach.expect("the update has its reach");
        let nreach = row_reach.len() as Int;
        if self.pivotlen + nreach > 2 * m {
            self.garbage_perm();
        }
        let mut put = u(self.pivotlen);
        for &row in &row_reach {
            self.is[pivotrow + put] = row;
            put += 1;
        }
        let mut put = u(self.pivotlen);
        for &col in &col_reach {
            self.is[pivotcol + put] = col;
            put += 1;
        }
        self.pivotlen += nreach;

        let used = self.is[ubegin + mu];
        if (used - unz - m) as f64 > self.compress_thres * used as f64 {
            compress_packed(mu, &mut self.is, ubegin, &mut self.ui, &mut self.ux);
        }
        let used = self.is[wbegin + mu];
        let need = trunc(unz as f64 + stretch * unz as f64 + (m * pad) as f64);
        if (used - need) as f64 > self.compress_thres * used as f64 {
            file_compress(&mut self.is, mu, wbegin, wend, wflink, &mut self.wi, &mut self.wx, stretch, pad);
        }
        self.pivot_error = piverr / (1.0 + newpiv.abs());
        self.unz = unz;
        self.btran_for_update = -1;
        self.ftran_for_update = -1;
        self.update_cost_numer += nz_roweta as f64;
        self.nupdate += 1;
        self.nupdate_total += 1;
        OK
    }

    /// `bfs_path`: a path from `j0` back to itself through the column
    /// file, written to `jlist` (an istore offset) from its end.
    fn bfs_path(&mut self, j0: usize, jlist: usize, queue: usize) -> Int {
        let m = self.m;
        let (wbegin, wend, marked) = (self.wbegin, self.wend, self.marked());
        let mut tail = 1usize;
        let mut top = m;
        let mut found = false;
        self.is[queue] = j0 as Int;
        let mut front = 0;
        let mut j = 0;
        while front < tail && !found {
            j = self.is[queue + front];
            for pos in self.is[wbegin + u(j)]..self.is[wend + u(j)] {
                let k = self.wi[u(pos)];
                if k == j0 as Int {
                    found = true;
                    break;
                }
                if self.is[marked + u(k)] >= 0 {
                    self.is[marked + u(k)] = flip(j);
                    self.is[queue + tail] = k;
                    tail += 1;
                }
            }
            front += 1;
        }
        if found {
            while j != j0 as Int {
                top -= 1;
                self.is[jlist + u(top)] = j;
                j = flip(self.is[marked + u(j)]);
            }
            top -= 1;
            self.is[jlist + u(top)] = j0 as Int;
        }
        for pos in 0..tail {
            let at = u(self.is[queue + pos]);
            self.is[marked + at] = 0;
        }
        top
    }

    fn permute(&mut self, jlist: &[Int], nswap: Int) {
        let (pmap, qmap, ubegin, wbegin, wend, wflink, wblink) = (self.pmap(), self.qmap(), self.ubegin, self.wbegin, self.wend, self.wflink, self.wblink);
        let nswap = u(nswap);
        let j0 = u(jlist[0]);
        let jn = u(jlist[nswap]);
        let i0 = u(self.is[pmap + j0]);
        let in_ = u(self.is[pmap + jn]);
        let begin = self.is[wbegin + jn];
        let end = self.is[wend + jn];
        let piv = self.col_pivot[jn];
        for n in (1..=nswap).rev() {
            let j = u(jlist[n]);
            let jprev = u(jlist[n - 1]);
            self.is[wbegin + j] = self.is[wbegin + jprev];
            self.is[wend + j] = self.is[wend + jprev];
            list_swap(&mut self.is, wflink, wblink, j, jprev);
            let where_ = u(find(j as Int, &self.wi, self.is[wbegin + j], self.is[wend + j]));
            if n > 1 {
                self.wi[where_] = jprev as Int;
                self.col_pivot[j] = self.wx[where_];
                self.wx[where_] = self.col_pivot[jprev];
            } else {
                self.col_pivot[j] = self.wx[where_];
                self.is[wend + j] -= 1;
                let last = u(self.is[wend + j]);
                self.wi[where_] = self.wi[last];
                self.wx[where_] = self.wx[last];
            }
            self.min_pivot = self.min_pivot.min(self.col_pivot[j].abs());
            self.max_pivot = self.max_pivot.max(self.col_pivot[j].abs());
        }
        self.is[wbegin + j0] = begin;
        self.is[wend + j0] = end;
        let where_ = u(find(j0 as Int, &self.wi, self.is[wbegin + j0], self.is[wend + j0]));
        self.wi[where_] = jn as Int;
        self.col_pivot[j0] = self.wx[where_];
        self.wx[where_] = piv;
        self.min_pivot = self.min_pivot.min(self.col_pivot[j0].abs());
        self.max_pivot = self.max_pivot.max(self.col_pivot[j0].abs());
        let begin = self.is[ubegin + i0];
        for n in 0..nswap {
            let i = u(self.is[pmap + u(jlist[n])]);
            let inext = u(self.is[pmap + u(jlist[n + 1])]);
            self.is[ubegin + i] = self.is[ubegin + inext];
            let where_ = u(find(i as Int, &self.ui, self.is[ubegin + i], -1));
            self.ui[where_] = inext as Int;
            self.row_pivot[i] = self.ux[where_];
            self.ux[where_] = self.row_pivot[inext];
        }
        self.is[ubegin + in_] = begin;
        let where_ = u(find(in_ as Int, &self.ui, self.is[ubegin + in_], -1));
        self.row_pivot[in_] = self.ux[where_];
        let mut end = where_;
        while self.ui[end] >= 0 {
            end += 1;
        }
        self.ui[where_] = self.ui[end - 1];
        self.ux[where_] = self.ux[end - 1];
        self.ui[end - 1] = -1;
        for n in (1..=nswap).rev() {
            let j = u(jlist[n]);
            let i = self.is[pmap + u(jlist[n - 1])];
            self.is[pmap + j] = i;
            self.is[qmap + u(i)] = j as Int;
        }
        self.is[pmap + j0] = in_ as Int;
        self.is[qmap + in_] = j0 as Int;
    }
}

fn onenorm(x: &[f64]) -> f64 {
    let mut d = 0.0;
    for v in x {
        d += v.abs();
    }
    d
}

/// `find`: the position of `j` in `index[start..end]`, or `end`; with a
/// negative `end`, in the list from `start` ended by a negative index.
fn find(j: Int, index: &[Int], mut start: Int, end: Int) -> Int {
    if end >= 0 {
        while start < end && index[u(start)] != j {
            start += 1;
        }
        start
    } else {
        while index[u(start)] != j && index[u(start)] >= 0 {
            start += 1;
        }
        if index[u(start)] == j { start } else { end }
    }
}

fn compress_packed(m: usize, is: &mut [Int], begin: usize, index: &mut [Int], value: &mut [f64]) -> Int {
    let mut nz = 0;
    let end = is[begin + m];
    for i in 0..m {
        let p = u(is[begin + i]);
        if index[p] == GAP {
            is[begin + i] = 0;
        } else {
            is[begin + i] = index[p];
            index[p] = GAP - i as Int - 1;
        }
    }
    let mut i: Int = -1;
    let mut put = 1usize;
    for get in 1..u(end) {
        if index[get] > GAP {
            index[put] = index[get];
            value[put] = value[get];
            put += 1;
            nz += 1;
        } else if index[get] < GAP {
            i = GAP - index[get] - 1;
            index[put] = is[begin + u(i)];
            is[begin + u(i)] = put as Int;
            value[put] = value[get];
            put += 1;
            nz += 1;
        } else if i >= 0 {
            i = -1;
            index[put] = GAP;
            put += 1;
        }
    }
    is[begin + m] = put as Int;
    nz
}

/// `lu_condest` and its `lu_normest`: the 1-norm of the triangular factor
/// at `begin`, an estimate of its inverse's, and their product.
#[allow(clippy::too_many_arguments)]
fn condest(m: usize, is: &[Int], begin: usize, ui: &[Int], ux: &[f64], pivot: Option<&[f64]>, perm: Option<usize>, upper: bool, work: &mut [f64]) -> (f64, f64, f64) {
    let mut unorm: f64 = 0.0;
    for j in 0..m {
        let mut colsum = pivot.map_or(1.0, |pivot| pivot[j].abs());
        let mut p = is[begin + j];
        while ui[u(p)] >= 0 {
            colsum += ux[u(p)].abs();
            p += 1;
        }
        unorm = unorm.max(colsum);
    }
    let order = |k: usize| perm.map_or(k, |perm| u(is[perm + k]));
    let mut x1norm = 0.0;
    let mut xinfnorm: f64 = 0.0;
    let forward: Vec<usize> = if upper { (0..m).collect() } else { (0..m).rev().collect() };
    for &k in &forward {
        let j = order(k);
        let mut temp = 0.0;
        let mut p = is[begin + j];
        loop {
            let i = ui[u(p)];
            if i < 0 {
                break;
            }
            temp -= work[u(i)] * ux[u(p)];
            p += 1;
        }
        temp += if temp >= 0.0 { 1.0 } else { -1.0 };
        if let Some(pivot) = pivot {
            temp /= pivot[j];
        }
        work[j] = temp;
        x1norm += f64::abs(temp);
        xinfnorm = xinfnorm.max(temp.abs());
    }
    let mut y1norm = 0.0;
    for &k in forward.iter().rev() {
        let j = order(k);
        if let Some(pivot) = pivot {
            work[j] /= pivot[j];
        }
        let temp = work[j];
        let mut p = is[begin + j];
        loop {
            let i = ui[u(p)];
            if i < 0 {
                break;
            }
            work[u(i)] -= temp * ux[u(p)];
            p += 1;
        }
        y1norm += f64::abs(temp);
    }
    let uinvnorm = (y1norm / x1norm).max(xinfnorm);
    (unorm, uinvnorm, unorm * uinvnorm)
}

// ---- Lists and files ------------------------------------------------------

fn list_init(is: &mut [Int], flink: usize, blink: usize, nelem: usize, nlist: usize, min_list: Option<&mut Int>) {
    for i in 0..nelem + nlist {
        is[flink + i] = i as Int;
        is[blink + i] = i as Int;
    }
    if let Some(min_list) = min_list {
        *min_list = (nlist as Int).max(1);
    }
}

fn list_add(is: &mut [Int], flink: usize, blink: usize, elem: usize, list: usize, nelem: usize, min_list: Option<&mut Int>) {
    let temp = is[blink + nelem + list];
    is[blink + nelem + list] = elem as Int;
    is[blink + elem] = temp;
    is[flink + u(temp)] = elem as Int;
    is[flink + elem] = (nelem + list) as Int;
    if list > 0
        && let Some(min_list) = min_list
        && (list as Int) < *min_list
    {
        *min_list = list as Int;
    }
}

fn list_remove(is: &mut [Int], flink: usize, blink: usize, elem: usize) {
    let next = is[flink + elem];
    let prev = is[blink + elem];
    is[flink + u(prev)] = next;
    is[blink + u(next)] = prev;
    is[flink + elem] = elem as Int;
    is[blink + elem] = elem as Int;
}

fn list_move(is: &mut [Int], flink: usize, blink: usize, elem: usize, list: usize, nelem: usize, min_list: Option<&mut Int>) {
    list_remove(is, flink, blink, elem);
    list_add(is, flink, blink, elem, list, nelem, min_list);
}

fn list_swap(is: &mut [Int], flink: usize, blink: usize, e1: usize, e2: usize) {
    let e1next = u(is[flink + e1]);
    let e2next = u(is[flink + e2]);
    let e1prev = u(is[blink + e1]);
    let e2prev = u(is[blink + e2]);
    let (e1i, e2i) = (e1 as Int, e2 as Int);
    if e1next == e2 {
        is[flink + e2] = e1i;
        is[blink + e1] = e2i;
        is[flink + e1prev] = e2i;
        is[blink + e2] = e1prev as Int;
        is[flink + e1] = e2next as Int;
        is[blink + e2next] = e1i;
    } else if e2next == e1 {
        is[flink + e1] = e2i;
        is[blink + e2] = e1i;
        is[flink + e2] = e1next as Int;
        is[blink + e1next] = e2i;
        is[flink + e2prev] = e1i;
        is[blink + e1] = e2prev as Int;
    } else {
        is[flink + e2] = e1next as Int;
        is[blink + e1next] = e2i;
        is[flink + e2prev] = e1i;
        is[blink + e1] = e2prev as Int;
        is[flink + e1prev] = e2i;
        is[blink + e2] = e1prev as Int;
        is[flink + e1] = e2next as Int;
        is[blink + e2next] = e1i;
    }
}

fn file_empty(is: &mut [Int], nlines: usize, begin: usize, end: usize, next: usize, prev: usize, fmem: Int) {
    is[begin + nlines] = 0;
    is[end + nlines] = fmem;
    for i in 0..nlines {
        is[begin + i] = 0;
        is[end + i] = 0;
    }
    for i in 0..nlines {
        is[next + i] = i as Int + 1;
        is[prev + i + 1] = i as Int;
    }
    is[next + nlines] = 0;
    is[prev] = nlines as Int;
}

#[allow(clippy::too_many_arguments)]
fn file_reappend(is: &mut [Int], line: usize, nlines: usize, begin: usize, end: usize, next: usize, prev: usize, index: &mut [Int], value: &mut [f64], extra_space: Int) {
    let mut used = is[begin + nlines];
    let ibeg = is[begin + line];
    let iend = is[end + line];
    is[begin + line] = used;
    for pos in ibeg..iend {
        index[u(used)] = index[u(pos)];
        value[u(used)] = value[u(pos)];
        used += 1;
    }
    is[end + line] = used;
    used += extra_space;
    is[begin + nlines] = used;
    list_move(is, next, prev, line, 0, nlines, None);
}

#[allow(clippy::too_many_arguments)]
fn file_compress(is: &mut [Int], nlines: usize, begin: usize, end: usize, next: usize, index: &mut [Int], value: &mut [f64], stretch: f64, pad: Int) -> Int {
    let mut nz = 0;
    let mut used: Int = 0;
    let mut extra_space: Int = 0;
    let mut i = u(is[next + nlines]);
    while i < nlines {
        let ibeg = is[begin + i];
        let iend = is[end + i];
        used += extra_space;
        if used > ibeg {
            used = ibeg;
        }
        is[begin + i] = used;
        for pos in ibeg..iend {
            index[u(used)] = index[u(pos)];
            value[u(used)] = value[u(pos)];
            used += 1;
        }
        is[end + i] = used;
        extra_space = trunc(stretch * (iend - ibeg) as f64 + pad as f64);
        nz += iend - ibeg;
        i = u(is[next + i]);
    }
    used += extra_space;
    if used > is[begin + nlines] {
        used = is[begin + nlines];
    }
    is[begin + nlines] = used;
    nz
}

// ---- Sparse triangular solves ---------------------------------------------

/// `lu_solve_symbolic`: the pattern of the solution from the right-hand
/// side's indices `irhs`, written to `is[ilhs + top..ilhs + m]`; returns
/// `top`.
#[allow(clippy::too_many_arguments)]
fn solve_symbolic(m: Int, is: &mut [Int], begin: usize, end: Option<usize>, index: &[Int], irhs: &[Int], ilhs: usize, pstack: &mut [Int], marked: usize, big_m: Int) -> Int {
    let mut top = m;
    for &i in irhs {
        if is[marked + u(i)] != big_m {
            top = dfs(i, is, begin, end, index, top, ilhs, pstack, marked, big_m);
        }
    }
    top
}

/// `lu_dfs`: depth-first search from `i`, writing the nodes finished to
/// `is[xi + ..top]`; returns the new `top`.
#[allow(clippy::too_many_arguments)]
fn dfs(mut i: Int, is: &mut [Int], begin: usize, end: Option<usize>, index: &[Int], mut top: Int, xi: usize, pstack: &mut [Int], marked: usize, big_m: Int) -> Int {
    if is[marked + u(i)] == big_m {
        return top;
    }
    let mut head: Int = 0;
    is[xi] = i;
    while head >= 0 {
        i = is[xi + u(head)];
        if is[marked + u(i)] != big_m {
            is[marked + u(i)] = big_m;
            pstack[u(head)] = is[begin + u(i)];
        }
        let mut done = true;
        let mut p = pstack[u(head)];
        loop {
            let inext = match end {
                Some(end) => {
                    if p >= is[end + u(i)] {
                        break;
                    }
                    index[u(p)]
                }
                None => {
                    let inext = index[u(p)];
                    if inext < 0 {
                        break;
                    }
                    inext
                }
            };
            if is[marked + u(inext)] == big_m {
                p += 1;
                continue;
            }
            pstack[u(head)] = p + 1;
            head += 1;
            is[xi + u(head)] = inext;
            done = false;
            break;
        }
        if done {
            head -= 1;
            top -= 1;
            is[xi + u(top)] = i;
        }
    }
    top
}

/// `lu_solve_triangular` over the pattern `is[symb..symb_end]`; returns
/// the number of nonzeros it wrote to `pattern`.
#[allow(clippy::too_many_arguments)]
fn solve_triangular(
    symb: usize,
    symb_end: usize,
    is: &mut [Int],
    begin: usize,
    end: Option<usize>,
    index: &[Int],
    value: &[f64],
    pivot: Option<&[f64]>,
    droptol: f64,
    lhs: &mut [f64],
    mut pattern: PatternOut,
    flops: &mut Int,
) -> Int {
    let mut nz = 0usize;
    let mut flop_count = 0;
    for n in symb..symb_end {
        let ipivot = u(is[n]);
        if lhs[ipivot] != 0.0 {
            let x = match pivot {
                Some(pivot) => {
                    lhs[ipivot] /= pivot[ipivot];
                    flop_count += 1;
                    lhs[ipivot]
                }
                None => lhs[ipivot],
            };
            let mut pos = is[begin + ipivot];
            loop {
                let i = match end {
                    Some(end) => {
                        if pos >= is[end + ipivot] {
                            break;
                        }
                        index[u(pos)]
                    }
                    None => {
                        let i = index[u(pos)];
                        if i < 0 {
                            break;
                        }
                        i
                    }
                };
                lhs[u(i)] -= x * value[u(pos)];
                flop_count += 1;
                pos += 1;
            }
            if x.abs() > droptol {
                match &mut pattern {
                    PatternOut::Istore(at) => is[*at + nz] = ipivot as Int,
                    PatternOut::Slice(out) => out[nz] = ipivot as Int,
                }
                nz += 1;
            } else {
                lhs[ipivot] = 0.0;
            }
        }
    }
    *flops += flop_count;
    nz as Int
}
