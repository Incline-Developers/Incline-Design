//! Port of `expr_pow.c`: power and signed power expression handlers.
//!
//! @todo signpower for exponent < 1 ?

use std::any::Any;

use crate::{
    def::{INVALID, eps_is_int},
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, ExprOwnerCreate, ExprhdlrId, INTERVAL_INFINITY, Monotone},
    exprcurv::exprcurv_power_inv,
    interval::{self, Interval},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_expr::call_expr_simplify,
    types::{ImplintType, SolId},
    var::NONE,
};

const POWEXPRHDLR_NAME: &str = "pow";
const POWEXPRHDLR_DESC: &str = "power expression";
const POWEXPRHDLR_PRECEDENCE: u32 = 55000;

/// `POWEXPRHDLR_HASHKEY`.
fn powexprhdlr_hashkey() -> u32 {
    crate::misc::calc_fib_hash(21163.0)
}

const SIGNPOWEXPRHDLR_NAME: &str = "signpower";
const SIGNPOWEXPRHDLR_DESC: &str = "signed power expression";
const SIGNPOWEXPRHDLR_PRECEDENCE: u32 = 56000;

/// `SIGNPOWEXPRHDLR_HASHKEY`.
fn signpowexprhdlr_hashkey() -> u32 {
    crate::misc::calc_fib_hash(21163.1)
}

/// Maximal allowed absolute value of power expression at bound, used for
/// adjusting bounds in the convex case in initestimates.
const INITLPMAXPOWVAL: f64 = 1e+06;

/// `SIGN`: sign of a value (-1 or +1); 0.0 has sign +1 here (shouldn't
/// matter, though).
#[inline]
fn sign(x: f64) -> f64 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// Up to which (integer) exponents precomputed roots have been stored.
const SIGNPOW_ROOTS_KNOWN: usize = 10;

/// The positive root of the polynomial (n-1) y^n + n y^(n-1) - 1 is needed
/// in separation. Here we store these roots for small integer values of n.
static SIGNPOW_ROOTS: [f64; SIGNPOW_ROOTS_KNOWN + 1] = [
    -1.0,                   // no root for n=0
    -1.0,                   // no root for n=1
    0.41421356237309504880, // root for n=2 (-1+sqrt(2))
    0.5,                    // root for n=3
    0.56042566045031785945, // root for n=4
    0.60582958618826802099, // root for n=5
    0.64146546982884663257, // root for n=6
    0.67033204760309682774, // root for n=7
    0.69428385661425826738, // root for n=8
    0.71453772716733489700, // root for n=9
    0.73192937842370733350, // root for n=10
];

/// `SCIP_ExprhdlrData`: expression handler data.
struct ExprhdlrData {
    /// Minimal distance from zero to enforce for child in bound tightening.
    minzerodistance: f64,
    /// Maximal exponent when to expand power of sum in simplify.
    expandmaxexponent: i32,
    /// Whether a fractional exponent is distributed onto factors on power of
    /// product.
    distribfracexponent: bool,
    /// Whether we warned on enforcing a minimal distance from zero for child.
    warnedonpole: bool,
}

/// `SCIP_ExprData`: expression data.
#[derive(Clone)]
struct ExprData {
    /// Exponent.
    exponent: f64,
    /// Positive root of (n-1) y^n + n y^(n-1) - 1, or SCIP_INVALID if not
    /// computed yet.
    root: f64,
}

fn data(scip: &Scip, expr: ExprId) -> &ExprData {
    scip.exprs[expr].exprdata.as_ref().unwrap().downcast_ref::<ExprData>().unwrap()
}

fn data_mut(scip: &mut Scip, expr: ExprId) -> &mut ExprData {
    scip.exprs[expr].exprdata.as_mut().unwrap().downcast_mut::<ExprData>().unwrap()
}

fn hdlrdata(scip: &Scip, expr: ExprId) -> &ExprhdlrData {
    let h = scip.exprs[expr].exprhdlr;
    scip.exprhdlrs[h].data.as_ref().unwrap().downcast_ref::<ExprhdlrData>().unwrap()
}

fn hdlrdata_mut(scip: &mut Scip, expr: ExprId) -> &mut ExprhdlrData {
    let h = scip.exprs[expr].exprhdlr;
    scip.exprhdlrs[h].data.as_mut().unwrap().downcast_mut::<ExprhdlrData>().unwrap()
}

/// Whether `expr` belongs to the expression handler called `name` (the
/// `SCIPisExpr...` test of handlers that are not ported).
fn is_expr_named(scip: &Scip, expr: ExprId, name: &str) -> bool {
    scip.exprhdlrs[scip.exprs[expr].exprhdlr].name == name
}

/* local methods */

/// `computeSignpowerRoot`: computes positive root of the polynomial (n-1)
/// y^n + n y^(n-1) - 1 for n > 1.
fn compute_signpower_root(scip: &Scip, exponent: f64) -> ScipResult<f64> {
    let set = &scip.set;

    // Lookup for popular integer exponent.
    if set.is_integral(exponent) && exponent - 0.5 < SIGNPOW_ROOTS_KNOWN as f64 {
        return Ok(SIGNPOW_ROOTS[set.floor(exponent + 0.5) as i32 as usize]);
    }

    // Lookup for weymouth exponent.
    if set.is_eq(exponent, 1.852) {
        return Ok(0.39821689389382575186);
    }

    // Search for a positive root of (n-1) y^n + n y^(n-1) - 1; use the closest
    // precomputed root as starting value.
    let mut root = if exponent >= SIGNPOW_ROOTS_KNOWN as f64 {
        SIGNPOW_ROOTS[SIGNPOW_ROOTS_KNOWN]
    } else if exponent <= 2.0 {
        SIGNPOW_ROOTS[2]
    } else {
        SIGNPOW_ROOTS[set.floor(exponent) as i32 as usize]
    };

    let mut polyval = 0.0;
    for _iter in 0..1000 {
        polyval = (exponent - 1.0) * root.powf(exponent) + exponent * root.powf(exponent - 1.0) - 1.0;
        if polyval.abs() < 1e-12 && set.is_zero(polyval) {
            break;
        }

        // Gradient of (n-1) y^n + n y^(n-1) - 1 is n(n-1)y^(n-1) +
        // n(n-1)y^(n-2).
        let gradval = (exponent - 1.0) * exponent * (root.powf(exponent - 1.0) + root.powf(exponent - 2.0));
        if set.is_zero(gradval) {
            break;
        }

        // Update root by adding -polyval/gradval (Newton's method).
        root -= polyval / gradval;
        if root < 0.0 {
            root = 0.0;
        }
    }

    if !set.is_zero(polyval) {
        log::error!("failed to compute root for exponent {exponent}");
        return Err(Retcode::Error);
    }
    // @todo cache root value for other expressions (an exponent seldom comes
    // alone)?? (they are actually really fast to compute...)

    Ok(root)
}

/// `computeHyperbolaRoot`: computes negative root of the polynomial (n-1)
/// y^n - n y^(n-1) + 1 for n < -1.
fn compute_hyperbola_root(scip: &Scip, exponent: f64) -> ScipResult<f64> {
    let set = &scip.set;

    let mut root: f64 = -2.0; // That's the solution for n=-2.

    let mut polyval = 0.0;
    for _iter in 0..1000 {
        polyval = (exponent - 1.0) * root.powf(exponent) - exponent * root.powf(exponent - 1.0) + 1.0;
        if polyval.abs() < 1e-12 && set.is_zero(polyval) {
            break;
        }

        // Gradient of (n-1) y^n - n y^(n-1) + 1 is n(n-1)y^(n-1) -
        // n(n-1)y^(n-2).
        let gradval = (exponent - 1.0) * exponent * (root.powf(exponent - 1.0) - root.powf(exponent - 2.0));
        if set.is_zero(gradval) {
            break;
        }

        // Update root by adding -polyval/gradval (Newton's method).
        root -= polyval / gradval;
        if root >= 0.0 {
            root = -1.0;
        }
    }

    if !set.is_zero(polyval) {
        log::error!("failed to compute root for exponent {exponent}");
        return Err(Retcode::Error);
    }
    // @todo cache root value for other expressions (an exponent seldom comes
    // alone)?? (they are actually really fast to compute...)

    Ok(root)
}

/// `createData`: creates expression data.
fn create_data(exponent: f64) -> ExprData {
    ExprData { exponent, root: INVALID }
}

/// `computeTangent`: computes a tangent at a reference point by
/// linearization.
///
/// For a normal power, linearization in xref is xref^exponent + exponent *
/// xref^(exponent-1) (x - xref) = (1-exponent) * xref^exponent + exponent *
/// xref^(exponent-1) * x.
///
/// For a signpower, linearization is the same if xref is positive; for xref
/// negative it is -(-xref)^exponent + exponent * (-xref)^(exponent-1)
/// (x-xref) = (1-exponent) * (-xref)^(exponent-1) * xref + exponent *
/// (-xref)^(exponent-1) * x.
fn compute_tangent(signpower: bool, exponent: f64, mut xref: f64, constant: &mut f64, slope: &mut f64, success: &mut bool) {
    // Non-integral exponent -> reference point must be >= 0 or we do
    // signpower.

    // TODO power is not differentiable at 0.0 for exponent < 0; should we
    // forbid here that xref > 0, do something smart here, or just return
    // success=FALSE?
    if !eps_is_int(exponent, 0.0) && !signpower && xref < 0.0 {
        xref = 0.0;
    }

    let xrefpow = (if signpower { xref.abs() } else { xref }).powf(exponent - 1.0);

    // If huge xref and/or exponent too large, then pow may overflow.
    if !xrefpow.is_finite() {
        *success = false;
        return;
    }

    *constant = (1.0 - exponent) * xrefpow * xref;
    *slope = exponent * xrefpow;
    *success = true;
}

/// `computeSecant`: computes a secant between lower and upper bound.
///
/// Secant is xlb^exponent + (xub^exponent - xlb^exponent) / (xub - xlb) *
/// (x - xlb) = xlb^exponent - slope * xlb + slope * x with slope =
/// (xub^exponent - xlb^exponent) / (xub - xlb); same if signpower.
#[allow(clippy::too_many_arguments)]
fn compute_secant(scip: &Scip, signpower: bool, exponent: f64, mut xlb: f64, mut xub: f64, constant: &mut f64, slope: &mut f64, success: &mut bool) {
    let set = &scip.set;

    *success = false;

    // Infinite bounds will not work.
    if set.is_infinity(-xlb) || set.is_infinity(xub) {
        return;
    }

    // First handle some special cases.
    if xlb == xub {
        // Usually taken care of in separatePointPow already, but we might be
        // called with different bounds here, e.g., when handling odd or signed
        // power.
        *slope = 0.0;
        *constant = xlb.powf(exponent);
    } else if eps_is_int(exponent / 2.0, 0.0) && !signpower && xub > 0.1 && set.is_feas_eq(xlb, -xub) {
        // For normal power with even exponents with xlb ~ -xub the slope would be
        // very close to 0; since xub^n - xlb^n is prone to cancellation here, we
        // omit computing this secant (it's probably useless) unless the bounds
        // are close to 0 as well (xub <= 0.1 in the "if" above) or we have
        // exactly xlb=-xub, where we can return a clean 0.0 (though it's probably
        // useless).
        if xlb == -xub {
            *slope = 0.0;
            *constant = xlb.powf(exponent);
        } else {
            return;
        }
    } else if xlb == 0.0 && exponent > 0.0 {
        *slope = xub.powf(exponent - 1.0);
        *constant = 0.0;
    } else if xub == 0.0 && exponent > 0.0 {
        // Normal pow: slope = - xlb^exponent / (-xlb) = xlb^(exponent-1)
        // signpower:  slope = (-xlb)^exponent / (-xlb) = (-xlb)^(exponent-1)
        if signpower {
            *slope = (-xlb).powf(exponent - 1.0);
        } else {
            *slope = xlb.powf(exponent - 1.0);
        }
        *constant = 0.0;
    } else if set.is_eq(xlb, xub) && (!signpower || xlb >= 0.0 || xub <= 0.0) {
        // Computing the slope as (xub^n - xlb^n)/(xub-xlb) can lead to
        // cancellation. To avoid this, we replace xub^n by a Taylor expansion of
        // pow at xlb (see expr_pow.c for the derivation):
        //   slope = 0.5*n*((3-n)*xlb^(n-1) + (n-1) xlb^(n-2)*xub)
        //   constant = (1-0.5*n*(3-n)) xlb^n - 0.5 n*(n-1) xlb^(n-1) xub
        //
        // For signpower with xub <= 0, we can negate xlb and xub.
        if signpower && xub <= 0.0 {
            xlb *= -1.0;
            xub *= -1.0;
        }

        let xlb_n = xlb.powf(exponent); // xlb^n
        let xlb_n1 = xlb.powf(exponent - 1.0); // xlb^(n-1)
        let xlb_n2 = xlb.powf(exponent - 2.0); // xlb^(n-2)

        *slope = 0.5 * exponent * ((3.0 - exponent) * xlb_n1 + (exponent - 1.0) * xlb_n2 * xub);
        *constant = (1.0 - 0.5 * exponent * (3.0 - exponent)) * xlb_n - 0.5 * exponent * (exponent - 1.0) * xlb_n1 * xub;

        if signpower && xub <= 0.0 {
            *constant *= -1.0;
        }
    } else {
        let lbval = if signpower { sign(xlb) * xlb.abs().powf(exponent) } else { xlb.powf(exponent) };
        if !lbval.is_finite() {
            return;
        }

        let ubval = if signpower { sign(xub) * xub.abs().powf(exponent) } else { xub.powf(exponent) };
        if !ubval.is_finite() {
            return;
        }

        // We still can have bad numerics when xlb^exponent and xub^exponent are
        // very close, but xlb and xub are not; for now, only check that things
        // did not cancel out completely.
        if lbval == ubval {
            return;
        }

        *slope = (ubval - lbval) / (xub - xlb);
        *constant = lbval - *slope * xlb;
    }

    // Check whether we had overflows.
    if !slope.is_finite() || !constant.is_finite() {
        return;
    }

    *success = true;
}

/// `estimateParabola`: separation for parabola
/// - even positive powers: x^2, x^4, x^6 with x arbitrary, or
/// - positive powers > 1: x^1.5, x^2.5 with x >= 0
#[allow(clippy::too_many_arguments)]
fn estimate_parabola(scip: &Scip, exponent: f64, overestimate: bool, xlb: f64, xub: f64, xref: f64, constant: &mut f64, slope: &mut f64, islocal: &mut bool, success: &mut bool) {
    if !overestimate {
        compute_tangent(false, exponent, xref, constant, slope, success);
        *islocal = false;
    } else {
        // Overestimation -> secant.
        compute_secant(scip, false, exponent, xlb, xub, constant, slope, success);
        *islocal = true;
    }
}

/// `estimateSignedpower`: separation for signpower
/// - odd positive powers, x^3, x^5, x^7
/// - sign(x)|x|^n for n > 1
/// - lower bound on x is negative (otherwise one should use separation for
///   parabola)
#[allow(clippy::too_many_arguments)]
fn estimate_signedpower(
    scip: &Scip,
    exponent: f64,
    root: f64,
    overestimate: bool,
    xlb: f64,
    xub: f64,
    mut xref: f64,
    xlbglobal: f64,
    xubglobal: f64,
    constant: &mut f64,
    slope: &mut f64,
    islocal: &mut bool,
    branchcand: &mut bool,
    success: &mut bool,
) {
    *success = false;

    if !scip.set.is_positive(xub) {
        // Easy case.
        if !overestimate {
            // Underestimator is secant.
            compute_secant(scip, true, exponent, xlb, xub, constant, slope, success);
            *islocal = true;
        } else {
            // Overestimator is tangent.

            // We must linearize left of 0.
            if xref > 0.0 {
                xref = 0.0;
            }

            compute_tangent(true, exponent, xref, constant, slope, success);

            // If global upper bound is > 0, then the tangent is only valid locally if
            // the reference point is right of -root*xubglobal.
            *islocal = scip.set.is_positive(xubglobal) && xref > -root * xubglobal;

            // Tangent doesn't move after branching.
            *branchcand = false;
        }
    } else if !overestimate {
        // Compute the special point which decides between secant and tangent.
        let c = -xlb * root;

        if xref < c {
            // Underestimator is secant between xlb and c.
            compute_secant(scip, true, exponent, xlb, c, constant, slope, success);
            *islocal = true;
        } else {
            // Underestimator is tangent.
            compute_tangent(true, exponent, xref, constant, slope, success);

            // If reference point is left of -root*xlbglobal (c w.r.t. global bounds),
            // then tangent is not valid w.r.t. global bounds.
            *islocal = xref < -root * xlbglobal;

            // Tangent doesn't move after branching.
            *branchcand = false;
        }
    } else {
        // Compute the special point which decides between secant and tangent.
        let c = -xub * root;

        if xref <= c {
            // Overestimator is tangent.
            compute_tangent(true, exponent, xref, constant, slope, success);

            // If reference point is right of -root*xubglobal (c w.r.t. global
            // bounds), then tangent is not valid w.r.t. global bounds.
            *islocal = xref > -root * xubglobal;

            // Tangent doesn't move after branching.
            *branchcand = false;
        } else {
            // Overestimator is secant.
            compute_secant(scip, true, exponent, c, xub, constant, slope, success);
            *islocal = true;
        }
    }
}

/// `estimateHyperbolaPositive`: separation for positive hyperbola
/// - x^-2, x^-4 with x arbitrary
/// - x^-0.5, x^-1, x^-1.5, x^-3, x^-5 with x >= 0
#[allow(clippy::too_many_arguments)]
fn estimate_hyperbola_positive(
    scip: &Scip,
    exponent: f64,
    root: f64,
    overestimate: bool,
    xlb: f64,
    xub: f64,
    mut xref: f64,
    xlbglobal: f64,
    xubglobal: f64,
    constant: &mut f64,
    slope: &mut f64,
    islocal: &mut bool,
    branchcand: &mut bool,
    success: &mut bool,
) {
    let set = &scip.set;

    *success = false;

    if !overestimate {
        if xlb >= 0.0 || xub <= 0.0 {
            // Underestimate and fixed sign -> tangent.

            // Make sure xref has the same sign as xlb,xub.
            if xref < 0.0 && xlb >= 0.0 {
                xref = xlb;
            } else if xref > 0.0 && xub <= 0.0 {
                xref = xub;
            }

            if set.is_zero(xref) {
                // Estimator would need to have an (essentially) infinite scope; first
                // try to make up a better refpoint.
                if xub > 0.0 {
                    // Thus xlb >= 0.0; stay close to xlb (probably = 0).
                    xref = if !set.is_infinity(xub) { 0.9 * xlb + 0.1 * xub } else { 0.1 };
                } else {
                    // xub <= 0.0; stay close to xub (probably = 0).
                    xref = if !set.is_infinity(-xlb) { 0.1 * xlb + 0.9 * xub } else { 0.1 };
                }

                // If still close to 0, then also bounds are close to 0, then just give
                // up.
                if set.is_zero(xref) {
                    return;
                }
            }

            compute_tangent(false, exponent, xref, constant, slope, success);

            // Tangent will not change if branching on x (even if only locally valid,
            // see checks below).
            *branchcand = false;

            if eps_is_int(exponent / 2.0, 0.0) {
                // For even exponents (as in the picture): if x has fixed sign
                // globally, then our tangent is also globally valid; however, if x has
                // mixed sign, then it depends on the constellation between reference
                // point and global bounds, whether the tangent is globally valid (see
                // also the longer discussion for the mixed-sign underestimator below).
                if xref > 0.0 && xlbglobal < 0.0 {
                    // If on right side, then tangent is only locally valid if xref is too
                    // much to the left.
                    *islocal = xref < xlbglobal * root;
                } else if xref < 0.0 && xubglobal > 0.0 {
                    // If on left side, then tangent is only locally valid if xref is too
                    // much to the right.
                    *islocal = xref > xubglobal * root;
                } else {
                    *islocal = false;
                }
            } else {
                // For odd exponents, the tangent is only locally valid if the sign of x
                // is not fixed globally.
                *islocal = xlbglobal * xubglobal < 0.0;
            }
        } else {
            // Underestimate but mixed sign.
            if set.is_infinity(-xlb) {
                if set.is_infinity(xub) {
                    // Underestimator is constant 0, but that is globally valid.
                    *constant = 0.0;
                    *slope = 0.0;
                    *islocal = false;
                    *success = true;
                    return;
                }

                // Switch sign of x (mirror on ordinate) to make left bound finite and use
                // its estimator.
                estimate_hyperbola_positive(
                    scip,
                    exponent,
                    root,
                    overestimate,
                    -xub,
                    -xlb,
                    -xref,
                    -xubglobal,
                    -xlbglobal,
                    constant,
                    slope,
                    islocal,
                    branchcand,
                    success,
                );
                if *success {
                    *slope = -*slope;
                }
            } else {
                // The convex envelope of x^exponent for x in [xlb, infinity] is a line
                // (secant) between xlb and some positive coordinate xhat, and
                // x^exponent for x > xhat. Further, on [xlb,xub] with xub < xhat, the
                // convex envelope is the secant between xlb and xub.
                //
                // To find xhat, consider the affine-linear function l(x) = xlb^n + c *
                // (x - xlb) where n = exponent; we look for a value of x such that f(x)
                // and l(x) coincide and such that l(x) will be tangent to f(x) on that
                // point, that is xhat > 0 such that f(xhat) = l(xhat) and f'(xhat) =
                // l'(xhat) => 0 = xlb^n + (n-1) * xhat^n - n * xhat^(n-1) * xlb.
                //
                // Divide by xlb^n, one gets a polynomial that looks very much like the
                // one for signpower, but a sign is different (since this is *not
                // signed* power): 0 = 1 + (n-1) * y^n - n * y^(n-1) where y = xhat/xlb.
                //
                // The solution y < 0 (because xlb < 0 and we want xhat > 0) is what we
                // expect to be given as "root".
                if xref <= xlb * root {
                    // If the reference point is left of xhat (=xlb*root), then we can take
                    // the secant between xlb and root*xlb (= tangent at root*xlb).
                    // However, if xub < root*xlb, then we can tilt the estimator to be the
                    // secant between xlb and xub.
                    let xr = xlb * root;
                    compute_secant(scip, false, exponent, xlb, if xr <= xub { xr } else { xub }, constant, slope, success);
                    *islocal = true;
                } else {
                    // If reference point is right of xhat, then take the tangent at xref.
                    // This will still be underestimating for x in [xlb,0], too. The tangent
                    // is globally valid, if we had also generated w.r.t. global bounds.
                    compute_tangent(false, exponent, xref, constant, slope, success);
                    *islocal = xref < xlbglobal * root;
                    *branchcand = false;
                }
            }
        }
    } else {
        // Overestimate and mixed sign -> pole is within domain -> cannot
        // overestimate.
        if xlb < 0.0 && xub > 0.0 {
            return;
        }

        // Overestimate and fixed sign -> secant.
        compute_secant(scip, false, exponent, xlb, xub, constant, slope, success);
        *islocal = true;
    }
}

/// `estimateHyperbolaMixed`: separation for mixed-sign hyperbola
/// - x^-1, x^-3, x^-5 without x >= 0 (either x arbitrary or x negative)
#[allow(clippy::too_many_arguments)]
fn estimate_hyperbola_mixed(
    scip: &Scip,
    exponent: f64,
    overestimate: bool,
    xlb: f64,
    xub: f64,
    mut xref: f64,
    xlbglobal: f64,
    xubglobal: f64,
    constant: &mut f64,
    slope: &mut f64,
    islocal: &mut bool,
    branchcand: &mut bool,
    success: &mut bool,
) {
    let set = &scip.set;

    *success = false;

    if xub <= 0.0 {
        // x is negative.
        if !overestimate {
            // Underestimation -> secant.
            compute_secant(scip, false, exponent, xlb, xub, constant, slope, success);
            *islocal = true;
        } else if !set.is_zero(xlb / 10.0) {
            // Overestimation -> tangent.

            // Need to linearize left of 0.
            if xref > 0.0 {
                xref = 0.0;
            }

            if set.is_zero(xref) {
                // If xref is very close to 0.0, then slope would be infinite; try to
                // move closer to lower bound (if xlb < -10*eps).
                xref = if !set.is_infinity(-xlb) { 0.1 * xlb + 0.9 * xub } else { 0.1 };
            }

            compute_tangent(false, exponent, xref, constant, slope, success);

            // If x does not have a fixed sign globally, then our tangent is not
            // globally valid (power is not convex on global domain).
            *islocal = xlbglobal * xubglobal < 0.0;

            // Tangent doesn't move by branching.
            *branchcand = false;
        }
        // Else: xlb is very close to zero, xub is <= 0, so slope would be
        // infinite (for any reference point in [xlb, xub]) -> do not estimate.
    }
    // Else: x has mixed sign -> pole is within domain -> cannot estimate.
}

/// `buildPowEstimator`: builds an estimator for a power function; `root` is
/// the expression data's cached root.
#[allow(clippy::too_many_arguments)]
fn build_pow_estimator(
    scip: &Scip,
    root: &mut f64,
    overestimate: bool,
    childlb: f64,
    childub: f64,
    childglb: f64,
    childgub: f64,
    childintegral: bool,
    mut refpoint: f64,
    exponent: f64,
    coef: &mut f64,
    constant: &mut f64,
    success: &mut bool,
    islocal: &mut bool,
    branchcand: &mut bool,
) -> ScipResult {
    let isinteger = eps_is_int(exponent, 0.0);
    let iseven = isinteger && eps_is_int(exponent / 2.0, 0.0);

    if exponent == 2.0 {
        // Important special case: quadratic case; initialize, because
        // SCIPaddSquareXyz only adds to existing values.
        *success = true;
        *coef = 0.0;
        *constant = 0.0;

        if overestimate {
            add_square_secant(scip, 1.0, childlb, childub, coef, constant, success);
            *islocal = true; // Secants are only valid locally.
        } else {
            add_square_linearization(scip, 1.0, refpoint, childintegral, coef, constant, success);
            *islocal = false; // Linearizations are globally valid.
            *branchcand = false; // There is no improvement due to branching.
        }
    } else if exponent > 0.0 && iseven {
        estimate_parabola(scip, exponent, overestimate, childlb, childub, refpoint, constant, coef, islocal, success);
        // If estimate is locally valid, then we computed a secant and so
        // branching can improve it.
        *branchcand = *islocal;
    } else if exponent > 1.0 && childlb >= 0.0 {
        // Make sure we linearize in convex region (if we will linearize).
        if refpoint < 0.0 {
            refpoint = 0.0;
        }

        estimate_parabola(scip, exponent, overestimate, childlb, childub, refpoint, constant, coef, islocal, success);

        // If estimate is locally valid, then we computed a secant and so
        // branching can improve it.
        *branchcand = *islocal;

        // If odd power, then check whether tangent on parabola is also globally
        // valid, that is reference point is right of -root*global-lower-bound.
        if !*islocal && !iseven && childglb < 0.0 {
            if scip.set.is_infinity(-childglb) {
                *islocal = true;
            } else {
                if *root == INVALID {
                    *root = compute_signpower_root(scip, exponent)?;
                }
                *islocal = refpoint < *root * (-childglb);
            }
        }
    } else if exponent > 1.0 {
        // And !iseven && childlb < 0.0 due to previous if.

        // Compute root if not known yet; only needed if mixed sign (global child
        // ub > 0).
        if *root == INVALID && childgub > 0.0 {
            *root = compute_signpower_root(scip, exponent)?;
        }
        estimate_signedpower(
            scip,
            exponent,
            *root,
            overestimate,
            childlb,
            childub,
            refpoint,
            -childglb,
            childgub,
            constant,
            coef,
            islocal,
            branchcand,
            success,
        );
    } else if exponent < 0.0 && (iseven || childlb >= 0.0) {
        // Compute root if not known yet; only needed if mixed sign (globally) and
        // iseven.
        if *root == INVALID && iseven {
            *root = compute_hyperbola_root(scip, exponent)?;
        }
        estimate_hyperbola_positive(
            scip,
            exponent,
            *root,
            overestimate,
            childlb,
            childub,
            refpoint,
            childglb,
            childgub,
            constant,
            coef,
            islocal,
            branchcand,
            success,
        );
    } else if exponent < 0.0 {
        estimate_hyperbola_mixed(
            scip,
            exponent,
            overestimate,
            childlb,
            childub,
            refpoint,
            childglb,
            childgub,
            constant,
            coef,
            islocal,
            branchcand,
            success,
        );
    } else {
        estimate_root(scip, exponent, overestimate, childlb, childub, refpoint, constant, coef, islocal, success);

        // If estimate is locally valid, then we computed a secant, and so
        // branching can improve it.
        *branchcand = *islocal;
    }

    Ok(())
}

/// `addTangentRefpoints`: fills an array of reference points for estimating
/// on the convex side.
fn add_tangent_refpoints(scip: &Scip, exponent: f64, mut lb: f64, mut ub: f64, refpoints: &mut [f64]) {
    let maxabsbnd = INITLPMAXPOWVAL.powf(1.0 / exponent);

    // Make sure the absolute values of bounds are not too large.
    if ub > -maxabsbnd {
        lb = cmax(lb, -maxabsbnd);
    }
    if lb < maxabsbnd {
        ub = cmin(ub, maxabsbnd);
    }

    // In the case when ub < -maxabsbnd or lb > maxabsbnd, we still want to at
    // least make bounds finite.
    if scip.set.is_infinity(-lb) {
        lb = cmin(-10.0, ub - 0.1 * ub.abs());
    }
    if scip.set.is_infinity(ub) {
        ub = cmax(10.0, lb + 0.1 * lb.abs());
    }

    refpoints[0] = (7.0 * lb + ub) / 8.0;
    refpoints[1] = (lb + ub) / 2.0;
    refpoints[2] = (lb + 7.0 * ub) / 8.0;
}

/// `addSignpowerRefpoints`: fills an array of reference points for
/// sign(x)*abs(x)^n or x^n (n odd), where x has mixed signs.
///
/// The reference points are: the lower and upper bounds (one for secant and
/// one for tangent); and for the second tangent, the point on the convex part
/// of the function between the point deciding between tangent and secant, and
/// the corresponding bound.
fn add_signpower_refpoints(scip: &Scip, root: &mut f64, mut lb: f64, mut ub: f64, exponent: f64, underestimate: bool, refpoints: &mut [f64]) -> ScipResult {
    if (underestimate && scip.set.is_infinity(-lb)) || (!underestimate && scip.set.is_infinity(ub)) {
        return Ok(());
    }

    if *root == INVALID {
        *root = compute_signpower_root(scip, exponent)?;
    }

    // Make bounds finite (due to a previous if, only one can be infinite here).
    if scip.set.is_infinity(-lb) {
        lb = -ub * *root - 1.0;
    } else if scip.set.is_infinity(ub) {
        ub = -lb * *root + 1.0;
    }

    if underestimate {
        // Secant point.
        refpoints[0] = lb;

        // Tangent points, depending on the special point.
        if -lb * *root < ub - 2.0 {
            refpoints[2] = ub;
        }
        if -lb * *root < ub - 4.0 {
            refpoints[1] = (-lb * *root + ub) / 2.0;
        }
    }

    if !underestimate {
        // Secant point.
        refpoints[2] = ub;

        // Tangent points, depending on the special point.
        if -ub * *root > lb + 2.0 {
            refpoints[0] = lb;
        }
        if -ub * *root > lb + 4.0 {
            refpoints[1] = (lb - ub * *root) / 2.0;
        }
    }

    Ok(())
}

/// `chooseRefpointsPow`: choose reference points for adding initestimates
/// cuts for a power expression.
#[allow(clippy::too_many_arguments)]
fn choose_refpoints_pow(
    scip: &Scip,
    exponent: f64,
    root: &mut f64,
    lb: f64,
    ub: f64,
    refpointsunder: &mut [f64],
    refpointsover: &mut [f64],
    underestimate: bool,
    overestimate: bool,
) -> ScipResult {
    let even = eps_is_int(exponent, 0.0) && eps_is_int(exponent / 2.0, 0.0);

    let mut convex = false;
    let mut concave = false;
    let mixedsign = lb < 0.0 && ub > 0.0;

    // Convex case:
    // - parabola with an even degree or positive domain
    // - hyperbola with a positive domain
    // - even hyperbola with a negative domain
    if (exponent > 1.0 && (lb >= 0.0 || even)) || (exponent < 0.0 && lb >= 0.0) || (exponent < 0.0 && even && ub <= 0.0) {
        convex = true;
    }
    // Concave case:
    // - parabola or hyperbola with a negative domain and (due to previous if)
    //   an uneven degree
    // - root
    else if ub <= 0.0 || (exponent > 0.0 && exponent < 1.0) {
        concave = true;
    }

    if underestimate {
        if convex {
            add_tangent_refpoints(scip, exponent, lb, ub, refpointsunder);
        } else if (concave && !scip.set.is_infinity(-lb) && !scip.set.is_infinity(ub)) || (exponent < 0.0 && even && mixedsign) {
            // Concave with finite bounds or mixed even hyperbola: for secant,
            // refpoint doesn't matter, but we add it to signal that the
            // corresponding cut should be created.
            refpointsunder[0] = (lb + ub) / 2.0;
        } else if exponent > 1.0 && !even && mixedsign {
            // Mixed signpower.
            add_signpower_refpoints(scip, root, lb, ub, exponent, true, refpointsunder)?;
        }
        // Else: mixed odd hyperbola or an infinite bound.
    }

    if overestimate {
        if convex && !scip.set.is_infinity(-lb) && !scip.set.is_infinity(ub) {
            refpointsover[0] = (lb + ub) / 2.0;
        } else if concave {
            add_tangent_refpoints(scip, exponent, lb, ub, refpointsover);
        } else if exponent > 1.0 && !even && mixedsign {
            // Mixed signpower.
            add_signpower_refpoints(scip, root, lb, ub, exponent, false, refpointsover)?;
        }
        // Else: mixed hyperbola or an infinite bound.
    }

    Ok(())
}

/* callback methods of expression handler */

/// `comparePow`: compares two power expressions.
///
/// The order of two power (normal or signed) is base_1^expo_1 <
/// base_2^expo_2 if and only if base_1 < base2 or, base_1 = base_2 and
/// expo_1 < expo_2.
fn compare_pow(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let compareresult = expr::expr_compare(scip, scip.exprs[expr1].children[0], scip.exprs[expr2].children[0]);
    if compareresult != 0 {
        return compareresult;
    }

    let expo1 = get_exponent_expr_pow(scip, expr1);
    let expo2 = get_exponent_expr_pow(scip, expr2);

    if expo1 == expo2 {
        0
    } else if expo1 < expo2 {
        -1
    } else {
        1
    }
}

/// `simplifyPow`: simplifies a pow expression.
///
/// Evaluates the power function when its child is a value expression.
fn simplify_pow(scip: &mut Scip, expr: ExprId, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    let base = scip.exprs[expr].children[0];

    let exponent = get_exponent_expr_pow(scip, expr);

    // Enforces POW1.
    if exponent == 0.0 {
        // TODO: more checks?
        *simplifiedexpr = crate::expr_value::create_expr_value(scip, 1.0, ownercreate, ownercreatedata)?;
        return Ok(());
    }

    // Enforces POW2.
    if exponent == 1.0 {
        *simplifiedexpr = base;
        expr::expr_capture(scip, *simplifiedexpr);
        return Ok(());
    }

    // Enforces POW3.
    if expr::expr_is_value(scip, base) {
        let mut baseval = crate::expr_value::get_value_expr_value(scip, base);

        // The assert below was failing on st_e35 for baseval=-1e-15 and
        // fractional exponent in the subNLP heuristic; I assume that this was
        // because baseval was evaluated after variable fixings and that there
        // were just floating-point inaccuracies and 0 was meant, so I treat
        // -1e-15 as 0 here.
        if baseval < 0.0 && exponent % 1.0 != 0.0 && baseval > -scip.set.epsilon() {
            baseval = 0.0;
        }

        // TODO check if those are all important asserts.

        if baseval != 0.0 || exponent > 0.0 {
            *simplifiedexpr = crate::expr_value::create_expr_value(scip, baseval.powf(exponent), ownercreate, ownercreatedata)?;
            return Ok(());
        }
    }

    // Enforces POW11 (exp(x)^n = exp(n*x)).
    if is_expr_named(scip, base, "exp") {
        unimplemented!("the exponential expression handler is not ported");
    }

    // Enforces POW10.
    if expr::expr_is_var(scip, base) {
        let basevar = crate::expr_var::get_var_expr_var(scip, base);

        // TODO: if exponent is negative, we could fix the binary variable to 1.
        // However, this is a bit tricky because variables can not be tighten in
        // EXITPRE, where the simplify is also called.
        if crate::var::var_is_binary(scip, basevar) && exponent > 0.0 {
            *simplifiedexpr = base;
            expr::expr_capture(scip, *simplifiedexpr);
            return Ok(());
        }
    }

    let (expandmaxexponent, distribfracexponent) = {
        let d = hdlrdata(scip, expr);
        (d.expandmaxexponent, d.distribfracexponent)
    };

    if eps_is_int(exponent, 0.0) {
        // Enforces POW12 (abs(x)^n = x^n if n is even).
        if is_expr_named(scip, base, "abs") && (exponent as i32) % 2 == 0 {
            let child = scip.exprs[base].children[0];
            let mut newpow = create_expr_pow(scip, child, exponent, ownercreate, ownercreatedata)?;
            simplify_pow(scip, newpow, simplifiedexpr, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut newpow)?;

            return Ok(());
        }

        // Enforces POW5: given (pow n (prod 1.0 expr_1 ... expr_k)) we distribute
        // the exponent: -> (prod 1.0 (pow n expr_1) ... (pow n expr_k)).
        // Notes: - since base is simplified and its coefficient is 1.0 (SP8)
        //        - n is an integer (excluding 1 and 0; see POW1-2 above)
        if expr::expr_is_product(scip, base) {
            power_of_product(scip, base, exponent, simplifiedexpr, ownercreate, ownercreatedata)?;
            return Ok(());
        }

        // Enforces POW6: given (pow n (sum 0.0 coef expr)) we can move `pow`
        // inside `sum`: (pow n (sum 0.0 coef expr) ) -> (sum 0.0 coef^n (pow n
        // expr)).
        // Notes: - since base is simplified and its constant is 0, then coef !=
        //          1.0 (SS7)
        //        - n is an integer (excluding 1 and 0; see POW1-2 above)
        if expr::expr_is_sum(scip, base) && scip.exprs[base].children.len() == 1 && crate::expr_sum::get_constant_expr_sum(scip, base) == 0.0 {
            // Assert SS7 holds.

            // Create (pow n expr) and simplify it; note: we call simplifyPow
            // directly, since we know that `expr` is simplified.
            let newcoef = crate::expr_sum::get_coefs_expr_sum(scip, base)[0].powf(exponent);
            let child = scip.exprs[base].children[0];
            let mut aux = create_expr_pow(scip, child, exponent, ownercreate, ownercreatedata)?;
            let mut simplifiedaux = NONE;
            simplify_pow(scip, aux, &mut simplifiedaux, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut aux)?;

            // Create (sum (pow n expr)) and simplify it; this calls simplifySum
            // directly, since we know its children are simplified.
            let mut aux = crate::expr_sum::create_expr_sum(scip, &[simplifiedaux], Some(&[newcoef]), 0.0, ownercreate, ownercreatedata)?;
            call_expr_simplify(scip, aux, simplifiedexpr, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut aux)?;
            expr::expr_release(scip, &mut simplifiedaux)?;
            return Ok(());
        }

        // Enforces POW7 for exponent 2:
        // (const + sum alpha_i expr_i)^2 = sum alpha_i^2 expr_i^2
        // + sum_{j < i} 2*alpha_i alpha_j expr_i expr_j
        // + sum const alpha_i expr_i
        // TODO: put some limits on the number of children of the sum being
        // expanded.
        if expr::expr_is_sum(scip, base) && exponent == 2.0 && expandmaxexponent >= 2 {
            let basechildren = scip.exprs[base].children.clone();
            let basecoefs = crate::expr_sum::get_coefs_expr_sum(scip, base).to_vec();
            let baseconstant = crate::expr_sum::get_constant_expr_sum(scip, base);
            let nchildren = basechildren.len();
            let nexpandedchildren = nchildren * (nchildren + 1) / 2 + nchildren;
            let mut coefs = vec![0.0; nexpandedchildren];
            let mut expandedchildren = vec![NONE; nexpandedchildren];

            for i in 0..nchildren {
                let prodchildren0 = basechildren[i];

                // Create and simplify expr_i * expr_j.
                for j in 0..i {
                    let prodchildren1 = basechildren[j];
                    coefs[i * (i + 1) / 2 + j] = 2.0 * basecoefs[i] * basecoefs[j];

                    let mut expansionchild = crate::expr_product::create_expr_product(scip, &[prodchildren0, prodchildren1], 1.0, ownercreate, ownercreatedata)?;
                    // This calls simplifyProduct.
                    call_expr_simplify(scip, expansionchild, &mut expandedchildren[i * (i + 1) / 2 + j], ownercreate, ownercreatedata)?;
                    expr::expr_release(scip, &mut expansionchild)?;
                }
                // Create and simplify expr_i * expr_i.
                let prodchildren1 = basechildren[i];
                coefs[i * (i + 1) / 2 + i] = basecoefs[i] * basecoefs[i];

                let mut expansionchild = crate::expr_product::create_expr_product(scip, &[prodchildren0, prodchildren1], 1.0, ownercreate, ownercreatedata)?;
                // This calls simplifyProduct.
                call_expr_simplify(scip, expansionchild, &mut expandedchildren[i * (i + 1) / 2 + i], ownercreate, ownercreatedata)?;
                expr::expr_release(scip, &mut expansionchild)?;
            }
            // Create const * alpha_i expr_i.
            for i in 0..nchildren {
                coefs[i + nexpandedchildren - nchildren] = 2.0 * baseconstant * basecoefs[i];
                expandedchildren[i + nexpandedchildren - nchildren] = basechildren[i];
            }

            let mut constant = baseconstant;
            constant *= constant;
            // Create sum of all the above and simplify it with simplifySum since all
            // of its children are simplified!
            let mut expansion = crate::expr_sum::create_expr_sum(scip, &expandedchildren, Some(&coefs), constant, ownercreate, ownercreatedata)?;
            // This calls simplifySum.
            call_expr_simplify(scip, expansion, simplifiedexpr, ownercreate, ownercreatedata)?;

            // Release everything.
            expr::expr_release(scip, &mut expansion)?;
            // Release the *created* expanded children.
            for i in 0..nexpandedchildren - nchildren {
                expr::expr_release(scip, &mut expandedchildren[i])?;
            }

            return Ok(());
        }

        // Enforces POW7 for exponent > 2.
        if expr::expr_is_sum(scip, base) && exponent > 2.0 && exponent <= expandmaxexponent as f64 {
            *simplifiedexpr = crate::expr_sum::power_expr_sum(scip, base, exponent as i32, true, ownercreate, ownercreatedata)?;

            return Ok(());
        }
    } else {
        // Enforces POW9.
        //
        // FIXME code of POW6 is very similar.
        if scip.exprs[base].children.len() == 1
            && expr::expr_is_sum(scip, base)
            && crate::expr_sum::get_constant_expr_sum(scip, base) == 0.0
            && crate::expr_sum::get_coefs_expr_sum(scip, base)[0] >= 0.0
        {
            // Assert SS7 holds.

            // Create (pow n expr) and simplify it; note: we call simplifyPow
            // directly, since we know that `expr` is simplified.
            let child = scip.exprs[base].children[0];
            let mut aux = create_expr_pow(scip, child, exponent, ownercreate, ownercreatedata)?;
            let mut simplifiedaux = NONE;
            simplify_pow(scip, aux, &mut simplifiedaux, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut aux)?;

            // Create (sum (pow n expr)) and simplify it; this calls simplifySum
            // directly, since we know its child is simplified!
            let newcoef = crate::expr_sum::get_coefs_expr_sum(scip, base)[0].powf(exponent);
            let mut aux = crate::expr_sum::create_expr_sum(scip, &[simplifiedaux], Some(&[newcoef]), 0.0, ownercreate, ownercreatedata)?;
            call_expr_simplify(scip, aux, simplifiedexpr, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut aux)?;
            expr::expr_release(scip, &mut simplifiedaux)?;

            return Ok(());
        }

        // Enforces POW5a: given (pow n (prod 1.0 expr_1 ... expr_k)) we
        // distribute the exponent: -> (prod 1.0 (pow n expr_1) ... (pow n
        // expr_k)).
        // Notes: - since base is simplified and its coefficient is 1.0 (SP8)
        // TODO we can enable this more often by default when simplify makes use
        // of bounds on factors.
        if distribfracexponent && expr::expr_is_product(scip, base) {
            power_of_product(scip, base, exponent, simplifiedexpr, ownercreate, ownercreatedata)?;
            return Ok(());
        }
    }

    // Enforces POW8: given (pow n (pow expo expr)) we distribute the exponent:
    // -> (pow n*expo expr).
    // Notes: n is not 1 or 0, see POW1-2 above.
    if expr::expr_is_power(scip, base) {
        let baseexponent = get_exponent_expr_pow(scip, base);
        let newexponent = baseexponent * exponent;

        // Some checks (see POW8 definition in scip_expr.h) to make sure we don't
        // loose an implicit SCIPexprGetChildren(base)[0] >= 0 constraint:
        // if newexponent is fractional, then we will still need expr >= 0; if
        // both exponents were integer, then we never required and will not
        // require expr >= 0; if base exponent was an even integer, then we did not
        // require expr >= 0 (but may need to use |expr|^newexponent).
        if !eps_is_int(newexponent, 0.0) || (eps_is_int(baseexponent, 0.0) && eps_is_int(exponent, 0.0)) || (eps_is_int(baseexponent, 0.0) && (baseexponent as i32) % 2 == 0) {
            let mut aux;

            if eps_is_int(baseexponent, 0.0) && (baseexponent as i32) % 2 == 0 && (!eps_is_int(newexponent, 0.0) || (newexponent as i32) % 2 == 1) {
                // If base exponent was even integer and new exponent will be
                // fractional, then simplify to |expr|^newexponent to allow eval for
                // expr < 0. If base exponent was even integer and new exponent will be
                // odd integer, then simplify to |expr|^newexponent to preserve value
                // for expr < 0.
                unimplemented!("the absolute value expression handler is not ported");
            } else {
                let child = scip.exprs[base].children[0];
                aux = create_expr_pow(scip, child, newexponent, ownercreate, ownercreatedata)?;
            }

            simplify_pow(scip, aux, simplifiedexpr, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut aux)?;

            return Ok(());
        }
    }

    *simplifiedexpr = expr;

    // We have to capture it, since it must simulate a "normal" simplified call
    // in which a new expression is created.
    expr::expr_capture(scip, *simplifiedexpr);

    Ok(())
}

/// POW5 and POW5a of `simplifyPow`: distributes the exponent over the
/// factors of the product `base`.
fn power_of_product(scip: &mut Scip, base: ExprId, exponent: f64, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    // Create empty product.
    let mut auxproduct = crate::expr_product::create_expr_product(scip, &[], 1.0, ownercreate, ownercreatedata)?;

    for i in 0..scip.exprs[base].children.len() {
        // Create (pow n expr_i) and simplify.
        let child = scip.exprs[base].children[i];
        let mut aux = create_expr_pow(scip, child, exponent, ownercreate, ownercreatedata)?;
        let mut simplifiedaux = NONE;
        simplify_pow(scip, aux, &mut simplifiedaux, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut aux)?;

        // Append (pow n expr_i) to product.
        expr::expr_append_child(scip, auxproduct, simplifiedaux);
        expr::expr_release(scip, &mut simplifiedaux)?;
    }

    // Simplify (prod 1.0 (pow n expr_1) ... (pow n expr_k)); this calls
    // simplifyProduct directly, since we know its children are simplified.
    call_expr_simplify(scip, auxproduct, simplifiedexpr, ownercreate, ownercreatedata)?;
    expr::expr_release(scip, &mut auxproduct)?;
    Ok(())
}

/// `copyhdlrPow`.
fn copyhdlr_pow(scip: &mut Scip, _source: &Scip, _exprhdlr: ExprhdlrId) -> ScipResult {
    include_exprhdlr_pow(scip)
}

/// `freehdlrPow`.
fn freehdlr_pow(scip: &mut Scip, exprhdlr: ExprhdlrId) -> ScipResult {
    scip.exprhdlrs[exprhdlr].data = None;
    Ok(())
}

/// `copydataPow`.
fn copydata_pow(scip: &mut Scip, sourceexpr: ExprId) -> ScipResult<Box<dyn Any>> {
    Ok(Box::new(create_data(data(scip, sourceexpr).exponent)))
}

/// `freedataPow`.
fn freedata_pow(scip: &mut Scip, expr: ExprId) -> ScipResult {
    scip.exprs[expr].exprdata = None;
    Ok(())
}

/// `evalPow`.
fn eval_pow(scip: &mut Scip, expr: ExprId, val: &mut f64, _sol: SolId) -> ScipResult {
    let exponent = get_exponent_expr_pow(scip, expr);
    let base = scip.exprs[scip.exprs[expr].children[0]].evalvalue;

    *val = base.powf(exponent);

    // If there is a domain, pole, or range error, pow() should return some kind
    // of NaN, infinity, or HUGE_VAL; we could also work with floating point
    // exceptions or errno, but I am not sure this would be thread-safe.
    if !val.is_finite() || *val == f64::INFINITY || *val == f64::NEG_INFINITY {
        *val = INVALID;
    }

    Ok(())
}

/// `fwdiffPow`: derivative evaluation callback computing <gradient,
/// children.dot>; if expr is child^p, then computes p child^(p-1)
/// dot(child).
fn fwdiff_pow(scip: &mut Scip, expr: ExprId, dot: &mut f64, _direction: SolId) -> ScipResult {
    let child = scip.exprs[expr].children[0];

    let exponent = get_exponent_expr_pow(scip, expr);

    // x^exponent is not differentiable for x = 0 and exponent in ]0,1[.
    if exponent > 0.0 && exponent < 1.0 && scip.exprs[child].evalvalue == 0.0 {
        *dot = INVALID;
    } else {
        *dot = exponent * scip.exprs[child].evalvalue.powf(exponent - 1.0) * scip.exprs[child].dot;
    }

    Ok(())
}

/// `bwfwdiffPow`: computes partial/partial child ( <gradient, children.dot>
/// ); if expr is child^n, then computes n * (n - 1) child^(n-2) dot(child).
fn bwfwdiff_pow(scip: &mut Scip, expr: ExprId, _childidx: i32, bardot: &mut f64, _direction: SolId) -> ScipResult {
    let child = scip.exprs[expr].children[0];

    let exponent = get_exponent_expr_pow(scip, expr);

    // x^exponent is not twice differentiable for x = 0 and exponent in ]0,1[ u
    // ]1,2[.
    if exponent > 0.0 && exponent < 2.0 && scip.exprs[child].evalvalue == 0.0 && exponent != 1.0 {
        *bardot = INVALID;
    } else {
        *bardot = exponent * (exponent - 1.0) * scip.exprs[child].evalvalue.powf(exponent - 2.0) * scip.exprs[child].dot;
    }

    Ok(())
}

/// `bwdiffPow`.
fn bwdiff_pow(scip: &mut Scip, expr: ExprId, _childidx: i32, val: &mut f64) -> ScipResult {
    let child = scip.exprs[expr].children[0];

    let childval = scip.exprs[child].evalvalue;

    let exponent = get_exponent_expr_pow(scip, expr);

    // x^exponent is not differentiable for x = 0 and exponent in ]0,1[.
    if exponent > 0.0 && exponent < 1.0 && childval == 0.0 {
        *val = INVALID;
    } else {
        *val = exponent * childval.powf(exponent - 1.0);
    }

    Ok(())
}

/// Moves a bound of the child of `pow(., exponent)` with `exponent < 0` out
/// of `(-minzerodistance, minzerodistance)`, warning once (see
/// `intevalPow` and `reversepropPow`).
fn avoid_pole(scip: &mut Scip, expr: ExprId, exponent: f64, childinterval: &mut Interval, upperwarning: &str) {
    let verblevel = scip.set.disp_verblevel;
    let d = hdlrdata_mut(scip, expr);

    if d.minzerodistance > 0.0 {
        // Avoid small interval around 0 if possible.
        if childinterval.inf > -d.minzerodistance && childinterval.inf < d.minzerodistance {
            if !d.warnedonpole && verblevel > 0 {
                log::info!(
                    "Changing lower bound for child of pow(.,{exponent}) from {} to {}.\nCheck your model formulation or use option expr/{POWEXPRHDLR_NAME}/minzerodistance to avoid this warning.",
                    childinterval.inf,
                    d.minzerodistance
                );
                d.warnedonpole = true;
            }
            childinterval.inf = d.minzerodistance;
        } else if childinterval.sup < d.minzerodistance && childinterval.sup > -d.minzerodistance {
            if !d.warnedonpole && verblevel > 0 {
                log::info!(
                    "Changing {upperwarning} bound for child of pow(.,{exponent}) from {} to {}.\nCheck your model formulation or use option expr/{POWEXPRHDLR_NAME}/minzerodistance to avoid this warning.",
                    childinterval.sup,
                    -d.minzerodistance
                );
                d.warnedonpole = true;
            }
            childinterval.sup = -d.minzerodistance;
        }
    }
}

/// `intevalPow`.
fn inteval_pow(scip: &mut Scip, expr: ExprId, interval: &mut Interval, _intevalvar: Option<ExprIntEvalVar>, _intevalvardata: usize) -> ScipResult {
    let mut childinterval = scip.exprs[scip.exprs[expr].children[0]].activity;

    let exponent = get_exponent_expr_pow(scip, expr);

    if exponent < 0.0 {
        // Avoid small interval around 0 if possible, see also reversepropPow.
        avoid_pole(scip, expr, exponent, &mut childinterval, "upper");
    }

    if interval::is_empty(INTERVAL_INFINITY, childinterval) {
        interval.set_empty();
        return Ok(());
    }

    interval::power_scalar(INTERVAL_INFINITY, interval, childinterval, exponent);

    // Make sure 0^negative is an empty interval, as some other codes do not
    // handle intervals like [inf,inf] well.
    // TODO maybe change SCIPintervalPowerScalar?
    if exponent < 0.0 && childinterval.inf == 0.0 && childinterval.sup == 0.0 {
        interval.set_empty();
    }

    Ok(())
}

/// `estimatePow`: expression estimator callback.
#[allow(clippy::too_many_arguments)]
fn estimate_pow(
    scip: &mut Scip,
    expr: ExprId,
    localbounds: &[Interval],
    globalbounds: &[Interval],
    refpoint: &[f64],
    overestimate: bool,
    _targetvalue: f64,
    coefs: &mut [f64],
    constant: &mut f64,
    islocal: &mut bool,
    success: &mut bool,
    branchcand: &mut [bool],
) -> ScipResult {
    *success = false;

    // Get aux variables: we over- or underestimate childvar^exponent.
    let child = scip.exprs[expr].children[0];

    // We can not generate a cut at +/- infinity.
    if scip.set.is_infinity(refpoint[0].abs()) {
        return Ok(());
    }

    let mut childlb = localbounds[0].inf;
    let childub = localbounds[0].sup;

    let exponent = data(scip, expr).exponent;

    // If child is constant, then return a constant estimator; this can help
    // with small infeasibilities if boundtightening is relaxing bounds too much.
    if childlb == childub {
        coefs[0] = 0.0;
        *constant = childlb.powf(exponent);
        *success = true;
        *islocal = globalbounds[0].inf != globalbounds[0].sup;
        branchcand[0] = false;
        return Ok(());
    }

    let isinteger = eps_is_int(exponent, 0.0);

    // If exponent is not integral, then child must be non-negative.
    if !isinteger && childlb < 0.0 {
        // Somewhere we should have tightened the bound on x, but small
        // tightening are not always applied by SCIP; it is ok to do this
        // tightening here, but let's assert that we were close to 0.0 already.
        childlb = 0.0;
    }

    let childintegral = scip.exprs[child].integrality != ImplintType::None;
    let mut root = data(scip, expr).root;
    build_pow_estimator(
        scip,
        &mut root,
        overestimate,
        childlb,
        childub,
        globalbounds[0].inf,
        globalbounds[0].sup,
        childintegral,
        cmax(childlb, refpoint[0]),
        exponent,
        &mut coefs[0],
        constant,
        success,
        islocal,
        &mut branchcand[0],
    )?;
    data_mut(scip, expr).root = root;

    Ok(())
}

/// `reversepropPow`: expression reverse propagaton callback.
fn reverseprop_pow(scip: &mut Scip, expr: ExprId, bounds: Interval, childrenbounds: &mut [Interval], infeasible: &mut bool) -> ScipResult {
    let exponent = get_exponent_expr_pow(scip, expr);
    let child = childrenbounds[0];
    let mut interval = Interval::default();

    if interval::is_empty(INTERVAL_INFINITY, child) {
        *infeasible = true;
        return Ok(());
    }

    if interval::is_entire(INTERVAL_INFINITY, bounds) {
        // If exponent is not integral, then make sure that child is
        // non-negative.
        if !eps_is_int(exponent, 0.0) && child.inf < 0.0 {
            interval.set_bounds(0.0, child.sup);
        } else {
            return Ok(());
        }
    } else {
        // f = pow(c0, alpha) -> c0 = pow(f, 1/alpha).
        interval::power_scalar_inverse(INTERVAL_INFINITY, &mut interval, child, exponent, bounds);
    }

    if exponent < 0.0 {
        // Push lower bound from >= -epsilon to >= epsilon to avoid pole at 0
        // (domain error); push upper bound from <= epsilon to <= -epsilon to
        // avoid pole at 0 (domain error); this can lead to a cutoff if domain
        // would otherwise be very close around 0.
        avoid_pole(scip, expr, exponent, &mut interval, "lower");
    }

    childrenbounds[0] = interval;

    Ok(())
}

/// `SCIP_EXPR_MAXINITESTIMATES`: maximal number of estimators an
/// initestimates callback can return.
pub const EXPR_MAXINITESTIMATES: i32 = 10;

/// `initestimatesPow`: initial estimates callback for a power expression.
fn initestimates_pow(scip: &mut Scip, expr: ExprId, bounds: &[Interval], overestimate: bool, coefs: &mut [Vec<f64>], constant: &mut [f64], nreturned: &mut i32) -> ScipResult {
    let mut refpointsunder = [INVALID, INVALID, INVALID];
    let mut refpointsover = [INVALID, INVALID, INVALID];
    let overest = [false, false, false, true, true, true];

    let child = scip.exprs[expr].children[0];

    let mut childlb = bounds[0].inf;
    let childub = bounds[0].sup;

    // If child is essentially constant, then there should be no point in
    // separation.
    if scip.set.is_eq(childlb, childub) {
        return Ok(());
    }

    let exponent = data(scip, expr).exponent;
    let mut root = data(scip, expr).root;

    let isinteger = eps_is_int(exponent, 0.0);

    // If exponent is not integral, then child must be non-negative.
    if !isinteger && childlb < 0.0 {
        // Somewhere we should have tightened the bound on x, but small
        // tightening are not always applied by SCIP; it is ok to do this
        // tightening here, but let's assert that we were close to 0.0 already.
        childlb = 0.0;
    }

    // TODO simplify to get 3 refpoints for either under- or overestimate.
    let r = choose_refpoints_pow(
        scip,
        exponent,
        &mut root,
        childlb,
        childub,
        &mut refpointsunder,
        &mut refpointsover,
        !overestimate,
        overestimate,
    );
    data_mut(scip, expr).root = root;
    r?;

    let childintegral = scip.exprs[child].integrality != ImplintType::None;

    let mut i = 0;
    while i < 6 && *nreturned < EXPR_MAXINITESTIMATES {
        if (overest[i] && !overestimate) || (!overest[i] && overestimate) {
            i += 1;
            continue;
        }

        let refpoint = if overest[i] { refpointsover[i % 3] } else { refpointsunder[i % 3] };

        if refpoint == INVALID {
            i += 1;
            continue;
        }

        let mut branchcand = true;
        let mut success = false;
        let mut islocal = false;
        let n = *nreturned as usize;
        let r = build_pow_estimator(
            scip,
            &mut root,
            overest[i],
            childlb,
            childub,
            childlb,
            childub,
            childintegral,
            refpoint,
            exponent,
            &mut coefs[n][0],
            &mut constant[n],
            &mut success,
            &mut islocal,
            &mut branchcand,
        );
        data_mut(scip, expr).root = root;
        r?;

        if success {
            *nreturned += 1;
        }
        i += 1;
    }

    Ok(())
}

/// `hashPow`.
fn hash_pow(_scip: &mut Scip, _expr: ExprId, hashkey: &mut u32, childrenhashes: &[u32]) -> ScipResult {
    // TODO include exponent into hashkey.
    *hashkey = powexprhdlr_hashkey();
    *hashkey ^= childrenhashes[0];

    Ok(())
}

/// `curvaturePow`.
fn curvature_pow(scip: &mut Scip, expr: ExprId, exprcurvature: ExprCurv, success: &mut bool, childcurv: &mut [ExprCurv]) -> ScipResult {
    let exponent = get_exponent_expr_pow(scip, expr);
    let child = scip.exprs[expr].children[0];

    expr::expr_eval_activity(scip, child)?;
    let childinterval = scip.exprs[child].activity;

    childcurv[0] = exprcurv_power_inv(childinterval, exponent, exprcurvature);
    // SCIPexprcurvPowerInv return unknown actually means that curv cannot be
    // obtained.
    *success = childcurv[0] != ExprCurv::Unknown;

    Ok(())
}

/// `monotonicityPow`.
fn monotonicity_pow(scip: &mut Scip, expr: ExprId, _childidx: i32, result: &mut Monotone) -> ScipResult {
    let child = scip.exprs[expr].children[0];
    expr::expr_eval_activity(scip, child)?;
    let interval = scip.exprs[child].activity;

    *result = Monotone::Unknown;
    let inf = interval.inf;
    let sup = interval.sup;
    let exponent = get_exponent_expr_pow(scip, expr);
    let expisint = eps_is_int(exponent, 0.0);

    if expisint {
        let expisodd = (exponent / 2.0).ceil() != exponent / 2.0;

        if expisodd {
            // x^1, x^3, ...
            if exponent >= 0.0 {
                *result = Monotone::Inc;
            }
            // ..., x^-3, x^-1 are decreasing if 0 is not in ]inf,sup[.
            else if inf >= 0.0 || sup <= 0.0 {
                *result = Monotone::Dec;
            }
        }
        // ..., x^-4, x^-2, x^2, x^4, ...
        else {
            // Function is not monotone if 0 is in ]inf,sup[.
            if inf >= 0.0 {
                *result = if exponent >= 0.0 { Monotone::Inc } else { Monotone::Dec };
            } else if sup <= 0.0 {
                *result = if exponent >= 0.0 { Monotone::Dec } else { Monotone::Inc };
            }
        }
    } else {
        // Note that the expression is not defined for negative input values
        // - increasing iff exponent >= 0
        // - decreasing iff exponent <= 0
        *result = if exponent >= 0.0 { Monotone::Inc } else { Monotone::Dec };
    }

    Ok(())
}

/// `integralityPow`.
fn integrality_pow(scip: &mut Scip, expr: ExprId, integrality: &mut ImplintType) -> ScipResult {
    let child = scip.exprs[expr].children[0];

    let exponent = get_exponent_expr_pow(scip, expr);
    let expisint = eps_is_int(exponent, 0.0);

    // Maintain child integrality if exponent is non-negative and integral.
    *integrality = if expisint && exponent >= 0.0 {
        scip.exprs[child].integrality
    } else {
        ImplintType::None
    };

    Ok(())
}

/// `simplifySignpower`: simplifies a signpower expression.
fn simplify_signpower(scip: &mut Scip, expr: ExprId, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    let base = scip.exprs[expr].children[0];

    let exponent = get_exponent_expr_pow(scip, expr);

    // Enforces SPOW2.
    if exponent == 1.0 {
        *simplifiedexpr = base;
        expr::expr_capture(scip, *simplifiedexpr);
        return Ok(());
    }

    // Enforces SPOW3.
    if expr::expr_is_value(scip, base) {
        let baseval = crate::expr_value::get_value_expr_value(scip, base);

        *simplifiedexpr = crate::expr_value::create_expr_value(scip, sign(baseval) * baseval.abs().powf(exponent), ownercreate, ownercreatedata)?;

        return Ok(());
    }

    // Enforces SPOW11 (exp(x)^n = exp(n*x)); since exp() is always nonnegative,
    // we can treat signpower as normal power here.
    if is_expr_named(scip, base, "exp") {
        unimplemented!("the exponential expression handler is not ported");
    }

    // Enforces SPOW6.
    if eps_is_int(exponent, 0.0) && (exponent as i32) % 2 == 1 {
        // We do not just change the expression data of expression to say it is a
        // normal power, since, at the moment, simplify identifies that
        // expressions changed by checking that the pointer of the input
        // expression is different from the returned (simplified) expression.
        let mut aux = create_expr_pow(scip, base, exponent, ownercreate, ownercreatedata)?;

        simplify_pow(scip, aux, simplifiedexpr, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut aux)?;

        return Ok(());
    }

    // Enforces SPOW10.
    if expr::expr_is_var(scip, base) {
        let basevar = crate::expr_var::get_var_expr_var(scip, base);

        if crate::var::var_is_binary(scip, basevar) {
            *simplifiedexpr = base;
            expr::expr_capture(scip, *simplifiedexpr);
            return Ok(());
        }
    }

    // TODO if( SCIPisExprSignpower(scip, base) ...

    // Enforces SPOW8: given (signpow n (pow expo expr)) we distribute the
    // exponent: -> (signpow n*expo expr) for even n (i.e., sign(x^n) * |x|^n =
    // 1 * x^n).
    // Notes: n is an even integer (see SPOW6 above).
    // FIXME: doesn't this extend to any exponent? (see expr_pow.c)
    if eps_is_int(exponent, 0.0) && expr::expr_is_power(scip, base) {
        let newexponent = get_exponent_expr_pow(scip, base) * exponent;
        let child = scip.exprs[base].children[0];
        let mut aux = create_expr_signpower(scip, child, newexponent, ownercreate, ownercreatedata)?;
        simplify_signpower(scip, aux, simplifiedexpr, ownercreate, ownercreatedata)?;

        expr::expr_release(scip, &mut aux)?;

        return Ok(());
    }

    // Enforces SPOW9.
    if expr::expr_is_sum(scip, base) && scip.exprs[base].children.len() == 1 && crate::expr_sum::get_constant_expr_sum(scip, base) == 0.0 {
        // Assert SS7 holds.

        // Create (signpow n expr) and simplify it; note: we call
        // simplifySignpower directly, since we know that `expr` is simplified.
        let child = scip.exprs[base].children[0];
        let mut aux = create_expr_signpower(scip, child, exponent, ownercreate, ownercreatedata)?;
        let c0 = crate::expr_sum::get_coefs_expr_sum(scip, base)[0];
        let newcoef = sign(c0) * c0.abs().powf(exponent);
        let mut simplifiedaux = NONE;
        simplify_signpower(scip, aux, &mut simplifiedaux, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut aux)?;

        // Create (sum (signpow n expr)) and simplify it; this calls simplifySum
        // directly, since we know its child is simplified.
        let mut aux = crate::expr_sum::create_expr_sum(scip, &[simplifiedaux], Some(&[newcoef]), 0.0, ownercreate, ownercreatedata)?;
        call_expr_simplify(scip, aux, simplifiedexpr, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut aux)?;
        expr::expr_release(scip, &mut simplifiedaux)?;
        return Ok(());
    }

    *simplifiedexpr = expr;

    // We have to capture it, since it must simulate a "normal" simplified call
    // in which a new expression is created.
    expr::expr_capture(scip, *simplifiedexpr);

    Ok(())
}

/// `copyhdlrSignpower`.
fn copyhdlr_signpower(scip: &mut Scip, _source: &Scip, _exprhdlr: ExprhdlrId) -> ScipResult {
    include_exprhdlr_signpower(scip)
}

/// `parseSignpower`: expression parse callback.
#[allow(clippy::too_many_arguments)]
fn parse_signpower(
    scip: &mut Scip,
    _exprhdlr: ExprhdlrId,
    string: &[u8],
    pos: usize,
    endstring: &mut usize,
    expr: &mut ExprId,
    success: &mut bool,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    let at = |p: usize| string.get(p).copied().unwrap_or(0);

    // Parse child expression string.
    let (mut childexpr, end) = crate::scip_expr::parse_expr(scip, string, pos, ownercreate, ownercreatedata)?;
    *endstring = end;

    let mut p = *endstring;
    while at(p) == b' ' {
        p += 1;
    }

    if at(p) != b',' {
        log::error!("Expected comma after first argument of signpower().");
        return Err(Retcode::ReadError);
    }
    p += 1;

    let Some((exponent, end)) = crate::api::parse_real(scip, string, p) else {
        log::error!("Expected numeric exponent for second argument of signpower().");
        return Err(Retcode::ReadError);
    };
    *endstring = end;

    if exponent <= 1.0 || !exponent.is_finite() || scip.set.is_infinity(exponent) {
        log::error!("Expected finite exponent >= 1.0 for signpower().");
        return Err(Retcode::ReadError);
    }

    // Create signpower expression.
    *expr = create_expr_signpower(scip, childexpr, exponent, ownercreate, ownercreatedata)?;

    // Release child expression since it has been captured by the signpower
    // expression.
    expr::expr_release(scip, &mut childexpr)?;

    *success = true;

    Ok(())
}

/// `evalSignpower`.
fn eval_signpower(scip: &mut Scip, expr: ExprId, val: &mut f64, _sol: SolId) -> ScipResult {
    let exponent = get_exponent_expr_pow(scip, expr);
    let base = scip.exprs[scip.exprs[expr].children[0]].evalvalue;

    *val = sign(base) * base.abs().powf(exponent);

    // If there is a range error, pow() should return some kind of infinity, or
    // HUGE_VAL; we could also work with floating point exceptions or errno, but
    // I am not sure this would be thread-safe.
    if !val.is_finite() || *val == f64::INFINITY || *val == f64::NEG_INFINITY {
        *val = INVALID;
    }

    Ok(())
}

/// `bwdiffSignpower`.
fn bwdiff_signpower(scip: &mut Scip, expr: ExprId, _childidx: i32, val: &mut f64) -> ScipResult {
    let child = scip.exprs[expr].children[0];

    let childval = scip.exprs[child].evalvalue;

    let exponent = get_exponent_expr_pow(scip, expr);

    *val = exponent * childval.abs().powf(exponent - 1.0);

    Ok(())
}

/// `intevalSignpower`.
fn inteval_signpower(scip: &mut Scip, expr: ExprId, interval: &mut Interval, _intevalvar: Option<ExprIntEvalVar>, _intevalvardata: usize) -> ScipResult {
    let childinterval = scip.exprs[scip.exprs[expr].children[0]].activity;
    if interval::is_empty(INTERVAL_INFINITY, childinterval) {
        interval.set_empty();
        return Ok(());
    }

    interval::sign_power_scalar(INTERVAL_INFINITY, interval, childinterval, get_exponent_expr_pow(scip, expr));

    Ok(())
}

/// `estimateSignpower`.
#[allow(clippy::too_many_arguments)]
fn estimate_signpower(
    scip: &mut Scip,
    expr: ExprId,
    localbounds: &[Interval],
    globalbounds: &[Interval],
    refpoint: &[f64],
    overestimate: bool,
    _targetvalue: f64,
    coefs: &mut [f64],
    constant: &mut f64,
    islocal: &mut bool,
    success: &mut bool,
    branchcand: &mut [bool],
) -> ScipResult {
    *success = false;

    // We can not generate a cut at +/- infinity.
    if scip.set.is_infinity(refpoint[0].abs()) {
        return Ok(());
    }

    let childlb = localbounds[0].inf;
    let childub = localbounds[0].sup;

    let childglb = globalbounds[0].inf;
    let childgub = globalbounds[0].sup;

    let exponent = data(scip, expr).exponent;
    let mut root = data(scip, expr).root;

    // If child is constant, then return a constant estimator; this can help
    // with small infeasibilities if boundtightening is relaxing bounds too much.
    if childlb == childub {
        coefs[0] = 0.0;
        *constant = sign(childlb) * childlb.abs().powf(exponent);
        *success = true;
        *islocal = childglb != childgub;
        branchcand[0] = false;
        return Ok(());
    }

    if childlb >= 0.0 {
        estimate_parabola(
            scip,
            exponent,
            overestimate,
            childlb,
            childub,
            cmax(0.0, refpoint[0]),
            constant,
            &mut coefs[0],
            islocal,
            success,
        );

        branchcand[0] = *islocal;

        // If odd or signed power, then check whether tangent on parabola is also
        // globally valid, that is reference point is right of
        // -root*global-lower-bound.
        if !*islocal && childglb < 0.0 {
            if scip.set.is_infinity(-childglb) {
                *islocal = true;
            } else {
                if root == INVALID {
                    root = compute_signpower_root(scip, exponent)?;
                    data_mut(scip, expr).root = root;
                }
                *islocal = refpoint[0] < root * (-childglb);
            }
        }
    } else {
        // And childlb < 0.0 due to previous if.

        // Compute root if not known yet; only needed if mixed sign (global child
        // ub > 0).
        if root == INVALID && childgub > 0.0 {
            root = compute_signpower_root(scip, exponent)?;
            data_mut(scip, expr).root = root;
        }
        estimate_signedpower(
            scip,
            exponent,
            root,
            overestimate,
            childlb,
            childub,
            refpoint[0],
            childglb,
            childgub,
            constant,
            &mut coefs[0],
            islocal,
            &mut branchcand[0],
            success,
        );
    }

    Ok(())
}

/// `initestimatesSignpower`: initial estimates callback for a signpower
/// expression.
fn initestimates_signpower(
    scip: &mut Scip,
    expr: ExprId,
    bounds: &[Interval],
    overestimate: bool,
    coefs: &mut [Vec<f64>],
    constant: &mut [f64],
    nreturned: &mut i32,
) -> ScipResult {
    let mut refpointsunder = [INVALID, INVALID, INVALID];
    let mut refpointsover = [INVALID, INVALID, INVALID];
    let overest = [false, false, false, true, true, true];

    let childlb = bounds[0].inf;
    let childub = bounds[0].sup;

    // If child is essentially constant, then there should be no point in
    // separation.
    if scip.set.is_eq(childlb, childub) {
        return Ok(());
    }

    let exponent = data(scip, expr).exponent;
    let mut root = data(scip, expr).root;

    if childlb >= 0.0 {
        if !overestimate {
            add_tangent_refpoints(scip, exponent, childlb, childub, &mut refpointsunder);
        }
        if overestimate && !scip.set.is_infinity(childub) {
            refpointsover[0] = (childlb + childub) / 2.0;
        }
    } else if childub <= 0.0 {
        if !overestimate && !scip.set.is_infinity(-childlb) {
            refpointsunder[0] = (childlb + childub) / 2.0;
        }
        if overestimate {
            add_tangent_refpoints(scip, exponent, childlb, childub, &mut refpointsunder);
        }
    } else {
        let r = add_signpower_refpoints(scip, &mut root, childlb, childub, exponent, !overestimate, &mut refpointsunder);
        data_mut(scip, expr).root = root;
        r?;
    }

    // Add cuts for all refpoints.
    let mut i = 0;
    while i < 6 && *nreturned < EXPR_MAXINITESTIMATES {
        if (overest[i] && !overestimate) || (!overest[i] && overestimate) {
            i += 1;
            continue;
        }

        let refpoint = if overest[i] { refpointsover[i % 3] } else { refpointsunder[i % 3] };
        if refpoint == INVALID {
            i += 1;
            continue;
        }

        let n = *nreturned as usize;
        let mut islocal = false;
        let mut success = false;
        if childlb >= 0.0 {
            estimate_parabola(
                scip,
                exponent,
                overest[i],
                childlb,
                childub,
                refpoint,
                &mut constant[n],
                &mut coefs[n][0],
                &mut islocal,
                &mut success,
            );
        } else {
            // Compute root if not known yet; only needed if mixed sign (global
            // child ub > 0).
            if root == INVALID && childub > 0.0 {
                root = compute_signpower_root(scip, exponent)?;
                data_mut(scip, expr).root = root;
            }
            let mut branchcand = true;
            estimate_signedpower(
                scip,
                exponent,
                root,
                overest[i],
                childlb,
                childub,
                refpoint,
                childlb,
                childub,
                &mut constant[n],
                &mut coefs[n][0],
                &mut islocal,
                &mut branchcand,
                &mut success,
            );
        }

        if success {
            *nreturned += 1;
        }
        i += 1;
    }

    Ok(())
}

/// `reversepropSignpower`.
fn reverseprop_signpower(scip: &mut Scip, expr: ExprId, bounds: Interval, childrenbounds: &mut [Interval], _infeasible: &mut bool) -> ScipResult {
    let exponent = get_exponent_expr_pow(scip, expr);

    if interval::is_entire(INTERVAL_INFINITY, bounds) {
        return Ok(());
    }

    // f = pow(c0, alpha) -> c0 = pow(f, 1/alpha).
    let mut exprecip = Interval::default();
    exprecip.set(exponent);
    let e = exprecip;
    interval::reciprocal(INTERVAL_INFINITY, &mut exprecip, e);
    let mut interval = Interval::default();
    if exprecip.inf == exprecip.sup {
        interval::sign_power_scalar(INTERVAL_INFINITY, &mut interval, bounds, exprecip.inf);
    } else {
        let mut interval1 = Interval::default();
        let mut interval2 = Interval::default();
        interval::sign_power_scalar(INTERVAL_INFINITY, &mut interval1, bounds, exprecip.inf);
        interval::sign_power_scalar(INTERVAL_INFINITY, &mut interval2, bounds, exprecip.sup);
        interval::unify(&mut interval, interval1, interval2);
    }

    childrenbounds[0] = interval;

    Ok(())
}

/// `hashSignpower`.
fn hash_signpower(_scip: &mut Scip, _expr: ExprId, hashkey: &mut u32, childrenhashes: &[u32]) -> ScipResult {
    // TODO include exponent into hashkey.
    *hashkey = signpowexprhdlr_hashkey();
    *hashkey ^= childrenhashes[0];

    Ok(())
}

/// `curvatureSignpower`.
fn curvature_signpower(scip: &mut Scip, expr: ExprId, exprcurvature: ExprCurv, success: &mut bool, childcurv: &mut [ExprCurv]) -> ScipResult {
    let child = scip.exprs[expr].children[0];

    expr::expr_eval_activity(scip, child)?;
    let childinterval = scip.exprs[child].activity;

    if exprcurvature == ExprCurv::Convex {
        // Signpower is only convex if argument is convex and non-negative.
        childcurv[0] = ExprCurv::Convex;
        *success = childinterval.inf >= 0.0;
    } else if exprcurvature == ExprCurv::Concave {
        // Signpower is only concave if argument is concave and non-positive.
        childcurv[0] = ExprCurv::Concave;
        *success = childinterval.sup <= 0.0;
    } else {
        *success = false;
    }

    Ok(())
}

/// `monotonicitySignpower`.
fn monotonicity_signpower(_scip: &mut Scip, _expr: ExprId, _childidx: i32, result: &mut Monotone) -> ScipResult {
    *result = Monotone::Inc;
    Ok(())
}

/// `SCIPincludeExprhdlrPow`: creates the handler for power expression and
/// includes it into SCIP.
pub fn include_exprhdlr_pow(scip: &mut Scip) -> ScipResult {
    let exprhdlrdata = ExprhdlrData {
        minzerodistance: 0.0,
        expandmaxexponent: 0,
        distribfracexponent: false,
        warnedonpole: false,
    };

    let exprhdlr = crate::scip_expr::include_exprhdlr(
        scip,
        POWEXPRHDLR_NAME,
        Some(POWEXPRHDLR_DESC),
        POWEXPRHDLR_PRECEDENCE,
        eval_pow,
        Some(Box::new(exprhdlrdata)),
    )?;

    let h = &mut scip.exprhdlrs[exprhdlr];
    h.copyhdlr = Some(copyhdlr_pow);
    h.freehdlr = Some(freehdlr_pow);
    h.copydata = Some(copydata_pow);
    h.freedata = Some(freedata_pow);
    h.simplify = Some(simplify_pow);
    h.print = true;
    h.inteval = Some(inteval_pow);
    h.initestimates = Some(initestimates_pow);
    h.estimate = Some(estimate_pow);
    h.reverseprop = Some(reverseprop_pow);
    h.hash = Some(hash_pow);
    h.compare = Some(compare_pow);
    h.bwdiff = Some(bwdiff_pow);
    h.fwdiff = Some(fwdiff_pow);
    h.bwfwdiff = Some(bwfwdiff_pow);
    h.curvature = Some(curvature_pow);
    h.monotonicity = Some(monotonicity_pow);
    h.integrality = Some(integrality_pow);
    h.getsymdata = true;

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, exprhdlrs, exprhdlr, data, ExprhdlrData, $f)
        };
    }
    let epsilon = scip.set.epsilon();
    crate::paramset::paramset_add_real(
        scip,
        &format!("expr/{POWEXPRHDLR_NAME}/minzerodistance"),
        "minimal distance from zero to enforce for child in bound tightening",
        t!(f64, minzerodistance),
        false,
        epsilon,
        0.0,
        1.0,
        None,
    )?;
    crate::paramset::paramset_add_int(
        scip,
        &format!("expr/{POWEXPRHDLR_NAME}/expandmaxexponent"),
        "maximal exponent when to expand power of sum in simplify",
        t!(i32, expandmaxexponent),
        false,
        2,
        1,
        i32::MAX,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        &format!("expr/{POWEXPRHDLR_NAME}/distribfracexponent"),
        "whether a fractional exponent is distributed onto factors on power of product",
        t!(bool, distribfracexponent),
        false,
        false,
        None,
    )?;

    Ok(())
}

/// `SCIPincludeExprhdlrSignpower`: creates the handler for signed power
/// expression and includes it into SCIP.
pub fn include_exprhdlr_signpower(scip: &mut Scip) -> ScipResult {
    let exprhdlr = crate::scip_expr::include_exprhdlr(scip, SIGNPOWEXPRHDLR_NAME, Some(SIGNPOWEXPRHDLR_DESC), SIGNPOWEXPRHDLR_PRECEDENCE, eval_signpower, None)?;

    let h = &mut scip.exprhdlrs[exprhdlr];
    h.copyhdlr = Some(copyhdlr_signpower);
    h.copydata = Some(copydata_pow);
    h.freedata = Some(freedata_pow);
    h.simplify = Some(simplify_signpower);
    h.print = true;
    h.parse = Some(parse_signpower);
    h.inteval = Some(inteval_signpower);
    h.initestimates = Some(initestimates_signpower);
    h.estimate = Some(estimate_signpower);
    h.reverseprop = Some(reverseprop_signpower);
    h.hash = Some(hash_signpower);
    h.compare = Some(compare_pow);
    h.bwdiff = Some(bwdiff_signpower);
    h.curvature = Some(curvature_signpower);
    h.monotonicity = Some(monotonicity_signpower);
    h.integrality = Some(integrality_pow);
    h.getsymdata = true;

    Ok(())
}

/// `SCIPcreateExprPow`: creates a power expression.
pub fn create_expr_pow(scip: &mut Scip, child: ExprId, exponent: f64, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    let h = scip.set.exprhdlrpow.expect("power expression handler");
    expr::expr_create(scip, h, Some(Box::new(create_data(exponent))), &[child], ownercreate, ownercreatedata)
}

/// `SCIPcreateExprSignpower`: creates a signpower expression.
pub fn create_expr_signpower(scip: &mut Scip, child: ExprId, exponent: f64, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    let h = expr::set_find_exprhdlr(scip, SIGNPOWEXPRHDLR_NAME);
    expr::expr_create(scip, h, Some(Box::new(create_data(exponent))), &[child], ownercreate, ownercreatedata)
}

/// `SCIPisExprSignpower`: indicates whether expression is of signpower-type.
pub fn is_expr_signpower(scip: &Scip, expr: ExprId) -> bool {
    is_expr_named(scip, expr, SIGNPOWEXPRHDLR_NAME)
}

/// `SCIPaddSquareLinearization`: computes coefficients of linearization of a
/// square term in a reference point.
pub fn add_square_linearization(scip: &Scip, sqrcoef: f64, refpoint: f64, isint: bool, lincoef: &mut f64, linconstant: &mut f64, success: &mut bool) {
    let set = &scip.set;

    if sqrcoef == 0.0 {
        return;
    }

    if set.is_infinity(refpoint.abs()) {
        *success = false;
        return;
    }

    if !isint || set.is_integral(refpoint) {
        // sqrcoef * x^2 -> tangent in refpoint = sqrcoef * 2 * refpoint * (x -
        // refpoint).
        let mut tmp = sqrcoef * refpoint;

        if set.is_infinity(2.0 * tmp.abs()) {
            *success = false;
            return;
        }

        *lincoef += 2.0 * tmp;
        tmp *= refpoint;
        *linconstant -= tmp;
    } else {
        // sqrcoef * x^2 -> secant between f=floor(refpoint) and f+1 = sqrcoef *
        // (f^2 + ((f+1)^2 - f^2) * (x-f)) = sqrcoef * (-f*(f+1) + (2*f+1)*x).
        let f = set.floor(refpoint);

        let coef = sqrcoef * (2.0 * f + 1.0);
        let constant = -sqrcoef * f * (f + 1.0);

        if set.is_infinity(coef.abs()) || set.is_infinity(constant.abs()) {
            *success = false;
            return;
        }

        *lincoef += coef;
        *linconstant += constant;
    }
}

/// `SCIPaddSquareSecant`: computes coefficients of secant of a square term.
pub fn add_square_secant(scip: &Scip, sqrcoef: f64, lb: f64, ub: f64, lincoef: &mut f64, linconstant: &mut f64, success: &mut bool) {
    let set = &scip.set;

    if sqrcoef == 0.0 {
        return;
    }

    if set.is_infinity(-lb) || set.is_infinity(ub) {
        // Unboundedness.
        *success = false;
        return;
    }

    // sqrcoef * x^2 -> sqrcoef * (lb * lb + (ub*ub - lb*lb)/(ub-lb) * (x-lb)) =
    // sqrcoef * (lb*lb + (ub+lb)*(x-lb)) = sqrcoef * ((lb+ub)*x - lb*ub).
    let coef = sqrcoef * (lb + ub);
    let constant = -sqrcoef * lb * ub;
    if set.is_infinity(coef.abs()) || set.is_infinity(constant.abs()) {
        *success = false;
        return;
    }

    *lincoef += coef;
    *linconstant += constant;
}

/// `SCIPestimateRoot`: separation for roots with exponent in [0,1]
/// - x^0.5 with x >= 0
#[allow(clippy::too_many_arguments)]
pub fn estimate_root(
    scip: &Scip,
    exponent: f64,
    overestimate: bool,
    xlb: f64,
    xub: f64,
    mut xref: f64,
    constant: &mut f64,
    slope: &mut f64,
    islocal: &mut bool,
    success: &mut bool,
) {
    if !overestimate {
        // Underestimate -> secant.
        compute_secant(scip, false, exponent, xlb, xub, constant, slope, success);
        *islocal = true;
    } else {
        // Overestimate -> tangent.

        // Need to linearize right of 0.
        if xref < 0.0 {
            xref = 0.0;
        }

        if scip.set.is_zero(xref) {
            if scip.set.is_zero(xub) {
                *success = false;
                *islocal = false;
                return;
            }

            // If xref is 0 (then xlb=0 probably), then slope is infinite, then try
            // to move away from 0.
            if xub < 0.2 {
                xref = 0.5 * xlb + 0.5 * xub;
            } else {
                xref = 0.1;
            }
        }

        compute_tangent(false, exponent, xref, constant, slope, success);
        *islocal = false;
    }
}

/// `SCIPgetExponentExprPow`: gets the exponent of a power or signed power
/// expression.
#[inline]
pub fn get_exponent_expr_pow(scip: &Scip, expr: ExprId) -> f64 {
    data(scip, expr).exponent
}

/// C's `MAX(a, b)`.
#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// C's `MIN(a, b)`.
#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}
