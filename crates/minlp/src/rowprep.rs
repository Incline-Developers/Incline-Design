//! Port of `misc_rowprep.c`: preparation of a linear inequality to become a
//! SCIP_ROW.

use crate::{
    def::INVALID,
    misc::{frexp, ldexp},
    retcode::ScipResult,
    scip::Scip,
    types::{ConsId, ConshdlrId, RowId, SepaId, SideType, SolId, VarId, VarStatus},
};

/// `SCIP_ROWPREP`: a linear inequality that is prepared to become a
/// SCIP_ROW.
///
/// The terms are `vars[i] * coefs[i]` for `i < vars.len()` (C's `nvars`).
#[derive(Clone, Debug)]
pub struct RowPrep {
    /// Variables.
    pub vars: Vec<VarId>,
    /// Coefficients of variables.
    pub coefs: Vec<f64>,
    /// Side.
    pub side: f64,
    /// Type of side.
    pub sidetype: SideType,
    /// Whether the row is only locally valid (i.e., for the current node).
    pub local: bool,
    /// Row name.
    pub name: String,

    /// Whether to remember variables which coefficients were modified during
    /// cleanup.
    pub recordmodifications: bool,
    /// Variables which coefficient were modified by cleanup.
    pub modifiedvars: Vec<VarId>,
    /// Whether the side was modified (relaxed) by cleanup.
    pub modifiedside: bool,
}

/// `ROWPREP_SCALEUP_VIOLNONZERO`: minimal violation for considering
/// up-scaling of rowprep (we want to avoid upscaling very small violations).
fn scaleup_violnonzero(scip: &Scip) -> f64 {
    10.0 * scip.set.epsilon()
}

/// `ROWPREP_SCALEUP_MINVIOLFACTOR`: scale up will target a violation of
/// ~MINVIOLFACTOR*minviol, where minviol is given by caller.
const ROWPREP_SCALEUP_MINVIOLFACTOR: f64 = 2.0;

/// `ROWPREP_SCALEUP_MAXMINCOEF`: scale up only if min. coef is below this
/// number (before scaling).
fn scaleup_maxmincoef(scip: &Scip) -> f64 {
    1.0 / scip.set.feastol()
}

/// `ROWPREP_SCALEUP_MAXMAXCOEF`: scale up only if max. coef will not exceed
/// this number by scaling.
fn scaleup_maxmaxcoef(scip: &Scip) -> f64 {
    scip.set.get_huge_value()
}

/// `ROWPREP_SCALEUP_MAXSIDE`: scale up only if side will not exceed this
/// number by scaling.
fn scaleup_maxside(scip: &Scip) -> f64 {
    scip.set.get_huge_value()
}

/// `ROWPREP_SCALEDOWN_MINMAXCOEF`: scale down if max. coef is at least this
/// number (before scaling).
fn scaledown_minmaxcoef(scip: &Scip) -> f64 {
    1.0 / scip.set.feastol()
}

/// `ROWPREP_SCALEDOWN_MINCOEF`: scale down only if min. coef does not drop
/// below this number by scaling.
fn scaledown_mincoef(scip: &Scip) -> f64 {
    scip.set.feastol()
}

/// `SCIPgetSepaMinEfficacy`: minimal efficacy for a cut to enter the LP.
///
/// (As in the C: the root value applies below the root.)
pub fn get_sepa_min_efficacy(scip: &Scip) -> f64 {
    if crate::tree::tree_get_current_depth(scip) != 0 {
        return scip.set.sepa_minefficacyroot;
    }
    scip.set.sepa_minefficacy
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

impl RowPrep {
    /// `rowprepRecordModifiedVar`: adds a variable to the
    /// `rowprep->modifiedvars` array, if recording of modification has been
    /// enabled and the variable is not fixed.
    fn record_modified_var(&mut self, scip: &Scip, var: VarId) {
        if !self.recordmodifications {
            return;
        }

        // Do not record for fixed variables, as they are not suitable for
        // branching.
        let v = &scip.vars[var];
        if scip.set.is_rel_eq(v.locdom.lb, v.locdom.ub) {
            return;
        }

        self.modifiedvars.push(var);
    }

    /// `rowprepCleanupSortTerms`: sort terms by absolute value of
    /// coefficients, from largest to smallest.
    fn cleanup_sort_terms(&mut self) {
        // Special treatment for cuts with few variables.
        match self.vars.len() {
            0 | 1 => {}
            2 => {
                if self.coefs[0].abs() < self.coefs[1].abs() {
                    self.coefs.swap(0, 1);
                    self.vars.swap(0, 1);
                }
            }
            _ => {
                let mut abscoefs: Vec<f64> = self.coefs.iter().map(|c| c.abs()).collect();
                crate::sort::sort_down(&mut abscoefs, (&mut self.coefs[..], &mut self.vars[..]), crate::sort::cmp_real);
            }
        }

        // Forget about coefs that are exactly zero (unlikely to have some).
        while !self.vars.is_empty() && *self.coefs.last().unwrap() == 0.0 {
            self.vars.pop();
            self.coefs.pop();
        }
    }

    /// `rowprepCleanupImproveCoefrange`: try to improve coef range by
    /// aggregating row with variable bounds.
    ///
    /// Assumes terms have been sorted by rowprepCleanupSortTerms().
    fn cleanup_improve_coefrange(&mut self, scip: &Scip, sol: SolId, maxcoefrange: f64) {
        let mut maxcoefidx = 0usize;
        let mut nvars = self.vars.len();
        let mut loss = [0.0f64; 2];

        let (mut maxcoef, mut mincoef) = if nvars > 0 { (self.coefs[0].abs(), self.coefs[nvars - 1].abs()) } else { (1.0, 1.0) };

        // Eliminate minimal or maximal coefs as long as coef range is too large;
        // this is likely going to eliminate coefs that are within eps of 0.0; if
        // not, then we do so after scaling (or should we enforce this here?).
        while maxcoef / mincoef > maxcoefrange {
            // Try to reduce coef range by aggregating with variable bounds; that is,
            // eliminate a term like a*x from a*x + ... <= side by adding -a*x <=
            // -a*lb(x); with ref(x) the reference point we try to eliminate, this
            // would weaken the cut by a*(lb(x)-ref(x)).
            //
            // We consider eliminating either the term with maximal or the one with
            // minimal coefficient, taking the one that leads to the least weakening
            // of the cut.
            for (pos, loss_pos) in loss[..2].iter_mut().enumerate() {
                let idx = if pos == 1 { nvars - 1 } else { maxcoefidx };
                let var = self.vars[idx];
                let coef = self.coefs[idx];
                let lb = scip.vars[var].locdom.lb;
                let ub = scip.vars[var].locdom.ub;
                let mut r = crate::api::get_sol_val(scip, sol, var);

                // Make sure reference point is something reasonable within the bounds,
                // preferable the value from the solution.
                if scip.set.is_infinity(r.abs()) {
                    r = 0.0;
                }
                r = cmax(lb, cmin(ub, r));

                // Check whether we can eliminate coef*var from rowprep and how much we
                // would loose w.r.t. ref(x).
                if (coef > 0.0 && self.sidetype == SideType::Right) || (coef < 0.0 && self.sidetype == SideType::Left) {
                    // We would need to aggregate with -coef*var <= -coef*lb(x).
                    *loss_pos = if scip.set.is_infinity(-lb) { INVALID } else { coef.abs() * (r - lb) };
                } else {
                    // We would need to aggregate with -coef*var >= -coef*ub(x).
                    *loss_pos = if scip.set.is_infinity(ub) { INVALID } else { coef.abs() * (ub - r) };
                }
            }

            if loss[0] == INVALID && loss[1] == INVALID {
                break; // Cannot eliminate coefficient.
            }

            // Select position with smaller loss.
            let pos = if loss[1] == INVALID || loss[1] > loss[0] { 0 } else { 1 };

            // Now do the actual elimination.
            let idx = if pos == 1 { nvars - 1 } else { maxcoefidx };
            let var = self.vars[idx];
            let coef = self.coefs[idx];

            // Eliminate coef*var from rowprep: increase side.
            let v = &scip.vars[var];
            if (coef > 0.0 && self.sidetype == SideType::Right) || (coef < 0.0 && self.sidetype == SideType::Left) {
                // We aggregate with -coef*var <= -coef*lb(x).
                self.add_constant(coef * v.locdom.lb);
                self.local |= scip.set.is_gt(v.locdom.lb, v.glbdom.lb);
            } else {
                // We aggregate with -coef*var >= -coef*ub(x).
                self.add_constant(coef * v.locdom.ub);
                self.local |= scip.set.is_lt(v.locdom.ub, v.glbdom.ub);
            }

            // Eliminate coef*var from rowprep: remove coef.
            if pos == 0 {
                // Set first term to zero.
                self.coefs[maxcoefidx] = 0.0;

                // Update index.
                maxcoefidx += 1;

                // Update maxcoef.
                maxcoef = self.coefs[maxcoefidx].abs();
            } else {
                // Forget last term.
                nvars -= 1;

                // Update mincoef.
                mincoef = self.coefs[nvars - 1].abs();
            }

            // (Potentially) remember the variable that has been removed here.
            self.record_modified_var(scip, var);
        }

        self.vars.truncate(nvars);
        self.coefs.truncate(nvars);

        // If maximal coefs were removed, then there are now 0's in the beginning
        // of the coefs array -> move all remaining coefs and vars up front.
        if maxcoefidx > 0 {
            self.vars.drain(..maxcoefidx);
            self.coefs.drain(..maxcoefidx);
        }
    }

    /// `rowprepCleanupScaleup`: scales up rowprep if it seems useful.
    fn cleanup_scaleup(&mut self, scip: &Scip, viol: &mut f64, minviol: f64) {
        // If violation is very small than better don't scale up.
        if *viol < scaleup_violnonzero(scip) {
            return;
        }

        // If violation is already above minviol, then nothing to do.
        if *viol >= minviol {
            return;
        }

        // If violation is sufficiently positive (>10*eps), but has not reached
        // minviol, then consider scaling up to reach approx MINVIOLFACTOR*minviol.
        let scalefactor = ROWPREP_SCALEUP_MINVIOLFACTOR * minviol / *viol;

        // Scale by approx. scalefactor, if minimal coef is not so large yet and
        // maximal coef and rhs don't get huge by doing so (or have been so
        // before).
        let n = self.vars.len();
        let mincoef = if n > 0 { self.coefs[n - 1].abs() } else { 1.0 };
        let maxcoef = if n > 0 { self.coefs[0].abs() } else { 1.0 };
        if mincoef < scaleup_maxmincoef(scip) && scalefactor * maxcoef < scaleup_maxmaxcoef(scip) && scalefactor * self.side.abs() < scaleup_maxside(scip) {
            // SCIPscaleRowprep returns the actually applied scale factor.
            let scaleexp = self.scale(scalefactor);
            *viol = ldexp(*viol, scaleexp);
        }
    }

    /// `rowprepCleanupScaledown`: scales down rowprep if it improves coefs and
    /// keeps rowprep violated.
    fn cleanup_scaledown(&mut self, scip: &Scip, viol: &mut f64, minviol: f64) {
        // If maxcoef < ROWPREP_SCALEDOWN_MINMAXCOEF (or no terms), then don't
        // consider scaling down.
        if self.vars.is_empty() || self.coefs[0].abs() < scaledown_minmaxcoef(scip) {
            return;
        }

        // Consider scaling down so that maxcoef ~ 10.
        let mut scalefactor = 10.0 / self.coefs[0].abs();

        // If minimal violation would be lost by scaling down, then increase
        // scalefactor such that minviol is still reached.
        if *viol > minviol && !scip.set.is_infinity(*viol) && scalefactor * *viol < minviol {
            scalefactor = ROWPREP_SCALEUP_MINVIOLFACTOR * minviol / *viol;
        }

        // Scale by approx. scalefactor if scaling down and minimal coef does not
        // get too small; myviol < minviol (-> scalefactor > 1) or mincoef <
        // feastol before scaling is possible, in which case we also don't scale
        // down.
        if scalefactor < 1.0 && scalefactor * self.coefs[self.vars.len() - 1].abs() > scaledown_mincoef(scip) {
            let scaleexp = self.scale(scalefactor);
            if !scip.set.is_infinity(*viol) {
                *viol = ldexp(*viol, scaleexp);
            }
        }
    }

    /// `rowprepCleanupIntegralCoefs`: rounds almost integral coefs to
    /// integrals, thereby trying to relax the cut.
    fn cleanup_integral_coefs(&mut self, scip: &Scip, mut viol: Option<&mut f64>) {
        // Coefficients smaller than epsilon are rounded to 0.0 when added to row
        // and coefficients very close to integral values are rounded to integers
        // when added to LP. Both cases can be problematic if variable value is
        // very large (bad numerics). Thus, we anticipate by rounding coef here,
        // but also modify constant so that cut is still valid (if possible), i.e.,
        // bound coef[i]*x by round(coef[i])*x + (coef[i]-round(coef[i])) *
        // bound(x). Or in other words, we aggregate with the variable bound.
        //
        // If the required bound of x is not finite, then only round coef
        // (introduces an error).
        // @TODO If only the opposite bound is available, then one could move the
        // coefficient away from the closest integer so that the SCIP_ROW won't
        // try to round it.
        //
        // Exception: If the coefficient is almost zero and there is only one
        // variable, then we scale up the row instead (if that doesn't make the
        // side too huge). Eventually, the row would have been changed to a
        // boundchange anyway and a similar operation happens, but we need to
        // ensure that coefficient isn't just rounded to 0 first.
        if self.vars.len() == 1 && self.coefs[0] != 0.0 && scip.set.is_zero(self.coefs[0]) && (self.side / self.coefs[0]).abs() < scaleup_maxside(scip) {
            let _ = self.scale((1.0 / self.coefs[0]).abs());
            return;
        }

        for i in 0..self.vars.len() {
            let coef = self.coefs[i];
            let roundcoef = scip.set.round(coef);
            if coef != roundcoef && scip.set.is_eq(coef, roundcoef) {
                let var = self.vars[i];
                let v = &scip.vars[var];

                let xbnd = if self.sidetype == SideType::Right {
                    if self.local {
                        if coef > roundcoef { v.locdom.lb } else { v.locdom.ub }
                    } else if coef > roundcoef {
                        v.glbdom.lb
                    } else {
                        v.glbdom.ub
                    }
                } else if self.local {
                    if coef > roundcoef { v.locdom.ub } else { v.locdom.lb }
                } else if coef > roundcoef {
                    v.glbdom.ub
                } else {
                    v.glbdom.lb
                };

                if !scip.set.is_infinity(xbnd.abs()) {
                    // If there is a bound, then relax row side so rounding coef will not
                    // introduce an error.
                    self.add_constant((coef - roundcoef) * xbnd);
                }
                // Else: if there is no bound, then we make the coef integral, too, even
                // though this will introduce an error; however, SCIP_ROW would do this
                // anyway, but doing this here might eliminate some epsilon coefs (so
                // they don't determine mincoef below) and helps to get a more accurate
                // row violation value.

                self.coefs[i] = roundcoef;
                if let Some(v) = viol.as_deref_mut() {
                    *v = INVALID;
                }

                // (Potentially) remember the variable which coef has been modified
                // here.
                self.record_modified_var(scip, var);
            }
        }

        // Forget about coefs that became exactly zero by the above step.
        while !self.vars.is_empty() && *self.coefs.last().unwrap() == 0.0 {
            self.vars.pop();
            self.coefs.pop();
        }
    }

    /// `rowprepCleanupSide`: relaxes almost zero side.
    fn cleanup_side(&mut self, scip: &Scip, viol: Option<&mut f64>) {
        // SCIP_ROW handling will replace a side close to 0 by 0.0, even if that
        // makes the row more restrictive; we thus relax the side here so that it
        // will either be 0 now or will not be rounded to 0 later.
        if self.side == 0.0 || !scip.set.is_zero(self.side) {
            return;
        }

        if self.side > 0.0 && self.sidetype == SideType::Right {
            self.side = 1.1 * scip.set.epsilon();
        } else if self.side < 0.0 && self.sidetype == SideType::Left {
            self.side = -1.1 * scip.set.epsilon();
        } else {
            self.side = 0.0;
        }

        if self.recordmodifications {
            self.modifiedside = true;
        }

        if let Some(v) = viol {
            *v = INVALID;
        }
    }

    /// `SCIPcreateRowprep`: creates a SCIP_ROWPREP datastructure.
    ///
    /// Initial row represents 0 <= 0.
    pub fn new(sidetype: SideType, local: bool) -> Self {
        Self {
            vars: Vec::new(),
            coefs: Vec::new(),
            side: 0.0,
            sidetype,
            local,
            name: String::new(),
            recordmodifications: false,
            modifiedvars: Vec::new(),
            modifiedside: false,
        }
    }

    /// `SCIPcopyRowprep`: creates a copy of a SCIP_ROWPREP datastructure.
    pub fn copy(&self) -> Self {
        let mut target = self.clone();
        target.recordmodifications = false;
        target.modifiedvars = Vec::new();
        target.modifiedside = false;
        target
    }

    /// `SCIProwprepGetNVars`: gives number of terms in rowprep.
    #[inline]
    pub fn get_n_vars(&self) -> usize {
        self.vars.len()
    }

    /// `SCIProwprepReset`: resets rowprep to have 0 terms and side 0.0.
    pub fn reset(&mut self) {
        self.vars.clear();
        self.coefs.clear();
        self.side = 0.0;
        self.recordmodifications = false;
        self.modifiedvars.clear();
        self.modifiedside = false;
    }

    /// `SCIProwprepSetCoef`: sets coefficient idx of rowprep.
    #[inline]
    pub fn set_coef(&mut self, idx: usize, newcoef: f64) {
        self.coefs[idx] = newcoef;
    }

    /// `SCIProwprepAddSide`: adds constant value to side of rowprep.
    #[inline]
    pub fn add_side(&mut self, side: f64) {
        self.side += side;
    }

    /// `SCIProwprepAddConstant`: adds constant term to rowprep; substracts
    /// constant from side.
    #[inline]
    pub fn add_constant(&mut self, constant: f64) {
        self.add_side(-constant);
    }

    /// `SCIProwprepRecordModifications`: enables recording for where
    /// modifications were done in cleanup.
    #[inline]
    pub fn record_modifications(&mut self) {
        self.recordmodifications = true;
    }

    /// `SCIPensureRowprepSize`: ensures that rowprep has space for at least
    /// given number of additional terms.
    pub fn ensure_size(&mut self, size: usize) {
        self.vars.reserve(size);
        self.coefs.reserve(size);
    }

    /// `SCIPaddRowprepTerm`: adds a term coef*var to a rowprep.
    pub fn add_term(&mut self, var: VarId, coef: f64) {
        if coef == 0.0 {
            return;
        }

        self.vars.push(var);
        self.coefs.push(coef);
    }

    /// `SCIPaddRowprepTerms`: adds several terms coef*var to a rowprep.
    pub fn add_terms(&mut self, vars: &[VarId], coefs: &[f64]) {
        self.vars.extend_from_slice(vars);
        self.coefs.extend_from_slice(&coefs[..vars.len()]);
    }

    /// `SCIPgetRowprepViolation`: computes violation of rowprep in a given
    /// solution.
    ///
    /// Can return whether the violation value is reliable from a
    /// floating-point accuracy point of view. The value will not be deemed
    /// reliable when its calculation involved the subtraction of large
    /// numbers. To be precise, the violation of an inequality sum_i a_ix_i <= b
    /// in a solution x^* is deemed reliable if |sum_i a_ix^*_i - b| >= 2^{-50}
    /// max (|b|, max_i |a_ix^*_i|).
    pub fn get_violation(&self, scip: &Scip, sol: SolId, mut reliable: Option<&mut bool>) -> f64 {
        let mut activity = 0.0;
        let mut maxterm = self.side.abs();

        for i in 0..self.vars.len() {
            // Loose variable have the best bound as LP solution value. HOWEVER,
            // they become column variables when they are added to a row (via
            // SCIPaddVarsToRow below). When this happens, their LP solution value
            // changes to 0.0! So when calculating the row activity for an LP
            // solution, we treat loose variable as if they were already column
            // variables.
            if sol != crate::var::NONE || scip.vars[self.vars[i]].varstatus != VarStatus::Loose {
                let val = crate::api::get_sol_val(scip, sol, self.vars[i]);

                // If a variable is at infinity, then this should lead to an immediate
                // decision. Having different contradicting infinities is something I
                // would now know how to handle and am ignoring now.
                if scip.set.is_infinity(val * if self.coefs[i] >= 0.0 { 1.0 } else { -1.0 }) {
                    // activity = SCIPinfinity(scip);
                    if let Some(r) = reliable.as_deref_mut() {
                        *r = true;
                    }
                    return if self.sidetype == SideType::Right {
                        scip.set.infinity() // infinity <= side -> always violated
                    } else {
                        0.0 // infinity >= side -> never violated
                    };
                }
                if scip.set.is_infinity(val * if self.coefs[i] >= 0.0 { -1.0 } else { 1.0 }) {
                    // activity = -SCIPinfinity(scip);
                    if let Some(r) = reliable.as_deref_mut() {
                        *r = true;
                    }
                    return if self.sidetype == SideType::Right {
                        0.0 // -infinity <= side -> never violated
                    } else {
                        scip.set.infinity() // -infinity >= side -> always violated
                    };
                }

                let term = self.coefs[i] * val;
                activity += term;

                if reliable.is_some() && term.abs() > maxterm {
                    maxterm = term.abs();
                }
            }
        }

        let violation = if self.sidetype == SideType::Right {
            // Cut is activity <= side -> violation is activity - side (if positive).
            activity - self.side
        } else {
            // Cut is activity >= side -> violation is side - activity (if positive).
            self.side - activity
        };

        // In double precision, the mantissa (or significand) of a floating point
        // number has 52 bit. Therefore, if the exponent in the violation is 52 (or
        // more) less than the one of maxterm, then it is essentially random. We
        // require here that the exponents differ by at most 50. To be more robust
        // w.r.t. scaling of the row, we look at the exponent of the quotient
        // maxterm/violation instead of the difference of the exponents of maxterm
        // and violation.
        if let Some(r) = reliable {
            if violation != 0.0 {
                // Difference in exponents for maxterm and violation.
                let (_, exponent) = frexp(maxterm / violation);
                *r = exponent <= 50;
            } else {
                *r = true; // Not clear how to evaluate reliability here, so think positive.
            }
        }

        cmax(violation, 0.0)
    }

    /// `SCIPisRowprepViolationReliable`: computes violation of rowprep in a
    /// given solution and reports whether that value seem numerically reliable.
    pub fn is_violation_reliable(&self, scip: &Scip, sol: SolId) -> bool {
        let mut reliable = false;
        let _ = self.get_violation(scip, sol, Some(&mut reliable));
        reliable
    }

    /// Moves term `i` into the side if its variable is fixed (part of
    /// `SCIPmergeRowprepTerms`).
    fn merge_fixed_term(&mut self, scip: &Scip, i: usize) {
        let v = &scip.vars[self.vars[i]];
        if self.local && scip.set.is_rel_eq(v.locdom.lb, v.locdom.ub) {
            if (self.coefs[i] > 0.0) == (self.sidetype == SideType::Right) {
                self.side -= self.coefs[i] * v.locdom.lb;
            } else {
                self.side -= self.coefs[i] * v.locdom.ub;
            }
            self.coefs[i] = 0.0; // So will be cleaned out below.
        } else if !self.local && scip.set.is_rel_eq(v.glbdom.lb, v.glbdom.ub) {
            if (self.coefs[i] > 0.0) == (self.sidetype == SideType::Right) {
                self.side -= self.coefs[i] * v.glbdom.lb;
            } else {
                self.side -= self.coefs[i] * v.glbdom.ub;
            }
            self.coefs[i] = 0.0; // So will be cleaned out below.
        }
    }

    /// `SCIPmergeRowprepTerms`: merge terms that use same variable and
    /// eliminate zero coefficients.
    ///
    /// Removes a variable if its bounds have a relative difference of below
    /// epsilon. Local bounds are checked for local rows, otherwise global bounds
    /// are used. If the bounds are not absolute equal, the bound that relaxes
    /// the row is used.
    ///
    /// Terms are sorted by variable (see SCIPvarComp()) after return.
    pub fn merge_terms(&mut self, scip: &Scip) {
        let nvars = self.vars.len();
        if nvars <= 1 {
            return;
        }

        // Sort terms by variable index.
        crate::sort::sort(&mut self.vars[..], (&mut self.coefs[..],), |&a, &b| crate::var::var_comp(scip, a, b));

        // Merge terms with same variable, drop 0 coefficients.
        let mut i: isize = 0;
        let mut j = 1usize;
        while j < nvars {
            let iu = i as usize;
            if self.vars[iu] == self.vars[j] {
                // Merge term j into term i.
                self.coefs[iu] += self.coefs[j];
                j += 1;
                continue;
            }

            // Move term i into side if fixed.
            self.merge_fixed_term(scip, iu);

            if self.coefs[iu] == 0.0 {
                // Move term j to position i.
                self.coefs[iu] = self.coefs[j];
                self.vars[iu] = self.vars[j];
                j += 1;
                continue;
            }

            // Move term j to position i+1 and move on.
            if j != iu + 1 {
                self.vars[iu + 1] = self.vars[j];
                self.coefs[iu + 1] = self.coefs[j];
            }
            i += 1;
            j += 1;
        }

        // Move term i into side if fixed.
        self.merge_fixed_term(scip, i as usize);

        // Remaining term can have coef zero -> forget about it.
        if self.coefs[i as usize] == 0.0 {
            i -= 1;
        }

        // i points to last term.
        let n = (i + 1) as usize;
        self.vars.truncate(n);
        self.coefs.truncate(n);
    }

    /// `SCIPcleanupRowprep`: cleans up and attempts to improve rowprep.
    ///
    /// Drops small or large coefficients if coefrange is too large, if this can
    /// be done by relaxing the row. Scales coefficients up to reach minimal
    /// violation, if possible. Scaling is omitted if violation is very small
    /// (ROWPREP_SCALEUP_VIOLNONZERO) or maximal coefficient would become huge
    /// (ROWPREP_SCALEUP_MAXMAXCOEF). Scales coefficients and side down if they
    /// are large and if the minimal violation is still reached. Rounds
    /// coefficients close to integral values to integrals, if this can be done
    /// by relaxing the row. Rounds side within epsilon of 0 to 0.0 or
    /// +/-1.1*epsilon, whichever relaxes the row least.
    ///
    /// After return, the terms in the rowprep will be sorted by absolute value
    /// of coefficient, in decreasing order.
    ///
    /// `success` is set to TRUE if and only if the rowprep satisfies the
    /// following:
    /// - the coefrange is below `maxcoefrange`
    /// - the violation is at least `minviol`
    /// - the violation is reliable or `minviol` = 0
    /// - the absolute value of coefficients are below SCIPinfinity()
    /// - the absolute value of the side is below SCIPinfinity()
    pub fn cleanup(&mut self, scip: &Scip, sol: SolId, minviol: f64, viol: Option<&mut f64>, success: Option<&mut bool>) {
        let mut violreliable = true;

        let maxcoefrange = scip.set.get_sepa_max_coef_ratio_rowprep();

        if self.recordmodifications {
            // Forget about possible previous modifications.
            self.modifiedvars.clear();
            self.modifiedside = false;
        }

        // Sort term by absolute value of coef.
        self.cleanup_sort_terms();

        // Improve coefficient range by aggregating out variables.
        self.cleanup_improve_coefrange(scip, sol, maxcoefrange);

        // Get current violation in sol (reliability info only needed if success
        // is not NULL).
        let mut myviol = self.get_violation(scip, sol, if success.is_some() { Some(&mut violreliable) } else { None });

        // If there is interest in achieving some minimal violation, then possibly
        // scale up to increase violation; this updates myviol; since this is only
        // scaling the cut, it doesn't change anything about the reliability of
        // the violation value.
        if minviol > 0.0 {
            // First, try to achieve scip's minefficacy (typically 1e-4).
            if get_sepa_min_efficacy(scip) > minviol {
                self.cleanup_scaleup(scip, &mut myviol, get_sepa_min_efficacy(scip));
            }
            // In case scip minefficacy could not be reached or was smaller than
            // minviol, try with the given minviol.
            self.cleanup_scaleup(scip, &mut myviol, minviol);
        }

        // Scale down to improve numerics, updates myviol (reliability doesn't
        // change).
        self.cleanup_scaledown(scip, &mut myviol, cmax(get_sepa_min_efficacy(scip), minviol));

        // Turn almost-integral coefs to integral values, may set myviol to
        // SCIP_INVALID.
        self.cleanup_integral_coefs(scip, Some(&mut myviol));

        // Relax almost-zero side, may set myviol to SCIP_INVALID.
        self.cleanup_side(scip, Some(&mut myviol));

        // Check whether rowprep could be turned into a reasonable row.
        if let Some(success) = success {
            *success = true;
            let n = self.vars.len();

            // Check whether the coef.range is below maxcoefrange.
            if n > 0 && self.coefs[0].abs() / self.coefs[n - 1].abs() > maxcoefrange {
                *success = false;
            }

            // Check whether coefficients are below SCIPinfinity (terms are order by
            // coef value).
            if *success && n > 0 && scip.set.is_infinity(self.coefs[0].abs()) {
                *success = false;
            }

            // Check whether the absolute value of the side is below SCIPinfinity.
            if *success && scip.set.is_infinity(self.side.abs()) {
                *success = false;
            }

            // Check if violation is at least minviol and reliable, if minviol > 0.
            if *success && minviol > 0.0 {
                // May need to recompute violation if coefs or side was modified above.
                if myviol == INVALID {
                    myviol = self.get_violation(scip, sol, Some(&mut violreliable));
                }

                if !violreliable || myviol < minviol {
                    *success = false;
                }
            }
        }

        // If we updated myviol correctly, then it should coincide with freshly
        // computed violation. I leave this assert off for now, since getting the
        // tolerance in the EQ correctly is not trivial. We recompute viol below
        // anyway.

        // Compute final violation, if requested by caller.
        if let Some(viol) = viol {
            *viol = if myviol == INVALID { self.get_violation(scip, sol, None) } else { myviol };
        }
    }

    /// `SCIPcleanupRowprep2`: cleans up and attempts to improve rowprep
    /// without regard for violation.
    ///
    /// Drops small or large coefficients if their ratio is beyond
    /// separating/maxcoefratiofacrowprep / numerics/feastol, if this can be done
    /// by relaxing the row. Scales coefficients and side to have maximal
    /// coefficient in `[1/maxcoefbound,maxcoefbound]`. Rounds coefficients close
    /// to integral values to integrals, if this can be done by relaxing the
    /// row. Rounds side within epsilon of 0 to 0.0 or +/-1.1*epsilon, whichever
    /// relaxes the row least.
    ///
    /// After return, the terms in the rowprep will be sorted by absolute value
    /// of coefficient, in decreasing order.
    ///
    /// `success` is set to TRUE if and only if the rowprep satisfies the
    /// following:
    /// - the coefratio is below separating/maxcoefratiofacrowprep /
    ///   numerics/feastol
    /// - the absolute value of coefficients are below SCIPinfinity()
    /// - the absolute value of the side is below SCIPinfinity()
    ///
    /// In difference to SCIPcleanupRowprep(), this function does not scale up
    /// the row to increase the absolute violation.
    pub fn cleanup2(&mut self, scip: &Scip, sol: SolId, maxcoefbound: f64, success: Option<&mut bool>) {
        let maxcoefrange = scip.set.get_sepa_max_coef_ratio_rowprep();

        if self.recordmodifications {
            // Forget about possible previous modifications.
            self.modifiedvars.clear();
            self.modifiedside = false;
        }

        // Sort term by absolute value of coef.
        self.cleanup_sort_terms();

        // Improve coefficient range by aggregating out variables.
        self.cleanup_improve_coefrange(scip, sol, maxcoefrange);

        // Scale up or down to improve numerics; if maximal coef is below
        // 1.0/maxcoefbound, scale up to reach ~ 1.0/maxcoefbound; if maximal coef
        // is above maxcoefbound, scale down to ~ maxcoefbound.
        if !self.vars.is_empty() && !scip.set.is_infinity(maxcoefbound) {
            if self.coefs[0].abs() < 1.0 / maxcoefbound {
                let _ = self.scale((1.0 / maxcoefbound) / self.coefs[0].abs());
            } else if self.coefs[0].abs() > maxcoefbound {
                let _ = self.scale(maxcoefbound / self.coefs[0].abs());
            }
        }

        // Turn almost-integral coefs to integral values.
        self.cleanup_integral_coefs(scip, None);

        // Relax almost-zero side.
        self.cleanup_side(scip, None);

        // Check whether rowprep could be turned into a reasonable row.
        if let Some(success) = success {
            *success = true;
            let n = self.vars.len();

            // Check whether the coef.range is below maxcoefrange.
            if n > 0 && self.coefs[0].abs() / self.coefs[n - 1].abs() > maxcoefrange {
                *success = false;
            }

            // Check whether coefficients are below SCIPinfinity (terms are order by
            // coef value).
            if *success && n > 0 && scip.set.is_infinity(self.coefs[0].abs()) {
                *success = false;
            }

            // Check whether the absolute value of the side is below SCIPinfinity.
            if *success && scip.set.is_infinity(self.side.abs()) {
                *success = false;
            }
        }
    }

    /// `SCIPscaleupRowprep`: scales up a rowprep to increase coefficients/sides
    /// that are within epsilon to an integer value, if possible.
    ///
    /// Computes the minimal fractionality of all fractional coefficients and the
    /// side of the rowprep. If this fractionality is below epsilon, the rowprep
    /// is scaled up such that the fractionality exceeds epsilon, if this will
    /// not put any coefficient or side above SCIPhugeValue().
    ///
    /// This function does not relax the rowprep.
    ///
    /// `success` is set to TRUE if the resulting rowprep can be turned into a
    /// SCIP_ROW, that is, all coefs and the side is below SCIPinfinity() and
    /// fractionalities are above epsilon. If `success` is set to FALSE, then the
    /// rowprep will not have been modified.
    ///
    /// Returns the applied scaling factor, if `success` is set to TRUE.
    pub fn scaleup(&mut self, scip: &Scip, minscaleup: f64, success: Option<&mut bool>) -> f64 {
        let mut minfrac = 0.5;
        let mut minfrac0 = 0.5;
        let mut factor;
        let mut makeintegral = true;

        // Find the smallest fractionality in rowprep sides and coefficients and
        // the largest absolute coefficient/side.
        let frac = ((self.side + 0.5).floor() - self.side).abs();
        if frac != 0.0 {
            if self.side.abs() > 0.5 {
                if frac < minfrac {
                    minfrac = frac;
                }
            } else if frac < minfrac0 {
                minfrac0 = frac;
            }
        }
        let mut maxval = self.side.abs();

        for coef in &self.coefs[..self.vars.len()] {
            let frac = ((*coef + 0.5).floor() - *coef).abs();
            if frac != 0.0 {
                if coef.abs() > 0.5 {
                    if frac < minfrac {
                        minfrac = frac;
                    }
                } else if frac < minfrac0 {
                    minfrac0 = frac;
                }
            }
            if coef.abs() > maxval {
                maxval = coef.abs();
            }
        }

        // In order for SCIP_ROW to not modify the coefs and side, they need to be
        // more than epsilon way from an integer value.
        //
        // If the integer value is 0, then scaling up the rowprep by
        // epsilon/minfrac will increase its minimal fractionality above epsilon.
        // If the integer value is not zero, then scaling up the rowprep by a
        // well-chosen fractional number alpha will increase the minimal
        // fractionality by about alpha*integer-value mod 1. To reduce the chance
        // that alpha*integer-value is integral, we use a "very fractional" value
        // for alpha.
        //
        // If the scaling increases the maximal coef/value beyond SCIPinfinity,
        // then the rowprep would be useless. We even check that we don't increase
        // beyond SCIPhugeValue here.
        if minfrac0 <= scip.set.epsilon() {
            factor = 1.1 * scip.set.epsilon() / minfrac0;

            if factor < minscaleup {
                factor = minscaleup;
            }
        } else if minfrac <= scip.set.epsilon() {
            factor = cmax(std::f64::consts::SQRT_2, minscaleup);
            makeintegral = false;
        } else if minscaleup > 1.0 {
            factor = minscaleup;
        } else {
            // Do not scale up, only check whether maxval is already below infinity.
            if let Some(success) = success {
                *success = !scip.set.is_infinity(maxval);
            }

            return 1.0;
        }

        if !scip.set.is_huge_value(factor * maxval) {
            if makeintegral {
                factor = self.scale(factor) as f64;
            } else {
                // Multiply each coefficient by factor.
                for c in self.coefs.iter_mut() {
                    *c *= factor;
                }

                // Multiply side by factor.
                self.side *= factor;
            }

            if let Some(success) = success {
                *success = true;
            }
        } else if let Some(success) = success {
            *success = false;
        }

        factor
    }

    /// `SCIPscaleRowprep`: scales a rowprep by given factor (after some
    /// rounding); returns the exponent of actually applied scaling factor, if
    /// written as 2^x.
    pub fn scale(&mut self, factor: f64) -> i32 {
        // Write factor as v*2^expon with v in [0.5,1).
        let (v, mut expon) = frexp(factor);
        // Adjust to v'*2^expon with v' in (0.5,1] by v'=v if v > 0.5, v'=1 if
        // v=0.5.
        if v == 0.5 {
            expon -= 1;
        }

        // Multiply each coefficient by 2^expon.
        for c in self.coefs.iter_mut() {
            *c = ldexp(*c, expon);
        }

        // Multiply side by 2^expon.
        self.side = ldexp(self.side, expon);

        expon
    }

    /// The sides of the row this rowprep becomes.
    fn row_sides(&self, scip: &Scip) -> (f64, f64) {
        (
            if self.sidetype == SideType::Left { self.side } else { -scip.set.infinity() },
            if self.sidetype == SideType::Right { self.side } else { scip.set.infinity() },
        )
    }

    /// `SCIPgetRowprepRowConshdlr`: generates a SCIP_ROW from a rowprep,
    /// setting its origin to given constraint handler.
    pub fn get_row_conshdlr(&self, scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult<RowId> {
        let (lhs, rhs) = self.row_sides(scip);
        let local = self.local && crate::api::get_depth(scip) > 0;
        let row = crate::scip_lp::create_empty_row_conshdlr(scip, conshdlr, &self.name, lhs, rhs, local, false, true)?;
        crate::scip_lp::add_vars_to_row(scip, row, &self.vars, &self.coefs)?;
        Ok(row)
    }

    /// `SCIPgetRowprepRowCons`: generates a SCIP_ROW from a rowprep, setting
    /// its origin to given constraint.
    pub fn get_row_cons(&self, scip: &mut Scip, cons: ConsId) -> ScipResult<RowId> {
        let (lhs, rhs) = self.row_sides(scip);
        let local = self.local && crate::api::get_depth(scip) > 0;
        let row = crate::scip_lp::create_empty_row_cons(scip, cons, &self.name, lhs, rhs, local, false, true)?;
        crate::scip_lp::add_vars_to_row(scip, row, &self.vars, &self.coefs)?;
        Ok(row)
    }

    /// `SCIPgetRowprepRowSepa`: generates a SCIP_ROW from a rowprep, setting
    /// its origin to given separator.
    pub fn get_row_sepa(&self, scip: &mut Scip, sepa: SepaId) -> ScipResult<RowId> {
        let (lhs, rhs) = self.row_sides(scip);
        let local = self.local && crate::api::get_depth(scip) > 0;
        let row = crate::scip_lp::create_empty_row_sepa(scip, sepa, &self.name, lhs, rhs, local, false, true)?;
        crate::scip_lp::add_vars_to_row(scip, row, &self.vars, &self.coefs)?;
        Ok(row)
    }
}
