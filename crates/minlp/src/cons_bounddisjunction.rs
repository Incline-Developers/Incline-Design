//! Port of `cons_bounddisjunction.c`: constraint handler for bound
//! disjunction constraints `(x1 {<=,>=} b1) or ... or (xn {<=,>=} bn)`.
//!
//! Not ported: printing, parsing, the symmetry graphs and sub-SCIP copying.

use std::any::Any;

use crate::{
    api,
    cons::{ConsFlags, *},
    cvec::CVec,
    event::*,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_var,
    scip_var::{compute_var_lb_global, compute_var_lb_local, compute_var_ub_global, compute_var_ub_local},
    types::{BoundType, ConsId, ConshdlrId, EventhdlrId, LockType, ScipResultCode, SolId, VarId, VarStatus},
    var::{BdChgIdx, NONE, var_compare},
};

/* constraint handler properties */
pub const CONSHDLR_NAME: &str = "bounddisjunction";
const CONSHDLR_DESC: &str = "bound disjunction constraints";
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -3000000;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -3000000;
/// Frequency for propagating domains; zero means only preprocessing
/// propagation.
const CONSHDLR_PROPFREQ: i32 = 1;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0
/// for first only.
const CONSHDLR_EAGERFREQ: i32 = 100;
/// Maximal number of presolving rounds the constraint handler participates
/// in (-1: no limit).
const CONSHDLR_MAXPREROUNDS: i32 = -1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const CONSHDLR_DELAYPROP: bool = false;
/// Should the constraint handler be skipped, if no constraints are
/// available?
const CONSHDLR_NEEDSCONS: bool = true;

const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST;
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

const EVENTHDLR_NAME: &str = "bounddisjunction";
const EVENTHDLR_DESC: &str = "event handler for bound disjunction constraints";

const CONFLICTHDLR_NAME: &str = "bounddisjunction";
const CONFLICTHDLR_DESC: &str = "conflict handler creating bound disjunction constraints";
const CONFLICTHDLR_PRIORITY: i32 = -3000000;

/// Maximal percantage of continuous variables within a conflict.
const DEFAULT_CONTINUOUSFRAC: f64 = 0.4;

/// `AGEINCREASE`.
#[inline]
fn ageincrease(n: f64) -> f64 {
    1.0 + 0.2 * n
}

/* Comparison for two values (the integral shortcuts are disabled in the
 * reference). */
#[inline]
fn is_feas_lt(scip: &Scip, val1: f64, val2: f64) -> bool {
    scip.set.is_feas_lt(val1, val2)
}
#[inline]
fn is_feas_le(scip: &Scip, val1: f64, val2: f64) -> bool {
    scip.set.is_feas_le(val1, val2)
}
#[inline]
fn is_feas_gt(scip: &Scip, val1: f64, val2: f64) -> bool {
    scip.set.is_feas_gt(val1, val2)
}
#[inline]
fn is_feas_ge(scip: &Scip, val1: f64, val2: f64) -> bool {
    scip.set.is_feas_ge(val1, val2)
}

/*
 * Data structures
 */

/// `SCIP_ConshdlrData`: constraint handler data.
pub struct ConshdlrData {
    /// Event handler for events on watched variables.
    pub eventhdlr: EventhdlrId,
}

/// `SCIP_ConsData`: bound disjunction constraint data.
#[derive(Default)]
pub struct ConsData {
    /// Variables of the literals in the constraint.
    pub vars: CVec<VarId>,
    /// Types of bounds of the literals (lower or upper bounds).
    pub boundtypes: CVec<BoundType>,
    /// Bounds of the literals.
    pub bounds: CVec<f64>,
    /// Size of vars, boundtypes, and bounds arrays.
    pub varssize: i32,
    /// Number of variables in the constraint.
    pub nvars: i32,
    /// Position of the first watched variable.
    pub watchedvar1: i32,
    /// Position of the second watched variable.
    pub watchedvar2: i32,
    /// Event filter position of first watched variable.
    pub filterpos1: i32,
    /// Event filter position of second watched variable.
    pub filterpos2: i32,
}

/// The constraint data of a bound disjunction constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a bound disjunction constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the bound disjunction constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/*
 * Local methods
 */

/// `lockRounding`: adds rounding locks for the given variable in the given
/// bound disjunction constraint.
fn lock_rounding(scip: &mut Scip, cons: ConsId, pos: i32) -> ScipResult {
    let (var, bt) = (cd(scip, cons).vars[pos], cd(scip, cons).boundtypes[pos]);
    if bt == BoundType::Lower {
        api::lock_var_cons(scip, var, cons, true, false)
    } else {
        api::lock_var_cons(scip, var, cons, false, true)
    }
}

/// `unlockRounding`: removes rounding locks for the given variable in the
/// given bound disjunction constraint.
fn unlock_rounding(scip: &mut Scip, cons: ConsId, pos: i32) -> ScipResult {
    let (var, bt) = (cd(scip, cons).vars[pos], cd(scip, cons).boundtypes[pos]);
    if bt == BoundType::Lower {
        api::unlock_var_cons(scip, var, cons, true, false)
    } else {
        api::unlock_var_cons(scip, var, cons, false, true)
    }
}

/// The events a literal's variable is watched for.
fn literal_events(bt: BoundType) -> EventType {
    if bt == BoundType::Lower {
        EVENTTYPE_UBTIGHTENED | EVENTTYPE_LBRELAXED
    } else {
        EVENTTYPE_LBTIGHTENED | EVENTTYPE_UBRELAXED
    }
}

/// `catchEvents`: catches the events on a single variable of the bound
/// disjunction constraint; the event data is the constraint. Returns the
/// filter position.
fn catch_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult<i32> {
    let (var, bt) = (cd(scip, cons).vars[pos], cd(scip, cons).boundtypes[pos]);
    let mut filterpos = -1;
    api::catch_var_event(scip, var, literal_events(bt), eventhdlr, cons, Some(&mut filterpos))?;
    Ok(filterpos)
}

/// `dropEvents`: drops the events on a single variable of the bound
/// disjunction constraint.
fn drop_events(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32, filterpos: i32) -> ScipResult {
    let (var, bt) = (cd(scip, cons).vars[pos], cd(scip, cons).boundtypes[pos]);
    api::drop_var_event(scip, var, literal_events(bt), eventhdlr, cons, filterpos)
}

/// The constraint data with the watched variables and filter positions
/// unset, from the given literals.
fn consdata_from(vars: Vec<VarId>, boundtypes: Vec<BoundType>, bounds: Vec<f64>) -> ConsData {
    let n = vars.len() as i32;
    ConsData {
        vars: CVec(vars),
        boundtypes: CVec(boundtypes),
        bounds: CVec(bounds),
        varssize: n,
        nvars: n,
        watchedvar1: -1,
        watchedvar2: -1,
        filterpos1: -1,
        filterpos2: -1,
    }
}

/// `consdataCreate`: creates a bound disjunction constraint data object.
fn consdata_create(scip: &mut Scip, vars: &[VarId], boundtypes: &[BoundType], bounds: &[f64]) -> ScipResult<ConsData> {
    let nvars = vars.len();
    let mut c = if nvars > 0 {
        if api::is_cons_compression_enabled(scip) {
            let mut varsbuffer = Vec::with_capacity(nvars);
            let mut boundtypesbuffer = Vec::with_capacity(nvars);
            let mut boundsbuffer = Vec::with_capacity(nvars);
            let mut redundant = false;

            // Loop over variables, compare fixed ones against its bound
            // disjunction.
            let mut v = 0;
            while v < nvars && !redundant {
                let var = vars[v];
                let boundtype = boundtypes[v];
                let bound = bounds[v];
                let x = &scip.vars[var];

                // Is the variable fixed?
                if scip.set.is_eq(x.glbdom.lb, x.glbdom.ub) {
                    if (boundtype == BoundType::Lower && is_feas_ge(scip, x.locdom.lb, bound)) || (boundtype == BoundType::Upper && is_feas_le(scip, x.locdom.ub, bound)) {
                        // Save this feasible assignment at the first position.
                        varsbuffer.clear();
                        boundtypesbuffer.clear();
                        boundsbuffer.clear();
                        varsbuffer.push(var);
                        boundtypesbuffer.push(boundtype);
                        boundsbuffer.push(bound);
                        redundant = true;
                    }
                } else {
                    // Append unfixed variable to buffer.
                    varsbuffer.push(var);
                    boundtypesbuffer.push(boundtype);
                    boundsbuffer.push(bound);
                }
                v += 1;
            }

            // Duplicate a single, infeasible assignment, wlog the first one.
            if varsbuffer.is_empty() {
                consdata_from(vars[..1].to_vec(), boundtypes[..1].to_vec(), bounds[..1].to_vec())
            } else {
                // If the bound disjunction is already trivially satisfied, we keep
                // only a single feasible assignment; we only copy the buffered
                // variables required to represent the constraint.
                consdata_from(varsbuffer, boundtypesbuffer, boundsbuffer)
            }
        } else {
            // Without problem compression, the entire vars array is copied.
            consdata_from(vars.to_vec(), boundtypes.to_vec(), bounds.to_vec())
        }
    } else {
        consdata_from(Vec::new(), Vec::new(), Vec::new())
    };

    // Get transformed variables, if we are in the transformed problem.
    if api::is_transformed(scip) {
        let n = c.nvars as usize;
        api::get_transformed_vars(scip, &mut c.vars.0[..n])?;
    }

    Ok(c)
}

/// `consdataCreateRedundant`: creates a bound disjunction constraint data
/// object with possibly redundant literals.
fn consdata_create_redundant(scip: &mut Scip, vars: &[VarId], boundtypes: &[BoundType], bounds: &[f64]) -> ScipResult<ConsData> {
    let nvars = vars.len();
    let mut c = if nvars > 0 {
        let mut varsbuffer = vars.to_vec();
        let mut boundtypesbuffer: Vec<i32> = boundtypes.iter().map(|&b| b as i32).collect();
        let mut boundsbuffer = bounds.to_vec();
        let mut nvarsbuffer = 0usize;
        let mut nviolated = 0;
        let mut v = 0;

        // Sort variables according to index; this allows us to check for
        // redundancy easily below because duplicate variables must now appear
        // consecutively.
        crate::sort::sort(&mut varsbuffer[..], (&mut boundsbuffer[..], &mut boundtypesbuffer[..]), |&a, &b| var_compare(scip, a, b));
        let bt = |b: i32| if b == BoundType::Lower as i32 { BoundType::Lower } else { BoundType::Upper };

        // Filter out redundant literals.
        while v < nvars {
            let var = varsbuffer[v];

            // Compress fixed variables.
            let x = &scip.vars[var];
            if api::is_cons_compression_enabled(scip) && scip.set.is_eq(x.glbdom.lb, x.glbdom.ub) {
                // If the literal is feasible for the fixed variable, then the whole
                // constraint is feasible, so we reduce it to only this literal.
                if (bt(boundtypesbuffer[v]) == BoundType::Lower && is_feas_ge(scip, x.locdom.lb, boundsbuffer[v]))
                    || (bt(boundtypesbuffer[v]) == BoundType::Upper && is_feas_le(scip, x.locdom.ub, boundsbuffer[v]))
                {
                    // Save this feasible assignment at the first position.
                    varsbuffer[0] = var;
                    boundtypesbuffer[0] = boundtypesbuffer[v];
                    boundsbuffer[0] = boundsbuffer[v];
                    nvarsbuffer = 1;
                    break;
                } else {
                    // Otherwise the literal is violated - we skip the literal.
                    nviolated += 1;
                    v += 1;
                    continue;
                }
            }

            // Initialize bound indices.
            let (mut lowerindex, mut upperindex): (i64, i64) = if bt(boundtypesbuffer[v]) == BoundType::Lower { (v as i64, -1) } else { (-1, v as i64) };
            v += 1;

            // Check subsequent variables with the same variable for redundancy.
            while v < nvars && varsbuffer[v] == var {
                if bt(boundtypesbuffer[v]) == BoundType::Lower {
                    // Keep the weaker lower bound.
                    if lowerindex == -1 || boundsbuffer[lowerindex as usize] > boundsbuffer[v] {
                        lowerindex = v as i64;
                    }
                } else {
                    // Keep the weaker upper bound.
                    if upperindex == -1 || boundsbuffer[upperindex as usize] < boundsbuffer[v] {
                        upperindex = v as i64;
                    }
                }
                v += 1;
            }

            // Keep bound sequence.
            if upperindex != -1 && lowerindex > upperindex {
                std::mem::swap(&mut lowerindex, &mut upperindex);
            }

            // Keep first bound.
            if lowerindex != -1 {
                let li = lowerindex as usize;
                varsbuffer[nvarsbuffer] = varsbuffer[li];
                boundtypesbuffer[nvarsbuffer] = boundtypesbuffer[li];
                boundsbuffer[nvarsbuffer] = boundsbuffer[li];
                nvarsbuffer += 1;
            }

            // Keep second bound.
            if upperindex != -1 {
                let ui = upperindex as usize;
                varsbuffer[nvarsbuffer] = varsbuffer[ui];
                boundtypesbuffer[nvarsbuffer] = boundtypesbuffer[ui];
                boundsbuffer[nvarsbuffer] = boundsbuffer[ui];
                nvarsbuffer += 1;
            }
        }

        // If all literals are infeasible, we keep the first.
        if api::is_cons_compression_enabled(scip) && nviolated > 0 && nvarsbuffer == 0 {
            nvarsbuffer = 1;
        }

        varsbuffer.truncate(nvarsbuffer);
        boundsbuffer.truncate(nvarsbuffer);
        let types = boundtypesbuffer[..nvarsbuffer].iter().map(|&b| bt(b)).collect();
        consdata_from(varsbuffer, types, boundsbuffer)
    } else {
        consdata_from(Vec::new(), Vec::new(), Vec::new())
    };

    // Get transformed variables, if we are in the transformed problem.
    if api::is_transformed(scip) {
        let n = c.nvars as usize;
        api::get_transformed_vars(scip, &mut c.vars.0[..n])?;
    }

    Ok(c)
}

/// `switchWatchedvars`: stores the given variable numbers as watched
/// variables, and updates the event processing.
fn switch_watchedvars(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, watchedvar1: i32, watchedvar2: i32) -> ScipResult {
    // Don't watch variables for non active constraints.
    if !scip.conss[cons].is_active() {
        return Ok(());
    }

    // If one watched variable is equal to the old other watched variable,
    // just switch positions.
    let c = cdm(scip, cons);
    if watchedvar1 == c.watchedvar2 || watchedvar2 == c.watchedvar1 {
        std::mem::swap(&mut c.watchedvar1, &mut c.watchedvar2);
        std::mem::swap(&mut c.filterpos1, &mut c.filterpos2);
    }

    // Drop events on old watched variables.
    let (w1, f1) = (cd(scip, cons).watchedvar1, cd(scip, cons).filterpos1);
    if w1 != -1 && w1 != watchedvar1 {
        drop_events(scip, cons, eventhdlr, w1, f1)?;
        cdm(scip, cons).watchedvar1 = -1;
    }
    let (w2, f2) = (cd(scip, cons).watchedvar2, cd(scip, cons).filterpos2);
    if w2 != -1 && w2 != watchedvar2 {
        drop_events(scip, cons, eventhdlr, w2, f2)?;
        cdm(scip, cons).watchedvar2 = -1;
    }

    // Catch events on new watched variables.
    if watchedvar1 != -1 && watchedvar1 != cd(scip, cons).watchedvar1 {
        let f = catch_events(scip, cons, eventhdlr, watchedvar1)?;
        cdm(scip, cons).filterpos1 = f;
    }
    if watchedvar2 != -1 && watchedvar2 != cd(scip, cons).watchedvar2 {
        let f = catch_events(scip, cons, eventhdlr, watchedvar2)?;
        cdm(scip, cons).filterpos2 = f;
    }

    // Set the new watched variables.
    let c = cdm(scip, cons);
    c.watchedvar1 = watchedvar1;
    c.watchedvar2 = watchedvar2;

    Ok(())
}

/// `isOverlapping`: check whether two intervals overlap.
fn is_overlapping(scip: &Scip, var: VarId, boundtype1: BoundType, bound1: f64, _boundtype2: BoundType, bound2: f64) -> bool {
    let integral = crate::var::var_is_integral(scip, var) as i32 as f64;
    if boundtype1 == BoundType::Lower {
        scip.set.is_le(bound1 - bound2, integral)
    } else {
        scip.set.is_le(bound2 - bound1, integral)
    }
}

/// `delCoefPos`: deletes coefficient at given position from bound
/// disjunction constraint data.
fn del_coef_pos(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, pos: i32) -> ScipResult {
    // Remove the rounding locks of variable.
    unlock_rounding(scip, cons, pos)?;

    if api::cons_is_transformed(scip, cons) {
        // If the position is watched, stop watching the position.
        if cd(scip, cons).watchedvar1 == pos {
            let w2 = cd(scip, cons).watchedvar2;
            switch_watchedvars(scip, cons, eventhdlr, w2, -1)?;
        }
        if cd(scip, cons).watchedvar2 == pos {
            let w1 = cd(scip, cons).watchedvar1;
            switch_watchedvars(scip, cons, eventhdlr, w1, -1)?;
        }
    }

    // Move the last variable to the free slot.
    let c = cdm(scip, cons);
    let last = c.nvars - 1;
    c.vars[pos] = c.vars[last];
    c.boundtypes[pos] = c.boundtypes[last];
    c.bounds[pos] = c.bounds[last];
    c.nvars -= 1;

    // If the last variable (that moved) was watched, update the watched
    // position.
    if c.watchedvar1 == c.nvars {
        c.watchedvar1 = pos;
    }
    if c.watchedvar2 == c.nvars {
        c.watchedvar2 = pos;
    }

    cons_enable_propagation(scip, cons);

    Ok(())
}

/// `addCoef`: adds literal to bound disjunction constraint data.
fn add_coef(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, var: VarId, boundtype: BoundType, bound: f64, redundant: &mut bool) -> ScipResult {
    // Ensure enough memory in consdata arrays.
    if cd(scip, cons).varssize == cd(scip, cons).nvars {
        let newsize = scip.set.calc_mem_grow_size(cd(scip, cons).nvars + 1);
        let c = cdm(scip, cons);
        c.vars.0.resize(newsize as usize, NONE);
        c.boundtypes.0.resize(newsize as usize, BoundType::Lower);
        c.bounds.0.resize(newsize as usize, 0.0);
        c.varssize = newsize;
    }

    // Remember the position of the literal in the constraint that has the
    // same bound type on the same variable
    //
    // example: (x >= 5) or (x <= 2) and literal (x >= 2) should be added. if
    // we see (x >= 5) first, we cannot stop immediately because only in
    // combination with the second literal we see that the constraint is
    // redundant.
    let mut samebndidx: i32 = -1;
    for v in 0..cd(scip, cons).nvars {
        let c = cd(scip, cons);

        // Check if the variable is already part of the constraint.
        if c.vars[v] == var {
            if c.boundtypes[v] == boundtype {
                samebndidx = v;
            } else if is_overlapping(scip, var, c.boundtypes[v], c.bounds[v], boundtype, bound) {
                *redundant = true;
                return Ok(());
            }
        }
    }

    // The combination of variable and boundtype is already part of the
    // constraint; check whether the clause can be relaxed.
    if samebndidx > -1 {
        let old = cd(scip, cons).bounds[samebndidx];
        if (boundtype == BoundType::Lower && scip.set.is_lt(bound, old)) || (boundtype == BoundType::Upper && scip.set.is_gt(bound, old)) {
            cdm(scip, cons).bounds[samebndidx] = bound;
        }
    } else {
        // Add the variable to the end of the array.
        let c = cdm(scip, cons);
        let n = c.nvars;
        c.vars[n] = var;
        c.boundtypes[n] = boundtype;
        c.bounds[n] = bound;
        c.nvars += 1;

        if api::cons_is_transformed(scip, cons) {
            // Add rounding lock of variable.
            let last = cd(scip, cons).nvars - 1;
            lock_rounding(scip, cons, last)?;

            // If less than 2 variables are watched, add the new one to the watched
            // variables.
            let (w1, w2) = (cd(scip, cons).watchedvar1, cd(scip, cons).watchedvar2);
            if w1 == -1 {
                switch_watchedvars(scip, cons, eventhdlr, last, -1)?;
            } else if w2 == -1 {
                switch_watchedvars(scip, cons, eventhdlr, w1, last)?;
            }
        }
    }

    cons_enable_propagation(scip, cons);

    Ok(())
}

/// `applyGlobalBounds`: deletes all variables with global bounds violating
/// the literal, checks for global bounds satisfying the literal; returns
/// whether the constraint is redundant.
fn apply_global_bounds(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId) -> ScipResult<bool> {
    let mut v = 0;
    while v < cd(scip, cons).nvars {
        let c = cd(scip, cons);
        let (var, bound) = (c.vars[v], c.bounds[v]);

        if c.boundtypes[v] == BoundType::Lower {
            let bnd = compute_var_lb_global(scip, var);
            if is_feas_ge(scip, bnd, bound) {
                return Ok(true);
            } else {
                let bnd = compute_var_ub_global(scip, var);
                if is_feas_lt(scip, bnd, bound) {
                    del_coef_pos(scip, cons, eventhdlr, v)?;
                } else {
                    v += 1;
                }
            }
        } else {
            let bnd = compute_var_ub_global(scip, var);
            if is_feas_le(scip, bnd, bound) {
                return Ok(true);
            } else {
                let bnd = compute_var_lb_global(scip, var);
                if is_feas_gt(scip, bnd, bound) {
                    del_coef_pos(scip, cons, eventhdlr, v)?;
                } else {
                    v += 1;
                }
            }
        }
    }

    Ok(false)
}

/// `isLiteralSatisfied`: returns whether literal at the given position is
/// satisfied in the local bounds.
fn is_literal_satisfied(scip: &Scip, cons: ConsId, pos: i32) -> bool {
    let c = cd(scip, cons);
    if c.boundtypes[pos] == BoundType::Lower {
        let bnd = compute_var_lb_local(scip, c.vars[pos]);
        is_feas_ge(scip, bnd, c.bounds[pos])
    } else {
        let bnd = compute_var_ub_local(scip, c.vars[pos]);
        is_feas_le(scip, bnd, c.bounds[pos])
    }
}

/// `isLiteralViolated`: returns whether literal at the given position is
/// violated in the local bounds.
fn is_literal_violated(scip: &Scip, cons: ConsId, pos: i32) -> bool {
    let c = cd(scip, cons);
    if c.boundtypes[pos] == BoundType::Lower {
        let bnd = compute_var_ub_local(scip, c.vars[pos]);
        is_feas_lt(scip, bnd, c.bounds[pos])
    } else {
        let bnd = compute_var_lb_local(scip, c.vars[pos]);
        is_feas_gt(scip, bnd, c.bounds[pos])
    }
}

/// `removeFixedVariables`: replace variables by their representative active
/// (or multi-aggregated) variables; sets `redundant` if the constraint has
/// been found redundant (C keeps going after `addCoef` sets it).
fn remove_fixed_variables(scip: &mut Scip, cons: ConsId, eventhdlr: EventhdlrId, redundant: &mut bool) -> ScipResult {
    let mut v = 0;
    while v < cd(scip, cons).nvars {
        let mut var = cd(scip, cons).vars[v];

        if crate::var::var_is_active(scip, var) || scip.vars[var].varstatus == VarStatus::MultAggr {
            // Check whether the literal is satisfied and the constraint is thus
            // redundant.
            if is_literal_satisfied(scip, cons, v) {
                *redundant = true;
                break;
            }
            if is_literal_violated(scip, cons, v) {
                del_coef_pos(scip, cons, eventhdlr, v)?;
                continue;
            }

            v += 1;
            continue;
        }

        // Get active/fixed/multiaggr equivalent of v'th literal.
        let mut bound = cd(scip, cons).bounds[v];
        let mut boundtype = cd(scip, cons).boundtypes[v];
        crate::var::var_get_probvar_bound(scip, &mut var, &mut bound, &mut boundtype)?;

        // If literal is satisfied, then constraint is redundant and we can stop.
        let x = &scip.vars[var];
        if (boundtype == BoundType::Lower && is_feas_le(scip, bound, x.glbdom.lb)) || (boundtype == BoundType::Upper && is_feas_ge(scip, bound, x.glbdom.ub)) {
            *redundant = true;
            break;
        }

        // If literal is not fixed, replace it.
        if scip.vars[var].varstatus != VarStatus::Fixed {
            // Add new literal.
            add_coef(scip, cons, eventhdlr, var, boundtype, bound, redundant)?;
        }

        // Remove old literal.
        del_coef_pos(scip, cons, eventhdlr, v)?;
    }

    Ok(())
}

/// `upgradeCons`: try to upgrade the bounddisjunction constraint to a set
/// packing or logic or constraint, if all literals are on binary variables.
fn upgrade_cons(scip: &mut Scip, cons: ConsId, ndelconss: &mut i32, naddconss: &mut i32) -> ScipResult {
    let nvars = cd(scip, cons).nvars;
    let mut allbinary = true;
    let mut newvars = vec![NONE; nvars as usize];

    let mut v = nvars - 1;
    while v >= 0 {
        let var = cd(scip, cons).vars[v];
        if !crate::var::var_is_binary(scip, var) {
            allbinary = false;
            break;
        } else if cd(scip, cons).boundtypes[v] == BoundType::Lower {
            newvars[v as usize] = if nvars == 2 { scip_var::get_negated_var(scip, var)? } else { var };
        } else {
            newvars[v as usize] = if nvars > 2 { scip_var::get_negated_var(scip, var)? } else { var };
        }
        v -= 1;
    }

    if allbinary {
        let name = scip.conss[cons].name.clone();
        let flags = cons_flags(scip, cons);
        let mut newcons = if nvars == 2 {
            crate::cons_setppc::create_cons_setpack(scip, &name, &newvars, &flags)?
        } else {
            crate::cons_logicor::create_cons_logicor(scip, &name, &newvars, &flags)?
        };

        // Add the upgraded constraint to the problem.
        scip_cons::add_cons_upgrade(scip, cons, &mut newcons)?;
        *naddconss += 1;

        // Remove the underlying constraint from the problem.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
    }

    Ok(())
}

/// `analyzeConflict`: analyzes conflicting assignment on given constraint,
/// and adds conflict constraint to problem.
fn analyze_conflict(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // Conflict analysis can only be applied in solving stage and if it is
    // turned on.
    if (scip.set.stage != Stage::Solving && !api::in_probing(scip)) || !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    // Initialize conflict analysis, and add all bounds of infeasible
    // constraint to conflict candidate queue.
    api::init_conflict_analysis(scip, crate::types::ConfType::Propagation, false)?;

    for v in 0..cd(scip, cons).nvars {
        // The opposite bound is in conflict with this literal.
        let (var, bt) = (cd(scip, cons).vars[v], cd(scip, cons).boundtypes[v]);
        api::add_conflict_bd(scip, var, bt.opposite(), None)?;
    }

    // Analyze the conflict.
    api::analyze_conflict_cons(scip, cons)?;

    Ok(())
}

/// `disableCons`: disables or deletes the given constraint, depending on the
/// current depth.
fn disable_cons(scip: &mut Scip, cons: ConsId) -> ScipResult {
    if api::get_depth(scip) == scip.conss[cons].get_valid_depth() {
        scip_cons::del_cons(scip, cons)?;
    } else {
        cons_disable(scip, cons)?;
    }

    Ok(())
}

/// `processWatchedVars`: checks constraint for violation only looking at the
/// watched variables, applies bound changes if possible.
fn process_watched_vars(
    scip: &mut Scip,
    cons: ConsId,
    eventhdlr: EventhdlrId,
    cutoff: &mut bool,
    infeasible: &mut bool,
    reduceddom: &mut bool,
    mustcheck: &mut bool,
) -> ScipResult {
    // Init bools.
    *cutoff = false;
    *infeasible = false;
    *reduceddom = false;
    *mustcheck = false;

    let nvars = cd(scip, cons).nvars;

    // Check watched variables if they are satisfying the literal.
    let (cw1, cw2) = (cd(scip, cons).watchedvar1, cd(scip, cons).watchedvar2);
    if cw1 >= 0 && is_literal_satisfied(scip, cons, cw1) {
        // The literal is satisfied, making the constraint redundant.
        disable_cons(scip, cons)?;
        return Ok(());
    }
    if cw2 >= 0 && is_literal_satisfied(scip, cons, cw2) {
        // The literal is satisfied, making the constraint redundant.
        disable_cons(scip, cons)?;
        return Ok(());
    }

    // Check if watched variables are still undecided.
    let mut watchedvar1 = -1;
    let mut watchedvar2 = -1;
    let mut nbranchings1 = i64::MAX;
    let mut nbranchings2 = i64::MAX;
    if cw1 >= 0 && !is_literal_violated(scip, cons, cw1) {
        watchedvar1 = cw1;
        nbranchings1 = -1; // prefer keeping the watched variable
    }
    if cw2 >= 0 && !is_literal_violated(scip, cons, cw2) {
        if watchedvar1 == -1 {
            watchedvar1 = cw2;
            nbranchings1 = -1; // prefer keeping the watched variable
        } else {
            watchedvar2 = cw2;
            nbranchings2 = -1; // prefer keeping the watched variable
        }
    }

    // Search for new watched variables.
    if watchedvar2 == -1 {
        for v in 0..nvars {
            // Don't process the watched variables again.
            if v == cw1 || v == cw2 {
                continue;
            }

            // Check, if the literal is violated.
            if is_literal_violated(scip, cons, v) {
                continue;
            }

            // Check, if the literal is satisfied.
            if is_literal_satisfied(scip, cons, v) {
                // The literal is satisfied, making the constraint redundant; make
                // sure, the feasible variable is watched and disable the constraint.
                if cw1 != -1 {
                    switch_watchedvars(scip, cons, eventhdlr, cw1, v)?;
                } else {
                    switch_watchedvars(scip, cons, eventhdlr, v, cw2)?;
                }
                disable_cons(scip, cons)?;
                return Ok(());
            }

            // The literal is still undecided and can be used as watched variable.
            let (var, bt) = (cd(scip, cons).vars[v], cd(scip, cons).boundtypes[v]);
            let dir = if bt == BoundType::Lower {
                crate::types::BranchDir::Downwards
            } else {
                crate::types::BranchDir::Upwards
            };
            let nbranchings = crate::var::var_get_n_branchings_current_run(scip, var, dir);
            if nbranchings < nbranchings2 {
                if nbranchings < nbranchings1 {
                    watchedvar2 = watchedvar1;
                    nbranchings2 = nbranchings1;
                    watchedvar1 = v;
                    nbranchings1 = nbranchings;
                } else {
                    watchedvar2 = v;
                    nbranchings2 = nbranchings;
                }
            }
        }
    }

    if watchedvar1 == -1 {
        // There is no undecided literal left -> the constraint is infeasible
        //  - a modifiable constraint is infeasible
        //  - an unmodifiable constraint is infeasible and the node can be cut off
        *infeasible = true;

        api::reset_cons_age(scip, cons);
        if !scip.conss[cons].modifiable {
            // Use conflict analysis to get a conflict constraint out of the
            // conflicting assignment.
            analyze_conflict(scip, cons)?;

            // Mark the node to be cut off.
            *cutoff = true;
        }
    } else if watchedvar2 == -1 {
        // There is only one undecided literal:
        // - a modifiable constraint must be checked manually
        // - we cannot change bounds of multi-aggregated variables and have to
        //   check manually
        // - an unmodifiable constraint is feasible and can be disabled after the
        //   remaining literal is satisfied
        let var = cd(scip, cons).vars[watchedvar1];
        let probvar = crate::var::var_get_probvar(scip, var);
        if scip.conss[cons].modifiable || scip.vars[probvar].varstatus == VarStatus::MultAggr {
            *mustcheck = true;
        } else {
            // Satisfy remaining literal and disable constraint; make sure, the
            // fixed-to-one variable is watched.
            let (bt, bound) = (cd(scip, cons).boundtypes[watchedvar1], cd(scip, cons).bounds[watchedvar1]);
            if bt == BoundType::Lower {
                scip_var::infer_var_lb_cons(scip, var, bound, cons, watchedvar1, true)?;
            } else {
                scip_var::infer_var_ub_cons(scip, var, bound, cons, watchedvar1, true)?;
            }
            api::reset_cons_age(scip, cons);
            let cw1 = cd(scip, cons).watchedvar1;
            if watchedvar1 != cw1 {
                // Keep one of the watched variables.
                switch_watchedvars(scip, cons, eventhdlr, watchedvar1, cw1)?;
            }
            disable_cons(scip, cons)?;
            *reduceddom = true;
        }
    } else {
        // Switch to the new watched variables.
        switch_watchedvars(scip, cons, eventhdlr, watchedvar1, watchedvar2)?;

        // There are at least two undecided variables -> the constraint must be
        // checked manually.
        *mustcheck = true;

        // Disable propagation of constraint until the corresponding bound of a
        // watched variable changed.
        cons_disable_propagation(scip, cons);

        // Increase aging counter.
        let nvars = cd(scip, cons).nvars;
        api::add_cons_age(scip, cons, ageincrease(nvars as f64))?;
    }

    Ok(())
}

/// `isConsViolated`: checks constraint for violation, returns TRUE iff
/// constraint is violated.
fn is_cons_violated(scip: &mut Scip, cons: ConsId, sol: SolId) -> bool {
    let nvars = cd(scip, cons).nvars;

    // Check the given solution.
    let mut absviol = f64::MAX;
    let mut violpos = -1;
    for v in 0..nvars {
        let c = cd(scip, cons);
        let (var, bt, bound) = (c.vars[v], c.boundtypes[v], c.bounds[v]);
        let solval = api::get_sol_val(scip, sol, var);

        // Update absolute violation if needed.
        let viol = if bt == BoundType::Lower { bound - solval } else { solval - bound };
        if viol < absviol {
            absviol = viol;
            violpos = v;
        }

        if (bt == BoundType::Lower && is_feas_ge(scip, solval, bound)) || (bt == BoundType::Upper && is_feas_le(scip, solval, bound)) {
            return false;
        }
    }

    // Update constraint violation in solution.
    if sol != NONE {
        let relviol = if nvars == 0 {
            f64::MAX
        } else {
            let c = cd(scip, cons);
            let (var, bound) = (c.vars[violpos], c.bounds[violpos]);
            crate::misc::rel_diff(api::get_sol_val(scip, sol, var), bound)
        };
        api::update_sol_cons_violation(scip, sol, absviol, relviol);
    }

    true
}

/// `registerBranchingCandidates`: registers variables of a constraint as
/// branching candidates; sets `neednarybranch` if an n-ary branch is
/// necessary to enforce this constraint, because all active literals are
/// w.r.t. continuous variables which bound (in the literal) is at the
/// variable's bound.
fn register_branching_candidates(scip: &mut Scip, cons: ConsId, sol: SolId, cutoff: &mut bool, neednarybranch: &mut bool) -> ScipResult {
    let nvars = cd(scip, cons).nvars;

    *cutoff = true;
    *neednarybranch = true;

    for v in 0..nvars {
        let c = cd(scip, cons);
        let (var, bt, bound) = (c.vars[v], c.boundtypes[v], c.bounds[v]);

        let varlb = compute_var_lb_local(scip, var);
        let varub = compute_var_ub_local(scip, var);

        // If literal is x >= varlb, but upper bound on x is < varlb, then this
        // literal can never be satisfied, thus there is no use for branching.
        if bt == BoundType::Lower && is_feas_lt(scip, varub, bound) {
            continue;
        }

        // If literal is x <= varub, but lower bound on x is > varub, then this
        // literal can never be satisfied, thus there is no use for branching.
        if bt == BoundType::Upper && is_feas_gt(scip, varlb, bound) {
            continue;
        }

        // If literal is always satisfied, then no need to branch on it may happen
        // if propagation is disabled for some reason and due to numerics current
        // solution does not satisfy literal, but variable bounds do.
        if is_literal_satisfied(scip, cons, v) {
            continue;
        }

        let violation = api::get_sol_val(scip, sol, var) - bound;

        // If variable is continuous, then we cannot branch on one of the variable
        // bounds.
        let s = &scip.set;
        if crate::var::var_is_integral(scip, var) || ((s.is_infinity(-varlb) || !s.is_feas_eq(bound, varlb)) && (s.is_infinity(varub) || !s.is_feas_eq(bound, varub))) {
            api::add_extern_branch_cand(scip, var, violation.abs(), bound);
            *neednarybranch = false;
        }
        *cutoff = false;
    }

    Ok(())
}

/// `enforceCurrentSol`: enforces the pseudo or LP solution on the given
/// constraint.
fn enforce_current_sol(
    scip: &mut Scip,
    cons: ConsId,
    sol: SolId,
    eventhdlr: EventhdlrId,
    cutoff: &mut bool,
    infeasible: &mut bool,
    reduceddom: &mut bool,
    registeredbrcand: &mut bool,
) -> ScipResult {
    let mut mustcheck = false;
    let mut neednarybranch = false;

    // Update and check the watched variables, if they were changed since last
    // processing.
    if scip.conss[cons].is_propagation_enabled() {
        process_watched_vars(scip, cons, eventhdlr, cutoff, infeasible, reduceddom, &mut mustcheck)?;
    } else {
        mustcheck = true;
    }

    if mustcheck && is_cons_violated(scip, cons, sol) {
        // Constraint was infeasible -> reset age.
        api::reset_cons_age(scip, cons);
        *infeasible = true;

        // Register branching candidates.
        register_branching_candidates(scip, cons, sol, cutoff, &mut neednarybranch)?;

        if !neednarybranch {
            *registeredbrcand = true;
        }
    }

    Ok(())
}

/// `createNAryBranch`: enforces a constraint by creating an n-ary branch
/// consisting of a set of child nodes, each enforcing one literal.
fn create_nary_branch(scip: &mut Scip, cons: ConsId, _sol: SolId) -> ScipResult {
    let nvars = cd(scip, cons).nvars;

    for v in 0..nvars {
        let c = cd(scip, cons);
        let (var, bt, bound) = (c.vars[v], c.boundtypes[v], c.bounds[v]);

        let varlb = compute_var_lb_local(scip, var);
        let varub = compute_var_ub_local(scip, var);

        // If literal is x >= varlb, but upper bound on x is < varlb, then this
        // literal can never be satisfied, thus there is no use in creating an
        // extra child for it.
        if bt == BoundType::Lower && is_feas_lt(scip, varub, bound) {
            continue;
        }
        // If literal is x <= varub, but lower bound on x is > varub, then this
        // literal can never be satisfied, thus there is no use in creating an
        // extra child for it.
        if bt == BoundType::Upper && is_feas_gt(scip, varlb, bound) {
            continue;
        }
        // If literal is always satisfied, then no need to branch on it.
        if is_literal_satisfied(scip, cons, v) {
            continue;
        }

        // Create a child that enforces the current literal.
        let dir = if bt == BoundType::Lower {
            crate::types::BranchDir::Upwards
        } else {
            crate::types::BranchDir::Downwards
        };
        let priority = api::calc_nodesel_priority(scip, var, dir, bound);
        let estimate = api::calc_child_estimate(scip, var, bound);

        let node = api::create_child(scip, priority, estimate)?;

        // Enforce current literal.
        if scip.vars[var].varstatus == VarStatus::MultAggr {
            let one = 1.0;
            let inf = scip.set.infinity();
            let (lhs, rhs) = if bt == BoundType::Lower { (bound, inf) } else { (-inf, bound) };
            let f = cons_flags(scip, cons);
            let mut brcons = crate::cons_linear::create_cons_linear(
                scip,
                "bounddisjbranch",
                &[var],
                &[one],
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
            scip_cons::add_cons_node(scip, node, brcons, NONE)?;
            scip_cons::release_cons(scip, &mut brcons)?;
        } else if bt == BoundType::Lower {
            scip_var::chg_var_lb_node(scip, node, var, bound)?;
        } else {
            scip_var::chg_var_ub_node(scip, node, var, bound)?;
        }

        // Delete bound disjunction constraint from child node.
        scip_cons::del_cons_node(scip, node, cons)?;
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce constraints.
fn enforce_constraint(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut infeasible = false;
    let mut reduceddom = false;
    let mut registeredbrcand = false;
    let mut narybranchcons = NONE;
    let mut nnarybranchconsvars = i32::MAX;

    // Check all bound disjunction constraints for feasibility.
    let mut c = 0;
    while c < conss.len() && !cutoff && !reduceddom {
        let mut infeasiblecons = false;
        enforce_current_sol(scip, conss[c], sol, eventhdlr, &mut cutoff, &mut infeasiblecons, &mut reduceddom, &mut registeredbrcand)?;
        infeasible |= infeasiblecons;
        if infeasiblecons && !registeredbrcand {
            // If cons. c has less literals than the previous candidate for an
            // n-ary branch, then keep cons. c as candidate for n-ary branch.
            if narybranchcons == NONE || cd(scip, conss[c]).nvars < nnarybranchconsvars {
                narybranchcons = conss[c];
                nnarybranchconsvars = cd(scip, narybranchcons).nvars;
            }
        }
        c += 1;
    }

    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if reduceddom {
        *result = ScipResultCode::ReducedDom;
    } else if infeasible {
        if registeredbrcand {
            *result = ScipResultCode::Infeasible;
        } else {
            create_nary_branch(scip, narybranchcons, sol)?;
            *result = ScipResultCode::Branched;
        }
    }

    Ok(())
}

/*
 * Callback methods of constraint handler
 */

/// `consFreeBounddisjunction`: destructor of constraint handler to free
/// constraint handler data.
fn cons_free_bounddisjunction(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consExitpreBounddisjunction`: presolving deinitialization method of
/// constraint handler (called after presolving has been finished).
fn cons_exitpre_bounddisjunction(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Fast processing of constraints, apply global bounds and remove fixed
    // variables.
    for &cons in conss {
        if scip.conss[cons].deleted {
            continue;
        }

        // Remove all literals that are violated in global bounds, check
        // redundancy due to global bounds.
        let mut redundant = apply_global_bounds(scip, cons, eventhdlr)?;

        if !redundant {
            // Replace variables by their representative active (or
            // multi-aggregated) variables.
            remove_fixed_variables(scip, cons, eventhdlr, &mut redundant)?;
        }

        if redundant && scip.conss[cons].is_added() {
            scip_cons::del_cons(scip, cons)?;
        }
    }

    Ok(())
}

// `consInitsolBounddisjunction` only adds nlrows to the NLP when it has been
// constructed, which the port never does (the NLP is not ported), so the
// callback is not set.

/// `consDeleteBounddisjunction`: frees specific constraint data.
fn cons_delete_bounddisjunction(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    *consdata = None;
    Ok(())
}

/// `consTransBounddisjunction`: transforms constraint data into data
/// belonging to the transformed problem.
fn cons_trans_bounddisjunction(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let s = cd(scip, sourcecons);
    let n = s.nvars as usize;
    let (vars, boundtypes, bounds) = (s.vars.0[..n].to_vec(), s.boundtypes.0[..n].to_vec(), s.bounds.0[..n].to_vec());

    // Create constraint data for target constraint.
    let targetdata = consdata_create(scip, &vars, &boundtypes, &bounds)?;

    // Create target constraint.
    let name = scip.conss[sourcecons].name.clone();
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &name, conshdlr, Some(Box::new(targetdata)), &flags)?;

    Ok(())
}

/// `consEnfolpBounddisjunction`: constraint enforcing method of constraint
/// handler for LP solutions.
fn cons_enfolp_bounddisjunction(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, _solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, NONE, result)
}

/// `consEnforelaxBounddisjunction`: constraint enforcing method of
/// constraint handler for relaxation solutions.
fn cons_enforelax_bounddisjunction(
    scip: &mut Scip,
    sol: SolId,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    enforce_constraint(scip, conshdlr, conss, sol, result)
}

/// `consEnfopsBounddisjunction`: constraint enforcing method of constraint
/// handler for pseudo solutions (unlike `enforceConstraint`, `infeasible`
/// carries over from constraint to constraint).
fn cons_enfops_bounddisjunction(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut infeasible = false;
    let mut reduceddom = false;
    let mut registeredbrcand = false;
    let mut narybranchcons = NONE;

    // Check all bound disjunction constraints for feasibility.
    let mut c = 0;
    while c < conss.len() && !cutoff && !reduceddom {
        enforce_current_sol(scip, conss[c], NONE, eventhdlr, &mut cutoff, &mut infeasible, &mut reduceddom, &mut registeredbrcand)?;
        if infeasible && !registeredbrcand {
            // If cons. c has less literals than the previous candidate for an
            // n-ary branch, then keep cons. c as candidate for n-ary branch.
            if narybranchcons == NONE || cd(scip, conss[c]).nvars < cd(scip, narybranchcons).nvars {
                narybranchcons = conss[c];
            }
        }
        c += 1;
    }

    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if reduceddom {
        *result = ScipResultCode::ReducedDom;
    } else if infeasible {
        if registeredbrcand {
            *result = ScipResultCode::Infeasible;
        } else {
            create_nary_branch(scip, narybranchcons, NONE)?;
            *result = ScipResultCode::Branched;
        }
    }

    Ok(())
}

/// `consCheckBounddisjunction`: feasibility check method of constraint
/// handler for integral solutions.
#[allow(clippy::too_many_arguments)]
fn cons_check_bounddisjunction(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    _checklprows: bool,
    _printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;

    // Check all bound disjunction constraints for feasibility.
    let mut c = 0;
    while c < conss.len() && (*result == ScipResultCode::Feasible || completely) {
        if is_cons_violated(scip, conss[c], sol) {
            // Constraint is violated.
            *result = ScipResultCode::Infeasible;
        }
        c += 1;
    }

    Ok(())
}

/// `consPropBounddisjunction`: domain propagation method of constraint
/// handler.
fn cons_prop_bounddisjunction(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nusefulconss: i32,
    _nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    let mut cutoff = false;
    let mut infeasible = false;
    let mut reduceddom = false;
    let mut mustcheck = false;
    let mut consreduceddom = false;

    // Propagate all useful bound disjunction constraints.
    let mut c = 0;
    while c < nusefulconss as usize && !cutoff {
        process_watched_vars(scip, conss[c], eventhdlr, &mut cutoff, &mut infeasible, &mut consreduceddom, &mut mustcheck)?;
        reduceddom = reduceddom || consreduceddom;
        c += 1;
    }

    // Return the correct result.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if reduceddom {
        ScipResultCode::ReducedDom
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `consPresolBounddisjunction`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_bounddisjunction(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Process constraints.
    let mut c = 0;
    while c < conss.len() && *result != ScipResultCode::Cutoff && !crate::solve::solve_is_stopped(scip, false) {
        let cons = conss[c];
        c += 1;

        // Force presolving the constraint in the initial round.
        if nrounds == 0 {
            cons_enable_propagation(scip, cons);
        }

        // Remove all literals that are violated in global bounds, check
        // redundancy due to global bounds.
        let mut redundant = apply_global_bounds(scip, cons, eventhdlr)?;

        if !redundant {
            // Replace variables by their representative active (or
            // multi-aggregated) variables.
            remove_fixed_variables(scip, cons, eventhdlr, &mut redundant)?;
        }

        if redundant {
            scip_cons::del_cons(scip, cons)?;
            counts.ndelconss += 1;
            *result = ScipResultCode::Success;
            continue;
        } else if !scip.conss[cons].modifiable {
            // If unmodifiable constraint has no variables, it is infeasible, if
            // unmodifiable constraint has only one variable, the literal can be
            // satisfied and the constraint deleted.
            if cd(scip, cons).nvars == 0 {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }
            if scip.conss[cons].nupgradelocks >= 1 {
                continue;
            }
            if cd(scip, cons).nvars == 1 {
                let k = cd(scip, cons);
                let (var, bt, bound) = (k.vars[0], k.boundtypes[0], k.bounds[0]);
                if crate::var::var_is_active(scip, var) {
                    let (infeasible, _tightened) = if bt == BoundType::Lower {
                        scip_var::tighten_var_lb(scip, var, bound, true)?
                    } else {
                        scip_var::tighten_var_ub(scip, var, bound, true)?
                    };
                    if infeasible {
                        *result = ScipResultCode::Cutoff;
                        return Ok(());
                    }
                    counts.nchgbds += 1;
                } else {
                    // Upgrade to a linear constraint, if vars[0] is
                    // multi-aggregated.
                    let one = 1.0;
                    let inf = scip.set.infinity();
                    let (lhs, rhs) = if bt == BoundType::Lower { (bound, inf) } else { (-inf, bound) };
                    let name = scip.conss[cons].name.clone();
                    let f = cons_flags(scip, cons);
                    let mut lincons = crate::cons_linear::create_cons_linear(
                        scip,
                        &name,
                        &[var],
                        &[one],
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

                    // Add the upgraded constraint to the problem.
                    scip_cons::add_cons_upgrade(scip, cons, &mut lincons)?;
                    counts.nupgdconss += 1;
                }

                scip_cons::del_cons(scip, cons)?;
                counts.ndelconss += 1;
                *result = ScipResultCode::Success;
                continue;
            } else {
                // Try to upgrade the bounddisjunction constraint.
                upgrade_cons(scip, cons, &mut counts.ndelconss, &mut counts.naddconss)?;
            }
        }
    }

    Ok(())
}

/// `consRespropBounddisjunction`: propagation conflict resolving method of
/// constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_bounddisjunction(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    _infervar: VarId,
    inferinfo: i32,
    _boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    // The only deductions are bounds tightened to a literal's bound on bound
    // disjunction constraints where all other literals are violated.
    for v in 0..cd(scip, cons).nvars {
        if v != inferinfo {
            // The reason literal must have been violated.
            let (var, bt) = (cd(scip, cons).vars[v], cd(scip, cons).boundtypes[v]);
            api::add_conflict_bd(scip, var, bt.opposite(), bdchgidx.as_ref())?;
        }
    }

    *result = ScipResultCode::Success;

    Ok(())
}

/// `consLockBounddisjunction`: variable rounding lock method of constraint
/// handler.
fn cons_lock_bounddisjunction(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    // Lock every single coefficient.
    for i in 0..cd(scip, cons).nvars {
        let (var, bt) = (cd(scip, cons).vars[i], cd(scip, cons).boundtypes[i]);
        if bt == BoundType::Lower {
            api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
        } else {
            api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
        }
    }

    Ok(())
}

/// `consActiveBounddisjunction`: constraint activation notification method
/// of constraint handler.
fn cons_active_bounddisjunction(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Catch events on watched variables.
    let w1 = cd(scip, cons).watchedvar1;
    if w1 != -1 {
        let f = catch_events(scip, cons, eventhdlr, w1)?;
        cdm(scip, cons).filterpos1 = f;
    }
    let w2 = cd(scip, cons).watchedvar2;
    if w2 != -1 {
        let f = catch_events(scip, cons, eventhdlr, w2)?;
        cdm(scip, cons).filterpos2 = f;
    }

    Ok(())
}

/// `consDeactiveBounddisjunction`: constraint deactivation notification
/// method of constraint handler.
fn cons_deactive_bounddisjunction(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    let eventhdlr = hd(scip, conshdlr).eventhdlr;

    // Drop events on watched variables.
    let (w1, f1) = (cd(scip, cons).watchedvar1, cd(scip, cons).filterpos1);
    if w1 != -1 {
        drop_events(scip, cons, eventhdlr, w1, f1)?;
        cdm(scip, cons).watchedvar1 = -1;
    }
    let (w2, f2) = (cd(scip, cons).watchedvar2, cd(scip, cons).filterpos2);
    if w2 != -1 {
        drop_events(scip, cons, eventhdlr, w2, f2)?;
        cdm(scip, cons).watchedvar2 = -1;
    }

    Ok(())
}

/// `consGetVarsBounddisjunction`: constraint method of constraint handler
/// which returns the variables (the vector always fits here).
fn cons_get_vars_bounddisjunction(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    let c = cd(scip, cons);
    vars.clear();
    vars.extend_from_slice(&c.vars.0[..c.nvars as usize]);
    *success = true;
    Ok(())
}

/// `consGetNVarsBounddisjunction`: constraint method of constraint handler
/// which returns the number of variables.
fn cons_get_n_vars_bounddisjunction(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *nvars = cd(scip, cons).nvars;
    *success = true;
    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecBounddisjunction`: the event data is the constraint.
fn event_exec_bounddisjunction(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let cons: ConsId = eventdata;

    if (event.eventtype & EVENTTYPE_BOUNDRELAXED) != 0 {
        cons_enable(scip, cons)?;
    }

    cons_enable_propagation(scip, cons);

    Ok(())
}

/*
 * Callback methods of conflict handler
 */

/// `SCIP_ConflicthdlrData`: conflict handler data.
pub struct ConflicthdlrData {
    /// Maximal percantage of continuous variables within a conflict.
    pub continuousfrac: f64,
}

/// `conflictExecBounddisjunction`: conflict processing method of conflict
/// handler (called when conflict was found).
fn conflict_exec_bounddisjunction(
    scip: &mut Scip,
    conflicthdlr: crate::types::ConflicthdlrId,
    args: &crate::conflict::ConflictExecArgs,
    result: &mut ScipResultCode,
) -> ScipResult {
    // Don't process already resolved conflicts.
    if args.resolved {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    let continuousfrac = scip.conflicthdlrs[conflicthdlr]
        .conflicthdlrdata
        .as_ref()
        .unwrap()
        .downcast_ref::<ConflicthdlrData>()
        .unwrap()
        .continuousfrac;

    *result = ScipResultCode::DidNotFind;
    let mut ncontinuous = 0;

    // Create array of variables, boundtypes, and bound values in conflict
    // constraint.
    let nbdchginfos = args.bdchginfos.len();
    let mut vars = vec![NONE; nbdchginfos];
    let mut boundtypes = vec![BoundType::Lower; nbdchginfos];
    let mut bounds = vec![0.0; nbdchginfos];

    let mut nliterals = 0;
    let mut i = 0;
    'outer: while i < nbdchginfos {
        let info = crate::conflict::bdchginfo(scip, args.bdchginfos[i]);
        let var = info.var;
        let boundtype = info.boundtype.opposite();
        let mut bound = args.relaxedbds[i];

        // For continuous variables, we can only use the relaxed version of the
        // bounds negation: !(x <= u) -> x >= u.
        if crate::var::var_is_integral(scip, var) {
            bound += if boundtype == BoundType::Lower { 1.0 } else { -1.0 };
        }

        // Check whether we have seen the variable before.
        let mut j = nliterals as i32 - 1;
        while j >= 0 {
            let ju = j as usize;
            j -= 1;
            if vars[ju] != var {
                continue;
            }

            // Check whether both literals contribute with the same bound type.
            if boundtypes[ju] == boundtype {
                // Check whether the lower or the upper bound can be relaxed.
                if (boundtype == BoundType::Lower && scip.set.is_lt(bound, bounds[ju])) || (boundtype == BoundType::Upper && scip.set.is_gt(bound, bounds[ju])) {
                    bounds[ju] = bound;
                }

                continue;
            }
            // Check whether the bounds are overlapping.
            else if is_overlapping(scip, var, boundtype, bound, boundtypes[ju], bounds[ju]) {
                // The conflict is redundant -> discard the conflict constraint.
                break 'outer;
            }
        }

        vars[nliterals] = var;
        boundtypes[nliterals] = boundtype;
        bounds[nliterals] = bound;

        // Check if the relaxed bound is really a relaxed bound; for continuous
        // variables, we can only use the relaxed version of the bounds negation:
        // !(x <= u) -> x >= u. C reads boundtypes[i] here, not
        // boundtypes[nliterals]; once literals were merged, that slot holds
        // whatever an earlier iteration (or nothing, here Lower) left there.
        if !crate::var::var_is_integral(scip, vars[nliterals]) {
            let x = &scip.vars[var];
            if (boundtypes[i] == BoundType::Lower && scip.set.is_feas_eq(x.glbdom.lb, bounds[nliterals]))
                || (boundtypes[i] == BoundType::Upper && scip.set.is_feas_eq(x.glbdom.ub, bounds[nliterals]))
            {
                // The literal is satisfied in global bounds (may happen due to weak
                // "negation" of continuous variables) -> discard the conflict
                // constraint.
                break 'outer;
            } else {
                ncontinuous += 1;
            }
        }

        nliterals += 1;
        i += 1;
    }

    // Create a constraint out of the conflict set.
    if i == nbdchginfos && (ncontinuous as f64) < continuousfrac * nbdchginfos as f64 + 0.5 {
        let napplied = scip.conflict.as_ref().map_or(0, |c| c.nappliedglbconss + c.nappliedlocconss);
        let consname = format!("cf{}_{}", api::get_n_runs(scip), napplied);
        let flags = ConsFlags {
            initial: false,
            separate: false,
            enforce: false,
            check: false,
            propagate: true,
            local: args.local,
            modifiable: false,
            dynamic: args.dynamic,
            removable: args.removable,
            stickingatnode: false,
        };
        let mut cons = create_cons_bounddisjunction(scip, &consname, &vars[..nliterals], &boundtypes[..nliterals], &bounds[..nliterals], &flags)?;

        // Add conflict to SCIP.
        scip_cons::add_conflict(scip, args.node, &mut cons, args.validnode, args.conftype, args.cutoffinvolved)?;
        *result = ScipResultCode::ConsAdded;
    }

    Ok(())
}

/// `conflictFreeBounddisjunction`: free method of conflict handler.
fn conflict_free_bounddisjunction(scip: &mut Scip, conflicthdlr: crate::types::ConflicthdlrId) -> ScipResult {
    scip.conflicthdlrs[conflicthdlr].conflicthdlrdata = None;
    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrBounddisjunction`: creates the handler for bound
/// disjunction constraints and includes it in SCIP. Sub-SCIP copying,
/// parsing, printing and the symmetry graphs are added with those modules.
pub fn include_conshdlr_bounddisjunction(scip: &mut Scip) -> ScipResult {
    // Create event handler for events on watched variables.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_bounddisjunction, None)?;

    // Allocate memory for conflict handler data. C adds the handler's
    // parameter before including the handler; here it binds to the handler's
    // data, so it is added right after (only the parameter listing order
    // differs).
    let conflicthdlrdata = ConflicthdlrData {
        continuousfrac: DEFAULT_CONTINUOUSFRAC,
    };

    // Create conflict handler for bound disjunction constraints.
    let conflicthdlr = api::include_conflicthdlr_basic(
        scip,
        CONFLICTHDLR_NAME,
        CONFLICTHDLR_DESC,
        CONFLICTHDLR_PRIORITY,
        conflict_exec_bounddisjunction,
        Some(Box::new(conflicthdlrdata)),
    )?;
    crate::paramset::paramset_add_real(
        scip,
        &format!("conflict/{CONSHDLR_NAME}/continuousfrac"),
        "maximal percantage of continuous variables within a conflict",
        crate::data_target!(f64, conflicthdlrs, conflicthdlr, conflicthdlrdata, ConflicthdlrData, continuousfrac),
        false,
        DEFAULT_CONTINUOUSFRAC,
        0.0,
        1.0,
        None,
    )?;
    scip.conflicthdlrs[conflicthdlr].conflictfree = Some(conflict_free_bounddisjunction);

    // Create constraint handler data.
    let conshdlrdata = ConshdlrData { eventhdlr };

    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_bounddisjunction),
        Some(cons_enfops_bounddisjunction),
        Some(cons_check_bounddisjunction),
        Some(cons_lock_bounddisjunction),
        Some(Box::new(conshdlrdata)),
    )?;

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_bounddisjunction);
    h.conscopy = Some(cons_copy_bounddisjunction);
    h.consactive = Some(cons_active_bounddisjunction);
    h.consdeactive = Some(cons_deactive_bounddisjunction);
    h.consdelete = Some(cons_delete_bounddisjunction);
    h.consexitpre = Some(cons_exitpre_bounddisjunction);
    h.consfree = Some(cons_free_bounddisjunction);
    h.consgetvars = Some(cons_get_vars_bounddisjunction);
    h.consgetnvars = Some(cons_get_n_vars_bounddisjunction);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_bounddisjunction), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(
        scip,
        conshdlr,
        Some(cons_prop_bounddisjunction),
        CONSHDLR_PROPFREQ,
        CONSHDLR_DELAYPROP,
        CONSHDLR_PROP_TIMING,
    )?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_bounddisjunction);
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_bounddisjunction);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_bounddisjunction);

    Ok(())
}

/// `SCIPcreateConsBounddisjunction`: creates and captures a bound
/// disjunction constraint.
pub fn create_cons_bounddisjunction(scip: &mut Scip, name: &str, vars: &[VarId], boundtypes: &[BoundType], bounds: &[f64], flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the bounddisjunction constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Bound disjunction constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Create the constraint specific data.
    let consdata = consdata_create(scip, vars, boundtypes, bounds)?;

    // Create constraint.
    scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)
}

/// `SCIPcreateConsBasicBounddisjunction`: creates and captures a bound
/// disjunction constraint in its most basic version.
pub fn create_cons_basic_bounddisjunction(scip: &mut Scip, name: &str, vars: &[VarId], boundtypes: &[BoundType], bounds: &[f64]) -> ScipResult<ConsId> {
    create_cons_bounddisjunction(scip, name, vars, boundtypes, bounds, &ConsFlags::default())
}

/// `SCIPcreateConsBounddisjunctionRedundant`: creates and captures a bound
/// disjunction constraint with possibly redundant literals.
pub fn create_cons_bounddisjunction_redundant(scip: &mut Scip, name: &str, vars: &[VarId], boundtypes: &[BoundType], bounds: &[f64], flags: &ConsFlags) -> ScipResult<ConsId> {
    // Find the bounddisjunction constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        // Bound disjunction constraint handler not found.
        return Err(Retcode::PluginNotFound);
    }

    // Create the constraint specific data.
    let consdata = consdata_create_redundant(scip, vars, boundtypes, bounds)?;

    // Create constraint.
    scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), flags)
}

/// `SCIPcreateConsBasicBounddisjunctionRedundant`: creates and captures a
/// bound disjunction constraint with possibly redundant literals in its most
/// basic version.
pub fn create_cons_basic_bounddisjunction_redundant(scip: &mut Scip, name: &str, vars: &[VarId], boundtypes: &[BoundType], bounds: &[f64]) -> ScipResult<ConsId> {
    create_cons_bounddisjunction_redundant(scip, name, vars, boundtypes, bounds, &ConsFlags::default())
}

/// `SCIPgetNVarsBounddisjunction`: gets number of variables in bound
/// disjunction constraint.
pub fn get_n_vars_bounddisjunction(scip: &Scip, cons: ConsId) -> i32 {
    cd(scip, cons).nvars
}

/// `SCIPgetVarsBounddisjunction`: gets array of variables in bound
/// disjunction constraint.
pub fn get_vars_bounddisjunction(scip: &Scip, cons: ConsId) -> &[VarId] {
    let c = cd(scip, cons);
    &c.vars.0[..c.nvars as usize]
}

/// `SCIPgetBoundtypesBounddisjunction`: gets array of bound types in bound
/// disjunction constraint.
pub fn get_boundtypes_bounddisjunction(scip: &Scip, cons: ConsId) -> &[BoundType] {
    let c = cd(scip, cons);
    &c.boundtypes.0[..c.nvars as usize]
}

/// `SCIPgetBoundsBounddisjunction`: gets array of bounds in bound
/// disjunction constraint.
pub fn get_bounds_bounddisjunction(scip: &Scip, cons: ConsId) -> &[f64] {
    let c = cd(scip, cons);
    &c.bounds.0[..c.nvars as usize]
}

/// `conshdlrCopyBounddisjunction`: copy method for constraint handler plugins (called
/// when SCIP copies plugins).
fn conshdlr_copy_bounddisjunction(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_bounddisjunction(scip)?;
    *valid = true;
    Ok(())
}

/// `consCopyBounddisjunction`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_bounddisjunction(
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
    *valid = true;

    // Get source data.
    let sourcevars = get_vars_bounddisjunction(sourcescip, sourcecons).to_vec();
    let nvars = sourcevars.len();
    let boundtypes = get_boundtypes_bounddisjunction(sourcescip, sourcecons).to_vec();
    let bounds = get_bounds_bounddisjunction(sourcescip, sourcecons).to_vec();

    // Map source variables to active variables of the target SCIP.
    let mut targetvars = vec![NONE; nvars];
    let mut v = 0;
    while v < nvars && *valid {
        (targetvars[v], *valid) = crate::copy::get_var_copy(sourcescip, scip, sourcevars[v], Some(maps), global)?;
        v += 1;
    }

    // Only create the target constraint, if all variables could be copied.
    if *valid {
        let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);
        *cons = create_cons_bounddisjunction(scip, &consname, &targetvars, &boundtypes, &bounds, flags)?;
    }
    Ok(())
}
