//! Port of `exprcurv.c`: functions to work with curvature (convex, concave,
//! etc).

use crate::{
    def::{eps_ge, eps_is_int, eps_le},
    expr::ExprCurv,
    interval::Interval,
};

/// `SCIPexprcurvAdd`: gives curvature for a sum of two functions with
/// given curvature.
#[inline]
pub fn exprcurv_add(curv1: ExprCurv, curv2: ExprCurv) -> ExprCurv {
    ExprCurv::from_bits(curv1.bits() & curv2.bits())
}

/// `SCIPexprcurvNegate`: gives the curvature for the negation of a
/// function with given curvature.
pub fn exprcurv_negate(curvature: ExprCurv) -> ExprCurv {
    match curvature {
        ExprCurv::Concave => ExprCurv::Convex,
        ExprCurv::Convex => ExprCurv::Concave,
        // Can return curvature, do this below.
        ExprCurv::Linear | ExprCurv::Unknown => curvature,
    }
}

/// `SCIPexprcurvMultiply`: gives curvature for a functions with given
/// curvature multiplied by a constant factor.
pub fn exprcurv_multiply(factor: f64, curvature: ExprCurv) -> ExprCurv {
    if factor == 0.0 {
        return ExprCurv::Linear;
    }
    if factor > 0.0 {
        return curvature;
    }
    exprcurv_negate(curvature)
}

/// `SCIPexprcurvPower`: gives curvature for base^exponent for given bounds
/// and curvature of base-function and constant exponent.
pub fn exprcurv_power(mut basebounds: Interval, basecurv: ExprCurv, exponent: f64) -> ExprCurv {
    if exponent == 0.0 {
        return ExprCurv::Linear;
    }

    if exponent == 1.0 {
        return basecurv;
    }

    let expisint = eps_is_int(exponent, 0.0);

    // If exponent is fractional, then power is not defined for a negative
    // base; thus, consider only positive part of basebounds.
    if !expisint && basebounds.inf < 0.0 {
        basebounds.inf = 0.0;
        if basebounds.sup < 0.0 {
            return ExprCurv::Linear;
        }
    }

    // If basebounds contains 0.0, consider negative and positive interval
    // separately, if possible.
    if basebounds.inf < 0.0 && basebounds.sup > 0.0 {
        // Something like x^(-2) may look convex on each side of zero, but is not
        // convex on the whole interval due to the singularity at 0.0.
        if exponent < 0.0 {
            return ExprCurv::Unknown;
        }

        let leftbounds = Interval::new(basebounds.inf, 0.0);
        let rightbounds = Interval::new(0.0, basebounds.sup);

        return ExprCurv::from_bits(exprcurv_power(leftbounds, basecurv, exponent).bits() & exprcurv_power(rightbounds, basecurv, exponent).bits());
    }

    // (base^exponent)'' = exponent * ( (exponent-1) base^(exponent-2)
    // (base')^2 + base^(exponent-1) base'' ); see exprcurv.c for the case
    // analysis.
    if basecurv == ExprCurv::Linear {
        // base^(exponent-2) is negative, if base < 0.0 and exponent is odd.
        let mut sign = exponent * (exponent - 1.0);
        if basebounds.inf < 0.0 && (exponent as i32) % 2 != 0 {
            sign *= -1.0;
        }

        return if sign > 0.0 { ExprCurv::Convex } else { ExprCurv::Concave };
    }

    if basecurv == ExprCurv::Convex {
        if basebounds.sup <= 0.0 && exponent < 0.0 && expisint {
            return if (exponent as i32) % 2 == 0 { ExprCurv::Convex } else { ExprCurv::Concave };
        }
        if basebounds.inf >= 0.0 && exponent > 1.0 {
            return ExprCurv::Convex;
        }
        return ExprCurv::Unknown;
    }

    if basecurv == ExprCurv::Concave {
        if basebounds.sup <= 0.0 && exponent > 1.0 && expisint {
            return if (exponent as i32) % 2 == 0 { ExprCurv::Convex } else { ExprCurv::Concave };
        }
        if basebounds.inf >= 0.0 && exponent < 1.0 {
            return if exponent < 0.0 { ExprCurv::Convex } else { ExprCurv::Concave };
        }
        return ExprCurv::Unknown;
    }

    ExprCurv::Unknown
}

/// `SCIPexprcurvPowerInv`: gives required curvature for base so that
/// base^exponent has given curvature under given bounds on base and constant
/// exponent.
pub fn exprcurv_power_inv(mut basebounds: Interval, exponent: f64, powercurv: ExprCurv) -> ExprCurv {
    if exponent == 1.0 {
        return powercurv;
    }

    // Power is usually never linear, now that exponent != 1.
    if powercurv == ExprCurv::Linear {
        return ExprCurv::Unknown;
    }

    let expisint = eps_is_int(exponent, 0.0);

    // If exponent is fractional, then power is only defined for a non-negative
    // base; boundtightening should have ensured this before calling this
    // function, but sometimes this does not work and so we correct this here
    // for us.
    if !expisint && basebounds.inf < 0.0 {
        basebounds.inf = 0.0;
        if basebounds.sup < 0.0 {
            return ExprCurv::Unknown;
        }
    }

    // If basebounds contains 0.0, consider negative and positive interval
    // separately, if possible.
    if basebounds.inf < 0.0 && basebounds.sup > 0.0 {
        // Something like x^(-2) may look convex on each side of zero, but is not
        // convex on the whole interval due to the singularity at 0.0.
        if exponent < 0.0 {
            return ExprCurv::Unknown;
        }

        let leftbounds = Interval::new(basebounds.inf, 0.0);
        let rightbounds = Interval::new(0.0, basebounds.sup);

        let leftcurv = exprcurv_power_inv(leftbounds, exponent, powercurv);
        let rightcurv = exprcurv_power_inv(rightbounds, exponent, powercurv);

        // Now need to intersect.
        if leftcurv == ExprCurv::Linear {
            return rightcurv;
        }
        if rightcurv == ExprCurv::Linear {
            return leftcurv;
        }
        if leftcurv == ExprCurv::Unknown || rightcurv == ExprCurv::Unknown {
            return ExprCurv::Unknown;
        }
        return ExprCurv::Linear;
    }

    // Inverting the logic from SCIPexprcurvPower here.
    if powercurv == ExprCurv::Convex {
        if basebounds.sup <= 0.0 && exponent < 0.0 && expisint && (exponent as i32) % 2 == 0 {
            return ExprCurv::Convex;
        }
        if basebounds.inf >= 0.0 && exponent > 1.0 {
            return ExprCurv::Convex;
        }
        if basebounds.sup <= 0.0 && exponent > 1.0 && expisint && (exponent as i32) % 2 == 0 {
            return ExprCurv::Concave;
        }
        if basebounds.inf >= 0.0 && exponent < 0.0 {
            return ExprCurv::Concave;
        }

        // base^(exponent-2) is negative, if base < 0.0 and exponent is odd.
        let mut sign = exponent * (exponent - 1.0);
        if basebounds.inf < 0.0 && (exponent as i32) % 2 != 0 {
            sign *= -1.0;
        }
        if sign > 0.0 {
            return ExprCurv::Linear;
        }
    } else {
        if basebounds.sup <= 0.0 && exponent < 0.0 && expisint && (exponent as i32) % 2 != 0 {
            return ExprCurv::Convex;
        }
        if basebounds.sup <= 0.0 && exponent > 1.0 && expisint && (exponent as i32) % 2 != 0 {
            return ExprCurv::Concave;
        }
        if basebounds.inf >= 0.0 && (0.0..1.0).contains(&exponent) {
            return ExprCurv::Concave;
        }

        // base^(exponent-2) is negative, if base < 0.0 and exponent is odd.
        let mut sign = exponent * (exponent - 1.0);
        if basebounds.inf < 0.0 && (exponent as i32) % 2 != 0 {
            sign *= -1.0;
        }
        if sign < 0.0 {
            return ExprCurv::Linear;
        }
    }

    ExprCurv::Unknown
}

/// `SCIPexprcurvMonomial`: gives curvature for a monomial with given
/// curvatures and bounds for each factor.
///
/// See Maranas and Floudas, Finding All Solutions of Nonlinearly Constrained
/// Systems of Equations, JOGO 7, 1995 for the categorization in the case
/// that all factors are linear.
///
/// Exponents can also be negative or rational.
pub fn exprcurv_monomial(nfactors: usize, exponents: Option<&[f64]>, factoridxs: Option<&[i32]>, factorcurv: &[ExprCurv], factorbounds: &[Interval]) -> ExprCurv {
    if nfactors == 0 {
        return ExprCurv::Linear;
    }

    if nfactors == 1 {
        let f = factoridxs.map_or(0, |fi| fi[0] as usize);
        let e = exponents.map_or(1.0, |ex| ex[0]);
        return exprcurv_power(factorbounds[f], factorcurv[f], e);
    }

    let mut mult = 1.0;

    let mut nnegative = 0; // number of negative exponents
    let mut npositive = 0; // number of positive exponents
    let mut sum = 0.0; // sum of exponents
    let mut expcurvpos = true; // whether exp_j * f_j''(x) >= 0 for all factors (assuming f_j >= 0)
    let mut expcurvneg = true; // whether exp_j * f_j''(x) <= 0 for all factors (assuming f_j >= 0)

    for j in 0..nfactors {
        let f = factoridxs.map_or(j, |fi| fi[j] as usize);
        if factorcurv[f] == ExprCurv::Unknown {
            return ExprCurv::Unknown;
        }

        let e = exponents.map_or(1.0, |ex| ex[j]);
        let mut bounds = factorbounds[f];

        // If argument is negative, then exponent should be integer; correct
        // bounds if that doesn't hold.
        if !eps_is_int(e, 0.0) && bounds.inf < 0.0 {
            bounds.inf = 0.0;
            if bounds.sup < 0.0 {
                return ExprCurv::Unknown;
            }
        }

        if bounds.inf < 0.0 && bounds.sup > 0.0 {
            return ExprCurv::Unknown;
        }

        if e < 0.0 {
            nnegative += 1;
        } else {
            npositive += 1;
        }
        sum += e;

        let mut fcurv;
        if bounds.inf < 0.0 {
            // Flip j'th argument: (f_j)^(exp_j) = (-1)^(exp_j) (-f_j)^(exp_j);
            // -f_j has negated curvature of f_j.
            fcurv = exprcurv_negate(factorcurv[f]);

            // Negate monomial, if exponent is odd, i.e., (-1)^(exp_j) = -1.
            if (e as i32) % 2 != 0 {
                mult *= -1.0;
            }
        } else {
            fcurv = factorcurv[f];
        }

        // Check if exp_j * fcurv is convex (>= 0) and/or concave.
        fcurv = exprcurv_multiply(e, fcurv);
        if fcurv.bits() & ExprCurv::Convex.bits() == 0 {
            expcurvpos = false;
        }
        if fcurv.bits() & ExprCurv::Concave.bits() == 0 {
            expcurvneg = false;
        }
    }

    // If all factors are linear, then a product f_j^exp_j with f_j >= 0 is
    // convex if
    // - all exponents are negative, or
    // - all except one exponent j* are negative and exp_j* >= 1 -
    //   sum_{j!=j*}exp_j, but the latter is equivalent to sum_j exp_j >= 1
    // further, the product is concave if
    // - all exponents are positive and the sum of exponents is <= 1.0
    //
    // If factors are nonlinear, then we require additionally, that for
    // convexity
    // - each factor is convex if exp_j >= 0, or concave if exp_j <= 0, i.e.,
    //   exp_j*f_j'' >= 0
    // and for concavity, we require that
    // - all factors are concave, i.e., exp_j*f_j'' <= 0
    let curv = if (nnegative == nfactors && expcurvpos) || (nnegative == nfactors - 1 && eps_ge(sum, 1.0, 1e-9) && expcurvpos) {
        ExprCurv::Convex
    } else if npositive == nfactors && eps_le(sum, 1.0, 1e-9) && expcurvneg {
        ExprCurv::Concave
    } else {
        ExprCurv::Unknown
    };

    exprcurv_multiply(mult, curv)
}

/// `SCIPexprcurvMonomialInv`: for a monomial with given bounds for each
/// factor, gives condition on the curvature of each factor, so that
/// monomial has a requested curvature, if possible; returns whether
/// `monomialcurv` can be achieved.
pub fn exprcurv_monomial_inv(mut monomialcurv: ExprCurv, nfactors: usize, exponents: Option<&[f64]>, factorbounds: &[Interval], factorcurv: &mut [ExprCurv]) -> bool {
    if nfactors == 1 {
        factorcurv[0] = exprcurv_power_inv(factorbounds[0], exponents.map_or(1.0, |e| e[0]), monomialcurv);
        return factorcurv[0] != ExprCurv::Unknown;
    }

    // Any decent monomial with at least 2 factors is not linear.
    if monomialcurv == ExprCurv::Linear {
        return false;
    }

    // Count positive and negative exponents, sum of exponents; flip negative
    // factors.
    let mut nnegative = 0; // number of negative exponents
    let mut npositive = 0; // number of positive exponents
    let mut sum = 0.0; // sum of exponents
    for j in 0..nfactors {
        let e = exponents.map_or(1.0, |ex| ex[j]);

        let mut bounds = factorbounds[j];

        // If argument is negative, then exponent should be integer; if that
        // didn't happen, consider argument as if non-negative.
        if !eps_is_int(e, 0.0) && bounds.inf < 0.0 {
            bounds.inf = 0.0;
            if bounds.sup < 0.0 {
                return false;
            }
        }

        // Mixed signs are bad.
        if bounds.inf < 0.0 && bounds.sup > 0.0 {
            return false;
        }

        if e < 0.0 {
            nnegative += 1;
        } else {
            npositive += 1;
        }
        sum += e;

        if bounds.inf < 0.0 {
            // Flip j'th argument: (f_j)^(exp_j) = (-1)^(exp_j) (-f_j)^(exp_j);
            // thus, negate monomial, if exponent is odd, i.e., (-1)^(exp_j) = -1.
            if (e as i32) % 2 != 0 {
                monomialcurv = exprcurv_negate(monomialcurv);
            }
        }
    }

    if monomialcurv == ExprCurv::Convex {
        if nnegative + 1 < nfactors {
            // At least two positive exponents.
            return false;
        }

        if nnegative < nfactors && !eps_ge(sum, 1.0, 1e-9) {
            // One negative exponent, but sum is not >= 1.
            return false;
        }

        // Monomial will be convex, if each factor is convex if exp_j >= 0, or
        // concave if exp_j <= 0, i.e., exp_j*f_j'' >= 0.
        for j in 0..nfactors {
            let mut e = exponents.map_or(1.0, |ex| ex[j]);

            // If factor is negative, then factorcurv[j] need to be flipped, which
            // we can also get by flipping e.
            if factorbounds[j].inf < 0.0 && eps_is_int(e, 0.0) {
                e = -e;
            }
            factorcurv[j] = if e >= 0.0 { ExprCurv::Convex } else { ExprCurv::Concave };
        }
    } else {
        if npositive < nfactors {
            // At least one negative exponent.
            return false;
        }

        if !eps_le(sum, 1.0, 1e-9) {
            // Sum is not <= 1.
            return false;
        }

        // Monomial will be concave, if each factor is concave.
        for j in 0..nfactors {
            let e = exponents.map_or(1.0, |ex| ex[j]);

            // If factor is negative, then factorcurv[j] need to be flipped, i.e.
            // convex.
            factorcurv[j] = if factorbounds[j].inf < 0.0 && eps_is_int(e, 0.0) {
                ExprCurv::Convex
            } else {
                ExprCurv::Concave
            };
        }
    }

    true
}

/// `SCIPexprcurvGetName`: gives name as string for a curvature.
pub fn exprcurv_get_name(curv: ExprCurv) -> &'static str {
    ["unknown", "convex", "concave", "linear"][curv as usize]
}
