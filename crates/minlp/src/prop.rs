//! SCIP's propagators (`prop.c`, `struct_prop.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    cons::{PRESOLTIMING_FAST, PRESOLTIMING_MAX, PRESOLTIMING_NONE, PROPTIMING_ALWAYS, PresolCounts, PresolTiming, PropTiming},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{BoundType, PropId, ScipResultCode, VarId},
    var::BdChgIdx,
};

pub type PropMethod = fn(&mut Scip, PropId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type PropCopy = fn(&mut Scip, &Scip, PropId) -> ScipResult;
pub type PropExitsol = fn(&mut Scip, PropId, bool) -> ScipResult;
/// `SCIP_DECL_PROPPRESOL`: `(nrounds, presoltiming, new counts, counts,
/// result)`.
pub type PropPresol = fn(&mut Scip, PropId, i32, PresolTiming, &PresolCounts, &mut PresolCounts, &mut ScipResultCode) -> ScipResult;
/// `SCIP_DECL_PROPEXEC`: `(proptiming, result)`.
pub type PropExec = fn(&mut Scip, PropId, PropTiming, &mut ScipResultCode) -> ScipResult;
/// `SCIP_DECL_PROPRESPROP`: `(infervar, inferinfo, boundtype, bdchgidx,
/// relaxedbd, result)`.
pub type PropResprop = fn(&mut Scip, PropId, VarId, i32, BoundType, Option<BdChgIdx>, f64, &mut ScipResultCode) -> ScipResult;

/// `PROPTIMING_NONE`.
pub const PROPTIMING_NONE: PropTiming = 0x000;

/// `SCIP_PROP`.
pub struct Prop {
    pub ncalls: i64,
    pub nrespropcalls: i64,
    pub ncutoffs: i64,
    pub ndomredsfound: i64,
    pub name: String,
    pub desc: String,
    pub propcopy: Option<PropCopy>,
    pub propfree: Option<PropMethod>,
    pub propinit: Option<PropMethod>,
    pub propexit: Option<PropMethod>,
    pub propinitpre: Option<PropMethod>,
    pub propexitpre: Option<PropMethod>,
    pub propinitsol: Option<PropMethod>,
    pub propexitsol: Option<PropExitsol>,
    pub proppresol: Option<PropPresol>,
    pub propexec: Option<PropExec>,
    pub propresprop: Option<PropResprop>,
    pub propdata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub proptime: Clock,
    pub sbproptime: Clock,
    pub resproptime: Clock,
    pub presoltime: Clock,
    pub priority: i32,
    pub freq: i32,
    pub timingmask: PropTiming,
    pub presoltiming: PresolTiming,
    pub presolpriority: i32,
    pub maxprerounds: i32,
    /// The counts at the last presolving call (`lastnfixedvars` ...).
    pub last: PresolCounts,
    /// The changes this propagator made in presolving (`nfixedvars` ...).
    pub found: PresolCounts,
    pub npresolcalls: i32,
    pub delay: bool,
    pub wasdelayed: bool,
    pub exact: bool,
    pub initialized: bool,
}

/// `SCIPpropComp`: compares two propagators w. r. to their priority.
pub fn prop_comp(scip: &Scip, elem1: PropId, elem2: PropId) -> i32 {
    scip.props[elem2].priority - scip.props[elem1].priority
}

/// `SCIPpropCompPresol`: compares two propagators w. r. to their presolving
/// priority.
pub fn prop_comp_presol(scip: &Scip, elem1: PropId, elem2: PropId) -> i32 {
    scip.props[elem2].presolpriority - scip.props[elem1].presolpriority
}

/// `paramChgdPropPriority`: the parameter already wrote the priority; mark
/// the propagators unsorted (`SCIPsetPropPriority`).
fn param_chgd_prop_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.propssorted = false;
    Ok(())
}

/// `paramChgdPropPresolPriority`: marks the propagators unsorted for
/// presolving (`SCIPsetPropPresolPriority`).
fn param_chgd_prop_presol_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.propspresolsorted = false;
    Ok(())
}

/// `SCIPpropCopyInclude`: copies the given propagator to a new scip.
pub fn prop_copy_include(scip: &mut Scip, sourcescip: &Scip, prop: PropId) -> ScipResult {
    if let Some(copy) = sourcescip.props[prop].propcopy {
        copy(scip, sourcescip, prop)?;
    }
    Ok(())
}

/// The callbacks `SCIPpropCreate` takes.
#[derive(Default)]
pub struct PropMethods {
    pub propcopy: Option<PropCopy>,
    pub propfree: Option<PropMethod>,
    pub propinit: Option<PropMethod>,
    pub propexit: Option<PropMethod>,
    pub propinitpre: Option<PropMethod>,
    pub propexitpre: Option<PropMethod>,
    pub propinitsol: Option<PropMethod>,
    pub propexitsol: Option<PropExitsol>,
    pub proppresol: Option<PropPresol>,
    pub propexec: Option<PropExec>,
    pub propresprop: Option<PropResprop>,
}

/// `SCIPpropCreate`: creates a propagator.
#[allow(clippy::too_many_arguments)]
pub fn prop_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    freq: i32,
    delay: bool,
    timingmask: PropTiming,
    presolpriority: i32,
    presolmaxrounds: i32,
    presoltiming: PresolTiming,
    methods: PropMethods,
    propdata: Option<Box<dyn Any>>,
) -> ScipResult<PropId> {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // The interface change from delay flags to timings cannot be recognized
    // at compile time: Exit with an appropriate error message.
    if !(PRESOLTIMING_NONE..=PRESOLTIMING_MAX).contains(&presoltiming) {
        log::error!("ERROR: 'PRESOLDELAY'-flag no longer available since SCIP 3.2, use an appropriate 'SCIP_PRESOLTIMING' for <{name}> propagator instead.");
        return Err(Retcode::ParameterWrongVal);
    }

    let prop = scip.props.push(Prop {
        name: name.to_string(),
        desc: desc.to_string(),
        priority,
        freq,
        propcopy: methods.propcopy,
        propfree: methods.propfree,
        propinit: methods.propinit,
        propexit: methods.propexit,
        propinitpre: methods.propinitpre,
        propexitpre: methods.propexitpre,
        propinitsol: methods.propinitsol,
        propexitsol: methods.propexitsol,
        proppresol: methods.proppresol,
        propexec: methods.propexec,
        propresprop: methods.propresprop,
        propdata,
        setuptime: Clock::new(),
        proptime: Clock::new(),
        sbproptime: Clock::new(),
        resproptime: Clock::new(),
        presoltime: Clock::new(),
        ncalls: 0,
        nrespropcalls: 0,
        ncutoffs: 0,
        ndomredsfound: 0,
        wasdelayed: false,
        exact: false,
        initialized: false,
        delay,
        timingmask,
        presolpriority,
        maxprerounds: presolmaxrounds,
        presoltiming,
        last: PresolCounts::default(),
        found: PresolCounts::default(),
        npresolcalls: 0,
    });

    // Add parameters.
    paramset_add_int(
        scip,
        &format!("propagating/{name}/priority"),
        &format!("priority of propagator <{name}>"),
        crate::field_target!(i32, props, prop, priority),
        true,
        priority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_prop_priority),
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{name}/freq"),
        &format!("frequency for calling propagator <{name}> (-1: never, 0: only in root node)"),
        crate::field_target!(i32, props, prop, freq),
        false,
        freq,
        -1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("propagating/{name}/delay"),
        "should propagator be delayed, if other propagators found reductions?",
        crate::field_target!(bool, props, prop, delay),
        true,
        delay,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{name}/timingmask"),
        &format!(
            "timing when propagator should be called ({}:BEFORELP, {}:DURINGLPLOOP, {}:AFTERLPLOOP, {}:ALWAYS))",
            1, 2, 4, PROPTIMING_ALWAYS
        ),
        crate::field_target!(i32, props, prop, timingmask),
        true,
        timingmask,
        PROPTIMING_NONE,
        PROPTIMING_ALWAYS,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{name}/presolpriority"),
        &format!("presolving priority of propagator <{name}>"),
        crate::field_target!(i32, props, prop, presolpriority),
        true,
        presolpriority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_prop_presol_priority),
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{name}/maxprerounds"),
        "maximal number of presolving rounds the propagator participates in (-1: no limit)",
        crate::field_target!(i32, props, prop, maxprerounds),
        false,
        presolmaxrounds,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("propagating/{name}/presoltiming"),
        &format!(
            "timing mask of the presolving method of propagator <{name}> ({}:FAST, {}:MEDIUM, {}:EXHAUSTIVE, {}:FINAL)",
            4, 8, 16, 32
        ),
        crate::field_target!(i32, props, prop, presoltiming),
        true,
        presoltiming,
        PRESOLTIMING_NONE,
        PRESOLTIMING_MAX,
        None,
    )?;
    Ok(prop)
}

/// `SCIPpropFree`: calls destructor and frees memory of propagator.
pub fn prop_free(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Call destructor of propagator.
    if let Some(free) = scip.props[prop].propfree {
        free(scip, prop)?;
    }
    scip.props.remove(prop);
    Ok(())
}

/// Runs one of the propagator's setup callbacks under its setup clock.
fn prop_setup_call(scip: &mut Scip, prop: PropId, method: Option<PropMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.props[prop].setuptime.start(enabled);
        method(scip, prop)?;
        // Stop timing.
        scip.props[prop].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPpropInit`: initializes propagator.
pub fn prop_init(scip: &mut Scip, prop: PropId) -> ScipResult {
    if scip.props[prop].initialized {
        log::error!("propagator <{}> already initialized", scip.props[prop].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let p = &mut scip.props[prop];
        p.proptime.reset();
        p.sbproptime.reset();
        p.resproptime.reset();
        p.presoltime.reset();
        p.setuptime.reset();

        p.ncalls = 0;
        p.nrespropcalls = 0;
        p.ncutoffs = 0;
        p.ndomredsfound = 0;
        p.last = PresolCounts::default();
        p.found = PresolCounts::default();
        p.npresolcalls = 0;
        p.wasdelayed = false;
    }

    let init = scip.props[prop].propinit;
    prop_setup_call(scip, prop, init)?;
    scip.props[prop].initialized = true;
    Ok(())
}

/// `SCIPpropExit`: calls exit method of propagator.
pub fn prop_exit(scip: &mut Scip, prop: PropId) -> ScipResult {
    if !scip.props[prop].initialized {
        log::error!("propagator <{}> not initialized", scip.props[prop].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.props[prop].propexit;
    prop_setup_call(scip, prop, exit)?;
    scip.props[prop].initialized = false;
    Ok(())
}

/// `SCIPpropInitpre`: informs propagator that the presolving process is
/// being started.
pub fn prop_initpre(scip: &mut Scip, prop: PropId) -> ScipResult {
    let p = &mut scip.props[prop];
    p.last = PresolCounts::default();
    p.wasdelayed = false;

    // Call presolving initialization method of propagator.
    let initpre = p.propinitpre;
    prop_setup_call(scip, prop, initpre)
}

/// `SCIPpropExitpre`: informs propagator that the presolving process is
/// finished.
pub fn prop_exitpre(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Call presolving deinitialization method of propagator.
    let exitpre = scip.props[prop].propexitpre;
    prop_setup_call(scip, prop, exitpre)
}

/// `SCIPpropInitsol`: informs propagator that the prop and bound process
/// is being started.
pub fn prop_initsol(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Call solving process initialization method of propagator.
    let initsol = scip.props[prop].propinitsol;
    prop_setup_call(scip, prop, initsol)
}

/// `SCIPpropExitsol`: informs propagator that the prop and bound process
/// data is being freed.
pub fn prop_exitsol(scip: &mut Scip, prop: PropId, restart: bool) -> ScipResult {
    // Call solving process deinitialization method of propagator.
    if let Some(exitsol) = scip.props[prop].propexitsol {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.props[prop].setuptime.start(enabled);
        exitsol(scip, prop, restart)?;
        // Stop timing.
        scip.props[prop].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPpropPresol`: executes presolving method of propagator.
pub fn prop_presol(scip: &mut Scip, prop: PropId, timing: PresolTiming, nrounds: i32, counts: &mut PresolCounts) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    let p = &scip.props[prop];
    let Some(presol) = p.proppresol else { return Ok(result) };

    // Check number of presolving rounds.
    if p.maxprerounds >= 0 && p.npresolcalls >= p.maxprerounds {
        return Ok(result);
    }

    // Check, if presolver should be delayed.
    if p.presoltiming & timing != 0 {
        // Calculate the number of changes since last call.
        let newcounts = counts.minus(&p.last);

        // Remember the number of changes prior to the call of the presolver
        // method of the propagator.
        scip.props[prop].last = *counts;

        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.props[prop].presoltime.start(enabled);

        // Call external method.
        presol(scip, prop, nrounds, timing, &newcounts, counts, &mut result)?;

        // Stop timing.
        scip.props[prop].presoltime.stop(enabled);

        // Add/count the new changes.
        let p = &mut scip.props[prop];
        let d = counts.minus(&p.last);
        p.found.add(&d);

        // Check result code of callback method.
        if !matches!(result, R::Cutoff | R::Unbounded | R::Success | R::DidNotFind | R::DidNotRun) {
            log::error!("propagator <{}> returned invalid result <{}>", p.name, result as i32);
            return Err(Retcode::InvalidResult);
        }

        // Increase the number of presolving calls, if the propagator tried to
        // find reductions.
        if result != R::DidNotRun {
            p.npresolcalls += 1;
        }
    }
    Ok(result)
}

/// `SCIPpropExec`: calls execution method of propagator.
pub fn prop_exec(scip: &mut Scip, prop: PropId, depth: i32, execdelayed: bool, instrongbranching: bool, proptiming: PropTiming) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result;
    let p = &scip.props[prop];

    if (depth == 0 && p.freq == 0) || (p.freq > 0 && depth % p.freq == 0) {
        if !p.delay || execdelayed {
            let stat = &scip.stat;
            let oldndomchgs = stat.nboundchgs + stat.nholechgs;
            let oldnprobdomchgs = stat.nprobboundchgs + stat.nprobholechgs;

            // Start timing.
            let enabled = scip.set.time_enabled;
            if instrongbranching {
                scip.props[prop].sbproptime.start(enabled);
            } else {
                scip.props[prop].proptime.start(enabled);
            }

            // Call external propagation method.
            result = R::DidNotRun;
            let exec = scip.props[prop].propexec.expect("an execution method");
            exec(scip, prop, proptiming, &mut result)?;

            // Stop timing.
            if instrongbranching {
                scip.props[prop].sbproptime.stop(enabled);
            } else {
                scip.props[prop].proptime.stop(enabled);
            }

            // Update statistics.
            let stat = &scip.stat;
            let ndomchgs = stat.nboundchgs + stat.nholechgs;
            let nprobdomchgs = stat.nprobboundchgs + stat.nprobholechgs;
            let p = &mut scip.props[prop];
            if result != R::DidNotRun && result != R::Delayed {
                p.ncalls += 1;
            }
            if result == R::Cutoff {
                p.ncutoffs += 1;
            }

            // Update domain reductions; therefore remove the domain reduction
            // counts which were generated in probing mode.
            p.ndomredsfound += ndomchgs - oldndomchgs;
            p.ndomredsfound -= nprobdomchgs - oldnprobdomchgs;

            // Evaluate result.
            if !matches!(result, R::Cutoff | R::ReducedDom | R::DidNotFind | R::DidNotRun | R::Delayed | R::DelayNode) {
                log::error!("execution method of propagator <{}> returned invalid result <{}>", p.name, result as i32);
                return Err(Retcode::InvalidResult);
            }
        } else {
            result = R::Delayed;
        }

        // Remember whether propagator was delayed.
        scip.props[prop].wasdelayed = result == R::Delayed;
    } else {
        result = R::DidNotRun;
    }
    Ok(result)
}

/// `SCIPpropResolvePropagation`: resolves the given conflicting bound, that
/// was deduced by the given propagator, by putting all "reason" bounds
/// leading to the deduction into the conflict queue; it is sufficient to
/// explain the relaxed bound change.
#[allow(clippy::too_many_arguments)]
pub fn prop_resolve_propagation(
    scip: &mut Scip,
    prop: PropId,
    infervar: VarId,
    inferinfo: i32,
    inferboundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    relaxedbd: f64,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    let Some(resprop) = scip.props[prop].propresprop else {
        log::error!("propagation conflict resolving method of propagator <{}> is not implemented", scip.props[prop].name);
        return Err(Retcode::PluginNotFound);
    };

    // Start timing.
    let enabled = scip.set.time_enabled;
    scip.props[prop].resproptime.start(enabled);

    resprop(scip, prop, infervar, inferinfo, inferboundtype, bdchgidx, relaxedbd, &mut result)?;

    // Stop timing.
    scip.props[prop].resproptime.stop(enabled);

    // Update statistic.
    scip.props[prop].nrespropcalls += 1;

    // Check result code.
    if result != R::Success && result != R::DidNotFind {
        log::error!(
            "propagation conflict resolving method of propagator <{}> returned invalid result <{}>",
            scip.props[prop].name,
            result as i32
        );
        return Err(Retcode::InvalidResult);
    }
    Ok(result)
}

/// `SCIPpropSetPresol`: sets presolving method of propagator.
pub fn prop_set_presol(scip: &mut Scip, prop: PropId, proppresol: PropPresol, presolpriority: i32, presolmaxrounds: i32, presoltiming: PresolTiming) -> ScipResult {
    let p = &mut scip.props[prop];
    p.proppresol = Some(proppresol);
    p.presolpriority = presolpriority;

    // The interface change from delay flags to timings cannot be recognized
    // at compile time: Exit with an appropriate error message.
    if !(PRESOLTIMING_FAST..=PRESOLTIMING_MAX).contains(&presoltiming) {
        log::error!(
            "ERROR: 'PRESOLDELAY'-flag no longer available since SCIP 3.2, use an appropriate 'SCIP_PRESOLTIMING' for <{}> constraint handler instead.",
            p.name
        );
        return Err(Retcode::ParameterWrongVal);
    }

    p.presoltiming = presoltiming;
    p.maxprerounds = presolmaxrounds;
    Ok(())
}

/// `SCIPpropSetPriority`: sets priority of propagator.
pub fn prop_set_priority(scip: &mut Scip, prop: PropId, priority: i32) {
    scip.props[prop].priority = priority;
    scip.set.propssorted = false;
}

/// `SCIPpropSetPresolPriority`: sets presolving priority of propagator.
pub fn prop_set_presol_priority(scip: &mut Scip, prop: PropId, presolpriority: i32) {
    scip.props[prop].presolpriority = presolpriority;
    scip.set.propspresolsorted = false;
}

/// `SCIPpropEnableOrDisableClocks`.
pub fn prop_enable_or_disable_clocks(scip: &mut Scip, prop: PropId, enable: bool) {
    let p = &mut scip.props[prop];
    p.setuptime.enable_or_disable(enable);
    p.presoltime.enable_or_disable(enable);
    p.proptime.enable_or_disable(enable);
    p.resproptime.enable_or_disable(enable);
    p.sbproptime.enable_or_disable(enable);
}

/// `SCIPpropDoesPresolve`: does the propagator perform presolving?
pub fn prop_does_presolve(scip: &Scip, prop: PropId) -> bool {
    scip.props[prop].proppresol.is_some()
}

/// `SCIPsetSortProps` (`set.c`): sorts propagators by priorities.
pub fn set_sort_props(scip: &mut Scip) {
    if !scip.set.propssorted {
        let n = scip.set.nprops as usize;
        let mut props = std::mem::take(&mut scip.set.props);
        crate::sort::sort(&mut props.0[..n], (), |&a, &b| prop_comp(scip, a, b));
        scip.set.props = props;
        scip.set.propssorted = true;
        scip.set.propsnamesorted = false;
    }
}

/// `SCIPsetSortPropsPresol` (`set.c`): sorts propagators by priorities
/// for presolving.
pub fn set_sort_props_presol(scip: &mut Scip) {
    if !scip.set.propspresolsorted {
        let n = scip.set.nprops as usize;
        let mut props = std::mem::take(&mut scip.set.props_presol);
        crate::sort::sort(&mut props.0[..n], (), |&a, &b| prop_comp_presol(scip, a, b));
        scip.set.props_presol = props;
        scip.set.propspresolsorted = true;
        scip.set.propsnamesorted = false;
    }
}
