//! The public helpers of `branch_distribution.c` (the variable and row
//! activity distributions distributiondiving scores with). The branching
//! rule itself is not ported: with default settings relpscost always
//! branches first.

use crate::{
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ImplintType, RowId, VarType},
};

/// `SCIPvarCalcDistributionParameters`: calculate the variable's
/// distribution parameters (mean and variance) for the bounds specified in
/// the arguments; special treatment of infinite bounds necessary.
pub fn var_calc_distribution_parameters(scip: &Scip, varlb: f64, varub: f64, vartype: VarType, impltype: ImplintType) -> (f64, f64) {
    let mean;
    let mut variance;

    // Calculate mean and variance of variable uniform distribution before and
    // after branching.
    if scip.set.is_infinity(varub) || scip.set.is_infinity(-varlb) {
        // Variables with infinite bounds are not kept in the row activity
        // variance.
        variance = 0.0;

        if !scip.set.is_infinity(varub) {
            mean = varub;
        } else if !scip.set.is_infinity(-varlb) {
            mean = varlb;
        } else {
            mean = 0.0;
        }
    } else {
        // If the variable is continuous, we assume a continuous uniform
        // distribution, otherwise a discrete one.
        if vartype == VarType::Continuous && impltype == ImplintType::None {
            variance = (varub - varlb) * (varub - varlb);
        } else {
            variance = (varub - varlb + 1.0) * (varub - varlb + 1.0) - 1.0;
        }
        variance /= 12.0;
        mean = (varub + varlb) * 0.5;
    }

    debug_assert!(!scip.set.is_negative(variance));
    (mean, variance)
}

/// `SCIPcalcCumulativeDistribution`: calculates the cumulative distribution
/// P(-infinity <= x <= value) that a normally distributed random variable x
/// takes a value between -infinity and parameter `value`.
pub fn calc_cumulative_distribution(scip: &Scip, mean: f64, variance: f64, value: f64) -> f64 {
    // We need to calculate the standard deviation from the variance.
    debug_assert!(!scip.set.is_negative(variance));
    let std = if scip.set.is_feas_zero(variance) { 0.0 } else { variance.sqrt() };

    // Special treatment for zero variance.
    if scip.set.is_feas_zero(std) {
        if scip.set.is_feas_le(value, mean) {
            return 1.0;
        } else {
            return 0.0;
        }
    }

    // Scale and translate to standard normal distribution. Factor sqrt(2) is
    // needed for SCIPerf() function.
    let normvalue = (value - mean) / (std * std::f64::consts::SQRT_2);

    // Calculate the cumulative distribution function for normvalue. For
    // negative normvalues, we negate the normvalue and use the oddness of the
    // SCIPerf()-function; special treatment for values close to zero.
    if scip.set.is_feas_zero(normvalue) {
        0.5
    } else if normvalue > 0.0 {
        let erfresult = crate::misc::erf(normvalue);
        erfresult / 2.0 + 0.5
    } else {
        let erfresult = crate::misc::erf(-normvalue);
        0.5 - erfresult / 2.0
    }
}

/// `SCIProwCalcProbability`: calculates the probability of satisfying an
/// LP-row under the assumption of uniformly distributed variable values.
pub fn row_calc_probability(scip: &Scip, row: RowId, mu: f64, sigma2: f64, rowinfinitiesdown: i32, rowinfinitiesup: i32) -> f64 {
    let lhs = scip.rows[row].lhs;
    let rhs = scip.rows[row].rhs;

    let mut lhsprob = 1.0;
    let mut rhsprob = 1.0;

    // Use the cumulative distribution if the row contains no variable to
    // repair every infeasibility.
    if !scip.set.is_infinity(rhs) && rowinfinitiesdown == 0 {
        rhsprob = calc_cumulative_distribution(scip, mu, sigma2, rhs);
    }

    // Use the cumulative distribution if the row contains no variable to
    // repair every infeasibility otherwise the row can always be made
    // feasible by increasing activity far enough.
    if !scip.set.is_infinity(-lhs) && rowinfinitiesup == 0 {
        lhsprob = 1.0 - calc_cumulative_distribution(scip, mu, sigma2, lhs);
    }

    // Use centeredness measure for equations; for inequalities, the minimum
    // of the two probabilities is the probability to satisfy the row.
    if scip.set.is_feas_eq(lhs, rhs) {
        let minprobability = if rhsprob <= lhsprob { rhsprob } else { lhsprob };
        let maxprobability = if lhsprob >= rhsprob { lhsprob } else { rhsprob };
        minprobability / maxprobability
    } else if rhsprob <= lhsprob {
        rhsprob
    } else {
        lhsprob
    }
}

/// `SCIPupdateDistributionScore`: update the up- and downscore of a single
/// variable after calculating the impact of branching on a particular row,
/// depending on the chosen score parameter.
pub fn update_distribution_score(scip: &Scip, currentprob: f64, newprobup: f64, newprobdown: f64, upscore: &mut f64, downscore: &mut f64, scoreparam: u8) -> ScipResult {
    // Update up and downscore depending on score parameter.
    match scoreparam {
        b'l' => {
            // 'l'owest cumulative probability.
            if scip.set.is_gt(1.0 - newprobup, *upscore) {
                *upscore = 1.0 - newprobup;
            }
            if scip.set.is_gt(1.0 - newprobdown, *downscore) {
                *downscore = 1.0 - newprobdown;
            }
        }
        b'd' => {
            // Biggest 'd'ifference currentprob - newprob.
            if scip.set.is_gt(currentprob - newprobup, *upscore) {
                *upscore = currentprob - newprobup;
            }
            if scip.set.is_gt(currentprob - newprobdown, *downscore) {
                *downscore = currentprob - newprobdown;
            }
        }
        b'h' => {
            // 'h'ighest cumulative probability.
            if scip.set.is_gt(newprobup, *upscore) {
                *upscore = newprobup;
            }
            if scip.set.is_gt(newprobdown, *downscore) {
                *downscore = newprobdown;
            }
        }
        b'v' => {
            // 'v'otes lowest cumulative probability.
            if scip.set.is_lt(newprobup, newprobdown) {
                *upscore += 1.0;
            } else if scip.set.is_gt(newprobup, newprobdown) {
                *downscore += 1.0;
            }
        }
        b'w' => {
            // Votes highest cumulative probability.
            if scip.set.is_gt(newprobup, newprobdown) {
                *upscore += 1.0;
            } else if scip.set.is_lt(newprobup, newprobdown) {
                *downscore += 1.0;
            }
        }
        _ => {
            log::error!(" ERROR! No branching scheme selected! Exiting  method.");
            return Err(Retcode::InvalidCall);
        }
    }

    Ok(())
}
