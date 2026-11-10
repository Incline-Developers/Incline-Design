//! SCIP's common definitions (`def.h`): numeric defaults and the
//! comparison macros, as functions with the macros' exact semantics.

pub const LONGINT_MAX: i64 = i64::MAX;
pub const REAL_MAX: f64 = f64::MAX;
pub const REAL_MIN: f64 = -f64::MAX;
pub const REAL_UNITROUNDOFF: f64 = 1.0 / 9007199254740992.0;
pub const DEFAULT_INFINITY: f64 = 1e+20;
pub const DEFAULT_EPSILON: f64 = 1e-09;
pub const DEFAULT_SUMEPSILON: f64 = 1e-06;
pub const DEFAULT_FEASTOL: f64 = 1e-06;
pub const DEFAULT_CHECKFEASTOLFAC: f64 = 1.0;
pub const DEFAULT_LPFEASTOLFACTOR: f64 = 1.0;
pub const DEFAULT_DUALFEASTOL: f64 = 1e-07;
pub const DEFAULT_BARRIERCONVTOL: f64 = 1e-10;
pub const DEFAULT_BOUNDSTREPS: f64 = 0.05;
pub const DEFAULT_PSEUDOCOSTEPS: f64 = 1e-01;
pub const DEFAULT_PSEUDOCOSTDELTA: f64 = 1e-04;
pub const DEFAULT_RECOMPFAC: f64 = 1e+06;
pub const DEFAULT_HUGEVAL: f64 = 1e+15;
pub const MAXEPSILON: f64 = 1e-03;
pub const MINEPSILON: f64 = 1e-20;
pub const INVALID: f64 = 1e+99;
pub const UNKNOWN: f64 = 1e+98;
pub const INTERVAL_INFINITY: f64 = 1e+300;
pub const MAXSTRLEN: usize = 1024;
pub const HASHSIZE_PARAMS: usize = 2048;
pub const HASHSIZE_NAMES: usize = 500;
pub const HASHSIZE_CUTPOOLS: usize = 500;
pub const HASHSIZE_CLIQUES: usize = 500;
pub const HASHSIZE_NAMES_SMALL: usize = 100;
pub const HASHSIZE_CUTPOOLS_SMALL: usize = 100;
pub const HASHSIZE_CLIQUES_SMALL: usize = 100;
pub const DEFAULT_MEM_ARRAYGROWFAC: f64 = 1.2;
pub const DEFAULT_MEM_ARRAYGROWINIT: i32 = 4;
pub const MAXTREEDEPTH: i32 = 1073741822;

/// `MAX(x, y)`: `x >= y ? x : y`.
#[inline(always)]
pub fn max<T: PartialOrd>(x: T, y: T) -> T {
    if x >= y { x } else { y }
}

/// `MIN(x, y)`: `x <= y ? x : y`.
#[inline(always)]
pub fn min<T: PartialOrd>(x: T, y: T) -> T {
    if x <= y { x } else { y }
}

/// `MAX3(x, y, z)`.
#[inline(always)]
pub fn max3<T: PartialOrd + Copy>(x: T, y: T, z: T) -> T {
    if x >= y { max(x, z) } else { max(y, z) }
}

/// `MIN3(x, y, z)`.
#[inline(always)]
pub fn min3<T: PartialOrd + Copy>(x: T, y: T, z: T) -> T {
    if x <= y { min(x, z) } else { min(y, z) }
}

/// `SQR(x)`.
#[inline(always)]
pub fn sqr(x: f64) -> f64 {
    x * x
}

#[inline(always)]
pub fn eps_eq(x: f64, y: f64, eps: f64) -> bool {
    (x - y).abs() <= eps
}
#[inline(always)]
pub fn eps_lt(x: f64, y: f64, eps: f64) -> bool {
    x - y < -eps
}
#[inline(always)]
pub fn eps_le(x: f64, y: f64, eps: f64) -> bool {
    x - y <= eps
}
#[inline(always)]
pub fn eps_gt(x: f64, y: f64, eps: f64) -> bool {
    x - y > eps
}
#[inline(always)]
pub fn eps_ge(x: f64, y: f64, eps: f64) -> bool {
    x - y >= -eps
}
#[inline(always)]
pub fn eps_z(x: f64, eps: f64) -> bool {
    x.abs() <= eps
}
#[inline(always)]
pub fn eps_p(x: f64, eps: f64) -> bool {
    x > eps
}
#[inline(always)]
pub fn eps_n(x: f64, eps: f64) -> bool {
    x < -eps
}
#[inline(always)]
pub fn eps_floor(x: f64, eps: f64) -> f64 {
    (x + eps).floor()
}
#[inline(always)]
pub fn eps_ceil(x: f64, eps: f64) -> f64 {
    (x - eps).ceil()
}
#[inline(always)]
pub fn eps_round(x: f64, eps: f64) -> f64 {
    (x - 0.5 + eps).ceil()
}
#[inline(always)]
pub fn eps_frac(x: f64, eps: f64) -> f64 {
    x - eps_floor(x, eps)
}
#[inline(always)]
pub fn eps_is_int(x: f64, eps: f64) -> bool {
    eps_frac(x, eps) <= eps
}

/// `EPSFLOOR`.
#[inline]
pub fn epsfloor(x: f64, eps: f64) -> f64 {
    (x + eps).floor()
}

/// `EPSFRAC`.
#[inline]
pub fn epsfrac(x: f64, eps: f64) -> f64 {
    x - epsfloor(x, eps)
}
