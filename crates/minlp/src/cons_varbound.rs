//! Port of `cons_varbound.c`: constraint handler for variable bound
//! constraints `lhs <= x + c y <= rhs`.
//!
//! A variable bound constraint has coefficient `c`, sides in `Q` or
//! infinite, and decision variables `x` (non-binary) and `y` (binary or
//! integer). Although `x` must be non-binary when the constraint is created,
//! it can happen that `x` is upgraded to a binary variable, e.g. due to
//! aggregations or bound changes in presolving.
//!
//! Not ported: printing, parsing, the NLP rows (the NLP is not ported), the
//! symmetry graphs and sub-SCIP copying.

use std::any::Any;

use crate::{
    api,
    cons::{ConsFlags, *},
    event::*,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var,
    types::{BoundType, ConsId, ConshdlrId, EventhdlrId, LockType, RowId, ScipResultCode, SolId, VarId, VarStatus},
    var::{BdChgIdx, NONE},
};

/* constraint handler properties */
pub const CONSHDLR_NAME: &str = "varbound";
const CONSHDLR_DESC: &str = "variable bounds  lhs <= x + c*y <= rhs, x non-binary, y non-continuous";
const CONSHDLR_SEPAPRIORITY: i32 = 900000;
const CONSHDLR_ENFOPRIORITY: i32 = -500000;
const CONSHDLR_CHECKPRIORITY: i32 = -500000;
const CONSHDLR_SEPAFREQ: i32 = 0;
const CONSHDLR_PROPFREQ: i32 = 1;
const CONSHDLR_EAGERFREQ: i32 = 100;
const CONSHDLR_MAXPREROUNDS: i32 = -1;
const CONSHDLR_DELAYSEPA: bool = false;
const CONSHDLR_DELAYPROP: bool = false;
const CONSHDLR_NEEDSCONS: bool = true;

const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST | PRESOLTIMING_MEDIUM;
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

const EVENTHDLR_NAME: &str = "varbound";
const EVENTHDLR_DESC: &str = "bound change event handler for variable bound constraints";

/// Priority of the constraint handler for upgrading of linear constraints.
const LINCONSUPGD_PRIORITY: i32 = 50000;

/* default parameter values */
/// Should pairwise constraint comparison be performed in presolving?
const DEFAULT_PRESOLPAIRWISE: bool = true;
/// Maximum coefficient in varbound constraint to be added as a row into LP.
const DEFAULT_MAXLPCOEF: f64 = 1e+09;
/// Should bound widening be used to initialize conflict analysis?
const DEFAULT_USEBDWIDENING: bool = true;

/// Maximal coefficient value after scaling.
const MAXSCALEDCOEF: i64 = 1000;

/// The events a variable of a varbound constraint is watched for.
const VARBOUND_EVENTS: EventType = EVENTTYPE_BOUNDTIGHTENED | EVENTTYPE_VARFIXED;

/*
 * Data structures
 */

/// `SCIP_ConsData`: variable bound constraint data.
pub struct ConsData {
    /// Coefficient c of bounding variable y.
    pub vbdcoef: f64,
    /// Left hand side of variable bound inequality.
    pub lhs: f64,
    /// Right hand side of variable bound inequality.
    pub rhs: f64,
    /// Variable x that has variable bound.
    pub var: VarId,
    /// Binary, integer or implicit integer bounding variable y.
    pub vbdvar: VarId,
    /// LP row, if constraint is already stored in LP row format.
    pub row: RowId,
    /// Is the variable bound constraint already presolved?
    pub presolved: bool,
    /// Are the globally valid variable bounds added?
    pub varboundsadded: bool,
    /// Was constraint changed since last aggregation round in preprocessing?
    pub changed: bool,
    /// Were the vbdcoef and all sides already tightened?
    pub tightened: bool,
}

/// `SCIP_ConshdlrData`: constraint handler data.
pub struct ConshdlrData {
    /// Event handler for bound change events.
    pub eventhdlr: EventhdlrId,
    /// Should pairwise constraint comparison be performed in presolving?
    pub presolpairwise: bool,
    /// Maximum coefficient in varbound constraint to be added as a row into
    /// LP.
    pub maxlpcoef: f64,
    /// Should bound widening be used to in conflict analysis?
    pub usebdwidening: bool,
}

/// `PROPRULE`: propagation rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum Proprule {
    /// Left hand side and bounds on y -> lower bound on x.
    Rule1 = 0,
    /// Left hand side and upper bound on x -> bound on y.
    Rule2 = 1,
    /// Right hand side and bounds on y -> upper bound on x.
    Rule3 = 2,
    /// Right hand side and lower bound on x -> bound on y.
    Rule4 = 3,
}

impl Proprule {
    fn from_i32(v: i32) -> Proprule {
        match v {
            0 => Proprule::Rule1,
            1 => Proprule::Rule2,
            2 => Proprule::Rule3,
            _ => Proprule::Rule4,
        }
    }
}

/// The constraint data of a varbound constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a varbound constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the varbound constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/*
 * Local methods
 */

/// `consVarboundComp`: compares two varbound constraints w.r.t. the indices
/// of the contained variables; -1 if x1 has the smaller index, or x1 = x2
/// and y1 has the smaller index, or x1 = x2, y1 = y2 and cons2 was recently
/// changed but cons1 not; 0 if x1 = x2, y1 = y2 and the changed status of
/// both is the same; +1 otherwise.
fn cons_varbound_comp(scip: &Scip, elem1: ConsId, elem2: ConsId) -> i32 {
    let (c1, c2) = (cd(scip, elem1), cd(scip, elem2));
    let (i1, i2) = (scip.vars[c1.var].index, scip.vars[c2.var].index);
    let (j1, j2) = (scip.vars[c1.vbdvar].index, scip.vars[c2.vbdvar].index);

    // Comparison is done over 3 ordered criteria: (i) variable index of
    // variable 1, (ii) variable index of variable 2, (iii) changed status.
    if i1 < i2 || (i1 == i2 && j1 < j2) || (i1 == i2 && j1 == j2 && !c1.changed && c2.changed) {
        -1
    } else if i1 == i2 && j1 == j2 && c1.changed == c2.changed {
        0
    } else {
        1
    }
}

/// `catchEvents`: catches events for variables; the event data is the
/// constraint.
fn catch_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    let (var, vbdvar) = (cd(scip, cons).var, cd(scip, cons).vbdvar);
    api::catch_var_event(scip, var, VARBOUND_EVENTS, eventhdlr, cons, None)?;
    api::catch_var_event(scip, vbdvar, VARBOUND_EVENTS, eventhdlr, cons, None)
}

/// `dropEvents`: drops events for variables.
fn drop_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult {
    let (var, vbdvar) = (cd(scip, cons).var, cd(scip, cons).vbdvar);
    api::drop_var_event(scip, var, VARBOUND_EVENTS, eventhdlr, cons, -1)?;
    api::drop_var_event(scip, vbdvar, VARBOUND_EVENTS, eventhdlr, cons, -1)
}

/// `consdataCreate`: creates a variable bound constraint data object.
fn consdata_create(scip: &mut Scip, var: VarId, vbdvar: VarId, mut vbdcoef: f64, mut lhs: f64, mut rhs: f64) -> ScipResult<ConsData> {
    let s = &scip.set;
    let inf = s.infinity();
    if s.is_infinity(rhs) {
        rhs = inf;
    } else if s.is_infinity(-rhs) {
        rhs = -inf;
    }

    if s.is_infinity(-lhs) {
        lhs = -inf;
    } else if s.is_infinity(lhs) {
        lhs = inf;
    }

    if s.is_gt(lhs, rhs) {
        // Left hand side of varbound constraint greater than right hand side.
        return Err(Retcode::InvalidData);
    }

    if s.is_zero(vbdcoef) {
        // Varbound coefficient must be different to zero.
        return Err(Retcode::InvalidData);
    }

    if s.is_infinity(vbdcoef) {
        vbdcoef = inf;
    } else if s.is_infinity(-vbdcoef) {
        vbdcoef = -inf;
    }

    let mut c = ConsData {
        var,
        vbdvar,
        vbdcoef,
        lhs,
        rhs,
        row: NONE,
        presolved: false,
        varboundsadded: false,
        changed: true,
        tightened: false,
    };

    // If we are in the transformed problem, get transformed variables, add
    // variable bound information, and catch events.
    if api::is_transformed(scip) {
        c.var = api::get_transformed_var(scip, c.var)?;
        c.vbdvar = api::get_transformed_var(scip, c.vbdvar)?;
    }

    // Capture variables.
    crate::var::var_capture(scip, c.var);
    crate::var::var_capture(scip, c.vbdvar);

    Ok(c)
}

/// `consdataFree`: frees a variable bound constraint data.
fn consdata_free(scip: &mut Scip, c: &mut ConsData) -> ScipResult {
    // Release the row.
    if c.row != NONE {
        scip_lp::release_row(scip, &mut c.row)?;
    }

    // Release variables.
    api::release_var(scip, &mut c.var)?;
    api::release_var(scip, &mut c.vbdvar)?;

    Ok(())
}

/// `createRelaxation`: creates LP row corresponding to variable bound
/// constraint.
fn create_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let k = &scip.conss[cons];
    let (name, local, modifiable, removable) = (k.name.clone(), k.is_local(), k.modifiable, k.is_removable());
    let c = cd(scip, cons);
    let (lhs, rhs, var, vbdvar, vbdcoef) = (c.lhs, c.rhs, c.var, c.vbdvar, c.vbdcoef);
    let row = scip_lp::create_empty_row_cons(scip, cons, &name, lhs, rhs, local, modifiable, removable)?;
    cdm(scip, cons).row = row;
    api::add_var_to_row(scip, row, var, 1.0)?;
    api::add_var_to_row(scip, row, vbdvar, vbdcoef)
}

/// `addRelaxation`: adds linear relaxation of variable bound constraint to
/// the LP; returns whether infeasibility was detected.
fn add_relaxation(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let mut infeasible = false;

    // Find the variable bound constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Variable bound constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Check whether the coefficient is too large to put the row into the LP.
    if scip.set.is_gt(cd(scip, cons).vbdcoef.abs(), hd(scip, conshdlr).maxlpcoef) {
        return Ok(infeasible);
    }

    if cd(scip, cons).row == NONE {
        create_relaxation(scip, cons)?;
    }

    let row = cd(scip, cons).row;
    if scip.rows[row].lppos < 0 {
        infeasible = scip_lp::add_row(scip, row, false)?;
    }

    Ok(infeasible)
}

/// `checkCons`: returns whether the given solution is feasible for the given
/// variable bound constraint.
fn check_cons(scip: &mut Scip, cons: ConsId, sol: SolId, checklprows: bool) -> bool {
    let row = cd(scip, cons).row;
    if checklprows || row == NONE || scip.rows[row].lppos < 0 {
        let c = cd(scip, cons);
        let (var, vbdvar, vbdcoef, lhs, rhs) = (c.var, c.vbdvar, c.vbdcoef, c.lhs, c.rhs);
        let sum = api::get_sol_val(scip, sol, var) + vbdcoef * api::get_sol_val(scip, sol, vbdvar);

        // Calculate constraint violation and update it in solution.
        let absviol = crate::def::max(lhs - sum, sum - rhs);
        let lhsrelviol = crate::misc::rel_diff(lhs, sum);
        let rhsrelviol = crate::misc::rel_diff(sum, rhs);
        let relviol = crate::def::max(lhsrelviol, rhsrelviol);
        if sol != NONE {
            api::update_sol_lp_cons_violation(scip, sol, absviol, relviol);
        }

        let s = &scip.set;
        (s.is_infinity(-lhs) || s.is_feas_ge(sum, lhs)) && (s.is_infinity(rhs) || s.is_feas_le(sum, rhs))
    } else {
        true
    }
}

/// Whether the inference bound and the side are small enough for the
/// integral `1 - z * eps` reduction in `resolvePropagation`.
fn widen_integral(scip: &Scip, var: VarId, inferbd: f64, side: f64) -> bool {
    let lim = scip.set.get_huge_value() * scip.set.feastol();
    crate::var::var_is_integral(scip, var) && inferbd < lim && side.abs() < lim
}

/// `resolvePropagation`: resolves a propagation on the given variable by
/// supplying the variables needed for applying the corresponding propagation
/// rule (see `propagate_cons`):
///  (1) left hand side and bounds on y -> lower bound on x
///  (2) left hand side and upper bound on x -> bound on y
///  (3) right hand side and bounds on y -> upper bound on x
///  (4) right hand side and lower bound on x -> bound on y
#[allow(clippy::too_many_arguments)]
fn resolve_propagation(
    scip: &mut Scip,
    cons: ConsId,
    _infervar: VarId,
    proprule: Proprule,
    _boundtype: BoundType,
    bdchgidx: Option<&BdChgIdx>,
    inferbd: f64,
    usebdwidening: bool,
) -> ScipResult {
    use crate::quad::*;

    let c = cd(scip, cons);
    let (var, vbdvar, vbdcoef, lhs, rhs) = (c.var, c.vbdvar, c.vbdcoef, c.lhs, c.rhs);
    let feastol = scip.set.feastol();

    match proprule {
        Proprule::Rule1 => {
            // lhs <= x + c*y: left hand side and bounds on y -> lower bound on x.
            if usebdwidening {
                // For integer variables, we can reduce the inferbound by 1 - z * eps,
                // because this will be adjusted to the bound we need; however, we need
                // to choose z large enough to prevent numerical troubles due to too
                // small and too large vbdcoef values. If inferbound has a large value,
                // adding z*eps might be lost due to fixed precision floating point
                // arithmetics, so we explicitly check this here.
                let mut relaxedbd;
                if widen_integral(scip, var, inferbd, lhs) {
                    let tmp = prod_qd(Quad::new(2.0), feastol);
                    relaxedbd = sum_dd(inferbd, -1.0);
                    relaxedbd = sum_qq(relaxedbd, tmp);
                    relaxedbd = sum_qd(-relaxedbd, lhs);
                    relaxedbd = div_qd(relaxedbd, vbdcoef);
                } else {
                    relaxedbd = sum_dd(lhs, -inferbd);
                    relaxedbd = div_qd(relaxedbd, vbdcoef);
                }

                if vbdcoef > 0.0 {
                    // Decrease the computed relaxed upper bound by an epsilon; this
                    // ensures that we get the actual inference bound due to the
                    // integrality condition of the variable bound variable.
                    relaxedbd = sum_qd(relaxedbd, -feastol);
                    api::add_conflict_relaxed_ub(scip, vbdvar, bdchgidx, relaxedbd.to_dbl())?;
                } else {
                    // Increase the computed relaxed lower bound by an epsilon; this
                    // ensures that we get the actual inference bound due to the
                    // integrality condition of the variable bound variable.
                    relaxedbd = sum_qd(relaxedbd, feastol);
                    api::add_conflict_relaxed_lb(scip, vbdvar, bdchgidx, relaxedbd.to_dbl())?;
                }
            } else if vbdcoef > 0.0 {
                api::add_conflict_ub(scip, vbdvar, bdchgidx)?;
            } else {
                api::add_conflict_lb(scip, vbdvar, bdchgidx)?;
            }
        }
        Proprule::Rule2 => {
            // lhs <= x + c*y: left hand side and upper bound on x -> bound on y.
            if usebdwidening {
                // Compute the relaxed upper bound of the variable which would be
                // sufficient to reach one less (greater) than the inference bound.
                let mut relaxedub;
                if vbdcoef > 0.0 {
                    // For integer variables, we can reduce the inferbound by 1-z*eps (see
                    // rule 1).
                    if widen_integral(scip, var, inferbd, rhs) {
                        let tmp = prod_qd(Quad::new(2.0), feastol);
                        relaxedub = sum_dd(inferbd, -1.0);
                        relaxedub = sum_qq(relaxedub, tmp);
                        relaxedub = prod_qd(relaxedub, vbdcoef);
                        relaxedub = sum_qd(-relaxedub, lhs);
                    } else {
                        relaxedub = prod_dd(inferbd, vbdcoef);
                        relaxedub = sum_qd(-relaxedub, lhs);
                    }
                } else {
                    // For integer variables, we can reduce the inferbound by 1-z*eps (see
                    // rule 1).
                    if widen_integral(scip, var, inferbd, lhs) {
                        let tmp = prod_qd(Quad::new(2.0), feastol);
                        relaxedub = sum_dd(inferbd, 1.0);
                        relaxedub = sum_qq(relaxedub, -tmp);
                        relaxedub = prod_qd(relaxedub, vbdcoef);
                        relaxedub = sum_qd(-relaxedub, lhs);
                    } else {
                        relaxedub = prod_dd(inferbd, vbdcoef);
                        relaxedub = sum_qd(-relaxedub, lhs);
                    }
                }

                // Decrease the computed relaxed upper bound by an epsilon; this ensures
                // that we get the actual inference bound due to the integrality
                // condition of the variable bound variable.
                relaxedub = sum_qd(relaxedub, -feastol);
                api::add_conflict_relaxed_ub(scip, var, bdchgidx, relaxedub.to_dbl())?;
            } else {
                api::add_conflict_ub(scip, var, bdchgidx)?;
            }
        }
        Proprule::Rule3 => {
            // x + c*y <= rhs: right hand side and bounds on y -> upper bound on x.
            if usebdwidening {
                // For integer variables, we can reduce the inferbound by 1-z*eps (see
                // rule 1).
                let mut relaxedbd;
                if widen_integral(scip, var, inferbd, rhs) {
                    let tmp = prod_qd(Quad::new(2.0), feastol);
                    relaxedbd = sum_dd(inferbd, 1.0);
                    relaxedbd = sum_qq(relaxedbd, -tmp);
                    relaxedbd = sum_qd(relaxedbd, -rhs);
                    relaxedbd = div_qd(relaxedbd, -vbdcoef);
                } else {
                    relaxedbd = sum_dd(rhs, -inferbd);
                    relaxedbd = div_qd(relaxedbd, vbdcoef);
                }

                if vbdcoef > 0.0 {
                    // Increase the computed relaxed lower bound by an epsilon; this
                    // ensures that we get the actual inference bound due to the
                    // integrality condition of the variable bound variable.
                    relaxedbd = sum_qd(relaxedbd, feastol);
                    api::add_conflict_relaxed_lb(scip, vbdvar, bdchgidx, relaxedbd.to_dbl())?;
                } else {
                    // Decrease the computed relaxed upper bound by an epsilon; this
                    // ensures that we get the actual inference bound due to the
                    // integrality condition of the variable bound variable.
                    relaxedbd = sum_qd(relaxedbd, -feastol);
                    api::add_conflict_relaxed_ub(scip, vbdvar, bdchgidx, relaxedbd.to_dbl())?;
                }
            } else if vbdcoef > 0.0 {
                api::add_conflict_lb(scip, vbdvar, bdchgidx)?;
            } else {
                api::add_conflict_ub(scip, vbdvar, bdchgidx)?;
            }
        }
        Proprule::Rule4 => {
            // x + c*y <= rhs: right hand side and lower bound on x -> bound on y.
            if usebdwidening {
                // Compute the relaxed lower bound of the variable which would be
                // sufficient to reach one greater (less) than the inference bound.
                let mut relaxedlb;
                if vbdcoef > 0.0 {
                    // For integer variables, we can reduce the inferbound by 1-z*eps (see
                    // rule 1).
                    if widen_integral(scip, var, inferbd, rhs) {
                        let tmp = prod_qd(Quad::new(2.0), feastol);
                        relaxedlb = sum_dd(inferbd, 1.0);
                        relaxedlb = sum_qq(relaxedlb, -tmp);
                        relaxedlb = prod_qd(relaxedlb, vbdcoef);
                        relaxedlb = sum_qd(-relaxedlb, rhs);
                    } else {
                        relaxedlb = prod_dd(inferbd, vbdcoef);
                        relaxedlb = sum_qd(-relaxedlb, rhs);
                    }
                } else {
                    // For integer variables, we can reduce the inferbound by 1-z*eps (see
                    // rule 1).
                    if widen_integral(scip, var, inferbd, lhs) {
                        let tmp = prod_qd(Quad::new(2.0), feastol);
                        relaxedlb = sum_dd(inferbd, -1.0);
                        relaxedlb = sum_qq(relaxedlb, tmp);
                        relaxedlb = prod_qd(relaxedlb, vbdcoef);
                        relaxedlb = sum_qd(-relaxedlb, rhs);
                    } else {
                        relaxedlb = prod_dd(inferbd, vbdcoef);
                        relaxedlb = sum_qd(-relaxedlb, rhs);
                    }
                }

                // Increase the computed relaxed lower bound by an epsilon; this ensures
                // that we get the actual inference bound due to the integrality
                // condition of the variable bound variable.
                relaxedlb = sum_qd(relaxedlb, feastol);
                api::add_conflict_relaxed_lb(scip, var, bdchgidx, relaxedlb.to_dbl())?;
            } else {
                api::add_conflict_lb(scip, var, bdchgidx)?;
            }
        }
    }

    Ok(())
}

/// `analyzeConflict`: analyze infeasibility.
#[allow(clippy::too_many_arguments)]
fn analyze_conflict(scip: &mut Scip, cons: ConsId, infervar: VarId, mut inferbd: f64, proprule: Proprule, boundtype: BoundType, usebdwidening: bool) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // applicable.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis, and add all variables of infeasible
    // constraint to conflict candidate queue.
    api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

    let feastol = scip.set.feastol();

    // Add the bound which got violated.
    if boundtype == BoundType::Lower {
        if usebdwidening {
            // Adjust lower bound.
            inferbd = scip_var::adjusted_var_lb(scip, infervar, inferbd);

            // Compute a relaxed upper bound which would be sufficient to be still
            // infeasible.
            let relaxedub = if crate::var::var_is_integral(scip, infervar) {
                inferbd - 1.0
            } else {
                // vbdvar can never be of non-integral type; due to resolving a the
                // propagation and dividing by the vbdcoef we need to make sure the
                // the relaxed bound is big enough, therefore we multiply here with
                // the vbdcoef (it does not matter if we deceed the current local
                // upper bound, because SCIPaddConflictRelaxedUb() is correcting the
                // bound afterwards).
                let abscoef = cd(scip, cons).vbdcoef.abs();
                inferbd - 2.0 * feastol * crate::def::max(1.0, abscoef)
            };

            // Try to relax inference variable upper bound such that the
            // infeasibility is still given.
            api::add_conflict_relaxed_ub(scip, infervar, None, relaxedub)?;

            // Collect the upper bound which is reported to the conflict analysis.
            inferbd = api::get_conflict_var_ub(scip, infervar);

            // Adjust inference bound with respect to the upper bound reported to
            // the conflict analysis.
            if crate::var::var_is_integral(scip, infervar) {
                inferbd += 1.0;
            } else {
                let abscoef = cd(scip, cons).vbdcoef.abs();
                inferbd += 2.0 * feastol * crate::def::max(1.0, abscoef);
            }
        } else {
            api::add_conflict_ub(scip, infervar, None)?;
        }
    } else if usebdwidening {
        // Adjust upper bound.
        inferbd = scip_var::adjusted_var_ub(scip, infervar, inferbd);

        // Compute a relaxed lower bound which would be sufficient to be still
        // infeasible.
        let relaxedlb = if crate::var::var_is_integral(scip, infervar) {
            inferbd + 1.0
        } else {
            // vbdvar can never be of non-integral type (see above).
            let abscoef = cd(scip, cons).vbdcoef.abs();
            inferbd + 2.0 * feastol * crate::def::max(1.0, abscoef)
        };

        // Try to relax inference variable upper bound such that the infeasibility
        // is still given.
        api::add_conflict_relaxed_lb(scip, infervar, None, relaxedlb)?;

        // Collect the lower bound which is reported to the conflict analysis.
        inferbd = api::get_conflict_var_lb(scip, infervar);

        // Adjust inference bound with respect to the lower bound reported to the
        // conflict analysis.
        if crate::var::var_is_integral(scip, infervar) {
            inferbd -= 1.0;
        } else {
            let abscoef = cd(scip, cons).vbdcoef.abs();
            inferbd -= 2.0 * feastol * crate::def::max(1.0, abscoef);
        }
    } else {
        api::add_conflict_lb(scip, infervar, None)?;
    }

    // Add the reason for the violated of the bound.
    resolve_propagation(scip, cons, infervar, proprule, boundtype, None, inferbd, usebdwidening)?;

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `separateCons`: separates the given variable bound constraint.
fn separate_cons(scip: &mut Scip, cons: ConsId, usebdwidening: bool, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    use crate::quad::*;

    // Find the variable bound constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Variable bound constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    let c = cd(scip, cons);
    let (var, vbdvar, vbdcoef) = (c.var, c.vbdvar, c.vbdcoef);

    // If x is not multiaggregated and y is fixed, propagate bounds on x.
    if scip.vars[var].varstatus != VarStatus::MultAggr && scip.vars[vbdvar].locdom.lb + 0.5 > scip.vars[vbdvar].locdom.ub {
        let lhs = cd(scip, cons).lhs;
        if !scip.set.is_infinity(-lhs) {
            let mut tmp = prod_dd(vbdcoef, scip.vars[vbdvar].locdom.lb);
            tmp = sum_qd(-tmp, lhs);
            let newlb = tmp.to_dbl();

            let (cutoff, tightened) = scip_var::infer_var_lb_cons(scip, var, newlb, cons, Proprule::Rule1 as i32, true)?;

            if cutoff {
                // Analyze infeasibility.
                analyze_conflict(scip, cons, var, newlb, Proprule::Rule1, BoundType::Lower, usebdwidening)?;
                *result = ScipResultCode::Cutoff;
                return Ok(());
            } else if tightened {
                *result = ScipResultCode::ReducedDom;
            }
        }

        let rhs = cd(scip, cons).rhs;
        if !scip.set.is_infinity(rhs) {
            let mut tmp = prod_dd(vbdcoef, scip.vars[vbdvar].locdom.lb);
            tmp = sum_qd(-tmp, rhs);
            let newub = tmp.to_dbl();

            let (cutoff, tightened) = scip_var::infer_var_ub_cons(scip, var, newub, cons, Proprule::Rule3 as i32, true)?;

            if cutoff {
                // Analyze infeasibility.
                analyze_conflict(scip, cons, var, newub, Proprule::Rule3, BoundType::Upper, usebdwidening)?;
                *result = ScipResultCode::Cutoff;
                return Ok(());
            } else if tightened {
                *result = ScipResultCode::ReducedDom;
            }
        }
    }

    // If we already changed a bound or the coefficient is too large to put
    // the row into the LP, stop here.
    if *result == ScipResultCode::ReducedDom {
        return Ok(());
    }

    // Check constraint for feasibility and create row if constraint is
    // violated.
    if !check_cons(scip, cons, sol, sol != NONE) {
        // Create LP relaxation if not yet existing.
        if cd(scip, cons).row == NONE {
            create_relaxation(scip, cons)?;
        }

        // Check non-LP rows for feasibility and add them as cut, if violated.
        let row = cd(scip, cons).row;
        if scip.rows[row].lppos < 0 {
            let feasibility = scip_lp::get_row_sol_feasibility(scip, row, sol);
            if scip.set.is_feas_negative(feasibility) {
                let infeasible = scip_lp::add_row(scip, row, false)?;
                *result = if infeasible { ScipResultCode::Cutoff } else { ScipResultCode::Separated };
            }
        }
    }

    Ok(())
}

/// `chgLhs`: sets left hand side of varbound constraint.
fn chg_lhs(scip: &mut Scip, cons: ConsId, mut lhs: f64) -> ScipResult {
    // Adjust value to not be smaller than -inf.
    if scip.set.is_infinity(-lhs) {
        lhs = -scip.set.infinity();
    }

    let c = cd(scip, cons);
    let (oldlhs, rhs, var, vbdvar, vbdcoef) = (c.lhs, c.rhs, c.var, c.vbdvar, c.vbdcoef);

    // Check whether the side is not changed.
    if scip.set.is_eq(oldlhs, lhs) {
        return Ok(());
    }

    // Ensure that rhs >= lhs is satisfied without numerical tolerance.
    if scip.set.is_eq(lhs, rhs) {
        cdm(scip, cons).rhs = lhs;
    }

    // Update the rounding locks of variables.

    // The left hand side switched from -infinity to a non-infinite value ->
    // install rounding locks.
    if scip.set.is_infinity(-oldlhs) && !scip.set.is_infinity(-lhs) {
        api::lock_var_cons(scip, var, cons, true, false)?;

        if vbdcoef > 0.0 {
            api::lock_var_cons(scip, vbdvar, cons, true, false)?;
        } else {
            api::lock_var_cons(scip, vbdvar, cons, false, true)?;
        }
    }
    // The left hand side switched from a non-infinite value to -infinity ->
    // remove rounding locks.
    else if !scip.set.is_infinity(-oldlhs) && scip.set.is_infinity(-lhs) {
        api::unlock_var_cons(scip, var, cons, true, false)?;
        if vbdcoef > 0.0 {
            api::unlock_var_cons(scip, vbdvar, cons, true, false)?;
        } else {
            api::unlock_var_cons(scip, vbdvar, cons, false, true)?;
        }
    }

    // If left hand side got tighter, we want to do additional presolving on
    // this constraint.
    if scip.set.is_lt(oldlhs, lhs) {
        let c = cdm(scip, cons);
        c.varboundsadded = false;
        c.tightened = false;

        cons_mark_propagate(scip, cons);
    }

    let c = cdm(scip, cons);
    c.presolved = false;
    c.lhs = lhs;
    c.changed = true;

    Ok(())
}

/// `chgRhs`: sets right hand side of varbound constraint.
fn chg_rhs(scip: &mut Scip, cons: ConsId, mut rhs: f64) -> ScipResult {
    // Adjust value to not be larger than inf.
    if scip.set.is_infinity(rhs) {
        rhs = scip.set.infinity();
    }

    let c = cd(scip, cons);
    let (lhs, oldrhs, var, vbdvar, vbdcoef) = (c.lhs, c.rhs, c.var, c.vbdvar, c.vbdcoef);

    // Check whether the side is not changed.
    if scip.set.is_eq(oldrhs, rhs) {
        return Ok(());
    }

    // Ensure that rhs >= lhs is satisfied without numerical tolerance.
    if scip.set.is_eq(rhs, lhs) {
        cdm(scip, cons).lhs = rhs;
    }

    // Update the locks of variables.

    // The right hand side switched from infinity to a non-infinite value ->
    // install locks.
    if scip.set.is_infinity(oldrhs) && !scip.set.is_infinity(rhs) {
        api::lock_var_cons(scip, var, cons, false, true)?;

        if vbdcoef > 0.0 {
            api::lock_var_cons(scip, vbdvar, cons, false, true)?;
        } else {
            api::lock_var_cons(scip, vbdvar, cons, true, false)?;
        }
    }
    // The right hand side switched from a non-infinite value to infinity ->
    // remove locks.
    else if !scip.set.is_infinity(oldrhs) && scip.set.is_infinity(rhs) {
        api::unlock_var_cons(scip, var, cons, false, true)?;
        if vbdcoef > 0.0 {
            api::unlock_var_cons(scip, vbdvar, cons, false, true)?;
        } else {
            api::unlock_var_cons(scip, vbdvar, cons, true, false)?;
        }
    }

    // If right hand side got tighter, we want to do additional presolving on
    // this constraint.
    if scip.set.is_gt(oldrhs, rhs) {
        let c = cdm(scip, cons);
        c.varboundsadded = false;
        c.tightened = false;

        cons_mark_propagate(scip, cons);
    }

    let c = cdm(scip, cons);
    c.presolved = false;
    c.rhs = rhs;
    c.changed = true;

    Ok(())
}

/// `propagateCons`: propagation method for variable bound constraint.
#[allow(clippy::too_many_arguments)]
fn propagate_cons(
    scip: &mut Scip,
    cons: ConsId,
    usebdwidening: bool,
    cutoff: &mut bool,
    nchgbds: &mut i32,
    nchgsides: Option<&mut i32>,
    ndelconss: Option<&mut i32>,
) -> ScipResult {
    use crate::quad::*;

    *cutoff = false;

    // Increase age of constraint; age is reset to zero, if a conflict or a
    // propagation was found.
    if !crate::tree::tree_in_repropagation(scip) {
        api::inc_cons_age(scip, cons)?;
    }

    let (var, vbdvar) = (cd(scip, cons).var, cd(scip, cons).vbdvar);
    let lb = |scip: &Scip, v: VarId| scip.vars[v].locdom.lb;
    let ub = |scip: &Scip, v: VarId| scip.vars[v].locdom.ub;

    // Get current bounds of variables.
    let mut xlb = lb(scip, var);
    let mut xub = ub(scip, var);
    let mut ylb = lb(scip, vbdvar);
    let mut yub = ub(scip, vbdvar);

    // It can happen that constraint is of form lhs <= x <= rhs.
    let c = cd(scip, cons);
    if scip.set.is_zero(c.vbdcoef) && scip.set.is_eq(c.lhs, c.rhs) {
        let lhs = c.lhs;
        let (infeasible, _fixed) = scip_var::fix_var(scip, var, lhs)?;
        if infeasible {
            *cutoff = true;
            return Ok(());
        }
    }

    // Tighten bounds of variables as long as possible.
    'rounds: loop {
        let mut tightenedround = false;

        // Propagate left hand side inequality: lhs <= x + c*y.
        let lhs = cd(scip, cons).lhs;
        if !scip.set.is_infinity(-lhs) {
            let vbdcoef = cd(scip, cons).vbdcoef;

            // Propagate bounds on x:
            //  (1) left hand side and bounds on y -> lower bound on x
            if scip.vars[var].varstatus != VarStatus::MultAggr {
                // Cannot change bounds of multaggr vars.
                let newlb = if vbdcoef > 0.0 {
                    if !scip.set.is_infinity(yub) {
                        let tmp = sum_qd(-prod_dd(vbdcoef, yub), lhs);
                        scip_var::adjusted_var_lb(scip, var, tmp.to_dbl())
                    } else {
                        -scip.set.infinity()
                    }
                } else if !scip.set.is_infinity(-ylb) {
                    let tmp = sum_qd(-prod_dd(vbdcoef, ylb), lhs);
                    scip_var::adjusted_var_lb(scip, var, tmp.to_dbl())
                } else {
                    -scip.set.infinity()
                };

                let tightened;
                (*cutoff, tightened) = scip_var::infer_var_lb_cons(scip, var, newlb, cons, Proprule::Rule1 as i32, yub < ylb + 0.5)?;

                if *cutoff {
                    api::reset_cons_age(scip, cons);

                    // Analyze infeasibility.
                    analyze_conflict(scip, cons, var, newlb, Proprule::Rule1, BoundType::Lower, usebdwidening)?;
                    break 'rounds;
                }

                if tightened {
                    tightenedround = true;
                    *nchgbds += 1;
                    api::reset_cons_age(scip, cons);
                }
                xlb = lb(scip, var);
            }

            // Propagate bounds on y:
            //  (2) left hand side and upper bound on x -> bound on y
            if scip.vars[vbdvar].varstatus != VarStatus::MultAggr && !scip.set.is_infinity(xub) {
                // Cannot change bounds of multaggr vars.
                if vbdcoef > 0.0 {
                    let tmp = div_qd(sum_dd(lhs, -xub), vbdcoef);
                    let newlb = scip_var::adjusted_var_lb(scip, vbdvar, tmp.to_dbl());
                    if newlb > ylb + 0.5 {
                        let tightened;
                        (*cutoff, tightened) = scip_var::infer_var_lb_cons(scip, vbdvar, newlb, cons, Proprule::Rule2 as i32, false)?;

                        if *cutoff {
                            // Analyze infeasibility.
                            analyze_conflict(scip, cons, vbdvar, newlb, Proprule::Rule2, BoundType::Lower, usebdwidening)?;
                            break 'rounds;
                        }

                        if tightened {
                            tightenedround = true;
                            *nchgbds += 1;
                        }
                        ylb = lb(scip, vbdvar);
                    }
                } else {
                    let tmp = div_qd(sum_dd(lhs, -xub), vbdcoef);
                    let newub = scip_var::adjusted_var_ub(scip, vbdvar, tmp.to_dbl());
                    if newub < yub - 0.5 {
                        let tightened;
                        (*cutoff, tightened) = scip_var::infer_var_ub_cons(scip, vbdvar, newub, cons, Proprule::Rule2 as i32, false)?;

                        if *cutoff {
                            api::reset_cons_age(scip, cons);

                            // Analyze infeasibility.
                            analyze_conflict(scip, cons, vbdvar, newub, Proprule::Rule2, BoundType::Upper, usebdwidening)?;
                            break 'rounds;
                        }

                        if tightened {
                            tightenedround = true;
                            *nchgbds += 1;
                            api::reset_cons_age(scip, cons);
                        }
                        yub = ub(scip, vbdvar);
                    }
                }
            }
        }

        // Propagate right hand side inequality: x + c*y <= rhs.
        let rhs = cd(scip, cons).rhs;
        if !scip.set.is_infinity(rhs) {
            let vbdcoef = cd(scip, cons).vbdcoef;

            // Propagate bounds on x:
            //  (3) right hand side and bounds on y -> upper bound on x
            if scip.vars[var].varstatus != VarStatus::MultAggr {
                // Cannot change bounds of multaggr vars.
                let newub = if vbdcoef > 0.0 {
                    if !scip.set.is_infinity(-ylb) {
                        let tmp = sum_qd(-prod_dd(vbdcoef, ylb), rhs);
                        scip_var::adjusted_var_ub(scip, var, tmp.to_dbl())
                    } else {
                        scip.set.infinity()
                    }
                } else if !scip.set.is_infinity(yub) {
                    let tmp = sum_qd(-prod_dd(vbdcoef, yub), rhs);
                    scip_var::adjusted_var_ub(scip, var, tmp.to_dbl())
                } else {
                    scip.set.infinity()
                };

                let tightened;
                (*cutoff, tightened) = scip_var::infer_var_ub_cons(scip, var, newub, cons, Proprule::Rule3 as i32, yub < ylb + 0.5)?;

                if *cutoff {
                    api::reset_cons_age(scip, cons);

                    // Analyze infeasibility.
                    analyze_conflict(scip, cons, var, newub, Proprule::Rule3, BoundType::Upper, usebdwidening)?;
                    break 'rounds;
                }

                if tightened {
                    tightenedround = true;
                    *nchgbds += 1;
                    api::reset_cons_age(scip, cons);
                }
                xub = ub(scip, var);
            }

            // Propagate bounds on y:
            //  (4) right hand side and lower bound on x -> bound on y
            if scip.vars[vbdvar].varstatus != VarStatus::MultAggr && !scip.set.is_infinity(-xlb) {
                // Cannot change bounds of multaggr vars.
                if vbdcoef > 0.0 {
                    let tmp = div_qd(sum_dd(rhs, -xlb), vbdcoef);
                    let newub = scip_var::adjusted_var_ub(scip, vbdvar, tmp.to_dbl());
                    if newub < yub - 0.5 {
                        let tightened;
                        (*cutoff, tightened) = scip_var::infer_var_ub_cons(scip, vbdvar, newub, cons, Proprule::Rule4 as i32, false)?;

                        if *cutoff {
                            api::reset_cons_age(scip, cons);

                            // Analyze infeasibility.
                            analyze_conflict(scip, cons, vbdvar, newub, Proprule::Rule4, BoundType::Upper, usebdwidening)?;
                            break 'rounds;
                        }

                        if tightened {
                            tightenedround = true;
                            *nchgbds += 1;
                            api::reset_cons_age(scip, cons);
                        }
                        yub = ub(scip, vbdvar);
                    }
                } else {
                    let tmp = div_qd(sum_dd(rhs, -xlb), vbdcoef);
                    let newlb = scip_var::adjusted_var_lb(scip, vbdvar, tmp.to_dbl());
                    if newlb > ylb + 0.5 {
                        let tightened;
                        (*cutoff, tightened) = scip_var::infer_var_lb_cons(scip, vbdvar, newlb, cons, Proprule::Rule4 as i32, false)?;

                        if *cutoff {
                            api::reset_cons_age(scip, cons);

                            // Analyze infeasibility.
                            analyze_conflict(scip, cons, vbdvar, newlb, Proprule::Rule4, BoundType::Lower, usebdwidening)?;
                            break 'rounds;
                        }

                        if tightened {
                            tightenedround = true;
                            *nchgbds += 1;
                            api::reset_cons_age(scip, cons);
                        }
                        ylb = lb(scip, vbdvar);
                    }
                }
            }
        }

        if !tightenedround {
            break;
        }
    }

    // Check for redundant sides.
    if !*cutoff {
        let mut islhsredundant = true;
        let mut isrhsredundant = true;
        let mut nvars = 2;
        let vbdcoef = cd(scip, cons).vbdcoef;
        let mut vars = crate::cvec::CVec(vec![var, vbdvar]);
        let mut scalars = crate::cvec::CVec(vec![1.0, vbdcoef]);
        let mut constant = 0.0;
        let mut requiredsize = 0;

        crate::var::var_get_active_representatives(scip, &mut vars, &mut scalars, &mut nvars, 2, &mut constant, &mut requiredsize)?;

        if requiredsize > nvars {
            vars.0.resize(requiredsize as usize, NONE);
            scalars.0.resize(requiredsize as usize, 0.0);

            crate::var::var_get_active_representatives(scip, &mut vars, &mut scalars, &mut nvars, requiredsize, &mut constant, &mut requiredsize)?;
        }

        let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
        if !scip.set.is_infinity(-lhs) {
            let mut activity = Quad::new(constant);

            for i in 0..nvars {
                let bound;
                if scalars[i] > 0.0 {
                    bound = lb(scip, vars[i]);

                    if scip.set.is_infinity(-bound) {
                        islhsredundant = false;
                        break;
                    }
                } else {
                    bound = ub(scip, vars[i]);

                    if scip.set.is_infinity(bound) {
                        islhsredundant = false;
                        break;
                    }
                }

                activity = sum_qd(activity, scalars[i] * bound);
            }

            if islhsredundant && scip.set.is_lt(activity.to_dbl(), lhs) {
                islhsredundant = false;
            }
        }

        if !scip.set.is_infinity(rhs) {
            let mut activity = Quad::new(constant);

            for i in 0..nvars {
                let bound;
                if scalars[i] > 0.0 {
                    bound = ub(scip, vars[i]);

                    if scip.set.is_infinity(bound) {
                        isrhsredundant = false;
                        break;
                    }
                } else {
                    bound = lb(scip, vars[i]);

                    if scip.set.is_infinity(-bound) {
                        isrhsredundant = false;
                        break;
                    }
                }

                activity = sum_qd(activity, scalars[i] * bound);
            }

            if isrhsredundant && scip.set.is_gt(activity.to_dbl(), rhs) {
                isrhsredundant = false;
            }
        }

        // Check varbound constraint for redundancy.
        if islhsredundant && isrhsredundant {
            // Remove the constraint locally unless it has become empty, in which
            // case it is removed globally.
            if nvars > 0 {
                scip_cons::del_cons_local(scip, cons)?;
            } else {
                scip_cons::del_cons(scip, cons)?;
            }

            if let Some(ndelconss) = ndelconss {
                *ndelconss += 1;
            }
        } else if !api::in_probing(scip) && scip.set.stage == Stage::Presolving {
            // Check left hand side for redundancy.
            if islhsredundant {
                if !scip.set.is_infinity(-lhs) {
                    let inf = scip.set.infinity();
                    chg_lhs(scip, cons, -inf)?;

                    if let Some(nchgsides) = nchgsides {
                        *nchgsides += 1;
                    }
                }
            }
            // Check right hand side for redundancy.
            else if isrhsredundant && !scip.set.is_infinity(rhs) {
                let inf = scip.set.infinity();
                chg_rhs(scip, cons, inf)?;

                if let Some(nchgsides) = nchgsides {
                    *nchgsides += 1;
                }
            }
        }
    }

    cons_unmark_propagate(scip, cons);

    Ok(())
}

/// The `y` values, or the `x` values, the infinite-value branch of
/// `checkRedundancySide` compares; `swap` picks the swapped coefficients.
#[inline]
fn infinite_pair(scip: &Scip, coef0: f64, coef1: f64, eqvals: (f64, f64), swap: bool) -> (f64, f64) {
    if scip.set.is_eq(coef0, coef1) {
        // Only consider sides if coefficients are equal.
        eqvals
    } else if swap {
        (coef1, coef0)
    } else {
        (coef0, coef1)
    }
}

/// `checkRedundancySide`: check whether one constraint side is redundant to
/// another constraint side by calculating extreme values for variables.
/// Returns `(sideequal, cons0sidered, cons1sidered)`: whether both
/// constraints have the same redundancy on the given side, and whether the
/// side of constraint 0 (1) is redundant.
#[allow(clippy::too_many_arguments)]
fn check_redundancy_side(scip: &Scip, var: VarId, vbdvar: VarId, coef0: f64, coef1: f64, side0: f64, side1: f64, islhs: bool) -> (bool, bool, bool) {
    use crate::def::{max, min};

    let s = &scip.set;
    let inf = s.infinity();

    // sidered[0] is cons0sidered, sidered[1] is cons1sidered; redundant0 and
    // redundant1 point into it.
    let mut sidered = [s.is_infinity(if islhs { -side0 } else { side0 }), s.is_infinity(if islhs { -side1 } else { side1 })];
    let mut sideequal = false;
    let (r0, r1) = if islhs { (1, 0) } else { (0, 1) };

    let lbvar = scip.vars[var].glbdom.lb;
    let ubvar = scip.vars[var].glbdom.ub;
    let lbvbdvar = scip.vars[vbdvar].glbdom.lb;
    let ubvbdvar = scip.vars[vbdvar].glbdom.ub;

    // If both constraints have this side.
    if !sidered[r0] && !sidered[r1] {
        // Calculate extreme values, which are reached by setting the other
        // variable to their lower/upper bound.
        let (boundxlb1, boundxlb2) = if s.is_infinity(-lbvbdvar) {
            if coef0 > 0.0 { (inf, inf) } else { (-inf, -inf) }
        } else {
            (side0 - lbvbdvar * coef0, side1 - lbvbdvar * coef1)
        };

        let (boundylb1, boundylb2) = if s.is_infinity(-lbvar) {
            if coef0 > 0.0 { (inf, inf) } else { (-inf, -inf) }
        } else {
            ((side0 - lbvar) / coef0, (side1 - lbvar) / coef1)
        };

        let (boundxub1, boundxub2) = if s.is_infinity(ubvbdvar) {
            if coef0 > 0.0 { (-inf, -inf) } else { (inf, inf) }
        } else {
            (side0 - ubvbdvar * coef0, side1 - ubvbdvar * coef1)
        };

        let (boundyub1, boundyub2) = if s.is_infinity(ubvar) {
            if coef0 > 0.0 { (-inf, -inf) } else { (inf, inf) }
        } else {
            ((side0 - ubvar) / coef0, (side1 - ubvar) / coef1)
        };

        let (boundvaluex1, boundvaluex2) = if islhs {
            (max(boundxlb1, boundxlb2), max(boundxub1, boundxub2))
        } else {
            (min(boundxlb1, boundxlb2), min(boundxub1, boundxub2))
        };

        // Calculate important values for variables.
        let mut valuex1;
        let mut valuex2;
        if coef0 > 0.0 {
            valuex1 = min(boundvaluex1, ubvar);
            valuex1 = max(valuex1, lbvar);
            valuex2 = max(boundvaluex2, lbvar);
            valuex2 = min(valuex2, ubvar);

            // If variable is of integral type make values integral too.
            if crate::var::var_is_integral(scip, var) {
                valuex1 = s.feas_floor(valuex1);
                valuex2 = s.feas_ceil(valuex2);
            }
        } else {
            valuex1 = max(boundvaluex1, lbvar);
            valuex1 = min(valuex1, ubvar);
            valuex2 = min(boundvaluex2, ubvar);
            valuex2 = max(valuex2, lbvar);

            // If variable is of integral type make values integral too.
            if crate::var::var_is_integral(scip, var) {
                valuex1 = s.feas_ceil(valuex1);
                valuex2 = s.feas_floor(valuex2);
            }
        }

        // Calculate resulting values of variable y by setting x to valuex1;
        // with an infinite x, only consider original coefficients if x
        // approaches plus infinity, swapped ones if minus infinity.
        let (mut valuey1, mut valuey2) = if s.is_infinity(valuex1.abs()) {
            infinite_pair(scip, coef0, coef1, (side0 / coef0, side1 / coef1), valuex1 <= 0.0)
        } else {
            ((side0 - valuex1) / coef0, (side1 - valuex1) / coef1)
        };

        // Determine redundancy of one constraints side.
        if s.is_eq(valuey1, valuey2) {
            sideequal = true;
        } else if coef0 > 0.0 {
            if valuey1 < valuey2 {
                sidered[r1] = true;
            } else {
                sidered[r0] = true;
            }
        } else if valuey1 < valuey2 {
            sidered[r0] = true;
        } else {
            sidered[r1] = true;
        }

        // Calculate resulting values of variable y by setting x to valuex2.
        (valuey1, valuey2) = if s.is_infinity(valuex2.abs()) {
            infinite_pair(scip, coef0, coef1, (side0 / coef0, side1 / coef1), valuex2 <= 0.0)
        } else {
            ((side0 - valuex2) / coef0, (side1 - valuex2) / coef1)
        };

        // Determine redundancy of one constraints side by checking for the
        // first valuex2.
        if coef0 > 0.0 {
            // If both constraints are equal on one value, only consider the other
            // value.
            if sideequal {
                if s.is_lt(valuey1, valuey2) {
                    sideequal = false;
                    sidered[r1] = true;
                } else if s.is_gt(valuey1, valuey2) {
                    sideequal = false;
                    sidered[r0] = true;
                }
            }
            // If both constraints are weaker than the other on one value, we have
            // no redundancy.
            else if (sidered[r1] && s.is_gt(valuey1, valuey2)) || (sidered[r0] && s.is_lt(valuey1, valuey2)) {
                sidered[r0] = false;
                sidered[r1] = false;
                return (sideequal, sidered[0], sidered[1]);
            }
        } else {
            // If both constraints are equal on one value, only consider the other
            // value.
            if sideequal {
                if s.is_lt(valuey1, valuey2) {
                    sideequal = false;
                    sidered[r0] = true;
                } else if s.is_gt(valuey1, valuey2) {
                    sideequal = false;
                    sidered[r1] = true;
                }
            }
            // If both constraints are weaker than the other one on one value, we
            // have no redundancy.
            else if (sidered[r0] && s.is_gt(valuey1, valuey2)) || (sidered[r1] && s.is_lt(valuey1, valuey2)) {
                sidered[r0] = false;
                sidered[r1] = false;
                return (sideequal, sidered[0], sidered[1]);
            }
        }

        // Calculate feasibility domain values for variable y concerning these
        // both constraints.
        if coef0 > 0.0 {
            let (boundvaluey1, boundvaluey2) = if islhs {
                (max(boundylb1, boundylb2), max(boundyub1, boundyub2))
            } else {
                (min(boundylb1, boundylb2), min(boundyub1, boundyub2))
            };

            valuey1 = min(boundvaluey1, ubvbdvar);
            valuey1 = max(valuey1, lbvbdvar);
            valuey2 = max(boundvaluey2, lbvbdvar);
            valuey2 = min(valuey2, ubvbdvar);

            valuey1 = s.feas_floor(valuey1);
            valuey2 = s.feas_ceil(valuey2);
        } else {
            let (boundvaluey1, boundvaluey2) = if islhs {
                (min(boundylb1, boundylb2), min(boundyub1, boundyub2))
            } else {
                (max(boundylb1, boundylb2), max(boundyub1, boundyub2))
            };

            valuey1 = max(boundvaluey1, lbvbdvar);
            valuey1 = min(valuey1, ubvbdvar);
            valuey2 = min(boundvaluey2, ubvbdvar);
            valuey2 = max(valuey2, lbvbdvar);

            valuey1 = s.feas_ceil(valuey1);
            valuey2 = s.feas_floor(valuey2);
        }

        // Calculate resulting values of variable x by setting y to valuey1; with
        // an infinite y, only consider swapped coefficients if y approaches plus
        // infinity, original ones if minus infinity.
        let (mut valuex1, mut valuex2) = if s.is_infinity(valuey1.abs()) {
            infinite_pair(scip, coef0, coef1, (side0, side1), valuey1 > 0.0)
        } else {
            (side0 - valuey1 * coef0, side1 - valuey1 * coef1)
        };

        // Determine redundancy of one constraints side by checking for the first
        // valuey1.
        if sideequal {
            if s.is_lt(valuex1, valuex2) {
                sideequal = false;
                sidered[r1] = true;
            } else if s.is_gt(valuex1, valuex2) {
                sideequal = false;
                sidered[r0] = true;
            }
        } else if (sidered[r1] && s.is_gt(valuex1, valuex2)) || (sidered[r0] && s.is_lt(valuex1, valuex2)) {
            sidered[r0] = false;
            sidered[r1] = false;
            return (sideequal, sidered[0], sidered[1]);
        }

        // Calculate resulting values of variable x by setting y to valuey2.
        (valuex1, valuex2) = if s.is_infinity(valuey2.abs()) {
            infinite_pair(scip, coef0, coef1, (side0, side1), valuey2 > 0.0)
        } else {
            (side0 - valuey2 * coef0, side1 - valuey2 * coef1)
        };

        // Determine redundancy of one constraints side by checking for the
        // second valuey2.
        if sideequal {
            if s.is_lt(valuex1, valuex2) {
                sideequal = false;
                sidered[r1] = true;
            } else if s.is_gt(valuex1, valuex2) {
                sideequal = false;
                sidered[r0] = true;
            }
        } else if (sidered[r1] && s.is_gt(valuex1, valuex2)) || (sidered[r0] && s.is_lt(valuex1, valuex2)) {
            sidered[r0] = false;
            sidered[r1] = false;
            return (sideequal, sidered[0], sidered[1]);
        }
    }

    (sideequal, sidered[0], sidered[1])
}

/// One bound derivation of the cross-propagation in
/// `preprocessConstraintPairs`: with a feasibly positive `scalar` tightens the
/// lower bound of `var` to `bnd` if `lower` (else the upper bound), with a
/// feasibly negative one the other bound. Returns whether a cutoff was found
/// (the caller then leaves the inner loop).
fn tighten_pair_bound(scip: &mut Scip, var: VarId, bnd: f64, scalar: f64, lower: bool, cutoff: &mut bool, nchgbds: &mut i32) -> ScipResult<bool> {
    let uselower = if scip.set.is_feas_positive(scalar) {
        lower
    } else if scip.set.is_feas_negative(scalar) {
        !lower
    } else {
        return Ok(false);
    };

    let tightened;
    if uselower {
        let newbnd = scip_var::adjusted_var_lb(scip, var, bnd);
        (*cutoff, tightened) = scip_var::tighten_var_lb(scip, var, newbnd, false)?;
    } else {
        let newbnd = scip_var::adjusted_var_ub(scip, var, bnd);
        (*cutoff, tightened) = scip_var::tighten_var_ub(scip, var, newbnd, false)?;
    }
    if *cutoff {
        return Ok(true);
    }
    if tightened {
        *nchgbds += 1;
    }

    Ok(false)
}

/// `preprocessConstraintPairs`: compares each constraint with all other
/// constraints for possible redundancy and removes or changes constraint.
///
/// All constraints are ordered to have constraints with same variables next
/// to each other. For two constraints `lhs1 <= x + b1*y <= rhs1` and
/// `lhs2 <= x + b2*y <= rhs2`: if `b1 == b2` the sides are intersected and
/// one constraint deleted; else if both have either a valid lhs or a valid
/// rhs, infinity on the same side and coefficients of the same sign, a binary
/// y merges both into one (new side and coefficient), otherwise possible
/// values decide which constraint is tighter. Pairs `lhs1 <= x + b1*y <=
/// rhs1` and `lhs2 <= y + b2*x <= rhs2` tighten the bounds of x and y
/// instead.
#[allow(clippy::too_many_arguments)]
fn preprocess_constraint_pairs(
    scip: &mut Scip,
    conss: &[ConsId],
    cutoff: &mut bool,
    nchgbds: &mut i32,
    ndelconss: &mut i32,
    nchgcoefs: &mut i32,
    nchgsides: &mut i32,
) -> ScipResult {
    // Create our temporary working array.
    let mut sortedconss = conss.to_vec();

    // Sort all constraints, so that all constraints with same variables stand
    // next to each other.
    crate::sort::sort(&mut sortedconss, (), |a, b| cons_varbound_comp(scip, *a, *b));

    let nconss = sortedconss.len() as i32;

    // Check all constraints for redundancy.
    let mut c = nconss - 1;
    while c > 0 && !*cutoff {
        let cons0 = sortedconss[c as usize];
        c -= 1;

        if !scip.conss[cons0].is_active() || scip.conss[cons0].modifiable {
            continue;
        }

        // Do not check for already redundant constraints.
        if !cd(scip, cons0).changed {
            continue;
        }

        cdm(scip, cons0).changed = false;

        let mut s = c;
        while s >= 0 {
            let cons1 = sortedconss[s as usize];
            s -= 1;

            if !scip.conss[cons1].is_active() || scip.conss[cons1].modifiable {
                continue;
            }

            // Do not check for already redundant constraints.
            let d0 = cd(scip, cons0);
            let mut lhs = d0.lhs;
            let mut rhs = d0.rhs;
            let mut coef = d0.vbdcoef;
            let (var0, vbdvar0) = (d0.var, d0.vbdvar);
            let d1 = cd(scip, cons1);
            let (var1, vbdvar1, coef1, lhs1, rhs1) = (d1.var, d1.vbdvar, d1.vbdcoef, d1.lhs, d1.rhs);

            // Check for propagation in the case: lhs1 <= x + b1*y <= rhs1 and lhs2
            // <= y + b2*x <= rhs2.
            if var0 == vbdvar1 && vbdvar0 == var1 && !scip.set.is_feas_zero(1.0 - coef * coef1) {
                let scalar = 1.0 - coef * coef1;
                let set = &scip.set;
                let mut bnd = crate::def::UNKNOWN;

                // Lower bounds for consdata0->var.
                if !set.is_infinity(-lhs) {
                    if set.is_positive(coef) {
                        if !set.is_infinity(rhs1) {
                            bnd = (lhs - coef * rhs1) / scalar;
                        }
                    } else if !set.is_infinity(lhs1) {
                        bnd = (lhs - coef * lhs1) / scalar;
                    }

                    if bnd != crate::def::UNKNOWN && tighten_pair_bound(scip, var0, bnd, scalar, true, cutoff, nchgbds)? {
                        break;
                    }
                }

                // Upper bound for consdata0>var.
                let set = &scip.set;
                if !set.is_infinity(rhs) {
                    bnd = crate::def::UNKNOWN;
                    if set.is_positive(coef) {
                        if !set.is_infinity(lhs1) {
                            bnd = (rhs - coef * lhs1) / scalar;
                        }
                    } else if !set.is_infinity(rhs1) {
                        bnd = (rhs - coef * rhs1) / scalar;
                    }

                    if bnd != crate::def::UNKNOWN && tighten_pair_bound(scip, var0, bnd, scalar, false, cutoff, nchgbds)? {
                        break;
                    }
                }

                // Lower bounds for consdata1->var.
                let set = &scip.set;
                if !set.is_infinity(-lhs1) {
                    bnd = crate::def::UNKNOWN;
                    if set.is_positive(coef1) {
                        if !set.is_infinity(rhs) {
                            bnd = (lhs1 - coef1 * rhs) / scalar;
                        }
                    } else if !set.is_infinity(lhs) {
                        bnd = (lhs1 - coef1 * lhs) / scalar;
                    }

                    if bnd != crate::def::UNKNOWN && tighten_pair_bound(scip, var1, bnd, scalar, true, cutoff, nchgbds)? {
                        break;
                    }
                }

                // Upper bound for consdata1->var.
                let set = &scip.set;
                if !set.is_infinity(rhs1) {
                    bnd = crate::def::UNKNOWN;
                    if set.is_positive(coef1) {
                        if !set.is_infinity(lhs) {
                            bnd = (rhs1 - coef1 * lhs) / scalar;
                        }
                    } else if !set.is_infinity(rhs) {
                        bnd = (rhs1 - coef1 * rhs) / scalar;
                    }

                    if bnd != crate::def::UNKNOWN && tighten_pair_bound(scip, var1, bnd, scalar, false, cutoff, nchgbds)? {
                        break;
                    }
                }
            }

            // Check for equal variables.
            if var0 != var1 || vbdvar0 != vbdvar1 {
                break;
            }

            // Mark constraint1 for deletion if possible.
            let mut deletecons1 = true;

            let set = &scip.set;
            // The coefficients of both constraints are equal.
            if set.is_eq(coef, coef1) {
                lhs = crate::def::max(lhs1, lhs);
                rhs = crate::def::min(rhs1, rhs);
            }
            // Now only one side and in both constraints the same side should be
            // infinity and the vbdvar should be binary then we neither do not need
            // to have the same side nor the same coefficient.
            else if crate::var::var_is_binary(scip, vbdvar0)
                && (set.is_infinity(-lhs) || set.is_infinity(rhs))
                && (set.is_infinity(-lhs1) || set.is_infinity(rhs1))
                && (set.is_infinity(-lhs) == set.is_infinity(-lhs1))
            {
                let d0 = cd(scip, cons0);
                let (d0lhs, d0rhs) = (d0.lhs, d0.rhs);
                // lhs <= x + b*y <= +inf
                if !set.is_infinity(-lhs) {
                    lhs = crate::def::max(lhs1, lhs);
                    coef = lhs - crate::def::max(lhs1 - coef1, d0lhs - coef);
                }
                // -inf <= x + b*y <= rhs
                else {
                    rhs = crate::def::min(rhs1, rhs);
                    coef = rhs - crate::def::min(rhs1 - coef1, d0rhs - coef);
                }

                cons_mark_propagate(scip, cons0);
            } else if set.is_positive(coef) == set.is_positive(coef1) && ((!set.is_infinity(-lhs) && !set.is_infinity(-lhs1)) || (!set.is_infinity(rhs) && !set.is_infinity(rhs1)))
            {
                // Check if a left hand side of one constraints is redundant.
                let (lhsequal, cons0lhsred, cons1lhsred) = check_redundancy_side(scip, var0, vbdvar0, coef, coef1, lhs, lhs1, true);

                // Check if a right hand side of one constraints is redundant.
                let (rhsequal, cons0rhsred, cons1rhsred) = check_redundancy_side(scip, var0, vbdvar0, coef, coef1, rhs, rhs1, false);

                let inf = scip.set.infinity();

                // If cons0 is redundant, update cons1 and delete cons0.
                if (lhsequal || cons0lhsred) && (rhsequal || cons0rhsred) {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons1, cons0)?;
                    scip_cons::del_cons(scip, cons0)?;
                    *ndelconss += 1;

                    // Get next cons0.
                    break;
                }
                // If cons1 is redundant, update cons0 and delete cons1.
                else if cons1lhsred && cons1rhsred {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons0, cons1)?;
                    scip_cons::del_cons(scip, cons1)?;
                    *ndelconss += 1;

                    // Get next cons1.
                    continue;
                }
                // If left hand side of cons0 is redundant set it to -infinity.
                else if (lhsequal || cons0lhsred) && !scip.set.is_infinity(-lhs) {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons1, cons0)?;

                    lhs = -inf;

                    // If right hand side of cons1 is redundant too, set it to
                    // infinity.
                    if cons1rhsred && !scip.set.is_infinity(cd(scip, cons1).rhs) {
                        // Update flags of constraint which caused the redundancy s.t.
                        // nonredundant information doesn't get lost.
                        scip_cons::update_cons_flags(scip, cons0, cons1)?;

                        chg_rhs(scip, cons1, inf)?;
                        *nchgsides += 1;
                    }

                    // Later on we do not want to delete cons1.
                    deletecons1 = false;
                }
                // If right hand side of cons0 is redundant set it to infinity.
                else if (rhsequal || cons0rhsred) && !scip.set.is_infinity(rhs) {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons1, cons0)?;

                    rhs = inf;

                    // If left hand side of cons1 is redundant too, set it to
                    // -infinity.
                    if cons1lhsred && !scip.set.is_infinity(-cd(scip, cons1).lhs) {
                        // Update flags of constraint which caused the redundancy s.t.
                        // nonredundant information doesn't get lost.
                        scip_cons::update_cons_flags(scip, cons0, cons1)?;

                        chg_lhs(scip, cons1, -inf)?;
                        *nchgsides += 1;
                    }

                    // Later on we do not want to delete cons1.
                    deletecons1 = false;
                }
                // If left hand side of cons1 is redundant set it to -infinity.
                else if cons1lhsred && !scip.set.is_infinity(-cd(scip, cons1).lhs) {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons0, cons1)?;

                    chg_lhs(scip, cons1, -inf)?;
                    *nchgsides += 1;
                    continue;
                }
                // If right hand side of cons1 is redundant set it to infinity.
                else if cons1rhsred && !scip.set.is_infinity(cd(scip, cons1).rhs) {
                    // Update flags of constraint which caused the redundancy s.t.
                    // nonredundant information doesn't get lost.
                    scip_cons::update_cons_flags(scip, cons0, cons1)?;

                    chg_rhs(scip, cons1, inf)?;
                    *nchgsides += 1;
                    continue;
                } else {
                    // Nothing was redundant.
                    continue;
                }
            } else {
                // There is no redundancy in both constraints with same variables.
                continue;
            }

            if scip.set.is_feas_lt(rhs, lhs) {
                *cutoff = true;
                break;
            }

            // Ensure that lhs <= rhs holds without tolerances as we only allow such
            // rows to enter the LP.
            if lhs > rhs {
                rhs = (lhs + rhs) / 2.0;
                lhs = rhs;
            }

            // We decide to let constraint cons0 stay, so update data structure
            // consdata0.

            // Update coefficient of cons0.

            // Special case if new coefficient becomes zero, both constraints are
            // redundant but we may tighten the bounds.
            if scip.set.is_zero(coef) {
                // Delete cons1.
                scip_cons::del_cons(scip, cons1)?;
                *ndelconss += 1;

                // Update upper bound if possible; we need to force the bound change
                // since we are deleting the constraint afterwards.
                let tightened;
                (*cutoff, tightened) = scip_var::tighten_var_ub(scip, var0, rhs, true)?;
                if *cutoff {
                    break;
                }
                if tightened {
                    *nchgbds += 1;
                }

                // Update lower bound if possible; we need to force the bound change
                // since we are deleting the constraint afterwards.
                let tightened;
                (*cutoff, tightened) = scip_var::tighten_var_lb(scip, var0, lhs, true)?;
                if *cutoff {
                    break;
                }
                if tightened {
                    *nchgbds += 1;
                }

                // Delete cons0.
                scip_cons::del_cons(scip, cons0)?;
                *ndelconss += 1;

                // Get next cons0.
                break;
            }

            // If sign of coefficient switches, update the locks of the variable.
            let d0 = cd(scip, cons0);
            let (d0coef, d0lhs, d0rhs) = (d0.vbdcoef, d0.lhs, d0.rhs);
            if d0coef * coef < 0.0 {
                let (haslhs, hasrhs) = (!scip.set.is_infinity(-d0lhs), !scip.set.is_infinity(d0rhs));

                // Remove locks for variable with old coefficient and install locks
                // for variable with new coefficient.
                if d0coef > 0.0 {
                    api::unlock_var_cons(scip, vbdvar0, cons0, haslhs, hasrhs)?;
                    api::lock_var_cons(scip, vbdvar0, cons0, hasrhs, haslhs)?;
                } else {
                    api::unlock_var_cons(scip, vbdvar0, cons0, hasrhs, haslhs)?;
                    api::lock_var_cons(scip, vbdvar0, cons0, haslhs, hasrhs)?;
                }
            }

            // Now change the coefficient.
            if !scip.set.is_eq(d0coef, coef) {
                *nchgcoefs += 1;

                // Mark to add new varbound information.
                let d0 = cdm(scip, cons0);
                d0.varboundsadded = false;
                d0.tightened = false;
                d0.presolved = false;
                d0.changed = false;

                d0.vbdcoef = coef;

                cons_mark_propagate(scip, cons0);
            }

            // Update lhs and rhs of cons0.
            if !scip.set.is_eq(cd(scip, cons0).lhs, lhs) {
                chg_lhs(scip, cons0, lhs)?;
                *nchgsides += 1;
            }
            if !scip.set.is_eq(cd(scip, cons0).rhs, rhs) {
                chg_rhs(scip, cons0, rhs)?;
                *nchgsides += 1;
            }

            // If cons1 is still marked for deletion, delete it.
            if deletecons1 {
                // Update flags of constraint which caused the redundancy s.t.
                // nonredundant information doesn't get lost.
                scip_cons::update_cons_flags(scip, cons0, cons1)?;

                // Delete cons1.
                scip_cons::del_cons(scip, cons1)?;
                *ndelconss += 1;
            }
        }
    }

    Ok(())
}

/// `prettifyConss`: for all varbound constraints with two integer variables
/// make the coefficients integral.
fn prettify_conss(scip: &mut Scip, conss: &[ConsId], nchgcoefs: &mut i32, nchgsides: &mut i32) {
    // If we cannot find any constraint for prettifying, stop.
    if api::get_n_int_vars(scip) + api::get_n_impl_vars(scip) < 1 {
        return;
    }

    for &cons in conss.iter().rev() {
        if scip.conss[cons].deleted {
            continue;
        }

        let c = cd(scip, cons);
        let (var, vbdvar, vbdcoef) = (c.var, c.vbdvar, c.vbdcoef);

        // Check for integer variables and one coefficient with an absolute value
        // smaller than 1; we allow that the variable type of the bounded
        // variable can be smaller than the variable type of the bounding
        // variable.
        if crate::var::var_is_integral(scip, var)
            && crate::var::var_is_integral(scip, vbdvar)
            && scip.vars[vbdvar].vartype != crate::types::VarType::Binary
            && scip.set.is_lt(vbdcoef.abs(), 1.0)
        {
            let feastol = scip.set.feastol();
            let eps = scip.set.epsilon();
            let mut maxmult = (feastol / eps + feastol) as i64;
            maxmult = crate::def::min(maxmult, MAXSCALEDCOEF);

            // This ensures that one coefficient in the scaled constraint will be
            // one as asserted below; 0.9 to be safe.
            let mut epsilon = eps / maxmult as f64;
            epsilon *= 0.9;

            let mut numerator = 0;
            let mut denominator = 0;
            let mut success = crate::misc::real_to_rational(vbdcoef, -epsilon, epsilon, maxmult, &mut numerator, &mut denominator);

            if success {
                // It is possible that the dominator is a multiple of the numerator.
                if scip.set.is_integral(denominator as f64 / numerator as f64) {
                    denominator /= numerator;
                    numerator = 1;
                }

                success = success && (denominator <= maxmult);

                // Scale the constraint denominator/numerator.
                if success && denominator.abs() > 1 && numerator == 1 {
                    let inf = scip.set.infinity();
                    let neglhsinf = scip.set.is_infinity(-cd(scip, cons).lhs);
                    let rhsinf = scip.set.is_infinity(cd(scip, cons).rhs);
                    let d = denominator as f64;
                    let c = cdm(scip, cons);

                    // Need to switch sides if coefficient is smaller then 0.
                    if c.vbdcoef < 0.0 {
                        // Compute new sides.

                        // Only right hand side exists.
                        if neglhsinf {
                            c.lhs = c.rhs * d;
                            c.rhs = inf;
                        }
                        // Only left hand side exists.
                        else if rhsinf {
                            c.rhs = c.lhs * d;
                            c.lhs = -inf;
                        }
                        // Both sides exist.
                        else {
                            let tmp = c.lhs;
                            c.lhs = c.rhs * d;
                            c.rhs = tmp * d;
                            c.tightened = false;
                        }
                        *nchgsides += 2;
                    }
                    // Coefficient > 0.
                    else {
                        // Compute new left hand side.
                        if !neglhsinf {
                            c.lhs *= d;
                            *nchgsides += 1;
                        }

                        // Compute new right hand side.
                        if !rhsinf {
                            c.rhs *= d;
                            *nchgsides += 1;
                        }
                    }

                    // Swap both variables.
                    std::mem::swap(&mut c.var, &mut c.vbdvar);

                    // Swap coefficient.
                    c.vbdcoef = d;
                    *nchgcoefs += 1;

                    // Mark to add new varbound information.
                    c.varboundsadded = false;
                    c.tightened = false;
                }
            }
        }
    }
}

/// Whether the constraint's left and right hand sides are finite.
#[inline]
fn finite_sides(scip: &Scip, cons: ConsId) -> (bool, bool) {
    let c = cd(scip, cons);
    (!scip.set.is_infinity(-c.lhs), !scip.set.is_infinity(c.rhs))
}

/// Locks (or unlocks) `var` of the constraint for its current sides.
fn lock_var(scip: &mut Scip, cons: ConsId, lock: bool) -> ScipResult {
    let (haslhs, hasrhs) = finite_sides(scip, cons);
    let var = cd(scip, cons).var;
    if lock {
        api::lock_var_cons(scip, var, cons, haslhs, hasrhs)
    } else {
        api::unlock_var_cons(scip, var, cons, haslhs, hasrhs)
    }
}

/// Locks (or unlocks) `vbdvar` of the constraint for its current sides and
/// the sign of its coefficient.
fn lock_vbdvar(scip: &mut Scip, cons: ConsId, lock: bool) -> ScipResult {
    let (haslhs, hasrhs) = finite_sides(scip, cons);
    let (vbdvar, vbdcoef) = (cd(scip, cons).vbdvar, cd(scip, cons).vbdcoef);
    let (down, up) = if vbdcoef > 0.0 { (haslhs, hasrhs) } else { (hasrhs, haslhs) };
    if lock {
        api::lock_var_cons(scip, vbdvar, cons, down, up)
    } else {
        api::unlock_var_cons(scip, vbdvar, cons, down, up)
    }
}

/// Tightens the lower (`lower`) or upper bound of `var` to `newbnd`, forced,
/// counting a tightening in `nchgbds`.
fn tighten_forced(scip: &mut Scip, var: VarId, newbnd: f64, lower: bool, cutoff: &mut bool, nchgbds: &mut i32) -> ScipResult {
    let tightened;
    (*cutoff, tightened) = if lower {
        scip_var::tighten_var_lb(scip, var, newbnd, true)?
    } else {
        scip_var::tighten_var_ub(scip, var, newbnd, true)?
    };
    if !*cutoff && tightened {
        *nchgbds += 1;
    }
    Ok(())
}

/// `applyFixings`: replaces fixed and aggregated variables in variable bound
/// constraint by active problem variables.
#[allow(clippy::too_many_arguments)]
fn apply_fixings(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, cutoff: &mut bool, nchgbds: &mut i32, ndelconss: &mut i32, naddconss: &mut i32) -> ScipResult {
    *cutoff = false;
    let mut redundant = false;

    // The variable bound constraint is: lhs <= x + c*y <= rhs.

    // Get active problem variables of x and y.
    let mut var = cd(scip, cons).var;
    let mut varscalar = 1.0;
    let mut varconstant = 0.0;
    crate::var::var_get_probvar_sum(scip, &mut var, &mut varscalar, &mut varconstant)?;
    let mut vbdvar = cd(scip, cons).vbdvar;
    let mut vbdvarscalar = 1.0;
    let mut vbdvarconstant = 0.0;
    crate::var::var_get_probvar_sum(scip, &mut vbdvar, &mut vbdvarscalar, &mut vbdvarconstant)?;
    let varschanged = var != cd(scip, cons).var || vbdvar != cd(scip, cons).vbdvar;

    // If the variables are equal, the variable bound constraint reduces to
    // standard bounds on the single variable.
    if var == vbdvar && scip.vars[var].varstatus != VarStatus::MultAggr {
        //    lhs <= a1*z + b1 + c(a2*z + b2) <= rhs
        // <=>  lhs <= (a1 + c*a2)z + (b1 + c*b2) <= rhs
        let c = cd(scip, cons);
        let (lhs, rhs, vbdcoef) = (c.lhs, c.rhs, c.vbdcoef);
        let scalar = varscalar + vbdcoef * vbdvarscalar;
        let constant = varconstant + vbdcoef * vbdvarconstant;
        if scip.set.is_zero(scalar) {
            // No variable is left: the constraint is redundant or infeasible.
            if scip.set.is_feas_lt(constant, lhs) || scip.set.is_feas_gt(constant, rhs) {
                *cutoff = true;
                return Ok(());
            }
        } else {
            // With scalar > 0 the lhs bounds from below, else from above.
            let pos = scalar > 0.0;
            if !scip.set.is_infinity(-lhs) {
                let newbnd = (lhs - constant) / scalar;
                tighten_forced(scip, var, newbnd, pos, cutoff, nchgbds)?;
                if *cutoff {
                    return Ok(());
                }
            }
            if !scip.set.is_infinity(rhs) {
                let newbnd = (rhs - constant) / scalar;
                tighten_forced(scip, var, newbnd, !pos, cutoff, nchgbds)?;
                if *cutoff {
                    return Ok(());
                }
            }
        }
        redundant = true;
    } else {
        // If the variables should be replaced, drop the events and catch the
        // events on the new variables afterwards.
        if varschanged {
            drop_events(scip, cons, eventhdlr)?;
        }

        // Apply aggregation on x.
        if scip.set.is_zero(varscalar) {
            let c = cd(scip, cons);
            let (lhs, rhs, vbdcoef, cvbdvar) = (c.lhs, c.rhs, c.vbdcoef, c.vbdvar);
            // The variable being fixed or corresponding to an aggregation might
            // lead to numerical difficulties.
            if scip.set.is_zero(vbdcoef * vbdvarscalar) {
                let activity = varconstant + vbdcoef * vbdvarconstant;

                if (!scip.set.is_infinity(-lhs) && scip.set.is_feas_lt(activity, lhs)) || (!scip.set.is_infinity(rhs) && scip.set.is_feas_gt(activity, rhs)) {
                    *cutoff = true;
                }

                redundant = true;
            }
            // Cannot change bounds on multi-aggregated variables.
            else if scip.vars[vbdvar].varstatus != VarStatus::MultAggr {
                // x is fixed to varconstant: update bounds of y and delete the
                // variable bound constraint.
                if !*cutoff && !scip.set.is_infinity(-lhs) {
                    let newbnd = (lhs - varconstant) / vbdcoef;
                    tighten_forced(scip, cvbdvar, newbnd, vbdcoef > 0.0, cutoff, nchgbds)?;
                }
                if !*cutoff && !scip.set.is_infinity(rhs) {
                    let newbnd = (rhs - varconstant) / vbdcoef;
                    tighten_forced(scip, cvbdvar, newbnd, vbdcoef <= 0.0, cutoff, nchgbds)?;
                }
                redundant = true;
            }
        } else if var != cd(scip, cons).var {
            // Release and unlock old variable.
            lock_var(scip, cons, false)?;
            let mut oldvar = cd(scip, cons).var;
            api::release_var(scip, &mut oldvar)?;

            // Unlock vbdvar, because we possibly change lhs/rhs/vbdcoef.
            lock_vbdvar(scip, cons, false)?;

            let inf_neg_lhs = scip.set.is_infinity(-cd(scip, cons).lhs);
            let inf_rhs = scip.set.is_infinity(cd(scip, cons).rhs);

            // Replace aggregated variable x in the constraint by its aggregation.
            if varscalar > 0.0 {
                // lhs := (lhs - varconstant) / varscalar
                // rhs := (rhs - varconstant) / varscalar
                // c   := c / varscalar
                let c = cdm(scip, cons);
                if !inf_neg_lhs {
                    c.lhs = (c.lhs - varconstant) / varscalar;
                }
                if !inf_rhs {
                    c.rhs = (c.rhs - varconstant) / varscalar;
                }
                c.vbdcoef /= varscalar;
            } else {
                // lhs := (rhs - varconstant) / varscalar
                // rhs := (lhs - varconstant) / varscalar
                // c   := c / varscalar
                let c = cdm(scip, cons);
                let lhs = c.lhs;
                c.lhs = -c.rhs;
                c.rhs = -lhs;
                let (nl, nr) = (c.lhs, c.rhs);
                let (nlinf, nrinf) = (scip.set.is_infinity(-nl), scip.set.is_infinity(nr));
                let c = cdm(scip, cons);
                if !nlinf {
                    c.lhs = (c.lhs + varconstant) / (-varscalar);
                }
                if !nrinf {
                    c.rhs = (c.rhs + varconstant) / (-varscalar);
                }
                c.vbdcoef /= varscalar;
            }

            // Try to avoid numerical troubles.
            let coef = cd(scip, cons).vbdcoef;
            if scip.set.is_integral(coef) {
                let r = scip.set.round(coef);
                cdm(scip, cons).vbdcoef = r;
            }

            let c = cdm(scip, cons);
            c.tightened = false;
            c.var = var;

            // Capture and lock new variable.
            crate::var::var_capture(scip, var);
            lock_var(scip, cons, true)?;

            // Lock vbdvar.
            lock_vbdvar(scip, cons, true)?;
        }

        // Apply aggregation on y.
        let c = cd(scip, cons);
        let (lhs, rhs, vbdcoef, cvar) = (c.lhs, c.rhs, c.vbdcoef, c.var);
        if scip.set.is_zero(vbdcoef * vbdvarscalar) {
            // Cannot change bounds on multi-aggregated variables.
            if !redundant && scip.vars[var].varstatus != VarStatus::MultAggr {
                // y is fixed to vbdvarconstant: update bounds of x and delete the
                // variable bound constraint.
                if !*cutoff && !scip.set.is_infinity(-lhs) {
                    let newbnd = lhs - vbdcoef * vbdvarconstant;
                    tighten_forced(scip, cvar, newbnd, true, cutoff, nchgbds)?;
                }
                if !*cutoff && !scip.set.is_infinity(rhs) {
                    let newbnd = rhs - vbdcoef * vbdvarconstant;
                    tighten_forced(scip, cvar, newbnd, false, cutoff, nchgbds)?;
                }
                redundant = true;
            }
        } else if !*cutoff && vbdvar != cd(scip, cons).vbdvar {
            // Release and unlock old variable.
            lock_vbdvar(scip, cons, false)?;
            let mut oldvbdvar = cd(scip, cons).vbdvar;
            api::release_var(scip, &mut oldvbdvar)?;

            // Also unlock var, because we possibly change lhs/rhs/vbdcoef.
            lock_var(scip, cons, false)?;

            // Replace aggregated variable y in the constraint by its aggregation:
            // lhs := lhs - c * vbdvarconstant
            // rhs := rhs - c * vbdvarconstant
            // c   := c * vbdvarscalar
            let (haslhs, hasrhs) = finite_sides(scip, cons);
            let c = cdm(scip, cons);
            if haslhs {
                c.lhs -= c.vbdcoef * vbdvarconstant;
            }
            if hasrhs {
                c.rhs -= c.vbdcoef * vbdvarconstant;
            }

            c.tightened = false;
            c.vbdcoef *= vbdvarscalar;
            c.vbdvar = vbdvar;

            // Capture and lock new variable.
            crate::var::var_capture(scip, vbdvar);
            lock_vbdvar(scip, cons, true)?;
            lock_var(scip, cons, true)?;
        }

        // Catch the events again on the new variables.
        if varschanged {
            catch_events(scip, cons, eventhdlr)?;
        }

        // Terminate on cutoff after catching events.
        if *cutoff {
            return Ok(());
        }
    }

    // Mark constraint changed, if a variable was exchanged.
    if varschanged {
        cdm(scip, cons).changed = true;
    }

    // Active multi aggregations are now resolved by creating a new linear
    // constraint.
    if !redundant && (scip.vars[var].varstatus == VarStatus::MultAggr || scip.vars[vbdvar].varstatus == VarStatus::MultAggr) {
        let c = cd(scip, cons);
        let (lhs, rhs) = (c.lhs, c.rhs);

        // Create upgraded linear constraint.
        let name = scip.conss[cons].name.clone();
        let f = cons_flags(scip, cons);
        let mut newcons = crate::cons_linear::create_cons_linear(
            scip,
            &name,
            &[],
            &[],
            lhs,
            rhs,
            f.initial,
            f.separate,
            f.enforce,
            f.check,
            f.propagate,
            f.local,
            f.modifiable,
            f.dynamic,
            f.removable,
            f.stickingatnode,
        )?;

        // If var was fixed, then the case that vbdvar was multi-aggregated, was
        // not yet resolved.
        if var != cd(scip, cons).var {
            // Add offset that results from the fixed variable.
            if !scip.set.is_zero(varconstant) {
                if !scip.set.is_infinity(rhs) {
                    crate::cons_linear::chg_rhs_linear(scip, newcons, rhs - varconstant)?;
                }
                if !scip.set.is_infinity(-lhs) {
                    crate::cons_linear::chg_lhs_linear(scip, newcons, lhs - varconstant)?;
                }
            }
        } else {
            let cvar = cd(scip, cons).var;
            crate::cons_linear::add_coef_linear(scip, newcons, cvar, 1.0)?;
        }

        // If vbdvar was fixed, then the case that var was multi-aggregated, was
        // not yet resolved.
        if vbdvar != cd(scip, cons).vbdvar {
            // Add offset that results from the fixed variable.
            if !scip.set.is_zero(vbdvarconstant) {
                let vbdcoef = cd(scip, cons).vbdcoef;
                if !scip.set.is_infinity(rhs) {
                    crate::cons_linear::chg_rhs_linear(scip, newcons, rhs - vbdcoef * vbdvarconstant)?;
                }
                if !scip.set.is_infinity(-lhs) {
                    crate::cons_linear::chg_lhs_linear(scip, newcons, lhs - vbdcoef * vbdvarconstant)?;
                }
            }
        } else {
            let (cvbdvar, vbdcoef) = (cd(scip, cons).vbdvar, cd(scip, cons).vbdcoef);
            crate::cons_linear::add_coef_linear(scip, newcons, cvbdvar, vbdcoef)?;
        }

        scip_cons::add_cons(scip, newcons)?;

        scip_cons::release_cons(scip, &mut newcons)?;

        redundant = true;
        *naddconss += 1;
    }

    // Delete a redundant constraint.
    if redundant {
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
    }

    Ok(())
}

/// `tightenCoefs`: tightens variable bound coefficient by inspecting the
/// global bounds of the involved variables; this is also performed by the
/// linear constraint handler, only necessary if the user directly creates
/// variable bound constraints.
#[allow(clippy::too_many_arguments)]
fn tighten_coefs(scip: &mut Scip, cons: ConsId, nchgcoefs: &mut i32, nchgsides: &mut i32, ndelconss: &mut i32, cutoff: &mut bool, nchgbds: &mut i32) -> ScipResult {
    // Tightening already done.
    if cd(scip, cons).tightened {
        return Ok(());
    }

    cdm(scip, cons).tightened = true;

    let (var, vbdvar) = (cd(scip, cons).var, cd(scip, cons).vbdvar);

    // If values and variable are integral the sides should it be too.
    if crate::var::var_is_integral(scip, var) && crate::var::var_is_integral(scip, vbdvar) && scip.set.is_integral(cd(scip, cons).vbdcoef) {
        let lhs = cd(scip, cons).lhs;
        if !scip.set.is_integral(lhs) {
            let v = scip.set.feas_ceil(lhs);
            let c = cdm(scip, cons);
            c.lhs = v;
            *nchgsides += 1;
            c.changed = true;
        }
        let rhs = cd(scip, cons).rhs;
        if !scip.set.is_integral(rhs) {
            let v = scip.set.feas_floor(rhs);
            let c = cdm(scip, cons);
            c.rhs = v;
            *nchgsides += 1;
            c.changed = true;
        }
    }

    // Coefficient tightening only works for binary bound variable.
    if !crate::var::var_is_binary(scip, vbdvar) {
        return Ok(());
    }

    let oldnchgcoefs = *nchgcoefs;
    let oldnchgsides = *nchgsides;
    let oldcoef = cd(scip, cons).vbdcoef;

    // Coefficients tightening when all variables are integer: for lhs <= x +
    // b*y <= rhs with binary y, integral x and fractional b, and at least one
    // side infinite or fractional:
    //
    // 1. if (lhs integral and finite) and (rhs infinite or b - floor(b) <=
    //    rhs - floor(rhs)): lhs <= x + floor(b)*y <= floor(rhs)
    // 2. if (rhs integral and finite) and (lhs infinite or b - floor(b) >=
    //    lhs - floor(lhs)): ceil(lhs) <= x + ceil(b)*y <= rhs
    // 3. if (lhs infinite or b - floor(b) >= lhs - floor(lhs)) and (rhs
    //    infinite or b - floor(b) > rhs - floor(rhs)): ceil(lhs) <= x +
    //    ceil(b)*y <= floor(rhs)
    // 4. if (lhs infinite or b - floor(b) < lhs - floor(lhs)) and (rhs
    //    infinite or b - floor(b) <= rhs - floor(rhs)): ceil(lhs) <= x +
    //    floor(b)*y <= floor(rhs)
    // 5. if lhs or rhs is fractional: a finite lhs becomes ceil(lhs) if b -
    //    floor(b) < lhs - floor(lhs), floor(lhs) + b - floor(b) if greater; a
    //    finite rhs becomes floor(rhs) + b - floor(b) if b - floor(b) < rhs -
    //    floor(rhs), floor(rhs) if greater.
    {
        let s = &scip.set;
        let c = cd(scip, cons);
        let (lhs, rhs, b) = (c.lhs, c.rhs, c.vbdcoef);
        if crate::var::var_is_integral(scip, var) && !s.is_integral(b) && (!s.is_integral(lhs) || s.is_infinity(-lhs) || !s.is_integral(rhs) || s.is_infinity(rhs)) {
            // The fractional part of b, of lhs and of rhs (feasibility floors).
            let bfrac = |scip: &Scip| {
                let b = cd(scip, cons).vbdcoef;
                b - scip.set.feas_floor(b)
            };
            let lfrac = |scip: &Scip| {
                let l = cd(scip, cons).lhs;
                l - scip.set.feas_floor(l)
            };
            let rfrac = |scip: &Scip| {
                let r = cd(scip, cons).rhs;
                r - scip.set.feas_floor(r)
            };

            // Ceils the lhs, counting a side change if it was fractional.
            let ceil_lhs = |scip: &mut Scip, nchgsides: &mut i32| {
                let l = cd(scip, cons).lhs;
                if !scip.set.is_infinity(-l) {
                    if !scip.set.is_integral(l) {
                        *nchgsides += 1;
                    }
                    let v = scip.set.feas_ceil(l);
                    cdm(scip, cons).lhs = v;
                }
            };
            // Floors the rhs, counting a side change if it was fractional.
            let floor_rhs = |scip: &mut Scip, nchgsides: &mut i32| {
                let r = cd(scip, cons).rhs;
                if !scip.set.is_infinity(r) {
                    if !scip.set.is_integral(r) {
                        *nchgsides += 1;
                    }
                    let v = scip.set.feas_floor(r);
                    cdm(scip, cons).rhs = v;
                }
            };

            // Case 1.
            if s.is_integral(lhs) && !s.is_infinity(-lhs) && (s.is_infinity(rhs) || s.is_feas_le(bfrac(scip), rfrac(scip))) {
                let v = scip.set.feas_floor(b);
                cdm(scip, cons).vbdcoef = v;
                *nchgcoefs += 1;

                if !scip.set.is_infinity(rhs) {
                    let v = scip.set.feas_floor(rhs);
                    cdm(scip, cons).rhs = v;
                    *nchgsides += 1;
                }
            }
            // Case 2.
            else if s.is_integral(rhs) && !s.is_infinity(rhs) && (s.is_infinity(-lhs) || s.is_feas_ge(bfrac(scip), lfrac(scip))) {
                let v = scip.set.feas_ceil(b);
                cdm(scip, cons).vbdcoef = v;
                *nchgcoefs += 1;

                ceil_lhs(scip, nchgsides);
            }
            // Case 3.
            else if (s.is_infinity(-lhs) || s.is_feas_ge(bfrac(scip), lfrac(scip))) && (s.is_infinity(rhs) || s.is_feas_gt(bfrac(scip), rfrac(scip))) {
                let v = scip.set.feas_ceil(b);
                cdm(scip, cons).vbdcoef = v;
                *nchgcoefs += 1;

                ceil_lhs(scip, nchgsides);
                floor_rhs(scip, nchgsides);
            }
            // Case 4.
            else if (s.is_infinity(-lhs) || s.is_feas_lt(bfrac(scip), lfrac(scip))) && (s.is_infinity(rhs) || s.is_feas_le(bfrac(scip), rfrac(scip))) {
                let v = scip.set.feas_floor(b);
                cdm(scip, cons).vbdcoef = v;
                *nchgcoefs += 1;

                ceil_lhs(scip, nchgsides);
                floor_rhs(scip, nchgsides);
            }

            // Case 5.
            let c = cd(scip, cons);
            if !scip.set.is_feas_integral(c.lhs) || !scip.set.is_feas_integral(c.rhs) {
                let lhs = cd(scip, cons).lhs;
                if !scip.set.is_infinity(-lhs) {
                    if scip.set.is_feas_lt(bfrac(scip), lfrac(scip)) {
                        let v = scip.set.feas_ceil(lhs);
                        cdm(scip, cons).lhs = v;
                        *nchgsides += 1;
                    } else if scip.set.is_feas_gt(bfrac(scip), lfrac(scip)) {
                        let v = scip.set.feas_floor(lhs) + bfrac(scip);
                        cdm(scip, cons).lhs = v;
                        *nchgsides += 1;
                    }
                }
                let rhs = cd(scip, cons).rhs;
                if !scip.set.is_infinity(rhs) {
                    if scip.set.is_feas_lt(bfrac(scip), rfrac(scip)) {
                        let v = scip.set.feas_floor(rhs) + bfrac(scip);
                        cdm(scip, cons).rhs = v;
                        *nchgsides += 1;
                    } else if scip.set.is_feas_gt(bfrac(scip), rfrac(scip)) {
                        let v = scip.set.feas_floor(rhs);
                        cdm(scip, cons).rhs = v;
                        *nchgsides += 1;
                    }
                }
            }
        }
    }

    // Check if due to tightening the constraint got redundant.
    if scip.set.is_zero(cd(scip, cons).vbdcoef) {
        // We have to make sure that the induced bound(s) is (are) actually
        // applied; if the relative change is too small, this may have been
        // skipped in propagation.
        let lhs = cd(scip, cons).lhs;
        if scip.set.is_lt(scip.vars[var].glbdom.lb, lhs) {
            let tightened;
            (*cutoff, tightened) = scip_var::tighten_var_lb_global(scip, var, lhs, true)?;
            if *cutoff {
                return Ok(());
            }
            if tightened {
                *nchgbds += 1;
            }
        }
        let rhs = cd(scip, cons).rhs;
        if scip.set.is_gt(scip.vars[var].glbdom.ub, rhs) {
            let tightened;
            (*cutoff, tightened) = scip_var::tighten_var_ub_global(scip, var, rhs, true)?;
            if *cutoff {
                return Ok(());
            }
            if tightened {
                *nchgbds += 1;
            }
        }

        // In order to correctly update the rounding locks, we need the
        // coefficient to have the same sign as before the coefficient
        // tightening.
        cdm(scip, cons).vbdcoef = oldcoef;

        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;

        return Ok(());
    }

    // Get bounds of variable x.
    let xlb = scip.vars[var].glbdom.lb;
    let xub = scip.vars[var].glbdom.ub;

    // It can happen that var is not of varstatus SCIP_VARSTATUS_FIXED but the
    // bounds are equal, in this case we need to stop.
    if scip.set.is_eq(xlb, xub) {
        return Ok(());
    }

    let c = cd(scip, cons);
    let (lhs, rhs, vbdcoef) = (c.lhs, c.rhs, c.vbdcoef);
    let s = &scip.set;

    // Modification of coefficient checking for slack in constraints.
    if !s.is_infinity(-lhs) && !s.is_infinity(rhs) {
        // lhs <= x + c*y <= rhs  =>  lhs - c*y <= x <= rhs - c*y
        if vbdcoef > 0.0 && s.is_feas_gt(xlb, lhs - vbdcoef) && s.is_feas_lt(xub, rhs) {
            let oldrhs = rhs;

            // Constraint has positive slack for both non-restricting cases y = 0,
            // or y = 1, respectively -> modify coefficients such that constraint
            // is tight in at least one of the non-restricting cases -> c' =
            // MAX(c - rhs + xub, lhs - xlb), rhs' = rhs - c + c'.
            let newcoef = crate::def::max(vbdcoef - rhs + xub, lhs - xlb);

            // In this case both sides are redundant and the constraint can be
            // removed.
            if s.is_le(newcoef, 0.0) {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            } else {
                let newrhs = rhs - vbdcoef + newcoef;

                // We cannot allow that the coefficient changes the sign because of
                // the rounding locks.
                let c = cdm(scip, cons);
                c.vbdcoef = newcoef;
                c.rhs = crate::def::max(newrhs, c.lhs);
                *nchgcoefs += 1;
                *nchgsides += 1;

                // Some of the cases 1. to 5. might be applicable after changing the
                // rhs to an integral value; one example is the varbound constraint
                // 0.225 <= x - 1.225 y <= 0.775 for which none of the above cases
                // apply but after tightening the lhs to 0.0 it is possible to reduce
                // the rhs by applying the 1. reduction.
                if !scip.set.is_feas_integral(oldrhs) && scip.set.is_feas_integral(newrhs) {
                    cdm(scip, cons).tightened = false;
                    tighten_coefs(scip, cons, nchgcoefs, nchgsides, ndelconss, cutoff, nchgbds)?;
                } else {
                    let c = cd(scip, cons);
                    let t = scip.set.is_integral(c.vbdcoef) && scip.set.is_integral(c.rhs);
                    cdm(scip, cons).tightened = t;
                }
            }
        } else if vbdcoef < 0.0 && s.is_feas_gt(xlb, lhs) && s.is_feas_lt(xub, rhs - vbdcoef) {
            let oldlhs = lhs;

            // Constraint has positive slack for both non-restricting cases y = 0,
            // or y = 1, respectively -> modify coefficients such that constraint
            // is tight in at least one of the non-restricting cases -> c' =
            // MIN(c - lhs + xlb, rhs - xub), lhs' = lhs - c + c'.
            let newcoef = crate::def::min(vbdcoef - lhs + xlb, rhs - xub);

            // In this case both sides are redundant and the constraint can be
            // removed.
            if s.is_ge(newcoef, 0.0) {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            } else {
                let newlhs = lhs - vbdcoef + newcoef;

                // We cannot allow that the coefficient changes the sign because of
                // the rounding locks.
                let c = cdm(scip, cons);
                c.vbdcoef = newcoef;
                c.lhs = crate::def::min(newlhs, c.rhs);
                *nchgcoefs += 1;
                *nchgsides += 1;

                // Some of the cases 1. to 5. might be applicable after changing the
                // rhs to an integral value (see above).
                if !scip.set.is_feas_integral(oldlhs) && scip.set.is_feas_integral(newlhs) {
                    cdm(scip, cons).tightened = false;
                    tighten_coefs(scip, cons, nchgcoefs, nchgsides, ndelconss, cutoff, nchgbds)?;
                } else {
                    let c = cd(scip, cons);
                    let t = scip.set.is_integral(c.vbdcoef) && scip.set.is_integral(c.lhs);
                    cdm(scip, cons).tightened = t;
                }
            }
        }
    } else if !s.is_infinity(-lhs) && s.is_infinity(rhs) {
        // lhs <= x + c*y  =>  x >= lhs - c*y
        if vbdcoef > 0.0 && s.is_feas_gt(xlb, lhs - vbdcoef) {
            // Constraint has positive slack for the non-restricting case y = 1 ->
            // modify coefficients such that constraint is tight in the
            // non-restricting case y = 1 and equivalent in the restricting case
            // y = 0 -> c' = lhs - xlb.
            let newcoef = lhs - xlb;

            // In this case the constraint is redundant and can be removed.
            if s.is_le(newcoef, 0.0) {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            } else {
                // We cannot allow that the coefficient changes the sign because of
                // the rounding locks.
                cdm(scip, cons).vbdcoef = newcoef;
                *nchgcoefs += 1;
            }
        } else if vbdcoef < 0.0 && s.is_feas_gt(xlb, lhs) {
            // Constraint has positive slack for the non-restricting case y = 0 ->
            // modify coefficients such that constraint is tight in the
            // non-restricting case y = 0 and equivalent in the restricting case
            // y = 1 -> c' = c - lhs + xlb, lhs' = xlb.
            let newcoef = vbdcoef - lhs + xlb;

            // In this case the constraint is redundant and can be removed.
            if s.is_ge(newcoef, 0.0) {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            } else {
                // We cannot allow that the coefficient changes the sign because of
                // the rounding locks.
                let c = cdm(scip, cons);
                c.vbdcoef = newcoef;
                c.lhs = xlb;
                *nchgcoefs += 1;
                *nchgsides += 1;
            }
        }
    } else if s.is_infinity(-lhs) && !s.is_infinity(rhs) {
        // x + c*y <= rhs  =>  x <= rhs - c*y
        if vbdcoef > 0.0 && s.is_feas_lt(xub, rhs) {
            // Constraint has positive slack for the non-restricting case y = 0 ->
            // modify coefficients such that constraint is tight in the
            // non-restricting case y = 0 and equivalent in the restricting case
            // y = 1 -> c' = c - rhs + xub, rhs' = xub.
            let newcoef = vbdcoef - rhs + xub;

            // In this case the constraint is redundant and can be removed.
            if s.is_le(newcoef, 0.0) {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            } else {
                // We cannot allow that the coefficient changes the sign because of
                // the rounding locks.
                let c = cdm(scip, cons);
                c.vbdcoef = newcoef;
                c.rhs = xub;
                *nchgcoefs += 1;
                *nchgsides += 1;
            }
        } else if vbdcoef < 0.0 && s.is_feas_lt(xub, rhs - vbdcoef) {
            // Constraint has positive slack for the non-restricting case y = 1 ->
            // modify coefficients such that constraint is tight in the
            // non-restricting case y = 1 and equivalent in the restricting case
            // y = 0 -> c' = rhs - xub.
            let newcoef = rhs - xub;

            // In this case the constraint is redundant and can be removed.
            if s.is_ge(newcoef, 0.0) {
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
            } else {
                // We cannot allow that the coefficient changes the sign because of
                // the rounding locks.
                cdm(scip, cons).vbdcoef = newcoef;
                *nchgcoefs += 1;
            }
        }
    }

    // If something a coefficient or side of the varbound constraint was
    // changed, ensure that the variable lower or upper bounds of the variables
    // are informed.
    if *nchgcoefs > oldnchgcoefs || *nchgsides > oldnchgsides {
        let c = cdm(scip, cons);
        c.varboundsadded = false;
        c.changed = true;

        cons_mark_propagate(scip, cons);
    }

    Ok(())
}

/// `upgradeConss`: check if we can upgrade to a set-packing constraint.
#[allow(clippy::too_many_arguments)]
fn upgrade_conss(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    cutoff: &mut bool,
    naggrvars: &mut i32,
    nchgbds: &mut i32,
    nchgcoefs: &mut i32,
    nchgsides: &mut i32,
    ndelconss: &mut i32,
    naddconss: &mut i32,
) -> ScipResult {
    // If we cannot find any constraint for upgrading, stop.
    if api::get_n_bin_vars(scip) + api::get_n_impl_vars(scip) <= 1 {
        return Ok(());
    }

    if conss.is_empty() {
        return Ok(());
    }

    let (eventhdlr, usebdwidening) = (hd(scip, conshdlr).eventhdlr, hd(scip, conshdlr).usebdwidening);

    for &cons in conss.iter().rev() {
        if !scip.conss[cons].is_active() || scip.conss[cons].nupgradelocks >= 1 {
            continue;
        }

        if !cd(scip, cons).presolved {
            // Incorporate fixings and aggregations in constraint.
            apply_fixings(scip, cons, eventhdlr, cutoff, nchgbds, ndelconss, naddconss)?;

            if *cutoff {
                return Ok(());
            }
            if !scip.conss[cons].is_active() {
                continue;
            }
        }

        if scip.conss[cons].is_marked_propagate() {
            // Propagate constraint.
            propagate_cons(scip, cons, usebdwidening, cutoff, nchgbds, Some(nchgsides), Some(ndelconss))?;

            if *cutoff {
                return Ok(());
            }
            if !scip.conss[cons].is_active() {
                continue;
            }
        }

        if !cd(scip, cons).tightened {
            // Tighten variable bound coefficient.
            tighten_coefs(scip, cons, nchgcoefs, nchgsides, ndelconss, cutoff, nchgbds)?;

            if *cutoff {
                return Ok(());
            }
            if !scip.conss[cons].is_active() {
                continue;
            }
        }

        // Check if both variables are of binary type.
        let c = cd(scip, cons);
        let (var, vbdvar, vbdcoef, lhs, rhs) = (c.var, c.vbdvar, c.vbdcoef, c.lhs, c.rhs);
        if crate::var::var_is_binary(scip, vbdvar) && crate::var::var_is_binary(scip, var) {
            // Coefficient and sides should be tightened and we assume that the
            // constraint is not redundant.
            let vars: [VarId; 2];

            // The case x + y <= 1 or x + y >= 1.
            if vbdcoef > 0.0 {
                if scip.set.is_eq(rhs, 1.0) {
                    // Check for aggregations like x + y == 1.
                    if scip.set.is_eq(lhs, 1.0) {
                        // Aggregate both variables.
                        scip_var::aggregate_vars(scip, var, vbdvar, 1.0, 1.0, 1.0)?;
                        *naggrvars += 1;

                        scip_cons::del_cons(scip, cons)?;
                        *ndelconss += 1;

                        continue;
                    }
                    vars = [var, vbdvar];
                } else {
                    let v0 = scip_var::get_negated_var(scip, var)?;
                    let v1 = scip_var::get_negated_var(scip, vbdvar)?;
                    vars = [v0, v1];
                }
            }
            // The case x - y <= 0 or x - y >= 0.
            else {
                // The case x - y <= 0.
                if scip.set.is_zero(rhs) {
                    // Check for aggregations like x - y == 0.
                    if scip.set.is_zero(lhs) {
                        // Aggregate both variables.
                        scip_var::aggregate_vars(scip, var, vbdvar, 1.0, -1.0, 0.0)?;
                        *naggrvars += 1;

                        scip_cons::del_cons(scip, cons)?;
                        *ndelconss += 1;

                        continue;
                    }
                    let v1 = scip_var::get_negated_var(scip, vbdvar)?;
                    vars = [var, v1];
                }
                // The case x - y >= 0.
                else {
                    let v0 = scip_var::get_negated_var(scip, var)?;
                    vars = [v0, vbdvar];
                }
            }

            let name = scip.conss[cons].name.clone();
            let flags = cons_flags(scip, cons);
            let mut newcons = crate::cons_setppc::create_cons_setpack(scip, &name, &vars, &flags)?;

            scip_cons::add_cons_upgrade(scip, cons, &mut newcons)?;
            *naddconss += 1;

            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
        }
    }

    Ok(())
}

/*
 * Linear constraint upgrading
 */

/// `linconsUpgdVarbound`: tries to upgrade a linear constraint into a
/// variable bound constraint `lhs <= x + a*y <= rhs`: there are exactly two
/// variables, one of them non-binary (the bounded variable x) and one
/// non-continuous (the bounding variable y).
fn lincons_upgd_varbound(scip: &mut Scip, cons: ConsId, s: &crate::cons_linear::LinConsStats, upgdcons: &mut ConsId) -> ScipResult {
    let (vars, vals, lhs, rhs) = (&s.vars, &s.vals, s.lhs, s.rhs);
    let upgrade = vars.len() == 2 && (s.nposbin + s.nnegbin <= 1) && (s.nposcont + s.nnegcont <= 1);

    if upgrade {
        let vtype = |v: VarId| {
            if crate::var::var_is_implied_integral(scip, v) {
                crate::types::VarType::ImplInt as i32
            } else {
                scip.vars[v].vartype as i32
            }
        };
        let zerotype = vtype(vars[0]);
        let onetype = vtype(vars[1]);
        let set = &scip.set;

        // Decide which variable we want to use as bounding variable y.
        let vbdind = if zerotype < onetype {
            0
        } else if zerotype > onetype {
            1
        } else if set.is_integral(vals[0]) && !set.is_integral(vals[1]) {
            0
        } else if !set.is_integral(vals[0]) && set.is_integral(vals[1]) {
            1
        } else if (vals[0].abs() - 1.0).abs() < (vals[1].abs() - 1.0).abs() {
            1
        } else {
            0
        };

        // Do not upgrade when it is numerical unstable.
        if set.is_zero(vals[vbdind] / vals[1 - vbdind]) {
            return Ok(());
        }

        let var = vars[1 - vbdind];
        let vbdvar = vars[vbdind];
        let vbdcoef = vals[vbdind] / vals[1 - vbdind];

        let inf = set.infinity();
        let (vbdlhs, vbdrhs) = if vals[1 - vbdind] > 0.0 {
            (
                if set.is_infinity(-lhs) { -inf } else { lhs / vals[1 - vbdind] },
                if set.is_infinity(rhs) { inf } else { rhs / vals[1 - vbdind] },
            )
        } else {
            (
                if set.is_infinity(rhs) { -inf } else { rhs / vals[1 - vbdind] },
                if set.is_infinity(-lhs) { inf } else { lhs / vals[1 - vbdind] },
            )
        };

        // Create the bin variable bound constraint (an automatically upgraded
        // constraint is always unmodifiable).
        let name = scip.conss[cons].name.clone();
        let flags = cons_flags(scip, cons);
        *upgdcons = create_cons_varbound(scip, &name, var, vbdvar, vbdcoef, vbdlhs, vbdrhs, &flags)?;
    }

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `consFreeVarbound`: destructor of constraint handler to free constraint
/// handler data.
fn cons_free_varbound(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

// `consInitsolVarbound` only adds nlrows to the NLP when it has been
// constructed, which the port never does (the NLP is not ported), so the
// callback is not set.

/// `consExitsolVarbound`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_varbound(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], _restart: bool) -> ScipResult {
    // Release the rows and nlrows of all constraints.
    for &cons in conss {
        let mut row = cd(scip, cons).row;
        if row != NONE {
            scip_lp::release_row(scip, &mut row)?;
            cdm(scip, cons).row = row;
        }
    }

    Ok(())
}

/// `consDeleteVarbound`: frees specific constraint data.
fn cons_delete_varbound(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    // Drop events.
    if api::is_transformed(scip) {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        let c = consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap();
        let (var, vbdvar) = (c.var, c.vbdvar);
        api::drop_var_event(scip, var, VARBOUND_EVENTS, eventhdlr, cons, -1)?;
        api::drop_var_event(scip, vbdvar, VARBOUND_EVENTS, eventhdlr, cons, -1)?;
    }

    let mut data = consdata.take().unwrap();
    consdata_free(scip, data.downcast_mut::<ConsData>().unwrap())
}

/// `consTransVarbound`: transforms constraint data into data belonging to
/// the transformed problem.
fn cons_trans_varbound(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let s = cd(scip, sourcecons);
    let (var, vbdvar, vbdcoef, lhs, rhs) = (s.var, s.vbdvar, s.vbdcoef, s.lhs, s.rhs);

    // Create target constraint data.
    let targetdata = consdata_create(scip, var, vbdvar, vbdcoef, lhs, rhs)?;

    // Create target constraint.
    let name = scip.conss[sourcecons].name.clone();
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    // Catch events for variables.
    let eventhdlr = hd(scip, conshdlr).eventhdlr;
    catch_events(scip, *targetcons, eventhdlr)
}

/// `consInitlpVarbound`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
fn cons_initlp_varbound(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let mut i = 0;
    while i < conss.len() && !*infeasible {
        *infeasible = add_relaxation(scip, conss[i])?;
        i += 1;
    }

    Ok(())
}

/// The separation loop shared by `consSepalpVarbound` and
/// `consSepasolVarbound`.
fn sepa_varbound(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let usebdwidening = hd(scip, conshdlr).usebdwidening;

    *result = ScipResultCode::DidNotFind;

    // Separate useful constraints.
    for &cons in &conss[..nusefulconss as usize] {
        separate_cons(scip, cons, usebdwidening, sol, result)?;
    }

    // Separate remaining constraints.
    let mut i = nusefulconss as usize;
    while i < conss.len() && *result == ScipResultCode::DidNotFind {
        separate_cons(scip, conss[i], usebdwidening, sol, result)?;
        i += 1;
    }

    Ok(())
}

/// `consSepalpVarbound`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_varbound(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    sepa_varbound(scip, conshdlr, conss, nusefulconss, NONE, result)
}

/// `consSepasolVarbound`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_varbound(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    sepa_varbound(scip, conshdlr, conss, nusefulconss, sol, result)
}

/// The enforcement loop shared by `consEnfolpVarbound` and
/// `consEnforelaxVarbound`.
fn enfo_varbound(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    let usebdwidening = hd(scip, conshdlr).usebdwidening;

    *result = ScipResultCode::Feasible;

    for &cons in conss {
        if !check_cons(scip, cons, sol, false) {
            *result = ScipResultCode::Infeasible;
            api::reset_cons_age(scip, cons);
            separate_cons(scip, cons, usebdwidening, sol, result)?;
            if *result != ScipResultCode::Infeasible {
                break;
            }
        } else {
            // Increase age of constraint.
            api::inc_cons_age(scip, cons)?;
        }
    }

    Ok(())
}

/// `consEnfolpVarbound`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_varbound(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enfo_varbound(scip, conshdlr, conss, NONE, result)
}

/// `consEnforelaxVarbound`: constraint enforcing method of constraint
/// handler for relaxation solutions.
fn cons_enforelax_varbound(
    scip: &mut Scip,
    sol: SolId,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    enfo_varbound(scip, conshdlr, conss, sol, result)
}

/// `consEnfopsVarbound`: constraint enforcing method of constraint handler
/// for pseudo solutions.
fn cons_enfops_varbound(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    for &cons in conss {
        if !check_cons(scip, cons, NONE, true) {
            api::reset_cons_age(scip, cons);
            *result = ScipResultCode::Infeasible;
            return Ok(());
        } else {
            // Increase age of constraint.
            api::inc_cons_age(scip, cons)?;
        }
    }
    *result = ScipResultCode::Feasible;

    Ok(())
}

/// `consCheckVarbound`: feasibility check method of constraint handler for
/// integral solutions.
#[allow(clippy::too_many_arguments)]
fn cons_check_varbound(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    checklprows: bool,
    _printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let mut i = 0;
    while i < conss.len() && (*result == ScipResultCode::Feasible || completely) {
        if !check_cons(scip, conss[i], sol, checklprows) {
            *result = ScipResultCode::Infeasible;
        }
        i += 1;
    }

    Ok(())
}

/// `consPropVarbound`: domain propagation method of constraint handler.
fn cons_prop_varbound(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let usebdwidening = hd(scip, conshdlr).usebdwidening;

    let mut cutoff = false;
    let mut nchgbds = 0;
    let mut nchgsides = 0;

    // Process constraints marked for propagation.
    let mut i = 0;
    while i < nmarkedconss as usize && !cutoff {
        propagate_cons(scip, conss[i], usebdwidening, &mut cutoff, &mut nchgbds, Some(&mut nchgsides), None)?;
        i += 1;
    }

    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if nchgbds > 0 {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consPresolVarbound`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_varbound(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nrounds: i32,
    presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    // Get constraint handler data.
    let (eventhdlr, usebdwidening, presolpairwise) = {
        let h = hd(scip, conshdlr);
        (h.eventhdlr, h.usebdwidening, h.presolpairwise)
    };

    let mut cutoff = false;

    let oldnchgbds = counts.nchgbds;
    let oldndelconss = counts.ndelconss;
    let oldnaddconss = counts.naddconss;
    let oldnchgcoefs = counts.nchgcoefs;
    let oldnchgsides = counts.nchgsides;
    let oldnaggrvars = counts.naggrvars;

    for (i, &cons) in conss.iter().enumerate() {
        if i % 1000 == 0 && crate::solve::solve_is_stopped(scip, false) {
            break;
        }

        // Force presolving the constraint in the initial round.
        if nrounds == 0 {
            cdm(scip, cons).presolved = false;
        }

        if cd(scip, cons).presolved {
            continue;
        }
        cdm(scip, cons).presolved = true;

        // Incorporate fixings and aggregations in constraint.
        apply_fixings(scip, cons, eventhdlr, &mut cutoff, &mut counts.nchgbds, &mut counts.ndelconss, &mut counts.naddconss)?;

        if cutoff {
            break;
        }
        if !scip.conss[cons].is_active() {
            continue;
        }

        // Propagate constraint.
        propagate_cons(
            scip,
            cons,
            usebdwidening,
            &mut cutoff,
            &mut counts.nchgbds,
            Some(&mut counts.nchgsides),
            Some(&mut counts.ndelconss),
        )?;

        if cutoff {
            break;
        }
        if !scip.conss[cons].is_active() {
            continue;
        }

        // Tighten variable bound coefficient.
        tighten_coefs(
            scip,
            cons,
            &mut counts.nchgcoefs,
            &mut counts.nchgsides,
            &mut counts.ndelconss,
            &mut cutoff,
            &mut counts.nchgbds,
        )?;
        if cutoff {
            break;
        }
        if !scip.conss[cons].is_active() {
            continue;
        }

        // Informs once variable x about a globally valid variable lower or upper
        // bound.
        if !cd(scip, cons).varboundsadded {
            let localoldnchgbds = counts.nchgbds;

            // If lhs is finite, we have a variable lower bound: lhs <= x + c*y  =>
            // x >= -c*y + lhs.
            let c = cd(scip, cons);
            let (var, vbdvar, vbdcoef, lhs) = (c.var, c.vbdvar, c.vbdcoef, c.lhs);
            if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(vbdcoef.abs()) {
                let (_infeasible, nlocalchgbds) = scip_var::add_var_vlb(scip, var, vbdvar, -vbdcoef, lhs)?;
                counts.nchgbds += nlocalchgbds;
            }

            // If rhs is finite, we have a variable upper bound: x + c*y <= rhs  =>
            // x <= -c*y + rhs.
            let c = cd(scip, cons);
            let (var, vbdvar, vbdcoef, rhs) = (c.var, c.vbdvar, c.vbdcoef, c.rhs);
            if !scip.set.is_infinity(rhs) && !scip.set.is_infinity(vbdcoef.abs()) {
                let (_infeasible, nlocalchgbds) = scip_var::add_var_vub(scip, var, vbdvar, -vbdcoef, rhs)?;
                counts.nchgbds += nlocalchgbds;
            }
            cdm(scip, cons).varboundsadded = true;

            if counts.nchgbds > localoldnchgbds {
                // Tighten variable bound coefficient.
                tighten_coefs(
                    scip,
                    cons,
                    &mut counts.nchgcoefs,
                    &mut counts.nchgsides,
                    &mut counts.ndelconss,
                    &mut cutoff,
                    &mut counts.nchgbds,
                )?;
                if cutoff {
                    break;
                }
            }
        }
    }

    if !cutoff {
        // For varbound constraint with two integer variables make coefficients
        // integral.
        prettify_conss(scip, conss, &mut counts.nchgcoefs, &mut counts.nchgsides);

        // Check if we can upgrade to a set-packing constraint.
        upgrade_conss(
            scip,
            conshdlr,
            conss,
            &mut cutoff,
            &mut counts.naggrvars,
            &mut counts.nchgbds,
            &mut counts.nchgcoefs,
            &mut counts.nchgsides,
            &mut counts.ndelconss,
            &mut counts.naddconss,
        )?;

        if !cutoff && presolpairwise && (presoltiming & PRESOLTIMING_MEDIUM) != 0 {
            // Preprocess pairs of variable bound constraints.
            preprocess_constraint_pairs(
                scip,
                conss,
                &mut cutoff,
                &mut counts.nchgbds,
                &mut counts.ndelconss,
                &mut counts.nchgcoefs,
                &mut counts.nchgsides,
            )?;
        }
    }

    // Return the correct result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if counts.nchgbds > oldnchgbds
        || counts.ndelconss > oldndelconss
        || counts.naddconss > oldnaddconss
        || counts.nchgcoefs > oldnchgcoefs
        || counts.nchgsides > oldnchgsides
        || counts.naggrvars > oldnaggrvars
    {
        ScipResultCode::Success
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consRespropVarbound`: propagation conflict resolving method of
/// constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_varbound(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    infervar: VarId,
    inferinfo: i32,
    boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    let usebdwidening = hd(scip, conshdlr).usebdwidening;

    resolve_propagation(scip, cons, infervar, Proprule::from_i32(inferinfo), boundtype, bdchgidx.as_ref(), relaxedbd, usebdwidening)?;

    *result = ScipResultCode::Success;

    Ok(())
}

/// `consLockVarbound`: variable rounding lock method of constraint handler.
fn cons_lock_varbound(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let c = cd(scip, cons);
    let (var, vbdvar, vbdcoef, lhs, rhs) = (c.var, c.vbdvar, c.vbdcoef, c.lhs, c.rhs);

    if !scip.set.is_infinity(-lhs) {
        api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
        if vbdcoef > 0.0 {
            api::add_var_locks_type(scip, vbdvar, locktype, nlockspos, nlocksneg)?;
        } else {
            api::add_var_locks_type(scip, vbdvar, locktype, nlocksneg, nlockspos)?;
        }
    }

    if !scip.set.is_infinity(rhs) {
        api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
        if vbdcoef > 0.0 {
            api::add_var_locks_type(scip, vbdvar, locktype, nlocksneg, nlockspos)?;
        } else {
            api::add_var_locks_type(scip, vbdvar, locktype, nlockspos, nlocksneg)?;
        }
    }

    Ok(())
}

// `consActiveVarbound` and `consDeactiveVarbound` only add and remove the
// nlrow (the NLP is not ported), so the callbacks are not set.

/// `consGetVarsVarbound`: constraint method of constraint handler which
/// returns the variables (the vector always fits here).
fn cons_get_vars_varbound(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.push(c.var);
    vars.push(c.vbdvar);
    *success = true;
    Ok(())
}

/// `consGetNVarsVarbound`: constraint method of constraint handler which
/// returns the number of variables.
fn cons_get_n_vars_varbound(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = 2;
    *success = true;
    Ok(())
}

/*
 * Event Handler
 */

/// `eventExecVarbound`: execution method of bound change event handler; the
/// event data is the constraint.
fn event_exec_varbound(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let cons: ConsId = eventdata;

    if event.eventtype == EVENTTYPE_VARFIXED {
        cdm(scip, cons).presolved = false;
    } else {
        let c = cdm(scip, cons);
        c.presolved = false;
        c.tightened = false;

        cons_mark_propagate(scip, cons);
    }

    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrVarbound`: creates the handler for variable bound
/// constraints and includes it in SCIP. Sub-SCIP copying, parsing, printing
/// and the symmetry graphs are added with those modules.
pub fn include_conshdlr_varbound(scip: &mut Scip) -> ScipResult {
    // Include event handler for bound change events.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_varbound, None)?;

    // Create variable bound constraint handler data.
    let conshdlrdata = ConshdlrData {
        eventhdlr,
        presolpairwise: DEFAULT_PRESOLPAIRWISE,
        maxlpcoef: DEFAULT_MAXLPCOEF,
        usebdwidening: DEFAULT_USEBDWIDENING,
    };

    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_varbound),
        Some(cons_enfops_varbound),
        Some(cons_check_varbound),
        Some(cons_lock_varbound),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_varbound);
    h.conscopy = Some(cons_copy_varbound);
    h.consdelete = Some(cons_delete_varbound);
    h.consexitsol = Some(cons_exitsol_varbound);
    h.consfree = Some(cons_free_varbound);
    h.consgetvars = Some(cons_get_vars_varbound);
    h.consgetnvars = Some(cons_get_n_vars_varbound);
    h.consinitlp = Some(cons_initlp_varbound);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_varbound), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_varbound), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_varbound);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_varbound),
        Some(cons_sepasol_varbound),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_varbound);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_varbound);

    if crate::set::set_find_conshdlr(scip, "linear") != NONE {
        // Include the linear constraint to varbound constraint upgrade in the
        // linear constraint handler.
        crate::cons_linear::include_lincons_upgrade(scip, lincons_upgd_varbound, LINCONSUPGD_PRIORITY, CONSHDLR_NAME)?;
    }

    // Add varbound constraint handler parameters.
    crate::paramset::paramset_add_bool(
        scip,
        &format!("constraints/{CONSHDLR_NAME}/presolpairwise"),
        "should pairwise constraint comparison be performed in presolving?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, presolpairwise),
        true,
        DEFAULT_PRESOLPAIRWISE,
        None,
    )?;
    crate::paramset::paramset_add_real(
        scip,
        &format!("constraints/{CONSHDLR_NAME}/maxlpcoef"),
        "maximum coefficient in varbound constraint to be added as a row into LP",
        crate::data_target!(f64, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, maxlpcoef),
        true,
        DEFAULT_MAXLPCOEF,
        0.0,
        1e+20,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        &format!("constraints/{CONSHDLR_NAME}/usebdwidening"),
        "should bound widening be used in conflict analysis?",
        crate::data_target!(bool, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, usebdwidening),
        false,
        DEFAULT_USEBDWIDENING,
        None,
    )?;

    Ok(())
}

/// `SCIPcreateConsVarbound`: creates and captures a variable bound
/// constraint: lhs <= x + c*y <= rhs.
#[allow(clippy::too_many_arguments)]
pub fn create_cons_varbound(scip: &mut Scip, name: &str, var: VarId, vbdvar: VarId, vbdcoef: f64, lhs: f64, rhs: f64, flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the variable bound constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Variable bound constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Create constraint data.
    let consdata = consdata_create(scip, var, vbdvar, vbdcoef, lhs, rhs)?;

    // Create constraint.
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)?;

    if api::is_transformed(scip) {
        // Catch events for variables.
        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        catch_events(scip, cons, eventhdlr)?;
    }

    Ok(cons)
}

/// `SCIPcreateConsBasicVarbound`: creates and captures a variable bound
/// constraint with all constraint flags set to their default values.
pub fn create_cons_basic_varbound(scip: &mut Scip, name: &str, var: VarId, vbdvar: VarId, vbdcoef: f64, lhs: f64, rhs: f64) -> ScipResult<ConsId> {
    create_cons_varbound(scip, name, var, vbdvar, vbdcoef, lhs, rhs, &ConsFlags::default())
}

/// `SCIPgetLhsVarbound`: gets left hand side of variable bound constraint.
pub fn get_lhs_varbound(scip: &Scip, cons: ConsId) -> f64 {
    cd(scip, cons).lhs
}

/// `SCIPgetRhsVarbound`: gets right hand side of variable bound constraint.
pub fn get_rhs_varbound(scip: &Scip, cons: ConsId) -> f64 {
    cd(scip, cons).rhs
}

/// `SCIPgetVarVarbound`: gets bounded variable x of variable bound
/// constraint.
pub fn get_var_varbound(scip: &Scip, cons: ConsId) -> VarId {
    cd(scip, cons).var
}

/// `SCIPgetVbdvarVarbound`: gets bounding variable y of variable bound
/// constraint.
pub fn get_vbdvar_varbound(scip: &Scip, cons: ConsId) -> VarId {
    cd(scip, cons).vbdvar
}

/// `SCIPgetVbdcoefVarbound`: gets bound coefficient c of variable bound
/// constraint.
pub fn get_vbdcoef_varbound(scip: &Scip, cons: ConsId) -> f64 {
    cd(scip, cons).vbdcoef
}

/// `SCIPgetDualsolVarbound`: gets the dual solution of the variable bound
/// constraint in the current LP.
pub fn get_dualsol_varbound(scip: &Scip, cons: ConsId) -> f64 {
    let row = cd(scip, cons).row;
    if row != NONE && scip.rows[row].lppos >= 0 { scip.rows[row].dualsol } else { 0.0 }
}

/// `SCIPgetDualfarkasVarbound`: gets the dual Farkas value of the variable
/// bound constraint in the current infeasible LP.
pub fn get_dualfarkas_varbound(scip: &Scip, cons: ConsId) -> f64 {
    let row = cd(scip, cons).row;
    if row != NONE && scip.rows[row].lppos >= 0 { scip.rows[row].dualfarkas } else { 0.0 }
}

/// `SCIPgetRowVarbound`: returns the linear relaxation of the given variable
/// bound constraint; may return `NONE` if no LP row was yet created.
pub fn get_row_varbound(scip: &Scip, cons: ConsId) -> RowId {
    cd(scip, cons).row
}

/// `SCIPcreateRowVarbound`: creates and returns the row of the given
/// varbound constraint.
pub fn create_row_varbound(scip: &mut Scip, cons: ConsId) -> ScipResult {
    create_relaxation(scip, cons)
}

/// `SCIPcleanupConssVarbound`: cleans up (multi-)aggregations and fixings
/// from varbound constraints; returns whether infeasibility was detected.
pub fn cleanup_conss_varbound(scip: &mut Scip, onlychecked: bool, naddconss: &mut i32, ndelconss: &mut i32, nchgbds: &mut i32) -> ScipResult<bool> {
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        return Ok(false);
    }

    let mut infeasible = false;
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let h = &scip.conshdlrs[conshdlr];
    let conss: Vec<ConsId> = if onlychecked {
        h.checkconss.0[..h.ncheckconss as usize].to_vec()
    } else {
        h.conss.0[..h.nactiveconss as usize].to_vec()
    };

    // Loop backwards since then deleted constraints do not interfere with the
    // loop.
    for &cons in conss.iter().rev() {
        apply_fixings(scip, cons, eventhdlr, &mut infeasible, nchgbds, ndelconss, naddconss)?;

        if infeasible {
            break;
        }
    }

    Ok(infeasible)
}

/// `conshdlrCopyVarbound`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_varbound(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_varbound(scip)?;
    *valid = true;
    Ok(())
}

/// `consCopyVarbound`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_varbound(
    scip: &mut Scip,
    cons: &mut ConsId,
    name: Option<&str>,
    sourcescip: &mut Scip,
    _sourceconshdlr: ConshdlrId,
    sourcecons: ConsId,
    maps: &mut crate::copy::CopyMaps,
    flags: &ConsFlags,
    global: bool,
    valid: &mut bool,
) -> ScipResult {
    let vars = [get_var_varbound(sourcescip, sourcecons), get_vbdvar_varbound(sourcescip, sourcecons)];
    let coefs = [1.0, get_vbdcoef_varbound(sourcescip, sourcecons)];
    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);

    // Copy the varbound using the linear constraint copy method.
    let (lhs, rhs) = (get_lhs_varbound(sourcescip, sourcecons), get_rhs_varbound(sourcescip, sourcecons));
    (*cons, *valid) = crate::cons_linear::copy_cons_linear(scip, sourcescip, &consname, &vars, Some(&coefs), lhs, rhs, maps, flags, global)?;
    Ok(())
}
