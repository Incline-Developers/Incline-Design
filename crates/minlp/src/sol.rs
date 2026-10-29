//! SCIP's primal solutions (`sol.c`, `struct_sol.h`).

use crate::{
    def::UNKNOWN,
    lp,
    misc::{BoolArray, RealArray},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{HeurId, LockType, RelaxId, SolId, SolOrigin, SolType, VarId, VarStatus},
    var::{self, NONE},
};

/// `SCIP_VIOL`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Viol {
    pub absviolbounds: f64,
    pub relviolbounds: f64,
    pub absviollprows: f64,
    pub relviollprows: f64,
    pub absviolintegrality: f64,
    pub absviolcons: f64,
    pub relviolcons: f64,
}

/// The `creator` union of `SCIP_SOL`.
#[derive(Clone, Copy, Debug)]
pub struct Creator {
    pub heur: HeurId,
    pub relax: RelaxId,
}

impl Default for Creator {
    fn default() -> Self {
        Self { heur: NONE, relax: NONE }
    }
}

/// `SCIP_SOL`.
#[derive(Clone, Debug)]
pub struct Sol {
    pub obj: f64,
    pub time: f64,
    pub nodenum: i64,
    pub vals: RealArray,
    pub valid: BoolArray,
    pub creator: Creator,
    pub viol: Viol,
    pub runnum: i32,
    pub depth: i32,
    pub primalindex: i32,
    pub index: i32,
    pub solorigin: SolOrigin,
    pub hasinfval: bool,
    pub type_: SolType,
}

/// `solClearArrays`: clears solution arrays of primal CIP solution.
fn sol_clear_arrays(scip: &mut Scip, sol: SolId) {
    let s = &mut scip.sols[sol];
    s.valid.clear();
    s.hasinfval = false;
}

/// `solSetArrayVal`: sets value of variable in the solution's array.
fn sol_set_array_val(scip: &mut Scip, sol: SolId, var: VarId, val: f64) {
    let idx = scip.vars[var].index;

    // From now on, variable must not be deleted.
    var::var_mark_not_deletable(scip, var);

    let infinite = val != UNKNOWN && (scip.set.is_infinity(val) || scip.set.is_infinity(-val));
    let s = &mut scip.sols[sol];

    // Mark the variable valid.
    s.valid.set_val(idx, true);

    // Set the value in the solution array.
    s.vals.set_val(idx, val);

    // Store whether the solution has infinite values assigned to variables.
    if val != UNKNOWN {
        s.hasinfval = s.hasinfval || infinite;
    }
}

/// `solIncArrayVal`: increases value of variable in the solution's array.
fn sol_inc_array_val(scip: &mut Scip, sol: SolId, var: VarId, incval: f64) {
    let idx = scip.vars[var].index;

    // From now on, variable must not be deleted.
    var::var_mark_not_deletable(scip, var);

    let s = &mut scip.sols[sol];

    // If the variable was not valid, mark it to be valid and set the value to
    // the incval (it is 0.0 if not valid).
    if !s.valid.get_val(idx) {
        // Mark the variable valid.
        s.valid.set_val(idx, true);

        // Set the value in the solution array.
        s.vals.set_val(idx, incval);
    } else {
        // Increase the value in the solution array.
        s.vals.inc_val(idx, incval);
    }

    // Store whether the solution has infinite values assigned to variables.
    let incval = s.vals.get_val(idx);
    if incval != UNKNOWN {
        let infinite = scip.set.is_infinity(incval) || scip.set.is_infinity(-incval);
        let s = &mut scip.sols[sol];
        s.hasinfval = s.hasinfval || infinite;
    }
}

/// `solGetArrayVal`: returns the value of the variable in the given
/// solution.
fn sol_get_array_val(scip: &Scip, sol: SolId, var: VarId) -> f64 {
    let idx = scip.vars[var].index;
    let s = &scip.sols[sol];

    // Check, if the variable's value is valid.
    if s.valid.get_val(idx) {
        s.vals.get_val(idx)
    } else {
        // Return the variable's value corresponding to the origin.
        match s.solorigin {
            SolOrigin::Original | SolOrigin::Zero => 0.0,
            SolOrigin::LpSol => var::var_get_lp_sol(scip, var),
            SolOrigin::NlpSol => var::var_get_nlp_sol(scip, var),
            SolOrigin::RelaxSol => var::var_get_relax_sol_trans_var(scip, var),
            SolOrigin::PseudoSol => var::var_get_pseudo_sol(scip, var),
            SolOrigin::Partial | SolOrigin::Unknown => UNKNOWN,
        }
    }
}

/// `solUnlinkVar`: stores solution value of variable in solution's own
/// array.
fn sol_unlink_var(scip: &mut Scip, sol: SolId, var: VarId) -> ScipResult {
    // If variable is already valid, nothing has to be done.
    if scip.sols[sol].valid.get_val(scip.vars[var].index) {
        return Ok(());
    }

    // Store the correct solution value into the solution array.
    match scip.sols[sol].solorigin {
        SolOrigin::Original => {
            log::error!("cannot unlink solutions of original problem space");
            return Err(Retcode::InvalidData);
        }
        SolOrigin::Zero => {}
        SolOrigin::LpSol => {
            let solval = var::var_get_lp_sol(scip, var);
            sol_set_array_val(scip, sol, var, solval);
        }
        SolOrigin::NlpSol => {
            let solval = var::var_get_nlp_sol(scip, var);
            sol_set_array_val(scip, sol, var, solval);
        }
        SolOrigin::RelaxSol => {
            let solval = var::var_get_relax_sol_trans_var(scip, var);
            sol_set_array_val(scip, sol, var, solval);
        }
        SolOrigin::PseudoSol => {
            let solval = var::var_get_pseudo_sol(scip, var);
            sol_set_array_val(scip, sol, var, solval);
        }
        SolOrigin::Partial | SolOrigin::Unknown => sol_set_array_val(scip, sol, var, UNKNOWN),
    }
    Ok(())
}

/// `solStamp`: sets the solution time, nodenum, runnum, and depth stamp to
/// the current values; `withtree` is false where the C passes no tree.
fn sol_stamp(scip: &mut Scip, sol: SolId, withtree: bool, checktime: bool) {
    let time = if checktime {
        scip.stat.solvingtime.get_time()
    } else {
        scip.stat.solvingtime.get_last_time()
    };
    let depth = if !withtree || scip.tree.is_none() {
        -1
    } else {
        crate::tree::tree_get_current_depth(scip)
    };
    let s = &mut scip.sols[sol];
    s.time = time;
    s.nodenum = scip.stat.nnodes;
    s.runnum = scip.stat.nruns;
    s.depth = depth;
}

/// A fresh solution of the given origin and objective (the shared body of
/// the `SCIPsolCreate...` functions); stamps it and registers it with
/// `primal`.
fn sol_new(scip: &mut Scip, primal: usize, withtree: bool, heur: HeurId, solorigin: SolOrigin, obj: f64) -> ScipResult<SolId> {
    let sol = scip.sols.push(Sol {
        obj,
        time: 0.0,
        nodenum: 0,
        vals: RealArray::new(),
        valid: BoolArray::new(),
        creator: Creator::default(),
        viol: Viol::default(),
        runnum: 0,
        depth: 0,
        primalindex: -1,
        index: scip.stat.solindex,
        solorigin,
        hasinfval: false,
        type_: SolType::Unknown,
    });
    scip.stat.solindex += 1;
    sol_stamp(scip, sol, withtree, true);
    sol_reset_violations(scip, sol);

    // Set solution type and creator depending on whether a heuristic or NULL
    // is passed.
    sol_set_heur(scip, sol, heur);

    crate::primal::primal_sol_created(scip, primal, sol);
    Ok(sol)
}

/// `SCIPsolCreate`: creates primal CIP solution, initialized to zero.
pub fn sol_create(scip: &mut Scip, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    sol_new(scip, primal, true, heur, SolOrigin::Zero, 0.0)
}

/// `SCIPsolCreateOriginal`: creates primal CIP solution in original
/// problem space, initialized to the offset in the original problem.
pub fn sol_create_original(scip: &mut Scip, origprob: usize, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    let obj = scip.probs[origprob].objoffset;
    sol_new(scip, primal, true, heur, SolOrigin::Original, obj)
}

/// `SCIPsolCopy`: creates a copy of a primal CIP solution.
pub fn sol_copy(scip: &mut Scip, primal: usize, sourcesol: SolId) -> ScipResult<SolId> {
    let src = scip.sols[sourcesol].clone();
    let index = scip.stat.solindex;
    let sol = scip.sols.push(Sol {
        vals: src.vals.clone(),
        valid: src.valid.clone(),
        creator: Creator::default(),
        type_: SolType::Unknown,
        obj: src.obj,
        primalindex: -1,
        time: src.time,
        nodenum: src.nodenum,
        solorigin: src.solorigin,
        runnum: src.runnum,
        depth: src.depth,
        index,
        hasinfval: src.hasinfval,
        viol: src.viol,
    });

    // Copy solution type and creator information.
    match src.type_ {
        SolType::Unknown | SolType::LpRelax | SolType::StrongBranch | SolType::Pseudo => scip.sols[sol].type_ = src.type_,
        SolType::Heur => sol_set_heur(scip, sol, src.creator.heur),
        SolType::Relax => sol_set_relax(scip, sol, src.creator.relax),
    }
    scip.stat.solindex += 1;

    crate::primal::primal_sol_created(scip, primal, sol);
    Ok(sol)
}

/// `SCIPsolTransform`: transformes given original solution to the
/// transformed space; a corresponding transformed solution has to be given
/// which is copied into the existing solution and freed afterwards.
pub fn sol_transform(scip: &mut Scip, sol: SolId, transsol: SolId, primal: usize) -> ScipResult {
    // Switch vals and valid arrays; the exisiting solution gets the arrays of
    // the transformed solution; the transformed one gets the original
    // arrays, because they have to be freed anyway and freeing the transsol
    // automatically frees its arrays.
    let mut tvals = std::mem::take(&mut scip.sols[transsol].vals);
    let mut tvalid = std::mem::take(&mut scip.sols[transsol].valid);
    std::mem::swap(&mut scip.sols[sol].vals, &mut tvals);
    std::mem::swap(&mut scip.sols[sol].valid, &mut tvalid);
    scip.sols[transsol].vals = tvals;
    scip.sols[transsol].valid = tvalid;

    // Copy solorigin and objective (should be the same, only to avoid
    // numerical issues); we keep the other statistics of the original
    // solution, since that was the first time that this solution as found.
    scip.sols[sol].solorigin = scip.sols[transsol].solorigin;
    scip.sols[sol].obj = scip.sols[transsol].obj;

    sol_free(scip, transsol, primal)
}

/// `SCIPsolAdjustImplicitSolVals`: adjusts solution values of implied
/// integral variables in handed solution, solution objective value is not
/// deteriorated by this method.
pub fn sol_adjust_implicit_sol_vals(scip: &mut Scip, sol: SolId, prob: usize, uselprows: bool) -> ScipResult {
    // Get number of implied integral variables.
    let p = &scip.probs[prob];
    let mut nimplvarsend = p.nbinimplvars + p.nintimplvars + p.ncontimplvars;

    if nimplvarsend == 0 {
        return Ok(());
    }

    // Get range of implied integral variables.
    let nimplvarsbegin = p.nbinvars + p.nintvars;
    nimplvarsend += nimplvarsbegin;

    // Loop over implied integral variables and round them up or down.
    for v in nimplvarsbegin..nimplvarsend {
        let var = scip.probs[prob].vars[v];
        let solval = sol_get_val(scip, sol, var);

        // We do not need to round integral solution values or those of
        // variables which are not column variables.
        if scip.set.is_feas_integral(solval) || scip.vars[var].varstatus != VarStatus::Column {
            continue;
        }

        let nuplocks = var::var_get_n_locks_up_type(scip, var, LockType::Model);
        let ndownlocks = var::var_get_n_locks_down_type(scip, var, LockType::Model);
        let obj = scip.vars[var].unchangedobj;

        let mut roundup = false;
        let mut rounddown = false;

        // In case of a non-zero objective coefficient, there is only one
        // possible rounding direction.
        if scip.set.is_feas_negative(obj) {
            roundup = true;
        } else if scip.set.is_feas_positive(obj) {
            rounddown = true;
        } else if uselprows {
            // Determine rounding direction based on row violations.
            let col = scip.vars[var].data.col;
            let (rows, vals) = {
                let c = &scip.cols[col];
                (c.rows.head(c.len).to_vec(), c.vals.head(c.len).to_vec())
            };

            // Loop over rows and search for equations whose violation can be
            // decreased by rounding.
            let mut r = 0;
            while r < rows.len() && !(roundup && rounddown) {
                let row = rows[r];
                let rr = &scip.rows[row];
                if rr.local || rr.lppos < 0 {
                    r += 1;
                    continue;
                }

                let rhs = rr.rhs;
                let lhs = rr.lhs;

                if scip.set.is_infinity(rhs) || scip.set.is_infinity(-lhs) {
                    r += 1;
                    continue;
                }

                let activity = lp::row_get_sol_activity(scip, row, sol);

                if scip.set.is_feas_le(activity, rhs) && scip.set.is_feas_le(lhs, activity) {
                    r += 1;
                    continue;
                }

                if (scip.set.is_feas_gt(activity, rhs) && scip.set.is_positive(vals[r])) || (scip.set.is_feas_lt(activity, lhs) && scip.set.is_negative(vals[r])) {
                    rounddown = true;
                } else {
                    roundup = true;
                }
                r += 1;
            }
        }

        // In case of a tie, we select the rounding step based on the number of
        // variable locks.
        if roundup == rounddown {
            rounddown = ndownlocks <= nuplocks;
            roundup = !rounddown;
        }

        // Round the variable up or down.
        let newsolval = if roundup { scip.set.ceil(solval) } else { scip.set.floor(solval) };

        sol_set_val(scip, sol, var, newsolval)?;
    }
    Ok(())
}

/// `SCIPsolCreateLPSol`: creates primal CIP solution, initialized to the
/// current LP solution.
pub fn sol_create_lp_sol(scip: &mut Scip, prob: usize, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    let sol = sol_create(scip, primal, heur)?;
    sol_link_lp_sol(scip, sol, prob)?;
    Ok(sol)
}

/// `SCIPsolCreateRelaxSol`: creates primal CIP solution, initialized to
/// the current relaxation solution.
pub fn sol_create_relax_sol(scip: &mut Scip, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    let sol = sol_create(scip, primal, heur)?;
    sol_link_relax_sol(scip, sol);

    // Update solution type and store relaxator as creator only if no
    // heuristic is specified as creator.
    if heur == NONE {
        let relax = crate::relax::relaxation_get_sol_relax(scip);
        sol_set_relax(scip, sol, relax);
    }
    Ok(sol)
}

/// `SCIPsolCreatePseudoSol`: creates primal CIP solution, initialized to
/// the current pseudo solution.
pub fn sol_create_pseudo_sol(scip: &mut Scip, prob: usize, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    let sol = sol_create(scip, primal, heur)?;
    sol_link_pseudo_sol(scip, sol, prob);

    // Update solution type to pseudo solution.
    if heur == NONE {
        sol_set_pseudo(scip, sol);
    }
    Ok(sol)
}

/// `SCIPsolCreateCurrentSol`: creates primal CIP solution, initialized to
/// the current solution.
pub fn sol_create_current_sol(scip: &mut Scip, prob: usize, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    if crate::tree::tree_has_current_node_lp(scip) {
        sol_create_lp_sol(scip, prob, primal, heur)
    } else {
        sol_create_pseudo_sol(scip, prob, primal, heur)
    }
}

/// `SCIPsolCreatePartial`: creates partial primal CIP solution,
/// initialized to unknown values.
pub fn sol_create_partial(scip: &mut Scip, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    sol_new(scip, primal, false, heur, SolOrigin::Partial, UNKNOWN)
}

/// `SCIPsolCreateUnknown`: creates primal CIP solution, initialized to
/// unknown values.
pub fn sol_create_unknown(scip: &mut Scip, primal: usize, heur: HeurId) -> ScipResult<SolId> {
    sol_new(scip, primal, true, heur, SolOrigin::Unknown, 0.0)
}

/// `SCIPsolFree`: frees primal CIP solution.
pub fn sol_free(scip: &mut Scip, sol: SolId, primal: usize) -> ScipResult {
    crate::primal::primal_sol_freed(scip, primal, sol);
    scip.sols.remove(sol);
    Ok(())
}

/// `SCIPsolLinkLPSol`: copies current LP solution into CIP solution by
/// linking.
pub fn sol_link_lp_sol(scip: &mut Scip, sol: SolId, prob: usize) -> ScipResult {
    // Clear the old solution arrays.
    sol_clear_arrays(scip, sol);

    // Link solution to LP solution.
    if lp::lp_diving_obj_changed(scip) {
        // The objective value has to be calculated manually, because the
        // LP's value is invalid; use objective values of variables, because
        // columns objective values are changed to dive values.
        let mut obj = lp::lp_get_loose_objval(scip, prob);
        if !scip.set.is_infinity(-obj) {
            let lp = scip.lp.as_ref().unwrap();
            for c in 0..lp.ncols {
                let col = &scip.cols[lp.cols[c]];
                obj += scip.vars[col.var].unchangedobj * col.primsol;
            }
        }
        scip.sols[sol].obj = obj;
    } else {
        // The objective value in the columns is correct, s.t. the LP's
        // objective value is also correct.
        scip.sols[sol].obj = lp::lp_get_objval(scip, prob);
    }
    scip.sols[sol].solorigin = SolOrigin::LpSol;
    sol_stamp(scip, sol, true, true);
    Ok(())
}

/// `SCIPsolLinkRelaxSol`: copies current relaxation solution into CIP
/// solution by linking.
pub fn sol_link_relax_sol(scip: &mut Scip, sol: SolId) {
    // Clear the old solution arrays.
    sol_clear_arrays(scip, sol);

    // The objective value in the columns is correct, s.t. the LP's objective
    // value is also correct.
    scip.sols[sol].obj = crate::relax::relaxation_get_sol_obj(scip);
    scip.sols[sol].solorigin = SolOrigin::RelaxSol;
    sol_stamp(scip, sol, true, true);
}

/// `SCIPsolLinkPseudoSol`: copies current pseudo solution into CIP
/// solution by linking.
pub fn sol_link_pseudo_sol(scip: &mut Scip, sol: SolId, prob: usize) {
    // Clear the old solution arrays.
    sol_clear_arrays(scip, sol);

    // Link solution to pseudo solution.
    scip.sols[sol].obj = lp::lp_get_pseudo_objval(scip, prob);
    scip.sols[sol].solorigin = SolOrigin::PseudoSol;
    sol_stamp(scip, sol, true, true);
}

/// `SCIPsolLinkCurrentSol`: copies current solution (LP or pseudo
/// solution) into CIP solution by linking.
pub fn sol_link_current_sol(scip: &mut Scip, sol: SolId, prob: usize) -> ScipResult {
    if crate::tree::tree_has_current_node_lp(scip) && lp::lp_is_solved(scip) {
        sol_link_lp_sol(scip, sol, prob)
    } else {
        sol_link_pseudo_sol(scip, sol, prob);
        Ok(())
    }
}

/// `SCIPsolClear`: clears primal CIP solution.
pub fn sol_clear(scip: &mut Scip, sol: SolId) {
    sol_clear_arrays(scip, sol);
    scip.sols[sol].solorigin = SolOrigin::Zero;
    scip.sols[sol].obj = 0.0;
    sol_stamp(scip, sol, true, true);
}

/// `SCIPsolSetUnknown`: declares all entries in the primal CIP solution to
/// be unknown.
pub fn sol_set_unknown(scip: &mut Scip, sol: SolId) {
    sol_clear_arrays(scip, sol);
    scip.sols[sol].solorigin = SolOrigin::Unknown;
    scip.sols[sol].obj = 0.0;
    sol_stamp(scip, sol, true, true);
}

/// `SCIPsolUnlink`: stores solution values of variables in solution's own
/// array.
pub fn sol_unlink(scip: &mut Scip, sol: SolId, prob: usize) -> ScipResult {
    let origin = scip.sols[sol].solorigin;
    if !sol_is_original(scip, sol) && origin != SolOrigin::Zero && origin != SolOrigin::Unknown {
        for v in 0..scip.probs[prob].nvars {
            let var = scip.probs[prob].vars[v];
            sol_unlink_var(scip, sol, var)?;
        }

        scip.sols[sol].solorigin = SolOrigin::Zero;
    }
    Ok(())
}

/// The objective update `SCIPsolSetVal` makes when a value changes from
/// `oldval` to `val`.
fn sol_update_obj_for_val(scip: &mut Scip, sol: SolId, var: VarId, oldval: f64, val: f64) {
    use crate::def::{INVALID, max};
    let set = &scip.set;
    let infinity = set.infinity();

    // An unknown solution value does not count towards the objective.
    let obj = scip.vars[var].unchangedobj;
    let oldobjcont = if oldval == UNKNOWN { 0.0 } else { obj * oldval };
    let newobjcont = if val == UNKNOWN { 0.0 } else { obj * val };
    let s = &mut scip.sols[sol];

    // We want to use a safe invalid if the contribution exchange contradicts
    // the infinity status of the objective value.
    if set.is_infinity(s.obj) {
        if (set.is_infinity(oldobjcont) && !set.is_infinity(newobjcont)) || (!set.is_infinity(-oldobjcont) && set.is_infinity(-newobjcont)) {
            s.obj = INVALID;
        }
    } else if set.is_infinity(-s.obj) {
        if (set.is_infinity(-oldobjcont) && !set.is_infinity(-newobjcont)) || (!set.is_infinity(oldobjcont) && set.is_infinity(newobjcont)) {
            s.obj = INVALID;
        }
    }
    // We want to use a clean infinity if the contribution exchange or the
    // resulting objective hits the infinity bound.
    else if !set.is_infinity(max(oldobjcont.abs(), newobjcont.abs())) {
        s.obj -= oldobjcont;
        s.obj += newobjcont;

        if set.is_infinity(s.obj) {
            s.obj = infinity;
        } else if set.is_infinity(-s.obj) {
            s.obj = -infinity;
        }
    } else if !set.is_infinity(max(oldobjcont, -newobjcont)) {
        s.obj = infinity;
    } else if !set.is_infinity(max(-oldobjcont, newobjcont)) {
        s.obj = -infinity;
    }
}

/// `SCIPsolSetVal`: sets value of variable in primal CIP solution.
pub fn sol_set_val(scip: &mut Scip, sol: SolId, var: VarId, val: f64) -> ScipResult {
    // We want to store only values for non fixed variables (LOOSE or
    // COLUMN); others have to be transformed.
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if sol_is_original(scip, sol) {
                let oldval = sol_get_array_val(scip, sol, var);
                if val != oldval {
                    sol_set_array_val(scip, sol, var, val);

                    // Update the objective value; we do not need to do this
                    // for invalid objectives or partial solutions.
                    if scip.sols[sol].obj != crate::def::INVALID && !sol_is_partial(scip, sol) {
                        sol_update_obj_for_val(scip, sol, var, oldval, val);
                    }

                    sol_stamp(scip, sol, true, false);
                }
                Ok(())
            } else {
                let transvar = v.data.original.transvar;
                sol_set_val(scip, sol, transvar, val)
            }
        }
        VarStatus::Loose | VarStatus::Column => {
            let oldval = sol_get_array_val(scip, sol, var);
            if val != oldval {
                sol_set_array_val(scip, sol, var, val);

                // Update the objective value; we do not need to do this for
                // invalid objectives.
                if scip.sols[sol].obj != crate::def::INVALID {
                    sol_update_obj_for_val(scip, sol, var, oldval, val);
                }

                sol_stamp(scip, sol, true, false);
            }
            Ok(())
        }
        VarStatus::Fixed => {
            let oldval = v.glbdom.lb;
            if val != oldval {
                log::error!("cannot set solution value for variable <{}> fixed to {:.15} to different value {:.15}", v.name, oldval, val);
                return Err(Retcode::InvalidData);
            }
            Ok(())
        }
        VarStatus::Aggregated => {
            // x = a*y + c  =>  y = (x-c)/a
            let a = v.data.aggregate;
            if val == UNKNOWN {
                return sol_set_val(scip, sol, a.var, val);
            }
            if scip.set.is_infinity(val) || scip.set.is_infinity(-val) {
                sol_set_val(scip, sol, a.var, if a.scalar > 0.0 { val } else { -val })
            } else {
                sol_set_val(scip, sol, a.var, (val - a.constant) / a.scalar)
            }
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            if m.nvars == 1 {
                let (mvar, mscalar, multaggrconstant) = (m.vars[0_usize], m.scalars[0_usize], m.constant);

                if scip.set.is_infinity(multaggrconstant) || scip.set.is_infinity(-multaggrconstant) {
                    if (scip.set.is_infinity(multaggrconstant) && !scip.set.is_infinity(val)) || (scip.set.is_infinity(-multaggrconstant) && !scip.set.is_infinity(-val)) {
                        log::error!(
                            "cannot set solution value for variable <{}> fixed to {:.15} to different value {:.15}",
                            v.name,
                            multaggrconstant,
                            val
                        );
                        return Err(Retcode::InvalidData);
                    }
                    return Ok(());
                } else if scip.set.is_infinity(val) || scip.set.is_infinity(-val) {
                    return sol_set_val(scip, sol, mvar, if mscalar > 0.0 { val } else { -val });
                } else {
                    return sol_set_val(scip, sol, mvar, (val - multaggrconstant) / mscalar);
                }
            }
            log::error!("cannot set solution value for multiple aggregated variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Negated => {
            let (negvar, negconst) = (v.negatedvar, v.data.negate.constant);
            if val == UNKNOWN {
                sol_set_val(scip, sol, negvar, val)
            } else if scip.set.is_infinity(val) || scip.set.is_infinity(-val) {
                sol_set_val(scip, sol, negvar, -val)
            } else {
                sol_set_val(scip, sol, negvar, negconst - val)
            }
        }
    }
}

/// `SCIPsolIncVal`: increases value of variable in primal CIP solution.
pub fn sol_inc_val(scip: &mut Scip, sol: SolId, var: VarId, incval: f64) -> ScipResult {
    if incval == 0.0 {
        return Ok(());
    }

    let oldval = sol_get_array_val(scip, sol, var);
    if scip.set.is_infinity(oldval) || scip.set.is_infinity(-oldval) {
        return Ok(());
    }

    // We want to store only values for non fixed variables (LOOSE or
    // COLUMN); others have to be transformed.
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if sol_is_original(scip, sol) {
                let obj = v.unchangedobj;
                sol_inc_array_val(scip, sol, var, incval);
                scip.sols[sol].obj += obj * incval;
                sol_stamp(scip, sol, true, false);
                Ok(())
            } else {
                let transvar = v.data.original.transvar;
                sol_inc_val(scip, sol, transvar, incval)
            }
        }
        VarStatus::Loose | VarStatus::Column => {
            let obj = v.unchangedobj;
            sol_inc_array_val(scip, sol, var, incval);
            scip.sols[sol].obj += obj * incval;
            sol_stamp(scip, sol, true, false);
            Ok(())
        }
        VarStatus::Fixed => {
            log::error!("cannot increase solution value for fixed variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Aggregated => {
            // x = a*y + c  =>  y = (x-c)/a
            let a = v.data.aggregate;
            sol_inc_val(scip, sol, a.var, incval / a.scalar)
        }
        VarStatus::MultAggr => {
            log::error!("cannot increase solution value for multiple aggregated variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Negated => {
            let negvar = v.negatedvar;
            sol_inc_val(scip, sol, negvar, -incval)
        }
    }
}

/// `SCIPsolGetVal`: returns value of variable in primal CIP solution.
pub fn sol_get_val(scip: &Scip, sol: SolId, var: VarId) -> f64 {
    use crate::def::INVALID;

    // If the value of a transformed variable in an original solution is
    // requested, we need to project the variable back to the original space,
    // the opposite case is handled below.
    if sol_is_original(scip, sol) && scip.vars[var].varstatus != VarStatus::Original {
        // We cannot get the value of a transformed variable for a solution
        // that lives in the original problem space -> get the corresponding
        // original variable first.
        let mut origvar = var;
        let mut scalar = 1.0;
        let mut constant = 0.0;
        if var::var_get_origvar_sum(scip, &mut origvar, &mut scalar, &mut constant).is_err() {
            return INVALID;
        }
        if origvar == NONE {
            // The variable has no original counterpart: in the original
            // solution, it has a value of zero.
            return 0.0;
        }

        let solval = sol_get_val(scip, sol, origvar);
        return if solval == UNKNOWN { UNKNOWN } else { scalar * solval + constant };
    }

    // Only values for non fixed variables (LOOSE or COLUMN) are stored;
    // others have to be transformed.
    let set = &scip.set;
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if sol_is_original(scip, sol) {
                sol_get_array_val(scip, sol, var)
            } else {
                sol_get_val(scip, sol, v.data.original.transvar)
            }
        }
        VarStatus::Loose | VarStatus::Column => sol_get_array_val(scip, sol, var),
        VarStatus::Fixed => v.glbdom.lb,
        VarStatus::Aggregated => {
            // x = a*y + c  =>  y = (x-c)/a
            let a = v.data.aggregate;
            let solval = sol_get_val(scip, sol, a.var);
            if solval == UNKNOWN {
                return UNKNOWN;
            }
            if set.is_infinity(solval) || set.is_infinity(-solval) {
                if a.scalar * solval > 0.0 {
                    return set.infinity();
                }
                if a.scalar * solval < 0.0 {
                    return -set.infinity();
                }
            }
            a.scalar * solval + a.constant
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut solvalsum = m.constant;
            for i in 0..m.nvars {
                let solval = sol_get_val(scip, sol, m.vars[i]);
                if solval == UNKNOWN {
                    return UNKNOWN;
                }
                if set.is_infinity(solval) || set.is_infinity(-solval) {
                    if m.scalars[i] * solval > 0.0 {
                        return set.infinity();
                    }
                    if m.scalars[i] * solval < 0.0 {
                        return -set.infinity();
                    }
                }
                solvalsum += m.scalars[i] * solval;
            }
            solvalsum
        }
        VarStatus::Negated => {
            let solval = sol_get_val(scip, sol, v.negatedvar);
            if solval == UNKNOWN {
                return UNKNOWN;
            }
            if set.is_infinity(solval) {
                return -set.infinity();
            }
            if set.is_infinity(-solval) {
                return set.infinity();
            }
            v.data.negate.constant - solval
        }
    }
}

/// `SCIPsolGetRayVal`: returns value of variable in primal ray represented
/// by primal CIP solution.
pub fn sol_get_ray_val(scip: &Scip, sol: SolId, var: VarId) -> f64 {
    // Only values for non fixed variables (LOOSE or COLUMN) are stored;
    // others have to be transformed.
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => sol_get_ray_val(scip, sol, v.data.original.transvar),
        VarStatus::Loose | VarStatus::Column => sol_get_array_val(scip, sol, var),
        // Constants are ignored for computing the ray direction.
        VarStatus::Fixed => 0.0,
        VarStatus::Aggregated => {
            // x = a*y + c  =>  y = (x-c)/a
            let solval = sol_get_ray_val(scip, sol, v.data.aggregate.var);
            v.data.aggregate.scalar * solval
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut solvalsum = 0.0;
            for i in 0..m.nvars {
                let solval = sol_get_ray_val(scip, sol, m.vars[i]);
                solvalsum += m.scalars[i] * solval;
            }
            solvalsum
        }
        VarStatus::Negated => -sol_get_ray_val(scip, sol, v.negatedvar),
    }
}

/// `SCIPsolGetObj`: gets objective value of primal CIP solution in
/// transformed problem.
pub fn sol_get_obj(scip: &Scip, sol: SolId, transprob: usize, origprob: usize) -> f64 {
    // For original solutions, sol->obj contains the external objective
    // value.
    if sol_is_original(scip, sol) {
        crate::prob::prob_intern_objval(scip, transprob, origprob, scip.sols[sol].obj)
    } else {
        scip.sols[sol].obj
    }
}

/// `SCIPsolUpdateVarObj`: updates primal solutions after a change in a
/// variable's objective value.
pub fn sol_update_var_obj(scip: &mut Scip, sol: SolId, var: VarId, oldobj: f64, newobj: f64) {
    let solval = sol_get_array_val(scip, sol, var);
    if solval != UNKNOWN {
        scip.sols[sol].obj += (newobj - oldobj) * solval;
    }
}

/// `SCIPsolMarkPartial`: mark the given solution as partial solution.
pub fn sol_mark_partial(scip: &mut Scip, sol: SolId, vars: &[VarId]) -> ScipResult {
    if vars.is_empty() {
        return Ok(());
    }

    // Get values.
    let vals: Vec<f64> = vars.iter().map(|&v| sol_get_val(scip, sol, v)).collect();

    // Change origin to partial.
    scip.sols[sol].solorigin = SolOrigin::Partial;

    // Set values.
    for (v, &var) in vars.iter().enumerate() {
        let idx = scip.vars[var].index;

        if vals[v] != UNKNOWN {
            // From now on, variable must not be deleted.
            var::var_mark_not_deletable(scip, var);

            // Mark the variable valid.
            scip.sols[sol].valid.set_val(idx, true);

            // Set the value in the solution array.
            scip.sols[sol].vals.set_val(idx, vals[v]);
        } else {
            // Mark the variable invalid.
            scip.sols[sol].valid.set_val(idx, false);
        }
    }
    Ok(())
}

/// `SCIPsolCheckOrig`: checks solution for feasibility in original problem
/// without adding it to the solution store. We first check the variable
/// bounds. Then we loop over all constraint handlers and constraints,
/// checking each in the order of their check priority.
#[allow(clippy::too_many_arguments)]
pub fn sol_check_orig(
    scip: &mut Scip,
    sol: SolId,
    prob: usize,
    primal: usize,
    printreason: bool,
    completely: bool,
    checkbounds: bool,
    checkintegrality: bool,
    checklprows: bool,
    checkmodifiable: bool,
) -> ScipResult<bool> {
    use crate::types::ScipResultCode;

    let mut feasible = true;

    sol_reset_violations(scip, sol);

    let completely = completely && printreason;

    // Check bounds.
    if checkbounds {
        for v in 0..scip.probs[prob].nvars {
            let var = scip.probs[prob].vars[v];
            let solval = sol_get_val(scip, sol, var);

            let lb = var::var_get_lb_original(scip, var);
            let ub = var::var_get_ub_original(scip, var);

            if scip.primals[primal].updateviolations {
                sol_update_bound_violation(scip, sol, lb - solval, crate::misc::rel_diff(lb, solval));
                sol_update_bound_violation(scip, sol, solval - ub, crate::misc::rel_diff(solval, ub));
            }

            if scip.set.is_feas_lt(solval, lb) || scip.set.is_feas_gt(solval, ub) {
                feasible = false;

                if printreason {
                    log::info!(
                        "solution violates original bounds of variable <{}> [{},{}] solution value <{}>",
                        scip.vars[var].name,
                        lb,
                        ub,
                        solval
                    );
                }

                if !completely {
                    return Ok(feasible);
                }
            }
        }
    }

    // Sort original constraint according to check priority.
    crate::prob::prob_sort_conss_check(scip, prob);

    // Check original constraints.
    //
    // In general modifiable constraints can not be checked, because the
    // variables to fulfill them might be missing in the original problem;
    // however, if the solution comes from a heuristic during presolving,
    // modifiable constraints have to be checked.
    let mut h = 0;
    for c in 0..scip.probs[prob].nconss {
        let cons = scip.probs[prob].origcheckconss[c];
        let priority = scip.conshdlrs[scip.conss[cons].conshdlr].checkpriority;

        // Check constraints handlers without constraints that have a check
        // priority at least as high as current constraint.
        while h < scip.set.nconshdlrs && scip.conshdlrs[scip.set.conshdlrs[h]].checkpriority >= priority {
            let hd = scip.set.conshdlrs[h];
            if !scip.conshdlrs[hd].needscons {
                let result = crate::cons::conshdlr_check(scip, hd, sol, checkintegrality, checklprows, printreason, completely)?;

                if result != ScipResultCode::Feasible {
                    feasible = false;

                    if !completely {
                        return Ok(feasible);
                    }
                }
            }
            h += 1;
        }

        // Now check constraint.
        if scip.conss[cons].check && (checkmodifiable || !scip.conss[cons].modifiable) {
            // Check solution.
            let result = crate::cons::cons_check(scip, cons, sol, checkintegrality, checklprows, printreason)?;

            if result != ScipResultCode::Feasible {
                feasible = false;

                if !completely {
                    return Ok(feasible);
                }
            }
        }
    }

    // One final loop over the remaining constraints handlers without
    // constraints.
    while h < scip.set.nconshdlrs {
        let hd = scip.set.conshdlrs[h];
        if !scip.conshdlrs[hd].needscons {
            let result = crate::cons::conshdlr_check(scip, hd, sol, checkintegrality, checklprows, printreason, completely)?;

            if result != ScipResultCode::Feasible {
                feasible = false;

                if !completely {
                    return Ok(feasible);
                }
            }
        }
        h += 1;
    }
    Ok(feasible)
}

/// `SCIPsolCheck`: checks primal CIP solution for feasibility.
///
/// The difference between SCIPsolCheck() and SCIPcheckSolOrig() is that
/// modifiable constraints are handled differently. There might be some
/// variables which do not have an original counter part (e.g. in
/// branch-and-price). Therefore, modifiable constraints can not be
/// double-checked in the original space.
#[allow(clippy::too_many_arguments)]
pub fn sol_check(scip: &mut Scip, sol: SolId, prob: usize, printreason: bool, completely: bool, checkbounds: bool, checkintegrality: bool, checklprows: bool) -> ScipResult<bool> {
    use crate::types::ScipResultCode;

    let mut feasible = true;

    sol_reset_violations(scip, sol);

    let completely = completely && printreason;

    // Check whether the solution respects the global bounds of the
    // variables.
    if checkbounds || scip.sols[sol].hasinfval {
        let mut v = 0;
        while v < scip.probs[prob].nvars && (feasible || completely) {
            let var = scip.probs[prob].vars[v];
            v += 1;
            let solval = sol_get_val(scip, sol, var);

            if solval != UNKNOWN {
                let set = &scip.set;
                let lb = scip.vars[var].glbdom.lb;
                let ub = scip.vars[var].glbdom.ub;

                // If we have to check bound and one of the current bounds is
                // violated.
                if checkbounds && ((!set.is_infinity(-lb) && set.is_feas_lt(solval, lb)) || (!set.is_infinity(ub) && set.is_feas_gt(solval, ub))) {
                    feasible = false;

                    if printreason {
                        log::info!(
                            "solution value {} violates bounds of <{}>[{},{}] by {}",
                            solval,
                            scip.vars[var].name,
                            lb,
                            ub,
                            crate::def::max(lb - solval, 0.0) + crate::def::max(solval - ub, 0.0)
                        );
                    }
                }

                // Check whether there are infinite variable values that lead
                // to an objective value of +infinity.
                if feasible && scip.sols[sol].hasinfval {
                    let obj = scip.vars[var].unchangedobj;
                    feasible = feasible && (!set.is_infinity(solval) || set.is_le(obj, 0.0));
                    feasible = feasible && (!set.is_infinity(-solval) || set.is_ge(obj, 0.0));

                    if ((set.is_infinity(solval) && set.is_gt(obj, 0.0)) || (set.is_infinity(-solval) && set.is_lt(obj, 0.0))) && printreason {
                        log::info!(
                            "infinite solution value {} for variable  <{}> with obj {} implies objective value +infinity",
                            solval,
                            scip.vars[var].name,
                            obj
                        );
                    }
                }
            }
        }
    }

    // Check whether the solution fulfills all constraints.
    let mut h = 0;
    while h < scip.set.nconshdlrs && (feasible || completely) {
        let hd = scip.set.conshdlrs[h];
        let result = crate::cons::conshdlr_check(scip, hd, sol, checkintegrality, checklprows, printreason, completely)?;
        feasible = feasible && result == ScipResultCode::Feasible;
        h += 1;
    }
    Ok(feasible)
}

/// `SCIPsolRound`: try to round given solution; returns whether rounding
/// was successful.
pub fn sol_round(scip: &mut Scip, sol: SolId, prob: usize) -> ScipResult<bool> {
    // Round all roundable fractional enforced integral variables as long as
    // no unroundable var was found.
    let p = &scip.probs[prob];
    let nvars = p.nvars - p.ncontvars - p.ncontimplvars;
    let mut v = 0;
    while v < nvars {
        let var = scip.probs[prob].vars[v];
        let mut solval = sol_get_array_val(scip, sol, var);

        // Solutions with unknown entries cannot be rounded.
        if solval == UNKNOWN {
            break;
        }

        // If solution value is already integral with feastol, continue.
        if scip.set.is_feas_integral(solval) {
            v += 1;
            continue;
        }

        // Get rounding possibilities.
        let mayrounddown = var::var_may_round_down(scip, var);
        let mayroundup = var::var_may_round_up(scip, var);

        // Choose rounding direction.
        if mayrounddown && mayroundup {
            // We can round in both directions: round in objective function
            // direction.
            if scip.vars[var].unchangedobj >= 0.0 {
                solval = scip.set.feas_floor(solval);
            } else {
                solval = scip.set.feas_ceil(solval);
            }
        } else if mayrounddown {
            solval = scip.set.feas_floor(solval);
        } else if mayroundup {
            solval = scip.set.feas_ceil(solval);
        } else {
            break;
        }

        // Store new solution value.
        sol_set_val(scip, sol, var, solval)?;
        v += 1;
    }

    // Check, if rounding was successful.
    Ok(v == nvars)
}

/// `SCIPsolUpdateVarsum`: updates the solution value sums in variables by
/// adding the value in the given solution.
pub fn sol_update_varsum(scip: &mut Scip, sol: SolId, prob: usize, weight: f64) {
    for v in 0..scip.probs[prob].nvars {
        let var = scip.probs[prob].vars[v];
        let solval = sol_get_val(scip, sol, var);
        if solval != UNKNOWN {
            let pv = &mut scip.vars[var];
            pv.primsolavg *= 1.0 - weight;
            pv.primsolavg += weight * solval;
        }
    }
}

/// `SCIPsolRetransform`: retransforms solution to original problem space;
/// returns whether the solution (potentially) contains an infinite value.
pub fn sol_retransform(scip: &mut Scip, sol: SolId, origprob: usize, transprob: usize) -> ScipResult<bool> {
    let mut hasinfval = false;

    // This method was a performance bottleneck when retransforming a
    // solution during presolving, before flattening the aggregation graph.
    // In that case, calling SCIPsolGetVal() on the original variable
    // consumed too much time. Therefore, we now first compute the active
    // representation of each original variable using
    // SCIPvarGetActiveRepresentatives(), which is much faster, and sum up
    // the solution values of the active variables by hand for each original
    // variable.
    let nvars = scip.probs[origprob].nvars;
    let ntransvars = scip.probs[transprob].nvars;

    // Allocate temporary memory for getting the active representation of
    // the original variables, buffering the solution values of all active
    // variables and storing the original solution values.
    let mut transsolvals = vec![0.0; ntransvars as usize + 1];
    let mut activevars: crate::cvec::CVec<VarId> = crate::cvec![NONE; ntransvars as usize + 1];
    let mut activevals: crate::cvec::CVec<f64> = crate::cvec![0.0; ntransvars as usize + 1];
    let mut solvals = vec![0.0; nvars as usize];

    // Get the solution values of all active variables.
    for v in 0..ntransvars as usize {
        let tv = scip.probs[transprob].vars[v];
        transsolvals[v] = sol_get_val(scip, sol, tv);
    }

    // Get the solution in original problem variables.
    for v in 0..nvars as usize {
        activevars[0_usize] = scip.probs[origprob].vars[v];
        activevals[0_usize] = 1.0;
        let mut nactivevars = 1;
        let mut constant = 0.0;
        let mut requiredsize = 0;

        // Get active representation of the original variable.
        var::var_get_active_representatives(scip, &mut activevars, &mut activevals, &mut nactivevars, ntransvars + 1, &mut constant, &mut requiredsize)?;

        // Compute solution value of the original variable.
        solvals[v] = constant;
        for i in 0..nactivevars {
            solvals[v] += activevals[i] * transsolvals[scip.vars[activevars[i]].probindex as usize];
        }

        if scip.set.is_infinity(solvals[v]) {
            solvals[v] = scip.set.infinity();
            hasinfval = true;
        } else if scip.set.is_infinity(-solvals[v]) {
            solvals[v] = -scip.set.infinity();
            hasinfval = true;
        }
    }

    // Clear the solution and convert it into original space.
    sol_clear_arrays(scip, sol);
    scip.sols[sol].solorigin = SolOrigin::Original;
    scip.sols[sol].obj = scip.probs[origprob].objoffset;

    // Reinsert the values of the original variables.
    for v in 0..nvars as usize {
        if solvals[v] != 0.0 {
            let var = scip.probs[origprob].vars[v];
            sol_set_array_val(scip, sol, var, solvals[v]);
            if solvals[v] != UNKNOWN {
                scip.sols[sol].obj += scip.vars[var].unchangedobj * solvals[v];
            }
        }
    }
    Ok(hasinfval)
}

/// `SCIPsolRecomputeObj`: recomputes the objective value of an original
/// solution, e.g., when transferring solutions from the solution pool
/// (objective coefficients might have changed in the meantime).
pub fn sol_recompute_obj(scip: &mut Scip, sol: SolId, origprob: usize) {
    // Recompute the objective value.
    let mut obj = scip.probs[origprob].objoffset;
    for v in 0..scip.probs[origprob].nvars {
        let var = scip.probs[origprob].vars[v];
        let solval = sol_get_val(scip, sol, var);
        if solval != 0.0 && solval != UNKNOWN {
            obj += scip.vars[var].unchangedobj * solval;
        }
    }

    if scip.set.is_infinity(-obj) {
        obj = -scip.set.infinity();
    }
    scip.sols[sol].obj = obj;
}

/// `SCIPsolsAreEqual`: returns whether the given solutions are equal;
/// `transprob` is `NONE` if both solution are defined in the original
/// problem space.
pub fn sols_are_equal(scip: &Scip, sol1: SolId, sol2: SolId, origprob: usize, transprob: usize) -> bool {
    let set = &scip.set;

    // If both solutions are original or both are transformed, take the
    // objective values stored in the solutions; one solution is original
    // and the other not, so we have to get for both the objective in the
    // transformed problem.
    let (obj1, obj2) = if sol_is_original(scip, sol1) == sol_is_original(scip, sol2) {
        (scip.sols[sol1].obj, scip.sols[sol2].obj)
    } else {
        (sol_get_obj(scip, sol1, transprob, origprob), sol_get_obj(scip, sol2, transprob, origprob))
    };

    // Solutions with different objective values cannot be the same; we
    // consider two infinite objective values with the same sign always to be
    // different.
    let infobjs = (set.is_infinity(obj1) && set.is_infinity(obj2)) || (set.is_infinity(-obj1) && set.is_infinity(-obj2));
    if !infobjs && !set.is_eq(obj1, obj2) {
        return false;
    }

    // If one of the solutions is defined in the original space, the
    // comparison has to be performed in the original space.
    let prob = if sol_is_original(scip, sol1) || sol_is_original(scip, sol2) {
        origprob
    } else {
        transprob
    };

    // Compare each variable value.
    for v in 0..scip.probs[prob].nvars {
        let var = scip.probs[prob].vars[v];
        let val1 = sol_get_val(scip, sol1, var);
        let val2 = sol_get_val(scip, sol2, var);
        if !set.is_eq(val1, val2) {
            return false;
        }
    }
    true
}

/// `SCIPsolSetOrigin`.
pub fn sol_set_origin(scip: &mut Scip, sol: SolId, origin: SolOrigin) {
    scip.sols[sol].solorigin = origin;
}

/*
 * methods for accumulated numerical violations of a solution
 */

/// `SCIPsolResetViolations`: reset violations of a solution.
pub fn sol_reset_violations(scip: &mut Scip, sol: SolId) {
    scip.sols[sol].viol = Viol::default();
}

/// `SCIPsolUpdateIntegralityViolation`.
pub fn sol_update_integrality_violation(scip: &mut Scip, sol: SolId, absviolintegrality: f64) {
    let v = &mut scip.sols[sol].viol;
    v.absviolintegrality = crate::def::max(v.absviolintegrality, absviolintegrality);
}

/// `SCIPsolUpdateBoundViolation`.
pub fn sol_update_bound_violation(scip: &mut Scip, sol: SolId, absviolbounds: f64, relviolbounds: f64) {
    let v = &mut scip.sols[sol].viol;
    v.absviolbounds = crate::def::max(v.absviolbounds, absviolbounds);
    v.relviolbounds = crate::def::max(v.relviolbounds, relviolbounds);
}

/// `SCIPsolUpdateLPRowViolation`.
pub fn sol_update_lp_row_violation(scip: &mut Scip, sol: SolId, absviollprows: f64, relviollprows: f64) {
    let v = &mut scip.sols[sol].viol;
    v.absviollprows = crate::def::max(v.absviollprows, absviollprows);
    v.relviollprows = crate::def::max(v.relviollprows, relviollprows);
}

/// `SCIPsolUpdateConsViolation`.
pub fn sol_update_cons_violation(scip: &mut Scip, sol: SolId, absviolcons: f64, relviolcons: f64) {
    let v = &mut scip.sols[sol].viol;
    v.absviolcons = crate::def::max(v.absviolcons, absviolcons);
    v.relviolcons = crate::def::max(v.relviolcons, relviolcons);
}

/// `SCIPsolUpdateLPConsViolation`: update violation of a constraint that
/// is represented in the LP.
pub fn sol_update_lp_cons_violation(scip: &mut Scip, sol: SolId, absviol: f64, relviol: f64) {
    sol_update_cons_violation(scip, sol, absviol, relviol);
    sol_update_lp_row_violation(scip, sol, absviol, relviol);
}

/// `SCIPsolComp`: comparison method for sorting solution by decreasing
/// objective value (best solution will be sorted to the end).
pub fn sol_comp(scip: &Scip, elem1: SolId, elem2: SolId) -> i32 {
    let d = scip.sols[elem1].obj - scip.sols[elem2].obj;
    if d < 0.0 {
        1
    } else if d > 0.0 {
        -1
    } else {
        0
    }
}

/*
 * simple functions implemented as defines
 */

/// `SCIPsolIsOriginal`: returns whether the given solution is defined on
/// original variables.
pub fn sol_is_original(scip: &Scip, sol: SolId) -> bool {
    matches!(scip.sols[sol].solorigin, SolOrigin::Original | SolOrigin::Partial)
}

/// `SCIPsolIsPartial`: returns whether the given solution is defined on
/// original variables and containes unknown solution values.
pub fn sol_is_partial(scip: &Scip, sol: SolId) -> bool {
    scip.sols[sol].solorigin == SolOrigin::Partial
}

/// `SCIPsolGetHeur`: gets heuristic, that found this solution or `NONE` if
/// solution has type different than heuristic.
pub fn sol_get_heur(scip: &Scip, sol: SolId) -> HeurId {
    let s = &scip.sols[sol];
    if s.type_ == SolType::Heur { s.creator.heur } else { NONE }
}

/// `SCIPsolSetHeur`: informs the solution that it now belongs to the given
/// primal heuristic. For convenience and backwards compatibility, the
/// method accepts `NONE` as input for `heur`, in which case the solution
/// type is set to LP relaxation.
pub fn sol_set_heur(scip: &mut Scip, sol: SolId, heur: HeurId) {
    if heur == NONE {
        sol_set_lp_relaxation(scip, sol);
    } else {
        let s = &mut scip.sols[sol];
        s.type_ = SolType::Heur;
        s.creator.heur = heur;
    }
}

/// `SCIPsolGetRelax`: gets relaxation handler that found this solution, or
/// `NONE` if solution has different type than relaxation.
pub fn sol_get_relax(scip: &Scip, sol: SolId) -> RelaxId {
    let s = &scip.sols[sol];
    if s.type_ == SolType::Relax { s.creator.relax } else { NONE }
}

/// `SCIPsolSetRelax`: informs the solution that it now belongs to the
/// given relaxation handler.
pub fn sol_set_relax(scip: &mut Scip, sol: SolId, relax: RelaxId) {
    let s = &mut scip.sols[sol];
    s.type_ = SolType::Relax;
    s.creator.relax = relax;
}

/// `SCIPsolSetLPRelaxation`: informs the solution that it is an LP
/// relaxation solution.
pub fn sol_set_lp_relaxation(scip: &mut Scip, sol: SolId) {
    scip.sols[sol].type_ = SolType::LpRelax;
}

/// `SCIPsolSetStrongbranching`: informs the solution that it is a solution
/// found during strong branching.
pub fn sol_set_strongbranching(scip: &mut Scip, sol: SolId) {
    scip.sols[sol].type_ = SolType::StrongBranch;
}

/// `SCIPsolSetPseudo`: informs the solution that it originates from a
/// pseudo solution.
pub fn sol_set_pseudo(scip: &mut Scip, sol: SolId) {
    scip.sols[sol].type_ = SolType::Pseudo;
}
