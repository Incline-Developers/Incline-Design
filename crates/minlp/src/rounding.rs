//! The floating-point rounding mode SCIP switches with `fesetround`
//! (`SCIPintervalSetRoundingMode`), emulated in software so that it behaves
//! the same on every target (wasm has no rounding control).
//!
//! The reference compiles `intervalarith.c` at `-O0` with
//! `-ffp-contract=off`, so every `+ - * /` and `sqrt` there is one IEEE
//! operation rounded in the current mode. Code that SCIP runs while it has
//! switched modes uses [`add`], [`sub`], [`mul`], [`div`] and [`sqrt`],
//! which round in the mode set by [`set_mode`]; each is correctly rounded
//! (the IEEE result in that mode, signed zeros, overflow and subnormals
//! included).

use std::cell::Cell;

/// `SCIP_ROUNDMODE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundMode {
    /// `SCIP_ROUND_DOWNWARDS`: round always down.
    Downwards,
    /// `SCIP_ROUND_UPWARDS`: round always up.
    Upwards,
    /// `SCIP_ROUND_NEAREST`: round always to nearest.
    Nearest,
    /// `SCIP_ROUND_ZERO`: round always towards 0.0.
    TowardsZero,
}

thread_local! {
    static MODE: Cell<RoundMode> = const { Cell::new(RoundMode::Nearest) };
}

/// `SCIPintervalSetRoundingMode`: sets rounding mode of floating point
/// operations.
#[inline]
pub fn set_mode(mode: RoundMode) {
    MODE.with(|m| m.set(mode));
}

/// `SCIPintervalGetRoundingMode`: gets current rounding mode of floating
/// point operations.
#[inline]
pub fn get_mode() -> RoundMode {
    MODE.with(|m| m.get())
}

/// The result of an operation that overflowed, in `mode`, with the sign of
/// the exact result.
fn overflow(neg: bool, mode: RoundMode) -> f64 {
    let inf = match mode {
        RoundMode::Nearest => true,
        RoundMode::Upwards => !neg,
        RoundMode::Downwards => neg,
        RoundMode::TowardsZero => false,
    };
    let mag = if inf { f64::INFINITY } else { f64::MAX };
    if neg { -mag } else { mag }
}

/// Rounds `m * 2^e` (with `sticky` set if nonzero bits below `m` were
/// dropped) to a double in `mode`; `neg` is the sign. `m` is nonzero.
fn round(neg: bool, m: u128, e: i32, sticky: bool, mode: RoundMode) -> f64 {
    let bits = 128 - m.leading_zeros() as i32;
    // Exponent of the leading bit.
    let lead = e + bits - 1;
    // Number of low bits of m below the target precision (the grid of
    // subnormals is 2^-1074).
    let shift = if lead < -1022 { -1074 - e } else { bits - 53 };

    let (mut mant, dropped_half, dropped_rest) = if shift <= 0 {
        (m << (-shift) as u32, false, sticky)
    } else if shift >= 128 {
        (0u128, false, true)
    } else {
        let mant = m >> shift as u32;
        let rem = m & ((1u128 << shift as u32) - 1);
        let half = 1u128 << (shift - 1) as u32;
        (mant, rem >= half, (rem & (half - 1)) != 0 || sticky)
    };
    let inexact = dropped_half || dropped_rest;

    let increment = match mode {
        RoundMode::Nearest => dropped_half && (dropped_rest || (mant & 1) == 1),
        RoundMode::Upwards => inexact && !neg,
        RoundMode::Downwards => inexact && neg,
        RoundMode::TowardsZero => false,
    };
    if increment {
        mant += 1;
    }

    let mut out = if lead < -1022 {
        // Subnormal: the mantissa is the bit pattern; a carry into 2^52 makes
        // it the smallest normal.
        mant as u64
    } else {
        // The exponent of the value mant * 2^(e + shift) with mant in [2^52,
        // 2^53] (a carry to 2^53 adds one to the exponent field).
        let exp = e + shift + 52;
        if exp > 1023 {
            return overflow(neg, mode);
        }
        (((exp + 1022) as u64) << 52) + mant as u64
    };
    if (out >> 52) >= 0x7ff {
        return overflow(neg, mode);
    }
    if neg {
        out |= 1 << 63;
    }
    f64::from_bits(out)
}

/// The integer mantissa and exponent of a finite nonzero double: `x = m *
/// 2^e`.
fn decompose(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    if exp == 0 { (frac, -1074) } else { (frac | (1u64 << 52), exp - 1075) }
}

/// `a + b` rounded in the current mode.
#[inline]
pub fn add(a: f64, b: f64) -> f64 {
    add_in(a, b, get_mode())
}

/// `a - b` rounded in the current mode.
#[inline]
pub fn sub(a: f64, b: f64) -> f64 {
    add_in(a, -b, get_mode())
}

/// `a * b` rounded in the current mode.
#[inline]
pub fn mul(a: f64, b: f64) -> f64 {
    mul_in(a, b, get_mode())
}

/// `a / b` rounded in the current mode.
#[inline]
pub fn div(a: f64, b: f64) -> f64 {
    div_in(a, b, get_mode())
}

/// `sqrt(a)` rounded in the current mode.
#[inline]
pub fn sqrt(a: f64) -> f64 {
    sqrt_in(a, get_mode())
}

/// `a + b` rounded in `mode`.
pub fn add_in(a: f64, b: f64, mode: RoundMode) -> f64 {
    let s = a + b;
    if mode == RoundMode::Nearest || s.is_nan() {
        return s;
    }
    if s.is_infinite() {
        // Infinite operands give exact infinities; otherwise it overflowed.
        return if a.is_infinite() || b.is_infinite() { s } else { overflow(s < 0.0, mode) };
    }
    if s == 0.0 {
        // Sums never underflow, so this is exact: -0 + -0 is -0, +0 + +0 is +0,
        // and any other exact zero sum (x + -x, +0 + -0) is -0 when rounding
        // downwards and +0 otherwise.
        let (na, nb) = (a.is_sign_negative(), b.is_sign_negative());
        return if na && nb {
            -0.0
        } else if !na && !nb {
            0.0
        } else if mode == RoundMode::Downwards {
            -0.0
        } else {
            0.0
        };
    }

    // Error-free transformation (Dekker's fast two-sum, larger operand
    // first): a + b = s + err exactly; with |a| >= |b|, s - a is bounded by
    // |b|, so nothing overflows.
    let (hi, lo) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
    let err = lo - (s - hi);
    if err == 0.0 {
        return s;
    }
    match mode {
        RoundMode::Upwards => {
            if err > 0.0 {
                s.next_up()
            } else {
                s
            }
        }
        RoundMode::Downwards => {
            if err < 0.0 {
                s.next_down()
            } else {
                s
            }
        }
        RoundMode::TowardsZero => {
            if s > 0.0 && err < 0.0 {
                s.next_down()
            } else if s < 0.0 && err > 0.0 {
                s.next_up()
            } else {
                s
            }
        }
        RoundMode::Nearest => unreachable!(),
    }
}

/// `a * b` rounded in `mode`.
pub fn mul_in(a: f64, b: f64, mode: RoundMode) -> f64 {
    if mode == RoundMode::Nearest || !a.is_finite() || !b.is_finite() || a == 0.0 || b == 0.0 {
        return a * b;
    }
    let neg = a.is_sign_negative() != b.is_sign_negative();
    let (ma, ea) = decompose(a);
    let (mb, eb) = decompose(b);
    round(neg, ma as u128 * mb as u128, ea + eb, false, mode)
}

/// `a / b` rounded in `mode`.
pub fn div_in(a: f64, b: f64, mode: RoundMode) -> f64 {
    if mode == RoundMode::Nearest || !a.is_finite() || !b.is_finite() || a == 0.0 || b == 0.0 {
        return a / b;
    }
    let neg = a.is_sign_negative() != b.is_sign_negative();
    let (ma, ea) = decompose(a);
    let (mb, eb) = decompose(b);
    // Normalize the dividend to 117 bits and the divisor below 2^53 so the
    // quotient has at least 64 bits.
    let na = (ma as u128) << (127 - (128 - (ma as u128).leading_zeros()));
    let sa = (127 - (128 - (ma as u128).leading_zeros())) as i32;
    let q = na / mb as u128;
    let r = na % mb as u128;
    round(neg, q, ea - sa - eb, r != 0, mode)
}

/// `sqrt(a)` rounded in `mode`.
pub fn sqrt_in(a: f64, mode: RoundMode) -> f64 {
    if mode == RoundMode::Nearest || !a.is_finite() || a <= 0.0 {
        return a.sqrt();
    }
    let (m, e) = decompose(a);
    // a = m * 2^e; scale m by an even shift so that e becomes even and the
    // root has at least 60 bits.
    let mut shift = 120 - (128 - (m as u128).leading_zeros()) as i32;
    if (e - shift) % 2 != 0 {
        shift += 1;
    }
    let n = (m as u128) << shift as u32;
    let r = isqrt(n);
    let rem = n - r * r;
    round(false, r, (e - shift) / 2, rem != 0, mode)
}

/// The integer square root (floor) of a `u128`.
fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    // Newton's iteration from a power of two above the root.
    let mut x = 1u128 << ((128 - n.leading_zeros()).div_ceil(2));
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}
