//! SCIP's problems (`prob.c`, `struct_prob.h`): the original and the
//! transformed problem, kept in `Scip::probs` at [`ORIGPROB`] and
//! [`TRANSPROB`].

use crate::{
    arena::Arena,
    cons::{self, Cons},
    cvec::CVec,
    def::{HASHSIZE_NAMES, HASHSIZE_NAMES_SMALL, INVALID, max, min},
    hash::{HashOps, HashTable, hash_key_val_string},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    types::{ConsId, ImplintType, LockType, ObjSense, VarId, VarStatus, VarType},
    var::{self, NONE, Var},
};

/// The id of the original problem.
pub const ORIGPROB: usize = 0;
/// The id of the transformed problem.
pub const TRANSPROB: usize = 1;

/// `SCIP_PROB`. Problem data callbacks are not ported: Improve's problems
/// carry no user problem data.
#[derive(Clone, Debug)]
pub struct Prob {
    pub objoffset: f64,
    pub objscale: f64,
    pub objlim: f64,
    pub dualbound: f64,
    pub name: String,
    pub varnames: Option<HashTable<VarId>>,
    pub vars: CVec<VarId>,
    pub fixedvars: CVec<VarId>,
    pub deletedvars: CVec<VarId>,
    pub consnames: Option<HashTable<ConsId>>,
    pub conss: CVec<ConsId>,
    pub origcheckconss: CVec<ConsId>,
    pub varssize: i32,
    pub nvars: i32,
    pub nbinvars: i32,
    pub nintvars: i32,
    pub nbinimplvars: i32,
    pub nintimplvars: i32,
    pub ncontimplvars: i32,
    pub ncontvars: i32,
    pub ncolvars: i32,
    pub fixedvarssize: i32,
    pub nfixedvars: i32,
    pub deletedvarssize: i32,
    pub ndeletedvars: i32,
    pub nobjvars: i32,
    pub consssize: i32,
    pub nconss: i32,
    pub maxnconss: i32,
    pub startnvars: i32,
    pub startnconss: i32,
    pub objsense: ObjSense,
    pub objisintegral: bool,
    pub transformed: bool,
    pub nlpenabled: bool,
    pub permuted: bool,
    pub consschecksorted: bool,
    pub conscompression: bool,
}

const OBJSCALE_MAXDNOM: i64 = 1000000;
const OBJSCALE_MAXSCALE: f64 = 1000000.0;
const OBJSCALE_MAXFINALSCALE: f64 = 1000.0;

/// The keys of the variable name table: `SCIPhashGetKeyVar`,
/// `SCIPhashKeyEqString`, `SCIPhashKeyValString`.
pub struct VarNames<'a>(pub &'a Arena<Var>);

impl<'a> HashOps<VarId> for VarNames<'a> {
    type Key = &'a str;
    fn get_key(&self, element: &VarId) -> &'a str {
        &self.0[*element].name
    }
    fn key_eq(&self, key1: &&'a str, key2: &&'a str) -> bool {
        key1 == key2
    }
    fn key_val(&self, key: &&'a str) -> u64 {
        hash_key_val_string(key)
    }
}

/// The keys of the constraint name table: `SCIPhashGetKeyCons`.
pub struct ConsNames<'a>(pub &'a Arena<Cons>);

impl<'a> HashOps<ConsId> for ConsNames<'a> {
    type Key = &'a str;
    fn get_key(&self, element: &ConsId) -> &'a str {
        &self.0[*element].name
    }
    fn key_eq(&self, key1: &&'a str, key2: &&'a str) -> bool {
        key1 == key2
    }
    fn key_val(&self, key: &&'a str) -> u64 {
        hash_key_val_string(key)
    }
}

/// `probEnsureVarsMem`.
fn prob_ensure_vars_mem(scip: &mut Scip, prob: usize, num: i32) {
    if num > scip.probs[prob].varssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.probs[prob].vars.realloc(newsize);
        scip.probs[prob].varssize = newsize;
    }
}

/// `probEnsureFixedvarsMem`.
fn prob_ensure_fixedvars_mem(scip: &mut Scip, prob: usize, num: i32) {
    if num > scip.probs[prob].fixedvarssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.probs[prob].fixedvars.realloc(newsize);
        scip.probs[prob].fixedvarssize = newsize;
    }
}

/// `probEnsureDeletedvarsMem`.
fn prob_ensure_deletedvars_mem(scip: &mut Scip, prob: usize, num: i32) {
    if num > scip.probs[prob].deletedvarssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.probs[prob].deletedvars.realloc(newsize);
        scip.probs[prob].deletedvarssize = newsize;
    }
}

/// `probEnsureConssMem`.
fn prob_ensure_conss_mem(scip: &mut Scip, prob: usize, num: i32) {
    if num > scip.probs[prob].consssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.probs[prob].conss.realloc(newsize);
        if !scip.probs[prob].origcheckconss.is_empty() {
            scip.probs[prob].origcheckconss.realloc(newsize);
        }
        scip.probs[prob].consssize = newsize;
    }
}

/// `consHasName`.
fn cons_has_name(scip: &Scip, cons: ConsId) -> bool {
    !scip.conss[cons].name.is_empty()
}

/// `varHasName`.
fn var_has_name(scip: &Scip, var: VarId) -> bool {
    !scip.vars[var].name.is_empty()
}

/// `SCIPprobCreate`: creates the problem at `prob` (`ORIGPROB` or
/// `TRANSPROB`).
pub fn prob_create(scip: &mut Scip, prob: usize, name: &str, transformed: bool) {
    let tablesize = if scip.set.misc_usesmalltables { HASHSIZE_NAMES_SMALL } else { HASHSIZE_NAMES } as i32;
    let newprob = Prob {
        objoffset: 0.0,
        objscale: 1.0,
        objlim: INVALID,
        dualbound: INVALID,
        name: name.to_owned(),
        varnames: scip.set.misc_usevartable.then(|| HashTable::new(tablesize)),
        vars: CVec::new(),
        fixedvars: CVec::new(),
        deletedvars: CVec::new(),
        consnames: scip.set.misc_useconstable.then(|| HashTable::new(tablesize)),
        conss: CVec::new(),
        origcheckconss: CVec::new(),
        varssize: 0,
        nvars: 0,
        nbinvars: 0,
        nintvars: 0,
        nbinimplvars: 0,
        nintimplvars: 0,
        ncontimplvars: 0,
        ncontvars: 0,
        ncolvars: 0,
        fixedvarssize: 0,
        nfixedvars: 0,
        deletedvarssize: 0,
        ndeletedvars: 0,
        nobjvars: 0,
        consssize: 0,
        nconss: 0,
        maxnconss: 0,
        startnvars: 0,
        startnconss: 0,
        objsense: ObjSense::Minimize,
        objisintegral: false,
        transformed,
        nlpenabled: false,
        permuted: false,
        consschecksorted: false,
        conscompression: false,
    };
    scip.probs.insert_at(prob, newprob);
}

/// `SCIPprobFree`.
pub fn prob_free(scip: &mut Scip, prob: usize) -> ScipResult {
    // Remove all constraints from the problem.
    while scip.probs[prob].nconss > 0 {
        let cons = scip.probs[prob].conss[scip.probs[prob].nconss - 1];
        prob_del_cons(scip, prob, cons)?;
    }
    if scip.probs[prob].transformed {
        // Unlock variables for all constraint handlers that don't need constraints.
        for h in 0..scip.set.nconshdlrs {
            let conshdlr = scip.set.conshdlrs[h];
            if !cons::conshdlr_needs_cons(scip, conshdlr) {
                cons::conshdlr_unlock_vars(scip, conshdlr)?;
            }
        }
    }
    scip.probs[prob].origcheckconss = CVec::new();
    scip.probs[prob].conss = CVec::new();

    // Release problem variables.
    let mut v = scip.probs[prob].nvars - 1;
    while v >= 0 {
        let pvar = scip.probs[prob].vars[v];
        if var::var_get_nuses(scip, pvar) > 1 {
            log::warn!(
                "{} variable <{}> not released when freeing SCIP problem <{}>.",
                if scip.probs[prob].transformed { "Transformed" } else { "Original" },
                scip.vars[pvar].name,
                scip.probs[prob].name
            );
        }
        var::var_remove(scip, pvar, true, false)?;
        let mut released = pvar;
        var::var_release(scip, &mut released)?;
        scip.probs[prob].vars[v] = released;
        v -= 1;
    }
    scip.probs[prob].vars = CVec::new();

    // Release fixed problem variables.
    let mut v = scip.probs[prob].nfixedvars - 1;
    while v >= 0 {
        let pvar = scip.probs[prob].fixedvars[v];
        if var::var_get_nuses(scip, pvar) > 1 {
            log::warn!(
                "{} variable <{}> not released when freeing SCIP problem <{}>.",
                if scip.probs[prob].transformed { "Transformed" } else { "Original" },
                scip.vars[pvar].name,
                scip.probs[prob].name
            );
        }
        let mut released = pvar;
        var::var_release(scip, &mut released)?;
        scip.probs[prob].fixedvars[v] = released;
        v -= 1;
    }
    scip.probs.remove(prob);
    Ok(())
}

/// `SCIPprobTransform`: transforms the original problem `source` into the
/// transformed problem `target`.
pub fn prob_transform(scip: &mut Scip, source: usize, target: usize) -> ScipResult {
    let transname = format!("t_{}", scip.probs[source].name);
    prob_create(scip, target, &transname, true);
    let objsense = scip.probs[source].objsense;
    prob_set_objsense(scip, target, objsense);

    // Transform objective limit.
    if scip.probs[source].objlim < INVALID {
        let objlim = scip.probs[source].objlim;
        prob_set_objlim(scip, target, objlim);
    }

    // Transform dual bound.
    if scip.probs[source].dualbound < INVALID {
        let dualbound = scip.probs[source].dualbound;
        prob_set_dualbound(scip, target, dualbound);
    }

    // Transform and copy all variables to the target problem.
    let nvars = scip.probs[source].nvars;
    prob_ensure_vars_mem(scip, target, nvars);
    for v in 0..nvars {
        let sourcevar = scip.probs[source].vars[v];
        let mut targetvar = NONE;
        var::var_transform(scip, sourcevar, objsense, &mut targetvar)?;
        prob_add_var(scip, target, targetvar)?;
        var::var_release(scip, &mut targetvar)?;
    }

    // Transform and copy all constraints to the target problem.
    for c in 0..scip.probs[source].nconss {
        let sourcecons = scip.probs[source].conss[c];
        let targetcons = cons::cons_transform(scip, sourcecons)?;
        prob_add_cons(scip, target, targetcons)?;
        cons::cons_release(scip, targetcons)?;
    }

    // Lock variables for all constraint handlers that don't need constraints.
    for h in 0..scip.set.nconshdlrs {
        let conshdlr = scip.set.conshdlrs[h];
        if !cons::conshdlr_needs_cons(scip, conshdlr) {
            cons::conshdlr_lock_vars(scip, conshdlr)?;
        }
    }

    // Objective value is always integral, iff original objective value is
    // always integral and shift is integral.
    scip.probs[target].objisintegral = scip.probs[source].objisintegral && scip.set.is_integral(scip.probs[target].objoffset);

    // Check whether the objective value is always integral by inspecting
    // the problem, if it is the case adjust the cutoff bound accordingly.
    prob_check_obj_integral(scip, target, source)?;

    scip.probs[target].nlpenabled = scip.probs[source].nlpenabled;
    scip.probs[target].permuted = scip.probs[source].permuted;

    // Transform the conflict pool.
    crate::conflictstore::conflictstore_transform(scip, target)?;
    Ok(())
}

/// `SCIPprobResetBounds`.
pub fn prob_reset_bounds(scip: &mut Scip, prob: usize) -> ScipResult {
    for v in 0..scip.probs[prob].nvars {
        let pvar = scip.probs[prob].vars[v];
        var::var_reset_bounds(scip, pvar)?;
    }
    Ok(())
}

/// `SCIPprobResortVars`: sorts each type block of the problem's variables
/// by index.
pub fn prob_resort_vars(scip: &mut Scip, prob: usize) {
    let p = &scip.probs[prob];
    let nvars = p.nvars;
    if nvars == 0 {
        return;
    }
    let blocks = [p.nbinvars, p.nintvars, p.nbinimplvars, p.nintimplvars, p.ncontimplvars, p.ncontvars];
    let mut vars = std::mem::take(&mut scip.probs[prob].vars);
    let mut start = 0;
    for n in blocks {
        if n > 0 {
            crate::sort::sort(&mut vars[start..start + n], (), |a, b| var::var_comp(scip, *a, *b));
        }
        start += n;
    }
    for v in 0..nvars {
        scip.vars[vars[v]].probindex = v;
    }
    scip.probs[prob].vars = vars;
}

/// `SCIPprobSortConssCheck`: sorts the original problem's constraints by
/// check priority.
pub fn prob_sort_conss_check(scip: &mut Scip, prob: usize) {
    if scip.probs[prob].consschecksorted || scip.probs[prob].transformed {
        return;
    }
    if scip.probs[prob].nconss > 0 {
        if scip.probs[prob].origcheckconss.is_empty() {
            let mut copy = scip.probs[prob].conss.clone();
            copy.realloc(scip.probs[prob].consssize);
            scip.probs[prob].origcheckconss = copy;
        }
        let n = scip.probs[prob].nconss;
        let mut checkconss = std::mem::take(&mut scip.probs[prob].origcheckconss);
        crate::sort::sort(checkconss.head_mut(n), (), |a, b| cons::cons_comp_check(scip, *a, *b));
        scip.probs[prob].origcheckconss = checkconss;
    }
    scip.probs[prob].consschecksorted = true;
}

/// `probProvidePos`: makes room in the variable array for a variable of
/// `vartype` and `impltype` and returns its position, keeping the blocks
/// binary, integer, implied binary, implied integer, implied continuous,
/// continuous.
fn prob_provide_pos(scip: &mut Scip, prob: usize, vartype: VarType, impltype: ImplintType) -> i32 {
    let p = &scip.probs[prob];
    let mut insertpos = p.nvars;
    let intstart = p.nbinvars;
    let binimplstart = intstart + p.nintvars;
    let intimplstart = binimplstart + p.nbinimplvars;
    let contimplstart = intimplstart + p.nintimplvars;
    let contstart = contimplstart + p.ncontimplvars;

    if vartype == VarType::Continuous && impltype == ImplintType::None {
        scip.probs[prob].ncontvars += 1;
        return insertpos;
    }
    if insertpos > contstart {
        let moved = scip.probs[prob].vars[contstart];
        scip.probs[prob].vars[insertpos] = moved;
        var::var_set_probindex(scip, moved, insertpos);
        insertpos = contstart;
    }
    if vartype == VarType::Continuous {
        scip.probs[prob].ncontimplvars += 1;
        return insertpos;
    }
    if insertpos > contimplstart {
        let moved = scip.probs[prob].vars[contimplstart];
        scip.probs[prob].vars[insertpos] = moved;
        var::var_set_probindex(scip, moved, insertpos);
        insertpos = contimplstart;
    }
    if vartype == VarType::Integer && impltype != ImplintType::None {
        scip.probs[prob].nintimplvars += 1;
        return insertpos;
    }
    if insertpos > intimplstart {
        let moved = scip.probs[prob].vars[intimplstart];
        scip.probs[prob].vars[insertpos] = moved;
        var::var_set_probindex(scip, moved, insertpos);
        insertpos = intimplstart;
    }
    if vartype == VarType::Binary && impltype != ImplintType::None {
        scip.probs[prob].nbinimplvars += 1;
        return insertpos;
    }
    if insertpos > binimplstart {
        let moved = scip.probs[prob].vars[binimplstart];
        scip.probs[prob].vars[insertpos] = moved;
        var::var_set_probindex(scip, moved, insertpos);
        insertpos = binimplstart;
    }
    if vartype == VarType::Integer {
        scip.probs[prob].nintvars += 1;
        return insertpos;
    }
    if insertpos > intstart {
        let moved = scip.probs[prob].vars[intstart];
        scip.probs[prob].vars[insertpos] = moved;
        var::var_set_probindex(scip, moved, insertpos);
        insertpos = intstart;
    }
    scip.probs[prob].nbinvars += 1;
    insertpos
}

/// `probInsertVar`.
fn prob_insert_var(scip: &mut Scip, prob: usize, var: VarId) {
    let vartype = scip.vars[var].vartype;
    let impltype = scip.vars[var].varimpltype;
    let insertpos = prob_provide_pos(scip, prob, vartype, impltype);
    scip.probs[prob].vars[insertpos] = var;
    var::var_set_probindex(scip, var, insertpos);
    scip.probs[prob].nvars += 1;
    if scip.vars[var].varstatus == VarStatus::Column {
        scip.probs[prob].ncolvars += 1;
    }
}

/// `probRemoveVar`: removes the variable from the problem's array, moving
/// the last of each block into the hole.
fn prob_remove_var(scip: &mut Scip, prob: usize, var: VarId, isupgraded: bool) -> ScipResult {
    let p = &scip.probs[prob];
    let intstart = p.nbinvars;
    let binimplstart = intstart + p.nintvars;
    let intimplstart = binimplstart + p.nbinimplvars;
    let contimplstart = intimplstart + p.nintimplvars;
    let contstart = contimplstart + p.ncontimplvars;
    let vartype = scip.vars[var].vartype;
    let impltype = scip.vars[var].varimpltype;

    let p = &mut scip.probs[prob];
    if impltype != ImplintType::None {
        match vartype {
            VarType::Binary => p.nbinimplvars -= 1,
            VarType::Integer => p.nintimplvars -= 1,
            VarType::Continuous => p.ncontimplvars -= 1,
            _ => {
                log::error!("unknown variable type");
                return Err(Retcode::InvalidData);
            }
        }
    } else {
        match vartype {
            VarType::Binary => p.nbinvars -= 1,
            VarType::Integer => p.nintvars -= 1,
            VarType::Continuous => p.ncontvars -= 1,
            _ => {
                log::error!("unknown variable type");
                return Err(Retcode::InvalidData);
            }
        }
    }

    // Move last binary, last integer, last implicit, and last continuous
    // variable forward to fill the free slot.
    let mut freepos = scip.vars[var].probindex;
    for blockend in [intstart, binimplstart, intimplstart, contimplstart, contstart, scip.probs[prob].nvars] {
        if freepos < blockend - 1 {
            let moved = scip.probs[prob].vars[blockend - 1];
            scip.probs[prob].vars[freepos] = moved;
            var::var_set_probindex(scip, moved, freepos);
            freepos = blockend - 1;
        }
    }
    scip.probs[prob].nvars -= 1;
    if scip.vars[var].varstatus == VarStatus::Column {
        scip.probs[prob].ncolvars -= 1;
    }

    // Inform the variable that it is no longer in the problem; if it is a
    // transformed variable, remove it from the global structures.
    var::var_remove(scip, var, false, isupgraded)?;
    Ok(())
}

/// `SCIPprobAddVarName`.
pub fn prob_add_var_name(scip: &mut Scip, prob: usize, var: VarId) {
    if var_has_name(scip, var)
        && let Some(table) = scip.probs[prob].varnames.as_mut()
    {
        table.insert(&VarNames(&scip.vars), var);
    }
}

/// `SCIPprobRemoveVarName`.
pub fn prob_remove_var_name(scip: &mut Scip, prob: usize, var: VarId) {
    if var_has_name(scip, var)
        && let Some(table) = scip.probs[prob].varnames.as_mut()
    {
        table.remove(&VarNames(&scip.vars), &var);
    }
}

/// `SCIPprobAddVar`.
pub fn prob_add_var(scip: &mut Scip, prob: usize, var: VarId) -> ScipResult {
    // Capture variable.
    var::var_capture(scip, var);

    // Allocate additional memory.
    let n = scip.probs[prob].nvars + 1;
    prob_ensure_vars_mem(scip, prob, n);

    // Insert variable in vars array and mark it to be in problem.
    prob_insert_var(scip, prob, var);

    // Add variable's name to the variable name hash table.
    prob_add_var_name(scip, prob, var);

    // Update branching candidates and pseudo and loose objective value in
    // the LP.
    if scip.vars[var].varstatus != VarStatus::Original {
        crate::branch::branchcand_update_var(scip, var)?;
        crate::lp::lp_update_add_var(scip, var)?;
    }

    // Issue VARADDED event.
    if scip.probs[prob].transformed {
        let event = crate::event::event_create_var_added(var);
        crate::event::eventqueue_add(scip, event)?;
        let obj = scip.vars[var].obj;
        prob_update_nobj_vars(scip, prob, 0.0, obj);
    }
    Ok(())
}

/// `SCIPprobDelVar`: marks the variable deleted and queues it for
/// `SCIPprobPerformVarDeletions`.
pub fn prob_del_var(scip: &mut Scip, prob: usize, var: VarId, deleted: &mut bool) -> ScipResult {
    *deleted = false;

    // Don't remove variables that are not in the problem.
    if scip.vars[var].probindex == -1 {
        return Ok(());
    }

    // Don't remove the direct counterpart of an original variable from the
    // transformed problem, because otherwise later the transformation will
    // be wrong.
    if var::var_is_transformed_origvar(scip, var) {
        return Ok(());
    }

    // Mark variable to be deleted from the problem.
    var::var_mark_deleted(scip, var);

    if scip.probs[prob].transformed {
        let event = crate::event::event_create_var_deleted(var);
        crate::event::eventqueue_add(scip, event)?;
    }

    // Remember that the variable should be deleted.
    let n = scip.probs[prob].ndeletedvars + 1;
    prob_ensure_deletedvars_mem(scip, prob, n);
    let nd = scip.probs[prob].ndeletedvars;
    scip.probs[prob].deletedvars[nd] = var;
    scip.probs[prob].ndeletedvars += 1;
    *deleted = true;
    Ok(())
}

/// `SCIPprobPerformVarDeletions`.
pub fn prob_perform_var_deletions(scip: &mut Scip, prob: usize) -> ScipResult {
    // Delete variables from the constraints; do this only in solving stage,
    // in presolving, it is already handled by the constraint handlers.
    if scip.set.stage == Stage::Solving {
        for i in 0..scip.set.nconshdlrs {
            let conshdlr = scip.set.conshdlrs[i];
            cons::conshdlr_del_vars(scip, conshdlr)?;
        }
    }
    for i in 0..scip.probs[prob].ndeletedvars {
        let var = scip.probs[prob].deletedvars[i];

        // Don't delete a variable that has already been removed.
        if scip.vars[var].probindex >= 0 {
            // Convert column variable back into loose variable, free LP
            // column.
            if scip.vars[var].varstatus == VarStatus::Column {
                var::var_loose(scip, var, prob)?;
            }

            // Update branching candidates and pseudo and loose objective
            // value in the LP.
            if scip.vars[var].varstatus != VarStatus::Original {
                crate::lp::lp_update_del_var(scip, var)?;
                crate::branch::branchcand_remove_var(scip, var)?;
            }

            // Remove variable's name from the variable name hash table.
            prob_remove_var_name(scip, prob, var);

            // Remove variable from vars array and mark it to be not in
            // problem.
            prob_remove_var(scip, prob, var, false)?;

            // Update number of variables with non-zero objective
            // coefficient.
            if scip.probs[prob].transformed {
                let obj = scip.vars[var].obj;
                prob_update_nobj_vars(scip, prob, obj, 0.0);
            }

            // Release variable.
            let mut released = var;
            var::var_release(scip, &mut released)?;
            scip.probs[prob].deletedvars[i] = released;
        }
    }
    scip.probs[prob].ndeletedvars = 0;
    Ok(())
}

/// `SCIPprobChgVarType`. `branchcand` false is C's `branchcand == NULL`.
pub fn prob_chg_var_type(scip: &mut Scip, prob: usize, var: VarId, vartype: VarType, branchcand: bool) -> ScipResult {
    if scip.vars[var].vartype == vartype {
        return Ok(());
    }
    if branchcand {
        crate::branch::branchcand_remove_var(scip, var)?;
    }
    let upgraded = vartype > scip.vars[var].vartype;
    prob_remove_var(scip, prob, var, upgraded)?;
    var::var_chg_type(scip, var, vartype)?;
    prob_insert_var(scip, prob, var);
    if branchcand {
        crate::branch::branchcand_update_var(scip, var)?;
    }
    Ok(())
}

/// `SCIPprobChgVarImplType`. `branchcand` false is C's
/// `branchcand == NULL`.
pub fn prob_chg_var_impl_type(scip: &mut Scip, prob: usize, var: VarId, impltype: ImplintType, branchcand: bool) -> ScipResult {
    if scip.vars[var].varimpltype == impltype {
        return Ok(());
    }
    if branchcand {
        crate::branch::branchcand_remove_var(scip, var)?;
    }
    let upgraded = impltype != ImplintType::None;
    prob_remove_var(scip, prob, var, upgraded)?;
    var::var_chg_impl_type(scip, var, impltype)?;
    prob_insert_var(scip, prob, var);
    if branchcand {
        crate::branch::branchcand_update_var(scip, var)?;
    }
    Ok(())
}

/// `SCIPprobVarChangedStatus`.
pub fn prob_var_changed_status(scip: &mut Scip, prob: usize, var: VarId) -> ScipResult {
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            log::error!("variables cannot switch to ORIGINAL status");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Loose => {
            scip.probs[prob].ncolvars -= 1;
        }
        VarStatus::Column => {
            scip.probs[prob].ncolvars += 1;
        }
        VarStatus::Fixed | VarStatus::Aggregated | VarStatus::MultAggr | VarStatus::Negated => {
            // Variable switched from active to fixed: remove it from the
            // vars array and put it into the fixedvars array.
            prob_remove_var(scip, prob, var, false)?;
            let n = scip.probs[prob].nfixedvars + 1;
            prob_ensure_fixedvars_mem(scip, prob, n);
            let nf = scip.probs[prob].nfixedvars;
            scip.probs[prob].fixedvars[nf] = var;
            scip.probs[prob].nfixedvars += 1;

            // Update branching candidates.
            crate::branch::branchcand_update_var(scip, var)?;
        }
    }
    Ok(())
}

/// `SCIPprobAddConsName`.
pub fn prob_add_cons_name(scip: &mut Scip, prob: usize, cons: ConsId) {
    if cons_has_name(scip, cons)
        && let Some(table) = scip.probs[prob].consnames.as_mut()
    {
        table.insert(&ConsNames(&scip.conss), cons);
    }
}

/// `SCIPprobRemoveConsName`: removes the name only if it maps to this
/// constraint.
pub fn prob_remove_cons_name(scip: &mut Scip, prob: usize, cons: ConsId) {
    if cons_has_name(scip, cons)
        && let Some(table) = scip.probs[prob].consnames.as_mut()
    {
        let ops = ConsNames(&scip.conss);
        let currentcons = table.retrieve(&ops, &scip.conss[cons].name.as_str());
        if currentcons == Some(cons) {
            table.remove(&ops, &cons);
        }
    }
}

/// `SCIPprobAddCons`.
pub fn prob_add_cons(scip: &mut Scip, prob: usize, cons: ConsId) -> ScipResult {
    // Mark the constraint as problem constraint, and remember the
    // constraint's position.
    scip.conss[cons].addconssetchg = NONE;
    scip.conss[cons].addarraypos = scip.probs[prob].nconss;

    // Add the constraint to the problem's constraint array.
    let n = scip.probs[prob].nconss + 1;
    prob_ensure_conss_mem(scip, prob, n);
    let p = &mut scip.probs[prob];
    p.conss[p.nconss] = cons;
    if !p.origcheckconss.is_empty() {
        p.origcheckconss[p.nconss] = cons;
    }
    p.nconss += 1;
    p.maxnconss = max(p.maxnconss, p.nconss);
    p.consschecksorted = false;
    scip.stat.nactiveconssadded += 1;

    // Undelete constraint, if it was globally deleted in the past.
    scip.conss[cons].deleted = false;

    // Mark constraint to be globally valid.
    cons::cons_set_local(scip, cons, false);

    // Capture constraint.
    cons::cons_capture(scip, cons);

    // Add constraint's name to the namespace.
    prob_add_cons_name(scip, prob, cons);

    // If the problem is the transformed problem, activate and lock
    // constraint.
    if scip.probs[prob].transformed {
        // Activate constraint.
        if !scip.conss[cons].active {
            let focusnode = scip.stat.nnodes <= 1;
            cons::cons_activate(scip, cons, -1, focusnode)?;
        }

        // If constraint is a check-constraint, lock roundings of
        // constraint's variables.
        if scip.conss[cons].check {
            cons::cons_add_locks(scip, cons, LockType::Model, 1, 0)?;
        }
    }
    Ok(())
}

/// `SCIPprobDelCons`.
pub fn prob_del_cons(scip: &mut Scip, prob: usize, cons: ConsId) -> ScipResult {
    // If the problem is the transformed problem, deactivate and unlock
    // constraint.
    if scip.probs[prob].transformed {
        // If constraint is a check-constraint, unlock roundings of
        // constraint's variables.
        if scip.conss[cons].check {
            cons::cons_add_locks(scip, cons, LockType::Model, -1, 0)?;
        }

        // Deactivate constraint, if it is currently active.
        if scip.conss[cons].active && !scip.conss[cons].updatedeactivate {
            cons::cons_deactivate(scip, cons)?;
        }
    }

    // Remove constraint's name from the namespace.
    prob_remove_cons_name(scip, prob, cons);

    // Remove the constraint from the problem's constraint array.
    let arraypos = scip.conss[cons].addarraypos;
    let p = &mut scip.probs[prob];
    let last = p.conss[p.nconss - 1];
    p.conss[arraypos] = last;
    scip.conss[last].addarraypos = arraypos;
    let p = &mut scip.probs[prob];
    p.nconss -= 1;
    p.consschecksorted = false;

    // Free origcheckconss array: it is recreated when needed.
    if !p.origcheckconss.is_empty() {
        p.origcheckconss = CVec::new();
    }
    scip.conss[cons].addarraypos = -1;

    // Release constraint.
    cons::cons_release(scip, cons)?;
    Ok(())
}

/// `SCIPprobMarkNConss`.
pub fn prob_mark_nconss(scip: &mut Scip, prob: usize) {
    let p = &mut scip.probs[prob];
    p.maxnconss = p.nconss;
    p.startnvars = p.nvars;
    p.startnconss = p.nconss;
}

/// `SCIPprobSetObjsense`.
pub fn prob_set_objsense(scip: &mut Scip, prob: usize, objsense: ObjSense) {
    scip.probs[prob].objsense = objsense;
}

/// `SCIPprobAddObjoffset`.
pub fn prob_add_objoffset(scip: &mut Scip, prob: usize, addval: f64) {
    scip.probs[prob].objoffset += addval;
}

/// `SCIPprobSetDualbound`.
pub fn prob_set_dualbound(scip: &mut Scip, prob: usize, dualbound: f64) {
    scip.probs[prob].dualbound = dualbound;
}

/// `SCIPprobSetObjlim`.
pub fn prob_set_objlim(scip: &mut Scip, prob: usize, objlim: f64) {
    scip.probs[prob].objlim = objlim;
}

/// `SCIPprobSetObjIntegral`.
pub fn prob_set_obj_integral(scip: &mut Scip, prob: usize) {
    scip.probs[prob].objisintegral = true;
}

/// `SCIPprobCheckObjIntegral`: whether the objective is integral on all
/// solutions, and if so the cutoff bound is tightened.
pub fn prob_check_obj_integral(scip: &mut Scip, transprob: usize, origprob: usize) -> ScipResult {
    // If we know already, that the objective value is integral, nothing has
    // to be done.
    if scip.probs[transprob].objisintegral {
        return Ok(());
    }

    // If there exist unknown variables, we cannot conclude that the
    // objective value is always integral.
    if scip.set.nactivepricers != 0 || scip.set.nactivebenders != 0 {
        return Ok(());
    }

    // If the objective value offset is fractional, the value itself is
    // possibly fractional.
    if !scip.set.is_integral(scip.probs[transprob].objoffset) {
        return Ok(());
    }

    // Scan through the variables.
    let nvars = scip.probs[transprob].nvars;
    let mut v = 0;
    while v < nvars {
        let pvar = scip.probs[transprob].vars[v];
        let obj = scip.vars[pvar].obj;

        // Check whether variable is integral and has integral objective
        // value.
        if !scip.set.is_zero(obj) {
            if !scip.set.is_integral(obj) {
                break;
            }
            if !var::var_is_integral(scip, pvar) {
                break;
            }
        }
        v += 1;
    }

    // Objective value is integral, if the variable loop scanned all
    // variables.
    if v == nvars {
        scip.probs[transprob].objisintegral = true;

        // Update upper bound and cutoff bound in primal data structure due
        // to new internality information.
        crate::primal::primal_update_objoffset(scip, crate::scip::TRANSPRIMAL, transprob, origprob)?;
    }
    Ok(())
}

/// `SCIPprobUpdateNObjVars`.
pub fn prob_update_nobj_vars(scip: &mut Scip, prob: usize, oldobj: f64, newobj: f64) {
    if !scip.set.is_zero(oldobj) {
        scip.probs[prob].nobjvars -= 1;
    }
    if !scip.set.is_zero(newobj) {
        scip.probs[prob].nobjvars += 1;
    }
}

/// `SCIPprobUpdateDualbound`.
pub fn prob_update_dualbound(scip: &mut Scip, prob: usize, newbound: f64) {
    if scip.probs[prob].dualbound == INVALID {
        prob_set_dualbound(scip, prob, newbound);
    } else {
        let p = &mut scip.probs[prob];
        match p.objsense {
            ObjSense::Minimize => p.dualbound = max(newbound, p.dualbound),
            ObjSense::Maximize => p.dualbound = min(newbound, p.dualbound),
        }
    }
}

/// `SCIPprobInvalidateDualbound`.
pub fn prob_invalidate_dualbound(scip: &mut Scip, prob: usize) {
    scip.probs[prob].dualbound = INVALID;
}

/// `SCIPprobScaleObj`: scales an objective over integer variables only to
/// integers, if a small enough scalar does it.
pub fn prob_scale_obj(scip: &mut Scip, transprob: usize, origprob: usize) -> ScipResult {
    // Do not scale objective coefficients if pricers or Benders are active
    // or scaling is disabled.
    if scip.set.nactivepricers != 0 || scip.set.nactivebenders != 0 || !scip.set.misc_scaleobj {
        return Ok(());
    }

    // Get maximal number of integer variables.
    let nvars = scip.probs[transprob].nvars;
    let nints = nvars - scip.probs[transprob].ncontvars;

    // Scan through the continuous variables.
    let mut v = nints;
    while v < nvars {
        let obj = scip.vars[scip.probs[transprob].vars[v]].obj;
        if !scip.set.is_zero(obj) {
            break;
        }
        v += 1;
    }

    // Only continue if all continuous variables have zero objective value.
    if v == nvars {
        let mut objvals: CVec<f64> = CVec::with_size(nints);
        for v in 0..nints {
            objvals[v] = scip.vars[scip.probs[transprob].vars[v]].obj;
        }

        // Calculate integral scalar.
        let scalar = crate::misc::calc_integral_scalar(&objvals[0..nints], -scip.set.epsilon(), scip.set.epsilon(), OBJSCALE_MAXDNOM, OBJSCALE_MAXSCALE);
        let success = scalar.is_some();
        let mut intscalar = scalar.unwrap_or(crate::def::INVALID);

        if success {
            // Calculate gcd of resulting integral coefficients.
            let mut gcd: i64 = 0;
            let mut v = 0;
            while v < nints && gcd != 1 {
                // If absolute value is too large, abort.
                if objvals[v].abs() * intscalar + 0.5 > i64::MAX as f64 {
                    return Ok(());
                }
                let absobj = crate::misc::c_f64_to_i64(objvals[v].abs() * intscalar + 0.5);
                if gcd == 0 {
                    gcd = absobj;
                } else if absobj > 0 {
                    gcd = crate::misc::calc_gre_com_div(gcd, absobj);
                }
                v += 1;
            }
            if gcd != 0 {
                intscalar /= gcd as f64;
            }

            // Only apply scaling if the final scalar is small enough.
            if intscalar <= OBJSCALE_MAXFINALSCALE {
                // Apply scaling.
                if !scip.set.is_eq(intscalar, 1.0) {
                    // Calculate scaled objective values.
                    let mut v = 0;
                    while v < nints {
                        let newobj = intscalar * scip.vars[scip.probs[transprob].vars[v]].obj;
                        if !scip.set.is_feas_integral(newobj) {
                            break;
                        }
                        objvals[v] = scip.set.feas_floor(newobj);
                        v += 1;
                    }

                    // Change the variables' objective values and adjust
                    // objscale and objoffset.
                    if v == nints {
                        for v in 0..nints {
                            let pvar = scip.probs[transprob].vars[v];
                            var::var_chg_obj(scip, pvar, transprob, objvals[v])?;
                        }
                        let p = &mut scip.probs[transprob];
                        p.objoffset *= intscalar;
                        p.objscale /= intscalar;
                        p.objisintegral = true;

                        // Update upper bound and cutoff bound in primal
                        // data structure.
                        crate::primal::primal_update_objoffset(scip, crate::scip::TRANSPRIMAL, transprob, origprob)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// `SCIPprobStoreRootSol`.
pub fn prob_store_root_sol(scip: &mut Scip, prob: usize, roothaslp: bool) {
    if roothaslp {
        for v in 0..scip.probs[prob].nvars {
            let pvar = scip.probs[prob].vars[v];
            var::var_store_root_sol(scip, pvar, roothaslp);
        }
        let isrelax = crate::lp::lp_is_relax(scip);
        crate::lp::lp_set_root_lpis_relax(scip, isrelax);
        crate::lp::lp_store_root_objval(scip, prob);

        // Compute root LP best-estimate.
        let objval = crate::lp::lp_get_column_objval(scip);
        let n = scip.probs[prob].nvars - scip.probs[prob].ncontvars;
        let vars = scip.probs[prob].vars.head(n).to_vec();
        crate::stat::stat_compute_root_lp_best_estimate(scip, objval, &vars);
    }
}

/// `SCIPprobUpdateBestRootSol`.
pub fn prob_update_best_root_sol(scip: &mut Scip, prob: usize) {
    // Update the best root solution only if there are objective variables.
    if prob_get_nobj_vars(scip, prob) == 0 {
        return;
    }

    // Check if the LP solution is reliable.
    if !crate::lp::lp_is_dual_reliable(scip) {
        return;
    }

    let rootlpobjval = crate::lp::lp_get_objval(scip, prob);

    // Update the best root solution of all variables.
    for v in 0..scip.probs[prob].nvars {
        let var = scip.probs[prob].vars[v];
        let mut rootsol = 0.0;
        let mut rootredcost = 0.0;

        // Skip loose variables.
        if scip.vars[var].varstatus != VarStatus::Column {
            continue;
        }
        let col = scip.vars[var].data.col;

        // Skip binary variables with a fractional root solution.
        if !var::var_is_binary(scip, var) {
            rootsol = var::var_get_sol(scip, var, true);
            rootredcost = crate::lp::col_get_redcost(scip, col);
        } else {
            let basestat = crate::lp::col_get_basis_status(scip, col);
            let lpissolbasic = crate::lp::lp_is_sol_basic(scip);
            let primsol = crate::lp::col_get_primsol(scip, col);

            // Check if the root solution is integral.
            if (lpissolbasic && (basestat == crate::lpi::BASESTAT_LOWER || basestat == crate::lpi::BASESTAT_UPPER))
                || (!lpissolbasic && (scip.set.is_feas_eq(scip.vars[var].locdom.lb, primsol) || scip.set.is_feas_eq(scip.vars[var].locdom.ub, primsol)))
            {
                let lbrootredcost = var::var_get_impl_redcost(scip, var, false, prob);
                let ubrootredcost = var::var_get_impl_redcost(scip, var, true, prob);
                if -lbrootredcost > ubrootredcost {
                    rootredcost = lbrootredcost;
                    rootsol = 1.0;
                } else {
                    rootredcost = ubrootredcost;
                    rootsol = 0.0;
                }
            }
        }

        // Update the current solution as best root solution in the problem
        // variables if it is better.
        var::var_update_best_root_sol(scip, var, rootsol, rootredcost, rootlpobjval);
    }
}

/// `SCIPprobExitPresolve`.
pub fn prob_exit_presolve(_scip: &mut Scip, _prob: usize) -> ScipResult {
    Ok(())
}

/// `SCIPprobInitSolve`.
pub fn prob_init_solve(scip: &mut Scip, prob: usize) -> ScipResult {
    // Reset constraint's ages.
    for c in 0..scip.probs[prob].nconss {
        let cons = scip.probs[prob].conss[c];
        cons::cons_reset_age(scip, cons);
    }

    // Initialize variables for solving.
    for v in 0..scip.probs[prob].nvars {
        let pvar = scip.probs[prob].vars[v];
        var::var_init_solve(scip, pvar);
    }
    Ok(())
}

/// `SCIPprobExitSolve`.
pub fn prob_exit_solve(scip: &mut Scip, prob: usize, restart: bool) -> ScipResult {
    // Convert all COLUMN variables back into LOOSE variables.
    if scip.probs[prob].ncolvars > 0 || restart {
        // `nvars` is read every iteration, as C does; deletions only queue.
        let mut v = 0;
        while v < scip.probs[prob].nvars {
            let var = scip.probs[prob].vars[v];
            if scip.vars[var].varstatus == VarStatus::Column {
                var::var_loose(scip, var, prob)?;
            }

            // Invalidate root reduced cost, root reduced solution, and root
            // LP objective value for each variable.
            var::var_set_best_root_sol(scip, var, 0.0, 0.0, INVALID);

            if scip.vars[var].relaxationonly && restart {
                // Relaxation variables should be unlocked and only captured
                // by prob at this moment.
                if scip.vars[var].deletable {
                    let mut deleted = false;
                    prob_del_var(scip, prob, var, &mut deleted)?;
                }
            }
            v += 1;
        }
    }
    Ok(())
}

/// `SCIPprobSetName`.
pub fn prob_set_name(scip: &mut Scip, prob: usize, name: &str) {
    scip.probs[prob].name = name.to_owned();
}

/// `SCIPprobGetNObjVars`.
pub fn prob_get_nobj_vars(scip: &Scip, prob: usize) -> i32 {
    let p = &scip.probs[prob];
    if p.transformed {
        p.nobjvars
    } else {
        let mut nobjvars = 0;
        let mut v = p.nvars - 1;
        while v >= 0 {
            if !scip.set.is_zero(scip.vars[p.vars[v]].obj) {
                nobjvars += 1;
            }
            v -= 1;
        }
        nobjvars
    }
}

/// `SCIPprobGetAbsMinObjCoef`.
pub fn prob_get_abs_min_obj_coef(scip: &Scip, prob: usize) -> f64 {
    let mut absmin = scip.set.infinity();
    for v in 0..scip.probs[prob].nvars {
        let objcoef = scip.vars[scip.probs[prob].vars[v]].obj;
        if !scip.set.is_zero(objcoef) && scip.set.is_lt(objcoef.abs(), absmin) {
            absmin = objcoef.abs();
        }
    }
    absmin
}

/// `SCIPprobGetAbsMaxObjCoef`.
pub fn prob_get_abs_max_obj_coef(scip: &Scip, prob: usize) -> f64 {
    let mut absmax = -scip.set.infinity();
    for v in 0..scip.probs[prob].nvars {
        let objcoef = scip.vars[scip.probs[prob].vars[v]].obj;
        if !scip.set.is_zero(objcoef) && scip.set.is_gt(objcoef.abs(), absmax) {
            absmax = objcoef.abs();
        }
    }
    absmax
}

/// `SCIPprobExternObjval`: the objective value of the original problem.
pub fn prob_extern_objval(scip: &Scip, transprob: usize, origprob: usize, objval: f64) -> f64 {
    let t = &scip.probs[transprob];
    let sense = t.objsense as i32 as f64;
    if scip.set.is_infinity(objval) {
        sense * scip.set.infinity()
    } else if scip.set.is_infinity(-objval) {
        -sense * scip.set.infinity()
    } else {
        sense * t.objscale * (objval + t.objoffset) + scip.probs[origprob].objoffset
    }
}

/// `SCIPprobInternObjval`: the objective value of the transformed problem.
pub fn prob_intern_objval(scip: &Scip, transprob: usize, origprob: usize, objval: f64) -> f64 {
    let t = &scip.probs[transprob];
    let sense = t.objsense as i32 as f64;
    if scip.set.is_infinity(objval) {
        sense * scip.set.infinity()
    } else if scip.set.is_infinity(-objval) {
        -sense * scip.set.infinity()
    } else {
        sense * (objval - scip.probs[origprob].objoffset) / t.objscale - t.objoffset
    }
}

/// `SCIPprobFindVar`.
pub fn prob_find_var(scip: &Scip, prob: usize, name: &str) -> VarId {
    let Some(table) = scip.probs[prob].varnames.as_ref() else {
        panic!("Cannot find variable if variable-names hashtable was disabled (due to parameter <misc/usevartable>)");
    };
    table.retrieve(&VarNames(&scip.vars), &name).unwrap_or(NONE)
}

/// `SCIPprobFindCons`.
pub fn prob_find_cons(scip: &Scip, prob: usize, name: &str) -> ConsId {
    let Some(table) = scip.probs[prob].consnames.as_ref() else {
        panic!("Cannot find constraint if constraint-names hashtable was disabled (due to parameter <misc/useconstable>)");
    };
    table.retrieve(&ConsNames(&scip.conss), &name).unwrap_or(NONE)
}

/// `SCIPprobAllColsInLP`.
pub fn prob_all_cols_in_lp(scip: &Scip, prob: usize) -> bool {
    scip.lp.as_ref().map_or(0, |lp| lp.ncols) == scip.probs[prob].ncolvars && scip.set.nactivepricers == 0
}

/// `SCIPprobGetObjlim`.
pub fn prob_get_objlim(scip: &Scip, prob: usize) -> f64 {
    let p = &scip.probs[prob];
    if p.objlim >= INVALID {
        p.objsense as i32 as f64 * scip.set.infinity()
    } else {
        p.objlim
    }
}
