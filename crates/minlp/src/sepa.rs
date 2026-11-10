//! SCIP's separators (`sepa.c`, `struct_sepa.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ScipResultCode, SepaId, SolId},
    var::NONE,
};

pub type SepaMethod = fn(&mut Scip, SepaId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type SepaCopy = fn(&mut Scip, &Scip, SepaId) -> ScipResult;
/// `SCIP_DECL_SEPAEXECLP`: `(result, allowlocal, depth)`.
pub type SepaExecLp = fn(&mut Scip, SepaId, &mut ScipResultCode, bool, i32) -> ScipResult;
/// `SCIP_DECL_SEPAEXECSOL`: `(sol, result, allowlocal, depth)`.
pub type SepaExecSol = fn(&mut Scip, SepaId, SolId, &mut ScipResultCode, bool, i32) -> ScipResult;

/// `SCIP_SEPA`.
pub struct Sepa {
    pub lastsepanode: i64,
    pub ncalls: i64,
    pub nrootcalls: i64,
    pub ncutoffs: i64,
    pub ncutsfound: i64,
    pub ncutsadded: i64,
    pub ncutsaddedviapool: i64,
    pub ncutsaddeddirect: i64,
    pub ncutsappliedviapool: i64,
    pub ncutsapplieddirect: i64,
    pub nconssfound: i64,
    pub ndomredsfound: i64,
    pub maxbounddist: f64,
    pub name: String,
    pub desc: String,
    pub sepacopy: Option<SepaCopy>,
    pub sepafree: Option<SepaMethod>,
    pub sepainit: Option<SepaMethod>,
    pub sepaexit: Option<SepaMethod>,
    pub sepainitsol: Option<SepaMethod>,
    pub sepaexitsol: Option<SepaMethod>,
    pub sepaexeclp: Option<SepaExecLp>,
    pub sepaexecsol: Option<SepaExecSol>,
    pub sepadata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub sepaclock: Clock,
    pub priority: i32,
    pub freq: i32,
    pub ncallsatnode: i32,
    pub ncutsfoundatnode: i32,
    pub expbackoff: i32,
    pub usessubscip: bool,
    pub delay: bool,
    pub lpwasdelayed: bool,
    pub solwasdelayed: bool,
    pub exact: bool,
    pub initialized: bool,
    pub isparentsepa: bool,
    pub parentsepa: SepaId,
}

/// `SCIPsepaComp`: compares two separators w. r. to their priority.
pub fn sepa_comp(scip: &Scip, elem1: SepaId, elem2: SepaId) -> i32 {
    scip.sepas[elem2].priority - scip.sepas[elem1].priority
}

/// `SCIPsepaCompName`: comparison method for sorting separators w.r.t. to
/// their name.
pub fn sepa_comp_name(scip: &Scip, elem1: SepaId, elem2: SepaId) -> std::cmp::Ordering {
    scip.sepas[elem1].name.as_bytes().cmp(scip.sepas[elem2].name.as_bytes())
}

/// `paramChgdSepaPriority`: the parameter already wrote the priority; mark
/// the separators unsorted (`SCIPsetSepaPriority`).
fn param_chgd_sepa_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.sepassorted = false;
    Ok(())
}

/// `SCIPsepaCopyInclude`: copies the given separator to a new scip.
pub fn sepa_copy_include(scip: &mut Scip, sourcescip: &Scip, sepa: SepaId) -> ScipResult {
    if let Some(copy) = sourcescip.sepas[sepa].sepacopy {
        copy(scip, sourcescip, sepa)?;
    }
    Ok(())
}

/// The callbacks `SCIPsepaCreate` takes.
#[derive(Default)]
pub struct SepaMethods {
    pub sepacopy: Option<SepaCopy>,
    pub sepafree: Option<SepaMethod>,
    pub sepainit: Option<SepaMethod>,
    pub sepaexit: Option<SepaMethod>,
    pub sepainitsol: Option<SepaMethod>,
    pub sepaexitsol: Option<SepaMethod>,
    pub sepaexeclp: Option<SepaExecLp>,
    pub sepaexecsol: Option<SepaExecSol>,
}

/// `SCIPsepaCreate`: creates a separator. `maxbounddist` is the maximal
/// relative distance from current node's dual bound to primal bound
/// compared to best node's dual bound for applying separation.
#[allow(clippy::too_many_arguments)]
pub fn sepa_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    freq: i32,
    maxbounddist: f64,
    usessubscip: bool,
    delay: bool,
    methods: SepaMethods,
    sepadata: Option<Box<dyn Any>>,
) -> ScipResult<SepaId> {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    let sepa = scip.sepas.push(Sepa {
        name: name.to_string(),
        desc: desc.to_string(),
        priority,
        freq,
        maxbounddist,
        usessubscip,
        sepacopy: methods.sepacopy,
        sepafree: methods.sepafree,
        sepainit: methods.sepainit,
        sepaexit: methods.sepaexit,
        sepainitsol: methods.sepainitsol,
        sepaexitsol: methods.sepaexitsol,
        sepaexeclp: methods.sepaexeclp,
        sepaexecsol: methods.sepaexecsol,
        sepadata,
        setuptime: Clock::new(),
        sepaclock: Clock::new(),
        lastsepanode: -1,
        ncalls: 0,
        nrootcalls: 0,
        ncutoffs: 0,
        ncutsfound: 0,
        ncutsadded: 0,
        ncutsaddedviapool: 0,
        ncutsaddeddirect: 0,
        ncutsappliedviapool: 0,
        ncutsapplieddirect: 0,
        nconssfound: 0,
        ndomredsfound: 0,
        ncallsatnode: 0,
        ncutsfoundatnode: 0,
        lpwasdelayed: false,
        solwasdelayed: false,
        exact: false,
        initialized: false,
        isparentsepa: false,
        parentsepa: NONE,
        delay,
        expbackoff: 4,
    });

    // Add parameters.
    paramset_add_int(
        scip,
        &format!("separating/{name}/priority"),
        &format!("priority of separator <{name}>"),
        crate::field_target!(i32, sepas, sepa, priority),
        true,
        priority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_sepa_priority),
    )?;
    paramset_add_int(
        scip,
        &format!("separating/{name}/freq"),
        &format!("frequency for calling separator <{name}> (-1: never, 0: only in root node)"),
        crate::field_target!(i32, sepas, sepa, freq),
        false,
        freq,
        -1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("separating/{name}/maxbounddist"),
        &format!(
            "maximal relative distance from current node's dual bound to primal bound compared to best node's dual bound for applying separator <{name}> (0.0: only on current best node, 1.0: on all nodes)"
        ),
        crate::field_target!(f64, sepas, sepa, maxbounddist),
        true,
        maxbounddist,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("separating/{name}/delay"),
        "should separator be delayed, if other separators found cuts?",
        crate::field_target!(bool, sepas, sepa, delay),
        true,
        delay,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("separating/{name}/expbackoff"),
        &format!("base for exponential increase of frequency at which separator <{name}> is called (1: call at each multiple of frequency)"),
        crate::field_target!(i32, sepas, sepa, expbackoff),
        true,
        4,
        1,
        100,
        None,
    )?;
    Ok(sepa)
}

/// `SCIPsepaFree`: calls destructor and frees memory of separator.
pub fn sepa_free(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Call destructor of separator.
    if let Some(free) = scip.sepas[sepa].sepafree {
        free(scip, sepa)?;
    }
    scip.sepas.remove(sepa);
    Ok(())
}

/// Runs one of the separator's setup callbacks under its setup clock.
fn sepa_setup_call(scip: &mut Scip, sepa: SepaId, method: Option<SepaMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.sepas[sepa].setuptime.start(enabled);
        method(scip, sepa)?;
        // Stop timing.
        scip.sepas[sepa].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPsepaInit`: initializes separator.
pub fn sepa_init(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    if scip.sepas[sepa].initialized {
        log::error!("separator <{}> already initialized", scip.sepas[sepa].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let s = &mut scip.sepas[sepa];
        s.setuptime.reset();
        s.sepaclock.reset();

        s.lastsepanode = -1;
        s.ncalls = 0;
        s.nrootcalls = 0;
        s.ncutoffs = 0;
        s.ncutsfound = 0;
        s.ncutsadded = 0;
        s.ncutsaddedviapool = 0;
        s.ncutsaddeddirect = 0;
        s.ncutsappliedviapool = 0;
        s.ncutsapplieddirect = 0;
        s.nconssfound = 0;
        s.ndomredsfound = 0;
        s.ncallsatnode = 0;
        s.ncutsfoundatnode = 0;
        s.lpwasdelayed = false;
        s.solwasdelayed = false;
    }

    let init = scip.sepas[sepa].sepainit;
    sepa_setup_call(scip, sepa, init)?;
    scip.sepas[sepa].initialized = true;
    Ok(())
}

/// `SCIPsepaExit`: calls exit method of separator.
pub fn sepa_exit(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    if !scip.sepas[sepa].initialized {
        log::error!("separator <{}> not initialized", scip.sepas[sepa].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.sepas[sepa].sepaexit;
    sepa_setup_call(scip, sepa, exit)?;
    scip.sepas[sepa].initialized = false;
    Ok(())
}

/// `SCIPsepaInitsol`: informs separator that the branch and bound process
/// is being started.
pub fn sepa_initsol(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    scip.sepas[sepa].lpwasdelayed = false;
    scip.sepas[sepa].solwasdelayed = false;

    // Call solving process initialization method of separator.
    let initsol = scip.sepas[sepa].sepainitsol;
    sepa_setup_call(scip, sepa, initsol)
}

/// `SCIPsepaExitsol`: informs separator that the branch and bound process
/// data is being freed.
pub fn sepa_exitsol(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Call solving process deinitialization method of separator.
    let exitsol = scip.sepas[sepa].sepaexitsol;
    sepa_setup_call(scip, sepa, exitsol)
}

/// The statistics `SCIPsepaExecLP` and `SCIPsepaExecSol` take before the
/// call and settle after it.
struct ExecCounts {
    oldndomchgs: i64,
    oldnprobdomchgs: i64,
    oldncutsfound: i32,
    oldnactiveconss: i32,
}

fn exec_update_stats(scip: &mut Scip, sepa: SepaId, depth: i32, result: ScipResultCode, before: &ExecCounts, ncutsnow: i32) -> ScipResult {
    use ScipResultCode as R;
    let stat = &scip.stat;
    let ntotalnodes = stat.ntotalnodes;
    let ndomchgs = stat.nboundchgs + stat.nholechgs;
    let nprobdomchgs = stat.nprobboundchgs + stat.nprobholechgs;
    let nactiveconss = stat.nactiveconss;
    let s = &mut scip.sepas[sepa];

    // Update statistics.
    if result != R::DidNotRun && result != R::Delayed {
        s.ncalls += 1;
        if depth == 0 {
            s.nrootcalls += 1;
        }
        s.ncallsatnode += 1;
        s.lastsepanode = ntotalnodes;
    }
    if result == R::Cutoff {
        s.ncutoffs += 1;
    }

    let ncutsfound = ncutsnow - before.oldncutsfound;
    s.ncutsfound += ncutsfound as i64;
    s.ncutsfoundatnode += ncutsfound;
    s.nconssfound += crate::def::max(nactiveconss - before.oldnactiveconss, 0) as i64;

    // Update domain reductions; therefore remove the domain reduction counts
    // which were generated in probing mode.
    s.ndomredsfound += ndomchgs - before.oldndomchgs;
    s.ndomredsfound -= nprobdomchgs - before.oldnprobdomchgs;

    // Evaluate result.
    if !matches!(
        result,
        R::Cutoff | R::ConsAdded | R::ReducedDom | R::Separated | R::NewRound | R::DidNotFind | R::DidNotRun | R::Delayed
    ) {
        log::error!("execution method of separator <{}> returned invalid result <{}>", s.name, result as i32);
        return Err(Retcode::InvalidResult);
    }
    Ok(())
}

fn exec_counts(scip: &Scip, ncuts: i32) -> ExecCounts {
    let stat = &scip.stat;
    ExecCounts {
        oldndomchgs: stat.nboundchgs + stat.nholechgs,
        oldnprobdomchgs: stat.nprobboundchgs + stat.nprobholechgs,
        oldncutsfound: ncuts,
        oldnactiveconss: stat.nactiveconss,
    }
}

/// The number of cuts `SCIPsepaExecLP` counts: the storage's and both
/// global cut pools'.
fn lp_ncuts(scip: &Scip, sepastore: usize) -> i32 {
    scip.sepastores[sepastore].ncuts
        + crate::cutpool::cutpool_get_ncuts(scip, crate::cutpool::GLOBALCUTPOOL)
        + crate::cutpool::cutpool_get_ncuts(scip, crate::cutpool::DELAYEDCUTPOOL)
}

/// `SCIPsepaExecLP`: calls LP separation method of separator.
pub fn sepa_exec_lp(scip: &mut Scip, sepa: SepaId, sepastore: usize, depth: i32, bounddist: f64, allowlocal: bool, execdelayed: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result;
    let s = &scip.sepas[sepa];

    if s.sepaexeclp.is_some()
        && scip.set.is_le(bounddist, s.maxbounddist)
        && ((depth == 0 && s.freq != -1)
            || (s.freq > 0 && depth % s.freq == 0 && (s.expbackoff == 1 || scip.set.is_integral((depth as f64 * (1.0 / s.freq as f64)).log2() / (s.expbackoff as f64).log2())))
            || s.lpwasdelayed)
    {
        if (!s.delay && !s.lpwasdelayed) || execdelayed {
            let before = exec_counts(scip, lp_ncuts(scip, sepastore));

            // Reset the statistics for current node.
            if scip.sepas[sepa].lastsepanode != scip.stat.ntotalnodes {
                scip.sepas[sepa].ncallsatnode = 0;
                scip.sepas[sepa].ncutsfoundatnode = 0;
            }

            // Start timing.
            let enabled = scip.set.time_enabled;
            scip.sepas[sepa].sepaclock.start(enabled);

            // Call external separation method.
            result = R::DidNotRun;
            let exec = scip.sepas[sepa].sepaexeclp.unwrap();
            exec(scip, sepa, &mut result, allowlocal, depth)?;

            // Stop timing.
            scip.sepas[sepa].sepaclock.stop(enabled);

            let ncutsnow = lp_ncuts(scip, sepastore);
            exec_update_stats(scip, sepa, depth, result, &before, ncutsnow)?;
        } else {
            result = R::Delayed;
        }

        // Remember whether separator was delayed.
        scip.sepas[sepa].lpwasdelayed = result == R::Delayed;
    } else {
        result = R::DidNotRun;
    }
    Ok(result)
}

/// `SCIPsepaExecSol`: calls primal solution separation method of
/// separator.
#[allow(clippy::too_many_arguments)]
pub fn sepa_exec_sol(scip: &mut Scip, sepa: SepaId, sepastore: usize, sol: SolId, depth: i32, allowlocal: bool, execdelayed: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result;
    let s = &scip.sepas[sepa];

    // The C puts the second LOG2 inside the first here (unlike
    // SCIPsepaExecLP); kept.
    if s.sepaexecsol.is_some()
        && ((depth == 0 && s.freq != -1)
            || (s.freq > 0 && depth % s.freq == 0 && (s.expbackoff == 1 || scip.set.is_integral((depth as f64 * (1.0 / s.freq as f64) / (s.expbackoff as f64).log2()).log2())))
            || s.solwasdelayed)
    {
        if (!s.delay && !s.solwasdelayed) || execdelayed {
            let before = exec_counts(scip, scip.sepastores[sepastore].ncuts);

            // Reset the statistics for current node.
            if scip.sepas[sepa].lastsepanode != scip.stat.ntotalnodes {
                scip.sepas[sepa].ncallsatnode = 0;
                scip.sepas[sepa].ncutsfoundatnode = 0;
            }

            // Start timing.
            let enabled = scip.set.time_enabled;
            scip.sepas[sepa].sepaclock.start(enabled);

            // Call external separation method.
            result = R::DidNotRun;
            let exec = scip.sepas[sepa].sepaexecsol.unwrap();
            exec(scip, sepa, sol, &mut result, allowlocal, depth)?;

            // Stop timing.
            scip.sepas[sepa].sepaclock.stop(enabled);

            let ncutsnow = scip.sepastores[sepastore].ncuts;
            exec_update_stats(scip, sepa, depth, result, &before, ncutsnow)?;
        } else {
            result = R::Delayed;
        }

        // Remember whether separator was delayed.
        scip.sepas[sepa].solwasdelayed = result == R::Delayed;
    } else {
        result = R::DidNotRun;
    }
    Ok(result)
}

/// `SCIPsepaSetPriority`: sets priority of separator.
pub fn sepa_set_priority(scip: &mut Scip, sepa: SepaId, priority: i32) {
    scip.sepas[sepa].priority = priority;
    scip.set.sepassorted = false;
}

/// `SCIPsepaEnableOrDisableClocks`.
pub fn sepa_enable_or_disable_clocks(scip: &mut Scip, sepa: SepaId, enable: bool) {
    let s = &mut scip.sepas[sepa];
    s.setuptime.enable_or_disable(enable);
    s.sepaclock.enable_or_disable(enable);
}

/// `SCIPsepaIncNCutsApplied`: increase count of applied cuts by one.
pub fn sepa_inc_ncuts_applied(scip: &mut Scip, sepa: SepaId, fromcutpool: bool) {
    let s = &mut scip.sepas[sepa];
    if fromcutpool {
        s.ncutsappliedviapool += 1;
    } else {
        s.ncutsapplieddirect += 1;
    }

    let parentsepa = s.parentsepa;
    if parentsepa != NONE {
        sepa_inc_ncuts_applied(scip, parentsepa, fromcutpool);
    }
}

/// `SCIPsepaIncNCutsAdded`: increase count of added cuts by one.
pub fn sepa_inc_ncuts_added(scip: &mut Scip, sepa: SepaId, fromcutpool: bool) {
    let s = &mut scip.sepas[sepa];
    s.ncutsadded += 1;
    if fromcutpool {
        s.ncutsaddedviapool += 1;
    } else {
        s.ncutsaddeddirect += 1;
    }

    let parentsepa = s.parentsepa;
    if parentsepa != NONE {
        sepa_inc_ncuts_added(scip, parentsepa, fromcutpool);
    }
}

/// `SCIPsepaDecNCutsAdded`: decrease the count of added cuts by one.
pub fn sepa_dec_ncuts_added(scip: &mut Scip, sepa: SepaId, fromcutpool: bool) {
    let s = &mut scip.sepas[sepa];
    s.ncutsadded -= 1;
    if fromcutpool {
        s.ncutsaddedviapool -= 1;
    } else {
        s.ncutsaddeddirect -= 1;
    }

    let parentsepa = s.parentsepa;
    if parentsepa != NONE {
        sepa_dec_ncuts_added(scip, parentsepa, fromcutpool);
    }
}

/// `SCIPsepaIncNCutsFound`: increase count of found cuts by one.
pub fn sepa_inc_ncuts_found(scip: &mut Scip, sepa: SepaId) {
    scip.sepas[sepa].ncutsfound += 1;
}

/// `SCIPsepaIncNCutsFoundAtNode`: increase count of found cuts at current
/// node by one.
pub fn sepa_inc_ncuts_found_at_node(scip: &mut Scip, sepa: SepaId) {
    scip.sepas[sepa].ncutsfoundatnode += 1;
}

/// `SCIPsepaWasLPDelayed`: was separation of the LP solution delayed at
/// the last call?
pub fn sepa_was_lp_delayed(scip: &Scip, sepa: SepaId) -> bool {
    scip.sepas[sepa].lpwasdelayed
}

/// `SCIPsepaWasSolDelayed`: was separation of the primal solution delayed
/// at the last call?
pub fn sepa_was_sol_delayed(scip: &Scip, sepa: SepaId) -> bool {
    scip.sepas[sepa].solwasdelayed
}

/// `SCIPsetSortSepas` (`set.c`): sorts separators by priorities.
pub fn set_sort_sepas(scip: &mut Scip) {
    if !scip.set.sepassorted {
        let n = scip.set.nsepas as usize;
        let mut sepas = std::mem::take(&mut scip.set.sepas);
        crate::sort::sort(&mut sepas.0[..n], (), |&a, &b| sepa_comp(scip, a, b));
        scip.set.sepas = sepas;
        scip.set.sepassorted = true;
        scip.set.sepasnamesorted = false;
    }
}
