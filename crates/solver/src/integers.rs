//! HiGHS's `HighsIntegers`: integer helpers for the MIP solver, above all
//! `integralScale`, the smallest multiplier making a set of values integral.

use crate::util::{CDouble, frexp};

/// `a mod m`, never negative.
pub(crate) fn mod_i64(a: i64, m: i64) -> i64 {
    let r = a % m;
    r + i64::from(r < 0) * m
}

/// HiGHS's `mod(double, double)`.
pub(crate) fn mod_f64(a: f64, m: f64) -> f64 {
    (a % m).trunc() + f64::from(u8::from(a < 0.0)) * m
}

/// The nearest integer, halves away from zero.
pub(crate) fn nearest_integer(x: f64) -> i64 {
    (x + 0.5f64.copysign(x)) as i64
}

pub(crate) fn is_integral(x: f64, eps: f64) -> bool {
    let y = (x - x.trunc()).abs();
    y.min(1.0 - y) <= eps
}

/// HiGHS's `modularInverse`.
pub(crate) fn modular_inverse(a: i64, mut m: i64) -> i64 {
    let mut y = 0i64;
    let mut x = 1i64;
    if m == 1 {
        return 0;
    }
    let mut a = mod_i64(a, m);
    while a > 1 {
        let q = a / m;
        let mut r = a - q * m;
        a = m;
        m = r;
        r = x - q * y;
        x = y;
        y = r;
    }
    x
}

pub(crate) fn gcd(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    loop {
        let h = a % b;
        a = b;
        b = h;
        if b == 0 {
            return a;
        }
    }
}

/// HiGHS's `denominator`: the denominator of the best rational
/// approximation of `x` with one no bigger than `maxdenom`, by continued
/// fractions.
pub(crate) fn denominator(x: f64, eps: f64, maxdenom: i64) -> i64 {
    let mut ai = x as i64;
    let mut m = [ai, 1i64, 1i64, 0i64];
    let mut xi = CDouble::new(x);
    let mut fraction = xi - ai as f64;
    while fraction > eps {
        xi = CDouble::new(1.0) / fraction;
        if xi.value() > (1i64 << 53) as f64 {
            break;
        }
        ai = xi.value() as i64;
        let t = m[2].wrapping_mul(ai).wrapping_add(m[3]);
        if t > maxdenom {
            break;
        }
        m[3] = m[2];
        m[2] = t;
        let t = m[0].wrapping_mul(ai).wrapping_add(m[1]);
        m[1] = m[0];
        m[0] = t;
        fraction = xi - ai as f64;
    }
    ai = (maxdenom - m[3]) / m[2];
    m[1] += m[0] * ai;
    m[3] += m[2] * ai;
    let x0 = m[0] as f64 / m[2] as f64;
    let x1 = m[1] as f64 / m[3] as f64;
    let x = x.abs();
    let err0 = (x - x0).abs();
    let err1 = (x - x1).abs();
    if err0 < err1 { m[2] } else { m[3] }
}

/// HiGHS's `integralScale`: a multiplier that makes every value integral
/// to within `deltadown` below and `deltaup` above, or zero if none of
/// the kind it tries does.
pub(crate) fn integral_scale(vals: &[f64], deltadown: f64, deltaup: f64) -> f64 {
    if vals.is_empty() {
        return 0.0;
    }
    // `std::minmax_element` by absolute value: the first smallest and the
    // last largest.
    let mut minval = vals[0];
    let mut maxval = vals[0];
    for &value in &vals[1..] {
        if value.abs() < minval.abs() {
            minval = value;
        }
        if !(value.abs() < maxval.abs()) {
            maxval = value;
        }
    }
    let mut expshift = 0;
    if minval < -deltadown || minval > deltaup {
        expshift = frexp(minval).1;
    }
    expshift = (-expshift).max(0) + 3;
    let mut exp_max_val = frexp(maxval).1;
    exp_max_val = exp_max_val.min(32);
    if exp_max_val + expshift > 32 {
        expshift = 32 - exp_max_val;
    }
    let mut denom: u64 = 75u64 << expshift;
    let mut startdenom: i64 = denom as i64;
    let mut val = CDouble::new(vals[0]) * startdenom as f64;
    let mut downval = (val + deltaup).floor();
    let mut fraction = val - downval;
    if fraction > deltadown {
        denom = denom.wrapping_mul(denominator(fraction.value(), deltaup, 1000) as u64);
        val = CDouble::new(vals[0]) * denom as f64;
        downval = (val + deltaup).floor();
        fraction = val - downval;
        if fraction > deltadown {
            return 0.0;
        }
    }
    let mut currgcd = downval.value().abs() as u64;
    for &value in &vals[1..] {
        val = CDouble::new(value) * denom as f64;
        downval = (val + deltaup).floor();
        fraction = val - downval;
        if fraction > deltadown {
            val = CDouble::new(value) * startdenom as f64;
            fraction = val - val.floor();
            denom = denom.wrapping_mul(denominator(fraction.value(), deltaup, 1000) as u64);
            val = CDouble::new(value) * denom as f64;
            downval = (val + deltaup).floor();
            fraction = val - downval;
            if fraction > deltadown {
                return 0.0;
            }
        }
        if currgcd != 1 {
            currgcd = gcd(currgcd as i64, downval.value() as i64) as u64;
            if denom > u32::MAX as u64 {
                denom /= currgcd;
                if startdenom != 1 {
                    startdenom /= gcd(currgcd as i64, startdenom);
                }
                currgcd = 1;
            }
        }
    }
    denom as f64 / currgcd as f64
}

/// HiGHS's `fractionality`: the distance to the nearest integer.
pub(crate) fn fractionality(x: f64) -> f64 {
    (x - x.round()).abs()
}
