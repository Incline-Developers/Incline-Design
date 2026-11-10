//! SCIP's double-double arithmetic (`dbldblarith.h`), on in the reference
//! build: a value is `hi + lo`. Each function is one of the C macros, with
//! the same operations in the same order.

/// `QUAD_EPSILON`: smaller epsilon value for use with quadprecision.
pub const QUAD_EPSILON: f64 = 1e-12;

/// A double-double value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    pub hi: f64,
    pub lo: f64,
}

impl Quad {
    /// `QUAD_ASSIGN`.
    #[inline(always)]
    pub fn new(constant: f64) -> Self {
        Quad { hi: constant, lo: 0.0 }
    }

    /// `QUAD_TO_DBL`.
    #[inline(always)]
    pub fn to_dbl(self) -> f64 {
        self.hi + self.lo
    }

    /// `QUAD_SCALE`.
    #[inline(always)]
    pub fn scale(&mut self, a: f64) {
        self.hi *= a;
        self.lo *= a;
    }
}

/// `__SCIPdbldblSplit`.
#[inline(always)]
fn split(x: f64) -> (f64, f64) {
    let tmp = 134217729.0 * x;
    let rhi = tmp - (tmp - x);
    let rlo = x - rhi;
    (rhi, rlo)
}

/// `SCIPdbldblProd`: multiply two floating point numbers, both given by
/// one double, and return the result as two doubles.
#[inline(always)]
pub fn prod_dd(a: f64, b: f64) -> Quad {
    let (ahi, alo) = split(a);
    let (bhi, blo) = split(b);
    let rhi = a * b;
    let rlo = alo * blo - (((rhi - ahi * bhi) - alo * bhi) - ahi * blo);
    Quad { hi: rhi, lo: rlo }
}

/// `SCIPdbldblSquare`: square a floating point number given by one double
/// and return the result as two doubles.
#[inline(always)]
pub fn square_d(a: f64) -> Quad {
    let (ahi, alo) = split(a);
    let rhi = a * a;
    let rlo = alo * alo - ((rhi - ahi * ahi) - 2.0 * alo * ahi);
    Quad { hi: rhi, lo: rlo }
}

/// `SCIPdbldblSum`: add two floating point numbers, both given by one
/// double, and return the result as two doubles.
#[inline(always)]
pub fn sum_dd(a: f64, b: f64) -> Quad {
    let tmp2 = a + b;
    let tmp1 = tmp2 - a;
    let rlo = (a - (tmp2 - tmp1)) + (b - tmp1);
    Quad { hi: tmp2, lo: rlo }
}

/// `SCIPdbldblSum21`: add two floating point numbers, the first is given
/// by two doubles, the second is given by one double, and return the
/// result as two doubles.
#[inline(always)]
pub fn sum_qd(a: Quad, b: f64) -> Quad {
    let t = sum_dd(a.hi, b);
    Quad { hi: t.hi, lo: t.lo + a.lo }
}

/// `SCIPdbldblProd21`: multiply two floating point numbers, the first is
/// given by two doubles, the second is given by one double, and return the
/// result as two doubles.
#[inline(always)]
pub fn prod_qd(a: Quad, b: f64) -> Quad {
    let t = prod_dd(a.hi, b);
    Quad { hi: t.hi, lo: a.lo * b + t.lo }
}

/// `SCIPdbldblDiv`: divide two floating point numbers, both given by one
/// double, and return the result as two doubles.
#[inline(always)]
pub fn div_dd(a: f64, b: f64) -> Quad {
    let estim = a / b;
    let mut t = prod_dd(b, estim);
    t = sum_qd(t, -a);
    t.hi /= b;
    t.lo /= b;
    sum_qd(Quad { hi: -t.hi, lo: -t.lo }, estim)
}

/// `SCIPdbldblDiv12`: divide two floating point numbers, the first is
/// given by one double, the second is given by two doubles, and return the
/// result as two doubles.
#[inline(always)]
pub fn div_dq(a: f64, b: Quad) -> Quad {
    let estim = a / b.hi;
    let mut t = prod_qd(b, estim);
    t = sum_qd(t, -a);
    t.hi /= b.hi;
    t.lo /= b.hi;
    sum_qd(Quad { hi: -t.hi, lo: -t.lo }, estim)
}

/// `SCIPdbldblSum22`: add two floating point numbers, both given by two
/// doubles, and return the result as two doubles.
#[inline(always)]
pub fn sum_qq(a: Quad, b: Quad) -> Quad {
    let t = sum_qd(a, b.hi);
    sum_qd(t, b.lo)
}

/// `SCIPdbldblDiv21`: divide two floating point numbers, the first is
/// given by two doubles, the second is given by one double, and return the
/// result as two doubles.
#[inline(always)]
pub fn div_qd(a: Quad, b: f64) -> Quad {
    let estim = Quad { hi: a.hi / b, lo: a.lo / b };
    let mut t = prod_qd(estim, b);
    t = sum_qq(t, Quad { hi: -a.hi, lo: -a.lo });
    t.hi /= b;
    t.lo /= b;
    sum_qq(estim, Quad { hi: -t.hi, lo: -t.lo })
}

/// `SCIPdbldblProd22`: multiply two floating point numbers, both given by
/// two doubles, and return the result as two doubles.
#[inline(always)]
pub fn prod_qq(a: Quad, b: Quad) -> Quad {
    let mut t = prod_dd(a.hi, b.hi);
    t = sum_qd(t, a.lo * b.hi);
    sum_qd(t, a.hi * b.lo)
}

/// `SCIPdbldblSquare2`: square a floating point number given by two
/// doubles and return the result as two doubles.
#[inline(always)]
pub fn square_q(a: Quad) -> Quad {
    let t = square_d(a.hi);
    sum_qd(t, 2.0 * a.hi * a.lo)
}

/// `SCIPdbldblDiv22`: divide two floating point numbers, both given by two
/// doubles, and return the result as two doubles.
#[inline(always)]
pub fn div_qq(a: Quad, b: Quad) -> Quad {
    let estim = Quad { hi: a.hi / b.hi, lo: a.lo / b.hi };
    let mut t = prod_qq(b, estim);
    t = sum_qq(t, Quad { hi: -a.hi, lo: -a.lo });
    t.hi /= b.hi;
    t.lo /= b.hi;
    sum_qq(estim, Quad { hi: -t.hi, lo: -t.lo })
}

/// `SCIPdbldblSqrt`: take the square root of a floating point number given
/// by one double and return the result as two doubles.
#[inline(always)]
pub fn sqrt_d(a: f64) -> Quad {
    let estim = a.sqrt();
    if estim != 0.0 {
        let mut r = div_dd(a, estim);
        r = sum_qd(r, estim);
        r.hi *= 0.5;
        r.lo *= 0.5;
        r
    } else {
        Quad { hi: 0.0, lo: 0.0 }
    }
}

/// `SCIPdbldblSqrt2`: take the square root of a floating point number
/// given by two doubles and return the result as two doubles.
#[inline(always)]
pub fn sqrt_q(a: Quad) -> Quad {
    let estim = (a.hi + a.lo).sqrt();
    if estim != 0.0 {
        let mut r = div_qd(a, estim);
        r = sum_qd(r, estim);
        r.hi *= 0.5;
        r.lo *= 0.5;
        r
    } else {
        Quad { hi: 0.0, lo: 0.0 }
    }
}

/// `SCIPdbldblAbs2`: compute the absolute value of the floating point
/// number given by two doubles.
#[inline(always)]
pub fn abs_q(a: Quad) -> Quad {
    if a.hi < 0.0 { Quad { hi: -a.hi, lo: -a.lo } } else { a }
}

/// `SCIPdbldblFloor2`: compute the floored value of the floating point
/// number given by two doubles.
#[inline(always)]
pub fn floor_q(a: Quad) -> Quad {
    let tmp = (a.hi + a.lo).floor();
    let r = sum_qd(a, -tmp);
    if (r.hi - 1.0) + r.lo < 0.0 && r.hi + r.lo >= 0.0 {
        // Floor in double precision was fine.
        Quad { hi: tmp, lo: 0.0 }
    } else {
        // Floor in double precision needs to be corrected.
        let tmp2 = (r.hi + r.lo).floor();
        sum_dd(tmp, tmp2)
    }
}

/// `SCIPdbldblCeil2`: compute the ceiled value of the floating point
/// number given by two doubles.
#[inline(always)]
pub fn ceil_q(a: Quad) -> Quad {
    let tmp = (a.hi + a.lo).ceil();
    let r = sum_qd(Quad { hi: -a.hi, lo: -a.lo }, tmp);
    if (r.hi - 1.0) + r.lo < 0.0 && r.hi + r.lo >= 0.0 {
        // Ceil in double precision was fine.
        Quad { hi: tmp, lo: 0.0 }
    } else {
        // Ceil in double precision needs to be corrected.
        let tmp2 = (r.hi + r.lo).floor();
        sum_dd(tmp, -tmp2)
    }
}

/// `SCIPdbldblEpsFloor2`: compute the floored value of the floating point
/// number given by two doubles, add epsilon first for safety.
#[inline(always)]
pub fn eps_floor_q(a: Quad, eps: f64) -> Quad {
    floor_q(Quad { hi: a.hi, lo: a.lo + eps })
}

/// `SCIPdbldblEpsCeil2`: compute the ceiled value of the floating point
/// number given by two doubles, subtract epsilon first for safety.
#[inline(always)]
pub fn eps_ceil_q(a: Quad, eps: f64) -> Quad {
    ceil_q(Quad { hi: a.hi, lo: a.lo - eps })
}

impl std::ops::Neg for Quad {
    type Output = Quad;
    /// `-x` on a quad macro argument negates both halves.
    #[inline(always)]
    fn neg(self) -> Quad {
        Quad { hi: -self.hi, lo: -self.lo }
    }
}
