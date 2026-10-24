//! HiGHS's `HighsCutGeneration`: cuts from a single row relaxation, by
//! lifted cover inequalities (pure binary, mixed binary, mixed integer
//! knapsacks) and complemented mixed integer rounding, then scaled,
//! cleaned and added to the cut pool.

use super::{cpp_partial_sort, cpp_partition, data::MipData, domain::Domain, lp_relaxation::LpRelaxation, transformed_lp::TransformedLp};
use crate::{
    hash_table::hash_words,
    integers::{integral_scale, is_integral, nearest_integer},
    pdqsort::{pdqsort, pdqsort_branchless},
    util::{CDouble, Random, frexp_exp, ldexp},
};

const INF: f64 = f64::INFINITY;
const TINY: f64 = super::TINY;

/// HiGHS's `fast_floor`.
fn fast_floor(x: f64) -> f64 {
    let t = x as i64;
    (t - i64::from(x < t as f64)) as f64
}

pub(crate) struct CutGeneration {
    randgen: Random,
    cover: Vec<usize>,
    coverweight: CDouble,
    lambda: CDouble,
    upper: Vec<f64>,
    solval: Vec<f64>,
    complementation: Vec<u8>,
    isintegral: Vec<bool>,
    feastol: f64,
    epsilon: f64,
    /// The row being worked on (HiGHS's `vals`/`inds` pointers), its first
    /// `rowlen` entries live.
    vals: Vec<f64>,
    inds: Vec<usize>,
    rhs: CDouble,
    integral_support: bool,
    integral_coefficients: bool,
    rowlen: usize,
    initial_scale: f64,
    integerinds: Vec<usize>,
    deltas: Vec<f64>,
    tmp_vals: Vec<f64>,
    tmp_inds: Vec<usize>,
    tmp_complementation: Vec<u8>,
    tmp_solval: Vec<f64>,
}

impl CutGeneration {
    pub fn new(lp: &LpRelaxation, data: &MipData) -> Self {
        let seed = (data.options.random_seed as i64 + lp.num_lp_iterations() + data.cutpool.num_cuts() as i64) as u32;
        Self {
            randgen: Random::new(seed),
            cover: Vec::new(),
            coverweight: CDouble::new(0.0),
            lambda: CDouble::new(0.0),
            upper: Vec::new(),
            solval: Vec::new(),
            complementation: Vec::new(),
            isintegral: Vec::new(),
            feastol: data.feastol,
            epsilon: data.epsilon,
            vals: Vec::new(),
            inds: Vec::new(),
            rhs: CDouble::new(0.0),
            integral_support: false,
            integral_coefficients: false,
            rowlen: 0,
            initial_scale: 0.0,
            integerinds: Vec::new(),
            deltas: Vec::new(),
            tmp_vals: Vec::new(),
            tmp_inds: Vec::new(),
            tmp_complementation: Vec::new(),
            tmp_solval: Vec::new(),
        }
    }

    /// HiGHS's `determineCover`.
    fn determine_cover(&mut self, lp_sol: bool, data: &MipData) -> bool {
        let feastol = self.feastol;
        if self.rhs <= 10.0 * feastol {
            return false;
        }
        self.cover.clear();
        self.cover.reserve(self.rowlen);
        for j in 0..self.rowlen {
            if !self.isintegral[j] {
                continue;
            }
            if lp_sol && self.solval[j] <= feastol {
                continue;
            }
            self.cover.push(j);
        }
        let max_cover_size = self.cover.len();
        let mut coversize = 0;
        let r = self.randgen.integer_any() as u32;
        self.coverweight = CDouble::new(0.0);
        let (vals, inds, upper, solval) = (&self.vals, &self.inds, &self.upper, &self.solval);
        let tie = |i: usize| hash_words(&[inds[i] as u32, r]);
        if lp_sol {
            coversize = cpp_partition(&mut self.cover, |&j| solval[j] >= upper[j] - feastol);
            for i in 0..coversize {
                let j = self.cover[i];
                self.coverweight += vals[j] * upper[j];
            }
            pdqsort(&mut self.cover[coversize..max_cover_size], |&i, &j| {
                if upper[i] < 1.5 && upper[j] > 1.5 {
                    return true;
                }
                if upper[i] > 1.5 && upper[j] < 1.5 {
                    return false;
                }
                let contribution_a = solval[i] * vals[i];
                let contribution_b = solval[j] * vals[j];
                if contribution_a > contribution_b + feastol {
                    return true;
                }
                if contribution_a < contribution_b - feastol {
                    return false;
                }
                if (vals[i] - vals[j]).abs() <= feastol {
                    return tie(i) > tie(j);
                }
                vals[i] > vals[j]
            });
        } else {
            let nodequeue = data.nodequeue.borrow();
            let complementation = &self.complementation;
            pdqsort(&mut self.cover[coversize..max_cover_size], |&i, &j| {
                if solval[i] > feastol && solval[j] <= feastol {
                    return true;
                }
                if solval[i] <= feastol && solval[j] > feastol {
                    return false;
                }
                let num_nodes_a = if complementation[i] != 0 {
                    nodequeue.num_nodes_down(inds[i])
                } else {
                    nodequeue.num_nodes_up(inds[i])
                };
                let num_nodes_b = if complementation[j] != 0 {
                    nodequeue.num_nodes_down(inds[j])
                } else {
                    nodequeue.num_nodes_up(inds[j])
                };
                if num_nodes_a > num_nodes_b {
                    return true;
                }
                if num_nodes_a < num_nodes_b {
                    return false;
                }
                tie(i) > tie(j)
            });
        }
        let minlambda = (10.0 * feastol).max(feastol * self.rhs.value().abs());
        while coversize != max_cover_size {
            let lambda = (self.coverweight - self.rhs).value();
            if lambda > minlambda {
                break;
            }
            let j = self.cover[coversize];
            self.coverweight += self.vals[j] * self.upper[j];
            coversize += 1;
        }
        if coversize == 0 {
            return false;
        }
        self.coverweight.renormalize();
        self.lambda = self.coverweight - self.rhs;
        if self.lambda <= minlambda {
            return false;
        }
        self.cover.truncate(coversize);
        true
    }

    /// HiGHS's `separateLiftedKnapsackCover`.
    fn separate_lifted_knapsack_cover(&mut self) {
        let coversize = self.cover.len();
        let mut s = vec![0.0; coversize];
        let mut coverflag = vec![0i8; self.rowlen];
        let vals = &self.vals;
        pdqsort_branchless(&mut self.cover, |&a, &b| vals[a] > vals[b]);
        let cover = &self.cover;
        let mut abartmp = CDouble::new(vals[cover[0]]);
        let mut sigma = self.lambda;
        for i in 1..coversize {
            let delta = abartmp - vals[cover[i]];
            let kdelta = i as f64 * delta;
            if kdelta.value() < sigma.value() {
                abartmp = CDouble::new(vals[cover[i]]);
                sigma -= kdelta;
            } else {
                abartmp -= sigma * (1.0 / i as f64);
                sigma = CDouble::new(0.0);
                break;
            }
        }
        if sigma.value() > 0.0 {
            abartmp = self.rhs / coversize as f64;
        }
        let abar = abartmp.value();
        let mut sum = CDouble::new(0.0);
        let mut cplussize = 0;
        for i in 0..coversize {
            sum += abar.min(vals[cover[i]]);
            s[i] = sum.value();
            if vals[cover[i]] > abar + self.feastol {
                cplussize += 1;
                coverflag[cover[i]] = 1;
            } else {
                coverflag[cover[i]] = -1;
            }
        }
        let mut halfintegral = false;
        let (feastol, epsilon) = (self.feastol, self.epsilon);
        let mut g = |z: f64| {
            let hfrac = z / abar;
            let mut coef = 0.0;
            let mut h = (hfrac + 0.5).floor() as i64;
            if h != 0 && (hfrac - h as f64).abs() * abar.max(1.0) <= epsilon && h < cplussize as i64 {
                halfintegral = true;
                coef = 0.5;
            }
            h = (h - 1).max(0);
            while (h as usize) < coversize {
                if z <= s[h as usize] + feastol {
                    break;
                }
                h += 1;
            }
            coef + h as f64
        };
        self.rhs = CDouble::new((coversize - 1) as f64);
        for i in 0..self.rowlen {
            if self.vals[i] == 0.0 {
                continue;
            }
            if coverflag[i] == -1 {
                self.vals[i] = 1.0;
            } else {
                self.vals[i] = g(self.vals[i]);
            }
        }
        if halfintegral {
            self.rhs *= 2.0;
            for i in 0..self.rowlen {
                self.vals[i] *= 2.0;
            }
        }
        self.integral_support = true;
        self.integral_coefficients = true;
    }

    /// HiGHS's `separateLiftedMixedBinaryCover`.
    fn separate_lifted_mixed_binary_cover(&mut self) -> bool {
        self.integral_support = false;
        self.integral_coefficients = false;
        let coversize = self.cover.len();
        let mut s = vec![0.0; coversize];
        let mut coverflag = vec![false; self.rowlen];
        if coversize == 0 {
            return false;
        }
        for i in 0..coversize {
            coverflag[self.cover[i]] = true;
        }
        let vals = &self.vals;
        pdqsort_branchless(&mut self.cover, |&a, &b| vals[a] > vals[b]);
        let mut sum = CDouble::new(0.0);
        let mut p = coversize;
        let lambda = self.lambda;
        for i in 0..coversize {
            if vals[self.cover[i]] - lambda <= self.epsilon {
                p = i;
                break;
            }
            sum += vals[self.cover[i]];
            s[i] = sum.value();
        }
        if p == 0 {
            return false;
        }
        let phi = |a: f64| {
            for i in 0..p {
                if a <= (s[i] - lambda).value() {
                    return (i as f64 * lambda).value();
                }
                if a <= s[i] {
                    return ((i + 1) as f64 * lambda + (CDouble::new(a) - s[i])).value();
                }
            }
            (p as f64 * lambda + (CDouble::new(a) - s[p - 1])).value()
        };
        self.rhs = -lambda;
        self.integral_coefficients = false;
        self.integral_support = true;
        for i in 0..self.rowlen {
            if !self.isintegral[i] {
                if self.vals[i] < 0.0 {
                    self.integral_support = false;
                } else {
                    self.vals[i] = 0.0;
                }
                continue;
            }
            if coverflag[i] {
                self.vals[i] = self.vals[i].min(lambda.value());
                self.rhs += self.vals[i];
            } else {
                self.vals[i] = phi(self.vals[i]);
            }
        }
        true
    }

    /// HiGHS's `separateLiftedMixedIntegerCover`.
    fn separate_lifted_mixed_integer_cover(&mut self) -> bool {
        self.integral_support = false;
        self.integral_coefficients = false;
        let feastol = self.feastol;
        let epsilon = self.epsilon;
        let coversize = self.cover.len();
        let mut coverflag = vec![false; self.rowlen];
        for &i in &self.cover {
            coverflag[i] = true;
        }
        let vals = &self.vals;
        pdqsort_branchless(&mut self.cover, |&a, &b| vals[a] > vals[b]);
        let mut a = vec![CDouble::new(0.0); coversize];
        let mut u = vec![CDouble::new(0.0); coversize + 1];
        let mut m = vec![CDouble::new(0.0); coversize + 1];
        let mut usum = CDouble::new(0.0);
        let mut msum = CDouble::new(0.0);
        for c in 0..coversize {
            let i = self.cover[c];
            u[c] = usum;
            m[c] = msum;
            a[c] = CDouble::new(vals[i]);
            let ub = self.upper[i];
            usum += ub;
            msum += ub * a[c];
        }
        u[coversize] = usum;
        m[coversize] = msum;

        let lambda = self.lambda;
        let mut lpos: Option<usize> = None;
        let mut bestl_cplusend = 0;
        let mut bestl_val = 0.0;
        let mut bestl_at_upper = true;
        for i in 0..coversize {
            let j = self.cover[i];
            let ub = self.upper[j];
            let at_upper = self.solval[j] >= ub - feastol;
            if at_upper && !bestl_at_upper {
                continue;
            }
            let mju = ub * vals[j];
            let mu = mju - lambda;
            if mu <= 10.0 * feastol {
                continue;
            }
            if vals[j].abs() < 1000.0 * feastol {
                continue;
            }
            let mudival = (mu / vals[j]).value();
            if is_integral(mudival, feastol) {
                continue;
            }
            let eta = mudival.ceil();
            let ulminusetaplusone = CDouble::new(ub) - eta + 1.0;
            let cplusthreshold = (ulminusetaplusone * vals[j]).value();
            let cplusend = self.cover.partition_point(|&i| !(cplusthreshold > vals[i]));
            let mut mcplus = m[cplusend];
            if i < cplusend {
                mcplus -= mju;
            }
            let jl_val = (mcplus + eta * vals[j]).value();
            if jl_val > bestl_val || (!at_upper && bestl_at_upper) {
                lpos = Some(i);
                bestl_cplusend = cplusend;
                bestl_val = jl_val;
                bestl_at_upper = at_upper;
            }
        }
        let Some(lpos) = lpos else { return false };
        let l = self.cover[lpos];
        let al = CDouble::new(vals[l]);
        let upperl = self.upper[l];
        let mlu = upperl * al;
        let mu = mlu - lambda;
        a.truncate(bestl_cplusend);
        self.cover.truncate(bestl_cplusend);
        u.truncate(bestl_cplusend + 1);
        m.truncate(bestl_cplusend + 1);
        if lpos < bestl_cplusend {
            a.remove(lpos);
            self.cover.remove(lpos);
            u.remove(lpos + 1);
            m.remove(lpos + 1);
            for i in lpos + 1..bestl_cplusend {
                u[i] -= upperl;
                m[i] -= mlu;
            }
        }
        let cplussize = a.len();
        let mudival = (mu / al).value();
        let eta = mudival.ceil();
        let mut r = mu - mudival.floor() * al;
        if r < 0.0 {
            r = CDouble::new(0.0);
        }
        let ulminusetaplusone = CDouble::new(upperl) - eta + 1.0;
        let cplusthreshold = ulminusetaplusone * al;
        let kmin = (eta - upperl - 0.5).floor() as i64;
        let phi_l = |a: f64| {
            let mut k = ((a / al.value()) as i64).min(-1);
            while k >= kmin {
                if a >= k as f64 * al + r {
                    return (a - (k + 1) as f64 * r).value();
                }
                if a >= k as f64 * al {
                    return (k as f64 * (al - r)).value();
                }
                k -= 1;
            }
            (kmin as f64 * (al - r)).value()
        };
        let kmax = (upperl - eta + 0.5).floor() as i64;
        let cover = &self.cover;
        let upper = &self.upper;
        let gamma_l = |z: f64| {
            for i in 0..cplussize {
                let upperi = upper[cover[i]] as i32;
                for h in 0..=upperi {
                    let mih = m[i] + h as f64 * a[i];
                    let uih = u[i] + h as f64;
                    let mihplusdeltai = mih + a[i] - cplusthreshold;
                    if z <= mihplusdeltai {
                        return (uih * ulminusetaplusone * (al - r)).value();
                    }
                    let mut k = ((z - mihplusdeltai) / al).value() as i64 - 1;
                    while k <= kmax {
                        if z <= mihplusdeltai + k as f64 * al + r {
                            return ((uih * ulminusetaplusone + k as f64) * (al - r)).value();
                        }
                        if z <= mihplusdeltai + (k + 1) as f64 * al {
                            return ((uih * ulminusetaplusone) * (al - r) + z - mih - a[i] + cplusthreshold - (k + 1) as f64 * r).value();
                        }
                        k += 1;
                    }
                }
            }
            let mut p = ((z - m[cplussize]) / al).value() as i64 - 1;
            loop {
                if z <= m[cplussize] + p as f64 * al + r {
                    return ((u[cplussize] * ulminusetaplusone + p as f64) * (al - r)).value();
                }
                if z <= m[cplussize] + (p + 1) as f64 * al {
                    return ((u[cplussize] * ulminusetaplusone) * (al - r) + z - m[cplussize] - (p + 1) as f64 * r).value();
                }
                p += 1;
            }
        };
        let _ = epsilon;
        let mut rhs = (CDouble::new(upperl) - eta) * r - lambda;
        let mut integral_support = true;
        for i in 0..self.rowlen {
            if self.vals[i] == 0.0 {
                continue;
            }
            if !self.isintegral[i] {
                if self.vals[i] < 0.0 {
                    integral_support = false;
                } else {
                    self.vals[i] = 0.0;
                }
                continue;
            }
            if coverflag[i] {
                self.vals[i] = -phi_l(-self.vals[i]);
                rhs += self.vals[i] * self.upper[i];
            } else {
                self.vals[i] = gamma_l(self.vals[i]);
            }
        }
        self.rhs = rhs;
        self.integral_support = integral_support;
        self.integral_coefficients = false;
        true
    }

    /// The efficacy check HiGHS runs for each candidate scale `delta`.
    fn cmir_efficacy(&self, delta: f64, continuouscontribution: f64, continuoussqrnorm: f64) -> Option<f64> {
        const MAX_CMIR_SCALE: f64 = 1e6;
        const F0MIN: f64 = 0.005;
        const F0MAX: f64 = 0.995;
        let scale = 1.0 / delta;
        let scalrhs = self.rhs.value() * scale;
        let downrhs = fast_floor(scalrhs);
        let f0 = scalrhs - downrhs;
        if !(F0MIN..=F0MAX).contains(&f0) {
            return None;
        }
        let oneoveroneminusf0 = 1.0 / (1.0 - f0);
        if oneoveroneminusf0 > MAX_CMIR_SCALE {
            return None;
        }
        let contscale = scale * oneoveroneminusf0;
        let mut sqrnorm = contscale * contscale * continuoussqrnorm;
        let mut viol = contscale * continuouscontribution - downrhs;
        for &j in &self.integerinds {
            let scalaj = self.vals[j] * scale;
            let downaj = fast_floor(scalaj + TINY);
            let fj = scalaj - downaj;
            let aj = downaj + 0.0f64.max((fj - f0) * oneoveroneminusf0);
            self.update_violation_and_norm(j, aj, &mut viol, &mut sqrnorm);
        }
        Some(viol / sqrnorm.sqrt())
    }

    /// HiGHS's `cmirCutGenerationHeuristic`.
    fn cmir_cut_generation_heuristic(&mut self, min_efficacy: f64, only_initial_cmir_scale: bool) -> bool {
        const F0MIN: f64 = 0.005;
        const F0MAX: f64 = 0.995;
        let feastol = self.feastol;
        self.integral_support = false;
        self.integral_coefficients = false;
        let mut continuouscontribution = 0.0;
        let mut continuoussqrnorm = 0.0;
        self.deltas.clear();
        self.deltas.reserve(self.rowlen + 3);
        self.integerinds.clear();
        self.integerinds.reserve(self.rowlen);
        let mut maxabsdelta = 0.0f64;
        self.complementation.resize(self.rowlen, 0);
        for i in 0..self.rowlen {
            if self.isintegral[i] {
                self.integerinds.push(i);
                if self.upper[i] < 2.0 * self.solval[i] {
                    self.flip_complementation(i);
                }
                if only_initial_cmir_scale {
                    continue;
                }
                if self.solval[i] > feastol {
                    let delta = self.vals[i].abs();
                    if delta <= 1e-4 || delta == maxabsdelta {
                        continue;
                    }
                    maxabsdelta = maxabsdelta.max(delta);
                    self.deltas.push(delta);
                }
            } else {
                let vi = self.vals[i];
                self.update_violation_and_norm(i, vi, &mut continuouscontribution, &mut continuoussqrnorm);
            }
        }
        if continuoussqrnorm == 0.0 && self.deltas.len() > 1 {
            let int_scale = integral_scale(&self.deltas, feastol, TINY);
            if int_scale != 0.0 && int_scale <= 1e4 {
                let scalrhs = self.rhs.value() * int_scale;
                let downrhs = fast_floor(scalrhs);
                let f0 = scalrhs - downrhs;
                if (F0MIN..=F0MAX).contains(&f0) {
                    self.deltas.push(1.0 / int_scale);
                }
            }
        }
        self.deltas.push(1.0f64.min(self.initial_scale));
        if !only_initial_cmir_scale {
            self.deltas.push(maxabsdelta + 1.0f64.min(self.initial_scale));
        }
        if self.deltas.is_empty() {
            return false;
        }
        pdqsort(&mut self.deltas, |a, b| a < b);
        let mut curdelta = self.deltas[0];
        for i in 1..self.deltas.len() {
            if self.deltas[i] - curdelta <= 10.0 * feastol {
                self.deltas[i] = 0.0;
            } else {
                curdelta = self.deltas[i];
            }
        }
        self.deltas.retain(|&d| d != 0.0);
        let mut bestdelta = -1.0;
        let mut bestefficacy = min_efficacy;
        for &delta in &self.deltas.clone() {
            if let Some(efficacy) = self.cmir_efficacy(delta, continuouscontribution, continuoussqrnorm)
                && efficacy > bestefficacy
            {
                bestdelta = delta;
                bestefficacy = efficacy;
            }
        }
        if bestdelta == -1.0 {
            return false;
        }
        let mut k = 1;
        while !only_initial_cmir_scale && k <= 3 {
            let delta = bestdelta * (1 << k) as f64;
            if let Some(efficacy) = self.cmir_efficacy(delta, continuouscontribution, continuoussqrnorm)
                && efficacy > bestefficacy
            {
                bestdelta = delta;
                bestefficacy = efficacy;
            }
            k += 1;
        }
        for idx in 0..self.integerinds.len() {
            let k = self.integerinds[idx];
            if self.upper[k] == INF {
                continue;
            }
            if self.solval[k] <= feastol {
                continue;
            }
            self.flip_complementation(k);
            match self.cmir_efficacy(bestdelta, continuouscontribution, continuoussqrnorm) {
                None => self.flip_complementation(k),
                Some(efficacy) => {
                    if efficacy > bestefficacy {
                        bestefficacy = efficacy;
                    } else {
                        self.flip_complementation(k);
                    }
                }
            }
        }
        let scale = 1.0 / CDouble::new(bestdelta);
        let scalrhs = self.rhs * scale;
        let downrhs = scalrhs.value().floor();
        let f0 = scalrhs - downrhs;
        let oneoveroneminusf0 = 1.0 / (1.0 - f0);
        self.rhs = CDouble::new(downrhs * bestdelta);
        self.integral_support = true;
        self.integral_coefficients = false;
        for j in 0..self.rowlen {
            if self.vals[j] == 0.0 {
                continue;
            }
            if !self.isintegral[j] {
                if self.vals[j] > 0.0 {
                    self.vals[j] = 0.0;
                } else {
                    self.vals[j] = (self.vals[j] * oneoveroneminusf0).value();
                    self.integral_support = false;
                }
            } else {
                let scalaj = scale * self.vals[j];
                let downaj = (scalaj + TINY).value().floor();
                let fj = scalaj - downaj;
                let mut aj = CDouble::new(downaj);
                if fj > f0 {
                    aj += (fj - f0) * oneoveroneminusf0;
                }
                self.vals[j] = (aj * bestdelta).value();
            }
        }
        true
    }

    /// HiGHS's `scale`: scale the row by a power of two so that `val`
    /// becomes about 1 (by at most 1024 upwards); returns the factor.
    fn scale(&mut self, val: f64) -> f64 {
        let mut expshift = -frexp_exp(val);
        expshift = expshift.min(10);
        self.rhs = self.rhs.ldexp(expshift);
        for i in 0..self.rowlen {
            self.vals[i] = ldexp(self.vals[i], expshift);
        }
        ldexp(1.0, expshift)
    }

    /// HiGHS's `postprocessCut`.
    fn postprocess_cut(&mut self, globaldom: &Domain, lp: &LpRelaxation, data: &MipData) -> bool {
        let feastol = self.feastol;
        let epsilon = self.epsilon;
        if self.rhs < 0.0 && self.rhs > -epsilon {
            self.rhs = CDouble::new(0.0);
        }
        if self.integral_support && self.integral_coefficients {
            self.remove_zeros();
            return true;
        }
        let mut max_abs_value = 0.0f64;
        for i in 0..self.rowlen {
            max_abs_value = self.vals[i].abs().max(max_abs_value);
        }
        let min_coefficient_value = 100.0 * feastol * max_abs_value.max(1e-3);
        self.integral_support = true;
        for i in (0..self.rowlen).rev() {
            if self.vals[i] == 0.0 {
                continue;
            }
            if self.vals[i].abs() <= min_coefficient_value {
                if self.vals[i] < 0.0 {
                    let ub = globaldom.col_upper[self.inds[i]];
                    if ub == INF {
                        return false;
                    }
                    self.rhs -= ub * self.vals[i];
                } else {
                    let lb = globaldom.col_lower[self.inds[i]];
                    if lb == -INF {
                        return false;
                    }
                    self.rhs -= lb * self.vals[i];
                }
                self.vals[i] = 0.0;
                continue;
            }
            if self.integral_support && !lp.is_col_integral(self.inds[i], data) {
                self.integral_support = false;
            }
        }
        self.remove_zeros();
        if self.rowlen == 0 {
            return false;
        }
        if self.integral_support {
            let intscale = integral_scale(&self.vals[..self.rowlen], feastol, epsilon);
            let mut scale_smallest_val_to_one = true;
            if intscale != 0.0 && intscale * max_abs_value.max(1.0) <= (1u64 << 52) as f64 {
                self.rhs.renormalize();
                self.rhs *= intscale;
                max_abs_value = nearest_integer(max_abs_value * intscale) as f64;
                for i in 0..self.rowlen {
                    let scaleval = intscale * CDouble::new(self.vals[i]);
                    let intval = nearest_integer(scaleval.value()) as f64;
                    let delta = (scaleval - intval).value();
                    self.vals[i] = intval;
                    if delta < 0.0 {
                        let ub = globaldom.col_upper[self.inds[i]];
                        if ub == INF {
                            return false;
                        }
                        self.rhs -= delta * ub;
                    } else {
                        let lb = globaldom.col_lower[self.inds[i]];
                        if lb == -INF {
                            return false;
                        }
                        self.rhs -= delta * lb;
                    }
                }
                self.rhs = (self.rhs + feastol).floor();
                if intscale * max_abs_value * feastol < 0.5 {
                    scale_smallest_val_to_one = false;
                    self.integral_coefficients = true;
                }
            }
            if scale_smallest_val_to_one {
                let mut min_abs_value = INF;
                for i in 0..self.rowlen {
                    min_abs_value = self.vals[i].abs().min(min_abs_value);
                }
                self.scale(min_abs_value - epsilon);
            }
        } else {
            self.scale(max_abs_value - epsilon);
        }
        true
    }

    /// Remove zero coefficients in place, as HiGHS does from the back.
    fn remove_zeros(&mut self) {
        for i in (0..self.rowlen).rev() {
            if self.vals[i] == 0.0 {
                self.rowlen -= 1;
                self.inds[i] = self.inds[self.rowlen];
                self.vals[i] = self.vals[self.rowlen];
            }
        }
    }

    /// HiGHS's `preprocessBaseInequality`: `(has_unbounded_ints,
    /// has_general_ints, has_continuous)`, or `None` if the row is useless.
    fn preprocess_base_inequality(&mut self, lp: &LpRelaxation, data: &MipData) -> Option<(bool, bool, bool)> {
        let feastol = self.feastol;
        let mut has_unbounded_ints = false;
        let mut has_continuous = false;
        let mut has_general_ints = false;
        let mut num_zeros = 0;
        let mut maxact = -feastol;
        let mut max_abs_val = 0.0f64;
        for i in 0..self.rowlen {
            max_abs_val = self.vals[i].abs().max(max_abs_val);
        }
        self.initial_scale = self.scale(max_abs_val);
        self.isintegral.resize(self.rowlen, false);
        for i in 0..self.rowlen {
            self.isintegral[i] = lp.is_col_integral(self.inds[i], data) && self.vals[i].abs() > 10.0 * feastol;
            if !self.isintegral[i] {
                if self.upper[i] < 2.0 * self.solval[i] {
                    if self.complementation.is_empty() {
                        self.complementation.resize(self.rowlen, 0);
                    }
                    self.flip_complementation(i);
                }
                if self.vals[i] > 0.0 || self.vals[i].abs() * self.upper[i] <= 10.0 * feastol {
                    if self.vals[i] < 0.0 {
                        if self.upper[i] == INF {
                            return None;
                        }
                        self.rhs -= self.vals[i] * self.upper[i];
                    }
                    num_zeros += 1;
                    self.vals[i] = 0.0;
                    continue;
                }
                has_continuous = true;
            } else {
                if self.upper[i] == INF {
                    has_unbounded_ints = true;
                    has_general_ints = true;
                } else if self.upper[i] != 1.0 {
                    has_general_ints = true;
                }
                if self.vals[i] > 0.0 {
                    maxact += self.vals[i] * self.upper[i];
                }
            }
        }
        let max_len = (100.0 + 0.15 * lp.num_cols() as f64) as i32 as usize;
        if self.rowlen - num_zeros > max_len {
            let num_cancel = self.rowlen - num_zeros - max_len;
            let mut cancel_nzs = Vec::new();
            for i in 0..self.rowlen {
                let cancel_slack = if self.vals[i] > 0.0 { self.solval[i] } else { self.upper[i] - self.solval[i] };
                if cancel_slack <= feastol {
                    cancel_nzs.push(i);
                }
            }
            if cancel_nzs.len() < num_cancel {
                return None;
            }
            if cancel_nzs.len() > num_cancel {
                let vals = &self.vals;
                cpp_partial_sort(&mut cancel_nzs, num_cancel, |&a, &b| vals[a].abs() < vals[b].abs());
            }
            for &j in &cancel_nzs[..num_cancel] {
                if self.vals[j] < 0.0 {
                    self.rhs -= self.vals[j] * self.upper[j];
                } else {
                    maxact -= self.vals[j] * self.upper[j];
                }
                self.vals[j] = 0.0;
            }
            num_zeros += num_cancel;
        }
        if num_zeros != 0 {
            let with_complementation = !self.complementation.is_empty();
            for i in (0..self.rowlen).rev() {
                if self.vals[i] == 0.0 {
                    self.rowlen -= 1;
                    let last = self.rowlen;
                    self.inds[i] = self.inds[last];
                    self.vals[i] = self.vals[last];
                    self.upper[i] = self.upper[last];
                    self.solval[i] = self.solval[last];
                    self.isintegral[i] = self.isintegral[last];
                    if with_complementation {
                        self.complementation[i] = self.complementation[last];
                    }
                    num_zeros -= 1;
                    if num_zeros == 0 {
                        break;
                    }
                }
            }
        }
        (maxact > self.rhs.value()).then_some((has_unbounded_ints, has_general_ints, has_continuous))
    }

    /// HiGHS's `generateCut`: separate the LP solution with the row
    /// `vals . x <= rhs`, adding the cut found to the pool.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_cut(
        &mut self,
        trans_lp: &mut TransformedLp,
        lp: &LpRelaxation,
        data: &mut MipData,
        inds: &mut Vec<usize>,
        vals: &mut Vec<f64>,
        rhs: &mut f64,
        only_initial_cmir_scale: bool,
    ) -> bool {
        let mut ints_positive = true;
        if !trans_lp.transform(lp, data, vals, &mut self.upper, &mut self.solval, inds, rhs, &mut ints_positive, false) {
            return false;
        }
        self.rowlen = inds.len();
        self.inds = std::mem::take(inds);
        self.vals = std::mem::take(vals);
        self.rhs = CDouble::new(*rhs);
        self.complementation.clear();
        let result = self.generate_cut_body(trans_lp, lp, data, rhs, ints_positive, only_initial_cmir_scale);
        *inds = std::mem::take(&mut self.inds);
        *vals = std::mem::take(&mut self.vals);
        result
    }

    fn generate_cut_body(
        &mut self,
        trans_lp: &mut TransformedLp,
        lp: &LpRelaxation,
        data: &mut MipData,
        rhs: &mut f64,
        ints_positive: bool,
        only_initial_cmir_scale: bool,
    ) -> bool {
        let Some((has_unbounded_ints, has_general_ints, has_continuous)) = self.preprocess_base_inequality(lp, data) else {
            return false;
        };
        if !has_unbounded_ints && !ints_positive {
            self.complementation.resize(self.rowlen, 0);
            for i in 0..self.rowlen {
                if self.vals[i] > 0.0 || !self.isintegral[i] {
                    continue;
                }
                self.flip_complementation(i);
            }
        }
        if !self.try_generate_cut(
            has_unbounded_ints,
            has_general_ints,
            has_continuous,
            10.0 * self.feastol,
            only_initial_cmir_scale,
            true,
            true,
            data,
        ) {
            return false;
        }
        self.remove_complementation();
        self.remove_zeros();
        *rhs = self.rhs.value();
        self.vals.truncate(self.rowlen);
        self.inds.truncate(self.rowlen);
        if !trans_lp.untransform(lp, data, &mut self.vals, &mut self.inds, rhs, false) {
            return false;
        }
        self.rowlen = self.inds.len();
        self.rhs = CDouble::new(*rhs);
        if !self.postprocess_cut(&data.domain, lp, data) {
            return false;
        }
        *rhs = self.rhs.value();
        self.vals.truncate(self.rowlen);
        self.inds.truncate(self.rowlen);
        self.finish_and_add(lp, data, rhs)
    }

    /// The common end of `generateCut` and `finalizeAndAddCut`: check the
    /// violation, tighten and add to the pool.
    fn finish_and_add(&mut self, lp: &LpRelaxation, data: &mut MipData, rhs: &mut f64) -> bool {
        let mut violation = CDouble::new(-*rhs);
        let sol = lp.lp_solver().col_value().expect("an LP solution");
        for i in 0..self.rowlen {
            violation += sol[self.inds[i]] * self.vals[i];
        }
        if violation <= 10.0 * self.feastol {
            return false;
        }
        data.domain.tighten_coefficients(&self.inds, &mut self.vals, rhs, &data.model, data.feastol);
        let integral = self.integral_support && self.integral_coefficients;
        data.add_cut(&mut self.inds, &mut self.vals, *rhs, integral, true, true, false).is_some()
    }

    /// HiGHS's `generateConflict`: a conflict from the proof row
    /// `proofvals . x <= proofrhs` that cuts off `localdomain`, added to the
    /// pool.
    pub fn generate_conflict(
        &mut self,
        localdomain: &Domain,
        lp: &LpRelaxation,
        data: &mut MipData,
        proofinds: &mut Vec<usize>,
        proofvals: &mut Vec<f64>,
        proofrhs: &mut f64,
    ) -> bool {
        self.inds = std::mem::take(proofinds);
        self.vals = std::mem::take(proofvals);
        self.rhs = CDouble::new(*proofrhs);
        self.rowlen = self.inds.len();
        let result = self.generate_conflict_body(localdomain, lp, data, proofrhs);
        *proofinds = std::mem::take(&mut self.inds);
        *proofvals = std::mem::take(&mut self.vals);
        result
    }

    fn generate_conflict_body(&mut self, localdomain: &Domain, lp: &LpRelaxation, data: &mut MipData, proofrhs: &mut f64) -> bool {
        let globaldom = &data.domain;
        self.complementation = vec![0; self.rowlen];
        self.upper.resize(self.rowlen, 0.0);
        self.solval.resize(self.rowlen, 0.0);
        let mut activity = 0.0;
        for i in 0..self.rowlen {
            let col = self.inds[i];
            self.upper[i] = globaldom.col_upper[col] - globaldom.col_lower[col];
            self.solval[i] = if self.vals[i] < 0.0 {
                globaldom.col_upper[col].min(localdomain.col_upper[col])
            } else {
                globaldom.col_lower[col].max(localdomain.col_lower[col])
            };
            if self.vals[i] < 0.0 && globaldom.col_upper[col] != INF {
                self.rhs -= globaldom.col_upper[col] * self.vals[i];
                self.vals[i] = -self.vals[i];
                self.complementation[i] = 1;
                self.solval[i] = globaldom.col_upper[col] - self.solval[i];
            } else {
                self.rhs -= globaldom.col_lower[col] * self.vals[i];
                self.complementation[i] = 0;
                self.solval[i] -= globaldom.col_lower[col];
            }
            activity += self.solval[i] * self.vals[i];
        }
        if activity > self.rhs.value() {
            let sol_scale = self.rhs.value() / activity;
            for i in 0..self.rowlen {
                self.solval[i] *= sol_scale;
            }
        }
        let Some((has_unbounded_ints, has_general_ints, has_continuous)) = self.preprocess_base_inequality(lp, data) else {
            return false;
        };
        if !self.try_generate_cut(has_unbounded_ints, has_general_ints, has_continuous, self.feastol, false, false, false, data) {
            return false;
        }
        let globaldom = &data.domain;
        if !self.complementation.is_empty() {
            for i in 0..self.rowlen {
                if self.complementation[i] != 0 {
                    self.rhs -= globaldom.col_upper[self.inds[i]] * self.vals[i];
                    self.vals[i] = -self.vals[i];
                } else {
                    self.rhs += globaldom.col_lower[self.inds[i]] * self.vals[i];
                }
            }
        }
        if !self.postprocess_cut(&data.domain, lp, data) {
            return false;
        }
        self.vals.truncate(self.rowlen);
        self.inds.truncate(self.rowlen);
        *proofrhs = self.rhs.value();
        let cutintegral = self.integral_support && self.integral_coefficients;
        data.domain.tighten_coefficients(&self.inds, &mut self.vals, proofrhs, &data.model, data.feastol);
        data.add_cut(&mut self.inds, &mut self.vals, *proofrhs, cutintegral, true, true, true).is_some()
    }

    /// HiGHS's `finalizeAndAddCut`: postprocess an outside cut and add it to
    /// the pool if the LP solution violates it enough.
    pub fn finalize_and_add_cut(&mut self, lp: &LpRelaxation, data: &mut MipData, inds: &mut Vec<usize>, vals: &mut Vec<f64>, rhs: &mut f64) -> bool {
        self.complementation.clear();
        self.rowlen = inds.len();
        self.inds = std::mem::take(inds);
        self.vals = std::mem::take(vals);
        self.rhs = CDouble::new(*rhs);
        self.integral_support = true;
        self.integral_coefficients = false;
        for i in (0..self.rowlen).rev() {
            if self.vals[i] == 0.0 {
                self.rowlen -= 1;
                self.inds[i] = self.inds[self.rowlen];
                self.vals[i] = self.vals[self.rowlen];
            } else {
                self.integral_support &= lp.is_col_integral(self.inds[i], data);
            }
        }
        self.vals.truncate(self.rowlen);
        self.inds.truncate(self.rowlen);
        let result = if self.postprocess_cut(&data.domain, lp, data) {
            *rhs = self.rhs.value();
            self.vals.truncate(self.rowlen);
            self.inds.truncate(self.rowlen);
            self.finish_and_add(lp, data, rhs)
        } else {
            false
        };
        *inds = std::mem::take(&mut self.inds);
        *vals = std::mem::take(&mut self.vals);
        result
    }

    /// HiGHS's `flipComplementation`.
    fn flip_complementation(&mut self, index: usize) {
        self.complementation[index] = 1 - self.complementation[index];
        self.solval[index] = self.upper[index] - self.solval[index];
        self.rhs -= self.upper[index] * self.vals[index];
        self.vals[index] = -self.vals[index];
    }

    /// HiGHS's `removeComplementation`.
    fn remove_complementation(&mut self) {
        if self.complementation.is_empty() {
            return;
        }
        for i in 0..self.rowlen {
            if self.complementation[i] != 0 {
                self.flip_complementation(i);
            }
        }
    }

    /// HiGHS's `updateViolationAndNorm`.
    fn update_violation_and_norm(&self, index: usize, aj: f64, violation: &mut f64, norm: &mut f64) {
        *violation += aj * self.solval[index];
        if aj > 0.0 && self.solval[index] <= self.feastol {
            return;
        }
        if aj < 0.0 && self.solval[index] >= self.upper[index] - self.feastol {
            return;
        }
        *norm += aj * aj;
    }

    /// HiGHS's `tryGenerateCut`: a lifted cover cut, unless complemented
    /// MIR does better.
    #[allow(clippy::too_many_arguments)]
    fn try_generate_cut(
        &mut self,
        has_unbounded_ints: bool,
        has_general_ints: bool,
        has_continuous: bool,
        min_efficacy: f64,
        only_initial_cmir_scale: bool,
        allow_reject_cut: bool,
        lp_sol: bool,
        data: &MipData,
    ) -> bool {
        if has_unbounded_ints {
            return self.cmir_cut_generation_heuristic(min_efficacy, only_initial_cmir_scale);
        }
        self.tmp_vals.clear();
        self.tmp_vals.extend_from_slice(&self.vals[..self.rowlen]);
        self.tmp_inds.clear();
        self.tmp_inds.extend_from_slice(&self.inds[..self.rowlen]);
        self.tmp_complementation.clone_from(&self.complementation);
        self.tmp_solval.clone_from(&self.solval);
        let mut tmp_rhs = self.rhs;
        let mut success = false;
        let mut save_integral_support = false;
        let mut save_integral_coefficients = false;
        if self.determine_cover(lp_sol, data) {
            if !has_continuous && !has_general_ints {
                self.separate_lifted_knapsack_cover();
                success = true;
            } else if has_general_ints {
                success = self.separate_lifted_mixed_integer_cover();
            } else {
                success = self.separate_lifted_mixed_binary_cover();
            }
        }
        let mut min_mir_efficacy = min_efficacy;
        if success {
            save_integral_support = self.integral_support;
            save_integral_coefficients = self.integral_coefficients;
            let mut violation = -self.rhs.value();
            let mut sqrnorm = 0.0;
            for i in 0..self.rowlen {
                let vi = self.vals[i];
                self.update_violation_and_norm(i, vi, &mut violation, &mut sqrnorm);
            }
            let efficacy = violation / sqrnorm.sqrt();
            if allow_reject_cut && efficacy <= min_efficacy {
                success = false;
                self.rhs = tmp_rhs;
            } else {
                min_mir_efficacy += efficacy;
                std::mem::swap(&mut tmp_rhs, &mut self.rhs);
            }
        }
        // Work on the saved row: the lifted one stays in the tmp arrays.
        std::mem::swap(&mut self.inds, &mut self.tmp_inds);
        std::mem::swap(&mut self.vals, &mut self.tmp_vals);
        if self.cmir_cut_generation_heuristic(min_mir_efficacy, only_initial_cmir_scale) {
            true
        } else if success {
            self.rhs = tmp_rhs;
            std::mem::swap(&mut self.complementation, &mut self.tmp_complementation);
            std::mem::swap(&mut self.solval, &mut self.tmp_solval);
            std::mem::swap(&mut self.inds, &mut self.tmp_inds);
            std::mem::swap(&mut self.vals, &mut self.tmp_vals);
            self.integral_support = save_integral_support;
            self.integral_coefficients = save_integral_coefficients;
            true
        } else {
            false
        }
    }
}
