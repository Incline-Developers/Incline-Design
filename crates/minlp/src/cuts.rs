//! SCIP's cut generation (`cuts.c`, `struct_cuts.h`): aggregation rows,
//! complemented MIR, flow cover, knapsack cover and strong CG cuts.
//!
//! C keeps quad precision vectors as interleaved `double` arrays
//! (`QUAD_ARRAY_LOAD`/`QUAD_ARRAY_STORE`); here they are `[Quad]` slices.
//! The exact-mode variants (`*Safely`, `*Rational`) are not ported.

use crate::{
    api::{get_sol_val, get_vars},
    quad::{QUAD_EPSILON, Quad, prod_dd, prod_qd, sum_dd, sum_qd, sum_qq},
    retcode::ScipResult,
    scip::Scip,
    types::{RowId, SolId, VarId},
};

/// `NONZERO`: the value must not be exactly zero due to sparsity pattern.
#[inline(always)]
fn nonzero(x: f64) -> f64 {
    1e-100f64.copysign(x) + x
}

/// `EPSZ`.
#[inline(always)]
fn epsz(x: f64, eps: f64) -> bool {
    x.abs() <= eps
}

/// `varVecAddScaledRowCoefs`: add a scaled row to a dense vector indexed
/// over the problem variables and keep the index of non-zeros up-to-date.
fn var_vec_add_scaled_row_coefs(scip: &Scip, inds: &mut [i32], vals: &mut [f64], nnz: &mut i32, row: RowId, scale: f64) {
    let r = &scip.rows[row];
    // Add the non-zeros to the aggregation row and keep non-zero index up to
    // date.
    for i in 0..r.len {
        let probindex = scip.cols[r.cols[i]].var_probindex as usize;
        let mut val = vals[probindex];

        if val == 0.0 {
            inds[*nnz as usize] = probindex as i32;
            *nnz += 1;
        }

        val += r.vals[i] * scale;

        // The value must not be exactly zero due to sparsity pattern.
        val = nonzero(val);

        vals[probindex] = val;
    }
}

/// `varVecAddScaledRowCoefsQuad`: add a scaled row to a dense vector
/// indexed over the problem variables and keep the index of non-zeros
/// up-to-date; this is the quad precision version of
/// varVecAddScaledRowCoefs().
fn var_vec_add_scaled_row_coefs_quad(scip: &Scip, inds: &mut [i32], vals: &mut [Quad], nnz: &mut i32, row: RowId, scale: f64) {
    let r = &scip.rows[row];
    // Add the non-zeros to the aggregation row and keep non-zero index up to
    // date.
    for i in 0..r.len {
        let probindex = scip.cols[r.cols[i]].var_probindex as usize;
        let mut val = vals[probindex];

        if val.hi == 0.0 {
            inds[*nnz as usize] = probindex as i32;
            *nnz += 1;
        }

        let scaledrowval = prod_dd(r.vals[i], scale);
        val = sum_qq(val, scaledrowval);

        // The value must not be exactly zero due to sparsity pattern.
        val.hi = nonzero(val.hi);

        vals[probindex] = val;
    }
}

/// `varVecAddScaledRowCoefsQuadScale`: add a scaled row to a dense vector
/// indexed over the problem variables and keep the index of non-zeros
/// up-to-date; this is the quad precision version of
/// varVecAddScaledRowCoefs() with a quad precision scaling factor.
fn var_vec_add_scaled_row_coefs_quad_scale(scip: &Scip, inds: &mut [i32], vals: &mut [Quad], nnz: &mut i32, row: RowId, scale: Quad) {
    let r = &scip.rows[row];
    // Add the non-zeros to the aggregation row and keep non-zero index up to
    // date.
    for i in 0..r.len {
        let probindex = scip.cols[r.cols[i]].var_probindex as usize;
        let mut val = vals[probindex];

        if val.hi == 0.0 {
            inds[*nnz as usize] = probindex as i32;
            *nnz += 1;
            val = prod_qd(scale, r.vals[i]);
        } else {
            let rowval = prod_qd(scale, r.vals[i]);
            val = sum_qq(val, rowval);
        }

        // The value must not be exactly zero due to sparsity pattern.
        val.hi = nonzero(val.hi);

        vals[probindex] = val;
    }
}

/// `calcEfficacy`: calculates the cut efficacy for the given solution.
fn calc_efficacy(scip: &Scip, sol: SolId, cutcoefs: &[f64], cutrhs: f64, cutinds: &[i32], cutnnz: i32) -> f64 {
    let vars = get_vars(scip);
    let mut norm = 0.0;
    let mut activity = 0.0;
    let n = cutnnz as usize;

    match scip.set.sepa_efficacynorm {
        b'e' => {
            for i in 0..n {
                activity += cutcoefs[i] * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                norm += cutcoefs[i] * cutcoefs[i];
            }
            norm = f64::sqrt(norm);
        }
        b'm' => {
            for i in 0..n {
                activity += cutcoefs[i] * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                let absval = cutcoefs[i].abs();
                norm = crate::def::max(norm, absval);
            }
        }
        b's' => {
            for i in 0..n {
                activity += cutcoefs[i] * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                norm += cutcoefs[i].abs();
            }
        }
        b'd' => {
            for i in 0..n {
                activity += cutcoefs[i] * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                if !scip.set.is_zero(cutcoefs[i]) {
                    norm = 1.0;
                }
            }
        }
        c => log::error!("invalid efficacy norm parameter '{}'", c as char),
    }

    (activity - cutrhs) / crate::def::max(1e-6, norm)
}

/// `calcEfficacyNormQuad`: calculates the efficacy norm of the given
/// aggregation row, which depends on the "separating/efficacynorm"
/// parameter.
fn calc_efficacy_norm_quad(scip: &Scip, vals: &[Quad], inds: &[i32], nnz: i32) -> f64 {
    let mut norm = 0.0;
    let n = nnz as usize;

    match scip.set.sepa_efficacynorm {
        b'e' => {
            for i in 0..n {
                let coef = vals[inds[i] as usize].to_dbl();
                norm += coef * coef;
            }
            norm = f64::sqrt(norm);
        }
        b'm' => {
            for i in 0..n {
                let absval = vals[inds[i] as usize].to_dbl().abs();
                norm = crate::def::max(norm, absval);
            }
        }
        b's' => {
            for i in 0..n {
                norm += vals[inds[i] as usize].to_dbl().abs();
            }
        }
        b'd' => {
            for i in 0..n {
                if !scip.set.is_zero(vals[inds[i] as usize].to_dbl()) {
                    norm = 1.0;
                    break;
                }
            }
        }
        c => log::error!("invalid efficacy norm parameter '{}.'", c as char),
    }

    norm
}

/// `calcEfficacyDenseStorageQuad`: calculates the cut efficacy for the
/// given solution; the cut coefs are stored densely and in quad precision.
fn calc_efficacy_dense_storage_quad(scip: &Scip, sol: SolId, cutcoefs: &[Quad], cutrhs: f64, cutinds: &[i32], cutnnz: i32) -> f64 {
    let vars = get_vars(scip);
    let mut norm = 0.0;
    let mut activity = 0.0;
    let n = cutnnz as usize;

    match scip.set.sepa_efficacynorm {
        b'e' => {
            for i in 0..n {
                let coef = cutcoefs[cutinds[i] as usize].to_dbl();
                activity += coef * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                norm += coef * coef;
            }
            norm = f64::sqrt(norm);
        }
        b'm' => {
            for i in 0..n {
                let coef = cutcoefs[cutinds[i] as usize].to_dbl();
                activity += coef * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                let absval = coef.abs();
                norm = crate::def::max(norm, absval);
            }
        }
        b's' => {
            for i in 0..n {
                let coef = cutcoefs[cutinds[i] as usize].to_dbl();
                activity += coef * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                norm += coef.abs();
            }
        }
        b'd' => {
            for i in 0..n {
                let coef = cutcoefs[cutinds[i] as usize].to_dbl();
                activity += coef * get_sol_val(scip, sol, vars[cutinds[i] as usize]);
                if !scip.set.is_zero(coef) {
                    norm = 1.0;
                }
            }
        }
        c => log::error!("invalid efficacy norm parameter '{}.'", c as char),
    }

    (activity - cutrhs) / crate::def::max(1e-6, norm)
}

/// Local or global bounds of a variable.
#[inline]
fn var_bounds(scip: &Scip, var: VarId, local: bool) -> (f64, f64) {
    let v = &scip.vars[var];
    if local { (v.locdom.lb, v.locdom.ub) } else { (v.glbdom.lb, v.glbdom.ub) }
}

/// `removeZerosQuad`: safely remove all items with |a_i| or |u_i - l_i)|
/// below the given value; returns TRUE if the cut became redundant. If it
/// is a local cut, use local bounds, otherwise, use global bounds.
fn remove_zeros_quad(scip: &Scip, minval: f64, cutislocal: bool, cutcoefs: &mut [Quad], cutrhs: &mut Quad, cutinds: &mut [i32], cutnnz: &mut i32) -> bool {
    let vars = get_vars(scip);

    let mut i = 0;
    while i < *cutnnz as usize {
        let v = cutinds[i] as usize;
        let mut val = cutcoefs[v];

        let (lb, ub) = var_bounds(scip, vars[v], cutislocal);

        let isfixed = !(scip.set.is_infinity(-lb) || scip.set.is_infinity(ub)) && scip.set.is_eq(ub, lb);

        if isfixed || epsz(val.to_dbl(), minval) {
            if val.to_dbl().abs() > QUAD_EPSILON {
                // Adjust right hand side with max contribution.
                if val.to_dbl() < 0.0 {
                    if scip.set.is_infinity(ub) {
                        return true;
                    } else {
                        val = prod_qd(val, ub);
                        *cutrhs = sum_qq(*cutrhs, -val);
                    }
                } else if scip.set.is_infinity(-lb) {
                    return true;
                } else {
                    val = prod_qd(val, lb);
                    *cutrhs = sum_qq(*cutrhs, -val);
                }
            }

            cutcoefs[v] = Quad::new(0.0);

            // Remove non-zero entry.
            *cutnnz -= 1;
            cutinds[i] = cutinds[*cutnnz as usize];
        } else {
            i += 1;
        }
    }

    // Relax rhs to 0, if it's very close to 0.
    if cutrhs.to_dbl() < 0.0 && cutrhs.to_dbl() >= -scip.set.epsilon() {
        *cutrhs = Quad::new(0.0);
    }

    false
}

/// `removeZeros`: safely remove all items with |a_i| or |u_i - l_i| below
/// the given value; returns TRUE if the cut became redundant. If it is a
/// local cut, use local bounds, otherwise, use global bounds.
fn remove_zeros(scip: &Scip, minval: f64, cutislocal: bool, cutcoefs: &mut [f64], cutrhs: &mut Quad, cutinds: &mut [i32], cutnnz: &mut i32) -> bool {
    let vars = get_vars(scip);

    // Loop over non-zeros and remove values below minval; values above
    // QUAD_EPSILON are cancelled with their bound to avoid numerical rounding
    // errors.
    let mut i = 0;
    while i < *cutnnz as usize {
        let v = cutinds[i] as usize;
        let val = cutcoefs[v];

        let (lb, ub) = var_bounds(scip, vars[v], cutislocal);

        let isfixed = !(scip.set.is_infinity(-lb) || scip.set.is_infinity(ub)) && scip.set.is_eq(ub, lb);

        if epsz(val, minval) || isfixed {
            if val.abs() > QUAD_EPSILON {
                // Adjust left and right hand sides with max contribution.
                if val < 0.0 {
                    if scip.set.is_infinity(ub) {
                        return true;
                    } else {
                        let quadprod = prod_dd(-val, ub);
                        *cutrhs = sum_qq(*cutrhs, quadprod);
                    }
                } else if scip.set.is_infinity(-lb) {
                    return true;
                } else {
                    let quadprod = prod_dd(-val, lb);
                    *cutrhs = sum_qq(*cutrhs, quadprod);
                }
            }

            cutcoefs[v] = 0.0;

            // Remove non-zero entry.
            *cutnnz -= 1;
            cutinds[i] = cutinds[*cutnnz as usize];
        } else {
            i += 1;
        }
    }

    // Relax rhs to 0, if it's very close to 0.
    if cutrhs.to_dbl() < 0.0 && cutrhs.to_dbl() >= -scip.set.epsilon() {
        *cutrhs = Quad::new(0.0);
    }

    false
}

/// `compareAbsCoefsQuad`: compare absolute values of coefficients in quad
/// precision.
fn compare_abs_coefs_quad(coefs: &[Quad], ind1: i32, ind2: i32) -> i32 {
    let abscoef1 = coefs[ind1 as usize].to_dbl().abs();
    let abscoef2 = coefs[ind2 as usize].to_dbl().abs();

    if abscoef1 < abscoef2 {
        return -1;
    }
    if abscoef2 < abscoef1 {
        return 1;
    }
    0
}

/// `compareAbsCoefs`: compare absolute values of coefficients.
fn compare_abs_coefs(coefs: &[f64], ind1: i32, ind2: i32) -> i32 {
    let abscoef1 = coefs[ind1 as usize].abs();
    let abscoef2 = coefs[ind2 as usize].abs();

    if abscoef1 < abscoef2 {
        return -1;
    }
    if abscoef2 < abscoef1 {
        return 1;
    }
    0
}

/// `chgCoeffWithBound`: change given coefficient to new given value,
/// adjust right hand side using the variables bound; returns TRUE if the
/// right hand side would need to be changed to infinity and FALSE
/// otherwise.
fn chg_coeff_with_bound(scip: &Scip, var: VarId, oldcoeff: f64, newcoeff: f64, cutislocal: bool, cutrhs: &mut Quad) -> bool {
    let mut delta = sum_dd(newcoeff, -oldcoeff);
    let (lb, ub) = var_bounds(scip, var, cutislocal);

    if delta.to_dbl() > QUAD_EPSILON {
        if scip.set.is_infinity(ub) {
            return true;
        } else {
            delta = prod_qd(delta, ub);
            *cutrhs = sum_qq(*cutrhs, delta);
        }
    } else if delta.to_dbl() < -QUAD_EPSILON {
        if scip.set.is_infinity(-lb) {
            return true;
        } else {
            delta = prod_qd(delta, lb);
            *cutrhs = sum_qq(*cutrhs, delta);
        }
    }

    false
}

/// `chgQuadCoeffWithBound`: change given (quad) coefficient to new given
/// value, adjust right hand side using the variables bound; returns TRUE if
/// the right hand side would need to be changed to infinity and FALSE
/// otherwise.
fn chg_quad_coeff_with_bound(scip: &Scip, var: VarId, oldcoeff: Quad, newcoeff: f64, cutislocal: bool, cutrhs: &mut Quad) -> bool {
    let mut delta = sum_qd(-oldcoeff, newcoeff);
    let (lb, ub) = var_bounds(scip, var, cutislocal);

    if delta.to_dbl() > QUAD_EPSILON {
        if scip.set.is_infinity(ub) {
            return true;
        } else {
            delta = prod_qd(delta, ub);
            *cutrhs = sum_qq(*cutrhs, delta);
        }
    } else if delta.to_dbl() < -QUAD_EPSILON {
        if scip.set.is_infinity(-lb) {
            return true;
        } else {
            delta = prod_qd(delta, lb);
            *cutrhs = sum_qq(*cutrhs, delta);
        }
    }

    false
}

/// `cutTightenCoefsQuad`: scales the cut and then tightens the
/// coefficients of the given cut based on the maximal activity; see
/// cons_linear.c consdataTightenCoefs() for details; the cut is given in a
/// semi-sparse quad precision array; returns whether the cut was detected
/// to be redundant.
///
/// This is the quad precision version of cutTightenCoefs() below.
fn cut_tighten_coefs_quad(scip: &Scip, cutislocal: bool, cutcoefs: &mut [Quad], cutrhs: &mut Quad, cutinds: &mut [i32], cutnnz: &mut i32) -> bool {
    let mut isintegral = true;
    let mut maxabsintval: f64 = 0.0;
    let mut maxabscontval: f64 = 0.0;

    let mut maxacttmp = Quad::new(0.0);

    let vars = get_vars(scip);
    let nintegralvars = crate::api::get_n_vars(scip) - crate::api::get_n_cont_vars(scip);

    // Compute maximal activity and maximal absolute coefficient values for all
    // and for integral variables in the cut.
    for i in 0..*cutnnz as usize {
        let mut val = cutcoefs[cutinds[i] as usize];
        let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

        if val.to_dbl() < 0.0 {
            if scip.set.is_infinity(-lb) {
                return false;
            }

            if cutinds[i] < nintegralvars {
                maxabsintval = crate::def::max(maxabsintval, -val.to_dbl());
            } else {
                maxabscontval = crate::def::max(maxabscontval, -val.to_dbl());
                isintegral = false;
            }

            val = prod_qd(val, lb);
            maxacttmp = sum_qq(maxacttmp, val);
        } else {
            if scip.set.is_infinity(ub) {
                return false;
            }

            if cutinds[i] < nintegralvars {
                maxabsintval = crate::def::max(maxabsintval, val.to_dbl());
            } else {
                maxabscontval = crate::def::max(maxabscontval, val.to_dbl());
                isintegral = false;
            }

            val = prod_qd(val, ub);
            maxacttmp = sum_qq(maxacttmp, val);
        }
    }

    let mut maxact = maxacttmp.to_dbl();

    // Cut is redundant in activity bounds.
    if scip.set.is_feas_le(maxact, cutrhs.to_dbl()) {
        return true;
    }

    // Cut is only on integral variables, try to scale to integral
    // coefficients.
    if isintegral {
        let equiscale = 1.0 / crate::def::min(maxact - cutrhs.to_dbl(), maxabsintval);

        let mut intcoeffs = vec![0.0; *cutnnz as usize];
        for i in 0..*cutnnz as usize {
            let val = prod_qd(cutcoefs[cutinds[i] as usize], equiscale);
            intcoeffs[i] = val.to_dbl();
        }

        let sumeps = scip.set.sumepsilon();
        let maxcoefratio = scip.set.sepa_maxcoefratio;
        let intscalar = crate::misc::calc_integral_scalar(&intcoeffs, -sumeps, sumeps, crate::misc::c_f64_to_i64(maxcoefratio), maxcoefratio);

        if let Some(mut intscalar) = intscalar {
            // If successful, apply the scaling.
            intscalar *= equiscale;

            *cutrhs = prod_qd(*cutrhs, intscalar);

            let mut i = 0;
            while i < *cutnnz as usize {
                let mut val = cutcoefs[cutinds[i] as usize];
                val = prod_qd(val, intscalar);

                let intval = scip.set.round(val.to_dbl());

                if chg_quad_coeff_with_bound(scip, vars[cutinds[i] as usize], val, intval, cutislocal, cutrhs) {
                    // TODO maybe change the coefficient to the other value instead
                    // of discarding the cut?
                    return true;
                }

                if intval != 0.0 {
                    cutcoefs[cutinds[i] as usize] = Quad::new(intval);
                    i += 1;
                } else {
                    // This must not be -0.0, otherwise the clean buffer memory is
                    // not cleared properly.
                    cutcoefs[cutinds[i] as usize] = Quad::new(0.0);
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                }
            }

            *cutrhs = crate::quad::eps_floor_q(*cutrhs, scip.set.feastol());

            // Recompute the maximal activity after scaling to integral values.
            maxacttmp = Quad::new(0.0);
            maxabsintval = 0.0;

            for i in 0..*cutnnz as usize {
                let mut val = cutcoefs[cutinds[i] as usize];
                let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

                if val.to_dbl() < 0.0 {
                    maxabsintval = crate::def::max(maxabsintval, -val.to_dbl());
                    val = prod_qd(val, lb);
                    maxacttmp = sum_qq(maxacttmp, val);
                } else {
                    maxabsintval = crate::def::max(maxabsintval, val.to_dbl());
                    val = prod_qd(val, ub);
                    maxacttmp = sum_qq(maxacttmp, val);
                }
            }

            maxacttmp = sum_qd(maxacttmp, 0.5);
            maxacttmp = crate::quad::floor_q(maxacttmp);
        } else {
            // Otherwise, apply the equilibrium scaling.
            isintegral = false;

            // Perform the scaling.
            maxacttmp = prod_qd(maxacttmp, equiscale);
            *cutrhs = prod_qd(*cutrhs, equiscale);
            maxabsintval *= equiscale;

            for i in 0..*cutnnz as usize {
                let c = &mut cutcoefs[cutinds[i] as usize];
                *c = prod_qd(*c, equiscale);
            }
        }
    } else {
        // Cut has integer and continuous variables, so scale it to
        // equilibrium.
        let mut maxabsval = maxact - cutrhs.to_dbl();
        maxabsval = crate::def::min(maxabsval, maxabsintval);
        maxabsval = crate::def::max(maxabsval, maxabscontval);

        let scale = 1.0 / maxabsval;

        // Perform the scaling.
        maxacttmp = prod_qd(maxacttmp, scale);
        *cutrhs = prod_qd(*cutrhs, scale);
        maxabsintval *= scale;

        for i in 0..*cutnnz as usize {
            let c = &mut cutcoefs[cutinds[i] as usize];
            *c = prod_qd(*c, scale);
        }
    }

    maxact = maxacttmp.to_dbl();

    // Check again for redundancy after scaling.
    if scip.set.is_feas_le(maxact, cutrhs.to_dbl()) {
        return true;
    }

    // No coefficient tightening can be performed since the precondition
    // doesn't hold for any of the variables.
    if scip.set.is_gt(maxact - maxabsintval, cutrhs.to_dbl()) {
        return false;
    }

    // First sort indices, so that in the following sort, the order for
    // coefficients with same absolute value does not depend on how cutinds
    // was initially ordered.
    let n = *cutnnz as usize;
    crate::sort::sort(&mut cutinds[..n], (), crate::sort::cmp_int);
    {
        let coefs: &[Quad] = cutcoefs;
        crate::sort::sort_down(&mut cutinds[..n], (), |a, b| compare_abs_coefs_quad(coefs, *a, *b));
    }

    // Loop over the integral variables and try to tighten the coefficients;
    // see cons_linear for more details.
    let mut i = 0;
    while i < *cutnnz as usize {
        if cutinds[i] >= nintegralvars {
            i += 1;
            continue;
        }

        let val = cutcoefs[cutinds[i] as usize];
        let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

        if val.to_dbl() < 0.0 && scip.set.is_le(maxact + val.to_dbl(), cutrhs.to_dbl()) {
            let mut coef = sum_qq(*cutrhs, -maxacttmp);

            if isintegral {
                // If cut is integral, the true coefficient must also be integral;
                // thus round it to exact integral value.
                coef = Quad::new(scip.set.round(coef.to_dbl()));
            }

            if coef.to_dbl() > val.to_dbl() {
                let mut delta = sum_qq(-val, coef);
                delta = prod_qd(delta, lb);

                let tmp = sum_qq(delta, *cutrhs);
                *cutrhs = tmp;

                if scip.set.is_negative(coef.to_dbl()) {
                    maxacttmp = sum_qq(maxacttmp, delta);
                    maxact = maxacttmp.to_dbl();
                    cutcoefs[cutinds[i] as usize] = coef;
                } else {
                    cutcoefs[cutinds[i] as usize] = Quad::new(0.0);
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                    continue;
                }
            }
        } else if val.to_dbl() > 0.0 && scip.set.is_le(maxact - val.to_dbl(), cutrhs.to_dbl()) {
            let mut coef = sum_qq(maxacttmp, -*cutrhs);

            if isintegral {
                // If cut is integral, the true coefficient must also be integral;
                // thus round it to exact integral value.
                coef = Quad::new(scip.set.round(coef.to_dbl()));
            }

            if coef.to_dbl() < val.to_dbl() {
                let mut delta = sum_qq(-val, coef);
                delta = prod_qd(delta, ub);

                let tmp = sum_qq(delta, *cutrhs);
                *cutrhs = tmp;

                if scip.set.is_positive(coef.to_dbl()) {
                    maxacttmp = sum_qq(maxacttmp, delta);
                    maxact = maxacttmp.to_dbl();
                    cutcoefs[cutinds[i] as usize] = coef;
                } else {
                    cutcoefs[cutinds[i] as usize] = Quad::new(0.0);
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                    continue;
                }
            }
        } else {
            // Due to sorting we can stop completely if the precondition was not
            // fulfilled for this variable.
            break;
        }

        i += 1;
    }

    false
}

/// `cutTightenCoefs`: scales the cut and then tightens the coefficients of
/// the given cut based on the maximal activity; see cons_linear.c
/// consdataTightenCoefs() for details; the cut is given in a semi-sparse
/// array; returns whether the cut was detected to be redundant.
fn cut_tighten_coefs(scip: &Scip, cutislocal: bool, cutcoefs: &mut [f64], cutrhs: &mut Quad, cutinds: &mut [i32], cutnnz: &mut i32) -> bool {
    let mut isintegral = true;
    let mut maxabsintval: f64 = 0.0;
    let mut maxabscontval: f64 = 0.0;

    let mut maxacttmp = Quad::new(0.0);

    let vars = get_vars(scip);
    let nintegralvars = crate::api::get_n_vars(scip) - crate::api::get_n_cont_vars(scip);

    // Compute maximal activity and maximal absolute coefficient values for all
    // and for integral variables in the cut.
    for i in 0..*cutnnz as usize {
        let val = cutcoefs[cutinds[i] as usize];
        let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

        if val < 0.0 {
            if scip.set.is_infinity(-lb) {
                return false;
            }

            if cutinds[i] < nintegralvars {
                maxabsintval = crate::def::max(maxabsintval, -val);
            } else {
                maxabscontval = crate::def::max(maxabscontval, -val);
                isintegral = false;
            }

            let quadprod = prod_dd(val, lb);
            maxacttmp = sum_qq(maxacttmp, quadprod);
        } else {
            if scip.set.is_infinity(ub) {
                return false;
            }

            if cutinds[i] < nintegralvars {
                maxabsintval = crate::def::max(maxabsintval, val);
            } else {
                maxabscontval = crate::def::max(maxabscontval, val);
                isintegral = false;
            }

            let quadprod = prod_dd(val, ub);
            maxacttmp = sum_qq(maxacttmp, quadprod);
        }
    }

    let mut maxact = maxacttmp.to_dbl();

    // Cut is redundant in activity bounds.
    if scip.set.is_feas_le(maxact, cutrhs.to_dbl()) {
        return true;
    }

    // Cut is only on integral variables, try to scale to integral
    // coefficients.
    if isintegral {
        let equiscale = 1.0 / crate::def::min(maxact - cutrhs.to_dbl(), maxabsintval);

        let mut intcoeffs = vec![0.0; *cutnnz as usize];
        for i in 0..*cutnnz as usize {
            intcoeffs[i] = equiscale * cutcoefs[cutinds[i] as usize];
        }

        let sumeps = scip.set.sumepsilon();
        let maxcoefratio = scip.set.sepa_maxcoefratio;
        let intscalar = crate::misc::calc_integral_scalar(&intcoeffs, -sumeps, sumeps, crate::misc::c_f64_to_i64(maxcoefratio), maxcoefratio);

        if let Some(mut intscalar) = intscalar {
            // If successful, apply the scaling.
            intscalar *= equiscale;

            *cutrhs = prod_qd(*cutrhs, intscalar);

            let mut i = 0;
            while i < *cutnnz as usize {
                let mut val = cutcoefs[cutinds[i] as usize];
                val *= intscalar;

                let intval = scip.set.round(val);

                if chg_coeff_with_bound(scip, vars[cutinds[i] as usize], val, intval, cutislocal, cutrhs) {
                    // TODO maybe change the coefficient to the other value instead
                    // of discarding the cut?
                    return true;
                }

                if intval != 0.0 {
                    cutcoefs[cutinds[i] as usize] = intval;
                    i += 1;
                } else {
                    // This must not be -0.0, otherwise the clean buffer memory is
                    // not cleared properly.
                    cutcoefs[cutinds[i] as usize] = 0.0;
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                }
            }

            *cutrhs = crate::quad::eps_floor_q(*cutrhs, scip.set.feastol());

            // Recompute the maximal activity after scaling to integral values.
            maxacttmp = Quad::new(0.0);
            maxabsintval = 0.0;

            for i in 0..*cutnnz as usize {
                let val = cutcoefs[cutinds[i] as usize];
                let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

                if val < 0.0 {
                    maxabsintval = crate::def::max(maxabsintval, -val);
                    let quadprod = prod_dd(val, lb);
                    maxacttmp = sum_qq(maxacttmp, quadprod);
                } else {
                    maxabsintval = crate::def::max(maxabsintval, val);
                    let quadprod = prod_dd(val, ub);
                    maxacttmp = sum_qq(maxacttmp, quadprod);
                }
            }

            maxacttmp = sum_qd(maxacttmp, 0.5);
            maxacttmp = crate::quad::floor_q(maxacttmp);
        } else {
            // Otherwise, apply the equilibrium scaling.
            isintegral = false;

            // Perform the scaling.
            maxacttmp = prod_qd(maxacttmp, equiscale);
            *cutrhs = prod_qd(*cutrhs, equiscale);
            maxabsintval *= equiscale;

            for i in 0..*cutnnz as usize {
                cutcoefs[cutinds[i] as usize] *= equiscale;
            }
        }
    } else {
        // Cut has integer and continuous variables, so scale it to
        // equilibrium.
        let mut maxabsval = maxact - cutrhs.to_dbl();
        maxabsval = crate::def::min(maxabsval, maxabsintval);
        maxabsval = crate::def::max(maxabsval, maxabscontval);

        let scale = 1.0 / maxabsval;

        // Perform the scaling.
        maxacttmp = prod_qd(maxacttmp, scale);
        *cutrhs = prod_qd(*cutrhs, scale);
        maxabsintval *= scale;

        for i in 0..*cutnnz as usize {
            cutcoefs[cutinds[i] as usize] *= scale;
        }
    }

    maxact = maxacttmp.to_dbl();

    // Check again for redundancy after scaling.
    if scip.set.is_feas_le(maxact, cutrhs.to_dbl()) {
        return true;
    }

    // No coefficient tightening can be performed since the precondition
    // doesn't hold for any of the variables.
    if scip.set.is_gt(maxact - maxabsintval, cutrhs.to_dbl()) {
        return false;
    }

    // First sort indices, so that in the following sort, the order for
    // coefficients with same absolute value does not depend on how cutinds
    // was initially ordered.
    let n = *cutnnz as usize;
    crate::sort::sort(&mut cutinds[..n], (), crate::sort::cmp_int);
    {
        let coefs: &[f64] = cutcoefs;
        crate::sort::sort_down(&mut cutinds[..n], (), |a, b| compare_abs_coefs(coefs, *a, *b));
    }

    // Loop over the integral variables and try to tighten the coefficients;
    // see cons_linear for more details.
    let mut i = 0;
    while i < *cutnnz as usize {
        if cutinds[i] >= nintegralvars {
            i += 1;
            continue;
        }

        let val = cutcoefs[cutinds[i] as usize];
        let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

        if val < 0.0 && scip.set.is_le(maxact + val, cutrhs.to_dbl()) {
            let mut coef = sum_qq(-maxacttmp, *cutrhs);

            if isintegral {
                // If cut is integral, the true coefficient must also be integral;
                // thus round it to exact integral value.
                coef = Quad::new(scip.set.round(coef.to_dbl()));
            }

            if coef.to_dbl() > val {
                let mut delta = sum_qd(coef, -val);
                delta = prod_qd(delta, lb);

                let tmp = sum_qq(delta, *cutrhs);
                *cutrhs = tmp;

                if scip.set.is_negative(coef.to_dbl()) {
                    maxacttmp = sum_qq(maxacttmp, delta);
                    maxact = maxacttmp.to_dbl();
                    cutcoefs[cutinds[i] as usize] = coef.to_dbl();
                } else {
                    cutcoefs[cutinds[i] as usize] = 0.0;
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                    continue;
                }
            }
        } else if val > 0.0 && scip.set.is_le(maxact - val, cutrhs.to_dbl()) {
            let mut coef = sum_qq(maxacttmp, -*cutrhs);

            if isintegral {
                // If cut is integral, the true coefficient must also be integral;
                // thus round it to exact integral value.
                coef = Quad::new(scip.set.round(coef.to_dbl()));
            }

            if coef.to_dbl() < val {
                let mut delta = sum_qd(coef, -val);
                delta = prod_qd(delta, ub);

                let tmp = sum_qq(delta, *cutrhs);
                *cutrhs = tmp;

                if scip.set.is_positive(coef.to_dbl()) {
                    maxacttmp = sum_qq(maxacttmp, delta);
                    maxact = maxacttmp.to_dbl();
                    cutcoefs[cutinds[i] as usize] = coef.to_dbl();
                } else {
                    cutcoefs[cutinds[i] as usize] = 0.0;
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                    continue;
                }
            }
        } else {
            // Due to sorting we can stop completely if the precondition was not
            // fulfilled for this variable.
            break;
        }

        i += 1;
    }

    false
}

/// `SCIPcutsTightenCoefficients`: perform activity based coefficient
/// tightening on the given cut (sparse coefficients); returns TRUE if the
/// cut was detected to be redundant due to activity bounds; counts changed
/// coefficients in `nchgcoefs`.
///
/// See also cons_linear.c:consdataTightenCoefs().
pub fn cuts_tighten_coefficients(scip: &Scip, cutislocal: bool, cutcoefs: &mut [f64], cutrhs: &mut f64, cutinds: &mut [i32], cutnnz: &mut i32, nchgcoefs: &mut i32) -> bool {
    let mut maxabsval: f64 = 0.0;

    let mut maxacttmp = Quad::new(0.0);

    let vars = get_vars(scip);
    let nintegralvars = crate::api::get_n_vars(scip) - crate::api::get_n_cont_vars(scip);
    let mut absvals = vec![0.0; *cutnnz as usize];

    *nchgcoefs = 0;

    for i in 0..*cutnnz as usize {
        let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

        if cutcoefs[i] < 0.0 {
            if scip.set.is_infinity(-lb) {
                return false;
            }

            if cutinds[i] < nintegralvars {
                maxabsval = crate::def::max(maxabsval, -cutcoefs[i]);
                absvals[i] = -cutcoefs[i];
            } else {
                absvals[i] = 0.0;
            }

            let quadprod = prod_dd(lb, cutcoefs[i]);
            maxacttmp = sum_qq(maxacttmp, quadprod);
        } else {
            if scip.set.is_infinity(ub) {
                return false;
            }

            if cutinds[i] < nintegralvars {
                maxabsval = crate::def::max(maxabsval, cutcoefs[i]);
                absvals[i] = cutcoefs[i];
            } else {
                absvals[i] = 0.0;
            }

            let quadprod = prod_dd(ub, cutcoefs[i]);
            maxacttmp = sum_qq(maxacttmp, quadprod);
        }
    }

    let mut maxact = maxacttmp.to_dbl();

    // Cut is redundant in activity bounds.
    if scip.set.is_feas_le(maxact, *cutrhs) {
        return true;
    }

    // Terminate, because coefficient tightening cannot be performed; also
    // excludes the case in which no integral variable is present.
    if scip.set.is_gt(maxact - maxabsval, *cutrhs) {
        return false;
    }

    let n = *cutnnz as usize;
    crate::sort::sort_down(&mut absvals[..n], (&mut cutcoefs[..n], &mut cutinds[..n]), crate::sort::cmp_real);

    // Loop over the integral variables and try to tighten the coefficients;
    // see cons_linear for more details.
    let mut i = 0;
    while i < *cutnnz as usize {
        // Due to sorting, we can exit if we reached a continuous variable: all
        // further integral variables have 0 coefficents anyway.
        if cutinds[i] >= nintegralvars {
            break;
        }

        let (lb, ub) = var_bounds(scip, vars[cutinds[i] as usize], cutislocal);

        if cutcoefs[i] < 0.0 && scip.set.is_le(maxact + cutcoefs[i], *cutrhs) {
            let mut coef = *cutrhs - maxact;
            coef = scip.set.floor(coef);

            if coef > cutcoefs[i] {
                let mut delta = sum_dd(coef, -cutcoefs[i]);
                delta = prod_qd(delta, lb);

                let tmp = sum_qd(delta, *cutrhs);
                *cutrhs = tmp.to_dbl();

                *nchgcoefs += 1;

                if scip.set.is_negative(coef) {
                    maxacttmp = sum_qq(maxacttmp, delta);
                    maxact = maxacttmp.to_dbl();
                    cutcoefs[i] = coef;
                } else {
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                    cutcoefs[i] = cutcoefs[*cutnnz as usize];
                    // The C `for` loop's increment still runs on `continue`.
                    i += 1;
                    continue;
                }
            }
        } else if cutcoefs[i] > 0.0 && scip.set.is_le(maxact - cutcoefs[i], *cutrhs) {
            let mut coef = maxact - *cutrhs;
            coef = scip.set.ceil(coef);

            if coef < cutcoefs[i] {
                let mut delta = sum_dd(coef, -cutcoefs[i]);
                delta = prod_qd(delta, ub);

                let tmp = sum_qd(delta, *cutrhs);
                *cutrhs = tmp.to_dbl();

                *nchgcoefs += 1;

                if scip.set.is_positive(coef) {
                    maxacttmp = sum_qq(maxacttmp, delta);
                    maxact = maxacttmp.to_dbl();
                    cutcoefs[i] = coef;
                } else {
                    *cutnnz -= 1;
                    cutinds[i] = cutinds[*cutnnz as usize];
                    cutcoefs[i] = cutcoefs[*cutnnz as usize];
                    // The C `for` loop's increment still runs on `continue`.
                    i += 1;
                    continue;
                }
            }
        } else {
            // Due to sorting we can stop completely if the precondition was not
            // fulfilled for this variable.
            break;
        }

        i += 1;
    }

    false
}

/*
 * Aggregation row
 */

/// `SCIP_AGGRROW`.
#[derive(Clone, Debug, Default)]
pub struct AggrRow {
    /// Non-zero coefficients of the cut row, indexed by problem index.
    pub vals: Vec<Quad>,
    /// Problem indices of variables with a non-zero coefficient in the cut
    /// row.
    pub inds: Vec<i32>,
    /// LP position of rows that have been added to the cutrow.
    pub rowsinds: Vec<i32>,
    /// Slacksign of rows that have been added to the cutrow.
    pub slacksign: Vec<i32>,
    /// Weights of rows that have been added to the cutrow.
    pub rowweights: Vec<f64>,
    /// Right hand side of the cut row.
    pub rhs: Quad,
    /// Number of non-zeros in the cut row.
    pub nnz: i32,
    /// Number of rows that have been added to the cutrow.
    pub nrows: i32,
    /// Size of the row and slacksign array.
    pub rowssize: i32,
    /// Rank of the cut row.
    pub rank: i32,
    /// Is the cut row only valid locally?
    pub local: bool,
}

/// `SCIPaggrRowCreate`: create an empty aggregation row.
pub fn aggr_row_create(scip: &Scip) -> AggrRow {
    let nvars = crate::api::get_n_vars(scip) as usize;
    AggrRow {
        vals: vec![Quad::new(0.0); nvars],
        inds: vec![0; nvars],
        ..AggrRow::default()
    }
}

/// `SCIPaggrRowCopy`: copy a aggregation row.
pub fn aggr_row_copy(source: &AggrRow) -> AggrRow {
    let n = source.nrows as usize;
    AggrRow {
        vals: source.vals.clone(),
        inds: source.inds.clone(),
        rowsinds: source.rowsinds[..n].to_vec(),
        slacksign: source.slacksign[..n].to_vec(),
        rowweights: source.rowweights[..n].to_vec(),
        rhs: source.rhs,
        nnz: source.nnz,
        nrows: source.nrows,
        rowssize: source.nrows,
        rank: source.rank,
        local: source.local,
    }
}

/// Grows the row arrays of the aggregation row to hold `aggrrow.nrows`
/// rows.
fn aggr_row_ensure_rows(scip: &Scip, aggrrow: &mut AggrRow) {
    if aggrrow.nrows > aggrrow.rowssize {
        let newsize = scip.set.calc_mem_grow_size(aggrrow.nrows);
        aggrrow.rowsinds.resize(newsize as usize, 0);
        aggrrow.slacksign.resize(newsize as usize, 0);
        aggrrow.rowweights.resize(newsize as usize, 0.0);
        aggrrow.rowssize = newsize;
    }
}

/// `SCIPaggrRowAddRow`: add weighted row to aggregation row; `sidetype`
/// specifies the row side type (-1 = lhs, 0 = automatic, 1 = rhs).
pub fn aggr_row_add_row(scip: &Scip, aggrrow: &mut AggrRow, row: RowId, weight: f64, sidetype: i32) {
    let r = &scip.rows[row];

    // Update local flag.
    aggrrow.local = aggrrow.local || r.local;

    // Update rank.
    aggrrow.rank = r.rank.max(aggrrow.rank);

    let i = aggrrow.nrows as usize;
    aggrrow.nrows += 1;
    aggr_row_ensure_rows(scip, aggrrow);
    aggrrow.rowsinds[i] = r.lppos;
    aggrrow.rowweights[i] = weight;

    let uselhs = if sidetype == -1 {
        true
    } else if sidetype == 1 {
        false
    } else {
        // Automatically decide, whether we want to use the left or the right
        // hand side of the row in the summation. If possible, use the side
        // that leads to a positive slack value in the summation.
        scip.set.is_infinity(r.rhs) || (!scip.set.is_infinity(-r.lhs) && weight < 0.0)
    };

    let mut sideval;
    if uselhs {
        aggrrow.slacksign[i] = -1;
        sideval = r.lhs - r.constant;
        if r.integral {
            // Row is integral: round left hand side up.
            sideval = scip.set.ceil(sideval);
        }
    } else {
        aggrrow.slacksign[i] = 1;
        sideval = r.rhs - r.constant;
        if r.integral {
            // Row is integral: round right hand side up.
            sideval = scip.set.floor(sideval);
        }
    }

    let quadprod = prod_dd(weight, sideval);
    aggrrow.rhs = sum_qq(aggrrow.rhs, quadprod);

    // Add up coefficients.
    let AggrRow { inds, vals, nnz, .. } = aggrrow;
    var_vec_add_scaled_row_coefs_quad(scip, inds, vals, nnz, row, weight);
}

/// `SCIPaggrRowCancelVarWithBound`: removes a given variable `var` from
/// position `pos` the aggregation row and updates the right-hand side
/// according to sign of the coefficient, i.e., rhs -= coef * bound, where
/// bound = lb if coef >= 0 and bound = ub, otherwise; returns whether the
/// aggregation row is still valid.
///
/// The choice of global or local bounds depend on the validity (global or
/// local) of the aggregation row. The list of non-zero indices will be
/// updated by swapping the last non-zero index to `pos`.
pub fn aggr_row_cancel_var_with_bound(scip: &Scip, aggrrow: &mut AggrRow, var: VarId, pos: i32) -> bool {
    let v = aggrrow.inds[pos as usize] as usize;
    let mut val = aggrrow.vals[v];
    let mut valid = true;
    let (lb, ub) = var_bounds(scip, var, aggrrow.local);

    // Adjust left and right hand sides with max contribution.
    if val.to_dbl() < 0.0 {
        if scip.set.is_infinity(ub) {
            aggrrow.rhs = Quad::new(scip.set.infinity());
        } else {
            val = prod_qd(val, ub);
            aggrrow.rhs = sum_qq(aggrrow.rhs, -val);
        }
    } else if scip.set.is_infinity(-lb) {
        aggrrow.rhs = Quad::new(scip.set.infinity());
    } else {
        val = prod_qd(val, lb);
        aggrrow.rhs = sum_qq(aggrrow.rhs, -val);
    }

    aggrrow.vals[v] = Quad::new(0.0);

    // Remove non-zero entry.
    aggrrow.nnz -= 1;
    aggrrow.inds[pos as usize] = aggrrow.inds[aggrrow.nnz as usize];

    if scip.set.is_infinity(aggrrow.rhs.hi) {
        valid = false;
    }
    valid
}

/// `SCIPaggrRowAddObjectiveFunction`: add the objective function with
/// right-hand side `rhs` and scaled by `scale` to the aggregation row.
pub fn aggr_row_add_objective_function(scip: &Scip, aggrrow: &mut AggrRow, rhs: f64, scale: f64) {
    let vars = get_vars(scip);
    let nvars = vars.len();

    // Add all variables straight forward if the aggregation row is empty.
    if aggrrow.nnz == 0 {
        for i in 0..nvars {
            let obj = scip.vars[vars[i]].obj;
            // Skip all variables with zero objective coefficient.
            if scip.set.is_zero(scale * obj) {
                continue;
            }

            aggrrow.vals[i] = Quad::new(scale * obj);
            aggrrow.inds[aggrrow.nnz as usize] = i as i32;
            aggrrow.nnz += 1;
        }

        // Add right-hand side value.
        aggrrow.rhs = Quad::new(scale * rhs);
    } else {
        // Add the non-zeros to the aggregation row and keep non-zero index up
        // to date.
        for i in 0..nvars {
            let varobj = scip.vars[vars[i]].obj;
            // Skip all variables with zero objective coefficient.
            if scip.set.is_zero(scale * varobj) {
                continue;
            }

            let mut val = aggrrow.vals[i];

            if val.hi == 0.0 {
                aggrrow.inds[aggrrow.nnz as usize] = i as i32;
                aggrrow.nnz += 1;
            }

            let quadprod = prod_dd(scale, varobj);
            val = sum_qq(val, quadprod);

            // The value must not be exactly zero due to sparsity pattern.
            val.hi = nonzero(val.hi);

            aggrrow.vals[i] = val;
        }

        // Add right-hand side value.
        let quadprod = prod_dd(scale, rhs);
        aggrrow.rhs = sum_qq(aggrrow.rhs, quadprod);
    }
}

/// `SCIPaggrRowAddCustomCons`: add weighted constraint to the aggregation
/// row.
#[allow(clippy::too_many_arguments)]
pub fn aggr_row_add_custom_cons(aggrrow: &mut AggrRow, inds: &[i32], vals: &[f64], len: i32, rhs: f64, weight: f64, rank: i32, local: bool) {
    // Update local flag.
    aggrrow.local = aggrrow.local || local;

    // Update rank.
    aggrrow.rank = rank.max(aggrrow.rank);

    // Add right hand side value.
    let quadprod = prod_dd(weight, rhs);
    aggrrow.rhs = sum_qq(aggrrow.rhs, quadprod);

    // Add the non-zeros to the aggregation row and keep non-zero index up to
    // date.
    for i in 0..len as usize {
        let probindex = inds[i] as usize;
        let mut val = aggrrow.vals[probindex];

        if val.hi == 0.0 {
            aggrrow.inds[aggrrow.nnz as usize] = probindex as i32;
            aggrrow.nnz += 1;
        }

        let quadprod = prod_dd(vals[i], weight);
        val = sum_qq(val, quadprod);

        // The value must not be exactly zero due to sparsity pattern.
        val.hi = nonzero(val.hi);

        aggrrow.vals[probindex] = val;
    }
}

/// `SCIPaggrRowClear`: clear all entries int the aggregation row but don't
/// free memory.
pub fn aggr_row_clear(aggrrow: &mut AggrRow) {
    for i in 0..aggrrow.nnz as usize {
        aggrrow.vals[aggrrow.inds[i] as usize] = Quad::new(0.0);
    }

    aggrrow.nnz = 0;
    aggrrow.nrows = 0;
    aggrrow.rank = 0;
    aggrrow.rhs = Quad::new(0.0);
    aggrrow.local = false;
}

/// `SCIPaggrRowCalcEfficacyNorm`: calculates the efficacy norm of the
/// given aggregation row, which depends on the "separating/efficacynorm"
/// parameter.
pub fn aggr_row_calc_efficacy_norm(scip: &Scip, aggrrow: &AggrRow) -> f64 {
    calc_efficacy_norm_quad(scip, &aggrrow.vals, &aggrrow.inds, aggrrow.nnz)
}

/// `addOneRow`: adds one row to the aggregation row; differs from
/// SCIPaggrRowAddRow() by providing some additional parameters required
/// for SCIPaggrRowSumRows(); returns whether the aggregated row is too
/// long. `negslack`: should negative slack variables allowed to be used?
/// (0: no, 1: only for integral rows, 2: yes).
#[allow(clippy::too_many_arguments)]
fn add_one_row(scip: &Scip, aggrrow: &mut AggrRow, row: RowId, weight: f64, sidetypebasis: bool, allowlocal: bool, negslack: i32, maxaggrlen: i32) -> bool {
    let r = &scip.rows[row];

    if scip.set.is_feas_zero(weight) || r.modifiable || (r.local && !allowlocal) {
        return false;
    }

    let uselhs;
    if sidetypebasis && !scip.set.is_eq(r.lhs, r.rhs) {
        let stat = r.basisstatus;

        if stat == crate::lpi::BASESTAT_LOWER {
            uselhs = true;
        } else if stat == crate::lpi::BASESTAT_UPPER {
            uselhs = false;
        } else {
            uselhs = scip.set.is_infinity(r.rhs) || (weight < 0.0 && !scip.set.is_infinity(-r.lhs));
        }
    } else {
        uselhs = (weight < 0.0 && !scip.set.is_infinity(-r.lhs)) || scip.set.is_infinity(r.rhs);
    }

    let mut sideval;
    if uselhs {
        if weight > 0.0 && (negslack == 0 || (negslack == 1 && !r.integral)) {
            return false;
        }

        sideval = r.lhs - r.constant;
        // Row is integral? round left hand side up.
        if r.integral {
            sideval = scip.set.ceil(sideval);
        }
    } else {
        if weight < 0.0 && (negslack == 0 || (negslack == 1 && !r.integral)) {
            return false;
        }

        sideval = r.rhs - r.constant;
        // Row is integral? round right hand side down.
        if r.integral {
            sideval = scip.set.floor(sideval);
        }
    }

    // Add right hand side, update rank and local flag.
    let quadprod = prod_dd(sideval, weight);
    aggrrow.rhs = sum_qq(aggrrow.rhs, quadprod);
    aggrrow.rank = aggrrow.rank.max(r.rank);
    aggrrow.local = aggrrow.local || r.local;

    // Ensure the array for storing the row information is large enough.
    let i = aggrrow.nrows as usize;
    aggrrow.nrows += 1;
    aggr_row_ensure_rows(scip, aggrrow);

    // Add information of addditional row.
    aggrrow.rowsinds[i] = r.lppos;
    aggrrow.rowweights[i] = weight;
    aggrrow.slacksign[i] = if uselhs { -1 } else { 1 };

    // Add up coefficients.
    {
        let AggrRow { inds, vals, nnz, .. } = aggrrow;
        var_vec_add_scaled_row_coefs_quad(scip, inds, vals, nnz, row, weight);
    }

    // Check if row is too long now.
    aggrrow.nnz > maxaggrlen
}

/// `SCIPaggrRowSumRows`: aggregate rows using the given weights; the
/// current content of the aggregation row is overwritten; returns whether
/// the aggregation is valid. `rowinds` holds the indices of non-zero
/// entries of the weights array, or `None`.
#[allow(clippy::too_many_arguments)]
pub fn aggr_row_sum_rows(
    scip: &Scip,
    aggrrow: &mut AggrRow,
    weights: &[f64],
    rowinds: Option<&[i32]>,
    sidetypebasis: bool,
    allowlocal: bool,
    negslack: i32,
    maxaggrlen: i32,
) -> bool {
    let lp = scip.lp.as_ref().unwrap();
    let rows = &lp.rows.0[..lp.nrows as usize];

    aggr_row_clear(aggrrow);
    let mut valid = true;

    if let Some(rowinds) = rowinds {
        for &k in rowinds {
            let k = k as usize;
            let rowtoolong = add_one_row(scip, aggrrow, rows[k], weights[k], sidetypebasis, allowlocal, negslack, maxaggrlen);

            if rowtoolong {
                valid = false;
            }

            if !valid {
                break;
            }
        }
    } else {
        for k in 0..rows.len() {
            if weights[k] != 0.0 {
                let rowtoolong = add_one_row(scip, aggrrow, rows[k], weights[k], sidetypebasis, allowlocal, negslack, maxaggrlen);

                if rowtoolong {
                    valid = false;
                }

                if !valid {
                    break;
                }
            }
        }
    }

    if valid {
        valid = aggr_row_remove_zeros(scip, aggrrow, false);
    }

    valid
}

/// `postprocessCut`: checks for cut redundancy and performs activity based
/// coefficient tightening; removes coefficients that are zero with
/// QUAD_EPSILON tolerance and uses variable bounds to remove small
/// coefficients (relative to the maximum absolute coefficient); returns
/// whether post-processing was successful (false: the cut is redundant).
fn postprocess_cut(scip: &Scip, cutislocal: bool, cutinds: &mut [i32], cutcoefs: &mut [f64], nnz: &mut i32, cutrhs: &mut f64) -> bool {
    let mut rhs = Quad::new(*cutrhs);

    if remove_zeros(scip, scip.set.feastol(), cutislocal, cutcoefs, &mut rhs, cutinds, nnz) {
        // Right hand side was changed to infinity -> cut is redundant.
        return false;
    }

    if *nnz == 0 {
        return false;
    }

    if cut_tighten_coefs(scip, cutislocal, cutcoefs, &mut rhs, cutinds, nnz) {
        // Cut is redundant.
        return false;
    }

    let mut maxcoef: f64 = 0.0;
    for i in 0..*nnz as usize {
        let absval = cutcoefs[cutinds[i] as usize].abs();
        maxcoef = crate::def::max(absval, maxcoef);
    }

    maxcoef /= scip.set.sepa_maxcoefratio;
    let mut minallowedcoef = scip.set.sumepsilon();
    minallowedcoef = crate::def::max(minallowedcoef, maxcoef);

    let success = !remove_zeros(scip, minallowedcoef, cutislocal, cutcoefs, &mut rhs, cutinds, nnz);
    *cutrhs = rhs.to_dbl();

    success
}

/// `postprocessCutQuad`: checks for cut redundancy and performs activity
/// based coefficient tightening; removes coefficients that are zero with
/// QUAD_EPSILON tolerance and uses variable bounds to remove small
/// coefficients (relative to the maximum absolute coefficient); the
/// cutcoefs are quad precision; returns whether the cleanup was successful
/// (false: it is useless).
fn postprocess_cut_quad(scip: &Scip, cutislocal: bool, cutinds: &mut [i32], cutcoefs: &mut [Quad], nnz: &mut i32, cutrhs: &mut Quad) -> bool {
    if remove_zeros_quad(scip, scip.set.feastol(), cutislocal, cutcoefs, cutrhs, cutinds, nnz) {
        // Right hand side was changed to infinity -> cut is redundant.
        return false;
    }

    if *nnz == 0 {
        return false;
    }

    if cut_tighten_coefs_quad(scip, cutislocal, cutcoefs, cutrhs, cutinds, nnz) {
        // Cut is redundant.
        return false;
    }

    let mut maxcoef: f64 = 0.0;
    for i in 0..*nnz as usize {
        let abscoef = cutcoefs[cutinds[i] as usize].to_dbl().abs();
        maxcoef = crate::def::max(abscoef, maxcoef);
    }

    maxcoef /= scip.set.sepa_maxcoefratio;
    let mut minallowedcoef = scip.set.sumepsilon();
    minallowedcoef = crate::def::max(minallowedcoef, maxcoef);

    !remove_zeros_quad(scip, minallowedcoef, cutislocal, cutcoefs, cutrhs, cutinds, nnz)
}

/// `SCIPaggrRowRemoveZeros`: removes almost zero entries from the
/// aggregation row; returns whether the aggregation row is still valid.
/// `useglbbounds`: consider global bound although the cut is local?
pub fn aggr_row_remove_zeros(scip: &Scip, aggrrow: &mut AggrRow, useglbbounds: bool) -> bool {
    let local = if useglbbounds { false } else { aggrrow.local };
    let AggrRow { vals, rhs, inds, nnz, .. } = aggrrow;
    !remove_zeros_quad(scip, scip.set.sumepsilon(), local, vals, rhs, inds, nnz)
}

/// `SCIPaggrRowHasRowBeenAdded`: checks whether a given row has been added
/// to the aggregation row.
pub fn aggr_row_has_row_been_added(scip: &Scip, aggrrow: &AggrRow, row: RowId) -> bool {
    let rowind = scip.rows[row].lppos;
    aggrrow.rowsinds[..aggrrow.nrows as usize].contains(&rowind)
}

/// `SCIPaggrRowGetInds`: gets the array of corresponding variable problem
/// indices for each non-zero in the aggregation row.
#[inline]
pub fn aggr_row_get_inds(aggrrow: &AggrRow) -> &[i32] {
    &aggrrow.inds
}

/// `SCIPaggrRowGetNNz`: gets the number of non-zeros in the aggregation
/// row.
#[inline]
pub fn aggr_row_get_n_nz(aggrrow: &AggrRow) -> i32 {
    aggrrow.nnz
}

/// `SCIPaggrRowGetRhs`: gets the right hand side of the aggregation row.
#[inline]
pub fn aggr_row_get_rhs(aggrrow: &AggrRow) -> f64 {
    aggrrow.rhs.to_dbl()
}

/// `SCIPaggrRowGetProbvarValue` (`cuts.h`): gets the non-zero value for
/// the given problem index of a variable.
#[inline]
pub fn aggr_row_get_probvar_value(aggrrow: &AggrRow, probindex: i32) -> f64 {
    aggrrow.vals[probindex as usize].to_dbl()
}

/// `SCIPaggrRowGetValue` (`cuts.h`): gets the non-zero value for the
/// given non-zero index.
#[inline]
pub fn aggr_row_get_value(aggrrow: &AggrRow, i: i32) -> f64 {
    aggrrow.vals[aggrrow.inds[i as usize] as usize].to_dbl()
}

/*
 * c-MIR
 */

/// `MAXCMIRSCALE`: maximal scaling (scale/(1-f0)) allowed in c-MIR
/// calculations.
const MAXCMIRSCALE: f64 = 1e+6;

/// `NSECTIONS`: in order to derive cuts, we partition the variable array up
/// in (not necessarily contiguous) sections. The only requirement we place
/// on these sections is that section i can only have variable bounds
/// variables whose section is strictly greater than i. This way, we can
/// process the variable array in a 'linear' manner.
///
/// @todo maintain a DAG for used varbounds and use topological ordering
/// instead, this would also allow variable bounds on variables of the same
/// section to be used.
const NSECTIONS: usize = 6;

/// `MIR_Data`.
#[derive(Default)]
struct MirData {
    /// The total number of nonzeros in all of the sections.
    totalnnz: i32,
    /// The indices of the variables belonging to the section.
    secindices: [Vec<i32>; NSECTIONS],
    /// The number of nonzero indices in the section.
    secnnz: [i32; NSECTIONS],
    /// Does the section have an integrality constraint?
    isenfint: [bool; NSECTIONS],
    /// Is the section implied integer variables?
    isimplint: [bool; NSECTIONS],
    /// Should variable bound substitution be done for this section?
    usevbds: [i32; NSECTIONS],
    /// Total number of variables.
    nvars: i32,
    /// Total number of non-implint binary variables.
    nbinvars: i32,
    /// Total number of non-implint integer variables.
    nintvars: i32,
    /// Total number of implint binary variables.
    nbinimplvars: i32,
    /// Total number of implint integer variables.
    nintimplvars: i32,
    /// Total number of implint continuous variables.
    ncontimplvars: i32,
    /// Total number of non-implied continuous variables.
    ncontvars: i32,
    /// Working cut indices value array.
    cutcoefs: Vec<Quad>,
    /// The working right hand side of the cut.
    cutrhs: Quad,
    /// Working cut variable problem index array.
    cutinds: Vec<i32>,
    /// Number of values in the working cut variable problem index array.
    ncutinds: i32,
}

/// `varSection`: returns the section of a variable; for now, this is
/// equal to the variable type section of the variable in the problem.
fn var_section(data: &MirData, probindex: i32) -> usize {
    let mut limit = data.nvars - data.ncontvars;
    if probindex >= limit {
        return 0;
    }
    limit -= data.ncontimplvars;
    if probindex >= limit {
        return 1;
    }
    limit -= data.nintimplvars;
    if probindex >= limit {
        return 2;
    }
    limit -= data.nbinimplvars;
    if probindex >= limit {
        return 3;
    }
    limit -= data.nintvars;
    if probindex >= limit {
        return 4;
    }
    5
}

/// `findBestLb`: finds the best lower bound of the variable to use for MIR
/// transformation; `usevbds`: should variable bounds be used in bound
/// transformation? (0: no, 1: only binary, 2: all); returns the best bound
/// value and its type (-2: local bound, -1: global bound, >= 0 variable
/// bound index).
fn find_best_lb(scip: &mut Scip, var: VarId, sol: SolId, usevbds: i32, allowlocal: bool) -> (f64, i32) {
    let mut bestlb = scip.vars[var].glbdom.lb;
    let mut bestlbtype = -1;

    if allowlocal {
        let loclb = scip.vars[var].locdom.lb;
        if scip.set.is_gt(loclb, bestlb) {
            bestlb = loclb;
            bestlbtype = -2;
        }
    }

    if usevbds > 0 {
        let mut bestvlb = 0.0;
        let mut bestvlbidx = 0;

        crate::var::var_get_closest_vlb(scip, var, sol, &mut bestvlb, &mut bestvlbidx);
        if bestvlbidx >= 0 && (bestvlb > bestlb || (bestlbtype < 0 && scip.set.is_ge(bestvlb, bestlb))) {
            // We have to avoid cyclic variable bound usage, so we enforce to use
            // only variable bounds variables of smaller index.
            // @todo this check is not needed for continuous variables; but
            // allowing all but binary variables to be replaced by variable
            // bounds seems to be buggy (wrong result on gesa2).
            let vlbvar = scip.vars[var].vlbs.as_ref().unwrap().vars[bestvlbidx];
            if usevbds == 2 || (scip.vars[vlbvar].vartype == crate::types::VarType::Binary && !crate::var::var_is_implied_integral(scip, vlbvar)) {
                bestlb = bestvlb;
                bestlbtype = bestvlbidx;
            }
        }
    }

    (bestlb, bestlbtype)
}

/// `findBestUb`: finds the best upper bound of the variable to use for MIR
/// transformation; see [`find_best_lb`].
fn find_best_ub(scip: &mut Scip, var: VarId, sol: SolId, usevbds: i32, allowlocal: bool) -> (f64, i32) {
    let mut bestub = scip.vars[var].glbdom.ub;
    let mut bestubtype = -1;

    if allowlocal {
        let locub = scip.vars[var].locdom.ub;
        if scip.set.is_lt(locub, bestub) {
            bestub = locub;
            bestubtype = -2;
        }
    }

    if usevbds > 0 {
        let mut bestvub = 0.0;
        let mut bestvubidx = 0;

        crate::var::var_get_closest_vub(scip, var, sol, &mut bestvub, &mut bestvubidx);
        if bestvubidx >= 0 && (bestvub < bestub || (bestubtype < 0 && scip.set.is_le(bestvub, bestub))) {
            // We have to avoid cyclic variable bound usage, so we enforce to use
            // only variable bounds variables of smaller index.
            // @todo this check is not needed for continuous variables; but
            // allowing all but binary variables to be replaced by variable
            // bounds seems to be buggy (wrong result on gesa2).
            let vubvar = scip.vars[var].vubs.as_ref().unwrap().vars[bestvubidx];
            if usevbds == 2 || (scip.vars[vubvar].vartype == crate::types::VarType::Binary && !crate::var::var_is_implied_integral(scip, vubvar)) {
                bestub = bestvub;
                bestubtype = bestvubidx;
            }
        }
    }

    (bestub, bestubtype)
}

/// `findMIRBestLb` and `findMIRBestUb`: finds the best lower (upper) bound
/// of the variable to use for MIR transformation; differs from
/// findBestLB() in that it allows more variable bound substitutions based
/// on the variable sections; returns `(bestbound, simplebound, besttype)`.
fn find_mir_best_bound(scip: &Scip, var: VarId, sol: SolId, data: &MirData, usevbds: i32, allowlocal: bool, upper: bool) -> (f64, f64, i32) {
    let v = &scip.vars[var];
    let mut best = if upper { v.glbdom.ub } else { v.glbdom.lb };
    let mut besttype = -1;

    if allowlocal {
        let loc = if upper { v.locdom.ub } else { v.locdom.lb };
        if if upper { scip.set.is_lt(loc, best) } else { scip.set.is_gt(loc, best) } {
            best = loc;
            besttype = -2;
        }
    }

    let simplebound = best;

    if usevbds > 0 {
        let vbds = if upper { &v.vubs } else { &v.vlbs };
        let nvbds = crate::implics::vbounds_get_n_vbds(vbds);
        if nvbds > 0 {
            let mut bestvbd = if upper { f64::MAX } else { f64::MIN };
            let mut bestvbdtype = -1;
            let boundedsection = var_section(data, v.probindex);
            let vb = vbds.as_ref().unwrap();

            // Search best VLB (VUB).
            for i in 0..nvbds as usize {
                // For now, we only allow variable bounds from sections that are
                // strictly greater to prevent cyclic usage.
                // @todo: We don't use the caching mechanism of
                // SCIPvarGetClosestVLB() because the cached variable bound may be
                // illegal. Building a local cache here may be worth it.
                let bvar = vb.vars[i];
                if crate::var::var_is_active(scip, bvar) && boundedsection < var_section(data, scip.vars[bvar].probindex) && (usevbds == 2 || crate::var::var_is_binary(scip, bvar))
                {
                    let vbdsol = get_sol_val(scip, sol, bvar);
                    let vbdbnd = vb.coefs[i] * vbdsol + vb.constants[i];
                    if if upper { vbdbnd < bestvbd } else { vbdbnd > bestvbd } {
                        bestvbd = vbdbnd;
                        bestvbdtype = i as i32;
                    }
                }
            }

            if bestvbdtype >= 0 && if upper { scip.set.is_le(bestvbd, best) } else { scip.set.is_ge(bestvbd, best) } {
                best = bestvbd;
                besttype = bestvbdtype;
            }
        }
    }

    (best, simplebound, besttype)
}

/// What `determineBestBounds` found for a variable.
#[derive(Clone, Copy, Default)]
struct BestBounds {
    bestlb: f64,
    bestub: f64,
    bestlbtype: i32,
    bestubtype: i32,
    selectedbound: crate::types::BoundType,
    freevariable: bool,
}

/// `determineBestBounds`: determine the best bounds with respect to the
/// given solution for complementing the given variable; `boundsfortrans`
/// are the bounds that should be used for transformed variables:
/// vlb_idx/vub_idx, -1 for global lb/ub, -2 for local lb/ub, or -3 for
/// using closest bound (`None` for using closest bound for all variables),
/// with their types in `boundtypesfortrans`.
#[allow(clippy::too_many_arguments)]
fn determine_best_bounds(
    scip: &Scip,
    var: VarId,
    sol: SolId,
    data: &MirData,
    boundswitch: f64,
    usevbds: i32,
    allowlocal: bool,
    fixintegralrhs: bool,
    ignoresol: bool,
    boundsfortrans: Option<&[i32]>,
    boundtypesfortrans: Option<&[crate::types::BoundType]>,
) -> BestBounds {
    use crate::types::BoundType;
    let mut b = BestBounds::default();
    let x = &scip.vars[var];
    let v = x.probindex as usize;

    // Check if the user specified a bound to be used.
    if let Some(bft) = boundsfortrans
        && bft[v] > -3
    {
        // User has explicitly specified a bound to be used.
        if boundtypesfortrans.unwrap()[v] == BoundType::Lower {
            // User wants to use lower bound.
            b.bestlbtype = bft[v];
            if b.bestlbtype == -1 {
                // Use global standard lower bound.
                b.bestlb = x.glbdom.lb;
            } else if b.bestlbtype == -2 {
                // Use local standard lower bound.
                b.bestlb = x.locdom.lb;
            } else {
                // Use the given variable lower bound.
                let vb = x.vlbs.as_ref().unwrap();
                let k = bft[v] as usize;
                let vlbsol = get_sol_val(scip, sol, vb.vars[k]);
                b.bestlb = vb.coefs[k] * vlbsol + vb.constants[k];
            }

            b.selectedbound = BoundType::Lower;

            // Find closest upper bound in standard upper bound (and variable
            // upper bounds for continuous variables).
            (b.bestub, _, b.bestubtype) = find_mir_best_bound(scip, var, sol, data, if fixintegralrhs { usevbds } else { 0 }, allowlocal && fixintegralrhs, true);
        } else {
            // User wants to use upper bound.
            b.bestubtype = bft[v];
            if b.bestubtype == -1 {
                // Use global standard upper bound.
                b.bestub = x.glbdom.ub;
            } else if b.bestubtype == -2 {
                // Use local standard upper bound.
                b.bestub = x.locdom.ub;
            } else {
                // Use the given variable upper bound.
                let vb = x.vubs.as_ref().unwrap();
                let k = bft[v] as usize;
                let vubsol = get_sol_val(scip, sol, vb.vars[k]);
                b.bestub = vb.coefs[k] * vubsol + vb.constants[k];
            }

            b.selectedbound = BoundType::Upper;

            // Find closest lower bound in standard lower bound (and variable
            // lower bounds for continuous variables).
            (b.bestlb, _, b.bestlbtype) = find_mir_best_bound(scip, var, sol, data, if fixintegralrhs { usevbds } else { 0 }, allowlocal && fixintegralrhs, false);
        }
    } else {
        // Bound selection should be done automatically.

        // Find closest lower bound in standard lower bound (and variable lower
        // bounds for continuous variables).
        let simplelb;
        (b.bestlb, simplelb, b.bestlbtype) = find_mir_best_bound(scip, var, sol, data, usevbds, allowlocal, false);

        // Find closest upper bound in standard upper bound (and variable upper
        // bounds for continuous variables).
        let simpleub;
        (b.bestub, simpleub, b.bestubtype) = find_mir_best_bound(scip, var, sol, data, usevbds, allowlocal, true);

        // Check, if variable is free variable.
        if scip.set.is_infinity(-b.bestlb) && scip.set.is_infinity(b.bestub) {
            // We found a free variable in the row with non-zero coefficient ->
            // MIR row can't be transformed in standard form.
            b.freevariable = true;
            return b;
        }

        if !ignoresol {
            // Select transformation bound.
            let varsol = get_sol_val(scip, sol, var);
            let mid = (1.0 - boundswitch) * b.bestlb + boundswitch * b.bestub;

            b.selectedbound = if scip.set.is_infinity(b.bestub) {
                // If there is no ub, use lb.
                BoundType::Lower
            } else if scip.set.is_infinity(-b.bestlb) {
                // If there is no lb, use ub.
                BoundType::Upper
            } else if scip.set.is_lt(varsol, mid) {
                BoundType::Lower
            } else if scip.set.is_gt(varsol, mid) {
                BoundType::Upper
            } else if b.bestlbtype == -1 {
                // Prefer global standard bounds.
                BoundType::Lower
            } else if b.bestubtype == -1 {
                // Prefer global standard bounds.
                BoundType::Upper
            } else if (b.bestlbtype >= 0 || b.bestubtype >= 0) && !scip.set.is_eq(b.bestlb - simplelb, simpleub - b.bestub) {
                if b.bestlb - simplelb > simpleub - b.bestub {
                    BoundType::Lower
                } else {
                    BoundType::Upper
                }
            } else if b.bestlbtype >= 0 {
                // Prefer variable bounds over local bounds.
                BoundType::Lower
            } else if b.bestubtype >= 0 {
                // Prefer variable bounds over local bounds.
                BoundType::Upper
            } else {
                // No decision yet? just use lower bound.
                BoundType::Lower
            };
        } else {
            let glbub = x.glbdom.ub;
            let glblb = x.glbdom.lb;
            let distlb = (glblb - b.bestlb).abs();
            let distub = (glbub - b.bestub).abs();

            b.selectedbound = if scip.set.is_infinity(-b.bestlb) {
                BoundType::Upper
            } else if !scip.set.is_negative(b.bestlb) {
                if scip.set.is_infinity(b.bestub) || scip.set.is_zero(glblb) || scip.set.is_le(distlb, distub) {
                    BoundType::Lower
                } else {
                    BoundType::Upper
                }
            } else {
                BoundType::Lower
            };
        }
    }

    b
}

/// The variable bound arrays of a variable: its variable lower bounds when
/// `varsign` is +1, its variable upper bounds otherwise.
#[inline]
fn var_vbounds(scip: &Scip, var: VarId, varsign: i32) -> &crate::var::VBounds {
    let v = &scip.vars[var];
    if varsign == 1 { v.vlbs.as_deref().unwrap() } else { v.vubs.as_deref().unwrap() }
}

/// `doMIRBoundSubstitution`: performs bound substitution for a MIR cut;
/// `boundtype` is the bound used for the transformed variable: vlb/vub_idx,
/// or -1 for global lb/ub, or -2 for local lb/ub.
fn do_mir_bound_substitution(scip: &Scip, data: &mut MirData, varsign: i32, boundtype: i32, boundval: f64, probindex: i32, localbdsused: &mut bool) {
    let coef = data.cutcoefs[probindex as usize];

    // Standard (bestlbtype < 0) or variable (bestlbtype >= 0) lower bound?
    if boundtype < 0 {
        let tmp = prod_qd(coef, boundval);
        data.cutrhs = sum_qq(data.cutrhs, -tmp);
        *localbdsused = *localbdsused || boundtype == -2;
    } else {
        let var = get_vars(scip)[probindex as usize];
        let vb = var_vbounds(scip, var, varsign);
        let b = boundtype as usize;

        let zidx = scip.vars[vb.vars[b]].probindex;
        let tmp = prod_qd(coef, vb.constants[b]);
        data.cutrhs = sum_qq(data.cutrhs, -tmp);

        // Check if integral variable already exists in the row.
        let mut zcoef = data.cutcoefs[zidx as usize];

        // If it is new, add it to the indices.
        if zcoef.hi == 0.0 {
            let section = var_section(data, zidx);
            data.secindices[section][data.secnnz[section] as usize] = zidx;
            data.secnnz[section] += 1;
            data.totalnnz += 1;
        }

        let tmp = prod_qd(coef, vb.coefs[b]);
        zcoef = sum_qq(zcoef, tmp);

        zcoef.hi = nonzero(zcoef.hi);
        data.cutcoefs[zidx as usize] = zcoef;
    }
}

/// `performBoundSubstitution`: performs the bound substitution step with
/// the given variable or simple bounds for the variable with the given
/// problem index.
#[allow(clippy::too_many_arguments)]
fn perform_bound_substitution(
    scip: &Scip,
    cutinds: &mut [i32],
    cutcoefs: &mut [Quad],
    cutrhs: &mut Quad,
    nnz: &mut i32,
    varsign: i32,
    boundtype: i32,
    boundval: f64,
    probindex: i32,
    localbdsused: &mut bool,
) {
    let coef = cutcoefs[probindex as usize];

    // Standard (bestlbtype < 0) or variable (bestlbtype >= 0) lower bound?
    if boundtype < 0 {
        let tmp = prod_qd(coef, boundval);
        *cutrhs = sum_qq(*cutrhs, -tmp);
        *localbdsused = *localbdsused || boundtype == -2;
    } else {
        let var = get_vars(scip)[probindex as usize];
        let vb = var_vbounds(scip, var, varsign);
        let b = boundtype as usize;

        let zidx = scip.vars[vb.vars[b]].probindex;
        let tmp = prod_qd(coef, vb.constants[b]);
        *cutrhs = sum_qq(*cutrhs, -tmp);

        // Check if integral variable already exists in the row.
        let mut zcoef = cutcoefs[zidx as usize];

        if zcoef.hi == 0.0 {
            cutinds[*nnz as usize] = zidx;
            *nnz += 1;
        }

        let tmp = prod_qd(coef, vb.coefs[b]);
        zcoef = sum_qq(zcoef, tmp);
        zcoef.hi = nonzero(zcoef.hi);
        cutcoefs[zidx as usize] = zcoef;
    }
}

/// `performBoundSubstitutionSimple`: performs the bound substitution step
/// with the simple bound for the variable with the given problem index.
fn perform_bound_substitution_simple(cutcoefs: &[Quad], cutrhs: &mut Quad, boundtype: i32, boundval: f64, probindex: i32, localbdsused: &mut bool) {
    let coef = cutcoefs[probindex as usize];

    // Must be a standard bound.
    let tmp = prod_qd(coef, boundval);
    *cutrhs = sum_qq(*cutrhs, -tmp);
    *localbdsused = *localbdsused || boundtype == -2;
}

/// The per-nonzero transformation `cutsTransformMIR` records: the sign of
/// the transformed variable in summation, and the bound used for it
/// (vlb/vub_idx, or -1 for global lb/ub, or -2 for local lb/ub).
struct MirTransform<'a> {
    varsign: &'a mut [i32],
    boundtype: &'a mut [i32],
}

/// `cutsTransformMIR`: transform equation a*x = b; lb <= x <= ub into
/// standard form a'*x' = b, 0 <= x' <= ub'.
///
/// Transform variables (lb or ub):
///   x'_j := x_j - lb_j,  x_j = x'_j + lb_j,  a'_j =  a_j,  if lb is used in transformation,
///   x'_j := ub_j - x_j,  x_j = ub_j - x'_j,  a'_j = -a_j,  if ub is used in transformation,
/// and move the constant terms a_j lb_j or a_j ub_j to the rhs.
///
/// Transform variables (vlb or vub):
///   x'_j := x_j - (bl_j zl_j + dl_j),  x_j = x'_j + (bl_j zl_j + dl_j),  a'_j =  a_j,  if vlb is used in transf.
///   x'_j := (bu_j zu_j + du_j) - x_j,  x_j = (bu_j zu_j + du_j) - x'_j,  a'_j = -a_j,  if vub is used in transf.
/// move the constant terms a_j dl_j or a_j du_j to the rhs, and update the
/// coefficient of the VLB variable:
///   a_{zl_j} := a_{zl_j} + a_j bl_j, or
///   a_{zu_j} := a_{zu_j} + a_j bu_j
///
/// Returns `(freevariable, localbdsused)`: whether a free variable was found
/// in the MIR row (an invalid summation), and whether local bounds were used
/// in the transformation.
#[allow(clippy::too_many_arguments)]
fn cuts_transform_mir(
    scip: &Scip,
    data: &mut MirData,
    sol: SolId,
    boundswitch: f64,
    allowlocal: bool,
    fixintegralrhs: bool,
    ignoresol: bool,
    boundsfortrans: Option<&[i32]>,
    boundtypesfortrans: Option<&[crate::types::BoundType]>,
    minfrac: f64,
    maxfrac: f64,
    t: MirTransform,
) -> ScipResult<(bool, bool)> {
    use crate::types::BoundType;
    let MirTransform { varsign, boundtype } = t;
    let mut freevariable = false;
    let mut localbdsused = false;

    let totalnnz = data.totalnnz;
    let allocsize = ((NSECTIONS as i32) * totalnnz).min(data.nvars) as usize;
    let vars = get_vars(scip);

    // Allocate temporary memory to store best bounds and bound types.
    let mut best = vec![BestBounds::default(); allocsize];

    // Transform the cut, one variable section at a time.
    for s in 0..NSECTIONS {
        let cutindsstart = data.ncutinds as usize;
        let usevbds = data.usevbds[s];

        let mut i = 0;
        while i < data.secnnz[s] as usize {
            let v = data.secindices[s][i];

            // Due to variable bound usage, cancellation may have occurred.
            let coef = data.cutcoefs[v as usize];
            if epsz(coef.to_dbl(), QUAD_EPSILON) {
                data.cutcoefs[v as usize] = Quad::new(0.0);
                data.secnnz[s] -= 1;
                data.totalnnz -= 1;
                data.secindices[s][i] = data.secindices[s][data.secnnz[s] as usize];
                // Do not increase the index.
                continue;
            }

            let cutindex = data.ncutinds as usize;
            best[cutindex] = determine_best_bounds(
                scip,
                vars[v as usize],
                sol,
                data,
                boundswitch,
                usevbds,
                allowlocal,
                fixintegralrhs,
                ignoresol,
                boundsfortrans,
                boundtypesfortrans,
            );
            data.cutinds[cutindex] = v;
            data.ncutinds += 1;
            i += 1;

            // If there is a free variable, we terminate because we cannot derive
            // a MIR cut.
            if best[cutindex].freevariable {
                freevariable = true;
                data.ncutinds = 0;

                // If we terminate early, we need to make sure all the zeros in the
                // cut coefficient array are cancelled.
                for j in 0..NSECTIONS {
                    for k in 0..data.secnnz[j] as usize {
                        data.cutinds[data.ncutinds as usize] = data.secindices[j][k];
                        data.ncutinds += 1;
                    }
                }
                return Ok((freevariable, localbdsused));
            }
        }

        // Perform bound substitution for added variables.
        for i in cutindsstart..data.ncutinds as usize {
            let v = data.cutinds[i];

            let bestbnd = if best[i].selectedbound == BoundType::Lower {
                // Use lower bound as transformation bound: x'_j := x_j - lb_j.
                boundtype[i] = best[i].bestlbtype;
                varsign[i] = 1;
                best[i].bestlb
            } else {
                // Use upper bound as transformation bound: x'_j := ub_j - x_j.
                boundtype[i] = best[i].bestubtype;
                varsign[i] = -1;
                best[i].bestub
            };
            do_mir_bound_substitution(scip, data, varsign[i], boundtype[i], bestbnd, v, &mut localbdsused);
        }
    }

    if fixintegralrhs {
        let sumeps = scip.set.sumepsilon();

        // Check if rhs is fractional.
        let f0 = crate::def::eps_frac(data.cutrhs.to_dbl(), sumeps);
        if f0 < minfrac || f0 > maxfrac {
            // Choose complementation of one variable differently such that f0
            // is in correct range.
            let mut besti: i32 = -1;
            let mut bestviolgain = -1e+100;
            let mut bestnewf0 = 1.0;
            for i in 0..data.ncutinds as usize {
                let v = data.cutinds[i];
                let coef = data.cutcoefs[v as usize];
                let b = &best[i];

                if boundtype[i] < 0
                    && ((varsign[i] == 1 && !scip.set.is_infinity(b.bestub) && b.bestubtype < 0) || (varsign[i] == -1 && !scip.set.is_infinity(-b.bestlb) && b.bestlbtype < 0))
                {
                    // Currently:              a'_j =  varsign * a_j  ->  f'_j =  a'_j - floor(a'_j)
                    // after complementation: a''_j = -varsign * a_j  -> f''_j = a''_j - floor(a''_j) = 1 - f'_j
                    //                        rhs'' = rhs' + varsign * a_j * (lb_j - ub_j)
                    // cut violation from f0 and fj:   f'_0 -  f'_j *  x'_j
                    // after complementation:         f''_0 - f''_j * x''_j
                    //
                    // for continuous variables, we just set f'_j = f''_j = |a'_j|
                    let newrhs = data.cutrhs.to_dbl() + varsign[i] as f64 * coef.to_dbl() * (b.bestlb - b.bestub);
                    let newf0 = crate::def::eps_frac(newrhs, sumeps);

                    if newf0 < minfrac || newf0 > maxfrac {
                        continue;
                    }
                    let (fj, newfj);
                    if v >= data.nvars - data.ncontvars {
                        fj = coef.to_dbl().abs();
                        newfj = fj;
                    } else {
                        fj = scip.set.frac(varsign[i] as f64 * coef.to_dbl());
                        newfj = scip.set.frac(-varsign[i] as f64 * coef.to_dbl());
                    }

                    let violgain = if !ignoresol {
                        let solval = get_sol_val(scip, sol, vars[v as usize]);
                        let viol = f0 - fj * if varsign[i] == 1 { solval - b.bestlb } else { b.bestub - solval };
                        let newviol = newf0 - newfj * if varsign[i] == -1 { solval - b.bestlb } else { b.bestub - solval };
                        newviol - viol
                    } else {
                        // todo: this should be done, this can improve the dualray
                        // significantly.
                        log::error!("Cannot handle closest bounds with ignoring the LP solution.");
                        return Err(crate::retcode::Retcode::InvalidCall);
                    };

                    // Prefer larger violations; for equal violations, prefer
                    // smaller f0 values since then the possibility that we f_j >
                    // f_0 is larger and we may improve some coefficients in
                    // rounding.
                    if scip.set.is_gt(violgain, bestviolgain) || (scip.set.is_ge(violgain, bestviolgain) && newf0 < bestnewf0) {
                        besti = i as i32;
                        bestviolgain = violgain;
                        bestnewf0 = newf0;
                    }
                }
            }

            if besti >= 0 {
                let bi = besti as usize;
                let mut coef = data.cutcoefs[data.cutinds[bi] as usize];
                coef.scale(varsign[bi] as f64);

                // Switch the complementation of this variable.
                let mut tmp = sum_dd(best[bi].bestlb, -best[bi].bestub);
                tmp = crate::quad::prod_qq(tmp, coef);
                data.cutrhs = sum_qq(data.cutrhs, tmp);

                if varsign[bi] == 1 {
                    // Switch to upper bound.
                    boundtype[bi] = best[bi].bestubtype;
                    varsign[bi] = -1;
                } else {
                    // Switch to lower bound.
                    boundtype[bi] = best[bi].bestlbtype;
                    varsign[bi] = 1;
                }
                localbdsused = localbdsused || boundtype[bi] == -2;
            }
        }
    }

    Ok((freevariable, localbdsused))
}

/// `cutsRoundMIR`: calculate fractionalities f_0 := b - down(b),
/// f_j := a'_j - down(a'_j), and derive MIR cut a~ * x' <= down(b)
///   integers:   a~_j = down(a'_j),                       if f_j <= f_0
///               a~_j = down(a'_j) + (f_j - f_0)/(1 - f_0), if f_j >  f_0
///   continuous: a~_j = 0,                                if a'_j >= 0
///               a~_j = a'_j/(1 - f_0),                   if a'_j <  0
///
/// Transform inequality back to a^ * x <= rhs:
///
/// (lb or ub):
///   x'_j := x_j - lb_j,  x_j = x'_j + lb_j,  a'_j =  a_j,  a^_j :=  a~_j,  if lb was used in transformation,
///   x'_j := ub_j - x_j,  x_j = ub_j - x'_j,  a'_j = -a_j,  a^_j := -a~_j,  if ub was used in transformation,
/// and move the constant terms -a~_j lb_j = -a^_j lb_j, or a~_j ub_j =
/// -a^_j ub_j to the rhs.
///
/// (vlb or vub):
///   x'_j := x_j - (bl_j zl_j + dl_j),  x_j = x'_j + (bl_j zl_j + dl_j),  a'_j =  a_j,  a^_j :=  a~_j,  (vlb)
///   x'_j := (bu_j zu_j + du_j) - x_j,  x_j = (bu_j zu_j + du_j) - x'_j,  a'_j = -a_j,  a^_j := -a~_j,  (vub)
/// move the constant terms -a~_j dl_j = -a^_j dl_j, or a~_j du_j = -a^_j du_j
/// to the rhs, and update the VB variable coefficients:
///   a^_{zl_j} := a^_{zl_j} - a~_j bl_j = a^_{zl_j} - a^_j bl_j, or
///   a^_{zu_j} := a^_{zu_j} + a~_j bu_j = a^_{zu_j} - a^_j bu_j
fn cuts_round_mir(scip: &Scip, data: &mut MirData, varsign: &[i32], boundtype: &[i32], f0: Quad) {
    cuts_round(scip, data, varsign, boundtype, f0, None);
}

/// `cutsRoundStrongCG`: calculate fractionalities f_0 := b - down(b),
/// f_j := a'_j - down(a'_j), and derive strong CG cut
/// a~ * x' <= (k+1) * down(b)
///   integers:   a~_j = down(a'_j)                                      , if f_j <= f_0
///               a~_j = down(a'_j) + p_j/(k + 1)                        , if f_j >  f_0
///               where p_j := ceil( k * (f_j - f_0)/(1 - f_0) ) - 1 and k := 1/f_0 - 1
///   continuous: a~_j = 0
/// and transform the inequality back like cutsRoundMIR(); `k` is the factor
/// to strengthen the strongcg cut.
fn cuts_round_strong_cg(scip: &Scip, data: &mut MirData, varsign: &[i32], boundtype: &[i32], f0: Quad, k: f64) {
    cuts_round(scip, data, varsign, boundtype, f0, Some(k));
}

/// The body `cutsRoundMIR` and `cutsRoundStrongCG` share; they differ only
/// in the coefficient each variable gets.
fn cuts_round(scip: &Scip, data: &mut MirData, varsign: &[i32], boundtype: &[i32], f0: Quad, strongcg: Option<f64>) {
    use crate::quad::{div_dd, div_dq, eps_floor_q, prod_qq};

    let mut onedivoneminusf0 = sum_qd(-f0, 1.0);
    onedivoneminusf0 = div_dq(1.0, onedivoneminusf0);

    let vars = get_vars(scip);

    // Loop backwards through the sections, so that the reversing of varbound
    // substitutions does not prematurely effect the coefficients of variables
    // in other sections, because the section index of a variable bound must
    // always be higher than that of the bounded variable.
    let mut cutindex = data.ncutinds - 1;
    for s in (0..NSECTIONS).rev() {
        let nnz = data.secnnz[s];
        let enfintegral = data.isenfint[s];
        let implintegral = data.isimplint[s];

        // Iterate backwards over indices in section, so we can easily shrink
        // the section if we find zeros.
        let mut i = nnz - 1;
        while i >= 0 {
            let v = data.secindices[s][i as usize];
            let sign = varsign[cutindex as usize];
            let ty = boundtype[cutindex as usize];
            cutindex -= 1;
            let var = vars[v as usize];

            let mut aj = data.cutcoefs[v as usize];
            let mut cutaj;

            if enfintegral || implintegral {
                // Variable is integral.
                aj.scale(sign as f64);

                let downaj = eps_floor_q(aj, scip.set.epsilon());
                let fj = sum_qq(aj, -downaj);

                if scip.set.is_le(fj.to_dbl(), f0.to_dbl()) {
                    // a^_j
                    cutaj = downaj;
                } else if let Some(k) = strongcg {
                    cutaj = sum_qq(fj, -f0);
                    cutaj = prod_qd(cutaj, k);
                    cutaj = prod_qq(cutaj, onedivoneminusf0);
                    let pj = scip.set.ceil(cutaj.to_dbl());
                    cutaj = div_dd(pj, k + 1.0);
                    cutaj = sum_qq(cutaj, downaj);
                } else {
                    let mut tmp = sum_qq(fj, -f0);
                    tmp = prod_qq(tmp, onedivoneminusf0);
                    cutaj = sum_qq(tmp, downaj);
                }

                cutaj.scale(sign as f64);
            } else if strongcg.is_some() {
                // Variable is continuous; must always be positive in strongcg
                // cut. It will be automatically deleted.
                cutaj = Quad::new(0.0);
            } else {
                // Variable is continuous.
                if aj.to_dbl() * sign as f64 >= 0.0 {
                    cutaj = Quad::new(0.0);
                } else {
                    // cutaj = aj * onedivoneminusf0
                    cutaj = prod_qq(onedivoneminusf0, aj);
                }
            }

            // Remove coefficient from cut if it becomes zero.
            if epsz(cutaj.to_dbl(), QUAD_EPSILON) {
                data.cutcoefs[v as usize] = Quad::new(0.0);
                data.totalnnz -= 1;
                data.secnnz[s] -= 1;
                data.secindices[s][i as usize] = data.secindices[s][data.secnnz[s] as usize];
                i -= 1;
                continue;
            }

            // Store the updated coefficient.
            data.cutcoefs[v as usize] = cutaj;

            // Undo bound transformations.
            if ty < 0 {
                // Standard bound: move the constant term  -a~_j * lb_j == -a^_j *
                // lb_j , or  a~_j * ub_j == -a^_j * ub_j  to the rhs.
                let x = &scip.vars[var];
                let bound = if sign == 1 {
                    // Lower bound was used.
                    if ty == -1 { x.glbdom.lb } else { x.locdom.lb }
                } else {
                    // Upper bound was used.
                    if ty == -1 { x.glbdom.ub } else { x.locdom.ub }
                };
                let tmp = prod_qd(cutaj, bound);
                data.cutrhs = sum_qq(data.cutrhs, tmp);
            } else {
                // Variable bound.
                let vbidx = ty as usize;

                // Change mirrhs and cutaj of integer variable z_j of variable
                // bound.
                let vb = var_vbounds(scip, var, sign);

                let zidx = scip.vars[vb.vars[vbidx]].probindex;

                let tmp = prod_qd(cutaj, vb.constants[vbidx]);
                data.cutrhs = sum_qq(data.cutrhs, tmp);

                let tmp = prod_qd(cutaj, vb.coefs[vbidx]);

                let mut zcoef = data.cutcoefs[zidx as usize];

                // Update sparsity pattern.
                if zcoef.hi == 0.0 {
                    let zsection = var_section(data, zidx);
                    data.secindices[zsection][data.secnnz[zsection] as usize] = zidx;
                    data.secnnz[zsection] += 1;
                    data.totalnnz += 1;
                }

                zcoef = sum_qq(zcoef, -tmp);
                zcoef.hi = nonzero(zcoef.hi);
                data.cutcoefs[zidx as usize] = zcoef;
            }
            i -= 1;
        }
    }

    // Finally, store the relevant data in cutinds which is the array used by
    // the other functions.
    data.ncutinds = 0;
    for s in 0..NSECTIONS {
        for i in 0..data.secnnz[s] as usize {
            data.cutinds[data.ncutinds as usize] = data.secindices[s][i];
            data.ncutinds += 1;
        }
    }
}

/// `cutsSubstituteMIR`: substitute aggregated slack variables:
///
/// The coefficient of the slack variable s_r is equal to the row's weight
/// times the slack's sign, because the slack variable only appears in its
/// own row: a'_r = scale * weight[r] * slacksign[r].
///
/// Depending on the slacks type (integral or continuous), its coefficient
/// in the cut calculates as follows:
///   integers:   a^_r = a~_r = down(a'_r),                       if f_r <= f_0
///               a^_r = a~_r = down(a'_r) + (f_r - f_0)/(1 - f_0), if f_r >  f_0
///   continuous: a^_r = a~_r = 0,                                if a'_r >= 0
///               a^_r = a~_r = a'_r/(1 - f_0),                   if a'_r <  0
///
/// Substitute a^_r * s_r by adding a^_r times the slack's definition to the
/// cut.
#[allow(clippy::too_many_arguments)]
fn cuts_substitute_mir(
    scip: &Scip,
    weights: &[f64],
    slacksign: &[i32],
    rowinds: &[i32],
    nrowinds: i32,
    scale: f64,
    cutcoefs: &mut [Quad],
    cutrhs: &mut Quad,
    cutinds: &mut [i32],
    nnz: &mut i32,
    f0: Quad,
) {
    use crate::quad::{div_dq, eps_ceil_q, eps_floor_q, prod_qq};

    let mut onedivoneminusf0 = sum_qd(-f0, 1.0);
    onedivoneminusf0 = div_dq(1.0, onedivoneminusf0);

    let rows = &scip.lp.as_ref().unwrap().rows;

    for i in 0..nrowinds as usize {
        let r = rowinds[i] as usize;
        let row = rows[r];
        let rw = &scip.rows[row];

        // Get the slack's coefficient a'_r in the aggregated row.
        let ar = prod_dd(slacksign[i] as f64 * scale, weights[i]);

        // Calculate slack variable's coefficient a^_r in the cut.
        let mut cutar;
        if rw.integral {
            // Slack variable is always integral:
            //   a^_r = a~_r = down(a'_r)                      , if f_r <= f0
            //   a^_r = a~_r = down(a'_r) + (f_r - f0)/(1 - f0), if f_r >  f0
            let downar = eps_floor_q(ar, scip.set.epsilon());
            let fr = sum_qq(ar, -downar);
            if scip.set.is_le(fr.to_dbl(), f0.to_dbl()) {
                // a^_r
                cutar = downar;
            } else {
                cutar = sum_qq(fr, -f0);
                cutar = prod_qq(cutar, onedivoneminusf0);
                cutar = sum_qq(cutar, downar);
            }
        } else {
            // Slack variable is continuous:
            //   a^_r = a~_r = 0                               , if a'_r >= 0
            //   a^_r = a~_r = a'_r/(1 - f0)                   , if a'_r <  0
            if ar.to_dbl() >= 0.0 {
                // Slack can be ignored, because its coefficient is reduced to 0.0.
                continue;
            } else {
                cutar = prod_qq(onedivoneminusf0, ar);
            }
        }

        // If the coefficient was reduced to zero, ignore the slack variable.
        if epsz(cutar.to_dbl(), QUAD_EPSILON) {
            continue;
        }

        // Depending on the slack's sign, we have
        //   a*x + c + s == rhs  =>  s == - a*x - c + rhs,  or  a*x + c - s == lhs  =>  s == a*x + c - lhs
        // substitute a^_r * s_r by adding a^_r times the slack's definition to
        // the cut.
        let myprod = prod_qd(cutar, -slacksign[i] as f64);

        // Add the slack's definition multiplied with a^_j to the cut.
        var_vec_add_scaled_row_coefs_quad_scale(scip, cutinds, cutcoefs, nnz, row, myprod);

        // Move slack's constant to the right hand side.
        if slacksign[i] == 1 {
            // a*x + c + s == rhs  =>  s == - a*x - c + rhs: move a^_r * (rhs - c)
            // to the right hand side.
            let mut rowrhs = Quad::new(rw.rhs - rw.constant);
            if rw.integral {
                // The right hand side was implicitly rounded down in row
                // aggregation.
                rowrhs = eps_floor_q(rowrhs, scip.set.epsilon());
            }
            let tmp = prod_qq(myprod, rowrhs);
            *cutrhs = sum_qq(*cutrhs, tmp);
        } else {
            // a*x + c - s == lhs  =>  s == a*x + c - lhs: move a^_r * (c - lhs) to
            // the right hand side.
            let mut rowlhs = Quad::new(rw.lhs - rw.constant);
            if rw.integral {
                // The left hand side was implicitly rounded up in row aggregation.
                rowlhs = eps_ceil_q(rowlhs, scip.set.epsilon());
            }
            let tmp = prod_qq(myprod, rowlhs);
            *cutrhs = sum_qq(*cutrhs, tmp);
        }
    }

    // Relax rhs to zero, if it's very close to 0.
    if cutrhs.to_dbl() < 0.0 && cutrhs.to_dbl() >= -scip.set.epsilon() {
        *cutrhs = Quad::new(0.0);
    }
}

/// The cut a cut generator stores when it improves `cutefficacy`.
pub struct CutOut<'a> {
    /// The non-zero coefficients in the cut.
    pub cutcoefs: &'a mut [f64],
    /// The right hand side of the cut.
    pub cutrhs: &'a mut f64,
    /// The indices of non-zero coefficients in the cut.
    pub cutinds: &'a mut [i32],
    /// The number of non-zeros in the cut.
    pub cutnnz: &'a mut i32,
    /// Efficacy of the cut, or `None`.
    pub cutefficacy: Option<&'a mut f64>,
    /// Rank of the generated cut, or `None`.
    pub cutrank: Option<&'a mut i32>,
    /// Whether the generated cut is only valid locally.
    pub cutislocal: &'a mut bool,
}

/// Sets up the MIR data for an aggregation row scaled by `scale`.
fn mir_data_create(scip: &Scip, aggrrow: &AggrRow, scale: f64, vartypeusevbds: i32) -> MirData {
    let nnz = aggrrow.nnz;
    let nvars = crate::api::get_n_vars(scip);
    let mut data = MirData {
        totalnnz: nnz,
        ..MirData::default()
    };

    // Initialize sections.
    for l in 0..NSECTIONS {
        data.secindices[l] = vec![0; nvars as usize];
        data.secnnz[l] = 0;
        // Cont. | cont impl. | int impl. | bin impl. | int | bin
        data.isenfint[l] = l >= 2;
        data.isimplint[l] = (1..=3).contains(&l);
        // Use variable bounds for the sections specified by the user.
        data.usevbds[l] = if (l as i32) < vartypeusevbds { 2 } else { 0 };
    }

    // Problem data needs to be initialized before cut data as it is used to
    // partition the variables into the sections.
    data.nvars = nvars;
    data.nbinvars = crate::api::get_n_bin_vars(scip);
    data.nintvars = crate::api::get_n_int_vars(scip);
    data.nbinimplvars = crate::api::get_n_bin_impl_vars(scip);
    data.nintimplvars = crate::api::get_n_int_impl_vars(scip);
    data.ncontimplvars = crate::api::get_n_cont_impl_vars(scip);
    data.ncontvars = crate::api::get_n_cont_vars(scip);

    data.cutcoefs = vec![Quad::new(0.0); nvars as usize];
    data.cutinds = vec![0; nvars as usize];

    data.cutrhs = prod_qd(aggrrow.rhs, scale);

    if nnz > 0 {
        // Initalize cut with the aggregation.
        data.cutinds[..nnz as usize].copy_from_slice(&aggrrow.inds[..nnz as usize]);

        for l in 0..nnz as usize {
            let m = aggrrow.inds[l] as usize;
            data.cutcoefs[m] = prod_qd(aggrrow.vals[m], scale);
        }

        // Sort the array by problem index and add the variables to their
        // sections.
        crate::sort::sort_down(&mut data.cutinds[..nnz as usize], (), crate::sort::cmp_int);
        for l in 0..nnz as usize {
            let section = var_section(&data, data.cutinds[l]);
            data.secindices[section][data.secnnz[section] as usize] = data.cutinds[l];
            data.secnnz[section] += 1;
        }
    }

    data.ncutinds = 0;
    data
}

/// `SCIPcalcMIR`: calculates an MIR cut out of the weighted sum of LP rows
/// given by an aggregation row; the weights of modifiable rows are set to
/// 0.0, because these rows cannot participate in an MIR cut. For all
/// variable types with index smaller than `vartypeusevbds`, variable type
/// substitution is allowed (0: continuous, 1: continuous implint., 2:
/// integer implint, 3: binary implint, 4: integer, 5: binary). Returns
/// whether the returned coefficients are a valid MIR cut and it improves
/// cutefficacy.
#[allow(clippy::too_many_arguments)]
pub fn calc_mir(
    scip: &Scip,
    sol: SolId,
    postprocess: bool,
    boundswitch: f64,
    vartypeusevbds: i32,
    allowlocal: bool,
    fixintegralrhs: bool,
    boundsfortrans: Option<&[i32]>,
    boundtypesfortrans: Option<&[crate::types::BoundType]>,
    minfrac: f64,
    maxfrac: f64,
    scale: f64,
    aggrrow: &AggrRow,
    out: CutOut,
) -> ScipResult<bool> {
    let mut success = false;

    // Setup data to track cut and initialize the cut with aggregation.
    let mut data = mir_data_create(scip, aggrrow, scale, vartypeusevbds);

    let mut tmpislocal = aggrrow.local;

    // Allocate temporary memory.
    let mut varsign = vec![0; data.nvars as usize];
    let mut boundtype = vec![0; data.nvars as usize];

    // Transform equation  a*x == b, lb <= x <= ub  into standard form
    //   a'*x' == b, 0 <= x' <= ub'
    // (see cutsTransformMIR).
    'terminate: {
        if data.totalnnz > 0 {
            let t = MirTransform {
                varsign: &mut varsign,
                boundtype: &mut boundtype,
            };
            let (freevariable, localbdsused) = cuts_transform_mir(
                scip,
                &mut data,
                sol,
                boundswitch,
                allowlocal,
                fixintegralrhs,
                false,
                boundsfortrans,
                boundtypesfortrans,
                minfrac,
                maxfrac,
                t,
            )?;
            tmpislocal = tmpislocal || localbdsused;

            if freevariable {
                break 'terminate;
            }
        }

        // Calculate fractionalities  f_0 := b - down(b), f_j := a'_j -
        // down(a'_j) , and derive MIR cut  a~*x' <= down(b), then transform the
        // inequality back to a^*x <= rhs (see cutsRoundMIR).
        let downrhs = crate::quad::eps_floor_q(data.cutrhs, scip.set.epsilon());
        let mut f0 = sum_qq(data.cutrhs, -downrhs);

        if f0.to_dbl() < minfrac || f0.to_dbl() > maxfrac {
            break 'terminate;
        }

        // We multiply the coefficients of the base inequality roughly by
        // scale/(1-f0). If this gives a scalar that is very big, we better do
        // not generate this cut.
        if scale.abs() / (1.0 - f0.to_dbl()) > MAXCMIRSCALE {
            break 'terminate;
        }

        // Renormalize f0 value.
        f0 = sum_dd(f0.hi, f0.lo);

        data.cutrhs = downrhs;

        if data.totalnnz > 0 {
            cuts_round_mir(scip, &mut data, &varsign, &boundtype, f0);
        }

        // Substitute aggregated slack variables (see cutsSubstituteMIR).
        {
            let MirData {
                cutcoefs,
                cutrhs,
                cutinds,
                ncutinds,
                ..
            } = &mut data;
            cuts_substitute_mir(
                scip,
                &aggrrow.rowweights,
                &aggrrow.slacksign,
                &aggrrow.rowsinds,
                aggrrow.nrows,
                scale,
                cutcoefs,
                cutrhs,
                cutinds,
                ncutinds,
                f0,
            );
        }

        {
            let MirData {
                cutcoefs,
                cutrhs,
                cutinds,
                ncutinds,
                ..
            } = &mut data;
            if postprocess {
                // Remove all nearly-zero coefficients from MIR row and relax the
                // right hand side correspondingly in order to prevent numerical
                // rounding errors.
                success = postprocess_cut_quad(scip, tmpislocal, cutinds, cutcoefs, ncutinds, cutrhs);
            } else {
                success = !remove_zeros_quad(scip, scip.set.sumepsilon(), tmpislocal, cutcoefs, cutrhs, cutinds, ncutinds);
            }
        }

        if success {
            let mirefficacy = calc_efficacy_dense_storage_quad(scip, sol, &data.cutcoefs, data.cutrhs.to_dbl(), &data.cutinds, data.ncutinds);

            if crate::api::is_efficacious(scip, mirefficacy) && out.cutefficacy.as_ref().is_none_or(|e| mirefficacy > **e) {
                let n = data.ncutinds as usize;
                out.cutinds[..n].copy_from_slice(&data.cutinds[..n]);
                *out.cutnnz = data.ncutinds;
                *out.cutrhs = data.cutrhs.to_dbl();
                *out.cutislocal = tmpislocal;

                // Clean tmpcoefs and go back to double precision.
                for i in 0..n {
                    let j = out.cutinds[i] as usize;
                    out.cutcoefs[i] = data.cutcoefs[j].to_dbl();
                }

                if let Some(e) = out.cutefficacy {
                    *e = mirefficacy;
                }

                if let Some(r) = out.cutrank {
                    *r = aggrrow.rank + 1;
                }
            } else {
                success = false;
            }
        }
    }

    Ok(success)
}

/// `computeMIREfficacy`: compute the efficacy of the MIR cut for the given
/// values without computing the cut. This is used for the CMIR cut
/// generation heuristic.
#[allow(clippy::too_many_arguments)]
fn compute_mir_efficacy(scip: &Scip, coefs: &[f64], solvals: &[f64], mut rhs: f64, contactivity: f64, contsqrnorm: f64, delta: f64, nvars: i32, minfrac: f64, maxfrac: f64) -> f64 {
    let scale = 1.0 / delta;
    rhs *= scale;
    let downrhs = scip.set.floor(rhs);
    let f0 = rhs - downrhs;

    if f0 < minfrac || f0 > maxfrac {
        return 0.0;
    }

    let onedivoneminusf0 = 1.0 / (1.0 - f0);

    let contscale = scale * onedivoneminusf0;

    // We multiply the coefficients of the base inequality roughly by
    // scale/(1-f0). If this gives a scalar that is very big, we better do not
    // generate this cut.
    if contscale > MAXCMIRSCALE {
        return 0.0;
    }

    rhs = downrhs;
    rhs -= contscale * contactivity;
    let mut norm = contscale * contscale * contsqrnorm;

    let f0pluseps = f0 + scip.set.epsilon();

    for i in 0..nvars as usize {
        let mut floorai = scip.set.floor(scale * coefs[i]);
        let fi = (scale * coefs[i]) - floorai;

        if fi > f0pluseps {
            floorai += (fi - f0) * onedivoneminusf0;
        }

        rhs -= solvals[i] * floorai;
        norm += floorai * floorai;
    }

    norm = f64::sqrt(norm);

    -rhs / crate::def::max(norm, 1e-6)
}

/// `SCIPcutGenerationHeuristicCMIR`: calculates an MIR cut out of an
/// aggregation of LP rows.
///
/// Given the aggregation, it is transformed to a mixed knapsack set via
/// complementation (using bounds or variable bounds). Then, different
/// scalings of the mkset are used to generate a MIR and the best is chosen.
/// One of the steps of the MIR is to round the coefficients of the integer
/// variables down, so one would prefer to have integer coefficients for
/// integer variables which are far away from their bounds in the mkset.
///
/// Only cuts that are strictly better than `out.cutefficacy` (which must be
/// given) on input are returned; returns whether a valid and efficacious cut
/// was returned.
#[allow(clippy::too_many_arguments)]
pub fn cut_generation_heuristic_cmir(
    scip: &mut Scip,
    sol: SolId,
    postprocess: bool,
    boundswitch: f64,
    vartypeusevbds: i32,
    allowlocal: bool,
    mut maxtestdelta: i32,
    boundsfortrans: Option<&[i32]>,
    boundtypesfortrans: Option<&[crate::types::BoundType]>,
    minfrac: f64,
    maxfrac: f64,
    aggrrow: &AggrRow,
    out: CutOut,
) -> ScipResult<bool> {
    let mut success = false;

    let nvars = crate::api::get_n_vars(scip) as usize;
    let firstcontvar = nvars as i32 - crate::api::get_n_cont_vars(scip) - crate::api::get_n_cont_impl_vars(scip);

    // Allocate temporary memory.
    let mut varsign = vec![0; nvars];
    let mut boundtype = vec![0; nvars];
    let mut tmpcoefs = vec![0.0; nvars + aggrrow.nrows as usize];
    let mut tmpvalues = vec![0.0; nvars + aggrrow.nrows as usize];
    // The +4 comes from a few rules that create extra delta candidates, see
    // usages of ndeltacands.
    let mut deltacands = vec![0.0; nvars + 4];
    // We only compute bound distance for integer variables; by variable bound
    // substitution, the number of integer variables can grow significantly.
    // Hence, these allocations are length nvars.
    let mut bounddist = vec![0.0; nvars];
    let mut bounddistpos = vec![0; nvars];

    // Initialize mkset with the unscaled aggregation.
    let mut data = mir_data_create(scip, aggrrow, 1.0, vartypeusevbds);

    *out.cutislocal = aggrrow.local;

    // Transform equation  a*x == b, lb <= x <= ub  into standard form
    //   a'*x' == b, 0 <= x' <= ub'
    // (see cutsTransformMIR).
    let (freevariable, mut localbdsused) = {
        let t = MirTransform {
            varsign: &mut varsign,
            boundtype: &mut boundtype,
        };
        cuts_transform_mir(
            scip,
            &mut data,
            sol,
            boundswitch,
            allowlocal,
            false,
            false,
            boundsfortrans,
            boundtypesfortrans,
            minfrac,
            maxfrac,
            t,
        )?
    };

    // mksetinds, mksetcoefs and mksetnnz are data.cutinds, data.cutcoefs and
    // data.ncutinds; mksetrhs needs to synchronize its values with
    // data.cutrhs again before calling cutsRoundMIR().
    let mksetnnz = data.ncutinds;
    let mut mksetrhs = data.cutrhs;

    'terminate: {
        if freevariable {
            break 'terminate;
        }

        // Found positions of integral variables that are strictly between their
        // bounds.
        let mut maxabsmksetcoef: f64 = -1.0;
        let mut nbounddist = 0;

        let mut i = mksetnnz - 1;
        while i >= 0 && data.cutinds[i as usize] < firstcontvar {
            let var = get_vars(scip)[data.cutinds[i as usize] as usize];
            let primsol = get_sol_val(scip, sol, var);
            let lb = scip.vars[var].locdom.lb;
            let ub = scip.vars[var].locdom.ub;
            let coef = data.cutcoefs[data.cutinds[i as usize] as usize];

            if scip.set.is_eq(primsol, lb) || scip.set.is_eq(primsol, ub) {
                i -= 1;
                continue;
            }

            bounddist[nbounddist] = crate::def::min(ub - primsol, primsol - lb);
            bounddistpos[nbounddist] = i;
            deltacands[nbounddist] = coef.to_dbl();
            nbounddist += 1;
            i -= 1;
        }

        // No fractional variable; so abort here.
        if nbounddist == 0 {
            break 'terminate;
        }

        let intstart = (i + 1) as usize;
        let mut ndeltacands = nbounddist;

        crate::sort::sort_down(
            &mut bounddist[..nbounddist],
            (&mut deltacands[..nbounddist], &mut bounddistpos[..nbounddist]),
            crate::sort::cmp_real,
        );

        if let Some(mut intscale) = crate::misc::calc_integral_scalar(&deltacands[..nbounddist], -scip.set.epsilon(), scip.set.sumepsilon(), 10000, 10000.0) {
            let mut intscalerhs = data.cutrhs.to_dbl() * intscale;
            let mut delta = 1.0 / intscale;
            let mut intf0 = intscalerhs - intscalerhs.floor();

            if !scip.set.is_feas_integral(intf0) {
                if intf0 < minfrac || intf0 > maxfrac {
                    intscale *= scip.set.ceil(crate::def::max(minfrac, 1.0 - maxfrac) / crate::def::min(intf0, 1.0 - intf0));
                    intscalerhs = data.cutrhs.to_dbl() * intscale;
                    delta = 1.0 / intscale;
                    intf0 = intscalerhs - intscalerhs.floor();
                }

                if intf0 >= minfrac && intf0 <= maxfrac {
                    if !scip.set.is_eq(delta, 1.0) {
                        deltacands[ndeltacands] = delta;
                        ndeltacands += 1;
                    }

                    if intf0 < maxfrac {
                        let delta2 = 1.0 / (intscale * scip.set.floor(maxfrac / intf0));

                        if !scip.set.is_eq(delta, delta2) && !scip.set.is_eq(delta2, 1.0) {
                            deltacands[ndeltacands] = delta2;
                            ndeltacands += 1;
                        }
                    }
                }
            }
        }

        for deltacand in &mut deltacands[..nbounddist] {
            let absmksetcoef = deltacand.abs();
            maxabsmksetcoef = crate::def::max(absmksetcoef, maxabsmksetcoef);

            *deltacand = absmksetcoef;
        }

        // Also test 1.0 and maxabsmksetcoef + 1.0 as last delta values.
        if maxabsmksetcoef != -1.0 {
            deltacands[ndeltacands] = maxabsmksetcoef + 1.0;
            ndeltacands += 1;
        }

        deltacands[ndeltacands] = 1.0;
        ndeltacands += 1;

        maxtestdelta = (ndeltacands as i32).min(maxtestdelta);

        // For each delta calculate fractionalities  f_0 := b - down(b), f_j :=
        // a'_j - down(a'_j), and derive the MIR cut (see cutsRoundMIR).
        let mut ntmpcoefs = 0;
        for i in intstart..mksetnnz as usize {
            let var = get_vars(scip)[data.cutinds[i] as usize];

            // Get the soltion value of the continuous variable.
            let mut solval = get_sol_val(scip, sol, var);

            // Now compute the solution value in the transform space considering
            // complementation.
            let x = &scip.vars[var];
            if boundtype[i] == -1 {
                // Variable was complemented with global (simple) bound.
                if varsign[i] == -1 {
                    solval = x.glbdom.ub - solval;
                } else {
                    solval -= x.glbdom.lb;
                }
            } else {
                // Variable was complemented with local (simple) bound.
                if varsign[i] == -1 {
                    solval = x.locdom.ub - solval;
                } else {
                    solval -= x.locdom.lb;
                }
            }

            tmpvalues[ntmpcoefs] = solval;
            let coef = data.cutcoefs[data.cutinds[i] as usize];
            tmpcoefs[ntmpcoefs] = varsign[i] as f64 * coef.to_dbl();
            ntmpcoefs += 1;
        }

        let mut contactivity = 0.0;
        let mut contsqrnorm = 0.0;
        for i in 0..intstart {
            let mksetcoef = data.cutcoefs[data.cutinds[i] as usize];

            if varsign[i] as f64 * mksetcoef.to_dbl() >= 0.0 {
                continue;
            }

            // Get the soltion value of the continuous variable.
            let var = get_vars(scip)[data.cutinds[i] as usize];
            let mut solval = get_sol_val(scip, sol, var);

            // Now compute the solution value in the transform space considering
            // complementation.
            let x = &scip.vars[var];
            match boundtype[i] {
                -1 => {
                    // Variable was complemented with global (simple) bound.
                    if varsign[i] == -1 {
                        solval = x.glbdom.ub - solval;
                    } else {
                        solval -= x.glbdom.lb;
                    }
                }
                -2 => {
                    // Variable was complemented with local (simple) bound.
                    if varsign[i] == -1 {
                        solval = x.locdom.ub - solval;
                    } else {
                        solval -= x.locdom.lb;
                    }
                }
                b => {
                    // Variable was complemented with a variable bound.
                    let vb = var_vbounds(scip, var, varsign[i]);
                    let coef = vb.coefs[b];
                    let constant = vb.constants[b];
                    let vbdsolval = get_sol_val(scip, sol, vb.vars[b]);
                    if varsign[i] == -1 {
                        solval = (coef * vbdsolval + constant) - solval;
                    } else {
                        solval -= coef * vbdsolval + constant;
                    }
                }
            }

            contactivity += solval * (mksetcoef.to_dbl() * varsign[i] as f64);
            contsqrnorm += mksetcoef.to_dbl() * mksetcoef.to_dbl();
        }

        {
            for i in 0..aggrrow.nrows as usize {
                let row = scip.lp.as_ref().unwrap().rows[aggrrow.rowsinds[i]];
                let integral = scip.rows[row].integral;

                if (aggrrow.rowweights[i] * aggrrow.slacksign[i] as f64) >= 0.0 && !integral {
                    continue;
                }

                // Compute solution value of slack variable.
                let mut slackval = crate::api::get_row_sol_activity(scip, row, sol);

                if aggrrow.slacksign[i] == 1 {
                    // Right hand side.
                    slackval = scip.rows[row].rhs - slackval;
                } else {
                    // Left hand side.
                    slackval -= scip.rows[row].lhs;
                }

                if integral {
                    // If row is integral add variable to tmp arrays.
                    tmpvalues[ntmpcoefs] = slackval;
                    tmpcoefs[ntmpcoefs] = aggrrow.rowweights[i] * aggrrow.slacksign[i] as f64;
                    ntmpcoefs += 1;
                } else {
                    let slackcoeff = aggrrow.rowweights[i] * aggrrow.slacksign[i] as f64;

                    // Otherwise add it to continuous activity.
                    contactivity += slackval * slackcoeff;
                    contsqrnorm += slackcoeff * slackcoeff;
                }
            }
        }

        // Try all candidates for delta and remember best.
        let mut bestdelta = crate::def::INVALID;
        let mut bestefficacy = -scip.set.infinity();

        let mut i = 0;
        while i < maxtestdelta as usize {
            // Check if we have seen this value of delta before.
            let deltaseenbefore = (0..i).any(|j| scip.set.is_eq(deltacands[i], deltacands[j]));

            // Skip this delta value and allow one more delta value if available.
            if deltaseenbefore {
                maxtestdelta = (maxtestdelta + 1).min(ndeltacands as i32);
                i += 1;
                continue;
            }

            let efficacy = compute_mir_efficacy(
                scip,
                &tmpcoefs,
                &tmpvalues,
                mksetrhs.to_dbl(),
                contactivity,
                contsqrnorm,
                deltacands[i],
                ntmpcoefs as i32,
                minfrac,
                maxfrac,
            );

            if efficacy > bestefficacy {
                bestefficacy = efficacy;
                bestdelta = deltacands[i];
            }
            i += 1;
        }

        // No delta was found that yielded any cut.
        if bestdelta == crate::def::INVALID {
            break 'terminate;
        }

        // Try bestdelta divided by 2, 4 and 8.
        {
            let basedelta = bestdelta;
            let mut i = 2;
            while i <= 8 {
                let delta = basedelta / i as f64;
                let efficacy = compute_mir_efficacy(
                    scip,
                    &tmpcoefs,
                    &tmpvalues,
                    mksetrhs.to_dbl(),
                    contactivity,
                    contsqrnorm,
                    delta,
                    ntmpcoefs as i32,
                    minfrac,
                    maxfrac,
                );

                if efficacy > bestefficacy {
                    bestefficacy = efficacy;
                    bestdelta = delta;
                }
                i *= 2;
            }
        }

        // Try to improve efficacy by switching complementation of integral
        // variables that are not at their bounds in order of non-increasing
        // bound distance.
        for pos in &bounddistpos[..nbounddist] {
            let k = *pos as usize;
            let var = get_vars(scip)[data.cutinds[k] as usize];

            let (bestlb, bestlbtype) = find_best_lb(scip, var, sol, 0, allowlocal);

            if scip.set.is_infinity(-bestlb) {
                continue;
            }

            let (bestub, bestubtype) = find_best_ub(scip, var, sol, 0, allowlocal);

            if scip.set.is_infinity(bestub) {
                continue;
            }

            // Switch the complementation of this variable: compute newrhs =
            // mksetrhs + tmpcoefs[k - intstart] * (bestlb - bestub).
            let quadprod = prod_dd(tmpcoefs[k - intstart], bestlb - bestub);
            let newrhs = sum_qq(mksetrhs, quadprod);
            tmpcoefs[k - intstart] = -tmpcoefs[k - intstart];

            let oldsolval = tmpvalues[k - intstart];
            let solval = get_sol_val(scip, sol, var);
            tmpvalues[k - intstart] = if varsign[k] == 1 { bestub - solval } else { solval - bestlb };

            // Compute new violation.
            let newefficacy = compute_mir_efficacy(
                scip,
                &tmpcoefs,
                &tmpvalues,
                newrhs.to_dbl(),
                contactivity,
                contsqrnorm,
                bestdelta,
                ntmpcoefs as i32,
                minfrac,
                maxfrac,
            );

            // Check if violation was increased.
            if newefficacy > bestefficacy {
                // Keep change of complementation.
                bestefficacy = newefficacy;
                mksetrhs = newrhs;

                if varsign[k] == 1 {
                    // Switch to upper bound.
                    boundtype[k] = bestubtype;
                    varsign[k] = -1;
                } else {
                    // Switch to lower bound.
                    boundtype[k] = bestlbtype;
                    varsign[k] = 1;
                }

                localbdsused = localbdsused || boundtype[k] == -2;
            } else {
                // Undo the change of the complementation.
                tmpcoefs[k - intstart] = -tmpcoefs[k - intstart];
                tmpvalues[k - intstart] = oldsolval;
            }
        }

        if bestefficacy > 0.0 {
            let scale = 1.0 / bestdelta;
            mksetrhs = prod_qd(mksetrhs, scale);

            let downrhs = crate::quad::eps_floor_q(mksetrhs, scip.set.epsilon());
            let mut f0 = sum_qq(mksetrhs, -downrhs);

            // Renormalize f0 value.
            f0 = sum_dd(f0.hi, f0.lo);

            // Scale entries by the chosen scale factor.
            for i in 0..mksetnnz as usize {
                let c = &mut data.cutcoefs[data.cutinds[i] as usize];
                *c = prod_qd(*c, scale);
            }

            mksetrhs = downrhs;
            data.cutrhs = mksetrhs;

            cuts_round_mir(scip, &mut data, &varsign, &boundtype, f0);

            // Substitute aggregated slack variables (see cutsSubstituteMIR).
            {
                let MirData {
                    cutcoefs,
                    cutrhs,
                    cutinds,
                    ncutinds,
                    ..
                } = &mut data;
                cuts_substitute_mir(
                    scip,
                    &aggrrow.rowweights,
                    &aggrrow.slacksign,
                    &aggrrow.rowsinds,
                    aggrrow.nrows,
                    scale,
                    cutcoefs,
                    cutrhs,
                    cutinds,
                    ncutinds,
                    f0,
                );
            }

            *out.cutislocal = *out.cutislocal || localbdsused;

            // Remove all nearly-zero coefficients from MIR row and relax the right
            // hand side correspondingly in order to prevent numerical rounding
            // errors.
            {
                let MirData {
                    cutcoefs,
                    cutrhs,
                    cutinds,
                    ncutinds,
                    ..
                } = &mut data;
                if postprocess {
                    success = postprocess_cut_quad(scip, *out.cutislocal, cutinds, cutcoefs, ncutinds, cutrhs);
                } else {
                    success = !remove_zeros_quad(scip, scip.set.sumepsilon(), *out.cutislocal, cutcoefs, cutrhs, cutinds, ncutinds);
                }
            }

            if success {
                let mirefficacy = calc_efficacy_dense_storage_quad(scip, sol, &data.cutcoefs, data.cutrhs.to_dbl(), &data.cutinds, data.ncutinds);

                let cutefficacy = out.cutefficacy.unwrap();
                if crate::api::is_efficacious(scip, mirefficacy) && scip.set.is_gt(mirefficacy, *cutefficacy) {
                    let n = data.ncutinds as usize;
                    out.cutinds[..n].copy_from_slice(&data.cutinds[..n]);
                    for i in 0..n {
                        let j = out.cutinds[i] as usize;
                        out.cutcoefs[i] = data.cutcoefs[j].to_dbl();
                    }

                    *out.cutnnz = data.ncutinds;
                    *out.cutrhs = data.cutrhs.to_dbl();
                    *cutefficacy = mirefficacy;
                    if let Some(r) = out.cutrank {
                        *r = aggrrow.rank + 1;
                    }
                    *out.cutislocal = *out.cutislocal || localbdsused;
                } else {
                    success = false;
                }
            }
        }
    }

    Ok(success)
}

/*
 * Flow cover
 */

/// `MAXABSVBCOEF`: maximal absolute coefficient in variable bounds used for
/// snf relaxation.
const MAXABSVBCOEF: f64 = 1e+5;
/// `MAXBOUND`: maximal value of normal bounds used for snf relaxation.
const MAXBOUND: f64 = 1e+10;

/// `LIFTINGDATA`: structure that contains all data required to perform the
/// sequence independent lifting.
#[derive(Default)]
struct LiftingData {
    /// M_0 := 0.0 and M_i := M_i-1 + m_i.
    big_m: Vec<f64>,
    /// Non-increasing array of variable upper bound coefficients for all
    /// variables in C^{++} and L^-, where C = C^+ \cup C^- is the flowcover
    /// and C^{++} := { j \in C^+ | u_j > lambda },
    /// L^- := { j \in (N^- \setminus C^-) | u_j > lambda }.
    m: Vec<f64>,
    /// Size of array m.
    r: i32,
    /// Index of smallest value in m that comes from a variable in C^{++}.
    t: i32,
    /// Right hand side of single-node-flow set plus the sum of all u_j for
    /// j \in C^-.
    d1: f64,
    /// Right hand side of single-node-flow set plus the sum of all u_j for
    /// j \in N^-.
    d2: f64,
    /// Excess of the flowcover.
    lambda: f64,
    /// Smallest variable bound coefficient of variable in C^{++}
    /// (min_{j \in C++} u_j).
    mp: f64,
    /// ml := min(lambda, \sum_{j \in C^+ \setminus C^{++}} u_j).
    ml: f64,
}

/// `SNF_RELAXATION`: structure that contains all the data that defines the
/// single-node-flow relaxation of an aggregation row.
#[derive(Default)]
struct SnfRelaxation {
    /// Coefficients of all vars in relaxed set.
    transvarcoefs: Vec<i32>,
    /// Sol val of bin var in vub of all vars in relaxed set.
    transbinvarsolvals: Vec<f64>,
    /// Sol val of all real vars in relaxed set.
    transcontvarsolvals: Vec<f64>,
    /// Coefficient in vub of all vars in relaxed set.
    transvarvubcoefs: Vec<f64>,
    /// Number of vars in relaxed set.
    ntransvars: i32,
    /// Rhs in relaxed set.
    transrhs: f64,
    /// Associated original binary var for all vars in relaxed set.
    origbinvars: Vec<i32>,
    /// Associated original continuous var for all vars in relaxed set.
    origcontvars: Vec<i32>,
    /// Aggregation coefficient of the original binary var used to define the
    /// continuous variable in the relaxed set.
    aggrcoefsbin: Vec<f64>,
    /// Aggregation coefficient of the original continuous var used to define
    /// the continuous variable in the relaxed set.
    aggrcoefscont: Vec<f64>,
    /// Aggregation constant used to define the continuous variable in the
    /// relaxed set.
    aggrconstants: Vec<f64>,
}

/// `getClosestVlb` and `getClosestVub`: get solution value and index of
/// variable lower (upper) bound (with binary variable) which is closest to
/// the current LP solution value of a given variable; candidates have to
/// meet certain criteria in order to ensure the nonnegativity of the
/// variable upper bound imposed on the real variable in the 0-1 single node
/// flow relaxation associated with the given variable. `binvarused` stores
/// if a binary variable was already used (+1), was not used (0) or was not
/// used but is contained in the row (-1); `bestsb` is the closest simple
/// upper (lower) bound of the variable. Returns the value and the index (-1
/// if none was found).
#[allow(clippy::too_many_arguments)]
fn get_closest_vbound(scip: &Scip, var: VarId, sol: SolId, rowcoefs: &[Quad], binvarused: &[i8], bestsb: f64, rowcoef: f64, upper: bool) -> (f64, i32) {
    let v = &scip.vars[var];
    let vbds = if upper { &v.vubs } else { &v.vlbs };
    let nvbds = crate::implics::vbounds_get_n_vbds(vbds);
    let nbinvars = crate::api::get_n_bin_vars(scip);

    let mut closestidx = -1;
    let mut closest = if upper { scip.set.infinity() } else { -scip.set.infinity() };

    if nvbds > 0 {
        let vb = vbds.as_ref().unwrap();

        for i in 0..nvbds as usize {
            if if upper { bestsb < vb.constants[i] } else { bestsb > vb.constants[i] } {
                continue;
            }

            // For numerical reasons, ignore variable bounds with large absolute
            // coefficient and those which lead to an infinite variable bound
            // coefficient (val2) in snf relaxation.
            if vb.coefs[i].abs() > MAXABSVBCOEF {
                continue;
            }

            // Use only variable bounds with x_i binary which are active.
            let probidxbinvar = scip.vars[vb.vars[i]].probindex;

            // If the variable is not active the problem index is -1, so we cast
            // to unsigned int before the comparison which ensures that the
            // problem index is between 0 and nbinvars - 1.
            if probidxbinvar as u32 >= nbinvars as u32 {
                continue;
            }

            // Check if the current variable bound imposed on y_j meets the
            // criteria (let a_j = coefficient of y_j in current row, u_j (l_j) =
            // closest simple upper (lower) bound imposed on y_j, c_i =
            // coefficient of x_i in current row):
            //  0. no other non-binary variable y_k has used a variable bound with
            //     x_i to get transformed variable y'_k yet
            // vlb l~_i * x_i + d_i, if a_j > 0 (if a_j < 0, the last two
            // inequalities reversed):
            //  1. u_j <= d_i
            //  2. a_j ( u_j - d_i ) + c_i <= 0
            //  3. a_j l~_i + c_i <= 0
            // vub u~_i * x_i + d_i, if a_j > 0 (if a_j < 0, the last two
            // inequalities reversed):
            //  1. l_j >= d_i
            //  2. a_j ( l_j - d_i ) + c_i >= 0
            //  3. a_j u~_i + c_i >= 0

            // Has already been used in the SNF relaxation.
            if binvarused[probidxbinvar as usize] == 1 {
                continue;
            }

            // Get the row coefficient.
            let rowcoefbinvar = rowcoefs[probidxbinvar as usize].to_dbl();

            let rowcoefsign = 1.0f64.copysign(rowcoef);

            let val2 = rowcoefsign * ((rowcoef * vb.coefs[i]) + rowcoefbinvar);

            // Variable bound does not meet criteria.
            if upper {
                if val2 < 0.0 || scip.set.is_infinity(val2) {
                    continue;
                }
            } else if val2 > 0.0 || scip.set.is_infinity(-val2) {
                continue;
            }

            let val1 = rowcoefsign * ((rowcoef * (bestsb - vb.constants[i])) + rowcoefbinvar);

            // Variable bound does not meet criteria.
            if if upper { val1 < 0.0 } else { val1 > 0.0 } {
                continue;
            }

            let vbsol = vb.coefs[i] * get_sol_val(scip, sol, vb.vars[i]) + vb.constants[i];
            if if upper { vbsol < closest } else { vbsol > closest } {
                closest = vbsol;
                closestidx = i as i32;
            }
        }
    }

    (closest, closestidx)
}

/// The bounds `determineBoundForSNF` stores per row position.
#[derive(Default)]
struct SnfBounds {
    bestlb: Vec<f64>,
    bestub: Vec<f64>,
    bestslb: Vec<f64>,
    bestsub: Vec<f64>,
    bestlbtype: Vec<i32>,
    bestubtype: Vec<i32>,
    bestslbtype: Vec<i32>,
    bestsubtype: Vec<i32>,
    selectedbounds: Vec<crate::types::BoundType>,
}

impl SnfBounds {
    fn new(n: usize) -> Self {
        SnfBounds {
            bestlb: vec![0.0; n],
            bestub: vec![0.0; n],
            bestslb: vec![0.0; n],
            bestsub: vec![0.0; n],
            bestlbtype: vec![0; n],
            bestubtype: vec![0; n],
            bestslbtype: vec![0; n],
            bestsubtype: vec![0; n],
            selectedbounds: vec![crate::types::BoundType::Lower; n],
        }
    }
}

/// `determineBoundForSNF`: determines the bounds to use for constructing
/// the single-node-flow relaxation of a variable in the given row; returns
/// whether the variable is a free variable.
#[allow(clippy::too_many_arguments)]
fn determine_bound_for_snf(
    scip: &mut Scip,
    sol: SolId,
    rowcoefs: &[Quad],
    rowinds: &[i32],
    varposinrow: usize,
    binvarused: &mut [i8],
    allowlocal: bool,
    boundswitch: f64,
    b: &mut SnfBounds,
) -> bool {
    use crate::types::{BoundType, VarType};
    let p = varposinrow;

    b.bestlb[p] = -scip.set.infinity();
    b.bestub[p] = scip.set.infinity();
    b.bestlbtype[p] = -3;
    b.bestubtype[p] = -3;

    let probidx = rowinds[p] as usize;
    let var = get_vars(scip)[probidx];
    let rowcoef = rowcoefs[probidx].to_dbl();

    // Get closest simple lower bound and closest simple upper bound.
    (b.bestslb[p], b.bestslbtype[p]) = find_best_lb(scip, var, sol, 0, allowlocal);
    (b.bestsub[p], b.bestsubtype[p]) = find_best_ub(scip, var, sol, 0, allowlocal);

    // Do not use too large bounds.
    if b.bestslb[p] <= -MAXBOUND {
        b.bestslb[p] = -scip.set.infinity();
    }

    if b.bestsub[p] >= MAXBOUND {
        b.bestsub[p] = scip.set.infinity();
    }

    let solval = get_sol_val(scip, sol, var);

    // Mixed integer set cannot be relaxed to 0-1 single node flow set because
    // both simple bounds are -infinity and infinity, respectively.
    if scip.set.is_infinity(-b.bestslb[p]) && scip.set.is_infinity(b.bestsub[p]) {
        return true;
    }

    let continuous = scip.vars[var].vartype == VarType::Continuous;

    // Get closest lower bound that can be used to define the real variable
    // y'_j in the 0-1 single node flow relaxation.
    if !scip.set.is_infinity(b.bestsub[p]) {
        b.bestlb[p] = b.bestslb[p];
        b.bestlbtype[p] = b.bestslbtype[p];

        if continuous {
            let (bestvlb, bestvlbidx) = get_closest_vbound(scip, var, sol, rowcoefs, binvarused, b.bestsub[p], rowcoef, false);
            if scip.set.is_gt(bestvlb, b.bestlb[p]) {
                b.bestlb[p] = bestvlb;
                b.bestlbtype[p] = bestvlbidx;
            }
        }
    }

    // Get closest upper bound that can be used to define the real variable
    // y'_j in the 0-1 single node flow relaxation.
    if !scip.set.is_infinity(-b.bestslb[p]) {
        b.bestub[p] = b.bestsub[p];
        b.bestubtype[p] = b.bestsubtype[p];

        if continuous {
            let (bestvub, bestvubidx) = get_closest_vbound(scip, var, sol, rowcoefs, binvarused, b.bestslb[p], rowcoef, true);
            if scip.set.is_lt(bestvub, b.bestub[p]) {
                b.bestub[p] = bestvub;
                b.bestubtype[p] = bestvubidx;
            }
        }
    }

    // Mixed integer set cannot be relaxed to 0-1 single node flow set because
    // there are no suitable bounds to define the transformed variable y'_j.
    if scip.set.is_infinity(-b.bestlb[p]) && scip.set.is_infinity(b.bestub[p]) {
        return true;
    }

    // Select best upper bound if it is closer to the LP value of y_j and best
    // lower bound otherwise and use this bound to define the real variable
    // y'_j with 0 <= y'_j <= u'_j x_j in the 0-1 single node flow relaxation;
    // prefer variable bounds.
    let mid = (1.0 - boundswitch) * b.bestlb[p] + boundswitch * b.bestub[p];
    b.selectedbounds[p] = if scip.set.is_eq(solval, mid) && b.bestlbtype[p] >= 0 {
        BoundType::Lower
    } else if scip.set.is_eq(solval, mid) && b.bestubtype[p] >= 0 {
        BoundType::Upper
    } else if scip.set.is_le(solval, mid) {
        BoundType::Lower
    } else {
        BoundType::Upper
    };

    if b.selectedbounds[p] == BoundType::Lower && b.bestlbtype[p] >= 0 {
        // Mark binary variable of vlb so that it is not used for other
        // continuous variables by setting it's position in the aggrrow to a
        // negative value.
        let vlbvar = scip.vars[var].vlbs.as_ref().unwrap().vars[b.bestlbtype[p]];
        binvarused[scip.vars[vlbvar].probindex as usize] = 1;
    } else if b.selectedbounds[p] == BoundType::Upper && b.bestubtype[p] >= 0 {
        // Mark binary variable of vub so that it is not used for other
        // continuous variables by setting it's position in the aggrrow to a
        // negative value.
        let vubvar = scip.vars[var].vubs.as_ref().unwrap().vars[b.bestubtype[p]];
        binvarused[scip.vars[vubvar].probindex as usize] = 1;
    }

    false
}

impl SnfRelaxation {
    /// `allocSNFRelaxation`: allocate buffer arrays for storing the
    /// single-node-flow relaxation.
    fn new(nvars: usize) -> Self {
        SnfRelaxation {
            transvarcoefs: vec![0; nvars],
            transbinvarsolvals: vec![0.0; nvars],
            transcontvarsolvals: vec![0.0; nvars],
            transvarvubcoefs: vec![0.0; nvars],
            origbinvars: vec![0; nvars],
            origcontvars: vec![0; nvars],
            aggrcoefsbin: vec![0.0; nvars],
            aggrcoefscont: vec![0.0; nvars],
            aggrconstants: vec![0.0; nvars],
            ..SnfRelaxation::default()
        }
    }

    /// Stores the transformed variable at the current position: its
    /// coefficient (+1 or -1), variable upper bound coefficient and the
    /// solution values of its binary and real variables.
    #[inline]
    fn set_trans(&mut self, coef: i32, vubcoef: f64, binsolval: f64, contsolval: f64) {
        let n = self.ntransvars as usize;
        self.transvarcoefs[n] = coef;
        self.transvarvubcoefs[n] = vubcoef;
        self.transbinvarsolvals[n] = binsolval;
        self.transcontvarsolvals[n] = contsolval;
    }
}

/// `constructSNFRelaxation`: construct a 0-1 single node flow relaxation
/// (with some additional simple constraints) of a mixed integer set
/// corresponding to the given aggrrow a * x <= rhs; returns `(success,
/// localbdsused)`: whether the transformation was valid, and whether local
/// bounds were used in it.
#[allow(clippy::too_many_arguments)]
fn construct_snf_relaxation(
    scip: &mut Scip,
    sol: SolId,
    boundswitch: f64,
    allowlocal: bool,
    rowcoefs: &[Quad],
    rowrhs: Quad,
    rowinds: &mut [i32],
    nnz: i32,
    snf: &mut SnfRelaxation,
) -> (bool, bool) {
    use crate::{quad::prod_qq, types::BoundType};

    let mut localbdsused = false;
    let nbinvars = crate::api::get_n_bin_vars(scip);
    let n = nnz as usize;

    // Arrays to store the selected bound for each non-binary variable in the
    // row.
    let mut b = SnfBounds::new(n);

    // Sort descending to have continuous variables first.
    crate::sort::sort_down(&mut rowinds[..n], (), crate::sort::cmp_int);

    // Array to store whether a binary variable is in the row (-1) or has been
    // used (1) due to variable bound usage.
    let mut binvarused = vec![0i8; nbinvars as usize];

    let mut i = nnz - 1;
    while i >= 0 && rowinds[i as usize] < nbinvars {
        binvarused[rowinds[i as usize] as usize] = -1;
        i -= 1;
    }

    let nnonbinvarsrow = (i + 1) as usize;

    // Determine the bounds to use for transforming the non-binary variables.
    for i in 0..nnonbinvarsrow {
        let freevariable = determine_bound_for_snf(scip, sol, rowcoefs, rowinds, i, &mut binvarused, allowlocal, boundswitch, &mut b);

        if freevariable {
            // The C code clears binvarused here; it is a local buffer.
            return (false, localbdsused);
        }
    }

    let mut transrhs = rowrhs;
    snf.ntransvars = 0;

    // Transform non-binary variables.
    for i in 0..nnonbinvarsrow {
        let probidx = rowinds[i] as usize;
        let var = get_vars(scip)[probidx];
        let rowcoef = rowcoefs[probidx];
        let solval = get_sol_val(scip, sol, var);
        let nt = snf.ntransvars as usize;

        if b.selectedbounds[i] == BoundType::Lower {
            // Use bestlb to define y'_j; store for y_j that bestlb is the bound
            // used to define y'_j and that y'_j is the associated real variable
            // in the relaxed set.
            snf.origcontvars[nt] = probidx as i32;

            if b.bestlbtype[i] < 0 {
                // Use simple lower bound in bestlb = l_j <= y_j <= u_j = bestsub
                // to define
                //   y'_j = - a_j ( y_j - u_j ) with 0 <= y'_j <=   a_j ( u_j - l_j ) x_j and x_j = 1    if a_j > 0
                //   y'_j =   a_j ( y_j - u_j ) with 0 <= y'_j <= - a_j ( u_j - l_j ) x_j and x_j = 1    if a_j < 0,
                // put j into the set
                //   N2   if a_j > 0
                //   N1   if a_j < 0
                // and update the right hand side of the constraint in the
                // relaxation
                //   rhs = rhs - a_j u_j
                let mut val = sum_dd(b.bestsub[i], -b.bestlb[i]);
                val = prod_qq(val, rowcoef);
                let mut contsolval = sum_dd(solval, -b.bestsub[i]);
                contsolval = prod_qq(contsolval, rowcoef);

                if b.bestlbtype[i] == -2 || b.bestsubtype[i] == -2 {
                    localbdsused = true;
                }

                let rowcoeftimesbestsub = prod_qd(rowcoef, b.bestsub[i]);

                // Store aggregation information for y'_j for transforming cuts
                // for the SNF relaxation back to the problem variables later.
                snf.origbinvars[nt] = -1;
                snf.aggrcoefsbin[nt] = 0.0;

                if rowcoef.to_dbl() >= 0.0 {
                    snf.set_trans(-1, val.to_dbl(), 1.0, -contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrconstants[nt] = rowcoeftimesbestsub.to_dbl();
                    snf.aggrcoefscont[nt] = -rowcoef.to_dbl();
                } else {
                    snf.set_trans(1, -val.to_dbl(), 1.0, contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrconstants[nt] = -rowcoeftimesbestsub.to_dbl();
                    snf.aggrcoefscont[nt] = rowcoef.to_dbl();
                }

                transrhs = sum_qq(transrhs, -rowcoeftimesbestsub);
            } else {
                let vb = scip.vars[var].vlbs.as_deref().unwrap();
                let k = b.bestlbtype[i] as usize;
                let (vlbvar, vlbconst, vlbcoef) = (vb.vars[k], vb.constants[k], vb.coefs[k]);

                // Use variable lower bound in bestlb = l~_j x_j + d_j <= y_j <=
                // u_j = bestsub to define
                //   y'_j = - ( a_j ( y_j - d_j ) + c_j x_j ) with 0 <= y'_j <= - ( a_j l~_j + c_j ) x_j    if a_j > 0
                //   y'_j =     a_j ( y_j - d_j ) + c_j x_j   with 0 <= y'_j <=   ( a_j l~_j + c_j ) x_j    if a_j < 0,
                // where c_j is the coefficient of x_j in the row, put j into the
                // set
                //   N2   if a_j > 0
                //   N1   if a_j < 0
                // and update the right hand side of the constraint in the
                // relaxation
                //   rhs = rhs - a_j d_j
                let vlbvarprobidx = scip.vars[vlbvar].probindex as usize;
                let rowcoefbinary = rowcoefs[vlbvarprobidx];
                let varsolvalbinary = get_sol_val(scip, sol, vlbvar);

                let mut val = prod_qd(rowcoef, vlbcoef);
                val = sum_qq(val, rowcoefbinary);
                let contsolval = {
                    let tmp = prod_qd(rowcoefbinary, varsolvalbinary);
                    let mut c = sum_dd(solval, -vlbconst);
                    c = prod_qq(c, rowcoef);
                    sum_qq(c, tmp)
                };

                let rowcoeftimesvlbconst = prod_qd(rowcoef, vlbconst);

                // Clear the binvarpos array, since the variable has been
                // processed.
                binvarused[vlbvarprobidx] = 0;

                // Store aggregation information for y'_j for transforming cuts
                // for the SNF relaxation back to the problem variables later.
                snf.origbinvars[nt] = vlbvarprobidx as i32;

                if rowcoef.to_dbl() >= 0.0 {
                    snf.set_trans(-1, -val.to_dbl(), varsolvalbinary, -contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrcoefsbin[nt] = -rowcoefbinary.to_dbl();
                    snf.aggrcoefscont[nt] = -rowcoef.to_dbl();
                    snf.aggrconstants[nt] = rowcoeftimesvlbconst.to_dbl();
                } else {
                    snf.set_trans(1, val.to_dbl(), varsolvalbinary, contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrcoefsbin[nt] = rowcoefbinary.to_dbl();
                    snf.aggrcoefscont[nt] = rowcoef.to_dbl();
                    snf.aggrconstants[nt] = -rowcoeftimesvlbconst.to_dbl();
                }

                transrhs = sum_qq(transrhs, -rowcoeftimesvlbconst);
            }
        } else {
            // Use bestub to define y'_j; store for y_j that y'_j is the
            // associated real variable in the relaxed set.
            snf.origcontvars[nt] = probidx as i32;

            if b.bestubtype[i] < 0 {
                // Use simple upper bound in bestslb = l_j <= y_j <= u_j = bestub
                // to define
                //   y'_j =   a_j ( y_j - l_j ) with 0 <= y'_j <=   a_j ( u_j - l_j ) x_j and x_j = 1    if a_j > 0
                //   y'_j = - a_j ( y_j - l_j ) with 0 <= y'_j <= - a_j ( u_j - l_j ) x_j and x_j = 1    if a_j < 0,
                // put j into the set
                //   N1   if a_j > 0
                //   N2   if a_j < 0
                // and update the right hand side of the constraint in the
                // relaxation
                //   rhs = rhs - a_j l_j
                let mut val = sum_dd(b.bestub[i], -b.bestslb[i]);
                val = prod_qq(val, rowcoef);
                let mut contsolval = sum_dd(solval, -b.bestslb[i]);
                contsolval = prod_qq(contsolval, rowcoef);

                if b.bestubtype[i] == -2 || b.bestslbtype[i] == -2 {
                    localbdsused = true;
                }

                let rowcoeftimesbestslb = prod_qd(rowcoef, b.bestslb[i]);

                // Store aggregation information for y'_j for transforming cuts
                // for the SNF relaxation back to the problem variables later.
                snf.origbinvars[nt] = -1;
                snf.aggrcoefsbin[nt] = 0.0;

                if rowcoef.to_dbl() >= 0.0 {
                    snf.set_trans(1, val.to_dbl(), 1.0, contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrcoefscont[nt] = rowcoef.to_dbl();
                    snf.aggrconstants[nt] = -rowcoeftimesbestslb.to_dbl();
                } else {
                    snf.set_trans(-1, -val.to_dbl(), 1.0, -contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrcoefscont[nt] = -rowcoef.to_dbl();
                    snf.aggrconstants[nt] = rowcoeftimesbestslb.to_dbl();
                }

                transrhs = sum_qq(transrhs, -rowcoeftimesbestslb);
            } else {
                let vb = scip.vars[var].vubs.as_deref().unwrap();
                let k = b.bestubtype[i] as usize;
                let (vubvar, vubconst, vubcoef) = (vb.vars[k], vb.constants[k], vb.coefs[k]);

                // Use variable upper bound in bestslb = l_j <= y_j <= u~_j x_j +
                // d_j = bestub to define
                //   y'_j =     a_j ( y_j - d_j ) + c_j x_j   with 0 <= y'_j <=   ( a_j u~_j + c_j ) x_j    if a_j > 0
                //   y'_j = - ( a_j ( y_j - d_j ) + c_j x_j ) with 0 <= y'_j <= - ( a_j u~_j + c_j ) x_j    if a_j < 0,
                // where c_j is the coefficient of x_j in the row, put j into the
                // set
                //   N1   if a_j > 0
                //   N2   if a_j < 0
                // and update the right hand side of the constraint in the
                // relaxation
                //   rhs = rhs - a_j d_j
                let vubvarprobidx = scip.vars[vubvar].probindex as usize;
                let rowcoefbinary = rowcoefs[vubvarprobidx];
                let varsolvalbinary = get_sol_val(scip, sol, vubvar);

                // Clear the binvarpos array, since the variable has been
                // processed.
                binvarused[vubvarprobidx] = 0;

                let mut val = prod_qd(rowcoef, vubcoef);
                val = sum_qq(val, rowcoefbinary);
                let contsolval = {
                    let tmp = prod_qd(rowcoefbinary, varsolvalbinary);
                    let mut c = sum_dd(solval, -vubconst);
                    c = prod_qq(c, rowcoef);
                    sum_qq(c, tmp)
                };

                let rowcoeftimesvubconst = prod_qd(rowcoef, vubconst);

                // Store aggregation information for y'_j for transforming cuts
                // for the SNF relaxation back to the problem variables later.
                snf.origbinvars[nt] = vubvarprobidx as i32;

                if rowcoef.to_dbl() >= 0.0 {
                    snf.set_trans(1, val.to_dbl(), varsolvalbinary, contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrcoefsbin[nt] = rowcoefbinary.to_dbl();
                    snf.aggrcoefscont[nt] = rowcoef.to_dbl();
                    snf.aggrconstants[nt] = -rowcoeftimesvubconst.to_dbl();
                } else {
                    snf.set_trans(-1, -val.to_dbl(), varsolvalbinary, -contsolval.to_dbl());

                    // Aggregation information for y'_j.
                    snf.aggrcoefsbin[nt] = -rowcoefbinary.to_dbl();
                    snf.aggrcoefscont[nt] = -rowcoef.to_dbl();
                    snf.aggrconstants[nt] = rowcoeftimesvubconst.to_dbl();
                }

                transrhs = sum_qq(transrhs, -rowcoeftimesvubconst);
            }
        }

        // Make sure the coefficient is not negative due to small numerical
        // rounding errors.
        snf.transvarvubcoefs[nt] = crate::def::max(snf.transvarvubcoefs[nt], 0.0);

        snf.ntransvars += 1;
    }

    snf.transrhs = transrhs.to_dbl();

    // Transform remaining binary variables of row.
    for rowind in &rowinds[nnonbinvarsrow..n] {
        let probidx = *rowind as usize;

        // Binary variable was processed together with a non-binary variable.
        if binvarused[probidx] == 0 {
            continue;
        }

        // Binary variable was not processed yet, so the binvarused value sould
        // be -1; set binvarused to zero since it has been processed.
        binvarused[probidx] = 0;

        let var = get_vars(scip)[probidx];
        let rowcoef = rowcoefs[probidx];
        let varsolval = get_sol_val(scip, sol, var);

        // Define
        //   y'_j =   c_j x_j with 0 <= y'_j <=   c_j x_j    if c_j > 0
        //   y'_j = - c_j x_j with 0 <= y'_j <= - c_j x_j    if c_j < 0,
        // where c_j is the coefficient of x_j in the row and put j into the set
        //   N1   if c_j > 0
        //   N2   if c_j < 0.
        let val = rowcoef.to_dbl();
        let contsolval = rowcoef.to_dbl() * varsolval;
        let nt = snf.ntransvars as usize;

        // Store aggregation information for y'_j for transforming cuts for the
        // SNF relaxation back to the problem variables later.
        snf.origbinvars[nt] = probidx as i32;
        snf.origcontvars[nt] = -1;
        snf.aggrcoefscont[nt] = 0.0;
        snf.aggrconstants[nt] = 0.0;

        if rowcoef.to_dbl() >= 0.0 {
            snf.set_trans(1, val, varsolval, contsolval);

            // Aggregation information for y'_j.
            snf.aggrcoefsbin[nt] = rowcoef.to_dbl();
        } else {
            snf.set_trans(-1, -val, varsolval, -contsolval);

            // Aggregation information for y'_j.
            snf.aggrcoefsbin[nt] = -rowcoef.to_dbl();
        }

        // Updates number of variables in transformed problem.
        snf.ntransvars += 1;
    }

    // Construction was successful.
    (true, localbdsused)
}

/// What `SCIPsolveKnapsackApproximatelyLT` selected.
#[derive(Default)]
struct KnapsackItems {
    solitems: Vec<i32>,
    nonsolitems: Vec<i32>,
    solval: f64,
}

/// `SCIPsolveKnapsackApproximatelyLT`: solve knapsack problem in
/// maximization form with "<" constraint approximately by greedy; returns
/// the selected items, the items not selected, and the solution value.
/// `weights`, `profits` and `items` are permuted.
fn solve_knapsack_approximately_lt(scip: &Scip, nitems: usize, weights: &mut [f64], profits: &mut [f64], capacity: f64, items: &mut [i32]) -> KnapsackItems {
    let mut k = KnapsackItems::default();

    // Allocate memory for temporary array used for sorting; array should
    // contain profits divided by corresponding weights (p_1 / w_1 ... p_n /
    // w_n).
    let mut tempsort: Vec<f64> = (0..nitems).map(|i| profits[i] / weights[i]).collect();

    // Decrease capacity slightly to make it tighter than the original
    // capacity.
    let mediancapacity = capacity * (1.0 - scip.set.feastol());

    // Rearrange items around.
    crate::sort::select_weighted_down(
        &mut tempsort,
        (&mut profits[..nitems], &mut items[..nitems]),
        crate::sort::cmp_real,
        Some(&mut weights[..nitems]),
        mediancapacity,
    );

    // Select items as long as they fit into the knapsack.
    let mut solitemsweight = 0.0;
    let mut j = 0;
    while j < nitems && scip.set.is_feas_lt(solitemsweight + weights[j], capacity) {
        k.solitems.push(items[j]);
        k.solval += profits[j];
        solitemsweight += weights[j];
        j += 1;
    }

    // Continue to put items into the knapsack if they entirely fit.
    while j < nitems {
        if scip.set.is_feas_lt(solitemsweight + weights[j], capacity) {
            k.solitems.push(items[j]);
            k.solval += profits[j];
            solitemsweight += weights[j];
        } else {
            k.nonsolitems.push(items[j]);
        }
        j += 1;
    }

    k
}

/// `buildFlowCover`: build the flow cover which corresponds to the given
/// exact or approximate solution of KP^SNF; given unfinished flow cover
/// contains variables which have been fixed in advance; returns lambda.
#[allow(clippy::too_many_arguments)]
fn build_flow_cover(
    coefs: &[i32],
    vubcoefs: &[f64],
    rhs: f64,
    k: &KnapsackItems,
    nflowcovervars: &mut i32,
    nnonflowcovervars: &mut i32,
    flowcoverstatus: &mut [i32],
    flowcoverweight: &mut Quad,
) -> f64 {
    // Get flowcover status for each item.
    for &item in &k.solitems {
        let item = item as usize;
        // j in N1 with z°_j = 1 => j in N1\C1
        if coefs[item] == 1 {
            flowcoverstatus[item] = -1;
            *nnonflowcovervars += 1;
        }
        // j in N2 with z_j = 1 => j in C2
        else {
            flowcoverstatus[item] = 1;
            *nflowcovervars += 1;
            *flowcoverweight = sum_qd(*flowcoverweight, -vubcoefs[item]);
        }
    }

    for &item in &k.nonsolitems {
        let item = item as usize;
        // j in N1 with z°_j = 0 => j in C1
        if coefs[item] == 1 {
            flowcoverstatus[item] = 1;
            *nflowcovervars += 1;
            *flowcoverweight = sum_qd(*flowcoverweight, vubcoefs[item]);
        }
        // j in N2 with z_j = 0 => j in N2\C2
        else {
            flowcoverstatus[item] = -1;
            *nnonflowcovervars += 1;
        }
    }

    // Get lambda = sum_{j in C1} u_j - sum_{j in C2} u_j - rhs.
    let tmp = sum_qd(*flowcoverweight, -rhs);
    tmp.to_dbl()
}

/// `getFlowCover` (the `NO_EXACT_KNAPSACK` variant the reference build
/// compiles): get a flow cover (C1, C2) for a given 0-1 single node flow set
///   {(x,y) in {0,1}^n x R^n : sum_{j in N1} y_j - sum_{j in N2} y_j <= b, 0 <= y_j <= u_j x_j},
/// i.e., get sets C1 \subset N1 and C2 \subset N2 with
/// \sum_{j in C1} u_j - sum_{j in C2} u_j = b + lambda and lambda > 0;
/// fills `flowcoverstatus` (+1 in the cover, -1 not) and returns
/// `(nflowcovervars, nnonflowcovervars, lambda, found)`.
fn get_flow_cover(scip: &Scip, snf: &SnfRelaxation, flowcoverstatus: &mut [i32]) -> (i32, i32, f64, bool) {
    let nt = snf.ntransvars as usize;
    let mut items = vec![0; nt];
    let mut transprofitsreal = vec![0.0; nt];
    let mut transweightsreal = vec![0.0; nt];

    flowcoverstatus[..nt].fill(0);
    let mut nflowcovervars = 0;
    let mut nnonflowcovervars = 0;
    let mut lambda = 0.0;
    let mut flowcoverweight = Quad::new(0.0);

    // Fix some variables in advance according to the following fixing
    // strategy
    //   put j into N1\C1,          if j in N1 and x*_j = 0,
    //   put j into C1,             if j in N1 and x*_j = 1,
    //   put j into C2,             if j in N2 and x*_j = 1,
    //   put j into N2\C2,          if j in N2 and x*_j = 0
    // and get the set of the remaining variables.
    let mut nitems = 0;
    let mut n1itemsweight = 0.0;
    for j in 0..nt {
        // If u_j = 0, put j into N1\C1 and N2\C2, respectively.
        if scip.set.is_feas_zero(snf.transvarvubcoefs[j]) {
            flowcoverstatus[j] = -1;
            nnonflowcovervars += 1;
            continue;
        }

        // x*_j is fractional.
        if !scip.set.is_feas_integral(snf.transbinvarsolvals[j]) {
            items[nitems] = j as i32;
            nitems += 1;
            if snf.transvarcoefs[j] == 1 {
                n1itemsweight += snf.transvarvubcoefs[j];
            }
        }
        // j is in N1 and x*_j = 0.
        else if snf.transvarcoefs[j] == 1 && snf.transbinvarsolvals[j] < 0.5 {
            flowcoverstatus[j] = -1;
            nnonflowcovervars += 1;
        }
        // j is in N1 and x*_j = 1.
        else if snf.transvarcoefs[j] == 1 && snf.transbinvarsolvals[j] > 0.5 {
            flowcoverstatus[j] = 1;
            nflowcovervars += 1;
            flowcoverweight = sum_qd(flowcoverweight, snf.transvarvubcoefs[j]);
        }
        // j is in N2 and x*_j = 1.
        else if snf.transvarcoefs[j] == -1 && snf.transbinvarsolvals[j] > 0.5 {
            flowcoverstatus[j] = 1;
            nflowcovervars += 1;
            flowcoverweight = sum_qd(flowcoverweight, -snf.transvarvubcoefs[j]);
        }
        // j is in N2 and x*_j = 0.
        else {
            flowcoverstatus[j] = -1;
            nnonflowcovervars += 1;
        }
    }

    // To find a flow cover, transform the following knapsack problem
    //
    // (KP^SNF)      max sum_{j in N1} ( x*_j - 1 ) z_j + sum_{j in N2} x*_j z_j
    //                   sum_{j in N1}          u_j z_j - sum_{j in N2} u_j  z_j > b
    //                                         z_j in {0,1} for all j in N1 & N2
    //
    // to a knapsack problem in maximization form, such that all variables in
    // the knapsack constraint have positive weights and the constraint is a
    // "<" constraint, by complementing all variables in N1
    //
    // (KP^SNF_rat)  max sum_{j in N1} ( 1 - x*_j ) z°_j + sum_{j in N2} x*_j z_j
    //                   sum_{j in N1}          u_j z°_j + sum_{j in N2} u_j  z_j < - b + sum_{j in N1} u_j
    //                                                 z°_j in {0,1} for all j in N1
    //                                                  z_j in {0,1} for all j in N2,
    // and solve it approximately under consideration of the fixing.

    // Get weight and profit of variables in KP^SNF_rat.
    for j in 0..nitems {
        let it = items[j] as usize;
        transweightsreal[j] = snf.transvarvubcoefs[it];

        if snf.transvarcoefs[it] == 1 {
            transprofitsreal[j] = 1.0 - snf.transbinvarsolvals[it];
        } else {
            transprofitsreal[j] = snf.transbinvarsolvals[it];
        }
    }

    // Get capacity of knapsack constraint in KP^SNF_rat.
    let transcapacityreal = -snf.transrhs + flowcoverweight.to_dbl() + n1itemsweight;

    // There exists no flow cover if the capacity of knapsack constraint in
    // KP^SNF_rat after fixing is less than or equal to zero.
    if scip.set.is_feas_le(transcapacityreal / 10.0, 0.0) {
        return (nflowcovervars, nnonflowcovervars, lambda, false);
    }

    // KP^SNF_rat has been solved by fixing some variables in advance.
    if nitems == 0 {
        // Get lambda = sum_{j in C1} u_j - sum_{j in C2} u_j - rhs.
        flowcoverweight = sum_qd(flowcoverweight, -snf.transrhs);
        lambda = flowcoverweight.to_dbl();
        return (nflowcovervars, nnonflowcovervars, lambda, true);
    }

    // Solve KP^SNF_rat approximately.
    let k = solve_knapsack_approximately_lt(scip, nitems, &mut transweightsreal, &mut transprofitsreal, transcapacityreal, &mut items);

    // Build the flow cover from the solution of KP^SNF_rat and the fixing.
    lambda = build_flow_cover(
        &snf.transvarcoefs,
        &snf.transvarvubcoefs,
        snf.transrhs,
        &k,
        &mut nflowcovervars,
        &mut nnonflowcovervars,
        flowcoverstatus,
        &mut flowcoverweight,
    );

    let found = scip.set.is_feas_gt(lambda, 0.0);
    (nflowcovervars, nnonflowcovervars, lambda, found)
}

/// `evaluateLiftingFunction`: evaluate the super-additive lifting function
/// for the lifted simple generalized flowcover inequalities for a given
/// value x \in { u_j | j \in C- }.
fn evaluate_lifting_function(scip: &Scip, l: &LiftingData, x: f64) -> f64 {
    let xpluslambda = x + l.lambda;

    let mut i = 0;
    while i < l.r && scip.set.is_gt(xpluslambda, l.big_m[(i + 1) as usize]) {
        i += 1;
    }
    let iu = i as usize;

    if i < l.t {
        if scip.set.is_le(l.big_m[iu], x) {
            return i as f64 * l.lambda;
        }

        // Return x - liftingdata->M[i] + i * liftingdata->lambda.
        let mut tmp = prod_dd(i as f64, l.lambda);
        tmp = sum_qd(tmp, x);
        tmp = sum_qd(tmp, -l.big_m[iu]);
        return tmp.to_dbl();
    }

    if i < l.r {
        // p = liftingdata->m[i] - (liftingdata->mp - liftingdata->lambda) -
        // liftingdata->ml;
        let mut tmp = sum_dd(l.m[iu], -l.mp);
        tmp = sum_qd(tmp, -l.ml);
        tmp = sum_qd(tmp, l.lambda);

        // p = MAX(0.0, p);
        if tmp.hi < 0.0 {
            tmp = Quad::new(0.0);
        }

        tmp = sum_qd(tmp, l.big_m[iu]);
        tmp = sum_qd(tmp, l.ml);

        if scip.set.is_lt(tmp.to_dbl(), xpluslambda) {
            return i as f64 * l.lambda;
        }

        let mut tmp = prod_dd(i as f64, l.lambda);
        tmp = sum_qd(tmp, x);
        tmp = sum_qd(tmp, -l.big_m[iu]);
        return tmp.to_dbl();
    }

    let mut tmp = prod_dd(l.r as f64, l.lambda);
    tmp = sum_qd(tmp, x);
    tmp = sum_qd(tmp, -l.big_m[l.r as usize]);
    tmp.to_dbl()
}

/// `getAlphaAndBeta`: computes
///   (alpha_j, beta_j) = (0, 0)               if M_i <= u_j <= M_{i+1} - lambda
///                       (1, M_i - i lambda)  if M_i - lambda < u_j < M_i
fn get_alpha_and_beta(scip: &Scip, l: &LiftingData, vubcoef: f64) -> (i32, f64) {
    let vubcoefpluslambda = vubcoef + l.lambda;

    let mut i = 0;
    while i < l.r && scip.set.is_gt(vubcoefpluslambda, l.big_m[(i + 1) as usize]) {
        i += 1;
    }

    if scip.set.is_lt(vubcoef, l.big_m[i as usize]) {
        let mut tmp = prod_dd(-i as f64, l.lambda);
        tmp = sum_qd(tmp, l.big_m[i as usize]);
        (1, tmp.to_dbl())
    } else {
        (0, 0.0)
    }
}

/// `computeLiftingData`: compute relevant data for performing the sequence
/// independent lifting; returns the data if it is valid.
fn compute_lifting_data(scip: &Scip, snf: &SnfRelaxation, transvarflowcoverstatus: &[i32], lambda: f64) -> Option<LiftingData> {
    let mut l = LiftingData {
        m: vec![0.0; snf.ntransvars as usize],
        ..LiftingData::default()
    };
    l.r = 0;
    let mut sumn2mc2le = Quad::new(0.0);
    let mut sumc1le = Quad::new(0.0);
    let mut sumn2mc2gt = Quad::new(0.0);
    let mut sumc2 = Quad::new(0.0);

    l.mp = scip.set.infinity();

    for i in 0..snf.ntransvars as usize {
        let s = (snf.transvarcoefs[i] + 1) + (transvarflowcoverstatus[i] + 1) / 2;

        match s {
            // Var is in N2 \ C2.
            0 => {
                if scip.set.is_gt(snf.transvarvubcoefs[i], lambda) {
                    sumn2mc2gt = sum_qd(sumn2mc2gt, snf.transvarvubcoefs[i]);
                    l.m[l.r as usize] = snf.transvarvubcoefs[i];
                    l.r += 1;
                } else {
                    sumn2mc2le = sum_qd(sumn2mc2le, snf.transvarvubcoefs[i]);
                }
            }
            // Var is in C2.
            1 => {
                sumc2 = sum_qd(sumc2, snf.transvarvubcoefs[i]);
            }
            // Var is in C1.
            3 => {
                if scip.set.is_gt(snf.transvarvubcoefs[i], lambda) {
                    l.m[l.r as usize] = snf.transvarvubcoefs[i];
                    l.r += 1;
                    l.mp = crate::def::min(l.mp, snf.transvarvubcoefs[i]);
                } else {
                    sumc1le = sum_qd(sumc1le, snf.transvarvubcoefs[i]);
                }
            }
            _ => continue,
        }
    }

    if scip.set.is_infinity(l.mp) {
        return None;
    }

    l.big_m = vec![0.0; (l.r + 1) as usize];

    let mut tmp = sum_qq(sumc1le, sumn2mc2le);
    l.ml = crate::def::min(lambda, tmp.to_dbl());
    tmp = sum_qd(sumc2, snf.transrhs);
    l.d1 = tmp.to_dbl();
    tmp = sum_qq(tmp, sumn2mc2gt);
    tmp = sum_qq(tmp, sumn2mc2le);
    l.d2 = tmp.to_dbl();

    let r = l.r as usize;
    crate::sort::sort_down(&mut l.m[..r], (), crate::sort::cmp_real);

    // Compute M[i] = sum_{i \in [1,r]} m[i] where m[*] is sorted decreasingly
    // and M[0] = 0.
    let mut tmp = Quad::new(0.0);
    for i in 0..r {
        l.big_m[i] = tmp.to_dbl();
        tmp = sum_qd(tmp, l.m[i]);
    }

    l.big_m[r] = tmp.to_dbl();

    l.t = match crate::sort::sortedvec_find(&l.m[..r], |a, b| -crate::sort::cmp_real(a, b), l.mp) {
        Ok(p) | Err(p) => p as i32,
    };

    // Compute t largest index sucht that m_t = mp; note that
    // liftingdata->m[t-1] == mp due to zero based indexing of liftingdata->m.
    l.t += 1;
    while l.t < l.r && l.m[l.t as usize] == l.mp {
        l.t += 1;
    }

    l.lambda = lambda;

    Some(l)
}

/// `generateLiftedFlowCoverCut`: store the simple lifted flowcover cut
/// defined by the given data in the given arrays; the array for storing the
/// cut coefficients must be all zeros; returns whether the cut was
/// successfully generated.
#[allow(clippy::too_many_arguments)]
fn generate_lifted_flow_cover_cut(
    scip: &mut Scip,
    snf: &SnfRelaxation,
    aggrrow: &AggrRow,
    flowcoverstatus: &[i32],
    lambda: f64,
    cutcoefs: &mut [f64],
    cutrhs: &mut f64,
    cutinds: &mut [i32],
    nnz: &mut i32,
) -> bool {
    let Some(l) = compute_lifting_data(scip, snf, flowcoverstatus, lambda) else {
        return false;
    };

    let mut rhs = Quad::new(l.d1);

    *nnz = 0;

    let push = |cutinds: &mut [i32], cutcoefs: &mut [f64], nnz: &mut i32, ind: i32, coef: f64| {
        cutinds[*nnz as usize] = ind;
        cutcoefs[ind as usize] = coef;
        *nnz += 1;
    };

    for i in 0..snf.ntransvars as usize {
        let s = (snf.transvarcoefs[i] + 1) + (flowcoverstatus[i] + 1) / 2;

        match s {
            // Var is in N2 \ C2.
            0 => {
                if scip.set.is_gt(snf.transvarvubcoefs[i], lambda) {
                    // Var is in L-.
                    if snf.origbinvars[i] != -1 {
                        push(cutinds, cutcoefs, nnz, snf.origbinvars[i], -lambda);
                    } else {
                        rhs = sum_qd(rhs, lambda);
                    }
                } else {
                    // Var is in L--.
                    if snf.origcontvars[i] != -1 && snf.aggrcoefscont[i] != 0.0 {
                        push(cutinds, cutcoefs, nnz, snf.origcontvars[i], -snf.aggrcoefscont[i]);
                    }

                    if snf.origbinvars[i] != -1 && snf.aggrcoefsbin[i] != 0.0 {
                        push(cutinds, cutcoefs, nnz, snf.origbinvars[i], -snf.aggrcoefsbin[i]);
                    }

                    rhs = sum_qd(rhs, snf.aggrconstants[i]);
                }
            }
            // Var is in C2.
            1 => {
                if snf.origbinvars[i] != -1 {
                    let liftedbincoef = evaluate_lifting_function(scip, &l, snf.transvarvubcoefs[i]);
                    if liftedbincoef != 0.0 {
                        push(cutinds, cutcoefs, nnz, snf.origbinvars[i], -liftedbincoef);
                        rhs = sum_qd(rhs, -liftedbincoef);
                    }
                }
            }
            // Var is in N1 \ C1.
            2 => {
                let (alpha, beta) = get_alpha_and_beta(scip, &l, snf.transvarvubcoefs[i]);

                if alpha == 1 {
                    if snf.origcontvars[i] != -1 && snf.aggrcoefscont[i] != 0.0 {
                        push(cutinds, cutcoefs, nnz, snf.origcontvars[i], snf.aggrcoefscont[i]);
                    }

                    let binvarcoef = sum_dd(snf.aggrcoefsbin[i], -beta);
                    if snf.origbinvars[i] != -1 {
                        let tmp = binvarcoef.to_dbl();
                        if tmp != 0.0 {
                            push(cutinds, cutcoefs, nnz, snf.origbinvars[i], tmp);
                        }
                    } else {
                        rhs = sum_qq(rhs, -binvarcoef);
                    }

                    rhs = sum_qd(rhs, -snf.aggrconstants[i]);
                }
            }
            // Var is in C1.
            3 => {
                let mut bincoef = snf.aggrcoefsbin[i];
                let mut constant = snf.aggrconstants[i];

                if snf.origbinvars[i] != -1 && scip.set.is_gt(snf.transvarvubcoefs[i], lambda) {
                    // Var is in C++.
                    let tmp = sum_dd(snf.transvarvubcoefs[i], -lambda);
                    let mut tmp2 = sum_qd(tmp, constant);
                    constant = tmp2.to_dbl();
                    tmp2 = sum_qd(tmp, -bincoef);
                    bincoef = -tmp2.to_dbl();
                }

                if snf.origbinvars[i] != -1 && bincoef != 0.0 {
                    push(cutinds, cutcoefs, nnz, snf.origbinvars[i], bincoef);
                }

                if snf.origcontvars[i] != -1 && snf.aggrcoefscont[i] != 0.0 {
                    push(cutinds, cutcoefs, nnz, snf.origcontvars[i], snf.aggrcoefscont[i]);
                }

                rhs = sum_qd(rhs, -constant);
            }
            _ => panic!("invalid flow cover status"),
        }
    }

    {
        for i in 0..aggrrow.nrows as usize {
            let slackcoef = aggrrow.rowweights[i] * aggrrow.slacksign[i] as f64;

            // Positive slack was implicitly handled in flow cover separation.
            if slackcoef > 0.0 {
                continue;
            }

            let row = scip.lp.as_ref().unwrap().rows[aggrrow.rowsinds[i]];

            // Add the slack's definition multiplied with its coefficient to the
            // cut.
            var_vec_add_scaled_row_coefs(scip, cutinds, cutcoefs, nnz, row, -aggrrow.rowweights[i]);

            // Retrieve sides of row.
            let r = &scip.rows[row];
            let mut rowlhs = r.lhs - r.constant;
            let mut rowrhs = r.rhs - r.constant;

            if r.integral {
                rowrhs = scip.set.floor(rowrhs);
                rowlhs = scip.set.ceil(rowlhs);
            }

            let mut slackub = rowrhs - rowlhs;

            // Move slack's constant to the right hand side, and add lambda to
            // the right hand side if the upper bound of the slack is larger than
            // lambda, since then an artifical binary variable for the slack
            // would get coefficient -lambda.
            if aggrrow.slacksign[i] == 1 {
                // a*x + c + s == rhs  =>  s == - a*x - c + rhs: move a^_r * (rhs -
                // c) to the right hand side.
                let rhsslack = rowrhs - crate::lp::row_get_min_activity(scip, row);
                slackub = -aggrrow.rowweights[i] * crate::def::min(rhsslack, slackub);

                if scip.set.is_ge(slackub, lambda) {
                    rhs = sum_qd(rhs, lambda);
                }

                rhs = sum_qd(rhs, -aggrrow.rowweights[i] * rowrhs);
            } else {
                // a*x + c - s == lhs  =>  s == a*x + c - lhs: move a^_r * (c - lhs)
                // to the right hand side.
                let lhsslack = crate::lp::row_get_max_activity(scip, row) - rowlhs;
                slackub = aggrrow.rowweights[i] * crate::def::min(lhsslack, slackub);

                if scip.set.is_ge(slackub, lambda) {
                    rhs = sum_qd(rhs, lambda);
                }

                rhs = sum_qd(rhs, -aggrrow.rowweights[i] * rowlhs);
            }
        }
    }

    *cutrhs = rhs.to_dbl();

    // Relax rhs to zero, if it's very close to 0.
    if *cutrhs < 0.0 && *cutrhs >= -scip.set.epsilon() {
        *cutrhs = 0.0;
    }

    true
}

/// `SCIPcalcFlowCover`: calculates a lifted simple generalized flow cover
/// cut out of the weighted sum of LP rows given by an aggregation row; the
/// aggregation row must not contain non-zero weights for modifiable rows,
/// because these rows cannot participate in the cut. For further details we
/// refer to:
///
/// Gu, Z., Nemhauser, G. L., & Savelsbergh, M. W. (1999). Lifted flow cover
/// inequalities for mixed 0-1 integer programs. Mathematical Programming,
/// 85(3), 439-467.
///
/// Sorts the aggregation row's indices. Returns whether a valid cut was
/// returned.
#[allow(clippy::too_many_arguments)]
pub fn calc_flow_cover(scip: &mut Scip, sol: SolId, postprocess: bool, boundswitch: f64, allowlocal: bool, aggrrow: &mut AggrRow, out: CutOut) -> ScipResult<bool> {
    let nvars = crate::api::get_n_vars(scip) as usize;

    // Get data structures.
    let mut transvarflowcoverstatus = vec![0; nvars];
    let mut snf = SnfRelaxation::new(nvars);

    let (success, localbdsused) = {
        let AggrRow { vals, rhs, inds, nnz, .. } = aggrrow;
        construct_snf_relaxation(scip, sol, boundswitch, allowlocal, vals, *rhs, inds, *nnz, &mut snf)
    };

    if !success {
        return Ok(false);
    }

    *out.cutislocal = aggrrow.local || localbdsused;

    let (_, _, lambda, success) = get_flow_cover(scip, &snf, &mut transvarflowcoverstatus);

    if !success {
        return Ok(false);
    }

    let mut tmpcoefs = vec![0.0; nvars];

    let mut success = generate_lifted_flow_cover_cut(scip, &snf, aggrrow, &transvarflowcoverstatus, lambda, &mut tmpcoefs, out.cutrhs, out.cutinds, out.cutnnz);

    if success {
        if postprocess {
            success = postprocess_cut(scip, *out.cutislocal, out.cutinds, &mut tmpcoefs, out.cutnnz, out.cutrhs);
        } else {
            let mut rhs = Quad::new(*out.cutrhs);
            success = !remove_zeros(scip, scip.set.sumepsilon(), *out.cutislocal, &mut tmpcoefs, &mut rhs, out.cutinds, out.cutnnz);
            *out.cutrhs = rhs.to_dbl();
        }

        if success {
            // Store cut sparse and calculate efficacy.
            for i in 0..*out.cutnnz as usize {
                let j = out.cutinds[i] as usize;
                out.cutcoefs[i] = tmpcoefs[j];
            }

            if let Some(e) = out.cutefficacy {
                *e = calc_efficacy(scip, sol, out.cutcoefs, *out.cutrhs, out.cutinds, *out.cutnnz);
            }

            if let Some(r) = out.cutrank {
                *r = aggrrow.rank + 1;
            }
        }
    }

    Ok(success)
}

/*
 * Knapsack cover
 */

/// `cutsTransformKnapsackCover`: relax the row to a possibly fractional
/// knapsack row containing no integer or continuous variables and only
/// having positive coefficients for binary variables. General integer and
/// continuous variables are complemented with variable or simple bounds such
/// that their coefficient becomes positive and then it is relaxed to zero.
/// All remaining binary variables are complemented with simple upper or
/// lower bounds such that their coefficient becomes positive.
///
/// Returns whether the row could successfully be transformed into a
/// knapsack constraint: FALSE in case a continuous or general integer
/// variable is unbounded in the required direction.
#[allow(clippy::too_many_arguments)]
fn cuts_transform_knapsack_cover(
    scip: &mut Scip,
    sol: SolId,
    allowlocal: bool,
    cutcoefs: &mut [Quad],
    cutrhs: &mut Quad,
    cutinds: &mut [i32],
    nnz: &mut i32,
    varsign: &mut [i32],
    boundtype: &mut [i32],
    localbdsused: &mut bool,
) -> bool {
    use crate::types::VarType;

    // Allocate temporary memory to store best bounds and bound types.
    let mut bestbds = vec![0.0; 2 * (*nnz as usize)];

    // Start with continuous variables, because using variable bounds can
    // affect the untransformed binary variables, and these changes have to be
    // incorporated in the transformation of the binary variables (binary
    // variables have the smallest problem indices!).
    crate::sort::sort_down(&mut cutinds[..*nnz as usize], (), crate::sort::cmp_int);

    let firstnonbinvar = crate::api::get_n_bin_vars(scip);

    // Determine best bounds for the continuous and general integer variables
    // such that they will have a positive coefficient in the transformation.
    let mut i = 0;
    while i < *nnz as usize && cutinds[i] >= firstnonbinvar {
        let v = cutinds[i] as usize;
        let coef = cutcoefs[v];
        let var = get_vars(scip)[v];
        let usevbds = if scip.vars[var].vartype == VarType::Continuous { 1 } else { 0 };

        if coef.to_dbl() > 0.0 {
            // Find closest lower bound in standard lower bound or variable lower
            // bound for continuous variable so that it will have a positive
            // coefficient.
            (bestbds[i], boundtype[i]) = find_best_lb(scip, var, sol, usevbds, allowlocal);

            // Cannot transform into knapsack.
            if scip.set.is_infinity(-bestbds[i]) {
                return false;
            }

            varsign[i] = 1;
        } else if coef.to_dbl() < 0.0 {
            // Find closest upper bound in standard upper bound or variable upper
            // bound for continuous variable so that it will have a positive
            // coefficient.
            (bestbds[i], boundtype[i]) = find_best_ub(scip, var, sol, usevbds, allowlocal);

            // Cannot transform into knapsack.
            if scip.set.is_infinity(bestbds[i]) {
                return false;
            }

            varsign[i] = -1;
        }
        i += 1;
    }

    // Remember start of integer variables in the aggrrow.
    let aggrrowbinstart = i;

    // Perform bound substitution for continuous variables.
    for i in 0..aggrrowbinstart {
        let v = cutinds[i];
        perform_bound_substitution(scip, cutinds, cutcoefs, cutrhs, nnz, varsign[i], boundtype[i], bestbds[i], v, localbdsused);

        // Relax non-binary coefficient to zero after bound substitution.
        cutcoefs[v as usize] = Quad::new(0.0);
    }

    // Remove non-binary variables because their coefficients have been set to
    // zero after bound substitution.
    if aggrrowbinstart != 0 {
        *nnz -= aggrrowbinstart as i32;
        cutinds.copy_within(aggrrowbinstart..aggrrowbinstart + *nnz as usize, 0);
    }

    // After doing bound substitution of non-binary vars, some coefficients of
    // binary vars might have changed, so here we remove the ones that became 0
    // if any; also, we need that all remaining binary vars have positive
    // coefficients, thus we perform bound substitution with simple bounds
    // (i.e. complementing) to achieve this.
    let mut i = 0;
    while i < *nnz as usize {
        let v = cutinds[i] as usize;
        let coef = cutcoefs[v];
        let var = get_vars(scip)[v];
        let setzero;

        // Due to variable bound usage for bound substitution of continuous
        // variables cancellation may have occurred.
        if epsz(coef.to_dbl(), QUAD_EPSILON) {
            // Do not increase i, since last element is copied to the i-th
            // position.
            setzero = true;
        } else {
            // Perform bound substitution.
            if coef.to_dbl() < 0.0 {
                let bestub;
                (bestub, boundtype[i]) = find_best_ub(scip, var, sol, 0, allowlocal);

                if scip.set.is_zero(bestub) {
                    // Binary variable is fixed to zero.
                    setzero = true;
                    *localbdsused = *localbdsused || boundtype[i] == -2;
                } else {
                    varsign[i] = -1;

                    perform_bound_substitution_simple(cutcoefs, cutrhs, boundtype[i], bestub, v as i32, localbdsused);
                    cutcoefs[v] = -coef;
                    setzero = false;
                }
            } else {
                let bestlb;
                (bestlb, boundtype[i]) = find_best_lb(scip, var, sol, 0, allowlocal);

                if !scip.set.is_zero(bestlb) {
                    // Binary variable is fixed to one.
                    perform_bound_substitution_simple(cutcoefs, cutrhs, boundtype[i], bestlb, v as i32, localbdsused);
                    setzero = true;
                } else {
                    varsign[i] = 1;
                    setzero = false;
                }
            }
        }

        // Increase i or remove zero coefficient (i.e. var with 0 coef) by
        // shifting last nonzero to current position.
        if setzero {
            cutcoefs[v] = Quad::new(0.0);
            *nnz -= 1;
            cutinds[i] = cutinds[*nnz as usize];
        } else {
            i += 1;
        }
    }

    // Relax rhs to zero if it is close to but slightly below zero.
    if cutrhs.to_dbl() < 0.0 && cutrhs.to_dbl() >= -scip.set.epsilon() {
        *cutrhs = Quad::new(0.0);
    }

    true
}

/// `computeInitialKnapsackCover`: determines the initial cover for the
/// given (fractional) knapsack row; fills `coverstatus` (per row position),
/// `coverpos` and `covervals`, sets `coversize` and `coverweight` (the sum
/// of the coefficient values of variables in the cover); returns whether a
/// cover exists.
#[allow(clippy::too_many_arguments)]
fn compute_initial_knapsack_cover(
    scip: &Scip,
    sol: SolId,
    cutcoefs: &[Quad],
    cutinds: &[i32],
    cutrhs: f64,
    cutnnz: i32,
    varsign: &[i32],
    coverstatus: &mut [i32],
    coverpos: &mut [i32],
    covervals: &mut [f64],
    coversize: &mut i32,
    coverweight: &mut Quad,
) -> bool {
    *coverweight = Quad::new(0.0);
    *coversize = 0;
    let mut j = cutnnz - 1;
    let vars = get_vars(scip);

    for k in 0..cutnnz as usize {
        let v = cutinds[k] as usize;
        let coef = cutcoefs[v];
        let mut solval = get_sol_val(scip, sol, vars[v]);

        if varsign[k] == -1 {
            solval = 1.0 - solval;
        }

        if scip.set.is_feas_eq(solval, 1.0) {
            // Every variable with solution value 1 is forced into the cover.
            coverpos[*coversize as usize] = k as i32;
            covervals[*coversize as usize] = coef.to_dbl();
            coverstatus[k] = 1;
            *coversize += 1;
            *coverweight = sum_qq(*coverweight, coef);
        } else {
            coverpos[j as usize] = k as i32;
            covervals[j as usize] = solval * coef.to_dbl();
            coverstatus[k] = 0;
            j -= 1;
        }
    }

    // Use these two arrays to sort the variables by decreasing contribution
    // and pick them greedily in the while loop below until they are a cover.
    // Since the cover does not need to be minimal we do not need to remove any
    // of the variables with a high activity contribution even if they are not
    // necessary after picking the last variable.
    let (cs, n) = (*coversize as usize, cutnnz as usize);
    crate::sort::sort_down(&mut covervals[cs..n], (&mut coverpos[cs..n],), crate::sort::cmp_real);

    // Overwrite covervals with the coefficients of the variables in the cover
    // as we need to sort decreasingly by those again for the lifting.
    while *coversize < cutnnz && scip.set.is_feas_le(coverweight.to_dbl(), cutrhs) {
        let k = coverpos[*coversize as usize] as usize;
        let v = cutinds[k] as usize;
        coverstatus[k] = 1;
        let coef = cutcoefs[v];
        covervals[*coversize as usize] = coef.to_dbl();
        *coverweight = sum_qq(*coverweight, coef);
        *coversize += 1;
    }

    // There is no cover.
    !(scip.set.is_feas_le(coverweight.to_dbl(), cutrhs) || *coversize == 0)
}

/// `prepareLiftingData`: prepares the data needed to evaluate the lifting
/// function; on output `covervals` stores the running sum of S^-(*) values
/// and variables in C^- have `coverstatus` -1 (C^+ keep 1, all variables
/// outside the cover 0); returns the reciprocal value of \bar{a} and the
/// size of C^+.
#[allow(clippy::too_many_arguments)]
fn prepare_lifting_data(
    scip: &Scip,
    cutcoefs: &[Quad],
    cutinds: &[i32],
    cutrhs: Quad,
    coverpos: &mut [i32],
    coversize: i32,
    coverweight: Quad,
    covervals: &mut [f64],
    coverstatus: &mut [i32],
) -> (Quad, i32) {
    let cs = coversize as usize;

    // Now compute \bar{a}, the unique rational number such that for the cover
    // C it holds that b = \sum_{a_i \in C} \min(\bar{a}, a_i). For that we
    // need to sort by decreasing coefficients of the variables in the cover.
    // After the sorting the covervals array is free to be reused.
    crate::sort::sort_down(&mut covervals[..cs], (&mut coverpos[..cs],), crate::sort::cmp_real);

    // Now follows Algorithm 1 in the paper to compute \bar{a}.

    // Set \bar{a} = l_1.
    let mut abar = cutcoefs[cutinds[coverpos[0] as usize] as usize];
    let mut sigma = sum_qq(coverweight, -cutrhs);

    for k in 1..cs {
        // Load next coefficient l_{k+1} in sorted order of cover.
        let lkplus1 = cutcoefs[cutinds[coverpos[k] as usize] as usize];

        // Let \delta = \bar{a} - l_{k+1} and compute k * \delta.
        let mut kdelta = sum_qq(abar, -lkplus1);
        kdelta = prod_qd(kdelta, k as f64);

        // Set tmp = k * \delta - \sigma to check condition k * \delta < \sigma
        // by tmp < 0.
        let tmp = sum_qq(kdelta, -sigma);
        if tmp.to_dbl() < 0.0 {
            // Set \bar{a} = l_{k+1} and \sigma = \sigma - k*\delta.
            abar = lkplus1;
            sigma = sum_qq(sigma, -kdelta);
        } else {
            // Set \bar{a} = \bar{a} - \sigma / k and \sigma = 0; break.
            let minusoneoverk = -1.0 / k as f64;
            sigma = prod_qd(sigma, minusoneoverk);
            abar = sum_qq(abar, sigma);
            sigma = Quad::new(0.0);
            break;
        }
    }

    if sigma.to_dbl() > 0.0 {
        let oneoverc = 1.0 / coversize as f64;
        abar = prod_qd(cutrhs, oneoverc);
    }

    // Now we partition C into C^+ and C^-, where C^+ are all the elements of C
    // whose weight is strictly larger than \bar{a} and C^- the rest. If a_i
    // are the weights of the elements in C, let a_i^- = min(a_i, \bar{a}) We
    // also compute S^-(h) = sum of the h largest a_i^- and store S^-(h+1) in
    // in covervals[h], for k = 0, ..., coversize - 1 (S^-(0) = 0 so it doesn't
    // need to be stored; we use S to compute the lifted cut, see below) we
    // remember which elements of C^- in coverstatus, so that element in C^+
    // have coverstatus 1 and elements in C^- have coverstatus -1 (elements not
    // in C have coverstatus 0).
    let mut tmp = Quad::new(0.0);
    let mut cplussize = 0;
    for k in 0..cs {
        let coef = cutcoefs[cutinds[coverpos[k] as usize] as usize];
        let coefminusabar = sum_qq(coef, -abar);
        if coefminusabar.to_dbl() > 0.0 {
            // Coefficient is in C^+ because it is greater than \bar{a} and
            // contributes only \bar{a} to the sum.
            tmp = sum_qq(tmp, abar);

            // Rather be on the safe side in numerical corner cases and relax the
            // coefficient to exactly \bar{a}. In that case the coefficient is
            // not treated as in C^+ but as being <= \bar{a} and therefore in
            // C^-.
            if coefminusabar.to_dbl() > scip.set.feastol() {
                cplussize += 1;
            } else {
                coverstatus[coverpos[k] as usize] = -1;
            }
        } else {
            // Coefficient is in C^- because it is smaller or equal to \bar{a}.
            coverstatus[coverpos[k] as usize] = -1;
            tmp = sum_qq(tmp, coef);
        }
        covervals[k] = tmp.to_dbl();
    }

    // Set abar to its reciprocal for faster computation of the lifting
    // coefficients.
    abar = crate::quad::div_dq(1.0, abar);

    (abar, cplussize)
}

/// `evaluateLiftingFunctionKnapsack`: evaluate the lifting function based
/// on the given values; `abar` is the reciprocal value of \bar{a},
/// `covervals` the running sum of S^-(*) values; updates `scale` to
/// integrality when a fractional value is returned.
fn evaluate_lifting_function_knapsack(scip: &Scip, x: Quad, abar: Quad, covervals: &[f64], coversize: i32, cplussize: i32, scale: &mut f64) -> f64 {
    // The lifted value is at least the coeficient (a_k) divided by \bar{a}
    // because the largest value contributed to the running sum stored in C is
    // \bar{a} therefore we start the search for the correct h at floor(a_k /
    // \bar{a}).
    let mut hfrac = crate::quad::prod_qq(x, abar);

    // If the coefficient is below \bar{a}, i.e. a / \bar{a} < 1 then g(a_k) =
    // 0, otherwise g(a_k) > 0.
    if hfrac.to_dbl() < 1.0 {
        return 0.0;
    }

    // We perform h = MIN(h, coversize) in floating-point first because on some
    // instances h was seen to exceed the range of int.
    let hreal = scip.set.floor(hfrac.to_dbl());
    let mut h = if hreal > coversize as f64 { coversize } else { hreal as i32 };

    hfrac = sum_qd(hfrac, -h as f64);

    let mut cutcoef;
    if h < cplussize && hfrac.to_dbl().abs() <= QUAD_EPSILON {
        // cutcoef can be increased by 0.5 because it is a multiple of \bar{a}
        // (This is the first non-dominated lifting function presented in the
        // paper).
        cutcoef = 0.5;
        *scale = 2.0;
    } else {
        cutcoef = 0.0;
    }

    // Decrease by one to make sure rounding errors or coefficients that are
    // larger than the right hand side by themselves did not push h too far.
    h -= 1;

    // Now increase coefficient to its lifted value based on its size relative
    // to the S^- values. The coefficient a_i is lifted to the unique integer h
    // such that S^-(h) < a_i <= S^-(h+1).
    // (todo: variables that have a coefficient above the right hand side can
    // get an arbitrarily large coefficient but can also be trivially fixed
    // using the base row. Currently they get the coefficient |C| which is 1
    // above the right hand side in the cover cut so that they can still be
    // trivially fixed by propagating the cover cut. We do not want to apply
    // fixings here though because the LP should stay flushed during
    // separation. Possibly add a parameter to return additional fixings to
    // the caller of the SCIPcalc*() functions in here and the caller can add
    // them as cuts to the sepastore or we add them to the sepastore here?)
    while h < coversize {
        // Recall: covervals[h] = S^-(h+1).
        let tmp = sum_qd(x, -covervals[h as usize]);
        // Compare with standard epsilon tolerance since computation involves
        // abar, which is computed like an activity.
        if !scip.set.is_positive(tmp.to_dbl()) {
            break;
        }

        h += 1;
    }

    cutcoef += h as f64;

    // The lifted coefficient is h increased possibly by 0.5 for the case
    // checked above.
    cutcoef
}

/// `SCIPcalcKnapsackCover`: calculates a lifted knapsack cover cut out of
/// the weighted sum of LP rows given by an aggregation row; the aggregation
/// row must not contain non-zero weights for modifiable rows, because these
/// rows cannot participate in the cut. For further details we refer to:
///
/// Letchford, A. N., & Souli, G. (2019). On lifted cover inequalities: A new
/// lifting procedure with unusual properties. Operations Research Letters,
/// 47(2), 83-87.
///
/// `out.cutefficacy` must be given; returns whether a valid cut was
/// returned.
pub fn calc_knapsack_cover(scip: &mut Scip, sol: SolId, allowlocal: bool, aggrrow: &AggrRow, out: CutOut) -> ScipResult<bool> {
    if aggrrow.nnz == 0 {
        return Ok(false);
    }

    for k in 0..aggrrow.nrows as usize {
        // Cannot handle negative slack variables.
        if aggrrow.rowweights[k] * (aggrrow.slacksign[k] as f64) < 0.0 {
            return Ok(false);
        }
    }

    // Allocate temporary memory.
    let nvars = crate::api::get_n_vars(scip) as usize;
    let mut varsign = vec![0; nvars];
    let mut boundtype = vec![0; nvars];
    let mut coverstatus = vec![0; nvars];
    let mut covervals = vec![0.0; nvars];
    let mut coverpos = vec![0; nvars];
    let mut tmpinds = vec![0; nvars];
    let mut tmpcoefs = vec![Quad::new(0.0); nvars];

    // Initialize cut with aggregation.
    let mut nnz = aggrrow.nnz;
    let mut rhs = aggrrow.rhs;

    tmpinds[..nnz as usize].copy_from_slice(&aggrrow.inds[..nnz as usize]);

    for tmpind in &tmpinds[..nnz as usize] {
        let j = *tmpind as usize;
        let mut coef = aggrrow.vals[j];
        coef.hi = nonzero(coef.hi);
        tmpcoefs[j] = coef;
    }

    // Transform aggregated row into a (fractional, i.e. with possibly
    // fractional weights) knapsack constraint. Uses simple or variable lower
    // or upper bounds to relax out continuous and general integers so that
    // only binary variables remain and complements those such that they have a
    // positive coefficient.
    let mut local = aggrrow.local;
    let transformed = cuts_transform_knapsack_cover(
        scip,
        sol,
        allowlocal,
        &mut tmpcoefs,
        &mut rhs,
        &mut tmpinds,
        &mut nnz,
        &mut varsign,
        &mut boundtype,
        &mut local,
    );

    if !transformed {
        return Ok(false);
    }

    let mut coversize = 0;
    let mut coverweight = Quad::new(0.0);
    if !compute_initial_knapsack_cover(
        scip,
        sol,
        &tmpcoefs,
        &tmpinds,
        rhs.to_dbl(),
        nnz,
        &varsign,
        &mut coverstatus,
        &mut coverpos,
        &mut covervals,
        &mut coversize,
        &mut coverweight,
    ) {
        return Ok(false);
    }

    // By default do not scale the cut.
    let mut scale = 1.0;

    if coversize == 1 {
        // Cover is trivial, return the fixing as cut.
        for k in 0..nnz as usize {
            if coverstatus[k] == 0 {
                tmpcoefs[tmpinds[k] as usize] = Quad::new(0.0);
            } else {
                tmpinds[0] = tmpinds[k];
                varsign[0] = varsign[k];
            }
        }

        nnz = 1;
        let tmp;
        if varsign[0] == -1 {
            rhs = Quad::new(-1.0);
            tmp = Quad::new(-1.0);
        } else {
            rhs = Quad::new(0.0);
            tmp = Quad::new(1.0);
        }

        tmpcoefs[tmpinds[0] as usize] = tmp;
    } else {
        // Compute lifted cover inequality:
        //   sum_{i \in C^-) x_i + sum_{i \in N \ C^-) g(a_i) x_i <= c - 1
        // where g(z) is equal to
        //   - 0 if z is 0 (irrelevant as there shouldn't be element with weight
        //     0 in the knapsack)
        //   - h + 1/2 if z = k * \bar{a} for some integer k \in [1, |C^+| - 1]
        //     and S^-(h) < z <= S^-(h+1) for some h = 0, ..., coversize -1
        //   - h if S^-(h) < z <= S^-(h+1) for some h = 0, ..., coversize -1
        // the function S^- is defined above. Note that S^-(0) = 0 we store the
        // cut coefficients in tmpcoef.

        // Prepare data required to evaluate lifting function.
        let (abar, cplussize) = prepare_lifting_data(scip, &tmpcoefs, &tmpinds, rhs, &mut coverpos, coversize, coverweight, &mut covervals, &mut coverstatus);

        // Compute lifted cover inequality.
        rhs = Quad::new((coversize - 1) as f64);
        let mut k = 0;
        while k < nnz as usize {
            let mut cutcoef;
            if coverstatus[k] == -1 {
                // Variables in C^- get the coefficients 1.
                cutcoef = 1.0;
            } else {
                // Variables is either in C^+ or not in the cover and its
                // coefficient value is computed with the lifing function.
                let coef = tmpcoefs[tmpinds[k] as usize];
                cutcoef = evaluate_lifting_function_knapsack(scip, coef, abar, &covervals, coversize, cplussize, &mut scale);

                // If the coefficient value is zero then remove the nonzero entry
                // and continue.
                if cutcoef == 0.0 {
                    tmpcoefs[tmpinds[k] as usize] = Quad::new(0.0);
                    nnz -= 1;
                    coverstatus[k] = coverstatus[nnz as usize];
                    tmpinds[k] = tmpinds[nnz as usize];
                    varsign[k] = varsign[nnz as usize];
                    continue;
                }
            }

            // Directly undo the complementation before storing back the
            // coefficient.
            if varsign[k] == -1 {
                // Variable was complemented so we have cutcoef * (1-x) = cutcoef -
                // cutcoef * x.Thus we need to adjust the rhs to rhs - cutcoef and
                // flip the sign of cutcoef.
                cutcoef = -cutcoef;
                rhs = sum_qd(rhs, cutcoef);
            }

            tmpcoefs[tmpinds[k] as usize] = Quad::new(cutcoef);

            k += 1;
        }
    }

    // Calculate the efficacy of the computed cut and store the success flag if
    // the efficacy exceeds the one stored in the cutefficacy variable by the
    // caller.
    let efficacy = calc_efficacy_dense_storage_quad(scip, sol, &tmpcoefs, rhs.to_dbl(), &tmpinds, nnz);
    let cutefficacy = out.cutefficacy.unwrap();
    let success = scip.set.is_gt(efficacy, *cutefficacy);

    if success {
        // Return the cut into the given arrays/pointers.
        *out.cutislocal = local;
        *out.cutrhs = scale * rhs.to_dbl();
        *out.cutnnz = nnz;

        // Store cut in given array in sparse representation and clean buffer
        // array.
        for k in 0..nnz as usize {
            let j = tmpinds[k];
            out.cutcoefs[k] = scale * tmpcoefs[j as usize].to_dbl();
            out.cutinds[k] = j;
        }

        // Calculate efficacy again to make sure it matches the coefficients
        // after they where rounded to double values and after the cleanup and
        // postprocessing step was applied.
        *cutefficacy = calc_efficacy(scip, sol, out.cutcoefs, *out.cutrhs, out.cutinds, nnz);

        if let Some(r) = out.cutrank {
            *r = aggrrow.rank + 1;
        }
    }

    Ok(success)
}

/*
 * Strong CG
 */

/// `cutsTransformStrongCG`: transform equation a*x = b; lb <= x <= ub into
/// standard form a'*x' = b, 0 <= x' <= ub'; differs from cutsTransformMIR
/// for continuous variables for which the lower bound must be used when in
/// case their coefficient is positive and the upper bound in case their
/// coefficient is negative. This forces all continuous variable to have a
/// positive coefficient in the transformed row. Returns `(freevariable,
/// localbdsused)`.
fn cuts_transform_strong_cg(scip: &Scip, data: &mut MirData, sol: SolId, boundswitch: f64, allowlocal: bool, t: MirTransform) -> (bool, bool) {
    use crate::types::BoundType;
    let MirTransform { varsign, boundtype } = t;
    let mut freevariable = false;
    let mut localbdsused = false;
    let vars = get_vars(scip);

    // Allocate temporary memory to store best bounds and bound types.
    let mut best = vec![BestBounds::default(); data.nvars as usize];

    'terminate: {
        // Transform the cut, one variable section at a time.
        for s in 0..NSECTIONS {
            let cutindsstart = data.ncutinds as usize;
            let usevbds = data.usevbds[s];

            // Iterate over all nonzeros in the section.
            let mut i = 0;
            while i < data.secnnz[s] as usize {
                let v = data.secindices[s][i];

                // Due to variable bound usage, cancellation may have occurred.
                let coef = data.cutcoefs[v as usize];
                if epsz(coef.to_dbl(), QUAD_EPSILON) {
                    data.cutcoefs[v as usize] = Quad::new(0.0);
                    data.secnnz[s] -= 1;
                    data.totalnnz -= 1;
                    data.secindices[s][i] = data.secindices[s][data.secnnz[s] as usize];
                    // Do not increase the index.
                    continue;
                }

                let cutindex = data.ncutinds as usize;
                let var = vars[v as usize];
                let b = &mut best[cutindex];

                // For continuous variables, we must choose the bound substitution
                // so that they become positive in the cut.
                if !data.isenfint[s] && !data.isimplint[s] {
                    if coef.to_dbl() > 0.0 {
                        // Find closest lower bound in standard lower bound or
                        // variable lower bound for continuous variable so that it
                        // will have a positive coefficient.
                        (b.bestlb, _, b.bestlbtype) = find_mir_best_bound(scip, var, sol, data, usevbds, allowlocal, false);

                        // Cannot create transformation for strongcg cut.
                        if scip.set.is_infinity(-b.bestlb) {
                            freevariable = true;
                            break 'terminate;
                        }

                        varsign[cutindex] = 1;
                        b.selectedbound = BoundType::Lower;
                    } else {
                        // Find closest upper bound in standard upper bound or
                        // variable upper bound for continuous variable so that it
                        // will have a positive coefficient.
                        (b.bestub, _, b.bestubtype) = find_mir_best_bound(scip, var, sol, data, usevbds, allowlocal, true);

                        // Cannot create transformation for strongcg cut.
                        if scip.set.is_infinity(b.bestub) {
                            freevariable = true;
                            break 'terminate;
                        }

                        varsign[cutindex] = -1;
                        b.selectedbound = BoundType::Upper;
                    }
                } else if data.isimplint[s] {
                    // For implied integers, we still prefer to choose the bound
                    // substitution that makes them positive, but if we cannot
                    // manage to do so it is not an error, because we can still
                    // treat them as integer variables.
                    (b.bestlb, _, b.bestlbtype) = find_mir_best_bound(scip, var, sol, data, usevbds, allowlocal, false);
                    (b.bestub, _, b.bestubtype) = find_mir_best_bound(scip, var, sol, data, usevbds, allowlocal, true);

                    let lowerinf = scip.set.is_infinity(-b.bestlb);
                    let upperinf = scip.set.is_infinity(b.bestub);
                    let positive = coef.to_dbl() > 0.0;

                    if lowerinf && upperinf {
                        // We found a free variable in the row with non-zero
                        // coefficient -> MIR row can't be transformed in standard
                        // form.
                        freevariable = true;
                        break 'terminate;
                    }

                    // Preferably, choose bound that makes value positive.
                    if (positive && lowerinf) || (!positive && !upperinf) {
                        varsign[cutindex] = -1;
                        b.selectedbound = BoundType::Upper;
                    } else {
                        varsign[cutindex] = 1;
                        b.selectedbound = BoundType::Lower;
                    }
                } else {
                    // For explicit integers, we have no restrictions.
                    *b = determine_best_bounds(scip, var, sol, data, boundswitch, usevbds, allowlocal, false, false, None, None);
                    if b.freevariable {
                        freevariable = true;
                        break 'terminate;
                    }
                }

                data.cutinds[cutindex] = v;
                data.ncutinds += 1;
                i += 1;
            }

            // Perform bound substitution for all nonzeros in the section.
            for i in cutindsstart..data.ncutinds as usize {
                let v = data.cutinds[i];

                let bestbnd = if best[i].selectedbound == BoundType::Lower {
                    // Use lower bound as transformation bound: x'_j := x_j - lb_j.
                    boundtype[i] = best[i].bestlbtype;
                    varsign[i] = 1;
                    best[i].bestlb
                } else {
                    // Use upper bound as transformation bound: x'_j := ub_j - x_j.
                    boundtype[i] = best[i].bestubtype;
                    varsign[i] = -1;
                    best[i].bestub
                };

                do_mir_bound_substitution(scip, data, varsign[i], boundtype[i], bestbnd, v, &mut localbdsused);
            }
        }

        // Relax rhs to zero if it is close to.
        if data.cutrhs.to_dbl() < 0.0 && data.cutrhs.to_dbl() >= -scip.set.epsilon() {
            data.cutrhs = Quad::new(0.0);
        }
    }

    // TERMINATE: if we terminate early, we need to make sure all the zeros in
    // the cut coefficient array are cancelled.
    if freevariable {
        data.ncutinds = 0;
        for j in 0..NSECTIONS {
            for k in 0..data.secnnz[j] as usize {
                data.cutinds[data.ncutinds as usize] = data.secindices[j][k];
                data.ncutinds += 1;
            }
        }
    }

    (freevariable, localbdsused)
}

/// `cutsSubstituteStrongCG`: substitute aggregated slack variables:
///
/// The coefficient of the slack variable s_r is equal to the row's weight
/// times the slack's sign, because the slack variable only appears in its
/// own row: a'_r = scale * weight[r] * slacksign[r].
///
/// Depending on the slacks type (integral or continuous), its coefficient
/// in the cut calculates as follows:
///   integers:   a_r = a~_r = (k + 1) * down(a'_r)      , if f_r <= f0
///               a_r = a~_r = (k + 1) * down(a'_r) + p_r, if f_r >  f0
///   continuous: a_r = a~_r = 0                         , if a'_r >= 0
///               a_r = a~_r = a'_r/(1 - f0)             , if a'_r <  0
///
/// Substitute a_r * s_r by adding a_r times the slack's definition to the
/// cut.
#[allow(clippy::too_many_arguments)]
fn cuts_substitute_strong_cg(
    scip: &Scip,
    weights: &[f64],
    slacksign: &[i32],
    rowinds: &[i32],
    nrowinds: i32,
    scale: f64,
    cutcoefs: &mut [Quad],
    cutrhs: &mut Quad,
    cutinds: &mut [i32],
    nnz: &mut i32,
    f0: Quad,
    k: f64,
) {
    use crate::quad::{div_dd, div_dq, eps_floor_q, prod_qq};

    let mut onedivoneminusf0 = sum_qd(-f0, 1.0);
    onedivoneminusf0 = div_dq(1.0, onedivoneminusf0);

    let rows = &scip.lp.as_ref().unwrap().rows;

    for i in 0..nrowinds as usize {
        let r = rowinds[i] as usize;
        let row = rows[r];
        let rw = &scip.rows[row];

        // Get the slack's coefficient a'_r in the aggregated row.
        let ar = prod_dd(slacksign[i] as f64 * scale, weights[i]);

        // Calculate slack variable's coefficient a_r in the cut.
        let mut cutar;
        if rw.integral {
            // Slack variable is always integral.
            let downar = eps_floor_q(ar, scip.set.epsilon());
            let fr = sum_qq(ar, -downar);

            if scip.set.is_le(fr.to_dbl(), f0.to_dbl()) {
                // a_r
                cutar = downar;
            } else {
                cutar = sum_qq(fr, -f0);
                cutar = prod_qd(cutar, k);
                cutar = prod_qq(cutar, onedivoneminusf0);
                let pr = scip.set.ceil(cutar.to_dbl());
                cutar = div_dd(pr, k + 1.0);
                cutar = sum_qq(cutar, downar);
            }
        } else {
            // Slack variable is continuous: slack can be ignored, because its
            // coefficient is reduced to 0.0.
            continue;
        }

        // If the coefficient was reduced to zero, ignore the slack variable.
        if epsz(cutar.to_dbl(), QUAD_EPSILON) {
            continue;
        }

        // Depending on the slack's sign, we have
        //   a*x + c + s == rhs  =>  s == - a*x - c + rhs,  or  a*x + c - s == lhs  =>  s == a*x + c - lhs
        // substitute a_r * s_r by adding a_r times the slack's definition to the
        // cut.
        let mul = -slacksign[i] as f64 * cutar.to_dbl();

        // Add the slack's definition multiplied with a_j to the cut.
        var_vec_add_scaled_row_coefs_quad(scip, cutinds, cutcoefs, nnz, row, mul);

        // Move slack's constant to the right hand side.
        if slacksign[i] == 1 {
            // a*x + c + s == rhs  =>  s == - a*x - c + rhs: move a_r * (rhs - c)
            // to the right hand side.
            let mut rhs = rw.rhs - rw.constant;
            if rw.integral {
                // The right hand side was implicitly rounded down in row
                // aggregation.
                rhs = scip.set.floor(rhs);
            }

            cutar = prod_qd(cutar, rhs);
            *cutrhs = sum_qq(*cutrhs, -cutar);
        } else {
            // a*x + c - s == lhs  =>  s == a*x + c - lhs: move a_r * (c - lhs) to
            // the right hand side.
            let mut lhs = rw.lhs - rw.constant;
            if rw.integral {
                // The left hand side was implicitly rounded up in row aggregation.
                lhs = scip.set.ceil(lhs);
            }

            cutar = prod_qd(cutar, lhs);
            *cutrhs = sum_qq(*cutrhs, cutar);
        }
    }

    // Relax rhs to zero, if it's very close to 0.
    if cutrhs.to_dbl() < 0.0 && cutrhs.to_dbl() >= -scip.set.epsilon() {
        *cutrhs = Quad::new(0.0);
    }
}

/// `SCIPcalcStrongCG`: calculates a strong CG cut out of the weighted sum
/// of LP rows given by an aggregation row; the aggregation row must not
/// contain non-zero weights for modifiable rows, because these rows cannot
/// participate in a strongcg cut; returns whether a valid cut was returned.
#[allow(clippy::too_many_arguments)]
pub fn calc_strong_cg(
    scip: &Scip,
    sol: SolId,
    postprocess: bool,
    boundswitch: f64,
    vartypeusevbds: i32,
    allowlocal: bool,
    minfrac: f64,
    maxfrac: f64,
    scale: f64,
    aggrrow: &AggrRow,
    out: CutOut,
) -> ScipResult<bool> {
    let mut success = false;

    // Determine value from which fractionalities are no longer reliable within
    // tolerance.
    let large = scip.set.get_huge_value() * scip.set.epsilon();

    // Terminate if an integral slack fractionality is unreliable or a negative
    // continuous slack variable is present.
    let lprows = &scip.lp.as_ref().unwrap().rows;
    for i in 0..aggrrow.nrows as usize {
        let integral = scip.rows[lprows[aggrrow.rowsinds[i]]].integral;
        if (integral && (aggrrow.rowweights[i] * scale).abs() > large) || (!integral && aggrrow.rowweights[i] * (aggrrow.slacksign[i] as f64) < 0.0) {
            return Ok(false);
        }
    }

    // Allocate temporary memory.
    let nvars = crate::api::get_n_vars(scip);
    let mut varsign = vec![0; nvars as usize];
    let mut boundtype = vec![0; nvars as usize];

    // Initialize cut data.
    let mut data = mir_data_create(scip, aggrrow, scale, vartypeusevbds);

    *out.cutislocal = aggrrow.local;

    'terminate: {
        if data.totalnnz > 0 {
            // Transform equation  a*x == b, lb <= x <= ub  into standard form
            //   a'*x' == b, 0 <= x' <= ub'
            // (see cutsTransformMIR).
            let t = MirTransform {
                varsign: &mut varsign,
                boundtype: &mut boundtype,
            };
            let (freevariable, localbdsused) = cuts_transform_strong_cg(scip, &mut data, sol, boundswitch, allowlocal, t);

            if freevariable {
                break 'terminate;
            }

            *out.cutislocal = *out.cutislocal || localbdsused;

            let firstcontvar = nvars - crate::api::get_n_cont_vars(scip);

            // Terminate if an integral coefficient fractionality is unreliable.
            let mut i = data.ncutinds - 1;
            while i >= 0 && data.cutinds[i as usize] < firstcontvar {
                let coef = data.cutcoefs[data.cutinds[i as usize] as usize];
                if coef.to_dbl().abs() > large {
                    break 'terminate;
                }
                i -= 1;
            }
        }

        // Terminate if the side fractionality is unreliable.
        if data.cutrhs.to_dbl().abs() > large {
            break 'terminate;
        }

        // Calculate
        //   - fractionalities  f_0 := b - down(b), f_j := a'_j - down(a'_j)
        //   - integer k >= 1 with 1/(k + 1) <= f_0 < 1/k
        //     (=> k = up(1/f_0) - 1)
        //   - integer 1 <= p_j <= k with f_0 + ((p_j - 1) * (1 - f_0)/k) < f_j <= f_0 + (p_j * (1 - f_0)/k)
        //     (=> p_j = up( (f_j - f_0)/((1 - f_0)/k) ))
        // and derive strong CG cut (see cutsRoundStrongCG).
        let downrhs = crate::quad::eps_floor_q(data.cutrhs, scip.set.epsilon());
        let mut f0 = sum_qq(data.cutrhs, -downrhs);

        if f0.to_dbl() < minfrac || f0.to_dbl() > maxfrac {
            break 'terminate;
        }

        // Renormalize the f0 value.
        f0 = sum_dd(f0.hi, f0.lo);

        let mut tmp = crate::quad::div_dq(1.0, f0);
        tmp = sum_qd(tmp, -1.0);
        let k = scip.set.ceil(tmp.to_dbl());

        data.cutrhs = downrhs;

        if data.totalnnz > 0 {
            cuts_round_strong_cg(scip, &mut data, &varsign, &boundtype, f0, k);
        }

        // Substitute aggregated slack variables (see cutsSubstituteStrongCG).
        {
            let MirData {
                cutcoefs,
                cutrhs,
                cutinds,
                ncutinds,
                ..
            } = &mut data;
            cuts_substitute_strong_cg(
                scip,
                &aggrrow.rowweights,
                &aggrrow.slacksign,
                &aggrrow.rowsinds,
                aggrrow.nrows,
                scale,
                cutcoefs,
                cutrhs,
                cutinds,
                ncutinds,
                f0,
                k,
            );
        }

        // Remove all nearly-zero coefficients from strong CG row and relax the
        // right hand side correspondingly in order to prevent numerical
        // rounding errors.
        {
            let MirData {
                cutcoefs,
                cutrhs,
                cutinds,
                ncutinds,
                ..
            } = &mut data;
            if postprocess {
                success = postprocess_cut_quad(scip, *out.cutislocal, cutinds, cutcoefs, ncutinds, cutrhs);
            } else {
                success = !remove_zeros_quad(scip, scip.set.sumepsilon(), *out.cutislocal, cutcoefs, cutrhs, cutinds, ncutinds);
            }
        }

        if success {
            *out.cutrhs = data.cutrhs.to_dbl();
            *out.cutnnz = data.ncutinds;

            // Store cut in given array in sparse representation and clean buffer
            // array.
            for i in 0..*out.cutnnz as usize {
                let j = data.cutinds[i];
                out.cutcoefs[i] = data.cutcoefs[j as usize].to_dbl();
                out.cutinds[i] = j;
            }

            if let Some(e) = out.cutefficacy {
                *e = calc_efficacy(scip, sol, out.cutcoefs, *out.cutrhs, out.cutinds, *out.cutnnz);
            }

            if let Some(r) = out.cutrank {
                *r = aggrrow.rank + 1;
            }
        }
    }

    Ok(success)
}
