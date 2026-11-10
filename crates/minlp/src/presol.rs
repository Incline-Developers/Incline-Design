//! SCIP's presolvers (`presol.c`, `struct_presol.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    cons::{PRESOLTIMING_FAST, PRESOLTIMING_MAX, PRESOLTIMING_NONE, PresolCounts, PresolTiming},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{PresolId, ScipResultCode},
};

pub type PresolMethod = fn(&mut Scip, PresolId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type PresolCopy = fn(&mut Scip, &Scip, PresolId) -> ScipResult;
/// `SCIP_DECL_PRESOLEXEC`: `(nrounds, presoltiming, new counts, counts,
/// result)`.
pub type PresolExec = fn(&mut Scip, PresolId, i32, PresolTiming, &PresolCounts, &mut PresolCounts, &mut ScipResultCode) -> ScipResult;

/// `SCIP_PRESOL`.
pub struct Presol {
    pub name: String,
    pub desc: String,
    pub presolcopy: Option<PresolCopy>,
    pub presolfree: Option<PresolMethod>,
    pub presolinit: Option<PresolMethod>,
    pub presolexit: Option<PresolMethod>,
    pub presolinitpre: Option<PresolMethod>,
    pub presolexitpre: Option<PresolMethod>,
    pub presolexec: Option<PresolExec>,
    pub presoldata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub presolclock: Clock,
    pub priority: i32,
    pub maxrounds: i32,
    /// The counts at the last call (`lastnfixedvars` ...).
    pub last: PresolCounts,
    /// The changes this presolver made (`nfixedvars` ...).
    pub found: PresolCounts,
    pub ncalls: i32,
    pub initialized: bool,
    pub exact: bool,
    pub timing: PresolTiming,
}

/// `SCIPpresolComp`: compares two presolvers w. r. to their priority.
pub fn presol_comp(scip: &Scip, elem1: PresolId, elem2: PresolId) -> i32 {
    scip.presols[elem2].priority - scip.presols[elem1].priority
}

/// `paramChgdPresolPriority`: the parameter already wrote the priority;
/// mark the presolvers unsorted (`SCIPsetPresolPriority`).
fn param_chgd_presol_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.presolssorted = false;
    Ok(())
}

/// `SCIPpresolCopyInclude`: copies the given presolver to a new scip.
pub fn presol_copy_include(scip: &mut Scip, sourcescip: &Scip, presol: PresolId) -> ScipResult {
    if let Some(copy) = sourcescip.presols[presol].presolcopy {
        copy(scip, sourcescip, presol)?;
    }
    Ok(())
}

/// The callbacks `SCIPpresolCreate` takes.
#[derive(Default)]
pub struct PresolMethods {
    pub presolcopy: Option<PresolCopy>,
    pub presolfree: Option<PresolMethod>,
    pub presolinit: Option<PresolMethod>,
    pub presolexit: Option<PresolMethod>,
    pub presolinitpre: Option<PresolMethod>,
    pub presolexitpre: Option<PresolMethod>,
    pub presolexec: Option<PresolExec>,
}

/// `SCIPpresolCreate`: creates a presolver.
pub fn presol_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    maxrounds: i32,
    timing: PresolTiming,
    methods: PresolMethods,
    presoldata: Option<Box<dyn Any>>,
) -> ScipResult<PresolId> {
    use crate::paramset::paramset_add_int;

    // The interface change from delay flags to timings cannot be recognized
    // at compile time: Exit with an appropriate error message.
    if !(PRESOLTIMING_NONE..=PRESOLTIMING_MAX).contains(&timing) {
        log::error!("ERROR: 'PRESOLDELAY'-flag no longer available since SCIP 3.2, use an appropriate 'SCIP_PRESOLTIMING' for <{name}> presolver instead.");
        return Err(Retcode::ParameterWrongVal);
    }

    let presol = scip.presols.push(Presol {
        name: name.to_string(),
        desc: desc.to_string(),
        presolcopy: methods.presolcopy,
        presolfree: methods.presolfree,
        presolinit: methods.presolinit,
        presolexit: methods.presolexit,
        presolinitpre: methods.presolinitpre,
        presolexitpre: methods.presolexitpre,
        presolexec: methods.presolexec,
        presoldata,
        setuptime: Clock::new(),
        presolclock: Clock::new(),
        initialized: false,
        exact: false,
        priority,
        maxrounds,
        timing,
        last: PresolCounts::default(),
        found: PresolCounts::default(),
        ncalls: 0,
    });

    // Add parameters.
    paramset_add_int(
        scip,
        &format!("presolving/{name}/priority"),
        &format!("priority of presolver <{name}>"),
        crate::field_target!(i32, presols, presol, priority),
        true,
        priority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_presol_priority),
    )?;
    paramset_add_int(
        scip,
        &format!("presolving/{name}/maxrounds"),
        "maximal number of presolving rounds the presolver participates in (-1: no limit)",
        crate::field_target!(i32, presols, presol, maxrounds),
        false,
        maxrounds,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("presolving/{name}/timing"),
        &format!("timing mask of presolver <{name}> ({}:FAST, {}:MEDIUM, {}:EXHAUSTIVE, {}:FINAL)", 4, 8, 16, 32),
        crate::field_target!(i32, presols, presol, timing),
        true,
        timing,
        PRESOLTIMING_FAST,
        PRESOLTIMING_MAX,
        None,
    )?;
    Ok(presol)
}

/// `SCIPpresolFree`: frees memory of presolver.
pub fn presol_free(scip: &mut Scip, presol: PresolId) -> ScipResult {
    // Call destructor of presolver.
    if let Some(free) = scip.presols[presol].presolfree {
        free(scip, presol)?;
    }
    scip.presols.remove(presol);
    Ok(())
}

/// Runs one of the presolver's setup callbacks under its setup clock.
fn presol_setup_call(scip: &mut Scip, presol: PresolId, method: Option<PresolMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.presols[presol].setuptime.start(enabled);
        method(scip, presol)?;
        // Stop timing.
        scip.presols[presol].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPpresolInit`: initializes presolver.
pub fn presol_init(scip: &mut Scip, presol: PresolId) -> ScipResult {
    if scip.presols[presol].initialized {
        log::error!("presolver <{}> already initialized", scip.presols[presol].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let p = &mut scip.presols[presol];
        p.setuptime.reset();
        p.presolclock.reset();

        p.last = PresolCounts::default();
        p.found = PresolCounts::default();
        p.ncalls = 0;
    }

    // Call initialization method of presolver.
    let init = scip.presols[presol].presolinit;
    presol_setup_call(scip, presol, init)?;
    scip.presols[presol].initialized = true;
    Ok(())
}

/// `SCIPpresolExit`: deinitializes presolver.
pub fn presol_exit(scip: &mut Scip, presol: PresolId) -> ScipResult {
    if !scip.presols[presol].initialized {
        log::error!("presolver <{}> not initialized", scip.presols[presol].name);
        return Err(Retcode::InvalidCall);
    }

    // Call deinitialization method of presolver.
    let exit = scip.presols[presol].presolexit;
    presol_setup_call(scip, presol, exit)?;
    scip.presols[presol].initialized = false;
    Ok(())
}

/// `SCIPpresolInitpre`: informs presolver that the presolving process is
/// being started.
pub fn presol_initpre(scip: &mut Scip, presol: PresolId) -> ScipResult {
    scip.presols[presol].last = PresolCounts::default();

    // Call presolving initialization method of presolver.
    let initpre = scip.presols[presol].presolinitpre;
    presol_setup_call(scip, presol, initpre)
}

/// `SCIPpresolExitpre`: informs presolver that the presolving process is
/// finished.
pub fn presol_exitpre(scip: &mut Scip, presol: PresolId) -> ScipResult {
    // Call presolving deinitialization method of presolver.
    let exitpre = scip.presols[presol].presolexitpre;
    presol_setup_call(scip, presol, exitpre)
}

/// `SCIPpresolExec`: executes presolver.
pub fn presol_exec(scip: &mut Scip, presol: PresolId, timing: PresolTiming, nrounds: i32, counts: &mut PresolCounts) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    // Check number of presolving rounds.
    let p = &scip.presols[presol];
    if p.maxrounds >= 0 && p.ncalls >= p.maxrounds {
        return Ok(result);
    }

    // Calculate the number of changes since last call.
    let newcounts = counts.minus(&p.last);

    // Remember the number of changes prior to the call of the presolver.
    scip.presols[presol].last = *counts;

    // Check, if presolver should be called with the current timing.
    if timing & scip.presols[presol].timing != 0 {
        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.presols[presol].presolclock.start(enabled);

        // Call external method.
        let exec = scip.presols[presol].presolexec.expect("an execution method");
        exec(scip, presol, nrounds, timing, &newcounts, counts, &mut result)?;

        // Stop timing.
        scip.presols[presol].presolclock.stop(enabled);

        // Add/count the new changes.
        let p = &mut scip.presols[presol];
        let d = counts.minus(&p.last);
        p.found.add(&d);

        // Check result code of callback method.
        if !matches!(result, R::Cutoff | R::Unbounded | R::Success | R::DidNotFind | R::DidNotRun) {
            log::error!("presolver <{}> returned invalid result <{}>", p.name, result as i32);
            return Err(Retcode::InvalidResult);
        }

        // Increase the number of calls, if the presolver tried to find
        // reductions.
        if result != R::DidNotRun {
            p.ncalls += 1;
        }
    }
    Ok(result)
}

/// `SCIPpresolSetPriority`: sets priority of presolver.
pub fn presol_set_priority(scip: &mut Scip, presol: PresolId, priority: i32) {
    scip.presols[presol].priority = priority;
    scip.set.presolssorted = false;
}

/// `SCIPpresolEnableOrDisableClocks`.
pub fn presol_enable_or_disable_clocks(scip: &mut Scip, presol: PresolId, enable: bool) {
    let p = &mut scip.presols[presol];
    p.setuptime.enable_or_disable(enable);
    p.presolclock.enable_or_disable(enable);
}

/// `SCIPsetSortPresols` (`set.c`): sorts presolvers by priorities.
pub fn set_sort_presols(scip: &mut Scip) {
    if !scip.set.presolssorted {
        let n = scip.set.npresols as usize;
        let mut presols = std::mem::take(&mut scip.set.presols);
        crate::sort::sort(&mut presols.0[..n], (), |&a, &b| presol_comp(scip, a, b));
        scip.set.presols = presols;
        scip.set.presolssorted = true;
        scip.set.presolsnamesorted = false;
    }
}
