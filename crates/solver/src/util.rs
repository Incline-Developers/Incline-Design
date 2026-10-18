//! Small pieces of HiGHS the solver leans on: compensated arithmetic, its
//! random number generator and basis hash, and an indexed set.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// HiGHS's `HighsCDouble`: a double-double, the sum `hi + lo`, for sums
/// that must not lose their small terms.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CDouble {
    hi: f64,
    lo: f64,
}

impl CDouble {
    pub fn new(value: f64) -> Self {
        Self { hi: value, lo: 0.0 }
    }

    pub fn value(self) -> f64 {
        self.hi + self.lo
    }

    fn two_sum(a: f64, b: f64) -> (f64, f64) {
        let x = a + b;
        let z = x - a;
        (x, (a - (x - z)) + (b - z))
    }

    fn split(a: f64) -> (f64, f64) {
        const FACTOR: f64 = ((1u64 << 27) + 1) as f64;
        let c = FACTOR * a;
        let x = c - (c - a);
        (x, a - x)
    }

    fn two_product(a: f64, b: f64) -> (f64, f64) {
        let x = a * b;
        let (a1, a2) = Self::split(a);
        let (b1, b2) = Self::split(b);
        (x, a2 * b2 - (((x - a1 * b1) - a2 * b1) - a1 * b2))
    }
}

/// HiGHS's friend `operator+(double, const HighsCDouble&)`: `b + a`.
impl Add<CDouble> for f64 {
    type Output = CDouble;
    fn add(self, b: CDouble) -> CDouble {
        b + self
    }
}

/// HiGHS's friend `operator-(double, const HighsCDouble&)`: `-b + a`.
impl Sub<CDouble> for f64 {
    type Output = CDouble;
    fn sub(self, b: CDouble) -> CDouble {
        -b + self
    }
}

/// HiGHS's friend `operator*(double, const HighsCDouble&)`: `b * a`.
impl Mul<CDouble> for f64 {
    type Output = CDouble;
    fn mul(self, b: CDouble) -> CDouble {
        b * self
    }
}

/// HiGHS's friend `operator/(double, const HighsCDouble&)`.
impl Div<CDouble> for f64 {
    type Output = CDouble;
    fn div(self, b: CDouble) -> CDouble {
        CDouble::new(self) / b
    }
}

impl PartialEq<CDouble> for f64 {
    fn eq(&self, other: &CDouble) -> bool {
        *self == other.value()
    }
}

impl PartialOrd<CDouble> for f64 {
    fn partial_cmp(&self, other: &CDouble) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&other.value())
    }
}

impl From<f64> for CDouble {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl AddAssign<f64> for CDouble {
    fn add_assign(&mut self, v: f64) {
        let (hi, c) = Self::two_sum(v, self.hi);
        self.hi = hi;
        self.lo += c;
    }
}

impl AddAssign<CDouble> for CDouble {
    fn add_assign(&mut self, v: CDouble) {
        *self += v.hi;
        self.lo += v.lo;
    }
}

impl Add<f64> for CDouble {
    type Output = CDouble;
    fn add(self, v: f64) -> CDouble {
        let (hi, lo) = Self::two_sum(self.hi, v);
        CDouble { hi, lo: lo + self.lo }
    }
}

impl Add<CDouble> for CDouble {
    type Output = CDouble;
    fn add(self, v: CDouble) -> CDouble {
        let mut res = self + v.hi;
        res.lo += v.lo;
        res
    }
}

impl Sub<CDouble> for CDouble {
    type Output = CDouble;
    fn sub(self, v: CDouble) -> CDouble {
        let mut res = self + -v.hi;
        res.lo -= v.lo;
        res
    }
}

impl Mul<f64> for CDouble {
    type Output = CDouble;
    fn mul(self, v: f64) -> CDouble {
        let (hi, lo) = Self::two_product(self.hi, v);
        let mut res = CDouble { hi, lo };
        res += self.lo * v;
        res
    }
}

impl Neg for CDouble {
    type Output = CDouble;
    fn neg(self) -> CDouble {
        CDouble { hi: -self.hi, lo: -self.lo }
    }
}

impl Sub<f64> for CDouble {
    type Output = CDouble;
    fn sub(self, v: f64) -> CDouble {
        let (hi, lo) = Self::two_sum(self.hi, -v);
        CDouble { hi, lo: lo + self.lo }
    }
}

impl SubAssign<f64> for CDouble {
    fn sub_assign(&mut self, v: f64) {
        *self += -v;
    }
}

impl SubAssign<CDouble> for CDouble {
    fn sub_assign(&mut self, v: CDouble) {
        *self -= v.hi;
        self.lo -= v.lo;
    }
}

impl MulAssign<f64> for CDouble {
    fn mul_assign(&mut self, v: f64) {
        let c = self.lo * v;
        let (hi, lo) = Self::two_product(self.hi, v);
        self.hi = hi;
        self.lo = lo;
        *self += c;
    }
}

impl Mul<CDouble> for CDouble {
    type Output = CDouble;
    fn mul(self, v: CDouble) -> CDouble {
        let mut res = self * v.hi;
        res += self.hi * v.lo;
        res
    }
}

impl DivAssign<f64> for CDouble {
    fn div_assign(&mut self, v: f64) {
        let d = CDouble { hi: self.hi / v, lo: self.lo / v };
        let mut c = d * v - *self;
        c.hi /= v;
        c.lo /= v;
        *self = d - c;
    }
}

impl DivAssign<CDouble> for CDouble {
    fn div_assign(&mut self, v: CDouble) {
        let vdbl = v.hi + v.lo;
        let d = CDouble {
            hi: self.hi / vdbl,
            lo: self.lo / vdbl,
        };
        let mut c = d * v - *self;
        c.hi /= vdbl;
        c.lo /= vdbl;
        *self = d - c;
    }
}

impl Div<f64> for CDouble {
    type Output = CDouble;
    fn div(mut self, v: f64) -> CDouble {
        self /= v;
        self
    }
}

impl Div<CDouble> for CDouble {
    type Output = CDouble;
    fn div(mut self, v: CDouble) -> CDouble {
        self /= v;
        self
    }
}

impl CDouble {
    /// HiGHS's `abs(HighsCDouble)`.
    pub fn abs(self) -> CDouble {
        if self.value() < 0.0 { -self } else { self }
    }

    /// HiGHS's `renormalize`.
    pub fn renormalize(&mut self) {
        (self.hi, self.lo) = Self::two_sum(self.hi, self.lo);
    }

    /// HiGHS's `sqrt(HighsCDouble)`.
    pub fn sqrt(self) -> CDouble {
        let c = (self.hi + self.lo).sqrt();
        if c == 0.0 {
            return CDouble::new(0.0);
        }
        let mut res = self / c;
        res += c;
        res.hi *= 0.5;
        res.lo *= 0.5;
        res
    }

    /// HiGHS's `floor(HighsCDouble)`.
    pub fn floor(self) -> CDouble {
        if self.abs().value() < 1.0 {
            return CDouble::new(if self.value() == 0.0 || self.value() > 0.0 { 0.0 } else { -1.0 });
        }
        let floor_x = self.value().floor();
        let (hi, lo) = Self::two_sum(floor_x, (self - floor_x).value().floor());
        CDouble { hi, lo }
    }

    /// HiGHS's `ceil(HighsCDouble)`.
    pub fn ceil(self) -> CDouble {
        if self.abs().value() < 1.0 {
            return CDouble::new(if self.value() == 0.0 || self.value() < 0.0 { 0.0 } else { 1.0 });
        }
        let ceil_x = self.value().ceil();
        let (hi, lo) = Self::two_sum(ceil_x, (self - ceil_x).value().ceil());
        CDouble { hi, lo }
    }

    /// HiGHS's `ldexp(HighsCDouble, exp)`.
    pub fn ldexp(self, exp: i32) -> CDouble {
        CDouble {
            hi: ldexp(self.hi, exp),
            lo: ldexp(self.lo, exp),
        }
    }
}

/// `std::ldexp`: `x` times two to the `exp`.
pub(crate) fn ldexp(mut x: f64, mut exp: i32) -> f64 {
    while exp > 1000 {
        x *= 2f64.powi(1000);
        exp -= 1000;
    }
    while exp < -1000 {
        x *= 2f64.powi(-1000);
        exp += 1000;
    }
    x * 2f64.powi(exp)
}

/// HiGHS compares a `HighsCDouble` by its value, `hi + lo`.
impl PartialEq for CDouble {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
    }
}

impl PartialOrd for CDouble {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value().partial_cmp(&other.value())
    }
}

impl PartialEq<f64> for CDouble {
    fn eq(&self, other: &f64) -> bool {
        self.value() == *other
    }
}

impl PartialOrd<f64> for CDouble {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.value().partial_cmp(other)
    }
}

/// HiGHS's `HighsHashHelpers`, as far as the solver uses it.
pub(crate) mod hash {
    pub const C: [u64; 64] = [
        0xc8497d2a400d9551,
        0x80c8963be3e4c2f3,
        0x042d8680e260ae5b,
        0x8a183895eeac1536,
        0xa94e9c75f80ad6de,
        0x7e92251dec62835e,
        0x07294165cb671455,
        0x89b0f6212b0a4292,
        0x31900011b96bf554,
        0xa44540f8eee2094f,
        0xce7ffd372e4c64fc,
        0x51c9d471bfe6a10f,
        0x758c2a674483826f,
        0xf91a20abe63f8b02,
        0xc2a069024a1fcc6f,
        0xd5bb18b70c5dbd59,
        0xd510adac6d1ae289,
        0x571d069b23050a79,
        0x60873b8872933e06,
        0x780481cc19670350,
        0x7a48551760216885,
        0xb5d68b918231e6ca,
        0xa7e5571699aa5274,
        0x7b6d309b2cfdcf01,
        0x04e77c3d474daeff,
        0x4dbf099fd7247031,
        0x5d70dca901130beb,
        0x9f8b5f0df4182499,
        0x293a74c9686092da,
        0xd09bdab6840f52b3,
        0xc05d47f3ab302263,
        0x6b79e62b884b65d6,
        0xa581106fc980c34d,
        0xf081b7145ea2293e,
        0xfb27243dd7c3f5ad,
        0x5211bf8860ea667f,
        0x9455e65cb2385e7f,
        0x0dfaf6731b449b33,
        0x4ec98b3c6f5e68c7,
        0x007bfd4a42ae936b,
        0x65c93061f8674518,
        0x640816f17127c5d1,
        0x6dd4bab17b7c3a74,
        0x34d9268c256fa1ba,
        0x0b4d0c6b5b50d7f4,
        0x30aa965bc9fadaff,
        0xc0ac1d0c2771404d,
        0xc5e64509abb76ef2,
        0xd606b11990624a36,
        0x0d3f05d242ce2fb7,
        0x469a803cb276fe32,
        0xa4a44d177a3e23f4,
        0xb9d9a120dcc1ca03,
        0x2e15af8165234a2e,
        0x10609ba2720573d4,
        0xaa4191b60368d1d5,
        0x333dd2300bc57762,
        0xdf6ec48f79fb402f,
        0x5ed20fcef1b734fa,
        0x4c94924ec8be21ee,
        0x5abe6ad9d131e631,
        0xbe10136a522e602d,
        0x53671115c340e779,
        0x9f392fe43e2144da,
    ];

    pub const M61: u64 = 0x1fffffffffffffff;

    pub fn pair_hash(k: usize, a: u32, b: u32) -> u64 {
        (a as u64).wrapping_add(C[2 * k]).wrapping_mul((b as u64).wrapping_add(C[2 * k + 1]))
    }

    pub fn log2i(n: u32) -> u32 {
        31 - n.leading_zeros()
    }

    pub fn multiply_mod_m61(a: u64, b: u64) -> u64 {
        let ahi = a >> 32;
        let bhi = b >> 32;
        let alo = a & 0xffffffff;
        let blo = b & 0xffffffff;
        let term_64 = ahi.wrapping_mul(bhi);
        let term_32 = ahi.wrapping_mul(blo).wrapping_add(bhi.wrapping_mul(alo));
        let mut term_0 = alo.wrapping_mul(blo);
        term_0 = (term_0 & M61).wrapping_add(term_0 >> 61);
        term_0 = term_0.wrapping_add(((term_32 >> 29).wrapping_add(term_32 << 32)) & M61);
        let ab61 = (term_64 << 3) | (term_0 >> 61);
        let mut result = (term_0 & M61).wrapping_add(ab61);
        if result >= M61 {
            result -= M61;
        }
        result
    }

    pub fn modexp_m61(a: u64, mut e: u64) -> u64 {
        let mut result = a;
        while e != 1 {
            result = multiply_mod_m61(result, result);
            if e & 1 != 0 {
                result = multiply_mod_m61(result, a);
            }
            e >>= 1;
        }
        result
    }

    /// Add `index` to the hash of a set.
    pub fn sparse_combine(hash: &mut u64, index: usize) {
        let a = C[index & 63] & M61;
        let degree = (index as u64 >> 6) + 1;
        *hash = hash.wrapping_add(modexp_m61(a, degree));
        *hash = (*hash >> 61) + (*hash & M61);
        if *hash >= M61 {
            *hash -= M61;
        }
    }

    /// HiGHS's `sparse_combine` with a value: add `index`'s entry `value`
    /// to the hash of a sparse vector.
    pub fn sparse_combine_value(hash: &mut u64, index: usize, value: u64) {
        let value = ((value << 1) & M61) | 1;
        let a = C[index & 63] & M61;
        let degree = (index as u64 >> 6) + 1;
        *hash = hash.wrapping_add(multiply_mod_m61(value, modexp_m61(a, degree)));
        *hash = (*hash >> 61) + (*hash & M61);
        if *hash >= M61 {
            *hash -= M61;
        }
    }

    /// HiGHS's `double_hash_code`: a hash on which values equal to about
    /// 15 bits of mantissa agree.
    pub fn double_hash_code(value: f64) -> u32 {
        let (hashbits, exponent) = super::frexp(value * 0.618_033_988_749_894_8);
        let high = exponent as i16 as u16 as u32;
        let low = (hashbits * 32768.0) as i16 as u16 as u32;
        (high << 16) | low
    }

    /// Take `index` out of the hash of a set.
    pub fn sparse_inverse_combine(hash: &mut u64, index: usize) {
        let a = C[index & 63] & M61;
        let degree = (index as u64 >> 6) + 1;
        *hash = hash.wrapping_add(M61 - modexp_m61(a, degree));
        *hash = (*hash >> 61) + (*hash & M61);
        if *hash >= M61 {
            *hash -= M61;
        }
    }
}

/// HiGHS's `HighsRandom`, so random choices match its.
#[derive(Clone, Debug)]
pub(crate) struct Random {
    state: u64,
}

impl Random {
    pub fn new(seed: u32) -> Self {
        let mut random = Self { state: 0 };
        random.initialise(seed);
        random
    }

    pub fn initialise(&mut self, seed: u32) {
        self.state = seed as u64;
        loop {
            self.state = hash::pair_hash(0, self.state as u32, (self.state >> 32) as u32);
            self.state ^= hash::pair_hash(1, (self.state >> 32) as u32, seed) >> 32;
            if self.state != 0 {
                break;
            }
        }
    }

    fn advance(&mut self) {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
    }

    fn draw_uniform(&mut self, sup: u32, nbits: u32) -> u32 {
        loop {
            self.advance();
            let lo = self.state as u32;
            let hi = (self.state >> 32) as u32;
            // HiGHS tries the hashes 0-7 then 9-31: 8 is skipped.
            for n in (0..8).chain(9..32) {
                let k = n;
                // pair_hash<k> uses constants 2k and 2k+1, which exist for
                // k < 32.
                let val = (hash::pair_hash(k, lo, hi) >> (64 - nbits)) as u32;
                if val < sup {
                    return val;
                }
            }
        }
    }

    /// A random integer in `[0, sup)`.
    pub fn integer(&mut self, sup: usize) -> usize {
        if sup <= 1 {
            return 0;
        }
        let nbits = hash::log2i((sup - 1) as u32) + 1;
        self.draw_uniform(sup as u32, nbits) as usize
    }

    /// A random fraction in `(0, 1)`.
    pub fn fraction(&mut self) -> f64 {
        self.advance();
        let lo = self.state as u32;
        let hi = (self.state >> 32) as u32;
        let output = (hash::pair_hash(0, lo, hi) >> (64 - 52)) ^ (hash::pair_hash(1, lo, hi) >> (64 - 26));
        (1 + output) as f64 * 2.2204460492503125e-16
    }

    /// A random integer in `[0, 2^31)`.
    pub fn integer_any(&mut self) -> usize {
        self.advance();
        (hash::pair_hash(0, self.state as u32, (self.state >> 32) as u32) >> 33) as usize
    }

    /// A random fraction in `[0, 1]`.
    pub fn closed_fraction(&mut self) -> f64 {
        self.advance();
        let lo = self.state as u32;
        let hi = (self.state >> 32) as u32;
        let output = (hash::pair_hash(0, lo, hi) >> (64 - 53)) ^ (hash::pair_hash(1, lo, hi) >> 32);
        output as f64 * 1.1102230246251566e-16
    }

    /// A random real in `[a, b]`.
    pub fn real(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.closed_fraction()
    }

    pub fn bit(&mut self) -> bool {
        self.advance();
        hash::pair_hash(0, self.state as u32, (self.state >> 32) as u32) >> 63 != 0
    }

    pub fn shuffle<T>(&mut self, data: &mut [T]) {
        for i in (2..=data.len()).rev() {
            let pos = self.integer(i);
            data.swap(pos, i - 1);
        }
    }
}

/// HiGHS's `HSet`: a set of indices with O(1) membership, insertion and
/// removal, listed in insertion order (removal moves the last entry in).
#[derive(Clone, Debug, Default)]
pub(crate) struct HSet {
    entry: Vec<usize>,
    pointer: Vec<usize>,
    count: usize,
}

impl HSet {
    const NO_POINTER: usize = usize::MAX;

    pub fn setup(&mut self, size: usize, max_entry: usize) {
        self.entry = Vec::with_capacity(size);
        self.pointer = vec![Self::NO_POINTER; max_entry + 1];
        self.count = 0;
    }

    pub fn clear(&mut self) {
        self.pointer.fill(Self::NO_POINTER);
        self.entry.clear();
        self.count = 0;
    }

    pub fn add(&mut self, entry: usize) -> bool {
        if entry >= self.pointer.len() {
            self.pointer.resize(entry + 1, Self::NO_POINTER);
        } else if self.pointer[entry] != Self::NO_POINTER {
            return false;
        }
        self.pointer[entry] = self.count;
        if self.count == self.entry.len() {
            self.entry.push(entry);
        } else {
            self.entry[self.count] = entry;
        }
        self.count += 1;
        true
    }

    pub fn remove(&mut self, entry: usize) -> bool {
        if entry >= self.pointer.len() {
            return false;
        }
        let pointer = self.pointer[entry];
        if pointer == Self::NO_POINTER {
            return false;
        }
        self.pointer[entry] = Self::NO_POINTER;
        if pointer + 1 < self.count {
            let last_entry = self.entry[self.count - 1];
            self.entry[pointer] = last_entry;
            self.pointer[last_entry] = pointer;
        }
        self.count -= 1;
        true
    }

    pub fn entries(&self) -> &[usize] {
        &self.entry[..self.count]
    }
}

/// The power of two nearest to `1 / value`, in the sense HiGHS uses to
/// scale vectors: for `value` in `[2^(e-1), 2^e)`, `2^-e`... as `frexp`.
pub(crate) fn nearest_power_of_two_scale(value: f64) -> f64 {
    let (mut fraction, mut exponent) = frexp(value);
    if fraction.abs() == 0.5 {
        fraction *= 2.0;
        exponent -= 1;
    }
    let _ = fraction;
    2f64.powi(-exponent)
}

/// C's `frexp`: `value = fraction * 2^exponent`, `|fraction|` in `[0.5, 1)`.
pub(crate) fn frexp(value: f64) -> (f64, i32) {
    if value == 0.0 || !value.is_finite() {
        return (value, 0);
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    if exponent == 0 {
        // Subnormal: normalise first.
        let (fraction, exponent) = frexp(value * 2f64.powi(64));
        return (fraction, exponent - 64);
    }
    let fraction = f64::from_bits((bits & !(0x7ff << 52)) | (1022 << 52));
    (fraction, exponent - 1022)
}

/// `std::frexp`'s exponent: `x = m 2^e` with `0.5 <= |m| < 1`, and 0 for
/// 0, infinities and NaN.
pub(crate) fn frexp_exp(x: f64) -> i32 {
    if x == 0.0 || !x.is_finite() {
        return 0;
    }
    let bits = x.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    if exponent == 0 {
        // Subnormal: scale by 2^54 into the normal range first.
        return frexp_exp(x * f64::from_bits(0x4350_0000_0000_0000)) - 54;
    }
    exponent - 1022
}

/// Debug tracing (`SOLVER_TRACE_PRESOLVE`): print each bound change,
/// reduction, LP and heuristic for diffing against an instrumented HiGHS.
/// Read from the environment once.
pub(crate) fn tracing() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SOLVER_TRACE_PRESOLVE").is_some())
}
