//! SCIP's relaxators and the global relaxation data (`relax.c`,
//! `struct_relax.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{RelaxId, ScipResultCode, VarId},
    var::NONE,
};

pub type RelaxMethod = fn(&mut Scip, RelaxId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type RelaxCopy = fn(&mut Scip, &Scip, RelaxId) -> ScipResult;
/// `SCIP_DECL_RELAXEXEC`: `(lowerbound, result)`.
pub type RelaxExec = fn(&mut Scip, RelaxId, &mut f64, &mut ScipResultCode) -> ScipResult;

/// `SCIP_RELAX`.
pub struct Relax {
    pub ncalls: i64,
    pub ncutoffs: i64,
    pub nimprbounds: i64,
    pub naddedconss: i64,
    pub nreduceddom: i64,
    pub nseparated: i64,
    pub lastsolvednode: i64,
    pub name: String,
    pub desc: String,
    pub relaxcopy: Option<RelaxCopy>,
    pub relaxfree: Option<RelaxMethod>,
    pub relaxinit: Option<RelaxMethod>,
    pub relaxexit: Option<RelaxMethod>,
    pub relaxinitsol: Option<RelaxMethod>,
    pub relaxexitsol: Option<RelaxMethod>,
    pub relaxexec: Option<RelaxExec>,
    pub relaxdata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub relaxclock: Clock,
    pub imprtime: f64,
    pub priority: i32,
    pub freq: i32,
    pub exact: bool,
    pub initialized: bool,
}

/// `SCIP_RELAXATION`: relaxation information data.
#[derive(Clone, Copy, Debug)]
pub struct Relaxation {
    pub relaxsolobjval: f64,
    pub relaxsolvalid: bool,
    pub relaxsolincludeslp: bool,
    pub relaxsolzero: bool,
    pub lastsolrelax: RelaxId,
}

impl Default for Relaxation {
    /// `SCIPrelaxationCreate`: creates global relaxation data.
    fn default() -> Self {
        Self {
            relaxsolobjval: 0.0,
            relaxsolvalid: false,
            relaxsolincludeslp: false,
            relaxsolzero: true,
            lastsolrelax: NONE,
        }
    }
}

/// `SCIPrelaxComp`: compares two relaxation handlers w. r. to their
/// priority.
pub fn relax_comp(scip: &Scip, elem1: RelaxId, elem2: RelaxId) -> i32 {
    scip.relaxs[elem2].priority - scip.relaxs[elem1].priority
}

/// `SCIPrelaxCompName`: comparison method for sorting relaxators w.r.t. to
/// their name.
pub fn relax_comp_name(scip: &Scip, elem1: RelaxId, elem2: RelaxId) -> std::cmp::Ordering {
    scip.relaxs[elem1].name.as_bytes().cmp(scip.relaxs[elem2].name.as_bytes())
}

/// `paramChgdRelaxPriority`: the parameter already wrote the priority;
/// mark the relaxators unsorted (`SCIPsetRelaxPriority`).
fn param_chgd_relax_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.relaxssorted = false;
    Ok(())
}

/// `SCIPrelaxCopyInclude`: copies the given relaxation handler to a new
/// scip.
pub fn relax_copy_include(scip: &mut Scip, sourcescip: &Scip, relax: RelaxId) -> ScipResult {
    if let Some(copy) = sourcescip.relaxs[relax].relaxcopy {
        copy(scip, sourcescip, relax)?;
    }
    Ok(())
}

/// The callbacks `SCIPrelaxCreate` takes.
#[derive(Default)]
pub struct RelaxMethods {
    pub relaxcopy: Option<RelaxCopy>,
    pub relaxfree: Option<RelaxMethod>,
    pub relaxinit: Option<RelaxMethod>,
    pub relaxexit: Option<RelaxMethod>,
    pub relaxinitsol: Option<RelaxMethod>,
    pub relaxexitsol: Option<RelaxMethod>,
    pub relaxexec: Option<RelaxExec>,
}

/// `SCIPrelaxCreate`: creates a relaxation handler.
pub fn relax_create(scip: &mut Scip, name: &str, desc: &str, priority: i32, freq: i32, methods: RelaxMethods, relaxdata: Option<Box<dyn Any>>) -> ScipResult<RelaxId> {
    let relax = scip.relaxs.push(Relax {
        name: name.to_string(),
        desc: desc.to_string(),
        priority,
        freq,
        relaxcopy: methods.relaxcopy,
        relaxfree: methods.relaxfree,
        relaxinit: methods.relaxinit,
        relaxexit: methods.relaxexit,
        relaxinitsol: methods.relaxinitsol,
        relaxexitsol: methods.relaxexitsol,
        relaxexec: methods.relaxexec,
        relaxdata,
        setuptime: Clock::new(),
        relaxclock: Clock::new(),
        ncalls: 0,
        ncutoffs: 0,
        nimprbounds: 0,
        imprtime: 0.0,
        naddedconss: 0,
        nreduceddom: 0,
        nseparated: 0,
        lastsolvednode: -1,
        initialized: false,
        exact: false,
    });

    // Add parameters.
    crate::paramset::paramset_add_int(
        scip,
        &format!("relaxing/{name}/priority"),
        &format!("priority of relaxation handler <{name}>"),
        crate::field_target!(i32, relaxs, relax, priority),
        false,
        priority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_relax_priority),
    )?;
    crate::paramset::paramset_add_int(
        scip,
        &format!("relaxing/{name}/freq"),
        &format!("frequency for calling relaxation handler <{name}> (-1: never, 0: only in root node)"),
        crate::field_target!(i32, relaxs, relax, freq),
        false,
        freq,
        -1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    Ok(relax)
}

/// `SCIPrelaxFree`: calls destructor and frees memory of relaxation
/// handler.
pub fn relax_free(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    // Call destructor of relaxation handler.
    if let Some(free) = scip.relaxs[relax].relaxfree {
        free(scip, relax)?;
    }
    scip.relaxs.remove(relax);
    Ok(())
}

/// Runs one of the relaxator's setup callbacks under its setup clock.
fn relax_setup_call(scip: &mut Scip, relax: RelaxId, method: Option<RelaxMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.relaxs[relax].setuptime.start(enabled);
        method(scip, relax)?;
        // Stop timing.
        scip.relaxs[relax].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPrelaxInit`: initializes relaxation handler.
pub fn relax_init(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    if scip.relaxs[relax].initialized {
        log::error!("relaxation handler <{}> already initialized", scip.relaxs[relax].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let r = &mut scip.relaxs[relax];
        r.setuptime.reset();
        r.relaxclock.reset();
        r.ncalls = 0;
        r.ncutoffs = 0;
        r.nimprbounds = 0;
        r.imprtime = 0.0;
        r.naddedconss = 0;
        r.nreduceddom = 0;
        r.nseparated = 0;
        r.lastsolvednode = -1;
    }

    let init = scip.relaxs[relax].relaxinit;
    relax_setup_call(scip, relax, init)?;
    scip.relaxs[relax].initialized = true;
    Ok(())
}

/// `SCIPrelaxExit`: calls exit method of relaxation handler.
pub fn relax_exit(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    if !scip.relaxs[relax].initialized {
        log::error!("relaxation handler <{}> not initialized", scip.relaxs[relax].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.relaxs[relax].relaxexit;
    relax_setup_call(scip, relax, exit)?;
    scip.relaxs[relax].initialized = false;
    Ok(())
}

/// `SCIPrelaxInitsol`: informs relaxation handler that the branch and
/// bound process is being started.
pub fn relax_initsol(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    let initsol = scip.relaxs[relax].relaxinitsol;
    relax_setup_call(scip, relax, initsol)
}

/// `SCIPrelaxExitsol`: informs relaxation handler that the branch and
/// bound process data is being freed.
pub fn relax_exitsol(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    let exitsol = scip.relaxs[relax].relaxexitsol;
    relax_setup_call(scip, relax, exitsol)
}

/// `SCIPrelaxExec`: calls execution method of relaxation handler.
pub fn relax_exec(scip: &mut Scip, relax: RelaxId, depth: i32, lowerbound: &mut f64) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    // Check, if the relaxation is already solved.
    if scip.relaxs[relax].lastsolvednode == scip.stat.ntotalnodes && !crate::tree::tree_probing(scip) {
        return Ok(result);
    }

    scip.relaxs[relax].lastsolvednode = scip.stat.ntotalnodes;

    let freq = scip.relaxs[relax].freq;
    if (depth == 0 && freq == 0) || (freq > 0 && depth % freq == 0) {
        let oldnactiveconss = scip.stat.nactiveconss;
        let oldncuts = crate::sepastore::sepastore_get_ncuts(scip);

        // Start timing.
        let enabled = scip.set.time_enabled;
        let starttime = scip.relaxs[relax].relaxclock.get_time();
        scip.relaxs[relax].relaxclock.start(enabled);

        // Call external relaxation method.
        let exec = scip.relaxs[relax].relaxexec.expect("an execution method");
        exec(scip, relax, lowerbound, &mut result)?;

        // Stop timing.
        scip.relaxs[relax].relaxclock.stop(enabled);

        // Evaluate result.
        if !matches!(result, R::Cutoff | R::ConsAdded | R::ReducedDom | R::Separated | R::Success | R::Suspended | R::DidNotRun) {
            log::error!(
                "execution method of relaxation handler <{}> returned invalid result <{}>",
                scip.relaxs[relax].name,
                result as i32
            );
            return Err(Retcode::InvalidResult);
        }
        if result != R::DidNotRun {
            scip.relaxs[relax].ncalls += 1;
            scip.stat.relaxcount += 1;
            if result == R::Suspended {
                relax_mark_unsolved(scip, relax);
            } else if result == R::Cutoff || scip.set.is_infinity(*lowerbound) {
                let t = scip.relaxs[relax].relaxclock.get_time();
                let r = &mut scip.relaxs[relax];
                r.ncutoffs += 1;
                r.imprtime += t - starttime;
            } else {
                let node = crate::tree::tree_get_current_node(scip);
                let oldlowerbound = if node != NONE { scip.nodes[node].lowerbound } else { -scip.set.infinity() };

                if !scip.set.is_infinity(-*lowerbound) && scip.set.is_rel_gt(*lowerbound, oldlowerbound) {
                    let t = scip.relaxs[relax].relaxclock.get_time();
                    let r = &mut scip.relaxs[relax];
                    r.nimprbounds += 1;
                    r.imprtime += t - starttime;
                }

                if scip.stat.nactiveconss > oldnactiveconss {
                    scip.relaxs[relax].naddedconss += 1;
                }
                if crate::sepastore::sepastore_get_ncuts(scip) > oldncuts {
                    scip.relaxs[relax].nseparated += 1;
                }
                if result == R::ReducedDom {
                    scip.relaxs[relax].nreduceddom += 1;
                }
            }
        }
    }
    Ok(result)
}

/// `SCIPrelaxSetPriority`: sets priority of relaxation handler.
pub fn relax_set_priority(scip: &mut Scip, relax: RelaxId, priority: i32) {
    scip.relaxs[relax].priority = priority;
    scip.set.relaxssorted = false;
}

/// `SCIPrelaxEnableOrDisableClocks`.
pub fn relax_enable_or_disable_clocks(scip: &mut Scip, relax: RelaxId, enable: bool) {
    let r = &mut scip.relaxs[relax];
    r.setuptime.enable_or_disable(enable);
    r.relaxclock.enable_or_disable(enable);
}

/// `SCIPrelaxIsSolved`: returns whether the relaxation was completely
/// solved at the current node.
pub fn relax_is_solved(scip: &Scip, relax: RelaxId) -> bool {
    scip.relaxs[relax].lastsolvednode == scip.stat.ntotalnodes
}

/// `SCIPrelaxMarkUnsolved`: marks the current relaxation unsolved, s.t.
/// the relaxation handler is called again in the next solving round.
pub fn relax_mark_unsolved(scip: &mut Scip, relax: RelaxId) {
    scip.relaxs[relax].lastsolvednode = -1;
}

/*
 * methods for the global relaxation data
 */

/// `SCIPrelaxationSetSolZero`.
pub fn relaxation_set_sol_zero(scip: &mut Scip, iszero: bool) {
    scip.relaxation.relaxsolzero = iszero;
}

/// `SCIPrelaxationIsSolZero`: returns whether the global relaxation
/// solution is cleared and all values are set to zero.
pub fn relaxation_is_sol_zero(scip: &Scip) -> bool {
    scip.relaxation.relaxsolzero
}

/// `SCIPrelaxationSetSolValid`: sets the relaxsolvalid and includeslp
/// flags in the relaxation data to the given values.
pub fn relaxation_set_sol_valid(scip: &mut Scip, isvalid: bool, includeslp: bool) {
    scip.relaxation.relaxsolvalid = isvalid;
    scip.relaxation.relaxsolincludeslp = includeslp;
}

/// `SCIPrelaxationIsSolValid`: returns whether the global relaxation
/// solution is valid.
pub fn relaxation_is_sol_valid(scip: &Scip) -> bool {
    scip.relaxation.relaxsolvalid
}

/// `SCIPrelaxationIsLpIncludedForSol`: returns whether the global
/// relaxation solution was computed by a relaxator which included all LP
/// cuts.
pub fn relaxation_is_lp_included_for_sol(scip: &Scip) -> bool {
    scip.relaxation.relaxsolincludeslp
}

/// `SCIPrelaxationSetSolObj`.
pub fn relaxation_set_sol_obj(scip: &mut Scip, obj: f64) {
    scip.relaxation.relaxsolobjval = obj;
}

/// `SCIPrelaxationGetSolObj`: returns the objective value of the global
/// relaxation solution w.r.t. the transformed problem.
pub fn relaxation_get_sol_obj(scip: &Scip) -> f64 {
    scip.relaxation.relaxsolobjval
}

/// `SCIPrelaxationSolObjAdd`: adds the given value to the global
/// relaxation solution's objective value.
pub fn relaxation_sol_obj_add(scip: &mut Scip, val: f64) {
    scip.relaxation.relaxsolobjval += val;
}

/// `SCIPrelaxationUpdateVarObj`: updates objective value of current
/// relaxation solution after change of objective coefficient.
pub fn relaxation_update_var_obj(scip: &mut Scip, var: VarId, oldobj: f64, newobj: f64) {
    let relaxsolval = crate::var::var_get_relax_sol(scip, var);
    scip.relaxation.relaxsolobjval += (newobj - oldobj) * relaxsolval;
}

/// `SCIPrelaxationSetSolRelax`: store the most recent relaxation handler
/// responsible for the solution.
pub fn relaxation_set_sol_relax(scip: &mut Scip, relax: RelaxId) {
    scip.relaxation.lastsolrelax = relax;
}

/// `SCIPrelaxationGetSolRelax`: returns the most recent relaxation handler
/// responsible for the solution, or `NONE` if unspecified.
pub fn relaxation_get_sol_relax(scip: &Scip) -> RelaxId {
    scip.relaxation.lastsolrelax
}

/// `SCIPsetSortRelaxs` (`set.c`): sorts relaxators by priorities.
pub fn set_sort_relaxs(scip: &mut Scip) {
    if !scip.set.relaxssorted {
        let n = scip.set.nrelaxs as usize;
        let mut relaxs = std::mem::take(&mut scip.set.relaxs);
        crate::sort::sort(&mut relaxs.0[..n], (), |&a, &b| relax_comp(scip, a, b));
        scip.set.relaxs = relaxs;
        scip.set.relaxssorted = true;
        scip.set.relaxsnamesorted = false;
    }
}
