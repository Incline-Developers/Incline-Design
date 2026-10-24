//! Port of `presol_domcol.c`: dominated column presolver.
//!
//! This presolver looks for dominance relations between variable pairs.
//! From a dominance relation and certain bound/clock conditions variable
//! fixings can be derived. In addition, a predictive bound strengthening
//! step is possible (off by default).

use std::any::Any;

use crate::{
    api,
    cons::*,
    matrix::{Matrix, matrix_create},
    retcode::ScipResult,
    scip::Scip,
    scip_var,
    types::{PresolId, ScipResultCode, VarType},
    var::var_is_integral,
};

const PRESOL_NAME: &str = "domcol";
const PRESOL_DESC: &str = "dominated column presolver";
/// Priority of the presolver (>= 0: before, < 0: after constraint handlers).
const PRESOL_PRIORITY: i32 = -1000;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_EXHAUSTIVE;

/// Minimal number of pair comparisons.
const DEFAULT_NUMMINPAIRS: i32 = 1024;
/// Maximal number of pair comparisons.
const DEFAULT_NUMMAXPAIRS: i32 = 1048576;
/// Should predictive bound strengthening be applied?
const DEFAULT_PREDBNDSTR: bool = false;
/// Should reductions for continuous variables be carried out?
const DEFAULT_CONTINUOUS_RED: bool = true;

/*
 * Data structures
 */

/// `SCIP_PresolData`: control parameters.
pub struct PresolData {
    /// Minimal number of pair comparisons.
    pub numminpairs: i32,
    /// Maximal number of pair comparisons.
    pub nummaxpairs: i32,
    /// Current number of pair comparisons.
    pub numcurrentpairs: i32,
    /// Flag indicating if predictive bound strengthening should be applied.
    pub predbndstr: bool,
    /// Flag indicating if reductions for continuous variables should be
    /// performed.
    pub continuousred: bool,
}

/// `FIXINGDIRECTION`: type of fixing direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fixingdirection {
    /// Fix variable at lower bound.
    FixAtLb = -1,
    /// Do not fix variable.
    NoFix = 0,
    /// Fix variable at upper bound.
    FixAtUb = 1,
}

/*
 * Local methods
 */

/// The residual activities `(minresactivity, maxresactivity)` of `row`
/// without column `col` (coefficient `coef`), from the given activity
/// sums and infinity counts; the tail shared by
/// `getActivityResidualsUpperBound` and `getActivityResidualsLowerBound`.
#[allow(clippy::too_many_arguments)]
fn residuals_from(scip: &Scip, lb: f64, ub: f64, coef: f64, tmpminact: f64, tmpmaxact: f64, minactinf: i32, maxactinf: i32) -> (f64, f64) {
    let inf = scip.set.infinity();
    let minresactivity;
    let maxresactivity;

    // The coefficient is positive, so the upper bound contributed to the
    // maxactivity and the lower bound to the minactivity.
    if coef >= 0.0 {
        maxresactivity = if scip.set.is_infinity(ub) {
            if maxactinf == 1 { tmpmaxact } else { inf }
        } else if maxactinf > 0 {
            inf
        } else {
            tmpmaxact - coef * ub
        };

        minresactivity = if scip.set.is_infinity(-lb) {
            if minactinf == 1 { tmpminact } else { -inf }
        } else if minactinf > 0 {
            -inf
        } else {
            tmpminact - coef * lb
        };
    }
    // The coefficient is negative, so the lower bound contributed to the
    // maxactivity and the upper bound to the minactivity.
    else {
        maxresactivity = if scip.set.is_infinity(-lb) {
            if maxactinf == 1 { tmpmaxact } else { inf }
        } else if maxactinf > 0 {
            inf
        } else {
            tmpmaxact - coef * lb
        };

        minresactivity = if scip.set.is_infinity(ub) {
            if minactinf == 1 { tmpminact } else { -inf }
        } else if minactinf > 0 {
            -inf
        } else {
            tmpminact - coef * ub
        };
    }

    (minresactivity, maxresactivity)
}

/// `getActivityResidualsUpperBound`: get minimum/maximum residual activity
/// for the specified variable and with another variable set to its upper
/// bound; `None` if that upper bound is infinite.
#[allow(clippy::too_many_arguments)]
fn get_activity_residuals_upper_bound(scip: &Scip, matrix: &Matrix, row: usize, col: usize, coef: f64, upperboundcol: usize, upperboundcoef: f64) -> Option<(f64, f64)> {
    let var = matrix.vars[col];
    let upperboundvar = matrix.vars[upperboundcol];
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;

    let mut maxactinf = matrix.maxactivityposinf[row] + matrix.maxactivityneginf[row];
    let mut minactinf = matrix.minactivityposinf[row] + matrix.minactivityneginf[row];

    let lbupperboundvar = scip.vars[upperboundvar].glbdom.lb;
    let ubupperboundvar = scip.vars[upperboundvar].glbdom.ub;

    let mut tmpminact = matrix.minactivity[row];
    let mut tmpmaxact = matrix.maxactivity[row];

    // First, adjust min and max activity such that upperboundvar is always set
    // to its upper bound; abort if the upper bound is infty.
    if scip.set.is_infinity(ubupperboundvar) {
        return None;
    }

    // coef > 0 --> the lower bound is used for the minactivity --> update the
    // minactivity.
    if upperboundcoef > 0.0 {
        if scip.set.is_infinity(-lbupperboundvar) {
            minactinf -= 1;
            tmpminact += upperboundcoef * ubupperboundvar;
        } else {
            tmpminact = tmpminact - (upperboundcoef * lbupperboundvar) + (upperboundcoef * ubupperboundvar);
        }
    }
    // coef < 0 --> the lower bound is used for the maxactivity --> update the
    // maxactivity.
    else if scip.set.is_infinity(-lbupperboundvar) {
        maxactinf -= 1;
        tmpmaxact += upperboundcoef * ubupperboundvar;
    } else {
        tmpmaxact = tmpmaxact - (upperboundcoef * lbupperboundvar) + (upperboundcoef * ubupperboundvar);
    }

    Some(residuals_from(scip, lb, ub, coef, tmpminact, tmpmaxact, minactinf, maxactinf))
}

/// `getActivityResidualsLowerBound`: get minimum/maximum residual activity
/// for the specified variable and with another variable set to its lower
/// bound; `None` if that lower bound is infinite.
#[allow(clippy::too_many_arguments)]
fn get_activity_residuals_lower_bound(scip: &Scip, matrix: &Matrix, row: usize, col: usize, coef: f64, lowerboundcol: usize, lowerboundcoef: f64) -> Option<(f64, f64)> {
    let var = matrix.vars[col];
    let lowerboundvar = matrix.vars[lowerboundcol];
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;

    let mut maxactinf = matrix.maxactivityposinf[row] + matrix.maxactivityneginf[row];
    let mut minactinf = matrix.minactivityposinf[row] + matrix.minactivityneginf[row];

    let lblowerboundvar = scip.vars[lowerboundvar].glbdom.lb;
    let ublowerboundvar = scip.vars[lowerboundvar].glbdom.ub;

    let mut tmpminact = matrix.minactivity[row];
    let mut tmpmaxact = matrix.maxactivity[row];

    // First, adjust min and max activity such that lowerboundvar is always set
    // to its lower bound; abort if the lower bound is -infty.
    if scip.set.is_infinity(-lblowerboundvar) {
        return None;
    }

    // coef > 0 --> the upper bound is used for the maxactivity --> update the
    // maxactivity.
    if lowerboundcoef > 0.0 {
        if scip.set.is_infinity(ublowerboundvar) {
            maxactinf -= 1;
            tmpmaxact += lowerboundcoef * lblowerboundvar;
        } else {
            tmpmaxact = tmpmaxact - (lowerboundcoef * ublowerboundvar) + (lowerboundcoef * lblowerboundvar);
        }
    }
    // coef < 0 --> the upper bound is used for the minactivity --> update the
    // minactivity.
    else if scip.set.is_infinity(ublowerboundvar) {
        minactinf -= 1;
        tmpminact += lowerboundcoef * lblowerboundvar;
    } else {
        tmpminact = tmpminact - (lowerboundcoef * ublowerboundvar) + (lowerboundcoef * lblowerboundvar);
    }

    Some(residuals_from(scip, lb, ub, coef, tmpminact, tmpmaxact, minactinf, maxactinf))
}

/// The predicted bounds of a variable from one row (`calcVarBoundsDominated`
/// and `calcVarBoundsDominating`); each is `Some` if it was calculated.
#[derive(Default)]
struct RowBounds {
    ub: Option<f64>,
    wclb: Option<f64>,
    lb: Option<f64>,
    wcub: Option<f64>,
}

/// The predictive rowbound analysis shared by `calcVarBoundsDominated` and
/// `calcVarBoundsDominating`: bounds of the variable with coefficient `val`
/// from the residual activities of `row` without it.
fn predictive_row_bounds(scip: &Scip, matrix: &Matrix, row: usize, val: f64, minresactivity: f64, maxresactivity: f64) -> RowBounds {
    let inf = scip.set.infinity();
    let lhs = matrix.lhs[row];
    let rhs = matrix.rhs[row];
    let mut b = RowBounds::default();

    // Predictive rowbound analysis.
    if val > 0.0 {
        // Lower bound calculation.
        if !scip.set.is_infinity(maxresactivity) {
            b.lb = Some((lhs - maxresactivity) / val);
        }

        // Worst case calculation of lower bound.
        b.wclb = Some(if !scip.set.is_infinity(-minresactivity) {
            (lhs - minresactivity) / val
        } else {
            // Worst case lower bound is infinity.
            inf
        });

        // We can only calculate upper bounds, of the right hand side is finite.
        if !matrix.isrhsinfinite[row] {
            // Upper bound calculation.
            if !scip.set.is_infinity(-minresactivity) {
                b.ub = Some((rhs - minresactivity) / val);
            }

            // Worst case calculation of upper bound.
            b.wcub = Some(if !scip.set.is_infinity(maxresactivity) {
                (rhs - maxresactivity) / val
            } else {
                // Worst case upper bound is -infinity.
                -inf
            });
        }
    } else {
        // Upper bound calculation.
        if !scip.set.is_infinity(maxresactivity) {
            b.ub = Some((lhs - maxresactivity) / val);
        }

        // Worst case calculation of upper bound.
        b.wcub = Some(if !scip.set.is_infinity(-minresactivity) {
            (lhs - minresactivity) / val
        } else {
            // Worst case upper bound is -infinity.
            -inf
        });

        // We can only calculate lower bounds, of the right hand side is finite.
        if !matrix.isrhsinfinite[row] {
            // Lower bound calculation.
            if !scip.set.is_infinity(-minresactivity) {
                b.lb = Some((rhs - minresactivity) / val);
            }

            // Worst case calculation of lower bound.
            b.wclb = Some(if !scip.set.is_infinity(maxresactivity) {
                (rhs - maxresactivity) / val
            } else {
                // Worst case lower bound is infinity.
                inf
            });
        }
    }

    b
}

/// `calcVarBoundsDominated`: calculate bounds of the dominated variable by
/// rowbound analysis; we use a special kind of predictive rowbound analysis
/// by first setting the dominating variable to its upper bound.
#[allow(clippy::too_many_arguments)]
fn calc_var_bounds_dominated(scip: &Scip, matrix: &Matrix, row: usize, coldominating: usize, valdominating: f64, coldominated: usize, valdominated: f64) -> RowBounds {
    // No rowbound analysis for multiaggregated variables, which should not
    // exist, because the matrix only consists of active variables. Get
    // minimum/maximum activity of this row without the dominated variable.
    match get_activity_residuals_upper_bound(scip, matrix, row, coldominated, valdominated, coldominating, valdominating) {
        Some((minres, maxres)) => predictive_row_bounds(scip, matrix, row, valdominated, minres, maxres),
        None => RowBounds::default(),
    }
}

/// `calcVarBoundsDominating`: calculate bounds of the dominating variable by
/// rowbound analysis; we use a special kind of predictive rowbound analysis
/// by first setting the dominated variable to its lower bound.
#[allow(clippy::too_many_arguments)]
fn calc_var_bounds_dominating(scip: &Scip, matrix: &Matrix, row: usize, coldominating: usize, valdominating: f64, coldominated: usize, valdominated: f64) -> RowBounds {
    // Get minimum/maximum activity of this row without the dominating
    // variable.
    match get_activity_residuals_lower_bound(scip, matrix, row, coldominating, valdominating, coldominated, valdominated) {
        Some((minres, maxres)) => predictive_row_bounds(scip, matrix, row, valdominating, minres, maxres),
        None => RowBounds::default(),
    }
}

/// The predicted bounds `updateBounds` improves.
#[derive(Clone, Copy)]
struct Predicted {
    upperbound: f64,
    wclowerbound: f64,
    lowerbound: f64,
    wcupperbound: f64,
}

/// `updateBounds`: try to find new variable bounds and update them when they
/// are better then the old bounds.
#[allow(clippy::too_many_arguments)]
fn update_bounds(scip: &Scip, matrix: &Matrix, row: usize, col1: usize, val1: f64, col2: usize, val2: f64, predictdominating: bool, p: &mut Predicted) {
    let b = if predictdominating {
        // Do predictive rowbound analysis for the dominating variable.
        calc_var_bounds_dominating(scip, matrix, row, col1, val1, col2, val2)
    } else {
        // Do predictive rowbound analysis for the dominated variable.
        calc_var_bounds_dominated(scip, matrix, row, col1, val1, col2, val2)
    };

    // Update bounds in case if they are better.
    if let Some(newub) = b.ub {
        if newub < p.upperbound {
            p.upperbound = newub;
        }
    }

    if let Some(newwclb) = b.wclb {
        if newwclb > p.wclowerbound {
            p.wclowerbound = newwclb;
        }
    }

    if let Some(newlb) = b.lb {
        if newlb > p.lowerbound {
            p.lowerbound = newlb;
        }
    }

    if let Some(newwcub) = b.wcub {
        if newwcub < p.wcupperbound {
            p.wcupperbound = newwcub;
        }
    }
}

/// `detectParallelCols`: detect parallel columns by using the algorithm of
/// Bixby and Wagner (see paper: "A note on Detecting Simple Redundancies in
/// Linear Systems", June 1986); only non-empty equations or ranged rows are
/// considered, and `varineq` marks the variables in them.
fn detect_parallel_cols(scip: &Scip, matrix: &Matrix, pclass: &mut [i32], varineq: &mut [bool]) {
    let ncols = matrix.ncols as usize;
    let mut scale = vec![0.0f64; ncols];

    let considered = |r: usize| !matrix.isrhsinfinite[r] && matrix.row_nnonzs(r) > 0;

    // Remember variable was part of an equation or ranged row.
    for r in 0..matrix.nrows as usize {
        if considered(r) {
            for k in matrix.row_range(r) {
                varineq[matrix.rowmatind[k] as usize] = true;
            }
        }
    }

    // Loop over all rows.
    let mut lines = (0..matrix.nrows as usize).map(|r| {
        if considered(r) {
            Some(matrix.row_range(r).map(|k| (matrix.rowmatind[k], matrix.rowmatval[k])).collect())
        } else {
            None
        }
    });
    crate::matrix::refine_parallel_classes(scip, ncols, &mut lines, &mut scale, pclass);
}

/// The type check `predBndStr` and `findFixings` share: we compare only
/// variables from the same type.
fn same_type(scip: &Scip, dominatingvar: usize, dominatedvar: usize) -> bool {
    use crate::var::{var_is_binary, var_is_implied_integral};

    // This was refactored to be equivalent to original code, but probably this
    // can handle more cases / be simplified.
    let dominatingimplied = var_is_implied_integral(scip, dominatingvar);
    let dominatedimplied = var_is_implied_integral(scip, dominatedvar);
    let mut good = var_is_binary(scip, dominatingvar) == var_is_binary(scip, dominatedvar);

    if !good {
        if !dominatingimplied && !dominatedimplied {
            good = scip.vars[dominatingvar].vartype == scip.vars[dominatedvar].vartype;
        } else if dominatingimplied {
            good = dominatedimplied || scip.vars[dominatedvar].vartype == VarType::Integer;
        } else {
            good = scip.vars[dominatingvar].vartype == VarType::Integer;
        }
    }

    good
}

/// `predBndStr`: try to improve variable bounds by predictive bound
/// strengthening.
#[allow(clippy::too_many_arguments)]
fn pred_bnd_str(
    scip: &mut Scip,
    dominatingvar: usize,
    dominatingidx: usize,
    dominatingub: f64,
    dominatinglb: f64,
    dominatingwcub: f64,
    dominatedvar: usize,
    dominatedidx: usize,
    dominatedub: f64,
    dominatedwclb: f64,
    dominatedlb: f64,
    varstofix: &[Fixingdirection],
    nchgbds: &mut i32,
) -> ScipResult {
    // We compare only variables from the same type.
    if !same_type(scip, dominatingvar, dominatedvar) {
        return Ok(());
    }

    if varstofix[dominatingidx] == Fixingdirection::NoFix {
        // Assume x dominates y (x->y). We get this bound from a positive
        // coefficient of the dominating variable. Because x->y the dominated
        // variable y has a positive coefficient too. Thus y contributes to the
        // minactivity with its lower bound. But this case is considered within
        // predictive bound analysis. Thus the dominating upper bound is a common
        // upper bound.
        if !scip.set.is_infinity(dominatingub) {
            let newub = dominatingub;
            let oldub = scip.vars[dominatingvar].glbdom.ub;
            let lb = scip.vars[dominatingvar].glbdom.lb;

            if scip.set.is_le(lb, newub) && scip.set.is_lt(newub, oldub) {
                scip_var::chg_var_ub(scip, dominatingvar, newub)?;
                *nchgbds += 1;
            }
        }

        // Assume x dominates y (x->y). We get this lower bound of the dominating
        // variable from a negative coefficient within a <= relation. If y has a
        // positive coefficient we get a common lower bound of x from predictive
        // bound analysis. If y has a negative coefficient we get an improved
        // lower bound of x because the minactivity is greater. For discrete
        // variables we have to round down the lower bound.
        if !scip.set.is_infinity(-dominatinglb) {
            let mut newlb = dominatinglb;
            let oldlb = scip.vars[dominatingvar].glbdom.lb;
            let ub = scip.vars[dominatingvar].glbdom.ub;

            if var_is_integral(scip, dominatingvar) {
                newlb = scip.set.floor(newlb);
            }

            if scip.set.is_lt(oldlb, newlb) && scip.set.is_le(newlb, ub) {
                scip_var::chg_var_lb(scip, dominatingvar, newlb)?;
                *nchgbds += 1;
            }
        }

        // Assume x dominates y (x->y). We get this bound from a positive
        // coefficient of x within a <= relation. From x->y it follows, that y has
        // a positive coefficient in this row too. The worst case upper bound of x
        // is an estimation of how great x can be in every case. If the objective
        // coefficient of x is negative we get thus a lower bound of x. For
        // discrete variables we have to round down the lower bound before.
        if !scip.set.is_infinity(dominatingwcub) && scip.set.is_negative(scip.vars[dominatingvar].obj) {
            let mut newlb = dominatingwcub;
            let oldlb = scip.vars[dominatingvar].glbdom.lb;
            let ub = scip.vars[dominatingvar].glbdom.ub;

            if var_is_integral(scip, dominatingvar) {
                newlb = scip.set.floor(newlb);
            }

            if scip.set.is_lt(oldlb, newlb) && scip.set.is_le(newlb, ub) {
                scip_var::chg_var_lb(scip, dominatingvar, newlb)?;
                *nchgbds += 1;
            }
        }
    }

    if varstofix[dominatedidx] == Fixingdirection::NoFix {
        // Assume x dominates y (x->y). We get this bound for a positive
        // coefficient of y within a <= relation. If x has a negative coefficient
        // we get a common upper bound of y. If x has a positive coefficient we get
        // an improved upper bound of y because the minactivity is greater.
        if !scip.set.is_infinity(dominatedub) {
            let newub = dominatedub;
            let oldub = scip.vars[dominatedvar].glbdom.ub;
            let lb = scip.vars[dominatedvar].glbdom.lb;

            if scip.set.is_le(lb, newub) && scip.set.is_lt(newub, oldub) {
                scip_var::chg_var_ub(scip, dominatedvar, newub)?;
                *nchgbds += 1;
            }
        }

        // Assume x dominates y (x->y). We get this bound only from a negative
        // coefficient of y within a <= relation. Because of x->y then x has a
        // negative coefficient too. The worst case lower bound is an estimation
        // what property the dominated variable must have if the dominating
        // variable is at its upper bound. To get an upper bound of the dominated
        // variable we need to consider a positive objective coefficient. For
        // discrete variables we have to round up the upper bound.
        if !scip.set.is_infinity(-dominatedwclb) && scip.set.is_positive(scip.vars[dominatedvar].obj) {
            let mut newub = dominatedwclb;
            let oldub = scip.vars[dominatedvar].glbdom.ub;
            let lb = scip.vars[dominatedvar].glbdom.lb;

            if var_is_integral(scip, dominatedvar) {
                newub = scip.set.ceil(newub);
            }

            if scip.set.is_le(lb, newub) && scip.set.is_lt(newub, oldub) {
                scip_var::chg_var_ub(scip, dominatedvar, newub)?;
                *nchgbds += 1;
            }
        }

        // Assume x dominates y (x->y). We get a lower bound of y from a negative
        // coefficient within a <= relation. But if x->y then x has a negative
        // coefficient too and contributes with its upper bound to the
        // minactivity. Thus in all we have a common lower bound calculation and
        // no rounding is necessary.
        if !scip.set.is_infinity(-dominatedlb) {
            let newlb = dominatedlb;
            let oldlb = scip.vars[dominatedvar].glbdom.lb;
            let ub = scip.vars[dominatedvar].glbdom.ub;

            if scip.set.is_lt(oldlb, newlb) && scip.set.is_le(newlb, ub) {
                scip_var::chg_var_lb(scip, dominatedvar, newlb)?;
                *nchgbds += 1;
            }
        }
    }

    Ok(())
}

/// `findFixings`: try to find variable fixings.
#[allow(clippy::too_many_arguments)]
fn find_fixings(
    scip: &Scip,
    matrix: &Matrix,
    dominatingvar: usize,
    dominatingidx: usize,
    dominatingub: f64,
    dominatingwclb: f64,
    dominatinglb: f64,
    dominatingwcub: f64,
    dominatedvar: usize,
    dominatedidx: usize,
    varstofix: &mut [Fixingdirection],
    onlybinvars: bool,
    onlyoneone: bool,
    nfixings: &mut i32,
) {
    // We compare only variables from the same type.
    if !same_type(scip, dominatingvar, dominatedvar) {
        return;
    }

    let dominatingvarub = scip.vars[dominatingvar].glbdom.ub;

    if varstofix[dominatedidx] == Fixingdirection::NoFix && matrix.col_nnonzs(dominatingidx) == 1 && matrix.col_nnonzs(dominatedidx) == 1 {
        // We have a x->y dominance relation and only one equality constraint
        // where the dominating variable x with an infinity upper bound and the
        // dominated variable y are present. Then the dominated variable y can be
        // fixed at its lower bound.
        let row = matrix.colmatind[matrix.colmatbeg[dominatedidx] as usize] as usize;

        if scip.set.is_eq(matrix.lhs[row], matrix.rhs[row]) && scip.set.is_infinity(dominatingvarub) {
            varstofix[dominatedidx] = Fixingdirection::FixAtLb;
            *nfixings += 1;

            return;
        }
    }

    if varstofix[dominatedidx] == Fixingdirection::NoFix && !scip.set.is_negative(scip.vars[dominatedvar].obj) {
        if !scip.set.is_infinity(-dominatingwclb) && scip.set.is_le(dominatingwclb, dominatingvarub) {
            // We have a x->y dominance relation with a positive obj coefficient of
            // the dominated variable y. We need to secure feasibility by testing if
            // the predicted lower worst case bound is less equal the current upper
            // bound. It is possible, that the lower worst case bound is infinity
            // and the upper bound of the dominating variable x is infinity too.
            varstofix[dominatedidx] = Fixingdirection::FixAtLb;
            *nfixings += 1;
        }
    }

    if varstofix[dominatedidx] == Fixingdirection::NoFix && !scip.set.is_infinity(dominatingub) && scip.set.is_le(dominatingub, dominatingvarub) {
        // We have a x->y dominance relation with an arbitrary obj coefficient of
        // the dominating variable x. In all cases we have to look if the
        // predicted upper bound of the dominating variable is great enough. By
        // testing, that the predicted upper bound is not infinity we avoid
        // problems with x->y e.g. min -x -y s.t. -x -y <= -1, 0<=x<=1, 0<=y<=1
        // where y is not at their lower bound.
        varstofix[dominatedidx] = Fixingdirection::FixAtLb;
        *nfixings += 1;
    }

    if varstofix[dominatingidx] == Fixingdirection::NoFix && !scip.set.is_positive(scip.vars[dominatingvar].obj) {
        // We have a x->y dominance relation with a negative obj coefficient of the
        // dominating variable x. If the worst case upper bound is greater equal
        // than upper bound, we fix x at the upper bound.
        if !scip.set.is_infinity(dominatingwcub) && scip.set.is_ge(dominatingwcub, dominatingvarub) {
            varstofix[dominatingidx] = Fixingdirection::FixAtUb;
            *nfixings += 1;
        }
    }

    if varstofix[dominatingidx] == Fixingdirection::NoFix && !scip.set.is_infinity(-dominatinglb) && scip.set.is_ge(dominatinglb, dominatingvarub) {
        // We have a x->y dominance relation with an arbitrary obj coefficient of
        // the dominating variable x. If the predicted lower bound is greater
        // equal than upper bound, we fix x at the upper bound.
        varstofix[dominatingidx] = Fixingdirection::FixAtUb;
        *nfixings += 1;
    }

    if onlybinvars {
        if varstofix[dominatedidx] == Fixingdirection::NoFix && (onlyoneone || crate::var::vars_have_common_clique(scip, dominatingvar, true, dominatedvar, true, true)) {
            // We have a (1->1)-clique with dominance relation (x->y) (x dominates
            // y). From this dominance relation, we know (1->0) is possible and not
            // worse than (0->1) concerning the objective function. It follows that
            // only (1->0) or (0->0) are possible, but in both cases y has the value
            // 0 => y=0.
            varstofix[dominatedidx] = Fixingdirection::FixAtLb;
            *nfixings += 1;
        }

        if varstofix[dominatingidx] == Fixingdirection::NoFix && crate::var::vars_have_common_clique(scip, dominatingvar, false, dominatedvar, false, true) {
            // We have a (0->0)-clique with dominance relation x->y (x dominates y).
            // From this dominance relation, we know (1->0) is possible and not
            // worse than (0->1) concerning the objective function. It follows that
            // only (1->0) or (1->1) are possible, but in both cases x has the value
            // 1 => x=1.
            varstofix[dominatingidx] = Fixingdirection::FixAtUb;
            *nfixings += 1;
        }
    }
}

/// The pair-comparison counters of `SCIP_PresolData` that
/// `findDominancePairs` reads and adapts.
struct PairControl {
    numminpairs: i32,
    nummaxpairs: i32,
    numcurrentpairs: i32,
    predbndstr: bool,
}

/// The fresh predicted bounds of a dominating or dominated column.
fn fresh_predicted(inf: f64) -> Predicted {
    Predicted {
        upperbound: inf,
        wclowerbound: -inf,
        lowerbound: -inf,
        wcupperbound: inf,
    }
}

/// `findDominancePairs`: find dominance relation between variable pairs.
#[allow(clippy::too_many_arguments)]
fn find_dominance_pairs(
    scip: &mut Scip,
    matrix: &Matrix,
    control: &mut PairControl,
    searchcols: &[usize],
    onlybinvars: bool,
    varstofix: &mut [Fixingdirection],
    nfixings: &mut i32,
    ndomrelations: &mut i64,
    nchgbds: &mut i32,
) -> ScipResult {
    let inf = scip.set.infinity();
    let searchsize = searchcols.len();
    let mut paircnt = 0;
    let mut oldnfixings = *nfixings;

    // Pair comparisons.
    for cnt1 in 0..searchsize {
        // Get index of the first variable.
        let col1 = searchcols[cnt1];

        if varstofix[col1] == Fixingdirection::FixAtLb {
            continue;
        }

        let varcol1 = matrix.vars[col1];
        let obj1 = scip.vars[varcol1].obj;

        for &col2 in &searchcols[cnt1 + 1..] {
            // Get index of the second variable.
            let varcol2 = matrix.vars[col2];
            let mut onlyoneone = false;
            let obj2 = scip.vars[varcol2].obj;

            // We always have minimize as obj sense. Column 1 dominating column 2.
            let mut col1domcol2 = obj1 <= obj2;

            // Column 2 dominating column 1.
            let mut col2domcol1 = obj2 <= obj1;

            // Search only if nothing was found yet.
            col1domcol2 = col1domcol2 && (varstofix[col2] == Fixingdirection::NoFix);
            col2domcol1 = col2domcol1 && (varstofix[col1] == Fixingdirection::NoFix);

            // We only search for a dominance relation if the lower bounds are not
            // negative.
            if !onlybinvars && (scip.set.is_lt(scip.vars[varcol1].glbdom.lb, 0.0) || scip.set.is_lt(scip.vars[varcol2].glbdom.lb, 0.0)) {
                col1domcol2 = false;
                col2domcol1 = false;
            }

            // Pair comparison control.
            if paircnt == control.numcurrentpairs {
                if *nfixings == oldnfixings {
                    // Not enough fixings found, decrement number of comparisons.
                    control.numcurrentpairs >>= 1;
                    if control.numcurrentpairs < control.numminpairs {
                        control.numcurrentpairs = control.numminpairs;
                    }

                    // Stop searching in this row.
                    return Ok(());
                }
                oldnfixings = *nfixings;
                paircnt = 0;

                // Increment number of comparisons.
                control.numcurrentpairs <<= 1;
                if control.numcurrentpairs > control.nummaxpairs {
                    control.numcurrentpairs = control.nummaxpairs;
                }
            }
            paircnt += 1;

            if !col1domcol2 && !col2domcol1 {
                continue;
            }

            // Get the data for both columns.
            let (b1, b2) = (matrix.colmatbeg[col1] as usize, matrix.colmatbeg[col2] as usize);
            let nrows1 = matrix.col_nnonzs(col1) as usize;
            let nrows2 = matrix.col_nnonzs(col2) as usize;
            let rows1 = &matrix.colmatind[b1..b1 + nrows1];
            let vals1 = &matrix.colmatval[b1..b1 + nrows1];
            let rows2 = &matrix.colmatind[b2..b2 + nrows2];
            let vals2 = &matrix.colmatval[b2..b2 + nrows2];
            let mut r1 = 0;
            let mut r2 = 0;

            // Do we have a obj constant?
            if nrows1 == 0 || nrows2 == 0 {
                continue;
            }

            // Initialize temporary bounds of dominating and dominated variable.
            let mut dominating1 = fresh_predicted(inf);
            let mut dominating2 = fresh_predicted(inf);
            let mut dominated1 = fresh_predicted(inf);
            let mut dominated2 = fresh_predicted(inf);

            // Compare rows of this column pair.
            while (col1domcol2 || col2domcol1) && (r1 < nrows1 || r2 < nrows2) {
                // There is a nonredundant row containing column 1 but not column 2.
                if r1 < nrows1 && (r2 == nrows2 || rows1[r1] < rows2[r2]) {
                    // Dominance depends on the type of relation.
                    if !matrix.isrhsinfinite[rows1[r1] as usize] {
                        // No dominance relation for equations or ranged rows.
                        col2domcol1 = false;
                        col1domcol2 = false;
                    } else {
                        // >= relation, larger coefficients dominate smaller ones.
                        if vals1[r1] > 0.0 {
                            col2domcol1 = false;
                        } else {
                            col1domcol2 = false;
                        }
                    }

                    r1 += 1;
                }
                // There is a nonredundant row containing column 2, but not column 1.
                else if r2 < nrows2 && (r1 == nrows1 || rows1[r1] > rows2[r2]) {
                    // Dominance depends on the type of relation.
                    if !matrix.isrhsinfinite[rows2[r2] as usize] {
                        // No dominance relation for equations or ranged rows.
                        col2domcol1 = false;
                        col1domcol2 = false;
                    } else {
                        // >= relation, larger coefficients dominate smaller ones.
                        if vals2[r2] < 0.0 {
                            col2domcol1 = false;
                        } else {
                            col1domcol2 = false;
                        }
                    }

                    r2 += 1;
                }
                // If both columns appear in a common row, compare the coefficients.
                else {
                    let row = rows1[r1] as usize;

                    // If both columns are binary variables we check if they have a
                    // common clique and do not calculate any bounds.
                    if onlybinvars && !onlyoneone {
                        if vals1[r1] < 0.0
                            && vals2[r2] < 0.0
                            && (matrix.maxactivityposinf[row] + matrix.maxactivityneginf[row] == 0)
                            && scip.set.is_feas_le(matrix.maxactivity[row] + crate::def::max(vals1[r1], vals2[r2]), matrix.lhs[row])
                        {
                            onlyoneone = true;
                        }

                        if !onlyoneone
                            && !matrix.isrhsinfinite[row]
                            && vals1[r1] > 0.0
                            && vals2[r2] > 0.0
                            && (matrix.minactivityposinf[row] + matrix.minactivityneginf[row] == 0)
                            && scip.set.is_feas_ge(matrix.minactivity[row] + crate::def::min(vals1[r1], vals2[r2]), matrix.rhs[row])
                        {
                            onlyoneone = true;
                        }

                        if onlyoneone {
                            // Reset bounds.
                            dominating1 = fresh_predicted(inf);
                            dominating2 = fresh_predicted(inf);
                            dominated1 = fresh_predicted(inf);
                            dominated2 = fresh_predicted(inf);
                        }
                    }

                    // Dominance depends on the type of inequality.
                    if !matrix.isrhsinfinite[row] {
                        // No dominance relation if coefficients differ for equations or
                        // ranged rows.
                        if !scip.set.is_eq(vals1[r1], vals2[r2]) {
                            col2domcol1 = false;
                            col1domcol2 = false;
                        }
                    } else {
                        // >= relation, larger coefficients dominate smaller ones.
                        if vals1[r1] > vals2[r2] {
                            col2domcol1 = false;
                        } else if vals1[r1] < vals2[r2] {
                            col1domcol2 = false;
                        }
                    }

                    // We do not use bound calulations if two binary variable are in one
                    // common clique. For the other cases we claim the same sign for the
                    // coefficients to achieve monotonically decreasing predictive bound
                    // functions.
                    if !onlyoneone && ((vals1[r1] < 0.0 && vals2[r2] < 0.0) || (vals1[r1] > 0.0 && vals2[r2] > 0.0)) {
                        if col1domcol2 {
                            // Update bounds of dominating variable for column 1.
                            update_bounds(scip, matrix, row, col1, vals1[r1], col2, vals2[r2], true, &mut dominating1);

                            // Update bounds of dominated variable for column 1.
                            update_bounds(scip, matrix, row, col1, vals1[r1], col2, vals2[r2], false, &mut dominated1);
                        }

                        if col2domcol1 {
                            let row2 = rows2[r2] as usize;

                            // Update bounds of dominating variable for column 2.
                            update_bounds(scip, matrix, row2, col2, vals2[r2], col1, vals1[r1], true, &mut dominating2);

                            // Update bounds of dominated variable for column 2.
                            update_bounds(scip, matrix, row2, col2, vals2[r2], col1, vals1[r1], false, &mut dominated2);
                        }
                    }

                    r1 += 1;
                    r2 += 1;
                }
            }

            // A column is only dominated, if there are no more rows in which it is
            // contained.
            col1domcol2 = col1domcol2 && r2 == nrows2;
            col2domcol1 = col2domcol1 && r1 == nrows1;

            if !col1domcol2 && !col2domcol1 {
                continue;
            }

            // No dominance relation for left equations or ranged rows.
            while r1 < nrows1 {
                if !matrix.isrhsinfinite[rows1[r1] as usize] {
                    col2domcol1 = false;
                    col1domcol2 = false;
                    break;
                }
                r1 += 1;
            }

            if !col1domcol2 && !col2domcol1 {
                continue;
            }

            while r2 < nrows2 {
                if !matrix.isrhsinfinite[rows2[r2] as usize] {
                    col2domcol1 = false;
                    col1domcol2 = false;
                    break;
                }
                r2 += 1;
            }

            if col1domcol2 || col2domcol1 {
                *ndomrelations += 1;
            }

            if col1domcol2 && col2domcol1 {
                // Prefer the variable as dominating variable with the greater upper
                // bound.
                if scip.set.is_ge(scip.vars[varcol1].glbdom.ub, scip.vars[varcol2].glbdom.ub) {
                    col2domcol1 = false;
                } else {
                    col1domcol2 = false;
                }
            }

            // Use dominance relation and clique/bound-information to find variable
            // fixings. Additionally try to strengthen variable bounds by predictive
            // bound strengthening.
            if col1domcol2 {
                let (g, d) = (dominating1, dominated1);
                find_fixings(
                    scip,
                    matrix,
                    varcol1,
                    col1,
                    g.upperbound,
                    g.wclowerbound,
                    g.lowerbound,
                    g.wcupperbound,
                    varcol2,
                    col2,
                    varstofix,
                    onlybinvars,
                    onlyoneone,
                    nfixings,
                );

                if control.predbndstr {
                    pred_bnd_str(
                        scip,
                        varcol1,
                        col1,
                        g.upperbound,
                        g.lowerbound,
                        g.wcupperbound,
                        varcol2,
                        col2,
                        d.upperbound,
                        d.wclowerbound,
                        d.lowerbound,
                        varstofix,
                        nchgbds,
                    )?;
                }
            } else if col2domcol1 {
                let (g, d) = (dominating2, dominated2);
                find_fixings(
                    scip,
                    matrix,
                    varcol2,
                    col2,
                    g.upperbound,
                    g.wclowerbound,
                    g.lowerbound,
                    g.wcupperbound,
                    varcol1,
                    col1,
                    varstofix,
                    onlybinvars,
                    onlyoneone,
                    nfixings,
                );

                if control.predbndstr {
                    pred_bnd_str(
                        scip,
                        varcol2,
                        col2,
                        g.upperbound,
                        g.lowerbound,
                        g.wcupperbound,
                        varcol1,
                        col1,
                        d.upperbound,
                        d.wclowerbound,
                        d.lowerbound,
                        varstofix,
                        nchgbds,
                    )?;
                }
            }

            if varstofix[col1] == Fixingdirection::FixAtLb {
                break;
            }
        }
    }

    Ok(())
}

#[inline]
fn pdm(scip: &mut Scip, presol: PresolId) -> &mut PresolData {
    scip.presols[presol].presoldata.as_mut().unwrap().downcast_mut::<PresolData>().unwrap()
}

/// Sorts the columns `cols` into the continuous, integer and binary search
/// lists, as both stages of `presolExecDomcol` do: we search only for
/// dominance relations between the same variable type.
fn split_search_cols(scip: &Scip, matrix: &Matrix, cols: &[usize], consearchcols: &mut Vec<usize>, intsearchcols: &mut Vec<usize>, binsearchcols: &mut Vec<usize>) {
    consearchcols.clear();
    intsearchcols.clear();
    binsearchcols.clear();
    for &varidx in cols {
        let var = matrix.vars[varidx];
        if !var_is_integral(scip, var) {
            consearchcols.push(varidx);
        } else if crate::var::var_is_binary(scip, var) {
            binsearchcols.push(varidx);
        } else {
            intsearchcols.push(varidx);
        }
    }
}

/*
 * Callback methods of presolver
 */

/// `presolFreeDomcol`: destructor of presolver to free user data (called
/// when SCIP is exiting).
fn presol_free_domcol(scip: &mut Scip, presol: PresolId) -> ScipResult {
    // Free presolver data.
    scip.presols[presol].presoldata = None;
    Ok(())
}

/// `presolExecDomcol`: execution method of presolver.
fn presol_exec_domcol(
    scip: &mut Scip,
    presol: PresolId,
    _nrounds: i32,
    presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if !scip_var::allow_strong_dual_reds(scip) || crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    let p = pdm(scip, presol);
    let continuousred = p.continuousred;
    let mut control = PairControl {
        numminpairs: p.numminpairs,
        nummaxpairs: p.nummaxpairs,
        numcurrentpairs: p.numcurrentpairs,
        predbndstr: p.predbndstr,
    };

    // Don't run for pure LPs.
    if !continuousred && (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip) == 0) {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let created = matrix_create(
        scip,
        true,
        &mut counts.naddconss,
        &mut counts.ndelconss,
        &mut counts.nchgcoefs,
        &mut counts.nchgbds,
        &mut counts.nfixedvars,
    )?;

    // If infeasibility was detected during matrix creation, return here.
    if created.infeasible {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    let Some(matrix) = created.matrix else {
        return Ok(());
    };

    let mut nfixings = 0;
    let mut ndomrelations: i64 = 0;

    let nrows = matrix.nrows as usize;
    let ncols = matrix.ncols as usize;

    let mut varstofix = vec![Fixingdirection::NoFix; ncols];

    // Do not process columns that do not have all locks represented in the
    // matrix.
    let mut varsprocessed: Vec<bool> = (0..ncols)
        .map(|v| crate::matrix::matrix_uplock_conflict(scip, &matrix, v) || crate::matrix::matrix_downlock_conflict(scip, &matrix, v))
        .collect();

    let mut consearchcols = Vec::with_capacity(ncols);
    let mut intsearchcols = Vec::with_capacity(ncols);
    let mut binsearchcols = Vec::with_capacity(ncols);

    let mut rowidxsorted: Vec<i32> = (0..nrows as i32).collect();
    let mut rowsparsity: Vec<i32> = (0..nrows).map(|r| matrix.row_nnonzs(r)).collect();

    let mut pclass = vec![0i32; ncols];
    let mut colidx: Vec<i32> = (0..ncols as i32).collect();
    let mut varineq = vec![false; ncols];

    // Init pair comparision control.
    control.numcurrentpairs = control.nummaxpairs;

    let mut varcount = 0;

    if (presoltiming & PRESOLTIMING_EXHAUSTIVE) != 0 {
        // 1.stage: search dominance relations of parallel columns within
        // equalities and ranged rows.
        detect_parallel_cols(scip, &matrix, &mut pclass, &mut varineq);
        crate::sort::sort(&mut pclass, (&mut colidx[..],), crate::sort::cmp_int);

        let mut pc = 0;
        while pc < ncols {
            let mut varidx = 0;
            let pclassstart = pclass[pc];

            // We only regard variables which were not processed yet and are
            // present within equalities or ranged rows.
            let mut members = Vec::new();
            while pc < ncols && pclassstart == pclass[pc] {
                varidx = colidx[pc] as usize;
                if !varsprocessed[varidx] && varineq[varidx] {
                    members.push(varidx);
                }
                pc += 1;
            }
            split_search_cols(scip, &matrix, &members, &mut consearchcols, &mut intsearchcols, &mut binsearchcols);

            // Continuous variables.
            if consearchcols.len() > 1 && continuousred {
                find_dominance_pairs(
                    scip,
                    &matrix,
                    &mut control,
                    &consearchcols,
                    false,
                    &mut varstofix,
                    &mut nfixings,
                    &mut ndomrelations,
                    &mut counts.nchgbds,
                )?;

                for &v in &consearchcols {
                    varsprocessed[v] = true;
                }
                varcount += consearchcols.len();
            } else if consearchcols.len() == 1 && varineq[varidx] {
                varsprocessed[consearchcols[0]] = true;
            }

            // Integer and impl-integer variables.
            if intsearchcols.len() > 1 {
                find_dominance_pairs(
                    scip,
                    &matrix,
                    &mut control,
                    &intsearchcols,
                    false,
                    &mut varstofix,
                    &mut nfixings,
                    &mut ndomrelations,
                    &mut counts.nchgbds,
                )?;

                for &v in &intsearchcols {
                    varsprocessed[v] = true;
                }
                varcount += intsearchcols.len();
            } else if intsearchcols.len() == 1 && varineq[varidx] {
                varsprocessed[intsearchcols[0]] = true;
            }

            // Binary variables.
            if binsearchcols.len() > 1 {
                find_dominance_pairs(
                    scip,
                    &matrix,
                    &mut control,
                    &binsearchcols,
                    true,
                    &mut varstofix,
                    &mut nfixings,
                    &mut ndomrelations,
                    &mut counts.nchgbds,
                )?;

                for &v in &binsearchcols {
                    varsprocessed[v] = true;
                }
                varcount += binsearchcols.len();
            } else if binsearchcols.len() == 1 && varineq[varidx] {
                varsprocessed[binsearchcols[0]] = true;
            }

            if varcount >= ncols {
                break;
            }
        }

        // 2.stage: search dominance relations for the remaining columns by
        // increasing row-sparsity.
        crate::sort::sort(&mut rowsparsity, (&mut rowidxsorted[..],), crate::sort::cmp_int);

        for r in 0..nrows {
            // Break if the time limit was reached; since the check is expensive,
            // we only check all 1000 constraints.
            if (r % 1000 == 0) && crate::solve::solve_is_stopped(scip, false) {
                break;
            }

            let rowidx = rowidxsorted[r] as usize;

            if matrix.row_nnonzs(rowidx) == 1 {
                continue;
            }

            let members: Vec<usize> = matrix.row_range(rowidx).map(|k| matrix.rowmatind[k] as usize).filter(|&v| !varsprocessed[v]).collect();
            split_search_cols(scip, &matrix, &members, &mut consearchcols, &mut intsearchcols, &mut binsearchcols);

            // Continuous variables.
            if consearchcols.len() > 1 && continuousred {
                find_dominance_pairs(
                    scip,
                    &matrix,
                    &mut control,
                    &consearchcols,
                    false,
                    &mut varstofix,
                    &mut nfixings,
                    &mut ndomrelations,
                    &mut counts.nchgbds,
                )?;

                for &v in &consearchcols {
                    varsprocessed[v] = true;
                }
                varcount += consearchcols.len();
            }

            // Integer and impl-integer variables.
            if intsearchcols.len() > 1 {
                find_dominance_pairs(
                    scip,
                    &matrix,
                    &mut control,
                    &intsearchcols,
                    false,
                    &mut varstofix,
                    &mut nfixings,
                    &mut ndomrelations,
                    &mut counts.nchgbds,
                )?;

                for &v in &intsearchcols {
                    varsprocessed[v] = true;
                }
                varcount += intsearchcols.len();
            }

            // Binary variables.
            if binsearchcols.len() > 1 {
                find_dominance_pairs(
                    scip,
                    &matrix,
                    &mut control,
                    &binsearchcols,
                    true,
                    &mut varstofix,
                    &mut nfixings,
                    &mut ndomrelations,
                    &mut counts.nchgbds,
                )?;

                for &v in &binsearchcols {
                    varsprocessed[v] = true;
                }
                varcount += binsearchcols.len();
            }

            if varcount >= ncols {
                break;
            }
        }
    }

    pdm(scip, presol).numcurrentpairs = control.numcurrentpairs;

    if nfixings > 0 {
        let oldnfixedvars = counts.nfixedvars;

        for v in (0..ncols).rev() {
            let var = matrix.vars[v];

            // There should be no fixings for variables with lock conflicts, since
            // they where marked as processed and therefore should be skipped.
            let fixval = match varstofix[v] {
                // Fix at lower bound.
                Fixingdirection::FixAtLb => scip.vars[var].glbdom.lb,
                // Fix at upper bound.
                Fixingdirection::FixAtUb => scip.vars[var].glbdom.ub,
                Fixingdirection::NoFix => continue,
            };

            let (infeasible, _fixed) = scip_var::fix_var(scip, var, fixval)?;
            if infeasible {
                *result = ScipResultCode::Cutoff;
                break;
            }
            counts.nfixedvars += 1;
        }

        if *result != ScipResultCode::Cutoff && counts.nfixedvars > oldnfixedvars {
            *result = ScipResultCode::Success;
        }
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `presolCopyDomcol`: copy method for presolver plugins (called when SCIP
/// copies plugins).
fn presol_copy_domcol(scip: &mut Scip, _sourcescip: &Scip, _presol: PresolId) -> ScipResult {
    // Call inclusion method of presolver.
    include_presol_domcol(scip)?;
    Ok(())
}

/// `SCIPincludePresolDomcol`: creates the domcol presolver and includes it
/// in SCIP.
pub fn include_presol_domcol(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // Create domcol presolver data (C leaves numcurrentpairs unset until the
    // first execution).
    let presoldata = PresolData {
        numminpairs: DEFAULT_NUMMINPAIRS,
        nummaxpairs: DEFAULT_NUMMAXPAIRS,
        numcurrentpairs: 0,
        predbndstr: DEFAULT_PREDBNDSTR,
        continuousred: DEFAULT_CONTINUOUS_RED,
    };

    // Include presolver.
    let presol = api::include_presol_basic(
        scip,
        PRESOL_NAME,
        PRESOL_DESC,
        PRESOL_PRIORITY,
        PRESOL_MAXROUNDS,
        PRESOL_TIMING,
        presol_exec_domcol,
        Some(Box::new(presoldata) as Box<dyn Any>),
    )?;
    scip.presols[presol].presolcopy = Some(presol_copy_domcol);

    scip.presols[presol].presolfree = Some(presol_free_domcol);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, presols, presol, presoldata, PresolData, $f)
        };
    }

    paramset_add_int(
        scip,
        "presolving/domcol/numminpairs",
        "minimal number of pair comparisons",
        t!(i32, numminpairs),
        false,
        DEFAULT_NUMMINPAIRS,
        100,
        DEFAULT_NUMMAXPAIRS,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/domcol/nummaxpairs",
        "maximal number of pair comparisons",
        t!(i32, nummaxpairs),
        false,
        DEFAULT_NUMMAXPAIRS,
        DEFAULT_NUMMINPAIRS,
        1000000000,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/domcol/predbndstr",
        "should predictive bound strengthening be applied?",
        t!(bool, predbndstr),
        false,
        DEFAULT_PREDBNDSTR,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/domcol/continuousred",
        "should reductions for continuous variables be performed?",
        t!(bool, continuousred),
        false,
        DEFAULT_CONTINUOUS_RED,
        None,
    )?;

    Ok(())
}
