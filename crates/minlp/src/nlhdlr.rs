//! Port of `nlhdlr.c` (and `type_nlhdlr.h`, `struct_nlhdlr.h`): functions
//! for nonlinear handlers.
//!
//! A nonlinear handler's expression data lives in
//! [`Scip::nlhdlrexprdatas`]; callbacks get its id (`NONE` for NULL).

use std::any::Any;

use crate::{
    clock::Clock,
    expr::{ExprId, ExprIntEvalVar},
    interval::Interval,
    paramset::paramset_add_bool,
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, ScipResultCode, SolId},
};

/// Id of a nonlinear handler in [`Scip::nlhdlrs`].
pub type NlhdlrId = usize;

/// `SCIP_NLHDLR_METHOD`: nlhdlr methods bitflags.
pub type NlhdlrMethod = u32;
/// No enforcement.
pub const NLHDLR_METHOD_NONE: NlhdlrMethod = 0x0;
/// Separation for expr <= auxvar, thus might estimate expr from below.
pub const NLHDLR_METHOD_SEPABELOW: NlhdlrMethod = 0x1;
/// Separation for expr >= auxvar, thus might estimate expr from above.
pub const NLHDLR_METHOD_SEPAABOVE: NlhdlrMethod = 0x2;
/// Separation for expr == auxvar.
pub const NLHDLR_METHOD_SEPABOTH: NlhdlrMethod = NLHDLR_METHOD_SEPABELOW | NLHDLR_METHOD_SEPAABOVE;
/// Activity computation (interval evaluation) and propagation (reverse
/// propagation).
pub const NLHDLR_METHOD_ACTIVITY: NlhdlrMethod = 0x4;
/// All enforcement methods.
pub const NLHDLR_METHOD_ALL: NlhdlrMethod = NLHDLR_METHOD_SEPABOTH | NLHDLR_METHOD_ACTIVITY;

/// `SCIP_DECL_NLHDLRCOPYHDLR`: (targetscip, targetconshdlr, sourcescip,
/// sourceconshdlr, sourcenlhdlr).
pub type NlhdlrCopyHdlr = fn(&mut Scip, ConshdlrId, &Scip, ConshdlrId, NlhdlrId) -> ScipResult;
/// `SCIP_DECL_NLHDLRFREEHDLRDATA`.
pub type NlhdlrFreeHdlrData = fn(&mut Scip, NlhdlrId) -> ScipResult;
/// `SCIP_DECL_NLHDLRFREEEXPRDATA`: (nlhdlr, expr, nlhdlrexprdata id).
pub type NlhdlrFreeExprData = fn(&mut Scip, NlhdlrId, ExprId, &mut usize) -> ScipResult;
/// `SCIP_DECL_NLHDLRINIT`.
pub type NlhdlrInit = fn(&mut Scip, NlhdlrId) -> ScipResult;
/// `SCIP_DECL_NLHDLREXIT`.
pub type NlhdlrExit = fn(&mut Scip, NlhdlrId) -> ScipResult;
/// `SCIP_DECL_NLHDLRDETECT`: (conshdlr, nlhdlr, expr, cons, enforcing,
/// participating, nlhdlrexprdata id).
pub type NlhdlrDetect = fn(&mut Scip, ConshdlrId, NlhdlrId, ExprId, ConsId, &mut NlhdlrMethod, &mut NlhdlrMethod, &mut usize) -> ScipResult;
/// `SCIP_DECL_NLHDLREVALAUX`: (nlhdlr, expr, nlhdlrexprdata, auxvalue, sol).
pub type NlhdlrEvalAux = fn(&mut Scip, NlhdlrId, ExprId, usize, &mut f64, SolId) -> ScipResult;
/// `SCIP_DECL_NLHDLRINTEVAL`: (nlhdlr, expr, nlhdlrexprdata, interval,
/// intevalvar, intevalvardata).
pub type NlhdlrIntEval = fn(&mut Scip, NlhdlrId, ExprId, usize, &mut Interval, Option<ExprIntEvalVar>, usize) -> ScipResult;
/// `SCIP_DECL_NLHDLRREVERSEPROP`: (conshdlr, nlhdlr, expr, nlhdlrexprdata,
/// bounds, infeasible, nreductions).
pub type NlhdlrReverseProp = fn(&mut Scip, ConshdlrId, NlhdlrId, ExprId, usize, Interval, &mut bool, &mut i32) -> ScipResult;
/// `SCIP_DECL_NLHDLRINITSEPA`: (conshdlr, cons, nlhdlr, expr,
/// nlhdlrexprdata, overestimate, underestimate, infeasible).
pub type NlhdlrInitSepa = fn(&mut Scip, ConshdlrId, ConsId, NlhdlrId, ExprId, usize, bool, bool, &mut bool) -> ScipResult;
/// `SCIP_DECL_NLHDLREXITSEPA`: (nlhdlr, expr, nlhdlrexprdata).
pub type NlhdlrExitSepa = fn(&mut Scip, NlhdlrId, ExprId, usize) -> ScipResult;
/// `SCIP_DECL_NLHDLRENFO`: (conshdlr, cons, nlhdlr, expr, nlhdlrexprdata,
/// sol, auxvalue, overestimate, allowweakcuts, separated, addbranchscores,
/// branchcandonly, result).
pub type NlhdlrEnfo = fn(&mut Scip, ConshdlrId, ConsId, NlhdlrId, ExprId, usize, SolId, f64, bool, bool, bool, bool, bool, &mut ScipResultCode) -> ScipResult;
/// `SCIP_DECL_NLHDLRESTIMATE`: (conshdlr, nlhdlr, expr, nlhdlrexprdata,
/// sol, auxvalue, overestimate, targetvalue, addbranchscores, rowpreps,
/// success, addedbranchscores).
///
/// The rowpreps (`SCIP_PTRARRAY`) are filled from index 0 on.
pub type NlhdlrEstimate = fn(&mut Scip, ConshdlrId, NlhdlrId, ExprId, usize, SolId, f64, bool, f64, bool, &mut Vec<RowPrep>, &mut bool, &mut bool) -> ScipResult;
/// `SCIP_DECL_NLHDLRSOLLINEARIZE`: (conshdlr, cons, nlhdlr, expr,
/// nlhdlrexprdata, sol, solisbest, overestimate, underestimate).
pub type NlhdlrSolLinearize = fn(&mut Scip, ConshdlrId, ConsId, NlhdlrId, ExprId, usize, SolId, bool, bool, bool) -> ScipResult;

/// `SCIP_NLHDLR`: nonlinear handler.
pub struct Nlhdlr {
    /// Nonlinear handler name.
    pub name: String,
    /// Nonlinear handler description (can be NULL).
    pub desc: Option<String>,
    /// Data of handler.
    pub data: Option<Box<dyn Any>>,

    /// Detection priority of nonlinear handler.
    pub detectpriority: i32,
    /// Enforcement priority of nonlinear handler.
    pub enfopriority: i32,
    /// Whether the nonlinear handler should be used.
    pub enabled: bool,

    /* callbacks */
    pub freehdlrdata: Option<NlhdlrFreeHdlrData>,
    pub freeexprdata: Option<NlhdlrFreeExprData>,
    pub copyhdlr: Option<NlhdlrCopyHdlr>,
    pub init: Option<NlhdlrInit>,
    pub exit: Option<NlhdlrExit>,
    pub detect: NlhdlrDetect,
    pub evalaux: NlhdlrEvalAux,
    pub initsepa: Option<NlhdlrInitSepa>,
    pub enfo: Option<NlhdlrEnfo>,
    pub estimate: Option<NlhdlrEstimate>,
    pub exitsepa: Option<NlhdlrExitSepa>,
    pub inteval: Option<NlhdlrIntEval>,
    pub reverseprop: Option<NlhdlrReverseProp>,
    pub sollinearize: Option<NlhdlrSolLinearize>,

    /* statistics */
    /// Number of times, the enforcement or estimation callback was called.
    pub nenfocalls: i64,
    /// Number of times, the interval evaluation callback was called.
    pub nintevalcalls: i64,
    /// Number of times, the propagation callback was called.
    pub npropcalls: i64,
    /// Number of times, the nonlinear handler enforced by separation.
    pub nseparated: i64,
    /// Number of cutoffs found so far by this nonlinear handler.
    pub ncutoffs: i64,
    /// Number of domain reductions found so far by this nonlinear handler.
    pub ndomreds: i64,
    /// Number of detect calls in which structure was detected (over all
    /// runs).
    pub ndetections: i64,
    /// Number of detect calls in which structure was detected (in last
    /// round).
    pub ndetectionslast: i64,
    /// Number of times, branching scores were added by this nonlinear
    /// handler.
    pub nbranchscores: i64,

    /// Time used for detection.
    pub detecttime: Clock,
    /// Time used for enforcement or estimation.
    pub enfotime: Clock,
    /// Time used for reverse propagation.
    pub proptime: Clock,
    /// Time used for interval evaluation.
    pub intevaltime: Clock,
}

/* setters */

/// `SCIPnlhdlrSetCopyHdlr`.
pub fn nlhdlr_set_copy_hdlr(scip: &mut Scip, nlhdlr: NlhdlrId, copy: Option<NlhdlrCopyHdlr>) {
    scip.nlhdlrs[nlhdlr].copyhdlr = copy;
}

/// `SCIPnlhdlrSetFreeHdlrData`.
pub fn nlhdlr_set_free_hdlr_data(scip: &mut Scip, nlhdlr: NlhdlrId, freehdlrdata: Option<NlhdlrFreeHdlrData>) {
    scip.nlhdlrs[nlhdlr].freehdlrdata = freehdlrdata;
}

/// `SCIPnlhdlrSetFreeExprData`.
pub fn nlhdlr_set_free_expr_data(scip: &mut Scip, nlhdlr: NlhdlrId, freeexprdata: Option<NlhdlrFreeExprData>) {
    scip.nlhdlrs[nlhdlr].freeexprdata = freeexprdata;
}

/// `SCIPnlhdlrSetInitExit`.
pub fn nlhdlr_set_init_exit(scip: &mut Scip, nlhdlr: NlhdlrId, init: Option<NlhdlrInit>, exit: Option<NlhdlrExit>) {
    scip.nlhdlrs[nlhdlr].init = init;
    scip.nlhdlrs[nlhdlr].exit = exit;
}

/// `SCIPnlhdlrSetProp`.
pub fn nlhdlr_set_prop(scip: &mut Scip, nlhdlr: NlhdlrId, inteval: Option<NlhdlrIntEval>, reverseprop: Option<NlhdlrReverseProp>) {
    scip.nlhdlrs[nlhdlr].inteval = inteval;
    scip.nlhdlrs[nlhdlr].reverseprop = reverseprop;
}

/// `SCIPnlhdlrSetSepa`.
pub fn nlhdlr_set_sepa(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    initsepa: Option<NlhdlrInitSepa>,
    enfo: Option<NlhdlrEnfo>,
    estimate: Option<NlhdlrEstimate>,
    exitsepa: Option<NlhdlrExitSepa>,
) {
    let h = &mut scip.nlhdlrs[nlhdlr];
    h.initsepa = initsepa;
    h.enfo = enfo;
    h.estimate = estimate;
    h.exitsepa = exitsepa;
}

/// `SCIPnlhdlrSetSollinearize`.
pub fn nlhdlr_set_sollinearize(scip: &mut Scip, nlhdlr: NlhdlrId, sollinearize: Option<NlhdlrSolLinearize>) {
    scip.nlhdlrs[nlhdlr].sollinearize = sollinearize;
}

/// `SCIPnlhdlrComp`: compares two nonlinear handlers by detection priority;
/// if handlers have same detection priority, then compare by name.
pub fn nlhdlr_comp(scip: &Scip, elem1: NlhdlrId, elem2: NlhdlrId) -> i32 {
    let h1 = &scip.nlhdlrs[elem1];
    let h2 = &scip.nlhdlrs[elem2];

    if h1.detectpriority != h2.detectpriority {
        return h1.detectpriority - h2.detectpriority;
    }

    h1.name.as_bytes().cmp(h2.name.as_bytes()) as i32
}

/* nlhdlr private API functions from nlhdlr.h */

/// `SCIPnlhdlrCreate`: creates a nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_create(
    scip: &mut Scip,
    name: &str,
    desc: Option<&str>,
    detectpriority: i32,
    enfopriority: i32,
    detect: NlhdlrDetect,
    evalaux: NlhdlrEvalAux,
    nlhdlrdata: Option<Box<dyn Any>>,
) -> ScipResult<NlhdlrId> {
    let nlhdlr = scip.nlhdlrs.push(Nlhdlr {
        name: name.to_string(),
        desc: desc.map(str::to_string),
        data: nlhdlrdata,
        detectpriority,
        enfopriority,
        enabled: false,
        freehdlrdata: None,
        freeexprdata: None,
        copyhdlr: None,
        init: None,
        exit: None,
        detect,
        evalaux,
        initsepa: None,
        enfo: None,
        estimate: None,
        exitsepa: None,
        inteval: None,
        reverseprop: None,
        sollinearize: None,
        nenfocalls: 0,
        nintevalcalls: 0,
        npropcalls: 0,
        nseparated: 0,
        ncutoffs: 0,
        ndomreds: 0,
        ndetections: 0,
        ndetectionslast: 0,
        nbranchscores: 0,
        detecttime: Clock::new(),
        enfotime: Clock::new(),
        proptime: Clock::new(),
        intevaltime: Clock::new(),
    });

    let target = Some(std::rc::Rc::new(crate::paramset::Target::<bool> {
        get: Box::new(move |scip: &Scip| &scip.nlhdlrs[nlhdlr].enabled),
        get_mut: Box::new(move |scip: &mut Scip| &mut scip.nlhdlrs[nlhdlr].enabled),
    }));
    paramset_add_bool(scip, &format!("nlhdlr/{name}/enabled"), "should this nonlinear handler be used", target, false, true, None)?;

    Ok(nlhdlr)
}

/// `SCIPnlhdlrFree`: frees a nonlinear handler.
pub fn nlhdlr_free(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    if let Some(freehdlrdata) = scip.nlhdlrs[nlhdlr].freehdlrdata {
        freehdlrdata(scip, nlhdlr)?;
    }

    scip.nlhdlrs.remove(nlhdlr);

    Ok(())
}

/// `SCIPnlhdlrCopyhdlr`: call the handler copy callback of a nonlinear
/// handler.
pub fn nlhdlr_copyhdlr(targetscip: &mut Scip, targetconshdlr: ConshdlrId, sourcescip: &Scip, sourceconshdlr: ConshdlrId, sourcenlhdlr: NlhdlrId) -> ScipResult {
    // TODO for now just don't copy disabled nlhdlr, a clean way would probably
    // be to first copy and disable then.
    let h = &sourcescip.nlhdlrs[sourcenlhdlr];
    if let Some(copyhdlr) = h.copyhdlr
        && h.enabled
    {
        copyhdlr(targetscip, targetconshdlr, sourcescip, sourceconshdlr, sourcenlhdlr)?;
    }

    Ok(())
}

/// `SCIPnlhdlrFreeexprdata`: call the free expression specific data
/// callback of a nonlinear handler.
pub fn nlhdlr_freeexprdata(scip: &mut Scip, nlhdlr: NlhdlrId, expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    if let Some(freeexprdata) = scip.nlhdlrs[nlhdlr].freeexprdata {
        freeexprdata(scip, nlhdlr, expr, nlhdlrexprdata)?;
    }

    Ok(())
}

/// `SCIPnlhdlrInit`: call the initialization callback of a nonlinear
/// handler.
pub fn nlhdlr_init(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    let h = &mut scip.nlhdlrs[nlhdlr];
    h.nenfocalls = 0;
    h.nintevalcalls = 0;
    h.npropcalls = 0;
    h.nseparated = 0;
    h.ncutoffs = 0;
    h.ndomreds = 0;
    h.nbranchscores = 0;
    h.ndetections = 0;
    h.ndetectionslast = 0;

    h.detecttime.reset();
    h.enfotime.reset();
    h.proptime.reset();
    h.intevaltime.reset();

    if let Some(init) = h.init {
        init(scip, nlhdlr)?;
    }

    Ok(())
}

/// `SCIPnlhdlrExit`: call the deinitialization callback of a nonlinear
/// handler.
pub fn nlhdlr_exit(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    if let Some(exit) = scip.nlhdlrs[nlhdlr].exit {
        exit(scip, nlhdlr)?;
    }

    Ok(())
}

/// `SCIPnlhdlrDetect`: call the detect callback of a nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_detect(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    let te = scip.set.time_enabled;
    scip.nlhdlrs[nlhdlr].detecttime.start(te);
    let detect = scip.nlhdlrs[nlhdlr].detect;
    detect(scip, conshdlr, nlhdlr, expr, cons, enforcing, participating, nlhdlrexprdata)?;
    scip.nlhdlrs[nlhdlr].detecttime.stop(te);

    if *participating != NLHDLR_METHOD_NONE {
        scip.nlhdlrs[nlhdlr].ndetections += 1;
        scip.nlhdlrs[nlhdlr].ndetectionslast += 1;
    }

    Ok(())
}

/// `SCIPnlhdlrEvalaux`: call the auxiliary evaluation callback of a
/// nonlinear handler.
pub fn nlhdlr_evalaux(scip: &mut Scip, nlhdlr: NlhdlrId, expr: ExprId, nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    let evalaux = scip.nlhdlrs[nlhdlr].evalaux;
    evalaux(scip, nlhdlr, expr, nlhdlrexprdata, auxvalue, sol)
}

/// `SCIPnlhdlrInteval`: call the interval evaluation callback of a nonlinear
/// handler.
pub fn nlhdlr_inteval(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    interval: &mut Interval,
    intevalvar: Option<ExprIntEvalVar>,
    intevalvardata: usize,
) -> ScipResult {
    if let Some(inteval) = scip.nlhdlrs[nlhdlr].inteval {
        let te = scip.set.time_enabled;
        scip.nlhdlrs[nlhdlr].intevaltime.start(te);
        inteval(scip, nlhdlr, expr, nlhdlrexprdata, interval, intevalvar, intevalvardata)?;
        scip.nlhdlrs[nlhdlr].intevaltime.stop(te);

        scip.nlhdlrs[nlhdlr].nintevalcalls += 1;
    }

    Ok(())
}

/// `SCIPnlhdlrReverseprop`: call the reverse propagation callback of a
/// nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_reverseprop(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    bounds: Interval,
    infeasible: &mut bool,
    nreductions: &mut i32,
) -> ScipResult {
    let Some(reverseprop) = scip.nlhdlrs[nlhdlr].reverseprop else {
        *infeasible = false;
        *nreductions = 0;

        return Ok(());
    };

    let te = scip.set.time_enabled;
    scip.nlhdlrs[nlhdlr].proptime.start(te);
    reverseprop(scip, conshdlr, nlhdlr, expr, nlhdlrexprdata, bounds, infeasible, nreductions)?;
    scip.nlhdlrs[nlhdlr].proptime.stop(te);

    // Update statistics.
    let h = &mut scip.nlhdlrs[nlhdlr];
    h.ndomreds += *nreductions as i64;
    if *infeasible {
        h.ncutoffs += 1;
    }
    h.npropcalls += 1;

    Ok(())
}

/// `SCIPnlhdlrInitsepa`: call the separation initialization callback of a
/// nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_initsepa(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    overestimate: bool,
    underestimate: bool,
    infeasible: &mut bool,
) -> ScipResult {
    let Some(initsepa) = scip.nlhdlrs[nlhdlr].initsepa else {
        *infeasible = false;
        return Ok(());
    };

    let te = scip.set.time_enabled;
    scip.nlhdlrs[nlhdlr].enfotime.start(te);
    initsepa(scip, conshdlr, cons, nlhdlr, expr, nlhdlrexprdata, overestimate, underestimate, infeasible)?;
    scip.nlhdlrs[nlhdlr].enfotime.stop(te);

    scip.nlhdlrs[nlhdlr].nenfocalls += 1;
    if *infeasible {
        scip.nlhdlrs[nlhdlr].ncutoffs += 1;
    }

    Ok(())
}

/// `SCIPnlhdlrExitsepa`: call the separation deinitialization callback of a
/// nonlinear handler.
pub fn nlhdlr_exitsepa(scip: &mut Scip, nlhdlr: NlhdlrId, expr: ExprId, nlhdlrexprdata: usize) -> ScipResult {
    if let Some(exitsepa) = scip.nlhdlrs[nlhdlr].exitsepa {
        let te = scip.set.time_enabled;
        scip.nlhdlrs[nlhdlr].enfotime.start(te);
        exitsepa(scip, nlhdlr, expr, nlhdlrexprdata)?;
        scip.nlhdlrs[nlhdlr].enfotime.stop(te);
    }

    Ok(())
}

/// `SCIPnlhdlrEnfo`: call the enforcement callback of a nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_enfo(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    allowweakcuts: bool,
    separated: bool,
    addbranchscores: bool,
    branchcandonly: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    let Some(enfo) = scip.nlhdlrs[nlhdlr].enfo else {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    };

    // Check that auxvalue is correct by reevaluating (the reference is built
    // without NDEBUG).
    {
        let mut auxvaluetest = 0.0;
        nlhdlr_evalaux(scip, nlhdlr, expr, nlhdlrexprdata, &mut auxvaluetest, sol)?;
        // We should get EXACTLY the same value from calling evalaux with the
        // same solution as before.
        debug_assert!(auxvalue == auxvaluetest);
    }

    let te = scip.set.time_enabled;
    scip.nlhdlrs[nlhdlr].enfotime.start(te);
    enfo(
        scip,
        conshdlr,
        cons,
        nlhdlr,
        expr,
        nlhdlrexprdata,
        sol,
        auxvalue,
        overestimate,
        allowweakcuts,
        separated,
        addbranchscores,
        branchcandonly,
        result,
    )?;
    scip.nlhdlrs[nlhdlr].enfotime.stop(te);

    // Update statistics.
    let h = &mut scip.nlhdlrs[nlhdlr];
    h.nenfocalls += 1;
    match *result {
        ScipResultCode::Separated => h.nseparated += 1,
        ScipResultCode::Branched => h.nbranchscores += 1,
        ScipResultCode::Cutoff => h.ncutoffs += 1,
        ScipResultCode::ReducedDom => h.ndomreds += 1,
        _ => {}
    }

    Ok(())
}

/// `SCIPnlhdlrEstimate`: call the estimator callback of a nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_estimate(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    targetvalue: f64,
    addbranchscores: bool,
    rowpreps: &mut Vec<RowPrep>,
    success: &mut bool,
    addedbranchscores: &mut bool,
) -> ScipResult {
    let Some(estimate) = scip.nlhdlrs[nlhdlr].estimate else {
        *success = false;
        *addedbranchscores = false;
        return Ok(());
    };

    // Check that auxvalue is correct by reevaluating (the reference is built
    // without NDEBUG).
    {
        let mut auxvaluetest = 0.0;
        nlhdlr_evalaux(scip, nlhdlr, expr, nlhdlrexprdata, &mut auxvaluetest, sol)?;
        // We should get EXACTLY the same value from calling evalaux with the
        // same solution as before.
        debug_assert!(auxvalue == auxvaluetest);
    }

    let te = scip.set.time_enabled;
    scip.nlhdlrs[nlhdlr].enfotime.start(te);
    estimate(
        scip,
        conshdlr,
        nlhdlr,
        expr,
        nlhdlrexprdata,
        sol,
        auxvalue,
        overestimate,
        targetvalue,
        addbranchscores,
        rowpreps,
        success,
        addedbranchscores,
    )?;
    scip.nlhdlrs[nlhdlr].enfotime.stop(te);

    // Update statistics.
    scip.nlhdlrs[nlhdlr].nenfocalls += 1;

    Ok(())
}

/// `SCIPnlhdlrSollinearize`: call the solution notification callback of a
/// nonlinear handler.
#[allow(clippy::too_many_arguments)]
pub fn nlhdlr_sollinearize(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    solisbest: bool,
    overestimate: bool,
    underestimate: bool,
) -> ScipResult {
    let Some(sollinearize) = scip.nlhdlrs[nlhdlr].sollinearize else {
        return Ok(());
    };

    sollinearize(scip, conshdlr, cons, nlhdlr, expr, nlhdlrexprdata, sol, solisbest, overestimate, underestimate)
}

/// `SCIPnlhdlrResetNDetectionslast`: reset number of detections counter for
/// last round.
pub fn nlhdlr_reset_n_detectionslast(scip: &mut Scip, nlhdlr: NlhdlrId) {
    scip.nlhdlrs[nlhdlr].ndetectionslast = 0;
}

/// `SCIPnlhdlrIncrementNCutoffs`: increments number of cutoffs in
/// statistics.
pub fn nlhdlr_increment_n_cutoffs(scip: &mut Scip, nlhdlr: NlhdlrId) {
    scip.nlhdlrs[nlhdlr].ncutoffs += 1;
}

/// `SCIPnlhdlrIncrementNSeparated`: increments number of separations in
/// statistics.
pub fn nlhdlr_increment_n_separated(scip: &mut Scip, nlhdlr: NlhdlrId) {
    scip.nlhdlrs[nlhdlr].nseparated += 1;
}

/// The nonlinear handler expression data with the given id, as `T`.
pub fn exprdata<T: 'static>(scip: &Scip, nlhdlrexprdata: usize) -> &T {
    scip.nlhdlrexprdatas[nlhdlrexprdata].downcast_ref::<T>().unwrap()
}

/// The nonlinear handler expression data with the given id, as mutable `T`.
pub fn exprdata_mut<T: 'static>(scip: &mut Scip, nlhdlrexprdata: usize) -> &mut T {
    scip.nlhdlrexprdatas[nlhdlrexprdata].downcast_mut::<T>().unwrap()
}

/// The nonlinear handler's data, as `T`.
pub fn hdlrdata<T: 'static>(scip: &Scip, nlhdlr: NlhdlrId) -> &T {
    scip.nlhdlrs[nlhdlr].data.as_ref().unwrap().downcast_ref::<T>().unwrap()
}

/// The nonlinear handler's data, as mutable `T`.
pub fn hdlrdata_mut<T: 'static>(scip: &mut Scip, nlhdlr: NlhdlrId) -> &mut T {
    scip.nlhdlrs[nlhdlr].data.as_mut().unwrap().downcast_mut::<T>().unwrap()
}
