//! Port of `intervalarith.c`: interval arithmetic with outward rounding.
//!
//! Every `+ - * /` and `sqrt` of the C goes through [`crate::rounding`], so
//! it is rounded in the mode the C has set at that point (the reference
//! compiles this file at `-O0`, one rounded operation per source
//! operation). Negation and `fabs` are exact. The reference is built without
//! `NDEBUG`, so the functions (not `intervalarith.h`'s macros) apply.
//!
//! Not ported (nothing ported calls them): `SCIPintervalSin`, `Cos`,
//! `Entropy`, `QuadBivar` and `SolveBivariateQuadExpressionAllScalar`, and
//! `pow`/`exp`/`log` under a directed rounding mode (the C library's results
//! there are not reproduced).

use crate::{
    def::{max, min},
    rounding::{self as fp, RoundMode},
};

/// `SCIP_INTERVAL`: interval given by infimum and supremum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interval {
    /// Infimum (lower bound) of interval.
    pub inf: f64,
    /// Supremum (upper bound) of interval.
    pub sup: f64,
}

/// `SCIPnextafter` (`nextafter`): the next double after `x` in the direction
/// of `y`.
pub fn nextafter(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        x + y
    } else if x == y {
        y
    } else if x < y {
        x.next_up()
    } else {
        x.next_down()
    }
}

/// `SCIPintervalHasRoundingControl`: rounding control is emulated, so it is
/// always available.
pub fn has_rounding_control() -> bool {
    true
}

/// `SCIPintervalSetRoundingMode`.
#[inline]
pub fn set_rounding_mode(roundmode: RoundMode) {
    fp::set_mode(roundmode);
}

/// `SCIPintervalGetRoundingMode`.
#[inline]
pub fn get_rounding_mode() -> RoundMode {
    fp::get_mode()
}

/// `SCIPintervalSetRoundingModeDownwards`.
#[inline]
pub fn set_rounding_mode_downwards() {
    fp::set_mode(RoundMode::Downwards);
}

/// `SCIPintervalSetRoundingModeUpwards`.
#[inline]
pub fn set_rounding_mode_upwards() {
    fp::set_mode(RoundMode::Upwards);
}

/// `SCIPintervalSetRoundingModeToNearest`.
#[inline]
pub fn set_rounding_mode_to_nearest() {
    fp::set_mode(RoundMode::Nearest);
}

/// `SCIPintervalSetRoundingModeTowardsZero`.
#[inline]
pub fn set_rounding_mode_towards_zero() {
    fp::set_mode(RoundMode::TowardsZero);
}

/// `SCIPintervalNegateReal`: negates a number in a way that the compiler
/// does not reorder operations (negation is exact).
#[inline]
pub fn negate_real(x: f64) -> f64 {
    -x
}

/// `negate`.
#[inline]
fn negate(x: f64) -> f64 {
    -x
}

impl Interval {
    /// `SCIPintervalSet`: stores given value as interval.
    #[inline]
    pub fn set(&mut self, value: f64) {
        self.inf = value;
        self.sup = value;
    }

    /// `SCIPintervalSetBounds`: stores given infimum and supremum as
    /// interval.
    #[inline]
    pub fn set_bounds(&mut self, inf: f64, sup: f64) {
        self.inf = inf;
        self.sup = sup;
    }

    /// `SCIPintervalSetEmpty`: sets interval to empty interval, which will
    /// be [1.0, -1.0].
    #[inline]
    pub fn set_empty(&mut self) {
        self.inf = 1.0;
        self.sup = -1.0;
    }

    /// `SCIPintervalSetEntire`: sets interval to entire [-infinity,
    /// +infinity].
    #[inline]
    pub fn set_entire(&mut self, infinity: f64) {
        self.inf = -infinity;
        self.sup = infinity;
    }

    /// An interval with the given bounds.
    #[inline]
    pub fn new(inf: f64, sup: f64) -> Self {
        Self { inf, sup }
    }
}

/// `SCIPintervalGetInf`.
#[inline]
pub fn get_inf(interval: Interval) -> f64 {
    interval.inf
}

/// `SCIPintervalGetSup`.
#[inline]
pub fn get_sup(interval: Interval) -> f64 {
    interval.sup
}

/// `SCIPintervalIsEmpty`: indicates whether interval is empty, i.e.,
/// whether inf > sup.
pub fn is_empty(infinity: f64, operand: Interval) -> bool {
    if operand.sup >= infinity || operand.inf <= -infinity {
        return false;
    }
    operand.sup < operand.inf
}

/// `SCIPintervalIsEntire`: indicates whether interval is entire, i.e.,
/// whether inf <= -infinity and sup >= infinity.
pub fn is_entire(infinity: f64, operand: Interval) -> bool {
    operand.inf <= -infinity && operand.sup >= infinity
}

/// `SCIPintervalIsPositiveInfinity`: indicates whether interval is positive
/// infinity, i.e., [infinity, infinity].
pub fn is_positive_infinity(infinity: f64, operand: Interval) -> bool {
    operand.inf >= infinity && operand.sup >= operand.inf
}

/// `SCIPintervalIsNegativeInfinity`: indicates whether interval is negative
/// infinity, i.e., [-infinity, -infinity].
pub fn is_negative_infinity(infinity: f64, operand: Interval) -> bool {
    operand.sup <= -infinity && operand.inf <= operand.sup
}

/// `SCIPintervalIsSubsetEQ`: indicates whether operand1 is contained in
/// operand2.
pub fn is_subset_eq(infinity: f64, operand1: Interval, operand2: Interval) -> bool {
    // The empty interval is contained everywhere.
    if operand1.inf > operand1.sup {
        return true;
    }

    // Something not-empty is not contained in the empty interval.
    if operand2.inf > operand2.sup {
        return false;
    }

    (max(-infinity, operand1.inf) >= operand2.inf) && (min(infinity, operand1.sup) <= operand2.sup)
}

/// `SCIPintervalAreDisjoint`: indicates whether operand1 and operand2 are
/// disjoint.
pub fn are_disjoint(operand1: Interval, operand2: Interval) -> bool {
    (operand1.sup < operand2.inf) || (operand2.sup < operand1.inf)
}

/// `SCIPintervalAreDisjointEps`: indicates whether operand1 and operand2 are
/// disjoint with epsilon tolerance.
///
/// Returns whether minimal (relative) distance of intervals is larger than
/// epsilon. Same as `SCIPintervalIsEmpty(SCIPintervalIntersectEps(operand1,
/// operand2))`.
pub fn are_disjoint_eps(eps: f64, operand1: Interval, operand2: Interval) -> bool {
    if operand1.sup < operand2.inf {
        return rel_diff(operand2.inf, operand1.sup) > eps;
    }

    if operand1.inf > operand2.sup {
        return rel_diff(operand1.inf, operand2.sup) > eps;
    }

    false
}

/// `SCIPrelDiff` (`misc.c`), in the current rounding mode.
fn rel_diff(val1: f64, val2: f64) -> f64 {
    let absval1 = val1.abs();
    let absval2 = val2.abs();
    let quot = max(max(1.0, absval1), absval2);
    fp::div(fp::sub(val1, val2), quot)
}

/// `SCIPintervalIntersect`: intersection of two intervals.
pub fn intersect(resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    resultant.inf = max(operand1.inf, operand2.inf);
    resultant.sup = min(operand1.sup, operand2.sup);
}

/// `SCIPintervalIntersectEps`: intersection of two intervals with epsilon
/// tolerance.
///
/// If intersection of operand1 and operand2 is empty, but minimal
/// (relative) distance of intervals is at most epsilon, then set resultant
/// to singleton containing the point in operand1 that is closest to
/// operand2, i.e., `resultant = { operand1.sup }`, if `operand1.sup <
/// operand2.inf` and `resultant = { operand1.inf }`, if `operand1.inf >
/// operand2.sup`.
pub fn intersect_eps(resultant: &mut Interval, eps: f64, operand1: Interval, operand2: Interval) {
    if operand1.sup < operand2.inf {
        if rel_diff(operand2.inf, operand1.sup) <= eps {
            resultant.set(operand1.sup);
            return;
        }
    } else if operand1.inf > operand2.sup && rel_diff(operand1.inf, operand2.sup) <= eps {
        resultant.set(operand1.inf);
        return;
    }

    intersect(resultant, operand1, operand2);
}

/// `SCIPintervalUnify`: interval enclosure of the union of two intervals.
pub fn unify(resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    if operand1.inf > operand1.sup {
        // operand1 is empty.
        *resultant = operand2;
        return;
    }

    if operand2.inf > operand2.sup {
        // operand2 is empty.
        *resultant = operand1;
        return;
    }

    resultant.inf = min(operand1.inf, operand2.inf);
    resultant.sup = max(operand1.sup, operand2.sup);
}

/// `SCIPintervalAddInf`: adds operand1 and operand2 and stores infimum of
/// result in infimum of resultant.
pub fn add_inf(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    // [a,...] + [-inf,...] = [-inf,...] for all a, in particular,
    // [+inf,...] + [-inf,...] = [-inf,...]
    if operand1.inf <= -infinity || operand2.inf <= -infinity {
        resultant.inf = -infinity;
    }
    // [a,...] + [+inf,...] = [+inf,...] for all a > -inf
    else if operand1.inf >= infinity || operand2.inf >= infinity {
        resultant.inf = infinity;
    } else {
        resultant.inf = fp::add(operand1.inf, operand2.inf);
    }
}

/// `SCIPintervalAddSup`: adds operand1 and operand2 and stores supremum of
/// result in supremum of resultant.
pub fn add_sup(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    // [...,b] + [...,+inf] = [...,+inf] for all b, in particular,
    // [...,-inf] + [...,+inf] = [...,+inf]
    if operand1.sup >= infinity || operand2.sup >= infinity {
        resultant.sup = infinity;
    }
    // [...,b] + [...,-inf] = [...,-inf] for all b < +inf
    else if operand1.sup <= -infinity || operand2.sup <= -infinity {
        resultant.sup = -infinity;
    } else {
        resultant.sup = fp::add(operand1.sup, operand2.sup);
    }
}

/// `SCIPintervalAdd`: adds operand1 and operand2 and stores result in
/// resultant.
pub fn add(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    let roundmode = get_rounding_mode();

    // Compute infimum of result.
    set_rounding_mode(RoundMode::Downwards);
    add_inf(infinity, resultant, operand1, operand2);

    // Compute supremum of result.
    set_rounding_mode(RoundMode::Upwards);
    add_sup(infinity, resultant, operand1, operand2);

    set_rounding_mode(roundmode);
}

/// `SCIPintervalAddScalar`: adds operand1 and scalar operand2 and stores
/// result in resultant.
pub fn add_scalar(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    let roundmode = get_rounding_mode();

    // -inf + something >= -inf
    if operand1.inf <= -infinity || operand2 <= -infinity {
        resultant.inf = -infinity;
    } else if operand1.inf >= infinity || operand2 >= infinity {
        // inf + finite = inf, inf + inf = inf
        resultant.inf = infinity;
    } else {
        set_rounding_mode(RoundMode::Downwards);
        resultant.inf = fp::add(operand1.inf, operand2);
    }

    // inf + something <= inf
    if operand1.sup >= infinity || operand2 >= infinity {
        resultant.sup = infinity;
    } else if operand1.sup <= -infinity || operand2 <= -infinity {
        // -inf + finite = -inf, -inf + (-inf) = -inf
        resultant.sup = -infinity;
    } else {
        set_rounding_mode(RoundMode::Upwards);
        resultant.sup = fp::add(operand1.sup, operand2);
    }

    set_rounding_mode(roundmode);
}

/// `SCIPintervalAddVectors`: adds vector operand1 and vector operand2 and
/// stores result in vector resultant.
pub fn add_vectors(infinity: f64, resultant: &mut [Interval], operand1: &[Interval], operand2: &[Interval]) {
    let length = resultant.len();
    let roundmode = get_rounding_mode();

    // Compute infimums of resultant array.
    set_rounding_mode(RoundMode::Downwards);
    for i in 0..length {
        add_inf(infinity, &mut resultant[i], operand1[i], operand2[i]);
    }

    // Compute supremums of result array.
    set_rounding_mode(RoundMode::Upwards);
    for i in 0..length {
        add_sup(infinity, &mut resultant[i], operand1[i], operand2[i]);
    }

    set_rounding_mode(roundmode);
}

/// `SCIPintervalSub`: subtracts operand2 from operand1 and stores result in
/// resultant.
pub fn sub(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    let roundmode = get_rounding_mode();

    if operand1.inf <= -infinity || operand2.sup >= infinity {
        resultant.inf = -infinity;
    }
    // [a,b] - [-inf,-inf] = [+inf,+inf]
    else if operand1.inf >= infinity || operand2.sup <= -infinity {
        resultant.inf = infinity;
        resultant.sup = infinity;
        return;
    } else {
        set_rounding_mode(RoundMode::Downwards);
        resultant.inf = fp::sub(operand1.inf, operand2.sup);
    }

    if operand1.sup >= infinity || operand2.inf <= -infinity {
        resultant.sup = infinity;
    }
    // [a,b] - [+inf,+inf] = [-inf,-inf]
    else if operand1.sup <= -infinity || operand2.inf >= infinity {
        resultant.sup = -infinity;
    } else {
        set_rounding_mode(RoundMode::Upwards);
        resultant.sup = fp::sub(operand1.sup, operand2.inf);
    }

    set_rounding_mode(roundmode);
}

/// `SCIPintervalSubScalar`: subtracts scalar operand2 from operand1 and
/// stores result in resultant.
pub fn sub_scalar(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    add_scalar(infinity, resultant, operand1, -operand2);
}

/// `SCIPintervalMulInf`: multiplies operand1 with operand2 and stores
/// infimum of result in infimum of resultant.
pub fn mul_inf(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    if operand1.inf >= infinity {
        // operand1 is infinity scalar.
        mul_scalar_inf(infinity, resultant, operand2, infinity);
    } else if operand2.inf >= infinity {
        // operand2 is infinity scalar.
        mul_scalar_inf(infinity, resultant, operand1, infinity);
    } else if operand1.sup <= -infinity {
        // operand1 is -infinity scalar.
        mul_scalar_inf(infinity, resultant, operand2, -infinity);
    } else if operand2.sup <= -infinity {
        // operand2 is -infinity scalar.
        mul_scalar_inf(infinity, resultant, operand1, -infinity);
    } else if (operand1.inf <= -infinity && operand2.sup > 0.0)
        || (operand1.sup > 0.0 && operand2.inf <= -infinity)
        || (operand1.inf < 0.0 && operand2.sup >= infinity)
        || (operand1.sup >= infinity && operand2.inf < 0.0)
    {
        resultant.inf = -infinity;
    } else {
        let cand1 = fp::mul(operand1.inf, operand2.inf);
        let cand2 = fp::mul(operand1.inf, operand2.sup);
        let cand3 = fp::mul(operand1.sup, operand2.inf);
        let cand4 = fp::mul(operand1.sup, operand2.sup);
        resultant.inf = min(min(cand1, cand2), min(cand3, cand4));
    }
}

/// `SCIPintervalMulSup`: multiplies operand1 with operand2 and stores
/// supremum of result in supremum of resultant.
pub fn mul_sup(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    if operand1.inf >= infinity {
        // operand1 is infinity scalar.
        mul_scalar_sup(infinity, resultant, operand2, infinity);
    } else if operand2.inf >= infinity {
        // operand2 is infinity scalar.
        mul_scalar_sup(infinity, resultant, operand1, infinity);
    } else if operand1.sup <= -infinity {
        // operand1 is -infinity scalar.
        mul_scalar_sup(infinity, resultant, operand2, -infinity);
    } else if operand2.sup <= -infinity {
        // operand2 is -infinity scalar.
        mul_scalar_sup(infinity, resultant, operand1, -infinity);
    } else if (operand1.inf <= -infinity && operand2.inf < 0.0)
        || (operand1.inf < 0.0 && operand2.inf <= -infinity)
        || (operand1.sup > 0.0 && operand2.sup >= infinity)
        || (operand1.sup >= infinity && operand2.sup > 0.0)
    {
        resultant.sup = infinity;
    } else {
        let cand1 = fp::mul(operand1.inf, operand2.inf);
        let cand2 = fp::mul(operand1.inf, operand2.sup);
        let cand3 = fp::mul(operand1.sup, operand2.inf);
        let cand4 = fp::mul(operand1.sup, operand2.sup);
        resultant.sup = max(max(cand1, cand2), max(cand3, cand4));
    }
}

/// `SCIPintervalMul`: multiplies operand1 with operand2 and stores result
/// in resultant.
pub fn mul(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    let roundmode = get_rounding_mode();

    // Compute infimum result.
    set_rounding_mode(RoundMode::Downwards);
    mul_inf(infinity, resultant, operand1, operand2);

    // Compute supremum of result.
    set_rounding_mode(RoundMode::Upwards);
    mul_sup(infinity, resultant, operand1, operand2);

    set_rounding_mode(roundmode);
}

/// `SCIPintervalMulScalarInf`: multiplies operand1 with scalar operand2 and
/// stores infimum of result in infimum of resultant.
pub fn mul_scalar_inf(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    if operand2 >= infinity {
        // result.inf defined by sign of operand1.inf
        if operand1.inf > 0.0 {
            resultant.inf = infinity;
        } else if operand1.inf < 0.0 {
            resultant.inf = -infinity;
        } else {
            resultant.inf = 0.0;
        }
    } else if operand2 <= -infinity {
        // result.inf defined by sign of operand1.sup
        if operand1.sup > 0.0 {
            resultant.inf = -infinity;
        } else if operand1.sup < 0.0 {
            resultant.inf = infinity;
        } else {
            resultant.inf = 0.0;
        }
    } else if operand2 == 0.0 {
        resultant.inf = 0.0;
    } else if operand2 > 0.0 {
        if operand1.inf <= -infinity {
            resultant.inf = -infinity;
        } else if operand1.inf >= infinity {
            resultant.inf = infinity;
        } else {
            resultant.inf = fp::mul(operand1.inf, operand2);
        }
    } else if operand1.sup >= infinity {
        resultant.inf = -infinity;
    } else if operand1.sup <= -infinity {
        resultant.inf = infinity;
    } else {
        resultant.inf = fp::mul(operand1.sup, operand2);
    }
}

/// `SCIPintervalMulScalarSup`: multiplies operand1 with scalar operand2 and
/// stores supremum of result in supremum of resultant.
pub fn mul_scalar_sup(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    if operand2 >= infinity {
        // result.sup defined by sign of operand1.sup
        if operand1.sup > 0.0 {
            resultant.sup = infinity;
        } else if operand1.sup < 0.0 {
            resultant.sup = -infinity;
        } else {
            resultant.sup = 0.0;
        }
    } else if operand2 <= -infinity {
        // result.sup defined by sign of operand1.inf
        if operand1.inf > 0.0 {
            resultant.sup = -infinity;
        } else if operand1.inf < 0.0 {
            resultant.sup = infinity;
        } else {
            resultant.sup = 0.0;
        }
    } else if operand2 == 0.0 {
        resultant.sup = 0.0;
    } else if operand2 > 0.0 {
        if operand1.sup >= infinity {
            resultant.sup = infinity;
        } else if operand1.sup <= -infinity {
            resultant.sup = -infinity;
        } else {
            resultant.sup = fp::mul(operand1.sup, operand2);
        }
    } else if operand1.inf <= -infinity {
        resultant.sup = infinity;
    } else if operand1.inf >= infinity {
        resultant.sup = -infinity;
    } else {
        resultant.sup = fp::mul(operand1.inf, operand2);
    }
}

/// `SCIPintervalMulScalar`: multiplies operand1 with scalar operand2 and
/// stores result in resultant.
pub fn mul_scalar(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    if operand2 == 1.0 {
        *resultant = operand1;
        return;
    }

    if operand2 == -1.0 {
        resultant.inf = -operand1.sup;
        resultant.sup = -operand1.inf;
        return;
    }

    let roundmode = get_rounding_mode();

    // Compute infimum result.
    set_rounding_mode(RoundMode::Downwards);
    mul_scalar_inf(infinity, resultant, operand1, operand2);

    // Compute supremum of result.
    set_rounding_mode(RoundMode::Upwards);
    mul_scalar_sup(infinity, resultant, operand1, operand2);

    set_rounding_mode(roundmode);
}

/// `SCIPintervalDiv`: divides operand1 by operand2 and stores result in
/// resultant.
pub fn div(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    let mut intmed = Interval::default();

    if operand2.inf <= 0.0 && operand2.sup >= 0.0 {
        // Division by [0,0] or interval containing 0 gives [-inf, +inf].
        resultant.inf = -infinity;
        resultant.sup = infinity;
        return;
    }

    if operand1.inf == 0.0 && operand1.sup == 0.0 {
        // Division of [0,0] by something nonzero.
        resultant.set(0.0);
        return;
    }

    let roundmode = get_rounding_mode();

    // Division by nonzero: resultant = x * (1/y)
    if operand2.sup >= infinity || operand2.sup <= -infinity {
        intmed.inf = 0.0;
    } else {
        set_rounding_mode(RoundMode::Downwards);
        intmed.inf = fp::div(1.0, operand2.sup);
    }
    if operand2.inf <= -infinity || operand2.inf >= infinity {
        intmed.sup = 0.0;
    } else {
        set_rounding_mode(RoundMode::Upwards);
        intmed.sup = fp::div(1.0, operand2.inf);
    }
    mul(infinity, resultant, operand1, intmed);

    set_rounding_mode(roundmode);
}

/// `SCIPintervalDivScalar`: divides operand1 by scalar operand2 and stores
/// result in resultant.
pub fn div_scalar(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    let roundmode = get_rounding_mode();

    if operand2 >= infinity || operand2 <= -infinity {
        // Division by +/-infinity is 0.0.
        resultant.inf = 0.0;
        resultant.sup = 0.0;
    } else if operand2 > 0.0 {
        if operand1.inf <= -infinity {
            resultant.inf = -infinity;
        } else if operand1.inf >= infinity {
            // infinity / + = infinity
            resultant.inf = infinity;
        } else {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::div(operand1.inf, operand2);
        }

        if operand1.sup >= infinity {
            resultant.sup = infinity;
        } else if operand1.sup <= -infinity {
            // -infinity / + = -infinity
            resultant.sup = -infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::div(operand1.sup, operand2);
        }
    } else if operand2 < 0.0 {
        if operand1.sup >= infinity {
            resultant.inf = -infinity;
        } else if operand1.sup <= -infinity {
            // -infinity / - = infinity
            resultant.inf = infinity;
        } else {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::div(operand1.sup, operand2);
        }

        if operand1.inf <= -infinity {
            resultant.sup = infinity;
        } else if operand1.inf >= infinity {
            // infinity / - = -infinity
            resultant.sup = -infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::div(operand1.inf, operand2);
        }
    } else {
        // Division by 0.0.
        if operand1.inf >= 0.0 {
            // [+,+] / [0,0] = [+inf, +inf]
            resultant.inf = infinity;
            resultant.sup = infinity;
        } else if operand1.sup <= 0.0 {
            // [-,-] / [0,0] = [-inf, -inf]
            resultant.inf = -infinity;
            resultant.sup = -infinity;
        } else {
            // [-,+] / [0,0] = [-inf, +inf]
            resultant.inf = -infinity;
            resultant.sup = infinity;
        }
        return;
    }

    set_rounding_mode(roundmode);
}

/// `SCIPintervalScalprod`: computes the scalar product of two vectors of
/// intervals and stores result in resultant.
pub fn scalprod(infinity: f64, resultant: &mut Interval, operand1: &[Interval], operand2: &[Interval]) {
    let length = operand1.len();
    let roundmode = get_rounding_mode();
    let mut prod = Interval::default();

    resultant.inf = 0.0;
    resultant.sup = 0.0;

    // Compute infimum of resultant.
    set_rounding_mode(RoundMode::Downwards);
    let mut i = 0;
    while i < length && resultant.inf > -infinity {
        prod.set_entire(infinity);
        mul_inf(infinity, &mut prod, operand1[i], operand2[i]);
        let r = *resultant;
        add_inf(infinity, resultant, r, prod);
        i += 1;
    }

    // Compute supremum of resultant.
    set_rounding_mode(RoundMode::Upwards);
    i = 0;
    while i < length && resultant.sup < infinity {
        prod.set_entire(infinity);
        mul_sup(infinity, &mut prod, operand1[i], operand2[i]);
        let r = *resultant;
        add_sup(infinity, resultant, r, prod);
        i += 1;
    }

    set_rounding_mode(roundmode);
}

/// `SCIPintervalScalprodScalarsInf`: computes the scalar product of a vector
/// of intervals and a vector of scalars and stores infimum of result in
/// infimum of resultant.
pub fn scalprod_scalars_inf(infinity: f64, resultant: &mut Interval, operand1: &[Interval], operand2: &[f64]) {
    let length = operand1.len();
    let mut prod = Interval::default();

    resultant.inf = 0.0;

    // Compute infimum of resultant.
    prod.set_entire(infinity);
    let mut i = 0;
    while i < length && resultant.inf > -infinity {
        mul_scalar_inf(infinity, &mut prod, operand1[i], operand2[i]);
        let r = *resultant;
        add_inf(infinity, resultant, r, prod);
        i += 1;
    }
}

/// `SCIPintervalScalprodScalarsSup`: computes the scalar product of a vector
/// of intervals and a vector of scalars and stores supremum of result in
/// supremum of resultant.
pub fn scalprod_scalars_sup(infinity: f64, resultant: &mut Interval, operand1: &[Interval], operand2: &[f64]) {
    let length = operand1.len();
    let mut prod = Interval::default();

    resultant.sup = 0.0;

    // Compute supremum of resultant.
    prod.set_entire(infinity);
    let mut i = 0;
    while i < length && resultant.sup < infinity {
        mul_scalar_sup(infinity, &mut prod, operand1[i], operand2[i]);
        let r = *resultant;
        add_sup(infinity, resultant, r, prod);
        i += 1;
    }
}

/// `SCIPintervalScalprodScalars`: computes the scalar product of a vector of
/// intervals and a vector of scalars and stores result in resultant.
pub fn scalprod_scalars(infinity: f64, resultant: &mut Interval, operand1: &[Interval], operand2: &[f64]) {
    let roundmode = get_rounding_mode();

    resultant.inf = 0.0;
    resultant.sup = 0.0;

    // Compute infimum of resultant.
    set_rounding_mode(RoundMode::Downwards);
    scalprod_scalars_inf(infinity, resultant, operand1, operand2);

    // Compute supremum of resultant.
    set_rounding_mode(RoundMode::Upwards);
    scalprod_scalars_sup(infinity, resultant, operand1, operand2);

    set_rounding_mode(roundmode);
}

/// `SCIPintervalSquare`: squares operand and stores result in resultant.
pub fn square(infinity: f64, resultant: &mut Interval, operand: Interval) {
    let roundmode = get_rounding_mode();

    if operand.sup <= 0.0 {
        // Operand is left of 0.0.
        if operand.sup <= -infinity {
            resultant.inf = infinity;
        } else {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::mul(operand.sup, operand.sup);
        }

        if operand.inf <= -infinity {
            resultant.sup = infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::mul(operand.inf, operand.inf);
        }
    } else if operand.inf >= 0.0 {
        // Operand is right of 0.0.
        if operand.inf >= infinity {
            resultant.inf = infinity;
        } else {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::mul(operand.inf, operand.inf);
        }

        if operand.sup >= infinity {
            resultant.sup = infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::mul(operand.sup, operand.sup);
        }
    } else {
        // [-,+]^2
        resultant.inf = 0.0;
        if operand.inf <= -infinity || operand.sup >= infinity {
            resultant.sup = infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            let x = fp::mul(operand.inf, operand.inf);
            let y = fp::mul(operand.sup, operand.sup);
            resultant.sup = max(x, y);
        }
    }
    set_rounding_mode(roundmode);
}

/// `SCIPintervalSquareRoot`: stores (positive part of) square root of
/// operand in resultant.
///
/// @attention we assume a correctly rounded sqrt(double) function when
/// rounding is to nearest.
pub fn square_root(infinity: f64, resultant: &mut Interval, operand: Interval) {
    if operand.sup < 0.0 {
        resultant.set_empty();
        return;
    }

    if operand.inf == operand.sup {
        if operand.inf >= infinity {
            resultant.inf = infinity;
            resultant.sup = infinity;
        } else {
            let tmp = fp::sqrt(operand.inf);
            resultant.inf = nextafter(tmp, crate::def::REAL_MIN);
            resultant.sup = nextafter(tmp, crate::def::REAL_MAX);
        }

        return;
    }

    if operand.inf <= 0.0 {
        resultant.inf = 0.0;
    } else if operand.inf >= infinity {
        resultant.inf = infinity;
        resultant.sup = infinity;
    } else {
        resultant.inf = nextafter(fp::sqrt(operand.inf), crate::def::REAL_MIN);
    }

    if operand.sup >= infinity {
        resultant.sup = infinity;
    } else {
        resultant.sup = nextafter(fp::sqrt(operand.sup), crate::def::REAL_MAX);
    }
}

/// `SCIPintervalPower`: stores operand1 to the power of operand2 in
/// resultant.
///
/// Uses `SCIPintervalPowerScalar` if operand2 is a scalar, otherwise
/// computes `exp(op2 * log(op1))`.
pub fn power(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    if operand2.inf == operand2.sup {
        // Operand is number.
        power_scalar(infinity, resultant, operand1, operand2.inf);
        return;
    }

    // log([..,0]) will give an empty interval below, but we want
    // [0,0]^exponent to be 0 if exponent is positive (and 1 if exponent is 0;
    // it's the empty set if exponent is negative).
    if operand1.sup == 0.0 {
        if operand2.inf <= 0.0 && operand2.sup >= 0.0 {
            resultant.set_bounds(0.0, 1.0);
        } else {
            resultant.set(0.0);
        }
        return;
    }

    // resultant := log(op1)
    log(infinity, resultant, operand1);
    if is_empty(infinity, *resultant) {
        return;
    }

    // resultant := op2 * resultant
    let r = *resultant;
    mul(infinity, resultant, operand2, r);

    // resultant := exp(resultant)
    let r = *resultant;
    exp(infinity, resultant, r);
}

/// `SCIPintervalPowerScalarIntegerInf`: computes lower bound on power of a
/// scalar operand1 to an integer operand2.
///
/// Both operands need to be finite numbers. Need to have operand1 >= 0 and
/// need to have operand2 >= 0 if operand1 == 0.
pub fn power_scalar_integer_inf(operand1: f64, operand2: i32) -> f64 {
    if operand1 == 0.0 {
        return if operand2 == 0 {
            1.0 // 0^0 = 1
        } else {
            0.0 // 0^positive = 0
        };
    }

    // 1^n = 1, x^0 = 1
    if operand1 == 1.0 || operand2 == 0 {
        return 1.0;
    }

    let mut result;
    if operand2 < 0 {
        // x^n = 1 / x^(-n)
        result = power_scalar_integer_sup(operand1, -operand2);

        let roundmode = get_rounding_mode();
        set_rounding_mode(RoundMode::Downwards);
        result = fp::div(1.0, result);
        set_rounding_mode(roundmode);
    } else {
        let roundmode = get_rounding_mode();

        result = 1.0;
        let mut n = operand2 as u32;
        let mut z = operand1;

        set_rounding_mode(RoundMode::Downwards);

        // Use a binary exponentiation algorithm: consider the binary
        // representation of n: n = sum_i 2^i d_i with d_i \in {0,1}; then x^n =
        // prod_i x^(2^i d_i) = prod_{i: d_i = 1} x^(2^i); in the following, we
        // loop over i=1,..., thereby storing x^(2^i) in z; whenever d_i = 1, we
        // multiply result with x^(2^i) (the current z); at the last (highest) i
        // with d_i = 1 we stop, thus having x^n stored in result.
        //
        // The binary representation of n and bit shifting is used for the loop.
        loop {
            if n & 1 != 0 {
                // n is odd (d_i=1), so multiply result with current z
                // (=x^{2^i}).
                result = fp::mul(result, z);
                n >>= 1;
                if n == 0 {
                    break;
                }
            } else {
                n >>= 1;
            }
            z = fp::mul(z, z);
        }

        set_rounding_mode(roundmode);
    }

    result
}

/// `SCIPintervalPowerScalarIntegerSup`: computes upper bound on power of a
/// scalar operand1 to an integer operand2.
///
/// Both operands need to be finite numbers. Need to have operand1 >= 0 and
/// need to have operand2 >= 0 if operand1 == 0.
pub fn power_scalar_integer_sup(operand1: f64, operand2: i32) -> f64 {
    if operand1 == 0.0 {
        return if operand2 == 0 {
            1.0 // 0^0 = 1
        } else {
            0.0 // 0^positive = 0
        };
    }

    // 1^n = 1, x^0 = 1
    if operand1 == 1.0 || operand2 == 0 {
        return 1.0;
    }

    let mut result;
    if operand2 < 0 {
        // x^n = 1 / x^(-n)
        result = power_scalar_integer_inf(operand1, -operand2);

        let roundmode = get_rounding_mode();
        set_rounding_mode(RoundMode::Upwards);
        result = fp::div(1.0, result);
        set_rounding_mode(roundmode);
    } else {
        let roundmode = get_rounding_mode();

        result = 1.0;
        let mut n = operand2 as u32;
        let mut z = operand1;

        set_rounding_mode(RoundMode::Upwards);

        // Use a binary exponentiation algorithm... see comments in
        // SCIPintervalPowerScalarIntegerInf.
        loop {
            if n & 1 != 0 {
                result = fp::mul(result, z);
                n >>= 1;
                if n == 0 {
                    break;
                }
            } else {
                n >>= 1;
            }
            z = fp::mul(z, z);
        }

        set_rounding_mode(roundmode);
    }

    result
}

/// `SCIPintervalPowerScalarInteger`: computes bounds on power of a scalar
/// operand1 to an integer operand2.
///
/// Both operands need to be finite numbers. Need to have operand1 >= 0 and
/// need to have operand2 >= 0 if operand1 == 0.
pub fn power_scalar_integer(resultant: &mut Interval, operand1: f64, operand2: i32) {
    if operand1 == 0.0 {
        if operand2 == 0 {
            resultant.set(1.0); // 0^0 = 1
        } else {
            resultant.set(0.0); // 0^positive = 0
        }
        return;
    }

    // 1^n = 1, x^0 = 1
    if operand1 == 1.0 || operand2 == 0 {
        resultant.set(1.0);
        return;
    }

    if operand2 < 0 {
        // x^n = 1 / x^(-n)
        power_scalar_integer(resultant, operand1, -operand2);
        // Value for infinity does not matter, since there should be no 0.0 in
        // the interval, so just use something large enough.
        let r = *resultant;
        reciprocal(crate::def::REAL_MAX, resultant, r);
    } else {
        let roundmode = get_rounding_mode();

        let mut result_inf = 1.0;
        let mut result_sup = 1.0;
        let mut z_inf = operand1;
        let mut z_sup = operand1;
        let mut n = operand2 as u32;

        set_rounding_mode(RoundMode::Upwards);

        // Use a binary exponentiation algorithm... see comments in
        // SCIPintervalPowerScalarIntegerInf; we compute both infimum and
        // supremum to have only one rounding mode switch.
        loop {
            if n & 1 != 0 {
                result_inf = negate(fp::mul(negate(result_inf), z_inf));
                result_sup = fp::mul(result_sup, z_sup);
                n >>= 1;
                if n == 0 {
                    break;
                }
            } else {
                n >>= 1;
            }
            z_inf = negate(fp::mul(negate(z_inf), z_inf));
            z_sup = fp::mul(z_sup, z_sup);
        }

        set_rounding_mode(roundmode);

        resultant.inf = result_inf;
        resultant.sup = result_sup;
    }
}

/// `SCIPintervalPowerScalarScalar`: computes bounds on power of a scalar
/// operand1 to a scalar operand2.
///
/// Both operands need to be finite numbers. Need to have operand1 >= 0. If
/// operand2 is not an integer, then need to have operand1 >= 0. If operand2
/// is < 0, then need to have operand1 > 0.
pub fn power_scalar_scalar(resultant: &mut Interval, operand1: f64, operand2: f64) {
    if operand1 == 0.0 {
        if operand2 == 0.0 {
            resultant.set(1.0); // 0^0 = 1
        } else {
            resultant.set(0.0); // 0^positive = 0
        }
        return;
    }

    // 1^n = 1, x^0 = 1
    if operand1 == 1.0 || operand2 == 0.0 {
        resultant.set(1.0);
        return;
    }

    let result = libm_pow(operand1, operand2);

    // To get safe bounds, get the floating point numbers just below and above
    // result.
    resultant.inf = nextafter(result, crate::def::REAL_MIN);
    resultant.sup = nextafter(result, crate::def::REAL_MAX);
}

/// The C library's `pow` in the current rounding mode: only round to
/// nearest is the same in Rust (the same libm on the reference's platform).
fn libm_pow(x: f64, y: f64) -> f64 {
    assert!(get_rounding_mode() == RoundMode::Nearest, "pow under a directed rounding mode is not ported");
    x.powf(y)
}

/// `SCIPintervalPowerScalar`: computes lower bound on power of a scalar
/// operand1 to an integer operand2.
///
/// Both operands need to be finite numbers. Need to have operand1 >= 0 and
/// need to have operand2 >= 0 if operand1 == 0.
pub fn power_scalar(infinity: f64, resultant: &mut Interval, mut operand1: Interval, operand2: f64) {
    if operand2 == infinity {
        // 0^infinity =  0
        // (0,1)^infinity = 0
        // 1^infinity = 1
        // (1,infinity)^infinity = infinity
        // [-1,0)^infinity is not defined
        if operand1.inf < 0.0 {
            resultant.inf = -infinity;
        } else {
            resultant.inf = 0.0;
        }
        if operand1.sup > 0.0 {
            resultant.sup = infinity;
        } else {
            resultant.sup = 0.0;
        }
        return;
    }

    if operand2 == 0.0 {
        // Special case, since x^0 = 1 for x != 0, but 0^0 = 0.
        if operand1.inf == 0.0 && operand1.sup == 0.0 {
            resultant.inf = 0.0;
            resultant.sup = 0.0;
        } else if operand1.inf <= 0.0 || operand1.sup >= 0.0 {
            // 0.0 in x gives [0,1].
            resultant.inf = 0.0;
            resultant.sup = 1.0;
        } else {
            // 0.0 outside x gives [1,1].
            resultant.inf = 1.0;
            resultant.sup = 1.0;
        }
        return;
    }

    if operand2 == 1.0 {
        // x^1 = x
        *resultant = operand1;
        return;
    }

    let op2isint = operand2.ceil() == operand2;

    if !op2isint && operand1.inf < 0.0 {
        // x^n with x negative not defined for n not integer.
        operand1.inf = 0.0;
        if operand1.sup < operand1.inf {
            resultant.set_empty();
            return;
        }
    }

    let half = fp::div(operand2, 2.0);
    if operand1.inf >= 0.0 {
        // Easy case: x^n with x >= 0.
        if operand2 >= 0.0 {
            // inf^+ = inf
            if operand1.inf >= infinity {
                resultant.inf = infinity;
            } else if operand1.inf > 0.0 {
                resultant.inf = nextafter(libm_pow(operand1.inf, operand2), crate::def::REAL_MIN);
            } else {
                resultant.inf = 0.0;
            }

            if operand1.sup >= infinity {
                resultant.sup = infinity;
            } else if operand1.sup > 0.0 {
                resultant.sup = nextafter(libm_pow(operand1.sup, operand2), crate::def::REAL_MAX);
            } else {
                resultant.sup = 0.0;
            }
        } else {
            if operand1.sup >= infinity {
                resultant.inf = 0.0;
            } else if operand1.sup == 0.0 {
                // x^(negative even) = infinity for x->0 (from both sides),
                // x^(negative odd) = -infinity for x->0 from left side.
                if half.ceil() == half {
                    resultant.inf = infinity;
                } else {
                    resultant.inf = -infinity;
                }
            } else {
                resultant.inf = nextafter(libm_pow(operand1.sup, operand2), crate::def::REAL_MIN);
            }

            // 0^(negative) = infinity
            if operand1.inf == 0.0 {
                resultant.sup = infinity;
            } else {
                resultant.sup = nextafter(libm_pow(operand1.inf, operand2), crate::def::REAL_MAX);
            }
        }
    } else if operand1.sup <= 0.0 {
        // More difficult case: x^n with x < 0; we now know, that n is integer.
        if operand2 >= 0.0 && half.ceil() == half {
            // x^n with n>=2 and even -> x^n is monotonically decreasing for x < 0.
            if operand1.sup == -infinity {
                // (-inf)^n = inf
                resultant.inf = infinity;
            } else {
                resultant.inf = power_scalar_integer_inf(-operand1.sup, operand2 as i32);
            }

            if operand1.inf <= -infinity {
                resultant.sup = infinity;
            } else {
                resultant.sup = power_scalar_integer_sup(-operand1.inf, operand2 as i32);
            }
        } else if operand2 <= 0.0 && half.ceil() != half {
            // x^n with n<=-1 and odd -> x^n = 1/x^(-n) is monotonically
            // decreasing for x<0.
            if operand1.sup == -infinity {
                // (-inf)^n = 1/(-inf)^(-n) = 1/(-inf) = 0
                resultant.inf = 0.0;
            } else if operand1.sup == 0.0 {
                // x^n -> -infinity for x->0 from left.
                resultant.inf = -infinity;
            } else {
                resultant.inf = -power_scalar_integer_sup(-operand1.sup, operand2 as i32);
            }

            if operand1.inf <= -infinity {
                // (-inf)^n = 1/(-inf)^(-n) = 1/(-inf) = 0
                resultant.sup = 0.0;
            } else if operand1.inf == 0.0 {
                // x^n -> infinity for x->0 from right.
                resultant.sup = infinity;
            } else {
                resultant.sup = -power_scalar_integer_inf(-operand1.inf, operand2 as i32);
            }
        } else if operand2 >= 0.0 {
            // x^n with n>0 and odd -> x^n is monotonically increasing for x<0.
            if operand1.inf <= -infinity {
                resultant.inf = -infinity;
            } else {
                resultant.inf = -power_scalar_integer_sup(-operand1.inf, operand2 as i32);
            }

            if operand1.sup <= -infinity {
                resultant.sup = -infinity;
            } else {
                resultant.sup = -power_scalar_integer_inf(-operand1.sup, operand2 as i32);
            }
        } else {
            // x^n with n<0 and even -> x^n is monotonically increasing for x<0.
            if operand1.inf <= -infinity {
                resultant.inf = 0.0;
            } else if operand1.inf == 0.0 {
                // x^n -> infinity for x->0 from both sides.
                resultant.inf = infinity;
            } else {
                resultant.inf = power_scalar_integer_sup(-operand1.inf, operand2 as i32);
            }

            if operand1.sup <= -infinity {
                resultant.sup = 0.0;
            } else if operand1.sup == 0.0 {
                // x^n -> infinity for x->0 from both sides.
                resultant.sup = infinity;
            } else {
                resultant.sup = power_scalar_integer_sup(-operand1.sup, operand2 as i32);
            }
        }
    } else {
        // Similar difficult case: x^n with x in [<0, >0], but n is integer.
        if operand2 >= 0.0 && half == half.ceil() {
            // n even positive integer.
            resultant.inf = 0.0;
            if operand1.inf == -infinity || operand1.sup == infinity {
                resultant.sup = infinity;
            } else {
                resultant.sup = power_scalar_integer_sup(max(-operand1.inf, operand1.sup), operand2 as i32);
            }
        } else if operand2 <= 0.0 && half.ceil() == half {
            // n even negative integer.
            resultant.sup = infinity; // since 0^n = infinity
            if operand1.inf == -infinity || operand1.sup == infinity {
                resultant.inf = 0.0;
            } else {
                resultant.inf = power_scalar_integer_inf(max(-operand1.inf, operand1.sup), operand2 as i32);
            }
        } else if operand2 >= 0.0 {
            // n odd positive integer, so monotonically increasing function.
            if operand1.inf == -infinity {
                resultant.inf = -infinity;
            } else {
                resultant.inf = -power_scalar_integer_sup(-operand1.inf, operand2 as i32);
            }
            if operand1.sup == infinity {
                resultant.sup = infinity;
            } else {
                resultant.sup = power_scalar_integer_sup(operand1.sup, operand2 as i32);
            }
        } else {
            // n odd negative integer: x^n in [-inf,inf] since x^n -> -infinity
            // for x->0 from left and x^n -> infinity for x->0 from right.
            resultant.inf = -infinity;
            resultant.sup = infinity;
        }
    }

    // If value for infinity is too small, relax intervals so they do not
    // appear empty.
    if resultant.inf > infinity {
        resultant.inf = infinity;
    }
    if resultant.sup < -infinity {
        resultant.sup = -infinity;
    }
}

/// `SCIPintervalPowerScalarInverse`: given an interval for the image of a
/// power operation, computes an interval for the origin.
///
/// That is, for \f$y = x^p\f$ with the exponent \f$p\f$ a given scalar and
/// \f$y\f$ = image a given interval, computes \f$x \subseteq\f$ basedomain
/// such that \f$y \in\f$ image for all \f$x \in\f$ basedomain.
pub fn power_scalar_inverse(infinity: f64, resultant: &mut Interval, basedomain: Interval, exponent: f64, image: Interval) {
    if exponent == 0.0 {
        // Exponent is 0.0.
        if image.inf <= 1.0 && image.sup >= 1.0 {
            // 1 in image -> resultant = entire.
            *resultant = basedomain;
        } else if image.inf <= 0.0 && image.sup >= 0.0 {
            // 0 in image, 1 not in image -> resultant = 0 (provided 0^0 = 0 ???)
            // -> resultant = {0} intersectDomain basedomain.
            resultant.set_bounds(max(0.0, basedomain.inf), min(0.0, basedomain.sup));
        } else {
            // 0 and 1 not in image -> resultant = empty.
            resultant.set_empty();
        }
        return;
    }

    // i = b^e
    //   i >= 0 -> b = i^(1/e) [and -i^(1/e), if e is even]
    //   i < 0  -> b = -(-i)^(1/e), if e is odd integer.
    //   i < 0, e is not integer -> b = empty.
    let mut exprecip = Interval::default();
    let mut tmp = Interval::default();
    exprecip.set_bounds(exponent, exponent);
    let e = exprecip;
    reciprocal(infinity, &mut exprecip, e);

    // Invert positive part of image, if any.
    if image.sup >= 0.0 {
        tmp.set_bounds(max(image.inf, 0.0), image.sup);
        power(infinity, resultant, tmp, exprecip);
        if basedomain.inf <= -resultant.inf && crate::def::eps_is_int(exponent, 0.0) && (exponent as i32) % 2 == 0 {
            if basedomain.sup < resultant.inf {
                resultant.set_bounds(-resultant.sup, -resultant.inf);
            } else {
                resultant.set_bounds(-resultant.sup, resultant.sup);
            }
        }

        let r = *resultant;
        intersect(resultant, r, basedomain);
    } else {
        resultant.set_empty();
    }

    // Invert negative part of image, if any and if base can take negative value
    // and if exponent is such that negative values are possible.
    if image.inf < 0.0 && basedomain.inf < 0.0 && crate::def::eps_is_int(exponent, 0.0) && ((exponent as i32) % 2 != 0) {
        tmp.set_bounds(max(-image.sup, 0.0), -image.inf);
        let t = tmp;
        power(infinity, &mut tmp, t, exprecip);
        tmp.set_bounds(-tmp.sup, -tmp.inf);
        let t = tmp;
        intersect(&mut tmp, basedomain, t);
        let r = *resultant;
        unify(resultant, r, tmp);
    }
}

/// `SCIPintervalSignPowerScalar`: stores operand1 to the signed power of
/// the scalar positive operand2 in resultant.
///
/// The signed power of x w.r.t. an exponent n >= 0 is given as
/// \f$\mathrm{sign}(x) |x|^n\f$.
///
/// @attention we assume correctly rounded sqrt(double) and pow(double)
/// functions when rounding is to nearest.
pub fn sign_power_scalar(infinity: f64, resultant: &mut Interval, operand1: Interval, operand2: f64) {
    if operand2 == infinity {
        // 0^infinity =  0
        // (0,1)^infinity = 0
        // 1^infinity = 1
        // (1,infinity)^infinity = infinity
        // -+^infinity = -infinity
        if operand1.inf < 0.0 {
            resultant.inf = -infinity;
        } else {
            resultant.inf = 0.0;
        }
        if operand1.sup > 0.0 {
            resultant.sup = infinity;
        } else {
            resultant.sup = 0.0;
        }
        return;
    }

    if operand2 == 0.0 {
        // Special case, since x^0 = 1 for x != 0, but 0^0 = 0.
        if operand1.inf < 0.0 {
            resultant.inf = -1.0;
        } else if operand1.inf == 0.0 {
            resultant.inf = 0.0;
        } else {
            resultant.inf = 1.0;
        }

        if operand1.sup < 0.0 {
            resultant.sup = -1.0;
        } else if operand1.sup == 0.0 {
            resultant.sup = 0.0;
        } else {
            resultant.sup = 1.0;
        }

        return;
    }

    if operand2 == 1.0 {
        // Easy case that should run fast.
        *resultant = operand1;
        return;
    }

    let roundmode = get_rounding_mode();

    if operand2 == 2.0 {
        // Common case where pow can easily be avoided.
        if operand1.inf <= -infinity {
            resultant.inf = -infinity;
        } else if operand1.inf >= infinity {
            resultant.inf = infinity;
        } else if operand1.inf > 0.0 {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::mul(operand1.inf, operand1.inf);
        } else {
            // Need upwards since we negate result of multiplication.
            set_rounding_mode(RoundMode::Upwards);
            resultant.inf = negate(fp::mul(operand1.inf, operand1.inf));
        }

        if operand1.sup >= infinity {
            resultant.sup = infinity;
        } else if operand1.sup <= -infinity {
            resultant.sup = -infinity;
        } else if operand1.sup > 0.0 {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::mul(operand1.sup, operand1.sup);
        } else {
            // Need downwards since we negate result of multiplication.
            set_rounding_mode(RoundMode::Downwards);
            resultant.sup = negate(fp::mul(operand1.sup, operand1.sup));
        }
    } else if operand2 == 0.5 {
        // Another common case where pow can easily be avoided.
        if operand1.inf <= -infinity {
            resultant.inf = -infinity;
        } else if operand1.inf >= infinity {
            resultant.inf = infinity;
        } else if operand1.inf >= 0.0 {
            resultant.inf = nextafter(fp::sqrt(operand1.inf), crate::def::REAL_MIN);
        } else {
            resultant.inf = -nextafter(fp::sqrt(-operand1.inf), crate::def::REAL_MAX);
        }

        if operand1.sup >= infinity {
            resultant.sup = infinity;
        } else if operand1.sup <= -infinity {
            resultant.sup = -infinity;
        } else if operand1.sup > 0.0 {
            resultant.sup = nextafter(fp::sqrt(operand1.sup), crate::def::REAL_MAX);
        } else {
            resultant.sup = -nextafter(fp::sqrt(-operand1.sup), crate::def::REAL_MAX);
        }
    } else {
        if operand1.inf <= -infinity {
            resultant.inf = -infinity;
        } else if operand1.inf >= infinity {
            resultant.inf = infinity;
        } else if operand1.inf > 0.0 {
            resultant.inf = nextafter(libm_pow(operand1.inf, operand2), crate::def::REAL_MIN);
        } else {
            resultant.inf = -nextafter(libm_pow(-operand1.inf, operand2), crate::def::REAL_MAX);
        }

        if operand1.sup >= infinity {
            resultant.sup = infinity;
        } else if operand1.sup <= -infinity {
            resultant.sup = -infinity;
        } else if operand1.sup > 0.0 {
            resultant.sup = nextafter(libm_pow(operand1.sup, operand2), crate::def::REAL_MAX);
        } else {
            resultant.sup = -nextafter(libm_pow(-operand1.sup, operand2), crate::def::REAL_MIN);
        }
    }

    set_rounding_mode(roundmode);
}

/// `SCIPintervalReciprocal`: computes the reciprocal of an interval.
pub fn reciprocal(infinity: f64, resultant: &mut Interval, operand: Interval) {
    if operand.inf == 0.0 && operand.sup == 0.0 {
        // 1/0 = [-inf,inf]
        resultant.inf = infinity;
        resultant.sup = -infinity;
        return;
    }

    let roundmode = get_rounding_mode();

    if operand.inf >= 0.0 {
        // 1/x with x >= 0
        if operand.sup >= infinity {
            resultant.inf = 0.0;
        } else {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::div(1.0, operand.sup);
        }

        if operand.inf >= infinity {
            resultant.sup = 0.0;
        } else if operand.inf == 0.0 {
            resultant.sup = infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::div(1.0, operand.inf);
        }

        set_rounding_mode(roundmode);
    } else if operand.sup <= 0.0 {
        // 1/x with x <= 0
        if operand.sup <= -infinity {
            resultant.inf = 0.0;
        } else if operand.sup == 0.0 {
            resultant.inf = -infinity;
        } else {
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::div(1.0, operand.sup);
        }

        if operand.inf <= -infinity {
            resultant.sup = infinity;
        } else {
            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::div(1.0, operand.inf);
        }
        set_rounding_mode(roundmode);
    } else {
        // 1/x with x in [-,+] is division by zero.
        resultant.inf = -infinity;
        resultant.sup = infinity;
    }
}

/// The C library's `exp` (round to nearest only, see [`libm_pow`]).
fn libm_exp(x: f64) -> f64 {
    assert!(get_rounding_mode() == RoundMode::Nearest, "exp under a directed rounding mode is not ported");
    x.exp()
}

/// The C library's `log` (round to nearest only, see [`libm_pow`]).
fn libm_log(x: f64) -> f64 {
    assert!(get_rounding_mode() == RoundMode::Nearest, "log under a directed rounding mode is not ported");
    x.ln()
}

/// `SCIPintervalExp`: stores exponential of operand in resultant.
///
/// @attention we assume a correctly rounded exp(double) function when
/// rounding is to nearest.
pub fn exp(infinity: f64, resultant: &mut Interval, operand: Interval) {
    if operand.sup <= -infinity {
        resultant.inf = 0.0;
        resultant.sup = 0.0;
        return;
    }

    if operand.inf >= infinity {
        resultant.inf = infinity;
        resultant.sup = infinity;
        return;
    }

    if operand.inf == operand.sup {
        if operand.inf == 0.0 {
            resultant.inf = 1.0;
            resultant.sup = 1.0;
        } else {
            let tmp = libm_exp(operand.inf);
            resultant.inf = if tmp > 0.0 { nextafter(tmp, crate::def::REAL_MIN) } else { 0.0 };
            resultant.sup = nextafter(tmp, crate::def::REAL_MAX);
            return;
        }
    }

    if operand.inf <= -infinity {
        resultant.inf = 0.0;
    } else if operand.inf == 0.0 {
        resultant.inf = 1.0;
    } else {
        let tmp = libm_exp(operand.inf);
        resultant.inf = if tmp > 0.0 { nextafter(tmp, crate::def::REAL_MIN) } else { 0.0 };
        // Make sure we do not exceed value for infinity, so interval is not
        // declared as empty if inf and sup are both > infinity.
        if resultant.inf >= infinity {
            resultant.inf = infinity;
        }
    }

    if operand.sup >= infinity {
        resultant.sup = infinity;
    } else if operand.sup == 0.0 {
        resultant.sup = 1.0;
    } else {
        resultant.sup = nextafter(libm_exp(operand.sup), crate::def::REAL_MAX);
        if resultant.sup < -infinity {
            resultant.sup = -infinity;
        }
    }
}

/// `SCIPintervalLog`: stores natural logarithm of operand in resultant.
///
/// @attention we assume a correctly rounded log(double) function when
/// rounding is to nearest.
pub fn log(infinity: f64, resultant: &mut Interval, operand: Interval) {
    // If operand.sup == 0.0, we could return -inf in resultant->sup, but that
    // imposes problems since -inf is not a valid upper bound (at least in
    // SCIP), so we return an empty interval.
    if operand.sup <= 0.0 {
        resultant.set_empty();
        return;
    }

    if operand.inf == operand.sup {
        if operand.sup == 1.0 {
            resultant.inf = 0.0;
            resultant.sup = 0.0;
        } else {
            let tmp = libm_log(operand.inf);
            resultant.inf = nextafter(tmp, crate::def::REAL_MIN);
            resultant.sup = nextafter(tmp, crate::def::REAL_MAX);
        }

        return;
    }

    if operand.inf <= 0.0 {
        resultant.inf = -infinity;
    } else if operand.inf == 1.0 {
        resultant.inf = 0.0;
    } else {
        resultant.inf = nextafter(libm_log(operand.inf), crate::def::REAL_MIN);
    }

    if operand.sup >= infinity {
        resultant.sup = infinity;
    } else if operand.sup == 1.0 {
        resultant.sup = 0.0;
    } else {
        resultant.sup = nextafter(libm_log(operand.sup), crate::def::REAL_MAX);
    }
}

/// `SCIPintervalMin`: stores minimum of operands in resultant.
pub fn interval_min(resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    resultant.inf = min(operand1.inf, operand2.inf);
    resultant.sup = min(operand1.sup, operand2.sup);
}

/// `SCIPintervalMax`: stores maximum of operands in resultant.
pub fn interval_max(resultant: &mut Interval, operand1: Interval, operand2: Interval) {
    resultant.inf = max(operand1.inf, operand2.inf);
    resultant.sup = max(operand1.sup, operand2.sup);
}

/// `SCIPintervalAbsMax`: returns the maximum of the absolute values of the
/// infimum and supremum of the interval.
pub fn abs_max(interval: Interval) -> f64 {
    max(interval.sup.abs(), interval.inf.abs())
}

/// `SCIPintervalAbs`: stores absolute value of operand in resultant.
pub fn abs(resultant: &mut Interval, operand: Interval) {
    if operand.inf <= 0.0 && operand.sup >= 0.0 {
        resultant.inf = 0.0;
        resultant.sup = max(-operand.inf, operand.sup);
    } else if operand.inf > 0.0 {
        *resultant = operand;
    } else {
        resultant.inf = -operand.sup;
        resultant.sup = -operand.inf;
    }
}

/// `SCIPintervalSign`: stores sign of operand in resultant.
pub fn sign(resultant: &mut Interval, operand: Interval) {
    if operand.sup < 0.0 {
        resultant.inf = -1.0;
        resultant.sup = -1.0;
    } else if operand.inf >= 0.0 {
        resultant.inf = 1.0;
        resultant.sup = 1.0;
    } else {
        resultant.inf = -1.0;
        resultant.sup = 1.0;
    }
}

/// `SCIPintervalQuadUpperBound`: computes exact upper bound on \f$ a x^2 +
/// b x \f$ for x in [xlb, xub], b an interval, and a scalar.
///
/// Uses Algorithm 2.2 from Domes and Neumaier: Constraint propagation on
/// quadratic constraints (2008).
pub fn quad_upper_bound(infinity: f64, a: f64, b_: Interval, mut x: Interval) -> f64 {
    let b;
    let mut u;

    // Handle b*x separately.
    if a == 0.0 {
        if (b_.inf <= -infinity && x.inf < 0.0) || (b_.inf < 0.0 && x.inf <= -infinity) || (b_.sup > 0.0 && x.sup >= infinity) || (b_.sup >= infinity && x.sup > 0.0) {
            u = infinity;
        } else {
            let roundmode = get_rounding_mode();
            set_rounding_mode(RoundMode::Upwards);
            let cand1 = fp::mul(b_.inf, x.inf);
            let cand2 = fp::mul(b_.inf, x.sup);
            let cand3 = fp::mul(b_.sup, x.inf);
            let cand4 = fp::mul(b_.sup, x.sup);
            u = max(max(cand1, cand2), max(cand3, cand4));

            set_rounding_mode(roundmode);
        }

        return u;
    }

    if x.sup <= 0.0 {
        // Change sign of x: enclose a*x^2 + [-bub, -blb]*(-x) for (-x) in
        // [-xub, -xlb].
        u = x.sup;
        x.sup = -x.inf;
        x.inf = -u;
        b = -b_.inf;
    } else {
        b = b_.sup;
    }

    if x.inf >= 0.0 {
        // Upper bound for a*x^2 + b*x.
        if b >= infinity {
            return infinity;
        }

        let roundmode = get_rounding_mode();
        set_rounding_mode(RoundMode::Upwards);

        u = max(fp::mul(x.inf, fp::add(fp::mul(a, x.inf), b)), fp::mul(x.sup, fp::add(fp::mul(a, x.sup), b)));
        let s = fp::div(b, 2.0);
        let t = fp::div(s, negate(a));
        if t > x.inf && fp::mul(negate(fp::mul(2.0, a)), x.sup) > b && fp::mul(s, t) > u {
            u = fp::mul(s, t);
        }

        set_rounding_mode(roundmode);
        u
    } else {
        let mut xlow = x;

        xlow.sup = 0.0; // so xlow is lower part of interval
        x.inf = 0.0; // so x is upper part of interval now
        let cand1 = quad_upper_bound(infinity, a, b_, xlow);
        let cand2 = quad_upper_bound(infinity, a, b_, x);
        max(cand1, cand2)
    }
}

/// `SCIPintervalQuad`: stores range of quadratic term in resultant.
///
/// Given scalar a and intervals b and x, computes interval for \f$ a x^2 + b
/// x \f$.
pub fn quad(infinity: f64, resultant: &mut Interval, sqrcoeff: f64, mut lincoeff: Interval, xrng: Interval) {
    if is_empty(infinity, xrng) {
        resultant.set_empty();
        return;
    }
    if sqrcoeff == 0.0 {
        mul(infinity, resultant, lincoeff, xrng);
        return;
    }

    resultant.sup = quad_upper_bound(infinity, sqrcoeff, lincoeff, xrng);

    let tmp = lincoeff.inf;
    lincoeff.inf = -lincoeff.sup;
    lincoeff.sup = -tmp;
    resultant.inf = -quad_upper_bound(infinity, -sqrcoeff, lincoeff, xrng);
}

/// `SCIPintervalSolveUnivariateQuadExpressionPositive`: computes interval
/// with positive solutions of a quadratic equation with interval
/// coefficients.
///
/// Given intervals a, b, and c, this function computes an interval that
/// contains all positive solutions of \f$ a x^2 + b x \in c\f$ within
/// xbnds.
pub fn solve_univariate_quad_expression_positive(infinity: f64, resultant: &mut Interval, sqrcoeff: Interval, lincoeff: Interval, rhs: Interval, xbnds: Interval) {
    // Find x>=0 s.t. a.inf x^2 + b.inf x <= c.sup -> -a.inf x^2 - b.inf x >=
    // -c.sup
    if lincoeff.inf <= -infinity || rhs.sup >= infinity || sqrcoeff.inf <= -infinity {
        resultant.inf = 0.0;
        resultant.sup = infinity;
    } else {
        solve_univariate_quad_expression_positive_all_scalar(infinity, resultant, -sqrcoeff.inf, -lincoeff.inf, -rhs.sup, xbnds);
    }

    // Find x>=0 s.t. a.sup x^2 + b.sup x >= c.inf
    if lincoeff.sup < infinity && rhs.inf > -infinity && sqrcoeff.sup < infinity {
        let mut res2 = Interval::default();
        solve_univariate_quad_expression_positive_all_scalar(infinity, &mut res2, sqrcoeff.sup, lincoeff.sup, rhs.inf, xbnds);

        // Intersect both results.
        let r = *resultant;
        intersect(resultant, r, res2);
    }
    // else res2 = [0, infty]

    if resultant.inf >= infinity || resultant.sup <= -infinity {
        resultant.set_empty();
    }
}

/// `SCIPintervalSolveUnivariateQuadExpressionNegative`: computes interval
/// with negative solutions of a quadratic equation with interval
/// coefficients.
///
/// Given intervals a, b, and c, this function computes an interval that
/// contains all negative solutions of \f$ a x^2 + b x \in c\f$ within
/// xbnds.
pub fn solve_univariate_quad_expression_negative(infinity: f64, resultant: &mut Interval, sqrcoeff: Interval, mut lincoeff: Interval, rhs: Interval, mut xbnds: Interval) {
    // Change in variables y = -x, thus get all positive solutions of a * y^2 +
    // (-b) * y in c with -xbnds.
    let mut tmp = lincoeff.inf;
    lincoeff.inf = -lincoeff.sup;
    lincoeff.sup = -tmp;

    tmp = xbnds.inf;
    xbnds.inf = -xbnds.sup;
    xbnds.sup = -tmp;

    solve_univariate_quad_expression_positive(infinity, resultant, sqrcoeff, lincoeff, rhs, xbnds);

    tmp = resultant.inf;
    resultant.inf = -resultant.sup;
    resultant.sup = -tmp;
}

/// `SCIPintervalSolveUnivariateQuadExpressionPositiveAllScalar`: computes
/// positive solutions of a quadratic equation with scalar coefficients.
///
/// Givens scalar a, b, and c, this function computes an interval that
/// contains all positive solutions of \f$ a x^2 + b x \geq c\f$ within
/// xbnds.
pub fn solve_univariate_quad_expression_positive_all_scalar(infinity: f64, resultant: &mut Interval, sqrcoeff: f64, lincoeff: f64, rhs: f64, xbnds: Interval) {
    if sqrcoeff == 0.0 {
        // Special handling for linear b * x >= c
        //
        // The non-negative solutions here are:
        // b <  0, c <= 0 : [0, c/b]
        // b <= 0, c >  0 : empty
        // b >  0, c >  0 : [c/b, infty]
        // b >= 0, c <= 0 : [0, infty]
        //
        // The same should have been computed below, but without the sqrcoeff,
        // terms simplify (thus, also less rounding).

        if lincoeff <= 0.0 && rhs > 0.0 {
            resultant.set_empty();
            return;
        }

        if lincoeff >= 0.0 && rhs <= 0.0 {
            // [0,infty] cap xbnds
            resultant.inf = max(0.0, xbnds.inf);
            resultant.sup = xbnds.sup;
            return;
        }

        let roundmode = get_rounding_mode();

        if lincoeff < 0.0 && rhs <= 0.0 {
            // [0,c/b] cap xbnds
            resultant.inf = max(0.0, xbnds.inf);

            set_rounding_mode(RoundMode::Upwards);
            resultant.sup = fp::div(rhs, lincoeff);
            if xbnds.sup < resultant.sup {
                resultant.sup = xbnds.sup;
            }
        } else {
            // [c/b, infty] cap xbnds
            set_rounding_mode(RoundMode::Downwards);
            resultant.inf = fp::div(rhs, lincoeff);
            if resultant.inf < xbnds.inf {
                resultant.inf = xbnds.inf;
            }

            resultant.sup = xbnds.sup;
        }

        set_rounding_mode(roundmode);

        return;
    }

    resultant.inf = 0.0;
    resultant.sup = infinity;

    let roundmode = get_rounding_mode();

    // This should actually be round_upwards, but unless lincoeff is
    // min_double, there shouldn't be any rounding happening when dividing by
    // 2, i.e., shifting exponent, so it is ok to not change the rounding mode
    // here.
    let b = fp::div(lincoeff, 2.0);

    if lincoeff >= 0.0 {
        // b >= 0.0
        if rhs > 0.0 {
            // b >= 0.0 and c > 0.0
            set_rounding_mode(RoundMode::Upwards);
            let delta = fp::add(fp::mul(b, b), fp::mul(sqrcoeff, rhs));
            if delta < 0.0 {
                resultant.set_empty();
            } else {
                set_rounding_mode(RoundMode::Nearest);
                let mut z = nextafter(fp::sqrt(delta), crate::def::REAL_MAX);
                set_rounding_mode(RoundMode::Upwards);
                z = fp::add(z, b);
                resultant.inf = negate(fp::div(negate(rhs), z));

                if sqrcoeff < 0.0 {
                    resultant.sup = fp::div(z, negate(sqrcoeff));
                }
            }
        } else {
            // b >= 0.0 and c <= 0.0
            if sqrcoeff < 0.0 {
                set_rounding_mode(RoundMode::Upwards);
                let delta = fp::add(fp::mul(b, b), fp::mul(sqrcoeff, rhs));
                set_rounding_mode(RoundMode::Nearest);
                let mut z = nextafter(fp::sqrt(delta), crate::def::REAL_MAX);
                set_rounding_mode(RoundMode::Upwards);
                z = fp::add(z, b);
                resultant.sup = fp::div(z, negate(sqrcoeff));
            }
        }
    } else {
        // b < 0.0
        if rhs > 0.0 {
            // b < 0.0 and c > 0.0
            if sqrcoeff > 0.0 {
                set_rounding_mode(RoundMode::Downwards);
                let delta = fp::add(fp::mul(b, b), fp::mul(sqrcoeff, rhs));
                set_rounding_mode(RoundMode::Nearest);
                let mut z = nextafter(fp::sqrt(delta), crate::def::REAL_MIN);
                set_rounding_mode(RoundMode::Downwards);
                z = fp::add(z, negate(b));
                resultant.inf = fp::div(z, sqrcoeff);
            } else {
                resultant.set_empty();
            }
        } else {
            // b < 0.0 and c <= 0.0
            set_rounding_mode(RoundMode::Downwards);
            let delta = fp::add(fp::mul(b, b), fp::mul(sqrcoeff, rhs));
            if delta >= 0.0 {
                // Let resultant = [0,-c/z] for now.
                set_rounding_mode(RoundMode::Nearest);
                let mut z = nextafter(fp::sqrt(delta), crate::def::REAL_MIN);
                // Continue with downward rounding, because we want z (>= 0) to be
                // small, because -rhs/z needs to be large (-rhs >= 0).
                set_rounding_mode(RoundMode::Downwards);
                z = fp::add(z, negate(b));
                // Also now compute rhs/z with downward rounding, so that -(rhs/z)
                // becomes large.
                resultant.sup = negate(fp::div(rhs, z));

                if sqrcoeff > 0.0 {
                    // For a > 0, the result is [0,-c/z] \vee [z/a,infinity];
                    // currently, resultant = [0,-c/z].
                    let zdiva = fp::div(z, sqrcoeff);

                    if xbnds.sup < zdiva {
                        // After intersecting with xbnds, result is [0,-c/z], so we
                        // are done.
                    } else if xbnds.inf > resultant.sup {
                        // After intersecting with xbnds, result is [z/a,infinity].
                        resultant.inf = zdiva;
                        resultant.sup = infinity;
                    } else {
                        // After intersecting with xbnds we can neither exclude
                        // [0,-c/z] nor [z/a,infinity], so put resultant =
                        // [0,infinity] (intersection with xbnds happens below).
                        // @todo we could create a hole here
                        resultant.sup = infinity;
                    }
                } else {
                    // For a < 0, the result is [0,-c/z], so we are done.
                }
            }
        }
    }

    let r = *resultant;
    intersect(resultant, r, xbnds);
    set_rounding_mode(roundmode);
}

/// `SCIPintervalSolveUnivariateQuadExpression`: solves a quadratic equation
/// with interval coefficients.
///
/// Given intervals a, b and c, this function computes an interval that
/// contains all solutions of \f$ a x^2 + b x \in c\f$ within xbnds.
pub fn solve_univariate_quad_expression(infinity: f64, resultant: &mut Interval, sqrcoeff: Interval, lincoeff: Interval, rhs: Interval, xbnds: Interval) {
    let mut xpos = Interval::default();
    let mut xneg = Interval::default();

    // Special handling for lincoeff * x = rhs without 0 in lincoeff:
    // rhs/lincoeff gives a good interval that we just have to intersect with
    // xbnds; the code below would also work, but uses many more case
    // distinctions to get to a result that should be the same (though epsilon
    // differences can sometimes be observed).
    if sqrcoeff.inf == 0.0 && sqrcoeff.sup == 0.0 && (lincoeff.inf > 0.0 || lincoeff.sup < 0.0) {
        div(infinity, resultant, rhs, lincoeff);
        let r = *resultant;
        intersect(resultant, r, xbnds);
        return;
    }

    // Find all x>=0 such that a*x^2+b*x = c.
    if xbnds.sup >= 0.0 {
        solve_univariate_quad_expression_positive(infinity, &mut xpos, sqrcoeff, lincoeff, rhs, xbnds);
    } else {
        xpos.set_empty();
    }

    // Find all x<=0 such that a*x^2-b*x = c.
    if xbnds.inf <= 0.0 {
        solve_univariate_quad_expression_negative(infinity, &mut xneg, sqrcoeff, lincoeff, rhs, xbnds);
    } else {
        xneg.set_empty();
    }

    unify(resultant, xpos, xneg);
}

/// `SCIPintervalPropagateWeightedSum`: propagates a weighted sum of
/// intervals in a given interval.
///
/// Given \f$\text{constant} + \sum_i \text{weights}_i \text{operands}_i \in
/// \text{rhs}\f$, computes possibly tighter interval for each term.
///
/// @attention Valid values are returned in resultants only if any tightening
/// has been found and no empty interval, that is, function returns with
/// non-zero and `*infeasible` = FALSE.
///
/// Returns the number of terms for which resulting interval is smaller than
/// operand interval.
#[allow(clippy::too_many_arguments)]
pub fn propagate_weighted_sum(infinity: f64, operands: &[Interval], weights: &[f64], constant: f64, rhs: Interval, resultants: &mut [Interval], infeasible: &mut bool) -> i32 {
    let noperands = operands.len();
    let mut nreductions = 0;

    *infeasible = false;

    // Not possible to conclude finite bounds if the rhs is [-inf,inf].
    if is_entire(infinity, rhs) {
        return 0;
    }

    let prevroundmode = get_rounding_mode();
    set_rounding_mode_downwards();

    let mut minlinactivity = constant;
    let mut maxlinactivity = -constant; // use -constant because of the rounding mode
    let mut minlinactivityinf = 0;
    let mut maxlinactivityinf = 0;

    // Shift coefficients into the intervals of the children (using resultants
    // as working memory), compute the min and max activities.
    let mut c = 0;
    'terminate: {
        while c < noperands {
            let childbounds = operands[c];
            if is_empty(infinity, childbounds) {
                *infeasible = true;
                // Signal for terminate code to not copy operands to resultants
                // because we return *infeasible == TRUE.
                c = noperands;
                break 'terminate;
            }

            mul_scalar(infinity, &mut resultants[c], childbounds, weights[c]);

            if resultants[c].sup >= infinity {
                maxlinactivityinf += 1;
            } else {
                maxlinactivity = fp::sub(maxlinactivity, resultants[c].sup);
            }

            if resultants[c].inf <= -infinity {
                minlinactivityinf += 1;
            } else {
                minlinactivity = fp::add(minlinactivity, resultants[c].inf);
            }
            c += 1;
        }
        maxlinactivity = -maxlinactivity; // correct sign

        // If there are too many unbounded bounds, then could only compute
        // infinite bounds for children, so give up.
        if (minlinactivityinf >= 2 || rhs.sup >= infinity) && (maxlinactivityinf >= 2 || rhs.inf <= -infinity) {
            // Signal for terminate code that it doesn't need to copy operands
            // to resultants because we return nreductions==0.
            c = noperands;
            break 'terminate;
        }

        c = 0;
        while c < noperands {
            let mut childbounds = Interval::default();

            // Upper bounds of c_i is
            //  node->bounds.sup - (minlinactivity - c_i.inf), if c_i.inf > -infinity and minlinactivityinf == 0
            //  node->bounds.sup - minlinactivity, if c_i.inf == -infinity and minlinactivityinf == 1
            childbounds.set_entire(infinity);
            if rhs.sup < infinity {
                // We are still in downward rounding mode, so negate and negate to
                // get upward rounding.
                if resultants[c].inf <= -infinity && minlinactivityinf <= 1 {
                    childbounds.sup = negate_real(fp::sub(minlinactivity, rhs.sup));
                } else if minlinactivityinf == 0 {
                    childbounds.sup = negate_real(fp::sub(fp::sub(minlinactivity, rhs.sup), resultants[c].inf));
                }
            }

            // Lower bounds of c_i is
            //  node->bounds.inf - (maxlinactivity - c_i.sup), if c_i.sup < infinity and maxlinactivityinf == 0
            //  node->bounds.inf - maxlinactivity, if c_i.sup == infinity and maxlinactivityinf == 1
            if rhs.inf > -infinity {
                if resultants[c].sup >= infinity && maxlinactivityinf <= 1 {
                    childbounds.inf = fp::sub(rhs.inf, maxlinactivity);
                } else if maxlinactivityinf == 0 {
                    childbounds.inf = fp::add(fp::sub(rhs.inf, maxlinactivity), resultants[c].sup);
                }
            }

            // If interval arithmetics works correctly, then childbounds cannot be
            // empty, which is also asserted in SCIPintervalDivScalar below;
            // however, if running under valgrind, then interval arithmetics does
            // not work correctly and thus one may run into an assert in
            // SCIPintervalDivScalar, so this check avoids this by declaring the
            // propagation to result in an empty interval (thus only do if asserts
            // are on; the reference is built without NDEBUG).
            if is_empty(infinity, childbounds) {
                *infeasible = true;
                c = noperands;
                break 'terminate;
            }

            // Divide by the child coefficient.
            let cb = childbounds;
            div_scalar(infinity, &mut childbounds, cb, weights[c]);

            intersect(&mut resultants[c], operands[c], childbounds);
            if is_empty(infinity, resultants[c]) {
                *infeasible = true;
                c = noperands;
                break 'terminate;
            }

            if resultants[c].inf != operands[c].inf || resultants[c].sup != operands[c].sup {
                nreductions += 1;
            }
            c += 1;
        }
    }

    set_rounding_mode(prevroundmode);

    if c < noperands {
        resultants[c..noperands].copy_from_slice(&operands[c..noperands]);
    }

    nreductions
}
