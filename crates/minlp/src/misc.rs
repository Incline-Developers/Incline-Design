//! SCIP's miscellaneous methods (`misc.c`, `pub_misc.h`): hash functions,
//! the KISS random number generator, and the numeric helpers the rest of
//! the solver shares.

use crate::{cvec::CVec, def::max3};

/// `SCIPhashSignature64`.
#[inline(always)]
pub fn hash_signature64(a: u32) -> u64 {
    0x8000000000000000u64 >> (0x9e3779b9u32.wrapping_mul(a) >> 26)
}

/// `SCIPhashTwo`.
#[inline(always)]
pub fn hash_two(a: u32, b: u32) -> u32 {
    ((a as u64).wrapping_add(0xd37e9a1ce2148403).wrapping_mul((b as u64).wrapping_add(0xe5fcc163aef32782)) >> 32) as u32
}

/// `SCIPhashThree`.
#[inline(always)]
pub fn hash_three(a: u32, b: u32, c: u32) -> u32 {
    ((a as u64)
        .wrapping_add(0xbd5c89185f082658)
        .wrapping_mul((b as u64).wrapping_add(0xe5fcc163aef32782))
        .wrapping_add((c as u64).wrapping_mul(0xd37e9a1ce2148403))
        >> 32) as u32
}

/// `SCIPhashFour`.
#[inline(always)]
pub fn hash_four(a: u32, b: u32, c: u32, d: u32) -> u32 {
    ((a as u64)
        .wrapping_add(0xbd5c89185f082658)
        .wrapping_mul((b as u64).wrapping_add(0xe5fcc163aef32782))
        .wrapping_add((c as u64).wrapping_add(0xd37e9a1ce2148403).wrapping_mul((d as u64).wrapping_add(0x926f2d4dc4a67218)))
        >> 32) as u32
}

/// `SCIPhashFive`.
#[inline(always)]
pub fn hash_five(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 {
    ((a as u64)
        .wrapping_add(0xbd5c89185f082658)
        .wrapping_mul((b as u64).wrapping_add(0xe5fcc163aef32782))
        .wrapping_add((c as u64).wrapping_add(0xd37e9a1ce2148403).wrapping_mul((d as u64).wrapping_add(0x926f2d4dc4a67218)))
        .wrapping_add((e as u64).wrapping_mul(0xf48d4cd331e14327))
        >> 32) as u32
}

/// `SCIPhashSix`.
#[inline(always)]
pub fn hash_six(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32) -> u32 {
    ((a as u64)
        .wrapping_add(0xbd5c89185f082658)
        .wrapping_mul((b as u64).wrapping_add(0xe5fcc163aef32782))
        .wrapping_add((c as u64).wrapping_add(0xd37e9a1ce2148403).wrapping_mul((d as u64).wrapping_add(0x926f2d4dc4a67218)))
        .wrapping_add((e as u64).wrapping_add(0xf48d4cd331e14327).wrapping_mul((f as u64).wrapping_add(0x80791a4edfc44c75)))
        >> 32) as u32
}

/// `SCIPhashSeven`.
#[inline(always)]
pub fn hash_seven(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32) -> u32 {
    ((a as u64)
        .wrapping_add(0xbd5c89185f082658)
        .wrapping_mul((b as u64).wrapping_add(0xe5fcc163aef32782))
        .wrapping_add((c as u64).wrapping_add(0xd37e9a1ce2148403).wrapping_mul((d as u64).wrapping_add(0x926f2d4dc4a67218)))
        .wrapping_add((e as u64).wrapping_add(0xf48d4cd331e14327).wrapping_mul((f as u64).wrapping_add(0x80791a4edfc44c75)))
        .wrapping_add((g as u64).wrapping_mul(0x7f497d9ba3bd83c0))
        >> 32) as u32
}

/// C's `frexp`: `x = m * 2^e` with `0.5 <= |m| < 1`, or `(x, 0)` for zero,
/// infinities and NaN.
pub fn frexp(x: f64) -> (f64, i32) {
    if x == 0.0 || !x.is_finite() {
        return (x, 0);
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    if exp == 0 {
        // Subnormal: scale into the normal range first.
        let (m, e) = frexp(x * f64::from_bits(0x4350000000000000)); // 2^54
        return (m, e - 54);
    }
    let m = f64::from_bits((bits & !(0x7ffu64 << 52)) | (1022u64 << 52));
    (m, exp - 1022)
}

/// C's `ldexp`: `x * 2^n`, rounded once (musl's `scalbn`).
pub fn ldexp(x: f64, mut n: i32) -> f64 {
    let x1p1023 = f64::from_bits(0x7fe0000000000000); // 0x1p1023 === 2 ^ 1023
    let x1p53 = f64::from_bits(0x4340000000000000); // 0x1p53 === 2 ^ 53
    let x1p_1022 = f64::from_bits(0x0010000000000000); // 0x1p-1022 === 2 ^ (-1022)

    let mut y = x;

    if n > 1023 {
        y *= x1p1023;
        n -= 1023;
        if n > 1023 {
            y *= x1p1023;
            n -= 1023;
            if n > 1023 {
                n = 1023;
            }
        }
    } else if n < -1022 {
        // Make sure final n < -53 to avoid double rounding in the subnormal
        // range.
        y *= x1p_1022 * x1p53;
        n += 1022 - 53;
        if n < -1022 {
            y *= x1p_1022 * x1p53;
            n += 1022 - 53;
            if n < -1022 {
                n = -1022;
            }
        }
    }
    y * f64::from_bits(((0x3ff + n) as u64) << 52)
}

/// `SCIPrealHashCode`: a hash of a finite double from its sign, exponent
/// and 15 significant bits.
pub fn real_hash_code(x: f64) -> u32 {
    debug_assert!(x.is_finite());
    let (fraction, mut theexp) = frexp(x.abs());
    // `ldexp(fraction, 16)`, exact: the fraction lies in [0.5, 1).
    let mut mantissa = ((fraction * 65536.0) as u16).wrapping_add(1);
    if mantissa == 0 {
        mantissa = 1 << 14;
        theexp += 1;
    } else {
        mantissa >>= 1;
    }
    let thenum: i32 = if x < 0.0 { -(mantissa as i32) } else { mantissa as i32 };
    (((theexp as u16) as u32) << 16) | ((thenum as u16) as u32)
}

const DEFAULT_SEED: u32 = 123456789;
const DEFAULT_XOR: u32 = 362436000;
const DEFAULT_MWC: u32 = 521288629;
const DEFAULT_CST: u32 = 7654321;

/// `SCIP_RANDNUMGEN`: George Marsaglia's KISS generator, a linear
/// congruential, a xorshift and a multiply-with-carry generator summed.
#[derive(Clone, Debug)]
pub struct RandNumGen {
    seed: u32,
    xor_seed: u32,
    mwc_seed: u32,
    cst_seed: u32,
}

impl RandNumGen {
    /// `SCIPrandomCreate`, with the seed as given (`SCIPcreateRandom`
    /// first shifts it by `randomization/randomseedshift`).
    pub fn new(initseed: u32) -> Self {
        let mut randnumgen = Self {
            seed: 0,
            xor_seed: 0,
            mwc_seed: 0,
            cst_seed: 0,
        };
        randnumgen.set_seed(initseed);
        randnumgen
    }

    /// `SCIPrandomSetSeed`.
    pub fn set_seed(&mut self, initseed: u32) {
        self.seed = hash_two(DEFAULT_SEED, initseed).max(1);
        self.xor_seed = hash_two(DEFAULT_XOR, initseed).max(1);
        self.mwc_seed = hash_two(DEFAULT_MWC, initseed).max(1);
        self.cst_seed = hash_two(DEFAULT_CST, initseed);
    }

    /// `randomGetRand`: a number in `[0, u32::MAX]`.
    #[inline]
    fn get_rand(&mut self) -> u32 {
        self.seed = (self.seed as u64).wrapping_mul(1103515245).wrapping_add(12345) as u32;
        self.xor_seed ^= self.xor_seed << 13;
        self.xor_seed ^= self.xor_seed >> 17;
        self.xor_seed ^= self.xor_seed << 5;
        let t = 698769069u64.wrapping_mul(self.mwc_seed as u64).wrapping_add(self.cst_seed as u64);
        self.cst_seed = (t >> 32) as u32;
        self.mwc_seed = t as u32;
        self.seed.wrapping_add(self.xor_seed).wrapping_add(self.mwc_seed)
    }

    /// `SCIPrandomGetInt`: an integer in `[minrandval, maxrandval]`.
    pub fn get_int(&mut self, minrandval: i32, maxrandval: i32) -> i32 {
        let randnumber = self.get_rand() as f64 / (u32::MAX as f64 + 1.0);
        let zeromax = maxrandval as i64 - minrandval as i64 + 1;
        ((zeromax as f64 * randnumber) as i64 + minrandval as i64) as i32
    }

    /// `SCIPrandomGetReal`: a real in `[minrandval, maxrandval]`.
    pub fn get_real(&mut self, minrandval: f64, maxrandval: f64) -> f64 {
        let randnumber = self.get_rand() as f64 / u32::MAX as f64;
        minrandval * (1.0 - randnumber) + maxrandval * randnumber
    }

    /// `SCIPrandomPermuteIntArray` and `SCIPrandomPermuteArray`: a
    /// Fisher-Yates shuffle of `array[begin..end]`.
    pub fn permute<T>(&mut self, array: &mut [T], begin: usize, mut end: usize) {
        while end > begin + 1 {
            end -= 1;
            let i = self.get_int(begin as i32, end as i32) as usize;
            array.swap(i, end);
        }
    }

    /// `SCIPrandomGetSubset`: `nsubelems` distinct elements of `set`, drawn
    /// one at a time.
    pub fn get_subset<T: Copy + PartialEq>(&mut self, set: &[T], nsubelems: usize) -> Vec<T> {
        let nelems = set.len();
        if nelems == nsubelems {
            return set.to_vec();
        }
        assert!(nsubelems <= nelems, "cannot create a {nsubelems}-element subset of a {nelems}-element set");
        let mut subset: Vec<T> = Vec::with_capacity(nsubelems);
        while subset.len() < nsubelems {
            let r = self.get_int(0, nelems as i32 - 1) as usize;
            let drawn = set[r];
            if !subset.contains(&drawn) {
                subset.push(drawn);
            }
        }
        subset
    }
}

/// `SCIPcalcFibHash`.
#[inline(always)]
pub fn calc_fib_hash(v: f64) -> u32 {
    if v >= 0.0 {
        return (c_f64_to_u64(v * 2654435769.0) % u32::MAX as u64) as u32;
    }
    (c_f64_to_u64(-v * 683565275.0) % u32::MAX as u64) as u32
}

/// C's conversion of a double to `unsigned long long` as x86-64 GCC emits
/// it: exact below 2^63, offset by 2^63 above, and 2^63 for what does not
/// fit (where Rust's `as` would saturate).
#[inline(always)]
pub fn c_f64_to_u64(v: f64) -> u64 {
    const TWO63: f64 = 9223372036854775808.0;
    if v < TWO63 {
        c_f64_to_i64(v) as u64
    } else {
        (c_f64_to_i64(v - TWO63) as u64) ^ (1u64 << 63)
    }
}

/// C's conversion of a double to a 64-bit integer as `cvttsd2si` does it:
/// truncation, and `i64::MIN` for NaN and what does not fit.
#[inline(always)]
pub fn c_f64_to_i64(v: f64) -> i64 {
    if v.is_nan() || v >= 9223372036854775808.0 || v < -9223372036854775808.0 {
        i64::MIN
    } else {
        v as i64
    }
}

/// C's conversion of a double to `int` as `cvttsd2si` does it.
#[inline(always)]
pub fn c_f64_to_i32(v: f64) -> i32 {
    if v.is_nan() || v >= 2147483648.0 || v <= -2147483649.0 { i32::MIN } else { v as i32 }
}

/// `SCIPrelDiff`: `(val1 - val2) / max(|val1|, |val2|, 1)`.
#[inline]
pub fn rel_diff(val1: f64, val2: f64) -> f64 {
    let absval1 = val1.abs();
    let absval2 = val2.abs();
    let quot = max3(1.0, absval1, absval2);
    (val1 - val2) / quot
}

/// `SCIPcalcGreComDiv`: the greatest common divisor of two positive
/// integers, by the binary method.
pub fn calc_gre_com_div(mut val1: i64, mut val2: i64) -> i64 {
    debug_assert!(val1 > 0 && val2 > 0);
    let mut t = 0;
    while (val1 & 1) == 0 {
        val1 >>= 1;
        if (val2 & 1) == 0 {
            val2 >>= 1;
            t += 1;
        } else {
            while (val1 & 1) == 0 {
                val1 >>= 1;
            }
            break;
        }
    }
    while (val2 & 1) == 0 {
        val2 >>= 1;
    }
    if t == 0 {
        if val1 > val2 {
            val1 -= val2;
            while (val1 & 1) == 0 {
                val1 >>= 1;
            }
        } else if val1 < val2 {
            val2 -= val1;
            while (val2 & 1) == 0 {
                val2 >>= 1;
            }
        }
    }
    while val1 != val2 {
        if val1 > val2 {
            if val2 == 1 {
                return val2 << t;
            }
            if ((val1 ^ val2) & 2) == 2 {
                val1 += val2;
            } else {
                val1 -= val2;
            }
            debug_assert!((val1 & 3) == 0);
            val1 >>= 2;
            while (val1 & 1) == 0 {
                val1 >>= 1;
            }
        } else {
            if val1 == 1 {
                return val1 << t;
            }
            if ((val2 ^ val1) & 2) == 2 {
                val2 += val1;
            } else {
                val2 -= val1;
            }
            debug_assert!((val2 & 3) == 0);
            val2 >>= 2;
            while (val2 & 1) == 0 {
                val2 >>= 1;
            }
        }
    }
    val1 << t
}

/// `SCIPcalcSmaComMul`.
pub fn calc_sma_com_mul(val1: i64, val2: i64) -> i64 {
    debug_assert!(val1 > 0 && val2 > 0);
    let gcd = calc_gre_com_div(val1, val2);
    val1 / gcd * val2
}

/// `SCIP_REGRESSION`: a running linear regression of Y on X.
#[derive(Clone, Debug)]
pub struct Regression {
    pub intercept: f64,
    pub slope: f64,
    pub meanx: f64,
    pub meany: f64,
    pub sumxy: f64,
    pub variancesumx: f64,
    pub variancesumy: f64,
    pub corrcoef: f64,
    pub nobservations: i32,
}

impl Default for Regression {
    fn default() -> Self {
        let mut regression = Self {
            intercept: 0.0,
            slope: 0.0,
            meanx: 0.0,
            meany: 0.0,
            sumxy: 0.0,
            variancesumx: 0.0,
            variancesumy: 0.0,
            corrcoef: 0.0,
            nobservations: 0,
        };
        regression.reset();
        regression
    }
}

/// `incrementalStatsUpdate`.
fn incremental_stats_update(value: f64, meanptr: &mut f64, sumvarptr: &mut f64, nobservations: i32, add: bool) {
    let addfactor = if add { 1.0 } else { -1.0 };
    let oldmean = *meanptr;
    *meanptr = oldmean + addfactor * (value - oldmean) / nobservations as f64;
    *sumvarptr += addfactor * (value - oldmean) * (value - *meanptr);
    *sumvarptr = crate::def::max(0.0, *sumvarptr);
}

impl Regression {
    /// `regressionRecompute`.
    fn recompute(&mut self) {
        use crate::def::{INVALID, eps_z};
        if self.nobservations <= 1 || eps_z(self.variancesumx, 1e-9) {
            self.slope = INVALID;
            self.intercept = INVALID;
            self.corrcoef = INVALID;
        } else if eps_z(self.variancesumy, 1e-9) {
            self.slope = 0.0;
            self.corrcoef = 0.0;
            self.intercept = self.meany;
        } else {
            self.slope = (self.sumxy - self.nobservations as f64 * self.meanx * self.meany) / self.variancesumx;
            self.intercept = self.meany - self.slope * self.meanx;
            self.corrcoef = (self.sumxy - self.nobservations as f64 * self.meanx * self.meany) / (self.variancesumx * self.variancesumy).sqrt();
        }
    }

    /// `SCIPregressionRemoveObservation`.
    pub fn remove_observation(&mut self, x: f64, y: f64) {
        if self.nobservations == 1 {
            self.reset();
        } else {
            self.nobservations -= 1;
            incremental_stats_update(x, &mut self.meanx, &mut self.variancesumx, self.nobservations, false);
            incremental_stats_update(y, &mut self.meany, &mut self.variancesumy, self.nobservations, false);
            self.sumxy -= x * y;
        }
        self.recompute();
    }

    /// `SCIPregressionAddObservation`.
    pub fn add_observation(&mut self, x: f64, y: f64) {
        self.nobservations += 1;
        incremental_stats_update(x, &mut self.meanx, &mut self.variancesumx, self.nobservations, true);
        incremental_stats_update(y, &mut self.meany, &mut self.variancesumy, self.nobservations, true);
        self.sumxy += x * y;
        self.recompute();
    }

    /// `SCIPregressionReset`.
    pub fn reset(&mut self) {
        use crate::def::INVALID;
        self.intercept = INVALID;
        self.slope = INVALID;
        self.corrcoef = INVALID;
        self.meanx = 0.0;
        self.variancesumx = 0.0;
        self.sumxy = 0.0;
        self.meany = 0.0;
        self.variancesumy = 0.0;
        self.nobservations = 0;
    }
}

/// `SCIP_REALARRAY`/`SCIP_BOOLARRAY`/`SCIP_INTARRAY`: an array over
/// non-negative indices whose untouched entries read as zero, tracking the
/// first and last nonzero index as C does. C keeps the values in a window
/// it shifts and grows; that layout is never observable, so it is not kept.
#[derive(Clone, Debug)]
pub struct DynArray<T> {
    vals: Vec<T>,
    pub minusedidx: i32,
    pub maxusedidx: i32,
}

impl<T: Copy + Default + PartialEq> Default for DynArray<T> {
    fn default() -> Self {
        Self {
            vals: Vec::new(),
            minusedidx: i32::MAX,
            maxusedidx: i32::MIN,
        }
    }
}

impl<T: Copy + Default + PartialEq> DynArray<T> {
    /// `SCIPrealarrayCreate`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `SCIPrealarrayExtend`: only reserves room.
    pub fn extend(&mut self, minidx: i32, maxidx: i32) {
        let _ = minidx;
        if maxidx >= 0 && self.vals.len() <= maxidx as usize {
            self.vals.resize(maxidx as usize + 1, T::default());
        }
    }

    /// `SCIPrealarrayClear`.
    pub fn clear(&mut self) {
        if self.minusedidx <= self.maxusedidx {
            for i in self.minusedidx..=self.maxusedidx {
                self.vals[i as usize] = T::default();
            }
            self.minusedidx = i32::MAX;
            self.maxusedidx = i32::MIN;
        }
    }

    /// `SCIPrealarrayGetVal`.
    #[inline]
    pub fn get_val(&self, idx: i32) -> T {
        if idx < self.minusedidx || idx > self.maxusedidx {
            T::default()
        } else {
            self.vals[idx as usize]
        }
    }

    /// `SCIPrealarraySetVal`.
    pub fn set_val(&mut self, idx: i32, val: T) {
        if val != T::default() {
            self.extend(idx, idx);
            self.vals[idx as usize] = val;
            self.minusedidx = self.minusedidx.min(idx);
            self.maxusedidx = self.maxusedidx.max(idx);
        } else if idx >= 0 && (idx as usize) < self.vals.len() {
            self.vals[idx as usize] = T::default();
            if idx == self.minusedidx {
                loop {
                    self.minusedidx += 1;
                    if !(self.minusedidx <= self.maxusedidx && self.vals[self.minusedidx as usize] == T::default()) {
                        break;
                    }
                }
                if self.minusedidx > self.maxusedidx {
                    self.minusedidx = i32::MAX;
                    self.maxusedidx = i32::MIN;
                }
            } else if idx == self.maxusedidx {
                loop {
                    self.maxusedidx -= 1;
                    if self.vals[self.maxusedidx as usize] != T::default() {
                        break;
                    }
                }
            }
        }
    }

    /// `SCIPrealarrayGetMinIdx`.
    pub fn get_min_idx(&self) -> i32 {
        self.minusedidx
    }

    /// `SCIPrealarrayGetMaxIdx`.
    pub fn get_max_idx(&self) -> i32 {
        self.maxusedidx
    }
}

impl DynArray<f64> {
    /// `SCIPrealarrayIncVal`.
    pub fn inc_val(&mut self, idx: i32, incval: f64) {
        let oldval = self.get_val(idx);
        if oldval != crate::def::INVALID {
            self.set_val(idx, oldval + incval);
        }
    }
}

pub type RealArray = DynArray<f64>;
pub type BoolArray = DynArray<bool>;
pub type IntArray = DynArray<i32>;

/// `SCIP_DISJOINTSET`: union-find with path compression and union by size.
#[derive(Clone, Debug, Default)]
pub struct DisjointSet {
    pub parents: CVec<i32>,
    pub sizes: CVec<i32>,
    pub size: i32,
    pub componentcount: i32,
}

impl DisjointSet {
    /// `SCIPdisjointsetCreate`.
    pub fn new(ncomponents: i32) -> Self {
        let mut djset = DisjointSet {
            parents: CVec::with_size(ncomponents),
            sizes: CVec::with_size(ncomponents),
            size: ncomponents,
            componentcount: 0,
        };
        djset.clear();
        djset
    }

    /// `SCIPdisjointsetClear`.
    pub fn clear(&mut self) {
        self.componentcount = self.size;
        for i in 0..self.componentcount {
            self.parents[i] = i;
            self.sizes[i] = 1;
        }
    }

    /// `SCIPdisjointsetFind`.
    pub fn find(&mut self, mut element: i32) -> i32 {
        let mut root = element;
        while root != self.parents[root] {
            root = self.parents[root];
        }
        while element != root {
            let newelement = self.parents[element];
            self.parents[element] = root;
            element = newelement;
        }
        root
    }

    /// `SCIPdisjointsetUnion`.
    pub fn union(&mut self, p: i32, q: i32, forcerepofp: bool) {
        let idp = self.find(p);
        let idq = self.find(q);
        if idp == idq {
            return;
        }
        if forcerepofp || self.sizes[idp] >= self.sizes[idq] {
            self.parents[idq] = idp;
            self.sizes[idp] += self.sizes[idq];
        } else {
            self.parents[idp] = idq;
            self.sizes[idq] += self.sizes[idp];
        }
        self.componentcount -= 1;
    }

    /// `SCIPdisjointsetGetComponentCount`.
    pub fn component_count(&self) -> i32 {
        self.componentcount
    }

    /// `SCIPdisjointsetGetSize`.
    pub fn get_size(&self) -> i32 {
        self.size
    }
}

/// Simple denominators tried first by [`real_to_rational`] (with their
/// powers of ten).
const SIMPLEDNOMS: [f64; 19] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 25.0];

/// `SCIPrealToRational`: converts a real number into a (approximate)
/// rational representation, and returns TRUE iff the conversion was
/// successful.
pub fn real_to_rational(val: f64, mindelta: f64, maxdelta: f64, maxdnom: i64, numerator: &mut i64, denominator: &mut i64) -> bool {
    // Try the simple denominators first: each value of the simpledenoms
    // table multiplied by powers of 10 is tried as denominator.
    if val.abs() >= (i64::MAX as f64) / maxdnom as f64 {
        return false;
    }
    for &simple in &SIMPLEDNOMS {
        let mut dnom = simple;
        while dnom <= maxdnom as f64 {
            let nom = (val * dnom).floor();
            let ratval0 = nom / dnom;
            let ratval1 = (nom + 1.0) / dnom;
            if mindelta <= val - ratval0 && val - ratval1 <= maxdelta {
                if val - ratval0 <= maxdelta {
                    *numerator = nom as i64;
                    *denominator = dnom as i64;
                    return true;
                }
                if mindelta <= val - ratval1 {
                    *numerator = (nom + 1.0) as i64;
                    *denominator = dnom as i64;
                    return true;
                }
            }
            dnom *= 10.0;
        }
    }

    // The simple denominators didn't work: calculate rational
    // representation with arbitrary denominator.
    let epsilon = crate::def::min(-mindelta, maxdelta) / 2.0;
    let mut b = val;
    let mut a = crate::def::eps_floor(b, epsilon);
    let mut g0 = a;
    let mut h0 = 1.0;
    let mut g1 = 1.0;
    let mut h1 = 0.0;
    let mut delta0 = val - g0 / h0;
    let mut delta1 = if delta0 < 0.0 { val - (g0 - 1.0) / h0 } else { val - (g0 + 1.0) / h0 };
    while (delta0 < mindelta || delta0 > maxdelta) && (delta1 < mindelta || delta1 > maxdelta) {
        b = 1.0 / (b - a);
        a = crate::def::eps_floor(b, epsilon);
        let gx = g0;
        let hx = h0;
        g0 = a * g0 + g1;
        h0 = a * h0 + h1;
        g1 = gx;
        h1 = hx;
        if h0 > maxdnom as f64 {
            return false;
        }
        delta0 = val - g0 / h0;
        delta1 = if delta0 < 0.0 { val - (g0 - 1.0) / h0 } else { val - (g0 + 1.0) / h0 };
    }
    if g0.abs() > (i64::MAX >> 4) as f64 || h0 > (i64::MAX >> 4) as f64 {
        return false;
    }
    if delta0 < mindelta {
        *numerator = (g0 - 1.0) as i64;
        *denominator = h0 as i64;
    } else if delta0 > maxdelta {
        *numerator = (g0 + 1.0) as i64;
        *denominator = h0 as i64;
    } else {
        *numerator = g0 as i64;
        *denominator = h0 as i64;
    }
    true
}

/// Critical values of Student's t distribution for the confidence levels
/// (columns) and degrees of freedom 1 to 15 (rows).
const STUDENTT_QUARTILES: [f64; 75] = [
    1.000, 2.414, 3.078, 6.314, 12.706, // 1
    0.816, 1.604, 1.886, 2.920, 4.303, // 2
    0.765, 1.423, 1.638, 2.353, 3.182, // 3
    0.741, 1.344, 1.533, 2.132, 2.776, // 4
    0.727, 1.301, 1.476, 2.015, 2.571, // 5
    0.718, 1.273, 1.440, 1.943, 2.447, // 6
    0.711, 1.254, 1.415, 1.895, 2.365, // 7
    0.706, 1.240, 1.397, 1.860, 2.306, // 8
    0.703, 1.230, 1.383, 1.833, 2.262, // 9
    0.700, 1.221, 1.372, 1.812, 2.228, // 10
    0.697, 1.214, 1.363, 1.796, 2.201, // 11
    0.695, 1.209, 1.356, 1.782, 2.179, // 12
    0.694, 1.204, 1.350, 1.771, 2.160, // 13
    0.692, 1.200, 1.345, 1.761, 2.145, // 14
    0.691, 1.197, 1.341, 1.753, 2.131, // 15
];

/// Critical values for higher degrees of freedom (the standard normal
/// distribution's).
const STUDENTT_QUARTILESABOVE: [f64; 5] = [0.674, 1.150, 1.282, 1.645, 1.960];

/// The maximum degrees of freedom represented before switching to normal
/// approximation.
const STUDENTT_MAXDF: i32 = 15;

/// `SCIPstudentTGetCriticalValue`.
pub fn student_t_get_critical_value(clevel: crate::types::ConfidenceLevel, df: i32) -> f64 {
    if df > STUDENTT_MAXDF {
        STUDENTT_QUARTILESABOVE[clevel as usize]
    } else {
        STUDENTT_QUARTILES[clevel as usize + 5 * (df - 1) as usize]
    }
}

/// `SCIPcomputeTwoSampleTTestValue`: the test statistic for two samples'
/// means, or `SCIP_INVALID` with fewer than two observations in either.
pub fn compute_two_sample_t_test_value(meanx: f64, meany: f64, variancex: f64, variancey: f64, countx: f64, county: f64) -> f64 {
    if countx < 1.9 || county < 1.9 {
        return crate::def::INVALID;
    }

    // Pooled variance is the weighted average of the two variances.
    let mut pooledvariance = (countx - 1.0) * variancex + (county - 1.0) * variancey;
    pooledvariance /= countx + county - 2.0;

    // A variance close to zero means the distributions are basically
    // constant.
    pooledvariance = crate::def::max(pooledvariance, 1e-9);

    // Tresult can be understood as realization of a Student-T distributed
    // variable with countx + county - 2 degrees of freedom.
    let mut tresult = (meanx - meany) / pooledvariance.sqrt();
    tresult *= (countx * county / (countx + county)).sqrt();
    tresult
}

#[cfg(not(target_arch = "wasm32"))]
unsafe extern "C" {
    /// The C library's `erf`, as SCIP calls it.
    #[link_name = "erf"]
    fn c_erf(x: f64) -> f64;
}

/// `SCIPerf`: the C library's error function. The browser has no C
/// library; there it is `libm`'s, a port of musl's, which can differ from
/// glibc's (correctly rounded since 2.40) in the last bit.
pub fn erf(x: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    // SAFETY: `erf` is a pure function of its argument.
    unsafe {
        c_erf(x)
    }
    #[cfg(target_arch = "wasm32")]
    libm::erf(x)
}

/// `SCIPnormalGetCriticalValue`.
pub fn normal_get_critical_value(clevel: crate::types::ConfidenceLevel) -> f64 {
    STUDENTT_QUARTILESABOVE[clevel as usize]
}

/// `SCIPnormalCDF`: the value of the normal distribution's cumulative
/// distribution function at `value`.
pub fn normal_cdf(mean: f64, variance: f64, value: f64) -> f64 {
    // We need to calculate the standard deviation from the variance.
    let std = if variance < 1e-9 { 0.0 } else { variance.sqrt() };

    // Special treatment for zero variance.
    if std < 1e-9 {
        return if value < mean + 1e-9 { 1.0 } else { 0.0 };
    }
    let normvalue = (value - mean) / (std * std::f64::consts::SQRT_2);

    // Calculate the cumulative distribution function for normvalue.
    if normvalue < 1e-9 && normvalue > -1e-9 {
        0.5
    } else if normvalue > 0.0 {
        erf(normvalue) / 2.0 + 0.5
    } else {
        0.5 - erf(-normvalue) / 2.0
    }
}

/// `isIntegralScalar` (misc.c): whether `val * scalar` is integral within
/// the relative deltas.
fn is_integral_scalar(val: f64, scalar: f64, mindelta: f64, maxdelta: f64) -> bool {
    let sval = val * scalar;
    let downval = sval.floor();
    let upval = sval.ceil();
    rel_diff(sval, downval) <= maxdelta || rel_diff(sval, upval) >= mindelta
}

/// Scalars tried by `SCIPcalcIntegralScalar` besides powers of two.
const SCALARS: [f64; 9] = [3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0, 17.0, 19.0];

/// `SCIPcalcIntegralScalar`: tries to find a value, such that all given
/// values, if scaled with this value become integral in relative allowed
/// difference in between mindelta and maxdelta; returns the scalar if one
/// was found.
pub fn calc_integral_scalar(vals: &[f64], mindelta: f64, maxdelta: f64, maxdnom: i64, maxscale: f64) -> Option<f64> {
    // Get minimal absolute non-zero value.
    let mut minval = f64::MAX;
    for &val in vals {
        if val < mindelta || val > maxdelta {
            minval = crate::def::min(minval, val.abs());
        }
    }
    if minval == f64::MAX {
        // All coefficients are zero (inside tolerances).
        return Some(1.0);
    }

    // Try, if values can be made integral multiplying them with the
    // reciprocal of the smallest value and a power of 2.
    let mut bestscalar = crate::def::INVALID;
    for i in 0..2 {
        let mut scalable = true;

        // Try, if values can be made integral by multiplying them with the
        // reciprocal of the smallest value and a power of 2, or by
        // multiplying them with a power of 2.
        let mut scaleval = if i == 0 { 1.0 / minval } else { 1.0 };
        let mut c = 0;
        while c < vals.len() && scalable {
            let val = vals[c];
            c += 1;
            if val == 0.0 {
                continue;
            }
            let absval = val.abs();
            while scaleval <= maxscale && (absval * scaleval < 0.5 || !is_integral_scalar(val, scaleval, mindelta, maxdelta)) {
                match SCALARS.iter().find(|&&s| is_integral_scalar(val, scaleval * s, mindelta, maxdelta)) {
                    Some(&s) => scaleval *= s,
                    None => scaleval *= 2.0,
                }
            }
            scalable = scaleval <= maxscale;
        }
        if scalable {
            // Make values integral by dividing them by the smallest value
            // (and multiplying them with a power of 2).
            if scaleval < bestscalar {
                bestscalar = scaleval;
            }

            // If the scalar is the reciprocal of the smallest value, it is
            // also the best one.
            if i == 0 && crate::def::eps_eq(scaleval, 1.0 / minval, crate::def::DEFAULT_EPSILON) {
                return Some(bestscalar);
            }
        }
    }

    // Convert each value into a rational number, calculate the greatest
    // common divisor of the nominators and the smallest common multiple of
    // the denominators.
    let mut gcd: i64 = 1;
    let mut scm: i64 = 1;
    let mut rational = true;

    // First value (to initialize gcd).
    let mut c = 0;
    while c < vals.len() && rational {
        let val = vals[c];
        if val != 0.0 {
            let (mut numerator, mut denominator) = (0, 0);
            rational = real_to_rational(val, mindelta, maxdelta, maxdnom, &mut numerator, &mut denominator);
            if rational && numerator != 0 {
                gcd = numerator.abs();
                scm = denominator;
                rational = (scm as f64) / (gcd as f64) <= maxscale;
                break;
            }
        }
        c += 1;
    }

    // Remaining values.
    c += 1;
    while c < vals.len() && rational {
        let val = vals[c];
        if val != 0.0 {
            let (mut numerator, mut denominator) = (0, 0);
            rational = real_to_rational(val, mindelta, maxdelta, maxdnom, &mut numerator, &mut denominator);
            if rational && numerator != 0 {
                gcd = calc_gre_com_div(gcd, numerator.abs());
                scm *= denominator / calc_gre_com_div(scm, denominator);
                rational = (scm as f64) / (gcd as f64) <= maxscale;
            }
        }
        c += 1;
    }
    if rational && (scm as f64) / (gcd as f64) < bestscalar {
        // Make values integral by multiplying them with the smallest common
        // multiple of the denominators.
        bestscalar = (scm as f64) / (gcd as f64);
    }
    if bestscalar < crate::def::INVALID { Some(bestscalar) } else { None }
}

/// `SCIPcomputeGap`: computes the gap from the primal and the dual bound.
pub fn compute_gap(eps: f64, inf: f64, primalbound: f64, dualbound: f64) -> f64 {
    if crate::def::eps_eq(primalbound, dualbound, eps) {
        0.0
    } else {
        let absdual = dualbound.abs();
        let absprimal = primalbound.abs();

        if crate::def::eps_z(dualbound, eps) || crate::def::eps_z(primalbound, eps) || absprimal >= inf || absdual >= inf || primalbound * dualbound < 0.0 {
            inf
        } else {
            ((primalbound - dualbound) / crate::def::min(absdual, absprimal)).abs()
        }
    }
}

/// `a - b` rounded towards negative infinity (C sets the FPU rounding mode
/// downwards around the subtraction): the nearest difference, stepped down
/// when its exact error (TwoSum) is negative.
pub fn sub_round_down(a: f64, b: f64) -> f64 {
    let s = a - b;
    if !s.is_finite() {
        return s;
    }
    let nb = -b;
    let bb = s - a;
    let e = (a - (s - bb)) + (nb - bb);
    if e < 0.0 { s.next_down() } else { s }
}

/// `SCIPfindSimpleRational`: checks, whether the given interval contains a
/// rational number with a denominator of at most `maxdnom`.
pub fn find_simple_rational(lb: f64, ub: f64, maxdnom: i64, numerator: &mut i64, denominator: &mut i64) -> bool {
    let center = 0.5 * (lb + ub);

    // In order to prevent numerical difficulties we use a downward rounded
    // delta.
    let delta = 0.5 * sub_round_down(ub, lb);

    real_to_rational(center, -delta, delta, maxdnom, numerator, denominator)
}

/// `SCIPselectSimpleValue`: given a (usually very small) interval, selects
/// a value inside this interval; it is tried to select a rational number
/// with a small denominator.
pub fn select_simple_value(lb: f64, ub: f64, maxdnom: i64) -> f64 {
    let mut val = 0.5 * (lb + ub);
    if lb < ub {
        let mut numerator = 0;
        let mut denominator = 0;

        // Try to find a "simple" rational number inside the interval.
        let success = find_simple_rational(lb, ub, maxdnom, &mut numerator, &mut denominator);
        if success {
            val = numerator as f64 / denominator as f64;

            // Due to numerical reasons the value might lie outside the
            // interval.
            if val - lb < 0.0 || val - ub > 0.0 {
                val = 0.5 * (lb + ub);
            }
        }
    }
    val
}

/// `SCIPnextafter`: the next representable floating point number after
/// `from` in the direction of `to` (C's `nextafter`).
pub fn nextafter(from: f64, to: f64) -> f64 {
    if from.is_nan() || to.is_nan() {
        f64::NAN
    } else if from == to {
        to
    } else if from < to {
        from.next_up()
    } else {
        from.next_down()
    }
}

/// `SCIP_DIGRAPH`: a directed graph with successor lists in insertion order
/// and, once computed, its undirected components. Arc data and articulation
/// points are not ported.
#[derive(Clone, Debug, Default)]
pub struct Digraph {
    successors: Vec<Vec<i32>>,
    components: Vec<i32>,
    componentstarts: Vec<i32>,
    ncomponents: i32,
}

impl Digraph {
    /// `SCIPdigraphCreate`.
    pub fn new(nnodes: i32) -> Self {
        Self {
            successors: vec![Vec::new(); nnodes as usize],
            components: Vec::new(),
            componentstarts: Vec::new(),
            ncomponents: 0,
        }
    }

    /// `SCIPdigraphSetSizes`: reserves the successor lists.
    pub fn set_sizes(&mut self, sizes: &[i32]) {
        for (s, &size) in self.successors.iter_mut().zip(sizes) {
            s.clear();
            s.reserve(size as usize);
        }
    }

    /// `SCIPdigraphAddArc`.
    pub fn add_arc(&mut self, startnode: i32, endnode: i32) {
        self.successors[startnode as usize].push(endnode);
    }

    /// `depthFirstSearch`: appends the nodes reachable from `startnode` to
    /// `dfsnodes`, in reverse dfs order.
    fn depth_first_search(&self, startnode: i32, visited: &mut [bool], dfsstack: &mut [i32], stackadjvisited: &mut [usize], dfsnodes: &mut Vec<i32>) {
        // Put start node on the stack.
        dfsstack[0] = startnode;
        stackadjvisited[0] = 0;
        let mut stackidx: isize = 0;

        while stackidx >= 0 {
            // Get next node from stack.
            let currnode = dfsstack[stackidx as usize] as usize;
            let mut sadv = stackadjvisited[stackidx as usize];

            // Mark current node as visited.
            visited[currnode] = true;

            // Iterate through the successor list until we reach unhandled node.
            let succ = &self.successors[currnode];
            while sadv < succ.len() && visited[succ[sadv] as usize] {
                sadv += 1;
            }

            if sadv == succ.len() {
                // The current node was completely handled, remove it from
                // stack and store it in the sorted nodes array.
                stackidx -= 1;
                dfsnodes.push(currnode as i32);
            } else {
                // Store current stackadjvisted index and put the successor node
                // onto the stack.
                stackadjvisited[stackidx as usize] = sadv + 1;
                stackidx += 1;
                dfsstack[stackidx as usize] = succ[sadv];
                stackadjvisited[stackidx as usize] = 0;
            }
        }
    }

    /// `SCIPdigraphComputeUndirectedComponents`: computes the components of
    /// the undirected graph (arcs taken in both directions) with at least
    /// `minsize` nodes, storing each node's component in `components`;
    /// returns their number.
    pub fn compute_undirected_components(&mut self, minsize: i32, mut components: Option<&mut [i32]>) -> i32 {
        let nnodes = self.successors.len();
        self.ncomponents = 0;
        self.components = Vec::with_capacity(nnodes);
        self.componentstarts = vec![0];

        let mut visited = vec![false; nnodes];
        let mut dfsstack = vec![0i32; nnodes];
        let mut stackadjvisited = vec![0usize; nnodes];

        // Store the number of directed arcs per node and add reverse arcs to
        // the graph.
        let ndirectedsuccessors: Vec<usize> = self.successors.iter().map(Vec::len).collect();
        for i in (0..nnodes).rev() {
            for j in 0..ndirectedsuccessors[i] {
                let s = self.successors[i][j];
                self.add_arc(s, i as i32);
            }
        }

        let mut dfsnodes = Vec::new();
        for v in 0..nnodes {
            if visited[v] {
                continue;
            }

            let compstart = self.components.len();
            dfsnodes.clear();
            self.depth_first_search(v as i32, &mut visited, &mut dfsstack, &mut stackadjvisited, &mut dfsnodes);

            // Forget about this component if it is too small.
            if dfsnodes.len() as i32 >= minsize {
                self.components.extend_from_slice(&dfsnodes);
                self.ncomponents += 1;
                self.componentstarts.push((compstart + dfsnodes.len()) as i32);

                // Store component number for contained nodes if array was
                // given.
                if let Some(components) = components.as_deref_mut() {
                    for i in (compstart..self.components.len()).rev() {
                        components[self.components[i] as usize] = self.ncomponents - 1;
                    }
                }
            }
        }

        // Restore the number of directed arcs per node.
        for (s, &n) in self.successors.iter_mut().zip(&ndirectedsuccessors) {
            s.truncate(n);
        }

        self.ncomponents
    }

    /// `SCIPdigraphGetNComponents`.
    pub fn get_n_components(&self) -> i32 {
        self.ncomponents
    }

    /// `SCIPdigraphGetComponent`: the nodes of component `compidx`.
    pub fn get_component(&self, compidx: i32) -> &[i32] {
        let c = compidx as usize;
        &self.components[self.componentstarts[c] as usize..self.componentstarts[c + 1] as usize]
    }

    /// `SCIPdigraphGetNNodes`.
    pub fn get_n_nodes(&self) -> i32 {
        self.successors.len() as i32
    }

    /// `SCIPdigraphTopoSortComponents`: performs an (almost) topological sort
    /// on the undirected components, computed before by
    /// `compute_undirected_components`.
    pub fn topo_sort_components(&mut self) {
        let nnodes = self.successors.len();
        let mut visited = vec![false; nnodes];
        let mut dfsnodes = Vec::with_capacity(nnodes);
        let mut dfsstack = vec![0i32; nnodes];
        let mut stackadjvisited = vec![0usize; nnodes];

        // Sort the components (almost) topologically.
        for i in 0..self.ncomponents as usize {
            let start = self.componentstarts[i] as usize;
            let end = self.componentstarts[i + 1] as usize;
            let endidx = end - 1;
            dfsnodes.clear();
            for j in start..end {
                let node = self.components[j];
                if visited[node as usize] {
                    continue;
                }

                // Perform depth first search, nodes visited in this call are
                // appended to the list dfsnodes in reverse dfs order, after the
                // nodes already contained; so at every point in time, the nodes
                // in dfsnode are in reverse (almost) topological order.
                self.depth_first_search(node, &mut visited, &mut dfsstack, &mut stackadjvisited, &mut dfsnodes);
            }
            debug_assert_eq!(dfsnodes.len(), end - start);

            // Copy reverse (almost) topologically sorted array of nodes reached
            // by the dfs searches; reverse their order to get an (almost)
            // topologically sort.
            for (k, &node) in dfsnodes.iter().enumerate() {
                self.components[endidx - k] = node;
            }
        }
    }

    /// `tarjan`: performs Tarjan's algorithm for a given directed graph to
    /// obtain the strongly connected components which are reachable from a
    /// given node.
    #[allow(clippy::too_many_arguments)]
    fn tarjan(
        &self,
        v: usize,
        lowlink: &mut [i32],
        dfsidx: &mut [i32],
        stack: &mut [i32],
        stacksize: &mut usize,
        unprocessed: &mut [bool],
        nodeinstack: &mut [bool],
        maxdfs: &mut i32,
        strongcomponents: &mut [i32],
        nstrongcomponents: &mut i32,
        strongcompstartidx: &mut [i32],
        nstorednodes: &mut i32,
    ) {
        dfsidx[v] = *maxdfs;
        lowlink[v] = *maxdfs;
        *maxdfs += 1;

        // Add v to the stack.
        stack[*stacksize] = v as i32;
        *stacksize += 1;
        nodeinstack[v] = true;

        // Mark v as processed.
        unprocessed[v] = false;

        for i in 0..self.successors[v].len() {
            // Edge (v,w).
            let w = self.successors[v][i] as usize;

            if unprocessed[w] {
                self.tarjan(
                    w,
                    lowlink,
                    dfsidx,
                    stack,
                    stacksize,
                    unprocessed,
                    nodeinstack,
                    maxdfs,
                    strongcomponents,
                    nstrongcomponents,
                    strongcompstartidx,
                    nstorednodes,
                );

                // Update lowlink.
                lowlink[v] = if lowlink[v] <= lowlink[w] { lowlink[v] } else { lowlink[w] };
            } else if nodeinstack[w] {
                // Update lowlink.
                lowlink[v] = if lowlink[v] <= dfsidx[w] { lowlink[v] } else { dfsidx[w] };
            }
        }

        // Found a root of a strong component.
        if lowlink[v] == dfsidx[v] {
            strongcompstartidx[*nstrongcomponents as usize] = *nstorednodes;
            *nstrongcomponents += 1;

            loop {
                // stack.pop()
                let w = stack[*stacksize - 1];
                *stacksize -= 1;
                nodeinstack[w as usize] = false;

                // Store the node in the corresponding component.
                strongcomponents[*nstorednodes as usize] = w;
                *nstorednodes += 1;

                if v as i32 == w {
                    break;
                }
            }
        }
    }

    /// `SCIPdigraphComputeDirectedComponents`: computes all strongly
    /// connected components of an undirected connected component with
    /// Tarjan's Algorithm, sorted topologically starting from the end of the
    /// `strongcomponents` array; returns their number.
    pub fn compute_directed_components(&self, compidx: i32, strongcomponents: &mut [i32], strongcompstartidx: &mut [i32]) -> i32 {
        let nnodes = self.successors.len();
        let mut lowlink = vec![-1i32; nnodes];
        let mut dfsidx = vec![-1i32; nnodes];
        let mut stack = vec![-1i32; nnodes];
        let mut unprocessed = vec![true; nnodes];
        let mut nodeinstack = vec![false; nnodes];

        let mut nstorednodes = 0;
        let mut stacksize = 0;
        let mut maxdfs = 0;
        let mut nstrongcomponents = 0;

        // Iterate over all nodes in the undirected connected component.
        let c = compidx as usize;
        for i in self.componentstarts[c] as usize..self.componentstarts[c + 1] as usize {
            let v = self.components[i] as usize;

            // Call Tarjan's algorithm for unprocessed nodes.
            if unprocessed[v] {
                self.tarjan(
                    v,
                    &mut lowlink,
                    &mut dfsidx,
                    &mut stack,
                    &mut stacksize,
                    &mut unprocessed,
                    &mut nodeinstack,
                    &mut maxdfs,
                    strongcomponents,
                    &mut nstrongcomponents,
                    strongcompstartidx,
                    &mut nstorednodes,
                );
            }
        }

        // To simplify the iteration over all strongly connected components.
        strongcompstartidx[nstrongcomponents as usize] = nstorednodes;

        nstrongcomponents
    }
}

/// C's `isspace` in the "C" locale.
#[inline]
pub fn c_isspace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `SCIP_SPACECONTROL`: control specifier for escaped spaces.
pub const SPACECONTROL: &[u8] = b" tnvfr";

/// `SCIPskipSpace`: skips leading blanks and escaped spaces of `s` from
/// position `pos`; returns the new position.
pub fn skip_space(s: &[u8], mut pos: usize) -> usize {
    let at = |p: usize| s.get(p).copied().unwrap_or(0);
    while c_isspace(at(pos)) || (at(pos) == b'\\' && at(pos + 1) != 0 && SPACECONTROL.contains(&at(pos + 1))) {
        pos += if at(pos) == b'\\' { 2 } else { 1 };
    }
    pos
}

/// `strtod` on `s` from position `pos`: the value and the end position, or
/// `None` if no number starts there (as `endptr == str`). Decimal numbers,
/// `inf`/`infinity` and `nan` (any case) with an optional sign, after
/// optional white space; hexadecimal floats are not ported.
pub fn strtod(s: &[u8], pos: usize) -> Option<(f64, usize)> {
    let at = |p: usize| s.get(p).copied().unwrap_or(0);
    let mut p = pos;
    while c_isspace(at(p)) {
        p += 1;
    }
    let start = p;
    if at(p) == b'+' || at(p) == b'-' {
        p += 1;
    }

    // inf, infinity, nan.
    let rest = &s[p.min(s.len())..];
    let lower: Vec<u8> = rest.iter().take(8).map(|c| c.to_ascii_lowercase()).collect();
    if lower.starts_with(b"inf") {
        let end = if lower.starts_with(b"infinity") { p + 8 } else { p + 3 };
        let neg = at(start) == b'-';
        return Some((if neg { f64::NEG_INFINITY } else { f64::INFINITY }, end));
    }
    if lower.starts_with(b"nan") {
        let mut end = p + 3;
        // nan(n-char-sequence)
        if at(end) == b'(' {
            let mut q = end + 1;
            while at(q).is_ascii_alphanumeric() || at(q) == b'_' {
                q += 1;
            }
            if at(q) == b')' {
                end = q + 1;
            }
        }
        return Some((f64::NAN, end));
    }
    assert!(!(at(p) == b'0' && (at(p + 1) == b'x' || at(p + 1) == b'X')), "hexadecimal floats are not ported");

    let mut ndigits = 0;
    while at(p).is_ascii_digit() {
        p += 1;
        ndigits += 1;
    }
    if at(p) == b'.' {
        p += 1;
        while at(p).is_ascii_digit() {
            p += 1;
            ndigits += 1;
        }
    }
    if ndigits == 0 {
        return None;
    }
    // An exponent only counts with at least one digit.
    if at(p) == b'e' || at(p) == b'E' {
        let mut q = p + 1;
        if at(q) == b'+' || at(q) == b'-' {
            q += 1;
        }
        if at(q).is_ascii_digit() {
            while at(q).is_ascii_digit() {
                q += 1;
            }
            p = q;
        }
    }
    let text = std::str::from_utf8(&s[start..p]).unwrap();
    // Rust's parser is correctly rounded, as glibc's strtod is.
    let value: f64 = text.parse().unwrap();
    Some((value, p))
}

/// `SCIPstrToRealValue`: extract the next real value of the given string;
/// the value and the final position, or `None` (then the position stays).
pub fn str_to_real_value(s: &[u8], pos: usize) -> Option<(f64, usize)> {
    strtod(s, pos)
}

/// `SCIPstrCopySection`: extracts a section of the string from `pos`
/// between `startchar` and `endchar`; returns the token and the end
/// position (`pos` itself if the section was not found).
pub fn str_copy_section(s: &[u8], pos: usize, startchar: u8, endchar: u8, size: usize) -> (String, usize) {
    let at = |p: usize| s.get(p).copied().unwrap_or(0);
    let copystr = pos;
    let mut p = pos;
    let mut token = Vec::new();

    // Find starting character.
    while at(p) != 0 && at(p) != startchar {
        p += 1;
    }

    // Did not find start character.
    if at(p) == 0 {
        return (String::new(), copystr);
    }

    // Skip start character.
    p += 1;

    // Copy string.
    while at(p) != 0 && at(p) != endchar && token.len() < size - 1 {
        token.push(at(p));
        p += 1;
    }

    // If section was longer than size, we want to reach the end of the parsing
    // section anyway.
    if token.len() == size - 1 {
        while at(p) != 0 && at(p) != endchar {
            p += 1;
        }
    }

    let token = String::from_utf8(token).unwrap();

    // Did not find end character.
    if at(p) == 0 {
        return (token, copystr);
    }

    // Skip end character.
    p += 1;

    (token, p)
}

/// C's `printf("%g", x)`: six significant digits, `%e` style if the
/// exponent is below -4 or at least 6, trailing zeros removed.
pub fn fmt_g(x: f64) -> String {
    fmt_g_prec(x, 6)
}

/// C's `printf("%.<prec>g", x)`: `prec` significant digits, `%e` style if
/// the exponent is below -4 or at least `prec`, trailing zeros removed.
pub fn fmt_g_prec(x: f64, prec: usize) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan".into() } else { "nan".into() };
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf".into() } else { "inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }

    // The exponent after rounding to `prec` significant digits.
    let prec = prec.max(1);
    let e = format!("{x:.*e}", prec - 1);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();

    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };

    if exp < -4 || exp >= prec as i32 {
        format!("{}e{}{:02}", strip(mant), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        let decimals = (prec as i32 - 1 - exp).max(0) as usize;
        strip(&format!("{x:.decimals$}"))
    }
}
